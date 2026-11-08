# Dust decisions

Decisions made instead of the user (W-121, Dust demo). **User?** marks ones to weigh in on.

1. **Wind as ln n = k·d (a planar gradient).** The simplest refractive field whose light bends one way
   everywhere; k = 0.0012 /m bends a ray ~6 cm over 10 m and ~5 m over 100 m. **User?** strength.
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
9. **Laser colour [1, 0.7, 0.25]** (display RGB) for every team, from the map file: warmer than the house map's
   near-white core so the gold shows through the core's whitening. **User?** shade.
10. **Map picture straight down, not oblique**, so marks drawn at map coordinates land on what they mark; height
    60 m, 3×3 supersampling, fog and wind off, sun fixed in the Fermi frame. Committed (0.8 MB) rather than
    rendered at each start, which would need the GPU before the window opens. **User?** look.
