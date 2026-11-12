---
name: devin-worktrees
description: Give every Devin session its own git worktree under `.devin/worktrees/`, the Devin counterpart of Claude Code's `.claude/worktrees/`, and know how the two map. Use whenever a Devin session gets a WORKTREE WARNING or WORKTREE ERROR at its start, you start a Devin session in a worktree (`devin-wt`), work in a worktree you weren't started in, rename a worktree's branch for a PR, start a subagent, clean up worktrees, or reason about Claude Code and Devin worktrees side by side.
---

# Devin worktrees

Every agent works in a git worktree of its own, so parallel agents never edit the same files (the
`managing-agents` skill). Claude Code does this itself; for Devin, the `SessionStart` hook in
`.devin/hooks.v1.json` runs `.devin/hooks/session_worktree.py`, which does the same in the same shape.

## Claude Code and Devin side by side

| | Claude Code | Devin |
| --- | --- | --- |
| Worktrees folder | `.claude/worktrees/`, ignored by `.claude/.gitignore` | `.devin/worktrees/`, ignored by `.devin/.gitignore` |
| Named session | `claude --worktree <name>`: `<name>/` on branch `worktree-<name>` | `devin-wt <name>` (below): `<name>/` on branch `worktree-<name>` |
| One worktree per agent | a subagent with `isolation: worktree`: `agent-<id>/` on branch `worktree-agent-<id>` | every session not in a free worktree: `agent-<session_id>/` on branch `worktree-agent-<session_id>` |
| The id | Claude's agent id, `a` and 16 hex digits | Devin's session id, such as `balanced-starburst` (`devin list`) |
| Base | `origin/HEAD` (`main` here), unless `worktree.baseRef` says otherwise | `origin/staging/user-<login>` (`gh api user -q .login`), whatever branch the main checkout is on, since PRs go there (the `merging-to-staging` skill) |
| Where the agent runs | inside the worktree | inside it with `devin-wt`; otherwise the hook can't move the session, so the agent works there by path |
| Cleanup | removes a subagent worktree with no changes | `SessionEnd` removes an `agent-` worktree that is clean and whose `HEAD` is already in `origin/staging/user-<login>` |
| Branch for a PR | renamed to `claude/<topic>` | renamed to `devin/<topic>` |

Treat a worktree in either folder as another agent's: never edit, commit in or remove it unless the
user asks, or it belongs to a subagent you started (below).

## Why names never collide

- **One name per session:** the `agent-` folder and branch come from the session id, which Devin makes
  unique, and `git worktree add -b` refuses an existing folder or branch, so two sessions never share one
  silently. A resumed session finds its own worktree again by that name.
- **One session per worktree:** a session claims a worktree with a `devin-session-id` file in the
  worktree's git folder (`.git/worktrees/<name>/`), created only if absent, so of two sessions that start
  in one worktree at once exactly one wins, and the other gets an `agent-` worktree of its own.
- **Stale claims:** `SessionEnd` releases a session's claims. A session that crashed keeps its claim; delete
  that file only after `devin list` shows the session is gone and the user agrees.

## WORKTREE WARNING

The session started in the main checkout, or in a worktree another session holds, and the hook gave it
`.devin/worktrees/agent-<session_id>/`.

1. Tell the user at the start of your first response, in one sentence.
2. Work only in that worktree: pass it as `workdir` to every shell command, use absolute paths under it for
   every file you read, edit or write, and change nothing in the folder the session started in.
3. Run the root `AGENTS.md`'s check for changes from others in the worktree. `refs/worktree/last-seen-staging`
   belongs to one worktree, so a new worktree starts tracking as the `compatible-changes` skill says.

## WORKTREE ERROR

The hook couldn't give the session a worktree. Tell the user the error and change no files until the user says
where to work. Common causes:

- `gh` isn't logged in, so the hook can't find the user's login.
- `origin/staging/user-<login>` doesn't exist (the `merging-to-staging` skill says how to create it).
- `git fetch` failed: offline, a git lock held by another agent, or a stale `origin/staging` ref (fix it as the
  `merging-to-staging` skill says).
- A stray folder sits at `.devin/worktrees/agent-<session_id>/`.

To retry after a fix, rerun the hook from the folder the session started in:

```bash
echo '{"hook_event_name": "SessionStart", "session_id": "<session_id>"}' \
  | DEVIN_PROJECT_DIR="$PWD" python3 -I .devin/hooks/session_worktree.py
```

## Starting a session in a worktree

In the Devin CLI, `devin-wt <name> [devin args]` works like `claude --worktree <name>`: it creates
`.devin/worktrees/<name>/` from `origin/staging/user-<login>`, or reopens it, and starts Devin inside it,
where the hook just claims it. Add it to `~/.zshrc` or `~/.bashrc`; don't pick a name starting with `agent-`,
which the hook uses.

```bash
devin-wt() {
  local name="${1:?usage: devin-wt <name> [devin args]}"; shift
  local main worktree base
  main="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")" || return
  worktree="$main/.devin/worktrees/$name"
  if [ ! -d "$worktree" ]; then
    base="staging/user-$(gh api user -q .login)" &&
      git -C "$main" fetch -q origin "$base" &&
      git -C "$main" worktree add -q --no-track -b "worktree-$name" "$worktree" "origin/$base" || return
  fi
  cd "$worktree" && devin "$@"
}
```

In Devin Desktop, open `.devin/worktrees/<name>/` as the workspace to get the same.

## Subagents

A Devin subagent gets no worktree of its own: it starts in the main checkout, the `SessionStart` hook doesn't run for
it, and it doesn't inherit its parent's worktree (seen in Devin Desktop, 2026-11-08). So the parent gives it one,
and runs its git. What every parent owes its subagents (depth, merge plan, outputs) is in the `managing-agents` skill.

- **Every subagent gets a worktree**, including one that only reads, created before the subagent starts.
- **Names:** `.devin/worktrees/sub-<parent-folder>-<n>/` on branch `worktree-sub-<parent-folder>-<n>`, where `<n>` is
  the next number `git worktree add -b` accepts (e.g. `sub-agent-blue-general-1`). The name doesn't start with
  `agent-`, so `SessionEnd` never removes it. Before the first push, rename the branch to `devin/<topic>`.
- **Base:** create it from `origin/staging/user-<login>`:
  `git worktree add -q --no-track -b worktree-sub-<parent-folder>-<n> <folder> origin/staging/user-<login>`.
  When its work builds on the parent's, move it to the parent's branch with `git -C <folder> merge --ff-only
  <parent-branch>`. The subagent's task says which base it got.
- **Its task** gives the worktree's absolute path and branch, and tells the subagent to:
  - pass that path as `workdir` to every command and use absolute paths under it for every file;
  - stop and report if `git rev-parse --show-toplevel` doesn't print that path;
  - change nothing outside it (not the main checkout, not the parent's worktree);
  - edit files and run only read-only git commands.
- **Git:** the parent reviews the subagent's changes, then commits and pushes them from the subagent's worktree after
  each step.
- **Cleanup:** when the subagent is done, the parent checks its worktree (branch, last commit, pushed or not,
  uncommitted files) and tells the user what it left. Before removing it, the parent copies every run folder the user
  reviewed into the same component's `.artifacts/` in its own worktree, because `git worktree remove` deletes
  git-ignored files without asking. Then it removes the worktree and branch as below.

## Branches and cleanup

- **Before the first push**, rename the branch to `devin/<topic>` (`git branch -m devin/<topic>`). The
  folder keeps its name, so a resumed session still finds it.
- **Listing:** `git worktree list` shows every worktree with its branch.
- **Removing:** only with the user's OK, and only once the work is merged or pushed: `git worktree remove
  <path>` (never `--force` without asking), then `git branch -d <branch>`.
