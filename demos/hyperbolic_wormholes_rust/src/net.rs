//! `net.rs` lets players on one local network share a game. A server (its own process, without a
//! window) keeps every player's eye frame, laser switch and the console they hold `E` on, sends
//! everyone a snapshot 60 times a second, removes a player another player's laser hit, turns off a
//! console held for the settings' `consoles.hold_to_disable_s`, ends the game when a team's consoles are all off, and
//! announces itself on the network so others can find it. A client joins, sends its own player
//! each frame and receives the others and the game's state.
//!
//! Interface: `DEFAULT_PORT`, `MAX_PLAYER_COUNT`, `MAX_CONSOLE_COUNT`,
//! `TEAM_NAMES`, `get_team`, `bind_server`, `serve`, `find_server`, `Client`, `RemotePlayer`,
//! `GameState`, `NetEvent`.
//!
//! Example: `let mut client = Client::Connect(net::find_server(Duration::from_secs(5))?)?;`
//!
//! Must know:
//! - Messages are single UDP datagrams, little-endian, starting with a `Kind` byte. A lost state
//!   or snapshot is replaced by the next one; joins are resent until welcomed. Nothing is
//!   encrypted or authenticated: it is for a trusted local network.
//! - Each client decides when its own laser hits someone (it traces the laser anyway) and which
//!   console it aims at, and the server takes its word.
//! - A player keeps their id, and so their team, when they rejoin from the same address.
//! - The server's welcome names its map and carries its game settings; a client of another map
//!   fails rather than playing on.

use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use glam::DMat4;

use crate::settings::GameSettings;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const DEFAULT_PORT: u16 = 47_800;
pub const MAX_PLAYER_COUNT: usize = 8;
pub const MAX_CONSOLE_COUNT: usize = 32;
pub const TEAM_NAMES: [&str; 2] = ["blue", "red"];
const NO_CONSOLE: u8 = u8::MAX;
const RIFLE_BIT: u8 = 1;
const NO_WINNER: u8 = u8::MAX;
const DAMAGE_TOLERANCE_FRAC: f64 = 1e-9;
const DISCOVERY_PORT: u16 = 47_801;
const SNAPSHOT_PERIOD_S: f64 = 1.0 / 60.0;
const BEACON_PERIOD_S: f64 = 0.5;
const JOIN_RETRY_S: f64 = 0.5;
const PLAYER_TIMEOUT_S: f64 = 3.0;
const SERVER_POLL_MS: u64 = 2;
const MAX_DATAGRAM_BYTES: usize = 2048;
const BEACON_MAGIC: &[u8; 8] = b"WORMHOLE";
const FLOATS_PER_FRAME: usize = 16;
const BYTES_PER_FLOAT: usize = 8;
const BYTES_PER_ID: usize = 4;
const BYTES_PER_PORT: usize = 2;
const BYTES_PER_PLAYER: usize = BYTES_PER_ID + FLOATS_PER_FRAME * BYTES_PER_FLOAT + 2;
const BYTES_PER_GAME: usize = 2 * BYTES_PER_ID + 1;

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
        ]
        .into_iter()
        .find(|kind| *kind as u8 == byte)
    }
}

/// `get_team` gives a player's team, an index into `TEAM_NAMES`: ids alternate between them.
///
/// Example: `let team = net::get_team(3);`
pub fn get_team(id: u32) -> u32 {
    id % TEAM_NAMES.len() as u32
}

/// `RemotePlayer` is another player as the last snapshot gave them: their eye frame, whether their
/// laser is on and from which gun, and the console they hold `E` on, if any.
#[derive(Clone, Copy)]
pub struct RemotePlayer {
    pub id: u32,
    pub frame: DMat4,
    pub is_laser_on: bool,
    pub is_rifle: bool,
    pub disabling_console: Option<u8>,
}

/// `GameState` is the game as the server keeps it: which game this is (from 0, one more each new
/// game), the consoles turned off (bit i for console i) and the winning team once there is one.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct GameState {
    pub number: u32,
    pub off_mask: u32,
    pub winner: Option<u32>,
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

    /// `player` appends a player: id, eye frame, laser switch and the console they hold `E` on.
    ///
    /// Example: `let message = Message::New(Kind::State).player(&player);`
    fn player(mut self, player: &RemotePlayer) -> Message {
        self = self.id(player.id);
        for value in player.frame.to_cols_array() {
            self.0.extend_from_slice(&value.to_le_bytes());
        }
        self.0
            .push(u8::from(player.is_laser_on) | u8::from(player.is_rifle) << RIFLE_BIT);
        self.0.push(player.disabling_console.unwrap_or(NO_CONSOLE));
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
        Some(RemotePlayer {
            id,
            frame: DMat4::from_cols_array(&values),
            is_laser_on: laser & 1 == 1,
            is_rifle: laser >> RIFLE_BIT & 1 == 1,
            disabling_console: (console != NO_CONSOLE).then_some(console),
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
}

/// `Slot` is a joined player on the server, since when they have held `E` on their console, and
/// the damage they took since joining (1 eliminates them).
struct Slot {
    player: RemotePlayer,
    address: SocketAddr,
    last_heard: Instant,
    disabling_since: Option<Instant>,
    damage_frac: f64,
}

/// `Server` is the server's state: its socket, its map's name, the joined players, the next id to
/// give, the id each address had, each console's team and the game.
struct Server {
    socket: UdpSocket,
    map_name: String,
    settings: GameSettings,
    slots: Vec<Slot>,
    next_id: u32,
    known_ids: Vec<(SocketAddr, u32)>,
    console_teams: Vec<u32>,
    game: GameState,
}

impl Server {
    /// `handle` reacts to one datagram from `address`.
    ///
    /// Example: `server.handle(&buffer[..length], address)?;`
    fn handle(&mut self, bytes: &[u8], address: SocketAddr) -> Result<()> {
        let mut reader: Reader = Reader { bytes, position: 1 };
        match bytes.first().copied().and_then(Kind::FromByte) {
            Some(Kind::Join) => {
                let existing: Option<u32> = self
                    .slots
                    .iter()
                    .find(|slot| slot.address == address)
                    .map(|slot| slot.player.id);
                let id: u32 = match existing {
                    Some(id) => id,
                    None if self.slots.len() < MAX_PLAYER_COUNT => {
                        let id: u32 = match self.known_ids.iter().find(|(a, _)| *a == address) {
                            Some(&(_, id)) => id,
                            None => {
                                self.next_id += 1;
                                self.known_ids.push((address, self.next_id));
                                self.next_id
                            }
                        };
                        self.slots.push(Slot {
                            player: RemotePlayer {
                                id,
                                frame: DMat4::IDENTITY,
                                is_laser_on: false,
                                is_rifle: false,
                                disabling_console: None,
                            },
                            address,
                            last_heard: Instant::now(),
                            disabling_since: None,
                            damage_frac: 0.0,
                        });
                        println!("player {id} joined from {address}");
                        id
                    }
                    None => return Ok(()),
                };
                let mut welcome: Message = Message::New(Kind::Welcome).id(id);
                welcome.0.push(self.map_name.len() as u8);
                welcome.0.extend_from_slice(self.map_name.as_bytes());
                welcome.0.extend_from_slice(&self.settings.to_json());
                self.socket.send_to(&welcome.0, address)?;
            }
            Some(Kind::State) => {
                if let Some(mut player) = reader.player()
                    && let Some(slot) = self
                        .slots
                        .iter()
                        .position(|slot| slot.address == address && slot.player.id == player.id)
                {
                    player.disabling_console = player
                        .disabling_console
                        .filter(|&console| self.can_disable(player.id, console));
                    let slot: &mut Slot = &mut self.slots[slot];
                    if player.disabling_console != slot.player.disabling_console {
                        slot.disabling_since = player.disabling_console.map(|_| Instant::now());
                    }
                    slot.player = player;
                    slot.last_heard = Instant::now();
                }
            }
            Some(Kind::Hit) => {
                let (Some(shooter_id), Some(victim_id), Some(&[hits_to_kill])) =
                    (reader.id(), reader.id(), reader.take(1))
                else {
                    return Ok(());
                };
                let is_shooter_here: bool = self
                    .slots
                    .iter()
                    .any(|slot| slot.address == address && slot.player.id == shooter_id);
                let Some(victim) = self
                    .slots
                    .iter()
                    .position(|slot| slot.player.id == victim_id)
                else {
                    return Ok(());
                };
                if !is_shooter_here || self.game.winner.is_some() || hits_to_kill == 0 {
                    return Ok(());
                }
                self.slots[victim].damage_frac += 1.0 / f64::from(hits_to_kill);
                if self.slots[victim].damage_frac < 1.0 - DAMAGE_TOLERANCE_FRAC {
                    return Ok(());
                }
                let victim_address: SocketAddr = self.slots.remove(victim).address;
                println!("player {victim_id} was hit by player {shooter_id}");
                let message: Message = Message::New(Kind::Eliminated).id(victim_id).id(shooter_id);
                for target in self
                    .slots
                    .iter()
                    .map(|slot| slot.address)
                    .chain([victim_address])
                {
                    self.socket.send_to(&message.0, target)?;
                }
            }
            Some(Kind::Leave) => {
                if let Some(id) = reader.id() {
                    self.slots
                        .retain(|slot| !(slot.address == address && slot.player.id == id));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// `can_disable` says whether player `id` may turn off `console`: an existing console of the
    /// other team, still on, in a game nobody has won yet.
    ///
    /// Example: `if server.can_disable(3, 7) { ... }`
    fn can_disable(&self, id: u32, console: u8) -> bool {
        let index: usize = usize::from(console);
        self.game.winner.is_none()
            && index < self.console_teams.len()
            && self.console_teams[index] != get_team(id)
            && self.game.off_mask >> index & 1 == 0
    }

    /// `update_consoles` turns off each console a player has held `E` on for `consoles.hold_to_disable_s`,
    /// and declares the other team the winner once all of a team's consoles are off.
    ///
    /// Example: `server.update_consoles();`
    fn update_consoles(&mut self) {
        for slot in &mut self.slots {
            if let (Some(console), Some(since)) =
                (slot.player.disabling_console, slot.disabling_since)
                && since.elapsed().as_secs_f64() >= self.settings.consoles.hold_to_disable_s
            {
                self.game.off_mask |= 1 << console;
                (slot.player.disabling_console, slot.disabling_since) = (None, None);
                println!("player {} turned off console {console}", slot.player.id);
            }
        }
        let losers: Vec<u32> = (0..TEAM_NAMES.len() as u32)
            .filter(|&team| {
                let mut consoles = (0..self.console_teams.len())
                    .filter(|&i| self.console_teams[i] == team)
                    .peekable();
                consoles.peek().is_some() && consoles.all(|i| self.game.off_mask >> i & 1 == 1)
            })
            .collect();
        if self.game.winner.is_none()
            && let [loser] = losers[..]
        {
            let winner: u32 = (loser + 1) % TEAM_NAMES.len() as u32;
            self.game.winner = Some(winner);
            println!(
                "The {} team won. Press Enter here to start a new game.",
                TEAM_NAMES[winner as usize]
            );
        }
    }

    /// `start_new_game` turns every console back on and clears the winner; clients see the game's
    /// number change and go back to their spawn.
    ///
    /// Example: `server.start_new_game();`
    fn start_new_game(&mut self) {
        self.game = GameState {
            number: self.game.number + 1,
            ..GameState::default()
        };
        for slot in &mut self.slots {
            (slot.player.disabling_console, slot.disabling_since) = (None, None);
        }
        println!("Game {} started.", self.game.number);
    }

    /// `send_snapshot` sends every player the list of all players, the game's state and each
    /// player's health (a byte each, 255 full).
    ///
    /// Example: `server.send_snapshot()?;`
    fn send_snapshot(&self) -> Result<()> {
        let mut message: Message = Message::New(Kind::Snapshot);
        message.0.push(self.slots.len() as u8);
        for slot in &self.slots {
            message = message.player(&slot.player);
        }
        message = message.game(&self.game);
        for slot in &self.slots {
            let health_frac: f64 = (1.0 - slot.damage_frac).clamp(0.0, 1.0);
            message
                .0
                .push((health_frac * f64::from(u8::MAX)).round() as u8);
        }
        for slot in &self.slots {
            self.socket.send_to(&message.0, slot.address)?;
        }
        Ok(())
    }
}

/// `bind_server` opens the server's socket on `port`, so a port already in use fails at once.
///
/// Example: `let socket = net::bind_server(net::DEFAULT_PORT)?;`
pub fn bind_server(port: u16) -> Result<UdpSocket> {
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|error| {
        format!("net: can't listen on UDP port {port} ({error}); pick another with --port")
    })?;
    socket.set_read_timeout(Some(Duration::from_millis(SERVER_POLL_MS)))?;
    Ok(socket)
}

/// `serve` runs a server for the map `map_name` with `settings` on `socket` (from `bind_server`)
/// until `should_stop` is set: it answers joins (naming the map and sending the settings), relays
/// players, removes players who are hit or silent for `PLAYER_TIMEOUT_S`, turns off
/// consoles (`console_teams` gives each one's team) and ends the game when a team's are all off,
/// starts a new game after that when `should_start_new_game` is set (clearing it), and broadcasts
/// a beacon on the local network every `BEACON_PERIOD_S` so `find_server` can find it.
///
/// Example: `net::serve(net::bind_server(net::DEFAULT_PORT)?, &should_stop, "lunar_surface".to_string(), GameSettings::Builtin(), vec![0, 1], &should_start_new_game)?;`
pub fn serve(
    socket: UdpSocket,
    should_stop: &AtomicBool,
    map_name: String,
    settings: GameSettings,
    console_teams: Vec<u32>,
    should_start_new_game: &AtomicBool,
) -> Result<()> {
    assert!(
        console_teams.len() <= MAX_CONSOLE_COUNT,
        "serve: {} consoles, more than {MAX_CONSOLE_COUNT}",
        console_teams.len()
    );
    let port: u16 = socket.local_addr()?.port();
    let beacon_socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    beacon_socket.set_broadcast(true)?;
    let mut beacon: Vec<u8> = vec![Kind::Beacon as u8];
    beacon.extend_from_slice(BEACON_MAGIC);
    beacon.extend_from_slice(&port.to_le_bytes());
    let mut server: Server = Server {
        socket,
        map_name,
        settings,
        slots: Vec::new(),
        next_id: 0,
        known_ids: Vec::new(),
        console_teams,
        game: GameState::default(),
    };
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    let (mut last_snapshot, mut last_beacon) = (Instant::now(), None::<Instant>);
    let mut has_warned_about_beacon: bool = false;
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
        if last_beacon.is_none_or(|time| time.elapsed().as_secs_f64() >= BEACON_PERIOD_S) {
            if let Err(error) =
                beacon_socket.send_to(&beacon, (Ipv4Addr::BROADCAST, DISCOVERY_PORT))
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
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)).map_err(|error| {
        format!("net: can't listen for servers on UDP port {DISCOVERY_PORT} ({error}); join with --join <address>")
    })?;
    socket.set_read_timeout(Some(timeout))?;
    let start: Instant = Instant::now();
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    while start.elapsed() < timeout {
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
            return Ok(SocketAddr::new(address.ip(), port));
        }
    }
    Err(format!(
        "net: no server announced itself within {} s; start one with --host, or join with --join <address>",
        timeout.as_secs()
    )
    .into())
}

/// `Client` is this player's connection: its map's name, its id and the server's game settings
/// once welcomed, the other players, the game, and this player's health (1 full, from the server).
pub struct Client {
    socket: UdpSocket,
    server: SocketAddr,
    map_name: String,
    pub id: Option<u32>,
    pub settings: Option<GameSettings>,
    pub players: Vec<RemotePlayer>,
    pub game: GameState,
    pub health_frac: f64,
    last_join: Instant,
    pub last_heard: Instant,
}

impl Client {
    /// `Connect` opens a socket and asks `server` to join a game on the map `map_name`.
    ///
    /// Example: `let mut client = Client::Connect("192.168.1.20:47800".parse()?, "lunar_surface")?;`
    #[allow(non_snake_case)]
    pub fn Connect(server: SocketAddr, map_name: &str) -> Result<Client> {
        let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        socket.send_to(&Message::New(Kind::Join).0, server)?;
        Ok(Client {
            socket,
            server,
            map_name: map_name.to_string(),
            id: None,
            settings: None,
            players: Vec::new(),
            game: GameState::default(),
            health_frac: 1.0,
            last_join: Instant::now(),
            last_heard: Instant::now(),
        })
    }

    /// `poll` reads every datagram that arrived, keeps the latest players (all but this one), and
    /// returns what happened; while not yet welcomed it asks to join again every `JOIN_RETRY_S`. It
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
            let bytes: &[u8] = &buffer[..length];
            let mut reader: Reader = Reader { bytes, position: 1 };
            match bytes.first().copied().and_then(Kind::FromByte) {
                Some(Kind::Welcome) if self.id.is_none() => {
                    if let Some(id) = reader.id()
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
                        self.settings = Some(GameSettings::FromJson(&bytes[reader.position..])?);
                        self.id = Some(id);
                        self.health_frac = 1.0;
                        events.push(NetEvent::Joined(id));
                    }
                }
                Some(Kind::Snapshot) => {
                    let count: usize = reader.take(1).map_or(0, |count| usize::from(count[0]));
                    if length != 2 + count * (BYTES_PER_PLAYER + 1) + BYTES_PER_GAME {
                        continue;
                    }
                    let players: Vec<RemotePlayer> =
                        (0..count).filter_map(|_| reader.player()).collect();
                    self.game = reader.game().expect("the length was checked");
                    let healths: &[u8] = reader.take(count).expect("the length was checked");
                    if let Some(i) = players.iter().position(|p| Some(p.id) == self.id) {
                        self.health_frac = f64::from(healths[i]) / f64::from(u8::MAX);
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

    /// `send_state` sends this player's eye frame, laser switch, whether the gun is the rifle, and
    /// the console they hold `E` on, once joined.
    ///
    /// Example: `client.send_state(player.frame, is_laser_on, false, None)?;`
    pub fn send_state(
        &mut self,
        frame: DMat4,
        is_laser_on: bool,
        is_rifle: bool,
        disabling_console: Option<u8>,
    ) -> Result<()> {
        if let Some(id) = self.id {
            let player: RemotePlayer = RemotePlayer {
                id,
                frame,
                is_laser_on,
                is_rifle,
                disabling_console,
            };
            self.send(&Message::New(Kind::State).player(&player))?;
        }
        Ok(())
    }

    /// `send_hit` tells the server this player's laser hit player `victim_id` once, with a weapon
    /// that eliminates in `hits_to_kill` such hits.
    ///
    /// Example: `client.send_hit(4, 12)?;`
    pub fn send_hit(&mut self, victim_id: u32, hits_to_kill: u8) -> Result<()> {
        if let Some(id) = self.id {
            let mut message: Message = Message::New(Kind::Hit).id(id).id(victim_id);
            message.0.push(hits_to_kill);
            self.send(&message)?;
        }
        Ok(())
    }

    /// `rejoin` asks to join again, after this player was eliminated.
    ///
    /// Example: `client.rejoin()?;`
    pub fn rejoin(&mut self) -> Result<()> {
        self.id = None;
        self.last_join = Instant::now();
        self.send(&Message::New(Kind::Join))
    }

    /// `keep_joining` resends the join request every `JOIN_RETRY_S` while not welcomed, in case
    /// it or the welcome was lost.
    ///
    /// Example: `client.keep_joining(is_waiting_to_rejoin)?;`
    pub fn keep_joining(&mut self) -> Result<()> {
        if self.id.is_none() && self.last_join.elapsed().as_secs_f64() >= JOIN_RETRY_S {
            self.last_join = Instant::now();
            self.send(&Message::New(Kind::Join))?;
        }
        Ok(())
    }

    /// `leave` tells the server this player is leaving.
    ///
    /// Example: `client.leave()?;`
    pub fn leave(&mut self) -> Result<()> {
        if let Some(id) = self.id.take() {
            self.send(&Message::New(Kind::Leave).id(id))?;
        }
        Ok(())
    }

    /// `send` sends a message to the server, dropping it if the socket is momentarily full.
    ///
    /// Example: `self.send(&Message::New(Kind::Join))?;`
    fn send(&self, message: &Message) -> Result<()> {
        match self.socket.send_to(&message.0, self.server) {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the message encoding and the server's handling of joins and hits.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Datagram length | complete, cut short | `players_survive_encoding`, `short_datagrams_read_as_nothing` |
    //! | Join | new player, repeated join, server full, rejoin after a hit (same id), a client of another map (fails) | `joins_get_ids_once_until_full`, `only_joined_players_can_hit`, `clients_of_another_map_fail` |
    //! | Hit | by a joined player, by a stranger, on a player who is gone | `only_joined_players_can_hit` |
    //! | Welcome size | the built-in settings with the longest map name | `the_welcome_fits_a_datagram` |
    //! | Hits to kill | 1 (eliminated at once), several (eliminated at the last) | `only_joined_players_can_hit`, `rifle_hits_add_up` |
    //! | Console held | the other team's (turned off after the settings' `hold_to_disable_s`), one's own team's (ignored) | `holding_consoles_turns_them_off_and_wins` |
    //! | Consoles off | some of a team's (no winner), all of a team's (the other team wins), then a new game (all back on) | `holding_consoles_turns_them_off_and_wins` |

    use super::*;

    const TEST_MAP_NAME: &str = "lunar_surface";

    /// `make_server` makes a server on a free local port, with no players.
    ///
    /// Example: `let mut server = make_server();`
    fn make_server() -> Server {
        Server {
            socket: UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free local port"),
            map_name: TEST_MAP_NAME.to_string(),
            settings: GameSettings::Builtin(),
            slots: Vec::new(),
            next_id: 0,
            known_ids: Vec::new(),
            console_teams: vec![0, 0, 1],
            game: GameState::default(),
        }
    }

    /// `hold_console` sends player `id`'s state from `from` holding `E` on `console`, then makes
    /// the server count it as held for its settings' `hold_to_disable_s` and update the consoles.
    ///
    /// Example: `hold_console(&mut server, 1, address(50_001), 2);`
    fn hold_console(server: &mut Server, id: u32, from: SocketAddr, console: u8) {
        let player: RemotePlayer = RemotePlayer {
            id,
            frame: DMat4::IDENTITY,
            is_laser_on: false,
            is_rifle: false,
            disabling_console: Some(console),
        };
        server
            .handle(&Message::New(Kind::State).player(&player).0, from)
            .expect("a state");
        let disable_s: f64 = server.settings.consoles.hold_to_disable_s;
        for slot in &mut server.slots {
            slot.disabling_since = slot
                .disabling_since
                .map(|since| since - Duration::from_secs_f64(disable_s));
        }
        server.update_consoles();
    }

    /// `address` gives a local address with a made-up port, standing for a client.
    ///
    /// Example: `let a = address(50000);`
    fn address(port: u16) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port)
    }

    #[test]
    fn players_survive_encoding() {
        let frame: DMat4 = DMat4::from_cols_array(&std::array::from_fn(|i| i as f64 * 0.25 - 1.0));
        let sent: RemotePlayer = RemotePlayer {
            id: 7,
            frame,
            is_laser_on: true,
            is_rifle: true,
            disabling_console: Some(3),
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
                player.disabling_console
            ),
            (7, frame, true, true, Some(3))
        );
        assert_eq!(reader.game(), Some(game));
    }

    #[test]
    fn short_datagrams_read_as_nothing() {
        let player: RemotePlayer = RemotePlayer {
            id: 7,
            frame: DMat4::IDENTITY,
            is_laser_on: false,
            is_rifle: false,
            disabling_console: None,
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
    fn joins_get_ids_once_until_full() {
        let mut server: Server = make_server();
        let join: Vec<u8> = Message::New(Kind::Join).0;
        server.handle(&join, address(50_000)).expect("a join");
        server
            .handle(&join, address(50_000))
            .expect("a repeated join");
        assert_eq!(server.slots.len(), 1);
        for port in 1..=MAX_PLAYER_COUNT as u16 {
            server
                .handle(&join, address(50_000 + port))
                .expect("a join");
        }
        assert_eq!(server.slots.len(), MAX_PLAYER_COUNT);
    }

    #[test]
    fn only_joined_players_can_hit() {
        let mut server: Server = make_server();
        let join: Vec<u8> = Message::New(Kind::Join).0;
        server.handle(&join, address(50_001)).expect("a join");
        server.handle(&join, address(50_002)).expect("a join");
        let mut stranger_hit: Vec<u8> = Message::New(Kind::Hit).id(1).id(2).0;
        stranger_hit.push(1);
        server
            .handle(&stranger_hit, address(50_003))
            .expect("a stranger's hit");
        assert_eq!(server.slots.len(), 2);
        server
            .handle(&stranger_hit, address(50_001))
            .expect("a hit");
        assert_eq!(
            server
                .slots
                .iter()
                .map(|slot| slot.player.id)
                .collect::<Vec<u32>>(),
            vec![1]
        );
        server
            .handle(&stranger_hit, address(50_001))
            .expect("a hit on a player who is gone");
        assert_eq!(server.slots.len(), 1);
        server.handle(&join, address(50_002)).expect("a rejoin");
        assert_eq!(server.slots[1].player.id, 2);
    }

    #[test]
    fn clients_of_another_map_fail() {
        let mut server: Server = make_server();
        let server_address: SocketAddr = server.socket.local_addr().expect("an address");
        let mut client: Client = Client::Connect(server_address, "house_demo").expect("a socket");
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        let (length, from) = server.socket.recv_from(&mut buffer).expect("the join");
        server.handle(&buffer[..length], from).expect("a join");
        let deadline: Instant = Instant::now() + Duration::from_secs(1);
        let error: String = loop {
            assert!(Instant::now() < deadline, "the welcome never arrived");
            if let Err(error) = client.poll() {
                break error.to_string();
            }
        };
        assert!(
            error.contains(TEST_MAP_NAME) && error.contains("house_demo"),
            "{error}"
        );
    }

    #[test]
    fn the_welcome_fits_a_datagram() {
        let header_bytes: usize = 1 + BYTES_PER_ID + 1 + usize::from(u8::MAX);
        assert!(header_bytes + GameSettings::Builtin().to_json().len() <= MAX_DATAGRAM_BYTES);
    }

    #[test]
    fn rifle_hits_add_up() {
        const HITS_TO_KILL: u8 = 3;
        let mut server: Server = make_server();
        let join: Vec<u8> = Message::New(Kind::Join).0;
        server.handle(&join, address(50_001)).expect("a join");
        server.handle(&join, address(50_002)).expect("a join");
        let mut hit: Vec<u8> = Message::New(Kind::Hit).id(1).id(2).0;
        hit.push(HITS_TO_KILL);
        for _ in 1..HITS_TO_KILL {
            server.handle(&hit, address(50_001)).expect("a hit");
            assert_eq!(server.slots.len(), 2);
        }
        server.handle(&hit, address(50_001)).expect("the last hit");
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn holding_consoles_turns_them_off_and_wins() {
        let mut server: Server = make_server();
        let join: Vec<u8> = Message::New(Kind::Join).0;
        let (blue, red) = (address(50_002), address(50_001));
        server.handle(&join, red).expect("a join");
        server.handle(&join, blue).expect("a join");
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
}
