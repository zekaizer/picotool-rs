#!/usr/bin/env bash
# PostToolUse(Edit|Write): format the crate after a Rust file is edited.
# Formatting only — type-checking, clippy, and tests run at the commit/push git hooks.
set -uo pipefail

input=$(cat)
file=$(printf '%s' "$input" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)

case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac

cd "${CLAUDE_PROJECT_DIR:-$PWD}" || exit 0
cargo fmt --quiet 2>/dev/null || true
exit 0
