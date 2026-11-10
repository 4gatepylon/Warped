//! `warped_dark_forest` is the Dark Forest level as a library: a bounded Euclidean forest with
//! treehouses, rope ladders and bridges, two pylons per team with one console each. Rust, SDL3 for
//! input and audio, Metal for rendering. The one-window app (`../warped_mvp_rust/`) plays it in
//! its own window through `play_level`; the `dark-forest` program (`main.rs`) plays it alone in a
//! window of its own through the same `level::Level`.
//!
//! Interface: `play_level` (a `warped_shared::app::play_level_fn_t`), and for `main.rs` the
//! modules `args`, `level`, `platform`, `server`, `map` and `forest_layout`.
//!
//! Example: `let end: LevelEnd = warped_dark_forest::play_level(&mut level_window, &join)?;`
//!
//! Must know:
//! - Gotchas: the app's join plays as `dark-forest --join <server> --team <team> --no-title`
//!   with the level's `--map` and client flags from the levels manifest, so the forest plays the
//!   same in the app as run alone.
//! - Gotchas: the forest doesn't read `LevelJoin::player_settings_path` yet: its keys and look
//!   speed are its own constants (Lunar Surface's defaults) until it reads the shared
//!   `player_settings`.

pub mod args;
#[allow(dead_code)]
mod asset_manifest;
mod assets;
mod console_reach;
mod credits;
pub mod forest_layout;
mod game;
pub mod level;
pub mod map;
mod match_play;
mod menu;
mod minimap;
#[allow(dead_code)]
mod net;
pub mod platform;
mod render;
mod scene;
pub mod server;
#[allow(dead_code)]
mod settings;
mod text_overlay;
#[allow(dead_code)]
mod thrown_wormholes;
pub mod title;

use std::error::Error;

use clap::Parser;
use warped_shared::app::{LevelEnd, LevelJoin, LevelWindow};
use warped_shared::match_state::Team;
use warped_shared::shot_noise;

use args::Args;
use level::{Level, LevelFiles};

/// `PROGRAM_NAME` is the name the join's command line starts with, as the program's own does.
const PROGRAM_NAME: &str = "dark-forest";

/// `get_join_args` gives the flags that play `join`: join its server on its team at once, with
/// no title screen, and the level's `--map` and client flags from the levels manifest.
///
/// Example: `let args: Args = get_join_args(&join)?;`
fn get_join_args(join: &LevelJoin) -> Result<Args, clap::Error> {
    let team: &str = match join.team {
        Team::Blue => "blue",
        Team::Red => "red",
    };
    let mut command_line: Vec<String> = [
        PROGRAM_NAME,
        "--join",
        &join.server.to_string(),
        "--team",
        team,
        "--no-title",
    ]
    .map(String::from)
    .to_vec();
    if let Some(map) = &join.level.map {
        command_line.extend(["--map".to_string(), map.clone()]);
    }
    command_line.extend(join.level.client_args.iter().cloned());
    Args::try_parse_from(command_line)
}

/// `play_level` plays `join` in the app's `window` until the match's round ends, the player
/// leaves for the game menu (the menu's QUIT page) or closes the window, and says which; it frees
/// its GPU resources and gives the window back as it found it before it returns. It fails if the
/// join's flags are wrong, the map or assets are missing, the server can't be reached, or a
/// client flag ended the game on its own (such as `--exit-after-s`).
///
/// Example: `let end: LevelEnd = play_level(&mut level_window, &join)?;`
pub fn play_level(window: &mut LevelWindow, join: &LevelJoin) -> Result<LevelEnd, Box<dyn Error>> {
    let args: Args = get_join_args(join)?;
    let files: LevelFiles = LevelFiles::Find(&args)?;
    let level: Level = Level::Load(window, &args, files);
    let end: Option<LevelEnd> = level.play(window, &args, false)?;
    end.ok_or_else(|| {
        format!(
            "Dark Forest stopped on its own; check the client flags {:?}",
            join.level.client_args
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for `lib.rs`: the flags a join plays with.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Team | blue, red | `a_join_plays_its_server_and_team` |
    //! | Level's `--map` | none, given | `a_join_plays_its_server_and_team`, `a_join_passes_the_levels_map_and_flags` |
    //! | Level's client flags | none, some, unknown | `a_join_plays_its_server_and_team`, `a_join_passes_the_levels_map_and_flags`, `an_unknown_client_flag_fails` |

    use std::net::SocketAddr;
    use std::path::PathBuf;

    use warped_shared::match_protocol::LevelEntry;

    use super::*;

    /// `make_join` is a join of Dark Forest's server at 127.0.0.1:48370 on `team`, with `map` and
    /// `client_args` in its manifest entry.
    ///
    /// Example: `let join: LevelJoin = make_join(Team::Red, None, &[]);`
    fn make_join(team: Team, map: Option<&str>, client_args: &[&str]) -> LevelJoin {
        LevelJoin {
            level: LevelEntry {
                name: "Dark Forest".to_string(),
                folder: "demos/dark_forest_rust".to_string(),
                branch: "staging/user-4gatepylon".to_string(),
                status: "playable".to_string(),
                program: "dark-forest".to_string(),
                map: map.map(String::from),
                server_args: Vec::new(),
                client_args: client_args.iter().map(|arg| arg.to_string()).collect(),
                stand_in_for: None,
                takes_bot_label: false,
                map_picture: None,
            },
            server: SocketAddr::from(([127, 0, 0, 1], 48370)),
            team,
            player_settings_path: PathBuf::from("player_settings.yaml"),
        }
    }

    #[test]
    fn a_join_plays_its_server_and_team() {
        for (team, team_flag) in [(Team::Blue, "blue"), (Team::Red, "red")] {
            let args: Args = get_join_args(&make_join(team, None, &[])).expect("valid flags");
            assert_eq!(args.join.as_deref(), Some("127.0.0.1:48370"));
            assert_eq!(args.team.as_deref(), Some(team_flag));
            assert!(args.no_title && !args.host && !args.serve && !args.bot);
            assert_eq!(args.map, None);
        }
    }

    #[test]
    fn a_join_passes_the_levels_map_and_flags() {
        let join: LevelJoin =
            make_join(Team::Blue, Some("forest.json"), &["--render-scale", "0.8"]);
        let args: Args = get_join_args(&join).expect("valid flags");
        assert_eq!(args.map, Some(PathBuf::from("forest.json")));
        assert_eq!(args.render_scale, 0.8);
    }

    #[test]
    fn an_unknown_client_flag_fails() {
        assert!(get_join_args(&make_join(Team::Red, None, &["--no-such-flag"])).is_err());
    }
}
