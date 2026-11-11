---
name: level-creation-and-documentation
description: Make or update a level's world doc - its zone tree, named objects and marked map pictures in `game_design/worlds/<level>/` - from one source of truth (`source/map.png`, `source/world.yaml`, `source/template.md`) with `make generate-world-docs`. Use whenever you add a level, add, move, rename or remove an object or a zone in a level, change a level's map picture, or write or fix anything in `game_design/worlds/`.
---

# Level creation and documentation

Each level's world doc (`game_design/worlds/README.md`) is generated, so its text and its pictures can't disagree:
one file says where every zone and object is, and the script draws the pictures and writes the tables from it.

## The level folder

```text
game_design/worlds/<level>/
├── source/            written by you
│   ├── map.png        the game's map picture without labels
│   ├── world.yaml     zones and objects, with shapes in map.png's pixels
│   └── template.md    the doc's prose, with slots
└── generated/         written only by make generate-world-docs
    ├── <level>.md
    ├── map.png, map_objects.png, map_zones.png
    └── zone_<zone>.png, one per zone
```

`<level>` is the level's folder name in `common/maps/` (`lunar_surface`).

## Steps

1. **Map picture.** `source/map.png` is the game's own map picture without its labels, never a screenshot with the
   HUD or the fog of war on it. For the voxel levels it is `MapLayout::get_plain_pixels()`
   (`demos/hyperbolic_wormholes_rust/src/map_layout.rs`); there is no command that saves it yet, so save it from a
   short test or program, and say in the PR how you made it.
2. **`world.yaml`.** Follow `game_design/worlds/world_doc_schema.py`:
   - `level` is the level's name, `root` the zone that holds the whole level, and `zones` and `objects` map each
     name to its entry.
   - A zone has a `shape`, a `description` (where it is), and lists `zones` and `objects` directly in it. Every zone
     but the root is listed in exactly one zone, and so is every object; the root lists only zones.
   - An object has a `shape`, a `description` (what it is) and its `layout_name` in `common/maps/<level>/map.json`.
   - A shape is a `circle` (`centre_px`, `radius_px`), an `ellipse` (`centre_px`, `radii_px`), a `rect` (`min_px`,
     `max_px`) or a `polygon` (`points_px`), in `map.png`'s pixels, x to the right and y down.
   - An object is in the zone its shape's centre is in, and in no sub-zone of it.
   - Every object gets its own entry: no groups. Names follow the README's naming rules and the glossary.
   - To place things from the layout, convert a point's Fermi coordinates (u, v) to pixels with the picture's
     placement (`MapLayout::get_placement`): `x_px = (u_m - corner_u_m) / m_per_px`, and the same for v and y.
3. **`template.md`.** Write the level's intro and put each slot where it belongs, at most once:
   `{{map}}`, `{{map_zones}}`, `{{world_tree}}`, `{{map_objects}}` and `{{zone_sections}}`. Links are relative to
   `generated/`, which is as deep as `source/`.
4. **Generate.** From `game_design/worlds/`, run `make generate-world-docs`, then open every picture in
   `generated/` and check that each mark is on its object and each name is readable. Fix `world.yaml`, never the
   output.
5. **Check.** `make test` and `make check-world-docs` pass.
6. **PRs.** A level's change is two stacked PRs: one for `source/` and one for `generated/` (the generated one
   doesn't count toward the PR size limit, as it is a script's output).

## Must know

- Gotchas:
  - `make check-world-docs` compares pictures pixel by pixel, so it needs the Pillow that `uv.lock` pins.
  - Zone sections are breadth first and headed by the zone's path, so `## Blue zone > blue top` follows every
    top-level zone's section.
