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

Dark Forest ([`../dark_forest_rust/`](../dark_forest_rust/README.md)) runs on the `fps_demo` engine and doesn't use
it yet; [`PLAN.md`](PLAN.md) is the plan for sharing movement, guns, health and the HUD with every level, Dark Forest
included.

## What is in it

| Module | What it does | Builds on Linux |
| --- | --- | --- |
| `geometry` | the hyperboloid model's maths (Lorentz boosts, rotations, distances) | yes |
| `app_bundle` | where the game's files are: in `Warped.app` or in the checkout; `get_model_path` | yes |
| `fps_demo_assets` (+ `build.rs`) | reads assets from disk or from the models built into the program | yes |
| `shapes` | the smooth solids structures are made of, and `MapShape`, a map file's description of one | yes |
| `play_settings` | the settings files' sections every level reads alike: movement, sprint energy, both guns, consoles, respawn, a hit's slowing and look, aiming, thrown wormholes, the view and the minimap | yes |
| `shot_noise` | how far each shot strays from the crosshair, and its beam's speckle | yes |
| `mesh`, `avatar_model` | glTF models of the maps' structures and the players' robot body | yes |
| `spaceships` | the spaceships in the sky | yes |
| `key_bindings`, `exit_prompt` | a player's controls (SDL), and the "leave the game?" prompt | no (SDL) |
| `texture_arrays` | mipmapped Metal texture arrays from image files | no (Metal) |
| `render_mesh` | the maps' models as GPU buffers for the voxel renderer | no (Metal) |
| `viewmodel` (+ `viewmodel.metal`) | the gun held in hand, drawn over the scene | no (Metal) |
| `overlay` | what is drawn over a frame: HUD rectangles, the wand, the minimap, the text mask, a hit's effect | no (Metal) |
| `hud_bars` | the HUD's bars: health, ammo, sprint energy, the console bar, and their glows | no (Metal) |
| `hud` | the crosshair | no (Metal) |
| `weapons` | the two guns' state (`Weapons`: fire, reload, heat and overheat, recoil, aim, each shot's noise), `Gun`, how much a hit weighs, the gun in hand's place and sway (`make_wand`, `get_sway`), `MUZZLE_M` | no (Metal) |

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
  `update_gun` (the step phase from the level's walking), the HUD's bars and lines. City Sphere's own
  `CitySession` (`city.rs`) still has its own guns, written apart from Lunar Surface's.
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
use warped_shared::{app_bundle, avatar_model, exit_prompt, fps_demo_assets, geometry, hud_bars, key_bindings, mesh,
    overlay, play_settings, render_mesh, shapes, shot_noise, spaceships, texture_arrays, viewmodel};
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
`make test`), and `make bench-views` in each level, comparing the pictures with the ones before the change. The
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
