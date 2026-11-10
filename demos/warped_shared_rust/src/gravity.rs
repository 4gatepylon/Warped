//! `gravity.rs` is the pull of black holes and wormhole mouths on a player on the ground, the same
//! in every level (W-144, gravity toward black holes and wormholes): each warp adds a speed toward
//! itself, along the ground, to the player's walk, so running toward a warp is faster, running
//! away slower, and a player standing still drifts toward it. Each warp's pull is full near its
//! horizon or throat and falls off to nothing at `GravitySettings::reach_m`; a mouth's fades to
//! nothing at its throat, so a player drifting in stops there rather than crossing back and forth.
//! All warps' pulls together are capped at `max_pull_per_walk_speed` of walking speed, below 1,
//! so no pull stops a player walking away. A level gives each warp's place in its own space
//! (`get_flat_pull_m_per_s` for flat levels, `get_hyperbolic_pull_m_per_s` for curved ones) and
//! adds the pull to its walk (`add_pull`).
//!
//! Interface: `FlatWarp`, `HyperbolicWarp`, `get_pull_frac`, `get_flat_pull_m_per_s`,
//! `get_hyperbolic_pull_m_per_s`, `add_pull`.
//!
//! Example: `let pull: DVec3 = get_flat_pull_m_per_s(&settings.gravity, 8.0, middle, up, &warps);`
//!
//! Must know:
//! - Limitations: the pull is a speed, not an acceleration, as walking is (no momentum), and it
//!   points straight at the warp, ignoring the warp's own bending of space.
//! - Gotchas: with no warp in reach the pull is exactly 0, and levels then move as before, bit for
//!   bit (their play traces away from warps stay the same).

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{AXIS_X, AXIS_Z, get_distance, minkowski_dot};
use crate::play_settings::GravitySettings;

/// `MIN_DIRECTION_LENGTH` is the shortest direction to a warp that has a direction; closer, the
/// player is at the warp's centre and it pulls nowhere.
const MIN_DIRECTION_LENGTH: f64 = 1e-12;

/// `FlatWarp` is a black hole or wormhole mouth in flat space: its centre, the radius of its
/// horizon or throat, and whether it is a mouth.
#[derive(Clone, Copy, Debug)]
pub struct FlatWarp {
    pub centre_m: DVec3,
    pub radius_m: f64,
    pub is_mouth: bool,
}

/// `HyperbolicWarp` is a black hole or wormhole mouth in H³ (the hyperboloid model,
/// `geometry.rs`): its centre, the radius of its horizon or throat, and whether it is a mouth.
#[derive(Clone, Copy, Debug)]
pub struct HyperbolicWarp {
    pub centre: DVec4,
    pub radius_m: f64,
    pub is_mouth: bool,
}

/// `get_pull_frac` gives one warp's pull, as a share of the strongest, on a player `distance_m`
/// from its centre: 1 from its horizon or throat (`radius_m`) out to `full_pull_m` beyond it,
/// then falling off as the square of the share of the way left to `reach_m` beyond it, and 0
/// past that. A mouth's pull also fades in a straight line to 0 at its throat, from `full_pull_m`
/// out.
///
/// Example: `let pull_frac: f64 = get_pull_frac(&settings.gravity, 6.0, 1.2, false);`
pub fn get_pull_frac(
    settings: &GravitySettings,
    distance_m: f64,
    radius_m: f64,
    is_mouth: bool,
) -> f64 {
    let outside_m: f64 = distance_m - radius_m;
    if outside_m >= settings.reach_m {
        return 0.0;
    }
    let falloff_frac: f64 = if outside_m <= settings.full_pull_m {
        1.0
    } else {
        ((settings.reach_m - outside_m) / (settings.reach_m - settings.full_pull_m)).powi(2)
    };
    let throat_frac: f64 = if is_mouth {
        (outside_m / settings.full_pull_m).clamp(0.0, 1.0)
    } else {
        1.0
    };
    falloff_frac * throat_frac
}

/// `sum_pulls` adds up the pulls of warps seen from a player, each a distance to its centre, its
/// radius, whether it is a mouth and the part of the unit direction to it along the ground, and
/// caps their sum at the strongest pull, `max_pull_per_walk_speed` times `walk_m_per_s`.
fn sum_pulls(
    settings: &GravitySettings,
    walk_m_per_s: f64,
    sightings: impl IntoIterator<Item = (f64, f64, bool, DVec3)>,
) -> DVec3 {
    let max_m_per_s: f64 = settings.max_pull_per_walk_speed * walk_m_per_s;
    let mut pull_m_per_s: DVec3 = DVec3::ZERO;
    for (distance_m, radius_m, is_mouth, toward) in sightings {
        let pull_frac: f64 = get_pull_frac(settings, distance_m, radius_m, is_mouth);
        if pull_frac > 0.0 {
            pull_m_per_s += toward * (pull_frac * max_m_per_s);
        }
    }
    let length_m_per_s: f64 = pull_m_per_s.length();
    if length_m_per_s > max_m_per_s {
        pull_m_per_s *= max_m_per_s / length_m_per_s;
    }
    pull_m_per_s
}

/// `get_flat_pull_m_per_s` gives the pull of `warps` on a player at `at_m` in flat space, whose
/// up is the unit vector `up`, as a velocity along the ground (across `up`), for a level whose
/// walking speed is `walk_m_per_s`. A warp above or below the player pulls by the part of its
/// direction along the ground, so one straight overhead pulls nowhere.
///
/// Example: `let pull: DVec3 = get_flat_pull_m_per_s(&settings.gravity, 8.0, middle, DVec3::Y, &warps);`
pub fn get_flat_pull_m_per_s(
    settings: &GravitySettings,
    walk_m_per_s: f64,
    at_m: DVec3,
    up: DVec3,
    warps: &[FlatWarp],
) -> DVec3 {
    sum_pulls(
        settings,
        walk_m_per_s,
        warps.iter().map(|warp| {
            let offset_m: DVec3 = warp.centre_m - at_m;
            let distance_m: f64 = offset_m.length();
            let toward: DVec3 = if distance_m > MIN_DIRECTION_LENGTH {
                let along: DVec3 = offset_m / distance_m;
                along - up * along.dot(up)
            } else {
                DVec3::ZERO
            };
            (distance_m, warp.radius_m, warp.is_mouth, toward)
        }),
    )
}

/// `get_hyperbolic_pull_m_per_s` gives the pull of `warps` on a player whose eye is the point
/// `eye` of H³ of curvature radius `r_m`, and whose body's frame is `body` (columns right, up,
/// back, its foot; `eye` straight above it), as speeds along the body's right and back axes, for
/// a level whose walking speed is `walk_m_per_s`. Each warp pulls along the geodesic from the eye
/// to its centre, by that direction's part across the body's up.
///
/// Example: `let [right_m_per_s, back_m_per_s] = get_hyperbolic_pull_m_per_s(&gravity, 10.0, body, eye, 26.7, &warps);`
pub fn get_hyperbolic_pull_m_per_s(
    settings: &GravitySettings,
    walk_m_per_s: f64,
    body: DMat4,
    eye: DVec4,
    r_m: f64,
    warps: &[HyperbolicWarp],
) -> [f64; 2] {
    let pull_m_per_s: DVec3 = sum_pulls(
        settings,
        walk_m_per_s,
        warps.iter().map(|warp| {
            let distance: f64 = get_distance(eye, warp.centre);
            let tangent: DVec4 = warp.centre + minkowski_dot(warp.centre, eye) * eye;
            let length: f64 = distance.sinh();
            let toward: DVec3 = if length > MIN_DIRECTION_LENGTH {
                DVec3::new(
                    minkowski_dot(tangent, body.col(AXIS_X)) / length,
                    0.0,
                    minkowski_dot(tangent, body.col(AXIS_Z)) / length,
                )
            } else {
                DVec3::ZERO
            };
            (distance * r_m, warp.radius_m, warp.is_mouth, toward)
        }),
    );
    [pull_m_per_s.x, pull_m_per_s.z]
}

/// `add_pull` adds a pull (`pull_m_per_s`, from `get_flat_pull_m_per_s` or
/// `get_hyperbolic_pull_m_per_s`) to a walk along the unit direction `walk_dir` (or [0, 0]
/// standing still) at `walk_m_per_s`, both in the same two axes along the ground, and gives the
/// unit direction and speed of the move, or `None` if the player doesn't move. With no pull it
/// gives the walk itself, bit for bit.
///
/// Example: `if let Some((dir, speed_m_per_s)) = add_pull([0.0, -1.0], 10.0, pull_m_per_s) { ... }`
pub fn add_pull(
    walk_dir: [f64; 2],
    walk_m_per_s: f64,
    pull_m_per_s: [f64; 2],
) -> Option<([f64; 2], f64)> {
    let is_walking: bool = walk_dir != [0.0, 0.0];
    if pull_m_per_s == [0.0, 0.0] {
        return is_walking.then_some((walk_dir, walk_m_per_s));
    }
    let x_m_per_s: f64 = walk_dir[0] * walk_m_per_s + pull_m_per_s[0];
    let z_m_per_s: f64 = walk_dir[1] * walk_m_per_s + pull_m_per_s[1];
    let speed_m_per_s: f64 = x_m_per_s.hypot(z_m_per_s);
    (speed_m_per_s > 0.0).then(|| {
        (
            [x_m_per_s / speed_m_per_s, z_m_per_s / speed_m_per_s],
            speed_m_per_s,
        )
    })
}

#[cfg(test)]
mod tests {
    //! Tests of the warps' pull.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Distance outside the horizon or throat | within `full_pull_m` (full), between (falling off, less farther out), at and past `reach_m` (0) | `pull_falls_off_with_distance` |
    //! | Warp kind | black hole (full to the horizon), mouth (fading to 0 at the throat) | `pull_falls_off_with_distance` |
    //! | Warps in reach | none (exactly 0), one (toward it), several (added, capped at the strongest) | `pulls_add_up_and_are_capped`, `flat_pull_points_along_the_ground` |
    //! | Warp's height | level with the player (full), above (its part along the ground), straight overhead (0) | `flat_pull_points_along_the_ground` |
    //! | Space | flat, hyperbolic (toward the warp along the body's right and back axes, the same strength at the same distance) | `flat_pull_points_along_the_ground`, `hyperbolic_pull_points_at_the_warp` |
    //! | Walk and pull | standing, no pull (no move); walking, no pull (the walk, bit for bit); toward (faster); away (slower, never stopped); standing, pulled (drifts) | `walking_with_the_pull` |
    //! | Settings | built in (valid), max pull 1 or more, full pull at or past the reach, a file without the section or with one key (built-in values) | `settings_are_checked_and_default` |

    use super::*;
    use crate::geometry::{AXIS_W, AXIS_Y, make_boost, make_rotation};

    const WALK_M_PER_S: f64 = 10.0;
    const R_M: f64 = 26.7;
    const TOLERANCE: f64 = 1e-9;

    #[test]
    fn pull_falls_off_with_distance() {
        let gravity: GravitySettings = GravitySettings::default();
        let radius_m: f64 = 1.2;
        let full_m: f64 = radius_m + gravity.full_pull_m;
        let reach_m: f64 = radius_m + gravity.reach_m;
        assert_eq!(get_pull_frac(&gravity, radius_m, radius_m, false), 1.0);
        assert_eq!(get_pull_frac(&gravity, full_m, radius_m, false), 1.0);
        let middle_frac: f64 = get_pull_frac(&gravity, (full_m + reach_m) / 2.0, radius_m, false);
        assert!((middle_frac - 0.25).abs() < TOLERANCE, "{middle_frac}");
        assert!(get_pull_frac(&gravity, reach_m - 1.0, radius_m, false) < middle_frac);
        assert_eq!(get_pull_frac(&gravity, reach_m, radius_m, false), 0.0);
        assert_eq!(get_pull_frac(&gravity, reach_m * 3.0, radius_m, false), 0.0);
        assert_eq!(get_pull_frac(&gravity, radius_m, radius_m, true), 0.0);
        let near_frac: f64 = get_pull_frac(&gravity, radius_m + 0.5, radius_m, true);
        assert!((near_frac - 0.5 / gravity.full_pull_m).abs() < TOLERANCE);
        assert_eq!(get_pull_frac(&gravity, full_m, radius_m, true), 1.0);
    }

    #[test]
    fn flat_pull_points_along_the_ground() {
        let gravity: GravitySettings = GravitySettings::default();
        let max_m_per_s: f64 = gravity.max_pull_per_walk_speed * WALK_M_PER_S;
        let hole = |centre_m: DVec3| FlatWarp {
            centre_m,
            radius_m: 1.0,
            is_mouth: false,
        };
        let level: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[hole(DVec3::X * 2.0)],
        );
        assert!(
            (level - DVec3::X * max_m_per_s).length() < TOLERANCE,
            "{level}"
        );
        let above: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[hole(DVec3::new(2.0, 2.0, 0.0))],
        );
        assert!(
            above.y == 0.0 && above.x > 0.0 && above.x < max_m_per_s,
            "{above}"
        );
        let overhead: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[hole(DVec3::Y * 2.0)],
        );
        assert_eq!(overhead, DVec3::ZERO);
        let far: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[hole(DVec3::Z * 40.0)],
        );
        assert_eq!(far, DVec3::ZERO);
    }

    #[test]
    fn pulls_add_up_and_are_capped() {
        let gravity: GravitySettings = GravitySettings::default();
        let max_m_per_s: f64 = gravity.max_pull_per_walk_speed * WALK_M_PER_S;
        let mouth = |centre_m: DVec3| FlatWarp {
            centre_m,
            radius_m: 1.0,
            is_mouth: true,
        };
        let one: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[mouth(DVec3::X * 12.0)],
        );
        let two: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[mouth(DVec3::X * 12.0), mouth(DVec3::Z * 12.0)],
        );
        assert!(one.x > 0.0 && one.x < max_m_per_s / 2.0);
        assert!(
            (two - DVec3::new(one.x, 0.0, one.x)).length() < TOLERANCE,
            "{two}"
        );
        let ring: Vec<FlatWarp> = (0..3)
            .map(|k| mouth(DVec3::new(4.0 + f64::from(k), 0.0, f64::from(k) - 1.0)))
            .collect();
        let capped: DVec3 =
            get_flat_pull_m_per_s(&gravity, WALK_M_PER_S, DVec3::ZERO, DVec3::Y, &ring);
        assert!(
            (capped.length() - max_m_per_s).abs() < TOLERANCE,
            "{capped}"
        );
        assert_eq!(
            get_flat_pull_m_per_s(&gravity, WALK_M_PER_S, DVec3::ZERO, DVec3::Y, &[]),
            DVec3::ZERO
        );
    }

    #[test]
    fn hyperbolic_pull_points_at_the_warp() {
        let gravity: GravitySettings = GravitySettings::default();
        let eye_height_m: f64 = 1.6;
        let turn_rad: f64 = 0.7;
        let body: DMat4 = make_rotation(AXIS_X, AXIS_Z, turn_rad);
        let eye: DVec4 = (body * make_boost(AXIS_Y, eye_height_m / R_M)).col(AXIS_W);
        let ahead_m: f64 = 8.0;
        let centre: DVec4 = (make_boost(AXIS_Y, eye_height_m / R_M)
            * make_boost(AXIS_X, ahead_m / R_M))
        .col(AXIS_W);
        let hole: HyperbolicWarp = HyperbolicWarp {
            centre,
            radius_m: 1.2,
            is_mouth: false,
        };
        let [right_m_per_s, back_m_per_s] =
            get_hyperbolic_pull_m_per_s(&gravity, WALK_M_PER_S, body, eye, R_M, &[hole]);
        let flat: DVec3 = get_flat_pull_m_per_s(
            &gravity,
            WALK_M_PER_S,
            DVec3::ZERO,
            DVec3::Y,
            &[FlatWarp {
                centre_m: DVec3::X * ahead_m,
                radius_m: 1.2,
                is_mouth: false,
            }],
        );
        let expected: [f64; 2] = [flat.x * turn_rad.cos(), flat.x * -turn_rad.sin()];
        assert!(
            (right_m_per_s - expected[0]).abs() < 1e-6 && (back_m_per_s - expected[1]).abs() < 1e-6,
            "{right_m_per_s} {back_m_per_s} against {expected:?}"
        );
        let far: HyperbolicWarp = HyperbolicWarp {
            centre: make_boost(AXIS_Z, 30.0 / R_M).col(AXIS_W),
            ..hole
        };
        assert_eq!(
            get_hyperbolic_pull_m_per_s(&gravity, WALK_M_PER_S, body, eye, R_M, &[far]),
            [0.0, 0.0]
        );
    }

    #[test]
    fn walking_with_the_pull() {
        let forward: [f64; 2] = [0.0, -1.0];
        assert_eq!(add_pull([0.0, 0.0], WALK_M_PER_S, [0.0, 0.0]), None);
        let walk_dir: [f64; 2] = [0.6, -0.8];
        assert_eq!(
            add_pull(walk_dir, WALK_M_PER_S, [0.0, 0.0]),
            Some((walk_dir, WALK_M_PER_S))
        );
        let pull_m_per_s: f64 = 0.75 * WALK_M_PER_S;
        let toward = add_pull(forward, WALK_M_PER_S, [0.0, -pull_m_per_s]).expect("moves");
        assert_eq!(toward, (forward, WALK_M_PER_S + pull_m_per_s));
        let away = add_pull(forward, WALK_M_PER_S, [0.0, pull_m_per_s]).expect("moves");
        assert_eq!(away, (forward, WALK_M_PER_S - pull_m_per_s));
        let drift = add_pull([0.0, 0.0], WALK_M_PER_S, [pull_m_per_s, 0.0]).expect("drifts");
        assert_eq!(drift, ([1.0, 0.0], pull_m_per_s));
    }

    #[test]
    fn settings_are_checked_and_default() {
        let gravity: GravitySettings = GravitySettings::default();
        assert_eq!(gravity.check(), Ok(()));
        let too_strong: GravitySettings = GravitySettings {
            max_pull_per_walk_speed: 1.0,
            ..gravity.clone()
        };
        assert!(too_strong.check().is_err());
        let full_past_reach: GravitySettings = GravitySettings {
            full_pull_m: gravity.reach_m,
            ..gravity.clone()
        };
        assert!(full_past_reach.check().is_err());
        let off: GravitySettings = GravitySettings {
            max_pull_per_walk_speed: 0.0,
            ..gravity.clone()
        };
        assert_eq!(off.check(), Ok(()));
        let one_key: GravitySettings = serde_yaml::from_str("reach_m: 12.0").expect("reads");
        assert_eq!(
            one_key,
            GravitySettings {
                reach_m: 12.0,
                ..gravity.clone()
            }
        );
        assert!(serde_yaml::from_str::<GravitySettings>("reach_mm: 12.0").is_err());
    }
}
