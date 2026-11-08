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
