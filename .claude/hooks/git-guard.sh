#!/usr/bin/env bash
# PreToolUse(Bash): enforce the CLAUDE.md workflow gate.
# Pushes always need explicit user confirmation; so do direct commits/merges on main.
set -uo pipefail

input=$(cat)
dir="${CLAUDE_PROJECT_DIR:-$PWD}"
branch=$(git -C "$dir" rev-parse --abbrev-ref HEAD 2>/dev/null || true)

if printf '%s' "$input" | grep -Eq 'git[[:space:]]+push'; then
  echo "BLOCKED: 'git push' needs explicit user confirmation (CLAUDE.md workflow). Ask the user first." >&2
  exit 2
fi

if [ "${branch:-}" = "main" ] && printf '%s' "$input" | grep -Eq 'git[[:space:]]+(commit|merge)'; then
  echo "BLOCKED: committing or merging directly on main needs explicit user confirmation (CLAUDE.md workflow)." >&2
  exit 2
fi
exit 0
