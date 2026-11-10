//! `level_loop.rs` is the part of a level's frame loop that every level shares, whether the
//! one-window app lends it its window (`app::LevelWindow`) or the level runs alone in a
//! `MetalWindow` of its own: the order of a frame (the match's news, the window's events, the
//! level's update, the loading screen's steps, the frame itself), how the loop ends
//! (`app::LevelEnd`), and the command line a level's entry point plays a `LevelJoin` with. The
//! level supplies what differs (`LevelFrames`): its session, its renderer, its loading steps.
//!
//! Interface: `LevelFrames`, `play_frames`, `get_level_end`, `make_level_command_line`,
//! `get_team_flag`.
//!
//! Example: `let end: LevelEnd = play_frames(&mut level_window, &mut frames, false)?;`
//!
//! Must know:
//! - Gotchas: the loop asks the match link once a frame, before the window's events, so a level
//!   in the app never stops saying hello to the match server (`app::MatchLink`).
//! - Gotchas: a level run alone shows its HUD in the window's title (`is_hud_in_title`); in the
//!   app the title stays the app's.

use std::error::Error;
use std::ffi::OsString;

use objc2_core_foundation::CGSize;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::KeyboardState;
use sdl3::mouse::{MouseState, MouseUtil};
use sdl3::video::Window;

use crate::app::{LevelEnd, LevelJoin, LevelWindow};
use crate::match_state::Team;
use crate::menu_and_map::PlayOutcome;
use crate::metal_window::set_window_title;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `LEVEL_PROGRAM_NAME` stands for the program's name in a level's command line made from a
/// `LevelJoin` (clap reads and ignores it).
const LEVEL_PROGRAM_NAME: &str = "level";

/// `LevelFrames` is a level's side of the frame loop: its session and renderer, as the loop asks
/// them for each step of a frame.
pub trait LevelFrames {
    /// `handle_event` reacts to one window event and says whether to play on, leave the level or
    /// close the window.
    ///
    /// Example: `if frames.handle_event(event, window, &mouse)? != PlayOutcome::Play { ... }`
    fn handle_event(
        &mut self,
        event: Event,
        window: &Window,
        mouse: &MouseUtil,
    ) -> Result<PlayOutcome>;

    /// `is_finished` says whether the level ended by itself (a timed run, a scripted route).
    ///
    /// Example: `if frames.is_finished() { ... }`
    fn is_finished(&self) -> bool;

    /// `update` advances the level by the time since the last frame, with the keys and mouse
    /// buttons held.
    ///
    /// Example: `frames.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;`
    fn update(&mut self, keyboard: &KeyboardState, mouse: &MouseState) -> Result<()>;

    /// `get_loading_frame_count` gives how many frames show the loading screen before play's
    /// first frame (0: the level loaded before its loop).
    ///
    /// Example: `let loading_frame_count = frames.get_loading_frame_count();`
    fn get_loading_frame_count(&self) -> usize;

    /// `load_step` shows the loading screen's frame `frame_index` and loads what that frame
    /// stands for (a step of the level's GPU resources, or what waits for loading, such as
    /// joining a team).
    ///
    /// Example: `frames.load_step(layer, 0)?;`
    fn load_step(&mut self, layer: &CAMetalLayer, frame_index: usize) -> Result<()>;

    /// `draw_frame` draws one frame into `layer` at `window_px`, and gives the window's title
    /// with the HUD's line when it is due.
    ///
    /// Example: `let hud = frames.draw_frame(layer, (2560, 1440))?;`
    fn draw_frame(&mut self, layer: &CAMetalLayer, window_px: (u32, u32))
    -> Result<Option<String>>;

    /// `set_match_lines` gives the open menu's MATCH page its lines (`None`: no MATCH page).
    ///
    /// Example: `frames.set_match_lines(link.get_match_lines());`
    fn set_match_lines(&mut self, lines: Option<Vec<(String, bool)>>);

    /// `get_winner` gives the team the level's server says won, if any.
    ///
    /// Example: `let winner = frames.get_winner();`
    fn get_winner(&self) -> Option<Team>;
}

/// `get_level_end` gives how a window event's `outcome` ends the level, or `None` to play on.
///
/// Example: `assert_eq!(get_level_end(PlayOutcome::LeaveLevel), Some(LevelEnd::LeaveLevel));`
pub fn get_level_end(outcome: PlayOutcome) -> Option<LevelEnd> {
    match outcome {
        PlayOutcome::Play => None,
        PlayOutcome::LeaveLevel => Some(LevelEnd::LeaveLevel),
        PlayOutcome::CloseWindow => Some(LevelEnd::CloseWindow),
    }
}

/// `play_frames` runs `frames` in `window` until the match's round ends, the player leaves or
/// closes the window, or the level finishes by itself (which closes the window), then frees the
/// mouse. With `is_hud_in_title` (a level run alone), the window's title shows the HUD's line.
///
/// Example: `let end = play_frames(&mut level_window, &mut frames, true)?;`
pub fn play_frames(
    window: &mut LevelWindow,
    frames: &mut dyn LevelFrames,
    is_hud_in_title: bool,
) -> Result<LevelEnd> {
    let mouse: MouseUtil = window.sdl.mouse();
    let loading_frame_count: usize = frames.get_loading_frame_count();
    let mut loading_frame_index: usize = 0;
    let result: Result<LevelEnd> = (|| loop {
        if window.match_link.is_round_over() {
            return Ok(LevelEnd::RoundOver {
                winner: frames.get_winner(),
            });
        }
        frames.set_match_lines(window.match_link.get_match_lines());
        for event in window.event_pump.poll_iter() {
            if let Some(end) = get_level_end(frames.handle_event(event, window.window, &mouse)?) {
                return Ok(end);
            }
        }
        if frames.is_finished() {
            return Ok(LevelEnd::CloseWindow);
        }
        frames.update(
            &window.event_pump.keyboard_state(),
            &window.event_pump.mouse_state(),
        )?;
        let window_px: (u32, u32) = window.window.size_in_pixels();
        window.layer.setDrawableSize(CGSize {
            width: f64::from(window_px.0),
            height: f64::from(window_px.1),
        });
        if loading_frame_index < loading_frame_count {
            frames.load_step(window.layer, loading_frame_index)?;
            loading_frame_index += 1;
            continue;
        }
        let title: Option<String> = frames.draw_frame(window.layer, window_px)?;
        if let Some(title) = title
            && is_hud_in_title
        {
            set_window_title(window.window, &title)?;
        }
    })();
    mouse.set_relative_mouse_mode(window.window, false);
    result
}

/// `get_team_flag` gives `team` as a level's `--team` takes it.
///
/// Example: `assert_eq!(get_team_flag(Team::Red), "red");`
pub fn get_team_flag(team: Team) -> &'static str {
    match team {
        Team::Blue => "blue",
        Team::Red => "red",
    }
}

/// `make_level_command_line` gives the command line a level's entry point parses to play `join`:
/// join its server at once on its team, with the player's settings file if it exists yet (a new
/// player has none, and a level then uses the built-in settings, as the app does), on the level's
/// map (`--map`, if the level has one), with the level's own client flags last.
///
/// Example: `let arguments = Arguments::try_parse_from(make_level_command_line(&join))?;`
pub fn make_level_command_line(join: &LevelJoin) -> Vec<OsString> {
    let mut command_line: Vec<OsString> = vec![
        LEVEL_PROGRAM_NAME.into(),
        "--join".into(),
        join.server.to_string().into(),
        "--team".into(),
        get_team_flag(join.team).into(),
    ];
    if join.player_settings_path.exists() {
        command_line.extend([
            "--player-settings".into(),
            join.player_settings_path.clone().into(),
        ]);
    }
    if let Some(map) = &join.level.map {
        command_line.extend(["--map".into(), map.into()]);
    }
    command_line.extend(join.level.client_args.iter().map(OsString::from));
    command_line
}

#[cfg(test)]
mod tests {
    //! Unit tests for `level_loop.rs`: the loop itself needs a window, so the tests cover how an
    //! event's outcome ends the level and the command line a `LevelJoin` makes.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Event outcome | play on, leave the level (the QUIT page), close the window | `each_outcome_ends_the_level_as_the_app_expects` |
    //! | Team | blue, red | `a_join_makes_its_command_line` |
    //! | Level's map | given, none | `a_join_makes_its_command_line` |
    //! | Level's client flags | none, some (after the rest) | `a_join_makes_its_command_line` |
    //! | Player's settings file | exists, not yet | `a_join_makes_its_command_line` |

    use super::*;
    use crate::match_protocol::LevelEntry;

    const EXISTING_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

    #[test]
    fn each_outcome_ends_the_level_as_the_app_expects() {
        assert_eq!(get_level_end(PlayOutcome::Play), None);
        assert_eq!(
            get_level_end(PlayOutcome::LeaveLevel),
            Some(LevelEnd::LeaveLevel)
        );
        assert_eq!(
            get_level_end(PlayOutcome::CloseWindow),
            Some(LevelEnd::CloseWindow)
        );
    }

    #[test]
    fn a_join_makes_its_command_line() {
        let level: LevelEntry = LevelEntry {
            name: "Dust".to_string(),
            folder: "demos/dust_rust".to_string(),
            branch: String::new(),
            status: String::new(),
            program: String::new(),
            map: Some("dust".to_string()),
            server_args: Vec::new(),
            client_args: Vec::new(),
            stand_in_for: None,
            takes_bot_label: true,
            map_picture: None,
            server_settings: None,
        };
        let mut join: LevelJoin = LevelJoin {
            level,
            server: "192.168.1.5:47852".parse().expect("an address"),
            team: Team::Blue,
            player_settings_path: EXISTING_FILE.into(),
        };
        let as_text = |join: &LevelJoin| -> Vec<String> {
            make_level_command_line(join)
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            as_text(&join),
            [
                "level",
                "--join",
                "192.168.1.5:47852",
                "--team",
                "blue",
                "--player-settings",
                EXISTING_FILE,
                "--map",
                "dust",
            ]
        );
        join.team = Team::Red;
        join.level.map = None;
        join.level.client_args = vec!["--no-vsync".to_string()];
        join.player_settings_path = format!("{EXISTING_FILE}.missing").into();
        assert_eq!(
            as_text(&join),
            [
                "level",
                "--join",
                "192.168.1.5:47852",
                "--team",
                "red",
                "--no-vsync",
            ]
        );
    }
}
