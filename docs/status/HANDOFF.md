# Handoff

Read this first in a fresh session. It is loaded into every Kiro session through `.kiro/steering/project-status.md`.

## What DORMBIT is

Local-first macOS app (Tauri 2 + Rust + React) that shelves dormant developer projects: discover, understand, preserve into a verified capsule, restore transactionally, and write an evidence-graded resurrection brief. A browser demo runs the same Rust core compiled to WebAssembly on a synthetic project. Built for Kiroween 2026, Frankenstein category. Deadline Fri Dec 4, 2026, 2:00 PM PT; target submit Wed Dec 2, 12:00 PM PT.

## Where things are

- Plan: `docs/MASTER_PLAN.md` (source of truth; ADR-011 replaced its §9 visual tokens).
- Specs: `.kiro/specs/{foundation,scope-and-discovery,capsule-and-verification,transactional-restore,intelligence-and-brief,native-experience,judge-demo,release-and-submission}/`.
- Status ledger and deviations: `docs/status/STATUS.md`. Evidence records: `docs/evidence/runs/`.
- Repo: https://github.com/AmirmLotfy/dormbit (public, MIT). Git identity is the GitHub noreply address.

## Current position (Nov 7, 2026)

- Phase 02 (`foundation`) is built and CI is green on `main` ([run](https://github.com/AmirmLotfy/dormbit/actions/runs/42354901364)). Open: fire each hook
  once in a fresh Kiro session and record it (tasks 5.9, 5.10), then close the gate.
- The landing page with the interactive 3D capsule is built in `apps/demo` (ADR-011).
- Next: Phase 03 (`scope-and-discovery`) with Phase 08 (design system) in parallel.
- User actions still open: paste `docs/kiro/mcp.json` and `docs/kiro/kiroignore.txt`, apply the
  permissions files, run Analyze Requirements and Correctness on the two security specs, and
  enable GitHub private vulnerability reporting before release.

## Environment facts that matter

- Agent shells need `. "$HOME/.cargo/env"` before cargo; `.scratch/run.sh <name> <cmd>` does this, sets `CI=true`, and logs to `.scratch/logs/`.
- Inline terminal output is unreliable in the IDE; write command output to a file and read it back.
- Background processes need the mise shims on `PATH`, or corepack prompts and hangs.
- Kiro always asks before writing `.kiro/agents` and `.kiro/hooks`; it always denies agent writes to `.kiro/settings/` and `.kiroignore`.
- Disk space is tight on this Mac; free space has dipped below 500 MB when other apps install runtimes.
- gitleaks runs as a pre-commit hook (`core.hooksPath=.githooks`). Reviewed false positives go in `.gitleaksignore`.

## Rules that must not be weakened

1. Never delete or overwrite user data; the only deletion is DORMBIT's own staging folders after an owner-token check.
2. Never run project code or the git CLI.
3. Never read real agent histories or credential stores (`~/.claude`, `~/.codex`, `~/.ssh`, `~/.aws`, `~/.config/gh`).
4. Report the four outcomes separately; never claim a session resumed or a project builds.
5. A result counts only if recorded by `scripts/evidence/run.sh` or CI.
6. Stop and ask before: installs outside the repo, publishing a release, Screen Recording, enabling the Chrome extension, making the video public, the final Devpost submit.
