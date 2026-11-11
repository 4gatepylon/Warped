# City Sphere

City Sphere is the purple level in the middle of the map (depth 0): a satellite city inside a hollow sphere in flat
space, 24 m in radius, where gravity pulls outward onto the inside of the sphere. Its streets cover the sphere's
surface, a glass shell 17 m in radius roofs them and is the glass level, six lifts lead up to the glass level, and a
glowing core hangs in the centre. Players see each other through the glass but can't shoot through it. It is the
smallest level and is built for close quarters, with no warp objects and one pylon per team, on the glass level. Red's
spawn and pylon are blue's reflected through the sphere's centre.

The level layout ([`common/maps/city_sphere/map.json`](../../../../common/maps/city_sphere/map.json)) splits the
sphere into two zones, the blue half and the red half, with no middle zone. Names follow the
[naming rules](../../README.md#naming-rules).

> NOTE: In the game, City Sphere's team areas are domes over the two pylons on the glass level, and everything else
> is the middle ([`DESIGN.md`](../../../DESIGN.md#zones-and-routes)). The zones here are the level layout's two halves,
> which the game doesn't use yet.

The pictures are the game's map picture of City Sphere (`demos/city_sphere_rust/assets/map_picture.png`), an unwrapped
view of the whole sphere like a world map: longitude across, latitude up and down. It is scaled up twice and shifted so
that each half is in one piece, blue's on the left and red's on the right, with a strip repeated at each side.

{{map_zones}}

## World tree

{{world_tree}}

{{map_objects}}

{{zone_sections}}

City Sphere's blocks and covers keep the level layout's numbers rather than being numbered from the spawn outward, and
its lifts have no team in their names, as in the level layout. Lifts 2 and 5 stand exactly on the line between the
halves; lift 2 is listed in the blue zone and lift 5 in the red zone.
