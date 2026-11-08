//! `warped-core` is everything the game window (`warped`) and the game server (`warped-server`)
//! share: maps, the world and walking in it, lasers, thrown wormholes, settings, the network's
//! messages and the window's side of a connection, and bots for tests. It has no window, GPU or
//! Apple dependency, so it and the server build on Linux.
//!
//! Interface: so far `app_bundle`, `avatar_model`, `black_hole_growth`, `fps_demo_assets`,
//! `geometry`, `map`, `material_surfaces`, `mesh`, `player` (only `Input` yet), `settings`,
//! `shapes`, `shot_noise`, `terrain`, `thrown_wormholes`, `voxel_clearance`, `voxel_world` and
//! `zone_walls`. The rest of step A4 of `REFACTOR_PLAN.md` moves `net::protocol`,
//! `net::connection` and `bot` here, and later phases add `rules` (phase D) and `space` (phase E).
//!
//! Example: `use warped_core::map::{Map, get_map_folder};`

pub mod app_bundle;
pub mod avatar_model;
pub mod black_hole_growth;
pub mod fps_demo_assets;
pub mod geometry;
pub mod map;
pub mod material_surfaces;
pub mod mesh;
pub mod player;
pub mod settings;
pub mod shapes;
pub mod shot_noise;
pub mod terrain;
pub mod thrown_wormholes;
pub mod voxel_clearance;
pub mod voxel_world;
pub mod zone_walls;
