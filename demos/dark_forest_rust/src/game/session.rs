//! `session` runs a match: it updates the player, their guns (`guns.rs`, Lunar Surface's) and
//! the bots, resolves shots, spawns effects (beams flying out, sparks, smoke, ground dust, decals,
//! light flashes), plays sounds, and turns the state into the renderer's per-frame data. An optional autopilot plays the game by itself for
//! screenshots and benchmarks.
//!
//! Example:
//!     session.update(&controls, &scene, &audio, dt_s);
//!     let frame: FrameData = session.frame_data(&scene, width_px, height_px);

use crate::game::bots::BODY_CENTRE_HEIGHT_M;
use crate::game::warps::{BeamPath, Warps};
use std::collections::VecDeque;
use std::ops::Range;

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use super::audio::{Audio, Sound};
use super::bots::{
    BOT_HEIGHT_M, BOT_MAX_HEALTH, Bot, BotClip, BotContext, BotShot, RESPAWN_DELAY_S,
};
use super::collision::{Aabb, cast_ray};
use super::console_bot::{BotWalker, WalkStep};
use super::dust::{DustBurst, DustParticle, DustShape, MAX_DUST_BURST_COUNT};
use super::guns::{
    Gun, GunInput, GunStep, Guns, HALF_FOV_RAD, LASER_MAX_LENGTH_M, MUZZLE_M, Tracer,
    take_hit_weight,
};
use super::hud::{BarPulsePair, HudState, OverlayHudState, build_hud, build_overlay_rects};
use super::nav::NavGraph;
use super::player::{MAX_HEALTH, Movement, Player, PlayerInput};
use super::world::{Arena, Surface};
use crate::assets::gltf_model::Trs;
use crate::console_reach::ConsolePlace;
use crate::render::overlay::{HitEffect, OverlayRect};
use crate::render::{Camera, Decal, DrawItem, HudRect, Particle, ParticleKind, PointLight};
use crate::scene::{GunViewmodel, Scene, draw_items_for};
use crate::settings::ServerSettings;
use crate::shot_noise::ShotNoise;

/// `MAX_DECAL_COUNT` is how many impact marks stay in the world.
const MAX_DECAL_COUNT: usize = 160;
/// `TEAM_PLAYER_BEAM_COLOR` is the HDR color of the player's beam (times the light scale).
const TEAM_PLAYER_BEAM_COLOR: Vec3 = Vec3::new(0.35, 0.8, 1.6);
/// `MAP_BEAM_HDR_SCALE` brightens a map's laser colour (`Arena::laser_rgb`) to a beam's HDR level.
const MAP_BEAM_HDR_SCALE: f32 = 1.8;
/// `TEAM_BOT_BEAM_COLOR` is the HDR color of the bots' bolts (times the light scale).
const TEAM_BOT_BEAM_COLOR: Vec3 = Vec3::new(1.8, 0.25, 0.12);
/// `AUTOPILOT_TURN_PER_S` is how fast the autopilot turns toward what it looks at (the share of
/// the way left it turns each second, as an exponential rate): fast enough to follow a sprinter.
const AUTOPILOT_TURN_PER_S: f32 = 15.0;
/// `AUTOPILOT_AIM_SLACK_M` is how near the target a shot aimed straight at it must still end,
/// after any wormhole's bending, for the autopilot to count the target as in sight.
const AUTOPILOT_AIM_SLACK_M: f32 = 0.3;
/// `AUTOPILOT_HIP_FIRE_RANGE_M` is the range inside which the autopilot fires from the hip.
const AUTOPILOT_HIP_FIRE_RANGE_M: f32 = 14.0;
/// `NEAR_M` is the camera's near plane.
const NEAR_M: f32 = 0.05;
/// `FAR_M` is the camera's far plane.
const FAR_M: f32 = 600.0;
/// `BEAM_RADIUS_M` is a laser beam's half width (Lunar Surface's narrowest), and `BEAM_HDR_SCALE`
/// brightens its colour to a beam's HDR level, times the light scale: the beams are drawn as this
/// level's tracers (with the shot's speckle share in the particle's seed) until the renderer draws
/// Lunar Surface's beam looks and speckle.
const BEAM_RADIUS_M: f32 = 0.02;
const BEAM_HDR_SCALE: f32 = 1.8;
/// `BEAM_DASH_SPACING_M` and `BEAM_DASH_OVERLAP` cut a beam into short tracers, each this many times
/// as long as their spacing: one tracer glows only around its middle, so a long beam drawn as one
/// would show only there.
const BEAM_DASH_SPACING_M: f32 = 0.15;
const BEAM_DASH_OVERLAP: f32 = 2.0;
/// `FLASHLIGHT_RGB` is the flashlight's colour, a warm white; `FLASHLIGHT_REACH_PER_RANGE` how far
/// it reaches, in its half-light ranges; `FLASHLIGHT_SPOT_STANDOFF_M` and
/// `FLASHLIGHT_SPOT_MARGIN_M` how far before the lit surface its spot light sits and how much wider
/// than the cone it reaches; and `FLASHLIGHT_NEAR_M`, `FLASHLIGHT_NEAR_RADIUS_M` and
/// `FLASHLIGHT_NEAR_FRAC` place, size and dim the light for what is close.
const FLASHLIGHT_RGB: Vec3 = Vec3::new(1.0, 0.95, 0.85);
const FLASHLIGHT_REACH_PER_RANGE: f32 = 2.0;
const FLASHLIGHT_SPOT_STANDOFF_M: f32 = 0.5;
const FLASHLIGHT_SPOT_MARGIN_M: f32 = 0.5;
const FLASHLIGHT_NEAR_M: f32 = 1.0;
const FLASHLIGHT_NEAR_RADIUS_M: f32 = 4.0;
const FLASHLIGHT_NEAR_FRAC: f32 = 0.15;
/// `GRAIN_SPRITE_PER_RADIUS` is how much larger than a dust grain its smoke sprite is.
const GRAIN_SPRITE_PER_RADIUS: f32 = 1.5;
/// `LUNAR_TEAM_BEAM_RGB` are Lunar Surface's laser colours per team (blue, red), linear, for a map
/// without a laser colour of its own.
const LUNAR_TEAM_BEAM_RGB: [Vec3; 2] = [
    Vec3::new(0.0707, 0.3491, 1.0),
    Vec3::new(1.0, 0.0177, 0.0039),
];
/// `MIN_GROUND_NORMAL_Y` is how upright a ground hit's normal must be to throw up dust, and
/// `BOT_GROUND_HIT_M` how low a bot's bolt must land to count as hitting the ground.
const MIN_GROUND_NORMAL_Y: f32 = 0.7;
const BOT_GROUND_HIT_M: f32 = 0.05;

/// `Controls` is one frame of input from the keyboard and mouse.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub player: PlayerInput,
    pub gun: GunInput,
    pub is_flashlight_held: bool,
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

/// `Landing` is what a shot does where its beam ends, once the beam's front gets there: sparks,
/// smoke and a light flash in `color_rgb` (an HDR multiple of the light scale) at `position_m` on a
/// surface facing `normal`, a scorch mark when `has_decal`, and ground dust thrown up against
/// `direction` (the shot's) when it hit the ground.
#[derive(Clone, Copy, Debug)]
struct Landing {
    position_m: Vec3,
    normal: Vec3,
    color_rgb: Vec3,
    has_decal: bool,
    is_ground: bool,
    direction: Vec3,
}

/// `ShotFlight` is a shot's beam flying out (`tracer`) and what it does when its front lands,
/// until it does.
struct ShotFlight {
    tracer: Tracer,
    landing: Option<Landing>,
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

/// `FrameData` owns everything a frame description points at; `lens_draws` are the robots among
/// `world_draws`, and `lens_particles` the robots' eye glows, lasers and their sparks and smoke.
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
    pub overlay_rects: Vec<OverlayRect>,
    pub hit_effect: HitEffect,
    pub lens_draws: Range<usize>,
    pub lens_particles: Vec<Particle>,
}

/// `OFF_PYLON_TINT` darkens a pylon whose console is off.
const OFF_PYLON_TINT: Vec3 = Vec3::new(0.3, 0.3, 0.3);
/// `BEACON_BEAM_M`, `BEACON_BEAM_WIDTH_M` and `BEACON_BEAM_RGB` are the beam a pylon sends up
/// while its console is on, in its team's colour (blue, red), as in the hyperbolic demo.
const BEACON_BEAM_M: f32 = 25.0;
const BEACON_BEAM_WIDTH_M: f32 = 0.12;
const BEACON_BEAM_RGB: [Vec3; 2] = [Vec3::new(0.3, 0.6, 2.0), Vec3::new(2.0, 0.35, 0.2)];
/// `HIT_EFFECT_S`, `HIT_RED_FRAC`, `HIT_SHAKE_PX` and `HIT_BLUR_PX` are the Lunar Surface demo's
/// `hit_effects` player settings: seconds the red, shake and blur of a hit take to fade, how red the
/// screen's edges turn, and how far the screen shakes and the edges blur, in window pixels.
const HIT_EFFECT_S: f32 = 0.6;
const HIT_RED_FRAC: f32 = 0.6;
const HIT_SHAKE_PX: f32 = 6.0;
const HIT_BLUR_PX: f32 = 6.0;
/// `HIT_SHAKE_TURN_RAD` is the golden angle, how far the shake's direction turns each frame (the
/// Lunar Surface demo's `SPAWN_TURN_RAD`).
const HIT_SHAKE_TURN_RAD: f64 = 2.399_963_229_728_653;
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
    pub guns: Guns,
    pub settings: ServerSettings,
    pub movement: Movement,
    since_movement_key_s: f32,
    pub bots: Vec<Bot>,
    pub kill_count: u32,
    pub death_count: u32,
    pub time_s: f32,
    pub is_autopilot: bool,
    pub show_frame_ms: Option<f32>,
    decals: VecDeque<Decal>,
    particles: Vec<SimParticle>,
    flights: Vec<ShotFlight>,
    dust_bursts: Vec<DustBurst>,
    was_firing: bool,
    flashes: Vec<Flash>,
    hit_marker_s: f32,
    kill_marker_s: f32,
    hit_effect_frac: f32,
    last_health: f32,
    frame_count: u64,
    spawn_counter: usize,
    noise_state: u32,
    pub autopilot_waypoint: usize,
    autopilot_trigger_s: f32,
    pub route: Vec<Vec3>,
    pub remote_bots: Vec<RemoteBot>,
    pub pending_hits: Vec<(u32, u8)>,
    pub team: u32,
    pub console_off: Vec<bool>,
    pub player_beam_color: Vec3,
    pub bot_beam_color: Vec3,
    pub console_hold_frac: Option<f32>,
    pub winner: Option<u32>,
    pub laps_done: u32,
    pub is_trigger_down: bool,
    pub is_view_covered: bool,
    pub console_places: Vec<ConsolePlace>,
    pub walker: Option<BotWalker>,
    pub is_console_objective: bool,
    pub bot_console: Option<usize>,
    pub is_team_known: bool,
    pub is_flashlight_on: bool,
    bar_pulses: BarPulsePair,
    robot_hold: Option<RobotHold>,
    robot_hold_s: f32,
}

/// `RemoteBot` is another player of a network game, drawn as a robot in their team's colour: their
/// camera's frame (while firing, the frame their shot is traced from), whether their laser is on
/// (and was last frame), with which gun, and their shot's speckle.
pub struct RemoteBot {
    pub id: u32,
    pub team: u32,
    pub bot: Bot,
    pub frame: Mat4,
    pub is_firing: bool,
    pub was_firing: bool,
    pub is_rifle: bool,
    pub beam_speckle_frac: f64,
}

/// `TEAM_TINTS` are the robots' main colours per team (blue, red).
const TEAM_TINTS: [Vec3; 2] = [Vec3::new(0.12, 0.25, 0.75), Vec3::new(0.75, 0.14, 0.1)];

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
        let movement: Movement = Movement::New(
            &arena.map_player,
            arena.map_gravity_m_per_s2,
            &ServerSettings::Builtin(),
        );
        Session {
            player: Player::new(spawn_m, yaw_rad),
            arena,
            nav,
            guns: Guns::New(ShotNoise::FromClock()),
            movement,
            since_movement_key_s: 0.0,
            settings: ServerSettings::Builtin(),
            bots,
            kill_count: 0,
            death_count: 0,
            time_s: 0.0,
            is_autopilot,
            show_frame_ms: None,
            decals: VecDeque::new(),
            particles: Vec::new(),
            flights: Vec::new(),
            dust_bursts: Vec::new(),
            was_firing: false,
            flashes: Vec::new(),
            hit_marker_s: 0.0,
            kill_marker_s: 0.0,
            hit_effect_frac: 0.0,
            last_health: MAX_HEALTH,
            frame_count: 0,
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
            is_view_covered: false,
            console_places: Vec::new(),
            walker: None,
            is_console_objective: false,
            bot_console: None,
            is_team_known: true,
            is_flashlight_on: false,
            bar_pulses: BarPulsePair::New(),
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

    /// `set_settings` makes `settings` the match's (the game server's, once it welcomes this
    /// window) and works out the player's movement from them.
    ///
    /// Example: `session.set_settings(settings.clone());`
    pub fn set_settings(&mut self, settings: ServerSettings) {
        self.movement = Movement::New(
            &self.arena.map_player,
            self.arena.map_gravity_m_per_s2,
            &settings,
        );
        self.settings = settings;
    }

    /// `respawn_player` brings the player back, whole, at their team's spawn farthest from the
    /// robots (`farthest_spawn`): after the respawn delay, and when they join
    /// a team.
    ///
    /// Example: `session.respawn_player();`
    pub fn respawn_player(&mut self) {
        let bot_positions: Vec<Vec3> = self.bots.iter().map(|bot| bot.feet_m).collect();
        let team_spawns: &[(Vec3, f32)] = &self.arena.team_spawns[self.team as usize % 2];
        let spawns: Vec<Vec3> = team_spawns.iter().map(|(p, _)| *p).collect();
        let spawn_m: Vec3 = Session::farthest_spawn(&spawns, &bot_positions, self.spawn_counter);
        let yaw_rad: f32 = team_spawns
            .iter()
            .find(|(p, _)| *p == spawn_m)
            .map_or(0.0, |(_, yaw)| *yaw);
        self.spawn_counter += 1;
        self.player = Player::new(spawn_m, yaw_rad);
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

    /// `muzzle_world_m` gives where the tip of the gun in hand shows in the world
    /// (`Guns::get_muzzle_m`), where this player's beams start.
    fn muzzle_world_m(&self) -> Vec3 {
        let zoom: f64 = self.guns.get_zoom(&self.settings);
        self.player
            .camera_world()
            .transform_point3(self.guns.get_muzzle_m(zoom))
    }

    /// `get_beam_rgb` is the linear colour of `team`'s lasers: the map's, or Lunar Surface's team
    /// colours without one.
    fn get_beam_rgb(&self, team: u32) -> Vec3 {
        self.arena
            .laser_rgb
            .unwrap_or(LUNAR_TEAM_BEAM_RGB[team as usize % 2])
    }

    /// `add_flight` starts a beam flying out along `path` (`Tracer::New`), landing as `landing`
    /// when its front arrives.
    #[allow(clippy::too_many_arguments)]
    fn add_flight(
        &mut self,
        path: &BeamPath,
        landing: Option<Landing>,
        muzzle_m: Vec3,
        is_rifle: bool,
        speckle_frac: f64,
        team: u32,
        is_own: bool,
    ) {
        let tracer: Tracer = Tracer {
            is_own,
            ..Tracer::New(
                &path.pieces,
                landing.is_some(),
                muzzle_m,
                f64::from(self.time_s),
                is_rifle,
                speckle_frac,
                self.get_beam_rgb(team),
                &self.settings,
            )
        };
        self.flights.push(ShotFlight { tracer, landing });
    }

    /// `land_shot` makes `landing` happen: sparks, smoke, a scorch mark and a flash
    /// (`add_impact`), and ground dust when the shot hit the ground (`DustBurst::FromShot`).
    fn land_shot(&mut self, landing: Landing, is_rifle: bool, light_scale: f32) {
        self.add_impact(
            landing.position_m,
            landing.normal,
            landing.color_rgb,
            light_scale,
            landing.has_decal,
        );
        if landing.is_ground {
            self.dust_bursts.push(DustBurst::FromShot(
                landing.position_m.as_dvec3(),
                landing.direction.as_dvec3(),
                f64::from(self.time_s),
                is_rifle,
            ));
            let excess_count: usize = self.dust_bursts.len().saturating_sub(MAX_DUST_BURST_COUNT);
            self.dust_bursts.drain(..excess_count);
        }
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

    /// `shoot_player_laser` traces this frame's laser while it is on, as Lunar Surface does: from
    /// the eye turned by the shot's stray, through the warps, to the world, a bot or another player.
    /// A hit on a player or bot counts as `take_hit_weight` gives it (every frame for the sniper,
    /// once a shot for the rifle); a bot loses that share of its health. The laser's first frame
    /// starts its beam flying out from the gun in hand, landing where the shot ends.
    fn shoot_player_laser(&mut self, scene: &Scene, audio: &Audio) {
        /// `ShotHit` is what a laser hit first.
        #[derive(Clone, Copy)]
        enum ShotHit {
            World(usize, Vec3),
            Bot(usize, bool, Vec3),
            Remote(u32, bool, Vec3),
        }
        let aim: Mat4 = self
            .guns
            .shot_offset
            .aim(self.player.camera_world().as_dmat4())
            .as_mat4();
        let origin_m: Vec3 = aim.w_axis.truncate();
        let direction: Vec3 = -aim.z_axis.truncate().normalize();
        let mut shot_hit: Option<ShotHit> = None;
        let path: BeamPath = self.arena.warps.trace_beam(
            origin_m,
            direction,
            LASER_MAX_LENGTH_M,
            |start_m: Vec3, piece_direction: Vec3, length_m: f32| {
                let mut nearest_m: f32 = length_m;
                let mut found: Option<ShotHit> = None;
                if let Some((index, hit)) =
                    cast_ray(&scene.solids, start_m, piece_direction, nearest_m)
                {
                    nearest_m = hit.distance_m;
                    found = Some(ShotHit::World(index, hit.normal));
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
                    for (aabb, is_head) in [(remote.bot.head(), true), (remote.bot.body(), false)] {
                        if let Some(hit) = aabb.cast_ray(start_m, piece_direction, nearest_m) {
                            nearest_m = hit.distance_m;
                            found = Some(ShotHit::Remote(remote.id, is_head, hit.normal));
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
        let gun: Gun = self.guns.gun;
        let is_head_hit: bool = matches!(
            shot_hit,
            Some(ShotHit::Bot(_, true, _) | ShotHit::Remote(_, true, _))
        );
        let hit_weight: Option<u8> =
            matches!(shot_hit, Some(ShotHit::Bot(..) | ShotHit::Remote(..)))
                .then(|| {
                    take_hit_weight(
                        &self.settings,
                        gun,
                        &mut self.guns.has_rifle_shot_hit,
                        is_head_hit,
                    )
                })
                .flatten();
        match (shot_hit, hit_weight) {
            (Some(ShotHit::Remote(id, _, _)), Some(hits_to_kill)) => {
                self.pending_hits.push((id, hits_to_kill));
                self.hit_marker_s = 0.25;
                audio.play(Sound::HitMarker, 0.5, 0.0);
            }
            (Some(ShotHit::Bot(index, _, _)), Some(hits_to_kill)) => {
                let did_kill: bool =
                    self.bots[index].take_damage(BOT_MAX_HEALTH / f32::from(hits_to_kill));
                self.hit_marker_s = 0.25;
                audio.play(Sound::HitMarker, 0.5, 0.0);
                if did_kill {
                    self.kill_count += 1;
                    self.kill_marker_s = 0.4;
                    let (gain, pan) = self.sound_placement(end_m);
                    audio.play(Sound::BotDeath, gain * 1.4, pan);
                }
            }
            _ => {}
        }
        let is_first_frame: bool = !self.was_firing;
        if !is_first_frame {
            return;
        }
        let landing: Option<Landing> = shot_hit.map(|hit| {
            let (normal, has_decal, is_ground) = match hit {
                ShotHit::World(index, normal) => (
                    normal,
                    true,
                    self.arena.boxes[index].surface == Surface::Ground
                        && normal.y > MIN_GROUND_NORMAL_Y,
                ),
                ShotHit::Bot(_, _, normal) | ShotHit::Remote(_, _, normal) => {
                    (normal, false, false)
                }
            };
            Landing {
                position_m: end_m,
                normal,
                color_rgb: self.player_beam_color,
                has_decal,
                is_ground,
                direction,
            }
        });
        let muzzle_m: Vec3 = self.muzzle_world_m();
        self.add_flight(
            &path,
            landing,
            muzzle_m,
            gun == Gun::Rifle,
            self.guns.beam_speckle_frac,
            self.team,
            true,
        );
        self.flashes.push(Flash {
            position_m: muzzle_m,
            color_rgb: self.player_beam_color * scene.light_scale * 0.5,
            radius_m: 7.0,
            age_s: 0.0,
            life_s: 0.07,
        });
        audio.play(Sound::Shot, 0.9, 0.0);
    }

    /// `get_hits_to_eliminate` gives how many hits like this one (to the head when `is_head`)
    /// eliminate a player or a bot: the server settings' rifle hits, as on Lunar Surface.
    ///
    /// Example: `let hits_to_kill: u8 = self.get_hits_to_eliminate(false);`
    fn get_hits_to_eliminate(&self, is_head: bool) -> u8 {
        if is_head {
            self.settings.rifle.head_hits_to_eliminate
        } else {
            self.settings.rifle.body_hits_to_eliminate
        }
    }

    /// `apply_bot_shot` sends a bot's bolt flying out as a rifle shot's beam (landing where it
    /// ends unless it hit the player) and applies its damage.
    fn apply_bot_shot(&mut self, shot: BotShot, scene: &Scene, audio: &Audio) {
        let path: BeamPath = BeamPath {
            pieces: vec![(shot.origin_m, shot.end_m)],
            end_m: shot.end_m,
        };
        let landing: Option<Landing> = (!shot.did_hit_player).then_some(Landing {
            position_m: shot.end_m,
            normal: shot.end_normal,
            color_rgb: self.bot_beam_color,
            has_decal: true,
            is_ground: shot.end_m.y < BOT_GROUND_HIT_M && shot.end_normal.y > MIN_GROUND_NORMAL_Y,
            direction: (shot.end_m - shot.origin_m).normalize_or(Vec3::NEG_Y),
        });
        self.add_flight(
            &path,
            landing,
            shot.origin_m,
            true,
            0.0,
            self.team + 1,
            false,
        );
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
            let body_hits: f32 = f32::from(self.get_hits_to_eliminate(false));
            self.player.take_damage(MAX_HEALTH / body_hits);
            audio.play(Sound::Hurt, 0.8, pan);
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

    /// `autopilot_controls` plays the game: walk to the other team's consoles and hold `E` on each
    /// (`is_console_objective`, with `walker`); or aim at visible robots and fire, walk toward the
    /// nearest enemy player when none is visible (with `walker`), and else walk the waypoint loop.
    /// A robot or player counts as in sight only along a straight line clear of the forest that a
    /// shot would follow to it (`AUTOPILOT_AIM_SLACK_M`), not bent away by a wormhole. It stands still while it has one in sight, since a shot fires only
    /// `shoot_after_moving_s` after the last step. It waits while its team is not known yet (`is_team_known`: an online game before the
    /// server's welcome).
    fn autopilot_controls(&mut self, scene: &Scene, dt_s: f32) -> Controls {
        if !self.is_team_known {
            return Controls::default();
        }
        if self.is_console_objective
            && let Some(walker) = &mut self.walker
        {
            let walk: WalkStep = walker.step_consoles(
                &self.player,
                &self.nav,
                &self.arena.ladders,
                &self.console_places,
                &self.console_off,
                self.team,
                self.settings.consoles.reach_m,
                self.time_s,
                dt_s,
            );
            self.bot_console = walk.console;
            return Controls {
                player: walk.input,
                gun: GunInput::default(),
                is_flashlight_held: false,
            };
        }
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
                    && self
                        .arena
                        .warps
                        .trace_beam(eye_m, to_bot, to_bot.length(), |_, _, _| None)
                        .end_m
                        .distance(*aim_m)
                        < AUTOPILOT_AIM_SLACK_M
            })
            .min_by(|a, b| a.distance(eye_m).total_cmp(&b.distance(eye_m)));
        let feet_m: Vec3 = self.player.feet_m;
        let hunted_m: Option<Vec3> = self
            .remote_bots
            .iter()
            .filter(|remote| remote.team != my_team && remote.bot.is_alive())
            .map(|remote| remote.bot.feet_m)
            .min_by(|a, b| a.distance(feet_m).total_cmp(&b.distance(feet_m)));
        if visible_bot.is_none()
            && let (Some(goal_m), Some(walker)) = (hunted_m, &mut self.walker)
        {
            let player: PlayerInput = walker.walk_to(
                &self.player,
                &self.nav,
                &self.arena.ladders,
                goal_m,
                0.0,
                self.time_s,
                dt_s,
            );
            self.autopilot_trigger_s = 0.0;
            return Controls {
                player,
                gun: GunInput::default(),
                is_flashlight_held: false,
            };
        }
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
        let turn_frac: f32 = 1.0 - (-AUTOPILOT_TURN_PER_S * dt_s).exp();
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
                    * if visible_bot.is_some() { 0.0 } else { 1.0 },
                look_delta_rad,
                is_sprint_held: visible_bot.is_none(),
                is_crouch_held: false,
                is_jump_pressed: false,
            },
            gun: GunInput {
                is_trigger_held: self.autopilot_trigger_s > 0.25,
                is_shoot_pressed: self.autopilot_trigger_s > 0.25,
                is_aim_held: should_aim,
                ..GunInput::default()
            },
            is_flashlight_held: false,
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
        self.frame_count += 1;
        if self.player.health < self.last_health {
            self.hit_effect_frac = 1.0;
            self.player.slow_down(&self.movement);
        }
        let is_walking_upright: bool =
            controls.player.move_axes != Vec2::ZERO && !controls.player.is_crouch_held;
        self.since_movement_key_s = if is_walking_upright || controls.player.is_jump_pressed {
            0.0
        } else {
            self.since_movement_key_s + dt_s
        };
        self.last_health = self.player.health;
        let bar_state: OverlayHudState = self.overlay_hud_state();
        self.bar_pulses.update(dt_s, &bar_state);
        let mut player_input: PlayerInput = controls.player;
        if !self.is_autopilot {
            player_input.look_delta_rad /= self.guns.get_zoom(&self.settings) as f32;
        }
        self.player.update(
            &player_input,
            &self.movement,
            &scene.solids,
            &self.arena.ladders,
            dt_s,
        );
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
        let can_shoot: bool = self.player.is_alive()
            && self.console_hold_frac.is_none()
            && self.winner.is_none()
            && f64::from(self.since_movement_key_s) >= self.settings.movement.shoot_after_moving_s;
        let step: GunStep = GunStep {
            is_sprinting: self.player.is_sprinting,
            bob_phase_rad: self.player.bob_phase_rad,
            bob_frac: self.player.bob_frac,
        };
        let mut gun_input: GunInput = controls.gun;
        gun_input.is_aim_held &= self.player.is_alive();
        let kick_rad: Vec2 = self.guns.update(
            &gun_input,
            &self.settings,
            can_shoot,
            &step,
            f64::from(dt_s),
        );
        self.player.kick_view(kick_rad);
        let is_firing: bool = self.guns.is_firing();
        if is_firing {
            self.shoot_player_laser(scene, audio);
        }
        self.was_firing = is_firing;
        self.is_flashlight_on = controls.is_flashlight_held && self.player.is_alive();
        self.is_trigger_down = is_firing;
        if self.player.should_respawn() {
            self.death_count += 1;
            self.respawn_player();
        }

        let context: BotContext = BotContext {
            solids: &scene.solids,
            nav: &self.nav,
            ladders: &self.arena.ladders,
            player_eye_m: self.player.eye_position_m(),
            player_velocity_m_s: self.player.velocity_m_s,
            is_player_alive: self.player.is_alive(),
            is_player_crouched: self.player.crouch_frac > 0.5,
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

        let starting: Vec<(Mat4, bool, f64, u32)> = self
            .remote_bots
            .iter()
            .filter(|remote| remote.is_firing && !remote.was_firing && remote.bot.is_alive())
            .map(|remote| {
                (
                    remote.frame,
                    remote.is_rifle,
                    remote.beam_speckle_frac,
                    remote.team,
                )
            })
            .collect();
        for remote in &mut self.remote_bots {
            remote.bot.animation.advance(dt_s, 1.0);
            remote.was_firing = remote.is_firing;
        }
        for (frame, is_rifle, speckle_frac, team) in starting {
            self.add_remote_flight(frame, is_rifle, speckle_frac, team, scene);
        }
        let now_s: f64 = f64::from(self.time_s);
        let landed: Vec<(Landing, bool)> = self
            .flights
            .iter_mut()
            .filter(|flight| now_s >= flight.tracer.get_arrival_s())
            .filter_map(|flight| Some((flight.landing.take()?, flight.tracer.is_rifle)))
            .collect();
        for (landing, is_rifle) in landed {
            self.land_shot(landing, is_rifle, scene.light_scale);
        }
        self.flights
            .retain(|flight| !flight.tracer.is_over(now_s) || flight.landing.is_some());
        self.dust_bursts.retain(|burst| !burst.is_over(now_s));
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
        for flash in &mut self.flashes {
            flash.age_s += dt_s;
        }
        self.flashes.retain(|flash| flash.age_s < flash.life_s);
        self.hit_marker_s = (self.hit_marker_s - dt_s).max(0.0);
        self.kill_marker_s = (self.kill_marker_s - dt_s).max(0.0);
        self.hit_effect_frac = (self.hit_effect_frac - dt_s / HIT_EFFECT_S).max(0.0);
    }

    /// `add_remote_flight` starts the beam of another player's shot as Lunar Surface draws it: traced
    /// from `frame` (their camera turned by their shot's stray) through the warps to the world, from
    /// their muzzle (`MUZZLE_M` in their camera), in their gun's look (`is_rifle`) and speckle.
    fn add_remote_flight(
        &mut self,
        frame: Mat4,
        is_rifle: bool,
        speckle_frac: f64,
        team: u32,
        scene: &Scene,
    ) {
        let origin_m: Vec3 = frame.w_axis.truncate();
        let direction: Vec3 = -frame.z_axis.truncate().normalize();
        let mut hit_box: Option<(usize, Vec3)> = None;
        let path: BeamPath = self.arena.warps.trace_beam(
            origin_m,
            direction,
            LASER_MAX_LENGTH_M,
            |start_m: Vec3, piece_direction: Vec3, length_m: f32| {
                cast_ray(&scene.solids, start_m, piece_direction, length_m).map(|(index, hit)| {
                    hit_box = Some((index, hit.normal));
                    hit.distance_m
                })
            },
        );
        let landing: Option<Landing> = hit_box.map(|(index, normal)| Landing {
            position_m: path.end_m,
            normal,
            color_rgb: self.bot_beam_color,
            has_decal: true,
            is_ground: self.arena.boxes[index].surface == Surface::Ground
                && normal.y > MIN_GROUND_NORMAL_Y,
            direction,
        });
        self.add_flight(
            &path,
            landing,
            frame.transform_point3(MUZZLE_M),
            is_rifle,
            speckle_frac,
            team,
            false,
        );
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

    /// `get_flashlight_lights` lights what the player looks at as Lunar Surface's flashlight does,
    /// with point lights since this renderer has no spot lights: one where the view meets the
    /// world (up to `FLASHLIGHT_REACH_PER_RANGE` times the settings' `range_m`), as wide as the
    /// cone there, as bright as the settings' share of sunlight, halved at `range_m`; and a dim one
    /// just ahead of the eye for what is close.
    ///
    /// Example: `point_lights.extend(self.get_flashlight_lights(scene));`
    fn get_flashlight_lights(&self, scene: &Scene) -> [PointLight; 2] {
        let flashlight = &self.settings.flashlight;
        let eye_m: Vec3 = self.player.eye_position_m();
        let ahead: Vec3 = self.player.rotation() * Vec3::NEG_Z;
        let range_m: f32 = flashlight.range_m as f32;
        let reach_m: f32 = range_m * FLASHLIGHT_REACH_PER_RANGE;
        let distance_m: f32 = cast_ray(&scene.solids, eye_m, ahead, reach_m)
            .map_or(reach_m, |(_, hit)| hit.distance_m);
        let cone_rad: f32 = (flashlight.cone_half_angle_rad + flashlight.soft_edge_rad) as f32;
        let spot_radius_m: f32 = distance_m * cone_rad.tan() + FLASHLIGHT_SPOT_MARGIN_M;
        let falloff: f32 = 1.0 / (1.0 + (distance_m / range_m).powi(2));
        let color_rgb: Vec3 =
            FLASHLIGHT_RGB * scene.light_scale * flashlight.brightness_frac as f32;
        [
            PointLight {
                position_m: eye_m + ahead * (distance_m - FLASHLIGHT_SPOT_STANDOFF_M).max(0.0),
                radius_m: spot_radius_m + FLASHLIGHT_SPOT_STANDOFF_M,
                color_rgb: color_rgb * falloff,
            },
            PointLight {
                position_m: eye_m + ahead * FLASHLIGHT_NEAR_M,
                radius_m: FLASHLIGHT_NEAR_RADIUS_M,
                color_rgb: color_rgb * FLASHLIGHT_NEAR_FRAC,
            },
        ]
    }

    /// `get_hit_effect` is what the last hit on the player still does to the view, as the Lunar
    /// Surface demo's `get_hit_effect` gives it: red edges, a blur from the edges inward, and a
    /// shake in a new direction each frame (the golden angle apart), fading over `HIT_EFFECT_S`.
    fn get_hit_effect(&self) -> HitEffect {
        let shake_rad: f64 = self.frame_count as f64 * HIT_SHAKE_TURN_RAD;
        let shake_px: f32 = HIT_SHAKE_PX * self.hit_effect_frac;
        HitEffect {
            strength_frac: self.hit_effect_frac,
            red_frac: HIT_RED_FRAC,
            blur_px: HIT_BLUR_PX,
            shake_px: [
                shake_rad.cos() as f32 * shake_px,
                shake_rad.sin() as f32 * shake_px,
            ],
        }
    }

    /// `overlay_hud_state` is what the HUD's bars show now, as Lunar Surface's: health in segments
    /// of one rifle hit to the body; the sniper's one block while it is loaded, or the rifle's heat
    /// in segments of one shot (red once overheated); the sprint energy, the console hold and
    /// whether the menu covers the view.
    fn overlay_hud_state(&self) -> OverlayHudState {
        let (ammo_frac, ammo_segment_count) = match self.guns.gun {
            Gun::Laser => (f32::from(u8::from(self.guns.sniper_reload_s <= 0.0)), 1),
            Gun::Rifle => (
                1.0 - self.guns.rifle_heat_frac.min(1.0) as f32,
                self.settings.rifle.shots_until_overheat,
            ),
        };
        OverlayHudState {
            health_frac: self.player.health / MAX_HEALTH,
            health_segment_count: u32::from(self.settings.rifle.body_hits_to_eliminate),
            ammo_frac,
            ammo_segment_count,
            is_overheated: self.guns.gun == Gun::Rifle && self.guns.is_overheated,
            energy_frac: self.player.energy_frac,
            console_hold_frac: self.console_hold_frac,
            is_view_covered: self.is_view_covered,
        }
    }

    /// `objective_hud` draws the consoles (lit while on, in their team's colour) and the winner's
    /// banner; the progress of holding `E` on a console is an overlay bar (`hud::build_overlay_rects`).
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
        let zoom: f32 = self.guns.get_zoom(&self.settings) as f32;
        let camera: Camera = Camera {
            view: camera_world.inverse(),
            position_m: self.player.eye_position_m(),
            fov_y_rad: 2.0 * (HALF_FOV_RAD.tan() / zoom).atan(),
            viewmodel_fov_y_rad: 2.0 * HALF_FOV_RAD,
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
        if self.is_flashlight_on {
            point_lights.extend(self.get_flashlight_lights(scene));
        }
        let lens_draws: Range<usize> = scene.static_draws.len()..world_draws.len();
        let robot_particle_count: usize = world_particles.len();
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
        let beacon_particle_end: usize = world_particles.len();
        let now_s: f64 = f64::from(self.time_s);
        let muzzle_m: Vec3 = self.muzzle_world_m();
        for flight in &self.flights {
            let tracer: &Tracer = &flight.tracer;
            for (start_m, end_m) in tracer.get_lit_pieces(now_s, muzzle_m) {
                let length_m: f32 = start_m.distance(end_m);
                let dash_count: usize = (length_m / BEAM_DASH_SPACING_M).ceil().max(1.0) as usize;
                let step_m: Vec3 = (end_m - start_m) / dash_count as f32;
                for dash in 0..dash_count {
                    world_particles.push(Particle {
                        position_m: start_m + step_m * (dash as f32 + 0.5),
                        stretch_m: step_m * BEAM_DASH_OVERLAP,
                        size_m: BEAM_RADIUS_M,
                        color_rgba: (tracer.rgb * BEAM_HDR_SCALE * scene.light_scale).extend(0.0),
                        kind: ParticleKind::Tracer,
                        seed: tracer.speckle_frac,
                    });
                }
            }
        }
        for dust in self
            .dust_bursts
            .iter()
            .flat_map(|burst| burst.get_cloud(now_s))
        {
            world_particles.push(get_dust_particle(&dust));
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
        let viewmodel_particles: Vec<Particle> = Vec::new();
        if self.player.is_alive() {
            let viewmodel: &GunViewmodel = match self.guns.gun {
                Gun::Laser => &scene.bolt_action_rifle,
                Gun::Rifle => &scene.revolver,
            };
            viewmodel_draws.extend(draw_items_for(
                &viewmodel.model,
                camera_world * self.guns.get_camera_from_muzzle() * viewmodel.muzzle_from_gltf,
                false,
            ));
        }

        let mut hud_rects: Vec<HudRect> = build_hud(
            &HudState {
                is_dead: !self.player.is_alive(),
                frame_ms: self.show_frame_ms,
            },
            width_px,
            height_px,
        );
        hud_rects.extend(self.objective_hud(width_px, height_px));
        let overlay_rects: Vec<OverlayRect> = build_overlay_rects(
            &self.overlay_hud_state(),
            &self.bar_pulses,
            (width_px as u32, height_px as u32),
        );
        let lens_particles: Vec<Particle> = world_particles[..robot_particle_count]
            .iter()
            .chain(&world_particles[beacon_particle_end..])
            .copied()
            .collect();
        FrameData {
            lens_draws,
            lens_particles,
            camera,
            world_draws,
            viewmodel_draws,
            joint_matrices,
            point_lights,
            decals: self.decals.iter().copied().collect(),
            world_particles,
            viewmodel_particles,
            hud_rects,
            overlay_rects,
            hit_effect: self.get_hit_effect(),
        }
    }
}

/// `get_dust_particle` draws a grain or puff of ground dust (`dust.rs`) as this level's smoke, in
/// its albedo and opacity, turned by its tumble; a grain's sprite is `GRAIN_SPRITE_PER_RADIUS`
/// times its radius, as the smoke's soft edge leaves its middle smaller (the renderer has no grain
/// shapes yet).
///
/// Example: `world_particles.push(get_dust_particle(&dust));`
fn get_dust_particle(dust: &DustParticle) -> Particle {
    let sprite_per_radius: f32 = if dust.shape == DustShape::Puff {
        1.0
    } else {
        GRAIN_SPRITE_PER_RADIUS
    };
    Particle {
        position_m: dust.position_m.as_vec3(),
        stretch_m: Vec3::ZERO,
        size_m: dust.radius_m as f32 * sprite_per_radius,
        color_rgba: Vec3::from(dust.albedo).extend(dust.opacity_frac as f32),
        kind: ParticleKind::Smoke,
        seed: (dust.orientation.to_axis_angle().1 / std::f64::consts::TAU) as f32,
    }
}
