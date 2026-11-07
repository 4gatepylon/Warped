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
//! - `--exit-after-s <seconds>`: close the window after that long and print its frame rate.
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
use objc2_core_foundation::CGSize;
use objc2_metal::MTLPixelFormat;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Texture;
use serde_json::{Value, json};

use geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, make_boost, make_rotation};
use physics::{Input, LaserReport, Player};
use scene_data::{Beam, SceneData};
use world::{DEFAULT_CURVATURE_RADIUS_M, World};

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;
const CPU_RENDER_SCALE: f64 = 0.35;
const METAL_RENDER_SCALE: f64 = 1.0;
const MIN_AUTO_RENDER_SCALE: f64 = 0.5;
const AUTO_SCALE_TARGET_GPU_MS: f64 = 14.0;
const AUTO_SCALE_MAX_STEP: f64 = 1.02;
const AUTO_SCALE_MAX_DROP: f64 = 0.8;
const RENDER_SCALE_QUANTUM: f64 = 0.05;
const RENDER_SCALE_ROUNDING: f64 = 1e-6;
const RENDER_SCALE_STEP: f64 = 1.25;
const MIN_RENDER_SCALE: f64 = 0.05;
const MAX_FRAME_DT_S: f64 = 0.05;
const AUTOPILOT_DT_S: f64 = 1.0 / 60.0;
const HUD_PERIOD_S: f64 = 0.25;
const ROUTE_WARMUP_FRAME_COUNT: usize = 5;
const TOUR_FRAMES_PER_MOUTH: usize = 300;
const TOUR_FAR_M: f64 = 5.0;
const TOUR_NEAR_M: f64 = 0.45;
const TOUR_YAW_RAD: f64 = 0.8;
const TOUR_PITCH_RAD: f64 = 0.4;
const TOUR_YAW_TURNS: f64 = 2.0;
const TOUR_PITCH_TURNS: f64 = 3.0;
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
const HUD_TEXT_SCALE: f32 = 2.0;
const HUD_LINE_PX: f32 = 10.0;
const HUD_MARGIN_PX: f32 = 4.0;
const WINDOW_TITLE: &str = "Hyperbolic wormholes (Rust)";
const MS_PER_S: f64 = 1000.0;
const HUD_COLOR: Color = Color::RGB(235, 240, 250);
const MESSAGE_DURATION_S: f64 = 3.0;
const MIN_IMAGE_SIDE_PX: u32 = 16;
const MAX_IMAGE_SIDE_PX: u32 = 8192;
const THROAT_VIEW_DISTANCE_M: f64 = 0.8;
const NEAR_VIEW_DISTANCE_M: f64 = 2.5;
const SIDE_VIEW_DISTANCE_M: f64 = 0.4;
const SIDE_VIEW_YAW_RAD: f64 = std::f64::consts::FRAC_PI_2;
const THROAT_APPROACH_STEP_M: f64 = 0.1;
const MAX_APPROACH_STEP_COUNT: usize = 2000;
const SECONDS_PER_DAY: u64 = 86_400;

/// `RendererKind` is which renderer draws the frames.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RendererKind {
    Cpu,
    Metal,
}

/// `Sampling` is which pixels Metal traces: every other pixel each way, interpolating the rest
/// where that is exact enough, or every pixel.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Sampling {
    Adaptive,
    EveryPixel,
}

/// `RouteKind` is which route `--play-route` plays: the scripted legs, or a tour around every
/// mouth from 5 blocks to just outside its throat and back.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RouteKind {
    Legs,
    MouthTour,
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
    /// Which pixels Metal traces: every other pixel each way, interpolating the others where the
    /// image is smooth enough (adaptive), or every pixel (every-pixel).
    #[arg(long, value_enum, default_value_t = Sampling::Adaptive)]
    sampling: Sampling,
    /// Fraction of the output's pixels per side to ray-trace, upscaled with MetalFX (Metal) or by
    /// repeating pixels (CPU). In the window it defaults to 0.35 on the CPU, and with Metal it
    /// starts at 1 and lowers itself (to no less than 0.5) when a frame takes more than about
    /// 14 ms of GPU time; otherwise 1.
    #[arg(long)]
    render_scale: Option<f64>,
    /// Render fixed views without a window, time them and save them under .artifacts/bench_views/.
    #[arg(long, conflicts_with = "autopilot")]
    bench_views: bool,
    /// Play a scripted route without a window and save frames and timings under .artifacts/autopilot/.
    #[arg(long)]
    autopilot: bool,
    /// Image width for --bench-views and --autopilot, and the window's starting width (in screen
    /// points, which are pixels on a 1x display).
    #[arg(long, default_value_t = 1280)]
    width_px: u32,
    /// Image height for --bench-views and --autopilot, and the window's starting height.
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
    /// Close the window after this many seconds and print its frame rate (to check the window).
    #[arg(long)]
    exit_after_s: Option<f64>,
    /// Play the scripted route in the window, one 1/60 s step per frame, instead of taking input,
    /// then close it and print frame-time statistics (to check the window's frame rate in motion).
    #[arg(long, conflicts_with_all = ["bench_views", "autopilot"])]
    play_route: bool,
    /// Which route --play-route plays: the scripted legs, or a tour circling every mouth from 5
    /// blocks to just outside its throat and back while the view swings around.
    #[arg(long, value_enum, default_value_t = RouteKind::Legs)]
    route: RouteKind,
}

/// `Renderer` draws one frame's packed scene into an RGBA pixel buffer.
trait Renderer {
    /// `render` draws `data` (ray-traced at its own render size) into `pixels` at `output_px`
    /// (top row first, 4 bytes per pixel), upscaling it if `output_px` is larger.
    ///
    /// Example: `renderer.render(&data, (2560, 1600), &mut pixels)?;`
    fn render(&mut self, data: &SceneData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()>;

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

    /// `as_metal` gives the Metal renderer, if this is it, so a window can present straight from
    /// the GPU.
    ///
    /// Example: `if let Some(metal) = renderer.as_metal() { present(metal); }`
    fn as_metal(&mut self) -> Option<&mut render_metal::MetalRenderer> {
        None
    }
}

/// `CpuRenderer` is the rayon ray tracer of `render_cpu.rs`.
struct CpuRenderer;

impl Renderer for CpuRenderer {
    fn render(&mut self, data: &SceneData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()> {
        let render_px: (u32, u32) = (
            data.uniforms.width_px as u32,
            data.uniforms.height_px as u32,
        );
        if render_px == output_px {
            render_cpu::render(data, pixels);
            return Ok(());
        }
        let mut rendered: Vec<u8> =
            vec![0; render_px.0 as usize * render_px.1 as usize * BYTES_PER_PIXEL];
        render_cpu::render(data, &mut rendered);
        for (y, line) in pixels
            .chunks_exact_mut(output_px.0 as usize * BYTES_PER_PIXEL)
            .enumerate()
        {
            let source_y: usize = y * render_px.1 as usize / output_px.1 as usize;
            for x in 0..output_px.0 as usize {
                let source: usize = (source_y * render_px.0 as usize
                    + x * render_px.0 as usize / output_px.0 as usize)
                    * BYTES_PER_PIXEL;
                line[x * BYTES_PER_PIXEL..(x + 1) * BYTES_PER_PIXEL]
                    .copy_from_slice(&rendered[source..source + BYTES_PER_PIXEL]);
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "cpu"
    }
}

/// `make_renderer` makes the renderer the command line asked for.
///
/// Example: `let mut renderer = make_renderer(RendererKind::Cpu, Sampling::Adaptive)?;`
fn make_renderer(kind: RendererKind, sampling: Sampling) -> Result<Box<dyn Renderer>> {
    match kind {
        RendererKind::Cpu => Ok(Box::new(CpuRenderer)),
        RendererKind::Metal => Ok(Box::new(render_metal::MetalRenderer::New(
            sampling == Sampling::Adaptive,
        )?)),
    }
}

impl Renderer for render_metal::MetalRenderer {
    fn render(&mut self, data: &SceneData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()> {
        self.render_to_pixels(data, (output_px.0 as usize, output_px.1 as usize), pixels)
    }

    fn as_metal(&mut self) -> Option<&mut render_metal::MetalRenderer> {
        Some(self)
    }

    fn name(&self) -> &'static str {
        "metal"
    }

    fn gpu_ms(&self) -> Option<f64> {
        Some(self.last_gpu_ms)
    }
}

/// `get_sampling_name` names which pixels the renderer traces, for results: the CPU always traces
/// every pixel.
///
/// Example: `let name = get_sampling_name(&arguments);`
fn get_sampling_name(arguments: &Arguments) -> &'static str {
    match (arguments.renderer, arguments.sampling) {
        (RendererKind::Metal, Sampling::Adaptive) => "adaptive",
        _ => "every-pixel",
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
    if let Some(seconds) = arguments.exit_after_s
        && (seconds.is_nan() || seconds <= 0.0)
    {
        return Err(format!(
            "--exit-after-s must be positive, got {seconds}; e.g. --exit-after-s 10"
        )
        .into());
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
    let renderer: Box<dyn Renderer> = make_renderer(arguments.renderer, arguments.sampling)?;
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

/// `pack_view` packs the scene for a camera at a render size, with the laser if `is_laser_on`
/// (traced through the packed scene, then packed again with its beam), and returns the laser's
/// report.
///
/// Example: `let (data, report) = pack_view(&world, frame, true, (1280, 800), 0.0);`
fn pack_view(
    world: &World,
    frame: DMat4,
    is_laser_on: bool,
    render_px: (u32, u32),
    time_s: f64,
) -> (SceneData, Option<LaserReport>) {
    let data: SceneData = SceneData::Build(world, frame, &Beam::default(), render_px, time_s, true);
    if !is_laser_on {
        return (data, None);
    }
    let (beam, report) = physics::trace_laser(world, &data);
    (
        SceneData::Build(world, frame, &beam, render_px, time_s, true),
        Some(report),
    )
}

/// `render_view` packs and renders the scene for a camera at `render_px`, delivered at
/// `output_px`, and returns the laser report.
///
/// Example: `let report = render_view(&world, frame, true, (1280, 800), (2560, 1600), 0.0, renderer, &mut pixels)?;`
#[allow(clippy::too_many_arguments)]
fn render_view(
    world: &World,
    frame: DMat4,
    is_laser_on: bool,
    render_px: (u32, u32),
    output_px: (u32, u32),
    time_s: f64,
    renderer: &mut dyn Renderer,
    pixels: &mut [u8],
) -> Result<Option<LaserReport>> {
    let (data, report) = pack_view(world, frame, is_laser_on, render_px, time_s);
    renderer.render(&data, output_px, pixels)?;
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

/// `make_bench_views` makes the benchmark's cameras: spawn, aimed at mouth 1A, a few blocks from it,
/// right at its throat, above the rails, high above spawn, looking sideways just outside 1A's
/// throat (where some rays stay trapped circling it), and aimed at 1A with the laser on.
///
/// Example: `let views = make_bench_views(&world);`
fn make_bench_views(world: &World) -> Vec<View> {
    let r: f64 = world.curvature_radius_m;
    let spawn: Player = Player::AtSpawn(world);
    let mut aimed: Player = spawn.clone();
    physics::aim_through_mouth(world, &mut aimed.frame, &world.mouths[0]);
    let approach = |distance_m: f64| -> DMat4 {
        let mut player: Player = aimed.clone();
        player.is_flying = true;
        for _ in 0..MAX_APPROACH_STEP_COUNT {
            if get_distance(player.get_position(), world.mouths[0].center) * r < distance_m {
                break;
            }
            physics::fly_along_light(world, &mut player, DVec3::NEG_Z, THROAT_APPROACH_STEP_M);
        }
        player.frame
    };
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
            name: "near_1A",
            frame: approach(NEAR_VIEW_DISTANCE_M),
            is_laser_on: false,
        },
        View {
            name: "at_throat_1A",
            frame: approach(THROAT_VIEW_DISTANCE_M),
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
            name: "side_at_throat_1A",
            frame: approach(SIDE_VIEW_DISTANCE_M)
                * make_rotation(AXIS_Z, AXIS_X, SIDE_VIEW_YAW_RAD),
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
    let render_px: (u32, u32) = scale_size(size_px, arguments.render_scale.unwrap_or(1.0));
    let folder: PathBuf = make_run_folder("bench_views", arguments.label.as_deref())?;
    let mut pixels: Vec<u8> = vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL];
    let mut results: Vec<Value> = Vec::new();
    let views: Vec<View> = make_bench_views(&world);
    eprintln!(
        "Benchmarking {} views at {}x{} (rendered at {}x{}) with the {} renderer...",
        views.len(),
        size_px.0,
        size_px.1,
        render_px.0,
        render_px.1,
        renderer.name()
    );
    for view in views {
        let report: Option<LaserReport> = render_view(
            &world,
            view.frame,
            view.is_laser_on,
            render_px,
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
                render_px,
                size_px,
                0.0,
                renderer.as_mut(),
                &mut pixels,
            )?;
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
            "laser": report.as_ref().map(describe_laser),
        }));
    }
    let summary: Value = json!({
        "run_folder": folder.display().to_string(),
        "renderer": renderer.name(),
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

/// `RouteFrame` is one `AUTOPILOT_DT_S` step of the scripted route: its leg, the player's camera
/// and state after the step, and the messages (wormhole crossings) the step produced.
struct RouteFrame {
    leg: usize,
    frame: DMat4,
    is_laser_on: bool,
    height_m: f64,
    is_flying: bool,
    is_on_ground: bool,
    messages: Vec<String>,
}

/// `Route` is the scripted route played without rendering: its legs, every step, and when each
/// leg's ending event happened.
struct Route {
    legs: Vec<Leg>,
    frames: Vec<RouteFrame>,
    ended_at_s: Vec<Option<f64>>,
}

/// `simulate_route` plays the scripted route's physics at a fixed `AUTOPILOT_DT_S` step, without
/// rendering.
///
/// Example: `let route = simulate_route(&world)?;`
fn simulate_route(world: &World) -> Result<Route> {
    let legs: Vec<Leg> = make_route();
    let mut player: Player = Player::AtSpawn(world);
    let mut frames: Vec<RouteFrame> = Vec::new();
    let mut ended: Vec<Option<f64>> = Vec::new();
    for (index, leg) in legs.iter().enumerate() {
        if leg.is_reset_to_spawn {
            player = Player::AtSpawn(world);
        }
        player.is_flying = leg.is_flying;
        if let Some(name) = leg.aim_at_mouth {
            let mouth = world
                .mouths
                .iter()
                .find(|m| m.name == name)
                .ok_or(format!("no mouth named {name}"))?;
            physics::aim_through_mouth(world, &mut player.frame, mouth);
        }
        let (mut leg_time_s, mut ended_at_s): (f64, Option<f64>) = (0.0, None);
        while leg_time_s < leg.max_s && ended_at_s.is_none_or(|t| leg_time_s < t + leg.after_end_s)
        {
            let input: Input = if ended_at_s.is_some() && leg.end == LegEnd::Landing {
                Input::default()
            } else {
                leg.input
            };
            let messages: Vec<String> =
                physics::update_player(world, &mut player, &input, AUTOPILOT_DT_S);
            physics::level_out(&mut player.frame, AUTOPILOT_DT_S);
            leg_time_s += AUTOPILOT_DT_S;
            if ended_at_s.is_none()
                && ((leg.end == LegEnd::Crossing && !messages.is_empty())
                    || (leg.end == LegEnd::Landing && player.is_on_ground))
            {
                ended_at_s = Some(leg_time_s);
            }
            frames.push(RouteFrame {
                leg: index,
                frame: player.frame,
                is_laser_on: leg.is_laser_on,
                height_m: player.get_height_m(world),
                is_flying: player.is_flying,
                is_on_ground: player.is_on_ground,
                messages,
            });
        }
        ended.push(ended_at_s);
    }
    Ok(Route {
        legs,
        frames,
        ended_at_s: ended,
    })
}

/// `make_mouth_tour` makes a tour of every mouth: for each, the eye circles it once at its own
/// height, from `TOUR_FAR_M` blocks to `TOUR_NEAR_M` (inside its jump sphere, just outside the
/// throat) and back, looking at it with the view swinging up to `TOUR_YAW_RAD` sideways and
/// `TOUR_PITCH_RAD` up and down, over `TOUR_FRAMES_PER_MOUTH` frames.
///
/// Example: `let tour = make_mouth_tour(&world);`
fn make_mouth_tour(world: &World) -> Vec<RouteFrame> {
    let r: f64 = world.curvature_radius_m;
    let mut frames: Vec<RouteFrame> = Vec::new();
    for (k, mouth) in world.mouths.iter().enumerate() {
        for n in 0..TOUR_FRAMES_PER_MOUTH {
            let turn: f64 = std::f64::consts::TAU * n as f64 / (TOUR_FRAMES_PER_MOUTH - 1) as f64;
            let distance_m: f64 =
                TOUR_NEAR_M + (TOUR_FAR_M - TOUR_NEAR_M) * (0.5 + 0.5 * turn.cos());
            let frame: DMat4 = mouth.frame
                * make_rotation(AXIS_X, AXIS_Z, turn)
                * make_boost(AXIS_Z, distance_m / r)
                * make_rotation(AXIS_X, AXIS_Z, TOUR_YAW_RAD * (TOUR_YAW_TURNS * turn).sin())
                * make_rotation(
                    AXIS_Y,
                    AXIS_Z,
                    TOUR_PITCH_RAD * (TOUR_PITCH_TURNS * turn).sin(),
                );
            frames.push(RouteFrame {
                leg: k,
                frame,
                is_laser_on: false,
                height_m: r * frame.col(AXIS_W)[AXIS_Y].asinh(),
                is_flying: true,
                is_on_ground: false,
                messages: Vec::new(),
            });
        }
    }
    frames
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
    let render_px: (u32, u32) = scale_size(size_px, arguments.render_scale.unwrap_or(1.0));
    let folder: PathBuf = make_run_folder("autopilot", arguments.label.as_deref())?;
    let mut pixels: Vec<u8> = vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL];
    let mut csv: String =
        String::from("frame,time_s,leg,render_ms,height_m,is_flying,is_on_ground,event\n");
    eprintln!(
        "Running the autopilot at {}x{} with the {} renderer...",
        size_px.0,
        size_px.1,
        renderer.name()
    );
    let Route {
        legs: route,
        frames,
        ended_at_s: ended,
    } = simulate_route(&world)?;
    let mut render_totals_ms: Vec<f64> = vec![0.0; route.len()];
    let mut frame_counts: Vec<u64> = vec![0; route.len()];
    let mut max_heights_m: Vec<f64> = vec![0.0; route.len()];
    let mut events: Vec<Vec<String>> = vec![Vec::new(); route.len()];
    let mut laser_texts: Vec<Option<String>> = vec![None; route.len()];
    for (frame_index, step) in frames.iter().enumerate() {
        let leg: &Leg = &route[step.leg];
        let start: Instant = Instant::now();
        let report: Option<LaserReport> = render_view(
            &world,
            step.frame,
            step.is_laser_on,
            render_px,
            size_px,
            frame_index as f64 * AUTOPILOT_DT_S,
            renderer.as_mut(),
            &mut pixels,
        )?;
        let render_ms: f64 = start.elapsed().as_secs_f64() * MS_PER_S;
        if let Some(report) = report {
            laser_texts[step.leg] = Some(describe_laser(&report));
        }
        render_totals_ms[step.leg] += render_ms;
        frame_counts[step.leg] += 1;
        max_heights_m[step.leg] = max_heights_m[step.leg].max(step.height_m);
        events[step.leg].extend(step.messages.iter().cloned());
        csv.push_str(&format!(
            "{frame_index},{:.4},{},{render_ms:.2},{:.3},{},{},{}\n",
            frame_index as f64 * AUTOPILOT_DT_S,
            leg.name,
            step.height_m,
            step.is_flying,
            step.is_on_ground,
            step.messages.join(" / "),
        ));
        if (frame_index as u64).is_multiple_of(u64::from(arguments.snapshot_every))
            || !step.messages.is_empty()
        {
            save_png(
                &folder.join(format!("frame_{frame_index:05}_{}.png", leg.name)),
                &pixels,
                size_px.0,
                size_px.1,
            )?;
        }
    }
    let legs: Vec<Value> = route
        .iter()
        .enumerate()
        .map(|(k, leg)| {
            json!({
                "leg": leg.name,
                "is_as_planned": leg.end == LegEnd::Time || ended[k].is_some(),
                "ended_at_s": ended[k],
                "frame_count": frame_counts[k],
                "mean_render_ms": render_totals_ms[k] / frame_counts[k].max(1) as f64,
                "max_height_m": max_heights_m[k],
                "events": events[k],
                "laser": laser_texts[k],
            })
        })
        .collect();
    fs::write(folder.join("frames.csv"), csv)?;
    let summary: Value = json!({
        "run_folder": folder.display().to_string(),
        "renderer": renderer.name(),
        "sampling": get_sampling_name(arguments),
        "width_px": size_px.0,
        "height_px": size_px.1,
        "render_width_px": render_px.0,
        "render_height_px": render_px.1,
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

/// `Session` is the state of a played window: the world, the player, the toggles and the HUD's
/// running averages.
struct Session {
    world: World,
    player: Player,
    render_scale: f64,
    last_used_scale: f64,
    is_auto_scale: bool,
    exit_after_s: Option<f64>,
    route: Vec<RouteFrame>,
    route_index: usize,
    frame_times_ms: Vec<f64>,
    render_scales: Vec<f64>,
    total_frame_count: u64,
    total_gpu_ms: f64,
    last_output_px: (u32, u32),
    is_laser_on: bool,
    is_auto_forward: bool,
    is_captured: bool,
    message: String,
    message_time_s: f64,
    laser: Option<String>,
    fps: f64,
    render_ms: f64,
    gpu_ms: Option<f64>,
    start: Instant,
    last: Instant,
    hud_time_s: f64,
    hud_frame_count: u32,
    hud_render_ms: f64,
}

impl Session {
    /// `New` starts a session at spawn with the command line's render scale.
    ///
    /// Example: `let mut session = Session::New(&arguments);`
    #[allow(non_snake_case)]
    fn New(arguments: &Arguments) -> Result<Session> {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let player: Player = Player::AtSpawn(&world);
        let route: Vec<RouteFrame> = match (arguments.play_route, arguments.route) {
            (false, _) => Vec::new(),
            (true, RouteKind::Legs) => simulate_route(&world)?.frames,
            (true, RouteKind::MouthTour) => make_mouth_tour(&world),
        };
        Ok(Session {
            world,
            player,
            render_scale: arguments
                .render_scale
                .unwrap_or(get_default_render_scale(arguments.renderer)),
            last_used_scale: 1.0,
            is_auto_scale: arguments.renderer == RendererKind::Metal
                && arguments.render_scale.is_none(),
            exit_after_s: arguments.exit_after_s,
            route,
            route_index: 0,
            frame_times_ms: Vec::new(),
            render_scales: Vec::new(),
            total_frame_count: 0,
            total_gpu_ms: 0.0,
            last_output_px: (0, 0),
            is_laser_on: false,
            is_auto_forward: false,
            is_captured: false,
            message: String::new(),
            message_time_s: 0.0,
            laser: None,
            fps: 0.0,
            render_ms: 0.0,
            gpu_ms: None,
            start: Instant::now(),
            last: Instant::now(),
            hud_time_s: 0.0,
            hud_frame_count: 0,
            hud_render_ms: 0.0,
        })
    }

    /// `handle_event` reacts to one window event and returns false when the player quits: Esc
    /// releases the mouse (or quits if it is free), a click captures it, the mouse looks, and H,
    /// L, F, R, G, - and = toggle flying, the laser, moving forward, respawn, aim and render scale.
    ///
    /// Example: `if !session.handle_event(event, &window, &mouse) { break; }`
    fn handle_event(
        &mut self,
        event: Event,
        window: &sdl3::video::Window,
        mouse: &sdl3::mouse::MouseUtil,
    ) -> bool {
        match event {
            Event::Quit { .. } => return false,
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => {
                if !self.is_captured {
                    return false;
                }
                self.is_captured = false;
                mouse.set_relative_mouse_mode(window, false);
            }
            Event::KeyDown {
                keycode: Some(key),
                repeat: false,
                ..
            } => match key {
                Keycode::H => {
                    self.player.is_flying = !self.player.is_flying;
                    self.player.velocity = glam::DVec4::ZERO;
                }
                Keycode::L => self.is_laser_on = !self.is_laser_on,
                Keycode::F => self.is_auto_forward = !self.is_auto_forward,
                Keycode::R => self.player = Player::AtSpawn(&self.world),
                Keycode::G => {
                    if let Some(k) = physics::find_aimed_mouth(&self.world, self.player.frame) {
                        physics::aim_through_mouth(
                            &self.world,
                            &mut self.player.frame,
                            &self.world.mouths[k],
                        );
                    }
                }
                Keycode::Minus => {
                    self.is_auto_scale = false;
                    self.render_scale =
                        (self.render_scale / RENDER_SCALE_STEP).max(MIN_RENDER_SCALE);
                }
                Keycode::Equals => {
                    self.is_auto_scale = false;
                    self.render_scale = (self.render_scale * RENDER_SCALE_STEP).min(1.0);
                }
                _ => {}
            },
            Event::MouseButtonDown { .. } if !self.is_captured => {
                self.is_captured = true;
                mouse.set_relative_mouse_mode(window, true);
            }
            Event::MouseMotion { xrel, yrel, .. } if self.is_captured => {
                physics::look_around(&mut self.player.frame, f64::from(xrel), f64::from(yrel));
            }
            _ => {}
        }
        true
    }

    /// `update` moves the player by the held keys for the time since the last frame.
    ///
    /// Example: `session.update(&event_pump.keyboard_state());`
    fn update(&mut self, keys: &sdl3::keyboard::KeyboardState) {
        let frame_s: f64 = self.last.elapsed().as_secs_f64();
        let dt_s: f64 = frame_s.min(MAX_FRAME_DT_S);
        self.last = Instant::now();
        if self.route_index < self.route.len() {
            self.frame_times_ms.push(frame_s * MS_PER_S);
            let step: &RouteFrame = &self.route[self.route_index];
            (self.player.frame, self.is_laser_on) = (step.frame, step.is_laser_on);
            self.route_index += 1;
            self.hud_time_s += dt_s;
            return;
        }
        let input: Input = Input {
            forward: keys.is_scancode_pressed(Scancode::W) || self.is_auto_forward,
            back: keys.is_scancode_pressed(Scancode::S),
            left: keys.is_scancode_pressed(Scancode::A),
            right: keys.is_scancode_pressed(Scancode::D),
            up: keys.is_scancode_pressed(Scancode::Space) || keys.is_scancode_pressed(Scancode::E),
            down: keys.is_scancode_pressed(Scancode::Q),
            sprint: keys.is_scancode_pressed(Scancode::LShift)
                || keys.is_scancode_pressed(Scancode::RShift),
        };
        for message in physics::update_player(&self.world, &mut self.player, &input, dt_s) {
            self.message = message;
            self.message_time_s = MESSAGE_DURATION_S;
        }
        physics::level_out(&mut self.player.frame, dt_s);
        self.message_time_s -= dt_s;
        self.hud_time_s += dt_s;
    }

    /// `get_render_size` gives the size to ray-trace for a window of `window_px` pixels.
    ///
    /// Example: `let render_px = session.get_render_size((2560, 1600));`
    fn get_render_size(&mut self, window_px: (u32, u32)) -> (u32, u32) {
        self.last_output_px = window_px;
        let quantized_scale: f64 = if self.is_auto_scale {
            ((self.render_scale / RENDER_SCALE_QUANTUM + RENDER_SCALE_ROUNDING).floor()
                * RENDER_SCALE_QUANTUM)
                .max(MIN_AUTO_RENDER_SCALE)
        } else {
            self.render_scale
        };
        self.last_used_scale = quantized_scale;
        if !self.route.is_empty() {
            self.render_scales.push(quantized_scale);
        }
        scale_size(window_px, quantized_scale)
    }

    /// `pack` packs this frame's scene (and laser) at a render size.
    ///
    /// Example: `let data = session.pack((1280, 800));`
    fn pack(&mut self, render_px: (u32, u32)) -> SceneData {
        let (data, report) = pack_view(
            &self.world,
            self.player.frame,
            self.is_laser_on,
            render_px,
            self.start.elapsed().as_secs_f64(),
        );
        self.laser = report.as_ref().map(describe_laser);
        data
    }

    /// `record_frame` adds one frame's render time to the HUD's averages, which refresh every
    /// `HUD_PERIOD_S`. With automatic resolution, each frame also sets the render scale to the one
    /// whose GPU time would be `AUTO_SCALE_TARGET_GPU_MS` (time grows with the pixel count, the
    /// scale's square), by down to `AUTO_SCALE_MAX_DROP` a frame when that is lower and by at most
    /// `AUTO_SCALE_MAX_STEP` a frame when it is higher, between `MIN_AUTO_RENDER_SCALE` and 1; the
    /// render size follows it,
    /// rounded down to steps of `RENDER_SCALE_QUANTUM`.
    ///
    /// Example: `session.record_frame(12.5, renderer.gpu_ms());`
    fn record_frame(&mut self, render_ms: f64, gpu_ms: Option<f64>) {
        self.hud_frame_count += 1;
        self.hud_render_ms += render_ms;
        self.gpu_ms = gpu_ms;
        self.total_frame_count += 1;
        self.total_gpu_ms += gpu_ms.unwrap_or(render_ms);
        if let Some(ms) = gpu_ms
            && self.is_auto_scale
        {
            let wanted: f64 = self.last_used_scale * (AUTO_SCALE_TARGET_GPU_MS / ms).sqrt();
            let raised: f64 = (self.render_scale * AUTO_SCALE_MAX_STEP).min(wanted);
            let lowered: f64 = wanted.max(self.last_used_scale * AUTO_SCALE_MAX_DROP);
            self.render_scale = if wanted < self.render_scale {
                lowered
            } else {
                raised
            }
            .clamp(MIN_AUTO_RENDER_SCALE, 1.0);
        }
        if self.hud_time_s <= HUD_PERIOD_S {
            return;
        }
        self.fps = f64::from(self.hud_frame_count) / self.hud_time_s;
        self.render_ms = self.hud_render_ms / f64::from(self.hud_frame_count);
        (self.hud_time_s, self.hud_frame_count, self.hud_render_ms) = (0.0, 0, 0.0);
    }

    /// `print_route_statistics` prints how the window kept up while playing the route, after its
    /// first `ROUTE_WARMUP_FRAME_COUNT` frames: the time between frames (median, 99th percentile,
    /// worst, and how many took longer than a 60 Hz frame) and the render scale it used.
    ///
    /// Example: `session.print_route_statistics();`
    fn print_route_statistics(&self) {
        let mut times_ms: Vec<f64> = self.frame_times_ms[ROUTE_WARMUP_FRAME_COUNT..].to_vec();
        times_ms.sort_by(f64::total_cmp);
        let scales: &[f64] = &self.render_scales[ROUTE_WARMUP_FRAME_COUNT..];
        let percentile =
            |fraction: f64| times_ms[((times_ms.len() - 1) as f64 * fraction).round() as usize];
        let over_count: usize = times_ms.iter().filter(|&&ms| ms > FRAME_BUDGET_MS).count();
        println!(
            "{} route frames in a {}x{} window: {:.1} fps on average; frame times median {:.1} ms, 99th percentile {:.1} ms, worst {:.1} ms; {} frames ({:.1}%) over {:.1} ms; render scale mean {:.2}, lowest {:.2}, full in {:.1}% of frames",
            times_ms.len(),
            self.last_output_px.0,
            self.last_output_px.1,
            MS_PER_S * times_ms.len() as f64 / times_ms.iter().sum::<f64>(),
            percentile(0.5),
            percentile(0.99),
            times_ms[times_ms.len() - 1],
            over_count,
            100.0 * over_count as f64 / times_ms.len() as f64,
            FRAME_BUDGET_MS,
            scales.iter().sum::<f64>() / scales.len() as f64,
            scales.iter().copied().fold(1.0, f64::min),
            100.0 * scales.iter().filter(|&&scale| scale >= 1.0).count() as f64
                / scales.len() as f64,
        );
    }

    /// `is_finished` says whether `--exit-after-s` has run out; if so it prints the window's
    /// average frame rate, GPU time and last render scale.
    ///
    /// Example: `if session.is_finished() { return Ok(()); }`
    fn is_finished(&self) -> bool {
        if !self.route.is_empty() {
            if self.route_index < self.route.len() {
                return false;
            }
            self.print_route_statistics();
            return true;
        }
        let Some(limit_s) = self.exit_after_s else {
            return false;
        };
        let elapsed_s: f64 = self.start.elapsed().as_secs_f64();
        if elapsed_s < limit_s {
            return false;
        }
        let frame_count: f64 = self.total_frame_count.max(1) as f64;
        println!(
            "{} frames in {elapsed_s:.1} s: {:.1} fps, {:.1} ms per frame on the GPU, render scale {:.2} of a {}x{} window",
            self.total_frame_count,
            frame_count / elapsed_s,
            self.total_gpu_ms / frame_count,
            self.render_scale,
            self.last_output_px.0,
            self.last_output_px.1
        );
        true
    }

    /// `get_hud_lines` gives the HUD's lines: speed, movement, laser, the last message and the
    /// controls.
    ///
    /// Example: `let lines = session.get_hud_lines("metal", (1280, 800), (2560, 1600));`
    fn get_hud_lines(
        &self,
        renderer_name: &str,
        render_px: (u32, u32),
        output_px: (u32, u32),
    ) -> Vec<String> {
        let scaling: String = if render_px == output_px {
            String::new()
        } else {
            format!(" upscaled to {}x{}", output_px.0, output_px.1)
        };
        let movement: &str = if self.player.is_flying {
            "flying"
        } else if self.player.is_on_ground {
            "walking"
        } else {
            "in the air"
        };
        let mut lines: Vec<String> = vec![
            format!(
                "{:.0} fps, {:.1} ms render{}, {renderer_name} at {}x{}{scaling}",
                self.fps,
                self.render_ms,
                self.gpu_ms
                    .map_or(String::new(), |ms| format!(" ({ms:.1} ms on the GPU)")),
                render_px.0,
                render_px.1
            ),
            format!(
                "{movement}, height {:.1} blocks",
                self.player.get_height_m(&self.world)
            ),
        ];
        if let Some(laser) = &self.laser {
            lines.push(format!("laser {laser}"));
        }
        if self.message_time_s > 0.0 {
            lines.push(self.message.clone());
        }
        lines.push(if self.is_captured {
            "WASD move, Space jump/jetpack, H fly, G aim, L laser, R respawn, -/= scale, Esc release".to_string()
        } else {
            "Click to capture the mouse; Esc quits".to_string()
        });
        lines
    }
}

/// `scale_size` scales a size by a factor, keeping each side at least `MIN_IMAGE_SIDE_PX`.
///
/// Example: `let render_px = scale_size((2560, 1600), 0.5);`
fn scale_size(size_px: (u32, u32), scale: f64) -> (u32, u32) {
    (
        ((f64::from(size_px.0) * scale).round() as u32).max(MIN_IMAGE_SIDE_PX),
        ((f64::from(size_px.1) * scale).round() as u32).max(MIN_IMAGE_SIDE_PX),
    )
}

/// `play` opens the window and runs the demo until it is closed, presenting with Metal straight
/// to the window's layer or, for the CPU, through SDL's renderer.
///
/// Example: `play(&arguments, renderer)?;`
fn play(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let sdl: sdl3::Sdl = sdl3::init()?;
    let video: sdl3::VideoSubsystem = sdl.video()?;
    let mut session: Session = Session::New(arguments)?;
    let window_size: (u32, u32) = (arguments.width_px, arguments.height_px);
    match renderer.as_metal() {
        Some(metal) => play_on_layer(&sdl, &video, window_size, &mut session, metal),
        None => play_on_canvas(&sdl, &video, window_size, &mut session, renderer.as_mut()),
    }
}

/// `play_on_canvas` runs the window through SDL's renderer: each frame is drawn into pixels, put
/// in a texture scaled to the window, and the HUD is drawn over it with SDL's built-in text.
///
/// Example: `play_on_canvas(&sdl, &video, (1280, 800), &mut session, renderer.as_mut())?;`
fn play_on_canvas(
    sdl: &sdl3::Sdl,
    video: &sdl3::VideoSubsystem,
    window_size: (u32, u32),
    session: &mut Session,
    renderer: &mut dyn Renderer,
) -> Result<()> {
    let window: sdl3::video::Window = video
        .window(WINDOW_TITLE, window_size.0, window_size.1)
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
    let mut pixels: Vec<u8> = Vec::new();
    loop {
        for event in event_pump.poll_iter() {
            if !session.handle_event(event, canvas.window(), &mouse) {
                return Ok(());
            }
        }
        if session.is_finished() {
            return Ok(());
        }
        session.update(&event_pump.keyboard_state());
        let render_px: (u32, u32) = session.get_render_size(canvas.window().size_in_pixels());
        pixels.resize(
            render_px.0 as usize * render_px.1 as usize * BYTES_PER_PIXEL,
            0,
        );
        let render_start: Instant = Instant::now();
        let data: SceneData = session.pack(render_px);
        renderer.render(&data, render_px, &mut pixels)?;
        session.record_frame(
            render_start.elapsed().as_secs_f64() * MS_PER_S,
            renderer.gpu_ms(),
        );
        if texture.as_ref().is_none_or(|(_, size)| *size != render_px) {
            texture = Some((
                texture_creator.create_texture_streaming(
                    PixelFormat::RGBA32,
                    render_px.0,
                    render_px.1,
                )?,
                render_px,
            ));
        }
        let (current_texture, _) = texture.as_mut().expect("the texture was just made");
        current_texture.update(None, &pixels, render_px.0 as usize * BYTES_PER_PIXEL)?;
        canvas.set_scale(1.0, 1.0)?;
        canvas.copy(current_texture, None, None)?;
        let density: f32 = canvas.window().pixel_density();
        canvas.set_scale(density * HUD_TEXT_SCALE, density * HUD_TEXT_SCALE)?;
        canvas.set_draw_color(HUD_COLOR);
        let window_px: (u32, u32) = canvas.window().size_in_pixels();
        for (i, line) in session
            .get_hud_lines(renderer.name(), render_px, window_px)
            .iter()
            .enumerate()
        {
            canvas.draw_debug_text(
                line.as_str(),
                (HUD_MARGIN_PX, HUD_MARGIN_PX + HUD_LINE_PX * i as f32),
            )?;
        }
        canvas.present();
    }
}

/// `play_on_layer` runs the window with Metal presenting straight to the window's `CAMetalLayer`:
/// no pixels go through the CPU, MetalFX upscales the ray-traced image to the window's size, and
/// the HUD is shown in the window's title, since SDL's renderer can't draw on the same layer.
///
/// Example: `play_on_layer(&sdl, &video, (1280, 800), &mut session, metal)?;`
fn play_on_layer(
    sdl: &sdl3::Sdl,
    video: &sdl3::VideoSubsystem,
    window_size: (u32, u32),
    session: &mut Session,
    metal: &mut render_metal::MetalRenderer,
) -> Result<()> {
    let mut builder: sdl3::video::WindowBuilder =
        video.window(WINDOW_TITLE, window_size.0, window_size.1);
    builder.high_pixel_density().resizable().position_centered();
    let flags: u32 = builder.window_flags() | sdl3::sys::video::SDL_WINDOW_METAL.0 as u32;
    let mut window: sdl3::video::Window = builder.set_window_flags(flags).build()?;
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
    let view: sdl3::sys::metal::SDL_MetalView =
        unsafe { sdl3::sys::metal::SDL_Metal_CreateView(window.raw()) };
    if view.is_null() {
        return Err(format!("SDL: couldn't make a Metal view: {}", sdl3::get_error()).into());
    }
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A LAYER POINTER THAT ISN'T A CAMetalLayer WILL HAPPEN
    let layer: &CAMetalLayer =
        unsafe { &*sdl3::sys::metal::SDL_Metal_GetLayer(view).cast::<CAMetalLayer>() };
    layer.setDevice(Some(metal.get_device()));
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setFramebufferOnly(false);
    let mouse: sdl3::mouse::MouseUtil = sdl.mouse();
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let result: Result<()> = (|| loop {
        for event in event_pump.poll_iter() {
            if !session.handle_event(event, &window, &mouse) {
                return Ok(());
            }
        }
        if session.is_finished() {
            return Ok(());
        }
        session.update(&event_pump.keyboard_state());
        let window_px: (u32, u32) = window.size_in_pixels();
        layer.setDrawableSize(CGSize {
            width: f64::from(window_px.0),
            height: f64::from(window_px.1),
        });
        let render_px: (u32, u32) = session.get_render_size(window_px);
        let render_start: Instant = Instant::now();
        let data: SceneData = session.pack(render_px);
        if metal.render_to_layer(&data, layer)? {
            session.record_frame(
                render_start.elapsed().as_secs_f64() * MS_PER_S,
                Some(metal.last_gpu_ms),
            );
        }
        if session.hud_frame_count == 0 {
            window.set_title(&format!(
                "{WINDOW_TITLE}: {}",
                session
                    .get_hud_lines("metal", render_px, window_px)
                    .join("  |  ")
            ))?;
        }
    })();
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
    unsafe { sdl3::sys::metal::SDL_Metal_DestroyView(view) };
    result
}
