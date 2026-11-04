//! The Dockline Git history (foundation Requirements 18.3 to 18.5, 18.11, 19.3): a fixed
//! `.git/config`, 24 commits by two fictional authors (17 on `main`, 7 on
//! `feature/pdf-export`), one stash, then uncommitted working changes.

use std::fs;
use std::path::Path;

use git2::{Commit, Repository, RepositoryInitOptions, Signature, Time};

use crate::time::pacific;
use crate::tree as t;
use crate::{Ctx, R};

#[derive(Clone, Copy)]
pub struct Person {
    pub name: &'static str,
    pub email: &'static str,
}

pub const MARA: Person = Person {
    name: "Mara Quill",
    email: "mara@dockline.invalid",
};
pub const THEO: Person = Person {
    name: "Theo Brandt",
    email: "theo@dockline.invalid",
};

pub const FEATURE: &str = "feature/pdf-export";

enum Body {
    Text(&'static str),
    /// `.kiro/specs/pdf-export/tasks.md` with this many tasks checked.
    Tasks(usize),
}

struct Change(&'static str, Body);

struct Entry {
    /// Create `feature/pdf-export` at HEAD and switch to it before this commit.
    branch_here: bool,
    who: Person,
    /// Pacific wall clock in 2025: (month, day, hour, minute).
    when: (i64, i64, i64, i64),
    summary: &'static str,
    changes: Vec<Change>,
}

use Body::{Tasks, Text};

fn entries() -> Vec<Entry> {
    let e = |who, when, summary, changes| Entry {
        branch_here: false,
        who,
        when,
        summary,
        changes,
    };
    vec![
        e(
            MARA,
            (3, 3, 9, 14),
            "Initial commit: Vite, React, and TypeScript",
            vec![
                Change(".gitignore", Text(t::GITIGNORE)),
                Change(".nvmrc", Text(t::NVMRC)),
                Change("package.json", Text(t::PACKAGE_JSON_V1)),
                Change("tsconfig.json", Text(t::TSCONFIG_JSON)),
                Change("vite.config.ts", Text(t::VITE_CONFIG)),
                Change("index.html", Text(t::INDEX_HTML)),
                Change("src/main.tsx", Text(t::MAIN_TSX)),
                Change("src/App.tsx", Text(t::APP_TSX)),
                Change("README.md", Text(t::README_V1)),
            ],
        ),
        e(
            MARA,
            (3, 4, 10, 2),
            "Add Fastify API with a health route",
            vec![
                Change("server/index.ts", Text(t::SERVER_INDEX)),
                Change("server/routes/health.ts", Text(t::HEALTH_TS)),
            ],
        ),
        e(
            THEO,
            (3, 5, 15, 40),
            "Add Drizzle schema for customers and invoices",
            vec![
                Change("drizzle.config.ts", Text(t::DRIZZLE_CONFIG)),
                Change("server/db/schema.ts", Text(t::DB_SCHEMA)),
            ],
        ),
        e(
            THEO,
            (3, 7, 11, 25),
            "List invoices from the API",
            vec![Change(
                "server/routes/invoices.ts",
                Text(t::INVOICES_ROUTE_V1),
            )],
        ),
        e(
            MARA,
            (3, 10, 9, 50),
            "Show invoices in the web app",
            vec![Change("src/pages/Invoices.tsx", Text(t::INVOICES_PAGE))],
        ),
        e(
            THEO,
            (3, 12, 14, 5),
            "Add customers endpoint",
            vec![Change(
                "server/routes/customers.ts",
                Text(t::CUSTOMERS_ROUTE),
            )],
        ),
        e(
            MARA,
            (3, 14, 16, 30),
            "Add money helpers for subtotals and tax",
            vec![Change("src/lib/money.ts", Text(t::MONEY_V1))],
        ),
        e(
            MARA,
            (3, 18, 10, 11),
            "Test the money helpers",
            vec![Change("src/lib/money.test.ts", Text(t::MONEY_TEST))],
        ),
        e(
            THEO,
            (3, 20, 13, 48),
            "Seed script with two sample customers",
            vec![Change("server/db/seed.ts", Text(t::SEED_TS))],
        ),
        e(
            MARA,
            (3, 24, 9, 30),
            "Add .env.example",
            vec![Change(".env.example", Text(t::ENV_EXAMPLE))],
        ),
        e(
            THEO,
            (3, 27, 17, 2),
            "Paginate the invoice list",
            vec![Change(
                "server/routes/invoices.ts",
                Text(t::INVOICES_ROUTE_V2),
            )],
        ),
        e(
            MARA,
            (4, 1, 11, 44),
            "Add roadmap and changelog",
            vec![
                Change("docs/ROADMAP.md", Text(t::ROADMAP_V1)),
                Change("CHANGELOG.md", Text(t::CHANGELOG_V1)),
            ],
        ),
        e(
            THEO,
            (4, 4, 15, 15),
            "Invoice detail view",
            vec![Change(
                "src/pages/InvoiceDetail.tsx",
                Text(t::INVOICE_DETAIL_V1),
            )],
        ),
        e(
            MARA,
            (4, 9, 10, 20),
            "Fix tax rounding: round once on the subtotal",
            vec![Change("src/lib/money.ts", Text(t::MONEY_V2))],
        ),
        e(
            THEO,
            (4, 15, 14, 33),
            "Spec PDF export in Kiro",
            vec![
                Change(
                    ".kiro/specs/pdf-export/requirements.md",
                    Text(t::PDF_REQUIREMENTS_V1),
                ),
                Change(".kiro/specs/pdf-export/tasks.md", Tasks(0)),
            ],
        ),
        e(
            MARA,
            (4, 23, 9, 58),
            "Document setup in the README",
            vec![Change("README.md", Text(t::README_MD))],
        ),
        e(
            MARA,
            (5, 2, 12, 0),
            "Release 0.1.0",
            vec![
                Change("package.json", Text(t::PACKAGE_JSON_V2)),
                Change("CHANGELOG.md", Text(t::CHANGELOG_V2)),
            ],
        ),
        Entry {
            branch_here: true,
            ..e(
                THEO,
                (5, 5, 10, 40),
                "Add a renderInvoice stub",
                vec![
                    Change("src/pdf/renderInvoice.ts", Text(t::RENDER_V1)),
                    Change(".kiro/specs/pdf-export/tasks.md", Tasks(1)),
                ],
            )
        },
        e(
            THEO,
            (5, 8, 15, 22),
            "Lay out the header and line items",
            vec![
                Change("src/pdf/renderInvoice.ts", Text(t::RENDER_V2)),
                Change(".kiro/specs/pdf-export/tasks.md", Tasks(2)),
            ],
        ),
        e(
            MARA,
            (5, 12, 11, 5),
            "Wire the Download PDF button",
            vec![
                Change("src/pages/InvoiceDetail.tsx", Text(t::INVOICE_DETAIL_V2)),
                Change(".kiro/specs/pdf-export/tasks.md", Tasks(3)),
            ],
        ),
        e(
            THEO,
            (5, 14, 16, 47),
            "Test renderInvoice",
            vec![Change(
                "src/pdf/renderInvoice.test.ts",
                Text(t::RENDER_TEST_V1),
            )],
        ),
        e(
            MARA,
            (5, 16, 9, 12),
            "Clarify totals in the PDF requirements",
            vec![Change(
                ".kiro/specs/pdf-export/requirements.md",
                Text(t::PDF_REQUIREMENTS),
            )],
        ),
        e(
            THEO,
            (5, 20, 13, 30),
            "Skip the totals test until the totals block lands",
            vec![Change(
                "src/pdf/renderInvoice.test.ts",
                Text(t::RENDER_TEST),
            )],
        ),
        e(
            MARA,
            (5, 21, 18, 45),
            "WIP: pdf totals",
            vec![Change("src/pdf/renderInvoice.ts", Text(t::RENDER_V3))],
        ),
    ]
}

fn signature(who: Person, (m, d, hh, mi): (i64, i64, i64, i64)) -> R<Signature<'static>> {
    let (secs, off) = pacific(m, d, hh, mi);
    Signature::new(who.name, who.email, &Time::new(secs, off)).ctx("signature")
}

pub fn write(root: &Path, rel: &str, bytes: &[u8]) -> R<()> {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ctx(rel)?;
    }
    fs::write(&path, bytes).ctx(rel)
}

/// Replaces what libgit2 created with fixed content, so nothing depends on the machine.
fn fix_git_dir(git: &Path) -> R<()> {
    fs::write(
        git.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = false\n\tlogallrefupdates = false\n\tignorecase = true\n\tprecomposeunicode = true\n\tautocrlf = false\n[init]\n\tdefaultBranch = main\n",
    )
    .ctx("config")?;
    let hooks = git.join("hooks");
    if hooks.exists() {
        fs::remove_dir_all(&hooks).ctx("hooks")?;
    }
    fs::create_dir_all(&hooks).ctx("hooks")?;
    let info = git.join("info");
    fs::create_dir_all(&info).ctx("info")?;
    fs::write(info.join("exclude"), "").ctx("info/exclude")?;
    fs::write(
        git.join("description"),
        "Dockline Invoices (fictional fixture)\n",
    )
    .ctx("description")
}

pub fn build(project: &Path) -> R<()> {
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head("main").external_template(false);
    Repository::init_opts(project, &opts).ctx("init")?;
    fix_git_dir(&project.join(".git"))?;
    let mut repo = Repository::open(project).ctx("open")?;

    commit_all(&repo, project)?;

    // One stash on top of the last commit; saving it restores the clean tree.
    write(
        project,
        "server/routes/invoices.ts",
        t::INVOICES_ROUTE_STASHED.as_bytes(),
    )?;
    let stasher = signature(MARA, (5, 22, 17, 10))?;
    let stash = repo
        .stash_save(&stasher, "rate limits", None)
        .ctx("stash")?;
    // `logallrefupdates = false` keeps reflogs off, but `git stash list` reads the stash's
    // reflog, so write that one entry with the same fixed signature.
    let mut reflog = repo.reflog("refs/stash").ctx("stash reflog")?;
    if reflog.is_empty() {
        reflog
            .append(stash, &stasher, Some("On feature/pdf-export: rate limits"))
            .ctx("stash reflog entry")?;
        reflog.write().ctx("write stash reflog")?;
    }

    // Working changes: 2 modified tracked files, 1 untracked note, and ignored files.
    write(project, "src/pdf/renderInvoice.ts", t::RENDER_V4.as_bytes())?;
    write(project, "docs/ROADMAP.md", t::ROADMAP_V2.as_bytes())?;
    write(project, "notes/pdf-ideas.md", t::NOTES_PDF_IDEAS.as_bytes())?;
    write(project, ".env", t::ENV.as_bytes())?;
    write(project, "data/dev.sqlite", &t::dev_sqlite())?;
    write(project, "dist/index.html", t::DIST_INDEX.as_bytes())?;
    write(
        project,
        "node_modules/.placeholder",
        t::NODE_MODULES_PLACEHOLDER.as_bytes(),
    )
}

fn commit_all(repo: &Repository, project: &Path) -> R<()> {
    let mut index = repo.index().ctx("index")?;
    for entry in entries() {
        if entry.branch_here {
            let head = repo
                .head()
                .ctx("head")?
                .peel_to_commit()
                .ctx("head commit")?;
            repo.branch(FEATURE, &head, false).ctx("branch")?;
            repo.set_head(&format!("refs/heads/{FEATURE}"))
                .ctx("set head")?;
        }
        for Change(rel, body) in &entry.changes {
            let bytes = match body {
                Text(s) => s.as_bytes().to_vec(),
                Tasks(n) => t::pdf_tasks(*n).into_bytes(),
            };
            write(project, rel, &bytes)?;
            index.add_path(Path::new(rel)).ctx(rel)?;
        }
        index.write().ctx("index write")?;
        let tree = repo
            .find_tree(index.write_tree().ctx("write tree")?)
            .ctx("tree")?;
        let sig = signature(entry.who, entry.when)?;
        let parent: Option<Commit<'_>> = match repo.head() {
            Ok(h) => Some(h.peel_to_commit().ctx("parent")?),
            Err(_) => None,
        };
        let parents: Vec<&Commit<'_>> = parent.iter().collect();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("{}\n", entry.summary),
            &tree,
            &parents,
        )
        .ctx(entry.summary)?;
    }
    Ok(())
}
