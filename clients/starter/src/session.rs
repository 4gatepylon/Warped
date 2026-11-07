//! `session.rs` plays over one WebSocket to a game server (`common/protocol/square_game.md`).
//!
//! Interface: `ServerMessage`, `Session`, `connect`, `receive_messages`, `send_move`,
//! `send_shoot`, `close`.
//!
//! Example: `let mut session = connect(&address.host, address.port, &player.player_id)?;`

use std::io::ErrorKind;
use std::net::TcpStream;

use serde::Deserialize;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use crate::management::Result;

/// `PlayerState` is one living player in a `state` message.
#[derive(Deserialize)]
pub struct PlayerState {
    pub number: u32,
    pub color_hex: String,
    pub x_m: f32,
    pub y_m: f32,
}

/// `LaserState` is the visible part of one laser in a `state` message.
#[derive(Deserialize)]
pub struct LaserState {
    pub points_m: Vec<[f32; 2]>,
}

/// `ServerMessage` is a message from the game server.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        number: u32,
        color_hex: String,
        map_size_m: f32,
        player_size_m: f32,
        schwarzschild_radius_m: f32,
        shot_cooldown_s: f32,
    },
    State {
        players: Vec<PlayerState>,
        lasers: Vec<LaserState>,
    },
}

/// `Session` is an open WebSocket to a game server, read without blocking.
pub struct Session {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

/// `connect` opens a WebSocket to a game server as a player the management server added.
///
/// Example: `let mut session = connect("10.0.0.5", 20000, &player_id)?;`
pub fn connect(host: &str, port: u16, player_id: &str) -> Result<Session> {
    let (socket, _) =
        tungstenite::connect(format!("ws://{host}:{port}/play?player_id={player_id}"))?;
    match socket.get_ref() {
        MaybeTlsStream::Plain(stream) => stream.set_nonblocking(true)?,
        _ => return Err("expected a plain ws:// connection".into()),
    }
    Ok(Session { socket })
}

/// `receive_messages` returns the messages that arrived since the last call, or why it closed.
///
/// Example: `let messages = receive_messages(&mut session)?;`
pub fn receive_messages(session: &mut Session) -> std::result::Result<Vec<ServerMessage>, String> {
    let mut messages: Vec<ServerMessage> = Vec::new();
    loop {
        match session.socket.read() {
            Ok(Message::Text(text)) => messages
                .push(serde_json::from_str(text.as_str()).map_err(|error| error.to_string())?),
            Ok(Message::Close(Some(frame))) => {
                return Err(format!("{} {}", u16::from(frame.code), frame.reason));
            }
            Ok(Message::Close(None)) => return Err("closed".to_string()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(error)) if error.kind() == ErrorKind::WouldBlock => {
                return Ok(messages);
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// `send_text` sends a message, leaving it queued if the socket can't take it yet.
///
/// Example: `send_text(session, "{}".to_string())?;`
fn send_text(session: &mut Session, text: String) -> Result<()> {
    match session.socket.send(Message::text(text)) {
        Err(tungstenite::Error::Io(error)) if error.kind() == ErrorKind::WouldBlock => Ok(()),
        result => Ok(result?),
    }
}

/// `send_move` tells the game server which way the player wants to go.
///
/// Example: `send_move(&mut session, (1.0, 0.0))?;`
pub fn send_move(session: &mut Session, direction: (f32, f32)) -> Result<()> {
    let message =
        serde_json::json!({"type": "move", "direction_x": direction.0, "direction_y": direction.1});
    send_text(session, message.to_string())
}

/// `send_shoot` fires the player's laser toward a point of the map.
///
/// Example: `send_shoot(&mut session, (20.0, 5.0))?;`
pub fn send_shoot(session: &mut Session, aim_m: (f32, f32)) -> Result<()> {
    let message = serde_json::json!({"type": "shoot", "aim_x_m": aim_m.0, "aim_y_m": aim_m.1});
    send_text(session, message.to_string())
}

/// `close` closes the WebSocket, without waiting for the game server's answer.
///
/// Example: `close(session);`
pub fn close(mut session: Session) {
    if session.socket.close(None).is_ok() {
        let _ = session.socket.flush();
    }
}
