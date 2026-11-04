#!/usr/bin/env bash
# PreToolUse hook on write tools (foundation Requirements 22.9, 22.10, 22.16): blocks a write
# whose new content contains a secret. Exit 2 blocks the tool call. The secret itself is never
# printed: gitleaks runs with --redact and only rule IDs are shown.
set -u

payload="$(cat)"
content="$(printf '%s' "$payload" | jq -r '[.tool_input.text, .tool_input.content, .tool_input.newStr, .tool_input.new_str, .tool_input.command] | map(select(type == "string")) | join("\n")' 2>/dev/null)"
[ -n "$content" ] || exit 0

if ! command -v gitleaks >/dev/null 2>&1; then
  echo "secret-scan: gitleaks is not installed, so the write could not be scanned. Install gitleaks and try again." >&2
  exit 2
fi

report="$(mktemp)"
trap 'rm -f "$report"' EXIT
if printf '%s' "$content" | gitleaks stdin --redact --no-banner --exit-code 1 --report-format json --report-path "$report" >/dev/null 2>&1; then
  exit 0
fi
rules="$(jq -r '[.[].RuleID] | unique | join(", ")' "$report" 2>/dev/null)"
echo "secret-scan: blocked a write that contains a secret (gitleaks rule: ${rules:-unknown}). Use an environment variable or a fake value instead." >&2
exit 2
