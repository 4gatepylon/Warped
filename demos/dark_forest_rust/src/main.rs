//! `dark-forest` runs the Dark Forest level (`lib.rs`) alone in a window of its own: against AI
//! robots, or online against other players through a game server (`--serve`, `--host`,
//! `--join`). It opens the window and plays in it through the same `Level` the one-window app's
//! `play_level` uses.
//!
//! Example:
//!     cargo run --release --bin dark-forest -- --host
//!
//! Interface: the command-line flags of `args::Args`; controls are in the README.

use std::sync::atomic::AtomicBool;

use clap::Parser;
use warped_shared::app::LevelWindow;
use warped_shared::metal_window::{MetalWindow, NoMatchLink, WindowOptions};

use warped_dark_forest::args::Args;
use warped_dark_forest::level::{Level, LevelFiles};
use warped_dark_forest::main_menu::Choice;
use warped_dark_forest::{forest_layout, server};

/// `MAP_PICTURE_SHADOW_DISTANCE_M` lets the sun's shadows reach the forest from as high as
/// `--map-picture` looks down from.
const MAP_PICTURE_SHADOW_DISTANCE_M: f32 = 1000.0;

/// `exit_with_error` prints `message` as an error and quits with status 2, as a bad flag does.
///
/// Example: `let files = LevelFiles::Find(&args).unwrap_or_else(|message| exit_with_error(&message));`
fn exit_with_error(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2);
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
    if let Some(path) = &args.export_map {
        let json: String = serde_json::to_string(&forest_layout::build()).expect("map JSON");
        std::fs::write(path, json + "\n").expect("can't write the map");
        println!("wrote {}", path.display());
        return;
    }
    if args.serve {
        let options: server::ServerOptions = server::ServerOptions {
            port: args.port,
            map_path: args.get_map_path(),
            level_result_file: args.level_result_file.clone(),
            game_password: Vec::new(),
            server_settings: args.server_settings.clone(),
        };
        if let Err(error) = server::run(&options, &AtomicBool::new(false)) {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
        return;
    }
    let files: LevelFiles =
        LevelFiles::Find(&args).unwrap_or_else(|message| exit_with_error(&message));

    let mut metal_window: MetalWindow = MetalWindow::Open(
        "Dark Forest",
        (args.width_pt, args.height_pt),
        WindowOptions {
            is_fullscreen: args.fullscreen,
            is_hidden: args.bot,
        },
    )
    .unwrap_or_else(|error| exit_with_error(&format!("can't open the window: {error}")));
    let mut no_match_link: NoMatchLink = NoMatchLink;
    let mut window: LevelWindow = metal_window.get_level_window(&mut no_match_link);
    let mut level: Level = Level::Load(&window, &args, files);
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
        match level.run_main_menu(&mut window) {
            None | Some(Choice::Quit) => return,
            Some(Choice::PlayAlone) => {}
            Some(Choice::Host) => args.host = true,
            Some(Choice::Join) => args.join = Some("auto".to_string()),
        }
    }
    if let Err(message) = level.play(&mut window, &args, true) {
        exit_with_error(&message);
    }
}
