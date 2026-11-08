//! `udp_loop_serves_windows.rs` runs `udp_loop::serve` with a `Game` on loopback and joins it with
//! the window's own `Client` (`warped_core::net`), so the game, its UDP loop and the window's side
//! of the protocol are tested together over real sockets.
//!
//! Partitions and the tests that cover each part:
//!
//! | Partitioning | Parts | Tests |
//! | --- | --- | --- |
//! | Client's password | the server's (welcomed), another (fails, naming the password) | `clients_join_with_the_password_only` |
//! | Client's map | the server's (welcomed), another (fails, naming both maps) | `clients_join_with_the_password_only`, `clients_of_another_map_fail` |
//! | Players already in the game | fewer than `MAX_PLAYER_COUNT` (welcomed), `MAX_PLAYER_COUNT` (full, and the client says so) | `full_games_are_noticed` |
//! | What a joined client hears | the scores, the damage enemy shots did to its wormholes | `clients_receive_scores`, `owners_learn_their_wormholes_are_destroyed` |

use std::error::Error;
use std::net::SocketAddr;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use glam::DMat4;
use warped_core::net::{Client, MAX_PLAYER_COUNT, NetEvent, PasswordRefused, Score};
use warped_core::settings::ServerSettings;
use warped_core::thrown_wormholes::{MouthPlace, PlayerWormholes};
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
    let (should_stop, should_start_next_game) = (AtomicBool::new(false), AtomicBool::new(false));
    std::thread::scope(|scope| {
        scope.spawn(|| {
            udp_loop::serve(socket, &mut game, &should_stop, &should_start_next_game)
                .expect("a server")
        });
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

#[test]
fn clients_of_another_map_fail() {
    with_server(|server| {
        let mut client: Client = connect(server, "house_demo", PASSWORD);
        let error: String = poll_until(&mut client, |c| c.id.is_some())
            .err()
            .expect("a failure")
            .to_string();
        assert!(
            error.contains(MAP_NAME) && error.contains("house_demo"),
            "{error}"
        );
    });
}

#[test]
fn full_games_are_noticed() {
    with_server(|server| {
        let mut players: Vec<Client> = Vec::new();
        for _ in 0..MAX_PLAYER_COUNT {
            players.push(connect(server, MAP_NAME, PASSWORD));
            poll_until(players.last_mut().expect("a player"), |c| c.id.is_some()).expect("a join");
        }
        let mut late: Client = connect(server, MAP_NAME, PASSWORD);
        poll_until(&mut late, Client::is_full).expect("a full game");
        assert!(late.is_waiting_for_welcome());
    });
}

#[test]
fn clients_receive_scores() {
    with_server(|server| {
        let mut client: Client = connect(server, MAP_NAME, PASSWORD);
        poll_until(&mut client, |c| c.scores.is_some()).expect("the welcome and the scores");
        assert_eq!(client.welcomed_id, Some(1));
        let only_score: Score = Score {
            id: 1,
            team: 0,
            kills: 0,
            deaths: 0,
        };
        assert_eq!(client.scores, Some(vec![only_score]));
    });
}

#[test]
fn owners_learn_their_wormholes_are_destroyed() {
    let hits_to_destroy: u8 = ServerSettings::Builtin().wormholes.sniper_hits_to_destroy;
    with_server(|server| {
        let (mut owner, mut enemy) = (
            connect(server, MAP_NAME, PASSWORD),
            connect(server, MAP_NAME, PASSWORD),
        );
        poll_until(&mut owner, |c| c.id.is_some()).expect("the owner's welcome");
        poll_until(&mut enemy, |c| c.id.is_some()).expect("the enemy's welcome");
        let place: MouthPlace = MouthPlace {
            u_m: 0.0,
            v_m: 0.0,
            h_m: 1.6,
            yaw_rad: 0.0,
        };
        let wormholes: PlayerWormholes = PlayerWormholes {
            green: Some(place),
            ..PlayerWormholes::default()
        };
        owner
            .send_state(DMat4::IDENTITY, false, false, None, 0.0, wormholes)
            .expect("a state");
        let has_wormholes = |c: &Client| {
            c.players
                .iter()
                .any(|p| !p.wormholes.get_places().is_empty())
        };
        poll_until(&mut enemy, has_wormholes).expect("the owner's wormholes");
        for _ in 0..hits_to_destroy {
            enemy
                .send_wormhole_hit(1, hits_to_destroy)
                .expect("a wormhole hit");
        }
        poll_until(&mut owner, |c| c.wormhole_damage_frac >= 1.0).expect("the damage");
    });
}
