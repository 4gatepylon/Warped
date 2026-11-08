# Dark Forest in Rust

A playable Rust demo of the Dark Forest level (W-122): a normal first-person shooter in an ordinary, bounded
Euclidean forest, whose feature is verticality. Each team's base has a treehouse on a 7 m deck, rope ladders up the
trunks, plank bridges between the decks, and a 12 m lookout in the middle. It is built on
[`../fps_demo/`](../fps_demo/README.md) (Rust, SDL3, a Metal PBR rasterizer) with the UDP multiplayer of
[`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/README.md). There are no black holes, wormholes or
curved space: the user asked for "just kind of a normal first-person shooter". Decisions made instead of the user are
in [`DECISIONS.md`](DECISIONS.md).

## Play

The forest textures are downloaded, not committed (10 MB). The robot, guns, sounds and sky are read from
`../fps_demo/assets/`.

Alone, against AI robots:

```bash
pushd demos/dark_forest_rust
make download-assets
make run-dark-forest BOT_COUNT=3
popd
```

Two players on one Mac (two terminals), or on two Macs with the server's address:

```bash
pushd demos/dark_forest_rust
make serve-game
popd
```

```bash
pushd demos/dark_forest_rust
make join-game SERVER=127.0.0.1 TEAM=red
popd
```

and `make join-game SERVER=127.0.0.1 TEAM=blue` in a second terminal. `cargo run --release --bin dark-forest --
--host` runs a server in the window's own process and joins it, so a second window can `--join` it.

| Input | Action |
| --- | --- |
| Mouse | look (click the window to capture the mouse) |
| `WASD` | move; walk into a ladder to climb it (`W` up, `S` or `Ctrl` down, or look down and `W`) |
| `Shift` / `Ctrl` or `C` / `Space` | sprint / crouch / jump (`Space` lets go of a ladder) |
| Left / right mouse | fire / aim down sights |
| `R` | reload |
| `E` (hold) | turn off an enemy console within 2.5 m (3 s) |
| `Esc` | release the mouse; a second `Esc` quits |
| `F1`-`F8`, `[` / `]` | effects and render scale, as in `../fps_demo/` |

## The level

- **Map:** [`../../common/maps/dark_forest/map.json`](../../common/maps/dark_forest/map.json), written by
  `src/forest_layout.rs` (`make export-map`) and read by both the game and the server. It is 60 m by 112 m, fenced by
  rock walls: 152 boxes, 93 round shapes (trunks and canopies), 10 ladders, 15 lights. Red's base is at +z and blue's
  is red's turned half a turn about the centre, so the two sides are the same.
- **Heights:** the ground; decks at 7 m (each team's treehouse, a middle deck and a flank deck, joined by bridges with
  rope rails); the lookout at 12 m on the great tree in the centre, with bridges from both sides.
- **Ladders:** a new optional `ladders` field in the map format (`common/maps/README.md`): a volume that isn't solid.
  A player inside one who walks toward it climbs at 3.2 m/s and steps onto the deck at its top; walking beside it
  does nothing (`src/game/player.rs`, with unit tests).
- **Objective:** two pylons per team (`game_design/DESIGN.md`), each with one console: one on the ground near the
  base, one on the treehouse deck. Hold `E` at an enemy console for 3 s to turn it off; a team wins once every console
  of the other team is off. The pips at the top of the screen show each console (lit or off), and a banner shows the
  winner.
- **Combat:** the FPS demo's hitscan revolver (or the CC0 rifle with `--rifle`), robots in team colours. Offline, AI
  robots fight you; online, other players are robots tinted blue or red.

## Match interface

The match session (`demos/warped_match_rust/`) starts levels with these flags:

| Command | Does |
| --- | --- |
| `dark-forest --serve --port <port> [--map <map.json>] [--level-result-file <path>]` | runs a server without a window. With `--level-result-file`, once a team has turned off every console of the other, it writes `{"winner": "blue"}` or `{"winner": "red"}` to `<path>.partial`, renames it to `<path>`, and keeps running (no new game) until killed. Without it, a new game starts 10 s after a win. |
| `dark-forest --join <address> --port <port> [--map <map.json>] [--team blue\|red]` | joins a server, on that team at once if `--team` is given. |
| `dark-forest --join <address> --port <port> --bot --exit-after-s <s> --team <team> [--bot-objective consoles]` | a bot window for tests; with `--bot-objective consoles` it holds `E` on the other team's consoles (the server takes its word for which one) until its team wins. |

The default port is 47840 (UDP), and server discovery uses 47841. `dark-forest-server` is the same server as its own
binary. The wire protocol is the hyperbolic demo's, with its wormhole fields always empty (see `DECISIONS.md`).

## Tools

- `--bench-views <s>`: holds five fixed views (`spawn`, `bridge`, `lookout`, `treehouse`, `firing`) for `<s>` seconds
  each and prints their frame times; `make bench-views` writes them to `.artifacts/bench_views/<run_id>/`.
- `--play-route`: the autopilot walks the vertical route from red's spawn: up the treehouse ladder, over the bridges
  to the middle and flank decks, up the lookout ladder, down the other side and back, printing each waypoint;
  `make bench-dark-forest` runs it full screen into `.artifacts/bench_dark_forest/<run_id>/`.
- `--camera-view <view>`, `--autopilot`, `--show-frame-time`, `--benchmark-seconds <s>`: as in `../fps_demo/`.
- `--export-map <path>`: writes the map and quits.

## Performance

Measured on a virtual Mac (`VirtualMac2,1`, Apple M4 Pro (Virtual), 12 cores, 16 GiB, macOS 26.6.2, Apple
Paravirtual GPU, 1600x1200 display at 60 Hz), Rust 1.99.0, Xcode 26.6, full screen at render scale 1.0, vsync on:

| Run | Mean | Median | p99 | GPU mean |
| --- | --- | --- | --- | --- |
| View `spawn` | 16.76 ms | 16.57 ms | 19.75 ms | 11.66 ms |
| View `bridge` | 16.95 ms | 16.66 ms | 21.43 ms | 11.72 ms |
| View `lookout` | 16.74 ms | 16.61 ms | 19.78 ms | 10.99 ms |
| View `treehouse` | 17.17 ms | 16.58 ms | 33.53 ms | 10.33 ms |
| View `firing` | 16.74 ms | 16.49 ms | 19.86 ms | 11.29 ms |
| `--play-route`, 3 robots, 60 s (5 s windows) | 16.67-17.12 ms | 16.60-16.73 ms | 19.05-33.64 ms | 11.37-11.75 ms |

The median sits on the 60 Hz vsync interval and the GPU needs 10-12 ms a frame, so there is headroom on the GPU, but
the p99 misses the 16.7 ms target on this virtual GPU: it has a frame or two of 20-34 ms in every window. A real Mac
at 2560x1440 has not been measured yet.

## Code

- `src/forest_layout.rs`: generates the map (trees, decks, bridges, ladders, pylons, spawns, lights).
- `src/map.rs`: reads and writes the portable map format.
- `src/game/world.rs`: turns the map into collision boxes, drawable shapes, ladders and consoles.
- `src/game/player.rs`: walking, jumping and climbing.
- `src/match_play.rs`: consoles, the win, and the online client (state, hits, remote robots).
- `src/net.rs`, `src/server.rs`, `src/bin/dark_forest_server.rs`: the UDP server and protocol, copied from the
  hyperbolic demo. `src/settings.rs`, `src/thrown_wormholes.rs` and `src/shot_noise.rs` keep only what that protocol
  needs.
- Everything else (renderer, assets, weapon, bots, audio, HUD) is the FPS demo's.

## Assets

| Asset | Source | Author | License | Size |
| --- | --- | --- | --- | --- |
| Texture `forest_ground_04` (1K) | https://polyhaven.com/a/forest_ground_04 | Rob Tuytel, Rico Cilliers | CC0 | 3.2 MB |
| Texture `bark_brown_02` (1K) | https://polyhaven.com/a/bark_brown_02 | Rob Tuytel | CC0 | 1.8 MB |
| Texture `old_planks_02` (1K) | https://polyhaven.com/a/old_planks_02 | Rob Tuytel | CC0 | 1.0 MB |
| Texture `forest_leaves_02` (1K) | https://polyhaven.com/a/forest_leaves_02 | Rob Tuytel | CC0 | 1.3 MB |
| Texture `mossy_rock` (1K) | https://polyhaven.com/a/mossy_rock | Rob Tuytel | CC0 | 2.9 MB |

`make download-assets` saves them in `assets/textures/` (git-ignored). The sky, robot, rifle, revolver and arms are
`../fps_demo/assets/`, credited in [its README](../fps_demo/README.md#assets); its arms are CC BY-NC 4.0 and its
revolver Sketchfab Standard, which limit sharing the game.

## Limits

- No title screen, menus, minimap or in-game credits page yet (the hyperbolic demo has them).
- AI robots walk on the ground only; they don't climb.
- Online, the server trusts the client about which console it holds and about its hits.
- The protocol still carries the hyperbolic demo's wormhole fields, always empty.
