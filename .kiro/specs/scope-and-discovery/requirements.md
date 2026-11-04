# Requirements Document

## Introduction
This spec covers master plan Phase 03, the discovery engine (P03, Tasks 15-18): scope enforcement, the walker and project metadata, Git inspection, and the Shelf wired to the UI. It formalizes SCP-1, SCP-2, DSC-1 to DSC-5, and GIT-1 from §4, the `ScopeGrant`, `ProjectRecord`, and `GitSnapshot` models from §8, and the §7 threats "Escape through symlinks", "Changes to source files", "IPC abuse", and "Code running through Git config or hooks". Its plan property is PROP-9 (no access outside granted folders, including through symlinks). It builds on the `foundation` spec (P02, Tasks 9-14).

## Glossary
- **Scope_Grant**: a record that gives DORMBIT access to one folder the user picked. Fields follow §8 `ScopeGrant`: `id`, `kind`, `root_display`, `granted_at`, `via`.
- **Grant kind**: one of `Read` (a source folder to discover projects in), `CapsuleLibrary` (a folder where capsules are stored), or `RestoreTarget` (a parent folder that restores are written into).
- **Grant root**: the folder a Scope_Grant points to.
- **Grant identity**: the device number and inode of the Grant root, recorded when the grant is created.
- **Scope_Registry**: the component in `dormbit-fs` that creates, stores, reopens, and revokes Scope_Grants and hands out directory handles for them.
- **Source_Dir**: the read-only handle type for a `Read` grant. It wraps a `cap-std` directory handle and has no write methods.
- **RelPath**: a validated relative path: not empty, not absolute, no `..` component, no empty component, no NUL byte.
- **ReadVfs**: the read-only filesystem trait in `dormbit-core`. Source_Dir implements it natively; MemVfs implements it in memory.
- **MemVfs**: an in-memory ReadVfs used by tests and by the WebAssembly build.
- **Clock**: the time source injected into `dormbit-core`, so tests can control elapsed time.
- **Command_Layer**: the allowlisted Tauri commands in `apps/desktop/src-tauri` that this spec adds.
- **Tauri_Shell**: the hardened desktop app from `foundation` Task 10 (capabilities, CSP, command allowlist).
- **Folder_Picker**: the native macOS folder dialog, opened from Rust through the Rust API of `tauri-plugin-dialog`.
- **Walker**: the bounded, step-based directory walker in `dormbit-core` that runs over a ReadVfs.
- **Discovery_Job**: one run of the Walker, metadata extraction, and Git inspection over one `Read` grant.
- **Discovery_Engine**: the code that turns a finished walk and Git snapshots into ProjectRecords and a ScanSummary.
- **Metadata_Extractor**: the part of the Discovery_Engine that reads names, stacks, docs, manifests, and agent artifacts.
- **Rebuildable_Classifier**: the part of the Discovery_Engine that decides whether a directory is a Rebuildable directory.
- **Git_Inspector**: the `dormbit-git` crate. It reads repositories with `git2` (libgit2) and never runs the git CLI.
- **State_Store**: the component that reads and writes DORMBIT's own JSON state files in Application Support.
- **Shelf**: the native screen that lists discovered projects (§9 screen 4).
- **Project root**: a directory that contains a Marker and has no ancestor inside the grant that is already a Project root.
- **Marker**: `.git` (directory or file), a file listed in the Manifest_Table, a `.kiro` directory, or an `AGENTS.md` file.
- **Manifest_Table**: the fixed list of package manifest file names in design.md, with the stack each one indicates.
- **Rebuildable directory**: a directory matched by the Rebuildable_Table, such as `node_modules`, whose contents a package manager or build tool can recreate.
- **Rebuildable_Table**: the fixed list of directory names and guard conditions in design.md.
- **Display path**: a path shown to the user, built from `root_display` and `rel_root`, with the home folder written as `~`.
- **Discovery_Limits**: the bounds of a walk: `max_depth`, `max_entries`, `time_budget_ms`, and `max_read_bytes` (the most bytes read from one manifest or ignore file).
- **Partial**: a flag that says a result is incomplete because a bound was hit or something couldn't be read.
- **ScanSummary**: the stored result of one finished Discovery_Job: grant ID, finish time, `partial`, the bounds hit, and entry counts.
- **ProjectRecord**: the stored record for one project, following §8.
- **GitSnapshot**: the Git state of one project, following §8.
- **GitIssue**: a code that says why a project has no GitSnapshot (for example `Unreadable`).
- **Finding**: §8 `{id, rule, statement, status, evidence[]}` with the INT-1 statuses Confirmed, Inferred, Unknown, Unsupported.
- **Risky config key**: one of the Git config keys GIT-1 names: `core.fsmonitor`, `core.hooksPath`, any `filter.*` key, `core.sshCommand`, `credential.helper` (including `credential.<url>.helper`).
- **Dormant mark**: the manual `marks.dormant` flag on a ProjectRecord.
- **Unborn branch**: the state of a repository with no commits, where HEAD names a branch that doesn't exist yet.
- **Linked worktree**: a working tree whose `.git` is a file pointing to a Git directory elsewhere.

## Requirements

### Requirement 1: Folder grants come only from pickers opened by Rust (SCP-1)

**User Story:** As a developer, I want DORMBIT to see only folders I picked myself in a system dialog, so that the app can't reach anything I didn't choose.

#### Acceptance Criteria

1. WHEN the UI invokes `scope_pick` with kind `Read` or `CapsuleLibrary`, THE Scope_Registry SHALL open the Folder_Picker from Rust.
2. THE Tauri_Shell SHALL grant the WebView no `dialog`, `fs`, or `shell` plugin permission.
3. WHEN the user confirms a folder in the Folder_Picker, THE Scope_Registry SHALL open it as a `cap-std` directory handle and return a new Scope_Grant with a fresh `id`, the requested `kind`, `root_display` with the home folder as `~`, `granted_at`, and `via: NativePicker`.
4. WHEN the user dismisses the Folder_Picker, THE Scope_Registry SHALL return no grant and leave the registry unchanged.
5. IF the picked folder can't be opened as a directory, THEN THE Scope_Registry SHALL return `SCOPE_DENIED` and create no grant.
6. WHEN the user picks a folder whose Grant identity matches an existing grant of the same kind, THE Scope_Registry SHALL return the existing grant and SHALL NOT create a second one.
7. IF `scope_pick` is invoked with kind `RestoreTarget`, THEN THE Command_Layer SHALL return `SCOPE_DENIED` without opening a picker.
8. THE release build SHALL contain only the native Folder_Picker, and the test picker stub SHALL compile only when the `test-picker` feature is enabled.

### Requirement 2: The UI refers to folders by grant ID, never by path (SCP-1)

**User Story:** As a developer, I want the UI to work only with IDs, so that a bug or injected content in the WebView can't point DORMBIT at an arbitrary path.

#### Acceptance Criteria

1. THE Command_Layer SHALL accept no filesystem path argument in `scope_pick`, `scope_list`, `scope_revoke`, `discovery_start`, `discovery_cancel`, `projects_list`, `project_get`, or `project_set_dormant`.
2. THE Command_Layer SHALL return folder locations only as `root_display` and `rel_root`, with the home folder written as `~`.
3. IF a command receives a grant ID that is malformed, unknown, or revoked, THEN THE Command_Layer SHALL return `SCOPE_DENIED`.
4. IF a command receives a project ID that is malformed or unknown, THEN THE Command_Layer SHALL return `SCOPE_DENIED`.
5. THE Command_Layer SHALL write no absolute path into any `DormbitError` message or log line.
6. WHEN the app launches, THE Scope_Registry SHALL reopen each stored grant and compare its device number and inode with the recorded Grant identity.
7. IF a stored grant can't be reopened or its Grant identity differs, THEN THE Scope_Registry SHALL report it in `scope_list` with `available: false` and return `SCOPE_DENIED` for every operation on it.

### Requirement 3: Grant kinds (SCP-1)

**User Story:** As a developer, I want each folder grant to have one fixed purpose, so that a folder I granted for reading is never used as a write target.

#### Acceptance Criteria

1. THE Scope_Registry SHALL give every Scope_Grant exactly one kind: `Read`, `CapsuleLibrary`, or `RestoreTarget`.
2. THE Scope_Registry SHALL hand out a Source_Dir only for `Read` grants.
3. IF `discovery_start` receives the ID of a `CapsuleLibrary` or `RestoreTarget` grant, THEN THE Command_Layer SHALL return `SCOPE_DENIED` and start no job.
4. THE Scope_Registry SHALL expose no write operation for any grant kind in this spec.

### Requirement 4: Revoking a grant clears its cached data (SCP-1, journey 9)

**User Story:** As a developer, I want removing a folder grant to remove what DORMBIT stored about it, so that revoking actually ends DORMBIT's knowledge of that folder.

#### Acceptance Criteria

1. WHEN `scope_revoke` receives the ID of an existing grant, THE Scope_Registry SHALL remove the grant and close its directory handle.
2. WHEN a grant is revoked, THE State_Store SHALL remove that grant's ProjectRecords, ScanSummary, and Dormant marks from `scopes.json`, `projects.json`, and `marks.json` before `scope_revoke` returns.
3. WHEN a grant is revoked while a Discovery_Job runs on it, THE Discovery_Job SHALL stop, emit `Cancelled`, and persist nothing.
4. WHEN a grant has been revoked, THE Command_Layer SHALL omit its projects from `projects_list` and return `SCOPE_DENIED` for its grant ID and its project IDs.
5. THE Scope_Registry SHALL NOT create, modify, or delete any file inside the Grant root when revoking.
6. IF `scope_revoke` receives an unknown grant ID, THEN THE Command_Layer SHALL return `SCOPE_DENIED` and leave all state files unchanged.

### Requirement 5: All access goes through `cap-std` handles (SCP-2, PROP-9)

**User Story:** As a developer, I want every file read to be confined to the folder I granted, so that a symlink or a crafted path can't make DORMBIT read elsewhere.

#### Acceptance Criteria

1. THE Source_Dir SHALL perform every read of a Grant root through the `cap-std` directory handle opened for that grant.
2. THE codebase SHALL call `cap_std::ambient_authority` only inside `crates/dormbit-fs/src/scope/`, enforced by a clippy `disallowed-methods` rule.
3. IF a requested path is empty, absolute, or contains a `..` component, an empty component, or a NUL byte, THEN THE Source_Dir SHALL return `PATH_UNSAFE` without opening anything.
4. IF a requested path resolves through a symlink to a location outside the Grant root, THEN THE Source_Dir SHALL return `PATH_UNSAFE` and return no bytes.
5. THE `dormbit-core` crate SHALL contain no use of `std::fs`, enforced by clippy `disallowed-types` and `disallowed-methods` rules.

### Requirement 6: Source folders are read-only at the type level (SCP-2)

**User Story:** As a developer, I want source folder access to have no write API at all, so that DORMBIT can't change my projects even by mistake.

#### Acceptance Criteria

1. THE Source_Dir SHALL expose only three operations: list a directory, read metadata without following symlinks, and read a regular file up to a byte limit.
2. THE `dormbit-fs` crate SHALL include `compile_fail` doc tests showing that create, write, remove, rename, and set-permissions calls on a Source_Dir don't compile.
3. THE Source_Dir SHALL open every file with read-only access.
4. THE Source_Dir SHALL NOT expose its inner `cap-std` handle, a raw file descriptor, or the absolute Grant root path, except the `GitLocation` it gives to the Git_Inspector (Requirement 16).
5. WHEN discovery and Git inspection finish on a fixture tree, THE fixture tree SHALL have the same bytes, modes, and modification times as before the scan.

### Requirement 7: Discovery never follows symlinks or opens special files (SCP-2)

**User Story:** As a developer, I want discovery to treat symlinks and special files as things to note, not things to open, so that a link loop or a FIFO can't hang or redirect the scan.

#### Acceptance Criteria

1. WHEN the Walker meets a symlink, THE Walker SHALL record it as kind `Symlink`, SHALL NOT descend into it, and SHALL NOT read its target.
2. IF the final component of a path passed to the Source_Dir's file read is a symlink, THEN THE Source_Dir SHALL return `PATH_UNSAFE`.
3. WHEN the Walker meets a FIFO, socket, or device file, THE Walker SHALL record it as kind `Special` and SHALL NOT open it.
4. IF a file opened by the Source_Dir is not a regular file, THEN THE Source_Dir SHALL close it and return `PATH_UNSAFE` without reading it.
5. WHEN the tree contains symlink loops, THE Walker SHALL finish without visiting any directory twice.

### Requirement 8: Project roots and monorepos (DSC-1)

**User Story:** As a developer, I want DORMBIT to find my projects by their usual markers and treat a monorepo as one project, so that the Shelf matches how I think about my work.

#### Acceptance Criteria

1. WHEN a directory contains a Marker and no ancestor inside the grant is a Project root, THE Walker SHALL report that directory as a Project root.
2. WHILE the Walker is inside a Project root, THE Walker SHALL NOT report nested directories as separate projects, and THE Metadata_Extractor SHALL list nested manifests in the enclosing project's `manifests`.
3. WHEN the Grant root itself contains a Marker, THE Walker SHALL report exactly one project, with `rel_root` set to `.`.
4. THE Walker SHALL NOT look for Markers, docs, or manifests inside `.git`, inside Rebuildable directories, or in paths matched by the enclosing project's ignore rules.
5. THE Walker SHALL read ignore rules only from `.gitignore` files and `.git/info/exclude` inside the project, through the Source_Dir.
6. THE Discovery_Engine SHALL derive each project ID from the grant ID and the NFC-normalized `rel_root`, so that rescanning the same tree gives the same ID.
7. WHEN a scan finishes and found no Project root, THE Discovery_Engine SHALL store a ScanSummary with a project count of 0.

### Requirement 9: Bounded, cancellable scans with progress (DSC-2)

**User Story:** As a developer, I want scans of big folders to stop at clear limits, report progress, and be cancellable, so that a huge tree never hangs the app and I know when a result is incomplete.

#### Acceptance Criteria

1. THE Walker SHALL NOT visit entries more than `max_depth` levels below the Grant root.
2. THE Walker SHALL stop after visiting `max_entries` entries.
3. THE Walker SHALL stop when the elapsed time measured by the Clock exceeds `time_budget_ms`.
4. WHEN any bound stops or limits a walk, THE Discovery_Engine SHALL set the ScanSummary's `partial` to true and list each bound hit in `bounds_hit`.
5. WHEN a bound leaves part of a project's tree unvisited, THE Discovery_Engine SHALL set that project's `activity.partial` and `size.partial` to true.
6. WHEN a walk visits every reachable entry without hitting a bound, THE Discovery_Engine SHALL set `partial` to false and leave `bounds_hit` empty.
7. WHEN `discovery_cancel` receives the job ID of a running Discovery_Job, THE Discovery_Job SHALL stop before its next step, emit `Cancelled`, and end with `CANCELLED`.
8. WHEN a Discovery_Job is cancelled, THE State_Store SHALL keep that grant's previous ProjectRecords and ScanSummary unchanged.
9. WHILE a Discovery_Job runs, THE Discovery_Job SHALL emit a `Progress` event after each step with the counts of directories, files, and projects found so far and the elapsed milliseconds.
10. THE Discovery_Job SHALL never report a count in a `Progress` event that is lower than the same count in an earlier event of that job.
11. WHEN a Discovery_Job finishes, THE counts in its `Finished` event SHALL equal the counts in the stored ScanSummary.
12. IF a directory or file can't be read, THEN THE Walker SHALL add it to the `unreadable` count, THE Discovery_Engine SHALL set the enclosing project's `size.partial` to true, and the walk SHALL continue.
13. WHEN `discovery_start` receives a grant ID that already has a running Discovery_Job, THE Command_Layer SHALL return that job's ID and start no second job.
14. WHEN a Discovery_Job finishes without being cancelled, THE State_Store SHALL replace that grant's ProjectRecords and ScanSummary in one atomic write.

### Requirement 10: Per-project metadata (DSC-3)

**User Story:** As a developer, I want each project to show where its name, stack, docs, and activity came from, so that I can trust what the Shelf tells me.

#### Acceptance Criteria

1. WHEN the project root contains a manifest with a name key (`package.json` `name`, `Cargo.toml` `[package].name`, or `pyproject.toml` `[project].name`), THE Metadata_Extractor SHALL use that value as `name.value` and record the file and key as `name.source`.
2. IF no root manifest provides a name, THEN THE Metadata_Extractor SHALL use the project folder's name as `name.value` and `FolderName` as `name.source`.
3. THE Shelf SHALL show each project's location as a Display path.
4. WHEN a manifest in the Manifest_Table is found, THE Metadata_Extractor SHALL add a stack Finding with status Confirmed, a rule ID, and at least one evidence reference to the manifest key or file it read.
5. THE Metadata_Extractor SHALL set `activity.latest_mtime` to the newest modification time among regular files visited in the project, excluding `.git/` and Rebuildable directories.
6. THE Metadata_Extractor SHALL list in `docs` the files at the project root whose names start with `README`, `CHANGELOG`, `CONTRIBUTING`, `LICENSE`, or `COPYING`, and the `*.md` files under the project's `docs/` folder.
7. THE Metadata_Extractor SHALL list in `manifests` the relative path, kind, and parse result of every Manifest_Table file found outside the paths excluded by Requirement 8.4.
8. IF a manifest is larger than `max_read_bytes` or fails to parse, THEN THE Metadata_Extractor SHALL record it with `parsed: false`, take no name or stack evidence from it, and continue.
9. THE Metadata_Extractor SHALL record in `agent_artifacts` the presence and relative path of `.kiro/`, `AGENTS.md`, `CLAUDE.md`, `.cursor/rules`, and `.specstory/`, without reading their contents.
10. WHILE no capsule exists for a project, THE Shelf SHALL show the project's preservation state as "No capsule yet".

### Requirement 11: Size split into rebuildable and other (DSC-3)

**User Story:** As a developer, I want to see how much of a project is rebuildable, so that I know what really needs preserving.

#### Acceptance Criteria

1. THE Rebuildable_Classifier SHALL mark a directory as a Rebuildable directory only when its name and guard condition match a row of the Rebuildable_Table.
2. IF a directory's name matches the Rebuildable_Table but its guard condition fails, THEN THE Rebuildable_Classifier SHALL treat it as an ordinary directory.
3. THE Discovery_Engine SHALL set `size.total` to the sum of the sizes of all regular files visited in the project, and `size.rebuildable` to the sum of the sizes of those inside Rebuildable directories.
4. WHILE walking a project, THE Walker SHALL visit `.git` and Rebuildable directories only after every other entry of that project.
5. THE Discovery_Engine SHALL count symlinks as 0 bytes and SHALL NOT measure their targets.

### Requirement 12: Dormancy is a manual mark (DSC-4)

**User Story:** As a developer, I want to decide myself which projects are dormant, so that DORMBIT never labels my work abandoned because of old timestamps.

#### Acceptance Criteria

1. THE Discovery_Engine SHALL NOT set `marks.dormant` or any abandonment label from timestamps, Git data, or any other scanned data.
2. WHEN a new project is first discovered, THE Discovery_Engine SHALL set its `marks.dormant` to false.
3. WHEN `project_set_dormant` receives a project ID and a value, THE State_Store SHALL write the value to `marks.json`, and THE Command_Layer SHALL return the updated ProjectRecord.
4. WHEN a project is rescanned, THE Discovery_Engine SHALL keep its `marks.dormant` value.
5. THE Shelf SHALL show the dormant label only for projects whose `marks.dormant` is true, and SHALL show latest activity as a date without the words "abandoned", "stale", or "inactive".

### Requirement 13: Shelf search, sort, filter, and multi-select (DSC-5)

**User Story:** As a developer with many projects, I want to search, sort, filter, and select several projects at once, so that I can find and mark the ones I care about quickly.

#### Acceptance Criteria

1. WHEN the user types in the Shelf search field, THE Shelf SHALL show only projects whose name or Display path contains the query, compared case-insensitively after NFC normalization.
2. WHEN the user picks a sort key, THE Shelf SHALL order projects by name, latest activity, or total size, ascending or descending.
3. WHEN the user applies filters, THE Shelf SHALL show only projects matching every active filter among: grant, stack, Dormant mark, uncommitted changes (any GitSnapshot status count above 0), and agent artifacts present.
4. THE Shelf SHALL support selecting rows by click, Shift-click for a range, Cmd-click to toggle, Cmd-A for all visible rows, and Space on the focused row, and SHALL show the number of selected rows.
5. WHEN the user marks or unmarks the selected projects as dormant, THE Shelf SHALL call `project_set_dormant` once per selected project and show which projects failed, if any.
6. THE Shelf SHALL show in each row the name, a middle-truncated Display path, stack chips, branch and dirty marks, latest activity, total and rebuildable size, and preservation state.
7. THE Shelf SHALL render all project-sourced text (names, paths, branch names, commit summaries) as plain text, with no raw HTML.
8. THE Shelf SHALL have loading, empty (no grants, or no projects found), scanning (with live progress counts), partial (naming the bounds hit), cancelled, and error states.
9. WHEN the Shelf shows upstream ahead and behind counts, THE Shelf SHALL label them "as of last fetch".
10. THE Shelf SHALL be fully usable by keyboard, SHALL give every control a VoiceOver label, SHALL pair every status color with a glyph and a text label, and SHALL use no motion when reduced motion is on.

### Requirement 14: Git is read with libgit2 only (GIT-1)

**User Story:** As a developer, I want DORMBIT to read my repositories without running Git or any configured command, so that scanning can't execute code from a repository.

#### Acceptance Criteria

1. THE Git_Inspector SHALL depend on `git2` with default features off and vendored libgit2, so that no HTTPS or SSH transport is compiled in.
2. THE crates `dormbit-core`, `dormbit-fs`, `dormbit-git`, and `apps/desktop/src-tauri` SHALL contain no use of `std::process::Command`, enforced by a clippy `disallowed-types` rule.
3. THE Git_Inspector SHALL open a repository only at the project root, with parent-directory search turned off.
4. THE Git_Inspector SHALL call no libgit2 API that writes to the repository, and SHALL NOT request index updates in status options.
5. WHEN the Git_Inspector inspects a repository whose config sets `core.fsmonitor`, `core.hooksPath`, a `filter.*` driver, `core.sshCommand`, or `credential.helper` to a command, THE configured command SHALL NOT run.
6. THE Git_Inspector SHALL return only GitIssue codes to callers, never libgit2 error messages, because those can contain absolute paths.

### Requirement 15: GitSnapshot contents (GIT-1, DSC-3)

**User Story:** As a developer, I want to see my branch, HEAD, uncommitted work, stashes, and upstream state, so that I know where I left off in Git.

#### Acceptance Criteria

1. THE Git_Inspector SHALL record in `head` the short branch name, whether HEAD is detached, the full commit hash, the commit time, and the first line of the commit message.
2. THE Git_Inspector SHALL record in `status` the counts of modified, staged, untracked, deleted, and conflicted files, using the flag mapping in design.md.
3. THE Git_Inspector SHALL record in `stash_count` the number of stash entries.
4. WHEN the current branch has a configured upstream whose remote-tracking ref exists locally, THE Git_Inspector SHALL record the upstream name and the ahead and behind counts computed from local refs only.
5. THE Git_Inspector SHALL set `upstream.as_of_last_fetch` to the modification time of `.git/FETCH_HEAD` when that file exists, and leave it empty otherwise.
6. IF the current branch has a configured upstream whose remote-tracking ref doesn't exist locally, THEN THE Git_Inspector SHALL leave `upstream` empty.
7. THE Git_Inspector SHALL list each submodule's name, path, and whether it is initialized, and SHALL NOT open submodule repositories or count their changes in `status`.
8. THE Git_Inspector SHALL set `lfs_detected` to true when the root `.gitattributes` contains `filter=lfs` or `.git/lfs` exists, and SHALL NOT run Git LFS.
9. THE Git_Inspector SHALL set `worktree_external` to true when the project's `.git` is a file that points to a Git directory.

### Requirement 16: Git edge cases (GIT-1)

**User Story:** As a developer, I want unusual or damaged repositories to be reported plainly, so that one odd repository doesn't break the scan or produce wrong data.

#### Acceptance Criteria

1. WHEN a repository has an Unborn branch, THE Git_Inspector SHALL record the branch name from HEAD, `detached: false`, an empty commit, time, and summary, the status counts, and a `stash_count` of 0.
2. WHEN HEAD is detached, THE Git_Inspector SHALL record an empty branch, `detached: true`, and the commit HEAD points to.
3. WHEN a project is a Linked worktree whose Git directory and common directory are inside the same Grant root, THE Git_Inspector SHALL record the worktree's own HEAD and status.
4. IF a project's `.git` is a symlink, THEN THE Git_Inspector SHALL NOT open the repository, and THE Discovery_Engine SHALL set `git_issue` to `GitDirIsSymlink`.
5. IF a project's `.git` file points to a Git directory or common directory outside the Grant root, THEN THE Git_Inspector SHALL NOT open the repository, and THE Discovery_Engine SHALL set `git_issue` to `GitDirOutsideGrant`.
6. IF a repository is bare, THEN THE Git_Inspector SHALL NOT compute status, and THE Discovery_Engine SHALL set `git_issue` to `Bare`.
7. IF libgit2 can't open a repository or resolve HEAD for a reason other than an Unborn branch, THEN THE Discovery_Engine SHALL set `git` to empty and `git_issue` to `Unreadable`, and keep every other field of the ProjectRecord.
8. WHEN Git inspection fails for one project, THE Discovery_Job SHALL continue with the remaining projects.

### Requirement 17: Risky Git config is flagged (GIT-1)

**User Story:** As a developer, I want to know when a repository's config can run commands, so that I'm warned before I or a tool open it with the git CLI later.

#### Acceptance Criteria

1. WHEN the repository-level config sets `core.fsmonitor`, `core.hooksPath`, any `filter.*` key, `core.sshCommand`, `credential.helper`, or `credential.<url>.helper`, THE Git_Inspector SHALL add an entry to `risky_config` with the exact key name.
2. THE Git_Inspector SHALL match section and variable names case-insensitively and subsection names case-sensitively, as Git does.
3. THE Git_Inspector SHALL NOT store, show, or log the values of Risky config keys.
4. WHEN no Risky config key is set, THE Git_Inspector SHALL leave `risky_config` empty.

### Requirement 18: Credentials in remote URLs are hidden (GIT-1)

**User Story:** As a developer, I want tokens embedded in remote URLs to never appear in DORMBIT, so that scanning doesn't expose my credentials on screen or in files.

#### Acceptance Criteria

1. WHEN a remote URL has the form `scheme://userinfo@host...`, THE Git_Inspector SHALL replace the userinfo with `***`.
2. THE Git_Inspector SHALL remove the query string and fragment from every remote URL.
3. WHEN a remote URL has the scp-like form `user@host:path`, THE Git_Inspector SHALL keep the user name, unless it contains `:`, in which case it SHALL replace it with `***`.
4. IF a remote URL matches neither form, THEN THE Git_Inspector SHALL show the text `(hidden: unrecognized URL format)` in place of the URL.
5. THE Git_Inspector SHALL NOT persist, log, or send over IPC any remote URL before redaction.

### Requirement 19: Local state persistence and single instance (§6 local state)

**User Story:** As a developer, I want DORMBIT's own records to survive crashes and never be corrupted by two copies of the app, so that my grants and marks are always intact.

#### Acceptance Criteria

1. THE State_Store SHALL write each state file by writing a temp file in the same folder, calling `sync_all`, and renaming it over the old file.
2. IF a state write fails before the rename, THEN THE State_Store SHALL leave the previous file unchanged.
3. THE State_Store SHALL include a `schema_version` field in every state file.
4. WHEN the app launches, THE Tauri_Shell SHALL take an exclusive lock on `dormbit.lock` in the state folder.
5. IF the lock is already held, THEN THE Tauri_Shell SHALL show "DORMBIT is already running" and exit without reading or writing any state file.
6. IF a state file fails to parse at launch, THEN THE State_Store SHALL rename it to `<name>.corrupt-<timestamp>`, start that file empty, and show a notice, without deleting anything.
7. THE State_Store SHALL store absolute Grant root paths only in `scopes.json`, and SHALL store only IDs and Display paths in `projects.json` and `marks.json`.
8. THE State_Store SHALL keep its files in the app data folder from Tauri's path resolver, `~/Library/Application Support/io.github.<owner>.dormbit/`.
