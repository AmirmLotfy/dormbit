//! The rule registry and the rules (design: Rule list). Each rule has one allowed status set
//! and feeds one brief section.

use super::tasks::count_tasks;
use super::{EvidenceInput, EvidenceRef, FileIndex, Sink, Span, StatKind, Status, clip};
use crate::plan::ReasonCode;
use crate::plan::secrets::{Scan, scan_secrets};
use crate::vfs::ReadVfs;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SectionId {
    Identity,
    Technologies,
    RepoState,
    Docs,
    VerifiedContent,
    UnfinishedWork,
    NextSteps,
    ContextReferences,
    MissingInformation,
    FirstSteps,
    PreRunChecks,
}

/// Section order and titles (Requirement 12.2).
pub const SECTIONS: [(SectionId, &str); 11] = [
    (SectionId::Identity, "Identity"),
    (SectionId::Technologies, "Technologies"),
    (SectionId::RepoState, "Repo state"),
    (SectionId::Docs, "Docs"),
    (SectionId::VerifiedContent, "Verified content"),
    (SectionId::UnfinishedWork, "Unfinished work"),
    (SectionId::NextSteps, "Evidence-backed next steps"),
    (SectionId::ContextReferences, "Context references"),
    (SectionId::MissingInformation, "Missing information"),
    (SectionId::FirstSteps, "First steps"),
    (SectionId::PreRunChecks, "Pre-run checks"),
];

pub struct RuleSpec {
    pub id: &'static str,
    pub allowed: &'static [Status],
    pub section: SectionId,
}

const C: &[Status] = &[Status::Confirmed];
const I: &[Status] = &[Status::Inferred];
const U: &[Status] = &[Status::Unknown];
const UNS: &[Status] = &[Status::Unsupported];

use SectionId::*;

pub const REGISTRY: &[RuleSpec] = &[
    RuleSpec {
        id: "identity.name",
        allowed: C,
        section: Identity,
    },
    RuleSpec {
        id: "stack.manifest",
        allowed: C,
        section: Technologies,
    },
    RuleSpec {
        id: "stack.dependency",
        allowed: C,
        section: Technologies,
    },
    RuleSpec {
        id: "stack.framework",
        allowed: I,
        section: Technologies,
    },
    RuleSpec {
        id: "toolchain.file",
        allowed: C,
        section: Technologies,
    },
    RuleSpec {
        id: "git.branch",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "git.detached",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "git.feature_branch",
        allowed: I,
        section: RepoState,
    },
    RuleSpec {
        id: "git.uncommitted",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "git.wip_commit",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "git.stash",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "git.upstream",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "git.absent",
        allowed: C,
        section: RepoState,
    },
    RuleSpec {
        id: "docs.present",
        allowed: C,
        section: Docs,
    },
    RuleSpec {
        id: "restore.verified",
        allowed: C,
        section: VerifiedContent,
    },
    RuleSpec {
        id: "restore.not_restored",
        allowed: C,
        section: VerifiedContent,
    },
    RuleSpec {
        id: "restore.git_match",
        allowed: C,
        section: VerifiedContent,
    },
    RuleSpec {
        id: "restore.none",
        allowed: U,
        section: VerifiedContent,
    },
    RuleSpec {
        id: "kiro.tasks_progress",
        allowed: C,
        section: UnfinishedWork,
    },
    RuleSpec {
        id: "kiro.task_unchecked",
        allowed: C,
        section: UnfinishedWork,
    },
    RuleSpec {
        id: "work.todo",
        allowed: C,
        section: UnfinishedWork,
    },
    RuleSpec {
        id: "work.skipped_test",
        allowed: C,
        section: UnfinishedWork,
    },
    RuleSpec {
        id: "work.area",
        allowed: I,
        section: UnfinishedWork,
    },
    RuleSpec {
        id: "kiro.steering",
        allowed: C,
        section: ContextReferences,
    },
    RuleSpec {
        id: "agent.instructions",
        allowed: C,
        section: ContextReferences,
    },
    RuleSpec {
        id: "adapter.session",
        allowed: C,
        section: ContextReferences,
    },
    RuleSpec {
        id: "env.example_key",
        allowed: C,
        section: MissingInformation,
    },
    RuleSpec {
        id: "sensitive.present",
        allowed: C,
        section: MissingInformation,
    },
    RuleSpec {
        id: "unknown.pause_reason",
        allowed: U,
        section: MissingInformation,
    },
    RuleSpec {
        id: "unknown.builds",
        allowed: U,
        section: MissingInformation,
    },
    RuleSpec {
        id: "unknown.deploy_target",
        allowed: U,
        section: MissingInformation,
    },
    RuleSpec {
        id: "scan.skipped_files",
        allowed: C,
        section: MissingInformation,
    },
    RuleSpec {
        id: "scan.truncated",
        allowed: C,
        section: MissingInformation,
    },
    RuleSpec {
        id: "source.unsupported",
        allowed: UNS,
        section: MissingInformation,
    },
    RuleSpec {
        id: "cmd.documented",
        allowed: C,
        section: FirstSteps,
    },
    RuleSpec {
        id: "prerun.lifecycle_script",
        allowed: C,
        section: PreRunChecks,
    },
    RuleSpec {
        id: "prerun.editor_autorun",
        allowed: C,
        section: PreRunChecks,
    },
    RuleSpec {
        id: "prerun.git_risky_config",
        allowed: C,
        section: PreRunChecks,
    },
    RuleSpec {
        id: "prerun.none",
        allowed: C,
        section: PreRunChecks,
    },
];

pub fn spec(id: &str) -> Option<&'static RuleSpec> {
    REGISTRY.iter().find(|r| r.id == id)
}

pub fn section_of(rule: &str) -> Option<SectionId> {
    spec(rule).map(|s| s.section)
}

pub(crate) fn sort_key(rule: &str) -> (SectionId, usize) {
    let i = REGISTRY
        .iter()
        .position(|r| r.id == rule)
        .unwrap_or(REGISTRY.len());
    (section_of(rule).unwrap_or(PreRunChecks), i)
}

fn t(s: &str) -> Span {
    Span::Text(s.to_owned())
}
/// Untrusted text as a code span, hidden entirely if it looks like a secret (R5.5).
fn code(s: &str) -> Span {
    match scan_secrets(s.as_bytes()) {
        Scan::Hit(_) => Span::Text(HIDDEN.into()),
        _ => Span::Code(clip(s)),
    }
}

pub(crate) const HIDDEN: &str = "[hidden: looks like a secret]";
fn lines(path: &str, line: u32, anchor: &str) -> EvidenceRef {
    EvidenceRef::FileLines {
        path: path.to_owned(),
        start_line: line,
        end_line: line,
        anchor: Some(anchor.to_owned()),
    }
}
fn key(path: &str, pointer: &str) -> EvidenceRef {
    EvidenceRef::ManifestKey {
        path: path.to_owned(),
        pointer: pointer.to_owned(),
    }
}
fn root() -> EvidenceRef {
    EvidenceRef::FileStat {
        path: String::new(),
        stat: StatKind::Dir,
        size: None,
    }
}

/// Untrusted text for a quote, replaced entirely if it looks like a secret.
fn quote(s: &str) -> Span {
    match scan_secrets(s.as_bytes()) {
        Scan::Hit(_) => Span::Text(HIDDEN.into()),
        _ => Span::Quote(clip(s)),
    }
}

fn short(oid: &str) -> String {
    oid.chars().take(12).collect()
}

/// RFC 6901 escaping for one pointer token.
fn ptr(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// npm package → label (design: stack detection table).
const NPM_LABELS: [(&str, &str); 20] = [
    ("react", "React"),
    ("vite", "Vite"),
    ("typescript", "TypeScript"),
    ("fastify", "Fastify"),
    ("drizzle-orm", "Drizzle ORM"),
    ("better-sqlite3", "SQLite (better-sqlite3)"),
    ("vitest", "Vitest"),
    ("express", "Express"),
    ("next", "Next.js"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("tailwindcss", "Tailwind CSS"),
    ("jest", "Jest"),
    ("prisma", "Prisma"),
    ("@prisma/client", "Prisma"),
    ("electron", "Electron"),
    ("@tauri-apps/api", "Tauri"),
    ("hono", "Hono"),
    ("astro", "Astro"),
    ("@angular/core", "Angular"),
];

const MANIFESTS: [(&str, &str); 10] = [
    ("package.json", "Node.js package"),
    ("Cargo.toml", "Rust crate"),
    ("pyproject.toml", "Python project"),
    ("requirements.txt", "Python requirements"),
    ("go.mod", "Go module"),
    ("Gemfile", "Ruby bundle"),
    ("pom.xml", "Maven project"),
    ("build.gradle", "Gradle project"),
    ("composer.json", "PHP Composer project"),
    ("Package.swift", "Swift package"),
];

const SOURCE_EXT: [&str; 22] = [
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "rs", "py", "go", "rb", "java", "kt", "swift", "c",
    "h", "cpp", "cs", "php", "vue", "svelte", "css", "scss",
];

fn is_source(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| SOURCE_EXT.contains(&ext))
}

const TODO_TOKENS: [&str; 4] = ["TODO", "FIXME", "HACK", "XXX"];
const JS_SKIP: [&str; 6] = [
    "it.skip(",
    "test.skip(",
    "describe.skip(",
    "xit(",
    "xtest(",
    "xdescribe(",
];
const PY_SKIP: [&str; 3] = ["@pytest.mark.skip", "@unittest.skip", "@skip("];
const RS_SKIP: [&str; 1] = ["#[ignore"];

/// The skip markers that apply to a file, by extension (R7.2).
fn skip_markers(path: &str) -> &'static [&'static str] {
    match path.rsplit_once('.').map(|(_, e)| e) {
        Some("ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs") => &JS_SKIP,
        Some("py") => &PY_SKIP,
        Some("rs") => &RS_SKIP,
        _ => &[],
    }
}

/// A skip marker in `line` that starts at a word boundary (so `xit(` never matches `exit(`).
fn find_skip(line: &str, markers: &'static [&'static str]) -> Option<(&'static str, usize)> {
    for m in markers {
        let mut from = 0;
        while let Some(pos) = line[from..].find(m) {
            let at = from + pos;
            let boundary = !line[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
            if boundary {
                return Some((m, at));
            }
            from = at + m.len();
        }
    }
    None
}

/// A TODO-style token used as a marker: preceded by a non-word character and followed by
/// `:`, `(`, a space, or the end of the line.
fn find_token(line: &str) -> Option<(&'static str, usize)> {
    for tok in TODO_TOKENS {
        let mut from = 0;
        while let Some(pos) = line[from..].find(tok) {
            let at = from + pos;
            let before_ok = at == 0
                || !line[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let after = &line[at + tok.len()..];
            // A marker is `TODO:` / `TODO(…)`, or a token that opens a comment
            // (`// TODO fix`). A token mentioned mid-sentence ("see the FIXME in …") isn't one.
            let before = line[..at].trim_end();
            let opens_comment = ["//", "#", "/*", "*", "--", "<!--"]
                .iter()
                .any(|o| before.ends_with(o));
            let after_ok = after.starts_with([':', '('])
                || (opens_comment && (after.is_empty() || after.starts_with(' ')));
            if before_ok && after_ok {
                return Some((tok, at));
            }
            from = at + tok.len();
        }
    }
    None
}

fn first_quoted(s: &str) -> Option<&str> {
    for q in ['"', '\'', '`'] {
        if let Some(start) = s.find(q)
            && let Some(len) = s[start + 1..].find(q)
        {
            return Some(&s[start + 1..start + 1 + len]);
        }
    }
    None
}

pub(crate) fn run_all<V: ReadVfs>(input: &EvidenceInput<'_, V>, ix: &FileIndex, s: &mut Sink) {
    let pkg: Option<serde_json::Value> = ix
        .text("package.json")
        .and_then(|t| serde_json::from_str(t).ok());
    // A manifest that exists but couldn't be read or parsed is reported, never ignored.
    let mut unreadable_sources = 0;
    for f in ["package.json", ".vscode/tasks.json"] {
        let parsed_ok = match ix.text(f) {
            Some(t) if f == "package.json" => pkg.is_some() || t.trim().is_empty(),
            Some(_) => true,
            None => false,
        };
        if ix.has(f) && !parsed_ok {
            unreadable_sources += 1;
            s.emit(
                "source.unsupported",
                Status::Unsupported,
                vec![
                    code(f),
                    t(" exists but couldn't be read or parsed, so its facts are unknown."),
                ],
                vec![ix.stat(f)],
            );
        }
    }

    // identity.name
    match pkg
        .as_ref()
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
    {
        Some(name) => {
            s.emit(
                "identity.name",
                Status::Confirmed,
                vec![
                    t("Project name: "),
                    code(name),
                    t(" (from `package.json`)."),
                ],
                vec![key("package.json", "/name")],
            );
        }
        None => {
            s.emit(
                "identity.name",
                Status::Confirmed,
                vec![
                    t("Project name: "),
                    code(input.folder_name),
                    t(" (the folder name)."),
                ],
                vec![root()],
            );
        }
    }

    // stack.manifest, stack.dependency, stack.framework
    for (file, label) in MANIFESTS {
        if ix.has(file) {
            s.emit(
                "stack.manifest",
                Status::Confirmed,
                vec![code(file), t(&format!(": {label}."))],
                vec![ix.stat(file)],
            );
        }
    }
    let mut labels: Vec<(&str, EvidenceRef)> = Vec::new();
    if let Some(p) = &pkg {
        for section in ["dependencies", "devDependencies"] {
            let Some(deps) = p.get(section).and_then(|d| d.as_object()) else {
                continue;
            };
            for (name, ver) in deps {
                if let Some((_, label)) = NPM_LABELS.iter().find(|(n, _)| n == name) {
                    let ev = key("package.json", &format!("/{section}/{}", ptr(name)));
                    let v = ver.as_str().unwrap_or("");
                    s.emit(
                        "stack.dependency",
                        Status::Confirmed,
                        vec![
                            t(&format!("{label}: ")),
                            code(name),
                            t(" "),
                            code(v),
                            t(&format!(" in `{section}`.")),
                        ],
                        vec![ev.clone()],
                    );
                    labels.push((label, ev));
                }
            }
        }
    }
    let has = |l: &str| labels.iter().any(|(x, _)| *x == l);
    let refs = |ls: &[&str]| -> Vec<EvidenceRef> {
        labels
            .iter()
            .filter(|(x, _)| ls.contains(x))
            .map(|(_, e)| e.clone())
            .collect()
    };
    if has("React") && has("Vite") {
        s.emit(
            "stack.framework",
            Status::Inferred,
            vec![t("React + Vite frontend (inferred from its dependencies).")],
            refs(&["React", "Vite"]),
        );
    }
    if has("Fastify") {
        let mut ev = refs(&["Fastify"]);
        let mut st = vec![t("Fastify API server")];
        if has("Drizzle ORM") {
            ev.extend(refs(&["Drizzle ORM"]));
            st.push(t(" with Drizzle ORM"));
        }
        st.push(t(" (inferred from its dependencies)."));
        s.emit("stack.framework", Status::Inferred, st, ev);
    }

    // toolchain.file
    for (file, what) in [
        (".nvmrc", "Node"),
        (".node-version", "Node"),
        (".python-version", "Python"),
        (".ruby-version", "Ruby"),
    ] {
        if let Some(text) = ix.text(file)
            && let Some(v) = text.lines().next().map(str::trim).filter(|v| !v.is_empty())
        {
            s.emit(
                "toolchain.file",
                Status::Confirmed,
                vec![
                    t(&format!("{what} ")),
                    code(v),
                    t(" (from "),
                    code(file),
                    t(")."),
                ],
                vec![lines(file, 1, v)],
            );
        }
    }
    if let Some(text) = ix.text("rust-toolchain.toml") {
        for (i, l) in text.lines().enumerate() {
            if let Some(v) = l.trim().strip_prefix("channel") {
                let v = v.trim_start_matches([' ', '=']).trim().trim_matches('"');
                s.emit(
                    "toolchain.file",
                    Status::Confirmed,
                    vec![t("Rust "), code(v), t(" (from `rust-toolchain.toml`).")],
                    vec![lines("rust-toolchain.toml", (i + 1) as u32, "channel")],
                );
            }
        }
    }
    if let Some(engines) = pkg
        .as_ref()
        .and_then(|p| p.get("engines"))
        .and_then(|e| e.as_object())
    {
        for (name, v) in engines {
            s.emit(
                "toolchain.file",
                Status::Confirmed,
                vec![
                    code(name),
                    t(" "),
                    code(v.as_str().unwrap_or("")),
                    t(" (from `package.json` engines)."),
                ],
                vec![key("package.json", &format!("/engines/{}", ptr(name)))],
            );
        }
    }

    // cmd.documented and prerun.lifecycle_script
    let mut hazards = 0;
    if let Some(scripts) = pkg
        .as_ref()
        .and_then(|p| p.get("scripts"))
        .and_then(|x| x.as_object())
    {
        for (name, cmd) in scripts {
            let ev = key("package.json", &format!("/scripts/{}", ptr(name)));
            let c = cmd.as_str().unwrap_or("");
            if ["preinstall", "install", "postinstall", "prepare"].contains(&name.as_str()) {
                hazards += 1;
                s.emit(
                    "prerun.lifecycle_script",
                    Status::Confirmed,
                    vec![
                        t("Lifecycle script "),
                        code(name),
                        t(" runs "),
                        code(c),
                        t(" on install. Review it before installing."),
                    ],
                    vec![ev.clone()],
                );
            }
            s.emit(
                "cmd.documented",
                Status::Confirmed,
                vec![
                    t("Documented command "),
                    code(&format!("npm run {name}")),
                    t(": "),
                    code(c),
                    t(". DORMBIT hasn't run it."),
                ],
                vec![ev],
            );
        }
    }
    if let Some(text) = ix.text(".vscode/tasks.json") {
        for (i, l) in text.lines().enumerate() {
            if l.contains("folderOpen") {
                hazards += 1;
                s.emit(
                    "prerun.editor_autorun",
                    Status::Confirmed,
                    vec![
                        t("An editor task in "),
                        code(".vscode/tasks.json"),
                        t(" runs when the folder opens."),
                    ],
                    vec![lines(".vscode/tasks.json", (i + 1) as u32, "folderOpen")],
                );
            }
        }
    }
    if let Some(g) = input.git {
        for k in &g.risky_config {
            hazards += 1;
            s.emit(
                "prerun.git_risky_config",
                Status::Confirmed,
                vec![
                    t("Git config key "),
                    code(k),
                    t(" can run commands. Review it before using Git in this folder."),
                ],
                vec![EvidenceRef::GitConfigKey { key: k.clone() }],
            );
        }
    }
    // Only claim "none" for what was actually checked, and only if every checked source
    // could be read.
    if hazards == 0 && unreadable_sources == 0 {
        let mut ev = vec![ix.stat("package.json"), ix.stat(".vscode/tasks.json")];
        let mut st = vec![t(
            "No lifecycle scripts in `package.json` and no folder-open tasks in `.vscode/tasks.json`",
        )];
        if input.git.is_some() && ix.has(".git/config") {
            ev.push(ix.stat(".git/config"));
            st.push(t(", and no risky Git config keys in the Git snapshot"));
        }
        st.push(t(" were found."));
        s.emit("prerun.none", Status::Confirmed, st, ev);
    }

    // env.example_key, sensitive.present
    for file in [".env.example", ".env.sample", ".env.template"] {
        if let Some(text) = ix.text(file) {
            for (i, l) in text.lines().enumerate() {
                let l = l.trim();
                if l.starts_with('#') {
                    continue;
                }
                if let Some((k, _)) = l.split_once('=') {
                    let k = k.trim().trim_start_matches("export ").trim();
                    if !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                        s.emit(
                            "env.example_key",
                            Status::Confirmed,
                            vec![
                                code(k),
                                t(" is listed in "),
                                code(file),
                                t("; its value needs to be supplied."),
                            ],
                            vec![lines(file, (i + 1) as u32, k)],
                        );
                    }
                }
            }
        }
    }
    for (p, reason, _) in &ix.entries {
        if matches!(
            reason,
            ReasonCode::SensitiveEnv
                | ReasonCode::SensitiveCert
                | ReasonCode::SensitivePrivateKey
                | ReasonCode::SensitiveCredentials
        ) {
            s.emit(
                "sensitive.present",
                Status::Confirmed,
                vec![
                    code(p),
                    t(" exists. It wasn't read and isn't in a capsule."),
                ],
                vec![EvidenceRef::FileStat {
                    path: p.clone(),
                    stat: StatKind::File,
                    size: None,
                }],
            );
        }
    }

    // Git facts from the repository files and the snapshot.
    let refs_found = crate::gitref::read_refs(input.vfs);
    let mut branch_name: Option<String> = None;
    match &refs_found.head {
        None => {
            let git_present = ix
                .entries
                .iter()
                .any(|(p, _, _)| p == ".git" || p.starts_with(".git/"));
            if git_present {
                // `.git` exists (folder or worktree file) but HEAD couldn't be read.
                s.emit(
                    "source.unsupported",
                    Status::Unsupported,
                    vec![t("A "), code(".git"), t(" entry exists, but Git HEAD couldn't be read, so the repo state is unknown.")],
                    vec![EvidenceRef::FileStat {
                        path: ".git".into(),
                        stat: StatKind::File,
                        size: None,
                    }],
                );
            } else {
                s.emit(
                    "git.absent",
                    Status::Confirmed,
                    vec![t("No Git repository was found in the project folder.")],
                    vec![EvidenceRef::FileStat {
                        path: ".git".into(),
                        stat: StatKind::Absent,
                        size: None,
                    }],
                );
            }
        }
        Some(h) if h.detached => {
            if let Some(oid) = h.commit.clone().filter(|o| !o.is_empty()) {
                s.emit(
                    "git.detached",
                    Status::Confirmed,
                    vec![t("HEAD is detached at "), code(&short(&oid)), t(".")],
                    vec![EvidenceRef::GitCommit { oid }],
                );
            }
        }
        Some(h) => {
            if let Some(b) = &h.branch {
                branch_name = Some(b.clone());
                let mut st = vec![t("Checked-out branch: "), code(b)];
                if let Some(c) = &h.commit {
                    st.push(t(" at "));
                    st.push(code(&short(c)));
                }
                st.push(t("."));
                let id = s.emit(
                    "git.branch",
                    Status::Confirmed,
                    st,
                    vec![EvidenceRef::GitRef {
                        name: format!("refs/heads/{b}"),
                    }],
                );
                if id.is_some() && !["main", "master", "develop", "trunk"].contains(&b.as_str()) {
                    s.emit(
                        "git.feature_branch",
                        Status::Inferred,
                        vec![
                            t("Work was happening on a feature branch, "),
                            code(b),
                            t(" (inferred from the branch name)."),
                        ],
                        vec![EvidenceRef::GitRef {
                            name: format!("refs/heads/{b}"),
                        }],
                    );
                }
            }
        }
    }
    if let Some(g) = input.git {
        let st = &g.status;
        let (m, sg, u) = (st.modified.get(), st.staged.get(), st.untracked.get());
        if m + sg + u > 0 {
            s.emit(
                "git.uncommitted",
                Status::Confirmed,
                vec![
                    t("Uncommitted changes at scan time: "),
                    Span::Count(m),
                    t(" modified, "),
                    Span::Count(sg),
                    t(" staged, "),
                    Span::Count(u),
                    t(" untracked."),
                ],
                vec![EvidenceRef::GitStatus {
                    modified: m,
                    staged: sg,
                    untracked: u,
                }],
            );
        }
        let summary = g.head.summary.trim();
        // `wip` as a whole word: followed by the end or a non-letter (R8.5).
        let lower = summary.to_ascii_lowercase();
        let wip = lower
            .strip_prefix("wip")
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphabetic()));
        if wip {
            s.emit(
                "git.wip_commit",
                Status::Confirmed,
                vec![
                    t("The last commit is marked work in progress: "),
                    quote(summary),
                    t("."),
                ],
                vec![EvidenceRef::GitCommit {
                    oid: g.head.commit.clone(),
                }],
            );
        }
        if g.stash_count.get() > 0 {
            s.emit(
                "git.stash",
                Status::Confirmed,
                vec![
                    t("There "),
                    t(if g.stash_count.get() == 1 {
                        "is "
                    } else {
                        "are "
                    }),
                    Span::Count(g.stash_count.get()),
                    t(if g.stash_count.get() == 1 {
                        " stash entry."
                    } else {
                        " stash entries."
                    }),
                ],
                vec![EvidenceRef::GitRef {
                    name: "refs/stash".into(),
                }],
            );
        }
        if let Some(up) = &g.upstream {
            s.emit(
                "git.upstream",
                Status::Confirmed,
                vec![
                    Span::Count(up.ahead.get()),
                    t(" commits ahead and "),
                    Span::Count(up.behind.get()),
                    t(" behind the upstream, as of the last fetch."),
                ],
                vec![EvidenceRef::GitRef {
                    name: "upstream".into(),
                }],
            );
        }
    }

    // docs.present
    for doc in ["README.md", "README", "CHANGELOG.md", "CONTRIBUTING.md"] {
        if ix.has(doc) {
            let title = ix.text(doc).and_then(|t| {
                t.lines()
                    .position(|l| l.starts_with("# "))
                    .map(|i| (i, t.lines().nth(i).unwrap_or("")[2..].trim().to_owned()))
            });
            match title {
                Some((i, h1)) => s.emit(
                    "docs.present",
                    Status::Confirmed,
                    vec![code(doc), t(": "), quote(&h1), t(".")],
                    vec![lines(doc, (i + 1) as u32, "# ")],
                ),
                None => s.emit(
                    "docs.present",
                    Status::Confirmed,
                    vec![code(doc), t(" is present.")],
                    vec![ix.stat(doc)],
                ),
            };
        }
    }
    for (p, _, _) in &ix.entries {
        if p.starts_with("docs/") && p.ends_with(".md") {
            s.emit(
                "docs.present",
                Status::Confirmed,
                vec![code(p), t(" is present.")],
                vec![ix.stat(p)],
            );
        }
    }

    // kiro.tasks_progress, kiro.task_unchecked, kiro.steering, agent.instructions
    for tf in &ix.texts {
        let parts: Vec<&str> = tf.path.split('/').collect();
        if parts.len() == 4 && parts[0] == ".kiro" && parts[1] == "specs" && parts[3] == "tasks.md"
        {
            let c = count_tasks(&tf.text);
            if c.total == 0 {
                continue;
            }
            s.emit(
                "kiro.tasks_progress",
                Status::Confirmed,
                vec![
                    code(&tf.path),
                    t(": "),
                    Span::Count(u64::from(c.done)),
                    t(" of "),
                    Span::Count(u64::from(c.total)),
                    t(" tasks complete."),
                ],
                vec![EvidenceRef::FileLines {
                    path: tf.path.clone(),
                    start_line: c.first_line,
                    end_line: c.last_line,
                    anchor: Some("- [".into()),
                }],
            );
            for (line, title) in &c.unchecked {
                s.emit(
                    "kiro.task_unchecked",
                    Status::Confirmed,
                    vec![t("Unchecked task: "), quote(title)],
                    vec![lines(&tf.path, *line, "- [")],
                );
            }
        }
    }
    for (p, _, _) in &ix.entries {
        if p.starts_with(".kiro/steering/") && p.ends_with(".md") {
            s.emit(
                "kiro.steering",
                Status::Confirmed,
                vec![t("Kiro steering file "), code(p), t(".")],
                vec![ix.stat(p)],
            );
        }
    }
    for f in [
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".cursorrules",
        ".github/copilot-instructions.md",
    ] {
        if ix.has(f) {
            s.emit(
                "agent.instructions",
                Status::Confirmed,
                vec![t("Agent instructions in "), code(f), t(".")],
                vec![ix.stat(f)],
            );
        }
    }
    for a in input.agent_sessions {
        let mut st = vec![
            // The label comes from a fixed table, never from the host or the store.
            t(match a.adapter.as_str() {
                "claude_code" => "Claude Code session ",
                "codex" => "Codex session ",
                "kiro_export" => "Kiro export session ",
                _ => "Agent session ",
            }),
            code(&a.session_id),
        ];
        if let Some(e) = &a.ended_at {
            st.push(t(", last active "));
            st.push(code(&e.chars().take(10).collect::<String>()));
        }
        if let Some(b) = &a.git_branch {
            st.push(t(", branch "));
            st.push(code(b));
        }
        st.push(t(". Not resumed by DORMBIT."));
        s.emit(
            "adapter.session",
            Status::Confirmed,
            st,
            vec![EvidenceRef::AgentSession {
                adapter: a.adapter.clone(),
                session_id: a.session_id.clone(),
            }],
        );
    }

    // work.todo, work.skipped_test
    let mut area_refs: Vec<EvidenceRef> = Vec::new();
    let area_tokens: Vec<String> = branch_name
        .as_deref()
        .map(|b| {
            b.rsplit('/')
                .next()
                .unwrap_or(b)
                .split(['-', '_'])
                .filter(|x| x.len() >= 3)
                .map(str::to_ascii_lowercase)
                .collect()
        })
        .unwrap_or_default();
    let in_area = |path: &str| {
        !area_tokens.is_empty()
            && area_tokens
                .iter()
                .any(|tk| path.to_ascii_lowercase().contains(tk.as_str()))
    };
    for tf in ix.texts.iter().filter(|t| is_source(&t.path)) {
        for (i, l) in tf.text.lines().enumerate() {
            let n = (i + 1) as u32;
            if let Some((tok, at)) = find_token(l) {
                let rest = l[at + tok.len()..]
                    .trim_start_matches([':', ' ', '('])
                    .trim();
                let ev = lines(&tf.path, n, tok);
                let mut st = vec![t(&format!("{tok} at ")), code(&format!("{}:{n}", tf.path))];
                if !rest.is_empty() {
                    st.push(t(": "));
                    st.push(quote(rest));
                }
                if s.emit("work.todo", Status::Confirmed, st, vec![ev.clone()])
                    .is_some()
                    && in_area(&tf.path)
                {
                    area_refs.push(ev);
                }
            }
            if let Some((m, at)) = find_skip(l, skip_markers(&tf.path)) {
                let ev = lines(&tf.path, n, m);
                let mut st = vec![t("Skipped test at "), code(&format!("{}:{n}", tf.path))];
                if let Some(name) = first_quoted(&l[at..]) {
                    st.push(t(": "));
                    st.push(quote(name));
                }
                if s.emit("work.skipped_test", Status::Confirmed, st, vec![ev.clone()])
                    .is_some()
                    && in_area(&tf.path)
                {
                    area_refs.push(ev);
                }
            }
        }
    }

    // work.area: groups facts that share a word with the feature branch.
    if let Some(b) = &branch_name
        && !area_tokens.is_empty()
    {
        let tasks_in_area: Vec<EvidenceRef> = ix
            .texts
            .iter()
            .filter(|t| {
                t.path.starts_with(".kiro/specs/")
                    && t.path.ends_with("/tasks.md")
                    && in_area(&t.path)
            })
            .flat_map(|t| {
                count_tasks(&t.text)
                    .unchecked
                    .into_iter()
                    .map(move |(n, _)| lines(&t.path, n, "- ["))
            })
            .collect();
        let (nt, nw) = (tasks_in_area.len() as u64, area_refs.len() as u64);
        if nt + nw > 0 {
            let todos = area_refs.iter().filter(|e| matches!(e, EvidenceRef::FileLines { anchor: Some(a), .. } if TODO_TOKENS.contains(&a.as_str()))).count() as u64;
            let skips = nw - todos;
            let mut ev = tasks_in_area;
            ev.extend(area_refs);
            ev.push(EvidenceRef::GitRef {
                name: format!("refs/heads/{b}"),
            });
            s.emit(
                "work.area",
                Status::Inferred,
                vec![
                    t("Unfinished work around "),
                    code(b.rsplit('/').next().unwrap_or(b)),
                    t(": "),
                    Span::Count(nt),
                    t(" unchecked tasks, "),
                    Span::Count(todos),
                    t(" TODO/FIXME, "),
                    Span::Count(skips),
                    t(" skipped test (inferred from shared names). Why it stopped is unknown."),
                ],
                ev,
            );
        }
    }

    // Standing unknowns, always present (Property 4).
    s.emit(
        "unknown.pause_reason",
        Status::Unknown,
        vec![t(
            "Why work paused: unknown. Nothing in the project states it.",
        )],
        vec![root()],
    );
    s.emit(
        "unknown.builds",
        Status::Unknown,
        vec![t(
            "Whether the project builds or runs: unknown. DORMBIT never runs project code.",
        )],
        vec![root()],
    );
    s.emit(
        "unknown.deploy_target",
        Status::Unknown,
        vec![t("Where it deploys: unknown.")],
        vec![root()],
    );
    if ix.skipped > 0 {
        s.emit("scan.skipped_files", Status::Confirmed, vec![Span::Count(ix.skipped), t(" files weren't read for evidence (sensitive, ignored, data, binary, or too large).")], vec![root()]);
    }
}
