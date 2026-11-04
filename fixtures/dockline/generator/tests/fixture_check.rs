//! Fixture_Check (foundation Requirements 19.5 and 19.6): the committed fixture is exactly what
//! the generator produces today. Regenerate with `cargo run -p dockline-gen --locked -- --out
//! fixtures/dockline` when the generator changes on purpose.

mod support;

use std::path::Path;

#[test]
fn committed_fixture_matches_a_fresh_run() {
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let fresh = support::temp_dir("check");
    support::run(&fresh, "UTC", "022");
    let diffs = support::differences(&committed, &fresh);
    std::fs::remove_dir_all(&fresh).ok();
    assert!(
        diffs.is_empty(),
        "the committed fixture differs from a fresh run:\n{}",
        diffs.join("\n")
    );
}
