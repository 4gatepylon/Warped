//! `shot_noise.rs` makes each laser shot stray a little from the crosshair (W-69): by a random
//! angle, toward a random direction around the crosshair, both sampled once per shot from a small
//! seeded generator. The sniper's angle is exponential with mean `sniper.noise_mean_rad`, cut at
//! `sniper.noise_max_rad`, so most shots land near the middle and a few stray further. The
//! rifle's shots land uniformly over a disc around the crosshair, whose angular radius grows in
//! step with the rifle's heat, from `rifle.noise_start_rad` cold to `rifle.noise_max_rad` at full
//! heat, and shrinks back as it cools.
//!
//! Each shot also gets its beam's speckle (W-84): the share of the beam's pixels that speckle each
//! frame, painted black, turned white or dropping out (`render_voxel.metal`). The sniper's is
//! exponential with mean `sniper.beam_speckle_mean_frac`, cut at 1; the rifle's grows in step with
//! the heat, from `rifle.beam_speckle_start_frac` cold to `rifle.beam_speckle_max_frac`. Both are
//! rounded to steps of 1 / `BEAM_SPECKLE_LEVEL_COUNT`, which the network carries.
//!
//! Interface: `ShotNoise` (`FromSeed`, `FromClock`, `sample_sniper`, `sample_rifle`,
//! `sample_sniper_speckle`), `ShotOffset` (`NONE`, `aim`), `get_rifle_spread_rad`,
//! `get_rifle_speckle_frac`, `BEAM_SPECKLE_LEVEL_COUNT`, `round_speckle_frac`.
//!
//! Example: `let laser_frame = noise.sample_sniper(&settings.sniper).aim(camera);`
//!
//! Must know:
//! - Gotchas:
//!   - A shot is traced from the camera turned by its offset (`ShotOffset::aim`), so its beam, its
//!     spot and its hit agree, and the window sends that turned frame while firing, so other windows
//!     trace the same strayed shot.

use std::f64::consts::TAU;
use std::time::{SystemTime, UNIX_EPOCH};

use glam::{DMat4, DVec3};

use crate::geometry::make_rotation_between;
use crate::settings::{RifleSettings, SniperSettings};

const SPLITMIX_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;
const SPLITMIX_MULTIPLIERS: [u64; 2] = [0xbf58_476d_1ce4_e5b9, 0x94d0_49bb_1331_11eb];
const SPLITMIX_SHIFTS: [u32; 3] = [30, 27, 31];
const MANTISSA_BITS: u32 = 53;
const BITS_PER_U64: u32 = 64;
pub const BEAM_SPECKLE_LEVEL_COUNT: u8 = 31;
const MAX_SPECKLE_FRAC: f64 = 1.0;

/// `ShotOffset` is how far one shot strays from the crosshair: the angle between the shot and the
/// crosshair's direction, and the direction around the crosshair it strays toward (0 to the
/// right, a quarter turn up).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ShotOffset {
    pub angle_rad: f64,
    pub direction_rad: f64,
}

impl ShotOffset {
    pub const NONE: ShotOffset = ShotOffset {
        angle_rad: 0.0,
        direction_rad: 0.0,
    };

    /// `aim` gives the frame a shot is traced from: `camera` (a frame looking along its -z, with x
    /// right and y up) turned by the smallest rotation that takes the crosshair's direction to the
    /// strayed one. Its position is `camera`'s, and `NONE` gives `camera` itself.
    ///
    /// Example: `let shot = world.trace_laser(offset.aim(camera));`
    pub fn aim(&self, camera: DMat4) -> DMat4 {
        let (sin_angle, cos_angle) = self.angle_rad.sin_cos();
        let (sin_direction, cos_direction) = self.direction_rad.sin_cos();
        let strayed: DVec3 = DVec3::new(
            sin_angle * cos_direction,
            sin_angle * sin_direction,
            -cos_angle,
        );
        camera * make_rotation_between(DVec3::NEG_Z, strayed)
    }
}

/// `ShotNoise` samples shots' offsets from a SplitMix64 generator, which is small, fast and good
/// enough for a game (not for cryptography).
pub struct ShotNoise {
    state: u64,
}

impl ShotNoise {
    /// `FromSeed` starts a generator at `seed`; the same seed gives the same offsets.
    ///
    /// Example: `let mut noise = ShotNoise::FromSeed(7);`
    #[allow(non_snake_case)]
    pub fn FromSeed(seed: u64) -> ShotNoise {
        ShotNoise { state: seed }
    }

    /// `FromClock` starts a generator seeded by the time now, in nanoseconds, so each game strays
    /// differently.
    ///
    /// Example: `let mut noise = ShotNoise::FromClock();`
    #[allow(non_snake_case)]
    pub fn FromClock() -> ShotNoise {
        let since_epoch_ns: u128 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is after 1970")
            .as_nanos();
        ShotNoise::FromSeed(since_epoch_ns as u64)
    }

    /// `sample_frac` gives the next number of the generator, uniform in [0, 1).
    ///
    /// Example: `let u = self.sample_frac();`
    fn sample_frac(&mut self) -> f64 {
        self.state = self.state.wrapping_add(SPLITMIX_GAMMA);
        let mut z: u64 = self.state;
        z = (z ^ (z >> SPLITMIX_SHIFTS[0])).wrapping_mul(SPLITMIX_MULTIPLIERS[0]);
        z = (z ^ (z >> SPLITMIX_SHIFTS[1])).wrapping_mul(SPLITMIX_MULTIPLIERS[1]);
        z ^= z >> SPLITMIX_SHIFTS[2];
        (z >> (BITS_PER_U64 - MANTISSA_BITS)) as f64 / (1_u64 << MANTISSA_BITS) as f64
    }

    /// `sample_sniper` gives one sniper shot's offset: an angle from an exponential distribution
    /// with mean `sniper.noise_mean_rad`, cut at `sniper.noise_max_rad`, toward a uniform
    /// direction.
    ///
    /// Example: `self.shot_offset = self.shot_noise.sample_sniper(&self.settings.sniper);`
    pub fn sample_sniper(&mut self, sniper: &SniperSettings) -> ShotOffset {
        let exponential_rad: f64 = -sniper.noise_mean_rad * (1.0 - self.sample_frac()).ln();
        ShotOffset {
            angle_rad: exponential_rad.min(sniper.noise_max_rad),
            direction_rad: TAU * self.sample_frac(),
        }
    }

    /// `sample_rifle` gives one rifle shot's offset, fired at `heat_frac` (0 cold, 1 overheated):
    /// uniform over the disc of angular radius `get_rifle_spread_rad` around the crosshair.
    ///
    /// Example: `self.shot_offset = self.shot_noise.sample_rifle(&rifle, self.rifle_heat_frac);`
    pub fn sample_rifle(&mut self, rifle: &RifleSettings, heat_frac: f64) -> ShotOffset {
        let spread_rad: f64 = get_rifle_spread_rad(rifle, heat_frac);
        ShotOffset {
            angle_rad: spread_rad * self.sample_frac().sqrt(),
            direction_rad: TAU * self.sample_frac(),
        }
    }

    /// `sample_sniper_speckle` gives one sniper shot's beam speckle: a share from an exponential
    /// distribution with mean `sniper.beam_speckle_mean_frac`, cut at 1 (`MAX_SPECKLE_FRAC`), and
    /// rounded (`round_speckle_frac`).
    ///
    /// Example: `self.beam_speckle_frac = self.shot_noise.sample_sniper_speckle(&self.settings.sniper);`
    pub fn sample_sniper_speckle(&mut self, sniper: &SniperSettings) -> f64 {
        let exponential_frac: f64 =
            -sniper.beam_speckle_mean_frac * (1.0 - self.sample_frac()).ln();
        round_speckle_frac(exponential_frac.min(MAX_SPECKLE_FRAC))
    }
}

/// `get_rifle_spread_rad` gives the most a rifle shot fired at `heat_frac` strays: from
/// `rifle.noise_start_rad` cold (0) to `rifle.noise_max_rad` at full heat (1), in step with the
/// heat. It panics on a heat outside [0, 1].
///
/// Example: `let spread_rad = get_rifle_spread_rad(&settings.rifle, 0.5);`
pub fn get_rifle_spread_rad(rifle: &RifleSettings, heat_frac: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&heat_frac),
        "get_rifle_spread_rad: heat_frac must be in [0, 1], not {heat_frac}"
    );
    rifle.noise_start_rad * (1.0 - heat_frac) + rifle.noise_max_rad * heat_frac
}

/// `get_rifle_speckle_frac` gives the beam speckle of a rifle shot fired at `heat_frac`: from
/// `rifle.beam_speckle_start_frac` cold (0) to `rifle.beam_speckle_max_frac` at full heat (1), in
/// step with the heat, rounded (`round_speckle_frac`). It panics on a heat outside [0, 1].
///
/// Example: `self.beam_speckle_frac = get_rifle_speckle_frac(&rifle, self.rifle_heat_frac);`
pub fn get_rifle_speckle_frac(rifle: &RifleSettings, heat_frac: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&heat_frac),
        "get_rifle_speckle_frac: heat_frac must be in [0, 1], not {heat_frac}"
    );
    round_speckle_frac(
        rifle.beam_speckle_start_frac * (1.0 - heat_frac) + rifle.beam_speckle_max_frac * heat_frac,
    )
}

/// `round_speckle_frac` rounds a beam speckle share in [0, 1] to the nearest step of
/// 1 / `BEAM_SPECKLE_LEVEL_COUNT`, the steps the network carries, so every window draws a shot's
/// beam with the same share. It panics on a share outside [0, 1].
///
/// Example: `let speckle_frac = round_speckle_frac(0.12);`
pub fn round_speckle_frac(speckle_frac: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&speckle_frac),
        "round_speckle_frac: speckle_frac must be in [0, 1], not {speckle_frac}"
    );
    let level_count: f64 = f64::from(BEAM_SPECKLE_LEVEL_COUNT);
    (speckle_frac * level_count).round() / level_count
}

#[cfg(test)]
mod tests {
    //! Tests of sampling shots' offsets and turning the camera by them.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Gun | sniper, rifle | `sniper_strays_exponentially_up_to_the_cap`, `rifle_spread_grows_with_heat` |
    //! | Sniper noise | on (the built-in mean and cap), off (mean 0) | `sniper_strays_exponentially_up_to_the_cap`, `no_noise_keeps_the_crosshair` |
    //! | Rifle heat | cold (0), half, full (1), outside [0, 1] (panics) | `rifle_spread_grows_with_heat`, `heat_outside_its_range_fails` |
    //! | Seed | the same twice, two different | `seeds_repeat_their_offsets` |
    //! | Offset | none (the camera itself), a stray (its angle and direction, the position kept) | `no_noise_keeps_the_crosshair`, `aim_turns_the_crosshair_by_the_offset` |
    //! | Beam speckle | the sniper's (exponential, cut at 1, rounded), the rifle's (cold, half, full heat), off (0) | `sniper_speckle_is_exponential_up_to_1`, `rifle_speckle_grows_with_heat` |
    //! | Share to round | 0, 1, between steps, outside [0, 1] (panics) | `speckles_round_to_network_steps`, `speckle_outside_its_range_fails` |

    use super::*;
    use crate::geometry::{
        AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, make_boost, make_rotation, minkowski_dot,
    };
    use crate::settings::ServerSettings;
    use glam::DVec4;

    const SEED: u64 = 69;
    const SAMPLE_COUNT: usize = 200_000;
    const MEAN_TOLERANCE_FRAC: f64 = 0.01;
    const DIRECTION_TOLERANCE_FRAC: f64 = 0.01;
    const ANGLE_TOLERANCE_RAD: f64 = 1e-12;
    const DIRECTION_TOLERANCE_RAD: f64 = 1e-9;
    const MEAN_PER_SPREAD_OF_A_DISC: f64 = 2.0 / 3.0;
    const HEATS_FRAC: [f64; 3] = [0.0, 0.5, 1.0];
    const BEYOND_FULL_HEAT_FRAC: f64 = 1.5;
    const ROUNDS_DOWN_STEPS: f64 = 2.4;
    const ROUNDS_UP_STEPS: f64 = 2.6;
    const REPEAT_COUNT: usize = 100;
    const CAMERA_BOOST: f64 = 0.4;
    const CAMERA_YAW_RAD: f64 = 0.7;
    const CAMERA_PITCH_RAD: f64 = 0.2;
    const STRAY: ShotOffset = ShotOffset {
        angle_rad: 0.01,
        direction_rad: 2.0,
    };

    /// `get_mean` gives the mean of `values`.
    ///
    /// Example: `let mean = get_mean(&[1.0, 2.0]);`
    fn get_mean(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    /// `make_test_camera` gives a camera away from the origin, turned and tilted.
    ///
    /// Example: `let camera = make_test_camera();`
    fn make_test_camera() -> DMat4 {
        make_boost(AXIS_X, CAMERA_BOOST)
            * make_rotation(AXIS_X, AXIS_Z, CAMERA_YAW_RAD)
            * make_rotation(AXIS_Z, AXIS_Y, CAMERA_PITCH_RAD)
    }

    #[test]
    fn sniper_strays_exponentially_up_to_the_cap() {
        let sniper: SniperSettings = ServerSettings::Builtin().sniper;
        let mut noise: ShotNoise = ShotNoise::FromSeed(SEED);
        let offsets: Vec<ShotOffset> = (0..SAMPLE_COUNT)
            .map(|_| noise.sample_sniper(&sniper))
            .collect();
        let angles_rad: Vec<f64> = offsets.iter().map(|o| o.angle_rad).collect();
        let (mean_rad, cap_rad) = (sniper.noise_mean_rad, sniper.noise_max_rad);
        let capped_mean_rad: f64 = mean_rad * (1.0 - (-cap_rad / mean_rad).exp());
        assert!((get_mean(&angles_rad) - capped_mean_rad).abs() < MEAN_TOLERANCE_FRAC * mean_rad);
        assert!((capped_mean_rad - mean_rad).abs() < MEAN_TOLERANCE_FRAC * mean_rad);
        let median_rad: f64 = {
            let mut sorted: Vec<f64> = angles_rad.clone();
            sorted.sort_by(f64::total_cmp);
            sorted[SAMPLE_COUNT / 2]
        };
        assert!(
            (median_rad - mean_rad * std::f64::consts::LN_2).abs() < MEAN_TOLERANCE_FRAC * mean_rad
        );
        assert!(angles_rad.iter().all(|&a| (0.0..=cap_rad).contains(&a)));
        assert!(angles_rad.contains(&cap_rad));
        let mean_x: f64 = get_mean(
            &offsets
                .iter()
                .map(|o| o.direction_rad.cos())
                .collect::<Vec<f64>>(),
        );
        let mean_y: f64 = get_mean(
            &offsets
                .iter()
                .map(|o| o.direction_rad.sin())
                .collect::<Vec<f64>>(),
        );
        assert!(mean_x.abs() < DIRECTION_TOLERANCE_FRAC && mean_y.abs() < DIRECTION_TOLERANCE_FRAC);
    }

    #[test]
    fn rifle_spread_grows_with_heat() {
        let rifle: RifleSettings = ServerSettings::Builtin().rifle;
        let mut noise: ShotNoise = ShotNoise::FromSeed(SEED);
        let mut last_mean_rad: f64 = 0.0;
        for heat_frac in HEATS_FRAC {
            let spread_rad: f64 = get_rifle_spread_rad(&rifle, heat_frac);
            let angles_rad: Vec<f64> = (0..SAMPLE_COUNT)
                .map(|_| noise.sample_rifle(&rifle, heat_frac).angle_rad)
                .collect();
            let mean_rad: f64 = get_mean(&angles_rad);
            assert!(
                (mean_rad - MEAN_PER_SPREAD_OF_A_DISC * spread_rad).abs()
                    < MEAN_TOLERANCE_FRAC * spread_rad
            );
            assert!(angles_rad.iter().all(|&a| (0.0..=spread_rad).contains(&a)));
            assert!(mean_rad > last_mean_rad);
            last_mean_rad = mean_rad;
        }
        assert_eq!(get_rifle_spread_rad(&rifle, 0.0), rifle.noise_start_rad);
        assert_eq!(get_rifle_spread_rad(&rifle, 1.0), rifle.noise_max_rad);
    }

    #[test]
    fn sniper_speckle_is_exponential_up_to_1() {
        let sniper: SniperSettings = ServerSettings::Builtin().sniper;
        let mut noise: ShotNoise = ShotNoise::FromSeed(SEED);
        let speckles_frac: Vec<f64> = (0..SAMPLE_COUNT)
            .map(|_| noise.sample_sniper_speckle(&sniper))
            .collect();
        let mean_frac: f64 = sniper.beam_speckle_mean_frac;
        let capped_mean_frac: f64 = mean_frac * (1.0 - (-MAX_SPECKLE_FRAC / mean_frac).exp());
        assert!(
            (get_mean(&speckles_frac) - capped_mean_frac).abs() < MEAN_TOLERANCE_FRAC * mean_frac
        );
        assert!(
            speckles_frac
                .iter()
                .all(|&x| (0.0..=MAX_SPECKLE_FRAC).contains(&x))
        );
        assert!(speckles_frac.contains(&MAX_SPECKLE_FRAC));
        assert!(speckles_frac.iter().all(|&x| x == round_speckle_frac(x)));
        let mut off: SniperSettings = sniper.clone();
        off.beam_speckle_mean_frac = 0.0;
        assert!((0..REPEAT_COUNT).all(|_| noise.sample_sniper_speckle(&off) == 0.0));
    }

    #[test]
    fn rifle_speckle_grows_with_heat() {
        let rifle: RifleSettings = ServerSettings::Builtin().rifle;
        let speckles_frac: Vec<f64> = HEATS_FRAC
            .iter()
            .map(|&heat_frac| get_rifle_speckle_frac(&rifle, heat_frac))
            .collect();
        assert!(speckles_frac.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            speckles_frac[0],
            round_speckle_frac(rifle.beam_speckle_start_frac)
        );
        assert_eq!(
            speckles_frac[HEATS_FRAC.len() - 1],
            round_speckle_frac(rifle.beam_speckle_max_frac)
        );
    }

    #[test]
    fn speckles_round_to_network_steps() {
        let step_frac: f64 = 1.0 / f64::from(BEAM_SPECKLE_LEVEL_COUNT);
        assert_eq!(round_speckle_frac(0.0), 0.0);
        assert_eq!(round_speckle_frac(1.0), 1.0);
        assert_eq!(
            round_speckle_frac(step_frac * ROUNDS_DOWN_STEPS),
            step_frac * 2.0
        );
        assert_eq!(
            round_speckle_frac(step_frac * ROUNDS_UP_STEPS),
            step_frac * 3.0
        );
    }

    #[test]
    #[should_panic(expected = "speckle_frac must be in [0, 1]")]
    fn speckle_outside_its_range_fails() {
        round_speckle_frac(BEYOND_FULL_HEAT_FRAC);
    }

    #[test]
    #[should_panic(expected = "heat_frac must be in [0, 1]")]
    fn heat_outside_its_range_fails() {
        get_rifle_spread_rad(&ServerSettings::Builtin().rifle, BEYOND_FULL_HEAT_FRAC);
    }

    #[test]
    fn seeds_repeat_their_offsets() {
        let sniper: SniperSettings = ServerSettings::Builtin().sniper;
        let sample = |seed: u64| -> Vec<ShotOffset> {
            let mut noise: ShotNoise = ShotNoise::FromSeed(seed);
            (0..REPEAT_COUNT)
                .map(|_| noise.sample_sniper(&sniper))
                .collect()
        };
        assert_eq!(sample(SEED), sample(SEED));
        assert_ne!(sample(SEED), sample(SEED + 1));
    }

    #[test]
    fn no_noise_keeps_the_crosshair() {
        let mut sniper: SniperSettings = ServerSettings::Builtin().sniper;
        sniper.noise_mean_rad = 0.0;
        let mut noise: ShotNoise = ShotNoise::FromSeed(SEED);
        assert!((0..REPEAT_COUNT).all(|_| noise.sample_sniper(&sniper).angle_rad == 0.0));
        let camera: DMat4 = make_test_camera();
        assert_eq!(ShotOffset::NONE.aim(camera), camera);
    }

    #[test]
    fn aim_turns_the_crosshair_by_the_offset() {
        let camera: DMat4 = make_test_camera();
        let aimed: DMat4 = STRAY.aim(camera);
        let forward: DVec4 = -aimed.col(AXIS_Z);
        let ahead: f64 = minkowski_dot(forward, -camera.col(AXIS_Z));
        let right: f64 = minkowski_dot(forward, camera.col(AXIS_X));
        let up: f64 = minkowski_dot(forward, camera.col(AXIS_Y));
        assert!((right.hypot(up).atan2(ahead) - STRAY.angle_rad).abs() < ANGLE_TOLERANCE_RAD);
        assert!((up.atan2(right) - STRAY.direction_rad).abs() < DIRECTION_TOLERANCE_RAD);
        assert!((aimed.col(AXIS_W) - camera.col(AXIS_W)).length() < ANGLE_TOLERANCE_RAD);
    }
}
