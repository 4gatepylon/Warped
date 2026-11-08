# Dark Forest in Rust

A playable Rust demo of the Dark Forest level (W-122): a first-person shooter in a dim, swampy bamboo labyrinth in
bounded Euclidean space, whose features are verticality and wormholes. Each team's base has a treehouse on a 7 m deck,
rope ladders, plank bridges between the decks, and a 12 m lookout in the middle; five fixed wormholes, drawn as real
lensed wormholes, are shortcuts through the maze. It is built on [`../fps_demo/`](../fps_demo/README.md) (Rust, SDL3, a
Metal PBR rasterizer) with the UDP multiplayer of [`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/README.md).
Decisions made instead of the user are in [`DECISIONS.md`](DECISIONS.md).

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
  `src/forest_layout.rs` (`make export-map`, compact JSON, 8.6 MB) and read by both the game and the server. It is
  60 m by 112 m, fenced by rock walls. Red's base is at +z and blue's is red's turned half a turn about the centre, so
  the two sides are the same.
- **Bamboo labyrinth:** `add_labyrinth` carves red's half into a seeded maze of 4 m cells (a winding depth-first tree
  plus about 12% extra openings for loops, so there are dead ends and loops), grows a wall of bamboo clumps on every
  closed side where nothing else stands, and mirrors it onto blue's half. Corridors are about 1.5-3 m wide. The seed
  and cell size are in the map's `scatter` field. A walk-grid flood test checks that every spawn, ground pylon,
  ladder foot and wormhole mouth can be reached. Each culm forks at the top into leafy branches whose shadows keep
  the forest dark, with light shafts through the gaps.
- **Heights:** the ground; decks at 7 m (each team's treehouse, a middle deck and a flank deck, joined by bridges with
  rope rails), held up by bundles of thick culms; the lookout at 12 m in the centre.
- **Ladders:** the optional `ladders` field of the map format: a volume that isn't solid. A player inside one who
  walks toward it climbs at 3.2 m/s and steps onto the deck at its top.
- **Wormholes:** five fixed pairs, no black holes: three across the ground and each team's lift from the ground to its
  treehouse deck. Each mouth is the hyperbolic demo's foil lens in flat space (`src/game/warps.rs`): it bends light
  and lasers within its reach, and whatever enters its throat leaves the partner's through the partner's mirror in z.
  The tonemap pass traces each pixel's ray through the lens (`get_lens_color` in `src/shaders/post.metal`) and shows
  the far side from a cube atlas of every mouth, drawn once at start-up (about 0.8 s).
- **Objective:** two pylons per team (the hyperbolic demo's `scifi_beacon`), one on the ground near the base, one on
  the treehouse deck, each sending up a beam in its team's colour while its console is on. Hold `E` at an enemy
  console for 3 s to turn it off; a team wins once every console of the other team is off.
- **Look:** a murky green grade of the HDRI sky, thick green haze, golden-white lasers for everyone (the map's
  `laser.rgb`), and 31 flickering candles (warm map lights with `flicker_frac`) on the decks, bridges and support
  bundles.
- **Map page:** the menu's map page (Tab, then E) is `assets/map_picture.png`, a photo of the forest from straight
  above made by `make map-picture`, with live marks over it; without it the page shows the old drawing.

## Match interface

The match session (`demos/warped_match_rust/`) starts levels with these flags:

| Command | Does |
| --- | --- |
| `dark-forest --serve --port <port> [--map <map.json>] [--level-result-file <path>]` | runs a server without a window. With `--level-result-file`, once a team has turned off every console of the other, it writes `{"winner": "blue"}` or `{"winner": "red"}` to `<path>.partial`, renames it to `<path>`, and keeps running (no new game) until killed. Without it, a new game starts 10 s after a win. |
| `dark-forest --join <address> --port <port> [--map <map.json>] [--team blue\|red]` | joins a server, on that team at once if `--team` is given. |
| `dark-forest --join <address> --port <port> --bot --exit-after-s <s> --team <team> [--bot-objective consoles]` | a bot window for tests; with `--bot-objective consoles` it holds `E` on the other team's consoles (the server takes its word for which one) until its team wins. |

The default port is 47840 (UDP), and server discovery uses 47841. `dark-forest-server` is the same server as its own
binary. The wire protocol is the hyperbolic demo's, with its wormhole fields always empty (see `DECISIONS.md`).

Every command also takes `--game-password-file <file>`: a server admits only players who prove they know the password
in it, and a player proves it.

### In the online match (W-127)

The online match ships this level in its app and its cloud server bundle (`demos/warped_match_rust/README.md`, "The
online match"):

- `make build-server-bundle` builds `dark-forest-server` for x86-64 Linux into `.build/server_bundle.tar.gz`; the
  Mac-only crates are dependencies only on macOS.
- `make package-mac-app` (after `make download-assets`) builds `.build/Warped/Warped.app` with every file the window
  reads: the fps demo's sky, robot and rifle in `Contents/Resources/assets/`, the pylon in `models/`, and this level's
  own textures and map picture in `dark_forest_rust/` (`src/app_bundle.rs`). The map is built into the program.
- Known gap: the Sketchfab revolver and arms may ship only built into a program, not as files, so the app's Dark
  Forest shows the rifle until they are embedded (as `demos/hyperbolic_wormholes_rust/build.rs` does).

## Tools

- `--bench-views <s>`: holds five fixed views (`spawn`, `bridge`, `lookout`, `treehouse`, `firing`) for `<s>` seconds
  each and prints their frame times; `make bench-views` writes them to `.artifacts/bench_views/<run_id>/`.
- `--play-route`: the autopilot walks the vertical route from red's spawn; `make bench-dark-forest` runs it full
  screen into `.artifacts/bench_dark_forest/<run_id>/`.
- `--photo <png> [--photo-width-px <px>] [--camera-pose x,y,z,yaw,pitch] [--fov-deg <deg>] [--hide-canopy]`: saves
  one frame without the HUD and quits; `--photo-map-page` opens the menu's map page.
- `--map-picture <png>` (`make map-picture`): renders the map page's picture.
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
at 2560x1440 has not been measured yet. The table above is from before the labyrinth and wormholes.

With the labyrinth (about 8,900 culms) and the five lensed wormholes, `--photo` runs in a 1600x900 pt window with 4
robots (60 frames each, vsync on, median at the 60 Hz interval in every run) give these GPU times:

| View | Render scale 2.0 | Render scale 1.0 |
| --- | --- | --- |
| No mouth in view | 11.8 ms | 7.3 ms |
| One mouth in view | 14.5-15.9 ms | not measured |
| A mouth filling the screen | 16.6 ms | 9.8 ms |

## Code

- `src/forest_layout.rs`: generates the map (labyrinth, bamboo, decks, bridges, ladders, pylons, wormholes, candles,
  spawns, lights).
- `src/map.rs`: reads and writes the portable map format.
- `src/game/world.rs`: turns the map into collision boxes, drawable shapes, ladders, consoles and pylons.
- `src/game/warps.rs`: the wormholes' lens, throat crossing and laser paths (and black holes, unused here).
- `src/game/player.rs`: walking, jumping and climbing.
- `src/match_play.rs`: consoles, the win, and the online client (state, hits, remote robots).
- `src/shaders/post.metal` (`get_lens_color`) and `capture_wormhole_lens` in `src/main.rs`: the wormhole lens pass.
- `src/net.rs`, `src/server.rs`, `src/bin/dark_forest_server.rs`: the UDP server and protocol, copied from the
  hyperbolic demo.
- Everything else (renderer, assets, weapon, bots, audio, HUD) is the FPS demo's.

## Assets

| Asset | Source | Author | License | Size |
| --- | --- | --- | --- | --- |
| Texture `forest_ground_04` (1K) | https://polyhaven.com/a/forest_ground_04 | Rob Tuytel, Rico Cilliers | CC0 | 3.2 MB |
| Texture `bamboo_wall` (1K) | https://polyhaven.com/a/bamboo_wall | Amal Kumar | CC0 | 2.0 MB |
| Texture `old_planks_02` (1K) | https://polyhaven.com/a/old_planks_02 | Rob Tuytel | CC0 | 1.0 MB |
| Texture `forest_leaves_02` (1K) | https://polyhaven.com/a/forest_leaves_02 | Rob Tuytel | CC0 | 1.3 MB |
| Texture `mossy_rock` (1K) | https://polyhaven.com/a/mossy_rock | Rob Tuytel | CC0 | 2.9 MB |
| Pylon model `scifi_beacon` (shared: `common/maps/models/scifi_beacon/`) | https://sketchfab.com/3d-models/sci-fi-beaconway-point-marker-free-model-a1d91ebb3e2d41bba31c02b11423d97f | AMMediaGames | CC BY 4.0 | 1.0 MB (committed) |

`make download-assets` saves them in `assets/textures/` (git-ignored). The sky, robot, rifle, revolver and arms are
`../fps_demo/assets/`, credited in [its README](../fps_demo/README.md#assets); its arms are CC BY-NC 4.0 and its
revolver Sketchfab Standard, which limit sharing the game.

## Limits

- Robots and players on the far side of a wormhole don't show through it: the lens shows a static picture of each partner's surroundings drawn at start-up. A parked, unfinished overlay is on branch `devin/dark-forest-lens-robots` (see DECISIONS.md).
- The online bot match (`--bot --bot-objective consoles`) was not rerun to a win after the server's console count was fixed; rerun it first.
- Through a wormhole you see the scenery and lights around its partner, drawn at start-up, but not robots, players or
  lasers there.
- Lasers of other players online are traced locally; their hits come from the shooter's own trace.
- Online, the server trusts the client about which console it holds and about its hits.
- The protocol still carries the hyperbolic demo's wormhole fields, always empty.
