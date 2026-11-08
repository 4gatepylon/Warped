# Decisions: Lunar Surface demo

- **Question:** What could Lunar Surface (red, depth 2, hyperbolic, very large, long range, open zones) look and play
  like, rendered correctly in H³ with light bent by its warp objects?
- **Given:** the level's row of `game_design/DESIGN.md` (red team, depth 2, lunar base theme, hyperbolic space, gravity
  down onto the ground, warp objects, very large, 5 consoles per pylon, long range, open zones) and the shared brief:
  at least one black hole and one wormhole pair, the controls, HUD, laser, consoles, sprint energy (30 s / 60 s),
  10 s per console, no pull on players from black holes.
- **Outcomes:** a playable level that shows H³ (exponential room, things shrinking fast with distance, the ground as
  a disc from above), three routes, a defensible red base, and lensing you can see and shoot through. Frame rate on
  Apple silicon is not measured (only SwiftShader was available).

## Scope

### D1. Curvature radius R = 40 m **(needs input)**

- **Choice:** R = 40 m (the existing demos use 8 m).
- **Alternatives:** 8 m (a person 40 m away would be about 2 pixels tall); 60 m or more (H³ would barely show inside
  the middle zone).
- **Rationale:** at 40 m things 100 m away look 2.4 times smaller than in flat space: long-range fights still work,
  while the level, 5.7 R across, still shows strong hyperbolic effects.
- **Revisit if:** playtests find long-range shots too hard (raise R) or the space not strange enough (lower it).

### D2. Size and zones

- **Choice:** spawns 227 m apart on the level's axis; zones split at u = ±45 m; the boundary is the ellipse
  (u/128 m)² + (v/72 m)² = 1 in Fermi coordinates; the middle zone (about 21,000 m²) is the biggest.
- **Alternatives:** a boundary of constant distance from the axis; a ring of mountains as the boundary (too many
  blocks: the ellipse's perimeter is about 1.8 km in H³).
- **Rationale:** big enough to be the largest level, small enough that a sprint across takes about 25 s.

### D3. Routes **(needs input)**

- **Choice:** centre: an open crater (the big open area) with the black hole; left: the habitat row (rooms and corners),
  joined to the crater by a covered walkway into the crater's left entrance (the connector); right: a ridge route with
  a high crest for long sight lines and the wormhole's entrance.
- **Alternatives:** routes that start together and fan apart (as hyperbolic rails do); a third route through a
  canyon.
- **Rationale:** each route has a different range: open and lensed, close quarters, and long sight lines from high
  ground.
- **Revisit if:** the flanks feel too long: they stretch by cosh(v/R), so a route 36 m off the axis is 36% longer than
  the centre route.

### D4. A defensible red base

- **Choice:** a barrier line with gaps at u = 56 m, the red pylon in front of the spawn hangar, a dome and a module on
  the left, a watchtower overlooking the wormhole's exit pad on the right.
- **Alternatives:** a walled compound with one gate.
- **Rationale:** depth 2 should favour red: every way in passes a chokepoint or open ground under the watchtower.

### D5. Zone marking

- **Choice:** glowing team-coloured strips across the ground at u = ±45 m, a 7% team tint on the regolith in each team
  zone, and team-coloured lights on that team's buildings.
- **Alternatives:** banners on poles (more structures to trace).
- **Rationale:** the ground is drawn everywhere anyway, so marking it costs nothing.

## Workload

### D6. One black hole over the crater

- **Choice:** r_s = 1.2 m, 13 m above the crater's centre; its light bending reaches 24 m.
- **Alternatives:** a larger black hole (it would hide most of the crater); several small ones.
- **Rationale:** sight lines across the crater at eye level bend by about 15°, so shots across it follow the bent path, and its
  shadow is 6 m across: visible from far away but never hiding a player for long.

### D7. One wormhole: from the right flank to the red landing pad **(needs input)**

- **Choice:** mouth A at (u, v) = (-14, 40) m at eye height, entered heading toward red; mouth B 116 m away on an open
  landing pad 30 m from the red pylon, facing the pylon. r_s = 1.6 m (throat 0.4 m), reach 15 m.
- **Alternatives:** a wormhole between the flanks (a fast rotation route); the exit inside the red base.
- **Rationale:** a fast but exposed way to the pylon, as the brief suggests. The pad is open ground under the
  watchtower, so red can hold it.
- **Revisit if:** attackers win too easily through it; move B farther out or behind the barrier line.

### D8. Gravity, jumps and speeds **(needs input)**

- **Choice:** 6 m/s² (0.6 g), jump speed 5.2 m/s (2.25 m, enough to clear 2-block barriers), walk 5 m/s, sprint
  9 m/s, fly 14 m/s (40 sprinting).
- **Alternatives:** the Moon's 1.62 m/s² (3 s in the air per hop); the laser gravity demos' 30 m/s².
- **Rationale:** lunar-feeling hops that are still quick enough for a shooter.

## Procedure

### D9. Rendering: a full ray tracer

- **Choice:** every pixel is ray traced in H³: straight geodesics outside the warp objects' reach spheres, stepped
  and bent inside them; structures are voxel grids in their own Klein coordinates (straight-line DDA); the ground is a
  closed-form plane test.
- **Alternatives:** the hybrid allowed by the brief (rasterize, ray trace only near warp objects).
- **Rationale:** one code path stays geometrically correct everywhere, including shadows and lensed laser beams.

### D10. Warp objects' optics and their taper

- **Choice:** refractive index n(ρ) of the hyperbolic distance (isotropic Schwarzschild for the black hole, the black
  hole foil with λ² = 0.25 for the mouths), with ln n multiplied by (1 - (ρ/reach)²)².
- **Alternatives:** subtracting the slope at the reach (shrinks the shadow by 6%); 1 - (ρ/reach)⁴ (bending jumps to
  zero at the reach).
- **Rationale:** the shadow's edge is exact (2.595 vs 2.598 r_s), deflection is within 8% of Schwarzschild up to 10 m,
  and the bending fades smoothly to zero at the reach, so outside it a ray is one geodesic. The index is the flat
  one, valid while r_s ≪ R (here r_s/R ≤ 0.04).

### D11. Blocks in Klein grids

- **Choice:** each structure's blocks are a grid in its frame's Klein coordinates, 1 m at its origin; structures stay
  within about 15 m of their origin (blocks up to about 1.2 m there); long features are 22 m chunks with 0.8 m of
  overlap at the seams.
- **Alternatives:** exponential-map blocks like the earlier demos (faces not geodesic, so no straight-line DDA).
- **Rationale:** geodesic faces make ray tracing exact and cheap.

### D12. Ground grid from the {4,5} tiling

- **Choice:** fold each ground point into the central square of the {4,5} tiling by reflections; cells are 1.2 m apart
  along the tile's midlines. Per-cell variation and small craters are keyed on global positions, so they don't depend
  on the fold.
- **Alternatives:** a grid in Fermi coordinates (cells stretch by cosh(v/R): 2.2 times at the ridge).
- **Rationale:** every tile is identical, as in the earlier H³ demos.

### D13. Sky fixed to the level centre

- **Choice:** stars, the planet and the sun are a skybox in the level centre's frame, carried to the viewer by parallel
  transport. The sun's direction at each surface point is carried from the centre, so it is 17° high everywhere.
- **Alternatives:** put them on H³'s sphere at infinity (physical, but across 5.7 R the planet would rise overhead
  at one end of the level and look 12 times smaller at the other, and the sun's height would change from 0° to 60°).
- **Rationale:** the design asks for a planet low over the horizon and long shadows across the whole level.

### D14. Lighting

- **Choice:** lunar-Lambert reflectance (70% Lommel-Seeliger, 30% Lambert), sun intensity 1.45, ambient 0.11 plus
  earthshine 0.08, straight shadow rays out to 80 m, lamps with falloff in hyperbolic distance and no shadows.
- **Alternatives:** plain Lambert (sunlit walls burn out while the ground stays dark under a 17° sun).
- **Rationale:** regolith looks flat and bright at grazing angles, which this reproduces.

### D15. Players near warp objects

- **Choice:** within a mouth's reach the player moves along light paths (as in `wormholes.html`) and crosses its
  throat; black holes never affect the player, and flying into a horizon respawns you.
- **Alternatives:** a larger "door" sphere around each mouth for players.
- **Rationale:** aiming the crosshair at the disc takes you through, exactly where light goes.

### D16. Laser

- **Choice:** traced from the eye along the crosshair (bent, through throats and glass, stopped by other blocks, the
  ground and horizons, up to 480 m); drawn from a gun 0.22 m right of and 0.25 m below the eye, joining the path 3 m
  out; no gravity drop; fades over 2 s; at most 3 beams on screen. Glass stopped the laser until W-70, when the user
  asked for windows to see and shoot through (the Rust demo's `DECISIONS.md` D90).
- **Alternatives:** the laser gravity demos' falling beam.
- **Rationale:** the brief asks for the shot along the crosshair; the short muzzle segment lets the shooter see it.

### D17. Consoles

- **Choice:** within 3 m of an active console, facing within 57° of the pylon's core; one console at a time.
- **Rationale:** the brief's "about 3 m, facing it".

### D21. Three pylons per team, one console each

- **Choice:** each team has three pylons with one console each (`<team> console 1` to `3`, in `LAYOUT` order), and a
  team loses when all three are off. The first pylon of each team stays where it was; the other two stand on the
  flanks, red's at (58, 42) and (56, −50) and blue's mirrored at (−58, 42) and (−56, −50): at least 2 m clear of every
  other structure's footprint, at least 25 m from every other pylon, and 16 m inside the boundary ellipse.
- **Alternatives:** five consoles on one pylon (the original brief); new red pylons deeper in the red zone (no spot
  there clears the dome, module, solar array, hangar and walkway by 2 m).
- **Rationale:** W-68 (the reality bend) has the black hole grow as pylons fall, and you chose one console per pylon.

## Environment

### D18. Render quality and testing **(needs input)**

- **Choice:** auto quality renders about 400k pixels and steps down by 25% while below 34 fps; tested only in headless
  Chrome with SwiftShader (about 3 fps at low quality on the CPU), plus the numerical checks in `#test=physics`.
- **Rationale:** no GPU was available to the agent; the per-pixel work (about 100 bounding-sphere tests and a few
  short DDAs) should fit an Apple-silicon GPU, but this is unmeasured.
- **Revisit if:** it runs below 30 fps on a Mac: cull structures per frame on the CPU, or skip shadow rays far away.

## Precision

### D19. The shader works relative to a render anchor near the player

- **Choice:** the CPU, in double precision, hands the shader the camera, structures, warp objects, lamps, beams and
  the sky's parallel transport to the centre relative to a render anchor: the level's centre while the player is
  within 80 m of it, otherwise the symmetry of the ground's {4,5} tiling that takes the central tile to the player's
  (found by the same folding the shader uses), moved each time they get 160 m from it. The ground's u and v use a
  sign-safe asinh (the GPU's returns −∞ for large negative arguments, which blacked out ground cells far out on the −v
  side).
- **Alternatives:** an anchor at the player's own Fermi frame (then the shader can't fold the tiling in its
  coordinates); moving it every frame; emulated double precision in the shader.
- **Rationale:** coordinates grow like cosh(d/R), so in the level's own coordinates single precision ran out by 300 m
  from the centre. In a headless test, moving the camera 2 mm changed 3–10% of the pixels at 300–900 m before, and at
  most 0.05% up to 450 m after, the same as at the centre. A new anchor rounds each ground cell's centre a little
  differently, which re-rolls the per-cell noise of about a fifth of the nearby cells, so the anchor stays at the
  centre while the player walks the level (whose rendering is unchanged) and moves only when they fly out.

### D20. Flying stops 400 m (10 R) from the centre **(needs input)**

- **Choice:** fly mode won't take the player more than 400 m from the level's centre ("Edge of the flight area").
- **Alternatives:** no limit, with the player's frame on the CPU also kept relative to the anchor.
- **Rationale:** the CPU keeps the player's frame in the level's coordinates in double precision, which loses digits
  from about 15 R. Beyond about 8 R the base is a few pixels across anyway.
- **Revisit if:** the game needs flying or spectating far from a level.
