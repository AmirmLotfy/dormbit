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

- Done and pushed, with clean-tree evidence and reviewer approval: the capsule (P04 Verified), the restore core and native layer (P05, gaps in D-40), the evidence engine and brief (P06, gaps in D-44), discovery, and the WebAssembly demo engine.
- The judge demo (`apps/demo`, `/dormbit/demo/`) runs all 12 steps on the real core. Record `20261108T004329Z-pnpm-filter.json` has 28 Playwright tests passing in Chromium, WebKit, Firefox, and iPhone 13 (P09-T02/T03 Tested, D-48). CI is green on `fa4d8b7` ([run](https://github.com/AmirmLotfy/dormbit/actions/runs/42431485143)); the wasm job builds the engine and shares it as an artifact.
- Next: a GitHub Pages workflow for `apps/demo` (P09-T04). Then the desktop Tauri commands over the native layers, the desktop UI reusing the demo screens, and native Git via `dormbit-git`. Then the phase gate notes, the release, the video, and Devpost.
- Still open from P02: fire each hook once in a fresh Kiro session and record it (tasks 5.9, 5.10).
- User actions still open: paste `docs/kiro/mcp.json` and `docs/kiro/kiroignore.txt`, apply the
  permissions files, run Analyze Requirements and Correctness on the two security specs, and
  enable GitHub private vulnerability reporting before release.

## Environment facts that matter

- Agent shells need `. "$HOME/.cargo/env"` before cargo; `.scratch/run.sh <name> <cmd>` does this, sets `CI=true`, and logs to `.scratch/logs/`.
- Inline terminal output is unreliable in the IDE; write command output to a file and read it back.
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
