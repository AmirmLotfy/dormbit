//! Project intelligence (intelligence-and-brief spec, INT-1..INT-3): evidence rules over a
//! read-only `ReadVfs` that emit Findings. Every Finding has at least one structured
//! evidence reference and a stable ID; nothing is claimed that the evidence doesn't show.
//!
//! Text is read only from files the plan classifier marks as ordinary project files, so
//! sensitive, rebuildable, gitignored, data, and `.git` files are never opened. `.env`-style
//! files are only stat'ed, and `.env.example` contributes key names, never values.

mod rules;
pub mod tasks;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::capsule::CapsuleLimits;
use crate::model::{GitSnapshot, GitTreatment, PlanItemKind};
use crate::plan::{ReasonCode, classify_tree};
use crate::util::sha256_hex;
use crate::vfs::{Limited, ReadVfs, RelPath};

pub use rules::{SECTIONS, SectionId, section_of as rules_section};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Confirmed,
    Inferred,
    Unknown,
    Unsupported,
}

/// Statement pieces. Only `Text` is DORMBIT's own template text; `Quote` and `Code` hold
/// untrusted text from the project and are always escaped when rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Span {
    Text(String),
    Quote(String),
    Code(String),
    Count(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatKind {
    File,
    Dir,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceRef {
    /// Lines `start_line..=end_line` (1-based) of a project file; `anchor` occurs in them.
    FileLines {
        path: String,
        start_line: u32,
        end_line: u32,
        anchor: Option<String>,
    },
    /// A JSON Pointer into a JSON file (for example `package.json` `/scripts/test`).
    ManifestKey {
        path: String,
        pointer: String,
    },
    /// That a path exists (or doesn't), without reading it.
    FileStat {
        path: String,
        stat: StatKind,
        size: Option<u64>,
    },
    GitRef {
        name: String,
    },
    GitCommit {
        oid: String,
    },
    /// Working-tree counts from the Git snapshot taken at scan time.
    GitStatus {
        modified: u64,
        staged: u64,
        untracked: u64,
    },
    /// A risky Git config key, never its value.
    GitConfigKey {
        key: String,
    },
    /// An agent session from a consented adapter (metadata only).
    AgentSession {
        adapter: String,
        session_id: String,
    },
    /// The capsule or restore record (for the brief's verified-content lines).
    Capsule {
        capsule_id: String,
        pointer: String,
    },
    Restore {
        op_id: String,
    },
    /// The engine itself: a rule reached its per-rule limit and `dropped` findings weren't kept.
    RuleLimit {
        rule: String,
        limit: u64,
        dropped: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub rule: String,
    pub statement: Vec<Span>,
    pub status: Status,
    pub evidence: Vec<EvidenceRef>,
}

impl Finding {
    /// The statement as plain text (for search, tests, and the UI's accessible name).
    pub fn plain(&self) -> String {
        self.statement
            .iter()
            .map(|s| match s {
                Span::Text(t) | Span::Quote(t) | Span::Code(t) => t.clone(),
                Span::Count(n) => n.to_string(),
            })
            .collect()
    }
}

/// Metadata of one agent session, from a consented adapter (or the demo's Sidecar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    /// `claude_code`, `codex`, or `kiro_export`; the label shown comes from a fixed table.
    pub adapter: String,
    pub session_id: String,
    pub ended_at: Option<String>,
    pub git_branch: Option<String>,
}

pub struct EvidenceInput<'a, V: ReadVfs> {
    pub vfs: &'a V,
    /// The folder name, used when no manifest names the project.
    pub folder_name: &'a str,
    pub git: Option<&'a GitSnapshot>,
    pub agent_sessions: &'a [AgentSession],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceResult {
    pub findings: Vec<Finding>,
    pub files_scanned: u64,
    pub files_skipped: u64,
}

impl EvidenceResult {
    pub fn by_rule<'a>(&'a self, rule: &'a str) -> impl Iterator<Item = &'a Finding> + 'a {
        self.findings.iter().filter(move |f| f.rule == rule)
    }

    pub fn get(&self, id: &str) -> Option<&Finding> {
        self.findings.iter().find(|f| f.id == id)
    }
}

pub const MAX_TEXT_BYTES: u64 = 1 << 20;
pub const MAX_PER_RULE: usize = 200;
pub const MAX_QUOTE_CHARS: usize = 200;

/// `F-` plus the first 12 hex digits of SHA-256(rule, 0x00, canonical evidence JSON).
pub fn finding_id(rule: &str, evidence: &[EvidenceRef]) -> String {
    let mut bytes = rule.as_bytes().to_vec();
    bytes.push(0);
    bytes.extend(serde_json::to_vec(evidence).unwrap_or_default());
    let h = sha256_hex(&bytes);
    format!("F-{}", &h[..12])
}

/// Collects Findings, enforcing the shape rules: registered rule, allowed status, non-empty
/// evidence, relative paths only, a per-rule cap, and merging identical emissions.
pub(crate) struct Sink {
    findings: BTreeMap<String, Finding>,
    per_rule: BTreeMap<&'static str, usize>,
    dropped: BTreeMap<&'static str, u64>,
}

impl Sink {
    fn new() -> Self {
        Self {
            findings: BTreeMap::new(),
            per_rule: BTreeMap::new(),
            dropped: BTreeMap::new(),
        }
    }

    /// Discloses every rule that hit its cap (R7.5), so nothing is silently under-reported.
    fn report_truncation(&mut self) {
        let dropped: Vec<(&'static str, u64)> =
            self.dropped.iter().map(|(r, n)| (*r, *n)).collect();
        for (rule, n) in dropped {
            self.emit(
                "scan.truncated",
                Status::Confirmed,
                vec![
                    Span::Count(n),
                    Span::Text(" more findings of rule ".into()),
                    Span::Code(rule.to_owned()),
                    Span::Text(format!(" weren't listed (limit {MAX_PER_RULE} per rule).")),
                ],
                vec![EvidenceRef::RuleLimit {
                    rule: rule.to_owned(),
                    limit: MAX_PER_RULE as u64,
                    dropped: n,
                }],
            );
        }
    }

    pub(crate) fn emit(
        &mut self,
        rule: &'static str,
        status: Status,
        statement: Vec<Span>,
        mut evidence: Vec<EvidenceRef>,
    ) -> Option<String> {
        let spec = rules::spec(rule)?;
        if !spec.allowed.contains(&status) || evidence.is_empty() {
            return None;
        }
        let paths_ok = evidence.iter().all(|e| match e {
            EvidenceRef::FileLines { path, .. }
            | EvidenceRef::ManifestKey { path, .. }
            | EvidenceRef::FileStat { path, .. } => path.is_empty() || RelPath::parse(path).is_ok(),
            _ => true,
        });
        if !paths_ok {
            return None;
        }
        evidence.sort();
        evidence.dedup();
        let id = finding_id(rule, &evidence);
        // An identical emission merges with the existing Finding; only new ones count
        // against the per-rule limit.
        if self.findings.contains_key(&id) {
            return Some(id);
        }
        let n = self.per_rule.entry(rule).or_insert(0);
        if *n >= MAX_PER_RULE {
            *self.dropped.entry(rule).or_insert(0) += 1;
            return None;
        }
        {
            *n += 1;
            self.findings.insert(
                id.clone(),
                Finding {
                    id: id.clone(),
                    rule: rule.to_owned(),
                    statement,
                    status,
                    evidence,
                },
            );
        }
        Some(id)
    }

    fn into_sorted(self) -> Vec<Finding> {
        let mut v: Vec<Finding> = self.findings.into_values().collect();
        v.sort_by(|a, b| {
            let ka = rules::sort_key(&a.rule);
            let kb = rules::sort_key(&b.rule);
            ka.cmp(&kb)
                .then_with(|| a.evidence.first().cmp(&b.evidence.first()))
                .then_with(|| a.id.cmp(&b.id))
        });
        v
    }
}

/// One readable text file of the project.
pub(crate) struct TextFile {
    pub path: String,
    pub text: String,
}

/// The files the rules may look at: every candidate with its reason, plus the text of
/// ordinary project files under the size limit.
pub(crate) struct FileIndex {
    pub entries: Vec<(String, ReasonCode, u64)>,
    pub texts: Vec<TextFile>,
    pub skipped: u64,
}

impl FileIndex {
    pub fn has(&self, path: &str) -> bool {
        self.entries.iter().any(|(p, _, _)| p == path)
    }

    pub fn stat(&self, path: &str) -> EvidenceRef {
        match self.entries.iter().find(|(p, _, _)| p == path) {
            Some((p, _, size)) => EvidenceRef::FileStat {
                path: p.clone(),
                stat: StatKind::File,
                size: Some(*size),
            },
            None => EvidenceRef::FileStat {
                path: path.to_owned(),
                stat: StatKind::Absent,
                size: None,
            },
        }
    }

    pub fn text(&self, path: &str) -> Option<&str> {
        self.texts
            .iter()
            .find(|t| t.path == path)
            .map(|t| t.text.as_str())
    }
}

fn build_index<V: ReadVfs>(vfs: &V) -> FileIndex {
    let limits = CapsuleLimits::default();
    let candidates = classify_tree(vfs, GitTreatment::Full, &limits).unwrap_or_default();
    let mut entries = Vec::new();
    let mut texts = Vec::new();
    let mut skipped = 0u64;
    for c in candidates {
        let is_file = c.kind == PlanItemKind::File;
        if is_file || c.kind == PlanItemKind::Dir {
            entries.push((c.path.clone(), c.reason, c.size));
        }
        if !is_file || c.reason == ReasonCode::GitInternal {
            continue;
        }
        if c.reason != ReasonCode::ProjectFile || c.size > MAX_TEXT_BYTES {
            skipped += 1;
            continue;
        }
        let Ok(rel) = RelPath::parse(&c.path) else {
            continue;
        };
        match vfs.read_limited(&rel, MAX_TEXT_BYTES) {
            Ok(Limited::Complete(b)) => match String::from_utf8(b) {
                Ok(text) if !text.contains('\0') => texts.push(TextFile { path: c.path, text }),
                _ => skipped += 1,
            },
            _ => skipped += 1,
        }
    }
    FileIndex {
        entries,
        texts,
        skipped,
    }
}

/// Runs every rule over the project. Deterministic: the same tree, snapshot, and sessions
/// always give the same Findings in the same order.
pub fn evaluate<V: ReadVfs>(input: &EvidenceInput<'_, V>) -> EvidenceResult {
    let index = build_index(input.vfs);
    let mut sink = Sink::new();
    rules::run_all(input, &index, &mut sink);
    sink.report_truncation();
    EvidenceResult {
        findings: sink.into_sorted(),
        files_scanned: index.texts.len() as u64,
        files_skipped: index.skipped,
    }
}

/// Shortens untrusted text for a quote, replacing control characters.
pub(crate) fn clip(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_QUOTE_CHARS)
        .collect();
    t.trim().to_owned()
}
