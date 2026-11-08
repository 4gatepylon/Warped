//! `thrown_wormholes.rs` lets each player throw a linked pair of wormhole mouths, one purple and one
//! green: a throw is lobbed from the eye under its own gravity, drops to the floor below whatever it
//! hits, and the mouth then hovers there at eye height, facing back the way it flew. Once both of a
//! player's mouths are down they link. Both maps use it; each says what a throw hits and where a
//! mouth may go.
//!
//! Interface: `WormholeColor`, `PURPLE_COLOR`, `GREEN_COLOR`, `MouthPlace`, `Flight`, `fly_throw`.
//!
//! Example: `let flight = fly_throw(camera, WormholeColor::Purple, &settings.wormholes, r, find_hit);`
//!
//! Must know:
//! - Limitations: a throw flies straight through other warps' bending and throats.

#![allow(dead_code)]

use glam::{DMat4, DVec4};

use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, UP_VECTOR, invert_lorentz, make_boost, make_rotation,
    minkowski_dot, normalize_point, normalize_tangent,
};
use crate::settings::WormholeSettings;
use crate::terrain::get_fermi_coordinates;

pub const PURPLE_COLOR: [f32; 3] = [0.62, 0.22, 1.0];
pub const GREEN_COLOR: [f32; 3] = [0.2, 1.0, 0.4];
const FLIGHT_STEP_S: f64 = 1.0 / 120.0;
const BACK_OFF_M: f64 = 0.05;
const MAX_DROP_M: f64 = 200.0;

/// `WormholeColor` is which of a player's two mouths: the purple one is thrown first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WormholeColor {
    Purple,
    Green,
}

impl WormholeColor {
    /// `get_other` gives the other mouth's colour.
    ///
    /// Example: `let next = WormholeColor::Purple.get_other();`
    pub fn get_other(self) -> WormholeColor {
        match self {
            WormholeColor::Purple => WormholeColor::Green,
            WormholeColor::Green => WormholeColor::Purple,
        }
    }

    /// `get_name` gives the colour's name, for the HUD.
    ///
    /// Example: `let message = format!("{} wormhole placed", color.get_name());`
    pub fn get_name(self) -> &'static str {
        match self {
            WormholeColor::Purple => "Purple",
            WormholeColor::Green => "Green",
        }
    }

    /// `get_rgb` gives the colour itself, in linear RGB.
    ///
    /// Example: `let [r, g, b] = WormholeColor::Green.get_rgb();`
    pub fn get_rgb(self) -> [f32; 3] {
        match self {
            WormholeColor::Purple => PURPLE_COLOR,
            WormholeColor::Green => GREEN_COLOR,
        }
    }
}

/// `MouthPlace` is where a thrown mouth hovers: Fermi coordinates over the ground plane (u and v
/// along it, h above it, in meters, as `get_fermi_coordinates` gives them) and the heading its back
/// axis faces (`yaw_rad`), which is all a mouth needs and fits a snapshot in 16 bytes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MouthPlace {
    pub u_m: f64,
    pub v_m: f64,
    pub h_m: f64,
    pub yaw_rad: f64,
}

impl MouthPlace {
    /// `Landed` gives the place of a mouth that came down on floor point `x` moving along `velocity`
    /// (a tangent at `x`): `hover_m` above it, its back axis facing back the way it flew.
    ///
    /// Example: `let place = MouthPlace::Landed(x, velocity, 1.6, 40.0);`
    #[allow(non_snake_case)]
    pub fn Landed(x: DVec4, velocity: DVec4, hover_m: f64, r: f64) -> MouthPlace {
        let [u_m, v_m, h_m] = get_fermi_coordinates(x, r);
        let level: DVec4 =
            invert_lorentz(get_level_frame(u_m, v_m, 0.0, r) * get_rise(h_m, r)) * velocity;
        MouthPlace {
            u_m,
            v_m,
            h_m: h_m + hover_m,
            yaw_rad: level.x.atan2(-level.z),
        }
    }

    /// `get_frame` gives the mouth's frame: columns right, up, back and centre, so a player passes
    /// through it moving along -back.
    ///
    /// Example: `let frame = place.get_frame(40.0);`
    pub fn get_frame(&self, r: f64) -> DMat4 {
        get_level_frame(self.u_m, self.v_m, self.yaw_rad, r) * get_rise(self.h_m, r)
    }

    /// `get_centre` gives the mouth's centre.
    ///
    /// Example: `let x = place.get_centre(40.0);`
    pub fn get_centre(&self, r: f64) -> DVec4 {
        self.get_frame(r).col(AXIS_W)
    }
}

/// `get_level_frame` gives the ground frame at Fermi coordinates (u, v), turned by `yaw_rad`.
///
/// Example: `let frame = get_level_frame(-12.0, 3.0, 0.0, 40.0);`
fn get_level_frame(u_m: f64, v_m: f64, yaw_rad: f64, r: f64) -> DMat4 {
    make_boost(AXIS_Z, -u_m / r)
        * make_boost(AXIS_X, v_m / r)
        * make_rotation(AXIS_X, AXIS_Z, yaw_rad)
}

/// `get_rise` gives the boost `h_m` straight up.
///
/// Example: `let frame = get_level_frame(0.0, 0.0, 0.0, 40.0) * get_rise(1.6, 40.0);`
fn get_rise(h_m: f64, r: f64) -> DMat4 {
    make_boost(AXIS_Y, h_m / r)
}

/// `Flight` is one throw: its colour, where it was every `FLIGHT_STEP_S` from the hand, and the
/// floor point it came down on with its velocity there, or `None` if it never landed.
#[derive(Clone)]
pub struct Flight {
    pub color: WormholeColor,
    pub points: Vec<DVec4>,
    pub landing: Option<(DVec4, DVec4)>,
}

impl Flight {
    /// `get_duration_s` gives how long the throw is in the air.
    ///
    /// Example: `if elapsed_s >= flight.get_duration_s() { land(); }`
    pub fn get_duration_s(&self) -> f64 {
        (self.points.len() - 1) as f64 * FLIGHT_STEP_S
    }

    /// `get_position` gives where the throw is `elapsed_s` after it left the hand (its end, after).
    ///
    /// Example: `let x = flight.get_position(0.25);`
    pub fn get_position(&self, elapsed_s: f64) -> DVec4 {
        let last: usize = self.points.len() - 1;
        let step: usize = ((elapsed_s / FLIGHT_STEP_S) as usize).min(last);
        self.points[step]
    }
}

/// `fly_throw` traces a throw of the `color` mouth from `camera`'s eye, `settings.throw_up_rad`
/// above its crosshair at `settings.throw_speed_m_per_s`, falling at
/// `settings.gravity_m_per_s_per_s`, until `find_hit` (a point, a unit tangent and a length, in
/// curvature radii, to the first surface within that length) finds a surface, or for
/// `settings.max_flight_s`. From a hit it drops straight down to the floor below; the floor point is
/// the landing.
///
/// Example: `let flight = fly_throw(camera, WormholeColor::Purple, &settings.wormholes, 40.0, |x, v, s| world.find_hit_s(x, v, s));`
pub fn fly_throw(
    camera: DMat4,
    color: WormholeColor,
    settings: &WormholeSettings,
    r: f64,
    find_hit: impl Fn(DVec4, DVec4, f64) -> Option<f64>,
) -> Flight {
    let up_rad: f64 = settings.throw_up_rad;
    let mut x: DVec4 = camera.col(AXIS_W);
    let mut v: DVec4 =
        settings.throw_speed_m_per_s * (camera * DVec4::new(0.0, up_rad.sin(), -up_rad.cos(), 0.0));
    let mut flight: Flight = Flight {
        color,
        points: vec![x],
        landing: None,
    };
    let step_count: usize = (settings.max_flight_s / FLIGHT_STEP_S).ceil() as usize;
    for _ in 0..step_count {
        let speed_m_per_s: f64 = minkowski_dot(v, v).sqrt();
        let (t, s) = (v / speed_m_per_s, speed_m_per_s * FLIGHT_STEP_S / r);
        if let Some(s_hit) = find_hit(x, t, s) {
            let hit: DVec4 = s_hit.cosh() * x + s_hit.sinh() * t;
            let before: DVec4 = normalize_point(hit - BACK_OFF_M / r * normalize_tangent(hit, t));
            let down: DVec4 = -normalize_tangent(before, UP_VECTOR);
            flight.landing = find_hit(before, down, MAX_DROP_M / r).map(|s_drop| {
                let floor: DVec4 = s_drop.cosh() * before + s_drop.sinh() * down;
                (floor, speed_m_per_s * (s_hit.sinh() * x + s_hit.cosh() * t))
            });
            flight.points.push(hit);
            return flight;
        }
        let x_next: DVec4 = normalize_point(s.cosh() * x + s.sinh() * t);
        let carried: DVec4 = speed_m_per_s * (s.sinh() * x + s.cosh() * t);
        let fall: DVec4 =
            settings.gravity_m_per_s_per_s * FLIGHT_STEP_S * normalize_tangent(x_next, UP_VECTOR);
        let w: DVec4 = carried - fall;
        (x, v) = (x_next, w + minkowski_dot(w, x_next) * x_next);
        flight.points.push(x);
    }
    flight
}

#[cfg(test)]
mod tests {
    //! Tests of thrown wormholes on flat ground (the plane h = 0) at a curvature radius of 40 m.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Mouth place | round trip through its frame; facing back the way it flew | `a_place_survives_its_frame`, `a_landed_mouth_faces_back_along_its_flight` |
    //! | Throw | lands on the ground ahead; hits a wall and drops to the floor below it | `a_throw_lands_ahead_on_the_ground`, `a_throw_at_a_wall_drops_below_it` |

    use super::*;
    use crate::settings::ServerSettings;

    const R_M: f64 = 40.0;
    const EYE_M: f64 = 1.6;
    const WALL_M: f64 = 5.0;
    const TOLERANCE_M: f64 = 1e-6;
    const LANDING_TOLERANCE_M: f64 = 0.01;
    const MIN_RANGE_M: f64 = 10.0;

    /// `find_ground_hit` finds the ground plane h = 0 along a chord of length `s_max`.
    fn find_ground_hit(x: DVec4, v: DVec4, s_max: f64) -> Option<f64> {
        (x.y > 0.0 && v.y < 0.0 && x.y / -v.y < 1.0)
            .then(|| (x.y / -v.y).atanh())
            .filter(|&s| s <= s_max)
    }

    /// `find_wall_or_ground_hit` adds a wall, the plane u = `WALL_M`, to `find_ground_hit`.
    fn find_wall_or_ground_hit(x: DVec4, v: DVec4, s_max: f64) -> Option<f64> {
        let plane: DVec4 = make_boost(AXIS_Z, -WALL_M / R_M) * DVec4::Z;
        let (a, b) = (minkowski_dot(x, plane), minkowski_dot(v, plane));
        let wall: Option<f64> = (a > 0.0 && b < 0.0 && a / -b < 1.0)
            .then(|| (a / -b).atanh())
            .filter(|&s| s <= s_max);
        match (wall, find_ground_hit(x, v, s_max)) {
            (Some(w), Some(g)) => Some(w.min(g)),
            (w, g) => w.or(g),
        }
    }

    /// `get_eye` gives a camera at the origin's eye height looking along -z, pitched up by `pitch_rad`.
    fn get_eye(pitch_rad: f64) -> DMat4 {
        get_rise(EYE_M, R_M) * make_rotation(AXIS_Z, AXIS_Y, -pitch_rad)
    }

    /// `throw_from_eye` throws the `color` mouth from `get_eye(pitch_rad)` with the built-in settings.
    fn throw_from_eye(
        pitch_rad: f64,
        find_hit: impl Fn(DVec4, DVec4, f64) -> Option<f64>,
    ) -> Flight {
        let settings: WormholeSettings = ServerSettings::Builtin().wormholes;
        fly_throw(
            get_eye(pitch_rad),
            WormholeColor::Purple,
            &settings,
            R_M,
            find_hit,
        )
    }

    #[test]
    fn a_place_survives_its_frame() {
        let place: MouthPlace = MouthPlace {
            u_m: -12.0,
            v_m: 7.5,
            h_m: EYE_M,
            yaw_rad: 0.7,
        };
        let frame: DMat4 = place.get_frame(R_M);
        let [u_m, v_m, h_m] = get_fermi_coordinates(frame.col(AXIS_W), R_M);
        for (got, want) in [(u_m, place.u_m), (v_m, place.v_m), (h_m, place.h_m)] {
            assert!((got - want).abs() < TOLERANCE_M, "{got} vs {want}");
        }
        let again: MouthPlace = MouthPlace::Landed(frame.col(AXIS_W), -frame.col(AXIS_Z), 0.0, R_M);
        for (got, want) in [(again.h_m, place.h_m), (again.yaw_rad, place.yaw_rad)] {
            assert!((got - want).abs() < TOLERANCE_M, "{got} vs {want}");
        }
    }

    #[test]
    fn a_landed_mouth_faces_back_along_its_flight() {
        let flight: Flight = throw_from_eye(0.0, find_ground_hit);
        let (x, velocity) = flight.landing.expect("it lands");
        let frame: DMat4 = MouthPlace::Landed(x, velocity, EYE_M, R_M).get_frame(R_M);
        let back_at_landing: DVec4 = normalize_tangent(x, frame.col(AXIS_Z));
        assert!(minkowski_dot(back_at_landing, velocity) < 0.0);
    }

    #[test]
    fn a_throw_lands_ahead_on_the_ground() {
        let flight: Flight = throw_from_eye(0.0, find_ground_hit);
        let (x, _) = flight.landing.expect("it lands");
        let [u_m, v_m, h_m] = get_fermi_coordinates(x, R_M);
        assert!(h_m.abs() < LANDING_TOLERANCE_M, "{h_m}");
        assert!(u_m > MIN_RANGE_M && v_m.abs() < TOLERANCE_M, "{u_m} {v_m}");
    }

    #[test]
    fn a_throw_at_a_wall_drops_below_it() {
        let flight: Flight = throw_from_eye(0.0, find_wall_or_ground_hit);
        let (x, _) = flight.landing.expect("it lands");
        let [u_m, _, h_m] = get_fermi_coordinates(x, R_M);
        assert!(h_m.abs() < LANDING_TOLERANCE_M, "{h_m}");
        assert!(u_m < WALL_M && u_m > WALL_M - 2.0 * BACK_OFF_M, "{u_m}");
    }
}
