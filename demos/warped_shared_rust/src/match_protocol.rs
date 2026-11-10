//! `match_protocol.rs` is what a match server and its clients share: the levels of a match
//! (`match_levels.yaml`, read by `LoadLevels`), what the match is doing (`Phase`, `MatchView`),
//! a client of the match server (`MatchClient`), and the beacon a match server broadcasts on the
//! local network so players can find it (`MatchBeacon`, `MatchAnnouncer`, `MatchListener`). The
//! match server itself, which starts the level programs, is
//! `../warped_match_rust/src/match_flow.rs`.
//!
//! Interface: `LevelEntry`, `LoadLevels`, `Phase`, `MatchView`, `MatchTimes`, `MatchClient`
//! (`Connect`, `poll`, `send_play`, `send_settings`), `parse_hello`, `parse_play`, `make_play`, `stop_child`, `is_sitting_out`, `is_level_restart_due`,
//! `describe_outcome`, `MatchBeacon`, `BeaconPhase`, `MatchAnnouncer`, `MatchListener`,
//! `FoundMatch`, `update_found_matches`, `MATCH_PORT`, `MATCH_DISCOVERY_PORT`,
//! `CLIENT_TIMEOUT_S`, `MAX_DATAGRAM_BYTES`, `SELF_PROGRAM`.
//!
//! Example: `let mut client = MatchClient::Connect(server, Some(Team::Blue))?; client.poll()?;`
//!
//! Must know:
//! - The protocol is one text datagram from each client every `HELLO_PERIOD_S`, `hello <team>`
//!   (`blue`, `red` or `any`), answered with the `MatchView` as JSON. A client silent for
//!   `CLIENT_TIMEOUT_S` no longer counts. Any client may also send `play <level>` or `play match`
//!   (the game master's choice; no authentication yet), and the game settings messages of
//!   `game_settings.rs`, answered with `settings <json>` instead of the view.
//! - A beacon is `MATCH_BEACON_MAGIC`, a version byte, the match server's port and a summary of
//!   its match, broadcast every `MATCH_BEACON_PERIOD_S` to UDP `MATCH_DISCOVERY_PORT`; a reader
//!   ignores bytes after the fields it knows, so a later version can append fields.

use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::game_settings::{GameSettings, SettingsMessage, make_settings_message};
use crate::match_state::{MatchState, Outcome, Team};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const MATCH_PORT: u16 = 47_850;
pub const MATCH_DISCOVERY_PORT: u16 = 47_851;
const HELLO_PERIOD_S: f64 = 0.5;
pub const CLIENT_TIMEOUT_S: f64 = 3.0;
pub const MAX_DATAGRAM_BYTES: usize = 65_000;
pub const SELF_PROGRAM: &str = "self";
const MATCH_BEACON_MAGIC: &[u8; 8] = b"WARPMTCH";
const MATCH_BEACON_VERSION: u8 = 1;
const MATCH_BEACON_PERIOD_S: f64 = 0.5;
const BYTES_PER_MATCH_BEACON: usize = MATCH_BEACON_MAGIC.len() + 1 + 2 + 2 + 1 + 1;
const FOUND_MATCH_TIMEOUT_S: f64 = 3.0;

/// `LevelEntry` is one level of the match's map in `match_levels.yaml`: its name, the demo folder
/// and branch it comes from, whether it is playable yet (`status`), and how to run it: `program`
/// (`self` for this demo's own binary, else a path from this demo's folder), the `--map` it
/// takes, extra flags for its server and its clients, its map picture for the progression page
/// (`map_picture`, a PNG path from this demo's folder), and its default server settings
/// (`server_settings`, a path from the levels file's folder; the game master's settings,
/// `game_settings.rs`, lay their overrides over them).
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
    /// Whether the level's bots take `--label` (the name of their artifact folder).
    #[serde(default = "get_true")]
    pub takes_bot_label: bool,
    #[serde(default)]
    pub map_picture: Option<String>,
    #[serde(default)]
    pub server_settings: Option<String>,
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

/// `stop_child` kills a level process and waits for it to end.
///
/// Example: `stop_child(&mut level_server);`
pub fn stop_child(child: &mut Option<Child>) {
    if let Some(mut process) = child.take() {
        let _ = process.kill();
        let _ = process.wait();
    }
}

/// `is_sitting_out` says whether `team`'s bot joins no level in round `round` (from 1) of a
/// steered match: `round_winners`, each round's winner in turn, names the other team there. An
/// unsteered bot (no `round_winners`) never sits out; a round past the list is an error.
///
/// Example: `assert!(is_sitting_out(&[Team::Blue], 1, Team::Red)?);`
pub fn is_sitting_out(round_winners: &[Team], round: usize, team: Team) -> Result<bool> {
    if round_winners.is_empty() {
        return Ok(false);
    }
    let winner: Team = *round_winners.get(round - 1).ok_or_else(|| {
        format!(
            "round {round} started, but --bot-round-winners names only {} rounds",
            round_winners.len()
        )
    })?;
    Ok(winner != team)
}

/// `is_level_restart_due` says whether a match client may start a level again that stopped by
/// itself at `stopped_at`: only once a view came (`view_at`) `restart_s` after the stop, so it
/// shows whether the round goes on. An older view may still say a round is on that the stopped
/// level has just won (City Sphere's bot quits when its team wins).
///
/// Example: `if is_level_restart_due(stopped_at, client.view_at, 1.0) { ... }`
pub fn is_level_restart_due(
    stopped_at: Option<Instant>,
    view_at: Option<Instant>,
    restart_s: f64,
) -> bool {
    match (stopped_at, view_at) {
        (Some(stopped_at), Some(view_at)) => {
            view_at >= stopped_at + Duration::from_secs_f64(restart_s)
        }
        _ => false,
    }
}

/// `parse_hello` gives the team a client's `hello <team>` datagram names (`None` for `any`), or
/// an error for any other datagram.
///
/// Example: `assert_eq!(parse_hello(b"hello red")?, Some(Team::Red));`
pub fn parse_hello(datagram: &[u8]) -> Result<Option<Team>> {
    match datagram {
        b"hello blue" => Ok(Some(Team::Blue)),
        b"hello red" => Ok(Some(Team::Red)),
        b"hello any" => Ok(None),
        _ => Err("not a hello".into()),
    }
}

/// `PLAY_MATCH` is the word of a `play` message that goes back from free play to the match.
const PLAY_MATCH: &str = "match";

/// `parse_play` gives the game master's choice in a `play <level>` datagram (a level's index, for
/// free play on it) or `play match` (`None`, back to the match), checked against `level_count`;
/// any other datagram is an error.
///
/// Example: `assert_eq!(parse_play(b"play 2", 5)?, Some(2));`
pub fn parse_play(datagram: &[u8], level_count: usize) -> Result<Option<usize>> {
    let text: &str = std::str::from_utf8(datagram).map_err(|_| "not a play message")?;
    let choice: &str = text.strip_prefix("play ").ok_or("not a play message")?;
    if choice == PLAY_MATCH {
        return Ok(None);
    }
    let level: usize = choice
        .parse()
        .map_err(|_| format!("play {choice:?}: expected a level's index or {PLAY_MATCH}"))?;
    if level >= level_count {
        return Err(format!(
            "play {level}: the match has levels 0 to {}",
            level_count - 1
        )
        .into());
    }
    Ok(Some(level))
}

/// `make_play` gives the datagram `parse_play` reads for `choice`.
///
/// Example: `socket.send_to(&make_play(Some(2)), server)?;`
pub fn make_play(choice: Option<usize>) -> Vec<u8> {
    match choice {
        Some(level) => format!("play {level}").into_bytes(),
        None => format!("play {PLAY_MATCH}").into_bytes(),
    }
}

/// `MatchClient` is a connection to a match server: the team it says it plays for, the last
/// `MatchView` the server sent and when it came (`view_at`), and the last game settings it sent
/// (only after this client asked, `send_settings`).
pub struct MatchClient {
    socket: UdpSocket,
    pub server: SocketAddr,
    pub team: Option<Team>,
    pub view: Option<MatchView>,
    pub view_at: Option<Instant>,
    pub settings: Option<GameSettings>,
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
            view_at: None,
            settings: None,
            last_hello: None,
            buffer: vec![0; MAX_DATAGRAM_BYTES],
        })
    }

    /// `send_play` asks the match server, as the game master, to play `choice`: one level freely
    /// (its index), or the match (`None`). A server older than free play ignores it.
    ///
    /// Example: `client.send_play(Some(4))?;`
    pub fn send_play(&mut self, choice: Option<usize>) -> Result<()> {
        self.socket.send_to(&make_play(choice), self.server)?;
        Ok(())
    }

    /// `send_settings` sends the match server a game settings message (`game_settings.rs`): a
    /// request, or the game master's change, which it answers with the settings. A server older
    /// than the game settings answers with its view instead.
    ///
    /// Example: `client.send_settings(&SettingsMessage::Request)?;`
    pub fn send_settings(&mut self, message: &SettingsMessage) -> Result<()> {
        self.socket
            .send_to(&make_settings_message(message), self.server)?;
        Ok(())
    }

    /// `poll` says hello every `HELLO_PERIOD_S` and keeps the newest view and game settings the
    /// server sent.
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
                    let datagram: &[u8] = &self.buffer[..length];
                    match GameSettings::FromAnswer(datagram) {
                        Some(settings) => self.settings = Some(settings?),
                        None => {
                            self.view = Some(serde_json::from_slice(datagram)?);
                            self.view_at = Some(Instant::now());
                        }
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

/// `BeaconPhase` is what a match does, as its beacon says it: the `Phase` without its details.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BeaconPhase {
    WaitingForTeams = 0,
    Intermission = 1,
    Playing = 2,
    RoundOver = 3,
    Over = 4,
}

impl BeaconPhase {
    /// `FromByte` reads a beacon's phase byte, or `None` for an unknown one.
    ///
    /// Example: `assert_eq!(BeaconPhase::FromByte(2), Some(BeaconPhase::Playing));`
    #[allow(non_snake_case)]
    pub fn FromByte(byte: u8) -> Option<BeaconPhase> {
        [
            BeaconPhase::WaitingForTeams,
            BeaconPhase::Intermission,
            BeaconPhase::Playing,
            BeaconPhase::RoundOver,
            BeaconPhase::Over,
        ]
        .into_iter()
        .find(|phase| *phase as u8 == byte)
    }

    /// `get_label` gives the phase in a few words for a list of games.
    ///
    /// Example: `assert_eq!(BeaconPhase::Over.get_label(), "over");`
    pub fn get_label(self) -> &'static str {
        match self {
            BeaconPhase::WaitingForTeams => "waiting for players",
            BeaconPhase::Intermission => "between rounds",
            BeaconPhase::Playing => "playing",
            BeaconPhase::RoundOver => "round over",
            BeaconPhase::Over => "over",
        }
    }
}

/// `MatchBeacon` is what a match server announces about itself: the UDP port its clients say
/// hello to, how many clients each team has (blue, red; at most 255 each), what the match does,
/// and the round being played or the number of rounds played.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MatchBeacon {
    pub match_port: u16,
    pub team_counts: [u8; 2],
    pub phase: BeaconPhase,
    pub round: u8,
}

impl MatchBeacon {
    /// `FromView` gives the beacon of the match in `view`, served on `match_port`.
    ///
    /// Example: `let beacon = MatchBeacon::FromView(&view, MATCH_PORT);`
    #[allow(non_snake_case)]
    pub fn FromView(view: &MatchView, match_port: u16) -> MatchBeacon {
        let (phase, round): (BeaconPhase, usize) = match view.phase {
            Phase::WaitingForTeams => (BeaconPhase::WaitingForTeams, 0),
            Phase::Intermission { .. } => (BeaconPhase::Intermission, view.state.rounds().len()),
            Phase::Playing { round, .. } => (BeaconPhase::Playing, round),
            Phase::RoundOver { round, .. } => (BeaconPhase::RoundOver, round),
            Phase::Over => (BeaconPhase::Over, view.state.rounds().len()),
        };
        MatchBeacon {
            match_port,
            team_counts: view
                .team_counts
                .map(|count| count.min(u32::from(u8::MAX)) as u8),
            phase,
            round: round.min(usize::from(u8::MAX)) as u8,
        }
    }

    /// `FromBytes` reads a beacon datagram: the magic, a version of at least 1, then the fields,
    /// ignoring any bytes after them (a later version's fields). It fails, saying why, on
    /// anything else.
    ///
    /// Example: `let beacon = MatchBeacon::FromBytes(&datagram)?;`
    #[allow(non_snake_case)]
    pub fn FromBytes(bytes: &[u8]) -> Result<MatchBeacon> {
        if bytes.len() < BYTES_PER_MATCH_BEACON || !bytes.starts_with(MATCH_BEACON_MAGIC) {
            return Err(format!(
                "not a match beacon: {} bytes, expected at least {BYTES_PER_MATCH_BEACON} starting with {:?}",
                bytes.len(),
                String::from_utf8_lossy(MATCH_BEACON_MAGIC)
            )
            .into());
        }
        let fields: &[u8] = &bytes[MATCH_BEACON_MAGIC.len()..];
        if fields[0] < MATCH_BEACON_VERSION {
            return Err(format!("match beacon version {} is below 1", fields[0]).into());
        }
        let phase: BeaconPhase = BeaconPhase::FromByte(fields[5])
            .ok_or_else(|| format!("match beacon phase {} isn't one of 0 to 4", fields[5]))?;
        Ok(MatchBeacon {
            match_port: u16::from_le_bytes([fields[1], fields[2]]),
            team_counts: [fields[3], fields[4]],
            phase,
            round: fields[6],
        })
    }

    /// `to_bytes` gives the beacon datagram `FromBytes` reads.
    ///
    /// Example: `socket.send_to(&beacon.to_bytes(), target)?;`
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = MATCH_BEACON_MAGIC.to_vec();
        bytes.push(MATCH_BEACON_VERSION);
        bytes.extend_from_slice(&self.match_port.to_le_bytes());
        bytes.extend_from_slice(&self.team_counts);
        bytes.push(self.phase as u8);
        bytes.push(self.round);
        bytes
    }

    /// `describe` gives the match in one line for a list of games: its players and phase.
    ///
    /// Example: `let line = beacon.describe(); // "blue 1, red 0, waiting for players"`
    pub fn describe(&self) -> String {
        let [blue, red] = self.team_counts;
        match self.phase {
            BeaconPhase::Playing | BeaconPhase::RoundOver => format!(
                "blue {blue}, red {red}, round {} {}",
                self.round,
                self.phase.get_label()
            ),
            _ => format!("blue {blue}, red {red}, {}", self.phase.get_label()),
        }
    }
}

/// `MatchAnnouncer` sends a match server's beacon to `target` (the local network's
/// `MATCH_DISCOVERY_PORT`, or a test's address) at most every `MATCH_BEACON_PERIOD_S`.
pub struct MatchAnnouncer {
    socket: UdpSocket,
    target: SocketAddr,
    last_sent: Option<Instant>,
    has_warned: bool,
}

impl MatchAnnouncer {
    /// `New` makes an announcer sending to `target`.
    ///
    /// Example: `let mut announcer = MatchAnnouncer::New("127.0.0.1:48301".parse()?)?;`
    #[allow(non_snake_case)]
    pub fn New(target: SocketAddr) -> Result<MatchAnnouncer> {
        let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        Ok(MatchAnnouncer {
            socket,
            target,
            last_sent: None,
            has_warned: false,
        })
    }

    /// `OnNetwork` makes an announcer broadcasting to `MATCH_DISCOVERY_PORT` on the local
    /// network.
    ///
    /// Example: `let mut announcer = MatchAnnouncer::OnNetwork()?;`
    #[allow(non_snake_case)]
    pub fn OnNetwork() -> Result<MatchAnnouncer> {
        MatchAnnouncer::New(SocketAddr::from((
            Ipv4Addr::BROADCAST,
            MATCH_DISCOVERY_PORT,
        )))
    }

    /// `announce` sends `beacon` if `MATCH_BEACON_PERIOD_S` has passed since the last one. A
    /// failed send is reported once on stderr, not fatal: the match goes on, joined by address.
    ///
    /// Example: `announcer.announce(&MatchBeacon::FromView(&view, MATCH_PORT));`
    pub fn announce(&mut self, beacon: &MatchBeacon) {
        if self
            .last_sent
            .is_some_and(|at| at.elapsed().as_secs_f64() < MATCH_BEACON_PERIOD_S)
        {
            return;
        }
        self.last_sent = Some(Instant::now());
        if let Err(error) = self.socket.send_to(&beacon.to_bytes(), self.target)
            && !self.has_warned
        {
            self.has_warned = true;
            eprintln!(
                "match: can't announce this match on the network ({error}); players can still join it by address"
            );
        }
    }
}

/// `FoundMatch` is a match server heard on the network: its address (the beacon's sender with
/// the beacon's port), its latest beacon, and when that came.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FoundMatch {
    pub server: SocketAddr,
    pub beacon: MatchBeacon,
    pub heard_at: Instant,
}

/// `update_found_matches` adds the beacon `datagram` from its sender heard at `now` to `found`
/// (replacing that server's older one), drops servers not heard from for
/// `FOUND_MATCH_TIMEOUT_S`, and keeps `found` sorted by address. A datagram that isn't a beacon
/// is ignored, and `None` only drops the quiet servers.
///
/// Example: `update_found_matches(&mut found, Some((&bytes, sender)), Instant::now());`
pub fn update_found_matches(
    found: &mut Vec<FoundMatch>,
    datagram: Option<(&[u8], SocketAddr)>,
    now: Instant,
) {
    if let Some((bytes, sender)) = datagram
        && let Ok(beacon) = MatchBeacon::FromBytes(bytes)
    {
        let server: SocketAddr = SocketAddr::new(sender.ip(), beacon.match_port);
        found.retain(|known| known.server != server);
        found.push(FoundMatch {
            server,
            beacon,
            heard_at: now,
        });
    }
    found.retain(|known| {
        now.saturating_duration_since(known.heard_at).as_secs_f64() < FOUND_MATCH_TIMEOUT_S
    });
    found.sort_by_key(|known| known.server);
}

/// `MatchListener` listens for match beacons on a UDP port and keeps the matches heard lately.
pub struct MatchListener {
    socket: UdpSocket,
    found: Vec<FoundMatch>,
    buffer: Vec<u8>,
}

impl MatchListener {
    /// `Bind` listens on UDP `port` (`MATCH_DISCOVERY_PORT` for the local network). It fails if
    /// another program holds the port.
    ///
    /// Example: `let mut listener = MatchListener::Bind(MATCH_DISCOVERY_PORT)?;`
    #[allow(non_snake_case)]
    pub fn Bind(port: u16) -> Result<MatchListener> {
        let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|error| {
            format!(
                "can't listen for games on UDP port {port} ({error}); join a game by its address"
            )
        })?;
        socket.set_nonblocking(true)?;
        Ok(MatchListener {
            socket,
            found: Vec::new(),
            buffer: vec![0; MAX_DATAGRAM_BYTES],
        })
    }

    /// `poll` reads every beacon that arrived and forgets servers gone quiet.
    ///
    /// Example: `listener.poll()?; for found in listener.get_found() { ... }`
    pub fn poll(&mut self) -> Result<()> {
        loop {
            match self.socket.recv_from(&mut self.buffer) {
                Ok((length, sender)) => update_found_matches(
                    &mut self.found,
                    Some((&self.buffer[..length], sender)),
                    Instant::now(),
                ),
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        update_found_matches(&mut self.found, None, Instant::now());
        Ok(())
    }

    /// `get_found` gives the matches heard in the last `FOUND_MATCH_TIMEOUT_S`, by address.
    ///
    /// Example: `let count = listener.get_found().len();`
    pub fn get_found(&self) -> &[FoundMatch] {
        &self.found
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `match_protocol.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Levels file | an even count (the shipped one: `match_flow.rs`) | `an_even_level_count_is_refused` |
    //! | Hello | each team, any, garbage | `hellos_name_a_team` |
    //! | Play | every level of a five-level match, the match, past the last level, garbage, a hello | `play_messages_choose_any_level_or_the_match` |
    //! | Steered bot | unsteered, its team's round, the other's, past the list | `steered_bots_sit_out_the_other_teams_rounds` |
    //! | Level restart | none stopped, no view, a view from before the stop, too soon after, late enough | `a_stopped_level_restarts_only_on_a_fresh_view` |
    //! | Beacon bytes | written then read, every phase, counts and round at 0 and 255; extra bytes after (a later version) | `beacons_round_trip_and_ignore_extra_bytes` |
    //! | Bad beacon | too short, wrong magic, version 0, unknown phase | `bad_beacons_are_refused` |
    //! | Beacon from a view | waiting, intermission, playing, round over, over; a count above 255 | `beacons_summarise_the_view` |
    //! | Found list | a new server, the same server again (replaced), another server (sorted), a non-beacon, a server gone quiet | `found_matches_update_and_expire` |
    //! | Over the network | a beacon sent to the listener's port is found at the sender's address | `a_listener_hears_an_announcer` |

    use super::*;
    use std::path::PathBuf;

    const TEST_DISCOVERY_PORT: u16 = 48_301;

    /// `make_view` gives a view of a new five-level match in `phase` with `team_counts`.
    ///
    /// Example: `let view = make_view(Phase::Over, [1, 0]);`
    fn make_view(phase: Phase, team_counts: [u32; 2]) -> MatchView {
        MatchView {
            state: MatchState::New(5),
            phase,
            levels: Vec::new(),
            team_counts,
            match_left_s: None,
        }
    }

    #[test]
    fn beacons_round_trip_and_ignore_extra_bytes() {
        for (phase, counts, round) in [
            (BeaconPhase::WaitingForTeams, [0, 0], 0),
            (BeaconPhase::Intermission, [1, 2], 3),
            (BeaconPhase::Playing, [255, 255], 255),
            (BeaconPhase::RoundOver, [0, 1], 1),
            (BeaconPhase::Over, [4, 4], 9),
        ] {
            let beacon: MatchBeacon = MatchBeacon {
                match_port: 48_300,
                team_counts: counts,
                phase,
                round,
            };
            assert_eq!(MatchBeacon::FromBytes(&beacon.to_bytes()).unwrap(), beacon);
            let mut later_version: Vec<u8> = beacon.to_bytes();
            later_version[MATCH_BEACON_MAGIC.len()] = 2;
            later_version.extend_from_slice(b"new fields");
            assert_eq!(MatchBeacon::FromBytes(&later_version).unwrap(), beacon);
        }
    }

    #[test]
    fn bad_beacons_are_refused() {
        let good: Vec<u8> =
            MatchBeacon::FromView(&make_view(Phase::Over, [1, 1]), 48_300).to_bytes();
        assert!(MatchBeacon::FromBytes(&good[..good.len() - 1]).is_err());
        let mut wrong_magic: Vec<u8> = good.clone();
        wrong_magic[0] = b'X';
        assert!(MatchBeacon::FromBytes(&wrong_magic).is_err());
        let mut version_0: Vec<u8> = good.clone();
        version_0[MATCH_BEACON_MAGIC.len()] = 0;
        assert!(MatchBeacon::FromBytes(&version_0).is_err());
        let mut unknown_phase: Vec<u8> = good.clone();
        unknown_phase[MATCH_BEACON_MAGIC.len() + 5] = 5;
        assert!(MatchBeacon::FromBytes(&unknown_phase).is_err());
    }

    #[test]
    fn beacons_summarise_the_view() {
        let playing: Phase = Phase::Playing {
            round: 2,
            level: 1,
            port: 48_302,
        };
        let round_over: Phase = Phase::RoundOver {
            round: 4,
            left_s: 1.0,
        };
        for (phase, beacon_phase, round) in [
            (Phase::WaitingForTeams, BeaconPhase::WaitingForTeams, 0),
            (
                Phase::Intermission { left_s: 3.0 },
                BeaconPhase::Intermission,
                0,
            ),
            (playing, BeaconPhase::Playing, 2),
            (round_over, BeaconPhase::RoundOver, 4),
            (Phase::Over, BeaconPhase::Over, 0),
        ] {
            let beacon: MatchBeacon = MatchBeacon::FromView(&make_view(phase, [1, 300]), 48_300);
            assert_eq!(beacon.phase, beacon_phase);
            assert_eq!(beacon.round, round);
            assert_eq!(beacon.team_counts, [1, 255]);
            assert_eq!(beacon.match_port, 48_300);
        }
        let waiting: MatchBeacon =
            MatchBeacon::FromView(&make_view(Phase::WaitingForTeams, [1, 0]), 1);
        assert_eq!(waiting.describe(), "blue 1, red 0, waiting for players");
    }

    #[test]
    fn found_matches_update_and_expire() {
        let start: Instant = Instant::now();
        let beacon = |port: u16, blue: u8| {
            MatchBeacon {
                match_port: port,
                team_counts: [blue, 0],
                phase: BeaconPhase::WaitingForTeams,
                round: 0,
            }
            .to_bytes()
        };
        let sender_b: SocketAddr = "10.0.0.9:5000".parse().unwrap();
        let sender_a: SocketAddr = "10.0.0.2:6000".parse().unwrap();
        let mut found: Vec<FoundMatch> = Vec::new();
        update_found_matches(&mut found, Some((&beacon(48_300, 1), sender_b)), start);
        update_found_matches(&mut found, Some((&beacon(48_300, 2), sender_b)), start);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].server, "10.0.0.9:48300".parse().unwrap());
        assert_eq!(found[0].beacon.team_counts, [2, 0]);
        let later: Instant = start + Duration::from_secs_f64(FOUND_MATCH_TIMEOUT_S / 2.0);
        update_found_matches(&mut found, Some((&beacon(48_303, 0), sender_a)), later);
        update_found_matches(&mut found, Some((b"WORMHOLE", sender_a)), later);
        let servers: Vec<SocketAddr> = found.iter().map(|known| known.server).collect();
        assert_eq!(
            servers,
            [
                "10.0.0.2:48303".parse().unwrap(),
                "10.0.0.9:48300".parse().unwrap()
            ]
        );
        let quiet: Instant = start + Duration::from_secs_f64(FOUND_MATCH_TIMEOUT_S);
        update_found_matches(&mut found, None, quiet);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].server.port(), 48_303);
    }

    #[test]
    fn a_listener_hears_an_announcer() {
        let mut listener: MatchListener = MatchListener::Bind(TEST_DISCOVERY_PORT).unwrap();
        let target: SocketAddr = SocketAddr::from((Ipv4Addr::LOCALHOST, TEST_DISCOVERY_PORT));
        let mut announcer: MatchAnnouncer = MatchAnnouncer::New(target).unwrap();
        let beacon: MatchBeacon =
            MatchBeacon::FromView(&make_view(Phase::WaitingForTeams, [0, 1]), 48_302);
        announcer.announce(&beacon);
        let deadline: Instant = Instant::now() + Duration::from_secs(2);
        while listener.get_found().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            listener.poll().unwrap();
        }
        let found: &[FoundMatch] = listener.get_found();
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].server,
            SocketAddr::from((Ipv4Addr::LOCALHOST, 48_302))
        );
        assert_eq!(found[0].beacon, beacon);
    }

    #[test]
    fn a_stopped_level_restarts_only_on_a_fresh_view() {
        let stopped_at: Instant = Instant::now();
        let restart_s: f64 = 1.0;
        let after = |seconds: f64| Some(stopped_at + Duration::from_secs_f64(seconds));
        assert!(!is_level_restart_due(None, after(5.0), restart_s));
        assert!(!is_level_restart_due(Some(stopped_at), None, restart_s));
        assert!(!is_level_restart_due(
            after(1.0),
            Some(stopped_at),
            restart_s
        ));
        assert!(!is_level_restart_due(
            Some(stopped_at),
            after(0.5),
            restart_s
        ));
        assert!(is_level_restart_due(
            Some(stopped_at),
            after(1.0),
            restart_s
        ));
    }

    #[test]
    fn steered_bots_sit_out_the_other_teams_rounds() {
        let winners: [Team; 2] = [Team::Blue, Team::Red];
        assert!(!is_sitting_out(&[], 7, Team::Red).unwrap());
        assert!(!is_sitting_out(&winners, 1, Team::Blue).unwrap());
        assert!(is_sitting_out(&winners, 1, Team::Red).unwrap());
        assert!(is_sitting_out(&winners, 2, Team::Blue).unwrap());
        assert!(is_sitting_out(&winners, 3, Team::Red).is_err());
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
    fn play_messages_choose_any_level_or_the_match() {
        for level in 0..5 {
            assert_eq!(parse_play(&make_play(Some(level)), 5).unwrap(), Some(level));
        }
        assert_eq!(parse_play(&make_play(None), 5).unwrap(), None);
        assert!(parse_play(b"play 5", 5).is_err());
        assert!(parse_play(b"play dust", 5).is_err());
        assert!(parse_play(b"hello blue", 5).is_err());
    }

    #[test]
    fn hellos_name_a_team() {
        assert_eq!(parse_hello(b"hello blue").unwrap(), Some(Team::Blue));
        assert_eq!(parse_hello(b"hello red").unwrap(), Some(Team::Red));
        assert_eq!(parse_hello(b"hello any").unwrap(), None);
        assert!(parse_hello(b"join").is_err());
    }
}
