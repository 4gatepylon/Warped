//! `console_reach.rs` says who may hold `E` on a console, as the rules say (W-126): a player whose
//! eye is within the settings' `consoles.reach_m` of the console and who aims at it. The game server
//! checks every hold with it, so no window or bot turns a console off from afar, and the console
//! bots walk until it holds.
//!
//! Interface: `ConsolePlace` (`New`, `is_held_from`, `get_foot`, `get_aim_point`),
//! `find_console_places`, `REACH_TOLERANCE_M`.
//!
//! Example: `let places = console_reach::find_console_places(&map)?;`
//!
//! Must know:
//! - Gotchas: the places come from the map as the game loads it (its curvature scaled by the
//!   server settings), so the server and its players must load it alike.

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{AXIS_W, AXIS_Z, invert_lorentz};
use crate::map::{Map, get_console_team, to_frame};
use crate::shapes::{Shape, find_hit_mu};

/// `REACH_TOLERANCE_M` is how much the server grows both the reach and the console's shape: the
/// pylon's mesh is drawn inside the shape, and a window's eye moves a little between the frame it
/// aimed in and the state the server reads.
pub const REACH_TOLERANCE_M: f64 = 0.5;
const SHAPE_KIND_CYLINDER: &str = "cylinder";

/// `ConsolePlace` is where a console is: its team (an index into `TEAM_NAMES`), the frame of its
/// pylon's structure, and its shape in that frame's chart (meters), on a map of curvature radius
/// `r_m`.
#[derive(Clone, Copy, Debug)]
pub struct ConsolePlace {
    pub team: u32,
    pub frame: DMat4,
    inverse: DMat4,
    shape: Shape,
    r_m: f64,
}

impl ConsolePlace {
    /// `New` makes the place of a console of `team` whose shape is `shape` in the chart of `frame`,
    /// on a map of curvature radius `r_m`.
    ///
    /// Example: `let place = ConsolePlace::New(0, DMat4::IDENTITY, shape, 40.0);`
    #[allow(non_snake_case)]
    pub fn New(team: u32, frame: DMat4, shape: Shape, r_m: f64) -> ConsolePlace {
        ConsolePlace {
            team,
            frame,
            inverse: invert_lorentz(frame),
            shape,
            r_m,
        }
    }

    /// `get_chart_point` gives point x in the console's chart, in meters.
    ///
    /// Example: `let q = place.get_chart_point(eye.col(AXIS_W));`
    fn get_chart_point(&self, x: DVec4) -> DVec3 {
        let p: DVec4 = self.inverse * x;
        p.truncate() / p.w * self.r_m
    }

    /// `is_held_from` says whether a player whose eye frame is `eye` reaches the console: the
    /// geodesic along the eye's view meets the console's shape within `reach_m`, both grown by
    /// `REACH_TOLERANCE_M`. Straight lines in the chart are geodesics, so the test is a line
    /// against the shape there.
    ///
    /// Example: `if place.is_held_from(player.frame, settings.consoles.reach_m) { ... }`
    pub fn is_held_from(&self, eye: DMat4, reach_m: f64) -> bool {
        let (x, v) = (eye.col(AXIS_W), -eye.col(AXIS_Z));
        let s: f64 = (reach_m + REACH_TOLERANCE_M) / self.r_m;
        let end: DVec4 = s.cosh() * x + s.sinh() * v;
        let (q0, q1) = (self.get_chart_point(x), self.get_chart_point(end));
        let grown: Shape = Shape {
            half: self.shape.half + DVec3::splat(REACH_TOLERANCE_M),
            ..self.shape
        };
        find_hit_mu(&[grown], q0, q1 - q0, 0.0, 1.0, None).is_some()
    }

    /// `get_foot` gives the point of the pylon's frame, on the ground at the pylon's foot.
    ///
    /// Example: `let goal = place.get_foot();`
    pub fn get_foot(&self) -> DVec4 {
        self.frame.col(AXIS_W)
    }

    /// `get_aim_point` gives the point a player at `eye_point` aims at to hold the console: on its
    /// shape's axis, level with the eye as far as the shape reaches.
    ///
    /// Example: `let aim = place.get_aim_point(camera.col(AXIS_W));`
    pub fn get_aim_point(&self, eye_point: DVec4) -> DVec4 {
        let axis: usize = self.shape.axis;
        let mut q: DVec3 = self.shape.centre;
        q[axis] = self.get_chart_point(eye_point)[axis].clamp(
            self.shape.centre[axis] - self.shape.half[axis],
            self.shape.centre[axis] + self.shape.half[axis],
        );
        let k: DVec3 = q / self.r_m;
        let f: f64 = 1.0 / (1.0 - k.length_squared()).sqrt();
        self.frame * (k * f).extend(f)
    }
}

/// `find_console_places` gives the place of each of a map's consoles, in the game server's order
/// (the map's console materials in material order, as `get_console_team` finds them): the one
/// shape of each console's material and its structure's frame. It fails when a console has no
/// shape or more than one, or a shape that isn't a cylinder.
///
/// Example: `let places = find_console_places(&map)?;`
pub fn find_console_places(map: &Map) -> Result<Vec<ConsolePlace>, String> {
    let r_m: f64 = map.get_curvature_radius_m();
    let mut places: Vec<ConsolePlace> = Vec::new();
    for material in &map.materials {
        let Some(team) = get_console_team(material) else {
            continue;
        };
        let found: Vec<(DMat4, Shape)> = map
            .structures
            .iter()
            .flat_map(|structure| {
                structure
                    .shapes
                    .iter()
                    .filter(|shape| shape.material == material.id)
                    .map(|shape| (to_frame(&structure.frame), shape))
            })
            .map(|(frame, shape)| {
                if shape.kind != SHAPE_KIND_CYLINDER {
                    return Err(format!(
                        "map {}: console {:?} is a {}, not a {SHAPE_KIND_CYLINDER}",
                        map.name, material.name, shape.kind
                    ));
                }
                Ok((frame, Shape::FromMap(shape)?))
            })
            .collect::<Result<Vec<(DMat4, Shape)>, String>>()?;
        let [(frame, shape)] = found[..] else {
            return Err(format!(
                "map {}: console {:?} has {} shapes, not one",
                map.name,
                material.name,
                found.len()
            ));
        };
        places.push(ConsolePlace::New(team, frame, shape, r_m));
    }
    Ok(places)
}

#[cfg(test)]
mod tests {
    //! Tests of who reaches a console.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Distance from the eye to the console's side | within reach, within the tolerance past it, past the tolerance (too far) | `a_console_is_held_only_within_reach` |
    //! | Where the eye looks | at the console, away from it (not aiming), above it | `a_console_is_held_only_when_aimed_at` |
    //! | Where a bot aims | level with its eye, from below the console's top, from above it | `a_bot_aiming_at_the_aim_point_holds_the_console` |
    //! | Map | every built-in hyperbolic map: one place per console, of its team | `every_built_in_console_has_a_place` |

    use clap::ValueEnum;
    use glam::{DMat4, DVec3, DVec4};

    use super::*;
    use crate::geometry::{
        AXIS_X, AXIS_Y, get_local_direction, make_boost, make_rotation, normalize_tangent,
    };
    use crate::map::MapName;
    use crate::shapes::ShapeKind;

    const R_M: f64 = 26.7;
    const REACH_M: f64 = 3.0;
    const PYLON_RADIUS_M: f64 = 1.35;
    const PYLON_CENTRE_M: DVec3 = DVec3::new(0.0, 5.654, 0.0);
    const PYLON_HALF_M: DVec3 = DVec3::new(PYLON_RADIUS_M, 6.346, PYLON_RADIUS_M);
    const EYE_HEIGHT_M: f64 = 1.6;
    const SMALL_M: f64 = 0.1;
    const POINT_TOLERANCE: f64 = 1e-9;

    /// `make_pylon` gives a console of team 0 shaped like the maps' pylons, at the origin.
    fn make_pylon() -> ConsolePlace {
        let shape: Shape = Shape {
            kind: ShapeKind::Cylinder,
            centre: PYLON_CENTRE_M,
            half: PYLON_HALF_M,
            round_m: 0.0,
            axis: 1,
            material: 0,
        };
        ConsolePlace::New(0, DMat4::IDENTITY, shape, R_M)
    }

    /// `make_eye` gives the eye frame of a player `gap_m` from the pylon's side, at eye height,
    /// looking at the pylon's axis and then turned `turn_rad` to their left.
    fn make_eye(gap_m: f64, turn_rad: f64) -> DMat4 {
        make_boost(AXIS_Z, (PYLON_RADIUS_M + gap_m) / R_M)
            * make_boost(AXIS_Y, EYE_HEIGHT_M / R_M)
            * make_rotation(AXIS_X, AXIS_Z, turn_rad)
    }

    #[test]
    fn a_console_is_held_only_within_reach() {
        let pylon: ConsolePlace = make_pylon();
        assert!(pylon.is_held_from(make_eye(REACH_M - SMALL_M, 0.0), REACH_M));
        assert!(pylon.is_held_from(make_eye(REACH_M + REACH_TOLERANCE_M, 0.0), REACH_M));
        assert!(!pylon.is_held_from(
            make_eye(REACH_M + 2.0 * REACH_TOLERANCE_M + SMALL_M, 0.0),
            REACH_M
        ));
    }

    #[test]
    fn a_console_is_held_only_when_aimed_at() {
        let pylon: ConsolePlace = make_pylon();
        assert!(pylon.is_held_from(make_eye(1.0, 0.0), REACH_M));
        assert!(!pylon.is_held_from(make_eye(1.0, std::f64::consts::PI), REACH_M));
        assert!(!pylon.is_held_from(make_eye(1.0, std::f64::consts::FRAC_PI_2), REACH_M));
        let looking_up: DMat4 = make_eye(1.0, 0.0) * make_rotation(AXIS_Z, AXIS_Y, -1.5);
        assert!(!pylon.is_held_from(looking_up, REACH_M));
    }

    #[test]
    fn a_bot_aiming_at_the_aim_point_holds_the_console() {
        let pylon: ConsolePlace = make_pylon();
        for (gap_m, rise_m) in [(2.0, 0.0), (2.0, 10.0), (1.0, 13.0)] {
            let eye: DMat4 = make_eye(gap_m, 0.3) * make_boost(AXIS_Y, rise_m / R_M);
            let aim: DVec4 = pylon.get_aim_point(eye.col(AXIS_W));
            let toward: DVec3 =
                get_local_direction(eye, normalize_tangent(eye.col(AXIS_W), aim)).normalize();
            let pitch_rad: f64 = toward.y.asin();
            let yaw_rad: f64 = toward.x.atan2(-toward.z);
            let aimed: DMat4 = eye
                * make_rotation(AXIS_X, AXIS_Z, yaw_rad)
                * make_rotation(AXIS_Z, AXIS_Y, -pitch_rad);
            assert!(
                pylon.is_held_from(aimed, REACH_M),
                "gap {gap_m}, rise {rise_m}"
            );
        }
    }

    #[test]
    fn every_built_in_console_has_a_place() {
        for name in MapName::value_variants() {
            let map: Map = Map::Builtin(name.get_folder_name()).expect("a built-in map");
            if map.space.curvature_radius_m.is_none() {
                continue;
            }
            let places: Vec<ConsolePlace> = find_console_places(&map).expect("console places");
            let teams: Vec<u32> = map.materials.iter().filter_map(get_console_team).collect();
            assert_eq!(places.iter().map(|p| p.team).collect::<Vec<u32>>(), teams);
            for place in places {
                let foot: DVec4 = place.get_foot();
                let on_hyperboloid: f64 = foot.truncate().length_squared() - foot.w * foot.w;
                assert!((on_hyperboloid + 1.0).abs() < POINT_TOLERANCE);
            }
        }
    }
}
