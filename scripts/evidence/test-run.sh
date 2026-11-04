#!/usr/bin/env bash
# Tests for scripts/evidence/run.sh (foundation Requirement 17). Each case writes into its own
# temporary folder, so nothing is added to docs/evidence/runs. Exits 1 if any check fails.
set -u

root="$(cd "$(dirname "$0")/../.." && pwd)"
run="$root/scripts/evidence/run.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
unset EVIDENCE_TASK PROPTEST_CASES FC_NUM_RUNS
fails=0

ok() { echo "ok    $1"; }
bad() {
  echo "FAIL  $1"
  fails=$((fails + 1))
}

# Points the runner at a fresh folder for one case.
use_case() {
  export EVIDENCE_DIR="$tmp/runs/$1" EVIDENCE_LOG_DIR="$tmp/logs/$1"
}

record() { find "$EVIDENCE_DIR" -name '*.json' | head -1; }
logfile() { find "$EVIDENCE_LOG_DIR" -name '*.log' | head -1; }

# Prints one field of the case's record as JSON, for example `field counts.passed`.
field() {
  node -e 'const r = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    const v = process.argv[2].split(".").reduce((o, k) => (o == null ? o : o[k]), r);
    process.stdout.write(JSON.stringify(v === undefined ? null : v));' "$(record)" "$1"
}

quiet() { "$@" >/dev/null 2>&1; }

use_case true
quiet "$run" true
code=$?
if [ "$code" -eq 0 ]; then ok "true exits 0"; else bad "true exits 0 (got $code)"; fi
if [ "$(field exit_code)" = 0 ] && [ "$(field counts)" = null ] && [ "$(field counts_source)" = '"unknown"' ]; then
  ok "true: exit_code 0, counts null, source unknown"
else bad "true: exit_code 0, counts null, source unknown"; fi
if [ "$(field task)" = null ] && [ "$(field env)" = '{}' ]; then
  ok "no task and no env values when unset"
else bad "no task and no env values when unset"; fi

use_case false
quiet "$run" false
code=$?
if [ "$code" -eq 1 ] && [ "$(field exit_code)" = 1 ]; then ok "false: exits 1 and records 1"; else bad "false: exits 1 and records 1"; fi

use_case missing
quiet "$run" dormbit-no-such-command-xyz
code=$?
if [ "$code" -eq 127 ] && [ "$(field exit_code)" = 127 ]; then
  ok "missing command: exits 127 and records 127"
else bad "missing command: exits 127 and records 127 (got $code)"; fi

use_case cargo
quiet "$run" printf 'test result: ok. 3 passed; 1 failed; 2 ignored; 0 measured\ntest result: ok. 4 passed; 0 failed; 0 ignored; 0 measured\n'
if [ "$(field counts)" = '{"passed":7,"failed":1,"skipped":2}' ] && [ "$(field counts_source)" = '"cargo"' ]; then
  ok "cargo summaries are summed (7 passed, 1 failed, 2 ignored)"
else bad "cargo summaries are summed"; fi

use_case vitest
quiet "$run" printf '      Tests  5 passed | 1 failed | 2 skipped (8)\n'
if [ "$(field counts)" = '{"passed":5,"failed":1,"skipped":2}' ] && [ "$(field counts_source)" = '"vitest"' ]; then
  ok "Vitest summary is parsed"
else bad "Vitest summary is parsed"; fi

use_case playwright
quiet "$run" printf '  6 passed (3.1s)\n  1 skipped\n'
if [ "$(field counts)" = '{"passed":6,"failed":0,"skipped":1}' ] && [ "$(field counts_source)" = '"playwright"' ]; then
  ok "Playwright summary is parsed"
else bad "Playwright summary is parsed"; fi

use_case env
EVIDENCE_TASK=P02-T04 PROPTEST_CASES=512 FC_NUM_RUNS=128 quiet "$run" true
if [ "$(field task)" = '"P02-T04"' ] && [ "$(field env)" = '{"PROPTEST_CASES":"512","FC_NUM_RUNS":"128"}' ]; then
  ok "EVIDENCE_TASK, PROPTEST_CASES, and FC_NUM_RUNS are recorded"
else bad "EVIDENCE_TASK, PROPTEST_CASES, and FC_NUM_RUNS are recorded"; fi

use_case home
quiet "$run" printf '%s\n' "$HOME/secret/path"
if grep -q '^~/secret/path$' "$(logfile)" && ! grep -qF "$HOME" "$(logfile)"; then
  ok "raw log has the home folder replaced with ~"
else bad "raw log has the home folder replaced with ~"; fi

use_case argv
quiet "$run" printf '%s\n' 'a; echo injected'
if [ "$(cat "$(logfile)")" = 'a; echo injected' ]; then
  ok "arguments run as argv, not through a shell"
else bad "arguments run as argv, not through a shell"; fi

use_case usage
quiet "$run"
code=$?
if [ "$code" -eq 2 ] && [ -z "$(find "$tmp/runs" -path "*/usage/*" -name '*.json')" ]; then
  ok "no arguments: usage, exit 2, no record"
else bad "no arguments: usage, exit 2, no record (got $code)"; fi

if ! grep -rqF "$HOME" "$tmp/runs"; then ok "no record contains the home folder"; else bad "no record contains the home folder"; fi

if node -e 'const fs = require("fs"), path = require("path");
    const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]);
    for (const f of walk(process.argv[1])) {
      const r = JSON.parse(fs.readFileSync(f, "utf8"));
      for (const k of ["schema_version","task","command","cwd","started_at","finished_at","duration_ms","exit_code","counts","counts_source","env","git","host","log"])
        if (!(k in r)) { console.error(f + " lacks " + k); process.exit(1); }
      if (!/^[0-9a-f]{40}$|^unknown$/.test(r.git.sha) || typeof r.git.dirty !== "boolean") process.exit(1);
    }' "$tmp/runs"; then
  ok "every record has all fields and a git sha"
else bad "every record has all fields and a git sha"; fi

if command -v shellcheck >/dev/null 2>&1; then
  if shellcheck "$run" "$root/scripts/evidence/test-run.sh"; then ok "shellcheck reports no findings"; else bad "shellcheck reports no findings"; fi
else
  bad "shellcheck is installed"
fi

if [ "$fails" -eq 0 ]; then
  echo "test-run: all checks passed"
else
  echo "test-run: $fails check(s) failed"
  exit 1
fi
