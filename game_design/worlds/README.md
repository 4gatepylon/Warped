# Worlds

A level's **world** is its 3D game world: the place where its actual objects are, such as the ground, structures,
pylons, warp objects and the sky ([glossary](../GLOSSARY.md#structure-of-play)). Each level's doc lists its world as
a tree, a "bill of materials" of what exists now: its zones first, then every named object in each zone. Map pictures
beside each doc outline the objects in red and name them.

> NOTE: These files describe the worlds as they are built now. If a level's features change (an object is added,
> moved, renamed or removed, or a zone changes), the agent making the change asks the user, or opens a PR, to update
> the level's file and its pictures in the same change.

## Source and generated files

A level's doc and pictures are generated from one source of truth. A level folder `<level>/` holds:

- `source/`, written by people and agents: `map.png` (the game's map picture without labels), `world.yaml` (the
  zones and objects, with their shapes in the picture's pixels) and `template.md` (the doc's prose, with slots).
- `generated/`, written only by `make generate-world-docs`: `<level>.md`, the map picture, a picture of every object,
  a picture of the zones, and one picture per zone.

Never edit `generated/` by hand. `make check-world-docs` fails when it is out of date. How to make or change a level's
source is in the [`level-creation-and-documentation`](../../.agents/skills/level-creation-and-documentation/SKILL.md)
skill.

## Levels

One doc per level, in the map's order from blue to red:

- [Dust](dust/generated/dust.md)
- [Snow Planet](snow_planet/generated/snow_planet.md)
- [City Sphere](city_sphere/generated/city_sphere.md)
- [Dark Forest](dark_forest/generated/dark_forest.md)
- [Lunar Surface](lunar_surface/generated/lunar_surface.md)

## Naming rules

Every object has a name, like a callout in Counter-Strike, so that anyone can say exactly which wall or rock they
mean.

- **Unique.** Every object in a world has a name unique in that world.
- **Team zones.** Things in a team's zone are `<team> <thing>`: "blue spawn", "red habitat".
- **Left, mid and right.** Where a team has several of one thing, they are left, mid and right as seen from blue's
  spawn, facing red, for both teams: "red left pylon" is the red pylon on blue's left.
- **Middle zone.** Middle-zone things have no team. They are "top" and "bottom", as on the main map, or
  "blue-side" and "red-side": "top arch", "red-side mesa".
- **Repeated pieces.** A cluster of repeated pieces, such as a row of Hesco blocks, gets one name, and clusters are
  numbered from the team's spawn outward: "blue hesco wall 3".

Each level's doc gives every object's name in its level layout (`common/maps/<level>/map.json`) beside its world
name, so a name can be found in the data.

> NOTE: When developing, left, mid and right are always from blue's perspective. Blue is the official team colour for
> global positions: where the origin or direction of a location depends on a team, it is blue's.

## Left and right

Left and right were checked in the game's code, not only read off the pictures.

- **Voxel levels** (Dust, Snow Planet, City Sphere, Lunar Surface). A player's camera looks along its frame's −z
  axis and its right is the frame's +x axis (`demos/snow_planet_rust/src/physics.rs`, `forward = -frame.col(2)`,
  `right = frame.col(0)`). The renderer puts that +x on the right of the screen (`render_pixel` in
  `demos/snow_planet_rust/src/render_cpu.rs`), and the map pictures are a top view that is not mirrored
  (`demos/snow_planet_rust/src/map_photo.rs`).
- **Dark Forest.** A player's forward is `R_y(yaw) * −z` and right is `R_y(yaw) * +x`, rendered with a right-handed
  view (`demos/dark_forest_rust/src/game/player.rs`).

> NOTE: Dark Forest will be modified to conform to the Voxel levels' standards in all ways. Because different agents implemented different levels, Dark Forest was unfortunately and randomly implemented slightly differently. No one should ever index on Dark Forest's idiosyncracies as being indicative of what will happen more broadly.

So on the pictures, where blue is on the left, left is the top of the picture. In Dark Forest, where blue is at the
top, left is the right of the picture.

## The pictures

- Every level's `source/map.png` is the game's own map picture, without labels. Lunar Surface's is exported from its
  level layout by the engine. Dust's and Dark Forest's are scaled up twice so the labels stay in proportion, and City
  Sphere's is shifted so that each half of the sphere is in one piece.
- The generator writes a picture of every object, a picture of the zones, and one picture per zone that shows the
  whole map with everything outside the zone blurred and darkened.
