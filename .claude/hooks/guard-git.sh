#!/usr/bin/env bash
# tbdflow guard: redirect raw git workflow MUTATIONS to tbdflow.
#
# PreToolUse(Bash) hook. Blocks `git commit|push|merge|rebase` typed directly,
# steering the agent to `tbdflow` (which stages, lints, signs, and pushes
# consistently). Read-only git (status/log/diff/show/tag) is always allowed.
#
# Escape hatches: a command that already mentions `tbdflow`, or that contains the
# marker `# raw-git-ok`, is permitted. The hook FAILS OPEN (never blocks) if it
# cannot parse its input, so it can't wedge the agent.

set -u
input=$(cat)

# Extract tool_input.command. Prefer jq, then python3; otherwise fail open.
cmd=""
if command -v jq >/dev/null 2>&1; then
  cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || cmd=""
elif command -v python3 >/dev/null 2>&1; then
  cmd=$(printf '%s' "$input" | python3 -c \
    'import sys,json; print(json.load(sys.stdin).get("tool_input",{}).get("command",""))' \
    2>/dev/null) || cmd=""
fi
[ -z "$cmd" ] && exit 0

# Allow escapes.
case "$cmd" in
  *tbdflow*) exit 0 ;;
  *"# raw-git-ok"*) exit 0 ;;
esac

# Block raw git workflow mutations (commit/push/merge/rebase).
if printf '%s' "$cmd" \
  | grep -Eq '(^|[;&|]|[[:space:]])git[[:space:]]+(commit|push|merge|rebase)([[:space:]]|$)'; then
  echo "Blocked raw git: '$cmd'" >&2
  echo "Use tbdflow for workflow actions — it stages, lints, signs, and pushes:" >&2
  echo "  tbdflow --non-interactive --toon commit -t <type> -m \"<subject>\"" >&2
  echo "  tbdflow complete | sync | undo <sha>" >&2
  echo "If you truly need raw git here, append '# raw-git-ok' to the command." >&2
  exit 2
fi

exit 0
