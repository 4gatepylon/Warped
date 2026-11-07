# Decisions: Rust port of the hyperbolic wormholes demo

- **Question:** How fast does the WebGL hyperbolic wormholes demo's ray tracer run natively, in Rust on the CPU and
  then with a Metal (MSL) shader, especially at full resolution, where the browser version is too slow? And how
  fast can Rust + MSL be made?
- **Given:** a Rust version, then Rust + MSL on top of it, as a stack of two PRs; one crate whose final version
  selects the renderer with a flag; SDL3 for the window and objc2-metal for Metal; walking with gravity, a jetpack
  and a toggle to fly; a laser to turn on, as in the earlier laser demos; no balls or other extras; a demo for
  measuring speed, not a general engine; benchmarking and scripted playtesting done by Claude before a manual test.
  Then a third layer that makes Rust + MSL as fast as it can, free to use other native app code, staying on this
  Mac, reaching at least 60 fps locally at full-screen size while looking realistic and high-fidelity, with
  heuristics allowed for speed.
- **Outcomes:** milliseconds per frame for fixed views at fixed resolutions, per renderer, compared with the WebGL
  demo; and whether a scripted route through every wormhole works, and at what frame rate. The CPU is expected to be
  much slower than the GPU; the question is by how much, and how much Metal gains over WebGL.

## Scope

### D1. Port the WebGL shader line by line, in 32-bit floats

- **Choice:** the CPU renderer is the WebGL fragment shader in Rust, with the same scene rows (camera row plus one
  row per mouth) and the same constants, so its images match and it is a reference for the Metal shader.
- **Alternatives:** a 64-bit CPU tracer without rows, which would be simpler but no longer the same computation.
- **Rationale:** the benchmark compares the same work on different hardware.

### D2. Replace `asinh` distance checks with q = 4 sinh²(d/2)

- **Choice:** the first profile spent about 38% of CPU time in `asinhf`; mouth, throat and box checks now compare q
  or sinh against thresholds, which gives pixel-identical images.
- **Alternatives:** keep the WebGL code exactly. A per-ray cache of nearby box clusters was also tried and removed:
  it gave no measurable gain and the Metal shader would have had to carry it too.
- **Rationale:** the same change helps the Metal shader, and it changes no pixels.
- **Revisit if:** the WebGL demo should be compared on exactly the same code; it hasn't been changed.

### D3. Drop what the benchmark doesn't need

- **Choice:** no placing or removing mouths, no R or mouth-size sliders, fog always on.
- **Alternatives:** full parity with the WebGL demo.
- **Rationale:** you asked for a demo that only needs to measure speed.

### D4. Mouths at standing eye height, and walking turns with the path

- **Choice:** the three entrances (and the house and red pylon exits) stand at eye height, 1.7 blocks, so you can
  walk into them; the exit above the rails is 12 blocks up. While walking, the view turns left and right with the
  path's bending, as flying turns fully, so walking at a mouth's dark disk takes you through.
- **Alternatives:** mouths at 4.5 blocks as in the WebGL demo, reached by jetpack only; walking that ignores the
  bending, which then has to hit the 0.34-block throat exactly (the first scripted run missed 3A that way).
- **Rationale:** with gravity, walking is the main way to move.

### D5. Gravity, jump and jetpack strengths

- **Choice:** 30 blocks/s² down (1.6× while falling, as in the laser gravity demos), a 7 blocks/s jump, and a 34
  blocks/s² jetpack (a net 4 upward). You collide only with the ground.
- **Alternatives:** the first try (9 blocks/s jump, 45 blocks/s² jetpack) took a one-second press to 27 blocks.
- **Rationale:** arbitrary; tuned so a one-second press reaches about 10 blocks.
- **Revisit if:** the manual test finds it floaty or stiff.

### D6. The laser is a light ray drawn by the ray tracer

- **Choice:** the beam starts at the hand, follows the same light paths as the image (so it bends and goes through
  wormholes), stops at what it hits or after 150 blocks, and is drawn by testing each piece of each pixel's ray
  against the beam's pieces in Klein coordinates.
- **Alternatives:** drawing the beam as screen-space lines, which can't show it through a wormhole.
- **Rationale:** it matches how the earlier laser demos drew beams.

### D11. Metal draws straight into the window

- **Choice:** in the window, Metal ray-traces into a GPU texture, MetalFX upscales it, and it is copied into the
  window's `CAMetalLayer` drawable and presented, without waiting for the GPU or copying pixels back. The HUD moves
  into the window's title.
- **Alternatives:** the second layer's way: write RGBA pixels into a shared buffer, wait for the GPU, copy them out
  and hand them to SDL's renderer, which also draws the HUD.
- **Rationale:** that wait and the copies were about half a millisecond at 1280×800, small next to the 6 to 37 ms of
  GPU work then, but the third layer brought the GPU work down to a few milliseconds.

### D12. Check for throats only when the bending sum finds a mouth that close

- **Choice:** the bending sum already measures every mouth each step, so a ray only runs the throat check when the
  nearest mouth is inside the throat's radius; in both renderers. Every pixel stays the same, and Metal got about 30%
  faster.
- **Alternatives:** the separate check each step that the WebGL demo does.
- **Rationale:** it was pure overhead.

### D13. Larger integration steps (needs input)

- **Choice:** not applied. Measured before D21 to D23: steps of 0.18 and 0.40 of the nearest mouth's distance
  (instead of 0.12 and 0.25) took the spawn view from 30 to 21 ms, and 0.25 and 0.50 took it to 17.5 ms (Metal,
  2560×1600; the throat view went from 51 to 37 and 29 ms). They move edges inside the lensed areas by about a pixel: 15% and
  22% of the spawn view's pixels change by more than 30 of 765. For scale, today's steps already differ that much
  from steps four times smaller on 12% of the spawn view's pixels.
- **Alternatives:** turning one of them on by default, or offering it as a quality setting.
- **Rationale:** it changes the picture, so it is your call.

### D14. Tried and dropped

- **Choice:** each of these was measured and removed:
  - a 300-step cap per ray (instead of 700): no change and no gain in these views;
  - 16×16 or 32×32 thread groups (instead of 8×8): slower;
  - the scene rows in Metal's `constant` address space, or the mouths' centres kept in registers: no gain, and
    slower;
  - compiling the laser code out when the laser is off, or skipping the glow's `exp`s for far beam pieces: no gain;
  - doing a full pass over the clusters in sinh space with one `asinh` instead of three per cluster: 4% slower;
  - candidate windows (D17) of 1 to 6 blocks gave the same times, and 12 and 24 blocks were slower;
  - growing the midpoint steps with distance beyond 10 throat radii (as √distance, up to 0.5 or 0.8 of it): 10% faster
    at spawn, but 16 to 22% of its pixels changed against the reference instead of 12%;
  - summing only the nearest mouth's bending inside its near zone: slower, and 24% of the throat view changed;
  - kick-drift-kick (Heun-like) steps that reuse each step's end bending as the next step's start, so each step
    sums the bending once instead of twice: 17% faster at spawn, but its error is larger than the midpoint
    method's. Against a reference with four times smaller steps, 43% of the throat view's pixels changed by more than
    30 of 765 (10% for the midpoint method), and crossing the throat before the end kick lost rays at the throat's
    edge instead.
- **Alternatives:** none.
- **Rationale:** each was either no faster, or traded accuracy no better than larger steps (D13) do.

### D17. Skip boxes a ray can't reach

- **Choice:** in both renderers, each cluster of boxes (the house, the pylons, each tree) is bounded by a box in its
  own frame instead of a sphere, so a ray skips the whole cluster unless it reaches that box before what it has
  already hit. The bounding box's half sizes are exact bounds, from each box's corners on one side of a plane and a
  linear bound on the other. In the Metal shader, a ray also remembers which clusters are within 3 blocks of it, and
  how far it can go before any other cluster could be; it only looks at clusters the row's distance order says it
  could have reached; and it skips clusters it is climbing away from above. Every pixel stays the same. At 2560×1600
  (spawn view), these took Metal from 81 to 30 ms, in this order: line culling of cluster spheres, 65 ms; the
  candidate mask, 47; the climbing skip, 46; the distance order, 44; bounding boxes (which replaced the line culling),
  30. The house was the main cost: its 22 boxes sat in a sphere 11 blocks across next to spawn.
- **Alternatives:** splitting the house into smaller clusters; a deeper hierarchy.
- **Rationale:** after the bounding boxes, a ray tests about 1.5 boxes per frame at spawn (77 before), so more
  structure would gain little; turning the box tests off entirely gains only another 10 ms at 2560×1600, which goes
  to SIMD groups waiting on their slowest ray.

### D18. Group the laser beam's points in fours

- **Choice:** the beam's points are grouped in chunks of 4 segments (up to 64 chunks) instead of 16, so each chunk's
  bounding sphere is smaller and fewer rays test its segments. Every pixel stays the same; the laser view at
  2560×1600 went from 55 to 43 ms (before D17's bounding boxes).
- **Alternatives:** 8 segments (44 ms) or 2 (48 ms).
- **Rationale:** measured, the fastest.

### D19. Upscale with MetalFX only when a frame is too slow

- **Choice:** Metal can ray-trace fewer pixels and upscale them with MetalFX's spatial scaler (`--render-scale`). In
  the window it starts at full resolution, and each frame moves the scale a little toward 14 ms of GPU time, between
  0.5 and 1, in steps of 0.05 of the window's size; `-` and `=` set it by hand. `--bench-views` and `--autopilot`
  render at full resolution unless `--render-scale` is given.
- **Alternatives:** always full resolution; a lower floor; MetalFX's temporal scaler, which looks better but needs
  motion vectors and jittered frames; the first version's start at 0.5 with a 12 ms target, updated four times a
  second.
- **Rationale:** with D21 to D23, most frames at 2560×1440 take 4 to 12 ms at full resolution, so upscaling is only
  for the frames close to a mouth (up to about 30 ms), where its grain in squeezed textures shows most; 14 ms leaves
  room for the CPU's part of a 60 Hz frame, and the 0.5 floor keeps the image from getting coarse.
- **Revisit if:** a 120 Hz display needs a lower target, or the grain near mouths is too visible.

### D21. Trace every other pixel, and interpolate where that is exact enough

- **Choice:** `--sampling adaptive` (the default) traces every other pixel each way, marks each 2×2 cell smooth when
  the twelve samples around it ended on the same surface, in the same row, without laser glow, and their second
  differences stay under 1.6 times the first (an interpolation error of about 0.4 pixel), interpolates the hit point
  of the other three pixels and shades them at full resolution, and traces the rest in a separate, densely packed
  pass. `--sampling every-pixel` traces every pixel.
- **Alternatives:** a threshold of 0.4 (0.1 pixel; the near view 29 instead of 20 ms) or 0.8; tracing the rest in the
  same pass (about 9 ms slower at spawn, as a thread group waits on its slowest ray); interpolating each sample's
  state where it leaves a mouth's near zone, which covered only 4 to 6% of cells and slowed the sample pass.
- **Rationale:** against tracing every pixel, at most 0.6% of pixels change by more than 30 of 765 (the near view;
  0.1% elsewhere), and textures stay sharp because every pixel is still shaded; spawn at 2560×1600 goes from 22 to
  10.5 ms.

### D22. Fourth-order steps away from the throat

- **Choice:** between 3 and 10 throat radii of a mouth, a ray takes classical Runge-Kutta steps of 0.5 of the
  distance to the mouth's centre, instead of midpoint steps of 0.12; within 3 throat radii it keeps midpoint steps.
- **Alternatives:** RK4 all the way to the throat (its stages land inside the throat sphere, and 21% of the throat
  view's pixels changed by more than 30 against a reference with four times smaller steps, however small the
  steps); Heun-like steps (D14); larger midpoint steps (D13).
- **Rationale:** it is at least as accurate as the midpoint steps against that reference (spawn 11.5% of pixels
  changed against 12.3%), and about 10% faster.

### D23. Cross each mouth's inner sphere in one jump (a heuristic)

- **Choice:** within 4.5 throat radii (1.6 blocks) of a mouth, which holds no boxes or ground (a test checks it),
  the field is taken to be that mouth's alone, which looks the same about every axis through its centre, so a ray's
  path there depends only on its angle. A table, traced once on the GPU with fine steps, gives for 4096 entry angles
  where a ray leaves the sphere again, through the throat or not, and how far it went; a ray that reaches the
  sphere jumps there in one step. Steps that would end deep inside the sphere are redone shorter, so rays enter it
  near its edge. When the eye itself is inside a sphere, a second table for the eye's distance is traced each frame
  (about 2 ms).
- **Alternatives:** stepping through (the near view 65 ms instead of 32 at 2560×1600, tracing every pixel); giving
  the other mouths' bending as two half kicks around the jump, which made the near view worse (40% of pixels
  changed instead of 27%).
- **Rationale:** it makes looking into and walking through a mouth about twice as fast. It is an approximation: the
  other entrances, 3 to 5 blocks away, still bend the rays a little near the throat, so the image seen through a
  throat comes out a few per cent larger or smaller. Against the fine-step reference, 27% of the near view's pixels
  and 17% of the throat view's change by more than 30 of 765, against 17% and 10% for midpoint steps without the
  jump; ignoring the other mouths in the same sphere without the jump gives the same 26% and 21%. Elsewhere the
  images are as close to the reference as before.
- **Revisit if:** the size of what you see through a throat matters, or mouths are placed closer together.

## Baselines

### D15. WebGL at the same drawing-buffer sizes

- **Choice:** the browser demo from PR 58, unchanged, in the Claude desktop app's built-in browser (Chromium 152,
  ANGLE on Metal), with its drawing buffer set to 1280×800 and 2560×1600 and frames timed with `readPixels`. Its
  scene differs a little: its mouths and eye are 4.5 blocks up, and it has no laser.
- **Alternatives:** moving the WebGL demo's mouths to match first.
- **Rationale:** it is the version you found too slow.

## Workload

### D7. Seven fixed views

- **Choice:** spawn, aimed at mouth 1A, 2.5 blocks from 1A's centre, 0.8 blocks from 1A's throat, above the rails,
  high above spawn, and the laser through 1A; the same kinds of views as the WebGL timings in PR 58, plus the
  2.5-block view, added in the third layer because walking into a mouth was the slowest part of the route.
- **Alternatives:** a recorded flight.
- **Rationale:** they cover cheap (above the rails), typical (spawn) and worst (at a throat) cases.

### D8. Scripted route

- **Choice:** walk into 1A, jetpack for 1 s and land, fly into 2A, walk into 3A and fall from above the rails, and
  fire the laser through 1A, at a fixed 1/60 s step, each leg ending on its event or a time limit.
- **Alternatives:** random wandering.
- **Rationale:** it exercises every wormhole, gravity, flying and the laser, and checks each one happened.

## Procedure

### D20. Check the window with a timed run

- **Choice:** `--exit-after-s <seconds>` opens the real window, plays without input for that long, closes it and
  prints the frame rate, GPU time and render scale; `--width-px` and `--height-px` set the window's starting size.
- **Alternatives:** only the windowless benchmarks, which leave out presenting and MetalFX in the window.
- **Rationale:** it measures what you see when you play, and needs no screen recording.

### D9. What a frame's time includes

- **Choice:** packing the scene, tracing the laser (when on) and rendering, not showing the image; median and
  fastest of N frames after one warm-up frame (for the CPU, N = 5 at 1280×800 and 3 at 2560×1600; for Metal and
  WebGL, N = 20).
- **Alternatives:** render time only.
- **Rationale:** that is what a frame costs the game loop.

## Environment

### D10. One shared machine

- **Choice:** an Apple M3 Pro, macOS 15.7.3, Rust 1.98.1, release build, while other sessions were also running
  (load averages of 8 to 18). The window runs were on a 2560×1440 display at 1× scaling, so a window's points are
  its pixels.
- **Alternatives:** an idle machine.
- **Rationale:** none was available; differences under about 15% are noise.
- **Revisit if:** two renderers come out within that margin.

## Analysis

### D16. Metal's images are checked against the CPU's

- **Choice:** each benchmark view from both renderers was compared pixel by pixel; they differ only in the clouds and
  block speckles, whose pseudo-random hash (`fract(sin(x) · 43758.5)`) comes out differently on the GPU. The laser,
  the lensing and every surface match.
  In the third layer, writing to a texture rounds where the second layer's byte conversion truncated (at most 1 of
  255 per channel), and D17 and D18 left every pixel the same, on both renderers. D21 to D23 are Metal's alone: the
  CPU renderer still traces every pixel with midpoint steps, so it stays the reference they are measured against
  (with four times smaller steps, in `.artifacts/bench_views/*-ref-fine7`).
- **Alternatives:** a GPU-stable integer hash, so the images match exactly.
- **Rationale:** the noise only decorates; matching it wasn't worth changing the look.
