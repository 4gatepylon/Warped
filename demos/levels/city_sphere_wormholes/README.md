# City Sphere with wormholes

`index.html` is a playable demo of City Sphere, Warped's purple level (depth 0, the smallest), plus three traversable
wormholes. It isn't in the design doc yet. You walk on the inside of a hollow sphere, 36 m in radius, and gravity pulls
you outward onto its surface. A concentric glass sphere 7 m above the streets is the second floor. A glowing core
hangs at the centre. The city is close quarters: walls, corners and short streets.

## Layout

The blocks sit on a cube projected onto the sphere, so the six cube faces are six districts. The red half is the blue
half reflected through the centre, with the team colours swapped.

- **Blue zone:** the cap around the south pole (face −Y). Blue floor and blue glass. The blue pylon (4 consoles)
  stands at the pole in a plaza, and the spawn room is 21 m from it. A ring of streets leads to four exits into the
  middle zone.
- **Red zone:** the cap around the north pole, the antipode of the blue zone.
- **Middle zone:** the wide band round the equator (four faces, 2/3 of the sphere), with three routes:
  - **Street route A** (face +X) and **street route B** (face −X, on the opposite side of the sphere): each has a
    5-block avenue and a 3-block alley that zigzag from zone to zone, joined by two connectors.
  - **The skyway:** the glass floor above everything, a big open area with a little cover. Four stairwells join it to
    the streets (one per street route, one per team zone).
  - The other two sides of the band (faces ±Z) are sealed habitat blocks. You see their closed courtyards only
    from the glass.
- **Wormholes**, each mouth in a dark doorway with a cyan frame:
  - **Crosslink:** joins the equatorial cross streets of routes A and B. Walk east through it and you come out of the
    other route's gate, still walking east. Your own route's alley leads to the other route's avenue, and the
    other way round.
  - **Blue lift:** from the blue half of route A straight onto the glass above the red zone.
  - **Red lift:** its mirror image, from the red half of route B onto the glass above the blue zone.

  A lift is fast but risky. You arrive alone on glass that the defenders below see through, and you still have to
  come down their stairwell.

## What to try

- **Look up** (`1`, `4`). The floor curves up ahead of you, and the far side of the city hangs overhead around the
  core.
- **Go up the stairs** (`8`). On the glass you stand on the inside of the inner sphere and look down through it. The
  laser stops at the glass. Fire straight up from a street and the beam ends on the ceiling.
- **Look into a gate** (`5`, `6`). The dark disk shows the world around the other mouth, lensed. Fire into it and the
  beam's continuation shows inside the disk.
- **Walk through a gate.** You glide along a light ray until you are clear of the mouth, then gravity takes over and
  the view rolls back upright. Jump into one or go in off-centre and you come out tilted, as light does.
- **Win** (`9` puts you, as red, next to the blue pylon). Hold `E` facing the pylon, within 3 m of it: 10 s per
  console. Letting go resets the console you were on.

## Controls

| Key | Action |
| --- | --- |
| Click / drag | capture the pointer / look |
| `W` `A` `S` `D` | move |
| `Shift` | sprint (uses energy: drains in 30 s, refills in 60 s) |
| `Space` | jump; in fly mode, up |
| `C` | in fly mode, down |
| `H` | toggle fly mode (no gravity, no collisions; still goes through wormholes) |
| `E` (hold) | deactivate a console of the other team's pylon |
| Click (captured) or `R` | fire the laser (shows once, fades over 2 s) |
| `T` | play as the other team (moves you to its spawn) |
| `1`–`9` | viewpoints |

## Viewpoints (`#view=`)

`blue_spawn`, `red_spawn`, `overview` (near the centre, looking at the corner where the blue zone meets route A),
`middle` (route A's avenue), `wormhole` (6 m from the Crosslink gate), `lift` (5 m from the Blue lift gate), `skyway`
(on the glass above route A), `stairs` (foot of route A's stairwell), `blue_pylon` (as red, within reach of the blue
pylon).

Other hash parameters: `#quality=low|medium|high|full` (default auto), `#panels=off`, `#team=red`, and `#test=laser`,
which fires one shot at load and keeps it. Combine them with `&`, e.g. `#view=wormhole&quality=low&test=laser`.
`window.__demo` exposes the state and functions such as `set_view`, `fire_laser`, `step_player` and `debug_map`.

## How it works

- **Blocks on a cubed sphere.** Each cube face is split into 48 × 48 equal angles, and each column into 15 radial
  layers of 1 m (the inner floor's buildings, the glass, the 7 m outer room and the floor). A cell's walls are planes
  through the centre and spheres around it, so a ray crosses a cell with exact ray–plane and ray–sphere tests. Blocks
  stand upright everywhere and the floors are smooth spheres. A block is 1.18 m wide on the outer floor and 0.9 m on
  the glass.
- **One fragment shader ray traces everything.** A ray walks cell to cell. The glass tints it and lets it continue,
  with a sheen and panel frames. The empty interior is crossed in big steps, and the core is an analytic sphere.
  Light comes from the core, so tops of blocks are brightest. Each block has noise, darkened edges and some lit
  windows.
- **Wormholes** reuse `wormholes.html`'s model. Each pair is a "black hole foil" (Damour & Solodukhin 2007): in
  isotropic coordinates light moves through glass of refractive index n = (1 + ρₜ/ρ)²/√A. A ray bends by ∇ln n,
  and one that enters a throat sphere (ρₜ = r_s/4) is inverted through it and moved to the partner mouth. Here
  r_s = 0.7 m and λ² = 0.25, so a mouth's shadow is 1.46 m in radius. Its centre is at eye height (1.65 m) in a
  doorway 2 blocks wide and 4 m tall. Several mouths are combined by adding their bending.
- **Walking.** Gravity is a constant 25 m/s² outward, on both floors. The player is a 0.5 m × 1.8 m column that
  collides with blocks and glass, slides along walls and climbs 1-block steps. Jumping reaches about 1.1 m. Walking
  is 5 m/s and sprinting 8 m/s.
- **Going through a mouth.** Walk within 2.2 m of a mouth, aimed inside its shadow, and you ride the light ray:
  gravity is off, you move as a point at your walking speed, and the view turns with the ray. Clear of every mouth,
  you walk again with the ray's velocity. Gravity then pulls you down to the local floor. Over 0.35 s the view rolls
  back upright, and over 0.8 s it returns to the pitch you went in with.
- **The laser** is traced on the CPU with the same optics (RK4 in arc length). It stops at blocks, glass, the core
  or 300 m. The shader tests each pixel's bent ray against the beam, so the beam is lensed too, including through
  the throats.

## Running

```bash
python3 -m http.server --directory demos
```

Then open `http://localhost:8000/levels/city_sphere_wormholes/index.html`. three.js loads from a CDN.

## Known limitations

- The bending of several mouths is summed, which is exact only for one. Every mouth bends light across the whole
  level, by a few degrees 10–20 m away.
- Gravity is a game rule. A real shell has no gravity inside it.
- No shadows, and no damage model. The laser stops at the first block or pane of glass.
- Performance was checked only in headless SwiftShader, where a frame costs 0.25–0.7× a frame of `wormholes.html`.
  On a real GPU it should hold the "auto" quality's 40 fps target, but that is not measured.
- A doorway that is 2 blocks wide fills most of the shadow, so the Einstein ring shows mainly above the shadow and
  when you look at a gate from the side.
