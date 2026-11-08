//! `net.rs` lets players on one local network share a game. A server (its own process, without a
//! window) keeps every player's eye frame, laser switch and the console they hold `E` on (with how
//! far along turning it off they are, so every window speeds up its pulse alike), sends
//! everyone a snapshot 60 times a second, removes a player another player's laser hit, turns off a
//! console held for the settings' `consoles.hold_to_disable_s`, ends the game when a team's consoles are all off, and
//! announces itself on the network so others can find it. It counts each player's kills and deaths
//! in the game and sends everyone the scores 4 times a second. A client joins, sends its own player
//! each frame and receives the others, the game's state and the scores.
//!
//! Interface: `DEFAULT_PORT`, `MAX_PLAYER_COUNT`, `MAX_CONSOLE_COUNT`,
//! `TEAM_NAMES`, `get_team`, `bind_server`, `serve`, `find_server`, `Client`, `RemotePlayer`,
//! `GameState`, `Score`, `NetEvent`.
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
//! - An elimination counts a death for the player hit and a kill for the shooter, unless they are on
//!   the same team. Scores go back to 0 when a new game starts.

use std::cell::Cell;
use std::error::Error;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use glam::{DMat4, DVec4};

use crate::settings::ServerSettings;
use crate::thrown_wormholes::{MouthPlace, PlayerWormholes, WormholeColor};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const DEFAULT_PORT: u16 = 47_800;
pub const MAX_PLAYER_COUNT: usize = 8;
pub const MAX_CONSOLE_COUNT: usize = 32;
pub const TEAM_NAMES: [&str; 2] = ["blue", "red"];
const NO_CONSOLE: u8 = u8::MAX;
const RIFLE_BIT: u8 = 1;
const TEAM_BIT: u8 = 2;
const ANY_TEAM: u8 = u8::MAX;
const NO_WINNER: u8 = u8::MAX;
const DAMAGE_TOLERANCE_FRAC: f64 = 1e-9;
const DISCOVERY_PORT: u16 = 47_801;
const SNAPSHOT_PERIOD_S: f64 = 1.0 / 60.0;
const SCORES_PERIOD_S: f64 = 0.25;
const BEACON_PERIOD_S: f64 = 0.5;
const JOIN_RETRY_S: f64 = 0.5;
const PLAYER_TIMEOUT_S: f64 = 3.0;
const BEACON_GATHER_S: f64 = 1.0;
const OTHER_VERSION_WINDOW_S: f64 = 1.0;
const SERVER_POLL_MS: u64 = 2;
const MAX_DATAGRAM_BYTES: usize = 2048;
const BEACON_MAGIC: &[u8; 8] = b"WORMHOLE";
const FLOATS_PER_FRAME: usize = 16;
const BYTES_PER_FLOAT: usize = 8;
const BYTES_PER_ID: usize = 4;
const BYTES_PER_PORT: usize = 2;
const BYTES_PER_F32: usize = 4;
const F32S_PER_PLACE: usize = 4;
const F32S_PER_POINT: usize = 4;
const BYTES_PER_WORMHOLES: usize = 1 + (2 * F32S_PER_PLACE + F32S_PER_POINT) * BYTES_PER_F32;
const PURPLE_DOWN_BIT: u8 = 0;
const GREEN_DOWN_BIT: u8 = 1;
const FLYING_BIT: u8 = 2;
const FLYING_GREEN_BIT: u8 = 3;
const BYTES_PER_PLAYER: usize =
    BYTES_PER_ID + FLOATS_PER_FRAME * BYTES_PER_FLOAT + 3 + BYTES_PER_WORMHOLES;
const BYTES_PER_GAME: usize = 2 * BYTES_PER_ID + 1;
const _: () =
    assert!(2 + MAX_PLAYER_COUNT * (BYTES_PER_PLAYER + 1) + BYTES_PER_GAME <= MAX_DATAGRAM_BYTES);

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
        ]
        .into_iter()
        .find(|kind| *kind as u8 == byte)
    }
}

/// `RemotePlayer` is another player as the last snapshot gave them: their eye frame (while their
/// laser is on, turned by their shot's noise, so it is the frame their laser fires from), whether
/// their laser is on and from which gun, their team (an index into `TEAM_NAMES`), the console they
/// hold `E` on, if any, how far along turning it off they are, from 0 to 1, as the server times it
/// (`disabling_frac`, in steps of 1/255; a client sends 0, and the server sets it), and their
/// thrown wormholes.
#[derive(Clone, Copy)]
pub struct RemotePlayer {
    pub id: u32,
    pub frame: DMat4,
    pub is_laser_on: bool,
    pub is_rifle: bool,
    pub team: u32,
    pub disabling_console: Option<u8>,
    pub disabling_frac: f64,
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

    /// `player` appends a player: id, eye frame, laser switch, the console they hold `E` on, how far
    /// along turning it off they are (a byte, 255 for done), and their wormholes.
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
                | (player.team as u8) << TEAM_BIT,
        );
        self.0.push(player.disabling_console.unwrap_or(NO_CONSOLE));
        self.0
            .push((player.disabling_frac.clamp(0.0, 1.0) * f64::from(u8::MAX)).round() as u8);
        self.wormholes(&player.wormholes)
    }

    /// `wormholes` appends a player's wormholes: which are down or flying (and the flying one's
    /// colour), each colour's place (u, v, h, yaw), and the flying one's point, as `f32`s (zeros
    /// for what isn't there).
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
        let disabling: u8 = self.take(1)?[0];
        Some(RemotePlayer {
            id,
            frame: DMat4::from_cols_array(&values),
            is_laser_on: laser & 1 == 1,
            is_rifle: laser >> RIFLE_BIT & 1 == 1,
            team: u32::from(laser >> TEAM_BIT & 1),
            disabling_console: (console != NO_CONSOLE).then_some(console),
            disabling_frac: f64::from(disabling) / f64::from(u8::MAX),
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

/// `Slot` is a joined player on the server, since when they have held `E` on their console, and
/// the damage they took since joining (1 eliminates them).
struct Slot {
    player: RemotePlayer,
    address: SocketAddr,
    last_heard: Instant,
    disabling_since: Option<Instant>,
    damage_frac: f64,
}

/// `Tally` is a player's score in this game on the server, their address, and since when they wait
/// to respawn after being eliminated, if they do.
struct Tally {
    score: Score,
    address: SocketAddr,
    eliminated_at: Option<Instant>,
}

/// `Server` is the server's state: its socket, its map's name, the joined players, the next id to
/// give, the id each address had, each console's team, the game, and the tally of every player who
/// joined this game.
struct Server {
    socket: UdpSocket,
    map_name: String,
    settings: ServerSettings,
    slots: Vec<Slot>,
    next_id: u32,
    known_ids: Vec<(SocketAddr, u32)>,
    console_teams: Vec<u32>,
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
                let requested: u8 = reader.take(1).map_or(ANY_TEAM, |bytes| bytes[0]);
                let existing: Option<(u32, u32)> = self
                    .slots
                    .iter()
                    .find(|slot| slot.address == address)
                    .map(|slot| (slot.player.id, slot.player.team));
                let (id, team) = match existing {
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
                        self.slots.push(Slot {
                            player: RemotePlayer {
                                id,
                                frame: DMat4::IDENTITY,
                                is_laser_on: false,
                                is_rifle: false,
                                team,
                                disabling_console: None,
                                disabling_frac: 0.0,
                                wormholes: PlayerWormholes::default(),
                            },
                            address,
                            last_heard: Instant::now(),
                            disabling_since: None,
                            damage_frac: 0.0,
                        });
                        self.start_tally(id, team, address);
                        println!(
                            "player {id} joined the {} team (their window's address: {address}; players join this server's port)",
                            TEAM_NAMES[team as usize]
                        );
                        (id, team)
                    }
                    None => {
                        self.socket.send_to(&Message::New(Kind::Full).0, address)?;
                        return Ok(());
                    }
                };
                let mut welcome: Message = Message::New(Kind::Welcome).id(id);
                welcome.0.push(team as u8);
                welcome.0.push(self.map_name.len() as u8);
                welcome.0.extend_from_slice(self.map_name.as_bytes());
                welcome.0.extend_from_slice(&self.settings.to_json());
                self.socket.send_to(&welcome.0, address)?;
            }
            Some(Kind::LobbyQuery) => {
                let mut lobby: Message = Message::New(Kind::Lobby);
                lobby.0.extend(self.count_teams().map(|count| count as u8));
                self.socket.send_to(&lobby.0, address)?;
            }
            Some(Kind::State) => {
                if let Some(mut player) = reader.player()
                    && let Some(slot) = self
                        .slots
                        .iter()
                        .position(|slot| slot.address == address && slot.player.id == player.id)
                {
                    player.team = self.slots[slot].player.team;
                    player.disabling_console = player
                        .disabling_console
                        .filter(|&console| self.can_disable(player.team, console));
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
                let shooter_team: Option<u32> = self
                    .slots
                    .iter()
                    .find(|slot| slot.address == address && slot.player.id == shooter_id)
                    .map(|slot| slot.player.team);
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
                self.slots[victim].damage_frac += 1.0 / f64::from(hits_to_kill);
                if self.slots[victim].damage_frac < 1.0 - DAMAGE_TOLERANCE_FRAC {
                    return Ok(());
                }
                let victim_slot: Slot = self.slots.remove(victim);
                let victim_address: SocketAddr = victim_slot.address;
                let is_enemy: bool = shooter_team != Some(victim_slot.player.team);
                self.count_elimination(victim_id, shooter_id, is_enemy);
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

    /// `count_elimination` counts a death for player `victim_id`, who now waits to respawn, and a
    /// kill for player `shooter_id` if `is_enemy` (they are on different teams).
    ///
    /// Example: `server.count_elimination(2, 1, true);`
    fn count_elimination(&mut self, victim_id: u32, shooter_id: u32, is_enemy: bool) {
        let victim: &mut Tally = self.get_tally_mut(victim_id);
        victim.score.deaths += 1;
        victim.eliminated_at = Some(Instant::now());
        if is_enemy {
            self.get_tally_mut(shooter_id).score.kills += 1;
        }
    }

    /// `is_waiting_to_respawn` says whether `tally`'s player was eliminated and may still rejoin:
    /// less than the settings' `respawn.delay_s` and `PLAYER_TIMEOUT_S` ago.
    ///
    /// Example: `if server.is_waiting_to_respawn(tally) { ... }`
    fn is_waiting_to_respawn(&self, tally: &Tally) -> bool {
        let wait_s: f64 = self.settings.respawn.delay_s + PLAYER_TIMEOUT_S;
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
            self.socket.send_to(&message.0, address)?;
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

    /// `can_disable` says whether a player of `team` may turn off `console`: an existing console of
    /// the other team, still on, in a game nobody has won yet.
    ///
    /// Example: `if server.can_disable(1, 7) { ... }`
    fn can_disable(&self, team: u32, console: u8) -> bool {
        let index: usize = usize::from(console);
        self.game.winner.is_none()
            && index < self.console_teams.len()
            && self.console_teams[index] != team
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

    /// `start_new_game` turns every console back on, clears the winner and sets every player's kills
    /// and deaths back to 0; clients see the game's number change and go back to their spawn.
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
    /// health (a byte each, 255 full).
    ///
    /// Example: `server.send_snapshot()?;`
    fn send_snapshot(&self) -> Result<()> {
        let mut message: Message = Message::New(Kind::Snapshot);
        message.0.push(self.slots.len() as u8);
        for slot in &self.slots {
            message = message.player(&RemotePlayer {
                disabling_frac: self.get_disabling_frac(slot),
                ..slot.player
            });
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
/// players, sends the scores every `SCORES_PERIOD_S`, removes players who are hit or silent for
/// `PLAYER_TIMEOUT_S`, turns off
/// consoles (`console_teams` gives each one's team) and ends the game when a team's are all off,
/// starts a new game after that when `should_start_new_game` is set (clearing it), and broadcasts
/// a beacon on the local network every `BEACON_PERIOD_S` so `find_server` can find it.
///
/// Example: `net::serve(net::bind_server(net::DEFAULT_PORT)?, &should_stop, "lunar_surface".to_string(), ServerSettings::Builtin(), vec![0, 1], &should_start_new_game)?;`
pub fn serve(
    socket: UdpSocket,
    should_stop: &AtomicBool,
    map_name: String,
    settings: ServerSettings,
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
        tallies: Vec::new(),
    };
    let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
    let (mut last_snapshot, mut last_beacon) = (Instant::now(), None::<Instant>);
    let mut last_scores: Instant = Instant::now();
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
        if last_scores.elapsed().as_secs_f64() >= SCORES_PERIOD_S {
            server.send_scores()?;
            last_scores = Instant::now();
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
            "net: no server announced itself within {} s; start one with --host, or join with --join <address>",
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

/// `Client` is this player's connection: its map's name, its id and the server's game settings
/// once welcomed, the id of the latest welcome (kept while waiting to respawn), the other players,
/// the game, this player's health (1 full, from the server), and the latest scores of every player
/// in the game (`None` until the server sends them).
pub struct Client {
    socket: UdpSocket,
    server: SocketAddr,
    map_name: String,
    pub id: Option<u32>,
    pub welcomed_id: Option<u32>,
    pub team: Option<u32>,
    pub settings: Option<ServerSettings>,
    pub players: Vec<RemotePlayer>,
    pub game: GameState,
    pub health_frac: f64,
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
    /// `Connect` opens a socket to `server`'s game on the map `map_name`, and asks how many players
    /// each team has; the player is in the lobby until `join`.
    ///
    /// Example: `let mut client = Client::Connect("192.168.1.20:47800".parse()?, "lunar_surface")?;`
    #[allow(non_snake_case)]
    pub fn Connect(server: SocketAddr, map_name: &str) -> Result<Client> {
        let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        socket.send_to(&Message::New(Kind::LobbyQuery).0, server)?;
        Ok(Client {
            socket,
            server,
            map_name: map_name.to_string(),
            id: None,
            welcomed_id: None,
            team: None,
            settings: None,
            players: Vec::new(),
            game: GameState::default(),
            health_frac: 1.0,
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
                Some(Kind::Welcome) if self.id.is_none() => {
                    if let Some(id) = reader.id()
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

    /// `send_state` sends this player's eye frame, laser switch, whether the gun is the rifle, the
    /// console they hold `E` on and their wormholes, once joined. The server times the hold itself,
    /// so the progress sent is 0.
    ///
    /// Example: `client.send_state(player.frame, is_laser_on, false, None, PlayerWormholes::default())?;`
    pub fn send_state(
        &mut self,
        frame: DMat4,
        is_laser_on: bool,
        is_rifle: bool,
        disabling_console: Option<u8>,
        wormholes: PlayerWormholes,
    ) -> Result<()> {
        if let Some(id) = self.id {
            let player: RemotePlayer = RemotePlayer {
                id,
                frame,
                is_laser_on,
                is_rifle,
                team: self.team.unwrap_or(0),
                disabling_console,
                disabling_frac: 0.0,
                wormholes,
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
        self.id = None;
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
                None => self.send(&Message::New(Kind::LobbyQuery))?,
            }
        }
        Ok(())
    }

    /// `send_join` sends the join request with the team asked for.
    ///
    /// Example: `self.send_join()?;`
    fn send_join(&self) -> Result<()> {
        let mut message: Message = Message::New(Kind::Join);
        message.0.extend(self.join_request);
        self.send(&message)
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
    //! | Datagram length | complete (progress kept to a 255th), cut short | `players_survive_encoding`, `short_datagrams_read_as_nothing` |
    //! | Player's wormholes | none, one down with a green throw in the air, both down | `players_survive_encoding` |
    //! | Join | new player, repeated join, server full, rejoin after a hit (same id), a client of another map (fails) | `joins_get_ids_once_until_full`, `only_joined_players_can_hit`, `clients_of_another_map_fail` |
    //! | Servers announcing themselves | one, several with one on the default port (chosen), several without (the first) | `the_default_port_wins_among_servers` |
    //! | A server that can't take this player | full (says so), of another version (snapshots that don't fit) | `full_games_and_other_versions_are_noticed` |
    //! | Team asked for | blue or red (given), any or none (the smaller team), a different one on a repeated join (kept) | `players_join_the_team_they_choose_or_the_smaller` |
    //! | Hit | by a joined player, by a stranger, on a player who is gone | `only_joined_players_can_hit` |
    //! | Welcome size | the built-in settings with the longest map name | `the_welcome_fits_a_datagram` |
    //! | Hits to kill | 1 (eliminated at once), several (eliminated at the last) | `only_joined_players_can_hit`, `rifle_hits_add_up` |
    //! | Console held | the other team's (turned off after the settings' `hold_to_disable_s`), one's own team's (ignored) | `holding_consoles_turns_them_off_and_wins` |
    //! | Progress of turning off a console, in a snapshot | none held (0), part way (the seconds held over `hold_to_disable_s`), past the hold time (1) | `snapshots_carry_how_far_consoles_are_turned_off` |
    //! | Consoles off | some of a team's (no winner), all of a team's (the other team wins), then a new game (all back on) | `holding_consoles_turns_them_off_and_wins` |
    //! | Scores datagram | complete (several players, none), cut short, too long, an unknown team | `scores_survive_encoding` |
    //! | Who a hit eliminates | nobody (a hit short of the limit), the other team's player (a kill and a death), a teammate (a death only) | `eliminations_count_kills_and_deaths` |
    //! | Player in the scores | joined, waiting to respawn, rejoined (tally kept), waiting too long (left out) | `eliminations_count_kills_and_deaths` |
    //! | Game | the same, a new one (scores back to 0) | `eliminations_count_kills_and_deaths` |
    //! | Scores sent | to a joined client (read with its id from the welcome) | `clients_receive_scores` |

    use super::*;

    const TEST_MAP_NAME: &str = "lunar_surface";
    const LOBBY_QUERY_AND_JOIN_COUNT: usize = 2;
    const BLUE: u8 = 0;
    const RED: u8 = 1;
    const SENT_DISABLING_FRAC: f64 = 0.37;
    const DISABLING_FRAC_STEP: f64 = 1.0 / u8::MAX as f64;
    const HALF: f64 = 0.5;
    const TWICE: f64 = 2.0;

    /// `join_team` makes a join request for `team` (or `ANY_TEAM`).
    ///
    /// Example: `server.handle(&join_team(RED), address(50_001))?;`
    fn join_team(team: u8) -> Vec<u8> {
        let mut message: Message = Message::New(Kind::Join);
        message.0.push(team);
        message.0
    }

    /// `make_server` makes a server on a free local port, with no players.
    ///
    /// Example: `let mut server = make_server();`
    fn make_server() -> Server {
        Server {
            socket: UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free local port"),
            map_name: TEST_MAP_NAME.to_string(),
            settings: ServerSettings::Builtin(),
            slots: Vec::new(),
            next_id: 0,
            known_ids: Vec::new(),
            console_teams: vec![0, 0, 1],
            game: GameState::default(),
            tallies: Vec::new(),
        }
    }

    /// `hit` makes player `shooter_id`'s report of one hit on player `victim_id` with a weapon that
    /// eliminates in `hits_to_kill` hits.
    ///
    /// Example: `server.handle(&hit(1, 2, 1), address(50_001))?;`
    fn hit(shooter_id: u32, victim_id: u32, hits_to_kill: u8) -> Vec<u8> {
        let mut message: Message = Message::New(Kind::Hit).id(shooter_id).id(victim_id);
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
            team: 0,
            disabling_console: Some(console),
            disabling_frac: 0.0,
            wormholes: PlayerWormholes::default(),
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

    /// `connect_client` connects a client to `server` and joins it on `team`, letting the server
    /// handle its lobby query and join.
    ///
    /// Example: `let client = connect_client(&mut server, RED);`
    fn connect_client(server: &mut Server, team: u8) -> Client {
        let server_address: SocketAddr = server.socket.local_addr().expect("an address");
        let mut client: Client = Client::Connect(server_address, TEST_MAP_NAME).expect("a socket");
        client.join(Some(u32::from(team))).expect("a join request");
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        for _ in 0..LOBBY_QUERY_AND_JOIN_COUNT {
            let (length, from) = server.socket.recv_from(&mut buffer).expect("a datagram");
            server
                .handle(&buffer[..length], from)
                .expect("a lobby query or join");
        }
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
        let join: Vec<u8> = Message::New(Kind::Join).0;
        for port in 0..MAX_PLAYER_COUNT as u16 {
            server
                .handle(&join, address(50_100 + port))
                .expect("a join");
        }
        let mut client: Client = connect_client(&mut server, RED);
        poll_until(&mut client, Client::is_full);
        assert!(client.is_full() && client.is_waiting_for_welcome());
        assert!(!client.is_other_version());
        let client_address: SocketAddr =
            address(client.socket.local_addr().expect("an address").port());
        let short_snapshot: [u8; 3] = [Kind::Snapshot as u8, 1, 0];
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
            },
            PlayerWormholes {
                purple: Some(place(12.0)),
                green: Some(place(-30.0)),
                flying: None,
            },
        ] {
            let sent: RemotePlayer = RemotePlayer {
                id: 7,
                frame,
                is_laser_on: true,
                is_rifle: true,
                team: 0,
                disabling_console: Some(3),
                disabling_frac: SENT_DISABLING_FRAC,
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
        client.join(None).expect("a join request");
        let mut buffer: [u8; MAX_DATAGRAM_BYTES] = [0; MAX_DATAGRAM_BYTES];
        for _ in 0..LOBBY_QUERY_AND_JOIN_COUNT {
            let (length, from) = server.socket.recv_from(&mut buffer).expect("a datagram");
            server
                .handle(&buffer[..length], from)
                .expect("a lobby query or join");
        }
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
        assert!(header_bytes + ServerSettings::Builtin().to_json().len() <= MAX_DATAGRAM_BYTES);
    }

    #[test]
    fn players_join_the_team_they_choose_or_the_smaller() {
        let mut server: Server = make_server();
        server
            .handle(&join_team(RED), address(50_001))
            .expect("a join");
        server
            .handle(&join_team(RED), address(50_002))
            .expect("a join");
        server
            .handle(&join_team(ANY_TEAM), address(50_003))
            .expect("a join");
        server
            .handle(&Message::New(Kind::Join).0, address(50_004))
            .expect("a join");
        let teams: Vec<u32> = server.slots.iter().map(|slot| slot.player.team).collect();
        assert_eq!(teams, vec![1, 1, 0, 0]);
        assert_eq!(server.count_teams(), [2, 2]);
        server
            .handle(&join_team(BLUE), address(50_001))
            .expect("a repeated join");
        assert_eq!(server.slots[0].player.team, 1);
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
        let (blue, red) = (address(50_002), address(50_001));
        server.handle(&join_team(RED), red).expect("a join");
        server.handle(&join_team(BLUE), blue).expect("a join");
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
    fn eliminations_count_kills_and_deaths() {
        const RIFLE_HITS_TO_KILL: u8 = 2;
        let mut server: Server = make_server();
        let (red_one, blue_two, red_three) = (address(50_001), address(50_002), address(50_003));
        server.handle(&join_team(RED), red_one).expect("a join");
        server.handle(&join_team(BLUE), blue_two).expect("a join");
        server.handle(&join_team(RED), red_three).expect("a join");
        server
            .handle(&hit(1, 2, RIFLE_HITS_TO_KILL), red_one)
            .expect("a hit short of the limit");
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 0, 0), (2, 0, 0), (3, 0, 0)]
        );
        server
            .handle(&hit(1, 2, RIFLE_HITS_TO_KILL), red_one)
            .expect("the last hit");
        assert_eq!(server.slots.len(), 2);
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 1, 0), (2, 0, 1), (3, 0, 0)]
        );
        server
            .handle(&hit(1, 3, 1), red_one)
            .expect("a hit on a teammate");
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 1, 0), (2, 0, 1), (3, 0, 1)]
        );
        server.handle(&join_team(BLUE), blue_two).expect("a rejoin");
        let wait_s: f64 = server.settings.respawn.delay_s + PLAYER_TIMEOUT_S;
        server.get_tally_mut(3).eliminated_at =
            Some(Instant::now() - Duration::from_secs_f64(wait_s));
        assert_eq!(get_kills_and_deaths(&server), vec![(1, 1, 0), (2, 0, 1)]);
        server.start_new_game();
        assert_eq!(get_kills_and_deaths(&server), vec![(1, 0, 0), (2, 0, 0)]);
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
        server.handle(&join_team(RED), holder).expect("a join");
        server.handle(&join_team(RED), idle).expect("a join");
        let holding: RemotePlayer = RemotePlayer {
            id: 2,
            frame: DMat4::IDENTITY,
            is_laser_on: false,
            is_rifle: false,
            team: u32::from(RED),
            disabling_console: Some(0),
            disabling_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        server
            .handle(&Message::New(Kind::State).player(&holding).0, holder)
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
}
