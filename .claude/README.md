# .claude

Claude Code's project settings, the counterpart of `.devin/` for Devin.

- `skills`: a symlink to `.agents/skills/` (the root `AGENTS.md`).
- `worktrees/` (git-ignored): the worktrees Claude Code writes for `claude --worktree <name>` (`<name>/`) and
  for subagents with `isolation: worktree` (`agent-<id>/`); the `devin-worktrees` skill maps them to Devin's.
