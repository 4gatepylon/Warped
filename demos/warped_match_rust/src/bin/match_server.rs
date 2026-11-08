//! `match_server.rs` runs one match of the online match (W-127) without a window, for a machine that
//! has no screen, such as the cloud VM: the same match server as `--serve-match`, starting each
//! round's level from the levels file's `server_program`s.
//!
//! Interface: `--port`, `--level-port`, `--match-levels`, `--game-password-file`, `--intermission-s`,
//! `--round-over-s`, `--match-s`; it exits once the match is over, so a service manager that
//! restarts it starts the next match.
//!
//! Example: `.build/x86_64-unknown-linux-musl/release/match_server --match-levels app/match_levels.yaml`
//!
//! Must know:
//! - It writes each round's result and level server log in a new folder, `match_<start time in s>`,
//!   in the current folder (on the cloud VM, the service's own writable folder).

#[allow(dead_code)]
#[path = "../app_bundle.rs"]
mod app_bundle;
#[allow(dead_code)]
#[path = "../geometry.rs"]
mod geometry;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../key_bindings.rs"]
mod key_bindings;
#[path = "../map.rs"]
mod map;
#[allow(dead_code)]
#[path = "../match_flow.rs"]
mod match_flow;
#[allow(dead_code)]
#[path = "../match_state.rs"]
mod match_state;
#[allow(dead_code)]
#[path = "../net.rs"]
mod net;
#[allow(dead_code)]
#[path = "../settings.rs"]
mod settings;
#[allow(dead_code)]
#[path = "../shot_noise.rs"]
mod shot_noise;
#[allow(dead_code)]
#[path = "../terrain.rs"]
mod terrain;
#[allow(dead_code)]
#[path = "../thrown_wormholes.rs"]
mod thrown_wormholes;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Run one match of the online match without a window, until it is over.",
    after_help = "Example: match_server --match-levels app/match_levels.yaml"
)]
struct Arguments {
    /// The match's UDP port.
    #[arg(long, default_value_t = match_flow::MATCH_PORT)]
    port: u16,
    /// The UDP port each round's level server runs on.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    level_port: u16,
    /// The levels file; its programs are paths from its folder.
    #[arg(long)]
    match_levels: PathBuf,
    /// A file holding the game's password, which players and each level server get.
    #[arg(long)]
    game_password_file: Option<PathBuf>,
    /// Seconds between rounds, showing the progression.
    #[arg(long, default_value_t = 10.0)]
    intermission_s: f64,
    /// Seconds a won level stays open, showing its win.
    #[arg(long, default_value_t = 5.0)]
    round_over_s: f64,
    /// The match's length in seconds; a match no team has won by then is a tie.
    #[arg(long, default_value_t = 1800.0)]
    match_s: f64,
}

/// `main` runs one match and prints how it ended.
///
/// Example: `main()`
fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Arguments = Arguments::parse();
    let levels: Vec<match_flow::LevelEntry> = match_flow::LoadLevels(&arguments.match_levels)?;
    let started_s: u64 = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let run_folder: PathBuf = PathBuf::from(format!("match_{started_s}"));
    std::fs::create_dir_all(&run_folder)?;
    let game_password: Vec<u8> = net::read_game_password(arguments.game_password_file.as_deref())?;
    let state: match_state::MatchState =
        match_flow::serve_match(&match_flow::MatchServerOptions {
            levels: &levels,
            levels_folder: arguments.match_levels.parent().unwrap_or(Path::new(".")),
            times: match_flow::MatchTimes {
                intermission_s: arguments.intermission_s,
                round_over_s: arguments.round_over_s,
                match_s: arguments.match_s,
            },
            run_folder: &run_folder,
            match_port: arguments.port,
            level_port: arguments.level_port,
            game_password: &game_password,
            game_password_file: arguments.game_password_file.as_deref(),
        })?;
    let outcome: match_state::Outcome = state.outcome().expect("serve_match ends with an outcome");
    println!("{}", match_flow::describe_outcome(outcome));
    Ok(())
}
