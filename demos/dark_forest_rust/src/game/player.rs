//! `player` moves the player by the movement rules every level shares (`warped_shared::movement`,
//! Lunar Surface's, W-126 (same play in every level)): crouching, sprinting and its energy, the
//! head bob and a hit's slowing come from `MoveState::step`, with the numbers of `Movement` (the
//! map's own times the server settings, as Lunar's `VoxelWorld::set_settings` works them out);
//! the forest (`Forest`, a `Ground`) then carries the step out: jumping, gravity, its own rope
//! ladders, collision and landing dip.
//!
//! Example:
//!     player.update(&input, &movement, &solids, &ladders, dt_s);
//!     let eye_m: Vec3 = player.eye_position_m();

use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};
use warped_shared::movement::{Ground, MoveInput, MoveIntent, MoveRules, MoveState};

use super::collision::{Aabb, MoveResult, move_body};
use super::world::LadderVolume;
use crate::map::PlayerSpec;
use crate::settings::{
    HEAD_BOB, MovementSettings, ServerSettings, SprintEnergySettings, WhenHitSettings,
};

/// `MAX_MOVE_STEP_M` is the longest move tried at once: collision checks only where a move ends,
/// so a sprint during a slow frame is cut into steps shorter than the body is wide, and never
/// skips through a thin rail or wall.
const MAX_MOVE_STEP_M: f32 = 0.25;
/// `MAX_FALL_SPEED_M_PER_S` is the fastest a player falls (Lunar Surface's).
const MAX_FALL_SPEED_M_PER_S: f32 = 40.0;
/// `LADDER_WALK_M_S` is the walking speed the rope ladders were made for: a climber moves sideways
/// and steps off at the top at shares of it, as before Lunar Surface's faster walking came here.
const LADDER_WALK_M_S: f32 = 4.4;
/// `STAND_HEIGHT_M` is the standing body height.
const STAND_HEIGHT_M: f32 = 1.8;
/// `EYE_BELOW_TOP_M` is how far the eye is below the top of the body.
const EYE_BELOW_TOP_M: f32 = 0.12;
/// `BODY_RADIUS_M` is half the body's width.
const BODY_RADIUS_M: f32 = 0.3;
/// `MAX_PITCH_RAD` limits looking up and down.
const MAX_PITCH_RAD: f32 = 1.54;
/// `MAX_HEALTH` is full health.
pub const MAX_HEALTH: f32 = 100.0;
/// `REGEN_DELAY_S` is how long after being hit health starts to regenerate.
const REGEN_DELAY_S: f32 = 5.0;
/// `REGEN_RATE_PER_S` is the regeneration speed.
const REGEN_RATE_PER_S: f32 = 12.0;
/// `CLIMB_SPEED_M_S` is how fast a player climbs a rope ladder.
const CLIMB_SPEED_M_S: f32 = 3.2;
/// `CLIMB_HORIZONTAL_FRAC` is the share of walking speed a climber keeps sideways and forward.
const CLIMB_HORIZONTAL_FRAC: f32 = 0.45;
/// `CLIMB_DOWN_PITCH_RAD` is how far down a climber looks for forward to climb down.
const CLIMB_DOWN_PITCH_RAD: f32 = -0.6;
/// `LADDER_FACING_DOT` is how squarely a player on the ground must walk into a ladder to start
/// climbing it.
const LADDER_FACING_DOT: f32 = 0.3;
/// `CLIMB_TOP_STEP_M` is how far below a ladder's top a climber's feet are when walking forward
/// steps them off it onto the deck.
const CLIMB_TOP_STEP_M: f32 = 1.1;

/// `Movement` is how a player moves on a map with the server's settings, as Lunar Surface's
/// `VoxelWorld::set_settings` works it out: walking and sprinting at multiples of the map's speeds,
/// and a jump that rises `jump_height_m` in `jump_time_per_map_jump_time` of the map's jump time,
/// which sets gravity; with the settings' crouching, sprint energy and slowing when hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Movement {
    pub walk_m_per_s: f32,
    pub sprint_m_per_s: f32,
    pub jump_m_per_s: f32,
    pub gravity_m_per_s2: f32,
    pub eye_height_m: f32,
    pub settings: MovementSettings,
    pub sprint_energy: SprintEnergySettings,
    pub when_hit: WhenHitSettings,
}

impl Movement {
    /// `New` works out the movement on a map whose players move as `map_player` under
    /// `map_gravity_m_per_s2`, with `settings`.
    ///
    /// Example: `let movement: Movement = Movement::New(&map.player, map.gravity_m_per_s2, &settings);`
    #[allow(non_snake_case)]
    pub fn New(
        map_player: &PlayerSpec,
        map_gravity_m_per_s2: f32,
        settings: &ServerSettings,
    ) -> Movement {
        let movement: &MovementSettings = &settings.movement;
        let half_jump_s: f64 = f64::from(map_player.jump_m_per_s) / f64::from(map_gravity_m_per_s2)
            * movement.jump_time_per_map_jump_time;
        let jump_m_per_s: f64 = 2.0 * movement.jump_height_m / half_jump_s;
        Movement {
            walk_m_per_s: (f64::from(map_player.walk_m_per_s) * movement.walk_speed_per_map_speed)
                as f32,
            sprint_m_per_s: (f64::from(map_player.sprint_m_per_s)
                * movement.sprint_speed_per_map_speed) as f32,
            jump_m_per_s: jump_m_per_s as f32,
            gravity_m_per_s2: (jump_m_per_s / half_jump_s) as f32,
            eye_height_m: map_player.eye_height_m,
            settings: movement.clone(),
            sprint_energy: settings.sprint_energy.clone(),
            when_hit: settings.when_hit.clone(),
        }
    }

    /// `get_crouch_drop_m` gives how far crouching (`crouch_frac`, 0 standing to 1 crouched)
    /// lowers the eye: up to the settings' `crouch_eye_height_frac` of the standing eye height, as
    /// Lunar Surface's `VoxelPlayer::get_crouch_drop_m`.
    ///
    /// Example: `let drop_m: f32 = movement.get_crouch_drop_m(1.0);`
    pub fn get_crouch_drop_m(&self, crouch_frac: f32) -> f32 {
        crouch_frac * self.eye_height_m * (1.0 - self.settings.crouch_eye_height_frac as f32)
    }
}

/// `PlayerInput` is one frame's player controls.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerInput {
    pub move_axes: Vec2,
    pub look_delta_rad: Vec2,
    pub is_sprint_held: bool,
    pub is_crouch_held: bool,
    pub is_jump_pressed: bool,
}

/// `Player` is the player's body and view: where its feet are, how it moves and looks, whether it
/// stands on something, how tall it is now, its health and death, the movement rules' state
/// (`moves`: crouch, sprint energy, sprinting, the head bob and a hit's slow), the landing dip,
/// whether this frame was a footstep, whether it climbs a ladder, and the warps' pull on it
/// along the ground (W-144, `Warps::get_walk_pull_m_per_s`), which its owner sets before each
/// `update`.
pub struct Player {
    pub feet_m: Vec3,
    pub velocity_m_s: Vec3,
    pub yaw_rad: f32,
    pub pitch_rad: f32,
    pub is_on_ground: bool,
    pub height_m: f32,
    pub health: f32,
    pub dead_time_s: Option<f32>,
    pub since_hit_s: f32,
    pub moves: MoveState,
    pub landing_dip_m: f32,
    pub did_step: bool,
    pub is_climbing: bool,
    pub pull_m_per_s: Vec3,
}

/// `Forest` is the forest as the movement rules' `Ground`: its boxes to collide with, its rope
/// ladders, and the map's jump and gravity (`Movement`).
pub struct Forest<'a> {
    pub movement: &'a Movement,
    pub solids: &'a [Aabb],
    pub ladders: &'a [LadderVolume],
}

impl Player {
    /// `new` makes a player standing at a spawn point.
    pub fn new(feet_m: Vec3, yaw_rad: f32) -> Player {
        Player {
            feet_m,
            velocity_m_s: Vec3::ZERO,
            yaw_rad,
            pitch_rad: 0.0,
            is_on_ground: false,
            height_m: STAND_HEIGHT_M,
            health: MAX_HEALTH,
            dead_time_s: None,
            since_hit_s: REGEN_DELAY_S,
            moves: MoveState::New(),
            landing_dip_m: 0.0,
            did_step: false,
            is_climbing: false,
            pull_m_per_s: Vec3::ZERO,
        }
    }

    /// `half_extent_m` is half the body's size.
    fn half_extent_m(height_m: f32) -> Vec3 {
        Vec3::new(BODY_RADIUS_M, height_m * 0.5, BODY_RADIUS_M)
    }

    /// `rotation` is the view rotation.
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(EulerRot::YXZ, self.yaw_rad, self.pitch_rad, 0.0)
    }

    /// `kick_view` turns the view by a gun's recoil, `kick_rad` (x right, y up), as Lunar
    /// Surface's mouse look turns it.
    ///
    /// Example: `player.kick_view(Vec2::new(0.0025, 0.0075));`
    pub fn kick_view(&mut self, kick_rad: Vec2) {
        self.yaw_rad -= kick_rad.x;
        self.pitch_rad = (self.pitch_rad + kick_rad.y).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);
    }

    /// `eye_position_m` is the camera position, with the head bob (`get_bob_m`) and landing dip.
    pub fn eye_position_m(&self) -> Vec3 {
        let bob_m: f32 = self.get_bob_m() as f32;
        self.feet_m + Vec3::Y * (self.height_m - EYE_BELOW_TOP_M - self.landing_dip_m + bob_m)
    }

    /// `get_bob_m` gives the head bob as Lunar Surface's: the eye dips at each footstep, deeper
    /// when sprinting, by `HEAD_BOB`.
    ///
    /// Example: `let bob_m: f64 = player.get_bob_m();`
    pub fn get_bob_m(&self) -> f64 {
        let depth_m: f64 = if self.moves.is_sprinting {
            HEAD_BOB.sprint_depth_m
        } else {
            HEAD_BOB.walk_depth_m
        };
        -self.moves.bob_frac * depth_m * self.moves.bob_phase_rad.sin().abs()
    }

    /// `camera_world` is the camera's world transform.
    pub fn camera_world(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation(), self.eye_position_m())
    }

    /// `is_alive` says whether the player is alive.
    pub fn is_alive(&self) -> bool {
        self.dead_time_s.is_none()
    }

    /// `take_damage` lowers health; it returns true if that killed the player.
    pub fn take_damage(&mut self, damage: f32) -> bool {
        if !self.is_alive() {
            return false;
        }
        self.health -= damage;
        self.since_hit_s = 0.0;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead_time_s = Some(0.0);
            return true;
        }
        false
    }

    /// `should_respawn` says whether the respawn wait, `respawn_delay_s` (the session's
    /// `respawn_wait_s`), has passed.
    ///
    /// Example: `if player.should_respawn(20.0) { ... }`
    pub fn should_respawn(&self, respawn_delay_s: f32) -> bool {
        self.dead_time_s
            .is_some_and(|time_s: f32| time_s > respawn_delay_s)
    }

    /// `get_respawn_in_s` is how long until a dead player respawns, after a wait of
    /// `respawn_delay_s` (the session's `respawn_wait_s`; `None` while alive).
    ///
    /// Example: `if let Some(left_s) = player.get_respawn_in_s(20.0) { ... }`
    pub fn get_respawn_in_s(&self, respawn_delay_s: f32) -> Option<f32> {
        self.dead_time_s
            .map(|time_s: f32| (respawn_delay_s - time_s).max(0.0))
    }

    /// `update` applies one frame of input as every level does: the shared movement rules first
    /// (`MoveState::step`: crouching toward `crouch_frac` 1 over `crouch_s`, the body and eye
    /// lowering together, though standing up waits for room; sprinting while energy lasts; the
    /// head bob; the share of walking or sprinting speed a hit and crouching allow), then the
    /// forest carries the step out (`Forest::move_body`: the walk at that speed, set at once on
    /// the ground and in the air, a jump, gravity up to `MAX_FALL_SPEED_M_PER_S`, the rope
    /// ladders, collision stepping up ledges, `collision::STEP_HEIGHT_M`, in steps of at most
    /// `MAX_MOVE_STEP_M`, and the landing dip). A footstep (`did_step`) falls at each dip of the
    /// head bob.
    pub fn update(
        &mut self,
        input: &PlayerInput,
        movement: &Movement,
        solids: &[Aabb],
        ladders: &[LadderVolume],
        dt_s: f32,
    ) {
        self.since_hit_s += dt_s;
        if let Some(time_s) = &mut self.dead_time_s {
            *time_s += dt_s;
            self.height_m = (self.height_m - dt_s * 3.0).max(0.5);
            self.velocity_m_s = Vec3::ZERO;
            return;
        }
        if self.since_hit_s > REGEN_DELAY_S {
            self.health = (self.health + REGEN_RATE_PER_S * dt_s).min(MAX_HEALTH);
        }
        self.yaw_rad -= input.look_delta_rad.x;
        self.pitch_rad =
            (self.pitch_rad - input.look_delta_rad.y).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);

        let axes: Vec2 = input.move_axes.clamp_length_max(1.0);
        let before: MoveState = self.moves;
        let rules: MoveRules = MoveRules {
            movement: &movement.settings,
            energy: &movement.sprint_energy,
            when_hit: &movement.when_hit,
            head_bob: &HEAD_BOB,
        };
        let move_input: MoveInput = MoveInput {
            is_moving: axes.length() > 0.0,
            is_sprint_held: input.is_sprint_held,
            is_crouch_held: input.is_crouch_held,
            is_flying: false,
            is_on_ground: self.is_on_ground,
        };
        let slow_frac: f64 = self.moves.step(&move_input, &rules, f64::from(dt_s));
        let new_height_m: f32 =
            STAND_HEIGHT_M - movement.get_crouch_drop_m(self.moves.crouch_frac as f32);
        let grown: Aabb = Aabb {
            min_m: self.feet_m - Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M) + Vec3::Y * 0.01,
            max_m: self.feet_m + Vec3::new(BODY_RADIUS_M, new_height_m, BODY_RADIUS_M),
        };
        if new_height_m <= self.height_m
            || !solids.iter().any(|solid: &Aabb| solid.overlaps(&grown))
        {
            self.height_m = new_height_m;
        } else {
            self.moves.crouch_frac = before.crouch_frac;
        }
        let stepped_rad: f64 =
            (self.moves.bob_phase_rad - before.bob_phase_rad).rem_euclid(std::f64::consts::TAU);
        self.did_step = ((before.bob_phase_rad + stepped_rad) / std::f64::consts::PI).floor()
            > (before.bob_phase_rad / std::f64::consts::PI).floor();
        let speed_m_per_s: f64 = slow_frac
            * f64::from(if self.moves.is_sprinting {
                movement.sprint_m_per_s
            } else {
                movement.walk_m_per_s
            });
        let intent: MoveIntent = MoveIntent {
            forward_frac: f64::from(axes.y),
            right_frac: f64::from(axes.x),
            speed_m_per_s,
            is_jump_held: input.is_jump_pressed,
            is_crouch_held: input.is_crouch_held,
        };
        let forest: Forest = Forest {
            movement,
            solids,
            ladders,
        };
        self.is_on_ground = forest.move_body(self, &intent, f64::from(dt_s));
    }

    /// `slow_down` starts a hit's slowing (`Movement::when_hit`'s `slow_s`), as Lunar Surface slows
    /// a player it hits.
    ///
    /// Example: `player.slow_down(&movement);`
    pub fn slow_down(&mut self, movement: &Movement) {
        self.moves.slowed_s = movement.when_hit.slow_s;
    }
}

impl Ground for Forest<'_> {
    type Body = Player;

    /// `move_body` carries out a step's `intent` for the player in the forest: the walk at its
    /// speed plus the warps' pull (`Player::pull_m_per_s`), set at once on the ground and in the
    /// air, a jump from the ground, gravity up to
    /// `MAX_FALL_SPEED_M_PER_S`; on a rope ladder (walked into, or already climbing), climbing
    /// up with forward (down while looking down, or holding crouch) and stepping off at the top;
    /// collision, stepping up ledges, in steps of at most `MAX_MOVE_STEP_M`; and the landing dip.
    /// It says whether the player then stands on something.
    ///
    /// Example: `let is_on_ground: bool = forest.move_body(&mut player, &intent, 1.0 / 60.0);`
    fn move_body(&self, body: &mut Player, intent: &MoveIntent, dt_s: f64) -> bool {
        let dt_s: f32 = dt_s as f32;
        let forward: Vec3 = Quat::from_rotation_y(body.yaw_rad) * Vec3::NEG_Z;
        let right: Vec3 = Quat::from_rotation_y(body.yaw_rad) * Vec3::X;
        let (forward_frac, right_frac) = (intent.forward_frac as f32, intent.right_frac as f32);
        let direction: Vec3 = forward * forward_frac + right * right_frac;
        let walk_m_s: Vec3 = direction * intent.speed_m_per_s as f32;
        let target_m_s: Vec3 = if body.pull_m_per_s == Vec3::ZERO {
            walk_m_s
        } else {
            walk_m_s + body.pull_m_per_s
        };
        body.velocity_m_s = target_m_s.with_y(
            (body.velocity_m_s.y - self.movement.gravity_m_per_s2 * dt_s)
                .max(-MAX_FALL_SPEED_M_PER_S),
        );
        if intent.is_jump_held && body.is_on_ground {
            body.velocity_m_s.y = self.movement.jump_m_per_s;
        }
        let aabb: Aabb = Aabb {
            min_m: body.feet_m - Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M),
            max_m: body.feet_m + Vec3::new(BODY_RADIUS_M, body.height_m, BODY_RADIUS_M),
        };
        let ladder: Option<&LadderVolume> = self.ladders.iter().find(|l| l.aabb.overlaps(&aabb));
        let ladder_m_s: Vec3 = direction * LADDER_WALK_M_S;
        body.is_climbing = ladder.is_some_and(|ladder| {
            let facing: Vec3 = -ladder.frame.z_axis.truncate();
            let walking_in: bool = ladder_m_s.dot(facing) > LADDER_FACING_DOT * LADDER_WALK_M_S;
            !intent.is_jump_held && (body.is_climbing || !body.is_on_ground || walking_in)
        });
        if body.is_climbing {
            let down_sign: f32 = if body.pitch_rad < CLIMB_DOWN_PITCH_RAD {
                -1.0
            } else {
                1.0
            };
            let climb_m_s: f32 = forward_frac * down_sign * CLIMB_SPEED_M_S
                - f32::from(u8::from(intent.is_crouch_held)) * CLIMB_SPEED_M_S;
            let ladder: &LadderVolume = ladder.expect("a climber is on a ladder");
            let facing: Vec3 = -ladder.frame.z_axis.truncate();
            let is_at_top: bool = body.feet_m.y > ladder.aabb.max_m.y - CLIMB_TOP_STEP_M;
            let along_m_s: Vec3 = if is_at_top {
                ladder_m_s
            } else {
                (ladder_m_s - facing * ladder_m_s.dot(facing)) * CLIMB_HORIZONTAL_FRAC
            };
            body.velocity_m_s = along_m_s.with_y(climb_m_s.max(-CLIMB_SPEED_M_S * 1.5));
        }

        let fall_speed_m_s: f32 = -body.velocity_m_s.y;
        let was_on_ground: bool = body.is_on_ground;
        let mut is_on_ground: bool = body.is_on_ground;
        let delta_m: Vec3 = body.velocity_m_s * dt_s;
        let step_count: usize = (delta_m.length() / MAX_MOVE_STEP_M).ceil().max(1.0) as usize;
        let mut did_hit_ceiling: bool = false;
        for _ in 0..step_count {
            let result: MoveResult = move_body(
                self.solids,
                body.feet_m,
                Player::half_extent_m(body.height_m),
                delta_m / step_count as f32,
                is_on_ground || body.is_climbing,
            );
            body.feet_m = result.position_m;
            is_on_ground = result.is_on_ground;
            did_hit_ceiling |= result.did_hit_ceiling;
        }
        if is_on_ground || did_hit_ceiling {
            body.velocity_m_s.y = body.velocity_m_s.y.clamp(-0.5, 0.0);
            if did_hit_ceiling {
                body.velocity_m_s.y = 0.0;
            }
        }
        if is_on_ground && !was_on_ground && fall_speed_m_s > 3.0 {
            body.landing_dip_m = (fall_speed_m_s * 0.012).min(0.12);
        }
        body.landing_dip_m = (body.landing_dip_m - dt_s * 0.5).max(0.0);
        is_on_ground
    }
}

/// `tests` checks climbing a rope ladder onto a deck, sprint energy, movement against Lunar
/// Surface's, the jump, crouching and the respawn delay.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | movement's numbers | walking, sprinting, jump speed and gravity on this map, against Lunar Surface's on its own map with its own settings | `test_movement_matches_lunar_surface` |
/// | gait on open ground | walking, sprinting, crouching (speed), slowed by a hit | `test_speeds_are_lunar_surfaces` |
/// | jump | a standing jump on flat ground, against Lunar Surface's own settings and map | `test_a_jump_matches_lunar_surface` |
/// | crouching | down (eye at its share of the standing eye, in `crouch_s`), up again, up under a low roof (waits) | `test_crouching_lowers_the_eye_and_waits_for_room` |
/// | sprint into a thin rail | at 60 frames a second, in a slow frame of 0.05 s | `test_sprinting_never_skips_through_a_thin_rail` |
/// | input at a ladder's foot | walk into it (climbs to the deck), walk past it sideways (stays on the ground) | `test_walking_into_a_ladder_climbs_onto_the_deck`, `test_walking_beside_a_ladder_stays_down` |
/// | sprint energy while sprinting | some left, empty | `test_sprinting_drains_energy_then_stops` |
/// | sprint energy while not sprinting | standing, crouched | `test_energy_refills_faster_crouched` |
/// | time since death | before the settings' respawn delay, after it | `test_respawn_waits_the_settings_delay` |
/// | a mouth near a walker (W-144) | walking toward it (farther in the same time), away (less far, still away), standing (drifts toward it); past the pull's reach (each the same as with no mouth, bit for bit) | `test_warps_pull_walkers_toward_them` |
/// | rules' state each step | walking, sprinting, crouching, jumping, standing, hit: the shared `MoveState` stepped with the same keys and ground, bit for bit | `test_the_rules_are_the_shared_ones` |
#[cfg(test)]
mod tests {
    use glam::Mat4;

    use super::*;
    use crate::game::bots::BODY_CENTRE_HEIGHT_M;
    use crate::game::warps::{Warps, WormholeMouth};
    use crate::map::{self, MapFile};
    use crate::settings::GravitySettings;

    /// `DECK_TOP_M` is the test deck's height.
    const DECK_TOP_M: f32 = 7.0;
    /// `SPEED_TOLERANCE_M_S` is how close a measured speed comes to its setting.
    const SPEED_TOLERANCE_M_S: f32 = 0.01;
    const FOREST_WALK_M_PER_S: f32 = 8.0;
    const FOREST_SPRINT_M_PER_S: f32 = 15.48;
    const PULL_WALK_S: f32 = 0.5;
    const NEAR_MOUTH_M: f32 = 8.0;
    const FAR_MOUTH_M: f32 = 40.0;
    const MOUTH_HEIGHT_M: f32 = 1.7;
    const MOUTH_THROAT_M: f32 = 1.6;
    const MOUTH_REACH_M: f32 = 5.0;
    const MOUTH_LAMBDA_SQUARED: f32 = 0.25;

    /// `forest_movement` is the movement on the Dark Forest map with the built-in settings.
    fn forest_movement() -> Movement {
        let map: MapFile = map::load(&map::default_map_path()).expect("the map loads");
        Movement::New(
            &map.player,
            map.gravity_m_per_s2,
            &ServerSettings::Builtin(),
        )
    }

    /// `lunar_movement` is the movement on Lunar Surface's map with Lunar Surface's own server
    /// settings file, read key by key.
    fn lunar_movement() -> Movement {
        let lunar_map_path: std::path::PathBuf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../common/maps/lunar_surface/map.json");
        let lunar_map: MapFile = serde_json::from_str(
            &std::fs::read_to_string(lunar_map_path).expect("Lunar Surface's map is there"),
        )
        .expect("Lunar Surface's map reads");
        let lunar = |key: &str| -> f64 {
            crate::settings::read_lunar_setting("server_settings.yaml", &["movement"], key)
        };
        let mut settings: ServerSettings = ServerSettings::Builtin();
        settings.movement = MovementSettings {
            walk_speed_per_map_speed: lunar("walk_speed_per_map_speed"),
            sprint_speed_per_map_speed: lunar("sprint_speed_per_map_speed"),
            jump_height_m: lunar("jump_height_m"),
            jump_time_per_map_jump_time: lunar("jump_time_per_map_jump_time"),
            crouch_speed_frac: lunar("crouch_speed_frac"),
            crouch_eye_height_frac: lunar("crouch_eye_height_frac"),
            crouch_s: lunar("crouch_s"),
            shoot_after_moving_s: lunar("shoot_after_moving_s"),
        };
        Movement::New(&lunar_map.player, lunar_map.gravity_m_per_s2, &settings)
    }

    /// `walk_by_a_mouth` walks a player facing -z from the origin of `open_ground` with
    /// `move_axes` for `PULL_WALK_S`, pulled each step by one mouth `mouth_m` ahead at the
    /// map's mouths' height (none for `None`), and gives where the feet end up.
    fn walk_by_a_mouth(move_axes: Vec2, mouth_m: Option<f32>) -> Vec3 {
        let movement: Movement = forest_movement();
        let gravity: GravitySettings = ServerSettings::Builtin().gravity;
        let warps: Warps = Warps {
            black_holes: Vec::new(),
            mouths: mouth_m
                .map(|ahead_m| WormholeMouth {
                    centre_m: Vec3::new(0.0, MOUTH_HEIGHT_M, -ahead_m),
                    throat_radius_m: MOUTH_THROAT_M,
                    reach_m: MOUTH_REACH_M,
                    lambda_squared: MOUTH_LAMBDA_SQUARED,
                    partner: 0,
                })
                .into_iter()
                .collect(),
        };
        let input: PlayerInput = PlayerInput {
            move_axes,
            ..PlayerInput::default()
        };
        let mut player: Player = Player::new(Vec3::ZERO, 0.0);
        let dt_s: f32 = 1.0 / 60.0;
        for _ in 0..(PULL_WALK_S / dt_s).round() as usize {
            player.pull_m_per_s = warps.get_walk_pull_m_per_s(
                &gravity,
                movement.walk_m_per_s,
                player.feet_m + Vec3::Y * BODY_CENTRE_HEIGHT_M,
            );
            player.update(&input, &movement, &open_ground(), &[], dt_s);
        }
        player.feet_m
    }

    /// `test_warps_pull_walkers_toward_them` walks toward, away from and stands by a mouth, and
    /// far from one.
    #[test]
    fn test_warps_pull_walkers_toward_them() {
        let (forward, back, still) = (Vec2::Y, Vec2::NEG_Y, Vec2::ZERO);
        let ahead_m = |feet_m: Vec3| -> f32 { -feet_m.z };
        let free_m: f32 = ahead_m(walk_by_a_mouth(forward, None));
        let toward_m: f32 = ahead_m(walk_by_a_mouth(forward, Some(NEAR_MOUTH_M)));
        assert!(toward_m > free_m, "{toward_m} against {free_m}");
        let free_back_m: f32 = ahead_m(walk_by_a_mouth(back, None));
        let away_m: f32 = ahead_m(walk_by_a_mouth(back, Some(NEAR_MOUTH_M)));
        assert!(
            free_back_m < away_m && away_m < 0.0,
            "{away_m} against {free_back_m}"
        );
        assert!(ahead_m(walk_by_a_mouth(still, Some(NEAR_MOUTH_M))) > 0.0);
        for move_axes in [forward, back, still] {
            assert_eq!(
                walk_by_a_mouth(move_axes, Some(FAR_MOUTH_M)),
                walk_by_a_mouth(move_axes, None)
            );
        }
    }

    /// `open_ground` is a large flat floor at height 0.
    fn open_ground() -> [Aabb; 1] {
        [Aabb {
            min_m: Vec3::new(-500.0, -1.0, -500.0),
            max_m: Vec3::new(500.0, 0.0, 500.0),
        }]
    }

    /// `walk_s` walks a player from `feet_m` with `move_axes` for `seconds`, past a deck whose
    /// edge is at z = 47 and a ladder just outside it, and returns where they end up.
    fn walk_s(feet_m: Vec3, move_axes: Vec2, seconds: f32) -> Vec3 {
        let solids: [Aabb; 2] = [
            Aabb {
                min_m: Vec3::new(-50.0, -1.0, -50.0),
                max_m: Vec3::new(50.0, 0.0, 50.0),
            },
            Aabb {
                min_m: Vec3::new(9.0, DECK_TOP_M - 0.2, 37.0),
                max_m: Vec3::new(19.0, DECK_TOP_M, 47.0),
            },
        ];
        let ladders: [LadderVolume; 1] = [LadderVolume {
            aabb: Aabb {
                min_m: Vec3::new(10.5, 0.0, 47.0),
                max_m: Vec3::new(11.5, DECK_TOP_M + 1.0, 47.6),
            },
            frame: Mat4::from_translation(Vec3::new(11.0, 4.0, 47.3)),
            half_m: Vec3::new(0.5, 4.0, 0.3),
        }];
        let mut player: Player = Player::new(feet_m, 0.0);
        let input: PlayerInput = PlayerInput {
            move_axes,
            ..PlayerInput::default()
        };
        let movement: Movement = forest_movement();
        let dt_s: f32 = 1.0 / 120.0;
        for _ in 0..(seconds / dt_s) as usize {
            player.update(&input, &movement, &solids, &ladders, dt_s);
        }
        player.feet_m
    }

    /// `test_walking_into_a_ladder_climbs_onto_the_deck` checks holding W at a ladder's foot
    /// ends on the deck above it.
    #[test]
    fn test_walking_into_a_ladder_climbs_onto_the_deck() {
        let feet_m: Vec3 = walk_s(Vec3::new(11.0, 0.0, 48.5), Vec2::Y, 3.0);
        assert!((feet_m.y - DECK_TOP_M).abs() < 0.05, "ended at {feet_m}");
        assert!(feet_m.z < 46.5, "ended at {feet_m}");
    }

    /// `test_walking_beside_a_ladder_stays_down` checks walking past a ladder doesn't climb it.
    #[test]
    fn test_walking_beside_a_ladder_stays_down() {
        let feet_m: Vec3 = walk_s(Vec3::new(14.0, 0.0, 48.5), Vec2::Y, 2.0);
        assert!(feet_m.y < 0.05, "ended at {feet_m}");
    }

    /// `run_s` runs a standing player on open ground with `input` for `seconds`, starting with
    /// `energy_frac` of sprint energy, and returns the player.
    fn run_s(input: &PlayerInput, energy_frac: f32, seconds: f32) -> Player {
        let solids: [Aabb; 1] = open_ground();
        let mut player: Player = Player::new(Vec3::ZERO, 0.0);
        player.moves.energy_frac = f64::from(energy_frac);
        let movement: Movement = forest_movement();
        let dt_s: f32 = 1.0 / 120.0;
        for _ in 0..(seconds / dt_s).round() as usize {
            player.update(input, &movement, &solids, &[], dt_s);
        }
        player
    }

    /// `test_movement_matches_lunar_surface` checks a jump's speed, gravity and every movement
    /// setting here are Lunar Surface's, except walking and sprinting, which the user asked
    /// slower: halfway between City Sphere's (6 and 13 m/s) and Lunar Surface's (10 and 18 m/s).
    #[test]
    fn test_movement_matches_lunar_surface() {
        let (forest, mut lunar) = (forest_movement(), lunar_movement());
        assert!((forest.walk_m_per_s - FOREST_WALK_M_PER_S).abs() < SPEED_TOLERANCE_M_S);
        assert!((forest.sprint_m_per_s - FOREST_SPRINT_M_PER_S).abs() < SPEED_TOLERANCE_M_S);
        lunar.settings.walk_speed_per_map_speed = forest.settings.walk_speed_per_map_speed;
        lunar.settings.sprint_speed_per_map_speed = forest.settings.sprint_speed_per_map_speed;
        assert!((forest.jump_m_per_s - lunar.jump_m_per_s).abs() < 1e-4);
        assert!((forest.gravity_m_per_s2 - lunar.gravity_m_per_s2).abs() < 1e-3);
        assert_eq!(forest.settings, lunar.settings);
    }

    /// `test_speeds_are_lunar_surfaces` checks a player on open ground walks, sprints and crouches
    /// at `Movement`'s speeds at once (no run-up, as on Lunar Surface), and a hit halves the speed.
    #[test]
    fn test_speeds_are_lunar_surfaces() {
        let movement: Movement = forest_movement();
        let walk: PlayerInput = PlayerInput {
            move_axes: Vec2::new(1.0, 1.0),
            ..PlayerInput::default()
        };
        let speed = |player: &Player| player.velocity_m_s.with_y(0.0).length();
        let walking: Player = run_s(&walk, 1.0, 0.5);
        assert!((speed(&walking) - movement.walk_m_per_s).abs() < SPEED_TOLERANCE_M_S);
        let sprint: PlayerInput = PlayerInput {
            is_sprint_held: true,
            ..walk
        };
        let sprinting: Player = run_s(&sprint, 1.0, 0.5);
        assert!((speed(&sprinting) - movement.sprint_m_per_s).abs() < SPEED_TOLERANCE_M_S);
        let crouch: PlayerInput = PlayerInput {
            is_crouch_held: true,
            ..sprint
        };
        let crouching: Player = run_s(&crouch, 1.0, 0.5);
        let crouch_m_s: f32 = movement.walk_m_per_s * movement.settings.crouch_speed_frac as f32;
        assert!((speed(&crouching) - crouch_m_s).abs() < SPEED_TOLERANCE_M_S);
        let mut hit: Player = Player::new(Vec3::ZERO, 0.0);
        hit.slow_down(&movement);
        hit.update(&walk, &movement, &open_ground(), &[], 1.0 / 120.0);
        let slow_m_s: f32 = movement.walk_m_per_s * movement.when_hit.slow_speed_frac as f32;
        assert!((speed(&hit) - slow_m_s).abs() < SPEED_TOLERANCE_M_S);
    }

    /// `test_sprinting_never_skips_through_a_thin_rail` checks a sprinter stops at a 6 cm rail,
    /// even when slow frames move them farther in one frame than their body is wide.
    #[test]
    fn test_sprinting_never_skips_through_a_thin_rail() {
        let rail_z_m: f32 = -3.0;
        let solids: [Aabb; 2] = [
            open_ground()[0],
            Aabb {
                min_m: Vec3::new(-5.0, 0.0, rail_z_m - 0.06),
                max_m: Vec3::new(5.0, 2.5, rail_z_m),
            },
        ];
        let sprint: PlayerInput = PlayerInput {
            move_axes: Vec2::Y,
            is_sprint_held: true,
            ..PlayerInput::default()
        };
        let movement: Movement = forest_movement();
        for dt_s in [1.0 / 60.0, 0.05] {
            let mut player: Player = Player::new(Vec3::ZERO, 0.0);
            for _ in 0..(1.0 / dt_s) as usize {
                player.update(&sprint, &movement, &solids, &[], dt_s);
            }
            assert!(
                player.feet_m.z > rail_z_m,
                "went through at {dt_s} s a frame"
            );
        }
    }

    /// `test_crouching_lowers_the_eye_and_waits_for_room` checks crouching takes `crouch_s` to lower
    /// the eye to its share of the standing eye, and that standing up waits under a low roof.
    #[test]
    fn test_crouching_lowers_the_eye_and_waits_for_room() {
        let movement: Movement = forest_movement();
        let crouch: PlayerInput = PlayerInput {
            is_crouch_held: true,
            ..PlayerInput::default()
        };
        let standing_eye_m: f32 = run_s(&PlayerInput::default(), 1.0, 0.5).eye_position_m().y;
        let crouched: Player = run_s(&crouch, 1.0, movement.settings.crouch_s as f32 + 0.01);
        let crouched_eye_m: f32 = standing_eye_m - movement.get_crouch_drop_m(1.0);
        assert!((crouched.eye_position_m().y - crouched_eye_m).abs() < 0.01);
        let roof_m: f32 = crouched.height_m + 0.1;
        let solids: [Aabb; 2] = [
            open_ground()[0],
            Aabb {
                min_m: Vec3::new(-5.0, roof_m, -5.0),
                max_m: Vec3::new(5.0, roof_m + 0.2, 5.0),
            },
        ];
        let mut player: Player = crouched;
        for _ in 0..60 {
            player.update(&PlayerInput::default(), &movement, &solids, &[], 1.0 / 60.0);
        }
        assert!(player.height_m <= roof_m, "stood up into the roof");
        assert!(player.moves.crouch_frac > 0.5);
    }

    /// `test_sprinting_drains_energy_then_stops` checks sprint energy empties in `drain_s` of
    /// sprinting, as on Lunar Surface, and that an empty bar stops the sprint.
    #[test]
    fn test_sprinting_drains_energy_then_stops() {
        let sprint: PlayerInput = PlayerInput {
            move_axes: Vec2::Y,
            is_sprint_held: true,
            ..PlayerInput::default()
        };
        let drain_s: f32 = forest_movement().sprint_energy.drain_s as f32;
        let half: Player = run_s(&sprint, 1.0, drain_s * 0.5);
        assert!(half.moves.is_sprinting && (half.moves.energy_frac - 0.5).abs() < 0.01);
        let empty: Player = run_s(&sprint, 1.0, drain_s + 0.5);
        assert!(!empty.moves.is_sprinting && empty.moves.energy_frac == 0.0);
    }

    /// `test_a_jump_matches_lunar_surface` checks a standing jump rises as high and stays in the
    /// air as long as on Lunar Surface, computed from Lunar Surface's own server settings and map
    /// as its `set_settings` computes them, within a step of the simulation.
    #[test]
    fn test_a_jump_matches_lunar_surface() {
        let lunar_map_path: std::path::PathBuf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../common/maps/lunar_surface/map.json");
        let lunar_map: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(lunar_map_path).unwrap()).unwrap();
        let map_jump_s: f64 = 2.0 * lunar_map["player"]["jump_m_per_s"].as_f64().unwrap()
            / lunar_map["gravity_m_per_s2"].as_f64().unwrap();
        let movement = |key: &str| -> f64 {
            crate::settings::read_lunar_setting("server_settings.yaml", &["movement"], key)
        };
        let air_s: f64 = map_jump_s * movement("jump_time_per_map_jump_time");
        let height_m: f64 = movement("jump_height_m");
        let solids: [Aabb; 1] = open_ground();
        let movement: Movement = forest_movement();
        let dt_s: f32 = 1.0 / 240.0;
        let mut player: Player = Player::new(Vec3::ZERO, 0.0);
        player.update(&PlayerInput::default(), &movement, &solids, &[], dt_s);
        assert!(player.is_on_ground);
        let jump: PlayerInput = PlayerInput {
            is_jump_pressed: true,
            ..PlayerInput::default()
        };
        player.update(&jump, &movement, &solids, &[], dt_s);
        let (mut apex_m, mut step_count) = (player.feet_m.y, 1_usize);
        while !player.is_on_ground {
            player.update(&PlayerInput::default(), &movement, &solids, &[], dt_s);
            apex_m = apex_m.max(player.feet_m.y);
            step_count += 1;
            assert!(step_count < 10_000, "the jump never landed");
        }
        let measured_air_s: f64 = step_count as f64 * f64::from(dt_s);
        assert!(
            (measured_air_s - air_s).abs() < 2.0 * f64::from(dt_s),
            "{measured_air_s} s, not {air_s} s"
        );
        assert!(
            (f64::from(apex_m) - height_m).abs() < 0.02,
            "{apex_m} m, not {height_m} m"
        );
    }

    /// `test_energy_refills_faster_crouched` checks an empty bar refills in `refill_walking_s`
    /// standing and in `refill_crouching_s` crouched.
    #[test]
    fn test_energy_refills_faster_crouched() {
        let energy: SprintEnergySettings = forest_movement().sprint_energy;
        let standing: Player = run_s(
            &PlayerInput::default(),
            0.0,
            energy.refill_walking_s as f32 * 0.5,
        );
        assert!((standing.moves.energy_frac - 0.5).abs() < 0.01);
        let crouch: PlayerInput = PlayerInput {
            is_crouch_held: true,
            ..PlayerInput::default()
        };
        let crouched: Player = run_s(&crouch, 0.0, energy.refill_crouching_s as f32 * 0.5);
        assert!(
            (crouched.moves.energy_frac - 0.5).abs() < 0.01,
            "{}",
            crouched.moves.energy_frac
        );
    }

    /// `test_the_rules_are_the_shared_ones` checks every step's crouch, sprint energy, sprinting,
    /// head bob and slow are those of the shared `MoveState` stepped with the same keys and the
    /// same ground under the player (W-126 (same play in every level)).
    #[test]
    fn test_the_rules_are_the_shared_ones() {
        let movement: Movement = forest_movement();
        let solids: [Aabb; 1] = open_ground();
        let dt_s: f32 = 1.0 / 60.0;
        let rules: MoveRules = MoveRules {
            movement: &movement.settings,
            energy: &movement.sprint_energy,
            when_hit: &movement.when_hit,
            head_bob: &HEAD_BOB,
        };
        let mut player: Player = Player::new(Vec3::ZERO, 0.0);
        let mut shared: MoveState = MoveState::New();
        let legs: [(Vec2, bool, bool, bool, usize); 6] = [
            (Vec2::Y, false, false, false, 60),
            (Vec2::Y, true, false, false, 240),
            (Vec2::Y, true, true, false, 60),
            (Vec2::new(1.0, 1.0), false, false, true, 90),
            (Vec2::ZERO, false, false, false, 60),
            (Vec2::NEG_Y, false, true, false, 60),
        ];
        player.slow_down(&movement);
        shared.slowed_s = movement.when_hit.slow_s;
        for (move_axes, is_sprint_held, is_crouch_held, is_jump_pressed, step_count) in legs {
            let input: PlayerInput = PlayerInput {
                move_axes,
                is_sprint_held,
                is_crouch_held,
                is_jump_pressed,
                ..PlayerInput::default()
            };
            for _ in 0..step_count {
                let shared_input: MoveInput = MoveInput {
                    is_moving: move_axes != Vec2::ZERO,
                    is_sprint_held,
                    is_crouch_held,
                    is_flying: false,
                    is_on_ground: player.is_on_ground,
                };
                shared.step(&shared_input, &rules, f64::from(dt_s));
                player.update(&input, &movement, &solids, &[], dt_s);
                assert_eq!(player.moves, shared);
            }
        }
    }

    /// `test_respawn_waits_the_settings_delay` checks a dead player respawns once the delay it is
    /// given (the session's `respawn_wait_s`, here not the built-in 20 s) has passed.
    #[test]
    fn test_respawn_waits_the_settings_delay() {
        let respawn_delay_s: f32 = 5.0;
        let dt_s: f32 = 0.1;
        let movement: Movement = forest_movement();
        let solids: [Aabb; 1] = open_ground();
        let mut player: Player = Player::new(Vec3::ZERO, 0.0);
        player.take_damage(f32::MAX);
        let mut temp_waited_s: f32 = 0.0;
        while temp_waited_s < respawn_delay_s - 2.0 * dt_s {
            player.update(&PlayerInput::default(), &movement, &solids, &[], dt_s);
            temp_waited_s += dt_s;
        }
        assert!(!player.should_respawn(respawn_delay_s));
        let left_s: f32 = player.get_respawn_in_s(respawn_delay_s).expect("dead");
        assert!(
            (left_s - (respawn_delay_s - temp_waited_s)).abs() < 1e-3,
            "{left_s}"
        );
        for _ in 0..3 {
            player.update(&PlayerInput::default(), &movement, &solids, &[], dt_s);
        }
        assert!(player.should_respawn(respawn_delay_s));
    }
}
