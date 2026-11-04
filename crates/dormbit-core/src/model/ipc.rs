//! The §8 data models and the argument/result types of the 28 commands.
//!
//! These start minimal. The owning spec finalizes each one and updates Rust, Zod, the
//! MockClient data, and the Parity_Harness strategies in the same commit. Timestamps are
//! RFC 3339 UTC strings. IDs are UUID strings. `Option` fields serialize as `null` (never
//! omitted), so TypeScript sees `T | null`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::error::DormbitError;
use super::id::{AdapterId, CapsuleId, OpId, PlanId, ProjectId, ScopeId};
use super::num::SafeU64;

// ---- jobs ------------------------------------------------------------------------------------

/// Streamed over a Channel by every job command. A job ends with exactly one of `done`,
/// `failed`, or `cancelled`. Progress counts are real file and byte counts, never estimates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum JobEvent<R> {
    State {
        state: String,
    },
    Progress {
        files_done: SafeU64,
        files_total: SafeU64,
        bytes_done: SafeU64,
        bytes_total: SafeU64,
    },
    Done {
        result: R,
    },
    Failed {
        error: DormbitError,
    },
    Cancelled,
}

// ---- scope -----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ScopeKind {
    Read,
    CapsuleLibrary,
    RestoreTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ScopeVia {
    NativePicker,
}

/// A folder the user granted. `root_display` is for humans only (home shown as `~`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ScopeGrant {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub root_display: String,
    pub granted_at: String,
    pub via: ScopeVia,
}

// ---- findings and projects -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EvidenceRef {
    /// Path relative to the project root.
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FindingStatus {
    Confirmed,
    Inferred,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Finding {
    pub id: String,
    pub rule: String,
    pub statement: String,
    pub status: FindingStatus,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NamedValue {
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GitHead {
    /// `None` when detached.
    pub branch: Option<String>,
    pub detached: bool,
    pub commit: String,
    pub time: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GitUpstream {
    pub ahead: SafeU64,
    pub behind: SafeU64,
    pub as_of_last_fetch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GitStatus {
    pub modified: SafeU64,
    pub staged: SafeU64,
    pub untracked: SafeU64,
    pub deleted: SafeU64,
    pub conflicted: SafeU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GitSnapshot {
    pub head: GitHead,
    pub upstream: Option<GitUpstream>,
    pub status: GitStatus,
    pub stash_count: SafeU64,
    pub worktree_external: bool,
    pub submodules: Vec<String>,
    pub lfs_detected: bool,
    pub risky_config: Vec<String>,
    /// Remote URLs with credentials removed.
    pub remotes_redacted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Activity {
    pub latest_mtime: Option<String>,
    pub scanned: SafeU64,
    pub partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SizeInfo {
    pub total: SafeU64,
    pub rebuildable: SafeU64,
    pub partial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Marks {
    pub dormant: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Preservation {
    pub last_capsule: Option<CapsuleId>,
    pub last_verified_at: Option<String>,
    pub last_verified_restore: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectRecord {
    pub id: ProjectId,
    pub scope_id: ScopeId,
    /// Path relative to the scope root. Never absolute.
    pub rel_root: String,
    pub name: NamedValue,
    pub stacks: Vec<Finding>,
    pub git: Option<GitSnapshot>,
    pub activity: Activity,
    pub size: SizeInfo,
    pub docs: Vec<String>,
    pub manifests: Vec<String>,
    pub agent_artifacts: Vec<String>,
    pub marks: Marks,
    pub preservation: Preservation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DiscoveryResult {
    pub projects_found: SafeU64,
    pub entries_scanned: SafeU64,
}

// ---- plan ------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Decision {
    Include,
    Exclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PlanItemKind {
    File,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanItem {
    /// Path relative to the project root. Never absolute.
    pub path: String,
    pub kind: PlanItemKind,
    pub size: SafeU64,
    pub decision: Decision,
    pub reason: String,
    pub editable: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanGroup {
    pub name: String,
    pub file_count: SafeU64,
    pub bytes: SafeU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GitTreatment {
    MetadataOnly,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanTotals {
    pub files: SafeU64,
    pub bytes: SafeU64,
    pub included_files: SafeU64,
    pub included_bytes: SafeU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CapturePlan {
    pub plan_id: PlanId,
    pub items: Vec<PlanItem>,
    pub groups: Vec<PlanGroup>,
    pub git_treatment: GitTreatment,
    pub totals: PlanTotals,
    /// SHA-256 over the included items; approval must present it back.
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PlanApproveInput {
    pub digest: String,
    pub acks: Vec<String>,
    pub overrides: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApprovedPlan {
    pub digest: String,
    pub approved_at: String,
    pub acks: Vec<String>,
    pub overrides: Vec<String>,
}

// ---- capsules and verification ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CapsuleRef {
    pub id: CapsuleId,
    pub project_name: String,
    pub created_at: String,
    pub size: SafeU64,
    pub file_count: SafeU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CaptureResult {
    pub capsule: CapsuleRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CheckStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum RegistryMatch {
    Match,
    Mismatch,
    NotInRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VerificationCheck {
    pub id: String,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VerificationReport {
    pub capsule_id: CapsuleId,
    pub file_sha256: String,
    pub manifest_sha256: String,
    pub checks: Vec<VerificationCheck>,
    pub failed_files: Vec<String>,
    pub registry_match: RegistryMatch,
    pub status: CheckStatus,
}

// ---- restore ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum RestoreState {
    Staged,
    Written,
    Verified,
    Committed,
    Cleaned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RestoreJournal {
    pub op_id: OpId,
    pub staging: String,
    pub owner_token: String,
    pub state: RestoreState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RestoreInput {
    pub capsule_id: CapsuleId,
    pub target_scope_id: ScopeId,
    /// Name of the new folder to create inside the target. Never a path.
    pub dest_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PrecheckReport {
    pub ok: bool,
    pub issues: Vec<String>,
    pub required_bytes: SafeU64,
    pub available_bytes: SafeU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RestoreReport {
    pub op_id: OpId,
    pub capsule_id: CapsuleId,
    pub destination_display: String,
    pub files_restored: SafeU64,
    pub bytes_restored: SafeU64,
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RecoverAction {
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecoverRequest {
    pub op_id: OpId,
    pub action: RecoverAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecoverResult {
    pub op_id: OpId,
    pub state: RestoreState,
}

// ---- brief, settings, consent, adapters ------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BriefSection {
    pub title: String,
    /// Markdown. Rendered without raw HTML.
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Outcome {
    Preserved,
    Partial,
    NotCaptured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BriefOutcomes {
    pub files: Outcome,
    pub context: Outcome,
    pub session: Outcome,
    pub build: Outcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Brief {
    pub sections: Vec<BriefSection>,
    pub outcomes: BriefOutcomes,
    pub evidence_index: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExportResult {
    pub file_name: String,
    pub bytes: SafeU64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Settings {
    pub reduce_motion: bool,
    pub capsule_max_mb: SafeU64,
}

/// `null` means "leave unchanged".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsPatch {
    pub reduce_motion: Option<bool>,
    pub capsule_max_mb: Option<SafeU64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConsentRecord {
    pub adapter_id: AdapterId,
    pub granted: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AdapterSupport {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AdapterInfo {
    pub id: AdapterId,
    pub name: String,
    pub support: AdapterSupport,
    pub granted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_fields_serialize_as_null_not_omitted() {
        let p = Preservation {
            last_capsule: None,
            last_verified_at: None,
            last_verified_restore: None,
        };
        let v = serde_json::to_value(p).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 3);
        assert!(v["last_capsule"].is_null());
    }

    #[test]
    fn scope_kind_uses_the_plan_spelling() {
        assert_eq!(
            serde_json::to_string(&ScopeKind::CapsuleLibrary).unwrap(),
            "\"CapsuleLibrary\""
        );
        assert_eq!(
            serde_json::to_string(&RegistryMatch::NotInRegistry).unwrap(),
            "\"NotInRegistry\""
        );
    }

    #[test]
    fn job_events_are_tagged_with_kind() {
        let done: JobEvent<DiscoveryResult> = JobEvent::Done {
            result: DiscoveryResult {
                projects_found: SafeU64::new(3).unwrap(),
                entries_scanned: SafeU64::new(120).unwrap(),
            },
        };
        let v = serde_json::to_value(&done).unwrap();
        assert_eq!(v["kind"], "done");
        assert_eq!(v["result"]["projects_found"], 3);
        let c: JobEvent<DiscoveryResult> = serde_json::from_str(r#"{"kind":"cancelled"}"#).unwrap();
        assert_eq!(c, JobEvent::Cancelled);
    }

    #[test]
    fn a_patch_with_values_keeps_them() {
        let p: SettingsPatch =
            serde_json::from_str(r#"{"reduce_motion":true,"capsule_max_mb":null}"#).unwrap();
        assert_eq!(p.reduce_motion, Some(true));
        assert_eq!(p.capsule_max_mb, None);
    }
}
