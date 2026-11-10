//! `warped` is Warped's one-window app (W-141, one window for the whole game): one SDL3 window
//! with a Metal layer, the main menu (games on the network, joining, hosting, playing alone or
//! against bots, the match or any one level, player settings, credits), the game menu (the
//! match's status, teams, rejoining, the game master's choice) and the levels, which will draw
//! into the same window once they are libraries (`warped_shared::app`). Servers and bots are
//! headless child processes (`children.rs`); no second window opens.
//!
//! Interface:
//! - `--width-px <n>`, `--height-px <n>`: the window's starting size in screen points (1280x800).
//! - `--player-settings <path>`: the player's settings file (default
//!   `~/.config/warped/player_settings.yaml`, which the levels read too).
//! - `--match-port <port>`, `--level-port <port>`: the UDP ports of a game this app hosts (47850
//!   and 47852).
//! - `--discovery-port <port>`: the UDP port it listens on for games on the network (47851).
//! - `--save-screen-pictures`: draw every screen without a window into
//!   `.artifacts/screen_pictures/<run_id>/` at `--width-px` x `--height-px` pixels, and exit.
//! - `--label <label>`: a suffix for the run folder's name.
//!
//! Example: `warped --width-px 2560 --height-px 1440`
//!
//! Must know:
//! - Gotchas: keys in the menus are fixed (arrows, Return, Escape, R, Backspace, digits), so a bad
//!   key binding can always be fixed from the player settings page.

mod app_state;
mod children;
mod game_menu;
mod levels_manifest;
mod main_menu;
mod menu_page;
mod painter;
mod player_settings_page;
mod screens;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use objc2_core_foundation::CGSize;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use warped_shared::key_bindings::get_pressed_control;
use warped_shared::match_protocol::{
    LevelEntry, MATCH_DISCOVERY_PORT, MATCH_PORT, MatchView, Phase,
};
use warped_shared::match_state::{MatchState, Team};
use warped_shared::metal_window::{MetalWindow, WindowOptions};
use warped_shared::page::Shape;
use warped_shared::player_settings::{PerPlayerSettings, get_saved_player_settings_path};

use crate::app_state::{App, Ports, format_utc_now, get_address_shapes, get_credits_shapes};
use crate::children::make_run_folder;
use crate::game_menu::{GameMenu, LevelMenu, get_level_shapes};
use crate::levels_manifest::{Picture, get_levels_manifest_path, load_levels, load_map_pictures};
use crate::main_menu::MainMenu;
use crate::menu_page::{MenuInput, page_t};
use crate::painter::{Painter, PixelOrder, render_page};
use crate::player_settings_page::PlayerSettingsPage;

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const WINDOW_TITLE: &str = "Warped";
const DEFAULT_LEVEL_PORT: u16 = 47_852;
const FRAME_MS: u64 = 16;

/// `Arguments` is the command line (see the file's docstring).
#[derive(Parser)]
#[command(
    about = "Warped's one-window app.",
    after_help = "Example: warped --width-px 2560 --height-px 1440"
)]
struct Arguments {
    /// The window's starting width in screen points, and the screen pictures' width in pixels.
    #[arg(long, default_value_t = 1280)]
    width_px: u32,
    /// The window's starting height in screen points, and the screen pictures' height in pixels.
    #[arg(long, default_value_t = 800)]
    height_px: u32,
    /// This player's own choices (keys, mouse speed, look), a YAML file with
    /// player_settings.yaml's keys (default ~/.config/warped/player_settings.yaml).
    #[arg(long)]
    player_settings: Option<PathBuf>,
    /// The UDP port of the match server of a game this app hosts.
    #[arg(long, default_value_t = MATCH_PORT)]
    match_port: u16,
    /// The UDP port of each round's level server in a game this app hosts.
    #[arg(long, default_value_t = DEFAULT_LEVEL_PORT)]
    level_port: u16,
    /// The UDP port this app listens on for games on the network.
    #[arg(long, default_value_t = MATCH_DISCOVERY_PORT)]
    discovery_port: u16,
    /// Draw every screen without a window into .artifacts/screen_pictures/<run_id>/, and exit.
    #[arg(long)]
    save_screen_pictures: bool,
    /// Suffix for the run folder's name (letters, digits, - and _).
    #[arg(long)]
    label: Option<String>,
}

/// `validate_arguments` fails, naming the argument, on a size of 0 or a label with other
/// characters than letters, digits, - and _.
///
/// Example: `validate_arguments(&arguments)?;`
fn validate_arguments(arguments: &Arguments) -> Result<()> {
    if arguments.width_px == 0 || arguments.height_px == 0 {
        return Err(
            "--width-px and --height-px must be at least 1; e.g. --width-px 1280 --height-px 800"
                .into(),
        );
    }
    if let Some(label) = &arguments.label
        && !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "--label must be letters, digits, - and _, got {label:?}; e.g. --label quiet"
        )
        .into());
    }
    Ok(())
}

/// `load_player_settings` reads the player's settings from `path`, or the built-in ones when the
/// default file doesn't exist yet; a file given with `--player-settings` must exist.
///
/// Example: `let mine = load_player_settings(&path, false)?;`
fn load_player_settings(path: &Path, is_given: bool) -> Result<PerPlayerSettings> {
    match is_given || path.exists() {
        true => PerPlayerSettings::Load(path),
        false => Ok(PerPlayerSettings::Builtin()),
    }
}

/// `get_menu_input` gives what a window event does in a menu, or `None`; while `is_rebinding`,
/// any pressed control is the binding's (Escape cancels).
///
/// Example: `if let Some(input) = get_menu_input(&event, app.is_rebinding()) { app.handle(input); }`
fn get_menu_input(event: &Event, is_rebinding: bool) -> Option<MenuInput> {
    if is_rebinding {
        return get_pressed_control(event).map(|control| match control {
            warped_shared::key_bindings::Control::Key(Scancode::Escape) => MenuInput::Back,
            control => MenuInput::Control(control.get_name()),
        });
    }
    let Event::KeyDown {
        scancode: Some(scancode),
        ..
    } = *event
    else {
        return None;
    };
    Some(match scancode {
        Scancode::Up => MenuInput::Up,
        Scancode::Down => MenuInput::Down,
        Scancode::Left => MenuInput::Left,
        Scancode::Right => MenuInput::Right,
        Scancode::Return | Scancode::KpEnter => MenuInput::Activate,
        Scancode::Escape => MenuInput::Back,
        Scancode::Backspace => MenuInput::Erase,
        Scancode::R => MenuInput::Reset,
        Scancode::Period | Scancode::KpPeriod => MenuInput::Typed('.'),
        Scancode::Semicolon => MenuInput::Typed(':'),
        _ => {
            let name: String = scancode.name().to_string();
            let digit: char = name
                .strip_prefix("Keypad ")
                .unwrap_or(&name)
                .chars()
                .next()?;
            if name.trim_start_matches("Keypad ").len() == 1 && digit.is_ascii_digit() {
                MenuInput::Typed(digit)
            } else {
                return None;
            }
        }
    })
}

/// `run_window` opens the app's window and runs it until the player quits.
///
/// Example: `run_window(&arguments, app)?;`
fn run_window(arguments: &Arguments, mut app: App) -> Result<()> {
    let mut window: MetalWindow = MetalWindow::Open(
        WINDOW_TITLE,
        (arguments.width_px, arguments.height_px),
        WindowOptions::default(),
    )?;
    let mut painter: Painter = Painter::New(window.get_device())?;
    let mut shown: Option<(Vec<Shape>, (u32, u32))> = None;
    while !app.should_quit() {
        let events: Vec<Event> = window.get_event_pump_mut().poll_iter().collect();
        for event in events {
            match event {
                Event::Quit { .. } => return Ok(()),
                Event::KeyDown {
                    scancode: Some(Scancode::Tab),
                    ..
                } => app.set_map_held(true),
                Event::KeyUp {
                    scancode: Some(Scancode::Tab),
                    ..
                } => app.set_map_held(false),
                Event::MouseButtonDown {
                    mouse_btn: sdl3::mouse::MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    let (width, height) = window.get_window().size();
                    app.click([x, y], [width as f32, height as f32]);
                }
                _ => {}
            }
            if let Some(input) = get_menu_input(&event, app.is_rebinding()) {
                app.handle(input);
            }
        }
        app.update();
        let (shapes, page_size, pictures): (Vec<Shape>, [f32; 2], Vec<Option<Picture>>) =
            app.get_page();
        let size_px: (u32, u32) = window.get_window().size_in_pixels();
        if shown.as_ref() != Some(&(shapes.clone(), size_px)) {
            let layer: &CAMetalLayer = window.get_layer();
            layer.setDrawableSize(CGSize {
                width: f64::from(size_px.0),
                height: f64::from(size_px.1),
            });
            let pixels: Vec<u8> =
                render_page(size_px, page_size, &shapes, &pictures, PixelOrder::Bgra)?;
            let size: (usize, usize) = (size_px.0 as usize, size_px.1 as usize);
            if painter.paint(layer, window.get_device(), &pixels, size)? {
                shown = Some((shapes, size_px));
            }
        }
        std::thread::sleep(Duration::from_millis(FRAME_MS));
    }
    Ok(())
}

/// `make_sample_view` gives a match of `levels` mid-way (blue won the first round, then red twice,
/// round 4 being played) for the screen pictures.
///
/// Example: `let view = make_sample_view(&levels);`
fn make_sample_view(levels: &[LevelEntry]) -> MatchView {
    let mut state: MatchState = MatchState::New(levels.len());
    for winner in [Team::Blue, Team::Red, Team::Red] {
        state.record_round(winner);
    }
    let level: usize = state.get_next_level().expect("the sample match goes on");
    MatchView {
        state,
        phase: Phase::Playing {
            round: 4,
            level,
            port: DEFAULT_LEVEL_PORT,
        },
        levels: levels.to_vec(),
        team_counts: [2, 1],
        match_left_s: Some(1234.0),
    }
}

/// `save_screen_pictures` draws every screen without a window at the arguments' size into a run
/// folder, as PNGs, and prints their folder.
///
/// Example: `save_screen_pictures(&arguments, &levels, &pictures, &path)?;`
fn save_screen_pictures(
    arguments: &Arguments,
    levels: &[LevelEntry],
    pictures: &[Option<Picture>],
    player_settings_path: &Path,
) -> Result<()> {
    let mut run_id: String = format_utc_now();
    if let Some(label) = &arguments.label {
        run_id = format!("{run_id}-{label}");
    }
    let folder: PathBuf = make_run_folder("screen_pictures", &run_id)?;
    let view: MatchView = make_sample_view(levels);
    let main_menu: MainMenu = MainMenu::New();
    let main_rows = main_menu.get_rows(&[], true);
    let game_menu: GameMenu = GameMenu::New();
    let team: Option<Team> = Some(Team::Blue);
    let round: (usize, usize) = game_menu::get_round(&view).expect("the sample plays a round");
    let mut free_view: MatchView = view.clone();
    free_view.state.start_free_play(0);
    free_view.state.record_free_round(Team::Red);
    let settings_page: PlayerSettingsPage =
        PlayerSettingsPage::New(&PerPlayerSettings::Builtin(), player_settings_path);
    let screens: Vec<(&str, page_t)> = vec![
        ("main_menu", main_menu.get_shapes(&main_rows, levels, None)),
        ("join_by_address", get_address_shapes("192.168.1.", None)),
        ("player_settings", settings_page.get_shapes()),
        ("credits", get_credits_shapes()),
        (
            "game_menu_connecting",
            game_menu.get_shapes(&game_menu.get_rows(None), None, None, "192.168.1.20:47850"),
        ),
        (
            "game_menu_mid_match",
            game_menu.get_shapes(&game_menu.get_rows(team), team, Some(&view), ""),
        ),
        (
            "game_menu_free_play",
            game_menu.get_shapes(&game_menu.get_rows(team), team, Some(&free_view), ""),
        ),
        (
            "level_stand_in",
            get_level_shapes(&view, Team::Blue, round, false),
        ),
        (
            "level_tab_map",
            get_level_shapes(&view, Team::Blue, round, true),
        ),
        (
            "in_level_menu",
            LevelMenu::New().get_shapes(Some(&view), team),
        ),
    ];
    for (name, (shapes, page_size)) in screens {
        let size_px: (u32, u32) = (arguments.width_px, arguments.height_px);
        let pixels: Vec<u8> = render_page(size_px, page_size, &shapes, pictures, PixelOrder::Rgba)?;
        let path: PathBuf = folder.join(format!("{name}.png"));
        image::save_buffer(
            &path,
            &pixels,
            size_px.0,
            size_px.1,
            image::ExtendedColorType::Rgba8,
        )?;
    }
    println!("{}", folder.display());
    Ok(())
}

/// `main` reads the levels manifest and the player's settings, then draws the screen pictures or
/// runs the window.
///
/// Example: `warped`
fn main() -> Result<()> {
    let arguments: Arguments = Arguments::parse();
    validate_arguments(&arguments)?;
    let manifest: PathBuf = get_levels_manifest_path();
    let levels: Vec<LevelEntry> = load_levels(&manifest)?;
    let pictures: Vec<Option<Picture>> = load_map_pictures(&manifest, &levels)?;
    let player_settings_path: PathBuf = match &arguments.player_settings {
        Some(path) => path.clone(),
        None => get_saved_player_settings_path()?,
    };
    if arguments.save_screen_pictures {
        return save_screen_pictures(&arguments, &levels, &pictures, &player_settings_path);
    }
    let player_settings: PerPlayerSettings =
        load_player_settings(&player_settings_path, arguments.player_settings.is_some())?;
    let ports: Ports = Ports {
        match_port: arguments.match_port,
        level_port: arguments.level_port,
        discovery_port: arguments.discovery_port,
    };
    let app: App = App::New(
        levels,
        &manifest,
        pictures,
        player_settings,
        &player_settings_path,
        ports,
    );
    run_window(&arguments, app)
}

#[cfg(test)]
mod tests {
    //! Unit tests for `main.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Arguments | the defaults, a size of 0, a bad label | `arguments_are_checked` |
    //! | Keys in menus | arrows, Return, Escape, digits (top row, keypad), a key that does nothing, rebinding (a key, Escape) | `keys_map_to_menu_inputs` |
    //! | Sample match | round 4 of the shipped levels on the attacker's wavefront | `the_sample_match_is_mid_way` |

    use super::*;

    /// `make_key` gives a key-down event for `scancode`.
    ///
    /// Example: `let event = make_key(Scancode::Up);`
    fn make_key(scancode: Scancode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: None,
            scancode: Some(scancode),
            keymod: sdl3::keyboard::Mod::NOMOD,
            repeat: false,
            which: 0,
            raw: 0,
        }
    }

    #[test]
    fn arguments_are_checked() {
        assert!(validate_arguments(&Arguments::parse_from(["warped"])).is_ok());
        assert!(validate_arguments(&Arguments::parse_from(["warped", "--width-px", "0"])).is_err());
        assert!(validate_arguments(&Arguments::parse_from(["warped", "--label", "a b"])).is_err());
    }

    #[test]
    fn keys_map_to_menu_inputs() {
        for (scancode, input) in [
            (Scancode::Up, MenuInput::Up),
            (Scancode::Return, MenuInput::Activate),
            (Scancode::Escape, MenuInput::Back),
            (Scancode::_7, MenuInput::Typed('7')),
            (Scancode::Kp3, MenuInput::Typed('3')),
        ] {
            assert_eq!(get_menu_input(&make_key(scancode), false), Some(input));
        }
        assert_eq!(get_menu_input(&make_key(Scancode::F5), false), None);
        assert_eq!(
            get_menu_input(&make_key(Scancode::J), true),
            Some(MenuInput::Control("J".to_string()))
        );
        assert_eq!(
            get_menu_input(&make_key(Scancode::Escape), true),
            Some(MenuInput::Back)
        );
    }

    #[test]
    fn the_sample_match_is_mid_way() {
        let levels: Vec<LevelEntry> = load_levels(&get_levels_manifest_path()).unwrap();
        let view: MatchView = make_sample_view(&levels);
        assert_eq!(view.state.rounds().len(), 3);
        assert!(matches!(view.phase, Phase::Playing { round: 4, .. }));
    }
}
