# Dust decisions

Decisions made instead of the user (W-121, Dust demo). **User?** marks ones to weigh in on.

1. **Wind as ln n = k·d (a planar gradient).** The simplest refractive field whose light bends one way
   everywhere; k = 0.0012 /m bends a ray ~6 cm over 10 m and ~5 m over 100 m. **User?** strength.
2. **Fog replaces the haze** (start 4 m, e-folding 60 m, max 94%, sand colour) and ends rays at 304 m, which
   also bounds the cost per pixel. **User?** thickness.
3. **No fog animation or blowing sand particles yet**: stills first.
4. **Curvature factor 1.0** in `server_settings.yaml` (Dust's own R = 50 m from the WebGL prior art), not
   Lunar's 1.5.
5. **Layout**: mirrored halves, spawns 208 m apart, one fixed wormhole pair across the middle, mesas and ruins
   for cover on long sight lines. **User?** size.
6. **Bench views replaced** with Dust's (the Lunar list did not fit the map).
7. **Package and binary names kept** (`hyperbolic-wormholes`, `wormholes_server`) so the copy stays a clean diff.
