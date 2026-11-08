//! `hyperbolic-wormholes` is the Rust port of the WebGL hyperbolic wormholes demo
//! (`../hyperbolic_wormholes/`), to see how fast the same ray tracer runs natively. You walk in
//! hyperbolic space (H³) with gravity, fly, go through three wormholes and fire a laser.
//!
//! Interface:
//! - `--renderer metal`: which renderer draws the frames: the GPU with Metal (the only one).
//! - `--render-scale <fraction>`: fraction of the window's pixels (per side) to render.
//! - `--bench-views`: render fixed views without a window, time them, and save them under
//!   `.artifacts/bench_views/<run_id>/`. `--frame-count`, `--width-px`, `--height-px` set it up.
//! - `--json`: print results as one JSON object. `--label`: suffix for the run folder.
//! - `--exit-after-s <seconds>`: close the window after that long and print its frame rate.
//! - `--host`, `--join auto|<address>`, `--serve`, `--bot`, `--port`: play with others on the local
//!   network (see `net.rs` and `multiplayer.rs`).
//! - `--black-hole-strength <0..1>`: on lunar-surface, force the black hole's strength (W-68).
//!
//! Example: `cargo run --release -- --bench-views --width-px 2560 --height-px 1600`
//!
//! Must know:
//! - Gotchas: run folders are found from the crate folder, not the current directory.

mod app_bundle;
mod avatar_animation;
mod avatar_model;
mod bench;
mod black_hole_growth;
mod credits;
mod dust;
mod exit_prompt;
mod fps_demo_assets;
mod geometry;
mod hud_bars;
mod key_bindings;
mod map;
mod map_layout;
mod menu;
mod mesh;
mod minimap;
mod multiplayer;
mod net;
mod physics;
mod play;
mod render_mesh;
mod render_metal;
mod render_voxel;
mod run_folder;
mod session;
mod settings;
mod shapes;
mod shot_noise;
mod spaceships;
mod terrain;
mod third_person;
mod thrown_wormholes;
mod title;
mod viewmodel;
mod voxel_clearance;
mod voxel_textures;
mod voxel_world;
mod zone_walls;

use std::error::Error;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use sdl3::pixels::Color;
use serde_json::json;

use bench::bench_views;
use map::MapName;
use play::{draw_text_overlay, play};
use render_metal::FrameData;
use run_folder::{format_utc_now, make_run_folder, save_png};
use session::OverlayText;
use settings::ServerSettings;
use voxel_world::VoxelWorld;

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;
const METAL_RENDER_SCALE: f64 = 1.0;
const WINDOW_TITLE: &str = "Hyperbolic wormholes (Rust)";
const MS_PER_S: f64 = 1000.0;
const ENERGY_SEGMENT_COUNT: u32 = 10;
const DEATH_MASK_PX: (u32, u32) = (240, 24);
const MENU_MASK_PX: (u32, u32) = (512, 312);
const BANNER_HEIGHT_FRAC: f32 = 0.2;
const MENU_HEIGHT_FRAC: f32 = 0.85;
const GAME_PASSWORD_HOME_PATH: &str = "Library/Application Support/Warped/game_password";
const LOGS_HOME_FOLDER: &str = "Library/Logs/Warped";
const ONLINE_GAMES_FILE: &str = "online_games.txt";
const OWNER_ONLY_FILE_MODE: u32 = 0o600;
const WRONG_PASSWORD_EXIT_CODE: u8 = 3;
const TITLE_WINDOW_PX: (u32, u32) = (960, 600);
const TITLE_FRAME_MS: u64 = 16;
const LOCAL_PLAY_OFF: &str = "Playing is only on a game server now: start one with `make serve-game` (--serve) and join it with `make join-game SERVER=<address> PORT=<port>` (--join <address>). Without a server, only the tools run: --bench-views and --play-route.";
const HOUSE_IN_DEMO: &str = "The house demo plays only in demos/hyperbolic_wormholes_rust/ now (make -C demos/hyperbolic_wormholes_rust join-game MAP=house-demo); the game plays lunar-surface.";
const DISABLE_BAR_MIN_FRAC: [f32; 2] = [0.4, 0.62];
const DISABLE_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.02];
const DISABLE_BAR_RGB: [f32; 3] = [1.0, 0.85, 0.3];
const MIN_IMAGE_SIDE_PX: u32 = 16;
const MAX_IMAGE_SIDE_PX: u32 = 8192;
const DISCOVERY_TIMEOUT_S: u64 = 5;
const JOIN_AUTO: &str = "auto";
const SERVER_START_S: f64 = 0.3;
const ROUTE_PROBE_ADDRESS: &str = "192.0.2.1:9";

/// `RendererKind` is which renderer draws the frames.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RendererKind {
    Metal,
}

/// `Sampling` is which pixels Metal traces: every other pixel each way, interpolating the rest
/// where that is exact enough, or every pixel.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Sampling {
    Adaptive,
    EveryPixel,
}

/// `RouteKind` is which route `--play-route` plays: a tour around each warp object.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RouteKind {
    WarpTour,
}

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Walk, fly and fire a laser through wormholes in hyperbolic space.",
    after_help = "Example: hyperbolic-wormholes --renderer metal --bench-views --width-px 2560 --height-px 1600"
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
    #[arg(long)]
    bench_views: bool,
    /// Image width for --bench-views, and the window's starting width (in screen
    /// points, which are pixels on a 1x display).
    #[arg(long, default_value_t = 1280)]
    width_px: u32,
    /// Image height for --bench-views, and the window's starting height.
    #[arg(long, default_value_t = 800)]
    height_px: u32,
    /// Timed frames per view for --bench-views.
    #[arg(long, default_value_t = 10)]
    frame_count: u32,
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
    #[arg(long, conflicts_with = "bench_views")]
    play_route: bool,
    /// Which route --play-route plays: a tour circling each warp object, from near to far and
    /// back, while the view swings around.
    #[arg(long, value_enum, default_value_t = RouteKind::WarpTour)]
    route: RouteKind,
    /// Host a game on the local network: start a game server as its own process (as --serve,
    /// without a window), play in it, and stop it when this window closes.
    #[arg(long, conflicts_with_all = ["join", "serve", "bench_views", "play_route"])]
    host: bool,
    /// Join a game on the local network: `auto` to find one, or a server's IP address (with an
    /// optional :port).
    #[arg(long, conflicts_with_all = ["serve", "bench_views", "play_route"])]
    join: Option<String>,
    /// Run only a game server, without a window, until --exit-after-s (or until stopped); it
    /// prints this computer's address and the command that joins it.
    #[arg(long, conflicts_with_all = ["bench_views", "play_route"])]
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
    #[arg(long, value_enum, default_value_t = MapName::LunarSurface)]
    map: MapName,
    /// With --map lunar-surface: start the window at one of the level's viewpoints (as
    /// --bench-views names them), e.g. black_hole.
    #[arg(long)]
    view: Option<String>,
    /// On a map whose black hole grows (lunar-surface): force the black hole's strength (0 to 1)
    /// instead of following the consoles (W-68).
    #[arg(long)]
    black_hole_strength: Option<f64>,
}

/// `Renderer` draws one frame's packed scene into an RGBA pixel buffer.
trait Renderer {
    /// `render` draws `data` (ray-traced at its own render size) into `pixels` at `output_px`
    /// (top row first, 4 bytes per pixel), upscaling it if `output_px` is larger.
    ///
    /// Example: `renderer.render(FrameData::Voxel(&frame), (2560, 1600), &mut pixels)?;`
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

/// `make_renderer` makes the renderer the command line asked for.
///
/// Example: `let mut renderer = make_renderer(RendererKind::Metal, Sampling::Adaptive)?;`
fn make_renderer(kind: RendererKind, sampling: Sampling) -> Result<Box<dyn Renderer>> {
    match kind {
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
/// Example: `let scale = get_default_render_scale(RendererKind::Metal);`
fn get_default_render_scale(kind: RendererKind) -> f64 {
    match kind {
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
    if arguments.frame_count == 0 {
        return Err("--frame-count must be at least 1; e.g. --frame-count 10".into());
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
    if arguments.map == MapName::HouseDemo {
        return Err(HOUSE_IN_DEMO.into());
    }
    if arguments.host {
        return Err(LOCAL_PLAY_OFF.into());
    }
    let is_window_without_server: bool = arguments.join.is_none()
        && !arguments.serve
        && !arguments.bench_views
        && !arguments.play_route;
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

/// `main` validates the command line and runs the server, the window or the benchmark.
///
/// Example: `cargo run --release -- --bench-views`
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

/// `run` validates the command line and runs what it asks for: the window, a server, the bot or
/// the benchmark.
///
/// Example: `run(Arguments::parse())?;`
fn run(arguments: Arguments) -> Result<()> {
    validate_arguments(&arguments)?;
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
/// Example: `start_title_choice(&title::Choice::Join(MapName::LunarSurface))?;`
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
    let (hit_count, eliminated_count) = multiplayer::run_voxel_bot(
        &VoxelWorld::FromMap(&map)?,
        &mut client,
        duration_s,
        arguments.black_hole_strength,
        |frame, id| save(FrameData::Voxel(frame), id),
    )?;
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
