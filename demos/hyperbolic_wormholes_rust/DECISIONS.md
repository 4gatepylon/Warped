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

### D13. Step sizes

- **Choice:** the steps outside 10 throat radii of a mouth are now RK4 steps of 0.35 of the distance to it (D22),
  instead of the WebGL demo's midpoint steps of 0.25; elsewhere the step sizes are unchanged. Against a reference
  with eight times smaller steps, features at spawn now sit 1.4 pixels (median, at 2560×1600; 0.07° of view) from
  the converged light paths instead of 3.2, for 23% more GPU time when tracing every pixel.
- **Alternatives:**
  - the demo's steps (3.2 pixels);
  - midpoint steps of 0.125 (1.1 pixels, 50% more time) or RK4 steps of 0.25 (1.1 pixels, 50% more);
  - shorter RK4 steps near a mouth (0.35 or 0.25 instead of 0.5): the view 2.5 blocks from 1A goes only from 2.8 to
    2.2 or 2.1 pixels, for 12% to 25% more time there;
  - larger steps everywhere (measured before D21 to D23: spawn 30 to 21 ms at 0.18 and 0.40, with 15% of pixels
    changed).
- **Rationale:** you asked for high fidelity at 60 fps; this halves the error in ordinary views at a cost the
  window's automatic scale absorbs (D19). Near a mouth the lens magnifies what error is left, and shorter steps
  there buy little.
- **Revisit if:** you want the demo's exact picture back, or more speed.

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
  - RK4 steps of 0.5 or 0.7 of the distance out to the bending cutoff (no faster, or faster but 18 to 36% of pixels
    changed), and keeping the other mouths' gradient fixed within midpoint steps (22% of the aimed view changed
    instead of 10%);
  - a lookup table, in log q, for each mouth's bending factor instead of computing it: no faster;
  - a four times finer throat table: no change in accuracy;
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

- **Choice:** Metal can ray-trace fewer pixels and upscale them with MetalFX's spatial scaler (`--render-scale`).
  - In the window it starts at full resolution. Each frame it moves the render scale toward the one that would
    take 14 ms of GPU time (time grows with the scale's square, from the scale actually used): down at once, by up
    to a fifth a frame, and up by at most 2% a frame.
  - The scale stays between 0.5 and 1, rounded down to steps of 0.05; `-` and `=` set it by hand.
  - The scaler, its textures and the sampling buffers are made once for the window's size, and the scaler runs once
    during the first frame, so a change of scale costs nothing (each first use cost 20 ms or more).
  - `--bench-views` and `--autopilot` render at full resolution unless `--render-scale` is given.
- **Alternatives:**
  - always full resolution: 10 of 719 frames of the scripted legs over 1/60 s, the worst 20.2 ms;
  - moving 2% a frame both ways: 1.3 to 1.5% of the mouth tour's frames late;
  - dropping all the way at once: the scale fell to 0.5 and full resolution held in 92.5% of the legs' frames;
  - MetalFX's temporal scaler, which needs motion vectors and jittered frames;
  - the first version's start at 0.5 with a 12 ms target.
- **Rationale:** in a full-screen window, 1 frame of the scripted legs took over 1/60 s in each of two runs (17.1 and
  17.2 ms), with 94.9% at full resolution; on the mouth tour, 5 of 1795 did in each run, with 83% at full
  resolution. Upscaling loses detail (SSIM 0.85 to 0.96 against full resolution at 0.8 scale, features within 0.4
  pixel), so it is kept for those frames; 14 ms leaves room for the CPU's part of a 60 Hz frame.
- **Revisit if:** a 120 Hz display needs a lower target.

### D21. Trace a coarse grid, and interpolate where that is exact enough

- **Choice:** `--sampling adaptive` (the default) works in three levels. It traces every fourth pixel each way. It
  fills in every second pixel by interpolating the hit points of four of those samples, where the twelve around the
  cell ended on the same surface, in the same row, without laser glow, and their second differences stay under 0.8
  of the first; it traces the rest. Then it interpolates the remaining pixels the same way with a threshold of 1.6.
  Both thresholds bound the interpolation error by about 0.4 pixel. Interpolated pixels are still shaded one by one.
  Whatever can't be interpolated is traced in separate passes that pack the listed rays densely, with indirect
  dispatches. `--sampling every-pixel` traces every pixel.
- **Alternatives:**
  - a single level (every second pixel; spawn 9.4 instead of 7.7 ms at 2560×1440);
  - thresholds of 0.4 (0.1 pixel; the near view 29 instead of 20 ms) or 0.8;
  - tracing the rest in the same pass (about 9 ms slower at spawn, as a thread group waits on its slowest ray);
  - interpolating each pixel of a cell on its own row or column (no faster);
  - interpolating each sample's state where it leaves a mouth's near zone, which covered only 4 to 6% of cells.
- **Rationale:** against tracing every pixel, at most 0.9% of pixels change by more than 30 of 765 (the near view;
  0.2% or less elsewhere), and SSIM is 0.98 to 0.998; spawn at 2560×1600 goes from 22 to 8.5 ms of GPU time.

### D22. Fourth-order steps away from the throat

- **Choice:** from 3 throat radii of a mouth out to the bending cutoff, a ray takes classical Runge-Kutta steps:
  0.5 of the distance to the mouth's centre up to 10 throat radii (instead of midpoint steps of 0.12), and 0.35
  beyond (instead of midpoint steps of 0.25, D13); within 3 throat radii it keeps midpoint steps.
  Within 10 throat radii, the later stages of a step re-evaluate only the nearest mouth, keeping the other mouths'
  gradient from the step's start, as they are at least a few blocks away (10% faster near a mouth, no change in
  accuracy); beyond, every stage sums every mouth.
- **Alternatives:** RK4 all the way to the throat (its stages land inside the throat sphere, and 21% of the throat
  view's pixels changed by more than 30 against a reference with four times smaller steps, however small the
  steps); Heun-like steps (D14); larger midpoint steps (D13).
- **Rationale:** it is at least as accurate as the midpoint steps against that reference (spawn 11.5% of pixels
  changed against 12.3%), and about 10% faster.

### D23. Cross each mouth's inner sphere in one jump

- **Choice:** within 4.5 throat radii (1.6 blocks) of a mouth, which holds no boxes or ground (a test checks it),
  the mouth's own field looks the same about every axis through its centre, so a ray's path there depends only on
  its angle.
  - A throat table, traced once on the GPU with fine steps, gives for 4096 entry angles where a ray leaves the
    sphere again, through the throat or not, and how far it went.
  - A camera table gives the same for 64 eye distances inside the sphere by 2048 angles, for the eye inside a
    sphere.
  - Each entry also stores how its exit would move per unit of a uniform extra field, along the ray's plane and
    across it, on each side of the throat, from six extra rays with a small field. At runtime the other mouths'
    gradient at each mouth's centre gives that field, so their bending inside the sphere is included to first order.
  - A ray that reaches the sphere jumps to its exit in one step. Steps that would end deep inside are redone
    shorter, so rays enter near the sphere's edge.
  - Both tables take 0.4 s in all at start-up.
- **Alternatives:**
  - stepping through (the near view 65 ms instead of 30 at 2560×1600, tracing every pixel);
  - leaving the other mouths out: 27% of the near view's pixels and 17% of the throat view's changed against the
    reference instead of 17% and 10%. Leaving them out even within 1.5 throat radii lost accuracy, as the throat
    magnifies small deflections;
  - two half kicks around the jump (40% changed);
  - a camera table traced each frame (2 to 6 ms a frame).
- **Rationale:** it makes looking into and walking through a mouth about twice as fast, and the images are as close
  to the reference as full stepping's: at the throat 8.5% of pixels changed (SSIM 0.873) against 10.1% (0.865),
  2.5 blocks away 18.4% (0.702) against 17.0% (0.699).
- **Revisit if:** mouths are placed so close that their fields vary a lot across a jump sphere.

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

### D20. Check the window with a timed run and with the route

- **Choice:** `--exit-after-s <seconds>` opens the real window, plays without input for that long, closes it and
  prints the frame rate, GPU time and render scale. `--play-route` plays the scripted route in the window, one
  1/60 s step per frame, and prints the frame times (median, 99th percentile, worst, how many took over 1/60 s,
  after 5 warm-up frames) and the render scale used. `--route mouth-tour` plays a harder tour: the eye circles each
  of the six mouths once, from 5 blocks to 0.45 and back, with the view swinging up to 46° sideways and 23° up and
  down, over 300 frames per mouth. `--width-px` and `--height-px` set the window's starting size.
- **Alternatives:** only the windowless benchmarks, which leave out presenting, MetalFX in the window, and the
  automatic scale.
- **Rationale:** it measures what you see when you play, in motion, and needs no screen recording.

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
  CPU renderer still traces every pixel with midpoint steps, so it stays the reference they are measured against:
  that tracer with four and eight times smaller steps (`.artifacts/bench_views/*-ref-fine7` and `*-ref-fine8`), which
  differ from each other by 0.03 to 0.2 pixel (median; 1 pixel at the throat) and from the default steps by 1 to 3
  pixels. Shifts are
  measured by phase correlation of 48×48 textured patches, besides the share of pixels changed and SSIM.
- **Alternatives:** a GPU-stable integer hash, so the images match exactly.
- **Rationale:** the noise only decorates; matching it wasn't worth changing the look.
