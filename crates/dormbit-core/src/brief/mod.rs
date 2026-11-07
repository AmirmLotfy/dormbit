//! The Resurrection Brief (intelligence-and-brief, BRF-1..BRF-3).
//!
//! Every section line cites at least one Finding ID. The four outcomes are always four
//! separate lines with fixed labels (BRF-2): files, context, session, build. The session and
//! build lines never claim more than "not performed" and "unknown (never executed)".

use serde::{Deserialize, Serialize};

use crate::evidence::finding_id;
use crate::evidence::{EvidenceRef, EvidenceResult, Finding, SECTIONS, SectionId, Span, Status};

pub const LABEL_FILES: &str = "Files restored and verified";
pub const LABEL_CONTEXT: &str = "Context reconstructed (inferred from evidence)";
pub const LINE_SESSION: &str = "AI session resumed: not performed.";
pub const LINE_BUILD: &str = "Builds/runs: unknown (never executed).";

/// Facts from a verified restore of this project, if there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreFacts {
    pub op_id: String,
    pub capsule_id: String,
    pub verified: bool,
    pub files_verified: u64,
    pub mismatches: u64,
    pub not_restored: u64,
    pub git_refs_match: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefLine {
    pub spans: Vec<Span>,
    pub cites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefSection {
    pub id: SectionId,
    pub title: String,
    pub lines: Vec<BriefLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcomes {
    pub files: BriefLine,
    pub context: BriefLine,
    pub session: BriefLine,
    pub build: BriefLine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brief {
    pub app_version: String,
    pub generated_on: String,
    pub outcomes: Outcomes,
    pub sections: Vec<BriefSection>,
    pub evidence_index: Vec<Finding>,
}

fn text(s: &str) -> Span {
    Span::Text(s.to_owned())
}

fn line(spans: Vec<Span>, cites: Vec<String>) -> BriefLine {
    BriefLine { spans, cites }
}

fn restore_findings(r: Option<&RestoreFacts>) -> Vec<Finding> {
    let mk =
        |rule: &str, status: Status, statement: Vec<Span>, evidence: Vec<EvidenceRef>| Finding {
            id: finding_id(rule, &evidence),
            rule: rule.to_owned(),
            statement,
            status,
            evidence,
        };
    match r {
        // "Verified" means every file re-hashed and none differed; anything else isn't.
        Some(r) if r.verified && r.mismatches == 0 => {
            let ev = vec![
                EvidenceRef::Capsule {
                    capsule_id: r.capsule_id.clone(),
                    pointer: "/totals".into(),
                },
                EvidenceRef::Restore {
                    op_id: r.op_id.clone(),
                },
            ];
            let mut out = vec![mk(
                "restore.verified",
                Status::Confirmed,
                vec![
                    Span::Count(r.files_verified),
                    text(" files restored and re-hashed against capsule "),
                    Span::Code(r.capsule_id.clone()),
                    text(", "),
                    Span::Count(r.mismatches),
                    text(" mismatches."),
                ],
                ev,
            )];
            if r.not_restored > 0 {
                out.push(mk(
                    "restore.not_restored",
                    Status::Confirmed,
                    vec![
                        Span::Count(r.not_restored),
                        text(" items were not restored; the restore report lists each with its reason."),
                    ],
                    vec![EvidenceRef::Capsule {
                        capsule_id: r.capsule_id.clone(),
                        pointer: "/excluded".into(),
                    }],
                ));
            }
            if let Some(m) = r.git_refs_match {
                out.push(mk(
                    "restore.git_match",
                    Status::Confirmed,
                    vec![text(if m {
                        "The restored Git HEAD and refs match the capsule."
                    } else {
                        "The restored Git HEAD or refs differ from the capsule."
                    })],
                    vec![EvidenceRef::Capsule {
                        capsule_id: r.capsule_id.clone(),
                        pointer: "/git".into(),
                    }],
                ));
            }
            out
        }
        _ => vec![mk(
            "restore.none",
            Status::Unknown,
            vec![text("No verified restore of this project yet.")],
            vec![EvidenceRef::FileStat {
                path: String::new(),
                stat: crate::evidence::StatKind::Dir,
                size: None,
            }],
        )],
    }
}

fn next_step(f: &Finding) -> Option<BriefLine> {
    let loc = |f: &Finding| match f.evidence.first() {
        Some(EvidenceRef::FileLines {
            path, start_line, ..
        }) => format!("{path}:{start_line}"),
        _ => String::new(),
    };
    let quoted: Vec<Span> = f
        .statement
        .iter()
        .filter(|s| matches!(s, Span::Quote(_)))
        .cloned()
        .collect();
    let spans = match f.rule.as_str() {
        "kiro.task_unchecked" => {
            let mut v = vec![text("Review unchecked task: ")];
            v.extend(quoted);
            v
        }
        "work.todo" => vec![text("Review the marker at "), Span::Code(loc(f)), text(".")],
        "work.skipped_test" => vec![
            text("Review the skipped test at "),
            Span::Code(loc(f)),
            text("."),
        ],
        "git.uncommitted" => vec![text("Review the uncommitted changes before anything else.")],
        "git.stash" => vec![
            text("Review the stash ("),
            Span::Code("git stash list".into()),
            text(")."),
        ],
        _ => return None,
    };
    Some(line(spans, vec![f.id.clone()]))
}

/// Builds the brief from the evidence and an optional verified restore.
pub fn generate(
    evidence: &EvidenceResult,
    restore: Option<&RestoreFacts>,
    app_version: &str,
    generated_on: &str,
) -> Brief {
    let mut index: Vec<Finding> = evidence.findings.clone();
    let restore_fs = restore_findings(restore);
    index.extend(restore_fs.iter().cloned());

    let mut sections: Vec<BriefSection> = SECTIONS
        .iter()
        .map(|(id, title)| BriefSection {
            id: *id,
            title: (*title).to_owned(),
            lines: Vec::new(),
        })
        .collect();
    for f in &index {
        let Some(sec) = crate::evidence::rules_section(&f.rule) else {
            continue;
        };
        if let Some(s) = sections.iter_mut().find(|s| s.id == sec) {
            s.lines.push(line(f.statement.clone(), vec![f.id.clone()]));
        }
        if let Some(step) = next_step(f)
            && let Some(s) = sections.iter_mut().find(|s| s.id == SectionId::NextSteps)
        {
            s.lines.push(step);
        }
    }

    let files = match restore_fs.first() {
        Some(f) if f.rule == "restore.verified" => {
            let mut spans = vec![text(&format!("{LABEL_FILES}. "))];
            spans.extend(f.statement.clone());
            line(spans, vec![f.id.clone()])
        }
        Some(f) => line(
            vec![text(&format!(
                "{LABEL_FILES}: none yet. No verified restore is recorded for this project."
            ))],
            vec![f.id.clone()],
        ),
        None => line(
            vec![text(&format!(
                "{LABEL_FILES}: none yet. No verified restore is recorded for this project."
            ))],
            Vec::new(),
        ),
    };
    let count = |s: Status| index.iter().filter(|f| f.status == s).count() as u64;
    let context = line(
        vec![
            text(&format!("{LABEL_CONTEXT}. ")),
            Span::Count(index.len() as u64),
            text(" findings: "),
            Span::Count(count(Status::Confirmed)),
            text(" Confirmed, "),
            Span::Count(count(Status::Inferred)),
            text(" Inferred, "),
            Span::Count(count(Status::Unknown)),
            text(" Unknown, "),
            Span::Count(count(Status::Unsupported)),
            text(" Unsupported."),
        ],
        Vec::new(),
    );
    let build_cite: Vec<String> = index
        .iter()
        .filter(|f| f.rule == "unknown.builds")
        .map(|f| f.id.clone())
        .collect();
    Brief {
        app_version: app_version.to_owned(),
        generated_on: generated_on.to_owned(),
        outcomes: Outcomes {
            files,
            context,
            session: line(vec![text(LINE_SESSION)], Vec::new()),
            build: line(vec![text(LINE_BUILD)], build_cite),
        },
        sections,
        evidence_index: index,
    }
}

// ---- rendering -------------------------------------------------------------------------------

/// Escapes untrusted inline text: control and bidi characters neutralized, HTML-significant
/// characters as entities, Markdown punctuation backslash-escaped.
pub fn escape_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200b}'..='\u{200f}'
            | '\u{061c}'
            | '\u{feff}' => {
                out.push_str(&format!("\\u{{{:04X}}}", c as u32));
            }
            c if c.is_control() => out.push(' '),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '#' | '!' | '|' | '~' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

/// A code span that can't be broken out of: backtick run one longer than any inside.
pub fn code_span(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| match c {
            '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200b}'..='\u{200f}'
            | '\u{061c}'
            | '\u{feff}' => '?',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let mut longest = 0;
    let mut run = 0;
    for c in clean.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(longest + 1);
    let pad = if clean.starts_with('`') || clean.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{clean}{pad}{fence}")
}

fn render_spans(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|s| match s {
            Span::Text(t) => t.clone(),
            Span::Quote(q) => format!("\u{201c}{}\u{201d}", escape_inline(q)),
            Span::Code(c) => code_span(c),
            Span::Count(n) => n.to_string(),
        })
        .collect()
}

fn render_line(l: &BriefLine) -> String {
    let mut s = render_spans(&l.spans);
    for c in &l.cites {
        s.push_str(&format!(" [{c}]"));
    }
    s
}

pub fn render_markdown(b: &Brief) -> String {
    let mut out = String::new();
    out.push_str("# Resurrection Brief\n\n");
    out.push_str(&format!(
        "Generated by DORMBIT {} on {}. Paths are relative to the project root; ~ is your home folder.\n\n",
        escape_inline(&b.app_version),
        escape_inline(&b.generated_on)
    ));
    out.push_str("## Outcomes\n\n");
    for l in [
        &b.outcomes.files,
        &b.outcomes.context,
        &b.outcomes.session,
        &b.outcomes.build,
    ] {
        out.push_str(&format!("- {}\n", render_line(l)));
    }
    for s in &b.sections {
        out.push_str(&format!("\n## {}\n\n", s.title));
        if s.lines.is_empty() {
            out.push_str("- No findings for this section.\n");
        }
        for l in &s.lines {
            out.push_str(&format!("- {}\n", render_line(l)));
        }
    }
    out.push_str(
        "\n## Evidence index\n\n| Finding | Status | Rule | Evidence |\n|---|---|---|---|\n",
    );
    for f in &b.evidence_index {
        let ev: Vec<String> = f.evidence.iter().map(describe_ref).collect();
        out.push_str(&format!(
            "| {} | {:?} | {} | {} |\n",
            escape_inline(&f.id),
            f.status,
            code_span(&f.rule),
            escape_inline(&ev.join("; "))
        ));
    }
    out
}

/// Human-readable form of an evidence reference (for the index and the Evidence View).
pub fn describe_ref(e: &EvidenceRef) -> String {
    match e {
        EvidenceRef::FileLines {
            path,
            start_line,
            end_line,
            ..
        } => {
            if start_line == end_line {
                format!("{path} line {start_line}")
            } else {
                format!("{path} lines {start_line}-{end_line}")
            }
        }
        EvidenceRef::ManifestKey { path, pointer } => format!("{path} {pointer}"),
        EvidenceRef::FileStat { path, stat, .. } => {
            let p = if path.is_empty() {
                "project folder"
            } else {
                path
            };
            match stat {
                crate::evidence::StatKind::Absent => format!("{p} (absent)"),
                _ => format!("{p} (exists)"),
            }
        }
        EvidenceRef::GitRef { name } => format!("Git ref {name}"),
        EvidenceRef::GitCommit { oid } => {
            format!("Git commit {}", oid.chars().take(12).collect::<String>())
        }
        EvidenceRef::GitStatus { .. } => "Git status at scan time".into(),
        EvidenceRef::GitConfigKey { key } => format!("Git config key {key}"),
        EvidenceRef::AgentSession {
            adapter,
            session_id,
        } => format!("{adapter} session {session_id} (metadata only)"),
        EvidenceRef::Capsule {
            capsule_id,
            pointer,
        } => format!("capsule {capsule_id} manifest.json {pointer}"),
        EvidenceRef::Restore { op_id } => format!("restore {op_id} report"),
        EvidenceRef::RuleLimit {
            rule,
            limit,
            dropped,
        } => format!("rule {rule} reached its limit of {limit}; {dropped} not listed"),
    }
}

fn is_finding_id(s: &str) -> bool {
    s.len() == 14
        && s.starts_with("F-")
        && s[2..]
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Strips trailing ` [F-<12 hex>]` citations; returns the body and the cited IDs.
fn trailing_citations(l: &str) -> (&str, Vec<&str>) {
    let mut rest = l;
    let mut ids = Vec::new();
    while let Some(body) = rest.strip_suffix(']')
        && let Some(open) = body.rfind(" [")
        && is_finding_id(&body[open + 2..])
    {
        ids.push(&body[open + 2..]);
        rest = &body[..open];
    }
    (rest, ids)
}

/// The context outcome line: `- Context reconstructed (inferred from evidence). <n> findings:
/// <a> Confirmed, <b> Inferred, <c> Unknown, <d> Unsupported.` with numbers only.
fn is_context_line(l: &str) -> bool {
    let Some(rest) = l.strip_prefix(&format!("- {LABEL_CONTEXT}. ")) else {
        return false;
    };
    if !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let shape: String = rest
        .chars()
        .map(|c| if c.is_ascii_digit() { '9' } else { c })
        .collect::<String>()
        .split('9')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("#");
    shape == " findings: # Confirmed, # Inferred, # Unknown, # Unsupported."
}

/// Test oracle (PROP-12): every line is an exact template line or ends with at least one
/// `[F-<12 hex>]` citation whose ID is in the index (R12.5, R12.6).
pub fn check_citations(markdown: &str, index: &[Finding]) -> Result<(), String> {
    let known = |id: &str| index.iter().any(|f| f.id == id);
    let mut headings: Vec<String> = SECTIONS.iter().map(|(_, t)| format!("## {t}")).collect();
    headings.push("## Outcomes".into());
    headings.push("## Evidence index".into());
    for (i, l) in markdown.lines().enumerate() {
        let exact_template = l.is_empty()
            || l == "# Resurrection Brief"
            || headings.iter().any(|h| h == l)
            || l == "- No findings for this section."
            || l == format!("- {LINE_SESSION}")
            || l == "| Finding | Status | Rule | Evidence |"
            || l == "|---|---|---|---|"
            || is_context_line(l)
            || (l.starts_with("Generated by DORMBIT ")
                && l.ends_with(" Paths are relative to the project root; ~ is your home folder."));
        if exact_template {
            continue;
        }
        if let Some(rest) = l.strip_prefix("| ") {
            let id = rest.split(" | ").next().unwrap_or("");
            if is_finding_id(id) && known(id) && l.ends_with(" |") {
                continue;
            }
            return Err(format!("line {}: bad evidence index row", i + 1));
        }
        let (_, ids) = trailing_citations(l);
        if ids.is_empty() {
            return Err(format!("line {}: no trailing citation: {l}", i + 1));
        }
        if let Some(bad) = ids.iter().find(|id| !known(id)) {
            return Err(format!("line {}: cites unknown {bad}", i + 1));
        }
    }
    Ok(())
}
