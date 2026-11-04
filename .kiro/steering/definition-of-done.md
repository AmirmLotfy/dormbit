---
inclusion: always
---

# Definition of done

Ledger statuses in `docs/status/STATUS.md`, in order:

| Status | Meaning |
| --- | --- |
| Planned | Not started |
| In progress | Being worked on |
| Implemented | Merged and builds |
| Tested | Its tests pass in a recorded run |
| Verified | Reviewer approved, and demonstrated with a log or screenshot |
| Released | In a tag or deployment |
| Submitted | Part of the final submission |

- **A result counts only if it was recorded through the Evidence_Runner (`scripts/evidence/run.sh`, records in `docs/evidence/runs/`) or by CI.** Numbers in docs, the README, or Devpost come from those records, never from memory.
- A record from a dirty tree or a later code change doesn't prove the current code; re-run it.
- **Review loop:** after a task, run the `reviewer` sub-agent on the task's diff. Its last line is `APPROVED` or `NEEDS_CHANGES`. Fix and re-review until `APPROVED`, at most 3 iterations (`max_iterations: 3`); then stop and report to the user. Record the verdict in STATUS.md and notable catches in `docs/kiro/usage-log.md`.
- Each phase gate writes `docs/status/gates/PXX.md` (what works, what's missing, what's next) and updates `docs/status/HANDOFF.md`.
- Every commit names its task ID. Push after each completed task.
