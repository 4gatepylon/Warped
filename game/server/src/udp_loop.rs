//! `udp_loop.rs` runs a `Game` on a UDP socket: it gives the game each datagram that arrives and
//! the time, sends what the game gives back, and announces the server on the local network so
//! windows find it with `--join auto`.
//!
//! Interface: `bind_server`, `serve`.
//!
//! Example: `udp_loop::serve(udp_loop::bind_server(47800)?, &mut game, &AtomicBool::new(false))?;`
//!
//! Must know:
//! - A failed send is logged, not fatal, so one unreachable player can't end the game for everyone.
//! - It waits at most `SERVER_POLL_MS` for a datagram, so the game moves on at least that often.

use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use warped_core::net::{BEACON_MAGIC, DISCOVERY_PORT, Kind, MAX_DATAGRAM_BYTES};

use crate::game::Game;

const SERVER_POLL_MS: u64 = 2;
const BEACON_PERIOD_S: f64 = 0.5;

/// `bind_server` opens the server's socket on `port`, so a port already in use fails at once.
///
/// Example: `let socket = udp_loop::bind_server(net::DEFAULT_PORT)?;`
pub fn bind_server(port: u16) -> Result<UdpSocket, Box<dyn Error>> {
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|error| {
        format!("net: can't listen on UDP port {port} ({error}); pick another with --port")
    })?;
    socket.set_read_timeout(Some(Duration::from_millis(SERVER_POLL_MS)))?;
    Ok(socket)
}

/// `serve` runs `game` on `socket` (from `bind_server`) until `should_stop` is set: it gives the
/// game every datagram (`Game::handle`) and, after each wait for one, the time (`Game::tick`), in
/// seconds since `serve` started; sends what the game gives back; and broadcasts a beacon every
/// `BEACON_PERIOD_S` so `net::find_server` can find it.
///
/// Example: `udp_loop::serve(socket, &mut game, &should_stop)?;`
pub fn serve(
    socket: UdpSocket,
    game: &mut Game,
    should_stop: &AtomicBool,
) -> Result<(), Box<dyn Error>> {
    let start: Instant = Instant::now();
    let beacon_socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    beacon_socket.set_broadcast(true)?;
    let mut beacon: Vec<u8> = vec![Kind::Beacon as u8];
    beacon.extend_from_slice(BEACON_MAGIC);
    beacon.extend_from_slice(&socket.local_addr()?.port().to_le_bytes());
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    let mut last_beacon_s: Option<f64> = None;
    let mut has_warned_about_beacon: bool = false;
    while !should_stop.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buffer) {
            Ok((length, from)) => {
                let now_s: f64 = start.elapsed().as_secs_f64();
                send_all(&socket, game.handle(&buffer[..length], from, now_s));
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => return Err(error.into()),
        }
        let now_s: f64 = start.elapsed().as_secs_f64();
        send_all(&socket, game.tick(now_s));
        if last_beacon_s.is_none_or(|time_s| now_s - time_s >= BEACON_PERIOD_S) {
            if let Err(error) =
                beacon_socket.send_to(&beacon, (Ipv4Addr::BROADCAST, DISCOVERY_PORT))
                && !has_warned_about_beacon
            {
                has_warned_about_beacon = true;
                eprintln!(
                    "net: can't announce this server on the network ({error}), so --join auto won't find it; on macOS, allow Local Network access for this app, or join with --join <this computer's IP address>"
                );
            }
            last_beacon_s = Some(now_s);
        }
    }
    Ok(())
}

/// `send_all` sends each datagram to its address on `socket`, logging a failure rather than
/// stopping the server.
///
/// Example: `send_all(&socket, game.tick(now_s));`
fn send_all(socket: &UdpSocket, datagrams: Vec<(SocketAddr, Vec<u8>)>) {
    for (to, datagram) in datagrams {
        if let Err(error) = socket.send_to(&datagram, to) {
            eprintln!("net: can't send to {to}: {error}");
        }
    }
}
