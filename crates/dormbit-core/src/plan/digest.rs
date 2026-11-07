//! The plan digest: SHA-256 over a canonical JSON form of every item and the Git treatment.
//!
//! Canonical form (version 1): `{"v":1,"git_treatment":..,"items":[{path,kind,size,decision,
//! reason,editable,warning}]}` with items sorted by path bytes and fields in that fixed order.
//! `plan_id`, groups, and totals are derived or random and are deliberately left out, so two
//! builds of the same tree with the same decisions give the same digest.

use serde::Serialize;

use crate::model::{CapturePlan, Decision, GitTreatment, PlanItemKind};
use crate::util::sha256_hex;

#[derive(Serialize)]
struct Canon<'a> {
    v: u32,
    git_treatment: GitTreatment,
    items: Vec<CanonItem<'a>>,
}

#[derive(Serialize)]
struct CanonItem<'a> {
    path: &'a str,
    kind: PlanItemKind,
    size: u64,
    decision: Decision,
    reason: &'a str,
    editable: bool,
    warning: Option<&'a str>,
}

/// Lowercase hex SHA-256 of the canonical form of `plan`.
pub fn plan_digest(plan: &CapturePlan) -> String {
    let mut items: Vec<CanonItem<'_>> = plan
        .items
        .iter()
        .map(|it| CanonItem {
            path: &it.path,
            kind: it.kind,
            size: it.size.get(),
            decision: it.decision,
            reason: &it.reason,
            editable: it.editable,
            warning: it.warning.as_deref(),
        })
        .collect();
    items.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    let canon = Canon {
        v: 1,
        git_treatment: plan.git_treatment,
        items,
    };
    // Serializing plain structs of strings, numbers, and unit enums can't fail; an empty
    // buffer would still give a digest that no real plan matches.
    let bytes = serde_json::to_vec(&canon).unwrap_or_default();
    sha256_hex(&bytes)
}
