#!/bin/sh
# Checks that core/fs/git/wasm never pull in a transport or process-spawning
# feature, and that git2's network features stay off (GIT-1, ADR-004).
set -u
# shellcheck source=/dev/null
. "$HOME/.cargo/env" 2>/dev/null
cd "$(dirname "$0")/.." || exit 1
fail=0

check_no_feature() {
  pkg="$1"; feat="$2"
  if cargo tree -p "$pkg" -e features --locked 2>/dev/null | grep -qi "feature \"$feat\""; then
    echo "forbidden: $pkg pulls in feature \"$feat\""
    fail=1
  fi
}

for pkg in dormbit-core dormbit-fs dormbit-git dormbit-agents dormbit-wasm; do
  check_no_feature "$pkg" https
  check_no_feature "$pkg" ssh
  check_no_feature "$pkg" ssh_key_from_memory
done

if cargo tree -p dormbit-wasm -e features --locked 2>/dev/null | grep -qi "git2"; then
  echo "forbidden: dormbit-wasm must not depend on git2 (native only, ADR-004)"
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "check-deps: OK"
fi
exit "$fail"
