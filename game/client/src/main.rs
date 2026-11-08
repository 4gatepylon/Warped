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
//! - `--join auto|<address>`, `--bot`, `--port`: play on a game server (`warped-server`, see
//!   `net.rs` and `multiplayer.rs`).
//! - `--black-hole-strength <0..1>`: on lunar-surface, force the black hole's strength (W-68).
//!
//! Example: `cargo run --release -- --bench-views --width-px 2560 --height-px 1600`
//!
//! Must know:
//! - Gotchas: run folders are found from the crate folder, not the current directory.

mod avatar_animation;
mod bench;
mod bot;
mod cli;
mod credits;
mod dust;
mod exit_prompt;
mod hud_bars;
mod key_bindings;
mod map_layout;
mod menu;
mod minimap;
mod multiplayer;
mod play;
mod player_settings;
mod render_mesh;
mod render_metal;
mod render_voxel;
mod renderer;
mod run_folder;
mod server_process;
mod session;
mod spaceships;
mod third_person;
mod title;
mod title_choice;
mod viewmodel;
mod voxel_textures;

use warped_core::{
    app_bundle, avatar_model, black_hole_growth, fps_demo_assets, geometry, map, mesh, net, player,
    settings, shapes, shot_noise, terrain, thrown_wormholes, voxel_clearance, voxel_world,
    zone_walls,
};

use std::error::Error;
use std::process::ExitCode;

use clap::Parser;

use crate::bot::run_bot;
use crate::cli::Arguments;
use crate::cli::RendererKind;
use crate::cli::validate_arguments;
use crate::renderer::Renderer;
use crate::renderer::make_renderer;
use crate::title_choice::run_title_screen;
use crate::title_choice::show_alert;
use bench::bench_views;
use map::MapName;
use play::play;

/// `Result` is this program's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;
const WINDOW_TITLE: &str = "Hyperbolic wormholes (Rust)";
const MS_PER_S: f64 = 1000.0;
const ENERGY_SEGMENT_COUNT: u32 = 10;
const DEATH_MASK_PX: (u32, u32) = (240, 24);
const MENU_MASK_PX: (u32, u32) = (512, 312);
const BANNER_HEIGHT_FRAC: f32 = 0.2;
const MENU_HEIGHT_FRAC: f32 = 0.85;
const WRONG_PASSWORD_EXIT_CODE: u8 = 3;
const DISABLE_BAR_MIN_FRAC: [f32; 2] = [0.4, 0.62];
const DISABLE_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.02];
const DISABLE_BAR_RGB: [f32; 3] = [1.0, 0.85, 0.3];
const MIN_IMAGE_SIDE_PX: u32 = 16;
const JOIN_AUTO: &str = "auto";

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

/// `run` validates the command line and runs what it asks for: the window, the bot or the
/// benchmark.
///
/// Example: `run(Arguments::parse())?;`
fn run(arguments: Arguments) -> Result<()> {
    validate_arguments(&arguments)?;
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
