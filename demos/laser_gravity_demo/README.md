# Laser + gravity demos

The three laser demos from `../three_js_vibecode_demo` (`blackholes.html`, `hyperbolic.html`, `spherical.html`), with
gravity added for you and for the laser. The laser controls are the same: `L` toggles it, click (with the mouse captured)
or `R` leaves an emitter, `V` fires a fan of emitters from 40° up to 20° down and moves you aside to watch, `Z` removes
emitters.

## How gravity works (the defaults)

- **Direction:** toward the ground under you, along the shortest way down. In the black-hole world that is straight down.
  In hyperbolic space it is toward the ground plane. In spherical space it is toward the ground sphere under your feet,
  not the ground you see overhead on the far side. From below the ground it pulls back up. At the spherical pole (the
  one spot that is equally far from the ground in every direction) there is no "down", so it vanishes there.
- **Strength:** Earth's, 9.8 blocks/s² (1 block = 1 m), the same everywhere.
- **You** fall, land on blocks, climb single-block steps and can't walk through walls. `Space`/`E` jumps, and holding it
  works as a jetpack. `H` toggles fly mode (no gravity for you), which buttons like "Fly way up" switch on. Black holes
  don't pull on you, so you can walk right up to one.
- **The laser:** real light falls by less than an atom's width over a kilometre, so the effect is exaggerated: the beam
  falls as if it had been fired at 20 blocks/s. A level beam starts curving down along a circle of radius about 41 blocks.
  Gravity only turns the beam, never slows it, so a shot aimed up rises, curves over and ends up heading straight down.
  The "Gravity on the laser" slider scales this from 0 (off) to 3×.
- **What you see:** light from the scene is not pulled down (it is only bent by black holes, as before), so the world looks
  the same as in the plain laser demos. Only the beam falls.

## What to look for

- **Black holes:** arcs that dive into a hole or swing around it. Holes still bend the beam exactly as before, on top of
  the downward pull.
- **Hyperbolic:** gravity fights the geometry. Without gravity a level beam never comes down, because the ground curves
  away from a straight line, and it curves away more strongly the higher you are. So level shots land when fired from
  below a certain height and sail off forever above it (1.6 blocks at R = 8, about eye height; the slider note shows it).
  Jetpack up and watch the live beam switch from landing to escaping.
- **Spherical:** gravity and the geometry pull the same way. With no gravity a level shot lands exactly πR/2 away from
  any height; with gravity it lands sooner. With the ground off, a beam falls through where the ground was, gets pulled
  back from the other side and swings back and forth without ever repeating. It would go on forever, so it fades and is
  cut off at 10% brightness (about 3.5 loops' worth of travel). With the gravity slider at 0 it instead closes into a
  loop after exactly 2πR, as in the plain demo.

## How the beam is computed

- **Black holes:** the same RK4 tracer as the plain demo, with a constant downward term added to the bending. The ray
  tracer then draws the beam with lensing as before.
- **Hyperbolic and spherical:** a beam with gravity is no longer a geodesic, so it is integrated step by step (RK4 on the
  hyperboloid or the 3-sphere: 0.25-block steps near things, 1 block elsewhere). Each step is checked against the ground,
  the walls and nearby block structures, then the beam is drawn as a ribbon through the traced points.
