//! `warped-server` runs one Warped game server without a window, for a machine that only hosts
//! games, such as a Linux server in the cloud, or beside the windows on a local network
//! (`make serve-game`): it builds without Metal or SDL, plays a map built into the program, prints
//! how to join it, and starts a new game a few seconds after a team wins, or at once when `Enter`
//! is pressed in its terminal.
//!
//! Interface: `--port <port>`, `--map <house-demo|lunar-surface>`, `--server-settings <path>`,
//! `--game-password-file <path>`.
//!
//! Example: `.build/x86_64-unknown-linux-musl/release/warped-server --port 47810 --map lunar-surface`
//!
//! Must know:
//! - It plays `warped_server::game` through `warped_server::udp_loop`, and the window joins with
//!   `warped_core::net`, so both speak the same protocol.
//! - It exits on the first error, so a supervisor such as systemd can restart it.

use warped_core::{map, net, settings};
use warped_server::game::Game;
use warped_server::udp_loop;

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, ValueEnum};

use map::{Map, MapName, get_console_team};
use settings::ServerSettings;

const NEW_GAME_AFTER_WIN_S: f64 = 10.0;
const ROUTE_PROBE_ADDRESS: &str = "192.0.2.1:9";

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Run one Warped game server, without a window, until stopped.",
    after_help = "Example: warped-server --port 47810 --map lunar-surface"
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
    let socket: std::net::UdpSocket = udp_loop::bind_server(arguments.port)?;
    println!("Game server for {map_name} on UDP port {}.", arguments.port);
    print_how_to_join(&arguments, !console_teams.is_empty());
    let should_start_next_game: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let flag: Arc<AtomicBool> = Arc::clone(&should_start_next_game);
    std::thread::spawn(move || {
        for _ in std::io::stdin().lines().map_while(std::io::Result::ok) {
            flag.store(true, Ordering::Relaxed);
        }
    });
    let mut game: Game = Game::New(
        map_name.to_string(),
        settings,
        console_teams,
        game_password,
        Some(NEW_GAME_AFTER_WIN_S),
    );
    let should_stop: AtomicBool = AtomicBool::new(false);
    udp_loop::serve(socket, &mut game, &should_stop, &should_start_next_game)
}

/// `print_how_to_join` prints the server settings players get, this computer's address, the
/// commands that join this server from `game/`, and, on a map with consoles (`has_consoles`), how
/// the next game starts.
///
/// Example: `print_how_to_join(&arguments, true);`
fn print_how_to_join(arguments: &Arguments, has_consoles: bool) {
    let settings_name: String = arguments
        .server_settings
        .as_ref()
        .map_or("the built-in server_settings.yaml".to_string(), |path| {
            path.display().to_string()
        });
    println!("Server settings: {settings_name} (players who join get them).");
    let address: String = match find_lan_address() {
        Some(address) => address.to_string(),
        None => {
            eprintln!("Couldn't find this computer's address; `ipconfig getifaddr en0` gives it");
            "<this computer's address>".to_string()
        }
    };
    let map: String = arguments
        .map
        .to_possible_value()
        .expect("every MapName has a --map value")
        .get_name()
        .to_string();
    let port: u16 = arguments.port;
    let password: String = arguments
        .game_password_file
        .as_ref()
        .map_or(String::new(), |path| {
            format!(" GAME_PASSWORD_FILE={}", path.display())
        });
    println!("This computer's address: {address}. Join from game/ with one of:");
    println!("  make join-game MAP={map} SERVER={address} PORT={port}{password}");
    println!(
        "  make join-game MAP={map} PORT={port}{password}  (finds this server on the network by itself)"
    );
    println!(
        "  make join-game MAP={map} SERVER=127.0.0.1 PORT={port}{password}  (on this computer)"
    );
    if has_consoles {
        println!(
            "A team wins by turning off the other team's consoles; the next game starts {NEW_GAME_AFTER_WIN_S} s later, or at once when you press Enter here."
        );
    }
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
