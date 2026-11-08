//! `server_process.rs` loads a map and its server settings, finds the server a window joins, and
//! starts a game server: in this process (`--serve`, `serve_only`) or as a child process beside a
//! window.
//!
//! Interface: `load_map`, `load_settings`, `get_server_address`, `parse_server_address`,
//! `start_server_process`, `get_map_flag`, `serve_only`.
//!
//! Example: `serve_only(&arguments)?;`

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use clap::ValueEnum;

use crate::map::MapName;
use crate::settings::ServerSettings;

use crate::cli::Arguments;
use crate::{JOIN_AUTO, Result, map, net, voxel_world};

const DISCOVERY_TIMEOUT_S: u64 = 5;
const SERVER_START_S: f64 = 0.3;
const ROUTE_PROBE_ADDRESS: &str = "192.0.2.1:9";

/// `load_map` loads a map built into the program (`Map::Builtin`) and, on a voxel map (one with
/// structures, such as Lunar Surface), curves its space as `settings` say (`Map::scale_curvature`).
/// The house demo keeps its own curvature: at twice it, its far pylon sits 13 curvature radii out,
/// too far for the renderer's single precision, and its scripted route breaks.
///
/// Example: `let map = load_map(MapName::LunarSurface, &ServerSettings::Builtin())?;`
pub(crate) fn load_map(name: MapName, settings: &ServerSettings) -> Result<map::Map> {
    let mut map: map::Map = map::Map::Builtin(name.get_folder_name())?;
    if !map.structures.is_empty() {
        map.scale_curvature(settings.space.curvature_per_map_curvature)?;
    }
    Ok(map)
}

/// `parse_server_address` reads `--join`'s address: an IP address with an optional port (`port`
/// when it has none).
///
/// Example: `let address = parse_server_address("192.168.1.20", 47800);`
pub(crate) fn parse_server_address(text: &str, port: u16) -> Option<SocketAddr> {
    text.parse::<SocketAddr>().ok().or_else(|| {
        text.parse::<IpAddr>()
            .ok()
            .map(|ip| SocketAddr::new(ip, port))
    })
}

/// `get_server_address` gives the server to join: found on the network for `--join auto`, the
/// given address, or this computer for `--host`.
///
/// Example: `let address = get_server_address(&arguments)?;`
pub(crate) fn get_server_address(arguments: &Arguments) -> Result<SocketAddr> {
    match arguments.join.as_deref() {
        Some(JOIN_AUTO) => net::find_server(Duration::from_secs(DISCOVERY_TIMEOUT_S)),
        Some(text) => {
            Ok(parse_server_address(text, arguments.port)
                .expect("validate_arguments checked --join"))
        }
        None => Ok(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            arguments.port,
        )),
    }
}

/// `start_server` binds the server's port and runs the server for a map with `console_teams` on a
/// thread until the returned flag is set, starting a new game after a win at each line read from
/// standard input (`Enter` in the server's terminal).
///
/// Example: `let (should_stop, thread) = start_server(47800, "lunar_surface", ServerSettings::Builtin(), vec![0, 1])?;`
fn start_server(
    port: u16,
    map_name: &str,
    settings: ServerSettings,
    console_teams: Vec<u32>,
    game_password: Vec<u8>,
) -> Result<(Arc<AtomicBool>, JoinHandle<()>)> {
    let socket: std::net::UdpSocket = net::bind_server(port)?;
    let should_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let flag: Arc<AtomicBool> = Arc::clone(&should_stop);
    let should_start_new_game: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let map_name: String = map_name.to_string();
    let new_game_flag: Arc<AtomicBool> = Arc::clone(&should_start_new_game);
    std::thread::spawn(move || {
        for _ in std::io::stdin().lines().map_while(std::io::Result::ok) {
            new_game_flag.store(true, Ordering::Relaxed);
        }
    });
    let thread: JoinHandle<()> = std::thread::spawn(move || {
        if let Err(error) = net::serve(
            socket,
            &flag,
            map_name,
            settings,
            console_teams,
            &should_start_new_game,
            None,
            game_password,
        ) {
            eprintln!("The game server stopped: {error}");
        }
    });
    Ok((should_stop, thread))
}

/// `start_server_process` starts this program as a game server (`--serve`, without a window) on
/// `arguments`' port, map and settings, and fails if it stops within `SERVER_START_S` (its port in
/// use, say).
///
/// Example: `let server = start_server_process(&arguments)?;`
pub(crate) fn start_server_process(arguments: &Arguments) -> Result<Child> {
    let map_name: String = get_map_flag(arguments.map);
    let mut command: Command = Command::new(std::env::current_exe()?);
    command.args([
        "--serve",
        "--port",
        &arguments.port.to_string(),
        "--map",
        &map_name,
    ]);
    if let Some(path) = &arguments.server_settings {
        command.arg("--server-settings").arg(path);
    }
    if let Some(path) = &arguments.game_password_file {
        command.arg("--game-password-file").arg(path);
    }
    let mut server: Child = command.spawn()?;
    std::thread::sleep(Duration::from_secs_f64(SERVER_START_S));
    if let Some(status) = server.try_wait()? {
        return Err(
            format!("the game server stopped at once ({status}); see its error above").into(),
        );
    }
    Ok(server)
}

/// `load_settings` gives the server settings `--server-settings` names, or the built-in ones.
///
/// Example: `let settings = load_settings(&arguments)?;`
pub(crate) fn load_settings(arguments: &Arguments) -> Result<ServerSettings> {
    match &arguments.server_settings {
        Some(path) => ServerSettings::Load(path),
        None => Ok(ServerSettings::Builtin()),
    }
}

/// `get_map_flag` gives a map's name as `--map` and `MAP=` take it, e.g. `lunar-surface`.
///
/// Example: `let name = get_map_flag(MapName::LunarSurface);`
pub(crate) fn get_map_flag(map: MapName) -> String {
    map.to_possible_value()
        .expect("every MapName has a --map value")
        .get_name()
        .to_string()
}

/// `find_lan_address` gives this computer's address on its network (the one its default route
/// uses), or `None` without a network. Connecting a UDP socket only picks the route; nothing is
/// sent.
///
/// Example: `let address = find_lan_address();`
fn find_lan_address() -> Option<IpAddr> {
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(ROUTE_PROBE_ADDRESS).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// `serve_only` runs a game server without a window until `--exit-after-s` passes (or forever),
/// after printing this computer's address and the commands that join the game.
///
/// Example: `serve_only(&arguments)?;`
pub(crate) fn serve_only(arguments: &Arguments) -> Result<()> {
    let settings: ServerSettings = load_settings(arguments)?;
    let map_data: map::Map = load_map(arguments.map, &settings)?;
    let consoles: Vec<voxel_world::Console> = voxel_world::find_consoles(&map_data);
    let (should_stop, thread) = start_server(
        arguments.port,
        arguments.map.get_folder_name(),
        settings,
        consoles.iter().map(|c| c.team).collect(),
        net::read_game_password(arguments.game_password_file.as_deref())?,
    )?;
    println!(
        "Server settings: {} (players who join get them).",
        arguments
            .server_settings
            .as_ref()
            .map_or("the built-in server_settings.yaml".to_string(), |path| path
                .display()
                .to_string())
    );
    let (map, port) = (get_map_flag(arguments.map), arguments.port);
    let address: String = match find_lan_address() {
        Some(address) => address.to_string(),
        None => {
            eprintln!("Couldn't find this computer's address; `ipconfig getifaddr en0` gives it");
            "<this computer's address>".to_string()
        }
    };
    println!("Game server for {map} on UDP port {port}, without a window. Stop it with Ctrl-C.");
    println!("This computer's address: {address}. Join from the Rust demo's folder:");
    println!("  make join-game MAP={map} SERVER={address} PORT={port}");
    println!(
        "  make join-game MAP={map} PORT={port}  (finds this server on the network by itself)"
    );
    println!("  make join-game MAP={map} SERVER=127.0.0.1 PORT={port}  (on this computer)");
    if !consoles.is_empty() {
        println!(
            "A team wins by turning off the other team's consoles; then press Enter here for a new game."
        );
    }
    match arguments.exit_after_s {
        Some(seconds) => std::thread::sleep(Duration::from_secs_f64(seconds)),
        None => loop {
            std::thread::park();
        },
    }
    should_stop.store(true, Ordering::Relaxed);
    thread
        .join()
        .map_err(|_| "the game server's thread panicked")?;
    Ok(())
}
