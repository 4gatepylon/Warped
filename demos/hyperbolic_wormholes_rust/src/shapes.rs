//! `shapes.rs` is the smooth solids structures are made of: rounded boxes, rounded cylinders and
//! ellipsoids, each in its structure's local chart q = R p / p.w (meters; straight lines there are
//! geodesics, and distances there are at most the hyperbolic ones). Each shape gives a lower bound
//! on the distance from q to it, so a chord can be sphere-traced, as `render_voxel.metal` does.
//!
//! Interface: `Shape`, `ShapeKind`, `get_distance_m`, `find_hit_mu`.
//!
//! Example: `let (d_m, i) = get_distance_m(&shapes, DVec3::ZERO);`

use glam::DVec3;

use crate::map::MapShape;

const HIT_M: f64 = 0.002;
const MAX_STEPS: usize = 160;

/// `ShapeKind` is a shape's form; its value is the kind `render_voxel.metal` reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShapeKind {
    Box = 0,
    Cylinder = 1,
    Ellipsoid = 2,
}

/// `Shape` is one solid: its kind, its centre and half sizes in meters, how much its edges are
/// rounded, a cylinder's axis (0, 1 or 2, its radius the next axis's half size), and its material.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub kind: ShapeKind,
    pub centre: DVec3,
    pub half: DVec3,
    pub round_m: f64,
    pub axis: usize,
    pub material: u8,
}

impl Shape {
    /// `FromMap` reads one of a map structure's shapes.
    ///
    /// Example: `let shape = Shape::FromMap(&map.structures[0].shapes[0])?;`
    #[allow(non_snake_case)]
    pub fn FromMap(shape: &MapShape) -> Result<Shape, String> {
        let kind: ShapeKind = match shape.kind.as_str() {
            "box" => ShapeKind::Box,
            "cylinder" => ShapeKind::Cylinder,
            "ellipsoid" => ShapeKind::Ellipsoid,
            other => return Err(format!("shapes: unknown shape kind {other:?}")),
        };
        Ok(Shape {
            kind,
            centre: DVec3::from_array(shape.centre_m),
            half: DVec3::from_array(shape.half_m),
            round_m: shape.round_m.unwrap_or(0.0),
            axis: shape.axis.unwrap_or(1) as usize,
            material: shape.material,
        })
    }

    /// `get_distance_m` bounds the distance from local point q to the shape from below (negative
    /// inside).
    ///
    /// Example: `let d_m = shape.get_distance_m(DVec3::new(0.0, 1.0, 0.0));`
    pub fn get_distance_m(&self, q: DVec3) -> f64 {
        let p: DVec3 = q - self.centre;
        match self.kind {
            ShapeKind::Box => {
                let d: DVec3 = p.abs() - (self.half - self.round_m);
                d.max(DVec3::ZERO).length() + d.max_element().min(0.0) - self.round_m
            }
            ShapeKind::Cylinder => {
                let (a, b, c) = (self.axis, (self.axis + 1) % 3, (self.axis + 2) % 3);
                let radial: f64 = p[b].hypot(p[c]) - (self.half[b] - self.round_m);
                let along: f64 = p[a].abs() - (self.half[a] - self.round_m);
                radial.max(0.0).hypot(along.max(0.0)) + radial.max(along).min(0.0) - self.round_m
            }
            ShapeKind::Ellipsoid => ((p / self.half).length() - 1.0) * self.half.min_element(),
        }
    }
}

/// `get_distance_m` gives the smallest of the shapes' distance bounds at q and which shape it is.
///
/// Example: `let (d_m, i) = get_distance_m(&shapes, q);`
pub fn get_distance_m(shapes: &[Shape], q: DVec3) -> (f64, usize) {
    get_solid_distance_m(shapes, q, None)
}

/// `get_solid_distance_m` is `get_distance_m` over the shapes not made of `see_through_material`
/// (every shape for `None`); it gives `f64::MAX` when no shape is left.
///
/// Example: `let (d_m, i) = get_solid_distance_m(&shapes, q, Some(glass));`
fn get_solid_distance_m(
    shapes: &[Shape],
    q: DVec3,
    see_through_material: Option<u8>,
) -> (f64, usize) {
    shapes
        .iter()
        .enumerate()
        .filter(|(_, shape)| Some(shape.material) != see_through_material)
        .map(|(i, shape)| (shape.get_distance_m(q), i))
        .fold(
            (f64::MAX, 0),
            |best, next| if next.0 < best.0 { next } else { best },
        )
}

/// `find_hit_mu` sphere-traces the line q0 + dq mu over [mu_lo, mu_hi] and gives where it first
/// meets a shape and which, or `None`. The line passes through shapes made of
/// `see_through_material` (glass, for a laser); with `None` every shape stops it.
///
/// Example: `let hit = find_hit_mu(&shapes, q0, dq, 0.0, 1.0, None);`
pub fn find_hit_mu(
    shapes: &[Shape],
    q0: DVec3,
    dq: DVec3,
    mu_lo: f64,
    mu_hi: f64,
    see_through_material: Option<u8>,
) -> Option<(f64, usize)> {
    let speed: f64 = dq.length();
    if speed <= 0.0 || shapes.is_empty() {
        return None;
    }
    let mut mu: f64 = mu_lo;
    for _ in 0..MAX_STEPS {
        let (d_m, i) = get_solid_distance_m(shapes, q0 + dq * mu, see_through_material);
        if d_m < HIT_M {
            return Some((mu, i));
        }
        mu += d_m / speed;
        if mu > mu_hi {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    //! Tests of the shapes' distance bounds and of tracing a line to them.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Line and a solid shape | meets it, passes it by | `a_cylinder_is_hit_at_its_side` |
    //! | See-through material | none (a pane stops the line), a pane's (passed, stops on the shape behind), a pane's with nothing behind (no hit) | `a_line_passes_through_a_see_through_pane` |
    //! | Shape kind for the distance bound | box, cylinder, ellipsoid | `distances_never_overshoot` |

    use super::*;

    const PILLAR: Shape = Shape {
        kind: ShapeKind::Cylinder,
        centre: DVec3::new(0.0, 1.0, 0.0),
        half: DVec3::new(0.5, 1.0, 0.5),
        round_m: 0.1,
        axis: 1,
        material: 1,
    };
    const PANE_MATERIAL: u8 = 2;
    const PANE: Shape = Shape {
        kind: ShapeKind::Box,
        centre: DVec3::new(-1.5, 1.0, 0.0),
        half: DVec3::new(0.1, 1.0, 1.0),
        round_m: 0.05,
        axis: 1,
        material: PANE_MATERIAL,
    };
    const START: DVec3 = DVec3::new(-3.0, 1.0, 0.0);
    const MAX_MU: f64 = 10.0;
    const PANE_FACE_MU: f64 = 1.4;
    const PILLAR_SIDE_MU: f64 = 2.5;
    const MU_TOLERANCE: f64 = 0.01;

    #[test]
    fn a_cylinder_is_hit_at_its_side() {
        let (mu, _) = find_hit_mu(&[PILLAR], START, DVec3::X, 0.0, MAX_MU, None).expect("a hit");
        assert!((mu - PILLAR_SIDE_MU).abs() < MU_TOLERANCE, "hit at {mu}");
        assert!(
            find_hit_mu(
                &[PILLAR],
                DVec3::new(-3.0, 2.5, 0.0),
                DVec3::X,
                0.0,
                MAX_MU,
                None
            )
            .is_none()
        );
    }

    /// `a_line_passes_through_a_see_through_pane` traces a line through a pane in front of a pillar:
    /// it stops on the pane, unless the pane's material is see-through, when it stops on the pillar
    /// behind, or on nothing without the pillar.
    #[test]
    fn a_line_passes_through_a_see_through_pane() {
        let trace = |shapes: &[Shape], see_through: Option<u8>| {
            find_hit_mu(shapes, START, DVec3::X, 0.0, MAX_MU, see_through)
        };
        let (mu, i) = trace(&[PANE, PILLAR], None).expect("a hit on the pane");
        assert!(
            (mu - PANE_FACE_MU).abs() < MU_TOLERANCE && i == 0,
            "hit {i} at {mu}"
        );
        let (mu, i) = trace(&[PANE, PILLAR], Some(PANE_MATERIAL)).expect("a hit on the pillar");
        assert!(
            (mu - PILLAR_SIDE_MU).abs() < MU_TOLERANCE && i == 1,
            "hit {i} at {mu}"
        );
        assert!(trace(&[PANE], Some(PANE_MATERIAL)).is_none());
    }

    #[test]
    fn distances_never_overshoot() {
        for shape in [
            PILLAR,
            Shape {
                kind: ShapeKind::Box,
                ..PILLAR
            },
            Shape {
                kind: ShapeKind::Ellipsoid,
                ..PILLAR
            },
        ] {
            for i in 0..200 {
                let t: f64 = f64::from(i) * 0.37;
                let q: DVec3 =
                    DVec3::new(t.sin() * 3.0, t.cos() * 2.0 + 1.0, (t * 1.7).sin() * 3.0);
                let d_m: f64 = shape.get_distance_m(q);
                let step: DVec3 = (shape.centre - q).normalize() * d_m.max(0.0);
                assert!(
                    shape.get_distance_m(q + step) >= -1e-9,
                    "{:?} overshoots at {q}",
                    shape.kind
                );
            }
        }
    }
}
