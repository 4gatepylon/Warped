//! `warped` is the Warped game window: the title screen, joining a game server, and playing in
//! curved space (REFACTOR_PLAN.md, section 5).
//!
//! Interface: none yet; step A1 copies the demo's `main.rs` and modules here, with its flags
//! (`--join`, `--port`, `--player-settings`, `--game-password-file`, `--bench-views`, `--bot`, ...).
//!
//! Example: `cargo run --release -p warped-client`
//!
//! Must know:
//! - Limitations: until step A1 it only says where to play instead, and exits with an error.

use std::process::ExitCode;

/// `main` says that the game isn't in `game/` yet and where to play it meanwhile.
///
/// Example: `warped` prints the demo's commands and exits with code 1.
fn main() -> ExitCode {
    eprintln!(
        "warped: the game isn't in game/ yet (game/REFACTOR_PLAN.md, step A1); play the demo meanwhile: make -C demos/hyperbolic_wormholes_rust join-game SERVER=<address>"
    );
    ExitCode::FAILURE
}
