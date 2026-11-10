//! `children.rs` starts the headless programs behind a game the player hosts: the match server
//! (which starts each round's level server itself) and the bots, each a match client without a
//! window. They are child processes of the app, logging to its run folder, stopped when the
//! player stops hosting or quits.
//!
//! Interface: `GameKind`, `BotObjective`, `BotPlan`, `Programs` (`Find`), `plan_bots`,
//! `MAX_ALLY_COUNT`, `MAX_ENEMY_COUNT`, `MAX_BOT_COUNT`,
//! `make_match_server_command`, `make_bot_command`, `HostedGame` (`Start`, `has_stopped`,
//! `move_sit_out_bot`, `stop`), `make_run_folder`, `SIT_OUT_ROUND_COUNT`.
//!
//! Example: `let game = HostedGame::Start(&programs, GameKind::Alone, Team::Blue, ports, &folder)?;`
//!
//! Must know:
//! - Gotchas: playing alone runs one bot on the other team that sits out every round
//!   (`--bot-round-winners` naming the player's team), because a match round starts only when
//!   each team has a player.
//! - Gotchas: `--bot-objective fight` reaches the match client, which (until its `main.rs` passes
//!   it on, after W-140) still starts console bots.
//! - Side effects: the match server writes its own run folder in its crate's `.artifacts/`.

use std::error::Error;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use warped_shared::app_bundle::get_app_resources_folder;
use warped_shared::level_net;
use warped_shared::match_protocol::stop_child;
use warped_shared::match_state::Team;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `SIT_OUT_ROUND_COUNT` is how many rounds the lone player's sitting-out bot lasts, more than
/// any match or free play needs.
pub const SIT_OUT_ROUND_COUNT: usize = 255;
const MATCH_PROGRAM_IN_APP: &str = "warped-match";
const CHECKOUT_MATCH_PROGRAM: &str = "../warped_match_rust/.build/release/hyperbolic-wormholes";
const LOGS_HOME_PATH: &str = "Library/Logs/Warped";
const CHECKOUT_ARTIFACTS_FOLDER: &str = ".artifacts";

/// `GameKind` is the game the player starts from the main menu: hosted for others, alone, or
/// with bots (how many on the player's team and on the other, and what they do).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameKind {
    Host,
    Alone,
    Bots {
        ally_count: u32,
        enemy_count: u32,
        objective: BotObjective,
    },
}

/// `MAX_ALLY_COUNT`, `MAX_ENEMY_COUNT` and `MAX_BOT_COUNT` bound the bots of a game: a level
/// takes `level_net::MAX_PLAYER_COUNT` players (12), half on each team, so the player and up to 5
/// allies face up to 6 enemies (W-155, play against bots: 5v5 and a spare player each).
pub const MAX_ALLY_COUNT: u32 = MAX_TEAM_PLAYER_COUNT - 1;
pub const MAX_ENEMY_COUNT: u32 = MAX_TEAM_PLAYER_COUNT;
pub const MAX_BOT_COUNT: u32 = MAX_ALLY_COUNT + MAX_ENEMY_COUNT;
/// `MAX_TEAM_PLAYER_COUNT` is how many players one team has at most, half a level's.
const MAX_TEAM_PLAYER_COUNT: u32 = level_net::MAX_PLAYER_COUNT as u32 / 2;

/// `BotObjective` is what a bot does in each round: turn off the other team's consoles, or fight.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BotObjective {
    Consoles,
    Fight,
}

impl BotObjective {
    /// `get_flag` gives the objective as the level programs' `--bot-objective` takes it.
    ///
    /// Example: `assert_eq!(BotObjective::Fight.get_flag(), "fight");`
    pub fn get_flag(self) -> &'static str {
        match self {
            BotObjective::Consoles => "consoles",
            BotObjective::Fight => "fight",
        }
    }
}

/// `BotPlan` is one bot to start: its team, its objective, and whether it sits out every round
/// (the lone player's stand-in opponent).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BotPlan {
    pub team: Team,
    pub objective: BotObjective,
    pub is_sitting_out: bool,
}

/// `plan_bots` gives the bots a game of `kind` needs for a player on `player_team`: none for a
/// hosted game; one sitting-out bot on the other team alone; the allies on the player's team and
/// the enemies on the other otherwise, with a sitting-out bot there when there are no enemies.
///
/// Example: `let bots = plan_bots(GameKind::Alone, Team::Blue);`
pub fn plan_bots(kind: GameKind, player_team: Team) -> Vec<BotPlan> {
    match kind {
        GameKind::Host => Vec::new(),
        GameKind::Alone => plan_bots(
            GameKind::Bots {
                ally_count: 0,
                enemy_count: 0,
                objective: BotObjective::Consoles,
            },
            player_team,
        ),
        GameKind::Bots {
            ally_count,
            enemy_count,
            objective,
        } => {
            let make = |team: Team, is_sitting_out: bool| BotPlan {
                team,
                objective,
                is_sitting_out,
            };
            let other: Team = player_team.get_other();
            let mut plans: Vec<BotPlan> =
                (0..ally_count).map(|_| make(player_team, false)).collect();
            plans.extend((0..enemy_count).map(|_| make(other, false)));
            if enemy_count == 0 {
                plans.push(BotPlan {
                    objective: BotObjective::Consoles,
                    ..make(other, true)
                });
            }
            plans
        }
    }
}

/// `Programs` is where the headless programs and the levels manifest are: the match server's
/// program (which also runs the bots), and the manifest it plays.
#[derive(Clone, PartialEq, Debug)]
pub struct Programs {
    pub match_server: PathBuf,
    pub levels_manifest: PathBuf,
}

impl Programs {
    /// `Find` gives the programs inside `Warped.app` when the app runs from one, else in the
    /// checkout (built by `make build-children`), failing with the fix when the match server
    /// isn't built.
    ///
    /// Example: `let programs = Programs::Find(&manifest)?;`
    #[allow(non_snake_case)]
    pub fn Find(levels_manifest: &Path) -> Result<Programs> {
        let match_server: PathBuf = match get_app_resources_folder() {
            Some(resources) => resources.with_file_name("MacOS").join(MATCH_PROGRAM_IN_APP),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).join(CHECKOUT_MATCH_PROGRAM),
        };
        if !match_server.exists() {
            return Err(format!(
                "the match server {} isn't built; run make build-children in demos/warped_mvp_rust",
                match_server.display()
            )
            .into());
        }
        Ok(Programs {
            match_server,
            levels_manifest: levels_manifest.to_path_buf(),
        })
    }
}

/// `make_match_server_command` gives the command that serves a match of the manifest's levels on
/// UDP `match_port`, each round's level server on `level_port`.
///
/// Example: `let command = make_match_server_command(&programs, 47850, 47852);`
pub fn make_match_server_command(programs: &Programs, match_port: u16, level_port: u16) -> Command {
    let mut command: Command = Command::new(&programs.match_server);
    command
        .arg("--serve-match")
        .arg("--match-levels")
        .arg(&programs.levels_manifest)
        .args(["--match-port", &match_port.to_string()])
        .args(["--port", &level_port.to_string()]);
    command
}

/// `make_bot_command` gives the command of a bot (`plan`) joining the match server at `server`.
///
/// Example: `let command = make_bot_command(&programs, server, &plan);`
pub fn make_bot_command(programs: &Programs, server: SocketAddr, plan: &BotPlan) -> Command {
    let mut command: Command = Command::new(&programs.match_server);
    command
        .args(["--match", &server.ip().to_string()])
        .args(["--match-port", &server.port().to_string()])
        .args(["--bot", "--team", plan.team.get_name()])
        .args(["--bot-objective", plan.objective.get_flag()]);
    if plan.is_sitting_out {
        let winners: Vec<&str> = vec![plan.team.get_other().get_name(); SIT_OUT_ROUND_COUNT];
        command.arg("--bot-round-winners").arg(winners.join(","));
    }
    command
}

/// `make_run_folder` makes the folder of a run of `script` (`warped` for the children's logs,
/// `screen_pictures`): in the checkout's `.artifacts/<script>/<run_id>/`, or in
/// `~/Library/Logs/Warped/<run_id>/` when the app runs from `Warped.app`; it holds
/// `command.txt` (the command line and the commit).
///
/// Example: `let folder = make_run_folder("warped", "20261109-120000")?;`
pub fn make_run_folder(script: &str, run_id: &str) -> Result<PathBuf> {
    let folder: PathBuf = match get_app_resources_folder() {
        Some(_) => {
            let home: String = std::env::var("HOME")?;
            Path::new(&home).join(LOGS_HOME_PATH).join(run_id)
        }
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(CHECKOUT_ARTIFACTS_FOLDER)
            .join(script)
            .join(run_id),
    };
    fs::create_dir_all(&folder)
        .map_err(|error| format!("can't make {}: {error}", folder.display()))?;
    let commit: String = Command::new("git")
        .args(["describe", "--always", "--dirty"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|error| format!("unknown ({error})"));
    let command: String = std::env::args().collect::<Vec<String>>().join(" ");
    fs::write(
        folder.join("command.txt"),
        format!("{command}\ncommit: {commit}\n"),
    )?;
    Ok(folder)
}

/// `spawn_logged` starts `command` with its output in `folder/<name>.log`.
///
/// Example: `let child = spawn_logged(command, &folder, "match_server")?;`
fn spawn_logged(mut command: Command, folder: &Path, name: &str) -> Result<Child> {
    let log: fs::File = fs::File::create(folder.join(format!("{name}.log")))?;
    eprintln!("warped: starting {command:?}");
    Ok(command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .map_err(|error| format!("can't start {name} ({error})"))?)
}

/// `HostedGame` is a game this app hosts: its kind, the match server's address, the match server
/// and the bots, and the folder of their logs.
pub struct HostedGame {
    pub kind: GameKind,
    pub server: SocketAddr,
    pub run_folder: PathBuf,
    programs: Programs,
    match_server: Option<Child>,
    bots: Vec<(BotPlan, Option<Child>)>,
}

impl HostedGame {
    /// `Start` starts the match server on `ports` (match, level) and the bots of `kind` for a
    /// player on `player_team`, logging into `run_folder`.
    ///
    /// Example: `let game = HostedGame::Start(&programs, kind, Team::Blue, (47850, 47852), &folder)?;`
    #[allow(non_snake_case)]
    pub fn Start(
        programs: &Programs,
        kind: GameKind,
        player_team: Team,
        ports: (u16, u16),
        run_folder: &Path,
    ) -> Result<HostedGame> {
        let server: SocketAddr = SocketAddr::from((Ipv4Addr::LOCALHOST, ports.0));
        let match_server: Child = spawn_logged(
            make_match_server_command(programs, ports.0, ports.1),
            run_folder,
            "match_server",
        )?;
        let mut game: HostedGame = HostedGame {
            kind,
            server,
            run_folder: run_folder.to_path_buf(),
            programs: programs.clone(),
            match_server: Some(match_server),
            bots: Vec::new(),
        };
        for plan in plan_bots(kind, player_team) {
            game.start_bot(plan)?;
        }
        Ok(game)
    }

    /// `start_bot` starts the bot of `plan`, logging as `bot_<n>_<team>`.
    ///
    /// Example: `self.start_bot(plan)?;`
    fn start_bot(&mut self, plan: BotPlan) -> Result<()> {
        let name: String = format!("bot_{}_{}", self.bots.len() + 1, plan.team.get_name());
        let child: Child = spawn_logged(
            make_bot_command(&self.programs, self.server, &plan),
            &self.run_folder,
            &name,
        )?;
        self.bots.push((plan, Some(child)));
        Ok(())
    }

    /// `has_stopped` says, with its exit status, whether the match server stopped by itself (a
    /// port in use, a level that failed, or the match over).
    ///
    /// Example: `if let Some(status) = game.has_stopped()? { ... }`
    pub fn has_stopped(&mut self) -> Result<Option<String>> {
        match self.match_server.as_mut() {
            Some(child) => Ok(child.try_wait()?.map(|status| status.to_string())),
            None => Ok(Some("stopped".to_string())),
        }
    }

    /// `move_sit_out_bot` puts the lone player's sitting-out bot on the team other than
    /// `player_team`, restarting it there; it does nothing in other games.
    ///
    /// Example: `game.move_sit_out_bot(Team::Red)?;`
    pub fn move_sit_out_bot(&mut self, player_team: Team) -> Result<()> {
        if self.kind != GameKind::Alone {
            return Ok(());
        }
        let wanted: Team = player_team.get_other();
        if self.bots.iter().any(|(plan, _)| plan.team == wanted) {
            return Ok(());
        }
        for (_, child) in &mut self.bots {
            stop_child(child);
        }
        self.bots.clear();
        for plan in plan_bots(GameKind::Alone, player_team) {
            self.start_bot(plan)?;
        }
        Ok(())
    }

    /// `stop` stops the bots and the match server (which stops its level server).
    ///
    /// Example: `game.stop();`
    pub fn stop(&mut self) {
        for (_, child) in &mut self.bots {
            stop_child(child);
        }
        stop_child(&mut self.match_server);
    }
}

impl Drop for HostedGame {
    /// `drop` stops the children, so none outlives the app.
    ///
    /// Example: `drop(game);`
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `children.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Game kind | hosted, alone (blue, red), bots (none: as alone; allies and enemies; allies only: a sitting-out enemy; the most) | `each_game_kind_plans_its_bots` |
    //! | Match server command | ports and manifest passed | `the_match_server_serves_the_manifest_on_its_ports` |
    //! | Bot command | consoles, fight, sitting out (as many rounds as `SIT_OUT_ROUND_COUNT`, the other team named) | `bots_join_with_their_team_and_objective` |

    use super::*;

    /// `get_args` gives a command's arguments as strings.
    ///
    /// Example: `let args = get_args(&command);`
    fn get_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    /// `make_programs` gives programs at made-up paths.
    ///
    /// Example: `let programs = make_programs();`
    fn make_programs() -> Programs {
        Programs {
            match_server: PathBuf::from("/app/warped-match"),
            levels_manifest: PathBuf::from("/app/match_levels.yaml"),
        }
    }

    #[test]
    fn each_game_kind_plans_its_bots() {
        assert!(plan_bots(GameKind::Host, Team::Blue).is_empty());
        for team in [Team::Blue, Team::Red] {
            let alone: Vec<BotPlan> = plan_bots(GameKind::Alone, team);
            assert_eq!(alone.len(), 1);
            assert_eq!(alone[0].team, team.get_other());
            assert!(alone[0].is_sitting_out);
        }
        let objective: BotObjective = BotObjective::Fight;
        let make = |ally_count: u32, enemy_count: u32| GameKind::Bots {
            ally_count,
            enemy_count,
            objective,
        };
        assert_eq!(
            plan_bots(make(0, 0), Team::Red),
            plan_bots(GameKind::Alone, Team::Red)
        );
        let both: Vec<BotPlan> = plan_bots(make(1, 2), Team::Red);
        let teams: Vec<Team> = both.iter().map(|plan| plan.team).collect();
        assert_eq!(teams, [Team::Red, Team::Blue, Team::Blue]);
        assert!(
            both.iter()
                .all(|plan| !plan.is_sitting_out && plan.objective == objective)
        );
        let allies_only: Vec<BotPlan> = plan_bots(make(MAX_ALLY_COUNT, 0), Team::Blue);
        assert_eq!(allies_only.len() as u32, MAX_ALLY_COUNT + 1);
        assert!(
            allies_only
                .last()
                .is_some_and(|plan| plan.is_sitting_out && plan.team == Team::Red)
        );
        assert_eq!(
            plan_bots(make(MAX_ALLY_COUNT, MAX_ENEMY_COUNT), Team::Blue).len() as u32,
            MAX_BOT_COUNT
        );
    }

    #[test]
    fn the_match_server_serves_the_manifest_on_its_ports() {
        let command: Command = make_match_server_command(&make_programs(), 48_304, 48_305);
        assert_eq!(command.get_program(), "/app/warped-match");
        assert_eq!(
            get_args(&command),
            [
                "--serve-match",
                "--match-levels",
                "/app/match_levels.yaml",
                "--match-port",
                "48304",
                "--port",
                "48305"
            ]
        );
    }

    #[test]
    fn bots_join_with_their_team_and_objective() {
        let server: SocketAddr = "127.0.0.1:48304".parse().unwrap();
        for objective in [BotObjective::Consoles, BotObjective::Fight] {
            let plan: BotPlan = BotPlan {
                team: Team::Red,
                objective,
                is_sitting_out: false,
            };
            assert_eq!(
                get_args(&make_bot_command(&make_programs(), server, &plan)),
                [
                    "--match",
                    "127.0.0.1",
                    "--match-port",
                    "48304",
                    "--bot",
                    "--team",
                    "red",
                    "--bot-objective",
                    objective.get_flag()
                ]
            );
        }
        let sitting_out: BotPlan = BotPlan {
            team: Team::Blue,
            objective: BotObjective::Consoles,
            is_sitting_out: true,
        };
        let args: Vec<String> = get_args(&make_bot_command(&make_programs(), server, &sitting_out));
        let winners_at: usize = args
            .iter()
            .position(|arg| arg == "--bot-round-winners")
            .unwrap();
        let winners: Vec<&str> = args[winners_at + 1].split(',').collect();
        assert_eq!(winners.len(), SIT_OUT_ROUND_COUNT);
        assert!(winners.iter().all(|&winner| winner == "red"));
    }
}
