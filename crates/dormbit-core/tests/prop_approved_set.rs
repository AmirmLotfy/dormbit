//! Property 3 (PROP-7): the capsule contains exactly the Approved set. Files added after
//! approval are never captured and are reported.

#[path = "support/capture_kit.rs"]
mod kit;

use std::collections::BTreeSet;

use dormbit_core::capsule::CapsuleLimits;
use dormbit_core::model::{Decision, GitTreatment, PlanItemKind};
use dormbit_core::plan::{ack_id, apply_edit, approve, build_plan};
use dormbit_core::vfs::{CountingIds, FixedClock, MemVfs};
use kit::*;
use proptest::prelude::*;

const FILES: [&str; 10] = [
    "a.txt",
    "b.rs",
    "src/c.rs",
    "src/d.ts",
    "docs/e.md",
    "f.json",
    "data/g.sqlite",
    "h.lock",
    "src/i.rs",
    "j.toml",
];

proptest! {
    #[test]
    fn captured_paths_equal_included_files_and_later_files_are_reported(
        present in prop::collection::btree_set(0..FILES.len(), 1..FILES.len()),
        flips in prop::collection::vec(any::<bool>(), FILES.len()),
        later in prop::collection::btree_set("[k-z]{1,6}\\.new", 0..4),
    ) {
        let mut v = MemVfs::new();
        for &i in &present {
            v.add_file(FILES[i], format!("content {i}\n"));
        }
        let mut plan = build_plan(&v, GitTreatment::Full, &CapsuleLimits::default(), &CountingIds::default())
            .unwrap_or_else(|e| panic!("{e}"));
        let paths: Vec<(String, bool)> = plan.items.iter().map(|i| (i.path.clone(), i.editable)).collect();
        for (k, (path, editable)) in paths.iter().enumerate() {
            if *editable {
                let d = if flips[k % flips.len()] { Decision::Include } else { Decision::Exclude };
                apply_edit(&mut plan, path, d).unwrap_or_else(|e| panic!("{e}"));
            }
        }
        let acks: Vec<String> = plan.items.iter()
            .filter_map(|i| i.warning.as_deref().map(|w| ack_id(&i.path, w))).collect();
        let ok = approve(&plan, &plan.digest, &acks, &FixedClock(NOW), &CapsuleLimits::default())
            .unwrap_or_else(|e| panic!("{e}"));
        let expected: BTreeSet<String> = plan.items.iter()
            .filter(|i| i.decision == Decision::Include && i.kind == PlanItemKind::File)
            .map(|i| i.path.clone()).collect();

        for p in &later {
            v.add_file(p, "added after approval\n");
        }
        let (out, _) = capture(&v, plan, ok);
        let got: BTreeSet<String> = out.manifest.entries.iter().map(|e| e.path().to_owned()).collect();
        prop_assert_eq!(got, expected);
        let reported: BTreeSet<String> = out.not_captured.iter().cloned().collect();
        prop_assert_eq!(reported, later);
    }
}
