//! Property 2: the Dockline generator is deterministic. For any output folder name, start
//! time, `TZ`, and umask, a run matches a reference run in every tar entry except
//! `.git/index`, and in the Sidecar except `fixture_sha256` and the `.git/index` hash.
//! Validates foundation Requirements 19.1, 19.2, 19.3, and 19.5.

mod support;

use std::path::PathBuf;
use std::sync::OnceLock;

use proptest::prelude::*;

fn reference() -> &'static PathBuf {
    static REF: OnceLock<PathBuf> = OnceLock::new();
    REF.get_or_init(|| {
        let dir = support::temp_dir("ref");
        support::run(&dir, "UTC", "022");
        dir
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn generator_is_deterministic(
        name in "[a-z]{1,8}( [a-z0-9]{1,6})?",
        tz in prop::sample::select(vec!["UTC", "Asia/Tokyo", "America/Los_Angeles", "Australia/Lord_Howe", "Pacific/Kiritimati"]),
        umask in prop::sample::select(vec!["022", "077", "002", "027"]),
    ) {
        let out = support::temp_dir("prop").join(name);
        support::run(&out, tz, umask);
        let diffs = support::differences(reference(), &out);
        if let Some(parent) = out.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
        prop_assert!(diffs.is_empty(), "TZ={} umask={}:\n{}", tz, umask, diffs.join("\n"));
    }
}
