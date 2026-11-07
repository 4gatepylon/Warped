# Dark Forest

Red's depth-1 level: a dark forest planet at dusk in T² × ℝ, flat space whose two horizontal axes loop back around
while up goes on forever. The loops are 112 m widthwise (x) and 160 m lengthwise (z, spawn to spawn and back). Light
loops too: from above the canopy you see copies of the level repeating to the horizon, and at ground level the fog hides
all but the nearest. Two black holes hang over the open areas, and three wormholes are the fast routes worth fighting
over. Red levels are themed on bases, so the red zone is a fortified camp of bunkers, a walled pylon courtyard and
watchtowers lit red.

`index.html` is the whole demo: a ray-traced voxel level with walking, a lensed laser and both pylons.

## The layout ("paired bands")

Red spawns at z = 0 and blue halfway round, at z = 80. Going all the way round in z passes every zone twice:

| z (m) | Zone | Routes, by x |
| --- | --- | --- |
| 144 to 16 | red zone (its halves meet at red's spawn) | open widthwise: a road all the way round in x, the spawn bunker (x 28–37), the walled pylon courtyard (x 48–64), four watchtowers, bunkers and walls, red moss and red lights |
| 16 to 64 | middle, **Glade side** | west trench (x 10–26), the **Glade** (open area, x 38–74, black hole overhead), east trench (x 86–102) |
| 64 to 96 | blue zone | the red zone turned by 180°, in blue |
| 96 to 144 | middle, **Bog side** | the **Bog** (open area, x 90 through 0 to 22, black hole overhead), the forest floor with the **canopy walkway** 9 m above it (x 38–74) |

- **Routes:** rocky ridges 5–9 m high, covered in trees, separate them. The trenches are 2 m deep, with plank walls,
  ledges to shoot down from and concrete bunker rooms you walk through. The walkway joins four tree platforms with plank
  bridges, with stairs down at each end.
- **Connectors:** a gap in the ridge joins each trench to the Glade (west trench near red's side, east trench near
  blue's). Stairs from the walkway lead down through a root tunnel into the Bog.
- **Symmetry:** the level is built for red's half and turned by 180° about (56, 40) for blue's half, with the team
  colours swapped. So each team sees the same level, but the two middle bands differ from each other.
- **Wormholes:** the *Red Glade–Bog gate* joins the Glade (44, 32) to the Bog (12, 128), both on red's side of the
  middle, so red can switch between its two ways to the other side. The *Blue Glade–Bog gate* (68, 48) ↔ (100, 112) is
  its blue twin. The *Trench gate* jumps 20 m forward and across, from the west trench near red (18, 30) to the east
  trench near blue (94, 50). Each team's entrance is the other team's exit. Walk straight in and you come out still
  heading toward the enemy.
- **Black holes:** over the Glade (56, 10.5, 40) and the Bog (0, 10, 120). They bend the sight lines across the open
  areas and glow with an accretion disk that lights the ground. They float out of reach and don't pull on players.

## What to try

- **The loops.** Walk along the road in x and you come back after 112 m. From view 3 (overview) or `#view=loop`, see
  the copies of the level: the black holes repeat every 112 m and 160 m.
- **Look across the Glade** (view 4): the black hole bends the trees behind it into rings, with a black shadow, a
  glowing disk and an Einstein ring.
- **Go through a gate** (view 6 or 7): the mouth shows the world around the other mouth. Walk into its middle with `W`.
  Within 3 m of a mouth you follow the light ray you are on, so you end up where the crosshair points. Enter off-centre
  and you come out deflected, as light does.
- **Shoot the laser** (`R`) through a gate or past a black hole and watch it bend, from the side. Add `&test=laser` to the
  URL to fire one shot at load and keep its beam on screen.
- **Capture:** `#view=blue_pylon`, then hold `E` facing the pylon: 10 s per console, 4 consoles.

## Controls

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (energy drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode |
| `E` (hold) | deactivate a console of the other team's pylon (within 4.5 m of its axis, facing it) |
| Click (captured) or `R` | fire the laser |
| `T` | play as the other team (moves you to its spawn) |
| `1`–`9` | viewpoints: red_spawn, blue_spawn, overview, middle, black_hole, wormhole, trench, canopy, bog |

## How it works

- **Space:** the voxel grid is one period of the torus (112 × 32 × 160 blocks, 1 block = 1 m); below it is bedrock and
  above it endless air. A ray keeps unwrapped coordinates and reads the grid with the cell wrapped into the period, so
  light that leaves one side comes back in on the other. Players are wrapped into the period every step.
- **Warp objects** are refractive indices in isotropic coordinates, so light bends by the gradient of ln n:
  - **Black holes:** n = (1 + u)³ / (1 − u), with u = ρ_h / ρ and ρ_h = r_s / 4. This is exact Schwarzschild.
  - **Wormhole mouths:** the "black hole foil" of `three_js_vibecode_demo/wormholes.html`,
    n = (1 + ρ_t/ρ)² / √A. A ray inside a throat is inverted through it and glued to the partner mouth by
    Q = F_B · diag(1, 1, −1) · F_Aᵀ.
  - **Periodic copies:** each object has a copy in every period. A ray feels only the nearest copy (minimum image), with
    the bending faded to zero between 28 and 44 m, so there is never a sum over copies. Different objects' bending is
    added.
  - **Check:** close passes match the exact Schwarzschild deflection at an impact parameter 1.028 times larger (that is
    n where the fade starts), within 0.3–1.2% for b = 3–6 m.
- **Rendering:** a WebGL2 fragment shader marches each pixel's ray from the eye. It walks the voxel grid cell by cell
  and bends the ray with the midpoint method. A disk crossing near a black hole adds glow. Fog thins with height. The
  laser's glow is tested against each piece of the ray, taking the nearest copy of each beam chunk.
- **Lighting** is Minecraft-style light, computed once in JavaScript:
  - **Channels:** sky light (dimmed by leaves, so the forest floor under canopies is dark) and two block-light channels.
    The warm one comes from lamps, red lights and the red pylon; the cool one from blue lights, the blue pylon and
    glowing mushrooms, plants and fungus.
  - **Sampling:** the light is a 3D texture sampled with trilinear filtering half a block in front of each face, which
    gives smooth light and soft corners.
- **Walking:** gravity 25 m/s², jump 8.2 m/s (a 1.3 m hop), walk 6 m/s, sprint 9.5 m/s. You collide with blocks, step up
  single blocks, and can't get closer than 2.2 m to a black hole.
  - **Through a wormhole:** within 3 m of a mouth and moving toward it, you follow the light ray along your velocity, at
    your speed in proper distance and without gravity. The view turns with the path and levels out afterwards.
- **Laser:** traced on the CPU with the same optics (RK4, 0.25 m steps, finer near objects). It goes through throats
  and across the loops, and stops at a block, a horizon or after 320 m. It shows once and fades over 2 s.

## Running

```bash
python3 -m http.server --directory demos
```

Then open `http://localhost:8000/levels/dark_forest/index.html`. three.js loads from a CDN.

- **Views (`#view=`):** `red_spawn`, `blue_spawn`, `overview`, `middle`, `black_hole`, `wormhole`, `trench`, `canopy`,
  `bog`, `red_pylon`, `blue_pylon`, `loop`.
- **Other hash parameters:** `quality=low|medium|high|full` (default auto), `team=blue`, `panels=off` and `test=laser`.
  Combine them with `&`, e.g. `#view=overview&quality=medium`.
- **Scripting:** `window.__demo` exposes the state and functions such as `set_view`, `fire_laser`, `trace_laser`,
  `step_player` and `switch_team`.

**Speed** (measured with Chrome 156 headless using ANGLE Metal, on an Apple M3 Pro running macOS 15.7.3, in a 1280 × 720
window):

- **Auto quality** (about 320k pixels, lower while under 40 fps): 43–60 fps across the views.
- **Heaviest view:** next to a black hole, where auto quality steps down to about 240k pixels.

## Known limitations

- **Gameplay:** there are no other players, bots or damage. Respawning, heat and the rifle aren't modelled.
- **Approximations in the optics:**
  - Several warp objects' bending is added, which is approximate where two are within about 30 m of each other (the
    Glade's black hole and its two gates).
  - The fade makes a black hole's shadow about 3% smaller than an isolated one's.
- **No shadows:** the black holes' disk light is scaled by the sky light rather than shadowed, and block light comes from
  the flood fill, not from rays.
- **Collisions:** leaves are solid. Railings are one block high, so you can step onto them.
- **Laser through a gate:** it starts 0.3 m off the eye line (the gun), and the throat magnifies that offset. A shot aimed
  at a mouth's centre comes out a little deflected.
