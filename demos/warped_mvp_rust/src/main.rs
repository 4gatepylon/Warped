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
//! - `--exit-after-s <s>`: close the window after that many seconds (to check it starts).
//! - `--print-level-programs`: print the levels `make package-mac-app` puts in the app, and exit.
//! - `--loopback-only`: keep the app and the games it hosts on 127.0.0.1 (checks; nothing open to
//!   the network); `WARPED_LOOPBACK_ONLY=1` does the same.
//! - `--tour-levels`: play alone, visit every level in turn (timing its frames, opening its
//!   menu, leaving), write the frame times to `.artifacts/tour_levels/<run_id>/` and quit.
//! - `--tour-level <name>`: with `--tour-levels`, visit only that level (repeatable).
//! - `--tour-ally-bot-count <count>`, `--tour-enemy-bot-count <count>`: with `--tour-levels`, play
//!   with that many fighting bots on the player's team and on the other (0 and 0: alone).
//!
//! Example: `warped --width-px 2560 --height-px 1440`
//!
//! Must know:
//! - Gotchas: keys in the menus are fixed (arrows, Return, Escape, R, Backspace, digits), so a bad
//!   key binding can always be fixed from the player settings page.

mod app_state;
mod children;
mod game_menu;
mod game_settings_page;
mod level_library;
mod level_link;
mod levels_manifest;
mod main_menu;
mod menu_page;
mod painter;
mod player_settings_page;
mod screens;
mod tour_run;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::Parser;
use objc2::rc::autoreleasepool;
use objc2_core_foundation::CGSize;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use warped_shared::app::{LevelEnd, LevelWindow};
use warped_shared::app_bundle::get_level_slug;
use warped_shared::game_settings::{
    GameSettings, SettingsMessage, SettingsScope, read_settings_file,
};
use warped_shared::key_bindings::get_pressed_control;
use warped_shared::match_protocol::{
    LevelEntry, MATCH_DISCOVERY_PORT, MATCH_PORT, MatchView, Phase, SELF_PROGRAM,
};
use warped_shared::match_state::{MatchState, Team};
use warped_shared::metal_window::{MetalWindow, WindowOptions};
use warped_shared::network_scope;
use warped_shared::page::Shape;
use warped_shared::player_settings::{PerPlayerSettings, get_saved_player_settings_path};

use crate::app_state::{App, Ports, format_utc_now, get_address_shapes, get_credits_shapes};
use crate::children::{BotObjective, GameKind, MAX_ALLY_COUNT, MAX_ENEMY_COUNT, make_run_folder};
use crate::game_menu::{GameMenu, LevelMenu, get_level_shapes};
use crate::game_settings_page::GameSettingsPage;
use crate::level_link::{AppLink, Tour};
use crate::levels_manifest::{Picture, get_levels_manifest_path, load_levels, load_map_pictures};
use crate::main_menu::MainMenu;
use crate::menu_page::{MenuInput, page_t};
use crate::painter::{Painter, PixelOrder, render_page};
use crate::player_settings_page::PlayerSettingsPage;
use crate::tour_run::TourRun;

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const WINDOW_TITLE: &str = "Warped";
const REPOSITORY_FROM_CRATE: &str = "../..";
const SETTINGS_FILE_NAME: &str = "server_settings.yaml";
const SAMPLE_SHARED_KEY: &str = "respawn.start_s";
const SAMPLE_SHARED_VALUE: f64 = 5.0;
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
    /// Close the window after this many seconds (to check that it starts).
    #[arg(long)]
    exit_after_s: Option<f64>,
    /// Print, for `make package-mac-app`, each level of the manifest that has its own program: its
    /// file name in the app, its crate folder (from the repository's root), its program's name and
    /// its map picture (`-` for none), tab-separated, one a line; and exit.
    #[arg(long)]
    print_level_programs: bool,
    /// Keep everything on 127.0.0.1, with nothing open to the network: listen for games there, and
    /// host games whose match and level servers do the same (checks and tests; also
    /// WARPED_LOOPBACK_ONLY=1).
    #[arg(long)]
    loopback_only: bool,
    /// Play alone and visit every level of the manifest in turn in this window: settle, time its
    /// frames, open the menu, leave; then write the frame times to
    /// .artifacts/tour_levels/<run_id>/ and quit (a check of the levels inside the app).
    #[arg(long)]
    tour_levels: bool,
    /// With --tour-levels, visit only this level, named as in the manifest or by its slug
    /// (e.g. "Dark Forest" or dark_forest); repeat for several.
    #[arg(long, requires = "tour_levels")]
    tour_level: Vec<String>,
    /// With --tour-levels, play with this many fighting bots on the player's team (0 to 5).
    #[arg(long, requires = "tour_levels", default_value_t = 0,
          value_parser = clap::value_parser!(u32).range(0..=MAX_ALLY_COUNT as i64))]
    tour_ally_bot_count: u32,
    /// With --tour-levels, play against this many fighting bots (0 to 6); with no bots on either
    /// team the tour plays alone.
    #[arg(long, requires = "tour_levels", default_value_t = 0,
          value_parser = clap::value_parser!(u32).range(0..=MAX_ENEMY_COUNT as i64))]
    tour_enemy_bot_count: u32,
}

/// `get_tour_game_kind` gives the game a tour plays: alone without bots, else with `ally_count`
/// fighting bots on the player's team and `enemy_count` on the other.
///
/// Example: `assert_eq!(get_tour_game_kind(0, 0), GameKind::Alone);`
fn get_tour_game_kind(ally_count: u32, enemy_count: u32) -> GameKind {
    match (ally_count, enemy_count) {
        (0, 0) => GameKind::Alone,
        _ => GameKind::Bots {
            ally_count,
            enemy_count,
            objective: BotObjective::Fight,
        },
    }
}

/// `print_level_programs` prints the levels `make package-mac-app` builds and puts in the app (see
/// `--print-level-programs`).
///
/// Example: `print_level_programs(&levels)?;`
fn print_level_programs(levels: &[LevelEntry]) -> Result<()> {
    for level in levels.iter().filter(|level| level.program != SELF_PROGRAM) {
        let program: &str = Path::new(&level.program)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "{}'s program {:?} has no file name",
                    level.name, level.program
                )
            })?;
        println!(
            "{}\t{}\t{program}\t{}\t{}",
            get_level_slug(&level.name),
            level.folder,
            level.map_picture.as_deref().unwrap_or("-"),
            level.server_settings.as_deref().unwrap_or("-")
        );
    }
    Ok(())
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

/// `get_tour_level_indices` gives the indices in `levels` of the levels `--tour-levels` visits:
/// those named in `names` (a level's name or its slug), in that order, or every level for none.
/// It fails on a name no level has.
///
/// Example: `let indices = get_tour_level_indices(&levels, &["dust".to_string()])?;`
fn get_tour_level_indices(levels: &[LevelEntry], names: &[String]) -> Result<Vec<usize>> {
    if names.is_empty() {
        return Ok((0..levels.len()).collect());
    }
    names
        .iter()
        .map(|name| {
            levels
                .iter()
                .position(|level| level.name == *name || get_level_slug(&level.name) == *name)
                .ok_or_else(|| {
                    let known: Vec<String> = levels
                        .iter()
                        .map(|level| get_level_slug(&level.name))
                        .collect();
                    format!(
                        "--tour-level {name:?} is no level of the manifest; the levels are {}",
                        known.join(", ")
                    )
                    .into()
                })
        })
        .collect()
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

/// `run_window` opens the app's window and runs it until the player quits: the menus, and each
/// round's level played in the same window through its library (`level_library`). With
/// `--tour-levels` it tours the manifest's levels `tour_level_indices` and quits.
///
/// Example: `run_window(&arguments, app, vec![0, 1, 2, 3, 4])?;`
fn run_window(arguments: &Arguments, mut app: App, tour_level_indices: Vec<usize>) -> Result<()> {
    let mut window: MetalWindow = MetalWindow::Open(
        WINDOW_TITLE,
        (arguments.width_px, arguments.height_px),
        WindowOptions::default(),
    )?;
    let mut painter: Painter = Painter::New(window.get_device())?;
    let mut shown: Option<(Vec<Shape>, (u32, u32))> = None;
    let mut tour_run: Option<TourRun> = match arguments.tour_levels {
        true => Some(TourRun::Start(
            &mut app,
            get_tour_game_kind(
                arguments.tour_ally_bot_count,
                arguments.tour_enemy_bot_count,
            ),
            tour_level_indices,
            window.get_window().size_in_pixels(),
        )?),
        false => None,
    };
    let started_at: Instant = Instant::now();
    while !app.should_quit() {
        if arguments
            .exit_after_s
            .is_some_and(|seconds| started_at.elapsed().as_secs_f64() >= seconds)
        {
            println!("warped: closing after {} s", started_at.elapsed().as_secs());
            return Ok(());
        }
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
                    clicks,
                    ..
                } => {
                    let (width, height) = window.get_window().size();
                    app.click([x, y], [width as f32, height as f32], clicks);
                }
                _ => {}
            }
            if let Some(input) = get_menu_input(&event, app.is_rebinding()) {
                app.handle(input);
            }
        }
        if let Some(tour_run) = &tour_run {
            tour_run.before_frame(&mut app);
        }
        app.update();
        if let Some((play, join)) = app.take_level_to_play() {
            let mut tour: Option<Tour> = tour_run.as_ref().map(TourRun::make_level_tour);
            let result: Result<LevelEnd> = autoreleasepool(|_| {
                let mut link: AppLink = AppLink::New(&mut app, tour.as_mut());
                let mut level_window: LevelWindow = window.get_level_window(&mut link);
                play(&mut level_window, &join)
            });
            app.end_level(result.map_err(|error| error.to_string()));
            if let (Some(tour_run), Some(tour)) = (&mut tour_run, &tour) {
                tour_run.after_level(&app, &join.level.name, tour)?;
                if tour_run.is_done() {
                    tour_run.finish(&mut app)?;
                    return Ok(());
                }
            }
            shown = None;
            continue;
        }
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

/// `make_sample_settings_page` gives the game master's settings page over the levels' own
/// settings files in this checkout (each `<folder>/server_settings.yaml`, none for a level without
/// one), with a shared and a level's change, for the screen pictures.
///
/// Example: `let page = make_sample_settings_page(&levels)?;`
fn make_sample_settings_page(levels: &[LevelEntry]) -> Result<GameSettingsPage> {
    let repository: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join(REPOSITORY_FROM_CRATE);
    let mut defaults: Vec<Option<serde_json::Value>> = Vec::new();
    for level in levels {
        let path: PathBuf = repository.join(&level.folder).join(SETTINGS_FILE_NAME);
        defaults.push(match path.exists() {
            true => Some(read_settings_file(&path)?),
            false => None,
        });
    }
    let mut settings: GameSettings = GameSettings::New(defaults);
    settings.apply(&SettingsMessage::Set {
        scope: SettingsScope::Shared,
        key: SAMPLE_SHARED_KEY.to_string(),
        value: SAMPLE_SHARED_VALUE,
    })?;
    let names: Vec<String> = levels.iter().map(|level| level.name.clone()).collect();
    let mut page: GameSettingsPage = GameSettingsPage::New(names);
    page.update(&settings);
    Ok(page)
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
    let game_settings_page: GameSettingsPage = make_sample_settings_page(levels)?;
    let mut game_settings_end_page: GameSettingsPage = make_sample_settings_page(levels)?;
    game_settings_end_page.handle(MenuInput::Up);
    let screens: Vec<(&str, page_t)> = vec![
        ("main_menu", main_menu.get_shapes(&main_rows, levels, None)),
        ("join_by_address", get_address_shapes("192.168.1.", None)),
        ("player_settings", settings_page.get_shapes()),
        ("game_settings", game_settings_page.get_shapes()),
        ("game_settings_levels", game_settings_end_page.get_shapes()),
        (
            "game_settings_waiting",
            GameSettingsPage::New(Vec::new()).get_shapes(),
        ),
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
    if arguments.print_level_programs {
        return print_level_programs(&levels);
    }
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
        is_loopback_only: arguments.loopback_only || network_scope::is_loopback_only(),
    };
    let tour_level_indices: Vec<usize> = get_tour_level_indices(&levels, &arguments.tour_level)?;
    let app: App = App::New(
        levels,
        &manifest,
        pictures,
        player_settings,
        &player_settings_path,
        ports,
    );
    run_window(&arguments, app, tour_level_indices)
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
    //! | Toured levels | none named (all), a name, a slug, a name no level has | `toured_levels_are_picked_by_name` |
    //! | Toured game | no bots (alone), allies only, enemies only, both; a count over the most | `a_tour_plays_alone_or_with_bots` |
    //! | Shipped settings files | every level's file on the game master's page: shared respawn, shooting switches and gravity, each level's own setting (City Sphere's glass and its team domes' size), Dark Forest's none | `the_shipped_settings_all_show` |

    use super::*;

    #[test]
    fn toured_levels_are_picked_by_name() {
        let levels: Vec<LevelEntry> = load_levels(&get_levels_manifest_path()).unwrap();
        let all: Vec<usize> = get_tour_level_indices(&levels, &[]).unwrap();
        assert_eq!(all, (0..levels.len()).collect::<Vec<usize>>());
        let last: usize = levels.len() - 1;
        let names: [String; 2] = [levels[last].name.clone(), get_level_slug(&levels[0].name)];
        assert_eq!(get_tour_level_indices(&levels, &names).unwrap(), [last, 0]);
        let error: String = get_tour_level_indices(&levels, &["no such level".to_string()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("no such level"), "{error}");
    }

    #[test]
    fn a_tour_plays_alone_or_with_bots() {
        assert_eq!(get_tour_game_kind(0, 0), GameKind::Alone);
        for (ally_count, enemy_count) in [(2, 0), (0, 3), (4, 5)] {
            assert_eq!(
                get_tour_game_kind(ally_count, enemy_count),
                GameKind::Bots {
                    ally_count,
                    enemy_count,
                    objective: BotObjective::Fight
                }
            );
        }
        let too_many: String = (MAX_ENEMY_COUNT + 1).to_string();
        let flags: [&str; 4] = [
            "warped",
            "--tour-levels",
            "--tour-enemy-bot-count",
            &too_many,
        ];
        assert!(Arguments::try_parse_from(flags).is_err());
        assert!(Arguments::try_parse_from(["warped", "--tour-ally-bot-count", "1"]).is_err());
    }

    #[test]
    fn the_shipped_settings_all_show() {
        let levels: Vec<LevelEntry> = load_levels(&get_levels_manifest_path()).unwrap();
        let page: GameSettingsPage = make_sample_settings_page(&levels).unwrap();
        let keys: &[String] = page.get_row_keys();
        for key in [
            "shared.respawn.start_s",
            "shared.respawn.step_s",
            "shared.respawn.period_s",
            "shared.shooting.can_shoot_while_jumping",
            "shared.shooting.is_friendly_fire_on",
            "shared.shooting.drop_m_per_s_per_s",
            "shared.gravity.reach_m",
            "shared.movement.walk_speed_per_map_speed",
        ] {
            assert!(keys.contains(&key.to_string()), "{key} in {keys:?}");
        }
        let own_keys = |name: &str| -> Vec<&String> {
            let level: usize = levels.iter().position(|level| level.name == name).unwrap();
            let prefix: String = format!("level_{level}.");
            keys.iter().filter(|key| key.starts_with(&prefix)).collect()
        };
        assert_eq!(
            own_keys("Lunar Surface"),
            ["level_4.surface.gravity_per_jump_gravity"]
        );
        assert_eq!(own_keys("Dust"), ["level_0.sandstorm.wind_per_map_wind"]);
        assert_eq!(
            own_keys("Snow Planet"),
            ["level_1.blizzard.radius_per_map_radius"]
        );
        let city_keys: Vec<&String> = own_keys("City Sphere");
        assert_eq!(city_keys.len(), 4);
        assert!(
            city_keys
                .iter()
                .any(|key| key.ends_with(".team_domes.circumference_frac")),
            "{city_keys:?}"
        );
        assert!(own_keys("Dark Forest").is_empty());
    }

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
