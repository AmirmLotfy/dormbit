//! The Recovery_Manager (design: Recovery_Manager). Decides from what's on disk, not from the
//! journal state alone, because a crash can land between the rename and the `Committed`
//! record. Recovery never touches the target, and never deletes a staging folder whose owner
//! token doesn't match.

use super::fs::{MetaKind, OwnerCheckFailure, RestoreFs, check_owner_token};
use super::journal::{Journal, JournalStore};
use super::keys::{STAGING_PREFIX, internal_name, is_op_id, validate_target_name};
use super::report::{
    GitCheck, REPORT_SCHEMA, RESTORE_LIMITS, RefCheck, RestoreReport, RestoreStatus, SourceCheck,
    Totals, WorktreeCheck,
};
use crate::model::{DormbitError, ErrorCode, RestoreState};
use crate::util::time::rfc3339;
use crate::vfs::Clock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Not committed: the staging folder was removed.
    RolledBack,
    /// Committed before the crash: staging removed (if present); the folder needs checking.
    Committed,
    /// The owner token didn't match: nothing was deleted.
    LeftInPlace,
    /// The restore had already finished; its report is returned unchanged.
    AlreadyFinished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryOutcome {
    pub action: RecoveryAction,
    pub report: RestoreReport,
}

fn report(j: &Journal, status: RestoreStatus, warnings: Vec<String>, now: String) -> RestoreReport {
    RestoreReport {
        schema_version: REPORT_SCHEMA,
        op_id: j.op_id.clone(),
        capsule_id: j.capsule_id.clone(),
        target_display: j.target_name.clone(),
        started_at: j.created_at.clone(),
        finished_at: now,
        status,
        recovered: true,
        error: None,
        files_written: Totals::default(),
        files_verified: Totals::default(),
        mismatches: Vec::new(),
        not_restored: Vec::new(),
        executables: Vec::new(),
        stripped_mode_bits: Vec::new(),
        source_unchanged: SourceCheck::NotChecked {
            reason: "Recovered after an interruption.".into(),
        },
        writes_outside_destination: 0,
        git_check: GitCheck {
            refs: RefCheck::NotApplicable,
            worktree: WorktreeCheck::NotApplicable,
        },
        warnings,
        limits: RESTORE_LIMITS.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// Finishes one interrupted restore. `parent` is the destination parent from the journal's
/// grant. Returns what was done and the report to show.
pub fn recover<F: RestoreFs, J: JournalStore, C: Clock>(
    fs: &mut F,
    parent: &F::Dir,
    journals: &mut J,
    op_id: &str,
    clock: &C,
) -> Result<RecoveryOutcome, DormbitError> {
    let mut j = journals.load(op_id)?.ok_or_else(|| {
        DormbitError::new(
            ErrorCode::IoFailed,
            "There's no interrupted restore with that ID.",
        )
    })?;
    // The journal is DORMBIT's own state, but it's still checked: the staging name must be
    // exactly the one this op ID produces, so a damaged journal can't point anywhere else.
    if j.op_id != op_id || !is_op_id(op_id) || j.staging != format!("{STAGING_PREFIX}{op_id}") {
        return Err(DormbitError::new(
            ErrorCode::PathUnsafe,
            "The restore journal is damaged. Nothing was changed.",
        ));
    }
    // A restore that already finished keeps its report; recovery never rewrites it.
    if j.state == RestoreState::Cleaned
        && let Some(existing) = journals.report(op_id)?
    {
        return Ok(RecoveryOutcome {
            action: RecoveryAction::AlreadyFinished,
            report: existing,
        });
    }
    let now = rfc3339(clock.now_unix());
    let staging = internal_name(&j.staging);
    let target = validate_target_name(&j.target_name).map_err(|_| {
        DormbitError::new(
            ErrorCode::PathUnsafe,
            "The journal's target name is not valid.",
        )
    })?;

    // Is the target the folder this restore staged (same inode as the staged tree)?
    let target_is_ours = match (fs.lookup_nofollow(parent, target.as_str()), j.tree_id) {
        (Ok(Some(m)), Some(id)) => m.kind == MetaKind::Dir && m.dev == id.dev && m.ino == id.ino,
        _ => false,
    };
    let staging_present = matches!(
        fs.lookup_nofollow(parent, staging.as_str()),
        Ok(Some(m)) if m.kind == MetaKind::Dir
    );

    let mut warnings = Vec::new();
    let action = if staging_present {
        match check_owner_token(fs, parent, &staging, &j.owner_line()) {
            Err(OwnerCheckFailure::StagingMissing) => unreachable_staging(),
            Err(why) => {
                let msg = match why {
                    OwnerCheckFailure::TokenMismatch => {
                        "its owner token doesn't match this restore"
                    }
                    _ => "its owner token is missing",
                };
                warnings.push(format!(
                    "Not deleted: the staging folder was left in place because {msg}."
                ));
                let r = report(&j, RestoreStatus::RolledBack, warnings, now);
                return Ok(RecoveryOutcome {
                    action: RecoveryAction::LeftInPlace,
                    report: r,
                });
            }
            Ok(proof) => {
                let staging_dir = fs.open_dir_nofollow(parent, &staging).ok();
                let tree_in_staging = staging_dir
                    .as_ref()
                    .and_then(|d| fs.lookup_nofollow(d, "tree").ok().flatten())
                    .is_some();
                fs.remove_staging(parent, proof)
                    .map_err(|f| f.to_error("The staging folder"))?;
                if tree_in_staging || !target_is_ours {
                    if !tree_in_staging {
                        warnings.push("The staged tree was missing; nothing was restored.".into());
                    }
                    RecoveryAction::RolledBack
                } else {
                    RecoveryAction::Committed
                }
            }
        }
    } else if target_is_ours {
        RecoveryAction::Committed
    } else {
        RecoveryAction::RolledBack
    };

    j.state = RestoreState::Cleaned;
    j.updated_at = now.clone();
    journals.set_state(&j)?;
    let status = match action {
        RecoveryAction::Committed => {
            warnings.push(
                "The restore finished before the interruption. Check the restored folder from the Library.".into(),
            );
            RestoreStatus::CommittedUnverified
        }
        _ => RestoreStatus::RolledBack,
    };
    let r = report(&j, status, warnings, now);
    journals.put_report(op_id, &r)?;
    Ok(RecoveryOutcome { action, report: r })
}

/// `staging_present` was checked just before, so a missing folder here means it vanished in
/// between; treat it like the no-staging case on the next run.
fn unreachable_staging() -> RecoveryAction {
    RecoveryAction::RolledBack
}
