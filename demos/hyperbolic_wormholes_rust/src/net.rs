//! `net.rs` lets players on one local network share a game. A server (inside the host's game, or
//! on its own) keeps every player's eye frame and laser switch, sends everyone a snapshot 60 times
//! a second, removes a player another player's laser hit, and announces itself on the network so
//! others can find it. A client joins, sends its own player each frame and receives the others.
//!
//! Interface: `DEFAULT_PORT`, `MAX_PLAYER_COUNT`, `bind_server`, `serve`, `find_server`, `Client`,
//! `RemotePlayer`, `NetEvent`.
//!
//! Example: `let mut client = Client::Connect(net::find_server(Duration::from_secs(5))?)?;`
//!
//! Must know:
//! - Messages are single UDP datagrams, little-endian, starting with a `Kind` byte. A lost state
//!   or snapshot is replaced by the next one; joins are resent until welcomed. Nothing is
//!   encrypted or authenticated: it is for a trusted local network.
//! - Each client decides when its own laser hits someone (it traces the laser anyway), and the
//!   server takes its word.

use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use glam::DMat4;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const DEFAULT_PORT: u16 = 47_800;
pub const MAX_PLAYER_COUNT: usize = 8;
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
const BYTES_PER_PLAYER: usize = BYTES_PER_ID + FLOATS_PER_FRAME * BYTES_PER_FLOAT + 1;

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

/// `RemotePlayer` is another player as the last snapshot gave them.
#[derive(Clone, Copy)]
pub struct RemotePlayer {
    pub id: u32,
    pub frame: DMat4,
    pub is_laser_on: bool,
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

    /// `player` appends a player: id, eye frame and laser switch.
    ///
    /// Example: `let message = Message::New(Kind::State).player(3, frame, true);`
    fn player(mut self, id: u32, frame: DMat4, is_laser_on: bool) -> Message {
        self = self.id(id);
        for value in frame.to_cols_array() {
            self.0.extend_from_slice(&value.to_le_bytes());
        }
        self.0.push(u8::from(is_laser_on));
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
        let is_laser_on: bool = self.take(1)?[0] != 0;
        Some(RemotePlayer {
            id,
            frame: DMat4::from_cols_array(&values),
            is_laser_on,
        })
    }
}

/// `Slot` is a joined player on the server.
struct Slot {
    player: RemotePlayer,
    address: SocketAddr,
    last_heard: Instant,
}

/// `Server` is the server's state: its socket, the joined players and the next id to give.
struct Server {
    socket: UdpSocket,
    slots: Vec<Slot>,
    next_id: u32,
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
                        self.next_id += 1;
                        self.slots.push(Slot {
                            player: RemotePlayer {
                                id: self.next_id,
                                frame: DMat4::IDENTITY,
                                is_laser_on: false,
                            },
                            address,
                            last_heard: Instant::now(),
                        });
                        println!("player {} joined from {address}", self.next_id);
                        self.next_id
                    }
                    None => return Ok(()),
                };
                self.socket
                    .send_to(&Message::New(Kind::Welcome).id(id).0, address)?;
            }
            Some(Kind::State) => {
                if let Some(player) = reader.player()
                    && let Some(slot) = self
                        .slots
                        .iter_mut()
                        .find(|slot| slot.address == address && slot.player.id == player.id)
                {
                    slot.player = player;
                    slot.last_heard = Instant::now();
                }
            }
            Some(Kind::Hit) => {
                let (Some(shooter_id), Some(victim_id)) = (reader.id(), reader.id()) else {
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
                if !is_shooter_here {
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

    /// `send_snapshot` sends every player the list of all players.
    ///
    /// Example: `server.send_snapshot()?;`
    fn send_snapshot(&self) -> Result<()> {
        let mut message: Message = Message::New(Kind::Snapshot);
        message.0.push(self.slots.len() as u8);
        for slot in &self.slots {
            message = message.player(slot.player.id, slot.player.frame, slot.player.is_laser_on);
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

/// `serve` runs a server on `socket` (from `bind_server`) until `should_stop` is set: it answers
/// joins, relays players, removes players who are hit or silent for `PLAYER_TIMEOUT_S`, and
/// broadcasts a beacon on the local network every `BEACON_PERIOD_S` so `find_server` can find it.
///
/// Example: `net::serve(net::bind_server(net::DEFAULT_PORT)?, &should_stop)?;`
pub fn serve(socket: UdpSocket, should_stop: &AtomicBool) -> Result<()> {
    let port: u16 = socket.local_addr()?.port();
    let beacon_socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    beacon_socket.set_broadcast(true)?;
    let mut beacon: Vec<u8> = vec![Kind::Beacon as u8];
    beacon.extend_from_slice(BEACON_MAGIC);
    beacon.extend_from_slice(&port.to_le_bytes());
    let mut server: Server = Server {
        socket,
        slots: Vec::new(),
        next_id: 0,
    };
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    let (mut last_snapshot, mut last_beacon) = (Instant::now(), None::<Instant>);
    let mut has_warned_about_beacon: bool = false;
    eprintln!("Serving on UDP port {port}; players on this network can join with --join auto");
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

/// `Client` is this player's connection: its id once welcomed, and the other players.
pub struct Client {
    socket: UdpSocket,
    server: SocketAddr,
    pub id: Option<u32>,
    pub players: Vec<RemotePlayer>,
    last_join: Instant,
    pub last_heard: Instant,
}

impl Client {
    /// `Connect` opens a socket and asks `server` to join.
    ///
    /// Example: `let mut client = Client::Connect("192.168.1.20:47800".parse()?)?;`
    #[allow(non_snake_case)]
    pub fn Connect(server: SocketAddr) -> Result<Client> {
        let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        socket.send_to(&Message::New(Kind::Join).0, server)?;
        Ok(Client {
            socket,
            server,
            id: None,
            players: Vec::new(),
            last_join: Instant::now(),
            last_heard: Instant::now(),
        })
    }

    /// `poll` reads every datagram that arrived, keeps the latest players (all but this one), and
    /// returns what happened; while not yet welcomed it asks to join again every `JOIN_RETRY_S`.
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
                    if let Some(id) = reader.id() {
                        self.id = Some(id);
                        events.push(NetEvent::Joined(id));
                    }
                }
                Some(Kind::Snapshot) => {
                    let count: usize = reader.take(1).map_or(0, |count| usize::from(count[0]));
                    if length != 2 + count * BYTES_PER_PLAYER {
                        continue;
                    }
                    self.players = (0..count)
                        .filter_map(|_| reader.player())
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

    /// `send_state` sends this player's eye frame and laser switch, once joined.
    ///
    /// Example: `client.send_state(player.frame, is_laser_on)?;`
    pub fn send_state(&mut self, frame: DMat4, is_laser_on: bool) -> Result<()> {
        if let Some(id) = self.id {
            self.send(&Message::New(Kind::State).player(id, frame, is_laser_on))?;
        }
        Ok(())
    }

    /// `send_hit` tells the server this player's laser hit player `victim_id`.
    ///
    /// Example: `client.send_hit(4)?;`
    pub fn send_hit(&mut self, victim_id: u32) -> Result<()> {
        if let Some(id) = self.id {
            self.send(&Message::New(Kind::Hit).id(id).id(victim_id))?;
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
    //! | Join | new player, repeated join, server full | `joins_get_ids_once_until_full` |
    //! | Hit | by a joined player, by a stranger, on a player who is gone | `only_joined_players_can_hit` |

    use super::*;

    /// `make_server` makes a server on a free local port, with no players.
    ///
    /// Example: `let mut server = make_server();`
    fn make_server() -> Server {
        Server {
            socket: UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free local port"),
            slots: Vec::new(),
            next_id: 0,
        }
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
        let message: Message = Message::New(Kind::State).player(7, frame, true);
        let player: RemotePlayer = Reader {
            bytes: &message.0,
            position: 1,
        }
        .player()
        .expect("a complete player");
        assert_eq!(
            (player.id, player.frame, player.is_laser_on),
            (7, frame, true)
        );
    }

    #[test]
    fn short_datagrams_read_as_nothing() {
        let message: Message = Message::New(Kind::State).player(7, DMat4::IDENTITY, false);
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
        let stranger_hit: Vec<u8> = Message::New(Kind::Hit).id(1).id(2).0;
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
    }
}
