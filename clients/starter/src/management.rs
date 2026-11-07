//! `management.rs` calls the management server's HTTP API
//! (`common/protocol/management_server_api.md`).
//!
//! Interface: `GameServer`, `Player`, `list_game_servers`, `create_game_server`,
//! `get_game_server`, `add_player`, `remove_player`.
//!
//! Example: `let game_servers = list_game_servers("http://127.0.0.1:8000")?;`

use serde::Deserialize;

/// `MAX_PLAYER_COUNT` is how many players a new game server takes; big, so that rooms never fill.
const MAX_PLAYER_COUNT: u32 = 1000;
/// `LIST_LIMIT` is how many game servers the lobby shows, one per number key.
const LIST_LIMIT: u32 = 9;

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
    let list: GameServerList = ureq::get(&format!("{url}/game-servers"))
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
    Ok(ureq::post(&format!("{url}/game-servers"))
        .send_json(serde_json::json!({ "max_player_count": MAX_PLAYER_COUNT }))?
        .into_json()?)
}

/// `get_game_server` returns a game server's state and, once it runs, its address.
///
/// Example: `let game_server = get_game_server(url, &game_server_id)?;`
pub fn get_game_server(url: &str, game_server_id: &str) -> Result<GameServer> {
    Ok(ureq::get(&format!("{url}/game-servers/{game_server_id}"))
        .call()?
        .into_json()?)
}

/// `add_player` adds a new player to a game server, which then admits it.
///
/// Example: `let player = add_player(url, &game_server_id)?;`
pub fn add_player(url: &str, game_server_id: &str) -> Result<Player> {
    Ok(
        ureq::post(&format!("{url}/game-servers/{game_server_id}/players"))
            .send_json(serde_json::json!({}))?
            .into_json()?,
    )
}

/// `remove_player` frees a player's place in a game server.
///
/// Example: `remove_player(url, &game_server_id, &player.player_id)?;`
pub fn remove_player(url: &str, game_server_id: &str, player_id: &str) -> Result<()> {
    ureq::delete(&format!(
        "{url}/game-servers/{game_server_id}/players/{player_id}"
    ))
    .call()?;
    Ok(())
}
