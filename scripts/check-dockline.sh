#!/bin/sh
# Extracts the committed Dockline fixture to .scratch/dockline/ and checks it (foundation task
# 4.13; Requirements 18.3 to 18.5, 18.11 to 18.13, 19.8). Exits 1 if any check fails.
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
D="$ROOT/.scratch/dockline"
P="$D/dockline-invoices"
rm -rf "$D"
mkdir -p "$D"
tar -xf "$ROOT/fixtures/dockline/dockline-invoices.tar" -C "$D"
fails=0

pass() { echo "ok    $1"; }
fail() {
  echo "FAIL  $1"
  fails=$((fails + 1))
}

# Git must not pick up the user's settings while reading the fixture.
g() { HOME="$D" XDG_CONFIG_HOME="$D" GIT_CONFIG_NOSYSTEM=1 git -C "$P" "$@"; }

top="$(cd "$D" && find . -mindepth 1 -maxdepth 1 | sort | tr '\n' ' ')"
if [ "$top" = "./agent-stores ./dockline-invoices " ]; then pass "two top-level folders"; else fail "two top-level folders"; fi

if gitleaks dir "$D" --no-banner --exit-code 1 >/dev/null 2>&1; then pass "gitleaks: zero findings"; else fail "gitleaks: zero findings"; fi

expected=" M docs/ROADMAP.md
 M src/pdf/renderInvoice.ts
?? notes/pdf-ideas.md"
if [ "$(g status --porcelain --untracked-files=all)" = "$expected" ]; then
  pass "status: 2 modified, 0 staged, 1 untracked note"
else fail "status: 2 modified, 0 staged, 1 untracked note"; fi

if [ "$(g stash list | wc -l | tr -d ' ')" = 1 ]; then pass "one stash entry"; else fail "one stash entry"; fi

main="$(g rev-list --count main)"
feature="$(g rev-list --count feature/pdf-export)"
if [ "$main" = 17 ] && [ "$feature" = 24 ] && [ "$(g symbolic-ref --short HEAD)" = feature/pdf-export ]; then
  pass "24 commits (17 on main, 7 more on feature/pdf-export), HEAD on feature/pdf-export"
else fail "24 commits (17 on main, 7 more on feature/pdf-export), HEAD on feature/pdf-export"; fi

authors="$(g log --all --format='%ae %ce' | tr ' ' '\n' | sort -u | tr '\n' ' ')"
if [ "$authors" = "mara@dockline.invalid theo@dockline.invalid " ]; then pass "only the two .invalid authors"; else fail "only the two .invalid authors"; fi

dates="$(g log --all --format='%aI %cI' | tr ' ' '\n' | cut -c1-10 | tr -d '-' | sort -u)"
if [ "$(echo "$dates" | head -1)" -ge 20250303 ] && [ "$(echo "$dates" | tail -1)" -le 20250523 ]; then
  pass "dates between 2025-03-03 and 2025-05-23"
else fail "dates between 2025-03-03 and 2025-05-23"; fi

if g log --all --format=%s | grep -q '^WIP'; then pass "a WIP commit exists"; else fail "a WIP commit exists"; fi

if [ -z "$(ls -A "$P/.git/hooks")" ]; then pass ".git/hooks is empty"; else fail ".git/hooks is empty"; fi

if ! grep -r -q -F "$HOME" "$D"; then pass "no home folder path"; else fail "no home folder path"; fi

# The user's names and email, read locally and never printed.
leaked=0
for v in "$(id -F 2>/dev/null)" "$(id -un)" "$(git config --global user.email 2>/dev/null)" "$(git config --global user.name 2>/dev/null)"; do
  if [ -n "$v" ] && grep -r -q -i -F "$v" "$D"; then leaked=1; fi
done
if [ "$leaked" -eq 0 ]; then pass "no real name, user name, or email"; else fail "no real name, user name, or email"; fi

if [ "$fails" -eq 0 ]; then
  echo "check-dockline: all checks passed"
else
  echo "check-dockline: $fails check(s) failed"
  exit 1
fi
