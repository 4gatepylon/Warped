//! `warped_server` is the game server's library, which the `warped-server` binary and its tests use:
//! the game itself, written sans-IO (`game`).
//!
//! Interface: `game`.
//!
//! Example: `let mut game = warped_server::game::Game::New(map_name, settings, console_teams, password, None);`
//!
//! Must know:
//! - Nothing outside this package depends on it (`REFACTOR_PLAN.md`, section 3): the window talks
//!   to a server only over the network.

pub mod game;
