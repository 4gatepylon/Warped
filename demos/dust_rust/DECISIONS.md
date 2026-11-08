# Dust decisions

Decisions made instead of the user (W-121, Dust demo). **User?** marks ones to weigh in on.

1. **Wind as ln n = k·d (a planar gradient).** The simplest refractive field whose light bends one way
   everywhere; k = 0.00144 /m (0.0012 until the user asked for "ever so slightly more wind") bends a ray ~7 cm over 10 m and ~7 m over 100 m. **User?** strength.
2. **Fog replaces the haze** (start 4 m, e-folding 60 m, max 94%, sand colour) and ends rays at 304 m, which
   also bounds the cost per pixel. **User?** thickness.
3. **Blowing sand in the shader, not particles**: gusts in the fog and two shells of streaks near the camera,
   drifting downwind at 9 m/s, about 1 ms a frame; particles would need a new draw pass. **User?** how strong.
4. **Curvature factor 1.0** in `server_settings.yaml` (Dust's own R = 50 m from the WebGL prior art), not
   Lunar's 1.5.
5. **Layout**: mirrored halves, spawns 208 m apart, one fixed wormhole pair across the middle, mesas and ruins
   for cover on long sight lines. **User?** size.
6. **Bench views replaced** with Dust's (the Lunar list did not fit the map).
7. **Wind chords of 48 m**, not 12 m: a 48 m chord strays from the curved ray by k·48²/8 = 0.35 m, invisible at
   that range, and saves about 5 ms a frame.
8. **Package and binary names kept** (`hyperbolic-wormholes`, `wormholes_server`) so the copy stays a clean diff.
9. **Laser colour [1, 0.8, 0.3]** (display RGB) for every team, from the map file: the user asked for "slightly more
   yellow", golden leaning to white; [1, 0.7, 0.25] before. **User?** shade.
10. **Map picture straight down, not oblique**, so marks drawn at map coordinates land on what they mark; height
    60 m, 3×3 supersampling, fog and wind off, sun fixed in the Fermi frame. Committed (0.8 MB) rather than
    rendered at each start, which would need the GPU before the window opens. **User?** look.
11. **Heavier storm strengths** (`atmosphere.sand`): gusts 50%, streaks 45%, sheets 35%, fog e-folding 50 m
    (was 60 m), warmer fog [0.74, 0.55, 0.35]. Visibility stays about 250 m. **User?** strength. The wind's
    strength does not yet gust in time and space (requested; needs a shared seed and server time in the protocol).
12. **Prop jitter** up to 1.5 m, 0.3 rad and ±12% size, seed 121, mirrored across the middle so both teams get the
    same cover. Pylons, consoles, spawns and the wormhole stay put. **User?** amount.
13. **Ground-hit sand** tinted from the fog colour (grains 80%, puffs 120%), puffs drifting downwind at 3 m/s. In
    the storm the puffs read darker than the lit sand around them. **User?** brighter.
14. **Gusts in strength only**, from the map's seed and the shared UTC clock: ±20% (`strength_frac` 0.2) in
    periods of 5–11 s, with wavelengths of thousands of meters so it is "roughly similar everywhere" (the user's
    words). **User?** amount and speed.
15. **Laser multiplier 2.4** (`laser_gradient_per_view_gradient`): the user asked for a bend you "can tell",
    3–6 m over 50–80 m. Against the bent view, a laser across the wind lands 2.9 m downwind at 50 m and 8.4 m at
    80 m; 2.0 would give about 2 m and 6 m. A test keeps the 50 m bend between 2 and 6 m. The crosshair no
    longer shows where a long shot lands across the wind: players aim upwind. **User?** strength.
16. **Storm sky in the shader**, only where a ray escapes (no texture, no geometry): haze bands, a brown storm
    wall and mesa silhouettes. Alternating runs with it off and on differ by 0–1.3 ms a frame. **User?** look.
17. **Speed by cutting quality** (the user: "get something working as soon as possible and performantly"): the
    Metal renderer starts at render scale 0.6 (`METAL_RENDER_SCALE`, also the `--bench-views` default; the window
    still auto-scales), terrain relief uses 3 noise octaves, not 5 (`OCTAVE_COUNT` in `terrain.rs` and
    `TERRAIN_OCTAVES` in the shader must match, or server hits and pictures part), and rays end 2 fog depths after
    a wormhole crossing, not 3. 2560x1440 medians, 20 frames: overview 22.8 → 11.6 ms (p99 23.2 → 13.8), wormhole
    28.9 → 17.8 (29.4 → 20.0), wormhole_exit 30.0 → 17.6 (30.5 → 20.2), laser_into_wormhole 39.1 → 19.3
    (39.8 → 20.0); the other 20 views are 8.5–15.8 ms. Still over 16.7 ms: the two wormhole views and the laser
    into the wormhole. This VM's runs vary by up to ±20%.
    - **For a future agent:** the costs measured are terrain height (noise octaves, in the primary march, the
      shadow march and normals) and per-pixel shading; wind chords, the sand, the storm sky and fog length cost
      little. The next step would be baking the whole terrain height into the existing 0.25 m `MoundGrid` buffer
      (CPU and GPU read it the same way), then lower `--render-scale` only for the wormhole views. Item 18 did the
      bake, and the render-scale cut is no longer needed.
18. **The whole terrain height baked into a 0.25 m grid** (handoff known gap 1, Dust speed): `Terrain::bake_heights`
    samples the terrain's shape (crater wall, rim noise, the 3 relief octaves and the mounds) every 0.25 m wherever a
    ray from inside the boundary can reach before the fog ends it: the boundary's box grown by the fog's end
    (4 m + 5 × 50 m = 254 m), so |u| ≤ 382 m and |v| ≤ 318 m, 3058 × 2546 points, 31 MB, baked in 0.07–0.16 s at
    start-up on 12 busy cores. Its points are the mound grid's, so the mounds keep their exact heights. Walking, the
    laser's hits, the ground puffs and the shader's primary and shadow marches all read it bilinearly; outside it
    both fall back to the shape, so `OCTAVE_COUNT` and `TERRAIN_OCTAVES` must still match. The shader keeps the grid
    in an R32Float texture and fetches a cell's four points with one `gather`, unfiltered, then interpolates them as
    `terrain.rs` does; at the test's points (`the_shader_draws_the_ground_the_cpu_walks_and_hits_on`) the CPU and the
    shader agree to 1e-7 m. Normals still come from the shape's height, because bilinear heights kink at every cell
    edge, which would show in the light as a 0.25 m grid on the crater walls. **User?** none expected: nothing looks
    different.
    - **Look:** per view, the mean difference is 0.003–0.23 of 255 and at most 0.012% of pixels differ by more
      than 8, at edges where a hit moved by under a millimetre. The laser views' puff of sand differs more (mean 0.52
      and 0.33% of pixels in `laser_ground`), because its particles are seeded from the hit point.
    - **Speed** (Apple M3 Pro, 18-core GPU, macOS 15.7.3, rustc 1.98.1): median GPU ms at 2560x1440, render scale
      0.6, 30 frames a view, the best of 4 alternating rounds, while other agents built and ran bots on the
      machine. `wormhole` 14.5 → 10.3 (p99 15.0 → 10.5), `wormhole_exit` 14.3 → 10.3 (14.8 → 10.6),
      `laser_into_wormhole` 20.5 → 15.8 (21.1 → 16.4); the other 21 views 6.9–13.2 → 5.4–10.3. Every view's median
      and p99 is now under 16.7 ms.
    - **Where `laser_into_wormhole`'s time goes** (it has the least headroom; 60-frame runs with parts of the shot
      left out, in rounds where `wormhole` was not slowed by other agents; without the shot it takes 10.6 ms, like
      `wormhole`): the laser's 19 segments, scanned at every chord and every bend step inside the
      mouth, about 4 ms; the puff of sand where it lands, about 1.8 ms; the laser's spot light, nothing measurable.
      Hoisting each segment's midpoint and reach out of `scan_beams` would be the next saving with the same look.
