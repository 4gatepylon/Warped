# Online design and system prototype: handoff

State on 2026-11-10, when the session that wrote the outside-game design, the window mock-ups and part 1 of this
prototype paused. Read this first if you continue the work. It is written for an agent that has only this
repository: it repeats what was otherwise only in that session.

## What the user asked for, in order

1. **Design the mechanics outside a match first**, as documents the user approves or changes:
   `game_design/OUTSIDE_GAME_DESIGN.md` and `game_design/WINDOW_DESIGN.md`. Done, awaiting the user's review.
2. **A playable prototype** of those mechanics with a local backend, "the most realistic simulation of the actual
   system design", with every component visible. Part 1 of 7 is done; parts 2 to 7 are below.
3. **The system design** in `game_design/cloud/`, as static web pages: system diagrams, the database schema drawn,
   the API with its schemas, the protocols, plus a document mapping the design to Google Cloud. Not started, and
   **not to be started until the user has approved the mechanics** (step 1).

## Where everything is

- **Stack of 12 PRs** on `4gatepylon/Warped`, base `staging/user-4gatepylon`, bottom to top. Nothing is merged.

  | PR | Branch | What |
  | --- | --- | --- |
  | 557 | `claude/game-design-inside-rename` | `DESIGN.md` renamed to `INSIDE_GAME_DESIGN.md` |
  | 558 | `claude/outside-design-access` | Outside design: accounts, paying, caps, kinds of game |
  | 559 | `claude/outside-design-custom-games` | Roles, saved maps, custom games, warm-up, watching, replays |
  | 560 | `claude/outside-design-matchmaking` | Matchmaking |
  | 561 | `claude/outside-design-rating` | Rating, leaving, votes, rollbacks |
  | 562 | `claude/outside-design-fair-play` | Fair play |
  | 563 | `claude/outside-design-social` | Friends, clans, chat, profiles |
  | 564 | `claude/window-design-built` | `WINDOW_DESIGN.md` from the built windows |
  | 565 | `claude/window-design-outside` | Windows for the outside-game rules |
  | 567 | `claude/game-design-picture-folders` | Where each design doc's pictures go |
  | 577 | `claude/window-design-mockups` | The window mock-up website |
  | 586 | `claude/prototype-accounts` | Part 1 of this prototype (despite the branch's name) |

- **The earlier session's transcript** is on the user's Mac only:
  `~/.claude/projects/-Users-4gate-git-Warped--claude-worktrees-game-cloud-system-diagram-00f22d/cec760d8-0369-41df-9944-b4d5c675c974.jsonl`,
  and its worktree is `.claude/worktrees/game-cloud-system-diagram-00f22d` in the user's checkout. An agent in the
  cloud can't read either, which is why this file exists.
- **An early architecture draft** with Mermaid diagrams (the whole system, the OAuth sign-in step by step, a data
  model) is `game_design/cloud/EARLY_DRAFT.md`. It predates most decisions; use it as raw material only.

## Before you change anything

- Read the root `AGENTS.md` and the skills it names. The ones this work keeps hitting: `stacked-prs`,
  `pr-description` (its template, unchanged, on every PR), `merging-to-staging`, `graphical-changes`,
  `docstrings`, `unit-tests`, `makefiles`, `clis`.
- **Continue the stack:** check out `claude/prototype-accounts` and add each new part as a branch on top of it
  (`gh stack add <branch>`, then `gh stack submit --auto --open --remote origin`; the repository has two remotes,
  so `gh stack` needs `--remote origin`). If `gh stack` isn't available, ask the user before stacking by hand.
- **Size:** the user chose one PR per part for this prototype, each over the 200-line limit. Say so in the PR's
  Size section with the line count.
- **After each part,** send the user screenshots of one common flow before asking them to try it, and attach them
  to the PR (`gh pr edit <n> --body-file body.md --attach <file>`).
- **Each PR's Stack line** names the layers below and above. When you add a layer, update the line in every PR.
- Never merge into `staging/global` or `main`. Merging into `staging/user-4gatepylon` waits for the user's review of
  each PR here, because these are designs they want to approve.
- `AGENTS.md` must stay under 16 KiB. It is within a few bytes of that, so don't add to it.

## How this prototype is built

`README.md` says what it is and how to run it (`make run-prototype`, then `http://127.0.0.1:8010/`; `make test`;
`make fmt`). The rules that matter when adding to it:

- **Simulate each component of the real design separately.** Each is a group of methods of `World` (`world.py`), and
  whenever one component would call another, `World._send` records a row in the `message` table. Add new components
  to `COMPONENTS` in `world.py` and to `NODES` in `web/system/system.js`, so the system view draws them.
- **Rules are plain functions** in `rules.py`, with unit tests chosen by partitioning their inputs
  (`tests/test_rules.py`). `World` holds state and calls them.
- **Every number the design marks ≈ is a row of `DEFAULT_SETTINGS`** in `store.py`, editable live in the website's
  superadmin console.
- **Time is the simulated clock** (`clock.py`). Nothing reads real time. `World.tick` walks skipped time in steps.
- **The simulation controls** (the yellow band, `web/shared/sim_controls.js`, and routes under `/api/simulation/`)
  are the only things the real system won't have. Keep them visibly separate from product features.
- **The mock-ups become live window by window.** `web/game/index.html` (the app) and `web/site/index.html` (the
  website) are static HTML with made-up content; a part replaces a window's made-up content with data from the
  backend, as `web/site/site.js` does for the status page. `README.md` lists the `data-` attributes.
- **Code style that is easy to miss:** class methods are lowerCamelCase and free functions snake_case; every local
  variable declares its type; every function has a docstring with an Example; names carry units (`_s`, `_count`);
  a dictionary is read with `d[key]`; no comments other than docstrings. JavaScript follows the same naming.

## The parts still to build

Each part is one PR. Each should leave the system view showing its new components and messages.

2. **Accounts.**
   - A stand-in identity provider ("fake Google") as its own component with its own routes: an authorize page that
     lists made-up identities, a token route, and signed ID tokens. The game app page does the real flow:
     authorization code with PKCE, `state` and `nonce`, then the backend redeems the code, verifies the token and
     issues its own short-lived access token and a rotating refresh token (reuse of an old one revokes the family).
     `game_design/cloud/EARLY_DRAFT.md` has this flow as a sequence diagram.
   - Accounts keyed on (provider, subject). First sign-in asks for a birth date (13 or older, only the
     confirmation is kept), a unique display name (3 to 16 letters, digits and `_`; nothing that imitates staff;
     one change every 30 days) and an icon from a preset list. One session per account. Delete account.
   - A stand-in payment provider ("fake Stripe"): a checkout page and a signed webhook to the backend, safe to
     deliver twice. One purchase of $13.37 unlocks online play; a refund locks it again.
   - The free allowance: online play is free until 1 finished match or 30 minutes played, whichever comes first.
   - Make live: the app's sign-in, Home, purchase and Settings (account) windows; the simulation controls gain
     "act as" a made-up player and "refund".
3. **A stand-in 2D game**, so game servers have something real to run, record and show. Keep it trivial: a flat
   arena seen from above, WASD to move, click to shoot, one shot kills, respawn, first team to a score wins. The
   game server is authoritative and ticks it; bots play the other players; the user can play in the browser.
   - A stand-in object store ("S3 buckets") as a component: the game server uploads the recording in chunks
     during the match (states, not video), and the system view lists the buckets and objects.
   - Watching live with the admin's delay, and replays from the stored recording.
4. **Custom games:** saved maps with roles and visibility, creating and joining (list, code, password, whitelist,
   ban list), the lobby with its admin panel, warm-up, idle stops and checkpoints.
5. **Matchmaking:** parties of 1 to 5, the queue with its widening rating window, the split into two even teams,
   the ready check, the offer of bots, regions. The simulated players queue like real ones, and the "players
   online" dial drives them.
6. **Matches and rating:** OpenSkill behind an interface that can be swapped (the user agreed to adding the
   `openskill` package), a results outbox and a rating worker as components, placement, tiers, seasons, rankings
   (also on the website), match history, leaving and reconnecting, cooldowns, votes, voided matches, rollbacks.
   Simulation controls: pick a winner, make a player drop, abandon, go idle or cheat.
7. **Fair play and social:** reports, automatic suspension, audits run by a worker after the match, bans,
   appeals, the inbox, friends, clans, text chat (no voice), profiles, commendations, privacy settings.

Left out on purpose: the real game, voice chat, real Google sign-in, real Stripe.

## Decisions the user made that are not obvious from the documents

- Ranked play is matchmade only; a superadmin can also start ranked "tournament games" by hand. Custom games are
  unranked. One ranked queue at launch.
- Leaving follows Counter-Strike: the first on a team to abandon takes a loss and a cooldown; teammates may then
  leave freely; a bot replaces whoever abandoned. A reconnect window comes first.
- All cheat audits run after the match, requested at random and for reported players; two failed audits ban.
  During the match the server only makes cheap checks. Players are told after a ban, not on suspicion.
- A match that starts with bots is unranked; a bot that replaces a leaver keeps it ranked.
- Whoever starts a custom game is its **root admin**; the **creator** is whoever made the saved map.
- Caps: 100 matches and 1,000 players at once, with a wait queue. Two regions, US West and US East. macOS only.
- The user picked AWS for the system design, with a document mapping it to Google Cloud. But the repository's
  existing deploy (`infra/`) is on Google Cloud. Raise this again when the system design starts.
- The window mock-ups copy the look of the game's own menus. The user has not yet given feedback on the windows.

## Open with the user

- Their review of PRs 558 to 565 (the mechanics) and of the mock-up windows. Each document ends with its open
  questions, and each PR's Decisions section lists what was guessed.
- `staging/user-4gatepylon` has moved since the stack was cut. Before any merge, rebase the stack
  (`gh stack rebase --remote origin`) and check that no new reference to `game_design/DESIGN.md` appeared
  (PR 557 renames it): `git grep -n -P '(?<![A-Za-z_])DESIGN\.md'` should find nothing.

## If you work without a Mac

Nothing in the prototype needs one. `open <url>` in the READMEs is macOS's way to open a browser; use the URL. The
screenshots so far were taken in a desktop app's browser pane; use whatever browser tooling you have, and ask the
user before downloading one.
