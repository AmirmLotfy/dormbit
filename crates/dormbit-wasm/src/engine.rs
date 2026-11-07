//! The demo engine: the real `dormbit-core` over an in-memory virtual home folder.
//!
//! Nothing here touches a real disk or the network. The fixture tar becomes
//! `~/Projects/dockline-invoices`; restores go to a virtual `~/Restores` that already holds
//! a `dockline-invoices` folder, so the first restore attempt shows the refusal. Every
//! command returns JSON shaped like the desktop app's results.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use dormbit_core::brief::{RestoreFacts, generate, render_markdown};
use dormbit_core::capsule::manifest::{Entry, Manifest, ManifestProject};
use dormbit_core::capsule::verify::{LayoutEntry, layout};
use dormbit_core::capsule::{
    Budget, BytesSource, CapsuleLimits, CapsuleRecord, CaptureContext, CaptureJob, Job,
    JobProgress, MemRegistry, MemWorkspace, Report, ReportStatus, Step, verify_all,
};
use dormbit_core::discovery::{DiscoveredProject, DiscoveryLimits, discover, project_view};
use dormbit_core::evidence::{AgentSession, EvidenceInput, EvidenceResult, evaluate};
use dormbit_core::model::{
    ApprovedPlan, CapturePlan, Decision, DormbitError, ErrorCode, GitSnapshot, GitTreatment,
    ProjectId, ScopeId,
};
use dormbit_core::plan::{apply_edit, approve, build_plan};
use dormbit_core::restore::fs::{MemNode, MemRestoreFs, RestoreFs};
use dormbit_core::restore::report::RefCheck;
use dormbit_core::restore::{
    CaseSensitivity, MemJournal, OpenedCapsule, PlanInput, RestoreContext, RestoreJob,
    RestoreReport, RestoreStatus, VolumeFacts, plan_restore,
};
use dormbit_core::util::{sha256_hex, time::parse_rfc3339};
use dormbit_core::vfs::{CountingIds, FixedClock, IdGen, MemVfs};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PROJECTS: &str = "Projects";
const FIXTURE_ROOT: &str = "dockline-invoices";
const SCOPE_ID: &str = "demo-projects";
const LIBRARY_ID: &str = "demo-library";
const RESTORES_DISPLAY: &str = "~/Restores";
const CAPACITY: u64 = 256 << 20;
/// Each step job runs in small budgets so the UI can show real phase-by-phase progress.
const STEP: Budget = Budget {
    max_bytes: 16 << 10,
    max_ops: 8,
};

fn err(code: ErrorCode, msg: &str) -> DormbitError {
    DormbitError::new(code, msg)
}

fn bad_args(e: serde_json::Error) -> DormbitError {
    err(ErrorCode::IoFailed, &format!("Invalid arguments: {e}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByteChange {
    pub capsule_offset: u64,
    pub region: String,
    pub old: u8,
    pub new: u8,
}

/// Loads the fixture tar (entries under `dockline-invoices/`) into `home` under `Projects/`.
/// Loads the fixture tar (entries under `dockline-invoices/`) into `home` under `Projects/`.
/// The fixture is ours, but it is still treated as input: unsafe paths, duplicates, unknown
/// entry types, and unsafe link targets are refused, and every file must match its SHA-256
/// in the Sidecar's `expected_hashes` (and every expected file must be present).
fn load_fixture(
    home: &mut MemVfs,
    tar_bytes: &[u8],
    expected: &serde_json::Map<String, Value>,
) -> Result<u64, DormbitError> {
    let bad = |m: &str| {
        err(
            ErrorCode::CapsuleMalformed,
            &format!("The demo dataset is damaged: {m}."),
        )
    };
    let unsafe_path = |p: &str| {
        p.starts_with('/')
            || p.split('/')
                .any(|c| c == ".." || c == "." || c.is_empty() || c.contains('\0'))
    };
    let mut archive = tar::Archive::new(tar_bytes);
    let mut files = 0;
    let mut seen = std::collections::BTreeSet::new();
    let prefix = format!("{FIXTURE_ROOT}/");
    for entry in archive.entries().map_err(|_| bad("not a tar"))? {
        let mut entry = entry.map_err(|_| bad("bad entry"))?;
        let name = entry
            .path()
            .map_err(|_| bad("bad name"))?
            .to_string_lossy()
            .into_owned();
        let Some(rel) = name.strip_prefix(&prefix) else {
            continue;
        };
        let rel = rel.trim_end_matches('/');
        if rel.is_empty() {
            continue;
        }
        if unsafe_path(rel) {
            return Err(bad("unsafe path"));
        }
        if !seen.insert(rel.to_owned()) {
            return Err(bad("duplicate entry"));
        }
        let path = format!("{PROJECTS}/{FIXTURE_ROOT}/{rel}");
        let kind = entry.header().entry_type();
        if kind.is_dir() {
            home.add_dir(&path);
        } else if kind.is_file() {
            let mode = entry.header().mode().unwrap_or(0o644);
            let mtime = i64::try_from(entry.header().mtime().unwrap_or(0)).unwrap_or(0);
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|_| bad("unreadable file"))?;
            let want = expected.get(&name).and_then(Value::as_str);
            if want != Some(sha256_hex(&data).as_str()) {
                return Err(bad("a file doesn't match its expected hash"));
            }
            home.add_file_with(&path, data, mode, mtime);
            files += 1;
        } else if kind.is_symlink() {
            let target = entry
                .link_name()
                .ok()
                .flatten()
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            if target.is_empty() || unsafe_path(&target) {
                return Err(bad("unsafe link target"));
            }
            home.add_symlink(&path, &target);
        } else {
            return Err(bad("unsupported entry type"));
        }
    }
    let missing = expected
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .any(|k| !seen.contains(k.trim_start_matches(&prefix)));
    if files == 0 || missing {
        return Err(bad("files are missing"));
    }
    Ok(files)
}

#[derive(Default, Clone)]
struct State {
    projects: Vec<DiscoveredProject>,
    project: Option<DiscoveredProject>,
    plan: Option<CapturePlan>,
    approved: Option<ApprovedPlan>,
    record: Option<CapsuleRecord>,
    manifest: Option<Manifest>,
    verification: Option<Report>,
    restore: Option<RestoreReport>,
    restored_root: Option<String>,
    pending_fault: Option<(u64, u8)>,
}

pub struct Engine {
    home: MemVfs,
    pristine_home: MemVfs,
    restores: MemRestoreFs,
    library: BTreeMap<String, Vec<u8>>,
    registry: MemRegistry,
    git: GitSnapshot,
    sessions: Vec<AgentSession>,
    now: i64,
    ids: CountingIds,
    st: State,
}

fn fresh_restores() -> MemRestoreFs {
    let mut fs = MemRestoreFs::new(true);
    // An existing folder, so restoring under the original name is refused (RST-1).
    fs.add("dockline-invoices", MemNode::Dir);
    fs.add(
        "dockline-invoices/README.md",
        MemNode::File {
            data: b"# A folder that was already here\n".to_vec(),
            mode: 0o644,
        },
    );
    fs
}

impl Engine {
    pub fn new(fixture_tar: &[u8], sidecar_json: &str) -> Result<Self, DormbitError> {
        let side: Value = serde_json::from_str(sidecar_json).map_err(|_| {
            err(
                ErrorCode::CapsuleMalformed,
                "The demo dataset's Sidecar is not valid JSON.",
            )
        })?;
        let want = side
            .get("fixture_sha256")
            .and_then(Value::as_str)
            .unwrap_or("");
        // Required: a dataset without its expected hash is refused, not trusted.
        if want.is_empty() || sha256_hex(fixture_tar) != want {
            return Err(err(
                ErrorCode::CapsuleHashMismatch,
                "The demo dataset doesn't match its expected SHA-256.",
            ));
        }
        let git: GitSnapshot =
            serde_json::from_value(side["git_snapshot"].clone()).map_err(|_| {
                err(
                    ErrorCode::CapsuleMalformed,
                    "The Sidecar's Git snapshot is not valid.",
                )
            })?;
        let now = side
            .get("demo_clock")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339)
            .ok_or_else(|| {
                err(
                    ErrorCode::CapsuleMalformed,
                    "The Sidecar's demo clock is not valid.",
                )
            })?;
        let mut sessions = Vec::new();
        if let Some(meta) = side.get("agent_metadata").and_then(Value::as_object) {
            for (adapter, list) in meta {
                for s in list.as_array().into_iter().flatten() {
                    sessions.push(AgentSession {
                        adapter: adapter.clone(),
                        session_id: s["session_id"].as_str().unwrap_or("").to_owned(),
                        ended_at: s["ended_at"].as_str().map(str::to_owned),
                        git_branch: s["git_branch"].as_str().map(str::to_owned),
                    });
                }
            }
        }
        let mut home = MemVfs::new();
        let expected = side
            .get("expected_hashes")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                err(
                    ErrorCode::CapsuleMalformed,
                    "The Sidecar has no expected hashes.",
                )
            })?;
        load_fixture(&mut home, fixture_tar, expected)?;
        home.add_file(
            &format!("{PROJECTS}/notes/ideas.txt"),
            "Not a project: no manifest, no Git.\n",
        );
        Ok(Self {
            pristine_home: home.clone(),
            home,
            restores: fresh_restores(),
            library: BTreeMap::new(),
            registry: MemRegistry::default(),
            git,
            sessions,
            now,
            ids: CountingIds::default(),
            st: State::default(),
        })
    }

    /// Back to the state right after loading.
    pub fn reset(&mut self) {
        self.home = self.pristine_home.clone();
        self.restores = fresh_restores();
        self.library.clear();
        self.registry = MemRegistry::default();
        self.ids = CountingIds::default();
        self.st = State::default();
    }

    /// SHA-256 over everything mutable (for the Reset property).
    pub fn state_digest(&self) -> String {
        let v = json!({
            "home": format!("{:?}", self.home),
            "restores": format!("{:?}", self.restores.nodes),
            "library": self.library.iter().map(|(k, v)| (k.clone(), sha256_hex(v))).collect::<BTreeMap<_, _>>(),
            "registry": self.registry.records.keys().collect::<Vec<_>>(),
            "plan": self.st.plan.as_ref().map(|p| p.digest.clone()),
            "approved": self.st.approved.is_some(),
            "restore": self.st.restore.is_some(),
            "pending": self.st.pending_fault,
        });
        sha256_hex(v.to_string().as_bytes())
    }

    fn project(&self) -> Result<DiscoveredProject, DormbitError> {
        self.st.project.clone().ok_or_else(|| {
            err(
                ErrorCode::ScopeDenied,
                "Run discovery and pick a project first.",
            )
        })
    }

    fn rel_root(p: &DiscoveredProject) -> String {
        format!("{PROJECTS}/{}", p.rel_root)
            .trim_end_matches('/')
            .to_owned()
    }

    fn evidence(&self) -> Result<EvidenceResult, DormbitError> {
        let p = self.project()?;
        let view = project_view(&self.home, &Self::rel_root(&p));
        Ok(evaluate(&EvidenceInput {
            vfs: &view,
            folder_name: p.rel_root.rsplit('/').next().unwrap_or("project"),
            git: Some(&self.git),
            agent_sessions: &self.sessions,
        }))
    }

    fn capsule_bytes_ref(&self) -> Result<&Vec<u8>, DormbitError> {
        let r = self
            .st
            .record
            .as_ref()
            .ok_or_else(|| err(ErrorCode::CapsuleMalformed, "Make a capsule first."))?;
        self.library.get(&r.file_name).ok_or_else(|| {
            err(
                ErrorCode::CapsuleMalformed,
                "The capsule is missing from the library.",
            )
        })
    }

    pub fn capsule_bytes(&self) -> Result<Vec<u8>, DormbitError> {
        self.capsule_bytes_ref().cloned()
    }

    fn volume(&self) -> VolumeFacts {
        let used: u64 = self
            .restores
            .nodes
            .values()
            .map(|(n, _)| match n {
                MemNode::File { data, .. } => data.len() as u64,
                _ => 0,
            })
            .sum();
        VolumeFacts {
            fs_type: "mem".into(),
            block_size: 4096,
            free_bytes: CAPACITY.saturating_sub(used),
            case: CaseSensitivity::Insensitive,
        }
    }

    fn precheck(
        &self,
        target: &str,
        bytes: &[u8],
    ) -> Result<dormbit_core::restore::PrecheckOutcome, DormbitError> {
        let opened = OpenedCapsule::open(Cursor::new(bytes.to_vec()), 1 << 26)?;
        let names: Vec<String> = self
            .restores
            .list(&self.restores.root())
            .map_err(|f| f.to_error("The destination folder"))?
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        Ok(plan_restore(&PlanInput {
            manifest: &opened.manifest,
            manifest_sha256_now: &opened.manifest_sha256,
            verification: self.st.verification.as_ref(),
            target_name: target,
            target_display: format!("{RESTORES_DISPLAY}/{target}"),
            parent_entries: &names,
            volume: self.volume(),
            inside_source: Some(false),
            inside_library: false,
        }))
    }

    /// One command. Arguments and results are JSON.
    pub fn call(&mut self, command: &str, args: &Value) -> Result<Value, DormbitError> {
        let arg = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        match command {
            "app_info" => {
                Ok(json!({ "version": dormbit_core::VERSION, "build": "wasm", "synthetic": true }))
            }

            "discover" => {
                let view = project_view(&self.home, PROJECTS);
                let out = discover(&view, SCOPE_ID, self.now, &DiscoveryLimits::default());
                self.st.projects = out.projects.clone();
                if let Some(p) = out.projects.iter().find(|p| p.rel_root == FIXTURE_ROOT) {
                    self.st.project = Some(p.clone());
                }
                Ok(json!({ "root_display": "~/Projects", "result": out }))
            }

            "evidence" => Ok(serde_json::to_value(self.evidence()?).map_err(bad_args)?),

            "plan_create" => {
                let p = self.project()?;
                let treatment = if arg("git_treatment") == "working_tree_only" {
                    GitTreatment::WorkingTreeOnly
                } else {
                    GitTreatment::Full
                };
                let view = project_view(&self.home, &Self::rel_root(&p));
                let plan = build_plan(&view, treatment, &CapsuleLimits::default(), &self.ids)?;
                self.st.plan = Some(plan.clone());
                self.st.approved = None;
                Ok(serde_json::to_value(plan).map_err(bad_args)?)
            }

            "plan_set_decision" => {
                let decision: Decision =
                    serde_json::from_value(args["decision"].clone()).map_err(bad_args)?;
                let plan = self
                    .st
                    .plan
                    .as_mut()
                    .ok_or_else(|| err(ErrorCode::PlanIncomplete, "Create a plan first."))?;
                apply_edit(plan, &arg("path"), decision)?;
                self.st.approved = None;
                Ok(serde_json::to_value(plan.clone()).map_err(bad_args)?)
            }

            "plan_approve" => {
                let acks: Vec<String> =
                    serde_json::from_value(args["acks"].clone()).unwrap_or_default();
                let plan = self
                    .st
                    .plan
                    .as_ref()
                    .ok_or_else(|| err(ErrorCode::PlanIncomplete, "Create a plan first."))?;
                let ok = approve(
                    plan,
                    &arg("digest"),
                    &acks,
                    &FixedClock(self.now),
                    &CapsuleLimits::default(),
                )?;
                self.st.approved = Some(ok.clone());
                Ok(serde_json::to_value(ok).map_err(bad_args)?)
            }

            "capture" => {
                let p = self.project()?;
                let plan = self
                    .st
                    .plan
                    .clone()
                    .ok_or_else(|| err(ErrorCode::PlanIncomplete, "Create a plan first."))?;
                let ok = self
                    .st
                    .approved
                    .clone()
                    .ok_or_else(|| err(ErrorCode::PlanIncomplete, "Approve the plan first."))?;
                let view = project_view(&self.home, &Self::rel_root(&p));
                let ws = MemWorkspace::with_library(self.library.clone());
                let ctx = CaptureContext {
                    app_version: dormbit_core::VERSION.into(),
                    project_id: ProjectId(p.id.clone()),
                    project: ManifestProject {
                        name: p.name.clone(),
                        name_source: p.name_source.clone(),
                    },
                    library_grant: ScopeId::from(LIBRARY_ID),
                    limits: CapsuleLimits::default(),
                };
                let mut job = CaptureJob::new(
                    &view,
                    plan,
                    ok,
                    ws,
                    self.registry.clone(),
                    &FixedClock(self.now),
                    &self.ids,
                    ctx,
                )?;
                let (events, out) = run_steps(&mut job)?;
                let (ws, reg) = job.into_parts();
                self.library = ws.library;
                self.registry = reg;
                self.st.record = Some(out.record.clone());
                self.st.pending_fault = None;
                self.st.restored_root = None;
                self.st.manifest = Some(out.manifest.clone());
                self.st.verification = None;
                self.st.restore = None;
                Ok(
                    json!({ "events": events, "record": out.record, "manifest": out.manifest, "not_captured": out.not_captured }),
                )
            }

            "verify" => {
                let bytes = self.capsule_bytes()?;
                let report = verify_all(
                    BytesSource::new(bytes),
                    self.registry.clone(),
                    CapsuleLimits::default(),
                )?;
                self.st.verification = Some(report.clone());
                Ok(serde_json::to_value(report).map_err(bad_args)?)
            }

            "destination" => {
                let names: Vec<Value> = self
                    .restores
                    .list(&self.restores.root())
                    .map_err(|f| f.to_error("The destination folder"))?
                    .into_iter()
                    .map(|(n, m)| json!({ "name": n, "kind": format!("{:?}", m.kind) }))
                    .collect();
                Ok(json!({ "root_display": RESTORES_DISPLAY, "entries": names }))
            }

            "restore_precheck" => {
                let bytes = self.capsule_bytes()?;
                let out = self.precheck(&arg("target_name"), &bytes)?;
                Ok(serde_json::to_value(out.report).map_err(bad_args)?)
            }

            "restore" => {
                let target = arg("target_name");
                let mut bytes = self.capsule_bytes()?;
                let out = self.precheck(&target, &bytes)?;
                let Some(plan) = out.plan else {
                    let code = out
                        .report
                        .first_error()
                        .unwrap_or(ErrorCode::RestoreDestExists);
                    return Ok(json!({ "refused": true, "code": code, "precheck": out.report }));
                };
                // "Damage after verification": the byte changes on the virtual disk now,
                // after pre-checks passed and before the executor reads that object.
                // The change is written to the capsule in the virtual library, so it stays
                // damaged afterwards (a later verify reports it).
                if let Some((offset, xor)) = self.st.pending_fault.take() {
                    let idx = usize::try_from(offset).unwrap_or(usize::MAX);
                    if let Some(b) = bytes.get_mut(idx) {
                        *b ^= xor;
                    }
                    let name = self
                        .st
                        .record
                        .as_ref()
                        .map(|r| r.file_name.clone())
                        .unwrap_or_default();
                    if let Some(b) = self.library.get_mut(&name).and_then(|lib| lib.get_mut(idx)) {
                        *b ^= xor;
                    }
                }
                let objects = OpenedCapsule::open(Cursor::new(bytes), 1 << 26)?;
                let fs = std::mem::take(&mut self.restores);
                let root = fs.root();
                let op_id = self.ids.new_uuid();
                let mut job = RestoreJob::new(
                    plan,
                    RestoreContext {
                        op_id,
                        owner_token: sha256_hex(target.as_bytes())[..32].to_owned(),
                        target_grant_id: "demo-restores".into(),
                        target_display: format!("{RESTORES_DISPLAY}/{target}"),
                        manifest_git: self.st.manifest.as_ref().and_then(|m| m.git.clone()),
                    },
                    fs,
                    MemJournal::default(),
                    objects,
                    FixedClock(self.now),
                    root,
                )?;
                let (events, report) = run_steps(&mut job)?;
                let (fs, _) = job.into_parts();
                self.restores = fs;
                if report.status == RestoreStatus::Verified {
                    self.st.restored_root = Some(target.clone());
                }
                self.st.restore = Some(report.clone());
                Ok(json!({ "refused": false, "events": events, "report": report }))
            }

            "restored_files" => {
                let root = self
                    .st
                    .restored_root
                    .clone()
                    .ok_or_else(|| err(ErrorCode::IoFailed, "Restore the capsule first."))?;
                let m = self
                    .st
                    .manifest
                    .as_ref()
                    .ok_or_else(|| err(ErrorCode::IoFailed, "No capsule manifest."))?;
                let files: Vec<Value> = m
                    .entries
                    .iter()
                    .filter_map(|e| match e {
                        Entry::File {
                            path, sha256, size, ..
                        } => Some((path, sha256, size)),
                        Entry::Symlink { .. } => None,
                    })
                    // Every manifest file is listed, with whether it exists in the restored
                    // folder, so a missing file can't shrink the independent check.
                    .map(|(p, s, z)| {
                        let present = self.restores.nodes.contains_key(&format!("{root}/{p}"));
                        json!({ "path": p, "expected_sha256": s, "size": z, "present": present })
                    })
                    .collect();
                Ok(json!({ "root_display": format!("{RESTORES_DISPLAY}/{root}"), "files": files }))
            }

            "brief" => {
                let e = self.evidence()?;
                let facts = self
                    .st
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
                let date = dormbit_core::util::time::rfc3339(self.now)[..10].to_owned();
                let b = generate(&e, facts.as_ref(), dormbit_core::VERSION, &date);
                let md = render_markdown(&b);
                Ok(json!({ "brief": b, "markdown": md }))
            }

            "corrupt" => {
                let control = arg("control");
                let seed = args.get("seed").and_then(Value::as_u64).unwrap_or(0);
                let xor = u8::try_from(args.get("xor").and_then(Value::as_u64).unwrap_or(1))
                    .unwrap_or(1)
                    .max(1);
                self.corrupt(&control, &arg("path"), seed, xor)
                    .map(|c| serde_json::to_value(c).unwrap_or(Value::Null))
            }

            "state" => Ok(json!({
                "has_projects": !self.st.projects.is_empty(),
                "has_plan": self.st.plan.is_some(),
                "approved": self.st.approved.is_some(),
                "has_capsule": self.st.record.is_some(),
                "verified": self.st.verification.as_ref().map(|r| r.status == ReportStatus::Verified),
                "restored": self.st.restored_root.is_some(),
                "digest": self.state_digest(),
            })),

            other => Err(err(
                ErrorCode::ScopeDenied,
                &format!("Unknown command: {other}"),
            )),
        }
    }

    fn corrupt(
        &mut self,
        control: &str,
        path: &str,
        seed: u64,
        xor: u8,
    ) -> Result<ByteChange, DormbitError> {
        let bytes = self.capsule_bytes()?;
        let lay = layout(&bytes);
        let m = self
            .st
            .manifest
            .clone()
            .ok_or_else(|| err(ErrorCode::CapsuleMalformed, "Make a capsule first."))?;
        let object_of = |p: &str| -> Option<&LayoutEntry> {
            let sha = m.entries.iter().find_map(|e| match e {
                Entry::File { path, sha256, .. } if path == p => Some(sha256.clone()),
                _ => None,
            })?;
            lay.iter().find(|l| l.name == format!("objects/{sha}"))
        };
        let (offset, region) = match control {
            "stored_file" | "after_verification" => {
                let target = if path.is_empty() {
                    "src/pdf/renderInvoice.ts"
                } else {
                    path
                };
                let o = object_of(target)
                    .ok_or_else(|| err(ErrorCode::PathUnsafe, "That file isn't in the capsule."))?;
                (
                    o.data_at + seed % o.data_len.max(1),
                    format!("object:{target}"),
                )
            }
            "capsule_bytes" => {
                // Any byte outside object data: marker, headers, manifest, padding, trailer.
                let in_object = |off: u64| {
                    lay.iter().any(|l| {
                        l.name.starts_with("objects/")
                            && off >= l.data_at
                            && off < l.data_at + l.data_len
                    })
                };
                let candidates = (bytes.len() as u64).saturating_sub(
                    lay.iter()
                        .filter(|l| l.name.starts_with("objects/"))
                        .map(|l| l.data_len)
                        .sum::<u64>(),
                );
                let mut k = seed % candidates.max(1);
                let mut off = 0u64;
                for i in 0..bytes.len() as u64 {
                    if !in_object(i) {
                        if k == 0 {
                            off = i;
                            break;
                        }
                        k -= 1;
                    }
                }
                let region = lay
                    .iter()
                    .find(|l| off >= l.header_at && off < l.data_at)
                    .map(|l| format!("header:{}", l.name))
                    .or_else(|| {
                        lay.iter()
                            .find(|l| off >= l.data_at && off < l.data_at + l.data_len)
                            .map(|l| format!("data:{}", l.name))
                    })
                    .unwrap_or_else(|| "padding_or_end".into());
                (off, region)
            }
            _ => return Err(err(ErrorCode::ScopeDenied, "Unknown corruption control.")),
        };
        let idx = usize::try_from(offset).map_err(|_| err(ErrorCode::IoFailed, "offset"))?;
        let old = *bytes
            .get(idx)
            .ok_or_else(|| err(ErrorCode::IoFailed, "offset out of range"))?;
        let change = ByteChange {
            capsule_offset: offset,
            region,
            old,
            new: old ^ xor,
        };
        if control == "after_verification" {
            self.st.pending_fault = Some((offset, xor));
        } else {
            let name = self
                .st
                .record
                .as_ref()
                .map(|r| r.file_name.clone())
                .unwrap_or_default();
            if let Some(b) = self.library.get_mut(&name).and_then(|b| b.get_mut(idx)) {
                *b ^= xor;
            }
            self.st.verification = None;
        }
        Ok(change)
    }

    pub fn restored_file_bytes(&self, path: &str) -> Result<Vec<u8>, DormbitError> {
        let root = self
            .st
            .restored_root
            .clone()
            .ok_or_else(|| err(ErrorCode::IoFailed, "Restore the capsule first."))?;
        match self.restores.nodes.get(&format!("{root}/{path}")) {
            Some((MemNode::File { data, .. }, _)) => Ok(data.clone()),
            _ => Err(err(
                ErrorCode::PathUnsafe,
                "That file isn't in the restored folder.",
            )),
        }
    }
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
