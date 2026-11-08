//! `warped-core` is everything the game window (`warped`) and the game server (`warped-server`)
//! share: maps, the world and walking in it, lasers, thrown wormholes, settings, the network's
//! messages and the window's side of a connection, and bots for tests. It has no window, GPU or
//! Apple dependency, so it and the server build on Linux.
//!
//! Interface: so far `app_bundle`, `fps_demo_assets`, `geometry`, `map`, `mesh`, `player` (only
//! `Input` yet), `settings`, `shapes`, `shot_noise`, `terrain`, `thrown_wormholes` and
//! `zone_walls`. The rest of step A4 of `REFACTOR_PLAN.md` moves the other modules of its section 4
//! here: `voxel_world`, the rest of `player`, `black_hole_growth`, `net::protocol`,
//! `net::connection`, `bot`, and later `rules` (phase D) and `space` (phase E).
//!
//! Example: `use warped_core::map::{Map, get_map_folder};`

pub mod app_bundle;
pub mod fps_demo_assets;
pub mod geometry;
pub mod map;
pub mod mesh;
pub mod player;
pub mod settings;
pub mod shapes;
pub mod shot_noise;
pub mod terrain;
pub mod thrown_wormholes;
pub mod zone_walls;
