---
name: disk-space
description: Keep the disk from filling up when many agents work in parallel worktrees - what takes the space (Rust build output first), what each agent cleans when it finishes, how the lead tells which worktrees are safe to remove, and how to remove them without losing work. Use whenever you start or finish an agent's work, merge a PR whose agent had its own worktree, see free space under 20 GB, the user asks about disk space or cleanup, or you clean up worktrees, build folders or run outputs.
---

# Disk space

Parallel agents each work in a worktree of their own (`managing-agents`), and each worktree builds its own copy of
every crate it touches. On 2026-11-08 fifteen worktrees reached about 125 GB and left 6 GB free on the user's Mac.

## What takes the space

Measured on one worktree that built six Rust crates (18 GB):

| What | Size | Notes |
| --- | --- | --- |
| A crate's debug build (`.build/debug/`) | about 2 GB | 1.3 GB compiled code with debug info, 0.5 GB incremental caches |
| A crate's release build (`.build/release/`) | about 0.8 GB | |
| Run outputs (`.artifacts/`) | up to 6 GB a worktree | raw frame dumps and intermediate renders |
| Source code | about 1.4 MB a crate | |
| Textures and models | tens of MB a copy | |

So a worktree costs about 3 GB for every Rust crate it builds. Build output is never "outdated", just disposable: it
can always be rebuilt. The waste is keeping it in worktrees where nobody will build again.

## Every agent

- **Build only what you change** or must check, not every crate.
- **Clean before you report:** run `cargo clean` in every crate you built (or `make clean`), and remove extra build
  folders such as a `.build/before/` kept for comparisons.
- **Keep `.artifacts/` small:** once the PR's sheets and tables are made, delete raw frame dumps and intermediate
  renders. Keep what the PR or its DECISIONS cite.
- **Watch free space:** if `df -h /Users` shows under 20 GB free, stop building and tell the lead or the user.

## The lead

- **At merge:** once an agent's PR is merged and the agent has stopped, remove its worktree (below).
- **When space is short, or every few hours of parallel work:** list every worktree with
  `git worktree list --porcelain`, and for each one its size (`du -sh`), uncommitted changes
  (`git -C <worktree> status --porcelain`), unpushed commits (`git -C <worktree> rev-list --count HEAD --not --remotes`)
  and its PR's state (`gh pr list --head <branch> --state all`).

## Safe to remove

- **Build output (`.build/`):** always safe to delete in a worktree no agent is building in (root `AGENTS.md`:
  "Recreated by rebuilding").
- **A worktree:** safe to remove (`git worktree remove <path>`) when all four hold:
  1. it has no uncommitted changes;
  2. its branch has no unpushed commits;
  3. its PR is merged or closed, or its work is otherwise done;
  4. no running session or agent uses it.
  The branch stays, so the worktree can be made again from it.
- **Uncommitted changes:** look at them first. If they are worth anything, save them as a patch in the scratchpad
  (`git diff > <scratchpad>/<branch>-uncommitted.patch`) before `git worktree remove --force`.
- **Not obvious, so ask:**
  - Worktrees of other sessions (other `.claude/worktrees/*` and `.devin/worktrees/*` folders not started by you):
    git can't tell whether the user still needs them. Ask the user which sessions to keep; a session id maps to its
    worktree through the folder name of its transcript under `~/.claude/projects/` (list names only, don't read them).
  - `.artifacts/`: pictures are usually attached to their PR on GitHub, but not always, and a timing run may be the
    only copy. Delete only what you made or what its PR already holds.
- **Never:** the user's own checkout, a worktree with work you haven't looked at, or `.artifacts/` of a running agent.

## Proposed, not built yet

- **Smaller debug builds:** `[profile.dev] debug = "line-tables-only"` (and perhaps `incremental = false`) in each
  crate's `.cargo/config.toml`; debug info and incremental caches are most of a debug build. Measure before adopting.
- **A cleanup script** (`clis` skill): lists every worktree with size, state, PR and session, `--dry-run` by default,
  and removes safe ones only with `--yes-to-remove-worktrees` (and build output with `--yes-to-delete-builds`).
- **A shared build folder per crate** across worktrees would save the most space, but Cargo lets one build at a time
  use it, so parallel agents would wait for each other.
