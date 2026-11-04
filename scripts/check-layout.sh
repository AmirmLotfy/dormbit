#!/bin/sh
# Checks the §6 repo layout exists. Exits 1 with one line per missing path.
set -u
cd "$(dirname "$0")/.."
fail=0
req() {
  if [ ! -e "$1" ]; then
    echo "missing: $1"
    fail=1
  fi
}
for d in apps/desktop apps/demo \
         crates/dormbit-core crates/dormbit-fs crates/dormbit-git \
         crates/dormbit-agents crates/dormbit-wasm crates/dormbit-cli \
         packages/schema packages/client packages/ui \
         fixtures/dockline fixtures/golden fixtures/malicious \
         e2e docs/architecture docs/security docs/release docs/submission \
         docs/evidence docs/status docs/kiro \
         .github/workflows scripts .githooks .kiro; do
  req "$d"
done
for f in package.json pnpm-workspace.yaml Cargo.toml tsconfig.base.json \
         rust-toolchain.toml .mise.toml .gitignore LICENSE; do
  req "$f"
done
if [ -e spikes ]; then
  echo "spikes/ should have been removed in Task 9 (P02-T01)"
  fail=1
fi
if [ "$fail" -eq 0 ]; then
  echo "check-layout: OK"
fi
exit "$fail"
