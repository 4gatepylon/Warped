//! `app.rs` is the interface between the one-window app (`../warped_mvp_rust/`, binary `warped`)
//! and a level: what a level gets when the app opens it (the app's window and its Metal layer,
//! the level server to join, the team, the player's settings file, and a way to hear that the
//! match's round is over), and what it gives back (how it ended). Each level crate's `lib.rs`
//! exposes one function of type `play_level_fn_t`; its `main.rs` stays a thin wrapper that runs
//! the level alone.
//!
//! Interface: `LevelJoin`, `LevelEnd`, `MatchLink` (`is_round_over`, `get_match_view`,
//! `get_match_lines`), and on macOS `LevelWindow` and `play_level_fn_t`.
//!
//! Example: `let end: LevelEnd = (level.play)(&mut level_window, &join)?;`
//!
//! Must know:
//! - Gotchas: a level must call `match_link.is_round_over()` once a frame and return
//!   `LevelEnd::RoundOver` soon after it says true; the app keeps saying hello to the match
//!   server inside that call, so a level that stops calling it drops its player from the match.
//! - Gotchas: the level draws only into `layer` and takes events only from `event_pump`; it opens
//!   no window of its own, and leaves the window as it found it (mouse released, title) when it
//!   returns.
//! - Gotchas: the level reads the player's settings from `player_settings_path` when it starts
//!   and saves there from its own menu; the app reads the file again after the level returns.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::match_protocol::{LevelEntry, MatchView};
use crate::match_state::Team;

/// `LevelJoin` is what the app tells a level to play: its entry in the levels manifest (name,
/// `--map`, extra client flags), the level server's address, the player's team, and the
/// player's settings file.
#[derive(Clone, PartialEq, Debug)]
pub struct LevelJoin {
    pub level: LevelEntry,
    pub server: SocketAddr,
    pub team: Team,
    pub player_settings_path: PathBuf,
}

/// `LevelEnd` is how a level ended: the match's round ended (the app's `MatchLink` said so),
/// with the winner the level saw (`None` if it saw none, e.g. the match timed out); or the
/// in-level menu's `PlayOutcome` (`menu_and_map.rs`) other than `Play`: the player left the level
/// for the game menu (`LeaveLevel`), or closed the window, quitting the app (`CloseWindow`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LevelEnd {
    RoundOver { winner: Option<Team> },
    LeaveLevel,
    CloseWindow,
}

/// `MatchLink` is the app's side of a level's frame loop: the level calls `is_round_over` once a
/// frame, and the app polls the match server there and says whether the round has ended; the
/// level's in-level menu shows the match's status on its MATCH page
/// (`Menu::set_match_lines(window.match_link.get_match_lines())`): the level played now, the
/// wavefronts, the round, who attacks and defends, free play's wins, players per team, from the
/// shared `match_menu::get_match_lines`.
pub trait MatchLink {
    /// `is_round_over` keeps the app's match client alive and says whether the round the level
    /// plays has ended, so the level should return `LevelEnd::RoundOver`.
    ///
    /// Example: `if window.match_link.is_round_over() { return Ok(LevelEnd::RoundOver { winner }); }`
    fn is_round_over(&mut self) -> bool;

    /// `get_match_view` gives the match's latest view from its server (`None` before the first
    /// one).
    ///
    /// Example: `if let Some(view) = window.match_link.get_match_view() { ... }`
    fn get_match_view(&self) -> Option<&MatchView>;

    /// `get_match_lines` gives the in-level menu's MATCH page for this player
    /// (`match_menu::get_match_lines`), or `None` before the server's first view.
    ///
    /// Example: `menu.set_match_lines(window.match_link.get_match_lines());`
    fn get_match_lines(&self) -> Option<Vec<(String, bool)>>;
}

/// `LevelWindow` is the app's window as a level gets it: SDL, the window (made with
/// `SDL_WINDOW_METAL`), its event pump, the window's `CAMetalLayer` (pixel format
/// `BGRA8Unorm`, not framebuffer-only) and its Metal device, and the app's `MatchLink`.
#[cfg(target_os = "macos")]
pub struct LevelWindow<'a> {
    pub sdl: &'a sdl3::Sdl,
    pub window: &'a sdl3::video::Window,
    pub event_pump: &'a mut sdl3::EventPump,
    pub layer: &'a objc2_quartz_core::CAMetalLayer,
    pub device: &'a objc2::runtime::ProtocolObject<dyn objc2_metal::MTLDevice>,
    pub match_link: &'a mut dyn MatchLink,
}

/// `play_level_fn_t` is a level's entry point: it plays `join` in the app's window until the round
/// ends, the player leaves or quits, and says which; it fails only on what the level can't recover
/// from (the app shows the error and goes back to the game menu).
#[cfg(target_os = "macos")]
#[allow(non_camel_case_types)]
pub type play_level_fn_t =
    fn(&mut LevelWindow, &LevelJoin) -> Result<LevelEnd, Box<dyn std::error::Error>>;

#[cfg(test)]
mod tests {
    //! Unit tests for `app.rs`: the interface is types, so the test checks that a level's entry
    //! point of the documented shape is a `play_level_fn_t` and can tell the app each ending.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Ending | round over, left to the game menu, quit | `a_level_function_fits_the_interface` |

    use super::*;

    /// `WatchAfter` is a `MatchLink` whose round ends after `frame_count` frames.
    struct WatchAfter {
        frame_count: u32,
    }

    impl MatchLink for WatchAfter {
        /// `is_round_over` counts a frame and says whether the round is over.
        ///
        /// Example: `watch.is_round_over();`
        fn is_round_over(&mut self) -> bool {
            self.frame_count = self.frame_count.saturating_sub(1);
            self.frame_count == 0
        }

        /// `get_match_view` has no match to show.
        ///
        /// Example: `assert!(watch.get_match_view().is_none());`
        fn get_match_view(&self) -> Option<&MatchView> {
            None
        }

        /// `get_match_lines` has no match to show.
        ///
        /// Example: `assert!(watch.get_match_lines().is_none());`
        fn get_match_lines(&self) -> Option<Vec<(String, bool)>> {
            None
        }
    }

    /// `play_frames` stands in for a level's loop: it ends with the round, or as `join`'s team
    /// says (blue leaves, red quits) after one frame.
    ///
    /// Example: `let end = play_frames(&mut watch, Team::Blue);`
    fn play_frames(watch: &mut dyn MatchLink, team: Team) -> LevelEnd {
        if watch.is_round_over() {
            return LevelEnd::RoundOver { winner: Some(team) };
        }
        match team {
            Team::Blue => LevelEnd::LeaveLevel,
            Team::Red => LevelEnd::CloseWindow,
        }
    }

    #[test]
    fn a_level_function_fits_the_interface() {
        assert_eq!(
            play_frames(&mut WatchAfter { frame_count: 1 }, Team::Blue),
            LevelEnd::RoundOver {
                winner: Some(Team::Blue)
            }
        );
        assert_eq!(
            play_frames(&mut WatchAfter { frame_count: 5 }, Team::Blue),
            LevelEnd::LeaveLevel
        );
        assert_eq!(
            play_frames(&mut WatchAfter { frame_count: 5 }, Team::Red),
            LevelEnd::CloseWindow
        );
        #[cfg(target_os = "macos")]
        {
            fn play_nothing(
                _: &mut LevelWindow,
                _: &LevelJoin,
            ) -> Result<LevelEnd, Box<dyn std::error::Error>> {
                Ok(LevelEnd::LeaveLevel)
            }
            let entry: play_level_fn_t = play_nothing;
            let _ = entry;
        }
    }
}
