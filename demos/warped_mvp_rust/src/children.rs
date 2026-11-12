//! `children.rs` starts the headless programs behind a game the player hosts: the game server
//! (which starts each round's level server itself) and the bots, each a match client without a
//! window. They are child processes of the app, logging to its run folder, stopped when the
//! player stops hosting or quits, and when the app ends in any other way (`ChildGroup`).
//!
//! Interface: `GameKind`, `BotObjective`, `BotPlan`, `Programs` (`Find`), `plan_bots`,
//! `MAX_ALLY_COUNT`, `MAX_ENEMY_COUNT`, `MAX_BOT_COUNT`,
//! `make_game_server_command`, `make_bot_command`, `HostedGame` (`Start`, `has_stopped`,
//! `move_sit_out_bot`, `stop`), `make_run_folder`, `describe_game_kind`, `SIT_OUT_ROUND_COUNT`.
//!
//! Example: `let game = HostedGame::Start(&programs, GameKind::Alone, Team::Blue, ports, &folder)?;`
//!
//! Must know:
//! - Gotchas: playing alone runs one bot on the other team that sits out every round
//!   (`--bot-round-winners` naming the player's team), because a match round starts only when
//!   each team has a player.
//! - Gotchas: `--bot-objective fight` reaches the match client, which (until its `main.rs` passes
//!   it on, after W-140) still starts console bots.
//! - Side effects: the game server writes its own run folder in its crate's `.artifacts/`.
//! - Gotchas: everything a game starts shares one process group, whose guard kills it all when
//!   the app ends, however it ends (a crash too). A level server is the game server's child, so
//!   only the group reaches it once the game server is killed.

use std::error::Error;
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use warped_shared::app_bundle::{get_app_logs_folder, get_app_resources_folder};
use warped_shared::level_net;
use warped_shared::match_protocol::stop_child;
use warped_shared::match_state::Team;
use warped_shared::network_scope;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `SIT_OUT_ROUND_COUNT` is how many rounds the lone player's sitting-out bot lasts, more than
/// any match or free play needs.
pub const SIT_OUT_ROUND_COUNT: usize = 255;
const MATCH_PROGRAM_IN_APP: &str = "warped-match";
const CHECKOUT_MATCH_PROGRAM: &str = "../warped_match_rust/.build/release/hyperbolic-wormholes";
const CHECKOUT_ARTIFACTS_FOLDER: &str = ".artifacts";
const GUARD_SHELL: &str = "/bin/sh";
const GUARD_SCRIPT: &str = "read -r line; kill -KILL 0";
const KILL_PROGRAM: &str = "/bin/kill";
const NO_SIGNAL_FLAG: &str = "-0";
const GROUP_EMPTY_WAIT_MS: u64 = 2000;
const GROUP_POLL_MS: u64 = 20;

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

/// `Programs` is where the headless programs and the levels manifest are: the game server's
/// program (which also runs the bots), and the manifest it plays.
#[derive(Clone, PartialEq, Debug)]
pub struct Programs {
    pub game_server: PathBuf,
    pub levels_manifest: PathBuf,
}

impl Programs {
    /// `Find` gives the programs inside `Warped.app` when the app runs from one, else in the
    /// checkout (built by `make build-children`), failing with the fix when the game server
    /// isn't built.
    ///
    /// Example: `let programs = Programs::Find(&manifest)?;`
    #[allow(non_snake_case)]
    pub fn Find(levels_manifest: &Path) -> Result<Programs> {
        let game_server: PathBuf = match get_app_resources_folder() {
            Some(resources) => resources.with_file_name("MacOS").join(MATCH_PROGRAM_IN_APP),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).join(CHECKOUT_MATCH_PROGRAM),
        };
        if !game_server.exists() {
            return Err(format!(
                "the game server {} isn't built; run make build-children in demos/warped_mvp_rust",
                game_server.display()
            )
            .into());
        }
        Ok(Programs {
            game_server,
            levels_manifest: levels_manifest.to_path_buf(),
        })
    }
}

/// `make_game_server_command` gives the command that serves a match of the manifest's levels on
/// UDP `match_port`, each round's level server on `level_port`, with it and every level on
/// 127.0.0.1 only (`WARPED_LOOPBACK_ONLY=1`) when `is_loopback_only`.
///
/// Example: `let command = make_game_server_command(&programs, 47850, 47852, false);`
pub fn make_game_server_command(
    programs: &Programs,
    match_port: u16,
    level_port: u16,
    is_loopback_only: bool,
) -> Command {
    let mut command: Command = Command::new(&programs.game_server);
    if is_loopback_only {
        command.env(network_scope::LOOPBACK_ONLY_VARIABLE, "1");
    }
    command
        .arg("--serve-match")
        .arg("--match-levels")
        .arg(&programs.levels_manifest)
        .args(["--match-port", &match_port.to_string()])
        .args(["--port", &level_port.to_string()]);
    command
}

/// `make_bot_command` gives the command of a bot (`plan`) joining the game server at `server`.
///
/// Example: `let command = make_bot_command(&programs, server, &plan);`
pub fn make_bot_command(programs: &Programs, server: SocketAddr, plan: &BotPlan) -> Command {
    let mut command: Command = Command::new(&programs.game_server);
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

/// `describe_game_kind` says in words which game `kind` is, for the frame times' headings.
///
/// Example: `assert_eq!(describe_game_kind(GameKind::Alone), "playing alone");`
pub fn describe_game_kind(kind: GameKind) -> String {
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

/// `make_run_folder` makes the folder of a run of `script` (`warped` for the children's logs,
/// `screen_pictures`): in the checkout's `.artifacts/<script>/<run_id>/`, or in
/// `~/Library/Logs/Warped/<script>/<run_id>/` when the app runs from `Warped.app`; it holds
/// `command.txt` (the command line and the commit, `app` inside the app).
///
/// Example: `let folder = make_run_folder("warped", "20261109-120000")?;`
pub fn make_run_folder(script: &str, run_id: &str) -> Result<PathBuf> {
    let app_logs: Option<PathBuf> = get_app_logs_folder();
    let folder: PathBuf = match &app_logs {
        Some(logs) => logs.join(script).join(run_id),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(CHECKOUT_ARTIFACTS_FOLDER)
            .join(script)
            .join(run_id),
    };
    fs::create_dir_all(&folder)
        .map_err(|error| format!("can't make {}: {error}", folder.display()))?;
    let commit: String = match app_logs {
        Some(_) => "app".to_string(),
        None => Command::new("git")
            .args(["describe", "--always", "--dirty"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .unwrap_or_else(|error| format!("unknown ({error})")),
    };
    let command: String = std::env::args().collect::<Vec<String>>().join(" ");
    fs::write(
        folder.join("command.txt"),
        format!("{command}\ncommit: {commit}\n"),
    )?;
    Ok(folder)
}

/// `is_group_alive` says whether a process of the process group `group_id` is still running
/// (it signals nothing: `kill -0`).
///
/// Example: `if is_group_alive(group_id)? { ... }`
fn is_group_alive(group_id: i32) -> Result<bool> {
    Ok(Command::new(KILL_PROGRAM)
        .args([NO_SIGNAL_FLAG, "--", &format!("-{group_id}")])
        .stderr(Stdio::null())
        .status()?
        .success())
}

/// `ChildGroup` is the process group of everything a hosted game starts, with its guard: a shell
/// that leads the group and waits on a pipe only the app writes to. When the app ends in any way
/// (a return, a panic, a crash, a kill), the pipe closes and the guard kills the whole group, so
/// no server or bot outlives the app.
struct ChildGroup {
    guard: Child,
    guard_pipe: Option<ChildStdin>,
}

impl ChildGroup {
    /// `Start` starts the guard, which leads a new process group.
    ///
    /// Example: `let group = ChildGroup::Start()?;`
    #[allow(non_snake_case)]
    fn Start() -> Result<ChildGroup> {
        let mut guard: Child = Command::new(GUARD_SHELL)
            .args(["-c", GUARD_SCRIPT])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|error| format!("can't start the guard of the game's processes ({error})"))?;
        let guard_pipe: Option<ChildStdin> = guard.stdin.take();
        Ok(ChildGroup { guard, guard_pipe })
    }

    /// `get_id` gives the group's id, for `Command::process_group`.
    ///
    /// Example: `command.process_group(group.get_id()?);`
    fn get_id(&self) -> Result<i32> {
        Ok(i32::try_from(self.guard.id())?)
    }

    /// `stop` closes the pipe, so the guard kills the group, and waits until the group is empty;
    /// it fails if a process of the group is still alive `GROUP_EMPTY_WAIT_MS` later. A second
    /// call does nothing.
    ///
    /// Example: `group.stop()?;`
    fn stop(&mut self) -> Result<()> {
        if self.guard_pipe.take().is_none() {
            return Ok(());
        }
        self.guard.wait()?;
        let group_id: i32 = self.get_id()?;
        let waited_from: Instant = Instant::now();
        while is_group_alive(group_id)? {
            if waited_from.elapsed() >= Duration::from_millis(GROUP_EMPTY_WAIT_MS) {
                return Err(format!(
                    "processes of the game's process group {group_id} are still alive \
                     {GROUP_EMPTY_WAIT_MS} ms after the app stopped it"
                )
                .into());
            }
            thread::sleep(Duration::from_millis(GROUP_POLL_MS));
        }
        Ok(())
    }
}

/// `spawn_logged` starts `command` in the process group `group_id` with its output in
/// `folder/<name>.log`.
///
/// Example: `let child = spawn_logged(command, group.get_id()?, &folder, "match_server")?;`
fn spawn_logged(mut command: Command, group_id: i32, folder: &Path, name: &str) -> Result<Child> {
    let log: fs::File = fs::File::create(folder.join(format!("{name}.log")))?;
    eprintln!("warped: starting {command:?}");
    Ok(command
        .process_group(group_id)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .map_err(|error| format!("can't start {name} ({error})"))?)
}

/// `HostedGame` is a game this app hosts: its kind, the game server's address, the game server
/// and the bots, the process group they all run in, and the folder of their logs.
pub struct HostedGame {
    pub kind: GameKind,
    pub server: SocketAddr,
    pub run_folder: PathBuf,
    programs: Programs,
    game_server: Option<Child>,
    bots: Vec<(BotPlan, Option<Child>)>,
    group: ChildGroup,
}

impl HostedGame {
    /// `Start` starts the game server on `ports` (match, level), on 127.0.0.1 only when
    /// `is_loopback_only`, and the bots of `kind` for a player on `player_team`, logging into
    /// `run_folder`.
    ///
    /// Example: `let game = HostedGame::Start(&programs, kind, Team::Blue, (47850, 47852), false, &folder)?;`
    #[allow(non_snake_case)]
    pub fn Start(
        programs: &Programs,
        kind: GameKind,
        player_team: Team,
        ports: (u16, u16),
        is_loopback_only: bool,
        run_folder: &Path,
    ) -> Result<HostedGame> {
        let server: SocketAddr = SocketAddr::from((Ipv4Addr::LOCALHOST, ports.0));
        let group: ChildGroup = ChildGroup::Start()?;
        let game_server: Child = spawn_logged(
            make_game_server_command(programs, ports.0, ports.1, is_loopback_only),
            group.get_id()?,
            run_folder,
            "match_server",
        )?;
        let mut game: HostedGame = HostedGame {
            kind,
            server,
            run_folder: run_folder.to_path_buf(),
            programs: programs.clone(),
            game_server: Some(game_server),
            bots: Vec::new(),
            group,
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
            self.group.get_id()?,
            &self.run_folder,
            &name,
        )?;
        self.bots.push((plan, Some(child)));
        Ok(())
    }

    /// `has_stopped` says, with its exit status, whether the game server stopped by itself (a
    /// port in use, a level that failed, or the match over).
    ///
    /// Example: `if let Some(status) = game.has_stopped()? { ... }`
    pub fn has_stopped(&mut self) -> Result<Option<String>> {
        match self.game_server.as_mut() {
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

    /// `stop` stops the bots and the game server, then the rest of their process group (the
    /// level server the game server started); it fails if a process of the group is still alive
    /// afterwards.
    ///
    /// Example: `game.stop()?;`
    pub fn stop(&mut self) -> Result<()> {
        for (_, child) in &mut self.bots {
            stop_child(child);
        }
        stop_child(&mut self.game_server);
        self.group.stop()
    }
}

impl Drop for HostedGame {
    /// `drop` stops the game's processes, so none outlives the app, and prints a failure to.
    ///
    /// Example: `drop(game);`
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("warped: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `children.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Game kind | hosted, alone (blue, red), bots (none: as alone; allies and enemies; allies only: a sitting-out enemy; the most) | `each_game_kind_plans_its_bots` |
    //! | Game kind in words | hosted, alone, with bots | `game_kinds_are_described` |
    //! | Game server command | ports and manifest passed; on the network (no variable), loopback only (`WARPED_LOOPBACK_ONLY=1`) | `the_game_server_serves_the_manifest_on_its_ports` |
    //! | Bot command | consoles, fight, sitting out (as many rounds as `SIT_OUT_ROUND_COUNT`, the other team named) | `bots_join_with_their_team_and_objective` |
    //! | How the game's processes end | the app stops the game (a level server left by its killed game server dies too), the app ends without stopping it (the guard's pipe closes) | `stopping_a_group_kills_a_grandchild`, `a_group_dies_with_the_app` |

    use super::*;
    use std::ffi::OsStr;
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::ExitStatusExt;

    const SLEEP_PROGRAM: &str = "/bin/sleep";
    const SLEEP_S: &str = "600";
    const SLEEP_SCRIPT: &str = "/bin/sleep 600 & echo $!; wait";
    const SIGKILL: i32 = 9;

    /// `is_process_alive` says whether the process `process_id` is still running.
    ///
    /// Example: `assert!(is_process_alive(grandchild_id));`
    fn is_process_alive(process_id: i32) -> bool {
        Command::new(KILL_PROGRAM)
            .args([NO_SIGNAL_FLAG, &process_id.to_string()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    #[test]
    fn stopping_a_group_kills_a_grandchild() {
        let mut group: ChildGroup = ChildGroup::Start().unwrap();
        let mut child: Child = Command::new(GUARD_SHELL)
            .args(["-c", SLEEP_SCRIPT])
            .stdout(Stdio::piped())
            .process_group(group.get_id().unwrap())
            .spawn()
            .unwrap();
        let mut line: String = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let grandchild_id: i32 = line.trim().parse().unwrap();
        stop_child(&mut Some(child));
        assert!(is_process_alive(grandchild_id));
        group.stop().unwrap();
        assert!(!is_group_alive(group.get_id().unwrap()).unwrap());
        assert!(!is_process_alive(grandchild_id));
        group.stop().unwrap();
    }

    #[test]
    fn a_group_dies_with_the_app() {
        let mut group: ChildGroup = ChildGroup::Start().unwrap();
        let mut child: Child = Command::new(SLEEP_PROGRAM)
            .arg(SLEEP_S)
            .process_group(group.get_id().unwrap())
            .spawn()
            .unwrap();
        drop(group.guard_pipe.take());
        assert_eq!(child.wait().unwrap().signal(), Some(SIGKILL));
        group.guard.wait().unwrap();
    }

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
            game_server: PathBuf::from("/app/warped-match"),
            levels_manifest: PathBuf::from("/app/match_levels.yaml"),
        }
    }

    #[test]
    fn game_kinds_are_described() {
        assert_eq!(describe_game_kind(GameKind::Host), "hosting a game");
        assert_eq!(describe_game_kind(GameKind::Alone), "playing alone");
        let bots: GameKind = GameKind::Bots {
            ally_count: 4,
            enemy_count: 5,
            objective: BotObjective::Fight,
        };
        assert_eq!(describe_game_kind(bots), "with 4 allied and 5 enemy bots");
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
    fn the_game_server_serves_the_manifest_on_its_ports() {
        let command: Command = make_game_server_command(&make_programs(), 48_304, 48_305, false);
        assert_eq!(command.get_envs().count(), 0);
        let loopback: Command = make_game_server_command(&make_programs(), 48_304, 48_305, true);
        let variable: (&OsStr, Option<&OsStr>) = loopback.get_envs().next().unwrap();
        assert_eq!(
            variable,
            ("WARPED_LOOPBACK_ONLY".as_ref(), Some("1".as_ref()))
        );
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
