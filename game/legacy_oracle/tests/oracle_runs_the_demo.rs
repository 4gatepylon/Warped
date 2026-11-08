//! `oracle_runs_the_demo.rs` checks that `legacy-oracle` runs the demo's own world on Lunar Surface,
//! and runs it the same way every time, which every comparison with the game relies on. Its helpers
//! are the pattern a comparison test follows: load the map as the demo's `load_map` does, walk a
//! scripted input sequence at 60 Hz, and compare frames exactly.
//!
//! Partitions and the tests that cover each part:
//!
//! | Partitioning | Part | Tests |
//! | --- | --- | --- |
//! | Movement keys held | none | `standing_still_stays_put` |
//! | | forward | `walking_forward_moves_the_same_way_every_run`, `walking_forward_leaves_the_spawn` |
//! | Runs of the same inputs | first and a repeat | `walking_forward_moves_the_same_way_every_run` |
//! | What is asked of the world | a step of walking | every walking test |
//! | | a laser's path | `a_laser_from_the_spawn_has_a_finite_length` |

use glam::{DMat4, DVec4};
use legacy_oracle::geometry::get_distance;
use legacy_oracle::map::{Map, get_map_folder};
use legacy_oracle::physics::Input;
use legacy_oracle::settings::ServerSettings;
use legacy_oracle::voxel_world::{VoxelPlayer, VoxelWorld};

const MAP_NAME: &str = "lunar_surface";
const DT_S: f64 = 1.0 / 60.0;
const WALK_STEP_COUNT: usize = 120;
const MIN_WALKED_M: f64 = 1.0;
const MAX_DRIFT_STANDING_M: f64 = 0.05;
const SPAWN_INDEX: usize = 0;

/// `load_world` builds the demo's world of Lunar Surface as its `load_map` and `play` do: the
/// map curved by the built-in settings, then the world with those settings.
///
/// Example: `let world = load_world();`
fn load_world() -> VoxelWorld {
    let settings: ServerSettings = ServerSettings::Builtin();
    let mut map: Map = Map::Load(&get_map_folder(MAP_NAME)).expect("Lunar Surface loads");
    map.scale_curvature(settings.space.curvature_per_map_curvature)
        .expect("the built-in curvature is valid");
    let mut world: VoxelWorld = VoxelWorld::FromMap(&map).expect("Lunar Surface builds a world");
    world.set_settings(settings);
    world
}

/// `walk` steps a player from the first spawn with `input` held for `step_count` steps of `DT_S`,
/// and gives their feet frame after each step.
///
/// Example: `let frames = walk(&world, Input::default(), 60);`
fn walk(world: &VoxelWorld, input: Input, step_count: usize) -> Vec<DMat4> {
    let mut player: VoxelPlayer = VoxelPlayer::AtSpawn(world, SPAWN_INDEX);
    (0..step_count)
        .map(|_| {
            player.step(world, &input, DT_S, SPAWN_INDEX);
            player.get_feet_frame(world)
        })
        .collect()
}

/// `get_feet_distance_m` gives how far apart, in meters, the feet of two feet frames are.
///
/// Example: `let d_m = get_feet_distance_m(&world, first, last);`
fn get_feet_distance_m(world: &VoxelWorld, a: DMat4, b: DMat4) -> f64 {
    let (a_point, b_point): (DVec4, DVec4) = (a.w_axis, b.w_axis);
    get_distance(a_point, b_point) * world.r
}

#[test]
fn walking_forward_moves_the_same_way_every_run() {
    let world: VoxelWorld = load_world();
    let forward: Input = Input {
        forward: true,
        ..Input::default()
    };
    assert!(walk(&world, forward, WALK_STEP_COUNT) == walk(&world, forward, WALK_STEP_COUNT));
}

#[test]
fn walking_forward_leaves_the_spawn() {
    let world: VoxelWorld = load_world();
    let start: DMat4 = VoxelPlayer::AtSpawn(&world, SPAWN_INDEX).get_feet_frame(&world);
    let forward: Input = Input {
        forward: true,
        ..Input::default()
    };
    let end: DMat4 = *walk(&world, forward, WALK_STEP_COUNT)
        .last()
        .expect("steps");
    let walked_m: f64 = get_feet_distance_m(&world, start, end);
    assert!(walked_m > MIN_WALKED_M, "walked only {walked_m} m");
}

#[test]
fn standing_still_stays_put() {
    let world: VoxelWorld = load_world();
    let start: DMat4 = VoxelPlayer::AtSpawn(&world, SPAWN_INDEX).get_feet_frame(&world);
    let end: DMat4 = *walk(&world, Input::default(), WALK_STEP_COUNT)
        .last()
        .expect("steps");
    let drift_m: f64 = get_feet_distance_m(&world, start, end);
    assert!(drift_m < MAX_DRIFT_STANDING_M, "drifted {drift_m} m");
}

#[test]
fn a_laser_from_the_spawn_has_a_finite_length() {
    let world: VoxelWorld = load_world();
    let player: VoxelPlayer = VoxelPlayer::AtSpawn(&world, SPAWN_INDEX);
    let length_m: f64 = world.trace_laser(player.get_camera_frame(&world)).length_m;
    assert!(
        length_m.is_finite() && length_m > 0.0,
        "laser length {length_m} m"
    );
}
