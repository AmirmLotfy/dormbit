//! Native restore end to end on real temp folders: capture, verify, pre-check with the real
//! volume probe, restore through `CapStdRestoreFs` and the file journal, then post-verify.

use std::collections::BTreeMap;
use std::path::Path;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use dormbit_core::capsule::manifest::ManifestProject;
use dormbit_core::capsule::{
    Budget, CapsuleLimits, CaptureContext, CaptureJob, Job, Step, run_to_end, verify_all,
};
use dormbit_core::model::{Decision, GitTreatment, ProjectId, RestoreState, ScopeId};
use dormbit_core::plan::{ack_id, apply_edit, approve, build_plan};
use dormbit_core::restore::report::RefCheck;
use dormbit_core::restore::{
    JournalStore, OpenedCapsule, PlanInput, RecoveryAction, RestoreContext, RestoreJob,
    RestoreStatus, plan_restore, recover,
};
use dormbit_core::util::sha256_hex;
use dormbit_core::vfs::{CountingIds, FixedClock};
use dormbit_fs::restore::{CapStdRestoreFs, FileJournalStore, list_names, probe_volume};
use dormbit_fs::{FsCapsuleRegistry, FsCapsuleSink, FsCapsuleSource, FsVfs};

fn open(p: &Path) -> Dir {
    Dir::open_ambient_dir(p, ambient_authority()).unwrap()
}

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let m = std::fs::symlink_metadata(&p).unwrap();
            if m.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("link:{}", std::fs::read_link(&p).unwrap().display()),
                );
            } else if m.is_dir() {
                out.insert(rel.clone(), "dir".into());
                walk(base, &p, out);
            } else {
                use std::os::unix::fs::MetadataExt;
                out.insert(
                    rel,
                    format!(
                        "{}:{:o}",
                        sha256_hex(&std::fs::read(&p).unwrap()),
                        m.mode() & 0o777
                    ),
                );
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(root, root, &mut m);
    m
}

struct World {
    src: tempfile::TempDir,
    lib: tempfile::TempDir,
    state: tempfile::TempDir,
    dest: tempfile::TempDir,
    journals: tempfile::TempDir,
    capsule_name: String,
}

fn world() -> World {
    let w = World {
        src: tempfile::tempdir().unwrap(),
        lib: tempfile::tempdir().unwrap(),
        state: tempfile::tempdir().unwrap(),
        dest: tempfile::tempdir().unwrap(),
        journals: tempfile::tempdir().unwrap(),
        capsule_name: String::new(),
    };
    let p = w.src.path();
    std::fs::create_dir_all(p.join("src/deep")).unwrap();
    std::fs::create_dir_all(p.join(".git/refs/heads")).unwrap();
    std::fs::write(p.join("README.md"), b"# demo\n").unwrap();
    std::fs::write(p.join("src/a.rs"), b"fn a() {}\n").unwrap();
    std::fs::write(p.join("src/deep/b.rs"), b"fn b() {}\n").unwrap();
    std::fs::write(p.join("run.sh"), b"#!/bin/sh\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(p.join(".env"), b"X=1\n").unwrap();
    std::fs::write(p.join(".git/HEAD"), b"ref: refs/heads/main\n").unwrap();
    std::fs::write(
        p.join(".git/refs/heads/main"),
        format!("{}\n", "cd".repeat(20)),
    )
    .unwrap();
    std::fs::write(w.dest.path().join("keep.txt"), b"mine").unwrap();

    let vfs = FsVfs::new(open(p));
    let mut plan = build_plan(
        &vfs,
        GitTreatment::Full,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap();
    let undecided: Vec<String> = plan
        .items
        .iter()
        .filter(|i| i.decision == Decision::Undecided)
        .map(|i| i.path.clone())
        .collect();
    for u in undecided {
        apply_edit(&mut plan, &u, Decision::Exclude).unwrap();
    }
    let acks: Vec<String> = plan
        .items
        .iter()
        .filter_map(|i| i.warning.as_deref().map(|x| ack_id(&i.path, x)))
        .collect();
    let ok = approve(
        &plan,
        &plan.digest,
        &acks,
        &FixedClock(1),
        &CapsuleLimits::default(),
    )
    .unwrap();
    let ws = FsCapsuleSink::new(open(w.lib.path()))
        .begin("01", "02")
        .unwrap();
    let reg = FsCapsuleRegistry::open(open(w.state.path())).unwrap();
    let ctx = CaptureContext {
        app_version: "0.1.0".into(),
        project_id: ProjectId::from("p"),
        project: ManifestProject {
            name: "demo".into(),
            name_source: "folder".into(),
        },
        library_grant: ScopeId::from("lib"),
        limits: CapsuleLimits::default(),
    };
    let mut job = CaptureJob::new(
        &vfs,
        plan,
        ok,
        ws,
        reg,
        &FixedClock(1_800_000_000),
        &CountingIds::default(),
        ctx,
    )
    .unwrap();
    let out = run_to_end(&mut job).unwrap();
    World {
        capsule_name: out.record.file_name,
        ..w
    }
}

type NativeJob =
    RestoreJob<CapStdRestoreFs, FileJournalStore, OpenedCapsule<std::fs::File>, FixedClock>;

fn prepare(
    w: &World,
    target: &str,
    op: &str,
) -> Result<NativeJob, Box<dormbit_core::restore::PrecheckReport>> {
    let reg = FsCapsuleRegistry::open(open(w.state.path())).unwrap();
    let src = FsCapsuleSource::new(open(w.lib.path()), &w.capsule_name).unwrap();
    let report = verify_all(src, reg, CapsuleLimits::default()).unwrap();
    let file = std::fs::File::open(w.lib.path().join(&w.capsule_name)).unwrap();
    let opened = OpenedCapsule::open(file, 1 << 26).unwrap();
    let parent = open(w.dest.path());
    let volume = probe_volume(&parent).unwrap();
    let names = list_names(&parent).unwrap();
    let manifest = opened.manifest.clone();
    let ms = opened.manifest_sha256.clone();
    let pre = plan_restore(&PlanInput {
        manifest: &manifest,
        manifest_sha256_now: &ms,
        verification: Some(&report),
        target_name: target,
        target_display: format!("~/{target}"),
        parent_entries: &names,
        volume,
        inside_source: Some(false),
        inside_library: false,
    });
    let Some(plan) = pre.plan else {
        return Err(Box::new(pre.report));
    };
    Ok(RestoreJob::new(
        plan,
        RestoreContext {
            op_id: op.into(),
            owner_token: "9e".repeat(16),
            target_grant_id: "g".into(),
            target_display: format!("~/{target}"),
            manifest_git: manifest.git.clone(),
        },
        CapStdRestoreFs,
        FileJournalStore::new(open(w.journals.path())),
        opened,
        FixedClock(1_800_000_500),
        parent,
    )
    .unwrap())
}

#[test]
fn restores_a_real_capsule_into_a_new_folder_and_changes_nothing_else() {
    let w = world();
    let src_before = snapshot(w.src.path());
    let capsule_before = std::fs::read(w.lib.path().join(&w.capsule_name)).unwrap();
    let mut job = prepare(&w, "demo-restored", "0000000a").unwrap_or_else(|r| panic!("{r:#?}"));
    let r = run_to_end(&mut job).unwrap();
    assert_eq!(r.status, RestoreStatus::Verified, "{r:#?}");
    assert_eq!(r.git_check.refs, RefCheck::Match);
    assert_eq!(r.writes_outside_destination, 0);

    let mut names: Vec<String> = std::fs::read_dir(w.dest.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["demo-restored".to_owned(), "keep.txt".to_owned()],
        "staging removed"
    );
    assert_eq!(
        std::fs::read(w.dest.path().join("keep.txt")).unwrap(),
        b"mine"
    );

    let restored = snapshot(&w.dest.path().join("demo-restored"));
    let mut expected = snapshot(w.src.path());
    expected.remove(".env");
    assert_eq!(
        restored, expected,
        "restored tree differs from the approved source"
    );
    assert_eq!(snapshot(w.src.path()), src_before, "source changed");
    assert_eq!(
        std::fs::read(w.lib.path().join(&w.capsule_name)).unwrap(),
        capsule_before
    );

    let j = FileJournalStore::new(open(w.journals.path()))
        .load("0000000a")
        .unwrap()
        .unwrap();
    assert_eq!(j.state, RestoreState::Cleaned);
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(w.journals.path().join("0000000a/journal.json"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn an_existing_folder_is_refused_and_left_untouched() {
    let w = world();
    std::fs::create_dir(w.dest.path().join("Demo")).unwrap();
    std::fs::write(w.dest.path().join("Demo/theirs.txt"), b"theirs").unwrap();
    let pre = prepare(&w, "demo", "0000000b")
        .err()
        .expect("must be refused");
    assert_eq!(
        pre.first_error(),
        Some(dormbit_core::model::ErrorCode::RestoreDestExists)
    );
    assert_eq!(
        std::fs::read(w.dest.path().join("Demo/theirs.txt")).unwrap(),
        b"theirs"
    );
    assert!(
        std::fs::read_dir(w.journals.path())
            .unwrap()
            .next()
            .is_none(),
        "no journal created"
    );
}

#[test]
fn an_interrupted_restore_is_rolled_back_by_recovery() {
    let w = world();
    let mut job = prepare(&w, "demo-restored", "0000000c").unwrap_or_else(|r| panic!("{r:#?}"));
    for _ in 0..3 {
        assert!(matches!(
            job.step(Budget {
                max_bytes: 1,
                max_ops: 1
            }),
            Ok(Step::Progress(_))
        ));
    }
    drop(job);
    let staging = std::fs::read_dir(w.dest.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".dormbit-staging-")
        });
    assert!(staging, "a staging folder exists mid-restore");
    let mut fs = CapStdRestoreFs;
    let mut journals = FileJournalStore::new(open(w.journals.path()));
    let parent = open(w.dest.path());
    let out = recover(&mut fs, &parent, &mut journals, "0000000c", &FixedClock(9)).unwrap();
    assert_eq!(out.action, RecoveryAction::RolledBack);
    let mut names: Vec<String> = std::fs::read_dir(w.dest.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, vec!["keep.txt".to_owned()]);
    assert!(journals.pending().unwrap().is_empty());
}
