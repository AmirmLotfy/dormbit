//! The capture plan for the fictional Dockline project, pinned as a golden snapshot.

#[path = "support/golden.rs"]
#[macro_use]
mod golden;
#[path = "support/dockline.rs"]
mod fixture;

use dormbit_core::capsule::CapsuleLimits;
use dormbit_core::model::{Decision, GitTreatment};
use dormbit_core::plan::build_plan;
use dormbit_core::vfs::CountingIds;
use fixture::dockline;

#[test]
fn dockline_plan_matches_its_snapshot() {
    let plan = build_plan(
        &dockline(),
        GitTreatment::Full,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    golden!("plan", "dockline_full", &plan);
}

#[test]
fn dockline_env_file_is_excluded_and_locked() {
    let plan = build_plan(
        &dockline(),
        GitTreatment::Full,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let env = plan
        .items
        .iter()
        .find(|i| i.path == ".env")
        .unwrap_or_else(|| panic!("no .env"));
    assert_eq!(env.decision, Decision::Exclude);
    assert!(!env.editable);
    let example = plan.items.iter().find(|i| i.path == ".env.example");
    assert_eq!(example.map(|i| i.decision), Some(Decision::Include));
}
