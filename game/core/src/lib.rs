//! `warped-core` is everything the game window (`warped`) and the game server (`warped-server`)
//! share: maps, the world and walking in it, lasers, thrown wormholes, settings, the network's
//! messages and the window's side of a connection, and bots for tests. It has no window, GPU or
//! Apple dependency, so it and the server build on Linux.
//!
//! Interface: empty until step A4 of `REFACTOR_PLAN.md` moves the modules of its section 4 here:
//! `geometry`, `map`, `terrain`, `shapes`, `mesh`, `zone_walls`, `voxel_world`, `player`,
//! `thrown_wormholes`, `shot_noise`, `black_hole_growth`, `settings`, `net::protocol`,
//! `net::connection`, `bot`, and later `rules` (phase D) and `space` (phase E).
//!
//! Example: `use warped_core::voxel_world::VoxelWorld;` (after step A4)
