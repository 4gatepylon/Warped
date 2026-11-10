# City Sphere in Rust

City Sphere (W-119), the purple, close-quarters level, playable in Rust with Metal: a satellite city inside a
hollow sphere in Euclidean space. You walk on the inside of the sphere (gravity points outward, so up is toward the
glowing core at the centre) through streets of buildings, under a glass shell that roofs them. Each team's stairs,
right in front of its spawns, and the lifts carry you up onto the glass level, the glass shell's inner side, where nothing is overhead: from there you can shoot straight
across the sphere, and shots reflect off the glass about its normal, up to 2 times by default. Below the glass it
is a normal shooter, and shots stop at the glass. Each team has one pylon, standing exposed on the glass level with no
cover right around it, with one console; a team wins when it has turned off the other team's console. Red's half is
the antipode of blue's.

This folder is a copy of [`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/README.md) with City Sphere
added beside it; the network, settings, guns, robots and overlay are that demo's (its README explains them), and it
still plays its own maps with `--map lunar-surface` or `--map house-demo`. Decisions made for the user are in
[`DECISIONS.md`](DECISIONS.md). The map is [`common/maps/city_sphere/`](../../common/maps/city_sphere/map.json),
written by [`tools/make_map.py`](tools/make_map.py).

## Play

Rust is Homebrew's rustup (`export PATH="$(brew --prefix rustup)/bin:$PATH"`); building SDL needs `cmake`
(`brew install cmake`), and Metal needs Xcode's Metal toolchain (`xcodebuild -downloadComponent MetalToolchain`).

Alone (the window hosts a game server on UDP port 47810; press `1` or `2` in the lobby to join blue or red, or pass `TEAM=blue`):

```bash
pushd demos/city_sphere_rust
make play-game TEAM=blue
```

Against a bot that wanders and shoots (second terminal, while the window above runs):

```bash
pushd demos/city_sphere_rust
make run-bot SERVER=127.0.0.1 TEAM=red BOT_S=300
```

Two players on one Mac, or on a network (`make serve-game` prints the address to join):

```bash
pushd demos/city_sphere_rust
make serve-game                      # terminal 1: the game server, no window
make join-game SERVER=127.0.0.1 TEAM=blue   # terminal 2: window 1
make join-game SERVER=127.0.0.1 TEAM=red    # terminal 3: window 2
```

Controls and play are Lunar Surface's (W-126 (same play in every level); `player_settings.yaml` and
`server_settings.yaml` are the hyperbolic demo's, except a faster sprint): `WASD` walk, mouse look (click to capture
the mouse), `Shift` sprint (Lunar's sprint energy), `Ctrl` crouch (Lunar's: lower and slower, and the guns fire while
walking crouched), `Space` jump (Lunar's 1.25 m in 0.58 s; hold it on a lift pad to
ride up; walk off at the top), walk up the stairs in front of your spawn to the glass level, left click shoots, right
click eases into the gun's zoom, `1` the laser, `2` the rifle (heats up; overheats; a window starts on it), wheel
switches gun, hold `E` aiming at the other team's console within reach to turn it off (10 s, Lunar's bar below the
middle), `Esc` opens the menu and frees the mouse (`Q`/`E` change page: scores, settings, map, credits, quit; `Esc` again
closes it), holding `Tab` shows the whole map over the game while you keep playing, `-` and `=` lower and raise the render resolution. The HUD has Lunar's health,
ammo and energy bars with their pulses, and Lunar's hit effects (red, blur and shake from the edges) whenever health
drops; an eliminated player sees "YOU DIED" and a countdown and comes back at a spawn after `respawn.delay_s` (3 s).
The minimap (top right, placed by the player settings' `minimap`) shows the sphere seen from its centre around you, the
way you face up, with straight Cartesian grid lines every 5 m: buildings, covers, each team's stairs in its colour,
lifts and holes, consoles in team colours (grey when off), other players and shots in team colours, and you as the
white arrow. The window's title shows your team, level, consoles still on, the gun's reload or heat and the frame
rate. Also as on Lunar Surface: `Q` throws your purple wormhole mouth, then your green one (walk or shoot through
one to come out of the other; enemy shots destroy a pair), `F` shines the flashlight while held, `V` switches to third
person, shots throw up dust where they hit a floor, and the sniper's beam is a white blade while the rifle's is a
laser. `make play-route ROUTE=wormholes` throws both mouths, shoots and walks through them.

### Flags for the match (W-123)

- `--serve --port <port> --level-result-file <path>`: when a team wins, write `{"winner": "blue"}` or
  `{"winner": "red"}` to `<path>` once, atomically (`<path>.partial`, then renamed), and keep the game won (no new
  game) until stopped.
- `--join <address> --port <port> --team blue|red`: join that team at once instead of waiting in the lobby.
- `--join <address> --port <port> --bot --exit-after-s <s> --team <team> --bot-objective consoles`: a bot without a
  window that walks on foot (up its stairs, then a planned path on the glass level) to each of the other team's
  consoles and holds `E` on it, aiming at it, until its team wins. The server counts a hold only within reach and while
  aiming, so a bot win shows the pylons can be reached on foot. A level result that can't be written stops the server
  with an error.
- The binaries are `city-sphere` and `city_sphere_server`; `--map city-sphere` is the default.

## How it works

- **Shared code**: the files every Rust level shares (geometry, the gun held in hand, the HUD's bars and overlay
  types, the maps' models and the robot body, the smooth shapes, the key bindings, finding assets) are in
  [`../warped_shared_rust/`](../warped_shared_rust/README.md), a library this crate depends on; the rest of the
  copied Lunar Surface code is still in `src/`.
- **World** (`src/city_world.rs`): reads the map (boxes as oriented boxes, the street sphere, the glass and core
  shells, the lift holes, spawns, consoles in the server's order). A player is feet, body axes (up toward the centre)
  and head pitch; each step turns the body the least way to keep up toward the centre, walks along the ground,
  applies gravity outward, and pushes out of the street, the glass (from whichever side the player is on, except in a
  hole) and the boxes (three spheres up the body); a player on the ground steps up onto a box at most 0.2 m higher (a
  stair). Holding jump in a lift hole (a hole over a lift pad, not the stairs') raises the player at 6 m/s to 1 m over
  the glass level and holds them there until they walk off. Speeds, jump and gravity follow Lunar Surface's
  `VoxelWorld::set_settings`: the map's numbers times `server_settings.yaml`'s `movement`. Before moving, each step
  runs the rules every level shares (`warped_shared::movement::MoveState`, kept in `CityPlayer::move_state`):
  crouching, sprint energy, the head bob's steps and a hit's slow (DECISIONS 36). The ignored test
  `record_city_play_trace` walks a player through a fixed script of keys on the City Sphere map and writes every
  step's state exactly (`PLAY_TRACE_FILE=<file>`; `PLAY_TRACE_CROUCH=0` and `PLAY_TRACE_HIT=0` leave out the crouch
  and the hit), to compare two builds byte for byte.
- **Shots** (`CityWorld::trace_shot`): a ray from the eye stops at the nearest of the street, a box, the core, another
  player (torso and head spheres) or the glass met from outside; the glass met from inside reflects it (`d - 2(d·n)n`)
  while bounces are left; the total length is capped at `glass.range_m`.
- **Guns** (`CitySession`, the shared `warped_shared::weapons::Weapons`): Lunar Surface's guns, the same code as
  there (DECISIONS 35): the sniper fires on a click when loaded, the rifle while held (heat, overheat, cooling);
  each shot strays from the crosshair by the sniper's noise or the rifle's heat, is traced on every frame it hits
  (the sniper's hit counts on each, the rifle's once a shot), kicks the view and jolts the gun (the rifle), and
  can't fire until `movement.shoot_after_moving_s` after the last walk or jump. The bot fires the same `Weapons`.
- **Rendering** (`src/render_city.rs`, `src/render_city.metal`): one Metal render pass into the shared renderer's
  render texture: the static scene (street and glass spheres, boxes, core) in buffers made once, then per frame the
  consoles and pylons (dark once off, pulsing while being turned off), the robots (the teal robot posed once per clip,
  tinted by team) and the beams (additive camera-facing quads), then the shared overlay (gun, crosshair, banner) and
  MetalFX upscaling when the render scale is below 1.
- **Game** (`src/city.rs`): the window, the in-process server (when not joining), the network (the hyperbolic demo's
  `net.rs`: state, hits, consoles, the winner), the bot, `--bench-views` and `--play-route`.
- **Settings**: `server_settings.yaml`'s new `glass` section (`max_bounce_count: 2`, `range_m: 90.0`,
  `reflectivity_frac: 1.0`, the share of the range left that each reflection keeps, W-158, per-map game settings) and
  `movement.walk_speed_per_map_speed: 1.2` and `movement.sprint_speed_per_map_speed: 1.44` (6 and 13 m/s, 40% slower than before, on the user's request); the rest are the hyperbolic demo's, line for line (a test checks it).
- **Stairs** (`tools/make_map.py`): 48 steps a team, each a box standing on the street, from 2 to 4 m in front of the
  spawns up through a hole in the glass to the glass level, about 5 m short of the team's pylon, with the map's
  seeded, mirrored jitter. The map's hole is round and wider than the stairs; the game opens it only as wide as the
  stairs under it (`fit_hole_to_stairs`), in collisions, shots and the drawn glass, and a body radius wider on each
  side for a player climbing (W-132 (City Sphere stairs hole), DECISIONS 34).

## Measure

```bash
pushd demos/city_sphere_rust
make bench-views WIDTH_PX=2560 HEIGHT_PX=1440 FRAME_COUNT=60   # six fixed views, pictures in .artifacts/bench_views/
make play-route                                                # the scripted route in the window, then its numbers
```

`--play-route` plays blue alone: ride a lift, fire four shots that reflect or cross, walk across the glass level to
red's console and turn it off (blue wins), then
print the frame times, GPU times, each part of the frame (`phases`) and winner as JSON. Its server doesn't announce
itself on the network, so macOS doesn't ask for Local Network access during a recording. Add `--no-vsync` to the
binary (`.build/release/city-sphere --play-route --no-vsync`) to time frames without waiting for the display.

## Results

Apple M4 Pro (virtual machine, 12 cores, 16 GB), macOS 26.6.2, rustc 1.99.0, 2026-11-08.

`make bench-views WIDTH_PX=2560 HEIGHT_PX=1440 FRAME_COUNT=60` (offscreen, full resolution, no upscaling):

| View | GPU median (ms) | GPU p99 (ms) | Frame incl. readback, median (ms) | p99 (ms) |
| --- | --- | --- | --- | --- |
| street_spawn | 2.60 | 3.60 | 4.21 | 5.02 |
| street_up_at_glass | 1.79 | 5.23 | 3.41 | 6.96 |
| glass_across | 1.82 | 5.03 | 3.17 | 6.37 |
| glass_edge_on | 1.83 | 4.83 | 3.09 | 6.14 |
| lift_up | 2.14 | 6.26 | 3.39 | 7.56 |
| glass_crowd_and_shots (8 robots, 8 reflected shots) | 6.34 | 8.50 | 13.44 | 16.05 |

These are with the finished station look (commit `f3685df`); every view's median frame is under 16.7 ms, so no
quality was cut. Match check: `--serve --level-result-file` with a blue `--bot --bot-objective consoles` and a red
bot wrote `{"winner":"blue"}` (through `<path>.partial` and a rename) after 11 s.

`--play-route` in a 1280x800 window (this VM's display has no Retina scaling), blue won. Frame interval in ms;
Lunar Surface is the hyperbolic demo's `--map lunar-surface --play-route warp-tour` (adaptive render scale) on the same
VM. "Before" is before frame pipelining (commit `6d4cd3b`):

| Route | Median | p99 | Worst |
| --- | --- | --- | --- |
| Lunar Surface, before, vsync | 16.7 | 33.0 | 322 |
| Lunar Surface, after, vsync | 16.7 | 23.0 | 173 |
| Lunar Surface, after, `--no-vsync` | 12.2 | 18.5 | 161 |
| City Sphere, before, vsync | 16.7 | 21 to 24 | |
| City Sphere, after, vsync | 16.7 | 21 to 28 | |
| City Sphere, after, `--no-vsync` | 2.5 | 7.1 | |

City Sphere's GPU time is 2.1 ms median and 4 to 6 ms p99. The route's JSON also gives each part of the frame: the
game's own work (input, update, building the frame, overlay) stays under 1 ms at p99 in total apart from input
(2.2 ms), and nearly all the rest is `render_and_present`, waiting in `CAMetalLayer::nextDrawable` for the display.
With `--no-vsync` only 12 of 16,856 frames take over 17.5 ms, so the vsync p99 is this VM's display pacing, not the
game; robot posing, network polling and allocations don't show up in the slow frames. Only a real Mac display can
say whether the vsync p99 meets 16.7 ms.

## Station look, pylons, map page, photos

- **Look**: a space station: steel deck plates (`metal_plate`) with lit cyan panel seams, buildings in Poly Haven CC0
  `metal_plate_02` and `blue_metal_plate` (`make download-assets` puts them in `assets/textures/`), metal highlights,
  and the glass as a window. Both shells are Goldberg grids (hexagons and 12 pentagons): the street's cells are
  about 7 m wide with faint seams, the glass's about 3 m with crisp white frames. Wall panels have metal frames,
  corner bolts, varied roughness, grime near the floor and a thin glowing strip 0.3 m up; lifts have hazard rings;
  seeded screens and status lights glow on walls; a soft halo surrounds the core. Blocks and covers are rounded-corner boxes (radius 0.35 m; collisions
  still use the boxes). Metal looks reflective through a fake reflection (the core light and a dark station floor
  looked up along the mirrored view direction, with Fresnel), so it casts no extra rays; the pylons keep their own
  textures.
- **Variation**: the map's `variation.seed` jitters block and cover positions, headings and heights a little in
  `tools/make_map.py` (the same jitter on each team's half, so it stays fair) and tints each block's panels; the
  shader adds blocky panel wear.
- **Pylons**: the hyperbolic demo's `scifi_beacon` (as on Lunar Surface) with its base colour and emissive textures,
  recoloured per team; it pulses while its console is being turned off and goes dark when off. Each console sits
  inside its pylon. The beacons are drawn once with the still scene; each console's state goes in the uniforms.
- **Laser colours** come from the map's optional `lasers` (`common/maps/README.md`); City Sphere's are blue and red.
- **Map page** (held `Tab`, or `Esc` then `E`): the whole sphere seen from its centre, unrolled (longitude across, latitude down),
  rendered by `make map-picture` into `assets/map_picture.png` and cached; consoles, players and you are drawn over
  it. Without the file it falls back to a flat drawing.
- **Photos**: `make photos WIDTH_PX=2560 HEIGHT_PX=1440` renders photogenic views into `.artifacts/photos/`.
- **Rifle check**: `make play-route ROUTE=rifle` with `make run-bot SERVER=127.0.0.1 TEAM=red BOT_OBJECTIVE=target`:
  20 shots to overheat, 2.0 s to cool, the target eliminated by 2 head hits and then by 12 body hits.

`make bench-views WIDTH_PX=2560 HEIGHT_PX=1440 FRAME_COUNT=60`, run back to back without and with the rounded blocks,
fake reflections and variation: GPU median 1.33 to 1.77 ms before and 1.17 to 1.71 ms after in the five plain views
(second run; a first run read 1.49 to 2.04 ms); eight robots and eight reflected shots 7.70 ms GPU and 15.87 ms
frame (p99 21.8 ms) before, 7.82 ms and 15.73 ms (p99 21.5 ms) after. The difference is within this VM's run-to-run
noise; the crowd view's p99 frame is over 16.7 ms in both, from posing eight robots on the CPU.

## Known limitations

- On this VM the frame interval's p99 with vsync (21 to 28 ms) is over 16.7 ms because of the display's pacing (see
  Results); not yet measured on a real Mac display.
- Posing robots costs about 1 ms of CPU per clip each frame; a crowd of eight is the worst view's main cost.
- No zone walls, thrown wormholes or shadows in City Sphere.
- Thrown wormholes are portals: a ring and a dark disc, not Lunar Surface's lensed view through the mouth (DECISIONS
  30).
