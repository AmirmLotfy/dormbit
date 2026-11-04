# Requirements Document

## Introduction
This spec covers safe restoration of a verified capsule into a new folder: pre-checks, the transactional executor, the journal and crash recovery, the restore report, and the Restore Wizard and Restoration Result screens. It implements master plan Phase 05 (P05, Tasks 23-26) and formalizes RST-1..RST-7, the restore threats in §7 (path traversal, symlink escape, overwrite and races, partial restore after a crash, changes to source files, code running through Git config or hooks), the `RestoreJournal` and `RestoreReport` models in §8, and the restore error codes. It also defines two P1 items: safe restore of internal relative symlinks (§3, P1 item 3) and the post-verification brief write (BRF-3). The correctness properties are PROP-1, PROP-2 (full round trip), PROP-4, PROP-5, PROP-6, and PROP-8.

## Glossary
- **Capsule**: a `.dormbit` file (capsule format v1, §8) holding a manifest and content-addressed objects.
- **Manifest**: `manifest.json` inside a capsule. Lists file entries `{path, size, mode, mtime, sha256, object{encoding, stored_size, stored_sha256}}`, symlink entries `{path, target, restored: false}`, `excluded[]`, and the `git` section.
- **Manifest entry path**: the relative path of a file or symlink entry, as stored in the manifest. It is the only source of output paths. Tar entry names are never used as output paths.
- **VerificationReport**: the verifier's result for a capsule (§8), with `status` (`Verified`, `Failed`, `Unsupported`), `manifest_sha256`, and `registry_match` (`Match`, `Mismatch`, `NotInRegistry`).
- **Scope grant**: a `ScopeGrant` record (§8) created only by a native folder picker opened by Rust. Kinds: `Read`, `CapsuleLibrary`, `RestoreTarget`.
- **Capsule library**: a folder granted with kind `CapsuleLibrary`, where capsules are stored.
- **Source project**: the project folder the capsule was captured from, found through the registry's `ProjectRecord` whose ID matches the manifest's project ID.
- **Destination parent**: the folder granted with kind `RestoreTarget`. The restore creates one new folder directly inside it.
- **Target name**: the new folder name the user types.
- **Target**: the folder `<destination parent>/<target name>` that the commit creates.
- **NFC key**: a name or path converted to Unicode Normalization Form C, compared component by component.
- **Fold key**: a case-folded key: NFD, then `to_uppercase`, then `to_lowercase`, then NFC, per component. It treats more names as equal than APFS does in some cases, which only causes extra refusals.
- **Variant**: a name whose NFC key or fold key equals another name's key (case variants, NFC/NFD variants, or both).
- **Case sensitivity**: whether the destination volume treats names that differ only by case as different. Values: case-sensitive, case-insensitive, unknown.
- **Staging folder**: `.dormbit-staging-<op_id>`, created fresh by DORMBIT directly in the destination parent. It holds the owner token file and the staged tree.
- **Owner token**: a 128-bit random value stored in the journal and in `.dormbit-owner` inside the staging folder. It proves DORMBIT created that staging folder.
- **Staged tree**: the folder `tree` inside the staging folder. All restored content is written under it. The commit renames it to the target.
- **Tree ID**: the device and inode numbers of the staged tree, recorded in the journal. A rename keeps them, so they identify the target after commit.
- **Commit**: one `renameatx_np(RENAME_EXCL)` call that moves the staged tree to the target and fails if any entry with the target name exists.
- **Destination**: the staging folder plus the target. Writes anywhere else count as writes outside the destination.
- **Journal**: the `RestoreJournal` record for one restore (§8) with `op_id`, `staging`, `owner_token`, and `state` (`Staged`, `Written`, `Verified`, `Committed`, `Cleaned`), kept in Application Support.
- **Write log**: `writes.jsonl` next to the journal. One record per directory created, file created, rename, or staging folder deletion.
- **Roll back**: end a restore that failed or was cancelled before commit. The target is never created, and the staging folder is deleted after the owner token check.
- **Restore plan**: the pure output of the Restore_Planner: directories and files to create, final modes, skipped entries with reasons, and required bytes.
- **Pre-check report**: the list of pre-check results returned by `restore_precheck`, each with an ID, a status, an optional error code, and detail.
- **Restore report**: the `RestoreReport` record (§8, RST-7) written when a restore ends.
- **Restore_Planner**: the pure planning and pre-check logic in `dormbit-core`, working over `Vfs` and data inputs.
- **Volume_Probe**: the read-only code in `dormbit-fs` that gathers volume facts (filesystem type, free space, block size, case sensitivity), canonical paths, and the destination parent's entry names.
- **Restore_Executor**: the transactional executor in `dormbit-fs`: staging, writing, staged verification, commit, rollback, and cleanup.
- **Journal_Store**: the code in `dormbit-fs` that writes journals, write logs, and reports in Application Support.
- **Recovery_Manager**: the code in `dormbit-fs` that finds unfinished restores at launch and recovers them.
- **Post_Verifier**: the code in `dormbit-core` that re-hashes the committed target through a new read-only handle.
- **Ref_Reader**: the small reader in `dormbit-core` for `.git/HEAD`, `refs/`, and `packed-refs`. It runs in both builds.
- **Git_Checker**: the post-commit Git check: Ref_Reader comparison in both builds, plus working-tree status through `dormbit-git` natively.
- **Report_Builder**: the code in `dormbit-core` that builds the restore report from the journal, the write log, and check results.
- **Restore_Commands**: the Tauri commands `restore_target_pick`, `restore_precheck`, `restore_start`, and `restore_recover` in `apps/desktop/src-tauri`.
- **Restore_Wizard**: screen 9, where the user picks a capsule, a destination, and a name, reviews pre-checks, and runs the restore.
- **Result_Screen**: screen 10 (Restoration Result), which shows the restore report.
- **Recovery_Prompt**: the dialog shown at launch when unfinished restores exist.
- **Single-instance lock**: the app-wide lock in Application Support that keeps two DORMBIT instances from running at once (§6).

## Requirements

### Requirement 1: Choose capsule, destination parent, and name (RST-1)

**User Story:** As a developer, I want to pick a verified capsule, a destination parent folder, and a new folder name, so that the restore lands exactly where I chose and nowhere else.

#### Acceptance Criteria

1. WHEN the user chooses "Choose destination folder…" in the Restore_Wizard, THE Restore_Commands SHALL open a native folder picker from Rust through `restore_target_pick` and return a `ScopeGrant` of kind `RestoreTarget` with its `id` and a `root_display` that writes the home folder as `~`.
2. IF the user closes the folder picker without choosing a folder, THEN THE Restore_Commands SHALL return `CANCELLED` and create no grant.
3. THE Restore_Commands SHALL accept the capsule as a capsule ID, the destination parent as a `RestoreTarget` grant ID, and the target name as a string, with no filesystem path field in any request type.
4. IF a request's grant ID is unknown, revoked, or not of kind `RestoreTarget`, THEN THE Restore_Commands SHALL refuse with `SCOPE_DENIED`.
5. IF a request's capsule ID is neither in the capsule registry nor opened through `capsule_pick` in the current session, THEN THE Restore_Commands SHALL refuse with `SCOPE_DENIED`.
6. IF the target name is empty, equals `.` or `..`, contains `/`, NUL, or a control character, is longer than 255 bytes in UTF-8, or starts with `.dormbit-staging-`, THEN THE Restore_Planner SHALL refuse with `PATH_UNSAFE`.
7. THE Restore_Executor SHALL create the target with the target name byte for byte as the user entered it.

### Requirement 2: Refuse an existing target and its variants (RST-1, §7 overwrite and race)

**User Story:** As a developer, I want restore to refuse any name that already exists in any case or normalization form, so that DORMBIT can never replace or merge into an existing project.

#### Acceptance Criteria

1. WHEN pre-checks run, THE Restore_Planner SHALL compare the target name with the name of every entry in the destination parent by NFC key and by fold key, whatever the volume's case sensitivity.
2. IF an entry of any type (file, directory, symlink, or dangling symlink) in the destination parent has the same NFC key or fold key as the target name, THEN THE Restore_Planner SHALL refuse with `RESTORE_DEST_EXISTS` and name that entry.
3. WHEN pre-checks refuse a restore with `RESTORE_DEST_EXISTS`, THE Restore_Commands SHALL create no journal and no staging folder.
4. WHEN staged verification has passed, THE Restore_Executor SHALL repeat the check in criterion 1 immediately before the commit rename.
5. IF the repeated check finds a matching entry, or the commit rename fails with `EEXIST`, THEN THE Restore_Executor SHALL fail with `RESTORE_DEST_EXISTS` and roll back as in Requirement 9.10.
6. WHEN a restore fails with `RESTORE_DEST_EXISTS`, THE Restore_Executor SHALL leave the existing entry unchanged in name, type, size, mode, content, and modification time.

### Requirement 3: Refuse targets inside the source project or a capsule library (RST-1)

**User Story:** As a developer, I want restore to refuse destinations inside the original project or my capsule storage, so that a restore can't tangle with the files it came from.

#### Acceptance Criteria

1. WHEN pre-checks run, THE Volume_Probe SHALL resolve the destination parent, the source project root, every `CapsuleLibrary` grant root, and the folder that contains the selected capsule file to canonical paths with symlinks resolved.
2. IF the destination parent equals the source project root or is inside it, THEN THE Restore_Planner SHALL refuse with `RESTORE_TARGET_FORBIDDEN`.
3. IF the destination parent equals or is inside a `CapsuleLibrary` grant root or the folder that contains the selected capsule file, THEN THE Restore_Planner SHALL refuse with `RESTORE_TARGET_FORBIDDEN`.
4. THE Restore_Planner SHALL decide "equals or is inside" by comparing canonical path components by fold key, so a case or normalization variant of a forbidden folder is also refused.
5. IF no project record on this Mac matches the manifest's project ID, THEN THE Restore_Planner SHALL report the source-location check as "Not applicable: source project not found on this Mac" and SHALL still apply criterion 3.

### Requirement 4: Capsule and volume pre-checks (RST-2)

**User Story:** As a developer, I want every condition checked before anything is written, so that a restore that can't succeed never starts.

#### Acceptance Criteria

1. IF the latest `VerificationReport` for the capsule is missing or its status is not `Verified`, THEN THE Restore_Planner SHALL refuse with `RESTORE_NOT_VERIFIED`.
2. IF the SHA-256 of the manifest read at pre-check time differs from the report's `manifest_sha256`, THEN THE Restore_Planner SHALL refuse with `CAPSULE_HASH_MISMATCH`.
3. IF the report's `registry_match` is `Mismatch`, THEN THE Restore_Planner SHALL refuse with `CAPSULE_HASH_MISMATCH`.
4. IF the report's `registry_match` is `NotInRegistry`, THEN THE Restore_Planner SHALL add the warning "Not in this Mac's capsule registry" to the pre-check report without blocking the restore.
5. THE Restore_Planner SHALL compute required bytes as the sum over planned files of each file's size rounded up to the volume block size, plus one block for each planned file and directory.
6. IF the free bytes available to the user (`f_bavail × f_bsize`) are fewer than the required bytes, THEN THE Restore_Planner SHALL refuse with `RESTORE_INSUFFICIENT_SPACE` and show both numbers.
7. IF the destination volume's filesystem type is not `apfs` or `hfs`, THEN THE Restore_Planner SHALL refuse with `RESTORE_VOLUME_UNSUPPORTED` and name the filesystem type.
8. WHEN pre-checks run, THE Volume_Probe SHALL classify the destination volume as case-sensitive, case-insensitive, or unknown by looking up the case-swapped name of an existing entry in the destination parent, without writing anything.
9. WHILE the case sensitivity is unknown, THE Restore_Planner SHALL apply the rules for a case-insensitive volume.
10. THE Restore_Planner and the Volume_Probe SHALL create, modify, or delete no entry on disk while running pre-checks.
11. WHEN `restore_start` is called, THE Restore_Commands SHALL run every pre-check again and start the restore only if every blocking check passes.

### Requirement 5: Manifest path rules (RST-2, §7 path traversal)

**User Story:** As a developer, I want every path in a capsule checked against strict rules, so that a crafted capsule can't write outside the folder I chose.

#### Acceptance Criteria

1. IF a manifest entry path starts with `/`, THEN THE Restore_Planner SHALL refuse with `PATH_UNSAFE` and name the entry.
2. IF a manifest entry path has a `..` component, a `.` component, or an empty component, THEN THE Restore_Planner SHALL refuse with `PATH_UNSAFE` and name the entry.
3. IF a manifest entry path contains NUL or any control character (Unicode category Cc: U+0000-U+001F and U+007F-U+009F), THEN THE Restore_Planner SHALL refuse with `PATH_UNSAFE` and name the entry.
4. IF any component of a manifest entry path is longer than 255 bytes in UTF-8, THEN THE Restore_Planner SHALL refuse with `PATH_UNSAFE` and name the entry.
5. IF a manifest entry path is longer than 1024 bytes in UTF-8, THEN THE Restore_Planner SHALL refuse with `PATH_UNSAFE` and name the entry.
6. THE Restore_Planner SHALL derive every output path from manifest entry paths only and SHALL never use a tar entry name as an output path.
7. WHEN any manifest entry fails a path rule, THE Restore_Planner SHALL refuse the whole restore instead of skipping that entry.

### Requirement 6: Collision detection (RST-2, §7 overwrite)

**User Story:** As a developer, I want paths that would land on the same file to be caught before writing, so that one capsule file can never silently replace another.

#### Acceptance Criteria

1. IF two manifest entries have the same NFC key, THEN THE Restore_Planner SHALL refuse with `RESTORE_COLLISION` and name both entries.
2. WHILE the destination volume is case-insensitive, IF two manifest entries have the same fold key, THEN THE Restore_Planner SHALL refuse with `RESTORE_COLLISION` and name both entries.
3. WHILE the destination volume is case-sensitive, THE Restore_Planner SHALL accept manifest entries whose paths differ only by letter case.
4. IF one entry's path equals a parent folder of another entry's path under the key in use (for example a file `a` and a file `a/b`), THEN THE Restore_Planner SHALL refuse with `RESTORE_COLLISION` and name both entries.
5. IF creating a directory or file in the staged tree fails because the name already exists, THEN THE Restore_Executor SHALL fail with `RESTORE_COLLISION` and roll back as in Requirement 9.10.

### Requirement 7: Staging folder and owner token (RST-3)

**User Story:** As a developer, I want restore to work in a fresh folder that DORMBIT can prove it owns, so that nothing of mine is touched while files are written.

#### Acceptance Criteria

1. WHEN `restore_start` passes pre-checks, THE Restore_Executor SHALL write a journal with state `Staged`, the staging folder name, and the owner token before it creates anything in the destination parent.
2. THE Restore_Executor SHALL create the staging folder `.dormbit-staging-<op_id>` directly in the destination parent with an exclusive create.
3. IF an entry with the staging folder's name already exists, THEN THE Restore_Executor SHALL fail with `RESTORE_DEST_EXISTS` marked retryable and leave that entry unchanged.
4. THE Restore_Executor SHALL write `.dormbit-owner` inside the staging folder with create-new semantics, containing the `op_id` and a 128-bit random owner token equal to the journal's, and sync it.
5. THE Restore_Executor SHALL create the staged tree `tree` inside the staging folder, write all restored content under it, and record its tree ID in the journal.
6. WHEN the owner token file has been written, THE Restore_Executor SHALL confirm the volume's case sensitivity by looking up `.DORMBIT-OWNER` in the staging folder without following symlinks.
7. IF the confirmed case sensitivity is case-insensitive while pre-checks used case-sensitive rules, THEN THE Restore_Executor SHALL re-run the collision check with fold keys before writing any capsule file and fail with `RESTORE_COLLISION` if it finds a collision.

### Requirement 8: Writing restored files (RST-3, §7 archive bombs and symlink escape)

**User Story:** As a developer, I want every restored file written fresh, checked byte by byte, and synced, so that what lands on disk is exactly what the capsule recorded.

#### Acceptance Criteria

1. THE Restore_Executor SHALL create each directory in the staged tree with an exclusive create that fails if the name exists.
2. THE Restore_Executor SHALL create each file through `cap-std` with create-new semantics.
3. THE Restore_Executor SHALL set each file's permission bits to the manifest mode AND `0o777`, which removes the setuid, setgid, and sticky bits and keeps the read, write, and execute bits.
4. WHILE copying an object, THE Restore_Executor SHALL hash the stored bytes and compare the hash and byte count with the entry's `stored_sha256` and `stored_size`.
5. WHILE copying an object, THE Restore_Executor SHALL hash the decoded bytes as it writes them and compare the hash and byte count with the entry's `sha256` and `size`.
6. IF the stored bytes exceed `stored_size` or the decoded bytes exceed `size`, THEN THE Restore_Executor SHALL stop reading that object, fail with `CAPSULE_MALFORMED`, and roll back.
7. IF a stored hash or a decoded hash differs from the manifest, THEN THE Restore_Executor SHALL fail with `CAPSULE_HASH_MISMATCH`, name the file, and roll back.
8. THE Restore_Executor SHALL sync each file with `sync_all` (`F_FULLFSYNC` on macOS) after its last byte and before the journal records `Written`.
9. THE Restore_Executor SHALL sync each staged directory after its entries are created and before the journal records `Written`.
10. IF a write fails with `ENOSPC`, THEN THE Restore_Executor SHALL fail with `RESTORE_INSUFFICIENT_SPACE` and roll back.
11. IF a write fails with any other I/O error, THEN THE Restore_Executor SHALL fail with `RESTORE_IO_FAILED` and roll back.
12. THE Restore_Executor SHALL open every staged path relative to its parent directory handle without following symlinks.

### Requirement 9: Staged verification, atomic commit, and rollback (RST-3, §7 partial restore)

**User Story:** As a developer, I want the restored folder to appear only after everything in it is proven correct, so that I never get a half-restored project.

#### Acceptance Criteria

1. WHEN every planned directory and file is written and synced, THE Restore_Executor SHALL record `Written` in the journal.
2. WHEN the journal state is `Written`, THE Restore_Executor SHALL re-open each staged file with a new read handle and compare its SHA-256, size, and permission bits with the restore plan.
3. WHEN the journal state is `Written`, THE Restore_Executor SHALL walk the staged tree without following symlinks and compare its set of entries with the restore plan.
4. IF a staged file differs from the restore plan, or the staged tree has an extra or missing entry, THEN THE Restore_Executor SHALL fail with `RESTORE_VERIFY_FAILED`, name the entries, and roll back.
5. WHEN staged verification passes, THE Restore_Executor SHALL record `Verified` in the journal.
6. WHEN the journal state is `Verified`, THE Restore_Executor SHALL commit with one `renameatx_np` call with `RENAME_EXCL` that moves `tree` from the staging folder to the target name in the destination parent.
7. IF the commit rename fails with `ENOTSUP`, THEN THE Restore_Executor SHALL fail with `RESTORE_VOLUME_UNSUPPORTED` and roll back.
8. WHEN the commit rename succeeds, THE Restore_Executor SHALL sync the destination parent directory and then record `Committed` in the journal.
9. WHEN the journal state is `Committed`, THE Restore_Executor SHALL delete the staging folder after the owner token check in Requirement 12.5 and record `Cleaned`.
10. IF any step before the commit rename fails or is cancelled, THEN THE Restore_Executor SHALL leave the target uncreated, delete the staging folder after the owner token check in Requirement 12.5, record `Cleaned`, and set the report status to `RolledBack`.
11. WHEN the user cancels through `job_cancel` before the commit rename starts, THE Restore_Executor SHALL stop at the next step boundary and fail with `CANCELLED`.
12. IF the user cancels after the commit rename has started, THEN THE Restore_Executor SHALL continue and send the progress event "Restore can no longer be cancelled".
13. WHILE a restore runs, THE Restore_Executor SHALL send progress events over the Tauri Channel with the journal state and real counts of files and bytes done and total.

### Requirement 10: Post-commit verification and Git check (RST-4)

**User Story:** As a developer, I want the restored folder re-checked independently after it appears, so that the "files restored" claim rests on a second reading of the actual result.

#### Acceptance Criteria

1. WHEN the journal state is `Cleaned` after a commit, THE Post_Verifier SHALL open the target with a new read-only handle and re-hash every restored file, comparing with the manifest's `sha256` and `size` and not with hashes computed while writing.
2. IF a restored file is missing or differs from the manifest, THEN THE Post_Verifier SHALL list it under mismatches and set the report status to `Failed`.
3. THE Post_Verifier SHALL never delete or modify the target, including when verification fails.
4. WHERE the capsule contains `.git`, THE Git_Checker SHALL read HEAD and refs from the restored `.git` with the Ref_Reader and compare them with the manifest's `git` section, in both the native app and the browser demo.
5. IF HEAD or any ref differs from the manifest, THEN THE Git_Checker SHALL record `Mismatch`, name each differing ref, and set the report status to `Failed`.
6. WHERE the native app runs, THE Git_Checker SHALL compute working-tree status counts (modified, staged, untracked, deleted, conflicted) with `dormbit-git` and compare them with the manifest's `git.status`.
7. IF the working-tree status counts differ, THEN THE Git_Checker SHALL record `Differs` with both sets of counts.
8. WHERE the browser demo runs, THE Git_Checker SHALL record the working-tree status as "Not checked in the browser".
9. IF the capsule contains no `.git`, THEN THE Git_Checker SHALL record the Git check as "Not applicable".

### Requirement 11: Restore journal (RST-5)

**User Story:** As a developer, I want every restore step recorded durably, so that DORMBIT always knows what it was doing if the Mac crashes.

#### Acceptance Criteria

1. THE Journal_Store SHALL keep each restore's journal at `~/Library/Application Support/io.github.<owner>.dormbit/restores/<op_id>/journal.json` with at least the fields `op_id`, `staging`, `owner_token`, and `state`.
2. THE Journal_Store SHALL write each journal update atomically: a new temp file created with create-new semantics, `sync_all`, a rename over `journal.json`, then a sync of the folder.
3. THE Restore_Executor SHALL advance the state only in the order `Staged`, `Written`, `Verified`, `Committed`, `Cleaned`, and SHALL skip directly to `Cleaned` only during rollback.
4. THE Restore_Executor SHALL record `Written`, `Verified`, `Committed`, and `Cleaned` only after the step each names is complete and synced.
5. IF a journal write fails before the commit rename, THEN THE Restore_Executor SHALL roll back and fail with `RESTORE_IO_FAILED`.
6. IF a journal write fails after the commit rename, THEN THE Restore_Executor SHALL stop, leave the staging folder for recovery, and fail with `RESTORE_IO_FAILED`.
7. THE Restore_Executor SHALL append one record to the write log for each directory created, file created, rename, and staging folder deletion, naming the root (`staging` or `target`) and the relative path.
8. THE Journal_Store SHALL sync the write log before each journal state change.
9. THE Journal_Store SHALL create journal, write log, and report files with mode `0600`.

### Requirement 12: Crash recovery and deletion rules (RST-5)

**User Story:** As a developer, I want DORMBIT to finish or undo an interrupted restore on the next launch, and to delete only what it can prove it created, so that a crash never leaves a mess or costs me data.

#### Acceptance Criteria

1. WHEN the app launches and holds the single-instance lock, THE Recovery_Manager SHALL list every journal whose state is not `Cleaned` and every `Cleaned` journal that has no report.
2. WHEN that list is not empty, THE Recovery_Prompt SHALL show each destination parent (with `~`), target name, and last recorded state before the user can start a new restore.
3. WHEN the user confirms recovery and the staging folder still contains `tree`, THE Recovery_Manager SHALL delete the staging folder after the owner token check in criterion 5, record `Cleaned`, and write a report with status `RolledBack`.
4. WHEN the user confirms recovery, the staging folder no longer contains `tree`, and the target's device and inode numbers equal the journal's tree ID, THE Recovery_Manager SHALL treat the restore as committed, delete the staging folder after the owner token check in criterion 5, record `Cleaned`, and run the post-commit verification in Requirement 10.
5. THE Restore_Executor and the Recovery_Manager SHALL delete a staging folder only after confirming that it is a directory (not a symlink) with the journal's staging name, and that its `.dormbit-owner` is a regular file of at most 4 KiB whose `op_id` and owner token equal the journal's.
6. IF the owner token check fails, THEN THE Recovery_Manager SHALL delete nothing and show the staging path as "Not deleted: DORMBIT could not confirm it created this folder".
7. IF the staging folder does not exist, THEN THE Recovery_Manager SHALL delete nothing, record `Cleaned`, and run the post-commit verification only when the target's device and inode numbers equal the journal's tree ID.
8. IF the `RestoreTarget` grant has been revoked or its folder is unreachable, THEN THE Recovery_Manager SHALL keep the journal, delete nothing, and show the restore as "Unreachable".
9. IF a journal file can't be parsed, THEN THE Recovery_Manager SHALL delete nothing and show it as "Unreadable journal" with its path written with `~`.
10. THE Recovery_Manager SHALL never delete, rename, or modify the target.
11. THE Restore_Commands SHALL perform recovery actions only through `restore_recover`.
12. THE Restore_Executor and the Recovery_Manager SHALL delete no file or folder other than a staging folder that passed the check in criterion 5.

### Requirement 13: No hooks, no overwrite, no execution (RST-6, §7 Git config and hooks)

**User Story:** As a developer, I want restore to be unable to run code or replace anything, so that restoring an untrusted capsule is safe.

#### Acceptance Criteria

1. THE Restore_Planner SHALL exclude every manifest entry whose path's fold key equals `.git/hooks` or starts with `.git/hooks/`, whatever the volume, and list it as not restored with reason `git_hooks_never_restored`.
2. THE Restore_Executor SHALL never open an existing file for writing, truncate a file, or rename onto an existing name.
3. THE Restore_Executor SHALL create no directory with a call that succeeds when the directory already exists.
4. THE Restore_Executor SHALL create, modify, or delete no entry in the destination parent other than its own staging folder and the target.
5. THE Restore_Executor, the Post_Verifier, and the Git_Checker SHALL start no process and SHALL never run the git CLI.
6. WHEN a restored `.git/config` sets `core.fsmonitor`, `core.hooksPath`, or a `filter.*` command, THE Git_Checker SHALL complete its checks without running that command.

### Requirement 14: Restore report (RST-7, §7 source changes and later command execution)

**User Story:** As a developer, I want a complete, honest report of what was restored, what wasn't, and what to watch out for, so that I can trust the result before I open the project.

#### Acceptance Criteria

1. WHEN a restore ends by success, failure, rollback, or recovery, THE Report_Builder SHALL write `report.json` next to the journal and THE Restore_Executor SHALL send the report in the final Channel event.
2. THE Report_Builder SHALL list the counts and byte totals of files written and files verified.
3. THE Report_Builder SHALL list each mismatch with its path and the expected and actual SHA-256, size, and mode.
4. THE Report_Builder SHALL list every manifest entry that wasn't restored with a reason: the capture reason from the manifest's `excluded[]`, `git_hooks_never_restored`, `symlink_not_restored`, or (P1) `symlink_unsafe_target`.
5. THE Report_Builder SHALL list each risky Git config key (`core.fsmonitor`, `core.hooksPath`, `filter.*`, `core.sshCommand`, `credential.helper`) found in the restored `.git/config`, with credentials in values hidden.
6. THE Report_Builder SHALL list every restored file that has an execute bit set.
7. THE Report_Builder SHALL list every restored file whose setuid, setgid, or sticky bit was removed.
8. THE Report_Builder SHALL record the source-unchanged check as `Unchanged`, `Changed` with the changed paths, or `NotChecked` with the reason.
9. WHERE the source project is reachable through a `Read` grant, THE Restore_Executor SHALL snapshot the type, size, mode, modification time, and inode of each manifest entry path in the source project when the restore starts and again when it ends, and THE Report_Builder SHALL compare the two snapshots.
10. THE Report_Builder SHALL compute "writes outside the destination" as the number of write log records whose root is neither the staging folder nor the target.
11. IF the count of writes outside the destination is greater than 0, THEN THE Report_Builder SHALL set the report status to `Failed`.
12. THE Report_Builder SHALL include the Git check results from Requirement 10.
13. THE Report_Builder SHALL state these limits: SHA-256 shows integrity, not authorship; the OS cache may serve the bytes DORMBIT reads; modification times are not restored.
14. THE Report_Builder SHALL write every path in the report with the home folder as `~`.

### Requirement 15: Restore Wizard screen (Task 26, UX-1)

**User Story:** As a developer, I want a guided restore flow that shows each check and real progress, so that I understand what will happen before and while it happens.

#### Acceptance Criteria

1. THE Restore_Wizard SHALL list capsules from `capsules_list` and let the user select only capsules whose latest verification status is `Verified`.
2. THE Restore_Wizard SHALL show each capsule that can't be selected with its status glyph and label (for example "Failed ✕") and the reason.
3. WHEN the user chooses "Choose destination folder…", THE Restore_Wizard SHALL call `restore_target_pick` and show the returned `root_display`.
4. WHEN the capsule, the destination parent, or the target name changes, THE Restore_Wizard SHALL call `restore_precheck` and show each check with its glyph, label, and detail.
5. WHILE any blocking pre-check fails, THE Restore_Wizard SHALL disable "Start restore" and show the failing check's message.
6. WHILE a restore runs, THE Restore_Wizard SHALL show the states `Staged`, `Written`, `Verified`, `Committed`, `Cleaned` as progress steps with the real file and byte counts from Channel events.
7. WHILE the restore has not reached the commit rename, THE Restore_Wizard SHALL enable "Cancel", and after that SHALL disable it with the text "Can't cancel after commit".
8. WHEN a command returns a `DormbitError`, THE Restore_Wizard SHALL show its code, its message, and a next action (for example "Choose another name" for `RESTORE_DEST_EXISTS`).
9. THE Restore_Wizard SHALL support keyboard-only use, VoiceOver labels, and reduced motion, and SHALL never use color as the only signal of a status.
10. IF a command response fails Zod validation, THEN THE Restore_Wizard SHALL show an error and SHALL NOT continue to the next step.

### Requirement 16: Restoration Result screen (Task 26, UX-1)

**User Story:** As a developer, I want the result screen to show exactly what the report says, so that I can see what was proven and what wasn't.

#### Acceptance Criteria

1. WHEN a restore ends, THE Result_Screen SHALL show every report field listed in Requirement 14.
2. THE Result_Screen SHALL show "Writes outside the destination: <count>" as a labeled line with the source "from the restore journal".
3. WHEN the report status is `RolledBack`, THE Result_Screen SHALL show "The destination folder was not created." and the error code.
4. WHEN the report status is `Failed` after a commit, THE Result_Screen SHALL show "DORMBIT did not delete the restored folder." and list the mismatches.
5. THE Result_Screen SHALL never state that the project builds or runs, or that an AI session was resumed.
6. THE Result_Screen SHALL support keyboard-only use, VoiceOver labels, and reduced motion, and SHALL never use color as the only signal of a status.

### Requirement 17: Restore internal relative symlinks (P1) (CAP-6, §3 P1 item 3, §7 symlink escape)

**User Story:** As a developer, I want symlinks that stay inside my project restored, so that the restored tree matches the original more closely without opening an escape path.

#### Acceptance Criteria

1. WHERE internal symlink restore (P1) is enabled, THE Restore_Planner SHALL mark a symlink entry restorable only if its target is non-empty, relative, at most 1023 bytes, free of NUL and control characters, and resolves lexically from the link's folder to a path inside the target.
2. WHERE internal symlink restore (P1) is enabled, THE Restore_Planner SHALL mark a symlink entry not restorable if its lexical resolution passes through another symlink entry or lands on `.git/hooks` or inside it.
3. WHERE internal symlink restore (P1) is enabled, THE Restore_Executor SHALL create restorable symlinks through `cap-std` only after all planned directories and files exist and are synced, without following any symlink.
4. WHERE internal symlink restore (P1) is enabled, THE Restore_Executor and the Post_Verifier SHALL compare each restored symlink's target with the manifest's `target`.
5. IF a symlink entry is not restorable, THEN THE Restore_Planner SHALL list it as not restored with reason `symlink_unsafe_target`.
6. WHILE internal symlink restore (P1) is not enabled, THE Restore_Planner SHALL list every symlink entry as not restored with reason `symlink_not_restored`.

### Requirement 18: Write the brief after verification (P1) (BRF-3)

**User Story:** As a developer, I want the Resurrection Brief written into the restored project only after the restore is proven, so that an agent can read it without it being confused with capsule content.

#### Acceptance Criteria

1. WHERE the brief write (P1) is enabled, THE Restore_Executor SHALL write `RESURRECTION_BRIEF.md` into the target only after the post-commit verification status is `Verified`.
2. WHERE the brief write (P1) is enabled, THE Restore_Executor SHALL create `RESURRECTION_BRIEF.md` with create-new semantics.
3. IF an entry named `RESURRECTION_BRIEF.md` already exists in the target, THEN THE Restore_Executor SHALL not write the brief and THE Report_Builder SHALL record "Not written: a file with this name already exists".
4. WHERE the brief write (P1) is enabled, THE Restore_Executor SHALL append a write log record for the brief with root `target`.
5. WHERE the brief write (P1) is enabled, THE Report_Builder SHALL list the brief under added files as "Not part of the capsule", with its size and SHA-256.
6. WHERE the brief write (P1) is enabled, THE Post_Verifier SHALL leave the brief out of the files verified and the mismatches.

## Open questions
1. **Error codes not in §8.** §8 says the stable codes "include" a list, so it isn't closed, but it has no code for four cases these criteria need. This spec proposes `RESTORE_NOT_VERIFIED` (4.1), `RESTORE_TARGET_FORBIDDEN` (3.2, 3.3), `RESTORE_VERIFY_FAILED` (9.4), and `RESTORE_IO_FAILED` (8.11, 11.5, 11.6). Conservative choice applied: separate codes instead of reusing a code with a different meaning. They need approval before Task 23 adds them to `DormbitError`.
2. **Capsules not in the registry (4.4).** VER-1 says such a capsule is "Unknown, not Failed". Blocking it would make restoring a capsule from another Mac's drive impossible, so 4.4 allows it with a visible warning. A `Mismatch` still blocks (4.3).
3. **Working-tree status after exclusions (10.7).** Files excluded at capture (for example an untracked `.env` that isn't gitignored) change the status counts after restore. Conservative choice applied: the report says `Differs` and never `Match` unless the counts are equal. A path-level status list in the manifest would let the report explain each difference; that belongs to `capsule-and-verification`.
4. **Refs in the manifest (10.4).** RST-4 requires refs to match the manifest. If the manifest's `git` section records only HEAD, the Git check compares HEAD and the report says refs weren't recorded. `capsule-and-verification` should record a `refs` map.
5. **Modification times (14.13).** No requirement in the plan restores mtimes, so this spec doesn't, and the report says so.
6. **Symlink `restored: false` (17).** §8 writes `restored: false` into symlink entries at capture. If the P1 feature ships, the restore report, not that manifest field, is the record of what was restored.
