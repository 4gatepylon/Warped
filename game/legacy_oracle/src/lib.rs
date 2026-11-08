//! `legacy-oracle` is the Rust demo's simulation, compiled from the demo's own source files where
//! they are (`demos/hyperbolic_wormholes_rust/src/`), untouched and uncopied. The game's tests add
//! it as a dev-dependency and compare the game's results with the demo's on the same inputs, so
//! any difference the port makes shows up at once (REFACTOR_PLAN.md, section 7.3.1).
//!
//! Interface: the demo's modules under their own names, the ones its world, network and settings
//! need: `voxel_world`, `net`, `settings`, `map`, `geometry`, `terrain`, `shapes`, `mesh`,
//! `physics`, `thrown_wormholes`, `shot_noise`, `black_hole_growth`, `zone_walls` and the modules
//! they use. The demo's `main.rs` and its window and Metal renderers are left out.
//!
//! Example: `let world = legacy_oracle::voxel_world::VoxelWorld::FromMap(&map)?;`
//!
//! Must know:
//! - Gotchas:
//!   - The modules keep the demo's `crate::` paths, so each module a listed one uses must be
//!     listed too; a new `crate::` use in the demo needs a line here.
//!   - `CARGO_MANIFEST_DIR` is this crate's folder, two levels down like the demo's, so the
//!     demo's `../../common/maps` still finds the maps; its fps_demo assets aren't found.
//!   - The demo's private items that only its `main.rs` used are dead code here, hence the allows.

#![allow(dead_code, unused, clippy::all)]

#[path = "../../../demos/hyperbolic_wormholes_rust/src/avatar_model.rs"]
pub mod avatar_model;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/black_hole_growth.rs"]
pub mod black_hole_growth;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/fps_demo_assets.rs"]
pub mod fps_demo_assets;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/geometry.rs"]
pub mod geometry;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/key_bindings.rs"]
pub mod key_bindings;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/map.rs"]
pub mod map;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/mesh.rs"]
pub mod mesh;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/net.rs"]
pub mod net;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/physics.rs"]
pub mod physics;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/render_cpu.rs"]
pub mod render_cpu;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/scene_data.rs"]
pub mod scene_data;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/settings.rs"]
pub mod settings;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/shapes.rs"]
pub mod shapes;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/shot_noise.rs"]
pub mod shot_noise;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/terrain.rs"]
pub mod terrain;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/thrown_wormholes.rs"]
pub mod thrown_wormholes;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/voxel_clearance.rs"]
pub mod voxel_clearance;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/voxel_textures.rs"]
pub mod voxel_textures;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/voxel_world.rs"]
pub mod voxel_world;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/world.rs"]
pub mod world;
#[path = "../../../demos/hyperbolic_wormholes_rust/src/zone_walls.rs"]
pub mod zone_walls;
