//! `city_wormholes` gives City Sphere Lunar Surface's thrown wormholes (`thrown_wormholes.rs`) in
//! its Euclidean sphere: `Q` lobs a mouth from the eye, falling outward, down to the floor below
//! whatever it hits, where it stands at eye height facing back the way it flew; once both of a
//! player's mouths are down they link, and a player or a shot that passes through one mouth's disc
//! comes out of the other, mirrored through it as Lunar's mouths carry rays.
//!
//! Interface: `fly_city_throw`, `land_city_mouth`, `get_mouth_frame`, `can_place_mouth`,
//! `find_crossing`, `carry_through`, `make_mouth_warps`, `MOUTH_RADIUS_M`.
//!
//! Example: `let flight = fly_city_throw(&world, eye, WormholeColor::Purple, &settings.wormholes);`
//!
//! Must know:
//! - A City Sphere `MouthPlace` keeps the mouth's centre in `u_m`, `v_m` and `h_m` (x, y and z in
//!   meters, not Fermi coordinates) and its heading about the local up in `yaw_rad`, so it travels
//!   in the same 16 bytes of a snapshot.

use glam::{DMat3, DMat4, DVec3, DVec4};
use warped_shared::gravity::FlatWarp;

use crate::city_world::{CityWorld, Shot, get_up};
use crate::settings::WormholeSettings;
use crate::thrown_wormholes::{Flight, MouthPlace, WormholeColor};

/// `MOUTH_RADIUS_M` is the radius of a mouth's disc, which a player's middle or a shot passes
/// through.
pub const MOUTH_RADIUS_M: f64 = 1.0;
const FLIGHT_STEP_S: f64 = 1.0 / 120.0;
const BACK_OFF_M: f64 = 0.05;
const MAX_DROP_M: f64 = 30.0;
const MIN_MOUTH_GAP_M: f64 = 3.0;
const MIN_WALL_GAP_M: f64 = 0.6;
const HELPER_SWITCH_ABS: f64 = 0.9;

/// `get_reference_axes` gives a right and a back axis across the local up at `centre`, the zero
/// heading for a mouth there (as `tools/make_map.py`'s `make_frame` does).
fn get_reference_axes(centre: DVec3) -> (DVec3, DVec3) {
    let up: DVec3 = get_up(centre);
    let helper: DVec3 = if up.y.abs() < HELPER_SWITCH_ABS {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let right: DVec3 = helper.cross(up).normalize();
    (right, right.cross(up))
}

/// `make_mouth_warps` gives both mouths of each linked pair of `pairs` as warps that pull walkers
/// (W-144, `CityWorld::mouths`), each disc's radius as its throat's.
///
/// Example: `world.mouths = make_mouth_warps(&[[purple, green]]);`
pub fn make_mouth_warps(pairs: &[[MouthPlace; 2]]) -> Vec<FlatWarp> {
    pairs
        .iter()
        .flatten()
        .map(|place| FlatWarp {
            centre_m: DVec3::new(place.u_m, place.v_m, place.h_m),
            radius_m: MOUTH_RADIUS_M,
            is_mouth: true,
        })
        .collect()
}

/// `get_mouth_frame` gives a mouth's frame: columns right, up (toward the sphere's centre), back
/// (the way the disc faces) and its centre.
///
/// Example: `let frame = get_mouth_frame(&place);`
pub fn get_mouth_frame(place: &MouthPlace) -> DMat4 {
    let centre: DVec3 = DVec3::new(place.u_m, place.v_m, place.h_m);
    let up: DVec3 = get_up(centre);
    let (right_0, back_0) = get_reference_axes(centre);
    let (sin, cos) = place.yaw_rad.sin_cos();
    let right: DVec3 = right_0 * cos - back_0 * sin;
    let back: DVec3 = right_0 * sin + back_0 * cos;
    DMat4::from_cols(
        right.extend(0.0),
        up.extend(0.0),
        back.extend(0.0),
        centre.extend(1.0),
    )
}

/// `land_city_mouth` gives the place of a mouth that came down on `floor` (a point, w 1) moving
/// along `velocity` (w 0): `hover_m` above the floor, its back facing back the way it flew.
///
/// Example: `let place = land_city_mouth(floor, velocity, 1.6);`
pub fn land_city_mouth(floor: DVec4, velocity: DVec4, hover_m: f64) -> MouthPlace {
    let floor: DVec3 = floor.truncate();
    let up: DVec3 = get_up(floor);
    let centre: DVec3 = floor + up * hover_m;
    let along: DVec3 = velocity.truncate() - up * velocity.truncate().dot(up);
    let back: DVec3 = (-along).normalize_or(get_reference_axes(centre).1);
    let (right_0, back_0) = get_reference_axes(centre);
    MouthPlace {
        u_m: centre.x,
        v_m: centre.y,
        h_m: centre.z,
        yaw_rad: back.dot(right_0).atan2(back.dot(back_0)),
    }
}

/// `find_first_hit` gives where a straight path from `from` along the unit `direction` first meets
/// the street, the glass (from either side, except in its holes), the core or a box within
/// `length_m`, if it does.
fn find_first_hit(
    world: &CityWorld,
    from: DVec3,
    direction: DVec3,
    length_m: f64,
) -> Option<DVec3> {
    let shot: Shot = world.trace_shot(from, direction, length_m, 0, &[], None);
    let end: DVec3 = shot.points[1];
    ((end - from).length() < length_m - BACK_OFF_M * BACK_OFF_M).then_some(end)
}

/// `fly_city_throw` traces a throw of the `color` mouth from `eye`, `settings.throw_up_rad` above
/// its crosshair at `settings.throw_speed_m_per_s`, falling outward at
/// `settings.gravity_m_per_s_per_s`, every `FLIGHT_STEP_S`, until it meets something or
/// `settings.max_flight_s` passes. From a hit it drops outward to the floor below (the street or
/// the glass level); that floor point, with the throw's velocity, is the landing.
///
/// Example: `let flight = fly_city_throw(&world, eye, WormholeColor::Purple, &settings.wormholes);`
pub fn fly_city_throw(
    world: &CityWorld,
    eye: DMat4,
    color: WormholeColor,
    settings: &WormholeSettings,
) -> Flight {
    let mut x: DVec3 = eye.w_axis.truncate();
    let (sin, cos) = settings.throw_up_rad.sin_cos();
    let mut v: DVec3 =
        (eye.y_axis.truncate() * sin - eye.z_axis.truncate() * cos) * settings.throw_speed_m_per_s;
    let mut flight: Flight = Flight {
        color,
        points: vec![x.extend(1.0)],
        landing: None,
    };
    let step_count: usize = (settings.max_flight_s / FLIGHT_STEP_S).ceil() as usize;
    for _ in 0..step_count {
        let step_m: f64 = v.length() * FLIGHT_STEP_S;
        if let Some(hit) = find_first_hit(world, x, v.normalize(), step_m) {
            let before: DVec3 = hit - v.normalize() * BACK_OFF_M;
            flight.landing = find_first_hit(world, before, -get_up(before), MAX_DROP_M)
                .map(|floor| (floor.extend(1.0), v.extend(0.0)));
            flight.points.push(hit.extend(1.0));
            return flight;
        }
        x += v * FLIGHT_STEP_S;
        v -= get_up(x) * settings.gravity_m_per_s_per_s * FLIGHT_STEP_S;
        flight.points.push(x.extend(1.0));
    }
    flight
}

/// `can_place_mouth` says whether a mouth may stand at `place`: at least `MIN_MOUTH_GAP_M` from
/// every mouth in `others`, and its centre at least `MIN_WALL_GAP_M` clear of every box.
///
/// Example: `if can_place_mouth(&world, &place, &others) { ... }`
pub fn can_place_mouth(world: &CityWorld, place: &MouthPlace, others: &[MouthPlace]) -> bool {
    let centre: DVec3 = get_mouth_frame(place).w_axis.truncate();
    let is_clear_of_mouths: bool = others.iter().all(|other| {
        (get_mouth_frame(other).w_axis.truncate() - centre).length() >= MIN_MOUTH_GAP_M
    });
    let is_clear_of_boxes: bool = world.boxes.iter().all(|b| {
        let local: DVec3 = b.to_local(centre);
        (local - local.clamp(-b.half_m, b.half_m)).length() >= MIN_WALL_GAP_M
    });
    is_clear_of_mouths && is_clear_of_boxes
}

/// `find_crossing` gives how far along the segment from `a` to `b` (0 to 1) it passes through the
/// disc of the mouth `frame` (`MOUTH_RADIUS_M` about its centre, across its back axis), if it does.
///
/// Example: `if let Some(t) = find_crossing(frame, before, after) { ... }`
pub fn find_crossing(frame: DMat4, a: DVec3, b: DVec3) -> Option<f64> {
    let (centre, back) = (frame.w_axis.truncate(), frame.z_axis.truncate());
    let (side_a, side_b) = ((a - centre).dot(back), (b - centre).dot(back));
    if side_a * side_b > 0.0 || side_a == side_b {
        return None;
    }
    let t: f64 = side_a / (side_a - side_b);
    ((a.lerp(b, t) - centre).length() <= MOUTH_RADIUS_M).then_some(t)
}

/// `carry_through` gives the rigid motion that takes what enters the mouth `from` out of its
/// partner `to`: into `from`'s frame, mirrored through the disc (its back axis reversed, as
/// Lunar's mouths mirror rays by diag(1, 1, -1)), then out through `to`'s frame. Points and
/// directions both go through it (w 1 and w 0).
///
/// Example: `let feet = carry_through(purple, green).transform_point3(feet);`
pub fn carry_through(from: DMat4, to: DMat4) -> DMat4 {
    let mirror: DMat4 = DMat4::from_mat3(DMat3::from_diagonal(DVec3::new(1.0, 1.0, -1.0)));
    to * mirror * from.inverse()
}

#[cfg(test)]
mod tests {
    //! Tests of City Sphere's thrown wormholes.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Mouth place | round trip through its frame; facing back the way it flew | `a_landed_mouth_faces_back_along_its_flight` |
    //! | Throw | from blue's spawn, lands on the street ahead | `a_throw_lands_on_the_street_ahead` |
    //! | Crossing | through the disc (crosses), beside it (no), not reaching it (no) | `crossings_are_found_only_through_the_disc` |
    //! | Carrying | a point in front of one mouth comes out in front of the other, the same distance | `carrying_mirrors_through_the_pair` |

    use super::*;
    use crate::city_world::CityPlayer;
    use crate::settings::ServerSettings;

    const EYE_M: f64 = 1.6;
    const TOLERANCE: f64 = 1e-9;

    #[test]
    fn a_landed_mouth_faces_back_along_its_flight() {
        let floor: DVec3 = DVec3::new(24.0, 0.0, 0.0);
        let velocity: DVec3 = DVec3::new(0.0, 0.0, -3.0);
        let place: MouthPlace = land_city_mouth(floor.extend(1.0), velocity.extend(0.0), EYE_M);
        let frame: DMat4 = get_mouth_frame(&place);
        assert!(
            (frame.w_axis.truncate() - DVec3::new(24.0 - EYE_M, 0.0, 0.0)).length() < TOLERANCE
        );
        assert!((frame.z_axis.truncate() - DVec3::Z).length() < TOLERANCE);
    }

    #[test]
    fn a_throw_lands_on_the_street_ahead() {
        let world: CityWorld = CityWorld::Builtin().expect("the built-in map");
        let mut player: CityPlayer = world.make_player(0, 0);
        player.pitch_rad = 0.0;
        let eye: DMat4 = player.get_eye_frame();
        let settings: WormholeSettings = ServerSettings::Builtin().wormholes;
        let flight: Flight = fly_city_throw(&world, eye, WormholeColor::Purple, &settings);
        let (floor, _) = flight.landing.expect("the throw lands");
        assert!(world.street_radius_m - floor.truncate().length() < BACK_OFF_M);
        let ahead: DVec3 = floor.truncate() - player.feet;
        assert!(ahead.dot(-player.body.z_axis) > 0.0, "{ahead:?}");
    }

    #[test]
    fn crossings_are_found_only_through_the_disc() {
        let frame: DMat4 = DMat4::IDENTITY;
        assert_eq!(
            find_crossing(frame, DVec3::new(0.2, 0.0, 1.0), DVec3::new(0.2, 0.0, -1.0)),
            Some(0.5)
        );
        assert_eq!(
            find_crossing(frame, DVec3::new(2.0, 0.0, 1.0), DVec3::new(2.0, 0.0, -1.0)),
            None
        );
        assert_eq!(
            find_crossing(frame, DVec3::new(0.0, 0.0, 2.0), DVec3::new(0.0, 0.0, 1.0)),
            None
        );
    }

    #[test]
    fn carrying_mirrors_through_the_pair() {
        let from: DMat4 = DMat4::IDENTITY;
        let to: DMat4 = DMat4::from_translation(DVec3::new(10.0, 0.0, 0.0));
        let carried: DVec3 = carry_through(from, to).transform_point3(DVec3::new(0.0, 0.0, -0.5));
        assert!((carried - DVec3::new(10.0, 0.0, 0.5)).length() < TOLERANCE);
    }
}
