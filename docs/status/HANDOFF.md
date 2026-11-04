# Handoff

Read this first in a fresh session. It is loaded into every Kiro session through the `project-status.md` steering file once that exists (Task 13).

## What DORMBIT is
Local-first macOS app (Tauri 2 + Rust + React) that shelves dormant developer projects: discover, understand, preserve into a verified capsule, restore transactionally, and write an evidence-graded handoff brief. A browser demo runs the same Rust core compiled to WebAssembly on a synthetic project. Built for Kiroween 2026, Frankenstein category. Deadline Fri Dec 4, 2026, 2:00 PM PT; target submit Wed Dec 2, 12:00 PM PT.

## Where things are
- Plan: `docs/MASTER_PLAN.md` (source of truth; do not contradict it).
- Specs: `.kiro/specs/{foundation,scope-and-discovery,capsule-and-verification,transactional-restore,intelligence-and-brief,native-experience,judge-demo,release-and-submission}/`.
- Status ledger: `docs/status/STATUS.md`. Deviations from the plan are listed there.
- Repo: https://github.com/AmirmLotfy/dormbit (public, MIT). Git identity is the GitHub noreply address.

## Current position
Phase 01 (design) is complete except two user actions: run Analyze Requirements, then the design-phase Correctness step, on the `capsule-and-verification` and `transactional-restore` specs in the Kiro Specs panel. Next: Phase 02, Task 9 (monorepo scaffold).

## Environment facts that matter
- Agent shells need `. "$HOME/.cargo/env"` before cargo; `.scratch/run.sh <name> <cmd>` does this and logs to `.scratch/logs/`.
- Inline terminal output is unreliable in the IDE; write command output to `.scratch/` and read the file.
- `CI` must be `true` (not `1`) for the Tauri CLI.
- Kiro always asks before writing `.kiro/agents`, `.kiro/hooks`, and `.git` paths. Workspace permissions live outside the repo at `~/.kiro/workspace-roots/<hash>/permissions.yaml`; the recommended contents are in `docs/kiro/`.
- Deny rules block `rm -rf` except for `.scratch/`, `target`, `node_modules`, `dist`, `spikes`.
- gitleaks runs as a pre-commit hook (`core.hooksPath=.githooks`). Reviewed false positives go in `.gitleaksignore`.

## Rules that must not be weakened
1. Never delete or overwrite user data; the only deletion is DORMBIT's own staging folders after an owner-token check.
2. Never run project code or the git CLI.
3. Never read real agent histories or credential stores (`~/.claude`, `~/.codex`, `~/.ssh`, `~/.aws`, `~/.config/gh`).
4. Report the four outcomes separately; never claim a session resumed or a project builds.
5. A result counts only if recorded by `scripts/evidence/run.sh` or CI.
6. Stop and ask before: installs outside the repo, publishing a release, Screen Recording, enabling the Chrome extension, making the video public, the final Devpost submit.
