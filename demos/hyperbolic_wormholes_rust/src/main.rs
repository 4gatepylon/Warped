//! `hyperbolic-wormholes` is the Rust port of the WebGL hyperbolic wormholes demo
//! (`../hyperbolic_wormholes/`), to see how fast the same ray tracer runs natively. You walk in
//! hyperbolic space (H³) with gravity, fly, go through three wormholes and fire a laser.
//!
//! Interface:
//! - `--renderer metal|cpu`: which renderer draws the frames: the GPU with Metal (the default) or
//!   every CPU core.
//! - `--render-scale <fraction>`: fraction of the window's pixels (per side) to render.
//! - `--bench-views`: render fixed views without a window, time them, and save them under
//!   `.artifacts/bench_views/<run_id>/`. `--frame-count`, `--width-px`, `--height-px` set it up.
//! - `--autopilot`: play a scripted route without a window at a fixed time step, and save frames,
//!   timings and events under `.artifacts/autopilot/<run_id>/`. `--snapshot-every` sets how often.
//! - `--json`: print results as one JSON object. `--label`: suffix for the run folder.
//!
//! Example: `cargo run --release -- --bench-views --width-px 2560 --height-px 1600`
//!
//! Must know:
//! - Gotchas: run folders are found from the crate folder, not the current directory.

mod geometry;
mod physics;
mod render_cpu;
mod render_metal;
mod scene_data;
mod world;

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use glam::{DMat4, DVec3};
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Texture;
use serde_json::{Value, json};

use geometry::{AXIS_Y, AXIS_Z, get_distance, make_boost, make_rotation};
use physics::{Input, LaserReport, Player};
use scene_data::{Beam, SceneData};
use world::{DEFAULT_CURVATURE_RADIUS_M, World};

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 800;
const BYTES_PER_PIXEL: usize = 4;
const CPU_RENDER_SCALE: f64 = 0.35;
const METAL_RENDER_SCALE: f64 = 1.0;
const RENDER_SCALE_STEP: f64 = 1.25;
const MIN_RENDER_SCALE: f64 = 0.05;
const MAX_FRAME_DT_S: f64 = 0.05;
const AUTOPILOT_DT_S: f64 = 1.0 / 60.0;
const HUD_PERIOD_S: f64 = 0.25;
const HUD_TEXT_SCALE: f32 = 2.0;
const HUD_LINE_PX: f32 = 10.0;
const HUD_COLOR: Color = Color::RGB(235, 240, 250);
const MESSAGE_DURATION_S: f64 = 3.0;
const MIN_IMAGE_SIDE_PX: u32 = 16;
const MAX_IMAGE_SIDE_PX: u32 = 8192;
const THROAT_VIEW_DISTANCE_M: f64 = 0.8;
const THROAT_APPROACH_STEP_M: f64 = 0.1;
const MAX_APPROACH_STEP_COUNT: usize = 2000;
const SECONDS_PER_DAY: u64 = 86_400;

/// `RendererKind` is which renderer draws the frames.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RendererKind {
    Cpu,
    Metal,
}

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Walk, fly and fire a laser through wormholes in hyperbolic space.",
    after_help = "Example: hyperbolic-wormholes --renderer cpu --bench-views --width-px 2560 --height-px 1600"
)]
struct Arguments {
    /// Which renderer draws the frames: the GPU with Metal, or every CPU core.
    #[arg(long, value_enum, default_value_t = RendererKind::Metal)]
    renderer: RendererKind,
    /// Fraction of the window's pixels per side to render (default 0.35 on the CPU, 1 with Metal).
    #[arg(long)]
    render_scale: Option<f64>,
    /// Render fixed views without a window, time them and save them under .artifacts/bench_views/.
    #[arg(long, conflicts_with = "autopilot")]
    bench_views: bool,
    /// Play a scripted route without a window and save frames and timings under .artifacts/autopilot/.
    #[arg(long)]
    autopilot: bool,
    /// Image width for --bench-views and --autopilot.
    #[arg(long, default_value_t = 1280)]
    width_px: u32,
    /// Image height for --bench-views and --autopilot.
    #[arg(long, default_value_t = 800)]
    height_px: u32,
    /// Timed frames per view for --bench-views.
    #[arg(long, default_value_t = 10)]
    frame_count: u32,
    /// Save every n-th --autopilot frame as a PNG.
    #[arg(long, default_value_t = 30)]
    snapshot_every: u32,
    /// Print results as one JSON object.
    #[arg(long)]
    json: bool,
    /// Suffix for the run folder's name (letters, digits, - and _).
    #[arg(long)]
    label: Option<String>,
}

/// `Renderer` draws one frame's packed scene into an RGBA pixel buffer.
trait Renderer {
    /// `render` draws `data` into `pixels` (top row first, 4 bytes per pixel).
    ///
    /// Example: `renderer.render(&data, &mut pixels)?;`
    fn render(&mut self, data: &SceneData, pixels: &mut [u8]) -> Result<()>;

    /// `name` names the renderer, for the HUD and results.
    ///
    /// Example: `println!("{}", renderer.name());`
    fn name(&self) -> &'static str;

    /// `gpu_ms` gives the GPU's own time for the last frame, for renderers that use one.
    ///
    /// Example: `if let Some(ms) = renderer.gpu_ms() { println!("{ms}"); }`
    fn gpu_ms(&self) -> Option<f64> {
        None
    }
}

/// `CpuRenderer` is the rayon ray tracer of `render_cpu.rs`.
struct CpuRenderer;

impl Renderer for CpuRenderer {
    fn render(&mut self, data: &SceneData, pixels: &mut [u8]) -> Result<()> {
        render_cpu::render(data, pixels);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "cpu"
    }
}

/// `make_renderer` makes the renderer the command line asked for.
///
/// Example: `let mut renderer = make_renderer(RendererKind::Cpu)?;`
fn make_renderer(kind: RendererKind) -> Result<Box<dyn Renderer>> {
    match kind {
        RendererKind::Cpu => Ok(Box::new(CpuRenderer)),
        RendererKind::Metal => Ok(Box::new(render_metal::MetalRenderer::New()?)),
    }
}

impl Renderer for render_metal::MetalRenderer {
    fn render(&mut self, data: &SceneData, pixels: &mut [u8]) -> Result<()> {
        render_metal::MetalRenderer::render(self, data, pixels)
    }

    fn name(&self) -> &'static str {
        "metal"
    }

    fn gpu_ms(&self) -> Option<f64> {
        Some(self.last_gpu_ms)
    }
}

/// `get_default_render_scale` gives the render scale a renderer starts at.
///
/// Example: `let scale = get_default_render_scale(RendererKind::Cpu);`
fn get_default_render_scale(kind: RendererKind) -> f64 {
    match kind {
        RendererKind::Cpu => CPU_RENDER_SCALE,
        RendererKind::Metal => METAL_RENDER_SCALE,
    }
}

/// `validate_arguments` checks every argument before anything happens, naming the argument and
/// the valid range on failure.
///
/// Example: `validate_arguments(&arguments)?;`
fn validate_arguments(arguments: &Arguments) -> Result<()> {
    if let Some(scale) = arguments.render_scale
        && !(scale > 0.0 && scale <= 1.0)
    {
        return Err(format!(
            "--render-scale must be in (0, 1], got {scale}; e.g. --render-scale 0.5"
        )
        .into());
    }
    for (name, side) in [
        ("--width-px", arguments.width_px),
        ("--height-px", arguments.height_px),
    ] {
        if !(MIN_IMAGE_SIDE_PX..=MAX_IMAGE_SIDE_PX).contains(&side) {
            return Err(format!("{name} must be in {MIN_IMAGE_SIDE_PX}..={MAX_IMAGE_SIDE_PX}, got {side}; e.g. {name} 1280").into());
        }
    }
    if arguments.frame_count == 0 || arguments.snapshot_every == 0 {
        return Err(
            "--frame-count and --snapshot-every must be at least 1; e.g. --frame-count 10".into(),
        );
    }
    if let Some(label) = &arguments.label
        && (label.is_empty()
            || !label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    {
        return Err(format!(
            "--label must be letters, digits, - and _, got {label:?}; e.g. --label m3-pro"
        )
        .into());
    }
    Ok(())
}

/// `main` validates the command line and runs the window, the benchmark or the autopilot.
///
/// Example: `cargo run --release -- --autopilot`
fn main() -> Result<()> {
    let arguments: Arguments = Arguments::parse();
    validate_arguments(&arguments)?;
    let renderer: Box<dyn Renderer> = make_renderer(arguments.renderer)?;
    if arguments.bench_views {
        bench_views(&arguments, renderer)
    } else if arguments.autopilot {
        run_autopilot(&arguments, renderer)
    } else {
        play(&arguments, renderer)
    }
}

/// `make_run_folder` creates `.artifacts/<script>/<run_id>/` in the crate folder, with
/// `command.txt` holding the command line and the git commit, and returns its path.
///
/// Example: `let folder = make_run_folder("bench_views", None)?;`
fn make_run_folder(script: &str, label: Option<&str>) -> Result<PathBuf> {
    let crate_folder: &Path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut run_id: String = format_utc_now();
    if let Some(label) = label {
        run_id = format!("{run_id}-{label}");
    }
    let folder: PathBuf = crate_folder.join(".artifacts").join(script).join(&run_id);
    if folder.exists() {
        return Err(format!(
            "run folder {} already exists; wait a second or pass another --label",
            folder.display()
        )
        .into());
    }
    fs::create_dir_all(&folder)?;
    let commit: String = std::process::Command::new("git")
        .args(["describe", "--always", "--dirty"])
        .current_dir(crate_folder)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())?;
    let command: String = std::env::args().collect::<Vec<String>>().join(" ");
    fs::write(
        folder.join("command.txt"),
        format!("{command}\ncommit: {commit}\n"),
    )?;
    Ok(folder)
}

/// `format_utc_now` formats the current UTC time as `YYYYMMDD-HHMMSS` (civil-from-days, Howard
/// Hinnant's algorithm).
///
/// Example: `let run_id = format_utc_now();`
fn format_utc_now() -> String {
    let seconds: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs();
    let (days, time_of_day) = (
        (seconds / SECONDS_PER_DAY) as i64,
        seconds % SECONDS_PER_DAY,
    );
    let z: i64 = days + 719_468;
    let era: i64 = z.div_euclid(146_097);
    let day_of_era: i64 = z - era * 146_097;
    let year_of_era: i64 =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year: i64 = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index: i64 = (5 * day_of_year + 2) / 153;
    let day: i64 = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month: i64 = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year: i64 = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        time_of_day / 3600,
        time_of_day / 60 % 60,
        time_of_day % 60
    )
}

/// `save_png` writes an RGBA image as a PNG file.
///
/// Example: `save_png(&folder.join("spawn.png"), &pixels, 1280, 800)?;`
fn save_png(path: &Path, pixels: &[u8], width_px: u32, height_px: u32) -> Result<()> {
    let file: fs::File = fs::File::create(path)?;
    let mut encoder: png::Encoder<std::io::BufWriter<fs::File>> =
        png::Encoder::new(std::io::BufWriter::new(file), width_px, height_px);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}

/// `render_view` packs the scene for a camera (with the laser if `is_laser_on`), renders it and
/// returns the laser report.
///
/// Example: `let report = render_view(&world, frame, true, size, renderer, &mut pixels)?;`
fn render_view(
    world: &World,
    frame: DMat4,
    is_laser_on: bool,
    size_px: (u32, u32),
    time_s: f64,
    renderer: &mut dyn Renderer,
    pixels: &mut [u8],
) -> Result<Option<LaserReport>> {
    let mut data: SceneData =
        SceneData::Build(world, frame, &Beam::default(), size_px, time_s, true);
    let mut report: Option<LaserReport> = None;
    if is_laser_on {
        let (beam, laser_report) = physics::trace_laser(world, &data);
        data = SceneData::Build(world, frame, &beam, size_px, time_s, true);
        report = Some(laser_report);
    }
    renderer.render(&data, pixels)?;
    Ok(report)
}

/// `describe_laser` turns a laser report into one line for the HUD and results.
///
/// Example: `let text = describe_laser(&report);`
fn describe_laser(report: &LaserReport) -> String {
    let through: String = match report.wormhole_count {
        0 => String::new(),
        1 => "through 1 wormhole, ".to_string(),
        n => format!("through {n} wormholes, "),
    };
    match &report.hit {
        Some(hit) => format!("{through}hits {hit} after {:.1} blocks", report.length_m),
        None => format!("{through}hits nothing in {:.0} blocks", report.length_m),
    }
}

/// `View` is a named camera for the benchmark.
struct View {
    name: &'static str,
    frame: DMat4,
    is_laser_on: bool,
}

/// `make_bench_views` makes the benchmark's cameras: spawn, aimed at mouth 1A, right at its throat,
/// above the rails, high above spawn, and aimed at 1A with the laser on.
///
/// Example: `let views = make_bench_views(&world);`
fn make_bench_views(world: &World) -> Vec<View> {
    let r: f64 = world.curvature_radius_m;
    let spawn: Player = Player::AtSpawn(world);
    let mut aimed: Player = spawn.clone();
    physics::aim_through_mouth(world, &mut aimed.frame, &world.mouths[0]);
    let mut at_throat: Player = aimed.clone();
    at_throat.is_flying = true;
    for _ in 0..MAX_APPROACH_STEP_COUNT {
        if get_distance(at_throat.get_position(), world.mouths[0].center) * r
            < THROAT_VIEW_DISTANCE_M
        {
            break;
        }
        physics::fly_along_light(world, &mut at_throat, DVec3::NEG_Z, THROAT_APPROACH_STEP_M);
    }
    let above_rails: DMat4 = world.make_ground_frame(0.0, -12.0, 0.0)
        * make_boost(AXIS_Y, 1.1)
        * make_rotation(AXIS_Z, AXIS_Y, 1.2);
    let way_up: DMat4 = make_boost(AXIS_Y, 1.7) * make_rotation(AXIS_Z, AXIS_Y, 1.45);
    vec![
        View {
            name: "spawn",
            frame: spawn.frame,
            is_laser_on: false,
        },
        View {
            name: "aimed_at_1A",
            frame: aimed.frame,
            is_laser_on: false,
        },
        View {
            name: "at_throat_1A",
            frame: at_throat.frame,
            is_laser_on: false,
        },
        View {
            name: "above_rails",
            frame: above_rails,
            is_laser_on: false,
        },
        View {
            name: "fly_way_up",
            frame: way_up,
            is_laser_on: false,
        },
        View {
            name: "laser_through_1A",
            frame: aimed.frame,
            is_laser_on: true,
        },
    ]
}

/// `bench_views` renders each benchmark view `--frame-count` times without a window, reports the
/// median and fastest frame times, and saves each view as a PNG in the run folder.
///
/// Example: `bench_views(&arguments, renderer)?;`
fn bench_views(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
    let size_px: (u32, u32) = (arguments.width_px, arguments.height_px);
    let folder: PathBuf = make_run_folder("bench_views", arguments.label.as_deref())?;
    let mut pixels: Vec<u8> = vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL];
    let mut results: Vec<Value> = Vec::new();
    eprintln!(
        "Benchmarking {} views at {}x{} with the {} renderer...",
        6,
        size_px.0,
        size_px.1,
        renderer.name()
    );
    for view in make_bench_views(&world) {
        let report: Option<LaserReport> = render_view(
            &world,
            view.frame,
            view.is_laser_on,
            size_px,
            0.0,
            renderer.as_mut(),
            &mut pixels,
        )?;
        let mut times_ms: Vec<f64> = Vec::new();
        let mut gpu_times_ms: Vec<f64> = Vec::new();
        for _ in 0..arguments.frame_count {
            let start: Instant = Instant::now();
            render_view(
                &world,
                view.frame,
                view.is_laser_on,
                size_px,
                0.0,
                renderer.as_mut(),
                &mut pixels,
            )?;
            times_ms.push(start.elapsed().as_secs_f64() * 1000.0);
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
            "laser": report.as_ref().map(describe_laser),
        }));
    }
    let summary: Value = json!({
        "run_folder": folder.display().to_string(),
        "renderer": renderer.name(),
        "width_px": size_px.0,
        "height_px": size_px.1,
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

/// `Leg` is one part of the autopilot's route: where it starts, what it holds, and when it ends.
struct Leg {
    name: &'static str,
    is_reset_to_spawn: bool,
    is_flying: bool,
    aim_at_mouth: Option<&'static str>,
    input: Input,
    is_laser_on: bool,
    end: LegEnd,
    after_end_s: f64,
    max_s: f64,
}

/// `LegEnd` is what ends a leg: going through a wormhole, landing on the ground, or time.
#[derive(Clone, Copy, PartialEq)]
enum LegEnd {
    Crossing,
    Landing,
    Time,
}

/// `make_route` makes the autopilot's route: walk into mouth 1A to the house, use the jetpack and
/// land, fly into 2A to the red pylon, walk into 3A and fall from above the rails, and fire the
/// laser through 1A.
///
/// Example: `let route = make_route();`
fn make_route() -> Vec<Leg> {
    let forward: Input = Input {
        forward: true,
        ..Input::default()
    };
    vec![
        Leg {
            name: "walk_into_1A",
            is_reset_to_spawn: true,
            is_flying: false,
            aim_at_mouth: Some("1A"),
            input: forward,
            is_laser_on: false,
            end: LegEnd::Crossing,
            after_end_s: 1.0,
            max_s: 10.0,
        },
        Leg {
            name: "jetpack_and_land",
            is_reset_to_spawn: false,
            is_flying: false,
            aim_at_mouth: None,
            input: Input {
                up: true,
                ..Input::default()
            },
            is_laser_on: false,
            end: LegEnd::Time,
            after_end_s: 0.0,
            max_s: 1.0,
        },
        Leg {
            name: "fall_after_jetpack",
            is_reset_to_spawn: false,
            is_flying: false,
            aim_at_mouth: None,
            input: Input::default(),
            is_laser_on: false,
            end: LegEnd::Landing,
            after_end_s: 0.5,
            max_s: 5.0,
        },
        Leg {
            name: "fly_into_2A",
            is_reset_to_spawn: true,
            is_flying: true,
            aim_at_mouth: Some("2A"),
            input: forward,
            is_laser_on: false,
            end: LegEnd::Crossing,
            after_end_s: 1.0,
            max_s: 10.0,
        },
        Leg {
            name: "walk_into_3A",
            is_reset_to_spawn: true,
            is_flying: false,
            aim_at_mouth: Some("3A"),
            input: forward,
            is_laser_on: false,
            end: LegEnd::Crossing,
            after_end_s: 0.0,
            max_s: 10.0,
        },
        Leg {
            name: "fall_from_3B",
            is_reset_to_spawn: false,
            is_flying: false,
            aim_at_mouth: None,
            input: Input::default(),
            is_laser_on: false,
            end: LegEnd::Landing,
            after_end_s: 0.5,
            max_s: 6.0,
        },
        Leg {
            name: "laser_through_1A",
            is_reset_to_spawn: true,
            is_flying: false,
            aim_at_mouth: Some("1A"),
            input: Input::default(),
            is_laser_on: true,
            end: LegEnd::Time,
            after_end_s: 0.0,
            max_s: 0.5,
        },
    ]
}

/// `run_autopilot` plays the scripted route without a window at a fixed 1/60 s time step,
/// rendering every frame. It records each frame (time, render time, height, events) in
/// `frames.csv`, saves every n-th frame as a PNG, and reports each leg: how long it took, whether
/// it ended as planned, and its frame rate.
///
/// Example: `run_autopilot(&arguments, renderer)?;`
fn run_autopilot(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
    let size_px: (u32, u32) = (arguments.width_px, arguments.height_px);
    let folder: PathBuf = make_run_folder("autopilot", arguments.label.as_deref())?;
    let mut pixels: Vec<u8> = vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL];
    let mut player: Player = Player::AtSpawn(&world);
    let mut csv: String =
        String::from("frame,time_s,leg,render_ms,height_m,is_flying,is_on_ground,event\n");
    let mut legs: Vec<Value> = Vec::new();
    let mut frame_index: u64 = 0;
    eprintln!(
        "Running the autopilot at {}x{} with the {} renderer...",
        size_px.0,
        size_px.1,
        renderer.name()
    );
    for leg in make_route() {
        if leg.is_reset_to_spawn {
            player = Player::AtSpawn(&world);
        }
        player.is_flying = leg.is_flying;
        if let Some(name) = leg.aim_at_mouth {
            let mouth = world
                .mouths
                .iter()
                .find(|m| m.name == name)
                .ok_or(format!("no mouth named {name}"))?;
            physics::aim_through_mouth(&world, &mut player.frame, mouth);
        }
        let (
            mut leg_time_s,
            mut ended_at_s,
            mut events,
            mut render_total_ms,
            mut leg_frame_count,
            mut max_height_m,
        ) = (
            0.0f64,
            None::<f64>,
            Vec::<String>::new(),
            0.0f64,
            0u64,
            0.0f64,
        );
        let mut laser_text: Option<String> = None;
        while leg_time_s < leg.max_s && ended_at_s.is_none_or(|t| leg_time_s < t + leg.after_end_s)
        {
            let input: Input = if ended_at_s.is_some() && leg.end == LegEnd::Landing {
                Input::default()
            } else {
                leg.input
            };
            let messages: Vec<String> =
                physics::update_player(&world, &mut player, &input, AUTOPILOT_DT_S);
            physics::level_out(&mut player.frame, AUTOPILOT_DT_S);
            leg_time_s += AUTOPILOT_DT_S;
            let is_crossing: bool = !messages.is_empty();
            if ended_at_s.is_none()
                && ((leg.end == LegEnd::Crossing && is_crossing)
                    || (leg.end == LegEnd::Landing && player.is_on_ground))
            {
                ended_at_s = Some(leg_time_s);
            }
            events.extend(messages.iter().cloned());
            let start: Instant = Instant::now();
            let report: Option<LaserReport> = render_view(
                &world,
                player.frame,
                leg.is_laser_on,
                size_px,
                frame_index as f64 * AUTOPILOT_DT_S,
                renderer.as_mut(),
                &mut pixels,
            )?;
            let render_ms: f64 = start.elapsed().as_secs_f64() * 1000.0;
            if let Some(report) = report {
                laser_text = Some(describe_laser(&report));
            }
            render_total_ms += render_ms;
            leg_frame_count += 1;
            max_height_m = max_height_m.max(player.get_height_m(&world));
            csv.push_str(&format!(
                "{frame_index},{:.4},{},{render_ms:.2},{:.3},{},{},{}\n",
                frame_index as f64 * AUTOPILOT_DT_S,
                leg.name,
                player.get_height_m(&world),
                player.is_flying,
                player.is_on_ground,
                messages.join(" / "),
            ));
            if frame_index.is_multiple_of(u64::from(arguments.snapshot_every)) || is_crossing {
                save_png(
                    &folder.join(format!("frame_{frame_index:05}_{}.png", leg.name)),
                    &pixels,
                    size_px.0,
                    size_px.1,
                )?;
            }
            frame_index += 1;
        }
        let is_as_planned: bool = leg.end == LegEnd::Time || ended_at_s.is_some();
        legs.push(json!({
            "leg": leg.name,
            "is_as_planned": is_as_planned,
            "ended_at_s": ended_at_s,
            "frame_count": leg_frame_count,
            "mean_render_ms": render_total_ms / leg_frame_count.max(1) as f64,
            "max_height_m": max_height_m,
            "events": events,
            "laser": laser_text,
        }));
    }
    fs::write(folder.join("frames.csv"), csv)?;
    let summary: Value = json!({
        "run_folder": folder.display().to_string(),
        "renderer": renderer.name(),
        "width_px": size_px.0,
        "height_px": size_px.1,
        "legs": legs,
    });
    fs::write(
        folder.join("results.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    if arguments.json {
        println!("{summary}");
    } else {
        println!(
            "{:<20} {:>7} {:>9} {:>8} {:>9}  events",
            "leg", "planned", "ended s", "fps", "max h m"
        );
        for leg in &legs {
            let mean_ms: f64 = leg["mean_render_ms"].as_f64().unwrap_or_default();
            println!(
                "{:<20} {:>7} {:>9} {:>8.1} {:>9.1}  {}{}",
                leg["leg"].as_str().unwrap_or_default(),
                if leg["is_as_planned"].as_bool().unwrap_or_default() {
                    "yes"
                } else {
                    "NO"
                },
                leg["ended_at_s"]
                    .as_f64()
                    .map_or("-".to_string(), |t| format!("{t:.2}")),
                1000.0 / mean_ms.max(1e-9),
                leg["max_height_m"].as_f64().unwrap_or_default(),
                leg["events"]
                    .as_array()
                    .map(|e| e
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("; "))
                    .unwrap_or_default(),
                leg["laser"]
                    .as_str()
                    .map(|t| format!(" laser {t}"))
                    .unwrap_or_default(),
            );
        }
        println!("Run folder: {}", folder.display());
    }
    Ok(())
}

/// `Hud` is what the window shows over the picture.
struct Hud {
    fps: f64,
    render_ms: f64,
    gpu_ms: Option<f64>,
    message: String,
    message_time_s: f64,
    laser: Option<String>,
}

/// `play` opens the window and runs the demo until it is closed: mouse to look (click to capture,
/// Esc to release, Esc again to quit), WASD to move, Space to jump or jetpack, H to fly, G to aim
/// through a mouth, L for the laser, R to respawn, - and = for render scale.
///
/// Example: `play(&arguments, renderer)?;`
fn play(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
    let sdl: sdl3::Sdl = sdl3::init()?;
    let video: sdl3::VideoSubsystem = sdl.video()?;
    let window: sdl3::video::Window = video
        .window("Hyperbolic wormholes (Rust)", WINDOW_WIDTH, WINDOW_HEIGHT)
        .high_pixel_density()
        .resizable()
        .position_centered()
        .build()?;
    let mouse: sdl3::mouse::MouseUtil = sdl.mouse();
    let mut canvas: sdl3::render::WindowCanvas = window.into_canvas();
    let texture_creator: sdl3::render::TextureCreator<sdl3::video::WindowContext> =
        canvas.texture_creator();
    let mut texture: Option<(Texture, (u32, u32))> = None;
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let mut player: Player = Player::AtSpawn(&world);
    let mut render_scale: f64 = arguments
        .render_scale
        .unwrap_or(get_default_render_scale(arguments.renderer));
    let (mut is_laser_on, mut is_auto_forward, mut is_captured) = (false, false, false);
    let mut hud: Hud = Hud {
        fps: 0.0,
        render_ms: 0.0,
        gpu_ms: None,
        message: String::new(),
        message_time_s: 0.0,
        laser: None,
    };
    let (start, mut last) = (Instant::now(), Instant::now());
    let (mut hud_time_s, mut hud_frames, mut hud_render_ms) = (0.0f64, 0u32, 0.0f64);
    let mut pixels: Vec<u8> = Vec::new();
    'running: loop {
        let dt_s: f64 = last.elapsed().as_secs_f64().min(MAX_FRAME_DT_S);
        last = Instant::now();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    if !is_captured {
                        break 'running;
                    }
                    is_captured = false;
                    mouse.set_relative_mouse_mode(canvas.window(), false);
                }
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => match key {
                    Keycode::H => {
                        player.is_flying = !player.is_flying;
                        player.velocity = glam::DVec4::ZERO;
                    }
                    Keycode::L => is_laser_on = !is_laser_on,
                    Keycode::F => is_auto_forward = !is_auto_forward,
                    Keycode::R => player = Player::AtSpawn(&world),
                    Keycode::G => {
                        if let Some(k) = physics::find_aimed_mouth(&world, player.frame) {
                            physics::aim_through_mouth(&world, &mut player.frame, &world.mouths[k]);
                        }
                    }
                    Keycode::Minus => {
                        render_scale = (render_scale / RENDER_SCALE_STEP).max(MIN_RENDER_SCALE)
                    }
                    Keycode::Equals => render_scale = (render_scale * RENDER_SCALE_STEP).min(1.0),
                    _ => {}
                },
                Event::MouseButtonDown { .. } if !is_captured => {
                    is_captured = true;
                    mouse.set_relative_mouse_mode(canvas.window(), true);
                }
                Event::MouseMotion { xrel, yrel, .. } if is_captured => {
                    physics::look_around(&mut player.frame, f64::from(xrel), f64::from(yrel));
                }
                _ => {}
            }
        }
        let keys: sdl3::keyboard::KeyboardState = event_pump.keyboard_state();
        let input: Input = Input {
            forward: keys.is_scancode_pressed(Scancode::W) || is_auto_forward,
            back: keys.is_scancode_pressed(Scancode::S),
            left: keys.is_scancode_pressed(Scancode::A),
            right: keys.is_scancode_pressed(Scancode::D),
            up: keys.is_scancode_pressed(Scancode::Space) || keys.is_scancode_pressed(Scancode::E),
            down: keys.is_scancode_pressed(Scancode::Q),
            sprint: keys.is_scancode_pressed(Scancode::LShift)
                || keys.is_scancode_pressed(Scancode::RShift),
        };
        for message in physics::update_player(&world, &mut player, &input, dt_s) {
            hud.message = message;
            hud.message_time_s = MESSAGE_DURATION_S;
        }
        physics::level_out(&mut player.frame, dt_s);
        let window_px: (u32, u32) = canvas.window().size_in_pixels();
        let size_px: (u32, u32) = (
            ((f64::from(window_px.0) * render_scale) as u32).max(MIN_IMAGE_SIDE_PX),
            ((f64::from(window_px.1) * render_scale) as u32).max(MIN_IMAGE_SIDE_PX),
        );
        pixels.resize(size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL, 0);
        let render_start: Instant = Instant::now();
        let report: Option<LaserReport> = render_view(
            &world,
            player.frame,
            is_laser_on,
            size_px,
            start.elapsed().as_secs_f64(),
            renderer.as_mut(),
            &mut pixels,
        )?;
        hud_render_ms += render_start.elapsed().as_secs_f64() * 1000.0;
        hud.laser = report.as_ref().map(describe_laser);
        hud.gpu_ms = renderer.gpu_ms();
        if texture.as_ref().is_none_or(|(_, size)| *size != size_px) {
            texture = Some((
                texture_creator.create_texture_streaming(
                    PixelFormat::RGBA32,
                    size_px.0,
                    size_px.1,
                )?,
                size_px,
            ));
        }
        let (current_texture, _) = texture.as_mut().expect("the texture was just made");
        current_texture.update(None, &pixels, size_px.0 as usize * BYTES_PER_PIXEL)?;
        canvas.set_scale(1.0, 1.0)?;
        canvas.copy(current_texture, None, None)?;
        hud_frames += 1;
        hud_time_s += dt_s;
        hud.message_time_s -= dt_s;
        if hud_time_s > HUD_PERIOD_S {
            hud.fps = f64::from(hud_frames) / hud_time_s;
            hud.render_ms = hud_render_ms / f64::from(hud_frames);
            (hud_time_s, hud_frames, hud_render_ms) = (0.0, 0, 0.0);
        }
        draw_hud(
            &mut canvas,
            &hud,
            &player,
            &world,
            renderer.name(),
            size_px,
            is_captured,
        )?;
        canvas.present();
    }
    Ok(())
}

/// `draw_hud` draws the stats and controls over the picture with SDL's built-in text.
///
/// Example: `draw_hud(&mut canvas, &hud, &player, &world, "cpu", (640, 400), true)?;`
fn draw_hud(
    canvas: &mut sdl3::render::WindowCanvas,
    hud: &Hud,
    player: &Player,
    world: &World,
    renderer_name: &str,
    size_px: (u32, u32),
    is_captured: bool,
) -> Result<()> {
    let density: f32 = canvas.window().pixel_density();
    canvas.set_scale(density * HUD_TEXT_SCALE, density * HUD_TEXT_SCALE)?;
    canvas.set_draw_color(HUD_COLOR);
    let mut lines: Vec<String> = vec![
        format!(
            "{:.0} fps, {:.1} ms render{}, {renderer_name} at {}x{}",
            hud.fps,
            hud.render_ms,
            hud.gpu_ms
                .map_or(String::new(), |ms| format!(" ({ms:.1} ms on the GPU)")),
            size_px.0,
            size_px.1
        ),
        format!(
            "{}, height {:.1} blocks",
            if player.is_flying {
                "flying"
            } else if player.is_on_ground {
                "walking"
            } else {
                "in the air"
            },
            player.get_height_m(world)
        ),
    ];
    if let Some(laser) = &hud.laser {
        lines.push(format!("laser {laser}"));
    }
    if hud.message_time_s > 0.0 {
        lines.push(hud.message.clone());
    }
    lines.push(if is_captured {
        "WASD move, Space jump/jetpack, H fly, G aim, L laser, R respawn, -/= scale, Esc release"
            .to_string()
    } else {
        "Click to capture the mouse; Esc quits".to_string()
    });
    for (i, line) in lines.iter().enumerate() {
        canvas.draw_debug_text(line.as_str(), (4.0, 4.0 + HUD_LINE_PX * i as f32))?;
    }
    Ok(())
}
