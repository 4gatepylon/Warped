//! `play_log.rs` keeps the frame times of the levels a person plays in the app: after each level
//! it adds a row (frames, median, 99th percentile, slowest, whether inside 60 fps) to
//! `frame_times.md` in a run folder made at the first level, so a real game's frame rate can be
//! read afterwards.
//!
//! Interface: `PlayLog` (`New`, `record`), `PLAY_SETTLE_S`.
//!
//! Example: `play_log.record("Dust", &app.describe_game(), (2560, 1334), timer.get_frame_times())?;`
//!
//! Must know:
//! - The run folder is `.artifacts/play_frame_times/<run_id>/` in the checkout, and
//!   `~/Library/Logs/Warped/play_frame_times/<run_id>/` when the app runs from `Warped.app`
//!   (`children.rs`'s `make_run_folder`).

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::app_state::format_utc_now;
use crate::children::make_run_folder;
use crate::level_link::{FRAME_TABLE_HEADER, format_frame_row, summarise_frame_times};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `PLAY_SETTLE_S` is how long a level loads and settles before its frames count.
pub const PLAY_SETTLE_S: f64 = 5.0;
const SCRIPT: &str = "play_frame_times";
const FILE_NAME: &str = "frame_times.md";

/// `PlayLog` is the table of the frame times of the levels played: its file, once a level has
/// been played.
pub struct PlayLog {
    path: Option<PathBuf>,
}

impl PlayLog {
    /// `New` makes a log that has no file yet.
    ///
    /// Example: `let mut play_log = PlayLog::New();`
    #[allow(non_snake_case)]
    pub fn New() -> PlayLog {
        PlayLog { path: None }
    }

    /// `record` adds the row of `level_name`, played in `game` (who was in it,
    /// `App::describe_game`) in a window of `size_px` with the frame times `times_ms`, making the
    /// run folder and the table's head at the first level, and gives the file.
    ///
    /// Example: `let path = play_log.record("Dust", &app.describe_game(), (2560, 1334), timer.get_frame_times())?;`
    pub fn record(
        &mut self,
        level_name: &str,
        game: &str,
        size_px: (u32, u32),
        times_ms: &[f64],
    ) -> Result<PathBuf> {
        let path: PathBuf = match &self.path {
            Some(path) => path.clone(),
            None => make_run_folder(SCRIPT, &format_utc_now())?.join(FILE_NAME),
        };
        let row_name: String = format!("{level_name} ({game}; {}x{} px)", size_px.0, size_px.1);
        add_row(&path, &row_name, times_ms)?;
        self.path = Some(path.clone());
        Ok(path)
    }
}

/// `add_row` appends the row named `row_name` (the level, who was in the game, the window's
/// size) to the table at `path`, writing the table's head first when the file isn't there yet.
///
/// Example: `add_row(&path, "Dust (playing alone; 2560x1334 px)", &[10.0, 11.0])?;`
fn add_row(path: &Path, row_name: &str, times_ms: &[f64]) -> Result<()> {
    let mut text: String = String::new();
    if !path.exists() {
        text.push_str(&format!(
            "Frame times of the levels played in the app (wall clock, from one frame's start to the next's, after each level's first {PLAY_SETTLE_S} s; each row names who was in the game, bots too, and the window's size)\n\n{FRAME_TABLE_HEADER}"
        ));
    }
    text.push_str(&format_frame_row(row_name, summarise_frame_times(times_ms)));
    let mut file: fs::File = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("can't open {}: {error}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| format!("can't write {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for `play_log.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Rows in the file | the first (writes the head), a later one (appends) | `rows_follow_one_head` |
    //! | A level's frames | some, none (left while it loaded) | `rows_follow_one_head` |

    use super::*;

    const TEST_FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.build/test_play_log");

    #[test]
    fn rows_follow_one_head() {
        let folder: &Path = Path::new(TEST_FOLDER);
        fs::create_dir_all(folder).unwrap();
        let path: PathBuf = folder.join(FILE_NAME);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        add_row(&path, "Dust (playing alone; 2560x1334 px)", &[10.0, 12.0]).unwrap();
        add_row(&path, "Dark Forest", &[]).unwrap();
        let text: String = fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches(FRAME_TABLE_HEADER).count(), 1);
        assert!(text.starts_with("Frame times of the levels played in the app"));
        assert!(text.ends_with(
            "| Dust (playing alone; 2560x1334 px) | 2 | 12.00 | 12.00 | 12.00 | yes |\n| Dark Forest | 0 | - | - | - | no frames |\n"
        ));
    }
}
