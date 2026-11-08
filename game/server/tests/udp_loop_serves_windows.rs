//! `udp_loop_serves_windows.rs` runs `udp_loop::serve` with a `Game` on loopback and joins it with
//! the window's own `Client` (`warped_core::net`), so the game, its UDP loop and the window's side
//! of the protocol are tested together over real sockets.
//!
//! Partitions and the tests that cover each part:
//!
//! | Partitioning | Parts | Tests |
//! | --- | --- | --- |
//! | Client's password | the server's (welcomed), another (fails, naming the password) | `clients_join_with_the_password_only` |

use std::error::Error;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use warped_core::net::{Client, NetEvent, PasswordRefused};
use warped_core::settings::ServerSettings;
use warped_server::game::Game;
use warped_server::udp_loop;

const MAP_NAME: &str = "lunar_surface";
const PASSWORD: &[u8] = b"correct horse";
const TIMEOUT_S: f64 = 5.0;
const POLL_MS: u64 = 5;

/// `with_server` serves a game of `MAP_NAME` that admits players who know `PASSWORD`, on a free
/// loopback port, while `test` runs with its address, then stops it.
///
/// Example: `with_server(|server| { let client = connect(server, MAP_NAME, PASSWORD); });`
fn with_server(test: impl FnOnce(SocketAddr)) {
    let socket: std::net::UdpSocket = udp_loop::bind_server(0).expect("a free UDP port");
    let port: u16 = socket.local_addr().expect("a bound socket").port();
    let mut game: Game = Game::New(
        MAP_NAME.to_string(),
        ServerSettings::Builtin(),
        vec![0, 0, 1],
        PASSWORD.to_vec(),
        None,
    );
    let should_stop: AtomicBool = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| udp_loop::serve(socket, &mut game, &should_stop).expect("a server"));
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            test(SocketAddr::from(([127, 0, 0, 1], port)))
        }));
        should_stop.store(true, Ordering::Relaxed);
        result.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    });
}

/// `connect` connects a client of `map_name` that knows `password` to `server` and asks to join
/// any team.
///
/// Example: `let mut client = connect(server, MAP_NAME, PASSWORD);`
fn connect(server: SocketAddr, map_name: &str, password: &[u8]) -> Client {
    let mut client: Client = Client::Connect(server, map_name, password).expect("a socket");
    client.join(None).expect("a join request");
    client
}

/// `poll_until` polls `client` until `is_done` holds, giving the events it read, or its first
/// error; it fails after `TIMEOUT_S`.
///
/// Example: `poll_until(&mut client, |c| c.id.is_some())?;`
fn poll_until(
    client: &mut Client,
    is_done: impl Fn(&Client) -> bool,
) -> Result<Vec<NetEvent>, Box<dyn Error>> {
    let start: Instant = Instant::now();
    let mut events: Vec<NetEvent> = Vec::new();
    while !is_done(client) {
        assert!(start.elapsed().as_secs_f64() < TIMEOUT_S, "no answer");
        events.extend(client.poll()?);
        std::thread::sleep(Duration::from_millis(POLL_MS));
    }
    Ok(events)
}

#[test]
fn clients_join_with_the_password_only() {
    with_server(|server| {
        let mut player: Client = connect(server, MAP_NAME, PASSWORD);
        let events: Vec<NetEvent> = poll_until(&mut player, |c| c.id.is_some()).expect("a welcome");
        assert!(matches!(events[..], [NetEvent::Joined(1)]));
        let mut stranger: Client = connect(server, MAP_NAME, b"wrong horse");
        let error: Box<dyn Error> = poll_until(&mut stranger, |c| c.id.is_some())
            .err()
            .expect("a refusal");
        assert!(error.downcast_ref::<PasswordRefused>().is_some(), "{error}");
    });
}
