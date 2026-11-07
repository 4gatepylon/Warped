# Decisions: City Sphere demo

- **Question:** What could City Sphere look and play like as a blocky, walkable level: a city on the
  inside of a sphere with outward gravity and a glass second floor, built for close quarters?
- **Given:** the design doc's City Sphere (purple, depth 0, smallest, close quarters, 4 consoles per
  pylon, spawns and pylons at antipodes, band zones, 2–3 routes, two floors with a glass inner floor
  that roofs the outer floor and stops shots, gravity outward on both, no warp objects), and the
  shared level brief (controls, HUD, laser, consoles, sprint energy, viewpoints, three.js).
- **Outcomes:** a playable level to judge by walking it: whether the curvature reads, whether the
  streets feel like close quarters, whether the glass floor is a useful third route, and whether the
  sizes are right.

## Space and grid

### D1. Radii: outer floor 36 m, glass floor 28.8 m, core 5 m (needs input)

- **Choice:** the outer floor is a sphere of radius 36 m (113 m from spawn to spawn along the floor,
  226 m round the equator). The glass floor is 8 blocks (7.2 m) further in, at 28.8 m. A glowing core
  of radius 5 m sits at the centre.
- **Alternatives:** 30 m (tighter, more curved, but too small for three routes on a cube-sphere
  grid); 40–45 m (more room, flatter streets).
- **Rationale:** at 36 m a level sight line meets the floor 11 m ahead, so the curvature is obvious
  everywhere, while the streets still have room for several turns. The 7.2 m gap fits 2–7 block
  buildings under the glass.
- **Revisit if:** playtests find the level too cramped for 5 v 5, or the glass floor too close to the
  core.

### D2. Blocks on a cube-sphere grid with exponentially thinning layers

- **Choice:** an equiangular cube-sphere grid, 56 cells per cube edge (about 1 m on the outer floor).
  Layer k spans R e^-(k+1)Δ to R e^-kΔ with Δ = 90°/56, so every block is a nearly cubic piece of a
  spherical shell: 1 m on the outer floor, 0.8 m on the glass floor.
- **Alternatives:** latitude–longitude cells (they shrink to slivers at the poles, where the spawns
  are); 1 m thick layers everywhere (blocks near the glass would be 0.8 m wide and 1 m tall).
- **Rationale:** the cube-sphere has no singular points and nearly square cells; the exponential
  layers keep blocks looking like blocks at every radius.

### D3. Red's half is blue's half turned 180° about an axis through the equator (needs input)

- **Choice:** the map is drawn for blue's hemisphere and copied to red's by a 180° rotation about the
  z axis. The pylon is offset from the spawn along x, where this rotation and the design's
  reflection through the centre give the same point, so spawns and pylons still match the design.
- **Alternatives:** reflecting the whole map through the centre, as the design says for the spawns
  and pylons.
- **Rationale:** a reflection through a point is a mirror image, so red would play a left-right
  mirrored map; the rotation gives both teams the same map. It also makes the plaza and the alleys
  each symmetric routes from blue to red.

## Layout

### D4. Zones are polar caps of 45° round each spawn

- **Choice:** blue's zone is everything within 45° of blue's spawn (seen from the centre), red's the
  same round red's, and the middle zone is the band between (71% of the floor). Team zones have
  tinted floors, team-coloured trim on building tops, tinted glass and a glowing border stripe.
- **Alternatives:** 30° caps (bigger middle zone); zones that follow the cube faces.
- **Rationale:** 45° puts the zone border just outside each team's ring road, so a team can rotate
  between routes inside its own zone.

### D5. Three routes: the plaza, the alleys and the glass floor, joined by an equator street

- **Choice:** the plaza (a 40 × 36 m open area on the equator, with kiosks and low walls for cover),
  the alleys (2 m alleys with corners, on the opposite side of the sphere) and the glass floor above
  everything. An 8 m wide street round the equator connects the plaza and the alleys on both sides.
  The rest of the middle band is filled with buildings, with a few alcoves off the equator street.
- **Alternatives:** four street routes, one per side face of the cube; a single floor (the design
  allows it).
- **Rationale:** the design asks for 2–3 routes with one big open area joined by a connector. Close
  quarters come from 2–4 m streets between buildings that mostly reach the glass.

### D6. Spawn and pylon 12 m apart; a ring road round each spawn

- **Choice:** the pylon is a 2 × 2 m tower, 5 blocks tall, 12 m from the spawn, in a courtyard open
  to the spawn yard and to the ring road, with a light beacon up to the glass. Its 4 consoles are on
  its four sides at 1.3 m.
- **Alternatives:** the pylon further out, at a route entrance.
- **Rationale:** "a short segment" in the design; attackers reach it from two sides of the ring road.

### D7. Stairs: four 4 m wide staircases through holes in the glass

- **Choice:** one staircase in each team zone and one in each half of the plaza, 8 steps of one block.
  The glass has a hole over the top three steps, and a 2-block railing on the glass floor rings the
  hole (except at the top step), so nobody falls through by accident.
- **Alternatives:** ramps (need slope collisions); open holes as one-way drops to the streets.
- **Rationale:** steps of one block work with the step-up rule every demo uses.

### D8. The glass floor has some cover but long sight lines across the sphere (needs input)

- **Choice:** the roofs of buildings that reach the glass are opaque patches of the glass floor, and a
  few 2–3 block walls stand on it. Players on the glass floor can see (and shoot) across the inside of
  the sphere, up to about 55 m; only the core blocks the lines between opposite sides.
- **Alternatives:** a bigger core, or spokes from the glass floor to the core, to break those lines;
  a glass floor with a glass roof of its own.
- **Rationale:** it makes the glass floor the risky, exposed route, which contrasts with the streets.
- **Revisit if:** the glass floor turns into a sniper's nest that dominates the streets' exits.

## Movement

### D9. Gravity 30 m/s² outward, the same everywhere

- **Choice:** a constant 30 m/s² pull away from the centre on both floors (as in the other demos),
  jump speed 8.5 m/s (1.2 m high: enough for one block, not for 2-block cover).
- **Alternatives:** a pull that grows with radius (like a spinning station).
- **Rationale:** matches the other demos; a real hollow shell has no gravity inside it anyway.

### D10. Walk 6 m/s, sprint 9.5 m/s (needs input)

- **Choice:** walking at 6 m/s (about a tactical shooter's run), sprinting at 9.5 m/s.
- **Alternatives:** the other demos' 7 and 18 m/s (18 m/s crosses a 2 m alley in 0.1 s).
- **Rationale:** the design leaves sprint speed TBD; close quarters favour a modest sprint.

### D11. A body of fixed angular size

- **Choice:** the body is a square column whose corners lie on radial lines 0.6 m apart on the outer
  floor (so 0.48 m on the glass floor); eye 1.7 m, head 1.85 m.
- **Alternatives:** a body of fixed width in metres.
- **Rationale:** with a fixed width in metres, jumping or stepping up widens the body's angular
  footprint and catches it on the wall it stands against.

## Laser and consoles

### D12. The laser is a straight line with no fire-rate limit

- **Choice:** one shot per click or `R`, from the eye along the crosshair, stopped by blocks, glass,
  the core, the ground and the static figures; no fire-rate limit, overheating or damage.
- **Alternatives:** the sniper rifle's one shot every 2 s.
- **Rationale:** the brief asks for no damage model; a limit would only slow testing.

### D13. Console rules

- **Choice:** holding `E` works within 3 m of the pylon's side, facing it within 60°, on the outer
  floor, and deactivates the nearest active console; the laser can't fire meanwhile.
- **Alternatives:** choosing the console you look at.
- **Rationale:** the simplest reading of the design; the design's open questions (interruptions,
  several players at once) are left out.

## Rendering

### D14. Rasterized meshes with baked shading, and a cut-open overview

- **Choice:** one merged mesh of exposed block faces with per-block noise, dark edges, windows, face
  shading and corner ambient occlusion baked in, plus distance haze; no lights or shadows. The
  overview puts the camera outside the sphere and cuts away the half facing it.
- **Alternatives:** a point light at the core with cube shadow maps (its light runs parallel to every
  wall, so walls would be unlit); an overview from inside, near the core.
- **Rationale:** cheap (about 100,000 triangles, 8 draw calls) and readable; the cut-open view shows
  the whole half-level at once.

## Environment

### D15. Tested in headless Chrome only

- **Choice:** checked with headless Chrome 156 and software WebGL (SwiftShader) on an Apple M3 Pro,
  macOS 15.7: screenshots of every viewpoint and scripted checks of walking, walls, jumping, stairs
  (both teams), railings, fly mode, energy, the laser against the glass and a figure, and the
  consoles.
- **Alternatives:** a run on the GPU in a normal browser window.
- **Rationale:** the brief rules out a visible browser while other agents run; the frame rate on a
  GPU is still to be measured.
