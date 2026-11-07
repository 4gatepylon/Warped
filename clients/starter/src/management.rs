//! `management.rs` calls the management server's HTTP API
//! (`common/protocol/management_server_api.md`).
//!
//! Interface: `GameServer`, `Player`, `list_game_servers`, `create_game_server`,
//! `get_game_server`, `add_player`, `remove_player`.
//!
//! Example: `let game_servers = list_game_servers("http://127.0.0.1:8000")?;`

use std::sync::LazyLock;
use std::time::Duration;

use serde::Deserialize;

/// `MAX_PLAYER_COUNT` is how many players a new game server takes; big, so that rooms never fill.
const MAX_PLAYER_COUNT: u32 = 1000;
/// `LIST_LIMIT` is how many game servers the lobby shows, one per number key.
const LIST_LIMIT: u32 = 9;
/// `CONNECT_TIMEOUT` is how long a request waits to connect, so a wrong URL fails fast.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// `READ_TIMEOUT` is how long a request waits for an answer once connected.
pub const READ_TIMEOUT: Duration = Duration::from_secs(3);
/// `AGENT` makes every request, with `CONNECT_TIMEOUT` and `READ_TIMEOUT`.
static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .build()
});

/// `Result` is a result whose error any call here may return.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// `Address` is where a running game server takes WebSocket connections.
#[derive(Deserialize, Clone)]
pub struct Address {
    pub host: String,
    pub port: u16,
}

/// `GameServer` is the API's `GameServer`, with the fields the client uses.
#[derive(Deserialize, Clone)]
pub struct GameServer {
    pub game_server_id: String,
    pub state: String,
    pub address: Option<Address>,
    pub player_count: u32,
}

/// `GameServerList` is the API's `GameServerList`, without paging.
#[derive(Deserialize)]
struct GameServerList {
    game_servers: Vec<GameServer>,
}

/// `Player` is the API's `Player`: the secret ID that admits this client to its game server.
#[derive(Deserialize, Clone)]
pub struct Player {
    pub player_id: String,
}

/// `list_game_servers` returns up to nine game servers that have room for another player.
///
/// Example: `let game_servers = list_game_servers(url)?;`
pub fn list_game_servers(url: &str) -> Result<Vec<GameServer>> {
    let list: GameServerList = AGENT
        .get(&format!("{url}/game-servers"))
        .query("has_open_place", "true")
        .query("limit", &LIST_LIMIT.to_string())
        .call()?
        .into_json()?;
    Ok(list.game_servers)
}

/// `create_game_server` asks the management server to start a new game server.
///
/// Example: `let game_server = create_game_server(url)?;`
pub fn create_game_server(url: &str) -> Result<GameServer> {
    Ok(AGENT
        .post(&format!("{url}/game-servers"))
        .send_json(serde_json::json!({ "max_player_count": MAX_PLAYER_COUNT }))?
        .into_json()?)
}

/// `get_game_server` returns a game server's state and, once it runs, its address.
///
/// Example: `let game_server = get_game_server(url, &game_server_id)?;`
pub fn get_game_server(url: &str, game_server_id: &str) -> Result<GameServer> {
    Ok(AGENT
        .get(&format!("{url}/game-servers/{game_server_id}"))
        .call()?
        .into_json()?)
}

/// `add_player` adds a new player to a game server, which then admits it.
///
/// Example: `let player = add_player(url, &game_server_id)?;`
pub fn add_player(url: &str, game_server_id: &str) -> Result<Player> {
    Ok(AGENT
        .post(&format!("{url}/game-servers/{game_server_id}/players"))
        .send_json(serde_json::json!({}))?
        .into_json()?)
}

/// `remove_player` frees a player's place in a game server.
///
/// Example: `remove_player(url, &game_server_id, &player.player_id)?;`
pub fn remove_player(url: &str, game_server_id: &str, player_id: &str) -> Result<()> {
    AGENT
        .delete(&format!(
            "{url}/game-servers/{game_server_id}/players/{player_id}"
        ))
        .call()?;
    Ok(())
}

/// `tests` checks that requests give up instead of freezing the window.
///
/// | Partitioning | Part | Tests |
/// | --- | --- | --- |
/// | management server host | doesn't answer the connection | `fails_fast_when_host_is_silent` |
/// | | connects but never replies | `fails_fast_when_server_never_replies` |
/// | | answers | `lists_game_servers_when_server_answers` |
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{CONNECT_TIMEOUT, READ_TIMEOUT, list_game_servers};

    /// `UNROUTED_URL` is in TEST-NET-1 (RFC 5737), which no network routes.
    const UNROUTED_URL: &str = "http://192.0.2.1:8000";
    /// `SLACK` is how much longer than its timeout a request may take to fail.
    const SLACK: Duration = Duration::from_secs(1);
    const EMPTY_LIST_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
        Content-Length: 45\r\nConnection: close\r\n\r\n\
        {\"game_servers\": [], \"next_page_token\": null}";

    #[test]
    fn fails_fast_when_host_is_silent() {
        let start: Instant = Instant::now();
        assert!(list_game_servers(UNROUTED_URL).is_err());
        assert!(start.elapsed() < CONNECT_TIMEOUT + SLACK);
    }

    #[test]
    fn fails_fast_when_server_never_replies() {
        let listener: TcpListener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url: String = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(READ_TIMEOUT + SLACK + SLACK);
        });
        let start: Instant = Instant::now();
        assert!(list_game_servers(&url).is_err());
        assert!(start.elapsed() >= READ_TIMEOUT);
        assert!(start.elapsed() < READ_TIMEOUT + SLACK);
    }

    #[test]
    fn lists_game_servers_when_server_answers() {
        let listener: TcpListener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url: String = format!("http://{}", listener.local_addr().unwrap());
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request: Vec<u8> = Vec::new();
            let mut buffer: [u8; 1024] = [0; 1024];
            while !request.ends_with(b"\r\n\r\n") {
                let read_count: usize = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read_count]);
            }
            stream.write_all(EMPTY_LIST_RESPONSE.as_bytes()).unwrap();
        });
        assert!(list_game_servers(&url).unwrap().is_empty());
    }
}
