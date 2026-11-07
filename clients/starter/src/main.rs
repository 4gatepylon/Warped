//! `warped-starter-client` plays the starter square game (`common/protocol/square_game.md`): a
//! lobby to create or join game servers, then the map with every player, laser and the black hole.
//!
//! Interface: `--management-server-url <url>` (default `http://127.0.0.1:8000`).
//!
//! Example: `cargo run --release -- --management-server-url http://10.0.0.5:8000`
//!
//! Must know:
//! - Gotchas: joining blocks the window for the second or two a new game server takes to start.

mod draw;
mod management;
mod session;

use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::mouse::MouseButton;
use sdl3::pixels::Color;
use sdl3::render::{BlendMode, WindowCanvas};

use management::Result;
use session::{ServerMessage, Session};

const FRAME: Duration = Duration::from_millis(16);
const LOBBY_REFRESH: Duration = Duration::from_secs(1);
const START_POLL: Duration = Duration::from_millis(200);
const START_TIMEOUT: Duration = Duration::from_secs(30);
const HUD_TOP: f32 = 22.0;
const TEXT_LEFT: f32 = 20.0;
const SWATCH_SIZE: f32 = 16.0;
const JOIN_KEYS: [Keycode; 9] = [
    Keycode::_1,
    Keycode::_2,
    Keycode::_3,
    Keycode::_4,
    Keycode::_5,
    Keycode::_6,
    Keycode::_7,
    Keycode::_8,
    Keycode::_9,
];

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Play the starter square game.",
    after_help = "Example: warped-starter-client --management-server-url http://10.0.0.5:8000"
)]
struct Arguments {
    /// Where the management server is.
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    management_server_url: String,
}

/// `Welcome` is what the game server said about this player and the map when it joined.
struct Welcome {
    number: u32,
    color: Color,
    map: draw::Map,
    player_size_m: f32,
    schwarzschild_radius_m: f32,
    shot_cooldown: Duration,
}

/// `Game` is one game server that this client plays in, as one player.
struct Game {
    session: Session,
    game_server_id: String,
    player_id: String,
    welcome: Option<Welcome>,
    players: Vec<session::PlayerState>,
    lasers: Vec<session::LaserState>,
    is_alive: bool,
    last_shot: Option<Instant>,
    sent_direction: (f32, f32),
    closed_reason: Option<String>,
}

/// `Screen` is what the window shows: the lobby, or a game.
enum Screen {
    Lobby {
        game_servers: Vec<management::GameServer>,
        refreshed: Option<Instant>,
        note: String,
    },
    Playing(Box<Game>),
}

/// `Action` is what an input event asks for.
enum Action {
    Quit,
    Escape,
    Key(Keycode),
    Shoot((f32, f32)),
}

/// `to_action` returns what an input event asks for, if anything.
///
/// Example: `let actions: Vec<Action> = events.poll_iter().filter_map(to_action).collect();`
fn to_action(event: Event) -> Option<Action> {
    match event {
        Event::Quit { .. } => Some(Action::Quit),
        Event::KeyDown {
            keycode: Some(Keycode::Escape),
            ..
        } => Some(Action::Escape),
        Event::KeyDown {
            keycode: Some(keycode),
            repeat: false,
            ..
        } => Some(Action::Key(keycode)),
        Event::MouseButtonDown {
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } => Some(Action::Shoot((x, y))),
        _ => None,
    }
}

/// `lobby` returns a lobby screen that refreshes its list right away.
///
/// Example: `screen = lobby(String::new());`
fn lobby(note: String) -> Screen {
    Screen::Lobby {
        game_servers: Vec::new(),
        refreshed: None,
        note,
    }
}

/// `join` adds a new player to a game server, waits for it to run, and connects to it.
///
/// Example: `let game = join(url, &game_server_id)?;`
fn join(url: &str, game_server_id: &str) -> Result<Game> {
    let player: management::Player = management::add_player(url, game_server_id)?;
    let deadline: Instant = Instant::now() + START_TIMEOUT;
    let address: management::Address = loop {
        let game_server: management::GameServer = management::get_game_server(url, game_server_id)?;
        match (game_server.state.as_str(), game_server.address) {
            ("running", Some(address)) => break address,
            ("starting", _) if Instant::now() < deadline => thread::sleep(START_POLL),
            (state, _) => {
                management::remove_player(url, game_server_id, &player.player_id)?;
                return Err(format!("the game server is {state}").into());
            }
        }
    };
    Ok(Game {
        session: session::connect(&address.host, address.port, &player.player_id)?,
        game_server_id: game_server_id.to_string(),
        player_id: player.player_id,
        welcome: None,
        players: Vec::new(),
        lasers: Vec::new(),
        is_alive: true,
        last_shot: None,
        sent_direction: (0.0, 0.0),
        closed_reason: None,
    })
}

/// `leave` closes a game's connection and frees its player's place, returning any error as text.
///
/// Example: `screen = lobby(leave(url, game));`
fn leave(url: &str, game: Game) -> String {
    session::close(game.session);
    match management::remove_player(url, &game.game_server_id, &game.player_id) {
        Ok(()) => String::new(),
        Err(error) => format!("Couldn't leave cleanly: {error}"),
    }
}

/// `update_game` applies the game server's messages and sends this frame's movement.
///
/// Example: `update_game(&mut game, &keyboard)?;`
fn update_game(game: &mut Game, keyboard: &sdl3::keyboard::KeyboardState) -> Result<()> {
    match session::receive_messages(&mut game.session) {
        Ok(messages) => {
            for message in messages {
                match message {
                    ServerMessage::Welcome {
                        number,
                        color_hex,
                        map_size_m,
                        player_size_m,
                        schwarzschild_radius_m,
                        shot_cooldown_s,
                    } => {
                        game.welcome = Some(Welcome {
                            number,
                            color: draw::parse_color(&color_hex)?,
                            map: draw::Map { size_m: map_size_m },
                            player_size_m,
                            schwarzschild_radius_m,
                            shot_cooldown: Duration::from_secs_f32(shot_cooldown_s),
                        });
                    }
                    ServerMessage::State { players, lasers } => {
                        if let Some(welcome) = &game.welcome {
                            game.is_alive =
                                players.iter().any(|player| player.number == welcome.number);
                        }
                        game.players = players;
                        game.lasers = lasers;
                    }
                }
            }
        }
        Err(reason) if game.closed_reason.is_none() => game.closed_reason = Some(reason),
        Err(_) => {}
    }
    if !game.is_alive || game.closed_reason.is_some() || game.welcome.is_none() {
        return Ok(());
    }
    let is_pressed = |scancodes: [Scancode; 2]| {
        scancodes
            .iter()
            .any(|scancode| keyboard.is_scancode_pressed(*scancode))
    };
    let direction: (f32, f32) = (
        is_pressed([Scancode::D, Scancode::Right]) as i32 as f32
            - is_pressed([Scancode::A, Scancode::Left]) as i32 as f32,
        is_pressed([Scancode::S, Scancode::Down]) as i32 as f32
            - is_pressed([Scancode::W, Scancode::Up]) as i32 as f32,
    );
    if direction != game.sent_direction {
        session::send_move(&mut game.session, direction)?;
        game.sent_direction = direction;
    }
    Ok(())
}

/// `shoot` fires toward a window point, if the player is alive and its laser has recharged.
///
/// Example: `shoot(&mut game, (400.0, 300.0))?;`
fn shoot(game: &mut Game, at: (f32, f32)) -> Result<()> {
    let Some(welcome) = &game.welcome else {
        return Ok(());
    };
    let is_recharged: bool = game
        .last_shot
        .is_none_or(|last_shot| last_shot.elapsed() >= welcome.shot_cooldown);
    if game.is_alive && game.closed_reason.is_none() && is_recharged {
        session::send_shoot(&mut game.session, welcome.map.to_map(at))?;
        game.last_shot = Some(Instant::now());
    }
    Ok(())
}

/// `draw_game` draws the map, the players and the lasers, and a line of status above them.
///
/// Example: `draw_game(&mut canvas, &game)?;`
fn draw_game(canvas: &mut WindowCanvas, game: &Game) -> Result<()> {
    let Some(welcome) = &game.welcome else {
        return draw::draw_text(
            canvas,
            "Connecting...",
            TEXT_LEFT,
            HUD_TOP,
            draw::TEXT_COLOR,
        );
    };
    draw::draw_map(canvas, &welcome.map, welcome.schwarzschild_radius_m)?;
    for player in &game.players {
        let color: Color = draw::parse_color(&player.color_hex)?;
        draw::draw_square(
            canvas,
            &welcome.map,
            (player.x_m, player.y_m),
            welcome.player_size_m,
            color,
            player.number == welcome.number,
        )?;
    }
    for laser in &game.lasers {
        draw::draw_laser(canvas, &welcome.map, &laser.points_m)?;
    }
    let status: String = if let Some(reason) = &game.closed_reason {
        format!("Disconnected ({reason}).  Esc: lobby")
    } else if !game.is_alive {
        "You were eliminated; watching.  Esc: lobby".to_string()
    } else {
        let recharge_s: f32 = game.last_shot.map_or(0.0, |last_shot| {
            (welcome.shot_cooldown.as_secs_f32() - last_shot.elapsed().as_secs_f32()).max(0.0)
        });
        let laser: String = if recharge_s > 0.0 {
            format!("{recharge_s:.1} s")
        } else {
            "ready".to_string()
        };
        format!("You    click: shoot ({laser})   Esc: lobby")
    };
    draw::draw_text(canvas, &status, TEXT_LEFT, HUD_TOP, draw::TEXT_COLOR)?;
    if game.is_alive && game.closed_reason.is_none() {
        canvas.set_draw_color(welcome.color);
        canvas.fill_rect(sdl3::render::FRect::new(
            TEXT_LEFT + 56.0,
            HUD_TOP,
            SWATCH_SIZE,
            SWATCH_SIZE,
        ))?;
    }
    Ok(())
}

/// `draw_lobby` lists the game servers that have room, with the keys that join them.
///
/// Example: `draw_lobby(&mut canvas, &game_servers, "")?;`
fn draw_lobby(
    canvas: &mut WindowCanvas,
    game_servers: &[management::GameServer],
    note: &str,
) -> Result<()> {
    let mut lines: Vec<String> = vec![
        "WARPED - starter square game".to_string(),
        String::new(),
        "N    create a game and join it".to_string(),
        "1-9  join a game below".to_string(),
        "Esc  quit".to_string(),
        String::new(),
    ];
    if game_servers.is_empty() {
        lines.push("No games yet.".to_string());
    }
    for (i, game_server) in game_servers.iter().enumerate() {
        let short_id: &str = &game_server.game_server_id[..8];
        lines.push(format!(
            "{}    game {short_id}  {} players  {}",
            i + 1,
            game_server.player_count,
            game_server.state
        ));
    }
    lines.push(String::new());
    lines.push(note.to_string());
    for (i, line) in lines.iter().enumerate() {
        draw::draw_text(
            canvas,
            line,
            TEXT_LEFT,
            HUD_TOP + i as f32 * draw::TEXT_LINE_HEIGHT,
            draw::TEXT_COLOR,
        )?;
    }
    Ok(())
}

/// `main` opens the window and runs the lobby and games until the window closes or Esc quits.
///
/// Example: `cargo run --release -- --management-server-url http://10.0.0.5:8000`
fn main() -> Result<()> {
    let arguments: Arguments = Arguments::parse();
    let url: &str = arguments.management_server_url.trim_end_matches('/');
    let sdl: sdl3::Sdl = sdl3::init()?;
    let window: sdl3::video::Window = sdl
        .video()?
        .window("Warped (starter)", draw::WINDOW_WIDTH, draw::WINDOW_HEIGHT)
        .position_centered()
        .high_pixel_density()
        .build()?;
    let mut canvas: WindowCanvas = window.into_canvas();
    canvas.set_blend_mode(BlendMode::Blend);
    let mut events: sdl3::EventPump = sdl.event_pump()?;
    let mut screen: Screen = lobby(String::new());
    'running: loop {
        let actions: Vec<Action> = events.poll_iter().filter_map(to_action).collect();
        for action in actions {
            match action {
                Action::Quit => break 'running,
                Action::Escape if matches!(screen, Screen::Lobby { .. }) => break 'running,
                Action::Escape => {
                    if let Screen::Playing(game) =
                        std::mem::replace(&mut screen, lobby(String::new()))
                    {
                        screen = lobby(leave(url, *game));
                    }
                }
                Action::Shoot(at) => {
                    if let Screen::Playing(game) = &mut screen {
                        shoot(game, at)?;
                    }
                }
                Action::Key(keycode) => {
                    let Screen::Lobby {
                        game_servers, note, ..
                    } = &mut screen
                    else {
                        continue;
                    };
                    let game_server_id: Option<String> = if keycode == Keycode::N {
                        match management::create_game_server(url) {
                            Ok(game_server) => Some(game_server.game_server_id),
                            Err(error) => {
                                *note = format!("Couldn't create a game: {error}");
                                None
                            }
                        }
                    } else {
                        JOIN_KEYS
                            .iter()
                            .position(|key| *key == keycode)
                            .and_then(|i| game_servers.get(i))
                            .map(|game_server| game_server.game_server_id.clone())
                    };
                    let Some(game_server_id) = game_server_id else {
                        continue;
                    };
                    show_note(&mut canvas, "Joining...")?;
                    match join(url, &game_server_id) {
                        Ok(game) => screen = Screen::Playing(Box::new(game)),
                        Err(error) => *note = format!("Couldn't join: {error}"),
                    }
                }
            }
        }
        draw::begin_frame(&mut canvas)?;
        match &mut screen {
            Screen::Lobby {
                game_servers,
                refreshed,
                note,
            } => {
                if refreshed.is_none_or(|refreshed| refreshed.elapsed() >= LOBBY_REFRESH) {
                    match management::list_game_servers(url) {
                        Ok(list) => *game_servers = list,
                        Err(error) => *note = format!("Couldn't reach {url}: {error}"),
                    }
                    *refreshed = Some(Instant::now());
                }
                draw_lobby(&mut canvas, game_servers, note)?;
            }
            Screen::Playing(game) => {
                update_game(game, &events.keyboard_state())?;
                draw_game(&mut canvas, game)?;
            }
        }
        canvas.present();
        thread::sleep(FRAME);
    }
    if let Screen::Playing(game) = screen {
        leave(url, *game);
    }
    Ok(())
}

/// `show_note` shows one line of text right away, before a call that blocks.
///
/// Example: `show_note(&mut canvas, "Joining...")?;`
fn show_note(canvas: &mut WindowCanvas, text: &str) -> Result<()> {
    draw::begin_frame(canvas)?;
    draw::draw_text(canvas, text, TEXT_LEFT, HUD_TOP, draw::TEXT_COLOR)?;
    canvas.present();
    Ok(())
}
