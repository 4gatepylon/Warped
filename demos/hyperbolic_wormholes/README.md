# Hyperbolic wormholes

Traversable wormholes in hyperbolic space (H³), the space of the Dust and Lunar Surface levels in
[`game_design/DESIGN.md`](../../game_design/DESIGN.md). It combines the two three.js demos in
[`three_js_vibecode_demo/`](../three_js_vibecode_demo/README.md): the block world in H³ (`hyperbolic.html`) and
the ray-traced wormholes in flat space (`wormholes.html`).

## What to look for

- **The rails.** Seven straight walls leave a straight cobblestone wall at right angles, 3 blocks apart. They fan
  apart exponentially, and the ground's square tiles meet five at each corner.
- **Mouth 1** (ahead on the left) leads to the house door. Like a black hole, it bends light around it, and where
  its shadow would be you see the far side.
- **Mouth 2** (behind you, by the blue pylon) leads to the red pylon, about 50 blocks down the rails. In H³ that
  far place is tiny and fogged; the wormhole puts it a few seconds' flight away, which is what long-range levels
  would use wormholes for.
- **Mouth 3** (far left) leads to a spot above the rails, looking down. From there the infinite ground is a disk
  and the rails fan out across it.

## Controls

- `W`/`S` fly where you look, `A`/`D` sideways, `E`/`Q` up and down, `Shift` fast, arrow keys turn. Click to
  capture the mouse, or drag.
- `F` keeps flying forward. `G` aims through the exact middle of the mouth nearest the crosshair: the throat
  magnifies small aiming errors, and an off-centre pass comes out tilted.
- `B` places a mouth 6 blocks ahead (press twice: entrance, then exit), `X` removes the wormhole nearest the
  crosshair, `C` clears them all. An entrance without an exit leads into itself.
- Sliders set the curvature radius R (5 to 14 blocks; applying it resets the scene) and the mouth size r_s.

## How it works

- **Geometry.** Points are 4-vectors on the hyperboloid ⟨x, x⟩ = -1 of the Minkowski product, and moves and turns
  are Lorentz matrices, as in `hyperbolic.html`. The camera is one such matrix.
- **Blocks.** Every solid is a hyperbolic box: the points within h_i of three perpendicular planes through its
  centre. Along a geodesic, with τ = tanh(distance), each of those conditions is a quadratic inequality in τ, so a
  ray meets a box exactly. A box that stands on the ground uses the ground as its middle plane, so a 50-block rail
  keeps the same height all along. The ground is the plane y = 0 with the {4,5} tiling: a hit point is folded into
  the central square by reflecting it across that square's edges.
- **Light.** The fragment shader traces each pixel's ray. Far from every mouth a ray is a single geodesic, tested
  exactly against the ground and the boxes. Within 2R of a mouth it takes short midpoint steps and bends by the
  gradient of ln n, as in `wormholes.html`.
- **Wormholes.** Each mouth uses the flat demo's lensing law: the "black hole foil" (Damour & Solodukhin 2007),
  with refractive index n(ρ) = (1 + ρ_t/ρ)² / √A and throat ρ_t = r_s/4. In H³ the radius ρ in that law is the
  effective radius (R/2)(e^(2d/R) - 1) at distance d from the mouth. Its inverse is the potential of a point mass in
  H³, (coth(d/R) - 1)/R, the hyperbolic version of 1/d. Close to a mouth it equals d, so the mouth looks like the
  flat one. Beyond R its pull fades exponentially, so the bending is cut off at 2R. A ray that gets inside a throat
  is reflected through the throat sphere and carried to the partner mouth with the mouth's back axis flipped, as in
  the flat demo. Several mouths' bending is added up, which is an approximation.
- **Precision.** The shader works in 32-bit floats, which lose distances between points far from the origin of
  their coordinates. So the scene is written once in the camera's coordinates and once in each mouth's coordinates
  (one row each of a float texture), and a ray that goes through a mouth continues in its partner's row.
- **Sky.** Directions come from the ray's point at infinity as seen from the camera. The elevation is stretched so
  that the ground's edge, which in H³ lies well below eye level, is the horizon.
- **Flying** follows light rays, as in `wormholes.html`: the path bends near mouths and goes through throats, and the
  view turns with it. `G` solves for the view direction whose ray crosses the throat at its centre.

"Auto" render quality draws about 380k pixels and lowers that while frames stay below 40 fps.

## Running

`python3 -m http.server --directory demos`, then open `http://localhost:8000/hyperbolic_wormholes/`.
