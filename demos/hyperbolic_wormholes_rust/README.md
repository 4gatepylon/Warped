# Hyperbolic wormholes in Rust

A Rust port of the WebGL demo in [`../hyperbolic_wormholes/`](../hyperbolic_wormholes/README.md), to see how
fast its ray tracer runs natively. It is the same scene and the same light paths (hyperbolic space H³, R = 8, three
wormholes), with gravity and a laser added. It is a demo for measuring speed, so it does no more than that.

## Play

```bash
make run-demo
```

- Click to capture the mouse (`Esc` releases it; `Esc` again quits).
- `W`/`A`/`S`/`D` walk, `Shift` sprint, `Space` jumps (hold it for the jetpack), `H` toggles flying (then `Space`/`E`
  and `Q` go up and down). You collide with the ground only.
- Your path bends near a mouth like light does, and the view turns with it, so walking or flying at a mouth's
  dark disk takes you through. `G` aims through the exact middle of the mouth nearest the crosshair; `F` keeps
  moving forward.
- `L` toggles the laser: a light ray from your hand that bends near mouths and goes through them. The HUD says what
  it hits.
- `R` respawns. `-` and `=` lower and raise the render resolution (the CPU starts at 0.35 of the window's pixels per
  side).

The three wormholes start ahead on the left (to the house door), behind you by the blue pylon (to the red pylon
about 50 blocks down the rails), and far left (to high above the rails, where you fall back to the ground).

## Measure

```bash
make bench-views WIDTH_PX=2560 HEIGHT_PX=1600
make run-autopilot
```

- `bench-views` renders six fixed views without a window (spawn, aimed at mouth 1A, 0.8 blocks from its throat,
  above the rails, high above spawn, and the laser through 1A), times each frame, and writes the median and fastest
  times and a PNG of each view to `.artifacts/bench_views/<run_id>/`.
- `run-autopilot` plays a scripted route without a window at a fixed 1/60 s step: walk into 1A, jetpack and land,
  fly into 2A, walk into 3A and fall from above the rails, and fire the laser through 1A. It writes every frame's
  time, render time, height and events to `frames.csv`, every 30th frame as a PNG, and a summary of each leg to
  `.artifacts/autopilot/<run_id>/`.
- Both add `--json` and `--label <name>` when run directly (`cargo run --release -- --help`).

## How it works

- `geometry.rs`: Lorentz-matrix maths in double precision (the hyperboloid model, as in the WebGL demo).
- `world.rs`: the scene: hyperbolic boxes grouped in clusters, and the wormholes' mouths with their gluing maps.
- `scene_data.rs`: each frame, packs the scene into rows of float 4-vectors, one row in the camera's coordinates and
  one per mouth, so a ray that goes through a mouth stays in small coordinates (the same layout as the WebGL demo's
  float texture).
- `render_cpu.rs`: the ray tracer, a port of the WebGL fragment shader, run over every core with rayon. It also
  draws the laser: each straight piece of a ray is tested against the beam's pieces in Klein coordinates, where
  geodesics are straight lines.
- `physics.rs`: walking, gravity, the jetpack and flying, crossing throats, aiming, and tracing the laser (with the
  renderer's own ray tracer, so the laser hits exactly what you see).
- `main.rs`: the command line, the SDL3 window and HUD, and the benchmark and autopilot runs.

Distances are compared through q = 4 sinh²(d/2), computed from the difference of two points, instead of the
distance itself, which saves most of the `asinh` calls that dominated the first profile.

## Results

Run on an Apple M3 Pro (6 performance and 6 efficiency cores), macOS 15.7.3, Rust 1.98.1, release build. The machine
was also running other work, so treat the times as rough.

Median milliseconds per frame (`make bench-views`):

| View | CPU, 1280×800 | CPU, 2560×1600 |
| --- | ---: | ---: |
| spawn | 957 | 4212 |
| aimed at mouth 1A | 770 | 3238 |
| 0.8 blocks from 1A's throat | 1897 | 8980 |
| above the rails | 360 | 1485 |
| high above spawn | 747 | 2487 |
| laser through 1A | 1474 | 5231 |

The CPU is far too slow to play at full resolution: at the default 0.35 scale of a 2560×1600 window (896×560) the
spawn view takes about 470 ms. The GPU version of the same ray tracer is what makes it playable.

The scripted route (`make run-autopilot`, 320×200) completes every leg: through 1A after 1.6 s of walking, a 1 s
jetpack press peaks at 12 blocks and lands 1.1 s after release, through 2A after 1.9 s of flying, through 3A after
2.4 s of walking and down from 12 blocks in 0.6 s, and the laser through 1A to the ground beyond.

See [`DECISIONS.md`](DECISIONS.md) for how the benchmark was chosen.
