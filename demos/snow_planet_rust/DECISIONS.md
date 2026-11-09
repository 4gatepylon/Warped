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

One blizzard (the user asked for a single one in the middle; there were three), an upright cylinder in the map's
`regions.blizzards` (format extended compatibly, see `common/maps/map.schema.json`): the white-out at the centre,
radius 10 m, 16 m high. Each blizzard has its own numbers, so the user can tune them in the map
(`tools/make_map.py`, `BLIZZARD_NUMBERS`):

| Field | Value | What it does |
| --- | ---: | --- |
| `speed_frac` | 0.5 | walking and sprinting speed inside (`VoxelPlayer::step`) |
| `freeze_health_per_s` | 0.08 | health lost per second inside: dead after 12.5 s (`Server::freeze_players`) |
| `shot_reach_m` | 8 | meters a laser goes inside one before it stops (`VoxelWorld::stop_in_blizzards`) |
| `fog_per_m` | 0.12 | fog density: 10 m of blizzard lets through 30% of the light |
| `snow_per_m3` | 0.6 | chance of a flake per cubic meter cell |

- Shots: each blizzard uses up the fraction (length inside / its `shot_reach_m`) of the shot's blizzard budget;
  the shot stops where the fractions add up to 1, and doesn't hit what is behind. So a shot goes 8 m into the
  white-out.
- Freezing is decided by the server (`net::Server::freeze_players`, each tick): it reads the blizzards from the map
  (`net::Freezing`), adds `freeze_health_per_s × dt` to the damage of every player whose last reported position is
  inside one, and eliminates a player at full damage as killed by themselves (a death, no one's kill). Players who
  haven't reported a position since joining don't freeze, and nothing freezes once a team has won.
- The slowdown stays in the moving window (`VoxelPlayer::step`): in this demo every window moves its own player and
  the server takes the position it reports, so a server-side speed limit would need server-side movement for all
  players (a bigger change). A cheating client could walk at full speed through a blizzard but still freezes.
  **User:** say if the server should enforce movement speed.
- Looks: fog and flakes are added per pixel along the first ray segment (straight in Fermi coordinates), after the
  surface or sky is shaded, so they cost nothing where no ray crosses a blizzard. Light bent by a black hole into
  a blizzard isn't fogged along the bent part.

## S4. Snow look

`appearance.style: snow` switches the Lunar sky (stars, Milky Way, spaceships) to a pale blue sky with clouds,
the ground to white snow with soft grain (no craters, no tile edges), and the distance haze to white fog. Ice and
packed snow use the map's palette; no external textures were downloaded yet (S6).

## S5. Black holes and wormholes

Five black holes and one wormhole pair (placed in S9), from the copied H³ lensing: they bend
light and lasers as on Lunar Surface. Their reach (11 m and 9 m) makes them large on screen; the user may want
them smaller.

## S6. No new assets

The robots and guns are the copied demo's (from `demos/fps_demo/assets/` and `common/maps/models/`). No textures
were downloaded: the snow is procedural. Snow and ice textures from ambientCG (CC0) are the obvious next step.

## S7. Match interface (W-123)

`--level-result-file <path>` (with `--serve`) writes `{"winner":"blue"}` or `{"winner":"red"}` once, through
`<path>.partial` and a rename, and then the server doesn't start a new game. `--team blue|red` (with `--join`)
joins at once. `--bot-objective consoles` makes a bot claim the enemy consoles one after another. Since W-126 (same
play in every level) the bot walks to each one, around the blizzard, and the server takes a hold only from within
`consoles.reach_m` of the console, aimed at it, so a bot win shows every pylon can be reached on foot.

## S8. Benchmark views and binary names

`--bench-views` renders Snow Planet viewpoints (the copied Lunar Surface ones are gone; other maps still load).
The binaries are `snow-planet` and `snow_planet_server`, so several levels can be built side by side.

## S9. Five black holes, scattered (User)

The user asked for five black holes: three north of the white-out and two south, placed a little at random and the
two south ones offset rather than mirrored. They start from hand-placed spots (u, v) (−22, 20), (0, 27), (21, 19)
and (−6, −21), (19, −27) (`BLACK_HOLES` in `tools/make_map.py`), and the generator moves each up to 2.5 m at random
with the seed `SCATTER_SEED` (7, named in the map's `source`), drawing again until each is at least 22 m from the
next so their 11 m reaches don't overlap, at least 12 m from every pylon and mound, 20 m from the wormhole mouths
and 21 m from the white-out's centre. The user asked for this slight randomness on most maps. Blue's mounds move up
to 2 m and turn up to 0.3 rad with the same seed, and red's are blue's turned half a turn, so cover stays fair. The wormhole pair moved to the corners, A at (−46, −26) by blue's
south pylon and B at (46, 26) by red's north pylon, so it is a flank route of the same length for each team (moved in and down in S12).

Fairness: the teams spawn west (blue) and east (red), so each sees the same three holes north and two south. The
holes are not mirrored between the halves: blue's half has the north-west hole and the south one nearest the
centre, red's has the north-east one and the south one 19 m out; the north hole is on the middle line. Pylons,
spawns, mounds and the wormhole stay point-symmetric.

## S10. Performance cut and handoff notes

`BEND_STEP_FRAC` is 0.2 (was 0.12): the only view over 16.7 ms median at 2560×1440 (black hole at 9 m, 18.3 ms)
drops to 15.5 ms with no visible change. The user froze the visuals, so no other quality changes were made.

For a future agent:
- Checked on this branch: a server with `--level-result-file` and two `--bot --bot-objective consoles --team
  blue|red --label` bots ends with `{"winner":"red"}` written atomically; two `--join --team` windows over UDP
  both join one server.
- The looping flat torus (S1 option A) is not on this branch. Work in progress is on the unpushed local branch
  `devin/snow-planet-torus` (worktree `.devin/worktrees/snow-planet-torus`); the blizzard light bending is parked
  on `devin/snow-planet-swirl-parked`. Neither is in any PR.

## S12. Same play as Lunar Surface, W-126 (same play in every level) (User: the mouths' move, the blizzard's look)

Checked against Lunar Surface (`../hyperbolic_wormholes_rust/`), the reference, on 2026-11-08 (Claude Code, branch
`claude/snow-planet-parity`):

- **Same code.** Every source file Snow Planet shares with Lunar Surface is the same except the level's own
  (`diff -rq`): the HUD, guns, lasers, minimap, menu, exit prompt, third person, thrown wormholes and consoles are
  Lunar's code. Lunar's later title-screen choice "join a server by its address" (W-118 (join any server)) isn't
  here; it isn't on W-126's list.
- **Same settings.** `server_settings.yaml` and `player_settings.yaml` are Lunar's, line for line, except
  `jump_time_per_map_jump_time` (`the_settings_files_match_lunar_surface`).
- **Jump.** The settings set the jump's time in the air as a multiple of the map's own (2 × jump speed / gravity).
  Snow Planet's map has Earth's 9.81 m/s² where Lunar's has 6, so its jumps lasted 0.35 s against Lunar's 0.58 s
  (gravity 80 m/s² against 30). `jump_time_per_map_jump_time` is now 0.545 here, so a jump rises 1.25 m in 0.58 s as
  on Lunar (`a_jump_matches_lunar_surface`). The map's gravity stays 9.81, so the snow puffs fall as before.
- **The wormhole couldn't be walked through.** Its mouths hung at 2.4 m, 0.8 m above the eye, and a walker crosses
  only within the 0.4 m throat; and both mouths stood just outside the playable ellipse (1.01 of it), so a walker
  stopped at the invisible edge in front of A, and anyone who came out of B was stuck outside it. The mouths now sit
  at eye height (1.6 m, as Lunar's sit at a walker's eye over its ridge), moved in to 0.83 of the ellipse with a
  slight seeded scatter (`MOUTH_SEED`, B is A turned half a turn): A (−45.5, −20.1), B (45.5, 20.1). The black holes
  didn't move. **User:** the mouths look 0.8 m lower and about 6 m nearer the middle.
- **The edge doesn't trap.** A step past the playable ellipse is refused only when it goes further out
  (`VoxelWorld::is_moving_out`), so a player put past it (by a mouth near the edge) walks back in. Lunar Surface's
  soft edge is unchanged.
- **The blizzard's damage** already shows Lunar's hit effects: the window turns them on whenever its health falls,
  and freezing lowers it every 50 ms, so inside the white-out the red edges, blur and shake stay at full strength,
  and the hit's slow (half speed for 1 s) stacks with the blizzard's own half: a quarter of walking speed inside.
  **User:** keep that, or make freezing pulse once per hit's worth of health, or keep the blizzard at half speed.
- **One blizzard.** The map has exactly one, the white-out (`points_find_their_blizzard`); the README and the map's
  description said three.
- **Scripted play** (`play_tests` in `src/voxel_world.rs`): eight players a team spawn on open snow facing 20 m of
  clear view; walking, sprinting, jumping and crouching for 15 s in twelve directions from each spawn never sticks,
  sinks or leaves the level; both level mouths and a thrown pair carry walkers and lasers from twelve directions;
  every console is reachable on foot and `E` reaches it from eight sides (`every_console_can_be_turned_off_on_foot`).

## S13. A sharp centre with softer edges, W-136 (sharp centre, soft edges)

The same code as Lunar Surface's renderer (its D115): the middle 66% of the width is drawn exactly as before and the
rest at half resolution (the even pixels shaded, the rest blended), by default in the window and the bench views
(`--sharp-width-frac`, `--half-width-frac`). At 3440x1440, median GPU ms: `wormhole` 23.1 → 19.3, `laser_open` 20.7 →
18.7, `black_hole` 18.3 → 17.6, the spawns 15.0 → 13.5; at 2560x1440 every view is now under 16.7 ms (`wormhole`
17.2 → 14.6). The centre is the same picture (at most 2 of 255, through the bloom).

## S14. Lunar Surface's half-resolution lens, for 60 fps at 3440x1440

With S13's sharp centre, three views were still over 16.7 ms at 3440x1440 (`wormhole` 19.3, `laser_open` 18.7,
`black_hole` 17.7 ms median): refinement inside the lenses took about 5 ms of each (refined pixels painted, and
skipping refinement altogether: 13.8, 13.7, 12.7 ms). The renderer now uses the half-resolution lens the user chose
for Lunar Surface (its D114, option D's part B): rays count their bent steps (`BENT_STEPS_FIRST_BIT` in `surface.w`;
the speckle share is now read through `BEAM_SPECKLE_MASK`), and a refined cell whose four samples all took 8 or more
bent steps blends its even pixels' colours instead of tracing its three odd pixels. Snow Planet keeps its own bent
steps (`BEND_STEP_FRAC` 0.2, S10), so Lunar's longer steps (option C) aren't needed.

- **Measured** (median GPU ms, best of 3 alternating rounds under the exclusive lock): at 3440x1440 every view is
  under 16.7 ms (`wormhole` 19.3 → 14.2, `laser_open` 18.7 → 15.6, `black_hole` 17.7 → 12.9; the rest 1-2 ms
  faster); at 2560x1440 every view is under 12.2 ms. The warp tour in a real window (3456x1996, automatic render
  scale): 63.4 → 71.4 fps, frames over 16.7 ms 30.4% → 1.3%.
- **Look:** only pixels deep in a lens change (at most 0.73% of a view's pixels past 8 of 255): edges inside a lens
  are a little stepped, and a thin laser seen through a lens is dotted.
- **Tried and not taken:** the beam's haze between samples (Lunar's D111): the same pictures but no faster here.
- **For the user:** like Lunar's option D, a visible trade-off; the lead takes it to the user.

## S15. No aliasing, W-138 (no aliasing)

The same code as Lunar Surface's renderer (its D116) without its half-resolution lens (#465 adds that, with its
smoothing): outside the sharp centre tone mapping shows a cubic B-spline of the half- or quarter-resolution pixels,
blended in over 48 px, so the soft edges show no steps, and FXAA 3.11 anti-aliases the whole image. At 3440x1440:
0.3-1.3 ms more per view. (S14 is #465's half-resolution lens.)

## S16. Option E is the default sharp centre, W-136 (sharp centre, soft edges)

As Lunar Surface's D117: the middle half of the width sharp, half resolution out to three quarters, a quarter beyond,
all smoothed (S15). At 3440x1440: up to 2.4 ms faster than B (`wormhole` 20.2 → 17.8 ms, `laser_open` 20.2 → 18.4).
