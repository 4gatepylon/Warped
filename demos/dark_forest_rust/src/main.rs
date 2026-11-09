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
mod console_reach;
mod credits;
mod exit_prompt;
mod forest_layout;
mod game;
mod hud_bars;
mod map;
mod match_play;
mod menu;
mod minimap;
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
mod text_overlay;
#[allow(dead_code)]
mod thrown_wormholes;
mod title;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use glam::{Vec2, Vec3, Vec4};
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use sdl3::mouse::MouseButton;
use shot_noise::ShotNoise;

use exit_prompt::{ExitOutcome, ExitPrompt};
use game::audio::Audio;
use game::console_bot::BotWalker;
use game::guns::{Gun, GunInput, Guns};
use game::player::{Player, PlayerInput};
use game::session::{Controls, FrameData, RobotHold, Session};
use game::world::Arena;
use match_play::{MatchOptions, MatchPlay};
use menu::{Menu, MenuOutcome, MenuSettings, Scoreboard};
use minimap::{Mark, Minimap};
use platform::Platform;
use render::frame_clock::FrameClock;
use render::frame_trace::{CpuFrameRow, FrameTrace};
use render::metal::MetalRenderer;
use render::overlay::HitEffect;
use render::{FrameDescription, FrameTiming, HudImage, HudRect, RenderSettings, Renderer};
use scene::Scene;
use text_overlay::TextPicture;

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
    Gate,
    Lift,
}

impl CameraView {
    /// `ALL` is every view, in `--bench-views` order.
    const ALL: [CameraView; 7] = [
        CameraView::Spawn,
        CameraView::Bridge,
        CameraView::Lookout,
        CameraView::Treehouse,
        CameraView::Firing,
        CameraView::Gate,
        CameraView::Lift,
    ];

    /// `robot_hold` is where the view holds robot 0, if it has robots: just beyond the partner of
    /// the mouth that `Gate` and `Lift` look into, facing back through it.
    fn robot_hold(self) -> Option<RobotHold> {
        let feet_m: Vec3 = match self {
            CameraView::Gate => Vec3::new(2.0, 0.0, -30.0),
            CameraView::Lift => Vec3::new(10.3, forest_layout::DECK_TOP_M, 35.5),
            _ => return None,
        };
        Some(RobotHold {
            feet_m,
            yaw_rad: 0.0,
            speed_m_s: 0.0,
        })
    }

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
            CameraView::Gate => (Vec3::new(-2.0, 0.0, 30.5), 0.0, 0.0),
            CameraView::Lift => (Vec3::new(-4.0, 0.0, 40.0), 0.0, 0.05),
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
    /// Write every frame's timing (CPU phases, GPU, when it was shown) to this CSV on exit.
    #[arg(long)]
    frame_times_file: Option<PathBuf>,
    /// Start with gun 2, the rifle (the energy revolver), in hand instead of gun 1, the sniper.
    #[arg(long)]
    rifle: bool,
    /// Hold the camera at a fixed view (no autopilot, no mouse look); `firing` holds the trigger
    /// (the rifle fires while it is held, the sniper each time it has reloaded).
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
    /// Let a bot play this window, hidden and drawing nothing (for tests and matches): it hunts the
    /// other team's players, or turns off their consoles with --bot-objective consoles.
    #[arg(long)]
    bot: bool,
    /// What the bot does: `consoles` walks to the other team's consoles and holds E on each, in
    /// reach, until its team wins (logging `bot reached console <i> on foot in <t> s`).
    #[arg(long, value_parser = ["consoles"])]
    bot_objective: Option<String>,
    /// Quit after this many seconds.
    #[arg(long)]
    exit_after_s: Option<f32>,
    /// With --serve: write {"winner": "blue"|"red"} here on a win, and start no new game.
    #[arg(long)]
    level_result_file: Option<PathBuf>,
    /// A name the match server gives this window or bot; accepted and ignored.
    #[arg(long)]
    label: Option<String>,
    /// Start playing at once instead of on the title screen (flags such as --join already do).
    #[arg(long)]
    no_title: bool,
    /// Save one frame, without the HUD, to this PNG after the scene settles, then quit.
    #[arg(long)]
    photo: Option<PathBuf>,
    /// Downscale the --photo to this width (keeping its aspect), e.g. to supersample.
    #[arg(long)]
    photo_width_px: Option<u32>,
    /// Save every frame, without the HUD, as frame_NNNN.png in this folder (which must exist), at
    /// 30 simulated frames a second, for --video-seconds, then quit.
    #[arg(long)]
    video_frames: Option<PathBuf>,
    /// Keep the HUD in --photo and --video-frames pictures, as the window shows it.
    #[arg(long)]
    photo_hud: bool,
    /// Take --photo at this many simulated seconds instead of after 3 s.
    #[arg(long, default_value_t = PHOTO_SETTLE_S)]
    photo_at_s: f32,
    /// Hurt the player by one robot shot every this many simulated seconds, for pictures of hits
    /// and of "YOU DIED".
    #[arg(long)]
    hurt_player_every_s: Option<f32>,
    /// Hold `E` (turning off a console within reach), for pictures of the console bar.
    #[arg(long)]
    hold_console: bool,
    /// Hold `F` (the flashlight), for pictures of it.
    #[arg(long)]
    hold_flashlight: bool,
    /// How many simulated seconds --video-frames records.
    #[arg(long, default_value_t = 8.0)]
    video_seconds: f32,
    /// Hold the camera here: feet x,y,z in meters, yaw and pitch in radians.
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    camera_pose: Option<Vec<f32>>,
    /// Vertical field of view in degrees, instead of the weapon's (narrow from far away looks
    /// almost flat, for map pictures).
    #[arg(long)]
    fov_deg: Option<f32>,
    /// How far from the camera the sun casts shadows, in meters.
    #[arg(long, default_value_t = 110.0)]
    shadow_distance_m: f32,
    /// Hold robot 0 here (it doesn't think or shoot): feet x,y,z in meters and yaw in radians.
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    robot_pose: Option<Vec<f32>>,
    /// With --robot-pose, the held robot walks straight ahead at this speed (through mouths) and
    /// starts again from its pose every 10 s.
    #[arg(long, default_value_t = 0.0)]
    robot_walk_m_per_s: f32,
    /// Hold the trigger (the sniper fires each time it has reloaded), for pictures of lasers.
    #[arg(long)]
    hold_trigger: bool,
    /// Leave out the tree canopies (for pictures from above).
    #[arg(long)]
    hide_canopy: bool,
    /// Open the menu on its map page (for --photo).
    #[arg(long)]
    photo_map_page: bool,
    /// Draw the whole forest from straight above (no canopies, robots, vignette or grain), crop it
    /// to the arena and save it to this PNG for the menu's map page, then quit.
    #[arg(long)]
    map_picture: Option<PathBuf>,
}

/// `PHOTO_SETTLE_S` is how long --photo lets the robots, shadows and exposure settle, in simulated
/// seconds.
const PHOTO_SETTLE_S: f32 = 3.0;
/// `HURT_PLAYER_DAMAGE` is how much --hurt-player-every-s takes each time: one rifle hit to the
/// body (of the server settings' 12).
const HURT_PLAYER_DAMAGE: f32 = game::player::MAX_HEALTH / 12.0;
/// `BOT_STEP_S` is how often a `--bot` (hidden, drawing nothing) steps its game.
const BOT_STEP_S: f32 = 1.0 / 60.0;
/// `RECORDING_DT_S` is the fixed step of --photo and --video-frames, so the same command gives the
/// same picture.
const RECORDING_DT_S: f32 = 1.0 / 30.0;
/// `RECORDING_SHOT_NOISE_SEED` seeds the shots' stray while recording, so pictures repeat.
const RECORDING_SHOT_NOISE_SEED: u64 = 69;

/// `MAP_PICTURE_PATH` is where `make map-picture` saves the map page's picture, from this demo's
/// folder.
const MAP_PICTURE_PATH: &str = "assets/map_picture.png";
/// `MAP_PICTURE_HEIGHT_M` is how high above the forest the map picture is taken, so it looks
/// almost flat; `MAP_PICTURE_SHADOW_DISTANCE_M` lets the sun's shadows reach that far.
const MAP_PICTURE_HEIGHT_M: f32 = 600.0;
const MAP_PICTURE_SHADOW_DISTANCE_M: f32 = 1000.0;

/// `save_map_picture` reads the last frame back, crops it to the arena's bounds and writes it to
/// `path` at `minimap::MAP_PICTURE_PX_PER_M`, opaque.
fn save_map_picture(renderer: &mut dyn Renderer, path: &Path, arena: &Arena) {
    let (frame_width_px, frame_height_px, rgba) = renderer.read_frame_rgba();
    let picture: image::RgbaImage =
        image::RgbaImage::from_raw(frame_width_px, frame_height_px, rgba).expect("RGBA frame");
    let size_m: Vec2 = Vec2::new(
        arena.bounds_max_m.x - arena.bounds_min_m.x,
        arena.bounds_max_m.z - arena.bounds_min_m.z,
    );
    let crop_width_px: u32 =
        ((frame_height_px as f32 * size_m.x / size_m.y).round() as u32).min(frame_width_px);
    let cropped: image::RgbaImage = image::imageops::crop_imm(
        &picture,
        (frame_width_px - crop_width_px) / 2,
        0,
        crop_width_px,
        frame_height_px,
    )
    .to_image();
    let mut map_picture: image::RgbaImage = image::imageops::resize(
        &cropped,
        (size_m.x * minimap::MAP_PICTURE_PX_PER_M).round() as u32,
        (size_m.y * minimap::MAP_PICTURE_PX_PER_M).round() as u32,
        image::imageops::FilterType::Lanczos3,
    );
    for pixel in map_picture.pixels_mut() {
        pixel.0[3] = u8::MAX;
    }
    map_picture
        .save(path)
        .expect("writing the map picture failed");
}

/// `LENS_FACE_PX` is the side of each face of the wormhole lens atlas.
const LENS_FACE_PX: u32 = 384;
/// `LENS_CAPTURE_RENDER_SCALE_FRAC` is the render scale the lens atlas is drawn at.
const LENS_CAPTURE_RENDER_SCALE_FRAC: f32 = 0.5;

/// `capture_wormhole_lens` draws the static world and the map's lights seen from every wormhole
/// mouth's centre (six 90° faces each, without vignette or grain) into the lens atlas.
fn capture_wormhole_lens(
    renderer: &mut dyn Renderer,
    session: &Session,
    scene: &Scene,
    settings: &RenderSettings,
    width_px: u32,
    height_px: u32,
) -> render::WormholeLens {
    let mouths: &[game::warps::WormholeMouth] = &session.arena.warps.mouths;
    let frame: FrameData = session.frame_data(scene, width_px as f32, height_px as f32);
    let capture_settings: RenderSettings = RenderSettings {
        render_scale_frac: LENS_CAPTURE_RENDER_SCALE_FRAC,
        ..*settings
    };
    let mut atlas: image::RgbaImage =
        image::RgbaImage::new(LENS_FACE_PX * 6, LENS_FACE_PX * mouths.len() as u32);
    renderer.set_is_capturing(true);
    for (row, mouth) in mouths.iter().enumerate() {
        for (face, (forward, up)) in render::LENS_FACES.iter().enumerate() {
            let camera: render::Camera = render::Camera {
                view: glam::Mat4::look_to_rh(mouth.centre_m, *forward, *up),
                position_m: mouth.centre_m,
                fov_y_rad: std::f32::consts::FRAC_PI_2,
                ..frame.camera
            };
            renderer.render_frame(
                &FrameDescription {
                    camera,
                    world_draws: &scene.static_draws,
                    static_draw_count: scene.static_draws.len(),
                    viewmodel_draws: &[],
                    joint_matrices: &[],
                    point_lights: &frame.point_lights,
                    decals: &[],
                    world_particles: &[],
                    viewmodel_particles: &[],
                    hud_rects: &[],
                    hud_images: &[],
                    time_s: 0.0,
                    hit_effect: HitEffect::default(),
                    lens_draws: &[],
                    lens_particles: &[],
                    beams: &[],
                    beam_speckle_shares: glam::Vec2::ZERO,
                    overlay_rects: &[],
                },
                &capture_settings,
            );
            let (frame_width_px, frame_height_px, rgba) = renderer.read_frame_rgba();
            let (_, _, depths) = renderer.read_frame_depth();
            let encoded_depths: Vec<u8> = depths
                .iter()
                .map(|&depth| render::encode_lens_depth(camera.near_m, depth))
                .collect();
            let depth_picture: image::GrayImage =
                image::GrayImage::from_raw(frame_width_px, frame_height_px, encoded_depths)
                    .expect("depth frame");
            let picture: image::RgbaImage =
                image::RgbaImage::from_raw(frame_width_px, frame_height_px, rgba)
                    .expect("RGBA frame");
            let side_px: u32 = frame_width_px.min(frame_height_px);
            let square: image::RgbaImage = image::imageops::crop_imm(
                &picture,
                (frame_width_px - side_px) / 2,
                (frame_height_px - side_px) / 2,
                side_px,
                side_px,
            )
            .to_image();
            let mut face_picture: image::RgbaImage = image::imageops::resize(
                &square,
                LENS_FACE_PX,
                LENS_FACE_PX,
                image::imageops::FilterType::Triangle,
            );
            let face_depths: image::GrayImage = image::imageops::resize(
                &image::imageops::crop_imm(
                    &depth_picture,
                    (frame_width_px - side_px) / 2,
                    (frame_height_px - side_px) / 2,
                    side_px,
                    side_px,
                )
                .to_image(),
                LENS_FACE_PX,
                LENS_FACE_PX,
                image::imageops::FilterType::Nearest,
            );
            for (pixel, depth) in face_picture.pixels_mut().zip(face_depths.pixels()) {
                pixel.0[3] = depth.0[0];
            }
            image::imageops::replace(
                &mut atlas,
                &face_picture,
                i64::from(face as u32 * LENS_FACE_PX),
                i64::from(row as u32 * LENS_FACE_PX),
            );
        }
    }
    renderer.set_is_capturing(false);
    render::WormholeLens {
        mouths: mouths
            .iter()
            .map(|mouth| render::LensMouth {
                centre_m: mouth.centre_m,
                lens_throat_m: mouth.get_lens_throat_m(),
                reach_m: mouth.reach_m,
                lambda_squared: mouth.lambda_squared,
                floor_m: game::collision::cast_ray(
                    &scene.solids,
                    mouth.centre_m,
                    Vec3::NEG_Y,
                    20.0,
                )
                .map_or(0.0, |(_, hit)| mouth.centre_m.y - hit.distance_m),
                partner: mouth.partner,
            })
            .collect(),
        atlas: render::TextureData {
            width: atlas.width(),
            height: atlas.height(),
            format: render::TextureFormat::Rgba8Linear,
            pixels: atlas.into_raw(),
        },
    }
}

/// `hold_player` puts the player at `feet_m` facing `yaw_rad`, standing and still, as a fixed view
/// does each frame, but keeps their health and whether they are dead, so hits and deaths still show.
///
/// Example: `hold_player(&mut session, Vec3::new(0.0, 0.0, 50.0), 0.0);`
fn hold_player(session: &mut Session, feet_m: Vec3, yaw_rad: f32) {
    let (health, dead_time_s) = (session.player.health, session.player.dead_time_s);
    session.player = Player::new(feet_m, yaw_rad);
    session.player.health = health;
    session.player.dead_time_s = dead_time_s;
}

/// `save_photo` reads the last frame back (with its HUD if `is_with_hud`, which needs
/// `set_is_capturing_hud`) and writes it to `path` as a PNG, downscaled to `width_px` if given.
fn save_photo(
    renderer: &mut dyn Renderer,
    path: &std::path::Path,
    width_px: Option<u32>,
    is_with_hud: bool,
) {
    let (frame_width_px, frame_height_px, rgba) = if is_with_hud {
        renderer.read_hud_frame_rgba()
    } else {
        renderer.read_frame_rgba()
    };
    let mut picture: image::RgbaImage =
        image::RgbaImage::from_raw(frame_width_px, frame_height_px, rgba).expect("RGBA frame");
    for pixel in picture.pixels_mut() {
        pixel[3] = 255;
    }
    if let Some(width_px) = width_px {
        let height_px: u32 = width_px * frame_height_px / frame_width_px;
        picture = image::imageops::resize(
            &picture,
            width_px,
            height_px,
            image::imageops::FilterType::Lanczos3,
        );
    }
    picture.save(path).expect("writing the photo failed");
    println!("wrote {}", path.display());
}

/// `VERTICAL_ROUTE` is `--play-route`'s loop from red's spawn: up a ladder to the treehouse, over
/// the bridges, up the lookout, down, and back round a grove and wide of the wormhole mouth at
/// (-2, 24), which would otherwise send it to the far side.
const VERTICAL_ROUTE: [Vec3; 22] = [
    Vec3::new(0.0, 0.0, 50.0),
    Vec3::new(11.0, 0.0, 49.5),
    Vec3::new(11.0, 7.0, 45.0),
    Vec3::new(14.0, 7.0, 38.0),
    Vec3::new(14.0, 7.0, 25.0),
    Vec3::new(12.0, 7.0, 22.0),
    Vec3::new(14.0, 7.0, 18.0),
    Vec3::new(14.0, 7.0, 3.0),
    Vec3::new(12.0, 7.0, 1.5),
    Vec3::new(10.0, 7.0, 0.0),
    Vec3::new(3.7, 7.0, 0.0),
    Vec3::new(3.9, 7.0, 3.5),
    Vec3::new(1.0, 12.0, 3.5),
    Vec3::new(-3.8, 7.0, 3.5),
    Vec3::new(-4.0, 0.0, 9.0),
    Vec3::new(-6.0, 0.0, 10.25),
    Vec3::new(-7.0, 0.0, 13.0),
    Vec3::new(-5.5, 0.0, 15.25),
    Vec3::new(-3.0, 0.0, 17.75),
    Vec3::new(3.25, 0.0, 19.0),
    Vec3::new(3.25, 0.0, 35.75),
    Vec3::new(0.0, 0.0, 46.0),
];

/// `validate_args` checks every flag before anything starts.
fn validate_args(args: &Args, assets_dir: &Path) -> Result<(), String> {
    let example: &str = "example: dark-forest --render-scale 0.8 --bot-count 4";
    if !(0.25..=2.0).contains(&args.render_scale) {
        return Err(format!(
            "--render-scale must be in 0.25..=2.0 (above 1 supersamples), got {}; {example}",
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
    if args
        .robot_pose
        .as_ref()
        .is_some_and(|pose| pose.len() != 4 || args.bot_count == 0)
    {
        return Err(format!(
            "--robot-pose takes x,y,z,yaw and needs --bot-count 1 or more; {example} --robot-pose 2,0,-30,0"
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

/// `MENU_DIM_FRAC` is how much the menu darkens the view behind it.
const MENU_DIM_FRAC: f32 = 0.75;
/// `MENU_HEIGHT_FRAC` is the most of the window's height the menu's text takes.
const MENU_HEIGHT_FRAC: f32 = 0.85;
/// `TITLE_PICTURE_PX` is the size of the title screen's text picture.
const TITLE_PICTURE_PX: (u32, u32) = (416, 168);
/// `TITLE_HEIGHT_FRAC` is the most of the window's height the title screen's text takes.
const TITLE_HEIGHT_FRAC: f32 = 0.5;
/// `TITLE_DIM_FRAC` is how much the title screen darkens the forest behind it.
const TITLE_DIM_FRAC: f32 = 0.45;
/// `DEATH_PICTURE_PX` and `BANNER_HEIGHT_FRAC` are the Lunar Surface demo's "YOU DIED" banner: its
/// text picture's size and the most of the window's height it takes.
const DEATH_PICTURE_PX: (u32, u32) = (240, 24);
const BANNER_HEIGHT_FRAC: f32 = 0.2;
/// `TITLE_TURN_RAD_PER_S` is how fast the view behind the title screen turns.
const TITLE_TURN_RAD_PER_S: f32 = 0.05;
const TEAM_MARK_RGBA: [[u8; 4]; 2] = [[80, 140, 255, 255], [240, 80, 60, 255]];
const ROBOT_MARK_RGBA: [u8; 4] = [255, 170, 40, 255];
const OFF_CONSOLE_MARK_RGBA: [u8; 4] = [130, 130, 130, 255];
const PLAYER_MARK_PX: f32 = 3.0;
const CONSOLE_MARK_PX: f32 = 4.5;

/// `fill_marks` replaces `marks` with what the minimap and map show: each console (grey once
/// off), each robot alone and each other player online in their team's colour.
///
/// Example: `fill_marks(&session, &mut marks);`
fn fill_marks(session: &Session, marks: &mut Vec<Mark>) {
    marks.clear();
    for (i, console) in session.arena.consoles.iter().enumerate() {
        let is_off: bool = session.console_off.get(i).copied().unwrap_or(false);
        marks.push(Mark {
            position_m: console.centre_m,
            rgba: if is_off {
                OFF_CONSOLE_MARK_RGBA
            } else {
                TEAM_MARK_RGBA[console.team as usize]
            },
            radius_px: CONSOLE_MARK_PX,
        });
    }
    for bot in session.bots.iter().filter(|bot| bot.health > 0.0) {
        marks.push(Mark {
            position_m: bot.feet_m,
            rgba: ROBOT_MARK_RGBA,
            radius_px: PLAYER_MARK_PX,
        });
    }
    for remote in &session.remote_bots {
        marks.push(Mark {
            position_m: remote.bot.feet_m,
            rgba: TEAM_MARK_RGBA[remote.team as usize % TEAM_MARK_RGBA.len()],
            radius_px: PLAYER_MARK_PX,
        });
    }
}

/// `run_title_screen` shows the title screen over a slowly turning view of the forest until a
/// choice is made with Return, or `None` if the window closes or Escape is pressed.
///
/// Example: `let choice = run_title_screen(&mut platform, &mut renderer, &mut event_pump, &scene, &map_file, &settings);`
fn run_title_screen(
    platform: &mut Platform,
    renderer: &mut MetalRenderer,
    event_pump: &mut sdl3::EventPump,
    scene: &Scene,
    map_file: &map::MapFile,
    settings: &RenderSettings,
) -> Option<title::Choice> {
    let mut title_screen: title::TitleScreen = title::TitleScreen::New();
    let mut picture: TextPicture = TextPicture::New(TITLE_PICTURE_PX.0, TITLE_PICTURE_PX.1);
    let mut preview: Session = Session::new(Arena::from_map(map_file), 0, false);
    let (feet_m, yaw_rad, pitch_rad) = CameraView::Lookout.pose();
    let start: Instant = Instant::now();
    let mut size_px: (u32, u32) = platform.window.size_in_pixels();
    loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => return None,
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } => match scancode {
                    Scancode::Escape => return None,
                    Scancode::Up | Scancode::W => title_screen.move_selection(-1),
                    Scancode::Down | Scancode::S => title_screen.move_selection(1),
                    Scancode::Return | Scancode::KpEnter | Scancode::Space => {
                        return Some(title_screen.get_selected());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let new_size_px: (u32, u32) = platform.window.size_in_pixels();
        if new_size_px != size_px && new_size_px.0 > 0 && new_size_px.1 > 0 {
            size_px = new_size_px;
            renderer.resize(size_px.0, size_px.1);
        }
        let (width_px, height_px) = (size_px.0 as f32, size_px.1 as f32);
        preview.player = Player::new(
            feet_m,
            yaw_rad + start.elapsed().as_secs_f32() * TITLE_TURN_RAD_PER_S,
        );
        preview.player.pitch_rad = pitch_rad;
        picture.draw(&title_screen.get_lines(), true);
        let mut frame: FrameData = preview.frame_data(scene, width_px, height_px);
        frame.viewmodel_draws.clear();
        frame.hud_rects.clear();
        frame.hud_rects.push(HudRect {
            x_px: 0.0,
            y_px: 0.0,
            width_px,
            height_px,
            color_rgba: Vec4::new(0.0, 0.0, 0.0, TITLE_DIM_FRAC),
        });
        renderer.render_frame(
            &FrameDescription {
                camera: frame.camera,
                world_draws: &frame.world_draws,
                static_draw_count: frame.lens_draws.start,
                viewmodel_draws: &frame.viewmodel_draws,
                joint_matrices: &frame.joint_matrices,
                point_lights: &frame.point_lights,
                decals: &frame.decals,
                world_particles: &frame.world_particles,
                viewmodel_particles: &frame.viewmodel_particles,
                hud_rects: &frame.hud_rects,
                hud_images: &[picture.get_image(width_px, height_px, TITLE_HEIGHT_FRAC)],
                time_s: start.elapsed().as_secs_f32(),
                hit_effect: HitEffect::default(),
                lens_draws: &frame.world_draws[frame.lens_draws.clone()],
                lens_particles: &frame.lens_particles,
                beams: &frame.beams,
                beam_speckle_shares: frame.beam_speckle_shares,
                overlay_rects: &[],
            },
            settings,
        );
    }
}

/// `FrameStats` collects frame times for reports.
#[derive(Default)]
struct FrameStats {
    frame_ms: Vec<f32>,
    gpu_ms: Vec<f32>,
    phase_ms: Vec<[f32; PHASE_NAMES.len()]>,
}

/// `PHASE_NAMES` names the parts of a frame's CPU time that `FrameStats` reports.
const PHASE_NAMES: [&str; 5] = ["sim", "build", "slot wait", "drawable wait", "render"];
/// `LATE_FRAME_MS` is the frame time above which a frame missed a 60 Hz display refresh.
const LATE_FRAME_MS: f32 = 17.5;

impl FrameStats {
    /// `summary` returns mean, median, 99th percentile frame time and median, mean and 99th
    /// percentile GPU time.
    fn summary(&self) -> String {
        if self.frame_ms.is_empty() {
            return "no frames".to_string();
        }
        let mut sorted: Vec<f32> = self.frame_ms.clone();
        sorted.sort_by(f32::total_cmp);
        let percentile = |frac: f32| sorted[((sorted.len() - 1) as f32 * frac) as usize];
        let mean_ms: f32 = sorted.iter().sum::<f32>() / sorted.len() as f32;
        let gpu_mean_ms: f32 = self.gpu_ms.iter().sum::<f32>() / self.gpu_ms.len().max(1) as f32;
        let late_count: usize = sorted.iter().filter(|&&ms| ms > LATE_FRAME_MS).count();
        let phase_p99s: Vec<String> = PHASE_NAMES
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut temp_phase_ms: Vec<f32> = self.phase_ms.iter().map(|p| p[i]).collect();
                temp_phase_ms.sort_by(f32::total_cmp);
                let p99_ms: f32 = temp_phase_ms
                    .get(((temp_phase_ms.len().max(1) - 1) as f32 * 0.99) as usize)
                    .copied()
                    .unwrap_or(0.0);
                format!("{name} {p99_ms:.2}")
            })
            .collect();
        let mut temp_gpu_ms: Vec<f32> = self.gpu_ms.clone();
        temp_gpu_ms.sort_by(f32::total_cmp);
        format!(
            "{} frames: mean {:.2} ms ({:.0} fps), median {:.2} ms, p99 {:.2} ms, worst {:.2} ms, {} over {LATE_FRAME_MS} ms, GPU median {:.2} ms, GPU mean {:.2} ms, GPU p99 {:.2} ms; p99 ms of {}",
            sorted.len(),
            mean_ms,
            1000.0 / mean_ms,
            percentile(0.5),
            percentile(0.99),
            sorted[sorted.len() - 1],
            late_count,
            temp_gpu_ms[(temp_gpu_ms.len() - 1) / 2],
            gpu_mean_ms,
            temp_gpu_ms[((temp_gpu_ms.len() - 1) as f32 * 0.99) as usize],
            phase_p99s.join(", ")
        )
    }
}

fn main() {
    let mut args: Args = Args::parse();
    if let Some(path) = &args.map_picture {
        args.photo = Some(path.clone());
        args.hide_canopy = true;
        args.no_title = true;
        args.bot_count = 0;
        args.shadow_distance_m = MAP_PICTURE_SHADOW_DISTANCE_M;
    }
    let map_path: PathBuf = args.map.clone().unwrap_or_else(map::default_map_path);
    if let Some(path) = &args.export_map {
        let json: String = serde_json::to_string(&forest_layout::build()).expect("map JSON");
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
    if let Err(message) = validate_args(&args, &assets_dir) {
        eprintln!("error: {message}");
        std::process::exit(2);
    }

    let mut platform: Platform = Platform::new(
        "Dark Forest",
        args.width_pt,
        args.height_pt,
        args.fullscreen,
        args.bot,
    );
    let mut renderer: MetalRenderer = MetalRenderer::new(platform.metal_layer());
    println!("GPU: {}", renderer.device_name());
    let (mut width_px, mut height_px) = platform.window.size_in_pixels();
    renderer.resize(width_px, height_px);
    let audio: Audio = Audio::new(&platform.sdl);
    let arena: Arena = Arena::from_map(&map_file);
    let mut scene: Scene = Scene::load(&mut renderer, &assets_dir, &textures_dir, &arena);
    if args.hide_canopy {
        let surfaces: Vec<game::world::Surface> = std::mem::take(&mut scene.static_surfaces);
        let draws: Vec<render::DrawItem> = std::mem::take(&mut scene.static_draws);
        (scene.static_draws, scene.static_surfaces) = draws
            .into_iter()
            .zip(surfaces)
            .filter(|(_, surface)| *surface != game::world::Surface::Leaves)
            .unzip();
    }
    let mut event_pump = platform.sdl.event_pump().expect("SDL event pump failed");
    let mut settings: RenderSettings = RenderSettings {
        render_scale_frac: args.render_scale,
        is_ssao_enabled: !args.disable_effect.contains(&Effect::Ssao),
        is_bloom_enabled: !args.disable_effect.contains(&Effect::Bloom),
        is_fxaa_enabled: !args.disable_effect.contains(&Effect::Fxaa),
        is_color_grade_enabled: !args.disable_effect.contains(&Effect::ColorGrade),
        is_volumetric_haze_enabled: !args.disable_effect.contains(&Effect::VolumetricHaze),
        is_material_response_enabled: !args.disable_effect.contains(&Effect::MaterialResponse),
        shadow_distance_m: args.shadow_distance_m,
        is_vsync_enabled: !args.no_vsync,
    };
    let is_title_shown: bool = !(args.no_title
        || args.host
        || args.join.is_some()
        || args.bot
        || args.autopilot
        || args.play_route
        || args.bench_views.is_some()
        || args.camera_view.is_some()
        || args.photo.is_some()
        || args.video_frames.is_some());
    if is_title_shown {
        match run_title_screen(
            &mut platform,
            &mut renderer,
            &mut event_pump,
            &scene,
            &map_file,
            &settings,
        ) {
            None | Some(title::Choice::Quit) => return,
            Some(title::Choice::PlayAlone) => {}
            Some(title::Choice::Host) => args.host = true,
            Some(title::Choice::Join) => args.join = Some("auto".to_string()),
        }
    }
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
    session.console_places = console_reach::find_console_places(&map_file);
    session.is_team_known = !is_online;
    if args.bot || (args.autopilot && is_online) {
        session.walker = Some(BotWalker::New());
        session.is_console_objective = args.bot_objective.is_some();
    }
    if args.photo.is_some() || args.video_frames.is_some() {
        session.guns = Guns::New(ShotNoise::FromSeed(RECORDING_SHOT_NOISE_SEED));
    }
    if args.rifle {
        session.guns.switch_gun(Gun::Rifle);
    }
    if !session.arena.warps.mouths.is_empty() && !args.bot {
        let capture_start: Instant = Instant::now();
        let lens: render::WormholeLens = capture_wormhole_lens(
            &mut renderer,
            &session,
            &scene,
            &settings,
            width_px,
            height_px,
        );
        renderer.set_wormhole_lens(Some(&lens));
        println!(
            "lens: drew {} wormhole mouths in {:.1} s",
            lens.mouths.len(),
            capture_start.elapsed().as_secs_f32()
        );
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
    let mut minimap: Minimap = Minimap::New(&session.arena);
    minimap.load_photo(&Path::new(env!("CARGO_MANIFEST_DIR")).join(MAP_PICTURE_PATH));
    let mut marks: Vec<Mark> = Vec::new();
    let mut menu: Option<Menu> = None;
    let mut exit_prompt: ExitPrompt = ExitPrompt::New();
    if args.photo_hud {
        renderer.set_is_capturing_hud(true);
    }
    let mut death_picture: TextPicture =
        TextPicture::Banner(DEATH_PICTURE_PX.0, DEATH_PICTURE_PX.1);
    let mut menu_picture: TextPicture =
        TextPicture::New(menu::MENU_PICTURE_PX.0, menu::MENU_PICTURE_PX.1);
    let mut menu_settings: MenuSettings = MenuSettings {
        look_rad_per_pt: MOUSE_RAD_PER_PT,
        render_scale_frac: settings.render_scale_frac,
        is_vsync_enabled: settings.is_vsync_enabled,
        is_showing_frame_time: args.show_frame_time,
        is_showing_minimap: true,
    };
    let mut last_title: Instant = Instant::now();
    let mut view_stats: Vec<FrameStats> = CameraView::ALL
        .iter()
        .map(|_| FrameStats::default())
        .collect();

    let mouse = platform.sdl.mouse();
    let mut is_mouse_captured: bool = true;
    mouse.set_relative_mouse_mode(&platform.window, true);
    let start: Instant = Instant::now();
    let mut last_frame: Instant = Instant::now();
    let mut stats: FrameStats = FrameStats::default();
    let mut report_stats: FrameStats = FrameStats::default();
    let mut last_report_s: f32 = 0.0;
    let mut smoothed_frame_ms: f32 = 8.3;
    let mut timing: FrameTiming = FrameTiming::default();
    let mut frame_trace: FrameTrace = FrameTrace::default();
    let mut frame_clock: FrameClock = FrameClock::default();
    renderer.set_is_tracing_frames(args.frame_times_file.is_some());
    if args.photo_map_page {
        let mut opened: Menu = Menu::New();
        for _ in 0..menu::PAGE_COUNT {
            if opened.get_page() == menu::Page::Map {
                break;
            }
            let _outcome: MenuOutcome = opened.handle(Scancode::E, &mut menu_settings);
        }
        menu = Some(opened);
    }
    'running: loop {
        let mut look_delta_pt: Vec2 = Vec2::ZERO;
        let mut is_jump_pressed: bool = false;
        let mut is_shoot_pressed: bool = false;
        let mut is_swap_pressed: bool = false;
        let mut picked_gun: Option<Gun> = None;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } if exit_prompt.is_shown() => {
                    if exit_prompt.answer(scancode) == ExitOutcome::Exit {
                        break 'running;
                    }
                }
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } if menu.is_some() => {
                    match menu
                        .as_mut()
                        .expect("the menu is open")
                        .handle(scancode, &mut menu_settings)
                    {
                        MenuOutcome::Stay => {}
                        MenuOutcome::Close => {
                            menu = None;
                            is_mouse_captured = true;
                            mouse.set_relative_mouse_mode(&platform.window, true);
                        }
                        MenuOutcome::Quit => break 'running,
                    }
                    settings.render_scale_frac = menu_settings.render_scale_frac;
                    settings.is_vsync_enabled = menu_settings.is_vsync_enabled;
                }
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } => match scancode {
                    Scancode::Escape => {
                        if exit_prompt.press_release_mouse(is_mouse_captured)
                            == ExitOutcome::ReleaseMouse
                        {
                            is_mouse_captured = false;
                            mouse.set_relative_mouse_mode(&platform.window, false);
                        }
                    }
                    Scancode::Tab => {
                        menu = Some(Menu::New());
                        menu_settings.render_scale_frac = settings.render_scale_frac;
                        is_mouse_captured = false;
                        mouse.set_relative_mouse_mode(&platform.window, false);
                    }
                    Scancode::Space => is_jump_pressed = true,
                    Scancode::_1 => picked_gun = Some(Gun::Laser),
                    Scancode::_2 => picked_gun = Some(Gun::Rifle),
                    Scancode::F1 => settings.is_ssao_enabled = !settings.is_ssao_enabled,
                    Scancode::F2 => settings.is_bloom_enabled = !settings.is_bloom_enabled,
                    Scancode::F3 => settings.is_fxaa_enabled = !settings.is_fxaa_enabled,
                    Scancode::F4 => {
                        menu_settings.is_showing_frame_time = !menu_settings.is_showing_frame_time
                    }
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
                Event::MouseButtonDown { .. }
                    if !is_mouse_captured && menu.is_none() && !exit_prompt.is_shown() =>
                {
                    is_mouse_captured = true;
                    mouse.set_relative_mouse_mode(&platform.window, true);
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } if is_mouse_captured => is_shoot_pressed = true,
                Event::MouseWheel { y, .. } if is_mouse_captured && y != 0.0 => {
                    is_swap_pressed = true;
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
        let loop_start_s: f64 = objc2_quartz_core::CACurrentMediaTime();
        let frame_s: f32 = (now - last_frame).as_secs_f32();
        last_frame = now;
        let is_recording: bool = args.photo.is_some() || args.video_frames.is_some();
        let refresh_interval_s: Option<f32> = settings
            .is_vsync_enabled
            .then(|| {
                platform
                    .window
                    .get_display()
                    .and_then(|display| display.get_mode())
            })
            .and_then(Result::ok)
            .filter(|mode| mode.refresh_rate > 0.0)
            .map(|mode| mode.refresh_rate.recip());
        let dt_s: f32 = if is_recording {
            RECORDING_DT_S
        } else {
            renderer.set_refresh_interval_s(refresh_interval_s);
            frame_clock.step_s(frame_s.min(MAX_FRAME_S), renderer.get_frame_interval_s())
        };
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
                look_delta_rad: look_delta_pt * menu_settings.look_rad_per_pt,
                is_sprint_held: keys.is_scancode_pressed(Scancode::LShift),
                is_crouch_held: keys.is_scancode_pressed(Scancode::LCtrl)
                    || keys.is_scancode_pressed(Scancode::C),
                is_jump_pressed,
            },
            gun: GunInput {
                is_trigger_held: is_mouse_captured && mouse_state.left(),
                is_shoot_pressed,
                is_aim_held: is_mouse_captured && mouse_state.right(),
                picked_gun,
                is_swap_pressed,
            },
            is_flashlight_held: args.hold_flashlight
                || (is_mouse_captured && keys.is_scancode_pressed(Scancode::F)),
        };
        let controls: Controls = if menu.is_some() || exit_prompt.is_shown() {
            Controls::default()
        } else {
            controls
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
        if let Some(pose) = &args.camera_pose {
            assert_eq!(pose.len(), 5, "--camera-pose takes x,y,z,yaw,pitch");
            hold_player(&mut session, Vec3::new(pose[0], pose[1], pose[2]), pose[3]);
            session.player.pitch_rad = pose[4];
            session.player.is_on_ground = true;
            session.is_autopilot = false;
        }
        let robot_hold: Option<RobotHold> = match &args.robot_pose {
            Some(pose) => Some(RobotHold {
                feet_m: Vec3::new(pose[0], pose[1], pose[2]),
                yaw_rad: pose[3],
                speed_m_s: args.robot_walk_m_per_s,
            }),
            None => camera_view.and_then(CameraView::robot_hold),
        };
        if !session.bots.is_empty() {
            session.set_robot_hold(robot_hold);
        }
        let controls: Controls = match camera_view {
            Some(view) => {
                let (feet_m, yaw_rad, pitch_rad) = view.pose();
                hold_player(&mut session, feet_m, yaw_rad);
                session.player.pitch_rad = pitch_rad;
                session.player.is_on_ground = true;
                session.is_autopilot = false;
                let is_firing_view: bool = view == CameraView::Firing;
                Controls {
                    gun: GunInput {
                        is_trigger_held: is_firing_view,
                        is_shoot_pressed: is_firing_view,
                        ..GunInput::default()
                    },
                    ..Controls::default()
                }
            }
            None => controls,
        };
        let controls: Controls = if args.hold_trigger {
            Controls {
                gun: GunInput {
                    is_trigger_held: true,
                    is_shoot_pressed: true,
                    ..controls.gun
                },
                ..controls
            }
        } else {
            controls
        };
        let sim_start: Instant = Instant::now();
        let waypoint_before: usize = session.autopilot_waypoint;
        if let Some(period_s) = args.hurt_player_every_s
            && (session.time_s / period_s).floor() != ((session.time_s + dt_s) / period_s).floor()
        {
            session.player.take_damage(HURT_PLAYER_DAMAGE);
        }
        session.update(&controls, &scene, &audio, dt_s);
        if args.play_route && session.autopilot_waypoint != waypoint_before {
            println!(
                "route: reached waypoint {waypoint_before} at {:.1?} after {:.1} s",
                session.player.feet_m,
                start.elapsed().as_secs_f32()
            );
        }
        let is_e_held: bool =
            menu.is_none() && (args.hold_console || keys.is_scancode_pressed(Scancode::E));
        if let Err(error) = play.update(&mut session, &scene, is_e_held, dt_s) {
            eprintln!("network error: {error}");
        }
        if last_title.elapsed().as_secs_f32() > 0.5 {
            last_title = Instant::now();
            let _ = platform.window.set_title(&play.status(&session));
        }

        smoothed_frame_ms += (frame_s * 1000.0 - smoothed_frame_ms) * 0.05;
        session.show_frame_ms = menu_settings
            .is_showing_frame_time
            .then_some(smoothed_frame_ms);
        let sim_ms: f32 = sim_start.elapsed().as_secs_f32() * 1000.0;
        if args.bot {
            if args
                .exit_after_s
                .is_some_and(|seconds| start.elapsed().as_secs_f32() > seconds)
            {
                break 'running;
            }
            std::thread::sleep(Duration::from_secs_f32(
                (BOT_STEP_S - now.elapsed().as_secs_f32()).max(0.0),
            ));
            continue;
        }
        let build_start: Instant = Instant::now();
        session.is_view_covered = menu.is_some() || !session.player.is_alive();
        let mut frame: FrameData = session.frame_data(&scene, width_px as f32, height_px as f32);
        if let Some(fov_deg) = args.fov_deg {
            frame.camera.fov_y_rad = fov_deg.to_radians();
            frame.camera.far_m = f32::MAX;
        }
        if args.map_picture.is_some() {
            let (low_m, high_m) = (session.arena.bounds_min_m, session.arena.bounds_max_m);
            let centre_m: Vec3 = (low_m + high_m) * 0.5;
            let eye_m: Vec3 = Vec3::new(centre_m.x, MAP_PICTURE_HEIGHT_M, centre_m.z);
            frame.camera.view = glam::Mat4::look_to_rh(eye_m, Vec3::NEG_Y, Vec3::NEG_Z);
            frame.camera.position_m = eye_m;
            frame.camera.fov_y_rad =
                2.0 * ((high_m.z - low_m.z) * 0.5 / MAP_PICTURE_HEIGHT_M).atan();
            frame.camera.far_m = f32::MAX;
            renderer.set_is_capturing(true);
        }
        if is_recording && !args.photo_hud {
            frame.viewmodel_draws.clear();
            frame.viewmodel_particles.clear();
        }
        let (window_width_px, window_height_px) = (width_px as f32, height_px as f32);
        fill_marks(&session, &mut marks);
        let mut hud_images: Vec<HudImage> = Vec::new();
        if let Some(open_menu) = &menu {
            let scoreboard: Scoreboard = match &play.client {
                None => Scoreboard::Alone {
                    kill_count: session.kill_count,
                    death_count: session.death_count,
                },
                Some(client) => match &client.scores {
                    None => Scoreboard::Waiting,
                    Some(scores) => Scoreboard::Scores {
                        scores,
                        own_id: client.welcomed_id,
                    },
                },
            };
            menu_picture.draw(&open_menu.get_lines(&menu_settings, scoreboard), false);
            frame.hud_rects.push(HudRect {
                x_px: 0.0,
                y_px: 0.0,
                width_px: window_width_px,
                height_px: window_height_px,
                color_rgba: Vec4::new(0.0, 0.0, 0.0, MENU_DIM_FRAC),
            });
            hud_images.push(menu_picture.get_image(
                window_width_px,
                window_height_px,
                MENU_HEIGHT_FRAC,
            ));
            if open_menu.get_page() == menu::Page::Map {
                minimap.draw_map(session.player.feet_m, session.player.yaw_rad, &marks);
                hud_images.push(minimap.get_map_image(window_width_px, window_height_px));
            }
        } else if menu_settings.is_showing_minimap && bench_view.is_none() && !args.play_route {
            minimap.draw_minimap(session.player.feet_m, session.player.yaw_rad, &marks);
            hud_images.push(minimap.get_minimap_image(window_width_px, window_height_px));
        }
        if exit_prompt.is_shown() {
            let [question, keys] = exit_prompt::get_lines();
            death_picture.draw(&[(question, true), (keys, false)], true);
            hud_images.push(death_picture.get_image(
                window_width_px,
                window_height_px,
                BANNER_HEIGHT_FRAC,
            ));
        } else if menu.is_none()
            && let Some(respawn_in_s) = session
                .player
                .get_respawn_in_s(session.settings.respawn.delay_s as f32)
        {
            death_picture.draw(
                &[
                    ("YOU DIED".to_string(), true),
                    (
                        format!("Respawning in {:.0}", respawn_in_s.ceil().max(1.0)),
                        false,
                    ),
                ],
                true,
            );
            hud_images.push(death_picture.get_image(
                window_width_px,
                window_height_px,
                BANNER_HEIGHT_FRAC,
            ));
        }
        let build_ms: f32 = build_start.elapsed().as_secs_f32() * 1000.0;
        timing = renderer.render_frame(
            &FrameDescription {
                camera: frame.camera,
                world_draws: &frame.world_draws,
                static_draw_count: frame.lens_draws.start,
                viewmodel_draws: &frame.viewmodel_draws,
                joint_matrices: &frame.joint_matrices,
                point_lights: &frame.point_lights,
                decals: &frame.decals,
                world_particles: &frame.world_particles,
                viewmodel_particles: &frame.viewmodel_particles,
                hud_rects: &frame.hud_rects,
                hud_images: &hud_images,
                time_s: session.time_s,
                hit_effect: frame.hit_effect,
                lens_draws: &frame.world_draws[frame.lens_draws.clone()],
                lens_particles: &frame.lens_particles,
                beams: &frame.beams,
                beam_speckle_shares: frame.beam_speckle_shares,
                overlay_rects: &frame.overlay_rects,
            },
            &settings,
        );

        let phase_ms: [f32; PHASE_NAMES.len()] = [
            sim_ms,
            build_ms,
            timing.slot_wait_ms as f32,
            timing.drawable_wait_ms as f32,
            timing.cpu_render_ms as f32,
        ];
        frame_trace.push(CpuFrameRow {
            frame_index: timing.frame_index,
            loop_start_s,
            frame_ms: frame_s * 1000.0,
            dt_ms: dt_s * 1000.0,
            sim_ms,
            build_ms,
            slot_wait_ms: phase_ms[2],
            drawable_wait_ms: phase_ms[3],
            render_ms: phase_ms[4],
        });
        let elapsed_s: f32 = start.elapsed().as_secs_f32();
        if let Some(directory) = &args.video_frames {
            if session.time_s > args.video_seconds {
                break 'running;
            }
            let frame_path: PathBuf = directory.join(format!(
                "frame_{:04}.png",
                (session.time_s / RECORDING_DT_S).round() as u32
            ));
            save_photo(
                &mut renderer,
                &frame_path,
                args.photo_width_px,
                args.photo_hud,
            );
        }
        if let Some(path) = &args.photo
            && session.time_s > args.photo_at_s
        {
            if args.map_picture.is_some() {
                save_map_picture(&mut renderer, path, &session.arena);
            } else {
                save_photo(&mut renderer, path, args.photo_width_px, args.photo_hud);
            }
            break 'running;
        }
        if stats.frame_ms.len() > 30 || elapsed_s > 2.0 {
            stats.frame_ms.push(frame_s * 1000.0);
            stats.gpu_ms.push(timing.gpu_ms as f32);
            stats.phase_ms.push(phase_ms);
            report_stats.frame_ms.push(frame_s * 1000.0);
            report_stats.gpu_ms.push(timing.gpu_ms as f32);
            report_stats.phase_ms.push(phase_ms);
            if let Some(index) = bench_view {
                let seconds: f32 = args.bench_views.unwrap_or(1.0);
                if start.elapsed().as_secs_f32() - index as f32 * seconds > 1.0 {
                    view_stats[index].frame_ms.push(frame_s * 1000.0);
                    view_stats[index].gpu_ms.push(timing.gpu_ms as f32);
                    view_stats[index].phase_ms.push(phase_ms);
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
    if let Some(path) = &args.frame_times_file {
        let completed = renderer.take_completed_frames();
        match frame_trace.write(path, &completed) {
            Ok(summary) => println!("frame trace: {summary}; wrote {}", path.display()),
            Err(error) => eprintln!("error: can't write {}: {error}", path.display()),
        }
    }
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
