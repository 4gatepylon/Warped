//! `third_person.rs` is the third-person view on voxel maps, W-61 (third-person view). The camera
//! hangs on an L-shaped boom: up from the eye to a pivot just above the head, then straight back
//! from there along a geodesic, looking the way the player looks, so the crosshair passes over the
//! head. Each leg pulls in toward the head when the ground or a block (glass too) is in the way, so
//! the camera never goes through a wall, and with no room above the head (a 2 m walkway) the
//! camera stays at the head. Switching eases the camera out along the boom and back, and aiming
//! brings it back to the eye while held. The window draws the player's own body from there, as the
//! others see it, and a shot still leaves the eye, turned toward the point the crosshair covers.
//!
//! Interface: `ViewSwitch`, `ThirdPersonCamera` (both `warped_shared::third_person`'s, every level's),
//! `place_camera`, `aim_at_crosshair`.
//!
//! Example: `let placed = third_person::place_camera(&world, eye, switch.get_boom_frac(0.0));`

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{
    AXIS_W, AXIS_Z, UP_VECTOR, get_distance, get_local_direction, make_boost, make_boost_along,
    make_rotation_between, normalize_tangent,
};
use crate::voxel_world::VoxelWorld;

pub use warped_shared::third_person::{BOOM_BACK_M, MIN_PIVOT_UP_M, PIVOT_UP_M, WALL_GAP_M};
pub use warped_shared::third_person::{ThirdPersonCamera, ViewSwitch};
const MAX_AIM_TURN_RAD: f64 = 0.6;
const MIN_AIM_DISTANCE_M: f64 = 0.01;

/// `place_camera` gives the camera `boom_frac` (0 to 1) of the way out its boom from the eye frame
/// `eye` in `world`. The boom goes up `PIVOT_UP_M` along the vertical, to a pivot above the head,
/// then back `BOOM_BACK_M` along the pivot's own back axis, so the camera orbits the pivot as the
/// player looks up and down; the camera looks the way the eye does, carried along the boom. Each
/// leg stops `WALL_GAP_M` short of the ground or a block. With the pivot held below
/// `MIN_PIVOT_UP_M` the camera stays at it, inside the head, and the body isn't shown.
///
/// Example: `let placed = place_camera(&world, eye, 1.0);`
pub fn place_camera(world: &VoxelWorld, eye: DMat4, boom_frac: f64) -> ThirdPersonCamera {
    let along_m: f64 = boom_frac.clamp(0.0, 1.0) * (PIVOT_UP_M + BOOM_BACK_M);
    let x: DVec4 = eye.col(AXIS_W);
    let up: DVec4 = normalize_tangent(x, UP_VECTOR);
    let wanted_up_m: f64 = along_m.min(PIVOT_UP_M);
    let up_m: f64 = (world.find_clear_m(x, up, wanted_up_m + WALL_GAP_M) - WALL_GAP_M)
        .min(wanted_up_m)
        .max(0.0);
    let local_up: DVec3 = get_local_direction(eye, up).normalize();
    let pivot: DMat4 = eye * make_boost_along(local_up, up_m / world.r);
    let wanted_back_m: f64 = along_m - PIVOT_UP_M;
    if up_m < MIN_PIVOT_UP_M || wanted_back_m <= 0.0 {
        return ThirdPersonCamera {
            frame: pivot,
            is_body_shown: up_m >= MIN_PIVOT_UP_M,
        };
    }
    let back: DVec4 = pivot.col(AXIS_Z);
    let back_m: f64 = (world.find_clear_m(pivot.col(AXIS_W), back, wanted_back_m + WALL_GAP_M)
        - WALL_GAP_M)
        .min(wanted_back_m)
        .max(0.0);
    ThirdPersonCamera {
        frame: pivot * make_boost(AXIS_Z, back_m / world.r),
        is_body_shown: true,
    }
}

/// `aim_at_crosshair` gives `eye` turned toward the point the crosshair of `camera` covers in
/// `world` (`VoxelWorld::find_crosshair_point`), so a shot from the eye lands where the crosshair
/// points; or `eye` itself when that point is at the eye or more than `MAX_AIM_TURN_RAD` off where
/// the eye looks (behind the player, say).
///
/// Example: `let shot_eye = aim_at_crosshair(&world, eye, placed.frame);`
///
/// Must know:
/// - Limitations: a wormhole's throat sends two rays 0.6 m apart far apart, so a shot through a
///   mouth lands only roughly where the crosshair points; aiming looks from the eye, exactly.
pub fn aim_at_crosshair(world: &VoxelWorld, eye: DMat4, camera: DMat4) -> DMat4 {
    let x: DVec4 = eye.col(AXIS_W);
    let target: DVec4 = world.find_crosshair_point(camera);
    if get_distance(x, target) * world.r < MIN_AIM_DISTANCE_M {
        return eye;
    }
    let toward: DVec3 = get_local_direction(eye, normalize_tangent(x, target)).normalize();
    if toward.dot(DVec3::NEG_Z) < MAX_AIM_TURN_RAD.cos() {
        return eye;
    }
    eye * make_rotation_between(DVec3::NEG_Z, toward)
}

#[cfg(test)]
mod tests {
    //! Tests of where the third-person camera goes, how switching to it eases, and where a shot
    //! aimed through its crosshair lands, on Lunar Surface.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | What is in the way | nothing (open ground), the ground (looking up), a wall behind (a habitat module), a low ceiling (a 2 m walkway) | `the_camera_hangs_behind_and_above_the_head`, `the_ground_and_walls_pull_the_camera_in` |
    //! | Pitch | level, looking down, looking up | `the_camera_hangs_behind_and_above_the_head`, `the_ground_and_walls_pull_the_camera_in` |
    //! | Boom fraction | 0 (at the eye), on the way up to the pivot, all the way out | `the_camera_hangs_behind_and_above_the_head` |
    //! | Chosen view | first person, third person, switched back | `switching_eases_out_and_back_and_aiming_brings_it_in` |
    //! | Time since switching | none, part of `SWITCH_S`, all of it and more | `switching_eases_out_and_back_and_aiming_brings_it_in` |
    //! | Aim fraction | not aiming, half way, fully aimed | `switching_eases_out_and_back_and_aiming_brings_it_in` |
    //! | Crosshair's ray | ahead and straight, ahead through the black hole's reach (bent alike), behind the player (the eye doesn't turn) | `a_shot_lands_where_the_crosshair_points` |

    use super::*;
    use warped_shared::third_person::SWITCH_S;
    use crate::geometry::{AXIS_X, invert_lorentz, make_rotation};
    use crate::map::{Map, get_map_folder};
    use crate::voxel_world::{Shot, VoxelPlayer};

    const OPEN_AT_M: [f64; 2] = [-27.0, 0.0];
    const OPEN_TOWARD_M: [f64; 2] = [-20.0, 0.0];
    const LOOK_DOWN_RAD: f64 = -0.5;
    const LOOK_UP_RAD: f64 = 1.2;
    const AIM_DOWN_RAD: f64 = -0.3;
    const HILL_AT_M: [f64; 2] = [-27.0, 3.0];
    const HILL_TOWARD_M: [f64; 2] = [-10.0, 2.0];
    const AIM_HILL_RAD: f64 = -0.08;
    const PART_WAY_FRAC: f64 = 0.1;
    const HALF_FRAC: f64 = 0.5;
    const OFFSET_TOLERANCE_M: f64 = 0.02;
    const GAP_TOLERANCE_M: f64 = 0.01;
    const SPOT_TOLERANCE_M: f64 = 0.05;
    const MIN_PARALLAX_M: f64 = 0.5;
    const TINY_M: f64 = 1e-6;
    const FRAC_TOLERANCE: f64 = 1e-9;

    /// `load_lunar_surface` loads the shipped Lunar Surface map as a voxel world.
    ///
    /// Example: `let world = load_lunar_surface();`
    fn load_lunar_surface() -> VoxelWorld {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        VoxelWorld::FromMap(&map).expect("a voxel world")
    }

    /// `get_local_m` gives point x in the local chart of `frame`, in meters (straight lines through
    /// the frame's origin are geodesics, and lengths near it are nearly meters).
    ///
    /// Example: `let seen_m = get_local_m(&world, camera, eye.col(AXIS_W));`
    fn get_local_m(world: &VoxelWorld, frame: DMat4, x: DVec4) -> DVec3 {
        let p: DVec4 = invert_lorentz(frame) * x;
        p.truncate() / p.w * world.r
    }

    /// `get_standing_eye` gives the eye of a player standing in structure `name`, turned a quarter
    /// turn from its length so a wall is behind them.
    ///
    /// Example: `let eye = get_standing_eye(&world, "red walkway");`
    fn get_standing_eye(world: &VoxelWorld, name: &str) -> DMat4 {
        let k: usize = world
            .structures
            .iter()
            .position(|st| st.name == name)
            .unwrap_or_else(|| panic!("the map has a {name}"));
        let frame: DMat4 = invert_lorentz(world.structures[k].inverse);
        let turn: DMat4 = make_rotation(AXIS_X, AXIS_Z, std::f64::consts::FRAC_PI_2);
        VoxelPlayer::Standing(world, frame * turn).get_camera_frame(world)
    }

    /// `get_gap_behind_m` gives how far the camera is from what is behind it along `direction`
    /// (a unit tangent at the camera), up to a meter.
    ///
    /// Example: `let gap_m = get_gap_behind_m(&world, placed.frame, placed.frame.col(AXIS_Z));`
    fn get_gap_behind_m(world: &VoxelWorld, camera: DMat4, direction: DVec4) -> f64 {
        world.find_clear_m(camera.col(AXIS_W), direction, 1.0)
    }

    #[test]
    fn the_camera_hangs_behind_and_above_the_head() {
        let world: VoxelWorld = load_lunar_surface();
        let eye_at = |pitch_rad: f64| -> DMat4 {
            VoxelPlayer::AtView(&world, OPEN_AT_M, OPEN_TOWARD_M, None, pitch_rad)
                .get_camera_frame(&world)
        };
        let eye: DMat4 = eye_at(0.0);
        let level: ThirdPersonCamera = place_camera(&world, eye, 1.0);
        let seen_m: DVec3 = get_local_m(&world, level.frame, eye.col(AXIS_W));
        assert!(
            seen_m.x.abs() < TINY_M,
            "the eye is {seen_m} from the camera"
        );
        assert!(
            (seen_m.y + PIVOT_UP_M).abs() < OFFSET_TOLERANCE_M,
            "{seen_m}"
        );
        assert!(
            (seen_m.z + BOOM_BACK_M).abs() < OFFSET_TOLERANCE_M,
            "{seen_m}"
        );
        assert!(level.is_body_shown);
        let looking_down: ThirdPersonCamera = place_camera(&world, eye_at(LOOK_DOWN_RAD), 1.0);
        assert!(
            looking_down.frame.col(AXIS_W).y > level.frame.col(AXIS_W).y,
            "looking down raises the camera"
        );
        assert!(looking_down.is_body_shown);
        let at_eye: ThirdPersonCamera = place_camera(&world, eye, 0.0);
        assert_eq!(at_eye.frame, eye);
        assert!(!at_eye.is_body_shown);
        let rising: ThirdPersonCamera = place_camera(&world, eye, PART_WAY_FRAC);
        let rise_m: DVec3 = -get_local_m(&world, rising.frame, eye.col(AXIS_W));
        let expected_m: f64 = PART_WAY_FRAC * (PIVOT_UP_M + BOOM_BACK_M);
        assert!(
            rise_m.y > 0.0 && (rise_m.length() - expected_m).abs() < OFFSET_TOLERANCE_M,
            "part way out, the camera rises first: {rise_m}"
        );
        assert!(
            !rising.is_body_shown,
            "inside the head, the body isn't drawn"
        );
    }

    #[test]
    fn the_ground_and_walls_pull_the_camera_in() {
        let world: VoxelWorld = load_lunar_surface();
        let full_m: f64 = DVec3::new(0.0, PIVOT_UP_M, BOOM_BACK_M).length();
        let looking_up: DMat4 =
            VoxelPlayer::AtView(&world, OPEN_AT_M, OPEN_TOWARD_M, None, LOOK_UP_RAD)
                .get_camera_frame(&world);
        for (what, eye) in [
            ("the ground", looking_up),
            (
                "a module's wall",
                get_standing_eye(&world, "red habitat module"),
            ),
        ] {
            let placed: ThirdPersonCamera = place_camera(&world, eye, 1.0);
            let boom_m: f64 = get_distance(eye.col(AXIS_W), placed.frame.col(AXIS_W)) * world.r;
            assert!(boom_m < full_m - WALL_GAP_M, "{what}: {boom_m} m out");
            let gap_m: f64 = get_gap_behind_m(&world, placed.frame, placed.frame.col(AXIS_Z));
            assert!(
                (gap_m - WALL_GAP_M).abs() < GAP_TOLERANCE_M,
                "{what}: the camera is {gap_m} m from it"
            );
            assert!(placed.is_body_shown, "{what}");
        }
        let eye: DMat4 = get_standing_eye(&world, "red walkway");
        let placed: ThirdPersonCamera = place_camera(&world, eye, 1.0);
        let boom_m: f64 = get_distance(eye.col(AXIS_W), placed.frame.col(AXIS_W)) * world.r;
        assert!(
            boom_m < MIN_PIVOT_UP_M,
            "under a low ceiling: {boom_m} m out"
        );
        let up: DVec4 = normalize_tangent(placed.frame.col(AXIS_W), UP_VECTOR);
        let gap_m: f64 = get_gap_behind_m(&world, placed.frame, up);
        assert!(
            (gap_m - WALL_GAP_M).abs() < GAP_TOLERANCE_M,
            "the camera is {gap_m} m under the ceiling"
        );
        assert!(!placed.is_body_shown);
    }

    #[test]
    fn switching_eases_out_and_back_and_aiming_brings_it_in() {
        let mut switch: ViewSwitch = ViewSwitch::FirstPerson();
        switch.update(SWITCH_S);
        assert_eq!(switch.get_boom_frac(0.0), 0.0);
        switch.toggle();
        switch.update(HALF_FRAC * SWITCH_S);
        let half_way: f64 = switch.get_boom_frac(0.0);
        assert!((half_way - HALF_FRAC).abs() < FRAC_TOLERANCE, "{half_way}");
        switch.update(SWITCH_S);
        assert_eq!(switch.get_boom_frac(0.0), 1.0);
        assert!((switch.get_boom_frac(HALF_FRAC) - HALF_FRAC).abs() < FRAC_TOLERANCE);
        assert_eq!(switch.get_boom_frac(1.0), 0.0);
        switch.toggle();
        switch.update(HALF_FRAC * SWITCH_S);
        assert!((switch.get_boom_frac(0.0) - HALF_FRAC).abs() < FRAC_TOLERANCE);
        switch.update(SWITCH_S);
        assert_eq!(switch.get_boom_frac(0.0), 0.0);
    }

    #[test]
    fn a_shot_lands_where_the_crosshair_points() {
        let world: VoxelWorld = load_lunar_surface();
        let spot = |shot: Shot| -> DVec4 { shot.spot.expect("the shot lands on the ground") };
        for (what, at_m, toward_m, pitch_rad) in [
            ("close ahead", OPEN_AT_M, OPEN_TOWARD_M, AIM_DOWN_RAD),
            (
                "in the black hole's reach",
                HILL_AT_M,
                HILL_TOWARD_M,
                AIM_HILL_RAD,
            ),
        ] {
            let eye: DMat4 = VoxelPlayer::AtView(&world, at_m, toward_m, None, pitch_rad)
                .get_camera_frame(&world);
            let camera: DMat4 = place_camera(&world, eye, 1.0).frame;
            let seen: DVec4 = spot(world.trace_laser(camera));
            let aimed: DVec4 = spot(world.trace_laser(aim_at_crosshair(&world, eye, camera)));
            let aimed_m: f64 = get_distance(aimed, seen) * world.r;
            assert!(aimed_m < SPOT_TOLERANCE_M, "{what}: {aimed_m} m off");
            let straight_m: f64 = get_distance(spot(world.trace_laser(eye)), seen) * world.r;
            assert!(
                straight_m > MIN_PARALLAX_M,
                "{what}: {straight_m} m off unturned"
            );
        }
        let eye: DMat4 = VoxelPlayer::AtView(&world, OPEN_AT_M, OPEN_TOWARD_M, None, 0.0)
            .get_camera_frame(&world);
        let camera: DMat4 = place_camera(&world, eye, 1.0).frame;
        let looking_back: DMat4 = camera * make_rotation(AXIS_Z, AXIS_X, std::f64::consts::PI);
        assert_eq!(aim_at_crosshair(&world, eye, looking_back), eye);
    }
}
