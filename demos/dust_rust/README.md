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
  (12 m): half a kick by ∇ln n's part across the ray, a geodesic chord, half a kick. Inside wormhole mouths the
  wind's gradient adds to the mouth's own. The Metal shader (`wind_gradient`, `wind_kick`) and the CPU laser
  (`WindField` in `src/voxel_world.rs`) use the same rule, so the laser lands where the crosshair shows.
- **Sandstorm**: `atmosphere.fog` replaces the haze: colour, start (4 m), e-folding distance (60 m) and the most
  it hides (94%). Rays stop 5 e-foldings past the start (304 m), where the fog hides all but 0.7%.
- **Textures**: `aerial_sand` (sand), `gravelly_sand` (dark sand), `cliff_side` (sandstone),
  `large_sandstone_blocks` (ruins), from `tools/download_textures.sh` into `assets/textures/` (git-ignored).

## Results (2026-11-08)

Run on a virtual Apple M4 Pro Mac (macOS, Metal), so frame times are slower than on the user's Mac: the
reference Lunar Surface level takes 21-71 ms per frame at the same size on this machine.

- `cargo build --release --locked`, `cargo test --release` (213 pass), `cargo clippy --all-targets`: clean.
- `make validate-maps` in `common/maps/`: dust, house_demo, lunar_surface valid.
- End to end: `--serve --level-result-file` with a blue `--bot-objective consoles` bot wrote
  `{"winner": "blue"}`; with two bots (red holding consoles, blue shooting) it wrote `{"winner": "red"}`.
- `--bench-views --width-px 2560 --height-px 1440` (Metal, median ms): blue_spawn 24.1, red_spawn 24.9,
  overview 27.7, middle 16.0, salt_flats_long 22.9, cross_wind 17.4, wormhole 38.8, wormhole_exit 38.7,
  blue_pylons 24.0, arch 17.9, overlook 22.9, blue_kazemat 31.2, laser_into_wormhole 53.1. Not yet under
  16.7 ms on this machine; not yet measured on the user's Mac, nor with `--play-route`.
