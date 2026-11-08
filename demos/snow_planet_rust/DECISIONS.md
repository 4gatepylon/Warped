# Snow Planet decisions

Decisions made for the Snow Planet level demo without asking the user, with why. This folder started as a
verbatim copy of [`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/DECISIONS.md) (commit `9f249fc`);
that demo's decisions still hold for everything this file doesn't change. "User" marks the ones the user should
weigh in on.

## S1. Space: nearly flat hyperbolic space, the torus not yet wrapped (User)

The design says Snow Planet is T² × ℝ: flat, the two horizontal axes loop. The copied renderer, physics and laser
all work in hyperbolic space H³ (geodesic ray marching, Lorentz frames). Writing a Euclidean renderer with periodic
copies would take far longer than getting the level playable, so the map is Euclidean (`space.kind: euclidean`,
`space.torus_periods_m: [112, 72]`) and the loader turns it into H³ with curvature radius
`FLAT_CURVATURE_RADIUS_M` = 160 m (`Map::curve_flat`, Fermi coordinates u = −z, v = x, h = y). Over the 112 × 72 m
playing field that bends things by at most a few percent, which reads as flat.

Cut for now: walking, seeing and shooting across the seams. The torus periods are in the map (so a later renderer
gets them), but the level ends at its edge like Lunar Surface does. Options for later: (a) a Euclidean ray marcher
with the cell wrapped by `mod` (cheapest, correct), (b) drawing periodic copies of the structures in the current
renderer (wrong near black holes), (c) keep the edge. I recommend (a) once the gameplay is liked.

## S2. Two pylons a team, one console each

`game_design/DESIGN.md` says 2 pylons per team on levels other than Dust and Lunar Surface, 1 console each; the
WebGL prototype has more. The map follows the design: blue pylons at (u, v) = (−34, ±11), red at (34, ±11).

## S3. Blizzards (User: the numbers)

Three blizzard zones, upright cylinders in the map's `regions.blizzards` (format extended compatibly, see
`common/maps/map.schema.json`): the white-out in the middle (radius 10 m) and two squalls (radius 7 m) at
(−14, 24) and (14, −24), all 16 m high. Each has its own numbers, so the user can tune them in the map
(`tools/make_map.py`, `BLIZZARD_NUMBERS`):

| Field | Value | What it does |
| --- | ---: | --- |
| `speed_frac` | 0.5 | walking and sprinting speed inside (`VoxelPlayer::step`) |
| `freeze_health_per_s` | 0.08 | health lost per second inside: dead after 12.5 s (in 50 steps, `FREEZES_TO_KILL`) |
| `shot_reach_m` | 8 | meters a laser goes inside one before it stops (`VoxelWorld::stop_in_blizzards`) |
| `fog_per_m` | 0.12 | fog density: 10 m of blizzard lets through 30% of the light |
| `snow_per_m3` | 0.6 | chance of a flake per cubic meter cell |

- Shots: each blizzard uses up the fraction (length inside / its `shot_reach_m`) of the shot's blizzard budget;
  the shot stops where the fractions add up to 1, and doesn't hit what is behind. So a shot can cross the 7 m
  squall's edge but not its middle, and goes 8 m into the white-out.
- Freezing is a hit the player's own window sends on itself (`Client::send_freeze`), so the server's health,
  death and respawn rules apply unchanged. The window decides it is in a blizzard; a cheating client could skip
  it. The server should own this once there is a real server.
- Looks: fog and flakes are added per pixel along the first ray segment (straight in Fermi coordinates), after the
  surface or sky is shaded, so they cost nothing where no ray crosses a blizzard. Light bent by a black hole into
  a blizzard isn't fogged along the bent part.

## S4. Snow look

`appearance.style: snow` switches the Lunar sky (stars, Milky Way, spaceships) to a pale blue sky with clouds,
the ground to white snow with soft grain (no craters, no tile edges), and the distance haze to white fog. Ice and
packed snow use the map's palette; no external textures were downloaded yet (S6).

## S5. Black holes and wormholes

Two black holes at (0, ±17), one wormhole pair from (−22, −27) to (22, 27), from the copied H³ lensing: they bend
light and lasers as on Lunar Surface. Their reach (11 m and 9 m) makes them large on screen; the user may want
them smaller.

## S6. No new assets

The robots and guns are the copied demo's (from `demos/fps_demo/assets/` and `common/maps/models/`). No textures
were downloaded: the snow is procedural. Snow and ice textures from ambientCG (CC0) are the obvious next step.

## S7. Match interface (W-123)

`--level-result-file <path>` (with `--serve`) writes `{"winner":"blue"}` or `{"winner":"red"}` once, through
`<path>.partial` and a rename, and then the server doesn't start a new game. `--team blue|red` (with `--join`)
joins at once. `--bot-objective consoles` makes a bot claim the enemy consoles one after another (the server takes
its word), so a match can be tested without walking.

## S8. Benchmark views and binary names

`--bench-views` renders Snow Planet viewpoints (the copied Lunar Surface ones are gone; other maps still load).
The binaries are `snow-planet` and `snow_planet_server`, so several levels can be built side by side.
