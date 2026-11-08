# Hyperbolic wormholes in Rust

A Rust port of the WebGL demo in [`../hyperbolic_wormholes/`](../hyperbolic_wormholes/README.md), to see how
fast its ray tracer runs natively. It is the same scene and the same light paths (hyperbolic space H³, R = 8, three
wormholes), with gravity and a laser added, drawn either on the GPU with Metal (the default) or on every CPU core.
It is a demo for measuring speed, so it does no more than that.

Features the user has asked for, and who is working on which, are in
[`USER_WISHLIST_CHECKLIST.md`](USER_WISHLIST_CHECKLIST.md); [`AGENTS.md`](AGENTS.md) says how to claim one and mark it
done.

## Play

```bash
make serve-game
make join-game SERVER=127.0.0.1
```

- Playing is only on a game server: `make serve-game` runs one (Lunar Surface by default), and every player joins
  it with `make join-game` (`SERVER=127.0.0.1` on the same computer, or the address the server prints). There is no
  local demo or host-and-play command; without a server, only the tools below run.
- Click to capture the mouse (`Esc` releases it; `Esc` again asks "Do you want to exit?": `Return` or `Y` exits,
  `Esc` or `N` goes back to the game). The window's close button quits at once.
- Every key below is the default. [`player_settings.yaml`](player_settings.yaml) lists them all with your other own
  choices (mouse speed, head bob, motion blur, how a hit looks); a copy is passed with `PLAYER_SETTINGS=mine.yaml`
  (`--player-settings`) to `make join-game`.
- `Tab` opens the menu on its SCORES page (below); `E` turns to the settings page, of those choices: `Up`/`Down` pick a row, `Return` then any key, button or the wheel rebinds
  an action, `Left`/`Right` change a number, and its last row saves them to `~/.config/warped/player_settings.yaml`
  (or the `--player-settings` file), which later games load by themselves. `Esc` or `Tab` closes it. Your player
  stands still while it is open, but the game goes on.
- `Q` and `E` turn the menu's pages: SCORES, SETTINGS (above), MAP and CREDITS. MAP is the whole map from above with you on it as a disk in
  your team's colour pointing the way you face: the ground's relief, structures brighter the higher they are,
  consoles as small squares, wormhole mouths as rings, the black hole, the spawns, and on Lunar Surface the team
  zones' edges (dashed) and the playable boundary. The map is drawn in its Fermi coordinates
  (`common/maps/README.md`), so things far above and below its middle line look squeezed from side to side: that is
  the hyperbolic plane. The SCORES page, where the menu opens, shows in a game on a server how many players there are and each
  one's team, kills and deaths in this game, sorted by team and then kills, with your line highlighted and each
  team's totals. The MAP page also rings everything within the minimap's reach of you (60 m by default); the rest is
  darker and blurred. The ring is a hyperbolic circle, so on the map it comes out lopsided. In a game on a server,
  every other player in the game shows as a smaller dot in their team's colour, ringed in dark grey, near or far.
  The last page, CREDITS, says where the game's assets come from: their authors, sources and licenses
  (`src/credits.rs`).
- The minimap, in the top right, is a Poincaré disk of the ground within 60 m of you, turned so you face up (the
  white arrow in the middle): faint lines every 10 m and paths away from you bend into arcs, as straight lines do
  in that picture of the hyperbolic plane. Other players show as dots and laser shots as lines, in their team's
  colours. `minimap` in `player_settings.yaml` hides it, moves it to another corner (`top_left`, `top_right`,
  `bottom_left` or `bottom_right`), or changes its size and reach.
- `W`/`A`/`S`/`D` walk, `Shift` runs, `Space` jumps (hold it on the house for the jetpack), click shoots. On the
  house you collide with the ground only.
- Your path bends near a mouth like light does, and the view turns with it, so walking at a mouth's dark disk takes
  you through. For the moment you are inside the mouth (within about 3.6 m of its centre), moving, your view's
  colours shift as if you moved near light speed (D84, D105): a light cyan, almost white, walking forward, a dark red
  walking backward, crisp, with dark staying dark. It is strongest at the throat and with speed, none standing
  still, and gone once you are 3.6 m out. `view.wormhole_tint_frac` in `player_settings.yaml` (0 to 1, also in the
  menu) sets its strength (0 for none). Only you see your own shift (Metal renderer, voxel maps).
- Click (once the mouse is captured) to fire a laser shot for a quarter of a second, standing still or crouched (hold
  `Ctrl`: a lower view and half speed); not while walking upright, running, nor in the air on Lunar Surface. The
  wand then reloads for 2 s (`sniper.reload_s` in `server_settings.yaml`): its ammo bar, one block above the energy
  bar, is empty until it can fire again.
  The mouse wheel or `1`/`2` switch to gun 2, the rifle, which fires 8 pulses a second while the button is held,
  eliminates with 2 hits to the head or 12 to the body, and overheats after 20 shots. In hand, the sniper is the
  energy revolver and the rifle a wooden bolt-action rifle with a scope (Poly Haven, CC0; D105, for W-96 (a
  different rifle model)), drawn small so its scope, bolt and stock show in the corner.
  Each shot strays a little from the crosshair, by a random angle sampled for that shot (D89): with the sniper
  usually by a hair (3 mrad on average, at most 15), with the rifle by up to 2 mrad cold, growing with its heat to 20
  mrad and shrinking as it cools (`noise_*_rad` under `sniper` and `rifle` in `server_settings.yaml`).
  The sniper's beam is a white lightsaber blade and the rifle's a laser in the team's colour (D99). Each shot's beam
  also flickers with shot noise: every frame a random share of its pixels is speckled, 1 in 10 of them black, 1 in 10
  white and 8 in 10 dropping out to show what is behind, while the rest is the normal beam. The share is random per
  sniper shot (0.2 on average, at most half the beam) and rises with the rifle's heat, from none cold to half the beam
  at full heat (`beam_speckle_*_frac` and `beam_speckle` in `server_settings.yaml`).
  You can't shoot while running (sprinting while moving): the wand in your right hand (a 3D prism with a tip in your
  team's colour, drawn with the Metal renderer) drops and swings with your stride, and points ahead again when you
  walk.
  The laser is a light ray from your hand that bends near mouths and goes through them, and through glass; the HUD
  says what it hits. On Lunar Surface the habitat modules' and walkways' windows are glass in real openings: you
  see out of them and shoot through them, but can't climb through them.
  On Lunar Surface, a shot into the ground (yours or anyone's) throws up grey-brown dust and a puff of mist where
  it lands: the grains, small tumbling polyhedra, fly up to a few meters and fall back, and all of it fades within
  2 s (D81).
- The bars in the bottom left (health in a game on a server, the gun's ammo, and on Lunar Surface the sprint energy)
  are segments that lean like `/`: empty ones dark glass outlined in the bar's colour, filled ones translucent,
  glossy and glowing (D85). While health or energy changes, soft, wide swells of light run over the bar's filled
  part: reddish-orange down the health bar from the last hit point after a hit, pale blue down the energy bar from
  the last filled slot while you sprint, and fainter ones up toward the filled end while it refills. They are as
  strong, fast and frequent as the value's smoothed rate of change (D94); the ammo bar has none. The CPU renderer's
  window draws the bars as flat boxes.
- Holding the right button aims: the view zooms in, 2.5 times with the sniper (gun 1) and 1.5 times with the rifle
  (gun 2), over 0.15 s, and the mouse turns it more slowly by the same factor (`aim` in `server_settings.yaml`).
- Holding `F` shines a flashlight: a cone of light from your eye (on voxel maps; others don't see it yet).
- `Q` throws your purple wormhole mouth, the next press your green one, then purple again, and so on, on both maps.
  A throw is lobbed under its own gravity, drops to the floor below whatever it hits, and hovers there at eye
  height facing back the way it flew; once both are down they link, and any player or laser that goes into one
  comes out of the other (`wormholes` in `server_settings.yaml`). The view through a thrown mouth is tinted its
  colour, and a mouth in the air, or down while its partner isn't, glows as a small ball. A throw fizzles if it
  never lands, or lands too close to a wall or another warp; your mouths disappear when you die.
  A shot that goes through an enemy's mouth (or hits their lone mouth's ball) makes both their mouths flash red;
  2 sniper shots or 5 rifle shots destroy the pair (`wormholes` in `server_settings.yaml`).
- `V` switches between first and third person (on voxel maps). The camera eases out over 0.3 s to 3 m behind and
  0.6 m above your head, looking where you look, and you see your own body as the others see it, without the gun in
  hand. It pulls in toward your head when the ground or a wall is in the way (in a 2 m walkway it stays at your
  head). Shots still leave your eye, turned toward the point under the crosshair, so the beam runs from your head to
  the crosshair; holding the right button to aim brings the camera back to your eye (D104).
- On Lunar Surface the crater's wall is the map's edge: climbing it, you slow down without end and never get past
  halfway up (`map_edge` in `server_settings.yaml`), while your view stretches out from the middle of the screen
  (`view.edge_stretch_frac` in `player_settings.yaml`). Walking back in is at full speed.
- Lunar Surface's space is 1.5 times as curved as its map file says: its curvature radius is 26.7 m, not 40 m
  (`space.curvature_per_map_curvature` in `server_settings.yaml`, from 1 to 2, default 1.5). Buildings, players and
  distances along the middle line keep their size in meters; far things shrink faster with distance, and the
  structures away from the middle line spread apart along it. At 2, surfaces more than about 90 m from the middle
  speckle. The house demo keeps its own curvature (D100).
- `-` and `=` lower and raise the render resolution. Metal starts at the window's full resolution and,
  on frames that take more than about 14 ms of GPU time (close to a mouth), ray-traces fewer pixels and upscales
  them with MetalFX, down to half per side, until you press `-` or `=`. The CPU starts at 0.35 per side.
- The HUD shows the frame rate, the render time and, with Metal, the GPU's own time and the render scale; with Metal
  it is in the window's title.
- `--width-px` and `--height-px` set the window's starting size (`cargo run --release -- --width-px 2560
  --height-px 1400`); `--render-scale 1` turns upscaling off.

The three wormholes start ahead on the left (to the house door), behind you by the blue pylon (to the red pylon
about 50 blocks down the rails), and far left (to high above the rails, where you fall back to the ground).

## Maps

```bash
make serve-game MAP=house-demo
make join-game MAP=house-demo SERVER=127.0.0.1
cargo run --release -- --map lunar-surface --bench-views --view black_hole
```

- The demo reads its maps from [`common/maps/`](../../common/maps/README.md) (the portable map format):
  `house-demo` (the default: the house, rails and three wormholes, R = 8) or `lunar-surface` (the Lunar Surface level of
  [`../levels/lunar_surface/`](../levels/lunar_surface/README.md), R = 40 m in its map file, played at 26.7 m: see
  below, with its black hole and wormhole).
- Lunar Surface is drawn by a Metal port of that level's ray tracer (`render_voxel.metal`): the same voxel walks, warp
  steps, lighting with soft sun shadows, lamps, ground, stars and planet, so it looks like the level (plus a thin haze
  and stronger bloom, W-76), with the house's adaptive sampling (interpolating ray ends on one block face, the ground or the sky, D37). Walking, jumping,
  flying, the laser and multiplayer follow the level's rules there (a short, quick jump: 1.25 m high, high enough for a block, in 0.58 s, a third of the level's jump time, so about 30 m/s² of gravity; walking and sprinting twice as fast as the level, `Shift` sprinting on an energy bar that empties in 7.5 s and refills in 7.5 s crouched or 15 s walking or standing, a head bob, jumping onto blocks with `Space`, moving
  along light paths into the wormhole, falling into the black hole sends you back to spawn). Players start on their
  team's spawn (blue, then red, alternating) and the laser is blue or red by team. The CPU renderer draws the house only.
- Spaceships fly over Lunar Surface, purely for looks (D82): a ring ship and three delta-wing fighters in a V, each
  crossing the sky in 2 to 5 minutes. They follow the UTC clock, so every player sees the same ships.
- `--view <name>` starts the window at one of the level's viewpoints (`blue_spawn`, `black_hole`, `wormhole`,
  `wormhole_exit`, `einstein_ring`, `overview`, and the others `--bench-views` renders).
- `--bend-strength <0..1>` draws the map under the reality bend (W-68, `bend.rs`) in the window, `--bench-views` and
  `--view`: the map's edges curl up into a bowl, and at 1 it closes into the inside of a sphere. The Metal ray tracer
  walks each ray through the bent world in short pieces, halved until each one's map chord follows the bent path within
  2 cm (or half a pixel far away), and traces every piece through the unbent map; flat frames use the unchanged tracer
  (`render_voxel.metal` is compiled with and without `HAS_BEND`).

## Play together

Everyone on the same network can play in one game. A game server, without a window, keeps the players in step;
every player, the one who started it included, is a client with a window. [`PLAYTEST.md`](PLAYTEST.md) has the steps
for a fresh Mac, the commands for each player and the known caveats.

```bash
make serve-game MAP=lunar-surface
make join-game MAP=lunar-surface
make join-game MAP=lunar-surface SERVER=192.168.1.20
```

- `make serve-game` (`--serve`) runs only the server and prints this computer's address with the commands that join
  it, so it can run on a computer nobody plays on; players on that computer join it too.
- `SERVER_SETTINGS=my_server.yaml` (`--server-settings`) gives the server a copy of
  [`server_settings.yaml`](server_settings.yaml) with other numbers that shape play (speeds, jump, both guns,
  consoles, respawn, the hit slowdown, the flashlight, thrown wormholes, the curvature); it sends them to every
  player who joins.
- `make join-game` finds a server on the network by its announcement (`--join auto`); give the server's address
  (`SERVER=`) if that fails.
- On macOS, the host and every joining computer must allow Local Network access for the app the game runs from
  (the Terminal, say) when asked; without it, other computers can't reach the game and `auto` finds nothing.
- Joining opens a lobby with each team's head count; `1` joins blue and `2` joins red (bots join the smaller team).
- Players start on a ring around their team's spawn and see each other as coloured boxes, bent through the wormholes like
  everything else. Everyone's laser is drawn. A laser that reaches another player removes them from the game: their
  view darkens and says "YOU DIED" with a countdown, and they come back at their spawn point 3 seconds later.
- On Lunar Surface each team has three pylons with one console each, and a team wins by turning off all three of the
  other team's consoles (hold `E` on one for 10 s within 3 m).
  Consoles pulse once a second while on, faster and faster while someone holds `E` on them (up to 8 times a second
  just before they turn off, the same in every window), easing back down over about a second if the hold stops, and
  go dark when off. After a win, `Enter` in the server's terminal starts a new game.
- Up to 8 players. The UDP port is 47800 (`PORT=` or `--port`); discovery uses 47801.
- `--join <address> --bot --exit-after-s <seconds>` plays by itself without a window (it walks until another player
  is within 12 blocks, then turns toward them and fires), to test a game; it saves its view at each hit under
  `.artifacts/bot/<run_id>/`.

## Measure

```bash
make bench-views WIDTH_PX=2560 HEIGHT_PX=1600
make bench-views RENDERER=cpu
make run-autopilot
```

- `bench-views` renders eight fixed views without a window (spawn, aimed at mouth 1A, 2.5 blocks from its centre,
  0.8 blocks from its throat, above the rails, high above spawn, looking sideways 0.4 blocks from 1A's centre, and
  the laser through 1A), times each frame, and
  writes the median and fastest times and a PNG of each view to `.artifacts/bench_views/<run_id>/`.
- `run-autopilot` plays a scripted route without a window at a fixed 1/60 s step: walk into 1A, jetpack and land,
  fly into 2A, walk into 3A and fall from above the rails, and fire the laser through 1A. It writes every frame's
  time, render time, height and events to `frames.csv`, every 30th frame as a PNG, and a summary of each leg to
  `.artifacts/autopilot/<run_id>/`.
- Both render at full resolution unless given `--render-scale` (`0.5` ray-traces half the pixels per side and
  upscales them with MetalFX), and add `--json` and `--label <name>` when run directly (`cargo run --release --
  --help`).
- `--sampling every-pixel` makes Metal trace every pixel instead of interpolating where it can (see below).
- `--export-map <folder>` writes the world as a portable map (`common/maps/house_demo/` is its output; see
  `common/maps/README.md`).
- `--exit-after-s 8` opens the real window, plays for 8 seconds without input, closes it and prints the frame rate,
  GPU time and render scale.
- `--play-route` plays the scripted route in the real window, one 1/60 s step per frame, then closes it and prints
  the frame times (median, 99th percentile, worst, how many took longer than 1/60 s) and the render scale it used
  (`cargo run --release -- --play-route --width-px 2560 --height-px 1440`). `--route mouth-tour` plays a harder
  tour instead: the eye circles each of the six mouths from 5 blocks to 0.45 (inside its jump sphere, just outside
  the throat) and back, while the view swings up to 46° sideways and 23° up and down. On `--map lunar-surface`,
  `--route warp-tour` circles the black hole and both wormhole mouths the same way, from 80% of each one's reach to
  10% (19 m to 2.4 m from the black hole, 12 m to 1.5 m from a mouth) and back, over 600 frames each.

## How it works

- `thrown_wormholes.rs`: each player's purple and green mouths: the lobbed throw, where a mouth hovers once down,
  and which colour is next. Each map adds the linked pairs as mouths of its own (after the map's), and
  `net.rs` sends every player's mouths in each snapshot.
- `geometry.rs`: Lorentz-matrix maths in double precision (the hyperboloid model, as in the WebGL demo).
- `world.rs`: the scene: hyperbolic boxes grouped in clusters, and the wormholes' mouths with their gluing maps.
- `scene_data.rs`: each frame, packs the scene into rows of float 4-vectors, one row in the camera's coordinates and
  one per mouth, so a ray that goes through a mouth stays in small coordinates (the same layout as the WebGL demo's
  float texture).
- `render_cpu.rs`: the ray tracer, a port of the WebGL fragment shader, run over every core with rayon. It also
  draws the laser: each straight piece of a ray is tested against the beam's pieces in Klein coordinates, where
  geodesics are straight lines.
- `render_metal.metal` and `render_metal.rs`: the same ray tracer in Metal Shading Language, compiled when the
  program starts, with three shortcuts of its own (below). Each frame copies the scene rows to the GPU, ray-traces
  into a texture, upscales it with MetalFX when the render scale is below 1, and copies it into the window's Metal
  layer, without waiting for the GPU.
- `physics.rs`: walking, gravity, the jetpack and flying, crossing throats, aiming, and tracing the laser (with the
  renderer's own ray tracer, so the laser hits exactly what you see).
- `net.rs`: the game server and client, over UDP: joins, a snapshot of every player 60 times a second, hits, each
  player's kills and deaths 4 times a second, and an announcement on the network for `--join auto`.
- `multiplayer.rs`: the other players as boxes in a per-frame copy of the world, everyone's laser, hits, spawn
  points, and the test bot.
- `map.rs`: reads a map of the portable format; `map_export.rs` writes the house as one (`--export-map`).
- `voxel_world.rs`: a voxel map on the CPU, ported from the Lunar Surface level: structures, warps, the voxel walk,
  the laser's light path, and walking on the map. `render_voxel.metal` and `render_voxel.rs` draw it on the GPU.
- `voxel_clearance.rs`: each warp's clearance grid, a lower bound on the distance to the nearest block, so the GPU
  skips blocks a bent step can't reach.
- `dust.rs`: the dust and mist a shot throws up where it hits the ground, which `render_voxel.metal` draws in the
  world (D81).
- `main.rs`: the command line, the SDL3 window and HUD, and the benchmark and autopilot runs.

Distances are compared through q = 4 sinh²(d/2), computed from the difference of two points, instead of the
distance itself, which saves most of the `asinh` calls that dominated the first CPU profile. And a ray only checks
whether it is inside a throat when the bending sum, which already measures every mouth, finds one that close, which
made the Metal shader about 30% faster.

Most of the rest of the speed-up comes from not testing boxes a ray can't reach. Each cluster of boxes (the house,
a pylon, a tree) has a bounding box in its own frame, and a ray skips the whole cluster unless it reaches that box.
The Metal shader also keeps, per ray, a bit mask of the clusters within 3 blocks and how far the ray can go before
another could matter, so most steps test no boxes at all. All of these leave every pixel the same; see `DECISIONS.md`
(D12, D17, D18) for what each gained.

The Metal shader then takes three shortcuts the CPU renderer doesn't, so the CPU stays the reference:

- Adaptive sampling: it traces every fourth pixel each way, fills in every second pixel by interpolation where
  those samples hit the same surface and their hit points vary almost linearly (tracing the rest), then does the
  same for the remaining pixels. Interpolated pixels are still shaded one by one, so textures stay sharp, and
  whatever can't be interpolated is traced exactly in separate, densely packed passes (D21).
- Fourth-order (Runge-Kutta) steps from 3 throat radii of a mouth out to the bending cutoff: 0.35 of the distance to
  the nearest mouth, at most 2 blocks, more accurate than the midpoint steps they replace (D22, D26). Where a step's
  straight chord finds a hit further off the curved path than 1e-4 radians seen from the eye, the ray steps again
  in short steps across the hit (D25).
- A jump across each mouth's inner sphere (1.6 blocks in radius): the mouth's own field there looks the same about
  every axis through its centre, so a ray's path depends only on its angle (and, for the eye inside the sphere, on
  the eye's distance). Tables traced once at start-up give where each ray comes out, through the throat or not,
  and how much a uniform or linear extra field would move it; the other mouths' bending (their gradient and its
  change across the sphere, computed per frame) is added from that, to first order (D23, D26).

Rays sent sideways just outside a throat can stay trapped circling it for good; the Metal shader paints them from
the pixels around them, with a pull–push fill, instead of black (D27). `OPTIMIZATIONS.md` sets out the maths of
every optimization tried, and how close the light paths are to converged ones.

## Results

Run on an Apple M3 Pro (6 performance and 6 efficiency cores), macOS 15.7.3, Rust 1.98.1, release build, on a
2560×1440 display at 1× scaling. The machine was also running other work, so treat the times as rough.

In the real window at full-screen size (macOS makes it 2560×1336), playing a route (`--play-route`, after 5 warm-up
frames; the automatic scale is the default), this layer and PR 72's run one after the other, on a busier machine than
PR 72's own timings (155 fps on the legs then):

| Route | Average | Median frame | 99th percentile | Frames over 1/60 s | At full resolution |
| --- | ---: | ---: | ---: | ---: | ---: |
| scripted legs, now | 90 fps | 10.7 ms | 15.1 ms | 5 of 724 (worst 21.3 ms) | 77% of frames (never below 0.55) |
| scripted legs, PR 72 | 95 fps | 10.4 ms | 15.4 ms | 4 of 724 (worst 20.4 ms) | 93% (never below 0.75) |
| mouth tour, now | 82 fps | 12.3 ms | 18.5 ms | 30 of 1795 (worst 43.9 ms) | 44% (never below 0.5) |
| mouth tour, PR 72 | 88 fps | 11.1 ms | 16.5 ms | 15 of 1795 (worst 25.5 ms) | 69% (never below 0.65) |

- The frames that drop below full resolution are those within about a block of a mouth, looking past it, where
  neighbouring rays scatter in every direction and the GPU's thread groups diverge. This layer's more accurate
  light paths (below) cost GPU time, so more of them are upscaled. The automatic scale drops at once to the scale
  its time model predicts, by up to a fifth a frame, so mostly only the first frame of a sudden spike is late.

Median milliseconds per frame at full resolution (`make bench-views`, 20 frames for WebGL and Metal, 3 to 5 for the
CPU):

| View | WebGL 1280×800 | Metal 1280×800 | CPU 1280×800 | WebGL 2560×1600 | Metal 2560×1600 | CPU 2560×1600 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| spawn | 35 | 5.7 (4.1) | 838 | 106 | 17 (12) | 3484 |
| aimed at mouth 1A | 34 | 5.8 (3.8) | 783 | 99 | 15 (11) | 3027 |
| 2.5 blocks from 1A | - | 13 (8.2) | - | - | 36 (24) | - |
| 0.8 blocks from 1A's throat | 75 | 6.5 (4.3) | 1403 | 235 | 17 (14) | 5719 |
| looking sideways 0.4 blocks from 1A | - | 8.6 | - | - | 23 | - |
| above the rails | 9 | 3.0 (2.2) | 441 | 32 | 8.4 (6.7) | 1776 |
| high above spawn | 33 | 5.1 (3.4) | 631 | 95 | 13 (9.6) | 2624 |
| laser through 1A | - | 7.1 (4.9) | 853 | - | 20 (15) | 3510 |

- In brackets: PR 72's Metal, run just before under the same load. PR 70 took 23, 18, 37, 6, 15 and 25 ms at
  1280×800 and 81, 64, 134, 25, 52 and 88 ms at 2560×1600 for spawn, aimed, the throat, above the rails, high above
  and the laser.
- WebGL is the browser demo at the same drawing-buffer sizes, in the Claude desktop app's built-in browser (Chromium
  152, ANGLE on Metal), timed with `readPixels`. Its scene differs a little: its mouths and eye are 4.5 blocks up,
  not 1.7, and it has no laser.
- So Metal is 3 to 14 times faster than WebGL and 2 to 8 times faster than PR 70. The CPU, which got the bounding
  boxes but none of Metal's shortcuts, is 150 to 330 times slower than Metal.

How close the images are, at 2560×1600: how far image features moved, in pixels, from phase correlation of 48×48
textured patches (median), against a reference traced like PR 70 but with steps 32 times smaller, over the patches
where it agrees with a 16-times reference within 0.5 pixel (the two agree to 0.01 to 0.12 pixel; 1 pixel is about
0.05° of view):

| View | PR 72 | Now |
| --- | ---: | ---: |
| spawn | 1.38 | 0.14 |
| aimed at mouth 1A | 1.34 | 0.07 |
| 2.5 blocks from 1A | 2.64 | 1.50 |
| 0.8 blocks from 1A's throat | 1.99 | 1.02 |
| above the rails | 0.13 | 0.08 |
| high above spawn | 1.04 | 0.05 |
| laser through 1A | 1.35 | 0.07 |

So ordinary views now sit within about a tenth of a pixel of the converged light paths, and views dominated by a
mouth's lens, which magnifies small errors, within 1 to 1.5 pixels. Adaptive sampling on its own moves features by
under 0.01 pixel (median; SSIM 0.98 to 0.998 against tracing every pixel). Frames the window upscales with MetalFX
keep features in place (0.2 to 0.4 pixel) but lose fine detail (SSIM 0.85 to 0.96 at 0.8 scale).

Lunar Surface (`--map lunar-surface --bench-views`), GPU milliseconds per frame at 2560×1600 and full resolution,
with adaptive sampling (and tracing every pixel):

| View | GPU ms | View | GPU ms |
| --- | ---: | --- | ---: |
| blue spawn | 8.4 (6.5) | red spawn | 6.6 (6.6) |
| overview (from 46 m up) | 10.0 (14.5) | middle | 6.7 (15.4) |
| black hole | 10.4 (25.7) | Einstein ring | 7.6 (27.4) |
| wormhole | 12.1 (21.9) | wormhole exit | 17.4 (39.6) |
| red pylon | 5.8 (6.1) | ridge | 6.5 (8.3) |
| overlook | 6.1 (5.2) | habitat row | 6.2 (6.5) |
| laser into the wormhole | 18.7 (32.6) | | |

In a full-screen window (2560×1336, automatic scale, `--view`), every view runs at 72 fps or more, all at full
resolution but the wormhole exit, at render scales 0.96 to 0.98: blue spawn 181 fps, middle 166, Einstein ring 152,
overview 118, black hole 108, wormhole 91, wormhole exit 73. At that size and full resolution every view takes under 11
ms of GPU time but the wormhole exit (13.7) and the laser view with its beam (14.9). After 90 iterations a ray's steps
are three times longer (D40); that changes only the most compressed rings inside a mouth.

Playing the warp tour in that window (`--map lunar-surface --play-route --route warp-tour --width-px 2560 --height-px
1600`), twice: 84.5 and 85.4 fps on average, frame times median 11.7 to 11.8 ms, 99th percentile 14.7 to 15.3 ms,
worst 17.2 to 21.0 ms, 3 and 4 frames of 1,795 (0.2%) over 1/60 s; render scale 0.98 on average, at full resolution
in 78% of frames, down to 0.70 at the closest approach to a mouth.

The scripted route (`make run-autopilot`) completes every leg, with both renderers and the same timings: through 1A
after 1.6 s of walking, a 1 s jetpack press peaks at 12 blocks and lands 1.1 s after release, through 2A after 1.9 s
of flying, through 3A after 2.4 s of walking and down from 12 blocks in 0.6 s, and the laser through 1A to the house beyond 1B (17.5 blocks).

See [`DECISIONS.md`](DECISIONS.md) for how the benchmark was chosen, and [`OPTIMIZATIONS.md`](OPTIMIZATIONS.md) for the
maths of each optimization tried.
