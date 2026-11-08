//! The desktop journey over real folders. It makes the same core calls as the web demo's
//! engine (`crates/dormbit-wasm/src/engine.rs`) and returns the same JSON shapes, so the UI
//! parses both with the schemas in `packages/schema/src/core.ts` (D-51).
//!
//! - Folders the user granted are opened once with cap-std; every read goes through `FsVfs`,
//!   which walks one component at a time and never follows a symlink.
//! - Git is read with libgit2 (`dormbit-git`), only when `.git` is a real folder.
//! - Capsules, the registry, and restore journals live in DORMBIT's own folders under the app
//!   data directory. Restores write only inside the granted target, through `CapStdRestoreFs`.
//! - Absolute paths never leave this module: results carry grant IDs and `~` display paths.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use dormbit_core::brief::{RestoreFacts, generate, render_markdown};
use dormbit_core::capsule::manifest::{Manifest, ManifestProject};
use dormbit_core::capsule::{
    Budget, CapsuleLimits, CapsuleRecord, CaptureContext, CaptureJob, Job, JobProgress, Report,
    Step, verify_all,
};
use dormbit_core::discovery::{DiscoveredProject, DiscoveryLimits, discover, project_view};
use dormbit_core::evidence::{EvidenceInput, EvidenceResult, evaluate};
use dormbit_core::model::{
    ApprovedPlan, CapturePlan, Decision, DormbitError, ErrorCode, GitSnapshot, GitTreatment,
    ProjectId, ScopeId,
};
use dormbit_core::plan::{apply_edit, approve, build_plan};
use dormbit_core::restore::report::RefCheck;
use dormbit_core::restore::{
    OpenedCapsule, PlanInput, RestoreContext, RestoreJob, RestoreReport, RestoreStatus,
    plan_restore,
};
use dormbit_core::vfs::{Clock, EntryKind, IdGen, ReadVfs, RelPath};
use dormbit_fs::restore::{CapStdRestoreFs, FileJournalStore, list_names, probe_volume};
use dormbit_fs::{FsCapsuleRegistry, FsCapsuleSink, FsCapsuleSource, FsVfs};
use serde_json::{Value, json};

use crate::redact::redact_home;

/// Jobs run in small budgets so the UI can show their real progress phase by phase.
const STEP: Budget = Budget {
    max_bytes: 256 << 10,
    max_ops: 32,
};
const MAX_MANIFEST: u64 = 1 << 26;
const LIBRARY_GRANT: &str = "app-library";

pub struct SystemClock;
impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0)
    }
}

pub struct RandomIds;
impl IdGen for RandomIds {
    fn new_uuid(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

fn err(code: ErrorCode, msg: impl Into<String>) -> DormbitError {
    DormbitError::new(code, msg)
}

fn io(what: &str) -> impl Fn(std::io::Error) -> DormbitError + '_ {
    move |_| {
        err(
            ErrorCode::IoFailed,
            format!("{what} could not be read or written."),
        )
    }
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<Value, DormbitError> {
    serde_json::to_value(v).map_err(|_| err(ErrorCode::IoFailed, "A result could not be encoded."))
}

/// A folder the user picked in the native panel, opened once.
struct Grant {
    id: String,
    root: PathBuf,
    display: String,
    dir: Dir,
}

impl Grant {
    fn dir(&self) -> Result<Dir, DormbitError> {
        self.dir.try_clone().map_err(io("The granted folder"))
    }
}

#[derive(Default)]
struct Journey {
    read: Option<Grant>,
    target: Option<Grant>,
    projects: Vec<DiscoveredProject>,
    project: Option<DiscoveredProject>,
    git: Option<GitSnapshot>,
    plan: Option<CapturePlan>,
    approved: Option<ApprovedPlan>,
    record: Option<CapsuleRecord>,
    manifest: Option<Manifest>,
    verification: Option<Report>,
    restore: Option<RestoreReport>,
}

pub struct Native {
    home: PathBuf,
    library: PathBuf,
    registry: PathBuf,
    journals: PathBuf,
    st: Mutex<Journey>,
}

fn open_dir(p: &Path) -> Result<Dir, DormbitError> {
    Dir::open_ambient_dir(p, ambient_authority()).map_err(io("A folder"))
}

fn rel(p: &DiscoveredProject) -> &str {
    if p.rel_root == "." { "" } else { &p.rel_root }
}

fn run_steps<J: Job>(job: &mut J) -> Result<(Vec<JobProgress>, J::Output), DormbitError> {
    let mut events = Vec::new();
    loop {
        match job.step(STEP)? {
            Step::Progress(p) => {
                if events.last() != Some(&p) {
                    events.push(p);
                }
            }
            Step::Done(o) => return Ok((events, o)),
        }
    }
}

impl Native {
    /// Creates DORMBIT's own folders (capsule library, registry, restore journals) under
    /// `data`. Nothing else is created.
    pub fn open(data: &Path, home: &Path) -> Result<Self, DormbitError> {
        let library = data.join("Capsules");
        let registry = data.join("Registry");
        let journals = data.join("Journals");
        for d in [&library, &registry, &journals] {
            std::fs::create_dir_all(d).map_err(io("DORMBIT's app data folder"))?;
        }
        Ok(Self {
            home: home.to_path_buf(),
            library,
            registry,
            journals,
            st: Mutex::new(Journey::default()),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Journey>, DormbitError> {
        self.st
            .lock()
            .map_err(|_| err(ErrorCode::IoFailed, "DORMBIT's state is unavailable."))
    }

    fn display(&self, p: &Path) -> String {
        redact_home(&p.to_string_lossy(), &self.home)
    }

    fn grant(&self, picked: &Path) -> Result<Grant, DormbitError> {
        let dir = open_dir(picked)?;
        Ok(Grant {
            id: RandomIds.new_uuid(),
            root: picked.to_path_buf(),
            display: self.display(picked),
            dir,
        })
    }

    /// The folder to look for projects in. Starts a fresh journey.
    pub fn grant_read(&self, picked: &Path) -> Result<Value, DormbitError> {
        let g = self.grant(picked)?;
        let view = json!({ "id": g.id, "kind": "Read", "root_display": g.display });
        let mut st = self.lock()?;
        *st = Journey {
            read: Some(g),
            ..Journey::default()
        };
        Ok(view)
    }

    pub fn discover(&self, scope_id: &str) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let g = st
            .read
            .as_ref()
            .filter(|g| g.id == scope_id)
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Grant a folder first."))?;
        let vfs = FsVfs::new(g.dir()?);
        let out = discover(
            &vfs,
            &g.id,
            SystemClock.now_unix(),
            &DiscoveryLimits::default(),
        );
        let root_display = g.display.clone();
        st.projects = out.projects.clone();
        Ok(json!({ "root_display": root_display, "result": out }))
    }

    fn select(st: &mut Journey, project_id: &str) -> Result<DiscoveredProject, DormbitError> {
        let p = st
            .projects
            .iter()
            .find(|p| p.id == project_id)
            .cloned()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "That project isn't in the scan."))?;
        if st.project.as_ref().map(|q| &q.id) != Some(&p.id) {
            st.project = Some(p.clone());
            st.git = None;
            st.plan = None;
            st.approved = None;
        }
        Ok(p)
    }

    fn evidence_of(
        &self,
        st: &mut Journey,
        p: &DiscoveredProject,
    ) -> Result<EvidenceResult, DormbitError> {
        let g = st
            .read
            .as_ref()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Grant a folder first."))?;
        let vfs = FsVfs::new(g.dir()?);
        // libgit2 opens the repository only when `.git` is a real folder (not a symlink and
        // not a `gitdir:` file that could point outside the grant).
        if st.git.is_none() {
            let dot_git = if rel(p).is_empty() {
                ".git".to_owned()
            } else {
                format!("{}/.git", rel(p))
            };
            let real_dir = RelPath::parse(&dot_git)
                .ok()
                .and_then(|r| vfs.symlink_metadata(&r).ok())
                .is_some_and(|m| m.kind == EntryKind::Dir);
            if real_dir {
                st.git = dormbit_git::inspect(&g.root.join(rel(p))).ok();
            }
        }
        let view = project_view(&vfs, rel(p));
        let folder = if rel(p).is_empty() {
            g.root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("project")
                .to_owned()
        } else {
            rel(p).rsplit('/').next().unwrap_or("project").to_owned()
        };
        Ok(evaluate(&EvidenceInput {
            vfs: &view,
            folder_name: &folder,
            git: st.git.as_ref(),
            agent_sessions: &[],
        }))
    }

    pub fn evidence(&self, project_id: &str) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let p = Self::select(&mut st, project_id)?;
        let e = self.evidence_of(&mut st, &p)?;
        to_json(&e)
    }

    pub fn plan_create(&self, project_id: &str) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let p = Self::select(&mut st, project_id)?;
        let g = st
            .read
            .as_ref()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Grant a folder first."))?;
        let vfs = FsVfs::new(g.dir()?);
        let view = project_view(&vfs, rel(&p));
        let plan = build_plan(
            &view,
            GitTreatment::Full,
            &CapsuleLimits::default(),
            &RandomIds,
        )?;
        st.plan = Some(plan.clone());
        st.approved = None;
        to_json(&plan)
    }

    fn plan_mut<'a>(
        st: &'a mut Journey,
        plan_id: &str,
    ) -> Result<&'a mut CapturePlan, DormbitError> {
        st.plan
            .as_mut()
            .filter(|p| p.plan_id.0 == plan_id)
            .ok_or_else(|| err(ErrorCode::PlanIncomplete, "Create a plan first."))
    }

    pub fn plan_set_decision(
        &self,
        plan_id: &str,
        path: &str,
        decision: Decision,
    ) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let plan = Self::plan_mut(&mut st, plan_id)?;
        apply_edit(plan, path, decision)?;
        let out = to_json(plan)?;
        st.approved = None;
        Ok(out)
    }

    pub fn plan_approve(
        &self,
        plan_id: &str,
        digest: &str,
        acks: &[String],
    ) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let plan = Self::plan_mut(&mut st, plan_id)?;
        let ok = approve(plan, digest, acks, &SystemClock, &CapsuleLimits::default())?;
        st.approved = Some(ok.clone());
        to_json(&ok)
    }

    pub fn capture(&self, plan_id: &str) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let plan = Self::plan_mut(&mut st, plan_id)?.clone();
        let ok = st
            .approved
            .clone()
            .ok_or_else(|| err(ErrorCode::PlanIncomplete, "Approve the plan first."))?;
        let p = st
            .project
            .clone()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Pick a project first."))?;
        let g = st
            .read
            .as_ref()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Grant a folder first."))?;
        let vfs = FsVfs::new(g.dir()?);
        let view = project_view(&vfs, rel(&p));
        let op = RandomIds.new_uuid();
        let owner = uuid::Uuid::new_v4().simple().to_string();
        let ws = FsCapsuleSink::new(open_dir(&self.library)?).begin(&op, &owner)?;
        let reg = FsCapsuleRegistry::open(open_dir(&self.registry)?)?;
        let ctx = CaptureContext {
            app_version: dormbit_core::VERSION.into(),
            project_id: ProjectId(p.id.clone()),
            project: ManifestProject {
                name: p.name.clone(),
                name_source: p.name_source.clone(),
            },
            library_grant: ScopeId::from(LIBRARY_GRANT),
            limits: CapsuleLimits::default(),
        };
        let mut job = CaptureJob::new(&view, plan, ok, ws, reg, &SystemClock, &RandomIds, ctx)?;
        let (events, out) = run_steps(&mut job)?;
        st.record = Some(out.record.clone());
        st.manifest = Some(out.manifest.clone());
        st.verification = None;
        st.restore = None;
        Ok(
            json!({ "events": events, "record": out.record, "manifest": out.manifest, "not_captured": out.not_captured }),
        )
    }

    fn record(st: &Journey, capsule_id: &str) -> Result<CapsuleRecord, DormbitError> {
        st.record
            .clone()
            .filter(|r| r.capsule_id.0 == capsule_id)
            .ok_or_else(|| err(ErrorCode::CapsuleMalformed, "Make a capsule first."))
    }

    pub fn verify(&self, capsule_id: &str) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let r = Self::record(&st, capsule_id)?;
        let src = FsCapsuleSource::new(open_dir(&self.library)?, &r.file_name)?;
        let reg = FsCapsuleRegistry::open(open_dir(&self.registry)?)?;
        let report = verify_all(src, reg, CapsuleLimits::default())?;
        if report.status == dormbit_core::capsule::ReportStatus::Verified {
            FsCapsuleRegistry::open(open_dir(&self.registry)?)?
                .set_last_verified(&r.capsule_id, SystemClock.now_unix())?;
        }
        st.verification = Some(report.clone());
        to_json(&report)
    }

    fn listing(&self, g: &Grant) -> Result<Vec<Value>, DormbitError> {
        Ok(list_names(&g.dir()?)?
            .into_iter()
            .map(|n| json!({ "name": n, "kind": "entry" }))
            .collect())
    }

    /// The folder new restores are created in.
    pub fn grant_target(&self, picked: &Path) -> Result<Value, DormbitError> {
        let g = self.grant(picked)?;
        let entries = self.listing(&g)?;
        let view = json!({ "id": g.id, "kind": "RestoreTarget", "root_display": g.display, "entries": entries });
        self.lock()?.target = Some(g);
        Ok(view)
    }

    fn precheck(
        &self,
        st: &Journey,
        name: &str,
    ) -> Result<
        (
            dormbit_core::restore::PrecheckOutcome,
            OpenedCapsule<std::fs::File>,
        ),
        DormbitError,
    > {
        let r = st
            .record
            .as_ref()
            .ok_or_else(|| err(ErrorCode::CapsuleMalformed, "Make a capsule first."))?;
        let t = st
            .target
            .as_ref()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Choose where to restore first."))?;
        let file = open_dir(&self.library)?
            .open(&r.file_name)
            .map_err(io("The capsule"))?
            .into_std();
        let opened = OpenedCapsule::open(file, MAX_MANIFEST)?;
        let parent = t.dir()?;
        let names = list_names(&parent)?;
        let volume = probe_volume(&parent)?;
        let canon = |p: &Path| std::fs::canonicalize(p).ok();
        let target = canon(&t.root);
        let source = st
            .read
            .as_ref()
            .zip(st.project.as_ref())
            .and_then(|(g, p)| canon(&g.root.join(rel(p))));
        let inside_source = target
            .as_ref()
            .zip(source.as_ref())
            .map(|(t, s)| t.starts_with(s));
        let inside_library = target
            .as_ref()
            .zip(canon(&self.library))
            .is_some_and(|(t, l)| t.starts_with(&l));
        let out = plan_restore(&PlanInput {
            manifest: &opened.manifest,
            manifest_sha256_now: &opened.manifest_sha256,
            verification: st.verification.as_ref(),
            target_name: name,
            target_display: format!("{}/{name}", t.display),
            parent_entries: &names,
            volume,
            inside_source,
            inside_library,
        });
        Ok((out, opened))
    }

    pub fn restore_precheck(&self, name: &str) -> Result<Value, DormbitError> {
        let st = self.lock()?;
        let (out, _) = self.precheck(&st, name)?;
        to_json(&out.report)
    }

    pub fn restore(&self, name: &str) -> Result<Value, DormbitError> {
        let mut st = self.lock()?;
        let (out, opened) = self.precheck(&st, name)?;
        let Some(plan) = out.plan else {
            let code = out
                .report
                .first_error()
                .unwrap_or(ErrorCode::RestoreDestExists);
            return Ok(json!({ "refused": true, "code": code, "precheck": out.report }));
        };
        let t = st
            .target
            .as_ref()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Choose where to restore first."))?;
        let ctx = RestoreContext {
            op_id: RandomIds.new_uuid(),
            owner_token: uuid::Uuid::new_v4().simple().to_string(),
            target_grant_id: t.id.clone(),
            target_display: format!("{}/{name}", t.display),
            manifest_git: opened.manifest.git.clone(),
        };
        let mut job = RestoreJob::new(
            plan,
            ctx,
            CapStdRestoreFs,
            FileJournalStore::new(open_dir(&self.journals)?),
            opened,
            SystemClock,
            t.dir()?,
        )?;
        let (events, report) = run_steps(&mut job)?;
        st.restore = Some(report.clone());
        Ok(json!({ "refused": false, "events": events, "report": report }))
    }

    pub fn brief(&self) -> Result<(Value, String), DormbitError> {
        let mut st = self.lock()?;
        let p = st
            .project
            .clone()
            .ok_or_else(|| err(ErrorCode::ScopeDenied, "Pick a project first."))?;
        let e = self.evidence_of(&mut st, &p)?;
        let facts = st
            .restore
            .as_ref()
            .filter(|r| r.status == RestoreStatus::Verified)
            .map(|r| RestoreFacts {
                op_id: r.op_id.clone(),
                capsule_id: r.capsule_id.clone(),
                verified: true,
                files_verified: r.files_verified.files,
                mismatches: r.mismatches.len() as u64,
                not_restored: r.not_restored.len() as u64,
                git_refs_match: match &r.git_check.refs {
                    RefCheck::Match => Some(true),
                    RefCheck::Mismatch { .. } => Some(false),
                    _ => None,
                },
            });
        let today = dormbit_core::util::time::rfc3339(SystemClock.now_unix());
        let b = generate(&e, facts.as_ref(), dormbit_core::VERSION, &today[..10]);
        let md = render_markdown(&b);
        Ok((json!({ "brief": b, "markdown": md }), md))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap() {
                let p = e.unwrap().path();
                let m = std::fs::symlink_metadata(&p).unwrap();
                if m.is_dir() {
                    stack.push(p);
                } else {
                    out.insert(
                        p.strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(&p).unwrap_or_default(),
                    );
                }
            }
        }
        out
    }

    fn s(v: &Value, k: &str) -> String {
        v[k].as_str()
            .unwrap_or_else(|| panic!("{k} missing in {v}"))
            .to_owned()
    }

    #[test]
    fn the_desktop_journey_runs_on_a_real_copy_of_dockline() {
        let tmp = tempfile::tempdir().unwrap();
        let projects = tmp.path().join("Projects");
        let restores = tmp.path().join("Restores");
        std::fs::create_dir_all(projects.join("notes")).unwrap();
        std::fs::write(projects.join("notes/ideas.txt"), "Not a project.\n").unwrap();
        std::fs::create_dir_all(restores.join("dockline-invoices")).unwrap();
        let tar_bytes = std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/dockline/dockline-invoices.tar"),
        )
        .unwrap();
        tar::Archive::new(tar_bytes.as_slice())
            .unpack(&projects)
            .unwrap();
        let source_before = tree(&projects);

        let n = Native::open(&tmp.path().join("AppData"), tmp.path()).unwrap();
        let grant = n.grant_read(&projects).unwrap();
        assert_eq!(s(&grant, "root_display"), "~/Projects");

        let found = n.discover(&s(&grant, "id")).unwrap();
        let list = found["result"]["projects"].as_array().unwrap();
        assert_eq!(list.len(), 1, "only Dockline is a project: {found}");
        let pid = s(&list[0], "id");

        let ev = n.evidence(&pid).unwrap().to_string();
        for want in ["feature/pdf-export", "renderInvoice.ts", "\"modified\":2"] {
            assert!(
                ev.contains(want)
                    || n.lock()
                        .unwrap()
                        .git
                        .as_ref()
                        .is_some_and(|g| g.status.modified.get() == 2),
                "{want}"
            );
        }
        assert_eq!(
            n.lock()
                .unwrap()
                .git
                .as_ref()
                .map(|g| g.head.branch.clone()),
            Some(Some("feature/pdf-export".into()))
        );

        let plan = n.plan_create(&pid).unwrap();
        let plan_id = s(&plan, "plan_id");
        let plan = n
            .plan_set_decision(&plan_id, "data/dev.sqlite", Decision::Exclude)
            .unwrap();
        assert!(
            plan.to_string().contains("sensitive_env"),
            ".env is classed as sensitive"
        );
        n.plan_approve(&plan_id, &s(&plan, "digest"), &[]).unwrap();

        let cap = n.capture(&plan_id).unwrap();
        let id = s(&cap["record"], "capsule_id");
        let file = s(&cap["record"], "file_name");
        assert!(tmp.path().join("AppData/Capsules").join(&file).is_file());
        let v = n.verify(&id).unwrap();
        assert_eq!(s(&v, "status"), "verified");

        let target = n.grant_target(&restores).unwrap();
        assert!(target.to_string().contains("dockline-invoices"));
        let refused = n.restore("dockline-invoices").unwrap();
        assert_eq!(refused["refused"], true);
        assert_eq!(s(&refused, "code"), "RESTORE_DEST_EXISTS");
        assert!(
            tree(&restores.join("dockline-invoices")).is_empty(),
            "the existing folder is untouched"
        );

        let done = n.restore("dockline-restored").unwrap();
        assert_eq!(done["refused"], false);
        assert_eq!(s(&done["report"], "status"), "verified", "{done}");
        let f = "dockline-invoices/src/pdf/renderInvoice.ts";
        assert_eq!(
            std::fs::read(restores.join("dockline-restored/src/pdf/renderInvoice.ts")).unwrap(),
            std::fs::read(projects.join(f)).unwrap()
        );
        assert!(
            !restores.join("dockline-restored/.env").exists(),
            ".env is never captured"
        );

        let (_, md) = n.brief().unwrap();
        for label in [
            "Files restored and verified.",
            "Context reconstructed (inferred from evidence).",
            "AI session resumed: not performed.",
            "Builds/runs: unknown (never executed).",
        ] {
            assert!(md.contains(label), "{label}");
        }
        assert_eq!(tree(&projects), source_before, "the source folder changed");
    }
}
