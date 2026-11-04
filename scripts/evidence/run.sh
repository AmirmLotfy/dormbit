#!/usr/bin/env bash
# Evidence_Runner (foundation Requirement 17). Runs one command, keeps its full log, and writes
# an Evidence record with the exit code, test counts, commit, and timing.
#
#   scripts/evidence/run.sh <command> [args...]
#
# Optional env: EVIDENCE_TASK (for example P02-T04), PROPTEST_CASES, FC_NUM_RUNS.
# EVIDENCE_DIR and EVIDENCE_LOG_DIR move the records and logs (scripts/evidence/test-run.sh uses them).
# The command runs as separate argv entries: no eval, no sh -c.
set -u
set -o pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: scripts/evidence/run.sh <command> [args...]" >&2
  exit 2
fi

root="$(cd "$(dirname "$0")/../.." && pwd)"
records="${EVIDENCE_DIR:-$root/docs/evidence/runs}"
logs="${EVIDENCE_LOG_DIR:-$root/.scratch/logs}"
mkdir -p "$records" "$logs"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
slug="$(printf '%s' "${1##*/} ${2:-}" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9' '-' |
  sed -e 's/--*/-/g' -e 's/^-//' -e 's/-$//' | cut -c1-40)"
base="$stamp-${slug:-run}"
name="$base"
n=1
while [ -e "$records/$name.json" ] || [ -e "$logs/$name.log" ]; do
  n=$((n + 1))
  name="$base-$n"
done
log="$logs/$name.log"

sha="$(git -C "$root" rev-parse HEAD 2>/dev/null || echo unknown)"
# Dirty means source changes; records written by earlier runs don't count.
if [ -n "$(git -C "$root" status --porcelain -- . ':(exclude)docs/evidence/runs' 2>/dev/null)" ]; then
  dirty=true
else
  dirty=false
fi
started="$(node -e 'process.stdout.write(String(Date.now()))')"

"$@" 2>&1 | tee "$log"
code="${PIPESTATUS[0]}"

finished="$(node -e 'process.stdout.write(String(Date.now()))')"
os="$(uname -s)"
if command -v sw_vers >/dev/null 2>&1; then os="macOS $(sw_vers -productVersion)"; fi

node "$root/scripts/evidence/write-record.mjs" \
  --record "$records/$name.json" --log "$log" --root "$root" \
  --exit "$code" --started "$started" --finished "$finished" \
  --sha "$sha" --dirty "$dirty" --cwd "$PWD" --os "$os" --arch "$(uname -m)" \
  -- "$@"

exit "$code"
