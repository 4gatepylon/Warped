# Maps

A map is a level's static layout: its space, ground, structures, warp objects, lights, sky and spawn points. It is
specified here once, in a format every client, server and demo can read whatever its language, so a level is designed
once and drawn and simulated the same way everywhere. Gameplay that changes during a match (players, lasers, pylons'
state) is not part of a map.

A map is a folder `common/maps/<name>/` holding `map.json`, a JSON manifest that [`map.schema.json`](map.schema.json)
describes. `validate_map.py` checks a map against this page (`make validate-maps`).

| Map | Made by | What it is |
| --- | --- | --- |
| [`house_demo/`](house_demo/) | `demos/hyperbolic_wormholes_rust` (`--export-map`) | the Rust wormholes demo's house, rails and pylons in H³ (R = 8 m), three wormholes |
| [`lunar_surface/`](lunar_surface/) | `demos/levels/lunar_surface/export_map.mjs` | the Lunar Surface level in H³ (R = 40 m), a crater, 25 smooth-shape structures, a black hole and a wormhole |

Both are exported from the programs that first built them; the map files are what other programs read.

## Conventions

- **Units:** meters, seconds and radians. Colours are linear RGB from 0 to 1.
- **Space** (`space`): `hyperbolic` (curvature radius R), `spherical` (radius R) or `euclidean`.
- **Points** are 4-vectors (x, y, z, w):
  - hyperbolic: on the hyperboloid x² + y² + z² − w² = −1, w > 0, in curvature units (a distance d meters is d/R), with
    the Minkowski product ⟨a, b⟩ = a_x b_x + a_y b_y + a_z b_z − a_w b_w;
  - spherical: on the unit 3-sphere x² + y² + z² + w² = 1, with the Euclidean product;
  - euclidean: (x, y, z, 1) in meters.
- **Frames** are 4×4 matrices, 16 numbers in column-major order, that map a local point to the world: Lorentz
  matrices (hyperbolic), rotations (spherical) or rigid motions (euclidean). A frame's origin is its last column. In
  every local frame +y is up, −z is forward and +x is right.
- **Local coordinates** of a local point p are its projective coordinates, scaled to meters at the origin:
  q = R · (p_x, p_y, p_z) / p_w (Klein coordinates in H³, gnomonic in S³), or q = (p_x, p_y, p_z) in E³. In all three
  spaces geodesics are straight lines in q and geodesic planes are planes, so a ray crosses a structure as a straight
  line in q. The map from q back to p is
  p = (q/R, 1)/√(1 − |q/R|²) in H³ and (q/R, 1)/√(1 + |q/R|²) in S³.
- **The ground** is the geodesic plane y = 0 of the world frame; the world origin (0, 0, 0, 1) is on it, and "up" at
  any point is the unit normal away from that plane.
- **Fermi coordinates** (u, v, h) of the world frame: the point on the ground at (u, v) is the origin of
  B_z(−u/R) · B_x(v/R), where B_a(s) is the boost (or rotation, in S³) by s along axis a; u runs along −z, v along +x,
  and h is the height above the ground. Regions (zones, boundaries) are given in them.

## The manifest

`map.json` is one object. Fields marked optional may be left out.

| Field | Meaning |
| --- | --- |
| `format` | always `"warped-map"` |
| `version` | the format's version, `1` |
| `name` | the folder's name, letters, digits and `_` |
| `description` | one line for people |
| `source` | the program that made the map (a path in this repository) |
| `space` | `{"kind": "hyperbolic" \| "spherical" \| "euclidean", "curvature_radius_m": R}` (no R for euclidean) |
| `gravity_m_per_s2` | the acceleration toward the ground |
| `player` | `{"eye_height_m", "walk_m_per_s", "sprint_m_per_s", "jump_m_per_s", "fly_m_per_s"}` |
| `materials` | the palette (below); material 0 is always empty space |
| `ground` | `{"material": id, "tiling": {...}, "terrain": {...}}` (optional `tiling` and `terrain`, below) |
| `structures` | structures of smooth shapes, optionally drawn as a model (below), optional |
| `boxes` | boxes (below), optional |
| `warps` | black holes and wormhole mouths (below), optional |
| `lights` | lamps (below), optional |
| `sky` | the sky and the sun (below) |
| `spawns` | where players start (below) |
| `regions` | zones and the boundary (below), optional |
| `appearance` | `{"style": name, ...}`: how the renderer details surfaces (below) |

### Materials

`{"id", "name", "color": [r, g, b], "kind"}`, ids from 0 to 255, each once. `kind` is `empty` (id 0 only), `solid`,
`glass` (see-through for light and lasers; stops players) or `light` (solid and glowing in its own colour, unlit).

### Structures

`{"name", "frame", "shapes": [...]}`: a solid made of smooth shapes, each
`{"kind", "centre_m": [c_x, c_y, c_z], "half_m": [h_x, h_y, h_z], "round_m", "axis", "material"}` in the frame's local
coordinates q (meters):

- `box`: the box |q − c| ≤ h per axis, its edges and corners rounded by `round_m` (default 0, at most the smallest
  half size).
- `cylinder`: a cylinder along local axis `axis` (0 = x, 1 = y, 2 = z; default 1), half length h along it and radius
  the next axis's h, its rims rounded by `round_m`.
- `ellipsoid`: the ellipsoid with semi-axes h centred at c.
- A ray or a player meets the structure where it enters any shape. `demos/hyperbolic_wormholes_rust/src/shapes.rs`
  defines the shapes' distance bounds exactly.
- `mesh` (optional): `{"model", "height_m"}` draws the glTF model `models/<model>/scene.gltf` (see Models) instead of
  the shapes, scaled evenly to `height_m` tall, with the bottom centre of its bounding box at the local origin, its
  +y up and its −z forward. Its triangles stop light and lasers; the shapes then only stop players, and a renderer
  that doesn't draw models draws them instead.

### Boxes

`{"name", "group", "frame", "half_size_m": [h_x, h_y, h_z], "material"}`: the points whose local point p satisfies
|⟨p, e_a⟩| ≤ sinh(h_a / R) for each axis a in H³ (the region within distance h_a of each of the frame's three
coordinate planes, whose faces are equidistant surfaces rather than planes), |p_a| ≤ sin(h_a / R) in S³, and
|q_a| ≤ h_a in E³. `group` names what the box is part of ("the house", "a tree"), for hit reports and for renderers
that cull groups at once.

### Warps

Each warp bends light (and, near a wormhole mouth, players walking along light paths) with a refractive index n of the
distance d to its centre, the origin of its `frame`:

```json
{"kind": "black_hole" | "wormhole_mouth", "name", "frame", "law": "isotropic_schwarzschild" | "foil",
 "schwarzschild_radius_m": r_s, "lambda_squared": λ², "radial_coordinate": "distance" | "flat_effective",
 "reach_m": L, "falloff": "taper" | "cutoff", "partner": index}
```

- **Radial coordinate** r: `distance` is r = d; `flat_effective` (H³ only) is r = (R/2)(e^{2d/R} − 1).
- **Laws**, with m = r_s/4 (the throat or horizon radius in r):
  - `isotropic_schwarzschild` (black holes): ln n = 3 ln(1 + m/r) − ln(1 − m/r). A ray that reaches r ≤ m has fallen
    in and is drawn black.
  - `foil` (wormhole mouths; `lambda_squared` is required): ψ = 1 + m/r, s = r ψ²,
    ln n = 2 ln ψ − ½ ln((1 − r_s/s + λ²) / (1 + λ²)).
- **Falloff** at the reach L: `taper` multiplies ln n by (1 − (d/L)²)² for d < L and makes it 0 beyond; `cutoff` uses
  ∇ln n as it is for d ≤ L and 0 beyond. Reach spheres of different warps may overlap only with `cutoff`, and then
  their ∇ln n add.
- **Light** follows the rays of the medium: with points x in curvature units, unit tangent v, arc length s in meters
  and ' = d/ds, x' = v/R and v' = x/R + (∇ ln n)⊥ in H³ (v' = −x/R + (∇ ln n)⊥ in S³; x' = v, v' = (∇ ln n)⊥ in E³),
  where ∇ ln n is per meter and ⊥ is its part across v.
- **Wormhole mouths** come in pairs (`partner` is the other mouth's index, each pointing at the other). A ray whose r
  drops below m inside a mouth is carried to its partner: in the mouth's local coordinates its direction from the
  centre û is kept, r becomes m²/r, the point is mirrored by G = diag(1, 1, −1, 1), and then mapped by the partner's
  frame; the ray's motion along û changes sign and its motion across û is carried by G.

### Lights

`{"position": [x, y, z, w], "color": [r, g, b], "radius_m"}`: a lamp. At distance d from it, a surface facing it at
angle θ gets `color` (already scaled by the lamp's brightness) times 1/(1 + (d/radius_m)²), times
0.3 + 0.7 max(cos θ, 0), faded smoothly to nothing between 1.8 and 3 times `radius_m`.

### Sky

`{"transport", "sun": {"direction": [x, y, z], "color": [r, g, b]}, "planet": {...}, "milky_way": {...}, "stars": bool,
"clouds": bool, "zenith_color", "horizon_color", "fog_m"}`:

- `transport` is how a ray that escapes finds its direction in the sky: `parallel` (the sky is fixed at the world
  origin like a skybox: the ray's last direction is carried by parallel transport along the geodesic to the eye, then
  along the geodesic from the eye to the origin, and read there; the sun then stands as high everywhere) or `ideal`
  (the ray's direction at infinity, where its geodesic meets the sphere at infinity, read in the world frame).
- `sun.direction` is a unit direction in the world origin's local frame.
- `planet` (optional): `{"direction": [x, y, z], "angular_radius_rad"}`, another body in the sky.
- `milky_way` (optional): `{"pole_direction": [x, y, z], "core_direction": [x, y, z]}`, where the galaxy's band lies:
  along the great circle 90° from `pole_direction`, brightest and widest at `core_direction`, a direction on that
  circle. Both are unit directions in the world origin's local frame, perpendicular to each other. How the band looks
  (brightness, colour, dust) is the appearance style's.
- `fog_m` (optional): a surface d meters away keeps e^(−d/`fog_m`) of its colour, and the rest is `horizon_color`.

### Sphere ground and shells

For maps inside a sphere (City Sphere), in Euclidean space with the origin at the sphere's centre:

- `ground.sphere` (optional): `{"radius_m"}` makes the ground the inside of the sphere of that radius instead of the
  plane y = 0. Gravity points outward, away from the centre, so a player's up is toward the centre.
- `shells` (optional): `[{"name", "radius_m", "thickness_m", "material", "walkable_side", "laser", "holes"}]`, thin
  spherical shells about the centre, `thickness_m` thick, centred on `radius_m`. `walkable_side` is which face
  players can stand on with gravity pulling them onto it (`inside`, the face toward the centre, `outside`, or `none`).
  `laser` is what a shot that meets the shell does: `stop`, or `reflect_inside` (a shot coming from the side toward
  the centre reflects about the shell's normal; one from outside stops). How many bounces and how far a shot goes are
  the game's settings, not the map's. `holes` are `[{"direction": [x, y, z], "angular_radius_rad"}]`: the shell is
  missing within that angle of the unit `direction`, so players and shots pass. A shell with `thickness_m` equal to its
  `radius_m` is a solid ball.

### Spawns

`{"team": "blue" | "red" | "any", "frame"}`: a player starts standing with their feet at the frame's origin (on the
ground or on a structure) and their eye `player.eye_height_m` above it, facing the frame's −z.

### Regions

`{"zones": [{"team", "u_min_m", "u_max_m"}], "boundary": {"kind": "fermi_ellipse", "u_m", "v_m"},
"zone_walls": {"height_m", "thickness_m", "opacity_frac"}}`, in the world's Fermi coordinates: a team's zone is a band
of u, and the playable area is (u/u_m)² + (v/v_m)² ≤ 1.

`zone_walls` (optional) marks each finite edge (`u_min_m` or `u_max_m`) of each blue or red zone with a wall in the
team's colour that stops neither players nor lasers. Its middle is the points straight above the edge (Fermi u equal to
the edge's, any v and h), from the ground (the terrain's height H(u, v), or h = 0 without terrain) up to `height_m`
above it, within the boundary when there is one. The edge is a geodesic of the ground, so in H³ (and S³) that middle is
part of a geodesic plane, and the wall is the slab of points within `thickness_m` / 2 of it. Seen straight through
its thickness, it is `opacity_frac` opaque at the ground (0 < `opacity_frac` < 1) and fades out toward its top;
renderers choose the fade and the glow.

### Ground tiling and appearance

- `ground.tiling` (optional): `{"kind": "square_order_5", "cells_per_side": N}` in H³: the ground is tiled by squares
  with 72° corners (five at each vertex), the world origin at a tile's centre and its edges along x and z, each tile
  split into N × N cells for texture.
- `ground.terrain` (optional; without it the ground is flat): `{"kind": "crater", "rim_u_m", "rim_v_m", "wall_m",
  "rim_height_m", "relief_m", "relief_wavelength_m", "seed", "mounds"}` raises the ground at Fermi (u, v) to height H(u, v) above
  h = 0: a crater wall rising over `wall_m` inward of the rim ellipse (semi-axes `rim_u_m`, `rim_v_m`; inward measured as
  (1 − e) · `rim_v_m`, e the ellipse's radius) to about `rim_height_m`, falling away outside, plus value-noise relief of
  up to `relief_m` at `relief_wavelength_m`, plus `mounds` (optional, at most 64): hills
  `{"u_m", "v_m", "radius_m", "height_m"}`, each adding `height_m` · (1 − d²)² where d < 1 is the distance from
  (`u_m`, `v_m`) over `radius_m`. `demos/hyperbolic_wormholes_rust/src/terrain.rs` defines H exactly.
- `appearance.style` names how a renderer details surfaces and the sky beyond what the map fixes: noise on blocks,
  craterlets, star fields, clouds. Renderers implement each style they support and draw any other plainly with the
  materials' colours. Defined styles: `meadow` (the house demo's grass, roads and block textures) and `lunar` (Lunar
  Surface's regolith, craterlets, ejecta, maria, stars and planet). `appearance.lighting` is
  `{"model": "ambient_sun", "ambient", "sun"}` (a constant plus Lambert sunlight) or
  `{"model": "lunar_lambert", "lommel_seeliger_frac", "ambient", "earthshine", "sun_intensity", "shadows": bool}`
  (that mix of Lommel–Seeliger and Lambert reflectance, with straight shadow rays toward the sun if `shadows`).

## Models

`models/<name>/` holds glTF models that maps draw (`scene.gltf`, `scene.bin`, `textures/`, `license.txt`), small
copies that `prepare_models.py` writes from Sketchfab downloads: same geometry, textures shrunk to 1024 px, spec-gloss
materials turned into metallic-roughness. It downloads into the git-ignored `.build/sketchfab/` and needs
`SKETCHFAB_API_TOKEN` only for models not downloaded yet:

```bash
SKETCHFAB_API_TOKEN=... uv run python prepare_models.py
```

All are licensed under [CC BY 4.0](http://creativecommons.org/licenses/by/4.0/) and need their authors credited:

| Folder | Model | Author |
| --- | --- | --- |
| `stekelvarken_kazemat/` | ["'Stekelvarken S3' Kazemat (Dutch WWII bunker)"](https://sketchfab.com/3d-models/stekelvarken-s3-kazemat-dutch-wwii-bunker-f409a77deaaf4e3888d47f76ec48dbdd) | [Sjoerd van Riel](https://sketchfab.com/sjoerdnijkerk) |
| `desert_bunker_garage/` | ["Desert Planet Bunker Garage Thing"](https://sketchfab.com/3d-models/desert-planet-bunker-garage-thing-ebe3177a1c7947b98328d8cab6aa8600) | [Santeri Riikonen](https://sketchfab.com/eKross) |
| `ww2_field_bunker/` | ["WW2 Field Bunker"](https://sketchfab.com/3d-models/ww2-field-bunker-6ea5cebdaa82455aa3aad26baa6e0375) | [Golden](https://sketchfab.com/goldenblack4) |
| `scifi_beacon/` | ["Sci-Fi Beacon/Way Point Marker Free Model"](https://sketchfab.com/3d-models/sci-fi-beaconway-point-marker-free-model-a1d91ebb3e2d41bba31c02b11423d97f) | [AMMediaGames](https://sketchfab.com/AMMediaGames) |
| `hesco_barrier/` | ["Hesco Barrier PBR"](https://sketchfab.com/3d-models/hesco-barrier-pbr-ab21217481094e9a84f989e71fd56a59) | [polbrainstorm](https://sketchfab.com/polbrainstorm) |

`teal_robot/` is the players' body, not part of any map: ["Teal V.2"](https://sketchfab.com/3d-models/teal-v2-d6a54ff9468c451bbdbd879afa03f20a)
by [Lagst](https://sketchfab.com/Lagst), under the [Sketchfab Standard](https://sketchfab.com/licenses) license (use
in any project, no redistribution of the model on its own; the repository is private). `prepare_models.py` runs
`animate_teal_blender.py` in Blender on it first (`--blender-path`), which gives it a clean rig and the clips `Idle`,
`Walk`, `Run`, `Jump` and `Death`.

## Checking a map

```bash
make -C common/maps validate-maps
make -C common/maps test
```

`make validate-maps` checks every folder here; `uv run python validate_map.py <folder>` checks one anywhere.

## Changing the format

A change that existing readers can ignore (a new optional field, a new style) keeps `version`; any other change
raises it, updates this page, the schema, the validator and every map and reader in the same stack of PRs.
