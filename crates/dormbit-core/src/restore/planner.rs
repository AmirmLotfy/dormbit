//! The Restore_Planner (design: Restore_Planner). Pure: the host gathers volume facts and the
//! destination parent's names; this decides. Every check runs and is reported, in a fixed
//! order, so the user sees every problem at once. A plan exists only if nothing blocking
//! failed.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::keys::{
    KeyMode, SafeName, SafePath, fold_key, nfc_key, path_key, validate_entry_path,
    validate_target_name,
};
use super::report::NotRestored;
use crate::capsule::manifest::{Encoding, Entry, Manifest};
use crate::capsule::verify::{RegistryState, Report, ReportStatus};
use crate::model::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeFacts {
    pub fs_type: String,
    pub block_size: u64,
    pub free_bytes: u64,
    pub case: CaseSensitivity,
}

/// Volume types with exclusive rename. `mem` is the in-memory filesystem of tests and the
/// web demo.
pub const SUPPORTED_VOLUMES: [&str; 3] = ["apfs", "hfs", "mem"];

pub struct PlanInput<'a> {
    pub manifest: &'a Manifest,
    /// SHA-256 of the manifest bytes read again at pre-check time.
    pub manifest_sha256_now: &'a str,
    pub verification: Option<&'a Report>,
    pub target_name: &'a str,
    pub target_display: String,
    pub parent_entries: &'a [String],
    pub volume: VolumeFacts,
    /// `None`: the source folder isn't known on this Mac.
    pub inside_source: Option<bool>,
    pub inside_library: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecheckState {
    Pass,
    Fail,
    Warn,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecheckItem {
    pub id: String,
    pub status: PrecheckState,
    pub code: Option<ErrorCode>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecheckReport {
    pub checks: Vec<PrecheckItem>,
    pub volume: VolumeFacts,
    pub required_bytes: u64,
    pub files_total: u64,
    pub bytes_total: u64,
    pub skipped: Vec<NotRestored>,
    pub target_display: String,
    pub blocking_failed: bool,
}

impl PrecheckReport {
    /// The first blocking failure's code, if any.
    pub fn first_error(&self) -> Option<ErrorCode> {
        self.checks
            .iter()
            .find(|c| c.status == PrecheckState::Fail)
            .and_then(|c| c.code)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: SafePath,
    pub size: u64,
    pub sha256: String,
    pub stored_sha256: String,
    pub stored_size: u64,
    pub encoding: Encoding,
    pub final_mode: u32,
    pub stripped_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    pub target_name: SafeName,
    pub capsule_id: String,
    pub manifest_sha256: String,
    /// Each folder once, parents first.
    pub dirs: Vec<SafePath>,
    pub files: Vec<PlannedFile>,
    pub skipped: Vec<NotRestored>,
    pub case_rules: CaseSensitivity,
    pub key_mode: KeyMode,
    pub required_bytes: u64,
    pub bytes_total: u64,
}

pub struct PrecheckOutcome {
    pub report: PrecheckReport,
    pub plan: Option<RestorePlan>,
}

struct Checks(Vec<PrecheckItem>);

impl Checks {
    fn put(&mut self, id: &str, status: PrecheckState, code: Option<ErrorCode>, detail: String) {
        self.0.push(PrecheckItem {
            id: id.to_owned(),
            status,
            code,
            detail,
        });
    }
    fn pass(&mut self, id: &str, detail: &str) {
        self.put(id, PrecheckState::Pass, None, detail.to_owned());
    }
    fn fail(&mut self, id: &str, code: ErrorCode, detail: String) {
        self.put(id, PrecheckState::Fail, Some(code), detail);
    }
}

fn round_up(n: u64, block: u64) -> u64 {
    let b = block.max(1);
    n.div_ceil(b).saturating_mul(b)
}

fn is_git_hooks(p: &SafePath) -> bool {
    let c = p.components();
    c.len() >= 2 && fold_key(c[0].as_str()) == ".git" && fold_key(c[1].as_str()) == "hooks"
}

fn mode_bits_removed(mode: u32) -> u32 {
    mode & 0o7000
}

pub fn plan_restore(input: &PlanInput<'_>) -> PrecheckOutcome {
    let m = input.manifest;
    let mut c = Checks(Vec::new());

    // 1. Capsule checks.
    match input.verification {
        Some(r) if r.status == ReportStatus::Verified => {
            c.pass("capsule_verified", "The capsule passed verification.");
        }
        Some(r) => c.fail(
            "capsule_verified",
            r.error_code.unwrap_or(ErrorCode::CapsuleHashMismatch),
            "The capsule didn't pass verification, so it can't be restored.".into(),
        ),
        None => c.fail(
            "capsule_verified",
            ErrorCode::CapsuleMalformed,
            "The capsule hasn't been verified yet. Verify it first.".into(),
        ),
    }
    let recorded = input
        .verification
        .and_then(|r| r.manifest_sha256.as_deref());
    if recorded == Some(input.manifest_sha256_now) {
        c.pass(
            "manifest_hash",
            "The manifest is the one that was verified.",
        );
    } else {
        c.fail(
            "manifest_hash",
            ErrorCode::CapsuleHashMismatch,
            "The capsule changed since it was verified. Verify it again.".into(),
        );
    }
    match input.verification.map(|r| r.registry_match) {
        Some(RegistryState::Match) => c.pass("registry_match", "Matches DORMBIT's record."),
        Some(RegistryState::Mismatch) => c.fail(
            "registry_match",
            ErrorCode::CapsuleHashMismatch,
            "The capsule doesn't match the hash DORMBIT recorded when it was made.".into(),
        ),
        _ => c.put(
            "registry_match",
            PrecheckState::Warn,
            None,
            "Not in this Mac's capsule list, so there's no record to compare.".into(),
        ),
    }

    // 2. Target name and variants.
    let target = validate_target_name(input.target_name);
    match &target {
        Ok(_) => c.pass("target_name_valid", "The folder name is valid."),
        Err(rule) => c.fail(
            "target_name_valid",
            ErrorCode::PathUnsafe,
            format!("The folder name {}.", rule.describe()),
        ),
    }
    let clash = input.parent_entries.iter().find(|e| {
        nfc_key(e) == nfc_key(input.target_name) || fold_key(e) == fold_key(input.target_name)
    });
    match clash {
        None => c.pass("target_absent", "Nothing with that name exists there."),
        Some(_) => c.fail(
            "target_absent",
            ErrorCode::RestoreDestExists,
            "Something with that name (or a case or accent variant of it) already exists there. DORMBIT never replaces anything.".into(),
        ),
    }

    // 3. Location.
    if input.inside_library {
        c.fail(
            "target_location",
            ErrorCode::ScopeDenied,
            "The destination is inside a capsule library. Choose another folder.".into(),
        );
    } else if input.inside_source == Some(true) {
        c.fail(
            "target_location",
            ErrorCode::ScopeDenied,
            "The destination is inside the original project. Choose another folder.".into(),
        );
    } else if input.inside_source.is_none() {
        c.put(
            "target_location",
            PrecheckState::Warn,
            None,
            "The original project isn't on this Mac, so the location couldn't be compared with it."
                .into(),
        );
    } else {
        c.pass(
            "target_location",
            "The destination is outside the project and library.",
        );
    }

    // 4. Volume.
    let v = &input.volume;
    if SUPPORTED_VOLUMES.contains(&v.fs_type.as_str()) {
        c.pass("volume_supported", "The volume supports exclusive rename.");
    } else {
        c.fail(
            "volume_supported",
            ErrorCode::RestoreVolumeUnsupported,
            "This volume type doesn't support the safe commit DORMBIT needs (APFS or HFS+).".into(),
        );
    }
    let key_mode = if v.case == CaseSensitivity::Sensitive {
        KeyMode::Nfc
    } else {
        KeyMode::NfcAndFold
    };

    // 5-6. Paths and skips.
    let mut skipped: Vec<NotRestored> = m
        .excluded
        .iter()
        .map(|e| NotRestored {
            path: e.path.clone(),
            reason: format!("excluded_at_capture:{}", e.reason),
        })
        .collect();
    let mut files = Vec::new();
    let mut bad_paths = Vec::new();
    for e in &m.entries {
        match e {
            Entry::Symlink { path, .. } => skipped.push(NotRestored {
                path: path.clone(),
                reason: "symlink_not_restored".into(),
            }),
            Entry::File {
                path,
                size,
                mode,
                sha256,
                object,
                ..
            } => match validate_entry_path(path) {
                Err(rule) => bad_paths.push(format!("{} {}", printable(path), rule.describe())),
                Ok(p) if is_git_hooks(&p) => skipped.push(NotRestored {
                    path: path.clone(),
                    reason: "git_hooks_never_restored".into(),
                }),
                Ok(p) => files.push(PlannedFile {
                    path: p,
                    size: *size,
                    sha256: sha256.clone(),
                    stored_sha256: object.stored_sha256.clone(),
                    stored_size: object.stored_size,
                    encoding: object.encoding,
                    final_mode: mode & 0o777,
                    stripped_bits: mode_bits_removed(*mode),
                }),
            },
        }
    }
    skipped.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    // 7. Collisions under the applicable key.
    let mut collisions = Vec::new();
    let mut by_key: BTreeMap<String, String> = BTreeMap::new();
    let mut dir_keys: BTreeMap<String, String> = BTreeMap::new();
    for f in &files {
        let k = path_key(&f.path, key_mode);
        if let Some(other) = by_key.insert(k, f.path.display()) {
            collisions.push((other, f.path.display()));
        }
        let comps = f.path.components();
        for i in 1..comps.len() {
            let dk = comps[..i]
                .iter()
                .map(|s| match key_mode {
                    KeyMode::Nfc => nfc_key(s.as_str()),
                    KeyMode::NfcAndFold => fold_key(s.as_str()),
                })
                .collect::<Vec<_>>()
                .join("/");
            let display = comps[..i]
                .iter()
                .map(SafeName::as_str)
                .collect::<Vec<_>>()
                .join("/");
            match dir_keys.get(&dk) {
                Some(existing) if *existing != display => {
                    collisions.push((format!("{existing}/"), format!("{display}/")));
                }
                Some(_) => {}
                None => {
                    dir_keys.insert(dk, display);
                }
            }
        }
    }
    for (k, dir) in &dir_keys {
        if let Some(file) = by_key.get(k) {
            collisions.push((file.clone(), format!("{dir}/")));
        }
    }

    // 5 (reported) and 7.
    if bad_paths.is_empty() {
        c.pass("paths_safe", "Every path stays inside the new folder.");
    } else {
        c.fail(
            "paths_safe",
            ErrorCode::PathUnsafe,
            format!("Unsafe path: {}.", bad_paths[0]),
        );
    }
    let case_detail = match v.case {
        CaseSensitivity::Sensitive => {
            "Case-sensitive volume: names that differ only by case are kept apart."
        }
        CaseSensitivity::Insensitive => {
            "Case-insensitive volume: names are compared ignoring case and accents."
        }
        CaseSensitivity::Unknown => {
            "Couldn't tell if the volume is case-sensitive, so names are compared ignoring case."
        }
    };
    let case_state = if v.case == CaseSensitivity::Unknown {
        PrecheckState::Warn
    } else {
        PrecheckState::Pass
    };
    c.put("case_sensitivity", case_state, None, case_detail.into());
    if collisions.is_empty() {
        c.pass(
            "no_collisions",
            "No two files would end up with the same name.",
        );
    } else {
        let (a, b) = &collisions[0];
        c.fail(
            "no_collisions",
            ErrorCode::RestoreCollision,
            format!(
                "{} and {} would be the same name on this volume.",
                printable(a),
                printable(b)
            ),
        );
    }

    // 8. Folders, sizes, free space.
    let mut dirs: BTreeSet<SafePath> = BTreeSet::new();
    for f in &files {
        let comps = f.path.components();
        for i in 1..comps.len() {
            if let Ok(p) = validate_entry_path(
                &comps[..i]
                    .iter()
                    .map(SafeName::as_str)
                    .collect::<Vec<_>>()
                    .join("/"),
            ) {
                dirs.insert(p);
            }
        }
    }
    let mut dirs: Vec<SafePath> = dirs.into_iter().collect();
    dirs.sort_by(|a, b| {
        a.components()
            .len()
            .cmp(&b.components().len())
            .then_with(|| a.cmp(b))
    });
    let bytes_total: u64 = files.iter().map(|f| f.size).fold(0, u64::saturating_add);
    let required = files
        .iter()
        .map(|f| round_up(f.size, v.block_size))
        .fold(0u64, u64::saturating_add)
        .saturating_add((dirs.len() as u64 + 3).saturating_mul(v.block_size.max(1)));
    if v.free_bytes >= required {
        c.pass("free_space", "There's enough free space.");
    } else {
        c.fail(
            "free_space",
            ErrorCode::RestoreInsufficientSpace,
            format!(
                "The restore needs {required} bytes and the volume has {} free.",
                v.free_bytes
            ),
        );
    }

    // Reorder into the documented order.
    let order = [
        "capsule_verified",
        "manifest_hash",
        "registry_match",
        "target_name_valid",
        "target_absent",
        "target_location",
        "volume_supported",
        "free_space",
        "case_sensitivity",
        "paths_safe",
        "no_collisions",
    ];
    let mut checks = c.0;
    checks.sort_by_key(|i| order.iter().position(|o| *o == i.id).unwrap_or(order.len()));
    let blocking_failed = checks.iter().any(|i| i.status == PrecheckState::Fail);

    let report = PrecheckReport {
        checks,
        volume: v.clone(),
        required_bytes: required,
        files_total: files.len() as u64,
        bytes_total,
        skipped: skipped.clone(),
        target_display: input.target_display.clone(),
        blocking_failed,
    };
    let plan = match (blocking_failed, target) {
        (false, Ok(target_name)) => Some(RestorePlan {
            target_name,
            capsule_id: m.capsule_id.clone(),
            manifest_sha256: input.manifest_sha256_now.to_owned(),
            dirs,
            files,
            skipped,
            case_rules: v.case,
            key_mode,
            required_bytes: required,
            bytes_total,
        }),
        _ => None,
    };
    PrecheckOutcome { report, plan }
}

fn printable(p: &str) -> String {
    p.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(200)
        .collect()
}
