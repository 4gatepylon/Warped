//! `online` is the `Session`'s game server side: it applies the server's messages each frame and
//! tells the player when the server can't be reached or refuses them.
//!
//! Interface: `Session::update_multiplayer`, `ConnectionProblem`.
//!
//! Example: `let is_running: bool = session.update_multiplayer()?;`

use glam::DVec4;

use crate::settings::ServerSettings;
use crate::voxel_world::{VoxelPlayer, VoxelWorld};

use super::{Multiplayer, Session, VoxelState, get_eyes, get_map_picture_max_px};
use crate::{Result, load_map, map, map_layout, net, settings};

const SERVER_SILENT_S: f64 = 2.0;

/// `ConnectionProblem` is why a window can't play on its server: nothing comes from it (no server
/// at that address and port, or it stopped), it runs another version of the game, or its game is
/// full.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ConnectionProblem {
    Silent,
    OtherVersion,
    Full,
}

impl ConnectionProblem {
    /// `get_lines` gives the banner's two lines for this problem with the server at `server`
    /// (short enough for the banner).
    ///
    /// Example: `let lines = ConnectionProblem::Silent.get_lines(server);`
    pub(crate) fn get_lines(self, server: std::net::SocketAddr) -> [String; 2] {
        match self {
            ConnectionProblem::Silent => [
                "NO ANSWER FROM THE SERVER".to_string(),
                format!("at {server}"),
            ],
            ConnectionProblem::OtherVersion => [
                "SERVER RUNS ANOTHER VERSION".to_string(),
                "Rebuild both from one commit".to_string(),
            ],
            ConnectionProblem::Full => [
                "THE GAME IS FULL".to_string(),
                format!("{} players already", net::MAX_PLAYER_COUNT),
            ],
        }
    }

    /// `get_hint` gives the longer advice for this problem with the server at `server`, for the
    /// terminal.
    ///
    /// Example: `eprintln!("{}", ConnectionProblem::Silent.get_hint(server));`
    fn get_hint(self, server: std::net::SocketAddr) -> String {
        match self {
            ConnectionProblem::Silent => format!(
                "No answer from a game server at {server} for {SERVER_SILENT_S} s. Is it running, on that address and port? The server prints the address and port to join (its own port, not the port of a window that joined it). Start one with `make serve-game`, then `make join-game SERVER=<address> PORT=<port>`."
            ),
            ConnectionProblem::OtherVersion => format!(
                "The game server at {server} runs another version of the game: its snapshots don't fit this one. Rebuild the server and this game from the same commit (pull, then restart `make serve-game`)."
            ),
            ConnectionProblem::Full => format!(
                "The game server at {server} already has {} players, so it can't take more until one leaves.",
                net::MAX_PLAYER_COUNT
            ),
        }
    }
}

impl Session {
    /// `update_multiplayer` reads the server's news: a welcome rebuilds a voxel map at the server's
    /// curvature if it differs from this window's and puts this player at its spawn point, and a
    /// hit is announced (and, if this player was hit, starts the respawn countdown).
    /// It returns whether this player is out of the game for now, and fails if the server can't be
    /// read.
    ///
    /// Example: `if session.update_multiplayer()? { return Ok(()); }`
    pub(crate) fn update_multiplayer(&mut self) -> Result<bool> {
        let Some(multiplayer) = &mut self.multiplayer else {
            return Ok(false);
        };
        let teams_before_poll: Vec<(u32, u32)> = multiplayer
            .client
            .players
            .iter()
            .map(|p| (p.id, p.team))
            .collect();
        let events: Vec<net::NetEvent> = multiplayer.client.poll()?;
        let mut messages: Vec<String> = Vec::new();
        for event in events {
            match event {
                net::NetEvent::Joined(id) => {
                    let others: Vec<DVec4> = get_eyes(&multiplayer.client.players);
                    let team: u32 = multiplayer.client.team.expect("a welcome brings the team");
                    let server_settings: ServerSettings = multiplayer
                        .client
                        .settings
                        .clone()
                        .expect("a welcome brings the server's settings");
                    if server_settings.space != self.settings.space {
                        let voxel: &mut VoxelState = &mut self.voxel;
                        let map: map::Map = load_map(self.map_name, &server_settings)?;
                        self.map_layout =
                            map_layout::MapLayout::FromMap(&map, get_map_picture_max_px())?;
                        let head_bob: settings::HeadBobSettings = voxel.world.head_bob.clone();
                        voxel.world = VoxelWorld::FromMap(&map)?;
                        voxel.world.head_bob = head_bob;
                        voxel.world.milky_way_brightness_frac =
                            self.player_settings.view.milky_way_brightness_frac;
                        messages.push(format!(
                            "The server curves space {} times as much as the map",
                            server_settings.space.curvature_per_map_curvature
                        ));
                    }
                    self.settings = server_settings;
                    self.voxel.world.set_settings(self.settings.clone());
                    self.voxel.spawn_index = id as usize;
                    self.voxel.player =
                        VoxelPlayer::NearSpawn(&self.voxel.world, team, id, &others);
                    messages.push(format!("You joined as player {id}"));
                }
                net::NetEvent::Eliminated {
                    victim_id,
                    shooter_id,
                    is_self,
                } => {
                    if is_self {
                        multiplayer.is_eliminated = true;
                        multiplayer.respawn_in_s = self.settings.respawn.delay_s;
                        multiplayer.disabling = None;
                        self.is_laser_on = false;
                        self.shot_time_s = 0.0;
                        self.thrown.clear();
                    }
                    if !is_self
                        && let Some(&(_, team)) =
                            teams_before_poll.iter().find(|(id, _)| *id == victim_id)
                    {
                        let now_s: f64 = self.start.elapsed().as_secs_f64();
                        self.avatars.add_corpse(victim_id, team, now_s);
                    }
                    messages.push(if is_self {
                        format!("You died: hit by player {shooter_id}")
                    } else {
                        format!("Player {victim_id} was hit by player {shooter_id}")
                    });
                }
            }
        }
        if multiplayer.client.health_frac < self.last_health_frac {
            self.hit_effect_frac = 1.0;
            {
                self.voxel.player.slowed_s = self.settings.when_hit.slow_s;
            }
        }
        self.last_health_frac = multiplayer.client.health_frac;
        if multiplayer.client.game.number != multiplayer.game_number {
            multiplayer.game_number = multiplayer.client.game.number;
            if let (voxel, Some(id), Some(team)) = (
                &mut self.voxel,
                multiplayer.client.id,
                multiplayer.client.team,
            ) {
                let others: Vec<DVec4> = get_eyes(&multiplayer.client.players);
                voxel.player = VoxelPlayer::NearSpawn(&voxel.world, team, id, &others);
            }
            messages.push(format!(
                "Game {} started: every console is back on",
                multiplayer.game_number
            ));
        }
        let is_out: bool = multiplayer.is_eliminated || multiplayer.client.id.is_none();
        if !multiplayer.is_eliminated
            && let Err(error) = multiplayer.client.keep_joining()
        {
            messages.push(format!("Couldn't reach the server: {error}"));
        }
        if let Some(message) = messages.pop() {
            self.show_message(message);
        }
        Ok(is_out)
    }

    /// `get_connection_problem` gives why this window can't play on its server right now, if it
    /// can't: nothing from the server for `SERVER_SILENT_S` while not out after a hit (no server
    /// at that address and port), its snapshots of another version, or a full game while asking
    /// to join.
    ///
    /// Example: `if let Some(problem) = self.get_connection_problem() { ... }`
    pub(crate) fn get_connection_problem(&self) -> Option<ConnectionProblem> {
        let multiplayer: &Multiplayer = self.multiplayer.as_ref()?;
        let client: &net::Client = &multiplayer.client;
        if !multiplayer.is_eliminated && client.get_silent_s() > SERVER_SILENT_S {
            Some(ConnectionProblem::Silent)
        } else if client.is_other_version() {
            Some(ConnectionProblem::OtherVersion)
        } else if client.is_full() && client.is_waiting_for_welcome() {
            Some(ConnectionProblem::Full)
        } else {
            None
        }
    }

    /// `report_connection_problem` prints a connection problem's longer hint to the terminal
    /// once, when it starts.
    ///
    /// Example: `self.report_connection_problem();`
    pub(crate) fn report_connection_problem(&mut self) {
        let problem: Option<ConnectionProblem> = self.get_connection_problem();
        if problem != self.last_connection_problem
            && let (Some(problem), Some(multiplayer)) = (problem, &self.multiplayer)
        {
            eprintln!("{}", problem.get_hint(multiplayer.client.get_server()));
        }
        self.last_connection_problem = problem;
    }
}
