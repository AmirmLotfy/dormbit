# ADR-003: The core does all I/O through a `Vfs` trait with injected `Clock` and `IdGen`

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Related:** [ADR-001](ADR-001-wasm-core.md), [architecture overview §4](../architecture/overview.md#4-the-vfs--clock--idgen-boundary)

## Context

`dormbit-core` holds the walker, capture plan, manifest, capsule writer, verifier, restore planner, evidence rules, and brief. It must:

1. compile for `wasm32-unknown-unknown`, where there is no filesystem, clock, or OS RNG (ADR-001);
2. produce byte-identical capsules natively and in WebAssembly (PROP-10), so nothing in it may depend on wall-clock time or randomness it doesn't receive;
3. be tested against injected failures at every restore step (PROP-5);
4. give source folders no write API at the type level (SCP-2).

Calling `std::fs` from core would break 1 and 3, and would make 4 a matter of discipline instead of types.

## Decision

- Core uses only two filesystem traits: `VfsRead` (list, lstat-style metadata, open for read, read link) and `VfsWrite: VfsRead` (create directory, create-new file, exclusive rename, remove own staging). A source folder is handed to core as `VfsRead` only.
- Time comes from an injected `Clock`. IDs come from an injected `IdGen`. Hosts supply them: the system clock and OS RNG natively; a fixed demo clock and `crypto.randomUUID()` in the browser.
- Paths inside core are a validated relative path type (`RelPath`) that can't hold `..`, absolute paths, or NUL.
- Implementations: `CapVfs` over `cap-std` in `dormbit-fs`; `MemVfs` in `dormbit-wasm`; a fault-injecting `Vfs` in tests.
- Core has `#![forbid(unsafe_code)]`, no `std::fs`, and no `unwrap` (steering rule `security.md`).
- A small ref reader in core (`.git/HEAD`, `refs/`, `packed-refs`) lets the post-restore HEAD and refs check run in both builds.

## Alternatives considered

- **`std::fs` in core with `cfg` stubs for wasm.** Rejected. Two code paths, no fault injection, and no type-level read-only guarantee.
- **Target `wasm32-wasip1` with a browser WASI shim.** Rejected. Adds a shim layer and a second runtime model for the demo, and still ties core to ambient filesystem calls.
- **`cap-std` directly in core.** Rejected. It wraps OS file APIs, which `wasm32-unknown-unknown` doesn't have.
- **A TypeScript port for the demo.** This was ADR-001's fallback. Rejected while the WebAssembly build works: two implementations, two sources of truth.

## Consequences

- One extra abstraction layer. Every filesystem behavior the safety properties rely on (no symlink following, create-new, exclusive rename, case sensitivity) must be modeled in `MemVfs` and the fault-injecting `Vfs` the same way `CapVfs` behaves, or tests prove the wrong thing.
- PROP-5 becomes practical: the fault-injecting `Vfs` fails each operation in turn and the test checks no final folder exists.
- Any new core dependency must still pass `cargo build --target wasm32-unknown-unknown` (ADR-001).
- The `unsafe` `renameatx_np` call lives in `dormbit-fs` behind `VfsWrite::rename_exclusive`, outside core ([ADR-006](ADR-006-staging-exclusive-rename.md)).
