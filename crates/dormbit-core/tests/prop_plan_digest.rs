//! Property 6: the plan digest tracks every decision (Requirements 5.6, 7.1-7.4).

#[path = "support/plan_trees.rs"]
mod plan_trees;

use dormbit_core::model::{Decision, GitTreatment};
use dormbit_core::plan::{apply_edit, plan_digest};
use plan_trees::{plan_for, treatment, tree};
use proptest::prelude::*;

proptest! {
    #[test]
    fn changing_any_editable_decision_changes_the_digest(
        entries in tree(),
        t in treatment(),
        pick in any::<prop::sample::Index>(),
    ) {
        let mut plan = plan_for(&entries, t);
        let editable: Vec<String> =
            plan.items.iter().filter(|i| i.editable).map(|i| i.path.clone()).collect();
        prop_assume!(!editable.is_empty());
        let path = pick.get(&editable).clone();
        let now = plan.items.iter().find(|i| i.path == path).map(|i| i.decision);
        let next = if now == Some(Decision::Include) { Decision::Exclude } else { Decision::Include };
        let before = plan.digest.clone();
        apply_edit(&mut plan, &path, next).unwrap_or_else(|e| panic!("{e}"));
        prop_assert_ne!(&before, &plan.digest);
        prop_assert_eq!(&plan.digest, &plan_digest(&plan));
    }

    #[test]
    fn the_git_treatment_is_part_of_the_digest(entries in tree()) {
        let full = plan_for(&entries, GitTreatment::Full);
        let wto = plan_for(&entries, GitTreatment::WorkingTreeOnly);
        prop_assert_ne!(full.digest, wto.digest);
    }

    #[test]
    fn the_same_tree_and_decisions_give_the_same_digest(entries in tree(), t in treatment()) {
        prop_assert_eq!(plan_for(&entries, t).digest, plan_for(&entries, t).digest);
    }
}
