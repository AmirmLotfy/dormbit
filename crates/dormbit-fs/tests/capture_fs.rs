//! End-to-end capture on a real folder (capsule-and-verification Task 2.10): the cap-std
//! `ReadVfs`, the library sink, and the registry together.

use std::collections::BTreeMap;
use std::path::Path;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use dormbit_core::capsule::manifest::ManifestProject;
use dormbit_core::capsule::{
    Budget, CapsuleLimits, CaptureContext, CaptureJob, Job, Registry, Step, run_to_end,
};
use dormbit_core::model::{CapsuleId, Decision, ErrorCode, GitTreatment, ProjectId, ScopeId};
use dormbit_core::plan::{ack_id, apply_edit, approve, build_plan};
use dormbit_core::util::sha256_hex;
use dormbit_core::vfs::{CountingIds, FixedClock};
use dormbit_fs::{FsCapsuleRegistry, FsCapsuleSink, FsVfs};

fn open(p: &Path) -> Dir {
    Dir::open_ambient_dir(p, ambient_authority()).unwrap()
}

fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let ft = std::fs::symlink_metadata(&p).unwrap();
            if ft.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("link:{}", std::fs::read_link(&p).unwrap().display()),
                );
            } else if ft.is_dir() {
                out.insert(rel.clone(), "dir".into());
                walk(base, &p, out);
            } else {
                out.insert(rel, sha256_hex(&std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(root, root, &mut m);
    m
}

fn project() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    let p = t.path();
    std::fs::create_dir_all(p.join("src")).unwrap();
    std::fs::create_dir_all(p.join("node_modules/x")).unwrap();
    std::fs::write(p.join("README.md"), b"# demo\n").unwrap();
    std::fs::write(p.join("src/main.rs"), b"fn main() {}\n").unwrap();
    std::fs::write(p.join(".env"), b"X=1\n").unwrap();
    std::fs::write(p.join("node_modules/x/i.js"), b"1\n").unwrap();
    std::os::unix::fs::symlink("README.md", p.join("readme-link")).unwrap();
    t
}

struct Setup {
    _src: tempfile::TempDir,
    src_path: std::path::PathBuf,
    lib: tempfile::TempDir,
    state: tempfile::TempDir,
}

fn setup() -> Setup {
    let src = project();
    Setup {
        src_path: src.path().to_owned(),
        _src: src,
        lib: tempfile::tempdir().unwrap(),
        state: tempfile::tempdir().unwrap(),
    }
}

fn ctx() -> CaptureContext {
    CaptureContext {
        app_version: "0.1.0".into(),
        project_id: ProjectId::from("p"),
        project: ManifestProject {
            name: "demo".into(),
            name_source: "folder".into(),
        },
        library_grant: ScopeId::from("lib"),
        limits: CapsuleLimits::default(),
    }
}

fn names(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(p)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

#[test]
fn captures_a_real_folder_into_the_library_and_leaves_the_source_unchanged() {
    let s = setup();
    let before = snapshot(&s.src_path);
    let vfs = FsVfs::new(open(&s.src_path));
    let mut plan = build_plan(
        &vfs,
        GitTreatment::Full,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap();
    apply_edit(&mut plan, "node_modules", Decision::Exclude).unwrap();
    let acks: Vec<String> = plan
        .items
        .iter()
        .filter_map(|i| i.warning.as_deref().map(|w| ack_id(&i.path, w)))
        .collect();
    let ok = approve(
        &plan,
        &plan.digest,
        &acks,
        &FixedClock(1),
        &CapsuleLimits::default(),
    )
    .unwrap();
    let ws = FsCapsuleSink::new(open(s.lib.path()))
        .begin("0a", "0b")
        .unwrap();
    let reg = FsCapsuleRegistry::open(open(s.state.path())).unwrap();
    let mut job = CaptureJob::new(
        &vfs,
        plan,
        ok,
        ws,
        reg,
        &FixedClock(1_800_000_000),
        &CountingIds::default(),
        ctx(),
    )
    .unwrap();
    let out = run_to_end(&mut job).unwrap();

    assert_eq!(names(s.lib.path()), vec![out.record.file_name.clone()]);
    let bytes = std::fs::read(s.lib.path().join(&out.record.file_name)).unwrap();
    assert_eq!(sha256_hex(&bytes), out.record.file_sha256);
    assert_eq!(snapshot(&s.src_path), before, "source tree changed");
    let paths: Vec<&str> = out.manifest.entries.iter().map(|e| e.path()).collect();
    assert_eq!(paths, vec!["README.md", "readme-link", "src/main.rs"]);

    let reg = FsCapsuleRegistry::open(open(s.state.path())).unwrap();
    let rec = reg
        .get(&CapsuleId::from(out.record.capsule_id.as_str()))
        .unwrap();
    assert_eq!(rec, Some(out.record));

    // The verifier reopens the file from disk and agrees with the registry.
    let src =
        dormbit_fs::FsCapsuleSource::new(open(s.lib.path()), &names(s.lib.path())[0]).unwrap();
    let report = dormbit_core::capsule::verify_all(src, reg, CapsuleLimits::default()).unwrap();
    assert_eq!(report.status, dormbit_core::capsule::ReportStatus::Verified);
    assert_eq!(
        report.registry_match,
        dormbit_core::capsule::verify::RegistryState::Match
    );

    // The system tar reads it (an independent reader).
    let listed = std::process::Command::new("tar")
        .arg("-tf")
        .arg(s.lib.path().join(&names(s.lib.path())[0]))
        .output()
        .unwrap();
    assert!(listed.status.success());
}

#[test]
fn cancel_and_failure_leave_no_staging_folder_and_no_capsule() {
    let s = setup();
    let vfs = FsVfs::new(open(&s.src_path));
    let mut plan = build_plan(
        &vfs,
        GitTreatment::Full,
        &CapsuleLimits::default(),
        &CountingIds::default(),
    )
    .unwrap();
    apply_edit(&mut plan, "node_modules", Decision::Exclude).unwrap();
    let ok = approve(
        &plan,
        &plan.digest,
        &[],
        &FixedClock(1),
        &CapsuleLimits::default(),
    )
    .unwrap();

    // Cancel after the first step.
    let ws = FsCapsuleSink::new(open(s.lib.path()))
        .begin("0c", "0d")
        .unwrap();
    let reg = FsCapsuleRegistry::open(open(s.state.path())).unwrap();
    let mut job = CaptureJob::new(
        &vfs,
        plan.clone(),
        ok.clone(),
        ws,
        reg,
        &FixedClock(2),
        &CountingIds::default(),
        ctx(),
    )
    .unwrap();
    assert!(matches!(
        job.step(Budget {
            max_bytes: 1,
            max_ops: 1
        }),
        Ok(Step::Progress(_))
    ));
    job.cancel();
    assert_eq!(
        job.step(Budget::UNBOUNDED).err().map(|e| e.code),
        Some(ErrorCode::Cancelled)
    );
    assert!(names(s.lib.path()).is_empty(), "staging folder left behind");

    // A file that changes after approval in a way the capture notices: delete it.
    std::fs::remove_file(s.src_path.join("src/main.rs")).unwrap();
    let ws = FsCapsuleSink::new(open(s.lib.path()))
        .begin("0e", "0f")
        .unwrap();
    let reg = FsCapsuleRegistry::open(open(s.state.path())).unwrap();
    let mut job = CaptureJob::new(
        &vfs,
        plan,
        ok,
        ws,
        reg,
        &FixedClock(3),
        &CountingIds::default(),
        ctx(),
    )
    .unwrap();
    assert_eq!(
        run_to_end(&mut job).err().map(|e| e.code),
        Some(ErrorCode::FileChangedDuringCapture)
    );
    assert!(names(s.lib.path()).is_empty());
    assert!(names(s.state.path()).is_empty(), "registry untouched");
}
