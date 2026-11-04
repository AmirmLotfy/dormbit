# Capsule format v1 (`.dormbit`)

- **Status:** Normative for v1. Draft until the v1.0.0 release (Task 53), frozen after it.
- **Task:** P01-T02 (Task 7)
- **Source of truth:** [`docs/MASTER_PLAN.md`](../MASTER_PLAN.md) §4 (CPS-1, CPS-2, VER-1, VER-2, RST-2) and §8. Container choice: [ADR-002](../adr/ADR-002-capsule-container.md).
- **Formalized in:** the `capsule-and-verification` spec (P04).

The key words MUST, MUST NOT, and MAY are normative. "Writer" is the capsule writer in `dormbit-core`. "Verifier" is the separate verifier in `dormbit-core`. "Reader" is any code that reads a capsule, including restore.

## 1. Overview

A capsule is an **uncompressed POSIX ustar archive** of regular-file entries, in this order:

| # | Tar entry name | Content |
|---|---|---|
| 1 | `DORMBIT` | Marker: `dormbit-capsule/1\n` |
| 2 | `manifest.json` | Canonical JSON manifest (§4) |
| 3 | `objects/<sha256>` (zero or more) | One blob per distinct file content, `gzip` or `identity` encoded (§5) |
| 4 | `END` | Trailer with the manifest digest and object count (§7) |

The archive ends with exactly 1024 zero bytes (two zero blocks) and nothing after them. There is no record padding beyond that.

Tar entry names are **never** used as output paths. Restore writes only manifest paths that pass §6.

Standard tools can read a capsule: `tar -tvf x.dormbit` lists it, and `tar -xOf x.dormbit objects/<sha256> | gunzip` recovers one gzip-encoded file by hand.

## 2. Tar entry order

1. The first entry MUST be `DORMBIT`.
2. The second entry MUST be `manifest.json`.
3. Then one entry per distinct `sha256` value referenced by file entries in the manifest, named `objects/<sha256>`, in **strictly ascending** order of the 64-character lowercase hex name. Strict order means no duplicates.
4. The last entry MUST be `END`.
5. After `END`'s data and padding: exactly two 512-byte blocks of zero bytes, then end of file.

No other entry names are allowed. Directories, symlinks, hard links, PAX headers, GNU long-name headers, and device entries are not allowed.

## 3. Fixed header fields

Every entry uses a 512-byte ustar header. All fields are fixed except `name`, `mode`, `size`, `mtime`, and `chksum`. The canonical header is what `tar` 0.4.46 `Header::new_ustar()` produces after setting path, size, mode, mtime, uid 0, gid 0, entry type Regular, and checksum.

Numeric fields are ASCII octal, left-padded with `0` to fill the field, with the last byte NUL (as `tar` 0.4.46 `octal_into` writes them).

| Field | Offset | Length | Value |
|---|---|---|---|
| `name` | 0 | 100 | Entry name (§1), NUL-padded. Longest is `objects/` + 64 hex = 72 bytes |
| `mode` | 100 | 8 | `0000644\0`, or `0000755\0` for an object referenced by at least one file entry with any execute bit (`mode & 0o111 != 0`). Marker, manifest, and `END` are always `0000644\0` |
| `uid` | 108 | 8 | `0000000\0` |
| `gid` | 116 | 8 | `0000000\0` |
| `size` | 124 | 12 | Data length, 11 octal digits + NUL |
| `mtime` | 136 | 12 | The manifest's `created_at` as Unix seconds, 11 octal digits + NUL. Same value for every entry |
| `chksum` | 148 | 8 | POSIX header checksum (sum of all 512 header bytes with this field counted as 8 spaces), 7 octal digits + NUL |
| `typeflag` | 156 | 1 | `0` (regular file) |
| `linkname` | 157 | 100 | All NUL |
| `magic` | 257 | 6 | `ustar\0` |
| `version` | 263 | 2 | `00` |
| `uname` | 265 | 32 | All NUL |
| `gname` | 297 | 32 | All NUL |
| `devmajor` | 329 | 8 | All NUL |
| `devminor` | 337 | 8 | All NUL |
| `prefix` | 345 | 155 | All NUL |
| (padding) | 500 | 12 | All NUL |

Entry data follows the header and is padded with zero bytes to the next 512-byte boundary.

The tar header mode is informational. Restore takes the mode from the manifest entry (§4.3), never from the tar header.

## 4. `manifest.json`

### 4.1 Encoding

- UTF-8, no byte-order mark, no trailing newline.
- **Canonical form:** compact JSON as `serde_json::to_vec` writes it. No insignificant whitespace. Object keys appear in the order listed in this section. Non-ASCII characters are written as raw UTF-8, not `\u` escapes. Only `"`, `\`, and control characters are escaped. `/` is not escaped.
- Every field listed is always present. Optional values are written as `null`, never omitted.
- No unknown fields. No duplicate keys. No floating-point numbers.
- All integers are in `0 ..= 9007199254740991` (2^53−1) so they survive a JavaScript round trip (PROP-11).
- The manifest digest is the SHA-256 of the exact `manifest.json` data bytes.

### 4.2 Top-level fields

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `schema_version` | integer | `1` |
| 2 | `capsule_id` | string | Lowercase hyphenated UUID, 36 chars, from the host `IdGen` |
| 3 | `created_at` | string | UTC, exactly `YYYY-MM-DDTHH:MM:SSZ`, from the host `Clock` at capture start, truncated to seconds. Year 1970 or later |
| 4 | `app_version` | string | DORMBIT version that wrote the capsule (semver), at most 64 bytes |
| 5 | `project` | object | §4.4 |
| 6 | `plan_digest` | string | `sha256:` + 64 lowercase hex. The approved plan digest (CAP-7) |
| 7 | `git` | object or `null` | §4.5. `null` when the project has no Git repository |
| 8 | `entries` | array | §4.3. Sorted strictly ascending by `path` as UTF-8 bytes |
| 9 | `excluded` | array | §4.6. Sorted ascending by `path` bytes |
| 10 | `warnings` | array | §4.7 |
| 11 | `limitations` | array of strings | §4.8 |
| 12 | `agent_refs` | array | §4.9. Empty in P0 |
| 13 | `totals` | object | §4.10 |

### 4.3 Entries

Each entry is an object tagged by `kind`. `kind` is always the first key.

**File entry** (`kind: "file"`):

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `kind` | string | `"file"` |
| 2 | `path` | string | §6 |
| 3 | `size` | integer | Content length in bytes. At most the per-file limit (LIM-2) |
| 4 | `mode` | integer | Permission bits as captured, `0 ..= 4095` (`0o7777`). JSON has no octal, so `0o644` is written `420`. Restore strips setuid, setgid, and sticky bits and keeps the rest (RST-3) |
| 5 | `mtime` | integer | Source modification time, Unix seconds. Times before 1970 are written as `0` |
| 6 | `sha256` | string | 64 lowercase hex: SHA-256 of the content |
| 7 | `object` | object | `{encoding, stored_size, stored_sha256}`, below |

`object` fields, in order:

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `encoding` | string | `"gzip"` or `"identity"` |
| 2 | `stored_size` | integer | Length of the `objects/<sha256>` tar data |
| 3 | `stored_sha256` | string | 64 lowercase hex: SHA-256 of the stored bytes. For `identity`, equals `sha256`, and `stored_size` equals `size` |

All file entries with the same `sha256` MUST have identical `object` values.

**Symlink entry** (`kind: "symlink"`):

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `kind` | string | `"symlink"` |
| 2 | `path` | string | §6 |
| 3 | `target` | string | The link text as read, at most 1024 bytes, no NUL. Recorded only; never followed, never validated as a path |
| 4 | `restored` | boolean | `false` in v1 (CAP-6) |

Directories are not entries. Restore creates parent directories implied by file paths.

### 4.4 `project`

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `name` | string | Project display name, at most 256 bytes |
| 2 | `name_source` | string | Where the name came from (for example `package.json:name` or `folder`), at most 256 bytes |

The manifest never records absolute paths, so a shared capsule doesn't reveal the user's folder layout or account name.

### 4.5 `git`

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `treatment` | string | `"repository"` (`.git` captured, minus §6.3 paths) or `"working_tree_only"` (CAP-5) |
| 2 | `head` | object | `{branch, detached, commit}`: `branch` string or `null`; `detached` boolean; `commit` 40 or 64 lowercase hex, or `null` for an unborn branch |
| 3 | `refs` | array | `[{name, target}]` sorted by `name`: every ref the core ref reader finds in loose `refs/**` and `packed-refs`. `target` is lowercase hex. Used by the post-restore HEAD and refs check (RST-4) |
| 4 | `status` | object | `{modified, staged, untracked, deleted, conflicted}`, integers, as of capture. Used by the native working-tree check (RST-4) |
| 5 | `stash_count` | integer | |
| 6 | `risky_config` | array of strings | Sorted config **keys** only (for example `core.fsmonitor`, `core.hooksPath`, `filter.lfs.process`). Values are not stored |

With `working_tree_only`, `head`, `refs`, and `status` are informational. The restore report shows the Git check as not applicable.

### 4.6 `excluded`

| # | Field | Type | Rules |
|---|---|---|---|
| 1 | `path` | string | Project-relative path. Follows §6.1 syntax rules |
| 2 | `is_dir` | boolean | `true` when a whole folder was excluded (for example `node_modules`) |
| 3 | `group` | string | One of `rebuildable`, `sensitive`, `user_excluded`, `unreadable`, `unsupported`, `special_file`, `git_internal`, `not_in_plan` |
| 4 | `reason` | string | The capture plan's reason code for the item (CAP-1), at most 64 bytes |

`not_in_plan` lists files that appeared after approval (CAP-7). Names are recorded; contents never are.

### 4.7 `warnings`

`[{code, message, paths}]`, sorted by `code`, then by `paths`. `code` is a string of at most 64 bytes. `message` is at most 1024 bytes. `paths` is a sorted array of project-relative paths. A warning names files and line numbers only. It MUST NOT contain the matched text of a suspected secret (CAP-3).

### 4.8 `limitations`

The v1 writer always writes exactly these strings, in this order:

1. `SHA-256 shows integrity, not authorship.`
2. `The capsule is not encrypted.`
3. `Symlinks are recorded, not restored.`
4. `Empty directories are not recorded.`

### 4.9 `agent_refs` (P1)

`[{tool, session_id, cwd_display, started_at, ended_at, branch, tool_version, note}]`, sorted by `tool`, then `session_id`. Metadata only (AGT-1):

| Field | Type | Rules |
|---|---|---|
| `tool` | string | `claude_code`, `codex`, or `kiro_export` |
| `session_id` | string | At most 256 bytes |
| `cwd_display` | string or `null` | Home folder written as `~` |
| `started_at`, `ended_at` | string or `null` | Same format as `created_at` |
| `branch`, `tool_version` | string or `null` | At most 256 bytes each |
| `note` | string | `not resumed by DORMBIT` (AGT-2) |

Message content and transcripts are never stored in a capsule.

### 4.10 `totals`

`{files, symlinks, objects, bytes, stored_bytes, excluded}`, all integers:
- `files`, `symlinks`: counts of each entry kind.
- `objects`: number of `objects/` tar entries (distinct `sha256` values).
- `bytes`: sum of `size` over all file entries, duplicates included. This is what restore writes.
- `stored_bytes`: sum of `stored_size` over distinct objects.
- `excluded`: length of `excluded`.

## 5. Object encoding

- **Content addressing:** an object's name is the lowercase hex SHA-256 of the uncompressed content. Identical contents are stored once.
- **Choosing the encoding (deterministic):** the writer compresses with `flate2` 1.1.10, `rust_backend`, level 6. It uses `gzip` only if the gzip stream is shorter than the content **and** the result satisfies LIM-6 with the default ratio. Otherwise it uses `identity`. The choice depends only on content, so native and WebAssembly builds produce the same bytes (PROP-10).
- **`gzip`:** exactly one gzip member, no trailing bytes. Header bytes: `1f 8b 08 00 00 00 00 00 00 ff` (CM 8, FLG 0 so no name, comment, or extra field, MTIME 0, XFL 0 for level 6, OS 255). CRC32 and ISIZE as RFC 1952 requires.
- **`identity`:** the stored bytes are the content.
- An empty file is always `identity` with 0 stored bytes. Its name is `e3b0c442…b855`, the SHA-256 of empty input.

## 6. Path rules

### 6.1 Syntax (every `entries[].path`, `excluded[].path`, and `warnings[].paths[]`)

A path MUST:
1. be valid UTF-8, stored exactly as read from the source (not re-normalized);
2. be relative: no leading `/`;
3. use `/` as the only separator, with no empty components (no `//`, no trailing `/`);
4. contain no `.` or `..` component;
5. contain no NUL, no C0 control characters (U+0001–U+001F), no U+007F, and no C1 control characters (U+0080–U+009F);
6. contain no `\` (so a capsule can't produce traversal on systems that treat `\` as a separator);
7. have every component at most 255 bytes, and the whole path at most 1024 bytes (RST-2).

Files whose names break these rules are placed in the plan's Unsupported group and appear in `excluded` with group `unsupported`. Since such a name can't be written in `excluded` either, the writer records the parent folder with a reason code instead.

### 6.2 Uniqueness and structure (entries only)

1. No two entries have the same path (guaranteed by strict sort).
2. No two entries are equal after Unicode NFC normalization.
3. No entry path is a proper ancestor of another entry path, compared after NFC. A file `a` and a file `a/b` can't both exist. A symlink entry can't be an ancestor of anything.
4. Paths that are equal only after case-folding are **allowed** in the capsule, because a case-sensitive source can hold them. The verifier reports them as a warning. Restore refuses them on case-insensitive volumes with `RESTORE_COLLISION` (RST-2, PROP-8).

### 6.3 Forbidden Git paths (entries only)

No entry path may:
- start with `.git/hooks/`, or match `.git/modules/<…>/hooks/<…>` (RST-6);
- be under `.git/` with a final component ending in `.lock`;
- be under `.git/` with a component starting with `fsmonitor--daemon`.

## 7. `END` trailer

ASCII, LF line endings, exactly three lines, nothing else:

```
dormbit-end/1
manifest_sha256=<64 lowercase hex>
objects=<decimal count, no leading zeros>
```

- `manifest_sha256` is the manifest digest (§4.1).
- `objects` equals the number of `objects/` entries and `totals.objects`.
- Length is 105 + (digits in the count − 1) bytes. For 3 objects it is 105 bytes.

A missing `END` means the file was cut short. The verifier reports `CAPSULE_TRUNCATED`.

## 8. Limits

Defaults are conservative. Every limit is configurable through the core's limits setting (the exact settings surface is defined in the `capsule-and-verification` spec). Hard ceilings can't be raised.

| ID | Limit | Default | Hard ceiling |
|---|---|---|---|
| LIM-1 | Manifest entries (files + symlinks) | 100,000 | 1,000,000 |
| LIM-2 | Per-file `size` | 1 GiB (1,073,741,824 bytes) | 8,589,934,591 bytes (ustar `size` field) |
| LIM-3 | `totals.bytes` | 4 GiB (4,294,967,296 bytes) | 2^53−1 |
| LIM-4 | Capsule file size | 5 GiB (5,368,709,120 bytes) | 2^53−1 |
| LIM-5 | `manifest.json` size | 64 MiB (67,108,864 bytes) | 512 MiB |
| LIM-6 | Expansion of a `gzip` object: `size ≤ max(200 × stored_size, 1 MiB)` | ratio 200, floor 1 MiB | ratio 1,032 (DEFLATE's maximum) |
| LIM-7 | `excluded` items | 100,000 | 1,000,000 |
| LIM-8 | `warnings` items, `agent_refs` items | 10,000 each | 100,000 each |

Path limits (255 bytes per component, 1024 bytes per path) and string lengths in §4 are format rules, not configurable limits.

- **Writer:** files over LIM-2 go to the plan's Unsupported group. A plan whose included total exceeds LIM-3 can't be approved. The writer never produces a capsule its own default verifier would reject.
- **Verifier:** checks LIM-1 to LIM-8 from the manifest **before** reading or decompressing any object. While decompressing, it stops at `size + 1` output bytes, so a stream can never expand past its declared size.

## 9. Verifier checks

The verifier reopens the capsule file and reads it once, front to back, computing the whole-file SHA-256 on the way. Each check has a stable ID that appears in `VerificationReport.checks[].id`. Check status is `Pass`, `Fail`, `Warn`, or `Unknown`.

**When it stops:** a failure in CON, MRK, MAN, PTH, or LIM stops the run, because later bytes can't be parsed safely. A failure in OBJ is recorded and the verifier moves to the next object, so the report can name every damaged file. `failed_files` lists every entry path that references a failed object.

| ID | Check | Error code on failure |
|---|---|---|
| CON-1 | Every header and its data are present before end of file | `CAPSULE_TRUNCATED` |
| CON-2 | Every header checksum is correct | `CAPSULE_MALFORMED` |
| CON-3 | Every header is byte-equal to the canonical header for its name, size, mode, and `created_at` (§3) | `CAPSULE_MALFORMED` |
| CON-4 | All data padding bytes are zero | `CAPSULE_MALFORMED` |
| CON-5 | Entry sequence is exactly §2; after `END`, exactly 1024 zero bytes, then end of file | `CAPSULE_MALFORMED` (`CAPSULE_TRUNCATED` if the file ends early) |
| CON-6 | Every entry name is on the §1 allowlist; object names are 64 lowercase hex | `CAPSULE_MALFORMED` |
| MRK-1 | Marker data is exactly `dormbit-capsule/1\n`. A well-formed `dormbit-capsule/<N>\n` with N ≠ 1 gives status Unsupported | `CAPSULE_MALFORMED` or `CAPSULE_VERSION_UNSUPPORTED` |
| MAN-1 | Manifest size within LIM-5 | `CAPSULE_MALFORMED` |
| MAN-2 | Strict parse: UTF-8, known fields only, no duplicate keys, types and ranges as §4 | `CAPSULE_MALFORMED` |
| MAN-3 | `schema_version` is `1`, otherwise status Unsupported | `CAPSULE_VERSION_UNSUPPORTED` |
| MAN-4 | Re-serializing the parsed manifest gives the same bytes (canonical form) | `CAPSULE_MALFORMED` |
| MAN-5 | `created_at` is well formed and equals every header's `mtime` | `CAPSULE_MALFORMED` |
| MAN-6 | `entries` strictly ascending by path bytes; `excluded` ascending; `limitations` exactly §4.8 | `CAPSULE_MALFORMED` |
| MAN-7 | `totals` equal the values computed from the manifest | `CAPSULE_MALFORMED` |
| MAN-8 | Entry field rules: `mode ≤ 4095`, hex formats, `identity` objects have `stored_size = size` and `stored_sha256 = sha256`, symlink `restored = false`, all entries sharing a `sha256` share one `object` value | `CAPSULE_MALFORMED` |
| LIM-1..8 | Limits from §8, checked before any object is read | `CAPSULE_MALFORMED` (detail names the limit and says it is configurable) |
| PTH-1 | Every path passes §6.1 | `PATH_UNSAFE` |
| PTH-2 | No NFC-equal entry paths (§6.2.2) | `PATH_UNSAFE` |
| PTH-3 | No entry path is an ancestor of another (§6.2.3) | `PATH_UNSAFE` |
| PTH-4 | No forbidden Git paths (§6.3) | `PATH_UNSAFE` |
| PTH-5 | Case-fold collisions present (§6.2.4). Status `Warn`, never `Fail` | none |
| OBJ-1 | The set of object entries equals the set of distinct file `sha256` values: each present once, none missing, none extra | `CAPSULE_MALFORMED` |
| OBJ-2 | Object entries are strictly ascending | `CAPSULE_MALFORMED` |
| OBJ-3 | Each object's tar `size` equals `stored_size`, and its header `mode` follows §3 | `CAPSULE_MALFORMED` |
| OBJ-4 | SHA-256 of the stored bytes equals `stored_sha256` | `CAPSULE_HASH_MISMATCH` |
| OBJ-5 | `gzip` objects: header exactly §5, one member, no trailing bytes, CRC32 and ISIZE correct | `CAPSULE_MALFORMED` |
| OBJ-6 | Decoded length equals `size` (decoding stops at `size + 1`) | `CAPSULE_MALFORMED` |
| OBJ-7 | SHA-256 of the decoded content equals the object name and the entries' `sha256` | `CAPSULE_HASH_MISMATCH` |
| END-1 | `END` is present as the last entry | `CAPSULE_TRUNCATED` |
| END-2 | Trailer text matches §7 exactly | `CAPSULE_MALFORMED` |
| END-3 | `manifest_sha256` equals the manifest digest | `CAPSULE_HASH_MISMATCH` |
| END-4 | `objects` equals the object entries read and `totals.objects` | `CAPSULE_MALFORMED` |
| REG-1 | Registry lookup by `capsule_id`: `Match` (file and manifest SHA-256 equal the registry) → `Pass`; `Mismatch` → `Fail`; `NotInRegistry` → `Unknown` | `CAPSULE_HASH_MISMATCH` on `Mismatch` |

**Overall status (VER-2):**
- **Unsupported:** MRK-1 or MAN-3 found another version. Nothing else is checked.
- **Failed:** any check is `Fail`. The report names the damaged files.
- **Verified:** every check is `Pass`, `Warn`, or `Unknown`, and only REG-1 may be `Unknown`. A capsule that isn't in the registry can still be Verified; the report says "not in this Mac's registry".

**Stated limits** (shown with every report): SHA-256 shows integrity, not authorship. Anyone who can rewrite both a capsule and the registry can produce a capsule that verifies. The OS file cache may serve the bytes the verifier reads, so a pass doesn't prove the bytes on the physical disk are good.

**Restore re-checks.** A capsule can change after verification. Restore re-reads every object it writes and repeats OBJ-4 to OBJ-7 while streaming (RST-3). A mismatch rolls the restore back and the destination is never created (journey 6, PROP-5).

## 10. Versioning and compatibility

- **Two version numbers.** The container version is in the marker (`dormbit-capsule/1`) and the trailer (`dormbit-end/1`). The manifest version is `schema_version`. v1 is container 1 with schema 1.
- **What forces a bump.** Any change to entry names, order, header rules, encodings, padding, or the trailer bumps the container version. Any change to manifest fields bumps `schema_version`. The v1 parser rejects unknown fields, so even an added field is a new version.
- **Readers support an explicit list.** A v1 reader accepts only (container 1, schema 1). Anything else is reported as Unsupported with `CAPSULE_VERSION_UNSUPPORTED`. Nothing is restored from it, and the reader never guesses.
- **Backward compatibility after release.** From v1.0.0 (Task 53), the v1 byte layout is frozen. Later DORMBIT versions keep reading v1 capsules. The v1 golden capsule built from the Dockline fixture stays in the repo as a compatibility test.
- **Before release.** The format may change during development. Golden capsules are regenerated only by the main agent at merge points (plan §10).
- **Future features** (encryption, Ed25519 signing, partial salvage; all P2) arrive as new versions, not as optional fields in v1.
- **File name.** The `.dormbit` file name is not part of the format. The verifier ignores it.

## 11. Worked example: a 3-file capsule

Inputs (the same three files as the Task 3 spike):

| Path | Content | Size | SHA-256 |
|---|---|---|---|
| `README.md` | `# Dockline Invoices\n\nPaused mid-PDF-export.\n` | 44 | `3af3ee427e3ccc8ab1648fedd1539a97990e10cadfa825750cc9d59d48240e2b` |
| `package.json` | `{\n  "name": "dockline-invoices"\n}\n` | 34 | `170f9beb77950743bc9aaf0b9400883f1a14625f42fe4c294b73436a541b651a` |
| `src/pdf/renderInvoice.ts` | `// TODO: finish PDF layout\nexport function render() {}\n` | 55 | `04db5703386f1e47629bcba803d530eea49d17ed66be597d77cce284360ab228` |

Settings: `created_at` `2025-01-01T00:00:00Z` (Unix 1735689600, octal `14735102600`), all files mode `0o644` (420), mtime 1735603200, no Git repository, nothing excluded. Each gzip stream would be longer than its content (64, 54, and 75 bytes against 44, 34, and 55), so all three objects are `identity`. `plan_digest` and `capsule_id` are example values.

**`manifest.json`** (1486 bytes, the exact data bytes):

```json
{"schema_version":1,"capsule_id":"6f1d2a33-8011-4c7e-9a01-23456789abcd","created_at":"2025-01-01T00:00:00Z","app_version":"0.1.0","project":{"name":"dockline-invoices","name_source":"package.json:name"},"plan_digest":"sha256:7900d73df80b731950b740f9191bfafcc272a3763ad4f329cdcedb9595bb371b","git":null,"entries":[{"kind":"file","path":"README.md","size":44,"mode":420,"mtime":1735603200,"sha256":"3af3ee427e3ccc8ab1648fedd1539a97990e10cadfa825750cc9d59d48240e2b","object":{"encoding":"identity","stored_size":44,"stored_sha256":"3af3ee427e3ccc8ab1648fedd1539a97990e10cadfa825750cc9d59d48240e2b"}},{"kind":"file","path":"package.json","size":34,"mode":420,"mtime":1735603200,"sha256":"170f9beb77950743bc9aaf0b9400883f1a14625f42fe4c294b73436a541b651a","object":{"encoding":"identity","stored_size":34,"stored_sha256":"170f9beb77950743bc9aaf0b9400883f1a14625f42fe4c294b73436a541b651a"}},{"kind":"file","path":"src/pdf/renderInvoice.ts","size":55,"mode":420,"mtime":1735603200,"sha256":"04db5703386f1e47629bcba803d530eea49d17ed66be597d77cce284360ab228","object":{"encoding":"identity","stored_size":55,"stored_sha256":"04db5703386f1e47629bcba803d530eea49d17ed66be597d77cce284360ab228"}}],"excluded":[],"warnings":[],"limitations":["SHA-256 shows integrity, not authorship.","The capsule is not encrypted.","Symlinks are recorded, not restored.","Empty directories are not recorded."],"agent_refs":[],"totals":{"files":3,"symlinks":0,"objects":3,"bytes":133,"stored_bytes":133,"excluded":0}}
```

Manifest digest: `9331af502cabcfe2c908b85f2d4054c36ed6df8a56167020ed8e06962c686af5`.

Entries are in path byte order (`R` 0x52 sorts before `p` 0x70). Objects are in hash order (`04db…` < `170f…` < `3af3…`), which is a different order.

**`END`** (105 bytes):

```
dormbit-end/1
manifest_sha256=9331af502cabcfe2c908b85f2d4054c36ed6df8a56167020ed8e06962c686af5
objects=3
```

**Archive layout** (all headers: uid/gid `0000000`, typeflag `0`, magic `ustar\0`, mtime `14735102600`):

| Offset | Entry | Header `mode` | Header `size` (octal) | Data bytes | Data + padding |
|---|---|---|---|---|---|
| 0 | `DORMBIT` | `0000644` | `00000000022` | 18 | 512 |
| 1024 | `manifest.json` | `0000644` | `00000002716` | 1486 | 1536 |
| 3072 | `objects/04db5703…360ab228` (`src/pdf/renderInvoice.ts`) | `0000644` | `00000000067` | 55 | 512 |
| 4096 | `objects/170f9beb…6a541b651a` (`package.json`) | `0000644` | `00000000042` | 34 | 512 |
| 5120 | `objects/3af3ee42…48240e2b` (`README.md`) | `0000644` | `00000000054` | 44 | 512 |
| 6144 | `END` | `0000644` | `00000000151` | 105 | 512 |
| 7168 | end-of-archive (two zero blocks) | | | | 1024 |

Total: **8192 bytes** (16 blocks). Each row's offset is its 512-byte header; data starts 512 bytes later. The whole-file SHA-256 depends on the header checksums. It is fixed by the golden test in Task 20 and recorded in the registry at capture time.

## Open questions

- **Header byte layout.** §3 describes `tar` 0.4.46 `octal_into` output (zero-padded octal, last byte NUL; checksum as 7 digits + NUL), read from the crate source. Task 20's golden bytes are the final authority. If they disagree with §3, fix this document before v1.0.0.
- **Header mode with shared objects.** The plan says headers use mode 0644 or 0755 but doesn't say which, and content addressing lets one object serve files with different modes. Resolved: 0755 if any referencing entry is executable. Restore never uses header modes, so this only affects `tar -tvf` output.
- **Limit error code.** Plan §8 lists no dedicated code for exceeded limits. Resolved conservatively: `CAPSULE_MALFORMED` with the limit named in the detail. The `capsule-and-verification` spec may add a dedicated code.
- **Registry mismatch.** VER-1 says a capsule missing from the registry is Unknown, not Failed, but doesn't cover a registry entry with a different hash. Resolved conservatively as Failed.
- **Empty directories.** The plan doesn't mention them. v1 doesn't record them, and the manifest's `limitations` says so.
- **Working-tree status check.** RST-4 asks the restored working-tree status to match. v1 stores status counts only, so the check compares counts. Per-path status is left to the `transactional-restore` spec.
- **Unsupported file names.** §6.1 file names that can't be stored are recorded through their parent folder. The exact reason code is set in the capture plan spec.
