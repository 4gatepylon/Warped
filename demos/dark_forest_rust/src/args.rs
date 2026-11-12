//! `args.rs` holds Dark Forest's command-line flags (`Args`), the fixed camera views of
//! `--camera-view` and `--bench-views` (`CameraView`), the effects `--disable-effect` turns off
//! (`Effect`), and the check of the flags before anything starts (`validate_args`). The program
//! (`main.rs`) parses them from its command line, and the one-window app's entry point
//! (`lib.rs`, `play_level`) builds them from the level's join, so both play the same way.
//!
//! Interface: `Args`, `CameraView` (`ALL`, `robot_hold`, `pose`), `Effect`, `validate_args`,
//! `PHOTO_SETTLE_S`.
//!
//! Example: `let args: Args = Args::parse(); validate_args(&args, &assets_dir)?;`

use std::f32::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use glam::Vec3;

use crate::asset_manifest;
use crate::forest_layout;
use crate::game::session::RobotHold;
use crate::{map, net};
use warped_shared::visuals::VisualsSettings;

/// `CameraView` is a fixed camera pose for screenshots and repeatable frame-time comparisons;
/// `--bench-views` runs them all. `Edge` stands at the playable rectangle's red end looking out,
/// and `PastEdge` 10 m into the slowing band past its east side looking further out (W-145, no
/// walls at map edges).
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum CameraView {
    Spawn,
    Bridge,
    Lookout,
    Treehouse,
    Firing,
    Gate,
    Lift,
    Edge,
    PastEdge,
}

impl CameraView {
    /// `ALL` is every view, in `--bench-views` order.
    pub const ALL: [CameraView; 9] = [
        CameraView::Spawn,
        CameraView::Bridge,
        CameraView::Lookout,
        CameraView::Treehouse,
        CameraView::Firing,
        CameraView::Gate,
        CameraView::Lift,
        CameraView::Edge,
        CameraView::PastEdge,
    ];

    /// `robot_hold` is where the view holds robot 0, if it has robots: just beyond the partner of
    /// the mouth that `Gate` and `Lift` look into, facing back through it.
    pub fn robot_hold(self) -> Option<RobotHold> {
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
    pub fn pose(self) -> (Vec3, f32, f32) {
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
            CameraView::Edge => (Vec3::new(0.0, 0.0, 55.0), PI, 0.05),
            CameraView::PastEdge => (Vec3::new(40.0, 0.0, 10.0), -FRAC_PI_2, 0.05),
        }
    }
}

/// `Effect` is a render effect that `--disable-effect` can turn off at start.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Effect {
    Ssao,
    Bloom,
    Fxaa,
    ColorGrade,
    VolumetricHaze,
    MaterialResponse,
    Foveation,
}

/// `Args` are the command-line flags.
#[derive(Parser, Clone, Debug)]
#[command(
    about = "Dark Forest, a Warped level (Rust + SDL3 + Metal)",
    after_help = "Examples:\n  dark-forest                     play alone against bots\n  dark-forest --host              serve and play; others --join auto\n  dark-forest --serve --level-result-file /tmp/r.json\n  dark-forest --join 127.0.0.1 --team blue --bot --bot-objective consoles --exit-after-s 60"
)]
pub struct Args {
    /// Internal render resolution as a fraction of the window's pixels (0.25 to 1.0).
    #[arg(long, default_value_t = 1.0)]
    pub render_scale: f32,
    /// The picture's brightness at the start, 0.5 to 2 (1 for none; the menu changes it, W-51).
    #[arg(long, default_value_t = 1.0)]
    pub brightness: f64,
    /// The picture's contrast at the start, 0.5 to 2 (1 for none).
    #[arg(long, default_value_t = 1.0)]
    pub contrast: f64,
    /// The picture's saturation at the start, 0 to 2 (1 for none, 0 for black and white).
    #[arg(long, default_value_t = 1.0)]
    pub saturation: f64,
    /// The picture's gamma at the start, 0.5 to 2 (1 for none, above 1 lifts the dark tones).
    #[arg(long, default_value_t = 1.0)]
    pub gamma: f64,
    /// Number of AI robots when playing alone (0 to 6; none online).
    #[arg(long, default_value_t = 3)]
    pub bot_count: usize,
    /// Window width in points.
    #[arg(long, default_value_t = 1600)]
    pub width_pt: u32,
    /// Window height in points.
    #[arg(long, default_value_t = 1000)]
    pub height_pt: u32,
    /// Start in fullscreen.
    #[arg(long)]
    pub fullscreen: bool,
    /// Render as fast as possible instead of syncing to the display.
    #[arg(long)]
    pub no_vsync: bool,
    /// Let the autopilot play (for demos, screenshots, and benchmarks).
    #[arg(long)]
    pub autopilot: bool,
    /// Quit after this many seconds and print frame-time statistics (0 runs until closed).
    #[arg(long, default_value_t = 0.0)]
    pub benchmark_seconds: f32,
    /// Show the frame time in the top-left corner.
    #[arg(long)]
    pub show_frame_time: bool,
    /// Write every frame's timing (CPU phases, GPU, when it was shown) to this CSV on exit.
    #[arg(long)]
    pub frame_times_file: Option<PathBuf>,
    /// Start with gun 2, the rifle (the energy revolver), in hand instead of gun 1, the sniper.
    #[arg(long)]
    pub rifle: bool,
    /// Hold the camera at a fixed view (no autopilot, no mouse look); `firing` holds the trigger
    /// (the rifle fires while it is held, the sniper each time it has reloaded).
    #[arg(long, value_enum, conflicts_with = "autopilot")]
    pub camera_view: Option<CameraView>,
    /// Turn an effect off at start (repeatable); the F keys still toggle it.
    #[arg(long, value_enum)]
    pub disable_effect: Vec<Effect>,
    /// Run every camera view for this many seconds each, print its frame times, and quit.
    #[arg(long)]
    pub bench_views: Option<f32>,
    /// Let the autopilot walk the vertical route (ladders, bridges, lookout) and print its laps.
    #[arg(long)]
    pub play_route: bool,
    /// Directory holding the robot, gun, sound and sky assets (shared with ../fps_demo).
    #[arg(long)]
    pub assets_dir: Option<PathBuf>,
    /// Directory holding the forest textures that `download-assets` saved.
    #[arg(long)]
    pub textures_dir: Option<PathBuf>,
    /// The map file (default: common/maps/dark_forest/map.json).
    #[arg(long)]
    pub map: Option<PathBuf>,
    /// Write the forest's map to this file and quit.
    #[arg(long)]
    pub export_map: Option<PathBuf>,
    /// Run a game server without a window until killed.
    #[arg(long, conflicts_with_all = ["join", "host"])]
    pub serve: bool,
    /// Run a game server in this process and join it.
    #[arg(long, conflicts_with = "join")]
    pub host: bool,
    /// Join the game server at this address (or `auto` to find one on the network).
    #[arg(long)]
    pub join: Option<String>,
    /// UDP port of the game server.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    pub port: u16,
    /// Join this team at once (`blue` or `red`); otherwise the server picks.
    #[arg(long, value_parser = ["blue", "red"])]
    pub team: Option<String>,
    /// Let a bot play this window, hidden and drawing nothing (for tests and matches): it hunts the
    /// other team's players, or turns off their consoles with --bot-objective consoles.
    #[arg(long)]
    pub bot: bool,
    /// What the bot does: `consoles` walks to the other team's consoles and holds E on each, in
    /// reach, until its team wins (logging `bot reached console <i> on foot in <t> s`).
    #[arg(long, value_parser = ["consoles"])]
    pub bot_objective: Option<String>,
    /// Quit after this many seconds.
    #[arg(long)]
    pub exit_after_s: Option<f32>,
    /// With --serve: write {"winner": "blue"|"red"} here on a win, and start no new game.
    #[arg(long)]
    pub level_result_file: Option<PathBuf>,
    /// With --serve or --host: the server settings file (default: the built-in server_settings.yaml).
    #[arg(long)]
    pub server_settings: Option<PathBuf>,
    /// A name the game server gives this window or bot; accepted and ignored.
    #[arg(long)]
    pub label: Option<String>,
    /// Start playing at once instead of on the main menu (flags such as --join already do).
    #[arg(long)]
    pub no_title: bool,
    /// Save one frame, without the HUD, to this PNG after the scene settles, then quit.
    #[arg(long)]
    pub photo: Option<PathBuf>,
    /// Downscale the --photo to this width (keeping its aspect), e.g. to supersample.
    #[arg(long)]
    pub photo_width_px: Option<u32>,
    /// Save every frame, without the HUD, as frame_NNNN.png in this folder (which must exist), at
    /// 30 simulated frames a second, for --video-seconds, then quit.
    #[arg(long)]
    pub video_frames: Option<PathBuf>,
    /// Keep the HUD in --photo and --video-frames pictures, as the window shows it.
    #[arg(long)]
    pub photo_hud: bool,
    /// Take --photo at this many simulated seconds instead of after 3 s.
    #[arg(long, default_value_t = PHOTO_SETTLE_S)]
    pub photo_at_s: f32,
    /// Hurt the player by one robot shot every this many simulated seconds, for pictures of hits
    /// and of "YOU DIED".
    #[arg(long)]
    pub hurt_player_every_s: Option<f32>,
    /// Hold `E` (turning off a console within reach), for pictures of the console bar.
    #[arg(long)]
    pub hold_console: bool,
    /// Hold `F` (the flashlight), for pictures of it.
    #[arg(long)]
    pub hold_flashlight: bool,
    /// Start in the third-person view (`V`), for pictures of it.
    #[arg(long)]
    pub third_person: bool,
    /// How many simulated seconds --video-frames records.
    #[arg(long, default_value_t = 8.0)]
    pub video_seconds: f32,
    /// Hold the camera here: feet x,y,z in meters, yaw and pitch in radians.
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    pub camera_pose: Option<Vec<f32>>,
    /// Vertical field of view in degrees, instead of the weapon's (narrow from far away looks
    /// almost flat, for map pictures).
    #[arg(long)]
    pub fov_deg: Option<f32>,
    /// How far from the camera the sun casts shadows, in meters.
    #[arg(long, default_value_t = 110.0)]
    pub shadow_distance_m: f32,
    /// Hold robot 0 here (it doesn't think or shoot): feet x,y,z in meters and yaw in radians.
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    pub robot_pose: Option<Vec<f32>>,
    /// With --robot-pose, the held robot walks straight ahead at this speed (through mouths) and
    /// starts again from its pose every 10 s.
    #[arg(long, default_value_t = 0.0)]
    pub robot_walk_m_per_s: f32,
    /// Hold the trigger (the sniper fires each time it has reloaded), for pictures of lasers.
    #[arg(long)]
    pub hold_trigger: bool,
    /// Leave out the tree canopies (for pictures from above).
    #[arg(long)]
    pub hide_canopy: bool,
    /// Open the menu on its map page (for --photo).
    #[arg(long)]
    pub photo_map_page: bool,
    /// Draw the whole forest from straight above (no canopies, robots, vignette or grain), crop it
    /// to the arena and save it to this PNG for the menu's map page, then quit.
    #[arg(long)]
    pub map_picture: Option<PathBuf>,
    /// Throw both wormhole mouths by script (for tests and pictures): the purple one turned to the
    /// first yaw (radians), the green one turned to the second, at the times of `THROW_SCRIPT_S`.
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    pub throw_yaws_rad: Option<Vec<f32>>,
    /// From this simulated time on, stop holding the player at --camera-pose and walk them into
    /// their purple thrown mouth.
    #[arg(long)]
    pub walk_into_thrown_at_s: Option<f32>,
    /// From this simulated time on, aim at the nearest thrown mouth (another player's, else this
    /// player's own) and hold the trigger.
    #[arg(long)]
    pub shoot_thrown_at_s: Option<f32>,
}

impl Args {
    /// `get_map_path` gives the map file the flags name (`--map`), else the forest's own.
    ///
    /// Example: `let map_path: PathBuf = args.get_map_path();`
    pub fn get_map_path(&self) -> PathBuf {
        self.map.clone().unwrap_or_else(map::default_map_path)
    }
}

/// `PHOTO_SETTLE_S` is how long --photo lets the robots, shadows and exposure settle, in simulated
/// seconds.
pub const PHOTO_SETTLE_S: f32 = 3.0;

/// `get_start_visuals` gives the picture's visuals from `--brightness`, `--contrast`,
/// `--saturation` and `--gamma` (W-51, basic visuals).
///
/// Example: `let visuals: VisualsSettings = get_start_visuals(&args);`
pub fn get_start_visuals(args: &Args) -> VisualsSettings {
    VisualsSettings {
        brightness: args.brightness,
        contrast: args.contrast,
        saturation: args.saturation,
        gamma: args.gamma,
    }
}

/// `validate_args` checks every flag before anything starts.
///
/// Example: `validate_args(&args, &assets_dir)?;`
pub fn validate_args(args: &Args, assets_dir: &Path) -> Result<(), String> {
    let example: &str = "example: dark-forest --render-scale 0.8 --bot-count 4";
    get_start_visuals(args).check("from the command line")?;
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
