//! The Sidecar (foundation Requirement 19.4 and 19.7): what the fixture is expected to contain,
//! so golden tests can check DORMBIT's findings against known answers.

use std::fs;
use std::path::Path;

use dormbit_core::model::{GitHead, GitSnapshot, GitStatus, SafeU64};
use git2::{Repository, Status, StatusOptions};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::pack::walk;
use crate::stores::{
    CLAUDE_SESSION, CLAUDE_TIMES, CLAUDE_VERSION, CWD, KIRO_ENDED, KIRO_SESSION, KIRO_STARTED,
    KIRO_VERSION,
};
use crate::time::rfc3339;
use crate::{Ctx, R};

pub const DEMO_CLOCK: &str = "2025-06-02T16:00:00Z";

pub fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn count(n: usize) -> R<SafeU64> {
    SafeU64::new(n as u64).map_err(|e| e.to_string())
}

pub fn git_snapshot(project: &Path) -> R<GitSnapshot> {
    let mut repo = Repository::open(project).ctx("open")?;
    let (mut modified, mut staged, mut untracked, mut deleted, mut conflicted) = (0, 0, 0, 0, 0);
    {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);
        for e in repo.statuses(Some(&mut opts)).ctx("status")?.iter() {
            let s = e.status();
            if s.contains(Status::CONFLICTED) {
                conflicted += 1;
                continue;
            }
            if s.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            ) {
                staged += 1;
            }
            if s.intersects(Status::WT_MODIFIED | Status::WT_TYPECHANGE | Status::WT_RENAMED) {
                modified += 1;
            }
            if s.contains(Status::WT_DELETED) {
                deleted += 1;
            }
            if s.contains(Status::WT_NEW) {
                untracked += 1;
            }
        }
    }
    let head = {
        let reference = repo.head().ctx("head")?;
        let commit = reference.peel_to_commit().ctx("head commit")?;
        GitHead {
            branch: reference.shorthand().ok().map(str::to_owned),
            detached: repo.head_detached().ctx("detached")?,
            commit: commit.id().to_string(),
            time: rfc3339(commit.time().seconds()),
            summary: commit
                .summary()
                .ok()
                .flatten()
                .unwrap_or_default()
                .to_owned(),
        }
    };
    let remotes = repo.remotes().ctx("remotes")?.len();
    let submodules: Vec<String> = repo
        .submodules()
        .ctx("submodules")?
        .iter()
        .map(|s| s.path().display().to_string())
        .collect();
    let mut stashes = 0;
    repo.stash_foreach(|_, _, _| {
        stashes += 1;
        true
    })
    .ctx("stash list")?;
    let lfs = fs::read_to_string(project.join(".gitattributes"))
        .map(|a| a.contains("filter=lfs"))
        .unwrap_or(false);
    if remotes != 0 {
        return Err("the fixture must have no remotes".into());
    }
    Ok(GitSnapshot {
        head,
        upstream: None,
        status: GitStatus {
            modified: count(modified)?,
            staged: count(staged)?,
            untracked: count(untracked)?,
            deleted: count(deleted)?,
            conflicted: count(conflicted)?,
        },
        stash_count: count(stashes)?,
        worktree_external: false,
        submodules,
        lfs_detected: lfs,
        risky_config: Vec::new(),
        remotes_redacted: Vec::new(),
    })
}

/// Rebuilds every object with its keys inserted in sorted order, whatever map type serde_json uses.
fn sorted(v: Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut entries: Vec<(String, Value)> = m.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = Map::new();
            for (k, v) in entries {
                out.insert(k, sorted(v));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(sorted).collect()),
        other => other,
    }
}

pub fn build(work: &Path, project: &Path, tar: &[u8]) -> R<String> {
    let mut hashes = Map::new();
    for rel in walk(work)? {
        if !rel.ends_with('/') {
            hashes.insert(
                rel.clone(),
                Value::String(hex(&fs::read(work.join(&rel)).ctx(&rel)?)),
            );
        }
    }
    let first = CLAUDE_TIMES[0];
    let last = CLAUDE_TIMES[CLAUDE_TIMES.len() - 1];
    let sidecar = json!({
        "schema_version": 1,
        "fixture_sha256": hex(tar),
        "demo_clock": DEMO_CLOCK,
        "git_snapshot": serde_json::to_value(git_snapshot(project)?).ctx("git snapshot")?,
        "agent_metadata": {
            "claude_code": [{
                "session_id": CLAUDE_SESSION, "working_directory": CWD, "started_at": first,
                "ended_at": last, "git_branch": crate::history::FEATURE, "tool_version": CLAUDE_VERSION,
            }],
            "kiro_export": [{
                "session_id": KIRO_SESSION, "working_directory": CWD, "started_at": KIRO_STARTED,
                "ended_at": KIRO_ENDED, "git_branch": crate::history::FEATURE, "tool_version": KIRO_VERSION,
            }],
        },
        "expected_hashes": Value::Object(hashes),
    });
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&sorted(sidecar)).ctx("sidecar")?
    ))
}
