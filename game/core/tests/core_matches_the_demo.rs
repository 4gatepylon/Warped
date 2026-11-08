//! `core_matches_the_demo.rs` checks that the modules step A4 moved into `warped-core` give exactly
//! the demo's results (through `legacy-oracle`) on the same inputs: Lunar Surface's map, its
//! terrain and models, and the hyperbolic geometry under them.
//!
//! Partitions and the tests that cover each part:
//!
//! | Partitioning | Part | Tests |
//! | --- | --- | --- |
//! | Module | `map` | `lunar_surface_loads_as_in_the_demo` |
//! | | `terrain` | `terrain_heights_match_the_demo` |
//! | | `mesh` | `model_distances_match_the_demo` |
//! | | `geometry` | `boosts_and_distances_match_the_demo` |
//! | Curvature | the map's own | `lunar_surface_loads_as_in_the_demo` |
//! | | halved | every test that loads a map |
//! | Sample point | inside the map's bounds, on and above the ground | every test with points |

#![cfg(target_os = "macos")]

use glam::{DMat4, DVec3, DVec4};
use legacy_oracle as demo;
use warped_core as core;

const MAP_NAME: &str = "lunar_surface";
const CURVATURE_PER_MAP_CURVATURE: f64 = 0.5;
const SAMPLE_COUNT_PER_SIDE: usize = 9;
const SAMPLE_HALF_WIDTH_M: f64 = 60.0;
const SAMPLE_HEIGHTS_M: [f64; 3] = [0.0, 1.5, 12.0];
const BOOST_RAPIDITIES: [f64; 4] = [-1.3, -0.2, 0.4, 2.1];

/// `load_maps` loads Lunar Surface in the game and in the demo, both curved by
/// `curvature_per_map_curvature`.
///
/// Example: `let (game_map, demo_map) = load_maps(1.0);`
fn load_maps(curvature_per_map_curvature: f64) -> (core::map::Map, demo::map::Map) {
    let mut game_map = core::map::Map::Load(&core::map::get_map_folder(MAP_NAME)).expect("loads");
    let mut demo_map = demo::map::Map::Load(&demo::map::get_map_folder(MAP_NAME)).expect("loads");
    game_map
        .scale_curvature(curvature_per_map_curvature)
        .expect("valid");
    demo_map
        .scale_curvature(curvature_per_map_curvature)
        .expect("valid");
    (game_map, demo_map)
}

/// `get_sample_points_m` gives a grid of (u, v) points, in meters, around the map's centre.
///
/// Example: `for (u_m, v_m) in get_sample_points_m() { ... }`
fn get_sample_points_m() -> Vec<(f64, f64)> {
    let step_m: f64 = 2.0 * SAMPLE_HALF_WIDTH_M / (SAMPLE_COUNT_PER_SIDE - 1) as f64;
    let offsets_m =
        (0..SAMPLE_COUNT_PER_SIDE).map(move |i| -SAMPLE_HALF_WIDTH_M + i as f64 * step_m);
    offsets_m
        .clone()
        .flat_map(|u_m| offsets_m.clone().map(move |v_m| (u_m, v_m)))
        .collect()
}

#[test]
fn lunar_surface_loads_as_in_the_demo() {
    for curvature_per_map_curvature in [1.0, CURVATURE_PER_MAP_CURVATURE] {
        let (game_map, demo_map) = load_maps(curvature_per_map_curvature);
        assert_eq!(
            game_map.get_curvature_radius_m(),
            demo_map.get_curvature_radius_m()
        );
        let game_spawns: Vec<(&str, [f64; 16])> = game_map
            .spawns
            .iter()
            .map(|s| (s.team.as_str(), s.frame))
            .collect();
        let demo_spawns: Vec<(&str, [f64; 16])> = demo_map
            .spawns
            .iter()
            .map(|s| (s.team.as_str(), s.frame))
            .collect();
        assert_eq!(game_spawns, demo_spawns);
        let game_structures: Vec<[f64; 16]> = game_map.structures.iter().map(|s| s.frame).collect();
        let demo_structures: Vec<[f64; 16]> = demo_map.structures.iter().map(|s| s.frame).collect();
        assert_eq!(game_structures, demo_structures);
    }
}

#[test]
fn terrain_heights_match_the_demo() {
    let (game_map, demo_map) = load_maps(CURVATURE_PER_MAP_CURVATURE);
    let game_terrain =
        core::terrain::Terrain::FromMap(game_map.ground.terrain.as_ref().expect("terrain"));
    let demo_terrain =
        demo::terrain::Terrain::FromMap(demo_map.ground.terrain.as_ref().expect("terrain"));
    assert_eq!(
        game_terrain.get_max_height_m(),
        demo_terrain.get_max_height_m()
    );
    for (u_m, v_m) in get_sample_points_m() {
        assert_eq!(
            game_terrain.get_height_m(u_m, v_m),
            demo_terrain.get_height_m(u_m, v_m)
        );
        assert_eq!(
            game_terrain.get_wall_frac(u_m, v_m),
            demo_terrain.get_wall_frac(u_m, v_m)
        );
    }
}

#[test]
fn model_distances_match_the_demo() {
    let (game_map, demo_map) = load_maps(CURVATURE_PER_MAP_CURVATURE);
    let game_meshes: Vec<&core::map::MapMesh> = game_map
        .structures
        .iter()
        .filter_map(|s| s.mesh.as_ref())
        .collect();
    assert!(
        !game_meshes.is_empty(),
        "Lunar Surface has no models to compare"
    );
    let demo_meshes = demo_map.structures.iter().filter_map(|s| s.mesh.as_ref());
    for (game_mesh, demo_mesh) in game_meshes.into_iter().zip(demo_meshes) {
        let game_model =
            core::mesh::MeshModel::Load(&game_mesh.model, game_mesh.height_m).expect("loads");
        let demo_model =
            demo::mesh::MeshModel::Load(&demo_mesh.model, demo_mesh.height_m).expect("loads");
        for (u_m, v_m) in get_sample_points_m() {
            for h_m in SAMPLE_HEIGHTS_M {
                let q: DVec3 = DVec3::new(u_m, h_m, v_m);
                assert_eq!(game_model.get_distance_m(q), demo_model.get_distance_m(q));
            }
        }
    }
}

#[test]
fn boosts_and_distances_match_the_demo() {
    let (game_map, _) = load_maps(CURVATURE_PER_MAP_CURVATURE);
    let r_m: f64 = game_map.get_curvature_radius_m();
    for (u_m, v_m) in get_sample_points_m() {
        for h_m in SAMPLE_HEIGHTS_M {
            let game_x: DVec4 = core::geometry::make_fermi_point(u_m, v_m, h_m, r_m);
            assert_eq!(game_x, demo::geometry::make_fermi_point(u_m, v_m, h_m, r_m));
            assert_eq!(
                core::terrain::get_fermi_coordinates(game_x, r_m),
                demo::terrain::get_fermi_coordinates(game_x, r_m)
            );
            for (i, s) in BOOST_RAPIDITIES.iter().enumerate() {
                let game_boost: DMat4 = core::geometry::make_boost(i % 3, *s);
                assert_eq!(game_boost, demo::geometry::make_boost(i % 3, *s));
                let boosted_x: DVec4 = game_boost * game_x;
                assert_eq!(
                    core::geometry::get_distance(game_x, boosted_x),
                    demo::geometry::get_distance(game_x, boosted_x)
                );
            }
        }
    }
}
