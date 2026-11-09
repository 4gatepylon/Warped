//! `torus.rs` is the looping world of a map whose ground's two axes loop (T² × ℝ, the map's
//! `space.torus_periods_m`, DECISIONS S11): the cell of one period each way around the origin,
//! the copy of a spot nearest another, and moving frames from one copy to another.
//!
//! The level is drawn in nearly flat H³ (`Map::curve_flat`), in Fermi coordinates (u, v, h). The
//! torus glues the cell's edges there (S11's option G1): crossing u = ±period/2 is a boost along
//! u, an exact isometry; crossing v = ±period/2 keeps u, h and a direction's parts along the Fermi
//! axes, which matches the metric on both sides.
//!
//! Interface: `get_cell_offset_m` and `get_nearest_offset_m` (any space; Dark Forest's flat
//! space can use them as they are), and `Torus` (frames and points in H³).
//!
//! Example: `let body = torus.wrap_frame(body);`

use glam::{DMat4, DVec4};

use crate::geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, invert_lorentz, make_boost};
use crate::terrain::get_fermi_coordinates;

/// `SEAM_MIN_M` is how near a seam crossing may be and still not count: the seam a ray was just carried
/// across (as the shader's constant of the same name).
const SEAM_MIN_M: f64 = 0.001;

/// `get_cell_offset_m` gives the multiple of the periods that brings ground spot (u, v) into the
/// cell around the origin, [-period/2, period/2) each way.
///
/// Example: `assert_eq!(get_cell_offset_m([60.0, 0.0], [112.0, 72.0]), [-112.0, 0.0]);`
pub fn get_cell_offset_m(spot_m: [f64; 2], periods_m: [f64; 2]) -> [f64; 2] {
    [0, 1].map(|i| -((spot_m[i] / periods_m[i] + 0.5).floor()) * periods_m[i])
}

/// `get_nearest_offset_m` gives the multiple of the periods to add to `to_m` to get its copy
/// nearest `from_m`.
///
/// Example: `assert_eq!(get_nearest_offset_m([-50.0, 0.0], [50.0, 0.0], [112.0, 72.0]), [-112.0, 0.0]);`
pub fn get_nearest_offset_m(from_m: [f64; 2], to_m: [f64; 2], periods_m: [f64; 2]) -> [f64; 2] {
    get_cell_offset_m([to_m[0] - from_m[0], to_m[1] - from_m[1]], periods_m)
}

/// `get_ghost_offsets_m` gives the multiples of the periods, other than none, at which a copy of
/// something at ground spot `spot_m` (in the cell) that reaches `reach_m` from it would reach into
/// the cell from outside: where its ghosts stand, so that it shows and blocks across the seams.
///
/// Example: `assert_eq!(get_ghost_offsets_m([50.0, 0.0], 10.0, [112.0, 72.0]), vec![[-112.0, 0.0]]);`
pub fn get_ghost_offsets_m(spot_m: [f64; 2], reach_m: f64, periods_m: [f64; 2]) -> Vec<[f64; 2]> {
    let along = |i: usize| -> Vec<f64> {
        let half_m: f64 = periods_m[i] / 2.0;
        let mut offsets_m: Vec<f64> = vec![0.0];
        if spot_m[i] > half_m - reach_m {
            offsets_m.push(-periods_m[i]);
        }
        if spot_m[i] < reach_m - half_m {
            offsets_m.push(periods_m[i]);
        }
        offsets_m
    };
    let (along_u, along_v) = (along(0), along(1));
    along_u
        .iter()
        .flat_map(|&du_m| along_v.iter().map(move |&dv_m| [du_m, dv_m]))
        .filter(|&offset_m| offset_m != [0.0, 0.0])
        .collect()
}

/// `Torus` is a map's looping world: its periods along u and v and the curvature radius the map
/// is drawn at.
#[derive(Clone, Copy, Debug)]
pub struct Torus {
    pub periods_m: [f64; 2],
    r_m: f64,
}

impl Torus {
    /// `New` makes the torus of `periods_m` (u, v) on a map drawn at curvature radius `r_m`.
    ///
    /// Example: `let torus = Torus::New([112.0, 72.0], 160.0);`
    #[allow(non_snake_case)]
    pub fn New(periods_m: [f64; 2], r_m: f64) -> Torus {
        Torus { periods_m, r_m }
    }

    /// `get_spot_m` gives point x's ground spot (u, v) in meters.
    ///
    /// Example: `let [u_m, v_m] = torus.get_spot_m(x);`
    pub fn get_spot_m(&self, x: DVec4) -> [f64; 2] {
        let [u_m, v_m, _] = get_fermi_coordinates(x, self.r_m);
        [u_m, v_m]
    }

    /// `shift_frame` moves `frame` (a body, a camera, a structure's frame) `offset_m` (u, v) along
    /// the ground by the glue: to the same Fermi coordinates plus the offset, turned the same way
    /// against the Fermi axes there.
    ///
    /// Example: `let ghost = torus.shift_frame(frame, [112.0, 0.0]);`
    pub fn shift_frame(&self, frame: DMat4, offset_m: [f64; 2]) -> DMat4 {
        let [u_m, v_m, h_m] = get_fermi_coordinates(frame.col(AXIS_W), self.r_m);
        let from: DMat4 = self.make_fermi_frame([u_m, v_m, h_m]);
        let to: DMat4 = self.make_fermi_frame([u_m + offset_m[0], v_m + offset_m[1], h_m]);
        to * invert_lorentz(from) * frame
    }

    /// `wrap_frame` moves `frame` into the cell around the origin, if it is past a seam.
    ///
    /// Example: `player.body = torus.wrap_frame(player.body);`
    pub fn wrap_frame(&self, frame: DMat4) -> DMat4 {
        let offset_m: [f64; 2] =
            get_cell_offset_m(self.get_spot_m(frame.col(AXIS_W)), self.periods_m);
        if offset_m == [0.0, 0.0] {
            return frame;
        }
        self.shift_frame(frame, offset_m)
    }

    /// `get_nearest_offset_m` gives the multiple of the periods that brings point `to` nearest
    /// point `from`.
    ///
    /// Example: `let offset_m = torus.get_nearest_offset_m(eye, other_eye);`
    pub fn get_nearest_offset_m(&self, from: DVec4, to: DVec4) -> [f64; 2] {
        get_nearest_offset_m(self.get_spot_m(from), self.get_spot_m(to), self.periods_m)
    }

    /// `move_nearest` gives the copy of point `to` nearest point `from`.
    ///
    /// Example: `let target = torus.move_nearest(eye, other_eye);`
    pub fn move_nearest(&self, from: DVec4, to: DVec4) -> DVec4 {
        let offset_m: [f64; 2] = self.get_nearest_offset_m(from, to);
        if offset_m == [0.0, 0.0] {
            return to;
        }
        let frame: DMat4 = self.make_fermi_frame(get_fermi_coordinates(to, self.r_m));
        self.shift_frame(frame, offset_m).col(AXIS_W)
    }

    /// `find_seam_s` gives how far (curvature units) the geodesic from point x along unit tangent
    /// v goes before it first crosses a seam (u or v at ± half its period), with the seam's axis
    /// (0 for u, 1 for v), as the shader's `find_seam_s` does: a linear equation in the chord's
    /// Klein parameter λ = tanh s for u, a quadratic for v. Crossings nearer than `SEAM_MIN_M`
    /// don't count, so a ray just carried across a seam goes on.
    ///
    /// Example: `if let Some((s, axis)) = torus.find_seam_s(x, v) { ... }`
    pub fn find_seam_s(&self, x: DVec4, v: DVec4) -> Option<(f64, usize)> {
        let lam_min: f64 = (SEAM_MIN_M / self.r_m).tanh();
        let mut best: Option<(f64, usize)> = None;
        let mut take = |lam: f64, axis: usize| {
            if lam > lam_min && lam < 1.0 && best.is_none_or(|(s, _)| lam.atanh() < s) {
                best = Some((lam.atanh(), axis));
            }
        };
        let k: f64 = (0.5 * self.periods_m[0] / self.r_m).tanh();
        for side in [-1.0, 1.0] {
            let den: f64 = v.z + side * k * v.w;
            if den.abs() > f64::EPSILON {
                take(-(x.z + side * k * x.w) / den, 0);
            }
        }
        let half_v: f64 = 0.5 * self.periods_m[1] / self.r_m;
        let (c2, s2) = (half_v.cosh().powi(2), half_v.sinh().powi(2));
        let qa: f64 = c2 * v.x * v.x - s2 * (v.w * v.w - v.z * v.z);
        let qb: f64 = 2.0 * (c2 * x.x * v.x - s2 * (x.w * v.w - x.z * v.z));
        let qc: f64 = c2 * x.x * x.x - s2 * (x.w * x.w - x.z * x.z);
        let disc: f64 = qb * qb - 4.0 * qa * qc;
        if disc >= 0.0 && qa.abs() > f64::EPSILON {
            for root in [-disc.sqrt(), disc.sqrt()] {
                take((-qb + root) / (2.0 * qa), 1);
            }
        }
        best
    }

    /// `cross_seam` carries a ray at a seam (point x, tangent v) across it by the glue: to its
    /// Fermi (u, v, h) less a period along `axis`, turned the same way against the Fermi axes.
    ///
    /// Example: `let (x, v) = torus.cross_seam(x, v, 0);`
    pub fn cross_seam(&self, x: DVec4, v: DVec4, axis: usize) -> (DVec4, DVec4) {
        let [u_m, v_m] = self.get_spot_m(x);
        let mut offset_m: [f64; 2] = [0.0, 0.0];
        offset_m[axis] = -[u_m, v_m][axis].signum() * self.periods_m[axis];
        self.move_ray(x, v, offset_m)
    }

    /// `wrap_ray` moves a ray past a seam (one that left a warp's reach there) back into the cell.
    ///
    /// Example: `let (x, v) = torus.wrap_ray(x, v);`
    pub fn wrap_ray(&self, x: DVec4, v: DVec4) -> (DVec4, DVec4) {
        let offset_m: [f64; 2] = get_cell_offset_m(self.get_spot_m(x), self.periods_m);
        if offset_m == [0.0, 0.0] {
            return (x, v);
        }
        self.move_ray(x, v, offset_m)
    }

    /// `move_ray` moves a ray (point x, tangent v) `offset_m` (u, v) along the ground by the glue.
    ///
    /// Example: `let (x, v) = torus.move_ray(x, v, [112.0, 0.0]);`
    pub fn move_ray(&self, x: DVec4, v: DVec4, offset_m: [f64; 2]) -> (DVec4, DVec4) {
        let fermi_m: [f64; 3] = get_fermi_coordinates(x, self.r_m);
        let to_m: [f64; 3] = [
            fermi_m[0] + offset_m[0],
            fermi_m[1] + offset_m[1],
            fermi_m[2],
        ];
        let glue: DMat4 =
            self.make_fermi_frame(to_m) * invert_lorentz(self.make_fermi_frame(fermi_m));
        (glue * x, glue * v)
    }

    /// `make_fermi_frame` is the frame at Fermi (u, v, h) whose axes are the Fermi axes there,
    /// as `Map::curve_flat` places a flat map's frames: B_z(−u/R) · B_x(v/R) · B_y(h/R).
    fn make_fermi_frame(&self, fermi_m: [f64; 3]) -> DMat4 {
        make_boost(AXIS_Z, -fermi_m[0] / self.r_m)
            * make_boost(AXIS_X, fermi_m[1] / self.r_m)
            * make_boost(AXIS_Y, fermi_m[2] / self.r_m)
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the torus.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Spot | inside the cell, past one seam (u or v), past both, on the seam's lower and upper side | `spots_wrap_into_the_cell` |
    //! | Reach of a spot past the seams | none, past u, past v, past both and the corner | `ghosts_stand_past_the_seams_they_reach` |
    //! | Nearest copy | the same cell, across the u seam, across the v seam | `the_nearest_copy_is_the_short_way_around` |
    //! | Seam crossed | u (an exact boost), v (the Fermi glue) | `wrapping_keeps_height_and_heading`, `crossing_u_is_a_boost` |

    use super::*;
    use crate::geometry::{get_distance, make_fermi_point, make_rotation};

    const PERIODS_M: [f64; 2] = [112.0, 72.0];
    const R_M: f64 = 160.0;
    const TOLERANCE: f64 = 1e-9;
    const DISTANCE_TOLERANCE_M: f64 = 0.01;

    #[test]
    fn spots_wrap_into_the_cell() {
        for (spot_m, offset_m) in [
            ([10.0, -5.0], [0.0, 0.0]),
            ([57.0, 0.0], [-112.0, 0.0]),
            ([0.0, -37.0], [0.0, 72.0]),
            ([-60.0, 40.0], [112.0, -72.0]),
            ([-56.0, 0.0], [0.0, 0.0]),
            ([56.0, 36.0], [-112.0, -72.0]),
        ] {
            assert_eq!(get_cell_offset_m(spot_m, PERIODS_M), offset_m, "{spot_m:?}");
        }
    }

    #[test]
    fn ghosts_stand_past_the_seams_they_reach() {
        assert!(get_ghost_offsets_m([0.0, 0.0], 10.0, PERIODS_M).is_empty());
        assert_eq!(
            get_ghost_offsets_m([50.0, 0.0], 10.0, PERIODS_M),
            vec![[-112.0, 0.0]]
        );
        assert_eq!(
            get_ghost_offsets_m([0.0, -30.0], 10.0, PERIODS_M),
            vec![[0.0, 72.0]]
        );
        assert_eq!(
            get_ghost_offsets_m([50.0, 30.0], 10.0, PERIODS_M),
            vec![[0.0, -72.0], [-112.0, 0.0], [-112.0, -72.0]]
        );
    }

    #[test]
    fn the_nearest_copy_is_the_short_way_around() {
        assert_eq!(
            get_nearest_offset_m([1.0, 2.0], [5.0, -3.0], PERIODS_M),
            [0.0, 0.0]
        );
        assert_eq!(
            get_nearest_offset_m([-50.0, 0.0], [50.0, 0.0], PERIODS_M),
            [-112.0, 0.0]
        );
        assert_eq!(
            get_nearest_offset_m([0.0, 30.0], [0.0, -30.0], PERIODS_M),
            [0.0, 72.0]
        );
        let torus: Torus = Torus::New(PERIODS_M, R_M);
        let a: DVec4 = make_fermi_point(-50.0, 0.0, 1.6, R_M);
        let b: DVec4 = make_fermi_point(50.0, 0.0, 1.6, R_M);
        assert!(
            (get_distance(a, torus.move_nearest(a, b)) * R_M - 12.0).abs() < DISTANCE_TOLERANCE_M
        );
    }

    #[test]
    fn wrapping_keeps_height_and_heading() {
        let torus: Torus = Torus::New(PERIODS_M, R_M);
        for (fermi_m, wrapped_m) in [
            ([57.0, 3.0, 1.6], [-55.0, 3.0]),
            ([-4.0, 37.5, 1.6], [-4.0, -34.5]),
        ] {
            let body: DMat4 = torus.make_fermi_frame(fermi_m) * make_rotation(AXIS_X, AXIS_Z, 0.7);
            let wrapped: DMat4 = torus.wrap_frame(body);
            let [u_m, v_m, h_m] = get_fermi_coordinates(wrapped.col(AXIS_W), R_M);
            assert!(
                (u_m - wrapped_m[0]).abs() < 1e-6 && (v_m - wrapped_m[1]).abs() < 1e-6,
                "{u_m} {v_m}"
            );
            assert!((h_m - fermi_m[2]).abs() < 1e-6);
            let turn = |frame: DMat4| {
                invert_lorentz(
                    torus.make_fermi_frame(get_fermi_coordinates(frame.col(AXIS_W), R_M)),
                ) * frame
            };
            assert!(turn(body).abs_diff_eq(turn(wrapped), 1e-6));
            assert!(torus.wrap_frame(wrapped).abs_diff_eq(wrapped, TOLERANCE));
        }
    }

    #[test]
    fn crossing_u_is_a_boost() {
        let torus: Torus = Torus::New(PERIODS_M, R_M);
        let body: DMat4 =
            torus.make_fermi_frame([50.0, 20.0, 1.6]) * make_rotation(AXIS_X, AXIS_Z, -1.1);
        let boosted: DMat4 = make_boost(AXIS_Z, PERIODS_M[0] / R_M) * body;
        assert!(
            torus
                .shift_frame(body, [-PERIODS_M[0], 0.0])
                .abs_diff_eq(boosted, 1e-9)
        );
    }
}
