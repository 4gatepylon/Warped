//! `match_flow.rs` runs a match across the levels: the match server (`serve_match`) keeps the
//! `MatchState`, starts each round's level server (a separate program, see `LevelEntry`), reads
//! the level's winner back from the file the level writes, and tells the match clients
//! (`MatchClient`) what is on, so each client can start and stop its level window.
//!
//! Interface: `LevelEntry`, `LoadLevels`, `find_level`, `Phase`, `MatchView`, `MatchServerOptions`,
//! `serve_match`, `MatchClient` (`Connect`, `poll`), `MatchPasswordRefused`,
//! `make_level_client_command`, `MATCH_PORT`.
//!
//! Example: `serve_match(&options)?;`
//!
//! Must know:
//! - The protocol is one text datagram from each client every `HELLO_PERIOD_S`. A client first
//!   says `hello <team>` (`blue`, `red` or `any`), padded to `MIN_HELLO_BYTES`, and the server
//!   answers `challenge <cookie>`, a keyed hash of the client's address. From then on the client
//!   says `hello <team> <cookie> <proof>`, the proof a keyed hash of the cookie with the game's
//!   password, and the server answers with the `MatchView` as JSON, or `refused` for a wrong
//!   password. So the server sends its large view only to an address that received the cookie
//!   and knows the password. A client silent for `CLIENT_TIMEOUT_S` no longer counts.
//! - A level's program paths are relative to the folder of the levels file that names them, and
//!   each side reads its own levels file: the server starts `server_program`, the clients
//!   `program` (README, "Level interface").
//! - A level server is started as `<server_program> --port <port> [--map <map>]
//!   --level-result-file <path> [--game-password-file <path>] <server_args>`, or without a
//!   `server_program` as `<program> --serve ...` with the same flags; it is joined as `<program>
//!   --join <address> --port <port> [--map <map>] --team <team> <client_args>`.

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::match_state::{MatchState, Outcome, Team};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const MATCH_PORT: u16 = 47_850;
const HELLO_PERIOD_S: f64 = 0.5;
const CLIENT_TIMEOUT_S: f64 = 3.0;
const SOCKET_TIMEOUT_MS: u64 = 50;
const MAX_DATAGRAM_BYTES: usize = 65_000;
const MIN_HELLO_BYTES: usize = 64;
const BYTES_PER_COOKIE: usize = 16;
const BYTES_PER_SECRET: usize = 32;
const CHALLENGE_PREFIX: &str = "challenge ";
const REFUSED: &str = "refused";
pub const SELF_PROGRAM: &str = "self";

/// `LevelEntry` is one level of the match's map in a levels file: its name, the demo folder and
/// branch it comes from, whether it is playable yet (`status`), and how to run it: `program`, the
/// window players join with (`self` for this demo's own binary, else a path from the levels file's
/// folder), `server_program`, a separate program for its server (a path from the same folder; a
/// level without one serves with `program --serve`), the `--map` it takes, extra flags for its
/// server and its clients, and its map picture for the progression page (`map_picture`, a PNG path
/// from the same folder).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LevelEntry {
    pub name: String,
    pub folder: String,
    pub branch: String,
    pub status: String,
    pub program: String,
    #[serde(default)]
    pub server_program: Option<String>,
    pub map: Option<String>,
    #[serde(default)]
    pub server_args: Vec<String>,
    #[serde(default)]
    pub client_args: Vec<String>,
    #[serde(default)]
    pub stand_in_for: Option<String>,
    /// Whether the level's bots take `--label` (the name of their artifact folder).
    #[serde(default = "get_true")]
    pub takes_bot_label: bool,
    #[serde(default)]
    pub map_picture: Option<String>,
}

/// `get_true` is serde's default for flags that are on unless a level turns them off.
fn get_true() -> bool {
    true
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

/// `find_level` gives the level called `name` in `levels`, so a client plays a round with its own
/// copy of the level the server names.
///
/// Example: `let level = find_level(&my_levels, &view.levels[index].name)?;`
pub fn find_level<'a>(levels: &'a [LevelEntry], name: &str) -> Result<&'a LevelEntry> {
    levels
        .iter()
        .find(|level| level.name == name)
        .ok_or_else(|| {
            format!("the match plays a level called {name:?}, which this game doesn't have; get the newest version").into()
        })
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

/// `resolve_program` gives the path of `program`, one of `level`'s programs: this binary for
/// `self`, else the path from `levels_folder`, the folder of the levels file.
///
/// Example: `let program = resolve_program(&level, &level.program, levels_folder)?;`
fn resolve_program(level: &LevelEntry, program: &str, levels_folder: &Path) -> Result<PathBuf> {
    if program == SELF_PROGRAM {
        return Ok(std::env::current_exe()?);
    }
    let path: PathBuf = levels_folder.join(program);
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

/// `start_level_server` starts `level`'s server on `port` (its `server_program`, else its
/// `program` with `--serve`), writing its winner to `result_path` and its output to `log_path`,
/// and admitting only players who know the password in `game_password_file`, if any.
///
/// Example: `let child = start_level_server(&level, folder, 47852, &result, &log, None)?;`
fn start_level_server(
    level: &LevelEntry,
    levels_folder: &Path,
    port: u16,
    result_path: &Path,
    log_path: &Path,
    game_password_file: Option<&Path>,
) -> Result<Child> {
    let mut command: Command = match &level.server_program {
        Some(program) => Command::new(resolve_program(level, program, levels_folder)?),
        None => {
            let mut command = Command::new(resolve_program(level, &level.program, levels_folder)?);
            command.arg("--serve");
            command
        }
    };
    command.args(["--port", &port.to_string()]);
    if let Some(map) = &level.map {
        command.args(["--map", map]);
    }
    command.arg("--level-result-file").arg(result_path);
    if let Some(path) = game_password_file {
        command.arg("--game-password-file").arg(path);
    }
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
/// on `team`, adding `extra_args` (e.g. a bot's flags), its program a path from `levels_folder`.
///
/// Example: `let command = make_level_client_command(&level, folder, ip, 47852, Team::Blue, &[])?;`
pub fn make_level_client_command(
    level: &LevelEntry,
    levels_folder: &Path,
    address: std::net::IpAddr,
    port: u16,
    team: Team,
    extra_args: &[String],
) -> Result<Command> {
    let mut command: Command = Command::new(resolve_program(level, &level.program, levels_folder)?);
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

/// `Hello` is a client's datagram to the match server: asking for a cookie, or proving the game's
/// password with one.
#[derive(Debug, PartialEq)]
enum Hello {
    Ask,
    Proven {
        team: Option<Team>,
        cookie: Vec<u8>,
        proof: Vec<u8>,
    },
}

/// `parse_team` gives the team a hello names (`None` for `any`).
///
/// Example: `assert_eq!(parse_team("red")?, Some(Team::Red));`
fn parse_team(word: &str) -> Result<Option<Team>> {
    match word {
        "blue" => Ok(Some(Team::Blue)),
        "red" => Ok(Some(Team::Red)),
        "any" => Ok(None),
        _ => Err(format!("a hello names team blue, red or any, not {word:?}").into()),
    }
}

/// `parse_hello` reads a client's hello: `hello <team>`, padded to at least `MIN_HELLO_BYTES`, or
/// `hello <team> <cookie> <proof>` in hexadecimal; anything else is an error.
///
/// Example: `let hello = parse_hello(format_hello(Some(Team::Red), None).as_bytes())?;`
fn parse_hello(datagram: &[u8]) -> Result<Hello> {
    if datagram.len() < MIN_HELLO_BYTES {
        return Err(format!("a hello has at least {MIN_HELLO_BYTES} bytes").into());
    }
    let words: Vec<&str> = std::str::from_utf8(datagram)?.split_whitespace().collect();
    match words.as_slice() {
        ["hello", team] => parse_team(team).map(|_| Hello::Ask),
        ["hello", team, cookie, proof] => Ok(Hello::Proven {
            team: parse_team(team)?,
            cookie: decode_hex(cookie)?,
            proof: decode_hex(proof)?,
        }),
        _ => Err("not a hello".into()),
    }
}

/// `format_hello` gives a client's hello for `team`: asking for a cookie, or proving the password
/// with `cookie_and_proof`; padded to `MIN_HELLO_BYTES`, so the challenge it gets back is smaller.
///
/// Example: `let hello = format_hello(Some(Team::Blue), None);`
fn format_hello(team: Option<Team>, cookie_and_proof: Option<(&[u8], &[u8])>) -> String {
    let team_name: &str = team.map_or("any", Team::get_name);
    let text: String = match cookie_and_proof {
        Some((cookie, proof)) => {
            format!(
                "hello {team_name} {} {}",
                encode_hex(cookie),
                encode_hex(proof)
            )
        }
        None => format!("hello {team_name}"),
    };
    format!("{text:<MIN_HELLO_BYTES$}")
}

/// `make_cookie` gives the server's cookie for a client at `address`, a keyed hash with the
/// server's random `secret`, so only that address gets it and the server keeps no state for it.
///
/// Example: `let cookie = make_cookie(&secret, address);`
fn make_cookie(secret: &[u8], address: SocketAddr) -> Vec<u8> {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(secret).expect("HMAC takes a key of any length");
    mac.update(address.to_string().as_bytes());
    mac.finalize().into_bytes()[..BYTES_PER_COOKIE].to_vec()
}

/// `make_proof` gives a client's proof that it knows `password`: a keyed hash of `cookie`.
///
/// Example: `let proof = make_proof(b"correct horse", &cookie);`
fn make_proof(password: &[u8], cookie: &[u8]) -> Vec<u8> {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(password).expect("HMAC takes a key of any length");
    mac.update(cookie);
    mac.finalize().into_bytes().to_vec()
}

/// `is_proof_right` says whether `proof` proves `password` for `cookie`, in constant time.
///
/// Example: `if is_proof_right(password, &cookie, &proof) { ... }`
fn is_proof_right(password: &[u8], cookie: &[u8], proof: &[u8]) -> bool {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(password).expect("HMAC takes a key of any length");
    mac.update(cookie);
    mac.verify_slice(proof).is_ok()
}

/// `is_same_cookie` says whether two cookies are equal, in constant time.
///
/// Example: `if is_same_cookie(&sent, &expected) { ... }`
fn is_same_cookie(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0, |diff, (a, b)| diff | (a ^ b))
            == 0
}

/// `encode_hex` gives `bytes` as lowercase hexadecimal.
///
/// Example: `assert_eq!(encode_hex(&[0, 255]), "00ff");`
fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `decode_hex` gives the bytes of hexadecimal `text`, or an error for anything else.
///
/// Example: `assert_eq!(decode_hex("00ff")?, [0, 255]);`
fn decode_hex(text: &str) -> Result<Vec<u8>> {
    if !text.is_ascii() || !text.len().is_multiple_of(2) {
        return Err(format!("{text:?} isn't hexadecimal bytes").into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16)
                .map_err(|error| format!("{text:?} isn't hexadecimal bytes: {error}").into())
        })
        .collect()
}

/// `send_to_client` sends `datagram` to the client at `address`; a failed send only prints why,
/// so one unreachable or forged address can't stop the match.
///
/// Example: `send_to_client(&socket, REFUSED.as_bytes(), address);`
fn send_to_client(socket: &UdpSocket, datagram: &[u8], address: SocketAddr) {
    if let Err(error) = socket.send_to(datagram, address) {
        eprintln!("can't send to {address}: {error}");
    }
}

/// `MatchPasswordRefused` is the error a match client gets when the match server refuses its
/// game password.
#[derive(Debug)]
pub struct MatchPasswordRefused(pub SocketAddr);

impl std::fmt::Display for MatchPasswordRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "the match server at {} refused the game password",
            self.0
        )
    }
}

impl Error for MatchPasswordRefused {}

/// `MatchServerOptions` is what a match server needs: the levels and the folder of the levels file
/// that names them, the times, the folder for each round's result and log, the match's UDP port
/// and its levels' port, and the game's password and the file that holds it, which each level
/// server gets.
pub struct MatchServerOptions<'a> {
    pub levels: &'a [LevelEntry],
    pub levels_folder: &'a Path,
    pub times: MatchTimes,
    pub run_folder: &'a Path,
    pub match_port: u16,
    pub level_port: u16,
    pub game_password: &'a [u8],
    pub game_password_file: Option<&'a Path>,
}

/// `serve_match` runs one match on `options.levels` until it ends and `round_over_s` more (so
/// the clients see the result), on UDP `match_port`, each round's level server on `level_port`,
/// its results and logs in `run_folder`. A round starts `intermission_s` after a player is on each
/// team; the match ends in a tie `match_s` after its first round started.
///
/// Example: `serve_match(&options)?;`
pub fn serve_match(options: &MatchServerOptions) -> Result<MatchState> {
    let MatchServerOptions {
        levels,
        levels_folder,
        times,
        run_folder,
        match_port,
        level_port,
        game_password,
        game_password_file,
    } = *options;
    let socket: UdpSocket = UdpSocket::bind(("0.0.0.0", match_port))
        .map_err(|error| format!("can't use UDP port {match_port} for the match: {error}"))?;
    socket.set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))?;
    let mut secret: [u8; BYTES_PER_SECRET] = [0; BYTES_PER_SECRET];
    getrandom::fill(&mut secret).map_err(|error| format!("no random bytes: {error}"))?;
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
        let mut proven_address: Option<SocketAddr> = None;
        if let Some((length, address)) = received
            && let Ok(hello) = parse_hello(&buffer[..length])
        {
            let cookie: Vec<u8> = make_cookie(&secret, address);
            match hello {
                Hello::Proven {
                    team,
                    cookie: sent_cookie,
                    proof,
                } if is_same_cookie(&sent_cookie, &cookie) => {
                    if is_proof_right(game_password, &cookie, &proof) {
                        clients.retain(|client| client.0 != address);
                        clients.push((address, team, Instant::now()));
                        proven_address = Some(address);
                    } else {
                        send_to_client(&socket, REFUSED.as_bytes(), address);
                    }
                }
                Hello::Ask | Hello::Proven { .. } => {
                    let challenge: String = format!("{CHALLENGE_PREFIX}{}", encode_hex(&cookie));
                    send_to_client(&socket, challenge.as_bytes(), address);
                }
            }
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
                    levels_folder,
                    level_port,
                    &result_path,
                    &log_path,
                    game_password_file,
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
        if let Some(address) = proven_address {
            let view: MatchView = MatchView {
                state: state.clone(),
                phase: phase.clone(),
                levels: levels.to_vec(),
                team_counts,
                match_left_s,
            };
            send_to_client(&socket, &serde_json::to_vec(&view)?, address);
        }
        if phase == Phase::Over {
            let at: Instant = *over_at.get_or_insert_with(Instant::now);
            if at.elapsed().as_secs_f64() >= times.round_over_s + CLIENT_TIMEOUT_S {
                return Ok(state);
            }
        }
    }
}

/// `MatchClient` is a connection to a match server: the team it says it plays for, the game's
/// password, the cookie the server gave it, and the last `MatchView` the server sent.
pub struct MatchClient {
    socket: UdpSocket,
    pub server: SocketAddr,
    pub team: Option<Team>,
    pub view: Option<MatchView>,
    game_password: Vec<u8>,
    cookie: Option<Vec<u8>>,
    last_hello: Option<Instant>,
    buffer: Vec<u8>,
}

impl MatchClient {
    /// `Connect` makes a client of the match server at `server` playing for `team` (`None` until
    /// the player picks one), proving it knows `game_password` (empty for none).
    ///
    /// Example: `let client = MatchClient::Connect(server, Some(Team::Blue), Vec::new())?;`
    #[allow(non_snake_case)]
    pub fn Connect(
        server: SocketAddr,
        team: Option<Team>,
        game_password: Vec<u8>,
    ) -> Result<MatchClient> {
        let socket: UdpSocket = UdpSocket::bind(("0.0.0.0", 0))?;
        socket.set_nonblocking(true)?;
        Ok(MatchClient {
            socket,
            server,
            team,
            view: None,
            game_password,
            cookie: None,
            last_hello: None,
            buffer: vec![0; MAX_DATAGRAM_BYTES],
        })
    }

    /// `poll` says hello every `HELLO_PERIOD_S` (at once after a new cookie), keeps the server's
    /// cookie and the newest view it sent, and fails with `MatchPasswordRefused` if the server
    /// refuses the password.
    ///
    /// Example: `client.poll()?;`
    pub fn poll(&mut self) -> Result<()> {
        if self
            .last_hello
            .is_none_or(|at| at.elapsed().as_secs_f64() >= HELLO_PERIOD_S)
        {
            let hello: String = match &self.cookie {
                Some(cookie) => {
                    let proof: Vec<u8> = make_proof(&self.game_password, cookie);
                    format_hello(self.team, Some((cookie, &proof)))
                }
                None => format_hello(self.team, None),
            };
            self.socket.send_to(hello.as_bytes(), self.server)?;
            self.last_hello = Some(Instant::now());
        }
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((length, address)) if address == self.server => {
                    let datagram: &[u8] = &self.buffer[..length];
                    if let Some(cookie) = datagram.strip_prefix(CHALLENGE_PREFIX.as_bytes()) {
                        self.cookie = Some(decode_hex(std::str::from_utf8(cookie)?)?);
                        self.last_hello = None;
                    } else if datagram == REFUSED.as_bytes() {
                        return Err(Box::new(MatchPasswordRefused(self.server)));
                    } else {
                        self.view = Some(serde_json::from_slice(datagram)?);
                    }
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
    //! | Hello | asking for each team and any, proving, too short, bad team, garbage | `hellos_ask_or_prove` |
    //! | Proof | right password, wrong password, other cookie; cookies per address | `proofs_check_the_password` |
    //! | Hexadecimal | round trip, odd length, not hexadecimal, not ASCII | `hex_round_trips` |
    //! | Level lookup | a level by name, a missing one | `levels_are_found_by_name` |

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
    fn hellos_ask_or_prove() {
        for team in [Some(Team::Blue), Some(Team::Red), None] {
            let hello: String = format_hello(team, None);
            assert_eq!(hello.len(), MIN_HELLO_BYTES);
            assert_eq!(parse_hello(hello.as_bytes()).unwrap(), Hello::Ask);
            let proven: String = format_hello(team, Some((&[1, 2], &[3, 255])));
            assert_eq!(
                parse_hello(proven.as_bytes()).unwrap(),
                Hello::Proven {
                    team,
                    cookie: vec![1, 2],
                    proof: vec![3, 255]
                }
            );
        }
        assert!(parse_hello(b"hello blue").is_err());
        assert!(parse_hello(format!("{:<64}", "hello green").as_bytes()).is_err());
        assert!(parse_hello(format!("{:<64}", "join").as_bytes()).is_err());
    }

    #[test]
    fn proofs_check_the_password() {
        let address: SocketAddr = "203.0.113.5:50000".parse().unwrap();
        let other: SocketAddr = "203.0.113.5:50001".parse().unwrap();
        let cookie: Vec<u8> = make_cookie(b"secret", address);
        assert_eq!(cookie.len(), BYTES_PER_COOKIE);
        assert!(!is_same_cookie(&cookie, &make_cookie(b"secret", other)));
        assert!(is_same_cookie(&cookie, &make_cookie(b"secret", address)));
        let proof: Vec<u8> = make_proof(b"correct horse", &cookie);
        assert!(is_proof_right(b"correct horse", &cookie, &proof));
        assert!(!is_proof_right(b"wrong horse", &cookie, &proof));
        assert!(!is_proof_right(
            b"correct horse",
            &make_cookie(b"secret", other),
            &proof
        ));
        assert!(is_proof_right(b"", &cookie, &make_proof(b"", &cookie)));
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(encode_hex(&[0, 16, 255]), "0010ff");
        assert_eq!(decode_hex("0010ff").unwrap(), [0, 16, 255]);
        assert!(decode_hex("0").is_err());
        assert!(decode_hex("zz").is_err());
        assert!(decode_hex("éé").is_err());
    }

    #[test]
    fn levels_are_found_by_name() {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("match_levels.yaml");
        let levels: Vec<LevelEntry> = LoadLevels(&path).unwrap();
        assert_eq!(find_level(&levels, "Dust").unwrap().name, "Dust");
        assert!(find_level(&levels, "Atlantis").is_err());
    }
}
