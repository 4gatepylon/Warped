//! `weapon` is the energy revolver's state machine (fire, reload, aim) and the procedural
//! animation of the first-person viewmodel: sway, bob, recoil kick, and a reload dip.
//!
//! Example:
//!     let shots: u32 = weapon.update(&weapon_input, &player, dt_s);
//!     let local: Mat4 = weapon.viewmodel_local(&player, time_s);

use glam::{EulerRot, Mat4, Quat, Vec2, Vec3};

use super::player::Player;

/// `MAGAZINE_SIZE` is how many shots one charge holds.
pub const MAGAZINE_SIZE: u32 = 12;
/// `FIRE_INTERVAL_S` is the time between shots.
const FIRE_INTERVAL_S: f32 = 0.17;
/// `RELOAD_DURATION_S` is how long a reload takes.
pub const RELOAD_DURATION_S: f32 = 1.3;
/// `ADS_RATE_PER_S` is how fast the weapon comes up to the eye.
const ADS_RATE_PER_S: f32 = 9.0;
/// `HIP_SPREAD_RAD` is the shot spread from the hip.
const HIP_SPREAD_RAD: f32 = 0.014;
/// `ADS_SPREAD_RAD` is the shot spread when aiming down sights.
const ADS_SPREAD_RAD: f32 = 0.0015;
/// `MOVING_SPREAD_RAD` is extra spread at full running speed.
const MOVING_SPREAD_RAD: f32 = 0.02;
/// `RECOIL_PITCH_RAD` is how far one shot kicks the view up.
const RECOIL_PITCH_RAD: f32 = 0.022;
/// `HIP_FOV_Y_RAD` is the camera's vertical field of view from the hip.
pub const HIP_FOV_Y_RAD: f32 = 1.2;
/// `ADS_FOV_Y_RAD` is the camera's vertical field of view when aiming.
pub const ADS_FOV_Y_RAD: f32 = 0.85;

/// `WeaponInput` is one frame's weapon controls.
#[derive(Clone, Copy, Debug, Default)]
pub struct WeaponInput {
    pub is_trigger_held: bool,
    pub is_reload_pressed: bool,
    pub is_aim_held: bool,
}

/// `WeaponEvent` is something the weapon did this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponEvent {
    Fired,
    ReloadStarted,
}

/// `ViewmodelPose` places the weapon model relative to the camera.
#[derive(Clone, Copy, Debug)]
pub struct ViewmodelPose {
    pub hip_offset_m: Vec3,
    pub ads_offset_m: Vec3,
    pub base_rotation: Quat,
}

/// `Weapon` is the revolver's state.
pub struct Weapon {
    pub ammo: u32,
    pub reload_time_s: Option<f32>,
    pub ads_frac: f32,
    pub since_shot_s: f32,
    pub kick_frac: f32,
    pub sway: Vec2,
    pub sprint_frac: f32,
    was_trigger_held: bool,
    noise_state: u32,
}

impl Weapon {
    /// `new` makes a fully charged weapon.
    pub fn new() -> Weapon {
        Weapon {
            ammo: MAGAZINE_SIZE,
            reload_time_s: None,
            ads_frac: 0.0,
            since_shot_s: 10.0,
            kick_frac: 0.0,
            sway: Vec2::ZERO,
            sprint_frac: 0.0,
            was_trigger_held: false,
            noise_state: 0x1234_5678,
        }
    }

    /// `random_signed` returns a pseudo-random number in [-1, 1].
    fn random_signed(&mut self) -> f32 {
        self.noise_state ^= self.noise_state << 13;
        self.noise_state ^= self.noise_state >> 17;
        self.noise_state ^= self.noise_state << 5;
        self.noise_state as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// `shot_direction` returns the direction of a shot, with spread.
    pub fn shot_direction(&mut self, player: &Player) -> Vec3 {
        let speed_frac: f32 = (player.velocity_m_s.with_y(0.0).length() / 7.0).min(1.0);
        let spread_rad: f32 = HIP_SPREAD_RAD
            + (ADS_SPREAD_RAD - HIP_SPREAD_RAD) * self.ads_frac
            + MOVING_SPREAD_RAD * speed_frac * (1.0 - self.ads_frac * 0.7)
            + if player.is_on_ground {
                0.0
            } else {
                MOVING_SPREAD_RAD
            };
        let offset: Vec2 = Vec2::new(self.random_signed(), self.random_signed()) * spread_rad;
        (player.rotation() * Vec3::new(offset.x, offset.y, -1.0)).normalize()
    }

    /// `fov_y_rad` is the camera field of view for the current aim.
    pub fn fov_y_rad(&self) -> f32 {
        let eased: f32 = self.ads_frac * self.ads_frac * (3.0 - 2.0 * self.ads_frac);
        HIP_FOV_Y_RAD + (ADS_FOV_Y_RAD - HIP_FOV_Y_RAD) * eased
    }

    /// `update` advances timers and returns what the weapon did.
    pub fn update(
        &mut self,
        input: &WeaponInput,
        player: &mut Player,
        look_delta_rad: Vec2,
        dt_s: f32,
    ) -> Option<WeaponEvent> {
        self.since_shot_s += dt_s;
        self.kick_frac *= (-14.0 * dt_s).exp();
        let sway_target: Vec2 = (-look_delta_rad / dt_s.max(1e-4) * 0.012).clamp_length_max(1.0);
        self.sway = self.sway.lerp(sway_target, 1.0 - (-10.0 * dt_s).exp());
        let sprint_target: f32 = if player.is_sprinting { 1.0 } else { 0.0 };
        self.sprint_frac += (sprint_target - self.sprint_frac) * (1.0 - (-8.0 * dt_s).exp());
        let is_reloading: bool = self.reload_time_s.is_some();
        let ads_target: f32 = if input.is_aim_held && !is_reloading && player.is_alive() {
            1.0
        } else {
            0.0
        };
        self.ads_frac = (self.ads_frac
            + (ads_target - self.ads_frac).signum() * ADS_RATE_PER_S * dt_s)
            .clamp(0.0, 1.0);
        if (self.ads_frac - ads_target).abs() < 0.02 {
            self.ads_frac = ads_target;
        }
        let was_trigger_held: bool = self.was_trigger_held;
        self.was_trigger_held = input.is_trigger_held;
        if !player.is_alive() {
            return None;
        }
        if let Some(time_s) = &mut self.reload_time_s {
            *time_s += dt_s;
            if *time_s >= RELOAD_DURATION_S {
                self.reload_time_s = None;
                self.ammo = MAGAZINE_SIZE;
            }
            return None;
        }
        if input.is_reload_pressed && self.ammo < MAGAZINE_SIZE {
            self.reload_time_s = Some(0.0);
            return Some(WeaponEvent::ReloadStarted);
        }
        if !input.is_trigger_held || self.since_shot_s < FIRE_INTERVAL_S || self.sprint_frac > 0.5 {
            return None;
        }
        if self.ammo == 0 {
            if !was_trigger_held {
                self.since_shot_s = 0.0;
                self.reload_time_s = Some(0.0);
                return Some(WeaponEvent::ReloadStarted);
            }
            return None;
        }
        self.ammo -= 1;
        self.since_shot_s = 0.0;
        self.kick_frac = 1.0;
        Some(WeaponEvent::Fired)
    }

    /// `apply_recoil` kicks the view after a shot has been traced, so the shot itself goes where
    /// the player aimed.
    pub fn apply_recoil(&mut self, player: &mut Player) {
        let recoil_scale: f32 = 1.0 - self.ads_frac * 0.45;
        player.recoil_pitch_rad += RECOIL_PITCH_RAD * recoil_scale;
        player.yaw_rad += self.random_signed() * RECOIL_PITCH_RAD * 0.3 * recoil_scale;
    }

    /// `reload_frac` is how far through a reload the weapon is (0 when not reloading).
    pub fn reload_frac(&self) -> f32 {
        self.reload_time_s
            .map_or(0.0, |time_s: f32| time_s / RELOAD_DURATION_S)
    }

    /// `viewmodel_local` returns the weapon's transform relative to the camera.
    ///
    /// Example:
    ///     let world: Mat4 = player.camera_world() * weapon.viewmodel_local(&player, &pose, t);
    pub fn viewmodel_local(&self, player: &Player, pose: &ViewmodelPose, time_s: f32) -> Mat4 {
        let ads: f32 = self.ads_frac * self.ads_frac * (3.0 - 2.0 * self.ads_frac);
        let hip: f32 = 1.0 - ads;
        let speed_frac: f32 = (player.velocity_m_s.with_y(0.0).length() / 7.0).min(1.0);
        let phase: f32 = player.stride_phase_rad;
        let bob_amount: f32 =
            if player.is_on_ground { speed_frac } else { 0.0 } * (0.25 + hip * 0.75);
        let bob: Vec3 =
            Vec3::new(phase.sin() * 0.012, -(phase * 2.0).sin().abs() * 0.01, 0.0) * bob_amount;
        let breathe: Vec3 = Vec3::new(0.0, (time_s * 1.4).sin() * 0.0015, 0.0) * (0.3 + hip);
        let sway: Vec3 =
            Vec3::new(self.sway.x * 0.018, self.sway.y * 0.014, 0.0) * (0.25 + hip * 0.75);
        let kick: f32 = self.kick_frac;
        let kick_offset: Vec3 = Vec3::new(0.0, 0.004, 0.05 + 0.03 * hip) * kick;
        let reload: f32 = (self.reload_frac() * std::f32::consts::PI).sin();
        let reload_offset: Vec3 = Vec3::new(0.02, -0.09, 0.03) * reload;
        let sprint: f32 = self.sprint_frac;
        let sprint_offset: Vec3 = Vec3::new(-0.02, -0.03, 0.02) * sprint;
        let landing: Vec3 = Vec3::new(0.0, -player.landing_dip_m * 0.4, 0.0);
        let offset: Vec3 = pose.hip_offset_m.lerp(pose.ads_offset_m, ads)
            + bob
            + breathe
            + sway
            + kick_offset
            + reload_offset
            + sprint_offset
            + landing;
        let rotation: Quat = Quat::from_euler(
            EulerRot::YXZ,
            self.sway.x * 0.05 * (0.3 + hip) + sprint * 0.5 + phase.sin() * 0.01 * bob_amount,
            self.sway.y * 0.04 * (0.3 + hip) + kick * (0.08 + 0.06 * hip)
                - reload * 0.55
                - sprint * 0.25,
            self.sway.x * 0.06 * hip + reload * 0.6 + sprint * 0.2,
        );
        Mat4::from_rotation_translation(rotation, offset) * Mat4::from_quat(pose.base_rotation)
    }
}
