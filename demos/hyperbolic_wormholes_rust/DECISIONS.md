# Decisions: Rust port of the hyperbolic wormholes demo

- **Question:** How fast does the WebGL hyperbolic wormholes demo's ray tracer run natively, in Rust on the CPU and
  then with a Metal (MSL) shader, especially at full resolution, where the browser version is too slow? And how
  fast can Rust + MSL be made?
- **Given:** a Rust version, then Rust + MSL on top of it, as a stack of two PRs; one crate whose final version
  selects the renderer with a flag; SDL3 for the window and objc2-metal for Metal; walking with gravity, a jetpack
  and a toggle to fly; a laser to turn on, as in the earlier laser demos; no balls or other extras; a demo for
  measuring speed, not a general engine; benchmarking and scripted playtesting done by Claude before a manual test.
  Then a third layer that makes Rust + MSL as fast as it can, free to use other native app code, staying on this
  Mac.
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

- **Choice:** not applied. Steps of 0.18 and 0.40 of the nearest mouth's distance (instead of 0.12 and 0.25) take
  the spawn view from 30 to 21 ms, and 0.25 and 0.50 take it to 17.5 ms (Metal, 2560×1600, after the third layer;
  the throat view goes from 51 to 37 and 29 ms). They move edges inside the lensed areas by about a pixel: 15% and
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

### D19. Upscale with MetalFX, and pick the window's resolution from the GPU time

- **Choice:** Metal can ray-trace fewer pixels and upscale them with MetalFX's spatial scaler (`--render-scale`). In
  the window it starts at 0.5 of the window's pixels per side and adjusts itself each frame toward 12 ms of GPU time,
  between 0.35 and 1; `-` and `=` set it by hand. `--bench-views` and `--autopilot` render at full resolution unless
  `--render-scale` is given.
- **Alternatives:** always full resolution; MetalFX's temporal scaler, which looks better but needs motion vectors
  and jittered frames; a fixed scale.
- **Rationale:** at 2560×1600, half the pixels per side runs the spawn view in 11 ms instead of 32. The upscaled image
  keeps edges sharp, but the squeezed textures near mouths turn grainy. The 12 ms target is arbitrary: it leaves
  room for a 60 Hz display with some headroom.
- **Revisit if:** the grain is too visible (raise the target or pass `--render-scale 1`), or a 120 Hz display needs
  a lower target.

## Baselines

### D15. WebGL at the same drawing-buffer sizes

- **Choice:** the browser demo from PR 58, unchanged, in the Claude desktop app's built-in browser (Chromium 152,
  ANGLE on Metal), with its drawing buffer set to 1280×800 and 2560×1600 and frames timed with `readPixels`. Its
  scene differs a little: its mouths and eye are 4.5 blocks up, and it has no laser.
- **Alternatives:** moving the WebGL demo's mouths to match first.
- **Rationale:** it is the version you found too slow.

## Workload

### D7. Six fixed views

- **Choice:** spawn, aimed at mouth 1A, 0.8 blocks from 1A's throat, above the rails, high above spawn, and the
  laser through 1A; the same kinds of views as the WebGL timings in PR 58.
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
  The third layer's Metal images differ from the second's by at most 1 of 255 per channel, as writing to a texture
  rounds where the second layer's byte conversion truncated; every other change in the third layer left every pixel
  the same, on both renderers.
- **Alternatives:** a GPU-stable integer hash, so the images match exactly.
- **Rationale:** the noise only decorates; matching it wasn't worth changing the look.
