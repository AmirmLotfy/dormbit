# Requirements Document

## Introduction
This spec covers project intelligence and the Resurrection Brief: master plan Phase 06 (P06, Tasks 27-30) and the agent adapters of Phase 07 (P07, Tasks 31-33, all P1). It formalizes INT-1..INT-3, AGT-1, AGT-2, and BRF-1..BRF-3, the `Finding` and `Brief` models from master plan §8, and the "Agent-store privacy" threat from §7. It also applies NAT-2 (no raw HTML for untrusted text) to the Evidence View and Brief screens. The correctness anchor is PROP-12: every finding has evidence that resolves, and every line of the brief cites an existing finding.

## Glossary
- **Project**: a project root found by discovery (scope-and-discovery, DSC-1) inside a Read scope grant. Described by a `ProjectRecord`.
- **Scope grant**: a folder the user picked in a native picker opened by Rust (SCP-1). The UI refers to it only by grant ID.
- **Agent store grant (P1)**: a scope grant that a user creates for one agent adapter, after consent, separate from the Read grants used by discovery.
- **Vfs**: the read-only filesystem trait that `dormbit-core` uses instead of the disk. Natively it wraps `cap-std` handles; in the browser it is an in-memory filesystem.
- **File index**: the walker's list of files in a project, with each file's classification (text, binary, gitignored, rebuildable, sensitive, symlink). Produced by scope-and-discovery Task 16.
- **Finding**: one statement about a project, with fields `{id, rule, statement, status, evidence[]}` (master plan §8).
- **Status**: the grade of a Finding. One of:
  - **Confirmed**: read directly from a cited source.
  - **Inferred**: derived by a named rule from Confirmed Findings.
  - **Unknown**: no evidence either way.
  - **Unsupported**: the source is something DORMBIT can't read.
- **Rule**: a named check that produces Findings. Each rule has a stable **rule ID** (for example `work.todo`), a list of statuses it may emit, and the brief section its Findings belong to.
- **Fact rule**: a rule that reads sources (files, the Git snapshot, restore facts) and emits Confirmed, Unknown, or Unsupported Findings.
- **Derived rule**: a rule that reads only Confirmed Findings, never sources, and emits Inferred Findings.
- **Rule_Registry**: the list of all rules, in a fixed order, that the Evidence_Engine runs.
- **Evidence reference**: a pointer from a Finding to its source. One of four kinds: a file and line range, a Git object, a manifest key, or a file stat.
- **Evidence source**: where an Evidence reference points: a project (through its Read grant), a capsule (by capsule ID), a restored project (by restore ID), or an agent store (by grant ID, P1).
- **Resolve**: check that an Evidence reference still points at what it cites. The result is Resolved, Stale, Missing, or NotAccessible.
- **Standing Unknowns**: three Findings that are always Unknown: why the project paused, whether it builds, and where it deploys.
- **Sensitive file**: a file the CAP-2 classifier marks sensitive: `.env*` except example files, private keys, `.pem`, `.p12`, and credential files.
- **Example env file**: `.env.example` and any other name the CAP-2 classifier treats as an example file.
- **Secret detector**: the content check from CAP-3 that flags text that looks like a secret.
- **Untrusted text**: any text that comes from a project, a Git repository, or an agent store, such as README lines, TODO comments, commit summaries, and task titles.
- **Evidence_Engine**: the component in `dormbit-core` that runs the Rule_Registry over a project and returns its Findings.
- **Evidence_Resolver**: the component in `dormbit-core` that resolves Evidence references and returns excerpts.
- **InRepo_Parser**: the rules in `dormbit-agents` that read agent-related files inside a project: `.kiro/specs/*/tasks.md`, `.kiro/steering/*.md`, `AGENTS.md`, `CLAUDE.md`, `.cursor/rules`, and `.specstory/`.
- **Task line**: a line in a `tasks.md` file, outside fenced code blocks, that starts with a Markdown checkbox list item (`- [ ]`, `- [x]`, `- [X]`), optionally followed by `*`.
- **Brief (Resurrection Brief)**: the handoff document for a project, stored as structured JSON with fields `{sections[], outcomes{files, context, session, build}, evidence_index}` (master plan §8) and rendered to Markdown.
- **Citation**: a Finding ID written on a brief line in the form `[F-` followed by 12 lowercase hex characters and `]`.
- **Template line**: a line of the rendered brief that is fixed text from DORMBIT and states nothing about the project, such as a heading or the "AI session resumed: not performed." outcome. The full list is in design.md.
- **Outcome**: one of the four separate results the brief reports (BRF-2): files, context, session, build.
- **Verified restore**: a restore whose report (transactional-restore, RST-4 and RST-7) shows every file written and verified, 0 mismatches, and a passing post-commit re-hash.
- **Brief_Generator**: the component in `dormbit-core` that builds a Brief from Findings and optional restore facts.
- **Brief_Renderer**: the component in `dormbit-core` that turns a Brief into Markdown bytes.
- **Brief_Exporter**: the component in `src-tauri` and `dormbit-fs` that writes the rendered brief to a file the user chose.
- **Command_Layer**: the allowlisted Tauri commands in `apps/desktop/src-tauri` used by this spec: `evidence_get`, `brief_get`, `brief_export`, `consent_set`, `adapters_list`.
- **Evidence_View**: screen 5 in master plan §9, with tabs for Overview, Evidence, Files, Git, Agent context, and History.
- **Brief_Screen**: screen 11 in master plan §9, the Resurrection Brief.
- **Agent adapter (P1)**: a reader for one external agent tool's local storage: Claude Code, Codex, or Kiro export.
- **Adapter_Framework (P1)**: the shared code in `dormbit-agents` that enforces consent, grants, the credential deny-list, metadata-only parsing, and format reporting for all adapters.
- **Consent record (P1)**: a stored record that the user agreed to let one adapter read metadata: `{adapter_id, granted_at, consent_text_version, grant_id?}`.
- **Credential deny-list (P1)**: file paths and names that no adapter ever opens: `~/.claude/.credentials.json`, `~/.codex/auth.json` (or `auth.json` at the root of `CODEX_HOME`), and any file named `.credentials.json` or `auth.json` inside an agent store grant.
- **Format (P1)**: one kind of file an adapter knows about, such as "Claude Code session JSONL". Each format is reported as Supported or Unsupported.
- **Session reference (P1)**: the metadata for one agent session: session ID, working directory, first and last timestamps, Git branch, and tool version.
- **Claude_Code_Adapter, Codex_Adapter, Kiro_Export_Adapter (P1)**: the three adapters.
- **Keys_Probe (P1)**: a `dormbit-cli` subcommand that the user runs on their own file to print its JSON key paths and value types, never values.
- **Dockline**: the synthetic "Dockline Invoices" fixture (DEM-3), stored as `fixtures/dockline/dockline-invoices.tar`.

## Requirements

### Requirement 1: Every finding has a status, a rule, and evidence (INT-1)

**User Story:** As a developer returning to a dormant project, I want every statement DORMBIT makes to carry a status and a source, so that I can tell what was read from what was guessed.

#### Acceptance Criteria

1. THE Evidence_Engine SHALL give every Finding exactly one Status from this set: Confirmed, Inferred, Unknown, Unsupported.
2. THE Evidence_Engine SHALL assign Confirmed only to a Finding whose statement is read directly from the source named by its Evidence references.
3. THE Evidence_Engine SHALL assign Inferred only to a Finding emitted by a Derived rule from Confirmed Findings.
4. THE Evidence_Engine SHALL assign Unknown to a Finding when the sources it checked contain no evidence either way.
5. THE Evidence_Engine SHALL assign Unsupported to a Finding whose source is a file or format DORMBIT can't read or parse.
6. THE Evidence_Engine SHALL give every Finding a rule ID that is registered in the Rule_Registry.
7. THE Evidence_Engine SHALL give every Finding at least one Evidence reference, and each reference SHALL be one of four kinds: a file and line range, a Git object, a manifest key, or a file stat.
8. IF a rule emits a Finding with no Evidence reference, an unregistered rule ID, or a status that rule is not allowed to emit, THEN THE Evidence_Engine SHALL drop that Finding and add a `rule_violation` entry naming the rule ID to the result's diagnostics.
9. THE Evidence_Engine SHALL derive each Finding ID from the Finding's rule ID and its Evidence references, so that the same inputs produce the same ID on every run.
10. THE Evidence_Engine SHALL serialize each Finding with exactly the fields `id`, `rule`, `statement`, `status`, and `evidence`.

### Requirement 2: Evidence references resolve (INT-1, PROP-12)

**User Story:** As a developer, I want to jump from any finding to its source, so that I can check DORMBIT's claim myself.

#### Acceptance Criteria

1. WHEN `evidence_get` is called with a project ID and a finding ID, THE Evidence_Resolver SHALL return one resolution state per Evidence reference of that Finding: Resolved, Stale, Missing, or NotAccessible.
2. THE Evidence_Resolver SHALL return Resolved for a file-and-line reference only if the file exists in the cited source, `1 <= start_line <= end_line <= line count`, and the reference's anchor text (when present) appears within those lines.
3. THE Evidence_Resolver SHALL return Resolved for a Git object reference only if the cited commit, ref, stash entry, or config key exists in the project's Git data.
4. THE Evidence_Resolver SHALL return Resolved for a manifest key reference only if the cited file parses and contains the cited key path.
5. THE Evidence_Resolver SHALL return Resolved for a file stat reference only if the path's existence and kind match the recorded stat, and SHALL return Stale if the existence and kind match but the size or modification time differs.
6. WHEN a file-and-line reference in a project or restored project resolves, THE Evidence_Resolver SHALL return an excerpt of the cited lines, at most `Limits::max_excerpt_lines` lines, with any text the Secret detector flags replaced by `[hidden: looks like a secret]`.
7. IF an Evidence reference points at an agent store (P1), THEN THE Evidence_Resolver SHALL return no excerpt.
8. IF the scope grant behind an Evidence reference has been revoked, THEN THE Evidence_Resolver SHALL return NotAccessible without reading anything.
9. IF an Evidence reference path is absolute, contains `..`, contains a NUL or control character, or leaves its grant, THEN THE Evidence_Resolver SHALL return the error `PATH_UNSAFE` without reading anything.
10. IF `evidence_get` receives a project ID or finding ID that does not exist, THEN THE Command_Layer SHALL return the error `SCOPE_DENIED`.

### Requirement 3: No claims without evidence (INT-1, INT-3)

**User Story:** As a developer, I want DORMBIT to say "unknown" instead of guessing, so that the handoff never misleads me about why the project stopped or what I meant to do.

#### Acceptance Criteria

1. THE Evidence_Engine SHALL build every Finding statement from its rule's fixed template, and SHALL insert Untrusted text only into quoted or code slots of that template.
2. THE Evidence_Engine SHALL NOT emit a statement about the author's intent, plans, reasons, or project history, except as a quote of cited project text that names its file and line.
3. THE Evidence_Engine SHALL emit the three Standing Unknowns for every project, with rule IDs `unknown.pause_reason`, `unknown.builds`, and `unknown.deploy_target`, status Unknown, and a file stat reference to the project root.
4. WHEN a Derived rule runs, THE Evidence_Engine SHALL give it only the Confirmed Findings from the fact phase, with no access to the Vfs, the Git snapshot, or restore facts.
5. THE Evidence_Engine SHALL set the Evidence references of an Inferred Finding to the union of the Evidence references of the Confirmed Findings it was derived from.
6. THE Evidence_Engine SHALL NOT emit any Finding that infers abandonment or dormancy from timestamps (DSC-4).

### Requirement 4: Manifests, stack, and toolchain files (INT-2)

**User Story:** As a developer, I want to see the project's stack and toolchain versions with their sources, so that I know what to install before I start.

#### Acceptance Criteria

1. WHEN the File index contains a package manifest recognized by the discovery walker, THE Evidence_Engine SHALL emit one Confirmed `stack.manifest` Finding per manifest, citing a file stat.
2. WHEN `package.json` lists a dependency from the stack detection table in `dependencies` or `devDependencies`, THE Evidence_Engine SHALL emit a Confirmed `stack.dependency` Finding citing the manifest key `/dependencies/<name>` or `/devDependencies/<name>`.
3. WHEN the Confirmed `stack.dependency` Findings match a combination in the stack detection table, THE Evidence_Engine SHALL emit an Inferred `stack.framework` Finding naming that combination.
4. WHEN `.nvmrc` exists, THE Evidence_Engine SHALL emit a Confirmed `toolchain.file` Finding that quotes the version and cites line 1 of `.nvmrc`.
5. WHEN `rust-toolchain.toml` contains a `channel` line, THE Evidence_Engine SHALL emit a Confirmed `toolchain.file` Finding that quotes the channel and cites that line.
6. WHEN `package.json` contains an `engines` object, THE Evidence_Engine SHALL emit one Confirmed `toolchain.file` Finding per key, citing the manifest key `/engines/<name>`.
7. IF a manifest or toolchain file fails to parse, THEN THE Evidence_Engine SHALL emit an Unsupported `source.unsupported` Finding citing a file stat of that file, and SHALL emit no other Findings from that file.
8. WHEN the Evidence_Engine runs on Dockline, THE Evidence_Engine SHALL emit Confirmed `stack.dependency` Findings for React, Vite, TypeScript, Fastify, and Drizzle, matching the golden snapshot.

### Requirement 5: Documented commands are listed, never run (INT-2)

**User Story:** As a developer, I want the project's documented commands listed, so that I can choose what to run myself after reading the pre-run checks.

#### Acceptance Criteria

1. WHEN `package.json` contains a `scripts` object, THE Evidence_Engine SHALL emit one Confirmed `cmd.documented` Finding per script that quotes the script's name and command, says "listed, not run", and cites the manifest key `/scripts/<name>`.
2. WHEN a `README*` file at the project root or a Markdown file under `docs/` contains a fenced code block whose info string is `sh`, `bash`, `shell`, `console`, or `zsh`, THE Evidence_Engine SHALL emit one Confirmed `cmd.documented` Finding per non-blank, non-comment line of that block, citing that line.
3. THE Evidence_Engine SHALL NOT execute any documented command, script, or project file.
4. THE crates `dormbit-core` and `dormbit-agents` SHALL contain no use of `std::process` or any other process-spawning API.
5. IF the Secret detector flags a documented command, THEN THE Evidence_Engine SHALL replace the quoted command with `[hidden: looks like a secret]` in the Finding statement.

### Requirement 6: Example env keys, never values; sensitive files never read (INT-2, CAP-2)

**User Story:** As a developer, I want to know which environment keys the project expects, so that I can supply them after a restore, without DORMBIT ever reading or showing secret values.

#### Acceptance Criteria

1. WHEN an Example env file exists, THE Evidence_Engine SHALL emit one Confirmed `env.example_key` Finding per key, quoting the key name and citing its line with the key name as the anchor.
2. THE Evidence_Engine SHALL NOT copy any value from an Example env file into any Finding, Brief, excerpt, log line, cache, or IPC payload.
3. THE Evidence_Engine SHALL NOT read the contents of any Sensitive file.
4. WHEN a Sensitive file is present in the File index, THE Evidence_Engine SHALL emit a Confirmed `sensitive.present` Finding that cites only a file stat and states that the file was not read and is never captured.
5. IF a line of an Example env file is not blank, not a `#` comment, and not in `KEY=VALUE` or `export KEY=VALUE` form, THEN THE Evidence_Engine SHALL skip the line and report the number of skipped lines in a Confirmed `env.example_skipped` Finding.

### Requirement 7: TODO/FIXME comments and skipped tests (INT-2)

**User Story:** As a developer, I want to see the unfinished markers left in the code, with exact line numbers, so that I can find where I stopped.

#### Acceptance Criteria

1. WHEN a scanned text file contains `TODO` or `FIXME` as a whole word (case-sensitive), THE Evidence_Engine SHALL emit one Confirmed `work.todo` Finding per occurrence, citing that line with the token as the anchor and quoting the text after the token, truncated to `Limits::max_quote_chars` characters.
2. WHEN a scanned file contains a skipped-test marker, THE Evidence_Engine SHALL emit one Confirmed `work.skipped_test` Finding per marker, citing that line. The markers are `it.skip(`, `test.skip(`, `describe.skip(`, `xit(`, `xtest(`, and `xdescribe(` in `.js`, `.jsx`, `.ts`, `.tsx`, `.mjs`, and `.cjs` files; `#[ignore` in `.rs` files; and `@pytest.mark.skip` and `@unittest.skip` in `.py` files.
3. THE Evidence_Engine SHALL scan only files in the File index that are text, not gitignored, not rebuildable, not inside `.git`, not Sensitive files, and at most `Limits::max_text_file_bytes` bytes.
4. IF a file is larger than `Limits::max_text_file_bytes` or is not valid UTF-8, THEN THE Evidence_Engine SHALL skip it and report the count of skipped files in a Confirmed `scan.skipped_files` Finding.
5. IF a rule produces more than `Limits::max_findings_per_rule` Findings, THEN THE Evidence_Engine SHALL keep the first `Limits::max_findings_per_rule` in path and line order and emit one Confirmed `scan.truncated` Finding stating how many were not listed.
6. WHEN the Evidence_Engine runs on Dockline, THE Evidence_Engine SHALL emit `work.todo` Findings for the TODO and FIXME lines in `src/pdf/renderInvoice.ts` and one `work.skipped_test` Finding, matching the golden snapshot.

### Requirement 8: Git work state (INT-2, GIT-1)

**User Story:** As a developer, I want to see uncommitted changes, branches, WIP commits, and stashes, so that I don't lose work I forgot about.

#### Acceptance Criteria

1. WHEN the Git snapshot reports modified, staged, untracked, deleted, or conflicted files, THE Evidence_Engine SHALL emit a Confirmed `git.uncommitted` Finding with the count per kind, citing the HEAD commit and a file stat for each listed path, up to `Limits::max_findings_per_rule` paths.
2. WHEN HEAD points at a branch, THE Evidence_Engine SHALL emit a Confirmed `git.branch` Finding citing the ref `refs/heads/<branch>`.
3. WHEN HEAD is detached, THE Evidence_Engine SHALL emit a Confirmed `git.detached` Finding citing the HEAD commit.
4. WHEN the checked-out branch is neither `main` nor `master`, THE Evidence_Engine SHALL emit an Inferred `git.feature_branch` Finding derived from the `git.branch` Finding.
5. WHEN the HEAD commit summary starts with `wip` (any case) followed by the end of the summary or a non-letter character, THE Evidence_Engine SHALL emit a Confirmed `git.wip_commit` Finding citing the HEAD commit.
6. WHEN the stash count is greater than 0, THE Evidence_Engine SHALL emit a Confirmed `git.stash` Finding with the count, citing `refs/stash`.
7. WHEN an upstream is configured, THE Evidence_Engine SHALL emit a Confirmed `git.upstream` Finding with ahead and behind counts labeled "as of last fetch".
8. IF the project has no `.git`, THEN THE Evidence_Engine SHALL emit a Confirmed `git.absent` Finding citing a file stat that records `.git` as absent.
9. THE Evidence_Engine SHALL take remote URLs only from the snapshot's redacted remote list, so that credentials in remote URLs never appear in any Finding.
10. THE Evidence_Engine SHALL read Git data only from the `GitSnapshot` and the core ref reader, and SHALL NOT run the git CLI.
11. WHEN the Evidence_Engine runs on Dockline, THE Evidence_Engine SHALL report branch `feature/pdf-export`, 2 modified files, 1 untracked file, and 1 stash, matching the golden snapshot.

### Requirement 9: Kiro spec tasks and steering files (INT-2, Task 28)

**User Story:** As a developer who planned the work in Kiro, I want to see how many spec tasks are done and which are left, so that I can resume at the right task.

#### Acceptance Criteria

1. WHEN `.kiro/specs/<name>/tasks.md` exists, THE InRepo_Parser SHALL count as top-level tasks the Task lines that start at column 0, and SHALL ignore lines inside fenced code blocks.
2. THE InRepo_Parser SHALL count a top-level task as complete only if its checkbox marker is `x` or `X`.
3. THE InRepo_Parser SHALL emit one Confirmed `kiro.tasks_progress` Finding per `tasks.md` with the statement "<done> of <total> tasks complete", citing the line range from the first to the last top-level Task line.
4. THE InRepo_Parser SHALL emit one Confirmed `kiro.task_unchecked` Finding per incomplete top-level task, quoting its title and citing its line.
5. WHEN a `tasks.md` contains indented Task lines, THE InRepo_Parser SHALL report them in a separate Confirmed `kiro.subtasks_progress` Finding and SHALL NOT include them in the top-level count.
6. IF a Task line has a checkbox marker other than a space, `x`, or `X`, THEN THE InRepo_Parser SHALL count it as a task that is not complete.
7. IF a `tasks.md` contains no Task lines, THEN THE InRepo_Parser SHALL emit a Confirmed `kiro.tasks_progress` Finding stating "no task lines found".
8. IF a `tasks.md` is not valid UTF-8 or is larger than `Limits::max_text_file_bytes`, THEN THE InRepo_Parser SHALL emit an Unsupported `source.unsupported` Finding citing a file stat.
9. WHEN the InRepo_Parser runs on Dockline, THE InRepo_Parser SHALL emit a `kiro.tasks_progress` Finding for `.kiro/specs/pdf-export/tasks.md` whose statement contains "3 of 7 tasks complete".
10. WHEN `.kiro/steering/*.md` files exist, THE InRepo_Parser SHALL emit one Confirmed `kiro.steering` Finding per file, quoting its front-matter `inclusion` value when present and citing that line, or citing a file stat when absent.

### Requirement 10: In-repo agent instruction files and `.specstory/` (INT-2)

**User Story:** As a developer, I want DORMBIT to point me to the agent instructions kept in my repo, so that I can hand them to an agent after a restore.

#### Acceptance Criteria

1. WHEN the File index contains `AGENTS.md` or `CLAUDE.md`, THE InRepo_Parser SHALL emit one Confirmed `agent.instructions` Finding per file with its line count, citing a file stat.
2. WHEN an `AGENTS.md` or `CLAUDE.md` file contains H1 or H2 headings, THE InRepo_Parser SHALL quote up to `Limits::max_headings` of them in that file's Finding, citing each heading line.
3. WHEN `.cursor/rules/` contains files, THE InRepo_Parser SHALL emit one Confirmed `agent.instructions` Finding per file with its line count, citing a file stat.
4. WHEN `.specstory/` exists, THE InRepo_Parser SHALL emit one Confirmed `specstory.meta` Finding with the number of files and the earliest and latest modification dates, citing a file stat of the folder that records those values.
5. THE InRepo_Parser SHALL NOT open any file under `.specstory/`.
6. THE InRepo_Parser SHALL read in-repo agent files through the project's Read grant as ordinary project files, with no consent step.

### Requirement 11: Bounds, partial results, and determinism (INT-1, DSC-2)

**User Story:** As a developer with a large project, I want evidence collection to stay bounded and say when it is incomplete, so that I never mistake a partial result for a full one.

#### Acceptance Criteria

1. WHEN an evidence run hits a discovery bound (depth, file count, or time), THE Evidence_Engine SHALL mark the result `partial: true` and emit a Confirmed `scan.partial` Finding naming the bound that was hit.
2. THE Evidence_Engine SHALL produce byte-identical serialized results for the same Vfs contents, Git snapshot, restore facts, and Clock value, regardless of the order in which the Vfs lists directory entries.
3. THE Evidence_Engine SHALL NOT follow symlinks, and SHALL cite a symlink only by a file stat with kind Symlink.
4. THE Evidence_Engine SHALL read project files only through the Vfs of the project's Read grant.
5. WHEN a scope grant is revoked, THE Command_Layer SHALL delete the cached evidence results for every project in that grant.

### Requirement 12: Brief structure and citations (BRF-1)

**User Story:** As a developer resuming a project, I want a single handoff document where every line points at its evidence, so that I can trust it or check it.

#### Acceptance Criteria

1. THE Brief_Generator SHALL produce a Brief as structured JSON with the fields `sections`, `outcomes` (with `files`, `context`, `session`, `build`), and `evidence_index`.
2. THE Brief_Generator SHALL emit the sections in this order: Identity, Technologies, Repo state, Docs, Verified content, Unfinished work, Evidence-backed next steps, Context references, Missing information, First steps, Pre-run checks.
3. THE Brief_Generator SHALL give every section line at least one citation of a Finding ID.
4. THE Brief_Generator SHALL include in `evidence_index` every Finding of the project's evidence result, so that every cited Finding ID exists in the index.
5. THE Brief_Renderer SHALL write each citation as `[F-<12 hex>]` at the end of the line it supports.
6. THE Brief_Renderer SHALL produce Markdown in which every line is either a Template line or contains at least one citation of a Finding ID present in `evidence_index`.
7. WHEN a section has no Findings, THE Brief_Renderer SHALL render the Template line "No findings for this section." under its heading.
8. THE Brief_Generator SHALL list in Pre-run checks each lifecycle script (`preinstall`, `install`, `postinstall`, `prepare` in `package.json`), each editor auto-run config (a task in `.vscode/tasks.json` with `runOptions.runOn` set to `folderOpen`), and each risky Git config key flagged under GIT-1, by its exact key.
9. WHEN none of the pre-run hazards in criterion 8 is found, THE Evidence_Engine SHALL emit a Confirmed `prerun.none` Finding that cites a file stat for each source it checked.
10. WHEN a Verified restore exists for the requested restore ID, THE Brief_Generator SHALL list in Verified content the capsule ID, the number of files written and verified, the mismatch count, what wasn't restored and why, and the Git HEAD check result, each line citing a `restore.*` Finding.
11. THE Brief_Generator SHALL build Evidence-backed next steps only from `kiro.task_unchecked`, `work.todo`, `work.skipped_test`, `git.uncommitted`, and `git.stash` Findings, each phrased as an item to review.
12. THE Brief_Renderer SHALL produce byte-identical Markdown for the same Brief, and SHALL take the generation date only from the injected Clock.
13. THE Brief_Generator SHALL write paths relative to the project root, or with the home folder written as `~`, and SHALL NOT include an absolute path that starts with the user's home folder.

### Requirement 13: Four separate outcomes (BRF-2)

**User Story:** As a developer, I want the brief to keep "files restored" separate from "context inferred", "session resumed", and "builds", so that a verified restore is never read as a promise that the project works.

#### Acceptance Criteria

1. THE Brief_Renderer SHALL render the four outcomes as four separate lines, in this order: files, context, session, build. The lines SHALL start with the labels "Files restored and verified" and "Context reconstructed (inferred from evidence)", and the last two lines SHALL be exactly "AI session resumed: not performed." and "Builds/runs: unknown (never executed)." followed only by citations.
2. WHEN the restore report for the requested restore ID shows every file written and verified, 0 mismatches, and a passing post-commit re-hash (RST-4), THE Brief_Renderer SHALL render the files outcome as "Files restored and verified." followed by the file count and mismatch count, citing the `restore.verified` Finding.
3. IF no Verified restore is recorded for the requested project and restore ID, THEN THE Brief_Renderer SHALL render the files outcome as "Files restored and verified: none yet. No verified restore is recorded for this project." citing an Unknown `restore.none` Finding.
4. THE Brief_Generator SHALL set the session outcome to "AI session resumed: not performed." in every case, including when session references exist.
5. THE Brief_Generator SHALL set the build outcome to "Builds/runs: unknown (never executed)." in every case, citing the `unknown.builds` Finding.
6. THE Brief_Renderer SHALL follow the context outcome wording with the number of Findings per Status taken from `evidence_index`.
7. THE Brief_Renderer SHALL use the word "resumed" only in the session outcome line and in the label "not resumed by DORMBIT".

### Requirement 14: Untrusted text is rendered without raw HTML (NAT-2)

**User Story:** As a developer, I want README text, commit messages, and TODO comments shown as plain text, so that a hostile project can't inject markup into DORMBIT or into the exported brief.

#### Acceptance Criteria

1. THE Brief_Renderer SHALL write `<`, `>`, and `&` in Untrusted text outside code spans as `&lt;`, `&gt;`, and `&amp;`.
2. THE Brief_Renderer SHALL replace line breaks, tabs, and other control characters in Untrusted text with a single space, so that Untrusted text can't start a new Markdown block.
3. THE Brief_Renderer SHALL backslash-escape the Markdown punctuation characters ``\ ` * _ { } [ ] ( ) # + - . ! | ~`` in Untrusted text outside code spans, so that it can't form links, images, emphasis, headings, lists, or tables.
4. WHEN the Brief_Renderer writes Untrusted text as a code span, THE Brief_Renderer SHALL delimit it with a backtick run longer than the longest backtick run inside the text.
5. THE Brief_Renderer SHALL replace the Unicode bidirectional control characters U+202A to U+202E and U+2066 to U+2069 in Untrusted text with their visible escape form `\u{XXXX}`.
6. THE Evidence_View SHALL render Finding statements and excerpts as React text nodes, never as HTML.
7. THE Brief_Screen SHALL render the brief Markdown with react-markdown and no raw HTML support.
8. THE Command_Layer SHALL log only IDs, rule IDs, statuses, and counts for this spec's commands, and SHALL NOT log Finding statements, excerpts, or env key names.

### Requirement 15: Brief export through a Rust-opened save dialog (Task 29)

**User Story:** As a developer, I want to save the brief as a Markdown file where I choose, so that I can keep it with the restored project or share it.

#### Acceptance Criteria

1. WHEN `brief_export` is called, THE Brief_Exporter SHALL open a native save dialog from Rust with a default file name of `<project-slug>-resurrection-brief.md`.
2. THE Command_Layer SHALL accept only a project ID and an optional restore ID for `brief_export`, and SHALL NOT accept a path from the WebView.
3. WHEN the user confirms a location, THE Brief_Exporter SHALL create the file with create-new semantics, write bytes identical to the `markdown` returned by `brief_get` for the same IDs, sync it, and return the display path (with `~`), the byte count, and the SHA-256.
4. IF the user cancels the save dialog, THEN THE Brief_Exporter SHALL return `CANCELLED` and write nothing.
5. IF a file already exists at the chosen path, THEN THE Brief_Exporter SHALL return `EXPORT_DEST_EXISTS` and leave the existing file unchanged.
6. IF writing fails after the file was created, THEN THE Brief_Exporter SHALL return `EXPORT_WRITE_FAILED` naming the display path of the partial file, and SHALL NOT delete it.
7. THE Brief_Exporter SHALL write exactly one file per successful export and nothing else outside DORMBIT's app data.

### Requirement 16: Brief written into the restored project (BRF-3) (P1)

**User Story:** As a developer, I want the restored project to contain the brief, so that an agent working in it can read the handoff.

#### Acceptance Criteria

1. WHERE BRF-3 is enabled, WHEN a restore becomes a Verified restore, THE Brief_Exporter SHALL write `RESURRECTION_BRIEF.md` at the restored project root as a separate write after the post-commit re-hash.
2. IF `RESURRECTION_BRIEF.md` already exists in the restored project, THEN THE Brief_Exporter SHALL leave it unchanged and record "not written: file already exists" in the restore report.
3. WHEN `RESURRECTION_BRIEF.md` is written, THE Brief_Exporter SHALL record it in the restore report as an added file that is not part of the capsule, with its size and SHA-256.
4. IF the restore did not become a Verified restore, THEN THE Brief_Exporter SHALL NOT write `RESURRECTION_BRIEF.md`.
5. WHERE the user opts in to the steering copy, THE Brief_Exporter SHALL also write `.kiro/steering/resurrection-brief.md` with front matter `inclusion: manual`, under the same create-new and reporting rules as criteria 2 and 3.
6. THE Brief_Exporter SHALL exclude the added files from the post-commit re-hash and report them in their own section of the restore report.

### Requirement 17: Evidence View and Brief screens (Task 30)

**User Story:** As a developer, I want to browse findings by status and read the brief in the app, so that I can understand the project before I restore or resume it.

#### Acceptance Criteria

1. WHEN `evidence_get` is called with a project ID and an optional status filter, THE Command_Layer SHALL return the matching Findings, the count per Status, the diagnostics, and the `partial` flag.
2. WHEN `brief_get` is called with a project ID and an optional restore ID, THE Command_Layer SHALL return the Brief JSON and its rendered Markdown.
3. IF a response from these commands fails Zod validation in the UI, THEN THE Evidence_View or Brief_Screen SHALL show its error state and render none of the response.
4. THE Evidence_View SHALL show each Finding's status label, rule ID, statement, and Evidence references, with file-and-line references written as `path:start-end`.
5. THE Evidence_View SHALL let the user filter Findings by Status and SHALL show the count for each Status.
6. WHEN the user selects a Finding, THE Evidence_View SHALL call `evidence_get` with its finding ID and show each reference's resolution state and, for file-and-line references, the excerpt with line numbers.
7. WHEN a project has a `kiro.tasks_progress` Finding, THE Evidence_View SHALL show its statement in the Overview tab with a citation control that opens the Finding.
8. THE Brief_Screen SHALL show the four outcomes, the sections, and the evidence index, and each citation SHALL be a control that opens that Finding in the Evidence_View.
9. WHEN the user activates Export on the Brief_Screen, THE Brief_Screen SHALL call `brief_export` and show the saved display path, show nothing on `CANCELLED`, and show a "choose another name" message on `EXPORT_DEST_EXISTS`.
10. THE Evidence_View and Brief_Screen SHALL each have loading, empty, error, and partial states.
11. THE Evidence_View and Brief_Screen SHALL support keyboard access and VoiceOver labels, and SHALL show each Status with a text label so that color is never the only signal (UX-1).

### Requirement 18: Adapter consent and separate grants (AGT-1) (P1)

**User Story:** As a developer, I want agent histories read only after I agree and pick the folder myself, so that DORMBIT never touches them by surprise.

#### Acceptance Criteria

1. THE Adapter_Framework SHALL NOT read any file of an agent store unless a Consent record for that adapter exists and an Agent store grant is linked to that Consent record.
2. WHEN `consent_set` is called with `granted: true`, THE Command_Layer SHALL write a Consent record with the adapter ID, the time from the Clock, and the consent text version, using an atomic write to DORMBIT's app state.
3. WHEN the user picks an agent store folder for an adapter that has a Consent record, THE Command_Layer SHALL open the native folder picker from Rust and record a scope grant of a kind distinct from discovery Read grants, linked to that adapter.
4. IF an agent store folder pick is requested for an adapter with no Consent record, THEN THE Command_Layer SHALL return `SCOPE_DENIED` and open no picker.
5. THE discovery walker and the Evidence_Engine SHALL NOT walk or read an Agent store grant.
6. WHEN `consent_set` is called with `granted: false`, THE Command_Layer SHALL delete the Consent record, revoke the linked Agent store grant, and delete the cached metadata for that adapter.
7. THE Adapter_Framework SHALL treat each adapter's consent separately, so that consent for one adapter enables no other adapter.
8. THE Settings screen SHALL show the consent text before consent is recorded, naming the metadata fields read, stating that message content and credential files are never read, and stating that sessions are not resumed by DORMBIT.

### Requirement 19: Read-only, metadata only, credential files never opened (AGT-1, §7 Agent-store privacy) (P1)

**User Story:** As a developer, I want DORMBIT to take only session metadata from agent stores, so that my transcripts and credentials stay private.

#### Acceptance Criteria

1. THE Adapter_Framework SHALL access an agent store only through a read-only Vfs that has no write operation.
2. THE Adapter_Framework SHALL extract only these fields: session ID, working directory, first and last timestamps, Git branch, and tool version.
3. THE Adapter_Framework SHALL discard every other field while parsing, and SHALL NOT store message content in any value, cache, log, IPC payload, Finding, Brief, or capsule `agent_refs[]` entry.
4. THE Adapter_Framework SHALL NOT open any path on the Credential deny-list.
5. THE Adapter_Framework SHALL open only files that match a Supported format of the adapter, and SHALL count other files without opening them.
6. THE Adapter_Framework SHALL NOT follow symlinks inside an agent store.
7. IF a line in a session file is longer than `AdapterLimits::max_line_bytes`, or a file or file count exceeds its `AdapterLimits` bound, THEN THE Adapter_Framework SHALL skip it, count it, and mark the adapter result partial.
8. THE Adapter_Framework SHALL cite agent store sources only with file stat references.
9. THE Adapter_Framework SHALL display working directories with the home folder written as `~`.
10. THE adapter tests SHALL use only synthetic agent stores, built in memory or in temporary folders, and SHALL NOT reference any path under the real `~/.claude` or `~/.codex`.

### Requirement 20: Supported and Unsupported formats (AGT-1) (P1)

**User Story:** As a developer, I want to know which agent formats DORMBIT can read, so that I don't assume missing sessions mean there were none.

#### Acceptance Criteria

1. WHEN `adapters_list` is called, THE Command_Layer SHALL return for each adapter its ID, consent state, linked grant ID if any, and each format with Supported or Unsupported and a reason.
2. THE Command_Layer SHALL include in `adapters_list` the Claude Code, Codex, and Kiro export adapters, plus the Kiro local session store and Cursor history as Unsupported with no consent option.
3. WHEN a file in an Agent store grant matches an Unsupported format, THE Adapter_Framework SHALL emit an Unsupported `adapter.format_unsupported` Finding citing a file stat, without opening the file.
4. IF a file of a Supported format fails to parse, THEN THE Adapter_Framework SHALL emit an Unsupported `adapter.format_unsupported` Finding for that file and continue with the next file.

### Requirement 21: Session references are labeled "not resumed by DORMBIT" (AGT-2) (P1)

**User Story:** As a developer, I want every session reference clearly labeled, so that I never think DORMBIT restored my agent session.

#### Acceptance Criteria

1. THE Evidence_View, the Brief_Screen, and the Brief_Renderer SHALL show the label "not resumed by DORMBIT" on every session reference.
2. THE Brief_Generator SHALL place session references only in the Context references section.
3. THE Command_Layer SHALL expose no command that opens, replays, or resumes an agent session.

### Requirement 22: Claude Code adapter (AGT-1, Task 32) (P1)

**User Story:** As a Claude Code user, I want to see which sessions worked on this project and that they are at risk of deletion, so that I can save what I need before it is gone.

#### Acceptance Criteria

1. THE Claude_Code_Adapter SHALL read only files matching `<project>/<session>.jsonl` under the `projects` folder of its grant, where the grant root is either `~/.claude/projects` or a folder whose `projects` child it uses.
2. THE Claude_Code_Adapter SHALL take the session ID from the `<session>` file name stem.
3. THE Claude_Code_Adapter SHALL report the session JSONL format as Unsupported until the field names for working directory, timestamp, Git branch, and tool version are confirmed from official Claude Code documentation or from a Keys_Probe run by the user, with the source recorded in `docs/product/research.md`.
4. WHEN a session's working directory equals a project's root path after Unicode NFC normalization, THE Claude_Code_Adapter SHALL attach that session reference to the project.
5. WHEN a project has at least one attached Claude Code session, THE Claude_Code_Adapter SHALL emit an Inferred `adapter.claude_code.retention` Finding, shown as "context at risk", stating that Claude Code deletes local transcripts after `cleanupPeriodDays`, with a documented default of 30 days.
6. WHEN the retention Finding is emitted, THE Claude_Code_Adapter SHALL emit an Unknown `adapter.claude_code.retention_setting` Finding stating that the user's own `cleanupPeriodDays` setting is Unknown because DORMBIT does not read Claude Code settings.
7. THE Claude_Code_Adapter SHALL NOT open files under `<session>/subagents/` or any Claude Code settings file.
8. WHEN sessions match no project, THE Claude_Code_Adapter SHALL report only their count in `adapters_list`.

### Requirement 23: Codex adapter (AGT-1, Task 32) (P1)

**User Story:** As a Codex user, I want DORMBIT to read Codex metadata only where the format is documented, so that it never guesses at an undocumented layout.

#### Acceptance Criteria

1. THE Codex_Adapter SHALL use its grant root as `CODEX_HOME` (default `~/.codex`).
2. THE Codex_Adapter SHALL parse a Codex file only after its path layout and field names are confirmed from official Codex documentation, with the source link recorded in `docs/product/research.md`.
3. IF the layout or field names of a Codex file are not confirmed, THEN THE Codex_Adapter SHALL report that format as Unsupported and open no file of that format.
4. THE Codex_Adapter SHALL NOT open `auth.json` at the grant root or anywhere in the grant.
5. WHEN the `history.jsonl` format is Supported, THE Codex_Adapter SHALL extract only the metadata fields listed in Requirement 19.2.

### Requirement 24: Kiro export adapter (AGT-1, Task 33) (P1)

**User Story:** As a Kiro user, I want my exported chats linked to the project, so that the brief points me to the right session.

#### Acceptance Criteria

1. THE Kiro_Export_Adapter SHALL read only `*.zip` files at the top level of its grant.
2. THE Kiro_Export_Adapter SHALL decompress only the `session.json` entry at the root of each zip, and SHALL NOT decompress `messages.jsonl`, `sub-executions/*.jsonl`, or any other entry.
3. IF the `session.json` entry's uncompressed size exceeds `AdapterLimits::max_entry_bytes` or its compression ratio exceeds `AdapterLimits::max_ratio`, THEN THE Kiro_Export_Adapter SHALL report that zip as Unsupported without decompressing the entry.
4. THE Kiro_Export_Adapter SHALL report the `session.json` format as Unsupported until its field names are confirmed from Kiro's chat export documentation or from a Keys_Probe run by the user on their own export.
5. THE Kiro_Export_Adapter SHALL NOT use zip entry names as filesystem paths.
6. THE Kiro_Export_Adapter SHALL NOT read Kiro's local session storage.
7. WHEN a Kiro export session reference is attached to a project, THE Brief_Generator SHALL list it in Context references with the label "not resumed by DORMBIT".

### Requirement 25: Keys-only schema probe (Task 32) (P1)

**User Story:** As a developer, I want to confirm an agent file's field names without exposing its contents, so that adapters can be built without anyone reading my transcripts.

#### Acceptance Criteria

1. WHEN the user runs `dormbit-cli agents keys-probe <file>`, THE Keys_Probe SHALL print each JSON key path with its value type and occurrence count.
2. THE Keys_Probe SHALL NOT print any string, number, or boolean value from the input file.
3. THE DORMBIT app SHALL NOT run the Keys_Probe, and development agents SHALL run it only on synthetic fixtures.

## Open questions
- **Restore facts as evidence.** INT-1 lists four Evidence reference kinds, and DORMBIT's own restore report is not one of them. Resolved conservatively: `restore.*` Findings cite the capsule's `manifest.json` keys (manifest key kind) and a file stat of the restored root. No fifth kind is added.
- **Files outcome with no restore.** BRF-2 gives the wording "Files restored and verified." Stating it when nothing was restored would be false. Resolved: the label text is kept verbatim and followed by "none yet" (Requirement 13.3). Please confirm this wording.
- **Agent store grant kind.** §8 `ScopeGrant.kind` is `Read|CapsuleLibrary|RestoreTarget`. Using `Read` for agent stores would let discovery walk transcripts. Resolved conservatively: a distinct kind (proposed `AgentStore`) is required (Requirement 18.3, 18.5). §8 and the scope-and-discovery spec need this added.
- **New error codes.** §8 lists stable codes "including" a set that has none for export. This spec proposes `EXPORT_DEST_EXISTS` and `EXPORT_WRITE_FAILED`. Export refuses to overwrite even if the macOS save panel asked "Replace?", because DORMBIT never deletes or overwrites user files.
- **WIP commits.** `GitSnapshot` in §8 carries only the HEAD commit summary, so `git.wip_commit` checks HEAD only. Checking more commits needs a `GitSnapshot` change in scope-and-discovery.
