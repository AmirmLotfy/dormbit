//! The Dockline capsule, pinned: SHA-256, length, and manifest JSON under the fixed test
//! clock and ID source (Property 4 golden; recorded for judge-demo Task 38).

#[path = "support/golden.rs"]
#[macro_use]
mod golden;
#[path = "support/dockline.rs"]
mod fixture;
#[path = "support/capture_kit.rs"]
mod kit;

use dormbit_core::model::GitTreatment;
use fixture::dockline;
use kit::*;
use serde::Serialize;

#[derive(Serialize)]
struct Pin {
    file_sha256: String,
    size: u64,
    manifest_sha256: String,
    entry_names: Vec<String>,
}

#[test]
fn dockline_capsule_bytes_are_pinned() {
    let v = dockline();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (out, bytes) = capture(&v, plan, ok);
    let names = read_tar(&bytes).into_iter().map(|e| e.name).collect();
    golden!(
        "capsule",
        "dockline_pin",
        &Pin {
            file_sha256: out.record.file_sha256.clone(),
            size: out.record.size,
            manifest_sha256: out.record.manifest_sha256.clone(),
            entry_names: names,
        }
    );
    golden!("capsule", "dockline_manifest", &out.manifest);
}

#[test]
fn dockline_capsule_leaves_out_env_and_records_the_git_head() {
    let v = dockline();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (out, _) = capture(&v, plan, ok);
    let m = out.manifest;
    assert!(m.entries.iter().all(|e| e.path() != ".env"));
    assert!(m.entries.iter().any(|e| e.path() == ".env.example"));
    assert!(
        m.excluded
            .iter()
            .any(|e| e.path == ".env" && e.group == "excluded_sensitive")
    );
    assert!(m.excluded.iter().any(|e| e.path == "data/dev.sqlite"));
    let head = m
        .git
        .and_then(|g| g.head)
        .unwrap_or_else(|| panic!("no head"));
    assert!(head.commit.is_some());
}
