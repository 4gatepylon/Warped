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

## S11. Plan: the looping world (User: options G, V, L)

Status, 2026-11-08: the user said to build it "After parity", and the lead chose the recommended options for the
user, who hasn't picked: G1, L1 and V1. They are built as stacked PRs from `claude/snow-planet-torus-layout` (layer
1, L1), on top of W-126 (same play in every level), which moved the wormhole mouths (S12). The plan as written,
with the mock-up, is on `claude/snow-planet-torus` (commit `902bd73`):

A plan (2026-11-08, Claude Code, branch `claude/snow-planet-torus`). `DESIGN.md` makes both
Snow Planet and Dark Forest T² × ℝ (the user called Dark Forest "the toroidal setup"); the user's W-120 (Snow Planet
demo) words don't ask for it. Snow Planet's period is 112 m (u) × 72 m (v). The devin branch S10 names
(`devin/snow-planet-torus`) was never pushed, so nothing of it is reused. Whatever is built here should serve Dark
Forest too.

**Geometry (option G):**
- **G1, wrap in today's nearly flat H³ (recommended).** Keep the renderer, physics and lensing. Glue the cell's edges
  in Fermi coordinates: crossing u = ±56 is a boost along u, an exact isometry; crossing v = ±36 keeps (u, h) and the
  direction's parts along the Fermi axes. Measured on the ground plane (`fermi_glue_lens.py`, scratch): copies seen
  across v look 1.7% bigger than on a flat torus, at every distance; copies along u shrink as in H³ today (8% at
  112 m, 26% at 224 m); a laser 1 m from the v seam drifts 0.44 m in 25 m, as one does anywhere today. About 6 PRs.
- **G2, a Euclidean renderer, physics and laser.** Exact, but rewrites every Minkowski formula in
  `render_voxel.metal`, `voxel_world.rs`, the minimap and the map page: 15 or more PRs.

**How each thing wraps (G1):**
- **Walking:** after each step, a body past a seam is moved by the glue (one function, `wrap_frame`). The ellipse
  boundary goes: the torus has no edge. Remote players aren't interpolated, so they jump across with no streak.
- **Lasers:** a shot's path is cut where it crosses a seam and goes on from the other side, like a throat (a new
  piece). A 480 m shot goes around 4 to 6 times, so the shooter's own copy must not be hit.
- **Light rays (shader):** the camera is wrapped into the cell; each straight chord also stops at the nearest seam
  and goes on from the other side. Inside a warp's reach the ray isn't wrapped; it is wrapped when it leaves.
  Shadow rays (80 m) wrap the same way.
- **Ghosts:** everything within a margin of a seam (structures, avatars, warps, thrown mouths, dust puffs, beam
  pieces) gets a copy past the opposite seam, so a chord near the seam sees it and lenses that straddle a seam work
  (the north black hole's 11 m reach crosses the v seam). Static ghosts are made once, moving ones each frame. A hit
  on a ghost counts as a hit on its original.
- **Blizzard:** fog and flakes for each copy the first ray piece passes; the fog's length becomes the ray's travelled
  distance, not the straight Fermi distance.
- **Black holes and wormholes:** as today, per copy; a mouth's exit is its partner's own place.

**What players see (option V):** the level repeats every 112 m along the teams' axis and every 72 m across. The snow
fog never passes 82% (`SNOW_FOG_MAX_FRAC`), so copies never fade out: without a limit, the horizon fills with black
holes, pylons and green beacon beams, 400 m high and at least a pixel wide.
- **V1, copies out to 224 m, then today's empty snowfield (recommended).**
- **V2, copies to the horizon:** the costliest, and the busiest.
- **V3, V1 with the fog reaching 100% at the limit:** hides where copies end, but whitens the far ground a little
  (visuals are frozen).

**Layout (option L):** as it stands, the torus breaks the level: blue's spawn (u = −48) is 16 m from red's (u = 48)
across the u seam, red's pylons stand 30 m behind blue's spawn, and the wormhole's mouths are 28 m apart through the
corner, nearer than through the throat is worth. `DESIGN.md`'s "paired bands" puts each spawn half a period from
the other.
- **L1, spawns, pylons and zones at the quarter points (u = ±28), period kept (recommended):** both routes 56 m.
- **L2, a 192 m period, layout kept:** both routes 96 m; the half beyond the seam needs new content.
- **L3, a ridge at the u seam:** the world loops only across v.
- Any of them: the mouths move so walking between them is long (at most 67 m on this torus), the black holes'
  seeded scatter uses wrapped distances (the north hole is 23.1 m from the south-west one across the seam, 1.1 m
  clear), placements stay slightly random, seeded and mirrored per team, and zones wrap.

**Cost in frame time:** counted per pixel (`ray_stats.py`, scratch; straight rays): today a pixel enters 0 to 0.33
reaches; with V1, it crosses 0.4 to 1.0 seams and enters 0.35 to 0.6 reaches, about 1.3 to 1.7 times today's work
(the spawn view 1.3×, the view back across the seam 1.7×); to the horizon (V2), 1.5 to 2.1×. Today's 7 to 16 ms GPU
views (Apple VM) would become about 10 to 25 ms, so views across a seam pass 16.7 ms unless far copies skip the lens
(only their black disk past about 120 m). The mock-up, which draws 25 copies by brute force (175 warps and 250
structures, each ray testing all), took 82 to 178 ms GPU against 14 to 27 ms without copies, on this Mac at load 40
to 60: an upper bound only. The first renderer layer measures it for real.

**Edge cases:**
- Seams: the snow's grain noise and the flake hashes aren't periodic, so a straight line shows in the snow at each
  seam unless the noise's lattice fits the periods (a tiny visible change everywhere).
- Fog: the far ground and the copies' fog must match, or a band shows where copies end (V1).
- Shooting: your own back (a shot along v at eye height comes back after 72 m); console reach, shot noise and the
  bot's aim use the nearest copy of each player.
- Players: the respawn ring's "never on top of another player", freezing in the blizzard, and the robot's walking
  speed (`avatar_animation.rs` takes the distance between two places, so a seam crossing reads as 100 m in one frame
  and the robot runs) all need the nearest-copy distance.
- Placing: thrown mouths near a seam, and their clearance from other warps, by wrapped distance.
- Views: the third-person camera across a seam; the minimap and map page (marks wrapped, the picture tiled); zone
  walls where zones wrap; a dust puff or beam cut at a seam without its ghost.
- Precision: wrapping keeps every position within 66 m of the middle, better than today.
- Compatibility (`compatible-changes`): the map changes (layout, no boundary, wrapping zones); `torus_periods_m` is in
  the schema but not yet in `common/maps/README.md`. The match runs Snow Planet as its own program, so only its map
  picture changes.
- Bench views: every view at a seam changes; new views look along each axis.

**Stacked PRs (G1, V1, L1), each under 200 lines:**
1. Layout L1: `tools/make_map.py`, the generated map, its picture, `common/maps/README.md` on torus periods. Still
   bounded, so it plays on its own.
2. Torus maths: `wrap_frame`, the nearest-copy offset and distance, with unit tests; nothing changes in play.
3. Ghosts: static and per-frame copies near the seams; a ghost's hits count for its original.
4. Rays wrap in the shader: camera, chords, shadows, blizzard copies, copy distance V1, no lens for far copies.
5. Walking and shooting wrap: steps, laser pieces, nearest-copy distances (respawn, bot, robot, consoles, freezing),
   no boundary on the torus.
6. Views: minimap, map page, third-person camera, seam-periodic snow noise.

**Mock-up:** WIP on `claude/snow-planet-torus`, never to merge: `TEMP_TORUS_MOCKUP_COPIES=<n>` (`map.rs`) copies the
level n periods each way, without wrapping, and `--bench-views` then renders seven views along the axes. Copies
along v are a little stretched (H³ placement); copies along u are what G1 would show. Pictures:
`.artifacts/bench_views/20261108-154239-torus-copies2/torus_mockup_{1,2}_of_2.png`.

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
- **Scripted play** (`play_tests` in `src/voxel_world.rs`): eight players a team spawn on open snow facing 8 m of
  clear view; walking, sprinting, jumping and crouching for 15 s in twelve directions from each spawn never sticks,
  sinks or leaves the level; both level mouths and a thrown pair carry walkers and lasers from twelve directions;
  every console is reachable on foot and `E` reaches it from eight sides (`every_console_can_be_turned_off_on_foot`).

## S13. Paired-band layout, the looping world's layer 1 (User: the layout, chosen for you as L1)

Built on `claude/snow-planet-torus-layout`, 2026-11-08 (Claude Code), the first layer of S11's stack. The level still
ends at its edge; only the layout changes, so the looping world can join it later without the spawns meeting at the
seam.

- **Spawns** at a quarter of the u period, u = ∓28 m (were ∓48 m): both ways around to the other team will be 56 m.
- **Pylons** at (∓30, ±8) m (were (∓34, ±16) m), beside their spawn, so a team defends both routes from one place.
  They stand 2 m toward the seam band, so pylon to pylon is 52 m through the seam band and 60 m through the middle.
- **Zones** in paired bands: blue from u = −42 to −14 m, red from 14 to 42 m, and the middle in two bands, around
  u = 0 (the white-out and the black holes) and around the u seam (the wormhole and two mounds).
- **Mounds:** the mound by blue's spawn (and red's, turned half a turn) moved behind it, to (∓40, ∓6) m before its
  seeded scatter: in front of the new spawn it stood 4 m from a spawning player's face. The other mounds, the black
  holes and the mouths stay where they were; the scatter now measures distances the shortest way around the torus
  (`get_torus_distance`), and it still draws the same black holes.
- **Not yet:** walking, seeing and shooting across the seams (S11's layers 2 to 6). The bench views keep their old
  viewpoints, so `blue_spawn` and `red_spawn` no longer stand at the spawns.

## S14. Walking around the loop, layer 2

Built on `claude/snow-planet-torus-walk`, 2026-11-08 (Claude Code), on layer 1 (S13).

- **`src/torus.rs`:** the cell, the copy of a spot nearest another and the glue (`Torus::shift_frame`,
  `wrap_frame`, `move_nearest`). The spot arithmetic (`get_cell_offset_m`, `get_nearest_offset_m`,
  `get_ghost_offsets_m`) knows nothing of H³, so Dark Forest's flat world can reuse it; the frame glue is H³'s.
- **Walking:** `VoxelPlayer::step` moves a body past a seam back into the cell by the glue, so positions stay within
  56 m and 36 m of the middle. A looping world has no edge (`is_outside_boundary`); the map keeps its ellipse for
  older programs. The spawn ring keeps players apart the short way round (`get_torus_distance_m`).
- **Ghosts:** each structure within `GHOST_MARGIN_M` (12 m) plus its own size of a seam, and each warp within its
  reach of one, is copied past the opposite seam when the world is built, so walkers and warps' lists meet it
  from both sides. On Snow Planet that is 8 ghosts of the 4 mounds near a seam (1 each past the v seam,
  3 for the two near a corner) and 2 of the north and south-east black holes, whose reach crosses the v seam.
- **Not yet:** rays (the view still ends at the seam, past which the ghosts show), lasers, other players and thrown
  mouths across the seams; the minimap and map page.
