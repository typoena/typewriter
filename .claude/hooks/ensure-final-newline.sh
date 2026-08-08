#!/usr/bin/env bash
# PostToolUse(Write|Edit): keep every text file ending in exactly one newline.
# grep -Iq . fails on binary files, so renders/gerbers/images are left alone.
set -uo pipefail

f=$(jq -r '.tool_input.file_path // empty')
[ -n "$f" ] && [ -f "$f" ] && [ -s "$f" ] || exit 0
grep -Iq . "$f" || exit 0
[ -n "$(tail -c1 "$f")" ] && printf '\n' >>"$f"
exit 0
