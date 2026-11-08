# City Sphere in Rust

City Sphere (W-119), the purple, close-quarters level, playable in Rust with Metal: a satellite city inside a
hollow sphere in Euclidean space. You walk on the inside of the sphere (gravity points outward, so up is toward the
glowing core at the centre) through streets of buildings, under a glass shell that roofs them. Lifts carry you up
onto the glass level, the glass shell's inner side, where nothing is overhead: from there you can shoot straight
across the sphere, and shots reflect off the glass about its normal, up to 2 times by default. Below the glass it
is a normal shooter, and shots stop at the glass. Each team has two pylons, one on the street and one on the glass
level, each with one console; a team wins when it has turned off both of the other team's consoles. Red's half is
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

Controls are the hyperbolic demo's (`player_settings.yaml`): `WASD` walk, mouse look (click to capture the mouse),
`Shift` sprint, `Space` jump (hold it on a lift pad to ride up; walk off at the top), left click shoots, right click
zooms, `1` the laser, `2` the rifle (heats up; overheats), wheel switches gun, hold `E` near the other team's console
to turn it off (10 s), `Esc` releases the mouse (close the window to quit), `Tab` opens the menu (`Q`/`E` change page:
scores, settings, map, credits), `-` and `=` lower and raise the render resolution. The minimap (top right) shows the
whole sphere centred on you: consoles in team colours (grey when off), robots, and you in white. The window's title
shows your team, level, consoles still on, the gun's reload or heat and the frame rate.

### Flags for the match (W-123)

- `--serve --port <port> --level-result-file <path>`: when a team wins, write `{"winner": "blue"}` or
  `{"winner": "red"}` to `<path>` once, atomically (`<path>.partial`, then renamed), and keep the game won (no new
  game) until stopped.
- `--join <address> --port <port> --team blue|red`: join that team at once instead of waiting in the lobby.
- `--join <address> --port <port> --bot --exit-after-s <s> --team <team> --bot-objective consoles`: a bot without a
  window that holds `E` on the other team's consoles until its team wins (it doesn't walk; the server takes its word).
- The binaries are `city-sphere` and `city_sphere_server`; `--map city-sphere` is the default.

## How it works

- **World** (`src/city_world.rs`): reads the map (boxes as oriented boxes, the street sphere, the glass and core
  shells, the lift holes, spawns, consoles in the server's order). A player is feet, body axes (up toward the centre)
  and head pitch; each step turns the body the least way to keep up toward the centre, walks along the ground,
  applies gravity outward, and pushes out of the street, the glass (from whichever side the player is on, except in a
  hole) and the boxes (three spheres up the body). Holding jump in a lift hole raises the player at 6 m/s to 1 m over
  the glass level and holds them there until they walk off.
- **Shots** (`CityWorld::trace_shot`): a ray from the eye stops at the nearest of the street, a box, the core, another
  player (torso and head spheres) or the glass met from outside; the glass met from inside reflects it (`d - 2(d·n)n`)
  while bounces are left; the total length is capped at `glass.range_m`.
- **Rendering** (`src/render_city.rs`, `src/render_city.metal`): one Metal render pass into the shared renderer's
  render texture: the static scene (street and glass spheres, boxes, core) in buffers made once, then per frame the
  consoles and pylons (dark once off, pulsing while being turned off), the robots (the teal robot posed once per clip,
  tinted by team) and the beams (additive camera-facing quads), then the shared overlay (gun, crosshair, banner) and
  MetalFX upscaling when the render scale is below 1.
- **Game** (`src/city.rs`): the window, the in-process server (when not joining), the network (the hyperbolic demo's
  `net.rs`: state, hits, consoles, the winner), the bot, `--bench-views` and `--play-route`.
- **Settings**: `server_settings.yaml`'s new `glass` section (`max_bounce_count: 2`, `range_m: 90.0`); the rest are
  the hyperbolic demo's (`sniper`, `consoles`, `respawn`, ...).

## Measure

```bash
pushd demos/city_sphere_rust
make bench-views WIDTH_PX=2560 HEIGHT_PX=1440 FRAME_COUNT=60   # six fixed views, pictures in .artifacts/bench_views/
make play-route                                                # the scripted route in the window, then its numbers
```

`--play-route` plays blue alone: ride a lift, fire four shots that reflect or cross, walk across the glass level to
red's glass console and turn it off, drop through a hole to red's street console and turn it off (blue wins), then
print the frame times, GPU times, each part of the frame (`phases`) and winner as JSON. Its server doesn't announce
itself on the network, so macOS doesn't ask for Local Network access during a recording. Add `--no-vsync` to the
binary (`.build/release/city-sphere --play-route --no-vsync`) to time frames without waiting for the display.

## Results

Apple M4 Pro (virtual machine, 12 cores, 16 GB), macOS 26.6.2, rustc 1.99.0, 2026-11-08.

`make bench-views WIDTH_PX=2560 HEIGHT_PX=1440 FRAME_COUNT=60` (offscreen, full resolution, no upscaling):

| View | GPU median (ms) | GPU p99 (ms) | Frame incl. readback, median (ms) | p99 (ms) |
| --- | --- | --- | --- | --- |
| street_spawn | 1.10 | 1.63 | 2.35 | 3.06 |
| street_up_at_glass | 1.35 | 2.93 | 2.57 | 4.31 |
| glass_across | 1.37 | 2.85 | 2.59 | 4.08 |
| glass_edge_on | 1.29 | 2.71 | 2.53 | 3.97 |
| lift_up | 1.49 | 3.75 | 2.69 | 5.02 |
| glass_crowd_and_shots (8 robots, 8 reflected shots) | 6.17 | 6.83 | 12.43 | 14.24 |

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
  and the glass as a window with metal ribs.
- **Pylons**: the hyperbolic demo's `scifi_beacon` (as on Lunar Surface) with its base colour and emissive textures,
  recoloured per team; it pulses while its console is being turned off and goes dark when off. Each console sits
  inside its pylon. The beacons are drawn once with the still scene; each console's state goes in the uniforms.
- **Laser colours** come from the map's optional `lasers` (`common/maps/README.md`); City Sphere's are blue and red.
- **Map page** (`Tab`, then `E`): the whole sphere seen from its centre, unrolled (longitude across, latitude down),
  rendered by `make map-picture` into `assets/map_picture.png` and cached; consoles, players and you are drawn over
  it. Without the file it falls back to a flat drawing.
- **Photos**: `make photos WIDTH_PX=2560 HEIGHT_PX=1440` renders photogenic views into `.artifacts/photos/`.
- **Rifle check**: `make play-route ROUTE=rifle` with `make run-bot SERVER=127.0.0.1 TEAM=red BOT_OBJECTIVE=target`:
  20 shots to overheat, 2.0 s to cool, the target eliminated by 2 head hits and then by 12 body hits.

`make bench-views WIDTH_PX=2560 HEIGHT_PX=1440 FRAME_COUNT=60` with the station look and beacons: GPU median 1.35 to
1.74 ms in the five plain views; eight robots and eight reflected shots 7.2 ms GPU, 14.8 ms frame (p99 15.8 ms).

## Known limitations

- On this VM the frame interval's p99 with vsync (21 to 28 ms) is over 16.7 ms because of the display's pacing (see
  Results); not yet measured on a real Mac display.
- Posing robots costs about 1 ms of CPU per clip each frame; a crowd of eight is the worst view's main cost.
- No zone walls, thrown wormholes or shadows in City Sphere. The menu's MAP page has no picture of City Sphere.
- Lifts instead of the WebGL version's stairs (see `DECISIONS.md`).
