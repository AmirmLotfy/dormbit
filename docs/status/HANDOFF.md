# Handoff

Read this first in a fresh session. It is loaded into every Kiro session through `.kiro/steering/project-status.md`.

## What DORMBIT is

Local-first macOS app (Tauri 2 + Rust + React) that shelves dormant developer projects: discover, understand, preserve into a verified capsule, restore transactionally, and write an evidence-graded resurrection brief. A browser demo runs the same Rust core compiled to WebAssembly on a synthetic project. Built for Kiroween 2026, Frankenstein category. Deadline Fri Dec 4, 2026, 2:00 PM PT; target submit Wed Dec 2, 12:00 PM PT.

## Where things are

- Plan: `docs/MASTER_PLAN.md` (source of truth; ADR-011 replaced its §9 visual tokens).
- Specs: `.kiro/specs/{foundation,scope-and-discovery,capsule-and-verification,transactional-restore,intelligence-and-brief,native-experience,judge-demo,release-and-submission}/`.
- Status ledger and deviations: `docs/status/STATUS.md`. Evidence records: `docs/evidence/runs/`.
- Repo: https://github.com/AmirmLotfy/dormbit (public, MIT). Git identity is the GitHub noreply address.

## Current position (Nov 8, 2026)

- Built, reviewed, and pushed with clean-tree evidence:
  - The capsule work (P04-T01 to T03, Verified).
  - The restore core and native layer (P05, gaps in D-40).
  - The evidence engine and brief (P06, gaps in D-44).
  - Discovery and the WebAssembly demo engine.
  - The judge demo (P09-T02/T03 Tested, D-48).
- The site is live at https://amirmlotfy.github.io/dormbit/ (demo at `/dormbit/demo/`).
  - `pages.yml` deploys only after a green `ci` run on `main`, ships the dist that ci tested, and re-runs Playwright against the live URL (D-50).
  - P09-T04 is In progress; its open subtasks are listed in STATUS.
- Repo settings done: private vulnerability reporting is on, and the homepage URL is set.
- The demo video's voice script, shot list, and music prompt are in `media/script/` (P15-T01, D-49). They wait for the user's approval; the user records the voice and generates the music.

## Next: the desktop app (critical path for the video)

Decisions already made. Record them as D-51 and D-52 when the code lands.

1. **Folder pickers:** use the `rfd` crate, called from Rust. Pin it with `=` and pass cargo-deny and check-deps.
   - `tauri-plugin-dialog` is banned by security rule 3 and `tests/allowlist.rs`.
   - This supersedes `blocking_pick_folder` in the scope-and-discovery design §7.
2. **IPC results:** commands return the core types, in the same JSON as the demo engine.
   - They are parsed by `packages/schema/src/core.ts`, and they replace the placeholder types in `model/ipc.rs` (D-24, D-29, D-48).
   - Update `DormbitClient`, `MockClient`, and the contract to match.
3. **Order of work:**
   1. `dormbit-fs` scope layer: `ScopeRegistry`, `PickedFolder`, dev/ino, `~` display. Then `StateStore` and `InstanceLock`. These are security modules, so the reviewer must approve them.
   2. `dormbit-git` `inspect` with git2, plus `redact_remote_url`. This fills `GitSnapshot`.
   3. `src-tauri`: app state, the job runner (core `Job::step` → `JobEvent` over a Channel), and strict ID parsing. Commands go in journey order. Each command group's handler, `build.rs`, `capabilities/main.json`, and `EXPECTED_PERMISSIONS` change in one commit.
   4. `TauriClient` and `runClientContract`.
   5. UI. Move the shared demo components (`spans.tsx`, VerifyView, RestoreView, Phases, outcomes) into `packages/ui`, then build onboarding (Read and CapsuleLibrary grants), Shelf, Evidence, Plan, Capture, Verify, Restore, and Brief.
   6. Ask the user for Screen Recording, then record the native takes on the Dockline fixture.

- Still open from P02: tasks 5.9 (permissions, user) and 5.10 (fire each hook in a fresh session).
- User actions still open:
  - Approve the video script; send the voice files and the music track with their licences.
  - Paste `docs/kiro/mcp.json` and `docs/kiro/kiroignore.txt`, and apply the permissions files.
  - Run Analyze Requirements and Correctness on the two security specs.

## Environment facts that matter

- Agent shells need `. "$HOME/.cargo/env"` before cargo; `.scratch/run.sh <name> <cmd>` does this, sets `CI=true`, and logs to `.scratch/logs/`.
- Inline terminal output is unreliable in the IDE; write command output to a file and read it back. A file just created may need a second read, or a `cat` in the shell, before the read tool sees it.
- Playwright browsers live in `.scratch/ms-playwright` (`.scratch/run.sh` sets `PLAYWRIGHT_BROWSERS_PATH`): macOS purged `~/Library/Caches/ms-playwright` when the disk filled. Only Chromium is installed there; CI runs all four browsers.
- Background processes need the mise shims on `PATH`, or corepack prompts and hangs.
- Kiro always asks before writing `.kiro/agents` and `.kiro/hooks`; it always denies agent writes to `.kiro/settings/` and `.kiroignore`.
- Disk space is tight on this Mac; free space has dipped below 500 MB, and a full disk shows up as `ENOSPC` test failures. `cargo clean --release` and removing `target/debug/incremental` free about 2.5 GB. Exclude `dockline-gen` from local `cargo test` (its determinism test is slow on this disk); CI runs it.
- gitleaks runs as a pre-commit hook (`core.hooksPath=.githooks`). Reviewed false positives go in `.gitleaksignore`.

## Rules that must not be weakened

1. Never delete or overwrite user data; the only deletion is DORMBIT's own staging folders after an owner-token check.
2. Never run project code or the git CLI.
3. Never read real agent histories or credential stores (`~/.claude`, `~/.codex`, `~/.ssh`, `~/.aws`, `~/.config/gh`).
4. Report the four outcomes separately; never claim a session resumed or a project builds.
5. A result counts only if recorded by `scripts/evidence/run.sh` or CI.
6. Stop and ask before: installs outside the repo, publishing a release, Screen Recording, enabling the Chrome extension, making the video public, the final Devpost submit.
