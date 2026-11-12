//! `net.rs` lets players on one local network share a game. A server (its own process, without a
//! window) keeps every player's eye frame, laser switch and the console they hold `E` on (with how
//! far along turning it off they are, so every window speeds up its pulse alike), sends
//! everyone a snapshot 60 times a second, removes a player another player's laser hit, turns off a
//! console held for the settings' `consoles.hold_to_disable_s`, ends the game when a team's consoles are all off, and
//! announces itself on the network so others can find it. It counts each player's kills and deaths
//! in the game and sends everyone the scores 4 times a second. A client joins, sends its own player
//! each frame and receives the others, the game's state and the scores.
//!
//! Interface: `DEFAULT_PORT`, `PROTOCOL_VERSION`, `MAX_PLAYER_COUNT`, `MAX_CONSOLE_COUNT`,
//! `TEAM_NAMES`, `PasswordRefused`, `get_team`, `read_game_password`, `bind_server`, `serve`,
//! `find_server`, `Client`, `RemotePlayer`, `GameState`, `Score`, `NetEvent`.
//!
//! Example: `let mut client = Client::Connect(net::find_server(Duration::from_secs(5))?)?;`
//!
//! Must know:
//! - A snapshot of every player goes out in pages of `PLAYERS_PER_PAGE` players, each a datagram of
//!   at most `level_net::MAX_SNAPSHOT_DATAGRAM_BYTES`, so a full server's snapshot is never split
//!   along the way; a window keeps the latest copy of each page (`level_net::SnapshotPages`).
//! - Messages are single UDP datagrams, little-endian, starting with a `Kind` byte. A lost state
//!   or snapshot is replaced by the next one; joins are resent until welcomed. Nothing is
//!   encrypted.
//! - A join carries `PROTOCOL_VERSION`, and must answer the server's challenge (a cookie keyed to
//!   the sender's address) with a keyed hash of the game's password, so the password never
//!   crosses the network; the server refuses another version or a wrong password. Every later
//!   message carries the token the welcome gave, so nobody else can speak for a player.
//! - A failed send is logged, not fatal, and no reply is bigger than its request until the sender
//!   has proven its address, so the server can't be used to flood someone else.
//! - Each client decides when its own laser hits someone (it traces the laser anyway), and the
//!   server takes its word. A client also says which console it holds `E` on, and the server takes
//!   that only from a player within the settings' `consoles.reach_m` of it who aims at it
//!   (`console_reach.rs`), so nobody turns a console off from afar.
//! - A player keeps their id, and so their team, when they rejoin from the same address.
//! - The server's welcome names its map and carries its game settings; a client of another map
//!   fails rather than playing on.
//! - An elimination counts a death for the player hit and a kill for the shooter, unless they are on
//!   the same team. Scores go back to 0 when a new game starts.
//! - An elimination carries its victim's respawn wait (`RespawnSettings::get_wait_s` on the time
//!   since this game on the level started) as an `f32` after the two ids, which older windows
//!   don't read; a window reads a message without it, from an older server, as that server's flat
//!   wait (`respawn.start_s`).

use std::cell::Cell;
use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use glam::{DMat4, DVec4};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::console_reach::ConsolePlace;
use crate::health::{add_damage, get_hit_damage_frac};
use crate::settings::ServerSettings;
use crate::shot_noise::{self, BEAM_SPECKLE_LEVEL_COUNT};
use crate::thrown_wormholes::{MouthPlace, PlayerWormholes, WormholeColor};
use warped_shared::level_net::{self, SNAPSHOT_PAGE_HEADER_BYTES, SnapshotPages};
use warped_shared::movement::{decode_crouch_level, encode_crouch_level};
use warped_shared::network_scope;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const DEFAULT_PORT: u16 = 47_830;
pub const PROTOCOL_VERSION: u16 = 2;
pub use warped_shared::level_net::MAX_PLAYER_COUNT;
pub const MAX_CONSOLE_COUNT: usize = 32;
pub const TEAM_NAMES: [&str; 2] = ["blue", "red"];
const NO_CONSOLE: u8 = u8::MAX;
const RIFLE_BIT: u8 = 1;
const TEAM_BIT: u8 = 2;
const SPECKLE_FIRST_BIT: u8 = 3;
const ANY_TEAM: u8 = u8::MAX;
const NO_WINNER: u8 = u8::MAX;
const DISCOVERY_PORT: u16 = 47_831;
const SNAPSHOT_PERIOD_S: f64 = 1.0 / 60.0;
const SCORES_PERIOD_S: f64 = 0.25;
const BEACON_PERIOD_S: f64 = 0.5;
const JOIN_RETRY_S: f64 = 0.5;
const PLAYER_TIMEOUT_S: f64 = 3.0;
const BEACON_GATHER_S: f64 = 1.0;
const OTHER_VERSION_WINDOW_S: f64 = 1.0;
const SERVER_POLL_MS: u64 = 2;
const MAX_DATAGRAM_BYTES: usize = 4096;
const BEACON_MAGIC: &[u8; 8] = b"WORMHOLE";
const FLOATS_PER_FRAME: usize = 16;
const BYTES_PER_FLOAT: usize = 8;
const BYTES_PER_ID: usize = 4;
const BYTES_PER_PORT: usize = 2;
const BYTES_PER_F32: usize = 4;
const F32S_PER_PLACE: usize = 4;
const F32S_PER_POINT: usize = 4;
const BYTES_PER_WORMHOLES: usize = 2 + (2 * F32S_PER_PLACE + F32S_PER_POINT) * BYTES_PER_F32;
const PURPLE_DOWN_BIT: u8 = 0;
const GREEN_DOWN_BIT: u8 = 1;
const FLYING_BIT: u8 = 2;
const FLYING_GREEN_BIT: u8 = 3;
const CROUCH_FIRST_BIT: u8 = 4;
const BYTES_PER_PLAYER: usize =
    BYTES_PER_ID + FLOATS_PER_FRAME * BYTES_PER_FLOAT + 3 + BYTES_PER_WORMHOLES;
const BYTES_PER_GAME: usize = 2 * BYTES_PER_ID + 1;
const BYTES_PER_VERSION: usize = 2;
const BYTES_PER_COOKIE: usize = 16;
const BYTES_PER_PROOF: usize = 32;
const BYTES_PER_TOKEN: usize = 8;
const BYTES_PER_SECRET: usize = 32;
const BYTES_PER_LOBBY: usize = 1 + TEAM_NAMES.len();
const REFUSED_FOR_VERSION: u8 = 1;
const REFUSED_FOR_PASSWORD: u8 = 2;
const PLAYERS_PER_PAGE: usize =
    level_net::get_players_per_page(BYTES_PER_PLAYER + 1, BYTES_PER_GAME);
const _: () = assert!(PLAYERS_PER_PAGE > 0 && MAX_PLAYER_COUNT <= u8::MAX as usize);

/// `Kind` is a message's first byte.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Join = 1,
    Welcome = 2,
    State = 3,
    Snapshot = 4,
    Hit = 5,
    Eliminated = 6,
    Beacon = 7,
    Leave = 8,
    LobbyQuery = 9,
    Lobby = 10,
    Scores = 11,
    Full = 12,
    WormholeHit = 13,
    Challenge = 14,
    Refused = 15,
}

impl Kind {
    /// `FromByte` reads a message's kind, or `None` for an unknown byte.
    ///
    /// Example: `let kind = Kind::FromByte(bytes[0]);`
    #[allow(non_snake_case)]
    fn FromByte(byte: u8) -> Option<Kind> {
        [
            Kind::Join,
            Kind::Welcome,
            Kind::State,
            Kind::Snapshot,
            Kind::Hit,
            Kind::Eliminated,
            Kind::Beacon,
            Kind::Leave,
            Kind::LobbyQuery,
            Kind::Lobby,
            Kind::Scores,
            Kind::Full,
            Kind::WormholeHit,
            Kind::Challenge,
            Kind::Refused,
        ]
        .into_iter()
        .find(|kind| *kind as u8 == byte)
    }
}

/// `RemotePlayer` is another player as the last snapshot gave them: their eye frame (while their
/// laser is on, turned by their shot's noise, so it is the frame their laser fires from), whether
/// their laser is on and from which gun, their team (an index into `TEAM_NAMES`), the console they
/// hold `E` on, if any, how far along turning it off they are, from 0 to 1, as the server times
/// it (`disabling_frac`, in steps of 1/255; a client sends 0, and the server sets it), the share of
/// their shot's beam pixels that speckle (`beam_speckle_frac`, in steps of
/// 1 / `shot_noise::BEAM_SPECKLE_LEVEL_COUNT`), their thrown wormholes, and how far down a
/// crouch they are (`crouch_frac`, in steps of 1 / `movement::CROUCH_LEVEL_COUNT`).
#[derive(Clone, Copy)]
pub struct RemotePlayer {
    pub id: u32,
    pub frame: DMat4,
    pub is_laser_on: bool,
    pub is_rifle: bool,
    pub team: u32,
    pub disabling_console: Option<u8>,
    pub disabling_frac: f64,
    pub beam_speckle_frac: f64,
    pub wormholes: PlayerWormholes,
    pub crouch_frac: f64,
}

/// `GameState` is the game as the server keeps it: which game this is (from 0, one more each new
/// game), the consoles turned off (bit i for console i) and the winning team once there is one.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct GameState {
    pub number: u32,
    pub off_mask: u32,
    pub winner: Option<u32>,
}

/// `Score` is one player's line on the scoreboard: their id, team (an index into `TEAM_NAMES`),
/// and the other team's players they eliminated (kills) and the times they were eliminated (deaths)
/// in this game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Score {
    pub id: u32,
    pub team: u32,
    pub kills: u32,
    pub deaths: u32,
}

/// `NetEvent` is something the server told this client; an elimination carries how long its victim
/// waits before coming back (`RespawnSettings::get_wait_s` on the server's level clock).
pub enum NetEvent {
    Joined(u32),
    Eliminated {
        victim_id: u32,
        shooter_id: u32,
        is_self: bool,
        respawn_wait_s: f64,
    },
}

/// `Message` builds a datagram.
struct Message(Vec<u8>);

impl Message {
    /// `New` starts a message of a kind.
    ///
    /// Example: `let message = Message::New(Kind::Join);`
    #[allow(non_snake_case)]
    fn New(kind: Kind) -> Message {
        Message(vec![kind as u8])
    }

    /// `id` appends a player id.
    ///
    /// Example: `let message = Message::New(Kind::Leave).id(3);`
    fn id(mut self, id: u32) -> Message {
        self.0.extend_from_slice(&id.to_le_bytes());
        self
    }

    /// `token` appends the token a welcome gave a player.
    ///
    /// Example: `let message = Message::New(Kind::Leave).token(token).id(3);`
    fn token(mut self, token: u64) -> Message {
        self.0.extend_from_slice(&token.to_le_bytes());
        self
    }

    /// `seconds` appends a time in seconds as an `f32`, such as an elimination's respawn wait
    /// (older windows stop reading before it).
    ///
    /// Example: `let message = Message::New(Kind::Eliminated).id(2).id(1).seconds(20.0);`
    fn seconds(mut self, time_s: f64) -> Message {
        self.0.extend_from_slice(&(time_s as f32).to_le_bytes());
        self
    }

    /// `player` appends a player: id, eye frame, laser switch (with the gun, the team and the beam
    /// speckle's level in its higher bits), the console they hold `E` on, how far along turning it
    /// off they are (a byte, 255 for done), and their wormholes.
    ///
    /// Example: `let message = Message::New(Kind::State).player(&player);`
    fn player(mut self, player: &RemotePlayer) -> Message {
        self = self.id(player.id);
        for value in player.frame.to_cols_array() {
            self.0.extend_from_slice(&value.to_le_bytes());
        }
        self.0.push(
            u8::from(player.is_laser_on)
                | u8::from(player.is_rifle) << RIFLE_BIT
                | (player.team as u8) << TEAM_BIT
                | encode_speckle_level(player.beam_speckle_frac) << SPECKLE_FIRST_BIT,
        );
        self.0.push(player.disabling_console.unwrap_or(NO_CONSOLE));
        self.0
            .push((player.disabling_frac.clamp(0.0, 1.0) * f64::from(u8::MAX)).round() as u8);
        self.wormholes(&player.wormholes, player.crouch_frac)
    }

    /// `wormholes` appends a player's wormholes: which are down or flying (and the flying one's
    /// colour), with the player's crouch share `crouch_frac` in the same byte's bits from
    /// `CROUCH_FIRST_BIT` up (W-137 (crouched robots sink); older windows read only the bits
    /// below), each colour's place (u, v, h, yaw), and the flying one's point, as `f32`s (zeros
    /// for what isn't there), then the pair's damage (a byte, 255 for destroyed).
    ///
    /// Example: `let message = message.wormholes(&PlayerWormholes::default(), 0.0);`
    fn wormholes(mut self, wormholes: &PlayerWormholes, crouch_frac: f64) -> Message {
        let flying_color: Option<WormholeColor> = wormholes.flying.map(|(color, _)| color);
        self.0.push(
            u8::from(wormholes.purple.is_some()) << PURPLE_DOWN_BIT
                | u8::from(wormholes.green.is_some()) << GREEN_DOWN_BIT
                | u8::from(flying_color.is_some()) << FLYING_BIT
                | u8::from(flying_color == Some(WormholeColor::Green)) << FLYING_GREEN_BIT
                | encode_crouch_level(crouch_frac) << CROUCH_FIRST_BIT,
        );
        let place_values = |place: Option<MouthPlace>| {
            place.map_or([0.0; F32S_PER_PLACE], |p| [p.u_m, p.v_m, p.h_m, p.yaw_rad])
        };
        let point: DVec4 = wormholes.flying.map_or(DVec4::ZERO, |(_, x)| x);
        for value in place_values(wormholes.purple)
            .into_iter()
            .chain(place_values(wormholes.green))
            .chain(point.to_array())
        {
            self.0.extend_from_slice(&(value as f32).to_le_bytes());
        }
        self.0
            .push((wormholes.damage_frac.clamp(0.0, 1.0) * f64::from(u8::MAX)).round() as u8);
        self
    }

    /// `game` appends the game's state.
    ///
    /// Example: `let message = message.game(&server.game);`
    fn game(mut self, game: &GameState) -> Message {
        self.0.extend_from_slice(&game.number.to_le_bytes());
        self.0.extend_from_slice(&game.off_mask.to_le_bytes());
        self.0
            .push(game.winner.map_or(NO_WINNER, |team| team as u8));
        self
    }

    /// `scores` appends a count and that many scores: each player's id, team, kills and deaths.
    ///
    /// Example: `let message = Message::New(Kind::Scores).scores(&server.get_scores());`
    fn scores(mut self, scores: &[Score]) -> Message {
        self.0
            .push(u8::try_from(scores.len()).expect("scores: at most 255 players are in a game"));
        for score in scores {
            self = self.id(score.id);
            self.0.push(score.team as u8);
            self.0.extend_from_slice(&score.kills.to_le_bytes());
            self.0.extend_from_slice(&score.deaths.to_le_bytes());
        }
        self
    }
}

/// `encode_speckle_level` gives a beam speckle share's level, 0 to `BEAM_SPECKLE_LEVEL_COUNT`,
/// which fits the laser byte's bits from `SPECKLE_FIRST_BIT` up; older windows read only the bits
/// below, so they ignore it.
///
/// Example: `let level = encode_speckle_level(0.12);`
fn encode_speckle_level(beam_speckle_frac: f64) -> u8 {
    (shot_noise::round_speckle_frac(beam_speckle_frac) * f64::from(BEAM_SPECKLE_LEVEL_COUNT))
        .round() as u8
}

/// `Reader` reads a datagram's fields in order.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Reader<'_> {
    /// `take` gives the next `count` bytes, or `None` if the datagram is too short.
    ///
    /// Example: `let id_bytes = reader.take(4)?;`
    fn take(&mut self, count: usize) -> Option<&[u8]> {
        let slice: &[u8] = self.bytes.get(self.position..self.position + count)?;
        self.position += count;
        Some(slice)
    }

    /// `id` reads a player id.
    ///
    /// Example: `let id = reader.id()?;`
    fn id(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.take(BYTES_PER_ID)?.try_into().ok()?,
        ))
    }

    /// `array` reads the next `N` bytes as an array.
    ///
    /// Example: `let cookie: [u8; 16] = reader.array()?;`
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    /// `token` reads a token written by `Message::token`.
    ///
    /// Example: `let token = reader.token()?;`
    fn token(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array::<BYTES_PER_TOKEN>()?))
    }

    /// `seconds` reads a time written by `Message::seconds`.
    ///
    /// Example: `let wait_s = reader.seconds()?;`
    fn seconds(&mut self) -> Option<f64> {
        Some(f64::from(f32::from_le_bytes(
            self.array::<BYTES_PER_F32>()?,
        )))
    }

    /// `player` reads a player written by `Message::player`.
    ///
    /// Example: `let player = reader.player()?;`
    fn player(&mut self) -> Option<RemotePlayer> {
        let id: u32 = self.id()?;
        let mut values: [f64; FLOATS_PER_FRAME] = [0.0; FLOATS_PER_FRAME];
        for value in &mut values {
            *value = f64::from_le_bytes(self.take(BYTES_PER_FLOAT)?.try_into().ok()?);
        }
        let laser: u8 = self.take(1)?[0];
        let console: u8 = self.take(1)?[0];
        let disabling: u8 = self.take(1)?[0];
        let (wormholes, crouch_frac) = self.wormholes()?;
        Some(RemotePlayer {
            id,
            frame: DMat4::from_cols_array(&values),
            is_laser_on: laser & 1 == 1,
            is_rifle: laser >> RIFLE_BIT & 1 == 1,
            team: u32::from(laser >> TEAM_BIT & 1),
            disabling_console: (console != NO_CONSOLE).then_some(console),
            disabling_frac: f64::from(disabling) / f64::from(u8::MAX),
            beam_speckle_frac: f64::from(laser >> SPECKLE_FIRST_BIT)
                / f64::from(BEAM_SPECKLE_LEVEL_COUNT),
            wormholes,
            crouch_frac,
        })
    }

    /// `wormholes` reads a player's wormholes and crouch share written by `Message::wormholes`.
    ///
    /// Example: `let (wormholes, crouch_frac) = reader.wormholes()?;`
    fn wormholes(&mut self) -> Option<(PlayerWormholes, f64)> {
        let flags: u8 = self.take(1)?[0];
        let mut values: [f64; 2 * F32S_PER_PLACE + F32S_PER_POINT] = [0.0; 12];
        for value in &mut values {
            *value = f64::from(f32::from_le_bytes(
                self.take(BYTES_PER_F32)?.try_into().ok()?,
            ));
        }
        let is_set = |bit: u8| flags >> bit & 1 == 1;
        let place = |at: usize, bit: u8| {
            let [u_m, v_m, h_m, yaw_rad] = [0, 1, 2, 3].map(|i| values[at + i]);
            is_set(bit).then_some(MouthPlace {
                u_m,
                v_m,
                h_m,
                yaw_rad,
            })
        };
        let color: WormholeColor = if is_set(FLYING_GREEN_BIT) {
            WormholeColor::Green
        } else {
            WormholeColor::Purple
        };
        let point: DVec4 = DVec4::from_slice(&values[2 * F32S_PER_PLACE..]);
        Some((
            PlayerWormholes {
                purple: place(0, PURPLE_DOWN_BIT),
                green: place(F32S_PER_PLACE, GREEN_DOWN_BIT),
                flying: is_set(FLYING_BIT).then_some((color, point)),
                damage_frac: f64::from(self.take(1)?[0]) / f64::from(u8::MAX),
            },
            decode_crouch_level(flags >> CROUCH_FIRST_BIT),
        ))
    }

    /// `game` reads a game state written by `Message::game`.
    ///
    /// Example: `let game = reader.game()?;`
    fn game(&mut self) -> Option<GameState> {
        let (number, off_mask) = (self.id()?, self.id()?);
        let winner: u8 = self.take(1)?[0];
        Some(GameState {
            number,
            off_mask,
            winner: (winner != NO_WINNER).then_some(u32::from(winner)),
        })
    }

    /// `scores` reads the rest of a datagram as scores written by `Message::scores`, or `None` if
    /// it is cut short, too long, or names a team that isn't in `TEAM_NAMES`.
    ///
    /// Example: `let scores = reader.scores()?;`
    fn scores(&mut self) -> Option<Vec<Score>> {
        let count: u8 = self.take(1)?[0];
        let mut scores: Vec<Score> = Vec::new();
        for _ in 0..count {
            let id: u32 = self.id()?;
            let team: u8 = self.take(1)?[0];
            let (kills, deaths) = (self.id()?, self.id()?);
            if usize::from(team) >= TEAM_NAMES.len() {
                return None;
            }
            scores.push(Score {
                id,
                team: u32::from(team),
                kills,
                deaths,
            });
        }
        (self.position == self.bytes.len()).then_some(scores)
    }
}

/// `make_proof` gives the keyed hash of a server's `cookie` that proves a player knows `password`.
///
/// Example: `let proof = make_proof(b"secret", &cookie);`
fn make_proof(password: &[u8], cookie: &[u8]) -> [u8; BYTES_PER_PROOF] {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(password).expect("HMAC takes a key of any length");
    mac.update(cookie);
    mac.finalize().into_bytes().into()
}

/// `is_proof_right` says, in constant time, whether `proof` is `make_proof(password, cookie)`.
///
/// Example: `if is_proof_right(&server.password, &cookie, &proof) { ... }`
fn is_proof_right(password: &[u8], cookie: &[u8], proof: &[u8]) -> bool {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(password).expect("HMAC takes a key of any length");
    mac.update(cookie);
    mac.verify_slice(proof).is_ok()
}

/// `make_random_bytes` gives `N` bytes from the operating system's random number generator.
///
/// Example: `let token = u64::from_le_bytes(make_random_bytes());`
fn make_random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes: [u8; N] = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
}

/// `Slot` is a joined player on the server, the token their welcome gave them, since when they have held `E` on their console, the
/// damage they took since joining (1 eliminates them), and the damage enemy shots did to their
/// wormholes since they last had none down (1 destroys them).
struct Slot {
    player: RemotePlayer,
    address: SocketAddr,
    token: u64,
    last_heard: Instant,
    disabling_since: Option<Instant>,
    damage_frac: f64,
    wormhole_damage_frac: f64,
}

/// `Tally` is a player's score in this game on the server, their address, and since when they wait
/// to respawn after being eliminated, if they do, and for how long (their last elimination's).
struct Tally {
    score: Score,
    address: SocketAddr,
    eliminated_at: Option<Instant>,
    respawn_wait_s: f64,
}

/// `Server` is the server's state: its socket, its map's name, the joined players, the next id to
/// give, the id each address had, each console's team and place, the game, the tally of every
/// player who joined this game, and when this game on the level started (its respawn clock).
struct Server {
    level_started_at: Instant,
    socket: UdpSocket,
    secret: [u8; BYTES_PER_SECRET],
    password: Vec<u8>,
    map_name: String,
    settings: ServerSettings,
    slots: Vec<Slot>,
    next_id: u32,
    known_ids: Vec<(SocketAddr, u32)>,
    consoles: Vec<ConsolePlace>,
    game: GameState,
    tallies: Vec<Tally>,
}

impl Server {
    /// `handle` reacts to one datagram from `address`.
    ///
    /// Example: `server.handle(&buffer[..length], address)?;`
    fn handle(&mut self, bytes: &[u8], address: SocketAddr) -> Result<()> {
        let mut reader: Reader = Reader { bytes, position: 1 };
        match bytes.first().copied().and_then(Kind::FromByte) {
            Some(Kind::Join) => {
                let (Some(version), Some(cookie), Some(proof)) = (
                    reader.array::<BYTES_PER_VERSION>(),
                    reader.array::<BYTES_PER_COOKIE>(),
                    reader.array::<BYTES_PER_PROOF>(),
                ) else {
                    return Ok(());
                };
                if version != PROTOCOL_VERSION.to_le_bytes() {
                    self.refuse(REFUSED_FOR_VERSION, address);
                    return Ok(());
                }
                let expected_cookie: [u8; BYTES_PER_COOKIE] = self.make_cookie(address);
                if cookie != expected_cookie {
                    let mut challenge: Message = Message::New(Kind::Challenge);
                    challenge.0.extend_from_slice(&expected_cookie);
                    self.send(&challenge.0, address);
                    return Ok(());
                }
                if !is_proof_right(&self.password, &cookie, &proof) {
                    self.refuse(REFUSED_FOR_PASSWORD, address);
                    return Ok(());
                }
                let requested: u8 = reader.take(1).map_or(ANY_TEAM, |bytes| bytes[0]);
                let existing: Option<(u32, u32, u64)> = self
                    .slots
                    .iter()
                    .find(|slot| slot.address == address)
                    .map(|slot| (slot.player.id, slot.player.team, slot.token));
                let (id, team, token) = match existing {
                    Some(found) => found,
                    None if self.slots.len() < MAX_PLAYER_COUNT => {
                        let team: u32 = self.choose_team(requested);
                        let id: u32 = match self.known_ids.iter().find(|(a, _)| *a == address) {
                            Some(&(_, id)) => id,
                            None => {
                                self.next_id += 1;
                                self.known_ids.push((address, self.next_id));
                                self.next_id
                            }
                        };
                        let token: u64 = u64::from_le_bytes(make_random_bytes());
                        self.slots.push(Slot {
                            player: RemotePlayer {
                                id,
                                frame: DMat4::IDENTITY,
                                is_laser_on: false,
                                is_rifle: false,
                                team,
                                disabling_console: None,
                                disabling_frac: 0.0,
                                beam_speckle_frac: 0.0,
                                crouch_frac: 0.0,
                                wormholes: PlayerWormholes::default(),
                            },
                            address,
                            token,
                            last_heard: Instant::now(),
                            disabling_since: None,
                            damage_frac: 0.0,
                            wormhole_damage_frac: 0.0,
                        });
                        self.start_tally(id, team, address);
                        println!(
                            "player {id} joined the {} team (their window's address: {address}; players join this server's port)",
                            TEAM_NAMES[team as usize]
                        );
                        (id, team, token)
                    }
                    None => {
                        self.send(&Message::New(Kind::Full).0, address);
                        return Ok(());
                    }
                };
                let mut welcome: Message = Message::New(Kind::Welcome).id(id).token(token);
                welcome.0.push(team as u8);
                welcome.0.push(self.map_name.len() as u8);
                welcome.0.extend_from_slice(self.map_name.as_bytes());
                welcome.0.extend_from_slice(&self.settings.to_json());
                self.send(&welcome.0, address);
            }
            Some(Kind::LobbyQuery) if bytes.len() >= BYTES_PER_LOBBY => {
                let mut lobby: Message = Message::New(Kind::Lobby);
                lobby.0.extend(self.count_teams().map(|count| count as u8));
                self.send(&lobby.0, address);
            }
            Some(Kind::State) => {
                if let Some(token) = reader.token()
                    && let Some(mut player) = reader.player()
                    && let Some(slot) = self.find_sender(address, token, player.id)
                {
                    player.team = self.slots[slot].player.team;
                    player.disabling_console = player
                        .disabling_console
                        .filter(|&console| self.can_disable(player.team, console, player.frame));
                    let slot: &mut Slot = &mut self.slots[slot];
                    if player.disabling_console != slot.player.disabling_console {
                        slot.disabling_since = player.disabling_console.map(|_| Instant::now());
                    }
                    if player.wormholes.get_places().is_empty() {
                        slot.wormhole_damage_frac = 0.0;
                    }
                    slot.player = player;
                    slot.last_heard = Instant::now();
                }
            }
            Some(Kind::Hit) => {
                let (Some(token), Some(shooter_id), Some(victim_id), Some(&[hits_to_kill])) =
                    (reader.token(), reader.id(), reader.id(), reader.take(1))
                else {
                    return Ok(());
                };
                let shooter_team: Option<u32> = self
                    .find_sender(address, token, shooter_id)
                    .map(|slot| self.slots[slot].player.team);
                let Some(victim) = self
                    .slots
                    .iter()
                    .position(|slot| slot.player.id == victim_id)
                else {
                    return Ok(());
                };
                if shooter_team.is_none() || self.game.winner.is_some() || hits_to_kill == 0 {
                    return Ok(());
                }
                if !self.settings.shooting.is_friendly_fire_on
                    && shooter_team == Some(self.slots[victim].player.team)
                {
                    return Ok(());
                }
                if !add_damage(
                    &mut self.slots[victim].damage_frac,
                    get_hit_damage_frac(hits_to_kill),
                ) {
                    return Ok(());
                }
                let victim_slot: Slot = self.slots.remove(victim);
                let victim_address: SocketAddr = victim_slot.address;
                let is_enemy: bool = shooter_team != Some(victim_slot.player.team);
                println!("player {victim_id} was hit by player {shooter_id}");
                let wait_s: f64 = self.count_elimination(victim_id, shooter_id, is_enemy);
                let message: Message = Message::New(Kind::Eliminated)
                    .id(victim_id)
                    .id(shooter_id)
                    .seconds(wait_s);
                for target in self
                    .slots
                    .iter()
                    .map(|slot| slot.address)
                    .chain([victim_address])
                {
                    self.send(&message.0, target);
                }
            }
            Some(Kind::WormholeHit) => {
                let (Some(token), Some(shooter_id), Some(owner_id), Some(&[hits_to_destroy])) =
                    (reader.token(), reader.id(), reader.id(), reader.take(1))
                else {
                    return Ok(());
                };
                let shooter_team: Option<u32> = self
                    .find_sender(address, token, shooter_id)
                    .map(|slot| self.slots[slot].player.team);
                if let Some(owner) = self.slots.iter_mut().find(|slot| {
                    slot.player.id == owner_id
                        && shooter_team.is_some_and(|team| team != slot.player.team)
                        && !slot.player.wormholes.get_places().is_empty()
                }) && hits_to_destroy > 0
                {
                    owner.wormhole_damage_frac =
                        (owner.wormhole_damage_frac + 1.0 / f64::from(hits_to_destroy)).min(1.0);
                }
            }
            Some(Kind::Leave) => {
                if let (Some(token), Some(id)) = (reader.token(), reader.id())
                    && let Some(slot) = self.find_sender(address, token, id)
                {
                    self.slots.remove(slot);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `find_sender` gives the slot of the joined player `id` whose welcome gave `token`, if the
    /// datagram came from their address.
    ///
    /// Example: `let slot = server.find_sender(address, token, 3);`
    fn find_sender(&self, address: SocketAddr, token: u64, id: u32) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.address == address && slot.token == token && slot.player.id == id)
    }

    /// `make_cookie` gives the challenge for `address`: a keyed hash of it that only this server
    /// can make, so a join that echoes it came from that address.
    ///
    /// Example: `let cookie = server.make_cookie(address);`
    fn make_cookie(&self, address: SocketAddr) -> [u8; BYTES_PER_COOKIE] {
        let hash: [u8; BYTES_PER_PROOF] = make_proof(&self.secret, address.to_string().as_bytes());
        hash[..BYTES_PER_COOKIE]
            .try_into()
            .expect("a hash is longer than a cookie")
    }

    /// `refuse` tells `address` why its join was refused (`REFUSED_FOR_VERSION` or
    /// `REFUSED_FOR_PASSWORD`), with this server's `PROTOCOL_VERSION`.
    ///
    /// Example: `server.refuse(REFUSED_FOR_PASSWORD, address);`
    fn refuse(&self, reason: u8, address: SocketAddr) {
        let mut refused: Message = Message::New(Kind::Refused);
        refused.0.push(reason);
        refused.0.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        self.send(&refused.0, address);
    }

    /// `send` sends a datagram to `address`, logging a failure rather than stopping the server, so
    /// one unreachable player can't end the game for everyone.
    ///
    /// Example: `server.send(&welcome.0, address);`
    fn send(&self, bytes: &[u8], address: SocketAddr) {
        if let Err(error) = self.socket.send_to(bytes, address) {
            eprintln!("net: can't send to {address}: {error}");
        }
    }

    /// `start_tally` gives player `id` a tally of this game's kills and deaths if they have none, and
    /// marks them as playing on `team` rather than waiting to respawn.
    ///
    /// Example: `server.start_tally(3, 1, address);`
    fn start_tally(&mut self, id: u32, team: u32, address: SocketAddr) {
        match self.tallies.iter_mut().find(|tally| tally.score.id == id) {
            Some(tally) => (tally.score.team, tally.eliminated_at) = (team, None),
            None => self.tallies.push(Tally {
                score: Score {
                    id,
                    team,
                    kills: 0,
                    deaths: 0,
                },
                address,
                eliminated_at: None,
                respawn_wait_s: 0.0,
            }),
        }
    }

    /// `get_tally_mut` lends player `id`'s tally, which every player who joined this game has.
    ///
    /// Example: `server.get_tally_mut(3).score.deaths += 1;`
    fn get_tally_mut(&mut self, id: u32) -> &mut Tally {
        self.tallies
            .iter_mut()
            .find(|tally| tally.score.id == id)
            .unwrap_or_else(|| panic!("get_tally_mut: player {id} never joined this game"))
    }

    /// `get_level_s` gives the seconds since this game on the level started (the server's start,
    /// or `start_new_game`), the clock the respawn wait grows by.
    ///
    /// Example: `let level_s: f64 = server.get_level_s();`
    fn get_level_s(&self) -> f64 {
        self.level_started_at.elapsed().as_secs_f64()
    }

    /// `count_elimination` counts a death for player `victim_id`, who now waits to respawn for the
    /// time it gives (`RespawnSettings::get_wait_s` at `get_level_s`, logged), and a kill for player
    /// `shooter_id` if `is_enemy` (they are on different teams).
    ///
    /// Example: `let wait_s: f64 = server.count_elimination(2, 1, true);`
    fn count_elimination(&mut self, victim_id: u32, shooter_id: u32, is_enemy: bool) -> f64 {
        let level_s: f64 = self.get_level_s();
        let wait_s: f64 = self.settings.respawn.get_wait_s(level_s);
        println!("player {victim_id} respawns in {wait_s:.1} s ({level_s:.1} s into the level)");
        let victim: &mut Tally = self.get_tally_mut(victim_id);
        victim.score.deaths += 1;
        (victim.eliminated_at, victim.respawn_wait_s) = (Some(Instant::now()), wait_s);
        if is_enemy {
            self.get_tally_mut(shooter_id).score.kills += 1;
        }
        wait_s
    }

    /// `is_waiting_to_respawn` says whether `tally`'s player was eliminated and may still rejoin:
    /// less than their respawn wait and `PLAYER_TIMEOUT_S` ago.
    ///
    /// Example: `if server.is_waiting_to_respawn(tally) { ... }`
    fn is_waiting_to_respawn(&self, tally: &Tally) -> bool {
        let wait_s: f64 = tally.respawn_wait_s + PLAYER_TIMEOUT_S;
        tally
            .eliminated_at
            .is_some_and(|time| time.elapsed().as_secs_f64() < wait_s)
    }

    /// `get_scores` gives the score of every player in the game: those joined and those waiting to
    /// respawn.
    ///
    /// Example: `let scores = server.get_scores();`
    fn get_scores(&self) -> Vec<Score> {
        self.tallies
            .iter()
            .filter(|tally| {
                self.slots
                    .iter()
                    .any(|slot| slot.player.id == tally.score.id)
                    || self.is_waiting_to_respawn(tally)
            })
            .map(|tally| tally.score)
            .collect()
    }

    /// `send_scores` sends the scores (`get_scores`) to every player in the game.
    ///
    /// Example: `server.send_scores()?;`
    fn send_scores(&self) -> Result<()> {
        let message: Message = Message::New(Kind::Scores).scores(&self.get_scores());
        let waiting = self
            .tallies
            .iter()
            .filter(|tally| self.is_waiting_to_respawn(tally))
            .map(|tally| tally.address);
        for address in self.slots.iter().map(|slot| slot.address).chain(waiting) {
            self.send(&message.0, address);
        }
        Ok(())
    }

    /// `count_teams` gives how many joined players each team has.
    ///
    /// Example: `let [blue, red] = server.count_teams();`
    fn count_teams(&self) -> [u32; TEAM_NAMES.len()] {
        let mut counts: [u32; TEAM_NAMES.len()] = [0; TEAM_NAMES.len()];
        for slot in &self.slots {
            counts[slot.player.team as usize] += 1;
        }
        counts
    }

    /// `choose_team` gives the team a joining player asked for (an index into `TEAM_NAMES`), or,
    /// for `ANY_TEAM` or an unknown team, the team with the fewest players (the first on a tie).
    ///
    /// Example: `let team = server.choose_team(ANY_TEAM);`
    fn choose_team(&self, requested: u8) -> u32 {
        if usize::from(requested) < TEAM_NAMES.len() {
            return u32::from(requested);
        }
        let counts: [u32; TEAM_NAMES.len()] = self.count_teams();
        (0..TEAM_NAMES.len() as u32)
            .min_by_key(|&team| counts[team as usize])
            .expect("there are teams")
    }

    /// `can_disable` says whether a player of `team` whose eye frame is `eye` may turn off
    /// `console`: an existing console of the other team, still on, in a game nobody has won yet,
    /// within the settings' `consoles.reach_m` of the eye and aimed at (`ConsolePlace::is_held_from`).
    ///
    /// Example: `if server.can_disable(1, 7, player.frame) { ... }`
    fn can_disable(&self, team: u32, console: u8, eye: DMat4) -> bool {
        let index: usize = usize::from(console);
        self.game.winner.is_none()
            && index < self.consoles.len()
            && self.consoles[index].team != team
            && self.game.off_mask >> index & 1 == 0
            && self.consoles[index].is_held_from(eye, self.settings.consoles.reach_m)
    }

    /// `update_consoles` turns off each console a player has held `E` on for `consoles.hold_to_disable_s`,
    /// and declares the other team the winner once all of a team's consoles are off.
    ///
    /// Example: `server.update_consoles();`
    fn update_consoles(&mut self) {
        for index in 0..self.slots.len() {
            let slot: &mut Slot = &mut self.slots[index];
            if let (Some(console), Some(since)) =
                (slot.player.disabling_console, slot.disabling_since)
                && since.elapsed().as_secs_f64() >= self.settings.consoles.hold_to_disable_s
            {
                (slot.player.disabling_console, slot.disabling_since) = (None, None);
                let id: u32 = slot.player.id;
                if self.game.winner.is_none() {
                    self.game.off_mask |= 1 << console;
                    println!("player {id} turned off console {console}");
                    self.update_winner();
                }
            }
        }
    }

    /// `update_winner` declares the other team the winner once all of a team's consoles are off;
    /// `update_consoles` calls it after each console, so the first team to finish wins even when
    /// both teams' last consoles go off in the same update.
    ///
    /// Example: `self.update_winner();`
    fn update_winner(&mut self) {
        let losers: Vec<u32> = (0..TEAM_NAMES.len() as u32)
            .filter(|&team| {
                let mut consoles = (0..self.consoles.len())
                    .filter(|&i| self.consoles[i].team == team)
                    .peekable();
                consoles.peek().is_some() && consoles.all(|i| self.game.off_mask >> i & 1 == 1)
            })
            .collect();
        if self.game.winner.is_none()
            && let [loser] = losers[..]
        {
            let winner: u32 = (loser + 1) % TEAM_NAMES.len() as u32;
            self.game.winner = Some(winner);
            println!("The {} team won.", TEAM_NAMES[winner as usize]);
        }
    }

    /// `start_new_game` turns every console back on, clears the winner, sets every player's kills
    /// and deaths back to 0 and restarts the level's respawn clock; clients see the game's number
    /// change and go back to their spawn.
    ///
    /// Example: `server.start_new_game();`
    fn start_new_game(&mut self) {
        self.game = GameState {
            number: self.game.number + 1,
            ..GameState::default()
        };
        self.level_started_at = Instant::now();
        for slot in &mut self.slots {
            (slot.player.disabling_console, slot.disabling_since) = (None, None);
        }
        for tally in &mut self.tallies {
            (tally.score.kills, tally.score.deaths) = (0, 0);
        }
        println!("Game {} started.", self.game.number);
    }

    /// `get_disabling_frac` gives how far along `slot`'s player is in turning off the console they
    /// hold `E` on: the seconds held over the settings' `consoles.hold_to_disable_s`, at most 1, or
    /// 0 when they hold `E` on none.
    ///
    /// Example: `let frac = server.get_disabling_frac(&server.slots[0]);`
    fn get_disabling_frac(&self, slot: &Slot) -> f64 {
        slot.disabling_since.map_or(0.0, |since| {
            (since.elapsed().as_secs_f64() / self.settings.consoles.hold_to_disable_s).min(1.0)
        })
    }

    /// `send_snapshot` sends every player the list of all players (each with how far along turning
    /// off a console they are, from `get_disabling_frac`), the game's state and each player's
    /// health (a byte each, 255 full), in pages of `PLAYERS_PER_PAGE` players.
    ///
    /// Example: `server.send_snapshot()?;`
    fn send_snapshot(&self) -> Result<()> {
        let page_count: usize = self.slots.len().div_ceil(PLAYERS_PER_PAGE);
        for (page_index, page) in self.slots.chunks(PLAYERS_PER_PAGE).enumerate() {
            self.send_snapshot_page(page_index, page_count, page);
        }
        Ok(())
    }

    /// `send_snapshot_page` sends every player page `page_index` of a snapshot of `page_count`
    /// pages: its index, the page count, then `send_snapshot`'s list for the players of `page`.
    ///
    /// Example: `self.send_snapshot_page(0, 2, &self.slots[..PLAYERS_PER_PAGE]);`
    fn send_snapshot_page(&self, page_index: usize, page_count: usize, page: &[Slot]) {
        let mut message: Message = Message::New(Kind::Snapshot);
        message
            .0
            .extend([page_index as u8, page_count as u8, page.len() as u8]);
        for slot in page {
            message = message.player(&RemotePlayer {
                disabling_frac: self.get_disabling_frac(slot),
                wormholes: PlayerWormholes {
                    damage_frac: slot.wormhole_damage_frac,
                    ..slot.player.wormholes
                },
                ..slot.player
            });
        }
        message = message.game(&self.game);
        for slot in page {
            let health_frac: f64 = (1.0 - slot.damage_frac).clamp(0.0, 1.0);
            message
                .0
                .push((health_frac * f64::from(u8::MAX)).round() as u8);
        }
        for slot in &self.slots {
            self.send(&message.0, slot.address);
        }
    }
}

/// `read_game_password` reads the game's password from the file `--game-password-file` names
/// (without its final line break), or gives the empty password, which suits a local network, when
/// there is no file.
///
/// Example: `let password = net::read_game_password(Some(Path::new("password.txt")))?;`
pub fn read_game_password(path: Option<&Path>) -> Result<Vec<u8>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let text: String = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "net: can't read the game password file {}: {error}",
            path.display()
        )
    })?;
    let password: &str = text.trim_end_matches(['\r', '\n']);
    if password.is_empty() {
        return Err(format!(
            "net: the game password file {} is empty; leave out --game-password-file for no password",
            path.display()
        )
        .into());
    }
    Ok(password.as_bytes().to_vec())
}

/// `is_loopback_only` says whether this program's sockets stay on 127.0.0.1: in this crate's
/// tests, and with `WARPED_LOOPBACK_ONLY=1` (`warped_shared::network_scope`).
///
/// Example: `let ip = network_scope::get_listen_ip(net::is_loopback_only());`
pub fn is_loopback_only() -> bool {
    cfg!(test) || network_scope::is_loopback_only()
}

/// `bind_server` opens the server's socket on `port`, so a port already in use fails at once.
///
/// Example: `let socket = net::bind_server(net::DEFAULT_PORT)?;`
pub fn bind_server(port: u16) -> Result<UdpSocket> {
    let listen_ip: Ipv4Addr = network_scope::get_listen_ip(is_loopback_only());
    let socket: UdpSocket = UdpSocket::bind((listen_ip, port)).map_err(|error| {
        format!("net: can't listen on UDP port {port} ({error}); pick another with --port")
    })?;
    socket.set_read_timeout(Some(Duration::from_millis(SERVER_POLL_MS)))?;
    Ok(socket)
}

/// `write_level_result` writes `{"winner": "<team>"}` to `path` atomically: to `<path>.partial`,
/// then renamed.
///
/// Example: `write_level_result(Path::new("result.json"), 0)?;`
fn write_level_result(path: &Path, winner: u32) -> Result<()> {
    let partial_path: PathBuf = PathBuf::from(format!("{}.partial", path.display()));
    std::fs::write(
        &partial_path,
        format!("{{\"winner\": \"{}\"}}\n", TEAM_NAMES[winner as usize]),
    )?;
    std::fs::rename(&partial_path, path)?;
    println!("Wrote the level result to {}.", path.display());
    Ok(())
}

/// `serve` runs a server for the map `map_name` with `settings` on `socket` (from `bind_server`)
/// until `should_stop` is set: it answers joins (naming the map and sending the settings), relays
/// players, sends the scores every `SCORES_PERIOD_S`, removes players who are hit or silent for
/// `PLAYER_TIMEOUT_S`, turns off
/// consoles (`consoles` gives each one's team and place; a hold counts only from a player in reach
/// who aims at it) and ends the game when a team's are all off,
/// starts a new game after that when `should_start_new_game` is set (clearing it) or, with
/// `new_game_after_win`, that long after a team won, admits only players who prove they know
/// `game_password` (empty for anyone), and broadcasts
/// a beacon on the local network (on 127.0.0.1 when `is_loopback_only`)
/// every `BEACON_PERIOD_S` so `find_server` can find it. With
/// `level_result_path` (a match's `--level-result-file`), it writes the winner there once, as
/// `{"winner": "blue"}`, and never starts a new game.
///
/// Example: `net::serve(net::bind_server(net::DEFAULT_PORT)?, &should_stop, "dust".to_string(), ServerSettings::Builtin(), places, &should_start_new_game, None, Vec::new(), None)?;`
#[allow(clippy::too_many_arguments)]
pub fn serve(
    socket: UdpSocket,
    should_stop: &AtomicBool,
    map_name: String,
    settings: ServerSettings,
    consoles: Vec<ConsolePlace>,
    should_start_new_game: &AtomicBool,
    new_game_after_win: Option<Duration>,
    game_password: Vec<u8>,
    level_result_path: Option<PathBuf>,
) -> Result<()> {
    assert!(
        consoles.len() <= MAX_CONSOLE_COUNT,
        "serve: {} consoles, more than {MAX_CONSOLE_COUNT}",
        consoles.len()
    );
    let port: u16 = socket.local_addr()?.port();
    let beacon_ip: Ipv4Addr = network_scope::get_beacon_ip(is_loopback_only());
    let beacon_target: SocketAddr = SocketAddr::from((beacon_ip, DISCOVERY_PORT));
    let beacon_socket: UdpSocket = network_scope::bind_socket_toward(beacon_target)?;
    let mut beacon: Vec<u8> = vec![Kind::Beacon as u8];
    beacon.extend_from_slice(BEACON_MAGIC);
    beacon.extend_from_slice(&port.to_le_bytes());
    let mut server: Server = Server {
        level_started_at: Instant::now(),
        socket,
        secret: make_random_bytes(),
        password: game_password,
        map_name,
        settings,
        slots: Vec::new(),
        next_id: 0,
        known_ids: Vec::new(),
        consoles,
        game: GameState::default(),
        tallies: Vec::new(),
    };
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    let (mut last_snapshot, mut last_beacon) = (Instant::now(), None::<Instant>);
    let mut last_scores: Instant = Instant::now();
    let mut has_warned_about_beacon: bool = false;
    let mut won_at: Option<Instant> = None;
    while !should_stop.load(Ordering::Relaxed) {
        match server.socket.recv_from(&mut buffer) {
            Ok((length, address)) => server.handle(&buffer[..length], address)?,
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(error) => return Err(error.into()),
        }
        server.slots.retain(|slot| {
            let is_alive: bool = slot.last_heard.elapsed().as_secs_f64() < PLAYER_TIMEOUT_S;
            if !is_alive {
                println!("player {} timed out", slot.player.id);
            }
            is_alive
        });
        server.update_consoles();
        if let (Some(path), Some(winner), None) = (&level_result_path, server.game.winner, won_at) {
            write_level_result(path, winner)?;
        }
        if server.game.winner.is_some() && won_at.is_none() && level_result_path.is_none() {
            println!("Press Enter here to start a new game.");
        }
        won_at = server.game.winner.and(won_at.or(Some(Instant::now())));
        if level_result_path.is_some() {
            should_start_new_game.store(false, Ordering::Relaxed);
        } else if let (Some(delay), Some(time)) = (new_game_after_win, won_at)
            && time.elapsed() >= delay
        {
            server.start_new_game();
        }
        if should_start_new_game.swap(false, Ordering::Relaxed) {
            match server.game.winner {
                Some(_) => server.start_new_game(),
                None => println!("No team has won yet, so the game goes on."),
            }
        }
        if last_snapshot.elapsed().as_secs_f64() >= SNAPSHOT_PERIOD_S {
            server.send_snapshot()?;
            last_snapshot = Instant::now();
        }
        if last_scores.elapsed().as_secs_f64() >= SCORES_PERIOD_S {
            server.send_scores()?;
            last_scores = Instant::now();
        }
        if last_beacon.is_none_or(|time| time.elapsed().as_secs_f64() >= BEACON_PERIOD_S) {
            if let Err(error) = beacon_socket.send_to(&beacon, beacon_target)
                && !has_warned_about_beacon
            {
                has_warned_about_beacon = true;
                eprintln!(
                    "net: can't announce this server on the network ({error}), so --join auto won't find it; on macOS, allow Local Network access for this app, or join with --join <this computer's IP address>"
                );
            }
            last_beacon = Some(Instant::now());
        }
    }
    Ok(())
}

/// `find_server` waits up to `timeout` for a server's beacon on the local network and gives its
/// address.
///
/// Example: `let address = net::find_server(Duration::from_secs(5))?;`
pub fn find_server(timeout: Duration) -> Result<SocketAddr> {
    let listen_ip: Ipv4Addr = network_scope::get_listen_ip(is_loopback_only());
    let socket: UdpSocket = UdpSocket::bind((listen_ip, DISCOVERY_PORT)).map_err(|error| {
        format!("net: can't listen for servers on UDP port {DISCOVERY_PORT} ({error}); join with --join <address>")
    })?;
    let mut deadline: Instant = Instant::now() + timeout;
    let mut found: Vec<SocketAddr> = Vec::new();
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    loop {
        let left: Duration = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        socket.set_read_timeout(Some(left))?;
        let Ok((length, address)) = socket.recv_from(&mut buffer) else {
            break;
        };
        let bytes: &[u8] = &buffer[..length];
        let magic_end: usize = 1 + BEACON_MAGIC.len();
        if length == magic_end + BYTES_PER_PORT
            && bytes[0] == Kind::Beacon as u8
            && &bytes[1..magic_end] == BEACON_MAGIC
        {
            let port: u16 = u16::from_le_bytes([bytes[magic_end], bytes[magic_end + 1]]);
            let server: SocketAddr = SocketAddr::new(address.ip(), port);
            if found.is_empty() {
                deadline = deadline.min(Instant::now() + Duration::from_secs_f64(BEACON_GATHER_S));
            }
            if !found.contains(&server) {
                found.push(server);
            }
        }
    }
    if found.is_empty() {
        return Err(format!(
            "net: no server announced itself within {} s; start one with --serve, or join with --join <address>; if macOS asked whether this app may find devices on the local network, allow it in System Settings, Privacy & Security, Local Network",
            timeout.as_secs()
        )
        .into());
    }
    let chosen: SocketAddr = choose_server(&found);
    if found.len() > 1 {
        let all: Vec<String> = found.iter().map(SocketAddr::to_string).collect();
        eprintln!(
            "Found {} servers ({}); joining {chosen}. To join another, use --join <address>.",
            found.len(),
            all.join(", ")
        );
    }
    Ok(chosen)
}

/// `choose_server` picks which of the servers that announced themselves to join: the first on
/// `DEFAULT_PORT` (a game started the usual way), else the first heard.
///
/// Example: `let server = choose_server(&found);`
fn choose_server(found: &[SocketAddr]) -> SocketAddr {
    match found.iter().find(|server| server.port() == DEFAULT_PORT) {
        Some(&server) => server,
        None => found[0],
    }
}

/// `PasswordRefused` is the error of a client whose game password the server at `.0` refused, so
/// the game can tell it from other failures and ask for the password again.
#[derive(Debug)]
pub struct PasswordRefused(pub SocketAddr);

impl std::fmt::Display for PasswordRefused {
    /// `fmt` says which server refused the password and how to fix it.
    ///
    /// Example: `println!("{}", PasswordRefused(server));`
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "net: the server at {} refused the game password; check it (--game-password-file) and join again",
            self.0
        )
    }
}

impl Error for PasswordRefused {}

/// `make_lobby_query` gives a lobby query padded to the size of the server's answer, so a stranger
/// can't make the server send more than it received.
///
/// Example: `socket.send_to(&make_lobby_query().0, server)?;`
fn make_lobby_query() -> Message {
    let mut query: Message = Message::New(Kind::LobbyQuery);
    query.0.resize(BYTES_PER_LOBBY, 0);
    query
}

/// `Client` is this player's connection: its map's name, its id and the server's game settings
/// once welcomed, the id of the latest welcome (kept while waiting to respawn), the other players,
/// the game, this player's health (1 full, from the server), the damage enemy shots did to this
/// player's wormholes (1 destroys them, from the server), and the latest scores of every player in
/// the game (`None` until the server sends them).
pub struct Client {
    socket: UdpSocket,
    server: SocketAddr,
    map_name: String,
    password: Vec<u8>,
    cookie: [u8; BYTES_PER_COOKIE],
    token: Option<u64>,
    pub id: Option<u32>,
    pub welcomed_id: Option<u32>,
    pub team: Option<u32>,
    pub settings: Option<ServerSettings>,
    pub players: Vec<RemotePlayer>,
    snapshot_pages: SnapshotPages<RemotePlayer>,
    pub game: GameState,
    pub health_frac: f64,
    pub wormhole_damage_frac: f64,
    pub lobby_counts: Option<[u32; TEAM_NAMES.len()]>,
    pub scores: Option<Vec<Score>>,
    join_request: Option<u8>,
    last_join: Instant,
    pub last_heard: Instant,
    is_full: bool,
    first_unanswered: Cell<Option<Instant>>,
    last_bad_snapshot: Option<Instant>,
    last_good_snapshot: Option<Instant>,
}

impl Client {
    /// `Connect` opens a socket to `server`'s game on the map `map_name`, which admits players who
    /// know `game_password`, and asks how many players each team has; the player is in the lobby
    /// until `join`.
    ///
    /// Example: `let mut client = Client::Connect("192.168.1.20:47800".parse()?, "lunar_surface", b"")?;`
    #[allow(non_snake_case)]
    pub fn Connect(server: SocketAddr, map_name: &str, game_password: &[u8]) -> Result<Client> {
        let socket: UdpSocket = network_scope::bind_socket_toward(server)?;
        socket.set_nonblocking(true)?;
        socket.send_to(&make_lobby_query().0, server)?;
        Ok(Client {
            socket,
            server,
            map_name: map_name.to_string(),
            password: game_password.to_vec(),
            cookie: [0; BYTES_PER_COOKIE],
            token: None,
            id: None,
            welcomed_id: None,
            team: None,
            settings: None,
            players: Vec::new(),
            snapshot_pages: SnapshotPages::New(),
            game: GameState::default(),
            health_frac: 1.0,
            wormhole_damage_frac: 0.0,
            lobby_counts: None,
            scores: None,
            join_request: None,
            last_join: Instant::now(),
            last_heard: Instant::now(),
            is_full: false,
            first_unanswered: Cell::new(Some(Instant::now())),
            last_bad_snapshot: None,
            last_good_snapshot: None,
        })
    }

    /// `poll` reads every datagram that arrived, keeps the latest players (all but this one) and
    /// scores, and returns what happened; while not yet welcomed it asks to join again every `JOIN_RETRY_S`. Once
    /// the server has left this welcomed player unanswered for longer than its `PLAYER_TIMEOUT_S`, after which it has
    /// dropped them, it joins the same team again (`rejoin`). It fails if the server plays another map.
    ///
    /// Example: `for event in client.poll()? { ... }`
    pub fn poll(&mut self) -> Result<Vec<NetEvent>> {
        let mut events: Vec<NetEvent> = Vec::new();
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        loop {
            let (length, address) = match self.socket.recv_from(&mut buffer) {
                Ok(received) => received,
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            };
            if address != self.server {
                continue;
            }
            self.last_heard = Instant::now();
            self.first_unanswered.set(None);
            let bytes: &[u8] = &buffer[..length];
            let mut reader: Reader = Reader { bytes, position: 1 };
            match bytes.first().copied().and_then(Kind::FromByte) {
                Some(Kind::Challenge) if self.id.is_none() => {
                    if let Some(cookie) = reader.array::<BYTES_PER_COOKIE>() {
                        self.cookie = cookie;
                        if self.join_request.is_some() {
                            self.send_join()?;
                        }
                    }
                }
                Some(Kind::Refused) if self.id.is_none() => {
                    let (Some([reason]), Some(version)) =
                        (reader.array::<1>(), reader.array::<BYTES_PER_VERSION>())
                    else {
                        continue;
                    };
                    if reason == REFUSED_FOR_PASSWORD {
                        return Err(Box::new(PasswordRefused(self.server)));
                    }
                    return Err(format!(
                        "net: the server at {} runs game version {}, and this game is version {PROTOCOL_VERSION}; get the same version as the server",
                        self.server,
                        u16::from_le_bytes(version)
                    )
                    .into());
                }
                Some(Kind::Welcome) if self.id.is_none() => {
                    if let Some(id) = reader.id()
                        && let Some(token) = reader.token()
                        && let Some(&[team]) = reader.take(1)
                        && let Some(&[name_bytes]) = reader.take(1)
                        && let Some(name) = reader.take(usize::from(name_bytes))
                    {
                        let server_map: String = String::from_utf8_lossy(name).into();
                        if server_map != self.map_name {
                            return Err(format!(
                                "net: the server at {} plays the map {server_map}, but this game loaded {}; join with the server's map (MAP={})",
                                self.server, self.map_name, server_map.replace('_', "-")
                            ).into());
                        }
                        let settings: ServerSettings =
                            ServerSettings::FromJson(&bytes[reader.position..]).map_err(|error| {
                                format!(
                                    "net: the server at {} sent settings this game can't read, so it runs another version of the game; rebuild the server and this game from the same commit ({error})",
                                    self.server
                                )
                            })?;
                        self.settings = Some(settings);
                        self.is_full = false;
                        self.id = Some(id);
                        self.token = Some(token);
                        self.welcomed_id = Some(id);
                        self.team = Some(u32::from(team));
                        self.health_frac = 1.0;
                        events.push(NetEvent::Joined(id));
                    }
                }
                Some(Kind::Full) if self.id.is_none() => self.is_full = true,
                Some(Kind::Lobby) => {
                    if let Some(counts) = reader.take(TEAM_NAMES.len()) {
                        self.lobby_counts = Some(std::array::from_fn(|i| u32::from(counts[i])));
                    }
                }
                Some(Kind::Scores) => {
                    if let Some(scores) = reader.scores() {
                        self.scores = Some(scores);
                    }
                }
                Some(Kind::Snapshot) => {
                    let Some(&[page_index, page_count, count]) = reader.take(3) else {
                        self.last_bad_snapshot = Some(Instant::now());
                        continue;
                    };
                    let count: usize = usize::from(count);
                    if page_index >= page_count
                        || length
                            != SNAPSHOT_PAGE_HEADER_BYTES
                                + count * (BYTES_PER_PLAYER + 1)
                                + BYTES_PER_GAME
                    {
                        self.last_bad_snapshot = Some(Instant::now());
                        continue;
                    }
                    self.last_good_snapshot = Some(Instant::now());
                    let players: Vec<RemotePlayer> =
                        (0..count).filter_map(|_| reader.player()).collect();
                    self.game = reader.game().expect("the length was checked");
                    let healths: &[u8] = reader.take(count).expect("the length was checked");
                    if let Some(i) = players.iter().position(|p| Some(p.id) == self.id) {
                        self.health_frac = f64::from(healths[i]) / f64::from(u8::MAX);
                        self.wormhole_damage_frac = players[i].wormholes.damage_frac;
                    }
                    let own_id: Option<u32> = self.id;
                    self.players = self
                        .snapshot_pages
                        .add_page(
                            usize::from(page_index),
                            usize::from(page_count),
                            players,
                            |player| player.id,
                        )
                        .into_iter()
                        .filter(|player| Some(player.id) != own_id)
                        .collect();
                }
                Some(Kind::Eliminated) => {
                    let old_server_wait_s: Option<f64> =
                        self.settings.as_ref().map(|s| s.respawn.start_s);
                    if let (Some(victim_id), Some(shooter_id), Some(respawn_wait_s)) = (
                        reader.id(),
                        reader.id(),
                        reader.seconds().or(old_server_wait_s),
                    ) {
                        let is_self: bool = Some(victim_id) == self.id;
                        if is_self {
                            self.id = None;
                            self.token = None;
                            self.players.clear();
                            self.snapshot_pages.clear();
                        }
                        self.players.retain(|player| player.id != victim_id);
                        self.snapshot_pages
                            .remove_player(victim_id, |player| player.id);
                        events.push(NetEvent::Eliminated {
                            victim_id,
                            shooter_id,
                            is_self,
                            respawn_wait_s,
                        });
                    }
                }
                _ => {}
            }
        }
        if self.id.is_some() && self.get_silent_s() > PLAYER_TIMEOUT_S {
            println!(
                "No answer from the server for {PLAYER_TIMEOUT_S} s, so it dropped this player; joining again."
            );
            self.rejoin()?;
        }
        Ok(events)
    }

    /// `send_state` sends this player's eye frame, laser switch, whether the gun is the rifle, the
    /// console they hold `E` on, their shot's beam speckle and their wormholes, once joined. The
    /// server times the hold itself, so the progress sent is 0.
    ///
    /// Example: `client.send_state(player.frame, is_laser_on, false, None, 0.0, PlayerWormholes::default(), 0.0)?;`
    #[allow(clippy::too_many_arguments)]
    pub fn send_state(
        &mut self,
        frame: DMat4,
        is_laser_on: bool,
        is_rifle: bool,
        disabling_console: Option<u8>,
        beam_speckle_frac: f64,
        wormholes: PlayerWormholes,
        crouch_frac: f64,
    ) -> Result<()> {
        if let (Some(id), Some(token)) = (self.id, self.token) {
            let player: RemotePlayer = RemotePlayer {
                id,
                frame,
                is_laser_on,
                is_rifle,
                team: self.team.unwrap_or(0),
                disabling_console,
                disabling_frac: 0.0,
                beam_speckle_frac,
                wormholes,
                crouch_frac,
            };
            self.send(&Message::New(Kind::State).token(token).player(&player))?;
        }
        Ok(())
    }

    /// `send_hit` tells the server this player's laser hit player `victim_id` once, with a weapon
    /// that eliminates in `hits_to_kill` such hits.
    ///
    /// Example: `client.send_hit(4, 12)?;`
    pub fn send_hit(&mut self, victim_id: u32, hits_to_kill: u8) -> Result<()> {
        if let (Some(id), Some(token)) = (self.id, self.token) {
            let mut message: Message = Message::New(Kind::Hit).token(token).id(id).id(victim_id);
            message.0.push(hits_to_kill);
            self.send(&message)?;
        }
        Ok(())
    }

    /// `send_wormhole_hit` tells the server this player's shot went through, or hit, the wormholes
    /// of player `owner_id` once, with a weapon that destroys them in `hits_to_destroy` such shots.
    ///
    /// Example: `client.send_wormhole_hit(4, 5)?;`
    pub fn send_wormhole_hit(&mut self, owner_id: u32, hits_to_destroy: u8) -> Result<()> {
        if let (Some(id), Some(token)) = (self.id, self.token) {
            let mut message: Message = Message::New(Kind::WormholeHit)
                .token(token)
                .id(id)
                .id(owner_id);
            message.0.push(hits_to_destroy);
            self.send(&message)?;
        }
        Ok(())
    }

    /// `get_server` gives the address of the server this client plays on.
    ///
    /// Example: `let server = client.get_server();`
    pub fn get_server(&self) -> SocketAddr {
        self.server
    }

    /// `get_silent_s` gives how long the server has left this client unanswered: the seconds
    /// since the first message sent after anything last came from it, or 0 when nothing is
    /// waiting for an answer. Time when this window sent nothing (a long first frame, say) doesn't
    /// count.
    ///
    /// Example: `if client.get_silent_s() > 2.0 { ... }`
    pub fn get_silent_s(&self) -> f64 {
        match self.first_unanswered.get() {
            Some(sent) => sent.elapsed().as_secs_f64(),
            None => 0.0,
        }
    }

    /// `is_full` says whether the server answered this player's join that the game is full
    /// (`MAX_PLAYER_COUNT` players), since their last welcome.
    ///
    /// Example: `if client.is_full() { ... }`
    pub fn is_full(&self) -> bool {
        self.is_full
    }

    /// `is_other_version` says whether the server's snapshots have the wrong length for this
    /// game, and none had the right one, within the last `OTHER_VERSION_WINDOW_S`: the server runs
    /// another version of the game.
    ///
    /// Example: `if client.is_other_version() { ... }`
    pub fn is_other_version(&self) -> bool {
        let is_recent = |at: Option<Instant>| {
            at.is_some_and(|at| at.elapsed().as_secs_f64() < OTHER_VERSION_WINDOW_S)
        };
        is_recent(self.last_bad_snapshot) && !is_recent(self.last_good_snapshot)
    }

    /// `is_waiting_for_welcome` says whether this player asked to join and the server hasn't
    /// welcomed them yet.
    ///
    /// Example: `if client.is_waiting_for_welcome() { ... }`
    pub fn is_waiting_for_welcome(&self) -> bool {
        self.join_request.is_some() && self.id.is_none()
    }

    /// `is_in_lobby` says whether this player hasn't chosen a team yet.
    ///
    /// Example: `if client.is_in_lobby() { ... }`
    pub fn is_in_lobby(&self) -> bool {
        self.join_request.is_none()
    }

    /// `join` asks to join the game on `team` (an index into `TEAM_NAMES`), or on whichever team
    /// has fewer players for `None`.
    ///
    /// Example: `client.join(Some(1))?;`
    pub fn join(&mut self, team: Option<u32>) -> Result<()> {
        self.join_request = Some(team.map_or(ANY_TEAM, |team| team as u8));
        self.last_join = Instant::now();
        self.send_join()
    }

    /// `rejoin` asks to join again on the same team, after this player was eliminated.
    ///
    /// Example: `client.rejoin()?;`
    pub fn rejoin(&mut self) -> Result<()> {
        if let Some(team) = self.team {
            self.join_request = Some(team as u8);
        }
        (self.id, self.token) = (None, None);
        self.last_join = Instant::now();
        self.send_join()
    }

    /// `keep_joining` resends the join request every `JOIN_RETRY_S` while not welcomed, in case
    /// it or the welcome was lost, or, in the lobby, asks again how many players each team has.
    ///
    /// Example: `client.keep_joining()?;`
    pub fn keep_joining(&mut self) -> Result<()> {
        if self.id.is_none() && self.last_join.elapsed().as_secs_f64() >= JOIN_RETRY_S {
            self.last_join = Instant::now();
            match self.join_request {
                Some(_) => self.send_join()?,
                None => self.send(&make_lobby_query())?,
            }
        }
        Ok(())
    }

    /// `send_join` sends the join request: this game's version, the server's latest challenge (none
    /// at first), the proof that this player knows the password, and the team asked for.
    ///
    /// Example: `self.send_join()?;`
    fn send_join(&self) -> Result<()> {
        let mut message: Message = Message::New(Kind::Join);
        message.0.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        message.0.extend_from_slice(&self.cookie);
        message
            .0
            .extend_from_slice(&make_proof(&self.password, &self.cookie));
        message.0.extend(self.join_request);
        self.send(&message)
    }

    /// `leave` tells the server this player is leaving.
    ///
    /// Example: `client.leave()?;`
    pub fn leave(&mut self) -> Result<()> {
        if let (Some(id), Some(token)) = (self.id.take(), self.token.take()) {
            self.send(&Message::New(Kind::Leave).token(token).id(id))?;
        }
        Ok(())
    }

    /// `send` sends a message to the server, dropping it if the socket is momentarily full, and
    /// starts the unanswered clock (`get_silent_s`) if it isn't running.
    ///
    /// Example: `self.send(&Message::New(Kind::Join))?;`
    fn send(&self, message: &Message) -> Result<()> {
        if self.first_unanswered.get().is_none() {
            self.first_unanswered.set(Some(Instant::now()));
        }
        match self.socket.send_to(&message.0, self.server) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the message encoding and the server's handling of joins, hits and scores.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Datagram length | complete (progress kept to a 255th; the beam speckle's top level beside the laser, rifle and red-team bits), cut short | `players_survive_encoding`, `short_datagrams_read_as_nothing` |
    //! | Player's wormholes | none, one down with a green throw in the air, both down and damaged | `players_survive_encoding` |
    //! | Player's crouch, beside their wormholes (W-137 (crouched robots sink)) | standing, crouched with none down, crouched with one down: the same length | `crouch_rides_in_the_wormholes_byte` |
    //! | Shot at a player's wormholes | by an enemy (adds up to destroyed, then no further), by a teammate (ignored), with none down (ignored); the owner then has none down (reset) | `enemy_shots_destroy_wormholes` |
    //! | Wormhole damage sent | to its owner's client, in the snapshot | `owners_learn_their_wormholes_are_destroyed` |
    //! | Join | new player, repeated join, server full, rejoin after a hit (same id), a client of another map (fails) | `joins_get_ids_once_until_full`, `only_joined_players_can_hit`, `clients_of_another_map_fail` |
    //! | Servers announcing themselves | one, several with one on the default port (chosen), several without (the first) | `the_default_port_wins_among_servers` |
    //! | A server that can't take this player | full (says so), of another version (snapshots that don't fit) | `full_games_and_other_versions_are_noticed` |
    //! | Join's version | this game's (goes on), another (refused, naming the server's) | `joins_get_ids_once_until_full`, `other_versions_and_wrong_passwords_are_refused` |
    //! | Join's challenge answer | none yet or another address's (challenged again), this address's (goes on) | `joins_need_the_challenge_answered` |
    //! | Join's proof of the password | right (welcomed), wrong (refused) | `joins_get_ids_once_until_full`, `other_versions_and_wrong_passwords_are_refused` |
    //! | Client's password | the server's (welcomed), another (fails, naming the password) | `clients_join_with_the_password_only` |
    //! | Token on a player's message | their own (taken), none or another player's (ignored) | `messages_need_the_senders_token` |
    //! | Lobby query | as long as the answer (answered), shorter (ignored) | `short_lobby_queries_are_ignored` |
    //! | Team asked for | blue or red (given), any or none (the smaller team), a different one on a repeated join (kept) | `players_join_the_team_they_choose_or_the_smaller` |
    //! | Hit | by a joined player, by a stranger, on a player who is gone | `only_joined_players_can_hit` |
    //! | Welcome size | the built-in settings with the longest map name | `the_welcome_fits_a_datagram` |
    //! | Snapshot of a full server | `MAX_PLAYER_COUNT` players: pages within `level_net::MAX_SNAPSHOT_DATAGRAM_BYTES`, every other player read back | `a_full_servers_snapshot_comes_in_safe_pages` |
    //! | Hits to kill | 1 (eliminated at once), several (eliminated at the last) | `only_joined_players_can_hit`, `rifle_hits_add_up` |
    //! | Console held | the other team's (turned off after the settings' `hold_to_disable_s`), one's own team's (ignored) | `holding_consoles_turns_them_off_and_wins` |
    //! | Who holds a console | from afar (ignored), turned away from it (ignored), in reach and aiming at it (turned off) | `a_console_is_turned_off_only_from_within_reach_and_aimed_at` |
    //! | Progress of turning off a console, in a snapshot | none held (0), part way (the seconds held over `hold_to_disable_s`), past the hold time (1) | `snapshots_carry_how_far_consoles_are_turned_off` |
    //! | Consoles off | some of a team's (no winner), all of a team's (the other team wins), then a new game (all back on) | `holding_consoles_turns_them_off_and_wins` |
    //! | Last consoles in one update | both teams' last consoles end their holds together (the first to finish wins) | `the_first_team_to_finish_in_an_update_wins` |
    //! | Scores datagram | complete (several players, none), cut short, too long, an unknown team | `scores_survive_encoding` |
    //! | Who a hit eliminates | nobody (a hit short of the limit), the other team's player (a kill and a death), a teammate (a death only) | `eliminations_count_kills_and_deaths` |
    //! | Friendly fire (`shooting.is_friendly_fire_on`) | on (a teammate falls), off (a teammate's hit does nothing, an enemy's still counts) | `eliminations_count_kills_and_deaths`, `friendly_fire_can_be_turned_off` |
    //! | Player in the scores | joined, waiting to respawn, rejoined (tally kept), waiting too long (left out) | `eliminations_count_kills_and_deaths` |
    //! | Game | the same, a new one (scores back to 0) | `eliminations_count_kills_and_deaths` |
    //! | Respawn wait an elimination carries | at the level's start, a period in (one step more), from a server older than the wait (its settings' flat wait) | `eliminations_carry_the_respawn_wait_of_the_levels_time` |
    //! | Scores sent | to a joined client (read with its id from the welcome) | `clients_receive_scores` |
    //! | Server silent to a welcomed client | shorter than its timeout (stays), longer (the server dropped it: joins the same team again) | `clients_the_server_dropped_join_again` |
    //! | Server socket in tests | bound on 127.0.0.1 (`is_loopback_only`) | `tests_listen_on_loopback_only` |

    use super::*;
    use crate::geometry::{AXIS_X, AXIS_Z, make_boost, make_rotation};
    use crate::health::DAMAGE_TOLERANCE_FRAC;
    use crate::settings::RespawnSettings;
    use crate::shapes::{Shape, ShapeKind};
    use glam::DVec3;

    const TEST_MAP_NAME: &str = "lunar_surface";
    const TEST_PASSWORD: &[u8] = b"correct horse";
    const LISTEN_MS: u64 = 200;
    const LOBBY_QUERY_AND_JOIN_COUNT: usize = 2;
    const BLUE: u8 = 0;
    const TEST_CURVATURE_RADIUS_M: f64 = 40.0;
    const PYLON_RADIUS_M: f64 = 1.35;
    const PYLON_HALF_HEIGHT_M: f64 = 6.0;
    const CONSOLE_GAP_M: f64 = 1.0;
    const FAR_M: f64 = 10.0;
    const SENT_DAMAGE_BYTE: u8 = 51;
    const RED: u8 = 1;
    const SENT_DISABLING_FRAC: f64 = 0.37;
    const SENT_SPECKLE_FRAC: f64 = 1.0;
    const DISABLING_FRAC_STEP: f64 = 1.0 / u8::MAX as f64;
    const HALF: f64 = 0.5;
    const TWICE: f64 = 2.0;

    /// `make_join` makes `from`'s join request for `team` (or `ANY_TEAM`) with `version`,
    /// answering `server`'s challenge with a proof of `password`.
    ///
    /// Example: `let message = make_join(&server, from, RED, PROTOCOL_VERSION, TEST_PASSWORD);`
    fn make_join(
        server: &Server,
        from: SocketAddr,
        team: u8,
        version: u16,
        password: &[u8],
    ) -> Vec<u8> {
        let cookie: [u8; BYTES_PER_COOKIE] = server.make_cookie(from);
        let mut message: Message = Message::New(Kind::Join);
        message.0.extend_from_slice(&version.to_le_bytes());
        message.0.extend_from_slice(&cookie);
        message.0.extend_from_slice(&make_proof(password, &cookie));
        message.0.push(team);
        message.0
    }

    /// `join` has the player at `from` join `server` on `team` with this game's version and the
    /// right password.
    ///
    /// Example: `join(&mut server, address(50_001), RED);`
    fn join(server: &mut Server, from: SocketAddr, team: u8) {
        let message: Vec<u8> = make_join(server, from, team, PROTOCOL_VERSION, TEST_PASSWORD);
        server.handle(&message, from).expect("a join");
    }

    /// `get_token` gives the token the welcome gave the player at `from`, or 0 for a stranger.
    ///
    /// Example: `let token = get_token(&server, address(50_001));`
    fn get_token(server: &Server, from: SocketAddr) -> u64 {
        server
            .slots
            .iter()
            .find(|slot| slot.address == from)
            .map_or(0, |slot| slot.token)
    }

    /// `make_listener` opens a local socket standing for a client, to read what the server sends.
    ///
    /// Example: `let listener = make_listener();`
    fn make_listener() -> UdpSocket {
        let listener: UdpSocket =
            UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free local port");
        listener
            .set_read_timeout(Some(Duration::from_millis(LISTEN_MS)))
            .expect("a timeout");
        listener
    }

    /// `receive` gives the next datagram `listener` gets, or `None` if none comes in `LISTEN_MS`.
    ///
    /// Example: `let reply = receive(&listener);`
    fn receive(listener: &UdpSocket) -> Option<Vec<u8>> {
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        let (length, _) = listener.recv_from(&mut buffer).ok()?;
        Some(buffer[..length].to_vec())
    }

    /// `exchange` passes datagrams between `server` and `client` for up to a second, until the
    /// client is welcomed (giving its events) or fails (giving its error).
    ///
    /// Example: `let error = exchange(&mut server, &mut client).err();`
    fn exchange(server: &mut Server, client: &mut Client) -> Result<Vec<NetEvent>> {
        server
            .socket
            .set_read_timeout(Some(Duration::from_millis(SERVER_POLL_MS)))
            .expect("a timeout");
        let deadline: Instant = Instant::now() + Duration::from_secs(1);
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        while Instant::now() < deadline {
            while let Ok((length, from)) = server.socket.recv_from(&mut buffer) {
                server.handle(&buffer[..length], from).expect("a datagram");
            }
            let events: Vec<NetEvent> = client.poll()?;
            if client.id.is_some() {
                return Ok(events);
            }
        }
        panic!("the client was neither welcomed nor refused within a second");
    }

    /// `make_server` makes a server on a free local port, with no players, admitting players who
    /// know `TEST_PASSWORD`.
    ///
    /// Example: `let mut server = make_server();`
    fn make_server() -> Server {
        Server {
            level_started_at: Instant::now(),
            socket: UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free local port"),
            secret: make_random_bytes(),
            password: TEST_PASSWORD.to_vec(),
            map_name: TEST_MAP_NAME.to_string(),
            settings: ServerSettings::Builtin(),
            slots: Vec::new(),
            next_id: 0,
            known_ids: Vec::new(),
            consoles: [BLUE, BLUE, RED].map(make_console_ahead).to_vec(),
            game: GameState::default(),
            tallies: Vec::new(),
        }
    }

    /// `make_hit` makes player `shooter_id`'s report, with `token`, of one hit on player
    /// `victim_id` with a weapon that eliminates in `hits_to_kill` hits.
    ///
    /// Example: `server.handle(&make_hit(token, 1, 2, 1), address(50_001))?;`
    fn make_hit(token: u64, shooter_id: u32, victim_id: u32, hits_to_kill: u8) -> Vec<u8> {
        let mut message: Message = Message::New(Kind::Hit)
            .token(token)
            .id(shooter_id)
            .id(victim_id);
        message.0.push(hits_to_kill);
        message.0
    }

    /// `get_kills_and_deaths` gives each player's `(id, kills, deaths)` in the server's scores,
    /// by id.
    ///
    /// Example: `assert_eq!(get_kills_and_deaths(&server), vec![(1, 1, 0)]);`
    fn get_kills_and_deaths(server: &Server) -> Vec<(u32, u32, u32)> {
        let mut scores: Vec<(u32, u32, u32)> = server
            .get_scores()
            .iter()
            .map(|score| (score.id, score.kills, score.deaths))
            .collect();
        scores.sort();
        scores
    }

    /// `make_console_ahead` gives a console of `team` shaped like the maps' pylons, standing
    /// `CONSOLE_GAP_M` in front of an eye at the origin that looks along its view (-z).
    ///
    /// Example: `let console = make_console_ahead(RED);`
    fn make_console_ahead(team: u8) -> ConsolePlace {
        let shape: Shape = Shape {
            kind: ShapeKind::Cylinder,
            centre: DVec3::new(0.0, PYLON_HALF_HEIGHT_M, 0.0),
            half: DVec3::new(PYLON_RADIUS_M, PYLON_HALF_HEIGHT_M, PYLON_RADIUS_M),
            round_m: 0.0,
            axis: 1,
            material: 0,
        };
        let gap_r: f64 = (PYLON_RADIUS_M + CONSOLE_GAP_M) / TEST_CURVATURE_RADIUS_M;
        let frame: DMat4 = make_boost(AXIS_Z, -gap_r);
        ConsolePlace::New(u32::from(team), frame, shape, TEST_CURVATURE_RADIUS_M)
    }

    /// `hold_console_from` sends player `id`'s state from `from`, their eye frame `eye`, holding
    /// `E` on `console`, then makes the server count it as held for its settings'
    /// `hold_to_disable_s` and update the consoles.
    ///
    /// Example: `hold_console_from(&mut server, 1, address(50_001), 2, DMat4::IDENTITY);`
    fn hold_console_from(server: &mut Server, id: u32, from: SocketAddr, console: u8, eye: DMat4) {
        start_holding_from(server, id, from, console, eye);
        server.update_consoles();
    }

    /// `hold_console` sends player `id`'s state from `from` holding `E` on `console`, then makes
    /// the server count it as held for its settings' `hold_to_disable_s` and update the consoles.
    ///
    /// Example: `hold_console(&mut server, 1, address(50_001), 2);`
    fn hold_console(server: &mut Server, id: u32, from: SocketAddr, console: u8) {
        start_holding(server, id, from, console);
        server.update_consoles();
    }

    /// `start_holding` is `hold_console` without the update, so several players' holds can end in
    /// the same update.
    ///
    /// Example: `start_holding(&mut server, 1, address(50_001), 2);`
    fn start_holding(server: &mut Server, id: u32, from: SocketAddr, console: u8) {
        start_holding_from(server, id, from, console, DMat4::IDENTITY);
    }

    /// `start_holding_from` is `start_holding` with the eye frame `eye` (`start_holding` puts it at
    /// the origin, looking at the consoles `make_server` gives).
    ///
    /// Example: `start_holding_from(&mut server, 1, address(50_001), 2, DMat4::IDENTITY);`
    fn start_holding_from(server: &mut Server, id: u32, from: SocketAddr, console: u8, eye: DMat4) {
        let player: RemotePlayer = RemotePlayer {
            id,
            frame: eye,
            is_laser_on: false,
            is_rifle: false,
            team: 0,
            disabling_console: Some(console),
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            crouch_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let state: Message = Message::New(Kind::State)
            .token(get_token(server, from))
            .player(&player);
        server.handle(&state.0, from).expect("a state");
        let disable_s: f64 = server.settings.consoles.hold_to_disable_s;
        for slot in &mut server.slots {
            slot.disabling_since = slot
                .disabling_since
                .map(|since| since - Duration::from_secs_f64(disable_s));
        }
    }

    /// `connect_client` connects a client to `server` and joins it on `team`, letting the server
    /// handle its lobby query and join, then, once the client has the challenge, its answer.
    ///
    /// Example: `let client = connect_client(&mut server, RED);`
    fn connect_client(server: &mut Server, team: u8) -> Client {
        let server_address: SocketAddr = server.socket.local_addr().expect("an address");
        let mut client: Client =
            Client::Connect(server_address, TEST_MAP_NAME, TEST_PASSWORD).expect("a socket");
        client.join(Some(u32::from(team))).expect("a join request");
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        for _ in 0..LOBBY_QUERY_AND_JOIN_COUNT {
            let (length, from) = server.socket.recv_from(&mut buffer).expect("a datagram");
            server
                .handle(&buffer[..length], from)
                .expect("a lobby query or join");
        }
        poll_until(&mut client, |client| client.cookie != [0; BYTES_PER_COOKIE]);
        let (length, from) = server
            .socket
            .recv_from(&mut buffer)
            .expect("the answered join");
        server
            .handle(&buffer[..length], from)
            .expect("the answered join");
        client
    }

    /// `address` gives a local address with a made-up port, standing for a client.
    ///
    /// Example: `let a = address(50000);`
    fn address(port: u16) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port)
    }

    /// `poll_until` polls `client` until `is_done` holds, for at most a second.
    ///
    /// Example: `poll_until(&mut client, |c| c.is_full());`
    fn poll_until(client: &mut Client, is_done: impl Fn(&Client) -> bool) {
        let start: Instant = Instant::now();
        while !is_done(client) && start.elapsed() < Duration::from_secs(1) {
            client.poll().expect("datagrams");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn full_games_and_other_versions_are_noticed() {
        let mut server: Server = make_server();
        for port in 0..MAX_PLAYER_COUNT as u16 {
            join(&mut server, address(50_100 + port), ANY_TEAM);
        }
        let mut client: Client = connect_client(&mut server, RED);
        poll_until(&mut client, Client::is_full);
        assert!(client.is_full() && client.is_waiting_for_welcome());
        assert!(!client.is_other_version());
        let client_address: SocketAddr =
            address(client.socket.local_addr().expect("an address").port());
        let short_snapshot: [u8; SNAPSHOT_PAGE_HEADER_BYTES] = [Kind::Snapshot as u8, 0, 1, 1];
        server
            .socket
            .send_to(&short_snapshot, client_address)
            .expect("a snapshot");
        poll_until(&mut client, Client::is_other_version);
        assert!(client.is_other_version());
    }

    #[test]
    fn the_default_port_wins_among_servers() {
        let other: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DEFAULT_PORT + 170);
        let usual: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DEFAULT_PORT);
        let third: SocketAddr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), DEFAULT_PORT + 171);
        assert_eq!(choose_server(&[other]), other);
        assert_eq!(choose_server(&[other, usual, third]), usual);
        assert_eq!(choose_server(&[third, other]), third);
    }

    #[test]
    fn players_survive_encoding() {
        let frame: DMat4 = DMat4::from_cols_array(&std::array::from_fn(|i| i as f64 * 0.25 - 1.0));
        let place = |u_m: f64| MouthPlace {
            u_m,
            v_m: -2.5,
            h_m: 1.75,
            yaw_rad: 0.5,
        };
        let flying: (WormholeColor, DVec4) =
            (WormholeColor::Green, DVec4::new(0.5, 0.25, 0.0, 1.5));
        for wormholes in [
            PlayerWormholes::default(),
            PlayerWormholes {
                purple: Some(place(12.0)),
                green: None,
                flying: Some(flying),
                damage_frac: 0.0,
            },
            PlayerWormholes {
                purple: Some(place(12.0)),
                green: Some(place(-30.0)),
                flying: None,
                damage_frac: f64::from(SENT_DAMAGE_BYTE) / f64::from(u8::MAX),
            },
        ] {
            let sent: RemotePlayer = RemotePlayer {
                id: 7,
                frame,
                is_laser_on: true,
                is_rifle: true,
                team: 1,
                disabling_console: Some(3),
                disabling_frac: SENT_DISABLING_FRAC,
                beam_speckle_frac: SENT_SPECKLE_FRAC,
                crouch_frac: 0.0,
                wormholes,
            };
            let game: GameState = GameState {
                number: 2,
                off_mask: 0b101,
                winner: Some(1),
            };
            let message: Message = Message::New(Kind::State).player(&sent).game(&game);
            let mut reader: Reader = Reader {
                bytes: &message.0,
                position: 1,
            };
            let player: RemotePlayer = reader.player().expect("a complete player");
            assert_eq!(
                (
                    player.id,
                    player.frame,
                    player.is_laser_on,
                    player.is_rifle,
                    player.disabling_console,
                    player.wormholes
                ),
                (7, frame, true, true, Some(3), wormholes)
            );
            assert!((player.disabling_frac - SENT_DISABLING_FRAC).abs() <= DISABLING_FRAC_STEP);
            assert_eq!(player.beam_speckle_frac, SENT_SPECKLE_FRAC);
            assert_eq!(player.team, 1);
            assert_eq!(reader.game(), Some(game));
        }
    }

    #[test]
    fn crouch_rides_in_the_wormholes_byte() {
        let purple: MouthPlace = MouthPlace {
            u_m: 1.0,
            v_m: 2.0,
            h_m: 0.5,
            yaw_rad: 0.25,
        };
        for (crouch_frac, wormholes) in [
            (0.0, PlayerWormholes::default()),
            (1.0, PlayerWormholes::default()),
            (
                1.0,
                PlayerWormholes {
                    purple: Some(purple),
                    ..PlayerWormholes::default()
                },
            ),
        ] {
            let sent: RemotePlayer = RemotePlayer {
                id: 3,
                frame: DMat4::IDENTITY,
                is_laser_on: false,
                is_rifle: false,
                team: 0,
                disabling_console: None,
                disabling_frac: 0.0,
                beam_speckle_frac: 0.0,
                wormholes,
                crouch_frac,
            };
            let message: Message = Message::New(Kind::State).player(&sent);
            assert_eq!(message.0.len(), 1 + BYTES_PER_PLAYER);
            let mut reader: Reader = Reader {
                bytes: &message.0,
                position: 1,
            };
            let player: RemotePlayer = reader.player().expect("a complete player");
            assert_eq!(
                (player.crouch_frac, player.wormholes),
                (crouch_frac, wormholes)
            );
        }
    }

    #[test]
    fn short_datagrams_read_as_nothing() {
        let player: RemotePlayer = RemotePlayer {
            id: 7,
            frame: DMat4::IDENTITY,
            is_laser_on: false,
            is_rifle: false,
            team: 0,
            disabling_console: None,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            crouch_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let message: Message = Message::New(Kind::State).player(&player);
        let cut: &[u8] = &message.0[..message.0.len() - 1];
        assert!(
            Reader {
                bytes: cut,
                position: 1
            }
            .player()
            .is_none()
        );
    }

    #[test]
    fn a_full_servers_snapshot_comes_in_safe_pages() {
        let mut server: Server = make_server();
        let mut client: Client = connect_client(&mut server, RED);
        let listener: UdpSocket = make_listener();
        let listener_address: SocketAddr = listener.local_addr().expect("an address");
        join(&mut server, listener_address, ANY_TEAM);
        for port in 2..MAX_PLAYER_COUNT as u16 {
            join(&mut server, address(50_100 + port), ANY_TEAM);
        }
        assert_eq!(server.slots.len(), MAX_PLAYER_COUNT);
        server.send_snapshot().expect("a snapshot");
        let pages: Vec<Vec<u8>> = std::iter::from_fn(|| receive(&listener))
            .filter(|datagram| datagram[0] == Kind::Snapshot as u8)
            .collect();
        assert_eq!(pages.len(), MAX_PLAYER_COUNT.div_ceil(PLAYERS_PER_PAGE));
        for page in &pages {
            assert!(page.len() <= level_net::MAX_SNAPSHOT_DATAGRAM_BYTES);
        }
        poll_until(&mut client, |client| {
            client.players.len() == MAX_PLAYER_COUNT - 1
        });
        let mut ids: Vec<u32> = client.players.iter().map(|player| player.id).collect();
        ids.sort();
        assert_eq!(ids, (2..=MAX_PLAYER_COUNT as u32).collect::<Vec<u32>>());
    }

    #[test]
    fn joins_get_ids_once_until_full() {
        let mut server: Server = make_server();
        join(&mut server, address(50_000), ANY_TEAM);
        join(&mut server, address(50_000), ANY_TEAM);
        assert_eq!(server.slots.len(), 1);
        for port in 1..=MAX_PLAYER_COUNT as u16 {
            join(&mut server, address(50_000 + port), ANY_TEAM);
        }
        assert_eq!(server.slots.len(), MAX_PLAYER_COUNT);
    }

    #[test]
    fn only_joined_players_can_hit() {
        let mut server: Server = make_server();
        join(&mut server, address(50_001), ANY_TEAM);
        join(&mut server, address(50_002), ANY_TEAM);
        let stranger_hit: Vec<u8> = make_hit(0, 1, 2, 1);
        server
            .handle(&stranger_hit, address(50_003))
            .expect("a stranger's hit");
        assert_eq!(server.slots.len(), 2);
        let hit: Vec<u8> = make_hit(get_token(&server, address(50_001)), 1, 2, 1);
        server.handle(&hit, address(50_001)).expect("a hit");
        assert_eq!(
            server
                .slots
                .iter()
                .map(|slot| slot.player.id)
                .collect::<Vec<u32>>(),
            vec![1]
        );
        server
            .handle(&hit, address(50_001))
            .expect("a hit on a player who is gone");
        assert_eq!(server.slots.len(), 1);
        join(&mut server, address(50_002), ANY_TEAM);
        assert_eq!(server.slots[1].player.id, 2);
    }

    #[test]
    fn clients_of_another_map_fail() {
        let mut server: Server = make_server();
        let server_address: SocketAddr = server.socket.local_addr().expect("an address");
        let mut client: Client =
            Client::Connect(server_address, "house_demo", TEST_PASSWORD).expect("a socket");
        client.join(None).expect("a join request");
        let error: String = exchange(&mut server, &mut client)
            .err()
            .expect("a failure")
            .to_string();
        assert!(
            error.contains(TEST_MAP_NAME) && error.contains("house_demo"),
            "{error}"
        );
    }

    #[test]
    fn the_welcome_fits_a_datagram() {
        let header_bytes: usize = 1 + BYTES_PER_ID + BYTES_PER_TOKEN + 1 + usize::from(u8::MAX);
        assert!(header_bytes + ServerSettings::Builtin().to_json().len() <= MAX_DATAGRAM_BYTES);
    }

    #[test]
    fn players_join_the_team_they_choose_or_the_smaller() {
        let mut server: Server = make_server();
        join(&mut server, address(50_001), RED);
        join(&mut server, address(50_002), RED);
        join(&mut server, address(50_003), ANY_TEAM);
        let mut without_team: Vec<u8> = make_join(
            &server,
            address(50_004),
            ANY_TEAM,
            PROTOCOL_VERSION,
            TEST_PASSWORD,
        );
        without_team.pop();
        server
            .handle(&without_team, address(50_004))
            .expect("a join");
        let teams: Vec<u32> = server.slots.iter().map(|slot| slot.player.team).collect();
        assert_eq!(teams, vec![1, 1, 0, 0]);
        assert_eq!(server.count_teams(), [2, 2]);
        join(&mut server, address(50_001), BLUE);
        assert_eq!(server.slots[0].player.team, 1);
    }

    #[test]
    fn rifle_hits_add_up() {
        const HITS_TO_KILL: u8 = 3;
        let mut server: Server = make_server();
        join(&mut server, address(50_001), ANY_TEAM);
        join(&mut server, address(50_002), ANY_TEAM);
        let hit: Vec<u8> = make_hit(get_token(&server, address(50_001)), 1, 2, HITS_TO_KILL);
        for _ in 1..HITS_TO_KILL {
            server.handle(&hit, address(50_001)).expect("a hit");
            assert_eq!(server.slots.len(), 2);
        }
        server.handle(&hit, address(50_001)).expect("the last hit");
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn owners_learn_their_wormholes_are_destroyed() {
        const SNIPER_HITS_TO_DESTROY: u8 = 2;
        let hits_to_destroy: u8 = SNIPER_HITS_TO_DESTROY;
        let mut server: Server = make_server();
        let mut owner: Client = connect_client(&mut server, RED);
        join(&mut server, address(50_001), BLUE);
        server.slots[0].player.wormholes.green = Some(MouthPlace {
            u_m: 0.0,
            v_m: 0.0,
            h_m: 1.6,
            yaw_rad: 0.0,
        });
        let mut hit: Message = Message::New(Kind::WormholeHit)
            .token(get_token(&server, address(50_001)))
            .id(2)
            .id(1);
        hit.0.push(hits_to_destroy);
        for _ in 0..hits_to_destroy {
            server
                .handle(&hit.0, address(50_001))
                .expect("a wormhole hit");
        }
        server.send_snapshot().expect("a snapshot");
        poll_until(&mut owner, |client| client.wormhole_damage_frac >= 1.0);
        assert_eq!(owner.wormhole_damage_frac, 1.0);
    }

    #[test]
    fn enemy_shots_destroy_wormholes() {
        const RIFLE_HITS_TO_DESTROY: u8 = 5;
        let builtin: ServerSettings = ServerSettings::Builtin();
        assert_eq!(
            (
                builtin.wormholes.sniper_hits_to_destroy,
                builtin.wormholes.rifle_hits_to_destroy
            ),
            (0, 0),
            "shots never destroy thrown wormholes by default (W-166)"
        );
        let hits_to_destroy: u8 = RIFLE_HITS_TO_DESTROY;
        let mut server: Server = make_server();
        for (team, port) in [(BLUE, 50_001), (RED, 50_002), (RED, 50_003)] {
            join(&mut server, address(port), team);
        }
        let shoot = |server: &mut Server, shooter_id: u32, port: u16| {
            let mut message: Message = Message::New(Kind::WormholeHit)
                .token(get_token(server, address(port)))
                .id(shooter_id)
                .id(2);
            message.0.push(hits_to_destroy);
            server
                .handle(&message.0, address(port))
                .expect("a wormhole hit");
            server.slots[1].wormhole_damage_frac
        };
        let send_wormholes = |server: &mut Server, wormholes: PlayerWormholes| {
            let mut owner: RemotePlayer = server.slots[1].player;
            owner.wormholes = wormholes;
            let state: Vec<u8> = Message::New(Kind::State)
                .token(get_token(server, address(50_002)))
                .player(&owner)
                .0;
            server.handle(&state, address(50_002)).expect("a state");
        };
        assert_eq!(shoot(&mut server, 1, 50_001), 0.0);
        let place: MouthPlace = MouthPlace {
            u_m: 0.0,
            v_m: 0.0,
            h_m: 1.6,
            yaw_rad: 0.0,
        };
        let down: PlayerWormholes = PlayerWormholes {
            purple: Some(place),
            ..PlayerWormholes::default()
        };
        send_wormholes(&mut server, down);
        assert_eq!(shoot(&mut server, 3, 50_003), 0.0);
        for k in 1..=hits_to_destroy {
            let expected: f64 = f64::from(k) / f64::from(hits_to_destroy);
            assert!((shoot(&mut server, 1, 50_001) - expected).abs() < DAMAGE_TOLERANCE_FRAC);
        }
        assert_eq!(shoot(&mut server, 1, 50_001), 1.0);
        send_wormholes(&mut server, PlayerWormholes::default());
        assert_eq!(server.slots[1].wormhole_damage_frac, 0.0);
    }

    #[test]
    fn the_first_team_to_finish_in_an_update_wins() {
        let mut server: Server = make_server();
        let (blue, red) = (address(50_002), address(50_001));
        join(&mut server, red, RED);
        join(&mut server, blue, BLUE);
        hold_console(&mut server, 1, red, 0);
        start_holding(&mut server, 1, red, 1);
        start_holding(&mut server, 2, blue, 2);
        server.update_consoles();
        assert_eq!((server.game.off_mask, server.game.winner), (0b011, Some(1)));
    }

    #[test]
    fn a_console_is_turned_off_only_from_within_reach_and_aimed_at() {
        let mut server: Server = make_server();
        let red: SocketAddr = address(50_001);
        join(&mut server, red, RED);
        let far: DMat4 = make_boost(AXIS_Z, FAR_M / TEST_CURVATURE_RADIUS_M);
        hold_console_from(&mut server, 1, red, 0, far);
        assert_eq!(server.game.off_mask, 0);
        let turned_away: DMat4 = make_rotation(AXIS_X, AXIS_Z, std::f64::consts::PI);
        hold_console_from(&mut server, 1, red, 0, turned_away);
        assert_eq!(server.game.off_mask, 0);
        hold_console_from(&mut server, 1, red, 0, DMat4::IDENTITY);
        assert_eq!(server.game.off_mask, 0b001);
    }

    #[test]
    fn holding_consoles_turns_them_off_and_wins() {
        let mut server: Server = make_server();
        let (blue, red) = (address(50_002), address(50_001));
        join(&mut server, red, RED);
        join(&mut server, blue, BLUE);
        hold_console(&mut server, 2, blue, 0);
        assert_eq!(server.game.off_mask, 0);
        hold_console(&mut server, 1, red, 0);
        assert_eq!((server.game.off_mask, server.game.winner), (0b001, None));
        hold_console(&mut server, 1, red, 1);
        assert_eq!((server.game.off_mask, server.game.winner), (0b011, Some(1)));
        server.start_new_game();
        assert_eq!(
            server.game,
            GameState {
                number: 1,
                off_mask: 0,
                winner: None
            }
        );
    }

    #[test]
    fn scores_survive_encoding() {
        let sent: Vec<Score> = vec![
            Score {
                id: 1,
                team: 1,
                kills: 3,
                deaths: 70_000,
            },
            Score {
                id: 4,
                team: 0,
                kills: 0,
                deaths: 2,
            },
        ];
        let read = |bytes: &[u8]| Reader { bytes, position: 1 }.scores();
        let message: Message = Message::New(Kind::Scores).scores(&sent);
        assert_eq!(read(&message.0), Some(sent.clone()));
        assert_eq!(
            read(&Message::New(Kind::Scores).scores(&[]).0),
            Some(Vec::new())
        );
        assert_eq!(read(&message.0[..message.0.len() - 1]), None);
        let mut too_long: Vec<u8> = message.0.clone();
        too_long.push(0);
        assert_eq!(read(&too_long), None);
        let unknown_team: Score = Score {
            team: TEAM_NAMES.len() as u32,
            ..sent[0]
        };
        assert_eq!(
            read(&Message::New(Kind::Scores).scores(&[unknown_team]).0),
            None
        );
    }

    #[test]
    fn friendly_fire_can_be_turned_off() {
        let mut server: Server = make_server();
        server.settings.shooting.is_friendly_fire_on = false;
        let (red_one, red_two, blue_three) = (address(50_001), address(50_002), address(50_003));
        join(&mut server, red_one, RED);
        join(&mut server, red_two, RED);
        join(&mut server, blue_three, BLUE);
        let token: u64 = get_token(&server, red_one);
        server
            .handle(&make_hit(token, 1, 2, 1), red_one)
            .expect("a hit on a teammate");
        assert_eq!(server.slots.len(), 3);
        server
            .handle(&make_hit(token, 1, 3, 1), red_one)
            .expect("a hit on an enemy");
        assert_eq!(server.slots.len(), 2);
    }

    #[test]
    fn eliminations_count_kills_and_deaths() {
        const RIFLE_HITS_TO_KILL: u8 = 2;
        let mut server: Server = make_server();
        let (red_one, blue_two, red_three) = (address(50_001), address(50_002), address(50_003));
        join(&mut server, red_one, RED);
        join(&mut server, blue_two, BLUE);
        join(&mut server, red_three, RED);
        server
            .handle(
                &make_hit(get_token(&server, red_one), 1, 2, RIFLE_HITS_TO_KILL),
                red_one,
            )
            .expect("a hit short of the limit");
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 0, 0), (2, 0, 0), (3, 0, 0)]
        );
        server
            .handle(
                &make_hit(get_token(&server, red_one), 1, 2, RIFLE_HITS_TO_KILL),
                red_one,
            )
            .expect("the last hit");
        assert_eq!(server.slots.len(), 2);
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 1, 0), (2, 0, 1), (3, 0, 0)]
        );
        server
            .handle(&make_hit(get_token(&server, red_one), 1, 3, 1), red_one)
            .expect("a hit on a teammate");
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 1, 0), (2, 0, 1), (3, 0, 1)]
        );
        join(&mut server, blue_two, BLUE);
        let wait_s: f64 = server.get_tally_mut(3).respawn_wait_s + PLAYER_TIMEOUT_S;
        server.get_tally_mut(3).eliminated_at =
            Some(Instant::now() - Duration::from_secs_f64(wait_s));
        assert_eq!(get_kills_and_deaths(&server), vec![(1, 1, 0), (2, 0, 1)]);
        server.start_new_game();
        assert_eq!(get_kills_and_deaths(&server), vec![(1, 0, 0), (2, 0, 0)]);
    }

    #[test]
    fn eliminations_carry_the_respawn_wait_of_the_levels_time() {
        let mut server: Server = make_server();
        let respawn: RespawnSettings = server.settings.respawn.clone();
        let shooter: SocketAddr = address(50_001);
        join(&mut server, shooter, RED);
        for (level_s, is_old_server, expected_wait_s) in [
            (0.0, false, respawn.start_s),
            (respawn.period_s, false, respawn.start_s + respawn.step_s),
            (respawn.period_s, true, respawn.start_s),
        ] {
            let mut victim: Client = connect_client(&mut server, BLUE);
            let victim_id: u32 = server.slots.last().expect("the victim").player.id;
            server.level_started_at = Instant::now() - Duration::from_secs_f64(level_s);
            if is_old_server {
                poll_until(&mut victim, |client| client.id.is_some());
                let old: Message = Message::New(Kind::Eliminated).id(victim_id).id(1);
                let victim_port: u16 = victim.socket.local_addr().expect("an address").port();
                let victim_address: SocketAddr =
                    SocketAddr::from((Ipv4Addr::LOCALHOST, victim_port));
                server.send(&old.0, victim_address);
            } else {
                let hit: Vec<u8> = make_hit(get_token(&server, shooter), 1, victim_id, 1);
                server.handle(&hit, shooter).expect("a hit");
                assert_eq!(
                    server.get_tally_mut(victim_id).respawn_wait_s,
                    expected_wait_s
                );
            }
            let deadline: Instant = Instant::now() + Duration::from_secs(1);
            let wait_s: f64 = loop {
                assert!(Instant::now() < deadline, "the elimination never arrived");
                let events: Vec<NetEvent> = victim.poll().expect("the welcome and the elimination");
                if let Some(wait_s) = events.into_iter().find_map(|event| match event {
                    NetEvent::Eliminated {
                        is_self: true,
                        respawn_wait_s,
                        ..
                    } => Some(respawn_wait_s),
                    _ => None,
                }) {
                    break wait_s;
                }
            };
            assert_eq!(wait_s, expected_wait_s);
        }
    }

    #[test]
    fn clients_the_server_dropped_join_again() {
        const SHORT_SILENCE_S: f64 = 1.0;
        let mut server: Server = make_server();
        let mut client: Client = connect_client(&mut server, RED);
        poll_until(&mut client, |client| client.id.is_some());
        let silent_since = |silent_s: f64| Some(Instant::now() - Duration::from_secs_f64(silent_s));
        client.first_unanswered.set(silent_since(SHORT_SILENCE_S));
        client.poll().expect("no datagrams");
        assert_eq!(client.id, Some(1));
        server.slots.clear();
        client
            .first_unanswered
            .set(silent_since(PLAYER_TIMEOUT_S + SHORT_SILENCE_S));
        client.poll().expect("no datagrams");
        assert_eq!(client.id, None);
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        let (length, from) = server.socket.recv_from(&mut buffer).expect("the join");
        server.handle(&buffer[..length], from).expect("the join");
        poll_until(&mut client, |client| client.id.is_some());
        assert_eq!((client.team, server.slots.len()), (Some(u32::from(RED)), 1));
    }

    #[test]
    fn clients_receive_scores() {
        let mut server: Server = make_server();
        let mut client: Client = connect_client(&mut server, RED);
        server.send_scores().expect("the scores");
        let deadline: Instant = Instant::now() + Duration::from_secs(1);
        while client.scores.is_none() {
            assert!(Instant::now() < deadline, "the scores never arrived");
            client.poll().expect("the welcome and the scores");
        }
        assert_eq!(client.welcomed_id, Some(1));
        assert_eq!(
            client.scores,
            Some(vec![Score {
                id: 1,
                team: u32::from(RED),
                kills: 0,
                deaths: 0
            }])
        );
    }

    #[test]
    fn snapshots_carry_how_far_consoles_are_turned_off() {
        let mut server: Server = make_server();
        let mut watcher: Client = connect_client(&mut server, BLUE);
        let (holder, idle) = (address(50_001), address(50_002));
        join(&mut server, holder, RED);
        join(&mut server, idle, RED);
        let holding: RemotePlayer = RemotePlayer {
            id: 2,
            frame: DMat4::IDENTITY,
            is_laser_on: false,
            is_rifle: false,
            team: u32::from(RED),
            disabling_console: Some(0),
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            crouch_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        server
            .handle(
                &Message::New(Kind::State)
                    .token(get_token(&server, holder))
                    .player(&holding)
                    .0,
                holder,
            )
            .expect("a state");
        let hold_s: f64 = server.settings.consoles.hold_to_disable_s;
        server.slots[1].disabling_since =
            Some(Instant::now() - Duration::from_secs_f64(HALF * hold_s));
        server.send_snapshot().expect("a snapshot");
        let deadline: Instant = Instant::now() + Duration::from_secs(1);
        while watcher.players.len() < 2 {
            assert!(Instant::now() < deadline, "the snapshot never arrived");
            watcher.poll().expect("the welcome and a snapshot");
        }
        let fracs: Vec<(u32, Option<u8>, f64)> = watcher
            .players
            .iter()
            .map(|p| (p.id, p.disabling_console, p.disabling_frac))
            .collect();
        assert_eq!((fracs[0].0, fracs[0].1), (2, Some(0)));
        assert!(
            (fracs[0].2 - HALF).abs() <= DISABLING_FRAC_STEP,
            "{fracs:?}"
        );
        assert_eq!(fracs[1], (3, None, 0.0));
        server.slots[1].disabling_since =
            Some(Instant::now() - Duration::from_secs_f64(TWICE * hold_s));
        assert_eq!(server.get_disabling_frac(&server.slots[1]), 1.0);
    }

    #[test]
    fn joins_need_the_challenge_answered() {
        let mut server: Server = make_server();
        let listener: UdpSocket = make_listener();
        let from: SocketAddr = listener.local_addr().expect("an address");
        let mut unanswered: Vec<u8> =
            make_join(&server, from, ANY_TEAM, PROTOCOL_VERSION, TEST_PASSWORD);
        unanswered[1 + BYTES_PER_VERSION..1 + BYTES_PER_VERSION + BYTES_PER_COOKIE].fill(0);
        let others: Vec<u8> = make_join(
            &server,
            address(50_009),
            ANY_TEAM,
            PROTOCOL_VERSION,
            TEST_PASSWORD,
        );
        for message in [unanswered, others] {
            server.handle(&message, from).expect("a join");
            let challenge: Vec<u8> = receive(&listener).expect("a challenge");
            assert_eq!(
                challenge,
                [
                    vec![Kind::Challenge as u8],
                    server.make_cookie(from).to_vec()
                ]
                .concat()
            );
            assert!(server.slots.is_empty());
        }
        join(&mut server, from, ANY_TEAM);
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn other_versions_and_wrong_passwords_are_refused() {
        let mut server: Server = make_server();
        let listener: UdpSocket = make_listener();
        let from: SocketAddr = listener.local_addr().expect("an address");
        let version: [u8; BYTES_PER_VERSION] = PROTOCOL_VERSION.to_le_bytes();
        for (message, reason) in [
            (
                make_join(&server, from, ANY_TEAM, PROTOCOL_VERSION + 1, TEST_PASSWORD),
                REFUSED_FOR_VERSION,
            ),
            (
                make_join(&server, from, ANY_TEAM, PROTOCOL_VERSION, b"wrong horse"),
                REFUSED_FOR_PASSWORD,
            ),
        ] {
            server.handle(&message, from).expect("a join");
            let refusal: Vec<u8> = receive(&listener).expect("a refusal");
            assert_eq!(
                refusal,
                [Kind::Refused as u8, reason, version[0], version[1]]
            );
            assert!(server.slots.is_empty());
        }
    }

    #[test]
    fn clients_join_with_the_password_only() {
        let mut server: Server = make_server();
        let server_address: SocketAddr = server.socket.local_addr().expect("an address");
        let mut client: Client =
            Client::Connect(server_address, TEST_MAP_NAME, TEST_PASSWORD).expect("a socket");
        client.join(None).expect("a join request");
        let events: Vec<NetEvent> = exchange(&mut server, &mut client).expect("a welcome");
        assert!(matches!(events[..], [NetEvent::Joined(1)]));
        let mut stranger: Client =
            Client::Connect(server_address, TEST_MAP_NAME, b"wrong horse").expect("a socket");
        stranger.join(None).expect("a join request");
        let error: Box<dyn Error> = exchange(&mut server, &mut stranger)
            .err()
            .expect("a refusal");
        assert!(error.downcast_ref::<PasswordRefused>().is_some(), "{error}");
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn messages_need_the_senders_token() {
        let mut server: Server = make_server();
        let (shooter, victim) = (address(50_001), address(50_002));
        join(&mut server, shooter, ANY_TEAM);
        join(&mut server, victim, ANY_TEAM);
        for token in [0, get_token(&server, victim)] {
            server
                .handle(&make_hit(token, 1, 2, 1), shooter)
                .expect("a hit");
            assert_eq!(server.slots.len(), 2);
        }
        server
            .handle(&make_hit(get_token(&server, shooter), 1, 2, 1), shooter)
            .expect("a hit");
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn short_lobby_queries_are_ignored() {
        let mut server: Server = make_server();
        let listener: UdpSocket = make_listener();
        let from: SocketAddr = listener.local_addr().expect("an address");
        server
            .handle(&[Kind::LobbyQuery as u8], from)
            .expect("a short query");
        assert_eq!(receive(&listener), None);
        server.handle(&make_lobby_query().0, from).expect("a query");
        assert_eq!(receive(&listener), Some(vec![Kind::Lobby as u8, 0, 0]));
    }

    #[test]
    fn tests_listen_on_loopback_only() {
        let socket: UdpSocket = bind_server(0).expect("a free local port");
        assert_eq!(socket.local_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    }
}
