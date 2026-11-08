//! `server_process.rs` loads a map and its server settings, and finds the server a window joins
//! (`warped-server` runs servers).
//!
//! Interface: `load_map`, `load_settings`, `get_server_address`, `parse_server_address`.
//!
//! Example: `let address = get_server_address(&arguments)?;`

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use crate::map::MapName;
use crate::settings::ServerSettings;

use crate::cli::Arguments;
use crate::{JOIN_AUTO, Result, map, net};

const DISCOVERY_TIMEOUT_S: u64 = 5;

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

/// `get_server_address` gives the server `--join` names: found on the network for `--join auto`,
/// or the given address.
///
/// Example: `let address = get_server_address(&arguments)?;`
pub(crate) fn get_server_address(arguments: &Arguments) -> Result<SocketAddr> {
    match arguments
        .join
        .as_deref()
        .expect("get_server_address: no --join")
    {
        JOIN_AUTO => net::find_server(Duration::from_secs(DISCOVERY_TIMEOUT_S)),
        text => {
            Ok(parse_server_address(text, arguments.port)
                .expect("validate_arguments checked --join"))
        }
    }
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
