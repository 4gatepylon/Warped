//! `collision` holds axis-aligned boxes: overlap tests, ray casts, and sliding a box through a
//! set of solid boxes with step-up.
//!
//! Example:
//!     let hit: Option<RayHit> = cast_ray(&solids, origin_m, direction, 100.0);

use glam::Vec3;

/// `STEP_HEIGHT_M` is the tallest ledge a mover climbs without jumping: Lunar Surface's
/// `MAX_WALK_RISE_M`.
pub const STEP_HEIGHT_M: f32 = 0.5;
/// `SKIN_M` is the gap kept between a mover and what it touches.
const SKIN_M: f32 = 0.001;

/// `Aabb` is an axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min_m: Vec3,
    pub max_m: Vec3,
}

/// `RayHit` is where a ray first hits a box.
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub distance_m: f32,
    pub normal: Vec3,
}

impl Aabb {
    /// `from_center` makes a box from its center and half size.
    ///
    /// Example:
    ///     let aabb: Aabb = Aabb::from_center(Vec3::ZERO, Vec3::ONE);
    pub fn from_center(center_m: Vec3, half_size_m: Vec3) -> Aabb {
        Aabb {
            min_m: center_m - half_size_m,
            max_m: center_m + half_size_m,
        }
    }

    /// `overlaps` says whether two boxes overlap (touching doesn't count).
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.min_m.cmplt(other.max_m).all() && other.min_m.cmplt(self.max_m).all()
    }

    /// `translated` returns the box moved by an offset.
    pub fn translated(&self, offset_m: Vec3) -> Aabb {
        Aabb {
            min_m: self.min_m + offset_m,
            max_m: self.max_m + offset_m,
        }
    }

    /// `center` returns the box's center.
    pub fn center(&self) -> Vec3 {
        (self.min_m + self.max_m) * 0.5
    }

    /// `cast_ray` returns where a ray from outside enters the box, if within `max_distance_m`.
    ///
    /// Example:
    ///     let hit: Option<RayHit> = aabb.cast_ray(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z, 10.0);
    pub fn cast_ray(&self, origin_m: Vec3, direction: Vec3, max_distance_m: f32) -> Option<RayHit> {
        let inverse: Vec3 = direction.recip();
        let t0: Vec3 = (self.min_m - origin_m) * inverse;
        let t1: Vec3 = (self.max_m - origin_m) * inverse;
        let near: Vec3 = t0.min(t1);
        let far: Vec3 = t0.max(t1);
        let entry_m: f32 = near.max_element();
        let exit_m: f32 = far.min_element();
        if entry_m > exit_m || exit_m < 0.0 || entry_m > max_distance_m || entry_m < 0.0 {
            return None;
        }
        let axis: usize = if near.x >= near.y && near.x >= near.z {
            0
        } else if near.y >= near.z {
            1
        } else {
            2
        };
        let mut normal: Vec3 = Vec3::ZERO;
        normal[axis] = -direction[axis].signum();
        Some(RayHit {
            distance_m: entry_m,
            normal,
        })
    }
}

/// `cast_ray` returns the nearest hit among many boxes, with the hit box's index.
///
/// Example:
///     let hit = cast_ray(&solids, eye_m, forward, 200.0);
pub fn cast_ray(
    solids: &[Aabb],
    origin_m: Vec3,
    direction: Vec3,
    max_distance_m: f32,
) -> Option<(usize, RayHit)> {
    let mut nearest: Option<(usize, RayHit)> = None;
    for (index, solid) in solids.iter().enumerate() {
        let limit_m: f32 = nearest.map_or(max_distance_m, |(_, hit)| hit.distance_m);
        if let Some(hit) = solid.cast_ray(origin_m, direction, limit_m) {
            nearest = Some((index, hit));
        }
    }
    nearest
}

/// `MoveResult` is where a moving box ended up and what it touched.
#[derive(Clone, Copy, Debug)]
pub struct MoveResult {
    pub position_m: Vec3,
    pub is_on_ground: bool,
    pub did_hit_ceiling: bool,
    pub did_hit_wall: bool,
}

/// `move_axis` moves a box along one axis as far as the solids allow.
fn move_axis(solids: &[Aabb], body: &Aabb, axis: usize, delta_m: f32) -> (f32, bool) {
    if delta_m == 0.0 {
        return (0.0, false);
    }
    let mut offset: Vec3 = Vec3::ZERO;
    offset[axis] = delta_m;
    let moved: Aabb = body.translated(offset);
    let mut allowed_m: f32 = delta_m;
    let mut did_hit: bool = false;
    for solid in solids {
        if !moved.overlaps(solid) {
            continue;
        }
        did_hit = true;
        let limit_m: f32 = if delta_m > 0.0 {
            (solid.min_m[axis] - body.max_m[axis] - SKIN_M).max(0.0)
        } else {
            (solid.max_m[axis] - body.min_m[axis] + SKIN_M).min(0.0)
        };
        if limit_m.abs() < allowed_m.abs() {
            allowed_m = limit_m;
        }
    }
    (allowed_m, did_hit)
}

/// `move_body` slides a box (given by its feet position and half extents) through the solids,
/// climbing ledges up to [`STEP_HEIGHT_M`] when `can_step` is set.
///
/// Example:
///     let result: MoveResult = move_body(&solids, feet_m, half_m, velocity_m_s * dt_s, true);
pub fn move_body(
    solids: &[Aabb],
    feet_m: Vec3,
    half_extent_m: Vec3,
    delta_m: Vec3,
    can_step: bool,
) -> MoveResult {
    let body_at = |feet: Vec3| Aabb {
        min_m: feet - Vec3::new(half_extent_m.x, 0.0, half_extent_m.z),
        max_m: feet + Vec3::new(half_extent_m.x, half_extent_m.y * 2.0, half_extent_m.z),
    };
    let mut position_m: Vec3 = feet_m;
    let mut did_hit_wall: bool = false;
    for axis in [0, 2] {
        let (allowed_m, did_hit) = move_axis(solids, &body_at(position_m), axis, delta_m[axis]);
        if did_hit && can_step && (allowed_m - delta_m[axis]).abs() > SKIN_M {
            let raised: Vec3 = position_m + Vec3::Y * STEP_HEIGHT_M;
            let (up_m, _) = move_axis(solids, &body_at(position_m), 1, STEP_HEIGHT_M);
            if (up_m - STEP_HEIGHT_M).abs() < SKIN_M {
                let (stepped_m, did_hit_again) =
                    move_axis(solids, &body_at(raised), axis, delta_m[axis]);
                if !did_hit_again {
                    let mut stepped: Vec3 = raised;
                    stepped[axis] += stepped_m;
                    let (down_m, _) =
                        move_axis(solids, &body_at(stepped), 1, -STEP_HEIGHT_M - SKIN_M);
                    position_m = stepped + Vec3::Y * down_m;
                    continue;
                }
            }
        }
        did_hit_wall |= did_hit;
        position_m[axis] += allowed_m;
    }
    let (allowed_y_m, did_hit_y) = move_axis(solids, &body_at(position_m), 1, delta_m.y);
    position_m.y += allowed_y_m;
    let (probe_m, is_touching) = move_axis(solids, &body_at(position_m), 1, -0.02);
    MoveResult {
        position_m,
        is_on_ground: is_touching && probe_m.abs() < 0.02 && delta_m.y <= 0.0,
        did_hit_ceiling: did_hit_y && delta_m.y > 0.0,
        did_hit_wall,
    }
}

/// `tests` checks ray casts and body movement against boxes.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | ray direction | toward the box | `test_ray_hits_box_face` |
/// | ray direction | away from the box | `test_ray_hits_box_face` |
/// | body motion | falling onto a floor | `test_body_lands_on_floor` |
/// | body motion | horizontal into a wall taller than a step | `test_body_stops_at_wall_and_climbs_step` |
/// | body motion | horizontal into a ledge lower than a step | `test_body_stops_at_wall_and_climbs_step` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `floor` is a big slab whose top is at y = 0.
    fn floor() -> Aabb {
        Aabb {
            min_m: Vec3::new(-50.0, -1.0, -50.0),
            max_m: Vec3::new(50.0, 0.0, 50.0),
        }
    }

    /// `test_ray_hits_box_face` checks the hit distance and normal.
    #[test]
    fn test_ray_hits_box_face() {
        let aabb: Aabb = Aabb::from_center(Vec3::ZERO, Vec3::ONE);
        let hit: RayHit = aabb
            .cast_ray(Vec3::new(0.0, 0.0, 5.0), Vec3::NEG_Z, 10.0)
            .unwrap();
        assert!((hit.distance_m - 4.0).abs() < 1e-5);
        assert_eq!(hit.normal, Vec3::Z);
        assert!(
            aabb.cast_ray(Vec3::new(0.0, 0.0, 5.0), Vec3::Z, 10.0)
                .is_none()
        );
    }

    /// `test_body_lands_on_floor` checks that falling stops on the floor.
    #[test]
    fn test_body_lands_on_floor() {
        let result: MoveResult = move_body(
            &[floor()],
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.3, 0.9, 0.3),
            Vec3::new(0.0, -1.0, 0.0),
            true,
        );
        assert!(result.position_m.y.abs() < 0.01);
        assert!(result.is_on_ground);
    }

    /// `test_body_stops_at_wall_and_climbs_step` checks walls block and low ledges are climbed.
    #[test]
    fn test_body_stops_at_wall_and_climbs_step() {
        let wall: Aabb = Aabb {
            min_m: Vec3::new(1.0, 0.0, -5.0),
            max_m: Vec3::new(2.0, 3.0, 5.0),
        };
        let step: Aabb = Aabb {
            min_m: Vec3::new(1.0, 0.0, -5.0),
            max_m: Vec3::new(2.0, 0.3, 5.0),
        };
        let half: Vec3 = Vec3::new(0.3, 0.9, 0.3);
        let blocked: MoveResult = move_body(
            &[floor(), wall],
            Vec3::ZERO,
            half,
            Vec3::new(2.0, 0.0, 0.0),
            true,
        );
        assert!(blocked.position_m.x < 0.71 && blocked.did_hit_wall);
        let climbed: MoveResult = move_body(
            &[floor(), step],
            Vec3::ZERO,
            half,
            Vec3::new(1.5, 0.0, 0.0),
            true,
        );
        assert!(climbed.position_m.x > 1.4 && (climbed.position_m.y - 0.3).abs() < 0.01);
    }
}
