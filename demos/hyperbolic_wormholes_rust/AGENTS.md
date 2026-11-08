These rules apply to `demos/hyperbolic_wormholes_rust/`, on top of the repository's root `AGENTS.md`. Codex and
Devin read them here; Claude Code reads them through `CLAUDE.md` beside this file.

## The user's wishlist

[`USER_WISHLIST_CHECKLIST.md`](USER_WISHLIST_CHECKLIST.md) lists every feature and fix the user has asked for in
this demo, so agents on different platforms can share the work without doing it twice or forgetting a request.
Each item has an id (`W-NN`) and a status: `[ ]` open, `[~]` claimed, `[?]` pending user review, `[x]` done (the
user confirmed it).

- **Always name an id with its short name.** Wherever you write an id, write the item's short name beside it, the bold
  title in the checklist: `W-85 (stuck leaving a bunker)`, never a bare `W-85`. This holds in commit messages, PR
  titles and descriptions, decisions, docs, the checklist's own cross-references, and every message to the user or to
  other agents, so the user never has to remember what a number means while tens or hundreds of items are in flight.
  For several ids, name each one: `wishlist: claim W-37 (stronger curvature) and W-64 (zone planes)`.
- **Read it first.** Before starting work on this demo, read the checklist on the latest `staging` (`git fetch origin
  staging`). Work only on items that are open, or that you have claimed.
- **Add requests at once.** As soon as the user asks for something new, add it under "Open" with the next id, in the
  user's words (fixing only spelling), in its own `wishlist: add W-NN (<short name>)` PR into `staging` that you merge
  at once, even while doing other work; then do it when its turn comes. Ask the user before splitting or merging
  items.
- **Claim before starting.** Change the item's `[ ]` to `[~]`, move it under "Claimed", and add who and where:
  `Claimed by <agent> (session <session or task id>), branch <branch>, <YYYY-MM-DD>.` Under it, add a `**Where the
  work is**` sub-bullet that lets any other agent (Claude Code, Codex, Devin) pick the work up if you stop: the
  worktree's full path and machine, the branch and whether it is pushed, where pictures and other outputs are, and the
  state of the work with its date. Keep it current while you work, and push your branch to `origin` whenever it has
  commits worth sharing, so the work survives your session. Put only that change in a PR into `staging` titled
  `wishlist: claim W-NN (<short name>)` and merge it at once (agents may merge into `staging`), so other agents see
  the claim before the work starts.
- **Mark it pending user review, never done.** In the PR that delivers the item, change `[~]` to `[?]`, move it under
  "Pending user review", and replace the claim with the PR's number (`#NNN`; add it once the PR is open, before
  merging). Tell the user it is on `staging` to test, with screenshots of anything visual. A fix for an item adds its
  PR number to that item and keeps it pending.
- **Only the user's confirmation makes it done.** The user tests every claimed fix and every visual before it counts
  as finished. When the user says, in chat, that they tested an item and it works, change `[?]` to `[x]`, move it
  under "Done" and add `Confirmed by the user, <YYYY-MM-DD>.`, in a `wishlist: W-NN (<short name>) confirmed` PR
  merged at once. Never mark an item `[x]` on your own judgement, from tests or screenshots, or from another agent's
  report. If the user reports a problem, keep the item `[?]`, add their words to it, and fix it.
- **Stale claims.** A claim with no open PR for two days may be taken over. Change the claim to yours and say whose it
  was in the PR description.
- **Unclaim.** If you stop working on an item, change it back to `[ ]` under "Open" in a `wishlist: unclaim W-NN
  (<short name>)` PR.

## Credits for assets

The game's CREDITS page (the `Tab` menu, text in [`src/credits.rs`](src/credits.rs)) says where every asset comes from.

- **Add credits with the asset.** Any change that adds an asset to the game (a texture, model, sound, font, picture or
  data from outside this repository) adds it to `CREDITS_LINES` in the same PR: what it is, its author, where it came
  from (a link) and its license. Follow any attribution the license requires (CC BY needs the author's name).
- **Remove credits with the asset.** A change that removes an asset removes its credit.
- **Fit the menu.** Each line fits 64 characters; `cargo test` checks it.
