//! `thrown_wormholes.rs` lets each player throw a linked pair of wormhole mouths, one purple and one
//! green: a throw is lobbed from the eye under its own gravity, drops to the floor below whatever it
//! hits, and the mouth then hovers there at eye height, facing back the way it flew. Once both of a
//! player's mouths are down they link. Both maps use it; each says what a throw hits and where a
//! mouth may go.
//!
//! Interface: `WormholeColor`, `PURPLE_COLOR`, `GREEN_COLOR`, `MouthPlace`.
//!
//! Example: `let frame = place.get_frame(r);`

#![allow(dead_code)]

use glam::{DMat4, DVec4};

use crate::geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, invert_lorentz, make_boost, make_rotation};
use crate::terrain::get_fermi_coordinates;

pub const PURPLE_COLOR: [f32; 3] = [0.62, 0.22, 1.0];
pub const GREEN_COLOR: [f32; 3] = [0.2, 1.0, 0.4];

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

#[cfg(test)]
mod tests {
    //! Tests of thrown wormholes on flat ground (the plane h = 0) at a curvature radius of 40 m.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Mouth place | round trip through its frame | `a_place_survives_its_frame` |

    use super::*;

    const R_M: f64 = 40.0;
    const EYE_M: f64 = 1.6;
    const TOLERANCE_M: f64 = 1e-6;

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
}
