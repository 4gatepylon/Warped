//! `dark-forest-server` runs one Dark Forest game server without a window, until stopped.
//!
//! Example: `dark-forest-server --port 47840 --level-result-file /tmp/result.json`
//!
//! Interface: `--port <port>`, `--map <path to map.json>`, `--level-result-file <path>`,
//! `--game-password-file <path>`.
//!
//! Must know: it shares `net.rs`, `server.rs`, `map.rs` and `settings.rs` with the game, so both
//! speak the same protocol.

#[allow(dead_code)]
#[path = "../map.rs"]
mod map;
#[allow(dead_code)]
#[path = "../net.rs"]
mod net;
#[path = "../server.rs"]
mod server;
#[allow(dead_code)]
#[path = "../settings.rs"]
mod settings;
#[allow(dead_code)]
#[path = "../shot_noise.rs"]
mod shot_noise;
#[allow(dead_code)]
#[path = "../thrown_wormholes.rs"]
mod thrown_wormholes;

use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use clap::Parser;

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Run one Dark Forest game server, without a window, until stopped.",
    after_help = "Example: dark-forest-server --port 47840 --level-result-file /tmp/result.json"
)]
struct Arguments {
    /// UDP port to serve on.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    port: u16,
    /// The map file (default: common/maps/dark_forest/map.json).
    #[arg(long)]
    map: Option<PathBuf>,
    /// Write {"winner": "blue"|"red"} here when a team wins, and start no new game.
    #[arg(long)]
    level_result_file: Option<PathBuf>,
    /// A file holding the game password players must know.
    #[arg(long)]
    game_password_file: Option<PathBuf>,
}

/// `main` serves until killed.
fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Arguments = Arguments::parse();
    server::run(
        &server::ServerOptions {
            port: arguments.port,
            map_path: arguments.map.unwrap_or_else(map::default_map_path),
            level_result_file: arguments.level_result_file,
            game_password: net::read_game_password(arguments.game_password_file.as_deref())?,
        },
        &AtomicBool::new(false),
    )
}
