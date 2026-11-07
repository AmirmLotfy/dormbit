//! The Restore_Executor (design: Restore_Executor, Restore sequence).
//!
//! Phases: Stage → Write → Verify staged → Commit (one exclusive rename) → Clean →
//! Post-verify. The journal records each state before the next step depends on it. Any
//! failure or cancel before the rename rolls back: the owner token is checked and only the
//! staging folder is removed; the target is never created. From the rename on, the restore
//! can't be cancelled and always finishes with a report.

use std::collections::{BTreeMap, BTreeSet};

use super::fs::{EntryMeta, MetaKind, OWNER_FILE, OwnerCheckFailure, RestoreFs, check_owner_token};
use super::journal::{
    DevIno, JOURNAL_SCHEMA, Journal, JournalStore, StateStamp, WriteOp, WriteRecord, WriteRoot,
};
use super::keys::{KeyMode, STAGING_PREFIX, SafeName, fold_key, internal_name, is_op_id, nfc_key};
use super::objects::CapsuleObjects;
use super::planner::{CaseSensitivity, RestorePlan};
use super::report::{
    GitCheck, Mismatch, REPORT_SCHEMA, RESTORE_LIMITS, RefCheck, RestoreReport, RestoreStatus,
    SourceCheck, StrippedBits, Totals, WorktreeCheck,
};
use crate::capsule::manifest::ManifestGit;
use crate::capsule::{Budget, Job, JobProgress, Phase, Step};
use crate::gitref::read_refs;
use crate::model::{DormbitError, ErrorCode, GitTreatment, RestoreState};
use crate::util::{sha256_hex, time::rfc3339};
use crate::vfs::{Clock, DirEntry, EntryKind, Limited, Metadata, ReadVfs, RelPath, VfsError};

const TREE: &str = "tree";

pub struct RestoreContext {
    pub op_id: String,
    pub owner_token: String,
    pub target_grant_id: String,
    pub target_display: String,
    pub manifest_git: Option<ManifestGit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelResult {
    Accepted,
    TooLate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RPhase {
    Stage,
    Write,
    Verify,
    Commit,
    Clean,
    PostVerify,
    Done,
}

pub struct RestoreJob<F: RestoreFs, J: JournalStore, O: CapsuleObjects, C: Clock> {
    plan: RestorePlan,
    ctx: RestoreContext,
    fs: F,
    journal_store: J,
    objects: O,
    clock: C,
    parent: F::Dir,
    phase: RPhase,
    cursor: usize,
    cancelled: bool,
    did_commit: bool,
    staging_name: SafeName,
    staging: Option<F::Dir>,
    dirs: BTreeMap<String, F::Dir>,
    journal: Option<Journal>,
    seq: u64,
    started_at: String,
    written: Totals,
    verified: Totals,
    warnings: Vec<String>,
    report: Option<RestoreReport>,
}

fn path_of(names: &[SafeName]) -> String {
    names
        .iter()
        .map(SafeName::as_str)
        .collect::<Vec<_>>()
        .join("/")
}

fn io_err(msg: &str) -> DormbitError {
    DormbitError::new(ErrorCode::IoFailed, msg)
}

impl<F: RestoreFs, J: JournalStore, O: CapsuleObjects, C: Clock> RestoreJob<F, J, O, C> {
    pub fn new(
        plan: RestorePlan,
        ctx: RestoreContext,
        fs: F,
        journal_store: J,
        objects: O,
        clock: C,
        parent: F::Dir,
    ) -> Result<Self, DormbitError> {
        if !is_op_id(&ctx.op_id) || !is_op_id(&ctx.owner_token) {
            return Err(DormbitError::new(
                ErrorCode::PathUnsafe,
                "The restore ID is not valid.",
            ));
        }
        let staging_name = internal_name(&format!("{STAGING_PREFIX}{}", ctx.op_id));
        let started_at = rfc3339(clock.now_unix());
        Ok(Self {
            plan,
            ctx,
            fs,
            journal_store,
            objects,
            clock,
            parent,
            phase: RPhase::Stage,
            cursor: 0,
            cancelled: false,
            did_commit: false,
            staging_name,
            staging: None,
            dirs: BTreeMap::new(),
            journal: None,
            seq: 0,
            started_at,
            written: Totals::default(),
            verified: Totals::default(),
            warnings: Vec::new(),
            report: None,
        })
    }

    pub fn fs(&self) -> &F {
        &self.fs
    }

    /// Test and interleave hook: the filesystem, to change the destination mid-restore.
    pub fn fs_mut(&mut self) -> &mut F {
        &mut self.fs
    }

    /// True once the commit rename has happened.
    pub fn committed(&self) -> bool {
        self.did_commit
    }

    pub fn journal_store(&self) -> &J {
        &self.journal_store
    }

    pub fn into_parts(self) -> (F, J) {
        (self.fs, self.journal_store)
    }

    /// Before the commit rename, a cancel is honored at the next step; after it, it's too
    /// late and the restore finishes (Requirements 9.11, 9.12).
    pub fn try_cancel(&mut self) -> CancelResult {
        if self.did_commit || self.phase == RPhase::Done {
            CancelResult::TooLate
        } else {
            self.cancelled = true;
            CancelResult::Accepted
        }
    }

    fn now(&self) -> String {
        rfc3339(self.clock.now_unix())
    }

    fn record(
        &mut self,
        op: WriteOp,
        root: WriteRoot,
        path: &str,
        bytes: Option<u64>,
    ) -> Result<(), DormbitError> {
        self.seq += 1;
        let rec = WriteRecord {
            seq: self.seq,
            op,
            root,
            path: path.to_owned(),
            bytes,
        };
        self.journal_store.append_write(&self.ctx.op_id, &rec)
    }

    fn set_state(&mut self, state: RestoreState) -> Result<(), DormbitError> {
        let now = self.now();
        let j = self
            .journal
            .as_mut()
            .ok_or_else(|| io_err("The restore journal is missing."))?;
        j.state = state;
        j.updated_at = now.clone();
        j.history.push(StateStamp { state, at: now });
        let snapshot = j.clone();
        self.journal_store.set_state(&snapshot)
    }

    // ---- phases ------------------------------------------------------------------------------

    fn stage(&mut self) -> Result<(), DormbitError> {
        let now = self.now();
        let j = Journal {
            schema_version: JOURNAL_SCHEMA,
            op_id: self.ctx.op_id.clone(),
            staging: self.staging_name.as_str().to_owned(),
            owner_token: self.ctx.owner_token.clone(),
            state: RestoreState::Staged,
            capsule_id: self.plan.capsule_id.clone(),
            manifest_sha256: self.plan.manifest_sha256.clone(),
            target_grant_id: self.ctx.target_grant_id.clone(),
            target_name: self.plan.target_name.as_str().to_owned(),
            tree_id: None,
            files: self.plan.files.len() as u64,
            bytes: self.plan.bytes_total,
            history: vec![StateStamp {
                state: RestoreState::Staged,
                at: now.clone(),
            }],
            created_at: now.clone(),
            updated_at: now,
        };
        // The journal exists before anything is written to the destination.
        self.journal_store.create(&j)?;
        let owner = j.owner_line();
        self.journal = Some(j);

        let staging = self
            .fs
            .create_dir_excl(&self.parent, &self.staging_name)
            .map_err(|f| f.to_error("The staging folder"))?;
        self.record(WriteOp::Mkdir, WriteRoot::Staging, "", None)?;
        self.fs
            .write_new_file(
                &staging,
                &internal_name(OWNER_FILE),
                owner.as_bytes(),
                0o600,
            )
            .map_err(|f| f.to_error("The owner token"))?;
        self.record(
            WriteOp::Create,
            WriteRoot::Staging,
            OWNER_FILE,
            Some(owner.len() as u64),
        )?;

        // Case confirmation (Requirements 7.6, 7.7): if pre-checks assumed a case-sensitive
        // volume but it folds case, re-check collisions with fold keys.
        if self.plan.case_rules == CaseSensitivity::Sensitive
            && self
                .fs
                .lookup_nofollow(&staging, &OWNER_FILE.to_uppercase())
                .ok()
                .flatten()
                .is_some()
            && let Some((a, b)) = fold_collision(&self.plan)
        {
            self.staging = Some(staging);
            return Err(DormbitError::new(
                ErrorCode::RestoreCollision,
                format!("{a} and {b} would be the same name on this volume."),
            ));
        }

        let tree = self
            .fs
            .create_dir_excl(&staging, &internal_name(TREE))
            .map_err(|f| f.to_error("The staged tree"))?;
        self.record(WriteOp::Mkdir, WriteRoot::Staging, TREE, None)?;
        let id: Option<EntryMeta> = self.fs.lookup_nofollow(&staging, TREE).ok().flatten();
        if let Some(j) = self.journal.as_mut() {
            j.tree_id = id.map(|m| DevIno {
                dev: m.dev,
                ino: m.ino,
            });
        }
        self.dirs.insert(String::new(), tree);
        self.staging = Some(staging);
        self.set_state(RestoreState::Staged)
    }

    fn write_some(&mut self, budget: Budget) -> Result<bool, DormbitError> {
        let ndirs = self.plan.dirs.len();
        let total = ndirs + self.plan.files.len();
        let (mut ops, mut bytes) = (0u32, 0u64);
        while self.cursor < total {
            if self.cursor < ndirs {
                let d = self.plan.dirs[self.cursor].clone();
                let parent_key = path_of(d.parent());
                let name = d
                    .file_name()
                    .cloned()
                    .ok_or_else(|| io_err("empty folder path"))?;
                let parent = self
                    .dirs
                    .get(&parent_key)
                    .ok_or_else(|| io_err("A parent folder wasn't staged."))?;
                let h = self
                    .fs
                    .create_dir_excl(parent, &name)
                    .map_err(|f| collision_or(f, &d.display()))?;
                self.dirs.insert(d.display(), h);
                self.record(
                    WriteOp::Mkdir,
                    WriteRoot::Staging,
                    &format!("{TREE}/{}", d.display()),
                    None,
                )?;
            } else {
                let f = self.plan.files[self.cursor - ndirs].clone();
                let content = self.objects.content(
                    &f.sha256,
                    &f.stored_sha256,
                    f.stored_size,
                    f.encoding,
                    f.size,
                )?;
                let parent_key = path_of(f.path.parent());
                let name = f
                    .path
                    .file_name()
                    .cloned()
                    .ok_or_else(|| io_err("empty file path"))?;
                let parent = self
                    .dirs
                    .get(&parent_key)
                    .ok_or_else(|| io_err("A parent folder wasn't staged."))?;
                self.fs
                    .write_new_file(parent, &name, &content, f.final_mode)
                    .map_err(|e| collision_or(e, &f.path.display()))?;
                self.record(
                    WriteOp::Create,
                    WriteRoot::Staging,
                    &format!("{TREE}/{}", f.path.display()),
                    Some(f.size),
                )?;
                self.written.files += 1;
                self.written.bytes = self.written.bytes.saturating_add(f.size);
                bytes = bytes.saturating_add(f.size);
            }
            self.cursor += 1;
            ops = ops.saturating_add(1);
            if ops >= budget.max_ops || bytes >= budget.max_bytes {
                break;
            }
        }
        if self.cursor < total {
            return Ok(false);
        }
        let keys: Vec<String> = self.dirs.keys().cloned().collect();
        for k in keys {
            if let Some(d) = self.dirs.get(&k) {
                self.fs
                    .sync_dir(d)
                    .map_err(|f| f.to_error("A staged folder"))?;
            }
        }
        self.set_state(RestoreState::Written)?;
        Ok(true)
    }

    /// Re-opens the staged tree with fresh handles from the parent.
    fn fresh_tree(&self) -> Result<F::Dir, DormbitError> {
        let staging = self
            .fs
            .open_dir_nofollow(&self.parent, &self.staging_name)
            .map_err(|f| f.to_error("The staging folder"))?;
        self.fs
            .open_dir_nofollow(&staging, &internal_name(TREE))
            .map_err(|f| f.to_error("The staged tree"))
    }

    fn verify_some(&mut self, budget: Budget) -> Result<bool, DormbitError> {
        let tree = self.fresh_tree()?;
        let (mut ops, mut bytes) = (0u32, 0u64);
        while self.cursor < self.plan.files.len() {
            let f = self.plan.files[self.cursor].clone();
            match check_file(&self.fs, &tree, &f) {
                None => {
                    self.verified.files += 1;
                    self.verified.bytes = self.verified.bytes.saturating_add(f.size);
                }
                Some(_) => {
                    return Err(DormbitError::new(
                        ErrorCode::CapsuleHashMismatch,
                        format!(
                            "{} didn't match the capsule after it was written. Nothing was restored.",
                            f.path.display()
                        ),
                    ));
                }
            }
            bytes = bytes.saturating_add(f.size);
            self.cursor += 1;
            ops = ops.saturating_add(1);
            if ops >= budget.max_ops || bytes >= budget.max_bytes {
                return Ok(false);
            }
        }
        // The staged tree holds exactly the planned folders and files, and nothing else.
        let mut found = BTreeSet::new();
        walk(&self.fs, &tree, "", &mut found, 0)?;
        let mut want: BTreeSet<String> = self
            .plan
            .dirs
            .iter()
            .map(|d| format!("{}/", d.display()))
            .collect();
        want.extend(self.plan.files.iter().map(|f| f.path.display()));
        if found != want {
            return Err(DormbitError::new(
                ErrorCode::CapsuleHashMismatch,
                "The staged folder doesn't hold exactly the planned files. Nothing was restored.",
            ));
        }
        self.set_state(RestoreState::Verified)?;
        Ok(true)
    }

    fn commit(&mut self) -> Result<(), DormbitError> {
        let target = self.plan.target_name.clone();
        // Re-check for the target or any variant of it right before the rename (2.4).
        let exists = self
            .fs
            .list(&self.parent)
            .map_err(|f| f.to_error("The destination folder"))?
            .iter()
            .any(|(n, _)| {
                nfc_key(n) == nfc_key(target.as_str())
                    || (self.plan.key_mode == KeyMode::NfcAndFold
                        && fold_key(n) == fold_key(target.as_str()))
            });
        if exists {
            return Err(DormbitError::new(
                ErrorCode::RestoreDestExists,
                "Something with the destination name appeared while restoring. Nothing was replaced.",
            ));
        }
        let staging = self
            .staging
            .as_ref()
            .ok_or_else(|| io_err("The staging folder is missing."))?;
        self.fs
            .rename_exclusive(staging, &internal_name(TREE), &self.parent, &target)
            .map_err(|f| f.to_error("The destination folder"))?;
        // From here on, the target exists: no rollback, no cancel.
        self.did_commit = true;
        self.phase = RPhase::Clean;
        self.record(WriteOp::Rename, WriteRoot::Target, "", None)?;
        if self.fs.sync_dir(&self.parent).is_err() {
            self.warnings
                .push("The destination folder couldn't be synced to disk after the commit.".into());
        }
        if let Err(e) = self.set_state(RestoreState::Committed) {
            self.warnings.push(format!(
                "The journal couldn't record the commit ({}); recovery will detect it from the disk.",
                e.message
            ));
        }
        Ok(())
    }

    fn clean(&mut self) {
        let owner = self
            .journal
            .as_ref()
            .map(Journal::owner_line)
            .unwrap_or_default();
        match check_owner_token(&self.fs, &self.parent, &self.staging_name, &owner) {
            Ok(proof) => match self.fs.remove_staging(&self.parent, proof) {
                Ok(()) => {
                    let _ = self.record(WriteOp::RemoveStaging, WriteRoot::Staging, "", None);
                    if self.set_state(RestoreState::Cleaned).is_err() {
                        self.warnings
                            .push("The journal couldn't record the cleanup.".into());
                    }
                }
                Err(_) => self.warnings.push(
                    "The staging folder couldn't be removed; it stays until the next launch."
                        .into(),
                ),
            },
            Err(why) => self.warnings.push(format!(
                "The staging folder was not deleted: {}.",
                owner_reason(&why)
            )),
        }
    }

    fn post_verify(&mut self) -> RestoreReport {
        let mut mismatches = Vec::new();
        let mut verified = Totals::default();
        let target = self
            .fs
            .open_dir_nofollow(&self.parent, &self.plan.target_name);
        let git_check = match &target {
            Ok(t) => {
                for f in &self.plan.files {
                    match check_file(&self.fs, t, f) {
                        None => {
                            verified.files += 1;
                            verified.bytes = verified.bytes.saturating_add(f.size);
                        }
                        Some(m) => mismatches.push(m),
                    }
                }
                let view = RestoreView {
                    fs: &self.fs,
                    root: t,
                };
                git_check(&view, self.ctx.manifest_git.as_ref())
            }
            Err(_) => {
                self.warnings
                    .push("The restored folder couldn't be opened to check it.".into());
                GitCheck {
                    refs: RefCheck::NotApplicable,
                    worktree: WorktreeCheck::NotApplicable,
                }
            }
        };
        let refs_ok = !matches!(git_check.refs, RefCheck::Mismatch { .. });
        let status = if target.is_ok() && mismatches.is_empty() && refs_ok {
            RestoreStatus::Verified
        } else {
            RestoreStatus::Failed
        };
        self.verified = verified;
        self.build(status, None, mismatches, git_check)
    }

    fn build(
        &self,
        status: RestoreStatus,
        error: Option<DormbitError>,
        mismatches: Vec<Mismatch>,
        git_check: GitCheck,
    ) -> RestoreReport {
        let writes = self
            .journal_store
            .writes(&self.ctx.op_id)
            .unwrap_or_default();
        let outside = writes.iter().filter(|w| w.root == WriteRoot::Other).count() as u64;
        let executables = self
            .plan
            .files
            .iter()
            .filter(|f| f.final_mode & 0o111 != 0)
            .map(|f| f.path.display())
            .collect();
        let stripped = self
            .plan
            .files
            .iter()
            .filter(|f| f.stripped_bits != 0)
            .map(|f| StrippedBits {
                path: f.path.display(),
                removed: [(0o4000, "setuid"), (0o2000, "setgid"), (0o1000, "sticky")]
                    .iter()
                    .filter(|(b, _)| f.stripped_bits & b != 0)
                    .map(|(_, n)| (*n).to_owned())
                    .collect(),
            })
            .collect();
        RestoreReport {
            schema_version: REPORT_SCHEMA,
            op_id: self.ctx.op_id.clone(),
            capsule_id: self.plan.capsule_id.clone(),
            target_display: self.ctx.target_display.clone(),
            started_at: self.started_at.clone(),
            finished_at: self.now(),
            status,
            recovered: false,
            error,
            files_written: self.written,
            files_verified: self.verified,
            mismatches,
            not_restored: self.plan.skipped.clone(),
            executables,
            stripped_mode_bits: stripped,
            source_unchanged: SourceCheck::NotChecked {
                reason: "The original folder is checked by the desktop app after the restore."
                    .into(),
            },
            writes_outside_destination: outside,
            git_check,
            warnings: self.warnings.clone(),
            limits: RESTORE_LIMITS.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// Rolls back after a failure or cancel before the commit rename (Requirement 9.10).
    fn rollback(&mut self, error: DormbitError) -> RestoreReport {
        // Nothing staged survives a rollback, so nothing counts as written.
        if self.written.files > 0 {
            self.warnings.push(format!(
                "{} file(s) were written to the staging folder and then removed.",
                self.written.files
            ));
        }
        self.written = Totals::default();
        self.verified = Totals::default();
        if self.staging.is_some() || self.journal.is_some() {
            let owner = self
                .journal
                .as_ref()
                .map(Journal::owner_line)
                .unwrap_or_default();
            match check_owner_token(&self.fs, &self.parent, &self.staging_name, &owner) {
                Ok(proof) => {
                    if self.fs.remove_staging(&self.parent, proof).is_ok() {
                        let _ = self.record(WriteOp::RemoveStaging, WriteRoot::Staging, "", None);
                    } else {
                        self.warnings.push(
                            "The staging folder couldn't be removed; DORMBIT will offer to clean it up at the next launch.".into(),
                        );
                    }
                }
                Err(OwnerCheckFailure::StagingMissing) => {}
                Err(why) => self.warnings.push(format!(
                    "The staging folder was not deleted: {}.",
                    owner_reason(&why)
                )),
            }
            if self.journal.is_some() && self.set_state(RestoreState::Cleaned).is_err() {
                self.warnings
                    .push("The journal couldn't record the rollback.".into());
            }
        }
        self.build(
            RestoreStatus::RolledBack,
            Some(error),
            Vec::new(),
            GitCheck {
                refs: RefCheck::NotApplicable,
                worktree: WorktreeCheck::NotApplicable,
            },
        )
    }

    fn finish(&mut self, report: RestoreReport) -> Step<RestoreReport> {
        if self.journal.is_some()
            && self
                .journal_store
                .put_report(&self.ctx.op_id, &report)
                .is_err()
        {
            // The report is still returned to the caller.
        }
        self.phase = RPhase::Done;
        self.report = Some(report.clone());
        Step::Done(report)
    }

    fn progress(&self) -> JobProgress {
        let phase = match self.phase {
            RPhase::Stage => Phase::Preparing,
            RPhase::Write => Phase::Hashing,
            RPhase::Verify => Phase::Rechecking,
            RPhase::Commit | RPhase::Clean => Phase::Committing,
            RPhase::PostVerify => Phase::Recording,
            RPhase::Done => Phase::Done,
        };
        JobProgress {
            phase,
            files_done: self.written.files.max(self.verified.files),
            files_total: self.plan.files.len() as u64,
            bytes_done: self.written.bytes.max(self.verified.bytes),
            bytes_total: self.plan.bytes_total,
        }
    }

    fn run(&mut self, budget: Budget) -> Result<(), DormbitError> {
        match self.phase {
            RPhase::Stage => {
                self.stage()?;
                self.phase = RPhase::Write;
                self.cursor = 0;
            }
            RPhase::Write => {
                if self.write_some(budget)? {
                    self.phase = RPhase::Verify;
                    self.cursor = 0;
                }
            }
            RPhase::Verify => {
                if self.verify_some(budget)? {
                    self.phase = RPhase::Commit;
                }
            }
            RPhase::Commit => self.commit()?,
            RPhase::Clean => {
                self.clean();
                self.phase = RPhase::PostVerify;
            }
            RPhase::PostVerify | RPhase::Done => {}
        }
        Ok(())
    }
}

impl<F: RestoreFs, J: JournalStore, O: CapsuleObjects, C: Clock> Job for RestoreJob<F, J, O, C> {
    type Output = RestoreReport;

    fn step(&mut self, budget: Budget) -> Result<Step<RestoreReport>, DormbitError> {
        if let Some(r) = &self.report {
            return Ok(Step::Done(r.clone()));
        }
        if self.cancelled && !self.did_commit {
            let r = self.rollback(DormbitError::new(
                ErrorCode::Cancelled,
                "Restore cancelled. Nothing was restored.",
            ));
            return Ok(self.finish(r));
        }
        if self.phase == RPhase::PostVerify {
            let r = self.post_verify();
            return Ok(self.finish(r));
        }
        match self.run(budget) {
            Ok(()) => Ok(Step::Progress(self.progress())),
            // Anything that fails before the rename happened rolls back, including a failure
            // inside the commit step itself (for example a target that just appeared).
            Err(e) if !self.did_commit => {
                let r = self.rollback(e);
                Ok(self.finish(r))
            }
            Err(e) => {
                self.warnings.push(e.message);
                self.phase = RPhase::PostVerify;
                Ok(Step::Progress(self.progress()))
            }
        }
    }

    fn cancel(&mut self) {
        let _ = self.try_cancel();
    }
}

// ---- helpers ---------------------------------------------------------------------------------

fn owner_reason(why: &OwnerCheckFailure) -> &'static str {
    match why {
        OwnerCheckFailure::NotStaging => "its name isn't a DORMBIT staging folder",
        OwnerCheckFailure::StagingMissing => "it was already gone",
        OwnerCheckFailure::TokenMissing => "its owner token is missing",
        OwnerCheckFailure::TokenMismatch => "its owner token doesn't match this restore",
    }
}

fn collision_or(f: super::fs::FsFault, path: &str) -> DormbitError {
    match f {
        super::fs::FsFault::Exists => DormbitError::new(
            ErrorCode::RestoreCollision,
            format!("{path} collides with another name on this volume. Nothing was restored."),
        ),
        other => other.to_error(path),
    }
}

/// Two planned paths that fold to the same key (used when the volume turns out to fold case).
fn fold_collision(plan: &RestorePlan) -> Option<(String, String)> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let all = plan
        .dirs
        .iter()
        .map(|d| format!("{}/", d.display()))
        .chain(plan.files.iter().map(|f| f.path.display()));
    for p in all {
        let k: String = p.split('/').map(fold_key).collect::<Vec<_>>().join("/");
        if let Some(prev) = seen.insert(k, p.clone())
            && prev != p
        {
            return Some((prev, p));
        }
    }
    None
}

/// Reads one planned file under `root` with no-follow handles and compares it with the plan.
fn check_file<F: RestoreFs>(
    fs: &F,
    root: &F::Dir,
    f: &super::planner::PlannedFile,
) -> Option<Mismatch> {
    let mismatch =
        |actual_sha: Option<String>, actual_size: Option<u64>, actual_mode: Option<u32>| {
            Some(Mismatch {
                path: f.path.display(),
                expected_sha256: f.sha256.clone(),
                actual_sha256: actual_sha,
                expected_size: f.size,
                actual_size,
                expected_mode: f.final_mode,
                actual_mode,
            })
        };
    let comps = f.path.components();
    let (dirs, name) = comps.split_at(comps.len().saturating_sub(1));
    let Some(name) = name.first() else {
        return mismatch(None, None, None);
    };
    let mut owned: Option<F::Dir> = None;
    for d in dirs {
        let next = match &owned {
            None => fs.open_dir_nofollow(root, d),
            Some(cur) => fs.open_dir_nofollow(cur, d),
        };
        match next {
            Ok(h) => owned = Some(h),
            Err(_) => return mismatch(None, None, None),
        }
    }
    let dir = owned.as_ref().unwrap_or(root);
    let meta = fs.lookup_nofollow(dir, name.as_str()).ok().flatten();
    let Some(meta) = meta.filter(|m| m.kind == MetaKind::File) else {
        return mismatch(None, None, None);
    };
    let data = match fs.read_file_nofollow(dir, name, f.size) {
        Ok(d) => d,
        Err(_) => return mismatch(None, Some(meta.size), Some(meta.mode & 0o777)),
    };
    let sha = sha256_hex(&data);
    let mode = meta.mode & 0o777;
    if sha != f.sha256 || data.len() as u64 != f.size || mode != f.final_mode {
        return mismatch(Some(sha), Some(data.len() as u64), Some(mode));
    }
    None
}

/// Lists every folder (with a trailing `/`) and file under `dir`.
fn walk<F: RestoreFs>(
    fs: &F,
    dir: &F::Dir,
    prefix: &str,
    out: &mut BTreeSet<String>,
    depth: u32,
) -> Result<(), DormbitError> {
    if depth > 256 {
        return Err(io_err("The staged folder is nested too deeply."));
    }
    for (name, meta) in fs.list(dir).map_err(|f| f.to_error("The staged folder"))? {
        let p = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        match meta.kind {
            MetaKind::Dir => {
                out.insert(format!("{p}/"));
                let n = internal_name(&name);
                let h = fs
                    .open_dir_nofollow(dir, &n)
                    .map_err(|f| f.to_error("The staged folder"))?;
                walk(fs, &h, &p, out, depth + 1)?;
            }
            _ => {
                out.insert(p);
            }
        }
    }
    Ok(())
}

/// A read-only `ReadVfs` over a restored folder, for the Git ref reader.
struct RestoreView<'a, F: RestoreFs> {
    fs: &'a F,
    root: &'a F::Dir,
}

impl<F: RestoreFs> RestoreView<'_, F> {
    fn with_dir<T>(
        &self,
        comps: &[&str],
        f: impl FnOnce(&F::Dir) -> Result<T, VfsError>,
    ) -> Result<T, VfsError> {
        let mut owned: Option<F::Dir> = None;
        for c in comps {
            let n = internal_name(c);
            let next = match &owned {
                None => self.fs.open_dir_nofollow(self.root, &n),
                Some(cur) => self.fs.open_dir_nofollow(cur, &n),
            }
            .map_err(|_| VfsError::NotFound)?;
            owned = Some(next);
        }
        f(owned.as_ref().unwrap_or(self.root))
    }

    fn split(path: &RelPath) -> (Vec<&str>, Option<&str>) {
        if path.is_root() {
            return (Vec::new(), None);
        }
        let mut parts: Vec<&str> = path.as_str().split('/').collect();
        let last = parts.pop();
        (parts, last)
    }
}

impl<F: RestoreFs> ReadVfs for RestoreView<'_, F> {
    fn read_dir(&self, dir: &RelPath) -> Result<Vec<DirEntry>, VfsError> {
        let comps: Vec<&str> = if dir.is_root() {
            Vec::new()
        } else {
            dir.as_str().split('/').collect()
        };
        self.with_dir(&comps, |d| {
            Ok(self
                .fs
                .list(d)
                .map_err(|_| VfsError::NotFound)?
                .into_iter()
                .map(|(name, m)| DirEntry {
                    name,
                    name_ok: true,
                    kind: match m.kind {
                        MetaKind::File => EntryKind::File,
                        MetaKind::Dir => EntryKind::Dir,
                        MetaKind::Symlink => EntryKind::Symlink,
                        MetaKind::Other => EntryKind::Special,
                    },
                })
                .collect())
        })
    }

    fn symlink_metadata(&self, path: &RelPath) -> Result<Metadata, VfsError> {
        let (dirs, last) = Self::split(path);
        let last = last.ok_or(VfsError::NotFound)?;
        self.with_dir(&dirs, |d| {
            let m = self
                .fs
                .lookup_nofollow(d, last)
                .map_err(|_| VfsError::NotFound)?
                .ok_or(VfsError::NotFound)?;
            Ok(Metadata {
                kind: match m.kind {
                    MetaKind::File => EntryKind::File,
                    MetaKind::Dir => EntryKind::Dir,
                    MetaKind::Symlink => EntryKind::Symlink,
                    MetaKind::Other => EntryKind::Special,
                },
                len: m.size,
                mtime: 0,
                mode: m.mode,
            })
        })
    }

    fn read_limited(&self, path: &RelPath, max: u64) -> Result<Limited, VfsError> {
        let (dirs, last) = Self::split(path);
        let last = last.ok_or(VfsError::NotFound)?;
        self.with_dir(&dirs, |d| {
            match self.fs.read_file_nofollow(d, &internal_name(last), max) {
                Ok(b) => Ok(Limited::Complete(b)),
                Err(super::fs::FsFault::Io(m)) if m == "file too large" => {
                    Ok(Limited::TooLarge { len: max + 1 })
                }
                Err(_) => Err(VfsError::NotFound),
            }
        })
    }

    fn read_prefix(&self, path: &RelPath, n: u64) -> Result<Vec<u8>, VfsError> {
        match self.read_limited(path, 1 << 20)? {
            Limited::Complete(mut b) => {
                b.truncate(usize::try_from(n).unwrap_or(usize::MAX));
                Ok(b)
            }
            Limited::TooLarge { .. } => Err(VfsError::Io("too large".into())),
        }
    }

    fn read_link(&self, _path: &RelPath) -> Result<String, VfsError> {
        Err(VfsError::Io("links are not restored".into()))
    }
}

/// Compares the restored repository's HEAD and refs with the manifest (RST-4).
fn git_check<V: ReadVfs>(view: &V, git: Option<&ManifestGit>) -> GitCheck {
    let worktree = WorktreeCheck::NotChecked {
        reason: "Working-tree status is checked by the desktop app with libgit2.".into(),
    };
    let Some(g) = git else {
        return GitCheck {
            refs: RefCheck::NotApplicable,
            worktree: WorktreeCheck::NotApplicable,
        };
    };
    if g.treatment == GitTreatment::WorkingTreeOnly {
        return GitCheck {
            refs: RefCheck::NotApplicable,
            worktree: WorktreeCheck::NotApplicable,
        };
    }
    let Some(head) = &g.head else {
        return GitCheck {
            refs: RefCheck::RefsNotRecorded,
            worktree,
        };
    };
    let found = read_refs(view);
    let mut differ = Vec::new();
    if found.head.as_ref() != Some(head) {
        differ.push("HEAD".to_owned());
    }
    let want: BTreeMap<&str, &str> = g
        .refs
        .iter()
        .map(|r| (r.name.as_str(), r.target.as_str()))
        .collect();
    let got: BTreeMap<&str, &str> = found
        .refs
        .iter()
        .map(|r| (r.name.as_str(), r.target.as_str()))
        .collect();
    for (name, target) in &want {
        if got.get(name) != Some(target) {
            differ.push((*name).to_owned());
        }
    }
    for name in got.keys() {
        if !want.contains_key(name) {
            differ.push((*name).to_owned());
        }
    }
    GitCheck {
        refs: if differ.is_empty() {
            RefCheck::Match
        } else {
            RefCheck::Mismatch { refs: differ }
        },
        worktree,
    }
}
