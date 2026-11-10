#!/usr/bin/env bash
# Prints the AGENT TRACES section for the current desktop Claude Code session. Usage: trace_block.sh "<contribution>"
set -euo pipefail
workspace=$(git rev-parse --show-toplevel)
transcript=~/.claude/projects/$(echo "$workspace" | tr '/.' '--')/$CLAUDE_CODE_SESSION_ID.jsonl
test -f "$transcript" || { echo "no transcript at $transcript" >&2; exit 1; }
version=${AI_AGENT#claude-code_}; version=${version%_agent}; version=${version//-/.}
transcript_shown=$(echo "${transcript/#$HOME/~}" | sed "s|-$USER-|-\$USER-|")
printf 'AGENT TRACES\n\n* Claude Code %s, trace %s, desktop\n  contributed: %s\n  workspace: %s, branch %s\n  transcript: %s\n  read it: jq -rf .agents/skills/pr-description/chat.jq %s | less\n' \
  "$version" "$CLAUDE_CODE_SESSION_ID" "$1" "${workspace/#$HOME/~}" "$(git branch --show-current)" \
  "$transcript_shown" "$transcript_shown"
