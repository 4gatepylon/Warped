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

7. **Lifts, not stairs, join the levels.** Six holes in the glass (two near each team's spawn, two in the middle, each
   the antipode of another) sit over glowing lift pads: hold `Space` on a pad to ride up, walk off onto the glass; walk
   into a hole on the glass level to drop to the street. Stairs or ramps on a curved shell are more work and easier to
   get stuck on. **Weigh in:** stairs like the WebGL version's, or jump pads.
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
15. **`--bot-objective consoles` doesn't walk**: the server takes a client's word for which console it holds, as the
    W-123 (match progression demo) interface allows.
16. **Two frames in flight**: the window no longer waits for the GPU to finish a frame before drawing the next, so
    the CPU and GPU overlap. Lunar Surface's vsync p99 fell from 33 to 23 ms on this VM; City Sphere's (21 to 28 ms,
    varying between runs) didn't clearly change, because its frames wait for the display, not the GPU. `--no-vsync`
    is a measuring flag only; play keeps vsync on.
17. **The scripted route's server doesn't announce itself** on the network, so recording it needs no Local Network
    permission; every other server still announces itself for `--join auto`.
18. **The minimap is a view of the whole sphere from outside, centred on the player** (consoles in team colours,
    grey when off, robots, the player in white), because the hyperbolic minimap draws the hyperbolic plane. The user
    should weigh in on its look.
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
    light and a station-floor colour with Fresnel, not traced; hazard stripes and metal frames are not done yet.
24. **`lasers` in the map format** is optional and either one colour or two team colours, so Dust and Dark Forest
    can be golden, Snow Planet green, and Lunar Surface and City Sphere red and blue.
25. **Seeded variation in the map file** (`variation.seed`): the generator applies the same small jitter (positions
    0.04 rad, headings 0.15 rad, heights down to 85 %) to each team's half, so the halves stay antipodal and fair;
    the renderer uses the seed only for panel tint.
