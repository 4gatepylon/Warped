# Dust

A playable demo of Dust, blue's deepest level (depth 2): a blocky desert in hyperbolic space H³ (curvature radius
R = 50 m) with a black hole and a wormhole, ray traced along the bent light paths. It is a first try at the README
checklist's first item for this level; the layout is a proposal (see [DECISIONS.md](DECISIONS.md)).

## The level

Blue's spawn and pylon are at one end of a 216 m axis, red's at the other. The middle zone (the middle 110 m) has three
routes, separated by a long ridge (the Spine) and a row of buttes:

- **The Salt Flats** (centre): open, long sight lines, a few boulders, buried columns and the Fallen Head. The black
  hole hangs 16 m over its middle.
- **The dry riverbed** (blue's right): a channel between 5 m sandstone banks, with ramps up the banks and two rock arches
  that bridge it. **The Eye**, a tunnel through the Spine, joins it to the flats.
- **The sunken ruins** (blue's left): a bath house, a colonnade, a climbable ziggurat and a gatehouse. The gaps between
  the buttes join them to the flats.

Blue's pylon stands in a walled courtyard with four gates and a wall walk (defensible); red's stands in an open ring of
standing stones. Each pylon has 5 consoles. The zones are marked by tinted sand, blue and red stripes across the level at
the zone edges, and banners. Cliffs 19–28 m tall close the level in.

The wormhole runs from the flats near red's side (mouth A, between two obelisks) to 25 m in front of blue's pylon (mouth
B): red's fastest way in, but its exit is in the open, under blue's walls.

## What to try

- `3` (overview): from 52 m up, the whole infinite desert fits in a disk below you, and a circle of radius 100 m is 1.1 km
  long rather than 630 m.
- `1` (blue spawn): the middle of each side route looks only 9° off the centre line (22° in flat space), but walking it
  is 35–40% longer than crossing the flats.
- `4`, `5`: the black hole wraps the far end of the flats into a ring around its shadow. Fire (`R`) just under it, then
  fly (`H`) round to watch the beam swing round the shadow before it lands.
- `6`: the wormhole's disk shows blue's courtyard 111 m away. Walk into it, or shoot through it.
- `T` to play red, take the wormhole and hold `E` at blue's crystal, facing it: five consoles, 10 s each.

## Controls

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (energy drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode (no gravity, no collisions, at most 500 m from the centre) |
| `E` (hold) | deactivate a console of the other team's pylon (within 4.5 m of its axis, facing it) |
| Click (captured) or `R` | fire the laser |
| `T` | play as the other team (moves you to its spawn) |
| `1`–`9` | viewpoints (also buttons in the panel) |

## How it works

- **Space.** Points are unit vectors of the hyperboloid model, distances in units of R. The ground is the geodesic plane
  y = 0. Layout positions are Fermi coordinates: s along the level's axis, t across it, h above the ground.
- **Blocks.** Every block belongs to a structure: a voxel grid in that structure's own Klein (projective) coordinates,
  placed by an isometry. Klein faces are geodesic planes and geodesics are straight lines, so a ray step is mapped into a
  structure's frame and walked through its grid with an ordinary voxel DDA. The cliffs, ridge and riverbed are one
  terrain function of (s, t), voxelized into 53 chunk structures; buildings, rocks and dunes are built block by block.
- **Light.** Each pixel's ray is traced backwards. Outside the warp objects' balls of influence it is a single geodesic,
  tested against the ground (one plane test) and against structures (bounding balls, nearest first, then the DDA).
  Inside a ball it bends as x'' = x + P(∇ln n) (the hyperboloid form of a ray in a medium of index n), integrated with
  midpoint steps of 0.18 times the distance to the object. The index is the flat demos' isotropic one (black hole:
  n = (1 + r_s/4ρ)³ / (1 − r_s/4ρ); wormhole: the "black hole foil" of `wormholes.html`), with its gradient scaled by
  the H³ falloff (ρ / R sinh(ρ/R))² and faded to zero at 34 m (black hole) and 16 m (mouths). A ray that reaches a mouth's
  throat is moved to the other mouth by a log map, inversion in the throat sphere, the gluing flip and an exp map.
  The sky is painted at infinity and seen from the camera, so lensed sky (the Einstein ring) has no seams.
- **Accuracy.** The ray steps were checked against a 100× finer integration: direction errors of 0.02° at 10 r_s,
  0.1° at 6 r_s, 1.2° at 3 r_s and 2.4° at the photon ring. The DDA agreed with brute-force sampling on 315 of 315
  random rays (within 3 cm). The black hole's shadow has a radius of 5.6 m (2.3 r_s), the wormhole's disk 2.9 m.
- **Laser.** The shot is traced on the CPU along the same light path from the eye along the crosshair, through the
  wormhole, until a block or the ground (max 450 m). The ray tracer draws the beam's glow wherever its light reaches you,
  so it is lensed too. Its first 4 m are drawn from the gun so you can see it leave. It fades over 2 s.
- **Walking.** Gravity pulls toward the ground plane along its normal (30 m/s², as in the laser gravity demos);
  collisions use the same Klein grids. Within 8 m of a wormhole mouth your walking direction bends like light, so you
  walk in where you see the disk. The black hole doesn't pull you, and fly mode stops you 6 m from it.
- **Precision.** Hyperboloid coordinates grow like cosh(d/R) and the GPU computes in single precision, so in the
  level's own coordinates everything near you lost its last digits from about 300 m out: the world shook and the sky
  turned to noise. The shader works relative to a render anchor instead: the level's centre while you are within 100 m
  of it, otherwise the ground point under you, moved each time you get 200 m from it. The CPU hands it the camera,
  structures, warp objects, beams and sky relative to the anchor, in double precision. Fly mode stops 500 m (10 R)
  from the centre, where double precision on the CPU is still exact and the level is a few pixels across.

## Running

```bash
python3 -m http.server --directory demos
```

Then open `http://localhost:8000/levels/dust/index.html`. The demo loads three.js from a CDN.

Hash parameters: `#view=<name>`, `quality=low|medium|high|full` (default auto: about 380k pixels, lowered while
below 40 fps), `panels=off`, `team=red`, `test=laser` (fires once after load); combine them with `&`. `window.__demo`
exposes the state and `set_view`, `fire_laser`, `trace_light_path`, `step_player` and more.

Views: `blue_spawn`, `red_spawn`, `overview`, `middle`, `black_hole`, `wormhole`, `riverbed`, `ruins`, `blue_pylon`.

## Known limitations

- Blocks are cubes only near a structure's centre; at the edge of a big structure they are up to ~20% bigger (the
  Klein chart's stretch), so steps are allowed up to 1.2 m. Neighbouring terrain chunks can't line up in H³, which shows
  as faint seams on the cliffs when seen from high up.
- The warp objects' bending is a model: the flat index near the object, cut off at a finite radius. Several objects'
  effects are added. Near the photon ring the image has errors of a few degrees.
- Walking through the wormhole bends only your horizontal heading; off-centre jumps can miss the throat.
- No shadows; the sun's direction is fixed in each structure's frame. Not tested on a real GPU: in SwiftShader a frame
  costs about a third of `wormholes.html`'s at the same resolution, so it should hold 30+ fps at the auto default.
