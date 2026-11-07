//! User edits and approval.

use super::{ReasonCode, plan_digest, refresh, sensitive_name};
use crate::capsule::CapsuleLimits;
use crate::model::{ApprovedPlan, CapturePlan, Decision, DormbitError, ErrorCode, PlanItemKind};
use crate::util::time::rfc3339;
use crate::vfs::Clock;

/// Sets the decision of the item at `path` and recomputes groups, totals, and the digest.
///
/// Only `Include` and `Exclude` can be chosen; locked items refuse with `PLAN_ITEM_LOCKED`, and
/// a path that isn't in the plan refuses with `PATH_UNSAFE` (the UI only sends plan paths).
pub fn apply_edit(
    plan: &mut CapturePlan,
    path: &str,
    decision: Decision,
) -> Result<(), DormbitError> {
    if decision == Decision::Undecided {
        return Err(DormbitError::new(
            ErrorCode::PlanIncomplete,
            "An item can be included or excluded, not set back to undecided.",
        ));
    }
    let item = plan
        .items
        .iter_mut()
        .find(|it| it.path == path)
        .ok_or_else(|| {
            DormbitError::new(ErrorCode::PathUnsafe, "That path is not part of this plan.")
        })?;
    // Editability comes from the reason table, never from the stored flag, so a plan whose
    // fields were altered can't unlock an item. An unknown reason counts as locked.
    if !item_is_editable(&item.reason) {
        return Err(DormbitError::new(
            ErrorCode::PlanItemLocked,
            format!("{path} is locked by a safety rule and can't be changed."),
        ));
    }
    item.decision = decision;
    refresh(plan);
    Ok(())
}

fn item_is_editable(reason: &str) -> bool {
    ReasonCode::parse(reason).is_some_and(ReasonCode::editable)
}

/// Refuses a plan whose items disagree with the rules, even if its digest was recomputed:
/// an unknown reason, an `editable` flag that differs from the reason table, a locked item
/// whose decision isn't the rule's, or an included file with a sensitive name.
fn check_well_formed(plan: &CapturePlan) -> Result<(), DormbitError> {
    let bad = |path: &str| {
        DormbitError::new(
            ErrorCode::PlanItemLocked,
            format!("{path} doesn't match DORMBIT's safety rules. Build the plan again."),
        )
    };
    for it in &plan.items {
        let Some(reason) = ReasonCode::parse(&it.reason) else {
            return Err(bad(&it.path));
        };
        if it.editable != reason.editable()
            || (!reason.editable() && it.decision != reason.default_decision())
        {
            return Err(bad(&it.path));
        }
        let name = it.path.rsplit('/').next().unwrap_or(&it.path);
        if it.decision == Decision::Include && sensitive_name(name).is_some() {
            return Err(bad(&it.path));
        }
    }
    Ok(())
}

/// The acknowledgement ID for a warning: `<path>#<code>`, where the code is the warning text
/// before its first `:` (for example `secret_like_content`).
pub fn ack_id(path: &str, warning: &str) -> String {
    let code = warning.split(':').next().unwrap_or(warning);
    format!("{path}#{code}")
}

/// Approves `plan` as shown. Checks, in order:
/// 1. `shown_digest` matches the plan (`PLAN_DIGEST_MISMATCH`);
/// 2. nothing is undecided and every warning on an included item is acknowledged
///    (`PLAN_INCOMPLETE`);
/// 3. included entries and bytes fit the limits (`CAPSULE_LIMIT_EXCEEDED`). Included folders
///    are expanded at capture time, where the writer checks the limits again.
pub fn approve<C: Clock>(
    plan: &CapturePlan,
    shown_digest: &str,
    acks: &[String],
    clock: &C,
    limits: &CapsuleLimits,
) -> Result<ApprovedPlan, DormbitError> {
    let digest = plan_digest(plan);
    if digest != shown_digest || plan.digest != digest {
        return Err(DormbitError::new(
            ErrorCode::PlanDigestMismatch,
            "The plan changed after it was shown. Review it again before approving.",
        ));
    }

    check_well_formed(plan)?;

    let undecided = plan
        .items
        .iter()
        .filter(|it| it.decision == Decision::Undecided)
        .count();
    if undecided > 0 {
        return Err(DormbitError::new(
            ErrorCode::PlanIncomplete,
            format!("{undecided} item(s) still need a decision."),
        ));
    }
    let mut needed: Vec<String> = plan
        .items
        .iter()
        .filter(|it| it.decision == Decision::Include)
        .filter_map(|it| it.warning.as_deref().map(|w| ack_id(&it.path, w)))
        .collect();
    needed.sort();
    needed.dedup();
    let missing = needed.iter().filter(|id| !acks.contains(id)).count();
    if missing > 0 {
        return Err(DormbitError::new(
            ErrorCode::PlanIncomplete,
            format!("{missing} warning(s) on included items are not acknowledged."),
        ));
    }

    let (mut entries, mut bytes) = (0u64, 0u64);
    for it in plan
        .items
        .iter()
        .filter(|it| it.decision == Decision::Include)
    {
        if matches!(it.kind, PlanItemKind::File | PlanItemKind::Symlink) {
            entries += 1;
        }
        bytes = bytes.saturating_add(it.size.get());
    }
    if entries > limits.max_entries || bytes > limits.max_total_bytes {
        return Err(DormbitError::new(
            ErrorCode::CapsuleLimitExceeded,
            "The included files are over the capsule limits. Exclude some items or raise the limits.",
        ));
    }

    let mut overrides: Vec<String> = plan
        .items
        .iter()
        .filter(|it| {
            ReasonCode::parse(&it.reason).is_some_and(|r| r.default_decision() != it.decision)
        })
        .map(|it| it.path.clone())
        .collect();
    overrides.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    Ok(ApprovedPlan {
        digest,
        approved_at: rfc3339(clock.now_unix()),
        acks: needed,
        overrides,
    })
}
