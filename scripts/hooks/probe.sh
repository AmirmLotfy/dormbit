#!/bin/sh
# Task 4 probe hook: record the JSON payload Kiro sends on stdin for one
# trigger, so real hooks can rely on confirmed field names.
# Usage (from a hook): sh scripts/hooks/probe.sh <TriggerName>
# Writes to .scratch/hook-probe/ (gitignored). Always exits 0 and never blocks.
trigger="${1:-unknown}"
root="$(cd "$(dirname "$0")/../.." 2>/dev/null && pwd)"
out="$root/.scratch/hook-probe"
mkdir -p "$out" 2>/dev/null
ts="$(date +%Y%m%dT%H%M%S)-$$"
{
  echo "trigger=$trigger"
  echo "cwd=$(pwd)"
  echo "env_keys=$(env | cut -d= -f1 | grep -iE 'kiro|hook|user_prompt|file|tool' | tr '\n' ' ')"
  echo "--- stdin"
  head -c 20000
  echo
} > "$out/$trigger-$ts.txt" 2>/dev/null
exit 0
