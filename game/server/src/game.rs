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
                        .filter(|&console| self.can_disable(player.team, console));
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
        for slot in &self.slots {
            self.send(&message.0, slot.address);
        }
        Ok(())
    }
}
