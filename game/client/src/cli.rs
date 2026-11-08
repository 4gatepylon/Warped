//! `cli.rs` is the command line: which window, server, bot or tool to run, with what map, renderer,
//! size and password, and the checks that a command line makes sense.
//!
//! Interface: `Arguments`, `validate_arguments`, `RendererKind`, `Sampling`, `RouteKind`,
//! `get_sampling_name`, `get_default_render_scale`.
//!
//! Example: `let arguments: Arguments = Arguments::parse(); validate_arguments(&arguments)?;`

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::map::MapName;

use crate::server_process::parse_server_address;
use crate::{JOIN_AUTO, MIN_IMAGE_SIDE_PX, Result, net};

const METAL_RENDER_SCALE: f64 = 1.0;
const LOCAL_PLAY_OFF: &str = "Playing is only on a game server now: start one with `make serve-game` (--serve) and join it with `make join-game SERVER=<address> PORT=<port>` (--join <address>). Without a server, only the tools run: --bench-views and --play-route.";
const HOUSE_IN_DEMO: &str = "The house demo plays only in demos/hyperbolic_wormholes_rust/ now (make -C demos/hyperbolic_wormholes_rust join-game MAP=house-demo); the game plays lunar-surface.";
const MAX_IMAGE_SIDE_PX: u32 = 8192;

/// `RendererKind` is which renderer draws the frames.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RendererKind {
    Metal,
}

/// `Sampling` is which pixels Metal traces: every other pixel each way, interpolating the rest
/// where that is exact enough, or every pixel.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Sampling {
    Adaptive,
    EveryPixel,
}

/// `RouteKind` is which route `--play-route` plays: a tour around each warp object.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RouteKind {
    WarpTour,
}

/// `Arguments` is the command line.
#[derive(Parser)]
#[command(
    about = "Walk, fly and fire a laser through wormholes in hyperbolic space.",
    after_help = "Example: hyperbolic-wormholes --renderer metal --bench-views --width-px 2560 --height-px 1600"
)]
pub(crate) struct Arguments {
    /// Which renderer draws the frames: the GPU with Metal, or every CPU core.
    #[arg(long, value_enum, default_value_t = RendererKind::Metal)]
    pub(crate) renderer: RendererKind,
    /// Which pixels Metal traces: every other pixel each way, interpolating the others where the
    /// image is smooth enough (adaptive), or every pixel (every-pixel).
    #[arg(long, value_enum, default_value_t = Sampling::Adaptive)]
    pub(crate) sampling: Sampling,
    /// Fraction of the output's pixels per side to ray-trace, upscaled with MetalFX (Metal) or by
    /// repeating pixels (CPU). In the window it defaults to 0.35 on the CPU, and with Metal it
    /// starts at 1 and lowers itself (to no less than 0.5) when a frame takes more than about
    /// 14 ms of GPU time; otherwise 1.
    #[arg(long)]
    pub(crate) render_scale: Option<f64>,
    /// Render fixed views without a window, time them and save them under .artifacts/bench_views/.
    #[arg(long)]
    pub(crate) bench_views: bool,
    /// Image width for --bench-views, and the window's starting width (in screen
    /// points, which are pixels on a 1x display).
    #[arg(long, default_value_t = 1280)]
    pub(crate) width_px: u32,
    /// Image height for --bench-views, and the window's starting height.
    #[arg(long, default_value_t = 800)]
    pub(crate) height_px: u32,
    /// Timed frames per view for --bench-views.
    #[arg(long, default_value_t = 10)]
    pub(crate) frame_count: u32,
    /// Print results as one JSON object.
    #[arg(long)]
    pub(crate) json: bool,
    /// Suffix for the run folder's name (letters, digits, - and _).
    #[arg(long)]
    pub(crate) label: Option<String>,
    /// Close the window after this many seconds and print its frame rate (to check the window).
    #[arg(long)]
    pub(crate) exit_after_s: Option<f64>,
    /// Play the scripted route in the window, one 1/60 s step per frame, instead of taking input,
    /// then close it and print frame-time statistics (to check the window's frame rate in motion).
    #[arg(long, conflicts_with = "bench_views")]
    pub(crate) play_route: bool,
    /// Which route --play-route plays: a tour circling each warp object, from near to far and
    /// back, while the view swings around.
    #[arg(long, value_enum, default_value_t = RouteKind::WarpTour)]
    pub(crate) route: RouteKind,
    /// Host a game on the local network: start a game server as its own process (as --serve,
    /// without a window), play in it, and stop it when this window closes.
    #[arg(long, conflicts_with_all = ["join", "serve", "bench_views", "play_route"])]
    pub(crate) host: bool,
    /// Join a game on the local network: `auto` to find one, or a server's IP address (with an
    /// optional :port).
    #[arg(long, conflicts_with_all = ["serve", "bench_views", "play_route"])]
    pub(crate) join: Option<String>,
    /// Run only a game server, without a window, until --exit-after-s (or until stopped); it
    /// prints this computer's address and the command that joins it.
    #[arg(long, conflicts_with_all = ["bench_views", "play_route"])]
    pub(crate) serve: bool,
    /// With --join and --exit-after-s: play by itself without a window (walk, aim at the nearest
    /// player and fire), to test a game; saves its view under .artifacts/bot/ at each hit.
    #[arg(long)]
    pub(crate) bot: bool,
    /// UDP port the server listens on (--host, --serve), and --join's port when its address has none.
    #[arg(long, default_value_t = net::DEFAULT_PORT)]
    pub(crate) port: u16,
    /// The numbers that shape play, a YAML file with server_settings.yaml's keys (the default): a
    /// server (--serve, --host) sends them to every player who joins; without a server, this game
    /// uses them.
    #[arg(long, conflicts_with = "join")]
    pub(crate) server_settings: Option<PathBuf>,
    /// A file holding the game's password: a server (--serve, --host) admits only players who prove
    /// they know it, and a player (--join, --host) proves it. Without it the password is empty, as
    /// suits a local network.
    #[arg(long)]
    pub(crate) game_password_file: Option<PathBuf>,
    /// This player's own choices (keys, mouse speed, head bob, motion blur, how a hit looks), a YAML
    /// file with player_settings.yaml's keys (the default).
    #[arg(long)]
    pub(crate) player_settings: Option<PathBuf>,
    /// Which map to play, from common/maps/.
    #[arg(long, value_enum, default_value_t = MapName::LunarSurface)]
    pub(crate) map: MapName,
    /// With --map lunar-surface: start the window at one of the level's viewpoints (as
    /// --bench-views names them), e.g. black_hole.
    #[arg(long)]
    pub(crate) view: Option<String>,
    /// On a map whose black hole grows (lunar-surface): force the black hole's strength (0 to 1)
    /// instead of following the consoles (W-68).
    #[arg(long)]
    pub(crate) black_hole_strength: Option<f64>,
}

/// `validate_arguments` checks every argument before anything happens, naming the argument and
/// the valid range on failure.
///
/// Example: `validate_arguments(&arguments)?;`
pub(crate) fn validate_arguments(arguments: &Arguments) -> Result<()> {
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

/// `get_sampling_name` names which pixels the renderer traces, for results: the CPU always traces
/// every pixel.
///
/// Example: `let name = get_sampling_name(&arguments);`
pub(crate) fn get_sampling_name(arguments: &Arguments) -> &'static str {
    match (arguments.renderer, arguments.sampling) {
        (RendererKind::Metal, Sampling::Adaptive) => "adaptive",
        _ => "every-pixel",
    }
}

/// `get_default_render_scale` gives the render scale a renderer starts at.
///
/// Example: `let scale = get_default_render_scale(RendererKind::Metal);`
pub(crate) fn get_default_render_scale(kind: RendererKind) -> f64 {
    match kind {
        RendererKind::Metal => METAL_RENDER_SCALE,
    }
}
