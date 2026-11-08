//! `player` moves the player: mouse look, walking, sprinting, crouching, jumping, gravity, and
//! collision, plus the camera bob that goes with them.
//!
//! Example:
//!     player.update(&input, &solids, dt_s);
//!     let eye_m: Vec3 = player.eye_position_m();

use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};

use super::collision::{Aabb, MoveResult, move_body};
use super::world::LadderVolume;

/// `WALK_SPEED_M_S` is the walking speed.
const WALK_SPEED_M_S: f32 = 4.4;
/// `SPRINT_SPEED_M_S` is the sprinting speed.
const SPRINT_SPEED_M_S: f32 = 7.0;
/// `CROUCH_SPEED_M_S` is the crouched walking speed.
const CROUCH_SPEED_M_S: f32 = 2.2;
/// `ADS_SPEED_FRAC` scales speed while aiming down sights.
const ADS_SPEED_FRAC: f32 = 0.6;
/// `GROUND_ACCELERATION_PER_S` is how fast velocity approaches the target on the ground.
const GROUND_ACCELERATION_PER_S: f32 = 12.0;
/// `AIR_ACCELERATION_PER_S` is how fast velocity approaches the target in the air.
const AIR_ACCELERATION_PER_S: f32 = 2.0;
/// `JUMP_SPEED_M_S` is the upward speed of a jump.
const JUMP_SPEED_M_S: f32 = 5.4;
/// `GRAVITY_M_S2` is the downward acceleration.
const GRAVITY_M_S2: f32 = 16.0;
/// `STAND_HEIGHT_M` is the standing body height.
const STAND_HEIGHT_M: f32 = 1.8;
/// `CROUCH_HEIGHT_M` is the crouched body height.
const CROUCH_HEIGHT_M: f32 = 1.15;
/// `EYE_BELOW_TOP_M` is how far the eye is below the top of the body.
const EYE_BELOW_TOP_M: f32 = 0.12;
/// `BODY_RADIUS_M` is half the body's width.
const BODY_RADIUS_M: f32 = 0.3;
/// `CROUCH_RATE_PER_S` is how fast the body crouches and stands.
const CROUCH_RATE_PER_S: f32 = 9.0;
/// `MAX_PITCH_RAD` limits looking up and down.
const MAX_PITCH_RAD: f32 = 1.54;
/// `MAX_HEALTH` is full health.
pub const MAX_HEALTH: f32 = 100.0;
/// `SPRINT_DRAIN_S`, `SPRINT_REFILL_CROUCHING_S` and `SPRINT_REFILL_WALKING_S` are the Lunar Surface
/// demo's `sprint_energy` settings: seconds of sprinting that empty a full bar, and seconds to refill
/// an empty one while crouched and while walking or standing upright.
const SPRINT_DRAIN_S: f32 = 7.5;
const SPRINT_REFILL_CROUCHING_S: f32 = 7.5;
const SPRINT_REFILL_WALKING_S: f32 = 15.0;
/// `RESPAWN_DELAY_S` is how long the player stays dead.
const RESPAWN_DELAY_S: f32 = 3.0;
/// `REGEN_DELAY_S` is how long after being hit health starts to regenerate.
const REGEN_DELAY_S: f32 = 5.0;
/// `REGEN_RATE_PER_S` is the regeneration speed.
const REGEN_RATE_PER_S: f32 = 12.0;
/// `STRIDE_M` is the distance per footstep.
const STRIDE_M: f32 = 1.9;
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

/// `PlayerInput` is one frame's player controls.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerInput {
    pub move_axes: Vec2,
    pub look_delta_rad: Vec2,
    pub is_sprint_held: bool,
    pub is_crouch_held: bool,
    pub is_jump_pressed: bool,
    pub is_aiming: bool,
}

/// `Player` is the player's body and view.
pub struct Player {
    pub feet_m: Vec3,
    pub velocity_m_s: Vec3,
    pub yaw_rad: f32,
    pub pitch_rad: f32,
    pub recoil_pitch_rad: f32,
    pub is_on_ground: bool,
    pub height_m: f32,
    pub health: f32,
    pub dead_time_s: Option<f32>,
    pub since_hit_s: f32,
    pub stride_phase_rad: f32,
    pub stride_distance_m: f32,
    pub landing_dip_m: f32,
    pub is_sprinting: bool,
    pub energy_frac: f32,
    pub did_step: bool,
    pub is_climbing: bool,
}

impl Player {
    /// `new` makes a player standing at a spawn point.
    pub fn new(feet_m: Vec3, yaw_rad: f32) -> Player {
        Player {
            feet_m,
            velocity_m_s: Vec3::ZERO,
            yaw_rad,
            pitch_rad: 0.0,
            recoil_pitch_rad: 0.0,
            is_on_ground: false,
            height_m: STAND_HEIGHT_M,
            health: MAX_HEALTH,
            dead_time_s: None,
            since_hit_s: REGEN_DELAY_S,
            stride_phase_rad: 0.0,
            stride_distance_m: 0.0,
            landing_dip_m: 0.0,
            is_sprinting: false,
            energy_frac: 1.0,
            did_step: false,
            is_climbing: false,
        }
    }

    /// `half_extent_m` is half the body's size.
    fn half_extent_m(height_m: f32) -> Vec3 {
        Vec3::new(BODY_RADIUS_M, height_m * 0.5, BODY_RADIUS_M)
    }

    /// `rotation` is the view rotation, including recoil.
    pub fn rotation(&self) -> Quat {
        let pitch_rad: f32 =
            (self.pitch_rad + self.recoil_pitch_rad).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);
        Quat::from_euler(EulerRot::YXZ, self.yaw_rad, pitch_rad, 0.0)
    }

    /// `eye_position_m` is the camera position, with walking bob and landing dip.
    pub fn eye_position_m(&self) -> Vec3 {
        let speed_frac: f32 = (self.velocity_m_s.truncate_y().length() / SPRINT_SPEED_M_S).min(1.0);
        let bob_m: f32 = if self.is_on_ground {
            (self.stride_phase_rad * 2.0).sin().abs() * 0.035 * speed_frac
        } else {
            0.0
        };
        self.feet_m + Vec3::Y * (self.height_m - EYE_BELOW_TOP_M - self.landing_dip_m + bob_m)
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

    /// `should_respawn` says whether the respawn delay has passed.
    pub fn should_respawn(&self) -> bool {
        self.dead_time_s
            .is_some_and(|time_s: f32| time_s > RESPAWN_DELAY_S)
    }

    /// `update` applies one frame of input and physics.
    pub fn update(
        &mut self,
        input: &PlayerInput,
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

        let target_height_m: f32 = if input.is_crouch_held {
            CROUCH_HEIGHT_M
        } else {
            STAND_HEIGHT_M
        };
        let mut new_height_m: f32 = self.height_m
            + (target_height_m - self.height_m)
                .clamp(-CROUCH_RATE_PER_S * dt_s, CROUCH_RATE_PER_S * dt_s);
        if new_height_m > self.height_m {
            let grown: Aabb = Aabb {
                min_m: self.feet_m - Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M) + Vec3::Y * 0.01,
                max_m: self.feet_m + Vec3::new(BODY_RADIUS_M, new_height_m, BODY_RADIUS_M),
            };
            if solids.iter().any(|solid: &Aabb| solid.overlaps(&grown)) {
                new_height_m = self.height_m;
            }
        }
        self.height_m = new_height_m;
        let is_crouched: bool = self.height_m < STAND_HEIGHT_M - 0.05;

        let forward: Vec3 = Quat::from_rotation_y(self.yaw_rad) * Vec3::NEG_Z;
        let right: Vec3 = Quat::from_rotation_y(self.yaw_rad) * Vec3::X;
        let axes: Vec2 = input.move_axes.clamp_length_max(1.0);
        let wants_to_sprint: bool =
            input.is_sprint_held && axes.y > 0.1 && !is_crouched && !input.is_aiming;
        self.is_sprinting = wants_to_sprint && self.energy_frac > 0.0;
        let energy_change_per_s: f32 = if wants_to_sprint {
            -1.0 / SPRINT_DRAIN_S
        } else if is_crouched {
            1.0 / SPRINT_REFILL_CROUCHING_S
        } else {
            1.0 / SPRINT_REFILL_WALKING_S
        };
        self.energy_frac = (self.energy_frac + energy_change_per_s * dt_s).clamp(0.0, 1.0);
        let mut speed_m_s: f32 = if is_crouched {
            CROUCH_SPEED_M_S
        } else if self.is_sprinting {
            SPRINT_SPEED_M_S
        } else {
            WALK_SPEED_M_S
        };
        if input.is_aiming {
            speed_m_s *= ADS_SPEED_FRAC;
        }
        let target_m_s: Vec3 = (forward * axes.y + right * axes.x) * speed_m_s;
        let acceleration_per_s: f32 = if self.is_on_ground {
            GROUND_ACCELERATION_PER_S
        } else {
            AIR_ACCELERATION_PER_S
        };
        let blend_frac: f32 = 1.0 - (-acceleration_per_s * dt_s).exp();
        let horizontal: Vec3 = self.velocity_m_s.with_y(0.0).lerp(target_m_s, blend_frac);
        self.velocity_m_s = horizontal.with_y(self.velocity_m_s.y - GRAVITY_M_S2 * dt_s);
        if input.is_jump_pressed && self.is_on_ground {
            self.velocity_m_s.y = JUMP_SPEED_M_S;
        }
        let body: Aabb = Aabb {
            min_m: self.feet_m - Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M),
            max_m: self.feet_m + Vec3::new(BODY_RADIUS_M, self.height_m, BODY_RADIUS_M),
        };
        let ladder: Option<&LadderVolume> = ladders.iter().find(|l| l.aabb.overlaps(&body));
        self.is_climbing = ladder.is_some_and(|ladder| {
            let facing: Vec3 = -ladder.frame.z_axis.truncate();
            let walking_in: bool = target_m_s.dot(facing) > LADDER_FACING_DOT * speed_m_s;
            !input.is_jump_pressed && (self.is_climbing || !self.is_on_ground || walking_in)
        });
        if self.is_climbing {
            let down_sign: f32 = if self.pitch_rad < CLIMB_DOWN_PITCH_RAD {
                -1.0
            } else {
                1.0
            };
            let climb_m_s: f32 = axes.y * down_sign * CLIMB_SPEED_M_S
                - f32::from(u8::from(input.is_crouch_held)) * CLIMB_SPEED_M_S;
            let ladder: &LadderVolume = ladder.expect("a climber is on a ladder");
            let facing: Vec3 = -ladder.frame.z_axis.truncate();
            let is_at_top: bool = self.feet_m.y > ladder.aabb.max_m.y - CLIMB_TOP_STEP_M;
            let along_m_s: Vec3 = if is_at_top {
                target_m_s
            } else {
                (target_m_s - facing * target_m_s.dot(facing)) * CLIMB_HORIZONTAL_FRAC
            };
            self.velocity_m_s = along_m_s.with_y(climb_m_s.max(-CLIMB_SPEED_M_S * 1.5));
        }

        let fall_speed_m_s: f32 = -self.velocity_m_s.y;
        let was_on_ground: bool = self.is_on_ground;
        let result: MoveResult = move_body(
            solids,
            self.feet_m,
            Player::half_extent_m(self.height_m),
            self.velocity_m_s * dt_s,
            self.is_on_ground || self.is_climbing,
        );
        let moved_m: f32 = (result.position_m - self.feet_m).with_y(0.0).length();
        self.feet_m = result.position_m;
        self.is_on_ground = result.is_on_ground;
        if result.is_on_ground || result.did_hit_ceiling {
            self.velocity_m_s.y = self.velocity_m_s.y.clamp(-0.5, 0.0);
            if result.did_hit_ceiling {
                self.velocity_m_s.y = 0.0;
            }
        }
        if self.is_on_ground && !was_on_ground && fall_speed_m_s > 3.0 {
            self.landing_dip_m = (fall_speed_m_s * 0.012).min(0.12);
        }
        self.landing_dip_m = (self.landing_dip_m - dt_s * 0.5).max(0.0);
        self.did_step = false;
        if self.is_on_ground {
            self.stride_phase_rad += moved_m / STRIDE_M * std::f32::consts::PI;
            self.stride_distance_m += moved_m;
            if self.stride_distance_m > STRIDE_M * 0.5 {
                self.stride_distance_m = 0.0;
                self.did_step = true;
            }
        }
        self.recoil_pitch_rad *= (-8.0 * dt_s).exp();
    }
}

/// `Vec3Ext` adds a helper for dropping the vertical component.
trait Vec3Ext {
    /// `truncate_y` returns the horizontal part of a vector.
    fn truncate_y(self) -> Vec2;
}

impl Vec3Ext for Vec3 {
    fn truncate_y(self) -> Vec2 {
        Vec2::new(self.x, self.z)
    }
}

/// `tests` checks climbing a rope ladder onto a deck.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | input at a ladder's foot | walk into it (climbs to the deck), walk past it sideways (stays on the ground) | `test_walking_into_a_ladder_climbs_onto_the_deck`, `test_walking_beside_a_ladder_stays_down` |
#[cfg(test)]
mod tests {
    use glam::Mat4;

    use super::*;

    /// `DECK_TOP_M` is the test deck's height.
    const DECK_TOP_M: f32 = 7.0;

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
        let dt_s: f32 = 1.0 / 120.0;
        for _ in 0..(seconds / dt_s) as usize {
            player.update(&input, &solids, &ladders, dt_s);
        }
        player.feet_m
    }

    /// `test_walking_into_a_ladder_climbs_onto_the_deck` checks holding W at a ladder's foot
    /// ends on the deck above it.
    #[test]
    fn test_walking_into_a_ladder_climbs_onto_the_deck() {
        let feet_m: Vec3 = walk_s(Vec3::new(11.0, 0.0, 48.5), Vec2::Y, 4.0);
        assert!((feet_m.y - DECK_TOP_M).abs() < 0.05, "ended at {feet_m}");
        assert!(feet_m.z < 46.5, "ended at {feet_m}");
    }

    /// `test_walking_beside_a_ladder_stays_down` checks walking past a ladder doesn't climb it.
    #[test]
    fn test_walking_beside_a_ladder_stays_down() {
        let feet_m: Vec3 = walk_s(Vec3::new(14.0, 0.0, 48.5), Vec2::Y, 2.0);
        assert!(feet_m.y < 0.05, "ended at {feet_m}");
    }
}
