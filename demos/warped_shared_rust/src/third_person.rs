//! `third_person.rs` is the third-person view every level plays alike, W-61 (third-person view)
//! and W-126 (same play in every level): `V` eases the camera out along an L-shaped boom, up from
//! the eye to a pivot just above the head, then straight back from there, looking the way the
//! player looks, and aiming brings it back to the eye while held (`ViewSwitch`). Each leg stops
//! short of whatever is in the way, and with no room above the head the camera stays at the head.
//! Lunar Surface and its copies place the boom in their curved space themselves; a level in flat
//! space places it with `place_camera_flat`.
//!
//! Interface: `ViewSwitch`, `ThirdPersonCamera`, `place_camera_flat`, `PIVOT_UP_M`, `BOOM_BACK_M`,
//! `MIN_PIVOT_UP_M`, `WALL_GAP_M`, `SWITCH_S`.
//!
//! Example: `let placed = place_camera_flat(eye, up, switch.get_boom_frac(0.0), |from, direction, max_m| max_m);`

use glam::{DMat4, DVec3};

/// `PIVOT_UP_M` is how far above the eye the boom's pivot is, and `BOOM_BACK_M` how far behind
/// the pivot the camera hangs, all the way out.
pub const PIVOT_UP_M: f64 = 0.6;
pub const BOOM_BACK_M: f64 = 3.0;
/// `MIN_PIVOT_UP_M` is the least room above the eye for the camera to leave the head (and show
/// the player's own body).
pub const MIN_PIVOT_UP_M: f64 = 0.45;
/// `WALL_GAP_M` is how far short of what is in the way each leg of the boom stops.
pub const WALL_GAP_M: f64 = 0.25;
/// `SWITCH_S` is how long switching between first and third person takes.
pub const SWITCH_S: f64 = 0.3;

/// `ViewSwitch` is whether this player chose the third-person view, and how far the camera has
/// moved toward it: from 0 (at the eye) to 1 (out on its boom), over `SWITCH_S`.
pub struct ViewSwitch {
    is_third_person: bool,
    out_frac: f64,
}

impl ViewSwitch {
    /// `FirstPerson` is the view a game starts in: first person, the camera at the eye.
    ///
    /// Example: `let switch = ViewSwitch::FirstPerson();`
    #[allow(non_snake_case)]
    pub fn FirstPerson() -> ViewSwitch {
        ViewSwitch {
            is_third_person: false,
            out_frac: 0.0,
        }
    }

    /// `toggle` switches between first and third person; the camera then moves over `SWITCH_S`.
    ///
    /// Example: `switch.toggle();`
    pub fn toggle(&mut self) {
        self.is_third_person = !self.is_third_person;
    }

    /// `update` moves the camera `dt_s` seconds further toward the chosen view.
    ///
    /// Example: `switch.update(1.0 / 60.0);`
    pub fn update(&mut self, dt_s: f64) {
        let step: f64 = dt_s / SWITCH_S;
        self.out_frac = if self.is_third_person {
            (self.out_frac + step).min(1.0)
        } else {
            (self.out_frac - step).max(0.0)
        };
    }

    /// `get_boom_frac` gives how far out along its boom the camera is now, from 0 to 1: the
    /// switch's progress, eased in and out, brought back to the eye as aiming (`aim_frac`, 0 to 1)
    /// eases in, so aiming looks from the eye.
    ///
    /// Example: `let boom_frac = switch.get_boom_frac(session.aim_frac);`
    pub fn get_boom_frac(&self, aim_frac: f64) -> f64 {
        smoothstep(self.out_frac) * (1.0 - smoothstep(aim_frac))
    }
}

/// `smoothstep` eases `t` (clamped to 0 to 1) in and out: 3t² - 2t³.
///
/// Example: `let eased = smoothstep(0.25);`
fn smoothstep(t: f64) -> f64 {
    let t: f64 = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `ThirdPersonCamera` is where the camera was put: its frame, and whether the player's own body
/// shows from there (the camera is up at a pivot clear of the head, or behind it) rather than the
/// gun in hand.
#[derive(Clone, Copy, Debug)]
pub struct ThirdPersonCamera {
    pub frame: DMat4,
    pub is_body_shown: bool,
}

/// `place_camera_flat` places the camera `boom_frac` (0 to 1) of the way out its boom from the eye
/// frame `eye` (columns right, up, back, position) in flat space, `up` being the player's up: up
/// `PIVOT_UP_M` along `up` to a pivot over the head, then back `BOOM_BACK_M` along the eye's back
/// axis, each leg stopping `WALL_GAP_M` short of what `get_clear_m` finds (how far a ray from a
/// point along a direction goes before meeting anything, up to a length). The camera looks the
/// way the eye does. The player's own body shows once the pivot clears `MIN_PIVOT_UP_M`.
///
/// Example: `let placed = place_camera_flat(eye, DVec3::Y, 1.0, |from, direction, max_m| max_m);`
pub fn place_camera_flat(
    eye: DMat4,
    up: DVec3,
    boom_frac: f64,
    get_clear_m: impl Fn(DVec3, DVec3, f64) -> f64,
) -> ThirdPersonCamera {
    let along_m: f64 = boom_frac * (PIVOT_UP_M + BOOM_BACK_M);
    if along_m <= 0.0 {
        return ThirdPersonCamera {
            frame: eye,
            is_body_shown: false,
        };
    }
    let clear_m = |from: DVec3, direction: DVec3, wanted_m: f64| -> f64 {
        (get_clear_m(from, direction, wanted_m + WALL_GAP_M) - WALL_GAP_M).clamp(0.0, wanted_m)
    };
    let at: DVec3 = eye.w_axis.truncate();
    let up_m: f64 = clear_m(at, up, along_m.min(PIVOT_UP_M));
    let pivot: DVec3 = at + up * up_m;
    let back: DVec3 = eye.z_axis.truncate();
    let back_m: f64 = clear_m(pivot, back, (along_m - PIVOT_UP_M).max(0.0));
    let mut frame: DMat4 = eye;
    frame.w_axis = (pivot + back * back_m).extend(1.0);
    ThirdPersonCamera {
        frame,
        is_body_shown: up_m >= MIN_PIVOT_UP_M,
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the flat third-person camera and of switching to it.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | What is in the way | nothing, a ceiling low over the head, a wall behind | `the_flat_boom_stops_short_of_what_is_in_the_way` |
    //! | Boom fraction | 0 (at the eye), all the way out | `the_flat_boom_stops_short_of_what_is_in_the_way` |
    //! | Chosen view | first person, third person, switched back; aiming | `switching_eases_out_and_back_and_aiming_brings_it_in` |

    use super::*;

    const STEP_S: f64 = 1.0 / 60.0;
    const CEILING_M: f64 = 0.3;
    const WALL_M: f64 = 1.0;

    #[test]
    fn the_flat_boom_stops_short_of_what_is_in_the_way() {
        let eye: DMat4 = DMat4::from_translation(DVec3::new(0.0, 1.6, 0.0));
        let open = |_: DVec3, _: DVec3, max_m: f64| max_m;
        assert_eq!(place_camera_flat(eye, DVec3::Y, 0.0, open).frame, eye);
        let out: ThirdPersonCamera = place_camera_flat(eye, DVec3::Y, 1.0, open);
        let expected: DVec3 = DVec3::new(0.0, 1.6 + PIVOT_UP_M, BOOM_BACK_M);
        assert!(out.is_body_shown && (out.frame.w_axis.truncate() - expected).length() < 1e-12);
        let low_ceiling = |_: DVec3, direction: DVec3, max_m: f64| {
            if direction.y > 0.5 { CEILING_M } else { max_m }
        };
        let under: ThirdPersonCamera = place_camera_flat(eye, DVec3::Y, 1.0, low_ceiling);
        assert!(!under.is_body_shown);
        let wall_behind = |_: DVec3, direction: DVec3, max_m: f64| {
            if direction.z > 0.5 { WALL_M } else { max_m }
        };
        let walled: ThirdPersonCamera = place_camera_flat(eye, DVec3::Y, 1.0, wall_behind);
        assert!((walled.frame.w_axis.z - (WALL_M - WALL_GAP_M)).abs() < 1e-12);
    }

    #[test]
    fn switching_eases_out_and_back_and_aiming_brings_it_in() {
        let mut switch: ViewSwitch = ViewSwitch::FirstPerson();
        assert_eq!(switch.get_boom_frac(0.0), 0.0);
        switch.toggle();
        switch.update(SWITCH_S / 2.0);
        assert!((switch.get_boom_frac(0.0) - 0.5).abs() < 1e-12);
        switch.update(SWITCH_S);
        assert_eq!(switch.get_boom_frac(0.0), 1.0);
        assert_eq!(switch.get_boom_frac(1.0), 0.0);
        switch.toggle();
        for _ in 0..(SWITCH_S / STEP_S).ceil() as usize {
            switch.update(STEP_S);
        }
        assert_eq!(switch.get_boom_frac(0.0), 0.0);
    }
}
