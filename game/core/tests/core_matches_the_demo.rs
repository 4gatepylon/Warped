//! `core_matches_the_demo.rs` checks that the modules step A4 moved into `warped-core` give exactly
//! the demo's results (through `legacy-oracle`) on the same inputs: Lunar Surface's map, its
//! terrain and models, the hyperbolic geometry under them, the built-in server settings, shot noise, landed thrown
//! wormholes, the zone walls, walking and lasers in the voxel world, and the network protocol (the
//! demo's server admits or refuses the game's window; `warped-server`'s `server_admits_players.rs`
//! checks the game's server with the demo's window). Values without a shared type are compared
//! through `Debug`, which prints every digit of a float.
//!
//! Partitions and the tests that cover each part:
//!
//! | Partitioning | Part | Tests |
//! | --- | --- | --- |
//! | Module | `map` | `lunar_surface_loads_as_in_the_demo` |
//! | | `terrain` | `terrain_heights_match_the_demo` |
//! | | `mesh` | `model_distances_match_the_demo` |
//! | | `geometry` | `boosts_and_distances_match_the_demo` |
//! | | `settings` | `builtin_server_settings_match_the_demo` |
//! | | `shot_noise` | `shot_noise_matches_the_demo` |
//! | | `thrown_wormholes` | `landed_mouths_match_the_demo` |
//! | | `zone_walls` | `zone_walls_match_the_demo` |
//! | | `voxel_world` | `walks_and_lasers_match_the_demo` |
//! | | `net` | `the_demos_server_admits_the_games_window`, `the_demos_server_refuses_wrong_passwords` |
//! | Server and window | the demo's server, the game's window | both `net` tests |
//! | Game password | right, wrong | `the_demos_server_admits_the_games_window`, `the_demos_server_refuses_wrong_passwords` |
//! | Team spawned on | blue, red | `walks_and_lasers_match_the_demo` |
//! | Movement | walking, sprinting while turning | `walks_and_lasers_match_the_demo` |
//! | Rifle heat | cold, half, hot | `shot_noise_matches_the_demo` |
//! | Curvature | the map's own | `lunar_surface_loads_as_in_the_demo` |
//! | | halved | every test that loads a map |
//! | Sample point | inside the map's bounds, on and above the ground | every test with points |

#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3, DVec4};
use legacy_oracle as demo;
use warped_core as core;

const MAP_NAME: &str = "lunar_surface";
const CURVATURE_PER_MAP_CURVATURE: f64 = 0.5;
const SAMPLE_COUNT_PER_SIDE: usize = 9;
const SAMPLE_HALF_WIDTH_M: f64 = 60.0;
const SAMPLE_HEIGHTS_M: [f64; 3] = [0.0, 1.5, 12.0];
const BOOST_RAPIDITIES: [f64; 4] = [-1.3, -0.2, 0.4, 2.1];
const SHOT_NOISE_SEED: u64 = 7;
const SHOT_COUNT: usize = 16;
const HEAT_FRACS: [f64; 3] = [0.0, 0.5, 1.0];
const MOUTH_HOVER_M: f64 = 0.8;
const TEAM_COUNT: u32 = 2;
const STEP_COUNT: usize = 180;
const DT_S: f64 = 1.0 / 60.0;
const SPRINT_FROM_STEP: usize = 90;
const GAME_PASSWORD: &[u8] = b"warped2-oracle";
const WRONG_PASSWORD: &[u8] = b"warped2-wrong";
const JOIN_TIMEOUT_S: f64 = 5.0;
const POLL_MS: u64 = 10;

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

#[test]
fn builtin_server_settings_match_the_demo() {
    assert_eq!(
        core::settings::ServerSettings::Builtin().to_json(),
        demo::settings::ServerSettings::Builtin().to_json()
    );
}

#[test]
fn shot_noise_matches_the_demo() {
    let game_settings = core::settings::ServerSettings::Builtin();
    let demo_settings = demo::settings::ServerSettings::Builtin();
    let mut game_noise = core::shot_noise::ShotNoise::FromSeed(SHOT_NOISE_SEED);
    let mut demo_noise = demo::shot_noise::ShotNoise::FromSeed(SHOT_NOISE_SEED);
    let camera: DMat4 = core::geometry::make_boost(0, 0.3);
    for heat_frac in HEAT_FRACS {
        for _ in 0..SHOT_COUNT {
            let game_sniper = game_noise.sample_sniper(&game_settings.sniper);
            let demo_sniper = demo_noise.sample_sniper(&demo_settings.sniper);
            assert_eq!(game_sniper.aim(camera), demo_sniper.aim(camera));
            let game_rifle = game_noise.sample_rifle(&game_settings.rifle, heat_frac);
            let demo_rifle = demo_noise.sample_rifle(&demo_settings.rifle, heat_frac);
            assert_eq!(format!("{game_rifle:?}"), format!("{demo_rifle:?}"));
            assert_eq!(
                game_noise.sample_sniper_speckle(&game_settings.sniper),
                demo_noise.sample_sniper_speckle(&demo_settings.sniper)
            );
        }
        assert_eq!(
            core::shot_noise::get_rifle_spread_rad(&game_settings.rifle, heat_frac),
            demo::shot_noise::get_rifle_spread_rad(&demo_settings.rifle, heat_frac)
        );
        assert_eq!(
            core::shot_noise::get_rifle_speckle_frac(&game_settings.rifle, heat_frac),
            demo::shot_noise::get_rifle_speckle_frac(&demo_settings.rifle, heat_frac)
        );
    }
}

#[test]
fn landed_mouths_match_the_demo() {
    let (game_map, _) = load_maps(CURVATURE_PER_MAP_CURVATURE);
    let r_m: f64 = game_map.get_curvature_radius_m();
    let velocity: DVec4 = DVec4::new(0.3, -0.2, 0.1, 0.0);
    for (u_m, v_m) in get_sample_points_m() {
        let x: DVec4 = core::geometry::make_fermi_point(u_m, v_m, 0.0, r_m);
        let game_mouth =
            core::thrown_wormholes::MouthPlace::Landed(x, velocity, MOUTH_HOVER_M, r_m);
        let demo_mouth =
            demo::thrown_wormholes::MouthPlace::Landed(x, velocity, MOUTH_HOVER_M, r_m);
        assert_eq!(format!("{game_mouth:?}"), format!("{demo_mouth:?}"));
        assert_eq!(game_mouth.get_frame(r_m), demo_mouth.get_frame(r_m));
    }
}

#[test]
fn zone_walls_match_the_demo() {
    let (game_map, demo_map) = load_maps(CURVATURE_PER_MAP_CURVATURE);
    let r_m: f64 = game_map.get_curvature_radius_m();
    let game_walls =
        core::zone_walls::ZoneWalls::FromRegions(game_map.regions.as_ref().expect("regions"))
            .expect("valid")
            .expect("Lunar Surface has zone walls");
    let demo_walls =
        demo::zone_walls::ZoneWalls::FromRegions(demo_map.regions.as_ref().expect("regions"))
            .expect("valid")
            .expect("Lunar Surface has zone walls");
    assert_eq!(format!("{game_walls:?}"), format!("{demo_walls:?}"));
    assert_eq!(
        format!("{:?}", game_walls.to_gpu(r_m)),
        format!("{:?}", demo_walls.to_gpu(r_m))
    );
}

#[test]
fn walks_and_lasers_match_the_demo() {
    let (game_map, demo_map) = load_maps(CURVATURE_PER_MAP_CURVATURE);
    let game_world = core::voxel_world::VoxelWorld::FromMap(&game_map).expect("a voxel world");
    let demo_world = demo::voxel_world::VoxelWorld::FromMap(&demo_map).expect("a voxel world");
    for team in 0..TEAM_COUNT {
        let mut game_player =
            core::voxel_world::VoxelPlayer::NearSpawn(&game_world, team, team, &[]);
        let mut demo_player =
            demo::voxel_world::VoxelPlayer::NearSpawn(&demo_world, team, team, &[]);
        for i in 0..STEP_COUNT {
            let is_sprinting: bool = i >= SPRINT_FROM_STEP;
            let game_input = core::player::Input {
                forward: true,
                left: is_sprinting,
                sprint: is_sprinting,
                ..Default::default()
            };
            let demo_input = demo::physics::Input {
                forward: true,
                left: is_sprinting,
                sprint: is_sprinting,
                ..Default::default()
            };
            let game_report = game_player.step(&game_world, &game_input, DT_S, 0);
            let demo_report = demo_player.step(&demo_world, &demo_input, DT_S, 0);
            assert_eq!(game_report.messages, demo_report.messages);
            assert_eq!(game_report.forward_m_per_s, demo_report.forward_m_per_s);
            assert_eq!(game_player.body, demo_player.body);
            assert_eq!(game_player.h_m, demo_player.h_m);
            let game_shot = game_world.trace_laser(game_player.body);
            let demo_shot = demo_world.trace_laser(demo_player.body);
            assert_eq!(
                (game_shot.spot, game_shot.is_hit, game_shot.length_m),
                (demo_shot.spot, demo_shot.is_hit, demo_shot.length_m)
            );
        }
    }
}

/// `start_server` runs `$net::serve` with `$settings::ServerSettings::Builtin()` on a free port of
/// this machine, for `GAME_PASSWORD`, on a thread of `$scope` until `$should_stop` is set, and
/// gives its address.
///
/// Example: `let server: SocketAddr = start_server!(core::net, core::settings, scope, &should_stop);`
macro_rules! start_server {
    ($net:ident :: $net_module:ident, $settings:ident :: $settings_module:ident, $scope:expr, $should_stop:expr) => {{
        let socket: std::net::UdpSocket =
            $net::$net_module::bind_server(0).expect("a free UDP port");
        let port: u16 = socket.local_addr().expect("a bound socket").port();
        let should_stop: &AtomicBool = $should_stop;
        $scope.spawn(move || {
            let should_start_new_game: AtomicBool = AtomicBool::new(false);
            $net::$net_module::serve(
                socket,
                should_stop,
                MAP_NAME.to_string(),
                $settings::$settings_module::ServerSettings::Builtin(),
                vec![0, 1],
                &should_start_new_game,
                None,
                GAME_PASSWORD.to_vec(),
            )
            .expect("the server runs");
        });
        std::net::SocketAddr::from(([127, 0, 0, 1], port))
    }};
}

/// `try_join` connects a `$net::Client` to `$server` with `$password`, joins any team and polls
/// until the server welcomes it (`true`) or refuses its password (`false`); it fails after
/// `JOIN_TIMEOUT_S`.
///
/// Example: `assert!(try_join!(demo::net, server, GAME_PASSWORD));`
macro_rules! try_join {
    ($net:ident :: $net_module:ident, $server:expr, $password:expr) => {{
        use $net::$net_module as net;
        let mut client: net::Client =
            net::Client::Connect($server, MAP_NAME, $password).expect("a client socket");
        client.join(None).expect("a join request");
        let start: Instant = Instant::now();
        loop {
            assert!(
                start.elapsed().as_secs_f64() < JOIN_TIMEOUT_S,
                "no answer from {}",
                $server
            );
            match client.poll() {
                Ok(_) if !client.is_waiting_for_welcome() => break true,
                Ok(_) => std::thread::sleep(Duration::from_millis(POLL_MS)),
                Err(error) if error.is::<net::PasswordRefused>() => break false,
                Err(error) => panic!("the client failed: {error}"),
            }
        }
    }};
}

#[test]
fn the_demos_server_admits_the_games_window() {
    let should_stop: AtomicBool = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let demo_server = start_server!(demo::net, demo::settings, scope, &should_stop);
        assert!(try_join!(core::net, demo_server, GAME_PASSWORD));
        should_stop.store(true, Ordering::Relaxed);
    });
}

#[test]
fn the_demos_server_refuses_wrong_passwords() {
    let should_stop: AtomicBool = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let demo_server = start_server!(demo::net, demo::settings, scope, &should_stop);
        assert!(!try_join!(core::net, demo_server, WRONG_PASSWORD));
        should_stop.store(true, Ordering::Relaxed);
    });
}
