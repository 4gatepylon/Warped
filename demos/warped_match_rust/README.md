# Warped match (Rust)

The whole game end to end: a **match** across the five levels by the rules of
`game_design/DESIGN.md` (Match, Wavefront). A match server keeps the match state (every round
and its winner, each team's wavefront, offensive and defensive ownership, who attacks, the
result), starts each round's level as its own program, reads back which team won it, and shows
the progression in each player's match window. W-123.

It is a copy of `demos/hyperbolic_wormholes_rust/` (first commit on the branch, verbatim) plus
the match (`src/match_state.rs`, `src/match_flow.rs`, `src/match_menu.rs`, `match_levels.yaml`)
and the level interface below added to the copied level code. Lunar Surface stands in for the
levels not plugged in yet (the house map has no consoles, so it can't be won).

## Play

```bash
export PATH="$(brew --prefix rustup)/bin:$PATH"
pushd demos/warped_match_rust
make serve-match                        # terminal 1: the match server (UDP 47850)
make join-match TEAM=blue               # terminal 2: blue's match window
make join-match TEAM=red                # terminal 3: red's (or: make bot-match TEAM=red)
popd
```

Alone: `make serve-match`, `make join-match TEAM=blue`, `make bot-match TEAM=red`. The bot turns
off blue's consoles (10 s each, 3 on Lunar Surface), so be quick. From another Mac on the same
network: `make join-match SERVER=<server's address> TEAM=red`. Without `TEAM`, press 1 (blue) or
2 (red) in the match window.

A round starts 10 s after each team has a player. Each round opens the level in its own window,
joined to your team; a team wins it by turning off all of the other team's consoles. The level
stays open 5 s to show the win, closes, and the match window shows the progression until the next
round. `make e2e-match` plays a whole match of two bots with consoles off in 1 s.

## The progression page

![The match window after a round](docs/progression.png)

- The line of levels, blue's deepest on the left, `*` on the next round's level.
- Each level's place (`blue 2` … `purple` … `red 2`), its defender (defensive ownership) and its
  offence: whose wavefront it is, who captured it and in which round, or not reached.
- Both wavefronts, the attacker (offensive ownership) and defender now.
- The history of rounds: level, attacker, winner, and what it did (attacker advances, defender
  holds and roles swap). The result: a team wins, or a tie after 30 minutes.

## Match rules (`src/match_state.rs`)

Exactly `game_design/DESIGN.md`: round 1 is City Sphere with no attacker; its winner attacks. Each
team's wavefront starts at the other team's depth-1 level. An attacker that wins moves its
wavefront one level deeper, or wins the match on the defender's deepest level. A defender that
wins becomes the attacker, at its own wavefront (wavefronts never move back). City Sphere is
played once. 30 minutes without a winner is a tie. Unit tests: the design doc's worked example
row by row, and every win sequence up to 14 rounds (866) against a separate reading of the
rules.

## Level interface

Each level is a program that the match runs once per round. `match_levels.yaml` says which
program plays each level (`program: self` is this demo's binary, else a path from this folder)
and its `--map`. A level program implements:

```text
Server: <program> --serve --port <port> [--map <map>] --level-result-file <path> [server_args]
```

When one team has turned off every console of the other team, the server writes `<path>` once,
atomically (`<path>.partial`, then a rename): `{"winner": "blue"}` or `{"winner": "red"}`. It
does not start a new game after that; it keeps running (players see the win) until the match
server kills it.

```text
Client: <program> --join <address> --port <port> [--map <map>] --team blue|red [client_args]
Bot:    <program> --join <address> --port <port> [--map <map>] --team <team> --bot --bot-objective consoles --exit-after-s <s> --label <label>
```

`--team` joins that team at once, without the lobby. The match client kills the level when the
round ends, so a level needs no exit logic. `--bot-objective consoles` holds `E` on the other
team's consoles in turn until its team wins (the copy's bot doesn't walk there: the server, as in
the hyperbolic demo, doesn't check a player's distance to a console).

Ports: match server UDP 47850 (47851 reserved for its discovery, not used yet: join by
address); each round's level server 47852, and this copy's level discovery 47853.

## The five levels (source of truth)

| # | Level | Side, depth | Pylons a team | Folder | Branch | In the match |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Dust | blue 2 | 3 | `demos/dust_rust/` | `devin/dust-demo` | plugged in (`make levels`) |
| 2 | Snow Planet | blue 1 | 2 | `demos/snow_planet_rust/` | `devin/snow-planet-demo` | plugged in (`make levels`) |
| 3 | City Sphere | purple 0 | 2 | `demos/city_sphere_rust/` | `devin/city-sphere-demo` | Lunar Surface stands in |
| 4 | Dark Forest | red 1 | 2 | `demos/dark_forest_rust/` | `devin/dark-forest-demo` | Lunar Surface stands in |
| 5 | Lunar Surface | red 2 | 3 | `demos/hyperbolic_wormholes_rust/` | `staging/user-4gatepylon` | playable (copied here) |

To plug a level in: add `<folder name>:<branch>` to `LEVEL_BRANCHES` in the `Makefile` (`make
levels` checks the branch out as a git worktree in `.levels/<folder name>/` and builds it there;
`make serve-match` runs it first), then in `match_levels.yaml` set the level's `program` to the
binary (e.g. `.levels/snow_planet/demos/snow_planet_rust/.build/release/snow-planet`) and `map`,
drop `stand_in_for`, and update this table.

## Results

See the PR for the latest runs; `make e2e-match` writes `.artifacts/match/<run>/match.json`
(rounds, winners, outcome) and each round's level server log.

## Files

- `src/match_state.rs`: the rules and the state; `src/match_flow.rs`: match server, protocol
  (`hello <team>` datagrams answered with the state as JSON), level processes;
  `src/match_menu.rs`: the progression page; `match_levels.yaml`: the levels.
- In the copied level code: `--level-result-file` (`net::write_level_result`), `--team`,
  `--bot-objective consoles` (`multiplayer::get_console_to_disable`), `--serve-match`, `--match`.
- `e2e/`: settings and levels for the fast bot match.
- Everything else is the hyperbolic demo; see its README for the level itself.
