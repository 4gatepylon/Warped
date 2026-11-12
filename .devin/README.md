# .devin

Devin's project settings, the counterpart of `.claude/` for Claude Code.

- `hooks.v1.json`: runs `hooks/session_worktree.py` when a Devin session starts and ends, so every session
  works in a git worktree of its own (the `devin-worktrees` skill).
- `worktrees/` (git-ignored): the worktrees, written by `hooks/session_worktree.py` (`agent-<session_id>/`)
  and by the `devin-wt` shell function (`<name>/`).
