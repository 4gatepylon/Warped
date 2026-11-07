# City Sphere

`index.html` is a playable demo of City Sphere, the purple level (depth 0) of the default map in
[`game_design/DESIGN.md`](../../../game_design/DESIGN.md): the middle level, played exactly once, the
smallest, and built for close quarters. It is a city on the inside of a hollow sphere (radius 36 m).
Gravity pulls outward, so the ground curves up into the sky all around you and the far side of the
city hangs overhead. A smaller glass sphere (radius 28.8 m) is the second floor: it roofs every
street, and on it you stand on the inside of the glass and look down at the streets. A glowing core
sits at the centre. There are no warp objects, so light travels in straight lines.

## What to try

- **Look up** (view 1): the glass roof, the core, and the far side of the city upside down, seen
  through two layers of glass.
- **Walk down a street.** A level sight line meets the floor about 11 m ahead, and the glass roof
  stops any shot along the outer floor within about 40 m.
- **Climb to the glass floor** (view 7): the stairs go up through a hole in the glass, ringed by a
  railing on top.
- **Shoot at the red figure standing on the glass** above the plaza (view 4): you see it, but the beam
  stops at the glass. From the glass floor (view 6) the same shot hits it.
- **Deactivate red's pylon** (view 8): hold `E`, facing it, 10 s per console, four times.
- **The overview** (view 3) cuts the sphere open; drag to turn it.

## Controls

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look (in the overview, drag turns the cut-open sphere) |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (uses energy: drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode |
| `E` (hold) | deactivate a console of the other team's pylon (10 s each) |
| Click (captured) or `R` | fire the laser |
| `T` | play as the other team (moves you to that team's spawn) |
| `1`–`9` | jump to the viewpoints below |

## Viewpoints

`#view=<name>` starts at a viewpoint; `#panels=off` hides the panels; `#test=laser` fires one shot
after load. `#quality=` is accepted and ignored (this demo rasterizes). Example:
`index.html#view=overview&panels=off`.

| Key | Name | What it shows |
| --- | --- | --- |
| 1 | `blue_spawn` | blue's spawn pad, facing blue's pylon |
| 2 | `red_spawn` | the same at red's spawn, on the opposite pole |
| 3 | `overview` | the sphere cut open, looking down into blue's half |
| 4 | `middle` | the plaza on the equator, looking across into red's half |
| 5 | `alleys` | an alley corner, with a red figure |
| 6 | `deck` | on the glass floor above the plaza |
| 7 | `stairs` | the foot of blue's stairs, looking up through the hole in the glass |
| 8 | `red_pylon` | next to red's pylon, as blue, ready to hold `E` |
| 9 | `core` | flying near the core, looking across the sphere |

## Layout

- **Spawns and pylons.** Blue spawns at the bottom pole, with its pylon 12 m away; red's spawn and
  pylon are that segment reflected through the centre.
- **Zones** are bands round the spawn axis: blue's within 45° of its spawn, red's within 45° of its
  own, and the middle zone between them (71% of the floor). The team zones have tinted floors,
  team-coloured trim on their buildings, tinted glass above them, and a glowing stripe on the border.
- **Routes across the middle zone:** the **plaza** (a 40 × 36 m open area on the equator, with
  kiosks for cover and stairs to the glass), the **alleys** (2 m wide alleys with corners, on the
  opposite side of the sphere), and the **glass floor** above everything. An 8 m wide street round
  the equator is the connector between the plaza and the alleys. Each team zone has a ring road
  round its spawn, so a team can switch routes quickly, and its own stairs to the glass floor.
- **Symmetry.** The map is drawn once, for blue's half, as an unfolded net of the cube-sphere grid
  (`NET_ROWS`, one character per 2 × 2 blocks). Red's half is blue's half turned 180° about an axis
  through the equator, so both teams play the same map (not a mirror image).

## How it works

- **Blocks on a sphere.** The sphere is cut into a cube-sphere grid: each face of a cube is projected
  onto the sphere with equal angles per cell, 56 cells along each edge, about 1 m on the outer
  floor. Radially, layer k spans R e^-(k+1)Δ to R e^-kΔ (Δ = 90°/56), so the layers thin toward the
  centre at the same rate as the cells narrow and every block stays cube-shaped: 1 m on the outer
  floor, 0.8 m on the glass. Each block is a piece of a spherical shell; the glass floor is the
  boundary of layer 8.
- **Gravity and walking.** Gravity is a constant 30 m/s² outward. Your feet are a point x; up is
  −x/|x|. Walking moves x along the sphere of its radius and re-projects your heading onto the new
  tangent plane, so "forward" is carried along as you walk round. The body is a square column whose
  corners lie on radial lines, checked against the blocks at a few heights; walking into a block up
  to 1.05 m high steps onto it, and walls slide you along the grid axes.
- **Laser.** A straight line from the eye, marched in 5 cm steps and refined by bisection, that stops
  at blocks, the glass (a 0.2 m shell), the core, the ground or a figure. The beam is drawn from the
  gun to that point and fades over 2 s.
- **Rendering.** Plain three.js rasterization: one merged mesh of every exposed block face, with
  per-block noise, darkened edges, lit and dark windows, baked face shading and corner ambient
  occlusion, and distance haze; the glass is a transparent mesh with a frame every 2 × 2 blocks. The
  overview moves the camera outside the sphere and discards everything on its side of a plane
  through the centre. The level is about 100,000 triangles in 8 draw calls.

## Running

```bash
python3 -m http.server --directory demos
```

Then open `http://localhost:8000/levels/city_sphere/index.html`. three.js loads from a CDN.

## Known limitations

- The figures are static targets; there is no damage model, fire rate or overheating.
- The core and the windows glow, but nothing casts light or shadows: shading is baked per face.
- The glass floor has long sight lines across the inside of the sphere (up to about 55 m); only the
  core blocks the lines between opposite sides.
- The frame rate was only checked in headless Chrome with software WebGL (SwiftShader), not on a GPU.
