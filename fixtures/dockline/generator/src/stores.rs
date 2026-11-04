//! Synthetic agent stores (foundation Requirement 18.10): a Claude Code session in JSONL and a
//! Kiro export zip. Message text is a placeholder; only the metadata is realistic.

use std::path::Path;

use serde_json::{Value, json};

use crate::history::{FEATURE, write};
use crate::pack::stored_zip;
use crate::{Ctx, R};

pub const CWD: &str = "/Users/dockline-demo/Projects/dockline-invoices";
pub const CLAUDE_SESSION: &str = "0b9e6c3a-5f1d-4e2b-9a7c-3d8e1f2a4b6c";
pub const CLAUDE_VERSION: &str = "1.0.31";
pub const CLAUDE_DIR: &str = "claude/projects/-Users-dockline-demo-Projects-dockline-invoices";
pub const KIRO_SESSION: &str = "kiro-7f3c2a10";
pub const KIRO_VERSION: &str = "0.9.2";
const PLACEHOLDER: &str = "synthetic message, not real";

/// Timestamps of the six Claude Code lines, on May 21, 2025 (UTC).
pub const CLAUDE_TIMES: [&str; 6] = [
    "2025-05-21T23:02:10Z",
    "2025-05-21T23:02:41Z",
    "2025-05-21T23:05:03Z",
    "2025-05-21T23:05:58Z",
    "2025-05-21T23:11:20Z",
    "2025-05-21T23:12:07Z",
];
pub const KIRO_STARTED: &str = "2025-05-20T22:00:00Z";
pub const KIRO_ENDED: &str = "2025-05-20T22:40:00Z";

fn jsonl(lines: &[Value]) -> R<String> {
    let mut s = String::new();
    for l in lines {
        s.push_str(&serde_json::to_string(l).ctx("jsonl")?);
        s.push('\n');
    }
    Ok(s)
}

pub fn build(stores: &Path) -> R<()> {
    let mut lines = Vec::new();
    let mut parent = Value::Null;
    for (i, at) in CLAUDE_TIMES.iter().enumerate() {
        let id = format!("5d0c1e2f-0000-4000-8000-00000000000{i}");
        let role = if i % 2 == 0 { "user" } else { "assistant" };
        lines.push(json!({
            "type": role,
            "sessionId": CLAUDE_SESSION,
            "uuid": id,
            "parentUuid": parent,
            "cwd": CWD,
            "gitBranch": FEATURE,
            "version": CLAUDE_VERSION,
            "timestamp": at,
            "message": { "role": role, "content": PLACEHOLDER },
        }));
        parent = Value::String(id);
    }
    write(
        stores,
        &format!("{CLAUDE_DIR}/{CLAUDE_SESSION}.jsonl"),
        jsonl(&lines)?.as_bytes(),
    )?;

    let session = json!({
        "session_id": KIRO_SESSION,
        "title": "PDF export totals",
        "working_directory": CWD,
        "started_at": KIRO_STARTED,
        "ended_at": KIRO_ENDED,
        "git_branch": FEATURE,
        "tool_version": KIRO_VERSION,
    });
    let messages: Vec<Value> = ["user", "assistant", "user", "assistant"]
        .iter()
        .enumerate()
        .map(|(i, role)| json!({ "role": role, "content": PLACEHOLDER, "turn": i + 1 }))
        .collect();
    let zip = stored_zip(&[
        (
            "session.json",
            format!(
                "{}\n",
                serde_json::to_string_pretty(&session).ctx("session")?
            )
            .into_bytes(),
        ),
        ("messages.jsonl", jsonl(&messages)?.into_bytes()),
    ]);
    write(stores, "kiro-export/dockline-pdf-export.zip", &zip)
}
