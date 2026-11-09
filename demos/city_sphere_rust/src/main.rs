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
//! - `--black-hole-strength <0..1>`: on lunar-surface, force the black hole's strength (W-68).
//!
//! Example: `cargo run --release -- --bench-views --width-px 2560 --height-px 1600`
//!
//! Must know:
//! - Gotchas: run folders are found from the crate folder, not the current directory.

mod avatar_animation;
mod black_hole_growth;
mod city;
mod city_world;
mod city_wormholes;
mod credits;
mod dust;
mod map;
mod map_export;
mod map_layout;
mod menu;
mod minimap;
mod multiplayer;
mod net;
mod physics;
#[cfg(test)]
mod play_trace;
mod render_city;
mod render_cpu;
mod render_metal;
mod render_voxel;
mod scene_data;
mod settings;
mod terrain;
mod third_person;
mod thrown_wormholes;
mod title;
mod voxel_clearance;
mod voxel_textures;
mod voxel_world;
mod world;
mod zone_walls;

use warped_shared::health::{HitFeedback, HitResponse, fade_effect_frac};
use warped_shared::hud::{
    DISABLE_BAR_MIN_FRAC, DISABLE_BAR_RGB, DISABLE_BAR_SIZE_FRAC, HudBars, HudPulses, describe_gun,
    get_crosshair_rects, get_death_lines, get_lobby_lines, make_bar_rects, make_hit_effect,
};
use warped_shared::weapons::{Gun, Weapons, get_sway, make_wand, take_hit_weight};
use warped_shared::{
    app_bundle, avatar_model, exit_prompt, fps_demo_assets, geometry, health, hud_bars,
    key_bindings, mesh, overlay, play_settings, render_mesh, shapes, shot_noise, spaceships,
    texture_arrays, viewmodel,
};

use std::error::Error;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use glam::{DMat4, DVec3, DVec4};
use objc2_core_foundation::CGSize;
use objc2_metal::MTLPixelFormat;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Texture;
use serde_json::{Value, json};

use black_hole_growth::BlackHoleGrowth;
use geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, make_boost, make_rotation};
use key_bindings::{Action, Control};
use map::MapName;
use physics::{Input, LaserReport, Player};
use render_metal::FrameData;
use render_voxel::VoxelFrame;
use scene_data::{Beam, SceneData};
use settings::{PerPlayerSettings, ServerSettings};
use thrown_wormholes::{
    HIT_FLASH_COLOR, Marker, MouthPlace, PlayerWormholes, ThrownWormholes, fly_throw,
};
use voxel_world::{VoxelPlayer, VoxelWorld};
use world::{DEFAULT_CURVATURE_RADIUS_M, EYE_HEIGHT_M, World};

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;
const LONE_MOUTH_HITS_TO_DESTROY: u8 = 1;
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
const WINDOW_TITLE: &str = "City Sphere (Rust)";
const MS_PER_S: f64 = 1000.0;
const HUD_COLOR: Color = Color::RGB(235, 240, 250);
const MESSAGE_DURATION_S: f64 = 3.0;
const MAX_BLUESHIFT_FRAC: f64 = 1.25;
const SKY_TRACER_M: f64 = 80.0;
const DEATH_MASK_PX: (u32, u32) = (240, 24);
const MENU_MASK_PX: (u32, u32) = (512, 312);
const BANNER_HEIGHT_FRAC: f32 = 0.2;
const MENU_HEIGHT_FRAC: f32 = 0.85;
const TEXT_TOP_PX: f32 = 2.0;
const TEXT_LINE_PX: f32 = 12.0;
const MENU_LEFT_PX: f32 = 4.0;
const PLAYER_SETTINGS_HOME_PATH: &str = ".config/warped/player_settings.yaml";
const GAME_PASSWORD_HOME_PATH: &str = "Library/Application Support/Warped/game_password";
const LOGS_HOME_FOLDER: &str = "Library/Logs/Warped";
const ONLINE_GAMES_FILE: &str = "online_games.txt";
const OWNER_ONLY_FILE_MODE: u32 = 0o600;
const WRONG_PASSWORD_EXIT_CODE: u8 = 3;
const TITLE_WINDOW_PX: (u32, u32) = (960, 600);
const TITLE_FRAME_MS: u64 = 16;
const DEBUG_GLYPH_PX: f32 = 8.0;
const DEATH_DIM_FRAC: f32 = 0.55;
const SERVER_SILENT_S: f64 = 2.0;
const LOCAL_PLAY_OFF: &str = "Playing is only on a game server now: start one with `make serve-game` (--serve) and join it with `make join-game SERVER=<address> PORT=<port>` (--join <address>). Without a server, only the tools run: --bench-views, --autopilot, --play-route and --export-map.";
const MENU_DIM_FRAC: f32 = 0.8;
const MAP_AREA_MIN_MASK_PX: [i32; 2] = [8, 28];
const MAP_AREA_MAX_EXCL_MASK_PX: [i32; 2] = [504, 306];
const PICTURE_PX_PER_MASK_PX: i32 = 2;
const DEATH_TITLE_MASK: u8 = 255;
const DEATH_COUNTDOWN_MASK: u8 = 128;
const LOADING_DIM_FRAC: f32 = 1.0;
const LOADING_TITLE: &str = "LOADING";
const LOADING_DONE_LABEL: &str = "ready";
const PERCENT_PER_FRAC: f64 = 100.0;
const GUN_WALK_HZ: f64 = 2.0;
const GUN_JOG_HZ: f64 = 2.8;
const GUN_POSE_PER_S: f64 = 6.0;
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

/// `RouteKind` is which route `--play-route` plays: the house's scripted legs, a tour around each
/// of its mouths from 5 blocks to just outside its throat and back, (on a voxel map) a tour
/// around each warp object, or (on City Sphere) the rifle check or the walk up the stairs.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RouteKind {
    Legs,
    MouthTour,
    WarpTour,
    Rifle,
    Stairs,
    Wormholes,
}

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "City Sphere: fight inside a sphere with outward gravity, a glass shell and a glowing core.",
    after_help = "Example: city-sphere --bench-views --width-px 2560 --height-px 1600"
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
    /// City Sphere: render photogenic views (the far side from near the core, the sphere from its
    /// centre, hero shots with robots, pylons and shots) once each and save them under
    /// .artifacts/photos/.
    #[arg(long, conflicts_with_all = ["bench_views", "autopilot", "play_route"])]
    photos: bool,
    /// City Sphere: render the menu's map picture (the whole sphere seen from its centre,
    /// unrolled, supersampled) into assets/map_picture.png, with a copy and the old drawing under
    /// .artifacts/map_picture/.
    #[arg(long, conflicts_with_all = ["bench_views", "autopilot", "play_route", "photos"])]
    map_picture: bool,
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
    /// house), or on a voxel map a tour circling each warp object the same way (`warp-tour`), or
    /// on City Sphere `rifle`: overheat the rifle, let it cool, then eliminate a `--bot-objective
    /// target` bot with head hits and again with body hits, or `stairs`: walk from blue's spawn
    /// up its stairs onto the glass level and to its pylon, or `wormholes`: throw both wormhole
    /// mouths, shoot through them and walk through them.
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
    /// The numbers that shape play, a YAML file with server_settings.yaml's keys (the default): a
    /// server (--serve, --host) sends them to every player who joins; without a server, this game
    /// uses them.
    #[arg(long, conflicts_with = "join")]
    server_settings: Option<PathBuf>,
    /// A file holding the game's password: a server (--serve, --host) admits only players who prove
    /// they know it, and a player (--join, --host) proves it. Without it the password is empty, as
    /// suits a local network.
    #[arg(long)]
    game_password_file: Option<PathBuf>,
    /// This player's own choices (keys, mouse speed, head bob, motion blur, how a hit looks), a YAML
    /// file with player_settings.yaml's keys (the default).
    #[arg(long)]
    player_settings: Option<PathBuf>,
    /// Which map to play, from common/maps/.
    #[arg(long, value_enum, default_value_t = MapName::CitySphere)]
    map: MapName,
    /// With --map lunar-surface: start the window at one of the level's viewpoints (as
    /// --bench-views names them), e.g. black_hole.
    #[arg(long)]
    view: Option<String>,
    /// On a map whose black hole grows (lunar-surface): force the black hole's strength (0 to 1)
    /// instead of following the consoles (W-68).
    #[arg(long)]
    black_hole_strength: Option<f64>,
    /// Write the world as a map in the portable format of common/maps/ (map.json) into this folder,
    /// replacing the folder's map.json, and exit.
    #[arg(long, conflicts_with_all = ["bench_views", "autopilot", "play_route", "host", "join", "serve", "bot"])]
    export_map: Option<PathBuf>,
    /// With `--serve`: when a team wins, write `{"winner": "blue"}` or `{"winner": "red"}` to this
    /// file (atomically, through `<file>.partial`) and keep the game won instead of starting another.
    #[arg(long, requires = "serve")]
    level_result_file: Option<PathBuf>,
    /// Join this team at once instead of waiting in the lobby.
    #[arg(long, value_enum)]
    team: Option<city::TeamName>,
    /// What the bot does: `consoles` holds E on the other team's consoles until its team wins.
    #[arg(long, value_enum, requires = "bot")]
    bot_objective: Option<city::BotObjective>,
    /// City Sphere: present frames as soon as they are drawn instead of waiting for the display's
    /// refresh, to time the frames without the display's pacing.
    #[arg(long)]
    no_vsync: bool,
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
    if let Some(strength_frac) = arguments.black_hole_strength
        && (!(0.0..=1.0).contains(&strength_frac) || arguments.map != MapName::LunarSurface)
    {
        return Err(format!(
            "--black-hole-strength must be in [0, 1] with --map lunar-surface, got {strength_frac}; e.g. --map lunar-surface --black-hole-strength 0.6"
        )
        .into());
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
    if arguments.host {
        return Err(LOCAL_PLAY_OFF.into());
    }
    let is_window_without_server: bool = arguments.join.is_none()
        && !arguments.serve
        && !arguments.bench_views
        && !arguments.photos
        && !arguments.map_picture
        && !arguments.autopilot
        && !arguments.play_route
        && arguments.export_map.is_none();
    if is_window_without_server {
        return Err(LOCAL_PLAY_OFF.into());
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
fn main() -> ExitCode {
    let is_title_screen: bool = std::env::args_os().len() == 1;
    let result: Result<()> = if is_title_screen {
        run_title_screen()
    } else {
        run(Arguments::parse())
    };
    let Err(error) = result else {
        return ExitCode::SUCCESS;
    };
    eprintln!("Error: {error}");
    if is_title_screen {
        let _ = show_alert(&error.to_string());
    }
    match error.downcast_ref::<net::PasswordRefused>() {
        Some(_) => ExitCode::from(WRONG_PASSWORD_EXIT_CODE),
        None => ExitCode::FAILURE,
    }
}

/// `run` validates the command line and runs what it asks for: the window, a server, the bot, the
/// benchmark or the autopilot.
///
/// Example: `run(Arguments::parse())?;`
fn run(arguments: Arguments) -> Result<()> {
    validate_arguments(&arguments)?;
    if arguments.map == MapName::CitySphere {
        return city::run(&arguments);
    }
    if let Some(folder) = &arguments.export_map {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let path: PathBuf = map_export::export_map(&world, folder)?;
        println!("Wrote {}", path.display());
        return Ok(());
    }
    if arguments.serve {
        return serve_only(&arguments);
    }
    if arguments.renderer == RendererKind::Metal {
        viewmodel::check_gun_models()?;
        if arguments.map == MapName::LunarSurface {
            voxel_textures::check_surface_textures()?;
        }
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

/// `run_title_screen` shows the title screen (`title.rs`) until the player quits, starting each
/// chosen game as this program with that game's flags and showing why in a pop-up when one fails.
///
/// Example: `run_title_screen()?;`
fn run_title_screen() -> Result<()> {
    let online_games_path: Option<PathBuf> = app_bundle::get_app_resources_folder()
        .map(|folder| folder.join(ONLINE_GAMES_FILE))
        .filter(|path| path.exists());
    let online_games: String = match &online_games_path {
        Some(path) => fs::read_to_string(path)?,
        None => String::new(),
    };
    let mut title_screen: title::TitleScreen = title::TitleScreen::New(&online_games)?;
    while let Some(choice) = pick_title_choice(&mut title_screen)? {
        if let Err(error) = start_title_choice(&choice) {
            show_alert(&error.to_string())?;
        }
    }
    Ok(())
}

/// `pick_title_choice` opens the title screen's window until the player picks a game (`Some`) or
/// quits (`None`).
///
/// Example: `let choice = pick_title_choice(&mut title_screen)?;`
fn pick_title_choice(title_screen: &mut title::TitleScreen) -> Result<Option<title::Choice>> {
    let sdl: sdl3::Sdl = sdl3::init()?;
    let window: sdl3::video::Window = sdl
        .video()?
        .window(WINDOW_TITLE, TITLE_WINDOW_PX.0, TITLE_WINDOW_PX.1)
        .high_pixel_density()
        .position_centered()
        .build()?;
    let mut canvas: sdl3::render::WindowCanvas = window.into_canvas();
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => return Ok(None),
                Event::KeyDown {
                    scancode: Some(scancode),
                    ..
                } => match scancode {
                    Scancode::Up | Scancode::W => title_screen.move_selection(-1),
                    Scancode::Down | Scancode::S => title_screen.move_selection(1),
                    Scancode::Escape => return Ok(None),
                    Scancode::Return | Scancode::KpEnter => {
                        let choice: title::Choice = title_screen.get_selected().clone();
                        return Ok((choice != title::Choice::Quit).then_some(choice));
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        draw_text_overlay(
            &mut canvas,
            &OverlayText {
                lines: title_screen.get_lines(),
                mask_px: MENU_MASK_PX,
                height_frac: MENU_HEIGHT_FRAC,
                is_centered: false,
                dim_frac: 0.0,
                picture: None,
            },
        )?;
        canvas.present();
        std::thread::sleep(Duration::from_millis(TITLE_FRAME_MS));
    }
}

/// `start_title_choice` runs the chosen game as this program with its flags until it ends, its
/// output in a log under `~/Library/Logs/Warped/`; an online game first asks for the game's
/// password once and keeps it, and forgets it if the server refuses it.
///
/// Example: `start_title_choice(&title::Choice::Join(MapName::HouseDemo))?;`
fn start_title_choice(choice: &title::Choice) -> Result<()> {
    let home: PathBuf = PathBuf::from(std::env::var_os("HOME").ok_or("HOME isn't set")?);
    let password_path: PathBuf = home.join(GAME_PASSWORD_HOME_PATH);
    let mut flags: Vec<String> = choice.get_flags();
    if choice.is_online() {
        if !password_path.exists() {
            let Some(password) = ask_game_password()? else {
                return Ok(());
            };
            save_game_password(&password_path, &password)?;
        }
        flags.extend([
            "--game-password-file".to_string(),
            password_path.display().to_string(),
        ]);
    }
    let logs_folder: PathBuf = home.join(LOGS_HOME_FOLDER);
    fs::create_dir_all(&logs_folder)?;
    let log_path: PathBuf = logs_folder.join(format!("{}.log", format_utc_now()));
    let log: fs::File = fs::File::create(&log_path)?;
    let status: std::process::ExitStatus = Command::new(std::env::current_exe()?)
        .args(&flags)
        .stdout(log.try_clone()?)
        .stderr(log)
        .status()?;
    if status.code() == Some(i32::from(WRONG_PASSWORD_EXIT_CODE)) {
        fs::remove_file(&password_path)?;
    }
    if status.success() {
        return Ok(());
    }
    let text: String = fs::read_to_string(&log_path)?;
    let reason: &str = match text.lines().rev().find(|line| line.starts_with("Error: ")) {
        Some(line) => line,
        None => "It stopped without saying why.",
    };
    Err(format!(
        "The game stopped. {reason}\n\nIts log: {}",
        log_path.display()
    )
    .into())
}

/// `ask_game_password` asks the player for the game's password in a dialog, or gives `None` if
/// they cancel.
///
/// Example: `let password = ask_game_password()?;`
fn ask_game_password() -> Result<Option<String>> {
    let output: std::process::Output = Command::new("osascript")
        .args([
            "-e",
            "text returned of (display dialog \"The game's password (ask whoever runs the server):\" default answer \"\" with hidden answer with title \"Warped\")",
        ])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let password: String = String::from_utf8(output.stdout)?.trim_end().to_string();
    Ok((!password.is_empty()).then_some(password))
}

/// `save_game_password` writes `password` to `path`, readable only by this user.
///
/// Example: `save_game_password(&path, "correct horse")?;`
fn save_game_password(path: &Path, password: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::create_dir_all(path.parent().ok_or("a password file has a folder")?)?;
    let mut file: fs::File = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(OWNER_ONLY_FILE_MODE)
        .open(path)?;
    writeln!(file, "{password}")?;
    Ok(())
}

/// `show_alert` shows `message` in a pop-up and waits for the player to close it.
///
/// Example: `show_alert("The game stopped.")?;`
fn show_alert(message: &str) -> Result<()> {
    Command::new("osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display alert \"Warped\" message (item 1 of argv)",
            "-e",
            "end run",
            message,
        ])
        .status()?;
    Ok(())
}

/// `load_map` loads a map built into the program (`Map::Builtin`) and, on a voxel map (one with
/// structures, such as Lunar Surface), curves its space as `settings` say (`Map::scale_curvature`).
/// The house demo keeps its own curvature: at twice it, its far pylon sits 13 curvature radii out,
/// too far for the renderer's single precision, and its scripted route breaks.
///
/// Example: `let map = load_map(MapName::LunarSurface, &ServerSettings::Builtin())?;`
fn load_map(name: MapName, settings: &ServerSettings) -> Result<map::Map> {
    let mut map: map::Map = map::Map::Builtin(name.get_folder_name())?;
    if !map.structures.is_empty() {
        map.scale_curvature(settings.space.curvature_per_map_curvature)?;
    }
    Ok(map)
}

/// `load_world` loads a map from `common/maps/` (`load_map`) as the world of boxes and mouths
/// this renderer draws.
///
/// Example: `let world = load_world(MapName::HouseDemo, &ServerSettings::Builtin())?;`
fn load_world(name: MapName, settings: &ServerSettings) -> Result<World> {
    Ok(World::FromMap(&load_map(name, settings)?)?)
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
/// Example: `let (should_stop, thread) = start_server(47800, "lunar_surface", ServerSettings::Builtin(), vec![0, 1])?;`
fn start_server(
    port: u16,
    map_name: &str,
    settings: ServerSettings,
    console_teams: Vec<u32>,
    game_password: Vec<u8>,
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
            None,
            game_password,
            None,
            true,
            None,
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
    if let Some(path) = &arguments.server_settings {
        command.arg("--server-settings").arg(path);
    }
    if let Some(path) = &arguments.game_password_file {
        command.arg("--game-password-file").arg(path);
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

/// `load_settings` gives the server settings `--server-settings` names, or the built-in ones.
///
/// Example: `let settings = load_settings(&arguments)?;`
fn load_settings(arguments: &Arguments) -> Result<ServerSettings> {
    match &arguments.server_settings {
        Some(path) => ServerSettings::Load(path),
        None => Ok(ServerSettings::Builtin()),
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
    let settings: ServerSettings = load_settings(arguments)?;
    let map_data: map::Map = load_map(arguments.map, &settings)?;
    let consoles: Vec<voxel_world::Console> = voxel_world::find_consoles(&map_data);
    let (should_stop, thread) = start_server(
        arguments.port,
        arguments.map.get_folder_name(),
        settings,
        consoles.iter().map(|c| c.team).collect(),
        net::read_game_password(arguments.game_password_file.as_deref())?,
    )?;
    println!(
        "Server settings: {} (players who join get them).",
        arguments
            .server_settings
            .as_ref()
            .map_or("the built-in server_settings.yaml".to_string(), |path| path
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

/// `bench_voxel_views` renders a voxel map's viewpoints without a window, timing each, as
/// `bench_views` does for the house.
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

/// `run_bot` plays as a bot without a window for `--exit-after-s` seconds (see
/// `multiplayer::run_bot`), saving its view at each hit under `.artifacts/bot/<run_id>/`, and
/// prints how many players it hit and how often it was hit.
///
/// Example: `run_bot(&arguments, renderer)?;`
fn run_bot(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let map: map::Map = load_map(arguments.map, &ServerSettings::Builtin())?;
    let mut client: net::Client = net::Client::Connect(
        get_server_address(arguments)?,
        arguments.map.get_folder_name(),
        &net::read_game_password(arguments.game_password_file.as_deref())?,
    )?;
    client.join(None)?;
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
        multiplayer::run_voxel_bot(
            &world,
            &mut client,
            duration_s,
            arguments.black_hole_strength,
            |frame, id| save(FrameData::Voxel(frame), id),
        )?
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

/// `pack_view` packs the scene for a camera (`frame`) at a render size, with the laser if it is
/// on, fired from `laser_aim` (`frame` turned by the shot's noise; traced through the scene packed
/// from there, then packed again with its beam), and returns the laser's report.
///
/// Example: `let (data, report) = pack_view(&world, frame, Some(frame), (1280, 800), 0.0);`
fn pack_view(
    world: &World,
    frame: DMat4,
    laser_aim: Option<DMat4>,
    render_px: (u32, u32),
    time_s: f64,
) -> (SceneData, Option<LaserReport>) {
    let data: SceneData = SceneData::Build(world, frame, &Beam::default(), render_px, time_s, true);
    let Some(aim) = laser_aim else {
        return (data, None);
    };
    let aimed: SceneData = if aim == frame {
        data
    } else {
        SceneData::Build(world, aim, &Beam::default(), render_px, time_s, true)
    };
    let (beam, report) = physics::trace_laser(world, &aimed);
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
    let (data, report) = pack_view(
        world,
        frame,
        is_laser_on.then_some(frame),
        render_px,
        time_s,
    );
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
    let map: map::Map = load_map(arguments.map, &load_settings(arguments)?)?;
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
    let world: World = load_world(arguments.map, &load_settings(arguments)?)?;
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

/// `get_blueshift_frac` gives how far the scene blueshifts (the overlay's `blueshift_frac`: 1 for
/// walking straight ahead at a mouth's throat, negative for a redshift, 0 for none): `strength_frac`
/// times how deep the eye is in a mouth (`VoxelWorld::get_mouth_depth_frac`) times the speed
/// along the view in walking speeds, held within ±`MAX_BLUESHIFT_FRAC`.
///
/// Example: `let blueshift_frac = get_blueshift_frac(0.5, -10.0, 10.0, 1.0);`
fn get_blueshift_frac(
    depth_frac: f64,
    view_m_per_s: f64,
    walk_m_per_s: f64,
    strength_frac: f64,
) -> f32 {
    (strength_frac * depth_frac * view_m_per_s / walk_m_per_s)
        .clamp(-MAX_BLUESHIFT_FRAC, MAX_BLUESHIFT_FRAC) as f32
}

/// `Session` is the state of a played window: the map it loaded, the world, the player, the
/// toggles and the HUD's running averages.
struct Session {
    map_name: MapName,
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
    since_movement_key_s: f64,
    settings: ServerSettings,
    player_settings: PerPlayerSettings,
    player_settings_path: Option<PathBuf>,
    menu: Option<menu::Menu>,
    last_connection_problem: Option<ConnectionProblem>,
    exit_prompt: exit_prompt::ExitPrompt,
    map_layout: map_layout::MapLayout,
    weapons: Weapons,
    is_flashlight_on: bool,
    view_switch: third_person::ViewSwitch,
    is_own_body_shown: bool,
    tracers: Vec<Tracer>,
    dust_bursts: Vec<dust::DustBurst>,
    was_firing: bool,
    remote_firing_ids: Vec<u32>,
    avatars: avatar_animation::AvatarAnimator,
    motion_blur_frac: f32,
    hit_effect_frac: f64,
    blueshift_frac: f32,
    hit_feedback: HitFeedback,
    hud_pulses: HudPulses,
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
    thrown: ThrownWormholes,
    thrown_pairs: Vec<[MouthPlace; 2]>,
    thrown_pair_owners: Vec<Option<u32>>,
    wormhole_damages: Vec<(Option<u32>, f64)>,
    wormhole_flashes: Vec<(Option<u32>, f64)>,
    has_shot_hit_wormholes: bool,
}

/// `VoxelState` is a window's game on a voxel map: the map, the player, their spawn point and the
/// black hole's growth.
struct VoxelState {
    world: VoxelWorld,
    player: VoxelPlayer,
    spawn_index: usize,
    black_hole_growth: BlackHoleGrowth,
}

/// `PackedFrame` is one frame packed for the renderer: the house's rows or a voxel map's frame.
enum PackedFrame {
    House(Box<SceneData>),
    Voxel(Box<VoxelFrame>),
}

impl PackedFrame {
    /// `zoom_in` narrows the camera's field of view so the world looks `zoom` times closer (1 for
    /// none), for aiming: the renderers read the view's width from `tan_half_fov` and the size of
    /// a pixel's view from `pixel_angle_rad`.
    ///
    /// Example: `packed.zoom_in(2.5);`
    fn zoom_in(&mut self, zoom: f64) {
        let zoom: f32 = zoom as f32;
        match self {
            PackedFrame::House(data) => {
                data.uniforms.tan_half_fov /= zoom;
                data.uniforms.pixel_angle_rad /= zoom;
            }
            PackedFrame::Voxel(frame) => {
                frame.uniforms.tan_half_fov /= zoom;
                frame.uniforms.pixel_angle_rad /= zoom;
            }
        }
    }

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
/// `spot` glows where it hit while the trail is there. It is drawn in its gun's style (`is_rifle`),
/// and a share `speckle_frac` of its pixels speckles each frame (the shot's noise). This player's
/// own shots (`is_own`) stay joined to the gun in hand while their trail still reaches the muzzle.
struct Tracer {
    segments: Vec<voxel_world::BeamSegment>,
    length_m: f64,
    spot: Option<glam::DVec4>,
    team: u32,
    is_rifle: bool,
    speckle_frac: f64,
    fired_s: f64,
    speed_m_per_s: f64,
    trail_s: f64,
    is_own: bool,
}

impl Tracer {
    /// `New` makes a tracer for `shot`, fired at `fired_s` by a player of `team` with the rifle
    /// (`is_rifle`) or the sniper, whose beam speed and trail come from `settings`, and whose
    /// beam speckles a share `speckle_frac` of its pixels. A shot that hit nothing flies only its
    /// first `SKY_TRACER_M`: farther out over the sky it is under a pixel wide, and the shader's
    /// single precision makes its glow noisy (D64, D66).
    ///
    /// Example: `let tracer = Tracer::New(&shot, 1, 12.5, false, 0.1, &settings, world.r);`
    #[allow(non_snake_case)]
    fn New(
        shot: &voxel_world::Shot,
        team: u32,
        fired_s: f64,
        is_rifle: bool,
        speckle_frac: f64,
        settings: &ServerSettings,
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
        let segments: Vec<voxel_world::BeamSegment> = if shot.is_hit {
            shot.segments.clone()
        } else {
            voxel_world::clip_beam(&shot.segments, r, 0.0, SKY_TRACER_M)
        };
        Tracer {
            length_m: segments.iter().map(|s| s.length).sum::<f64>() * r,
            segments,
            spot: shot.spot,
            team,
            is_rifle,
            speckle_frac,
            fired_s,
            speed_m_per_s,
            trail_s,
            is_own: false,
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

    /// `get_arrival_s` gives when the beam's front reaches the end of its path, in seconds into
    /// the session.
    ///
    /// Example: `let start_s = tracer.get_arrival_s();`
    fn get_arrival_s(&self) -> f64 {
        self.fired_s + self.length_m / self.speed_m_per_s
    }
}

/// `add_dust_burst` adds the dust burst of `shot`, fired as `tracer` by the rifle (`is_rifle`) or
/// the sniper, to `bursts` when it hit the ground of `world` (`DustBurst::FromShot`), starting when
/// its beam front arrives, and drops the oldest bursts beyond `MAX_DUST_BURST_COUNT`.
///
/// Example: `add_dust_burst(&mut self.dust_bursts, &voxel.world, shot, &tracer, false);`
fn add_dust_burst(
    bursts: &mut Vec<dust::DustBurst>,
    world: &VoxelWorld,
    shot: &voxel_world::Shot,
    tracer: &Tracer,
    is_rifle: bool,
) {
    let Some(burst) = dust::DustBurst::FromShot(world, shot, tracer.get_arrival_s(), is_rifle)
    else {
        return;
    };
    bursts.push(burst);
    let excess_count: usize = bursts.len().saturating_sub(dust::MAX_DUST_BURST_COUNT);
    bursts.drain(..excess_count);
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
        let settings: ServerSettings = load_settings(arguments)?;
        let map: map::Map = load_map(arguments.map, &settings)?;
        let player_settings_path: Option<PathBuf> =
            arguments.player_settings.clone().or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(PLAYER_SETTINGS_HOME_PATH))
            });
        let player_settings: PerPlayerSettings = match &player_settings_path {
            Some(path) if arguments.player_settings.is_some() || path.exists() => {
                eprintln!("Player settings: {}", path.display());
                PerPlayerSettings::Load(path)?
            }
            _ => PerPlayerSettings::Builtin(),
        };
        let voxel: Option<VoxelState> = if map.structures.is_empty() {
            None
        } else {
            let mut world: VoxelWorld = VoxelWorld::FromMap(&map)?;
            world.set_settings(settings.clone());
            world.head_bob = player_settings.view.head_bob.clone();
            world.milky_way_brightness_frac = player_settings.view.milky_way_brightness_frac;
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
                black_hole_growth: BlackHoleGrowth::New(arguments.black_hole_strength),
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
            (true, RouteKind::Rifle | RouteKind::Stairs | RouteKind::Wormholes) => {
                return Err(
                    "--route rifle, stairs and wormholes are only for --map city-sphere".into(),
                );
            }
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
                    &net::read_game_password(arguments.game_password_file.as_deref())?,
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
            map_name: arguments.map,
            world,
            player,
            settings,
            player_settings,
            player_settings_path,
            menu: None,
            last_connection_problem: None,
            exit_prompt: exit_prompt::ExitPrompt::New(),
            map_layout: map_layout::MapLayout::FromMap(&map, get_map_picture_max_px())?,
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
            since_movement_key_s: f64::INFINITY,
            weapons: Weapons::New(shot_noise::ShotNoise::FromClock()),
            is_flashlight_on: false,
            view_switch: third_person::ViewSwitch::FirstPerson(),
            is_own_body_shown: false,
            tracers: Vec::new(),
            dust_bursts: Vec::new(),
            was_firing: false,
            remote_firing_ids: Vec::new(),
            avatars: avatar_animation::AvatarAnimator::New(),
            motion_blur_frac: 0.0,
            hit_effect_frac: 0.0,
            blueshift_frac: 0.0,
            hit_feedback: HitFeedback::New(),
            hud_pulses: HudPulses::New(),
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
            thrown: ThrownWormholes::New(),
            thrown_pairs: Vec::new(),
            thrown_pair_owners: Vec::new(),
            wormhole_damages: Vec::new(),
            wormhole_flashes: Vec::new(),
            has_shot_hit_wormholes: false,
        })
    }

    /// `handle_event` reacts to one window event and returns false when the player quits: the
    /// window's close button quits, Esc releases the mouse (or asks whether to exit if it is free,
    /// and while it asks every pressed control answers it), a click captures it (and once captured,
    /// a left click fires a shot), the mouse looks (more slowly while aiming zooms in, by the same
    /// factor), `1`, `2` and the wheel switch guns, `V` switches between first and third person,
    /// and - and = change the render scale. Players can't respawn by choice.
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
            Event::MouseButtonDown { .. } if !self.is_captured && !self.exit_prompt.is_shown() => {
                self.is_captured = true;
                mouse.set_relative_mouse_mode(window, true);
                return true;
            }
            Event::MouseMotion { xrel, yrel, .. } if self.is_captured && self.menu.is_none() => {
                let speed: f64 = self.player_settings.view.look_rad_per_px
                    / voxel_world::LOOK_RAD_PER_PX
                    / self.get_zoom();
                let (dx_px, dy_px) = (f64::from(xrel) * speed, f64::from(yrel) * speed);
                match &mut self.voxel {
                    Some(voxel) => voxel.player.look(dx_px, dy_px),
                    None => physics::look_around(&mut self.player.frame, dx_px, dy_px),
                }
            }
            _ => {}
        }
        match key_bindings::get_pressed_control(&event) {
            Some(control) if self.exit_prompt.is_shown() => {
                self.exit_prompt.answer(control) != exit_prompt::ExitOutcome::Exit
            }
            Some(control) if self.menu.is_some() => {
                self.handle_menu_control(control);
                true
            }
            Some(control) if self.player_settings.keys.matches(Action::OpenMenu, control) => {
                self.menu = Some(menu::Menu::New());
                true
            }
            Some(control) => {
                self.handle_control(control, window, mouse);
                true
            }
            None => true,
        }
    }

    /// `handle_menu_control` passes a pressed control to the open menu, closes it or saves when
    /// the menu says so, and puts the changed head bob and Milky Way brightness into the world.
    ///
    /// Example: `self.handle_menu_control(Control::Key(Scancode::Down));`
    fn handle_menu_control(&mut self, control: Control) {
        let menu: &mut menu::Menu = self.menu.as_mut().expect("the menu is open");
        match menu.handle(control, &mut self.player_settings) {
            menu::MenuOutcome::Stay => {}
            menu::MenuOutcome::Close => self.menu = None,
            menu::MenuOutcome::Save => {
                let message: String = match self.save_player_settings() {
                    Ok(path) => format!("Saved to {path}"),
                    Err(error) => format!("Couldn't save: {error}"),
                };
                self.menu
                    .as_mut()
                    .expect("the menu is open")
                    .show_message(message);
            }
        }
        if let Some(voxel) = &mut self.voxel {
            voxel.world.head_bob = self.player_settings.view.head_bob.clone();
            voxel.world.milky_way_brightness_frac =
                self.player_settings.view.milky_way_brightness_frac;
        }
    }

    /// `save_player_settings` writes this player's settings to their file (`--player-settings`, or
    /// `~/.config/warped/player_settings.yaml`, which later games load), giving its path.
    ///
    /// Example: `let path = self.save_player_settings()?;`
    fn save_player_settings(&self) -> Result<String> {
        let path: &PathBuf = self
            .player_settings_path
            .as_ref()
            .ok_or("no file to save to: pass --player-settings (HOME isn't set)")?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(path, self.player_settings.to_yaml())?;
        Ok(path.display().to_string())
    }

    /// `handle_control` does what a pressed control (key, mouse button or wheel) is bound to:
    /// releasing the mouse (or asking whether to exit when it is free), the render resolution,
    /// choosing a team in the lobby (before the guns, which may share its keys), the guns, the
    /// view (first or third person), and firing the sniper.
    ///
    /// Example: `self.handle_control(control, window, mouse);`
    fn handle_control(
        &mut self,
        control: Control,
        window: &sdl3::video::Window,
        mouse: &sdl3::mouse::MouseUtil,
    ) {
        let actions: Vec<Action> = self.player_settings.keys.get_actions(control);
        let is = |action: Action| actions.contains(&action);
        if is(Action::ReleaseMouse) {
            if self.exit_prompt.press_release_mouse(self.is_captured)
                == exit_prompt::ExitOutcome::ReleaseMouse
            {
                self.is_captured = false;
                mouse.set_relative_mouse_mode(window, false);
            }
        } else if is(Action::LowerResolution) || is(Action::RaiseResolution) {
            self.is_auto_scale = false;
            self.render_scale = if is(Action::LowerResolution) {
                (self.render_scale / RENDER_SCALE_STEP).max(MIN_RENDER_SCALE)
            } else {
                (self.render_scale * RENDER_SCALE_STEP).min(1.0)
            };
        } else if self.is_in_lobby() && (is(Action::JoinBlue) || is(Action::JoinRed)) {
            let team: u32 = u32::from(is(Action::JoinRed));
            let multiplayer: &mut Multiplayer = self.multiplayer.as_mut().expect("in the lobby");
            if let Err(error) = multiplayer.client.join(Some(team)) {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
        } else if is(Action::Gun1) {
            self.switch_gun(Gun::Laser);
        } else if is(Action::Gun2) {
            self.switch_gun(Gun::Rifle);
        } else if is(Action::SwitchGun) {
            self.switch_gun(match self.weapons.gun {
                Gun::Laser => Gun::Rifle,
                Gun::Rifle => Gun::Laser,
            });
        } else if is(Action::SwitchView) {
            self.view_switch.toggle();
        }
        if is(Action::ThrowWormhole) && self.can_throw() {
            self.throw_wormhole();
        }
        if is(Action::Shoot)
            && self.is_captured
            && self.weapons.gun == Gun::Laser
            && self.weapons.sniper_reload_s <= 0.0
            && self.can_shoot()
            && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated)
        {
            self.has_shot_hit_wormholes = false;
            self.weapons.fire_sniper(&self.settings.sniper);
        }
    }

    /// `update` moves the player by the held keys for the time since the last frame, and fails if
    /// the game server can't be read (it plays another map, say). Near a black hole the player's
    /// own time runs slower (W-107, `get_time_rate_frac`): moving, turning off a console and the
    /// guns go by that time, while looking around, the menus and respawning keep the window's.
    ///
    /// Example: `session.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;`
    fn update(
        &mut self,
        keyboard: &sdl3::keyboard::KeyboardState,
        mouse: &sdl3::mouse::MouseState,
    ) -> Result<()> {
        let frame_s: f64 = self.last.elapsed().as_secs_f64();
        let dt_s: f64 = frame_s.min(MAX_FRAME_DT_S);
        let own_dt_s: f64 = dt_s * self.get_time_rate_frac();
        let blur_s: f64 = self.player_settings.view.motion_blur_s;
        self.hit_effect_frac = fade_effect_frac(
            self.hit_effect_frac,
            dt_s,
            self.player_settings.hit_effects.effect_s,
        );
        self.update_bar_pulses(dt_s);
        self.view_switch.update(dt_s);
        self.blueshift_frac = 0.0;
        self.motion_blur_frac = if blur_s > 0.0 {
            (-dt_s / blur_s).exp() as f32
        } else {
            0.0
        };
        self.last = Instant::now();
        if self.route_index < self.route.len() {
            self.frame_times_ms.push(frame_s * MS_PER_S);
            let step: &RouteFrame = &self.route[self.route_index];
            (self.player.frame, self.weapons.is_laser_on) = (step.frame, step.is_laser_on);
            self.route_index += 1;
            self.hud_time_s += dt_s;
            return Ok(());
        }
        self.weapons.tick(own_dt_s);
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
        let is_out: bool = self.update_multiplayer()?;
        self.report_connection_problem();
        if is_out {
            self.message_time_s -= dt_s;
            self.hud_time_s += dt_s;
            return Ok(());
        }
        let is_playing: bool = self.menu.is_none() && !self.exit_prompt.is_shown();
        let held = |action: Action| {
            is_playing && self.player_settings.keys.is_held(action, keyboard, mouse)
        };
        let input: Input = Input {
            forward: held(Action::MoveForward),
            back: held(Action::MoveBack),
            left: held(Action::MoveLeft),
            right: held(Action::MoveRight),
            up: held(Action::Jump),
            down: false,
            sprint: held(Action::Sprint),
            crouch: held(Action::Crouch),
        };
        self.weapons.is_trigger_held = self.is_captured && held(Action::Shoot);
        self.is_flashlight_on = self.is_captured && held(Action::Flashlight);
        let is_aiming: bool = self.is_captured
            && held(Action::Aim)
            && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated);
        self.weapons.update_aim(is_aiming, dt_s, &self.settings.aim);
        let is_holding_e: bool = held(Action::TurnOffConsole);
        let team: usize = self.get_team() as usize;
        let messages: Vec<String> = match &mut self.voxel {
            Some(voxel) => {
                let report: voxel_world::StepReport =
                    voxel.player.step(&voxel.world, &input, own_dt_s, team);
                let pitch_rad: f64 = voxel.player.pitch_rad;
                let view_m_per_s: f64 = report.forward_m_per_s * pitch_rad.cos()
                    + voxel.player.vertical_m_per_s * pitch_rad.sin();
                let eye: DVec4 = voxel.player.get_camera_frame(&voxel.world).col(AXIS_W);
                self.blueshift_frac = get_blueshift_frac(
                    voxel.world.get_mouth_depth_frac(eye),
                    view_m_per_s,
                    voxel.world.player.walk_m_per_s,
                    self.player_settings.view.wormhole_tint_frac,
                );
                report.messages
            }
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
        self.update_wormholes();
        self.update_disabling(is_holding_e, own_dt_s);
        self.update_gun(&input, own_dt_s);
        self.update_rifle(own_dt_s);
        let eye: DMat4 = self.get_eye_frame();
        let laser_aim: Option<DMat4> = self.get_laser_aim(self.get_shot_eye());
        if let Some(multiplayer) = &mut self.multiplayer
            && let Err(error) = multiplayer.client.send_state(
                laser_aim.unwrap_or(eye),
                laser_aim.is_some(),
                self.weapons.gun == Gun::Rifle,
                multiplayer.disabling.map(|(console, _)| console),
                self.weapons.beam_speckle_frac,
                self.thrown.placed,
            )
        {
            self.message = format!("Couldn't reach the server: {error}");
            self.message_time_s = MESSAGE_DURATION_S;
        }
        self.message_time_s -= dt_s;
        self.hud_time_s += dt_s;
        Ok(())
    }

    /// `get_hud_bars` gives what the HUD's bars show now: this player's health in a game on a
    /// server, the sprint energy on a voxel map, and how far turning off a console has gone while
    /// holding `E` on one.
    ///
    /// Example: `let bars: HudBars = self.get_hud_bars();`
    fn get_hud_bars(&self) -> HudBars {
        HudBars {
            health_frac: self.multiplayer.as_ref().map(|m| m.client.health_frac),
            energy_frac: self.voxel.as_ref().map(|voxel| voxel.player.energy_frac),
            console_hold_frac: self
                .multiplayer
                .as_ref()
                .and_then(|m| m.disabling)
                .map(|(_, held_s)| held_s / self.settings.consoles.hold_to_disable_s),
        }
    }

    /// `update_bar_pulses` moves the pulses running along the HUD's bars on by `dt_s`
    /// (`hud_bars.rs`), driven by how fast each bar's value changes: this player's health in a
    /// game on a server, and the sprint energy on a voxel map.
    ///
    /// Example: `self.update_bar_pulses(dt_s);`
    fn update_bar_pulses(&mut self, dt_s: f64) {
        let bars: HudBars = self.get_hud_bars();
        self.hud_pulses.update(dt_s, &bars, &self.settings.rifle);
    }

    /// `can_throw` says whether this player may throw a wormhole mouth now: playing, with the
    /// mouse captured, without a throw in the air, and without both mouths down.
    ///
    /// Example: `if self.can_throw() { self.throw_wormhole(); }`
    fn can_throw(&self) -> bool {
        self.is_captured
            && self.route.is_empty()
            && !self.is_in_lobby()
            && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated)
            && self.thrown.can_throw()
    }

    /// `throw_wormhole` throws this player's next wormhole mouth from their eye.
    ///
    /// Example: `self.throw_wormhole();`
    fn throw_wormhole(&mut self) {
        let (camera, color) = (self.get_eye_frame(), self.thrown.get_next_color());
        let settings = &self.settings.wormholes;
        let flight = match &self.voxel {
            Some(voxel) => fly_throw(camera, color, settings, voxel.world.r, |x, v, s| {
                voxel.world.find_hit_s(x, v, s)
            }),
            None => fly_throw(
                camera,
                color,
                settings,
                self.world.curvature_radius_m,
                |x, v, s| self.world.find_hit_s(x, v, s),
            ),
        };
        self.thrown
            .throw(flight, self.start.elapsed().as_secs_f64());
    }

    /// `update_wormholes` lands this player's throw once it comes down (where the map allows, clear
    /// of the other mouths down, other players' too), flashes and destroys wormholes by the
    /// server's damage (`update_wormhole_damage`), and puts every linked pair, this player's first,
    /// into the world when they change, flashing red where they were just hit.
    ///
    /// Example: `self.update_wormholes();`
    fn update_wormholes(&mut self) {
        let now_s: f64 = self.start.elapsed().as_secs_f64();
        let mut others: Vec<MouthPlace> = self.thrown.get_kept_places();
        for (_, wormholes) in &self.get_all_wormholes()[1..] {
            others.extend(wormholes.get_places());
        }
        let message: Option<String> = match &self.voxel {
            Some(voxel) => {
                let (eye_m, r) = (voxel.world.player.eye_height_m, voxel.world.r);
                let world: &VoxelWorld = &voxel.world;
                self.thrown
                    .update(now_s, eye_m, r, |p| world.can_place_mouth(p, &others))
            }
            None => {
                let (world, r) = (&self.world, self.world.curvature_radius_m);
                self.thrown.update(now_s, EYE_HEIGHT_M, r, |p| {
                    world.can_place_mouth(p, &others)
                })
            }
        };
        if let Some(message) = message {
            self.show_message(message);
        }
        self.update_wormhole_damage(now_s);
        let (owners, pairs): (Vec<Option<u32>>, Vec<[MouthPlace; 2]>) = self
            .get_all_wormholes()
            .into_iter()
            .filter_map(|(owner, wormholes)| Some((owner, wormholes.get_pair()?)))
            .unzip();
        let flashing: Vec<bool> = owners.iter().map(|&o| self.is_flashing(o, now_s)).collect();
        if pairs != self.thrown_pairs {
            match &mut self.voxel {
                Some(voxel) => voxel.world.set_thrown_wormholes(&pairs),
                None => self.world.set_thrown_wormholes(&pairs),
            }
            (self.thrown_pairs, self.thrown_pair_owners) = (pairs, owners);
        }
        match &mut self.voxel {
            Some(voxel) => voxel.world.set_flashing_pairs(&flashing),
            None => self.world.set_flashing_pairs(&flashing),
        }
    }

    /// `update_wormhole_damage` makes the wormholes of every player whose damage (from the server)
    /// rose flash red for the settings' `wormholes.hit_flash_s` from `now_s`, and destroys this
    /// player's once theirs reaches 1.
    ///
    /// Example: `self.update_wormhole_damage(now_s);`
    fn update_wormhole_damage(&mut self, now_s: f64) {
        let all: Vec<(Option<u32>, PlayerWormholes)> = self.get_all_wormholes();
        for &(owner, wormholes) in &all {
            let last_frac: f64 = match self.wormhole_damages.iter().find(|(o, _)| *o == owner) {
                Some(&(_, frac)) => frac,
                None => 0.0,
            };
            if wormholes.damage_frac > last_frac {
                self.wormhole_flashes.retain(|(o, _)| *o != owner);
                let until_s: f64 = now_s + self.settings.wormholes.hit_flash_s;
                self.wormhole_flashes.push((owner, until_s));
            }
        }
        self.wormhole_damages = all.iter().map(|&(o, w)| (o, w.damage_frac)).collect();
        if all[0].1.damage_frac >= 1.0 && !self.thrown.placed.get_places().is_empty() {
            self.thrown.clear();
            self.show_message("Your wormholes were destroyed".to_string());
        }
    }

    /// `is_flashing` says whether `owner`'s wormholes (`None` for this player's) flash red at `now_s`.
    ///
    /// Example: `let is_red = self.is_flashing(Some(3), now_s);`
    fn is_flashing(&self, owner: Option<u32>, now_s: f64) -> bool {
        self.wormhole_flashes
            .iter()
            .any(|&(o, until_s)| o == owner && until_s > now_s)
    }

    /// `get_all_wormholes` gives every player's wormholes with their owner: this player's first
    /// (`None`, with the damage the server counts for them), then the others' by the last snapshot.
    ///
    /// Example: `let all = self.get_all_wormholes();`
    fn get_all_wormholes(&self) -> Vec<(Option<u32>, PlayerWormholes)> {
        let (own_damage_frac, remote) = self.multiplayer.as_ref().map_or((0.0, Vec::new()), |m| {
            let remote = m.client.players.iter().map(|p| (Some(p.id), p.wormholes));
            (m.client.wormhole_damage_frac, remote.collect())
        });
        let own: PlayerWormholes = PlayerWormholes {
            damage_frac: own_damage_frac,
            ..self.thrown.placed
        };
        std::iter::once((None, own)).chain(remote).collect()
    }

    /// `get_wormhole_markers` lists the glowing balls to draw for every player's thrown mouths, with
    /// their owner (as `get_all_wormholes`): throws in the air, and mouths down whose partner isn't
    /// (red while they flash).
    ///
    /// Example: `let markers = self.get_wormhole_markers();`
    fn get_wormhole_markers(&self) -> Vec<(Option<u32>, Marker)> {
        let r: f64 = match &self.voxel {
            Some(voxel) => voxel.world.r,
            None => self.world.curvature_radius_m,
        };
        let now_s: f64 = self.start.elapsed().as_secs_f64();
        let mut markers: Vec<(Option<u32>, Marker)> = Vec::new();
        for (owner, wormholes) in self.get_all_wormholes() {
            for mut marker in wormholes.get_markers(r) {
                if marker.is_mouth && self.is_flashing(owner, now_s) {
                    marker.rgb = HIT_FLASH_COLOR;
                }
                markers.push((owner, marker));
            }
        }
        markers
    }

    /// `report_wormhole_hits` tells the server, once per shot, whose enemy wormholes this player's
    /// shot went into (`entered_pairs`, indices into the world's thrown pairs), which counts toward
    /// destroying them by the gun, or hit (`hit_marker`, an index into `markers`, if it is a lone
    /// mouth's ball), which destroys it at once. A shot at this player's own or a teammate's
    /// wormholes does nothing.
    ///
    /// Example: `self.report_wormhole_hits(&[0], None, &markers);`
    fn report_wormhole_hits(
        &mut self,
        entered_pairs: &[usize],
        hit_marker: Option<usize>,
        markers: &[(Option<u32>, Marker)],
    ) {
        let Some(multiplayer) = &self.multiplayer else {
            return;
        };
        let team: Option<u32> = multiplayer.client.team;
        let is_enemy = |&(id, _): &(u32, u8)| {
            let player = multiplayer.client.players.iter().find(|p| p.id == id);
            player.is_some_and(|p| Some(p.team) != team)
        };
        let gun_hits_to_destroy: u8 = match self.weapons.gun {
            Gun::Laser => self.settings.wormholes.sniper_hits_to_destroy,
            Gun::Rifle => self.settings.wormholes.rifle_hits_to_destroy,
        };
        let mut enemy_hits: Vec<(u32, u8)> = entered_pairs
            .iter()
            .filter_map(|&i| self.thrown_pair_owners[i])
            .map(|id| (id, gun_hits_to_destroy))
            .chain(
                hit_marker
                    .filter(|&k| markers[k].1.is_mouth)
                    .and_then(|k| markers[k].0)
                    .map(|id| (id, LONE_MOUTH_HITS_TO_DESTROY)),
            )
            .filter(is_enemy)
            .collect();
        enemy_hits.dedup_by_key(|&mut (id, _)| id);
        if self.has_shot_hit_wormholes || enemy_hits.is_empty() {
            return;
        }
        self.has_shot_hit_wormholes = true;
        let multiplayer: &mut Multiplayer = self.multiplayer.as_mut().expect("checked above");
        for (id, hits_to_destroy) in enemy_hits {
            if let Err(error) = multiplayer.client.send_wormhole_hit(id, hits_to_destroy) {
                self.show_message(format!("Couldn't reach the server: {error}"));
                return;
            }
        }
    }

    /// `is_firing` says whether this player's laser is on: on for a scripted route, or a shot fired
    /// by a click in the last `sniper.shot_s` (the settings), and only while this player can shoot.
    ///
    /// Example: `let is_firing = self.is_firing();`
    fn is_firing(&self) -> bool {
        self.weapons.is_firing(self.can_shoot())
    }

    /// `get_laser_aim` gives the frame this player's laser fires from while it is on: `eye` (the
    /// eye's frame, turned toward the crosshair's point in third person: `get_shot_eye`) turned by
    /// this shot's noise (`shot_offset`), or `None` while it is off. The window sends it instead of
    /// the eye's frame while firing, so others trace the same shot.
    ///
    /// Example: `let laser_aim = self.get_laser_aim(self.get_shot_eye());`
    fn get_laser_aim(&self, eye: DMat4) -> Option<DMat4> {
        self.is_firing().then(|| self.weapons.shot_offset.aim(eye))
    }

    /// `get_boom_frac` gives how far out along its boom this player's camera is now
    /// (`third_person::ViewSwitch::get_boom_frac`): 0, at the eye, on the house and on a scripted
    /// route.
    ///
    /// Example: `if self.get_boom_frac() > 0.0 { ... }`
    fn get_boom_frac(&self) -> f64 {
        if self.voxel.is_none() || !self.route.is_empty() {
            return 0.0;
        }
        self.view_switch.get_boom_frac(self.weapons.aim_frac)
    }

    /// `get_third_person_view` gives, while the camera is out of the eye on a voxel map, where it
    /// is (`third_person::place_camera`) and the eye turned toward what its crosshair covers
    /// (`third_person::aim_at_crosshair`), both in the map with the other players' bodies; `None`
    /// while the camera is at the eye.
    ///
    /// Example: `if let Some((placed, shot_eye)) = self.get_third_person_view() { ... }`
    fn get_third_person_view(&self) -> Option<(third_person::ThirdPersonCamera, DMat4)> {
        let boom_frac: f64 = self.get_boom_frac();
        let voxel: &VoxelState = self.voxel.as_ref().filter(|_| boom_frac > 0.0)?;
        let players: &[net::RemotePlayer] = self
            .multiplayer
            .as_ref()
            .map_or(&[], |m| m.client.players.as_slice());
        let world: VoxelWorld = multiplayer::with_bodies(&voxel.world, players);
        let eye: DMat4 = voxel.player.get_camera_frame(&voxel.world);
        let placed: third_person::ThirdPersonCamera =
            third_person::place_camera(&world, eye, boom_frac);
        Some((
            placed,
            third_person::aim_at_crosshair(&world, eye, placed.frame),
        ))
    }

    /// `get_shot_eye` gives the frame a shot leaves from before its noise: the eye's, turned toward
    /// what the crosshair covers while the camera is out of the eye (`get_third_person_view`).
    ///
    /// Example: `let laser_aim = self.get_laser_aim(self.get_shot_eye());`
    fn get_shot_eye(&self) -> DMat4 {
        self.get_third_person_view()
            .map_or_else(|| self.get_eye_frame(), |(_, shot_eye)| shot_eye)
    }

    /// `can_shoot` says whether this player may shoot: once no movement key has been held for the
    /// server settings' `movement.shoot_after_moving_s` (walking crouched doesn't count, and
    /// standing, falling or flying with no key held may shoot), and, in a game on a server, only
    /// once in the game (welcomed, so not in the lobby, while joining or while out after a hit),
    /// not while holding `E` on a console, and not once a team has won.
    ///
    /// Example: `if self.can_shoot() { ... }`
    fn can_shoot(&self) -> bool {
        self.since_movement_key_s >= self.settings.movement.shoot_after_moving_s
            && self.multiplayer.as_ref().is_none_or(|m| {
                m.client.id.is_some() && m.disabling.is_none() && m.client.game.winner.is_none()
            })
    }

    /// `switch_gun` makes `gun` the one in hand, stopping a shot in flight.
    ///
    /// Example: `self.switch_gun(Gun::Rifle);`
    fn switch_gun(&mut self, gun: Gun) {
        self.weapons.switch_gun(gun);
    }

    /// `get_time_rate_frac` gives how fast this player's own time runs where their eye is, as a
    /// share of time far from the black hole (`VoxelWorld::get_time_rate_frac`, W-107): 1 off a
    /// voxel map.
    ///
    /// Example: `let own_dt_s = dt_s * self.get_time_rate_frac();`
    fn get_time_rate_frac(&self) -> f64 {
        self.voxel.as_ref().map_or(1.0, |voxel| {
            let eye: DMat4 = voxel.player.get_camera_frame(&voxel.world);
            voxel.world.get_time_rate_frac(eye.col(AXIS_W))
        })
    }

    /// `update_rifle` fires the rifle while its trigger is held and this player can shoot: a
    /// `RIFLE_PULSE_S` pulse of the beam every `RIFLE_SHOT_PERIOD_S`, each adding
    /// 1 / `RIFLE_OVERHEAT_SHOT_COUNT` of heat. At full heat it overheats; it cools fully in
    /// `cool_down_s` while not firing, and fires again once cool. Each shot strays by noise that
    /// grows with the heat (`shot_noise::ShotNoise::sample_rifle`), and recoils: the view kicks
    /// up by `recoil_up_rad` and sideways by `recoil_sideways_rad`, alternately left and right, and
    /// the rifle jolts back (`recoil_frac`, settling over `recoil_settle_s`); all from the settings.
    ///
    /// Example: `self.update_rifle(1.0 / 60.0);`
    fn update_rifle(&mut self, dt_s: f64) {
        let can_shoot: bool = self.can_shoot();
        let Some([sideways_rad, up_rad]) =
            self.weapons
                .update_rifle(&self.settings.rifle, can_shoot, dt_s)
        else {
            return;
        };
        self.has_shot_hit_wormholes = false;
        let (dx_px, dy_px) = (
            sideways_rad / voxel_world::LOOK_RAD_PER_PX,
            up_rad / voxel_world::LOOK_RAD_PER_PX,
        );
        match &mut self.voxel {
            Some(voxel) => voxel.player.look(dx_px, dy_px),
            None => physics::look_around(&mut self.player.frame, dx_px, dy_px),
        }
    }

    /// `update_disabling` follows this player holding `E` (`is_holding_e`) on one of the other
    /// team's consoles that is still on, within `CONSOLE_REACH_M` straight ahead of the eye (toward
    /// the crosshair's point in third person: `get_shot_eye`): the seconds held on that same
    /// console, or nothing. The server times it too, and turns the console off.
    ///
    /// Example: `self.update_disabling(true, 1.0 / 60.0);`
    fn update_disabling(&mut self, is_holding_e: bool, dt_s: f64) {
        let shot_eye: Option<DMat4> = is_holding_e.then(|| self.get_shot_eye());
        let (Some(voxel), Some(multiplayer)) = (&self.voxel, &mut self.multiplayer) else {
            return;
        };
        let game: net::GameState = multiplayer.client.game;
        let aimed: Option<u8> = match (multiplayer.client.team, shot_eye) {
            (Some(team), Some(eye)) if game.winner.is_none() => voxel
                .world
                .find_aimed_material(eye, self.settings.consoles.reach_m)
                .and_then(|m| voxel.world.consoles.iter().position(|c| c.material == m))
                .filter(|&i| voxel.world.consoles[i].team != team && game.off_mask >> i & 1 == 0)
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

    /// `get_console_states` gives the consoles turned off (bit i for console i), and how far along
    /// turning off each console is, from 0 to 1: the most of everyone holding `E` on it, with this
    /// player's own seconds held and the server's timing of the others. Without a server, every
    /// console is on and nobody turns any off.
    ///
    /// Example: `let (off_mask, disabling_fracs) = self.get_console_states();`
    fn get_console_states(&self) -> (u32, [f64; net::MAX_CONSOLE_COUNT]) {
        let mut disabling_fracs: [f64; net::MAX_CONSOLE_COUNT] = [0.0; net::MAX_CONSOLE_COUNT];
        let Some(multiplayer) = &self.multiplayer else {
            return (0, disabling_fracs);
        };
        let hold_s: f64 = self.settings.consoles.hold_to_disable_s;
        let own: Option<(u8, f64)> = multiplayer
            .disabling
            .map(|(console, held_s)| (console, (held_s / hold_s).min(1.0)));
        for (console, frac) in multiplayer
            .client
            .players
            .iter()
            .filter_map(|player| Some((player.disabling_console?, player.disabling_frac)))
            .chain(own)
        {
            let most_frac: &mut f64 = &mut disabling_fracs[usize::from(console)];
            *most_frac = most_frac.max(frac);
        }
        (multiplayer.client.game.off_mask, disabling_fracs)
    }

    /// `update_gun` counts the time since this player last held a movement key (moving without
    /// crouching on a voxel map, or jumping), dropping a shot in flight while one is held, and
    /// moves the gun: its step phase while walking or
    /// running on the ground (on a voxel map the head bob's, so the gun steps with the eye), how
    /// far into stepping it is (`step_frac`: easing in and out over the head bob's `fade_s`), and
    /// its pose toward jogging while running and back to aiming otherwise.
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
        let is_walking_upright: bool = is_moving && !(input.crouch && self.voxel.is_some());
        self.since_movement_key_s = if is_walking_upright || input.up {
            0.0
        } else {
            self.since_movement_key_s + dt_s
        };
        if !self.can_shoot() {
            self.weapons.shot_time_s = 0.0;
        }
        match &self.voxel {
            Some(voxel) => {
                (self.weapons.gun_phase_rad, self.weapons.step_frac) =
                    (voxel.player.bob_phase_rad, voxel.player.bob_frac);
            }
            None => {
                let is_stepping: bool = is_moving && is_on_ground;
                if is_stepping {
                    let hz: f64 = if is_running { GUN_JOG_HZ } else { GUN_WALK_HZ };
                    self.weapons.gun_phase_rad = (self.weapons.gun_phase_rad
                        + std::f64::consts::TAU * hz * dt_s)
                        % std::f64::consts::TAU;
                }
                let fade_frac: f64 = dt_s / self.player_settings.view.head_bob.fade_s;
                self.weapons.step_frac += (f64::from(u8::from(is_stepping))
                    - self.weapons.step_frac)
                    .clamp(-fade_frac, fade_frac);
            }
        }
        let target: f64 = f64::from(u8::from(is_running));
        self.weapons.run_pose_frac += (target - self.weapons.run_pose_frac)
            .clamp(-GUN_POSE_PER_S * dt_s, GUN_POSE_PER_S * dt_s);
    }

    /// `get_hit_effect` gives what the last hit on this player still does to the scene, fading over
    /// the player settings' `hit_effects.effect_s`: red edges, a blur from the edges inward, and a
    /// shake in a new direction each frame (the golden angle apart).
    ///
    /// Example: `let hit = self.get_hit_effect();`
    fn get_hit_effect(&self) -> render_metal::HitEffect {
        make_hit_effect(
            self.hit_effect_frac,
            self.total_frame_count,
            &self.player_settings.hit_effects,
        )
    }

    /// `get_team` gives this player's team (an index into `net::TEAM_NAMES`): the one the server
    /// put them on, or the first without a server or before joining.
    ///
    /// Example: `let team = self.get_team();`
    fn get_team(&self) -> u32 {
        self.multiplayer
            .as_ref()
            .and_then(|m| m.client.team)
            .unwrap_or(0)
    }

    /// `is_in_lobby` says whether this player is on a server and hasn't chosen a team yet.
    ///
    /// Example: `if self.is_in_lobby() { ... }`
    fn is_in_lobby(&self) -> bool {
        self.multiplayer
            .as_ref()
            .is_some_and(|m| m.client.is_in_lobby())
    }

    /// `get_wand` gives this player's wand (`make_wand`, with its tip in the team's colour), or
    /// `None` while text covers the view (`is_view_covered`) or this player's own body shows in
    /// third person (the last frame packed).
    ///
    /// Example: `let wand = self.get_wand();`
    fn get_wand(&self) -> Option<render_metal::OverlayWand> {
        let team: u32 = self.get_team();
        (!self.is_view_covered() && !self.is_own_body_shown).then(|| {
            make_wand(
                self.weapons.run_pose_frac,
                self.weapons.gun_phase_rad,
                self.weapons.recoil_frac,
                team,
                self.weapons.gun,
                get_sway(
                    self.weapons.gun.get_model(),
                    self.weapons.step_frac,
                    self.weapons.run_pose_frac,
                    self.weapons.gun_phase_rad,
                    &self.player_settings.view.head_bob,
                ),
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

    /// `update_multiplayer` reads the server's news: a welcome rebuilds a voxel map at the server's
    /// curvature if it differs from this window's and puts this player at its spawn point, and a
    /// hit is announced (and, if this player was hit, starts the respawn countdown).
    /// It returns whether this player is out of the game for now, and fails if the server can't be
    /// read.
    ///
    /// Example: `if session.update_multiplayer()? { return Ok(()); }`
    fn update_multiplayer(&mut self) -> Result<bool> {
        let Some(multiplayer) = &mut self.multiplayer else {
            return Ok(false);
        };
        let teams_before_poll: Vec<(u32, u32)> = multiplayer
            .client
            .players
            .iter()
            .map(|p| (p.id, p.team))
            .collect();
        let events: Vec<net::NetEvent> = multiplayer.client.poll()?;
        let mut messages: Vec<String> = Vec::new();
        for event in events {
            match event {
                net::NetEvent::Joined(id) => {
                    let others: Vec<DVec4> = get_eyes(&multiplayer.client.players);
                    let team: u32 = multiplayer.client.team.expect("a welcome brings the team");
                    let server_settings: ServerSettings = multiplayer
                        .client
                        .settings
                        .clone()
                        .expect("a welcome brings the server's settings");
                    if server_settings.space != self.settings.space
                        && let Some(voxel) = &mut self.voxel
                    {
                        let map: map::Map = load_map(self.map_name, &server_settings)?;
                        self.map_layout =
                            map_layout::MapLayout::FromMap(&map, get_map_picture_max_px())?;
                        let head_bob: settings::HeadBobSettings = voxel.world.head_bob.clone();
                        voxel.world = VoxelWorld::FromMap(&map)?;
                        voxel.world.head_bob = head_bob;
                        voxel.world.milky_way_brightness_frac =
                            self.player_settings.view.milky_way_brightness_frac;
                        self.world = World::Empty(map.get_curvature_radius_m());
                        messages.push(format!(
                            "The server curves space {} times as much as the map",
                            server_settings.space.curvature_per_map_curvature
                        ));
                    }
                    self.settings = server_settings;
                    match &mut self.voxel {
                        Some(voxel) => {
                            voxel.world.set_settings(self.settings.clone());
                            voxel.spawn_index = id as usize;
                            voxel.player = VoxelPlayer::NearSpawn(&voxel.world, team, id, &others);
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
                        self.weapons.is_laser_on = false;
                        self.weapons.shot_time_s = 0.0;
                        self.thrown.clear();
                    }
                    if !is_self
                        && let Some(&(_, team)) =
                            teams_before_poll.iter().find(|(id, _)| *id == victim_id)
                    {
                        let now_s: f64 = self.start.elapsed().as_secs_f64();
                        self.avatars.add_corpse(victim_id, team, now_s);
                    }
                    messages.push(if is_self {
                        format!("You died: hit by player {shooter_id}")
                    } else {
                        format!("Player {victim_id} was hit by player {shooter_id}")
                    });
                }
            }
        }
        let hit_frac: f64 = 1.0 / f64::from(self.settings.rifle.body_hits_to_eliminate);
        let response: HitResponse = self
            .hit_feedback
            .take_health(multiplayer.client.health_frac, hit_frac);
        if response.should_show {
            self.hit_effect_frac = 1.0;
        }
        if response.should_slow
            && let Some(voxel) = &mut self.voxel
        {
            voxel.player.slowed_s = self.settings.when_hit.slow_s;
        }
        if multiplayer.client.game.number != multiplayer.game_number {
            multiplayer.game_number = multiplayer.client.game.number;
            if let (Some(voxel), Some(id), Some(team)) = (
                &mut self.voxel,
                multiplayer.client.id,
                multiplayer.client.team,
            ) {
                let others: Vec<DVec4> = get_eyes(&multiplayer.client.players);
                voxel.player = VoxelPlayer::NearSpawn(&voxel.world, team, id, &others);
            }
            messages.push(format!(
                "Game {} started: every console is back on",
                multiplayer.game_number
            ));
        }
        let is_out: bool = multiplayer.is_eliminated || multiplayer.client.id.is_none();
        if !multiplayer.is_eliminated
            && let Err(error) = multiplayer.client.keep_joining()
        {
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
        let owned_markers: Vec<(Option<u32>, Marker)> = self.get_wormhole_markers();
        let markers: Vec<Marker> = owned_markers.iter().map(|&(_, marker)| marker).collect();
        let (off_mask, disabling_fracs) = self.get_console_states();
        let time_s: f64 = self.start.elapsed().as_secs_f64();
        if let Some(voxel) = &mut self.voxel {
            voxel
                .world
                .light_consoles(off_mask, &disabling_fracs, time_s);
            voxel.world.sky_time_s = spaceships::get_utc_s();
            voxel
                .black_hole_growth
                .update(&mut voxel.world, off_mask, time_s);
            let eye: DMat4 = voxel.player.get_camera_frame(&voxel.world);
            voxel.world.flashlight_eye = self.is_flashlight_on.then_some(eye);
        }
        if let Some(voxel) = &self.voxel {
            let eye: DMat4 = if self.route.is_empty() {
                voxel.player.get_camera_frame(&voxel.world)
            } else {
                self.player.frame
            };
            let third_person_view: Option<(third_person::ThirdPersonCamera, DMat4)> =
                self.get_third_person_view();
            let camera: DMat4 = third_person_view.map_or(eye, |(placed, _)| placed.frame);
            let laser_aim: Option<DMat4> =
                self.get_laser_aim(third_person_view.map_or(eye, |(_, shot_eye)| shot_eye));
            let team: u32 = self.get_team();
            let own_body: Option<(DMat4, usize)> = third_person_view
                .filter(|(placed, _)| placed.is_body_shown)
                .map(|_| multiplayer::get_body(&voxel.world, laser_aim.unwrap_or(eye), team));
            self.is_own_body_shown = own_body.is_some();
            let players: Vec<net::RemotePlayer> = self
                .multiplayer
                .as_ref()
                .map_or(Vec::new(), |m| m.client.players.clone());
            let (now_s, r) = (self.start.elapsed().as_secs_f64(), voxel.world.r);
            let mut eyes: Vec<(u32, DMat4)> = players.iter().map(|p| (p.id, p.frame)).collect();
            eyes.push((avatar_animation::OWN_TRACK_ID, laser_aim.unwrap_or(eye)));
            let mut poses: Vec<(avatar_model::AvatarClip, f64)> =
                self.avatars.update(&voxel.world, &eyes, now_s);
            poses.truncate(players.len() + usize::from(own_body.is_some()));
            let corpses: Vec<(DMat4, usize, (avatar_model::AvatarClip, f64))> = self
                .avatars
                .get_corpses(now_s, self.settings.respawn.delay_s)
                .into_iter()
                .map(|(corpse, pose)| (corpse.feet, corpse.team as usize, pose))
                .collect();
            self.tracers
                .retain(|tracer| tracer.get_lit_m(now_s).0 < tracer.length_m);
            self.dust_bursts.retain(|burst| !burst.is_over(now_s));
            let clouds: Vec<dust::DustCloud> = self
                .dust_bursts
                .iter()
                .filter_map(|burst| burst.get_cloud(now_s))
                .collect();
            let beams: Vec<render_voxel::LaserBeam> = self.get_lit_beams(r);
            let spot: Option<(glam::DVec4, u32)> = self
                .tracers
                .iter()
                .rev()
                .find(|tracer| tracer.get_lit_m(now_s).1 >= tracer.length_m)
                .and_then(|tracer| tracer.spot.map(|x| (x, tracer.team)));
            let view: multiplayer::SharedVoxelView = multiplayer::pack_shared_voxel_view(
                &voxel.world,
                camera,
                laser_aim,
                team,
                &players,
                &poses,
                &markers,
                own_body,
                &corpses,
                self.route.is_empty().then_some(multiplayer::Tracers {
                    beams: &beams,
                    spot,
                    dust: &clouds,
                }),
                render_px,
            );
            if is_firing
                && !self.was_firing
                && let Some(shot) = &view.shot
            {
                let is_rifle: bool = self.weapons.gun == Gun::Rifle;
                let speckle_frac: f64 = self.weapons.beam_speckle_frac;
                let tracer = Tracer {
                    is_own: true,
                    ..Tracer::New(shot, team, now_s, is_rifle, speckle_frac, &self.settings, r)
                };
                add_dust_burst(&mut self.dust_bursts, &voxel.world, shot, &tracer, is_rifle);
                self.tracers.push(tracer);
            }
            self.was_firing = is_firing;
            for (id, shot) in &view.other_shots {
                if !self.remote_firing_ids.contains(id) {
                    let shooter: Option<&net::RemotePlayer> = players.iter().find(|p| p.id == *id);
                    let (team, is_rifle, speckle_frac) = shooter.map_or((0, false, 0.0), |p| {
                        (p.team, p.is_rifle, p.beam_speckle_frac)
                    });
                    let tracer =
                        Tracer::New(shot, team, now_s, is_rifle, speckle_frac, &self.settings, r);
                    add_dust_burst(&mut self.dust_bursts, &voxel.world, shot, &tracer, is_rifle);
                    self.tracers.push(tracer);
                }
            }
            self.remote_firing_ids = view.other_shots.iter().map(|(id, _)| *id).collect();
            self.laser = multiplayer::describe_voxel_laser(&voxel.world, &view);
            let first: usize = voxel.world.static_warp_count;
            if let Some(victim_id) = view.hit_player_id
                && let Some(hits_to_kill) = take_hit_weight(
                    &self.settings.sniper,
                    &self.settings.rifle,
                    self.weapons.gun,
                    &mut self.weapons.has_rifle_shot_hit,
                    view.is_head_hit,
                )
                && let Some(multiplayer) = &mut self.multiplayer
                && let Err(error) = multiplayer.client.send_hit(victim_id, hits_to_kill)
            {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
            let entered: Vec<usize> = view.shot.as_ref().map_or(Vec::new(), |shot| {
                shot.through_warps
                    .iter()
                    .filter_map(|w| w.checked_sub(first).map(|k| k / 2))
                    .collect()
            });
            self.report_wormhole_hits(&entered, view.hit_marker, &owned_markers);
            return PackedFrame::Voxel(Box::new(view.frame));
        }
        let laser_aim: Option<DMat4> = self.get_laser_aim(self.player.frame);
        if let Some(multiplayer) = &mut self.multiplayer {
            let view: multiplayer::SharedView = multiplayer::pack_shared_view(
                &self.world,
                self.player.frame,
                laser_aim,
                &multiplayer.client.players,
                &markers,
                render_px,
                self.start.elapsed().as_secs_f64(),
            );
            self.laser = view.report.as_ref().map(describe_laser);
            if let Some(victim_id) = view.hit_player_id
                && let Some(hits_to_kill) = take_hit_weight(
                    &self.settings.sniper,
                    &self.settings.rifle,
                    self.weapons.gun,
                    &mut self.weapons.has_rifle_shot_hit,
                    false,
                )
                && let Err(error) = multiplayer.client.send_hit(victim_id, hits_to_kill)
            {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
            let first: usize = self.world.static_mouth_count;
            let entered: Vec<usize> = view.report.as_ref().map_or(Vec::new(), |report| {
                report
                    .mouths_entered
                    .iter()
                    .filter_map(|m| m.checked_sub(first).map(|k| k / 2))
                    .collect()
            });
            self.report_wormhole_hits(&entered, view.hit_marker, &owned_markers);
            return PackedFrame::House(Box::new(view.data));
        }
        let mut marked: World = self.world.clone();
        marked.add_markers(&markers);
        let (data, report) = pack_view(
            &marked,
            self.player.frame,
            laser_aim,
            render_px,
            self.start.elapsed().as_secs_f64(),
        );
        self.laser = report.as_ref().map(describe_laser);
        PackedFrame::House(Box::new(data))
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

    /// `get_connection_problem` gives why this window can't play on its server right now, if it
    /// can't: nothing from the server for `SERVER_SILENT_S` while not out after a hit (no server
    /// at that address and port), its snapshots of another version, or a full game while asking
    /// to join.
    ///
    /// Example: `if let Some(problem) = self.get_connection_problem() { ... }`
    fn get_connection_problem(&self) -> Option<ConnectionProblem> {
        let multiplayer: &Multiplayer = self.multiplayer.as_ref()?;
        let client: &net::Client = &multiplayer.client;
        if !multiplayer.is_eliminated && client.get_silent_s() > SERVER_SILENT_S {
            Some(ConnectionProblem::Silent)
        } else if client.is_other_version() {
            Some(ConnectionProblem::OtherVersion)
        } else if client.is_full() && client.is_waiting_for_welcome() {
            Some(ConnectionProblem::Full)
        } else {
            None
        }
    }

    /// `report_connection_problem` prints a connection problem's longer hint to the terminal
    /// once, when it starts.
    ///
    /// Example: `self.report_connection_problem();`
    fn report_connection_problem(&mut self) {
        let problem: Option<ConnectionProblem> = self.get_connection_problem();
        if problem != self.last_connection_problem
            && let (Some(problem), Some(multiplayer)) = (problem, &self.multiplayer)
        {
            eprintln!("{}", problem.get_hint(multiplayer.client.get_server()));
        }
        self.last_connection_problem = problem;
    }

    /// `get_banner_lines` gives the two lines shown over the view: while the exit prompt shows,
    /// its question and keys; when this window can't play on its server
    /// (`get_connection_problem`), what is wrong; in the lobby, whether it is connected to the
    /// server yet (`get_lobby_lines`); after asking to join, until welcomed, that
    /// it is joining and the server's address; once a team has won, the winner and how to start a
    /// new game; while this player is out after a hit, "YOU DIED" and the countdown to respawning;
    /// or `None`.
    ///
    /// Example: `if let Some(lines) = session.get_banner_lines() { ... }`
    fn get_banner_lines(&self) -> Option<[String; 2]> {
        if self.exit_prompt.is_shown() {
            return Some(exit_prompt::get_lines());
        }
        let multiplayer: &Multiplayer = self.multiplayer.as_ref()?;
        let client: &net::Client = &multiplayer.client;
        if let Some(problem) = self.get_connection_problem() {
            return Some(problem.get_lines(client.get_server()));
        }
        if multiplayer.client.is_in_lobby() {
            let keys = &self.player_settings.keys;
            return Some(get_lobby_lines(
                client.lobby_counts,
                client.get_server(),
                format!(
                    "{} joins blue, {} joins red",
                    keys.get_label(Action::JoinBlue),
                    keys.get_label(Action::JoinRed)
                ),
            ));
        }
        if client.is_waiting_for_welcome() && !multiplayer.is_eliminated {
            return Some([
                "JOINING THE GAME".to_string(),
                format!("server {}", client.get_server()),
            ]);
        }
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
        Some(get_death_lines(multiplayer.respawn_in_s))
    }

    /// `get_zoom` gives how much closer the world looks now: the gun in hand's full zoom
    /// (`aim` in the server settings), eased in by how far this player has aimed (`get_aim_zoom`).
    ///
    /// Example: `let zoom = self.get_zoom();`
    fn get_zoom(&self) -> f64 {
        self.weapons.get_zoom(&self.settings.aim)
    }

    /// `get_lit_beams` gives the lit part of each laser shot now (`Tracer::get_lit_m`), with the
    /// shooter's team and gun and the shot's speckle share, on a map of curvature radius `r`. While the trail of one of this player's own shots still reaches the
    /// muzzle, its beam starts at the gun's tip as drawn now (`get_muzzle`), so it stays on the tip
    /// as the gun moves.
    ///
    /// Example: `let beams = self.get_lit_beams(voxel.world.r);`
    fn get_lit_beams(&self, r: f64) -> Vec<render_voxel::LaserBeam> {
        let now_s: f64 = self.start.elapsed().as_secs_f64();
        let muzzle: Option<glam::DVec4> = self.get_muzzle(r);
        self.tracers
            .iter()
            .map(|tracer| {
                let (tail_m, head_m) = tracer.get_lit_m(now_s);
                let mut lit: Vec<voxel_world::BeamSegment> =
                    voxel_world::clip_beam(&tracer.segments, r, tail_m, head_m);
                if tracer.is_own
                    && tail_m <= 0.0
                    && let (Some(muzzle), Some(first)) = (muzzle, lit.first_mut())
                    && let Some(joined) = voxel_world::BeamSegment::Between(
                        muzzle,
                        voxel_world::geodesic_point(first.a, first.b, first.length),
                    )
                {
                    *first = joined;
                }
                render_voxel::LaserBeam {
                    segments: lit,
                    team: tracer.team,
                    is_rifle: tracer.is_rifle,
                    speckle_frac: tracer.speckle_frac,
                }
            })
            .collect()
    }

    /// `get_muzzle` gives where the tip of the gun in hand shows in the world, on a map of
    /// curvature radius `r`: the wand's tip in camera space, moved
    /// toward the view's centre by the aim zoom (which magnifies the scene but not the gun), or
    /// `None` with no gun in view.
    ///
    /// Example: `let muzzle = self.get_muzzle(voxel.world.r);`
    fn get_muzzle(&self, r: f64) -> Option<glam::DVec4> {
        let tip_m: glam::DVec3 = self.get_wand()?.get_tip_m().as_dvec3();
        let zoom: f64 = self.get_zoom();
        Some(
            self.get_eye_frame()
                * voxel_world::map_exp(
                    glam::DVec3::new(tip_m.x / zoom, tip_m.y / zoom, tip_m.z),
                    r,
                ),
        )
    }

    /// `make_overlay_minimap` gives the minimap for a window of `window_px` (`minimap.rs`): around
    /// this player, with the other players and the shots, or `None` when the player settings hide
    /// it or text covers the view.
    ///
    /// Example: `let minimap = session.make_overlay_minimap((2560, 1600));`
    fn make_overlay_minimap(
        &self,
        window_px: (u32, u32),
    ) -> Option<render_metal::OverlayMinimap<'_>> {
        if !self.player_settings.minimap.is_shown || self.is_view_covered() {
            return None;
        }
        let (feet, beams): (DMat4, Vec<(Vec<voxel_world::BeamSegment>, u32)>) = match &self.voxel {
            Some(voxel) => (
                voxel.player.get_feet_frame(&voxel.world),
                self.get_lit_beams(voxel.world.r)
                    .into_iter()
                    .map(|beam| (beam.segments, beam.team))
                    .collect(),
            ),
            None => (self.player.frame, Vec::new()),
        };
        let players: Vec<(DVec4, u32)> = match &self.multiplayer {
            Some(multiplayer) => multiplayer
                .client
                .players
                .iter()
                .map(|player| (player.frame.col(AXIS_W), player.team))
                .collect(),
            None => Vec::new(),
        };
        let size_px: (u32, u32) = self.map_layout.get_size_px();
        Some(render_metal::OverlayMinimap {
            map_pixels: self.map_layout.get_plain_pixels(),
            map_size_px: (size_px.0 as usize, size_px.1 as usize),
            minimap: minimap::make_minimap(
                &self.map_layout,
                &self.player_settings.minimap,
                feet,
                &players,
                &beams,
                window_px,
            ),
        })
    }

    /// `get_edge_stretch_frac` gives how far this player's view stretches at the map's soft edge:
    /// the square of how far into it they stand (`VoxelWorld::get_edge_frac`), so it starts gently
    /// and builds near the stop, times their `view.edge_stretch_frac`; 0 off a voxel map.
    ///
    /// Example: `let stretch_frac = session.get_edge_stretch_frac();`
    fn get_edge_stretch_frac(&self) -> f32 {
        match &self.voxel {
            Some(voxel) => {
                let feet: DVec4 = voxel.player.get_feet_frame(&voxel.world).col(AXIS_W);
                (voxel.world.get_edge_frac(feet).powi(2)
                    * self.player_settings.view.edge_stretch_frac) as f32
            }
            None => 0.0,
        }
    }

    /// `is_view_covered` says whether text covers the view: the menu or a banner.
    ///
    /// Example: `if !self.is_view_covered() { ... }`
    fn is_view_covered(&self) -> bool {
        self.menu.is_some() || self.get_banner_lines().is_some()
    }

    /// `get_overlay_text` gives the text shown over the dimmed view: the menu while it is open,
    /// else the banner lines (`get_banner_lines`), or `None`.
    ///
    /// Example: `if let Some(text) = session.get_overlay_text() { ... }`
    fn get_overlay_text(&self) -> Option<OverlayText> {
        if let Some(menu) = &self.menu {
            let save_path: String = match &self.player_settings_path {
                Some(path) => path.display().to_string(),
                None => "nowhere: HOME isn't set, pass --player-settings".to_string(),
            };
            let scoreboard: menu::Scoreboard = match &self.multiplayer {
                None => menu::Scoreboard::NoServer,
                Some(Multiplayer { client, .. }) => match &client.scores {
                    None => menu::Scoreboard::Waiting,
                    Some(scores) => menu::Scoreboard::Scores {
                        scores,
                        own_id: client.welcomed_id,
                    },
                },
            };
            return Some(OverlayText {
                lines: menu.get_lines(&self.player_settings, &save_path, scoreboard),
                mask_px: MENU_MASK_PX,
                height_frac: MENU_HEIGHT_FRAC,
                is_centered: false,
                dim_frac: MENU_DIM_FRAC,
                picture: (menu.get_page() == menu::Page::Map).then(|| self.make_map_picture()),
            });
        }
        let [title, detail] = self.get_banner_lines()?;
        Some(OverlayText {
            lines: vec![(title, true), (detail, false)],
            mask_px: DEATH_MASK_PX,
            height_frac: BANNER_HEIGHT_FRAC,
            is_centered: true,
            dim_frac: DEATH_DIM_FRAC,
            picture: None,
        })
    }

    /// `make_map_picture` gives the map page's picture: the whole map from above
    /// (`map_layout.rs`) with this player on it and, in a game on a server, every other player in
    /// the game, centred in the page's area for it.
    ///
    /// Example: `let picture = self.make_map_picture();`
    fn make_map_picture(&self) -> Picture {
        let feet: glam::DMat4 = match &self.voxel {
            Some(voxel) => voxel.player.get_feet_frame(&voxel.world),
            None => self.player.frame,
        };
        let others: Vec<(DVec4, u32)> = match &self.multiplayer {
            Some(multiplayer) => get_points_and_teams(&multiplayer.client.players),
            None => Vec::new(),
        };
        let size_px: (u32, u32) = self.map_layout.get_size_px();
        let size_mask_px: [i32; 2] =
            [size_px.0 as i32, size_px.1 as i32].map(|n| n / PICTURE_PX_PER_MASK_PX);
        let min_mask_px: [i32; 2] = [0, 1].map(|a| {
            MAP_AREA_MIN_MASK_PX[a]
                + (MAP_AREA_MAX_EXCL_MASK_PX[a] - MAP_AREA_MIN_MASK_PX[a] - size_mask_px[a]) / 2
        });
        Picture {
            pixels: self.map_layout.draw_players(
                feet,
                self.get_team(),
                self.player_settings.minimap.range_m,
                &others,
            ),
            size_px,
            min_mask_px,
            max_excl_mask_px: [0, 1].map(|a| min_mask_px[a] + size_mask_px[a]),
        }
    }

    /// `get_overlay_rects` gives the HUD's rectangles drawn over the view: the crosshair (unless
    /// text covers the view) and, in the bottom left, as segmented bars in the HUD's look
    /// (`hud_bars.rs`): this player's health in a game on a server (one segment per rifle body
    /// hit), the gun in hand's ammo (the rifle's shots left before it overheats, or the sniper's
    /// one block, empty while it reloads; no pulses), and on a voxel map the sprint energy. Health
    /// and energy carry pulses that run over their filled part while they change
    /// (`update_bar_pulses`). Below the middle, the progress of turning off a console (while
    /// holding `E` on one).
    ///
    /// Example: `let rects = session.get_overlay_rects();`
    fn get_overlay_rects(&self) -> Vec<render_metal::OverlayRect> {
        let mut rects: Vec<render_metal::OverlayRect> = Vec::new();
        if !self.is_view_covered() {
            rects.extend(get_crosshair_rects(self.last_output_px));
        }
        rects.extend(make_bar_rects(
            &self.get_hud_bars(),
            &self.weapons,
            &self.settings.rifle,
            &self.hud_pulses,
        ));
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
        let gun: String = describe_gun(&self.weapons);
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
        if let Some(voxel) = self
            .voxel
            .as_ref()
            .filter(|voxel| voxel.world.can_grow_black_hole())
        {
            let strength_pct: f64 = voxel.black_hole_growth.strength_frac * PERCENT_PER_FRAC;
            let rate_pct: f64 = self.get_time_rate_frac() * PERCENT_PER_FRAC;
            lines.push(format!(
                "BLACK HOLE {strength_pct:.0}%, your time runs at {rate_pct:.0}%"
            ));
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
            self.player_settings.keys.describe()
        } else {
            "Click to capture the mouse; Esc to exit".to_string()
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
        Some(metal) => play_on_layer(
            &sdl,
            &video,
            window_size,
            &mut session,
            metal,
            arguments.no_vsync,
        ),
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

/// `get_points_and_teams` gives where other players are (their eyes) and their teams (indices
/// into `net::TEAM_NAMES`), for the map page.
///
/// Example: `let others = get_points_and_teams(&client.players);`
fn get_points_and_teams(players: &[net::RemotePlayer]) -> Vec<(DVec4, u32)> {
    players
        .iter()
        .map(|player| (player.frame.col(AXIS_W), player.team))
        .collect()
}

/// `ConnectionProblem` is why a window can't play on its server: nothing comes from it (no server
/// at that address and port, or it stopped), it runs another version of the game, or its game is
/// full.
#[derive(Clone, Copy, PartialEq, Debug)]
enum ConnectionProblem {
    Silent,
    OtherVersion,
    Full,
}

impl ConnectionProblem {
    /// `get_lines` gives the banner's two lines for this problem with the server at `server`
    /// (short enough for the banner).
    ///
    /// Example: `let lines = ConnectionProblem::Silent.get_lines(server);`
    fn get_lines(self, server: std::net::SocketAddr) -> [String; 2] {
        match self {
            ConnectionProblem::Silent => [
                "NO ANSWER FROM THE SERVER".to_string(),
                format!("at {server}"),
            ],
            ConnectionProblem::OtherVersion => [
                "SERVER RUNS ANOTHER VERSION".to_string(),
                "Rebuild both from one commit".to_string(),
            ],
            ConnectionProblem::Full => [
                "THE GAME IS FULL".to_string(),
                format!("{} players already", net::MAX_PLAYER_COUNT),
            ],
        }
    }

    /// `get_hint` gives the longer advice for this problem with the server at `server`, for the
    /// terminal.
    ///
    /// Example: `eprintln!("{}", ConnectionProblem::Silent.get_hint(server));`
    fn get_hint(self, server: std::net::SocketAddr) -> String {
        match self {
            ConnectionProblem::Silent => format!(
                "No answer from a game server at {server} for {SERVER_SILENT_S} s. Is it running, on that address and port? The server prints the address and port to join (its own port, not the port of a window that joined it). Start one with `make serve-game`, then `make join-game SERVER=<address> PORT=<port>`."
            ),
            ConnectionProblem::OtherVersion => format!(
                "The game server at {server} runs another version of the game: its snapshots don't fit this one. Rebuild the server and this game from the same commit (pull, then restart `make serve-game`)."
            ),
            ConnectionProblem::Full => format!(
                "The game server at {server} already has {} players, so it can't take more until one leaves.",
                net::MAX_PLAYER_COUNT
            ),
        }
    }
}

/// `OverlayText` is text shown over the dimmed view: its lines, each with whether it is
/// highlighted, the size of the mask it is drawn into (`make_text_mask`), how much of the window's
/// height that mask may take, whether its lines are centred (a banner) or start at the left (the
/// menu), how much the view behind it is darkened, and a picture under it.
struct OverlayText {
    lines: Vec<(String, bool)>,
    mask_px: (u32, u32),
    height_frac: f32,
    is_centered: bool,
    dim_frac: f32,
    picture: Option<Picture>,
}

/// `Picture` is an RGBA picture (rows top first) of `size_px` under overlay text, stretched
/// between two corners in the text mask's pixels.
struct Picture {
    pixels: Vec<u8>,
    size_px: (u32, u32),
    min_mask_px: [i32; 2],
    max_excl_mask_px: [i32; 2],
}

/// `get_map_picture_max_px` gives the largest map picture the map page holds, in pixels.
///
/// Example: `let layout = MapLayout::FromMap(&map, get_map_picture_max_px())?;`
fn get_map_picture_max_px() -> (u32, u32) {
    let [width, height] = [0, 1].map(|a| {
        ((MAP_AREA_MAX_EXCL_MASK_PX[a] - MAP_AREA_MIN_MASK_PX[a]) * PICTURE_PX_PER_MASK_PX) as u32
    });
    (width, height)
}

/// `get_text_positions` places each of `text`'s lines in its mask: the line cut to the mask's
/// width, and where its top left goes.
///
/// Example: `for (line, (x, y), is_highlighted) in get_text_positions(&text) { ... }`
fn get_text_positions(text: &OverlayText) -> Vec<(String, (f32, f32), bool)> {
    let (width, height) = text.mask_px;
    let bottom_px: f32 =
        TEXT_TOP_PX + TEXT_LINE_PX * text.lines.len().saturating_sub(1) as f32 + DEBUG_GLYPH_PX;
    assert!(
        bottom_px <= height as f32,
        "get_text_positions: {} lines don't fit a mask {height} px high",
        text.lines.len()
    );
    let max_char_count: usize = (width as f32 / DEBUG_GLYPH_PX) as usize;
    text.lines
        .iter()
        .enumerate()
        .map(|(i, (line, is_highlighted))| {
            let shown: String = line.chars().take(max_char_count).collect();
            let x: f32 = if text.is_centered {
                (width as f32 - DEBUG_GLYPH_PX * shown.chars().count() as f32) / 2.0
            } else {
                MENU_LEFT_PX
            };
            (
                shown,
                (x, TEXT_TOP_PX + TEXT_LINE_PX * i as f32),
                *is_highlighted,
            )
        })
        .collect()
}

/// `make_text_mask` draws `text` with SDL's built-in text into a mask of `text.mask_px`, one byte
/// a pixel: `DEATH_TITLE_MASK` on highlighted lines' text, `DEATH_COUNTDOWN_MASK` on the others',
/// 0 elsewhere, for the Metal overlay.
///
/// Example: `let mask = make_text_mask(&text)?;`
fn make_text_mask(text: &OverlayText) -> Result<Vec<u8>> {
    let (width, height) = text.mask_px;
    let surface = sdl3::surface::Surface::new(width, height, PixelFormat::RGBA32)?;
    let mut canvas = surface.into_canvas()?;
    for (line, position, is_highlighted) in get_text_positions(text) {
        canvas.set_draw_color(if is_highlighted {
            Color::RGB(255, 0, 0)
        } else {
            Color::RGB(0, 255, 0)
        });
        canvas.draw_debug_text(line.as_str(), position)?;
    }
    canvas.present();
    let surface = canvas.into_surface();
    let pitch: usize = surface.pitch() as usize;
    Ok(surface.with_lock(|pixels: &[u8]| {
        (0..height as usize)
            .flat_map(|y| (0..width as usize).map(move |x| y * pitch + x * BYTES_PER_PIXEL))
            .map(|offset| match (pixels[offset], pixels[offset + 1]) {
                (0, 0) => 0,
                (0, _) => DEATH_COUNTDOWN_MASK,
                _ => DEATH_TITLE_MASK,
            })
            .collect()
    }))
}

/// `draw_overlay_rects` draws the HUD's rectangles (`Session::get_overlay_rects`) on the canvas,
/// for the SDL-renderer window: flat boxes in their colours, without the Metal overlay's style
/// (slant, outline, gloss, glow) or pulses.
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

/// `draw_text_overlay` dims the canvas and draws `text` in its middle, scaled like the Metal
/// overlay's mask (`text.height_frac` of the height, at most `OVERLAY_MAX_WIDTH_FRAC` of the
/// width), for the SDL-renderer window.
///
/// Example: `draw_text_overlay(&mut canvas, &text)?;`
fn draw_text_overlay(canvas: &mut sdl3::render::WindowCanvas, text: &OverlayText) -> Result<()> {
    let (width, height) = canvas.window().size_in_pixels();
    let (mask_width, mask_height) = text.mask_px;
    let scale: f32 = ((height as f32 * text.height_frac) as u32 / mask_height)
        .min((f64::from(width) * render_metal::OVERLAY_MAX_WIDTH_FRAC) as u32 / mask_width)
        .max(1) as f32;
    canvas.set_scale(1.0, 1.0)?;
    canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
    canvas.set_draw_color(Color::RGBA(0, 0, 0, (text.dim_frac * 255.0) as u8));
    canvas.fill_rect(None)?;
    canvas.set_scale(scale, scale)?;
    let left: f32 = (width as f32 / scale - mask_width as f32) / 2.0;
    let top: f32 = (height as f32 / scale - mask_height as f32) / 2.0;
    if let Some(picture) = &text.picture {
        let creator = canvas.texture_creator();
        let mut texture = creator.create_texture_static(
            PixelFormat::RGBA32,
            picture.size_px.0,
            picture.size_px.1,
        )?;
        texture.update(
            None,
            &picture.pixels,
            picture.size_px.0 as usize * BYTES_PER_PIXEL,
        )?;
        let [min_x, min_y] = picture.min_mask_px.map(|n| n as f32);
        let [max_x, max_y] = picture.max_excl_mask_px.map(|n| n as f32);
        canvas.copy(
            &texture,
            None,
            sdl3::render::FRect::new(left + min_x, top + min_y, max_x - min_x, max_y - min_y),
        )?;
    }
    for (line, (x, y), is_highlighted) in get_text_positions(text) {
        canvas.set_draw_color(if is_highlighted {
            Color::RGB(255, 90, 80)
        } else {
            HUD_COLOR
        });
        canvas.draw_debug_text(line.as_str(), (left + x, top + y))?;
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
        session.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;
        let render_px: (u32, u32) = session.get_render_size(canvas.window().size_in_pixels());
        pixels.resize(
            render_px.0 as usize * render_px.1 as usize * BYTES_PER_PIXEL,
            0,
        );
        let render_start: Instant = Instant::now();
        let mut packed: PackedFrame = session.pack(render_px);
        packed.zoom_in(session.get_zoom());
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
        if let Some(text) = session.get_overlay_text() {
            draw_text_overlay(&mut canvas, &text)?;
        }
        canvas.present();
    }
}

/// `draw_loading_screen` draws the loading screen on a window's layer: black, the banner
/// `LOADING_TITLE` over `label` (what is loading), and a bar in the pylon-disabling bar's place and
/// look, filled to `done_frac`.
///
/// Example: `draw_loading_screen(metal, layer, 0.5, "revolver")?;`
fn draw_loading_screen(
    metal: &mut render_metal::MetalRenderer,
    layer: &CAMetalLayer,
    done_frac: f32,
    label: &str,
) -> Result<()> {
    let text: OverlayText = OverlayText {
        lines: vec![
            (LOADING_TITLE.to_string(), true),
            (label.to_string(), false),
        ],
        mask_px: DEATH_MASK_PX,
        height_frac: BANNER_HEIGHT_FRAC,
        is_centered: true,
        dim_frac: LOADING_DIM_FRAC,
        picture: None,
    };
    let mask: Vec<u8> = make_text_mask(&text)?;
    let rects: Vec<render_metal::OverlayRect> = hud_bars::make_progress_bar(
        DISABLE_BAR_MIN_FRAC,
        DISABLE_BAR_SIZE_FRAC,
        done_frac,
        DISABLE_BAR_RGB,
    );
    metal.render_overlay_to_layer(
        layer,
        &render_metal::Overlay {
            dim_frac: text.dim_frac,
            mask: Some((&mask, (text.mask_px.0 as usize, text.mask_px.1 as usize))),
            rects: &rects,
            wand: None,
            blur_frac: 0.0,
            hit: render_metal::HitEffect::default(),
            mask_height_frac: text.height_frac,
            picture: None,
            stretch_frac: 0.0,
            minimap: None,
            blueshift_frac: 0.0,
        },
    )?;
    Ok(())
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
    is_vsync_off: bool,
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
    layer.setDisplaySyncEnabled(!is_vsync_off);
    let mouse: sdl3::mouse::MouseUtil = sdl.mouse();
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let loading_steps: &[render_metal::LoadingStep] =
        render_metal::get_loading_steps(session.voxel.is_some());
    let mut loading_step_index: usize = 0;
    let result: Result<()> = (|| loop {
        for event in event_pump.poll_iter() {
            if !session.handle_event(event, &window, &mouse) {
                return Ok(());
            }
        }
        if session.is_finished() {
            return Ok(());
        }
        session.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;
        let window_px: (u32, u32) = window.size_in_pixels();
        layer.setDrawableSize(CGSize {
            width: f64::from(window_px.0),
            height: f64::from(window_px.1),
        });
        if loading_step_index <= loading_steps.len() {
            let step: Option<render_metal::LoadingStep> =
                loading_steps.get(loading_step_index).copied();
            let done_frac: f32 = loading_step_index as f32 / loading_steps.len() as f32;
            let label: &str = step.map_or(LOADING_DONE_LABEL, render_metal::LoadingStep::get_label);
            draw_loading_screen(metal, layer, done_frac, label)?;
            if let Some(step) = step {
                metal.load(step)?;
            }
            loading_step_index += 1;
            continue;
        }
        let render_px: (u32, u32) = session.get_render_size(window_px);
        let render_start: Instant = Instant::now();
        let mut packed: PackedFrame = session.pack(render_px);
        packed.zoom_in(session.get_zoom());
        let overlay_text: Option<OverlayText> = session.get_overlay_text();
        let text_mask: Option<Vec<u8>> = overlay_text.as_ref().map(make_text_mask).transpose()?;
        let rects: Vec<render_metal::OverlayRect> = session.get_overlay_rects();
        let wand: Option<(render_metal::OverlayWand, viewmodel::GunModel)> = session
            .get_wand()
            .map(|wand| (wand, session.weapons.gun.get_model()));
        let stretch_frac: f32 = session.get_edge_stretch_frac();
        let minimap: Option<render_metal::OverlayMinimap> = session.make_overlay_minimap(window_px);
        let blueshift_frac: f32 = session.blueshift_frac;
        let overlay: Option<render_metal::Overlay> = (text_mask.is_some()
            || !rects.is_empty()
            || wand.is_some()
            || stretch_frac > 0.0
            || blueshift_frac != 0.0
            || minimap.is_some())
        .then(|| render_metal::Overlay {
            dim_frac: match &overlay_text {
                Some(text) => text.dim_frac,
                None => 0.0,
            },
            mask: text_mask
                .as_deref()
                .zip(overlay_text.as_ref())
                .map(|(pixels, text)| (pixels, (text.mask_px.0 as usize, text.mask_px.1 as usize))),
            mask_height_frac: match &overlay_text {
                Some(text) => text.height_frac,
                None => 0.0,
            },
            picture: overlay_text
                .as_ref()
                .and_then(|text| text.picture.as_ref())
                .map(|picture| render_metal::OverlayPicture {
                    pixels: &picture.pixels,
                    size_px: (picture.size_px.0 as usize, picture.size_px.1 as usize),
                    min_mask_px: picture.min_mask_px,
                    max_excl_mask_px: picture.max_excl_mask_px,
                }),
            stretch_frac,
            minimap,
            rects: &rects,
            wand,
            blur_frac: session.motion_blur_frac,
            hit: session.get_hit_effect(),
            blueshift_frac,
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
    //! Tests of the guns' hit rules and models in hand, aiming's zoom, where overlay text goes and
    //! the tint after a wormhole.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Depth in a mouth | at the throat (1), part way, outside (0) | `wormhole_shift_follows_depth_and_speed` |
    //! | Speed along the view | forward (blueshift), backward (redshift), still (none), past the cap (held at it) | `wormhole_shift_follows_depth_and_speed` |
    //! | Tint strength setting | 1, between 0 and 1, 0 | `wormhole_shift_follows_depth_and_speed` |
    //! | Gun in hand | laser (the bolt-action rifle), rifle (the revolver) | `each_gun_holds_its_model` |
    //! | Map kind loading | voxel map, house map: each loads every gun model | `each_gun_holds_its_model` |
    //! | Overlay text | a centred two-line banner, the exit prompt's banner uncut, the lobby's banner connecting to the longest address and connected with full teams, the left-aligned menu with a line too long for its mask, the scores page with more players than it shows and five-digit kills and deaths | `overlay_text_fits_its_mask` |

    use super::*;

    #[test]
    fn wormhole_shift_follows_depth_and_speed() {
        const WALK_M_PER_S: f64 = 10.0;
        const SPRINT_M_PER_S: f64 = 18.0;
        const HALF_FRAC: f64 = 0.5;
        const WEAK_STRENGTH_FRAC: f64 = 0.4;
        let shift = |depth_frac: f64, view_m_per_s: f64, strength_frac: f64| -> f64 {
            f64::from(get_blueshift_frac(
                depth_frac,
                view_m_per_s,
                WALK_M_PER_S,
                strength_frac,
            ))
        };
        assert_eq!(shift(1.0, WALK_M_PER_S, 1.0), 1.0);
        assert_eq!(shift(1.0, -WALK_M_PER_S, 1.0), -1.0);
        assert_eq!(shift(1.0, 0.0, 1.0), 0.0);
        assert_eq!(shift(0.0, WALK_M_PER_S, 1.0), 0.0);
        assert_eq!(shift(1.0, WALK_M_PER_S, 0.0), 0.0);
        let part_way: f64 = shift(HALF_FRAC, -WALK_M_PER_S, WEAK_STRENGTH_FRAC);
        assert!((part_way + HALF_FRAC * WEAK_STRENGTH_FRAC).abs() < 1e-6);
        assert!((shift(1.0, SPRINT_M_PER_S, 1.0) - MAX_BLUESHIFT_FRAC).abs() < 1e-6);
        assert!((shift(1.0, -SPRINT_M_PER_S, 1.0) + MAX_BLUESHIFT_FRAC).abs() < 1e-6);
    }

    #[test]
    fn each_gun_holds_its_model() {
        assert_eq!(Gun::Laser.get_model(), viewmodel::GunModel::BoltActionRifle);
        assert_eq!(Gun::Rifle.get_model(), viewmodel::GunModel::Revolver);
        for is_voxel_map in [true, false] {
            let steps: &[render_metal::LoadingStep] = render_metal::get_loading_steps(is_voxel_map);
            for gun in [Gun::Laser, Gun::Rifle] {
                let step: render_metal::LoadingStep =
                    render_metal::LoadingStep::Gun(gun.get_model());
                assert_eq!(steps.iter().filter(|s| **s == step).count(), 1);
            }
        }
    }

    #[test]
    fn overlay_text_fits_its_mask() {
        let banner: OverlayText = OverlayText {
            lines: vec![
                ("YOU DIED".to_string(), true),
                ("Respawning in 3".to_string(), false),
            ],
            mask_px: DEATH_MASK_PX,
            height_frac: BANNER_HEIGHT_FRAC,
            is_centered: true,
            dim_frac: DEATH_DIM_FRAC,
            picture: None,
        };
        let placed = get_text_positions(&banner);
        let title_px: f32 = DEBUG_GLYPH_PX * "YOU DIED".len() as f32;
        assert_eq!(placed[0].1.0 * 2.0 + title_px, DEATH_MASK_PX.0 as f32);
        assert!(placed[0].2 && !placed[1].2);
        let long_path: String = "/very/long".repeat(20);
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        let scores_menu: menu::Menu = menu::Menu::New();
        let mut settings_menu: menu::Menu = menu::Menu::New();
        settings_menu.handle(Control::Key(sdl3::keyboard::Scancode::E), &mut mine);
        let mut credits_menu: menu::Menu = menu::Menu::New();
        credits_menu.handle(Control::Key(sdl3::keyboard::Scancode::Q), &mut mine);
        let scores: Vec<net::Score> = (0..3 * net::MAX_PLAYER_COUNT as u32)
            .map(|id| net::Score {
                id: u32::MAX - id,
                team: id % net::TEAM_NAMES.len() as u32,
                kills: u32::from(u16::MAX),
                deaths: u32::from(u16::MAX),
            })
            .collect();
        let scoreboard = menu::Scoreboard::Scores {
            scores: &scores,
            own_id: Some(u32::MAX),
        };
        let max_char_count: usize = (MENU_MASK_PX.0 as f32 / DEBUG_GLYPH_PX) as usize;
        for line in scores_menu.get_lines(&mine, &long_path, scoreboard) {
            assert!(line.0.chars().count() <= max_char_count, "{}", line.0);
        }
        for menu in [settings_menu, scores_menu, credits_menu] {
            let menu_text: OverlayText = OverlayText {
                lines: menu.get_lines(&mine, &long_path, scoreboard),
                mask_px: MENU_MASK_PX,
                height_frac: MENU_HEIGHT_FRAC,
                is_centered: false,
                dim_frac: MENU_DIM_FRAC,
                picture: None,
            };
            for (line, (x, _), _) in get_text_positions(&menu_text) {
                assert_eq!(x, MENU_LEFT_PX);
                assert!(line.chars().count() <= max_char_count, "{line}");
            }
        }
        let banner_char_count: usize = (DEATH_MASK_PX.0 as f32 / DEBUG_GLYPH_PX) as usize;
        for line in exit_prompt::get_lines() {
            assert!(line.chars().count() <= banner_char_count, "{line}");
        }
        let longest_server: SocketAddr = "255.255.255.255:65535".parse().expect("an address");
        let full_counts: [u32; 2] = [net::MAX_PLAYER_COUNT as u32; 2];
        let keys_line: String = "1 joins blue, 2 joins red".to_string();
        let connecting = get_lobby_lines(None, longest_server, keys_line.clone());
        let connected = get_lobby_lines(Some(full_counts), longest_server, keys_line);
        assert!(connecting[0].starts_with("CONNECTING") && connected[0].starts_with("CONNECTED "));
        for line in connecting.iter().chain(&connected) {
            assert!(line.chars().count() <= banner_char_count, "{line}");
        }
    }
}
