#!/usr/bin/env bash
# PostToolUse(Edit|Write): when a Rust file is edited, format the crate and run clippy.
# clippy failures are surfaced back to Claude (exit 2) so they get fixed in the same turn.
set -uo pipefail

input=$(cat)
file=$(printf '%s' "$input" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)

case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac

dir="${CLAUDE_PROJECT_DIR:-$PWD}"
cd "$dir" || exit 0

cargo fmt --quiet 2>/dev/null || true

if ! out=$(cargo clippy --all-targets --quiet -- -D warnings 2>&1); then
  printf 'clippy failed after editing %s:\n%s\n' "$file" "$out" >&2
  exit 2
fi
exit 0
