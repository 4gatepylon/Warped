//! `level.rs` plays Dark Forest in a window it is lent: `Level::Load` sets up the renderer, the
//! sounds and the forest's scene on the window's Metal layer; `Level::run_title_screen` shows the
//! title screen (the program run alone); `Level::play` joins the game (alone against robots, or
//! online) and runs the frame loop until the player leaves or closes the window, the caller's
//! `MatchLink` says the round is over, or a flag's run (a photo, a benchmark) ends. Dropping the
//! `Level` frees its GPU resources and gives the layer back as it found it.
//!
//! Interface: `LevelFiles` (`Find`), `Level` (`Load`, `run_title_screen`, `play`).
//!
//! Example:
//!     let files: LevelFiles = LevelFiles::Find(&args)?;
//!     let level: Level = Level::Load(&window, &args, files);
//!     let end: Option<LevelEnd> = level.play(&mut window, &args, true)?;
//!
//! Must know:
//! - Gotchas: the frame loop's cadence (`render::frame_clock`, DECISIONS.md D39) and the sharp
//!   centre (`render::metal::foveation`, D40) need nothing from the window but its size and its
//!   display's refresh rate, so they work the same in the one-window app's window.

use warped_shared::app::{LevelEnd, LevelWindow};
use warped_shared::app_bundle::{ASSETS, get_level_folder};
use warped_shared::hud::{get_death_lines, make_respawn_bar_rects};
use warped_shared::match_state::Team;
use warped_shared::menu_and_map::{
    Cover, MenuAndMap, MouseChange, PlayOutcome, TextCell, TextLayout,
};
use warped_shared::metal_window::set_window_title;
use warped_shared::shot_noise;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use glam::{Vec2, Vec3, Vec4};
use objc2::Message;
use sdl3::event::Event;
use sdl3::keyboard::Scancode;
use sdl3::mouse::MouseButton;
use shot_noise::ShotNoise;

use crate::args::{Args, CameraView, Effect, validate_args};
use crate::game::audio::Audio;
use crate::game::console_bot::BotWalker;
use crate::game::guns::{Gun, GunInput, make_weapons};
use crate::game::player::{Player, PlayerInput};
use crate::game::session::{Controls, FrameData, RobotHold, Session};
use crate::game::world::Arena;
use crate::match_play::{MatchOptions, MatchPlay};
use crate::menu::{Menu, MenuOutcome, MenuSettings, Scoreboard};
use crate::minimap::{Mark, Minimap};
use crate::render::frame_clock::FrameClock;
use crate::render::frame_trace::{CpuFrameRow, FrameTrace};
use crate::render::metal::MetalRenderer;
use crate::render::overlay::HitEffect;
use crate::render::{FrameDescription, FrameTiming, HudImage, HudRect, RenderSettings, Renderer};
use crate::scene::Scene;
use crate::text_overlay::TextPicture;
use crate::{asset_manifest, console_reach, game, map, menu, minimap, net, render, server, title};

/// `MOUSE_RAD_PER_PT` is the look speed per point of mouse motion.
const MOUSE_RAD_PER_PT: f32 = 0.0022;
/// `MAX_FRAME_S` caps a frame's simulated time after a stall.
const MAX_FRAME_S: f32 = 0.05;
/// `REPORT_INTERVAL_S` is how often frame-time statistics are printed.
const REPORT_INTERVAL_S: f32 = 5.0;
/// `RENDER_SCALE_STEP_FRAC` is how much `[` and `]` change the render scale.
const RENDER_SCALE_STEP_FRAC: f32 = 0.1;

/// `THROW_SCRIPT_S` are the simulated times at which `--throw-yaws-rad` throws the purple and the
/// green mouth.
const THROW_SCRIPT_S: [f32; 2] = [0.5, 1.5];
/// `MESSAGE_PICTURE_PX` is the size of the picture of a message (a wormhole placed or destroyed),
/// `MESSAGE_HEIGHT_FRAC` the most of the window's height it takes, and `MESSAGE_TOP_FRAC` how far
/// down the window its top is.
const MESSAGE_PICTURE_PX: (u32, u32) = (480, 16);
const MESSAGE_HEIGHT_FRAC: f32 = 0.035;
const MESSAGE_TOP_FRAC: f32 = 0.62;

/// `script_wormholes` plays the wormhole flags of `args` into this frame's `controls`: the scripted
/// throws (`--throw-yaws-rad`), walking into the purple mouth (`--walk-into-thrown-at-s`), and
/// shooting at the nearest thrown mouth (`--shoot-thrown-at-s`); it turns the player as each needs.
///
/// Example: `let controls: Controls = script_wormholes(args, &mut session, controls, dt_s);`
fn script_wormholes(args: &Args, session: &mut Session, controls: Controls, dt_s: f32) -> Controls {
    let mut controls: Controls = controls;
    let (now_s, next_s) = (session.time_s, session.time_s + dt_s);
    if let Some(yaws_rad) = &args.throw_yaws_rad {
        assert_eq!(yaws_rad.len(), 2, "--throw-yaws-rad takes two yaws");
        for (yaw_rad, at_s) in yaws_rad.iter().zip(THROW_SCRIPT_S) {
            if now_s <= at_s && at_s < next_s {
                session.player.yaw_rad = *yaw_rad;
                controls.is_throw_pressed = true;
            }
        }
    }
    let is_due = |at_s: Option<f32>| at_s.is_some_and(|at_s| now_s >= at_s);
    if is_due(args.walk_into_thrown_at_s)
        && let Some(purple) = session.thrown.placed.purple
    {
        let to_m: Vec3 = purple.get_centre_m() - session.player.feet_m;
        session.player.yaw_rad = (-to_m.x).atan2(-to_m.z);
        controls.player.move_axes = Vec2::Y;
    }
    let eye_m: Vec3 = session.player.eye_position_m();
    let remote_places: Vec<Vec3> = session
        .remote_bots
        .iter()
        .flat_map(|remote| remote.wormholes.get_places())
        .map(|place| place.get_centre_m())
        .collect();
    let targets_m: Vec<Vec3> = if remote_places.is_empty() {
        let own = session.thrown.placed.get_places();
        own.iter().map(|place| place.get_centre_m()).collect()
    } else {
        remote_places
    };
    if is_due(args.shoot_thrown_at_s)
        && let Some(target_m) = targets_m
            .into_iter()
            .min_by(|a, b| a.distance(eye_m).total_cmp(&b.distance(eye_m)))
    {
        let to_m: Vec3 = target_m - eye_m;
        session.player.yaw_rad = (-to_m.x).atan2(-to_m.z);
        session.player.pitch_rad = (to_m.y / to_m.length()).asin();
        controls.gun.is_trigger_held = true;
        controls.gun.is_shoot_pressed = true;
    }
    controls
}

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
/// almost flat.
const MAP_PICTURE_HEIGHT_M: f32 = 600.0;

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
                view: crate::render::camera_math::make_view_rh(mouth.centre_m, *forward, *up),
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
                    thrown_lens_mouths: &[],
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

/// `MENU_DIM_FRAC` is how much the menu darkens the view behind it.
const MENU_DIM_FRAC: f32 = 0.75;
/// `HELD_MAP_DIM_FRAC` is how much the map shown while `Tab` is held darkens the game behind it.
const HELD_MAP_DIM_FRAC: f32 = 0.6;
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
/// Example: `let choice = run_title_screen(window, &mut renderer, &mut event_pump, &scene, &map_file, &settings);`
fn run_title_screen(
    window: &sdl3::video::Window,
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
    let mut size_px: (u32, u32) = window.size_in_pixels();
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
        let new_size_px: (u32, u32) = window.size_in_pixels();
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
                thrown_lens_mouths: &frame.thrown_lens_mouths,
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

/// `LevelFiles` is what the forest reads from disk before it draws: the map (its path and what it
/// holds) and the folders of the shared assets and of the forest's textures.
pub struct LevelFiles {
    map_path: PathBuf,
    map_file: map::MapFile,
    assets_dir: PathBuf,
    textures_dir: PathBuf,
}

impl LevelFiles {
    /// `Find` reads the map `args` names and checks every flag, or says what is wrong.
    ///
    /// Example: `let files: LevelFiles = LevelFiles::Find(&args)?;`
    #[allow(non_snake_case)]
    pub fn Find(args: &Args) -> Result<LevelFiles, String> {
        let map_path: PathBuf = args.get_map_path();
        let assets_dir: PathBuf = args.assets_dir.clone().unwrap_or_else(|| ASSETS.get_path());
        let textures_dir: PathBuf = args.textures_dir.clone().unwrap_or_else(|| {
            get_level_folder(env!("CARGO_MANIFEST_DIR")).join(asset_manifest::ASSETS_DIR_NAME)
        });
        let map_file: map::MapFile = map::load(&map_path).map_err(|error| {
            format!("{error}; write it with --export-map {}", map_path.display())
        })?;
        validate_args(args, &assets_dir)?;
        Ok(LevelFiles {
            map_path,
            map_file,
            assets_dir,
            textures_dir,
        })
    }
}

/// `FrameEvents` is what one frame's events asked of play: the mouse's motion, the controls
/// pressed, and whether the view (`V`) or the autopilot (`F5`) switches.
#[derive(Default)]
struct FrameEvents {
    look_delta_pt: Vec2,
    is_jump_pressed: bool,
    is_throw_pressed: bool,
    is_shoot_pressed: bool,
    is_swap_pressed: bool,
    picked_gun: Option<Gun>,
    is_view_switched: bool,
    is_autopilot_switched: bool,
}

/// `PlayScreen` is what covers the view (the menu and the held map), the menu's settings, and
/// whether the mouse is captured for play.
struct PlayScreen {
    menu_and_map: MenuAndMap<Menu>,
    menu_settings: MenuSettings,
    is_mouse_captured: bool,
}

/// `handle_event` acts on one window event: the menu gets the keys while it is open, `Esc`
/// opens it, the other keys and the mouse fill `events` or switch a render setting. It says
/// whether play goes on (or the player left the level or closed the window), and how the mouse
/// changes, which the caller applies before the next event.
///
/// Example: `let (outcome, mouse_change) = handle_event(event, &mut screen, &mut settings, &mut events);`
fn handle_event(
    event: Event,
    screen: &mut PlayScreen,
    settings: &mut RenderSettings,
    events: &mut FrameEvents,
) -> (PlayOutcome, Option<MouseChange>) {
    let mut mouse_change: Option<MouseChange> = None;
    match event {
        Event::Quit { .. } => return (PlayOutcome::CloseWindow, None),
        Event::KeyDown {
            scancode: Some(scancode),
            repeat: false,
            ..
        } if screen.menu_and_map.is_menu_open() => {
            match screen
                .menu_and_map
                .get_menu_mut()
                .expect("the menu is open")
                .handle(scancode, &mut screen.menu_settings)
            {
                MenuOutcome::Stay => {}
                MenuOutcome::Close => mouse_change = Some(screen.menu_and_map.close_menu()),
                MenuOutcome::LeaveLevel => return (PlayOutcome::LeaveLevel, None),
            }
            settings.render_scale_frac = screen.menu_settings.render_scale_frac;
            settings.is_vsync_enabled = screen.menu_settings.is_vsync_enabled;
            settings.visuals = screen.menu_settings.visuals.to_gpu();
        }
        Event::KeyDown {
            scancode: Some(scancode),
            repeat: false,
            ..
        } => match scancode {
            Scancode::Escape => {
                mouse_change = Some(screen.menu_and_map.open_menu(Menu::New()));
                screen.menu_settings.render_scale_frac = settings.render_scale_frac;
            }
            Scancode::Space => events.is_jump_pressed = true,
            Scancode::Q => events.is_throw_pressed = screen.is_mouse_captured,
            Scancode::V => events.is_view_switched = !events.is_view_switched,
            Scancode::_1 => events.picked_gun = Some(Gun::Laser),
            Scancode::_2 => events.picked_gun = Some(Gun::Rifle),
            Scancode::F1 => settings.is_ssao_enabled = !settings.is_ssao_enabled,
            Scancode::F2 => settings.is_bloom_enabled = !settings.is_bloom_enabled,
            Scancode::F3 => settings.is_fxaa_enabled = !settings.is_fxaa_enabled,
            Scancode::F4 => {
                screen.menu_settings.is_showing_frame_time =
                    !screen.menu_settings.is_showing_frame_time
            }
            Scancode::F5 => events.is_autopilot_switched = !events.is_autopilot_switched,
            Scancode::F6 => settings.is_color_grade_enabled = !settings.is_color_grade_enabled,
            Scancode::F7 => {
                settings.is_volumetric_haze_enabled = !settings.is_volumetric_haze_enabled
            }
            Scancode::F8 => {
                settings.is_material_response_enabled = !settings.is_material_response_enabled
            }
            Scancode::F9 => settings.is_foveation_enabled = !settings.is_foveation_enabled,
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
            if !screen.is_mouse_captured && !screen.menu_and_map.is_menu_open() =>
        {
            mouse_change = Some(MouseChange::Capture);
        }
        Event::MouseButtonDown {
            mouse_btn: MouseButton::Left,
            ..
        } if screen.is_mouse_captured => events.is_shoot_pressed = true,
        Event::MouseWheel { y, .. } if screen.is_mouse_captured && y != 0.0 => {
            events.is_swap_pressed = true;
        }
        Event::MouseMotion { xrel, yrel, .. } if screen.is_mouse_captured => {
            events.look_delta_pt += Vec2::new(xrel, yrel);
        }
        _ => {}
    }
    (PlayOutcome::Play, mouse_change)
}

/// `point_at_menu` tells the open `menu` where in its text the mouse of `event` is, and passes
/// it a left click there (a click on a page's name opens that page); other events change
/// nothing. `layout` is how the menu's text lies in `window`.
///
/// Example: `point_at_menu(open_menu, &event, window.window, menu_picture.get_layout(0.85));`
fn point_at_menu(menu: &mut Menu, event: &Event, window: &sdl3::video::Window, layout: TextLayout) {
    let (at_points, is_click) = match *event {
        Event::MouseMotion { x, y, .. } => ((x, y), false),
        Event::MouseButtonDown {
            mouse_btn: MouseButton::Left,
            x,
            y,
            ..
        } => ((x, y), true),
        _ => return,
    };
    let (size_points, size_px) = (window.size(), window.size_in_pixels());
    let px_per_point: f32 = size_px.0 as f32 / size_points.0 as f32;
    let cell: Option<TextCell> = layout.get_cell_at(
        size_px,
        (at_points.0 * px_per_point, at_points.1 * px_per_point),
    );
    menu.set_pointer(cell);
    if let (true, Some(cell)) = (is_click, cell) {
        menu.click(cell);
    }
}

/// `get_team` gives the team of a team index of this level's game (0 blue, 1 red).
///
/// Example: `assert_eq!(get_team(1), Team::Red);`
fn get_team(team_index: u32) -> Team {
    match team_index {
        0 => Team::Blue,
        1 => Team::Red,
        _ => panic!("get_team: team index {team_index} is neither 0 (blue) nor 1 (red)"),
    }
}

/// `connect` joins the game server `args` names (`--join`, or `auto` to find one), after starting
/// one in this process with `--host`; `None` plays alone.
///
/// Example: `let client: Option<net::Client> = connect(&args, &files)?;`
fn connect(args: &Args, files: &LevelFiles) -> Result<Option<net::Client>, String> {
    if !(args.host || args.join.is_some()) {
        return Ok(None);
    }
    if args.host {
        let options: server::ServerOptions = server::ServerOptions {
            port: args.port,
            map_path: files.map_path.clone(),
            level_result_file: args.level_result_file.clone(),
            game_password: Vec::new(),
            server_settings: args.server_settings.clone(),
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
        Some("auto") => net::find_server(Duration::from_secs(5))
            .map_err(|error| format!("no game server found ({error})"))?,
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
                .ok_or_else(|| format!("can't resolve --join {host}"))?
        }
    };
    net::Client::Connect(address, &files.map_file.name, b"")
        .map(Some)
        .map_err(|error| format!("can't reach {address}: {error}"))
}

/// `Level` is the forest set up in a lent window: its files, the renderer on the window's layer,
/// the sounds, the arena and its drawable scene, and the render settings the flags chose.
pub struct Level {
    files: LevelFiles,
    renderer: MetalRenderer,
    audio: Audio,
    arena: Arena,
    scene: Scene,
    settings: RenderSettings,
}

impl Level {
    /// `Load` sets up the forest in `window` as `args` say: the renderer on its layer and device,
    /// the sounds, and the scene uploaded to the GPU.
    ///
    /// Example: `let level: Level = Level::Load(&window, &args, files);`
    #[allow(non_snake_case)]
    pub fn Load(window: &LevelWindow, args: &Args, files: LevelFiles) -> Level {
        let mut renderer: MetalRenderer =
            MetalRenderer::new(window.layer.retain(), window.device.retain());
        println!("GPU: {}", renderer.device_name());
        let (width_px, height_px) = window.window.size_in_pixels();
        renderer.resize(width_px, height_px);
        let audio: Audio = Audio::new(window.sdl);
        let arena: Arena = Arena::from_map(&files.map_file);
        let mut scene: Scene = Scene::load(
            &mut renderer,
            &files.assets_dir,
            &files.textures_dir,
            &arena,
        );
        if args.hide_canopy {
            let surfaces: Vec<game::world::Surface> = std::mem::take(&mut scene.static_surfaces);
            let draws: Vec<render::DrawItem> = std::mem::take(&mut scene.static_draws);
            (scene.static_draws, scene.static_surfaces) = draws
                .into_iter()
                .zip(surfaces)
                .filter(|(_, surface)| *surface != game::world::Surface::Leaves)
                .unzip();
        }
        let settings: RenderSettings = RenderSettings {
            render_scale_frac: args.render_scale,
            is_ssao_enabled: !args.disable_effect.contains(&Effect::Ssao),
            is_bloom_enabled: !args.disable_effect.contains(&Effect::Bloom),
            is_fxaa_enabled: !args.disable_effect.contains(&Effect::Fxaa),
            is_color_grade_enabled: !args.disable_effect.contains(&Effect::ColorGrade),
            is_volumetric_haze_enabled: !args.disable_effect.contains(&Effect::VolumetricHaze),
            is_material_response_enabled: !args.disable_effect.contains(&Effect::MaterialResponse),
            shadow_distance_m: args.shadow_distance_m,
            is_vsync_enabled: !args.no_vsync,
            is_foveation_enabled: !args.disable_effect.contains(&Effect::Foveation)
                && !((args.photo.is_some() || args.video_frames.is_some()) && !args.photo_hud),
            visuals: crate::args::get_start_visuals(args).to_gpu(),
        };
        Level {
            files,
            renderer,
            audio,
            arena,
            scene,
            settings,
        }
    }

    /// `run_title_screen` shows the title screen in `window` until a choice is made, or `None`
    /// if the window closes or Escape is pressed.
    ///
    /// Example: `let choice: Option<title::Choice> = level.run_title_screen(&mut window);`
    pub fn run_title_screen(&mut self, window: &mut LevelWindow) -> Option<title::Choice> {
        run_title_screen(
            window.window,
            &mut self.renderer,
            window.event_pump,
            &self.scene,
            &self.files.map_file,
            &self.settings,
        )
    }

    /// `play` joins the game `args` names (alone against robots, or a server's) and plays it in
    /// `window` until the player leaves (`LevelEnd::LeaveLevel`) or closes the window
    /// (`LevelEnd::CloseWindow`), `window.match_link` says the round is over
    /// (`LevelEnd::RoundOver`, with the winner the forest saw), or a flag's run (a photo, a
    /// benchmark, `--exit-after-s`) ends (`None`). It sets the window's title to the game's
    /// status when `is_titling_window` (the program run alone), releases the mouse, and frees the
    /// level's GPU resources as it returns; it fails if it can't join the game.
    ///
    /// Example: `let end: Option<LevelEnd> = level.play(&mut window, &args, true)?;`
    pub fn play(
        self,
        window: &mut LevelWindow,
        args: &Args,
        is_titling_window: bool,
    ) -> Result<Option<LevelEnd>, String> {
        let Level {
            files,
            mut renderer,
            audio,
            arena,
            scene,
            mut settings,
        } = self;
        let client: Option<net::Client> = connect(args, &files)?;
        let (mut width_px, mut height_px) = window.window.size_in_pixels();
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
        session.console_places = console_reach::find_console_places(&files.map_file);
        session.is_team_known = !is_online;
        if args.bot || (args.autopilot && is_online) {
            session.walker = Some(BotWalker::New());
            session.is_console_objective = args.bot_objective.is_some();
        }
        if args.photo.is_some() || args.video_frames.is_some() {
            session.weapons = make_weapons(ShotNoise::FromSeed(RECORDING_SHOT_NOISE_SEED));
        }
        if args.rifle {
            session.weapons.switch_gun(Gun::Rifle);
        }
        if args.third_person {
            session.view_switch.toggle();
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
        .map_err(|error| format!("can't join: {error}"))?;
        let mut minimap: Minimap = Minimap::New(&session.arena);
        minimap.load_photo(&get_level_folder(env!("CARGO_MANIFEST_DIR")).join(MAP_PICTURE_PATH));
        let mut marks: Vec<Mark> = Vec::new();
        let mut screen: PlayScreen = PlayScreen {
            menu_and_map: MenuAndMap::New(),
            menu_settings: MenuSettings {
                look_rad_per_pt: MOUSE_RAD_PER_PT,
                render_scale_frac: settings.render_scale_frac,
                is_vsync_enabled: settings.is_vsync_enabled,
                is_showing_frame_time: args.show_frame_time,
                is_showing_minimap: true,
                visuals: crate::args::get_start_visuals(args),
            },
            is_mouse_captured: true,
        };
        if args.photo_hud {
            renderer.set_is_capturing_hud(true);
        }
        let mut death_picture: TextPicture =
            TextPicture::Banner(DEATH_PICTURE_PX.0, DEATH_PICTURE_PX.1);
        let mut message_picture: TextPicture =
            TextPicture::New(MESSAGE_PICTURE_PX.0, MESSAGE_PICTURE_PX.1);
        let mut menu_picture: TextPicture =
            TextPicture::New(menu::MENU_PICTURE_PX.0, menu::MENU_PICTURE_PX.1);
        let mut last_title: Instant = Instant::now();
        let mut view_stats: Vec<FrameStats> = CameraView::ALL
            .iter()
            .map(|_| FrameStats::default())
            .collect();

        let mouse = window.sdl.mouse();
        mouse.set_relative_mouse_mode(window.window, true);
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
                let _outcome: MenuOutcome = opened.handle(Scancode::E, &mut screen.menu_settings);
            }
            let _change: MouseChange = screen.menu_and_map.open_menu(opened);
        }
        let end: Option<LevelEnd> = 'running: loop {
            if window.match_link.is_round_over() {
                break 'running Some(LevelEnd::RoundOver {
                    winner: session.winner.map(get_team),
                });
            }
            let mut events: FrameEvents = FrameEvents::default();
            for event in window.event_pump.poll_iter() {
                if let Some(open_menu) = screen.menu_and_map.get_menu_mut() {
                    let layout: TextLayout = menu_picture.get_layout(MENU_HEIGHT_FRAC);
                    point_at_menu(open_menu, &event, window.window, layout);
                }
                let (outcome, mouse_change) =
                    handle_event(event, &mut screen, &mut settings, &mut events);
                if let Some(change) = mouse_change {
                    change.apply(&mut screen.is_mouse_captured, window.window, &mouse);
                }
                match outcome {
                    PlayOutcome::Play => {}
                    PlayOutcome::LeaveLevel => break 'running Some(LevelEnd::LeaveLevel),
                    PlayOutcome::CloseWindow => break 'running Some(LevelEnd::CloseWindow),
                }
            }
            if events.is_view_switched {
                session.view_switch.toggle();
            }
            if events.is_autopilot_switched {
                session.is_autopilot = !session.is_autopilot;
            }
            let (new_width_px, new_height_px) = window.window.size_in_pixels();
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
                    window
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
            let keys = window.event_pump.keyboard_state();
            let mouse_state = window.event_pump.mouse_state();
            let axis = |positive: Scancode, negative: Scancode| -> f32 {
                f32::from(u8::from(keys.is_scancode_pressed(positive)))
                    - f32::from(u8::from(keys.is_scancode_pressed(negative)))
            };
            let is_mouse_captured: bool = screen.is_mouse_captured;
            let controls: Controls = Controls {
                player: PlayerInput {
                    move_axes: Vec2::new(
                        axis(Scancode::D, Scancode::A),
                        axis(Scancode::W, Scancode::S),
                    ),
                    look_delta_rad: events.look_delta_pt * screen.menu_settings.look_rad_per_pt,
                    is_sprint_held: keys.is_scancode_pressed(Scancode::LShift),
                    is_crouch_held: keys.is_scancode_pressed(Scancode::LCtrl)
                        || keys.is_scancode_pressed(Scancode::C),
                    is_jump_pressed: events.is_jump_pressed,
                },
                gun: GunInput {
                    is_trigger_held: is_mouse_captured && mouse_state.left(),
                    is_shoot_pressed: events.is_shoot_pressed,
                    is_aim_held: is_mouse_captured && mouse_state.right(),
                    picked_gun: events.picked_gun,
                    is_swap_pressed: events.is_swap_pressed,
                },
                is_flashlight_held: args.hold_flashlight
                    || (is_mouse_captured && keys.is_scancode_pressed(Scancode::F)),
                is_throw_pressed: events.is_throw_pressed,
            };
            screen
                .menu_and_map
                .set_map_held(keys.is_scancode_pressed(Scancode::Tab));
            let controls: Controls = if screen.menu_and_map.is_menu_open() {
                Controls::default()
            } else {
                controls
            };
            let bench_view: Option<usize> = args
                .bench_views
                .map(|seconds| (start.elapsed().as_secs_f32() / seconds) as usize);
            if bench_view.is_some_and(|index| index >= CameraView::ALL.len()) {
                break 'running None;
            }
            let camera_view: Option<CameraView> = bench_view
                .map(|index| CameraView::ALL[index])
                .or(args.camera_view);
            let is_walking_by_script: bool = args
                .walk_into_thrown_at_s
                .is_some_and(|at_s| session.time_s >= at_s);
            if let Some(pose) = &args.camera_pose
                && !is_walking_by_script
            {
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
            let controls: Controls = script_wormholes(args, &mut session, controls, dt_s);
            let sim_start: Instant = Instant::now();
            let waypoint_before: usize = session.autopilot_waypoint;
            if let Some(period_s) = args.hurt_player_every_s
                && (session.time_s / period_s).floor()
                    != ((session.time_s + dt_s) / period_s).floor()
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
            let is_e_held: bool = !screen.menu_and_map.is_menu_open()
                && (args.hold_console || keys.is_scancode_pressed(Scancode::E));
            if let Err(error) = play.update(&mut session, &scene, is_e_held, dt_s) {
                eprintln!("network error: {error}");
            }
            if last_title.elapsed().as_secs_f32() > 0.5 {
                last_title = Instant::now();
                if is_titling_window {
                    let _ = set_window_title(window.window, &play.status(&session));
                }
            }

            smoothed_frame_ms += (frame_s * 1000.0 - smoothed_frame_ms) * 0.05;
            session.show_frame_ms = screen
                .menu_settings
                .is_showing_frame_time
                .then_some(smoothed_frame_ms);
            let sim_ms: f32 = sim_start.elapsed().as_secs_f32() * 1000.0;
            if args.bot {
                if args
                    .exit_after_s
                    .is_some_and(|seconds| start.elapsed().as_secs_f32() > seconds)
                {
                    break 'running None;
                }
                std::thread::sleep(Duration::from_secs_f32(
                    (BOT_STEP_S - now.elapsed().as_secs_f32()).max(0.0),
                ));
                continue;
            }
            let build_start: Instant = Instant::now();
            session.is_view_covered =
                screen.menu_and_map.get_cover() != Cover::Nothing || !session.player.is_alive();
            let mut frame: FrameData =
                session.frame_data(&scene, width_px as f32, height_px as f32);
            if let Some(fov_deg) = args.fov_deg {
                frame.camera.fov_y_rad = fov_deg.to_radians();
                frame.camera.far_m = f32::MAX;
            }
            if args.map_picture.is_some() {
                let (low_m, high_m) = (session.arena.bounds_min_m, session.arena.bounds_max_m);
                let centre_m: Vec3 = (low_m + high_m) * 0.5;
                let eye_m: Vec3 = Vec3::new(centre_m.x, MAP_PICTURE_HEIGHT_M, centre_m.z);
                frame.camera.view =
                    crate::render::camera_math::make_view_rh(eye_m, Vec3::NEG_Y, Vec3::NEG_Z);
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
            if let Some(open_menu) = screen.menu_and_map.get_menu_mut() {
                open_menu.set_match_lines(window.match_link.get_match_lines());
            }
            if let Some(open_menu) = screen.menu_and_map.get_menu() {
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
                menu_picture.draw(
                    &open_menu.get_lines(&screen.menu_settings, scoreboard),
                    false,
                );
                if let (menu::Page::Scores, false, Some((team, lines))) = (
                    open_menu.get_page(),
                    open_menu.is_help_shown(),
                    menu::get_own_team_lines(scoreboard),
                ) {
                    menu_picture.draw_team_box(team, lines);
                }
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
                if open_menu.get_page() == menu::Page::Map && !open_menu.is_help_shown() {
                    minimap.draw_map(session.player.feet_m, session.player.yaw_rad, &marks);
                    hud_images.push(minimap.get_map_image(window_width_px, window_height_px));
                }
            } else if screen.menu_and_map.get_cover() == Cover::Map {
                menu_picture.draw(&menu::get_held_map_lines(), false);
                frame.hud_rects.push(HudRect {
                    x_px: 0.0,
                    y_px: 0.0,
                    width_px: window_width_px,
                    height_px: window_height_px,
                    color_rgba: Vec4::new(0.0, 0.0, 0.0, HELD_MAP_DIM_FRAC),
                });
                hud_images.push(menu_picture.get_image(
                    window_width_px,
                    window_height_px,
                    MENU_HEIGHT_FRAC,
                ));
                minimap.draw_map(session.player.feet_m, session.player.yaw_rad, &marks);
                hud_images.push(minimap.get_map_image(window_width_px, window_height_px));
            } else if screen.menu_settings.is_showing_minimap
                && bench_view.is_none()
                && !args.play_route
            {
                minimap.draw_minimap(session.player.feet_m, session.player.yaw_rad, &marks);
                hud_images.push(minimap.get_minimap_image(window_width_px, window_height_px));
            }
            if !screen.menu_and_map.is_menu_open()
                && let Some(respawn_wait_s) = session.respawn_wait_s
                && let Some(respawn_in_s) = session.player.get_respawn_in_s(respawn_wait_s)
            {
                frame.overlay_rects.extend(make_respawn_bar_rects(
                    f64::from(respawn_in_s),
                    f64::from(respawn_wait_s),
                ));
                death_picture.draw(
                    &{
                        let [title, countdown] = get_death_lines(f64::from(respawn_in_s));
                        [(title, true), (countdown, false)]
                    },
                    true,
                );
                hud_images.push(death_picture.get_image(
                    window_width_px,
                    window_height_px,
                    BANNER_HEIGHT_FRAC,
                ));
            }
            if screen.menu_and_map.get_cover() == Cover::Nothing
                && let Some((text, _)) = &session.message
            {
                message_picture.draw(&[(text.clone(), false)], true);
                let mut image: HudImage = message_picture.get_image(
                    window_width_px,
                    window_height_px,
                    MESSAGE_HEIGHT_FRAC,
                );
                image.y_px = (window_height_px * MESSAGE_TOP_FRAC).floor();
                hud_images.push(image);
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
                    thrown_lens_mouths: &frame.thrown_lens_mouths,
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
                    break 'running None;
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
                break 'running None;
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
                break 'running None;
            }
            if args.exit_after_s.is_some_and(|seconds| elapsed_s > seconds) {
                break 'running None;
            }
        };
        mouse.set_relative_mouse_mode(window.window, false);
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
        Ok(end)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `level.rs`'s event handling, which decides how a level ends from the
    //! window's events, without a window or a GPU.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Event | the window's close button, a key with the menu closed, a key with the menu open, a click while the mouse is free | `closing_the_window_closes`, `confirming_the_quit_page_leaves_the_level`, `escape_opens_and_closes_the_menu`, `a_click_captures_the_mouse` |
    //! | Key on the QUIT page | `Return` (leave), `Y` (leave), another key (stay) | `confirming_the_quit_page_leaves_the_level` |
    //! | MATCH page | none (run alone), first (in the app) | `confirming_the_quit_page_leaves_the_level` |
    //! | Switch key pressed in one frame | once (switches), twice (doesn't) | `switch_keys_switch_once_per_press` |
    //! | Team index | 0, 1 | `team_indices_are_blue_and_red` |

    use sdl3::keyboard::Mod;

    use super::*;

    /// `make_screen` is play's screen at the start: menu closed, mouse captured.
    ///
    /// Example: `let mut screen: PlayScreen = make_screen();`
    fn make_screen() -> PlayScreen {
        PlayScreen {
            menu_and_map: MenuAndMap::New(),
            menu_settings: MenuSettings {
                look_rad_per_pt: MOUSE_RAD_PER_PT,
                render_scale_frac: 1.0,
                is_vsync_enabled: true,
                is_showing_frame_time: false,
                is_showing_minimap: true,
                visuals: warped_shared::visuals::VisualsSettings::IDENTITY,
            },
            is_mouse_captured: true,
        }
    }

    /// `make_settings` is the render settings with every effect on.
    ///
    /// Example: `let mut settings: RenderSettings = make_settings();`
    fn make_settings() -> RenderSettings {
        RenderSettings {
            render_scale_frac: 1.0,
            is_ssao_enabled: true,
            is_bloom_enabled: true,
            is_fxaa_enabled: true,
            is_color_grade_enabled: true,
            is_volumetric_haze_enabled: true,
            is_material_response_enabled: true,
            is_vsync_enabled: true,
            is_foveation_enabled: true,
            shadow_distance_m: 110.0,
            visuals: warped_shared::visuals::GpuPlayerVisuals::IDENTITY,
        }
    }

    /// `press` is the event of `scancode` going down.
    ///
    /// Example: `let event: Event = press(Scancode::Escape);`
    fn press(scancode: Scancode) -> Event {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: None,
            scancode: Some(scancode),
            keymod: Mod::NOMOD,
            repeat: false,
            which: 0,
            raw: 0,
        }
    }

    /// `handle_keys` hands each of `scancodes` to `handle_event` as play does, applying each mouse
    /// change before the next, and gives the last outcome.
    ///
    /// Example: `let outcome = handle_keys(&mut screen, &[Scancode::Escape]);`
    fn handle_keys(screen: &mut PlayScreen, scancodes: &[Scancode]) -> PlayOutcome {
        let mut settings: RenderSettings = make_settings();
        let mut events: FrameEvents = FrameEvents::default();
        let mut outcome: PlayOutcome = PlayOutcome::Play;
        for scancode in scancodes {
            let mouse_change: Option<MouseChange>;
            (outcome, mouse_change) =
                handle_event(press(*scancode), screen, &mut settings, &mut events);
            if let Some(change) = mouse_change {
                screen.is_mouse_captured = change == MouseChange::Capture;
            }
        }
        outcome
    }

    #[test]
    fn closing_the_window_closes() {
        let (outcome, mouse_change) = handle_event(
            Event::Quit { timestamp: 0 },
            &mut make_screen(),
            &mut make_settings(),
            &mut FrameEvents::default(),
        );
        assert_eq!((outcome, mouse_change), (PlayOutcome::CloseWindow, None));
    }

    #[test]
    fn confirming_the_quit_page_leaves_the_level() {
        for leave_key in [Scancode::Return, Scancode::Y] {
            let mut screen: PlayScreen = make_screen();
            let keys: [Scancode; 3] = [Scancode::Escape, Scancode::Q, leave_key];
            assert_eq!(handle_keys(&mut screen, &keys), PlayOutcome::LeaveLevel);
        }
        let mut screen: PlayScreen = make_screen();
        let keys: [Scancode; 3] = [Scancode::Escape, Scancode::Q, Scancode::N];
        assert_eq!(handle_keys(&mut screen, &keys), PlayOutcome::Play);
        let mut screen: PlayScreen = make_screen();
        assert_eq!(
            handle_keys(&mut screen, &[Scancode::Escape]),
            PlayOutcome::Play
        );
        screen
            .menu_and_map
            .get_menu_mut()
            .expect("the menu is open")
            .set_match_lines(Some(vec![("ROUND 1".to_string(), true)]));
        assert_eq!(
            handle_keys(&mut screen, &[Scancode::Q, Scancode::Return]),
            PlayOutcome::LeaveLevel
        );
    }

    #[test]
    fn escape_opens_and_closes_the_menu() {
        let mut screen: PlayScreen = make_screen();
        assert_eq!(
            handle_keys(&mut screen, &[Scancode::Escape]),
            PlayOutcome::Play
        );
        assert!(screen.menu_and_map.is_menu_open() && !screen.is_mouse_captured);
        assert_eq!(
            handle_keys(&mut screen, &[Scancode::Escape]),
            PlayOutcome::Play
        );
        assert!(!screen.menu_and_map.is_menu_open() && screen.is_mouse_captured);
    }

    #[test]
    fn a_click_captures_the_mouse() {
        let mut screen: PlayScreen = make_screen();
        screen.is_mouse_captured = false;
        let click: Event = Event::MouseButtonDown {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn: MouseButton::Left,
            clicks: 1,
            x: 0.0,
            y: 0.0,
        };
        let mut events: FrameEvents = FrameEvents::default();
        let (outcome, mouse_change) =
            handle_event(click, &mut screen, &mut make_settings(), &mut events);
        assert_eq!(
            (outcome, mouse_change),
            (PlayOutcome::Play, Some(MouseChange::Capture))
        );
        assert!(!events.is_shoot_pressed);
    }

    #[test]
    fn switch_keys_switch_once_per_press() {
        let mut screen: PlayScreen = make_screen();
        let mut settings: RenderSettings = make_settings();
        let mut events: FrameEvents = FrameEvents::default();
        for scancode in [Scancode::V, Scancode::F5, Scancode::F5] {
            handle_event(press(scancode), &mut screen, &mut settings, &mut events);
        }
        assert!(events.is_view_switched && !events.is_autopilot_switched);
    }

    #[test]
    fn team_indices_are_blue_and_red() {
        assert_eq!((get_team(0), get_team(1)), (Team::Blue, Team::Red));
    }
}
