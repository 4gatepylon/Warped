//! `match_play` runs the forest's objective and, in a network game, keeps the session in step
//! with the game server: other players become robots in their team's colour, hits, console holds,
//! thrown wormholes and hits on them go to the server, and the server's eliminations, consoles,
//! winner and wormhole damage come back.
//! Without a server it plays the same objective locally against the AI robots.
//!
//! Example: `play.update(&mut session, &scene, is_e_held, dt_s)?;`
//!
//! Interface: `MatchPlay`, `MatchOptions`.

use std::error::Error;
use std::time::Instant;

use glam::{DMat4, Mat4, Vec3};

use crate::console_reach::ConsolePlace;
use crate::game::bots::Bot;
use crate::game::guns::Gun;
use crate::game::session::{RemoteBot, Session};
use crate::net::{Client, NetEvent};
use crate::scene::Scene;
use crate::settings::ServerSettings;
use crate::thrown_wormholes::PlayerWormholes;
use warped_shared::movement::get_crouched_height_frac;

/// `STATE_PERIOD_S` is how often this player's state goes to the server.
const STATE_PERIOD_S: f64 = 1.0 / 30.0;

/// `MatchOptions` is how this window takes part in the game.
pub struct MatchOptions {
    pub team: Option<u32>,
    pub is_console_bot: bool,
}

/// `MatchPlay` is the objective state and, in a network game, the client.
pub struct MatchPlay {
    pub client: Option<Client>,
    options: MatchOptions,
    settings: ServerSettings,
    held_console: Option<usize>,
    hold_s: f32,
    last_state: Instant,
    was_dead: bool,
}

impl MatchPlay {
    /// `New` starts the objective; with a client it joins the game at once.
    ///
    /// Example: `let play = MatchPlay::New(Some(client), options)?;`
    #[allow(non_snake_case)]
    pub fn New(client: Option<Client>, options: MatchOptions) -> Result<MatchPlay, Box<dyn Error>> {
        let mut play: MatchPlay = MatchPlay {
            client,
            options,
            settings: ServerSettings::Builtin(),
            held_console: None,
            hold_s: 0.0,
            last_state: Instant::now(),
            was_dead: false,
        };
        let team: Option<u32> = play.options.team;
        if let Some(client) = &mut play.client {
            client.join(team)?;
        }
        Ok(play)
    }

    /// `status` is a one-line summary for the window title.
    ///
    /// Example: `window.set_title(&play.status(&session))?;`
    pub fn status(&self, session: &Session) -> String {
        let team_names: [&str; 2] = ["Blue", "Red"];
        let on_count = |team: u32| {
            session
                .arena
                .consoles
                .iter()
                .enumerate()
                .filter(|(i, c)| {
                    c.team == team && !session.console_off.get(*i).copied().unwrap_or(false)
                })
                .count()
        };
        let winner: String = session.winner.map_or(String::new(), |w| {
            format!(" | {} TEAM WINS", team_names[w as usize % 2].to_uppercase())
        });
        let connection: &str = match &self.client {
            None => "single player",
            Some(client) if client.id.is_none() => "joining",
            Some(_) => "online",
        };
        format!(
            "Dark Forest ({connection}) | you: {} | blue consoles on {} | red consoles on {}{winner}",
            team_names[session.team as usize % 2],
            on_count(0),
            on_count(1)
        )
    }

    /// `update` runs one frame of the objective and the network, after `Session::update`.
    ///
    /// Example: `play.update(&mut session, &scene, is_e_held, dt_s)?;`
    pub fn update(
        &mut self,
        session: &mut Session,
        scene: &Scene,
        is_e_held: bool,
        dt_s: f32,
    ) -> Result<(), Box<dyn Error>> {
        if session.console_off.len() != session.arena.consoles.len() {
            session.console_off = vec![false; session.arena.consoles.len()];
        }
        let target: Option<usize> = self.console_target(session, is_e_held);
        if target != self.held_console {
            self.hold_s = 0.0;
        }
        self.held_console = target;
        let hold_needed_s: f32 = self.settings.consoles.hold_to_disable_s as f32;
        if target.is_some() {
            self.hold_s += dt_s;
        }
        session.console_hold_frac = target.map(|_| (self.hold_s / hold_needed_s).min(1.0));
        match self.client.take() {
            None => self.update_local(session, hold_needed_s),
            Some(mut client) => {
                let result = self.update_online(&mut client, session, scene, dt_s);
                self.client = Some(client);
                result?;
            }
        }
        Ok(())
    }

    /// `console_target` is the other team's console this player holds `E` on: for the console bot,
    /// the one it holds once in reach (`console_bot.rs`); for a player holding `E`, the nearest one
    /// still on that their view meets within the settings' `consoles.reach_m`
    /// (`ConsolePlace::is_held_from`, the server's own test).
    fn console_target(&self, session: &Session, is_e_held: bool) -> Option<usize> {
        if !session.player.is_alive() || session.winner.is_some() {
            return None;
        }
        if self.options.is_console_bot {
            return session.bot_console;
        }
        if !is_e_held {
            return None;
        }
        let eye: DMat4 = session.player.camera_world().as_dmat4();
        session
            .console_places
            .iter()
            .enumerate()
            .filter(|(i, place)| {
                place.team != session.team
                    && !session.console_off[*i]
                    && place.is_held_from(eye, self.settings.consoles.reach_m)
            })
            .min_by(|(_, a), (_, b)| {
                let distance =
                    |place: &ConsolePlace| place.get_centre_m().distance(eye.w_axis.truncate());
                distance(a).total_cmp(&distance(b))
            })
            .map(|(i, _)| i)
    }

    /// `update_local` switches consoles off and picks the winner without a server.
    fn update_local(&mut self, session: &mut Session, hold_needed_s: f32) {
        if let Some(index) = self.held_console
            && self.hold_s >= hold_needed_s
        {
            session.console_off[index] = true;
            self.held_console = None;
            self.hold_s = 0.0;
        }
        for team in 0..2 {
            let consoles = session.arena.consoles.iter().enumerate();
            let mut owned = consoles.filter(|(_, c)| c.team == team).peekable();
            let has_consoles: bool = owned.peek().is_some();
            if session.winner.is_none()
                && has_consoles
                && owned.all(|(i, _)| session.console_off[i])
            {
                session.winner = Some(1 - team);
                println!("level won by {}", ["blue", "red"][1 - team as usize]);
            }
        }
    }

    /// `update_online` exchanges this frame's state with the server. A player eliminated by the
    /// server asks to join again only once the server's respawn wait, which the elimination
    /// carries, has passed (`client.rejoin`), not every `JOIN_RETRY_S` while dead, so the server
    /// sees them back after the same wait as in every level.
    fn update_online(
        &mut self,
        client: &mut Client,
        session: &mut Session,
        scene: &Scene,
        dt_s: f32,
    ) -> Result<(), Box<dyn Error>> {
        for event in client.poll()? {
            if let NetEvent::Eliminated {
                is_self: true,
                respawn_wait_s,
                ..
            } = event
            {
                session.respawn_wait_s = Some(respawn_wait_s as f32);
                session.player.take_damage(f32::MAX);
            }
        }
        if session.player.is_alive() {
            client.keep_joining()?;
        }
        if let Some(settings) = &client.settings {
            self.settings = settings.clone();
            if session.settings != *settings {
                session.set_settings(settings.clone());
            }
        }
        if let Some(team) = client.team {
            if team != session.team {
                session.team = team;
                session.respawn_player();
            }
            session.is_team_known = true;
        }
        let is_dead: bool = !session.player.is_alive();
        if self.was_dead && !is_dead {
            client.rejoin()?;
        }
        self.was_dead = is_dead;
        if !is_dead && client.id.is_some() {
            let max_health: f32 = crate::game::player::MAX_HEALTH;
            session.player.health = (client.health_frac as f32 * max_health).max(1.0);
        }
        for (index, is_off) in session.console_off.iter_mut().enumerate() {
            *is_off = client.game.off_mask & (1 << index) != 0;
        }
        session.winner = client.game.winner;
        self.sync_remotes(client, session, scene, dt_s);
        for (victim_id, hits_to_kill) in std::mem::take(&mut session.pending_hits) {
            client.send_hit(victim_id, hits_to_kill)?;
        }
        for (owner_id, hits_to_destroy) in std::mem::take(&mut session.pending_wormhole_hits) {
            client.send_wormhole_hit(owner_id, hits_to_destroy)?;
        }
        session.own_wormhole_damage_frac = client.wormhole_damage_frac;
        if self.last_state.elapsed().as_secs_f64() >= STATE_PERIOD_S && client.id.is_some() {
            self.last_state = Instant::now();
            let is_firing: bool = session.is_trigger_down && !is_dead;
            let eye: DMat4 = session.shot_eye.as_dmat4();
            let frame: DMat4 = if is_firing {
                session.weapons.shot_offset.aim(eye)
            } else {
                eye
            };
            client.send_state(
                frame,
                is_firing,
                session.weapons.gun == Gun::Rifle,
                self.held_console.map(|i| i as u8),
                session.weapons.beam_speckle_frac,
                session.thrown.placed,
                session.player.moves.crouch_frac,
            )?;
        }
        Ok(())
    }

    /// `sync_remotes` makes one robot per other player, placed from their frames: its feet one eye
    /// height below the eye they send (as crouched, W-137 (crouched robots sink): a crouched
    /// player's robot stands on the floor, squashed to the share of standing they crouch to). The
    /// session moves each robot's gait on (`Bot::follow_gait`).
    fn sync_remotes(&mut self, client: &Client, session: &mut Session, scene: &Scene, dt_s: f32) {
        let _ = (scene, dt_s);
        let own_id: Option<u32> = client.id;
        let mut remotes: Vec<RemoteBot> = Vec::new();
        let mut old: Vec<RemoteBot> = std::mem::take(&mut session.remote_bots);
        for player in client.players.iter().filter(|p| Some(p.id) != own_id) {
            let frame: Mat4 = player.frame.as_mat4();
            let eye_m: Vec3 = frame.w_axis.truncate();
            let forward: Vec3 = -frame.z_axis.truncate();
            let height_frac: f32 =
                get_crouched_height_frac(player.crouch_frac, &session.settings.movement) as f32;
            let feet_m: Vec3 = eye_m - Vec3::Y * (session.movement.eye_height_m * height_frac);
            let mut remote: RemoteBot = match old.iter().position(|r| r.id == player.id) {
                Some(i) => old.swap_remove(i),
                None => {
                    println!(
                        "sees player {} of the {} team",
                        player.id,
                        ["blue", "red"][player.team as usize % 2]
                    );
                    RemoteBot {
                        id: player.id,
                        team: player.team,
                        bot: Bot::new(feet_m, player.id),
                        frame,
                        is_firing: false,
                        was_firing: false,
                        is_rifle: false,
                        beam_speckle_frac: 0.0,
                        wormholes: PlayerWormholes::default(),
                    }
                }
            };
            remote.team = player.team;
            remote.bot.feet_m = feet_m;
            remote.bot.height_frac = height_frac;
            remote.bot.yaw_rad = forward.x.atan2(forward.z);
            remote.frame = frame;
            remote.is_firing = player.is_laser_on;
            remote.is_rifle = player.is_rifle;
            remote.beam_speckle_frac = player.beam_speckle_frac;
            remote.wormholes = player.wormholes;
            remotes.push(remote);
        }
        session.remote_bots = remotes;
    }
}
