//! Property 7, plan side: sensitive files can never be included (Requirements 2.1-2.9).
//! Task 2.11 extends this to the manifest.

#[path = "support/plan_trees.rs"]
mod plan_trees;

use dormbit_core::model::{Decision, ErrorCode};
use dormbit_core::plan::{apply_edit, sensitive_name};
use plan_trees::{plan_for, treatment, tree};
use proptest::prelude::*;

fn is_sensitive(path: &str) -> bool {
    sensitive_name(path.rsplit('/').next().unwrap_or(path)).is_some()
}

proptest! {
    #[test]
    fn every_include_attempt_on_a_sensitive_file_is_refused(
        entries in tree(),
        t in treatment(),
        edits in prop::collection::vec((any::<prop::sample::Index>(), any::<bool>()), 0..32),
    ) {
        let mut plan = plan_for(&entries, t);
        let paths: Vec<String> = plan.items.iter().map(|i| i.path.clone()).collect();
        for (pick, include) in edits {
            let path = pick.get(&paths);
            let d = if include { Decision::Include } else { Decision::Exclude };
            let r = apply_edit(&mut plan, path, d);
            if is_sensitive(path) && include {
                prop_assert_eq!(r.err().map(|e| e.code), Some(ErrorCode::PlanItemLocked));
            }
        }
        for it in plan.items.iter().filter(|i| is_sensitive(&i.path)) {
            prop_assert_eq!(it.decision, Decision::Exclude, "{}", it.path);
            prop_assert!(!it.editable);
        }
    }
}
