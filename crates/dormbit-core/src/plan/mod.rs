//! The capture plan: classify a project, attach decisions, digest, edit, approve.
//!
//! Pure logic over a [`ReadVfs`]. The digest covers every item's decision, so an approval
//! only counts for exactly the plan the user was shown.

mod classify;
mod digest;
mod edit;
mod gitignore;
mod reason;
mod secrets;

pub use classify::{
    Candidate, REBUILDABLE_DIRS, classify_tree, expand_dir, sensitive_name, sensitive_pem,
};
pub use digest::plan_digest;
pub use edit::{ack_id, apply_edit, approve, check_well_formed};
pub use reason::{Group, ReasonCode};

use crate::capsule::CapsuleLimits;
use crate::model::{
    CapturePlan, Decision, DormbitError, GitTreatment, PlanGroup, PlanId, PlanItem, PlanItemKind,
    PlanTotals, SafeU64,
};
use crate::vfs::{IdGen, Limited, ReadVfs, RelPath};
use secrets::{Scan, scan_secrets};

/// Files larger than this are not secret-scanned (counted in `secret_scan_skipped`).
const SCAN_LIMIT: u64 = 1 << 20;

pub(crate) fn safe(v: u64) -> SafeU64 {
    SafeU64::saturating(v)
}

/// Classifies the project, scans included text files for secret-like content, and returns a
/// plan whose digest is already computed.
pub fn build_plan<V: ReadVfs, I: IdGen>(
    vfs: &V,
    treatment: GitTreatment,
    limits: &CapsuleLimits,
    ids: &I,
) -> Result<CapturePlan, DormbitError> {
    limits.validate()?;
    let candidates = classify::classify_tree(vfs, treatment, limits)?;
    let mut skipped = 0u64;
    let mut items = Vec::with_capacity(candidates.len());
    for c in candidates {
        let decision = c.reason.default_decision();
        let mut warning = c.warning.clone();
        // Git object storage and the index are compressed or binary; everything else in
        // `.git` (config, refs, logs, HEAD) is scanned like working-tree text (Requirement 3.5).
        let git_binary = c.path == ".git/index" || c.path.starts_with(".git/objects/");
        let wants_scan = c.kind == PlanItemKind::File && decision != Decision::Exclude;
        if wants_scan && git_binary {
            skipped += 1;
        } else if wants_scan {
            let rel = RelPath::parse(&c.path)?;
            match vfs
                .read_limited(&rel, SCAN_LIMIT)
                .map_err(|e| e.to_error(&c.path))?
            {
                Limited::TooLarge { .. } => skipped += 1,
                Limited::Complete(bytes) => match scan_secrets(&bytes) {
                    Scan::Clean => {}
                    Scan::SkippedBinary => skipped += 1,
                    Scan::Hit(h) => {
                        if warning.is_none() {
                            warning = Some(format!("secret_like_content:{}", h.line));
                        }
                    }
                },
            }
        }
        items.push(PlanItem {
            path: c.path,
            kind: c.kind,
            size: safe(c.size),
            decision,
            reason: c.reason.as_str().to_owned(),
            editable: c.reason.editable(),
            warning,
        });
    }
    let mut plan = CapturePlan {
        plan_id: PlanId(ids.new_uuid()),
        items,
        groups: Vec::new(),
        git_treatment: treatment,
        totals: PlanTotals {
            files: safe(0),
            bytes: safe(0),
            included_files: safe(0),
            included_bytes: safe(0),
            secret_scan_skipped: safe(skipped),
        },
        digest: String::new(),
    };
    refresh(&mut plan);
    Ok(plan)
}

/// Recomputes groups, totals, and the digest after any change to `items`.
pub(crate) fn refresh(plan: &mut CapturePlan) {
    let skipped = plan.totals.secret_scan_skipped;
    let (mut files, mut bytes, mut inc_files, mut inc_bytes) = (0u64, 0u64, 0u64, 0u64);
    let mut per: std::collections::BTreeMap<Group, (u64, u64)> = Default::default();
    for it in &plan.items {
        let size = it.size.get();
        files += 1;
        bytes = bytes.saturating_add(size);
        if it.decision == Decision::Include {
            inc_files += 1;
            inc_bytes = inc_bytes.saturating_add(size);
        }
        let group = group_of(it);
        let e = per.entry(group).or_insert((0, 0));
        e.0 += 1;
        e.1 = e.1.saturating_add(size);
    }
    plan.groups = Group::ALL
        .iter()
        .filter_map(|g| {
            per.get(g).map(|(n, b)| PlanGroup {
                name: g.label().to_owned(),
                file_count: safe(*n),
                bytes: safe(*b),
            })
        })
        .collect();
    plan.totals = PlanTotals {
        files: safe(files),
        bytes: safe(bytes),
        included_files: safe(inc_files),
        included_bytes: safe(inc_bytes),
        secret_scan_skipped: skipped,
    };
    plan.digest = plan_digest(plan);
}

/// The group an item is shown in. An undecided item always sits in "Needs decision", and an
/// edited item moves with its decision.
pub fn group_of(it: &PlanItem) -> Group {
    let base = ReasonCode::parse(&it.reason).map_or(Group::Included, ReasonCode::group);
    match (it.decision, base) {
        (Decision::Undecided, _) => Group::NeedsDecision,
        (Decision::Include, Group::NeedsDecision | Group::ExcludedRebuildable) => Group::Included,
        (Decision::Exclude, Group::Included | Group::NeedsDecision) => Group::ExcludedByYou,
        (_, g) => g,
    }
}

#[cfg(test)]
mod tests;
