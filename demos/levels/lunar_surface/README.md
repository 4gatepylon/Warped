# Lunar Surface

A playable demo of Lunar Surface, the deepest red level of the default map (red, depth 2): a lunar base on a grey
regolith plain in hyperbolic space H³, with a black hole hanging over a blocky crater and a wormhole from the right
flank to the red base. Every pixel is ray traced along H³ geodesics, bent by the black hole and the wormhole. It is a
first try at the README checklist's first item for this level; the layout is a proposal to argue with.

## The level

- **Space:** H³ with curvature radius R = 40 m. Gravity pulls toward the ground plane at 6 m/s² (moon-like).
- **Zones** (u runs from blue to red along the level's axis, v across it): the blue zone is u < -45 m, the middle zone
  -45 to 45 m, the red zone u > 45 m. Glowing strips on the ground mark the zone edges and the regolith is tinted in
  the team zones. The boundary is the ellipse (u/128 m)² + (v/72 m)² = 1, striped yellow and black. The spawns are
  227 m apart.
- **Routes across the middle zone:**
  1. **Crater** (centre, the big open area): a crater 48 m across with raised blocky rims, entrances on the blue, red
     and left sides, a central peak, and the black hole 13 m above it.
  2. **Habitat row** (left): derelict habitat modules, a comms dome, a relay mast, crates and solar panels, for corners
     and cover. A covered walkway from the dome into the crater's left entrance connects it to the crater route.
  3. **Ridge route** (right): boulders along the foot of a ridge whose crest has long sight lines over the crater, and
     wormhole mouth A, which leads to the red landing pad 30 m from the red pylon.
- **Blue zone:** the blue lander (spawn), the blue pylon on a deployed pad, crates, a solar array and a relay mast.
- **Red zone** (the most defensible): a barrier line with gaps, a red habitat module, the red dome, a watchtower over
  the landing pad where the wormhole comes out, and the red pylon in front of the red spawn hangar, all red-lit.

## What to try

- **The black hole** (`5`): the far red base appears in an Einstein ring around the black shadow, and the crater rim
  behind it bends into an arc. From `4` (the crater's blue entrance) sight lines across the crater bend upward, and
  the planet shows twice. `Einstein ring` (a panel button) lines the black hole up with the planet.
- **The laser** (`R` or click): shoot past the black hole and the beam bends exactly as the light you see does, so you
  hit what the crosshair covers; its lensed second image shows next to the shadow. Shoot into the wormhole (`6`) and
  it hits the red pylon 42 m away.
- **The wormhole** (`6`): its disc shows the red landing pad 116 m away. Walk in and you come out on that pad (`7`
  looks back at the exit), in view of the red watchtower.
- **Room grows exponentially:** the overview (`3`) shows the whole infinite plain as a disc. Lengths along the flanks
  stretch by cosh(v/R): the ridge, 57 m off the axis, is 211 m long but spans only 96 m of the axis.
- **Long range** (`9`, the ridge crest): things shrink with distance by R·sinh(d/R)/d more than in flat space (2.4
  times at 100 m). A level shot rises away from the curving ground: 100 m out it passes 10 m up, so aim low.
- **Harsh light:** the sun is 17° high everywhere on the level, so shadows are 3.3 times as long as things are tall.
- **Consoles** (`8`): hold `E` within 3 m of a red console, facing the pylon, for 10 s. `T` switches team.

## Controls

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (energy drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode (at most 400 m from the centre) |
| `E` (hold) | deactivate a console of the other team's pylon |
| Click (captured) or `R` | fire the laser |
| `T` | play as the other team (moves you to its spawn) |
| `1`–`9` | viewpoints |

## Viewpoints and hash parameters

`#view=` names (keys `1`–`9` are the first nine): `blue_spawn`, `red_spawn`, `overview`, `middle`, `black_hole`,
`wormhole`, `wormhole_exit`, `red_pylon`, `ridge`, `einstein_ring`, `overlook`, `habitat_row`.

Also: `#quality=low|medium|high|full` (default auto), `#pitch=<degrees>`, `#panels=off`, `#team=red`, `#test=laser`
(fires once after the first frame and holds the beam; `&after=<view>` then moves the camera), and `#test=physics`
(logs the numerical checks below to the console). `window.__demo` exposes the state, `set_view`, `fire_laser`,
`trace_laser`, `step_player`, `update_consoles` and `run_physics_checks`.

## How it works

- **Model.** Points are 4-vectors on the hyperboloid <x, x> = -1 (curvature units) and isometries are Lorentz
  matrices, as in `../../three_js_vibecode_demo/hyperbolic.html`. The layout uses Fermi coordinates of the level's
  axis: the frame at (u, v) is a boost by -u/R along z, then by v/R along x.
- **Blocks.** Each of the 53 structures has its own frame and a voxel grid in that frame's Klein coordinates, where
  geodesics and geodesic planes are straight. So block faces are geodesic planes and a ray crosses a structure as a
  straight line: an ordinary voxel DDA, done at the ray's closest approach to the structure for float precision. A
  block is 1 m at its structure's origin and up to about 1.2 m at 15 m from it. The crater rim and the ridge are split
  into chunks of about 22 m whose seams overlap by 0.8 m.
- **Ground.** The ground is one geodesic plane, hit with one closed-form test. Its block cells come from the {4,5}
  tiling (squares with 72° corners, five at a corner) folded into the central tile by reflections, 1.2 m apart.
- **Rays.** Outside every warp object's reach sphere a ray is one geodesic, tested against the ground and against every
  structure's bounding sphere. Inside a reach sphere it is integrated in steps of 8% of the distance to the object:
  half a kick v' = P(∇ln n), a geodesic chord (x'' = x), half a kick. Each chord is tested against the structures in
  that sphere. The reach spheres don't overlap (the code checks), so no two objects' effects need combining.
- **Warp objects** are refractive indices of the hyperbolic distance ρ to their centre: the black hole uses the
  isotropic Schwarzschild index n = (1 + m/ρ)³ / (1 - m/ρ) with m = r_s/4 (r_s = 1.2 m); the wormhole mouths use the
  black hole foil of `../../three_js_vibecode_demo/wormholes.html` (r_s = 1.6 m, λ² = 0.25). ln n is tapered by
  (1 - (ρ/reach)²)² to zero at the reach (24 m for the black hole, 15 m for a mouth). A ray inside a throat sphere is
  log-mapped to the mouth's tangent space, inverted in the throat sphere, carried to the partner mouth's frame by
  diag(1, 1, -1) and exp-mapped there.
- **Checks** (`#test=physics`): the shadow's edge is at 2.595 r_s (Schwarzschild: 2.598); deflection is within 8% of
  exact Schwarzschild up to 10 m (b = 4, 6, 10 m: 75.0°, 36.6°, 18.3° against 72.5°, 33.8°, 16.9°) and fades to zero
  at 24 m; a ray into mouth A leaves mouth B along B's axis; you stand on the pad at 2.60 m eye height, stop 0.08 m
  short of a wall, and jump 2.21 m (formula: 2.25 m).
- **Sky and light.** Stars, the planet and the sun are fixed in the level centre's frame and carried to you by parallel
  transport, like a skybox. The sun's direction at each point is carried the same way, so it stands 17° high
  everywhere. Surfaces use lunar-Lambert reflectance (70% Lommel-Seeliger, 30% Lambert) with a shadow ray toward the
  sun, plus faint earthshine and lamps whose light falls off with hyperbolic distance.
- **Player.** As in `../../laser_gravity_demo/hyperbolic.html`: a body frame on the ground plane plus an eye height,
  probe points for collisions, single-block steps. Near a wormhole mouth you move along light paths (as in
  `wormholes.html`), so walking into the disc takes you through.
- **Laser.** The shot is traced on the CPU from the eye along the crosshair with the same optics, and drawn from a
  gun beside the eye that joins the path 3 m out. The ray tracer tests each ray chord against the beam's geodesic
  segments in closed form (cosh² of the distance from the chord to a segment's geodesic is K0 + K1 cosh 2s + K2 sinh
  2s), so you see beams bent and lensed too.
- **Precision.** Hyperboloid coordinates grow like cosh(d/R) and the GPU computes in single precision, so in the
  level's own coordinates everything near you lost its last digits by 300 m out: the ground broke into noise and the
  stars jittered. The shader works relative to a render anchor instead: the level's centre while you are within 80 m
  of it, otherwise the symmetry of the ground's tiling that takes the central tile to yours, moved each time you get
  160 m from it. Because the anchor maps the tiling to itself, the shader folds the ground exactly as at the centre.
  The CPU hands the shader the camera, structures, warp objects, lamps, beams and the sky's transport to the centre
  relative to the anchor, in double precision. Fly mode stops 400 m (10 R) from the centre, where double precision on
  the CPU is still exact and the base is a few pixels across.

## Running

```bash
python3 -m http.server --directory demos
```

Then open `http://localhost:8000/levels/lunar_surface/index.html`. three.js loads from a CDN.

## Exporting the map

```bash
node demos/levels/lunar_surface/export_map.mjs
```

writes the level's layout (structures, black hole, wormhole, lamps, sky, spawns) to `common/maps/lunar_surface/` in the
portable map format (`common/maps/README.md`), by running this page's own layout code in Node. `shape_builders.js` turns the
blue lander into the Stekelvarken kazemat (6 m tall), the red hangar into the desert bunker garage (9 m), the derelict
habitat modules into WW2 field bunkers (5.5 m), the pylons into sci-fi beacons, each one console that a team turns off to
win, and the red barriers into rows of Hesco blocks, with seven more rows on the blue side. Each is drawn as its Sketchfab
model (`common/maps/models/`, credits there), whose triangles stop players, so the bunkers can be walked into; the red
spawn is inside the garage. Rerun it after changing
the layout, then `make -C common/maps validate-maps`.

## Known limitations

- The frame rate was only tested in headless Chrome with SwiftShader (a software renderer), not on a real GPU. Auto
  quality renders about 400k pixels and steps down while below 34 fps.
- Shadow rays are straight, so the black hole doesn't lens shadows. Lamps cast no shadows.
- Blocks are not all the same size (the Klein grid stretches away from each structure's origin), and terrain chunks
  overlap slightly at their seams.
- Light bends only inside the reach spheres; between 10 m and 24 m from the black hole the bending is weaker than a
  real black hole's, and beyond 24 m there is none.
- The sky is a parallel-transported skybox rather than the sphere at infinity, so it barely moves as you walk.
- Glass lets light and the laser through but doesn't refract; it stops players. The laser has no gravity drop. No damage model.
