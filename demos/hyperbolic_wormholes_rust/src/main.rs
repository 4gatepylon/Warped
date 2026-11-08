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
//! - `--host`, `--join auto|<address>`, `--serve`, `--bot`, `--port`: play with others on the local
//!   network (see `net.rs` and `multiplayer.rs`).
//! - `--export-map <folder>`: write the world as a portable map (`common/maps/`).
//!
//! Example: `cargo run --release -- --bench-views --width-px 2560 --height-px 1600`
//!
//! Must know:
//! - Gotchas: run folders are found from the crate folder, not the current directory.

mod geometry;
mod map;
mod map_export;
mod multiplayer;
mod net;
mod physics;
mod render_cpu;
mod render_metal;
mod render_voxel;
mod scene_data;
mod settings;
mod voxel_clearance;
mod voxel_world;
mod world;

use std::error::Error;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use glam::{DMat4, DVec3, DVec4, EulerRot, Mat3, Vec3};
use objc2_core_foundation::CGSize;
use objc2_metal::MTLPixelFormat;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::{Keycode, Scancode};
use sdl3::mouse::MouseButton;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Texture;
use serde_json::{Value, json};

use geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, make_boost, make_rotation};
use physics::{Input, LaserReport, Player};
use render_metal::FrameData;
use render_voxel::VoxelFrame;
use scene_data::{Beam, SceneData};
use settings::GameSettings;
use voxel_world::{VoxelPlayer, VoxelWorld};
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
const FULL_RESOLUTION_FROM_SCALE: f64 = 0.95;
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
const WARP_TOUR_FRAMES_PER_WARP: usize = 600;
const WARP_TOUR_NEAR_PER_REACH: f64 = 0.1;
const WARP_TOUR_FAR_PER_REACH: f64 = 0.8;
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
const HUD_TEXT_SCALE: f32 = 2.0;
const HUD_LINE_PX: f32 = 10.0;
const HUD_MARGIN_PX: f32 = 4.0;
const WINDOW_TITLE: &str = "Hyperbolic wormholes (Rust)";
const MS_PER_S: f64 = 1000.0;
const HUD_COLOR: Color = Color::RGB(235, 240, 250);
const MESSAGE_DURATION_S: f64 = 3.0;
const WAND_RECOIL_M: f32 = 0.035;
const WAND_RECOIL_TILT_RAD: f32 = 0.12;
const HEAT_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.9];
const HEAT_BAR_FILL_COLOR: [f32; 4] = [1.0, 0.6, 0.2, 0.9];
const HEAT_BAR_OVERHEATED_COLOR: [f32; 4] = [1.0, 0.15, 0.1, 0.9];
const WAND_LASER_SHAFT_HALF_M: [f32; 4] = [0.011, 0.011, 0.16, 0.0];
const WAND_LASER_SHAFT_COLOR: [f32; 4] = [0.2, 0.17, 0.22, 1.0];
const WAND_LASER_TIP_HALF_M: [f32; 4] = [0.016, 0.016, 0.028, 0.0];
const WAND_RIFLE_SHAFT_HALF_M: [f32; 4] = [0.024, 0.032, 0.17, 0.0];
const WAND_RIFLE_SHAFT_COLOR: [f32; 4] = [0.42, 0.45, 0.5, 1.0];
const WAND_RIFLE_TIP_HALF_M: [f32; 4] = [0.007, 0.007, 0.07, 0.0];
const DEATH_MASK_PX: (u32, u32) = (240, 24);
const DEATH_TITLE_Y_PX: f32 = 2.0;
const DEATH_COUNTDOWN_Y_PX: f32 = 14.0;
const DEBUG_GLYPH_PX: f32 = 8.0;
const DEATH_DIM_FRAC: f32 = 0.55;
const DEATH_TITLE_MASK: u8 = 255;
const DEATH_COUNTDOWN_MASK: u8 = 128;
const ENERGY_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.93];
const ENERGY_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.018];
const ENERGY_BAR_BACK_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.55];
const ENERGY_BAR_FILL_COLOR: [f32; 4] = [0.5, 0.82, 1.0, 0.9];
const DISABLE_BAR_MIN_FRAC: [f32; 2] = [0.4, 0.62];
const DISABLE_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.02];
const DISABLE_BAR_BACK_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.5];
const DISABLE_BAR_FILL_COLOR: [f32; 4] = [1.0, 0.85, 0.3, 0.9];
const PERCENT_PER_FRAC: f64 = 100.0;
const GUN_WALK_HZ: f64 = 2.0;
const GUN_JOG_HZ: f64 = 2.8;
const GUN_POSE_PER_S: f64 = 6.0;
const WAND_JOG_CENTER_M: [f32; 3] = [0.22, -0.22, -0.34];
const WAND_AIM_YAW_RAD: f32 = 0.06;
const WAND_JOG_YAW_RAD: f32 = 0.35;
const WAND_AIM_PITCH_RAD: f32 = 0.06;
const WAND_JOG_PITCH_RAD: f32 = -0.35;
const WAND_JOG_ROLL_RAD: f32 = 0.5;
const WAND_JOG_SWING_RAD: f32 = 0.18;
const WAND_WALK_BOB_M: f32 = 0.006;
const WAND_JOG_BOB_M: f32 = 0.03;
const WAND_BLUE_TIP_COLOR: [f32; 4] = [0.35, 0.68, 1.0, 1.0];
const WAND_RED_TIP_COLOR: [f32; 4] = [1.0, 0.28, 0.14, 1.0];
const MIN_IMAGE_SIDE_PX: u32 = 16;
const MAX_IMAGE_SIDE_PX: u32 = 8192;
const THROAT_VIEW_DISTANCE_M: f64 = 0.8;
const NEAR_VIEW_DISTANCE_M: f64 = 2.5;
const SIDE_VIEW_DISTANCE_M: f64 = 0.4;
const SIDE_VIEW_YAW_RAD: f64 = std::f64::consts::FRAC_PI_2;
const THROAT_APPROACH_STEP_M: f64 = 0.1;
const MAX_APPROACH_STEP_COUNT: usize = 2000;
const SECONDS_PER_DAY: u64 = 86_400;
const DISCOVERY_TIMEOUT_S: u64 = 5;
const JOIN_AUTO: &str = "auto";
const SERVER_START_S: f64 = 0.3;
const ROUTE_PROBE_ADDRESS: &str = "192.0.2.1:9";

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

/// `MapName` is which map to play, from `common/maps/`.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum MapName {
    HouseDemo,
    LunarSurface,
}

impl MapName {
    /// `get_folder_name` gives the map's folder name in `common/maps/`.
    ///
    /// Example: `let name = MapName::HouseDemo.get_folder_name();`
    fn get_folder_name(self) -> &'static str {
        match self {
            MapName::HouseDemo => "house_demo",
            MapName::LunarSurface => "lunar_surface",
        }
    }
}

/// `RouteKind` is which route `--play-route` plays: the house's scripted legs, a tour around each
/// of its mouths from 5 blocks to just outside its throat and back, or (on a voxel map) a tour
/// around each warp object.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RouteKind {
    Legs,
    MouthTour,
    WarpTour,
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
    /// blocks to just outside its throat and back while the view swings around (both on the
    /// house), or on a voxel map a tour circling each warp object the same way (`warp-tour`).
    #[arg(long, value_enum, default_value_t = RouteKind::Legs)]
    route: RouteKind,
    /// Host a game on the local network: start a game server as its own process (as --serve,
    /// without a window), play in it, and stop it when this window closes.
    #[arg(long, conflicts_with_all = ["join", "serve", "bench_views", "autopilot", "play_route"])]
    host: bool,
    /// Join a game on the local network: `auto` to find one, or a server's IP address (with an
    /// optional :port).
    #[arg(long, conflicts_with_all = ["serve", "bench_views", "autopilot", "play_route"])]
    join: Option<String>,
    /// Run only a game server, without a window, until --exit-after-s (or until stopped); it
    /// prints this computer's address and the command that joins it.
    #[arg(long, conflicts_with_all = ["bench_views", "autopilot", "play_route"])]
    serve: bool,
    /// With --join and --exit-after-s: play by itself without a window (walk, aim at the nearest
    /// player and fire), to test a game; saves its view under .artifacts/bot/ at each hit.
    #[arg(long)]
    bot: bool,
    /// UDP port the server listens on (--host, --serve), and --join's port when its address has none.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    port: u16,
    /// Game settings, a YAML file with game_settings.yaml's keys (the default): a server (--serve,
    /// --host) sends them to every player who joins; without a server, this game uses them.
    #[arg(long, conflicts_with = "join")]
    settings: Option<PathBuf>,
    /// Which map to play, from common/maps/.
    #[arg(long, value_enum, default_value_t = MapName::HouseDemo)]
    map: MapName,
    /// With --map lunar-surface: start the window at one of the level's viewpoints (as
    /// --bench-views names them), e.g. black_hole.
    #[arg(long)]
    view: Option<String>,
    /// Write the world as a map in the portable format of common/maps/ (map.json) into this folder,
    /// replacing the folder's map.json, and exit.
    #[arg(long, conflicts_with_all = ["bench_views", "autopilot", "play_route", "host", "join", "serve", "bot"])]
    export_map: Option<PathBuf>,
}

/// `Renderer` draws one frame's packed scene into an RGBA pixel buffer.
trait Renderer {
    /// `render` draws `data` (ray-traced at its own render size) into `pixels` at `output_px`
    /// (top row first, 4 bytes per pixel), upscaling it if `output_px` is larger.
    ///
    /// Example: `renderer.render(FrameData::House(&data), (2560, 1600), &mut pixels)?;`
    fn render(&mut self, data: FrameData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()>;

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
    fn render(&mut self, frame: FrameData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()> {
        let FrameData::House(data) = frame else {
            return Err(
                "the CPU renderer draws the house demo only; use --renderer metal for voxel maps"
                    .into(),
            );
        };
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
    fn render(&mut self, data: FrameData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()> {
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
    if arguments.view.is_some() && arguments.map != MapName::LunarSurface {
        return Err(
            "--view needs --map lunar-surface; e.g. --map lunar-surface --view black_hole".into(),
        );
    }
    if arguments.map == MapName::LunarSurface
        && (arguments.autopilot || (arguments.play_route && arguments.route != RouteKind::WarpTour))
    {
        return Err("--autopilot and the house's routes need --map house-demo; on lunar-surface use --play-route --route warp-tour".into());
    }
    if arguments.map == MapName::HouseDemo
        && arguments.play_route
        && arguments.route == RouteKind::WarpTour
    {
        return Err("--route warp-tour is for voxel maps; use --map lunar-surface".into());
    }
    if arguments.bot && (arguments.join.is_none() || arguments.exit_after_s.is_none()) {
        return Err(
            "--bot needs --join and --exit-after-s; e.g. --join auto --bot --exit-after-s 30"
                .into(),
        );
    }
    if let Some(join) = &arguments.join
        && join != JOIN_AUTO
        && parse_server_address(join, arguments.port).is_none()
    {
        return Err(format!(
            "--join must be auto or an IP address with an optional :port, got {join:?}; e.g. --join 192.168.1.20"
        )
        .into());
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
    if let Some(folder) = &arguments.export_map {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let path: PathBuf = map_export::export_map(&world, folder)?;
        println!("Wrote {}", path.display());
        return Ok(());
    }
    if arguments.serve {
        return serve_only(&arguments);
    }
    let renderer: Box<dyn Renderer> = make_renderer(arguments.renderer, arguments.sampling)?;
    if arguments.bot {
        run_bot(&arguments, renderer)
    } else if arguments.bench_views {
        bench_views(&arguments, renderer)
    } else if arguments.autopilot {
        run_autopilot(&arguments, renderer)
    } else {
        play(&arguments, renderer)
    }
}

/// `load_world` loads a map from `common/maps/` as the world of boxes and mouths this renderer
/// draws.
///
/// Example: `let world = load_world(MapName::HouseDemo)?;`
fn load_world(name: MapName) -> Result<World> {
    let map: map::Map = map::Map::Load(&map::get_map_folder(name.get_folder_name()))?;
    Ok(World::FromMap(&map)?)
}

/// `parse_server_address` reads `--join`'s address: an IP address with an optional port (`port`
/// when it has none).
///
/// Example: `let address = parse_server_address("192.168.1.20", 47800);`
fn parse_server_address(text: &str, port: u16) -> Option<SocketAddr> {
    text.parse::<SocketAddr>().ok().or_else(|| {
        text.parse::<IpAddr>()
            .ok()
            .map(|ip| SocketAddr::new(ip, port))
    })
}

/// `get_server_address` gives the server to join: found on the network for `--join auto`, the
/// given address, or this computer for `--host`.
///
/// Example: `let address = get_server_address(&arguments)?;`
fn get_server_address(arguments: &Arguments) -> Result<SocketAddr> {
    match arguments.join.as_deref() {
        Some(JOIN_AUTO) => net::find_server(Duration::from_secs(DISCOVERY_TIMEOUT_S)),
        Some(text) => {
            Ok(parse_server_address(text, arguments.port)
                .expect("validate_arguments checked --join"))
        }
        None => Ok(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            arguments.port,
        )),
    }
}

/// `start_server` binds the server's port and runs the server for a map with `console_teams` on a
/// thread until the returned flag is set, starting a new game after a win at each line read from
/// standard input (`Enter` in the server's terminal).
///
/// Example: `let (should_stop, thread) = start_server(47800, "lunar_surface", GameSettings::Builtin(), vec![0, 1])?;`
fn start_server(
    port: u16,
    map_name: &str,
    settings: GameSettings,
    console_teams: Vec<u32>,
) -> Result<(Arc<AtomicBool>, JoinHandle<()>)> {
    let socket: std::net::UdpSocket = net::bind_server(port)?;
    let should_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let flag: Arc<AtomicBool> = Arc::clone(&should_stop);
    let should_start_new_game: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let map_name: String = map_name.to_string();
    let new_game_flag: Arc<AtomicBool> = Arc::clone(&should_start_new_game);
    std::thread::spawn(move || {
        for _ in std::io::stdin().lines().map_while(std::io::Result::ok) {
            new_game_flag.store(true, Ordering::Relaxed);
        }
    });
    let thread: JoinHandle<()> = std::thread::spawn(move || {
        if let Err(error) = net::serve(
            socket,
            &flag,
            map_name,
            settings,
            console_teams,
            &should_start_new_game,
        ) {
            eprintln!("The game server stopped: {error}");
        }
    });
    Ok((should_stop, thread))
}

/// `start_server_process` starts this program as a game server (`--serve`, without a window) on
/// `arguments`' port, map and settings, and fails if it stops within `SERVER_START_S` (its port in
/// use, say).
///
/// Example: `let server = start_server_process(&arguments)?;`
fn start_server_process(arguments: &Arguments) -> Result<Child> {
    let map_name: String = get_map_flag(arguments.map);
    let mut command: Command = Command::new(std::env::current_exe()?);
    command.args([
        "--serve",
        "--port",
        &arguments.port.to_string(),
        "--map",
        &map_name,
    ]);
    if let Some(path) = &arguments.settings {
        command.arg("--settings").arg(path);
    }
    let mut server: Child = command.spawn()?;
    std::thread::sleep(Duration::from_secs_f64(SERVER_START_S));
    if let Some(status) = server.try_wait()? {
        return Err(
            format!("the game server stopped at once ({status}); see its error above").into(),
        );
    }
    Ok(server)
}

/// `load_settings` gives the game settings `--settings` names, or the built-in ones.
///
/// Example: `let settings = load_settings(&arguments)?;`
fn load_settings(arguments: &Arguments) -> Result<GameSettings> {
    match &arguments.settings {
        Some(path) => GameSettings::Load(path),
        None => Ok(GameSettings::Builtin()),
    }
}

/// `get_map_flag` gives a map's name as `--map` and `MAP=` take it, e.g. `lunar-surface`.
///
/// Example: `let name = get_map_flag(MapName::LunarSurface);`
fn get_map_flag(map: MapName) -> String {
    map.to_possible_value()
        .expect("every MapName has a --map value")
        .get_name()
        .to_string()
}

/// `find_lan_address` gives this computer's address on its network (the one its default route
/// uses), or `None` without a network. Connecting a UDP socket only picks the route; nothing is
/// sent.
///
/// Example: `let address = find_lan_address();`
fn find_lan_address() -> Option<IpAddr> {
    let socket: UdpSocket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(ROUTE_PROBE_ADDRESS).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// `serve_only` runs a game server without a window until `--exit-after-s` passes (or forever),
/// after printing this computer's address and the commands that join the game.
///
/// Example: `serve_only(&arguments)?;`
fn serve_only(arguments: &Arguments) -> Result<()> {
    let map_data: map::Map = map::Map::Load(&map::get_map_folder(arguments.map.get_folder_name()))?;
    let consoles: Vec<voxel_world::Console> = voxel_world::find_consoles(&map_data);
    let (should_stop, thread) = start_server(
        arguments.port,
        arguments.map.get_folder_name(),
        load_settings(arguments)?,
        consoles.iter().map(|c| c.team).collect(),
    )?;
    println!(
        "Game settings: {} (players who join get them).",
        arguments
            .settings
            .as_ref()
            .map_or("the built-in game_settings.yaml".to_string(), |path| path
                .display()
                .to_string())
    );
    let (map, port) = (get_map_flag(arguments.map), arguments.port);
    let address: String = match find_lan_address() {
        Some(address) => address.to_string(),
        None => {
            eprintln!("Couldn't find this computer's address; `ipconfig getifaddr en0` gives it");
            "<this computer's address>".to_string()
        }
    };
    println!("Game server for {map} on UDP port {port}, without a window. Stop it with Ctrl-C.");
    println!("This computer's address: {address}. Join from the Rust demo's folder:");
    println!("  make join-game MAP={map} SERVER={address} PORT={port}");
    println!(
        "  make join-game MAP={map} PORT={port}  (finds this server on the network by itself)"
    );
    println!("  make join-game MAP={map} SERVER=127.0.0.1 PORT={port}  (on this computer)");
    if !consoles.is_empty() {
        println!(
            "A team wins by turning off the other team's consoles; then press Enter here for a new game."
        );
    }
    match arguments.exit_after_s {
        Some(seconds) => std::thread::sleep(Duration::from_secs_f64(seconds)),
        None => loop {
            std::thread::park();
        },
    }
    should_stop.store(true, Ordering::Relaxed);
    thread
        .join()
        .map_err(|_| "the game server's thread panicked")?;
    Ok(())
}

/// `VoxelView` is a named viewpoint on a voxel map, from the Lunar Surface level's own list: where
/// the eye stands (Fermi coordinates), what it faces, its height when flying and its pitch.
struct VoxelView {
    name: &'static str,
    at_m: [f64; 2],
    target_m: [f64; 2],
    h_m: Option<f64>,
    pitch_rad: f64,
    is_laser_on: bool,
}

/// `make_voxel_bench_views` gives the voxel benchmark's viewpoints: the Lunar Surface level's
/// views, and the laser fired into the wormhole from its viewpoint.
///
/// Example: `let views = make_voxel_bench_views();`
fn make_voxel_bench_views() -> Vec<VoxelView> {
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
        view("red_spawn", [120.0, 0.0], [0.0, 0.0], None, 0.0),
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
        VoxelView {
            is_laser_on: true,
            ..view(
                "laser_into_wormhole",
                [-22.0, 40.0],
                [-14.0, 40.0],
                None,
                0.0,
            )
        },
    ]
}

/// `bench_voxel_views` renders a voxel map's viewpoints without a window, timing each, as
/// `bench_views` does for the house.
///
/// Example: `bench_voxel_views(&arguments, renderer, &world)?;`
fn bench_voxel_views(
    arguments: &Arguments,
    mut renderer: Box<dyn Renderer>,
    world: &VoxelWorld,
) -> Result<()> {
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
        let beams: Vec<(Vec<voxel_world::BeamSegment>, u32)> =
            shot.iter().map(|s| (s.segments.clone(), 0)).collect();
        let spot: Option<(glam::DVec4, u32)> = shot.as_ref().and_then(|s| s.spot).map(|x| (x, 0));
        let warm_up: VoxelFrame = VoxelFrame::Build(world, camera, &beams, spot, render_px);
        renderer.render(FrameData::Voxel(&warm_up), size_px, &mut pixels)?;
        let (mut times_ms, mut gpu_times_ms): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        for _ in 0..arguments.frame_count {
            let start: Instant = Instant::now();
            let frame: VoxelFrame = VoxelFrame::Build(world, camera, &beams, spot, render_px);
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

/// `run_bot` plays as a bot without a window for `--exit-after-s` seconds (see
/// `multiplayer::run_bot`), saving its view at each hit under `.artifacts/bot/<run_id>/`, and
/// prints how many players it hit and how often it was hit.
///
/// Example: `run_bot(&arguments, renderer)?;`
fn run_bot(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let map: map::Map = map::Map::Load(&map::get_map_folder(arguments.map.get_folder_name()))?;
    let mut client: net::Client = net::Client::Connect(
        get_server_address(arguments)?,
        arguments.map.get_folder_name(),
    )?;
    let folder: PathBuf = make_run_folder("bot", arguments.label.as_deref())?;
    let mut saved_count: usize = 0;
    let duration_s: f64 = arguments
        .exit_after_s
        .expect("validate_arguments checked --bot");
    let mut save = |frame: FrameData, victim_id: u32| -> Result<()> {
        let size: (u32, u32) = {
            let (w, h) = frame.get_render_size();
            (w as u32, h as u32)
        };
        let mut pixels: Vec<u8> = vec![0; size.0 as usize * size.1 as usize * BYTES_PER_PIXEL];
        renderer.render(frame, size, &mut pixels)?;
        saved_count += 1;
        save_png(
            &folder.join(format!("hit_{saved_count:03}_player_{victim_id}.png")),
            &pixels,
            size.0,
            size.1,
        )
    };
    let (hit_count, eliminated_count) = if map.structures.is_empty() {
        let world: World = World::FromMap(&map)?;
        multiplayer::run_bot(&world, &mut client, duration_s, |data, id| {
            save(FrameData::House(data), id)
        })?
    } else {
        let world: VoxelWorld = VoxelWorld::FromMap(&map)?;
        multiplayer::run_voxel_bot(&world, &mut client, duration_s, |frame, id| {
            save(FrameData::Voxel(frame), id)
        })?
    };
    if arguments.json {
        println!(
            "{}",
            json!({"hit_count": hit_count, "eliminated_count": eliminated_count, "run_folder": folder})
        );
    } else {
        println!("The bot hit {hit_count} players and was hit {eliminated_count} times.");
        println!("Run folder: {}", folder.display());
    }
    Ok(())
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
    renderer.render(FrameData::House(&data), output_px, pixels)?;
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
    let map: map::Map = map::Map::Load(&map::get_map_folder(arguments.map.get_folder_name()))?;
    if !map.structures.is_empty() {
        return bench_voxel_views(arguments, renderer, &VoxelWorld::FromMap(&map)?);
    }
    let world: World = World::FromMap(&map)?;
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
    report_bench(arguments, &folder, renderer.name(), render_px, results)
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

/// `make_warp_tour` makes a tour of a voxel map's warp objects: for each, the eye circles it once
/// at its centre's height, from `WARP_TOUR_FAR_PER_REACH` of its reach to `WARP_TOUR_NEAR_PER_REACH`
/// and back (19 m to 2.4 m around the black hole, 12 m to 1.5 m around a mouth), looking at it with
/// the view swinging as in `make_mouth_tour`, over `WARP_TOUR_FRAMES_PER_WARP` frames.
///
/// Example: `let tour = make_warp_tour(&world);`
fn make_warp_tour(world: &VoxelWorld) -> Vec<RouteFrame> {
    let mut frames: Vec<RouteFrame> = Vec::new();
    for (k, warp) in world.warps.iter().enumerate() {
        for n in 0..WARP_TOUR_FRAMES_PER_WARP {
            let turn: f64 =
                std::f64::consts::TAU * n as f64 / (WARP_TOUR_FRAMES_PER_WARP - 1) as f64;
            let distance: f64 = warp.reach
                * (WARP_TOUR_NEAR_PER_REACH
                    + (WARP_TOUR_FAR_PER_REACH - WARP_TOUR_NEAR_PER_REACH)
                        * (0.5 + 0.5 * turn.cos()));
            let frame: DMat4 = warp.frame
                * make_rotation(AXIS_X, AXIS_Z, turn)
                * make_boost(AXIS_Z, distance)
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
                height_m: world.r * frame.col(AXIS_W)[AXIS_Y].asinh(),
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
    let world: World = load_world(arguments.map)?;
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
    shot_time_s: f64,
    is_running: bool,
    is_jumping: bool,
    settings: GameSettings,
    gun: Gun,
    is_trigger_held: bool,
    rifle_next_shot_s: f64,
    rifle_heat_frac: f64,
    is_overheated: bool,
    has_rifle_shot_hit: bool,
    tracers: Vec<Tracer>,
    was_firing: bool,
    remote_firing_ids: Vec<u32>,
    rifle_shot_count: u64,
    recoil_frac: f64,
    gun_phase_rad: f64,
    run_pose_frac: f64,
    motion_blur_frac: f32,
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
    multiplayer: Option<Multiplayer>,
    voxel: Option<VoxelState>,
}

/// `VoxelState` is a window's game on a voxel map: the map, the player and their spawn point.
struct VoxelState {
    world: VoxelWorld,
    player: VoxelPlayer,
    spawn_index: usize,
}

/// `PackedFrame` is one frame packed for the renderer: the house's rows or a voxel map's frame.
enum PackedFrame {
    House(SceneData),
    Voxel(VoxelFrame),
}

impl PackedFrame {
    /// `as_frame_data` lends the frame to a renderer.
    ///
    /// Example: `renderer.render(packed.as_frame_data(), size, &mut pixels)?;`
    fn as_frame_data(&self) -> FrameData<'_> {
        match self {
            PackedFrame::House(data) => FrameData::House(data),
            PackedFrame::Voxel(frame) => FrameData::Voxel(frame),
        }
    }
}

/// `Tracer` is a fired laser drawn as a beam flying out along its path (`segments`, traced when it
/// was fired, `length_m` long): its front leaves the muzzle at `fired_s` (seconds into the session)
/// and moves at `speed_m_per_s`, each point stays lit for `trail_s` after the front passes it, and
/// `spot` glows where it hit while the trail is there.
struct Tracer {
    segments: Vec<voxel_world::BeamSegment>,
    length_m: f64,
    spot: Option<glam::DVec4>,
    team: u32,
    fired_s: f64,
    speed_m_per_s: f64,
    trail_s: f64,
}

impl Tracer {
    /// `New` makes a tracer for `shot`, fired at `fired_s` by a player of `team` with the rifle
    /// (`is_rifle`) or the sniper, whose beam speed and trail come from `settings`.
    ///
    /// Example: `let tracer = Tracer::New(&shot, 1, 12.5, false, &settings, world.r);`
    #[allow(non_snake_case)]
    fn New(
        shot: &voxel_world::Shot,
        team: u32,
        fired_s: f64,
        is_rifle: bool,
        settings: &GameSettings,
        r: f64,
    ) -> Tracer {
        let (speed_m_per_s, trail_s) = if is_rifle {
            (
                settings.rifle.beam_speed_m_per_s,
                settings.rifle.beam_trail_s,
            )
        } else {
            (
                settings.sniper.beam_speed_m_per_s,
                settings.sniper.beam_trail_s,
            )
        };
        Tracer {
            segments: shot.segments.clone(),
            length_m: shot.segments.iter().map(|s| s.length).sum::<f64>() * r,
            spot: shot.spot,
            team,
            fired_s,
            speed_m_per_s,
            trail_s,
        }
    }

    /// `get_lit_m` gives the lit stretch of the path at `now_s`, as meters from its start: from the
    /// trail's end to the front.
    ///
    /// Example: `let (tail_m, head_m) = tracer.get_lit_m(13.0);`
    fn get_lit_m(&self, now_s: f64) -> (f64, f64) {
        let head_m: f64 = self.speed_m_per_s * (now_s - self.fired_s);
        (head_m - self.speed_m_per_s * self.trail_s, head_m)
    }
}

/// `Gun` is the weapon a player holds: gun 1, the laser (a quarter-second shot per click; one
/// hit eliminates), or gun 2, the design's rifle (fires while the button is held, its beam
/// pulsing; two hits to the head or twelve to the body eliminate; overheats).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Gun {
    Laser,
    Rifle,
}

/// `Multiplayer` is a window's game on the local network: its connection, the server process it
/// started with `--host`, whether this player was hit and waits to rejoin, the number of the game
/// it plays, and the console this player holds `E` on, with the seconds held.
struct Multiplayer {
    client: net::Client,
    server: Option<Child>,
    is_eliminated: bool,
    respawn_in_s: f64,
    game_number: u32,
    disabling: Option<(u8, f64)>,
}

impl Drop for Multiplayer {
    /// `drop` tells the server this player is leaving and stops the hosted server, if any.
    ///
    /// Example: `drop(multiplayer);`
    fn drop(&mut self) {
        if let Err(error) = self.client.leave() {
            eprintln!("Couldn't tell the server this player left: {error}");
        }
        if let Some(mut server) = self.server.take()
            && let Err(error) = server.kill().and_then(|()| server.wait().map(drop))
        {
            eprintln!("Couldn't stop the game server: {error}");
        }
    }
}

impl Session {
    /// `New` starts a session at spawn with the command line's render scale.
    ///
    /// Example: `let mut session = Session::New(&arguments);`
    #[allow(non_snake_case)]
    fn New(arguments: &Arguments) -> Result<Session> {
        let map: map::Map = map::Map::Load(&map::get_map_folder(arguments.map.get_folder_name()))?;
        let settings: GameSettings = load_settings(arguments)?;
        let voxel: Option<VoxelState> = if map.structures.is_empty() {
            None
        } else {
            let mut world: VoxelWorld = VoxelWorld::FromMap(&map)?;
            world.set_settings(settings.clone());
            let player: VoxelPlayer = match &arguments.view {
                Some(name) => {
                    let view: VoxelView = make_voxel_bench_views()
                        .into_iter()
                        .find(|v| v.name == name)
                        .ok_or_else(|| {
                            format!("--view {name:?} isn't a viewpoint; see --bench-views")
                        })?;
                    VoxelPlayer::AtView(&world, view.at_m, view.target_m, view.h_m, view.pitch_rad)
                }
                None => VoxelPlayer::AtSpawn(&world, 0),
            };
            Some(VoxelState {
                world,
                player,
                spawn_index: 0,
            })
        };
        let world: World = match voxel {
            Some(_) => World::Empty(map.get_curvature_radius_m()),
            None => World::FromMap(&map)?,
        };
        let player: Player = Player::AtSpawn(&world);
        let route: Vec<RouteFrame> = match (arguments.play_route, arguments.route) {
            (false, _) => Vec::new(),
            (true, RouteKind::Legs) => simulate_route(&world)?.frames,
            (true, RouteKind::MouthTour) => make_mouth_tour(&world),
            (true, RouteKind::WarpTour) => make_warp_tour(
                &voxel
                    .as_ref()
                    .ok_or("--route warp-tour needs a voxel map")?
                    .world,
            ),
        };
        let multiplayer: Option<Multiplayer> = if arguments.host || arguments.join.is_some() {
            let server: Option<Child> = if arguments.host {
                Some(start_server_process(arguments)?)
            } else {
                None
            };
            Some(Multiplayer {
                client: net::Client::Connect(
                    get_server_address(arguments)?,
                    arguments.map.get_folder_name(),
                )?,
                server,
                is_eliminated: false,
                respawn_in_s: 0.0,
                game_number: 0,
                disabling: None,
            })
        } else {
            None
        };
        Ok(Session {
            world,
            player,
            settings,
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
            shot_time_s: 0.0,
            is_running: false,
            is_jumping: false,
            gun: Gun::Laser,
            is_trigger_held: false,
            rifle_next_shot_s: 0.0,
            rifle_heat_frac: 0.0,
            is_overheated: false,
            has_rifle_shot_hit: false,
            tracers: Vec::new(),
            was_firing: false,
            remote_firing_ids: Vec::new(),
            rifle_shot_count: 0,
            recoil_frac: 0.0,
            gun_phase_rad: 0.0,
            run_pose_frac: 0.0,
            motion_blur_frac: 0.0,
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
            multiplayer,
            voxel,
        })
    }

    /// `handle_event` reacts to one window event and returns false when the player quits: Esc
    /// releases the mouse (or quits if it is free), a click captures it (and once captured, a left
    /// click fires a laser shot for the settings' `sniper.shot_s`), the mouse looks, R respawns, and - and =
    /// change the render scale.
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
                Keycode::R => {
                    let others: Vec<DVec4> = self
                        .multiplayer
                        .as_ref()
                        .map_or(Vec::new(), |m| get_eyes(&m.client.players));
                    match &mut self.voxel {
                        Some(voxel) => {
                            let id: u32 = voxel.spawn_index as u32;
                            voxel.player = VoxelPlayer::NearSpawn(&voxel.world, id, &others);
                        }
                        None => {
                            let id: u32 = self
                                .multiplayer
                                .as_ref()
                                .and_then(|m| m.client.id)
                                .unwrap_or(0);
                            self.player = multiplayer::get_spawn_player(&self.world, id, &others);
                        }
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
                Keycode::_1 => self.switch_gun(Gun::Laser),
                Keycode::_2 => self.switch_gun(Gun::Rifle),
                _ => {}
            },
            Event::MouseButtonDown { .. } if !self.is_captured => {
                self.is_captured = true;
                mouse.set_relative_mouse_mode(window, true);
            }
            Event::MouseButtonDown {
                mouse_btn: MouseButton::Left,
                ..
            } => {
                self.is_trigger_held = true;
                if self.gun == Gun::Laser
                    && self.can_shoot()
                    && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated)
                {
                    self.shot_time_s = self.settings.sniper.shot_s;
                }
            }
            Event::MouseButtonUp {
                mouse_btn: MouseButton::Left,
                ..
            } => self.is_trigger_held = false,
            Event::MouseWheel { y, .. } if y != 0.0 => self.switch_gun(match self.gun {
                Gun::Laser => Gun::Rifle,
                Gun::Rifle => Gun::Laser,
            }),
            Event::MouseMotion { xrel, yrel, .. } if self.is_captured => match &mut self.voxel {
                Some(voxel) => voxel.player.look(f64::from(xrel), f64::from(yrel)),
                None => {
                    physics::look_around(&mut self.player.frame, f64::from(xrel), f64::from(yrel))
                }
            },
            _ => {}
        }
        true
    }

    /// `update` moves the player by the held keys for the time since the last frame, and fails if
    /// the game server can't be read (it plays another map, say).
    ///
    /// Example: `session.update(&event_pump.keyboard_state())?;`
    fn update(&mut self, keys: &sdl3::keyboard::KeyboardState) -> Result<()> {
        let frame_s: f64 = self.last.elapsed().as_secs_f64();
        let dt_s: f64 = frame_s.min(MAX_FRAME_DT_S);
        let blur_s: f64 = self.settings.display.motion_blur_s;
        self.motion_blur_frac = if blur_s > 0.0 {
            (-dt_s / blur_s).exp() as f32
        } else {
            0.0
        };
        self.last = Instant::now();
        if self.route_index < self.route.len() {
            self.frame_times_ms.push(frame_s * MS_PER_S);
            let step: &RouteFrame = &self.route[self.route_index];
            (self.player.frame, self.is_laser_on) = (step.frame, step.is_laser_on);
            self.route_index += 1;
            self.hud_time_s += dt_s;
            return Ok(());
        }
        self.shot_time_s = (self.shot_time_s - dt_s).max(0.0);
        if let Some(multiplayer) = &mut self.multiplayer
            && multiplayer.is_eliminated
        {
            multiplayer.respawn_in_s -= dt_s;
            if multiplayer.respawn_in_s <= 0.0 {
                multiplayer.is_eliminated = false;
                if let Err(error) = multiplayer.client.rejoin() {
                    self.message = format!("Couldn't rejoin: {error}");
                    self.message_time_s = MESSAGE_DURATION_S;
                }
            }
        }
        if self.update_multiplayer()? {
            self.message_time_s -= dt_s;
            self.hud_time_s += dt_s;
            return Ok(());
        }
        let input: Input = Input {
            forward: keys.is_scancode_pressed(Scancode::W),
            back: keys.is_scancode_pressed(Scancode::S),
            left: keys.is_scancode_pressed(Scancode::A),
            right: keys.is_scancode_pressed(Scancode::D),
            up: keys.is_scancode_pressed(Scancode::Space),
            down: false,
            sprint: keys.is_scancode_pressed(Scancode::LShift)
                || keys.is_scancode_pressed(Scancode::RShift),
        };
        let messages: Vec<String> = match &mut self.voxel {
            Some(voxel) => voxel
                .player
                .step(&voxel.world, &input, dt_s, voxel.spawn_index),
            None => {
                let messages: Vec<String> =
                    physics::update_player(&self.world, &mut self.player, &input, dt_s);
                physics::level_out(&mut self.player.frame, dt_s);
                messages
            }
        };
        for message in messages {
            self.message = message;
            self.message_time_s = MESSAGE_DURATION_S;
        }
        self.update_disabling(keys.is_scancode_pressed(Scancode::E), dt_s);
        self.update_gun(&input, dt_s);
        self.update_rifle(dt_s);
        let eye: DMat4 = self.get_eye_frame();
        let is_firing: bool = self.is_firing();
        if let Some(multiplayer) = &mut self.multiplayer
            && let Err(error) = multiplayer.client.send_state(
                eye,
                is_firing,
                self.gun == Gun::Rifle,
                multiplayer.disabling.map(|(console, _)| console),
            )
        {
            self.message = format!("Couldn't reach the server: {error}");
            self.message_time_s = MESSAGE_DURATION_S;
        }
        self.message_time_s -= dt_s;
        self.hud_time_s += dt_s;
        Ok(())
    }

    /// `is_firing` says whether this player's laser is on: on for a scripted route, or a shot fired
    /// by a click in the last `sniper.shot_s` (the settings), and only while this player can shoot.
    ///
    /// Example: `let is_firing = self.is_firing();`
    fn is_firing(&self) -> bool {
        (self.is_laser_on || self.shot_time_s > 0.0) && self.can_shoot()
    }

    /// `can_shoot` says whether this player may shoot: not while running, not while jumping or
    /// falling on a voxel map, not while holding `E` on a console, and not once a team has won.
    ///
    /// Example: `if self.can_shoot() { ... }`
    fn can_shoot(&self) -> bool {
        !self.is_running
            && !self.is_jumping
            && self
                .multiplayer
                .as_ref()
                .is_none_or(|m| m.disabling.is_none() && m.client.game.winner.is_none())
    }

    /// `switch_gun` makes `gun` the one in hand, stopping a shot in flight.
    ///
    /// Example: `self.switch_gun(Gun::Rifle);`
    fn switch_gun(&mut self, gun: Gun) {
        self.gun = gun;
        self.shot_time_s = 0.0;
    }

    /// `update_rifle` fires the rifle while its trigger is held and this player can shoot: a
    /// `RIFLE_PULSE_S` pulse of the beam every `RIFLE_SHOT_PERIOD_S`, each adding
    /// 1 / `RIFLE_OVERHEAT_SHOT_COUNT` of heat. At full heat it overheats; it cools fully in
    /// `cool_down_s` while not firing, and fires again once cool. Each shot recoils: the view kicks
    /// up by `recoil_up_rad` and sideways by `recoil_sideways_rad`, alternately left and right, and
    /// the rifle jolts back (`recoil_frac`, settling over `recoil_settle_s`); all from the settings.
    ///
    /// Example: `self.update_rifle(1.0 / 60.0);`
    fn update_rifle(&mut self, dt_s: f64) {
        let rifle: settings::RifleSettings = self.settings.rifle.clone();
        self.rifle_next_shot_s = (self.rifle_next_shot_s - dt_s).max(0.0);
        self.recoil_frac = if rifle.recoil_settle_s > 0.0 {
            (self.recoil_frac - dt_s / rifle.recoil_settle_s).max(0.0)
        } else {
            0.0
        };
        let is_firing_rifle: bool = self.gun == Gun::Rifle
            && self.is_trigger_held
            && self.can_shoot()
            && !self.is_overheated;
        if !is_firing_rifle {
            self.rifle_heat_frac = (self.rifle_heat_frac - dt_s / rifle.cool_down_s).max(0.0);
            self.is_overheated &= self.rifle_heat_frac > 0.0;
        } else if self.rifle_next_shot_s <= 0.0 {
            self.shot_time_s = rifle.pulse_s;
            self.rifle_next_shot_s = 1.0 / rifle.shots_per_s;
            self.has_rifle_shot_hit = false;
            self.rifle_heat_frac += 1.0 / f64::from(rifle.shots_until_overheat);
            self.is_overheated = self.rifle_heat_frac >= 1.0;
            self.rifle_shot_count += 1;
            self.recoil_frac = f64::from(u8::from(rifle.recoil_settle_s > 0.0));
            let side: f64 = if self.rifle_shot_count.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
            let (dx_px, dy_px) = (
                side * rifle.recoil_sideways_rad / voxel_world::LOOK_RAD_PER_PX,
                -rifle.recoil_up_rad / voxel_world::LOOK_RAD_PER_PX,
            );
            match &mut self.voxel {
                Some(voxel) => voxel.player.look(dx_px, dy_px),
                None => physics::look_around(&mut self.player.frame, dx_px, dy_px),
            }
        }
    }

    /// `update_disabling` follows this player holding `E` (`is_holding_e`) on one of the other
    /// team's consoles that is still on, within `CONSOLE_REACH_M` straight ahead: the seconds held
    /// on that same console, or nothing. The server times it too, and turns the console off.
    ///
    /// Example: `self.update_disabling(true, 1.0 / 60.0);`
    fn update_disabling(&mut self, is_holding_e: bool, dt_s: f64) {
        let (Some(voxel), Some(multiplayer)) = (&self.voxel, &mut self.multiplayer) else {
            return;
        };
        let game: net::GameState = multiplayer.client.game;
        let aimed: Option<u8> = match multiplayer.client.id {
            Some(id) if is_holding_e && game.winner.is_none() => voxel
                .world
                .find_aimed_material(
                    voxel.player.get_camera_frame(&voxel.world),
                    self.settings.consoles.reach_m,
                )
                .and_then(|m| voxel.world.consoles.iter().position(|c| c.material == m))
                .filter(|&i| {
                    voxel.world.consoles[i].team != net::get_team(id) && game.off_mask >> i & 1 == 0
                })
                .map(|i| i as u8),
            _ => None,
        };
        multiplayer.disabling = match (aimed, multiplayer.disabling) {
            (Some(console), Some((held, held_s))) if console == held => {
                Some((console, held_s + dt_s))
            }
            (Some(console), _) => Some((console, 0.0)),
            (None, _) => None,
        };
    }

    /// `get_console_masks` gives the consoles turned off and those someone holds `E` on (bit i for
    /// console i), none without a server.
    ///
    /// Example: `let (off_mask, disabling_mask) = self.get_console_masks();`
    fn get_console_masks(&self) -> (u32, u32) {
        let Some(multiplayer) = &self.multiplayer else {
            return (0, 0);
        };
        let disabling_mask: u32 = multiplayer
            .client
            .players
            .iter()
            .filter_map(|player| player.disabling_console)
            .chain(multiplayer.disabling.map(|(console, _)| console))
            .fold(0, |mask, console| mask | 1 << console);
        (multiplayer.client.game.off_mask, disabling_mask)
    }

    /// `update_gun` notes whether this player runs (sprints while moving) or jumps (is in the air
    /// on a voxel map, not flying), dropping a shot in flight if either, and moves the gun: its
    /// step phase while walking or running on the ground, and its pose toward jogging while
    /// running and back to aiming otherwise.
    ///
    /// Example: `self.update_gun(&input, dt_s);`
    fn update_gun(&mut self, input: &Input, dt_s: f64) {
        let is_moving: bool = input.forward || input.back || input.left || input.right;
        let (is_running, is_on_ground) = match &self.voxel {
            Some(voxel) => (
                voxel.player.is_sprinting,
                voxel.player.is_on_ground && !voxel.player.is_flying,
            ),
            None => (input.sprint && is_moving, self.player.is_on_ground),
        };
        self.is_running = is_running;
        self.is_jumping = self
            .voxel
            .as_ref()
            .is_some_and(|voxel| !voxel.player.is_on_ground && !voxel.player.is_flying);
        if !self.can_shoot() {
            self.shot_time_s = 0.0;
        }
        if is_moving && is_on_ground {
            let hz: f64 = if is_running { GUN_JOG_HZ } else { GUN_WALK_HZ };
            self.gun_phase_rad =
                (self.gun_phase_rad + std::f64::consts::TAU * hz * dt_s) % std::f64::consts::TAU;
        }
        let target: f64 = f64::from(u8::from(is_running));
        self.run_pose_frac +=
            (target - self.run_pose_frac).clamp(-GUN_POSE_PER_S * dt_s, GUN_POSE_PER_S * dt_s);
    }

    /// `get_wand` gives this player's wand (`make_wand`, with its tip in the team's colour), or
    /// `None` while a banner shows.
    ///
    /// Example: `let wand = self.get_wand();`
    fn get_wand(&self) -> Option<render_metal::OverlayWand> {
        let team: u32 = self
            .voxel
            .as_ref()
            .map_or(0, |voxel| net::get_team(voxel.spawn_index as u32));
        self.get_banner_lines().is_none().then(|| {
            make_wand(
                self.run_pose_frac,
                self.gun_phase_rad,
                self.recoil_frac,
                team,
                self.gun,
            )
        })
    }

    /// `get_eye_frame` gives the eye's frame: the house player's frame, or the voxel player's camera.
    ///
    /// Example: `let eye = session.get_eye_frame();`
    fn get_eye_frame(&self) -> DMat4 {
        match &self.voxel {
            Some(voxel) => voxel.player.get_camera_frame(&voxel.world),
            None => self.player.frame,
        }
    }

    /// `show_message` shows a line in the HUD for `MESSAGE_DURATION_S`.
    ///
    /// Example: `self.show_message("player 3 was hit by player 2".to_string());`
    fn show_message(&mut self, message: String) {
        self.message = message;
        self.message_time_s = MESSAGE_DURATION_S;
    }

    /// `update_multiplayer` reads the server's news: a welcome puts this player at its spawn
    /// point, and a hit is announced (and, if this player was hit, starts the respawn countdown).
    /// It returns whether this player is out of the game for now, and fails if the server can't be
    /// read.
    ///
    /// Example: `if session.update_multiplayer()? { return Ok(()); }`
    fn update_multiplayer(&mut self) -> Result<bool> {
        let Some(multiplayer) = &mut self.multiplayer else {
            return Ok(false);
        };
        let events: Vec<net::NetEvent> = multiplayer.client.poll()?;
        let mut messages: Vec<String> = Vec::new();
        for event in events {
            match event {
                net::NetEvent::Joined(id) => {
                    let others: Vec<DVec4> = get_eyes(&multiplayer.client.players);
                    self.settings = multiplayer
                        .client
                        .settings
                        .clone()
                        .expect("a welcome brings the server's settings");
                    match &mut self.voxel {
                        Some(voxel) => {
                            voxel.world.set_settings(self.settings.clone());
                            voxel.spawn_index = id as usize;
                            voxel.player = VoxelPlayer::NearSpawn(&voxel.world, id, &others);
                        }
                        None => {
                            self.player = multiplayer::get_spawn_player(&self.world, id, &others)
                        }
                    }
                    messages.push(format!("You joined as player {id}"));
                }
                net::NetEvent::Eliminated {
                    victim_id,
                    shooter_id,
                    is_self,
                } => {
                    if is_self {
                        multiplayer.is_eliminated = true;
                        multiplayer.respawn_in_s = self.settings.respawn.delay_s;
                        multiplayer.disabling = None;
                        self.is_laser_on = false;
                        self.shot_time_s = 0.0;
                    }
                    messages.push(if is_self {
                        format!("You died: hit by player {shooter_id}")
                    } else {
                        format!("Player {victim_id} was hit by player {shooter_id}")
                    });
                }
            }
        }
        if multiplayer.client.game.number != multiplayer.game_number {
            multiplayer.game_number = multiplayer.client.game.number;
            if let (Some(voxel), Some(id)) = (&mut self.voxel, multiplayer.client.id) {
                let others: Vec<DVec4> = get_eyes(&multiplayer.client.players);
                voxel.player = VoxelPlayer::NearSpawn(&voxel.world, id, &others);
            }
            messages.push(format!(
                "Game {} started: every console is back on",
                multiplayer.game_number
            ));
        }
        let is_out: bool = multiplayer.is_eliminated;
        if !is_out && let Err(error) = multiplayer.client.keep_joining() {
            messages.push(format!("Couldn't reach the server: {error}"));
        }
        if let Some(message) = messages.pop() {
            self.show_message(message);
        }
        Ok(is_out)
    }

    /// `get_render_size` gives the size to ray-trace for a window of `window_px` pixels. With
    /// automatic resolution, a scale of `FULL_RESOLUTION_FROM_SCALE` or more renders at full
    /// resolution, as MetalFX costs about as much as those scales save and loses as much detail as
    /// lower ones; others round down to steps of `RENDER_SCALE_QUANTUM`.
    ///
    /// Example: `let render_px = session.get_render_size((2560, 1600));`
    fn get_render_size(&mut self, window_px: (u32, u32)) -> (u32, u32) {
        self.last_output_px = window_px;
        let quantized_scale: f64 = if !self.is_auto_scale {
            self.render_scale
        } else if self.render_scale >= FULL_RESOLUTION_FROM_SCALE {
            1.0
        } else {
            ((self.render_scale / RENDER_SCALE_QUANTUM + RENDER_SCALE_ROUNDING).floor()
                * RENDER_SCALE_QUANTUM)
                .max(MIN_AUTO_RENDER_SCALE)
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
    fn pack(&mut self, render_px: (u32, u32)) -> PackedFrame {
        let is_firing: bool = self.is_firing();
        let (off_mask, disabling_mask) = self.get_console_masks();
        let time_s: f64 = self.start.elapsed().as_secs_f64();
        if let Some(voxel) = &mut self.voxel {
            voxel.world.light_consoles(off_mask, disabling_mask, time_s);
        }
        if let Some(voxel) = &self.voxel {
            let camera: DMat4 = if self.route.is_empty() {
                voxel.player.get_camera_frame(&voxel.world)
            } else {
                self.player.frame
            };
            let players: Vec<net::RemotePlayer> = self
                .multiplayer
                .as_ref()
                .map_or(Vec::new(), |m| m.client.players.clone());
            let team: u32 = voxel.spawn_index as u32 % 2;
            let (now_s, r) = (self.start.elapsed().as_secs_f64(), voxel.world.r);
            self.tracers
                .retain(|tracer| tracer.get_lit_m(now_s).0 < tracer.length_m);
            let beams: Vec<(Vec<voxel_world::BeamSegment>, u32)> = self
                .tracers
                .iter()
                .map(|tracer| {
                    let (tail_m, head_m) = tracer.get_lit_m(now_s);
                    (
                        voxel_world::clip_beam(&tracer.segments, r, tail_m, head_m),
                        tracer.team,
                    )
                })
                .collect();
            let spot: Option<(glam::DVec4, u32)> = self
                .tracers
                .iter()
                .rev()
                .find(|tracer| tracer.get_lit_m(now_s).1 >= tracer.length_m)
                .and_then(|tracer| tracer.spot.map(|x| (x, tracer.team)));
            let view: multiplayer::SharedVoxelView = multiplayer::pack_shared_voxel_view(
                &voxel.world,
                camera,
                is_firing,
                team,
                &players,
                self.route.is_empty().then_some(multiplayer::Tracers {
                    beams: &beams,
                    spot,
                }),
                render_px,
            );
            if is_firing
                && !self.was_firing
                && let Some(shot) = &view.shot
            {
                let is_rifle: bool = self.gun == Gun::Rifle;
                let tracer = Tracer::New(shot, team, now_s, is_rifle, &self.settings, r);
                self.tracers.push(tracer);
            }
            self.was_firing = is_firing;
            for (id, shot) in &view.other_shots {
                if !self.remote_firing_ids.contains(id) {
                    let is_rifle: bool = players.iter().any(|p| p.id == *id && p.is_rifle);
                    let tracer =
                        Tracer::New(shot, net::get_team(*id), now_s, is_rifle, &self.settings, r);
                    self.tracers.push(tracer);
                }
            }
            self.remote_firing_ids = view.other_shots.iter().map(|(id, _)| *id).collect();
            self.laser = multiplayer::describe_voxel_laser(&voxel.world, &view);
            if let Some(victim_id) = view.hit_player_id
                && let Some(hits_to_kill) = take_hit_weight(
                    &self.settings,
                    self.gun,
                    &mut self.has_rifle_shot_hit,
                    view.is_head_hit,
                )
                && let Some(multiplayer) = &mut self.multiplayer
                && let Err(error) = multiplayer.client.send_hit(victim_id, hits_to_kill)
            {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
            return PackedFrame::Voxel(view.frame);
        }
        if let Some(multiplayer) = &mut self.multiplayer {
            let view: multiplayer::SharedView = multiplayer::pack_shared_view(
                &self.world,
                self.player.frame,
                is_firing,
                &multiplayer.client.players,
                render_px,
                self.start.elapsed().as_secs_f64(),
            );
            self.laser = view.report.as_ref().map(describe_laser);
            if let Some(victim_id) = view.hit_player_id
                && let Some(hits_to_kill) = take_hit_weight(
                    &self.settings,
                    self.gun,
                    &mut self.has_rifle_shot_hit,
                    false,
                )
                && let Err(error) = multiplayer.client.send_hit(victim_id, hits_to_kill)
            {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
            return PackedFrame::House(view.data);
        }
        let (data, report) = pack_view(
            &self.world,
            self.player.frame,
            is_firing,
            render_px,
            self.start.elapsed().as_secs_f64(),
        );
        self.laser = report.as_ref().map(describe_laser);
        PackedFrame::House(data)
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

    /// `get_banner_lines` gives the two lines shown over the view once a team has won (the winner,
    /// and how to start a new game), or else while this player is out after a hit ("YOU DIED" and
    /// the countdown to respawning), or `None`.
    ///
    /// Example: `if let Some(lines) = session.get_banner_lines() { ... }`
    fn get_banner_lines(&self) -> Option<[String; 2]> {
        let multiplayer: &Multiplayer = self.multiplayer.as_ref()?;
        if let Some(winner) = multiplayer.client.game.winner {
            return Some([
                format!(
                    "{} TEAM WINS",
                    net::TEAM_NAMES[winner as usize].to_uppercase()
                ),
                "New game: Enter on the server".to_string(),
            ]);
        }
        if !multiplayer.is_eliminated {
            return None;
        }
        Some([
            "YOU DIED".to_string(),
            format!(
                "Respawning in {:.0}",
                multiplayer.respawn_in_s.ceil().max(1.0)
            ),
        ])
    }

    /// `get_overlay_rects` gives the HUD's rectangles drawn over the view: the progress of turning
    /// off a console (while holding `E` on one) below the middle
    /// and, on a voxel map, the sprint energy bar (its frame and its fill) in the bottom left corner.
    ///
    /// Example: `let rects = session.get_overlay_rects();`
    fn get_overlay_rects(&self) -> Vec<render_metal::OverlayRect> {
        let mut rects: Vec<render_metal::OverlayRect> = Vec::new();
        if self.gun == Gun::Rifle {
            let ([x, y], [w, h]) = (HEAT_BAR_MIN_FRAC, ENERGY_BAR_SIZE_FRAC);
            rects.extend([
                render_metal::OverlayRect {
                    min_frac: [x, y],
                    max_frac: [x + w, y + h],
                    color: ENERGY_BAR_BACK_COLOR,
                },
                render_metal::OverlayRect {
                    min_frac: [x, y],
                    max_frac: [x + w * self.rifle_heat_frac.min(1.0) as f32, y + h],
                    color: if self.is_overheated {
                        HEAT_BAR_OVERHEATED_COLOR
                    } else {
                        HEAT_BAR_FILL_COLOR
                    },
                },
            ]);
        }
        if let Some((_, held_s)) = self.multiplayer.as_ref().and_then(|m| m.disabling) {
            let ([x, y], [w, h]) = (DISABLE_BAR_MIN_FRAC, DISABLE_BAR_SIZE_FRAC);
            let fill_frac: f32 =
                (held_s / self.settings.consoles.hold_to_disable_s).min(1.0) as f32;
            rects.extend([
                render_metal::OverlayRect {
                    min_frac: [x, y],
                    max_frac: [x + w, y + h],
                    color: DISABLE_BAR_BACK_COLOR,
                },
                render_metal::OverlayRect {
                    min_frac: [x, y],
                    max_frac: [x + w * fill_frac, y + h],
                    color: DISABLE_BAR_FILL_COLOR,
                },
            ]);
        }
        let Some(voxel) = &self.voxel else {
            return rects;
        };
        let [x, y] = ENERGY_BAR_MIN_FRAC;
        let [w, h] = ENERGY_BAR_SIZE_FRAC;
        rects.extend([
            render_metal::OverlayRect {
                min_frac: [x, y],
                max_frac: [x + w, y + h],
                color: ENERGY_BAR_BACK_COLOR,
            },
            render_metal::OverlayRect {
                min_frac: [x, y],
                max_frac: [x + w * voxel.player.energy_frac as f32, y + h],
                color: ENERGY_BAR_FILL_COLOR,
            },
        ]);
        rects
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
        let energy: String = self.voxel.as_ref().map_or(String::new(), |voxel| {
            format!(
                ", energy {:.0}%",
                voxel.player.energy_frac * PERCENT_PER_FRAC
            )
        });
        let gun: String = match (self.gun, self.is_overheated) {
            (Gun::Laser, _) => ", gun 1 (laser)".to_string(),
            (Gun::Rifle, true) => ", gun 2 (rifle) overheated".to_string(),
            (Gun::Rifle, false) => format!(
                ", gun 2 (rifle) heat {:.0}%",
                self.rifle_heat_frac * PERCENT_PER_FRAC
            ),
        };
        let (is_flying, is_on_ground, height_m) = match &self.voxel {
            Some(voxel) => (
                voxel.player.is_flying,
                voxel.player.is_on_ground,
                voxel.player.h_m - voxel.world.player.eye_height_m,
            ),
            None => (
                self.player.is_flying,
                self.player.is_on_ground,
                self.player.get_height_m(&self.world),
            ),
        };
        let movement: &str = if is_flying {
            "flying"
        } else if is_on_ground {
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
            format!("{movement}, height {:.1} blocks{energy}{gun}", height_m),
        ];
        if let Some(laser) = &self.laser {
            lines.push(format!("laser {laser}"));
        }
        if let Some(multiplayer) = &self.multiplayer {
            lines.push(match multiplayer.client.id {
                Some(id) => format!("player {id}, {} others", multiplayer.client.players.len()),
                None if multiplayer.is_eliminated => format!(
                    "You died; respawning in {:.0} s",
                    multiplayer.respawn_in_s.ceil()
                ),
                None => "joining".to_string(),
            });
        }
        if self.message_time_s > 0.0 {
            lines.push(self.message.clone());
        }
        lines.push(if self.is_captured {
            "WASD move, Shift run, Space jump, click shoot (not while running), 1/2 or wheel switch gun, hold E on an enemy console to turn it off, R respawn, -/= scale, Esc release".to_string()
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

/// `get_eyes` gives where other players' eyes are, so spawning players can keep clear of them.
///
/// Example: `let others = get_eyes(&client.players);`
fn get_eyes(players: &[net::RemotePlayer]) -> Vec<DVec4> {
    players
        .iter()
        .map(|player| player.frame.col(AXIS_W))
        .collect()
}

/// `take_hit_weight` gives how many hits like this one `gun` needs to eliminate someone by
/// `settings` (a rifle hit to the head counts more), or `None` when this rifle shot already hit
/// (`has_rifle_shot_hit`, which it then sets): the laser reports every frame it hits, the rifle
/// once per shot.
///
/// Example: `if let Some(hits_to_kill) = take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false) { ... }`
fn take_hit_weight(
    settings: &GameSettings,
    gun: Gun,
    has_rifle_shot_hit: &mut bool,
    is_head_hit: bool,
) -> Option<u8> {
    match (gun, is_head_hit) {
        (Gun::Laser, _) => Some(settings.sniper.hits_to_eliminate),
        (Gun::Rifle, _) if *has_rifle_shot_hit => None,
        (Gun::Rifle, is_head) => {
            *has_rifle_shot_hit = true;
            Some(if is_head {
                settings.rifle.head_hits_to_eliminate
            } else {
                settings.rifle.body_hits_to_eliminate
            })
        }
    }
}

/// `make_wand` places the wand in camera space: held on the right, aimed toward the crosshair with
/// the front of its tip at the muzzle (`voxel_world::MUZZLE_M`, where the laser starts), dipping a little at each step (`phase_rad`), and, as `run_pose_frac` goes to 1
/// (eased), lowered, tipped down and rolled, bouncing and swinging with each stride; its tip has
/// `team`'s colour. The laser (`gun`) is a thin dark wand with a short glowing tip; the rifle is a
/// thick grey block with a long, thin glowing barrel. Recoil (`recoil_frac`, 1 just after a shot)
/// pushes it back toward the eye and tips it up.
///
/// Example: `let wand = make_wand(0.0, 1.2, 0.0, 1, Gun::Laser);`
fn make_wand(
    run_pose_frac: f64,
    phase_rad: f64,
    recoil_frac: f64,
    team: u32,
    gun: Gun,
) -> render_metal::OverlayWand {
    let jog: f32 = (run_pose_frac * run_pose_frac * (3.0 - 2.0 * run_pose_frac)) as f32;
    let (step, swing) = (phase_rad.sin().abs() as f32, phase_rad.sin() as f32);
    let lerp = |aim: f32, run: f32| aim + (run - aim) * jog;
    let bob_m: f32 = lerp(WAND_WALK_BOB_M, WAND_JOG_BOB_M) * step;
    let (shaft_half_m, tip_half_m) = match gun {
        Gun::Laser => (WAND_LASER_SHAFT_HALF_M, WAND_LASER_TIP_HALF_M),
        Gun::Rifle => (WAND_RIFLE_SHAFT_HALF_M, WAND_RIFLE_TIP_HALF_M),
    };
    let aim_rotation: Mat3 =
        Mat3::from_euler(EulerRot::YXZ, WAND_AIM_YAW_RAD, WAND_AIM_PITCH_RAD, 0.0);
    let aim_center: Vec3 = voxel_world::MUZZLE_M.as_vec3()
        + aim_rotation * (Vec3::Z * (shaft_half_m[2] + 2.0 * tip_half_m[2]));
    let kick: f32 = recoil_frac as f32;
    let center: Vec3 = aim_center.lerp(Vec3::from(WAND_JOG_CENTER_M), jog) - Vec3::Y * bob_m
        + Vec3::Z * WAND_RECOIL_M * kick;
    let rotation: Mat3 = Mat3::from_euler(
        EulerRot::YXZ,
        lerp(WAND_AIM_YAW_RAD, WAND_JOG_YAW_RAD),
        lerp(WAND_AIM_PITCH_RAD, WAND_JOG_PITCH_RAD) + WAND_RECOIL_TILT_RAD * kick,
        (WAND_JOG_ROLL_RAD + WAND_JOG_SWING_RAD * swing) * jog,
    );
    render_metal::OverlayWand {
        center_m: center.extend(0.0).to_array(),
        axes: [rotation.x_axis, rotation.y_axis, rotation.z_axis]
            .map(|axis| axis.extend(0.0).to_array()),
        shaft_half_m,
        shaft_color: match gun {
            Gun::Laser => WAND_LASER_SHAFT_COLOR,
            Gun::Rifle => WAND_RIFLE_SHAFT_COLOR,
        },
        tip_half_m,
        tip_color: if team == 1 {
            WAND_RED_TIP_COLOR
        } else {
            WAND_BLUE_TIP_COLOR
        },
    }
}

/// `make_death_mask` draws the banner lines (`Session::get_banner_lines`) with SDL's built-in text into a `DEATH_MASK_PX` mask,
/// one byte a pixel: `DEATH_TITLE_MASK` on the first line's text, `DEATH_COUNTDOWN_MASK` on the
/// second's, 0 elsewhere, for the Metal overlay.
///
/// Example: `let mask = make_death_mask(&lines)?;`
fn make_death_mask(lines: &[String; 2]) -> Result<Vec<u8>> {
    let (width, height) = DEATH_MASK_PX;
    let surface = sdl3::surface::Surface::new(width, height, PixelFormat::RGBA32)?;
    let mut canvas = surface.into_canvas()?;
    canvas.set_draw_color(Color::RGB(255, 255, 255));
    for (line, y) in lines.iter().zip([DEATH_TITLE_Y_PX, DEATH_COUNTDOWN_Y_PX]) {
        let x: f32 = (width as f32 - DEBUG_GLYPH_PX * line.len() as f32) / 2.0;
        canvas.draw_debug_text(line.as_str(), (x, y))?;
    }
    canvas.present();
    let surface = canvas.into_surface();
    let pitch: usize = surface.pitch() as usize;
    Ok(surface.with_lock(|pixels: &[u8]| {
        (0..height as usize)
            .flat_map(|y| (0..width as usize).map(move |x| (x, y)))
            .map(|(x, y)| match pixels[y * pitch + x * BYTES_PER_PIXEL] {
                0 => 0,
                _ if (y as f32) < DEATH_COUNTDOWN_Y_PX => DEATH_TITLE_MASK,
                _ => DEATH_COUNTDOWN_MASK,
            })
            .collect()
    }))
}

/// `draw_overlay_rects` draws the HUD's rectangles (`Session::get_overlay_rects`) on the canvas,
/// for the SDL-renderer window.
///
/// Example: `draw_overlay_rects(&mut canvas, &session.get_overlay_rects())?;`
fn draw_overlay_rects(
    canvas: &mut sdl3::render::WindowCanvas,
    rects: &[render_metal::OverlayRect],
) -> Result<()> {
    let (width, height) = canvas.window().size_in_pixels();
    canvas.set_scale(1.0, 1.0)?;
    canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
    for rect in rects {
        let [r, g, b, a] = rect.color.map(|c| (c * 255.0) as u8);
        canvas.set_draw_color(Color::RGBA(r, g, b, a));
        canvas.fill_rect(sdl3::render::FRect::new(
            rect.min_frac[0] * width as f32,
            rect.min_frac[1] * height as f32,
            (rect.max_frac[0] - rect.min_frac[0]) * width as f32,
            (rect.max_frac[1] - rect.min_frac[1]) * height as f32,
        ))?;
    }
    Ok(())
}

/// `draw_death_overlay` dims the canvas and draws the banner lines large in its middle, for the
/// SDL-renderer window.
///
/// Example: `draw_death_overlay(&mut canvas, &lines)?;`
fn draw_death_overlay(canvas: &mut sdl3::render::WindowCanvas, lines: &[String; 2]) -> Result<()> {
    let (width, height) = canvas.window().size_in_pixels();
    let scale: f32 = (height / DEATH_MASK_PX.1 / 2).max(1) as f32 / 2.0;
    canvas.set_scale(1.0, 1.0)?;
    canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
    canvas.set_draw_color(Color::RGBA(0, 0, 0, (DEATH_DIM_FRAC * 255.0) as u8));
    canvas.fill_rect(None)?;
    canvas.set_scale(scale, scale)?;
    for (line, y) in lines.iter().zip([DEATH_TITLE_Y_PX, DEATH_COUNTDOWN_Y_PX]) {
        canvas.set_draw_color(if y == DEATH_TITLE_Y_PX {
            Color::RGB(255, 90, 80)
        } else {
            HUD_COLOR
        });
        let x: f32 = (width as f32 / scale - DEBUG_GLYPH_PX * line.len() as f32) / 2.0;
        let top: f32 = (height as f32 / scale - DEATH_MASK_PX.1 as f32) / 2.0;
        canvas.draw_debug_text(line.as_str(), (x, top + y))?;
    }
    Ok(())
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
        session.update(&event_pump.keyboard_state())?;
        let render_px: (u32, u32) = session.get_render_size(canvas.window().size_in_pixels());
        pixels.resize(
            render_px.0 as usize * render_px.1 as usize * BYTES_PER_PIXEL,
            0,
        );
        let render_start: Instant = Instant::now();
        let packed: PackedFrame = session.pack(render_px);
        renderer.render(packed.as_frame_data(), render_px, &mut pixels)?;
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
        draw_overlay_rects(&mut canvas, &session.get_overlay_rects())?;
        if let Some(lines) = session.get_banner_lines() {
            draw_death_overlay(&mut canvas, &lines)?;
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
        session.update(&event_pump.keyboard_state())?;
        let window_px: (u32, u32) = window.size_in_pixels();
        layer.setDrawableSize(CGSize {
            width: f64::from(window_px.0),
            height: f64::from(window_px.1),
        });
        let render_px: (u32, u32) = session.get_render_size(window_px);
        let render_start: Instant = Instant::now();
        let packed: PackedFrame = session.pack(render_px);
        let death_mask: Option<Vec<u8>> = session
            .get_banner_lines()
            .map(|lines| make_death_mask(&lines))
            .transpose()?;
        let rects: Vec<render_metal::OverlayRect> = session.get_overlay_rects();
        let wand: Option<render_metal::OverlayWand> = session.get_wand();
        let overlay: Option<render_metal::Overlay> = (death_mask.is_some()
            || !rects.is_empty()
            || wand.is_some())
        .then(|| render_metal::Overlay {
            dim_frac: if death_mask.is_some() {
                DEATH_DIM_FRAC
            } else {
                0.0
            },
            mask: death_mask
                .as_deref()
                .map(|pixels| (pixels, (DEATH_MASK_PX.0 as usize, DEATH_MASK_PX.1 as usize))),
            rects: &rects,
            wand,
            blur_frac: session.motion_blur_frac,
        });
        if metal.render_to_layer(packed.as_frame_data(), layer, overlay.as_ref())? {
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

#[cfg(test)]
mod tests {
    //! Tests of the guns' hit rules.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Gun | laser (every frame's hit counts, one eliminates), rifle (one hit per shot) | `hits_weigh_by_gun_and_place` |
    //! | Where a rifle hits | the head, the body | `hits_weigh_by_gun_and_place` |

    use super::*;

    #[test]
    fn hits_weigh_by_gun_and_place() {
        let settings: GameSettings = GameSettings::Builtin();
        let mut has_hit: bool = false;
        for _ in 0..2 {
            assert_eq!(
                take_hit_weight(&settings, Gun::Laser, &mut has_hit, false),
                Some(settings.sniper.hits_to_eliminate)
            );
        }
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, true),
            Some(settings.rifle.head_hits_to_eliminate)
        );
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false),
            None
        );
        has_hit = false;
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false),
            Some(settings.rifle.body_hits_to_eliminate)
        );
    }
}
