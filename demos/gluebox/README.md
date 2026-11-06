# Gluebox

Build a universe by gluing together the faces of a cube, then fly around inside it.

This demo builds on the earlier demos in [`../three_js_vibecode_demo`](../three_js_vibecode_demo/README.md). It uses the same block world, single-player controls and three.js setup. Those demos curve space (a 3-sphere, hyperbolic space, black holes). Gluebox keeps space flat and changes its **topology** instead: what is connected to what.

You start in a menu where you choose, for each of the cube's six faces, what happens when you go through it:

- **wall**: it's solid.
- **mirror**: you, and light, bounce back as a mirror image.
- **glued to another face**: you come out of that face. The gluing can include a **twist** (quarter turns) and a **flip** (mirror image).
- **glued to itself**: the face is folded onto itself (along a midline or diagonal, or with a half turn about its centre).

Press **Enter the space** to go in. Press **Esc** to come back to the menu.

## Running it

Open `index.html` in a browser. Double-clicking it works, or you can serve the folder:

```bash
cd demos/gluebox
python3 -m http.server 8123
```

and open <http://localhost:8123>. three.js is loaded from a CDN, so you need to be online. WebGL 2 is required.

The code is a set of ES modules in `src/`, and browsers won't load those from a page opened straight from disk. In that case `index.html` runs `gluebox.bundle.js` instead: the same code joined into one ordinary script by `tools/build.mjs`. Served over http, the page always uses `src/` directly.

**After editing `src/`, run `npm run build`** so that opening the file from disk picks up your change. `npm test` fails if the bundle is out of date.

The current gluing is kept in the URL hash, for example `#t3` for a preset or `#10-00-30-20-52f-42f` for a custom gluing, so you can bookmark or share a space.

### Tests

```bash
cd demos/gluebox
npm test          # node --test, no dependencies
npm run build     # regenerate gluebox.bundle.js after editing src/
node tools/enumerate.mjs           # every way to glue the six faces in pairs (7,680 gluings, ~2 s)
node tools/enumerate.mjs --folds   # also with faces folded onto themselves (170,305 gluings, ~1 min)
```

The tests cover:

- the gluing algebra: face maps, inverses, folds, URL encoding;
- the portal maps, which must send each face onto its partner and the cube onto the neighbouring cube;
- the topology of every preset, checked against the known answers below;
- player movement through portals (you come back mirrored in Klein bottle × circle, upside down in the half-turn space, and are stopped by walls).
- that `gluebox.bundle.js` matches `src/`.

## Controls

| In the menu | |
|---|---|
| Preset list | load a known space |
| Face table | for each face choose *glued to*, *twist* and *flip*, or a *fold* when a face is glued to itself |
| Drag the cube | turn the preview |
| Options | cube size, fog, edge frame, tinted glued faces, showing yourself, render quality |

| In the world | |
|---|---|
| Mouse | look (click the view to capture the mouse, or drag) |
| <kbd>W</kbd><kbd>A</kbd><kbd>S</kbd><kbd>D</kbd> | move |
| <kbd>E</kbd> / <kbd>Space</kbd>, <kbd>Q</kbd> | up, down |
| <kbd>Shift</kbd> | sprint |
| <kbd>F</kbd> | auto-fly forward |
| <kbd>←</kbd> <kbd>→</kbd> | turn |
| <kbd>R</kbd> | respawn |
| <kbd>Esc</kbd> | back to the menu |

There is no gravity and no global "up". Once a gluing has turned you upside down, you stay that way, just as you would in the real space. The panel in the top right tracks this:

- how many faces you have crossed;
- whether you are currently a mirror image;
- which cube direction your "up" points along;
- the *holonomy*: the total rotation or reflection that the faces you crossed have applied to you.

## What you're looking at

- **The island.** The cube holds a small floating island. It is lopsided on purpose so that rotated and mirrored copies are easy to tell apart: the door faces south, the chimney is at the back right, and the axis marker is right-handed (red = +x, green = +y, blue = +z).
- **Copies of the island.** Every copy you see is the same island, seen along a different path through the glued faces. In the 3-torus they repeat on a grid. In twisted spaces the copies are rotated, and in non-orientable spaces they are mirrored.
- **You.** The little figure (red right side, blue left side) is you, seen by light that has gone through some faces and come back round. In the 3-torus, look along any axis and you see the back of your own head.
- **Glued faces** are faintly tinted with their pair's colour. The **F** decal marks matching points: the F on one face is the same set of points as the F on its partner.
- **Cube edges** are drawn as thin beams. Edges of the same colour are the same edge of the glued space, and the dark-to-light shading shows its direction. Dashed beams are *singular*: the cube corners that meet there don't add up to 360°.

## How it works

The code is in `src/`.

| File | What it does |
|---|---|
| `math/cube.js` | The cube, its faces, and its 48 symmetries as signed permutation matrices. |
| `math/gluing.js` | The gluing model, twist/flip ↔ matrix conversion, consistency checks, portal maps, URL encoding. |
| `math/topology.js` | Builds the quotient cell complex and computes its invariants. |
| `math/homology.js` | Smith normal form, giving H₁ over the integers. |
| `catalog.js` | The named presets, and how a gluing is recognised. |
| `world/scene.js` | The voxel island. |
| `world/space.js` | Portal maps in block coordinates; collisions through portals. |
| `world/player.js` | The body frame, movement, and carrying the frame through portals. |
| `world/shader.js`, `world/renderer.js` | The GPU ray tracer and its three.js setup. |
| `menu/menu.js`, `menu/preview.js` | The menu, and the turnable cube preview. |
| `main.js` | Switches between the menu and the world, and runs the game loop. |
| `../tools/build.mjs` | Joins `src/` into `gluebox.bundle.js`, for opening `index.html` from disk. |

### Gluings

Faces are numbered +x, −x, +y, −y, +z, −z (east, west, ceiling, floor, south, north).

A gluing of face *i* to face *j* is one of the 8 symmetries of a square. Every such map extends to exactly one symmetry *P* of the cube, so a gluing is stored as that matrix. Gluing *i* to *j* forces *j* back onto *i* with *P*⁻¹. A face glued to itself must use an involution, otherwise light wouldn't retrace its path.

Walking through face *i* applies the isometry

  h(x) = R_j(P x)

where R_j is the reflection in face *j*. This map sends face *i* onto face *j*, and sends whatever is just past face *i* to just inside face *j*. Its linear part is what gets applied to your body frame. If its determinant is −1, you come out mirrored.

*Twist* and *flip* are measured from a "natural" gluing:

- for opposite faces, straight across (a translation);
- for neighbouring faces, folding one onto the other about their shared edge.

### Rendering

Rendering is ray tracing, like the black-hole demo. Each pixel sends a ray backwards from the eye. Inside the cube the ray is a straight line, stepped through the voxel grid. At a glued face the ray's position and direction are mapped by h, and it carries on from the partner face. Walls stop the ray, and mirrors reflect it. So the picture is exactly what someone living in the glued-up space would see, including for gluings that don't fit together smoothly.

### Topology analysis

Each face is cut into 8 triangles by its midlines and diagonals. This way every face map, including folds, sends cells to cells. Identifying cells gives a cell complex for the glued space, and from it Gluebox computes:

- **χ**, the Euler characteristic;
- **H₁**, the first homology group with integer coefficients, from the Smith normal form of the boundary maps;
- **manifold or not**: the link of every vertex must be a sphere (or a disk on a wall);
- **orientability**: whether every gluing preserves handedness;
- **boundary surfaces** (walls and mirrors) and what they are;
- **the angle around every edge**: 360° everywhere means the space is genuinely flat.

A gluing is named if it is one of the presets up to rotating or reflecting the cube, all 48 ways. A closed flat manifold is also named from (orientability, H₁), because that pair tells the 10 flat 3-manifolds apart. Anything else is described by its invariants. If those invariants match a preset, Gluebox says so, but it doesn't claim the spaces are the same.

## Spaces you can build

Gluing codes list one token per face, in the order +x −x +y −y +z −z:

- `w` means wall and `m` means mirror;
- otherwise the token is `<partner face 0–5><quarter turns of twist 0–3>`, with an `f` added if the face is flipped.

The table values in this section were computed by Gluebox (`npm test` checks them), and they agree with the standard references.

### Flat universes (the platycosms)

There are exactly **10 closed flat 3-manifolds** (Hantzsche–Wendt 1935; Conway & Rossetti call them *platycosms*). In each of them, every small region looks like ordinary flat space. **One cube can make 9 of them.**

| Space | How to glue | Code | Orientable | H₁ |
|---|---|---|---|---|
| **3-torus** T³ | all three pairs straight across | `10-00-30-20-50-40` | yes | ℤ³ |
| **Half-turn space** (dicosm) | north–south with a half turn | `10-00-30-20-52-42` | yes | ℤ ⊕ (ℤ/2)² |
| **Quarter-turn space** (tetracosm) | north–south with a quarter turn | `10-00-30-20-51-41` | yes | ℤ ⊕ ℤ/2 |
| **Third-turn space** (tricosm) | neighbouring faces glued in pairs so the cube turns 120° about its long diagonal | `21-51-01-41-31-11` | yes | ℤ ⊕ ℤ/3 |
| **Hantzsche–Wendt space** (didicosm) | east–west with a half turn; floor ↔ north and ceiling ↔ south with half turns | `12-02-42-52-22-32` | yes | ℤ/4 ⊕ ℤ/4 |
| **Klein bottle × circle** (first amphicosm) | north–south with a flip | `10-00-30-20-50f-40f` | no | ℤ² ⊕ ℤ/2 |
| **Second amphicosm** | north–south flipped across a diagonal | `10-00-30-20-51f-43f` | no | ℤ² |
| **First amphidicosm** | floor–ceiling flipped, north–south half turn | `10-00-30f-20f-52-42` | no | ℤ ⊕ (ℤ/2)² |
| **Second amphidicosm** | every pair flipped, east–west across a diagonal | `11f-03f-30f-20f-50f-40f` | no | ℤ ⊕ ℤ/4 |
| ~~Sixth-turn space~~ (hexacosm) | needs a hexagonal prism | — | yes | ℤ |

Two of these usually get a different shape of building block. The third-turn space is normally made from a hexagonal prism, and the Hantzsche–Wendt space from two boxes. Both turn out to be possible with a single cube, as long as you glue *neighbouring* faces, which `tools/enumerate.mjs` finds.

The sixth-turn space is impossible: going around a loop in it turns you by 60°, and a cube has no 60° rotational symmetry.

### Curved spaces built from a flat cube

These closed manifolds don't have a flat geometry: their natural geometry is spherical, or S² × ℝ. Built from a flat cube, some edges have total angle 180° or 270° instead of 360°. The topology is still exactly right, and the ray tracer shows what the singular geometry really looks like: doubled or missing wedges near the dashed edges.

| Space | How to glue | Code | H₁ |
|---|---|---|---|
| **3-sphere** S³ | floor ↔ ceiling straight across; each side wall folded onto itself along its horizontal midline | `02f-12f-30-20-42f-52f` | 0 |
| **Projective space** RP³ | every face to the opposite face with a half turn (each boundary point glued to its antipode) | `12-02-32-22-52-42` | ℤ/2 |
| **Quaternionic space** S³/Q₈ | all three pairs with a quarter turn, same direction | `11-01-31-21-51-41` | (ℤ/2)² |
| **Sphere × circle** S² × S¹ | floor–ceiling and north–south straight across; east and west walls each folded | `02f-12f-30-20-50-40` | ℤ |

The search script also finds closed orientable manifolds with H₁ = ℤ/n for n = 3, 4, 5, 6, 7, 8, 10, 12 and 14. Lens spaces are natural candidates, but homology alone can't say which spaces they are.

### Spaces with walls

| Space | How to glue | Code | Boundary |
|---|---|---|---|
| **Solid torus** D² × S¹ | only north–south glued | `w-w-w-w-50-40` | torus |
| **Thickened torus** T² × I | east–west and north–south glued; floor and ceiling are walls | `10-00-w-w-50-40` | two tori |
| **Klein bottle × interval** | like the thickened torus, but north–south flipped | `10-00-w-w-50f-40f` | two Klein bottles |
| **Solid Klein bottle** | only north–south glued, flipped | `w-w-w-w-50f-40f` | Klein bottle |
| **Ball** | nothing glued | `w-w-w-w-w-w` | sphere |

### Mirrors and things that aren't manifolds

- **Kaleidoscope** `m-m-m-m-m-m`: six mirrors. Topologically it's just a ball. Geometrically it's an *orbifold* whose universe looks like the 3-torus, with alternate copies mirrored.
- **Inversion box** `02-12-22-32-42-52`: each face is glued to itself by a half turn about its centre. Eight points are pinched: the six face centres, plus the two points that the cube's corners collapse to. Each one's neighbourhood is a cone on the projective plane, not a ball, so this is not a manifold. Gluebox flags such points in the menu.

## Checking the guesses in the original request

- *"Identifying opposite sides with the same orientation gives a torus."* Yes: the 3-torus.
- *"Identifying them with opposing orientations in the right way gives projective space."* Yes, but in 3D the right way is a **half turn** on every pair, which is the antipodal map. In 2D, the projective plane comes from flipping both pairs of a square's edges. In 3D the antipodal map reverses *both* directions of each face, which is a half turn, not a flip. Flipping all three pairs (`10f-00f-30f-20f-50f-40f`) gives a different, non-orientable space with H₁ = ℤ ⊕ ℤ/2. A single flip on one pair gives Klein bottle × circle.
- *"Identifying all of them at one point is a sphere."* Yes, but it's the **3-sphere** S³ (the 3D version of the 2-sphere), not the ordinary sphere: cube / boundary ≅ S³. That collapse can't be walked through, because a whole face would have to shrink to one point. Gluebox makes S³ with an equivalent "zip" gluing instead: fold the cube's surface shut along its equator, the way you close a coin purse.

## References

- J. H. Conway, J. P. Rossetti, *Describing the platycosms*, 2003 — [arXiv:math/0311476](https://arxiv.org/abs/math/0311476)
- J. R. Weeks, *The Shape of Space*, 2nd ed., 2002 — gluing diagrams, and what living in these spaces looks like
- W. P. Thurston, *Three-Dimensional Geometry and Topology*, vol. 1, 1997 — gluing polyhedra, and when the result is a manifold
- [Platycosm / flat manifolds](https://en.wikipedia.org/wiki/Flat_manifold), [Real projective space](https://en.wikipedia.org/wiki/Real_projective_space), [3-sphere](https://en.wikipedia.org/wiki/3-sphere) on Wikipedia
