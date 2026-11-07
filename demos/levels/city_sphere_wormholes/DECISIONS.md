# Decisions: City Sphere with wormholes demo

- **Question:** What could City Sphere look and play like with traversable wormholes added? The playable demo
  covers its layout, two glass floors, outward gravity and wormholes, as a first try at the README checklist's first
  item.
- **Given:** the design doc's City Sphere (purple level, depth 0, smallest, city or space station theme, close
  quarters, 4 consoles per pylon, inside a sphere, gravity outward, spawns and pylons at antipodes, "bands" zones,
  two floors with a glass inner floor that the laser can't cross). The level brief adds 2–3 traversable wormhole
  pairs, a ray tracer, and the controls, HUD and file layout shared by all level demos.
- **Outcomes:** whether the space, layout and wormholes read clearly and play well. It runs at 30+ fps at "auto"
  quality, or it doesn't.

## Scope

### D1. Three wormhole pairs, symmetric under the point reflection

- **Choice:** the Crosslink is its own mirror image. The Blue lift and Red lift are mirror images of each other.
- **Alternatives:** two pairs (Crosslink plus one lift, which would favour one team); between-floor shortcuts only.
- **Rationale:** the red half is the blue half reflected through the centre, so every warp object must have a mirror
  image. Three pairs is the most the brief allows.

### D2. Players are not pulled by the mouths

- **Choice:** mouths bend light, lasers and your path only while you go through. They don't pull on you otherwise.
- **Alternatives:** a game-scaled pull, like the balls in `wormholes.html`.
- **Rationale:** the brief's default. A pull in narrow streets would mostly be a nuisance.

## Layout

### D3. Size: outer floor radius 36 m, glass at 28–29 m, inner buildings up to 22 m

- **Choice:** a 7 m outer room under 1 m thick glass. The inner floor's buildings rise up to 6 m toward the centre.
  The core has a 2.5 m radius.
- **Alternatives:** the mirror sphere demos' 30 m; 40–45 m.
- **Rationale:** the four faces you can walk on outside cover about 12,800 m² (streets and buildings), plus
  9,800 m² of glass: the smallest level. Looking level, the
  concave floor rises into view about 10 m ahead, which keeps sight lines short.
- **Revisit if:** playtests feel cramped with 5 v 5.

### D4. Cube faces are the districts; the zones are the two polar faces and the equatorial band (needs input)

- **Choice:** blue zone = face −Y (cap round the south pole), red = +Y, middle = the four equatorial faces (2/3 of
  the surface). The design says "thinner bands" for the team zones; here they are caps, the band's ends.
- **Alternatives:** zones cut by latitude (they would cut across the block grid diagonally).
- **Rationale:** walls on grid lines look straight, and the cube edges make clean zone borders.

### D5. Routes: two street routes plus the glass skyway; the habitat sides of the band are sealed (needs input)

- **Choice:** the point reflection pairs face +X with −X and +Z with −Z. So outer-floor routes come in pairs. Routes
  A (+X) and B (−X) are streets; the skyway (the whole glass floor) is the third route and the "big open area",
  joined to the streets by 4 stairwells. Faces ±Z are sealed habitat blocks (a third of the outer floor).
- **Alternatives:** four street routes (one per equatorial face, more than the design's 2–3); ±Z as dead-end rooms.
- **Rationale:** it matches "2–3 routes" and keeps the smallest level small, and the skyway gives the open area
  plus the connectors.

### D6. Pylon at the pole, spawn 21 m away toward a sealed side

- **Choice:** the pylon (2 × 2 blocks, 5 m tall, consoles on its four sides) stands in a plaza at the pole. The spawn
  room sits 21 m away, backed by the habitat blocks.
- **Alternatives:** spawn at the pole, pylon off-centre.
- **Rationale:** the pylon is the same distance from all four route exits, and the spawn can't be entered from
  behind.

### D7. Lifts land on the glass above the enemy zone (needs input)

- **Choice:** the risky fast route: a lift gate in your own half of a street route puts you on the glass above the
  enemy pylon. From there you take their stairwell down.
- **Alternatives:** landing on the outer floor next to the pylon (too strong); inner-to-outer shortcuts only.
- **Rationale:** fast but exposed: defenders below see you through the glass but can't shoot through it, and the
  stairwell is a choke point.

## Wormholes

### D8. Mouth size r_s = 0.7 m (shadow 1.46 m), in 2 × 4 m doorways, centred at eye height

- **Choice:** the mouth's centre is 1.65 m above the floor (eye height plus a 5 cm lift). Anyone walking through the
  doorway is aimed inside the shadow.
- **Alternatives:** larger mouths (more lensing everywhere); wider doorways (players hugging the frame would miss the
  throat).
- **Rationale:** a walker enters level and comes out level. Entering 15 cm off-centre swings you about 45° (the
  throat magnifies offsets), which threw walkers into the floor.

### D9. Through a mouth you ride the light ray, then gravity resumes (needs input)

- **Choice:** within 2.2 m of a mouth, moving with an impact parameter (times the local index) under 0.85 of the
  shadow radius:
  - You move as a point along the light ray at your speed, with no gravity, as `wormholes.html`'s flying does.
  - Past every mouth's 2.2 m zone, walking resumes with the ray's velocity.
  - Over 0.35 s the view rolls back to the local up, and over 0.8 s it returns to the pitch you went in with.
  - A block in the ray's way ends the transit. A walker whose eye still reaches a throat crosses anyway.
- **Alternatives:** portal-style teleport (straight out, ignoring the optics); keeping gravity on in the throat.
- **Rationale:** what you see in the mouth is where you go, as in `wormholes.html`, and you land on your feet.
  The physics keeps the exit tilt when you go in off-centre.

### D10. Several mouths' bending is summed

- **Choice:** grad ln n is summed over all six mouths, everywhere (not cut off with distance).
- **Alternatives:** a smooth cut-off radius (straight light away from mouths, cheaper).
- **Rationale:** same as `wormholes.html`, and faithful far from mouths. Lines 10–20 m from a mouth bend by a few
  degrees.

## Movement

### D11. Gravity 25 m/s² outward on both floors; jump 7.5 m/s (about 1.1 m)

- **Choice:** a constant outward pull everywhere, with stepping up one block and air control.
- **Alternatives:** the mirror sphere's 32 m/s²; the laser gravity demos' 30 m/s² with a heavier fall.
- **Rationale:** snappy but readable. A jump clears one block. A real shell has no gravity inside, so this is a game
  rule.

### D12. Walk 5 m/s, sprint 8 m/s (needs input)

- **Choice:** sprint energy as in the design (drains in 30 s, refills in 60 s).
- **Alternatives:** the laser gravity demos' 7 and 18 blocks/s.
- **Rationale:** close quarters on a small map; 5 m/s is close to tactical shooters' run speed. The design leaves
  sprint speed TBD.

### D13. Console reach and facing

- **Choice:** within 3 m of the pylon's side (measured along the floor), on the outer floor, with your walking
  direction within 46° of the pylon. The nearest live console is the one deactivated. Firing is blocked while you
  hold E.
- **Alternatives:** reach measured from each console; a 30° facing cone.
- **Rationale:** the brief's "about 3 m, facing it" and the design's "can't use a weapon".

## Rendering

### D14. Voxels on an equiangular cubed sphere, not a Cartesian grid

- **Choice:** 6 × 48 × 48 columns of 15 radial layers of 1 m. Exact ray–plane and ray–sphere cell walls; the glass is
  one voxel layer with openings for the stairs.
- **Alternatives:** a Cartesian voxel grid around the sphere (buildings would lean and the floor would terrace,
  except near the six cube-face centres); a list of oriented boxes (too slow to test per step).
- **Rationale:** upright blocks and smooth floors everywhere, and a cell walk only a few times costlier than a
  Cartesian one.
- **Revisit if:** blocks 0.7–1.2 m wide look wrong next to 1 m blocks in other levels.

### D15. Light from the core, no shadows

- **Choice:** fixed shading per block face (tops brightest), darkening at the foot of walls, emissive windows,
  haze, and an under-glass tint.
- **Alternatives:** shadow rays toward the core (doubles the cost).
- **Rationale:** reads well, and stays within the frame budget.

## Environment

### D16. Performance checked only in headless SwiftShader (needs input)

- **Choice:** in SwiftShader a frame costs 0.25–0.7× a frame of `wormholes.html` at the same resolution. "Auto"
  quality scales down until about 40 fps, as `wormholes.html` does.
- **Alternatives:** none available here: no real GPU was used.
- **Rationale:** the brief forbids the shared browser pane. Someone should check fps on an Apple-silicon Mac.
