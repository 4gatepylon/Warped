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
  player; the server freezes players standing in one (`net::Server::freeze_players`, `net::Freezing`); the shader's
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

| View | Snow Planet | GPU | before longer steps | Lunar Surface, same spot | GPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| blue spawn | 12.1 | 10.4 | 12.6 | 23.9 | 20.0 |
| red spawn | 10.9 | 9.8 | 11.4 | 22.0 | 17.7 |
| overview (60 m up) | 10.7 | 9.5 | 11.1 | 14.4 | 10.6 |
| white-out, outside | 12.1 | 10.9 | 13.0 | 23.0 | 18.7 |
| white-out, inside | 10.5 | 9.3 | 10.8 | 19.8 | 16.1 |
| white-out, above | 11.4 | 10.2 | 12.1 | 24.1 | 20.4 |
| north squall | 10.2 | 9.1 | 10.7 | 27.6 | 25.2 |
| black hole (9 m) | 17.1 | 16.0 | 19.1 | 21.8 | 17.8 |
| black hole, far | 10.9 | 9.7 | 11.4 | 26.8 | 24.3 |
| wormhole | 16.0 | 14.9 | 17.7 | 21.4 | 17.4 |
| blue pylon | 7.2 | 6.1 | 7.2 | 19.3 | 16.3 |
| red pylon | 7.3 | 6.1 | 7.3 | 23.8 | 19.4 |
| map edge (seam) | 8.1 | 6.9 | 8.3 | 21.9 | 18.0 |
| laser into the white-out | 13.9 | 12.8 | 15.1 | 23.6 | 21.6 |

Bent rays near a black hole or mouth step 12% of their distance to it, up to 6 m (`BEND_STEP_FRAC`,
`MAX_BEND_STEP_M`; were 8% and 3 m): about 2 ms off both warp views with no visible change
(`.artifacts/pr/bend_steps.png`). The Lunar Surface column is the copied Lunar map seen from the same coordinates
(`--map lunar-surface --bench-views`), so its views aren't Lunar's own landmarks. Steps of 16% and 10 m gained
nothing more.

The window adapts its render scale (MetalFX upscaling) to hold 60 fps. Playing the warp tour (circling both black
holes and both mouths from 80% to 10% of their reach, `--play-route --route warp-tour --width-px 2560 --height-px
1440`; the VM's screen caps the window at 1600×1053 and 60 Hz), twice on Snow Planet and once on Lunar Surface, one
after the other:

| Level | fps | median ms | 99th percentile ms | render scale, mean |
| --- | ---: | ---: | ---: | ---: |
| Snow Planet | 58.4, 59.0 | 16.7, 16.5 | 32.8, 33.0 | 0.84, 0.81 |
| Lunar Surface | 57.1 | 16.8 | 28.2 | 0.64 |

An earlier run (before this change) measured 21.8 ms (Snow) and 24.2 ms (Lunar) at the 99th percentile, so the VM
varies by about 10 ms between runs; the median is the 60 Hz vsync. Snow Planet holds a higher render scale than
Lunar Surface. The 99th-percentile target (16.7 ms) and the warp views (16-17 ms) are not met on this VM's virtual
GPU; they should be measured on a real Mac.

## Not done yet

- The torus: walking, seeing and shooting across the map's edges (DECISIONS S1).
- Ice that slides, textures for snow and ice (procedural for now).
- The blizzard slowdown is the moving window's own (like all movement in this demo); the server doesn't check it.
- Fog of light bent into a blizzard by a black hole is only along the first straight part of the ray.
