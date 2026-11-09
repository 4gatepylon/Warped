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

### D50. No shooting in the air (replaced by D109)

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

### D59. Rifle recoil

- **Choice:** each rifle shot kicks the view up 0.0075 rad (0.43°) and 0.0025 rad sideways, alternately left and
  right. A full 20-shot burst climbs 8.6° that the player pulls back down, as in tactical shooters. The rifle
  jolts 3.5 cm back toward the eye and tips up 0.12 rad, settling over 0.08 s, before the next shot 0.125 s later.
  The laser wand has no recoil.
- **Alternatives:** a random sideways kick (the zigzag is predictable to learn, as in CS:GO's patterns); the view
  drifting back down by itself.
- **Rationale:** you asked for recoil on the rifle.

### D60. Game settings in a YAML file

- **Choice:**
  - **The file:** `game_settings.yaml` holds the numbers that tune play, in sections: movement, sprint energy,
    head bob, the sniper (gun 1), the rifle (gun 2), consoles, respawn and display. Each key has a one-line note
    beside it, because you asked for a file that is easy to understand; the repository's no-comments rule is for
    code.
  - **Distribution:** `--settings <file>` (`SETTINGS=` with `make serve-game`, `make host-game` and
    `make run-demo`) gives a server its settings, and the server sends them in its welcome to every player who
    joins. The window adopts them on joining, so everyone plays by the server's numbers. Without `--settings`, the
    file as built in is used.
  - **Checks:** every key is required, unknown keys fail, and every number must be above 0. Zero is allowed for
    the bob depths, the recoil, the respawn delay and the motion blur. An error names the key.
  - **Left out:** mouse sensitivity (each player's own preference, not the server's), the player limit and
    timeouts (part of the protocol), and the wand's looks.
  - **Bots:** they keep the built-in movement settings.
- **Alternatives:**
  - a file of overrides on top of the defaults (shorter files, but a typo in a key name would be silently
    ignored unless checked separately);
  - TOML (the Rust crates' usual format; you asked for YAML);
  - each player reading their own file (players could play by different numbers).
- **Rationale:** you asked for speed, head bob, shots per second and the other game parameters in a YAML file that
  the server takes with a flag, so players can tune the game. The YAML parser is `serde_yaml` 0.9, which its author
  has marked deprecated but which is stable and widely used.

### D61. Beams fly out like tracers

- **Choice:**
  - **Flight:** on voxel maps each shot's path is traced once, when it is fired, and stays where it was fired. A
    lit stretch flies along it: its front leaves the gun's tip at `beam_speed_m_per_s` (250 m/s for both guns), and
    each point stays lit for `beam_trail_s` after the front passes it (0.25 s for the sniper, a 62 m streak; 0.05 s
    for the rifle, 12 m). The impact glows while the trail reaches it. Both numbers are in `game_settings.yaml`, per
    gun.
  - **Other players' shots:** each window traces another player's shot when their laser comes on. The snapshot now
    says which gun fired (a bit in the laser byte), so each gun keeps its own look.
  - **Hits:** unchanged. A hit counts at once along the crosshair's path, so the beam is only a picture, as you
    said.
  - **Unchanged:** the house, scripted routes and bots still draw whole beams.
- **Alternatives:** moving the hit to when the beam's front arrives (closer to a bullet, but a hit would lag the
  shot); re-tracing the path every frame from the current aim (the beam would swing with the view).
- **Rationale:** you asked for the laser to disappear from the tip onward and fly forward like a bullet, lasting
  longer for the sniper than the rifle, with both settings in the YAML file.

### D62. Players never spawn on top of each other

- **Choice:**
  - **Slots:** every spawn tries slots around the team's spawn point, starting from the player's own (the golden
    angle times their id on a 3 m ring). It takes the first slot at least 1.5 m from every other player it can
    see, going on to rings 5 m and 7 m out (24 slots in all, 16 on the house).
  - **Where it applies:** joining, a new game, `R`, and falling into a black hole. Before, `R` and the black hole
    put players exactly on the spawn point.
  - **Why ids alone weren't enough:** a player's slot came from their id alone, and ids keep growing over a
    session (each new address gets the next one), so two players could share a slot: ids 1 and 35 are 5° apart.
- **Alternatives:** the server handing out spawn slots (it would know every player even before snapshots arrive,
  but it needs another message).
- **Rationale:** you asked that players never spawn one on top of another.

### D63. A health bar, and bars in segments

- **Choice:**
  - **Health:** the server adds a byte of health per player to every snapshot (it already counted each player's
    damage), and each window reads its own. In a game on a server the health bar sits in the bottom left, above
    the others. It has one segment per rifle body hit (12): a rifle body hit takes one, a head hit six, the sniper
    all of them. It is full again after respawning.
  - **Segments:** the rifle's bar now shows its shots left before overheating, one segment per shot, red while
    overheated. The sprint energy bar has 10 segments.
  - **Rounding:** a segment counts as full once more than 5% of it is filled, so the byte's rounding never shows
    a lost segment as full.
- **Alternatives:** health in each player's record (other players' health could then be shown over their heads,
  but every record would change); a separate message to the player who was hit (it could be lost).
- **Rationale:** you asked for a health bar so players know they were hit, and for the health, ammo and energy
  bars to be discrete.

### D64. Beams stop being drawn past the map

- **Choice:** a beam is drawn only up to where it first gets 4 curvature radii (160 m on Lunar Surface) from the
  map's origin; the map lies within about 3.2.
  - **Cause:** a shot into the sky flies on to 480 m, where its points' world coordinates reach 10⁵ (they grow as
    cosh of the distance). The shader works out each pixel's nearness to a beam in single precision, and at that
    size the maths turns to noise. Every sky pixel within reach of the far part of the beam got a noisy glow in the
    team's colour, the pink blob you saw.
  - **Before and after tracers:** before them, the far part came with the whole beam for a 0.06–0.25 s shot, the
    flashing. Tracers (D61) drew the far stretch alone for about 2 s while it flew outward, which made it bloom.
  - **Reproduction:** a sky shot's 300–312 m stretch drew a large noisy pink disk; with this change it draws
    nothing, and the 100 m stretch and the whole beam look as before.
- **Alternatives:** double precision in the shader (much slower on Apple GPUs); working out each beam relative to
  the camera instead of the map's origin (a larger change to the shader).
- **Rationale:** you found that shooting into the sky, and at the black hole's edge, flooded the sky with
  flickering pink noise.

### D65. What a hit feels like

- **Choice:**
  - **Trigger:** when a snapshot shows this player's health lower than before (D63), the window plays the hit.
  - **Slowdown:** the player moves at half speed for 1 s (on voxel maps; the house's walking has no slowdown).
  - **Screen:** the edges turn red (60% at the corners), blur over 6 px (an 8-tap ring), and the whole picture
    shakes up to 6 px in a new direction each frame. All of it fades over 0.6 s, and the red and blur draw back
    toward the corners as it fades. The HUD and the wand stay sharp.
  - **Settings:** all six numbers are under `when_hit` in `game_settings.yaml`.
- **Alternatives:** a hit message from the server (it could be lost, and health already arrives every snapshot);
  blurring with a separate pass (more GPU time for an effect that lasts half a second).
- **Rationale:** you asked that a player who is hit is slowed (set in the YAML), sees red and a blur come in from
  the edges, and feels the screen shake.

### D66. Shots into the sky draw only their first 80 m

- **Choice:** a tracer for a shot that hit nothing (D61) flies only its first 80 m, two curvature radii, and is
  then gone. Shots that hit something are drawn whole, up to D64's limit. Hits don't change.
  - **Why:** you still saw a little noise after D64. Counting pixels changed by a 12 m stretch of a sky shot
    against no beam at all, over six views: 0–50 pixels at 40 m, 0–30 at 80 m and none beyond (some of them the
    beam itself). From the map's edge shooting outward, none.
  - **The cut:** 80 m keeps a sky shot's drawn points near the shooter, where single precision is ample. Past it a
    beam is under a pixel wide anyway.
- **Alternatives:** working out beam glow relative to the camera in the shader (the real cure, but the renderer
  works in map coordinates throughout); fading the tracer out instead of cutting it.
- **Rationale:** you suggested special-casing far-off tracers, keeping gameplay the same when players shoot each
  other.

### D67. Crouching, and shooting only standing still or crouched

- **Choice:**
  - **Crouching:** holding `Ctrl` (either one) lowers the eye to 60% of its standing height over 0.15 s and moves the player at
    half walking speed, with no sprinting. The numbers are in `game_settings.yaml`'s `movement`.
  - **Shooting:** a player can shoot only standing still or crouched (walking crouched included). Walking
    upright, running and jumping stop a shot, as running already did.
  - **Seen by others:** avatars hang the standing eye height below the eye, so a crouched player's box sinks
    into the floor by the drop. They look shorter and their head moves down with them.
  - **The house:** it has no crouch, so walking there stops shooting.
- **Alternatives:** `C` to crouch (my first pick, as on a Mac `Ctrl`+click can mean a right-click; but SDL hands
  the game `Ctrl`+click as a left click unless its `SDL_HINT_MAC_CTRL_CLICK_EMULATE_RIGHT_CLICK` hint is set, and
  you asked for `Ctrl`); a lower collision body when crouched (to fit under things; nothing on the map needs it yet).
- **Rationale:** you asked for crouching that slows walking, for shooting while crouched, and for no shooting while
  walking.

### D68. No respawning by choice

- **Choice:** the `R` key is gone. A player comes back at their spawn only after being eliminated (or a new game),
  so nobody can escape a fight or skip the walk back.
- **Alternatives:** keeping `R` without a server only (it helped testing alone; `--view` still starts a solo game at
  any viewpoint).
- **Rationale:** you asked to get rid of respawning by choice.

### D69. A crosshair

- **Choice:** four short white arms around the middle of the window, where shots and console holds aim. Each arm
  is 1.2% of the window's height long, 0.6% from the middle and 0.25% thick, sized by the height so it looks the
  same at any window size. It hides while a banner ("YOU DIED", a win) shows.
- **Alternatives:** a dot (harder to see on bright blocks); arms with a dark outline (more rectangles; white reads
  well on the dark sky and the lit blocks).
- **Rationale:** you asked for a crosshair.

### D70. A flashlight

- **Choice:** right-click turns a flashlight on or off. On voxel maps it lights surfaces in a cone from the eye along
  the view, measured along the curved space's geodesics:
  - **Cone:** full inside 0.3 rad of the axis, softening to nothing 0.08 rad further out.
  - **Range:** half as bright at 25 m.
  - **Facing:** as bright as the surface faces it.
  - **Brightness:** twice full sunlight, so its cone shows on the sunlit moon as well as in shadows.
  - **Settings:** all four numbers are under `flashlight` in `game_settings.yaml`.
  - **What it lights:** it casts no shadows (from the eye they would be hidden anyway), lights no dust in the air,
    and only its owner sees it.
- **Alternatives:** a light at the wand's tip (it would light the wand's side of everything and show shadows);
  other players' flashlights (each would need a bit in the snapshot and a light in the shader).
- **Rationale:** you asked for a right-click flashlight that shines in a neat cone.

### D71. A lobby where players choose their team

- **Choice:**
  - **The lobby:** a window that connects to a server first sees a lobby over a dimmed view, with each team's head
    count (asked of the server every 0.5 s) and "1 joins blue, 2 joins red". `1` or `2` asks to join that team.
  - **Teams:** the server keeps each player's team, keeps it when they rejoin after a hit, and ignores any other
    team in later messages. A join without a team (bots) gets the team with fewer players, blue on a tie.
  - **What a team decides:** snapshots carry each player's team (a bit in the laser byte). It decides their spawn
    point, their beam's and tip's colour, and which consoles they can turn off. Before, teams came from odd and even
    ids. Lasers still hit teammates.
- **Alternatives:** a separate lobby window or web page (more code than a banner); letting players switch team
  later (it would need a way back to the lobby).
- **Rationale:** you asked for a lobby page showing how many players are on each team, where players choose a team
  and then play as it, with friendly fire on and only the enemy's consoles open to them.

### D72. Hold the right button for the flashlight

- **Choice:** the flashlight shines only while the right button is held, instead of toggling on each click (D70).
- **Alternatives:** none considered; you asked for click and hold.
- **Rationale:** you asked for the flashlight to stay on only while the button is held.

### D73. Key bindings in YAML

- **Choice:**
  - **The file:** `key_bindings.yaml` lists every action the window takes from a key, mouse button or the wheel:
    moving, jumping, running, crouching, shooting, the flashlight, consoles, the guns, the lobby's team keys, the
    render resolution and releasing the mouse. Each action takes one or more controls: SDL's key names (`W`,
    `Left Shift`, `"1"`), `Mouse Left` and the other buttons, or `Wheel` (only for actions done once per press).
  - **Whose file:** it is each player's own, passed with `--key-bindings` (`KEY_BINDINGS=` with `run-demo`,
    `host-game` and `join-game`). It is not the server's: players can want different keys, while game numbers must
    match.
  - **Checks:** every action is required, and an unknown action or control fails, naming it.
  - **Keys follow the file:** keys are read by their place on the keyboard (scancodes), so they don't move with
    the keyboard layout. The HUD's help line and the lobby's hint are built from the bindings, so they stay right
    after a change.
  - **The flag:** `--key-bindings` joins the CLI vocabulary in the `clis` skill.
- **Alternatives:**
  - the bindings in `game_settings.yaml` (the server would impose them on everyone);
  - key names by character, SDL keycodes (they follow the layout, so `W` would move on an AZERTY keyboard, but
    held keys are read by scancode).
- **Rationale:** you asked for crouch, move, jump, switching weapons and the other keys to be settable in YAML.

### D74. Player settings and server settings

- **Choice:** the settings are two files.
  - **`server_settings.yaml`** (`--server-settings`, `SERVER_SETTINGS=`): what shapes play for everyone, which the
    server sends to every player. Movement, sprint energy, both guns (with their beams and recoil), consoles,
    respawn, how a hit slows a player, and the flashlight.
  - **`player_settings.yaml`** (`--player-settings`, `PLAYER_SETTINGS=`): each player's own choices, which change
    only their own experience.
    - `keys`: D73's key bindings, moved in from `key_bindings.yaml`;
    - `view`: a new mouse look speed (`look_rad_per_px`), the head bob and motion blur;
    - `hit_effects`: how strongly a hit reddens, shakes and blurs the screen.
  - **Replaced:** `game_settings.yaml` (D60) and `key_bindings.yaml` (D73), and their flags.
- **Alternatives:**
  - the flashlight's cone and brightness as a player's choice (it lights only its owner's view, but a brighter one
    shows more in shadows, so it is fairer on the server);
  - recoil as a player's choice (it changes aim, so it is the server's).
- **Rationale:** you asked to split the YAML into a part each player can change and a part that belongs to the
  server.

### D75. A menu of each player's settings

- **Choice:**
  - **Opening:** `Tab` (the new `open_menu` binding) opens a menu over the darkened view; `Esc` or `Tab` closes it.
    While it is open, the player stands still, can't look or shoot, and the game goes on (it doesn't pause, since
    others are playing).
  - **Rows:** every action's controls (`Return`, then the next key, button or wheel turn rebinds it; `Esc`
    cancels), then the numbers a player is likeliest to tune (mouse look speed, motion blur, the head bob's depth
    walking and sprinting, and a hit's red, shake and blur), changed with `Left`/`Right` in fixed steps between
    fixed limits, then a row that saves. Inside the menu, the arrows, `Return` and `Esc` work whatever the
    bindings say, so a bad binding can always be fixed.
  - **Saving:** to the `--player-settings` file if one was given, else to `~/.config/warped/player_settings.yaml`,
    which every later game loads when it exists. The file is written from the settings, so it has no comments.
  - **Drawing:** the menu is text in the same mask as the "YOU DIED" banner, now any size: 512×312 mask pixels,
    scaled by a whole number to at most 85% of the window's height, left-aligned, the chosen row in the banner's
    title colour, the view darkened by 80% (the banner's 55% left the text hard to read).
- **Alternatives:**
  - pausing the game while the menu is open (only possible offline);
  - saving into the repository's `player_settings.yaml` (every `git pull` would then conflict with it);
  - a menu drawn with a UI library such as egui (a new dependency and a second way to draw text).
- **Rationale:** you asked for a screen to change each player's own choices, such as hotkeys. It is one page now;
  W-49 adds pages to it.

### D76. Menu pages, and a map of the whole level

- **Choice:**
  - **Pages:** the menu has a tab bar (`MENU  [SETTINGS]  MAP`); `Q` and `E` turn to the previous and next page and
    work whatever the bindings say, like the arrows. The menu opens on SETTINGS each time.
  - **The map page** shows the whole map from above, drawn once at start-up (`map_layout.rs`, under 0.1 s for
    either map) in the map's Fermi coordinates (u to the right, v down), the coordinates its zones and boundary are
    given in. Each column shows the top of the highest structure shape or box there in its material's colour,
    brighter the higher it stands above the ground (full brightness at 12 m; lights always full), or else the
    ground. Structures are read with the game's own sphere tracer (`shapes.rs`), straight down through each
    column, and the ground's height comes from its terrain (`terrain.rs`). Everything is then lit from the top
    left by the slope of those heights, which shows the crater's walls, the mounds and the structures' edges.
    On top: console shapes as small squares in their team's colour,
    wormhole mouths as cyan rings at their Schwarzschild radius, the black hole as a black disk ringed in orange,
    the spawns, the zones' edges dashed, the boundary outlined with the outside darkened, and labels. On each frame
    the window adds the player: a disk in their team's colour, ringed in white, with a white line the way they
    face.
  - **Size:** Lunar Surface's boundary (256 × 144 m) with a 4 m margin fills a 966 × 556 picture; a map without a
    boundary (the house) shows everything on it with the same margin. The picture is drawn under the menu's text
    at 2 picture pixels per text-mask pixel, through a new picture texture in the Metal overlay (and an SDL
    texture with the CPU renderer).
- **Alternatives:**
  - the Poincaré disk, which shows the whole hyperbolic plane but shrinks the map's ends to slivers (kept for the
    minimap, W-48);
  - drawing the map with the ray tracer from high above (slow, and a perspective view hides what is under the
    rim);
  - other players on the map (left to the minimap, W-48, which shows players and shots in range).
- **Rationale:** you asked for a general menu with pages: one for the hotkeys, another showing a layout of the
  entire map.

### D77. The sniper reloads, and its ammo bar is one block

- **Choice:**
  - **Reload:** after a shot, the sniper (gun 1) can't fire again for `sniper.reload_s`, 2 s by default, a new
    server setting (0 turns it off). Before this, a click could fire again as soon as the quarter-second shot
    ended.
  - **Bar:** with the sniper in hand, the ammo bar (where the rifle's heat bar is) is one block, as wide as the
    other bars. It is full when the sniper can fire, and empty from a shot until the reload ends.
- **Alternatives:**
  - a block that fills up gradually while reloading (you asked for it to come back when the timer resets);
  - no reload, with the block only flashing empty during the shot (a quarter second is too short to see, and
    `game_design/DESIGN.md` calls for about one sniper shot every 2 s).
- **Rationale:** you asked for an ammo bar for the sniper of only one block, which comes back when the timer
  resets, letting you shoot again.

### D78. A soft map edge up the crater's wall

- **Choice:**
  - **Where:** on a map with terrain (Lunar Surface), the crater's wall is the edge, and the old invisible ellipse
    (`regions.boundary`, 128 × 72 m) no longer stops anyone. That ellipse stopped players right at the wall's foot
    along u, but partway up it along v. A map without terrain keeps the ellipse.
  - **How far in:** a player's edge fraction runs from 0 at `map_edge.start_wall_frac` up the wall (0, its foot) to
    1 at `map_edge.stop_wall_frac` (0.5, halfway to the rim, about 10 to 14 m up). "Up the wall" is the terrain's
    own wall coordinate, before smoothing (`Terrain::get_wall_frac`).
  - **Slowing:** a step that would take a player further out is cut by (1 − edge fraction). So the distance left
    shrinks in proportion each step and is never covered: "Zeno's paradox". Measured walking out on the crater
    floor at 8.2 m/s: 2.5 m/s 9 m up the wall, 0.36 m/s at 13 m, then ever slower toward about 14 m. Steps inward
    or along the wall keep full speed, so nobody is stuck.
  - **Stretch:** the overlay shader's `read_stretched_scene` pulls the scene out from the middle of the screen
    (up to 2.5 times at the middle, not at all at the corners), smears it along lines from the middle and splits
    red from blue a little. Its strength is the square of the edge fraction, so it starts gently and builds near
    the stop, times each player's `view.edge_stretch_frac` (1; 0 turns it off).
- **Alternatives:**
  - slowing all motion near the edge, inward too (players far up the wall would take minutes to come back);
  - the stop at the rim (players would see out over it, and the slowdown would stretch over 30 to 60 m);
  - the stop at the old ellipse (along u it is only 1 m from the wall's foot, so it would act like a wall);
  - a stretch growing in step with the edge fraction (already heavy at 0.4, while the player still moves at 60%).
- **Rationale:** you asked that players slow down without end as they move off the map, instead of meeting a wall,
  with the slowdown starting where they climb the hill, and that their view warp as if they were being stretched
  ever further.

### D79. A scores page in the menu

- **Choice:**
  - **The page:** the menu's third page, SCORES (`MENU  SETTINGS  MAP  [SCORES]`), shows how many players are in
    the game and on each team, then a row per player (team, `player <id>`, kills, deaths) sorted by team, then by
    most kills, then by id, with this player's row marked `(you)` and highlighted, then each team's totals. It shows
    at most 16 rows (a server takes 8 players). Without a server it says there is no game on a server; before the
    first scores arrive (in the lobby, or from an older server), that there are none yet.
  - **Counting, on the server:** the server already decides eliminations, and each hit report names its shooter
    (checked against the sender's address), so the killer is always known. An elimination counts a death for the
    player hit and a kill for the shooter, unless they are teammates (friendly fire is on, D71): then it counts the
    death only.
  - **Who is in the game:** the joined players, and those eliminated less than `respawn.delay_s` plus the 3 s
    timeout ago, so players don't drop off the page while they wait to respawn. A player who rejoins (same address,
    so same id) keeps their counts. Every count goes back to 0 when the server starts a new game.
  - **Protocol:** a new message, `Scores` (kind 11), sent 4 times a second to every player in the game, those
    waiting to respawn included: a count, then per player the id (4 bytes), team (1) and kills and deaths (4 each).
    Like every message, it is told apart by its kind byte: an older client ignores it, and an older server never
    sends it.
- **Alternatives:**
  - the counts in every snapshot (60 times a second for numbers that change a few times a minute, and a respawning
    player gets no snapshots);
  - counting on each client from the `Eliminated` messages (a lost message, or a player who joins late, would make
    the windows disagree);
  - a kill for eliminating a teammate too (it would reward friendly fire);
  - counts kept across games (the page then mixes games that were won or lost separately).
- **Rationale:** you asked for a menu page that shows how many players there are, and each player's kills, deaths
  and team.

### D80. A Poincaré-disk minimap, and your range on the map page

- **Choice:**
  - **Model:** the minimap is the Poincaré disk of the ground around the player, centred on the ground point under
    them and turned so they face up. It reaches `minimap.range_m` (60 m, 1.5 curvature radii on Lunar Surface),
    which lands on the rim at disk radius tanh(range / 2R). Geodesics away from the player bend into arcs, which
    shows the hyperbolic plane, while near the player it looks like an ordinary map.
  - **Drawing:** the overlay shader draws it (`draw_minimap`). Each pixel of the disk goes onto the hyperboloid,
    through the player's ground frame into the world, to its Fermi (u, v), and so to the map page's picture without
    labels (`map_layout.rs`), uploaded once. On it go faint grid lines every 10 m of u and v (a pixel wide, from the
    disk's metric), other players as dots, the lit parts of laser shots as lines (each segment sampled 6 times so
    it bends), and a white arrow for the player.
  - **Settings:** each player's `minimap` in `player_settings.yaml`: whether it shows, its corner (top right by
    default, away from the bars in the bottom left), its radius (14% of the window's height) and its reach. It
    hides under the menu and the banners.
  - **Map page (W-66):** everything farther than the minimap's reach from the player is darker and blurred (a
    3-pixel box blur, 55% brightness, made once), behind a white rim. The distance is hyperbolic, computed per pixel
    from a term per row and per column, so the ring comes out lopsided in the map's Fermi coordinates.
- **Alternatives:**
  - a flat, Euclidean minimap of the Fermi coordinates around the player (it would hide what is hyperbolic);
  - the whole Poincaré disk, out to infinity (you asked that it not show the whole map);
  - drawing the minimap on the CPU each frame (about 2 ms a frame, against nothing measurable on the GPU).
- **Rationale:** you asked for a round minimap that shows off the map's hyperbolic nature, just around the player,
  with players and shots in range, in a corner you can pick; and for the map page to ring your range and dim the
  rest.

### D81. Dust and mist from shots into the ground

- **Choice:**
  - **When:** a shot of either gun, by any player, that hits the ground (the flat ground or the crater's terrain)
    throws up a burst where it lands, when its beam front gets there. Shots into blocks or players raise none. At
    most 12 bursts show at once; a new one replaces the oldest.
  - **Grains:** a sniper shot throws 33 grains of grey-brown dust, a rifle shot 12 at 0.65 times the speed. Each is
    a small polyhedron (a tetrahedron, a cube or an octahedron, picked at random) 2 to 3.2 cm in radius, turned at
    random and tumbling at 4 to 14 rad/s while it flies. They fly up a cone (0.5 rad) around the ground's normal,
    tilted a little back toward the shooter, at 2 to 6 m/s, rising up to 3 m, then land on the ground's slope and lie
    there, still.
  - **W-86 (after you saw the first version):** the grains were spheres, 44 and 16 of them, 2 to 4.5 cm; you asked
    for polyhedra, smaller big ones and slightly less dust, so there are 25% fewer and the largest radius is 3.2 cm.
  - **Mist:** 7 puffs for a sniper shot, 3 for a rifle shot (0.65 times as big), lighter than the grains. They drift
    out at 1.5 to 3.5 m/s, slowing to a stop within about half a second, while they swell from 0.12 m to 0.6 to
    1.2 m.
  - **Fading:** each particle fades from a third of its life (0.9 to 2 s) to nothing, so a burst is gone within 2 s.
  - **Gravity:** the map's own, 6 m/s² on Lunar Surface, not the 30 m/s² the server settings give players for their
    quick jumps. At 30 m/s² the grains would be back on the ground within half a second, before anyone saw them in
    the air.
  - **Light:** the sun lights the dust like the ground: a grain's faces are flat, each as bright as it faces the
    sun (a little light wraps round, 0.2), so a tumbling grain flickers; a puff's side that faces the sun is
    brighter, and dust seen toward the sun glows. A burst in shadow (checked once, when it starts) gets the ambient light and
    some earthshine only.
  - **Drawing:** in the world, by the voxel ray tracer (`scan_dust`), so the dust is seen from any angle, through
    wormholes and behind blocks. Each straight piece of a ray is tested against each burst's bounding sphere, then
    against its particles in the burst's Klein coordinates, where the piece is a straight line. A grain is its
    polyhedron's faces (`GRAIN_PLANES`, hardcoded) turned by its quaternion: the face the ray enters gives the normal,
    and the ray's distance outside the faces, midway between where it crosses them, gives a pixel-wide soft edge
    (`cover_grain`). Grains are at least 1.5 pixels wide, puffs are Gaussians, and their optical depths add up along
    the ray. Each ray end carries
    its dust, so it grows from 96 to 112 bytes, and adaptive sampling blends dust between samples only where its
    depth changes by less than 0.02.
  - **Cost:** Apple M3 Pro, macOS 15.7.3, Rust 1.98.1, release build, 2560 × 1600 with adaptive sampling, medians of
    10 frames, runs alternated. With no dust on screen, each bench view takes 0.4 to 1.8 ms more GPU time than
    before (about 2%, from the larger ray ends). One sniper burst 12 m away adds 0.1 to 1 ms to a 33 ms frame; the
    rifle held on one spot 5 m away (12 bursts, 193 particles) adds 2.7 ms to 34 ms. Twelve sniper bursts at once
    (513 particles, out of reach in play with its 2 s reload) would add 11 to 20 ms. The polyhedra cost no more than
    the spheres did, as there are fewer of them: one sniper burst 6.5 m away added 2.6 to 3.2 ms with spheres and 1.5
    to 2.9 ms with polyhedra, one 2.7 m away 3.7 ms and 2.6 ms.
- **Alternatives:**
  - sprites drawn over the picture at the hit's place on screen (cheaper, but they would not bend through
    wormholes, hide behind blocks, or move in 3D as you walk around them);
  - the players' 30 m/s² gravity (see above);
  - sending the particles over the network (each window makes them from the shots it already draws, so everyone sees
    a burst where a shot lands, though the grains differ from screen to screen);
  - keeping ray ends at 96 bytes by storing the laser glow and the dust as half floats, which might win back the 2%
    at some cost in the glow's precision;
  - grains as spheres or soft discs (the first version; you asked for polyhedra), or as rocks of many faces (more
    work per ray for a difference too small to see at a few pixels).
- **Rationale:** you asked that shooting the ground throw some mist and dust particles up into the air, and then
  that the particles be polyhedra, the big ones smaller, and the dust slightly less.

### D82. Spaceships flying over Lunar Surface

- **Choice:**
  - **Ships:** four, purely for looks: a ring ship whose ring turns, and three fighters flying in a V. Each is a
    flat silhouette seen from below, drawn by the sky shader (`render_voxel.metal`'s `draw_spaceship`) from boxes,
    tapered capsules and triangles (tables `HULL_*`), lit by the sun on the edges that face it and from below by the
    sunlit ground, with glowing engines and a short plume, red lights to port and green to starboard, a flashing
    white strobe and lit windows.
  - **Delta fighters (W-88):** each fighter is one flat triangle, about as wide as it is long, with a darker spine
    and a dark glass canopy painted on (`HULL_INSETS`, which colour a hull without changing its outline), two
    engines at the back and nearly plain panels. Until W-88 they had a thin fuselage, thin swept wings and engine
    pods. You picked this wide delta over a slimmer one.
  - **No freighter (W-87):** a fifth ship, a container freighter with coloured containers, a lit bridge and orange
    engines, flew from #193 until W-87 removed it, with its hull tables and its pulsing red beacon.
  - **Paths:** each crosses the sky on a great circle from horizon to horizon, in 110 s (the fighters) or 300 s
    (the ring ship), highest at 84° and 74° above the horizon, then comes back round below the horizon in a fifth of
    that time. So a view straight up often has a ship in it.
  - **Shared time:** where the ships are depends only on the UTC clock (`spaceships.rs`), so every window whose
    clock is set by the network shows the same ships in the same place, to within its clock's error (tens of
    milliseconds, under a pixel). The game has no clock all clients share (the server sends no time). Fixed views
    (`--bench-views`) show the sky at time 0, so their images stay the same from run to run.
  - **Background only:** the ships are part of the sky. They show only where a ray escapes to the sky, behind
    everything (and bent through the lenses, like the stars), and nothing else traces them, so they have no
    collisions and no effect on lasers or play.
  - **Style, not map:** like the stars, they belong to the shader's `lunar` appearance style, not to the map: a
    map neither places them nor turns them off.
  - **Cost:** each sky pixel rules a ship out with a few dot products; only pixels within 1.5 half lengths of a ship
    shade its hull. On D10's shared machine, the window at 1280×800 ran 85.5 fps (11.1 ms on the GPU) with the
    ships and 85.7 fps (11.1 ms) without; `--bench-views` at 2560×1600 moved by −4 to +4 ms per view (mean +1 ms),
    within the noise, and views straight up with all five ships of then (the freighter too) in sight took about
    0.4 ms more on the GPU.
- **Alternatives:**
  - ships as structures traced like the blocks (properly shadowed, but every ray would pay for them, and they would
    be in the game's world);
  - a time sent by the server (exactly shared, but a protocol change for background art, and a window without a
    server would still need a clock);
  - whole orbits at a steady speed (half of each below the horizon, so looking up often showed an empty sky);
  - ships described in the map format (`common/maps/`), which only this renderer would read.
- **Rationale:** you asked for a few spaceships flying overhead in the sky, purely background world-building, to
  look cool and pretty; then, seeing them, that the colourful freighter go ("the freighter is indeed meant to be
  deleted and is ugly") and the ring station stay; and that the fighters become simple, geometric deltas without
  thin arms ("A wide delta").

### D83. Aiming zooms in, and the flashlight moves to F

- **Choice:**
  - **Keys:** a new `aim` control, the right button by default. The flashlight moves from the right button to `F`.
    It still shines only while held.
  - **Zoom:** holding `aim` zooms in, 2.5 times with the sniper and 1.5 times with the rifle (`aim.sniper_zoom` and
    `aim.rifle_zoom` in `server_settings.yaml`, at least 1). It eases in and out over `aim.zoom_s` (0.15 s, a
    smoothstep). The zoom divides the camera's `tan_half_fov` and `pixel_angle_rad` in the frame after it is packed
    (`PackedFrame::zoom_in`), for both renderers and both maps.
  - **Turning:** the mouse turns the view more slowly by the same factor, so aiming is steadier.
  - **The gun stays put** and keeps its normal field of view: the laser leaves its tip as before, and other
    players' windows, which don't know who is aiming, draw the same beams.
- **Alternatives:**
  - moving the gun to the middle of the view (the laser would then start somewhere else than other windows draw
    it);
  - zoom as a player's own setting (it changes what a player can see, so it is the server's, like recoil);
  - slowing walking while aiming (not asked for).
- **Rationale:** you asked for a way to aim on the right button, with the flashlight moved to another key.

### D84. A crisp colour shift while moving through a wormhole's throat

- **Choice** (W-82 (wormhole colour from motion) replaced the 1.5 s cyan tint after each crossing of W-52 (cyan
  through a wormhole); after testing, #272 moved it to the throat, and #289 made the colours crisp and the time twice
  as long):
  - **When:** the shift depends only on where the player is and how they move, with no timer. Its strength is the
    tint setting × how deep the eye is inside a mouth × the speed along the view (forward speed × cos pitch +
    vertical speed × sin pitch) in walking speeds, held within ±1.25. Forward gives a light cyan, backward a dark
    red, standing still or stepping sideways none.
  - **Where:** only inside the mouth, where it bends space strongly. With ρ the eye's distance to the mouth's centre
    and m the throat's radius (0.4 m on Lunar Surface), the depth is 1 within 1.8 throat radii (0.72 m), falling
    along a smoothstep to 0 at 9 throat radii (3.6 m), and 0 beyond. Through the throat the eye is at the other
    mouth's throat, so the shift carries on there over the same distance. `INSIDE_MOUTH_SCALE` (1.8) scales both
    radii.
  - **How long:** walking through mouth A, it shows for 38 frames (0.63 s at 60 fps), twice #272's 19 (0.32 s). The
    eye passes 1.2 throat radii (0.48 m) from the centre, not through it, so doubling both radii would have made it
    2.3 times as long (43 frames); 1.8 makes it exactly twice.
  - **The colour:** the overlay shader's `blueshift_scene` reads each pixel's red, green and blue as samples of a
    spectrum at 610, 550 and 465 nm that falls to 0 at 400 and 700 nm, and shows each channel's light at D times its
    wavelength, with D = √(1 − β²) / (1 − β cos θ) (cos θ taken half way from 1). Nothing is added over the image,
    so dark stays dark and edges stay sharp.
    - Forward: β = 0.16 at walking speed, D³ times as bright, shown at 45% over the scene: greys turn a light
      cyan, almost white.
    - Backward: as #259, β = −0.3, D times as bright, shown in full, with a faint red glow, (0.4, 0.04, 0) × (1 − D),
      on the black sky: greys turn a dark red.
    - Below strength 1 the result fades in.
  - **What it touches:** the scene only, after motion blur; the HUD, text, minimap and the gun in your hand stay as
    they are, since they move with you. Each window steps only its own player, so in a game on a server only that
    player sees it. The house map moves players by other rules, so it shows none.
  - **Settings:** each player's `view.wormhole_tint_frac` (1 by default; 0 to 1, 0 for none; also in the menu as
    `wormhole tint 0-1`). `view.wormhole_tint_s` is gone. A saved settings file that still has it loads, because
    `REMOVED_PLAYER_KEYS` in `settings.rs` names it to be dropped; any other unknown key still fails.
  - **Defaults you can change:** the shift carries on past the throat in the other mouth; red objects turn green
    under the forward shift, the real effect; it snaps on and off with movement, with no smoothing.
- **Alternatives:**
  - the whole 15 m reach, 1 − (ρ / reach)² (#259; you said it started far too early: "it should happen only for a
    split second as you are in it");
  - the full forward shift with β = 0.2 (#259; deep blue, which you called dark blue);
  - a blend 85% toward a pale blue-white or a warm white (#272; you said it was not crisp, "like diffused by this
    white screen");
  - a smaller forward shift without the D³ brightening (mint green rather than cyan on the warm ground);
  - both radii doubled (2.3 times as long on mouth A, since the eye misses the centre);
  - the player's real velocity direction per pixel (stepping sideways would tint one side of the screen one way and
    the other side the other), instead of the speed along the view.
- **Rationale:** you asked that the colour depend on where you are in the wormhole and how fast you move, not on a
  timer, go away once you are out, and redshift walking backwards. Testing #259: "You cannot start the
  transformation so early when going into the wormhole, it should happen only for a split second as you are in it.
  It happens when you are INSIDE it". Seeing #272: "for the "at the throat" thing you still need to have the dark
  red and the light cyan almost white instead of doing this "warm" stuff. It has to be CRISP (not like diffused by
  this white screen), like the previous pictures I was shown", "so overall your throat timing looks good", and "its
  just the specific colors should be a bit more crisp". Then: "make a PR that makes it take slightly longer; ideally
  just double its radius or whatever (i.e. walking at same speed should take exactly twice as long of time showing
  me cyan or red)".
- **Superseded in part by:** D105, W-82 (wormhole colour from motion): the colours are now option 8 forward and R2
  backward, which you picked from the colour sheets; where and how long the shift shows stays as here.

### D85. Slanted neon HUD bars that pulse (W-54)

- **Choice:**
  - **Look:** you picked, from four options rendered over Lunar Surface, B's shape with C's background, a bit more
    translucent than C. Each segment leans like `/` (its sides shift 0.45 of its height) with corners rounded to
    0.15% of the screen's height. An empty segment is a slot of dark glass (`[0.02, 0.04, 0.08]`, 38% opaque, C's was
    45%) outlined faintly in the bar's colour (35%). A filled one is the bar's colour at 60% (C's was 70%), lighter
    at the top and darker at the bottom (±20%), with a gloss over its top half (30%), an outline in its colour
    lightened 35% toward white (85%), and a glow in its colour 0.6% of the screen's height wide. Bars are 1.6% of
    the screen's height tall (1.8% before), with 0.35% of its width between segments. The console bar below the
    crosshair has the same look, as one slot with a filled part.
  - **Pulses:** a band of light (a Gaussian, 2% of the screen's width at half width, leaning with the segments)
    crosses a bar from 2.5 half widths beyond one end to as far beyond the other:
    - **Hit:** each hit sends one reddish-orange (`[1.0, 0.38, 0.12]`, 80%) pulse down the health bar, right to left,
      in 0.5 s. Another hit restarts it.
    - **Sprinting:** while you sprint, blue (`[0.15, 0.4, 1.0]`, 80%) pulses run down the energy bar, right to left,
      in 0.5 s, one every 0.8 s.
    - **Refilling:** while the energy refills, a fainter pulse (the bar's colour 65% of the way to white, 40%) runs up
      it, left to right, in 1 s, one every 1.6 s. Health gets one when it rises (a respawn or a new game); it doesn't
      regenerate. A sprint pulse cuts a refill pulse short; a refill pulse waits for a sprint pulse to finish.
    - **Ammo:** the rifle's heat and the sniper's block get no pulse, not even while the rifle cools, as you said
      ammo is normal.
  - **Drawing:** each overlay rectangle carries a style (corner radius, outline, gloss, drop shadow, slant, glow,
    shading) and a pulse. The overlay shader draws it from a rounded box's signed distance with half-pixel
    antialiasing: first every rectangle's shadow and glow (the strongest of each at a pixel), then the rectangles in
    order. The crosshair keeps a flat style. The look and the pulses are in `hud_bars.rs`. The CPU renderer's window
    draws the same rectangles as flat boxes, without style or pulse.
- **Alternatives:**
  - the four options as they were: A, glass capsules (rounded pills, a strong gloss, drop shadows); B, slanted
    tactical (on one dark panel); C, neon (upright slots); D, a slim track (thin segments in a dark rounded track);
  - pulses lighting whole segments one after another, computed on the CPU (a coarser, stepped band);
  - a refill pulse while the rifle cools (it would run nearly all the time while firing).
- **Rationale:** you asked for bars that look more professional, at least slightly translucent, slightly shiny and
  rounded, chose "slanted tactical but with the neo background", slightly more translucent than the neon example,
  and asked for a reddish-orange pulse down the health bar when taking damage, a blue one down the energy bar while
  running, none on ammo, and a much more subtle one while regaining things such as energy.
- **Since D94 (W-80):** wide swells over the filled part, paced by the smoothed rate of change, replace these
  pulses.

### D86. Ask before exiting on Escape

- **Choice:**
  - **When:** the release-mouse control (`Escape`) still frees a captured mouse first. Pressed again with the mouse
    free, it no longer quits: it shows "Do you want to exit?" with its keys, `Return/Y: exit   Esc/N: stay`, as a
    banner in the middle of the darkened view (the "YOU DIED" banner's mask, colours and 55% dimming). The prompt
    comes before any other banner. Inside the menu, `Escape` still just closes the menu.
  - **Keys:** `Return` or `Y` exits; `Escape` or `N` goes back, with the mouse still free. These keys work whatever
    the key bindings say, like the menu's arrows. While the prompt shows, every other key, click and the wheel does
    nothing, so the prompt and the menu never both take a key, and a click doesn't capture the mouse behind it.
  - **Play:** while the prompt shows, held keys are ignored, as under the menu, so this player stands still. The
    world goes on (a server's game doesn't pause).
  - **Close button:** the window's close button still quits at once, without asking.
  - **Code:** a small state machine, `exit_prompt.rs` (`ExitPrompt`), with its own unit tests; the window shows its
    lines through `get_banner_lines`.
- **Alternatives:**
  - an OK button to click on screen (the keyboard is enough, and the overlay has no buttons yet);
  - asking on the first `Escape` with the mouse captured (freeing the mouse is a common thing to do, so it would
    ask too often);
  - `Escape` exiting from the prompt, as a "press it again to confirm" (a player pressing `Escape` repeatedly
    would quit by accident, which is what the prompt guards against).
- **Rationale:** you asked that `Escape` ask "do you want to exit?" instead of exiting, exiting only if you press
  OK.

### D87. Consoles pulse faster and faster while being turned off

- **Choice:**
  - **Ramp:** a console that someone holds `E` on pulses 8^p times a second at progress p (the seconds held over
    `consoles.hold_to_disable_s`, from 0 to 1): from the normal 1 Hz up to a frenzy of 8 Hz right before it turns
    off. Each equal step of progress multiplies the rate by the same factor (2.8 Hz halfway, 6.5 Hz at 90%), so the
    rate climbs fastest at the end. This replaces D53's fixed double rate.
  - **Easing back:** when the hold stops (letting go, looking away, the holder hit or gone from the server), the
    rate falls exponentially toward the normal rate with a 0.3 s time constant: about half the way in 0.2 s, and
    within 4% after 1 s. A rate that has to rise (a new hold) follows the progress at once.
  - **No jumps:** each console keeps its own phase, which adds up its rate frame by frame instead of using
    sin(2π f t), so its light never jumps when the rate changes. The other consoles keep their 1 Hz pulse.
  - **Same in every window:** each player in the server's snapshot carries one more byte, how far along their hold
    is by the server's own timer (in 255ths). A window lights each console by the furthest of its holders, using
    its own seconds held for itself. The snapshot's length check drops snapshots between a server and a client of
    different versions, so both must be rebuilt.
- **Alternatives:**
  - a linear ramp, 1 to 8 Hz evenly (most of the change comes early and the end feels flat);
  - 10 Hz at the top (at 60 fps only 6 frames a pulse, and more flicker);
  - each window timing other players' holds from when it sees them start (no protocol change, but a player who
    joins mid-hold or a lost snapshot would put windows out of step);
  - the progress of each console in the game's state (32 bytes a snapshot instead of 1 per player);
  - easing down linearly over exactly 1 s (each console would need the rate and time its easing started).
- **Rationale:** you asked that a console's pulse speed up continuously, reaching a fervor right before it turns
  off, and slow down smoothly instead of jumping when the turning off stops.
- **Revisit if:** the flashing bothers anyone. 8 Hz is above the 3 flashes a second that accessibility guidelines
  (WCAG 2.3.1) allow for large, bright or red flashes. A console fills little of the screen, but
  `CONSOLE_FRENZY_PULSE_HZ` in `voxel_world.rs` lowers the top rate.

### D88. Lasers still stop on window glass (W-57)

- **Choice:**
  - **Finding:** the room is the red habitat module (and its twins, the derelict modules), with four windows one
    meter wide in each long wall. A shot at a window stops on the window's glass pane. It always did: the voxel walk
    stopped at any filled cell, glass included, while the picture looked through glass, so the window looked open
    but held the laser. That is the "hitbox". It isn't a fault of the tracer: the shot stops exactly on the pane's
    near face whether the shooter stands 5 cm, 30 cm or 3.5 m from it, aims square on or slantwise, or shoots in
    from outside. The shot is traced from the eye, so the gun's muzzle (`MUZZLE_M`, 0.555 m ahead) reaching past
    the pane changes only where the drawn beam starts.
  - **No change:** the map format makes `glass` see-through for light but a stop for lasers and players, as the
    WebGL level does (`demos/levels/lunar_surface/DECISIONS.md` D16). A new test,
    `the_laser_stops_on_a_window_pane`, keeps it so.
  - **Since #147 (W-59):** the smooth-shape export lays each pane over a solid hull wall, so the habitat modules'
    and walkways' windows no longer show the outside. A laser through glass would now stop on the hull 6 cm behind.
- **Alternatives:**
  - lasers through glass (the format and the level would change, and the walls would need holes behind the panes);
  - window holes in the walls without glass, as doorways are (players could climb through).
- **Rationale:** you reported that a laser shot out of a window close up doesn't leave the room; it stops on the
  glass by design, so whether lasers should pass through windows is your call.
- **Superseded by:** D90 (W-70): lasers now pass through glass, and the windows are real openings.

### D89. Shot noise: each shot strays a little (W-69)

- **Choice:**
  - **Per shot:** each shot strays from the crosshair by a random angle, toward a random direction around it,
    sampled once per shot (`shot_noise.rs`): once for the sniper's whole quarter-second shot, and once for each
    rifle pulse. The crosshair stays in the middle; only the shot moves.
  - **Sniper:** the angle is exponential with mean `sniper.noise_mean_rad` (3 mrad), cut at `sniper.noise_max_rad`
    (15 mrad, five means, so 0.7% of shots are cut). Most shots land within a few centimeters at 20 m, and about 1
    in 30 strays past 10 mrad. Cutting keeps the mean at 99.3% of the setting.
  - **Rifle:** a shot lands uniformly over a disc around the crosshair, of angular radius
    `rifle.noise_start_rad` (2 mrad) cold, growing in step with the heat to `rifle.noise_max_rad` (20 mrad) at full
    heat, the 20th shot in a row. The heat already falls over `rifle.cool_down_s` when the trigger is let go, so the
    spread recovers with it. The shot uses the heat before it adds its own, so a burst's first shot is the most
    accurate.
  - **Sizes:** in a 1280 × 800 window 1 mrad is half a pixel at the middle, so the strays are hard to see on screen
    but count at range: at 20 m the sniper's mean is 6 cm and the rifle's full-heat limit 40 cm, against a body
    50 cm wide. A sniper shot centred on a body hits it 99.5% of the time at 20 m, 97.5% at 30 m and 91% at 50 m; a
    rifle shot at full heat 74% at 20 m and 52% at 30 m (simulated with these defaults, sideways misses only).
    Aiming's zoom (D83) doesn't change them. Each setting can be 0 for none; `noise_start_rad` above
    `noise_max_rad` fails.
  - **One frame for everything:** the shot is traced from the camera turned by its offset (the smallest rotation
    taking the crosshair's direction to the strayed one), so the drawn beam, the glow where it lands, the HUD's
    line and the hit decision all come from the same trace. This holds on the house too.
  - **Other players:** while the laser is on, the window sends that turned frame as its eye frame, and other
    windows already trace a firing player's laser from the frame they receive, so they draw the same strayed beam.
    The network format doesn't change, so older windows draw it too. The body others see turns with it, by at most
    1.1° (the rifle at full heat) and only while the shot lasts, which doesn't show.
  - **Randomness:** a SplitMix64 generator (a few lines, no new dependency), seeded by the clock when the window
    starts. The tests use a fixed seed.
- **Alternatives:**
  - a new field in the player record for the offset (a snapshot's length is checked exactly, so an older window
    would drop every snapshot from a newer server), or a new message kind for it (more code for the same result);
  - a Gaussian spread for the rifle (no hard limit; the disc makes the limit the ring players can learn);
  - spread by shots in a row rather than heat (heat already counts them and cools gradually, so the spread
    recovers gradually rather than all at once);
  - rerolling a sniper stray beyond the cap instead of cutting it (the same mean within 1%; cutting is simpler).
- **Rationale:** you asked for a little random shot noise on the laser, sampled per shot, growing as the rifle keeps
  shooting, exponential for the sniper, and all of it in the YAML settings. The defaults keep the crosshair
  meaningful: the sniper still hits what it is centred on nearly always within 30 m, while a long rifle burst
  spreads wide enough that short bursts pay off.

### D90. Windows you see and shoot through (W-70)

- **Choice:**
  - **Walls:** the level's exporter (`demos/levels/lunar_surface/shape_builders.js`, `add_wall_around_windows`)
    builds each habitat module's and walkway's side wall as a piece below the window row, a piece above it, and a
    pillar between each pair of windows, so each glass pane fills a real 1 m hole. The pieces keep the wall's 1 m
    thickness and 0.3 m rounding. Each pillar reaches one cell into the pieces above and below, so its rounded ends
    are buried and no seam shows. The panes are unchanged: 1 m blocks, 6 cm proud of each face. The map goes from
    340 to 398 shapes (a habitat module from 29 to 41); no other structure changes.
  - **Format:** `glass` is now see-through for light and lasers, and stops players (`common/maps/README.md`). The
    schema and validator never encoded it.
  - **Lunar Surface:** the laser's tracer leaves glass shapes out (`find_hit_mu` takes a see-through material), so
    the drawn beam and its spot go on to whatever lies behind. The console aim ray (`E`) passes through glass too.
    The picture already did; `render_voxel.metal` needed no change.
  - **Players:** glass still stops them: their collision probes count every shape.
  - **House:** its laser passes through glass boxes too (`Scene::ForLaser`). But the house's windows are glass laid
    on solid plank walls, and both house renderers draw glass as an opaque sheen, so a shot at a house window stops
    on the wall 6 cm behind the glass, which looks the same as before.
  - **WebGL level:** its laser passes through glass too (one line in `trace_structure_cpu`). Its voxel walls
    already had holes behind the panes.
  - **Tests:** `the_laser_stops_on_a_window_pane` (D88) became `a_laser_passes_through_a_window`: shots from inside
    (touching the pane, close, across the room, slantwise) leave the module, a shot from outside comes in to the
    floor, and the pane's middle is still solid for players. `the_laser_passes_through_a_house_window` and
    `a_line_passes_through_a_see_through_pane` cover the house and the tracer.
  - **Frame time:** no change measured. The machine's GPU was shared with other agents' renders, so the same view
    varied by up to 40% between runs at 2560 × 1600.
- **Alternatives:**
  - a second glass kind that stops lasers (City Sphere's design in `game_design/DESIGN.md` wants a glass floor that
    players see through but can't shoot through; the format can add one when that level is built);
  - thinner panes in the middle of the wall (the level's 1 m panes were kept);
  - holes without glass, as doorways are (nothing would show it is a window, and players could climb through);
  - see-through glass in the house as well (its walls would need holes and its two renderers a glass pass);
  - a stronger glass look, such as a reflection, so panes show from outside (the voxel renderer tints glass 20%
    darker, so from outside a window reads mostly as an opening).
- **Rationale:** asked whether windows should be see-through and let lasers through, you answered "Both".

### D91. A window says why it can't play on its server (W-72, W-78)

- **Choice:**
  - **No answer:** a window shows "NO ANSWER FROM THE SERVER" and the server's address when nothing has answered its
    messages for 2 s, and prints a fuller hint in the terminal once. It counts only time with a message waiting for an
    answer, so a long first frame, when the window sends nothing, doesn't count.
  - **Another version:** "SERVER RUNS ANOTHER VERSION" shows when the server's snapshots have had the wrong length
    for this game, and none the right one, for a second. A welcome whose settings this game can't read stops the
    game with an error saying the server runs another version.
  - **Full:** a full server now answers a join with a new `Full` message (kind 12, which old clients ignore). The
    window shows "THE GAME IS FULL".
  - **Joining:** after `1` or `2`, "JOINING THE GAME" shows until the welcome, instead of no banner.
  - **Finding a server:** `--join auto` listens for 1 s after the first announcement and joins the server on
    `DEFAULT_PORT` if one announced itself, else the first heard, printing the others.
  - **Server log:** a join now prints the window's address as "their window's address", so it isn't mistaken for
    the server's port.
  - **Shooting (W-78):** a player in a game on a server can shoot only once welcomed: not from the team screen,
    while joining, or while out after a hit.
- **Alternatives:**
  - a version number in the join message (part of W-58, another agent's);
  - a list of servers to pick from, when several announce themselves (more than this needed).
- **Rationale:** you couldn't join with `1` or `2`, which turned out to be a join sent to a port no server listened
  on, and nothing said so. You asked to cover the cases where a server doesn't answer, and that you can't shoot
  before leaving the team screen.

### D92. Playing is only on a game server

- **Choice:**
  - **Make targets:** only `make serve-game` and `make join-game` play. `make run-demo` (the local demo) and
    `make host-game` (a server and a window in one) are gone.
  - **Default map:** Lunar Surface for both targets, so a server and a window started with the defaults play the
    same map.
  - **The program:** it refuses `--host`, and a window started without `--join`, with a message saying to start a
    server and join it.
  - **Tools kept:** `--serve`, `--join` (with bots), and the tools that need no server (`--bench-views`,
    `--autopilot`, `--play-route` and `--export-map`) still run, because the tests, benchmarks and screenshots use
    them.
  - **Code kept:** the `--host` code path stays, so turning it back on is a one-line change.
- **Alternatives:**
  - deleting the local play and `--host` code (a bigger change, and harder to undo);
  - keeping `host-game` (you asked for only starting a server and connecting to one).
- **Rationale:** you recommended disabling everything except starting a server and connecting to one, since nobody
  will use the local demo.

### D93. Crouching refills the sprint energy faster

- **Choice:** `sprint_energy.refill_s` becomes two settings. `refill_crouching_s` (7.5 s, as fast as sprinting
  empties the bar) applies while crouched; `refill_walking_s` (15 s, half as fast, the old rate) applies while
  walking or standing upright. Sprinting drains the bar as before (`drain_s`, 7.5 s).
- **Alternatives:**
  - a third rate for standing still upright (you named only crouching and walking, so standing keeps the walking
    rate);
  - keeping `refill_s` beside a crouch multiplier (two rates in seconds read more plainly).
- **Rationale:** you asked that crouching recharge the energy for running at the same rate as running uses it, and
  walking at half that, as YAML defaults that can be changed.

### D94. Soft swells along the bars' filled part, paced by how fast they change (W-80)

- **Choice:** you picked variant D from four rendered samples (A quick glints, B rate-paced, C soft swells, D C's
  swells made wider and rate-paced). It replaces D85's pulses:
  - **Where:** the swells run over a bar's filled part only. Down from the end of the last filled segment (the last
    hit point, or the last energy slot) toward the bar's start while the value falls; up from the start toward that
    end while it rises. They fade in and out smoothly inside both ends, over half a half width (at most half the
    filled part), so the empty slots stay dark.
  - **Shape:** a wide Gaussian glow, 4.5% of the screen's width at half width (about a quarter of a bar) and 1.1 bar
    heights tall, over segments and gaps alike. The overlay shader draws it after the segments (`get_pulse_glow`):
    it tints toward the pulse's colour and adds 0.6 of that colour as light. Up to 4 swells run on a bar at once.
  - **Rate:** each bar keeps an exponential moving average of its value (0.3 s time constant), and the average's rate
    of change drives everything. Strength is that rate over 15% of the bar per second, at most 1. It sets the swells'
    opacity (50% at full strength while falling, 45% while rising), their speed (0.8 to 2.2 bar widths a second) and
    the time between them (0.9 s down to 0.45 s). A swell starts each period while the strength is at least 5%.
  - **What that gives:**
    - **Hit:** a burst of 2 or 3 swells over about a second, fast and strong at first, then slower and fainter.
    - **Sprinting:** a steady train.
    - **Refilling:** about half as strong, as the energy refills at half the rate it drains (faster while crouching,
      D93).
    - **Respawn:** a strong rising burst.
    - **No change, no swell:** full health, full energy, or sprint held with no energy left.
  - **Colours:** reddish-orange `[1.0, 0.45, 0.2]` on health; pale blue-white `[0.8, 0.94, 1.0]` on energy (lighter
    than D85's deep blue, which looked dark); the bar's own colour 65% of the way to white while rising. Ammo still
    never pulses.
- **Alternatives:**
  - A, B and C, as rendered (narrow glints, medium rate-paced glows, narrower swells at a fixed pace);
  - the glow only over the segments (it read as whole segments lighting up);
  - pulses started by events (a hit, the sprint key) rather than by the value changing (they would not follow how
    fast it changes, and a server with endless sprint would pulse while the energy stays full).
- **Rationale:** you asked that the pulsing start where the filled part ends, that the blue be less dark, that the
  pulses look Gaussian, more subtle and faster, in proportion to the smoothed rate of change; then for "soft swell"
  but rate based, "really wide gaussian like; think like smoother convolution vibes".

### D95. A step up is searched from the bottom (W-85)

- **Choice:** when a step forward is blocked, the walk tries lifts of 2, 4, 6 cm and so on up to 30 cm
  (`MAX_WALK_RISE_M`), takes the first that clears, and bisects between it and the lift before it. It used to try 30
  cm first and give up if that didn't clear.
- **Why it got stuck:** a structure's floor is a geodesic plane, so a step toward a habitat module's door or along a
  walkway lifts it 1 to 2 cm into the feet, and the walk needs a small lift. The doorways and walkways are only 2 m
  high, so a 30 cm lift put the head (15 cm above the 1.6 m eye) into the lintel or the roof, and the walk gave up.
  The player stopped about 1 m before a module's door and moved only 0.3 to 0.5 m in a walkway.
- **Alternatives:**
  - a smaller `MAX_WALK_RISE_M` (it would stop climbing the low curbs and plinths it climbs now);
  - a lower head probe (the head would show inside low ceilings);
  - a taller doorway in the map (only those doorways, and any low ceiling added later would trap players again).
- **Rationale:** the smallest lift that clears is what a step needs. Searching from below finds it wherever there is
  room for it, and costs at most 15 more checks, only on a blocked step.

### D97. Every other player on the menu's map page (W-90)

- **Choice:** the MAP page shows every other player in the game as a dot in their team's colour, 4 px across the
  picture's radius (yours is 5 px), ringed in dark grey (yours is ringed in white and points the way you face). It
  shows both teams, as the minimap does, and players beyond your 60 m ring too, undimmed. A dot sits over the ground
  point under the player's eye, and your own disk is drawn over any dot under it. The list is the one the minimap
  uses, the server's last snapshot, so it already leaves out players who are dead or still in the lobby: the
  server sends only players who have joined a team and drops a player when they are hit, until they rejoin.
- **Alternatives:**
  - teammates only (you can ask for it; the minimap shows both teams too);
  - only players within your ring, like the minimap (the page shows the whole map, so far players are worth seeing);
  - a label with each player's number, as the spawns have (more clutter, and the scores page lists the numbers);
  - a line for the dots in the legend (it is full: 62 of the 63 characters that fit, so a new line would push out
    another one).
- **Rationale:** you asked to show where other people are on the map, like the minimap's dots. A dark ring keeps a
  dot apart from your own white-ringed disk, the console markers and the spawns, and shows on both the bright and
  the dimmed ground.

### D96. Thrown wormholes

- **Choice:**
  - **Keys and colours:** `throw_wormhole` (`Q` by default) throws the player's purple mouth, the next press the
    green one, then purple again, and so on; a press while a throw is in the air does nothing. Each new throw moves
    that colour's mouth once it comes down.
  - **Flight:** the mouth leaves the eye 0.3 rad above the crosshair at 16 m/s and falls at 20 m/s² (`wormholes` in
    `server_settings.yaml`), straight through other warps' bending. From whatever it hits (the ground, or on Lunar
    Surface any structure) it drops to the floor below and hovers at the map's eye height there, its back axis
    facing back along its flight, so a player walking the way it flew goes through.
  - **Fizzling:** a throw that hasn't landed in 4 s, or lands where its mouth doesn't fit, fizzles with a message;
    the old mouth of that colour stays and the next throw is the same colour. On Lunar Surface a thrown mouth is
    1.6 m (Schwarzschild radius) with a 6 m reach, which may not overlap any warp's reach (the tapered bending
    adds up wrongly where they do). On the house mouths share the house's size, and a thrown mouth's 1.6 m jump
    sphere must hold no box or ground and stay clear of every other mouth's (D23).
  - **Linking:** once both are down, the pair is added to the world after the map's own warps, so the renderer,
    walking and lasers all use it; anyone and any laser goes through, teammates or not.
  - **Looks:** a ray that came through a thrown mouth is tinted its colour (45%, by brightness), which colours the
    mouth's disk. A mouth in the air, or down while its partner isn't, is a glowing ball (a cube on the house).
  - **Network:** every player's state carries their mouths (each colour's place as u, v, h and heading) and the
    throw in the air (its point), 49 bytes, so a snapshot of 8 players stays under the 2048-byte datagram. A
    player who dies is out of the snapshot until they rejoin, so their mouths vanish for everyone, and their own
    window clears them.
  - **House limit:** the house shader holds 24 mouths instead of 8 (6 the map's, 2 for each of 8 players).
- **Alternatives:**
  - a throw that flies straight to the first surface, or one placed a set distance ahead (you chose a lob);
  - keeping mouths after death, or allowing only two throws (you chose alternating, cleared on death);
  - mouths only the thrower's team can use (you chose everyone; the renderer would need a world per team);
  - gravity of 9.8 m/s²: on the house (R = 8 m) a 16 m/s throw then curves away from the ground for good;
  - bending throws near warps like light (they would cross wormholes, at the cost of the flight's simple path);
  - checking another player's mouths for overlap again on arrival (two throws landing at once may overlap).
- **Rationale:** you asked for each player to throw a linked purple and green pair with `Q`, on both maps, lobbed,
  alternating, cleared on death, and usable by everyone. With no thrown mouths, the house's 24-mouth limit costs
  nothing measurable: `make bench-views` at 1280×800 (30 frames, Metal, Apple M3 Pro, macOS 26.5.2, Rust 1.98.0,
  release build) took 3.3 to 13.6 ms per view at its fastest frame with 24 mouths allowed and 3.6 to 13.9 ms
  with 8.

### D98. Shooting enemy wormholes (W-89)

- **Choice:**
  - **A hit:** a shot that goes through the throat of a mouth of a player on the other team counts, and goes on
    out of the other mouth as before; so does a shot that hits the glowing ball of a mouth down alone. Each
    shot counts once (the sniper's 0.25 s beam and each rifle pulse), whichever mouths it goes through.
  - **Destroying:** 2 sniper shots or 5 rifle shots destroy both mouths (`wormholes.sniper_hits_to_destroy` and
    `rifle_hits_to_destroy`), adding up across the pair's two mouths. The damage stays until they are destroyed:
    moving a mouth by throwing it again keeps it, and dying clears the mouths as before (D96).
  - **Flash:** each hit makes both mouths of the pair flash red for `wormholes.hit_flash_s` (0.25 s), for everyone,
    the owner included: the view through them is tinted red instead of their colour, and a lone mouth's ball turns
    red.
  - **Who counts:** the server, like hits on players: a shooter's window reports a shot through an enemy's mouths
    (`WormholeHit`), the server adds it up (ignoring teammates' shots and players with no mouth down) and sends each
    player's damage in the snapshot (one byte); the owner's window destroys the pair at 1, and the server resets the
    damage once the owner has no mouth down.
- **Alternatives:**
  - mouths that block enemy shots, or any shot through their bending (you chose shots through the throat);
  - 1 or 5 sniper shots (you chose 2), damage that heals or resets on a new throw (you chose only when destroyed);
  - flashing only the mouth hit (you chose both).
- **Rationale:** you asked that shooting enemy wormholes make them flash red, and that a few shots (5 rifle shots)
  destroy both.

### D103. Zone walls: see-through slabs straight above the zones' edges (W-64 (zone planes))

- **Choice:**
  - **Where:** each edge of the blue and red zones (u = ±45 m on Lunar Surface) is a wall around the points
    straight above the edge in Fermi height, from the ground up to `regions.zone_walls.height_m` (6 m) above it, so
    its foot and its top follow the ground over the mounds and up the crater's wall. It ends at the playable ellipse
    (`regions.boundary`), fading out over its last 4 m. The edge is a geodesic of the ground, so those points are part
    of a geodesic plane.
  - **Thickness:** the wall is the slab within half of `regions.zone_walls.thickness_m` (0.6 m, a player's width) of
    that plane, ±0.3 m, the same thickness everywhere. A ray is inside it where |⟨x, n⟩| ≤ sinh(0.3 m / R), an
    interval in tanh s found in closed form.
  - **Look:** a glowing slab in the team's colour. Seen straight through, it is `regions.zone_walls.opacity_frac`
    (0.25, level A, the user's pick) opaque at the ground, falling to 0.6 of that within about 0.4 m and on to about
    0.36 of it near the top, then fading out over its top quarter: a soft top edge. Of that, 70% is in its two
    faces, each thicker as you look along it (up to 4 times), and 30% is spread through the slab, so seen at a slant
    the tint builds up with the way through it: edge-on and from above it shows as a band as wide as the slab, and
    from beside it the feet of both faces show as two lines. Far away it thins in the haze. It is blended like the
    dust of shots (into the ray end's `dust`), so the adaptive sampling refines where the wall meets the ground.
  - **The painted line:** gone on maps with zone walls (the wall's brighter foot marks the edge); a map without them
    keeps it.
  - **Map format:** the height, thickness and opacity are map data, the optional `regions.zone_walls` (so the format
    stays version 1); renderers choose the fade and the glow.
  - **Physics:** the walls are only drawn: they stop neither players nor lasers.
- **Alternatives:**
  - opacity 0.5 at the ground (level B in the PR's first pictures);
  - 8 m or 5 m tall (8 m fills much of the sky from 10 m away; 5 m hides behind the mounds);
  - a slab 0.6 m wide in Fermi u (it would widen away from the middle line, to 1.4 m at the crater's edge);
  - all of the tint in the faces, or all in the slab (the faces alone don't show the thickness; the slab alone
    hardly shows the faces);
  - a wall rising from h = 0 or to a fixed top (it would be buried by the crater's wall and float over hollows);
  - walls along the whole edge, out to infinity (seen from above they would run out over the crater's rim);
  - keeping the painted line under the wall, or a grid on the slab (more to look at than a plane);
  - a separate ray-end field for the walls (16 more bytes per sample, where the dust's blending already fits).
- **Rationale:** you asked that the painted lines become planes jutting out of the ground, just barely translucent,
  to mark out the real zones, picked level A ("Level A, faint"), and then asked: "the blates should also have a little
  bit of thickness (rougly 1 player worth of thickenss)". A point is in blue's zone exactly when its Fermi u is below
  −45 m, at any height, so the slab around the plane straight above the edge marks the zone's real boundary. GPU
  cost, in interleaved runs against `staging` at 2560 × 1600 on an Apple M3 Pro (macOS 15.7): with thin planes, 6
  rounds of 7 views summed to 246.2 ms on `staging` and 244.3 ms with the walls (−0.8%); with the slabs, 4 rounds of
  7 views facing the walls gave each view's least median GPU time 1% to 5% higher (about 1 ms in 26 ms) in 6 of
  them; both are within the run-to-run noise (single views ±12%).

## Baselines

### D99. Shot noise in the beam, for W-84 (shot noise in the beam itself): pixels turn black, white or drop out

- **Choice:**
  - **Each gun's beam:** the sniper's is the lightsaber blade of W-92 (lightsaber lasers, rifle first, guns further
    out; #256): a flat-topped white core in a soft, pale haze. The rifle's is the laser from before #256: a Gaussian
    core whitened at its middle over a halo in the team's colour. Each beam carries its gun (`LaserBeam.is_rifle`,
    packed into `GpuBeam.info.w` as the team plus 2 for the rifle), so `scan_beams` shapes its core and
    `shade_ray_end` shades it by gun. Other windows know the gun from the rifle bit already in the player record.
  - **The speckle:** each frame, a share of the beam's screen pixels is speckled, and the rest is the gun's normal
    beam. A speckled pixel is one of three kinds, picked by its own roll in the shares of `beam_speckle` in
    `server_settings.yaml`, 1 : 1 : 8 by default:
    - black (`black_frac`, 0.1): the colour darkened by the beam's glow there, black where the glow is full;
    - white (`white_frac`, 0.1): the colour moved toward white by the glow, never brighter than white;
    - dropout (`dropout_frac`, 0.8): the beam is left out, so what is behind shows through: the sky over the sky,
      ground specks over the ground.
    Only pixels where the glow is above 2% count as the beam's, so no specks show beyond its visible edge. The
    pixels are picked anew each frame and independently for each pixel (`hash_pixel` on the pixel and a seed from
    the frame's clock, in milliseconds), so they flicker as single pixels.
  - **The share, per shot (`shot_noise.rs`), from 0 to 0.5:** the sniper's is exponential with mean
    `sniper.beam_speckle_mean_frac` (0.2), cut at `sniper.beam_speckle_max_frac` (0.5, for 8% of shots), so most
    shots are lightly speckled and some heavily. The rifle's grows in step with the heat, from
    `rifle.beam_speckle_start_frac` (0) on a cold rifle to `rifle.beam_speckle_max_frac` (0.5) at full heat, like a
    weapon malfunctioning as it overheats: the heat is the one before the shot adds its own, so a burst's first shot
    is clean and its 20th is 47% speckled, and it falls back as the rifle cools. The shares are 0 for none; a share
    above 1, a rifle start above its maximum, or kinds that don't add up to 1 fail.
  - **Every window draws a shot alike:** the share is rounded to steps of 1/30 (so 0.5 and 0.2 are steps) and sent
    in the five unused high bits of the laser byte of the player record (bits 3 to 7; bits 0 to 2 are the laser, the
    rifle and the team), so the snapshot keeps its length. Older windows read only the low bits and draw plain
    beams; an older server drops the bits, so others' beams show plain. Other windows start a tracer with the share
    they receive with the shot; the kinds' shares come with the server's settings.
  - **On the GPU:** each beam segment carries its share in `GpuBeam.info.y`; the ray keeps the share and code of the
    beam whose halo is brightest, packed in `VoxelRayEnd.surface.w` beside the team bit and the tint bits of W-60
    (throw wormholes): bit 0 the team, bits 1 and 2 the tint, bit 3 the rifle, the share from bit 4. The tint
    used to read every bit above bit 0 and now reads its two. `shade_ray_end` hands the pixel's colour with and
    without the beam to `speckle_beam`.
  - **Kept:** #216's aim stray (D89), unchanged: you called it neutral.
- **Alternatives:**
  - the lightsaber for both guns (#256) or the laser for both (before it);
  - shares from 0 to 1 (the version just before: you then asked for 0 to 0.5);
  - dropout alone, with the surviving pixels brighter and whiter (the version this PR first had; you asked for the
    beam otherwise to stay the normal beam);
  - other ratios of black : white : dropout (1:1:4, 1:1:2, 2:1:7, 1:2:7, 1:1:18, in the samples);
  - grain inside the glow, bright motes around the beam, or a glitchy beam with gaps and jumps (the first samples);
  - deriving the share in each window from the shooter's id and shot count (the rifle's depends on its heat, which
    other windows don't know);
  - picking pixels after MetalFX upscaling (the overlay pass doesn't know where beams are): at a render scale under 1
    the specks are render pixels, up to 2 window pixels across.
- **Rationale:** you asked that the laser beam itself have shot noise, "a combination of black AND dropout AND
  white": "i recommend the ratio is always 1 black to 1 white to 8 dropout and the total amount at this fixed ratio
  is scaled as I had described" (random per sniper shot, rising with the rifle's shots). Then: "stick with 118 and
  goes from amount 0 to 1 as you fire more, exponential average should be at 0.2". Then: "I see Leah made the
  lightsaber thing. That's OK. Let's do lightsaber for the sniper and do the old laser view for the rifle. ALSO add
  the SPECKLE on top of that with the 1/1/8 and 0.0-0.5 ratio with the 0.2 mean for exponential distribution for
  sniper."

### D100. A stronger curvature, set in the server's settings, for W-37 (stronger curvature)

- **Choice:**
  - **Setting:** `space.curvature_per_map_curvature` in `server_settings.yaml`, from 1 to 2, default 1.5: the
    curvature (1 over the curvature radius) as a multiple of the map's own. Lunar Surface's 40 m radius becomes
    26.7 m (20 m at 2). A value outside 1 to 2 fails with the key's name. A curvature k times as strong in this sense
    makes the Gaussian curvature, −1/R², k² times as strong.
  - **Default 1.5, not 2:** the item asked for twice the curvature by default. After seeing pictures at 1, 1.5 and
    2, with the speckle at 2 described below, you chose 1.5: "1.5x curvature is fine". 2 stays allowed.
  - **What stays in meters:** every frame (structures, warps, spawns) and lamp keeps its Fermi coordinates (u, v, h)
    and its turn against the Fermi axes there (`Map::scale_curvature`). Shapes in their charts, the terrain, the
    zones, the warps' reach and the players' speeds and jump keep their meters. So distances along the middle line
    (v = 0) and straight across it stay; along u away from it, a stretch Δu is cosh(v/R) Δu long, which at
    v = 40 m grows from 1.5 Δu (at 1) to 2.4 Δu (at 1.5) and 3.8 Δu (at 2). A structure's chart must reach less
    than R from its origin; Lunar Surface's furthest shape corner is 15.8 m, so it could go up to 2.5.
  - **Where:** voxel maps only (Lunar Surface). The house demo keeps R = 8 m: at 2 its far pylon would sit 13
    curvature radii out (coordinates near 2 × 10⁵), and its scripted route (`--autopilot`) flies back and forth
    through wormhole 1 without end.
  - **Networking:** the server sends its settings in the welcome (as before). A window that joins builds the map at
    the built-in value; if the welcome's differs, it rebuilds the map, the minimap and the map page at the server's
    and says so in the HUD. The renderer uploads the minimap's picture again when its placement changes. A bot
    (`--bot`) fails with a clear error on a server with another value.
  - **Beams:** they are drawn up to 160 m from the map's origin at any curvature, instead of 4 curvature radii (D64),
    which at 1.5 would have hidden beams fired from either spawn (both lie beyond 4 radii, 107 m).
- **Speckle at 2:** the renderer works in the map's coordinates in single precision, and a point's error there grows
  as cosh² of its distance from the origin in curvature radii. Beyond about 90 m at 2 (both spawns, the red pylon
  and the buildings around it) surfaces and shadows speckle; at 1.5 the limit is about 120 m (a faint grain at the
  red spawn's roof), and at 1 about 170 m, outside the map. The cure is to trace relative to the camera, a larger
  change to the shaders (D64 and D66 put it off too); until then, 2 speckles at the spawns.
- **Tests at the map's own curvature:** the tests that aim or walk by the map's numbers load it at 1 (`Map::Load`
  doesn't scale): at 2, the laser into the wormhole misses the red pylon, the walk into the mouth misses it, and
  the dust test's shots land elsewhere.
- **Alternatives:**
  - a default of 2, as the item first asked (speckle at both spawns until the renderer traces relative to the
    camera);
  - keeping geodesic distance and direction from the map's origin instead of Fermi coordinates (spacing around the
    origin would grow by sinh: 10 times at the spawns at 2, and structures would leave the terrain they stand on);
  - scaling the curvature radius by 1/√k for k times the Gaussian curvature (you asked for the radius ÷ factor).
- **Rationale:** you asked that the curvature be set in the YAML settings, higher than now, at most twice the
  current curvature, and then chose 1.5 as the default after seeing the speckle at 2.

### D101. Giant ships far in the sky, for W-83 (giant ships in the distance)

- **Choice:**
  - **Ships:** two giants far behind D82's ships, purely for looks, as original designs in the spirit of the films'
    (no copies, and no film names in the code, which calls them `battle_station` and `capital_ship`): a moon-sized
    battle station and a long wedge-shaped capital ship. `spaceships.rs` flies them like the others, first in
    `FLIGHTS`, so the shader draws them behind the small ships.
  - **Battle station (`draw_battle_station`):** a sphere 8.6° across, about half the planet, 14° above the horizon at
    azimuth 240°, ahead and a little left from the blue spawn. It has panel bands with thin seams, a dark trench round
    its equator with 14 small pale lights, a concave dish in its upper half (a bowl cut out of the sphere, with two
    rings and a dark lens), a few lit windows on its night side and a red beacon flashing on its pole. Its pole leans
    17° toward the eye so the upper half shows. The sun is 90° away from it there, so it shows half lit. You picked the
    dish in the upper half (option B) over one on the equator.
  - **Hovering:** the station holds its place (`Path::Hovering`), swaying 0.5° along its great circle and back every
    15 minutes, so it never sets and moves about a pixel every 15 s at 2560×1600.
  - **Capital ship (`draw_capital_ship`):** a long flat wedge 7° long, with one deck block and a bridge tower at the
    stern, three blue engines with plumes, red and green lights at the stern's corners and a white strobe on the
    bridge. Rows of lit windows run along the superstructure's sides and the hull's two edges; once a window is
    smaller than a pixel they show their mean light, so they don't flicker as the ship drifts. It crosses the sky like
    D82's ships, highest at 30° (azimuth 290°, ahead and a little right from the blue spawn), in 30 minutes, then
    comes back below the horizon in 6. You picked the plain hull (option A) over one with stepped tiers and panel
    stripes, with lights like that one's.
  - **Solids, not silhouettes:** D82's ships are flat silhouettes; the giants are shaded as solids, along parallel
    rays since they are far: the station as a sphere, the capital ship as a convex wedge (five planes) and three boxes.
    The capital ship is drawn tilted 52° from facing the eye, so its top and superstructure show, standing up toward
    the top of the sky. Both are lit by the sun and, more faintly, by the sunlit ground, with a faint glow so their
    night sides show against the stars.
  - **Background only:** as in D82, they are part of the sky: behind everything, bent through the lenses like the
    stars, on the UTC clock, and in the `lunar` appearance style rather than the map.
  - **Cost:** as in D82, each sky pixel rules a giant out with a few dot products, and only pixels within 1.5 half
    lengths of one shade it (about 19,000 for the station and 3,000 for the capital ship at 2560×1600, 0.5% of the
    frame). On D10's shared machine, with other agents' work running, three alternating `--bench-views` runs at
    2560×1600 against `staging`: by the best of the three, the fastest frames moved by −0.5 to +0.8 ms per view
    (mean +0.2 ms), and the median GPU times by −0.2 to +0.5 ms, apart from two noisy views (+1.5 ms at the red spawn
    and −14 ms at the wormhole's exit). The blue spawn, with both giants in sight, took 28.0 ms on the GPU against
    27.5 ms. The window at 1280×800 ran 85.8 fps (10.0 ms on the GPU).
- **Alternatives:**
  - flat silhouettes like D82's ships (a sphere and a superstructure read as flat discs and blocks);
  - the station crossing the sky like the others (it would set for a sixth of the time; a moon-sized station reads
    better holding still);
  - the station further from the sun, more fully lit (around azimuth 300° it would crowd the planet in the spawns'
    view);
  - the options not picked: the dish on the equator, and a detailed hull with three stepped tiers, panel stripes and
    seven engines.
- **Rationale:** you asked for a moon-sized battle station with a dish and a long wedge-shaped capital ship off in the
  distance, as original designs in that spirit; seeing two options of each, you said "I like Death Star option B and
  I like ship option A but you can add a bunch of lights on it like B".

### D104. A third-person view on `V`, W-61 (third-person view)

- **Choice:**
  - **Key:** `switch_view` in `player_settings.yaml`, `V` by default, switches between first and third person, on
    voxel maps (on the house it does nothing).
  - **Camera:** an L-shaped boom (`third_person.rs`): up 0.6 m along the vertical from the eye to a pivot above the
    head (the body's head reaches 0.4 m above the eye), then 3 m back along the pivot's back axis, which tilts with
    the view, so the camera swings around the pivot as you look up and down. It looks the way the eye does, carried
    along the geodesics, so the crosshair passes just over your head and your body fills the lower middle.
  - **Walls:** each leg is traced straight (glass stops it, though it doesn't stop lasers) and stops 0.25 m short of
    the ground or a block. Where the pivot can't rise 0.45 m (the 2 m walkways and module doorways), the camera stays
    at the pivot, inside the head: your body isn't drawn and the gun in hand shows, as in first person.
  - **Switch:** the camera moves out along the boom (up first, then back) over 0.3 s, eased in and out, and back the
    same way.
  - **Your body:** the figure the others see you as (`multiplayer::get_body`, from the frame your window sends the
    server, in your team's colour), drawn by the voxel renderer, with its shadow. Your own laser passes through it;
    the other players' lasers stop at it, as they do in their windows. The gun in hand is hidden while it shows.
  - **Aiming (right button):** brings the camera back to the eye while held, with the usual zoom, so aiming looks
    from the eye and is exact.
  - **Shots:** still leave the eye, as the other windows trace each shot from the frame sent (the server takes each
    window's word for its hits). The eye turns toward the point the crosshair's straight ray meets
    (`VoxelWorld::find_crosshair_point`: the ground or a block, through glass, ignoring the warps' bending), and
    that turned frame, with the shot's noise, is what the window traces and sends, so every window draws and scores
    the same beam, from your head to the crosshair. A point more than 0.6 rad off your view (behind you) doesn't
    turn it. Holding `E` on a console uses the same turned eye.
  - **Saved settings:** a `player_settings.yaml` saved before this, without `switch_view`, still loads and gets the
    built-in `V`: `ADDED_PLAYER_KEYS` in `settings.rs` names the keys added since saves began, as
    `REMOVED_PLAYER_KEYS` (#259) names those removed. Any other missing key still fails.
- **Alternatives:**
  - one straight boom back and up from the eye (pulled in toward the eye, the head covers the crosshair once the
    camera is within about 2 m);
  - an over-the-shoulder camera (you asked for straight behind);
  - zooming in third person while aiming (the head grows over the view);
  - easing the camera back out after a wall (it comes in and out at once, as the wall allows);
  - tracing a shot through a wormhole several times until it lands on the crosshair (see the limitation).
- **Limitation:** through a wormhole's mouth, rays 0.6 m apart come out far apart, so a third-person shot through one
  lands only roughly where the crosshair points (19 m off in a test); aim to look from the eye. Elsewhere, under the
  black hole's reach too, it lands within 5 cm (the unturned eye's shot lands 2.7 m off at 20 m).
- **Rationale:** you asked for a way to switch to a third-person view, the camera slightly behind the character,
  controlled from there, seeing your own character.

### D15. WebGL at the same drawing-buffer sizes

- **Choice:** the browser demo from PR 58, unchanged, in the Claude desktop app's built-in browser (Chromium 152,
  ANGLE on Metal), with its drawing buffer set to 1280×800 and 2560×1600 and frames timed with `readPixels`. Its
  scene differs a little: its mouths and eye are 4.5 blocks up, and it has no laser.
- **Alternatives:** moving the WebGL demo's mouths to match first.
- **Rationale:** it is the version you found too slow.

### D105. The wormhole colour shift in the colours you picked, for W-82 (wormhole colour from motion)

- **Choice:** D84 keeps where, when and how long the shift shows (inside the mouth, × the speed along the view, no
  timer, 0.63 s walking through mouth A); its colours are now the two you picked from sheets made by an exact Python
  copy of the shader. The real shader matches that copy to within 1 level in 255 on the `middle` and `red_pylon`
  views.
  - **The Doppler step:** each channel shows the light at D times its wavelength (the spectrum at 610, 550 and 465
    nm, 0 at 400 and 700 nm), with D = √(1 − β²) / (1 − β cos θ′), cos θ′ = 1 − (1 − cos θ) / 2, θ the angle off
    the middle of the screen. β is 0.09 forward and −0.3 backward at walking speed, growing with it beyond.
  - **Forward, option 8 "toward light cyan":** D² as bright plus a blue glow (0, 0.12, 0.4) × (D² − 1); paled 30%
    toward its grey (luminance 0.2126 R + 0.7152 G + 0.0722 B); moved 60% toward light cyan #E0FFFF at the same
    luminance, so dark stays dark; × 1.5 through a shoulder that rolls the brightest channel off above 0.8 toward
    1, so nothing clips to flat white; then green moved into blue and held to 0.95 of blue, so nothing turns green.
    On the `middle` view the ground turns from #A8A295 to #B3E3EF.
  - **Backward, R2 "darker":** D as bright plus the red glow (0.4, 0.04, 0) × |D − 1|, then × 0.7. On the `middle`
    view the ground turns #540F00.
  - **Below walking strength:** the picked colour fades in, the gain and the green cap with it.
  - **Knobs:** named constants at the top of `render_metal.metal` (`BLUESHIFT_PALE_FRAC`, `BLUESHIFT_WHITE_FRAC`,
    `BLUESHIFT_TINT`, `BLUESHIFT_GAIN`, `BLUESHIFT_SHOULDER_KNEE`, `BLUESHIFT_LIFT_FRAC` at 0,
    `BLUESHIFT_MAX_GREEN_PER_BLUE`, `REDSHIFT_GAIN`). `view.wormhole_tint_frac` still fades the whole shift in.
- **Alternatives:**
  - the other forward options on the sheet: 1 to 7 (bluer or greener, less white), 9 to 12 (nearly white, #E0FFFF
    to #F0FFFF, keeping less of the scene's texture), and lifting the blacks toward the tint (options 7 and 12);
  - the redshift as it was (#259 and #289), or another darkness (R1, R3, R4);
  - #289's forward look (β 0.16, D³, shown at 45%), chosen from side-by-side renders before your picks reached it.
- **Rationale:** from the sheets you said "i like option 8 for the wormhole" and "for red i like r2"; after #272 you
  asked for "the dark red and the light cyan almost white", crisp, "like the previous pictures I was shown".

### D106. The rifle is a bolt-action rifle in hand, W-96 (a different rifle model)

- **Choice:**
  - **Model:** gun 2, the rifle, is drawn as Poly Haven's "Bolt Action Rifle 7.62" by Mateusz Sadek (CC0), already
    committed for the fps_demo in `demos/fps_demo/assets/models/bolt_action_rifle_7_62/`; nothing was downloaded. Gun
    1, the sniper, keeps the energy revolver. `Gun::get_model` in `main.rs` picks the model, and `viewmodel.rs`
    loads each model (`GunModel`) once, both before the first frame (two loading-screen steps, "revolver" and
    "rifle").
  - **Size:** `BOLT_ACTION_RIFLE_SCALE` = 0.28, so the 1.23 m rifle is drawn 0.34 m long. With its muzzle at the
    wand's tip, at full size (or 1.6x like the revolver) only its barrel shows; at 0.28x its scope, bolt, magazine,
    trigger and the start of its stock come into the lower right corner. 0.25x shows more of the stock but makes the
    rifle look thin and far; 0.35x and up push the stock and then the scope off the bottom of the view.
  - **Placement:** the model is turned a quarter turn about its up axis (its barrel lies along +x in its glTF), and
    its muzzle (the middle of its front face) is put at the wand's tip, as the revolver's is, so the beam still
    leaves the muzzle. The pose, bob and recoil are the wand's, unchanged.
  - **Missing file:** the game checks both models at start and fails naming the file and the command that restores
    the fps_demo's assets, as it did for the revolver.
  - **Credit:** the CREDITS page names the rifle, its author, Poly Haven and its link.
- **Alternatives:**
  - a sci-fi rifle from Sketchfab under CC BY (AR-H470 by Frostoise, Sci-FI Sniper Rifle by Matija Švaco and
    others): a closer match to the revolver's look, but it needs a download with a Sketchfab account;
  - the Kenney Blaster Kit (CC0): toy-like low-poly blasters that clash with the textured revolver;
  - drawing the rifle at the revolver's 1.6x or at full size (only the barrel shows), or moving the gun so a
    full-size rifle fits (the beam's start would move).
- **Rationale:** you asked for the rifle to be a different gun ("for the rifle the gun has to change to a different
  one ideally; see if you can find something for that"), and chose the bolt-action from the pictures ("bolt action
  is good"). The wooden rifle stands out from the sci-fi look, which also makes the two guns easy to tell apart.

### D108. Time slows near the black hole instead of the map bending, W-107 (time dilation)

- **Choice:**
  - **No more bend:** W-68 (reality bend)'s curling of the map into a sphere is gone: `bend.rs`, the laser's trace
    through the bent world, and the shader's `HAS_BEND` tracer. `reality_bend.rs` became `black_hole_growth.rs`,
    the `reality_bend` server settings became `black_hole` (without `edge_radius_m`), and `--bend-strength` became
    `--black-hole-strength`.
  - **The black part grows a little, the distortion a lot:** each console turned off grows the black hole's
    Schwarzschild radius from 1.2 m to only 2 m, its reach from 24 m to 28 m, and a wide lens around it from none to
    one that turns a ray by up to 0.35 rad (`max_lens_rad`). The lens is ln n = h a / √(ρ² + a²), with a core a of
    0.35 of the reach (`lens_core_per_reach`): it bends light across the whole reach, most for rays passing a from the
    centre, and has no horizon, so it adds no black. The CPU (the laser) and the shader use the same index. The black
    hole stays where the map puts it, 17 m above the map's middle; it used to rise toward the bent sphere's centre.
  - **How slow:** `VoxelWorld::get_time_rate_frac` is general relativity's clock rate for someone standing still
    near a black hole, (1 − q) / (1 + q) in isotropic coordinates with q = k m / ρ, with k =
    `time_dilation_per_general_relativity` = 15. The wide lens slows time too, as a weak field does: the rate's log
    falls by k times half its ln n. The rate's log fades to 0 at the reach, along the window the light bending uses,
    and never drops below `min_time_rate_frac` = 0.1, so a player near the horizon crawls rather than freezes. On the
    ground under the black hole (15.4 m from its centre) time runs at 81% with no console off, 45% with three off and
    25% with five.
  - **Whose time:** each player's own. Their window steps their movement (walking, gravity, jumping, sprint
    energy), turning off a console, both guns' firing, reload and heat, and the gun's sway by their own time.
    Looking around, the menus, the HUD and respawning keep the window's time. Bots step the same way. Other players
    see a slowed player move slowly, since each window sends where its player is.
  - **Shown:** the HUD's line `BLACK HOLE <n>%, your time runs at <n>%` replaces `REALITY <n>%`.
  - **Reach at most 28 m:** mouth A is 46 m from the black hole with a 15 m reach, and reaches may not overlap; the
    nearest structure (29.6 m) stays outside, since the black hole's list of structures is built at the map's reach.
- **Alternatives:**
  - growing the Schwarzschild radius to 6 m (the first version): the black part filled the view; you asked for "the
    black part grows a little but mostly the distortion area grows";
  - k = 3 (the first version): 65% under the black hole at full strength and 96% with no console off, which you
    didn't notice; k = 1, real general relativity, is weaker still;
  - no fade at the reach: everyone on the map would run a little slow, all the time;
  - slowing the whole game: impossible with several players, who are at different distances;
  - slowing looking around too: the mouse would feel broken rather than time slowed;
  - a colour cue (blueshift toward the black hole): left for later.
- **Rationale:** you asked for the black hole to grow as pylons fall and for time to slow "time dilation style"
  closer to it, then for "more intense" dilation "all around". The rate keeps general relativity's shape, so it
  slows ever faster toward the horizon, and k makes it strong enough to feel at the distances players stand. All
  the numbers are server settings, to tune in a playtest.

### D102. A photo-like Milky Way across the sky, W-98 (Milky Way)

- **Choice:**
  - **Look:** option C, photo-like, from the mock-ups, drawn in code with no downloaded photo: a band brighter and
    wider toward its core with a warm bulge there, mottled by star clouds and grain, split by a dark rift along its
    middle and dimmed by dust clouds, with faint pink nebulae and the stars thicker inside it.
  - **Placement:** the band arches across the sky from base to base, so both teams see it ahead of them, with its
    core about 30° along it from the horizon. Its pole and core are map data, `sky.milky_way` (`pole_direction`,
    `core_direction`) in `common/maps/`, so moving it is one change to `export_map.mjs`; the house has none.
  - **Brightness:** each player's `view.milky_way_brightness_frac` in `player_settings.yaml` (0 to 1, 1 by default, 0
    for none), also a row of the settings menu. An older settings file without the key gets 1.
  - **Cost:** `render_voxel.metal`'s `bake_milky_way` draws the band once into a 4096 × 1024 HDR texture (longitude
    by latitude in the band's own frame, up to 45° from its middle) before the first frame; each frame only reads it
    where a ray reaches the sky. Alternating runs against `staging` (4 rounds, 15 frames, four views at 2560 × 1600)
    showed no cost: the least GPU time per view was 1.5% to 4.6% lower with the band, within the noise.
- **Alternatives:**
  - the looks A, subtle, and B, rich, from the same mock-ups;
  - a real photo (ESO's Milky Way panorama, CC BY 4.0), which needs a download and a credit;
  - the band low across the horizon (the mock-ups' first position) rather than arching over from base to base;
  - drawing the band anew every frame (the mock-ups did; it cost GPU time at every pixel of sky).
- **Rationale:** you asked for a Milky Way in the skybox ("add to wishlist having a milky way", "in teh skybox") and
  picked option C from the pictures ("get milky way photo like merged"). You didn't pick a placement, so the arch
  from base to base was chosen for you, since it gives both teams the same view; tell us if you want it elsewhere.

### D107. Both guns sway left and right with your steps, by their size, W-102 (rifle sways when jogging)

- **Choice:**
  - **Slide:** the hands slide the gun in hand left on one step and right on the next (`get_sway` in `main.rs`): 2 cm
    each way while walking, growing to 4 cm while sprinting (with the run pose's easing), the same for both guns.
  - **Swing by length:** a longer, heavier gun lags behind the hands, so its muzzle turns the other way: 0.03 rad
    (1.7°) walking and 0.06 rad (3.4°) sprinting for a gun as long as the bolt-action rifle (1.23 m in its glTF,
    the sniper since #318), scaled by each model's real length (`GunModel::get_modelled_length_m`), so the 0.30 m
    revolver (the rifle, gun 2) swings a quarter as much and mostly slides.
  - **Kept:** today's dip at each step and the sprint pose's tilt stay as they were.
  - **In step with the eye:** on a voxel map the gun takes the head bob's step phase and fade (`fade_s`), so it moves
    with each footstep the camera dips on and eases in and out; the house keeps the gun's own step clock, at the same
    rates.
  - **Settings:** `view.head_bob.gun_walk_sway_m`, `gun_sprint_sway_m`, `rifle_walk_swing_rad` and
    `rifle_sprint_swing_rad` in `player_settings.yaml` (0 for none). Older saved settings files without them load
    with the built-in values (`ADDED_PLAYER_KEYS`).
  - **Shooting:** a shot leaves from the fixed muzzle point (`voxel_world::MUZZLE_M`), so while moving crouched the
    drawn gun can be up to 2 cm beside it; standing still there is no offset, and you can't shoot walking upright or
    sprinting.
- **Alternatives:**
  - no change: the sprint pose already tilted the gun back and forth with each stride;
  - the slide on the rifle (gun 2) only, as first built; a stronger sway (5 cm walking, 8 cm sprinting);
  - the same swing for both guns, whatever their size.
- **Rationale:** you asked for the rifle "going side to side when jogging like a normal rifle", chose the slide as
  built from the first pictures ("ok menrge to staging", "i like your animation"), then, after #318 swapped the
  models, asked for both guns: "I want them both swaying ... get both working realistically based on their size",
  and approved the result ("merge this").

### D109. Shoot once no movement key is held, in the air too, W-108 (shoot whenever not moving)

- **Choice:**
  - **Rule:** a player can shoot once no movement key has been held for 0.25 s (`movement.shoot_after_moving_s` in
    `server_settings.yaml`, 0 for none). The movement keys are `W`/`A`/`S`/`D` while upright, `Shift` with them, and
    `Space`. Letting go starts the cooldown; pressing one again stops a shot in flight, as before.
  - **In the air:** falling or floating with no key held may shoot, which replaces D50 (no shooting in the air).
  - **Kept:** walking crouched still shoots (D67), and nothing else about shooting changed (consoles, the lobby, a
    won level).
  - **The house:** holding `Space` for its jetpack now stops shooting too, since `Space` is a movement key.
  - **Time dilation:** the cooldown counts the player's own time, as the reload does (D108), so near the black hole
    it lasts longer in real time, in step with everything else the player does.
- **Alternatives:** no cooldown; a longer one (0.5 s); counting crouched walking as moving; a constant in code
  instead of a server setting (which would let windows from before this change still join).
- **Rationale:** you asked: "with a slight cooldown, you should be able to shoot as long as no keys are pressed. No
  movement keys I mean, so if you're in the air while time dilated you should be able to shoot."

### D110. Lunar Surface at full resolution: baked ground, culled structures and shared sunlight, W-131 (Lunar wormhole_exit slow at full size)

- **Choice:** make the voxel renderer (`render_voxel.rs`, `render_voxel.metal`, `terrain.rs`, `voxel_world.rs`, the
  same in `demos/warped_match_rust/` and, for the shader, `demos/dust_rust/`) faster with changes that keep the
  pictures, profiled by switching parts of the shader off one at a time (shadows, shading, structures, terrain, beams):
  - **Baked ground** (Dust's #420): the whole terrain height, 5 octaves, in a 0.25 m grid over the boundary's box
    grown by 250 m (`BAKED_TERRAIN_REACH_M`), which walking, laser hits and the shader read alike; its slopes are
    baked on the GPU at the same points (`bake_terrain_slopes`) and read bilinearly for the ground's normal, instead
    of four 5-octave heights per pixel.
  - **Structure groups:** static structures grouped by 12 m cells of Fermi (u, v), each with a bounding sphere, so a
    ray or shadow ray tests a group's members only if it can reach the group (`trace_structures`).
  - **Shapes a chord can reach:** a structure's sphere tracing evaluates only the shapes whose box (grown by the
    hit tolerance, and by the penumbra's reach for shadow rays) the chord crosses (`get_shape_mask`).
  - **Mesh BVHs nearer child first,** and a shadow ray stops at its first triangle.
  - **Beams:** each beam's midpoint is packed once; in bent steps only the beams that come near the warp are
    scanned (`get_beam_mask`); far lamps are skipped before their distance is measured.
  - **Shared sunlight (the one heuristic):** each fine sample (every second pixel each way) on the ground traces its
    sun shadow once; a pixel between four samples that are all fully in the sun, with no structure near their shadow
    rays, takes full sun instead of tracing its own. Everything else (penumbra, shadow edges, structures, near misses)
    is traced per pixel as before.
- **Look:** before against after, 2560×1440, every bench view: see #450 for the table and the difference
  pictures; the changes are sub-millimetre shifts of ground hits (the bake's bilinear heights, at most a few
  millimetres off the 5-octave shape), which move the regolith's fine texture by a pixel here and there, and
  sunlight 0.0003% brighter beside structures (`NEAR_STRUCTURE_LIT`).
- **Gives up:** a shadow narrower than two pixels between four fully lit samples whose shadow rays pass no structure
  is lost; no such shadow was found in the bench views (thin shadows are cast by structures, whose rays are near).
- **Alternatives tried and not taken:** longer bent steps (Dust's 0.16 and 0.48 of the distance to the centre, or steps
  that grow beyond 4 throat radii): `wormhole_exit` 49 → 36 or 41 ms, but the lens's rings move by a few pixels, so
  they wait for the user; interpolating the sun between samples 4 pixels apart broke thin shadows into dashes; a 2 cm
  limit on sharing kept them but lost most of the gain.
- **Rationale:** the user wants 60 frames a second at full resolution in every level, and pictures that look the
  same. `wormhole_exit` (4 m from a mouth, the lens filling the view) stays far over 16.7 ms: its rays take a hundred
  or more bent steps each; in play the window's automatic render scale keeps it fast there.

### D111. Safe terrain steps, the beam's haze between samples, lamp cutoffs once, W-131 (Lunar wormhole_exit slow at full size)

- **Choice:** three more changes to `render_voxel.*` (Lunar Surface, the match's copy and Dust):
  - **Safe terrain steps:** a camera ray's terrain march also steps `TERRAIN_SAFE_STEP_FRAC` (0.8) of its gap over
    its descent plus the steepest slope near its 4 m block (`make_slope_bound_grid`, read by `get_safe_step_m`), when
    that is longer than the old step; a Fermi coordinate moves no faster than the ray, so the step can't pass under
    the ground. Shadow rays keep their steps, so the penumbra is unchanged.
  - **Beam haze:** adaptive sampling interpolates pixels whose four samples see the same beam's haze, within 0.02,
    with no core (`HALO_SMOOTH_DEPTH`), instead of tracing each.
  - **Lamp cutoffs:** each lamp's cutoff cosh is packed once per frame.
- **Look:** ground hits move within the bisection's tolerance, which moves the regolith's fine texture by a pixel here
  and there (`red_spawn`: 3% of pixels past 8 of 255, as with the bake); elsewhere under 0.05%.
- **Alternatives tried and not taken:** a grid of the highest ground near each block (steps over clear air only):
  slower; growing terrain steps with the whole path, as Dust does: no gain on Lunar Surface.
- **Rationale:** 2 to 6% off every view but `wormhole_exit`; `laser_into_wormhole` 17.5 → 16.6 ms at 2560×1440
  (p99 17.0). The 3440×1440 views over 16.7 ms (16.9 to 21 ms) and `wormhole_exit` need more than this.

### D112. Exact ground hits far from the origin, shadow safe steps, blocks of groups, W-131 (Lunar wormhole_exit slow at full size)

- **Choice:** five more changes to `render_voxel.*` (Lunar Surface, the match's copy and Dust):
  - **Fermi u from a logarithm:** `get_fermi` took u from atanh(z / w), which near the spawns (u ≈ 4.2 R, z / w ≈
    0.9995) turns a float's rounding into several millimetres; ln((w + |z|) / √(1 + x² + y²)) is the same on the
    hyperboloid and good to micrometres. Camera rays also end their bisection with a secant step.
  - **Smoothness from the eye:** a float world point there is itself only good to about a millimetre, more than the
    step between neighbouring pixels' hits on the near ground, so `is_cell_smooth` found the near ground and walls
    rough and traced them pixel by pixel (a third of `red_spawn`'s time). A ray that went straight from the eye
    (`STRAIGHT_RAY_BIT`) is now measured by its direction from the eye times its distance (`get_eye_point`), which is
    exact; cells with a bent ray keep the world points.
  - **Shadow safe steps:** a shadow ray's terrain march takes the camera ray's safe step, shortened so that its gap
    can't fall below its least penumbra ratio so far (`hit.lit`) on the way: the penumbra is unchanged.
  - **Shared full shadow:** a ground pixel between four fine samples whose sun rays all hit something takes their
    shadow, as one between four fully lit samples already took their light (D110).
  - **Blocks of groups:** the static structure groups (12 m cells) are packed by 4 × 4 blocks with a bounding sphere
    each (`pack_structure_groups`), so a ray, above all a shadow ray, tests a block before its groups.
- **Look:** the same, but where the ground hits moved: they are now on the ground rather than up to a bisection's
  width under it, which moves the regolith's fine texture by a pixel here and there (`red_spawn`: 3.7% of pixels past
  8 of 255, mean 1.1; `red_hangar` 0.9%; elsewhere under 0.7%). The shared shadow changed at most 0.003% of a view's
  pixels; the blocks and the shadow steps none.
- **Gives up:** a lit gap narrower than two pixels between four fully shadowed samples is lost; none was found in the
  bench views.
- **Not taken, for the user** (`wormhole_exit`, the lens filling the view; options sheet in the PR): a half-resolution
  lens (cells whose four corners took 8 or more bent steps blend their corners' colours instead of tracing): 50 → 25.5
  ms at 2560×1440, edges a little stepped; Dust's longer bent steps: 36.7 ms, rings move a few pixels; both: 21.2 ms;
  the half-resolution lens tracing edges between surfaces: 34.4 ms; both, with the long late steps after 40 steps
  instead of 90: 20.1 ms. None reaches 16.7 ms at 3440×1440 (24.6 ms at best).
- **Rationale:** the user wants 60 frames a second at full size on the 3440×1440 display. At 3440×1440, 19 of 21 views
  are now under 16.7 ms (before: 14); `wormhole_exit` and `laser_into_wormhole` (the lens) are left.

### D113. A crouched player's robot stands on the ground, squashed, W-137 (crouched robots sink)

- **Choice:** each window sends how far down a crouch its player is (0 to 1 in 15 steps) in the 4 spare high bits of
  the wormholes byte of its state, which the server relays in its snapshots. Other windows raise the eye they receive
  by the crouch's drop before placing the robot (`VoxelWorld::raise_crouched_eye`), so its feet stay on the ground,
  and squash the drawn body (`AvatarModel::make_squashed`), its hit shapes and its head line toward the feet to the
  crouched eye height's share of standing (0.6 crouched). The same in Dust, Snow Planet, the match and City Sphere
  (City squashes its robots and lowers its torso and head spheres alike).
- **Gives up:** a squashed robot is a shorter robot, not a crouching one: the body model has no crouch clip, and
  bending its skeleton by hand was left out. Adding a clip would need a new animation asset, which needs the user's
  approval.
- **Compatibility:** the snapshot's length is unchanged, so windows and servers from before this change still play
  with it: an older window ignores the new bits (and draws a crouched player sunk, as before), and an older server
  relays nothing in them. A standing player's robot is drawn exactly as before.
- **Rationale:** a crouched player's robot sank about 0.64 m into the ground (a standing eye height below its
  lowered eye), and its hit shapes with it. The lead asked for this heuristic, for the user, who wants cheap
  heuristics that look right.

### D114. Lunar's lens at half resolution with Dust's longer bent steps (option D), W-131 (Lunar wormhole_exit slow at full size)

- **Choice:** the user's option D from D112 (2026-11-09), in `render_voxel.metal` (Lunar Surface and the match's
  copy):
  - **Half-resolution lens:** `trace_ray` counts its bent steps (`BENT_STEPS_FIRST_BIT` in `surface.w`, up to 255).
    `refine_pixels` blends the colours of a cell's four even pixels into its three odd ones, instead of tracing them,
    when all four samples took at least `LENS_HALF_RES_STEPS` (8) bent steps: deep in a lens, where neighbouring
    rays land far apart anyway.
  - **Dust's longer bent steps:** `BEND_STEP_FRAC` 0.08 → 0.16 of the distance to the warp's centre, and
    `LATE_BEND_STEP_FRAC` 0.24 → 0.48. `voxel_world.rs`'s `BEND_STEP_FRAC` (the CPU's laser paths, the clearance caps)
    changes with it, so lasers still hit where they are drawn.
- **Measured** (median GPU ms, best of 3 alternating rounds under the exclusive GPU lock, 2560×1440 / 3440×1440):
  `wormhole_exit` 48.8 / 57.7 → 20.2 / 24.5; `laser_into_wormhole` 16.1 / 18.6 → 10.8 / 13.1; `wormhole` 13.1 / 15.6
  → 9.9 / 12.2; views with a warp object in sight up to 3.3 ms faster, the rest within noise. At 3440×1440, 20 of 21
  views are under 16.7 ms; `wormhole_exit` (24.5 ms) is left.
- **Look:** a lens's most compressed rings are a little stepped and move a few pixels; nothing outside a warp's reach
  changes (the pictures are identical there; the views without a warp object in sight are identical).
- **Gives up:** detail finer than two pixels deep in a lens.
- **Rationale:** the user chose it from D112's options sheet.

### D115. A sharp centre with softer edges, W-136 (sharp centre, soft edges)

- **Choice:** the window and the bench views draw a circle around the image's centre, `sharp_width_frac` of the
  image's width across, exactly as before (adaptive sampling), and the rest at half resolution: there only the even
  pixels (every second each way) are shaded and the others are blended from them, with no refinement. Beyond a second
  circle, `half_width_frac` of the width across, only every fourth pixel each way is shaded (a quarter resolution).
  The default (`DEFAULT_SHARP_CENTRE`) is the middle 66% sharp with the rest at half (option B); `--sharp-width-frac`
  and `--half-width-frac` choose others (`inf` for none), and every other picture (the map's, tests) stays all sharp.
  The same code is in Dust's and Snow Planet's renderers.
- **How:** `get_fovea_level` gives each 4 × 4 pixel block its level from its centre's distance to the image's centre.
  `fill_samples` skips the fine samples no half or sharp block needs (`is_sample_needed`, 8 pixels of margin);
  `classify_cells` marks no cell outside the centre smooth and lists them all with their level; `resolve_pixels`
  shades only the even pixels whose sample is needed and every fourth pixel; `refine_pixels` blends the rest
  bilinearly from pixels `resolve_pixels` wrote, so no thread reads a pixel another writes in the same pass.
- **Options measured** (Lunar Surface, 3440×1440, median GPU ms, best of 3 alternating rounds under the exclusive
  lock; red_hangar / blue_hesco_rows / beacon / wormhole_exit): A all sharp 16.0 / 15.7 / 15.6 / 24.9; B middle 66%
  sharp, rest half 15.5 / 13.9 / 14.8 / 24.6; C middle 50%, rest half 15.3 / 13.2 / 14.1 / 24.5; D middle 66%, half
  to 100%, corners quarter 15.4 / 13.7 / 14.8 / 24.5; E middle 50%, half to 75%, rest quarter 14.8 / 12.7 / 13.6 /
  23.4.
- **Why the savings are small:** about 5.5 ms of a 3440×1440 frame doesn't depend on how many pixels are shaded
  (filling and classifying the sample grid, bloom, tone mapping, the passes over every pixel), and the coarse pass
  (every fourth pixel, always traced) costs 3-10 ms; even the whole image at a quarter takes 8.9 ms on red_hangar and
  15.9 ms on wormhole_exit, whose lens fills the middle.
- **Look:** the centre is the same picture (at most 1-2 of 255, through the bloom); outside it, edges between
  surfaces are stepped at 2 pixels (half) or 4 pixels (quarter) and fine texture is softer.
- **Gives up:** detail at the image's edges; nothing in the middle.
- **Rationale:** the user (2026-11-09): "Try to at least have the ability to see a nice, good resolution for the
  important things that are right in front of you." B keeps the centre identical and gives each heavy view a margin
  under 16.7 ms; E is there for slower machines. The user picks.

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
