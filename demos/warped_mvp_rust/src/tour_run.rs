//! `tour_run.rs` drives `warped --tour-levels`: it plays alone or with bots and asks the match
//! server for each toured level in turn (free play), lets each run its `Tour` in the app's window (settle, time
//! its frames, open the menu, leave), saves the map menu after each level, writes every
//! level's frame times to its run folder, and opens the game master's settings page to check that
//! the match server's settings show there, before quitting.
//!
//! Interface: `TourRun` (`Start`, `before_frame`, `make_level_tour`, `after_level`, `is_done`,
//! `finish`).
//!
//! Example: `let mut tour_run = TourRun::Start(&mut app, vec![0, 1, 2], size_px)?;`

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use warped_shared::app_bundle::get_level_slug;

use crate::app_state::{App, format_utc_now};
use crate::children::{GameKind, describe_game_kind, make_run_folder};
use crate::level_link::{
    FRAME_TABLE_HEADER, FrameSummary, Tour, format_frame_row, summarise_frame_times,
};
use crate::menu_page::MenuInput;
use crate::painter::{PixelOrder, render_page};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SETTLE_S: f64 = 10.0;
const TIME_S: f64 = 8.0;
const STEP_S: f64 = 1.2;
const SETTINGS_ANSWER_TIMEOUT_S: f64 = 5.0;
const SETTINGS_POLL_MS: u64 = 20;
const SETTINGS_PICTURE_NAME: &str = "game_settings.png";
const SAVED_PICTURE_NAME: &str = "game_settings_saved.png";
const REFUSED_PICTURE_NAME: &str = "game_settings_refused.png";
const SAVED_SETTING_ROW: &str = "shared.respawn.start_s";
const REFUSED_SETTING_ROW: &str = "shared.beam_speckle.black_frac";
const LOWERED_SHARE_ROW: &str = "shared.beam_speckle.dropout_frac";
const SHARE_STEP_COUNT: usize = 7;

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

    /// `before_frame` asks the match server for the next level while the map menu shows.
    ///
    /// Example: `tour_run.before_frame(&mut app);`
    pub fn before_frame(&self, app: &mut App) {
        if app.is_in_map_menu() && !self.is_done() {
            app.request_play(Some(self.level_indices[self.done_count]));
        }
    }

    /// `make_level_tour` gives the script of the level about to play.
    ///
    /// Example: `let mut tour = tour_run.make_level_tour();`
    pub fn make_level_tour(&self) -> Tour {
        Tour::New(SETTLE_S, TIME_S, STEP_S)
    }

    /// `after_level` records `level_name`'s frame times from `tour`, saves the map menu as it
    /// shows after the level, and moves to the next level.
    ///
    /// Example: `tour_run.after_level(&app, "Dust", &tour)?;`
    pub fn after_level(&mut self, app: &App, level_name: &str, tour: &Tour) -> Result<()> {
        let summary: Option<FrameSummary> = summarise_frame_times(tour.get_frame_times());
        println!("tour: {level_name}: {summary:?}");
        self.summaries.push((level_name.to_string(), summary));
        self.save_page(
            app,
            &format!("map_menu_after_{}.png", get_level_slug(level_name)),
        )?;
        self.done_count += 1;
        Ok(())
    }

    /// `save_page` saves the page `app` shows, at the window's size, as `file_name` in the run
    /// folder.
    ///
    /// Example: `self.save_page(app, "game_settings.png")?;`
    fn save_page(&self, app: &App, file_name: &str) -> Result<()> {
        let (shapes, page_size, pictures) = app.get_page();
        let pixels: Vec<u8> = render_page(
            self.size_px,
            page_size,
            &shapes,
            &pictures,
            PixelOrder::Rgba,
        )?;
        image::save_buffer(
            self.folder.join(file_name),
            &pixels,
            self.size_px.0,
            self.size_px.1,
            image::ExtendedColorType::Rgba8,
        )?;
        Ok(())
    }

    /// `check_game_settings` opens the game master's settings page on the game the tour hosts,
    /// waits for the match server's settings to show (`game_settings.png`), raises one setting
    /// and saves it, then raises one of the beam speckle's shares and lowers another by as much
    /// and saves that, each time waiting for the match server to take it
    /// (`game_settings_saved.png`), then raises a share again and saves, which the page must refuse
    /// (`game_settings_refused.png`), and leaves. It fails if the settings don't show or the
    /// save isn't taken within `SETTINGS_ANSWER_TIMEOUT_S` (a match server that stopped never
    /// answers), or if the bad save isn't refused.
    ///
    /// Example: `self.check_game_settings(app)?;`
    fn check_game_settings(&self, app: &mut App) -> Result<()> {
        app.open_game_settings();
        let deadline: Instant = Instant::now() + Duration::from_secs_f64(SETTINGS_ANSWER_TIMEOUT_S);
        while !app.has_game_settings() {
            if Instant::now() >= deadline {
                return Err(format!(
                    "tour: the game settings page didn't get the match server's settings within {SETTINGS_ANSWER_TIMEOUT_S} s"
                )
                .into());
            }
            app.update();
            std::thread::sleep(Duration::from_millis(SETTINGS_POLL_MS));
        }
        self.save_page(app, SETTINGS_PICTURE_NAME)?;
        println!("tour: the game settings page shows the match server's settings");
        app.step_game_setting(SAVED_SETTING_ROW, true)?;
        self.save_and_wait(app, SAVED_SETTING_ROW)?;
        for _ in 0..SHARE_STEP_COUNT {
            app.step_game_setting(REFUSED_SETTING_ROW, true)?;
            app.step_game_setting(LOWERED_SHARE_ROW, false)?;
        }
        self.save_and_wait(app, "the beam speckle's shares, one raised and one lowered")?;
        self.save_page(app, SAVED_PICTURE_NAME)?;
        app.step_game_setting(REFUSED_SETTING_ROW, true)?;
        app.save_game_settings();
        if !app.has_unsaved_game_settings() {
            return Err(format!(
                "tour: a save with {REFUSED_SETTING_ROW} raised (shares no longer adding up to 1) wasn't refused"
            )
            .into());
        }
        self.save_page(app, REFUSED_PICTURE_NAME)?;
        println!("tour: a save that breaks a rule was refused on the page");
        app.handle(MenuInput::Back);
        app.handle(MenuInput::Back);
        Ok(())
    }

    /// `save_and_wait` saves the game settings page's changes (`what` names them) and waits for
    /// the match server to take them; it fails if the page still has unsaved changes after
    /// `SETTINGS_ANSWER_TIMEOUT_S`.
    ///
    /// Example: `self.save_and_wait(app, "shared.respawn.start_s")?;`
    fn save_and_wait(&self, app: &mut App, what: &str) -> Result<()> {
        app.save_game_settings();
        let deadline: Instant = Instant::now() + Duration::from_secs_f64(SETTINGS_ANSWER_TIMEOUT_S);
        while app.has_unsaved_game_settings() {
            if Instant::now() >= deadline {
                return Err(format!(
                    "tour: the match server didn't take a save of {what} within {SETTINGS_ANSWER_TIMEOUT_S} s"
                )
                .into());
            }
            app.update();
            std::thread::sleep(Duration::from_millis(SETTINGS_POLL_MS));
        }
        println!("tour: the match server took a save of {what}");
        Ok(())
    }

    /// `is_done` says whether every toured level has been visited.
    ///
    /// Example: `if tour_run.is_done() { ... }`
    pub fn is_done(&self) -> bool {
        self.done_count >= self.level_indices.len()
    }

    /// `finish` writes `frame_times.md`, a table of each level's frame times, prints it, checks
    /// the game settings page (`check_game_settings`) and stops the game `app` hosts; it fails if
    /// the page got no settings, or if a process the game started is still alive then.
    ///
    /// Example: `tour_run.finish(&mut app)?;`
    pub fn finish(&self, app: &mut App) -> Result<()> {
        let mut table: String = format!(
            "Frame times inside the app at {}x{} px, {} (wall clock, from one frame's start to the next's)\n\n{FRAME_TABLE_HEADER}",
            self.size_px.0,
            self.size_px.1,
            describe_game_kind(self.kind)
        );
        for (name, summary) in &self.summaries {
            table.push_str(&format_frame_row(name, *summary));
        }
        fs::write(self.folder.join("frame_times.md"), &table)?;
        println!("{table}");
        let settings_check: Result<()> = self.check_game_settings(app);
        app.stop_hosting()?;
        settings_check?;
        println!("tour: every process the game started has stopped");
        Ok(())
    }
}
