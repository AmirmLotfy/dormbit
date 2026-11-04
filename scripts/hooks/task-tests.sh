#!/usr/bin/env bash
# Runs a spec's tests through the Evidence_Runner (foundation Requirement 22.12).
#
#   scripts/hooks/task-tests.sh <spec-name> [task-id]     (the task-tests Skill)
#   scripts/hooks/task-tests.sh < hook-payload.json       (a PostTaskExec hook, if its payload names the spec)
#
# Commands per spec live in scripts/hooks/task-tests.json as argv lists; nothing is run
# through a shell string.
set -u

root="$(cd "$(dirname "$0")/../.." && pwd -P)"
map="$root/scripts/hooks/task-tests.json"
spec="${1:-}"
task="${2:-}"
if [ -z "$spec" ] && [ ! -t 0 ]; then
  payload="$(cat)"
  spec="$(printf '%s' "$payload" | jq -r '.spec_name // .spec // .task.spec // empty' 2>/dev/null)"
  task="$(printf '%s' "$payload" | jq -r '.task_id // .task.id // empty' 2>/dev/null)"
fi
if [ -z "$spec" ]; then
  echo "task-tests: no spec name (pass one, for example: scripts/hooks/task-tests.sh foundation)" >&2
  exit 0
fi
if ! jq -e --arg s "$spec" 'has($s)' "$map" >/dev/null 2>&1; then
  echo "task-tests: no test commands for spec '$spec' in scripts/hooks/task-tests.json" >&2
  exit 1
fi

[ -n "$task" ] && export EVIDENCE_TASK="$task"
status=0
count="$(jq -r --arg s "$spec" '.[$s] | length' "$map")"
i=0
while [ "$i" -lt "$count" ]; do
  args=()
  while IFS= read -r -d '' a; do args+=("$a"); done < <(jq -j --arg s "$spec" --argjson i "$i" '.[$s][$i][] | ., "\u0000"' "$map")
  echo "task-tests: ${args[*]}"
  (cd "$root" && bash scripts/evidence/run.sh "${args[@]}") || status=1
  i=$((i + 1))
done
exit "$status"
