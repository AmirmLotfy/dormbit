//! Plan engine tests over an in-memory tree. Secret-like inputs are built at run time from
//! short fragments so no such literal is committed.

use super::*;
use crate::model::ErrorCode;
use crate::vfs::{CountingIds, FixedClock, MemVfs};

fn token() -> String {
    format!("{}{}", "gh", "p_") + &"aB3d".repeat(9)
}

fn tree() -> MemVfs {
    let mut v = MemVfs::new();
    v.add_file("src/main.rs", "fn main() {}\n")
        .add_file("README.md", "# demo\n")
        .add_file(".env", "X=1\n")
        .add_file("node_modules/pkg/index.js", "module.exports = 1;\n")
        .add_file("data/dev.sqlite", "SQLite format 3")
        .add_file(
            "config/app.toml",
            format!("name = \"x\"\nkey = \"{}\"\n", token()),
        );
    v
}

fn plan_of(v: &MemVfs) -> CapturePlan {
    build_plan(
        v,
        GitTreatment::Full,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap_or_else(|e| panic!("plan failed: {e}"))
}

fn item<'a>(p: &'a CapturePlan, path: &str) -> &'a PlanItem {
    p.items
        .iter()
        .find(|i| i.path == path)
        .unwrap_or_else(|| panic!("no item {path}"))
}

fn acks_for(p: &CapturePlan) -> Vec<String> {
    p.items
        .iter()
        .filter_map(|i| i.warning.as_deref().map(|w| ack_id(&i.path, w)))
        .collect()
}

#[test]
fn defaults_follow_the_reason_table() {
    let p = plan_of(&tree());
    assert_eq!(item(&p, "src/main.rs").decision, Decision::Include);
    let env = item(&p, ".env");
    assert_eq!(env.decision, Decision::Exclude);
    assert!(!env.editable);
    assert_eq!(item(&p, "node_modules").kind, PlanItemKind::Dir);
    assert_eq!(item(&p, "node_modules").decision, Decision::Exclude);
    assert_eq!(item(&p, "data/dev.sqlite").decision, Decision::Undecided);
    assert_eq!(p.plan_id.as_str(), "00000000-0000-4000-8000-000000000001");
}

#[test]
fn secret_like_content_is_a_warning_with_only_a_line_number() {
    let p = plan_of(&tree());
    let w = item(&p, "config/app.toml").warning.clone();
    assert_eq!(w.as_deref(), Some("secret_like_content:2"));
    assert_eq!(item(&p, "config/app.toml").decision, Decision::Include);
}

#[test]
fn totals_and_groups_add_up() {
    let p = plan_of(&tree());
    assert_eq!(p.totals.files.get(), p.items.len() as u64);
    let grouped: u64 = p.groups.iter().map(|g| g.file_count.get()).sum();
    assert_eq!(grouped, p.items.len() as u64);
    let inc = p
        .items
        .iter()
        .filter(|i| i.decision == Decision::Include)
        .count();
    assert_eq!(p.totals.included_files.get(), inc as u64);
    assert!(p.groups.iter().any(|g| g.name == "Needs decision"));
}

#[test]
fn digest_is_stable_and_ignores_plan_id() {
    let v = tree();
    let a = plan_of(&v);
    let ids = CountingIds::default();
    ids.new_uuid();
    let b = build_plan(&v, GitTreatment::Full, &CapsuleLimits::default(), &ids)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(a.plan_id, b.plan_id);
    assert_eq!(a.digest, b.digest);
    assert_eq!(a.digest.len(), 64);
    assert_eq!(a.digest, plan_digest(&a));
}

#[test]
fn every_edit_changes_the_digest() {
    let mut p = plan_of(&tree());
    let d0 = p.digest.clone();
    apply_edit(&mut p, "data/dev.sqlite", Decision::Exclude).unwrap_or_else(|e| panic!("{e}"));
    let d1 = p.digest.clone();
    assert_ne!(d0, d1);
    apply_edit(&mut p, "node_modules", Decision::Include).unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(d1, p.digest);
    assert_eq!(group_of(item(&p, "data/dev.sqlite")), Group::ExcludedByYou);
    assert_eq!(group_of(item(&p, "node_modules")), Group::Included);
}

#[test]
fn locked_and_unknown_items_refuse_edits() {
    let mut p = plan_of(&tree());
    let e = apply_edit(&mut p, ".env", Decision::Include)
        .err()
        .map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::PlanItemLocked));
    let e = apply_edit(&mut p, "nope.txt", Decision::Include)
        .err()
        .map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::PathUnsafe));
    let e = apply_edit(&mut p, "README.md", Decision::Undecided)
        .err()
        .map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::PlanIncomplete));
    assert_eq!(item(&p, ".env").decision, Decision::Exclude);
}

#[test]
fn approval_checks_digest_then_decisions_then_acks_then_limits() {
    let clock = FixedClock(1_800_000_000);
    let lim = CapsuleLimits::default();
    let mut p = plan_of(&tree());

    let e = approve(&p, "0".repeat(64).as_str(), &[], &clock, &lim).err();
    assert_eq!(e.map(|e| e.code), Some(ErrorCode::PlanDigestMismatch));

    let e = approve(&p, &p.digest, &[], &clock, &lim).err();
    assert_eq!(e.map(|e| e.code), Some(ErrorCode::PlanIncomplete));

    apply_edit(&mut p, "data/dev.sqlite", Decision::Exclude).unwrap_or_else(|e| panic!("{e}"));
    let e = approve(&p, &p.digest, &[], &clock, &lim).err();
    assert_eq!(
        e.map(|e| e.code),
        Some(ErrorCode::PlanIncomplete),
        "unacked warning"
    );

    let tight = CapsuleLimits {
        max_entries: 1,
        ..CapsuleLimits::default()
    };
    let acks = acks_for(&p);
    let e = approve(&p, &p.digest, &acks, &clock, &tight).err();
    assert_eq!(e.map(|e| e.code), Some(ErrorCode::CapsuleLimitExceeded));

    let ok = approve(&p, &p.digest, &acks, &clock, &lim).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ok.digest, p.digest);
    assert_eq!(ok.approved_at, "2027-01-15T08:00:00Z");
    assert_eq!(
        ok.acks,
        vec!["config/app.toml#secret_like_content".to_owned()]
    );
    assert_eq!(ok.overrides, vec!["data/dev.sqlite".to_owned()]);
}

#[test]
fn a_tampered_plan_is_refused_even_with_its_own_digest() {
    let mut p = plan_of(&tree());
    let shown = p.digest.clone();
    if let Some(it) = p.items.iter_mut().find(|i| i.path == ".env") {
        it.decision = Decision::Include;
    }
    let e = approve(&p, &shown, &[], &FixedClock(0), &CapsuleLimits::default()).err();
    assert_eq!(e.map(|e| e.code), Some(ErrorCode::PlanDigestMismatch));
}

/// Simulates a host-side plan altered after building, with its digest recomputed to match.
fn forged(mut p: CapturePlan, path: &str, f: impl Fn(&mut PlanItem)) -> CapturePlan {
    if let Some(it) = p.items.iter_mut().find(|i| i.path == path) {
        f(it);
    }
    refresh(&mut p);
    p
}

#[test]
fn edits_ignore_a_forged_editable_flag() {
    let mut p = forged(plan_of(&tree()), ".env", |it| it.editable = true);
    let e = apply_edit(&mut p, ".env", Decision::Include)
        .err()
        .map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::PlanItemLocked));
}

#[test]
fn approval_refuses_a_forged_plan_with_a_matching_digest() {
    let clock = FixedClock(0);
    let lim = CapsuleLimits::default();
    let base = {
        let mut p = plan_of(&tree());
        apply_edit(&mut p, "data/dev.sqlite", Decision::Exclude).unwrap_or_else(|e| panic!("{e}"));
        p
    };
    let acks = acks_for(&base);
    assert!(approve(&base, &base.digest, &acks, &clock, &lim).is_ok());

    let cases = [
        forged(base.clone(), ".env", |it| {
            it.decision = Decision::Include;
            it.editable = true;
        }),
        forged(base.clone(), ".env", |it| it.decision = Decision::Include),
        forged(base.clone(), ".env", |it| it.reason = "project_file".into()),
        forged(base.clone(), "README.md", |it| it.reason = "made_up".into()),
        forged(base.clone(), "README.md", |it| it.editable = false),
    ];
    for p in cases {
        let e = approve(&p, &p.digest, &acks, &clock, &lim)
            .err()
            .map(|e| e.code);
        assert_eq!(e, Some(ErrorCode::PlanItemLocked));
    }
}

#[test]
fn git_refs_and_logs_are_scanned_but_objects_and_index_are_not() {
    let mut v = MemVfs::new();
    v.add_file(".git/HEAD", "ref: refs/heads/main\n")
        .add_file(".git/logs/HEAD", format!("x {}\n", token()))
        .add_file(".git/index", "DIRC")
        .add_file(".git/objects/ab/cdef", "x");
    let p = plan_of(&v);
    assert_eq!(
        item(&p, ".git/logs/HEAD").warning.as_deref(),
        Some("secret_like_content:1")
    );
    assert_eq!(p.totals.secret_scan_skipped.get(), 2);
}

#[test]
fn working_tree_only_excludes_git() {
    let mut v = tree();
    v.add_file(".git/HEAD", "ref: refs/heads/main\n")
        .add_file(".git/config", "[core]\n");
    let p = build_plan(
        &v,
        GitTreatment::WorkingTreeOnly,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    for it in p.items.iter().filter(|i| i.path.starts_with(".git")) {
        assert_eq!(it.decision, Decision::Exclude, "{}", it.path);
    }
}
