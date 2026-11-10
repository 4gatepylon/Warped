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
| `--join <addr> --port <p> --bot --exit-after-s <s> --team <t> --bot-objective consoles` | The bot walks to the other team's consoles and holds `E` on each once in reach (`console_bot.rs`; the server takes a hold only from within `consoles.reach_m`, aimed at it: `console_reach.rs`) until a team wins. |

## How it works

- **Shared code**: the files every Rust level shares (geometry, the gun held in hand, the HUD's bars and overlay
  types, the maps' models and the robot body, the smooth shapes, the key bindings, finding assets) are in
  [`../warped_shared_rust/`](../warped_shared_rust/README.md), a library this crate depends on; the rest of the
  copied Lunar Surface code is still in `src/`.
- **Map**: `common/maps/dust/map.json`, written by `tools/make_dust_map.py` (rerun it after editing it): a crater
  basin with dunes, sandstone mesas, pillars, ruin walls and an arch, HESCO rows, bunkers, 3 pylons per team
  (mirrored), spawns at u = ±104 m, a wormhole pair near (-50, -40) and (50, 40) m, centred at eye height so
  players walk through it.
- **Gravity toward warps** (W-144, `gravity` in the server settings, the shared rule in
  [`../warped_shared_rust/src/gravity.rs`](../warped_shared_rust/src/gravity.rs)): `VoxelPlayer::step` adds the pull
  of every black hole and wormhole mouth, the map's and the linked thrown ones (`VoxelWorld::get_warp_pull_m_per_s`,
  along the geodesic to each, across the body's up), to the walk: faster toward a warp, slower away, a slow drift
  standing still. Bots step by the same rule.
- **Wind**: the map's `atmosphere.wind` is a refractive-index field, ln n = k d, where d is the signed hyperbolic
  distance (m) to the plane through the origin perpendicular to `direction`, and k is
  `log_index_gradient_per_m` (0.00144 /m). Light follows the ray equation, so it curves toward higher n, i.e.
  downwind, with curvature k per meter. Rays and the laser integrate it kick-drift-kick in chords of `segment_m`
  (48 m; 12 m looked the same and cost 5 ms more a frame): half a kick by ∇ln n's part across the ray, a geodesic chord, half a kick. Inside wormhole mouths the
  wind's gradient adds to the mouth's own. The Metal shader (`wind_gradient`, `wind_kick`) and the CPU laser
  (`WindField` in `src/voxel_world.rs`) use the same rule. Dust's `direction` is the map's x, which is the
  Fermi v axis, so the wind blows across the line between the spawns.
- **Wind strength** (`sandstorm.wind_per_map_wind` in `server_settings.yaml`, W-158, per-map game settings): the
  map's k times this, so the game master can calm the storm (0) or strengthen it; it also scales the drift of a
  shot's dust.
- **Gusts** (`atmosphere.wind.gust`): the wind keeps one direction and only its strength changes, by 1 plus
  `strength_frac` (20%) times the mean of three sine waves with phases from `seed`, periods of 5, 7 and 11 s and
  wavelengths of a few `size_m` (3000 m, so almost the same everywhere on the map). Every window and the server
  evaluate them at the shared UTC clock, so lasers, views, fog and streaks agree (`two_worlds_agree_on_the_gusts`).
- **Laser bend** (`atmosphere.wind.laser_gradient_per_view_gradient`, Dust: 2.4): the laser feels 2.4 times the
  light's wind, so it visibly curves downwind: fired across the wind it lands about 3 m downwind of the
  crosshair at 50 m and about 8 m at 80 m (light alone bends 2 m and 6 m there). The window draws the beam and
  the server decides hits from the same `trace_laser` path. Maps without the key bend the laser like light, so it
  lands on the crosshair.
- **Storm sky** (`paint_storm_sky` in the shader, only for rays that leave the world): haze bands drifting
  downwind, a darker brown storm wall on the horizon and faded mesa silhouettes below it. Nothing in the world
  changes; it costs 0 to 1.3 ms a frame (noisy on this VM).
- **Sandstorm**: `atmosphere.fog` replaces the haze: colour, start (4 m), e-folding distance (60 m) and the most
  it hides (94%). Rays stop 5 e-foldings past the start (304 m), where the fog hides all but 0.7%.
- **Blowing sand** (`blow_sand` in the shader, on maps with fog; strengths in the map's `atmosphere.sand`, a map
  without it keeps 30% gusts and 15% streaks): gusts, value noise 40 m long, 14 m wide and 6 m tall, drifting
  downwind at 14 m/s, thicken or thin the fog by up to `gust_frac` (Dust: 50%); sheets 28 m by 5 m by 2.2 m
  within 70 m add up to `sheet_frac` (35%) more fog. Gusts thicken the fog where each ray is halfway to its end
  (or 60 m out); thin streaks (4 m by 8 cm, `streak_frac` 45%) in three layers from 2.5 m in front of the camera drift the same way. The
  clock is the window's sky time, so `--bench-views` pictures are still frames. Cost: about 1 ms a frame.
- **Ground hits** (`src/dust.rs`, the hyperbolic demo's `DustBurst`): on a map with fog, grains take 80% of the
  fog's colour and mist puffs 120% of it, and puffs drift downwind at 3 m/s.
- **Natural props** (dunes, rocks, debris): `tools/make_dust_map.py` jitters each by up to 1.5 m, 0.3 rad and 12% in
  size from seed 121 (named in the map's `source`), and mirrors the result onto the other team's half.
- **Laser colour**: the map's `appearance.laser_color` (Dust: golden white, [1, 0.8, 0.3]) is the one colour
  every team's laser takes; a map without it keeps the team colours.
- **Map page** (held `Tab`, or `Esc` then E) and minimap: `make make-map-picture MAP=dust` (`--make-map-picture`) renders the map
  straight down along the vertical geodesics from 60 m, with sun shadows, no fog or wind, 3×3 samples a pixel,
  into `map_pictures/dust_r50m.png` (committed, 0.8 MB). The window reads it at start-up and draws the labels and
  live marks over it, so it costs nothing a frame; without the file (or at another curvature) it falls back to the
  drawn map. The sun keeps one direction in the map's Fermi frame there, so shadows fall one way as in a photo. The
  spokes and the round patch in the middle are the ground's hyperbolic tiling.
- **Ground height**: `terrain.rs` bakes the whole terrain (crater wall, rim noise, 3 relief octaves, mounds) into
  a 0.25 m grid at start-up (about 0.1 s, 31 MB) over everywhere a ray from inside the boundary reaches before
  the fog ends it, |u| ≤ 382 m and |v| ≤ 318 m. Walking, laser hits and the shader's terrain marches read it
  bilinearly in the same way, and the shader's normals come from the unbaked height, so the light shows no
  grid (DECISIONS.md, item 18).
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
- Since then the renderer starts at render scale 0.6 with 3 terrain octaves (DECISIONS.md, item 17): at 2560x1440
  every view but `wormhole`, `wormhole_exit` and `laser_into_wormhole` (17.6–19.3 ms) has a median under 16.7 ms.
- With the terrain's height baked (DECISIONS.md, item 18), on an Apple M3 Pro (18-core GPU, macOS 15.7.3) at
  2560x1440, best of 4 alternating rounds of 30 frames: every view's median and 99th percentile GPU time is under
  16.7 ms. `wormhole` 14.5 → 10.3 ms, `wormhole_exit` 14.3 → 10.3, `laser_into_wormhole` 20.5 → 15.8 (p99
  16.4), the other views 5.4–10.3 ms. The pictures are the same.
- With Lunar Surface's renderer speed-ups (DECISIONS.md, item 20), under the agents' exclusive GPU lock, best of 3
  alternating rounds of 20 frames: at render scale 1 (full resolution) and 2560x1440 every view's median and p99 is
  at most 16.7 ms but `laser_into_wormhole` (25.2 → 21.0 ms); the others take 10–16.3 ms (before: 12–20.1). At the
  default 0.6, every view is under 11.6 ms. At 3440x1440 and scale 1, 13 of 24 views are under 16.7 ms, the rest
  17–25 ms. The pictures are the same.
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
