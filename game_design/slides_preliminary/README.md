# Preliminary design slides

This is the preliminary design deck, from before the game was built, kept for history. Some of its rules
and numbers have changed since. The official deck, which describes the game as it is, is
[`../slides/`](../slides/README.md).

`index.html` is a slide deck, in HTML and SVG, of the design in [`../INSIDE_GAME_DESIGN.md`](../INSIDE_GAME_DESIGN.md) at the time:

1. The map: five levels and their spaces.
2. The wavefront: which level is played after each round, with an example match.
3. Pylons and consoles: how a level is won, and console counts per level.
4. Respawns and sprinting: the respawn wait over a round, and sprint energy through sprints, rests and a kill.
5. Zone templates: flat, inside a sphere, and T² × ℝ.
6. An example layout: a sketch of Snow Planet.
7. Look and feel: reference screenshots of other shooters.
8. Weapons: the sniper rifle and the rifle.
9. Shooting in curved space: laser paths near a black hole, inside City Sphere, and in hyperbolic space.
10. The three.js demos, and a slot for a video.

## Viewing

Open `index.html` in a browser; it needs no server. Arrow keys, Space and Page Up/Down move between
slides, and `#3` in the URL opens slide 3. Printing gives one PDF page per slide. The reference
images on slide 7 load from Wikimedia Commons, so they need a network connection.

## Adding your own images and video

Some slots stay empty until you add a file:

- Slide 7 has tiles for Counter-Strike 2, Halo Infinite, Splitgate and Destiny 2. Their screenshots
  aren't freely licensed, so none are included. Add one at the path shown on its tile, such as
  `images/references/counter_strike_2.jpg`.
- Slide 10 has a video slot that plays `videos/demo.mp4` once it exists.

## Demo screenshots

`images/demo_*.jpg` are screenshots of the demos in
[`demos/three_js_vibecode_demo/`](../../demos/three_js_vibecode_demo/README.md). Each was taken in
headless Chrome at 1600 × 900, with the demo's `.panel` overlays hidden and its **Laser fan** button
pressed, then resized to 1280 × 720.
