//! The restore report (design: RestoreReport, RST-7). Every field states what was checked;
//! anything not checked says so.

use serde::{Deserialize, Serialize};

use crate::model::DormbitError;

pub const REPORT_SCHEMA: u32 = 1;

/// Always part of a restore report (Requirement 14.13).
pub const RESTORE_LIMITS: [&str; 4] = [
    "Restore checks file bytes and modes against the capsule; it never runs project code.",
    "Builds/runs: unknown (never executed).",
    "Symlinks are recorded in the capsule, not restored.",
    "Empty folders are not preserved.",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreStatus {
    Verified,
    Failed,
    RolledBack,
    /// Recovery found the restore committed but the folder hasn't been re-checked yet.
    CommittedUnverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Totals {
    pub files: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mismatch {
    pub path: String,
    pub expected_sha256: String,
    pub actual_sha256: Option<String>,
    pub expected_size: u64,
    pub actual_size: Option<u64>,
    pub expected_mode: u32,
    pub actual_mode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotRestored {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrippedBits {
    pub path: String,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceCheck {
    Unchanged,
    Changed { paths: Vec<String> },
    NotChecked { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RefCheck {
    Match,
    Mismatch { refs: Vec<String> },
    NotApplicable,
    RefsNotRecorded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorktreeCheck {
    Match,
    Differs { detail: String },
    NotChecked { reason: String },
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCheck {
    pub refs: RefCheck,
    pub worktree: WorktreeCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub schema_version: u32,
    pub op_id: String,
    pub capsule_id: String,
    pub target_display: String,
    pub started_at: String,
    pub finished_at: String,
    pub status: RestoreStatus,
    pub recovered: bool,
    pub error: Option<DormbitError>,
    pub files_written: Totals,
    pub files_verified: Totals,
    pub mismatches: Vec<Mismatch>,
    pub not_restored: Vec<NotRestored>,
    pub executables: Vec<String>,
    pub stripped_mode_bits: Vec<StrippedBits>,
    pub source_unchanged: SourceCheck,
    pub writes_outside_destination: u64,
    pub git_check: GitCheck,
    pub warnings: Vec<String>,
    pub limits: Vec<String>,
}
