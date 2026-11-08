//! `console_reach` says who may hold `E` on a console, as Lunar Surface's `console_reach.rs` does
//! (W-126): a player whose view meets the console within the settings' `consoles.reach_m`. The game
//! server checks every hold with it, so no window or bot turns a console off from afar, a window
//! fills its console bar only for such a hold, and the console bot walks until it holds.
//!
//! Interface: `ConsolePlace` (`is_held_from`, `get_centre_m`), `find_console_places`,
//! `REACH_TOLERANCE_M`.
//!
//! Example: `let places: Vec<ConsolePlace> = console_reach::find_console_places(&map);`

use glam::{DMat4, DVec3, Mat4, Vec3};

use crate::map::{self, MapFile};

/// `REACH_TOLERANCE_M` is how much the reach and the console's box both grow: the pylon's model is
/// drawn inside its box, and a window's eye moves a little between the frame it aimed in and the
/// state the server reads (Lunar Surface's).
pub const REACH_TOLERANCE_M: f64 = 0.5;

/// `ConsolePlace` is where a console is: its team (0 blue, 1 red, as `net::TEAM_NAMES`) and the
/// corners of its box (the pylon's console shape), in meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConsolePlace {
    pub team: u32,
    pub min_m: DVec3,
    pub max_m: DVec3,
}

impl ConsolePlace {
    /// `is_held_from` says whether a player whose eye frame is `eye` (looking along its -z) reaches
    /// the console: their view meets its box within `reach_m`, both grown by `REACH_TOLERANCE_M`.
    ///
    /// Example: `if place.is_held_from(player.frame, settings.consoles.reach_m) { ... }`
    pub fn is_held_from(&self, eye: DMat4, reach_m: f64) -> bool {
        let origin_m: DVec3 = eye.w_axis.truncate();
        let direction: DVec3 = -eye.z_axis.truncate().normalize();
        let grown: DVec3 = DVec3::splat(REACH_TOLERANCE_M);
        let inverse: DVec3 = direction.recip();
        let t0: DVec3 = (self.min_m - grown - origin_m) * inverse;
        let t1: DVec3 = (self.max_m + grown - origin_m) * inverse;
        let entry_m: f64 = t0.min(t1).max_element();
        let exit_m: f64 = t0.max(t1).min_element();
        entry_m <= exit_m && exit_m >= 0.0 && entry_m <= reach_m + REACH_TOLERANCE_M
    }

    /// `get_centre_m` gives the middle of the console's box, where a player aims to hold `E`.
    ///
    /// Example: `let aim_m: DVec3 = place.get_centre_m();`
    pub fn get_centre_m(&self) -> DVec3 {
        (self.min_m + self.max_m) * 0.5
    }
}

/// `find_console_places` gives every console of `map`, in the order `net.rs` numbers them: the
/// boxes and the shapes of pylon meshes whose material is named `blue console` or `red console`.
///
/// Example: `let places: Vec<ConsolePlace> = find_console_places(&map);`
pub fn find_console_places(map: &MapFile) -> Vec<ConsolePlace> {
    let place = |material: u8, centre_m: Vec3, half_m: Vec3| -> Option<ConsolePlace> {
        Some(ConsolePlace {
            team: map::get_console_team(map, material)?,
            min_m: (centre_m - half_m).as_dvec3(),
            max_m: (centre_m + half_m).as_dvec3(),
        })
    };
    let box_places = map.boxes.iter().filter_map(|map_box| {
        place(
            map_box.material,
            map::frame_translation_m(&map_box.frame),
            map::frame_half_extent_m(&map_box.frame, Vec3::from(map_box.half_size_m)),
        )
    });
    let pylon_places = map
        .structures
        .iter()
        .filter(|structure| structure.mesh.is_some())
        .flat_map(|structure| {
            let frame: Mat4 = Mat4::from_cols_array(&structure.frame);
            structure.shapes.iter().filter_map(move |shape| {
                place(
                    shape.material,
                    frame.transform_point3(Vec3::from(shape.centre_m)),
                    map::frame_half_extent_m(&structure.frame, Vec3::from(shape.half_m)),
                )
            })
        });
    box_places.chain(pylon_places).collect()
}

/// `tests` checks who reaches a console and that the Dark Forest map's consoles are found.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | distance | in reach, just past the tolerance | `test_a_console_is_held_only_in_reach_and_aimed_at` |
/// | aim | at it, turned away, beside it | `test_a_console_is_held_only_in_reach_and_aimed_at` |
/// | eye | outside the box, inside it | `test_a_console_is_held_only_in_reach_and_aimed_at` |
/// | map | Dark Forest (two pylons a team) | `test_the_forest_has_two_consoles_a_team` |
#[cfg(test)]
mod tests {
    use super::*;

    const REACH_M: f64 = 3.0;
    const HALF_M: DVec3 = DVec3::new(0.36, 1.6, 0.36);
    const EYE_HEIGHT_M: f64 = 1.6;

    /// `eye_at` is an eye at `position_m` looking along `direction`.
    fn eye_at(position_m: DVec3, direction: DVec3) -> DMat4 {
        DMat4::look_to_rh(position_m, direction, DVec3::Y).inverse()
    }

    /// `test_a_console_is_held_only_in_reach_and_aimed_at` checks a pylon-shaped console is held
    /// from in front of it within reach, and not from too far, turned away or aimed beside it.
    #[test]
    fn test_a_console_is_held_only_in_reach_and_aimed_at() {
        let place: ConsolePlace = ConsolePlace {
            team: 1,
            min_m: DVec3::new(-HALF_M.x, 0.0, -HALF_M.z),
            max_m: DVec3::new(HALF_M.x, 2.0 * HALF_M.y, HALF_M.z),
        };
        let gap_m: f64 = HALF_M.z + REACH_M;
        let in_reach: DMat4 = eye_at(DVec3::new(0.0, EYE_HEIGHT_M, gap_m), DVec3::NEG_Z);
        assert!(place.is_held_from(in_reach, REACH_M));
        let too_far_m: f64 = gap_m + 2.0 * REACH_TOLERANCE_M + 0.1;
        let too_far: DMat4 = eye_at(DVec3::new(0.0, EYE_HEIGHT_M, too_far_m), DVec3::NEG_Z);
        assert!(!place.is_held_from(too_far, REACH_M));
        let turned_away: DMat4 = eye_at(DVec3::new(0.0, EYE_HEIGHT_M, gap_m), DVec3::Z);
        assert!(!place.is_held_from(turned_away, REACH_M));
        let beside: DMat4 = eye_at(
            DVec3::new(0.0, EYE_HEIGHT_M, gap_m),
            DVec3::new(1.0, 0.0, -1.0),
        );
        assert!(!place.is_held_from(beside, REACH_M));
        let inside: DMat4 = eye_at(DVec3::new(0.0, EYE_HEIGHT_M, 0.0), DVec3::Z);
        assert!(place.is_held_from(inside, REACH_M));
    }

    /// `test_the_forest_has_two_consoles_a_team` checks the Dark Forest map gives two consoles of
    /// each team, each a pylon's height tall.
    #[test]
    fn test_the_forest_has_two_consoles_a_team() {
        let map: MapFile = map::load(&map::default_map_path()).expect("the map loads");
        let places: Vec<ConsolePlace> = find_console_places(&map);
        for team in 0..2 {
            assert_eq!(places.iter().filter(|p| p.team == team).count(), 2);
        }
        assert!(
            places
                .iter()
                .all(|p| p.max_m.y - p.min_m.y > 2.0 * EYE_HEIGHT_M - 0.1)
        );
    }
}
