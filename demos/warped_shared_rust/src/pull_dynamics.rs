//! `pull_dynamics.rs` is the pull of black holes and wormhole mouths as an acceleration, and the
//! sprint that speeds up and slows down (W-202, pull toward wormholes and black holes in the game
//! settings), the same in every level and the same as the tuning page's `pull_dynamics.js`
//! (`demos/warp_pull_js/`, equations in its `EQUATIONS.md`): inside `gravity.radius_m` of a warp's
//! centre a player is pulled toward it; the pulls build up a drift, which nothing brakes inside a
//! radius and which is braked outside every radius; the legs' speed rises to the sprint's top
//! speed while sprinting and falls back when not. A level finds each warp's distance and
//! direction in its own space and calls `get_pull_m_per_s_per_s`, `step_drift` and
//! `ramp_legs_speed`; `step_flat_player` is the whole step on a flat floor with one wormhole, the
//! page's `step_player`, which the tests check against a recording the page's code made.
//!
//! Interface: `get_pull_m_per_s_per_s`, `get_flat_pull`, `get_hyperbolic_pull`, `turn_drift`,
//! `step_drift`, `ramp_legs_speed`, `FLAT_STEP_S`,
//! `FlatPlayer`, `FlatInput`, `FlatLevel`, `step_flat_player`.
//!
//! Example: `let drift: DVec3 = step_drift(drift, pull, gravity.brake_m_per_s_per_s, dt_s);`
//!
//! Must know:
//! - Gotchas:
//!   - A warp whose throat or horizon is wider than `gravity.radius_m` doesn't pull: the radius
//!     is from the warp's centre.
//!   - `step_flat_player` does its sums in `pull_dynamics.js`'s order, so the two agree to the
//!     last bits; change them together.

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{AXIS_X, AXIS_Z, get_distance, minkowski_dot};
use crate::gravity::{FlatWarp, HyperbolicWarp};
use crate::play_settings::{GravitySettings, MovementSettings};

/// `FLAT_STEP_S` is the time step of `step_flat_player`, the page's `DT_S`.
pub const FLAT_STEP_S: f64 = 1.0 / 120.0;
const MIN_LENGTH: f64 = 1e-12;

/// `get_pull_m_per_s_per_s` gives how hard a warp pulls a player `distance_m` from its centre,
/// where its throat or horizon has `warp_radius_m`: `None` beyond `settings.radius_m` (out of
/// reach), 0 inside the throat or horizon, and between them `settings.acceleration_m_per_s_per_s`
/// times the profile's share there.
///
/// Example: `if let Some(pull) = get_pull_m_per_s_per_s(&settings.gravity, 3.0, 1.6) { ... }`
pub fn get_pull_m_per_s_per_s(
    settings: &GravitySettings,
    distance_m: f64,
    warp_radius_m: f64,
) -> Option<f64> {
    if distance_m > settings.radius_m {
        return None;
    }
    if distance_m <= warp_radius_m || distance_m < MIN_LENGTH {
        return Some(0.0);
    }
    let edge_frac: f64 = (distance_m - warp_radius_m) / (settings.radius_m - warp_radius_m);
    Some(settings.acceleration_m_per_s_per_s * settings.profile.get_frac(edge_frac))
}

/// `get_flat_pull` gives the pull of `warps` on a player at `at_m` in flat space, whose up is the
/// unit vector `up`, as an acceleration along the ground (across `up`), or `None` out of every
/// warp's reach. A warp above or below the player pulls by the part of its direction along the
/// ground.
///
/// Example: `let pull: Option<DVec3> = get_flat_pull(&settings.gravity, middle, DVec3::Y, &warps);`
pub fn get_flat_pull(
    settings: &GravitySettings,
    at_m: DVec3,
    up: DVec3,
    warps: &[FlatWarp],
) -> Option<DVec3> {
    let mut pull: Option<DVec3> = None;
    for warp in warps {
        let offset_m: DVec3 = warp.centre_m - at_m;
        let distance_m: f64 = offset_m.length();
        let Some(strength) = get_pull_m_per_s_per_s(settings, distance_m, warp.radius_m) else {
            continue;
        };
        let toward: DVec3 = if strength > 0.0 {
            let along: DVec3 = offset_m / distance_m;
            (along - up * along.dot(up)) * strength
        } else {
            DVec3::ZERO
        };
        pull = Some(pull.unwrap_or(DVec3::ZERO) + toward);
    }
    pull
}

/// `get_hyperbolic_pull` gives the pull of `warps` on a player whose eye is the point `eye` of H³
/// of curvature radius `r_m`, and whose body's frame is `body` (columns right, up, back, its
/// foot), as an acceleration along the body's right (x) and back (z) axes, or `None` out of every
/// warp's reach. Each warp pulls along the geodesic from the eye to its centre.
///
/// Example: `let pull: Option<DVec3> = get_hyperbolic_pull(&gravity, body, eye, 26.7, &warps);`
pub fn get_hyperbolic_pull(
    settings: &GravitySettings,
    body: DMat4,
    eye: DVec4,
    r_m: f64,
    warps: &[HyperbolicWarp],
) -> Option<DVec3> {
    let mut pull: Option<DVec3> = None;
    for warp in warps {
        let distance: f64 = get_distance(eye, warp.centre);
        let Some(strength) = get_pull_m_per_s_per_s(settings, distance * r_m, warp.radius_m) else {
            continue;
        };
        let length: f64 = distance.sinh();
        let toward: DVec3 = if strength > 0.0 && length > MIN_LENGTH {
            let tangent: DVec4 = warp.centre + minkowski_dot(warp.centre, eye) * eye;
            DVec3::new(
                minkowski_dot(tangent, body.col(AXIS_X)) / length * strength,
                0.0,
                minkowski_dot(tangent, body.col(AXIS_Z)) / length * strength,
            )
        } else {
            DVec3::ZERO
        };
        pull = Some(pull.unwrap_or(DVec3::ZERO) + toward);
    }
    pull
}

/// `turn_drift` gives a drift kept in a body's right (x) and back (z) axes after the body has
/// turned by `turn` (the rotation the body's frame was multiplied by on the right, as looking
/// around does), so that the drift keeps its direction in the world.
///
/// Example: `self.drift_m_per_s = turn_drift(self.drift_m_per_s, make_rotation(AXIS_X, AXIS_Z, yaw_rad));`
pub fn turn_drift(drift_m_per_s: DVec3, turn: DMat4) -> DVec3 {
    (turn.inverse() * drift_m_per_s.extend(0.0)).truncate()
}

/// `step_drift` moves a player's drift on by `dt_s`: with a pull (`Some`, the sum of the pulls of
/// the warps in reach, even if 0) the pull is added to it; out of every warp's reach (`None`) its
/// size falls by `brake_m_per_s_per_s`, to 0 at most.
///
/// Example: `let drift: DVec3 = step_drift(drift, Some(pull), 15.0, 1.0 / 60.0);`
pub fn step_drift(
    drift_m_per_s: DVec3,
    pull_m_per_s_per_s: Option<DVec3>,
    brake_m_per_s_per_s: f64,
    dt_s: f64,
) -> DVec3 {
    match pull_m_per_s_per_s {
        Some(pull) => drift_m_per_s + pull * dt_s,
        None => {
            let speed_m_per_s: f64 = drift_m_per_s.length();
            let braked_m_per_s: f64 = (speed_m_per_s - brake_m_per_s_per_s * dt_s).max(0.0);
            if speed_m_per_s > MIN_LENGTH {
                drift_m_per_s * (braked_m_per_s / speed_m_per_s)
            } else {
                DVec3::ZERO
            }
        }
    }
}

/// `ramp_legs_speed` moves the speed the legs give on by `dt_s`: toward `sprint_m_per_s` at
/// `movement.sprint_up_m_per_s_per_s` while sprinting, and back toward `walk_m_per_s` at
/// `movement.sprint_down_m_per_s_per_s` when not, never past either.
///
/// Example: `self.legs_m_per_s = ramp_legs_speed(self.legs_m_per_s, self.is_sprinting, 8.0, 15.5, movement, dt_s);`
pub fn ramp_legs_speed(
    legs_m_per_s: f64,
    is_sprinting: bool,
    walk_m_per_s: f64,
    sprint_m_per_s: f64,
    movement: &MovementSettings,
    dt_s: f64,
) -> f64 {
    let target_m_per_s: f64 = if is_sprinting {
        sprint_m_per_s
    } else {
        walk_m_per_s
    };
    if legs_m_per_s < target_m_per_s {
        (legs_m_per_s + movement.sprint_up_m_per_s_per_s * dt_s).min(target_m_per_s)
    } else {
        (legs_m_per_s - movement.sprint_down_m_per_s_per_s * dt_s).max(target_m_per_s)
    }
}

/// `FlatPlayer` is a player on a flat floor seen from above: where it is, the speed its legs
/// give, its drift, its velocity in the last step, its sprint energy (0 to 1), whether it
/// sprints, and how many times it has gone through a mouth.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlatPlayer {
    pub at_m: [f64; 2],
    pub legs_m_per_s: f64,
    pub drift_m_per_s: [f64; 2],
    pub velocity_m_per_s: [f64; 2],
    pub energy_frac: f64,
    pub is_sprinting: bool,
    pub pass_count: u32,
}

/// `FlatInput` is one step's keys: the move keys' direction (each part -1, 0 or 1) and Shift.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlatInput {
    pub dir: [f64; 2],
    pub is_sprint_held: bool,
}

/// `FlatLevel` is the flat floor's rules: the pull, the sprint's ramps, the walking and sprint
/// speeds, how long the sprint energy lasts and takes to refill, and one wormhole's two mouths
/// with their throat's radius.
#[derive(Clone, Copy)]
pub struct FlatLevel<'a> {
    pub gravity: &'a GravitySettings,
    pub movement: &'a MovementSettings,
    pub walk_m_per_s: f64,
    pub sprint_m_per_s: f64,
    pub energy_drain_s: f64,
    pub energy_refill_s: f64,
    pub mouths_m: [[f64; 2]; 2],
    pub throat_radius_m: f64,
}

/// `step_flat_player` moves `player` on by `FLAT_STEP_S` under `input` in `level`, as the page's
/// `step_player` does: the energy and the legs' speed, the mouths' pull into the drift, the move,
/// and through a mouth's throat to the opposite point of the other with the velocity kept.
///
/// Example: `step_flat_player(&mut player, &FlatInput { dir: [1.0, 0.0], is_sprint_held: false }, &level);`
pub fn step_flat_player(player: &mut FlatPlayer, input: &FlatInput, level: &FlatLevel) {
    let length: f64 = input.dir[0].hypot(input.dir[1]);
    let is_moving: bool = length > 0.0;
    let h: [f64; 2] = if is_moving {
        [input.dir[0] / length, input.dir[1] / length]
    } else {
        [0.0, 0.0]
    };
    let wants_to_sprint: bool = input.is_sprint_held && is_moving;
    player.is_sprinting = wants_to_sprint && player.energy_frac > 0.0;
    let energy_per_s: f64 = if wants_to_sprint {
        -1.0 / level.energy_drain_s
    } else {
        1.0 / level.energy_refill_s
    };
    player.energy_frac = (player.energy_frac + energy_per_s * FLAT_STEP_S).clamp(0.0, 1.0);
    player.legs_m_per_s = ramp_legs_speed(
        player.legs_m_per_s,
        player.is_sprinting,
        level.walk_m_per_s,
        level.sprint_m_per_s,
        level.movement,
        FLAT_STEP_S,
    );

    let mut pull: Option<DVec3> = None;
    for mouth in level.mouths_m {
        let (dx, dy) = (mouth[0] - player.at_m[0], mouth[1] - player.at_m[1]);
        let distance_m: f64 = dx.hypot(dy);
        let Some(strength) =
            get_pull_m_per_s_per_s(level.gravity, distance_m, level.throat_radius_m)
        else {
            continue;
        };
        let toward: DVec3 = if strength > 0.0 {
            DVec3::new(dx / distance_m * strength, dy / distance_m * strength, 0.0)
        } else {
            DVec3::ZERO
        };
        pull = Some(pull.unwrap_or(DVec3::ZERO) + toward);
    }
    let drift: DVec3 = step_drift(
        DVec3::new(player.drift_m_per_s[0], player.drift_m_per_s[1], 0.0),
        pull,
        level.gravity.brake_m_per_s_per_s,
        FLAT_STEP_S,
    );
    player.drift_m_per_s = [drift.x, drift.y];

    player.velocity_m_per_s = [
        h[0] * player.legs_m_per_s + drift.x,
        h[1] * player.legs_m_per_s + drift.y,
    ];
    player.at_m[0] += player.velocity_m_per_s[0] * FLAT_STEP_S;
    player.at_m[1] += player.velocity_m_per_s[1] * FLAT_STEP_S;

    for i in 0..level.mouths_m.len() {
        let (ox, oy) = (
            player.at_m[0] - level.mouths_m[i][0],
            player.at_m[1] - level.mouths_m[i][1],
        );
        let is_inside: bool = ox.hypot(oy) < level.throat_radius_m;
        let is_going_in: bool =
            ox * player.velocity_m_per_s[0] + oy * player.velocity_m_per_s[1] < 0.0;
        if is_inside && is_going_in {
            let other: [f64; 2] = level.mouths_m[1 - i];
            player.at_m = [other[0] - ox, other[1] - oy];
            player.pass_count += 1;
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the pull, the drift, the sprint's ramps and the flat step.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Distance from a warp | beyond the radius (out of reach), between throat and radius (the profile's share of the strength), inside the throat (0, in reach), a warp wider than the radius (0 or out of reach) | `the_pull_is_inside_the_radius_only` |
    //! | Profile | constant, stronger near the mouth, harmonic | `the_pull_is_inside_the_radius_only` |
    //! | Drift | pulled (the pull added), in reach with no pull (kept), out of reach (braked, to 0 and no further), already 0 | `drift_builds_in_reach_and_is_braked_outside` |
    //! | Pull in a space | flat: none in reach, one (toward it along the ground, its strength), one overhead (in reach, no pull), two (added); hyperbolic: the same strength and direction in the body's axes at the same distance, none in reach | `pulls_point_at_the_warps_along_the_ground` |
    //! | Body turned | by a quarter turn (the drift's parts swap so its direction in the world stays), then back | `a_turned_body_keeps_its_drifts_direction` |
    //! | Legs' speed | sprinting from walking speed (rises, stops at the top), not sprinting from the top (falls, stops at walking speed), below walking speed (rises to it) | `the_legs_speed_ramps_between_walking_and_sprinting` |
    //! | The page's recording (scripted keys through `pull_dynamics.js`: a swing through both mouths, sprinting in and out, walking away) | every sampled place, the times through a mouth, the energy and the legs' speed at the end | `the_flat_step_follows_the_pages_recording` |

    use serde_json::Value;

    use super::*;
    use crate::play_settings::PullProfile;

    const RECORDING_JSON: &str = include_str!("pull_dynamics_recording.json");
    const MAX_STRAY_M: f64 = 1e-6;
    const THROAT_M: f64 = 1.6;

    /// `make_movement` gives movement settings with the sprint ramps `up` and `down`.
    fn make_movement(up: f64, down: f64) -> MovementSettings {
        MovementSettings {
            walk_speed_per_map_speed: 1.0,
            sprint_speed_per_map_speed: 1.0,
            sprint_up_m_per_s_per_s: up,
            sprint_down_m_per_s_per_s: down,
            jump_height_m: 1.25,
            jump_time_per_map_jump_time: 1.0,
            crouch_speed_frac: 0.5,
            crouch_eye_height_frac: 0.6,
            crouch_s: 0.15,
            shoot_after_moving_s: 0.25,
        }
    }

    #[test]
    fn the_pull_is_inside_the_radius_only() {
        let mut gravity: GravitySettings = GravitySettings::default();
        let strength: f64 = gravity.acceleration_m_per_s_per_s;
        assert_eq!(get_pull_m_per_s_per_s(&gravity, 5.01, THROAT_M), None);
        assert_eq!(
            get_pull_m_per_s_per_s(&gravity, 5.0, THROAT_M),
            Some(strength)
        );
        assert_eq!(
            get_pull_m_per_s_per_s(&gravity, 2.0, THROAT_M),
            Some(strength)
        );
        assert_eq!(get_pull_m_per_s_per_s(&gravity, 1.6, THROAT_M), Some(0.0));
        assert_eq!(get_pull_m_per_s_per_s(&gravity, 0.0, THROAT_M), Some(0.0));
        assert_eq!(get_pull_m_per_s_per_s(&gravity, 4.0, 6.0), Some(0.0));
        assert_eq!(get_pull_m_per_s_per_s(&gravity, 5.5, 6.0), None);
        let quarter_m: f64 = THROAT_M + (gravity.radius_m - THROAT_M) * 0.25;
        gravity.profile = PullProfile::Harmonic;
        let harmonic: f64 = get_pull_m_per_s_per_s(&gravity, quarter_m, THROAT_M).unwrap();
        assert!((harmonic - 0.25 * strength).abs() < 1e-12);
        gravity.profile = PullProfile::StrongerNearMouth;
        let near: f64 = get_pull_m_per_s_per_s(&gravity, quarter_m, THROAT_M).unwrap();
        assert!((near - 0.75 * strength).abs() < 1e-12);
    }

    #[test]
    fn pulls_point_at_the_warps_along_the_ground() {
        use crate::geometry::{AXIS_W, AXIS_Y, make_boost};
        let gravity: GravitySettings = GravitySettings::default();
        let strength: f64 = gravity.acceleration_m_per_s_per_s;
        let warp = |x: f64, y: f64| FlatWarp {
            centre_m: DVec3::new(x, y, 0.0),
            radius_m: THROAT_M,
            is_mouth: true,
        };
        let pull_of = |warps: &[FlatWarp]| get_flat_pull(&gravity, DVec3::ZERO, DVec3::Y, warps);
        assert_eq!(pull_of(&[warp(6.0, 0.0)]), None);
        let one: DVec3 = pull_of(&[warp(3.0, 0.0)]).expect("in reach");
        assert!((one - DVec3::X * strength).length() < 1e-12);
        let overhead: DVec3 = pull_of(&[warp(0.0, 3.0)]).expect("in reach");
        assert!(overhead.length() < 1e-12);
        let two: DVec3 = pull_of(&[warp(3.0, 0.0), warp(-4.0, 0.0), warp(9.0, 0.0)]).unwrap();
        assert!(two.length() < 1e-12);
        const CURVATURE_RADIUS_M: f64 = 26.7;
        let body: DMat4 = DMat4::IDENTITY;
        let eye: DVec4 = (body * make_boost(AXIS_Y, 1.6 / CURVATURE_RADIUS_M)).col(AXIS_W);
        let ahead = |distance_m: f64| HyperbolicWarp {
            centre: (make_boost(AXIS_Y, 1.6 / CURVATURE_RADIUS_M)
                * make_boost(AXIS_Z, -distance_m / CURVATURE_RADIUS_M))
            .col(AXIS_W),
            radius_m: THROAT_M,
            is_mouth: true,
        };
        let curved = |distance_m: f64| {
            get_hyperbolic_pull(
                &gravity,
                body,
                eye,
                CURVATURE_RADIUS_M,
                &[ahead(distance_m)],
            )
        };
        assert_eq!(curved(6.0), None);
        let pull: DVec3 = curved(3.0).expect("in reach");
        assert!((pull.length() - strength).abs() < 0.05 * strength, "{pull}");
        assert!(
            pull.z < 0.0 && pull.x.abs() < 1e-9 && pull.y == 0.0,
            "{pull}"
        );
    }

    #[test]
    fn a_turned_body_keeps_its_drifts_direction() {
        use crate::geometry::make_rotation;
        let drift: DVec3 = DVec3::new(2.0, 0.0, -5.0);
        let quarter: DMat4 = make_rotation(AXIS_X, AXIS_Z, std::f64::consts::FRAC_PI_2);
        let turned: DVec3 = turn_drift(drift, quarter);
        assert!((turned.length() - drift.length()).abs() < 1e-12 && turned.y == 0.0);
        assert!((quarter * turned.extend(0.0) - drift.extend(0.0)).length() < 1e-12);
        assert!((turn_drift(turned, quarter.inverse()) - drift).length() < 1e-12);
        assert!(turned.x.abs() > 4.9 || turned.z.abs() > 4.9);
    }

    #[test]
    fn drift_builds_in_reach_and_is_braked_outside() {
        let drift: DVec3 = DVec3::new(3.0, 0.0, -4.0);
        let pull: DVec3 = DVec3::new(0.0, 0.0, 10.0);
        assert_eq!(
            step_drift(drift, Some(pull), 15.0, 0.1),
            DVec3::new(3.0, 0.0, -3.0)
        );
        assert_eq!(step_drift(drift, Some(DVec3::ZERO), 15.0, 0.1), drift);
        let braked: DVec3 = step_drift(drift, None, 15.0, 0.1);
        assert!((braked.length() - 3.5).abs() < 1e-12);
        assert!((braked.normalize() - drift.normalize()).length() < 1e-12);
        assert_eq!(step_drift(drift, None, 15.0, 1.0), DVec3::ZERO);
        assert_eq!(step_drift(DVec3::ZERO, None, 15.0, 0.1), DVec3::ZERO);
    }

    #[test]
    fn the_legs_speed_ramps_between_walking_and_sprinting() {
        let movement: MovementSettings = make_movement(10.0, 20.0);
        let ramp = |legs: f64, is_sprinting: bool, dt_s: f64| {
            ramp_legs_speed(legs, is_sprinting, 8.0, 15.5, &movement, dt_s)
        };
        assert_eq!(ramp(8.0, true, 0.1), 9.0);
        assert_eq!(ramp(15.0, true, 0.1), 15.5);
        assert_eq!(ramp(15.5, false, 0.1), 13.5);
        assert_eq!(ramp(9.0, false, 0.1), 8.0);
        assert_eq!(ramp(8.0, false, 0.1), 8.0);
        assert_eq!(ramp(5.0, false, 0.1), 6.0);
    }

    #[test]
    fn the_flat_step_follows_the_pages_recording() {
        let recording: Value = serde_json::from_str(RECORDING_JSON).expect("the recording");
        let number = |value: &Value| value.as_f64().expect("a number");
        let settings: &Value = &recording["settings"];
        assert_eq!(settings["pull_profile"], "constant");
        assert!((number(&recording["dt_s"]) - FLAT_STEP_S).abs() < 1e-15);
        let gravity: GravitySettings = GravitySettings {
            radius_m: number(&settings["pull_radius_m"]),
            acceleration_m_per_s_per_s: number(&settings["pull_m_per_s_per_s"]),
            profile: PullProfile::Constant,
            brake_m_per_s_per_s: number(&settings["drift_brake_m_per_s_per_s"]),
            ..GravitySettings::default()
        };
        let movement: MovementSettings = make_movement(
            number(&settings["sprint_up_m_per_s_per_s"]),
            number(&settings["sprint_down_m_per_s_per_s"]),
        );
        let half_gap_m: f64 = number(&settings["mouth_gap_m"]) / 2.0;
        let level: FlatLevel = FlatLevel {
            gravity: &gravity,
            movement: &movement,
            walk_m_per_s: number(&settings["walk_m_per_s"]),
            sprint_m_per_s: number(&settings["sprint_m_per_s"]),
            energy_drain_s: number(&settings["energy_drain_s"]),
            energy_refill_s: number(&settings["energy_refill_s"]),
            mouths_m: [[-half_gap_m, 0.0], [half_gap_m, 0.0]],
            throat_radius_m: number(&settings["throat_radius_m"]),
        };
        let start: &Value = &recording["initial_state"];
        let mut player: FlatPlayer = FlatPlayer {
            at_m: [number(&start["x"]), number(&start["y"])],
            legs_m_per_s: number(&start["speed_cap_m_per_s"]),
            drift_m_per_s: [number(&start["u"][0]), number(&start["u"][1])],
            velocity_m_per_s: [number(&start["v"][0]), number(&start["v"][1])],
            energy_frac: number(&start["energy_frac"]),
            is_sprinting: false,
            pass_count: 0,
        };
        let bit = |name: &str| recording["input_bits"][name].as_u64().expect("a bit");
        let inputs: Vec<(u64, u64)> = recording["inputs"]
            .as_array()
            .expect("the key changes")
            .iter()
            .map(|change| (change[0].as_u64().unwrap(), change[1].as_u64().unwrap()))
            .collect();
        let samples: &Vec<Value> = recording["samples"].as_array().expect("the samples");
        let (mut next_input, mut bits, mut next_sample) = (0, 0, 0);
        let mut worst_m: f64 = 0.0;
        let tick_count: u64 = recording["tick_count"].as_u64().expect("the ticks");
        for tick in 0..tick_count {
            while next_input < inputs.len() && inputs[next_input].0 == tick {
                bits = inputs[next_input].1;
                next_input += 1;
            }
            let held = |name: &str| f64::from(u8::from(bits & bit(name) != 0));
            let input: FlatInput = FlatInput {
                dir: [held("right") - held("left"), held("up") - held("down")],
                is_sprint_held: bits & bit("sprint") != 0,
            };
            step_flat_player(&mut player, &input, &level);
            if next_sample < samples.len() && samples[next_sample][0].as_u64() == Some(tick + 1) {
                let (x, y) = (
                    number(&samples[next_sample][1]),
                    number(&samples[next_sample][2]),
                );
                worst_m = worst_m.max((player.at_m[0] - x).hypot(player.at_m[1] - y));
                next_sample += 1;
            }
        }
        assert_eq!(next_sample, samples.len());
        assert!(worst_m <= MAX_STRAY_M, "the replay strays {worst_m} m");
        let end: &Value = &recording["final_state"];
        assert_eq!(
            u64::from(player.pass_count),
            end["pass_count"].as_u64().unwrap()
        );
        assert!(player.pass_count >= 5);
        assert!((player.energy_frac - number(&end["energy_frac"])).abs() < 1e-9);
        assert!((player.legs_m_per_s - number(&end["speed_cap_m_per_s"])).abs() < 1e-9);
    }
}
