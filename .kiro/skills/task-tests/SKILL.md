---
name: task-tests
description: Run a finished spec task's tests through the Evidence_Runner and report the counts. Use right after completing any task in .kiro/specs, before marking it Tested.
---

# Task tests

1. Find the task's spec name (the folder under `.kiro/specs/`) and its task ID (for example `P03-T02`).
2. Run `bash scripts/hooks/task-tests.sh <spec-name> <task-id>`. It reads the commands for that spec from `scripts/hooks/task-tests.json` and runs each through `scripts/evidence/run.sh`, so every run leaves an Evidence record in `docs/evidence/runs/`.
3. Report each command's exit code and counts from the new records. A task is Tested only if every command passed from a clean tree.
4. If a spec's tests change, update its entry in `scripts/hooks/task-tests.json` (an argv list per command, never a shell string).

This runs as a Skill instead of a `PostTaskExec` hook because Kiro's task-hook payload isn't documented and hasn't been observed to name the task (foundation Requirement 22.15).
