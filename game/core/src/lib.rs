//! `warped-core` is everything the game window (`warped`) and the game server (`warped-server`)
//! share: maps, the world and walking in it, lasers, thrown wormholes, settings, the network's
//! messages and the window's side of a connection, and bots for tests. It has no window, GPU or
//! Apple dependency, so it and the server build on Linux.
//!
//! Interface: so far `app_bundle`, `fps_demo_assets`, `geometry`, `map`, `mesh`, `shapes` and
//! `terrain`. The rest of step A4 of `REFACTOR_PLAN.md` moves the other modules of its section 4
//! here: `zone_walls`, `voxel_world`, `player`, `thrown_wormholes`, `shot_noise`,
//! `black_hole_growth`, `settings`, `net::protocol`, `net::connection`, `bot`, and later `rules`
//! (phase D) and `space` (phase E).
//!
//! Example: `use warped_core::map::{Map, get_map_folder};`

pub mod app_bundle;
pub mod fps_demo_assets;
pub mod geometry;
pub mod map;
pub mod mesh;
pub mod settings;
pub mod shapes;
pub mod terrain;
