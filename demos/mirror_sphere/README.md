# Mirror sphere

`index.html` puts you on the inside of a hollow sphere (radius 30 blocks by default, adjustable 12–60) whose whole inner
surface is a mirror. Gravity pulls you outward onto the mirror, so "up" always points at a sun in the centre and the far
side of the world hangs overhead. Pillars stand on the inside of the shell and coloured orbs float in between.

The laser reflects off the mirror every time it reaches it. Its power drops linearly with distance travelled (reaching zero
after the "beam range", 300 blocks by default), with an optional fixed loss per reflection. The beam fades as its power
drops, and tracing stops once the power reaches zero. Pillars, orbs and the sun absorb it.

## Controls

- `WASD` walk, `Shift` sprint, `Space` jump, `E`/`Q` jetpack toward / away from the centre.
- `L` laser on/off. Click (with the mouse captured) or `R` leaves an emitter that keeps firing, `Z` removes emitters.
- Sliders: beam range, power lost per reflection, sphere radius. Toggle for the etched grid on the mirror.

## How it works

- Everything is ray traced in one fragment shader: each pixel's ray reflects off the sphere up to 12 times, picking up a
  little matte light from the mirror (82% reflective) at each bounce. You therefore see the pillars, the sun and the laser
  reflected again and again.
- The beam is traced on the CPU. A beam reflecting inside a sphere stays in one plane through the centre, every chord is
  the same distance from the centre, and each reflection turns the chord by the same angle. The shader uses this to find
  the few chords near each pixel's ray without reading every segment.
- Each pixel ray's footprint is tracked through the reflections (the concave mirror focuses it and it spreads again), and
  small things (the sun, grid lines, beams) are blurred by it. That keeps deep reflections from sparkling.

## Game rules (not real physics)

- A real hollow shell has no gravity inside it (Newton's shell theorem). Here gravity pulls outward, in proportion to your
  distance from the centre, so it fades to zero at the centre. With the jetpack you can fly through the middle and land on
  the opposite side.
- Light travels in straight lines; gravity doesn't bend it.
- No collisions: you can walk through pillars and fly through the sun.

## Running

`python3 -m http.server --directory demos`, then open `http://localhost:8000/mirror_sphere/`.
