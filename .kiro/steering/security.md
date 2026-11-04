---
inclusion: always
---

# Security hard rules

These are not preferences. Breaking one is a bug even if tests pass.

1. **No `std::fs` in core.** `dormbit-core` does no I/O; filesystem access lives in `dormbit-fs` behind cap-std handles for granted folders only. Discovery never follows symlinks.
2. **No git CLI.** Never spawn `git`; read repositories with `git2` (vendored, no network features).
3. **No fs or shell plugins.** The Tauri app adds no `tauri-plugin-fs`, `-shell`, `-dialog`, `-http`, `-opener`, or `-process`, and no `core:path:*` permission. Folder pickers are opened by Rust.
4. **No raw HTML.** No `dangerouslySetInnerHTML`; Markdown renders without raw HTML. ESLint enforces it and inline disable comments are off.
5. **No `unwrap` in core.** `unwrap`, `expect`, and `panic!` are denied outside tests in `dormbit-core`; return a `DormbitError` with a stable code.
6. **Never read real agent stores.** Tests and demos use `fixtures/dockline/agent-stores/`. Real `~/.claude`, `~/.codex`, or Kiro data are never opened by the agent, and the app reads them only with consent.
7. **Security modules have one writer.** Changes to path handling, scopes, capsules, verification, restore, or the rename primitive (`crates/dormbit-core/src/{path,scope,capsule,verify,restore}*`, `crates/dormbit-fs/**`) are delegated to the `rust-security` agent and reviewed by `reviewer`.
8. Paths crossing IPC are grant IDs or plan-relative paths, never absolute. Messages replace the home folder with `~`.
9. Secrets never go in code, logs, fixtures, or commits. The pre-commit hook runs gitleaks; fixtures use `fake` values and `.invalid` emails.
10. The only `unsafe` in the project is `renameatx_np(RENAME_EXCL)` in `dormbit-fs`.
