//! `dark-forest` is the Dark Forest level: a bounded Euclidean forest with treehouses, rope
//! ladders and bridges, two pylons per team with one console each. Rust, SDL3 for the window,
//! input and audio, Metal for rendering. It plays alone against AI robots, or online against
//! other players through a game server (`--serve`, `--host`, `--join`).
//!
//! Example:
//!     cargo run --release --bin dark-forest -- --host
//!
//! Interface: the command-line flags below; controls are in the README.

#[allow(dead_code)]
mod asset_manifest;
mod assets;
mod forest_layout;
mod game;
mod map;
mod match_play;
#[allow(dead_code)]
mod net;
mod platform;
mod render;
mod scene;
mod server;
#[allow(dead_code)]
mod settings;
#[allow(dead_code)]
mod shot_noise;
#[allow(dead_code)]
mod thrown_wormholes;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use glam::{Vec2, Vec3};
use sdl3::event::Event;
use sdl3::keyboard::Scancode;

use game::audio::Audio;
use game::player::{Player, PlayerInput};
use game::session::{Controls, FrameData, Session};
use game::weapon::WeaponInput;
use game::world::Arena;
use match_play::{MatchOptions, MatchPlay};
use platform::Platform;
use render::metal::MetalRenderer;
use render::{FrameDescription, FrameTiming, RenderSettings, Renderer};
use scene::{Scene, SceneOptions};

/// `MOUSE_RAD_PER_PT` is the look speed per point of mouse motion.
const MOUSE_RAD_PER_PT: f32 = 0.0022;
/// `MAX_FRAME_S` caps a frame's simulated time after a stall.
const MAX_FRAME_S: f32 = 0.05;
/// `REPORT_INTERVAL_S` is how often frame-time statistics are printed.
const REPORT_INTERVAL_S: f32 = 5.0;
/// `RENDER_SCALE_STEP_FRAC` is how much `[` and `]` change the render scale.
const RENDER_SCALE_STEP_FRAC: f32 = 0.1;

/// `CameraView` is a fixed camera pose for screenshots and repeatable frame-time comparisons;
/// `--bench-views` runs them all.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum CameraView {
    Spawn,
    Bridge,
    Lookout,
    Treehouse,
    Firing,
}

impl CameraView {
    /// `ALL` is every view, in `--bench-views` order.
    const ALL: [CameraView; 5] = [
        CameraView::Spawn,
        CameraView::Bridge,
        CameraView::Lookout,
        CameraView::Treehouse,
        CameraView::Firing,
    ];

    /// `pose` returns the view's feet position, yaw and pitch (yaw 0 looks along -z, towards
    /// blue's base from red's).
    fn pose(self) -> (Vec3, f32, f32) {
        match self {
            CameraView::Spawn => (Vec3::new(0.0, 0.0, 50.0), 0.0, 0.0),
            CameraView::Bridge => (Vec3::new(11.0, forest_layout::DECK_TOP_M, 30.0), 0.3, -0.1),
            CameraView::Lookout => (Vec3::new(0.0, forest_layout::LOOKOUT_TOP_M, 2.0), 0.0, -0.3),
            CameraView::Treehouse => (
                Vec3::new(14.5, forest_layout::DECK_TOP_M, 46.0),
                -0.8,
                -0.05,
            ),
            CameraView::Firing => (Vec3::new(-4.0, 0.0, 20.0), 0.1, 0.02),
        }
    }
}

/// `Effect` is a render effect that `--disable-effect` can turn off at start.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
enum Effect {
    Ssao,
    Bloom,
    Fxaa,
    ColorGrade,
    VolumetricHaze,
    MaterialResponse,
}

/// `Args` are the command-line flags.
#[derive(Parser, Debug)]
#[command(
    about = "Dark Forest, a Warped level (Rust + SDL3 + Metal)",
    after_help = "Examples:\n  dark-forest                     play alone against robots\n  dark-forest --host              serve and play; others --join auto\n  dark-forest --serve --level-result-file /tmp/r.json\n  dark-forest --join 127.0.0.1 --team blue --bot --bot-objective consoles --exit-after-s 60"
)]
struct Args {
    /// Internal render resolution as a fraction of the window's pixels (0.25 to 1.0).
    #[arg(long, default_value_t = 1.0)]
    render_scale: f32,
    /// Number of AI robots when playing alone (0 to 6; none online).
    #[arg(long, default_value_t = 3)]
    bot_count: usize,
    /// Window width in points.
    #[arg(long, default_value_t = 1600)]
    width_pt: u32,
    /// Window height in points.
    #[arg(long, default_value_t = 1000)]
    height_pt: u32,
    /// Start in fullscreen.
    #[arg(long)]
    fullscreen: bool,
    /// Render as fast as possible instead of syncing to the display.
    #[arg(long)]
    no_vsync: bool,
    /// Let the autopilot play (for demos, screenshots, and benchmarks).
    #[arg(long)]
    autopilot: bool,
    /// Quit after this many seconds and print frame-time statistics (0 runs until closed).
    #[arg(long, default_value_t = 0.0)]
    benchmark_seconds: f32,
    /// Show the frame time in the top-left corner.
    #[arg(long)]
    show_frame_time: bool,
    /// Use the CC0 Poly Haven rifle even if the Sketchfab revolver is present.
    #[arg(long)]
    rifle: bool,
    /// Hold the camera at a fixed view (no autopilot, no mouse look); `firing` holds the trigger
    /// and reloads when the magazine is empty.
    #[arg(long, value_enum, conflicts_with = "autopilot")]
    camera_view: Option<CameraView>,
    /// Turn an effect off at start (repeatable); the F keys still toggle it.
    #[arg(long, value_enum)]
    disable_effect: Vec<Effect>,
    /// Run every camera view for this many seconds each, print its frame times, and quit.
    #[arg(long)]
    bench_views: Option<f32>,
    /// Let the autopilot walk the vertical route (ladders, bridges, lookout) and print its laps.
    #[arg(long)]
    play_route: bool,
    /// Directory holding the robot, gun, sound and sky assets (shared with ../fps_demo).
    #[arg(long)]
    assets_dir: Option<PathBuf>,
    /// Directory holding the forest textures that `download-assets` saved.
    #[arg(long)]
    textures_dir: Option<PathBuf>,
    /// The map file (default: common/maps/dark_forest/map.json).
    #[arg(long)]
    map: Option<PathBuf>,
    /// Write the forest's map to this file and quit.
    #[arg(long)]
    export_map: Option<PathBuf>,
    /// Run a game server without a window until killed.
    #[arg(long, conflicts_with_all = ["join", "host"])]
    serve: bool,
    /// Run a game server in this process and join it.
    #[arg(long, conflicts_with = "join")]
    host: bool,
    /// Join the game server at this address (or `auto` to find one on the network).
    #[arg(long)]
    join: Option<String>,
    /// UDP port of the game server.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    port: u16,
    /// Join this team at once (`blue` or `red`); otherwise the server picks.
    #[arg(long, value_parser = ["blue", "red"])]
    team: Option<String>,
    /// Let the autopilot play this window (a bot for tests).
    #[arg(long)]
    bot: bool,
    /// What the bot does: `consoles` holds E on the other team's consoles until its team wins.
    #[arg(long, value_parser = ["consoles"])]
    bot_objective: Option<String>,
    /// Quit after this many seconds.
    #[arg(long)]
    exit_after_s: Option<f32>,
    /// With --serve: write {"winner": "blue"|"red"} here on a win, and start no new game.
    #[arg(long)]
    level_result_file: Option<PathBuf>,
}

/// `VERTICAL_ROUTE` is `--play-route`'s loop from red's spawn: up a ladder to the treehouse, over
/// the bridges, up the lookout, down and back.
const VERTICAL_ROUTE: [Vec3; 0] = [];

/// `validate_args` checks every flag before anything starts.
fn validate_args(args: &Args, assets_dir: &Path) -> Result<(), String> {
    let example: &str = "example: dark-forest --render-scale 0.8 --bot-count 4";
    if !(0.25..=1.0).contains(&args.render_scale) {
        return Err(format!(
            "--render-scale must be in 0.25..=1.0, got {}; {example}",
            args.render_scale
        ));
    }
    if args.bot_count > 6 {
        return Err(format!(
            "--bot-count must be in 0..=6, got {}; {example}",
            args.bot_count
        ));
    }
    if args.width_pt < 320 || args.height_pt < 200 {
        return Err(format!(
            "--width-pt/--height-pt must be at least 320x200; {example}"
        ));
    }
    if !(0.0..=3600.0).contains(&args.benchmark_seconds) {
        return Err(format!(
            "--benchmark-seconds must be in 0..=3600; {example}"
        ));
    }
    if !asset_manifest::hdri_path(assets_dir).exists() {
        return Err(format!(
            "assets are missing from {}; run `make download-assets` first",
            assets_dir.display()
        ));
    }
    Ok(())
}

/// `FrameStats` collects frame times for reports.
#[derive(Default)]
struct FrameStats {
    frame_ms: Vec<f32>,
    gpu_ms: Vec<f32>,
}

impl FrameStats {
    /// `summary` returns mean, median, 99th percentile frame time and mean GPU time.
    fn summary(&self) -> String {
        if self.frame_ms.is_empty() {
            return "no frames".to_string();
        }
        let mut sorted: Vec<f32> = self.frame_ms.clone();
        sorted.sort_by(f32::total_cmp);
        let percentile = |frac: f32| sorted[((sorted.len() - 1) as f32 * frac) as usize];
        let mean_ms: f32 = sorted.iter().sum::<f32>() / sorted.len() as f32;
        let gpu_mean_ms: f32 = self.gpu_ms.iter().sum::<f32>() / self.gpu_ms.len().max(1) as f32;
        format!(
            "{} frames: mean {:.2} ms ({:.0} fps), median {:.2} ms, p99 {:.2} ms, GPU mean {:.2} ms",
            sorted.len(),
            mean_ms,
            1000.0 / mean_ms,
            percentile(0.5),
            percentile(0.99),
            gpu_mean_ms
        )
    }
}

fn main() {
    let args: Args = Args::parse();
    let map_path: PathBuf = args.map.clone().unwrap_or_else(map::default_map_path);
    if let Some(path) = &args.export_map {
        let json: String = serde_json::to_string_pretty(&forest_layout::build()).expect("map JSON");
        std::fs::write(path, json + "\n").expect("can't write the map");
        println!("wrote {}", path.display());
        return;
    }
    if args.serve {
        let options: server::ServerOptions = server::ServerOptions {
            port: args.port,
            map_path,
            level_result_file: args.level_result_file.clone(),
            game_password: Vec::new(),
        };
        if let Err(error) = server::run(&options, &AtomicBool::new(false)) {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
        return;
    }
    let manifest_dir: &Path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets_dir: PathBuf = args.assets_dir.clone().unwrap_or_else(|| {
        manifest_dir
            .join("../fps_demo")
            .join(asset_manifest::ASSETS_DIR_NAME)
    });
    let textures_dir: PathBuf = args
        .textures_dir
        .clone()
        .unwrap_or_else(|| manifest_dir.join(asset_manifest::ASSETS_DIR_NAME));
    let map_file: map::MapFile = map::load(&map_path).unwrap_or_else(|error| {
        eprintln!(
            "error: {error}; write it with --export-map {}",
            map_path.display()
        );
        std::process::exit(2);
    });
    let client: Option<net::Client> = if args.host || args.join.is_some() {
        if args.host {
            let options: server::ServerOptions = server::ServerOptions {
                port: args.port,
                map_path: map_path.clone(),
                level_result_file: args.level_result_file.clone(),
                game_password: Vec::new(),
            };
            std::thread::spawn(move || {
                if let Err(error) = server::run(&options, &AtomicBool::new(false)) {
                    eprintln!("server error: {error}");
                }
            });
            std::thread::sleep(Duration::from_millis(200));
        }
        let address: std::net::SocketAddr = match args.join.as_deref() {
            None => ([127, 0, 0, 1], args.port).into(),
            Some("auto") => net::find_server(Duration::from_secs(5)).unwrap_or_else(|error| {
                eprintln!("error: no game server found ({error})");
                std::process::exit(2);
            }),
            Some(host) => {
                use std::net::ToSocketAddrs;
                let with_port: String = if host.contains(':') {
                    host.to_string()
                } else {
                    format!("{host}:{}", args.port)
                };
                with_port
                    .to_socket_addrs()
                    .ok()
                    .and_then(|mut a| a.next())
                    .unwrap_or_else(|| {
                        eprintln!("error: can't resolve --join {host}");
                        std::process::exit(2);
                    })
            }
        };
        Some(
            net::Client::Connect(address, &map_file.name, b"").unwrap_or_else(|error| {
                eprintln!("error: can't reach {address}: {error}");
                std::process::exit(2);
            }),
        )
    } else {
        None
    };
    if let Err(message) = validate_args(&args, &assets_dir) {
        eprintln!("error: {message}");
        std::process::exit(2);
    }

    let mut platform: Platform = Platform::new(
        "Dark Forest",
        args.width_pt,
        args.height_pt,
        args.fullscreen,
    );
    let mut renderer: MetalRenderer = MetalRenderer::new(platform.metal_layer());
    println!("GPU: {}", renderer.device_name());
    let (mut width_px, mut height_px) = platform.window.size_in_pixels();
    renderer.resize(width_px, height_px);
    let audio: Audio = Audio::new(&platform.sdl);
    let arena: Arena = Arena::from_map(&map_file);
    let scene: Scene = Scene::load(
        &mut renderer,
        &assets_dir,
        &textures_dir,
        &arena,
        &SceneOptions {
            should_use_rifle: args.rifle,
        },
    );
    let is_online: bool = client.is_some();
    let bot_count: usize = if is_online { 0 } else { args.bot_count };
    let mut session: Session = Session::new(
        arena,
        bot_count,
        args.autopilot || args.bot || args.play_route,
    );
    if args.play_route {
        session.route = VERTICAL_ROUTE.to_vec();
    }
    let mut play: MatchPlay = MatchPlay::New(
        client,
        MatchOptions {
            team: args.team.as_deref().map(|team| u32::from(team == "red")),
            is_console_bot: args.bot_objective.is_some(),
        },
    )
    .unwrap_or_else(|error| {
        eprintln!("error: can't join: {error}");
        std::process::exit(2);
    });
    let mut last_title: Instant = Instant::now();
    let mut view_stats: Vec<FrameStats> = CameraView::ALL
        .iter()
        .map(|_| FrameStats::default())
        .collect();
    let mut settings: RenderSettings = RenderSettings {
        render_scale_frac: args.render_scale,
        is_ssao_enabled: !args.disable_effect.contains(&Effect::Ssao),
        is_bloom_enabled: !args.disable_effect.contains(&Effect::Bloom),
        is_fxaa_enabled: !args.disable_effect.contains(&Effect::Fxaa),
        is_color_grade_enabled: !args.disable_effect.contains(&Effect::ColorGrade),
        is_volumetric_haze_enabled: !args.disable_effect.contains(&Effect::VolumetricHaze),
        is_material_response_enabled: !args.disable_effect.contains(&Effect::MaterialResponse),
        is_vsync_enabled: !args.no_vsync,
    };
    let mut is_showing_frame_time: bool = args.show_frame_time;

    let mouse = platform.sdl.mouse();
    let mut is_mouse_captured: bool = true;
    mouse.set_relative_mouse_mode(&platform.window, true);
    let mut event_pump = platform.sdl.event_pump().expect("SDL event pump failed");
    let start: Instant = Instant::now();
    let mut last_frame: Instant = Instant::now();
    let mut stats: FrameStats = FrameStats::default();
    let mut report_stats: FrameStats = FrameStats::default();
    let mut last_report_s: f32 = 0.0;
    let mut smoothed_frame_ms: f32 = 8.3;
    let mut timing: FrameTiming = FrameTiming::default();
    'running: loop {
        let mut look_delta_pt: Vec2 = Vec2::ZERO;
        let mut is_jump_pressed: bool = false;
        let mut is_reload_pressed: bool = false;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } => match scancode {
                    Scancode::Escape if is_mouse_captured => {
                        is_mouse_captured = false;
                        mouse.set_relative_mouse_mode(&platform.window, false);
                    }
                    Scancode::Escape => break 'running,
                    Scancode::Space => is_jump_pressed = true,
                    Scancode::R => is_reload_pressed = true,
                    Scancode::F1 => settings.is_ssao_enabled = !settings.is_ssao_enabled,
                    Scancode::F2 => settings.is_bloom_enabled = !settings.is_bloom_enabled,
                    Scancode::F3 => settings.is_fxaa_enabled = !settings.is_fxaa_enabled,
                    Scancode::F4 => is_showing_frame_time = !is_showing_frame_time,
                    Scancode::F5 => session.is_autopilot = !session.is_autopilot,
                    Scancode::F6 => {
                        settings.is_color_grade_enabled = !settings.is_color_grade_enabled
                    }
                    Scancode::F7 => {
                        settings.is_volumetric_haze_enabled = !settings.is_volumetric_haze_enabled
                    }
                    Scancode::F8 => {
                        settings.is_material_response_enabled =
                            !settings.is_material_response_enabled
                    }
                    Scancode::LeftBracket => {
                        settings.render_scale_frac =
                            (settings.render_scale_frac - RENDER_SCALE_STEP_FRAC).max(0.3);
                    }
                    Scancode::RightBracket => {
                        settings.render_scale_frac =
                            (settings.render_scale_frac + RENDER_SCALE_STEP_FRAC).min(1.0);
                    }
                    _ => {}
                },
                Event::MouseButtonDown { .. } if !is_mouse_captured => {
                    is_mouse_captured = true;
                    mouse.set_relative_mouse_mode(&platform.window, true);
                }
                Event::MouseMotion { xrel, yrel, .. } if is_mouse_captured => {
                    look_delta_pt += Vec2::new(xrel, yrel);
                }
                _ => {}
            }
        }
        let (new_width_px, new_height_px) = platform.window.size_in_pixels();
        if (new_width_px, new_height_px) != (width_px, height_px)
            && new_width_px > 0
            && new_height_px > 0
        {
            (width_px, height_px) = (new_width_px, new_height_px);
            renderer.resize(width_px, height_px);
        }

        let now: Instant = Instant::now();
        let frame_s: f32 = (now - last_frame).as_secs_f32();
        last_frame = now;
        let dt_s: f32 = frame_s.min(MAX_FRAME_S);
        let keys = event_pump.keyboard_state();
        let mouse_state = event_pump.mouse_state();
        let axis = |positive: Scancode, negative: Scancode| -> f32 {
            f32::from(u8::from(keys.is_scancode_pressed(positive)))
                - f32::from(u8::from(keys.is_scancode_pressed(negative)))
        };
        let controls: Controls = Controls {
            player: PlayerInput {
                move_axes: Vec2::new(
                    axis(Scancode::D, Scancode::A),
                    axis(Scancode::W, Scancode::S),
                ),
                look_delta_rad: look_delta_pt * MOUSE_RAD_PER_PT,
                is_sprint_held: keys.is_scancode_pressed(Scancode::LShift),
                is_crouch_held: keys.is_scancode_pressed(Scancode::LCtrl)
                    || keys.is_scancode_pressed(Scancode::C),
                is_jump_pressed,
                is_aiming: false,
            },
            weapon: WeaponInput {
                is_trigger_held: is_mouse_captured && mouse_state.left(),
                is_reload_pressed,
                is_aim_held: is_mouse_captured && mouse_state.right(),
            },
        };
        let bench_view: Option<usize> = args
            .bench_views
            .map(|seconds| (start.elapsed().as_secs_f32() / seconds) as usize);
        if bench_view.is_some_and(|index| index >= CameraView::ALL.len()) {
            break 'running;
        }
        let camera_view: Option<CameraView> = bench_view
            .map(|index| CameraView::ALL[index])
            .or(args.camera_view);
        let controls: Controls = match camera_view {
            Some(view) => {
                let (feet_m, yaw_rad, pitch_rad) = view.pose();
                session.player = Player::new(feet_m, yaw_rad);
                session.player.pitch_rad = pitch_rad;
                session.player.is_on_ground = true;
                session.is_autopilot = false;
                let is_firing_view: bool = view == CameraView::Firing;
                Controls {
                    weapon: WeaponInput {
                        is_trigger_held: is_firing_view,
                        is_reload_pressed: is_firing_view && session.weapon.ammo == 0,
                        ..WeaponInput::default()
                    },
                    ..Controls::default()
                }
            }
            None => controls,
        };
        session.update(&controls, &scene, &audio, dt_s);
        let is_e_held: bool = keys.is_scancode_pressed(Scancode::E);
        if let Err(error) = play.update(&mut session, &scene, is_e_held, dt_s) {
            eprintln!("network error: {error}");
        }
        if last_title.elapsed().as_secs_f32() > 0.5 {
            last_title = Instant::now();
            let _ = platform.window.set_title(&play.status(&session));
        }

        smoothed_frame_ms += (frame_s * 1000.0 - smoothed_frame_ms) * 0.05;
        session.show_frame_ms = is_showing_frame_time.then_some(smoothed_frame_ms);
        let frame: FrameData = session.frame_data(&scene, width_px as f32, height_px as f32);
        timing = renderer.render_frame(
            &FrameDescription {
                camera: frame.camera,
                world_draws: &frame.world_draws,
                viewmodel_draws: &frame.viewmodel_draws,
                joint_matrices: &frame.joint_matrices,
                point_lights: &frame.point_lights,
                decals: &frame.decals,
                world_particles: &frame.world_particles,
                viewmodel_particles: &frame.viewmodel_particles,
                hud_rects: &frame.hud_rects,
                time_s: session.time_s,
                damage_flash_frac: frame.damage_flash_frac,
            },
            &settings,
        );

        let elapsed_s: f32 = start.elapsed().as_secs_f32();
        if stats.frame_ms.len() > 30 || elapsed_s > 2.0 {
            stats.frame_ms.push(frame_s * 1000.0);
            stats.gpu_ms.push(timing.gpu_ms as f32);
            report_stats.frame_ms.push(frame_s * 1000.0);
            report_stats.gpu_ms.push(timing.gpu_ms as f32);
            if let Some(index) = bench_view {
                let seconds: f32 = args.bench_views.unwrap_or(1.0);
                if start.elapsed().as_secs_f32() - index as f32 * seconds > 1.0 {
                    view_stats[index].frame_ms.push(frame_s * 1000.0);
                    view_stats[index].gpu_ms.push(timing.gpu_ms as f32);
                }
            }
        }
        if elapsed_s - last_report_s > REPORT_INTERVAL_S {
            last_report_s = elapsed_s;
            println!(
                "{}x{} px at render scale {:.2}: {}",
                width_px,
                height_px,
                settings.render_scale_frac,
                report_stats.summary()
            );
            report_stats = FrameStats::default();
        }
        if args.benchmark_seconds > 0.0 && elapsed_s > args.benchmark_seconds {
            break 'running;
        }
        if args.exit_after_s.is_some_and(|seconds| elapsed_s > seconds) {
            break 'running;
        }
    }
    let _ = timing;
    if args.bench_views.is_some() {
        for (view, view_stat) in CameraView::ALL.iter().zip(&view_stats) {
            println!(
                "view {view:?} at {width_px}x{height_px} px: {}",
                view_stat.summary()
            );
        }
    }
    if args.play_route {
        println!(
            "route laps done: {}; ended at {:?}",
            session.laps_done, session.player.feet_m
        );
    }
    if let Some(winner) = session.winner {
        println!("winner: {}", ["blue", "red"][winner as usize % 2]);
    }
    println!("session: {}", stats.summary());
}
