//! Property 7, plan side: sensitive files can never be included (Requirements 2.1-2.9).
//! Task 2.11 extends this to the manifest.

#[path = "support/capture_kit.rs"]
mod kit;
#[path = "support/plan_trees.rs"]
mod plan_trees;

use dormbit_core::model::{Decision, ErrorCode};
use dormbit_core::plan::{apply_edit, sensitive_name};
use dormbit_core::vfs::MemVfs;
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

proptest! {
    /// Task 2.11: no sensitive path reaches `entries`, including files under a rebuildable
    /// folder the user included (for example `node_modules/pkg/.env`).
    #[test]
    fn no_sensitive_file_enters_a_capsule(entries in tree(), t in treatment(), include_nm in any::<bool>()) {
        let mut v = MemVfs::new();
        for &(d, n, sensitive) in &entries {
            let name = if sensitive { plan_trees::SENSITIVE[n] } else { plan_trees::ORDINARY[n] };
            v.add_file(&format!("{}{}", plan_trees::DIRS[d], name), "plain text\n");
        }
        let has_nm = entries.iter().any(|&(d, _, _)| plan_trees::DIRS[d].starts_with("node_modules"));
        let includes: Vec<&str> = if include_nm && has_nm { vec!["node_modules"] } else { vec![] };
        let (plan, ok) = kit::approved(&v, t, &includes);
        let (out, _) = kit::capture(&v, plan, ok);
        for e in &out.manifest.entries {
            prop_assert!(!is_sensitive(e.path()), "{} was captured", e.path());
        }
    }
}
