//! `laser_drop.rs` is bullet drop, the same in every level (W-157 (bullet drop, by game setting)):
//! the server settings' `shooting.drop_m_per_s_per_s` curves every shot downward as a ball thrown
//! at the gun's beam speed would fall, so its path bends by `get_drop_curvature_per_m` per meter.
//! A level traces a dropping shot in straight pieces of at most `DROP_PIECE_M`, turning its
//! direction down between them by `turn_flat` (flat space) or `turn_hyperbolic` (the
//! hyperboloid), and draws the beam along the same pieces; every window traces with the server's
//! settings, so a shot falls alike in all of them. At 0 (the default) nothing bends.
//!
//! Interface: `DROP_PIECE_M`, `get_drop_curvature_per_m`, `turn_flat`, `turn_hyperbolic`.
//!
//! Example: `let k_per_m = get_drop_curvature_per_m(9.8, 250.0); v = turn_flat(v, DVec3::NEG_Y, k_per_m, 4.0);`

use glam::{DVec3, DVec4};

use crate::geometry::{minkowski_dot, normalize_tangent};

pub const DROP_PIECE_M: f64 = 4.0;
const UP_NORMAL: DVec4 = DVec4::new(0.0, 1.0, 0.0, 0.0);

/// `get_drop_curvature_per_m` gives how much a shot's path turns down per meter (1/m) when it
/// falls at `drop_m_per_s_per_s` while moving at `beam_speed_m_per_s`: a / v², a thrown path's
/// curvature at its start.
///
/// Example: `let k_per_m: f64 = get_drop_curvature_per_m(10.0, 250.0); // 0.00016`
pub fn get_drop_curvature_per_m(drop_m_per_s_per_s: f64, beam_speed_m_per_s: f64) -> f64 {
    assert!(
        beam_speed_m_per_s > 0.0,
        "get_drop_curvature_per_m: the beam speed must be above 0, not {beam_speed_m_per_s}"
    );
    drop_m_per_s_per_s / (beam_speed_m_per_s * beam_speed_m_per_s)
}

/// `turn_flat` turns unit direction `v` toward `down` by `curvature_per_m` over `ds_m` meters
/// (the part of `down` across `v`), in flat space.
///
/// Example: `let v = turn_flat(v, DVec3::NEG_Y, 0.00016, 4.0);`
pub fn turn_flat(v: DVec3, down: DVec3, curvature_per_m: f64, ds_m: f64) -> DVec3 {
    let across: DVec3 = down - down.dot(v) * v;
    (v + across * curvature_per_m * ds_m).normalize()
}

/// `turn_hyperbolic` turns unit tangent `v` at `x` on the hyperboloid of curvature radius `r_m`
/// toward the ground (away from the plane y = 0's up normal) by `curvature_per_m` over `ds` (in
/// units of `r_m`), as Dust's wind turns light (`WindField::kick`), with the plane's normal
/// pointing down.
///
/// Example: `let v = turn_hyperbolic(x, v, 0.00016, 26.7, 4.0 / 26.7);`
pub fn turn_hyperbolic(x: DVec4, v: DVec4, curvature_per_m: f64, r_m: f64, ds: f64) -> DVec4 {
    let f: f64 = minkowski_dot(x, UP_NORMAL);
    let up: DVec4 = (UP_NORMAL + f * x) / (1.0 + f * f).sqrt();
    let g: DVec4 = -curvature_per_m * r_m * up;
    normalize_tangent(x, v + (g - minkowski_dot(g, v) * v) * ds)
}

#[cfg(test)]
mod tests {
    //! Tests of bullet drop's turn.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Drop | 0 (no turn), above 0 (falls about a t² / 2) | `a_flat_shot_falls_like_a_thrown_ball`, `no_drop_turns_nothing` |
    //! | Space | flat, hyperboloid near its origin (matches flat) | `a_flat_shot_falls_like_a_thrown_ball`, `the_hyperboloid_turns_down_like_flat_space` |
    //! | Direction | level, straight down (no turn across it) | `a_flat_shot_falls_like_a_thrown_ball`, `no_drop_turns_nothing` |

    use super::*;

    const DROP_M_PER_S_PER_S: f64 = 10.0;
    const BEAM_SPEED_M_PER_S: f64 = 250.0;
    const RANGE_M: f64 = 200.0;
    const R_M: f64 = 1000.0;
    const TOLERANCE_FRAC: f64 = 0.02;

    #[test]
    fn a_flat_shot_falls_like_a_thrown_ball() {
        let k_per_m: f64 = get_drop_curvature_per_m(DROP_M_PER_S_PER_S, BEAM_SPEED_M_PER_S);
        let (mut x, mut v) = (DVec3::ZERO, DVec3::X);
        let mut travelled_m: f64 = 0.0;
        while travelled_m < RANGE_M {
            v = turn_flat(v, DVec3::NEG_Y, k_per_m, 0.5 * DROP_PIECE_M);
            x += v * DROP_PIECE_M;
            v = turn_flat(v, DVec3::NEG_Y, k_per_m, 0.5 * DROP_PIECE_M);
            travelled_m += DROP_PIECE_M;
        }
        let t_s: f64 = RANGE_M / BEAM_SPEED_M_PER_S;
        let expected_m: f64 = 0.5 * DROP_M_PER_S_PER_S * t_s * t_s;
        assert!(
            ((-x.y) - expected_m).abs() < expected_m * TOLERANCE_FRAC,
            "{x}"
        );
    }

    #[test]
    fn no_drop_turns_nothing() {
        assert_eq!(
            turn_flat(DVec3::X, DVec3::NEG_Y, 0.0, DROP_PIECE_M),
            DVec3::X
        );
        assert_eq!(
            turn_flat(DVec3::NEG_Y, DVec3::NEG_Y, 1.0, DROP_PIECE_M),
            DVec3::NEG_Y
        );
        let origin: DVec4 = DVec4::W;
        let ahead: DVec4 = DVec4::X;
        assert_eq!(turn_hyperbolic(origin, ahead, 0.0, R_M, 1.0), ahead);
    }

    #[test]
    fn the_hyperboloid_turns_down_like_flat_space() {
        let k_per_m: f64 = get_drop_curvature_per_m(DROP_M_PER_S_PER_S, BEAM_SPEED_M_PER_S);
        let ds: f64 = DROP_PIECE_M / R_M;
        let (mut x, mut v) = (DVec4::W, DVec4::X);
        let mut travelled_m: f64 = 0.0;
        while travelled_m < RANGE_M {
            v = turn_hyperbolic(x, v, k_per_m, R_M, 0.5 * ds);
            (x, v) = (x * ds.cosh() + v * ds.sinh(), x * ds.sinh() + v * ds.cosh());
            v = turn_hyperbolic(x, v, k_per_m, R_M, 0.5 * ds);
            travelled_m += DROP_PIECE_M;
        }
        let t_s: f64 = RANGE_M / BEAM_SPEED_M_PER_S;
        let expected_m: f64 = 0.5 * DROP_M_PER_S_PER_S * t_s * t_s;
        let fallen_m: f64 = -x.y * R_M;
        assert!(
            (fallen_m - expected_m).abs() < expected_m * TOLERANCE_FRAC,
            "{x}"
        );
    }
}
