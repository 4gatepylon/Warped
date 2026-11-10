//! `match_flow.rs` runs a match server across the levels (`serve_match`): it keeps the
//! `MatchState`, starts each round's level server (a separate program, see `LevelEntry`), reads
//! the level's winner back from the file the level writes, tells the match clients what is on, so
//! each client can start and stop its level, and announces the match on the local network
//! (`MatchAnnouncer`). What the server and its clients share (the levels file, the view, the
//! client, the beacon) is `warped_shared::match_protocol`, re-exported here.
//!
//! Interface: `serve_match`, `serve_match_on`, `make_level_client_command`, and from `match_protocol`:
//! `LevelEntry`, `LoadLevels`, `Phase`, `MatchView`, `MatchTimes`, `MatchClient`,
//! `describe_outcome`, `is_sitting_out`, `is_level_restart_due`, `stop_child`, `MATCH_PORT`,
//! `SELF_PROGRAM`.
//!
//! Example: `serve_match(&levels, &MatchTimes::default(), &run_folder, MATCH_PORT, 47852)?;`
//!
//! Must know:
//! - A level program is started as `<program> --serve --port <port> [--map <map>]
//!   --level-result-file <path> <server_args>` and joined as `<program> --join <address> --port
//!   <port> [--map <map>] --team <team> <client_args>` (README, "Level interface").

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::match_state::{MatchState, Team};
use crate::net;
use warped_shared::match_protocol::{
    CLIENT_TIMEOUT_S, MAX_DATAGRAM_BYTES, MatchAnnouncer, MatchBeacon, parse_hello, parse_play,
};
pub use warped_shared::match_protocol::{
    LevelEntry, LoadLevels, MATCH_PORT, MatchClient, MatchTimes, MatchView, Phase, SELF_PROGRAM,
    describe_outcome, is_level_restart_due, is_sitting_out, stop_child,
};
use warped_shared::network_scope;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SOCKET_TIMEOUT_MS: u64 = 50;

/// `resolve_program` gives the program a level runs: this binary for `self`; inside `Warped.app`
/// the level's program in `Contents/MacOS/levels/` (`app_bundle::get_app_level_program`); else
/// the path from this demo's folder.
///
/// Example: `let program = resolve_program(&level)?;`
fn resolve_program(level: &LevelEntry) -> Result<PathBuf> {
    if level.program == SELF_PROGRAM {
        return Ok(std::env::current_exe()?);
    }
    let path: PathBuf = match warped_shared::app_bundle::get_app_level_program(&level.name) {
        Some(path) => path,
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join(&level.program),
    };
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

/// `serve_match` runs one match on `levels` until it ends and `times.round_over_s` more (so the
/// clients see the result), on UDP `match_port`, each round's level server on `level_port`, its
/// results and logs in `run_folder`. A round starts `times.intermission_s` after a player is on
/// each team; the match ends in a tie `times.match_s` after its first round started. A client's
/// `play <level>` (the game master's) sets the match aside for free play on that level, round
/// after round, and `play match` goes back to it; either ends the round being played and starts
/// the next after an intermission, so every client follows.
///
/// Example: `serve_match(&levels, &MatchTimes::default(), &folder, MATCH_PORT, 47852)?;`
pub fn serve_match(
    levels: &[LevelEntry],
    times: &MatchTimes,
    run_folder: &Path,
    match_port: u16,
    level_port: u16,
) -> Result<MatchState> {
    let is_loopback_only: bool = net::is_loopback_only();
    let announcer: MatchAnnouncer = MatchAnnouncer::OnNetwork(is_loopback_only)?;
    let listen_ip: Ipv4Addr = network_scope::get_listen_ip(is_loopback_only);
    let address: SocketAddr = SocketAddr::from((listen_ip, match_port));
    serve_match_on(
        levels,
        times,
        run_folder,
        (address, level_port),
        Some(announcer),
    )
}

/// `serve_match_on` is `serve_match` listening at `address` (the match server's) with each
/// round's level server on `level_port`, announcing itself with `announcer`, or not at all with
/// `None` (a test on 127.0.0.1: no beacon, nothing open to the network).
///
/// Example: `serve_match_on(&levels, &times, &folder, ("127.0.0.1:48302".parse()?, 48303), None)?;`
pub fn serve_match_on(
    levels: &[LevelEntry],
    times: &MatchTimes,
    run_folder: &Path,
    (address, level_port): (SocketAddr, u16),
    mut announcer: Option<MatchAnnouncer>,
) -> Result<MatchState> {
    let match_port: u16 = address.port();
    let socket: UdpSocket = UdpSocket::bind(address)
        .map_err(|error| format!("can't use UDP port {match_port} for the match: {error}"))?;
    socket.set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))?;
    let mut state: MatchState = MatchState::New(levels.len());
    let mut phase: Phase = Phase::WaitingForTeams;
    let mut phase_ends_at: Option<Instant> = None;
    let mut match_started_at: Option<Instant> = None;
    let mut over_at: Option<Instant> = None;
    let mut clients: Vec<(SocketAddr, Option<Team>, Instant)> = Vec::new();
    let mut level_server: Option<Child> = None;
    let mut round_result_path: Option<PathBuf> = None;
    let mut free_round_total: usize = 0;
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
        if let Some((length, _)) = received
            && let Ok(choice) = parse_play(&buffer[..length], levels.len())
            && choice != state.get_free_play().map(|free_play| free_play.level)
        {
            match choice {
                Some(level) => {
                    println!("The game master chose free play on {}.", levels[level].name);
                    state.start_free_play(level);
                }
                None => {
                    println!("The game master went back to the match.");
                    state.end_free_play();
                }
            }
            stop_child(&mut level_server);
            over_at = None;
            phase = match phase {
                Phase::WaitingForTeams => Phase::WaitingForTeams,
                Phase::Over if choice.is_none() => Phase::Over,
                _ => {
                    phase_ends_at =
                        Some(Instant::now() + Duration::from_secs_f64(times.intermission_s));
                    Phase::Intermission {
                        left_s: times.intermission_s,
                    }
                }
            };
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
        if state.get_free_play().is_none() && state.outcome().is_none() && match_left_s == Some(0.0)
        {
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
                let (level, round, file_stem): (usize, usize, String) = match state.get_free_play()
                {
                    Some(free_play) => {
                        free_round_total += 1;
                        let round: usize = free_play.round_count + 1;
                        (
                            free_play.level,
                            round,
                            format!("free_round_{free_round_total}"),
                        )
                    }
                    None => {
                        let level: usize = state
                            .get_next_level()
                            .expect("an unfinished match has a next round");
                        let round: usize = state.rounds().len() + 1;
                        match_started_at.get_or_insert_with(Instant::now);
                        (level, round, format!("round_{round}"))
                    }
                };
                let result_path: PathBuf = run_folder.join(format!("{file_stem}.json"));
                let log_path: PathBuf = run_folder.join(format!("{file_stem}_server.log"));
                level_server = Some(start_level_server(
                    &levels[level],
                    level_port,
                    &result_path,
                    &log_path,
                )?);
                round_result_path = Some(result_path);
                match state.get_free_play() {
                    Some(_) => println!("Free round {round}: {}", levels[level].name),
                    None => println!(
                        "Round {round}: {}, attacker {:?}",
                        levels[level].name,
                        state.get_attacker()
                    ),
                }
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
                let result_path: &Path = round_result_path
                    .as_deref()
                    .expect("a round being played has its result file");
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
                match read_level_result(result_path)? {
                    Some(winner) => {
                        let prefix: &str = match state.get_free_play() {
                            Some(_) => {
                                state.record_free_round(winner);
                                "Free round"
                            }
                            None => {
                                state.record_round(winner);
                                "Round"
                            }
                        };
                        println!(
                            "{prefix} {round}: {} won {}",
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
                if let (None, Some(outcome)) = (state.get_free_play(), state.outcome()) {
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
        let view: MatchView = MatchView {
            state: state.clone(),
            phase: phase.clone(),
            levels: levels.to_vec(),
            team_counts,
            match_left_s,
        };
        if let Some(announcer) = &mut announcer {
            announcer.announce(&MatchBeacon::FromView(&view, match_port));
        }
        if let Some((_, address)) = received {
            socket.send_to(&serde_json::to_vec(&view)?, address)?;
        }
        if phase == Phase::Over && state.get_free_play().is_none() {
            let at: Instant = *over_at.get_or_insert_with(Instant::now);
            if at.elapsed().as_secs_f64() >= times.round_over_s + CLIENT_TIMEOUT_S {
                return Ok(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `match_flow.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Levels file | the shipped one (an even count: `warped_shared::match_protocol`) | `the_shipped_levels_file_loads` |
    //! | Level result | none yet, blue, red, garbage | `level_results_are_read` |
    //! | Game master's choice (a match server on 127.0.0.1, no beacon) | free play on a level (every round there, wins counted, no match round), back to the match (its next level) | `the_game_master_switches_levels_and_everyone_follows` |

    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use warped_shared::match_state::FreePlay;

    const TEST_MATCH_PORT: u16 = 48_302;
    const TEST_LEVEL_PORT: u16 = 48_303;
    const TEST_TIMEOUT_S: f64 = 20.0;
    const FAKE_LEVEL_SCRIPT: &str = "#!/bin/sh\nwhile [ \"$1\" != --level-result-file ]; do shift; done\nsleep 0.3\nprintf '{\"winner\": \"blue\"}' > \"$2.partial\" && mv \"$2.partial\" \"$2\"\nexec sleep 5\n";

    /// `make_fake_levels` writes a level program that wins for blue 0.3 s after it starts, and
    /// gives a three-level match of it.
    ///
    /// Example: `let levels = make_fake_levels(&folder);`
    fn make_fake_levels(folder: &Path) -> Vec<LevelEntry> {
        let program: PathBuf = folder.join("fake_level.sh");
        fs::write(&program, FAKE_LEVEL_SCRIPT).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        ["A", "B", "C"]
            .map(|name| LevelEntry {
                name: name.to_string(),
                folder: String::new(),
                branch: String::new(),
                status: String::new(),
                program: program.display().to_string(),
                map: None,
                server_args: Vec::new(),
                client_args: Vec::new(),
                stand_in_for: None,
                takes_bot_label: true,
                map_picture: None,
            })
            .to_vec()
    }

    /// `poll_until` polls both clients until `is_done` holds for blue's view, failing after
    /// `TEST_TIMEOUT_S`; it gives every playing level blue saw on the way.
    ///
    /// Example: `let played = poll_until(&mut clients, |view| view.phase == Phase::Over);`
    fn poll_until(
        clients: &mut [MatchClient; 2],
        is_done: impl Fn(&MatchView) -> bool,
    ) -> Vec<usize> {
        let deadline: Instant = Instant::now() + Duration::from_secs_f64(TEST_TIMEOUT_S);
        let mut played: Vec<usize> = Vec::new();
        loop {
            for client in clients.iter_mut() {
                client.poll().unwrap();
            }
            if let Some(view) = &clients[0].view {
                if let Phase::Playing { level, .. } = view.phase
                    && played.last() != Some(&level)
                {
                    played.push(level);
                }
                if is_done(view) {
                    return played;
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out; last view {:?}",
                clients[0].view
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn the_game_master_switches_levels_and_everyone_follows() {
        let folder: PathBuf =
            std::env::temp_dir().join(format!("serve_match_test_{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let levels: Vec<LevelEntry> = make_fake_levels(&folder);
        let times: MatchTimes = MatchTimes {
            intermission_s: 0.2,
            round_over_s: 0.2,
            match_s: 600.0,
        };
        let server_folder: PathBuf = folder.clone();
        std::thread::spawn(move || {
            let address: SocketAddr = SocketAddr::from((Ipv4Addr::LOCALHOST, TEST_MATCH_PORT));
            let result: Result<MatchState> = serve_match_on(
                &levels,
                &times,
                &server_folder,
                (address, TEST_LEVEL_PORT),
                None,
            );
            result.map(|_| ()).map_err(|error| error.to_string())
        });
        let server: SocketAddr = SocketAddr::from(([127, 0, 0, 1], TEST_MATCH_PORT));
        let mut clients: [MatchClient; 2] = [
            MatchClient::Connect(server, Some(Team::Blue)).unwrap(),
            MatchClient::Connect(server, Some(Team::Red)).unwrap(),
        ];
        poll_until(&mut clients, |view| view.team_counts == [1, 1]);
        clients[0].send_play(Some(0)).unwrap();
        let free: Vec<usize> = poll_until(&mut clients, |view| {
            view.state
                .get_free_play()
                .is_some_and(|free_play: FreePlay| free_play.wins[0] >= 2)
        });
        let first_free: usize = free.iter().position(|&level| level == 0).unwrap();
        assert!(
            free[first_free..].iter().all(|&level| level == 0),
            "{free:?}"
        );
        clients[1].send_play(None).unwrap();
        let back: Vec<usize> = poll_until(&mut clients, |view| {
            view.state.get_free_play().is_none() && matches!(view.phase, Phase::Playing { .. })
        });
        let view: &MatchView = clients[0].view.as_ref().unwrap();
        assert_eq!(back.last().copied(), view.state.get_next_level());
        assert!(view.state.rounds().len() <= 1, "{:?}", view.state.rounds());
        let _ = fs::remove_dir_all(&folder);
    }

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
}
