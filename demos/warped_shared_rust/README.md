# warped_shared: the Rust levels' shared code

`warped_shared` is a library crate holding the code that the Rust level demos of Warped share, so each level stays its
own program (its own map, world, renderer and server) while the code they have in common is written once. A file here
was byte for byte the same in every level before it moved.

## Who uses it

| Level | Crate | Binary |
| --- | --- | --- |
| Lunar Surface (the reference) | [`../hyperbolic_wormholes_rust/`](../hyperbolic_wormholes_rust/README.md) | `hyperbolic-wormholes` |
| Dust | [`../dust_rust/`](../dust_rust/README.md) | `hyperbolic-wormholes` |
| Snow Planet | [`../snow_planet_rust/`](../snow_planet_rust/README.md) | `snow-planet` |
| City Sphere | [`../city_sphere_rust/`](../city_sphere_rust/README.md) | `city-sphere` |
| The match (and its own Lunar Surface) | [`../warped_match_rust/`](../warped_match_rust/README.md) | `hyperbolic-wormholes` |
| Dark Forest | [`../dark_forest_rust/`](../dark_forest_rust/README.md) | `dark-forest` |

Dark Forest runs on the `fps_demo` engine; it uses the HUD, the hit effect, the overlay rectangles, `shot_noise`,
the settings sections, `weapons`, `movement` (through `Ground`), `gait`, `avatar_model`'s clip names and
`third_person`; [`PLAN.md`](PLAN.md) has what is left.

## What is in it

| Module | What it does | Builds on Linux |
| --- | --- | --- |
| `geometry` | the hyperboloid model's maths (Lorentz boosts, rotations, distances) | yes |
| `app_bundle` | where the game's files are: in `Warped.app` or in the checkout; `get_model_path` | yes |
| `fps_demo_assets` (+ `build.rs`) | reads assets from disk or from the models built into the program | yes |
| `shapes` | the smooth solids structures are made of, and `MapShape`, a map file's description of one | yes |
| `play_settings` | the settings files' sections every level reads alike: movement, sprint energy, both guns, consoles, respawn, a hit's slowing and look, aiming, thrown wormholes, the warps' pull (`gravity`), the view and the minimap | yes |
| `health` | the damage a hit adds and when it eliminates (the game server's count), what a fall in this player's health shows and does (`HitFeedback`: a shot shows the hit effect and slows, a blizzard's slow damage shows once per hit's worth), the hit effect's fading | yes |
| `shot_noise` | how far each shot strays from the crosshair, and its beam's speckle | yes |
| `movement` | the rules of moving (`MoveState::step`): crouching, sprint energy, the head bob's steps, a hit's and crouching's slow; each level moves the player in its own space (`VoxelPlayer::step`, City Sphere's `CityWorld::move_player`, Dark Forest's `Forest` through the `Ground` trait and a step's `MoveIntent`); a crouch's height share and its 4-bit network level (`get_crouched_height_frac`, `encode_crouch_level`) | yes |
| `gravity` | the pull of black holes and wormhole mouths on a player on the ground (W-144): a speed toward each warp, full near its horizon or throat and falling off to `gravity.reach_m`, a mouth's fading at its throat, all capped at `gravity.max_pull_per_walk_speed` of walking speed; each level gives its warps in its own space (`get_flat_pull_m_per_s`, `get_hyperbolic_pull_m_per_s`) and adds the pull to its walk (`add_pull`). Its settings, `GravitySettings` in `play_settings`, take built-in values when left out of a settings file | yes |
| `mesh`, `avatar_model` | glTF models of the maps' structures and the players' robot body, posed by a clip, and squashed for a crouched player (`make_squashed`) | yes |
| `spaceships` | the spaceships in the sky | yes |
| `key_bindings`, `menu_and_map` | a player's controls (SDL), with saved ones from older builds upgraded (`upgrade_saved_names`); what covers the view (`MenuAndMap`: the menu on `Esc`, which frees the mouse and whose QUIT page leaves the level, and the map while `Tab` is held, over the game going on; W-140, hold Tab for the map, Esc for the menu), and the menu's tab bar with its optional MATCH page (`format_tab_bar`, `MATCH_PAGE_NAME`) | no (SDL) |
| `texture_arrays` | mipmapped Metal texture arrays from image files | no (Metal) |
| `render_mesh` | the maps' models as GPU buffers for the voxel renderer | no (Metal) |
| `viewmodel` (+ `viewmodel.metal`) | the gun held in hand, drawn over the scene | no (Metal) |
| `overlay` | what is drawn over a frame: HUD rectangles, the wand, the minimap, the text mask, a hit's effect | no (Metal) |
| `hud_bars` | the HUD's bars: health, ammo, sprint energy, the console bar, and their glows | no (Metal) |
| `hud` | the HUD: the crosshair, the bars (health, ammo or heat, sprint energy, the console being turned off; `HudBars`, `make_bar_rects`) and their pulses (`HudPulses`), the hit effect's look (`make_hit_effect`), the banner's lobby and "YOU DIED" lines, the HUD line's gun | no (Metal) |
| `gait` | which of the robot's clips another player shows, and where in it, from their speed over the ground and take-off (Lunar's `avatar_animation` rules; every level's) | yes |
| `third_person` | the third-person view's switch (`ViewSwitch`: `V`, eased, back to the eye while aiming) and its boom in flat space (`place_camera_flat`, City Sphere's and Dark Forest's); Lunar Surface's copies place it in their curved space themselves | yes |
| `app` | the interface between the one-window app (`../warped_mvp_rust/`) and a level: `LevelJoin`, `LevelEnd`, `MatchLink`, `LevelWindow`, `play_level_fn_t` | yes (`LevelWindow` macOS) |
| `metal_window` | the one window a program draws in with Metal: `MetalWindow` (`Open`: SDL3, a window made with `SDL_WINDOW_METAL`, its `CAMetalLayer` in `BGRA8Unorm`, not framebuffer-only, on the system's Metal device, and the event pump; full screen or hidden by `WindowOptions`), lent to a level with `get_level_window` (`NoMatchLink` for a level run alone); `set_window_title` through a shared reference. The one-window app and every level run alone open their window with it | no (SDL, Metal) |
| `match_state`, `match_protocol`, `match_menu` | the match's rules (and free play on one level), what its server and clients share (levels file, view, `MatchClient`, the game master's `play` message, the network beacon) and its progression page and MATCH page lines; moved from `../warped_match_rust/` | yes |
| `game_settings` | the game master's settings (W-158, per-map game settings): each level's default server settings and the overrides laid over them (one shared section, one per level), the `settings?`, `set` and `reset` messages and the `settings <json>` answer, each round's merged settings | yes |
| `page` | a menu page's shapes in the debug font, drawn with SDL (`draw_shapes`) | yes (`draw_shapes` macOS) |
| `player_settings` | `player_settings.yaml`: reading (old saves upgraded), checking, saving, and the numbers a settings page edits; the levels' own copies switch to it when they become libraries | no (SDL keys) |
| `settings_editor` | a settings page any menu shows: numbers (stepped or typed), key bindings, buttons, over a settings file's keys | yes |
| `weapons` | the two guns' state (`Weapons`: fire, reload, heat and overheat, recoil, aim, each shot's noise), `Gun`, how much a hit weighs, the gun in hand's place and sway (`make_wand`, `get_sway`; `make_wand_camera_from_muzzle` for a level that draws a gun model there), `MUZZLE_M` | no (Metal) |

`overlay` and `texture_arrays` are parts of files that differ per level, taken out verbatim: the overlay types of
each level's `render_metal.rs` (which re-exports them, so `render_metal::OverlayRect` still works) and `make_mips` and
`make_array` of each level's `voxel_textures.rs`.

## What stays in each level, and why

Everything else differs between levels, or needs a module that does:

- `map.rs` (each level adds map fields, such as Dust's atmosphere, and its own `MapName`s), `settings.rs` (each embeds
  its own `server_settings.yaml` and `player_settings.yaml` and composes the shared
  `play_settings` sections into its `ServerSettings` and `PerPlayerSettings`; City Sphere adds `glass`), `voxel_world.rs`, `render_voxel.*`, `map_layout.rs`,
  `net.rs`, `multiplayer.rs`, `main.rs`, `title.rs`, `credits.rs`, `dust.rs`, `terrain.rs` (Dust's own), and
  `voxel_textures.rs` (each level's own surface sets).
- Files identical in every level but needing one of those: `avatar_animation`, `black_hole_growth`,
  `voxel_clearance`, `minimap` and `third_person` need `voxel_world`; `menu` needs `net`, `settings` and `credits`;
  the house demo's engine (`world`, `scene_data`, `physics`, `render_cpu`,
  `map_export`) needs `map` and `thrown_wormholes`.
- In each `main.rs`, the `Session` keeps its guns in `weapons: Weapons` and calls it; what stays is what needs
  the level's world: `Tracer` (a shot's beam, `voxel_world`'s segments), tracing shots, the view's recoil,
  `update_gun` (the step phase from the level's walking), the HUD's text lines (speed, height, the map's
  own lines); its bars, hit effect and banner's lobby and death lines come from `hud`, and its hit feedback
  and the server's damage count from `health`. City Sphere's own
  `CitySession` (`city.rs`) keeps its guns in a `Weapons` too and traces each shot through its glass and wormholes.
- Files identical in all but City Sphere: `render_metal.*` (City adds its own frame kind), `thrown_wormholes`
  (City adds `update_placing`, which Lunar's `update` could call), `third_person` (City makes four constants
  `pub(crate)`), and `console_reach` and `picture_saver` (City has none). `zone_walls` differs in Snow Planet only
  by a field of its world. They all need a level-specific module too, so they wait for [`PLAN.md`](PLAN.md).

## How a level uses it

A level crate depends on it by path and brings its modules in at its root, so the level's own files keep naming them
`crate::geometry` and so on:

```toml
# Cargo.toml
[dependencies]
warped_shared = { path = "../warped_shared_rust" }
```

```rust
// src/main.rs, beside the level's own `mod` lines
use warped_shared::{app_bundle, avatar_model, fps_demo_assets, geometry, hud_bars, key_bindings,
    menu_and_map, mesh, overlay, play_settings, render_mesh, shapes, shot_noise, spaceships, texture_arrays, viewmodel};
use warped_shared::hud::get_crosshair_rects;
use warped_shared::weapons::{Gun, Weapons, get_sway, make_wand, take_hit_weight};
```

A level's game server alone (`src/bin/*_server.rs`, built for Linux by `make build-server-bundle`) uses only the
modules that build there: `use warped_shared::{geometry, play_settings, shapes, shot_noise};`.

A level's `settings.rs` re-exports the `play_settings` sections (`pub use crate::play_settings::{...}`), so its
own files keep naming them `crate::settings::RifleSettings`. `shot_noise`'s tests read Lunar Surface's
`server_settings.yaml` for the guns' built-in numbers.

## Adding a level

1. Copy a level crate (Lunar Surface's, the reference) and give its package and binaries new names.
2. Keep the `warped_shared` dependency and the `use warped_shared::{...}` line; never copy a shared file back into
   the level. A level that needs a shared file to behave differently passes the difference in (a parameter, a
   setting, a trait the level implements) in a change to this crate that keeps every other level the same.
3. Add the level to the table above and to `LEVELS` in the `Makefile`.

## Changing a shared file

A change here changes every level, so check them all: `make test` here, then `make test-levels` (each level's
`make test`), and `make bench-views` in each level, comparing the pictures with the ones before the change. Each level's
play trace (`src/play_trace.rs`, an ignored test) walks a player through a fixed script of keys and writes
every step's state exactly; record it before and after a change to movement and compare the files:
`PLAY_TRACE_MAP=lunar-surface PLAY_TRACE_FILE=/tmp/after.txt cargo test --release record_play_trace --
--ignored`. The
overlay structs are also declared in each level's `render_metal.metal`, and `viewmodel.metal` is compiled by each
level's renderer, so a change to their layout changes those shaders in the same commit.

The key bindings' test reads Lunar Surface's `player_settings.yaml`, the file every level's copy must equal (the
levels' own tests check theirs against it, W-126 (same play in every level)).

## Commands

```bash
pushd demos/warped_shared_rust
make help          # the targets
make test          # the library's tests
make test-levels   # make test in every level crate that uses it
make fmt           # rustfmt and clippy
popd
```

`cargo build` writes to `.build/` (`make clean` removes it); the library writes no run outputs. Each level builds its
own copy of the library into its own `.build/`.

## Relation to `game/` (the refactor plan, #375)

`game/REFACTOR_PLAN.md` (open PR #375) plans one `game/` workspace whose `warped-core` has no window, GPU or Apple
dependency, with `warped-client` for drawing. This crate is a step for the demos, not that crate: it holds both
windowless code (`geometry`, `shapes`, `mesh`, ...) and drawing code (`viewmodel`, `hud_bars`, ...), and gates the
drawing code to macOS. When the plan's step A4 runs, its windowless modules go to `warped-core` and the rest to
`warped-client`. The plan's legacy oracle (section 7.3.1) compiles the demo's files in place; for the files moved
here, its `#[path]`s point at `demos/warped_shared_rust/src/` instead of the demo's `src/`.
