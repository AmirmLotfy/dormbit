# DORMBIT product requirements (PRD)

- **Task:** P01-T01 (Task 6)
- **Source of truth:** [`docs/MASTER_PLAN.md`](../MASTER_PLAN.md) §1–5. If this file and the plan disagree, the plan wins.
- **Tagline:** Pick up where your project stopped.
- **Built for:** Kiroween 2026, Frankenstein category. Deadline Fri Dec 4, 2026, 2:00 PM PT.

## 1. Product summary

DORMBIT is a local macOS app for **verified project shelving with an evidence-graded handoff**.

You point it at folders you choose. It finds the projects inside, reads their files and Git state, and shows what it found. Every finding is labeled Confirmed, Inferred, Unknown, or Unsupported, and cites its source. You review and approve a capture plan. DORMBIT then writes a `.dormbit` capsule, checks it with a separate verifier, and can later restore it into a new folder through a transactional restore that never overwrites anything. Finally it writes a Resurrection Brief: a Markdown handoff that tells you (or an AI agent) what was preserved, what is unfinished, and what is still unknown.

One Rust core runs both the macOS app (Tauri 2) and a browser judge demo (the same core compiled to WebAssembly, running on a synthetic project).

## 2. Problem

A developer pauses a project. Months later they want to pick it up again. Three things go wrong:

1. **Files that matter are not in the remote.** `git push` leaves behind gitignored files that matter, such as local notes and data files (`data/dev.sqlite`, scratch docs).
2. **Context disappears.** Claude Code stores plaintext transcripts under `~/.claude/projects/` and deletes them after `cleanupPeriodDays`, which defaults to 30 days ([research.md §2](research.md#2-local-ai-agent-storage-for-p1-adapters)). Unfinished work lives in TODOs, stashes, feature branches, and unchecked spec tasks that nobody wrote down.
3. **Nobody can say what a backup will give back.** Archives and backups store bytes. They don't show the project's state, don't separate rebuildable output from real work, and don't prove a restore matches.

The question DORMBIT answers: "Can I shelve this project now and resume later, knowing exactly what I'll get back?"

## 3. Target users

The plan does not define personas. These are derived from §1–2 of the plan (see Open questions).

| User                                                                                | Need                                                                                    | Main path                               |
| ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------- |
| Developer on macOS (13 or later) who pauses side projects and uses AI coding agents | Shelve a project with the files that matter, and get an honest handoff when they return | Native app                              |
| Hackathon judge                                                                     | Test the safety claims without installing anything or exposing local files              | Web judge demo, then optionally the DMG |

## 4. Positioning

Every individual capability exists somewhere. DORMBIT doesn't claim to be the first tool to archive projects, inspect Git, or browse agent histories. It claims the combination. Sources: [research.md §1](research.md#1-competitors-and-adjacent-tools) and plan §2.

| Tool                                                                                                       | What it does well                                                                  | What it doesn't do (DORMBIT's gap)                                                                                       |
| ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| [Agent Sessions](https://github.com/jazzyalex/agent-sessions) (macOS, open source)                         | Searches local conversations from 16 coding agents; resumes supported CLI sessions | No project preservation, capsule, or restore proof                                                                       |
| [Claude Code History Viewer](https://github.com/jhlee0409/claude-code-history-viewer) (Tauri, open source) | Browses conversations from 31 assistants offline                                   | Same gap                                                                                                                 |
| [SpecStory](https://specstory.com/)                                                                        | Saves agent conversations as you work                                              | Forward-only; doesn't shelve a dormant project or verify a restore                                                       |
| Git clients, `git bundle`                                                                                  | Repository state and history                                                       | Miss gitignored files that matter; no handoff; no restore verification                                                   |
| Time Machine, Arq, restic, Borg                                                                            | Verified backups                                                                   | Not project-aware; no capture plan or development context                                                                |
| [kondo](https://github.com/tbillington/kondo), npkill, disk cleaners                                       | Reclaim space from `node_modules`, `target`, build output                          | Destructive by design. DORMBIT only reads, and uses similar classification to keep rebuildable output out of the capsule |

DORMBIT combines four things no single tool found in research does together:

1. an evidence-graded reconstruction of the project's state;
2. an approved capture into a verifiable capsule;
3. a transactional, independently verified restore;
4. a handoff that keeps "files restored" separate from "context inferred", "session resumed", and "builds".

**Frankenstein rationale.** DORMBIT stitches together local developer intelligence (filesystem, Git, manifests), AI-agent context awareness, a cryptographic capsule format, a transactional restore, and one Rust core shared by a native app and a WebAssembly demo. The parts depend on each other: the evidence engine shapes what the capsule must contain, and the verifier backs the brief's "files restored" claim.

## 5. Scope

### P0 (must ship)

- Discovery limited to folders the user picks, plus Git snapshot and project metadata.
- Evidence View with the four statuses, including in-repo agent files.
- An editable capture plan with explicit approval.
- Capsule format v1, a separate verification pass, and transactional restore with crash recovery.
- The Resurrection Brief exported as Markdown.
- All 12 native screens, plus the judge demo (WebAssembly core) with corruption tests and reset.
- Property tests and malicious fixtures.
- CI, the public repo, the deployed demo, the ad-hoc-signed DMG release, the video, and the Devpost submission.

### P1 (ship if feasible; cut in this order when behind)

1. Claude Code, Codex, and Kiro-export metadata adapters, including the "context at risk" warning.
2. Writing the brief into the restored project as `RESURRECTION_BRIEF.md`, optionally also as a Kiro steering file.
3. Safe restore of internal relative symlinks.
4. Native end-to-end tests through WebdriverIO (test builds only).
5. Universal (Intel plus Apple Silicon) binary.
6. Detection of more frameworks.
7. Accuracy validation on public repos (Task 48).

### P2 (future, not in v1)

- Encrypted capture (age plus Keychain).
- Deeper session analysis and opt-in local AI summaries.
- Cursor and Gemini adapters, and reading Kiro's local session store if it gets documented.
- Ed25519 capsule signing.
- The macOS App Sandbox with security-scoped bookmarks.
- A storage-analysis view and a dark theme.
- A DORMBIT MCP server so agents can read briefs.
- Sync, team workspaces, and cross-platform builds.
- Partial salvage of damaged capsules.
- Notarized builds.

## 6. Functional requirements

One line per plan requirement. The full EARS text is in plan §4. Each link points to the Kiro spec that formalizes it (the specs are written in Task 8, P01-T03). Spec folder: `.kiro/specs/`.

### Scope and discovery

- **SCP-1:** Folder access comes only from native pickers opened by Rust; the UI uses grant IDs, never raw paths. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **SCP-2:** All filesystem access goes through `cap-std` handles; discovery never follows symlinks; source folders are read-only at the type level. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **DSC-1:** Project roots are detected by markers (`.git`, package manifests, `.kiro`, `AGENTS.md`); a monorepo is one project. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **DSC-2:** Scans are bounded by depth, file count, and time, are cancellable, report progress, and are marked partial when a bound is hit. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **DSC-3:** Each project shows name and its source, `~` path, stack with evidence, Git state, activity, size split, docs, agent context, and preservation status. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **DSC-4:** Abandonment is never inferred from timestamps; dormancy is a manual mark. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **DSC-5:** The Shelf supports search, sort, filter, and multi-select. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)
- **GIT-1:** Git is read with `git2` (no network), the git CLI is never run, command-running config keys are flagged, and remote URL credentials are hidden. → [scope-and-discovery](../../.kiro/specs/scope-and-discovery/requirements.md)

### Project intelligence

- **INT-1:** Every finding has a status (Confirmed, Inferred, Unknown, Unsupported), a rule ID, and at least one evidence reference. → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)
- **INT-2:** Rules cover manifests, documented commands (never run), toolchain files, `.env.example` key names, TODO/FIXME, Git work in progress, Kiro tasks, agent files, `.specstory/` metadata, and standing Unknowns. → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)
- **INT-3:** The system never states intent or history it has no evidence for. → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)

### Agent adapters (P1)

- **AGT-1 (P1):** Each adapter needs consent and its own folder grant, stays read-only, extracts metadata only, never opens credential files, and reports Supported or Unsupported per format. → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)
- **AGT-2 (P1):** Session references are labeled "not resumed by DORMBIT". → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)

### Capture plan

- **CAP-1:** The plan sorts files into Included, Excluded: rebuildable, Excluded: sensitive, Needs decision, Unreadable, and Unsupported, each with a reason code and an editable flag. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CAP-2:** Sensitive files are excluded with no override in v1. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CAP-3:** Secret-like content triggers a warning that must be acknowledged; test strings are generated at runtime and never committed. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CAP-4:** Gitignored files that aren't rebuildable are shown for review; data files need a decision. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CAP-5:** `.git` is captured except `hooks/`, lock files, and fsmonitor state; a working-tree-only option exists. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CAP-6:** Symlinks are recorded but not restored (P0); special files are excluded. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CAP-7:** The plan has a digest; approval records it with acknowledgments; capture includes only approved paths and reports files that appeared later as not captured. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)

### Capsule and verification

- **CPS-1:** Capture runs in three steps (hash and compress, re-hash sources, assemble with the manifest first) and fails with `FILE_CHANGED_DURING_CAPTURE` on any change. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **CPS-2:** The capsule is written to a temp file, synced, renamed into place only if the name is free, and recorded in the registry with its file and manifest SHA-256. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **VER-1:** A separate verifier reopens and strictly parses the capsule, checking marker, headers, padding, hashes, manifest/object match, paths, trailer, and registry. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)
- **VER-2:** The verifier reports Verified, Failed (naming damaged files), or Unsupported, and states its limits. → [capsule-and-verification](../../.kiro/specs/capsule-and-verification/requirements.md)

### Restore

- **RST-1:** The user picks a capsule, a destination parent, and a new folder name; restore is refused if the target exists in any case or normalization variant, or is inside the source project or capsule library. → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)
- **RST-2:** Pre-checks cover verification, free space, exclusive-rename support (`RESTORE_VOLUME_UNSUPPORTED`), case sensitivity, path rules, and normalization collisions. → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)
- **RST-3:** Restore writes into a fresh staging folder with an owner token, using create-new writes, re-hashing, and sync, then commits with `renameatx_np(RENAME_EXCL)`. → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)
- **RST-4:** After commit, restored files are re-hashed independently and Git HEAD and refs are checked (both builds); working-tree status is checked natively. → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)
- **RST-5:** A journal records each step; after a crash the next launch offers recovery; DORMBIT only ever deletes its own staging folders after checking the owner token. → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)
- **RST-6:** `.git/hooks/*` is never restored, nothing is overwritten, and restored content is never executed. → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)
- **RST-7:** The restore report lists files written and verified, mismatches, what wasn't restored and why, risky Git config, executables, the source-unchanged check, and writes outside the destination (must be 0). → [transactional-restore](../../.kiro/specs/transactional-restore/requirements.md)

### Resurrection Brief

- **BRF-1:** The brief is structured JSON rendered to Markdown with fixed sections, and every line cites a finding ID. → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)
- **BRF-2:** The brief reports the four outcomes separately (see §10). → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)
- **BRF-3 (P1):** `RESURRECTION_BRIEF.md` is written into the restored project only after verification, as a separate reported write that the restore report lists as an added file. → [intelligence-and-brief](../../.kiro/specs/intelligence-and-brief/requirements.md)

### Native app and UX

- **NAT-1:** No `fs` or `shell` plugin in the WebView, allowlisted commands, strict CSP, devtools off in release. → [foundation](../../.kiro/specs/foundation/requirements.md) (Task 10)
- **NAT-2:** Untrusted project text is rendered without raw HTML, logs hide sensitive details, and there is no telemetry. → [native-experience](../../.kiro/specs/native-experience/requirements.md)
- **UX-1:** All 13 screens (12 native plus the Judge Demo) support keyboard access, VoiceOver labels, and reduced motion, and never use color as the only signal. → [native-experience](../../.kiro/specs/native-experience/requirements.md)

### Judge demo

- **DEM-1:** The demo always shows "Synthetic Demo — No Local Files Accessed", runs the real Rust core in WebAssembly over an in-memory filesystem, and implements the 12 steps with corruption tests, a collision test, reset, and downloads. → [judge-demo](../../.kiro/specs/judge-demo/requirements.md)
- **DEM-2:** The folder picker is a labeled mock, hashes are real, the independent re-hash uses WebCrypto, and any pacing is labeled with the real elapsed time. → [judge-demo](../../.kiro/specs/judge-demo/requirements.md)
- **DEM-3:** The Dockline Invoices fixture is a fictional invoicing app committed once as `fixtures/dockline/dockline-invoices.tar`, the single source for golden tests natively and in the browser. → [foundation](../../.kiro/specs/foundation/requirements.md) (generator, Task 12) and [judge-demo](../../.kiro/specs/judge-demo/requirements.md) (use)

### Delivery (no plan requirement IDs)

The plan gives the release and submission work task IDs, not requirement IDs. It is formalized in [release-and-submission](../../.kiro/specs/release-and-submission/requirements.md) (P10, P12–P18). Hackathon requirements it covers: a working app URL that stays free through Jan 29, 2027; an English description; a public YouTube video under 3:00 showing the app on its target device; a public GitHub repo with an MIT license visible in About and `/.kiro` at the root; a Kiro usage write-up; exactly one category (Frankenstein).

## 7. User journeys

1. **First launch:** Welcome → Privacy (what DORMBIT does and doesn't do: local-only, no deletion, consent for agent histories) → pick a parent folder → scan with live progress → Shelf.
2. **Understand:** open a project → Evidence View → filter by status → jump to the source of any finding.
3. **Preserve:** Plan → resolve "Needs decision" items → acknowledge warnings → choose a capsule library folder → Approve → Capture (live state machine) → separate verification pass → the capsule seal appears.
4. **Re-verify later:** open a capsule from an external drive → Verify → registry comparison.
5. **Restore and resume:** Restore Wizard → pre-checks → staging → commit → result → Brief → export (P1: also written into the restored project for an agent to read).
6. **Corruption:**
   - A damaged capsule fails verification, and restore is blocked.
   - Damage introduced after verification is caught during the restore. The restore rolls back, and the destination is never created.
7. **Agent consent (P1):** Settings → enable the Claude Code adapter → pick `~/.claude/projects` → session metadata appears with the "context at risk" note.
8. **Judge demo:** open the URL → banner → guided rail through the 12 steps:
   1. Discover Dockline.
   2. Open its details.
   3. Inspect the stack and the unfinished work.
   4. Review the plan.
   5. Approve it.
   6. Run the capture state machine.
   7. Verify integrity.
   8. Restore to a virtual destination (including a refused collision with an existing folder).
   9. Inspect the independent verification.
   10. Read the brief.
   11. Run a corruption test.
   12. Observe the failure, then reset.
9. **Revoke:** Settings → remove a folder grant or consent. Cached data for it is cleared.
10. **Crash recovery:** on the next launch, DORMBIT detects the unfinished restore and offers to discard its staging folder.

## 8. Non-goals

- **Not a cleaner.** DORMBIT never deletes project files or reclaims space. Rebuildable classification only shapes the capture plan.
- **Never deletes user data.** The only thing DORMBIT ever deletes is its own staging folders, and only after checking the owner token (RST-5). Restore never overwrites anything (RST-6).
- **Never runs code.** No project scripts, no restored content, no git CLI, no documented commands (they are listed, never run). Build status is always reported as unknown.
- **No cloud, no accounts, no telemetry.** Everything runs locally. The judge demo is a static page with no analytics.
- **Never claims to resume an AI session.** Session references are labeled "not resumed by DORMBIT", and the brief reports "AI session resumed: not performed".
- **Never claims a restored project builds or runs.** The brief reports "Builds/runs: unknown (never executed)".
- **Doesn't infer abandonment or intent.** Dormancy is a manual mark (DSC-4), and the system never states intent or history without evidence (INT-3).
- **Not an agent-history viewer.** Existing tools cover browsing histories. Adapters read metadata only (P1).
- **No encryption in v1.** Sensitive files and external transcripts are never captured, and the app says so. Encryption is P2 ([ADR-007](../adr/ADR-007-no-encryption-v1.md)).

## 9. Success and validation metrics

**Accuracy and safety (plan §2):**

- On 5 public open-source repos with different stacks, at least 90% of Confirmed findings are judged correct against the source, and every Inferred finding cites evidence (Task 48, P1).
- 100% of restores in the test corpus are byte-verified, and no malicious fixture is accepted.
- A first-time user completes all 12 demo steps without help, checked by a Playwright run plus one manual walkthrough by the user.

**Engineering gates (plan §13–15):**

- PROP-1 to PROP-12 pass, run at 10,000 cases locally and 512 in CI.
- At least 25 attack fixtures, each rejected with its expected error code (Task 42).
- The 12-step demo passes on Chromium, WebKit, and Firefox (Task 41).
- axe reports no critical issues; accessibility meets WCAG AA (Tasks 41, 47).
- A result counts only if it was recorded through `scripts/evidence/run.sh` or CI.

**Delivery:**

- Submission targeted for Wed Dec 2, 2026, 12:00 PM PT; hard deadline Fri Dec 4, 2026, 2:00 PM PT.
- Video under 3:00 (target 2:35–2:45), public on YouTube.
- Demo URL available through Jan 29, 2027, watched by a daily smoke test.

**Not measured:** adoption. It can't be validated in five weeks, and the final report (`docs/status/FINAL_REPORT.md`) says so. We validate accuracy and the workflow only.

## 10. The four outcomes

The brief, the Restoration Result screen, the demo, the README, the video, and the Devpost text all use this wording, reported as four separate lines (BRF-2):

1. **Files restored and verified.**
2. **Context reconstructed (inferred from evidence).**
3. **AI session resumed: not performed.**
4. **Builds/runs: unknown (never executed).**

The first line appears only when the post-commit re-hash (RST-4) passed. No copy may merge these outcomes or imply that a restored project resumes or builds. Task 48 checks all wording against this list.

## Open questions

- **Target users.** The plan doesn't name personas. §3 derives two from plan §1–2. Confirm or replace during spec review.
- **Screen count.** UX-1 says 13 screens; P08 says 12 native screens. This PRD reads them as consistent: 12 native screens plus the Judge Demo (screen 13 in plan §9).
- **Spec links.** The linked `requirements.md` files are created in Task 8. Links will resolve once those specs exist.
