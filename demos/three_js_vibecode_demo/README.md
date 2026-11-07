This is the contents from https://claude.ai/share/eea2bc43-667d-4f29-887c-e328064f3356.

It should be possible to play these games but the code won't be super elegant.

`conversations.json` should contain the relevant conversation. It may contain spurious other stuff too.

## Laser versions

`blackholes_laser.html`, `hyperbolic_laser.html` and `spherical_laser.html` are copies of the three demos with a laser added.
A red beam shows where a laser fired from your hand would go. `L` toggles it. Click (with the mouse captured) or `R` leaves an
emitter that keeps firing, so you can walk around and look at its path. `V` fires a fan of emitters and moves you to watch from
the side. `Z` removes the emitters. The stats panel says what the beam hits and how far it travels.

- **Black holes:** the beam is traced along the same light-bending law as the renderer (RK4, matches exact Schwarzschild
  deflection angles). The ray tracer then tests each pixel's bent ray against the beam, so the beam is itself lensed: you can
  see it behind holes and in the Einstein ring. Beams stop at a block, at a horizon, or after 600 blocks.
- **Hyperbolic:** beams are straight (geodesics) and never come back. A beam that hits nothing fades slowly and is drawn out to
  10R, where it has shrunk to a point on the horizon. Aim level and it never comes down.
- **Spherical:** every straight line closes after 2πR. With the ground on, every beam lands within half a loop: a level shot
  lands exactly πR/2 away at any height. With the ground off, a beam that misses everything comes back to your gun from
  behind. It would loop forever, so it is drawn exactly once around, fading slowly so you can see which way it travels.

## Wormholes

`wormholes.html` is a flat block world with traversable wormholes, built on the black hole laser demo. Each mouth bends light
like a black hole of the same size. Where the black hole's shadow would be, you see the world around the other mouth, and
you, balls and laser beams can go through. The scene starts with two wormholes: one from the plaza to the house door, and one
from beside the plaza to a spot high above it, looking down.

- **Flying follows light rays.** `W`/`S` move along your view and the view turns with the path, so the crosshair shows where
  you will end up. `F` keeps flying forward. A throat magnifies small aiming errors a lot, so `G` turns you to aim through the
  exact middle of the mouth you are facing. Going in off-centre or from the side tilts you (the throat is a sphere, not a flat
  doorway), and the view then levels out.
- `B` places a mouth 6 blocks ahead: the first press places the entrance, the second its exit. Each mouth is entered cleanly
  along the direction you faced when you placed it. `X` removes the wormhole nearest the crosshair, `C` clears them all, and
  `T` throws a ball. The laser keys are the same as above.

How it works: each wormhole is a "black hole foil" (Damour & Solodukhin 2007): ds² = -A dt² + dr²/B + r² dΩ², with
A = (1 - r_s/r + λ²)/(1 + λ²), B = 1 - r_s/r and λ² = 0.25. Outside it is close to a black hole, but r = r_s is a throat to
another copy of the outside rather than a horizon. In isotropic coordinates light moves as if through glass with refractive
index n = (1 + ρ_t/ρ)²/√A, where the throat is the sphere ρ_t = r_s/4. Inversion in that sphere swaps the two sides. So rays
bend by the gradient of ln n, and a ray that enters a throat sphere is inverted through it and moved to the partner mouth
by an orthogonal map with determinant -1, which keeps handedness overall. Deflection angles match the exact integral
(checked to 0.03°). Several wormholes are combined by adding their effects, which is an approximation. Clocks at a throat
run at 45% of the far-away rate, so while you pass through the outside looks strongly blueshifted. Balls use game-scaled
gravity that is the same seen from either mouth, so a ball comes out as fast as it went in.

Speed: the ray tracer does more work per pixel than the black hole demo, because light that would end at a horizon here
carries on through the other mouth. "Auto" quality renders about 380k pixels and steps down while it runs below ~40 fps.

## Optics lab (flat space)

`optics/` is a ray-traced, flat-space lab for trying out optical elements with a laser. It has five demos, one page each
(`optics/index.html` lists them), and each adds one element to the previous one. The pages share `optics/lab.js` and
`optics/lab.css`, and also work when opened straight from disk:

- `optics/mirrors.html`: mirrors. The scene has a zigzag, a periscope, two facing mirrors and a corner reflector.
- `optics/splitters.html`: adds beam splitters (half-silvered mirrors). The scene has a 1-into-4 cascade and a
  Mach–Zehnder layout.
- `optics/diffraction.html`: adds diffraction gratings. Red, green, blue or white light; adjustable lines per mm.
- `optics/heat_haze.html`: adds heaters. Hot air rising off a heater bends a beam outward and upward, and its turbulence
  spreads the beam into a widening, soft glow. The effect is about 1000× stronger than real hot air.
- `optics/refraction.html`: adds glass blocks and prisms. Beams bend by Snell's law, partly reflect off every face, can be
  trapped by total internal reflection, and white light splits into a rainbow in a prism.

Click the scene to capture the mouse (or drag to look), `WASD` to fly and `E`/`Q` to go up and down. The laser controls are
the same as above (`L`, click or `R`, `Z`). `1`/`2`/`3`/`5`/`6` place a mirror, splitter, grating, glass block or prism
facing you. Aim at one and hold `T` (or the right mouse button) and move the mouse to turn it, or use `J`/`K`/`U`/`I` for 5°
steps. `X` deletes it. `4` puts a heater on the block you aim at, or takes away the heater you aim at.

Mirrors and splitters really reflect what you see, laser beams included. Splitters halve the light each way. Gratings send
order m to sin θ = sin θ<sub>in</sub> + mλ/d, with brightness from a thin sinusoidal phase grating (about 30% straight
through, 30% per first order, 4.6% per second). Limitations: beams are rays, so there is no interference; looking through a
grating shows only the straight-through view.

## Running

To serve them locally: `python3 -m http.server --directory demos/three_js_vibecode_demo`, then open e.g.
`http://localhost:8000/blackholes_laser.html`, `http://localhost:8000/wormholes.html` or
`http://localhost:8000/optics/`.
