//! `title_choice.rs` runs the title screen of the Mac app: it shows the online games, asks for and
//! saves a game password, and starts the game window the player chose (exiting 3 when the server
//! refuses the password).
//!
//! Interface: `run_title_screen`, `show_alert`.
//!
//! Example: `run_title_screen()?;`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use sdl3::pixels::Color;

use crate::play::draw_text_overlay;
use crate::run_folder::format_utc_now;
use crate::session::OverlayText;

use crate::{
    MENU_HEIGHT_FRAC, MENU_MASK_PX, Result, WINDOW_TITLE, WRONG_PASSWORD_EXIT_CODE, app_bundle,
    title,
};

const GAME_PASSWORD_HOME_PATH: &str = "Library/Application Support/Warped/game_password";
const LOGS_HOME_FOLDER: &str = "Library/Logs/Warped";
const ONLINE_GAMES_FILE: &str = "online_games.txt";
const OWNER_ONLY_FILE_MODE: u32 = 0o600;
const TITLE_WINDOW_PX: (u32, u32) = (960, 600);
const TITLE_FRAME_MS: u64 = 16;

/// `run_title_screen` shows the title screen (`title.rs`) until the player quits, starting each
/// chosen game as this program with that game's flags and showing why in a pop-up when one fails.
///
/// Example: `run_title_screen()?;`
pub(crate) fn run_title_screen() -> Result<()> {
    let online_games_path: Option<PathBuf> = app_bundle::get_app_resources_folder()
        .map(|folder| folder.join(ONLINE_GAMES_FILE))
        .filter(|path| path.exists());
    let online_games: String = match &online_games_path {
        Some(path) => fs::read_to_string(path)?,
        None => String::new(),
    };
    let mut title_screen: title::TitleScreen = title::TitleScreen::New(&online_games)?;
    while let Some(choice) = pick_title_choice(&mut title_screen)? {
        if let Err(error) = start_title_choice(&choice) {
            show_alert(&error.to_string())?;
        }
    }
    Ok(())
}

/// `pick_title_choice` opens the title screen's window until the player picks a game (`Some`) or
/// quits (`None`).
///
/// Example: `let choice = pick_title_choice(&mut title_screen)?;`
fn pick_title_choice(title_screen: &mut title::TitleScreen) -> Result<Option<title::Choice>> {
    let sdl: sdl3::Sdl = sdl3::init()?;
    let window: sdl3::video::Window = sdl
        .video()?
        .window(WINDOW_TITLE, TITLE_WINDOW_PX.0, TITLE_WINDOW_PX.1)
        .high_pixel_density()
        .position_centered()
        .build()?;
    let mut canvas: sdl3::render::WindowCanvas = window.into_canvas();
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => return Ok(None),
                Event::KeyDown {
                    scancode: Some(scancode),
                    ..
                } => match scancode {
                    Scancode::Up | Scancode::W => title_screen.move_selection(-1),
                    Scancode::Down | Scancode::S => title_screen.move_selection(1),
                    Scancode::Escape => return Ok(None),
                    Scancode::Return | Scancode::KpEnter => {
                        let choice: title::Choice = title_screen.get_selected().clone();
                        return Ok((choice != title::Choice::Quit).then_some(choice));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        draw_text_overlay(
            &mut canvas,
            &OverlayText {
                lines: title_screen.get_lines(),
                mask_px: MENU_MASK_PX,
                height_frac: MENU_HEIGHT_FRAC,
                is_centered: false,
                dim_frac: 0.0,
                picture: None,
            },
        )?;
        canvas.present();
        std::thread::sleep(Duration::from_millis(TITLE_FRAME_MS));
    }
}

/// `start_title_choice` runs the chosen game as this program with its flags until it ends, its
/// output in a log under `~/Library/Logs/Warped/`; an online game first asks for the game's
/// password once and keeps it, and forgets it if the server refuses it.
///
/// Example: `start_title_choice(&title::Choice::Join(MapName::LunarSurface))?;`
fn start_title_choice(choice: &title::Choice) -> Result<()> {
    let home: PathBuf = PathBuf::from(std::env::var_os("HOME").ok_or("HOME isn't set")?);
    let password_path: PathBuf = home.join(GAME_PASSWORD_HOME_PATH);
    let mut flags: Vec<String> = choice.get_flags();
    if choice.is_online() {
        if !password_path.exists() {
            let Some(password) = ask_game_password()? else {
                return Ok(());
            };
            save_game_password(&password_path, &password)?;
        }
        flags.extend([
            "--game-password-file".to_string(),
            password_path.display().to_string(),
        ]);
    }
    let logs_folder: PathBuf = home.join(LOGS_HOME_FOLDER);
    fs::create_dir_all(&logs_folder)?;
    let log_path: PathBuf = logs_folder.join(format!("{}.log", format_utc_now()));
    let log: fs::File = fs::File::create(&log_path)?;
    let status: std::process::ExitStatus = Command::new(std::env::current_exe()?)
        .args(&flags)
        .stdout(log.try_clone()?)
        .stderr(log)
        .status()?;
    if status.code() == Some(i32::from(WRONG_PASSWORD_EXIT_CODE)) {
        fs::remove_file(&password_path)?;
    }
    if status.success() {
        return Ok(());
    }
    let text: String = fs::read_to_string(&log_path)?;
    let reason: &str = match text.lines().rev().find(|line| line.starts_with("Error: ")) {
        Some(line) => line,
        None => "It stopped without saying why.",
    };
    Err(format!(
        "The game stopped. {reason}\n\nIts log: {}",
        log_path.display()
    )
    .into())
}

/// `ask_game_password` asks the player for the game's password in a dialog, or gives `None` if
/// they cancel.
///
/// Example: `let password = ask_game_password()?;`
fn ask_game_password() -> Result<Option<String>> {
    let output: std::process::Output = Command::new("osascript")
        .args([
            "-e",
            "text returned of (display dialog \"The game's password (ask whoever runs the server):\" default answer \"\" with hidden answer with title \"Warped\")",
        ])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let password: String = String::from_utf8(output.stdout)?.trim_end().to_string();
    Ok((!password.is_empty()).then_some(password))
}

/// `save_game_password` writes `password` to `path`, readable only by this user.
///
/// Example: `save_game_password(&path, "correct horse")?;`
fn save_game_password(path: &Path, password: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::create_dir_all(path.parent().ok_or("a password file has a folder")?)?;
    let mut file: fs::File = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(OWNER_ONLY_FILE_MODE)
        .open(path)?;
    writeln!(file, "{password}")?;
    Ok(())
}

/// `show_alert` shows `message` in a pop-up and waits for the player to close it.
///
/// Example: `show_alert("The game stopped.")?;`
pub(crate) fn show_alert(message: &str) -> Result<()> {
    Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display alert \"Warped\" message (item 1 of argv)",
            "-e",
            "end run",
            message,
        ])
        .status()?;
    Ok(())
}
