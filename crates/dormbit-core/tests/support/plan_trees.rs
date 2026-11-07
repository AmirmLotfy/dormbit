//! Random project trees for the plan property tests. Every file holds plain text, so the
//! secret scanner never adds warnings; sensitive names come from a fixed list.

use dormbit_core::capsule::CapsuleLimits;
use dormbit_core::model::{CapturePlan, GitTreatment};
use dormbit_core::plan::build_plan;
use dormbit_core::vfs::{CountingIds, MemVfs};
use proptest::prelude::*;

pub const SENSITIVE: [&str; 6] = [
    ".env",
    ".env.local",
    "id_rsa",
    "server.pem",
    "client.p12",
    ".netrc",
];
pub const ORDINARY: [&str; 6] = [
    "main.rs",
    "README.md",
    "app.ts",
    "notes.txt",
    "dev.sqlite",
    "a.json",
];
pub const DIRS: [&str; 5] = ["", "src/", "config/", "deep/er/", "node_modules/pkg/"];

/// `(dir index, name index, sensitive?)` per file.
pub fn tree() -> impl Strategy<Value = Vec<(usize, usize, bool)>> {
    prop::collection::vec((0..DIRS.len(), 0..ORDINARY.len(), any::<bool>()), 1..24)
}

pub fn plan_for(entries: &[(usize, usize, bool)], treatment: GitTreatment) -> CapturePlan {
    let mut v = MemVfs::new();
    for &(d, n, sensitive) in entries {
        let name = if sensitive { SENSITIVE[n] } else { ORDINARY[n] };
        v.add_file(&format!("{}{}", DIRS[d], name), "plain text\n");
    }
    v.add_file(".git/HEAD", "ref: refs/heads/main\n");
    build_plan(
        &v,
        treatment,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

pub fn treatment() -> impl Strategy<Value = GitTreatment> {
    prop_oneof![
        Just(GitTreatment::Full),
        Just(GitTreatment::WorkingTreeOnly)
    ]
}
