//! `bench.rs` renders the benchmark's fixed views of a voxel map without a window (`--bench-views`), times
//! them, saves each as a PNG in its run folder and reports the frame times.
//!
//! Interface: `bench_views`, `make_voxel_bench_views`, `VoxelView`.
//!
//! Example: `bench_views(&arguments, renderer)?;`

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::DMat4;
use serde_json::{Value, json};

use crate::black_hole_growth::BlackHoleGrowth;
use crate::render_metal::FrameData;
use crate::render_voxel::VoxelFrame;
use crate::voxel_world::{VoxelPlayer, VoxelWorld};

use crate::run_folder::{make_run_folder, save_png};
use crate::{
    Arguments, BYTES_PER_PIXEL, MS_PER_S, Renderer, Result, get_sampling_name, load_map,
    load_settings, map, multiplayer, render_voxel, scale_size, voxel_world,
};

/// `VoxelView` is a named viewpoint on a voxel map, from the Lunar Surface level's own list: where
/// the eye stands (Fermi coordinates), what it faces, its height when flying and its pitch.
pub(crate) struct VoxelView {
    pub(crate) name: &'static str,
    pub(crate) at_m: [f64; 2],
    pub(crate) target_m: [f64; 2],
    pub(crate) h_m: Option<f64>,
    pub(crate) pitch_rad: f64,
    pub(crate) is_laser_on: bool,
}

/// `make_voxel_bench_views` gives the voxel benchmark's viewpoints: the Lunar Surface level's
/// views, and the laser fired into the wormhole from its viewpoint.
///
/// Example: `let views = make_voxel_bench_views();`
pub(crate) fn make_voxel_bench_views() -> Vec<VoxelView> {
    let view = |name, at_m, target_m, h_m, pitch_rad| VoxelView {
        name,
        at_m,
        target_m,
        h_m,
        pitch_rad,
        is_laser_on: false,
    };
    vec![
        view("blue_spawn", [-107.0, 0.0], [0.0, 0.0], None, 0.0),
        view("red_spawn", [112.0, 0.0], [0.0, 0.0], None, 0.0),
        view("overview", [0.0, 0.0], [0.0, -10.0], Some(46.0), -1.55),
        view("middle", [-27.0, 0.0], [40.0, 0.0], None, 0.1),
        view("black_hole", [-12.0, 4.0], [0.0, 0.0], Some(19.0), -0.42),
        view("wormhole", [-22.0, 40.0], [-14.0, 40.0], None, 0.0),
        view("wormhole_exit", [81.0, 26.0], [78.0, 30.0], None, 0.0),
        view("red_pylon", [82.0, -1.0], [94.0, 6.0], None, 0.05),
        view("ridge", [10.0, 58.5], [45.0, -5.0], None, -0.12),
        view("einstein_ring", [-10.0, -3.0], [0.0, 0.0], Some(11.0), 0.15),
        view("overlook", [-85.0, 0.0], [0.0, 0.0], Some(28.0), -0.7),
        view("habitat_row", [-46.0, -36.0], [0.0, -42.0], None, 0.0),
        view("crater_wall", [-30.0, -30.0], [-60.0, -90.0], None, 0.1),
        view("red_hangar", [100.0, 9.0], [121.0, 0.0], None, 0.1),
        view("blue_kazemat", [-100.0, 9.0], [-114.0, 0.0], None, 0.0),
        view(
            "blue_hesco_rows",
            [-40.0, 4.0],
            [-80.0, 0.0],
            Some(9.0),
            -0.25,
        ),
        view("field_bunker", [-18.0, -30.0], [-30.0, -40.0], None, 0.0),
        view(
            "field_bunker_above",
            [-22.0, -46.0],
            [-30.0, -40.0],
            Some(14.0),
            -0.7,
        ),
        view("hesco_barrier", [46.0, -8.0], [56.0, -15.0], None, 0.0),
        view("beacon", [87.0, 2.0], [94.0, 6.0], None, 0.2),
        VoxelView {
            is_laser_on: true,
            ..view(
                "laser_into_wormhole",
                [-22.0, 40.5],
                [-14.0, 40.0],
                None,
                0.0,
            )
        },
    ]
}

/// `bench_voxel_views` renders a voxel map's viewpoints without a window, timing each.
///
/// Example: `bench_voxel_views(&arguments, renderer, &world)?;`
fn bench_voxel_views(
    arguments: &Arguments,
    mut renderer: Box<dyn Renderer>,
    world: &VoxelWorld,
) -> Result<()> {
    let mut grown_world: VoxelWorld = world.clone();
    BlackHoleGrowth::New(arguments.black_hole_strength).update(&mut grown_world, 0, 0.0);
    let world: &VoxelWorld = &grown_world;
    let size_px: (u32, u32) = (arguments.width_px, arguments.height_px);
    let render_px: (u32, u32) = scale_size(size_px, arguments.render_scale.unwrap_or(1.0));
    let folder: PathBuf = make_run_folder("bench_views", arguments.label.as_deref())?;
    let mut pixels: Vec<u8> = vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL];
    let views: Vec<VoxelView> = make_voxel_bench_views();
    eprintln!(
        "Benchmarking {} views of {} at {}x{} with the {} renderer...",
        views.len(),
        world.name,
        size_px.0,
        size_px.1,
        renderer.name()
    );
    let mut results: Vec<Value> = Vec::new();
    for view in views {
        let player: VoxelPlayer =
            VoxelPlayer::AtView(world, view.at_m, view.target_m, view.h_m, view.pitch_rad);
        let camera: DMat4 = player.get_camera_frame(world);
        let shot: Option<voxel_world::Shot> = view.is_laser_on.then(|| world.trace_laser(camera));
        let beams: Vec<render_voxel::LaserBeam> = shot
            .iter()
            .map(|s| render_voxel::LaserBeam {
                segments: s.segments.clone(),
                team: 0,
                is_rifle: false,
                speckle_frac: 0.0,
            })
            .collect();
        let spot: Option<(glam::DVec4, u32)> = shot.as_ref().and_then(|s| s.spot).map(|x| (x, 0));
        let warm_up: VoxelFrame = VoxelFrame::Build(world, camera, &beams, spot, &[], render_px);
        renderer.render(FrameData::Voxel(&warm_up), size_px, &mut pixels)?;
        let (mut times_ms, mut gpu_times_ms): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        for _ in 0..arguments.frame_count {
            let start: Instant = Instant::now();
            let frame: VoxelFrame = VoxelFrame::Build(world, camera, &beams, spot, &[], render_px);
            renderer.render(FrameData::Voxel(&frame), size_px, &mut pixels)?;
            times_ms.push(start.elapsed().as_secs_f64() * MS_PER_S);
            gpu_times_ms.extend(renderer.gpu_ms());
        }
        times_ms.sort_by(f64::total_cmp);
        gpu_times_ms.sort_by(f64::total_cmp);
        save_png(
            &folder.join(format!("{}.png", view.name)),
            &pixels,
            size_px.0,
            size_px.1,
        )?;
        results.push(json!({
            "view": view.name,
            "median_ms": times_ms[times_ms.len() / 2],
            "fastest_ms": times_ms[0],
            "median_gpu_ms": gpu_times_ms.get(gpu_times_ms.len() / 2),
            "laser": shot.as_ref().map(|s| multiplayer::describe_shot(world, s)),
        }));
    }
    report_bench(arguments, &folder, renderer.name(), render_px, results)
}

/// `bench_views` renders each benchmark view `--frame-count` times without a window, reports the
/// median and fastest frame times, and saves each view as a PNG in the run folder.
///
/// Example: `bench_views(&arguments, renderer)?;`
pub(crate) fn bench_views(arguments: &Arguments, renderer: Box<dyn Renderer>) -> Result<()> {
    let map: map::Map = load_map(arguments.map, &load_settings(arguments)?)?;
    bench_voxel_views(arguments, renderer, &VoxelWorld::FromMap(&map)?)
}

/// `report_bench` writes a benchmark's results to `results.json` in its run folder and prints them
/// (or `--json`).
///
/// Example: `report_bench(&arguments, &folder, "metal", (1280, 800), results)?;`
fn report_bench(
    arguments: &Arguments,
    folder: &Path,
    renderer_name: &str,
    render_px: (u32, u32),
    results: Vec<Value>,
) -> Result<()> {
    let size_px: (u32, u32) = (arguments.width_px, arguments.height_px);
    let summary: Value = json!({
        "run_folder": folder.display().to_string(),
        "renderer": renderer_name,
        "sampling": get_sampling_name(arguments),
        "width_px": size_px.0,
        "height_px": size_px.1,
        "render_width_px": render_px.0,
        "render_height_px": render_px.1,
        "frame_count": arguments.frame_count,
        "views": results,
    });
    fs::write(
        folder.join("results.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    if arguments.json {
        println!("{summary}");
    } else {
        println!(
            "{:<18} {:>10} {:>10} {:>10}  laser",
            "view", "median ms", "fastest ms", "GPU ms"
        );
        for view in &results {
            println!(
                "{:<18} {:>10.1} {:>10.1} {:>10}  {}",
                view["view"].as_str().unwrap_or_default(),
                view["median_ms"].as_f64().unwrap_or_default(),
                view["fastest_ms"].as_f64().unwrap_or_default(),
                view["median_gpu_ms"]
                    .as_f64()
                    .map_or("-".to_string(), |ms| format!("{ms:.1}")),
                view["laser"].as_str().unwrap_or("-"),
            );
        }
        println!("Run folder: {}", folder.display());
    }
    Ok(())
}
