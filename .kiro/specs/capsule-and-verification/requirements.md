# Requirements Document

## Introduction
This spec covers the capsule system: the capture plan engine, the manifest and capsule writer, the independent verifier, and the Plan, Capture, and Verification screens. It implements master plan Phase 04 (P04, Tasks 19-22). It formalizes plan requirements CAP-1..CAP-7, CPS-1, CPS-2, VER-1, and VER-2, the capsule format v1 and error codes in §8, and the §7 threats for archive bombs, malformed tar, path traversal in capsules, tampering, secret leakage, and code running through Git hooks. Restore (RST-*) is out of scope and belongs to the `transactional-restore` spec.

## Glossary
- **Project**: a project root found by discovery (`scope-and-discovery` spec), identified by a project ID.
- **Source tree**: the files under a Project's root, read only through the read-only `Vfs` of a Read scope grant.
- **Scope grant**: a folder the user picked in a native picker, referred to by grant ID. Kinds: `Read`, `CapsuleLibrary`, `RestoreTarget`.
- **Capsule library**: a folder granted with kind `CapsuleLibrary`, where capsules are written.
- **Capture plan**: the editable list of every Source tree item with its group, reason code, decision, and editable flag (`CapturePlan` in §8).
- **Plan item**: one entry in a Capture plan: `{path, kind, size, decision, reason, editable, warning?}`.
- **Group**: one of the six plan groups: Included, Excluded: rebuildable, Excluded: sensitive, Needs decision, Unreadable, Unsupported.
- **Reason code**: a stable snake_case string that says why a Plan item is in its Group (for example `sensitive_env`).
- **Decision**: `Include`, `Exclude`, or `Undecided` for a Plan item.
- **Sensitive file**: a file matching the v1 sensitive rules: `.env*` except example files, private keys, `.pem`, `.p12`, and credential files.
- **Secret-like content**: file content that matches the secret scanner's patterns. It produces a warning, not an exclusion.
- **Acknowledgment**: a user confirmation of one plan warning, recorded in the Approved plan.
- **Git treatment**: `Full` (capture `.git` except hooks, lock files, and fsmonitor state) or `WorkingTreeOnly` (capture no `.git` content).
- **Plan digest**: the SHA-256 of the canonical serialization of a Capture plan.
- **Approved plan**: the approval record `{digest, approved_at, acks[], overrides[]}` (§8).
- **Approved set**: the Plan item paths with Decision `Include` in an Approved plan.
- **Capsule**: a `.dormbit` file in format v1: an uncompressed ustar archive with entries `DORMBIT`, `manifest.json`, `objects/<sha256>`, `END`, in that order.
- **Marker**: the `DORMBIT` entry, containing the line `dormbit-capsule/1`.
- **Manifest**: the `manifest.json` entry describing the Capsule's contents.
- **Object**: an `objects/<sha256>` entry holding one gzip stream of one unique file content. `<sha256>` is the hash of the uncompressed content.
- **Trailer**: the `END` entry, holding the Manifest SHA-256 and the Object count.
- **Stored hash**: SHA-256 of an Object's bytes as stored in the tar (`stored_sha256`).
- **Content hash**: SHA-256 of an Object's bytes after decompression (`sha256`).
- **Capsule registry**: the local atomic JSON record of capsules this install wrote, with whole-file SHA-256 and Manifest SHA-256.
- **Capsule limits**: configurable bounds on per-file size, entry count, capsule size, Manifest size, and expansion ratio. Defaults are documented in `docs/architecture/capsule-format.md`.
- **Job**: a long-running operation (capture, verify) that advances through `step(budget)` and reports real file and byte counts.
- **Capture_Planner**: the `dormbit-core::plan` module that builds, edits, digests, and approves Capture plans.
- **Secret_Scanner**: the `dormbit-core::plan::secrets` module that flags Secret-like content.
- **Capsule_Writer**: the `dormbit-core::capsule::writer` module that runs the three-step capture and assembles the Capsule bytes.
- **Capsule_Sink**: the `dormbit-fs` component that writes the temp file, calls `sync_all`, and renames it into place without overwriting.
- **Capsule_Registry**: the `dormbit-fs` component that stores registry records.
- **Verifier**: the `dormbit-core::capsule::verifier` module that reopens a Capsule and parses it strictly. It shares no parsing code with the Capsule_Writer.
- **Verification report**: `VerificationReport` in §8, with overall status Verified, Failed, or Unsupported.
- **Desktop_UI**: the React screens Preservation Plan (6), Capsule Creation (7), and Integrity Verification (8) in `packages/ui`, with their Tauri commands.
- **Safe path**: a relative path with `/` separators, no `..` or `.` components, no empty component, no NUL or control characters, valid UTF-8, each component at most 255 bytes, and total length at most 1024 bytes (rules from RST-2).

## Requirements

### Requirement 1: Plan groups and reason codes (CAP-1)

**User Story:** As a developer shelving a project, I want every file sorted into a named group with a reason, so that I can see exactly what will and won't be preserved.

#### Acceptance Criteria

1. WHEN `plan_create` runs for a Project, THE Capture_Planner SHALL assign every Source tree item to exactly one Group.
2. THE Capture_Planner SHALL give every Plan item a Reason code, a Decision, and an editable flag.
3. WHEN an item matches more than one classification rule, THE Capture_Planner SHALL apply the rules in the fixed precedence order documented in the design and record only the first matching Reason code.
4. THE Capture_Planner SHALL set the default Decision to `Include` for Included items, `Undecided` for Needs decision items, and `Exclude` for all other Groups.
5. IF an item cannot be read through the `Vfs`, THEN THE Capture_Planner SHALL place it in Unreadable with Reason code `unreadable`, Decision `Exclude`, and editable `false`.
6. IF an item's name is not valid UTF-8 or the path is not a Safe path, THEN THE Capture_Planner SHALL place it in Unsupported with Reason code `unsafe_name` and editable `false`.
7. IF two item paths are equal after NFC normalization, THEN THE Capture_Planner SHALL place both in Unsupported with Reason code `name_collision` and editable `false`.
8. IF a file is larger than the per-file size limit, THEN THE Capture_Planner SHALL place it in Unsupported with Reason code `over_size_limit` and editable `false`.
9. WHEN a plan is built, THE Capture_Planner SHALL compute `totals` with item counts and byte sums per Group and per Decision.
10. THE Capture_Planner SHALL read the Source tree only through the read-only `Vfs` and SHALL never follow symlinks.
11. WHEN the Dockline fixture is planned, THE Capture_Planner SHALL produce output equal to the committed golden plan snapshot.

### Requirement 2: Sensitive files excluded with no override (CAP-2)

**User Story:** As a developer, I want credentials and keys kept out of every capsule, so that a capsule can't leak them.

#### Acceptance Criteria

1. WHEN a file name starts with `.env` and is not an example file, THE Capture_Planner SHALL place it in Excluded: sensitive with Reason code `sensitive_env`.
2. WHEN a file name is an example file (`.env.example`, `.env.sample`, `.env.template`), THE Capture_Planner SHALL classify it with the non-sensitive rules.
3. WHEN a file has extension `.pem` or `.p12`, THE Capture_Planner SHALL place it in Excluded: sensitive with Reason code `sensitive_cert`.
4. WHEN a file is a private key by name (`id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`, `*.key`) or begins with a PEM `PRIVATE KEY` header, THE Capture_Planner SHALL place it in Excluded: sensitive with Reason code `sensitive_private_key`.
5. WHEN a file name is on the credential-file list in the design, THE Capture_Planner SHALL place it in Excluded: sensitive with Reason code `sensitive_credentials`.
6. THE Capture_Planner SHALL set editable `false` and Decision `Exclude` on every Excluded: sensitive item.
7. IF `plan_set_decision` requests `Include` for an Excluded: sensitive item, THEN THE Capture_Planner SHALL reject the edit with `PLAN_ITEM_LOCKED` and leave the plan and its digest unchanged.
8. IF any Approved set path matches the sensitive rules at capture time, THEN THE Capsule_Writer SHALL fail with `PATH_UNSAFE` naming that path and write no Capsule.
9. THE Capture_Planner SHALL apply the sensitive rules before the rebuildable, data-file, gitignore, and Git-treatment rules.

### Requirement 3: Secret-like content warning and acknowledgment (CAP-3)

**User Story:** As a developer, I want a warning when a file I'm keeping looks like it contains a secret, so that I decide knowingly.

#### Acceptance Criteria

1. WHEN a Plan item with Decision `Include` or `Undecided` contains Secret-like content, THE Secret_Scanner SHALL attach warning `secret_like_content` to that item.
2. THE Secret_Scanner SHALL report the item path and a line number for each warning, and SHALL NOT store or display the matched text.
3. IF `plan_approve` is called while an item with Decision `Include` has a warning without an Acknowledgment, THEN THE Capture_Planner SHALL refuse approval with `PLAN_INCOMPLETE` and record nothing.
4. WHEN the user acknowledges a warning, THE Capture_Planner SHALL record the warning ID in `ApprovedPlan.acks`.
5. THE Secret_Scanner SHALL scan `.git/config` and every working-tree file with Decision `Include` or `Undecided`.
6. WHEN the Secret_Scanner skips a file (`.git/objects/**`, `.git/index`, or a binary file detected by a NUL byte), THE Capture_Planner SHALL count it in plan `totals.secret_scan_skipped`.
7. THE test suite SHALL generate secret-like test strings at runtime, and the repository SHALL contain no committed secret-like test strings (checked by gitleaks).
8. THE Capture_Planner SHALL NOT let an Acknowledgment change the Group or Decision of any item.

### Requirement 4: Gitignored files and data files (CAP-4)

**User Story:** As a developer, I want gitignored files that can't be rebuilt and local databases brought to my attention, so that I don't lose notes or data that `git push` leaves behind.

#### Acceptance Criteria

1. WHEN a file is gitignored and is not classified rebuildable by discovery, THE Capture_Planner SHALL place it in Needs decision with Reason code `gitignored_not_rebuildable`.
2. WHEN a file matches `*.sqlite`, `*.db`, or a database dump pattern from the design, THE Capture_Planner SHALL place it in Needs decision with Reason code `data_file`.
3. WHEN a path is inside a folder that discovery classifies as rebuildable (for example `node_modules/`, `dist/`), THE Capture_Planner SHALL place it in Excluded: rebuildable with Reason code `rebuildable` and editable `true`.
4. IF `plan_approve` is called while any item has Decision `Undecided`, THEN THE Capture_Planner SHALL refuse approval with `PLAN_INCOMPLETE`.
5. WHEN the Dockline fixture is planned, THE Capture_Planner SHALL place `.env` in Excluded: sensitive and `data/dev.sqlite` in Needs decision.

### Requirement 5: `.git` treatment (CAP-5)

**User Story:** As a developer, I want my repository history preserved without anything that can run code or hold stale locks, so that the capsule is complete and safe.

#### Acceptance Criteria

1. WHILE Git treatment is `Full`, THE Capture_Planner SHALL place `.git` contents in Included, except the items in criteria 2-4.
2. THE Capture_Planner SHALL place every path under `.git/hooks/` in Unsupported with Reason code `git_hooks` and editable `false`.
3. THE Capture_Planner SHALL place every file under `.git/` whose name ends in `.lock` in Unsupported with Reason code `git_lock` and editable `false`.
4. THE Capture_Planner SHALL place `.git/fsmonitor--daemon/` contents and `.git/fsmonitor--daemon.ipc` in Unsupported with Reason code `git_fsmonitor` and editable `false`.
5. WHERE the user selects Git treatment `WorkingTreeOnly`, THE Capture_Planner SHALL set Decision `Exclude` with Reason code `git_working_tree_only` on every `.git` item.
6. WHEN Git treatment changes, THE Capture_Planner SHALL produce a new Plan digest.
7. THE Capsule_Writer SHALL record the Git treatment in the Manifest `git.treatment` field.
8. WHILE Git treatment is `Full`, THE Capsule_Writer SHALL record HEAD (branch or detached commit) and all refs in the Manifest `git` field, read by the core ref reader from `.git/HEAD`, `.git/refs/`, and `.git/packed-refs`.
9. IF `.git` is a file (linked worktree or submodule pointer), THEN THE Capture_Planner SHALL capture that file as a normal Included item and add plan warning `git_external_dir`.

### Requirement 6: Symlinks and special files (CAP-6)

**User Story:** As a developer, I want symlinks noted and special files skipped, so that the capsule can't be used to reach outside the project.

#### Acceptance Criteria

1. WHEN the Source tree contains a symlink, THE Capture_Planner SHALL place it in Included with kind `Symlink`, Reason code `symlink_record_only`, and editable `false`.
2. THE Capture_Planner SHALL read a symlink's target string without following it.
3. THE Capsule_Writer SHALL write each symlink as a Manifest entry `{type: "symlink", path, target, restored: false}` and SHALL NOT write an Object or a tar link entry for it.
4. WHEN the Source tree contains a FIFO, socket, block device, or character device, THE Capture_Planner SHALL place it in Unsupported with Reason code `special_file` and editable `false`.
5. THE Capture_Planner SHALL NOT descend into a symlinked directory.

### Requirement 7: Plan digest and approval record (CAP-7)

**User Story:** As a developer, I want approval tied to exactly the plan I reviewed, so that nothing changes between my review and the capture.

#### Acceptance Criteria

1. THE Capture_Planner SHALL compute the Plan digest as the SHA-256 of the canonical plan serialization defined in the design.
2. WHEN any item's Decision changes, THE Capture_Planner SHALL produce a different Plan digest.
3. WHEN the same plan content is serialized twice, THE Capture_Planner SHALL produce the same Plan digest.
4. IF `plan_approve` receives a digest that differs from the plan's current digest, THEN THE Capture_Planner SHALL refuse with `PLAN_DIGEST_MISMATCH` and record nothing.
5. WHEN `plan_approve` succeeds, THE Capture_Planner SHALL record an Approved plan with the digest, `approved_at` from the injected `Clock`, the acknowledged warning IDs, and the list of Decisions that differ from defaults (`overrides`).
6. IF `plan_set_decision` targets an item whose editable flag is `false`, THEN THE Capture_Planner SHALL reject the edit with `PLAN_ITEM_LOCKED` and leave the plan unchanged.
7. WHEN a plan is edited after approval, THE Capture_Planner SHALL discard that approval.

### Requirement 8: Capture limited to the Approved set (CAP-7, PROP-7)

**User Story:** As a developer, I want the capsule to contain exactly what I approved, so that the plan is a promise.

#### Acceptance Criteria

1. WHEN capture starts, THE Capsule_Writer SHALL recompute the Plan digest and SHALL fail with `PLAN_DIGEST_MISMATCH` if it differs from the Approved plan digest.
2. THE Capsule_Writer SHALL write a Manifest entry for every Approved set path and for no other path.
3. WHEN capture starts, THE Capsule_Writer SHALL re-list the Source tree and record every file not present in the plan in Manifest `excluded[]` with Reason code `appeared_after_approval`.
4. WHEN files appeared after approval, THE Desktop_UI SHALL list them as not captured on the Capture result.
5. IF an Approved set path is missing, or its kind changed (for example a file became a symlink or directory), THEN THE Capsule_Writer SHALL fail with `FILE_CHANGED_DURING_CAPTURE` naming that path.
6. THE Capsule_Writer SHALL record every Plan item with Decision `Exclude` in Manifest `excluded[]` with its Group and Reason code.
7. THE Capsule_Writer SHALL write the Approved plan digest to Manifest `plan_digest`.

### Requirement 9: Three-step capture (CPS-1)

**User Story:** As a developer, I want capture to detect files changing under it, so that a capsule never holds a torn copy.

#### Acceptance Criteria

1. WHEN capture runs step 1, THE Capsule_Writer SHALL read each Approved set file once, compute its Content hash, gzip it, and store the Object in a temporary object store keyed by Content hash.
2. WHEN two Approved set files have the same Content hash, THE Capsule_Writer SHALL store one Object for both.
3. WHEN capture runs step 2, THE Capsule_Writer SHALL re-read and re-hash every Approved set file and re-read every Approved symlink target.
4. IF any step 2 hash, size, or symlink target differs from step 1, THEN THE Capsule_Writer SHALL fail with `FILE_CHANGED_DURING_CAPTURE` naming the changed path and write no Capsule.
5. WHEN capture runs step 3, THE Capsule_Writer SHALL write the Marker, then the Manifest, then the Objects, then the Trailer.
6. WHILE copying each Object into the Capsule in step 3, THE Capsule_Writer SHALL re-hash its stored bytes and compare them with the step 1 Stored hash.
7. IF a step 3 Stored hash differs from step 1, THEN THE Capsule_Writer SHALL fail with `CAPSULE_HASH_MISMATCH` and write no Capsule.
8. IF capture fails or is cancelled, THEN THE Capsule_Writer SHALL remove its own temp folder after checking its owner token and SHALL leave the Capsule library otherwise unchanged.
9. THE Capsule_Writer SHALL never write to the Source tree.

### Requirement 10: Durable no-clobber write and registry record (CPS-2)

**User Story:** As a developer, I want a capsule to appear only when it is complete and on disk, so that a crash or name clash can't leave a half-written or overwritten file.

#### Acceptance Criteria

1. THE Capsule_Sink SHALL write the Capsule bytes to a temp file inside a temp folder in the Capsule library.
2. WHEN all bytes are written, THE Capsule_Sink SHALL call `sync_all` on the temp file before renaming it.
3. THE Capsule_Sink SHALL rename the temp file to `<capsule_id>.dormbit` with an exclusive rename that fails if the name exists.
4. IF a file named `<capsule_id>.dormbit` already exists, THEN THE Capsule_Sink SHALL fail with `CAPSULE_EXISTS` and leave the existing file unchanged.
5. WHEN the rename succeeds, THE Capsule_Sink SHALL sync the Capsule library directory.
6. WHEN the Capsule is in place, THE Capsule_Registry SHALL record `capsule_id`, project ID, Capsule library grant ID, file name, whole-file SHA-256, Manifest SHA-256, Plan digest, size, and `created_at`.
7. THE Capsule_Writer SHALL compute the whole-file SHA-256 from the exact bytes passed to the Capsule_Sink.
8. IF the registry write fails after the rename, THEN THE Capsule_Sink SHALL report `IO_FAILED` and leave the Capsule in place, so the Verifier reports it as not in the registry.
9. IF the Capsule library grant is missing or not of kind `CapsuleLibrary`, THEN THE Capsule_Writer SHALL fail with `SCOPE_DENIED` before reading any file.

### Requirement 11: Capsule format v1 and determinism (§8, PROP-10)

**User Story:** As a judge or maintainer, I want the same inputs to give the same capsule bytes in the app and the browser, so that golden tests and the demo prove the real code.

#### Acceptance Criteria

1. THE Capsule_Writer SHALL write an uncompressed ustar archive whose entries are, in order: `DORMBIT`, `manifest.json`, `objects/<sha256>` sorted by ascending lowercase hex, `END`.
2. THE Capsule_Writer SHALL set every tar header's uid and gid to 0, uname and gname to empty, and mtime to the Manifest `created_at` in Unix seconds.
3. THE Capsule_Writer SHALL set tar mode 0755 on an Object referenced by at least one entry with an executable bit, and 0644 on every other entry.
4. THE Capsule_Writer SHALL write each Object as a gzip stream with header mtime 0, no file name, no comment, and a fixed compression level.
5. THE Capsule_Writer SHALL serialize the Manifest as compact UTF-8 JSON with fields in schema order and entries sorted by path bytes.
6. THE Capsule_Writer SHALL take `capsule_id` from the injected `IdGen` and `created_at` from the injected `Clock`.
7. WHEN the same Source tree, Approved plan, `Clock`, and `IdGen` are used, THE Capsule_Writer SHALL produce byte-identical Capsules on every run.
8. WHEN the same inputs are used natively and in `wasm32-unknown-unknown`, THE Capsule_Writer SHALL produce byte-identical Capsules.
9. WHEN the Dockline fixture is captured with the fixed test `Clock` and `IdGen`, THE Capsule_Writer SHALL produce bytes equal to the committed golden capsule.
10. THE Capsule_Writer SHALL produce a Capsule that `tar -tvf` lists without error.

### Requirement 12: Limits and archive-bomb defense (§7, §8)

**User Story:** As a user opening a capsule from anywhere, I want hostile archives stopped before they use unbounded memory or disk, so that verification is safe on untrusted input.

#### Acceptance Criteria

1. THE Capsule_Writer and THE Verifier SHALL read Capsule limits from settings, with defaults documented in `docs/architecture/capsule-format.md`.
2. IF the Approved set exceeds the entry-count limit, THEN THE Capture_Planner SHALL refuse approval with `CAPSULE_LIMIT_EXCEEDED`.
3. IF the assembled Capsule would exceed the capsule size limit, THEN THE Capsule_Writer SHALL fail with `CAPSULE_LIMIT_EXCEEDED` and write no Capsule.
4. IF a Capsule's file size, tar entry count, Manifest size, or any declared file size exceeds a limit, THEN THE Verifier SHALL report status Unsupported with `CAPSULE_LIMIT_EXCEEDED` and stop reading.
5. IF an Object's declared size divided by its `stored_size` exceeds the expansion-ratio limit, THEN THE Verifier SHALL report status Unsupported with `CAPSULE_LIMIT_EXCEEDED` without decompressing it.
6. WHILE decompressing an Object, THE Verifier SHALL stop as soon as output exceeds the Manifest's declared size and report that file as damaged.
7. THE Verifier SHALL bound its memory use by the Manifest size limit plus a fixed buffer, independent of Object sizes.

### Requirement 13: Verifier container checks (VER-1)

**User Story:** As a user, I want the verifier to reject any archive that isn't exactly what the writer produces, so that malformed input never reaches restore.

#### Acceptance Criteria

1. THE Verifier SHALL reopen the Capsule from its file and SHALL NOT use any in-memory state from the Capsule_Writer.
2. THE Verifier SHALL parse tar headers with its own strict parser and SHALL NOT use the `tar` crate's reader.
3. IF the first entry is not named `DORMBIT` or its content does not start with `dormbit-capsule/`, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
4. IF the Marker content is `dormbit-capsule/<N>` followed by a newline, where N is a decimal integer other than `1`, THEN THE Verifier SHALL report Unsupported with `CAPSULE_VERSION_UNSUPPORTED`.
5. IF the Marker content starts with `dormbit-capsule/` but is neither exactly `dormbit-capsule/1` plus a newline nor the form in criterion 4, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
6. IF any header checksum is wrong or any header field differs from the format in the design, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
7. IF any entry's typeflag is not `0` (regular file), THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
8. IF any entry name is not `DORMBIT`, `manifest.json`, `objects/` followed by 64 lowercase hex digits, or `END`, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
9. IF entries are out of the required order or an entry name appears twice, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
10. IF any data padding byte or end-of-archive block byte is non-zero, or any byte follows the two end-of-archive blocks, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
11. IF the file ends before the Trailer and the two end-of-archive blocks are read, THEN THE Verifier SHALL report Failed with `CAPSULE_TRUNCATED`.
12. IF the Trailer's Manifest SHA-256 or Object count differs from the actual values, THEN THE Verifier SHALL report Failed with `CAPSULE_HASH_MISMATCH`.
13. THE Verifier SHALL never use a tar entry name as a filesystem path.

### Requirement 14: Verifier manifest and object checks (VER-1)

**User Story:** As a user, I want every file's bytes and every path checked against the manifest, so that verification means the content is intact.

#### Acceptance Criteria

1. THE Verifier SHALL parse the Manifest strictly, rejecting unknown fields, missing fields, and wrong types with `CAPSULE_MALFORMED`.
2. IF Manifest `schema_version` is not `1`, THEN THE Verifier SHALL report Unsupported with `CAPSULE_VERSION_UNSUPPORTED`.
3. IF any Manifest entry path is not a Safe path, THEN THE Verifier SHALL report Failed with `PATH_UNSAFE` naming that path.
4. IF two Manifest entry paths are equal or equal after NFC normalization, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED` naming both paths.
5. IF a path is both an entry and a parent folder of another entry, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
6. IF a referenced Object is missing, or an Object is present that no entry references, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
7. IF an Object's tar size differs from `stored_size` or its bytes' SHA-256 differs from `stored_sha256`, THEN THE Verifier SHALL report Failed with `CAPSULE_HASH_MISMATCH` and add every entry referencing that Object to `failed_files`.
8. IF an Object's decompressed size differs from the entry `size` or its Content hash differs from the entry `sha256` or the Object name, THEN THE Verifier SHALL report Failed with `CAPSULE_HASH_MISMATCH` and add every entry referencing that Object to `failed_files`.
9. WHEN one Object fails a hash check, THE Verifier SHALL continue checking the remaining Objects.
10. IF a structural check stops parsing, THEN THE Verifier SHALL list every entry whose Object was not checked in `failed_files` with reason `unverifiable`.
11. IF Manifest `totals` disagree with the entries, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.
12. IF a symlink entry has `restored` not `false`, an empty target, or a target containing NUL, THEN THE Verifier SHALL report Failed with `CAPSULE_MALFORMED`.

### Requirement 15: Registry match (VER-1)

**User Story:** As a user re-verifying an old capsule, I want to know whether it matches what this Mac recorded, so that I can spot replacement or tampering.

#### Acceptance Criteria

1. WHEN verification runs, THE Verifier SHALL compute the whole-file SHA-256 and the Manifest SHA-256 from the bytes it reads.
2. WHEN the registry has a record with the same `capsule_id` and both hashes equal, THE Verifier SHALL set `registry_match` to `Match`.
3. IF the registry has a record with the same `capsule_id` and either hash differs, THEN THE Verifier SHALL set `registry_match` to `Mismatch` and report Failed.
4. WHEN the registry has no record for the `capsule_id`, THE Verifier SHALL set `registry_match` to `NotInRegistry` and the registry check status to Unknown.
5. THE Verifier SHALL NOT report Failed only because a Capsule is not in the registry.

### Requirement 16: Report statuses and stated limits (VER-2)

**User Story:** As a user, I want a plain verdict that names damaged files and says what verification can't prove, so that I trust it for the right reasons.

#### Acceptance Criteria

1. WHEN every check passes and `registry_match` is `Match` or `NotInRegistry`, THE Verifier SHALL report status Verified.
2. IF any check fails, THEN THE Verifier SHALL report status Failed and list damaged files in `failed_files` with path and reason.
3. IF the Marker version, `schema_version`, or a Capsule limit is outside what this build supports, THEN THE Verifier SHALL report status Unsupported.
4. THE Verifier SHALL include one entry per check ID in `checks[]` with status Pass, Fail, Unknown, or NotRun and a detail string.
5. THE Verifier SHALL include in every report the limitations "SHA-256 shows integrity, not authorship" and "The operating system's cache may serve the bytes that were read".
6. THE Verifier SHALL record `file_sha256` and `manifest_sha256` in the report when they were computed.
7. WHEN a report is Verified and the Capsule is in the registry, THE Capsule_Registry SHALL record `last_verified_at` from the injected `Clock`.
8. THE Verifier SHALL NOT write to the Capsule file.

### Requirement 17: Step jobs, progress, and cancellation (§6)

**User Story:** As a user, I want capture and verification to show real progress and stop when I cancel, so that long operations stay under my control in the app and the browser.

#### Acceptance Criteria

1. THE Capsule_Writer and THE Verifier SHALL each expose a Job with `step(budget)` that does at most the budgeted work and returns progress or a result.
2. WHEN a step returns, THE Job SHALL report its phase, files done and total, and bytes done and total, counted from real file and byte operations.
3. WHEN `job_cancel` is called before the capture rename, THE Capsule_Writer SHALL stop at the next step, remove its own temp folder, and report `CANCELLED`.
4. IF `job_cancel` is called after the capture rename, THEN THE Capsule_Writer SHALL finish the registry record and report that the Capsule was already written.
5. WHEN `job_cancel` is called during verification, THE Verifier SHALL stop and report `CANCELLED` with no status change in the registry.
6. THE Job SHALL produce the same final result for any sequence of budgets.

### Requirement 18: Plan, Capture, and Verification screens (Task 22)

**User Story:** As a developer, I want to review the plan, approve it, watch capture, and see the verification result in the app, so that I can preserve a project without the CLI.

#### Acceptance Criteria

1. WHEN the Preservation Plan screen opens, THE Desktop_UI SHALL show the six Groups with item counts, byte totals, and each item's Reason code in plain words.
2. THE Desktop_UI SHALL allow Decision changes only on items whose editable flag is `true`.
3. WHILE any item is `Undecided` or any warning lacks an Acknowledgment, THE Desktop_UI SHALL disable Approve and say what remains.
4. WHEN the user approves, THE Desktop_UI SHALL call `plan_approve` with the digest it displayed.
5. WHEN capture runs, THE Desktop_UI SHALL show each capture phase with real file and byte counts received over a Tauri Channel.
6. THE Desktop_UI SHALL offer Cancel during capture and verification and call `job_cancel`.
7. WHEN capture finishes, THE Desktop_UI SHALL start a separate verification Job with `verify_start`.
8. THE Desktop_UI SHALL show the capsule seal as pressed only when the latest Verification report status is Verified.
9. WHEN a report is Failed, THE Desktop_UI SHALL list each damaged file and its reason.
10. THE Desktop_UI SHALL show each status with a glyph and a label, never color alone.
11. THE Desktop_UI SHALL refer to the Capsule library and capsules by grant ID and capsule ID, never by raw path.
12. THE Desktop_UI SHALL show the Verifier's stated limitations on the Integrity Verification screen.
13. THE Desktop_UI SHALL render project file names and paths as text, never as raw HTML.

### Requirement 19: Shared contracts for capsule types (PROP-11)

**User Story:** As a maintainer, I want the plan, approval, progress, manifest, and report types defined once, so that Rust and TypeScript can't drift.

#### Acceptance Criteria

1. THE build SHALL export `CapturePlan`, `PlanItem`, `ApprovedPlan`, `Manifest`, `JobProgress`, `VerificationReport`, and `CapsuleRecord` from Rust with ts-rs into `packages/schema/generated`.
2. THE `packages/schema` package SHALL define a Zod schema for each exported type.
3. WHEN any valid Rust value of these types is serialized to JSON, THE Zod schema SHALL parse it, and re-serializing the parsed value SHALL produce the same JSON value.
4. IF a command receives an input that fails Zod or serde validation, THEN THE command SHALL return a typed `DormbitError` and change no state.

## Open questions
1. The plan doesn't list example-file names for `.env*`. This spec uses `.env.example`, `.env.sample`, and `.env.template` and treats every other `.env*` as sensitive (the safer side).
2. CAP-4 says gitignored, non-rebuildable files are "shown for review". This spec places them in Needs decision so they block approval until reviewed. That's stricter than the plan's wording.
3. Git hooks, lock files, and fsmonitor state don't fit Excluded: rebuildable or Excluded: sensitive, so this spec puts them in Unsupported with non-editable reason codes.
4. §8 lists stable error codes as "include". This spec needs five more: `PLAN_INCOMPLETE`, `PLAN_ITEM_LOCKED`, `CAPSULE_LIMIT_EXCEEDED`, `CAPSULE_EXISTS`, `IO_FAILED`. They need adding to `DormbitError` (foundation Task 11) and §8.
5. The plan gives no default values for Capsule limits. They must be set in `docs/architecture/capsule-format.md` (Task 7) before Task 20 starts.
6. Limit violations are reported as Unsupported, not Failed, because the capsule may be undamaged. Restore still refuses any capsule that isn't Verified.
7. `VerificationReport` in §8 has no limitations field. Criterion 16.5 needs one; this spec adds `limitations[]`.
8. `.git/config` can hold credentials in remote URLs. This spec relies on the Secret_Scanner warning for it. Whether to strip or exclude remote credentials at capture is not decided in the plan.
