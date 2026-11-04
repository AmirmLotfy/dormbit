---
name: release-gate
description: Run the DORMBIT release gate by hand before any release-and-submission task from Phase 12 on. Checks that the latest evidence is green with no later code change, gitleaks is clean, LICENSE exists, and .kiro is not ignored. Use before building, publishing, or submitting.
---

# Release gate

Run `sh scripts/hooks/release-gate.sh --manual` from the repo root. It exits 0 only when every check passes and names each failed check otherwise:

1. The newest record in `docs/evidence/runs/` has `exit_code` 0.
2. No code changed after that record: `git diff --quiet <record git.sha> HEAD -- . ':!docs/'`.
3. `gitleaks git --redact --no-banner` finds nothing.
4. `LICENSE` exists at the repo root.
5. `git check-ignore --no-index .kiro` prints nothing (the hackathon requires `/.kiro` in the repo; `--no-index` catches an ignore rule even when `.kiro` files are already tracked).

If a check fails, stop the release task, fix the cause, record a fresh green run through `scripts/evidence/run.sh`, and run the gate again. Never edit an Evidence record by hand.

This runs as a Skill instead of a `PreTaskExec` hook because Kiro's task-hook payload isn't documented and hasn't been observed to name the task (foundation Requirement 22.15).
