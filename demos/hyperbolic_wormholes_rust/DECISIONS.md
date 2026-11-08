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
  heuristics allowed for speed. Then lighter layers: light paths close to converged ones, and play on one local
  network with lasers that remove whoever they hit (who rejoins), tested by playing against bots.
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

### D24. Measure accuracy where the reference has converged

- **Choice:** the reference is now the PR 70 tracer with steps 32 times smaller, and a view's shift is the median
  over the 48×48 patches where it agrees with the 16-times reference within 0.5 pixel (61% of the throat view's
  textured patches, 90 to 99.8% elsewhere). The two agree to 0.01 to 0.12 pixel.
- **Alternatives:** the eight-times reference against the four-times one, as in D16, whose 1 pixel at the throat
  turned out to be the four-times reference's own error.
- **Rationale:** a shift is only meaningful where the truth is known to better than it.

### D25. Test hits closer to the curved path

- **Choice:** when a step's chord hits something farther off the curved path than 1e-4 radians seen from the eye
  (`get_chord_limit_m`), the ray steps again from the step's start, lands just before the hit and crosses it in
  short steps (OPTIMIZATIONS.md 7.2). RK4 steps are also capped at 2 blocks (7.3).
- **Alternatives:** capping every step by the same bound (spawn 0.07 pixel but twice the time); capping steps at 0.3
  of the distance travelled; testing the step's Hermite curve in up to 16 chords (slower); ending steps on the
  cutoff spheres; a cap of 3 blocks, or a bound of 2e-4.
- **Rationale:** most of the 1.4 pixels left in ordinary views came from these chords and long outer steps; together
  they now sit 0.05 to 0.14 pixel from the converged paths.

### D26. Whole RK4 steps near a mouth, and the jump's tidal correction

- **Choice:** every RK4 stage sums every mouth, with steps of 0.35 of the distance from 3 throat radii out to the
  cutoff (one rule instead of D22's two). The jump tables also store the response to a linear extra field, and each
  frame a kernel stores the other mouths' gradient and its change at each mouth, so a jump adds their tidal part to
  first order; the throat table uses the near side's part only (OPTIMIZATIONS.md 7.5). The step entering a jump
  sphere is cut by three rounds of regula falsi.
- **Alternatives:** D22 and D23 as they were; one round at the entry (near view 2.02 pixels instead of 1.50); the far
  side's tidal part with the throat table too (near view 1.98 instead of 1.44); the entry taken from the step's
  Hermite curve; larger or finer tables.
- **Rationale:** the jump's uniform-field assumption was most of the error left near a mouth. The near view goes from
  2.64 to 1.50 pixels and the throat view from 1.99 to 1.02. Why the far side's part hurts with the throat table is
  not understood; it was chosen by measurement.

### D27. Paint trapped rays from their surroundings

- **Choice:** rays that stay trapped circling a throat (a wide band looking sideways just outside one) are painted
  with a pull–push fill of the pixels around them instead of black. `side_at_throat_1A`, an eighth benchmark view,
  shows it.
- **Alternatives:** black, as the CPU renderer still draws them; a longer step budget (20,000 steps and ten times
  the path length still leave them trapped).
- **Rationale:** you asked for something simple that doesn't look all black; the lensing law traps them, so no
  colour is physically right.

### D28. Accept slower frames near mouths for the accuracy

- **Choice:** D25 and D26 cost GPU time (spawn 9.7 to 13.6 ms at 2560×1600, 2.5 blocks from 1A 21.6 to 33.3), so in
  a full-screen window more frames near a mouth are upscaled: full resolution in 77% of the legs' frames instead of
  93%, and 44% of the mouth tour's instead of 69%, with about as many frames over 1/60 s (5 and 30 against 4 and 15,
  under load).
- **Alternatives:** keeping PR 72's paths, 5 to 20 times further from the converged ones in ordinary views.
- **Rationale:** I decided it under the goal's "high fidelity"; say if you'd rather have full resolution near
  mouths.
- **Revisit if:** the upscaled frames near mouths look worse to you than the old paths did.

### D29. Play together over UDP, each player trusted with its own moves and hits

- **Choice:** a server keeps each player's eye frame (16 doubles) and laser switch and sends everyone a snapshot 60
  times a second; each client moves its own player and decides when its own laser hits someone (it traces the beam
  every frame anyway), and the server removes the victim and tells everyone. Plain UDP from the standard library,
  joins resent until welcomed, players dropped after 3 s of silence. The host runs the server on a thread in its
  game (`--host`); `--serve` runs it alone. The server announces itself by broadcast every 0.5 s so `--join auto`
  finds it.
- **Alternatives:** a server that simulates everyone from their inputs (fairer, but laggier and much more code);
  TCP; a networking crate.
- **Rationale:** you asked for a demo on one local network, where latency is low and players trust each other.
- **Revisit if:** it is played over the internet or with strangers.

### D30. Players are boxes in the ray-traced world

- **Choice:** each other player is a coloured box (0.6 × 1.9 × 0.6 blocks, upright, around their eye) added to a
  copy of the world every frame, so it is lensed through wormholes, casts nothing, and stops lasers like any box;
  each player with a laser on has their beam traced through the same world and drawn. Up to 8 players, which the
  shader's 32 clusters and 96 boxes leave room for.
- **Alternatives:** drawing players on top of the ray-traced image, which can't show them through a wormhole.
- **Rationale:** the same reason the laser is a light ray (D6). It costs no measurable frame time with three
  players (a hosted window at 1280×800: 88 fps alone, 91 with two bots, 84 single-player, all under the same load).
- **Revisit if:** a player standing inside a mouth's jump sphere should be visible (rays jump across it, D23).

### D31. Lasers aim at the crosshair 10 blocks out

- **Choice:** the beam leaves the hand toward the point 10 blocks straight ahead of the eye, not parallel to the line
  of sight.
- **Alternatives:** parallel to the line of sight, as before: in hyperbolic space a line 0.25 blocks to the side
  drifts away (0.32 blocks off after 5.7 blocks), so a laser aimed straight at a player missed them.
- **Rationale:** a laser should hit what the crosshair is on. The laser through 1A still goes through it.

### D32. Test games with bots

- **Choice:** `--bot` plays without a window: it walks and turns until another player is within 12 blocks, turns
  toward them with its laser on, rejoins 2 s after being hit, and saves its view at each hit.
- **Alternatives:** only playing by hand.
- **Rationale:** you suggested playing against myself. Three bots for 30 s: 25 hits, and each bot's count of hits
  and times hit matched the server's log.

### D33. Load the house from its map, and add Lunar Surface

- **Choice:** `--map house-demo` (the default) builds the world from `common/maps/house_demo` (boxes grouped by
  name, mouths, spawn), and draws pixel for pixel as the world built in code did (checked on all eight views, and by a
  test that the loaded world's boxes, clusters, mouths and spawn equal the built one's). `--map lunar-surface` loads
  the Lunar Surface level's map and draws it with a Metal port of that level's WebGL shader.
- **Alternatives:** porting Lunar Surface's look onto the house renderer (boxes, the house's jump tables), which
  would not match the level; a renderer that draws both kinds of map.
- **Rationale:** you asked for maps chosen by name, with lunar_surface matching the level; porting the level's own
  shader is the surest match. Side by side with the WebGL level in the built-in browser, the blue spawn, black hole,
  wormhole and overview views show the same scene, and the laser fired into the wormhole hits the red pylon 42.4 m
  away, as the level's README says (42 m).

### D34. Lunar Surface keeps the level's light paths

- **Choice:** the same kick–drift–kick steps of 8% of the distance to a warp as the level, with each step's end
  bending reused for the next step's start (10% faster, the same picture), and each step testing only the structures
  in the warp's reach whose distance from its centre the step can reach (another 5 to 15% near warps, every pixel the
  same).
- **Alternatives:** steps of 12% or 16% (30% and 45% less GPU time near warps, but features up to 0.5 pixel further
  from converged paths: against the level's steps eight times smaller, the level's own steps are 0.3 to 2.1 pixels
  off near warps at 2560×1600, 12% steps 0.8 to 2.2, 16% steps 0.9 to 2.3); steps up to three times longer where the
  bending is weak (no faster, as the time goes to rays winding close to the black hole and the throats, where steps
  must stay short; up to 0.9 pixel further off); the house's RK4 steps (much more code, with chord errors to
  handle near the structures inside the warps' reach); jump tables (D37).
- **Rationale:** the map should look like the level; the window's automatic scale keeps 60 fps (D36).
- **Revisit if:** the upscaled frames near the black hole and wormhole look too soft.

### D35. Lunar Surface plays by the level's rules

- **Choice:** the level's player (a body frame on the ground, an eye height, probes for collisions, stepping up single
  blocks, 6 m/s² gravity, flying), its laser trace (from the eye, drawn from the gun) and its falling into the black
  hole, ported to Rust. The laser stays this demo's toggle (`L`) rather than the level's single shots, so the
  multiplayer rule (a laser that reaches a player removes them) is the same on both maps. Other players are 0.5 ×
  2 × 0.5 m voxel bodies in their colour; players alternate between the blue and red spawns.
- **Alternatives:** the house demo's movement on the lunar map; the level's single shots that fade.
- **Rationale:** the map should play like the level, and both maps should share one multiplayer rule.

### D36. Speed on Lunar Surface

- **Choice:** adaptive sampling (D37), clearance grids (D38), escaping rays read where their bending ends (D39), longer
  steps after 90 iterations (D40), and the same automatic render scale as the house (D19). At 2560×1600 and full resolution the views away from warps take
  6 to 8 ms of GPU time, those across the crater 7 to 10 ms and those near the black hole and wormhole 8 to 20 ms (33
  to 58 ms when this layer stack started). At the full-screen window's size (2560×1336) every view takes under 11 ms
  but the wormhole exit and the laser view, 16.4. In the window, every view runs at 74 fps or more, all at full
  resolution but the wormhole exit, at 0.88 (0.53 to 0.62 for the views near warps at the start). With D40 the
  wormhole exit takes 13.7 ms at the window's size and full resolution and runs at render scales 0.96 to 0.98. On the
  warp tour (D20), in motion down to 1.5 m from each mouth, the window averages 85 fps, 0.2% of frames take longer
  than 1/60 s, and 78% are at full resolution.
- **Alternatives:** larger steps (D34); jump tables (D37); a higher scale target (the window measured 78 fps for the
  wormhole exit at full resolution with a 16 ms target, but the views run then differ from run to run, as the player
  moves differently; left as it is, at 14 ms, D19).
- **Rationale:** it keeps 60 fps with the level's picture; only the wormhole exit, where most rays wind near the
  throat, still costs an eighth of the resolution.

### D37. Lunar Surface interpolates ray ends on voxel faces

- **Choice:** the house's adaptive sampling (D21), with the same passes and buffers: trace every fourth pixel, then
  interpolate where the twelve samples around a cell ended on one face plane of one structure (same axis, side and
  layer of cells), on the ground, in the sky or lost, behind the same glass and without laser glow, and map nearly
  linearly. An interpolated block hit is kept only if the cell under it is solid and the one in front of its face
  open; otherwise that ray is traced. `--sampling every-pixel` traces every pixel.
- **Alternatives:**
  - tracing every pixel (2 to 3 times slower near warps; about 1 ms faster in views with little bending);
  - jump tables across each warp's empty middle, traced at load with finer steps: 7 to 30% faster tracing every
    pixel, but 10 to 35% slower with adaptive sampling, as jumped rays and their stepped neighbours disagree by the
    level's step error and cells along the sphere's edge stop interpolating (OPTIMIZATIONS.md, 8.3);
  - a cheaper per-structure test on each bent step, or one call site for straight and bent chords (no faster).
- **Rationale:** near the black hole and wormhole it takes 42 → 17, 44 → 16, 33 → 18 and 58 → 29 ms at 2560×1600.
  Against tracing every pixel, SSIM is 0.976 to 0.999 and the changed pixels are texture noise where texels are smaller
  than a pixel; against steps eight times smaller the median feature shift matches tracing every pixel within 0.05
  pixel (but 4.1 against 3.7 at the wormhole exit).

### D38. Lunar Surface skips blocks a bent step can't reach

- **Choice:** a clearance grid per warp (0.5 m cells in the warp's local chart, out to 1.1 times its reach; one byte
  per cell in 0.1 m steps, capped at the warp's longest step plus 0.5 m), holding a lower bound on the distance from
  the cell's centre to a static block. A bent step shorter than that bound, less the step's start's distance from the
  cell's centre, tests only the ground and avatars. Exact: every picture is unchanged.
- **Alternatives:**
  - a cheaper test per structure (bounding sphere within the step's reach): no faster (OPTIMIZATIONS.md, 8.1);
  - bounds from each block's bounding sphere and the cell's half-diagonal: 1.3 m looser near blocks, and the wormhole
    exit 0.5 ms slower;
  - 0.25 m cells (not tried): 8 times the memory and build time for the black hole's grid.
- **Rationale:** near the black hole and the wormhole it takes a quarter off the frame (29.3 → 21.2 ms for the
  wormhole exit at 2560×1600); it costs 18 ms at load and 2.1 MB.

### D39. Escaping rays are read where their bending ends

- **Choice:** a ray that leaves a warp's reach and escapes is moved back along its straight line to the reach's sphere,
  and its direction is carried to the eye from there (the map format's `parallel` sky transport), not from wherever
  its last step ended, up to one step further.
- **Alternatives:** the level's last point (rings of jumps in the sky between rays taking different numbers of
  steps, which also stop adaptive sampling from interpolating there); the same at the throat (slower, OPTIMIZATIONS.md
  8.5).
- **Rationale:** it is the picture the level's steps converge to, and closer to it (black hole 0.57 → 0.11 pixel
  against steps eight times smaller; Einstein ring 1.33 → 1.00), and a third faster on the Einstein ring view. The
  picture differs from the level's along the lensed sky (SSIM 0.96 to 1.0).

### D40. Longer steps after 90 iterations

- **Choice:** after 90 iterations a ray's bent steps are 24% of the distance to the warp instead of the level's 8%.
- **Alternatives:** after 120 or 80 iterations; a budget per warp visit; ending rays at 120 or 160 iterations (the
  photon ring turns black); see OPTIMIZATIONS.md 8.7.
- **Rationale:** the wormhole exit gets 12% faster (19.8 → 17.4 ms at 2560×1600), close enough to the window's 14 ms
  target to run within a few percent of full resolution. Only rays in the lens's most compressed, aliased rings take
  that long; 2.9% of that view's pixels change, and their distance from a converged reference doesn't.
- **Revisit if:** the rings inside a mouth look different when played.

### D41. Scales from 0.95 up render at full resolution

- **Choice:** when the automatic scale would pick 0.95 or more, the window renders at full resolution.
- **Alternatives:** the 0.95 step (D19's rounding).
- **Rationale:** MetalFX costs about as much as that step saves (the wormhole exit 13.8 → 13.5 ms at 2560×1336) and
  loses as much detail as 0.90 (SSIM 0.89 against full resolution). Full-resolution frames of up to about 15.5 ms of
  GPU time still fit a 60 Hz frame. In the window: the wormhole exit at full resolution, 69 fps; the warp tour full in
  90% of frames (78% before), 0.3% over 1/60 s; the house's mouth tour full in 60% (52%), 1.2% over 1/60 s (2.1%).
- **Revisit if:** frames near 15 ms stutter when played.

### D42. Click to shoot

- **Choice:** once the mouse is captured, a left click fires the laser for 0.25 s: it is drawn, sent to the other
  players and hits during that time, as the toggled laser does. `L` still toggles a laser that stays on. An eliminated
  player can't shoot until they rejoin.
- **Alternatives:** holding the button to fire (closer to the toggle); dropping `L`.
- **Rationale:** you asked for click to shoot; a quarter of a second is long enough to see the shot and short enough
  that each click is one shot. `L` stays for showing the light paths.

### D43. Jump to get onto blocks

- **Choice:** on Lunar Surface, a block stops a walking player like a wall; `Space` jumps onto it (a jump clears about
  2.25 m). Bots jump when a block stops them.
- **Alternatives:** the level's automatic step onto single blocks; stepping up only after pushing into a block for a
  moment; stepping up smoothly over a fraction of a second.
- **Rationale:** you chose jumping: walking into a block popped the player onto it at once.

### D44. Respawn after a countdown

- **Choice:** a player who is hit sees their view darkened to 45% with "YOU DIED" in red and "Respawning in N" in
  white, and rejoins at their spawn 3 seconds later; `Enter` no longer rejoins. The window's title and the HUD say the
  same. The text is SDL's built-in font drawn into a 160×24 mask on the CPU, which a compute pass scales up by a whole
  number (to about a fifth of the window's height) and draws over the frame, as Metal presents straight to the
  window's layer, where SDL can't draw.
- **Alternatives:** pressing `Enter` to rejoin (the earlier rule; players often didn't see the message, which was
  only in the window's title); a longer delay.
- **Rationale:** you asked for a "you died" message and a visible respawn timer.

### D45. Faster, heavier movement with sprint energy and a head bob

- **Choice:** on voxel maps the game scales the map's player settings: walking and sprinting twice as fast (10 and
  18 m/s on Lunar Surface), gravity three times as strong (18 m/s²), and the jump speed by √3 so a jump still rises
  2.25 m (high enough to get onto a block, D43) in less time. Sprinting uses energy as in the WebGL levels: a full bar
  empties in 30 s of sprinting and refills in 60 s, and an empty bar means walking; the bar is drawn in the bottom
  left. The eye dips at each footstep (4 cm walking, 7 cm sprinting, 2 and 2.8 bobs a second), fading in and out over
  about a sixth of a second.
- **Alternatives:** changing the map's own values (they would no longer match the level the map was exported from);
  keeping the jump speed (a jump would rise only 0.75 m, too little for a block).
- **Rationale:** you asked for twice the speeds, three times the gravity, a sprint energy bar as described in the
  levels, and head bobbing.

### D46. A gun on screen, and no shooting while running

- **Choice:** a gun drawn in the bottom right with four rectangles (grip, body, barrel, and a muzzle in the team's
  colour). Walking or standing, it points ahead and sways a little with each step; running (sprinting while moving),
  it eases over a sixth of a second into a lowered, leaning pose that bounces with each stride. While running, clicks
  don't shoot, a shot in flight stops, and a laser toggled on with `L` is off.
- **Alternatives:** a sprite or 3D model of a gun (more code and assets); letting players shoot while running.
- **Rationale:** you asked for a simple moving gun of rectangles that jogs when running and points forward when
  walking, and for no shooting while running.

### D47. Only the game's own keys

- **Choice:** the window's keys are WASD to move, `Shift` to run, `Space` to jump, click to shoot, `R` to respawn,
  `-`/`=` for the render resolution and `Esc` to free the mouse. `E` no longer jumps, and the keys for flying (`H`,
  `Q`), a laser that stays on (`L`), aiming through a mouth (`G`) and walking on its own (`F`) are gone. Flying and the
  steady laser remain in the code for the scripted routes.
- **Alternatives:** removing only `E`; removing `R` and `-`/`=` as well.
- **Rationale:** you asked for WASD to move, `Space` to jump and click to shoot, and chose to drop the keys that gave
  an edge (flying, a steady laser, auto-aim, auto-walk) while keeping respawn and resolution.

### D48. A smaller energy bar and a deeper head bob

- **Choice:** the sprint energy bar holds a quarter of D45's: it empties in 7.5 s of sprinting and refills in 15 s
  (the refill rate is unchanged). The eye dips twice as deep at each footstep: 8 cm walking, 14 cm sprinting, at the
  same 2 and 2.8 bobs a second.
- **Alternatives:** draining 4 times faster but refilling in 60 s as before; a sideways sway as well as the dip.
- **Rationale:** you asked for the bar to run out 4 times faster by shrinking it 4 times, and for more head bob;
  twice the depth is a first guess, easy to change (`BOB_WALK_M`, `BOB_SPRINT_M` in `src/voxel_world.rs`).

### D49. No running on an empty energy bar

- **Choice:** energy refills only while the player isn't holding `Shift` to move. Before, an empty bar refilled a
  little in the very next frame, so holding `Shift` switched between running and walking every frame and the player
  kept running at about two thirds of the sprint speed.
- **Alternatives:** letting the bar refill while `Shift` is held but requiring some energy (say a tenth of the bar)
  before running again.
- **Rationale:** you asked that a player can run only while there is energy.

### D50. No shooting in the air

- **Choice:** on voxel maps a player can't shoot while jumping or falling (in the air and not flying), as while
  running (D46): clicks don't shoot, and a shot in flight stops when the player leaves the ground. On the house,
  `Space` is a jetpack rather than a jump, and shooting while on it is unchanged.
- **Alternatives:** allowing a shot started on the ground to finish in the air; also blocking shots on the house's
  jetpack.
- **Rationale:** you asked that players can't shoot while jumping.

### D51. A low, quick jump

- **Choice:** on voxel maps a jump rises 1.25 m (enough to get onto a 1 m block, D43) and lands in a third of the
  map's jump time: 0.58 s on Lunar Surface, against the level's 1.73 s and D45's 1.0 s. Gravity follows from those
  two numbers, about 30 m/s² (five times the level's 6 m/s², up from D45's 18), with a jump speed of 8.7 m/s, so
  falls are faster too. At 60 frames a second a jump peaks at about 1.18 m, as each step applies gravity before it
  moves the player.
- **Alternatives:** three times the level's gravity with the level's jump speed (the same 0.58 s, but a jump
  would rise only 0.75 m, too low for a block); three times D45's gravity, 54 m/s², with a 1.25 m jump (0.43 s,
  a hop).
- **Rationale:** you found D45's jump too long (it still rose 2.25 m and lasted 1 s) and asked for gravity three
  times as strong in the sense that a jump is snappy and falls are quick; a jump three times shorter than the
  level's is that, and 1.25 m keeps jumping onto blocks (the same jump height as Minecraft's, for 1 m blocks).

### D52. A game server without a window

- **Choice:** the game server always runs as its own process, without a window, and every player is a client with a
  window. `--serve` (`make serve-game`) runs only the server and prints this computer's address (the one its
  default route uses) and the `make join-game` commands that join it. `--host` (`make host-game`) starts that
  server as a child process, waits 0.3 s to fail fast if it stops (its port taken, say), joins it over loopback,
  and stops it when the window closes. Before, `--host` ran the server on a thread inside the host's game.
- **Alternatives:** keeping the server thread for `--host` and only adding the printout to `--serve`; starting the
  server from the Makefile in the background (it would outlive a crashed window more often).
- **Rationale:** you asked for a headless server that prints its address, with clients connecting to it directly,
  and for the launcher to start its own client, so the server can later run in the cloud.

### D53. Winning by turning off consoles

- **Choice:**
  - As the game design says, a player holds `E` for 10 s on one of the other team's consoles, looking straight at
    it from within 3 m; letting go or looking away resets it, and they can't shoot meanwhile.
  - Consoles pulse once a second between 0.6 and 2 times their colour, twice a second while anyone holds `E` on
    them, and go dark (an eighth of their colour, unlit) once off.
  - The server times each hold from the players' states, turns the console off, and declares the other team the
    winner once all of a team's consoles are off. It then ignores hits and holds until `Enter` in its terminal
    starts a new game, which turns every console back on and sends everyone to their spawn.
  - The window shows the winner over a dimmed view.
  - A player who rejoins from the same address keeps their id, so they stay on their team.
  - Consoles are the map's materials named "<team> console <n>".
- **Alternatives:**
  - one press of `E` (your first words; but you also asked for a faster pulse during disabling, which needs it to
    take time, and the design says hold for 10 s);
  - a new game from a key in the launcher's window;
  - restarting the server for a new game (everyone would have to rejoin);
  - listing consoles in the map format (it would need a format change).
- **Rationale:** you asked for consoles that pulse until disabled and darken after, a faster pulse while being
  disabled, the game to end with the winner shown, and the server's launcher to start the next game.

### D54. A 3D wand instead of the flat gun

- **Choice:** D46's four rectangles become a wand.
  - **Shape and light:** a rectangular prism 2.2 cm thick and 32 cm long, with a 3.2 cm tip in the team's
    colour. The overlay pass ray-traces it in camera space with 2 × 2 rays a pixel for smooth edges. The shaft is
    dark and lit from above, with a highlight and darker edges; the tip glows with a faint halo.
  - **Aiming:** the wand is held on the right with its tip toward the crosshair, dipping a little at each step.
  - **Running:** it eases into a lowered pose, tipped down 0.35 rad and rolled 0.5 rad, that bounces and swings
    with each stride. The CPU sets its position and rotation each frame.
  - **CPU renderer:** it draws no wand; it is for the house only.
- **Alternatives:** a mesh drawn with a vertex pipeline (a second render pass and a depth buffer for one object);
  drawing it into the scene's rays (it would be lensed by warps, though it is in the player's hand).
- **Rationale:** you asked for a prettier gun, "a rectangular prism with a colored tip" that turns like a real object.

### D55. A little motion blur

- **Choice:** in the window, each frame's scene is blended with the previous frame's blended scene, weighted
  e^(-dt / 10 ms) for a frame of dt: 0.19 at 60 frames a second, 0.43 at 120. Whatever the frame rate, a moving
  scene smears over about 10 ms. The HUD, the banner and the wand are drawn after the blend, so they stay sharp. The
  blend happens in the overlay pass with two history textures, swapped each frame; after a resize the first frame
  isn't blended. It costs about 0.2 ms of GPU time a frame at 1280×800. Benchmarks (`--bench-views`, the
  autopilot) have no overlay and so no blur.
- **Alternatives:**
  - a fixed weight (the blur would shrink as the frame rate rises);
  - per-pixel velocity blur from the camera's motion (more correct for fast turns, but much more code in curved
    space).
- **Rationale:** you asked for a tiny bit of motion blur; 10 ms is about half a frame at 60 fps. The time is
  `MOTION_BLUR_S` in `src/main.rs`.

### D56. A second gun: the rifle

- **Choice:**
  - **Switching:** gun 1 is the laser so far (a quarter-second shot per click; one hit eliminates; the design's
    sniper rifle without its fire-rate limit or overheating yet). Gun 2 is the design's rifle. The mouse wheel
    (either way) swaps them, and `1` and `2` pick one.
  - **Firing:** the rifle fires while the button is held: a 0.06 s pulse of the beam every 0.125 s (8 shots a
    second, within the design's 2–10). Each shot counts at most one hit. Two hits to the head (the avatar's top
    half-meter cell) or twelve to the body eliminate, so sustained body fire kills in 1.5 s (the design: 1–2 s).
    The server adds up each player's damage until they rejoin.
  - **Heat:** it overheats after 20 shots in a row (the design: 15–25), shown as a bar above the energy bar, and
    cools fully in 2 s without firing (the design leaves the cooling time open).
  - **Looks:** the rifle is a thick grey block with a long, thin glowing barrel; the laser stays a thin dark wand.
  - **Design document:** `game_design/DESIGN.md` now says that a player with two weapons switches with the wheel or
    `1`/`2`, and has a table of the controls.
- **Alternatives:** a health bar for the player who is hit (it needs damage in the snapshots); slowing a player
  who is hit, as the design says (not yet).
- **Rationale:** you asked for a second gun, the rifle from the design, switched with the scroll wheel or `1`/`2`,
  looking different from the first.

### D57. The laser leaves from the gun's tip

- **Choice:** one muzzle point in camera space, `MUZZLE_M` = (0.187, -0.157, -0.555) m, is both where the drawn beam
  starts and where the front of the gun's tip is drawn while aiming: each gun is placed back from it by its own
  length. The beam still joins the crosshair's path 3 m ahead. The point is where the laser wand's tip already was,
  so the wand doesn't move. Before, the beam started 0.1 m ahead of the eye, below and behind the tip.
- **Alternatives:** moving the beam's start with the wand's small dip at each step (a few millimeters).
- **Rationale:** you found that the laser didn't come out of the wand's tip.

### D58. A client of another map stops with an error

- **Choice:** the server's welcome names its map, and a client loaded with another map stops with an error that
  names both and the `MAP=` to use. Before, a server started without `MAP=lunar-surface` (`make serve-game` plays
  the house by default) let Lunar Surface players join. It had no consoles, so it ignored every hold of `E` while
  the window's bar still filled: you saw a console stay on after 10 s. Errors reading the server now end the
  window's game too, instead of a line in the title bar.
- **Alternatives:** the server refusing the join; a warning instead of an error (the game would still be wrong).
- **Rationale:** you found that turning off a console did nothing. A scripted client holding a console against a
  Lunar Surface server saw it turn off after 10.02 s, so a server on another map is the likeliest cause.


## Baselines

### D15. WebGL at the same drawing-buffer sizes

- **Choice:** the browser demo from PR 58, unchanged, in the Claude desktop app's built-in browser (Chromium 152,
  ANGLE on Metal), with its drawing buffer set to 1280×800 and 2560×1600 and frames timed with `readPixels`. Its
  scene differs a little: its mouths and eye are 4.5 blocks up, and it has no laser.
- **Alternatives:** moving the WebGL demo's mouths to match first.
- **Rationale:** it is the version you found too slow.

## Workload

### D7. Eight fixed views

- **Choice:** spawn, aimed at mouth 1A, 2.5 blocks from 1A's centre, 0.8 blocks from 1A's throat, above the rails,
  high above spawn, the laser through 1A, and looking sideways 0.4 blocks from 1A's centre; the same kinds of views
  as the WebGL timings in PR 58, plus the 2.5-block view, added in the third layer because walking into a mouth was
  the slowest part of the route, and the sideways view, added in the fourth because it showed the trapped band.
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
  down, over 300 frames per mouth. On a voxel map, `--route warp-tour` circles each warp object the same way, from 80%
  of its reach to 10% and back, over 600 frames each. `--width-px` and `--height-px` set the window's starting size.
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
