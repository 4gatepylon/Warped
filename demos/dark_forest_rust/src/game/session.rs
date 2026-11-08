//! `session` runs a match: it updates the player, weapon, and bots, resolves shots, spawns
//! effects (beams, sparks, smoke, decals, light flashes), plays sounds, and turns the state
//! into the renderer's per-frame data. An optional autopilot plays the game by itself for
//! screenshots and benchmarks.
//!
//! Example:
//!     session.update(&controls, &scene, &audio, dt_s);
//!     let frame: FrameData = session.frame_data(&scene, width_px, height_px);

use crate::game::bots::BODY_CENTRE_HEIGHT_M;
use crate::game::warps::{BeamPath, Warps};
use std::collections::VecDeque;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use super::audio::{Audio, Sound};
use super::bots::{BOT_HEIGHT_M, Bot, BotClip, BotContext, BotShot, RESPAWN_DELAY_S, SHOT_DAMAGE};
use super::collision::{Aabb, cast_ray};
use super::hud::{HudState, build_hud};
use super::nav::NavGraph;
use super::player::{MAX_HEALTH, Player, PlayerInput};
use super::weapon::{MAGAZINE_SIZE, Weapon, WeaponEvent, WeaponInput};
use super::world::Arena;
use crate::assets::gltf_model::Trs;
use crate::render::{Camera, Decal, DrawItem, HudRect, Particle, ParticleKind, PointLight};
use crate::scene::{Scene, draw_items_for};

/// `MAX_DECAL_COUNT` is how many impact marks stay in the world.
const MAX_DECAL_COUNT: usize = 160;
/// `BODY_DAMAGE` is the damage of a body hit.
const BODY_DAMAGE: f32 = 34.0;
/// `HEAD_DAMAGE` is the damage of a headshot.
const HEAD_DAMAGE: f32 = 75.0;
/// `SHOT_RANGE_M` is the weapon's range.
const SHOT_RANGE_M: f32 = 250.0;
/// `TEAM_PLAYER_BEAM_COLOR` is the HDR color of the player's beam (times the light scale).
const TEAM_PLAYER_BEAM_COLOR: Vec3 = Vec3::new(0.35, 0.8, 1.6);
/// `MAP_BEAM_HDR_SCALE` brightens a map's laser colour (`Arena::laser_rgb`) to a beam's HDR level.
const MAP_BEAM_HDR_SCALE: f32 = 1.8;
/// `TEAM_BOT_BEAM_COLOR` is the HDR color of the bots' bolts (times the light scale).
const TEAM_BOT_BEAM_COLOR: Vec3 = Vec3::new(1.8, 0.25, 0.12);
/// `AUTOPILOT_HIP_FIRE_RANGE_M` is the range inside which the autopilot fires from the hip.
const AUTOPILOT_HIP_FIRE_RANGE_M: f32 = 14.0;
/// `NEAR_M` is the camera's near plane.
const NEAR_M: f32 = 0.05;
/// `FAR_M` is the camera's far plane.
const FAR_M: f32 = 600.0;
/// `VIEWMODEL_FOV_Y_RAD` is the viewmodel's field of view.
const VIEWMODEL_FOV_Y_RAD: f32 = 0.95;

/// `Controls` is one frame of input from the keyboard and mouse.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub player: PlayerInput,
    pub weapon: WeaponInput,
}

/// `RobotHold` holds robot 0 for pictures and benchmarks: it stands at `feet_m` facing `yaw_rad`,
/// doesn't think or shoot, and walks straight ahead at `speed_m_s` (through any mouth it enters),
/// starting again from `feet_m` every `ROBOT_HOLD_RESTART_S`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RobotHold {
    pub feet_m: Vec3,
    pub yaw_rad: f32,
    pub speed_m_s: f32,
}

/// `ROBOT_HOLD_RESTART_S` is how long a held robot walks before it starts again from its pose.
const ROBOT_HOLD_RESTART_S: f32 = 10.0;

/// `SimParticle` is a simulated particle.
#[derive(Clone, Copy, Debug)]
struct SimParticle {
    position_m: Vec3,
    velocity_m_s: Vec3,
    age_s: f32,
    life_s: f32,
    size_m: f32,
    color_rgba: Vec4,
    kind: ParticleKind,
    gravity_frac: f32,
    seed: f32,
}

/// `Beam` is a fading energy beam between two points.
#[derive(Clone, Copy, Debug)]
struct Beam {
    start_m: Vec3,
    end_m: Vec3,
    age_s: f32,
    life_s: f32,
    color_rgb: Vec3,
    width_m: f32,
}

/// `Flash` is a short-lived point light.
#[derive(Clone, Copy, Debug)]
struct Flash {
    position_m: Vec3,
    color_rgb: Vec3,
    radius_m: f32,
    age_s: f32,
    life_s: f32,
}

/// `FrameData` owns everything a frame description points at.
pub struct FrameData {
    pub camera: Camera,
    pub world_draws: Vec<DrawItem>,
    pub viewmodel_draws: Vec<DrawItem>,
    pub joint_matrices: Vec<Mat4>,
    pub point_lights: Vec<PointLight>,
    pub decals: Vec<Decal>,
    pub world_particles: Vec<Particle>,
    pub viewmodel_particles: Vec<Particle>,
    pub hud_rects: Vec<HudRect>,
    pub damage_flash_frac: f32,
}

/// `OFF_PYLON_TINT` darkens a pylon whose console is off.
const OFF_PYLON_TINT: Vec3 = Vec3::new(0.3, 0.3, 0.3);
/// `BEACON_BEAM_M`, `BEACON_BEAM_WIDTH_M` and `BEACON_BEAM_RGB` are the beam a pylon sends up
/// while its console is on, in its team's colour (blue, red), as in the hyperbolic demo.
const BEACON_BEAM_M: f32 = 25.0;
const BEACON_BEAM_WIDTH_M: f32 = 0.12;
const BEACON_BEAM_RGB: [Vec3; 2] = [Vec3::new(0.3, 0.6, 2.0), Vec3::new(2.0, 0.35, 0.2)];
/// `FLICKER_SLOW_RAD_PER_S` and `FLICKER_FAST_RAD_PER_S` are the two waves a flame flickers by.
const FLICKER_SLOW_RAD_PER_S: f32 = 11.0;
const FLICKER_FAST_RAD_PER_S: f32 = 27.0;

/// `get_flicker_scale` is how bright light `index` is at `time_s`: dimmed by up to
/// `flicker_frac`, out of step with the other lights.
///
/// Example: `color_rgb * get_flicker_scale(time_s, 3, 0.35)`
fn get_flicker_scale(time_s: f32, index: usize, flicker_frac: f32) -> f32 {
    let phase_rad: f32 = index as f32 * 1.7;
    let wave: f32 = 0.6 * (time_s * FLICKER_SLOW_RAD_PER_S + phase_rad).sin()
        + 0.4 * (time_s * FLICKER_FAST_RAD_PER_S + phase_rad * 2.3).sin();
    1.0 - flicker_frac * (0.5 + 0.5 * wave)
}

/// `Session` is one match.
pub struct Session {
    pub arena: Arena,
    pub nav: NavGraph,
    pub player: Player,
    pub weapon: Weapon,
    pub bots: Vec<Bot>,
    pub kill_count: u32,
    pub death_count: u32,
    pub time_s: f32,
    pub is_autopilot: bool,
    pub show_frame_ms: Option<f32>,
    decals: VecDeque<Decal>,
    particles: Vec<SimParticle>,
    beams: Vec<Beam>,
    flashes: Vec<Flash>,
    hit_marker_s: f32,
    kill_marker_s: f32,
    damage_flash_frac: f32,
    spawn_counter: usize,
    noise_state: u32,
    pub autopilot_waypoint: usize,
    autopilot_trigger_s: f32,
    pub route: Vec<Vec3>,
    pub remote_bots: Vec<RemoteBot>,
    pub pending_hits: Vec<u32>,
    pub team: u32,
    pub console_off: Vec<bool>,
    pub player_beam_color: Vec3,
    pub bot_beam_color: Vec3,
    pub console_hold_frac: Option<f32>,
    pub winner: Option<u32>,
    pub laps_done: u32,
    pub is_trigger_down: bool,
    robot_hold: Option<RobotHold>,
    robot_hold_s: f32,
}

/// `RemoteBot` is another player of a network game, drawn as a robot in their team's colour.
pub struct RemoteBot {
    pub id: u32,
    pub team: u32,
    pub bot: Bot,
    pub is_firing: bool,
    pub fire_cooldown_s: f32,
}

/// `TEAM_TINTS` are the robots' main colours per team (blue, red).
const TEAM_TINTS: [Vec3; 2] = [Vec3::new(0.12, 0.25, 0.75), Vec3::new(0.75, 0.14, 0.1)];
/// `REMOTE_FIRE_PERIOD_S` is how often a firing remote player's tracer is drawn.
const REMOTE_FIRE_PERIOD_S: f32 = 0.11;

/// `GROUND_PATROL` is the loop the autopilot walks by default, on the ground.
const GROUND_PATROL: [Vec3; 6] = [
    Vec3::new(0.0, 0.0, 40.0),
    Vec3::new(-8.0, 0.0, 10.0),
    Vec3::new(-6.0, 0.0, -20.0),
    Vec3::new(8.0, 0.0, -36.0),
    Vec3::new(8.0, 0.0, -10.0),
    Vec3::new(6.0, 0.0, 20.0),
];

impl Session {
    /// `new` starts a match with a number of bots.
    pub fn new(arena: Arena, bot_count: usize, is_autopilot: bool) -> Session {
        let laser_rgb: Option<Vec3> = arena.laser_rgb;
        let solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let nav: NavGraph = NavGraph::build(
            &solids,
            &arena.ladders,
            &arena.warps,
            arena.bounds_min_m,
            arena.bounds_max_m,
        );
        let (spawn_m, yaw_rad) = arena.team_spawns[1]
            .first()
            .copied()
            .unwrap_or(arena.player_spawns[0]);
        let bots: Vec<Bot> = (0..bot_count)
            .map(|index| {
                Bot::new(
                    arena.bot_spawns[index % arena.bot_spawns.len()],
                    index as u32 + 7,
                )
            })
            .collect();
        Session {
            player: Player::new(spawn_m, yaw_rad),
            arena,
            nav,
            weapon: Weapon::new(),
            bots,
            kill_count: 0,
            death_count: 0,
            time_s: 0.0,
            is_autopilot,
            show_frame_ms: None,
            decals: VecDeque::new(),
            particles: Vec::new(),
            beams: Vec::new(),
            flashes: Vec::new(),
            hit_marker_s: 0.0,
            kill_marker_s: 0.0,
            damage_flash_frac: 0.0,
            spawn_counter: 1,
            noise_state: 0xdead_beef,
            autopilot_waypoint: 0,
            autopilot_trigger_s: 0.0,
            route: GROUND_PATROL.to_vec(),
            remote_bots: Vec::new(),
            pending_hits: Vec::new(),
            team: 1,
            console_off: Vec::new(),
            player_beam_color: laser_rgb
                .map_or(TEAM_PLAYER_BEAM_COLOR, |rgb| rgb * MAP_BEAM_HDR_SCALE),
            bot_beam_color: laser_rgb.map_or(TEAM_BOT_BEAM_COLOR, |rgb| rgb * MAP_BEAM_HDR_SCALE),
            console_hold_frac: None,
            winner: None,
            laps_done: 0,
            is_trigger_down: false,
            robot_hold: None,
            robot_hold_s: 0.0,
        }
    }

    /// `set_robot_hold` holds robot 0 from the next update on (`None` lets it think again); a new
    /// hold starts at its pose.
    ///
    /// Example: `session.set_robot_hold(Some(hold));`
    pub fn set_robot_hold(&mut self, hold: Option<RobotHold>) {
        assert!(
            hold.is_none() || !self.bots.is_empty(),
            "a robot hold needs at least one robot"
        );
        if hold != self.robot_hold {
            self.robot_hold = hold;
            self.robot_hold_s = ROBOT_HOLD_RESTART_S;
        }
    }

    /// `random_signed` returns a pseudo-random number in [-1, 1].
    fn random_signed(&mut self) -> f32 {
        self.noise_state ^= self.noise_state << 13;
        self.noise_state ^= self.noise_state >> 17;
        self.noise_state ^= self.noise_state << 5;
        self.noise_state as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// `random_unit` returns a pseudo-random unit vector.
    fn random_unit(&mut self) -> Vec3 {
        Vec3::new(
            self.random_signed(),
            self.random_signed(),
            self.random_signed(),
        )
        .normalize_or(Vec3::Y)
    }

    /// `sound_placement` returns the gain and pan of a sound at a point.
    fn sound_placement(&self, position_m: Vec3) -> (f32, f32) {
        let to_sound: Vec3 = position_m - self.player.eye_position_m();
        let distance_m: f32 = to_sound.length();
        let right: Vec3 = self.player.rotation() * Vec3::X;
        let pan: f32 = if distance_m > 0.01 {
            right.dot(to_sound / distance_m) * 0.8
        } else {
            0.0
        };
        (1.0 / (1.0 + distance_m * 0.07), pan)
    }

    /// `muzzle_world_m` returns where the viewmodel's muzzle is in the world.
    fn muzzle_world_m(&self, scene: &Scene) -> Vec3 {
        let local: Mat4 =
            self.weapon
                .viewmodel_local(&self.player, &scene.viewmodel.pose, self.time_s);
        (self.player.camera_world() * local * scene.viewmodel.weapon_local)
            .transform_point3(scene.viewmodel.muzzle_local_m)
    }

    /// `add_impact` spawns sparks, smoke, a decal, and a flash where a shot hit a surface.
    fn add_impact(
        &mut self,
        position_m: Vec3,
        normal: Vec3,
        color_rgb: Vec3,
        light_scale: f32,
        has_decal: bool,
    ) {
        for _ in 0..14 {
            let direction: Vec3 = (normal * 1.2 + self.random_unit()).normalize_or(normal);
            let speed_m_s: f32 = 3.0 + (self.random_signed() + 1.0) * 4.0;
            let seed: f32 = self.random_signed();
            self.particles.push(SimParticle {
                position_m: position_m + normal * 0.02,
                velocity_m_s: direction * speed_m_s,
                age_s: 0.0,
                life_s: 0.25 + (seed + 1.0) * 0.2,
                size_m: 0.012,
                color_rgba: (color_rgb * light_scale * 0.6).extend(0.0),
                kind: ParticleKind::Spark,
                gravity_frac: 1.0,
                seed,
            });
        }
        for index in 0..3 {
            let seed: f32 = self.random_signed();
            let drift: Vec3 = self.random_unit();
            self.particles.push(SimParticle {
                position_m: position_m + normal * (0.05 + index as f32 * 0.04),
                velocity_m_s: normal * 0.6 + drift * 0.25 + Vec3::Y * 0.3,
                age_s: 0.0,
                life_s: 1.4 + seed * 0.3,
                size_m: 0.22 + index as f32 * 0.06,
                color_rgba: Vec4::new(0.32, 0.31, 0.3, 0.38),
                kind: ParticleKind::Smoke,
                gravity_frac: -0.02,
                seed,
            });
        }
        if has_decal {
            if self.decals.len() >= MAX_DECAL_COUNT {
                self.decals.pop_front();
            }
            let seed: f32 = self.random_signed();
            self.decals.push_back(Decal {
                position_m,
                normal,
                tangent: normal.any_orthogonal_vector().normalize(),
                size_m: 0.16 + seed.abs() * 0.06,
                seed,
            });
        }
        self.flashes.push(Flash {
            position_m: position_m + normal * 0.3,
            color_rgb: color_rgb * light_scale * 0.25,
            radius_m: 5.0,
            age_s: 0.0,
            life_s: 0.12,
        });
    }

    /// `fire_player_shot` resolves one player shot against the world and the bots.
    fn fire_player_shot(&mut self, scene: &Scene, audio: &Audio) {
        /// `ShotHit` is what a laser hit first.
        #[derive(Clone, Copy)]
        enum ShotHit {
            World(Vec3),
            Bot(usize, bool, Vec3),
            Remote(u32, Vec3),
        }
        let origin_m: Vec3 = self.player.eye_position_m();
        let direction: Vec3 = self.weapon.shot_direction(&self.player);
        let mut shot_hit: Option<ShotHit> = None;
        let path: BeamPath = self.arena.warps.trace_beam(
            origin_m,
            direction,
            SHOT_RANGE_M,
            |start_m: Vec3, piece_direction: Vec3, length_m: f32| {
                let mut nearest_m: f32 = length_m;
                let mut found: Option<ShotHit> = None;
                if let Some((_, hit)) = cast_ray(&scene.solids, start_m, piece_direction, nearest_m)
                {
                    nearest_m = hit.distance_m;
                    found = Some(ShotHit::World(hit.normal));
                }
                for (index, bot) in self
                    .bots
                    .iter()
                    .enumerate()
                    .filter(|(_, bot)| bot.is_alive())
                {
                    for (aabb, is_head) in [(bot.head(), true), (bot.body(), false)] {
                        if let Some(hit) = aabb.cast_ray(start_m, piece_direction, nearest_m) {
                            nearest_m = hit.distance_m;
                            found = Some(ShotHit::Bot(index, is_head, hit.normal));
                        }
                    }
                }
                for remote in self.remote_bots.iter().filter(|r| r.bot.is_alive()) {
                    for aabb in [remote.bot.head(), remote.bot.body()] {
                        if let Some(hit) = aabb.cast_ray(start_m, piece_direction, nearest_m) {
                            nearest_m = hit.distance_m;
                            found = Some(ShotHit::Remote(remote.id, hit.normal));
                        }
                    }
                }
                found.map(|hit| {
                    shot_hit = Some(hit);
                    nearest_m
                })
            },
        );
        let end_m: Vec3 = path.end_m;
        let muzzle_m: Vec3 = self.muzzle_world_m(scene);
        for (index, (start_m, piece_end_m)) in path.pieces.iter().enumerate() {
            self.beams.push(Beam {
                start_m: if index == 0 { muzzle_m } else { *start_m },
                end_m: *piece_end_m,
                age_s: 0.0,
                life_s: 0.16,
                color_rgb: self.player_beam_color * scene.light_scale,
                width_m: 0.022,
            });
        }
        self.flashes.push(Flash {
            position_m: muzzle_m,
            color_rgb: self.player_beam_color * scene.light_scale * 0.5,
            radius_m: 7.0,
            age_s: 0.0,
            life_s: 0.07,
        });
        audio.play(Sound::Shot, 0.9, 0.0);
        match shot_hit {
            Some(ShotHit::Remote(id, normal)) => {
                self.pending_hits.push(id);
                self.add_impact(
                    end_m,
                    normal,
                    self.player_beam_color,
                    scene.light_scale,
                    false,
                );
                self.hit_marker_s = 0.25;
                audio.play(Sound::HitMarker, 0.5, 0.0);
            }
            Some(ShotHit::Bot(index, is_head, normal)) => {
                let damage: f32 = if is_head { HEAD_DAMAGE } else { BODY_DAMAGE };
                let did_kill: bool = self.bots[index].take_damage(damage);
                self.add_impact(
                    end_m,
                    normal,
                    self.player_beam_color,
                    scene.light_scale,
                    false,
                );
                self.hit_marker_s = 0.25;
                audio.play(Sound::HitMarker, 0.5, 0.0);
                if did_kill {
                    self.kill_count += 1;
                    self.kill_marker_s = 0.4;
                    let (gain, pan) = self.sound_placement(end_m);
                    audio.play(Sound::BotDeath, gain * 1.4, pan);
                }
            }
            Some(ShotHit::World(normal)) => {
                self.add_impact(
                    end_m,
                    normal,
                    self.player_beam_color,
                    scene.light_scale,
                    true,
                );
                let (gain, pan) = self.sound_placement(end_m);
                audio.play(Sound::Impact, gain, pan);
            }
            None => {}
        }
    }

    /// `apply_bot_shot` shows a bot's bolt and applies its damage.
    fn apply_bot_shot(&mut self, shot: BotShot, scene: &Scene, audio: &Audio) {
        self.beams.push(Beam {
            start_m: shot.origin_m,
            end_m: shot.end_m,
            age_s: 0.0,
            life_s: 0.22,
            color_rgb: self.bot_beam_color * scene.light_scale,
            width_m: 0.03,
        });
        self.flashes.push(Flash {
            position_m: shot.origin_m,
            color_rgb: self.bot_beam_color * scene.light_scale * 0.4,
            radius_m: 6.0,
            age_s: 0.0,
            life_s: 0.1,
        });
        let (gain, pan) = self.sound_placement(shot.origin_m);
        audio.play(Sound::BotShot, gain * 1.3, pan);
        if shot.did_hit_player {
            self.player.take_damage(SHOT_DAMAGE);
            self.damage_flash_frac = 1.0;
            audio.play(Sound::Hurt, 0.8, pan);
        } else {
            self.add_impact(
                shot.end_m,
                shot.end_normal,
                self.bot_beam_color,
                scene.light_scale,
                true,
            );
        }
    }

    /// `farthest_spawn` picks the spawn point farthest from a set of points.
    fn farthest_spawn(spawns: &[Vec3], others: &[Vec3], counter: usize) -> Vec3 {
        let mut ranked: Vec<(f32, Vec3)> = spawns
            .iter()
            .map(|spawn| {
                let nearest_m: f32 = others
                    .iter()
                    .map(|other| other.distance(*spawn))
                    .fold(f32::MAX, f32::min);
                (nearest_m, *spawn)
            })
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
        ranked[counter % ranked.len().min(2)].1
    }

    /// `autopilot_controls` plays the game: walk the waypoint loop, aim at visible bots, fire.
    fn autopilot_controls(&mut self, scene: &Scene, dt_s: f32) -> Controls {
        let eye_m: Vec3 = self.player.eye_position_m();
        let waypoint: Vec3 = self.route[self.autopilot_waypoint % self.route.len()];
        let target: Vec2 = Vec2::new(waypoint.x, waypoint.z);
        let feet: Vec2 = Vec2::new(self.player.feet_m.x, self.player.feet_m.z);
        if feet.distance(target) < 1.0 && (self.player.feet_m.y - waypoint.y).abs() < 1.2 {
            self.autopilot_waypoint = (self.autopilot_waypoint + 1) % self.route.len();
            if self.autopilot_waypoint == 0 {
                self.laps_done += 1;
            }
        }
        let walk_direction: Vec2 = (target - feet).normalize_or_zero();
        let my_team: u32 = self.team;
        let visible_bot: Option<Vec3> = self
            .bots
            .iter()
            .chain(
                self.remote_bots
                    .iter()
                    .filter(|r| r.team != my_team)
                    .map(|r| &r.bot),
            )
            .filter(|bot| bot.is_alive())
            .map(|bot| bot.feet_m + Vec3::Y * (BOT_HEIGHT_M * 0.65))
            .filter(|aim_m| {
                let to_bot: Vec3 = *aim_m - eye_m;
                to_bot.length() < 45.0
                    && cast_ray(&scene.solids, eye_m, to_bot.normalize(), to_bot.length()).is_none()
            })
            .min_by(|a, b| a.distance(eye_m).total_cmp(&b.distance(eye_m)));
        let look_direction: Vec3 = match visible_bot {
            Some(aim_m) => (aim_m - eye_m).normalize(),
            None => Vec3::new(walk_direction.x, -0.04, walk_direction.y).normalize_or(Vec3::NEG_Z),
        };
        let target_yaw_rad: f32 = (-look_direction.x).atan2(-look_direction.z);
        let target_pitch_rad: f32 = look_direction.y.asin();
        let mut yaw_delta_rad: f32 = target_yaw_rad - self.player.yaw_rad;
        yaw_delta_rad = (yaw_delta_rad + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let pitch_delta_rad: f32 = target_pitch_rad - self.player.pitch_rad;
        let turn_frac: f32 = 1.0 - (-5.0 * dt_s).exp();
        let look_delta_rad: Vec2 = Vec2::new(-yaw_delta_rad, -pitch_delta_rad) * turn_frac;
        let forward: Vec2 = Vec2::new(-self.player.yaw_rad.sin(), -self.player.yaw_rad.cos());
        let right: Vec2 = Vec2::new(self.player.yaw_rad.cos(), -self.player.yaw_rad.sin());
        let is_on_target: bool = visible_bot.is_some() && yaw_delta_rad.abs() < 0.05;
        let should_aim: bool =
            visible_bot.is_some_and(|aim_m| aim_m.distance(eye_m) > AUTOPILOT_HIP_FIRE_RANGE_M);
        self.autopilot_trigger_s = if is_on_target {
            self.autopilot_trigger_s + dt_s
        } else {
            0.0
        };
        Controls {
            player: PlayerInput {
                move_axes: Vec2::new(walk_direction.dot(right), walk_direction.dot(forward))
                    * if visible_bot.is_some() { 0.5 } else { 1.0 },
                look_delta_rad,
                is_sprint_held: visible_bot.is_none(),
                is_crouch_held: false,
                is_jump_pressed: false,
                is_aiming: should_aim,
            },
            weapon: WeaponInput {
                is_trigger_held: self.autopilot_trigger_s > 0.25,
                is_reload_pressed: self.weapon.ammo == 0,
                is_aim_held: should_aim,
            },
        }
    }

    /// `update` advances the match by one frame.
    pub fn update(&mut self, controls: &Controls, scene: &Scene, audio: &Audio, dt_s: f32) {
        self.time_s += dt_s;
        let controls: Controls = if self.is_autopilot {
            self.autopilot_controls(scene, dt_s)
        } else {
            *controls
        };
        self.is_trigger_down = controls.weapon.is_trigger_held && self.weapon.ammo > 0;
        let mut player_input: PlayerInput = controls.player;
        player_input.is_aiming = controls.weapon.is_aim_held && self.weapon.reload_time_s.is_none();
        self.player
            .update(&player_input, &scene.solids, &self.arena.ladders, dt_s);
        if self.player.is_alive() {
            let warps: &Warps = &self.arena.warps;
            let centre_m: Vec3 = self.player.feet_m + Vec3::Y * BODY_CENTRE_HEIGHT_M;
            self.player.velocity_m_s += warps.get_pull_m_per_s2(centre_m) * dt_s;
            if let Some((exit_m, turn)) = warps.get_mouth_exit_feet_m(self.player.feet_m, centre_m)
            {
                let yaw_rad: f32 = self.player.yaw_rad;
                let facing: Vec3 = turn * Vec3::new(-yaw_rad.sin(), 0.0, -yaw_rad.cos());
                self.player.feet_m = exit_m;
                self.player.velocity_m_s = turn * self.player.velocity_m_s;
                self.player.yaw_rad = (-facing.x).atan2(-facing.z);
            }
            if warps.is_inside_horizon(self.player.eye_position_m()) {
                self.player.take_damage(f32::MAX);
            }
        }
        if self.player.did_step {
            audio.play(
                Sound::Footstep,
                if self.player.is_sprinting { 0.35 } else { 0.22 },
                0.0,
            );
        }
        match self.weapon.update(
            &controls.weapon,
            &mut self.player,
            controls.player.look_delta_rad,
            dt_s,
        ) {
            Some(WeaponEvent::Fired) => {
                self.fire_player_shot(scene, audio);
                self.weapon.apply_recoil(&mut self.player);
            }
            Some(WeaponEvent::ReloadStarted) => audio.play(Sound::Reload, 0.6, 0.15),
            None => {}
        }
        if self.player.should_respawn() {
            self.death_count += 1;
            let bot_positions: Vec<Vec3> = self.bots.iter().map(|bot| bot.feet_m).collect();
            let team_spawns: &[(Vec3, f32)] = &self.arena.team_spawns[self.team as usize % 2];
            let spawns: Vec<Vec3> = team_spawns.iter().map(|(p, _)| *p).collect();
            let spawn_m: Vec3 =
                Session::farthest_spawn(&spawns, &bot_positions, self.spawn_counter);
            let yaw_rad: f32 = self.arena.team_spawns[self.team as usize % 2]
                .iter()
                .find(|(p, _)| *p == spawn_m)
                .map_or(0.0, |(_, yaw)| *yaw);
            self.spawn_counter += 1;
            self.player = Player::new(spawn_m, yaw_rad);
            self.weapon = Weapon::new();
        }

        let context: BotContext = BotContext {
            solids: &scene.solids,
            nav: &self.nav,
            ladders: &self.arena.ladders,
            player_eye_m: self.player.eye_position_m(),
            player_velocity_m_s: self.player.velocity_m_s,
            is_player_alive: self.player.is_alive(),
            is_player_crouched: self.player.height_m < 1.5,
        };
        let held_count: usize = usize::from(self.robot_hold.is_some());
        let shots: Vec<BotShot> = self
            .bots
            .iter_mut()
            .skip(held_count)
            .filter_map(|bot| bot.update(&context, dt_s))
            .collect();
        if let Some(hold) = self.robot_hold {
            self.robot_hold_s += dt_s;
            if self.robot_hold_s > ROBOT_HOLD_RESTART_S {
                self.robot_hold_s = 0.0;
                self.bots[0] = Bot::new(hold.feet_m, 0);
                self.bots[0].yaw_rad = hold.yaw_rad;
            }
            self.bots[0].walk_ahead(hold.speed_m_s, dt_s);
        }
        for shot in shots {
            self.apply_bot_shot(shot, scene, audio);
        }
        for bot in self.bots.iter_mut().filter(|bot| bot.is_alive()) {
            bot.apply_warps(&self.arena.warps, dt_s);
        }
        let player_feet_m: Vec3 = self.player.feet_m;
        for index in 0..self.bots.len() {
            if self.bots[index]
                .dead_time_s
                .is_some_and(|time_s| time_s > RESPAWN_DELAY_S)
            {
                let spawn_m: Vec3 =
                    Session::farthest_spawn(&self.arena.bot_spawns, &[player_feet_m], index);
                self.bots[index].respawn(spawn_m);
            }
        }

        let mut remote_beams: Vec<Beam> = Vec::new();
        for remote in &mut self.remote_bots {
            remote.fire_cooldown_s -= dt_s;
            remote.bot.animation.advance(dt_s, 1.0);
            if remote.is_firing && remote.bot.is_alive() && remote.fire_cooldown_s <= 0.0 {
                remote.fire_cooldown_s = REMOTE_FIRE_PERIOD_S;
                let origin_m: Vec3 = remote.bot.eye_m();
                let direction: Vec3 = remote.bot.facing();
                let path: BeamPath = self.arena.warps.trace_beam(
                    origin_m,
                    direction,
                    SHOT_RANGE_M,
                    |start_m: Vec3, piece_direction: Vec3, length_m: f32| {
                        cast_ray(&scene.solids, start_m, piece_direction, length_m)
                            .map(|(_, hit)| hit.distance_m)
                    },
                );
                for (start_m, end_m) in path.pieces {
                    remote_beams.push(Beam {
                        start_m,
                        end_m,
                        age_s: 0.0,
                        life_s: 0.12,
                        color_rgb: self.bot_beam_color * scene.light_scale,
                        width_m: 0.022,
                    });
                }
            }
        }
        self.beams.extend(remote_beams);
        for particle in &mut self.particles {
            particle.age_s += dt_s;
            particle.velocity_m_s.y -= 9.8 * particle.gravity_frac * dt_s;
            particle.velocity_m_s *= (-1.5 * dt_s).exp();
            particle.position_m += particle.velocity_m_s * dt_s;
            if particle.kind == ParticleKind::Spark && particle.position_m.y < 0.01 {
                particle.position_m.y = 0.01;
                particle.velocity_m_s.y = particle.velocity_m_s.y.abs() * 0.35;
            }
        }
        self.particles
            .retain(|particle| particle.age_s < particle.life_s);
        for beam in &mut self.beams {
            beam.age_s += dt_s;
        }
        self.beams.retain(|beam| beam.age_s < beam.life_s);
        for flash in &mut self.flashes {
            flash.age_s += dt_s;
        }
        self.flashes.retain(|flash| flash.age_s < flash.life_s);
        self.hit_marker_s = (self.hit_marker_s - dt_s).max(0.0);
        self.kill_marker_s = (self.kill_marker_s - dt_s).max(0.0);
        self.damage_flash_frac = (self.damage_flash_frac - dt_s * 2.5).max(0.0);
    }

    /// `bot_pose` samples a bot's blended animation pose.
    fn bot_pose(bot: &Bot, scene: &Scene) -> Vec<Trs> {
        let skeleton = scene
            .robot
            .skeleton
            .as_ref()
            .expect("the robot is animated");
        let clip_index = |clip: BotClip| match clip {
            BotClip::Idle => scene.robot_clips[0],
            BotClip::Walk => scene.robot_clips[1],
            BotClip::Run => scene.robot_clips[2],
            BotClip::Death => scene.robot_clips[3],
        };
        let clip_time = |clip: BotClip, time_s: f32| {
            let duration_s: f32 = skeleton.animations[clip_index(clip)].duration_s.max(1e-3);
            if clip == BotClip::Death {
                time_s.min(duration_s)
            } else {
                time_s.rem_euclid(duration_s)
            }
        };
        let animation = &bot.animation;
        let current: Vec<Trs> = skeleton.sample(
            clip_index(animation.clip),
            clip_time(animation.clip, animation.time_s),
        );
        if animation.blend_frac >= 1.0 {
            return current;
        }
        let previous: Vec<Trs> = skeleton.sample(
            clip_index(animation.previous_clip),
            clip_time(animation.previous_clip, animation.previous_time_s),
        );
        previous
            .iter()
            .zip(&current)
            .map(|(a, b)| a.lerp(b, animation.blend_frac))
            .collect()
    }

    /// `objective_hud` draws the consoles (lit while on, in their team's colour), the hold
    /// progress while `E` is held on one, and the winner's banner.
    fn objective_hud(&self, width_px: f32, height_px: f32) -> Vec<HudRect> {
        let scale: f32 = height_px / 1080.0;
        let mut rects: Vec<HudRect> = Vec::new();
        let colors: [Vec4; 2] = [
            Vec4::new(0.2, 0.45, 1.0, 0.95),
            Vec4::new(1.0, 0.25, 0.15, 0.95),
        ];
        let pip_px: f32 = 34.0 * scale;
        let gap_px: f32 = 14.0 * scale;
        let count: usize = self.arena.consoles.len();
        let total_px: f32 = count as f32 * pip_px + count.saturating_sub(1) as f32 * gap_px;
        for (index, console) in self.arena.consoles.iter().enumerate() {
            let x_px: f32 = (width_px - total_px) * 0.5 + index as f32 * (pip_px + gap_px);
            let is_off: bool = self.console_off.get(index).copied().unwrap_or(false);
            rects.push(HudRect {
                x_px: x_px - 3.0 * scale,
                y_px: 24.0 * scale - 3.0 * scale,
                width_px: pip_px + 6.0 * scale,
                height_px: pip_px + 6.0 * scale,
                color_rgba: Vec4::new(0.0, 0.0, 0.0, 0.5),
            });
            let color: Vec4 = colors[console.team as usize % 2];
            rects.push(HudRect {
                x_px,
                y_px: 24.0 * scale,
                width_px: pip_px,
                height_px: pip_px,
                color_rgba: if is_off {
                    color * Vec4::new(0.2, 0.2, 0.2, 0.8)
                } else {
                    color
                },
            });
        }
        if let Some(frac) = self.console_hold_frac {
            let bar_px: Vec2 = Vec2::new(320.0, 16.0) * scale;
            let x_px: f32 = (width_px - bar_px.x) * 0.5;
            let y_px: f32 = height_px * 0.5 + 70.0 * scale;
            rects.push(HudRect {
                x_px,
                y_px,
                width_px: bar_px.x,
                height_px: bar_px.y,
                color_rgba: Vec4::new(0.0, 0.0, 0.0, 0.55),
            });
            rects.push(HudRect {
                x_px,
                y_px,
                width_px: bar_px.x * frac.clamp(0.0, 1.0),
                height_px: bar_px.y,
                color_rgba: Vec4::new(1.0, 0.85, 0.35, 0.95),
            });
        }
        if let Some(winner) = self.winner {
            rects.push(HudRect {
                x_px: 0.0,
                y_px: height_px * 0.28,
                width_px,
                height_px: 110.0 * scale,
                color_rgba: colors[winner as usize % 2] * Vec4::new(1.0, 1.0, 1.0, 0.7),
            });
        }
        rects
    }

    /// `frame_data` turns the match into one frame for the renderer.
    pub fn frame_data(&self, scene: &Scene, width_px: f32, height_px: f32) -> FrameData {
        let camera_world: Mat4 = self.player.camera_world();
        let camera: Camera = Camera {
            view: camera_world.inverse(),
            position_m: self.player.eye_position_m(),
            fov_y_rad: self.weapon.fov_y_rad(),
            viewmodel_fov_y_rad: VIEWMODEL_FOV_Y_RAD,
            near_m: NEAR_M,
            far_m: FAR_M,
        };
        let mut world_draws: Vec<DrawItem> = scene.static_draws.clone();
        let mut joint_matrices: Vec<Mat4> = Vec::new();
        let skeleton = scene
            .robot
            .skeleton
            .as_ref()
            .expect("the robot is animated");
        let mut point_lights: Vec<PointLight> = self
            .arena
            .lights
            .iter()
            .enumerate()
            .map(|(index, light)| PointLight {
                position_m: light.position_m,
                radius_m: light.radius_m,
                color_rgb: light.color_rgb
                    * scene.light_scale
                    * get_flicker_scale(self.time_s, index, light.flicker_frac),
            })
            .collect();
        let mut world_particles: Vec<Particle> = Vec::new();
        let remote_entries = self
            .remote_bots
            .iter()
            .map(|remote| (&remote.bot, TEAM_TINTS[remote.team as usize % 2]));
        for (bot, main_tint) in self
            .bots
            .iter()
            .map(|bot| (bot, TEAM_TINTS[(self.team as usize + 1) % 2]))
            .chain(remote_entries)
        {
            let sink_m: f32 = bot.dead_time_s.map_or(0.0, |time_s| {
                ((time_s - (RESPAWN_DELAY_S - 1.5)) / 1.5).clamp(0.0, 1.0) * 1.2
            });
            let model: Mat4 = Mat4::from_translation(bot.feet_m - Vec3::Y * sink_m)
                * Mat4::from_quat(Quat::from_rotation_y(bot.yaw_rad))
                * Mat4::from_scale(Vec3::splat(scene.robot_scale));
            let joint_offset: u32 = joint_matrices.len() as u32;
            skeleton.write_palette(&Session::bot_pose(bot, scene), model, &mut joint_matrices);
            let flash: f32 = bot.hit_flash_frac;
            for part in &scene.robot.parts {
                let name: &str = part
                    .material_index
                    .and_then(|index| scene.robot_material_names.get(index))
                    .map_or("", String::as_str);
                let tint: Vec3 = match name {
                    "Main" => main_tint,
                    "Grey" => Vec3::new(0.75, 0.74, 0.72),
                    _ => Vec3::ONE,
                };
                world_draws.push(DrawItem {
                    mesh: part.mesh,
                    material: part.material,
                    model,
                    tint: (tint + Vec3::new(2.0, 0.3, 0.2) * flash).extend(1.0),
                    joint_offset: Some(joint_offset),
                    bounds_center_m: bot.feet_m + Vec3::Y * 1.0,
                    bounds_radius_m: 1.6,
                    does_cast_shadow: true,
                });
            }
            if bot.is_alive() {
                let eye_m: Vec3 = bot.feet_m + Vec3::Y * (BOT_HEIGHT_M * 0.87) + bot.facing() * 0.3;
                world_particles.push(Particle {
                    position_m: eye_m,
                    stretch_m: Vec3::ZERO,
                    size_m: 0.07 + bot.muzzle_flash_frac * 0.2,
                    color_rgba: (self.bot_beam_color
                        * scene.light_scale
                        * (0.25 + bot.muzzle_flash_frac))
                        .extend(0.0),
                    kind: ParticleKind::MuzzleFlash,
                    seed: 0.3,
                });
                point_lights.push(PointLight {
                    position_m: eye_m,
                    radius_m: 3.0 + bot.muzzle_flash_frac * 4.0,
                    color_rgb: self.bot_beam_color
                        * scene.light_scale
                        * (0.02 + bot.muzzle_flash_frac * 0.3),
                });
            }
        }
        for flash in &self.flashes {
            let frac: f32 = 1.0 - flash.age_s / flash.life_s;
            point_lights.push(PointLight {
                position_m: flash.position_m,
                radius_m: flash.radius_m,
                color_rgb: flash.color_rgb * frac * frac,
            });
        }
        let beacon_height_m: f32 = scene.beacon.bounds_max_m.y - scene.beacon.bounds_min_m.y;
        for pylon in &self.arena.pylons {
            let is_off: bool = self
                .console_off
                .get(pylon.console_index)
                .copied()
                .unwrap_or(false);
            let scale: f32 = pylon.height_m / beacon_height_m.max(1e-3);
            let model: Mat4 = Mat4::from_translation(
                pylon.foot_m - Vec3::Y * (scene.beacon.bounds_min_m.y * scale),
            ) * Mat4::from_scale(Vec3::splat(scale));
            let tint: Vec3 = if is_off { OFF_PYLON_TINT } else { Vec3::ONE };
            for part in &scene.beacon.parts {
                world_draws.push(DrawItem {
                    mesh: part.mesh,
                    material: part.material,
                    model,
                    tint: tint.extend(1.0),
                    joint_offset: None,
                    bounds_center_m: pylon.foot_m + Vec3::Y * (pylon.height_m * 0.5),
                    bounds_radius_m: pylon.height_m,
                    does_cast_shadow: true,
                });
            }
            if !is_off {
                let team: usize = self.arena.consoles[pylon.console_index].team as usize % 2;
                let top_m: Vec3 = pylon.foot_m + Vec3::Y * pylon.height_m;
                world_particles.push(Particle {
                    position_m: top_m + Vec3::Y * (BEACON_BEAM_M * 0.5),
                    stretch_m: Vec3::Y * BEACON_BEAM_M,
                    size_m: BEACON_BEAM_WIDTH_M,
                    color_rgba: (BEACON_BEAM_RGB[team] * scene.light_scale).extend(0.0),
                    kind: ParticleKind::Tracer,
                    seed: 0.0,
                });
            }
        }
        for beam in &self.beams {
            let frac: f32 = 1.0 - beam.age_s / beam.life_s;
            world_particles.push(Particle {
                position_m: (beam.start_m + beam.end_m) * 0.5,
                stretch_m: beam.end_m - beam.start_m,
                size_m: beam.width_m * (0.4 + frac * 0.6),
                color_rgba: (beam.color_rgb * frac * frac).extend(0.0),
                kind: ParticleKind::Tracer,
                seed: 0.0,
            });
        }
        for particle in &self.particles {
            let frac: f32 = 1.0 - particle.age_s / particle.life_s;
            let (size_m, color_rgba, stretch_m) = match particle.kind {
                ParticleKind::Smoke => (
                    particle.size_m * (1.0 + (1.0 - frac) * 2.5),
                    particle.color_rgba.with_w(particle.color_rgba.w * frac),
                    Vec3::ZERO,
                ),
                _ => (
                    particle.size_m,
                    particle.color_rgba * frac,
                    particle.velocity_m_s * 0.02,
                ),
            };
            world_particles.push(Particle {
                position_m: particle.position_m,
                stretch_m,
                size_m,
                color_rgba,
                kind: particle.kind,
                seed: particle.seed,
            });
        }

        let mut viewmodel_draws: Vec<DrawItem> = Vec::new();
        let mut viewmodel_particles: Vec<Particle> = Vec::new();
        if self.player.is_alive() {
            let viewmodel = &scene.viewmodel;
            let weapon_world: Mat4 = camera_world
                * self
                    .weapon
                    .viewmodel_local(&self.player, &viewmodel.pose, self.time_s);
            viewmodel_draws.extend(draw_items_for(
                &viewmodel.weapon,
                weapon_world * viewmodel.weapon_local,
                false,
            ));
            if let Some(arms) = &viewmodel.arms {
                viewmodel_draws.extend(draw_items_for(
                    arms,
                    weapon_world * viewmodel.arms_local,
                    false,
                ));
            }
            let flash_frac: f32 = (1.0 - self.weapon.since_shot_s / 0.06).max(0.0);
            let muzzle_m: Vec3 =
                (weapon_world * viewmodel.weapon_local).transform_point3(viewmodel.muzzle_local_m);
            if flash_frac > 0.0 {
                viewmodel_particles.push(Particle {
                    position_m: muzzle_m,
                    stretch_m: Vec3::ZERO,
                    size_m: 0.06 + flash_frac * 0.05,
                    color_rgba: (self.player_beam_color * scene.light_scale * flash_frac * 1.5)
                        .extend(0.0),
                    kind: ParticleKind::MuzzleFlash,
                    seed: self.weapon.since_shot_s,
                });
            }
            if viewmodel.is_energy_weapon {
                let charge_frac: f32 = self.weapon.ammo as f32 / MAGAZINE_SIZE as f32;
                let pulse: f32 = 0.75 + 0.25 * (self.time_s * 3.0).sin();
                viewmodel_particles.push(Particle {
                    position_m: muzzle_m,
                    stretch_m: Vec3::ZERO,
                    size_m: 0.012,
                    color_rgba: (self.player_beam_color
                        * scene.light_scale
                        * 0.12
                        * pulse
                        * charge_frac)
                        .extend(0.0),
                    kind: ParticleKind::MuzzleFlash,
                    seed: 0.5,
                });
            }
        }

        let speed_frac: f32 = (self.player.velocity_m_s.with_y(0.0).length() / 7.0).min(1.0);
        let mut hud_rects: Vec<HudRect> = build_hud(
            &HudState {
                health_frac: self.player.health / MAX_HEALTH,
                ammo: self.weapon.ammo,
                magazine_size: MAGAZINE_SIZE,
                reload_frac: self.weapon.reload_time_s.map(|_| self.weapon.reload_frac()),
                spread_frac: (speed_frac + self.weapon.kick_frac * 0.6).min(1.0),
                ads_frac: self.weapon.ads_frac,
                hit_marker_frac: (self.hit_marker_s / 0.25).min(1.0),
                kill_marker_frac: self.kill_marker_s,
                kill_count: self.kill_count,
                is_dead: !self.player.is_alive(),
                frame_ms: self.show_frame_ms,
            },
            width_px,
            height_px,
        );
        hud_rects.extend(self.objective_hud(width_px, height_px));
        FrameData {
            camera,
            world_draws,
            viewmodel_draws,
            joint_matrices,
            point_lights,
            decals: self.decals.iter().copied().collect(),
            world_particles,
            viewmodel_particles,
            hud_rects,
            damage_flash_frac: self.damage_flash_frac,
        }
    }
}
