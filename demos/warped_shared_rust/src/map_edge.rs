//! `map_edge.rs` is the soft edge every level shares (W-145, no walls at map edges): no wall stops
//! a player at the end of a map; past the playable area each step outward is cut more and more,
//! so the player slows without end and never quite leaves ("Zeno's paradox"). An edge fraction
//! runs from 0 where the slowing starts to 1 where speed would reach 0, and a step that would end
//! further into the edge is cut by (1 − the edge fraction where it starts). Steps inward or along
//! the edge keep full speed, so nobody is stuck.
//!
//! - On a map with a crater (Lunar Surface, Dust) the edge is up the crater's wall, by the server
//!   settings' `map_edge` (`get_edge_frac` of how far up the wall).
//! - On a flat map (Snow Planet, Dark Forest) it is the band `soft_edge_m` wide outside the map's
//!   `regions.boundary`, an ellipse or a rectangle in Fermi (u, v) (`SoftEdge`).
//!
//! Interface: `get_edge_frac`, `take_step`, `SoftEdge`.
//!
//! Example:
//!     let edge_frac: f64 = soft_edge.get_edge_frac(u_m, v_m);
//!     let body_new = take_step(distance_m, edge_frac, move_m, |body| world.get_edge_frac(body));
//!
//! Must know: with each step shorter than the band, the share of the band left shrinks by at most
//! that step over the band's width, so the edge fraction never reaches 1; once rounding would
//! make it 1 (after about half a minute walking straight out), `take_step` stays put
//! (`take_step`'s tests).

/// `ELLIPSE_KIND` and `RECTANGLE_KIND` are the `regions.boundary.kind`s a soft edge follows.
const ELLIPSE_KIND: &str = "fermi_ellipse";
const RECTANGLE_KIND: &str = "fermi_rectangle";

/// `get_edge_frac` gives how far into the edge a point `past_m` beyond the slowing's start is, for
/// an edge whose speed would reach 0 `width_m` beyond it: 0 at or before the start, 1 at or past
/// `width_m`. Both are in one unit (meters, or a share of a crater's wall).
///
/// Example: `let edge_frac: f64 = get_edge_frac(wall_frac - start_wall_frac, stop - start);`
pub fn get_edge_frac(past_m: f64, width_m: f64) -> f64 {
    (past_m / width_m).clamp(0.0, 1.0)
}

/// `take_step` moves a body `step` (a length, or any amount `move_by` takes) from where its edge
/// fraction is `edge_frac`: `move_by(step)`, unless that ends further into the edge, which then
/// gives `move_by(step × (1 − edge_frac))`, or `move_by(0)` if even that reaches the far side
/// (which only rounding allows, after half a minute walking out). `get_edge_frac_of` gives the
/// edge fraction where a moved body is.
///
/// Example:
///     let body_new: DMat4 = take_step(distance_m, edge_frac, move_m, |b| world.get_edge_frac(b.col(AXIS_W)));
pub fn take_step<T>(
    step: f64,
    edge_frac: f64,
    move_by: impl Fn(f64) -> T,
    get_edge_frac_of: impl Fn(&T) -> f64,
) -> T {
    let moved: T = move_by(step);
    if get_edge_frac_of(&moved) <= edge_frac {
        return moved;
    }
    let cut: T = move_by(step * (1.0 - edge_frac));
    if get_edge_frac_of(&cut) >= 1.0 {
        move_by(0.0)
    } else {
        cut
    }
}

/// `PlayableShape` is the shape of a flat map's playable area in Fermi (u, v).
#[derive(Clone, Copy, PartialEq, Debug)]
enum PlayableShape {
    Ellipse,
    Rectangle,
}

/// `SoftEdge` is a flat map's soft edge: the playable area (`u_m` and `v_m` are its half sizes
/// along u and v) and the width of the band outside it where players slow.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SoftEdge {
    shape: PlayableShape,
    u_m: f64,
    v_m: f64,
    width_m: f64,
}

impl SoftEdge {
    /// `FromBoundary` makes the soft edge of a map's `regions.boundary` of `kind`
    /// (`fermi_ellipse` or `fermi_rectangle`) with half sizes `u_m` and `v_m` and `soft_edge_m`.
    /// It panics on another kind or a size that isn't positive.
    ///
    /// Example: `let edge = SoftEdge::FromBoundary("fermi_ellipse", 60.0, 40.0, 12.0);`
    #[allow(non_snake_case)]
    pub fn FromBoundary(kind: &str, u_m: f64, v_m: f64, soft_edge_m: f64) -> SoftEdge {
        let shape: PlayableShape = match kind {
            ELLIPSE_KIND => PlayableShape::Ellipse,
            RECTANGLE_KIND => PlayableShape::Rectangle,
            _ => panic!(
                "a soft edge needs a {ELLIPSE_KIND} or {RECTANGLE_KIND} boundary, not {kind}"
            ),
        };
        assert!(
            u_m > 0.0 && v_m > 0.0 && soft_edge_m > 0.0,
            "a soft edge needs positive sizes, not u {u_m}, v {v_m} and width {soft_edge_m}"
        );
        SoftEdge {
            shape,
            u_m,
            v_m,
            width_m: soft_edge_m,
        }
    }

    /// `get_width_m` gives the band's width: how far past the playable area speed would reach 0.
    ///
    /// Example: `let reach_m: f64 = edge.get_width_m();`
    pub fn get_width_m(&self) -> f64 {
        self.width_m
    }

    /// `get_past_m` gives how far outside the playable area the point at Fermi (u, v) is, 0
    /// inside. Past a rectangle it is the distance to it; past an ellipse it is the distance to
    /// first order (its excess over 1 of (u/a)² + (v/b)², rooted, over that root's gradient), exact
    /// on the axes and growing evenly along every line from the middle.
    ///
    /// Example: `let past_m: f64 = edge.get_past_m(65.0, 0.0);`
    pub fn get_past_m(&self, u_m: f64, v_m: f64) -> f64 {
        match self.shape {
            PlayableShape::Rectangle => (u_m.abs() - self.u_m)
                .max(0.0)
                .hypot((v_m.abs() - self.v_m).max(0.0)),
            PlayableShape::Ellipse => {
                let e: f64 = (u_m / self.u_m).hypot(v_m / self.v_m);
                if e <= 1.0 {
                    return 0.0;
                }
                let gradient: f64 =
                    (u_m / (self.u_m * self.u_m)).hypot(v_m / (self.v_m * self.v_m)) / e;
                (e - 1.0) / gradient
            }
        }
    }

    /// `get_edge_frac` gives the edge fraction at Fermi (u, v): 0 inside the playable area, 1 at
    /// the band's far side and beyond.
    ///
    /// Example: `let edge_frac: f64 = edge.get_edge_frac(u_m, v_m);`
    pub fn get_edge_frac(&self, u_m: f64, v_m: f64) -> f64 {
        get_edge_frac(self.get_past_m(u_m, v_m), self.width_m)
    }
}

/// `tests` checks the edge fraction, the shapes' distances and that steps outward never reach the
/// band's far side.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | where a point is, against the band | before it, inside it, past its far side | `test_edge_frac_runs_from_0_to_1` |
/// | playable shape | ellipse (on each axis, off the axes), rectangle (beside a side, past a corner), inside each | `test_ellipse_distance`, `test_rectangle_distance` |
/// | boundary kind | ellipse, rectangle, another (panics) | `test_ellipse_distance`, `test_rectangle_distance`, `test_another_kind_panics` |
/// | step direction | outward (cut), inward (full), along the edge (full) | `test_steps_outward_are_cut_and_others_kept` |
/// | walking straight out | for 10 minutes at 1/60 s at sprint speed, from inside, onto ellipse and rectangle | `test_walking_out_never_reaches_the_far_side` |
/// | walking back in | from deep in the band | `test_walking_back_in_keeps_full_speed` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `DT_S` is a frame at 60 frames a second, and `SPRINT_M_PER_S` a fast sprint.
    const DT_S: f64 = 1.0 / 60.0;
    const SPRINT_M_PER_S: f64 = 15.0;
    /// `TOLERANCE_M` is how closely distances are checked.
    const TOLERANCE_M: f64 = 1e-9;

    /// `walk_s` walks a point at Fermi `from` along `direction` at `speed_m_per_s` for `seconds`,
    /// stepping through `edge` frame by frame, and gives where it ends.
    fn walk_s(
        edge: &SoftEdge,
        from: [f64; 2],
        direction: [f64; 2],
        speed_m_per_s: f64,
        seconds: f64,
    ) -> [f64; 2] {
        let mut at: [f64; 2] = from;
        for _ in 0..(seconds / DT_S).round() as usize {
            let edge_frac: f64 = edge.get_edge_frac(at[0], at[1]);
            let move_by =
                |step_m: f64| [at[0] + direction[0] * step_m, at[1] + direction[1] * step_m];
            at = take_step(speed_m_per_s * DT_S, edge_frac, move_by, |p| {
                edge.get_edge_frac(p[0], p[1])
            });
        }
        at
    }

    #[test]
    fn test_edge_frac_runs_from_0_to_1() {
        assert_eq!(get_edge_frac(-3.0, 12.0), 0.0);
        assert_eq!(get_edge_frac(0.0, 12.0), 0.0);
        assert_eq!(get_edge_frac(3.0, 12.0), 0.25);
        assert_eq!(get_edge_frac(12.0, 12.0), 1.0);
        assert_eq!(get_edge_frac(40.0, 12.0), 1.0);
    }

    #[test]
    fn test_ellipse_distance() {
        let edge: SoftEdge = SoftEdge::FromBoundary(ELLIPSE_KIND, 60.0, 40.0, 12.0);
        assert_eq!(edge.get_past_m(0.0, 0.0), 0.0);
        assert_eq!(edge.get_past_m(59.0, 0.0), 0.0);
        assert!((edge.get_past_m(66.0, 0.0) - 6.0).abs() < TOLERANCE_M);
        assert!((edge.get_past_m(-66.0, 0.0) - 6.0).abs() < TOLERANCE_M);
        assert!((edge.get_past_m(0.0, -43.0) - 3.0).abs() < TOLERANCE_M);
        let [u_m, v_m] = [60.0 * 0.6_f64, 40.0 * 0.8_f64];
        let (normal_u, normal_v) = (u_m / 3600.0, v_m / 1600.0);
        let normal_length: f64 = normal_u.hypot(normal_v);
        let out_m: f64 = 2.0;
        let past_m: f64 = edge.get_past_m(
            u_m + out_m * normal_u / normal_length,
            v_m + out_m * normal_v / normal_length,
        );
        assert!((past_m - out_m).abs() < 0.2, "{past_m}");
        assert!((edge.get_edge_frac(66.0, 0.0) - 0.5).abs() < TOLERANCE_M);
    }

    #[test]
    fn test_rectangle_distance() {
        let edge: SoftEdge = SoftEdge::FromBoundary(RECTANGLE_KIND, 56.0, 30.0, 12.0);
        assert_eq!(edge.get_past_m(55.0, -29.0), 0.0);
        assert!((edge.get_past_m(0.0, 33.0) - 3.0).abs() < TOLERANCE_M);
        assert!((edge.get_past_m(-60.0, 10.0) - 4.0).abs() < TOLERANCE_M);
        assert!((edge.get_past_m(59.0, -34.0) - 5.0).abs() < TOLERANCE_M);
        assert_eq!(edge.get_edge_frac(80.0, 0.0), 1.0);
    }

    #[test]
    #[should_panic(expected = "fermi_ellipse or fermi_rectangle")]
    fn test_another_kind_panics() {
        SoftEdge::FromBoundary("fermi_circle", 1.0, 1.0, 1.0);
    }

    #[test]
    fn test_steps_outward_are_cut_and_others_kept() {
        let edge: SoftEdge = SoftEdge::FromBoundary(RECTANGLE_KIND, 10.0, 10.0, 4.0);
        let from: [f64; 2] = [12.0, 0.0];
        let edge_frac: f64 = edge.get_edge_frac(from[0], from[1]);
        let step = |direction: [f64; 2]| {
            take_step(
                1.0,
                edge_frac,
                |step_m: f64| {
                    [
                        from[0] + direction[0] * step_m,
                        from[1] + direction[1] * step_m,
                    ]
                },
                |p| edge.get_edge_frac(p[0], p[1]),
            )
        };
        assert_eq!(step([1.0, 0.0]), [12.5, 0.0]);
        assert_eq!(step([-1.0, 0.0]), [11.0, 0.0]);
        assert_eq!(step([0.0, 1.0]), [12.0, 1.0]);
    }

    #[test]
    fn test_walking_out_never_reaches_the_far_side() {
        let shapes: [(&str, [f64; 2]); 3] = [
            (ELLIPSE_KIND, [1.0, 0.0]),
            (ELLIPSE_KIND, [0.6, 0.8]),
            (RECTANGLE_KIND, [0.6, 0.8]),
        ];
        for (kind, direction) in shapes {
            let edge: SoftEdge = SoftEdge::FromBoundary(kind, 60.0, 40.0, 12.0);
            let mut last_past_m: f64 = 0.0;
            for seconds in [1.0, 2.0, 5.0, 60.0, 600.0] {
                let at: [f64; 2] = walk_s(&edge, [0.0, 0.0], direction, SPRINT_M_PER_S, seconds);
                let past_m: f64 = edge.get_past_m(at[0], at[1]);
                assert!(
                    edge.get_edge_frac(at[0], at[1]) < 1.0,
                    "{kind} after {seconds} s"
                );
                assert!(past_m >= last_past_m, "{kind} after {seconds} s");
                last_past_m = past_m;
            }
            assert!(last_past_m > 11.0, "{kind}: {last_past_m}");
        }
    }

    #[test]
    fn test_walking_back_in_keeps_full_speed() {
        let edge: SoftEdge = SoftEdge::FromBoundary(ELLIPSE_KIND, 60.0, 40.0, 12.0);
        let at: [f64; 2] = walk_s(&edge, [71.0, 0.0], [-1.0, 0.0], SPRINT_M_PER_S, 1.0);
        assert!((at[0] - (71.0 - SPRINT_M_PER_S)).abs() < 1e-6, "{at:?}");
    }
}
