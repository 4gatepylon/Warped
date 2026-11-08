These rules apply to `game/`, on top of the repository's root `AGENTS.md`. Codex and Devin read them here; Claude
Code reads them through `CLAUDE.md` beside this file.

## The refactor

- **Follow [`REFACTOR_PLAN.md`](REFACTOR_PLAN.md).** It says which step comes next, what each step may change, the
  tests that say it's done, and how agents share the PR stack (its section 9). If the plan is wrong, fix the plan
  in its own small PR first, then do the work.
- **Never delete or break the demo.** `demos/hyperbolic_wormholes_rust/` stays runnable as the way back. Copy from
  it; change the copies here.
- **Regression first.** Ported code must match the demo through `legacy-oracle` (plan, 7.3) before any change of
  behaviour, and a deliberate change says so in its PR and in `DECISIONS.md` (numbered `G1`, `G2`, ...).
- **Report what can't run.** On a machine without Metal (check with `make check-environment`), picture and frame-time
  tests are "Not run: no Metal device" in the PR, never passed.

## Working beside the infra stack

The infra stack (PRs #179 to #185, branches `claude/infra-*`) owns the title screen, the Mac app, the Linux server
bundle and `infra/` (plan, section 3.1). Don't change those, the server's flags, `online_games.txt`, the password
files or the app's folders without the infra stack's agreement on its PR. Authentication follows
[`common/protocol/authentication.md`](../common/protocol/authentication.md) exactly; changing it means changing that
page in the same PR.

## Until step A1

The wishlist, decisions and graphical-change rules still live in the demo's `AGENTS.md` and
`USER_WISHLIST_CHECKLIST.md`; step A1 copies them here, and from then on this folder's copies are the live ones.
