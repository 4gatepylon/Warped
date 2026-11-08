---
name: stacked-prs
description: Split a change into stacked pull requests and manage the stack with the `gh stack` CLI. Use whenever a change splits into a sequence of dependent pieces, the user asks for a stacked PR or a stack of PRs, or you need to add a layer, change a lower layer after review, rebase, push, merge or sync a stack.
---

# Stacked pull requests

- When a change splits into a sequence of dependent pieces, make one PR per logical piece and stack
  them. Each piece's branch is built on top of the previous piece's branch, and each PR's base is
  the branch below it, so every PR shows only its own piece. Each layer should build and pass its
  own tests.
- Before building a stack, tell the user the planned layers and how they will be stacked, and wait
  for their agreement.
- By default, stack with GitHub's native stacked PRs, using the `gh stack` CLI extension
  (`gh extension install github/gh-stack`). It needs Git 2.36 or later, and stacked PRs must be
  enabled for the repository.
  - `gh stack` switches branches, so run it only in a worktree of your own, NEVER in the user's
    checkout (the `managing-agents` skill).
  - `gh stack init --base staging/user-<username> <branch>` starts the stack on your user's
    staging branch (see the `merging-to-staging` skill) with its first branch. Commit the first piece there.
    If `<branch>` already exists (e.g. the branch of the worktree you are in), `gh stack init`
    adopts it instead of creating a new one.
  - `gh stack add <branch>` adds a branch on top of the stack. Commit the next piece there, and
    repeat for each piece.
  - `gh stack submit` pushes every branch and opens the linked PRs. Then give each layer with a
    graphical change its screenshots and videos with `gh pr edit <n> --body-file body.md --attach
    <file>` (the `pr-description` skill).
  - To change a lower piece (e.g. after review), switch to its branch with
    `gh stack checkout <branch>` or `gh stack down`, and commit there rather than in a higher layer.
    Then run `gh stack rebase --upstack` and `gh stack push`.
  - PRs merge bottom-up into `staging/user-<username>`, with `gh stack merge` or from the merge
    box on GitHub; agents may do this themselves. Afterwards, `gh stack sync --prune` rebases the
    remaining branches onto it and removes merged ones. Agents merge into `staging/global` or
    `main` only with the user's explicit permission for that merge.
- If `gh stack` can't be used (the extension is missing, Git is older than 2.36, or it exits with
  code 9 because stacked PRs aren't enabled), tell the user and ask before falling back to stacking
  by hand: open the PRs with `gh pr create --base <branch below>`, then link them into a stack on
  GitHub.
