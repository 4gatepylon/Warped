//! `weapons.rs` is a player's two guns, as every level plays them (W-126, same play in every
//! level): gun 1, the sniper (a laser on for `sniper.shot_s` per click, then a reload), and gun 2,
//! the rifle (fires while held, heats up and overheats, recoils, its shots straying more as it
//! heats); aiming's zoom; how much a hit weighs; and where the gun in hand is drawn (`make_wand`:
//! its pose toward running, its dip and sway with each step, its jolt after a shot).
//!
//! Interface: `Gun`, `Weapons` (`New`, `switch_gun`, `is_firing`, `fire_sniper`, `tick`,
//! `update_aim`, `get_zoom`, `update_rifle`), `get_aim_zoom`, `take_hit_weight`, `make_wand`,
//! `make_wand_camera_from_muzzle`, `get_sway`, `ease_frac`, `MUZZLE_M`.
//!
//! Example: `let recoil: Option<[f64; 2]> = weapons.update_rifle(&settings.rifle, can_shoot, dt_s);`
//!
//! Must know:
//! - Gotchas: `Weapons` only keeps time and state; the level traces each shot in its own space,
//!   turns the view by a shot's recoil, and decides whether this player may shoot (`can_shoot`).

use glam::{DVec3, EulerRot, Mat3, Mat4, Vec3};

use crate::overlay::OverlayWand;
use crate::play_settings::{AimSettings, HeadBobSettings, RifleSettings, SniperSettings};
use crate::shot_noise::{self, ShotNoise, ShotOffset};
use crate::viewmodel::{BOLT_ACTION_RIFLE_MODELLED_LENGTH_M, GunModel};

/// `MUZZLE_M` is where the gun in hand's muzzle is in camera space while aiming (x right, y up,
/// -z ahead, meters), where the laser starts.
pub const MUZZLE_M: DVec3 = DVec3::new(0.187, -0.157, -0.555);
pub const WAND_RECOIL_M: f32 = 0.035;
pub const WAND_RECOIL_TILT_RAD: f32 = 0.12;
pub const WAND_LASER_SHAFT_HALF_M: [f32; 4] = [0.011, 0.011, 0.16, 0.0];
pub const WAND_LASER_SHAFT_COLOR: [f32; 4] = [0.2, 0.17, 0.22, 1.0];
pub const WAND_LASER_TIP_HALF_M: [f32; 4] = [0.016, 0.016, 0.028, 0.0];
pub const WAND_RIFLE_SHAFT_HALF_M: [f32; 4] = [0.024, 0.032, 0.17, 0.0];
pub const WAND_RIFLE_SHAFT_COLOR: [f32; 4] = [0.42, 0.45, 0.5, 1.0];
pub const WAND_RIFLE_TIP_HALF_M: [f32; 4] = [0.007, 0.007, 0.07, 0.0];
pub const WAND_JOG_CENTER_M: [f32; 3] = [0.22, -0.22, -0.34];
pub const WAND_AIM_YAW_RAD: f32 = 0.076;
pub const WAND_JOG_YAW_RAD: f32 = 0.35;
pub const WAND_AIM_PITCH_RAD: f32 = 0.064;
pub const WAND_JOG_PITCH_RAD: f32 = -0.35;
pub const WAND_JOG_ROLL_RAD: f32 = 0.5;
pub const WAND_JOG_SWING_RAD: f32 = 0.18;
pub const WAND_WALK_BOB_M: f32 = 0.006;
pub const WAND_JOG_BOB_M: f32 = 0.03;
pub const WAND_BLUE_TIP_COLOR: [f32; 4] = [0.35, 0.68, 1.0, 1.0];
pub const WAND_RED_TIP_COLOR: [f32; 4] = [1.0, 0.28, 0.14, 1.0];

/// `Gun` is the weapon a player holds: gun 1, the laser (a quarter-second shot per click; one
/// hit eliminates), or gun 2, the design's rifle (fires while the button is held, its beam
/// pulsing; two hits to the head or twelve to the body eliminate; overheats).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gun {
    Laser,
    Rifle,
}

impl Gun {
    /// `get_model` gives the model drawn in hand for this gun: the bolt-action rifle for the laser
    /// (the sniper) and the energy revolver for the rifle.
    ///
    /// Example: `let model: GunModel = Gun::Rifle.get_model();`
    pub fn get_model(self) -> GunModel {
        match self {
            Gun::Laser => GunModel::BoltActionRifle,
            Gun::Rifle => GunModel::Revolver,
        }
    }
}

/// `Weapons` is the state of a player's guns: the gun in hand; whether the laser is held on (the
/// house demo's scripted route) and how long the current shot still hits; the sniper's reload; the
/// trigger; the rifle's next shot, heat, overheating, whether its current shot has hit, its shot
/// count (its recoil alternates sides) and its recoil; aiming; each shot's noise, stray and
/// speckle; and the gun in hand's motion: its step phase, how much it steps, and how far it is in
/// its running pose.
pub struct Weapons {
    pub gun: Gun,
    pub is_laser_on: bool,
    pub shot_time_s: f64,
    pub aim_frac: f64,
    pub sniper_reload_s: f64,
    pub is_trigger_held: bool,
    pub rifle_next_shot_s: f64,
    pub rifle_heat_frac: f64,
    pub is_overheated: bool,
    pub has_rifle_shot_hit: bool,
    pub rifle_shot_count: u64,
    pub recoil_frac: f64,
    pub shot_noise: ShotNoise,
    pub shot_offset: ShotOffset,
    pub beam_speckle_frac: f64,
    pub gun_phase_rad: f64,
    pub step_frac: f64,
    pub run_pose_frac: f64,
}

impl Weapons {
    /// `New` is a player's guns at the start: the rifle in hand, cold, loaded and still, its
    /// shots strayed by `shot_noise`.
    ///
    /// Example: `let weapons = Weapons::New(ShotNoise::FromClock());`
    #[allow(non_snake_case)]
    pub fn New(shot_noise: ShotNoise) -> Weapons {
        Weapons {
            gun: Gun::Rifle,
            is_laser_on: false,
            shot_time_s: 0.0,
            aim_frac: 0.0,
            sniper_reload_s: 0.0,
            is_trigger_held: false,
            rifle_next_shot_s: 0.0,
            rifle_heat_frac: 0.0,
            is_overheated: false,
            has_rifle_shot_hit: false,
            rifle_shot_count: 0,
            recoil_frac: 0.0,
            shot_noise,
            shot_offset: ShotOffset::NONE,
            beam_speckle_frac: 0.0,
            gun_phase_rad: 0.0,
            step_frac: 0.0,
            run_pose_frac: 0.0,
        }
    }

    /// `switch_gun` takes `gun` in hand, ending the shot under way.
    ///
    /// Example: `weapons.switch_gun(Gun::Laser);`
    pub fn switch_gun(&mut self, gun: Gun) {
        self.gun = gun;
        self.shot_time_s = 0.0;
    }

    /// `is_firing` says whether a shot hits this frame: the laser is held on or a shot is under
    /// way, and this player `can_shoot`.
    ///
    /// Example: `let is_firing: bool = weapons.is_firing(true);`
    pub fn is_firing(&self, can_shoot: bool) -> bool {
        (self.is_laser_on || self.shot_time_s > 0.0) && can_shoot
    }

    /// `fire_sniper` fires the sniper: its shot hits for `sniper.shot_s`, it reloads for
    /// `sniper.reload_s`, and the shot gets its stray and its beam's speckle.
    ///
    /// Example: `weapons.fire_sniper(&settings.sniper);`
    pub fn fire_sniper(&mut self, sniper: &SniperSettings) {
        self.shot_time_s = sniper.shot_s;
        self.sniper_reload_s = sniper.reload_s;
        self.shot_offset = self.shot_noise.sample_sniper(sniper);
        self.beam_speckle_frac = self.shot_noise.sample_sniper_speckle(sniper);
    }

    /// `tick` counts `dt_s` off the shot under way and the sniper's reload.
    ///
    /// Example: `weapons.tick(1.0 / 60.0);`
    pub fn tick(&mut self, dt_s: f64) {
        self.shot_time_s = (self.shot_time_s - dt_s).max(0.0);
        self.sniper_reload_s = (self.sniper_reload_s - dt_s).max(0.0);
    }

    /// `update_aim` moves the aim toward full (`is_aiming`) or none, over `aim.zoom_s`.
    ///
    /// Example: `weapons.update_aim(true, 1.0 / 60.0, &settings.aim);`
    pub fn update_aim(&mut self, is_aiming: bool, dt_s: f64, aim: &AimSettings) {
        let aim_step: f64 = dt_s / aim.zoom_s;
        self.aim_frac =
            (self.aim_frac + if is_aiming { aim_step } else { -aim_step }).clamp(0.0, 1.0);
    }

    /// `get_zoom` gives how much closer things look: the gun in hand's full zoom, eased in by the
    /// aim (`get_aim_zoom`).
    ///
    /// Example: `let zoom: f64 = weapons.get_zoom(&settings.aim);`
    pub fn get_zoom(&self, aim: &AimSettings) -> f64 {
        let full_zoom: f64 = match self.gun {
            Gun::Laser => aim.sniper_zoom,
            Gun::Rifle => aim.rifle_zoom,
        };
        get_aim_zoom(self.aim_frac, full_zoom)
    }

    /// `update_rifle` fires the rifle while its trigger is held, this player `can_shoot` and it
    /// isn't overheated: a `rifle.pulse_s` shot every 1 / `rifle.shots_per_s`, each adding
    /// 1 / `rifle.shots_until_overheat` of heat, straying more as it heats. It cools fully in
    /// `rifle.cool_down_s` while not firing, and its jolt settles over `rifle.recoil_settle_s`. It
    /// gives the view's kick when a shot fires (radians right, then up, sideways alternately left
    /// and right), for the level to turn the view by, else `None`.
    ///
    /// Example: `let kick: Option<[f64; 2]> = weapons.update_rifle(&settings.rifle, true, 1.0 / 60.0);`
    pub fn update_rifle(
        &mut self,
        rifle: &RifleSettings,
        can_shoot: bool,
        dt_s: f64,
    ) -> Option<[f64; 2]> {
        self.rifle_next_shot_s = (self.rifle_next_shot_s - dt_s).max(0.0);
        self.recoil_frac = if rifle.recoil_settle_s > 0.0 {
            (self.recoil_frac - dt_s / rifle.recoil_settle_s).max(0.0)
        } else {
            0.0
        };
        let is_firing_rifle: bool =
            self.gun == Gun::Rifle && self.is_trigger_held && can_shoot && !self.is_overheated;
        if !is_firing_rifle {
            self.rifle_heat_frac = (self.rifle_heat_frac - dt_s / rifle.cool_down_s).max(0.0);
            self.is_overheated &= self.rifle_heat_frac > 0.0;
            return None;
        }
        if self.rifle_next_shot_s > 0.0 {
            return None;
        }
        self.shot_time_s = rifle.pulse_s;
        self.rifle_next_shot_s = 1.0 / rifle.shots_per_s;
        self.has_rifle_shot_hit = false;
        self.shot_offset = self.shot_noise.sample_rifle(rifle, self.rifle_heat_frac);
        self.beam_speckle_frac = shot_noise::get_rifle_speckle_frac(rifle, self.rifle_heat_frac);
        self.rifle_heat_frac += 1.0 / f64::from(rifle.shots_until_overheat);
        self.is_overheated = self.rifle_heat_frac >= 1.0;
        self.rifle_shot_count += 1;
        self.recoil_frac = f64::from(u8::from(rifle.recoil_settle_s > 0.0));
        let side: f64 = if self.rifle_shot_count.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        Some([side * rifle.recoil_sideways_rad, -rifle.recoil_up_rad])
    }
}

/// `get_aim_zoom` gives how much closer the world looks at aim fraction `aim_frac` (0 not aiming,
/// 1 fully aimed), eased in and out, toward `full_zoom`.
///
/// Example: `let zoom = get_aim_zoom(0.5, 2.5);`
pub fn get_aim_zoom(aim_frac: f64, full_zoom: f64) -> f64 {
    let t: f64 = aim_frac.clamp(0.0, 1.0);
    1.0 + (full_zoom - 1.0) * t * t * (3.0 - 2.0 * t)
}

/// `take_hit_weight` gives how many hits like this one eliminate a player, or `None` when it
/// doesn't count: the sniper's (`sniper.hits_to_eliminate`) on every frame it hits, the rifle's
/// only once a shot (`has_rifle_shot_hit`, which it sets), by where it hit (the head or the body).
///
/// Example: `let hits_to_kill = take_hit_weight(&settings.sniper, &settings.rifle, Gun::Rifle, &mut has_hit, true);`
pub fn take_hit_weight(
    sniper: &SniperSettings,
    rifle: &RifleSettings,
    gun: Gun,
    has_rifle_shot_hit: &mut bool,
    is_head_hit: bool,
) -> Option<u8> {
    match (gun, is_head_hit) {
        (Gun::Laser, _) => Some(sniper.hits_to_eliminate),
        (Gun::Rifle, _) if *has_rifle_shot_hit => None,
        (Gun::Rifle, is_head) => {
            *has_rifle_shot_hit = true;
            Some(if is_head {
                rifle.head_hits_to_eliminate
            } else {
                rifle.body_hits_to_eliminate
            })
        }
    }
}

/// `make_wand` places the wand in camera space: held on the right, aimed toward the crosshair with
/// the front of its tip at the muzzle (`MUZZLE_M`, where the laser starts), dipping a little at each step (`phase_rad`), and, as `run_pose_frac` goes to 1
/// (eased), lowered, tipped down and rolled, bouncing and swinging with each stride; its tip has
/// `team`'s colour. The laser (`gun`) is a thin dark wand with a short glowing tip; the rifle is a
/// thick grey block with a long, thin glowing barrel. Recoil (`recoil_frac`, 1 just after a shot)
/// pushes it back toward the eye and tips it up, and `sway` (`get_sway`: metres right, radians of
/// muzzle turned left) slides and swings it with the steps.
///
/// Example: `let wand = make_wand(0.0, 1.2, 0.0, 1, Gun::Laser, (0.0, 0.0));`
pub fn make_wand(
    run_pose_frac: f64,
    phase_rad: f64,
    recoil_frac: f64,
    team: u32,
    gun: Gun,
    sway: (f32, f32),
) -> OverlayWand {
    let (sway_m, swing_rad) = sway;
    let jog: f32 = ease_frac(run_pose_frac) as f32;
    let (step, swing) = (phase_rad.sin().abs() as f32, phase_rad.sin() as f32);
    let lerp = |aim: f32, run: f32| aim + (run - aim) * jog;
    let bob_m: f32 = lerp(WAND_WALK_BOB_M, WAND_JOG_BOB_M) * step;
    let (shaft_half_m, tip_half_m) = match gun {
        Gun::Laser => (WAND_LASER_SHAFT_HALF_M, WAND_LASER_TIP_HALF_M),
        Gun::Rifle => (WAND_RIFLE_SHAFT_HALF_M, WAND_RIFLE_TIP_HALF_M),
    };
    let aim_rotation: Mat3 =
        Mat3::from_euler(EulerRot::YXZ, WAND_AIM_YAW_RAD, WAND_AIM_PITCH_RAD, 0.0);
    let aim_center: Vec3 =
        MUZZLE_M.as_vec3() + aim_rotation * (Vec3::Z * (shaft_half_m[2] + 2.0 * tip_half_m[2]));
    let kick: f32 = recoil_frac as f32;
    let center: Vec3 = aim_center.lerp(Vec3::from(WAND_JOG_CENTER_M), jog) - Vec3::Y * bob_m
        + Vec3::Z * WAND_RECOIL_M * kick
        + Vec3::X * sway_m;
    let rotation: Mat3 = Mat3::from_euler(
        EulerRot::YXZ,
        lerp(WAND_AIM_YAW_RAD, WAND_JOG_YAW_RAD) + swing_rad,
        lerp(WAND_AIM_PITCH_RAD, WAND_JOG_PITCH_RAD) + WAND_RECOIL_TILT_RAD * kick,
        (WAND_JOG_ROLL_RAD + WAND_JOG_SWING_RAD * swing) * jog,
    );
    OverlayWand {
        center_m: center.extend(0.0).to_array(),
        axes: [rotation.x_axis, rotation.y_axis, rotation.z_axis]
            .map(|axis| axis.extend(0.0).to_array()),
        shaft_half_m,
        shaft_color: match gun {
            Gun::Laser => WAND_LASER_SHAFT_COLOR,
            Gun::Rifle => WAND_RIFLE_SHAFT_COLOR,
        },
        tip_half_m,
        tip_color: if team == 1 {
            WAND_RED_TIP_COLOR
        } else {
            WAND_BLUE_TIP_COLOR
        },
    }
}

/// `make_wand_camera_from_muzzle` gives the frame of `wand`'s muzzle, the front of its tip, in
/// camera space (its barrel along -z), for a level that draws a gun model there instead of the
/// wand.
///
/// Example: `let camera_from_muzzle = make_wand_camera_from_muzzle(&make_wand(0.0, 1.2, 0.0, 1, Gun::Laser, (0.0, 0.0)));`
pub fn make_wand_camera_from_muzzle(wand: &OverlayWand) -> Mat4 {
    let rotation: Mat3 = Mat3::from_cols(
        Vec3::from_slice(&wand.axes[0][..3]),
        Vec3::from_slice(&wand.axes[1][..3]),
        Vec3::from_slice(&wand.axes[2][..3]),
    );
    let reach_m: f32 = wand.shaft_half_m[2] + 2.0 * wand.tip_half_m[2];
    let center: Vec3 = Vec3::from_slice(&wand.center_m[..3]);
    Mat4::from_mat3_translation(rotation, center - rotation.z_axis * reach_m)
}

/// `ease_frac` eases a fraction from 0 to 1 (smoothstep): slow at both ends.
///
/// Example: `let jog: f64 = ease_frac(0.5);`
pub fn ease_frac(frac: f64) -> f64 {
    frac * frac * (3.0 - 2.0 * frac)
}

/// `get_sway` gives how the gun in hand sways now with the steps (`phase_rad`, the step phase: a
/// step each half turn), as (metres slid right, radians its muzzle turns left): the hands slide it
/// left on one step and right on the next by `head_bob`'s walking slide, and a longer, heavier gun
/// lags behind them, its muzzle swinging the other way by `head_bob`'s swing for a gun as long as
/// the bolt-action rifle, times `model`'s length over the rifle's. Both grow to their sprinting
/// sizes as the run pose (`run_pose_frac`, eased as `make_wand` eases it) goes to 1, and fade in
/// and out with `step_frac` (0 standing still, 1 stepping).
///
/// Example: `let sway = get_sway(GunModel::Revolver, 1.0, 0.0, 0.5, &settings.view.head_bob);`
pub fn get_sway(
    model: GunModel,
    step_frac: f64,
    run_pose_frac: f64,
    phase_rad: f64,
    head_bob: &HeadBobSettings,
) -> (f32, f32) {
    let jog: f64 = ease_frac(run_pose_frac);
    let lerp = |walk: f64, sprint: f64| walk + (sprint - walk) * jog;
    let length_frac: f64 =
        f64::from(model.get_modelled_length_m() / BOLT_ACTION_RIFLE_MODELLED_LENGTH_M);
    let side: f64 = step_frac * phase_rad.sin();
    let sway_m: f64 = lerp(head_bob.gun_walk_sway_m, head_bob.gun_sprint_sway_m) * side;
    let swing_rad: f64 = lerp(
        head_bob.rifle_walk_swing_rad,
        head_bob.rifle_sprint_swing_rad,
    ) * length_frac
        * side;
    (sway_m as f32, swing_rad as f32)
}

#[cfg(test)]
mod tests {
    //! Tests of the guns' state, aiming, hit weights and the gun in hand's sway.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Gun | laser (every frame's hit counts, one eliminates), rifle (one hit per shot) | `hits_weigh_by_gun_and_place` |
    //! | Where a rifle hits | the head, the body | `hits_weigh_by_gun_and_place` |
    //! | Aim fraction | not aiming, half way (eased, half the extra zoom), fully aimed, past either end (clamped) | `aiming_zooms_in_eased`, `aiming_eases_the_gun_zoom` |
    //! | Gun model swaying | the bolt-action rifle (full swing), the revolver (its swing scaled by its length) | `guns_sway_with_the_steps_by_their_length` |
    //! | Stepping fraction | standing (0: no sway), easing in (0.5: half), stepping (1) | `guns_sway_with_the_steps_by_their_length` |
    //! | Run pose | walking (0: the walking sway), sprinting (1: the sprinting sway), half way (eased between) | `guns_sway_with_the_steps_by_their_length` |
    //! | Step phase | between steps (0: centred), one step (a quarter turn: right), the next (three quarters: left) | `guns_sway_with_the_steps_by_their_length` |
    //! | Rifle trigger | held (fires at its rate, recoil alternating sides, overheats after its shots, cools), released (no shot) | `rifle_fires_overheats_and_cools` |
    //! | May shoot | yes, no (no shot) | `rifle_fires_overheats_and_cools`, `sniper_shoots_reloads_and_switches` |
    //! | Sniper | fired (its shot and reload), ticking down, switching guns (ends the shot) | `sniper_shoots_reloads_and_switches` |
    //! | The wand's muzzle frame, at rest | each gun: at `MUZZLE_M`, its barrel toward the crosshair | `the_muzzle_frame_is_at_the_muzzle` |

    use super::*;
    use serde::Deserialize;

    const LUNAR_SERVER_YAML: &str =
        include_str!("../../hyperbolic_wormholes_rust/server_settings.yaml");
    const LUNAR_PLAYER_YAML: &str =
        include_str!("../../hyperbolic_wormholes_rust/player_settings.yaml");
    const STEP_S: f64 = 1.0 / 240.0;
    const SEED: u64 = 7;

    /// `ServerSettings` is the guns' and aiming's sections of Lunar Surface's
    /// `server_settings.yaml`, which every level's copy shares.
    #[derive(Deserialize)]
    struct ServerSettings {
        sniper: SniperSettings,
        rifle: RifleSettings,
        aim: AimSettings,
    }

    /// `PlayerView` is the head bob of Lunar Surface's `player_settings.yaml`.
    #[derive(Deserialize)]
    struct PlayerView {
        head_bob: HeadBobSettings,
    }

    /// `PlayerFile` is the view of Lunar Surface's `player_settings.yaml`.
    #[derive(Deserialize)]
    struct PlayerFile {
        view: PlayerView,
    }

    /// `read_settings` reads Lunar Surface's built-in server settings' guns and aiming.
    fn read_settings() -> ServerSettings {
        serde_yaml::from_str(LUNAR_SERVER_YAML).expect("Lunar Surface's server settings")
    }

    #[test]
    fn hits_weigh_by_gun_and_place() {
        let settings: ServerSettings = read_settings();
        let (sniper, rifle) = (&settings.sniper, &settings.rifle);
        let mut has_hit: bool = false;
        for _ in 0..2 {
            assert_eq!(
                take_hit_weight(sniper, rifle, Gun::Laser, &mut has_hit, false),
                Some(sniper.hits_to_eliminate)
            );
        }
        assert_eq!(
            take_hit_weight(sniper, rifle, Gun::Rifle, &mut has_hit, true),
            Some(rifle.head_hits_to_eliminate)
        );
        assert_eq!(
            take_hit_weight(sniper, rifle, Gun::Rifle, &mut has_hit, false),
            None
        );
        has_hit = false;
        assert_eq!(
            take_hit_weight(sniper, rifle, Gun::Rifle, &mut has_hit, false),
            Some(rifle.body_hits_to_eliminate)
        );
    }

    #[test]
    fn aiming_zooms_in_eased() {
        const FULL_ZOOM: f64 = 2.5;
        assert_eq!(get_aim_zoom(0.0, FULL_ZOOM), 1.0);
        assert_eq!(get_aim_zoom(0.5, FULL_ZOOM), 1.0 + (FULL_ZOOM - 1.0) / 2.0);
        assert_eq!(get_aim_zoom(1.0, FULL_ZOOM), FULL_ZOOM);
        assert_eq!(get_aim_zoom(-1.0, FULL_ZOOM), 1.0);
        assert_eq!(get_aim_zoom(2.0, FULL_ZOOM), FULL_ZOOM);
        assert!(
            get_aim_zoom(0.1, FULL_ZOOM) - 1.0 < 0.1 * (FULL_ZOOM - 1.0),
            "eased in"
        );
    }

    #[test]
    fn aiming_eases_the_gun_zoom() {
        let aim: AimSettings = read_settings().aim;
        let mut weapons: Weapons = Weapons::New(ShotNoise::FromSeed(SEED));
        weapons.update_aim(true, aim.zoom_s / 2.0, &aim);
        assert_eq!(weapons.aim_frac, 0.5);
        weapons.update_aim(true, aim.zoom_s, &aim);
        assert_eq!(weapons.get_zoom(&aim), aim.rifle_zoom);
        weapons.switch_gun(Gun::Laser);
        assert_eq!(weapons.get_zoom(&aim), aim.sniper_zoom);
        weapons.update_aim(false, 2.0 * aim.zoom_s, &aim);
        assert_eq!(weapons.get_zoom(&aim), 1.0);
    }

    #[test]
    fn rifle_fires_overheats_and_cools() {
        let rifle: RifleSettings = read_settings().rifle;
        let mut weapons: Weapons = Weapons::New(ShotNoise::FromSeed(SEED));
        weapons.is_trigger_held = true;
        assert_eq!(
            weapons.update_rifle(&rifle, false, STEP_S),
            None,
            "may not shoot"
        );
        let mut kicks: Vec<[f64; 2]> = Vec::new();
        let mut elapsed_s: f64 = 0.0;
        while !weapons.is_overheated {
            kicks.extend(weapons.update_rifle(&rifle, true, STEP_S));
            elapsed_s += STEP_S;
        }
        assert_eq!(kicks.len(), rifle.shots_until_overheat as usize);
        let expected_s: f64 = f64::from(rifle.shots_until_overheat - 1) / rifle.shots_per_s;
        assert!(
            (elapsed_s - expected_s).abs() <= f64::from(rifle.shots_until_overheat + 1) * STEP_S,
            "{elapsed_s} s"
        );
        assert!(kicks.iter().all(|kick| kick[1] == -rifle.recoil_up_rad));
        assert!(
            kicks.windows(2).all(|pair| pair[0][0] == -pair[1][0]),
            "sides alternate"
        );
        assert!(weapons.shot_time_s > 0.0 && weapons.recoil_frac == 1.0);
        assert_eq!(
            weapons.update_rifle(&rifle, true, rifle.cool_down_s / 2.0),
            None
        );
        assert!(
            weapons.is_overheated,
            "still too hot half way through cooling"
        );
        weapons.update_rifle(&rifle, true, rifle.cool_down_s);
        assert!(!weapons.is_overheated && weapons.rifle_heat_frac == 0.0);
        assert_eq!(weapons.recoil_frac, 0.0);
        weapons.is_trigger_held = false;
        assert_eq!(
            weapons.update_rifle(&rifle, true, STEP_S),
            None,
            "trigger released"
        );
    }

    #[test]
    fn sniper_shoots_reloads_and_switches() {
        let sniper: SniperSettings = read_settings().sniper;
        let mut weapons: Weapons = Weapons::New(ShotNoise::FromSeed(SEED));
        weapons.switch_gun(Gun::Laser);
        assert!(!weapons.is_firing(true));
        weapons.fire_sniper(&sniper);
        assert!(weapons.is_firing(true) && !weapons.is_firing(false));
        assert_eq!(weapons.sniper_reload_s, sniper.reload_s);
        weapons.tick(sniper.shot_s);
        assert!(!weapons.is_firing(true), "the shot ended");
        assert_eq!(weapons.sniper_reload_s, sniper.reload_s - sniper.shot_s);
        weapons.tick(sniper.reload_s);
        assert_eq!(weapons.sniper_reload_s, 0.0);
        weapons.fire_sniper(&sniper);
        weapons.switch_gun(Gun::Rifle);
        assert!(!weapons.is_firing(true), "switching guns ends the shot");
    }

    #[test]
    fn guns_sway_with_the_steps_by_their_length() {
        const TOLERANCE: f64 = 1e-6;
        const HALF_FRAC: f64 = 0.5;
        let file: PlayerFile =
            serde_yaml::from_str(LUNAR_PLAYER_YAML).expect("Lunar Surface's player settings");
        let head_bob: HeadBobSettings = file.view.head_bob;
        let right_rad: f64 = std::f64::consts::FRAC_PI_2;
        let left_rad: f64 = 3.0 * std::f64::consts::FRAC_PI_2;
        let sway = |model: GunModel, step_frac: f64, run_pose_frac: f64, phase_rad: f64| {
            let (sway_m, swing_rad) =
                get_sway(model, step_frac, run_pose_frac, phase_rad, &head_bob);
            (f64::from(sway_m), f64::from(swing_rad))
        };
        let is_near = |a: (f64, f64), b: (f64, f64)| {
            (a.0 - b.0).abs() < TOLERANCE && (a.1 - b.1).abs() < TOLERANCE
        };
        let rifle = GunModel::BoltActionRifle;
        let revolver = GunModel::Revolver;
        let (walk_m, sprint_m) = (head_bob.gun_walk_sway_m, head_bob.gun_sprint_sway_m);
        let (walk_rad, sprint_rad) = (
            head_bob.rifle_walk_swing_rad,
            head_bob.rifle_sprint_swing_rad,
        );
        assert!(walk_m > 0.0 && sprint_m > walk_m && walk_rad > 0.0 && sprint_rad > walk_rad);
        assert!(is_near(sway(rifle, 0.0, 1.0, right_rad), (0.0, 0.0)));
        assert!(is_near(sway(rifle, 1.0, 0.0, 0.0), (0.0, 0.0)));
        assert!(is_near(
            sway(rifle, 1.0, 0.0, right_rad),
            (walk_m, walk_rad)
        ));
        assert!(is_near(
            sway(rifle, 1.0, 0.0, left_rad),
            (-walk_m, -walk_rad)
        ));
        assert!(is_near(
            sway(rifle, 1.0, 1.0, right_rad),
            (sprint_m, sprint_rad)
        ));
        assert!(is_near(
            sway(rifle, HALF_FRAC, 0.0, right_rad),
            (HALF_FRAC * walk_m, HALF_FRAC * walk_rad)
        ));
        let half_way: (f64, f64) = sway(rifle, 1.0, HALF_FRAC, right_rad);
        assert!(is_near(
            half_way,
            (
                (walk_m + sprint_m) * HALF_FRAC,
                (walk_rad + sprint_rad) * HALF_FRAC
            )
        ));
        let length_frac: f64 =
            f64::from(revolver.get_modelled_length_m() / rifle.get_modelled_length_m());
        assert!(length_frac < 1.0);
        assert!(is_near(
            sway(revolver, 1.0, 1.0, right_rad),
            (sprint_m, sprint_rad * length_frac)
        ));
    }

    #[test]
    fn the_muzzle_frame_is_at_the_muzzle() {
        for gun in [Gun::Laser, Gun::Rifle] {
            let frame: Mat4 =
                make_wand_camera_from_muzzle(&make_wand(0.0, 0.0, 0.0, 0, gun, (0.0, 0.0)));
            assert!(frame.w_axis.truncate().distance(MUZZLE_M.as_vec3()) < 1e-5);
            assert!(frame.z_axis.truncate().dot(Vec3::Z) > 0.99);
        }
    }
}
