//! The classifier: walks a project through [`ReadVfs`] and gives every file one [`ReasonCode`],
//! in the fixed precedence of the design (first match wins). Symlinks are never followed.

use std::collections::BTreeMap;

use unicode_normalization::UnicodeNormalization;

use super::gitignore::Gitignore;
use super::reason::ReasonCode;
use crate::capsule::CapsuleLimits;
use crate::model::{DormbitError, ErrorCode, GitTreatment, PlanItemKind};
use crate::vfs::{EntryKind, Limited, ReadVfs, RelPath};

/// Folders that tools recreate from the manifest (rule 11). Shown as one editable item each.
pub const REBUILDABLE_DIRS: [&str; 18] = [
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".parcel-cache",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".gradle",
    "Pods",
    "DerivedData",
    "coverage",
];

const CREDENTIAL_FILES: [&str; 8] = [
    ".netrc",
    ".npmrc",
    ".pypirc",
    "credentials",
    "credentials.json",
    ".credentials.json",
    "auth.json",
    ".git-credentials",
];

const PRIVATE_KEY_NAMES: [&str; 4] = ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"];
const ENV_SAFE_NAMES: [&str; 3] = [".env.example", ".env.sample", ".env.template"];

/// Hard cap on plan size, so a hostile tree can't exhaust memory before limits are checked.
const MAX_PLAN_ITEMS: usize = 2_000_000;
/// How much of a file is read to look at its first line (private-key header).
const KEY_PROBE: u64 = 16 * 1024;

/// A sensitive file by name alone (rules 3-6). Names are compared case-insensitively, since
/// macOS volumes usually are.
pub fn sensitive_name(name: &str) -> Option<ReasonCode> {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with(".env") && !ENV_SAFE_NAMES.contains(&lower.as_str()) {
        return Some(ReasonCode::SensitiveEnv);
    }
    if lower.ends_with(".pem") || lower.ends_with(".p12") {
        return Some(ReasonCode::SensitiveCert);
    }
    if PRIVATE_KEY_NAMES.contains(&lower.as_str()) || lower.ends_with(".key") {
        return Some(ReasonCode::SensitivePrivateKey);
    }
    if CREDENTIAL_FILES.contains(&lower.as_str()) {
        return Some(ReasonCode::SensitiveCredentials);
    }
    None
}

/// A PEM private key by content: the first line is a `-----BEGIN` header naming a private key.
pub fn sensitive_pem(bytes: &[u8]) -> bool {
    let first = bytes.split(|&b| b == b'\n').next().unwrap_or(&[]);
    let line = String::from_utf8_lossy(first);
    line.starts_with("-----BEGIN") && line.contains("PRIVATE KEY-----")
}

fn is_data_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    const DB: [&str; 5] = [".sqlite", ".sqlite3", ".db", ".dump", ".sql.gz"];
    if DB.iter().any(|e| lower.ends_with(e)) {
        return true;
    }
    lower.strip_suffix(".bak").is_some_and(|base| {
        base.ends_with(".db") || base.ends_with(".sqlite") || base.ends_with(".sqlite3")
    })
}

/// One classified file or collapsed folder, before decisions and warnings are attached.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: String,
    pub kind: PlanItemKind,
    pub size: u64,
    pub reason: ReasonCode,
    pub warning: Option<String>,
}

struct Ctx<'a> {
    treatment: GitTreatment,
    limits: &'a CapsuleLimits,
    gitignore: Gitignore,
}

pub fn classify_tree<V: ReadVfs>(
    vfs: &V,
    treatment: GitTreatment,
    limits: &CapsuleLimits,
) -> Result<Vec<Candidate>, DormbitError> {
    let gitignore = match vfs.read_limited(&RelPath::parse(".gitignore")?, 1 << 20) {
        Ok(Limited::Complete(b)) => Gitignore::parse(&String::from_utf8_lossy(&b)),
        _ => Gitignore::default(),
    };
    let ctx = Ctx {
        treatment,
        limits,
        gitignore,
    };
    let mut out = Vec::new();
    walk(vfs, &RelPath::root(), false, &ctx, &mut out)?;
    mark_collisions(&mut out);
    out.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    Ok(out)
}

fn too_many() -> DormbitError {
    DormbitError::new(
        ErrorCode::CapsuleLimitExceeded,
        "The project has more files than DORMBIT can plan in one capsule.",
    )
}

fn push(out: &mut Vec<Candidate>, c: Candidate) -> Result<(), DormbitError> {
    if out.len() >= MAX_PLAN_ITEMS {
        return Err(too_many());
    }
    out.push(c);
    Ok(())
}

fn kind_of(k: EntryKind) -> PlanItemKind {
    match k {
        EntryKind::File => PlanItemKind::File,
        EntryKind::Dir => PlanItemKind::Dir,
        EntryKind::Symlink => PlanItemKind::Symlink,
        EntryKind::Special => PlanItemKind::Special,
    }
}

fn walk<V: ReadVfs>(
    vfs: &V,
    dir: &RelPath,
    in_git: bool,
    ctx: &Ctx<'_>,
    out: &mut Vec<Candidate>,
) -> Result<(), DormbitError> {
    let mut entries = match vfs.read_dir(dir) {
        Ok(e) => e,
        Err(e) if dir.is_root() => return Err(e.to_error("the project folder")),
        Err(_) => {
            return push(
                out,
                Candidate {
                    path: dir.as_str().to_owned(),
                    kind: PlanItemKind::Dir,
                    size: 0,
                    reason: ReasonCode::Unreadable,
                    warning: None,
                },
            );
        }
    };
    entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    for entry in entries {
        let lossy_path = if dir.is_root() {
            entry.name.clone()
        } else {
            format!("{}/{}", dir.as_str(), entry.name)
        };
        let path = match (entry.name_ok, dir.join(&entry.name)) {
            (true, Ok(p)) => p,
            _ => {
                push(
                    out,
                    Candidate {
                        path: lossy_path,
                        kind: kind_of(entry.kind),
                        size: 0,
                        reason: ReasonCode::UnsafeName,
                        warning: None,
                    },
                )?;
                continue;
            }
        };
        let name = entry.name.as_str();

        if entry.kind == EntryKind::Dir {
            let is_git_root = !in_git && dir.is_root() && name == ".git";
            if in_git
                && (path.as_str() == ".git/hooks"
                    || (path.as_str().starts_with(".git/modules/") && name == "hooks"))
            {
                collapse(vfs, &path, ReasonCode::GitHooks, out)?;
            } else if in_git && name.starts_with("fsmonitor--daemon") {
                collapse(vfs, &path, ReasonCode::GitFsmonitor, out)?;
            } else if !in_git && REBUILDABLE_DIRS.contains(&name) {
                collapse(vfs, &path, ReasonCode::Rebuildable, out)?;
            } else {
                walk(vfs, &path, in_git || is_git_root, ctx, out)?;
            }
            continue;
        }

        let (kind, size, reason, warning) =
            classify_leaf(vfs, ctx, &path, name, entry.kind, in_git);
        push(
            out,
            Candidate {
                path: path.as_str().to_owned(),
                kind,
                size,
                reason,
                warning,
            },
        )?;
    }
    Ok(())
}

/// A folder shown and decided as one item. Its size is the sum of its files (links not followed).
fn collapse<V: ReadVfs>(
    vfs: &V,
    path: &RelPath,
    reason: ReasonCode,
    out: &mut Vec<Candidate>,
) -> Result<(), DormbitError> {
    let size = dir_size(vfs, path, 0);
    push(
        out,
        Candidate {
            path: path.as_str().to_owned(),
            kind: PlanItemKind::Dir,
            size,
            reason,
            warning: None,
        },
    )
}

fn dir_size<V: ReadVfs>(vfs: &V, dir: &RelPath, depth: u32) -> u64 {
    if depth > 64 {
        return 0;
    }
    let Ok(entries) = vfs.read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for e in entries {
        let Ok(p) = dir.join(&e.name) else { continue };
        match e.kind {
            EntryKind::File => {
                if let Ok(m) = vfs.symlink_metadata(&p) {
                    total = total.saturating_add(m.len);
                }
            }
            EntryKind::Dir => total = total.saturating_add(dir_size(vfs, &p, depth + 1)),
            EntryKind::Symlink | EntryKind::Special => {}
        }
    }
    total
}

fn classify_leaf<V: ReadVfs>(
    vfs: &V,
    ctx: &Ctx<'_>,
    path: &RelPath,
    name: &str,
    kind: EntryKind,
    in_git: bool,
) -> (PlanItemKind, u64, ReasonCode, Option<String>) {
    let item_kind = kind_of(kind);
    let done = |reason: ReasonCode, size: u64| (item_kind, size, reason, None);

    // 2: special files.
    if kind == EntryKind::Special {
        return done(ReasonCode::SpecialFile, 0);
    }
    // 3-6: sensitive by name, then by content for regular files.
    if let Some(r) = sensitive_name(name) {
        return done(r, 0);
    }
    if kind == EntryKind::File
        && let Ok(bytes) = vfs.read_prefix(path, KEY_PROBE)
        && sensitive_pem(&bytes)
    {
        return done(ReasonCode::SensitivePrivateKey, 0);
    }
    // 7-9: Git files that must never travel.
    if in_git {
        if path.as_str().starts_with(".git/modules/") && path.as_str().contains("/hooks/") {
            return done(ReasonCode::GitHooks, 0);
        }
        if name.ends_with(".lock") {
            return done(ReasonCode::GitLock, 0);
        }
        if name.starts_with("fsmonitor--daemon") {
            return done(ReasonCode::GitFsmonitor, 0);
        }
    }
    // 12: unreadable.
    let meta = match vfs.symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return done(ReasonCode::Unreadable, 0),
    };
    // 13: over the per-file limit.
    if kind == EntryKind::File && meta.len > ctx.limits.max_file_size {
        return done(ReasonCode::OverSizeLimit, meta.len);
    }
    // 14: symlinks are recorded, never followed.
    if kind == EntryKind::Symlink {
        return done(ReasonCode::SymlinkRecordOnly, meta.len);
    }
    let size = meta.len;
    // 17 and the Git treatment: every `.git` file, and the `.git` pointer file of a worktree.
    let git_file_at_root = path.as_str() == ".git";
    if in_git || git_file_at_root {
        let reason = if ctx.treatment == GitTreatment::WorkingTreeOnly {
            ReasonCode::GitWorkingTreeOnly
        } else if git_file_at_root {
            ReasonCode::ProjectFile
        } else {
            ReasonCode::GitInternal
        };
        let warning = git_file_at_root.then(|| "git_external_dir".to_owned());
        return (item_kind, size, reason, warning);
    }
    // 15: database files need a decision, 16: gitignored files that aren't rebuildable too.
    if is_data_file(name) {
        return done(ReasonCode::DataFile, size);
    }
    if ctx.gitignore.is_ignored(path.as_str(), false) {
        return done(ReasonCode::GitignoredNotRebuildable, size);
    }
    // 18: everything else.
    done(ReasonCode::ProjectFile, size)
}

/// Rule 10: two paths that are the same after NFC and case-folding can't both be restored on a
/// default macOS volume. The first (by path bytes) keeps its reason; the rest become collisions.
fn mark_collisions(items: &mut [Candidate]) {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, c) in items.iter().enumerate() {
        groups
            .entry(c.path.nfc().collect::<String>().to_lowercase())
            .or_default()
            .push(i);
    }
    for idxs in groups.values().filter(|g| g.len() > 1) {
        let mut sorted = idxs.clone();
        sorted.sort_by(|&a, &b| items[a].path.as_bytes().cmp(items[b].path.as_bytes()));
        for &i in &sorted[1..] {
            items[i].reason = ReasonCode::NameCollision;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::MemVfs;

    fn plan(v: &MemVfs) -> BTreeMap<String, ReasonCode> {
        classify_tree(v, GitTreatment::Full, &CapsuleLimits::default())
            .unwrap()
            .into_iter()
            .map(|c| (c.path, c.reason))
            .collect()
    }

    fn reason(v: &MemVfs, path: &str) -> ReasonCode {
        *plan(v)
            .get(path)
            .unwrap_or_else(|| panic!("{path} not in plan: {:?}", plan(v)))
    }

    #[test]
    fn sensitive_names_follow_the_table() {
        for (n, r) in [
            (".env", ReasonCode::SensitiveEnv),
            (".env.local", ReasonCode::SensitiveEnv),
            (".ENV", ReasonCode::SensitiveEnv),
            (".envrc", ReasonCode::SensitiveEnv),
            ("server.pem", ReasonCode::SensitiveCert),
            ("cert.P12", ReasonCode::SensitiveCert),
            ("id_rsa", ReasonCode::SensitivePrivateKey),
            ("id_ed25519", ReasonCode::SensitivePrivateKey),
            ("deploy.key", ReasonCode::SensitivePrivateKey),
            (".netrc", ReasonCode::SensitiveCredentials),
            (".npmrc", ReasonCode::SensitiveCredentials),
            ("credentials.json", ReasonCode::SensitiveCredentials),
            (".git-credentials", ReasonCode::SensitiveCredentials),
        ] {
            assert_eq!(sensitive_name(n), Some(r), "{n}");
        }
        for n in [
            ".env.example",
            ".env.sample",
            ".env.template",
            "id_rsa.pub",
            "environment.ts",
            "README.md",
            "keys.rs",
        ] {
            assert_eq!(sensitive_name(n), None, "{n}");
        }
    }

    #[test]
    fn a_pem_private_key_is_sensitive_whatever_its_name() {
        let mut v = MemVfs::new();
        v.add_file(
            "notes.txt",
            format!("-----BEGIN OPENSSH {}-----\nabc\n", "PRIVATE KEY"),
        );
        v.add_file("pub.txt", "-----BEGIN PUBLIC KEY-----\nabc\n");
        assert_eq!(reason(&v, "notes.txt"), ReasonCode::SensitivePrivateKey);
        assert_eq!(reason(&v, "pub.txt"), ReasonCode::ProjectFile);
    }

    #[test]
    fn a_pem_private_key_larger_than_the_probe_is_still_sensitive() {
        let mut v = MemVfs::new();
        let mut big = format!("-----BEGIN OPENSSH {}-----\n", "PRIVATE KEY").into_bytes();
        big.extend(std::iter::repeat_n(b'A', 3 * 1024 * 1024));
        v.add_file("archive.txt", big);
        assert_eq!(reason(&v, "archive.txt"), ReasonCode::SensitivePrivateKey);
    }

    #[test]
    fn precedence_sensitive_beats_rebuildable_data_and_gitignore() {
        let mut v = MemVfs::new();
        v.add_file(".gitignore", ".env\n*.pem\n")
            .add_file(".env", "A=1")
            .add_file("data/dev.sqlite", "x")
            .add_file("tls.pem", "x");
        assert_eq!(reason(&v, ".env"), ReasonCode::SensitiveEnv);
        assert_eq!(reason(&v, "tls.pem"), ReasonCode::SensitiveCert);
        assert_eq!(reason(&v, "data/dev.sqlite"), ReasonCode::DataFile);
    }

    #[test]
    fn rebuildable_folders_collapse_into_one_item_with_a_size() {
        let mut v = MemVfs::new();
        v.add_file("node_modules/a/index.js", vec![0u8; 10])
            .add_file("node_modules/b.js", vec![0u8; 5])
            .add_file("src/main.ts", "x");
        let items = classify_tree(&v, GitTreatment::Full, &CapsuleLimits::default()).unwrap();
        let nm = items.iter().find(|c| c.path == "node_modules").unwrap();
        assert_eq!(
            (nm.kind, nm.reason, nm.size),
            (PlanItemKind::Dir, ReasonCode::Rebuildable, 15)
        );
        assert!(items.iter().all(|c| !c.path.starts_with("node_modules/")));
    }

    #[test]
    fn symlinks_special_files_and_unreadable_files_have_their_own_reasons() {
        let mut v = MemVfs::new();
        v.add_symlink("link", "/etc/passwd")
            .add_special("pipe")
            .add_file("locked", "x")
            .add_unreadable("locked");
        assert_eq!(reason(&v, "link"), ReasonCode::SymlinkRecordOnly);
        assert_eq!(reason(&v, "pipe"), ReasonCode::SpecialFile);
        assert_eq!(reason(&v, "locked"), ReasonCode::Unreadable);
    }

    #[test]
    fn a_symlink_named_like_a_secret_is_still_sensitive() {
        let mut v = MemVfs::new();
        v.add_symlink(".env", "elsewhere");
        assert_eq!(reason(&v, ".env"), ReasonCode::SensitiveEnv);
    }

    #[test]
    fn files_over_the_size_limit_are_unsupported() {
        let mut v = MemVfs::new();
        v.add_file("big.bin", vec![0u8; 20])
            .add_file("small.bin", vec![0u8; 5]);
        let limits = CapsuleLimits {
            max_file_size: 10,
            ..CapsuleLimits::default()
        };
        let m: BTreeMap<_, _> = classify_tree(&v, GitTreatment::Full, &limits)
            .unwrap()
            .into_iter()
            .map(|c| (c.path, c.reason))
            .collect();
        assert_eq!(m["big.bin"], ReasonCode::OverSizeLimit);
        assert_eq!(m["small.bin"], ReasonCode::ProjectFile);
    }

    #[test]
    fn data_files_and_gitignored_files_need_a_decision() {
        let mut v = MemVfs::new();
        v.add_file(".gitignore", "local/\n*.cache\n")
            .add_file("db.sqlite3", "x")
            .add_file("dev.sqlite.bak", "x")
            .add_file("notes.bak", "x")
            .add_file("local/a.txt", "x")
            .add_file("x.cache", "x")
            .add_file("kept.txt", "x");
        assert_eq!(reason(&v, "db.sqlite3"), ReasonCode::DataFile);
        assert_eq!(reason(&v, "dev.sqlite.bak"), ReasonCode::DataFile);
        assert_eq!(reason(&v, "notes.bak"), ReasonCode::ProjectFile);
        assert_eq!(
            reason(&v, "local/a.txt"),
            ReasonCode::GitignoredNotRebuildable
        );
        assert_eq!(reason(&v, "x.cache"), ReasonCode::GitignoredNotRebuildable);
        assert_eq!(reason(&v, "kept.txt"), ReasonCode::ProjectFile);
    }

    fn git_tree() -> MemVfs {
        let mut v = MemVfs::new();
        v.add_file("src/a.rs", "x")
            .add_file(".git/HEAD", "ref: refs/heads/main\n")
            .add_file(".git/config", "[core]\n")
            .add_file(".git/objects/ab/cdef", "x")
            .add_file(".git/hooks/pre-commit", "#!/bin/sh\n")
            .add_file(".git/index.lock", "")
            .add_file(".git/modules/sub/hooks/post-merge", "x")
            .add_file(".git/modules/sub/config", "x")
            .add_special(".git/fsmonitor--daemon.ipc");
        v
    }

    #[test]
    fn git_treatment_full_keeps_internals_but_drops_hooks_locks_and_sockets() {
        let v = git_tree();
        assert_eq!(reason(&v, ".git/HEAD"), ReasonCode::GitInternal);
        assert_eq!(reason(&v, ".git/objects/ab/cdef"), ReasonCode::GitInternal);
        assert_eq!(reason(&v, ".git/hooks"), ReasonCode::GitHooks);
        assert!(
            !plan(&v).contains_key(".git/hooks/pre-commit"),
            "hooks collapse into one item"
        );
        assert_eq!(reason(&v, ".git/index.lock"), ReasonCode::GitLock);
        assert_eq!(reason(&v, ".git/modules/sub/hooks"), ReasonCode::GitHooks);
        assert_eq!(
            reason(&v, ".git/modules/sub/config"),
            ReasonCode::GitInternal
        );
        assert_eq!(
            reason(&v, ".git/fsmonitor--daemon.ipc"),
            ReasonCode::SpecialFile
        );
    }

    #[test]
    fn git_treatment_working_tree_only_excludes_every_git_item() {
        let v = git_tree();
        let m: BTreeMap<_, _> =
            classify_tree(&v, GitTreatment::WorkingTreeOnly, &CapsuleLimits::default())
                .unwrap()
                .into_iter()
                .map(|c| (c.path, c.reason))
                .collect();
        assert_eq!(m[".git/HEAD"], ReasonCode::GitWorkingTreeOnly);
        assert_eq!(m[".git/objects/ab/cdef"], ReasonCode::GitWorkingTreeOnly);
        assert_eq!(
            m[".git/hooks"],
            ReasonCode::GitHooks,
            "rules 7-9 still apply"
        );
        assert_eq!(m["src/a.rs"], ReasonCode::ProjectFile);
    }

    #[test]
    fn a_git_file_is_a_project_file_with_a_warning() {
        let mut v = MemVfs::new();
        v.add_file(".git", "gitdir: ../real/.git/worktrees/x\n");
        let items = classify_tree(&v, GitTreatment::Full, &CapsuleLimits::default()).unwrap();
        assert_eq!(
            (items[0].reason, items[0].warning.as_deref()),
            (ReasonCode::ProjectFile, Some("git_external_dir"))
        );
    }

    #[test]
    fn unsafe_names_are_unsupported_and_never_become_paths() {
        let mut v = MemVfs::new();
        v.add_bad_name("", "bad\u{fffd}name")
            .add_file("ok.txt", "x");
        let items = classify_tree(&v, GitTreatment::Full, &CapsuleLimits::default()).unwrap();
        let bad = items
            .iter()
            .find(|c| c.reason == ReasonCode::UnsafeName)
            .expect("unsafe name item");
        assert!(bad.path.contains('\u{fffd}'));
        assert!(
            items
                .iter()
                .any(|c| c.path == "ok.txt" && c.reason == ReasonCode::ProjectFile)
        );
    }

    #[test]
    fn nfc_and_case_collisions_keep_the_first_path_only() {
        let mut v = MemVfs::new();
        v.add_file("caf\u{e9}.txt", "a")
            .add_file("cafe\u{301}.txt", "b")
            .add_file("README.md", "c")
            .add_file("readme.md", "d");
        let m = plan(&v);
        let nfc = m["caf\u{e9}.txt"];
        let nfd = m["cafe\u{301}.txt"];
        assert_eq!(
            [nfc, nfd]
                .iter()
                .filter(|r| **r == ReasonCode::NameCollision)
                .count(),
            1
        );
        assert_eq!(
            m["README.md"],
            ReasonCode::ProjectFile,
            "uppercase sorts first and keeps its reason"
        );
        assert_eq!(m["readme.md"], ReasonCode::NameCollision);
    }

    #[test]
    fn output_is_sorted_by_path_bytes_and_stable() {
        let mut v = MemVfs::new();
        v.add_file("b", "1")
            .add_file("a/z", "1")
            .add_file("a/b", "1")
            .add_file("B", "1");
        let paths: Vec<_> = classify_tree(&v, GitTreatment::Full, &CapsuleLimits::default())
            .unwrap()
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(paths, ["B", "a/b", "a/z", "b"]);
    }
}
