//! Capture writer behavior (capsule-and-verification Task 2).

#[path = "support/capture_kit.rs"]
mod kit;

use std::cell::Cell;

use dormbit_core::capsule::manifest::{Entry, MARKER};
use dormbit_core::capsule::{Budget, CapsuleLimits, Job, MemRegistry, MemWorkspace, Phase, Step};
use dormbit_core::model::{Decision, ErrorCode, GitTreatment};
use dormbit_core::util::sha256_hex;
use dormbit_core::vfs::{DirEntry, Limited, MemVfs, Metadata, ReadVfs, RelPath, VfsError};
use kit::*;

fn tree() -> MemVfs {
    let mut v = MemVfs::new();
    v.add_file("README.md", "# demo\n")
        .add_file_with("bin/run.sh", "#!/bin/sh\necho hi\n", 0o4755, 1_700_000_000)
        .add_file("src/a.rs", "same\n")
        .add_file("src/b.rs", "same\n")
        .add_file("empty.txt", "")
        .add_file(".env", "X=1\n")
        .add_symlink("link", "README.md")
        .add_file("node_modules/pkg/index.js", "module.exports = 1;\n")
        .add_file("node_modules/pkg/.env", "Y=2\n");
    v
}

#[test]
fn writes_marker_manifest_sorted_objects_trailer_and_two_zero_blocks() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (out, bytes) = capture(&v, plan, ok);
    let entries = read_tar(&bytes);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names[0], "DORMBIT");
    assert_eq!(names[1], "manifest.json");
    assert_eq!(*names.last().unwrap_or(&""), "END");
    let objs: Vec<&str> = names[2..names.len() - 1].to_vec();
    let mut sorted = objs.clone();
    sorted.sort_unstable();
    assert_eq!(objs, sorted);
    assert_eq!(entries[0].data, MARKER);
    assert!(entries.iter().all(|e| e.mtime == NOW as u64));
    assert!(bytes.len() % 512 == 0);
    assert!(bytes[bytes.len() - 1024..].iter().all(|&b| b == 0));
    assert_eq!(out.record.size, bytes.len() as u64);
    assert_eq!(out.record.file_sha256, sha256_hex(&bytes));
    assert_eq!(out.record.manifest_sha256, sha256_hex(&entries[1].data));
}

#[test]
fn every_object_decompresses_to_its_source_file() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (_, bytes) = capture(&v, plan, ok);
    let entries = read_tar(&bytes);
    let m = manifest_of(&entries);
    for e in &m.entries {
        if let Entry::File {
            path, sha256, mode, ..
        } = e
        {
            let obj = entries
                .iter()
                .find(|t| t.name == format!("objects/{sha256}"))
                .unwrap_or_else(|| panic!("no object for {path}"));
            let src = match v
                .read_limited(&RelPath::parse(path).unwrap(), 1 << 20)
                .unwrap()
            {
                Limited::Complete(b) => b,
                Limited::TooLarge { .. } => panic!(),
            };
            assert_eq!(gunzip(&obj.data), src, "{path}");
            assert_eq!(sha256_hex(&src), *sha256);
            if path == "bin/run.sh" {
                assert_eq!(*mode, 0o755, "setuid is never recorded");
                assert_eq!(obj.mode, 0o755);
            }
        }
    }
}

#[test]
fn duplicate_contents_share_one_object() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (out, _) = capture(&v, plan, ok);
    let shas: Vec<&str> = out
        .manifest
        .entries
        .iter()
        .filter_map(|e| match e {
            Entry::File { path, sha256, .. } if path.starts_with("src/") => Some(sha256.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(shas.len(), 2);
    assert_eq!(shas[0], shas[1]);
    assert_eq!(out.manifest.totals.objects as usize, {
        let mut s: Vec<_> = out
            .manifest
            .entries
            .iter()
            .filter_map(|e| match e {
                Entry::File { sha256, .. } => Some(sha256.clone()),
                Entry::Symlink { .. } => None,
            })
            .collect();
        s.sort();
        s.dedup();
        s.len()
    });
}

#[test]
fn sensitive_files_and_symlinks_follow_the_rules() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &["node_modules"]);
    let (out, _) = capture(&v, plan, ok);
    let paths: Vec<&str> = out.manifest.entries.iter().map(|e| e.path()).collect();
    assert!(paths.contains(&"node_modules/pkg/index.js"));
    assert!(!paths.contains(&"node_modules/pkg/.env"));
    assert!(!paths.contains(&".env"));
    let ex: Vec<(&str, &str)> = out
        .manifest
        .excluded
        .iter()
        .map(|e| (e.path.as_str(), e.reason.as_str()))
        .collect();
    assert!(ex.contains(&("node_modules/pkg/.env", "sensitive_env")));
    assert!(ex.contains(&(".env", "sensitive_env")));
    let link = out.manifest.entries.iter().find(|e| e.path() == "link");
    assert!(matches!(
        link,
        Some(Entry::Symlink { target, restored: false, .. }) if target == "README.md"
    ));
    assert_eq!(out.manifest.totals.symlinks, 1);
}

#[test]
fn bytes_do_not_depend_on_the_budget() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &["node_modules"]);
    let (_, a) = capture(&v, plan.clone(), ok.clone());
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let out = run_with(
        &mut j,
        Budget {
            max_bytes: 1,
            max_ops: 1,
        },
    )
    .unwrap();
    let (ws, _) = j.into_parts();
    assert_eq!(ws.library.get(&out.record.file_name), Some(&a));
}

#[test]
fn a_plan_changed_after_approval_is_refused() {
    let v = tree();
    let (mut plan, ok) = approved(&v, GitTreatment::Full, &[]);
    dormbit_core::plan::apply_edit(&mut plan, "README.md", Decision::Exclude).unwrap();
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let e = run_with(&mut j, Budget::UNBOUNDED).err().map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::PlanDigestMismatch));
    assert!(j.workspace().aborted);
}

#[test]
fn files_that_appear_after_approval_are_left_out_and_reported() {
    let mut v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    v.add_file("later.txt", "new\n");
    let (out, _) = capture(&v, plan, ok);
    assert_eq!(out.not_captured, vec!["later.txt".to_owned()]);
    assert!(out.manifest.entries.iter().all(|e| e.path() != "later.txt"));
    assert!(
        out.manifest
            .excluded
            .iter()
            .any(|e| e.path == "later.txt" && e.reason == "appeared_after_approval")
    );
}

/// Wraps a `MemVfs` and returns different bytes for one file from its second read on.
struct ChangesOnReread<'a> {
    inner: &'a MemVfs,
    path: &'static str,
    reads: Cell<u32>,
}

impl ReadVfs for ChangesOnReread<'_> {
    fn read_dir(&self, d: &RelPath) -> Result<Vec<DirEntry>, VfsError> {
        self.inner.read_dir(d)
    }
    fn symlink_metadata(&self, p: &RelPath) -> Result<Metadata, VfsError> {
        self.inner.symlink_metadata(p)
    }
    fn read_limited(&self, p: &RelPath, max: u64) -> Result<Limited, VfsError> {
        let r = self.inner.read_limited(p, max)?;
        if p.as_str() == self.path {
            self.reads.set(self.reads.get() + 1);
            if self.reads.get() >= 3
                && let Limited::Complete(mut b) = r
            {
                b[0] ^= 1;
                return Ok(Limited::Complete(b));
            }
        }
        Ok(r)
    }
    fn read_prefix(&self, p: &RelPath, n: u64) -> Result<Vec<u8>, VfsError> {
        self.inner.read_prefix(p, n)
    }
    fn read_link(&self, p: &RelPath) -> Result<String, VfsError> {
        self.inner.read_link(p)
    }
}

#[test]
fn a_file_that_changes_between_hashing_and_rechecking_fails_the_capture() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    // Read 1 is the plan's secret scan (not this wrapper), 2 is hashing, 3 is rechecking.
    let w = ChangesOnReread {
        inner: &v,
        path: "README.md",
        reads: Cell::new(1),
    };
    let mut j = job(&w, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let e = run_with(&mut j, Budget::UNBOUNDED).err();
    assert_eq!(
        e.as_ref().map(|e| e.code),
        Some(ErrorCode::FileChangedDuringCapture)
    );
    assert!(e.map(|e| e.message.contains("README.md")).unwrap_or(false));
    assert!(j.workspace().aborted);
    assert!(j.workspace().library.is_empty());
}

#[test]
fn cancel_before_commit_saves_nothing() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let one = Budget {
        max_bytes: 1,
        max_ops: 1,
    };
    assert!(matches!(j.step(one), Ok(Step::Progress(_))));
    j.cancel();
    assert_eq!(
        j.step(one).err().map(|e| e.code),
        Some(ErrorCode::Cancelled)
    );
    assert!(j.workspace().aborted);
    assert!(j.workspace().library.is_empty());
    assert!(j.registry().records.is_empty());
}

#[test]
fn cancel_from_committing_on_still_finishes_the_capsule() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    loop {
        match j.step(Budget::UNBOUNDED).unwrap() {
            Step::Progress(p) if p.phase == Phase::Committing => break,
            Step::Progress(_) => {}
            Step::Done(_) => panic!("finished early"),
        }
    }
    j.cancel();
    let out = run_with(&mut j, Budget::UNBOUNDED).unwrap();
    assert!(j.workspace().library.contains_key(&out.record.file_name));
    assert_eq!(j.registry().records.len(), 1);
}

#[test]
fn an_existing_capsule_name_is_refused_and_left_unchanged() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let name = "00000000-0000-4000-8000-000000000001.dormbit";
    let ws = MemWorkspace::with_existing(name, b"keep me".to_vec());
    let mut j = job(&v, plan, ok, ws, MemRegistry::default());
    let e = run_with(&mut j, Budget::UNBOUNDED).err().map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::CapsuleExists));
    assert_eq!(j.workspace().library.get(name), Some(&b"keep me".to_vec()));
    assert!(j.workspace().aborted);
}

#[test]
fn a_registry_failure_keeps_the_capsule_in_place() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let reg = MemRegistry {
        fail_put: true,
        ..MemRegistry::default()
    };
    let mut j = job(&v, plan, ok, MemWorkspace::new(), reg);
    let e = run_with(&mut j, Budget::UNBOUNDED).err().map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::IoFailed));
    assert_eq!(j.workspace().library.len(), 1);
    assert!(!j.workspace().aborted);
}

#[test]
fn a_staged_object_changed_before_assembly_fails_with_hash_mismatch() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    loop {
        match j.step(Budget::UNBOUNDED).unwrap() {
            Step::Progress(p) if p.phase == Phase::Assembling => break,
            Step::Progress(_) => {}
            Step::Done(_) => panic!("finished early"),
        }
    }
    j.workspace_mut()
        .tamper_object(&sha256_hex(b"# demo\n"), b"not gzip".to_vec());
    let e = run_with(&mut j, Budget::UNBOUNDED).err().map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::CapsuleHashMismatch));
    assert!(j.workspace().library.is_empty());
}

#[test]
fn a_tiny_capsule_size_limit_is_enforced() {
    let v = tree();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut c = ctx();
    c.limits = CapsuleLimits {
        max_capsule_size: 2048,
        ..CapsuleLimits::default()
    };
    let mut j = dormbit_core::capsule::CaptureJob::new(
        &v,
        plan,
        ok,
        MemWorkspace::new(),
        MemRegistry::default(),
        &dormbit_core::vfs::FixedClock(NOW),
        &dormbit_core::vfs::CountingIds::default(),
        c,
    )
    .unwrap();
    let e = run_with(&mut j, Budget::UNBOUNDED).err().map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::CapsuleLimitExceeded));
}

#[test]
fn git_head_and_refs_are_recorded_under_full_treatment_only() {
    let mut v = tree();
    let oid = "a".repeat(40);
    v.add_file(".git/HEAD", "ref: refs/heads/main\n")
        .add_file(".git/refs/heads/main", format!("{oid}\n"));
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let (out, _) = capture(&v, plan, ok);
    let g = out.manifest.git.unwrap();
    assert_eq!(g.head.unwrap().commit.as_deref(), Some(oid.as_str()));
    assert_eq!(g.refs.len(), 1);

    let (plan, ok) = approved(&v, GitTreatment::WorkingTreeOnly, &[]);
    let (out, _) = capture(&v, plan, ok);
    let g = out.manifest.git.unwrap();
    assert!(g.head.is_none() && g.refs.is_empty());
    assert!(
        out.manifest
            .entries
            .iter()
            .all(|e| !e.path().starts_with(".git"))
    );
}

/// Serves `swap` (same length) for full reads of `path`, while prefix probes still see the
/// original: a file that becomes a private key after `preparing`.
struct BecomesKey<'a> {
    inner: &'a MemVfs,
    path: &'static str,
    swap: Vec<u8>,
}

impl ReadVfs for BecomesKey<'_> {
    fn read_dir(&self, d: &RelPath) -> Result<Vec<DirEntry>, VfsError> {
        self.inner.read_dir(d)
    }
    fn symlink_metadata(&self, p: &RelPath) -> Result<Metadata, VfsError> {
        self.inner.symlink_metadata(p)
    }
    fn read_limited(&self, p: &RelPath, max: u64) -> Result<Limited, VfsError> {
        if p.as_str() == self.path {
            return Ok(Limited::Complete(self.swap.clone()));
        }
        self.inner.read_limited(p, max)
    }
    fn read_prefix(&self, p: &RelPath, n: u64) -> Result<Vec<u8>, VfsError> {
        self.inner.read_prefix(p, n)
    }
    fn read_link(&self, p: &RelPath) -> Result<String, VfsError> {
        self.inner.read_link(p)
    }
}

#[test]
fn a_file_that_becomes_a_private_key_after_preparing_is_never_captured() {
    let mut v = tree();
    v.add_file("notes.txt", "x".repeat(80));
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut key = format!("-----BEGIN OPENSSH {}-----\n", "PRIVATE KEY").into_bytes();
    key.resize(80, b'A');
    let w = BecomesKey {
        inner: &v,
        path: "notes.txt",
        swap: key,
    };
    let mut j = job(&w, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let e = run_with(&mut j, Budget::UNBOUNDED).err().map(|e| e.code);
    assert_eq!(e, Some(ErrorCode::PathUnsafe));
    assert!(j.workspace().library.is_empty());
    assert!(j.workspace().aborted);
}
