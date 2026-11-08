//! `game.rs` is a game server, written sans-IO: `Game::handle` reacts to a datagram and `Game::tick`
//! moves the game on, each given the time and giving back the datagrams to send, so a UDP loop or a
//! test drives it without sockets.
//!
//! Interface: `Game`, `Game::New`, `Game::handle`, `Game::tick`, `Game::startNextGame`.
//!
//! Example: `for (to, datagram) in game.tick(now_s) { socket.send_to(&datagram, to)?; }`
//!
//! Must know:
//! - It is the server half of the demo's `net.rs` with its socket and clock taken out (step A5b),
//!   so it answers as the demo's server does, and logs the same lines on standard output.
//! - `now_s` is seconds from 0 on one clock that never goes back (`udp_loop::serve`'s, from its start).

use std::net::SocketAddr;

use glam::DMat4;
use warped_core::net::{
    ANY_TEAM, BYTES_PER_COOKIE, BYTES_PER_LOBBY, BYTES_PER_PROOF, BYTES_PER_VERSION, GameState,
    Kind, MAX_CONSOLE_COUNT, MAX_PLAYER_COUNT, Message, PROTOCOL_VERSION, REFUSED_FOR_PASSWORD,
    REFUSED_FOR_VERSION, Reader, RemotePlayer, Score, TEAM_NAMES, is_proof_right, make_proof,
    make_random_bytes,
};
use warped_core::settings::ServerSettings;
use warped_core::thrown_wormholes::PlayerWormholes;

const DAMAGE_TOLERANCE_FRAC: f64 = 1e-9;
const SNAPSHOT_PERIOD_S: f64 = 1.0 / 60.0;
const SCORES_PERIOD_S: f64 = 0.25;
const PLAYER_TIMEOUT_S: f64 = 3.0;
const BYTES_PER_SECRET: usize = 32;

/// `Slot` is a joined player on the server, the token their welcome gave them, since when they have held `E` on their console, the
/// damage they took since joining (1 eliminates them), and the damage enemy shots did to their
/// wormholes since they last had none down (1 destroys them).
struct Slot {
    player: RemotePlayer,
    address: SocketAddr,
    token: u64,
    last_heard_s: f64,
    disabling_since_s: Option<f64>,
    damage_frac: f64,
    wormhole_damage_frac: f64,
}

/// `Tally` is a player's score in this game on the server, their address, and since when they wait
/// to respawn after being eliminated, if they do.
struct Tally {
    score: Score,
    address: SocketAddr,
    eliminated_at_s: Option<f64>,
}

/// `Game` is a game server's state: the datagrams it has yet to give back, the time, its map's
/// name, the joined players, the next id to give, the id each address had, each console's team, the
/// game, the tally of every player who joined this game, how long after a win the next game starts
/// by itself (if it does), when the game was won, and when it last sent a snapshot and the scores.
pub struct Game {
    outbox: Vec<(SocketAddr, Vec<u8>)>,
    now_s: f64,
    secret: [u8; BYTES_PER_SECRET],
    password: Vec<u8>,
    map_name: String,
    settings: ServerSettings,
    slots: Vec<Slot>,
    next_id: u32,
    known_ids: Vec<(SocketAddr, u32)>,
    console_teams: Vec<u32>,
    game: GameState,
    tallies: Vec<Tally>,
    new_game_after_win_s: Option<f64>,
    won_at_s: Option<f64>,
    last_snapshot_s: f64,
    last_scores_s: f64,
}

impl Game {
    /// `New` makes a game server for the map `map_name` with `settings`, whose consoles belong to
    /// `console_teams`, that admits only players who prove they know `game_password` (empty for
    /// anyone) and, with `new_game_after_win_s`, starts the next game that long after a team wins.
    /// Its clock starts at 0.
    ///
    /// Example: `let mut game = Game::New("lunar_surface".to_string(), ServerSettings::Builtin(), vec![0, 1], Vec::new(), Some(10.0));`
    #[allow(non_snake_case)]
    pub fn New(
        map_name: String,
        settings: ServerSettings,
        console_teams: Vec<u32>,
        game_password: Vec<u8>,
        new_game_after_win_s: Option<f64>,
    ) -> Game {
        assert!(
            console_teams.len() <= MAX_CONSOLE_COUNT,
            "Game::New: {} consoles, more than {MAX_CONSOLE_COUNT}",
            console_teams.len()
        );
        Game {
            outbox: Vec::new(),
            now_s: 0.0,
            secret: make_random_bytes(),
            password: game_password,
            map_name,
            settings,
            slots: Vec::new(),
            next_id: 0,
            known_ids: Vec::new(),
            console_teams,
            game: GameState::default(),
            tallies: Vec::new(),
            new_game_after_win_s,
            won_at_s: None,
            last_snapshot_s: 0.0,
            last_scores_s: 0.0,
        }
    }

    /// `handle` reacts to one `datagram` from `from` at `now_s`, and gives the datagrams to send
    /// back, each with its address.
    ///
    /// Example: `let replies = game.handle(&buffer[..length], from, now_s);`
    pub fn handle(
        &mut self,
        datagram: &[u8],
        from: SocketAddr,
        now_s: f64,
    ) -> Vec<(SocketAddr, Vec<u8>)> {
        self.advance_clock(now_s);
        self.react(datagram, from);
        std::mem::take(&mut self.outbox)
    }

    /// `tick` moves the game on to `now_s`: it removes players silent for `PLAYER_TIMEOUT_S`, turns
    /// off consoles held long enough, starts the next game `new_game_after_win_s` after a win, and
    /// gives a snapshot every `SNAPSHOT_PERIOD_S` and the scores every `SCORES_PERIOD_S` to send.
    ///
    /// Example: `for (to, datagram) in game.tick(now_s) { socket.send_to(&datagram, to)?; }`
    pub fn tick(&mut self, now_s: f64) -> Vec<(SocketAddr, Vec<u8>)> {
        self.advance_clock(now_s);
        self.slots.retain(|slot| {
            let is_alive: bool = now_s - slot.last_heard_s < PLAYER_TIMEOUT_S;
            if !is_alive {
                println!("player {} timed out", slot.player.id);
            }
            is_alive
        });
        self.update_consoles();
        self.won_at_s = self.game.winner.and(self.won_at_s.or(Some(now_s)));
        if let (Some(delay_s), Some(won_at_s)) = (self.new_game_after_win_s, self.won_at_s)
            && now_s - won_at_s >= delay_s
        {
            self.start_new_game();
        }
        if now_s - self.last_snapshot_s >= SNAPSHOT_PERIOD_S {
            self.send_snapshot();
            self.last_snapshot_s = now_s;
        }
        if now_s - self.last_scores_s >= SCORES_PERIOD_S {
            self.send_scores();
            self.last_scores_s = now_s;
        }
        std::mem::take(&mut self.outbox)
    }

    /// `startNextGame` starts a new game if a team has won, as `Enter` in the server's terminal
    /// asks, or says that the game goes on.
    ///
    /// Example: `game.startNextGame();`
    #[allow(non_snake_case)]
    pub fn startNextGame(&mut self) {
        match self.game.winner {
            Some(_) => self.start_new_game(),
            None => println!("No team has won yet, so the game goes on."),
        }
    }

    /// `advance_clock` sets the time to `now_s`, which may not be earlier than the time already set.
    ///
    /// Example: `self.advance_clock(now_s);`
    fn advance_clock(&mut self, now_s: f64) {
        assert!(
            now_s >= self.now_s,
            "Game: the time went back from {} s to {now_s} s",
            self.now_s
        );
        self.now_s = now_s;
    }

    /// `react` reacts to one datagram from `address`, queueing its replies.
    ///
    /// Example: `self.react(&buffer[..length], address);`
    fn react(&mut self, bytes: &[u8], address: SocketAddr) {
        let mut reader: Reader = Reader { bytes, position: 1 };
        match bytes.first().copied().and_then(Kind::FromByte) {
            Some(Kind::Join) => {
                let (Some(version), Some(cookie), Some(proof)) = (
                    reader.array::<BYTES_PER_VERSION>(),
                    reader.array::<BYTES_PER_COOKIE>(),
                    reader.array::<BYTES_PER_PROOF>(),
                ) else {
                    return;
                };
                if version != PROTOCOL_VERSION.to_le_bytes() {
                    self.refuse(REFUSED_FOR_VERSION, address);
                    return;
                }
                let expected_cookie: [u8; BYTES_PER_COOKIE] = self.make_cookie(address);
                if cookie != expected_cookie {
                    let mut challenge: Message = Message::New(Kind::Challenge);
                    challenge.0.extend_from_slice(&expected_cookie);
                    self.send(&challenge.0, address);
                    return;
                }
                if !is_proof_right(&self.password, &cookie, &proof) {
                    self.refuse(REFUSED_FOR_PASSWORD, address);
                    return;
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
                                wormholes: PlayerWormholes::default(),
                            },
                            address,
                            token,
                            last_heard_s: self.now_s,
                            disabling_since_s: None,
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
                        return;
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
                        .filter(|&console| self.can_disable(player.team, console));
                    let slot: &mut Slot = &mut self.slots[slot];
                    if player.disabling_console != slot.player.disabling_console {
                        slot.disabling_since_s = player.disabling_console.map(|_| self.now_s);
                    }
                    if player.wormholes.get_places().is_empty() {
                        slot.wormhole_damage_frac = 0.0;
                    }
                    slot.player = player;
                    slot.last_heard_s = self.now_s;
                }
            }
            Some(Kind::Hit) => {
                let (Some(token), Some(shooter_id), Some(victim_id), Some(&[hits_to_kill])) =
                    (reader.token(), reader.id(), reader.id(), reader.take(1))
                else {
                    return;
                };
                let shooter_team: Option<u32> = self
                    .find_sender(address, token, shooter_id)
                    .map(|slot| self.slots[slot].player.team);
                let Some(victim) = self
                    .slots
                    .iter()
                    .position(|slot| slot.player.id == victim_id)
                else {
                    return;
                };
                if shooter_team.is_none() || self.game.winner.is_some() || hits_to_kill == 0 {
                    return;
                }
                self.slots[victim].damage_frac += 1.0 / f64::from(hits_to_kill);
                if self.slots[victim].damage_frac < 1.0 - DAMAGE_TOLERANCE_FRAC {
                    return;
                }
                let victim_slot: Slot = self.slots.remove(victim);
                let victim_address: SocketAddr = victim_slot.address;
                let is_enemy: bool = shooter_team != Some(victim_slot.player.team);
                self.count_elimination(victim_id, shooter_id, is_enemy);
                println!("player {victim_id} was hit by player {shooter_id}");
                let message: Message = Message::New(Kind::Eliminated).id(victim_id).id(shooter_id);
                let targets = self.slots.iter().map(|slot| slot.address);
                let datagrams = targets
                    .chain([victim_address])
                    .map(|to| (to, message.0.clone()));
                self.outbox.extend(datagrams);
            }
            Some(Kind::WormholeHit) => {
                let (Some(token), Some(shooter_id), Some(owner_id), Some(&[hits_to_destroy])) =
                    (reader.token(), reader.id(), reader.id(), reader.take(1))
                else {
                    return;
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
    fn refuse(&mut self, reason: u8, address: SocketAddr) {
        let mut refused: Message = Message::New(Kind::Refused);
        refused.0.push(reason);
        refused.0.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        self.send(&refused.0, address);
    }

    /// `send` queues a datagram to `address`, for `handle` or `tick` to give back.
    ///
    /// Example: `server.send(&welcome.0, address);`
    fn send(&mut self, bytes: &[u8], address: SocketAddr) {
        self.outbox.push((address, bytes.to_vec()));
    }

    /// `start_tally` gives player `id` a tally of this game's kills and deaths if they have none, and
    /// marks them as playing on `team` rather than waiting to respawn.
    ///
    /// Example: `server.start_tally(3, 1, address);`
    fn start_tally(&mut self, id: u32, team: u32, address: SocketAddr) {
        match self.tallies.iter_mut().find(|tally| tally.score.id == id) {
            Some(tally) => (tally.score.team, tally.eliminated_at_s) = (team, None),
            None => self.tallies.push(Tally {
                score: Score {
                    id,
                    team,
                    kills: 0,
                    deaths: 0,
                },
                address,
                eliminated_at_s: None,
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
        let now_s: f64 = self.now_s;
        let victim: &mut Tally = self.get_tally_mut(victim_id);
        victim.score.deaths += 1;
        victim.eliminated_at_s = Some(now_s);
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
            .eliminated_at_s
            .is_some_and(|time_s| self.now_s - time_s < wait_s)
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
    /// Example: `server.send_scores();`
    fn send_scores(&mut self) {
        let message: Message = Message::New(Kind::Scores).scores(&self.get_scores());
        let waiting = self
            .tallies
            .iter()
            .filter(|tally| self.is_waiting_to_respawn(tally))
            .map(|tally| tally.address);
        let players = self.slots.iter().map(|slot| slot.address);
        for address in players.chain(waiting).collect::<Vec<SocketAddr>>() {
            self.send(&message.0, address);
        }
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
            if let (Some(console), Some(since_s)) =
                (slot.player.disabling_console, slot.disabling_since_s)
                && self.now_s - since_s >= self.settings.consoles.hold_to_disable_s
            {
                self.game.off_mask |= 1 << console;
                (slot.player.disabling_console, slot.disabling_since_s) = (None, None);
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
            (slot.player.disabling_console, slot.disabling_since_s) = (None, None);
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
        slot.disabling_since_s.map_or(0.0, |since_s| {
            ((self.now_s - since_s) / self.settings.consoles.hold_to_disable_s).min(1.0)
        })
    }

    /// `send_snapshot` sends every player the list of all players (each with how far along turning
    /// off a console they are, from `get_disabling_frac`), the game's state and each player's
    /// health (a byte each, 255 full).
    ///
    /// Example: `server.send_snapshot();`
    fn send_snapshot(&mut self) {
        let mut message: Message = Message::New(Kind::Snapshot);
        message.0.push(self.slots.len() as u8);
        for slot in &self.slots {
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
        for slot in &self.slots {
            let health_frac: f64 = (1.0 - slot.damage_frac).clamp(0.0, 1.0);
            message
                .0
                .push((health_frac * f64::from(u8::MAX)).round() as u8);
        }
        for address in self
            .slots
            .iter()
            .map(|slot| slot.address)
            .collect::<Vec<SocketAddr>>()
        {
            self.send(&message.0, address);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the server's handling of joins, hits, consoles and scores, and of its clock, fed
    //! datagrams and times directly, without sockets.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Shot at a player's wormholes | by an enemy (adds up to destroyed, then no further), by a teammate (ignored), with none down (ignored); the owner then has none down (reset) | `enemy_shots_destroy_wormholes` |
    //! | Join | new player, repeated join, server full, rejoin after a hit (same id) | `joins_get_ids_once_until_full`, `only_joined_players_can_hit` |
    //! | Join's version | this game's (goes on), another (refused, naming the server's) | `joins_get_ids_once_until_full`, `other_versions_and_wrong_passwords_are_refused` |
    //! | Join's challenge answer | none yet or another address's (challenged again), this address's (goes on) | `joins_need_the_challenge_answered` |
    //! | Join's proof of the password | right (welcomed), wrong (refused) | `joins_get_ids_once_until_full`, `other_versions_and_wrong_passwords_are_refused` |
    //! | Token on a player's message | their own (taken), none or another player's (ignored) | `messages_need_the_senders_token` |
    //! | Lobby query | as long as the answer (answered), shorter (ignored) | `short_lobby_queries_are_ignored` |
    //! | Team asked for | blue or red (given), any or none (the smaller team), a different one on a repeated join (kept) | `players_join_the_team_they_choose_or_the_smaller` |
    //! | Hit | by a joined player, by a stranger, on a player who is gone | `only_joined_players_can_hit` |
    //! | Hits to kill | 1 (eliminated at once), several (eliminated at the last) | `only_joined_players_can_hit`, `rifle_hits_add_up` |
    //! | Console held | the other team's (turned off after the settings' `hold_to_disable_s`), one's own team's (ignored) | `holding_consoles_turns_them_off_and_wins` |
    //! | Progress of turning off a console, in a snapshot | none held (0), part way (the seconds held over `hold_to_disable_s`), past the hold time (1) | `snapshots_carry_how_far_consoles_are_turned_off` |
    //! | Consoles off | some of a team's (no winner), all of a team's (the other team wins), then a new game (all back on) | `holding_consoles_turns_them_off_and_wins` |
    //! | Who a hit eliminates | nobody (a hit short of the limit), the other team's player (a kill and a death), a teammate (a death only) | `eliminations_count_kills_and_deaths` |
    //! | Player in the scores | joined, waiting to respawn, rejoined (tally kept), waiting too long (left out) | `eliminations_count_kills_and_deaths` |
    //! | Game | the same, a new one (scores back to 0) | `eliminations_count_kills_and_deaths` |
    //! | Time since a player was last heard | less than `PLAYER_TIMEOUT_S` (kept), `PLAYER_TIMEOUT_S` (removed) | `silent_players_time_out` |
    //! | Time since the last snapshot and scores | less than their period (nothing sent), a snapshot's period, both periods (both sent to every player) | `snapshots_and_scores_follow_their_periods` |
    //! | Next game after a win | by itself, before `new_game_after_win_s` (not yet) and at it (started); never by itself (`None`); asked for (`startNextGame`) with a winner (started) and without (the game goes on) | `the_next_game_starts_after_a_win` |
    //! | Time given | later or the same (taken), earlier (fails) | every test, `time_cannot_go_back` |

    use super::*;
    use warped_core::thrown_wormholes::MouthPlace;

    const TEST_MAP_NAME: &str = "lunar_surface";
    const TEST_PASSWORD: &[u8] = b"correct horse";
    const BLUE: u8 = 0;
    const RED: u8 = 1;
    const DISABLING_FRAC_STEP: f64 = 1.0 / u8::MAX as f64;
    const HALF: f64 = 0.5;
    const TWICE: f64 = 2.0;
    const NEW_GAME_AFTER_WIN_S: f64 = 10.0;

    /// `make_join` makes `from`'s join request for `team` (or `ANY_TEAM`) with `version`,
    /// answering `server`'s challenge with a proof of `password`.
    ///
    /// Example: `let message = make_join(&server, from, RED, PROTOCOL_VERSION, TEST_PASSWORD);`
    fn make_join(
        server: &Game,
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
    /// right password, at the server's time.
    ///
    /// Example: `join(&mut server, address(50_001), RED);`
    fn join(server: &mut Game, from: SocketAddr, team: u8) {
        let message: Vec<u8> = make_join(server, from, team, PROTOCOL_VERSION, TEST_PASSWORD);
        server.handle(&message, from, server.now_s);
    }

    /// `get_token` gives the token the welcome gave the player at `from`, or 0 for a stranger.
    ///
    /// Example: `let token = get_token(&server, address(50_001));`
    fn get_token(server: &Game, from: SocketAddr) -> u64 {
        server
            .slots
            .iter()
            .find(|slot| slot.address == from)
            .map_or(0, |slot| slot.token)
    }

    /// `make_server` makes a server with no players, admitting players who know `TEST_PASSWORD`,
    /// that starts the next game after a win only when asked.
    ///
    /// Example: `let mut server = make_server();`
    fn make_server() -> Game {
        Game::New(
            TEST_MAP_NAME.to_string(),
            ServerSettings::Builtin(),
            vec![0, 0, 1],
            TEST_PASSWORD.to_vec(),
            None,
        )
    }

    /// `make_hit` makes player `shooter_id`'s report, with `token`, of one hit on player
    /// `victim_id` with a weapon that eliminates in `hits_to_kill` hits.
    ///
    /// Example: `server.handle(&make_hit(token, 1, 2, 1), address(50_001), 0.0);`
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
    fn get_kills_and_deaths(server: &Game) -> Vec<(u32, u32, u32)> {
        let mut scores: Vec<(u32, u32, u32)> = server
            .get_scores()
            .iter()
            .map(|score| (score.id, score.kills, score.deaths))
            .collect();
        scores.sort();
        scores
    }

    /// `make_state` makes player `id`'s state, with the token the welcome gave the player at
    /// `from`, holding `E` on `console` if any.
    ///
    /// Example: `server.handle(&make_state(&server, 1, address(50_001), Some(2)), address(50_001), 1.0);`
    fn make_state(server: &Game, id: u32, from: SocketAddr, console: Option<u8>) -> Vec<u8> {
        let player: RemotePlayer = RemotePlayer {
            id,
            frame: DMat4::IDENTITY,
            is_laser_on: false,
            is_rifle: false,
            team: 0,
            disabling_console: console,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        Message::New(Kind::State)
            .token(get_token(server, from))
            .player(&player)
            .0
    }

    /// `hold_console` sends player `id`'s state from `from` holding `E` on `console`, then makes
    /// the server count it as held for its settings' `hold_to_disable_s` and update the consoles.
    ///
    /// Example: `hold_console(&mut server, 1, address(50_001), 2);`
    fn hold_console(server: &mut Game, id: u32, from: SocketAddr, console: u8) {
        let state: Vec<u8> = make_state(server, id, from, Some(console));
        server.handle(&state, from, server.now_s);
        let disable_s: f64 = server.settings.consoles.hold_to_disable_s;
        for slot in &mut server.slots {
            slot.disabling_since_s = slot.disabling_since_s.map(|since_s| since_s - disable_s);
        }
        server.update_consoles();
    }

    /// `address` gives a local address with a made-up port, standing for a client.
    ///
    /// Example: `let a = address(50000);`
    fn address(port: u16) -> SocketAddr {
        SocketAddr::new(std::net::Ipv4Addr::LOCALHOST.into(), port)
    }

    /// `get_kinds` gives the address and `Kind` byte of each datagram in `outgoing`.
    ///
    /// Example: `assert_eq!(get_kinds(&server.tick(1.0)), vec![(address(50_001), Kind::Snapshot as u8)]);`
    fn get_kinds(outgoing: &[(SocketAddr, Vec<u8>)]) -> Vec<(SocketAddr, u8)> {
        outgoing.iter().map(|(to, bytes)| (*to, bytes[0])).collect()
    }

    #[test]
    fn joins_get_ids_once_until_full() {
        let mut server: Game = make_server();
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
        let mut server: Game = make_server();
        join(&mut server, address(50_001), ANY_TEAM);
        join(&mut server, address(50_002), ANY_TEAM);
        let stranger_hit: Vec<u8> = make_hit(0, 1, 2, 1);
        server.handle(&stranger_hit, address(50_003), 0.0);
        assert_eq!(server.slots.len(), 2);
        let hit: Vec<u8> = make_hit(get_token(&server, address(50_001)), 1, 2, 1);
        server.handle(&hit, address(50_001), 0.0);
        assert_eq!(
            server
                .slots
                .iter()
                .map(|slot| slot.player.id)
                .collect::<Vec<u32>>(),
            vec![1]
        );
        server.handle(&hit, address(50_001), 0.0);
        assert_eq!(server.slots.len(), 1);
        join(&mut server, address(50_002), ANY_TEAM);
        assert_eq!(server.slots[1].player.id, 2);
    }

    #[test]
    fn players_join_the_team_they_choose_or_the_smaller() {
        let mut server: Game = make_server();
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
        server.handle(&without_team, address(50_004), 0.0);
        let teams: Vec<u32> = server.slots.iter().map(|slot| slot.player.team).collect();
        assert_eq!(teams, vec![1, 1, 0, 0]);
        assert_eq!(server.count_teams(), [2, 2]);
        join(&mut server, address(50_001), BLUE);
        assert_eq!(server.slots[0].player.team, 1);
    }

    #[test]
    fn rifle_hits_add_up() {
        const HITS_TO_KILL: u8 = 3;
        let mut server: Game = make_server();
        join(&mut server, address(50_001), ANY_TEAM);
        join(&mut server, address(50_002), ANY_TEAM);
        let hit: Vec<u8> = make_hit(get_token(&server, address(50_001)), 1, 2, HITS_TO_KILL);
        for _ in 1..HITS_TO_KILL {
            server.handle(&hit, address(50_001), 0.0);
            assert_eq!(server.slots.len(), 2);
        }
        server.handle(&hit, address(50_001), 0.0);
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn enemy_shots_destroy_wormholes() {
        let hits_to_destroy: u8 = ServerSettings::Builtin().wormholes.rifle_hits_to_destroy;
        let mut server: Game = make_server();
        for (team, port) in [(BLUE, 50_001), (RED, 50_002), (RED, 50_003)] {
            join(&mut server, address(port), team);
        }
        let shoot = |server: &mut Game, shooter_id: u32, port: u16| {
            let mut message: Message = Message::New(Kind::WormholeHit)
                .token(get_token(server, address(port)))
                .id(shooter_id)
                .id(2);
            message.0.push(hits_to_destroy);
            server.handle(&message.0, address(port), 0.0);
            server.slots[1].wormhole_damage_frac
        };
        let send_wormholes = |server: &mut Game, wormholes: PlayerWormholes| {
            let mut owner: RemotePlayer = server.slots[1].player;
            owner.wormholes = wormholes;
            let state: Vec<u8> = Message::New(Kind::State)
                .token(get_token(server, address(50_002)))
                .player(&owner)
                .0;
            server.handle(&state, address(50_002), 0.0);
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
    fn holding_consoles_turns_them_off_and_wins() {
        let mut server: Game = make_server();
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
    fn eliminations_count_kills_and_deaths() {
        const RIFLE_HITS_TO_KILL: u8 = 2;
        let mut server: Game = make_server();
        let (red_one, blue_two, red_three) = (address(50_001), address(50_002), address(50_003));
        join(&mut server, red_one, RED);
        join(&mut server, blue_two, BLUE);
        join(&mut server, red_three, RED);
        let hit: Vec<u8> = make_hit(get_token(&server, red_one), 1, 2, RIFLE_HITS_TO_KILL);
        server.handle(&hit, red_one, 0.0);
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 0, 0), (2, 0, 0), (3, 0, 0)]
        );
        server.handle(&hit, red_one, 0.0);
        assert_eq!(server.slots.len(), 2);
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 1, 0), (2, 0, 1), (3, 0, 0)]
        );
        server.handle(
            &make_hit(get_token(&server, red_one), 1, 3, 1),
            red_one,
            0.0,
        );
        assert_eq!(
            get_kills_and_deaths(&server),
            vec![(1, 1, 0), (2, 0, 1), (3, 0, 1)]
        );
        join(&mut server, blue_two, BLUE);
        let wait_s: f64 = server.settings.respawn.delay_s + PLAYER_TIMEOUT_S;
        server.get_tally_mut(3).eliminated_at_s = Some(server.now_s - wait_s);
        assert_eq!(get_kills_and_deaths(&server), vec![(1, 1, 0), (2, 0, 1)]);
        server.start_new_game();
        assert_eq!(get_kills_and_deaths(&server), vec![(1, 0, 0), (2, 0, 0)]);
    }

    #[test]
    fn snapshots_carry_how_far_consoles_are_turned_off() {
        let mut server: Game = make_server();
        let (watcher, holder, idle) = (address(50_000), address(50_001), address(50_002));
        join(&mut server, watcher, BLUE);
        join(&mut server, holder, RED);
        join(&mut server, idle, RED);
        server.handle(&make_state(&server, 2, holder, Some(0)), holder, 0.0);
        let hold_s: f64 = server.settings.consoles.hold_to_disable_s;
        server.slots[1].disabling_since_s = Some(server.now_s - HALF * hold_s);
        server.send_snapshot();
        let snapshot: Vec<u8> = server.outbox[0].1.clone();
        let mut reader: Reader = Reader {
            bytes: &snapshot,
            position: 2,
        };
        let fracs: Vec<(u32, Option<u8>, f64)> = (0..server.slots.len())
            .map(|_| reader.player().expect("a player"))
            .map(|p| (p.id, p.disabling_console, p.disabling_frac))
            .collect();
        assert_eq!((fracs[1].0, fracs[1].1), (2, Some(0)));
        assert!(
            (fracs[1].2 - HALF).abs() <= DISABLING_FRAC_STEP,
            "{fracs:?}"
        );
        assert_eq!(fracs[2], (3, None, 0.0));
        server.slots[1].disabling_since_s = Some(server.now_s - TWICE * hold_s);
        assert_eq!(server.get_disabling_frac(&server.slots[1]), 1.0);
    }

    #[test]
    fn joins_need_the_challenge_answered() {
        let mut server: Game = make_server();
        let from: SocketAddr = address(50_001);
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
            let replies: Vec<(SocketAddr, Vec<u8>)> = server.handle(&message, from, 0.0);
            let challenge: Vec<u8> = [
                vec![Kind::Challenge as u8],
                server.make_cookie(from).to_vec(),
            ]
            .concat();
            assert_eq!(replies, vec![(from, challenge)]);
            assert!(server.slots.is_empty());
        }
        join(&mut server, from, ANY_TEAM);
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn other_versions_and_wrong_passwords_are_refused() {
        let mut server: Game = make_server();
        let from: SocketAddr = address(50_001);
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
            let refusal: Vec<u8> = vec![Kind::Refused as u8, reason, version[0], version[1]];
            assert_eq!(server.handle(&message, from, 0.0), vec![(from, refusal)]);
            assert!(server.slots.is_empty());
        }
    }

    #[test]
    fn messages_need_the_senders_token() {
        let mut server: Game = make_server();
        let (shooter, victim) = (address(50_001), address(50_002));
        join(&mut server, shooter, ANY_TEAM);
        join(&mut server, victim, ANY_TEAM);
        for token in [0, get_token(&server, victim)] {
            server.handle(&make_hit(token, 1, 2, 1), shooter, 0.0);
            assert_eq!(server.slots.len(), 2);
        }
        server.handle(
            &make_hit(get_token(&server, shooter), 1, 2, 1),
            shooter,
            0.0,
        );
        assert_eq!(server.slots.len(), 1);
    }

    #[test]
    fn short_lobby_queries_are_ignored() {
        let mut server: Game = make_server();
        let from: SocketAddr = address(50_001);
        assert_eq!(server.handle(&[Kind::LobbyQuery as u8], from, 0.0), vec![]);
        let mut query: Vec<u8> = vec![Kind::LobbyQuery as u8];
        query.resize(BYTES_PER_LOBBY, 0);
        let lobby: Vec<u8> = vec![Kind::Lobby as u8, 0, 0];
        assert_eq!(server.handle(&query, from, 0.0), vec![(from, lobby)]);
    }

    #[test]
    fn silent_players_time_out() {
        let mut server: Game = make_server();
        let (talking, silent) = (address(50_001), address(50_002));
        join(&mut server, talking, ANY_TEAM);
        join(&mut server, silent, ANY_TEAM);
        server.handle(&make_state(&server, 1, talking, None), talking, HALF);
        server.tick(PLAYER_TIMEOUT_S);
        let ids: Vec<u32> = server.slots.iter().map(|slot| slot.player.id).collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn snapshots_and_scores_follow_their_periods() {
        let mut server: Game = make_server();
        let (one, two) = (address(50_001), address(50_002));
        join(&mut server, one, ANY_TEAM);
        join(&mut server, two, ANY_TEAM);
        assert_eq!(get_kinds(&server.tick(HALF * SNAPSHOT_PERIOD_S)), vec![]);
        let snapshot: u8 = Kind::Snapshot as u8;
        let scores: u8 = Kind::Scores as u8;
        assert_eq!(
            get_kinds(&server.tick(SNAPSHOT_PERIOD_S)),
            vec![(one, snapshot), (two, snapshot)]
        );
        assert_eq!(
            get_kinds(&server.tick(SCORES_PERIOD_S)),
            vec![
                (one, snapshot),
                (two, snapshot),
                (one, scores),
                (two, scores)
            ]
        );
    }

    #[test]
    fn the_next_game_starts_after_a_win() {
        let mut by_itself: Game = Game::New(
            TEST_MAP_NAME.to_string(),
            ServerSettings::Builtin(),
            vec![0, 0, 1],
            TEST_PASSWORD.to_vec(),
            Some(NEW_GAME_AFTER_WIN_S),
        );
        let mut never: Game = make_server();
        for server in [&mut by_itself, &mut never] {
            join(server, address(50_001), RED);
            hold_console(server, 1, address(50_001), 0);
            hold_console(server, 1, address(50_001), 1);
            server.tick(0.0);
            server.tick(HALF * NEW_GAME_AFTER_WIN_S);
            assert_eq!((server.game.number, server.game.winner), (0, Some(1)));
            server.tick(NEW_GAME_AFTER_WIN_S);
        }
        assert_eq!((by_itself.game.number, by_itself.game.winner), (1, None));
        assert_eq!((never.game.number, never.game.winner), (0, Some(1)));
        never.startNextGame();
        assert_eq!((never.game.number, never.game.winner), (1, None));
        never.startNextGame();
        assert_eq!(never.game.number, 1);
    }

    #[test]
    #[should_panic(expected = "the time went back")]
    fn time_cannot_go_back() {
        let mut server: Game = make_server();
        server.tick(1.0);
        server.handle(&[Kind::LobbyQuery as u8], address(50_001), HALF);
    }
}
