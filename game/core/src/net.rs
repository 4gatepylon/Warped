//! `net.rs` is the window's side of a game on a game server, and the messages both sides send. The
//! server (`warped-server`'s `game.rs`, its own process, without a window) keeps every player's eye
//! frame, laser switch and the console they hold `E` on (with how far along turning it off they
//! are, so every window speeds up its pulse alike), sends everyone a snapshot 60 times a second,
//! removes a player another player's laser hit, turns off a console held for the settings'
//! `consoles.hold_to_disable_s`, ends the game when a team's consoles are all off, and announces
//! itself on the network so others can find it. It counts each player's kills and deaths in the
//! game and sends everyone the scores 4 times a second. A client joins, sends its own player each
//! frame and receives the others, the game's state and the scores.
//!
//! Interface: `DEFAULT_PORT`, `PROTOCOL_VERSION`, `MAX_PLAYER_COUNT`, `MAX_CONSOLE_COUNT`,
//! `TEAM_NAMES`, `PasswordRefused`, `get_team`, `read_game_password`, `find_server`, `Client`,
//! `RemotePlayer`, `GameState`, `Score`, `NetEvent`; and, for the server in `warped-server`, the
//! wire format: `Kind`, `Message`, `Reader`,
//! `make_proof`, `is_proof_right`, `make_random_bytes`, `ANY_TEAM`, `DISCOVERY_PORT`,
//! `MAX_DATAGRAM_BYTES`, `BEACON_MAGIC`, the `BYTES_PER_*` sizes a join carries and the
//! `REFUSED_FOR_*` reasons.
//!
//! Example: `let mut client = Client::Connect(net::find_server(Duration::from_secs(5))?)?;`
//!
//! Must know:
//! - Messages are single UDP datagrams, little-endian, starting with a `Kind` byte. A lost state
//!   or snapshot is replaced by the next one; joins are resent until welcomed. Nothing is
//!   encrypted.
//! - A join carries `PROTOCOL_VERSION`, and must answer the server's challenge (a cookie keyed to
//!   the sender's address) with a keyed hash of the game's password, so the password never
//!   crosses the network; the server refuses another version or a wrong password. Every later
//!   message carries the token the welcome gave, so nobody else can speak for a player.
//! - A failed send is logged, not fatal, and no reply is bigger than its request until the sender
//!   has proven its address, so the server can't be used to flood someone else.
//! - Each client decides when its own laser hits someone (it traces the laser anyway) and which
//!   console it aims at, and the server takes its word.
//! - A player keeps their id, and so their team, when they rejoin from the same address.
//! - The server's welcome names its map and carries its game settings; a client of another map
//!   fails rather than playing on.
//! - An elimination counts a death for the player hit and a kill for the shooter, unless they are on
//!   the same team. Scores go back to 0 when a new game starts.

use std::cell::Cell;
use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec4};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::settings::ServerSettings;
use crate::shot_noise::{self, BEAM_SPECKLE_LEVEL_COUNT};
use crate::thrown_wormholes::{MouthPlace, PlayerWormholes, WormholeColor};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const DEFAULT_PORT: u16 = 47_800;
pub const PROTOCOL_VERSION: u16 = 1;
pub use crate::map::{MAX_CONSOLE_COUNT, MAX_PLAYER_COUNT, TEAM_NAMES};
const NO_CONSOLE: u8 = u8::MAX;
const RIFLE_BIT: u8 = 1;
const TEAM_BIT: u8 = 2;
const SPECKLE_FIRST_BIT: u8 = 3;
pub const ANY_TEAM: u8 = u8::MAX;
const NO_WINNER: u8 = u8::MAX;
pub const DISCOVERY_PORT: u16 = 47_801;
const JOIN_RETRY_S: f64 = 0.5;
const BEACON_GATHER_S: f64 = 1.0;
const OTHER_VERSION_WINDOW_S: f64 = 1.0;
pub const MAX_DATAGRAM_BYTES: usize = 4096;
pub const BEACON_MAGIC: &[u8; 8] = b"WORMHOLE";
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
const BYTES_PER_PLAYER: usize =
    BYTES_PER_ID + FLOATS_PER_FRAME * BYTES_PER_FLOAT + 3 + BYTES_PER_WORMHOLES;
const BYTES_PER_GAME: usize = 2 * BYTES_PER_ID + 1;
pub const BYTES_PER_VERSION: usize = 2;
pub const BYTES_PER_COOKIE: usize = 16;
pub const BYTES_PER_PROOF: usize = 32;
const BYTES_PER_TOKEN: usize = 8;
pub const BYTES_PER_LOBBY: usize = 1 + TEAM_NAMES.len();
pub const REFUSED_FOR_VERSION: u8 = 1;
pub const REFUSED_FOR_PASSWORD: u8 = 2;
const _: () =
    assert!(2 + MAX_PLAYER_COUNT * (BYTES_PER_PLAYER + 1) + BYTES_PER_GAME <= MAX_DATAGRAM_BYTES);

/// `Kind` is a message's first byte.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
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
    pub fn FromByte(byte: u8) -> Option<Kind> {
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
/// 1 / `shot_noise::BEAM_SPECKLE_LEVEL_COUNT`), and their thrown wormholes.
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

/// `NetEvent` is something the server told this client.
pub enum NetEvent {
    Joined(u32),
    Eliminated {
        victim_id: u32,
        shooter_id: u32,
        is_self: bool,
    },
}

/// `Message` builds a datagram.
pub struct Message(pub Vec<u8>);

impl Message {
    /// `New` starts a message of a kind.
    ///
    /// Example: `let message = Message::New(Kind::Join);`
    #[allow(non_snake_case)]
    pub fn New(kind: Kind) -> Message {
        Message(vec![kind as u8])
    }

    /// `id` appends a player id.
    ///
    /// Example: `let message = Message::New(Kind::Leave).id(3);`
    pub fn id(mut self, id: u32) -> Message {
        self.0.extend_from_slice(&id.to_le_bytes());
        self
    }

    /// `token` appends the token a welcome gave a player.
    ///
    /// Example: `let message = Message::New(Kind::Leave).token(token).id(3);`
    pub fn token(mut self, token: u64) -> Message {
        self.0.extend_from_slice(&token.to_le_bytes());
        self
    }

    /// `player` appends a player: id, eye frame, laser switch (with the gun, the team and the beam
    /// speckle's level in its higher bits), the console they hold `E` on, how far along turning it
    /// off they are (a byte, 255 for done), and their wormholes.
    ///
    /// Example: `let message = Message::New(Kind::State).player(&player);`
    pub fn player(mut self, player: &RemotePlayer) -> Message {
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
        self.wormholes(&player.wormholes)
    }

    /// `wormholes` appends a player's wormholes: which are down or flying (and the flying one's
    /// colour), each colour's place (u, v, h, yaw), and the flying one's point, as `f32`s (zeros
    /// for what isn't there), then the pair's damage (a byte, 255 for destroyed).
    ///
    /// Example: `let message = message.wormholes(&PlayerWormholes::default());`
    fn wormholes(mut self, wormholes: &PlayerWormholes) -> Message {
        let flying_color: Option<WormholeColor> = wormholes.flying.map(|(color, _)| color);
        self.0.push(
            u8::from(wormholes.purple.is_some()) << PURPLE_DOWN_BIT
                | u8::from(wormholes.green.is_some()) << GREEN_DOWN_BIT
                | u8::from(flying_color.is_some()) << FLYING_BIT
                | u8::from(flying_color == Some(WormholeColor::Green)) << FLYING_GREEN_BIT,
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
    pub fn game(mut self, game: &GameState) -> Message {
        self.0.extend_from_slice(&game.number.to_le_bytes());
        self.0.extend_from_slice(&game.off_mask.to_le_bytes());
        self.0
            .push(game.winner.map_or(NO_WINNER, |team| team as u8));
        self
    }

    /// `scores` appends a count and that many scores: each player's id, team, kills and deaths.
    ///
    /// Example: `let message = Message::New(Kind::Scores).scores(&server.get_scores());`
    pub fn scores(mut self, scores: &[Score]) -> Message {
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
pub struct Reader<'a> {
    pub bytes: &'a [u8],
    pub position: usize,
}

impl Reader<'_> {
    /// `take` gives the next `count` bytes, or `None` if the datagram is too short.
    ///
    /// Example: `let id_bytes = reader.take(4)?;`
    pub fn take(&mut self, count: usize) -> Option<&[u8]> {
        let slice: &[u8] = self.bytes.get(self.position..self.position + count)?;
        self.position += count;
        Some(slice)
    }

    /// `id` reads a player id.
    ///
    /// Example: `let id = reader.id()?;`
    pub fn id(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(
            self.take(BYTES_PER_ID)?.try_into().ok()?,
        ))
    }

    /// `array` reads the next `N` bytes as an array.
    ///
    /// Example: `let cookie: [u8; 16] = reader.array()?;`
    pub fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    /// `token` reads a token written by `Message::token`.
    ///
    /// Example: `let token = reader.token()?;`
    pub fn token(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array::<BYTES_PER_TOKEN>()?))
    }

    /// `player` reads a player written by `Message::player`.
    ///
    /// Example: `let player = reader.player()?;`
    pub fn player(&mut self) -> Option<RemotePlayer> {
        let id: u32 = self.id()?;
        let mut values: [f64; FLOATS_PER_FRAME] = [0.0; FLOATS_PER_FRAME];
        for value in &mut values {
            *value = f64::from_le_bytes(self.take(BYTES_PER_FLOAT)?.try_into().ok()?);
        }
        let laser: u8 = self.take(1)?[0];
        let console: u8 = self.take(1)?[0];
        let disabling: u8 = self.take(1)?[0];
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
            wormholes: self.wormholes()?,
        })
    }

    /// `wormholes` reads a player's wormholes written by `Message::wormholes`.
    ///
    /// Example: `let wormholes = reader.wormholes()?;`
    fn wormholes(&mut self) -> Option<PlayerWormholes> {
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
        Some(PlayerWormholes {
            purple: place(0, PURPLE_DOWN_BIT),
            green: place(F32S_PER_PLACE, GREEN_DOWN_BIT),
            flying: is_set(FLYING_BIT).then_some((color, point)),
            damage_frac: f64::from(self.take(1)?[0]) / f64::from(u8::MAX),
        })
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
pub fn make_proof(password: &[u8], cookie: &[u8]) -> [u8; BYTES_PER_PROOF] {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(password).expect("HMAC takes a key of any length");
    mac.update(cookie);
    mac.finalize().into_bytes().into()
}

/// `is_proof_right` says, in constant time, whether `proof` is `make_proof(password, cookie)`.
///
/// Example: `if is_proof_right(&server.password, &cookie, &proof) { ... }`
pub fn is_proof_right(password: &[u8], cookie: &[u8], proof: &[u8]) -> bool {
    let mut mac: Hmac<Sha256> =
        Hmac::new_from_slice(password).expect("HMAC takes a key of any length");
    mac.update(cookie);
    mac.verify_slice(proof).is_ok()
}

/// `make_random_bytes` gives `N` bytes from the operating system's random number generator.
///
/// Example: `let token = u64::from_le_bytes(make_random_bytes());`
pub fn make_random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes: [u8; N] = [0; N];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
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

/// `find_server` waits up to `timeout` for a server's beacon on the local network and gives its
/// address.
///
/// Example: `let address = net::find_server(Duration::from_secs(5))?;`
pub fn find_server(timeout: Duration) -> Result<SocketAddr> {
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)).map_err(|error| {
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
        let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
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
    /// scores, and returns what happened; while not yet welcomed it asks to join again every `JOIN_RETRY_S`. It
    /// fails if the server plays another map.
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
                    let count: usize = reader.take(1).map_or(0, |count| usize::from(count[0]));
                    if length != 2 + count * (BYTES_PER_PLAYER + 1) + BYTES_PER_GAME {
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
                    self.players = players
                        .into_iter()
                        .filter(|player| Some(player.id) != self.id)
                        .collect();
                }
                Some(Kind::Eliminated) => {
                    if let (Some(victim_id), Some(shooter_id)) = (reader.id(), reader.id()) {
                        let is_self: bool = Some(victim_id) == self.id;
                        if is_self {
                            self.id = None;
                            self.token = None;
                            self.players.clear();
                        }
                        self.players.retain(|player| player.id != victim_id);
                        events.push(NetEvent::Eliminated {
                            victim_id,
                            shooter_id,
                            is_self,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(events)
    }

    /// `send_state` sends this player's eye frame, laser switch, whether the gun is the rifle, the
    /// console they hold `E` on, their shot's beam speckle and their wormholes, once joined. The
    /// server times the hold itself, so the progress sent is 0.
    ///
    /// Example: `client.send_state(player.frame, is_laser_on, false, None, 0.0, PlayerWormholes::default())?;`
    pub fn send_state(
        &mut self,
        frame: DMat4,
        is_laser_on: bool,
        is_rifle: bool,
        disabling_console: Option<u8>,
        beam_speckle_frac: f64,
        wormholes: PlayerWormholes,
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
    //! Tests of the message encoding and of what a window notices about its server, against a fake
    //! server's socket; `warped-server` tests the server, and the two together over loopback.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Datagram length | complete (progress kept to a 255th; the beam speckle's top level beside the laser, rifle and red-team bits), cut short | `players_survive_encoding`, `short_datagrams_read_as_nothing` |
    //! | Player's wormholes | none, one down with a green throw in the air, both down and damaged | `players_survive_encoding` |
    //! | Servers announcing themselves | one, several with one on the default port (chosen), several without (the first) | `the_default_port_wins_among_servers` |
    //! | A server that can't take this player | full (says so), of another version (snapshots that don't fit) | `full_games_and_other_versions_are_noticed` |
    //! | Welcome size | the built-in settings with the longest map name | `the_welcome_fits_a_datagram` |
    //! | Scores datagram | complete (several players, none), cut short, too long, an unknown team | `scores_survive_encoding` |

    use super::*;

    const TEST_MAP_NAME: &str = "lunar_surface";
    const TEST_PASSWORD: &[u8] = b"correct horse";
    const LISTEN_MS: u64 = 200;
    const SENT_DAMAGE_BYTE: u8 = 51;
    const SENT_DISABLING_FRAC: f64 = 0.37;
    const SENT_SPECKLE_FRAC: f64 = 1.0;
    const DISABLING_FRAC_STEP: f64 = 1.0 / u8::MAX as f64;

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
        let server: UdpSocket = make_listener();
        let server_address: SocketAddr = server.local_addr().expect("an address");
        let mut client: Client =
            Client::Connect(server_address, TEST_MAP_NAME, TEST_PASSWORD).expect("a socket");
        client.join(None).expect("a join request");
        let client_address: SocketAddr =
            address(client.socket.local_addr().expect("an address").port());
        let full: Message = Message::New(Kind::Full);
        server
            .send_to(&full.0, client_address)
            .expect("a full game");
        poll_until(&mut client, Client::is_full);
        assert!(client.is_full() && client.is_waiting_for_welcome());
        assert!(!client.is_other_version());
        let short_snapshot: [u8; 3] = [Kind::Snapshot as u8, 1, 0];
        server
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
    fn the_welcome_fits_a_datagram() {
        let header_bytes: usize = 1 + BYTES_PER_ID + BYTES_PER_TOKEN + 1 + usize::from(u8::MAX);
        assert!(header_bytes + ServerSettings::Builtin().to_json().len() <= MAX_DATAGRAM_BYTES);
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
}
