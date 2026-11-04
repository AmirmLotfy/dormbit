#!/usr/bin/env bash
# Release gate (foundation Requirement 22.14). Exit 2 blocks and names each failed check.
#
#   scripts/hooks/release-gate.sh --manual      check now (the release-gate Skill)
#   scripts/hooks/release-gate.sh < payload     PreTaskExec: gate only release-and-submission
#                                                tasks 5-17 (Phases 12-18); pass everything else
#
# RELEASE_GATE_ROOT points the checks at another repo (used by test-hooks.sh).
set -u

root="${RELEASE_GATE_ROOT:-$(cd "$(dirname "$0")/../.." && pwd -P)}"

if [ "${1:-}" != "--manual" ]; then
  payload="$(cat)"
  spec="$(printf '%s' "$payload" | jq -r '.spec_name // .spec // .task.spec // empty' 2>/dev/null)"
  task="$(printf '%s' "$payload" | jq -r '.task_id // .task.id // empty' 2>/dev/null)"
  [ "$spec" = "release-and-submission" ] || exit 0
  n="${task%%[!0-9]*}"
  case "$n" in '' | *[!0-9]*) exit 0 ;; esac
  if [ "$n" -lt 5 ] || [ "$n" -gt 17 ]; then exit 0; fi
fi

cd "$root" || exit 2
failed=()

latest="$(find docs/evidence/runs -name '*.json' 2>/dev/null | sort | tail -1)"
if [ -z "$latest" ]; then
  failed+=("no Evidence record in docs/evidence/runs/")
else
  code="$(jq -r '.exit_code' "$latest" 2>/dev/null)"
  sha="$(jq -r '.git.sha' "$latest" 2>/dev/null)"
  [ "$code" = "0" ] || failed+=("the latest Evidence record ($latest) has exit code $code")
  if ! git cat-file -e "$sha^{commit}" 2>/dev/null; then
    failed+=("the latest Evidence record names commit $sha, which is not in this repo")
  elif ! git diff --quiet "$sha" HEAD -- . ':!docs/'; then
    failed+=("code changed after the latest Evidence record (commit ${sha:0:7}); record a fresh run")
  fi
fi

if ! command -v gitleaks >/dev/null 2>&1; then
  failed+=("gitleaks is not installed")
elif ! gitleaks git --redact --no-banner --exit-code 1 . >/dev/null 2>&1; then
  failed+=("gitleaks found a secret in the Git history")
fi

[ -f LICENSE ] || failed+=("LICENSE is missing")

# --no-index tests the ignore rules even when .kiro files are already tracked.
ignored="$(git check-ignore --no-index .kiro 2>/dev/null)"
[ -z "$ignored" ] || failed+=(".kiro is ignored by Git ($ignored); the hackathon requires it in the repo")

if [ "${#failed[@]}" -eq 0 ]; then
  echo "release-gate: all checks passed"
  exit 0
fi
echo "release-gate: blocked" >&2
for f in "${failed[@]}"; do echo "  - $f" >&2; done
exit 2
