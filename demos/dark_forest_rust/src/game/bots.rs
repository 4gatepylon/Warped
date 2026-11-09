//! `bots` are the enemy robots: they hunt the player, keep a fighting distance, strafe, shoot
//! energy bolts with distance-dependent accuracy, die, and respawn.
//!
//! Example:
//!     if let Some(shot) = bot.update(&context, dt_s) { ... }

use crate::game::nav::{NavGraph, NavStep};
use crate::game::warps::Warps;
use crate::game::world::LadderVolume;
use glam::{Quat, Vec3};
use warped_shared::gait::Gait;

use super::collision::{Aabb, MoveResult, cast_ray, move_body};

/// `BOT_MAX_HEALTH` is a bot's full health.
pub const BOT_MAX_HEALTH: f32 = 100.0;
/// `BOT_RADIUS_M` is half a bot's width.
const BOT_RADIUS_M: f32 = 0.4;
/// `BOT_HEIGHT_M` is a bot's height standing, as its teal robot is drawn (Lunar Surface's
/// `AVATAR_HEIGHT_M`).
pub const BOT_HEIGHT_M: f32 = 2.0;
/// `RUN_SPEED_M_S` is a bot's running speed.
const RUN_SPEED_M_S: f32 = 4.6;
/// `WALK_SPEED_M_S` is a bot's strafing speed.
const WALK_SPEED_M_S: f32 = 2.0;
/// `TURN_RATE_RAD_S` is how fast a bot turns.
const TURN_RATE_RAD_S: f32 = 5.0;
/// `SIGHT_RANGE_M` is how far a bot sees.
const SIGHT_RANGE_M: f32 = 55.0;
/// `ENGAGE_DISTANCE_M` is the distance beyond which a bot closes in.
const ENGAGE_DISTANCE_M: f32 = 15.0;
/// `RETREAT_DISTANCE_M` is the distance below which a bot backs off.
const RETREAT_DISTANCE_M: f32 = 6.0;
/// `REACTION_DELAY_S` is how long a bot waits after spotting the player before shooting.
const REACTION_DELAY_S: f32 = 0.7;
/// `RESPAWN_DELAY_S` is how long a dead bot lies before respawning.
pub const RESPAWN_DELAY_S: f32 = 6.0;
/// `GRAVITY_M_S2` is the downward acceleration.
const GRAVITY_M_S2: f32 = 16.0;
/// `CLIMB_SPEED_M_S` is how fast a robot climbs a ladder (a player's speed).
const CLIMB_SPEED_M_S: f32 = 3.2;
/// `REPLAN_S` is how often a robot that can't fight the player where it stands finds a new path.
const REPLAN_S: f32 = 0.8;
/// `WAYPOINT_REACH_M` is how close a robot comes to a path's spot before heading for the next.
const WAYPOINT_REACH_M: f32 = 0.35;
/// `WAYPOINT_HEIGHT_M` is how far above or below a spot a robot may be and still have reached it.
const WAYPOINT_HEIGHT_M: f32 = 0.6;
/// `LEVEL_HEIGHT_M` is how far below or above the robot the player may stand to be fought
/// directly rather than reached by a path.
const LEVEL_HEIGHT_M: f32 = 2.0;
/// `PLAYER_EYE_ABOVE_FEET_M` turns the player's eye into their feet.
const PLAYER_EYE_ABOVE_FEET_M: f32 = 1.6;
/// `LADDER_HOLD_BACK_M` keeps a climbing robot this far behind a ladder's middle, clear of the
/// deck's edge; `LADDER_HOLD_GAIN_PER_S` and `LADDER_HOLD_MAX_M_S` pull it there.
const LADDER_HOLD_BACK_M: f32 = 0.25;
const LADDER_HOLD_GAIN_PER_S: f32 = 4.0;
const LADDER_HOLD_MAX_M_S: f32 = 1.5;

/// `BotShot` is one bolt a bot fired.
#[derive(Clone, Copy, Debug)]
pub struct BotShot {
    pub origin_m: Vec3,
    pub end_m: Vec3,
    pub end_normal: Vec3,
    pub did_hit_player: bool,
}

/// `BotContext` is what a bot knows about the world this frame.
pub struct BotContext<'a> {
    pub solids: &'a [Aabb],
    pub nav: &'a NavGraph,
    pub ladders: &'a [LadderVolume],
    pub player_eye_m: Vec3,
    pub player_velocity_m_s: Vec3,
    pub is_player_alive: bool,
    pub is_player_crouched: bool,
}

/// `BODY_CENTRE_HEIGHT_M` is the middle of a body above its feet, where warps act on it.
pub const BODY_CENTRE_HEIGHT_M: f32 = 0.9;

/// `Bot` is one robot, an enemy or another player: where it stands and faces, its health and how
/// long it has been dead, its gait (`warped_shared::gait`, which clip it shows and where in it)
/// and where its feet were when the gait last followed them, its height as a share of standing
/// (less while another player crouches, W-137 (crouched robots sink)), and its flashes.
pub struct Bot {
    pub feet_m: Vec3,
    pub yaw_rad: f32,
    pub health: f32,
    pub dead_time_s: Option<f32>,
    pub gait: Gait,
    gait_feet_m: Vec3,
    pub height_frac: f32,
    pub hit_flash_frac: f32,
    pub muzzle_flash_frac: f32,
    velocity_m_s: Vec3,
    seen_time_s: f32,
    fire_cooldown_s: f32,
    strafe_sign: f32,
    strafe_time_s: f32,
    avoid_time_s: f32,
    avoid_direction: Vec3,
    last_seen_m: Option<Vec3>,
    noise_state: u32,
    path: Vec<NavStep>,
    path_index: usize,
    replan_s: f32,
    is_climbing: bool,
}

impl Bot {
    /// `apply_warps` pulls the bot towards black holes, kills it inside a horizon and carries it
    /// through a wormhole mouth it walks into.
    ///
    /// Example: `bot.apply_warps(&arena.warps, dt_s);`
    pub fn apply_warps(&mut self, warps: &Warps, dt_s: f32) {
        let centre_m: Vec3 = self.feet_m + Vec3::Y * BODY_CENTRE_HEIGHT_M;
        self.velocity_m_s += warps.get_pull_m_per_s2(centre_m) * dt_s;
        if let Some((exit_m, turn)) = warps.get_mouth_exit_feet_m(self.feet_m, centre_m) {
            self.feet_m = exit_m;
            self.velocity_m_s = turn * self.velocity_m_s;
            let facing: Vec3 = turn * Vec3::new(self.yaw_rad.sin(), 0.0, self.yaw_rad.cos());
            self.yaw_rad = facing.x.atan2(facing.z);
        }
        if warps.is_inside_horizon(centre_m) {
            self.take_damage(f32::MAX);
        }
    }

    /// `follow_path` steers along the bot's path: the direction and speed to walk, and the climbing
    /// speed while it climbs a ladder (holding just behind its middle, then stepping off at the
    /// spot it leads to).
    fn follow_path(&mut self, ladders: &[LadderVolume]) -> (Vec3, f32, Option<f32>) {
        while let Some(step) = self.path.get(self.path_index) {
            let is_reached: bool = (step.position_m - self.feet_m).with_y(0.0).length()
                < WAYPOINT_REACH_M
                && (step.position_m.y - self.feet_m.y).abs() < WAYPOINT_HEIGHT_M;
            if !is_reached {
                break;
            }
            self.path_index += 1;
        }
        let Some(step) = self.path.get(self.path_index).copied() else {
            self.is_climbing = false;
            return (Vec3::ZERO, 0.0, None);
        };
        let to_step: Vec3 = (step.position_m - self.feet_m).with_y(0.0);
        let Some(ladder_index) = step.ladder else {
            self.is_climbing = false;
            return (to_step, RUN_SPEED_M_S, None);
        };
        let ladder: &LadderVolume = &ladders[ladder_index];
        let facing: Vec3 = (-ladder.frame.z_axis.truncate())
            .with_y(0.0)
            .normalize_or_zero();
        let centre_m: Vec3 = ((ladder.aabb.min_m + ladder.aabb.max_m) * 0.5).with_y(self.feet_m.y);
        let is_going_up: bool = step.position_m.y > self.feet_m.y;
        let is_done: bool = if is_going_up {
            self.feet_m.y > step.position_m.y + 0.05
        } else {
            self.feet_m.y < step.position_m.y + 0.05
        };
        if self.is_climbing && is_done {
            self.is_climbing = false;
            return (to_step, WALK_SPEED_M_S, None);
        }
        if !self.is_climbing && !ladder.aabb.overlaps(&self.body()) {
            return (centre_m - self.feet_m, WALK_SPEED_M_S, None);
        }
        self.is_climbing = true;
        let to_hold: Vec3 = centre_m - facing * LADDER_HOLD_BACK_M - self.feet_m;
        let hold_m_s: f32 = (to_hold.length() * LADDER_HOLD_GAIN_PER_S).min(LADDER_HOLD_MAX_M_S);
        let climb_m_s: f32 = if is_going_up {
            CLIMB_SPEED_M_S
        } else {
            -CLIMB_SPEED_M_S
        };
        (to_hold, hold_m_s, Some(climb_m_s))
    }

    /// `new` spawns a bot.
    pub fn new(feet_m: Vec3, seed: u32) -> Bot {
        Bot {
            feet_m,
            yaw_rad: 0.0,
            health: BOT_MAX_HEALTH,
            dead_time_s: None,
            gait: Gait::New(0.0),
            gait_feet_m: feet_m,
            height_frac: 1.0,
            hit_flash_frac: 0.0,
            muzzle_flash_frac: 0.0,
            velocity_m_s: Vec3::ZERO,
            seen_time_s: 0.0,
            fire_cooldown_s: 1.0,
            strafe_sign: 1.0,
            strafe_time_s: 0.0,
            avoid_time_s: 0.0,
            avoid_direction: Vec3::ZERO,
            last_seen_m: None,
            path: Vec::new(),
            path_index: 0,
            replan_s: 0.0,
            is_climbing: false,
            noise_state: seed.wrapping_mul(0x9e37_79b9) | 1,
        }
    }

    /// `random_frac` returns a pseudo-random number in [0, 1].
    fn random_frac(&mut self) -> f32 {
        self.noise_state ^= self.noise_state << 13;
        self.noise_state ^= self.noise_state >> 17;
        self.noise_state ^= self.noise_state << 5;
        self.noise_state as f32 / u32::MAX as f32
    }

    /// `is_alive` says whether the bot is alive.
    pub fn is_alive(&self) -> bool {
        self.dead_time_s.is_none()
    }

    /// `facing` is the direction the bot faces; the robot model faces +z at zero yaw.
    pub fn facing(&self) -> Vec3 {
        Quat::from_rotation_y(self.yaw_rad) * Vec3::Z
    }

    /// `eye_m` is where the bot sees and shoots from.
    pub fn eye_m(&self) -> Vec3 {
        self.feet_m + Vec3::Y * 1.45 + self.facing() * 0.35
    }

    /// `body` is the bot's body hit box.
    pub fn body(&self) -> Aabb {
        Aabb {
            min_m: self.feet_m - Vec3::new(BOT_RADIUS_M, 0.0, BOT_RADIUS_M),
            max_m: self.feet_m
                + Vec3::new(
                    BOT_RADIUS_M,
                    BOT_HEIGHT_M * 0.78 * self.height_frac,
                    BOT_RADIUS_M,
                ),
        }
    }

    /// `head` is the bot's head hit box (headshots do double damage).
    pub fn head(&self) -> Aabb {
        Aabb::from_center(
            self.feet_m + Vec3::Y * (BOT_HEIGHT_M * 0.86 * self.height_frac) + self.facing() * 0.05,
            Vec3::splat(0.27),
        )
    }

    /// `take_damage` lowers health; it returns true if that killed the bot.
    pub fn take_damage(&mut self, damage: f32) -> bool {
        if !self.is_alive() {
            return false;
        }
        self.health -= damage;
        self.hit_flash_frac = 1.0;
        self.seen_time_s = self.seen_time_s.max(REACTION_DELAY_S * 0.5);
        if self.health <= 0.0 {
            self.dead_time_s = Some(0.0);
            return true;
        }
        false
    }

    /// `respawn` brings a dead bot back at a new spot.
    pub fn respawn(&mut self, feet_m: Vec3) {
        let seed: u32 = self.noise_state;
        *self = Bot::new(feet_m, seed);
    }

    /// `can_see` says whether the bot has a clear line of sight to a point.
    fn can_see(&self, solids: &[Aabb], target_m: Vec3) -> bool {
        let to_target: Vec3 = target_m - self.eye_m();
        let distance_m: f32 = to_target.length();
        if distance_m > SIGHT_RANGE_M {
            return false;
        }
        cast_ray(solids, self.eye_m(), to_target / distance_m, distance_m).is_none()
    }

    /// `walk_ahead` moves a held bot straight ahead at `speed_m_s` for `dt_s`, through walls, without
    /// thinking or shooting (`apply_warps` still carries it through a mouth).
    ///
    /// Example: `bot.walk_ahead(1.5, dt_s);`
    pub fn walk_ahead(&mut self, speed_m_s: f32, dt_s: f32) {
        self.velocity_m_s = self.facing() * speed_m_s;
        self.feet_m += self.velocity_m_s * dt_s;
    }

    /// `follow_gait` moves the bot's gait on to `now_s`, `dt_s` after the last frame, by the
    /// shared rules (`Gait::follow`): the distance its feet went over the ground since the gait
    /// last followed them (none while they stood still), whether it stands on something
    /// (`is_on_ground`), and the speed from which it runs (`run_from_m_per_s`).
    ///
    /// Example: `bot.follow_gait(true, 9.0, session.time_s, dt_s);`
    pub fn follow_gait(&mut self, is_on_ground: bool, run_from_m_per_s: f32, now_s: f64, dt_s: f64) {
        let moved_m: Option<f64> = (self.feet_m != self.gait_feet_m)
            .then(|| f64::from((self.feet_m - self.gait_feet_m).with_y(0.0).length()));
        self.gait_feet_m = self.feet_m;
        self.gait.follow(
            moved_m,
            is_on_ground,
            f64::from(run_from_m_per_s),
            now_s,
            dt_s,
        );
    }

    /// `is_climbing` says whether the bot is on a ladder.
    pub fn is_climbing(&self) -> bool {
        self.is_climbing
    }

    /// `update` runs one frame of behavior and returns a bolt if the bot fired.
    pub fn update(&mut self, context: &BotContext, dt_s: f32) -> Option<BotShot> {
        self.hit_flash_frac = (self.hit_flash_frac - dt_s * 4.0).max(0.0);
        self.muzzle_flash_frac = (self.muzzle_flash_frac - dt_s * 8.0).max(0.0);
        if let Some(time_s) = &mut self.dead_time_s {
            *time_s += dt_s;
            return None;
        }
        let sees_player: bool =
            context.is_player_alive && self.can_see(context.solids, context.player_eye_m);
        if sees_player {
            self.seen_time_s += dt_s;
            self.last_seen_m = Some(context.player_eye_m);
        } else {
            self.seen_time_s = (self.seen_time_s - dt_s).max(0.0);
        }
        let target_m: Vec3 = self.last_seen_m.unwrap_or(context.player_eye_m);
        let to_target: Vec3 = (target_m - self.feet_m).with_y(0.0);
        let distance_m: f32 = to_target.length();
        let toward: Vec3 = to_target.normalize_or_zero();
        let side: Vec3 = Vec3::new(-toward.z, 0.0, toward.x);
        self.strafe_time_s -= dt_s;
        if self.strafe_time_s <= 0.0 {
            self.strafe_sign = if self.random_frac() < 0.5 { -1.0 } else { 1.0 };
            self.strafe_time_s = 1.0 + self.random_frac() * 1.8;
        }
        let player_feet_m: Vec3 = context.player_eye_m - Vec3::Y * PLAYER_EYE_ABOVE_FEET_M;
        let is_level_with_player: bool = (player_feet_m.y - self.feet_m.y).abs() < LEVEL_HEIGHT_M;
        let should_follow_path: bool =
            context.is_player_alive && (self.is_climbing || !(sees_player && is_level_with_player));
        let mut climb_m_s: Option<f32> = None;
        let (mut desired, speed_m_s): (Vec3, f32) = if should_follow_path {
            self.replan_s -= dt_s;
            if self.replan_s <= 0.0 && !self.is_climbing {
                self.replan_s = REPLAN_S;
                self.path = context
                    .nav
                    .find_path(self.feet_m, player_feet_m)
                    .unwrap_or_default();
                self.path_index = 0;
            }
            let (direction, path_speed_m_s, climb) = self.follow_path(context.ladders);
            climb_m_s = climb;
            if self.path.is_empty() {
                (toward, RUN_SPEED_M_S)
            } else {
                (direction, path_speed_m_s)
            }
        } else if !context.is_player_alive {
            (Vec3::ZERO, 0.0)
        } else if sees_player {
            if distance_m > ENGAGE_DISTANCE_M {
                (toward + side * self.strafe_sign * 0.3, RUN_SPEED_M_S)
            } else if distance_m < RETREAT_DISTANCE_M {
                (-toward + side * self.strafe_sign, WALK_SPEED_M_S)
            } else {
                (side * self.strafe_sign, WALK_SPEED_M_S)
            }
        } else if distance_m > 1.5 {
            (toward, RUN_SPEED_M_S)
        } else {
            self.last_seen_m = None;
            (Vec3::ZERO, 0.0)
        };
        if self.avoid_time_s > 0.0 && !should_follow_path {
            self.avoid_time_s -= dt_s;
            desired = self.avoid_direction;
        }
        let desired: Vec3 = desired.normalize_or_zero();
        let target_velocity: Vec3 = desired * speed_m_s;
        let blend_frac: f32 = 1.0 - (-8.0 * dt_s).exp();
        let horizontal: Vec3 = self
            .velocity_m_s
            .with_y(0.0)
            .lerp(target_velocity, blend_frac);
        self.velocity_m_s = match climb_m_s {
            Some(climb_m_s) => horizontal.with_y(climb_m_s),
            None => horizontal.with_y(self.velocity_m_s.y - GRAVITY_M_S2 * dt_s),
        };
        let result: MoveResult = move_body(
            context.solids,
            self.feet_m,
            Vec3::new(BOT_RADIUS_M, BOT_HEIGHT_M * 0.5, BOT_RADIUS_M),
            self.velocity_m_s * dt_s,
            true,
        );
        if result.is_on_ground {
            self.velocity_m_s.y = 0.0;
        }
        if result.did_hit_wall
            && !should_follow_path
            && self.avoid_time_s <= 0.0
            && desired != Vec3::ZERO
        {
            let turn: f32 = if self.random_frac() < 0.5 { -1.0 } else { 1.0 };
            self.avoid_direction = Vec3::new(-desired.z * turn, 0.0, desired.x * turn);
            self.avoid_time_s = 0.6 + self.random_frac() * 0.6;
        }
        self.feet_m = result.position_m;

        let face_direction: Vec3 = if sees_player || desired == Vec3::ZERO {
            toward
        } else {
            desired
        };
        if face_direction != Vec3::ZERO {
            let target_yaw_rad: f32 = face_direction.x.atan2(face_direction.z);
            let mut delta_rad: f32 = target_yaw_rad - self.yaw_rad;
            delta_rad = (delta_rad + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            self.yaw_rad += delta_rad.clamp(-TURN_RATE_RAD_S * dt_s, TURN_RATE_RAD_S * dt_s);
        }

        self.fire_cooldown_s -= dt_s;
        let is_facing: bool = self.facing().dot(toward) > 0.9;
        if !sees_player
            || self.seen_time_s < REACTION_DELAY_S
            || self.fire_cooldown_s > 0.0
            || !is_facing
        {
            return None;
        }
        self.fire_cooldown_s = 0.9 + self.random_frac() * 0.8;
        self.muzzle_flash_frac = 1.0;
        let origin_m: Vec3 = self.eye_m();
        let to_player: Vec3 = context.player_eye_m - Vec3::Y * 0.3 - origin_m;
        let player_distance_m: f32 = to_player.length();
        let player_speed_m_s: f32 = context.player_velocity_m_s.with_y(0.0).length();
        let mut hit_chance_frac: f32 =
            (0.7 - player_distance_m * 0.012 - player_speed_m_s * 0.05).clamp(0.12, 0.7);
        if context.is_player_crouched {
            hit_chance_frac *= 0.7;
        }
        let did_hit_player: bool = self.random_frac() < hit_chance_frac;
        let miss_offset: Vec3 = if did_hit_player {
            Vec3::ZERO
        } else {
            let angle_rad: f32 = self.random_frac() * std::f32::consts::TAU;
            let right: Vec3 = to_player.cross(Vec3::Y).normalize_or_zero();
            (right * angle_rad.cos() + Vec3::Y * angle_rad.sin() * 0.6) * (0.6 + self.random_frac())
        };
        let direction: Vec3 = (to_player + miss_offset).normalize();
        let (end_m, end_normal) = if did_hit_player {
            (origin_m + direction * player_distance_m, -direction)
        } else {
            match cast_ray(context.solids, origin_m, direction, 120.0) {
                Some((_, hit)) => (origin_m + direction * hit.distance_m, hit.normal),
                None => (origin_m + direction * 120.0, -direction),
            }
        };
        Some(BotShot {
            origin_m,
            end_m,
            end_normal,
            did_hit_player,
        })
    }
}
