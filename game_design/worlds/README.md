# Worlds

A level's **world** is its 3D game world: the place where its actual objects are, such as the ground, structures,
pylons, warp objects and the sky ([glossary](../GLOSSARY.md#structure-of-play)). Each file here lists one level's
world as a tree, a bill of materials of what exists now: its zones first, then every named object in each zone. Map
pictures beside each file mark the objects with red circles and rectangles and their names.

> NOTE: These files describe the worlds as they are built now. If a level's features change (an object is added,
> moved, renamed or removed, or a zone changes), the agent making the change asks the user, or opens a PR, to update
> the level's file and its pictures in the same change.

## Levels

One file per level, in the map's order from blue to red:

- [Dust](dust.md)
- [Snow Planet](snow_planet.md)
- [City Sphere](city_sphere.md)
- [Dark Forest](dark_forest.md)

## Naming rules

Every object has a name, like a callout in Counter-Strike, so that anyone can say exactly which wall or rock they
mean.

- **Unique.** Every object in a world has a name unique in that world.
- **Team zones.** Things in a team's zone are `<team> <thing>`: "blue spawn", "red habitat".
- **Left, mid and right.** Where a team has several of one thing, they are left, mid and right as seen from that
  team's own spawn, facing the other team: "red left pylon".
- **Middle zone.** Middle-zone things have no team. They are "top" and "bottom", as on the main map, or
  "blue-side" and "red-side": "top arch", "red-side mesa".
- **Repeated pieces.** A cluster of repeated pieces, such as a row of Hesco blocks, gets one name, and clusters are
  numbered from the team's spawn outward: "blue hesco wall 3".

Each level's file gives every object's name in its level layout (`common/maps/<level>/map.json`) beside its world
name, so a name can be found in the data.

## Left and right

Left and right were checked in the game's code, not only read off the pictures.

- **Voxel levels** (Dust, Snow Planet, City Sphere, Lunar Surface). A player's camera looks along its frame's −z
  axis and its right is the frame's +x axis (`demos/snow_planet_rust/src/physics.rs`, `forward = -frame.col(2)`,
  `right = frame.col(0)`). The renderer puts that +x on the right of the screen (`render_pixel` in
  `demos/snow_planet_rust/src/render_cpu.rs`), and the map pictures are a top view that is not mirrored
  (`demos/snow_planet_rust/src/map_photo.rs`).
- **Dark Forest.** A player's forward is `R_y(yaw) * −z` and right is `R_y(yaw) * +x`, rendered with a right-handed
  view (`demos/dark_forest_rust/src/game/player.rs`).

So on the pictures, where blue is on the left, blue's left is the top of the picture and red's left is the bottom.
In Dark Forest, where blue is at the top, blue's left is the right of the picture.

## The pictures

- Four levels mark the game's own map picture from the repo. Lunar Surface has none yet (making one needs the engine,
  which runs on a Mac), so its pictures are drawn from its level layout and say so on the image.
- A level has at most 3 pictures, one per zone where one picture would hold too many names.
- A thin red line joins a name to its mark when the name had to sit away from it.
