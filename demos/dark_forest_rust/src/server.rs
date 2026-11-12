//! `server` runs one Dark Forest game server: it reads the map's consoles and serves the
//! protocol of `net.rs` until stopped. The windowed game (`--serve`, or in a thread when it
//! hosts) and the headless `dark-forest-server` share it.
//!
//! Example: `server::run(&ServerOptions { port: net::DEFAULT_PORT, ..options }, &should_stop)?;`
//!
//! Interface: `ServerOptions`, `run`.

use std::error::Error;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::console_reach::{self, ConsolePlace};
use crate::map::{self, MapFile};
use crate::net;
use crate::settings::ServerSettings;

/// `NEW_GAME_AFTER_WIN_S` is how long after a win a new game starts, unless the server writes a
/// level result (then the game server decides what comes next).
const NEW_GAME_AFTER_WIN_S: u64 = 10;

/// `ServerOptions` is what a server needs to start: its port, map, where it writes a win, the
/// game password, and its settings file (`None` for the built-in `server_settings.yaml`).
pub struct ServerOptions {
    pub port: u16,
    pub map_path: PathBuf,
    pub level_result_file: Option<PathBuf>,
    pub game_password: Vec<u8>,
    pub server_settings: Option<PathBuf>,
}

/// `run` binds the port and serves until `should_stop` is set or an error happens.
///
/// Example: `server::run(&options, &AtomicBool::new(false))?;`
pub fn run(options: &ServerOptions, should_stop: &AtomicBool) -> Result<(), Box<dyn Error>> {
    let settings: ServerSettings = match &options.server_settings {
        Some(path) => ServerSettings::Load(path)?,
        None => ServerSettings::Builtin(),
    };
    let map: MapFile = map::load(&options.map_path)?;
    let consoles: Vec<ConsolePlace> = console_reach::find_console_places(&map);
    let socket: std::net::UdpSocket = net::bind_server(options.port)?;
    println!(
        "Dark Forest game server for {} on UDP port {} ({} consoles).",
        map.name,
        options.port,
        consoles.len()
    );
    let new_game_after_win: Option<Duration> = match options.level_result_file {
        Some(_) => None,
        None => Some(Duration::from_secs(NEW_GAME_AFTER_WIN_S)),
    };
    net::serve(
        socket,
        should_stop,
        map.name.clone(),
        settings,
        consoles,
        &AtomicBool::new(false),
        new_game_after_win,
        options.level_result_file.as_deref(),
        options.game_password.clone(),
    )
}
