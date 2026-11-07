# Design slides

`index.html` is a slide deck, in HTML and SVG, of the design in [`../DESIGN.md`](../DESIGN.md):

1. The map: five levels and their spaces.
2. The wavefront: which level is played after each round, with an example match.
3. Pylons and consoles: how a level is won, and console counts per level.
4. Zone templates: flat, inside a sphere, and T² × ℝ.
5. An example layout: a sketch of Snow Planet.
6. Look and feel: reference screenshots of other shooters.
7. Weapons: the sniper rifle and the rifle.
8. Shooting in curved space: laser paths near a black hole, inside City Sphere, and in hyperbolic space.
9. The three.js demos, and a slot for a video.

## Viewing

Open `index.html` in a browser; it needs no server. Arrow keys, Space and Page Up/Down move between
slides, and `#3` in the URL opens slide 3. Printing gives one PDF page per slide. The reference
images on slide 6 load from Wikimedia Commons, so they need a network connection.

## Adding your own images and video

Some slots stay empty until you add a file:

- Slide 6 has tiles for Counter-Strike 2, Halo Infinite, Splitgate and Destiny 2. Their screenshots
  aren't freely licensed, so none are included. Add one at the path shown on its tile, such as
  `images/references/counter_strike_2.jpg`.
- Slide 9 has a video slot that plays `videos/demo.mp4` once it exists.

## Demo screenshots

`images/demo_*.jpg` are screenshots of the demos in
[`demos/three_js_vibecode_demo/`](../../demos/three_js_vibecode_demo/README.md). Each was taken in
headless Chrome at 1600 × 900, with the demo's `.panel` overlays hidden and its **Laser fan** button
pressed, then resized to 1280 × 720.
