# Agent traces

Every commit says which agents produced it and where each one's transcript is, so the user can read how a
change came about, from the commit alone. The section sits below the commit's body and history, and above
its trailers ([`commit-fields.md`](commit-fields.md)). A PR lists the agents of all its commits in its
`## Agents` section, in the same form.

## The section

```text
AGENT TRACES

* <agent> <version>, trace <trace id>, <desktop|cloud>
  contributed: <what this agent did in this commit>
  workspace: <workspace>, branch <branch>
  transcript: <path or URL>
  read it: <command>
```

- One bullet per agent whose work the commit contains, in the order they worked. An agent with no trace
  of its own (a subagent that has no separate transcript) counts as part of the agent that started it.
- `<agent> <version>` is the agent type and its version, such as `Claude Code 2.1.321`.
- `<trace id>` is the agent's session or trace id.
- `desktop` means the agent ran on a user's computer, in any app. `cloud` means it ran on a remote
  machine.
- `contributed` is one sentence about this commit, not the agent's whole session.
- `workspace` is the worktree's folder, and `branch` the branch the commit is on. On a computer, a path is
  written without user-specific parts: the home folder is `~`, and the user name is `$USER`.
- `transcript` is the file that holds the agent's chat log and tool calls, or the session's URL.
- `read it` is a command the user can paste to read the agent's chat log and tool calls. It fails at once when the
  transcript can't be read. For a transcript file it is the `chat.jq` filter (see "Reading a transcript").

```text
AGENT TRACES

* Claude Code 2.1.321, trace 00000000-0000-4000-8000-000000000000, desktop
  contributed: wrote the commit-fields rules and the glossary
  workspace: ~/git/Warped/.claude/worktrees/example, branch claude/example
  transcript: ~/.claude/projects/-Users-$USER-git-Warped--claude-worktrees-example/00000000-0000-4000-8000-000000000000.jsonl
  read it: jq -rf .agents/skills/pr-description/chat.jq ~/.claude/projects/-Users-$USER-git-Warped--claude-worktrees-example/00000000-0000-4000-8000-000000000000.jsonl | less
```

## Where each agent keeps its trace

| Agent | Trace id | Transcript | Read it |
| --- | --- | --- | --- |
| Claude Code, desktop app | `$CLAUDE_CODE_SESSION_ID`; `$CLAUDE_CODE_ENTRYPOINT` is `claude-desktop` | `~/.claude/projects/<workspace path with every / and . replaced by ->/<trace id>.jsonl` | `jq -rf .agents/skills/pr-description/chat.jq <transcript> \| less` |
| Claude Code, cloud | the id in the session's URL | the session's URL | open the URL |
| Codex | the session id that `codex resume` lists | `~/.codex/sessions/<year>/<month>/<day>/rollout-<time>-<trace id>.jsonl` | `codex resume <trace id>` |
| Devin | the id in the `Link to Devin session` URL | that URL | open the URL |

Checked on a desktop Claude Code session on 2026-11-10: the first row's variables and file exist, and its command
printed that session's chat. The other rows come from the agents' documented behavior and were not checked on this
machine. The first agent of that type that commits fixes its row if it is wrong.

## Reading a transcript

[`chat.jq`](chat.jq) is a `jq` filter that prints a Claude Code transcript as plain text: each `[user]` and
`[assistant]` message, and one `[tool]` line per tool call. Run it from the repository's root, with the path the
commit gives:

```bash
jq -rf .agents/skills/pr-description/chat.jq <transcript> | less
```

- It fails at once, with `Could not open file` and exit code 2, when the transcript isn't on this computer: it is
  on the machine named by the commit's workspace, or it was deleted.
- It needs `jq`. It reads only; `claude --resume <trace id>`, run in the workspace, opens the session itself, and
  what you type there continues it.
- A cloud agent's `read it` is its session's URL, which only the session's owner can open.

## Writing it: the command

A desktop Claude Code agent prints its entry's fields with this command, which fails when the transcript is
missing:

```bash
workspace=$(git rev-parse --show-toplevel)
transcript=~/.claude/projects/$(echo "$workspace" | tr '/.' '--')/$CLAUDE_CODE_SESSION_ID.jsonl
test -f "$transcript" || { echo "no transcript at $transcript" >&2; exit 1; }
echo "workspace: ${workspace/#$HOME/~}, branch $(git branch --show-current)"
transcript_shown=$(echo "${transcript/#$HOME/~}" | sed "s|-$USER-|-\$USER-|")
echo "transcript: $transcript_shown"
echo "read it: jq -rf .agents/skills/pr-description/chat.jq $transcript_shown | less"
```

## Failing fast

An agent that can't fill every field of its bullet with a real value (no trace id, a transcript that doesn't
exist, an unknown place) does not commit. It tells the user which field is missing and asks how to find it.
The section names transcripts and never copies them: the chat logs stay on the machine or in the cloud
service that holds them, and only the user can open them.
