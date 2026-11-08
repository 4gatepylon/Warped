# Snow Planet in Rust

A playable demo of the Snow Planet level (blue, depth 1, mid range): two teams of robots on a small snowfield with
two black holes, a wormhole and three blizzards, in the Rust + SDL3 + Metal game of
[`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/README.md). It started as a verbatim copy of that
demo (commit `9f249fc`); everything it doesn't describe here (controls, menus, HUD, settings files, the app, the
server bundle, the renderer and its optimizations) works as described there. The map is
[`common/maps/snow_planet/`](../../common/maps/snow_planet/map.json), made by [`tools/make_map.py`](tools/make_map.py).
Decisions are in [`DECISIONS.md`](DECISIONS.md). `OPTIMIZATIONS.md`, `PLAYTEST.md` and
`USER_WISHLIST_CHECKLIST.md` are the copied demo's and describe it, not this level.

## Play

From the repository's root, on a Mac with Homebrew's rustup (`export PATH="$(brew --prefix rustup)/bin:$PATH"`):

```bash
pushd demos/snow_planet_rust
make serve-game                      # terminal 1: a Snow Planet server on UDP port 47820
make join-game SERVER=127.0.0.1      # terminal 2: your window; press 1 (blue) or 2 (red)
```

- Two players on one Mac: run `make join-game SERVER=127.0.0.1` in a third terminal, or play against a bot:
  `cargo run --release -- --join 127.0.0.1 --port 47820 --bot --exit-after-s 900 --team red`.
- On another computer: `make join-game SERVER=<the address the server prints>`.
- `WASD` walks, `Shift` sprints, `Space` jumps, the mouse aims and the left button fires; hold `E` on one of the
  other team's two consoles (at the foot of its pylons) for 10 s to turn it off. A team wins when both of the other
  team's consoles are off. `Tab` opens the menu (scores, settings, map, credits).
- Blizzards (the white columns): inside, you walk at half speed and freeze (health falls 8% a second), and a laser
  goes only 8 m into one. They are in the map's `regions.blizzards` with their numbers (DECISIONS S3).

## Match interface

The match demo (`demos/warped_match_rust/`) launches the level with:

| Flag | With | What it does |
| --- | --- | --- |
| `--level-result-file <path>` | `--serve` | when a team wins, writes `{"winner":"blue"}` or `{"winner":"red"}` to `<path>` once (`<path>.partial`, then a rename), and doesn't start a new game; the server keeps running |
| `--team blue\|red` | `--join` | joins that team at once instead of waiting for `1`/`2` |
| `--bot-objective consoles` | `--bot` | the bot claims the other team's consoles one by one until its team wins |
| `--map snow-planet` | any | the default; `lunar-surface` and `house-demo` also load |

```bash
cargo run --release -- --serve --port 47820 --level-result-file /tmp/result.json
cargo run --release -- --join 127.0.0.1 --port 47820 --team blue
cargo run --release -- --join 127.0.0.1 --port 47820 --bot --exit-after-s 60 --team blue --bot-objective consoles
```

## How it works

- Space: the map is Euclidean with torus periods 112 × 72 m; the loader (`Map::curve_flat`) turns it into
  hyperbolic space of curvature radius 160 m, nearly flat, so the copied H³ ray marcher, physics, lasers and
  black-hole and wormhole lensing work unchanged. The level ends at its edge; walking across the torus seam is not
  done yet (DECISIONS S1).
- Blizzards: `VoxelWorld::find_blizzard` (a point's blizzard), `get_blizzard_frac` (how much of a shot's reach a
  chord uses) and `stop_in_blizzards` (cuts the laser) in `src/voxel_world.rs`; `VoxelPlayer::step` slows the
  player; the window sends freezing hits on itself (`Session::freeze_frac`, `net::Client::send_freeze`); the shader's
  `add_blizzards` (`src/render_voxel.metal`) adds the fog and falling flakes.
- Snow look: `appearance.style: snow` turns on the shader's snow sky, ground and fog (`VoxelUniforms::snow`).

## Validation

Run on the session's Mac VM (Apple M4 Pro, virtualized GPU), release build, 2026-11-08:

- `cargo build --locked`, `cargo clippy --all-targets`: no warnings. `cargo test`: 215 pass, including
  `points_find_their_blizzard`, `blizzards_slow_walking` and `lasers_stop_in_a_blizzard`.
- Map: `uv run python validate_map.py snow_planet lunar_surface` in `common/maps/`: both valid.
- Level win, end to end: a server with `--level-result-file`, one blue bot with `--bot-objective consoles`: it
  turned off consoles 2 and 3, the server printed "The blue team won", wrote `{"winner":"blue"}`, left no
  `.partial` file and kept running.
- Two players: a server and two bots (blue and red, 30 s): both joined their teams, shot each other (2 and 1
  hits), respawned.
- The laser into the white-out from 20 m stops after 18.0 m (10 m to its edge and 8 m in), as the map says.

## Performance

`--bench-views --width-px 2560 --height-px 1440 --frame-count 20` (milliseconds per frame, median, and GPU time):

| View | Median | GPU | View | Median | GPU |
| --- | ---: | ---: | --- | ---: | ---: |
| blue spawn | 12.5 | 10.8 | red spawn | 12.4 | 10.9 |
| overview (60 m up) | 15.5 | 13.8 | white-out, outside | 16.8 | 15.1 |
| white-out, inside | 11.2 | 9.6 | white-out, above | 14.2 | 12.5 |
| north squall | 18.7 | 16.8 | black hole (9 m) | 21.5 | 18.6 |
| black hole, far | 11.8 | 10.3 | wormhole | 19.1 | 17.2 |
| blue pylon | 8.1 | 6.8 | red pylon | 7.5 | 6.2 |
| map edge (seam) | 16.5 | 14.6 | laser into the white-out | 17.9 | 16.2 |

The window adapts its render scale (MetalFX upscaling) to hold 60 fps. Playing the warp tour (circling both black
holes and both mouths from 80% to 10% of their reach, `--play-route --route warp-tour --width-px 2560 --height-px
1440`; the VM's screen caps the window at 1600×1053 and 60 Hz): 59.6 fps, median 16.6 ms, 99th percentile 21.8 ms,
render scale 0.90 on average (full in 58% of frames). The copied Lunar Surface level on the same VM and route:
57.7 fps, median 16.9 ms, 99th percentile 24.2 ms, scale 0.64. So Snow Planet costs no more than the level it was
copied from; the VM's virtual GPU is the limit here. The 99th-percentile target (16.7 ms) is not met on this VM;
it should be measured on a real Mac.

## Not done yet

- The torus: walking, seeing and shooting across the map's edges (DECISIONS S1).
- Ice that slides, textures for snow and ice (procedural for now), freezing decided by the server.
- Fog of light bent into a blizzard by a black hole is only along the first straight part of the ray.
