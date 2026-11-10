//! `network_scope.rs` decides whether a program's sockets reach the local network or stay on
//! this computer (loopback, 127.0.0.1), so that tests never make macOS ask for Local Network or
//! firewall permission while real players still find and join games across computers.
//!
//! Interface: `LOOPBACK_ONLY_VARIABLE`, `is_loopback_only`, `get_listen_ip`, `get_beacon_ip`,
//! `bind_socket_toward`.
//!
//! Example: `let socket = UdpSocket::bind((network_scope::get_listen_ip(network_scope::is_loopback_only()), port))?;`
//!
//! Must know:
//! - `WARPED_LOOPBACK_ONLY=1` in the environment makes a program and every program it starts
//!   (level servers, bots) listen on 127.0.0.1 and send its beacons to 127.0.0.1 instead of
//!   broadcasting them, so discovery finds only servers on this computer. A crate's tests are
//!   loopback-only without it (`cfg!(test)` in the crate's `net.rs`).

use std::io;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};

pub const LOOPBACK_ONLY_VARIABLE: &str = "WARPED_LOOPBACK_ONLY";

/// `parse_loopback_only` reads `LOOPBACK_ONLY_VARIABLE`'s `value`: unset or `0` is the local
/// network, `1` is loopback only, and anything else stops the program.
///
/// Example: `let is_loopback_only = parse_loopback_only(Some("1"));`
fn parse_loopback_only(value: Option<&str>) -> bool {
    match value {
        None | Some("0") => false,
        Some("1") => true,
        Some(other) => panic!("{LOOPBACK_ONLY_VARIABLE} must be 0 or 1, got {other:?}"),
    }
}

/// `is_loopback_only` says whether `WARPED_LOOPBACK_ONLY=1` keeps this program on loopback.
///
/// Example: `if network_scope::is_loopback_only() { ... }`
pub fn is_loopback_only() -> bool {
    parse_loopback_only(std::env::var(LOOPBACK_ONLY_VARIABLE).ok().as_deref())
}

/// `get_listen_ip` gives the address a server or discovery listener binds: 127.0.0.1 when
/// `is_loopback_only`, else every interface (0.0.0.0).
///
/// Example: `let socket = UdpSocket::bind((get_listen_ip(true), 47800))?;`
pub fn get_listen_ip(is_loopback_only: bool) -> Ipv4Addr {
    match is_loopback_only {
        true => Ipv4Addr::LOCALHOST,
        false => Ipv4Addr::UNSPECIFIED,
    }
}

/// `get_beacon_ip` gives where a server sends its beacons: 127.0.0.1 when `is_loopback_only`,
/// else the local network's broadcast address.
///
/// Example: `socket.send_to(&beacon, (get_beacon_ip(false), DISCOVERY_PORT))?;`
pub fn get_beacon_ip(is_loopback_only: bool) -> Ipv4Addr {
    match is_loopback_only {
        true => Ipv4Addr::LOCALHOST,
        false => Ipv4Addr::BROADCAST,
    }
}

/// `bind_socket_toward` opens a socket on a free port for talking to `peer_ip`: on 127.0.0.1
/// when `peer_ip` is loopback (so nothing reaches the network), else on every interface, allowed
/// to broadcast only when `peer_ip` is the broadcast address.
///
/// Example: `let socket = bind_socket_toward(server.ip())?;`
pub fn bind_socket_toward(peer_ip: IpAddr) -> io::Result<UdpSocket> {
    if peer_ip.is_loopback() {
        return UdpSocket::bind((Ipv4Addr::LOCALHOST, 0));
    }
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_broadcast(peer_ip == IpAddr::V4(Ipv4Addr::BROADCAST))?;
    Ok(socket)
}

#[cfg(test)]
mod tests {
    //! Unit tests for `network_scope.rs`. No test opens a socket on the local network, so none
    //! makes macOS ask for permission.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Variable | unset, `0`, `1` | `the_variable_turns_loopback_only_on` |
    //! | Bad variable | `yes` | `a_bad_variable_stops_the_program` |
    //! | Addresses | loopback only, local network | `loopback_only_listens_and_beacons_on_loopback` |
    //! | Socket toward a peer | a loopback peer (127.0.0.1, its beacon target when loopback only) | `a_socket_toward_loopback_stays_on_loopback` |

    use super::*;

    #[test]
    fn the_variable_turns_loopback_only_on() {
        assert!(!parse_loopback_only(None));
        assert!(!parse_loopback_only(Some("0")));
        assert!(parse_loopback_only(Some("1")));
    }

    #[test]
    #[should_panic(expected = "WARPED_LOOPBACK_ONLY must be 0 or 1")]
    fn a_bad_variable_stops_the_program() {
        parse_loopback_only(Some("yes"));
    }

    #[test]
    fn loopback_only_listens_and_beacons_on_loopback() {
        assert_eq!(get_listen_ip(true), Ipv4Addr::LOCALHOST);
        assert_eq!(get_beacon_ip(true), Ipv4Addr::LOCALHOST);
        assert_eq!(get_listen_ip(false), Ipv4Addr::UNSPECIFIED);
        assert_eq!(get_beacon_ip(false), Ipv4Addr::BROADCAST);
    }

    #[test]
    fn a_socket_toward_loopback_stays_on_loopback() {
        let socket: UdpSocket = bind_socket_toward(IpAddr::V4(get_beacon_ip(true))).unwrap();
        assert_eq!(socket.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
        assert!(!socket.broadcast().unwrap());
    }
}
