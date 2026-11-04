# ADR-005: The UI talks to Rust through allowlisted, ID-based commands

- **Status:** Accepted, Nov 4, 2026
- **Task:** P01-T02
- **Evidence:** [docs/evidence/P00-T03-spikes.md](../evidence/P00-T03-spikes.md) (spike a2)
- **Related:** SCP-1, NAT-1, NAT-2; [architecture overview §6](../architecture/overview.md#6-ipc-model); [threat model](../security/threat-model.md) T-09, T-10

## Context
The WebView renders untrusted text: READMEs, commit messages, file names. If a rendering bug ever let that text run script, the script could call any command the UI can call. Tauri's docs say capabilities don't protect against incorrect scope checks inside command implementations ([research.md §3](../product/research.md#3-platform-facts-that-shaped-the-design)). So the Rust side must be safe even when the caller is hostile.

If commands accepted paths, a hostile caller could ask Rust to read `~/.ssh` or restore over `~/Documents`, and every command would need its own path checks.

## Decision
- **No paths from the UI.** Every folder or file enters through a picker opened by Rust (`scope_pick`, `capsule_pick`, `restore_target_pick`, and the save dialog in `brief_export`). Rust returns opaque IDs. All later commands take IDs.
- **Two narrow string inputs.** `plan_set_decision` accepts a plan item path only if it exactly matches an item already in that plan. `restore_start` accepts a new folder name only as a single path component that passes the capsule path rules.
- **Allowlist.** `tauri_build::AppManifest::commands` lists the 28 commands in plan §6, and the capability grants only those plus `core:default`. No `fs`, `shell`, or dialog permissions go to the WebView; the dialog plugin is called from Rust. The spike built and ran this pattern with `app_info` and `pick_folder`.
- **Strict types both ways.** Rust deserializes with strict serde types. The client validates every response with Zod. Types are generated once with ts-rs (PROP-11).
- **Typed errors.** `DormbitError { code, message, retryable }` with stable codes.
- **Display paths only.** Responses carry paths for display, with the home folder as `~`. They are never accepted back as input.

## Alternatives considered
- **Path-based commands with Rust-side validation.** Rejected. Every command becomes a place to get a check wrong. ID lookup fails closed: an unknown ID is `SCOPE_DENIED`.
- **Tauri `fs` plugin with a scope.** Rejected. It gives the WebView direct filesystem access, and capabilities aren't the security boundary.
- **A local HTTP server for the UI.** Rejected. More attack surface (other local processes and web pages can reach it) and no benefit over Tauri IPC.

## Consequences
- More commands, because each picker is its own command.
- The UI can't offer a "type a path" field. That is intended.
- `dormbit-cli` stays path-based. It is a dev and test harness where the developer is the trust root, and it doesn't ship in the DMG.
- Task 10 adds a test that fails if a plugin permission or an unlisted command appears. Task 44 adds random-input IPC tests.
- The demo uses the same `DormbitClient` interface through `WasmClient`, so the UI code can't tell the difference.
