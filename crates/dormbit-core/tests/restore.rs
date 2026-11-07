//! Transactional restore end to end over `MemRestoreFs` (transactional-restore Tasks 23-25).
//! Token-like values are built at run time, so no secret-looking literal is committed.

#[path = "support/capture_kit.rs"]
mod kit;

use std::io::Cursor;

use dormbit_core::capsule::Job;
use dormbit_core::capsule::manifest::Manifest;
use dormbit_core::capsule::{
    Budget, BytesSource, CapsuleLimits, MemRegistry, MemWorkspace, Report, Step, verify_all,
};
use dormbit_core::model::{ErrorCode, GitTreatment, RestoreState};
use dormbit_core::restore::fs::{FsFault, MemNode, MemRestoreFs, RestoreFs};
use dormbit_core::restore::report::RefCheck;
use dormbit_core::restore::{
    CancelResult, CaseSensitivity, JournalStore, MemJournal, OpenedCapsule, PlanInput,
    PrecheckReport, PrecheckState, RecoveryAction, RestoreContext, RestoreJob, RestorePlan,
    RestoreReport, RestoreStatus, VolumeFacts, plan_restore, recover,
};
use dormbit_core::util::sha256_hex;
use dormbit_core::vfs::{FixedClock, MemVfs};
use kit::*;

const OP: &str = "0000aaaa-0000-4000-8000-000000000001";

fn oid() -> String {
    "ab".repeat(20)
}

fn token() -> String {
    "0f".repeat(16)
}

fn source() -> MemVfs {
    let mut v = MemVfs::new();
    v.add_file("README.md", "# demo\n")
        .add_file_with("bin/run.sh", "#!/bin/sh\necho hi\n", 0o4755, 1_700_000_000)
        .add_file("src/a.rs", "same\n")
        .add_file("src/deep/b.rs", "same\n")
        .add_file("empty.txt", "")
        .add_file(".env", "X=1\n")
        .add_symlink("ln", "README.md")
        .add_file(".git/HEAD", "ref: refs/heads/main\n")
        .add_file(".git/refs/heads/main", format!("{}\n", oid()))
        .add_file(".git/config", "[core]\n\tbare = false\n");
    v
}

struct Fixture {
    bytes: Vec<u8>,
    manifest: Manifest,
    verification: Report,
}

fn fixture() -> Fixture {
    let v = source();
    let (plan, ok) = approved(&v, GitTreatment::Full, &[]);
    let mut j = job(&v, plan, ok, MemWorkspace::new(), MemRegistry::default());
    let out = run_with(&mut j, Budget::UNBOUNDED).unwrap();
    let (ws, reg) = j.into_parts();
    let bytes = ws.library[&out.record.file_name].clone();
    let verification = verify_all(
        BytesSource::new(bytes.clone()),
        reg,
        CapsuleLimits::default(),
    )
    .unwrap();
    Fixture {
        bytes,
        manifest: out.manifest,
        verification,
    }
}

fn volume(case: CaseSensitivity) -> VolumeFacts {
    VolumeFacts {
        fs_type: "mem".into(),
        block_size: 4096,
        free_bytes: 1 << 30,
        case,
    }
}

fn names(fs: &MemRestoreFs) -> Vec<String> {
    fs.list(&fs.root())
        .unwrap()
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

fn input<'a>(
    fx: &'a Fixture,
    m: &'a Manifest,
    ms: &'a str,
    name: &'a str,
    entries: &'a [String],
    v: VolumeFacts,
) -> PlanInput<'a> {
    PlanInput {
        manifest: m,
        manifest_sha256_now: ms,
        verification: Some(&fx.verification),
        target_name: name,
        target_display: format!("~/Projects/{name}"),
        parent_entries: entries,
        volume: v,
        inside_source: Some(false),
        inside_library: false,
    }
}

fn precheck(fx: &Fixture, fs: &MemRestoreFs, name: &str) -> (PrecheckReport, Option<RestorePlan>) {
    let entries = names(fs);
    let ms = fx.verification.manifest_sha256.clone().unwrap();
    let case = if fs.case_insensitive {
        CaseSensitivity::Insensitive
    } else {
        CaseSensitivity::Sensitive
    };
    let out = plan_restore(&input(fx, &fx.manifest, &ms, name, &entries, volume(case)));
    (out.report, out.plan)
}

type TestJob = RestoreJob<MemRestoreFs, MemJournal, OpenedCapsule<Cursor<Vec<u8>>>, FixedClock>;

fn start(fx: &Fixture, fs: MemRestoreFs, plan: RestorePlan) -> TestJob {
    let objects = OpenedCapsule::open(Cursor::new(fx.bytes.clone()), 1 << 26).unwrap();
    let root = fs.root();
    RestoreJob::new(
        plan,
        RestoreContext {
            op_id: OP.into(),
            owner_token: token(),
            target_grant_id: "g-1".into(),
            target_display: "~/Projects/demo-restored".into(),
            manifest_git: fx.manifest.git.clone(),
        },
        fs,
        MemJournal::default(),
        objects,
        FixedClock(1_800_000_100),
        root,
    )
    .unwrap()
}

fn finish(j: &mut TestJob) -> RestoreReport {
    loop {
        if let Step::Done(r) = j
            .step(Budget {
                max_bytes: 64,
                max_ops: 2,
            })
            .unwrap()
        {
            return r;
        }
    }
}

fn file(fs: &MemRestoreFs, path: &str) -> Option<(Vec<u8>, u32)> {
    match fs.nodes.get(path) {
        Some((MemNode::File { data, mode }, _)) => Some((data.clone(), *mode)),
        _ => None,
    }
}

fn one() -> Budget {
    Budget {
        max_bytes: 1,
        max_ops: 1,
    }
}

#[test]
fn restores_every_approved_file_byte_for_byte_and_checks_git_refs() {
    let fx = fixture();
    let mut fs = MemRestoreFs::new(true);
    fs.add(
        "keep.txt",
        MemNode::File {
            data: b"mine".to_vec(),
            mode: 0o600,
        },
    );
    let before = fs.snapshot_except(&["demo-restored"]);
    let (pre, plan) = precheck(&fx, &fs, "demo-restored");
    assert!(!pre.blocking_failed, "{pre:#?}");
    let mut j = start(&fx, fs, plan.unwrap());
    let r = finish(&mut j);
    assert_eq!(r.status, RestoreStatus::Verified, "{r:#?}");
    assert_eq!(r.git_check.refs, RefCheck::Match);
    assert_eq!(r.writes_outside_destination, 0);
    assert!(r.mismatches.is_empty());
    let (fs, journals) = j.into_parts();

    for (p, want) in [
        ("README.md", "# demo\n"),
        ("src/a.rs", "same\n"),
        ("src/deep/b.rs", "same\n"),
        ("empty.txt", ""),
    ] {
        let (data, _) =
            file(&fs, &format!("demo-restored/{p}")).unwrap_or_else(|| panic!("{p} missing"));
        assert_eq!(data, want.as_bytes(), "{p}");
    }
    let (_, mode) = file(&fs, "demo-restored/bin/run.sh").unwrap();
    // Capture never records setuid, setgid, or sticky, so there's nothing left to strip;
    // the executable bit is kept.
    assert_eq!(mode, 0o755);
    assert!(r.stripped_mode_bits.is_empty());
    assert!(r.executables.contains(&"bin/run.sh".to_owned()));
    assert!(file(&fs, "demo-restored/.env").is_none());
    assert!(file(&fs, "demo-restored/ln").is_none());
    assert!(
        r.not_restored
            .iter()
            .any(|n| n.path == "ln" && n.reason == "symlink_not_restored")
    );
    assert!(
        r.not_restored
            .iter()
            .any(|n| n.path == ".env" && n.reason.starts_with("excluded_at_capture:"))
    );
    assert_eq!(names(&fs).len(), 2, "only keep.txt and the target");
    assert_eq!(
        fs.snapshot_except(&["demo-restored"]),
        before,
        "other entries changed"
    );
    let jn = journals.load(OP).unwrap().unwrap();
    assert_eq!(jn.state, RestoreState::Cleaned);
    let states: Vec<RestoreState> = jn.history.iter().map(|h| h.state).collect();
    assert_eq!(
        states,
        vec![
            RestoreState::Staged,
            RestoreState::Staged,
            RestoreState::Written,
            RestoreState::Verified,
            RestoreState::Committed,
            RestoreState::Cleaned
        ]
    );
}

#[test]
fn an_existing_target_or_any_variant_of_it_is_refused() {
    let fx = fixture();
    for (existing, node) in [
        ("demo", MemNode::Dir),
        (
            "Demo",
            MemNode::File {
                data: vec![],
                mode: 0o644,
            },
        ),
        (
            "DEMO",
            MemNode::Symlink {
                target: "nowhere".into(),
            },
        ),
    ] {
        let mut fs = MemRestoreFs::new(true);
        fs.add(existing, node);
        let (pre, plan) = precheck(&fx, &fs, "demo");
        assert!(plan.is_none());
        assert_eq!(
            pre.first_error(),
            Some(ErrorCode::RestoreDestExists),
            "{existing}"
        );
    }
    let mut fs = MemRestoreFs::new(false);
    fs.add("Cafe\u{301}", MemNode::Dir);
    let (pre, _) = precheck(&fx, &fs, "Caf\u{e9}");
    assert_eq!(
        pre.first_error(),
        Some(ErrorCode::RestoreDestExists),
        "NFC variant"
    );
}

#[test]
fn a_target_that_appears_just_before_the_commit_is_never_replaced() {
    let fx = fixture();
    let fs = MemRestoreFs::new(true);
    let (_, plan) = precheck(&fx, &fs, "demo-restored");
    let mut j = start(&fx, fs, plan.unwrap());
    for _ in 0..200 {
        let _ = j.step(Budget::UNBOUNDED).unwrap();
        let verified = j
            .journal_store()
            .journals
            .values()
            .any(|x| x.state == RestoreState::Verified);
        if verified {
            break;
        }
    }
    assert!(!j.committed());
    j.fs_mut().add(
        "Demo-Restored",
        MemNode::File {
            data: b"theirs".to_vec(),
            mode: 0o644,
        },
    );
    let r = finish(&mut j);
    assert_eq!(r.status, RestoreStatus::RolledBack);
    assert_eq!(
        r.error.as_ref().map(|e| e.code),
        Some(ErrorCode::RestoreDestExists)
    );
    let (fs, _) = j.into_parts();
    assert_eq!(
        file(&fs, "Demo-Restored"),
        Some((b"theirs".to_vec(), 0o644))
    );
    assert_eq!(
        names(&fs),
        vec!["Demo-Restored".to_owned()],
        "no staging left"
    );
}

#[test]
fn a_fault_at_any_operation_leaves_either_nothing_or_a_complete_verified_folder() {
    let fx = fixture();
    let fs = MemRestoreFs::new(true);
    let (_, plan) = precheck(&fx, &fs, "demo-restored");
    let plan = plan.unwrap();
    let mut j = start(&fx, fs, plan.clone());
    finish(&mut j);
    let n = j.fs().ops;
    assert!(n > 10);
    for k in 1..=n {
        for fault in [
            FsFault::Io("boom".into()),
            FsFault::NoSpace,
            FsFault::Exists,
        ] {
            let mut fs = MemRestoreFs::new(true);
            fs.fail_at = Some((k, fault.clone()));
            let mut j = start(&fx, fs, plan.clone());
            let r = finish(&mut j);
            let committed = j.committed();
            let (fs, journals) = j.into_parts();
            let target = fs.nodes.contains_key("demo-restored");
            assert_eq!(target, committed, "op {k} {fault:?}: target iff committed");
            if r.status == RestoreStatus::RolledBack {
                assert!(r.error.is_some(), "op {k}");
            }
            if target {
                assert!(
                    fs.nodes.contains_key("demo-restored/src/deep/b.rs"),
                    "op {k}: partial target"
                );
            }
            let staging_left = names(&fs)
                .iter()
                .any(|t| t.starts_with(".dormbit-staging-"));
            if staging_left {
                // Only when removing it failed; the journal keeps it recoverable.
                assert!(journals.load(OP).unwrap().is_some(), "op {k}");
                assert!(!r.warnings.is_empty(), "op {k}: staging left silently");
            }
        }
    }
}

#[test]
fn cancel_before_the_commit_rolls_back_and_after_it_is_too_late() {
    let fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    let plan = plan.unwrap();

    let mut j = start(&fx, MemRestoreFs::new(true), plan.clone());
    let _ = j.step(one()).unwrap();
    let _ = j.step(one()).unwrap();
    assert_eq!(j.try_cancel(), CancelResult::Accepted);
    let r = finish(&mut j);
    assert_eq!(r.status, RestoreStatus::RolledBack);
    assert_eq!(r.error.map(|e| e.code), Some(ErrorCode::Cancelled));
    let (fs, _) = j.into_parts();
    assert!(names(&fs).is_empty());

    let mut j = start(&fx, MemRestoreFs::new(true), plan);
    while !j.committed() {
        let _ = j.step(Budget::UNBOUNDED).unwrap();
    }
    assert_eq!(j.try_cancel(), CancelResult::TooLate);
    assert_eq!(finish(&mut j).status, RestoreStatus::Verified);
}

#[test]
fn recovery_after_a_crash_before_and_after_the_commit() {
    let fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    let plan = plan.unwrap();

    let mut j = start(&fx, MemRestoreFs::new(true), plan.clone());
    for _ in 0..3 {
        let _ = j.step(one()).unwrap();
    }
    assert!(!j.committed());
    let (mut fs, mut journals) = j.into_parts();
    assert!(
        names(&fs)
            .iter()
            .any(|t| t.starts_with(".dormbit-staging-"))
    );
    let root = fs.root();
    let out = recover(&mut fs, &root, &mut journals, OP, &FixedClock(1)).unwrap();
    assert_eq!(out.action, RecoveryAction::RolledBack);
    assert!(out.report.recovered);
    assert!(names(&fs).is_empty());
    assert!(journals.pending().unwrap().is_empty());

    let mut j = start(&fx, MemRestoreFs::new(true), plan);
    while !j.committed() {
        let _ = j.step(Budget::UNBOUNDED).unwrap();
    }
    let (mut fs, mut journals) = j.into_parts();
    let staging = format!(".dormbit-staging-{OP}");
    let target_before = fs.snapshot_except(&[staging.as_str()]);
    let root = fs.root();
    let out = recover(&mut fs, &root, &mut journals, OP, &FixedClock(1)).unwrap();
    assert_eq!(out.action, RecoveryAction::Committed);
    assert_eq!(out.report.status, RestoreStatus::CommittedUnverified);
    assert_eq!(names(&fs), vec!["demo-restored".to_owned()]);
    assert_eq!(
        fs.snapshot_except(&[]),
        target_before,
        "recovery touched the target"
    );
}

#[test]
fn recovery_never_deletes_a_staging_folder_whose_owner_token_changed() {
    let fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    let mut j = start(&fx, MemRestoreFs::new(true), plan.unwrap());
    for _ in 0..2 {
        let _ = j.step(one()).unwrap();
    }
    let (mut fs, mut journals) = j.into_parts();
    let staging = names(&fs)
        .into_iter()
        .find(|t| t.starts_with(".dormbit-staging-"))
        .unwrap();
    let key = format!("{staging}/.dormbit-owner");
    if let Some((MemNode::File { data, .. }, _)) = fs.nodes.get_mut(&key) {
        data.extend_from_slice(b"x");
    }
    let root = fs.root();
    let out = recover(&mut fs, &root, &mut journals, OP, &FixedClock(1)).unwrap();
    assert_eq!(out.action, RecoveryAction::LeftInPlace);
    assert!(names(&fs).contains(&staging));
    assert!(
        out.report
            .warnings
            .iter()
            .any(|w| w.starts_with("Not deleted"))
    );
}

#[test]
fn a_capsule_changed_after_verification_is_never_restored() {
    let mut fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    // Flip a byte inside the first object's data (after marker and manifest), so the index
    // still opens and the damage is found when that object is read.
    let field = std::str::from_utf8(&fx.bytes[1024 + 124..1024 + 135]).unwrap();
    let manifest_size = usize::from_str_radix(field, 8).unwrap();
    let first_data = 1024 + 512 + manifest_size.div_ceil(512) * 512 + 512;
    fx.bytes[first_data + 12] ^= 0x01;
    let mut j = start(&fx, MemRestoreFs::new(true), plan.unwrap());
    let r = finish(&mut j);
    assert_eq!(r.status, RestoreStatus::RolledBack, "{r:#?}");
    let (fs, _) = j.into_parts();
    assert!(names(&fs).is_empty());
}

// ---- planner ---------------------------------------------------------------------------------

fn with_paths(fx: &Fixture, rename: &[(&str, &str)]) -> Manifest {
    let mut m = fx.manifest.clone();
    for e in &mut m.entries {
        if let dormbit_core::capsule::manifest::Entry::File { path, .. } = e {
            for (from, to) in rename {
                if path == from {
                    *path = (*to).to_owned();
                }
            }
        }
    }
    m
}

fn plan_for(fx: &Fixture, m: &Manifest, v: VolumeFacts) -> PrecheckReport {
    let ms = fx.verification.manifest_sha256.clone().unwrap();
    plan_restore(&input(fx, m, &ms, "t", &[], v)).report
}

#[test]
fn case_and_normalization_collisions_follow_the_volume() {
    let fx = fixture();
    let ins = || volume(CaseSensitivity::Insensitive);
    let sens = || volume(CaseSensitivity::Sensitive);
    let m = with_paths(&fx, &[("src/a.rs", "README.MD")]);
    assert_eq!(
        plan_for(&fx, &m, ins()).first_error(),
        Some(ErrorCode::RestoreCollision)
    );
    assert_eq!(
        plan_for(&fx, &m, volume(CaseSensitivity::Unknown)).first_error(),
        Some(ErrorCode::RestoreCollision)
    );
    assert_eq!(plan_for(&fx, &m, sens()).first_error(), None);

    let m = with_paths(&fx, &[("src/a.rs", "Src/a.rs")]);
    assert_eq!(
        plan_for(&fx, &m, ins()).first_error(),
        Some(ErrorCode::RestoreCollision),
        "folder variants"
    );

    let m = with_paths(
        &fx,
        &[("README.md", "Caf\u{e9}"), ("empty.txt", "Cafe\u{301}")],
    );
    assert_eq!(
        plan_for(&fx, &m, sens()).first_error(),
        Some(ErrorCode::RestoreCollision),
        "NFC always"
    );

    let m = with_paths(&fx, &[("README.md", "src")]);
    assert_eq!(
        plan_for(&fx, &m, sens()).first_error(),
        Some(ErrorCode::RestoreCollision),
        "file vs folder"
    );
}

#[test]
fn unsafe_paths_space_and_volume_are_refused() {
    let fx = fixture();
    for bad in ["../x", "/etc/x", "a/./b", "a//b"] {
        let m = with_paths(&fx, &[("README.md", bad)]);
        assert_eq!(
            plan_for(&fx, &m, volume(CaseSensitivity::Insensitive)).first_error(),
            Some(ErrorCode::PathUnsafe),
            "{bad}"
        );
    }
    let mut v = volume(CaseSensitivity::Insensitive);
    v.free_bytes = 10;
    assert_eq!(
        plan_for(&fx, &fx.manifest, v).first_error(),
        Some(ErrorCode::RestoreInsufficientSpace)
    );
    let mut v = volume(CaseSensitivity::Insensitive);
    v.fs_type = "msdos".into();
    assert_eq!(
        plan_for(&fx, &fx.manifest, v).first_error(),
        Some(ErrorCode::RestoreVolumeUnsupported)
    );
}

#[test]
fn an_unverified_or_changed_capsule_is_refused_and_every_check_is_reported() {
    let fx = fixture();
    let other = sha256_hex(b"other");
    let r = plan_restore(&PlanInput {
        manifest: &fx.manifest,
        manifest_sha256_now: &other,
        verification: None,
        target_name: "t",
        target_display: "~/t".into(),
        parent_entries: &[],
        volume: volume(CaseSensitivity::Insensitive),
        inside_source: Some(true),
        inside_library: false,
    })
    .report;
    assert!(r.blocking_failed);
    assert_eq!(r.checks.len(), 11);
    let failed: Vec<&str> = r
        .checks
        .iter()
        .filter(|c| c.status == PrecheckState::Fail)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        failed,
        vec!["capsule_verified", "manifest_hash", "target_location"]
    );
}

#[test]
fn recovery_refuses_a_journal_whose_staging_name_was_changed() {
    let fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    let mut j = start(&fx, MemRestoreFs::new(true), plan.unwrap());
    for _ in 0..2 {
        let _ = j.step(one()).unwrap();
    }
    let (mut fs, mut journals) = j.into_parts();
    fs.add("projects", MemNode::Dir);
    fs.add(
        "projects/.dormbit-owner",
        MemNode::File {
            data: format!("{OP}:{}", token()).into_bytes(),
            mode: 0o600,
        },
    );
    let mut jn = journals.load(OP).unwrap().unwrap();
    jn.staging = "projects".into();
    journals.journals.insert(OP.into(), jn);
    let root = fs.root();
    let e = recover(&mut fs, &root, &mut journals, OP, &FixedClock(1)).err();
    assert_eq!(e.map(|e| e.code), Some(ErrorCode::PathUnsafe));
    assert!(
        fs.nodes.contains_key("projects/.dormbit-owner"),
        "nothing deleted"
    );
}

#[test]
fn recovery_never_rewrites_the_report_of_a_finished_restore() {
    let fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    let mut j = start(&fx, MemRestoreFs::new(true), plan.unwrap());
    let done = finish(&mut j);
    assert_eq!(done.status, RestoreStatus::Verified);
    let (mut fs, mut journals) = j.into_parts();
    // The user moves the restored folder away; the inode no longer matches the target name.
    fs.rename_exclusive(
        &String::new(),
        &dormbit_core::restore::keys::validate_target_name("demo-restored").unwrap(),
        &String::new(),
        &dormbit_core::restore::keys::validate_target_name("moved").unwrap(),
    )
    .unwrap();
    let root = fs.root();
    let out = recover(&mut fs, &root, &mut journals, OP, &FixedClock(1)).unwrap();
    assert_eq!(out.action, RecoveryAction::AlreadyFinished);
    assert_eq!(out.report, done);
    assert_eq!(journals.report(OP).unwrap(), Some(done));
}

#[test]
fn a_rolled_back_restore_reports_nothing_written() {
    let fx = fixture();
    let (_, plan) = precheck(&fx, &MemRestoreFs::new(true), "demo-restored");
    let mut j = start(&fx, MemRestoreFs::new(true), plan.unwrap());
    for _ in 0..4 {
        let _ = j.step(one()).unwrap();
    }
    j.try_cancel();
    let r = finish(&mut j);
    assert_eq!(r.status, RestoreStatus::RolledBack);
    assert_eq!(r.files_written.files, 0);
}
