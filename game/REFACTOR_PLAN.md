# Refactor plan: the Rust demo becomes the Warped game

This plan turns [`demos/hyperbolic_wormholes_rust/`](../demos/hyperbolic_wormholes_rust/README.md) (the "demo") into
the game itself, in `game/`: a game window (`warped`) and a game server (`warped-server`), with shared code written
once, ready for every level of [`game_design/DESIGN.md`](../game_design/DESIGN.md), 5v5 matches between friends over
the internet, and spaces other than hyperbolic ones. It is written for the agents who will carry it out, possibly
on a cloud macOS VM that can't run the game at full fidelity, so the testing plan (section 7) is as detailed as the
refactor itself.

Another agent works on the same game at the same time: the **infra stack** (#179 to #185, branches
`claude/infra-*`) packages the demo as a Mac app with a title screen, builds its game server alone for Linux,
protects games with a password, and deploys game servers and a download page on Google Cloud. Section 3.1 says who
owns what, so the two never do the same work twice or in two ways.

Contents:

1. Ground rules
2. Decisions
3. Target layout (3.1: who owns what)
4. Where every demo file goes
5. How the game is used afterwards
6. The steps (the PR stack)
7. Testing plan
8. Checking the machine you run on
9. Working on the stack together
10. Open questions for the user

## 1. Ground rules

- **Never delete the demo.** `demos/hyperbolic_wormholes_rust/` stays as it is and keeps building and running, so
  that anyone can go back to it if the game breaks. Copy files from it into `game/` and change the copies there. The
  only edits the demo may get are path fixes forced by a shared move (step A6, assets), each tested by building and
  running the demo afterwards.
- **One PR per step, at most 200 reviewable lines** (root `AGENTS.md`). Verbatim copies, moves, lockfiles and prose
  don't count, so copy a file verbatim in one commit and change it in the next, and say in the PR how many lines
  are copies.
- **Every layer builds and passes `make test` in `game/`**, and the demo's own `make test` still passes.
- **Regression first.** A step that ports code must show it behaves as the demo did (section 7.3) before a later step
  changes the behaviour on purpose. A deliberate change of behaviour says so in its PR and in `game/DECISIONS.md`.
- **The repository's rules hold** in `game/`: naming, docstrings, failing fast, `.build/` and `.artifacts/`, the
  `clis` and `makefiles` skills, the `compatible-changes` skill for settings, the network and maps.

## 2. Decisions

Made by the user (2026-11-08):

- Two binaries: one for the game server, one for the game window.
- Where the Rust code lives is up to us: follow separation of concerns, and don't repeat code.
- Remove the house demo's engine (its world, renderer and routes) from the game, but keep it in the demo.
- Copy, don't delete, the demo (section 1).
- Shared assets go in `common/assets/`.
- Keep the demo's UDP message layouts; no Protocol Buffers. Change them where that's simplest and makes most sense
  (section 6, phase C).
- Copy the wishlist over; the product is named Warped.
- Delete the prototype square game with its HTTP management server (done in this stack, layer 2).
- Players are friends, possibly in different cities: 5v5, on a game server on a VM in a nearby cloud region (e.g.
  Oregon for the US West Coast), 3 to 4 games at once, perhaps tournaments; no large scale yet.
- Games go over the internet, so they need a simple, efficient way to refuse strangers: a secret the server's owner
  gives their friends (e.g. by WhatsApp). The scheme is decided now, so no two agents build it differently:
  [`common/protocol/authentication.md`](../common/protocol/authentication.md).
- Traffic is encrypted with a scheme in common use for real traffic. Every server must have a password; running
  without one is not allowed. The earlier proposal may be changed to be more secure.
- Use `snow` (Noise) for the encryption for now. That `snow` has had no formal audit is a known limitation, written
  in `game/README.md` and `authentication.md`.
- Another agent (the infra stack) makes the downloadable Mac app (double-click, pick a server, give an address)
  and the cloud deployment.
- Later the same day: the infra stack pauses its coding. The refactor works on its own, owns every file of the game
  (what the infra stack made too), and changes `infra/` itself where the game needs it; the infra stack adapts
  afterwards. The workloads the infra stack built must keep working from `game/` (section 3.1).
- The goal now is phase A: a working game, refactored into `game/`. Phases B to G come later.

Made by the agent who wrote this plan, for the user to confirm or overturn:

- **`game/` is one Cargo workspace** with crates `core`, `client`, `server`, plus test-only crates. One workspace
  keeps one lockfile, one `.build/` and one `make test`, and `common/` stays specifications and data, as
  `common/README.md` says.
- **`warped-core` has no window, GPU or Apple dependency** (no `sdl3`, `objc2*`, `metal`), so `warped-core` and
  `warped-server` build on Linux and the game server can run on a cheap Linux VM. The window stays macOS-only.
- **The server and the window's connection are written "sans-IO"**: state machines that take a datagram and the
  time and return the datagrams to send. The UDP loop around them is thin. This is what lets multiplayer be tested
  on one machine, deterministically, with simulated latency and loss (section 7.5).
- **No management server for now.** 3 to 4 games on one VM are 3 to 4 `warped-server` processes on ports
  47800 to 47803; friends join with the VM's address. A launcher can come back later (its code is in git history,
  the commit before layer 2 of this stack).
- **The demo's names stay during the port** (`VoxelWorld`, `VoxelPlayer`, ...), so that ported files diff cleanly
  against the demo's. Renames come after the regression tests are in place.
- **Authentication and encryption: v2 of `authentication.md`, built in step C1.** A Noise handshake
  (`Noise_NNpsk0_25519_ChaChaPoly_SHA256`, through the `snow` crate) proves the password on both sides and makes
  fresh keys. Every datagram is then sealed with ChaCha20-Poly1305 and numbered against replays, as WireGuard does.
  The password is generated (160 random bits, `warped2-...`, with a checksum for typos), never chosen: a chosen
  password could be guessed offline from a recorded handshake. `warped-server` refuses to start without one.
  - Until C1, the game keeps the infra stack's v1 (#181) as the demo has it, but from A5 on, `warped-server`
    already refuses to start without a non-empty password.
  - v2 replaces v1 in one layer, rather than the demo being changed while the infra stack works on it.
- **The infra stack lands in the demo first, and the game copies it** (step A0, done in #379 and #381), so the app, the Linux server, the
  password and the title screen arrive in `game/` with the rest of the demo instead of being redone.

## 3. Target layout

```text
game/                                the Warped game (a component folder, one Cargo workspace)
  Cargo.toml, Cargo.lock             the workspace and its one lockfile
  .cargo/config.toml                 target-dir = ".build"
  Makefile                           the entry point: make help
  README.md                          how to play, build and test
  AGENTS.md, CLAUDE.md               rules for agents working in game/ (copied from the demo's, then adapted)
  REFACTOR_PLAN.md                   this file, deleted when the plan is done
  DECISIONS.md                       decisions made while building the game, numbered G1, G2, ...
  USER_WISHLIST_CHECKLIST.md         copied from the demo in step A1; the game's wishlist from then on
  player_settings.yaml               a player's own settings (keys, view), as in the demo
  server_settings.yaml               the numbers that shape play, as in the demo
  core/                              crate warped-core (library): everything both binaries need
  client/                            crate warped-client: the game window, binary `warped` (macOS, Metal)
  server/                            crate warped-server: the game server, binary `warped-server` (macOS, Linux)
  legacy_oracle/                     crate legacy-oracle (tests only): the demo's own code, compiled in place
  tools/udp_impair/                  crate udp-impair: a UDP proxy that adds latency, jitter, loss and reordering
common/assets/                       textures and gun models the game draws (moved from demos/fps_demo/assets/)
common/maps/                         maps, unchanged until phase E extends the format
```

Dependencies only point downwards: `client` and `server` depend on `core`; nothing depends on `client` or `server`;
`legacy-oracle` is only a dev-dependency of tests.

### 3.1 Who owns what

The infra stack (the Mac app, the Linux server bundle, the cloud) is paused, so the refactor owns all of it for now
(section 2). The table says where the infra stack's work lives before and after A1; once A1 lands, the demo is
frozen (section 1), and the infra stack's future work on the game happens in `game/`.

| What | Before A1 (in the demo) | After A1 (in `game/`) | Owner |
| --- | --- | --- | --- |
| Game code: world, rendering, HUD, network messages | `demos/hyperbolic_wormholes_rust/src/` | `game/core/`, `game/client/`, `game/server/` | refactor |
| Authentication (`authentication.md`) | `net.rs` (#181) | `game/core/src/net/` | refactor keeps it working; changes go through the spec |
| Title screen, password dialog (`title.rs`, `run_title_screen`) | the demo (#182) | `game/client/src/title.rs` | infra stack |
| Mac app: `app/Info.plist`, icon, `how_to_open.txt`, `make package-mac-app`, `online_games.txt` | the demo (#183) | `game/client/app/`, `game/Makefile` | infra stack |
| Assets inside the app (`Contents/Resources/assets/`, the built-in revolver) | the demo (#183) | `game/client/src/assets.rs` | infra stack, with A6's move to `common/assets/` done by the refactor |
| Linux server build: `make build-server-bundle`, the musl target in `.cargo/config.toml` | the demo (#180) | `game/Makefile`, `game/.cargo/config.toml` | infra stack |
| Maps built into the programs (#179) | `map.rs` | `game/core/src/map.rs` | refactor |
| Cloud: `infra/` (Terraform, startup script, download page) | `infra/` | `infra/` | infra stack |
| The server's command line, which `infra/startup.sh.tftpl` runs | `wormholes_server --port --map --game-password-file` | `warped-server` with the same flags (A5 renames it and updates the startup script in the same PR, reviewed by the infra stack) | refactor |

Workloads the game must keep supporting, from the step that brings each into `game/` on. A step that changes one
updates its users in `infra/` in the same PR and says so:

- `make package-mac-app ONLINE_GAMES=<file>` builds `Warped.app` and zips it into `.build/Warped.zip`, with the
  icon at `app/app_icon.png` of the game folder (`infra/Makefile`'s `GAME_DIR`).
- `make build-server-bundle` builds the server alone for `x86_64-unknown-linux-musl` into
  `.build/server_bundle.tar.gz`, holding the one binary `infra/startup.sh.tftpl` runs.
- `make serve-game` and `make join-game` with `PORT`, `MAP`, `SERVER_SETTINGS`, `PLAYER_SETTINGS`,
  `GAME_PASSWORD_FILE`.
- The window, from `infra/Makefile`'s `check-game-servers`: `--join <address:port> --map <map>
  --game-password-file <file> --bot --exit-after-s <s> --label <label>`, without a display or Metal device.

Interfaces those workloads rely on:

- The server's flags: `--port`, `--map` (later `--maps`), `--game-password-file`, `--server-settings`; exit on
  SIGTERM; one process per game.
- The window's flags the title screen passes: `--join <address>`, `--port`, `--map`, `--game-password-file`; with
  no flags at all, the window opens the title screen.
- `online_games.txt`: one game a line, `<address>:<port> <map> <name>`.
- The exit code 3 for a refused password.
- From step C1: the password format `warped2-...` (`authentication.md` v2). The cloud deployment then makes each
  game's password with `warped-server create-game-password` instead of choosing one, and the app keeps one password
  per server address.
- Where the app keeps things: `~/Library/Application Support/Warped/game_password`, `~/Library/Logs/Warped/`,
  `~/.config/warped/player_settings.yaml`; assets in `Contents/Resources/assets/`; maps inside the program.

## 4. Where every demo file goes

Paths are relative to `demos/hyperbolic_wormholes_rust/src/` (from) and `game/` (to). "Split" means part of the
file goes to each place; the parts are listed.

| Demo file | Lines | Goes to | Notes |
| --- | --- | --- | --- |
| `main.rs` | 5,136 | `client/src/main.rs` + `client/src/{cli,renderer,title_choice,server_process,bot,bench,play,run_folder}.rs`, `client/src/session/{mod,weapons,online,hud}.rs`, then `server/src/main.rs` | Split (step A3, done): command line, the `Session` (≈1,900 lines: input, weapons, HUD, net glue, and the warp tour), offscreen benchmark views, the window loop; `serve_only` waits in `server_process.rs` and goes to the server in A5. House routes, house views and `--autopilot` are dropped. |
| `net.rs` | 2,428 | `core/src/net/protocol.rs`, `core/src/net/connection.rs`, `server/src/game.rs` | Split (steps A4, A5): message layouts (`Message`, `Reader`, `Kind`, sizes) and the window's `Client` go to core; `Server`, `serve`, the beacon go to the server. |
| `voxel_world.rs` | 3,550 | `core/src/voxel_world.rs` (+ `core/src/player.rs`, `core/src/laser.rs` later) and `client/src/scene.rs` | Split (step A4): walking, collisions, lasers, consoles, black hole, time rate stay in core; avatars posed for drawing (`with_avatars`, `with_posed_avatars`), clearance grids, markers and material surfaces go to the client. |
| `settings.rs` | 911 | `core/src/settings.rs` and `client/src/player_settings.rs` | Split: `ServerSettings` to core; `PerPlayerSettings` (keys, through `key_bindings.rs`, which needs SDL) to the client. |
| `multiplayer.rs` | 963 | `client/src/others.rs` and `core/src/bot.rs` | Split: drawing other players to the client; the windowless bot (`run_voxel_bot`) to core, so tests can run bots without a window. The house half (`pack_shared_view`, `run_bot`) is dropped. |
| `geometry.rs`, `map.rs`, `terrain.rs`, `shapes.rs`, `mesh.rs` | 2,000 | `core/src/` | As they are (step A4a, done, with `app_bundle.rs`, `fps_demo_assets.rs` and `build.rs`; `TEAM_NAMES` moved from `net.rs` to `map.rs`, which uses it). `map.rs` finds `common/maps/` as `../../common/maps` from the crate folder (`CARGO_MANIFEST_DIR`); every crate in `game/` is two levels down, like the demo, so that path still holds. |
| `thrown_wormholes.rs`, `shot_noise.rs`, `black_hole_growth.rs` | 1,180 | `core/src/` | As they are. |
| `zone_walls.rs` | 385 | `core/src/zone_walls.rs` and `client/` | Split: the planes to core; `GpuZoneWalls` and `to_gpu` to the client. |
| `avatar_model.rs` | 535 | `client/src/` (+ hit shape to core) | The posed glTF body is drawing; the hit cylinder and head the server needs go to core. |
| `render_metal.rs` | 1,633 | `client/src/render/present.rs` | Split (step A2): keep MetalFX upscaling, overlays, presenting to the window, the loading screen; drop the house ray tracer that drives `render_metal.metal`. |
| `render_voxel.rs`, `render_voxel.metal`, `render_mesh.rs`, `voxel_clearance.rs`, `voxel_textures.rs`, `viewmodel.rs`, `viewmodel.metal` | 6,900 | `client/src/render/` | As they are. |
| `fps_demo_assets.rs` | 46 | `client/src/assets.rs` | Points at `common/assets/` after step A6. |
| `hud_bars.rs`, `menu.rs`, `minimap.rs`, `map_layout.rs`, `exit_prompt.rs`, `credits.rs`, `third_person.rs`, `key_bindings.rs`, `avatar_animation.rs`, `dust.rs`, `spaceships.rs` | 4,600 | `client/src/` | As they are. |
| `world.rs`, `scene_data.rs`, `render_cpu.rs`, `render_metal.metal`, `physics.rs`, `map_export.rs` | 6,500 | dropped | The house demo's engine. `physics::Input` moves to `core/src/player.rs` (A2c leaves it alone in `physics.rs` until A4), since `voxel_world.rs` uses it. `--renderer cpu` goes with `render_cpu.rs`. The overlay kernel at the end of `render_metal.metal` stays, as `client/src/overlay.metal`. |
| From the infra stack: `app_bundle.rs` (#183) | 54 | `client/src/app_bundle.rs` | Finds assets and models inside `Warped.app` or in the checkout; its checkout folders are relative to the crate folder, so A1 fixes them for `game/client/`. |
| From the infra stack: `../build.rs` (#183) | 80 | `client/build.rs` | Builds the Sketchfab-licensed models into the program; its folders are relative to the crate folder, fixed in A1 like `app_bundle.rs`'s. |
| `../Makefile` | 130 | `game/Makefile` | Its targets join the game's in A1, pointing at `client/`; A5 points the server ones at `warped-server`. |
| From the infra stack: `bin/wormholes_server.rs` (#180) | 107 | `server/src/main.rs` | The Linux game server's `main`; A5 builds `warped-server` from it and the server half of `net.rs`. |
| From the infra stack: `title.rs` (#182), the title screen and password dialog in `main.rs` | 260 + 200 | `client/src/title.rs`, `client/src/main.rs` | As they are (infra stack's). |
| From the infra stack: `../app/` (#183: `Info.plist`, `app_icon.png`, `how_to_open.txt`) | | `client/app/` | Copied verbatim (infra stack's). |
| `../player_settings.yaml`, `../server_settings.yaml`, `../rustfmt.toml` | | `game/` | Copied verbatim. |
| `../USER_WISHLIST_CHECKLIST.md`, `../DECISIONS.md`, `../OPTIMIZATIONS.md`, `../PLAYTEST.md`, `../AGENTS.md` | | `game/` | Copied in step A1; the demo's copies get one line at the top saying the game's copies are the live ones. Past decisions keep their `D` numbers; new ones are `G1`, `G2`, ... |

Outside the demo:

- `common/maps/house_demo/` stays (the demo plays it); the game doesn't list it.
- `demos/fps_demo/assets/`: the textures and two gun models the game uses move to `common/assets/` (step A6, about
  140 MB: the `moon_01`, `moon_meteor_02`, `concrete_*`, `corrugated_iron_02`, `metal_plate`, `rusty_metal_02` sets,
  `sketchfab/sci_fi_revolver` and `models/bolt_action_rifle_7_62`); both demos point there.
- Docs and skills that name the demo as the game (`game_design/DESIGN.md`, `adding-assets`, `graphical-changes`,
  `capturing-screenshots.md`, `common/maps/README.md`) switch to `game/` in step A7.

## 5. How the game is used afterwards

### Playing

```bash
cd game
make serve-game MAP=lunar-surface PORT=47800 GAME_PASSWORD_FILE=password.txt
#   warped-server --map lunar-surface --port 47800 --game-password-file password.txt
make join-game SERVER=203.0.113.7 PORT=47800 GAME_PASSWORD_FILE=password.txt
#   warped --join 203.0.113.7 --port 47800 --game-password-file password.txt
make join-game SERVER=auto                         # a server on the local network, without a password
make package-mac-app ONLINE_GAMES=online_games.txt # Warped.app, zipped (infra stack)
make build-server-bundle                           # warped-server for x86-64 Linux, to upload (infra stack)
```

Most players never type these: they download Warped.app, double-click it, pick a game on the title screen (or type
an address), and type the password once (infra stack). On the cloud (`infra/`, infra stack): one `warped-server`
per game, each on its own UDP port, started at boot with the game's password.

### `warped-server`

| Flag | Meaning |
| --- | --- |
| `--port <port>` | UDP port (default 47800) |
| `--map <name>` | One level per game, as today (phases A to C) |
| `--maps <name,...>` | The levels of a match, blue's deepest first (phase D); replaces `--map` |
| `--server-settings <path>` | The settings sent to every player (default: the built-in `server_settings.yaml`) |
| `--game-password-file <path>` | Required: only players who prove they know this password join, and all traffic is encrypted with keys from it (`authentication.md`) |
| `create-game-password --output-file <path>` | A subcommand: writes a new generated password (`warped2-...`, mode 0600) and prints the file's path (step C1) |
| `--max-player-count <count>` | At most 10 (5v5) after phase C; 8 before |
| `--next-game-after-s <seconds>` | Starts the next game this long after a win, since a server on a VM has no one to press `Enter` |
| `--exit-after-s <seconds>` | Stops after that long (tests) |
| `--json` | Prints joins, eliminations, wins and per-second statistics as JSON lines (tests, monitoring) |

### `warped`

| Flag | Meaning |
| --- | --- |
| (no flags) | The title screen (infra stack): pick an online game from the app's `online_games.txt`, or one on this Wi-Fi |
| `--join <auto\|address>`, `--port <port>`, `--player-settings <path>`, `--game-password-file <path>` | As in the demo (after the infra stack) |
| `--bench-views`, `--view <name>`, `--play-route --route warp-tour`, `--width-px`, `--height-px`, `--frame-count`, `--render-scale`, `--sampling`, `--label`, `--json`, `--exit-after-s` | As in the demo, for the voxel maps |
| `--map <name>` | Only for the offline tools above; when joining, the window loads the map the server names (phase D) |
| `--bot` | A windowless player that joins and plays (tests); needs no GPU |

Gone from the window: `--serve` (now `warped-server`, which also replaces the infra stack's `wormholes_server`),
`--renderer` (only Metal remains), `--autopilot` and the house routes, `--export-map`.

### `warped-core`

The library both binaries use, and the tests. Its public modules after phase A:

| Module | What it gives |
| --- | --- |
| `geometry` | the hyperboloid model's maths (phase E: `space`, for every kind of space) |
| `map`, `terrain`, `shapes`, `mesh`, `zone_walls` | a map of `common/maps/` and its parts |
| `voxel_world` | `VoxelWorld::FromMap`, walking (`VoxelPlayer::step`), `trace_laser`, consoles, the black hole, time rate |
| `player` | `Input`, the controls a player holds in one step |
| `thrown_wormholes`, `shot_noise`, `black_hole_growth` | as in the demo |
| `settings` | `ServerSettings` |
| `net::protocol` | every message's layout, encoding and decoding |
| `net::connection` | the window's side of a game: joining, sending its state, reading snapshots (sans-IO) |
| `bot` | a scripted player for tests |
| `rules` (phase D) | a match's rounds and wavefront, as pure functions |

```rust
use warped_core::{map, settings::ServerSettings, voxel_world::{VoxelPlayer, VoxelWorld}, player::Input};

let settings: ServerSettings = ServerSettings::Builtin();
let mut world: VoxelWorld = VoxelWorld::FromMap(&map::Map::Load(&map::get_map_folder("lunar_surface"))?)?;
world.set_settings(settings);
let mut player: VoxelPlayer = VoxelPlayer::AtSpawn(&world, 0);
let input: Input = Input { forward: true, ..Input::default() };
let report = player.step(&world, &input, 1.0 / 60.0, 0);
let shot = world.trace_laser(player.get_camera_frame(&world));
```

## 6. The steps (the PR stack)

Each step is one layer of the stack, or a few when it is long. "Done when" lists the tests that must pass, from
section 7.

### Already in this stack

1. `game: refactor plan` (this file).
1. `game: build the legacy oracle against the demo with the infra stack merged`: layer 4 predated #379 and #381.
1. `game: scope the plan to phase A` (this change).
2. `delete the prototype square game and its management server`: `clients/`, `game_server/`, `management_server/`,
   `common/protocol/`'s square-game docs, and the docs that pointed at them.
3. `game: workspace scaffold`: the folders of section 3 with their READMEs, crates that build, and the Makefile.
4. `game: legacy oracle`: the crate that compiles the demo's code in place for regression tests (section 7.3).
5. `common/protocol: security case`: `security_case.md`, the argument for `authentication.md` v2, for a human or
   expert review.

Not yet written: `tools/udp_impair/` (section 7.6). The first agent that needs it (step A5 or phase C) adds it as
its own layer.

### Phase A: port without changing behaviour

- **A0, the infra stack lands in the demo.** #179 to #185 are based on an older `infra` branch; the infra stack
  brings them up to date with `staging/user-4gatepylon` and merges them into the demo (its owner's job, not this
  plan's). A1 starts only after that, so the game copies the app, the Linux server, the password and the title
  screen with everything else. If A1 can't wait, A1 copies the demo as it is, and each infra PR is then ported into
  `game/` as its own layer, in the infra stack's order, by agreement with the infra stack's agent.
- **A1, copy (done, #385).** Copy the demo's `src/`, settings files, wishlist and docs into `game/client/` and `game/` verbatim,
  then fix only what the new place breaks: the package and binary names (`warped-client`, `warped`), the fps_demo
  assets path (`fps_demo_assets.rs` takes `demos/` as the crate folder's parent, which is now `game/`), and any
  `include_str!` path. The window then plays exactly as the demo does, including `--serve`.
  Done when: the demo's unit tests pass in `game/client/`; 7.3.1 (oracle) and 7.3.3 (cross-version) pass; 7.4 views
  match the demo's pixel for pixel where Metal runs.
- **A2, drop the house engine.** Delete `world.rs`, `scene_data.rs`, `render_cpu.rs`, `render_metal.metal`,
  `physics.rs` (keep `Input`), `map_export.rs` and their uses, after splitting `render_metal.rs` (keep presenting,
  upscaling, overlays). Done when: the same tests as A1 pass with `--map lunar-surface`. Three layers: A2a drops
  the house's tools (`--autopilot`, `--export-map`, the house routes and benchmark views) and makes lunar-surface
  the default map, refusing `--map house-demo` with a pointer to the demo; A2b makes play voxel-only (no
  `--renderer cpu`, no house half in `multiplayer.rs`); A2c deletes the files.
- **A3, split `main.rs`** into modules, no behaviour change (several layers; each moves code verbatim).
- **A4, extract `warped-core`.** Move the modules of section 4 into `core/`, cutting every dependency on SDL, Metal
  and objc2 as listed there. Done when: `cargo check -p warped-core --target x86_64-unknown-linux-gnu` passes
  (7.1), and every oracle test passes against `warped_core` instead of the client.
- **A5, `warped-server`, moved as it is.** Build `warped-server` in `server/` from `bin/wormholes_server.rs` and
  the server half of `net.rs`, unchanged (it still runs its own UDP loop), with the same flags and exit codes.
  `make build-server-bundle` packs it, and `infra/startup.sh.tftpl` and `infra/Makefile` point at the game. Done
  when: the demo's server tests pass in `server/`, and 7.3.3 (cross-version) passes.
- **A5b, sans-IO server.** Rewrite the server half of `net.rs` and `serve_only` into `server/`, written sans-IO
  (`Game::handle(datagram, from, now_s) -> Vec<(to, datagram)>`, `Game::tick(now_s)`), with a thin UDP loop, from
  the infra stack's `wormholes_server` `main`. The binary becomes `warped-server` with the same flags; the same PR
  changes `infra/startup.sh.tftpl` and `make build-server-bundle` to the new name, and the infra stack reviews it.
  Remove `--serve` from the window. The password challenge (`authentication.md` v1) moves with the server and keeps
  its tests, and `warped-server` now refuses to start without a non-empty `--game-password-file` (exit code 2). Done
  when: 7.5 (in-process games) and 7.3.3 (cross-version, still byte-identical) pass.
- **A6, assets.** Move the used assets to `common/assets/` and point the game, both demos and `make
  package-mac-app` (which copies them into `Contents/Resources/assets/`) at them. Done when: both demos and the game
  build and draw (7.4 where Metal runs, otherwise their `check_*` functions find every file), and a packaged app
  starts (7.8).
- **A7, docs.** README, design doc, skills and `common/` docs name `game/` as the game; the demo's README says it is
  kept as the reference the game came from.

### Phase B: clean up

- Rename what the port kept for diffing (`VoxelWorld` to `World`, ...), one rename per layer.
- Make maps data-driven: any folder of `common/maps/` the window and server can read; no `MapName` enum, no
  `LunarSurface` checks in argument validation.
- Move Lunar Surface's own looks (spaceships, the `lunar` style, its dust) behind its map's `appearance.style`, and
  the black hole's growth behind a per-level rule.

### Phase C: network for 5v5 between friends in different cities

Each item is its own layer, with its tests (7.5, 7.6). Every layer that changes bytes raises `PROTOCOL_VERSION`,
updates `authentication.md` or the protocol docs if it touches them, and says in its PR that older windows can't
join. Cross-version tests with the demo stop at C1 (7.3.3 is retired; the oracle tests stay).

- **C1, encrypted sessions (`authentication.md` v2).** Several layers, in order:
  1. `core/src/net/password.rs`: generating, writing and parsing `warped2-` passwords, and the HKDF key, with the
     spec's password tests. `warped-server create-game-password`. The server and window refuse a missing or
     malformed password.
  2. `core/src/net/secure.rs`, sans-IO:
     - the handshake (`snow`, `Noise_NNpsk0_25519_ChaChaPoly_SHA256`) for both sides;
     - `Transport` sealing and opening with the counter as nonce and a 64-datagram replay window;
     - the limits on half-open handshakes;
     - the spec's handshake, transport and amplification tests.
  3. Wire it in. Every game message travels in `Transport` datagrams. v1's `Challenge`, `Refused` and token go.
     Sessions move to a player's new address (home routers sometimes change a player's port mid-game).
     `OtherVersion` replaces `Refused` for versions.
  4. With the infra stack: the title screen asks for a `warped2-` password per server, and `infra/` generates
     passwords with `create-game-password` (the infra stack's layer, reviewed by the refactor).
- **C2, write the protocol up.** `common/protocol/messages.md`: every game message's layout, from the code, with the
  tests that hold it to the page. No byte changes.
- **C3, smaller pose, 10 players.** Each player's eye frame (16 `f64`, 128 bytes) becomes a position in the map's
  coordinates plus a unit quaternion (7 `f32`, 28 bytes): plain `f32` matrices would lose centimetres far out in
  hyperbolic space. `MAX_PLAYER_COUNT` becomes 10. A test asserts a 10-player snapshot fits in 1,200 bytes, under
  the 1,472 bytes of one ordinary internet packet, so it is never split into IP fragments.
- **C4, stale snapshots dropped.** A window drops a snapshot whose `Transport` counter is lower than the newest
  snapshot's it has shown, so a late, reordered snapshot never moves players back. The counter of C1 is the
  sequence number, so no bytes are added.
- **C5, eliminations that can't be lost.** The snapshot lists eliminated players too, with a flag and the time left
  until they may respawn, so a lost `Eliminated` datagram no longer leaves a "ghost" player, and the server, not the
  window, decides when a player may respawn.
- **C6, hits that can't be lost or forged by mistake.** A window numbers its hits and resends each until a snapshot
  acknowledges it; it reports the weapon and whether the head or the body was hit, and the server works out the
  damage from its settings instead of taking `hits_to_kill` from the window.
- **C7, smoothing.** Other players are drawn about two snapshots in the past, interpolated along geodesics, so they
  move smoothly on displays faster than 60 Hz and through jitter.

### Phase D: a match across levels

- **D1, `rules`.** The wavefront of `DESIGN.md` as pure functions in core: which level comes next, who attacks, who
  won the match. Unit tests cover every row of the design doc's example match.
- **D2, rounds on the server.** `--maps`, round state in snapshots, a `RoundStart { round, map }` message.
- **D3, map switching in the window.** Load the next map between rounds (rebuild the GPU buffers, meshes, clearance
  grids and textures) behind a loading screen; join loads the server's map.

### Phase E: other spaces and topologies

- **E1, one space type.** `core/src/space.rs`: curvature k = -1, 0, +1, with the inner product, the pair of functions
  C_k(s), S_k(s) (cosh/sinh, 1/s, cos/sin), distance and translations; every `minkowski_dot`, `cosh`, `make_boost`
  in core goes through it. In Metal, the kind of space is a function constant, so each space gets its own compiled
  pipeline. Hyperbolic results must not change (7.3.1 at 1e-12 relative).
- **E2, flat space** works end to end on a test map (E³ with the existing structures).
- **E3, map format v2** (`common/maps/README.md`, `compatible-changes` skill): a `topology` field (face pairings,
  for T² × ℝ), a ground that is the inside of a sphere (City Sphere), gravity along the ground's normal, zones
  measured on that ground. Raises `version`; old maps still load.
- **E4, wrapping.** Rays and players cross a face pairing; warps act through their nearest copy; the map page and
  minimap draw the fundamental domain.
- Spherical space (S³) only if a level needs it; its chart bound in `shapes.rs` and `voxel_clearance.rs` goes the
  other way (chart distances exceed true ones there).

### Phase F: the other levels

Each level of the design (Dust, Snow Planet, City Sphere, Dark Forest) gets an exporter from its demo in
`demos/levels/` into `common/maps/<name>/`, like Lunar Surface's `export_map.mjs`, then is played and tuned.

### Phase G: running games on a VM

The infra stack owns this (`infra/`: Terraform on Google Cloud, one systemd service per game port, the password
from the VM's metadata, a download page). This plan only keeps the server's interface stable (section 3.1) and
adds `--json` statistics the infra stack can watch. For budgeting: outbound traffic is ≈4 GB per game-hour with
today's snapshot sizes, about $0.35 per game-hour at typical cloud rates, roughly half after C3.

## 7. Testing plan

The machine running the tests may be a cloud macOS VM: it may have no GPU, a virtual GPU without some Metal
features (MetalFX upscaling in particular), no display, and no one to play. So almost every test runs without a
window or a GPU, multiplayer is tested by headless bots and a simulated network, and what truly needs eyes and
hands goes to the user's playtest at the end (7.8). Each tier says where it runs:

| Tier | What | Needs | Runs in |
| --- | --- | --- | --- |
| 7.1 | Formatting, lints, Linux build check | Rust | every layer |
| 7.2 | Unit tests | Rust | every layer |
| 7.3 | Regression against the demo | Rust, (7.3.3) the demo's build | every layer of phases A and B |
| 7.4 | Pictures and frame times against the demo | Metal (and MetalFX for full fidelity) | layers that touch drawing |
| 7.5 | Multiplayer on a simulated network | Rust | from A5 on |
| 7.6 | Multiplayer over real UDP with impairment | Rust, loopback | from A5 on, and every phase C layer |
| 7.7 | Performance budgets | Rust (7.4's GPU parts: Metal) | phase C, and before each merge to staging |
| 7.8 | The user's playtest | the user's Mac, friends | end of each phase |

`make test` runs 7.1, 7.2, 7.3 (but 7.3.3), 7.5. Slower tiers have their own targets (section 7.9). A tier that
can't run on the machine (no Metal, for example) is reported as "Not run: <reason>" in the PR, never as passing.

### 7.1 Static checks

- `make fmt`: `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
- `make check-linux`: `cargo check -p warped-core -p warped-server --target x86_64-unknown-linux-gnu` (after
  `rustup target add x86_64-unknown-linux-gnu`). This keeps the server deployable on Linux; it needs no linker.

### 7.2 Unit tests

- The demo's unit tests move with their modules and must keep passing unchanged during phase A (they are the first
  regression test).
- Every new or changed function gets tests chosen by partitioning its inputs, with the coverage table in the test
  module's docstring (`unit-tests` skill). Partitions worth stating for the new code:
  - `net::protocol`: each message kind × {smallest, typical, largest (10 players, every field at its maximum)};
    truncated, over-long and unknown-kind datagrams; round trips bit for bit.
  - Authentication and encryption: the partitions listed in `common/protocol/authentication.md` (v1's until C1, kept
    from the infra stack's tests; then v2's password text, server start, handshake, transport and amplification).
  - `rules` (phase D): every row of the design doc's example match; a team winning at depth 0, 1, 2; a defender
    winning its own level.
  - `space` (phase E): k ∈ {-1, 0, +1} × {a point at the origin, near, far (10 R)}: geodesics stay on the manifold,
    distance is symmetric and satisfies the triangle inequality, translations preserve the inner product, chart
    lines are geodesics.

### 7.3 Regression against the demo

The demo is the reference: the port must behave exactly as it did.

#### 7.3.1 The oracle: the demo's code compiled beside the game's

`game/legacy_oracle/` is a library crate whose modules are the demo's own source files, included in place with
`#[path = "../../../demos/hyperbolic_wormholes_rust/src/<file>.rs"]`; nothing is copied, and the demo is not
touched. Tests in `game/` add `legacy-oracle` as a dev-dependency and compare it with the game's code on the same
inputs, so any difference the port makes shows up at once, without golden files to regenerate.

What they compare (each a test, each written in the layer that ports the code it covers):

| Area | Inputs | Equal means |
| --- | --- | --- |
| Map loading | `lunar_surface` at the server settings' curvature | same structures, warps, spawns, consoles, boundary, zones |
| Terrain | a 64 × 64 grid of Fermi (u, v) over the crater, plus the rim | same heights |
| Shapes and meshes | 1,000 seeded points near each structure | same distance bounds and hits |
| Walking | scripted inputs, 60 Hz, 20 s each: walk to a wall, step up a block, jump onto a bunker, sprint until out of energy, crouch, walk into the wormhole and out of its partner, fall into the black hole, climb the crater's wall | same feet and eye frames every step, same `StepReport` messages |
| Lasers | a fan of 32 × 32 directions from each spawn, from the wormhole's mouths and from under the black hole at strengths 0, 0.5, 1 | same `Shot`: segments, hit point, structure, warps passed |
| Thrown wormholes | throws from each spawn at 8 yaws × 3 pitches | same flights and placements |
| Shot noise | seeds 0 to 99, both guns, cold and hot | same angles and speckle |
| Settings | the shipped `server_settings.yaml` and `player_settings.yaml` | same parsed values, same JSON sent in the welcome |
| Net layouts (until C1) | every message kind, built by the demo's `Client` and server, read by the game's, and the other way round, over loopback | same values both ways |

Tolerance: exact equality (`==` on every `f64`) while the code is a port, since the same operations in the same
order give the same bits. Once a step changes maths on purpose (E1), the test switches to a stated tolerance
(1e-12 relative) and the PR says why.

Gotchas: the oracle compiles the demo's modules with their own `crate::` paths, so it includes every module they
need (the list is in `legacy_oracle/src/lib.rs`), including `key_bindings.rs` (SDL) and `voxel_textures.rs`
(objc2-metal). It builds on macOS only, and needs CMake for SDL, like the demo.

#### 7.3.2 The demo's own tests

`make -C demos/hyperbolic_wormholes_rust test` must still pass after every layer, to prove the demo was left
working (it may only change in step A6).

#### 7.3.3 Cross-version play (phase A and B only)

Until C1 changes the messages, the game and the demo must play together, which tests the port end to end:

- the demo's server (`cargo run --release -- --serve` in the demo) with the game's bots joining it;
- the game's server with the demo's bots (`--bot`; check it runs without Metal, else use the game's bots only);
- both sides with the same `--game-password-file`, and once with different ones (refused both ways);
- both ways, a scripted game: 4 bots join, choose teams, hold a console until it turns off, shoot each other until
  one is eliminated and respawns, throw wormholes, leave. Every event both sides log must agree.

`make test-cross-version` builds the demo and runs both directions.

### 7.4 Pictures and frame times against the demo (needs Metal)

- `make check-environment` (section 8) says whether Metal and MetalFX are available.
- **Pictures.** Render the fixed views of the demo's `--bench-views --map lunar-surface` with the demo and with the
  game, at the same size (1280 × 800 and 2560 × 1600), and compare each pair with `tools/compare_views` (to be
  written in A1): the largest per-channel difference and the share of pixels that differ. While the renderer is a
  verbatim port, the pictures must be identical on the same machine. Save both sets and a side-by-side sheet under
  `game/.artifacts/compare_views/<run_id>/` (`graphical-changes` skill for sheets).
- **Frame times.** The same views with `--frame-count 60`: the game's median GPU time per view within 3% of the
  demo's on the same machine (both runs back to back, nothing else running). On a virtual GPU, record the numbers
  but don't gate on them.
- **Without MetalFX** (a virtual GPU may lack it): run with `--render-scale 1`, so no upscaling is needed; if the
  window still requires MetalFX, that is a bug to fix in A2 (fall back to drawing at full size).
- **Without Metal:** this tier is "Not run"; the user checks pictures in 7.8.

### 7.5 Multiplayer on a simulated network

Because the server (`Game`) and the window's connection (`Connection`) are sans-IO (A5), a test can run a whole game
in one process, on a simulated clock, with no sockets:

- `core/src/net/sim.rs` (test support): a `SimNetwork` that carries datagrams between endpoints with, per link, a
  latency, a jitter, a loss rate, a reordering rate and a duplication rate, all from one seeded random generator, so
  every run with the same seed is the same run. Time advances in fixed steps (1 ms).
- Bots (`core/src/bot.rs`) play scripted roles: walk to the enemy's console and hold `E`; strafe and shoot at the
  nearest visible enemy (with `trace_laser`, so shots bend through warps); throw wormholes; idle; leave and rejoin.
- Network profiles, chosen to match the user's setting (a server in Oregon, friends from Seattle to Los Angeles):

  | Profile | One-way latency | Jitter | Loss | Reorder | Duplicate |
  | --- | --- | --- | --- | --- | --- |
  | `lan` | 1 ms | 0 | 0 | 0 | 0 |
  | `west_coast` | 15 ms | ±3 ms | 0.5% | 0.1% | 0 |
  | `bad_wifi` | 30 ms | ±15 ms | 3% | 1% | 0.5% |
  | `awful` | 80 ms | ±40 ms | 10% | 5% | 2% |

- Scenarios, each over every profile, with 10 bots (5v5) unless said otherwise:
  1. Join: all 10 get ids and teams they asked for, an 11th gets "full", the lobby counts are right.
  2. Consoles: a bot holds `E` for the settings' time and the console turns off; letting go resets it; a team
     loses when all its consoles are off; the next game starts after `--next-game-after-s`.
  3. Combat: hits eliminate after the right number of hits per weapon and zone; kills and deaths add up; friendly
     fire eliminates but scores no kill; respawn happens after `respawn.delay_s`, not before.
  4. Thrown wormholes: placed, linked, destroyed by enemy shots.
  5. Churn: bots leave, time out, rejoin, and change address mid-game (the session follows them after C1).
  6. A 10-minute match of game time (runs in seconds of real time).
  7. Strangers: an endpoint without the password, one with a wrong password, one replaying datagrams captured from
     a player (from its own address), and one sending random bytes, all while a 5v5 game runs. None gets a slot or
     changes the game; no reply to them is longer than what they sent; the players notice nothing.
- Invariants checked every simulated millisecond, on every profile:
  - every player sees every elimination of themselves within 1 s (no "ghosts"; after C5);
  - every hit a window sends is applied exactly once (after C6);
  - no window shows a player position older than 200 ms on `west_coast` (after C4, C7);
  - every snapshot fits in 1,200 bytes (after C3); the server never panics on any datagram.
- On `lan` and `west_coast` before phase C, the tests document today's behaviour (e.g. how often a lost
  `Eliminated` leaves a ghost) rather than gate on it, and each phase C layer turns its invariant into a gate.
- A fuzz test feeds the server random and mutated datagrams for 10^6 iterations (seeded): it must never panic.

### 7.6 Multiplayer over real UDP with impairment

The same scenarios, end to end with the real binaries, to catch what a simulation can't (sockets, timing, the OS):

- `tools/udp_impair/`: a UDP proxy between the windows (or bots) and the server that applies a profile of 7.5 to
  real datagrams in both directions. It needs no administrator rights, unlike macOS's `dnctl` and `pfctl`, so it
  runs on a cloud VM.
- `make test-network PROFILE=west_coast BOT_COUNT=10`: starts `warped-server --json --game-password-file <a
  generated password>` on loopback, `udp-impair` in front of it, and 10 `warped --bot` processes through it with the
  same password for 2 minutes, plus one with a wrong password that must be refused; then checks the server's and bots' JSON
  logs for the invariants of 7.5, and saves logs, per-second bytes and the summary under
  `game/.artifacts/test_network/<run_id>/` with a `command.txt`.
- One window (`warped --join`) instead of one bot, when Metal is available, to check the game draws other players
  smoothly under each profile (screenshots of the run per `graphical-changes`).

### 7.7 Performance budgets

Measured on the machine at hand and recorded in the PR with the system (`system-docs` skill). They are first guesses
for the user to confirm (`experiment-decisions` skill):

| What | Budget | How |
| --- | --- | --- |
| Server time per tick, 10 bots | p99 under 1 ms | `warped-server --json` per-second statistics |
| Snapshot size, 10 players | at most 1,200 bytes | unit test (C3) |
| Download per window | at most 80 KB/s after C3 | 7.6's byte counts |
| Server upload per game | at most 1 MB/s | 7.6's byte counts |
| Window frame time (Metal) | not worse than the demo by more than 3% | 7.4 |
| Map switch between rounds (phase D) | under 3 s on the user's Mac | log timestamps |

### 7.8 The user's playtest

At the end of each phase, following the `playtesting` and `interactive-review` skills: commands to run the server
and two windows on one Mac, then the server on a VM with friends; what to try (the phase's changes) and what was
already checked automatically. The user's confirmation, not tests, marks wishlist items done.

### 7.9 Commands

| Command | Runs |
| --- | --- |
| `make test` | 7.1's lints and formatting check, 7.2, 7.3.1, 7.3.2, 7.5 |
| `make check-linux` | 7.1's Linux build check |
| `make check-environment` | section 8 |
| `make test-cross-version` | 7.3.3 |
| `make compare-views` | 7.4 (Metal) |
| `make test-network PROFILE=<profile> BOT_COUNT=<count>` | 7.6 |

Targets that don't exist yet are added by the step that first needs them, with `make help` and the Makefile's
docstring updated in the same commit.

## 8. Checking the machine you run on

Run `make check-environment` first and put its output in your first PR's Testing section. It prints the macOS
version and chip, whether a Metal device exists and what it is, the Rust, Cargo and CMake versions, and whether
the Linux target is installed. Then, if anything is missing:

- **No CMake:** `brew install cmake` (SDL3 is built from source).
- **No Metal device** (a VM without GPU): skip 7.4 and the window parts of 7.6, and say so.
- **A virtual GPU** (Apple's Virtualization framework gives macOS guests one): Metal works, but MetalFX may not;
  use `--render-scale 1`, and treat frame times as rough.
- **No display:** everything but `make join-game` runs without a window (`--bench-views`, `--play-route`, `--bot`
  are offscreen or windowless).

## 9. Working on the stack together

- The stack lives on GitHub as linked PRs into `staging/user-4gatepylon` (`stacked-prs` skill). Layers 1 to 3 (and
  4, 5) were opened by the agent who wrote this plan; each later step adds layers on top with
  `gh stack add devin/<step>` (or `claude/<step>`, `codex/<step>`).
- Work in a worktree of your own (`devin-worktrees`, `managing-agents`), never in the user's checkout.
- Several agents can work at once only on steps that don't depend on each other: phase A is a chain; within phase C,
  C1 must come first, then C2 to C7 can be parallel layers rebased in order; phases E and F can start once A4 is
  in. Before starting a step, say so in the PR of the layer below (a comment) so no one doubles it.
- Change a lower layer with `gh stack checkout <branch>`, commit there, then `gh stack rebase --upstack` and
  `gh stack push`.
- When a step's done, tick it in this file (in that step's own layer) by adding "(done, #<PR>)" after its title.
- **With the infra stack** (paused, section 2): keep 3.1's workloads working, and change `infra/` in the same PR
  when a step changes what it runs. Run `git log origin/staging/global` at the start of each session to see whether
  the infra stack has started again.

## 10. Open questions for the user

- The budgets of 7.7 and the network profiles of 7.5 are first guesses: are they right for your friends?
- Phase C's order: is smoothing (C7) more urgent than the hit and elimination fixes (C5, C6)?
- City Sphere's design may change to one floor (`DESIGN.md`); phase E's sphere ground waits for that decision.
- Should a management server (to start games from a web page or the window) come back in phase G, or are the infra
  stack's Terraform-managed `warped-server` processes enough for tournaments?
- Passwords are generated (`warped2-` and 36 letters), not chosen, so they can't be guessed offline. Is copying a
  generated password into WhatsApp acceptable, or do you want memorable passwords? Those would need slow hashing
  (Argon2id) and a minimum length, and a weak one could still be guessed by someone who records a handshake.
