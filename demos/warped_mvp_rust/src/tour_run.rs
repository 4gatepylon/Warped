//! `tour_run.rs` drives `warped --tour-levels`: it plays alone or with bots and asks the match
//! server for each toured level in turn (free play), lets each run its `Tour` in the app's window (settle, time
//! its frames, open the menu, leave), saves the game menu after each level, and writes every
//! level's frame times to its run folder before quitting.
//!
//! Interface: `TourRun` (`Start`, `before_frame`, `make_level_tour`, `after_level`, `is_done`,
//! `finish`).
//!
//! Example: `let mut tour_run = TourRun::Start(&mut app, vec![0, 1, 2], size_px)?;`

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use warped_shared::app_bundle::get_level_slug;

use crate::app_state::{App, format_utc_now};
use crate::children::{GameKind, make_run_folder};
use crate::level_link::{FrameSummary, Tour, summarise_frame_times};
use crate::painter::{PixelOrder, render_page};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SETTLE_S: f64 = 10.0;
const TIME_S: f64 = 8.0;
const STEP_S: f64 = 1.2;
const TARGET_FRAME_MS: f64 = 1000.0 / 60.0;

/// `TourRun` is a tour of levels: the game it plays, the levels' indices in the manifest, how many
/// are done, the frame summaries so far (level name, summary), the window's size in pixels, and
/// the run folder.
pub struct TourRun {
    kind: GameKind,
    level_indices: Vec<usize>,
    done_count: usize,
    summaries: Vec<(String, Option<FrameSummary>)>,
    size_px: (u32, u32),
    folder: PathBuf,
}

impl TourRun {
    /// `Start` hosts a game of `kind` (alone or with bots) on the first toured level, for a tour
    /// of the manifest's levels `level_indices` (at least one) in a window of `size_px`.
    ///
    /// Example: `let tour_run = TourRun::Start(&mut app, GameKind::Alone, vec![0, 3], (2560, 1440))?;`
    #[allow(non_snake_case)]
    pub fn Start(
        app: &mut App,
        kind: GameKind,
        level_indices: Vec<usize>,
        size_px: (u32, u32),
    ) -> Result<TourRun> {
        let first_level: usize = *level_indices.first().ok_or("a tour needs a level")?;
        let folder: PathBuf = make_run_folder("tour_levels", &format_utc_now())?;
        app.start_game(kind, Some(first_level))?;
        println!("tour: run folder {}", folder.display());
        Ok(TourRun {
            kind,
            level_indices,
            done_count: 0,
            summaries: Vec::new(),
            size_px,
            folder,
        })
    }

    /// `before_frame` asks the match server for the next level while the game menu shows.
    ///
    /// Example: `tour_run.before_frame(&mut app);`
    pub fn before_frame(&self, app: &mut App) {
        if app.is_in_game_menu() && !self.is_done() {
            app.request_play(Some(self.level_indices[self.done_count]));
        }
    }

    /// `make_level_tour` gives the script of the level about to play.
    ///
    /// Example: `let mut tour = tour_run.make_level_tour();`
    pub fn make_level_tour(&self) -> Tour {
        Tour::New(SETTLE_S, TIME_S, STEP_S)
    }

    /// `after_level` records `level_name`'s frame times from `tour`, saves the game menu as it
    /// shows after the level, and moves to the next level.
    ///
    /// Example: `tour_run.after_level(&app, "Dust", &tour)?;`
    pub fn after_level(&mut self, app: &App, level_name: &str, tour: &Tour) -> Result<()> {
        let summary: Option<FrameSummary> = summarise_frame_times(tour.get_frame_times());
        println!("tour: {level_name}: {summary:?}");
        self.summaries.push((level_name.to_string(), summary));
        let (shapes, page_size, pictures) = app.get_page();
        let pixels: Vec<u8> = render_page(
            self.size_px,
            page_size,
            &shapes,
            &pictures,
            PixelOrder::Rgba,
        )?;
        let path: PathBuf = self.folder.join(format!(
            "game_menu_after_{}.png",
            get_level_slug(level_name)
        ));
        image::save_buffer(
            &path,
            &pixels,
            self.size_px.0,
            self.size_px.1,
            image::ExtendedColorType::Rgba8,
        )?;
        self.done_count += 1;
        Ok(())
    }

    /// `is_done` says whether every toured level has been visited.
    ///
    /// Example: `if tour_run.is_done() { ... }`
    pub fn is_done(&self) -> bool {
        self.done_count >= self.level_indices.len()
    }

    /// `finish` writes `frame_times.md`, a table of each level's frame times, prints it, and
    /// stops the game `app` hosts; it fails if a process the game started is still alive then.
    ///
    /// Example: `tour_run.finish(&mut app)?;`
    pub fn finish(&self, app: &mut App) -> Result<()> {
        let mut table: String = format!(
            "Frame times inside the app at {}x{} px, {} (wall clock, from one frame's start to the next's)\n\n| Level | Frames | Median ms | p99 ms | Max ms | p99 under 16.7 ms |\n| --- | --- | --- | --- | --- | --- |\n",
            self.size_px.0,
            self.size_px.1,
            describe_game(self.kind)
        );
        for (name, summary) in &self.summaries {
            match summary {
                Some(s) => writeln!(
                    table,
                    "| {name} | {} | {:.2} | {:.2} | {:.2} | {} |",
                    s.frame_count,
                    s.median_ms,
                    s.p99_ms,
                    s.max_ms,
                    if s.p99_ms < TARGET_FRAME_MS {
                        "yes"
                    } else {
                        "NO"
                    }
                )?,
                None => writeln!(table, "| {name} | 0 | - | - | - | no frames |")?,
            }
        }
        fs::write(self.folder.join("frame_times.md"), &table)?;
        println!("{table}");
        app.stop_hosting()?;
        println!("tour: every process the game started has stopped");
        Ok(())
    }
}

/// `describe_game` says in words which game a tour played, for the frame times' heading.
///
/// Example: `assert_eq!(describe_game(GameKind::Alone), "playing alone");`
fn describe_game(kind: GameKind) -> String {
    match kind {
        GameKind::Host => "hosting a game".to_string(),
        GameKind::Alone => "playing alone".to_string(),
        GameKind::Bots {
            ally_count,
            enemy_count,
            ..
        } => format!("with {ally_count} allied and {enemy_count} enemy bots"),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `tour_run.rs` (a tour itself needs a window and the levels; `warped
    //! --tour-levels` runs one).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Game described | hosted, alone, with bots | `a_tour_names_its_game` |

    use super::*;
    use crate::children::BotObjective;

    #[test]
    fn a_tour_names_its_game() {
        assert_eq!(describe_game(GameKind::Host), "hosting a game");
        assert_eq!(describe_game(GameKind::Alone), "playing alone");
        let bots: GameKind = GameKind::Bots {
            ally_count: 4,
            enemy_count: 5,
            objective: BotObjective::Fight,
        };
        assert_eq!(describe_game(bots), "with 4 allied and 5 enemy bots");
    }
}
