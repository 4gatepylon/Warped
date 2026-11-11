# Dark Forest

Dark Forest is the inner red level (depth 1): a dark forest in flat space whose soft level edge slows players without
end. Each team has two pylons: one in a palisade and one in a treehouse. Rope ladders lead up to decks in the trees,
bridges join the decks into a high route across the middle, and the great tree in the centre carries a deck with two
lookouts. Five fixed wormholes, called gates here, join places across the level. The zones are bands across the
level's length: the blue zone is more than 24 m from the middle on blue's side, the red zone more than 24 m on red's
side, and the middle zone lies between.

The pictures are the game's map picture of Dark Forest (`demos/dark_forest_rust/assets/map_picture.png`), scaled up
twice, with blue at the top. Blue faces down the picture, so left (blue's left) is the right of a picture. Names follow
the [naming rules](../../README.md#naming-rules), with the exceptions at the end of this file; the layout is
[`common/maps/dark_forest/map.json`](../../../../common/maps/dark_forest/map.json).

{{map_zones}}

## World tree

{{world_tree}}

{{map_objects}}

{{zone_sections}}

## Exceptions to the naming rules

These names were approved as they are, from the level layout, though they break two rules:

- **Team names in the middle zone.** The decks, bridges, lookouts and ladders of the high route, and the middle zone's
  rocks, carry the name of the team whose side they are on ("blue middle deck"), not "blue-side".
- **Rock numbers.** Each team's rocks are numbered 0 to 3 roughly from the centre toward its spawn: rocks 0 and 1 are
  in the middle zone, and rock 3 is the nearest to the spawn, not the farthest.

The forest's other trees aren't named here. Each treehouse and deck stands in a tree of its own in the level layout
(such as "blue home tree", "blue middle tree" and "blue flank tree").
