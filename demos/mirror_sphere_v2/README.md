# Mirror sphere v2: solid or mirror wall

A version of the mirror sphere demo (`demos/mirror_sphere/`) where the wall is solid by default, which is much easier to
read. You still stand on the inside of a hollow sphere, pulled outward onto its wall, with a sun in the centre, pillars on
the shell and coloured orbs floating in between. The laser still reflects off the wall, and its power still drops linearly
with distance until tracing stops at zero.

## What's new

- **Solid wall** (default): stone tiles with a lat/long grid, lit by the sun with shadows, and a light distance haze for
  depth. It reflects a little (5–20%, slider, default 12%), blurred like a mirror with mist on it (slider for how blurred,
  0–25°, default 9°).
- **Mirror toggle** (checkbox or `M`): switches to the full mirror wall of the original demo.
- **Bounce dots:** each place the laser hits the wall shows a red dot, which makes the bounces easy to follow.
- The laser reflects perfectly off the wall in both modes, whatever the wall looks like. Use the "power lost per
  reflection" slider to make the wall absorb some of it.

## Controls

- `WASD` walk, `Shift` sprint, `Space` jump, `E`/`Q` jetpack toward / away from the centre.
- `L` laser on/off. Click (with the mouse captured) or `R` leaves an emitter that keeps firing, `Z` removes emitters.
- `M` mirror wall on/off.

## How the misted reflection works

Where a view ray meets the solid wall, it takes most of the wall's own colour, then sends five reflected rays spread over a
cone (the mist angle), each carrying a fifth of the reflectivity. Each pixel turns the pattern by a random angle, so the
blur shows as faint noise rather than ghost copies. Those reflected rays skip shadows and laser dots, which wouldn't show
at that strength. With the mist at 0 a single sharp reflected ray is used.

## Running

`python3 -m http.server --directory demos`, then open `http://localhost:8000/mirror_sphere_v2/`.
