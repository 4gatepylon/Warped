# Online design and system prototype: handoff

State on 2026-11-11. A Claude Code session on the user's Mac wrote the outside-game design, the window mock-ups and
part 1 of this prototype; a Cursor cloud agent then built part 2 (accounts) and is going on with the parts after it.
Read this first if you continue the work. It is written for an agent that has only this repository: it repeats
what was otherwise only in those sessions. [`DECISIONS.md`](DECISIONS.md) lists what was decided without the user,
and [`VERIFY.md`](VERIFY.md) how to check each feature.

## The goal

The user is working on something else and wants to come back to **a finished project of high quality that needs
only small changes to be deployed for real**: the design tried out in a prototype, then all the real code written
and running on one computer against mocks of the cloud, with no cloud account.

**Work without waiting for the user.** They are not reviewing as you go. Where a decision would be theirs, choose
the option that best fits `game_design/OUTSIDE_GAME_DESIGN.md` and what is written here, write the decision and
its reason in the PR's Decisions section and in `DECISIONS.md` beside this file (create it, numbered, newest
last), and go on. Stop only for something that can't be undone or that costs money.

**When in doubt, do what Counter-Strike does.** The user's words: "games like this are well known; if in doubt the
other agent can copy the CSGO semantics". Where the documents are silent or unclear about how ranked play,
matchmaking, parties, leaving, votes, cooldowns or the like should behave, look up how Counter-Strike (CS:GO or
CS2) does it, copy that, and record it as a decision with where you found it.

**Work for a long time.** The point is that you and the user work in parallel for as long as possible. Don't stop
at the end of a part to report or to ask: go straight on to the next. If one thing is blocked, write down why and
what you tried, and move to the next thing that isn't. Commit and push after every piece that works, and keep this
file current, so that nothing is lost if you are cut off and another agent can take over.

**Everything important must be checkable afterwards, without you.** The user wasn't watching, so a claim counts
only if they can verify it later:

- every rule and every behaviour that matters has an automated test, and `make test` runs them all;
- every flow a person would use has a command that reproduces it and screenshots of it attached to its PR;
- every run that produced a result keeps its `command.txt` and outputs in `.artifacts/` (root `AGENTS.md`);
- each PR's Testing section lists the commands run and what they printed, and says plainly what was not tested;
- keep `VERIFY.md` beside this file: one row per feature, with what to run or open and what the user should see.

## The work, in order

1. **The mechanics outside a match, as documents:** `game_design/OUTSIDE_GAME_DESIGN.md` and
   `game_design/WINDOW_DESIGN.md`. Done. The user has not reviewed them yet; build on them as written.
2. **Finish the interactive prototype** (this folder): parts 2 to 7 below. It is where the design is tried and
   shown. When it contradicts the documents, fix whichever is wrong and say so.
3. **Write the system design** in `game_design/cloud/` as static web pages: system diagrams, the database schema
   drawn, the API with its schemas, the protocols. Take it from the documents and from what the prototype showed
   to work.
4. **Implement the real thing, against mocks.** All the code the online side needs, at production quality, not a
   simulation: the backend services, the database schema with its migrations, the game server's and the app's
   side of each protocol, the Terraform and deploy code for Google Cloud, and tests. It must run end to end on one
   computer with the cloud provider's APIs mocked or emulated (the user's example was mocking `boto3`; with Google
   Cloud it is that provider's client libraries or emulators), so that deploying for real means changing
   configuration, not code. Build on what exists: `management_server/`, `game_server/`, `common/protocol/`,
   `infra/` and the Rust game in `demos/`. Say in the README exactly what is mocked and what a real deploy must
   change.

Each step is a run of stacked PRs on top of the one before. Keep the whole stack open and unmerged for the user.

## The rule above all others

The user's words: "as much as possible has to be succinctly represented and visualized and/or documented so that
a human with low context can validate or give feedback", and they called it super important. So, for everything
you make:

- **Show it.** A picture, a diagram, a live page or a short table before a paragraph. The system view exists for
  this: every new component, message, table and queue must appear in it.
- **Keep it short.** Lead with what the thing is and what the reader should check. Cut what they don't need.
- **Assume the reader knows nothing** of this work: define each term once, name things in plain words, and never
  point at a conversation they weren't in.
- **Make feedback easy.** Say exactly what you want judged, give named options where there is a choice, and keep
  the feedback bar on every mocked window.
- **Attach screenshots of one common flow to each part's PR,** so the user can check it at a glance when they
  are back. Don't wait for an answer.

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
  | see below | `cursor/prototype-2-accounts-fe19` | Part 2: accounts, fake Google, fake Stripe |

  Each later part is a branch `cursor/prototype-<n>-<what>-fe19` on the one before; `git log --oneline
  claude/prototype-accounts..` on the top branch, or the PRs' Stack lines, list them.

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
- **Continue the stack:** check out the top branch and add each new part as a branch on top of it
  (`gh stack add <branch>`, then `gh stack submit --auto --open --remote origin`; the repository has two remotes,
  so `gh stack` needs `--remote origin`). Where `gh stack` isn't available (a Cursor cloud agent, whose `gh` is
  read-only, 2026-11-10), open each PR with the agent's own PR tool, based on the branch below and stacked on its
  PR (`DECISIONS.md` 1).
- **Size:** the user chose one PR per part for this prototype, each over the 200-line limit. Say so in the PR's
  Size section with the line count.
- **After each part,** take screenshots of one common flow and attach them to the PR
  (`gh pr edit <n> --body-file body.md --attach <file>`). `make capture-flows` plays every flow in Chrome and
  screenshots it; add each new flow to `scripts/capture_flows.py` and a row to `VERIFY.md`.
- **Each PR's Stack line** names the layers below and above. When you add a layer, update the line in every PR.
- Never merge into `staging/global` or `main`, and leave this stack unmerged too: the user wants to review it as a
  whole when they are back. Rebase it when `staging/user-4gatepylon` moves.
- `AGENTS.md` must stay under 16 KiB. It is within a few bytes of that, so don't add to it.

## How this prototype is built

`README.md` says what it is and how to run it (`make run-prototype`, then `http://127.0.0.1:8010/`; `make test`;
`make fmt`). The rules that matter when adding to it:

- **Simulate each component of the real design separately.** The fleet's components are groups of methods of
  `World` (`world.py`); since part 2, a new component is its own class in its own file (`accounts.py`,
  `identity_provider.py`, ...) that reaches the database and the other components only through the `Bus`
  (`bus.py`). Whenever one component would call another, `Bus.send` records a row in the `message` table. Add new
  components to `COMPONENTS` in `bus.py` and to `NODES` in `web/system/system.js`, and a table's owner to
  `_TABLE_OWNERS_BY_PREFIX` in `world.py`, so the system view draws them.
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

2. **Accounts. Built** (`cursor/prototype-2-accounts-fe19`). Not built, left for later parts: Google's "Cancel"
   (an `access_denied` answer) on the fake Google page; checking the EU at sign-in, not only at sign-up; and the
   superadmin console's `PUT /api/settings` still takes no sign-in (part 7 should require a superadmin's session).
   What was asked:
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
- **Google Cloud, not AWS, for now.** The user first picked AWS, then changed to Google Cloud at the end of the
  session, which also matches the repository's existing deploy (`infra/`, Terraform). The early draft, the cost
  notes in PR 586 (a machine at $0.145 an hour is an AWS list price) and anything else that says AWS are to be
  redone for Google Cloud; working that out is part of the job.
- The window mock-ups copy the look of the game's own menus. The user has not yet given feedback on the windows.

## Open with the user (none of it blocks you)

- Their review of PRs 558 to 565 (the mechanics) and of the mock-up windows. Each document ends with its open
  questions, and each PR's Decisions section lists what was guessed. Settle the open questions yourself where the
  work needs an answer, and record each as a decision.
- `staging/user-4gatepylon` has moved since the stack was cut. Before any merge, rebase the stack
  (`gh stack rebase --remote origin`) and check that no new reference to `game_design/DESIGN.md` appeared
  (PR 557 renames it): `git grep -n -P '(?<![A-Za-z_])DESIGN\.md'` should find nothing.

## If you work without a Mac

Nothing in the prototype needs one. `open <url>` in the READMEs is macOS's way to open a browser; use the URL.
`make capture-flows` drives the installed Google Chrome through Playwright (`channel="chrome"`), so it downloads
no browser; on a machine without Chrome, ask the user before downloading one.
