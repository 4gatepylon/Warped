# City Sphere (Rust): decisions

Decisions made for the user while building W-119 (City Sphere demo), each with why and what the user may want
instead. The user's words for this level win over anything here; **Weigh in** marks the ones to confirm.
The copied hyperbolic demo's decisions are in [`../hyperbolic_wormholes_rust/DECISIONS.md`](../hyperbolic_wormholes_rust/DECISIONS.md).

## Shots and the glass

1. **"Above the glass" is the glass shell's inner side, the side facing the centre.** Gravity points outward, so up is
   toward the centre and the glass (a smaller sphere) is above the street. Players on the glass level stand on its
   inner face with nothing over them but the open middle of the sphere (no roof) and can shoot straight across it.
   **Weigh in:** the other reading (standing on the glass's outer face, between it and the street) would put a roof
   (the street's buildings) overhead, which the user said the glass level doesn't have.
2. **A shot from the street that meets the glass stops there.** The user said below the glass is a normal shooter, and
   `game_design/DESIGN.md` says shots don't pass the glass. **Weigh in:** shots from below could reflect too, or pass
   through.
3. **Only shots coming from the inner side reflect**, about the glass's normal at the hit (angle in = angle out), at
   most `glass.max_bounce_count` times (2 by default, 0 for none); the next time such a shot meets the glass it stops
   there. A shot ends after `glass.range_m` (90 m, every leg counted; the sphere is 48 m across). Both are server
   settings (`server_settings.yaml`), so a server can change them for everyone.
4. **Shots go through the lift holes** (there is no glass there), so a street player can shoot up through a hole and a
   glass-level player down through one.
5. **The laser is hitscan**: the hit is decided at once along the crosshair, from the eye; the beam is drawn from the
   gun's muzzle and fades over `sniper.shot_s + sniper.beam_trail_s`. One hit eliminates (`sniper.hits_to_eliminate`).
   The rifle (gun 2) isn't wired up in City Sphere yet.
6. **Other players' shots are traced again in each window** from the eye frame they send when their laser turns on, so
   their reflections show without changing the network protocol. Only the shooter's window reports hits.

## Level

7. **Stairs and lifts join the levels.** Six holes in the glass (two near each team's spawn, two in the middle, each
   the antipode of another) sit over glowing lift pads: hold `Space` on a pad to ride up, walk off onto the glass; walk
   into a hole on the glass level to drop to the street. Since W-119's bugs ("I separately cannot find how to get on top
   of the inner sphere ... There should be like stairs going up"), each team also has a flight of stairs: see 26.
8. **A small map first**: the street is a sphere of radius 24 m, the glass 17 m (7 m above the street), against the
   WebGL version's 36 m and 28.8 m. City Sphere is the smallest, close-quarters level, and the user asked to start
   small. Ten buildings and six glass-level covers a team.
9. **Each team has one pylon, exposed on the glass level** (the user's choice: pylons per team rise Dust 3, Snow 2,
   City 1, Forest 2, Lunar 3), with no cover within about 6 m, and every red thing is the antipode of a blue one, so
   the teams' halves are the same. Street pylons were removed.
10. **The map extends the portable format compatibly**: optional `ground.sphere` and `shells` (documented in
    `common/maps/README.md`, in the schema, version still 1). `demos/city_sphere_rust/tools/make_map.py` writes the map.

## Rendering and code

11. **A Metal rasterizer, not a ray tracer**: light goes straight in Euclidean space, and rasterizing is the fastest
    correct way (about 1.5 ms of GPU at 2560x1440). The glowing core at the centre lights everything (Lambert plus a
    little specular, no shadows); the glass is a see-through shell with grid lines and lit hole rims. Textures are
    Poly Haven CC0 ones the repo already has (`asphalt_02`, `concrete_wall_008`, `brick_wall_02`, `metal_plate`).
12. **The whole hyperbolic demo is copied and still builds** (it can still play `--map lunar-surface`), and City
    Sphere is added beside it (`city.rs`, `city_world.rs`, `render_city.rs`, `render_city.metal`), so the shared
    parts (network, settings, guns, overlay, robots) diff cleanly against the original. Pruning the hyperbolic parts
    is for the refactor into the full game.
13. **Playing alone needs no separate server**: without `--join` the window hosts a game server on `--port` (47810)
    that a second window or a bot can join. The hyperbolic demo only plays on a server.
14. **Robots share one pose per animation clip each frame** (posing the skinned model is the CPU's biggest cost).
15. **`--bot-objective consoles` walks**: up its stairs and along a planned path on the glass level
    (`CityWorld::find_glass_path`, an A* search on a 0.025 rad grid keeping 0.75 m from covers and 1.9 m from holes) to
    each enemy console, and the server counts a hold only while the player aims at the console within
    `consoles.reach_m` plus 0.6 m (the window's own check uses `reach_m`), as on Lunar Surface. A bot that can't aim at
    its console for 3 s starts its walk again.
16. **Two frames in flight**: the window no longer waits for the GPU to finish a frame before drawing the next, so
    the CPU and GPU overlap. Lunar Surface's vsync p99 fell from 33 to 23 ms on this VM; City Sphere's (21 to 28 ms,
    varying between runs) didn't clearly change, because its frames wait for the display, not the GPU. `--no-vsync`
    is a measuring flag only; play keeps vsync on.
17. **The scripted route's server doesn't announce itself** on the network, so recording it needs no Local Network
    permission; every other server still announces itself for `--join auto`.
18. **The minimap is Lunar's, with a sphere branch** in its shader (`get_sphere_minimap_ground`): the sphere seen from
    its centre around the player, gnomonic (straight lines through the centre, and so the grid, show straight), the way
    they face up, reaching the player settings' `minimap.range_m` of street but at most 1 rad (24 m of street), since
    a gnomonic map can't reach a quarter of the sphere and the whole level is 75 m from you to its far side. The grid
    lines are Cartesian lines on the faces of a cube around the sphere, every 5 m (0.21 rad), so they bend only where
    two faces meet. The ground is drawn once from the centre (buildings brown, covers grey-blue, stairs in a light
    shade of their team's colour, lift pads violet in cyan holes, the stairs' holes darker cyan); consoles, players and
    shots are marks and lines in team colours. It was a dark square with three dots. **Weigh in:** its reach and
    grid spacing.
19. **Gun 2 is the hyperbolic demo's rifle**: heat, overheat, head and body hits from `server_settings.yaml`. Its
    pulses reflect off the glass like the laser's shots.
20. **Pylons are Lunar Surface's beacon** (the user asked every level to share the hyperbolic demo's pylons): the
    `scifi_beacon` model scaled to each pylon box's height (5.2 m on the street, 4 m on the glass level, since the
    levels are only 7 m apart), and each console moved inside its pylon, as on Lunar Surface.
21. **Beacons are still geometry with per-console state in the uniforms**: rebuilding 4 x 15k triangles each frame
    doubled the GPU time and pushed the worst view to 22 ms.
22. **Map page picture from the centre, unrolled**: a sphere has no "from above"; from the centre every street and
    the glass level show at once, in the same projection as the live marks. It has no shadows (the renderer has none).
23. **Station look on the existing renderer**: metal textures are desaturated to read as steel; panel seams are
    drawn in the shader, not textures. No brick: the user asked for metal only. Blocks are rounded-corner boxes for
    drawing only (collision keeps the boxes, at most 0.35 m inside the corners). Reflections are faked from the core
    light and a station-floor colour with Fresnel, not traced. Panel frames, bolts, grime, hazard rings, floor strips, screens and LEDs are all shader or emissive
    detail, with no extra lights; each step's cost was within this VM's noise. The look is frozen at the user's
    request ("looks good enough").
24. **`lasers` in the map format** is optional and either one colour or two team colours, so Dust and Dark Forest
    can be golden, Snow Planet green, and Lunar Surface and City Sphere red and blue.
25. **Seeded variation in the map file** (`variation.seed`): the generator applies the same small jitter (positions
    0.04 rad, headings 0.15 rad, heights down to 85 %) to each team's half, so the halves stay antipodal and fair;
    the renderer uses the seed only for panel tint.
26. **Stairs, for W-119 (City Sphere demo)**: 48 steps a team, 0.148 m rise and 0.22 m tread (34°), 2.4 m wide, each
    step a box standing on the street so the treads stay level on the curve, the last 1.2 m deeper as a landing flush
    with the glass level. They start 3 to 4 m in front of the spawns (which face them, slightly to the right) and
    climb toward the team's own pylon, turned 0.14 rad north so their hole clears the glass covers; the landing ends
    about 5 m from the pylon. Seeded jitter like the blocks' (base ±0.02 rad, heading ±0.05 rad), mirrored. Their
    hole (0.151 rad) covers every step where a head would reach the glass; the game opens it only as wide as the
    stairs (34). Steps rather than a ramp: the user asked for
    stairs, treads don't slide a standing player, and the map format needs nothing new. Players on the ground step up
    onto a box at most 0.2 m higher (new; nothing else in the level is that low except the lift pads), so stairs climb
    at walking and sprinting speed (a test walks from every spawn at both). Look: lit station metal tinted violet
    like the lift pads (option A of three: A violet, B plain steel, C amber); glowing stairs read as a flat white
    shape. The stairs' holes get no hazard ring and don't lift a player. **Weigh in:** the colour, and a second
    flight per team toward the middle (the glass has room for 8 holes, all used now).
27. **Movement is Lunar Surface's, with the sprint 20 % faster than Lunar's** (W-126 (same play in every level) and
    W-119's "you should be able to run a little faster"): City Sphere now multiplies the map's speeds by
    `server_settings.yaml`'s `movement`, as Lunar does, and the map's numbers are Lunar's (walk 5, sprint 9, jump
    5.2 m/s at 6 m/s²). Walking was 5.0 m/s and is now 10 m/s; sprinting was 8.5 m/s and is now 21.6 m/s
    (`sprint_speed_per_map_speed: 2.4`; Lunar's 2.0 gives 18 m/s); the jump was 1.28 m in 1.02 s and is now Lunar's
    1.25 m in 0.58 s. "Run" is the sprint (`Shift`, "Run while held"), so walking only changed to match Lunar.
    Sprinting now drains Lunar's sprint energy (7.5 s). **Weigh in:** `sprint_speed_per_map_speed` is the number to
    tune.
28. **Spawns look up 0.25 rad**: inside the sphere level ground rises ahead, so a level view at the spawn was mostly
    deck (the W-126 audit: "spawns looking down at the deck").
29. **Lunar's beam speckle on City Sphere's drawn beams**: the share of speckled pixels per shot is Lunar's (the
    sniper's sampled per shot, the rifle's growing with heat, sent to other windows as on Lunar), with Lunar's pixel
    hash and its 1 black : 1 white : 8 dropout kinds. City draws beams as camera-facing quads, so they now blend as
    premultiplied light (unchanged for unspeckled pixels) so that a black speckle can darken the scene behind.
30. **Thrown wormholes in City Sphere are cheap portals** (W-126 (same play in every level); the user: "you can use
    heuristics ... even if it might be sometimes physically incorrect slightly"): Lunar's throw, colours, two per
    player, linking and destruction by enemy shots (the shared `thrown_wormholes.rs` and network), but in Euclidean
    space a mouth is a 1 m disc standing at eye height, drawn as a glowing ring over a dark disc instead of Lunar's
    lensed view through it (a second view per mouth would cost a render pass each). A player's middle or a shot
    crossing the disc comes out of the partner, mirrored through it as Lunar's mouths mirror rays; a shot passes at
    most 2 mouths. A mouth may not stand within 3 m of another or 0.6 m of a box.
31. **Dust, flashlight and third person are Lunar's, simplified for the rasterizer**: dust bursts are Lunar's
    (`dust.rs`, the same particles and timing) where a shot ends on the street or on the glass seen from the glass
    level, lit from above, the grains drawn as round specks rather than polyhedra; the flashlight is Lunar's cone and
    falloff added to each surface's light; the third-person camera is Lunar's boom (up 0.6 m, back 3 m), stopped
    0.25 m short of whatever a ray from the eye meets, with shots aimed at the crosshair's point.
32. **Lunar's beam looks**: the sniper's white blade (`exp(-q^3)`) in a team-coloured haze and the rifle's Gaussian
    laser over a halo, Lunar's formulas and brightnesses on City's camera-facing beam quads, now 0.16 m wide so the
    haze fits.
33. **Players spawn clear of teammates**: a window or bot takes the team's spawn farthest from the other players (two
    windows of one team both took the first spawn, on top of each other).
34. **A stairs' hole is only as wide as its stairs** (W-132 (City Sphere stairs hole)): the map's round hole (0.151
    rad, a radius of 2.6 m on the glass) reached 1.4 m past each side of the 2.4 m stairs, so shots passed the glass there
    and players on the glass level fell in beside the steps. The map keeps its round hole (the format has only round
    holes), and the game opens it only within the steps' half width (1.2 m) of the flight's middle plane, taken from
    the steps under the hole (`fit_hole_to_stairs`): in collisions, shots, thrown wormholes, the minimap, the map page
    and the drawn glass, whose glowing rim now follows the cut. A player under the glass, climbing, gets a body radius
    (0.4 m) more on each side, so the glass doesn't knock off the stairs a player whose feet hang over a step's edge
    (a test fails without it); on the glass level the glass holds a player up to the stairs' edge. The console bot's
    path planning still keeps clear of the whole round hole. **Weigh in:** this changes the look: the glass now covers
    the round hole's sides, and the hole's rim hugs the stairs. Cutting only the collisions would keep today's look
    but leave players standing, and shots stopping, on glass that isn't drawn.
35. **City Sphere's guns are Lunar Surface's code** (W-126 (same play in every level), the user: "the same guns
    everywhere"): `CitySession` keeps its guns in the shared `Weapons`, as Lunar's `Session` does, so the differences
    City had are gone, each now as on Lunar: (a) a shot is traced on every frame it hits, not once when fired, and
    hits weigh by `take_hit_weight` (the sniper's on every frame, the rifle's once a shot); the beam is still drawn
    once, from the shot's first frame, as Lunar's `Tracer` is; (b) shots stray from the crosshair (the sniper's
    noise, the rifle's heat), and the window sends the strayed aim while firing so other windows draw the same shot;
    (c) the rifle kicks the view up and alternately sideways and its jolt settles over `rifle.recoil_settle_s`, and
    the sniper has no jolt, instead of a fixed 0.25 s jolt for both; (d) no shot until
    `movement.shoot_after_moving_s` after the last walk or jump, and moving ends a shot under way; (e) the sniper
    fires on a click, not while the button is held, the rifle cools only while not firing, each gun keeps its own
    reload, and switching guns keeps the sniper's reload; (f) the bot fires a `Weapons` sniper (its stray, reload
    and per-frame hits) at a target it can see, instead of a hit sent at once. What City keeps: its tracing through
    the glass and wormholes, its beam drawing, and enemy wormholes hit once a shot.
36. **City Sphere moves by Lunar Surface's rules, crouch included** (W-126 (same play in every level)): `CityPlayer`
    keeps the shared `MoveState`, and `CityWorld::move_player` steps it first, as Lunar's `VoxelPlayer::step` does, in
    place of City's own sprint energy, head bob and hit slow. So City gains Lunar's crouch (`Ctrl`): it takes
    `movement.crouch_s`, lowers the eye to `crouch_eye_height_frac` of standing, walks at `crouch_speed_frac`, refills
    sprint energy at the crouched rate, stops sprinting, and lets the guns fire while walking crouched (a test compares
    the first three with Lunar's). Two things now follow Lunar's order instead of City's, each by one 1/60 s step: (a)
    the head bob steps by whether the player stood on the ground before the move (City's own code used after it), so
    the bob's phase differs from the first take-off on (positions don't); (b) a hit's slow lasts one more step, since
    the rules count it down after using it. City's play trace (`record_city_play_trace`) without crouching is
    otherwise byte for byte the same as before. As on Lunar, crouching doesn't shrink the body for collisions.
