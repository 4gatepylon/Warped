//! `warped-server` runs one Warped game on a UDP port, without a window: it lets in players who
//! know the game's password, relays their state, keeps score and runs the consoles
//! (REFACTOR_PLAN.md, section 5).
//!
//! Interface: none yet; step A5 builds it from the demo's server half of `net.rs` and the infra
//! stack's `wormholes_server`, with the flags `--port`, `--map`, `--server-settings` and
//! `--game-password-file`.
//!
//! Example: `cargo run --release -p warped-server`
//!
//! Must know:
//! - Limitations: until step A5 it only says where to run a server instead, and exits with an
//!   error.

use std::process::ExitCode;

/// `main` says that the game server isn't in `game/` yet and where to run one meanwhile.
///
/// Example: `warped-server` prints the demo's command and exits with code 1.
fn main() -> ExitCode {
    eprintln!(
        "warped-server: the game server isn't in game/ yet (game/REFACTOR_PLAN.md, step A5); run the demo's meanwhile: make -C demos/hyperbolic_wormholes_rust serve-game"
    );
    ExitCode::FAILURE
}
