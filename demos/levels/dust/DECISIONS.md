# Decisions: Dust demo

- **Question:** What does Dust (blue, depth 2, hyperbolic, very large, long range, open zones, 5 consoles) look and play
  like as a blocky level with real obstacles, with light following H³ geodesics bent by a black hole and a wormhole?
- **Given:** the level's row in `game_design/DESIGN.md` (theme, space, gravity onto the ground, warp objects, size,
  combat focus, consoles, open zone layout, 2–3 routes with a connector) and the shared level brief (files, controls,
  HUD, hash parameters, code practices, laser fading over about 2 s, 10 s per console, energy 30 s / 60 s).
- **Outcomes:** a playable page with the space rendered correctly, the layout, walking, warp objects, laser and
  consoles; plus the accuracy of the light paths (checked against a finer integration) and the frame cost (checked
  against `wormholes.html` in SwiftShader).

## Scope

### D1. Rendering method: a full ray tracer

- **Choice:** every pixel is ray traced in the hyperboloid model: single geodesics outside the warp objects' balls of
  influence, bent midpoint steps inside them. No rasterization.
- **Alternatives:** the hybrid of hint (d) (rasterize as `hyperbolic.html` does, ray trace near warp objects).
- **Rationale:** the black hole sits over the middle of the level and bends most sight lines across it, so a hybrid
  would ray trace most of the screen anyway, and the beam must be lensed too. One tracer is simpler and exactly correct.

### D2. Blocks are cubes in each structure's own Klein coordinates (needs input)

- **Choice:** each structure is a voxel grid in its local Klein coordinates (meters), placed by an isometry. Faces are
  exact geodesic planes and rays cross the grid with a plain DDA; blocks grow up to ~20% toward a big structure's corners.
- **Alternatives:** blocks laid out by the exponential map (as `hyperbolic.html`, uniform near the base but faces only
  approximately geodesic); a {4,5} tiling of the ground with Klein-subdivided tiles (tiles 62 m across at R = 50 m, so
  blocks ~1.9× bigger at tile corners).
- **Rationale:** exact faces, the same lookup for rendering, the laser and collisions, and modest distortion as long as
  structures stay within ~20 m of their centre.
- **Revisit if:** the game needs uniform 1 m blocks everywhere (then structures must be smaller, or blocks curved).

### D3. Warp-object optics: flat index near the object, H³ falloff, cut off (needs input)

- **Choice:** n(ρ) is the flat demos' isotropic index (black hole; wormhole "black hole foil", λ² = 0.25); its gradient
  is multiplied by (ρ / R sinh(ρ/R))² (a point mass's field in H³) and faded to zero between half the cut radius and the
  cut radius (34 m for the black hole, 16 m for the mouths). Several objects' gradients are added.
- **Alternatives:** no cutoff (every ray in the level would need bending steps); an exact H³ black-hole metric
  (Kottler-like; much more work for a demo).
- **Rationale:** close to the object (ρ ≪ R) it is the validated flat optics; the cutoff makes rays far from warp
  objects exact geodesics and keeps the tracer fast. It shrinks the shadow from 2.6 to 2.3 r_s.

## Workload

### D4. Curvature radius R = 50 m (needs input)

- **Choice:** R = 50 m.
- **Alternatives:** 8 m as in the existing demos (a 40 m sight line would already look 9× farther away); 80–100 m.
- **Rationale:** at 100 m a target looks as small as at 180 m in flat space, so long range still works, while the side
  routes are still 35–40% longer than the centre and a 100 m circle is 1.8× longer than in flat space.
- **Revisit if:** playtests find long shots too hard (raise R) or the space not hyperbolic enough (lower it).

### D5. Size and positions (needs input)

- **Choice:** spawns at s = ±108 m (216 m apart), pylons at ±88 m, zone edges at ±55 m (middle zone 110 m long), a
  boundary ellipse with Fermi semi-axes 128 m × 62 m closed by cliffs 14 m thick and 19–28 m tall.
- **Alternatives:** a larger level (sight lines beyond ~150 m become a few pixels at R = 50 m); an open boundary.
- **Rationale:** "very large" while keeping every sight line across the middle readable.

### D6. Routes and connectors

- **Choice:** the open Salt Flats in the centre; a dry riverbed (blue's right) between 5 m banks, with ramps and two arch
  bridges; sunken ruins (blue's left). The Spine ridge separates the flats from the riverbed, with a tunnel (the Eye) as
  the connector; four buttes separate the flats from the ruins, their gaps acting as connectors.
- **Alternatives:** routes that are straight geodesics fanning out from each base.
- **Rationale:** matches "2–3 routes, one a big open area joined to another by a connector"; the side routes follow
  curves at constant distance from the axis, so hyperbolic geometry makes them visibly longer.

### D7. Blue's base is the more defensible one (needs input)

- **Choice:** blue's pylon stands in a walled courtyard with four gates, corner stairs and a wall walk; red's stands in an
  open ring of standing stones with a few low walls.
- **Alternatives:** mirror-symmetric bases.
- **Rationale:** the design makes deeper levels slightly easier for their own team, and Dust is only ever attacked by red.

### D8. Warp objects and where they are (needs input)

- **Choice:** one black hole (r_s = 2.4 m) 16 m above the middle of the flats; one wormhole (mouths r_s = 1.6 m,
  throat 0.4 m, at eye height) from the flats near red's side (s = 40 m) to 25 m in front of blue's pylon, 111 m apart.
- **Alternatives:** two black holes; a symmetric pair of wormholes.
- **Rationale:** the black hole bends every long sight line across the open middle without blocking walking; the
  wormhole is red's fast but exposed route toward the pylon. One of each keeps the tracer's cost down.

### D9. Zone marking

- **Choice:** sand tinted 12% toward the team colour in each team's zone, a team-coloured stripe across the level at each
  zone edge with two banners, team-coloured trim and cloth at the bases and spawns.
- **Alternatives:** coloured blocks along the zone edges (more structures to trace).
- **Rationale:** reads at a glance and costs nothing in the tracer (the sand colour is a texture lookup).

## Procedure

### D10. Movement numbers (needs input)

- **Choice:** walk 5.5 m/s, sprint 9.5 m/s, gravity 30 m/s², jump 8.5 m/s, falling 1.6× faster than rising (the last
  three as in the laser gravity demos), step-up 1.2 m, fly 16 m/s (55 m/s with Shift).
- **Alternatives:** step-up 1.05 m as in the laser gravity demos.
- **Rationale:** the design leaves sprint speed TBD; 1.2 m lets you climb blocks that the Klein chart makes up to ~8%
  taller near big structures' edges, while 2-block walls stay unclimbable.

### D11. Walking and the warp objects

- **Choice:** the black hole doesn't pull players (fly mode stops 6 m from it). Within 8 m of a mouth (faded from 5 m),
  the walking direction bends by light's law, horizontally, so you walk in where you see the disk; crossing the throat
  maps your eye and view through the gluing.
- **Alternatives:** no bending (players must hit a 0.4 m throat exactly); full 3D light-like motion (lifts players off
  the ground).
- **Rationale:** consistent with the light paths, and walking into the disk works from up to about 2 m off-centre.

### D12. The laser

- **Choice:** traced from the eye along the crosshair (so it hits exactly what the crosshair covers), with its first 4 m
  drawn from the gun; stopped by every block; at most 450 m; an escaping beam is drawn for 300 m; brightness
  (1 − age / 2 s)²; up to 4 beams at once.
- **Alternatives:** traced from the gun parallel to the view (as the earlier demos): it then misses the 0.4 m throat
  when you aim at the mouth's centre.
- **Rationale:** the brief's "from the eye along the crosshair", while still letting the shooter see the beam.

### D13. Consoles

- **Choice:** within 4.5 m of the pylon's axis (about 3 m from the crystal) and facing it within 46°; consoles go off in
  order; no shooting while deactivating.
- **Alternatives:** per-console positions you must stand at.
- **Rationale:** simple and matches the design's rules.

## Metrics

### D14. Integration step 0.18 ρ

- **Choice:** bending steps of 0.18 times the distance to the nearest warp object (0.02–6 m), midpoint method with the
  original direction parallel-transported.
- **Alternatives:** 0.12 ρ (half the error, 1.4× the steps).
- **Rationale:** against a 100× finer integration the direction error is 0.02° at 10 r_s, 0.1° at 6 r_s and 2.4° at
  the photon ring, which only affects a thin ring of the image.

## Environment

### D15. Performance not measured on a real GPU (needs input)

- **Choice:** the auto render quality starts at about 380k pixels and steps down while below 40 fps, as
  `wormholes.html` does. Measured only in headless Chrome with SwiftShader: 0.35–0.75 s per 640×360 frame, against
  ~1.5 s for `wormholes.html` at the same size.
- **Alternatives:** none in this session (no GPU access).
- **Rationale:** `wormholes.html` is reported to hold ~40 fps at this budget on Apple silicon, and Dust costs about a
  third as much per pixel, so 30+ fps is likely but unverified.
- **Revisit if:** a measurement on Apple silicon shows otherwise.

## Analysis

### D16. Sky and sunlight

- **Choice:** the sky is painted at infinity and coloured by the direction from the camera to the ray's ideal end point;
  sunlight is a fixed direction in each structure's frame and a constant brightness on the sand, with no shadows.
- **Alternatives:** a sun at an ideal point (its elevation would change by tens of degrees across the level).
- **Rationale:** a seam-free lensed sky (Einstein rings) and even lighting across a level 4 R long.
