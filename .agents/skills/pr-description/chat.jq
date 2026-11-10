# `chat.jq` prints an agent transcript's chat log and tool calls as plain text.
#
# Interface:
#     input: A Claude Code transcript, one JSON object per line (`<trace id>.jsonl`).
#     output: One `[user]` or `[assistant]` entry per message, and one `[tool]` line per tool call.
#
# Example:
#     $ jq -rf .agents/skills/pr-description/chat.jq ~/.claude/projects/<project>/<trace id>.jsonl | less
#
# Must know:
#     Limitations:
#         - `chat.jq` reads Claude Code transcripts only; other agents' transcripts have other shapes.
#         - `chat.jq` cuts each tool call's input to `MAX_TOOL_INPUT_CHARS` and leaves tool results out.

def MAX_TOOL_INPUT_CHARS: 200;

select(.type == "user" or .type == "assistant")
| .type as $who
| .message.content
| if type == "string" then "[\($who)] \(.)"
  else .[]
    | if .type == "text" then "[\($who)] \(.text)"
      elif .type == "tool_use" then "[tool] \(.name): \(.input | tostring | .[0:MAX_TOOL_INPUT_CHARS])"
      else empty end
  end
