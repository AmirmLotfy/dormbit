//! The capsule writer (design: Capsule writer). A [`CaptureJob`] runs in budgeted steps:
//! `preparing`, `hashing`, `rechecking`, `assembling`, `committing`, `recording`.
//!
//! The job keeps only per-item state between steps and walks items in a fixed order, so the
//! capsule bytes don't depend on how the work is split into budgets (Requirement 17.6).
//! Disk access goes through the host's [`ReadVfs`] (source) and [`CaptureWorkspace`] (output).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::limits::CapsuleLimits;
use super::manifest::{
    Encoding, Entry, Excluded, LIMITATIONS, MARKER, Manifest, ManifestGit, ManifestProject,
    ManifestWarning, ObjectInfo, SCHEMA_VERSION, Totals, Trailer, to_canonical,
};
use super::ustar::{self, BLOCK, MODE_EXEC, MODE_FILE};
use crate::gitref::read_refs;
use crate::model::{
    ApprovedPlan, CapsuleId, CapturePlan, Decision, DormbitError, ErrorCode, GitTreatment,
    PlanItemKind, ProjectId, ScopeId,
};
use crate::plan::{
    Group, check_well_formed, classify_tree, expand_dir, group_of, plan_digest, sensitive_name,
    sensitive_pem,
};
use crate::util::{hex, sha256_hex, time::rfc3339};
use crate::vfs::{Clock, EntryKind, IdGen, Limited, ReadVfs, RelPath};

/// Gzip level 6, fixed: part of the deterministic capsule form.
const GZIP_LEVEL: u32 = 6;
const KEY_PROBE: u64 = 16 * 1024;

// ---- job plumbing ----------------------------------------------------------------------------

/// How much work one `step` may do. A step always makes progress on at least one item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub max_bytes: u64,
    pub max_ops: u32,
}

impl Budget {
    pub const UNBOUNDED: Budget = Budget {
        max_bytes: u64::MAX,
        max_ops: u32::MAX,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Preparing,
    Hashing,
    Rechecking,
    Assembling,
    Committing,
    Recording,
    Done,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Preparing => "preparing",
            Phase::Hashing => "hashing",
            Phase::Rechecking => "rechecking",
            Phase::Assembling => "assembling",
            Phase::Committing => "committing",
            Phase::Recording => "recording",
            Phase::Done => "done",
        }
    }
}

/// Real counts of work done in the current phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobProgress {
    pub phase: Phase,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

pub enum Step<T> {
    Progress(JobProgress),
    Done(T),
}

pub trait Job {
    type Output;
    fn step(&mut self, budget: Budget) -> Result<Step<Self::Output>, DormbitError>;
    fn cancel(&mut self);
}

/// Runs a job to completion with an unbounded budget (tests and the CLI).
pub fn run_to_end<J: Job>(job: &mut J) -> Result<J::Output, DormbitError> {
    loop {
        if let Step::Done(out) = job.step(Budget::UNBOUNDED)? {
            return Ok(out);
        }
    }
}

// ---- host-side traits ------------------------------------------------------------------------

/// The capsule's staging area in the library (temp folder with an owner token on disk).
pub trait CaptureWorkspace {
    /// Stores one gzip object under its content hash. Returns `true` if it already existed.
    fn put_object(&mut self, content_sha256: &str, stored: &[u8]) -> Result<bool, DormbitError>;
    fn read_object(&self, content_sha256: &str) -> Result<Vec<u8>, DormbitError>;
    /// Appends to the capsule file being assembled.
    fn write_out(&mut self, bytes: &[u8]) -> Result<(), DormbitError>;
    /// Makes the capsule durable and moves it into the library under `final_name`, refusing
    /// an existing name with `CAPSULE_EXISTS`.
    fn commit(&mut self, final_name: &str) -> Result<(), DormbitError>;
    /// Removes the staging area (token-checked on disk).
    fn abort(&mut self) -> Result<(), DormbitError>;
}

/// One registry row per capsule (DORMBIT's own state, not inside a granted folder).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapsuleRecord {
    pub capsule_id: CapsuleId,
    pub project_id: ProjectId,
    pub project_name: String,
    pub library_grant: ScopeId,
    pub file_name: String,
    pub file_sha256: String,
    pub manifest_sha256: String,
    pub plan_digest: String,
    pub size: u64,
    pub file_count: u64,
    pub created_at: i64,
    pub last_verified_at: Option<i64>,
}

pub trait Registry {
    fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError>;
    fn put(&mut self, record: CapsuleRecord) -> Result<(), DormbitError>;
}

pub struct CaptureContext {
    pub app_version: String,
    pub project_id: ProjectId,
    pub project: ManifestProject,
    pub library_grant: ScopeId,
    pub limits: CapsuleLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOutput {
    pub record: CapsuleRecord,
    pub manifest: Manifest,
    /// Paths that appeared after approval and were left out (Requirement 8.3).
    pub not_captured: Vec<String>,
}

// ---- the job ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    Symlink,
}

#[derive(Debug, Clone)]
struct Item {
    path: String,
    kind: Kind,
    size: u64,
    mode: u32,
    mtime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Read1 {
    File { sha: String },
    Link { target: String },
}

#[derive(Debug, Clone)]
struct Obj {
    stored_size: u64,
    stored_sha: String,
    exec: bool,
}

fn changed(path: &str) -> DormbitError {
    DormbitError::new(
        ErrorCode::FileChangedDuringCapture,
        format!("{path} changed while DORMBIT was reading it. Nothing was saved; try again."),
    )
}

/// A read of an approved item failed. A path that turned into a link (or vanished) since
/// `preparing` means the file changed during capture.
fn read_error(e: crate::vfs::VfsError, path: &str) -> DormbitError {
    match e {
        crate::vfs::VfsError::Unsafe | crate::vfs::VfsError::NotFound => changed(path),
        other => other.to_error(path),
    }
}

fn unsafe_path(path: &str) -> DormbitError {
    DormbitError::new(
        ErrorCode::PathUnsafe,
        format!("{path} is a sensitive file and can't go into a capsule."),
    )
}

fn limit(msg: &str) -> DormbitError {
    DormbitError::new(ErrorCode::CapsuleLimitExceeded, msg)
}

fn gzip(data: &[u8]) -> Result<Vec<u8>, DormbitError> {
    let io = |_| DormbitError::new(ErrorCode::IoFailed, "DORMBIT could not compress a file.");
    let mut enc = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), flate2::Compression::new(GZIP_LEVEL));
    enc.write_all(data).map_err(io)?;
    enc.finish().map_err(io)
}

pub struct CaptureJob<'a, V: ReadVfs, W: CaptureWorkspace, R: Registry> {
    vfs: &'a V,
    plan: CapturePlan,
    approved: ApprovedPlan,
    ws: W,
    registry: R,
    ctx: CaptureContext,
    capsule_id: String,
    created_at: i64,

    phase: Phase,
    cancelled: bool,
    failed: Option<DormbitError>,
    cursor: usize,

    items: Vec<Item>,
    first: Vec<Option<Read1>>,
    excluded: Vec<Excluded>,
    not_captured: Vec<String>,
    git: Option<ManifestGit>,
    objects: BTreeMap<String, Obj>,

    manifest: Option<Manifest>,
    manifest_sha: String,
    out_len: u64,
    out_hash: Sha256,
    record: Option<CapsuleRecord>,
}

impl<'a, V: ReadVfs, W: CaptureWorkspace, R: Registry> CaptureJob<'a, V, W, R> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<C: Clock, I: IdGen>(
        vfs: &'a V,
        plan: CapturePlan,
        approved: ApprovedPlan,
        ws: W,
        registry: R,
        clock: &C,
        ids: &I,
        ctx: CaptureContext,
    ) -> Result<Self, DormbitError> {
        ctx.limits.validate()?;
        Ok(Self {
            vfs,
            plan,
            approved,
            ws,
            registry,
            ctx,
            capsule_id: ids.new_uuid(),
            created_at: clock.now_unix().max(0),
            phase: Phase::Preparing,
            cancelled: false,
            failed: None,
            cursor: 0,
            items: Vec::new(),
            first: Vec::new(),
            excluded: Vec::new(),
            not_captured: Vec::new(),
            git: None,
            objects: BTreeMap::new(),
            manifest: None,
            manifest_sha: String::new(),
            out_len: 0,
            out_hash: Sha256::new(),
            record: None,
        })
    }

    pub fn capsule_id(&self) -> &str {
        &self.capsule_id
    }

    pub fn workspace(&self) -> &W {
        &self.ws
    }

    pub fn workspace_mut(&mut self) -> &mut W {
        &mut self.ws
    }

    pub fn registry(&self) -> &R {
        &self.registry
    }

    pub fn into_parts(self) -> (W, R) {
        (self.ws, self.registry)
    }

    // ---- preparing ---------------------------------------------------------------------------

    fn add_approved(&mut self, path: &str, kind: PlanItemKind) -> Result<(), DormbitError> {
        let rel = RelPath::parse(path)?;
        let name = rel.file_name();
        if sensitive_name(name).is_some() {
            return Err(unsafe_path(path));
        }
        let meta = self
            .vfs
            .symlink_metadata(&rel)
            .map_err(|e| e.to_error(path))?;
        let kind = match (kind, meta.kind) {
            (PlanItemKind::File, EntryKind::File) => Kind::File,
            (PlanItemKind::Symlink, EntryKind::Symlink) => Kind::Symlink,
            _ => return Err(changed(path)),
        };
        if kind == Kind::File {
            let head = self
                .vfs
                .read_prefix(&rel, KEY_PROBE)
                .map_err(|e| e.to_error(path))?;
            if sensitive_pem(&head) {
                return Err(unsafe_path(path));
            }
            if meta.len > self.ctx.limits.max_file_size {
                return Err(limit("A file is larger than the per-file capsule limit."));
            }
        }
        self.items.push(Item {
            path: path.to_owned(),
            kind,
            size: if kind == Kind::File { meta.len } else { 0 },
            mode: meta.mode & 0o777,
            mtime: meta.mtime,
        });
        Ok(())
    }

    fn prepare(&mut self) -> Result<(), DormbitError> {
        let digest = plan_digest(&self.plan);
        if digest != self.approved.digest || digest != self.plan.digest {
            return Err(DormbitError::new(
                ErrorCode::PlanDigestMismatch,
                "The plan changed after it was approved. Review and approve it again.",
            ));
        }
        check_well_formed(&self.plan)?;
        let limits = self.ctx.limits.clone();
        let items = self.plan.items.clone();
        for it in &items {
            match it.decision {
                Decision::Undecided => {
                    return Err(DormbitError::new(
                        ErrorCode::PlanIncomplete,
                        "An item still needs a decision.",
                    ));
                }
                Decision::Exclude => self.excluded.push(Excluded {
                    path: it.path.clone(),
                    group: group_of(it).code().to_owned(),
                    reason: it.reason.clone(),
                }),
                Decision::Include => match it.kind {
                    PlanItemKind::File | PlanItemKind::Symlink => {
                        self.add_approved(&it.path, it.kind)?;
                    }
                    PlanItemKind::Dir => {
                        let dir = RelPath::parse(&it.path)?;
                        for c in expand_dir(self.vfs, &dir, &limits)? {
                            let takes = c.reason.editable() || c.reason.group() == Group::Included;
                            let leaf = matches!(c.kind, PlanItemKind::File | PlanItemKind::Symlink);
                            if takes && leaf {
                                self.add_approved(&c.path, c.kind)?;
                            } else {
                                self.excluded.push(Excluded {
                                    path: c.path,
                                    group: c.reason.group().code().to_owned(),
                                    reason: c.reason.as_str().to_owned(),
                                });
                            }
                        }
                    }
                    PlanItemKind::Special => {
                        return Err(DormbitError::new(
                            ErrorCode::PlanItemLocked,
                            format!("{} can't be captured.", it.path),
                        ));
                    }
                },
            }
        }

        // Anything new since the plan was built is left out and reported (Requirement 8.3).
        let known: BTreeSet<&str> = items.iter().map(|i| i.path.as_str()).collect();
        for c in classify_tree(self.vfs, self.plan.git_treatment, &limits)? {
            if !known.contains(c.path.as_str()) {
                self.not_captured.push(c.path.clone());
                self.excluded.push(Excluded {
                    path: c.path,
                    group: "not_captured".to_owned(),
                    reason: "appeared_after_approval".to_owned(),
                });
            }
        }

        self.items
            .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        self.excluded
            .sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
        self.not_captured
            .sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        let entries = self.items.len() as u64;
        let bytes: u64 = self
            .items
            .iter()
            .map(|i| i.size)
            .fold(0, u64::saturating_add);
        if entries > limits.max_entries || bytes > limits.max_total_bytes {
            return Err(limit(
                "The approved files are over the capsule limits. Exclude some items and try again.",
            ));
        }

        let has_git = items
            .iter()
            .any(|i| i.path == ".git" || i.path.starts_with(".git/"));
        if has_git {
            self.git = Some(match self.plan.git_treatment {
                GitTreatment::Full => {
                    let r = read_refs(self.vfs);
                    ManifestGit {
                        treatment: GitTreatment::Full,
                        head: r.head,
                        refs: r.refs,
                    }
                }
                GitTreatment::WorkingTreeOnly => ManifestGit {
                    treatment: GitTreatment::WorkingTreeOnly,
                    head: None,
                    refs: Vec::new(),
                },
            });
        }
        self.first = vec![None; self.items.len()];
        Ok(())
    }

    // ---- hashing and rechecking --------------------------------------------------------------

    fn read_item(&self, i: usize) -> Result<(Read1, Option<Vec<u8>>), DormbitError> {
        let it = &self.items[i];
        let rel = RelPath::parse(&it.path)?;
        match it.kind {
            Kind::File => {
                let bytes = match self
                    .vfs
                    .read_limited(&rel, self.ctx.limits.max_file_size)
                    .map_err(|e| read_error(e, &it.path))?
                {
                    Limited::Complete(b) => b,
                    Limited::TooLarge { .. } => return Err(changed(&it.path)),
                };
                if bytes.len() as u64 != it.size {
                    return Err(changed(&it.path));
                }
                // The bytes that go into the capsule are checked again, so a file turned into
                // a private key after `preparing` can't slip in (Property 7).
                if sensitive_pem(&bytes) {
                    return Err(unsafe_path(&it.path));
                }
                Ok((
                    Read1::File {
                        sha: sha256_hex(&bytes),
                    },
                    Some(bytes),
                ))
            }
            Kind::Symlink => {
                let target = self
                    .vfs
                    .read_link(&rel)
                    .map_err(|e| read_error(e, &it.path))?;
                Ok((Read1::Link { target }, None))
            }
        }
    }

    fn hash_one(&mut self, i: usize) -> Result<u64, DormbitError> {
        let (read, bytes) = self.read_item(i)?;
        let mut n = 0;
        if let (Read1::File { sha }, Some(bytes)) = (&read, bytes) {
            n = bytes.len() as u64;
            if !self.objects.contains_key(sha) {
                let gz = gzip(&bytes)?;
                self.ws.put_object(sha, &gz)?;
                self.objects.insert(
                    sha.clone(),
                    Obj {
                        stored_size: gz.len() as u64,
                        stored_sha: sha256_hex(&gz),
                        exec: false,
                    },
                );
            }
            if self.items[i].mode & 0o111 != 0
                && let Some(o) = self.objects.get_mut(sha)
            {
                o.exec = true;
            }
        }
        self.first[i] = Some(read);
        Ok(n)
    }

    fn recheck_one(&mut self, i: usize) -> Result<u64, DormbitError> {
        let (read, bytes) = self.read_item(i)?;
        if self.first[i].as_ref() != Some(&read) {
            return Err(changed(&self.items[i].path));
        }
        Ok(bytes.map_or(0, |b| b.len() as u64))
    }

    /// Runs `f` over items from the cursor until the budget is spent or the phase is done.
    fn per_item(
        &mut self,
        budget: Budget,
        f: fn(&mut Self, usize) -> Result<u64, DormbitError>,
    ) -> Result<bool, DormbitError> {
        let (mut ops, mut bytes) = (0u32, 0u64);
        while self.cursor < self.items.len() {
            bytes = bytes.saturating_add(f(self, self.cursor)?);
            self.cursor += 1;
            ops = ops.saturating_add(1);
            if ops >= budget.max_ops || bytes >= budget.max_bytes {
                break;
            }
        }
        Ok(self.cursor >= self.items.len())
    }

    // ---- assembling --------------------------------------------------------------------------

    fn out(&mut self, bytes: &[u8]) -> Result<(), DormbitError> {
        self.out_len = self.out_len.saturating_add(bytes.len() as u64);
        if self.out_len > self.ctx.limits.max_capsule_size {
            return Err(limit(
                "The capsule would be larger than the capsule size limit.",
            ));
        }
        self.out_hash.update(bytes);
        self.ws.write_out(bytes)
    }

    fn entry(&mut self, name: &str, data: &[u8], mode: u32) -> Result<(), DormbitError> {
        let mtime = u64::try_from(self.created_at).unwrap_or(0);
        let h = ustar::header(name, data.len() as u64, mode, mtime)?;
        self.out(&h)?;
        self.out(data)?;
        let pad = ustar::padding(data.len() as u64);
        if pad > 0 {
            self.out(&[0u8; BLOCK][..pad])?;
        }
        Ok(())
    }

    fn build_manifest(&self) -> Manifest {
        let mut entries = Vec::with_capacity(self.items.len());
        let (mut files, mut symlinks, mut bytes) = (0u64, 0u64, 0u64);
        for (it, read) in self.items.iter().zip(&self.first) {
            match read {
                Some(Read1::File { sha }) => {
                    let o = self.objects.get(sha);
                    files += 1;
                    bytes = bytes.saturating_add(it.size);
                    entries.push(Entry::File {
                        path: it.path.clone(),
                        size: it.size,
                        mode: it.mode,
                        mtime: it.mtime,
                        sha256: sha.clone(),
                        object: ObjectInfo {
                            encoding: Encoding::Gzip,
                            stored_size: o.map_or(0, |o| o.stored_size),
                            stored_sha256: o.map_or_else(String::new, |o| o.stored_sha.clone()),
                        },
                    });
                }
                Some(Read1::Link { target }) => {
                    symlinks += 1;
                    entries.push(Entry::Symlink {
                        path: it.path.clone(),
                        target: target.clone(),
                        restored: false,
                    });
                }
                None => {}
            }
        }
        let warnings = self
            .plan
            .items
            .iter()
            .filter(|i| i.decision == Decision::Include)
            .filter_map(|i| {
                let w = i.warning.as_deref()?;
                let id = crate::plan::ack_id(&i.path, w);
                let mut parts = w.splitn(2, ':');
                let code = parts.next().unwrap_or(w).to_owned();
                let line = parts.next().and_then(|l| l.parse().ok());
                Some(ManifestWarning {
                    acknowledged: self.approved.acks.contains(&id),
                    id,
                    code,
                    path: i.path.clone(),
                    line,
                })
            })
            .collect();
        Manifest {
            schema_version: SCHEMA_VERSION,
            capsule_id: self.capsule_id.clone(),
            created_at: rfc3339(self.created_at),
            app_version: self.ctx.app_version.clone(),
            project: self.ctx.project.clone(),
            plan_digest: self.approved.digest.clone(),
            git: self.git.clone(),
            entries,
            totals: Totals {
                files,
                symlinks,
                objects: self.objects.len() as u64,
                bytes,
                stored_bytes: self
                    .objects
                    .values()
                    .map(|o| o.stored_size)
                    .fold(0, u64::saturating_add),
                excluded: self.excluded.len() as u64,
            },
            excluded: self.excluded.clone(),
            warnings,
            limitations: LIMITATIONS.iter().map(|s| (*s).to_owned()).collect(),
            agent_refs: Vec::new(),
        }
    }

    /// Cursor 0 writes the marker and manifest; 1..=n one object each; n+1 the trailer.
    fn assemble(&mut self, budget: Budget) -> Result<bool, DormbitError> {
        let names: Vec<String> = self.objects.keys().cloned().collect();
        let (mut ops, mut bytes) = (0u32, 0u64);
        loop {
            if self.cursor == 0 {
                let m = self.build_manifest();
                let mbytes = to_canonical(&m)?;
                if mbytes.len() as u64 > self.ctx.limits.max_manifest_size {
                    return Err(limit("The capsule manifest would be over its size limit."));
                }
                self.manifest_sha = sha256_hex(&mbytes);
                self.entry("DORMBIT", MARKER, MODE_FILE)?;
                self.entry("manifest.json", &mbytes, MODE_FILE)?;
                bytes = bytes.saturating_add(mbytes.len() as u64);
                self.manifest = Some(m);
            } else if self.cursor <= names.len() {
                let sha = &names[self.cursor - 1];
                let (stored_size, stored_sha, exec) = match self.objects.get(sha) {
                    Some(o) => (o.stored_size, o.stored_sha.clone(), o.exec),
                    None => (0, String::new(), false),
                };
                let data = self.ws.read_object(sha)?;
                if data.len() as u64 != stored_size || sha256_hex(&data) != stored_sha {
                    return Err(DormbitError::new(
                        ErrorCode::CapsuleHashMismatch,
                        "A stored file changed in the staging folder before the capsule was assembled.",
                    ));
                }
                let mode = if exec { MODE_EXEC } else { MODE_FILE };
                self.entry(&format!("objects/{sha}"), &data, mode)?;
                bytes = bytes.saturating_add(stored_size);
            } else {
                let trailer = Trailer {
                    manifest_sha256: self.manifest_sha.clone(),
                    object_count: names.len() as u64,
                };
                let t = to_canonical(&trailer)?;
                self.entry("END", &t, MODE_FILE)?;
                self.out(&[0u8; BLOCK * 2])?;
                self.cursor += 1;
                return Ok(true);
            }
            self.cursor += 1;
            ops = ops.saturating_add(1);
            if ops >= budget.max_ops || bytes >= budget.max_bytes {
                return Ok(false);
            }
        }
    }

    fn progress(&self) -> JobProgress {
        let file_bytes: u64 = self
            .items
            .iter()
            .map(|i| i.size)
            .fold(0, u64::saturating_add);
        let (done, total, bdone, btotal) = match self.phase {
            Phase::Hashing | Phase::Rechecking => {
                let bdone = self.items[..self.cursor.min(self.items.len())]
                    .iter()
                    .map(|i| i.size)
                    .fold(0, u64::saturating_add);
                (
                    self.cursor as u64,
                    self.items.len() as u64,
                    bdone,
                    file_bytes,
                )
            }
            Phase::Assembling => {
                let total = self.objects.len() as u64 + 2;
                (self.cursor as u64, total, self.out_len, self.out_len)
            }
            _ => (0, self.items.len() as u64, 0, file_bytes),
        };
        JobProgress {
            phase: self.phase,
            files_done: done,
            files_total: total,
            bytes_done: bdone,
            bytes_total: btotal,
        }
    }

    fn advance(&mut self, next: Phase) {
        self.phase = next;
        self.cursor = 0;
    }

    fn run(&mut self, budget: Budget) -> Result<Step<CaptureOutput>, DormbitError> {
        match self.phase {
            Phase::Preparing => {
                self.prepare()?;
                self.advance(Phase::Hashing);
            }
            Phase::Hashing => {
                if self.per_item(budget, Self::hash_one)? {
                    self.advance(Phase::Rechecking);
                }
            }
            Phase::Rechecking => {
                if self.per_item(budget, Self::recheck_one)? {
                    self.advance(Phase::Assembling);
                }
            }
            Phase::Assembling => {
                if self.assemble(budget)? {
                    self.advance(Phase::Committing);
                }
            }
            Phase::Committing => {
                self.ws.commit(&format!("{}.dormbit", self.capsule_id))?;
                let m = self.manifest.as_ref();
                self.record = Some(CapsuleRecord {
                    capsule_id: CapsuleId(self.capsule_id.clone()),
                    project_id: self.ctx.project_id.clone(),
                    project_name: self.ctx.project.name.clone(),
                    library_grant: self.ctx.library_grant.clone(),
                    file_name: format!("{}.dormbit", self.capsule_id),
                    file_sha256: hex(&self.out_hash.clone().finalize()),
                    manifest_sha256: self.manifest_sha.clone(),
                    plan_digest: self.approved.digest.clone(),
                    size: self.out_len,
                    file_count: m.map_or(0, |m| m.totals.files),
                    created_at: self.created_at,
                    last_verified_at: None,
                });
                self.advance(Phase::Recording);
            }
            Phase::Recording => {
                let record = self.record.clone().ok_or_else(|| {
                    DormbitError::new(ErrorCode::IoFailed, "The capsule record is missing.")
                })?;
                self.registry.put(record.clone()).map_err(|_| {
                    DormbitError::new(
                        ErrorCode::IoFailed,
                        "The capsule was saved, but DORMBIT could not add it to its list.",
                    )
                })?;
                self.advance(Phase::Done);
                let manifest = self.manifest.clone().ok_or_else(|| {
                    DormbitError::new(ErrorCode::IoFailed, "The capsule manifest is missing.")
                })?;
                return Ok(Step::Done(CaptureOutput {
                    record,
                    manifest,
                    not_captured: self.not_captured.clone(),
                }));
            }
            Phase::Done => {
                return Err(DormbitError::new(
                    ErrorCode::Cancelled,
                    "This capture has already finished.",
                ));
            }
        }
        Ok(Step::Progress(self.progress()))
    }
}

impl<V: ReadVfs, W: CaptureWorkspace, R: Registry> Job for CaptureJob<'_, V, W, R> {
    type Output = CaptureOutput;

    fn step(&mut self, budget: Budget) -> Result<Step<CaptureOutput>, DormbitError> {
        if let Some(e) = &self.failed {
            return Err(e.clone());
        }
        if self.cancelled && self.phase < Phase::Committing {
            let _ = self.ws.abort();
            let e = DormbitError::new(
                ErrorCode::Cancelled,
                "Capture cancelled. Nothing was saved.",
            );
            self.failed = Some(e.clone());
            return Err(e);
        }
        match self.run(budget) {
            Ok(s) => Ok(s),
            Err(e) => {
                // A failed commit leaves nothing in the library, so staging is cleaned up too.
                // From `recording` on, the capsule stays in place (Requirement 17.4).
                if self.phase <= Phase::Committing {
                    let _ = self.ws.abort();
                }
                self.failed = Some(e.clone());
                Err(e)
            }
        }
    }

    /// Before `committing`, the next step aborts and returns `CANCELLED`. From `committing`
    /// on, the capsule is finished and recorded (Requirements 17.3, 17.4).
    fn cancel(&mut self) {
        self.cancelled = true;
    }
}
