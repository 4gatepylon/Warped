<!-- game_design/worlds/make_world_doc.py writes this file from ../source/. Edit those files, not this one. -->

# Snow Planet

Snow Planet is the inner blue level (depth 1): an open snow field in flat space whose soft level edge slows players
without end, with light snowfall and fog. Each team has two pylons. Five black holes and one fixed wormhole bend the
field, six rounded snow mounds give cover, and a tall blizzard stands in the middle. The zones are bands across the
level's length: the blue zone is more than 20 m from the middle on blue's side, the red zone more than 20 m on red's
side, and the middle zone lies between.

The pictures are the game's map picture of Snow Planet (`demos/snow_planet_rust/map_pictures/snow_planet.png`), with
blue on the left, so left (blue's left) is the top of a picture. Names follow the
[naming rules](../../README.md#naming-rules); the layout is
[`common/maps/snow_planet/map.json`](../../../../common/maps/snow_planet/map.json).

![Snow Planet's map, with its zones outlined and named](map_zones.png)

## World tree

```text
Snow Planet
├── blue zone
│   ├── blue spawn
│   ├── blue left pylon
│   ├── blue right pylon
│   ├── blue mid mound
│   ├── blue right mound
│   ├── blue black hole
│   └── blue mouth
├── middle zone
│   ├── blizzard
│   ├── top black hole
│   ├── bottom black hole
│   ├── top mound
│   └── bottom mound
└── red zone
    ├── red spawn
    ├── red left pylon
    ├── red right pylon
    ├── red mid mound
    ├── red left mound
    ├── red left black hole
    ├── red right black hole
    └── red mouth
```

![Snow Planet's map, with every object outlined and named](map_objects.png)

## Blue zone

Blue's end of the level: everything more than 20 m from the middle on blue's side.

![Snow Planet's blue zone: the whole map, blurred and darkened outside the zone, with the zone's sub-zones and objects outlined and named](zone_blue_zone.png)

| Name | What it is | Name in the level layout |
| --- | --- | --- |
| **blue spawn** | Where blue's players enter the level. | blue spawn |
| **blue left pylon** | Blue's pylon on blue's left, as seen from blue's spawn facing red. | blue pylon |
| **blue right pylon** | Blue's pylon on blue's right, as seen from blue's spawn facing red. | blue pylon |
| **blue mid mound** | The snow mound in front of blue's spawn. | blue snow mound |
| **blue right mound** | The snow mound behind the blue right pylon. | blue snow mound |
| **blue black hole** | The black hole on blue's right, at the edge of the blue zone. | black hole north-west |
| **blue mouth** | The blue zone's mouth of the wormhole that leads to the red mouth. | wormhole mouth A |

## Middle zone

The 40 m band across the middle of the level, between the team zones.

![Snow Planet's middle zone: the whole map, blurred and darkened outside the zone, with the zone's sub-zones and objects outlined and named](zone_middle_zone.png)

| Name | What it is | Name in the level layout |
| --- | --- | --- |
| **blizzard** | The tall column of snow and fog in the middle, 10 m in radius and 16 m high. A player inside it moves at half speed and slowly loses health, and a shot goes at most 8 m inside it. | white-out |
| **top black hole** | The black hole of the middle zone at the top of the main map. | black hole south-west |
| **bottom black hole** | The black hole of the middle zone at the bottom of the main map. | black hole north |
| **top mound** | The snow mound of the middle zone at the top of the main map. | blue snow mound |
| **bottom mound** | The snow mound of the middle zone at the bottom of the main map. | red snow mound |

## Red zone

Red's end of the level: everything more than 20 m from the middle on red's side.

![Snow Planet's red zone: the whole map, blurred and darkened outside the zone, with the zone's sub-zones and objects outlined and named](zone_red_zone.png)

| Name | What it is | Name in the level layout |
| --- | --- | --- |
| **red spawn** | Where red's players enter the level. | red spawn |
| **red left pylon** | Red's pylon on blue's left, as seen from blue's spawn facing red. | red pylon |
| **red right pylon** | Red's pylon on blue's right, as seen from blue's spawn facing red. | red pylon |
| **red mid mound** | The snow mound in front of red's spawn. | red snow mound |
| **red left mound** | The snow mound behind the red left pylon. | red snow mound |
| **red left black hole** | The black hole on blue's left at the edge of the red zone, as seen from blue's spawn. | black hole south-east |
| **red right black hole** | The black hole on blue's right at the edge of the red zone, as seen from blue's spawn. | black hole north-east |
| **red mouth** | The red zone's mouth of the wormhole that leads to the blue mouth. | wormhole mouth B |

The layout's compass names for the black holes don't match the main map's top and bottom; the world names are the
official ones.
