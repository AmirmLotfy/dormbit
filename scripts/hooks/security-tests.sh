#!/usr/bin/env bash
# PostFileSave hook on security modules (foundation Requirement 22.11): runs the fast security
# subset (tests named `security_fast`) with 64 property cases and prints any failures.
set -u

root="$(cd "$(dirname "$0")/../.." && pwd -P)"
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cat >/dev/null
command -v cargo >/dev/null 2>&1 || exit 0
cd "$root" || exit 0

out="$(mktemp)"
trap 'rm -f "$out"' EXIT
if PROPTEST_CASES=64 cargo test -p dormbit-core -p dormbit-fs --locked security_fast >"$out" 2>&1; then
  echo "security-tests: fast security subset passed"
  exit 0
fi
echo "security-tests: the fast security subset failed"
grep -E '^(test .* FAILED|failures:|---- |thread .* panicked|error(\[|:))' "$out" | head -40
exit 1
