---
name: merging-to-staging
description: Bring a branch up to date with `staging`, resolve the conflicts that come up again and again in this repo (the wishlist checklist, DECISIONS.md numbers, shared Rust and Metal structs, network bits), check it, and merge its PR into `staging`. Use whenever you merge `staging` into a branch, resolve a merge conflict, or merge a PR into `staging`, especially while other agents are merging too.
---

# Merging to staging

Many agents merge into `staging` at once, so `staging` moves while you work, and the same few files conflict every
time. PRs go into `staging` only; never into `main` (the `pr-description` skill).

## The loop

1. **Fetch.** `git fetch origin staging`. If it fails with "cannot lock ref" (another session fetching at the same
   moment), wait a few seconds and fetch again.
2. **Merge `staging` in.** `git merge origin/staging` into your branch. Don't rebase a branch you have pushed.
3. **Resolve conflicts** with the recipes below, keeping everyone's work.
4. **Check:**
   - the build passes, `fmt` is clean, the linter gives no warnings and every test passes, all on the merged tree;
   - if rendering changed, one screenshot is rechecked (`graphical-changes`);
   - for a game, one short window run joined to your own server.
5. **Push, then merge the PR.** `gh pr merge <n> --merge`.
   - "not mergeable" or new conflicts mean `staging` moved: go back to step 1.
   - "UNKNOWN" mergeability means GitHub is still computing it: wait a few seconds and ask again.
6. **Report:** the merge commit, and pictures of anything visual.

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

- **Fresh branches:** cut each new branch from `origin/staging`: `git checkout -b <branch> origin/staging`.
- **Checklist-only changes** (add, claim, confirm) go in their own small PR, merged at once.
- **Two steps for the PR number:** to mark your item pending with its PR number, open the PR first, then commit the
  number in a second commit.
- **Someone else's linter warning:** a warning that comes in with `staging` gets fixed in its own small PR, not mixed
  into yours.
- **Shell habits:** start each shell command by changing into the folder you mean, since the working folder can reset
  between commands. Never run a bare `git stash`: the stash is shared with other worktrees.
