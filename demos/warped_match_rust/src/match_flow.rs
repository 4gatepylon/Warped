//! `match_flow.rs` runs a match across the levels: the match server (`serve_match`) keeps the
//! `MatchState`, starts each round's level server (a separate program, see `LevelEntry`), reads
//! the level's winner back from the file the level writes, and tells the match clients
//! (`MatchClient`) what is on, so each client can start and stop its level window.
//!
//! Interface: `LevelEntry`, `LoadLevels`, `Phase`, `MatchView`, `serve_match`, `MatchClient`
//! (`Connect`, `poll`), `make_level_client_command`, `MATCH_PORT`.
//!
//! Example: `serve_match(&levels, &MatchTimes::default(), &run_folder, MATCH_PORT, 47852)?;`
//!
//! Must know:
//! - The protocol is one text datagram from each client every `HELLO_PERIOD_S`, `hello <team>`
//!   (`blue`, `red` or `any`), answered with the `MatchView` as JSON. A client silent for
//!   `CLIENT_TIMEOUT_S` no longer counts.
//! - A level program is started as `<program> --serve --port <port> [--map <map>]
//!   --level-result-file <path> <server_args>` and joined as `<program> --join <address> --port
//!   <port> [--map <map>] --team <team> <client_args>` (README, "Level interface").

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::match_state::{MatchState, Outcome, Team};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const MATCH_PORT: u16 = 47_850;
const HELLO_PERIOD_S: f64 = 0.5;
const CLIENT_TIMEOUT_S: f64 = 3.0;
const SOCKET_TIMEOUT_MS: u64 = 50;
const MAX_DATAGRAM_BYTES: usize = 65_000;
const SELF_PROGRAM: &str = "self";

/// `LevelEntry` is one level of the match's map in `match_levels.yaml`: its name, the demo folder
/// and branch it comes from, whether it is playable yet (`status`), and how to run it: `program`
/// (`self` for this demo's own binary, else a path from this demo's folder), the `--map` it
/// takes, and extra flags for its server and its clients.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LevelEntry {
    pub name: String,
    pub folder: String,
    pub branch: String,
    pub status: String,
    pub program: String,
    pub map: Option<String>,
    #[serde(default)]
    pub server_args: Vec<String>,
    #[serde(default)]
    pub client_args: Vec<String>,
    #[serde(default)]
    pub stand_in_for: Option<String>,
}

/// `LevelFile` is `match_levels.yaml`: the map's levels, blue's deepest first.
#[derive(Deserialize)]
struct LevelFile {
    levels: Vec<LevelEntry>,
}

/// `LoadLevels` reads the match's levels from a `match_levels.yaml` and checks there are an odd
/// number, at least 3.
///
/// Example: `let levels = LoadLevels(Path::new("match_levels.yaml"))?;`
#[allow(non_snake_case)]
pub fn LoadLevels(path: &Path) -> Result<Vec<LevelEntry>> {
    let text: String = fs::read_to_string(path)
        .map_err(|error| format!("can't read the levels file {}: {error}", path.display()))?;
    let file: LevelFile = serde_yaml::from_str(&text)
        .map_err(|error| format!("{} isn't a levels file: {error}", path.display()))?;
    if file.levels.len() < 3 || file.levels.len().is_multiple_of(2) {
        return Err(format!(
            "{} has {} levels; a match needs an odd number, at least 3",
            path.display(),
            file.levels.len()
        )
        .into());
    }
    Ok(file.levels)
}

/// `Phase` is what the match is doing: waiting for a player on each team, a pause before the next
/// round, a round being played (on the level server's `port`), the pause after a round's win
/// while its level still shows the win, or over.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum Phase {
    WaitingForTeams,
    Intermission {
        left_s: f64,
    },
    Playing {
        round: usize,
        level: usize,
        port: u16,
    },
    RoundOver {
        round: usize,
        left_s: f64,
    },
    Over,
}

/// `MatchView` is everything a match client shows: the match state, the phase, the levels, how
/// many clients each team has, and the match's time left (`None` before its first round).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct MatchView {
    pub state: MatchState,
    pub phase: Phase,
    pub levels: Vec<LevelEntry>,
    pub team_counts: [u32; 2],
    pub match_left_s: Option<f64>,
}

/// `MatchTimes` is how long the pauses and the match last.
#[derive(Clone, Copy, Debug)]
pub struct MatchTimes {
    pub intermission_s: f64,
    pub round_over_s: f64,
    pub match_s: f64,
}

impl Default for MatchTimes {
    fn default() -> MatchTimes {
        MatchTimes {
            intermission_s: 10.0,
            round_over_s: 5.0,
            match_s: 30.0 * 60.0,
        }
    }
}

/// `resolve_program` gives the program a level runs: this binary for `self`, else the path from
/// this demo's folder.
///
/// Example: `let program = resolve_program(&level)?;`
fn resolve_program(level: &LevelEntry) -> Result<PathBuf> {
    if level.program == SELF_PROGRAM {
        return Ok(std::env::current_exe()?);
    }
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join(&level.program);
    if !path.exists() {
        return Err(format!(
            "{}'s program {} doesn't exist; build it in its folder ({}) first",
            level.name,
            path.display(),
            level.folder
        )
        .into());
    }
    Ok(path)
}

/// `start_level_server` starts `level`'s server on `port`, writing its winner to `result_path`
/// and its output to `log_path`.
///
/// Example: `let child = start_level_server(&level, 47852, &result_path, &log_path)?;`
fn start_level_server(
    level: &LevelEntry,
    port: u16,
    result_path: &Path,
    log_path: &Path,
) -> Result<Child> {
    let mut command: Command = Command::new(resolve_program(level)?);
    command.args(["--serve", "--port", &port.to_string()]);
    if let Some(map) = &level.map {
        command.args(["--map", map]);
    }
    command.arg("--level-result-file").arg(result_path);
    command.args(&level.server_args);
    let log: fs::File = fs::File::create(log_path)?;
    println!("round level server: {command:?}");
    Ok(command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()?)
}

/// `make_level_client_command` gives the command that joins `level`'s server at `address`:`port`
/// on `team`, adding `extra_args` (e.g. a bot's flags).
///
/// Example: `let command = make_level_client_command(&level, ip, 47852, Team::Blue, &[])?;`
pub fn make_level_client_command(
    level: &LevelEntry,
    address: std::net::IpAddr,
    port: u16,
    team: Team,
    extra_args: &[String],
) -> Result<Command> {
    let mut command: Command = Command::new(resolve_program(level)?);
    command.args(["--join", &address.to_string(), "--port", &port.to_string()]);
    if let Some(map) = &level.map {
        command.args(["--map", map]);
    }
    command.args(["--team", team.get_name()]);
    command.args(&level.client_args);
    command.args(extra_args);
    Ok(command)
}

/// `read_level_result` gives the winner a level wrote to `path`, or `None` before it has.
///
/// Example: `if let Some(winner) = read_level_result(&path)? { ... }`
fn read_level_result(path: &Path) -> Result<Option<Team>> {
    #[derive(Deserialize)]
    struct LevelResult {
        winner: Team,
    }
    match fs::read_to_string(path) {
        Ok(text) => {
            let result: LevelResult = serde_json::from_str(&text)
                .map_err(|error| format!("{} isn't a level result: {error}", path.display()))?;
            Ok(Some(result.winner))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// `stop_child` kills a level process and waits for it to end.
///
/// Example: `stop_child(&mut level_server);`
pub fn stop_child(child: &mut Option<Child>) {
    if let Some(mut process) = child.take() {
        let _ = process.kill();
        let _ = process.wait();
    }
}

/// `parse_hello` gives the team a client's `hello <team>` datagram names (`None` for `any`), or
/// an error for any other datagram.
///
/// Example: `assert_eq!(parse_hello(b"hello red")?, Some(Team::Red));`
fn parse_hello(datagram: &[u8]) -> Result<Option<Team>> {
    match datagram {
        b"hello blue" => Ok(Some(Team::Blue)),
        b"hello red" => Ok(Some(Team::Red)),
        b"hello any" => Ok(None),
        _ => Err("not a hello".into()),
    }
}

/// `serve_match` runs one match on `levels` until it ends and `times.round_over_s` more (so the
/// clients see the result), on UDP `match_port`, each round's level server on `level_port`, its
/// results and logs in `run_folder`. A round starts `times.intermission_s` after a player is on
/// each team; the match ends in a tie `times.match_s` after its first round started.
///
/// Example: `serve_match(&levels, &MatchTimes::default(), &folder, MATCH_PORT, 47852)?;`
pub fn serve_match(
    levels: &[LevelEntry],
    times: &MatchTimes,
    run_folder: &Path,
    match_port: u16,
    level_port: u16,
) -> Result<MatchState> {
    let socket: UdpSocket = UdpSocket::bind(("0.0.0.0", match_port))
        .map_err(|error| format!("can't use UDP port {match_port} for the match: {error}"))?;
    socket.set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))?;
    let mut state: MatchState = MatchState::New(levels.len());
    let mut phase: Phase = Phase::WaitingForTeams;
    let mut phase_ends_at: Option<Instant> = None;
    let mut match_started_at: Option<Instant> = None;
    let mut over_at: Option<Instant> = None;
    let mut clients: Vec<(SocketAddr, Option<Team>, Instant)> = Vec::new();
    let mut level_server: Option<Child> = None;
    let mut buffer: Vec<u8> = vec![0; MAX_DATAGRAM_BYTES];
    println!("Match server on UDP port {match_port}; levels on port {level_port}.");
    loop {
        let received: Option<(usize, SocketAddr)> = match socket.recv_from(&mut buffer) {
            Ok(received) => Some(received),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                None
            }
            Err(error) => return Err(error.into()),
        };
        clients.retain(|client| client.2.elapsed().as_secs_f64() < CLIENT_TIMEOUT_S);
        if let Some((length, address)) = received
            && let Ok(team) = parse_hello(&buffer[..length])
        {
            clients.retain(|client| client.0 != address);
            clients.push((address, team, Instant::now()));
        }
        let team_counts: [u32; 2] = [Team::Blue, Team::Red].map(|team| {
            clients
                .iter()
                .filter(|client| client.1 == Some(team))
                .count() as u32
        });
        let left_s = |at: Option<Instant>| {
            at.map_or(0.0, |at| {
                at.saturating_duration_since(Instant::now()).as_secs_f64()
            })
        };
        let match_left_s: Option<f64> =
            match_started_at.map(|at| (times.match_s - at.elapsed().as_secs_f64()).max(0.0));
        if state.outcome().is_none() && match_left_s == Some(0.0) {
            println!("The match's time ran out: a tie.");
            state.end_in_tie();
            stop_child(&mut level_server);
            phase = Phase::Over;
        }
        phase = match phase {
            Phase::WaitingForTeams if team_counts.iter().all(|&count| count > 0) => {
                phase_ends_at =
                    Some(Instant::now() + Duration::from_secs_f64(times.intermission_s));
                Phase::Intermission {
                    left_s: times.intermission_s,
                }
            }
            Phase::Intermission { .. } if left_s(phase_ends_at) == 0.0 => {
                let level: usize = state
                    .get_next_level()
                    .expect("an unfinished match has a next round");
                let round: usize = state.rounds().len() + 1;
                let result_path: PathBuf = run_folder.join(format!("round_{round}.json"));
                let log_path: PathBuf = run_folder.join(format!("round_{round}_server.log"));
                level_server = Some(start_level_server(
                    &levels[level],
                    level_port,
                    &result_path,
                    &log_path,
                )?);
                match_started_at.get_or_insert_with(Instant::now);
                println!(
                    "Round {round}: {}, attacker {:?}",
                    levels[level].name,
                    state.get_attacker()
                );
                Phase::Playing {
                    round,
                    level,
                    port: level_port,
                }
            }
            Phase::Intermission { .. } => Phase::Intermission {
                left_s: left_s(phase_ends_at),
            },
            Phase::Playing { round, level, port } => {
                let result_path: PathBuf = run_folder.join(format!("round_{round}.json"));
                if let Some(status) = level_server
                    .as_mut()
                    .and_then(|child| child.try_wait().ok().flatten())
                {
                    return Err(format!(
                        "round {round}'s level server stopped ({status}); see {}",
                        run_folder.display()
                    )
                    .into());
                }
                match read_level_result(&result_path)? {
                    Some(winner) => {
                        state.record_round(winner);
                        println!(
                            "Round {round}: {} won {}",
                            winner.get_name(),
                            levels[level].name
                        );
                        phase_ends_at =
                            Some(Instant::now() + Duration::from_secs_f64(times.round_over_s));
                        Phase::RoundOver {
                            round,
                            left_s: times.round_over_s,
                        }
                    }
                    None => Phase::Playing { round, level, port },
                }
            }
            Phase::RoundOver { round, .. } if left_s(phase_ends_at) == 0.0 => {
                stop_child(&mut level_server);
                if let Some(outcome) = state.outcome() {
                    println!("The match is over: {outcome:?}");
                    Phase::Over
                } else {
                    let _ = round;
                    phase_ends_at =
                        Some(Instant::now() + Duration::from_secs_f64(times.intermission_s));
                    Phase::Intermission {
                        left_s: times.intermission_s,
                    }
                }
            }
            Phase::RoundOver { round, .. } => Phase::RoundOver {
                round,
                left_s: left_s(phase_ends_at),
            },
            other => other,
        };
        if let Some((_, address)) = received {
            let view: MatchView = MatchView {
                state: state.clone(),
                phase: phase.clone(),
                levels: levels.to_vec(),
                team_counts,
                match_left_s,
            };
            socket.send_to(&serde_json::to_vec(&view)?, address)?;
        }
        if phase == Phase::Over {
            let at: Instant = *over_at.get_or_insert_with(Instant::now);
            if at.elapsed().as_secs_f64() >= times.round_over_s + CLIENT_TIMEOUT_S {
                return Ok(state);
            }
        }
    }
}

/// `MatchClient` is a connection to a match server: the team it says it plays for and the last
/// `MatchView` the server sent.
pub struct MatchClient {
    socket: UdpSocket,
    pub server: SocketAddr,
    pub team: Option<Team>,
    pub view: Option<MatchView>,
    last_hello: Option<Instant>,
    buffer: Vec<u8>,
}

impl MatchClient {
    /// `Connect` makes a client of the match server at `server` playing for `team` (`None` until
    /// the player picks one).
    ///
    /// Example: `let client = MatchClient::Connect(server, Some(Team::Blue))?;`
    #[allow(non_snake_case)]
    pub fn Connect(server: SocketAddr, team: Option<Team>) -> Result<MatchClient> {
        let socket: UdpSocket = UdpSocket::bind(("0.0.0.0", 0))?;
        socket.set_nonblocking(true)?;
        Ok(MatchClient {
            socket,
            server,
            team,
            view: None,
            last_hello: None,
            buffer: vec![0; MAX_DATAGRAM_BYTES],
        })
    }

    /// `poll` says hello every `HELLO_PERIOD_S` and keeps the newest view the server sent.
    ///
    /// Example: `client.poll()?;`
    pub fn poll(&mut self) -> Result<()> {
        if self
            .last_hello
            .is_none_or(|at| at.elapsed().as_secs_f64() >= HELLO_PERIOD_S)
        {
            let team: &str = self.team.map_or("any", Team::get_name);
            self.socket
                .send_to(format!("hello {team}").as_bytes(), self.server)?;
            self.last_hello = Some(Instant::now());
        }
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((length, address)) if address == self.server => {
                    self.view = Some(serde_json::from_slice(&self.buffer[..length])?);
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == ErrorKind::ConnectionRefused => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
    }
}

/// `describe_outcome` gives a finished match's result for a line of text.
///
/// Example: `let text = describe_outcome(Outcome::Tie);`
pub fn describe_outcome(outcome: Outcome) -> String {
    match outcome {
        Outcome::Won(team) => format!("{} wins the match", team.get_name()),
        Outcome::Tie => "a tie (the match's time ran out)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `match_flow.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Levels file | the shipped one, an even count | `the_shipped_levels_file_loads`, `an_even_level_count_is_refused` |
    //! | Level result | none yet, blue, red, garbage | `level_results_are_read` |
    //! | Hello | each team, any, garbage | `hellos_name_a_team` |

    use super::*;

    #[test]
    fn the_shipped_levels_file_loads() {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("match_levels.yaml");
        let levels: Vec<LevelEntry> = LoadLevels(&path).expect("the shipped levels load");
        let names: Vec<&str> = levels.iter().map(|level| level.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Dust",
                "Snow Planet",
                "City Sphere",
                "Dark Forest",
                "Lunar Surface"
            ]
        );
    }

    #[test]
    fn an_even_level_count_is_refused() {
        let path: PathBuf =
            std::env::temp_dir().join(format!("even_levels_{}.yaml", std::process::id()));
        let level: &str =
            "  - {name: A, folder: a, branch: b, status: s, program: self, map: lunar-surface}\n";
        fs::write(&path, format!("levels:\n{}", level.repeat(4))).unwrap();
        assert!(LoadLevels(&path).is_err());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn level_results_are_read() {
        let path: PathBuf =
            std::env::temp_dir().join(format!("level_result_{}.json", std::process::id()));
        assert_eq!(read_level_result(&path).unwrap(), None);
        for (text, team) in [
            ("{\"winner\": \"blue\"}", Team::Blue),
            ("{\"winner\": \"red\"}\n", Team::Red),
        ] {
            fs::write(&path, text).unwrap();
            assert_eq!(read_level_result(&path).unwrap(), Some(team));
        }
        fs::write(&path, "{\"winner\": \"green\"}").unwrap();
        assert!(read_level_result(&path).is_err());
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn hellos_name_a_team() {
        assert_eq!(parse_hello(b"hello blue").unwrap(), Some(Team::Blue));
        assert_eq!(parse_hello(b"hello red").unwrap(), Some(Team::Red));
        assert_eq!(parse_hello(b"hello any").unwrap(), None);
        assert!(parse_hello(b"join").is_err());
    }
}
