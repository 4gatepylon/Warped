//! `bend.rs` bends a map onto the inside of a sphere (W-68, reality bend). The bend works in the
//! normal coordinates ξ = log_o(p) of the map's origin o = (0, 0, 0, 1), in meters: the ground is
//! the plane ξ_y = 0 and |ξ| is the distance from o, so lines out from o keep their lengths. With
//! curvature k, the point at horizontal distance r from o's vertical axis and height h goes to
//! polar angle θ = k r on a sphere of radius 1/k that touches the ground at o, h inward from its
//! surface, and back to H³ by exp_o. Everything farther than the edge radius L from the axis is
//! cut away, and k = π · strength / L, so strength 1 closes the disc of radius L into the whole
//! sphere and strength 0 leaves the map as it is.
//!
//! Interface: `DEFAULT_EDGE_RADIUS_M`, `Bend` (`FromStrength`, `is_flat`, `bend_point`, `unbend_point`,
//! `is_beyond_edge`, `bend_frame`).
//!
//! Example: `let world_eye = Bend::FromStrength(0.4, 160.0, 40.0).bend_point(eye);`
//!
//! Must know:
//! - Gotchas: map points are hyperboloid points in curvature units, as everywhere in this demo;
//!   `curvature_per_m`, `edge_radius_m` and ξ are in meters.
//! - Limitations: `unbend_point` gives polar angles up to π, so it inverts `bend_point` only for
//!   points within the edge radius.

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, minkowski_dot, normalize_tangent};

pub const DEFAULT_EDGE_RADIUS_M: f64 = 160.0;
const SERIES_BELOW_RAD: f64 = 1e-4;
const FLAT_BELOW_PER_M: f64 = 1e-12;
const FRAME_STEP: f64 = 1e-6;

/// `Bend` is one strength of the bend: its curvature `curvature_per_m` (0 when flat), the edge
/// radius `edge_radius_m` beyond which the map is cut away, and the map's curvature radius `r_m`.
#[derive(Clone, Copy, Debug)]
pub struct Bend {
    pub curvature_per_m: f64,
    pub edge_radius_m: f64,
    pub r_m: f64,
}

/// `get_sinc` is sin(t) / t, with its series near 0.
fn get_sinc(t: f64) -> f64 {
    if t.abs() < SERIES_BELOW_RAD {
        1.0 - t * t / 6.0
    } else {
        t.sin() / t
    }
}

/// `get_versinc` is (1 - cos t) / t, with its series near 0.
fn get_versinc(t: f64) -> f64 {
    if t.abs() < SERIES_BELOW_RAD {
        t / 2.0
    } else {
        2.0 * (t / 2.0).sin().powi(2) / t
    }
}

/// `log_origin` gives a point's normal coordinates at o in meters (curvature radius `r_m`).
fn log_origin(p: DVec4, r_m: f64) -> DVec3 {
    let spatial: DVec3 = p.truncate();
    let sinh_d: f64 = spatial.length();
    if sinh_d == 0.0 {
        return DVec3::ZERO;
    }
    spatial * (r_m * sinh_d.asinh() / sinh_d)
}

/// `exp_origin` gives the point at normal coordinates ξ (meters) at o.
fn exp_origin(xi: DVec3, r_m: f64) -> DVec4 {
    let d: f64 = xi.length() / r_m;
    if d == 0.0 {
        return DVec4::W;
    }
    (xi * (d.sinh() / (d * r_m))).extend(d.cosh())
}

impl Bend {
    /// `FromStrength` gives the bend that raises the edge (at `edge_radius_m`) to polar angle
    /// π · `strength_frac`, on a map of curvature radius `r_m`.
    ///
    /// Example: `let bend = Bend::FromStrength(1.0, 160.0, 40.0);`
    #[allow(non_snake_case)]
    pub fn FromStrength(strength_frac: f64, edge_radius_m: f64, r_m: f64) -> Bend {
        assert!(
            (0.0..=1.0).contains(&strength_frac),
            "Bend::FromStrength: strength {strength_frac}, expected 0 to 1"
        );
        assert!(
            edge_radius_m > 0.0 && r_m > 0.0,
            "Bend::FromStrength: edge {edge_radius_m} m, R {r_m} m, expected both > 0"
        );
        Bend {
            curvature_per_m: std::f64::consts::PI * strength_frac / edge_radius_m,
            edge_radius_m,
            r_m,
        }
    }

    /// `is_flat` says whether the bend leaves every point where it is.
    ///
    /// Example: `if bend.is_flat() { trace_straight() }`
    pub fn is_flat(&self) -> bool {
        self.curvature_per_m < FLAT_BELOW_PER_M
    }

    /// `bend_point` moves a map point to where the bent world shows it.
    ///
    /// Example: `let world_eye = bend.bend_point(eye);`
    pub fn bend_point(&self, p: DVec4) -> DVec4 {
        if self.is_flat() {
            return p;
        }
        let xi: DVec3 = log_origin(p, self.r_m);
        let (r, h) = (xi.x.hypot(xi.z), xi.y);
        let theta: f64 = self.curvature_per_m * r;
        let out_m: f64 = r * get_sinc(theta) - h * theta.sin();
        let y: f64 = r * get_versinc(theta) + h * theta.cos();
        let scale: f64 = if r == 0.0 { 0.0 } else { out_m / r };
        exp_origin(DVec3::new(xi.x * scale, y, xi.z * scale), self.r_m)
    }

    /// `unbend_point` gives the map point that the bent world shows at `x`, the inverse of
    /// `bend_point` within the edge radius.
    ///
    /// Example: `let map_point = bend.unbend_point(sample);`
    pub fn unbend_point(&self, x: DVec4) -> DVec4 {
        if self.is_flat() {
            return x;
        }
        let xi: DVec3 = log_origin(x, self.r_m);
        let sphere_radius_m: f64 = 1.0 / self.curvature_per_m;
        let (out_m, dy) = (xi.x.hypot(xi.z), xi.y - sphere_radius_m);
        let theta: f64 = out_m.atan2(-dy);
        let h: f64 = sphere_radius_m - out_m.hypot(dy);
        let scale: f64 = if out_m == 0.0 {
            0.0
        } else {
            theta / (self.curvature_per_m * out_m)
        };
        exp_origin(DVec3::new(xi.x * scale, h, xi.z * scale), self.r_m)
    }

    /// `is_beyond_edge` says whether a map point is farther than the edge radius from o's
    /// vertical axis, so the bent world cuts it away. A flat bend cuts nothing.
    ///
    /// Example: `if bend.is_beyond_edge(map_point) { continue }`
    pub fn is_beyond_edge(&self, p: DVec4) -> bool {
        let xi: DVec3 = log_origin(p, self.r_m);
        !self.is_flat() && xi.x.hypot(xi.z) > self.edge_radius_m
    }

    /// `bend_frame` moves a map frame (columns right, up, back, position) into the bent world:
    /// its position by `bend_point`, its axes by the bend's derivative there, made orthonormal
    /// keeping the back axis exact, then the up axis.
    ///
    /// Example: `let world_camera = bend.bend_frame(camera);`
    pub fn bend_frame(&self, m: DMat4) -> DMat4 {
        if self.is_flat() {
            return m;
        }
        let origin: DVec4 = m.col(AXIS_W);
        let x: DVec4 = self.bend_point(origin);
        let push = |axis: usize| -> DVec4 {
            let e: DVec4 = m.col(axis);
            let ahead: DVec4 = self.bend_point(origin * FRAME_STEP.cosh() + e * FRAME_STEP.sinh());
            let behind: DVec4 = self.bend_point(origin * FRAME_STEP.cosh() - e * FRAME_STEP.sinh());
            (ahead - behind) / (2.0 * FRAME_STEP)
        };
        let back: DVec4 = normalize_tangent(x, push(AXIS_Z));
        let up_raw: DVec4 = push(AXIS_Y) + minkowski_dot(push(AXIS_Y), x) * x;
        let up: DVec4 = normalize_tangent(x, up_raw - minkowski_dot(up_raw, back) * back);
        let right_raw: DVec4 = push(AXIS_X) + minkowski_dot(push(AXIS_X), x) * x;
        let right: DVec4 = normalize_tangent(
            x,
            right_raw - minkowski_dot(right_raw, back) * back - minkowski_dot(right_raw, up) * up,
        );
        DMat4::from_cols(right, up, back, x)
    }
}

#[cfg(test)]
mod tests {
    //! Tests for `bend.rs`.
    //!
    //! Partitions of the input:
    //! - strength: 0 (flat), between, 1 (closed);
    //! - the point: on o's vertical axis, on the ground away from it, above the ground, at the
    //!   edge, beyond the edge;
    //! - the operation: `bend_point`, `unbend_point`, `is_beyond_edge`, `bend_frame`.
    //!
    //! | Test | Strength | Point | Operation |
    //! | --- | --- | --- | --- |
    //! | `a_flat_bend_moves_nothing` | 0 | ground, above | all |
    //! | `unbending_undoes_bending` | between, 1 | axis, ground, above | bend, unbend |
    //! | `the_edge_closes_at_the_top` | 1 | edge | bend |
    //! | `the_axis_stays_put` | between | axis | bend |
    //! | `points_beyond_the_edge_are_cut` | between, 0 | ground, beyond the edge | is_beyond_edge |
    //! | `bent_frames_are_orthonormal_and_turn_up_inward` | 1 | ground | bend_frame |

    use super::*;
    use crate::geometry::{get_distance, make_fermi_point};

    const R_M: f64 = 40.0;
    const EDGE_M: f64 = 160.0;
    const TOLERANCE: f64 = 1e-9;
    const STEP: f64 = 1e-4;

    fn sample_points() -> Vec<DVec4> {
        vec![
            make_fermi_point(0.0, 0.0, 17.0, R_M),
            make_fermi_point(-96.0, -16.0, 1.1, R_M),
            make_fermi_point(40.0, 30.0, 0.0, R_M),
            make_fermi_point(10.0, -5.0, 6.0, R_M),
        ]
    }

    #[test]
    fn a_flat_bend_moves_nothing() {
        let bend: Bend = Bend::FromStrength(0.0, EDGE_M, R_M);
        for p in sample_points() {
            assert_eq!(bend.bend_point(p), p);
            assert_eq!(bend.unbend_point(p), p);
            assert!(!bend.is_beyond_edge(p));
        }
    }

    #[test]
    fn unbending_undoes_bending() {
        for strength_frac in [0.3, 1.0] {
            let bend: Bend = Bend::FromStrength(strength_frac, EDGE_M, R_M);
            for p in sample_points() {
                let back: DVec4 = bend.unbend_point(bend.bend_point(p));
                assert!(
                    get_distance(back, p) < TOLERANCE,
                    "strength {strength_frac}: {p} came back as {back}"
                );
            }
        }
    }

    #[test]
    fn the_edge_closes_at_the_top() {
        let bend: Bend = Bend::FromStrength(1.0, EDGE_M, R_M);
        let top: DVec4 = exp_origin(
            DVec3::new(0.0, 2.0 * EDGE_M / std::f64::consts::PI, 0.0),
            R_M,
        );
        for angle_rad in [0.0_f64, 1.0, 2.5, 4.0] {
            let edge: DVec4 = exp_origin(
                DVec3::new(EDGE_M * angle_rad.cos(), 0.0, EDGE_M * angle_rad.sin()),
                R_M,
            );
            assert!(get_distance(bend.bend_point(edge), top) * R_M < 1e-6);
        }
    }

    #[test]
    fn the_axis_stays_put() {
        let bend: Bend = Bend::FromStrength(0.6, EDGE_M, R_M);
        let above: DVec4 = make_fermi_point(0.0, 0.0, 17.0, R_M);
        assert!(get_distance(bend.bend_point(above), above) < TOLERANCE);
    }

    #[test]
    fn points_beyond_the_edge_are_cut() {
        let beyond: DVec4 = make_fermi_point(170.0, 0.0, 0.0, R_M);
        let inside: DVec4 = make_fermi_point(150.0, 0.0, 0.0, R_M);
        assert!(Bend::FromStrength(0.5, EDGE_M, R_M).is_beyond_edge(beyond));
        assert!(!Bend::FromStrength(0.5, EDGE_M, R_M).is_beyond_edge(inside));
        assert!(!Bend::FromStrength(0.0, EDGE_M, R_M).is_beyond_edge(beyond));
    }

    #[test]
    fn bent_frames_are_orthonormal_and_turn_up_inward() {
        let bend: Bend = Bend::FromStrength(1.0, EDGE_M, R_M);
        let feet: DVec4 = exp_origin(DVec3::new(0.0, 0.0, -EDGE_M / 2.0), R_M);
        let frame: DMat4 = DMat4::from_cols(
            DVec4::X,
            normalize_tangent(feet, DVec4::Y),
            normalize_tangent(feet, DVec4::Z),
            feet,
        );
        let bent: DMat4 = bend.bend_frame(frame);
        for i in 0..3 {
            for j in 0..3 {
                let expected: f64 = if i == j { 1.0 } else { 0.0 };
                assert!((minkowski_dot(bent.col(i), bent.col(j)) - expected).abs() < 1e-6);
            }
            assert!(minkowski_dot(bent.col(i), bent.col(AXIS_W)).abs() < 1e-6);
        }
        let world_feet: DVec4 = bend.bend_point(feet);
        let step = |f: DMat4, axis: usize| -> DVec4 {
            f.col(AXIS_W) * STEP.cosh() + f.col(axis) * STEP.sinh()
        };
        let map_back: DVec4 = normalize_tangent(feet, bend.unbend_point(step(bent, AXIS_Z)));
        let back_cos: f64 = minkowski_dot(map_back, frame.col(AXIS_Z));
        assert!(
            back_cos > 1.0 - 1e-6,
            "the bent back axis should point along the map's back axis, cosine {back_cos}"
        );
        let lift_m: f64 = log_origin(bend.unbend_point(step(bent, AXIS_Y)), R_M).y;
        assert!(
            lift_m > 0.5 * STEP * R_M,
            "the bent up axis should lift off the ground, lifted {lift_m} m"
        );
        let world_xi: DVec3 = log_origin(step(bent, AXIS_Y), R_M) - log_origin(world_feet, R_M);
        assert!(
            world_xi.z > 0.0,
            "at a quarter turn, up should lean back toward the axis: {world_xi}"
        );
    }
}
