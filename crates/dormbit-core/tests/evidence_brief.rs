//! Evidence and the Resurrection Brief over the Dockline fixture (intelligence-and-brief).

#[path = "support/golden.rs"]
#[macro_use]
mod golden;
#[path = "support/dockline.rs"]
mod fixture;

use dormbit_core::brief::{
    LINE_BUILD, LINE_SESSION, RestoreFacts, check_citations, code_span, escape_inline, generate,
    render_markdown,
};
use dormbit_core::evidence::{
    AgentSession, EvidenceInput, EvidenceRef, EvidenceResult, Status, evaluate,
};
use dormbit_core::model::GitSnapshot;

const SIDECAR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/dockline/dockline-invoices.sidecar.json"
));

fn git() -> GitSnapshot {
    let v: serde_json::Value = serde_json::from_str(SIDECAR).unwrap();
    serde_json::from_value(v["git_snapshot"].clone()).unwrap()
}

fn sessions() -> Vec<AgentSession> {
    vec![AgentSession {
        adapter: "claude_code".into(),
        session_id: "0b9e6c3a-5f1d-4e2b-9a7c-3d8e1f2a4b6c".into(),
        ended_at: Some("2025-05-21T23:12:07Z".into()),
        git_branch: Some("feature/pdf-export".into()),
    }]
}

fn dockline_evidence() -> EvidenceResult {
    let v = fixture::dockline();
    let g = git();
    let s = sessions();
    evaluate(&EvidenceInput {
        vfs: &v,
        folder_name: "dockline-invoices",
        git: Some(&g),
        agent_sessions: &s,
    })
}

fn plain(e: &EvidenceResult, rule: &str) -> Vec<String> {
    e.by_rule(rule).map(|f| f.plain()).collect()
}

#[test]
fn dockline_evidence_shows_what_the_demo_needs() {
    let e = dockline_evidence();
    assert_eq!(
        plain(&e, "identity.name"),
        vec!["Project name: dockline-invoices (from `package.json`)."]
    );
    assert!(
        plain(&e, "git.branch")[0]
            .starts_with("Checked-out branch: feature/pdf-export at bcbfc5718b44")
    );
    assert_eq!(plain(&e, "git.feature_branch").len(), 1);
    assert_eq!(
        plain(&e, "git.wip_commit"),
        vec!["The last commit is marked work in progress: WIP: pdf totals."]
    );
    assert_eq!(plain(&e, "git.stash"), vec!["There is 1 stash entry."]);
    assert!(plain(&e, "git.uncommitted")[0].contains("2 modified, 0 staged, 1 untracked"));

    let stack = plain(&e, "stack.dependency").join(" | ");
    for label in [
        "React:",
        "Vite:",
        "Fastify:",
        "Drizzle ORM:",
        "TypeScript:",
        "Vitest:",
    ] {
        assert!(stack.contains(label), "{label} in {stack}");
    }
    assert!(
        plain(&e, "stack.framework")
            .iter()
            .any(|s| s.starts_with("React + Vite frontend"))
    );
    assert_eq!(
        plain(&e, "toolchain.file"),
        vec!["Node 22.13.1 (from .nvmrc)."]
    );

    assert_eq!(
        plain(&e, "kiro.tasks_progress"),
        vec![".kiro/specs/pdf-export/tasks.md: 3 of 7 tasks complete."]
    );
    assert_eq!(plain(&e, "kiro.task_unchecked").len(), 4);

    let todo: Vec<(String, u32)> = e
        .by_rule("work.todo")
        .filter_map(|f| match &f.evidence[0] {
            EvidenceRef::FileLines {
                path, start_line, ..
            } => Some((path.clone(), *start_line)),
            _ => None,
        })
        .collect();
    assert!(
        todo.contains(&("src/pdf/renderInvoice.ts".into(), 14)),
        "{todo:?}"
    );
    assert!(
        todo.contains(&("src/pdf/renderInvoice.ts".into(), 15)),
        "{todo:?}"
    );
    let skip = plain(&e, "work.skipped_test");
    assert_eq!(
        skip,
        vec![
            "Skipped test at src/pdf/renderInvoice.test.ts:9: prints totals that match the invoice"
        ]
    );
    let area = plain(&e, "work.area");
    assert_eq!(area.len(), 1, "{area:?}");
    assert!(
        area[0].starts_with(
            "Unfinished work around pdf-export: 4 unchecked tasks, 2 TODO/FIXME, 1 skipped test"
        ),
        "{area:?}"
    );

    // `.env` is stat'ed only; `.env.example` gives key names and never values.
    assert_eq!(
        plain(&e, "sensitive.present"),
        vec![".env exists. It wasn't read and isn't in a capsule."]
    );
    let keys = plain(&e, "env.example_key");
    assert_eq!(keys.len(), 3);
    assert!(keys[0].starts_with("DATABASE_URL is listed in .env.example"));

    for r in [
        "unknown.pause_reason",
        "unknown.builds",
        "unknown.deploy_target",
    ] {
        assert_eq!(e.by_rule(r).count(), 1, "{r}");
        assert!(e.by_rule(r).all(|f| f.status == Status::Unknown));
    }
    assert!(plain(&e, "adapter.session")[0].contains("Not resumed by DORMBIT."));
    assert_eq!(plain(&e, "prerun.none").len(), 1);
}

#[test]
fn no_finding_quotes_a_secret_or_the_env_file() {
    let e = dockline_evidence();
    let env_file = {
        let v = fixture::dockline();
        match dormbit_core::vfs::ReadVfs::read_limited(
            &v,
            &dormbit_core::vfs::RelPath::parse(".env").unwrap(),
            4096,
        )
        .unwrap()
        {
            dormbit_core::vfs::Limited::Complete(b) => String::from_utf8(b).unwrap(),
            _ => panic!(),
        }
    };
    let all = serde_json::to_string(&e).unwrap();
    for line in env_file.lines() {
        if let Some((_, value)) = line.split_once('=')
            && value.len() > 3
        {
            assert!(!all.contains(value.trim()), "a .env value leaked");
        }
    }
}

#[test]
fn every_finding_is_well_formed_and_ids_are_stable() {
    let a = dockline_evidence();
    let b = dockline_evidence();
    assert_eq!(a, b, "evidence must be deterministic");
    let mut ids = std::collections::BTreeSet::new();
    for f in &a.findings {
        assert!(!f.evidence.is_empty(), "{}", f.rule);
        assert!(f.id.starts_with("F-") && f.id.len() == 14, "{}", f.id);
        assert!(ids.insert(f.id.clone()), "duplicate id {}", f.id);
    }
}

#[test]
fn the_brief_cites_every_line_and_keeps_the_four_outcomes_fixed() {
    let e = dockline_evidence();
    let b = generate(&e, None, "0.1.0", "2025-06-02");
    let md = render_markdown(&b);
    check_citations(&md, &b.evidence_index).unwrap_or_else(|err| panic!("{err}\n{md}"));
    let outcomes: Vec<&str> = md
        .lines()
        .skip_while(|l| *l != "## Outcomes")
        .skip(2)
        .take(4)
        .collect();
    assert!(
        outcomes[0].starts_with("- Files restored and verified: none yet."),
        "{outcomes:?}"
    );
    assert!(outcomes[1].starts_with("- Context reconstructed (inferred from evidence). "));
    assert_eq!(outcomes[2], format!("- {LINE_SESSION}"));
    assert!(outcomes[3].starts_with(&format!("- {LINE_BUILD} [F-")));
    let titles: Vec<&str> = md.lines().filter_map(|l| l.strip_prefix("## ")).collect();
    assert_eq!(
        titles,
        vec![
            "Outcomes",
            "Identity",
            "Technologies",
            "Repo state",
            "Docs",
            "Verified content",
            "Unfinished work",
            "Evidence-backed next steps",
            "Context references",
            "Missing information",
            "First steps",
            "Pre-run checks",
            "Evidence index"
        ]
    );
    golden!("brief", "dockline_no_restore", &md);

    let restored = RestoreFacts {
        op_id: "0001".into(),
        capsule_id: "c-1".into(),
        verified: true,
        files_verified: 182,
        mismatches: 0,
        not_restored: 5,
        git_refs_match: Some(true),
    };
    let b = generate(&e, Some(&restored), "0.1.0", "2025-06-02");
    let md = render_markdown(&b);
    check_citations(&md, &b.evidence_index).unwrap_or_else(|err| panic!("{err}"));
    assert!(md.contains("- Files restored and verified. 182 files restored and re-hashed against capsule `c-1`, 0 mismatches. [F-"));
    golden!("brief", "dockline_restored", &md);
    golden!("brief", "dockline_findings", &e);
}

#[test]
fn untrusted_text_cannot_create_markup_or_break_out_of_code() {
    assert_eq!(escape_inline("<img src=x>"), "&lt;img src=x&gt;");
    assert_eq!(
        escape_inline("[x](javascript:1)"),
        "\\[x\\]\\(javascript:1\\)"
    );
    assert_eq!(escape_inline("a\u{202e}b\nc"), "a\\u{202E}b c");
    assert_eq!(code_span("a`b"), "``a`b``");
    assert_eq!(code_span("`x`"), "`` `x` ``");
    assert_eq!(code_span("x\u{2066}y"), "`x?y`");
}

#[test]
fn a_brief_with_an_uncited_line_is_rejected() {
    let e = dockline_evidence();
    let b = generate(&e, None, "0.1.0", "2025-06-02");
    let mut md = render_markdown(&b);
    md.push_str("- This project definitely builds.\n");
    assert!(check_citations(&md, &b.evidence_index).is_err());
    let md2 = render_markdown(&b).replace(&b.evidence_index[0].id, "F-000000000000");
    assert!(check_citations(&md2, &b.evidence_index).is_err());
}

#[test]
fn the_citation_checker_accepts_only_exact_template_lines() {
    let e = dockline_evidence();
    let b = generate(&e, None, "0.1.0", "2025-06-02");
    let base = render_markdown(&b);
    let id = b.evidence_index[0].id.clone();
    for bad in [
        "## Identity and also it builds".to_owned(),
        "- No findings for this section. It builds.".to_owned(),
        "- Context reconstructed (inferred from evidence). It definitely works.".to_owned(),
        format!("- A quote \\[{id}"),
        format!("- Claimed [{id}] then more text"),
        "| F-zzzzzzzzzzzz | Confirmed | x | y |".to_owned(),
    ] {
        let md = format!("{base}{bad}\n");
        assert!(
            check_citations(&md, &b.evidence_index).is_err(),
            "accepted: {bad}"
        );
    }
}

fn evidence_of(files: &[(&str, &str)]) -> EvidenceResult {
    let mut v = dormbit_core::vfs::MemVfs::new();
    for (p, c) in files {
        v.add_file(p, *c);
    }
    evaluate(&EvidenceInput {
        vfs: &v,
        folder_name: "x",
        git: None,
        agent_sessions: &[],
    })
}

#[test]
fn secrets_in_scripts_and_versions_are_hidden() {
    let token = format!("{}{}", "gh", "p_") + &"aB3d".repeat(9);
    let pkg = format!(
        r#"{{"name":"x","scripts":{{"deploy":"TOKEN={token} node d.js"}},"dependencies":{{"react":"git+https://u:{token}@example.invalid/r.git"}}}}"#
    );
    let e = evidence_of(&[("package.json", &pkg)]);
    let all = serde_json::to_string(&e).unwrap();
    assert!(!all.contains(&token), "a token leaked");
    assert!(all.contains("[hidden: looks like a secret]"));
}

#[test]
fn exit_calls_and_mentions_are_not_markers() {
    let e = evidence_of(&[
        (
            "a.ts",
            "process.exit(1);\nconst x = 1; // see the FIXME in b.ts\n",
        ),
        ("b.py", "sys.exit(0)\n"),
        ("c.ts", "  xit(\"later\", () => {});\n// TODO: real one\n"),
    ]);
    assert_eq!(
        plain(&e, "work.skipped_test"),
        vec!["Skipped test at c.ts:1: later"]
    );
    assert_eq!(plain(&e, "work.todo"), vec!["TODO at c.ts:2: real one"]);
}

#[test]
fn wip_must_be_a_word_and_unreadable_manifests_are_reported() {
    let mut g = git();
    g.head.summary = "wipe the cache".into();
    let v = fixture::dockline();
    let e = evaluate(&EvidenceInput {
        vfs: &v,
        folder_name: "d",
        git: Some(&g),
        agent_sessions: &[],
    });
    assert!(plain(&e, "git.wip_commit").is_empty());

    let e = evidence_of(&[("package.json", "{ not json")]);
    assert_eq!(e.by_rule("source.unsupported").count(), 1);
    assert_eq!(e.by_rule("prerun.none").count(), 0, "must not claim none");
    assert_eq!(e.by_rule("git.absent").count(), 1);
}
