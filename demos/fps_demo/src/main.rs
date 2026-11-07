//! `warped-fps-demo` is a single-player FPS demo: Rust, SDL3 for the window, input and audio,
//! and Metal for rendering. It loads the assets that `download-assets` saved in `.assets/`.
//!
//! Example:
//!     cargo run --release --bin warped-fps-demo -- --render-scale 0.8
//!
//! Interface: the command-line flags below; controls are in the README.

#[allow(dead_code)]
mod asset_manifest;
mod assets;
mod game;
mod platform;
mod render;
mod scene;

use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use glam::Vec2;
use sdl3::event::Event;
use sdl3::keyboard::Scancode;

use game::audio::Audio;
use game::player::PlayerInput;
use game::session::{Controls, FrameData, Session};
use game::weapon::WeaponInput;
use game::world::Arena;
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

/// `Args` are the command-line flags.
#[derive(Parser, Debug)]
#[command(about = "Warped FPS demo (Rust + SDL3 + Metal)")]
struct Args {
    /// Internal render resolution as a fraction of the window's pixels (0.25 to 1.0).
    #[arg(long, default_value_t = 1.0)]
    render_scale: f32,
    /// Number of enemy bots (0 to 6).
    #[arg(long, default_value_t = 4)]
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
    /// Directory holding the downloaded assets.
    #[arg(long)]
    assets_dir: Option<PathBuf>,
}

/// `validate_args` checks every flag before anything starts.
fn validate_args(args: &Args, assets_dir: &Path) -> Result<(), String> {
    let example: &str = "example: warped-fps-demo --render-scale 0.8 --bot-count 4";
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
    let assets_dir: PathBuf = args.assets_dir.clone().unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(asset_manifest::ASSETS_DIR_NAME)
    });
    if let Err(message) = validate_args(&args, &assets_dir) {
        eprintln!("error: {message}");
        std::process::exit(2);
    }

    let platform: Platform = Platform::new(
        "Warped FPS demo",
        args.width_pt,
        args.height_pt,
        args.fullscreen,
    );
    let mut renderer: MetalRenderer = MetalRenderer::new(platform.metal_layer());
    println!("GPU: {}", renderer.device_name());
    let (mut width_px, mut height_px) = platform.window.size_in_pixels();
    renderer.resize(width_px, height_px);
    let audio: Audio = Audio::new(&platform.sdl);
    let arena: Arena = Arena::new();
    let scene: Scene = Scene::load(
        &mut renderer,
        &assets_dir,
        &arena,
        &SceneOptions {
            should_use_rifle: args.rifle,
        },
    );
    let mut session: Session = Session::new(arena, args.bot_count, args.autopilot);
    let mut settings: RenderSettings = RenderSettings {
        render_scale_frac: args.render_scale,
        is_ssao_enabled: true,
        is_bloom_enabled: true,
        is_fxaa_enabled: true,
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
        session.update(&controls, &scene, &audio, dt_s);

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
    }
    let _ = timing;
    println!("session: {}", stats.summary());
}
