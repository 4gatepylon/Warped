//! `guns` plays this level's two guns on the shared `Weapons` (`warped_shared::weapons`, Lunar
//! Surface's guns, W-126 (same play in every level)): each frame's controls (`update_weapons`:
//! picking or swapping a gun, a sniper shot on a click, the shot and reload timers, aiming, the
//! gun's steps and pose, and the rifle), the gun in hand's place (`get_camera_from_muzzle`,
//! `get_muzzle_m`), and each shot's beam flying out (`Tracer`). Their numbers are the server
//! settings' (`settings.rs`) and the head bob's (`settings::HEAD_BOB`).
//!
//! Interface: `Gun`, `GunModel`, `Weapons`, `GunInput`, `GunStep`, `make_weapons`,
//! `update_weapons`, `get_camera_from_muzzle`, `get_muzzle_m`, `Tracer`,
//! `MUZZLE_M`, `HALF_FOV_RAD`, `LASER_MAX_LENGTH_M`, `LASER_MUZZLE_M`.
//!
//! Example:
//!     let kick_rad: Vec2 = update_weapons(&mut weapons, &input, &settings, true, &step, dt_s);
//!     let camera_from_muzzle: Mat4 = get_camera_from_muzzle(&weapons);

use glam::{Mat4, Vec2, Vec3};
pub use warped_shared::viewmodel::GunModel;
pub use warped_shared::weapons::{Gun, Weapons};
use warped_shared::weapons::{get_sway, make_wand, make_wand_camera_from_muzzle};

use crate::settings::{HEAD_BOB, ServerSettings};
use crate::shot_noise::ShotNoise;

/// `MUZZLE_M` is where the gun in hand's muzzle is in camera space while aiming (x right, y up,
/// -z ahead), where the laser starts: the shared `weapons::MUZZLE_M`.
pub const MUZZLE_M: Vec3 = Vec3::new(
    warped_shared::weapons::MUZZLE_M.x as f32,
    warped_shared::weapons::MUZZLE_M.y as f32,
    warped_shared::weapons::MUZZLE_M.z as f32,
);
/// `HALF_FOV_RAD` is half the view's vertical angle before aiming zooms it: Lunar Surface's.
pub const HALF_FOV_RAD: f32 = 37.5 * std::f32::consts::PI / 180.0;
/// `LASER_MAX_LENGTH_M` is the farthest a shot reaches, and `LASER_MUZZLE_M` how far along its
/// path the drawn beam joins it from the muzzle: Lunar Surface's.
pub const LASER_MAX_LENGTH_M: f32 = 480.0;
pub const LASER_MUZZLE_M: f32 = 3.0;
/// `SKY_TRACER_M` is how far the beam of a shot that hit nothing flies: Lunar Surface's.
const SKY_TRACER_M: f32 = 80.0;
/// `GUN_POSE_PER_S` is how fast the gun moves between its aiming and jogging poses.
const GUN_POSE_PER_S: f64 = 6.0;
/// `WAND_TEAM` is the team `make_wand` colours its tip by; the forest draws the gun model, not
/// the wand, so it only places the gun.
const WAND_TEAM: u32 = 0;

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

/// `make_weapons` gives a player's guns with the sniper in hand, loaded and cold, its shots
/// strayed by `shot_noise`.
///
/// Example: `let weapons: Weapons = make_weapons(ShotNoise::FromClock());`
pub fn make_weapons(shot_noise: ShotNoise) -> Weapons {
    let mut weapons: Weapons = Weapons::New(shot_noise);
    weapons.switch_gun(Gun::Laser);
    weapons
}

/// `update_weapons` takes this frame's controls (`input`) and moves `weapons` on by `dt_s` with
/// `settings`, as Lunar Surface does: the gun picked or swapped, a sniper shot on a click when it
/// is loaded, the shot and reload timers, aiming, the gun in hand's steps and pose toward jogging
/// (with the player's `step`), and the rifle. `can_shoot` says whether the player may fire now.
/// It gives how far the rifle's recoil turns the view this frame, in radians (x right, y up).
///
/// Example: `let kick_rad: Vec2 = update_weapons(&mut weapons, &input, &settings, true, &step, 1.0 / 60.0);`
pub fn update_weapons(
    weapons: &mut Weapons,
    input: &GunInput,
    settings: &ServerSettings,
    can_shoot: bool,
    step: &GunStep,
    dt_s: f64,
) -> Vec2 {
    if let Some(gun) = input.picked_gun {
        weapons.switch_gun(gun);
    }
    if input.is_swap_pressed {
        weapons.switch_gun(match weapons.gun {
            Gun::Laser => Gun::Rifle,
            Gun::Rifle => Gun::Laser,
        });
    }
    if input.is_shoot_pressed
        && weapons.gun == Gun::Laser
        && weapons.sniper_reload_s <= 0.0
        && can_shoot
    {
        weapons.fire_sniper(&settings.sniper);
    }
    weapons.tick(dt_s);
    weapons.update_aim(input.is_aim_held, dt_s, &settings.aim);
    if !can_shoot {
        weapons.shot_time_s = 0.0;
    }
    (weapons.gun_phase_rad, weapons.step_frac) = (step.bob_phase_rad, step.bob_frac);
    let target: f64 = f64::from(u8::from(step.is_sprinting));
    weapons.run_pose_frac +=
        (target - weapons.run_pose_frac).clamp(-GUN_POSE_PER_S * dt_s, GUN_POSE_PER_S * dt_s);
    weapons.is_trigger_held = input.is_trigger_held;
    weapons
        .update_rifle(&settings.rifle, can_shoot, dt_s)
        .map_or(Vec2::ZERO, |[side_rad, down_rad]| {
            Vec2::new(side_rad as f32, (-down_rad) as f32)
        })
}

/// `get_camera_from_muzzle` places the gun in hand in camera space as every level places it
/// (`make_wand`, swayed by `get_sway`): its muzzle at the origin of the result, its barrel along
/// -z.
///
/// Example: `let camera_from_model = get_camera_from_muzzle(&weapons) * muzzle_from_gltf;`
pub fn get_camera_from_muzzle(weapons: &Weapons) -> Mat4 {
    let sway: (f32, f32) = get_sway(
        weapons.gun.get_model(),
        weapons.step_frac,
        weapons.run_pose_frac,
        weapons.gun_phase_rad,
        &HEAD_BOB,
    );
    make_wand_camera_from_muzzle(&make_wand(
        weapons.run_pose_frac,
        weapons.gun_phase_rad,
        weapons.recoil_frac,
        WAND_TEAM,
        weapons.gun,
        sway,
    ))
}

/// `get_muzzle_m` gives where the tip of the gun in hand shows from the eye, in camera space:
/// moved toward the view's centre by the aim `zoom`, which magnifies the scene but not the gun.
///
/// Example: `let muzzle_m: Vec3 = camera_world.transform_point3(get_muzzle_m(&weapons, zoom));`
pub fn get_muzzle_m(weapons: &Weapons, zoom: f64) -> Vec3 {
    let tip_m: Vec3 = get_camera_from_muzzle(weapons).w_axis.truncate();
    let zoom: f32 = zoom as f32;
    Vec3::new(tip_m.x / zoom, tip_m.y / zoom, tip_m.z)
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
/// fire rate, heat, overheating and recoil, switching, aiming's zoom, the gun's pose, and a
/// tracer's flight, through `update_weapons` (the shared `weapons` tests the hits' weights).
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | sniper | click loaded, click reloading, can't shoot | `test_sniper_fires_once_per_reload` |
/// | rifle | held cold, held to overheat, cooling | `test_rifle_overheats_and_cools` |
/// | recoil | rifle shot (up, alternately sideways), sniper (none) | `test_rifle_recoil_alternates` |
/// | switching | `1`/`2`, the wheel, mid-shot | `test_switching_stops_the_shot` |
/// | aim | none, half, full; per gun | `test_aim_zoom_eases_to_the_gun_zoom` |
/// | gun pose | aiming (muzzle at `MUZZLE_M`), jogging (lowered) | `test_gun_pose_puts_the_muzzle_at_the_crosshair` |
/// | tracer | before, during and after flight; a miss; an own shot | `test_tracer_flies_and_ends` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::RifleSettings;
    use warped_shared::weapons::get_aim_zoom;

    const DT_S: f64 = 1.0 / 128.0;
    const SEED: u64 = 69;
    const TOLERANCE_M: f32 = 1e-5;

    /// `update_for` runs `guns` for `seconds_s` with `input` and returns the summed view kick.
    fn update_for(guns: &mut Weapons, input: &GunInput, can_shoot: bool, seconds_s: f64) -> Vec2 {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut kick_rad: Vec2 = Vec2::ZERO;
        for _ in 0..(seconds_s / DT_S).round() as usize {
            kick_rad +=
                update_weapons(guns, input, &settings, can_shoot, &GunStep::default(), DT_S);
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
        let mut guns: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
        update_for(&mut guns, &click(), false, DT_S);
        assert!(!guns.is_firing(true));
        update_for(&mut guns, &click(), true, DT_S);
        assert!(guns.is_firing(true));
        update_for(
            &mut guns,
            &GunInput::default(),
            true,
            settings.sniper.shot_s,
        );
        assert!(!guns.is_firing(true));
        update_for(&mut guns, &click(), true, DT_S);
        assert!(!guns.is_firing(true), "a click while reloading fires");
        update_for(
            &mut guns,
            &GunInput::default(),
            true,
            settings.sniper.reload_s,
        );
        update_for(&mut guns, &click(), true, DT_S);
        assert!(guns.is_firing(true));
    }

    /// `test_rifle_overheats_and_cools` checks the rifle fires `shots_per_s`, overheats after
    /// `shots_until_overheat` shots, stops firing, and cools in `cool_down_s`.
    #[test]
    fn test_rifle_overheats_and_cools() {
        let rifle: RifleSettings = ServerSettings::Builtin().rifle;
        let mut guns: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
        let (mut shot_count, mut held_s, mut was_firing) = (0_u32, 0.0_f64, false);
        while !guns.is_overheated {
            update_for(&mut guns, &rifle_held(), true, DT_S);
            shot_count += u32::from(guns.is_firing(true) && !was_firing);
            was_firing = guns.is_firing(true);
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
        assert!(!guns.is_firing(true), "an overheated rifle fires");
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
        let mut guns: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
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
        let mut sniper: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
        assert_eq!(update_for(&mut sniper, &click(), true, DT_S), Vec2::ZERO);
    }

    /// `test_switching_stops_the_shot` checks `1`, `2` and the wheel pick the guns, and switching
    /// stops a laser in flight.
    #[test]
    fn test_switching_stops_the_shot() {
        let mut guns: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
        update_for(&mut guns, &click(), true, DT_S);
        assert!(guns.is_firing(true));
        let swap: GunInput = GunInput {
            is_swap_pressed: true,
            ..GunInput::default()
        };
        update_for(&mut guns, &swap, true, DT_S);
        assert!(guns.gun == Gun::Rifle && !guns.is_firing(true));
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
        let mut guns: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
        assert_eq!(guns.get_zoom(&settings.aim), 1.0);
        let aim: GunInput = GunInput {
            is_aim_held: true,
            ..GunInput::default()
        };
        update_for(&mut guns, &aim, true, settings.aim.zoom_s + DT_S);
        assert!((guns.get_zoom(&settings.aim) - settings.aim.sniper_zoom).abs() < 1e-9);
        guns.switch_gun(Gun::Rifle);
        assert!((guns.get_zoom(&settings.aim) - settings.aim.rifle_zoom).abs() < 1e-9);
        assert_eq!(get_aim_zoom(0.5, 3.0), 2.0);
    }

    /// `test_gun_pose_puts_the_muzzle_at_the_crosshair` checks the muzzle of a gun at rest is at
    /// `MUZZLE_M`, pointing near the crosshair, and that jogging lowers it.
    #[test]
    fn test_gun_pose_puts_the_muzzle_at_the_crosshair() {
        let mut guns: Weapons = make_weapons(ShotNoise::FromSeed(SEED));
        for gun in [Gun::Laser, Gun::Rifle] {
            guns.switch_gun(gun);
            let pose: Mat4 = get_camera_from_muzzle(&guns);
            assert!(pose.w_axis.truncate().distance(MUZZLE_M) < TOLERANCE_M);
            assert!(pose.z_axis.truncate().dot(Vec3::Z) > 0.99);
        }
        let sprint: GunStep = GunStep {
            is_sprinting: true,
            ..GunStep::default()
        };
        let settings: ServerSettings = ServerSettings::Builtin();
        for _ in 0..60 {
            update_weapons(
                &mut guns,
                &GunInput::default(),
                &settings,
                true,
                &sprint,
                DT_S,
            );
        }
        assert!(get_camera_from_muzzle(&guns).w_axis.y < MUZZLE_M.y);
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
