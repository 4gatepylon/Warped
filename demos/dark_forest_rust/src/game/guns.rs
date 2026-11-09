//! `guns` holds the Lunar Surface demo's two guns (`../hyperbolic_wormholes_rust/src/main.rs`),
//! ported to this level: gun 1, the sniper (a laser on for `sniper.shot_s` per click, then a
//! reload), and gun 2, the rifle (fires while held, overheats, recoils); aiming's zoom; where the
//! gun in hand is (`Guns::get_camera_from_muzzle`: its pose toward jogging, its dip and sway with
//! each step, and its jolt after a shot); and each shot's beam flying out (`Tracer`). Their
//! numbers are the server settings' (`settings.rs`) and the head bob's (`settings::HEAD_BOB`).
//!
//! Interface: `Gun`, `GunModel`, `GunInput`, `GunStep`, `Guns`, `Tracer`, `take_hit_weight`,
//! `get_aim_zoom`, `MUZZLE_M`, `HALF_FOV_RAD`, `LASER_MAX_LENGTH_M`, `LASER_MUZZLE_M`.
//!
//! Example:
//!     let kick_rad: Vec2 = guns.update(&input, &settings, true, &step, dt_s);
//!     let camera_from_muzzle: Mat4 = guns.get_camera_from_muzzle();

use glam::{EulerRot, Mat3, Mat4, Vec2, Vec3};

use crate::settings::{HEAD_BOB, HeadBobSettings, RifleSettings, ServerSettings};
use crate::shot_noise::{self, ShotNoise, ShotOffset};

/// `MUZZLE_M` is where the gun in hand's muzzle is in camera space while aiming (x right, y up,
/// -z ahead), where the laser starts: Lunar Surface's `voxel_world::MUZZLE_M`.
pub const MUZZLE_M: Vec3 = Vec3::new(0.187, -0.157, -0.555);
/// `HALF_FOV_RAD` is half the view's vertical angle before aiming zooms it: Lunar Surface's.
pub const HALF_FOV_RAD: f32 = 37.5 * std::f32::consts::PI / 180.0;
/// `LASER_MAX_LENGTH_M` is the farthest a shot reaches, and `LASER_MUZZLE_M` how far along its
/// path the drawn beam joins it from the muzzle: Lunar Surface's.
pub const LASER_MAX_LENGTH_M: f32 = 480.0;
pub const LASER_MUZZLE_M: f32 = 3.0;
/// `SKY_TRACER_M` is how far the beam of a shot that hit nothing flies: Lunar Surface's.
const SKY_TRACER_M: f32 = 80.0;
/// `REVOLVER_MODELLED_LENGTH_M` and `BOLT_ACTION_RIFLE_MODELLED_LENGTH_M` are the gun models'
/// lengths in their glTFs (Lunar Surface's `viewmodel.rs`).
const REVOLVER_MODELLED_LENGTH_M: f32 = 0.302;
const BOLT_ACTION_RIFLE_MODELLED_LENGTH_M: f32 = 1.231;
/// The `WAND_*` constants place the gun in hand as Lunar Surface's `make_wand` does: its recoil,
/// its reach from the muzzle back to the centre of the wand it replaced (the shaft's half length
/// plus the tip's length, per gun), its jogging pose, its aiming turn, and its bob per step.
const WAND_RECOIL_M: f32 = 0.035;
const WAND_RECOIL_TILT_RAD: f32 = 0.12;
const WAND_LASER_REACH_M: f32 = 0.16 + 2.0 * 0.028;
const WAND_RIFLE_REACH_M: f32 = 0.17 + 2.0 * 0.07;
const WAND_JOG_CENTER_M: [f32; 3] = [0.22, -0.22, -0.34];
const WAND_AIM_YAW_RAD: f32 = 0.076;
const WAND_JOG_YAW_RAD: f32 = 0.35;
const WAND_AIM_PITCH_RAD: f32 = 0.064;
const WAND_JOG_PITCH_RAD: f32 = -0.35;
const WAND_JOG_ROLL_RAD: f32 = 0.5;
const WAND_JOG_SWING_RAD: f32 = 0.18;
const WAND_WALK_BOB_M: f32 = 0.006;
const WAND_JOG_BOB_M: f32 = 0.03;
/// `GUN_POSE_PER_S` is how fast the gun moves between its aiming and jogging poses.
const GUN_POSE_PER_S: f64 = 6.0;

/// `Gun` is the weapon a player holds: gun 1, the laser (the sniper), or gun 2, the rifle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gun {
    Laser,
    Rifle,
}

impl Gun {
    /// `get_model` gives the model drawn in hand for this gun: the bolt-action rifle for the laser
    /// (the sniper) and the energy revolver for the rifle.
    ///
    /// Example: `let model: GunModel = Gun::Laser.get_model();`
    pub fn get_model(self) -> GunModel {
        match self {
            Gun::Laser => GunModel::BoltActionRifle,
            Gun::Rifle => GunModel::Revolver,
        }
    }
}

/// `GunModel` is a model drawn in the player's hand: the bolt-action rifle (the sniper, gun 1) or
/// the energy revolver (the rifle, gun 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GunModel {
    Revolver,
    BoltActionRifle,
}

impl GunModel {
    /// `get_modelled_length_m` gives how long the model is in its glTF, its real size.
    ///
    /// Example: `let length_m: f32 = GunModel::Revolver.get_modelled_length_m();`
    pub fn get_modelled_length_m(self) -> f32 {
        match self {
            GunModel::Revolver => REVOLVER_MODELLED_LENGTH_M,
            GunModel::BoltActionRifle => BOLT_ACTION_RIFLE_MODELLED_LENGTH_M,
        }
    }
}

/// `GunInput` is one frame's gun controls: the trigger held, a click this frame, the aim control
/// held, a gun picked (`1`, `2`) and a swap (the wheel).
#[derive(Clone, Copy, Debug, Default)]
pub struct GunInput {
    pub is_trigger_held: bool,
    pub is_shoot_pressed: bool,
    pub is_aim_held: bool,
    pub picked_gun: Option<Gun>,
    pub is_swap_pressed: bool,
}

/// `GunStep` is how the player's steps move the gun this frame: whether they sprint, and their
/// head bob's phase and how far into stepping they are (`Player::bob_phase_rad`, `bob_frac`).
#[derive(Clone, Copy, Debug, Default)]
pub struct GunStep {
    pub is_sprinting: bool,
    pub bob_phase_rad: f64,
    pub bob_frac: f64,
}

/// `Guns` is one player's guns, as Lunar Surface's session keeps them: the gun in hand, the shot
/// in flight (`shot_time_s`, while the laser is on) and the sniper's reload, the rifle's next shot,
/// heat and shot count, the recoil's jolt, aiming, the shot's stray and speckle, and the gun's
/// step phase, how far into stepping it is and its pose toward jogging.
pub struct Guns {
    pub gun: Gun,
    pub shot_time_s: f64,
    pub sniper_reload_s: f64,
    pub rifle_heat_frac: f64,
    pub is_overheated: bool,
    pub aim_frac: f64,
    pub has_rifle_shot_hit: bool,
    pub shot_offset: ShotOffset,
    pub beam_speckle_frac: f64,
    rifle_next_shot_s: f64,
    rifle_shot_count: u64,
    recoil_frac: f64,
    shot_noise: ShotNoise,
    gun_phase_rad: f64,
    step_frac: f64,
    run_pose_frac: f64,
}

impl Guns {
    /// `New` gives a player's guns with the sniper in hand, loaded and cold, its shots strayed by
    /// `shot_noise`.
    ///
    /// Example: `let guns: Guns = Guns::New(ShotNoise::FromClock());`
    #[allow(non_snake_case)]
    pub fn New(shot_noise: ShotNoise) -> Guns {
        Guns {
            gun: Gun::Laser,
            shot_time_s: 0.0,
            sniper_reload_s: 0.0,
            rifle_heat_frac: 0.0,
            is_overheated: false,
            aim_frac: 0.0,
            has_rifle_shot_hit: false,
            shot_offset: ShotOffset::NONE,
            beam_speckle_frac: 0.0,
            rifle_next_shot_s: 0.0,
            rifle_shot_count: 0,
            recoil_frac: 0.0,
            shot_noise,
            gun_phase_rad: 0.0,
            step_frac: 0.0,
            run_pose_frac: 0.0,
        }
    }

    /// `switch_gun` makes `gun` the one in hand, stopping a shot in flight.
    ///
    /// Example: `guns.switch_gun(Gun::Rifle);`
    pub fn switch_gun(&mut self, gun: Gun) {
        self.gun = gun;
        self.shot_time_s = 0.0;
    }

    /// `update` takes this frame's controls (`input`) and moves the guns on by `dt_s` with
    /// `settings`, as Lunar Surface does: the gun picked or swapped, a sniper shot on a click when
    /// it is loaded, the shot and reload timers, aiming, the gun in hand (`update_gun`, with the
    /// player's `step`) and the rifle (`update_rifle`). `can_shoot` says whether the player may
    /// fire now. It gives how far the rifle's recoil turns the view this frame, in radians (x
    /// right, y up).
    ///
    /// Example: `let kick_rad: Vec2 = guns.update(&input, &settings, true, &step, 1.0 / 60.0);`
    pub fn update(
        &mut self,
        input: &GunInput,
        settings: &ServerSettings,
        can_shoot: bool,
        step: &GunStep,
        dt_s: f64,
    ) -> Vec2 {
        if let Some(gun) = input.picked_gun {
            self.switch_gun(gun);
        }
        if input.is_swap_pressed {
            self.switch_gun(match self.gun {
                Gun::Laser => Gun::Rifle,
                Gun::Rifle => Gun::Laser,
            });
        }
        if input.is_shoot_pressed
            && self.gun == Gun::Laser
            && self.sniper_reload_s <= 0.0
            && can_shoot
        {
            self.shot_time_s = settings.sniper.shot_s;
            self.sniper_reload_s = settings.sniper.reload_s;
            self.shot_offset = self.shot_noise.sample_sniper(&settings.sniper);
            self.beam_speckle_frac = self.shot_noise.sample_sniper_speckle(&settings.sniper);
        }
        self.shot_time_s = (self.shot_time_s - dt_s).max(0.0);
        self.sniper_reload_s = (self.sniper_reload_s - dt_s).max(0.0);
        let aim_step: f64 = dt_s / settings.aim.zoom_s;
        self.aim_frac = (self.aim_frac
            + if input.is_aim_held {
                aim_step
            } else {
                -aim_step
            })
        .clamp(0.0, 1.0);
        if !can_shoot {
            self.shot_time_s = 0.0;
        }
        self.update_gun(step, dt_s);
        self.update_rifle(&settings.rifle, input.is_trigger_held && can_shoot, dt_s)
    }

    /// `update_gun` moves the gun in hand with the player's `step`: its step phase and how far into
    /// stepping it is (the head bob's, so the gun steps with the eye), and its pose toward jogging
    /// while sprinting and back to aiming otherwise.
    ///
    /// Example: `self.update_gun(&step, dt_s);`
    fn update_gun(&mut self, step: &GunStep, dt_s: f64) {
        (self.gun_phase_rad, self.step_frac) = (step.bob_phase_rad, step.bob_frac);
        let target: f64 = f64::from(u8::from(step.is_sprinting));
        self.run_pose_frac +=
            (target - self.run_pose_frac).clamp(-GUN_POSE_PER_S * dt_s, GUN_POSE_PER_S * dt_s);
    }

    /// `update_rifle` fires the rifle while its trigger is held and this player can shoot
    /// (`is_firing_rifle`, unless it is not in hand or overheated): a `pulse_s` pulse of the beam
    /// every 1 / `shots_per_s`, each adding 1 / `shots_until_overheat` of heat. At full heat it
    /// overheats; it cools fully in `cool_down_s` while not firing, and fires again once cool. Each
    /// shot strays by noise that grows with the heat, and recoils: it gives the view's kick up by
    /// `recoil_up_rad` and sideways by `recoil_sideways_rad`, alternately left and right, and jolts
    /// the gun back (settling over `recoil_settle_s`).
    ///
    /// Example: `let kick_rad: Vec2 = self.update_rifle(&settings.rifle, true, 1.0 / 60.0);`
    fn update_rifle(&mut self, rifle: &RifleSettings, is_trigger_held: bool, dt_s: f64) -> Vec2 {
        self.rifle_next_shot_s = (self.rifle_next_shot_s - dt_s).max(0.0);
        self.recoil_frac = if rifle.recoil_settle_s > 0.0 {
            (self.recoil_frac - dt_s / rifle.recoil_settle_s).max(0.0)
        } else {
            0.0
        };
        let is_firing_rifle: bool =
            self.gun == Gun::Rifle && is_trigger_held && !self.is_overheated;
        if !is_firing_rifle {
            self.rifle_heat_frac = (self.rifle_heat_frac - dt_s / rifle.cool_down_s).max(0.0);
            self.is_overheated &= self.rifle_heat_frac > 0.0;
            return Vec2::ZERO;
        }
        if self.rifle_next_shot_s > 0.0 {
            return Vec2::ZERO;
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
        Vec2::new(
            (side * rifle.recoil_sideways_rad) as f32,
            rifle.recoil_up_rad as f32,
        )
    }

    /// `is_firing` says whether the laser is on now: a sniper shot in its `shot_s`, or a rifle
    /// shot in its `pulse_s`.
    ///
    /// Example: `if guns.is_firing() { ... }`
    pub fn is_firing(&self) -> bool {
        self.shot_time_s > 0.0
    }

    /// `get_zoom` gives how much closer the world looks now: the gun in hand's full zoom (`aim`
    /// in `settings`), eased in by how far this player has aimed (`get_aim_zoom`).
    ///
    /// Example: `let zoom: f64 = guns.get_zoom(&settings);`
    pub fn get_zoom(&self, settings: &ServerSettings) -> f64 {
        let full_zoom: f64 = match self.gun {
            Gun::Laser => settings.aim.sniper_zoom,
            Gun::Rifle => settings.aim.rifle_zoom,
        };
        get_aim_zoom(self.aim_frac, full_zoom)
    }

    /// `get_camera_from_muzzle` places the gun in hand in camera space (`make_wand`, swayed by
    /// `get_sway`): its muzzle at the origin of the result, its barrel along -z.
    ///
    /// Example: `let camera_from_model = guns.get_camera_from_muzzle() * muzzle_from_gltf;`
    pub fn get_camera_from_muzzle(&self) -> Mat4 {
        let sway: (f32, f32) = get_sway(
            self.gun.get_model(),
            self.step_frac,
            self.run_pose_frac,
            self.gun_phase_rad,
            &HEAD_BOB,
        );
        make_wand(
            self.run_pose_frac,
            self.gun_phase_rad,
            self.recoil_frac,
            self.gun,
            sway,
        )
    }

    /// `get_muzzle_m` gives where the tip of the gun in hand shows from the eye, in camera space:
    /// moved toward the view's centre by the aim `zoom`, which magnifies the scene but not the gun.
    ///
    /// Example: `let muzzle_m: Vec3 = camera_world.transform_point3(guns.get_muzzle_m(zoom));`
    pub fn get_muzzle_m(&self, zoom: f64) -> Vec3 {
        let tip_m: Vec3 = self.get_camera_from_muzzle().w_axis.truncate();
        let zoom: f32 = zoom as f32;
        Vec3::new(tip_m.x / zoom, tip_m.y / zoom, tip_m.z)
    }
}

/// `take_hit_weight` gives how many hits like this one `gun` needs to eliminate someone by
/// `settings` (a rifle hit to the head counts more), or `None` when this rifle shot already hit
/// (`has_rifle_shot_hit`, which it then sets): the laser reports every frame it hits, the rifle
/// once per shot.
///
/// Example: `if let Some(hits_to_kill) = take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false) { ... }`
pub fn take_hit_weight(
    settings: &ServerSettings,
    gun: Gun,
    has_rifle_shot_hit: &mut bool,
    is_head_hit: bool,
) -> Option<u8> {
    match (gun, is_head_hit) {
        (Gun::Laser, _) => Some(settings.sniper.hits_to_eliminate),
        (Gun::Rifle, _) if *has_rifle_shot_hit => None,
        (Gun::Rifle, is_head) => {
            *has_rifle_shot_hit = true;
            Some(if is_head {
                settings.rifle.head_hits_to_eliminate
            } else {
                settings.rifle.body_hits_to_eliminate
            })
        }
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

/// `ease_frac` eases a fraction from 0 to 1 (smoothstep): slow at both ends.
///
/// Example: `let jog: f64 = ease_frac(0.5);`
fn ease_frac(frac: f64) -> f64 {
    frac * frac * (3.0 - 2.0 * frac)
}

/// `make_wand` places the gun in camera space as Lunar Surface places its wand: held on the right,
/// aimed toward the crosshair with its muzzle at `MUZZLE_M`, dipping a little at each step
/// (`phase_rad`), and, as `run_pose_frac` goes to 1 (eased), lowered, tipped down and rolled,
/// bouncing and swinging with each stride. Recoil (`recoil_frac`, 1 just after a shot) pushes it
/// back toward the eye and tips it up, and `sway` (`get_sway`: meters right, radians of muzzle
/// turned left) slides and swings it with the steps. The result takes the muzzle's frame (the
/// barrel along -z) to camera space.
///
/// Example: `let camera_from_muzzle = make_wand(0.0, 1.2, 0.0, Gun::Laser, (0.0, 0.0));`
fn make_wand(
    run_pose_frac: f64,
    phase_rad: f64,
    recoil_frac: f64,
    gun: Gun,
    sway: (f32, f32),
) -> Mat4 {
    let (sway_m, swing_rad) = sway;
    let jog: f32 = ease_frac(run_pose_frac) as f32;
    let (step, swing) = (phase_rad.sin().abs() as f32, phase_rad.sin() as f32);
    let lerp = |aim: f32, run: f32| aim + (run - aim) * jog;
    let bob_m: f32 = lerp(WAND_WALK_BOB_M, WAND_JOG_BOB_M) * step;
    let reach_m: f32 = match gun {
        Gun::Laser => WAND_LASER_REACH_M,
        Gun::Rifle => WAND_RIFLE_REACH_M,
    };
    let aim_rotation: Mat3 =
        Mat3::from_euler(EulerRot::YXZ, WAND_AIM_YAW_RAD, WAND_AIM_PITCH_RAD, 0.0);
    let aim_center: Vec3 = MUZZLE_M + aim_rotation * (Vec3::Z * reach_m);
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
    Mat4::from_mat3_translation(rotation, center - rotation.z_axis * reach_m)
}

/// `get_sway` gives how the gun in hand sways now with the steps (`phase_rad`, the step phase: a
/// step each half turn), as (meters slid right, radians its muzzle turns left): the hands slide it
/// left on one step and right on the next by `head_bob`'s walking slide, and a longer, heavier gun
/// lags behind them, its muzzle swinging the other way by `head_bob`'s swing for a gun as long as
/// the bolt-action rifle, times `model`'s length over the rifle's. Both grow to their sprinting
/// sizes as the run pose (`run_pose_frac`, eased as `make_wand` eases it) goes to 1, and fade in
/// and out with `step_frac` (0 standing still, 1 stepping).
///
/// Example: `let sway = get_sway(GunModel::Revolver, 1.0, 0.0, 0.5, &HEAD_BOB);`
fn get_sway(
    model: GunModel,
    step_frac: f64,
    run_pose_frac: f64,
    phase_rad: f64,
    head_bob: &HeadBobSettings,
) -> (f32, f32) {
    let jog: f64 = ease_frac(run_pose_frac);
    let lerp = |walk: f64, sprint: f64| walk + (sprint - walk) * jog;
    let length_frac: f64 = f64::from(
        model.get_modelled_length_m() / GunModel::BoltActionRifle.get_modelled_length_m(),
    );
    let side: f64 = step_frac * phase_rad.sin();
    let sway_m: f64 = lerp(head_bob.gun_walk_sway_m, head_bob.gun_sprint_sway_m) * side;
    let swing_rad: f64 = lerp(
        head_bob.rifle_walk_swing_rad,
        head_bob.rifle_sprint_swing_rad,
    ) * length_frac
        * side;
    (sway_m as f32, swing_rad as f32)
}

/// `Tracer` is a fired laser drawn as a beam flying out along its path (`pieces`, traced when it
/// was fired, `length_m` long), as Lunar Surface's: its front leaves the muzzle at `fired_s`
/// (seconds into the match) and moves at `speed_m_per_s`, and each point stays lit for `trail_s`
/// after the front passes it. It is drawn in its gun's style (`is_rifle`) and colour (`rgb`, linear,
/// 0 to 1), and a share `speckle_frac` of its pixels speckles each frame. This player's own shots
/// (`is_own`) stay joined to the gun in hand while their trail still reaches the muzzle.
pub struct Tracer {
    pub pieces: Vec<(Vec3, Vec3)>,
    pub length_m: f32,
    pub rgb: Vec3,
    pub is_rifle: bool,
    pub speckle_frac: f32,
    pub fired_s: f64,
    pub speed_m_per_s: f64,
    pub trail_s: f64,
    pub is_own: bool,
}

impl Tracer {
    /// `New` makes the tracer of a shot along `path` (pieces from the eye, `is_hit` when it hit
    /// something) fired at `fired_s` from `muzzle_m` with the rifle (`is_rifle`) or the sniper,
    /// whose beam speed and trail come from `settings`: the drawn beam leaves the muzzle and joins
    /// the path `LASER_MUZZLE_M` along it, as Lunar Surface's `trace_laser` draws it. A shot that
    /// hit nothing flies only its first `SKY_TRACER_M`.
    ///
    /// Example: `let tracer = Tracer::New(&path.pieces, true, muzzle_m, 12.5, false, 0.1, rgb, &settings);`
    #[allow(non_snake_case, clippy::too_many_arguments)]
    pub fn New(
        path: &[(Vec3, Vec3)],
        is_hit: bool,
        muzzle_m: Vec3,
        fired_s: f64,
        is_rifle: bool,
        speckle_frac: f64,
        rgb: Vec3,
        settings: &ServerSettings,
    ) -> Tracer {
        let (speed_m_per_s, trail_s) = if is_rifle {
            (
                settings.rifle.beam_speed_m_per_s,
                settings.rifle.beam_trail_s,
            )
        } else {
            (
                settings.sniper.beam_speed_m_per_s,
                settings.sniper.beam_trail_s,
            )
        };
        let mut pieces: Vec<(Vec3, Vec3)> = path.to_vec();
        if let Some((start_m, end_m)) = pieces.first().copied() {
            if start_m.distance(end_m) > LASER_MUZZLE_M {
                let joint_m: Vec3 = start_m + (end_m - start_m).normalize() * LASER_MUZZLE_M;
                pieces.splice(0..1, [(muzzle_m, joint_m), (joint_m, end_m)]);
            } else {
                pieces[0].0 = muzzle_m;
            }
        }
        if !is_hit {
            pieces = clip_pieces(&pieces, 0.0, SKY_TRACER_M);
        }
        Tracer {
            length_m: pieces.iter().map(|(a, b)| a.distance(*b)).sum(),
            pieces,
            rgb,
            is_rifle,
            speckle_frac: speckle_frac as f32,
            fired_s,
            speed_m_per_s,
            trail_s,
            is_own: false,
        }
    }

    /// `get_lit_m` gives the lit stretch of the path at `now_s`, as meters from its start: from the
    /// trail's end to the front.
    ///
    /// Example: `let (tail_m, head_m) = tracer.get_lit_m(13.0);`
    pub fn get_lit_m(&self, now_s: f64) -> (f32, f32) {
        let head_m: f64 = self.speed_m_per_s * (now_s - self.fired_s);
        (
            (head_m - self.speed_m_per_s * self.trail_s) as f32,
            head_m as f32,
        )
    }

    /// `get_lit_pieces` gives the lit pieces of the path at `now_s`; an own shot whose trail still
    /// reaches the muzzle starts at `muzzle_m`, the gun's tip as drawn now.
    ///
    /// Example: `let pieces = tracer.get_lit_pieces(13.0, muzzle_m);`
    pub fn get_lit_pieces(&self, now_s: f64, muzzle_m: Vec3) -> Vec<(Vec3, Vec3)> {
        let (tail_m, head_m) = self.get_lit_m(now_s);
        let mut lit: Vec<(Vec3, Vec3)> = clip_pieces(&self.pieces, tail_m, head_m);
        if self.is_own
            && tail_m <= 0.0
            && let Some(first) = lit.first_mut()
        {
            first.0 = muzzle_m;
        }
        lit
    }

    /// `is_over` says whether the trail has passed the end of the path at `now_s`.
    ///
    /// Example: `tracers.retain(|tracer| !tracer.is_over(now_s));`
    pub fn is_over(&self, now_s: f64) -> bool {
        self.get_lit_m(now_s).0 >= self.length_m
    }

    /// `get_arrival_s` gives when the beam's front reaches the end of its path, in seconds into
    /// the match.
    ///
    /// Example: `let start_s = tracer.get_arrival_s();`
    pub fn get_arrival_s(&self) -> f64 {
        self.fired_s + f64::from(self.length_m) / self.speed_m_per_s
    }
}

/// `clip_pieces` gives the parts of `pieces` (a path, piece after piece) between `from_m` and
/// `to_m` along it.
///
/// Example: `let first_m = clip_pieces(&pieces, 0.0, 80.0);`
fn clip_pieces(pieces: &[(Vec3, Vec3)], from_m: f32, to_m: f32) -> Vec<(Vec3, Vec3)> {
    let mut start_along_m: f32 = 0.0;
    let mut clipped: Vec<(Vec3, Vec3)> = Vec::new();
    for (a, b) in pieces {
        let length_m: f32 = a.distance(*b);
        let (lo_m, hi_m) = (
            (from_m - start_along_m).max(0.0),
            (to_m - start_along_m).min(length_m),
        );
        if hi_m > lo_m && length_m > 0.0 {
            let direction: Vec3 = (*b - *a) / length_m;
            clipped.push((*a + direction * lo_m, *a + direction * hi_m));
        }
        start_along_m += length_m;
    }
    clipped
}

/// `tests` checks the guns as Lunar Surface's: the sniper's click, shot and reload, the rifle's
/// fire rate, heat, overheating and recoil, switching, aiming's zoom, hit weights, the gun's
/// pose, and a tracer's flight.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | sniper | click loaded, click reloading, can't shoot | `test_sniper_fires_once_per_reload` |
/// | rifle | held cold, held to overheat, cooling | `test_rifle_overheats_and_cools` |
/// | recoil | rifle shot (up, alternately sideways), sniper (none) | `test_rifle_recoil_alternates` |
/// | switching | `1`/`2`, the wheel, mid-shot | `test_switching_stops_the_shot` |
/// | aim | none, half, full; per gun | `test_aim_zoom_eases_to_the_gun_zoom` |
/// | hit weight | sniper, rifle body, rifle head, second hit of one shot | `test_hit_weights` |
/// | gun pose | aiming (muzzle at `MUZZLE_M`), jogging (lowered) | `test_gun_pose_puts_the_muzzle_at_the_crosshair` |
/// | tracer | before, during and after flight; a miss; an own shot | `test_tracer_flies_and_ends` |
#[cfg(test)]
mod tests {
    use super::*;

    const DT_S: f64 = 1.0 / 128.0;
    const SEED: u64 = 69;
    const TOLERANCE_M: f32 = 1e-5;

    /// `update_for` runs `guns` for `seconds_s` with `input` and returns the summed view kick.
    fn update_for(guns: &mut Guns, input: &GunInput, can_shoot: bool, seconds_s: f64) -> Vec2 {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut kick_rad: Vec2 = Vec2::ZERO;
        for _ in 0..(seconds_s / DT_S).round() as usize {
            kick_rad += guns.update(input, &settings, can_shoot, &GunStep::default(), DT_S);
        }
        kick_rad
    }

    /// `click` is a click of the trigger.
    fn click() -> GunInput {
        GunInput {
            is_trigger_held: true,
            is_shoot_pressed: true,
            ..GunInput::default()
        }
    }

    /// `rifle_held` is the rifle in hand with the trigger held.
    fn rifle_held() -> GunInput {
        GunInput {
            is_trigger_held: true,
            picked_gun: Some(Gun::Rifle),
            ..GunInput::default()
        }
    }

    /// `test_sniper_fires_once_per_reload` checks a click fires a `shot_s` laser, a click while
    /// reloading or when the player can't shoot doesn't, and the sniper fires again after
    /// `reload_s`.
    #[test]
    fn test_sniper_fires_once_per_reload() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut guns: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        update_for(&mut guns, &click(), false, DT_S);
        assert!(!guns.is_firing());
        update_for(&mut guns, &click(), true, DT_S);
        assert!(guns.is_firing());
        update_for(
            &mut guns,
            &GunInput::default(),
            true,
            settings.sniper.shot_s,
        );
        assert!(!guns.is_firing());
        update_for(&mut guns, &click(), true, DT_S);
        assert!(!guns.is_firing(), "a click while reloading fires");
        update_for(
            &mut guns,
            &GunInput::default(),
            true,
            settings.sniper.reload_s,
        );
        update_for(&mut guns, &click(), true, DT_S);
        assert!(guns.is_firing());
    }

    /// `test_rifle_overheats_and_cools` checks the rifle fires `shots_per_s`, overheats after
    /// `shots_until_overheat` shots, stops firing, and cools in `cool_down_s`.
    #[test]
    fn test_rifle_overheats_and_cools() {
        let rifle: RifleSettings = ServerSettings::Builtin().rifle;
        let mut guns: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        let (mut shot_count, mut held_s, mut was_firing) = (0_u32, 0.0_f64, false);
        while !guns.is_overheated {
            update_for(&mut guns, &rifle_held(), true, DT_S);
            shot_count += u32::from(guns.is_firing() && !was_firing);
            was_firing = guns.is_firing();
            held_s += DT_S;
            assert!(
                held_s < rifle.cool_down_s * 10.0,
                "the rifle never overheats"
            );
        }
        assert_eq!(shot_count, rifle.shots_until_overheat);
        let shot_period_s: f64 = 1.0 / rifle.shots_per_s;
        let overheat_s: f64 = f64::from(rifle.shots_until_overheat - 1) * shot_period_s;
        assert!((held_s - overheat_s).abs() < shot_period_s);
        update_for(&mut guns, &rifle_held(), true, rifle.pulse_s + DT_S);
        assert!(!guns.is_firing(), "an overheated rifle fires");
        update_for(
            &mut guns,
            &GunInput::default(),
            true,
            rifle.cool_down_s + DT_S,
        );
        assert!(!guns.is_overheated && guns.rifle_heat_frac == 0.0);
    }

    /// `test_rifle_recoil_alternates` checks two rifle shots kick the view up twice and sideways
    /// left then right, and a sniper shot doesn't kick it.
    #[test]
    fn test_rifle_recoil_alternates() {
        let rifle: RifleSettings = ServerSettings::Builtin().rifle;
        let mut guns: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        let first_rad: Vec2 = update_for(&mut guns, &rifle_held(), true, DT_S);
        let second_rad: Vec2 = update_for(
            &mut guns,
            &rifle_held(),
            true,
            1.0 / rifle.shots_per_s + 2.0 * DT_S,
        );
        assert_eq!(first_rad.y, rifle.recoil_up_rad as f32);
        assert_eq!(first_rad.x, -second_rad.x);
        assert_eq!(first_rad.x.abs(), rifle.recoil_sideways_rad as f32);
        let mut sniper: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        assert_eq!(update_for(&mut sniper, &click(), true, DT_S), Vec2::ZERO);
    }

    /// `test_switching_stops_the_shot` checks `1`, `2` and the wheel pick the guns, and switching
    /// stops a laser in flight.
    #[test]
    fn test_switching_stops_the_shot() {
        let mut guns: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        update_for(&mut guns, &click(), true, DT_S);
        assert!(guns.is_firing());
        let swap: GunInput = GunInput {
            is_swap_pressed: true,
            ..GunInput::default()
        };
        update_for(&mut guns, &swap, true, DT_S);
        assert!(guns.gun == Gun::Rifle && !guns.is_firing());
        update_for(&mut guns, &swap, true, DT_S);
        assert_eq!(guns.gun, Gun::Laser);
        let pick_rifle: GunInput = GunInput {
            picked_gun: Some(Gun::Rifle),
            ..GunInput::default()
        };
        update_for(&mut guns, &pick_rifle, true, DT_S);
        assert_eq!(guns.gun, Gun::Rifle);
    }

    /// `test_aim_zoom_eases_to_the_gun_zoom` checks the zoom is 1 unaimed, its gun's full zoom
    /// after `zoom_s`, and halfway between at half the aim.
    #[test]
    fn test_aim_zoom_eases_to_the_gun_zoom() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut guns: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        assert_eq!(guns.get_zoom(&settings), 1.0);
        let aim: GunInput = GunInput {
            is_aim_held: true,
            ..GunInput::default()
        };
        update_for(&mut guns, &aim, true, settings.aim.zoom_s + DT_S);
        assert!((guns.get_zoom(&settings) - settings.aim.sniper_zoom).abs() < 1e-9);
        guns.switch_gun(Gun::Rifle);
        assert!((guns.get_zoom(&settings) - settings.aim.rifle_zoom).abs() < 1e-9);
        assert_eq!(get_aim_zoom(0.5, 3.0), 2.0);
    }

    /// `test_hit_weights` checks the sniper eliminates in its hits every frame, and a rifle shot
    /// counts once, as a head or a body hit.
    #[test]
    fn test_hit_weights() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut has_hit: bool = false;
        assert_eq!(
            take_hit_weight(&settings, Gun::Laser, &mut has_hit, false),
            Some(settings.sniper.hits_to_eliminate)
        );
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, true),
            Some(settings.rifle.head_hits_to_eliminate)
        );
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false),
            None
        );
        has_hit = false;
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false),
            Some(settings.rifle.body_hits_to_eliminate)
        );
    }

    /// `test_gun_pose_puts_the_muzzle_at_the_crosshair` checks the muzzle of a gun at rest is at
    /// `MUZZLE_M`, pointing near the crosshair, and that jogging lowers it.
    #[test]
    fn test_gun_pose_puts_the_muzzle_at_the_crosshair() {
        let mut guns: Guns = Guns::New(ShotNoise::FromSeed(SEED));
        for gun in [Gun::Laser, Gun::Rifle] {
            guns.switch_gun(gun);
            let pose: Mat4 = guns.get_camera_from_muzzle();
            assert!(pose.w_axis.truncate().distance(MUZZLE_M) < TOLERANCE_M);
            assert!(pose.z_axis.truncate().dot(Vec3::Z) > 0.99);
        }
        let sprint: GunStep = GunStep {
            is_sprinting: true,
            ..GunStep::default()
        };
        let settings: ServerSettings = ServerSettings::Builtin();
        for _ in 0..60 {
            guns.update(&GunInput::default(), &settings, true, &sprint, DT_S);
        }
        assert!(guns.get_camera_from_muzzle().w_axis.y < MUZZLE_M.y);
    }

    /// `test_tracer_flies_and_ends` checks a tracer joins its path from the muzzle, lights only up
    /// to its front, ends after its trail passes its end, and that a miss flies `SKY_TRACER_M`.
    #[test]
    fn test_tracer_flies_and_ends() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let path: [(Vec3, Vec3); 1] = [(Vec3::ZERO, Vec3::new(0.0, 0.0, -50.0))];
        let muzzle_m: Vec3 = Vec3::new(0.2, -0.2, -0.5);
        let tracer: Tracer =
            Tracer::New(&path, true, muzzle_m, 0.0, false, 0.0, Vec3::ONE, &settings);
        assert_eq!(tracer.pieces[0].0, muzzle_m);
        assert_eq!(tracer.pieces[1].0, Vec3::new(0.0, 0.0, -LASER_MUZZLE_M));
        let speed_m_per_s: f64 = settings.sniper.beam_speed_m_per_s;
        let lit: Vec<(Vec3, Vec3)> = tracer.get_lit_pieces(10.0 / speed_m_per_s, muzzle_m);
        let lit_m: f32 = lit.iter().map(|(a, b)| a.distance(*b)).sum();
        assert!((lit_m - 10.0).abs() < 1e-3);
        assert!(!tracer.is_over(tracer.get_arrival_s()));
        assert!(tracer.is_over(tracer.get_arrival_s() + settings.sniper.beam_trail_s + DT_S));
        let far: [(Vec3, Vec3); 1] = [(Vec3::ZERO, Vec3::new(0.0, 0.0, -400.0))];
        let miss: Tracer = Tracer::New(
            &far,
            false,
            Vec3::ZERO,
            0.0,
            true,
            0.0,
            Vec3::ONE,
            &settings,
        );
        assert!((miss.length_m - SKY_TRACER_M).abs() < 1e-3);
    }
}
