These rules apply to `demos/hyperbolic_wormholes_rust/`, on top of the repository's root `AGENTS.md`. Codex and
Devin read them here; Claude Code reads them through `CLAUDE.md` beside this file.

## The user's wishlist

[`USER_WISHLIST_CHECKLIST.md`](USER_WISHLIST_CHECKLIST.md) lists every feature and fix the user has asked for in
this demo, so agents on different platforms can share the work without doing it twice or forgetting a request.
Each item has an id (`W-NN`) and a status: `[ ]` open, `[~]` claimed, `[x]` done.

- **Read it first.** Before starting work on this demo, read the checklist on the latest `staging`
  (`git fetch origin staging`). Work only on items that are open, or that you have claimed.
- **Add requests at once.** As soon as the user asks for something new, add it under "Open" with the next id, in
  the user's words (fixing only spelling), in its own `wishlist: add W-NN` PR into `staging` that you merge at once,
  even while doing other work; then do it when its turn comes. Ask the user before splitting or merging items.
- **Claim before starting.** Change the item's `[ ]` to `[~]`, move it under "Claimed", and add who and where:
  `Claimed by <agent> (session <session or task id>), branch <branch>, <YYYY-MM-DD>.` Put only that change in a PR
  into `staging` titled `wishlist: claim W-NN` and merge it at once (agents may merge into `staging`), so other agents
  see the claim before the work starts.
- **Mark it done.** In the PR that delivers the item, change `[~]` to `[x]`, move it under "Done", and replace the
  claim with the PR's number (`#NNN`; add it once the PR is open, before merging). A fix for an item already done
  adds its PR number to that item.
- **Stale claims.** A claim with no open PR for two days may be taken over. Change the claim to yours and say whose
  it was in the PR description.
- **Unclaim.** If you stop working on an item, change it back to `[ ]` under "Open" in a `wishlist: unclaim W-NN` PR.
