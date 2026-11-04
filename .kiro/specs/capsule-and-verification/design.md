# Design Document

## Overview
This design covers master plan Phase 04 (Tasks 19-22). It adds four `dormbit-core` modules (`plan`, `capsule::manifest`, `capsule::writer`, `capsule::verifier`), the `dormbit-fs` pieces that make a capsule durable on disk (temp folder, `sync_all`, exclusive rename, registry), the Tauri commands for the plan, capture, and verification flow, and three UI screens.

The core does no I/O of its own. It reads the source tree through the read-only `Vfs` trait from `scope-and-discovery`, writes through traits defined here, and takes time and IDs from injected `Clock` and `IdGen`. The same code builds for `aarch64-apple-darwin` and `wasm32-unknown-unknown` and produces the same bytes on both (ADR-001, PROP-10).

The capsule container follows ADR-002: an uncompressed ustar archive with fixed headers holding a marker, the manifest, one gzip stream per unique content, and a trailer. The verifier is a separate module with its own strict tar parser. It never shares parsing code with the writer.

## Architecture

```mermaid
flowchart TB
  subgraph UI["packages/ui (React)"]
    PP[Preservation Plan screen] --> CL[DormbitClient]
    CC[Capsule Creation screen] --> CL
    IV[Integrity Verification screen] --> CL
  end
  CL -->|invoke + Channel| CMD["src-tauri commands: plan_create, plan_set_decision, plan_approve, capture_start, job_cancel, capsule_pick, capsules_list, verify_start"]
  CMD --> JR[Job runner thread: calls step budget, streams JobProgress]
  JR --> CORE
  subgraph CORE["dormbit-core (no I/O, forbid unsafe)"]
    PL[plan: classify, secrets, digest, approve]
    MF[capsule::manifest: schema, canonical JSON]
    WR[capsule::writer: CaptureJob, tar assembly]
    VR[capsule::verifier: strict ustar parser, VerifyJob]
    GR[gitref: HEAD, refs, packed-refs reader]
    WR --> MF
    WR --> GR
    VR --> MF
  end
  subgraph FS["dormbit-fs (native)"]
    RV[ReadOnly Vfs over cap-std Dir]
    SK[CapsuleSink: temp folder, owner token, sync_all, rename_excl]
    RG[CapsuleRegistry: atomic JSON]
    CS[CapsuleSource: read-only file handle]
  end
  PL -->|Vfs| RV
  WR -->|Vfs| RV
  WR -->|CaptureWorkspace| SK
  WR -->|Registry| RG
  VR -->|CapsuleSource| CS
  VR -->|RegistryRead| RG
```

**Crate boundaries**
- `dormbit-core` holds all logic in this spec. It has `#![forbid(unsafe_code)]`, no `std::fs`, and no `unwrap`. Any new dependency must build for `wasm32-unknown-unknown` (ADR-001). It uses `sha2`, `flate2` (`rust_backend` only), `tar` (writer side only), `ignore` (gitignore matching, through discovery), `unicode-normalization`, `serde`/`serde_json`, `thiserror`, `uuid` (no RNG features), and `ts-rs`.
- `dormbit-fs` implements `Vfs` (from `scope-and-discovery`), `CaptureWorkspace`, `CapsuleSink`, `CapsuleSource`, and `Registry` on top of `cap-std` handles. It holds the project's single `unsafe` call, the `renameatx_np(RENAME_EXCL)` wrapper.
- `apps/desktop/src-tauri` holds commands and the job runner thread. It refers to folders by grant ID and capsules by capsule ID or picked-handle ID.
- `dormbit-cli` gets `plan`, `capture`, and `verify` subcommands for development and demos.
- `packages/schema`, `packages/client`, `packages/ui` get the shared types, client methods, and screens.

**Capture data flow**
1. `plan_create` builds a `CapturePlan` from the `Vfs` and the discovery results (rebuildable folders, gitignore matcher).
2. The user edits decisions with `plan_set_decision`. Each edit returns a new plan and digest.
3. `plan_approve` checks the digest, unresolved items, and acknowledgments, then stores an `ApprovedPlan`.
4. `capture_start` creates a `CaptureJob`. The runner calls `step(budget)` until done, sending `JobProgress` over a Channel.
5. The job commits the capsule through the `CaptureWorkspace` and writes the registry record.
6. The UI calls `verify_start`. A `VerifyJob` reopens the file and produces a `VerificationReport`.

## Components and Interfaces

### Shared core traits (`dormbit-core/src/env.rs`, `vfs.rs`)
The read-only `ReadVfs` trait, `RelPath`, and `MemVfs` (`crates/dormbit-core/src/vfs/mem.rs`) are defined by `scope-and-discovery`. In this document, "`Vfs`" means that read-only trait. This spec uses its three methods and adds one read-only method, `read_link`, which `ReadVfs` doesn't have yet. None of them follow symlinks.

```rust
// crates/dormbit-core/src/vfs.rs (from scope-and-discovery)
pub trait ReadVfs {
    fn read_dir(&self, dir: &RelPath) -> Result<Vec<DirEntry>, VfsError>;
    fn symlink_metadata(&self, path: &RelPath) -> Result<Metadata, VfsError>;   // lstat semantics
    fn read_limited(&self, path: &RelPath, max_bytes: u64) -> Result<Limited, VfsError>;
    // Added by this spec: the link text, never resolved.
    fn read_link(&self, path: &RelPath) -> Result<String, VfsError>;
}
pub struct Metadata { pub kind: EntryKind, pub len: u64, pub mtime: Timestamp, pub mode: u32 }
pub enum EntryKind { File, Dir, Symlink, Special }
pub enum Limited { Complete(Vec<u8>), TooLarge { len: u64 } }

pub trait Clock { fn now_unix(&self) -> i64; }        // seconds, UTC
pub trait IdGen { fn new_uuid(&self) -> uuid::Uuid; } // host supplies randomness
```

- The writer reads each file with `read_limited(path, limits.max_file_size)`. `TooLarge` at capture time means the file grew after approval and fails with `FILE_CHANGED_DURING_CAPTURE`. Peak memory per file is bounded by `max_file_size`.
- `VfsError::Unsafe` maps to `PATH_UNSAFE`. Other errors during planning put the item in Unreadable. During capture they fail with `FILE_CHANGED_DURING_CAPTURE` if the file is gone, or `IO_FAILED` otherwise.
- `RelPath` is a validated Safe path (relative, `/`-separated, no `.`/`..`/empty components, no NUL or control characters, valid UTF-8, components ≤ 255 bytes, total ≤ 1024 bytes). `RelPath::parse` returns `PATH_UNSAFE` otherwise.

### Plan module (`dormbit-core/src/plan/`)
Files: `model.rs`, `classify.rs`, `secrets.rs`, `digest.rs`, `approve.rs`.

```rust
pub struct PlanInputs<'a> {
    pub project_id: ProjectId,
    pub rebuildable: &'a RebuildableSet,      // from discovery (Task 16)
    pub gitignore: &'a GitignoreMatcher,      // from discovery (Task 16)
    pub git_treatment: GitTreatment,
    pub limits: &'a CapsuleLimits,
}

pub fn build_plan<V: ReadVfs>(vfs: &V, inputs: PlanInputs<'_>, ids: &dyn IdGen)
    -> Result<CapturePlan, DormbitError>;

pub enum PlanEdit {
    Decision { path: RelPath, decision: Decision },
    GitTreatment(GitTreatment),
}

pub fn apply_edit(plan: &CapturePlan, edit: PlanEdit, inputs: PlanInputs<'_>)
    -> Result<CapturePlan, DormbitError>;   // PLAN_ITEM_LOCKED for non-editable items

pub fn plan_digest(plan: &CapturePlan) -> Sha256Hex;

pub fn approve(plan: &CapturePlan, shown_digest: &Sha256Hex, acks: &[WarningId],
               clock: &dyn Clock, limits: &CapsuleLimits)
    -> Result<ApprovedPlan, DormbitError>;  // PLAN_DIGEST_MISMATCH, PLAN_INCOMPLETE, CAPSULE_LIMIT_EXCEEDED

pub fn scan_secrets(bytes: &[u8]) -> Option<SecretHit>; // SecretHit { line: u32 }; never returns matched text
```

**Classification precedence** (first match wins; satisfies Requirement 1.3):

| # | Rule | Group | Reason code | Editable | Default decision |
|---|---|---|---|---|---|
| 1 | Name not UTF-8 or path not a Safe path | Unsupported | `unsafe_name` | no | Exclude |
| 2 | FIFO, socket, block or char device | Unsupported | `special_file` | no | Exclude |
| 3 | Sensitive: `.env*` except `.env.example`, `.env.sample`, `.env.template` | Excluded: sensitive | `sensitive_env` | no | Exclude |
| 4 | Sensitive: `*.pem`, `*.p12` | Excluded: sensitive | `sensitive_cert` | no | Exclude |
| 5 | Sensitive: `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`, `*.key`, or content starting with `-----BEGIN` and containing `PRIVATE KEY-----` in the first line | Excluded: sensitive | `sensitive_private_key` | no | Exclude |
| 6 | Sensitive: credential files `.netrc`, `.npmrc`, `.pypirc`, `credentials`, `credentials.json`, `.credentials.json`, `auth.json`, `.git-credentials` | Excluded: sensitive | `sensitive_credentials` | no | Exclude |
| 7 | `.git/hooks/**` | Unsupported | `git_hooks` | no | Exclude |
| 8 | `.git/**/*.lock` | Unsupported | `git_lock` | no | Exclude |
| 9 | `.git/fsmonitor--daemon/**`, `.git/fsmonitor--daemon.ipc` | Unsupported | `git_fsmonitor` | no | Exclude |
| 10 | NFC collision with another path | Unsupported | `name_collision` | no | Exclude |
| 11 | Inside a rebuildable folder from discovery | Excluded: rebuildable | `rebuildable` | yes | Exclude |
| 12 | Read or stat failed | Unreadable | `unreadable` | no | Exclude |
| 13 | Size over `limits.max_file_size` | Unsupported | `over_size_limit` | no | Exclude |
| 14 | Symlink | Included | `symlink_record_only` | no | Include |
| 15 | `*.sqlite`, `*.sqlite3`, `*.db`, `*.dump`, `*.sql.gz`, `*.bak` of a database | Needs decision | `data_file` | yes | Undecided |
| 16 | Gitignored and not rebuildable | Needs decision | `gitignored_not_rebuildable` | yes | Undecided |
| 17 | `.git/**` (Full treatment) | Included | `git_internal` | no | Include |
| 18 | Everything else | Included | `project_file` | yes | Include |

- Rebuildable folders, `.git/hooks/`, and `.git/fsmonitor--daemon/` appear as one `Dir` item with an aggregated size. The planner does not descend into them.
- Rule 5's content test reads at most the first line of a file.
- Under `WorkingTreeOnly`, every `.git` item from rules 17 and the `.git` file case gets Decision `Exclude` with reason `git_working_tree_only` (Requirement 5.5). Rules 7-9 still apply.
- When `.git` is a regular file, it is an Included item (rule 18) with warning `git_external_dir` (Requirement 5.9).
- The database-dump patterns in rule 15 are the v1 list. `*.bak` counts only when the base name ends in `.db` or `.sqlite` (for example `dev.sqlite.bak`).
- `Group` is derived from `ReasonCode` with `ReasonCode::group()`. Each reason maps to exactly one group, so the item doesn't store it.

**Secret scanner.** `scan_secrets` runs on each item with Decision `Include` or `Undecided` in the working tree, plus `.git/config` (Requirement 3.5). It skips `.git/objects/**`, `.git/index`, and files with a NUL byte in the scanned prefix, and counts skips in `totals.secret_scan_skipped` (Requirement 3.6). Patterns: known token prefixes (AWS access key IDs, GitHub tokens, Slack tokens, Stripe live keys), `password=`/`secret=`/`token=` assignments with a non-empty literal value, and URL userinfo with a password (`scheme://user:pass@host`). It returns the first line number only. Tests build matching strings at runtime by concatenating fragments, so no secret-like literal is ever committed (Requirement 3.7).

**Digest.** `plan_digest` is SHA-256 over this compact JSON, with keys in this order and items sorted by path bytes:

```json
{"v":1,"project_id":"<id>","git_treatment":"full|working_tree_only",
 "items":[{"path":"…","kind":"file|dir|symlink|special","size":0,
           "decision":"include|exclude|undecided","reason":"…","editable":true,
           "warning":"secret_like_content|git_external_dir|null"}]}
```

`plan_id`, `totals`, and `groups` are left out because `plan_id` is random and the others are derived. Acknowledgments are not in the digest; they are recorded in `ApprovedPlan` (Requirement 7.5).

**Approval** checks, in order: shown digest equals current digest (`PLAN_DIGEST_MISMATCH`); no item `Undecided` (`PLAN_INCOMPLETE`); every warning on an item with Decision `Include` is in `acks` (`PLAN_INCOMPLETE`); file and symlink entries to include ≤ `limits.max_entries` (`CAPSULE_LIMIT_EXCEEDED`). On success, `overrides` lists every item whose decision differs from its rule's default. Any later edit drops the stored approval (Requirement 7.7).

### Git ref reader (`dormbit-core/src/gitref.rs`)

```rust
pub fn read_refs<V: ReadVfs>(vfs: &V, git_dir: &RelPath) -> Result<GitRefs, DormbitError>;
pub struct GitRefs { pub head: HeadRef, pub refs: Vec<RefEntry> }      // refs sorted by name
pub struct HeadRef { pub branch: Option<String>, pub detached: bool, pub commit: Option<String> }
pub struct RefEntry { pub name: String, pub target: String }           // 40 or 64 hex
```

It reads `.git/HEAD`, loose refs under `.git/refs/`, and `.git/packed-refs` (loose refs win over packed). It never runs git and never reads config. The writer calls it only when Git treatment is `Full` (Requirement 5.8). The same reader is reused by `transactional-restore` for the HEAD check (RST-4).

### Capsule writer (`dormbit-core/src/capsule/writer.rs`)

```rust
pub trait Job {
    type Output;
    fn step(&mut self, budget: Budget) -> Result<Step<Self::Output>, DormbitError>;
    fn cancel(&mut self);
}
pub struct Budget { pub max_bytes: u64, pub max_ops: u32 }
pub enum Step<T> { Progress(JobProgress), Done(T) }

pub trait CaptureWorkspace {
    fn put_object(&mut self) -> Result<Box<dyn ObjectWrite + '_>, DormbitError>;
    fn read_object(&self, content_sha: &Sha256Hex) -> Result<Box<dyn std::io::Read + '_>, DormbitError>;
    fn capsule_out(&mut self) -> Result<&mut dyn std::io::Write, DormbitError>;
    fn commit(&mut self, final_name: &CapsuleFileName) -> Result<(), DormbitError>; // sync_all + rename_excl + dir sync
    fn abort(&mut self) -> Result<(), DormbitError>;                                 // token-checked cleanup
}
pub trait ObjectWrite: std::io::Write {
    /// Stores the finished gzip stream under `content_sha`. If that object exists, drops the new copy.
    fn finish(self: Box<Self>, content_sha: &Sha256Hex) -> Result<StoredObject, DormbitError>;
}
pub struct StoredObject { pub stored_size: u64, pub stored_sha256: Sha256Hex, pub deduplicated: bool }

pub trait Registry {
    fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError>;
    fn put(&mut self, record: CapsuleRecord) -> Result<(), DormbitError>;
}

pub struct CaptureJob<V: ReadVfs, W: CaptureWorkspace, R: Registry, C: Clock, I: IdGen> { /* … */ }

impl<V, W, R, C, I> CaptureJob<V, W, R, C, I> {
    pub fn new(src: V, plan: CapturePlan, approved: ApprovedPlan, ws: W, registry: R,
               clock: C, ids: I, ctx: CaptureContext) -> Result<Self, DormbitError>;
}
pub struct CaptureContext { pub app_version: String, pub project: ManifestProject,
                            pub library_grant: ScopeId, pub limits: CapsuleLimits }
impl<…> Job for CaptureJob<…> { type Output = CaptureResult; /* … */ }

pub struct CaptureResult {
    pub record: CapsuleRecord,
    pub not_captured: Vec<RelPath>,   // appeared after approval (Requirement 8.3)
}
```

**Capture phases** (`JobProgress.phase`):

| Phase | Work | Fails with |
|---|---|---|
| `preparing` | Recompute plan digest and compare with `approved.digest`. Re-apply sensitive rules to the Approved set. Re-list the source tree; record new paths as `appeared_after_approval`. Check every approved path still exists with the same kind. | `PLAN_DIGEST_MISMATCH`, `PATH_UNSAFE`, `FILE_CHANGED_DURING_CAPTURE` |
| `hashing` (step 1) | For each approved file in path order: stream through SHA-256 and a gzip encoder into `put_object()`, then `finish(content_sha)`. Record size, mode & 0o777, mtime, content hash, stored size, stored hash. Read symlink targets. | `FILE_CHANGED_DURING_CAPTURE` (size differs from metadata), `IO_FAILED` |
| `rechecking` (step 2) | Re-read and re-hash every approved file; re-read every symlink target. Compare size, content hash, and target with step 1. | `FILE_CHANGED_DURING_CAPTURE` naming the path |
| `assembling` (step 3) | Build the manifest. Write marker, manifest, objects (ascending content hash), trailer, and two zero blocks to `capsule_out()`. While copying each object, re-hash its stored bytes and compare with step 1. Hash the whole output as it's written. Check running size against `max_capsule_size`. | `CAPSULE_HASH_MISMATCH`, `CAPSULE_LIMIT_EXCEEDED` |
| `committing` | `ws.commit("<capsule_id>.dormbit")`. | `CAPSULE_EXISTS`, `IO_FAILED` |
| `recording` | `registry.put(record)`. | `IO_FAILED` (capsule stays in place) |

`cancel()` sets a flag checked at the start of each `step`. Before `committing`, the job calls `ws.abort()` and returns `CANCELLED`. From `committing` on, cancel is ignored and the job finishes (Requirement 17.3, 17.4). On any error before `committing`, the job calls `ws.abort()`.

The job holds only per-file state between steps, so the output doesn't depend on how the work is split into budgets (Requirement 17.6). `JobProgress` counts come from real reads and writes. `bytes_total` for `hashing` and `rechecking` is the sum of approved file sizes. For `assembling` it's the sum of stored sizes plus manifest size.

**Tar assembly.** The writer uses `tar::Header::new_ustar()` and sets every field explicitly (see the table under Data Models). It does not use `Builder::append_path` or any call that reads filesystem metadata. Gzip uses `flate2::GzBuilder::new().mtime(0)` with no filename or comment and `flate2::Compression::default()` (level 6), fixed as a constant. Object encoding is always `gzip` in v1. The verifier also accepts `identity` because the §8 schema allows it.

### Capsule verifier (`dormbit-core/src/capsule/verifier/`)
Files: `ustar.rs` (strict header parser, no `tar` crate), `checks.rs`, `report.rs`, `job.rs`.

```rust
pub trait CapsuleSource {
    fn len(&self) -> Result<u64, DormbitError>;
    fn open(&self) -> Result<Box<dyn std::io::Read + '_>, DormbitError>; // fresh read-only handle
}
pub trait RegistryRead { fn get(&self, id: &CapsuleId) -> Result<Option<CapsuleRecord>, DormbitError>; }

pub struct VerifyJob<S: CapsuleSource, R: RegistryRead> { /* … */ }
impl<S, R> VerifyJob<S, R> {
    pub fn new(source: S, registry: R, limits: CapsuleLimits) -> Self;
}
impl<S, R> Job for VerifyJob<S, R> { type Output = VerificationReport; /* … */ }

/// Convenience for tests and the CLI: runs the job to completion with an unbounded budget.
pub fn verify_all<S: CapsuleSource, R: RegistryRead>(source: S, registry: R, limits: CapsuleLimits)
    -> Result<VerificationReport, DormbitError>;
```

The verifier reads the file in one sequential pass. It keeps the manifest in memory (bounded by `max_manifest_size`) and one fixed-size I/O buffer. The marker and manifest headers are read before the manifest gives `created_at`, so their mtime is checked right after the manifest is parsed; every header must carry that one mtime. Each object's bytes feed a SHA-256 for the stored hash and, at the same time, a bounded gzip decoder whose output feeds a second SHA-256 for the content hash. A third SHA-256 covers every byte of the file for `file_sha256`. Verify phases: `reading_header`, `objects`, `trailer`, `registry`.

**Check list.** Every report has one `checks[]` entry per ID below (Requirement 16.4).

| ID | Check | Fail result | Criteria |
|---|---|---|---|
| `V01_MARKER` | First entry named `DORMBIT`; content exactly `dormbit-capsule/1\n`. `dormbit-capsule/<N>\n` with other N → Unsupported. | `CAPSULE_MALFORMED` / `CAPSULE_VERSION_UNSUPPORTED` | 13.3, 13.4, 13.5 |
| `V02_HEADER` | For every header: checksum matches; every field byte-equal to the format table (magic, version, uid, gid, uname, gname, mtime equal to manifest `created_at`, mode 0644 or 0755 as the writer would set it, linkname, prefix, dev fields, pad). Numeric fields parse as strict octal. | `CAPSULE_MALFORMED` | 13.2, 13.6 |
| `V03_ENTRY_TYPE_NAME` | Typeflag `0` only. Names only `DORMBIT`, `manifest.json`, `objects/[0-9a-f]{64}`, `END`. | `CAPSULE_MALFORMED` | 13.7, 13.8 |
| `V04_ORDER` | Marker, manifest, objects in strictly ascending name order, `END`. No name twice. | `CAPSULE_MALFORMED` | 13.9 |
| `V05_PADDING` | Data padding to 512 is all zero. After `END`, exactly two all-zero 512-byte blocks, then end of file. | `CAPSULE_MALFORMED` | 13.10 |
| `V06_LIMITS` | File length ≤ `max_capsule_size`; entries ≤ `max_entries`; manifest size ≤ `max_manifest_size`; each declared `size` ≤ `max_file_size`; each `size / max(stored_size,1)` ≤ `max_expansion_ratio`. Checked before reading the bytes each limit protects. | `CAPSULE_LIMIT_EXCEEDED` (Unsupported) | 12.4, 12.5 |
| `V07_MANIFEST` | Strict serde parse (`deny_unknown_fields`, no missing fields, exact types). `schema_version == 1` else Unsupported. | `CAPSULE_MALFORMED` / `CAPSULE_VERSION_UNSUPPORTED` | 14.1, 14.2, 14.12 |
| `V08_PATHS` | Every entry path is a Safe path. No exact or NFC-equal duplicates. No path is also a parent folder of another. | `PATH_UNSAFE` / `CAPSULE_MALFORMED` | 14.3, 14.4, 14.5 |
| `V09_OBJECT_SET` | Set of referenced content hashes equals the set of object entries; each object appears once. | `CAPSULE_MALFORMED` | 14.6 |
| `V10_STORED_HASH` | Tar size equals `stored_size`; SHA-256 of stored bytes equals `stored_sha256` for all entries sharing the object. | `CAPSULE_HASH_MISMATCH` | 14.7, 14.9 |
| `V11_CONTENT_HASH` | Decompressed length equals `size` (decoder stops at `size + 1` bytes); content SHA-256 equals entry `sha256` and the object name. | `CAPSULE_HASH_MISMATCH` | 12.6, 14.8, 14.9 |
| `V12_TOTALS` | `totals` equal counts and sums computed from entries and objects. | `CAPSULE_MALFORMED` | 14.11 |
| `V13_TRAILER` | `END` present (else `CAPSULE_TRUNCATED`); its `manifest_sha256` equals SHA-256 of the manifest bytes; `object_count` equals objects read. | `CAPSULE_TRUNCATED` / `CAPSULE_HASH_MISMATCH` | 13.11, 13.12 |
| `V14_REGISTRY` | Look up `capsule_id`. Both hashes equal → `Match` (Pass). Any differ → `Mismatch` (Fail). No record → `NotInRegistry` (Unknown). | Fail only on `Mismatch` | 15.1-15.5 |

**Status rules.**
- A check that can't continue (V01, V02, V03, V04, V05, V06, V07, V13 truncation) stops parsing. Later checks become `NotRun`.
- After a stop, every entry whose object wasn't verified goes into `failed_files` with reason `unverifiable` (Requirement 14.10). If the manifest was never parsed, `failed_files` is empty and the detail says so.
- V10 and V11 failures don't stop parsing. The verifier moves to the next object (Requirement 14.9).
- Overall status: any `Fail` → Failed. Otherwise a stop caused by V01 version, V06, or V07 version → Unsupported. Otherwise → Verified, including when V14 is Unknown (Requirement 16.1-16.3).
- If the end of file arrives anywhere before the two zero blocks after `END`, the stopping check is reported as `CAPSULE_TRUNCATED` (Requirement 13.11).

### dormbit-fs responsibilities (`crates/dormbit-fs/src/capsule_sink.rs`, `rename.rs`, `registry.rs`)

```rust
pub struct FsCapsuleSink { library: cap_std::fs::Dir, grant: ScopeId }
impl FsCapsuleSink {
    pub fn open(scopes: &ScopeRegistry, grant: ScopeId) -> Result<Self, DormbitError>; // SCOPE_DENIED unless kind CapsuleLibrary
    pub fn begin(&self, op_id: uuid::Uuid, owner_token: uuid::Uuid) -> Result<FsWorkspace, DormbitError>;
}
pub struct FsWorkspace { /* temp Dir, token, open capsule.tmp File */ }
impl dormbit_core::capsule::CaptureWorkspace for FsWorkspace { /* … */ }

/// The one `unsafe` call in the project: renameatx_np(fromfd, from, tofd, to, RENAME_EXCL).
pub fn rename_excl(from_dir: &cap_std::fs::Dir, from: &str, to_dir: &cap_std::fs::Dir, to: &str)
    -> Result<(), RenameError>; // RenameError::Exists on EEXIST

pub struct FsCapsuleRegistry { /* Application Support JSON via the foundation atomic-JSON helper */ }
impl dormbit_core::capsule::Registry for FsCapsuleRegistry { /* … */ }
impl dormbit_core::capsule::RegistryRead for FsCapsuleRegistry { /* … */ }

pub struct FsCapsuleSource { /* read-only cap-std File from a CapsuleLibrary grant or a picked file */ }
impl dormbit_core::capsule::CapsuleSource for FsCapsuleSource { /* reopens on every open() */ }
```

- `begin` creates `.dormbit-capture-<op_id>/` in the library with `create_dir` (fails if it exists), writes `owner-token` (create-new) holding `<op_id>:<owner_token>`, and creates `objects/` and `capsule.tmp` (create-new).
- `put_object` writes to `objects/tmp-<n>` and `finish` renames it to `objects/<sha256>` with `rename_excl`. If the target exists, the temp copy is removed and `deduplicated` is `true`.
- `commit` calls `File::sync_all` on `capsule.tmp` (Rust uses `F_FULLFSYNC` on macOS), then `rename_excl` into the library as `<capsule_id>.dormbit`, then `sync_all` on the library directory handle. `EEXIST` maps to `CAPSULE_EXISTS` (Requirement 10.2-10.5).
- `abort` and post-commit cleanup read `owner-token` and remove the temp folder only if the token matches. They remove only `objects/*`, `capsule.tmp`, `owner-token`, and the folder itself, never following symlinks. A mismatched or missing token leaves the folder in place and reports it.
- The registry is DORMBIT's own state, written with the foundation's atomic JSON helper (temp file, `sync_all`, replace). It is not inside any granted folder.
- `FsCapsuleSource::open` opens the file again each time, so the verifier never reuses the writer's handle (Requirement 13.1).

The in-memory equivalents (`MemWorkspace`, `MemRegistry`, `BytesSource`) live in `dormbit-core/src/capsule/mem.rs`. They are used by core tests and later by `dormbit-wasm` (judge-demo Task 38). `MemWorkspace::commit` refuses an existing name with `CAPSULE_EXISTS`, matching the native behavior.

### Tauri commands (`apps/desktop/src-tauri/src/commands/{plan,capture,verify}.rs`)
All are in the `AppManifest::commands` allowlist. Inputs are IDs and validated values, never raw paths.

| Command | Input | Output |
|---|---|---|
| `plan_create` | `{project_id, git_treatment}` | `CapturePlan` |
| `plan_set_decision` | `{plan_id, edit: PlanEdit}` | `CapturePlan` |
| `plan_approve` | `{plan_id, digest, acks: WarningId[]}` | `ApprovedPlan` |
| `capture_start` | `{plan_id, library_grant_id, on_progress: Channel<JobProgress>}` | `{job_id}`; final `CaptureResult` arrives on the Channel |
| `job_cancel` | `{job_id}` | `()` |
| `capsule_pick` | `{}` (Rust opens a native file dialog for `.dormbit`) | `{capsule_ref}` (handle ID) |
| `capsules_list` | `{project_id?}` | `CapsuleRecord[]` |
| `verify_start` | `{capsule_ref, on_progress: Channel<JobProgress>}` | `{job_id}`; final `VerificationReport` arrives on the Channel |

The job runner thread calls `step(budget)` in a loop with a fixed `Budget` constant defined in `src-tauri` and sends a `JobProgress` after each step. The budget is a runner setting, not part of the capsule format, and Property 4 shows the result doesn't depend on it.

### UI (`packages/ui/src/screens/`, `packages/client/src/`)
- `DormbitClient` gains `planCreate`, `planSetDecision`, `planApprove`, `captureStart(…, onProgress)`, `jobCancel`, `capsulePick`, `capsulesList`, `verifyStart(…, onProgress)`. `TauriClient` and `MockClient` implement them; every response is parsed with Zod.
- `PreservationPlan.tsx`: six group sections (virtualized lists), a summary rail with totals, per-item decision controls only when `editable`, a Git treatment toggle, a warning list with acknowledgment checkboxes, and an Approve button that stays disabled with a reason list while anything is unresolved (Requirement 18.1-18.4).
- `CapsuleCreation.tsx`: a `ProgressSteps` state machine over the capture phases with real counts, Cancel, and the not-captured list (Requirement 8.4, 18.5, 18.6).
- `IntegrityVerification.tsx`: check list with glyph-plus-label statuses, damaged files, registry match, the stated limitations, and the `CapsuleSeal` pressed only on Verified (Requirement 18.7-18.12).
- File names and paths render as text nodes only; no raw HTML (Requirement 18.13).

## Data Models

### CapturePlan and ApprovedPlan (§8)

```rust
pub struct CapturePlan {
    pub plan_id: PlanId,
    pub project_id: ProjectId,
    pub items: Vec<PlanItem>,              // sorted by path bytes
    pub groups: Vec<GroupSummary>,         // six entries, fixed order
    pub git_treatment: GitTreatment,       // Full | WorkingTreeOnly
    pub totals: PlanTotals,
    pub digest: Sha256Hex,
}
pub struct PlanItem {
    pub path: RelPath,
    pub kind: ItemKind,                    // File | Dir | Symlink | Special
    pub size: u64,                         // Dir: aggregated; Symlink: target length
    pub decision: Decision,                // Include | Exclude | Undecided
    pub reason: ReasonCode,                // table above; group() derives Group
    pub editable: bool,
    pub warning: Option<PlanWarning>,
}
pub struct PlanWarning { pub id: WarningId, pub code: WarningCode, pub line: Option<u32> }
// WarningId = "<code>:<path>", deterministic. WarningCode: SecretLikeContent | GitExternalDir
pub struct GroupSummary { pub group: Group, pub items: u32, pub bytes: u64 }
pub struct PlanTotals { pub include_items: u32, pub include_bytes: u64, pub exclude_items: u32,
                        pub undecided_items: u32, pub secret_scan_skipped: u32 }

pub struct ApprovedPlan {
    pub digest: Sha256Hex,
    pub approved_at: i64,                  // from Clock
    pub acks: Vec<WarningId>,
    pub overrides: Vec<DecisionOverride>,  // { path, default: Decision, chosen: Decision }
}
```

### Capsule limits

```rust
pub struct CapsuleLimits {
    pub max_file_size: u64,
    pub max_entries: u32,
    pub max_capsule_size: u64,
    pub max_manifest_size: u64,
    pub max_expansion_ratio: u32,
}
```
Defaults come from `docs/architecture/capsule-format.md` and are stored in settings. The writer and verifier read the same settings.

### Tar layout (exact entry order)

| # | Entry name | Content | Tar mode |
|---|---|---|---|
| 1 | `DORMBIT` | `dormbit-capsule/1\n` (18 bytes) | 0644 |
| 2 | `manifest.json` | Manifest, compact JSON | 0644 |
| 3..n+2 | `objects/<sha256>` | gzip stream of one content; sorted by ascending lowercase hex | 0755 if any referencing entry has an executable bit, else 0644 |
| n+3 | `END` | `{"manifest_sha256":"<64 hex>","object_count":<n>}` (compact JSON, no newline) | 0644 |
| — | end of archive | two 512-byte zero blocks, then end of file | — |

**Header fields** (every entry; the verifier requires these exact bytes):

| Field | Offset, length | Value |
|---|---|---|
| name | 0, 100 | entry name, NUL-padded |
| mode | 100, 8 | `0000644\0` or `0000755\0` |
| uid, gid | 108, 8 / 116, 8 | `0000000\0` |
| size | 124, 12 | 11 octal digits + NUL |
| mtime | 136, 12 | manifest `created_at` as Unix seconds, 11 octal digits + NUL |
| chksum | 148, 8 | 6 octal digits, NUL, space |
| typeflag | 156, 1 | `0` |
| linkname | 157, 100 | all NUL |
| magic, version | 257, 6 / 263, 2 | `ustar\0`, `00` |
| uname, gname | 265, 32 / 297, 32 | all NUL |
| devmajor, devminor | 329, 8 / 337, 8 | all NUL |
| prefix | 345, 155 | all NUL |
| pad | 500, 12 | all NUL |

The golden capsule test pins these bytes. If the `tar` crate's output differs from this table, the table and the verifier's expected bytes are corrected together in one reviewed change. The verifier is never relaxed to accept more than one form.

### Manifest (`manifest.json`, schema_version 1)

Fields appear in this order. All are required. Unknown fields are rejected.

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | integer | `1` |
| `capsule_id` | string | lowercase hyphenated UUID from `IdGen` |
| `created_at` | string | RFC 3339 UTC, seconds precision, `Z` suffix, from `Clock`; also the tar mtime |
| `app_version` | string | DORMBIT version that wrote it |
| `project` | object | `{name, name_source}` from discovery. No absolute path. |
| `plan_digest` | string | 64 hex, the Approved plan digest |
| `git` | object or null | null if the project has no `.git`; otherwise `{treatment, head, refs}` |
| `git.treatment` | string | `full` or `working_tree_only` |
| `git.head` | object or null | `{branch: string or null, detached: bool, commit: string or null}`; null under `working_tree_only` |
| `git.refs` | array | `{name, target}` sorted by name; empty under `working_tree_only` |
| `entries` | array | file and symlink entries, sorted by path bytes |
| `excluded` | array | `{path, group, reason}` for every Exclude item and every `appeared_after_approval` path, sorted by path |
| `warnings` | array | `{id, code, path, line, acknowledged}` |
| `limitations` | array of string | fixed v1 list, below |
| `agent_refs` | array | empty in P0; filled by P1 adapters (`intelligence-and-brief`) |
| `totals` | object | `{files, symlinks, objects, bytes, stored_bytes, excluded}` |

**File entry:** `{"type":"file","path","size","mode","mtime","sha256","object":{"encoding","stored_size","stored_sha256"}}`
- `size`: u64 content bytes. `mode`: integer, permission bits `& 0o777` (setuid, setgid, sticky never recorded). `mtime`: integer Unix seconds. `sha256`: content hash, equal to the object name. `encoding`: `gzip` (writer) or `identity` (accepted by the verifier).

**Symlink entry:** `{"type":"symlink","path","target","restored":false}`
- `target`: the link text as read, not resolved. No object, no tar entry.

**Totals:** `files` and `symlinks` count entries; `objects` counts distinct objects; `bytes` sums file `size`; `stored_bytes` sums `stored_size` once per object; `excluded` is the length of `excluded`.

**Limitations (v1, fixed text):** "Not encrypted." "Sensitive files are excluded and can't be included." "Symlinks are recorded, not restored." "Git hooks, lock files, and fsmonitor state are not captured." "Empty folders are not preserved."

**Canonical form:** `serde_json::to_vec` (compact) over structs with fields in this order, no maps with unordered keys, no floats. `manifest_sha256` is SHA-256 of these exact bytes.

### CapsuleRecord (registry)

```rust
pub struct CapsuleRecord {
    pub capsule_id: CapsuleId,
    pub project_id: ProjectId,
    pub library_grant: ScopeId,
    pub file_name: CapsuleFileName,    // "<capsule_id>.dormbit"
    pub file_sha256: Sha256Hex,
    pub manifest_sha256: Sha256Hex,
    pub plan_digest: Sha256Hex,
    pub size: u64,
    pub created_at: i64,
    pub last_verified_at: Option<i64>,
}
```

### VerificationReport (§8, plus `limitations`)

```rust
pub struct VerificationReport {
    pub capsule_id: Option<CapsuleId>,     // None if the manifest wasn't parsed
    pub file_sha256: Option<Sha256Hex>,
    pub manifest_sha256: Option<Sha256Hex>,
    pub checks: Vec<CheckResult>,          // one per V01..V14, in order
    pub failed_files: Vec<FailedFile>,
    pub registry_match: RegistryMatch,     // Match | Mismatch | NotInRegistry | NotChecked
    pub status: ReportStatus,              // Verified | Failed | Unsupported
    pub error_code: Option<ErrorCode>,     // first failing code, if any
    pub limitations: Vec<String>,
}
pub struct CheckResult { pub id: CheckId, pub status: CheckStatus, pub detail: String } // Pass | Fail | Unknown | NotRun
pub struct FailedFile { pub path: RelPath, pub reason: FailReason } // StoredHashMismatch | ContentHashMismatch | SizeMismatch | Unverifiable
```
`limitations` always contains "SHA-256 shows integrity, not authorship." and "The operating system's cache may serve the bytes that were read." (Requirement 16.5). `NotChecked` is used only when parsing stopped before the manifest gave a `capsule_id`.

### JobProgress

```rust
pub struct JobProgress {
    pub job_id: JobId,
    pub kind: JobKind,                     // Capture | Verify
    pub phase: JobPhase,
    pub files_done: u32, pub files_total: u32,
    pub bytes_done: u64, pub bytes_total: u64,
}
```

## Correctness Properties

### Property 1: Captured content equals the source (PROP-2, writer side)
*For any* generated source tree (files with random bytes, sizes including 0, random permission bits, duplicate contents, symlinks) and any Approved plan over it, running `CaptureJob` to completion and then decompressing each file entry's object gives bytes identical to the source file, each entry's `mode` equals the source mode `& 0o777`, each symlink entry's `target` equals the link text, and `verify_all` on the output reports Verified with `registry_match = Match`.
**Validates: Requirements 6.3, 8.2, 9.1, 9.2, 11.1, 11.4, 14.7, 14.8, 16.1**

### Property 2: Any single-byte flip or truncation fails verification (PROP-3)
*For any* valid capsule from Property 1's generator, any byte offset, and any non-zero XOR mask, the Verifier's status on the modified bytes is not Verified. *For any* length shorter than the file, the status on the truncated prefix is not Verified. When the flipped byte lies inside an object's stored bytes, `failed_files` contains every entry that references that object.
**Validates: Requirements 13.5, 13.6, 13.10, 13.11, 13.12, 14.7, 14.8, 14.9, 16.2**

### Property 3: The capsule contains exactly the Approved set (PROP-7)
*For any* generated source tree, plan, and decision edits, after approval and capture the set of manifest entry paths equals the set of plan paths with Decision `Include` of kind file or symlink. The set of object names equals the set of distinct content hashes of those files. Every Exclude item and every file added to the tree after approval appears in `excluded`, and no added file appears in `entries`.
**Validates: Requirements 8.2, 8.3, 8.6, 9.2, 14.6**

### Property 4: Capsule bytes are deterministic (PROP-10)
*For any* source tree, Approved plan, fixed `Clock`, and fixed `IdGen`, two captures produce byte-identical capsules, regardless of directory listing order from the `Vfs` and of the sequence of `step` budgets used. The same inputs built for `wasm32-unknown-unknown` give the same bytes (checked in judge-demo Task 38 against this spec's golden bytes).
**Validates: Requirements 11.2, 11.3, 11.5, 11.6, 11.7, 11.8, 11.9, 17.6**

### Property 5: Capsule types round-trip through Zod (PROP-11)
*For any* valid Rust value of `CapturePlan`, `ApprovedPlan`, `Manifest`, `JobProgress`, `VerificationReport`, and `CapsuleRecord`, serializing to JSON, parsing with the matching Zod schema, and serializing again gives the same JSON value.
**Validates: Requirements 19.1, 19.2, 19.3**

### Property 6: The plan digest tracks every decision
*For any* plan and any edit that changes one item's Decision or the Git treatment, the digest after the edit differs from the digest before. Rebuilding the same plan from the same inputs gives the same digest. Approval with any digest other than the current one fails with `PLAN_DIGEST_MISMATCH`.
**Validates: Requirements 5.6, 7.1, 7.2, 7.3, 7.4**

### Property 7: Sensitive files never enter a capsule
*For any* generated source tree containing files that match the sensitive rules (at any depth, including gitignored and tracked ones), and *for any* sequence of `plan_set_decision` edits, no sensitive path appears in the manifest `entries`, and every attempt to include one fails with `PLAN_ITEM_LOCKED`.
**Validates: Requirements 2.1, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9**

### Property 8: Verification work is bounded on hostile input
*For any* byte string (random, or a valid capsule with mutated sizes, declared sizes, or gzip streams), the Verifier finishes without panicking, decompresses at most `size + 1` bytes per object and at most `max_capsule_size × max_expansion_ratio` bytes in total, and never holds more than `max_manifest_size` plus its fixed buffer.
**Validates: Requirements 12.4, 12.5, 12.6, 12.7**

## Error Handling

All errors are `DormbitError {code, message, retryable}`. Messages never include file contents or secret matches, and paths are shown relative to the project root.

| Code | Raised by | Retryable | What the UI shows |
|---|---|---|---|
| `SCOPE_DENIED` | Writer setup, `capsule_pick`, `verify_start` | no | "This folder isn't granted for capsules. Choose a capsule library folder." |
| `PATH_UNSAFE` | Writer `preparing` (sensitive path in Approved set), Verifier V08 | no | The unsafe path, as text |
| `PLAN_DIGEST_MISMATCH` | `plan_approve`, writer `preparing` | no | "The plan changed since you reviewed it. Review and approve again." |
| `PLAN_INCOMPLETE` (proposed) | `plan_approve` | no | The list of undecided items and unacknowledged warnings |
| `PLAN_ITEM_LOCKED` (proposed) | `plan_set_decision` | no | "This item can't be changed," with its reason |
| `FILE_CHANGED_DURING_CAPTURE` | Writer `preparing`, `hashing`, `rechecking` | yes | The changed path and "Nothing was written. Try again when the file stops changing." |
| `CAPSULE_HASH_MISMATCH` | Writer `assembling`; Verifier V10, V11, V13 | no | Writer: "Temporary data was damaged; nothing was written." Verifier: damaged files |
| `CAPSULE_LIMIT_EXCEEDED` (proposed) | `plan_approve`, writer `assembling`, Verifier V06 | no | The limit name, its value, and the actual value |
| `CAPSULE_EXISTS` (proposed) | Writer `committing` | yes (a retry uses a new capsule ID) | "A capsule with this name already exists and was left unchanged." |
| `CAPSULE_MALFORMED` | Verifier V01-V05, V07-V09, V12 | no | Failed, with the check ID and detail |
| `CAPSULE_TRUNCATED` | Verifier V13 or any early end of file | no | Failed: "The file is incomplete." |
| `CAPSULE_VERSION_UNSUPPORTED` | Verifier V01, V07 | no | Unsupported: "Made by a newer or unknown format version." |
| `IO_FAILED` (proposed) | `dormbit-fs` sink, registry, source | yes | The operation that failed; for a registry failure, "The capsule was written but not recorded." |
| `CANCELLED` | Any job | yes | "Cancelled. Nothing was written." (capture before commit) or "Verification cancelled." |

Verifier results that describe the capsule (Failed, Unsupported) are returned as a `VerificationReport`, not as an `Err`. `Err` from a verify job means the verifier couldn't run (for example `IO_FAILED` opening the file, `SCOPE_DENIED`, `CANCELLED`).

## Testing Strategy

- **Unit tests** (`dormbit-core`): every classification rule and its precedence; secret scanner hits and skips with runtime-built strings; digest canonical form; approval refusals; ref reader on loose, packed, detached, and unborn HEAD; manifest canonical bytes; each verifier check in isolation on hand-built byte strings.
- **Property tests** (proptest, in `crates/dormbit-core/tests/prop_*.rs`): Properties 1, 2, 3, 7, and 8 are security properties and run 10,000 cases locally and 512 in CI. Properties 4, 5, and 6 use proptest's default case count. Counterexamples are recorded in `docs/kiro/usage-log.md`.
- **PROP-11 harness**: a Rust proptest writes seeded JSON samples of each type to a temp folder; a Vitest test parses each with Zod and compares the re-serialized value. fast-check generates TS-side values for the reverse direction. This uses the parity harness from foundation Task 11.
- **Golden tests** (insta): the Dockline plan as JSON (`.env` in Excluded: sensitive, `data/dev.sqlite` in Needs decision, `dist/` and `node_modules/` in Excluded: rebuildable); the Dockline manifest; the Dockline capsule's SHA-256 and length with the fixed test `Clock` and `IdGen`; the `tar -tvf` listing of that capsule.
- **Malicious fixtures** (`fixtures/malicious/`): each is generated by `crates/dormbit-core/tests/support/malicious.rs` as raw bytes (not through the writer), with correct hashes and trailer everywhere except the defect under test, so the intended check is the one that fires. Each has an `.expected.json` with status and code.

  | Fixture | Defect | Expected |
  |---|---|---|
  | `traversal` | entry path `../outside.txt` | Failed, `PATH_UNSAFE` |
  | `absolute-path` | entry path `/etc/hosts` | Failed, `PATH_UNSAFE` |
  | `symlink-entry` | an object entry with typeflag `2` | Failed, `CAPSULE_MALFORMED` (V03) |
  | `duplicate-entry` | same `objects/<sha>` twice; variant: same manifest path twice | Failed, `CAPSULE_MALFORMED` |
  | `truncation` | file cut in the middle of an object | Failed, `CAPSULE_TRUNCATED` |
  | `missing-trailer` | `END` removed, zero blocks kept | Failed, `CAPSULE_TRUNCATED` |
  | `archive-bomb-overflow` | object inflates far past its declared `size` | Failed, `CAPSULE_HASH_MISMATCH`; decoder stops at `size + 1` |
  | `archive-bomb-ratio` | declared `size / stored_size` over the ratio limit | Unsupported, `CAPSULE_LIMIT_EXCEEDED`; nothing decompressed |
  | `bad-marker` | marker content `dormbit-capsuel/1\n`; variant: first entry not `DORMBIT` | Failed, `CAPSULE_MALFORMED` |
  | `unknown-version` | marker `dormbit-capsule/2\n`; variant: `schema_version: 2` | Unsupported, `CAPSULE_VERSION_UNSUPPORTED` |
  | `manifest-object-mismatch` | referenced object missing; variant: unreferenced extra object | Failed, `CAPSULE_MALFORMED` |
  | `corrupt-object` | one byte flipped in one object | Failed, `CAPSULE_HASH_MISMATCH`, `failed_files` names the file |

- **Integration tests** (`dormbit-fs`, temp folders): no-clobber commit refuses an existing `<capsule_id>.dormbit` and leaves it unchanged; temp folder removed after success, failure, and cancel; a planted folder with a wrong owner token is left in place; registry record written after rename; registry write failure leaves the capsule and reports `IO_FAILED`.
- **Command tests** (`src-tauri`): each new command with valid input, unknown IDs, and random inputs (the IPC random-input test from §7) returns a typed error and changes no state.
- **UI tests** (Vitest, Testing Library, `MockClient`): Approve disabled until decisions and acks are complete; locked items have no decision control; capture steps advance with real counts from mock progress; Cancel calls `job_cancel`; seal pressed only on Verified; damaged files listed on Failed; statuses have glyph plus label.
- **CLI smoke**: `dormbit-cli plan`, `capture`, and `verify` on the extracted Dockline fixture; `tar -tvf` lists the capsule.
- **Builds**: `cargo build -p dormbit-core --target wasm32-unknown-unknown` must pass at every checkpoint.

## Security Considerations
- **Untrusted capsules.** The verifier treats every byte as hostile. It never uses tar entry names as paths, accepts only regular-file entries with four name forms, enforces limits before reading the data they protect, and bounds decompression by the declared size (§7 archive bombs, malformed tar, path traversal).
- **Independent verification.** The verifier has its own ustar parser and reopens the file. A writer bug that produces a bad header or hash is caught rather than repeated.
- **Tampering.** Hashes and the registry match detect changes. SHA-256 shows integrity, not authorship, and anyone who can rewrite a capsule can also write a self-consistent one. The registry match only helps for capsules this install wrote. The report says so.
- **Secret leakage.** Sensitive files are excluded with no override, rules are re-applied at capture, the scanner never stores matched text, and logs record paths and codes only.
- **Code execution.** Git hooks are never captured. Nothing captured is executed. Git is read only by the core ref reader; no git CLI.
- **Source safety.** The source `Vfs` has no write methods. Capture reads can update access times, which is a stated limit.
- **Change detection limit.** The three-step capture catches a file that differs between step 1 and step 2. It can't catch a file that changes and changes back between the two reads.
- **Deletion.** The only things the capture path deletes are files inside its own temp folder, after an owner-token check.
- **Unsafe code.** `rename_excl` is the project's one `unsafe` call and gets the dedicated review the plan requires.

## Open questions
1. This spec places the `renameatx_np` wrapper in `dormbit-fs` during Task 20, because the capsule writer is its first user. `transactional-restore` (Task 24) must reuse it rather than add a second `unsafe` call.
2. The core ref reader is built here (Task 20) for the manifest `git` block. `transactional-restore` reuses it for RST-4.
3. This spec uses `ReadVfs` as `scope-and-discovery` defines it and needs one more read-only method, `read_link`, to record symlink targets (CAP-6). That method has to be added to `ReadVfs`, `MemVfs`, and the `dormbit-fs` implementation. If `scope-and-discovery` changes its trait, this spec adapts to it.
4. A crash can leave a `.dormbit-capture-*` folder in the capsule library. This spec never deletes one automatically on the next launch. Offering cleanup, like restore recovery (RST-5), is not in the plan for capture.
5. The plan says tar mode is "0644 or 0755". With content addressing, one object can back both an executable and a non-executable file. This spec uses 0755 if any referencing entry is executable. Restore uses the manifest `mode`, never the tar mode.
6. The credential-file list, database-dump patterns, and secret-scanner patterns are v1 choices made here. The plan names the categories but not the full lists.
7. Default values for `CapsuleLimits` and the expansion ratio are not in the plan. They must be set in `docs/architecture/capsule-format.md` before Task 20.
8. "Empty folders are not preserved" follows from §8 having only file and symlink entries. It's listed as a capsule limitation.
