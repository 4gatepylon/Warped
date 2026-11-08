//! `wormholes_server.rs` runs one game server of the Rust demo without a window, for a machine that
//! only hosts games, such as a Linux server in the cloud: it builds without Metal or SDL, plays a
//! map built into the program, and starts a new game a few seconds after a team wins.
//!
//! Interface: `--port <port>`, `--map <house-demo|lunar-surface>`, `--server-settings <path>`,
//! `--game-password-file <path>`.
//!
//! Example: `.build/x86_64-unknown-linux-musl/release/wormholes_server --port 47810 --map lunar-surface`
//!
//! Must know:
//! - It shares `net.rs` and the modules it needs (`map.rs`, `settings.rs`, `thrown_wormholes.rs`,
//!   `shot_noise.rs`, `terrain.rs`, `geometry.rs`, `app_bundle.rs`) with the game, so both speak
//!   the same protocol.
//! - It exits on the first error, so a supervisor such as systemd can restart it.

#[allow(dead_code)]
#[path = "../app_bundle.rs"]
mod app_bundle;
#[allow(dead_code)]
#[path = "../geometry.rs"]
mod geometry;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../key_bindings.rs"]
mod key_bindings;
#[path = "../map.rs"]
mod map;
#[allow(dead_code)]
#[path = "../net.rs"]
mod net;
#[allow(dead_code)]
#[path = "../settings.rs"]
mod settings;
#[allow(dead_code)]
#[path = "../shot_noise.rs"]
mod shot_noise;
#[allow(dead_code)]
#[path = "../terrain.rs"]
mod terrain;
#[allow(dead_code)]
#[path = "../thrown_wormholes.rs"]
mod thrown_wormholes;

use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use clap::Parser;

use map::{Map, MapName, get_console_team};
use settings::ServerSettings;

const NEW_GAME_AFTER_WIN_S: u64 = 10;

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Run one game server of the Rust wormholes demo, without a window, until stopped.",
    after_help = "Example: wormholes_server --port 47810 --map lunar-surface"
)]
struct Arguments {
    /// UDP port the server listens on.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    port: u16,
    /// Which built-in map the game is on; players must join with the same map.
    #[arg(long, value_enum)]
    map: MapName,
    /// A copy of server_settings.yaml with the numbers that shape play (default: the built-in one).
    #[arg(long)]
    server_settings: Option<PathBuf>,
    /// A file holding the game's password; only players who prove they know it may join (default:
    /// no password, so anyone may).
    #[arg(long)]
    game_password_file: Option<PathBuf>,
}

/// `main` serves one game until the process is stopped, failing at once on a bad map, settings
/// file or port.
///
/// Example: `main()`
fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Arguments = Arguments::parse();
    let map_name: &str = arguments.map.get_folder_name();
    let console_teams: Vec<u32> = Map::Builtin(map_name)?
        .materials
        .iter()
        .filter_map(get_console_team)
        .collect();
    let game_password: Vec<u8> = net::read_game_password(arguments.game_password_file.as_deref())?;
    let settings: ServerSettings = match &arguments.server_settings {
        Some(path) => ServerSettings::Load(path)?,
        None => ServerSettings::Builtin(),
    };
    let socket: std::net::UdpSocket = net::bind_server(arguments.port)?;
    println!("Game server for {map_name} on UDP port {}.", arguments.port);
    net::serve(
        socket,
        &AtomicBool::new(false),
        map_name.to_string(),
        settings,
        console_teams,
        &AtomicBool::new(false),
        Some(Duration::from_secs(NEW_GAME_AFTER_WIN_S)),
        game_password,
        None,
    )
}
