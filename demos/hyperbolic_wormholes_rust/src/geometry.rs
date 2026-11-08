//! `geometry.rs` does the double-precision maths of the hyperboloid model of H³: points are
//! 4-vectors x with ⟨x, x⟩ = -1 under the Minkowski product, and moves and turns are Lorentz
//! matrices (column-major, columns right, up, back, position for a frame).
//!
//! Interface: `minkowski_dot`, `make_rotation`, `make_boost`, `make_boost_along`,
//! `make_rotation_about`, `make_rotation_between`, `invert_lorentz`, `normalize_point`,
//! `normalize_tangent`, `get_distance`, `orthonormalize_frame`, `get_local_direction`,
//! `make_fermi_point`.
//!
//! Example: `let ahead = make_boost(AXIS_Z, -6.0 / curvature_radius_m);`

use glam::{DMat3, DMat4, DQuat, DVec3, DVec4};

pub const AXIS_X: usize = 0;
pub const AXIS_Y: usize = 1;
pub const AXIS_Z: usize = 2;
pub const AXIS_W: usize = 3;
pub const UP_VECTOR: DVec4 = DVec4::new(0.0, 1.0, 0.0, 0.0);
const MINKOWSKI_SIGNS: DVec4 = DVec4::new(1.0, 1.0, 1.0, -1.0);

/// `minkowski_dot` computes ax bx + ay by + az bz - aw bw.
///
/// Example: `let cosh_distance = -minkowski_dot(p, q);`
pub fn minkowski_dot(a: DVec4, b: DVec4) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z - a.w * b.w
}

/// `make_rotation` makes a rotation in the plane of spatial axes `i` and `j`, turning `i`
/// toward `j` by `angle_rad`.
///
/// Example: `let turn = make_rotation(AXIS_X, AXIS_Z, 0.5);`
pub fn make_rotation(i: usize, j: usize, angle_rad: f64) -> DMat4 {
    let mut m: DMat4 = DMat4::IDENTITY;
    let (s, c) = angle_rad.sin_cos();
    m.col_mut(i)[i] = c;
    m.col_mut(i)[j] = s;
    m.col_mut(j)[i] = -s;
    m.col_mut(j)[j] = c;
    m
}

/// `make_boost` makes the hyperbolic translation that moves the origin `s` curvature radii along
/// spatial axis `i`.
///
/// Example: `let ahead = make_boost(AXIS_Z, -0.5);`
pub fn make_boost(i: usize, s: f64) -> DMat4 {
    let mut m: DMat4 = DMat4::IDENTITY;
    m.col_mut(AXIS_W)[AXIS_W] = s.cosh();
    m.col_mut(AXIS_W)[i] = s.sinh();
    m.col_mut(i)[AXIS_W] = s.sinh();
    m.col_mut(i)[i] = s.cosh();
    m
}

/// `make_boost_along` makes the hyperbolic translation that moves the origin `s` curvature radii
/// along the unit spatial direction `direction`.
///
/// Example: `let step = make_boost_along(DVec3::NEG_Z, 0.1);`
pub fn make_boost_along(direction: DVec3, s: f64) -> DMat4 {
    let mut m: DMat4 = DMat4::IDENTITY;
    let (c, sh) = (s.cosh(), s.sinh());
    for col in 0..3 {
        for r in 0..3 {
            m.col_mut(col)[r] += (c - 1.0) * direction[col] * direction[r];
        }
        m.col_mut(col)[AXIS_W] = sh * direction[col];
        m.col_mut(AXIS_W)[col] = sh * direction[col];
    }
    m.col_mut(AXIS_W)[AXIS_W] = c;
    m
}

/// `make_rotation_about` makes a spatial rotation about a unit axis, counter-clockwise looking
/// down the axis.
///
/// Example: `let yaw = make_rotation_about(DVec3::Y, 0.1);`
pub fn make_rotation_about(axis: DVec3, angle_rad: f64) -> DMat4 {
    DMat4::from_mat3(DMat3::from_axis_angle(axis, angle_rad))
}

/// `make_rotation_between` makes the smallest spatial rotation that turns unit vector `from` into
/// unit vector `to`.
///
/// Example: `let turn = make_rotation_between(DVec3::NEG_Z, toward);`
pub fn make_rotation_between(from: DVec3, to: DVec3) -> DMat4 {
    DMat4::from_quat(DQuat::from_rotation_arc(from, to))
}

/// `invert_lorentz` inverts a Lorentz matrix as J Mᵀ J, with J = diag(1, 1, 1, -1).
///
/// Example: `let world_to_camera = invert_lorentz(camera_frame);`
pub fn invert_lorentz(m: DMat4) -> DMat4 {
    let j: DMat4 = DMat4::from_diagonal(MINKOWSKI_SIGNS);
    j * m.transpose() * j
}

/// `normalize_point` puts a 4-vector close to a point of H³ back on the hyperboloid.
///
/// Example: `position = normalize_point(position);`
pub fn normalize_point(x: DVec4) -> DVec4 {
    x / (-minkowski_dot(x, x)).sqrt()
}

/// `normalize_tangent` projects `w` onto the tangent space at point `x` and scales it to unit
/// length; `w` must not be parallel to `x`.
///
/// Example: `let up = normalize_tangent(position, UP_VECTOR);`
pub fn normalize_tangent(x: DVec4, w: DVec4) -> DVec4 {
    let t: DVec4 = w + minkowski_dot(w, x) * x;
    t / minkowski_dot(t, t).sqrt()
}

/// `get_distance` gives the hyperbolic distance between two points in curvature radii, accurate
/// even when they are close.
///
/// Example: `let distance_m = get_distance(p, q) * curvature_radius_m;`
pub fn get_distance(a: DVec4, b: DVec4) -> f64 {
    let d: DVec4 = a - b;
    2.0 * (0.5 * minkowski_dot(d, d).max(0.0).sqrt()).asinh()
}

/// `make_fermi_point` gives the point at Fermi coordinates (u, v, h) in meters on a map of
/// curvature radius `r_m`, the inverse of `terrain::get_fermi_coordinates`.
///
/// Example: `let x = make_fermi_point(12.0, -3.0, 0.0, 40.0);`
pub fn make_fermi_point(u_m: f64, v_m: f64, h_m: f64, r_m: f64) -> DVec4 {
    let (u, v, h) = (u_m / r_m, v_m / r_m, h_m / r_m);
    let ground_w: f64 = v.cosh() * h.cosh();
    DVec4::new(
        v.sinh() * h.cosh(),
        h.sinh(),
        -u.sinh() * ground_w,
        u.cosh() * ground_w,
    )
}

/// `orthonormalize_frame` repairs rounding drift: the last column becomes a point of H³ and the
/// other three an orthonormal basis of the tangent space there.
///
/// Example: `camera_frame = orthonormalize_frame(camera_frame);`
pub fn orthonormalize_frame(m: DMat4) -> DMat4 {
    let x: DVec4 = normalize_point(m.col(AXIS_W));
    let mut basis: Vec<DVec4> = Vec::with_capacity(3);
    for c in 0..3 {
        let mut e: DVec4 = m.col(c) + minkowski_dot(m.col(c), x) * x;
        for f in &basis {
            e -= minkowski_dot(e, *f) * *f;
        }
        basis.push(normalize_tangent(x, e));
    }
    DMat4::from_cols(basis[0], basis[1], basis[2], x)
}

/// `get_local_direction` expresses a tangent vector at a frame's position in the frame's own axes.
///
/// Example: `let up_local = get_local_direction(camera_frame, up_world);`
pub fn get_local_direction(frame: DMat4, w: DVec4) -> DVec3 {
    (invert_lorentz(frame) * w).truncate()
}

#[cfg(test)]
mod tests {
    //! Tests of the Lorentz maths.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Matrix kind | rotation, boost along an axis, boost along any direction | `isometries_keep_the_hyperboloid`, `boost_along_matches_axis_boost` |
    //! | Inverse of | identity-like, general composition | `inverse_undoes_a_frame` |
    //! | Distance | zero, small, large | `distance_matches_boost_length` |

    use super::*;

    const TOLERANCE: f64 = 1e-9;
    const ORIGIN_POINT: DVec4 = DVec4::new(0.0, 0.0, 0.0, 1.0);

    /// `isometries_keep_the_hyperboloid` checks that rotations and boosts map points of H³ to
    /// points of H³.
    #[test]
    fn isometries_keep_the_hyperboloid() {
        let m: DMat4 = make_rotation(AXIS_X, AXIS_Z, 0.7)
            * make_boost(AXIS_Y, 1.3)
            * make_boost_along(DVec3::new(0.6, 0.0, 0.8), 0.4);
        let x: DVec4 = m * ORIGIN_POINT;
        assert!((minkowski_dot(x, x) + 1.0).abs() < TOLERANCE);
    }

    /// `boost_along_matches_axis_boost` checks the general boost against the axis boost.
    #[test]
    fn boost_along_matches_axis_boost() {
        let difference: DMat4 = make_boost_along(DVec3::Z, 0.9) - make_boost(AXIS_Z, 0.9);
        assert!(
            difference
                .to_cols_array()
                .iter()
                .all(|v| v.abs() < TOLERANCE)
        );
    }

    /// `inverse_undoes_a_frame` checks that the Lorentz inverse is the inverse.
    #[test]
    fn inverse_undoes_a_frame() {
        for m in [
            DMat4::IDENTITY,
            make_rotation(AXIS_Y, AXIS_Z, 1.1) * make_boost(AXIS_X, 2.0),
        ] {
            let product: DMat4 = invert_lorentz(m) * m - DMat4::IDENTITY;
            assert!(product.to_cols_array().iter().all(|v| v.abs() < TOLERANCE));
        }
    }

    /// `distance_matches_boost_length` checks distances of 0, 0.001 and 5 curvature radii.
    #[test]
    fn distance_matches_boost_length() {
        for s in [0.0, 0.001, 5.0] {
            let distance: f64 = get_distance(ORIGIN_POINT, make_boost(AXIS_X, s) * ORIGIN_POINT);
            assert!((distance - s).abs() < 1e-7 * (1.0 + s));
        }
    }
}
