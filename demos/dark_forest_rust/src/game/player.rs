//! `player` moves the player as Lunar Surface's `VoxelPlayer::step` moves it (W-126, same play in
//! every level): mouse look, walking, sprinting and its energy, crouching, jumping, gravity, a hit's
//! slowing and the head bob, with the numbers of `Movement` (the map's own times the server
//! settings, as Lunar's `VoxelWorld::set_settings` works them out); plus the forest's own rope
//! ladders, collision and landing dip.
//!
//! Example:
//!     player.update(&input, &movement, &solids, &ladders, dt_s);
//!     let eye_m: Vec3 = player.eye_position_m();

use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};

use super::collision::{Aabb, MoveResult, move_body};
use super::world::LadderVolume;
use crate::map::PlayerSpec;
use crate::settings::{
    HEAD_BOB, MovementSettings, ServerSettings, SprintEnergySettings, WhenHitSettings,
};

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
/// `RESPAWN_DELAY_S` is how long the player stays dead.
const RESPAWN_DELAY_S: f32 = 3.0;
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

/// `Player` is the player's body and view.
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
    pub bob_phase_rad: f64,
    pub bob_frac: f64,
    pub crouch_frac: f32,
    pub slowed_s: f32,
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
            is_on_ground: false,
            height_m: STAND_HEIGHT_M,
            health: MAX_HEALTH,
            dead_time_s: None,
            since_hit_s: REGEN_DELAY_S,
            bob_phase_rad: 0.0,
            bob_frac: 0.0,
            crouch_frac: 0.0,
            slowed_s: 0.0,
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
        let depth_m: f64 = if self.is_sprinting {
            HEAD_BOB.sprint_depth_m
        } else {
            HEAD_BOB.walk_depth_m
        };
        -self.bob_frac * depth_m * self.bob_phase_rad.sin().abs()
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

    /// `get_respawn_in_s` is how long until a dead player respawns (`None` while alive).
    ///
    /// Example: `if let Some(left_s) = player.get_respawn_in_s() { ... }`
    pub fn get_respawn_in_s(&self) -> Option<f32> {
        self.dead_time_s
            .map(|time_s: f32| (RESPAWN_DELAY_S - time_s).max(0.0))
    }

    /// `update` applies one frame of input and physics as Lunar Surface's `VoxelPlayer::step`
    /// does, with `movement`'s numbers: crouching toward `crouch_frac` 1 over `crouch_s` (the body
    /// and eye lowering together; standing up waits for room), sprinting (any direction, not
    /// crouched) while energy lasts, the head bob, a speed of walking or sprinting times the
    /// crouching share and a hit's slowing, set at once on the ground and in the air, a jump, gravity
    /// up to `MAX_FALL_SPEED_M_PER_S`; then the forest's rope ladders, collision (stepping up
    /// ledges, `collision::STEP_HEIGHT_M`) and landing dip. A footstep (`did_step`) falls at each
    /// dip of the head bob.
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

        let settings: &MovementSettings = &movement.settings;
        let crouch_step: f32 = dt_s / settings.crouch_s as f32;
        let crouch_target: f32 = f32::from(u8::from(input.is_crouch_held));
        let new_crouch_frac: f32 =
            self.crouch_frac + (crouch_target - self.crouch_frac).clamp(-crouch_step, crouch_step);
        let new_height_m: f32 = STAND_HEIGHT_M - movement.get_crouch_drop_m(new_crouch_frac);
        let grown: Aabb = Aabb {
            min_m: self.feet_m - Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M) + Vec3::Y * 0.01,
            max_m: self.feet_m + Vec3::new(BODY_RADIUS_M, new_height_m, BODY_RADIUS_M),
        };
        if new_height_m <= self.height_m
            || !solids.iter().any(|solid: &Aabb| solid.overlaps(&grown))
        {
            (self.crouch_frac, self.height_m) = (new_crouch_frac, new_height_m);
        }

        let forward: Vec3 = Quat::from_rotation_y(self.yaw_rad) * Vec3::NEG_Z;
        let right: Vec3 = Quat::from_rotation_y(self.yaw_rad) * Vec3::X;
        let axes: Vec2 = input.move_axes.clamp_length_max(1.0);
        let is_moving: bool = axes.length() > 0.0;
        let energy: &SprintEnergySettings = &movement.sprint_energy;
        let wants_to_sprint: bool = input.is_sprint_held && !input.is_crouch_held && is_moving;
        self.is_sprinting = wants_to_sprint && self.energy_frac > 0.0;
        let energy_change_per_s: f64 = match (wants_to_sprint, input.is_crouch_held) {
            (true, _) => -1.0 / energy.drain_s,
            (false, true) => 1.0 / energy.refill_crouching_s,
            (false, false) => 1.0 / energy.refill_walking_s,
        };
        self.energy_frac = (self.energy_frac + energy_change_per_s as f32 * dt_s).clamp(0.0, 1.0);
        let is_walking: bool = is_moving && self.is_on_ground;
        let fade_frac: f64 = f64::from(dt_s) / HEAD_BOB.fade_s;
        self.bob_frac +=
            (f64::from(u8::from(is_walking)) - self.bob_frac).clamp(-fade_frac, fade_frac);
        self.did_step = false;
        if is_walking {
            let steps_per_s: f64 = if self.is_sprinting {
                HEAD_BOB.sprint_steps_per_s
            } else {
                HEAD_BOB.walk_steps_per_s
            };
            let phase_rad: f64 =
                self.bob_phase_rad + std::f64::consts::TAU * steps_per_s * f64::from(dt_s);
            self.did_step = (phase_rad / std::f64::consts::PI).floor()
                > (self.bob_phase_rad / std::f64::consts::PI).floor();
            self.bob_phase_rad = phase_rad % std::f64::consts::TAU;
        }
        let slow_frac: f32 = if self.slowed_s > 0.0 {
            movement.when_hit.slow_speed_frac as f32
        } else {
            1.0
        } * if input.is_crouch_held {
            settings.crouch_speed_frac as f32
        } else {
            1.0
        };
        self.slowed_s = (self.slowed_s - dt_s).max(0.0);
        let speed_m_s: f32 = slow_frac
            * if self.is_sprinting {
                movement.sprint_m_per_s
            } else {
                movement.walk_m_per_s
            };
        let direction: Vec3 = forward * axes.y + right * axes.x;
        let target_m_s: Vec3 = direction * speed_m_s;
        self.velocity_m_s = target_m_s.with_y(
            (self.velocity_m_s.y - movement.gravity_m_per_s2 * dt_s).max(-MAX_FALL_SPEED_M_PER_S),
        );
        if input.is_jump_pressed && self.is_on_ground {
            self.velocity_m_s.y = movement.jump_m_per_s;
        }
        let body: Aabb = Aabb {
            min_m: self.feet_m - Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M),
            max_m: self.feet_m + Vec3::new(BODY_RADIUS_M, self.height_m, BODY_RADIUS_M),
        };
        let ladder: Option<&LadderVolume> = ladders.iter().find(|l| l.aabb.overlaps(&body));
        let ladder_m_s: Vec3 = direction * LADDER_WALK_M_S;
        self.is_climbing = ladder.is_some_and(|ladder| {
            let facing: Vec3 = -ladder.frame.z_axis.truncate();
            let walking_in: bool = ladder_m_s.dot(facing) > LADDER_FACING_DOT * LADDER_WALK_M_S;
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
                ladder_m_s
            } else {
                (ladder_m_s - facing * ladder_m_s.dot(facing)) * CLIMB_HORIZONTAL_FRAC
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
    }

    /// `slow_down` starts a hit's slowing (`Movement::when_hit`'s `slow_s`), as Lunar Surface slows
    /// a player it hits.
    ///
    /// Example: `player.slow_down(&movement);`
    pub fn slow_down(&mut self, movement: &Movement) {
        self.slowed_s = movement.when_hit.slow_s as f32;
    }
}

/// `tests` checks climbing a rope ladder onto a deck, sprint energy, movement against Lunar
/// Surface's, the jump and crouching.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | movement's numbers | walking, sprinting, jump speed and gravity on this map, against Lunar Surface's on its own map with its own settings | `test_movement_matches_lunar_surface` |
/// | gait on open ground | walking, sprinting, crouching (speed), slowed by a hit | `test_speeds_are_lunar_surfaces` |
/// | jump | a standing jump on flat ground, against Lunar Surface's own settings and map | `test_a_jump_matches_lunar_surface` |
/// | crouching | down (eye at its share of the standing eye, in `crouch_s`), up again, up under a low roof (waits) | `test_crouching_lowers_the_eye_and_waits_for_room` |
/// | input at a ladder's foot | walk into it (climbs to the deck), walk past it sideways (stays on the ground) | `test_walking_into_a_ladder_climbs_onto_the_deck`, `test_walking_beside_a_ladder_stays_down` |
/// | sprint energy while sprinting | some left, empty | `test_sprinting_drains_energy_then_stops` |
/// | sprint energy while not sprinting | standing, crouched | `test_energy_refills_faster_crouched` |
#[cfg(test)]
mod tests {
    use glam::Mat4;

    use super::*;
    use crate::map::{self, MapFile};

    /// `DECK_TOP_M` is the test deck's height.
    const DECK_TOP_M: f32 = 7.0;
    /// `SPEED_TOLERANCE_M_S` is how close a measured speed comes to its setting.
    const SPEED_TOLERANCE_M_S: f32 = 0.01;

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
        player.energy_frac = energy_frac;
        let movement: Movement = forest_movement();
        let dt_s: f32 = 1.0 / 120.0;
        for _ in 0..(seconds / dt_s).round() as usize {
            player.update(input, &movement, &solids, &[], dt_s);
        }
        player
    }

    /// `test_movement_matches_lunar_surface` checks walking, sprinting, a jump's speed and gravity
    /// here are Lunar Surface's: its map's numbers times its own server settings.
    #[test]
    fn test_movement_matches_lunar_surface() {
        let (forest, lunar) = (forest_movement(), lunar_movement());
        assert_eq!(forest.walk_m_per_s, lunar.walk_m_per_s);
        assert_eq!(forest.sprint_m_per_s, lunar.sprint_m_per_s);
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
        assert!(player.crouch_frac > 0.5);
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
        assert!(half.is_sprinting && (half.energy_frac - 0.5).abs() < 0.01);
        let empty: Player = run_s(&sprint, 1.0, drain_s + 0.5);
        assert!(!empty.is_sprinting && empty.energy_frac == 0.0);
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
        assert!((standing.energy_frac - 0.5).abs() < 0.01);
        let crouch: PlayerInput = PlayerInput {
            is_crouch_held: true,
            ..PlayerInput::default()
        };
        let crouched: Player = run_s(&crouch, 0.0, energy.refill_crouching_s as f32 * 0.5);
        assert!(
            (crouched.energy_frac - 0.5).abs() < 0.01,
            "{}",
            crouched.energy_frac
        );
    }
}
