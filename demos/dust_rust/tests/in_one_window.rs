//! `in_one_window.rs` checks Dust's entry point (`play_level`) as the one-window app uses
//! it, without a window on screen (`warped_shared::level_test`): leaving by the menu's QUIT page
//! gives `LeaveLevel`, the match's round ending gives `RoundOver`, and a second level starts in the
//! same process with no GPU memory left behind. It uses the GPU, so it runs only with
//! `RUN_GPU_TESTS=1`; otherwise it says so and passes.
//!
//! | Partitioning | Parts | Tests |
//! | --- | --- | --- |
//! | How the level ends | the QUIT page, the round over | `main` |
//! | Plays in one process | first, after another | `main` |
//!
//! Example: `RUN_GPU_TESTS=1 cargo test --release --test in_one_window`

use std::process::ExitCode;

use warped_shared::app::LevelJoin;
use warped_shared::level_test::{check_level_in_one_window, make_sample_join};
use warped_shared::match_state::Team;

const SILENT_SERVER: &str = "127.0.0.1:48369";
const PLAYER_SETTINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/player_settings.yaml");

/// `main` runs the check when `RUN_GPU_TESTS=1`, and fails with its reason.
///
/// Example: `RUN_GPU_TESTS=1 cargo test --release --test in_one_window`
fn main() -> ExitCode {
    if std::env::var_os("RUN_GPU_TESTS").is_none() {
        println!("in_one_window: skipped (it uses the GPU; run it with RUN_GPU_TESTS=1)");
        return ExitCode::SUCCESS;
    }
    let join: LevelJoin = make_sample_join(
        "Dust",
        Some("dust"),
        SILENT_SERVER.parse().expect("an address"),
        Team::Blue,
        PLAYER_SETTINGS_PATH.into(),
    );
    match check_level_in_one_window(dust_level::play_level, &join) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("in_one_window: {error}");
            ExitCode::FAILURE
        }
    }
}
