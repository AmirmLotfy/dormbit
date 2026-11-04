#!/usr/bin/env bash
# Stop hook (foundation Requirements 22.6 to 22.8): when the agent ends a turn, check that the
# workspace still compiles and type-checks. Exit 1 hands the errors back to the agent for
# another turn; after 3 failures in a row it gives up so it can never loop forever.
set -u

root="$(cd "$(dirname "$0")/../.." && pwd -P)"
counter="${STOP_GATE_COUNTER:-$root/.scratch/stop-gate.count}"
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cat >/dev/null # the Stop payload carries nothing this gate needs

reset() {
  mkdir -p "$(dirname "$counter")"
  printf '0\n' >"$counter"
}

if ! command -v cargo >/dev/null 2>&1 || ! command -v pnpm >/dev/null 2>&1 ||
  [ ! -f "$root/Cargo.toml" ] || [ ! -d "$root/node_modules" ]; then
  reset
  exit 0
fi

cd "$root" || exit 0
out="$(mktemp)"
trap 'rm -f "$out"' EXIT
ok=1
# Tauri's build step needs the desktop frontend's dist/; skip that crate until it is built.
exclude=()
[ -d "$root/apps/desktop/dist" ] || exclude=(--exclude dormbit-desktop)
if ! cargo check --workspace ${exclude[@]+"${exclude[@]}"} --locked --message-format short >"$out" 2>&1; then
  ok=0
  echo "stop-gate: cargo check failed"
  grep -E '(error|warning)(\[[A-Z0-9]+\])?:' "$out" | head -40
fi
if ! pnpm --silent typecheck >"$out" 2>&1; then
  ok=0
  echo "stop-gate: pnpm typecheck failed"
  grep -E 'error TS[0-9]+' "$out" | head -40
fi

if [ "$ok" -eq 1 ]; then
  reset
  exit 0
fi

n="$(cat "$counter" 2>/dev/null || echo 0)"
case "$n" in '' | *[!0-9]*) n=0 ;; esac
n=$((n + 1))
if [ "$n" -ge 3 ]; then
  echo "stop-gate: 3 failed turns in a row; giving up. Fix the errors above, then continue."
  reset
  exit 0
fi
mkdir -p "$(dirname "$counter")"
printf '%s\n' "$n" >"$counter"
echo "stop-gate: fix the errors above (attempt $n of 3)."
exit 1
