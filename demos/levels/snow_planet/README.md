# Snow Planet

Blue's depth-1 level: a snow planet in T² × ℝ, flat space whose two horizontal axes loop back around while up goes on
forever. The loops are 176 m lengthwise (x, spawn to spawn and back) and 112 m widthwise (z). Light loops too, so
looking along an axis you see further copies of the level fading into the haze. The level is mid range, with two black
holes over its open areas and two wormholes that link its two ways across.

`index.html` is the whole demo: a ray-traced voxel level with walking, a lensed laser and both pylons.

## The layout ("paired bands")

The zones are bands across the lengthwise loop. Blue spawns at x = 0, red halfway round at x = 88, and going all the
way round passes every zone twice:

| x (m) | Zone | Routes, by z |
| --- | --- | --- |
| 156 to 20 | blue zone (its two halves meet at blue's spawn) | open widthwise: spawn, walled pylon courtyard, banners and a blue stripe at its edges |
| 20 to 68 | middle, **outpost way** | outpost huts (z 5–38), open snowfield with a black hole (z 44–74), rock ridge path (z 80–106) |
| 68 to 108 | red zone | mirror image of the blue zone |
| 108 to 156 | middle, **lake way** | ice canyon (z 5–38), frozen lake with a black hole (z 44–74), pine forest (z 80–106) |

Rock ridges separate the routes of each way. In each way a connector, a gap in the ridge at the band's centre, joins the
first route to the big open area. The level is built for blue's half and mirrored through x = 44 and x = 132, so each
route is the same seen from either team.

- **Wormholes:** W1 links the outpost courtyard (44, 15) to the ice canyon (132, 15), and W2 the top of the rock ridge
  (44, 93) to the pine forest (132, 93). They are fast ways to switch between the two ways mid-field, halfway between
  the spawns, so both teams reach them at the same time. A walk straight in comes out still heading toward the enemy.
- **Black holes:** over the snowfield (44, 7, 59) and the frozen lake (132, 7.5, 59). They bend the sight lines across
  the open areas. They float out of reach and don't pull on players.
- **Pylons:** each sits in a walled courtyard 19 m from its spawn, with 4 consoles (glowing team-coloured panels) on its
  four sides.
- **Ice** blocks (canyon walls and spires, base and hut windows) are translucent: you see through them, but you and the
  laser can't pass.

## What to try

- Viewpoint 3 (`overview`) shows the whole period from 85 m up, with the blue zone wrapping round both edges.
- Viewpoint 9 (`loop`) looks along the widthwise loop: the blue pylon repeats 112 m away.
- From blue's spawn, walk along the packed-snow path at z = 15 through the first hut and into W1 (or start at
  viewpoint 6, `wormhole`). You come out in the ice canyon on the lake way, still heading toward red.
- Fire the laser (`R`) past a black hole or into a mouth: the beam is drawn along its true path, and you see it lensed.
- Viewpoint 8 (`red_pylon`): hold `E` facing the pylon to deactivate its consoles, 10 s each.

## Controls

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (energy drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode (no gravity, no collisions) |
| `E` (hold) | deactivate a console of the other team's pylon |
| Click (captured) or `R` | fire the laser |
| `T` | play as the other team (moves you to its spawn) |
| `1`–`9` | viewpoints |

`#view=<name>` starts at a viewpoint: `blue_spawn`, `red_spawn`, `overview`, `middle` (snowfield, eye height),
`black_hole`, `wormhole`, `lake`, `red_pylon`, `loop`. `#quality=low|medium|high|full` fixes the render quality
(default auto), `#panels=off` hides the panels, `#test=laser` fires one shot after the first frame. Combine them with `&`,
e.g. `#view=overview&quality=low`. `window.__demo` exposes the state, `set_view`, `fire_laser`, `trace_laser`,
`step_player` and the other helpers.

## How it works

- **The space.** The voxel grid (176 × 24 × 112 blocks of 1 m) is one period of the torus. Every block lookup wraps x
  and z, so a ray that leaves one side of the grid comes back in on the other. Below the grid is rock; above it, sky.
- **Rendering.** The ray tracer of `three_js_vibecode_demo/wormholes.html`, extended. Each pixel's ray is marched
  backwards from the eye. Away from warp objects it jumps through empty space using a precomputed max-norm distance to
  the nearest block, so it crosses a whole empty box in one step. The ground layer is left out of that distance and
  handled as a plane, so rays skimming the snow still take long steps. Height fog (130 m at the ground, thinning with a
  24 m scale height) hides the far copies.
- **Black holes.** A ray turns by the Schwarzschild null-geodesic law written as a force, a = −(3/2) r_s |x × v|² x / r⁵,
  exact for one hole (as in `blackholes_laser.html`). Checked against the exact deflection integral: 72.3° vs 72.5° at
  b = 5 m, 22.2° vs 22.5° at 10 m (r_s = 1.5 m).
- **Wormholes.** Each is a "black hole foil" (Damour & Solodukhin 2007), as in `wormholes.html`. Light moves as if
  through glass of refractive index n = (1 + ρ_t/ρ)²/√A, and a ray that enters the throat sphere ρ_t = r_s/4 is inverted
  through it and moved to the partner mouth by an orthogonal map. All mouths face +z. The map then sends "toward red" on
  one way to "toward red" on the other.
- **Periodic copies.** Every warp object has a copy in each period. A ray feels only the nearest copy of each object,
  and that copy's bending fades to zero between 20 and 32 m. The effects of several objects are added.
- **You.** Walking, landing, single-block steps and the energy rules follow the laser gravity demos. Within 4 m of a
  mouth, your steps follow light paths instead (no gravity there), so you are drawn into the throat and through it, and
  your view turns with the path.
- **The laser** is traced once per shot on the CPU (RK4 with the renderer's bending, through throats). It stops at a
  block (ice included), in a horizon, or after 400 m, and fades over 2 s. The ray tracer draws it as a lensed glowing
  line, testing each ray step against the nearest periodic copy of each piece of the beam.

## Running

```bash
python3 -m http.server --directory demos
```

Then open `http://localhost:8000/levels/snow_planet/index.html`. three.js 0.160.0 loads from a CDN.

## Known limitations

- Far-field bending is cut off: rays passing 15 m from a hole bend 6% less than the exact value.
- Walking into a mouth more than about 2.25 m off its centre line deflects you rather than taking you through. Inside
  that, you come out turned by about 90° per metre you went in off centre (the path winds round the throat first):
  0.25 m off turns you 22°, 1 m off 92°. Only a walk straight in comes out straight.
- Fly mode isn't blocked by black holes: flying into a horizon turns the screen black until you leave (`1`).
- Snowfall is a screen overlay fixed to view directions, not part of the ray-traced scene.
- Frame rate was only measured relative to `wormholes.html` in headless Chrome on the CPU (it is cheaper per frame), not
  on a real GPU.
