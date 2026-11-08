//! The native Git snapshot of the extracted Dockline fixture matches the snapshot its
//! generator recorded in the Sidecar (the one the web demo uses), and reading it changes
//! nothing in the repository.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/dockline")
}

fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let m = std::fs::symlink_metadata(&p).unwrap();
            if m.is_dir() {
                stack.push(p);
            } else if m.is_file() {
                out.insert(
                    p.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

#[test]
fn the_native_snapshot_of_dockline_matches_the_sidecar_and_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let tar_bytes = std::fs::read(fixture().join("dockline-invoices.tar")).unwrap();
    tar::Archive::new(tar_bytes.as_slice())
        .unpack(tmp.path())
        .unwrap();
    let repo = tmp.path().join("dockline-invoices");
    let before = tree(&repo);

    let snap = dormbit_git::inspect(&repo).unwrap();

    let side: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture().join("dockline-invoices.sidecar.json")).unwrap(),
    )
    .unwrap();
    let want = &side["git_snapshot"];
    let got = serde_json::to_value(&snap).unwrap();
    for key in [
        "head",
        "status",
        "stash_count",
        "upstream",
        "submodules",
        "lfs_detected",
        "risky_config",
        "remotes_redacted",
        "worktree_external",
    ] {
        assert_eq!(got[key], want[key], "{key}");
    }
    assert_eq!(tree(&repo), before, "inspect changed the repository");
}

#[test]
fn a_folder_without_git_is_not_a_repo() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        dormbit_git::inspect(tmp.path()),
        Err(dormbit_git::GitIssue::NotARepo)
    );
}
