# Hyperbolic wormholes in Rust

A Rust port of the WebGL demo in [`../hyperbolic_wormholes/`](../hyperbolic_wormholes/README.md), to see how
fast its ray tracer runs natively. It is the same scene and the same light paths (hyperbolic space H³, R = 8, three
wormholes), with gravity and a laser added, drawn either on the GPU with Metal (the default) or on every CPU core.
It is a demo for measuring speed, so it does no more than that.

## Play

```bash
make run-demo
make run-demo RENDERER=cpu
```

- Click to capture the mouse (`Esc` releases it; `Esc` again quits).
- `W`/`A`/`S`/`D` walk, `Shift` sprint, `Space` jumps (hold it for the jetpack), `H` toggles flying (then `Space`/`E`
  and `Q` go up and down). You collide with the ground only.
- Your path bends near a mouth like light does, and the view turns with it, so walking or flying at a mouth's
  dark disk takes you through. `G` aims through the exact middle of the mouth nearest the crosshair; `F` keeps
  moving forward.
- `L` toggles the laser: a light ray from your hand that bends near mouths and goes through them. The HUD says what
  it hits.
- `R` respawns. `-` and `=` lower and raise the render resolution. Metal starts at the window's full resolution and,
  on frames that take more than about 14 ms of GPU time (close to a mouth), ray-traces fewer pixels and upscales
  them with MetalFX, down to half per side, until you press `-` or `=`. The CPU starts at 0.35 per side.
- The HUD shows the frame rate, the render time and, with Metal, the GPU's own time and the render scale; with Metal
  it is in the window's title.
- `--width-px` and `--height-px` set the window's starting size (`cargo run --release -- --width-px 2560
  --height-px 1400`); `--render-scale 1` turns upscaling off.

The three wormholes start ahead on the left (to the house door), behind you by the blue pylon (to the red pylon
about 50 blocks down the rails), and far left (to high above the rails, where you fall back to the ground).

## Measure

```bash
make bench-views WIDTH_PX=2560 HEIGHT_PX=1600
make bench-views RENDERER=cpu
make run-autopilot
```

- `bench-views` renders seven fixed views without a window (spawn, aimed at mouth 1A, 2.5 blocks from its centre,
  0.8 blocks from its throat, above the rails, high above spawn, and the laser through 1A), times each frame, and
  writes the median and fastest times and a PNG of each view to `.artifacts/bench_views/<run_id>/`.
- `run-autopilot` plays a scripted route without a window at a fixed 1/60 s step: walk into 1A, jetpack and land,
  fly into 2A, walk into 3A and fall from above the rails, and fire the laser through 1A. It writes every frame's
  time, render time, height and events to `frames.csv`, every 30th frame as a PNG, and a summary of each leg to
  `.artifacts/autopilot/<run_id>/`.
- Both render at full resolution unless given `--render-scale` (`0.5` ray-traces half the pixels per side and
  upscales them with MetalFX), and add `--json` and `--label <name>` when run directly (`cargo run --release --
  --help`).
- `--sampling every-pixel` makes Metal trace every pixel instead of interpolating where it can (see below).
- `--exit-after-s 8` opens the real window, plays for 8 seconds without input, closes it and prints the frame rate,
  GPU time and render scale.
- `--play-route` plays the scripted route in the real window, one 1/60 s step per frame, then closes it and prints
  the frame times (median, 99th percentile, worst, how many took longer than 1/60 s) and the render scale it used
  (`cargo run --release -- --play-route --width-px 2560 --height-px 1440`). `--route mouth-tour` plays a harder
  tour instead: the eye circles each of the six mouths from 5 blocks to 0.45 (inside its jump sphere, just outside
  the throat) and back, while the view swings up to 46° sideways and 23° up and down.

## How it works

- `geometry.rs`: Lorentz-matrix maths in double precision (the hyperboloid model, as in the WebGL demo).
- `world.rs`: the scene: hyperbolic boxes grouped in clusters, and the wormholes' mouths with their gluing maps.
- `scene_data.rs`: each frame, packs the scene into rows of float 4-vectors, one row in the camera's coordinates and
  one per mouth, so a ray that goes through a mouth stays in small coordinates (the same layout as the WebGL demo's
  float texture).
- `render_cpu.rs`: the ray tracer, a port of the WebGL fragment shader, run over every core with rayon. It also
  draws the laser: each straight piece of a ray is tested against the beam's pieces in Klein coordinates, where
  geodesics are straight lines.
- `render_metal.metal` and `render_metal.rs`: the same ray tracer in Metal Shading Language, compiled when the
  program starts, with three shortcuts of its own (below). Each frame copies the scene rows to the GPU, ray-traces
  into a texture, upscales it with MetalFX when the render scale is below 1, and copies it into the window's Metal
  layer, without waiting for the GPU.
- `physics.rs`: walking, gravity, the jetpack and flying, crossing throats, aiming, and tracing the laser (with the
  renderer's own ray tracer, so the laser hits exactly what you see).
- `main.rs`: the command line, the SDL3 window and HUD, and the benchmark and autopilot runs.

Distances are compared through q = 4 sinh²(d/2), computed from the difference of two points, instead of the
distance itself, which saves most of the `asinh` calls that dominated the first CPU profile. And a ray only checks
whether it is inside a throat when the bending sum, which already measures every mouth, finds one that close, which
made the Metal shader about 30% faster.

Most of the rest of the speed-up comes from not testing boxes a ray can't reach. Each cluster of boxes (the house,
a pylon, a tree) has a bounding box in its own frame, and a ray skips the whole cluster unless it reaches that box.
The Metal shader also keeps, per ray, a bit mask of the clusters within 3 blocks and how far the ray can go before
another could matter, so most steps test no boxes at all. All of these leave every pixel the same; see `DECISIONS.md`
(D12, D17, D18) for what each gained.

The Metal shader then takes three shortcuts the CPU renderer doesn't, so the CPU stays the reference:

- Adaptive sampling: it traces every fourth pixel each way, fills in every second pixel by interpolation where
  those samples hit the same surface and their hit points vary almost linearly (tracing the rest), then does the
  same for the remaining pixels. Interpolated pixels are still shaded one by one, so textures stay sharp, and
  whatever can't be interpolated is traced exactly in separate, densely packed passes (D21).
- Fourth-order (Runge-Kutta) steps between 3 and 10 throat radii of a mouth, three times longer than the midpoint
  steps they replace and as accurate (D22).
- A jump across each mouth's inner sphere (1.6 blocks in radius): the mouth's own field there looks the same about
  every axis through its centre, so a ray's path depends only on its angle (and, for the eye inside the sphere, on
  the eye's distance). Tables traced once at start-up give where each ray comes out, through the throat or not,
  and how much a uniform extra field would move it; the other mouths' bending is added from that, to first order
  (D23). Against a reference with four times smaller steps, the images are as close as with full stepping.

## Results

Run on an Apple M3 Pro (6 performance and 6 efficiency cores), macOS 15.7.3, Rust 1.98.1, release build, on a
2560×1440 display at 1× scaling. The machine was also running other work, so treat the times as rough.

In the real window at full-screen size (macOS makes it 2560×1336), playing a route (`--play-route`, after 5 warm-up
frames; the automatic scale is the default):

| Route and render scale | Average | Median frame | 99th percentile | Frames over 1/60 s | At full resolution |
| --- | ---: | ---: | ---: | ---: | ---: |
| scripted legs, automatic (2 runs) | 166 fps | 5.5 ms | 14.0 ms | 0 of 719 | 95.9% of frames (never below 0.80) |
| scripted legs, always full | 164 fps | 5.5 ms | 16.9 ms | 8 of 719 (worst 18.7 ms) | 100% |
| mouth tour, automatic (2 runs) | 113 fps | 8.4 ms | 14.9 ms | 2 and 5 of 1795 (worst 18.6, 36.9 ms) | 89% of frames (never below 0.70) |

- The frames that drop below full resolution are those within about a block of a mouth, looking past it, where
  neighbouring rays scatter in every direction and the GPU's thread groups diverge (about 24 ms at full resolution).
  The automatic scale drops at once to the scale its time model predicts, by up to a fifth a frame, so only the
  first frame of a sudden spike is late.
- Standing at spawn (`--exit-after-s 8`), the window runs at 129 fps at full resolution.

Median milliseconds per frame at full resolution (`make bench-views`, 20 frames for WebGL and Metal, 3 to 5 for the
CPU):

| View | WebGL 1280×800 | Metal 1280×800 | CPU 1280×800 | WebGL 2560×1600 | Metal 2560×1600 | CPU 2560×1600 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| spawn | 35 | 3.7 (23) | 838 | 106 | 10 (81) | 3484 |
| aimed at mouth 1A | 34 | 3.7 (18) | 783 | 99 | 9.6 (64) | 3027 |
| 2.5 blocks from 1A | - | 7.5 | - | - | 22 | - |
| 0.8 blocks from 1A's throat | 75 | 3.8 (37) | 1403 | 235 | 11 (134) | 5719 |
| above the rails | 9 | 1.9 (6) | 441 | 32 | 5.7 (25) | 1776 |
| high above spawn | 33 | 3.0 (15) | 631 | 95 | 8.5 (52) | 2624 |
| laser through 1A | - | 4.5 (25) | 853 | - | 12 (88) | 3510 |

- In brackets: Metal before the third layer's changes (PR 70).
- WebGL is the browser demo at the same drawing-buffer sizes, in the Claude desktop app's built-in browser (Chromium
  152, ANGLE on Metal), timed with `readPixels`. Its scene differs a little: its mouths and eye are 4.5 blocks up,
  not 1.7, and it has no laser.
- So Metal is now 5 to 22 times faster than WebGL, and 3 to 12 times faster than before the third layer. The CPU,
  which got the bounding boxes but none of Metal's shortcuts, is 190 to 530 times slower than Metal.
- Tracing every pixel (`--sampling every-pixel`), Metal at 2560×1600 takes 24 ms at spawn, 32 ms 2.5 blocks from 1A
  and 27 ms at the throat.

How close the images are, at 2560×1600, against a reference traced like PR 70 but with steps eight times smaller
(itself within 0.03 to 0.2 pixel, median, of one with four times smaller steps, and 1 pixel at the throat). The first numbers are how far image
features moved, in pixels, from phase correlation of 48×48 textured patches (median, 90th percentile); then the
share of pixels that changed by more than 30 of 765, and the structural similarity (SSIM, on brightness):

| View | PR 70 | Now |
| --- | --- | --- |
| spawn | 3.2 px, 5.8 px; 12.3%, 0.826 | 3.2 px, 6.1 px; 11.6%, 0.833 |
| aimed at mouth 1A | 3.1 px, 5.4 px; 9.7%, 0.858 | 3.0 px, 5.6 px; 9.3%, 0.863 |
| 2.5 blocks from 1A | 2.2 px, 7.1 px; 17.0%, 0.699 | 2.8 px, 8.1 px; 18.4%, 0.702 |
| 0.8 blocks from 1A's throat | 2.5 px, 9.1 px; 10.1%, 0.865 | 2.4 px, 7.1 px; 8.5%, 0.873 |
| above the rails | 1.0 px, 2.3 px; 1.8%, 0.957 | 1.0 px, 2.3 px; 1.8%, 0.957 |

(The percentages and SSIM are against the four-times reference.) So the shortcuts leave the light paths as exact as
PR 70's and the WebGL demo's: their step sizes put features about 3 pixels (0.14° of view) from the converged
paths, and the jump, the RK4 steps and adaptive sampling stay within that. Adaptive sampling on its own moves
features by under 0.01 pixel (median; SSIM 0.98 to 0.998 against tracing every pixel). Frames the window upscales
with MetalFX keep features in place (0.2 to 0.4 pixel) but lose fine detail (SSIM 0.85 to 0.96 at 0.8 scale).
Smaller steps would bring the paths closer to the reference at a cost; see D13 in `DECISIONS.md`.

The scripted route (`make run-autopilot`) completes every leg, with both renderers and the same timings: through 1A
after 1.6 s of walking, a 1 s jetpack press peaks at 12 blocks and lands 1.1 s after release, through 2A after 1.9 s
of flying, through 3A after 2.4 s of walking and down from 12 blocks in 0.6 s, and the laser through 1A to the
ground beyond.

See [`DECISIONS.md`](DECISIONS.md) for how the benchmark was chosen.
