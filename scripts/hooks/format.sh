#!/usr/bin/env bash
# PostFileSave hook (foundation Requirements 22.4, 22.5, 22.16): formats one saved file.
# Reads the hook JSON from stdin, resolves the path, and formats it only when it is a regular
# file inside this repo with a known extension. The path is passed after `--`, never into a shell.
set -u

root="$(cd "$(dirname "$0")/../.." && pwd -P)"
payload="$(cat)"
raw="$(printf '%s' "$payload" | jq -r '.tool_input.path // .tool_input.file_path // .file_path // .path // empty' 2>/dev/null)"
[ -n "$raw" ] || exit 0

case "$raw" in
  /*) candidate="$raw" ;;
  *) candidate="$root/$raw" ;;
esac
[ -f "$candidate" ] || exit 0
path="$(realpath "$candidate" 2>/dev/null)" || exit 0

case "$path" in
  "$root"/*) ;;
  *) exit 0 ;;
esac
case "$path" in
  "$root"/node_modules/* | "$root"/*/node_modules/* | "$root"/target/* | "$root"/.scratch/* | "$root"/.kiro/* | "$root"/fixtures/* | "$root"/packages/schema/generated/* | "$root"/docs/evidence/runs/*) exit 0 ;;
esac

cd "$root" || exit 0 # Prettier reads .prettierignore from the working folder
case "$path" in
  *.rs)
    command -v rustfmt >/dev/null 2>&1 || exit 0
    rustfmt --edition 2024 -- "$path"
    ;;
  *.ts | *.tsx | *.css | *.json | *.md)
    [ -x "$root/node_modules/.bin/prettier" ] || exit 0
    "$root/node_modules/.bin/prettier" --log-level warn --write -- "$path"
    ;;
  *) exit 0 ;;
esac
exit 0
