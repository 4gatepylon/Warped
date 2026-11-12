//! `warped_dark_forest` is the Dark Forest level as a library: a bounded Euclidean forest with
//! treehouses, rope ladders and bridges, two pylons per team with one console each. Rust, SDL3 for
//! input and audio, Metal for rendering. The one-window app (`../warped_mvp_rust/`) plays it in
//! its own window through `play_level`; the `dark-forest` program (`main.rs`) plays it alone in a
//! window of its own through the same `level::Level`.
//!
//! Interface: `play_level` (a `warped_shared::app::play_level_fn_t`), and for `main.rs` the
//! modules `args`, `level`, `server`, `map`, `forest_layout` and `title`.
//!
//! Example: `let end: LevelEnd = warped_dark_forest::play_level(&mut level_window, &join)?;`
//!
//! Must know:
//! - Gotchas: the app's join plays as `dark-forest --join <server> --team <team> --no-title`
//!   with the level's `--map` and client flags from the levels manifest, so the forest plays the
//!   same in the app as run alone.
//! - Gotchas: of `LevelJoin::player_settings_path` the forest reads only the `visuals` (W-51,
//!   basic visuals): its keys and look speed are its own constants (Lunar Surface's defaults)
//!   until it reads the rest of the shared `player_settings`.

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
mod render;
mod scene;
pub mod server;
#[allow(dead_code)]
mod settings;
mod text_overlay;
#[allow(dead_code)]
mod thrown_wormholes;
pub mod main_menu;

use std::error::Error;

use clap::Parser;
use warped_shared::app::{LevelEnd, LevelJoin, LevelWindow};
use warped_shared::match_state::Team;
use warped_shared::player_settings::PerPlayerSettings;
use warped_shared::shot_noise;
use warped_shared::visuals::VisualsSettings;

use args::Args;
use level::{Level, LevelFiles};

/// `PROGRAM_NAME` is the name the join's command line starts with, as the program's own does.
const PROGRAM_NAME: &str = "dark-forest";

/// `get_join_visuals` gives the player's `visuals` for `join`: those of the player's settings
/// file where it exists, as the app's settings pages saved them, and the built-in ones (no
/// change) where the player has saved none.
///
/// Example: `let visuals: VisualsSettings = get_join_visuals(&join)?;`
fn get_join_visuals(join: &LevelJoin) -> Result<VisualsSettings, Box<dyn Error>> {
    if !join.player_settings_path.exists() {
        return Ok(VisualsSettings::IDENTITY);
    }
    Ok(PerPlayerSettings::Load(&join.player_settings_path)?.visuals)
}

/// `get_join_args` gives the flags that play `join`: join its server on its team at once, with
/// no main menu, the player's visuals (W-51, basic visuals), and the level's `--map` and
/// client flags from the levels manifest.
///
/// Example: `let args: Args = get_join_args(&join)?;`
fn get_join_args(join: &LevelJoin) -> Result<Args, Box<dyn Error>> {
    let team: &str = match join.team {
        Team::Blue => "blue",
        Team::Red => "red",
    };
    let visuals: VisualsSettings = get_join_visuals(join)?;
    let mut command_line: Vec<String> = [
        PROGRAM_NAME,
        "--join",
        &join.server.to_string(),
        "--team",
        team,
        "--no-title",
        "--brightness",
        &visuals.brightness.to_string(),
        "--contrast",
        &visuals.contrast.to_string(),
        "--saturation",
        &visuals.saturation.to_string(),
        "--gamma",
        &visuals.gamma.to_string(),
    ]
    .map(String::from)
    .to_vec();
    if let Some(map) = &join.level.map {
        command_line.extend(["--map".to_string(), map.clone()]);
    }
    command_line.extend(join.level.client_args.iter().cloned());
    Ok(Args::try_parse_from(command_line)?)
}

/// `play_level` plays `join` in the app's `window` until the match's round ends, the player
/// leaves for the map menu (the menu's QUIT page) or closes the window, and says which; it frees
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
    //! | Player's settings file | missing, with visuals, broken | `a_join_plays_its_server_and_team`, `a_join_plays_the_players_visuals`, `a_broken_settings_file_fails` |

    use std::net::SocketAddr;
    use std::path::PathBuf;

    use warped_shared::match_protocol::LevelEntry;

    use super::*;

    /// `make_join` is a join of Dark Forest's server at 127.0.0.1:48370 on `team`, with `map` and
    /// `client_args` in its manifest entry, and a player's settings file that doesn't exist.
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
                server_settings: None,
            },
            server: SocketAddr::from(([127, 0, 0, 1], 48370)),
            team,
            player_settings_path: PathBuf::from("no_such_folder/player_settings.yaml"),
        }
    }

    /// `make_join_with_settings_path` is `make_join`'s blue join with the player's settings file
    /// `file_name` in the tests' own folder under `.build/`, for the test to write.
    ///
    /// Example: `let join: LevelJoin = make_join_with_settings_path("mine.yaml");`
    fn make_join_with_settings_path(file_name: &str) -> LevelJoin {
        let folder: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".build/test_joins");
        std::fs::create_dir_all(&folder).expect("the tests' folder");
        let mut join: LevelJoin = make_join(Team::Blue, None, &[]);
        join.player_settings_path = folder.join(file_name);
        join
    }

    #[test]
    fn a_join_plays_its_server_and_team() {
        for (team, team_flag) in [(Team::Blue, "blue"), (Team::Red, "red")] {
            let args: Args = get_join_args(&make_join(team, None, &[])).expect("valid flags");
            assert_eq!(args.join.as_deref(), Some("127.0.0.1:48370"));
            assert_eq!(args.team.as_deref(), Some(team_flag));
            assert!(args.no_title && !args.host && !args.serve && !args.bot);
            assert_eq!(args.map, None);
            assert_eq!(args::get_start_visuals(&args), VisualsSettings::IDENTITY);
        }
    }

    #[test]
    fn a_join_plays_the_players_visuals() {
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        mine.visuals = VisualsSettings {
            brightness: 1.25,
            contrast: 0.75,
            saturation: 1.5,
            gamma: 1.75,
        };
        let join: LevelJoin = make_join_with_settings_path("with_visuals.yaml");
        mine.save(&join.player_settings_path)
            .expect("the settings file");
        let args: Args = get_join_args(&join).expect("valid flags");
        assert_eq!(args::get_start_visuals(&args), mine.visuals);
    }

    #[test]
    fn a_broken_settings_file_fails() {
        let join: LevelJoin = make_join_with_settings_path("broken.yaml");
        std::fs::write(&join.player_settings_path, "keys: 7").expect("the settings file");
        assert!(get_join_args(&join).is_err());
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
