---
name: merging-to-staging
description: Use the staging branches - `staging/user-<username>` for each user, `staging/global` for the latest version of the game, `main` for the stable one - bring a branch up to date, resolve the conflicts that come up again and again in this repo (the wishlist checklist, DECISIONS.md numbers, shared Rust and Metal structs, network bits), check it, and merge its PR. Use whenever you open or merge a PR, merge a staging branch into a branch, resolve a merge conflict, or are asked to merge into `staging/global` or `main`, especially while other agents are merging too.
---

# Merging to staging

## The branches

Work moves up through three levels of branches:

| Branch | What it is | Who merges into it |
| --- | --- | --- |
| `staging/user-<username>` | one user's newest work, theirs and their agents' | the user, or their agents on their own once the PR's tests pass |
| `staging/global` | officially the latest version of the game, where every user's work meets | a human, or an agent only with explicit permission |
| `main` | the stable version, which works | a human, or an agent only with explicit permission |

- **`<username>`** is the GitHub login of the user you work for, from `gh api user -q .login`: `staging/user-4gatepylon`.
- **PRs** go into your user's `staging/user-<username>`, never straight into `staging/global` or `main`. A stack's
  layer 1 is based on it (the `stacked-prs` skill).
- **Up to `staging/global`:** when the user's branch is ready, a PR from `staging/user-<username>` into
  `staging/global`, after the loop below has merged `staging/global` into the user's branch and checked it.
- **Up to `main`:** when `staging/global` is stable and works, a PR from `staging/global` into `main`.
- **Explicit permission:** an agent never merges into `staging/global` or `main` on its own, even when its task says
  to merge. Before such a merge it asks the user, naming the PR, the two branches and what goes in, and merges only
  after the user says yes to that merge. A yes covers that one merge, not later ones. Opening the PR needs no
  permission.
- **Missing user branch:** if `origin/staging/user-<username>` doesn't exist, create it from `staging/global` with
  `git push origin origin/staging/global:refs/heads/staging/user-<username>`, and tell the user.
- **Staying current:** merge `origin/staging/global` into the user's branch often, so other users' work arrives in
  small pieces.
- **No `staging` beside `staging/...`:** git stores a branch as a file named after it, so a branch `staging` and a
  branch `staging/global` can't both exist. If a fetch or a switch fails with "cannot lock ref ...
  'refs/remotes/origin/staging' exists" (expected once per clone after `staging` moved to `staging/global`,
  2026-11-08), run `git fetch --prune origin`, and rename a local `staging` with `git branch -m staging staging/global`.

## The loop

Many agents merge at once, so the staging branches move while you work, and the same few files conflict every time.
The PR's base is `staging/user-<username>` for a feature branch and `staging/global` for a user's branch.

1. **Fetch.** `git fetch origin staging/global staging/user-<username>`. If it fails with "cannot lock ref" (another
   session fetching at the same moment), wait a few seconds and fetch again.
2. **Merge the base in.** `git merge origin/<base>` into your branch. Don't rebase a branch you have pushed.
3. **Resolve conflicts** with the recipes below, keeping everyone's work.
4. **Check:**
   - the build passes, `fmt` is clean, the linter gives no warnings and every test passes, all on the merged tree;
   - if rendering changed, one screenshot is rechecked (`graphical-changes`);
   - for a game, one short window run joined to your own server.
5. **Tell the user about the game design.** Before merging, tell the user:
   - whether the PR changes the game's design (rules, controls, screens, menus, weapons, levels, maps);
   - whether the design docs describe the change once the PR is in: `game_design/INSIDE_GAME_DESIGN.md` for what
     happens inside a match, `game_design/OUTSIDE_GAME_DESIGN.md` for whatever manages how and when matches are
     played (accounts, matchmaking, rating), `game_design/WINDOW_DESIGN.md` for the app's windows and their
     settings;
   - anything else you know the design docs get wrong.

   If the PR changes the design but not the doc, update the doc in the same PR, or ask the user before merging
   without it.
6. **Push, then merge the PR.** `gh pr merge <n> --merge`, into `staging/user-<username>` on your own, into
   `staging/global` or `main` only after the user's yes for this merge.
   - "not mergeable" or new conflicts mean the base moved: go back to step 1.
   - "UNKNOWN" mergeability means GitHub is still computing it: wait a few seconds and ask again.
7. **Report:** the merge commit, and pictures of anything visual.

## Conflict recipes

- **The wishlist checklist** (`USER_WISHLIST_CHECKLIST.md`):
  - Each side usually moved different items: keep both sides' changes.
  - Each item appears exactly once, in the section of its newest status. Done beats pending, and pending beats
    claimed.
  - A claim that your PR delivers is removed. Other agents' claims stay as they are.
  - Check with `grep -c` that every id you touched appears once.
- **`DECISIONS.md`:**
  - Keep every decision, in number order.
  - If another agent took your number, renumber yours to the next free one, and fix every reference to it in code,
    docs and the PR.
- **Shared Rust and Metal structs** (uniforms, `OverlayParams`, frame data):
  - keep both sides' fields, new ones appended at the end;
  - the Rust and Metal layouts must stay identical, including size checks such as `static_assert`.
- **The network snapshot:**
  - two changes may have used the same spare bits (the shot-noise share and the thrown wormholes' tint both did);
  - give each its own bits, and check the tests that assert byte sizes and layouts.
- **Docs and design files:** keep both sides' bullets. Replace only a bullet your change makes untrue.
- **Generated files** (a map exported by a script): regenerate them rather than hand-merging, then check them with the
  map validator.

## Small things that save time

- **Fresh branches:** cut each new branch from the user's branch, in a worktree of its own, NEVER in the user's
  checkout (the `managing-agents` skill):
  `git worktree add -b <branch> .devin/worktrees/<name> origin/staging/user-<username>`.
- **Checklist-only changes** (add, claim, confirm) go in their own small PR into the user's branch, merged at once.
- **Two steps for the PR number:** to mark your item pending with its PR number, open the PR first, then commit the
  number in a second commit.
- **Someone else's linter warning:** a warning that comes in with `staging/global` gets fixed in its own small PR, not
  mixed into yours.
- **Shell habits:** start each shell command by changing into the folder you mean, since the working folder can reset
  between commands. Never run a bare `git stash`: the stash is shared with other worktrees.
