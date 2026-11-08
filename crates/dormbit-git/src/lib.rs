//! Read-only Git inspection through libgit2 (`git2`), for the evidence engine's
//! `GitSnapshot` (ADR-004).
//!
//! - Never runs the git CLI or any other program. libgit2 doesn't run fsmonitor hooks,
//!   credential helpers, or external filter drivers during these reads.
//! - Never contacts a remote: no transport feature is compiled in (`scripts/check-deps.sh`).
//! - Never writes to the repository: status runs without refreshing the index, and only the
//!   key names of risky config entries are kept, never their values.
//! - libgit2 reads the user's global Git config to compute status (for example
//!   `core.excludesFile`), as `git status` would. Nothing from it leaves this module except
//!   the counts.
//!
//! Callers pass only a working directory whose `.git` is a real folder inside the grant; the
//! desktop host checks that without following symlinks before calling [`inspect`].

use std::path::Path;

use dormbit_core::model::{GitHead, GitSnapshot, GitStatus, GitUpstream, SafeU64};
use dormbit_core::util::time::rfc3339;
use git2::{BranchType, Config, ErrorCode, Repository, RepositoryOpenFlags, Status, StatusOptions};

pub const CORE_VERSION: &str = dormbit_core::VERSION;

/// Why there is no snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitIssue {
    NotARepo,
    Bare,
    Unreadable,
}

/// Reads HEAD, status counts, stash count, upstream, submodules, LFS use, risky config key
/// names, and remotes (credentials removed) of the repository at `workdir`.
pub fn inspect(workdir: &Path) -> Result<GitSnapshot, GitIssue> {
    let mut repo = Repository::open_ext(
        workdir,
        RepositoryOpenFlags::NO_SEARCH,
        &[] as &[&std::ffi::OsStr],
    )
    .map_err(|e| {
        if e.code() == ErrorCode::NotFound {
            GitIssue::NotARepo
        } else {
            GitIssue::Unreadable
        }
    })?;
    if repo.is_bare() {
        return Err(GitIssue::Bare);
    }
    let head = head_of(&repo)?;
    let status = status_of(&repo)?;
    let upstream = upstream_of(&repo);
    let submodules = repo
        .submodules()
        .map(|v| {
            v.iter()
                .filter_map(|s| s.name().ok().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let lfs_detected = lfs_detected(&repo, workdir);
    let risky_config = risky_config(&repo);
    let remotes_redacted = remotes(&repo);
    let mut stashes = 0u64;
    // A repository without a stash reports an error here; that just means zero.
    let _ = repo.stash_foreach(|_, _, _| {
        stashes += 1;
        true
    });
    Ok(GitSnapshot {
        head,
        upstream,
        status,
        stash_count: SafeU64::saturating(stashes),
        worktree_external: false,
        submodules,
        lfs_detected,
        risky_config,
        remotes_redacted,
    })
}

fn head_of(repo: &Repository) -> Result<GitHead, GitIssue> {
    let detached = repo.head_detached().unwrap_or(false);
    match repo.head() {
        Ok(r) => {
            let commit = r.peel_to_commit().map_err(|_| GitIssue::Unreadable)?;
            Ok(GitHead {
                branch: if detached {
                    None
                } else {
                    r.shorthand().ok().map(str::to_owned)
                },
                detached,
                commit: commit.id().to_string(),
                time: rfc3339(commit.time().seconds()),
                summary: commit.summary().ok().flatten().unwrap_or("").to_owned(),
            })
        }
        // HEAD names a branch that has no commits yet.
        Err(e) if e.code() == ErrorCode::UnbornBranch => {
            let branch = repo.find_reference("HEAD").ok().and_then(|h| {
                h.symbolic_target()
                    .ok()
                    .flatten()
                    .map(|t| t.trim_start_matches("refs/heads/").to_owned())
            });
            Ok(GitHead {
                branch,
                detached: false,
                commit: String::new(),
                time: String::new(),
                summary: String::new(),
            })
        }
        Err(_) => Err(GitIssue::Unreadable),
    }
}

fn status_of(repo: &Repository) -> Result<GitStatus, GitIssue> {
    let mut o = StatusOptions::new();
    o.include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true)
        .include_ignored(false)
        .update_index(false);
    let list = repo
        .statuses(Some(&mut o))
        .map_err(|_| GitIssue::Unreadable)?;
    let staged_flags = Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE;
    let modified_flags = Status::WT_MODIFIED | Status::WT_TYPECHANGE | Status::WT_RENAMED;
    let (mut modified, mut staged, mut untracked, mut deleted, mut conflicted) = (0, 0, 0, 0, 0);
    // A file counts once in each category it has a flag for.
    for e in list.iter() {
        let f = e.status();
        if f.intersects(staged_flags) {
            staged += 1;
        }
        if f.intersects(modified_flags) {
            modified += 1;
        }
        if f.contains(Status::WT_DELETED) {
            deleted += 1;
        }
        if f.contains(Status::WT_NEW) {
            untracked += 1;
        }
        if f.contains(Status::CONFLICTED) {
            conflicted += 1;
        }
    }
    Ok(GitStatus {
        modified: SafeU64::saturating(modified),
        staged: SafeU64::saturating(staged),
        untracked: SafeU64::saturating(untracked),
        deleted: SafeU64::saturating(deleted),
        conflicted: SafeU64::saturating(conflicted),
    })
}

fn upstream_of(repo: &Repository) -> Option<GitUpstream> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let local = repo
        .find_branch(head.shorthand().ok()?, BranchType::Local)
        .ok()?;
    let up = local.upstream().ok()?;
    let (ahead, behind) = repo
        .graph_ahead_behind(head.target()?, up.get().target()?)
        .ok()?;
    Some(GitUpstream {
        ahead: SafeU64::saturating(ahead as u64),
        behind: SafeU64::saturating(behind as u64),
        // The counts are only as fresh as the last fetch; no remote is contacted here.
        as_of_last_fetch: repo.path().join("FETCH_HEAD").exists(),
    })
}

fn lfs_detected(repo: &Repository, workdir: &Path) -> bool {
    if repo.path().join("lfs").is_dir() {
        return true;
    }
    // Read .gitattributes only if it is a small regular file (never through a symlink).
    let p = workdir.join(".gitattributes");
    match std::fs::symlink_metadata(&p) {
        Ok(m) if m.file_type().is_file() && m.len() <= 64 * 1024 => std::fs::read_to_string(&p)
            .map(|t| t.contains("filter=lfs"))
            .unwrap_or(false),
        _ => false,
    }
}

/// Key names of risky entries in the repository's own config files (never their values).
fn risky_config(repo: &Repository) -> Vec<String> {
    let mut keys = Vec::new();
    for name in ["config", "config.worktree"] {
        let p = repo.path().join(name);
        let regular = std::fs::symlink_metadata(&p)
            .map(|m| m.file_type().is_file())
            .unwrap_or(false);
        if !regular {
            continue;
        }
        let Ok(cfg) = Config::open(&p) else { continue };
        let Ok(entries) = cfg.entries(None) else {
            continue;
        };
        let _ = entries.for_each(|e| {
            if let Ok(n) = e.name()
                && risky_key(n)
            {
                keys.push(n.to_owned());
            }
        });
    }
    keys.sort();
    keys.dedup();
    keys
}

fn remotes(repo: &Repository) -> Vec<String> {
    let Ok(names) = repo.remotes() else {
        return Vec::new();
    };
    names
        .iter()
        .filter_map(|n| n.ok().flatten())
        .filter_map(|n| {
            let r = repo.find_remote(n).ok()?;
            Some(format!("{n} {}", redact_remote_url(r.url().unwrap_or(""))))
        })
        .collect()
}

/// Config keys that can make Git run a program or reveal a credential: `core.fsmonitor`,
/// `core.hookspath`, `core.sshcommand`, `credential.helper`, `credential.<url>.helper`, and
/// any `filter.<driver>.<var>`. Section and variable compare case-insensitively.
pub fn risky_key(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some((section, rest)) = lower.split_once('.') else {
        return false;
    };
    let var = rest.rsplit('.').next().unwrap_or("");
    let has_sub = rest.contains('.');
    match section {
        "core" => !has_sub && matches!(var, "fsmonitor" | "hookspath" | "sshcommand"),
        "credential" => var == "helper",
        "filter" => has_sub,
        _ => false,
    }
}

/// A remote URL with any credential removed:
/// - `scheme://user:pass@host/path?query#frag` becomes `scheme://***@host/path`.
/// - scp-like `user@host:path` keeps the user unless it holds a `:` (then `***`).
/// - Local paths show the home folder as `~`.
/// - Anything else is hidden.
pub fn redact_remote_url(raw: &str) -> String {
    let raw = raw.trim();
    let home = std::env::var("HOME").unwrap_or_default();
    let tilde = |p: &str| -> String {
        if !home.is_empty() && (p == home || p.starts_with(&format!("{home}/"))) {
            format!("~{}", &p[home.len()..])
        } else {
            p.to_owned()
        }
    };
    if let Some(path) = raw.strip_prefix("file://") {
        return format!("file://{}", tilde(path));
    }
    if raw.starts_with('/') || raw.starts_with("./") || raw.starts_with("../") {
        return tilde(raw);
    }
    if let Some((scheme, rest)) = raw.split_once("://") {
        if scheme.is_empty()
            || !scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        {
            return "(hidden: unrecognized URL format)".to_owned();
        }
        // Everything before the last '@' is user info, even if it holds '/', '?', or '#':
        // over-hiding a path is fine, leaking a password is not.
        let (userinfo, after) = match rest.rfind('@') {
            Some(i) => (true, &rest[i + 1..]),
            None => (false, rest),
        };
        let clean = after.split(['?', '#']).next().unwrap_or("");
        return if userinfo {
            format!("{scheme}://***@{clean}")
        } else {
            format!("{scheme}://{clean}")
        };
    }
    // scp-like: [user@]host:path. The user is everything before the last '@'.
    let (user, host_path) = match raw.rfind('@') {
        Some(i) => (Some(&raw[..i]), &raw[i + 1..]),
        None => (None, raw),
    };
    if let Some((host, path)) = host_path.split_once(':')
        && !host.is_empty()
        && !host.contains('/')
    {
        let who = match user {
            Some(u) if u.is_empty() || u.contains(':') || u.contains('/') => "***@".to_owned(),
            Some(u) => format!("{u}@"),
            None => String::new(),
        };
        let path = path.split(['?', '#']).next().unwrap_or("");
        return format!("{who}{host}:{path}");
    }
    "(hidden: unrecognized URL format)".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_removes_userinfo_and_query_from_scheme_urls() {
        let t = "s3cr3t-token-value";
        let out = redact_remote_url(&format!(
            "https://bob:{t}@git.example.invalid/acme/app.git?x={t}#{t}"
        ));
        assert_eq!(out, "https://***@git.example.invalid/acme/app.git");
        assert!(!out.contains(t));
    }

    #[test]
    fn redaction_keeps_a_plain_scp_user_and_hides_one_with_a_password() {
        assert_eq!(
            redact_remote_url("git@git.example.invalid:acme/app.git"),
            "git@git.example.invalid:acme/app.git"
        );
        let out = redact_remote_url("bob:pw-value@git.example.invalid:acme/app.git");
        assert_eq!(out, "***@git.example.invalid:acme/app.git");
    }

    #[test]
    fn redaction_never_leaks_a_password_holding_separators() {
        let t = "pa#ss?w@rd";
        for raw in [
            format!("https://bob:{t}@git.example.invalid/acme/app.git"),
            format!("bob:{t}@git.example.invalid:acme/app.git"),
        ] {
            let out = redact_remote_url(&raw);
            assert!(
                !out.contains("pa#ss") && !out.contains("ss?w"),
                "{raw} -> {out}"
            );
            assert!(out.contains("git.example.invalid"), "{raw} -> {out}");
        }
    }

    #[test]
    fn redaction_hides_unrecognized_formats() {
        assert_eq!(
            redact_remote_url("::::"),
            "(hidden: unrecognized URL format)"
        );
        assert_eq!(redact_remote_url(""), "(hidden: unrecognized URL format)");
    }

    #[test]
    fn risky_keys_match_section_and_variable_case_insensitively() {
        for k in [
            "core.fsmonitor",
            "core.hooksPath",
            "core.sshCommand",
            "credential.helper",
            "credential.https://example.invalid.helper",
            "filter.lfs.clean",
        ] {
            assert!(risky_key(k), "{k}");
        }
        for k in [
            "core.autocrlf",
            "user.name",
            "filter",
            "credential.username",
        ] {
            assert!(!risky_key(k), "{k}");
        }
    }
}
