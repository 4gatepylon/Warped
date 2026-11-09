# Dark Forest in Rust

A playable Rust demo of the Dark Forest level (W-122): a first-person shooter in a dim, swampy bamboo labyrinth in
bounded Euclidean space, whose features are verticality and wormholes. Each team's base has a treehouse on a 7 m deck,
rope ladders, plank bridges between the decks, and a 12 m lookout in the middle; five fixed wormholes, drawn as real
lensed wormholes, are shortcuts through the maze. It is built on [`../fps_demo/`](../fps_demo/README.md) (Rust, SDL3, a
Metal PBR rasterizer) with the UDP multiplayer of [`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/README.md).
Decisions made instead of the user are in [`DECISIONS.md`](DECISIONS.md).

## Play

The forest textures are downloaded, not committed (10 MB). The robot, guns, sounds and sky are read from
`../fps_demo/assets/`.

Alone, against AI robots:

```bash
pushd demos/dark_forest_rust
make download-assets
make run-dark-forest BOT_COUNT=3
popd
```

Two players on one Mac (two terminals), or on two Macs with the server's address:

```bash
pushd demos/dark_forest_rust
make serve-game
popd
```

```bash
pushd demos/dark_forest_rust
make join-game SERVER=127.0.0.1 TEAM=red
popd
```

and `make join-game SERVER=127.0.0.1 TEAM=blue` in a second terminal. `cargo run --release --bin dark-forest --
--host` runs a server in the window's own process and joins it, so a second window can `--join` it.

| Input | Action |
| --- | --- |
| Mouse | look (click the window to capture the mouse) |
| `WASD` | move; walk into a ladder to climb it (`W` up, `S` or `Ctrl` down, or look down and `W`) |
| `Shift` / `Ctrl` or `C` / `Space` | sprint (any direction, while the energy bar lasts) / crouch (half speed, eye lowered) / jump (`Space` lets go of a ladder) |
| `F` (hold) | flashlight |
| `V` | third person (the camera eases out behind and above your robot; aiming brings it back) |
| Left mouse | fire: the sniper (gun 1) once a click, then 2 s to reload; the rifle (gun 2) while held, until it overheats |
| Right mouse | aim: zoom in 2.5 times (sniper) or 1.5 times (rifle); the mouse turns the view more slowly by as much |
| `1` / `2` / wheel | take the sniper / take the rifle / swap guns |
| `E` (hold) | turn off an enemy console: look at it from within 3 m and hold 10 s |
| `Esc` | release the mouse; a second `Esc` asks "Do you want to exit?" (`Return`/`Y` exits, `Esc`/`N` stays) |
| `Tab` | menu (scores, settings, map, credits) |
| `F1`-`F8`, `[` / `]` | effects and render scale, as in `../fps_demo/` |

The guns, hits, console holds and movement are the Lunar Surface demo's too (W-126; DECISIONS.md D17 to D22 and D26 to D28): one
sniper hit eliminates a player, the rifle takes 12 hits to the body or 2 to the head; you walk at 10 m/s and sprint at
18 m/s (no run-up, the same in the air), crouch to half speed with the eye at 0.6 of its height, a jump rises 1.25 m
and lasts 0.58 s, a hit slows you to half speed for 1 s, and a shot fires only 0.25 s after your last step upright or
jump (crouch-walking doesn't count). Ladders keep the forest's own pace.

The HUD is the Lunar Surface demo's (W-126, same play in every level; DECISIONS.md D16): in the bottom left the
health bar (a segment per rifle hit to the body), the gun's ammo bar (the sniper's one block while loaded, the
rifle's heat in 20 segments, red once it overheats) and the sprint energy bar (sprinting empties it in 7.5 s),
with glows that run along a bar while it changes; Lunar's crosshair; the console bar below the middle while `E` is
held on an enemy console; red, blurred edges and a shake when you take any damage; and "YOU DIED" with the respawn
countdown.

## The level

- **Map:** [`../../common/maps/dark_forest/map.json`](../../common/maps/dark_forest/map.json), written by
  `src/forest_layout.rs` (`make export-map`, compact JSON, 8.6 MB) and read by both the game and the server. It is
  60 m by 112 m, fenced by rock walls. Red's base is at +z and blue's is red's turned half a turn about the centre, so
  the two sides are the same.
- **Bamboo labyrinth:** `add_labyrinth` carves red's half into a seeded maze of 4 m cells (a winding depth-first tree
  plus about 12% extra openings for loops, so there are dead ends and loops), grows a wall of bamboo clumps on every
  closed side where nothing else stands, and mirrors it onto blue's half. Corridors are about 1.5-3 m wide. The seed
  and cell size are in the map's `scatter` field. A walk-grid flood test checks that every spawn, ground pylon,
  ladder foot and wormhole mouth can be reached. Each culm forks at the top into leafy branches whose shadows keep
  the forest dark, with light shafts through the gaps.
- **Heights:** the ground; decks at 7 m (each team's treehouse, a middle deck and a flank deck, joined by bridges with
  rope rails), held up by bundles of thick culms; the lookout at 12 m in the centre.
- **Ladders:** the optional `ladders` field of the map format: a volume that isn't solid. A player inside one who
  walks toward it climbs at 3.2 m/s and steps onto the deck at its top.
- **Wormholes:** five fixed pairs, no black holes: three across the ground and each team's lift from the ground to its
  treehouse deck. Each mouth is the hyperbolic demo's foil lens in flat space (`src/game/warps.rs`): it bends light
  and lasers within its reach, and whatever enters its throat leaves the partner's through the partner's mirror in z.
  The tonemap pass traces each pixel's ray through the lens (`get_lens_color` in `src/shaders/post.metal`) and shows
  the far side from a cube atlas of every mouth, drawn once at start-up (about 0.8 s). Robots, players and lasers
  near the mouths in view are drawn live each frame from the mouths' centres (`encode_lens_live` in
  `src/render/metal/mod.rs`), so they show through the wormholes too, hidden by the forest where it stands in front
  (DECISIONS.md D15).
- **Objective:** two pylons per team (the hyperbolic demo's `scifi_beacon`), one on the ground near the base, one on
  the treehouse deck, each sending up a beam in its team's colour while its console is on. Hold `E` at an enemy
  console for 3 s to turn it off; a team wins once every console of the other team is off.
- **Look:** a murky green grade of the HDRI sky, thick green haze, golden lasers for everyone (the map's
  `laser.rgb`) drawn as Lunar Surface's beams, speckle included (DECISIONS.md D33), and 31 flickering candles (warm map lights with `flicker_frac`) on the decks, bridges and support
  bundles.
- **Map page:** the menu's map page (Tab, then E) is `assets/map_picture.png`, a photo of the forest from straight
  above made by `make map-picture`, with live marks over it; without it the page shows the old drawing.

## Match interface

The match session (`demos/warped_match_rust/`) starts levels with these flags:

| Command | Does |
| --- | --- |
| `dark-forest --serve --port <port> [--map <map.json>] [--level-result-file <path>]` | runs a server without a window. With `--level-result-file`, once a team has turned off every console of the other, it writes `{"winner": "blue"}` or `{"winner": "red"}` to `<path>.partial`, renames it to `<path>`, and keeps running (no new game) until killed. Without it, a new game starts 10 s after a win. |
| `dark-forest --join <address> --port <port> [--map <map.json>] [--team blue\|red]` | joins a server, on that team at once if `--team` is given. |
| `dark-forest --join <address> --port <port> --bot --exit-after-s <s> --team <team> [--bot-objective consoles]` | a bot for tests and matches, in a hidden window that draws nothing. It hunts the other team's players and shoots them; with `--bot-objective consoles` it walks instead (by the ladders where needed) to each of the other team's consoles, holds `E` once in reach (which the server checks), and logs `bot reached console <i> on foot in <t> s`, until its team wins. |

The default port is 47840 (UDP), and server discovery uses 47841. `dark-forest-server` is the same server as its own
binary. The wire protocol is the hyperbolic demo's, with its wormhole fields always empty (see `DECISIONS.md`).

## Tools

- `--bench-views <s>`: holds seven fixed views (`spawn`, `bridge`, `lookout`, `treehouse`, `firing`, and the
  wormhole views `gate` and `lift`) for `<s>` seconds each and prints their frame times; `make bench-views` writes
  them to `.artifacts/bench_views/<run_id>/`. `gate` looks into the centre gate and `lift` into red's lift from the
  ground, and each holds robot 0 just beyond the partner mouth, facing back through it.
- `--robot-pose x,y,z,yaw [--robot-walk-m-per-s <speed>]`: holds robot 0 there (it doesn't think or shoot); with a
  speed it walks straight ahead, through any mouth, and starts again every 10 s. `--hold-trigger` keeps firing. For
  pictures and videos of robots and lasers through the wormholes.
- `--photo-hud` keeps the HUD in `--photo` and `--video-frames` pictures; `--photo-at-s <s>` takes the photo at that
  simulated time; `--hurt-player-every-s <s>` hurts the player by a robot's shot that often; `--hold-console` holds
  `E`. For pictures of the HUD, hits and "YOU DIED".
- `--play-route`: the autopilot walks the vertical route from red's spawn; `make bench-dark-forest` runs it full
  screen into `.artifacts/bench_dark_forest/<run_id>/`.
- `--photo <png> [--photo-width-px <px>] [--camera-pose x,y,z,yaw,pitch] [--fov-deg <deg>] [--hide-canopy]`: saves
  one frame without the HUD and quits; `--photo-map-page` opens the menu's map page.
- `--map-picture <png>` (`make map-picture`): renders the map page's picture.
- `--camera-view <view>`, `--autopilot`, `--show-frame-time`, `--benchmark-seconds <s>`: as in `../fps_demo/`.
- `--export-map <path>`: writes the map and quits.
- `--frame-times-file <csv>`: on exit, writes every frame's timing (the loop's interval and simulated step, the CPU's
  phases, the GPU's time, and when the frame reached the display) and prints how evenly frames were shown, for
  chasing stutter.

## Performance

Measured on a virtual Mac (`VirtualMac2,1`, Apple M4 Pro (Virtual), 12 cores, 16 GiB, macOS 26.6.2, Apple
Paravirtual GPU, 1600x1200 display at 60 Hz), Rust 1.99.0, Xcode 26.6, full screen at render scale 1.0, vsync on:

| Run | Mean | Median | p99 | GPU mean |
| --- | --- | --- | --- | --- |
| View `spawn` | 16.76 ms | 16.57 ms | 19.75 ms | 11.66 ms |
| View `bridge` | 16.95 ms | 16.66 ms | 21.43 ms | 11.72 ms |
| View `lookout` | 16.74 ms | 16.61 ms | 19.78 ms | 10.99 ms |
| View `treehouse` | 17.17 ms | 16.58 ms | 33.53 ms | 10.33 ms |
| View `firing` | 16.74 ms | 16.49 ms | 19.86 ms | 11.29 ms |
| `--play-route`, 3 robots, 60 s (5 s windows) | 16.67-17.12 ms | 16.60-16.73 ms | 19.05-33.64 ms | 11.37-11.75 ms |

The median sits on the 60 Hz vsync interval and the GPU needs 10-12 ms a frame, so there is headroom on the GPU, but
the p99 misses the 16.7 ms target on this virtual GPU: it has a frame or two of 20-34 ms in every window. A real Mac
at 2560x1440 has not been measured yet. The table above is from before the labyrinth and wormholes.

With the labyrinth (about 8,900 culms) and the five lensed wormholes, `--photo` runs in a 1600x900 pt window with 4
robots (60 frames each, vsync on, median at the 60 Hz interval in every run) give these GPU times:

| View | Render scale 2.0 | Render scale 1.0 |
| --- | --- | --- |
| No mouth in view | 11.8 ms | 7.3 ms |
| One mouth in view | 14.5-15.9 ms | not measured |
| A mouth filling the screen | 16.6 ms | 9.8 ms |

### Frame pacing

With vsync on, each frame stays on screen a whole number of display refreshes, the same for every frame: one while
frames are shown on time, two (or three) once more than a tenth of the last 30 were shown late, and one again when
the GPU's recent frames fit well inside one refresh (`render::frame_clock::choose_refreshes_per_frame`, fed by Metal's
presented times); the game simulates exactly that long a step per frame. So on a 100 Hz display a frame the GPU can't
finish in 10 ms is shown for a steady 20 ms (50 fps) instead of a mix of 10 and 20 ms, which looked like constant
jitter. While no frame reaches a display (it sleeps, or the window is hidden), frames are shown as soon as they are
ready. The forest is cut into 8 m tiles so that each pass (four shadow
cascades, the depth prepass and the lit pass) draws only the tiles it sees.

The static forest's shadows are drawn into a cache only when a shadow cascade moves (the cascades are centred on the
camera and stay put until it walks out of their margin; DECISIONS.md D29), and only the robots and other moving
casters are drawn into the shadow map each frame. The frame statistics' GPU time is each frame's own: from when it
started or the frame before it ended, whichever is later, to when it ended.

## Code

- `src/forest_layout.rs`: generates the map (labyrinth, bamboo, decks, bridges, ladders, pylons, wormholes, candles,
  spawns, lights).
- `src/map.rs`: reads and writes the portable map format.
- `src/game/world.rs`: turns the map into collision boxes, drawable shapes, ladders, consoles and pylons.
- `src/game/warps.rs`: the wormholes' lens, throat crossing and laser paths (and black holes, unused here).
- `src/game/player.rs`: walking, jumping and climbing: the shared movement rules (`MoveState`), carried out in
  the forest by `Forest`, a shared `Ground` (D37); `src/game/play_trace.rs` records a play trace to compare with
  Lunar Surface's.
- `src/game/guns.rs` (on the shared `Weapons`), `src/game/dust.rs`, and the shared `shot_noise`: the Lunar Surface demo's two guns, their beams'
  flight and ground dust, and each shot's stray.
- `src/console_reach.rs`: who may hold `E` on a console (in reach and aimed at), for the server and the windows.
- `src/game/console_bot.rs`: the bots' walking, along `src/game/nav.rs`'s paths and up and down the ladders.
- `src/match_play.rs`: consoles, the win, and the online client (state, hits, remote robots).
- `src/shaders/post.metal` (`get_lens_color`) and `capture_wormhole_lens` in `src/main.rs`: the wormhole lens pass.
- `src/net.rs`, `src/server.rs`, `src/bin/dark_forest_server.rs`: the UDP server and protocol, copied from the
  hyperbolic demo.
- `../warped_shared_rust/` (a library this crate depends on): the HUD's bars, crosshair and hit effect, the overlay
  rectangles, the two guns (`weapons`), each shot's stray (`shot_noise`) and the settings sections, the same code
  every level uses (D34, D35).
- `src/render/camera_math.rs`: the view and projection matrices, with glam 0.30's arithmetic (D34).
- Robots (`src/game/bots.rs`, drawn in `src/game/session.rs`): Lunar Surface's teal robot through this renderer's
  GPU skinning, its teal in the team's colour, its clip and stride by the shared `gait` (D36).
- Everything else (renderer, assets, audio) is the FPS demo's.

## Assets

| Asset | Source | Author | License | Size |
| --- | --- | --- | --- | --- |
| Texture `forest_ground_04` (1K) | https://polyhaven.com/a/forest_ground_04 | Rob Tuytel, Rico Cilliers | CC0 | 3.2 MB |
| Texture `bamboo_wall` (1K) | https://polyhaven.com/a/bamboo_wall | Amal Kumar | CC0 | 2.0 MB |
| Texture `old_planks_02` (1K) | https://polyhaven.com/a/old_planks_02 | Rob Tuytel | CC0 | 1.0 MB |
| Texture `forest_leaves_02` (1K) | https://polyhaven.com/a/forest_leaves_02 | Rob Tuytel | CC0 | 1.3 MB |
| Texture `mossy_rock` (1K) | https://polyhaven.com/a/mossy_rock | Rob Tuytel | CC0 | 2.9 MB |
| Players' robot `teal_robot` (shared: `common/maps/models/teal_robot/`, Lunar Surface's) | https://sketchfab.com/3d-models/teal-v2-d6a54ff9468c451bbdbd879afa03f20a | Lagst (clips made in this repository) | Sketchfab Standard | committed |
| Pylon model `scifi_beacon` (shared: `common/maps/models/scifi_beacon/`) | https://sketchfab.com/3d-models/sci-fi-beaconway-point-marker-free-model-a1d91ebb3e2d41bba31c02b11423d97f | AMMediaGames | CC BY 4.0 | 1.0 MB (committed) |

`make download-assets` saves them in `assets/textures/` (git-ignored). The sky, rifle, revolver and arms are
`../fps_demo/assets/`, credited in [its README](../fps_demo/README.md#assets); its arms are CC BY-NC 4.0 and its
revolver Sketchfab Standard, which limit sharing the game.

## Limits

- Through a wormhole, robots, players and lasers show live, but the scenery and lights around the partner are a
  picture drawn at start-up, and pylons and their beams don't show (DECISIONS.md D15). At most 4 mouths' surroundings
  are live at once, and a robot more than 50 m from a mouth doesn't show through it.
- Lasers of other players online are traced locally; their hits come from the shooter's own trace.
- Online, the server trusts the client about its hits; it checks a console hold's reach and aim (as Lunar Surface's
  does, without walls in between).
- The beams are drawn as the forest's tracers, without Lunar Surface's blade and laser looks or their speckle, and
  the ground dust as smoke (DECISIONS.md D20).
- The protocol still carries the hyperbolic demo's wormhole fields, always empty.
