# Dust (Rust demo)

Dust is the deepest blue level (W-121, Dust demo): a long-range desert in hyperbolic space (curvature radius
R = 50 m), in a constant sandstorm whose wind bends light. It has no black hole, one fixed wormhole pair, and the
thrown wormholes, robots, guns, pylons and consoles of `demos/hyperbolic_wormholes_rust/`, which this folder
copies (the first commit on `devin/dust-demo` is that copy, so `git diff 688f402` shows only Dust's changes).

## Play

```sh
pushd demos/dust_rust
make download-assets        # once: Dust's sand and sandstone textures (Poly Haven, CC0)
cargo run --release -- --host              # one window, hosting a game; press 1 (blue) or 2 (red)
```

Two players on one Mac: `cargo run --release -- --serve` in one terminal, then
`cargo run --release -- --join 127.0.0.1 --team blue` and `... --team red` in two others. A bot:
`cargo run --release -- --join 127.0.0.1 --bot --exit-after-s 120 --team red`.

A team wins by holding `E` on each of the other team's 3 consoles (one per pylon). Default UDP port 47830,
LAN discovery 47831.

## Match flags

| Flag | Meaning |
| --- | --- |
| `--serve --port <p> [--map dust] --level-result-file <path>` | When a team has turned off all the other team's consoles, write `{"winner": "blue"}` or `{"winner": "red"}` to `<path>` once (`<path>.partial`, then renamed), and never start a new game. Also on `wormholes_server`. |
| `--join <addr> --port <p> [--team blue\|red]` | Join that team at once instead of choosing in the lobby. |
| `--join <addr> --port <p> --bot --exit-after-s <s> --team <t> --bot-objective consoles` | The bot holds `E` on the other team's consoles (the server takes its word for which one) until a team wins. |

## How it works

- **Map**: `common/maps/dust/map.json`, written by `tools/make_dust_map.py` (rerun it after editing it): a crater
  basin with dunes, sandstone mesas, pillars, ruin walls and an arch, HESCO rows, bunkers, 3 pylons per team
  (mirrored), spawns at u = ±104 m, a wormhole pair at (-50, -40) and (50, 40) m.
- **Wind**: the map's `atmosphere.wind` is a refractive-index field, ln n = k d, where d is the signed hyperbolic
  distance (m) to the plane through the origin perpendicular to `direction`, and k is
  `log_index_gradient_per_m` (0.0012 /m). Light follows the ray equation, so it curves toward higher n, i.e.
  downwind, with curvature k per meter. Rays and the laser integrate it kick-drift-kick in chords of `segment_m`
  (48 m; 12 m looked the same and cost 5 ms more a frame): half a kick by ∇ln n's part across the ray, a geodesic chord, half a kick. Inside wormhole mouths the
  wind's gradient adds to the mouth's own. The Metal shader (`wind_gradient`, `wind_kick`) and the CPU laser
  (`WindField` in `src/voxel_world.rs`) use the same rule, so the laser lands where the crosshair shows.
- **Sandstorm**: `atmosphere.fog` replaces the haze: colour, start (4 m), e-folding distance (60 m) and the most
  it hides (94%). Rays stop 5 e-foldings past the start (304 m), where the fog hides all but 0.7%.
- **Blowing sand** (`blow_sand` in the shader, on maps with fog): gusts, value noise 40 m long, 14 m wide and 6 m
  tall, drifting downwind at 9 m/s, thicken or thin the fog by up to 30% where each ray is halfway to its end
  (or 60 m out); thin streaks (4 m by 8 cm) at 2.5 m and 6.5 m in front of the camera drift the same way. The
  clock is the window's sky time, so `--bench-views` pictures are still frames. Cost: about 1 ms a frame.
- **Textures**: `aerial_sand` (sand), `gravelly_sand` (dark sand), `cliff_side` (sandstone),
  `large_sandstone_blocks` (ruins), from `tools/download_textures.sh` into `assets/textures/` (git-ignored).

## Results (2026-11-08)

Run on a virtual Apple M4 Pro Mac (macOS, Metal), so frame times are slower than on the user's Mac: the
reference Lunar Surface level takes 21-71 ms per frame at the same size on this machine.

- `cargo build --release --locked`, `cargo test --release` (214 pass), `cargo clippy --all-targets`: clean.
- `make validate-maps` in `common/maps/`: dust, house_demo, lunar_surface valid.
- End to end: `--serve --level-result-file` with a blue `--bot-objective consoles` bot wrote
  `{"winner": "blue"}`; with two bots (red holding consoles, blue shooting) it wrote `{"winner": "red"}`.
- `--bench-views --width-px 2560 --height-px 1440` (Metal, median ms; Lunar Surface run by this same binary
  and shader for comparison):

  | | Dust before | Dust now | Lunar before | Lunar now |
  | --- | --- | --- | --- | --- |
  | median of the views | 23.5 | 20.2 | 25.9 | 25.0 |
  | worst view | 53.1 (laser_into_wormhole) | 46.1 (laser_into_wormhole) | 70.7 (wormhole_exit) | 33.3 |
  | blue_spawn | 24.1 | 20.2 | 22.6 | 21.9 |
  | wormhole / wormhole_exit | 38.8 / 38.7 | 33.1 / 33.2 | 41.7 / 70.7 | 22.8 / 28.1 |

  "Now" has 48 m wind chords and terrain steps that grow with the whole distance a ray has gone (they used to
  restart at each chord and each wormhole step), which helped both levels; Dust's wormhole views are still
  slower than Lunar's (the fog chords after the mouth). Not under 16.7 ms on this machine.
- `--play-route --route warp-tour` at 1280x800 (frame times include 60 Hz vsync; the first 5 frames are skipped):

  | | median ms | 99th percentile ms | worst ms | render scale mean |
  | --- | --- | --- | --- | --- |
  | Dust before (1600x1053) | 21.9 | 189.1 | 802.6 | 0.50 |
  | Dust now, dynamic scale | 16.6 | 20.0 | 295.2 | 0.59 |
  | Lunar now, dynamic scale | 16.7 | 22.3 | 318.7 | 0.72 |
  | Dust now, `--render-scale 1` | 23.5 | 33.8 | 277.3 | 1.00 |
  | Lunar now, `--render-scale 1` | 20.3 | 33.1 | 308.2 | 1.00 |

  The 189 ms 99th percentile is gone; it did not come back in three runs. The worst frame is frame 6, a one-time
  start-up stall (frames 1-6 take 0.3, 2.5, 0.1, 0.1, 0.001 and 0.3 s on both levels, while Metal and MetalFX
  set up), not a mid-run spike; the slowest frame after it is 35 ms. `.artifacts/play_route/<run>/frames.csv` has
  every frame's time and render scale.
- Thrown wormholes: `a_dust_throw_carries_the_laser_through_the_wind` shoots through a thrown purple mouth on
  Dust's terrain and wind.
