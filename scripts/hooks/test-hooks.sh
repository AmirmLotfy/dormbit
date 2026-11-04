#!/usr/bin/env bash
# shellcheck disable=SC2181 # each check reads $? right after the command it tests
# Tests for the hook scripts (foundation task 5.4). Payloads use the field names recorded by
# the Task 4 probes: session_id, hook_event_name, cwd, tool_name, tool_input. Fake secrets are
# generated at run time, so none is ever committed. Exits 1 if any check fails.
set -u

root="$(cd "$(dirname "$0")/../.." && pwd -P)"
H="$root/scripts/hooks"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fails=0
ok() { echo "ok    $1"; }
bad() {
  echo "FAIL  $1"
  fails=$((fails + 1))
}

payload() { # <tool_name> <path> [text]
  jq -n --arg t "$1" --arg p "$2" --arg x "${3:-}" --arg c "$root" \
    '{session_id: "sess_test", hook_event_name: "PreToolUse", cwd: $c, tool_name: $t, tool_input: {path: $p, text: $x}}'
}

# ---- format.sh ------------------------------------------------------------------------------
outside="$tmp/outside.rs"
printf 'fn  main( ){}\n' >"$outside"
payload fs_write "$outside" | bash "$H/format.sh"
if [ "$?" -eq 0 ] && [ "$(cat "$outside")" = 'fn  main( ){}' ]; then ok "format: a file outside the repo is left alone"; else bad "format: a file outside the repo is left alone"; fi

inside="$root/.scratch/hooktest-format.rs"
mkdir -p "$root/.scratch"
printf 'fn  main( ){}\n' >"$inside"
payload fs_write "$inside" | bash "$H/format.sh"
if [ "$(cat "$inside")" = 'fn  main( ){}' ]; then ok "format: .scratch/ is skipped"; else bad "format: .scratch/ is skipped"; fi
rm -f "$inside"

rsfile="$root/crates/dormbit-cli/src/hooktest_tmp.rs"
printf 'pub fn  x( )->u8{1}\n' >"$rsfile"
payload fs_write "crates/dormbit-cli/src/hooktest_tmp.rs" | bash "$H/format.sh"
if [ "$(cat "$rsfile")" = 'pub fn x() -> u8 {' ] || grep -q '^pub fn x() -> u8 {' "$rsfile"; then ok "format: a .rs file in the repo is formatted by rustfmt"; else bad "format: a .rs file in the repo is formatted by rustfmt"; fi
rm -f "$rsfile"

evil="$root/.scratch/a;touch $tmp/pwned.ts"
payload fs_write "$evil" | bash "$H/format.sh"
if [ ! -e "$tmp/pwned.ts" ]; then ok "format: a path with shell syntax is never executed"; else bad "format: a path with shell syntax is never executed"; fi

printf '{}' | bash "$H/format.sh"
if [ "$?" -eq 0 ]; then ok "format: a payload without a path exits 0"; else bad "format: a payload without a path exits 0"; fi

# ---- secret-scan.sh -------------------------------------------------------------------------
fake="$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40)"
payload fs_write "notes.md" "aws_secret_access_key = \"$fake\"" | bash "$H/secret-scan.sh" 2>"$tmp/err"
code=$?
if [ "$code" -eq 2 ] && grep -q "rule" "$tmp/err" && ! grep -qF "$fake" "$tmp/err"; then
  ok "secret-scan: blocks a run-time fake secret (exit 2) and never prints it"
else bad "secret-scan: blocks a run-time fake secret (exit 2) and never prints it (got $code)"; fi

payload fs_write "notes.md" "just some notes, nothing secret" | bash "$H/secret-scan.sh" 2>/dev/null
if [ "$?" -eq 0 ]; then ok "secret-scan: clean content passes"; else bad "secret-scan: clean content passes"; fi

payload fs_write "notes.md" "x" | PATH="/usr/bin:/bin" bash "$H/secret-scan.sh" 2>"$tmp/err"
code=$?
if [ "$code" -eq 2 ] && grep -q "could not be scanned" "$tmp/err"; then ok "secret-scan: blocks when gitleaks is missing"; else bad "secret-scan: blocks when gitleaks is missing (got $code)"; fi

# ---- stop-gate.sh (fake cargo and pnpm, so the test is fast and controllable) ---------------
fakebin="$tmp/bin"
mkdir -p "$fakebin"
printf '#!/bin/sh\necho "error[E0308]: mismatched types"\nexit 1\n' >"$fakebin/cargo"
printf '#!/bin/sh\necho "src/x.ts(1,1): error TS2322: nope"\nexit 1\n' >"$fakebin/pnpm"
chmod +x "$fakebin/cargo" "$fakebin/pnpm"
counter="$tmp/stop.count"
codes=""
for _ in 1 2 3; do
  echo '{}' | HOME="$tmp" STOP_GATE_COUNTER="$counter" PATH="$fakebin:/usr/bin:/bin" bash "$H/stop-gate.sh" >"$tmp/stop.out" 2>&1
  codes="$codes$?"
done
if [ "$codes" = "110" ] && grep -q "giving up" "$tmp/stop.out" && [ "$(cat "$counter")" = 0 ]; then
  ok "stop-gate: exits 1, 1, then gives up on the 3rd failure and resets"
else bad "stop-gate: exits 1, 1, then gives up on the 3rd failure and resets (got $codes)"; fi

printf '#!/bin/sh\nexit 0\n' >"$fakebin/cargo"
printf '#!/bin/sh\nexit 0\n' >"$fakebin/pnpm"
printf '2\n' >"$counter"
echo '{}' | HOME="$tmp" STOP_GATE_COUNTER="$counter" PATH="$fakebin:/usr/bin:/bin" bash "$H/stop-gate.sh" >/dev/null 2>&1
if [ "$?" -eq 0 ] && [ "$(cat "$counter")" = 0 ]; then ok "stop-gate: passing checks reset the counter"; else bad "stop-gate: passing checks reset the counter"; fi

echo '{}' | HOME="$tmp" STOP_GATE_COUNTER="$counter" PATH="/usr/bin:/bin" bash "$H/stop-gate.sh" >/dev/null 2>&1
if [ "$?" -eq 0 ]; then ok "stop-gate: no toolchain means exit 0"; else bad "stop-gate: no toolchain means exit 0"; fi

# ---- release-gate.sh (a throwaway repo, one failing condition at a time) --------------------
repo="$tmp/repo"
mkdir -p "$repo/docs/evidence/runs" "$repo/.kiro"
git -C "$repo" init -q
git -C "$repo" config user.email test@example.invalid
git -C "$repo" config user.name test
printf 'MIT\n' >"$repo/LICENSE"
printf 'x\n' >"$repo/.kiro/a.md"
printf 'code\n' >"$repo/code.txt"
git -C "$repo" add -A && git -C "$repo" commit -qm init
sha="$(git -C "$repo" rev-parse HEAD)"
record() { jq -n --arg s "$sha" --argjson c "$1" '{exit_code: $c, git: {sha: $s}}' >"$repo/docs/evidence/runs/20260101T000000Z-x.json"; }
gate() { RELEASE_GATE_ROOT="$repo" bash "$H/release-gate.sh" --manual 2>"$tmp/gate.err"; }

record 0
gate
if [ "$?" -eq 0 ]; then ok "release-gate: passes when every check is green"; else bad "release-gate: passes when every check is green ($(tr '\n' ' ' <"$tmp/gate.err"))"; fi

record 1
gate
if [ "$?" -eq 2 ] && grep -q "exit code 1" "$tmp/gate.err"; then ok "release-gate: blocks a red Evidence record"; else bad "release-gate: blocks a red Evidence record"; fi
record 0

printf 'changed\n' >"$repo/code.txt"
git -C "$repo" commit -qam change
gate
if [ "$?" -eq 2 ] && grep -q "code changed" "$tmp/gate.err"; then ok "release-gate: blocks when code changed after the record"; else bad "release-gate: blocks when code changed after the record"; fi
sha="$(git -C "$repo" rev-parse HEAD)"
record 0

mv "$repo/LICENSE" "$repo/LICENSE.bak"
gate
if [ "$?" -eq 2 ] && grep -q "LICENSE is missing" "$tmp/gate.err"; then ok "release-gate: blocks a missing LICENSE"; else bad "release-gate: blocks a missing LICENSE"; fi
mv "$repo/LICENSE.bak" "$repo/LICENSE"

printf '.kiro\n' >"$repo/.gitignore"
gate
if [ "$?" -eq 2 ] && grep -q ".kiro is ignored" "$tmp/gate.err"; then ok "release-gate: blocks an ignored .kiro"; else bad "release-gate: blocks an ignored .kiro"; fi
rm "$repo/.gitignore"

leak="$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 40)"
printf 'aws_secret_access_key = "%s"\n' "$leak" >"$repo/leak.txt"
git -C "$repo" add leak.txt && git -C "$repo" commit -qm leak
sha="$(git -C "$repo" rev-parse HEAD)"
record 0
gate
if [ "$?" -eq 2 ] && grep -q "gitleaks found" "$tmp/gate.err" && ! grep -qF "$leak" "$tmp/gate.err"; then
  ok "release-gate: blocks a secret in the history without printing it"
else bad "release-gate: blocks a secret in the history without printing it"; fi

jq -n '{hook_event_name: "PreTaskExec", spec_name: "native-experience", task_id: "34"}' | RELEASE_GATE_ROOT="$repo" bash "$H/release-gate.sh" 2>/dev/null
if [ "$?" -eq 0 ]; then ok "release-gate: tasks outside release-and-submission 5-17 pass"; else bad "release-gate: tasks outside release-and-submission 5-17 pass"; fi
jq -n '{hook_event_name: "PreTaskExec", spec_name: "release-and-submission", task_id: "9"}' | RELEASE_GATE_ROOT="$repo" bash "$H/release-gate.sh" 2>/dev/null
if [ "$?" -eq 2 ]; then ok "release-gate: release-and-submission task 9 is gated"; else bad "release-gate: release-and-submission task 9 is gated"; fi

# ---- task-tests.sh --------------------------------------------------------------------------
bash "$H/task-tests.sh" no-such-spec >/dev/null 2>&1
if [ "$?" -eq 1 ]; then ok "task-tests: an unknown spec fails"; else bad "task-tests: an unknown spec fails"; fi
if jq -e 'to_entries | all(.value | type == "array" and all(type == "array" and length > 0 and all(type == "string")))' "$H/task-tests.json" >/dev/null; then
  ok "task-tests: every spec maps to argv lists"
else bad "task-tests: every spec maps to argv lists"; fi

# ---- shellcheck -----------------------------------------------------------------------------
if shellcheck "$H"/*.sh; then ok "shellcheck reports no findings"; else bad "shellcheck reports no findings"; fi

if [ "$fails" -eq 0 ]; then
  echo "test-hooks: all checks passed"
else
  echo "test-hooks: $fails check(s) failed"
  exit 1
fi
