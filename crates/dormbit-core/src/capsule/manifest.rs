//! `manifest.json`, schema version 1 (design: Manifest). Field order is the declaration order;
//! every field is required and unknown fields are rejected. No maps, no floats, so
//! `serde_json::to_vec` gives one canonical byte form.

use serde::{Deserialize, Serialize};

use crate::model::{DormbitError, ErrorCode, GitTreatment};

pub const SCHEMA_VERSION: u32 = 1;
pub const MARKER: &[u8] = b"dormbit-capsule/1\n";

/// The fixed v1 limitations, in order.
pub const LIMITATIONS: [&str; 5] = [
    "Not encrypted.",
    "Sensitive files are excluded and can't be included.",
    "Symlinks are recorded, not restored.",
    "Git hooks, lock files, and fsmonitor state are not captured.",
    "Empty folders are not preserved.",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub capsule_id: String,
    pub created_at: String,
    pub app_version: String,
    pub project: ManifestProject,
    pub plan_digest: String,
    pub git: Option<ManifestGit>,
    pub entries: Vec<Entry>,
    pub excluded: Vec<Excluded>,
    pub warnings: Vec<ManifestWarning>,
    pub limitations: Vec<String>,
    pub agent_refs: Vec<AgentRef>,
    pub totals: Totals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestProject {
    pub name: String,
    pub name_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestGit {
    pub treatment: GitTreatment,
    pub head: Option<HeadRef>,
    pub refs: Vec<RefEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadRef {
    pub branch: Option<String>,
    pub detached: bool,
    pub commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefEntry {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Entry {
    File {
        path: String,
        size: u64,
        mode: u32,
        mtime: i64,
        sha256: String,
        object: ObjectInfo,
    },
    Symlink {
        path: String,
        target: String,
        restored: bool,
    },
}

impl Entry {
    pub fn path(&self) -> &str {
        match self {
            Entry::File { path, .. } | Entry::Symlink { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Gzip,
    Identity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectInfo {
    pub encoding: Encoding,
    pub stored_size: u64,
    pub stored_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excluded {
    pub path: String,
    pub group: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestWarning {
    pub id: String,
    pub code: String,
    pub path: String,
    pub line: Option<u32>,
    pub acknowledged: bool,
}

/// Filled by the P1 agent adapters; always empty in v1 captures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRef {
    pub adapter_id: String,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    pub files: u64,
    pub symlinks: u64,
    pub objects: u64,
    pub bytes: u64,
    pub stored_bytes: u64,
    pub excluded: u64,
}

/// The `END` trailer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trailer {
    pub manifest_sha256: String,
    pub object_count: u64,
}

/// Canonical compact JSON bytes.
pub fn to_canonical<T: Serialize>(value: &T) -> Result<Vec<u8>, DormbitError> {
    serde_json::to_vec(value).map_err(|_| {
        DormbitError::new(
            ErrorCode::CapsuleMalformed,
            "DORMBIT could not write the capsule manifest.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_tagged_and_strict() {
        let e = Entry::Symlink {
            path: "a".into(),
            target: "b".into(),
            restored: false,
        };
        let s = String::from_utf8(to_canonical(&e).unwrap()).unwrap();
        assert_eq!(
            s,
            r#"{"type":"symlink","path":"a","target":"b","restored":false}"#
        );
        let extra = r#"{"type":"symlink","path":"a","target":"b","restored":false,"x":1}"#;
        assert!(serde_json::from_str::<Entry>(extra).is_err());
        let missing = r#"{"type":"symlink","path":"a","target":"b"}"#;
        assert!(serde_json::from_str::<Entry>(missing).is_err());
    }

    #[test]
    fn trailer_has_the_documented_shape() {
        let t = Trailer {
            manifest_sha256: "ab".into(),
            object_count: 3,
        };
        assert_eq!(
            to_canonical(&t).unwrap(),
            br#"{"manifest_sha256":"ab","object_count":3}"#
        );
    }
}
