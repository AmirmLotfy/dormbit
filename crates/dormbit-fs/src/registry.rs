//! The capsule registry: one JSON file in DORMBIT's own state folder (Application Support),
//! never inside a granted folder. Writes are atomic: a create-new temp file is synced and
//! then renamed over the previous registry, so a crash leaves either the old or new list.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};

use cap_std::fs::{Dir, OpenOptions};
use dormbit_core::capsule::{CapsuleRecord, Registry};
use dormbit_core::model::{CapsuleId, DormbitError, ErrorCode};

const FILE: &str = "capsules.json";
const MAX_BYTES: u64 = 64 << 20;

pub struct FsCapsuleRegistry {
    state: Dir,
    records: BTreeMap<String, CapsuleRecord>,
    counter: u64,
}

fn io_fail(_: std::io::Error) -> DormbitError {
    DormbitError::new(
        ErrorCode::IoFailed,
        "DORMBIT could not update its capsule list.",
    )
}

impl FsCapsuleRegistry {
    /// Loads the registry from `state`. A missing file is an empty registry.
    pub fn open(state: Dir) -> Result<Self, DormbitError> {
        let records = match state.open_with(FILE, OpenOptions::new().read(true)) {
            Ok(f) => {
                let mut buf = Vec::new();
                f.take(MAX_BYTES + 1)
                    .read_to_end(&mut buf)
                    .map_err(io_fail)?;
                if buf.len() as u64 > MAX_BYTES {
                    return Err(DormbitError::new(
                        ErrorCode::IoFailed,
                        "DORMBIT's capsule list is too large to read.",
                    ));
                }
                let list: Vec<CapsuleRecord> = serde_json::from_slice(&buf).map_err(|_| {
                    DormbitError::new(
                        ErrorCode::IoFailed,
                        "DORMBIT's capsule list is damaged. It was left unchanged.",
                    )
                })?;
                list.into_iter()
                    .map(|r| (r.capsule_id.as_str().to_owned(), r))
                    .collect()
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(io_fail(e)),
        };
        Ok(Self {
            state,
            records,
            counter: 0,
        })
    }

    pub fn list(&self) -> Vec<CapsuleRecord> {
        self.records.values().cloned().collect()
    }

    fn save(&mut self) -> Result<(), DormbitError> {
        let list: Vec<&CapsuleRecord> = self.records.values().collect();
        let bytes = serde_json::to_vec_pretty(&list).map_err(|_| {
            DormbitError::new(
                ErrorCode::IoFailed,
                "DORMBIT could not write its capsule list.",
            )
        })?;
        self.counter += 1;
        let tmp = format!("{FILE}.tmp-{}-{}", std::process::id(), self.counter);
        let mut o = OpenOptions::new();
        o.write(true).create_new(true);
        let mut f = self.state.open_with(&tmp, &o).map_err(io_fail)?;
        let written = f.write_all(&bytes).and_then(|()| f.sync_all());
        drop(f);
        // Replacing DORMBIT's own state file is intended; user files are never touched here.
        let renamed = written.and_then(|()| self.state.rename(&tmp, &self.state, FILE));
        if let Err(e) = renamed {
            // Only the temp file this call created is removed.
            let _ = self.state.remove_file(&tmp);
            return Err(io_fail(e));
        }
        if let Ok(d) = self.state.open(".") {
            let _ = d.sync_all();
        }
        Ok(())
    }

    pub fn set_last_verified(&mut self, id: &CapsuleId, at: i64) -> Result<(), DormbitError> {
        if let Some(r) = self.records.get_mut(id.as_str()) {
            r.last_verified_at = Some(at);
            self.save()?;
        }
        Ok(())
    }
}

impl Registry for FsCapsuleRegistry {
    fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError> {
        Ok(self.records.get(id.as_str()).cloned())
    }

    fn put(&mut self, record: CapsuleRecord) -> Result<(), DormbitError> {
        let key = record.capsule_id.as_str().to_owned();
        let previous = self.records.insert(key.clone(), record);
        if let Err(e) = self.save() {
            // Keep memory and disk in agreement.
            match previous {
                Some(p) => self.records.insert(key, p),
                None => self.records.remove(&key),
            };
            return Err(e);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;
    use dormbit_core::model::{ProjectId, ScopeId};

    fn rec(id: &str) -> CapsuleRecord {
        CapsuleRecord {
            capsule_id: CapsuleId::from(id),
            project_id: ProjectId::from("p"),
            project_name: "demo".into(),
            library_grant: ScopeId::from("lib"),
            file_name: format!("{id}.dormbit"),
            file_sha256: "a".repeat(64),
            manifest_sha256: "b".repeat(64),
            plan_digest: "c".repeat(64),
            size: 1024,
            file_count: 3,
            created_at: 1_800_000_000,
            last_verified_at: None,
        }
    }

    #[test]
    fn records_survive_a_reopen_and_no_temp_files_remain() {
        let t = tempfile::tempdir().unwrap();
        let open = || {
            FsCapsuleRegistry::open(Dir::open_ambient_dir(t.path(), ambient_authority()).unwrap())
                .unwrap()
        };
        let mut r = open();
        r.put(rec("one")).unwrap();
        r.put(rec("two")).unwrap();
        r.set_last_verified(&CapsuleId::from("one"), 42).unwrap();
        let r2 = open();
        assert_eq!(r2.list().len(), 2);
        assert_eq!(
            r2.get(&CapsuleId::from("one"))
                .unwrap()
                .unwrap()
                .last_verified_at,
            Some(42)
        );
        let files: Vec<_> = std::fs::read_dir(t.path()).unwrap().collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn a_damaged_registry_is_reported_and_left_unchanged() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join(FILE), b"{not json").unwrap();
        let d = Dir::open_ambient_dir(t.path(), ambient_authority()).unwrap();
        assert!(FsCapsuleRegistry::open(d).is_err());
        assert_eq!(std::fs::read(t.path().join(FILE)).unwrap(), b"{not json");
    }
}
