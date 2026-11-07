//! The capture plan for the fictional Dockline project, pinned as a golden snapshot.
//! The fixture tar is loaded into a `MemVfs`; core itself never reads the disk.

#[path = "support/golden.rs"]
#[macro_use]
mod golden;

use std::io::Read as _;

use dormbit_core::capsule::CapsuleLimits;
use dormbit_core::model::{Decision, GitTreatment};
use dormbit_core::plan::build_plan;
use dormbit_core::vfs::{CountingIds, MemVfs};

const PREFIX: &str = "dockline-invoices/";

/// Embedded at compile time, so the test itself does no I/O (core's clippy denies `std::fs`).
const FIXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/dockline/dockline-invoices.tar"
));

fn dockline() -> MemVfs {
    let mut archive = tar::Archive::new(FIXTURE);
    let mut vfs = MemVfs::new();
    for entry in archive.entries().unwrap_or_else(|e| panic!("{e}")) {
        let mut entry = entry.unwrap_or_else(|e| panic!("{e}"));
        let name = entry
            .path()
            .unwrap_or_else(|e| panic!("{e}"))
            .to_string_lossy()
            .into_owned();
        let Some(rel) = name.strip_prefix(PREFIX) else {
            continue;
        };
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() {
            continue;
        }
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            vfs.add_dir(rel);
        } else if kind.is_symlink() {
            let target = entry
                .link_name()
                .ok()
                .flatten()
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            vfs.add_symlink(rel, &target);
        } else if kind.is_file() {
            let mode = entry.header().mode().unwrap_or(0o644);
            let mtime = i64::try_from(entry.header().mtime().unwrap_or(0)).unwrap_or(0);
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .unwrap_or_else(|e| panic!("{e}"));
            vfs.add_file_with(rel, data, mode, mtime);
        }
    }
    vfs
}

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
