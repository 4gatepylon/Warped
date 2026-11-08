//! `city` runs City Sphere: the window (walking inside the sphere, shooting, consoles, robots,
//! the glass level), its game server, the headless bot, `--bench-views` and `--play-route`. The
//! level is `city_world.rs`, the drawing `render_city.rs`; the network, settings, guns and
//! overlay are the hyperbolic demo's, shared.
//!
//! Interface: `run`, `TeamName`, `BotObjective`.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use clap::ValueEnum;
use glam::{DMat3, DMat4, DQuat, DVec3, Mat4};
use objc2_core_foundation::CGSize;
use objc2_metal::MTLPixelFormat;
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use serde_json::json;

use super::{
    Arguments, BANNER_HEIGHT_FRAC, BYTES_PER_PIXEL, DEATH_DIM_FRAC, DEATH_MASK_PX,
    DISABLE_BAR_MIN_FRAC, DISABLE_BAR_RGB, DISABLE_BAR_SIZE_FRAC, ENERGY_BAR_MIN_FRAC,
    ENERGY_BAR_RGB, ENERGY_PULSE_RGB, ENERGY_SEGMENT_COUNT, Gun, HEALTH_BAR_MIN_FRAC,
    HEALTH_BAR_RGB, HEALTH_PULSE_RGB, HEAT_BAR_MIN_FRAC, HEAT_BAR_OVERHEATED_RGB, HEAT_BAR_RGB,
    MAP_AREA_MAX_EXCL_MASK_PX, MAP_AREA_MIN_MASK_PX, MENU_DIM_FRAC, MENU_HEIGHT_FRAC, MENU_MASK_PX,
    MS_PER_S, OverlayText, PICTURE_PX_PER_MASK_PX, PLAYER_SETTINGS_HOME_PATH, Picture, Renderer,
    RendererKind, Result, RouteKind, WINDOW_TITLE, find_lan_address, get_crosshair_rects,
    get_lobby_lines, get_server_address, load_settings, make_renderer, make_run_folder,
    make_text_mask, make_wand, save_png,
};
use crate::avatar_model::{AvatarClip, AvatarModel, BODY_MODEL};
use crate::city_world::{
    CityInput, CityPlayer, CityWorld, HEAD_HEIGHT_M, Shot, ShotHit, Side, TORSO_HEIGHT_M, Target,
    get_up,
};
use crate::hud_bars;
use crate::key_bindings::{self, Action, KeyBindings};
use crate::menu;
use crate::net;
use crate::render_city::{
    BEACON_SLOT_COUNT, CityFrame, CityScene, build_scene, push_beam, push_core_halo, push_mesh,
};
use crate::render_metal::{self, FrameData};
use crate::settings::{PerPlayerSettings, ServerSettings};
use crate::thrown_wormholes::PlayerWormholes;
use crate::viewmodel;
use crate::voxel_world::{MUZZLE_M, SPAWN_TURN_RAD};

const MAP_NAME: &str = "city_sphere";
const TAN_HALF_FOV: f32 = 0.62;
const BEAM_WIDTH_M: f64 = 0.06;
const AVATAR_HEIGHT_M: f64 = 1.8;
const TEAM_COLORS: [[f32; 4]; 2] = [[0.3, 0.55, 1.0, 1.0], [1.0, 0.3, 0.25, 1.0]];
const OVERHEAT_PITCH_RAD: f64 = 1.2;
const PHOTO_TIME_S: f64 = 0.4;
const BEACON_OFF_BODY_FRAC: f32 = 0.25;
const PHOTO_CENTRE_EYE_M: f64 = 3.5;
const MAP_PICTURE_PATH: &str = "assets/map_picture.png";
const MAP_PICTURE_FACE_PX: usize = 1536;
const MAP_PICTURE_SUPERSAMPLE: usize = 3;
const RIFLE_ROUTE_SETTLE_S: f64 = 1.0;
const RIFLE_ROUTE_TIMEOUT_S: f64 = 20.0;
const RIFLE_ROUTE_RESPAWN_WAIT_S: f64 = 4.0;
const TARGET_DISTANCE_M: f64 = 7.0;
const CITY_MAP_LEGEND: &str = "The whole sphere, unrolled: you white, cyan holes to the street";
const MAP_STREET_COLOR: [u8; 4] = [70, 70, 78, 255];
const MAP_BUILDING_COLOR: [u8; 4] = [140, 105, 85, 255];
const MAP_COVER_COLOR: [u8; 4] = [150, 165, 190, 255];
const MAP_HOLE_COLOR: [u8; 4] = [90, 230, 255, 255];
const MAP_CONSOLE_HALF_PX: i32 = 6;
const MAP_PLAYER_HALF_PX: i32 = 4;
const MAP_SELF_HALF_PX: i32 = 6;
const MAP_COVER_ABOVE_GLASS_M: f64 = 1.0;
const TEAM_TINTS: [[f32; 3]; 2] = [[0.55, 0.8, 1.5], [1.5, 0.6, 0.5]];
const MINIMAP_RADIUS_FRAC: f64 = 0.12;
const MINIMAP_MARGIN_FRAC: f64 = 0.03;
const MINIMAP_CONSOLE_FRAC: f64 = 0.008;
const MINIMAP_DOT_FRAC: f64 = 0.006;
const MINIMAP_BACKGROUND_COLOR: [f32; 4] = [0.05, 0.05, 0.08, 0.55];
const MINIMAP_OFF_COLOR: [f32; 4] = [0.25, 0.25, 0.28, 0.9];
const MINIMAP_SELF_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const PULSE_RAD_PER_S: f64 = 12.0;
const CORE_HALO_RADIUS_PER_CORE_RADIUS: f64 = 3.0;
const RUN_M_PER_S: f64 = 6.0;
const WALK_M_PER_S: f64 = 0.6;
const STEPS_RAD_PER_M: f64 = 2.2;
const RECOIL_S: f64 = 0.25;
const TURN_RAD_PER_S: f64 = 5.0;
const STUCK_S: f64 = 1.2;
const STUCK_M: f64 = 0.5;
const UNSTICK_S: f64 = 0.7;
const RIDE_START_UNDER_GLASS_M: f64 = 0.5;
const STEER_MIN_M: f64 = 0.3;
const MESSAGE_S: f64 = 3.0;
const MAX_DT_S: f64 = 0.1;
const BOT_STEP_S: f64 = 1.0 / 60.0;
const CONSOLE_REACH_SLACK_M: f64 = 0.6;
const LOBBY_ORBIT_RAD_PER_S: f64 = 0.08;
const RENDER_SCALE_STEP: f64 = 0.1;
const MIN_RENDER_SCALE: f64 = 0.3;
const HUD_PERIOD_S: f64 = 0.25;
const ROUTE_TIMEOUT_S: f64 = 240.0;
const BENCH_ROBOT_COUNT: usize = 8;
const PHASE_COUNT: usize = 5;
const PHASE_NAMES: [&str; PHASE_COUNT] =
    ["events", "update", "pack", "overlay", "render_and_present"];
const WARMUP_FRAME_COUNT: usize = 30;
const SLOW_FRAME_MS: f64 = 17.5;
const STAIRS_ARRIVE_M: f64 = 0.5;
const PYLON_ARRIVE_M: f64 = 1.8;
const STAIRS_ROUTE_END_WAIT_S: f64 = 2.0;

/// `TeamName` is a team for `--team`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum TeamName {
    Blue,
    Red,
}

impl TeamName {
    /// `get_index` is the team's index in `net::TEAM_NAMES`.
    fn get_index(self) -> u32 {
        match self {
            TeamName::Blue => 0,
            TeamName::Red => 1,
        }
    }
}

/// `BotObjective` is what `--bot` does instead of wandering and shooting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum BotObjective {
    Consoles,
    /// `Target` stands still in the open in front of blue's spawn (back there after each
    /// respawn) without shooting, as the target of `--play-route --route rifle`.
    Target,
}

/// `run` runs City Sphere as the command line asks: a server, the bot, the benchmark or the window.
///
/// Example: `city::run(&arguments)?;`
pub fn run(arguments: &Arguments) -> Result<()> {
    if arguments.serve {
        return serve_only(arguments);
    }
    if arguments.bot {
        return run_bot(arguments);
    }
    if arguments.renderer != RendererKind::Metal {
        return Err("City Sphere draws with Metal only; leave out --renderer".into());
    }
    viewmodel::check_gun_models()?;
    let mut renderer: Box<dyn Renderer> = make_renderer(arguments.renderer, arguments.sampling)?;
    if arguments.bench_views {
        return bench_views(arguments, renderer.as_mut());
    }
    if arguments.photos {
        return render_photos(arguments, renderer.as_mut());
    }
    if arguments.map_picture {
        return render_map_picture(arguments, renderer.as_mut());
    }
    let metal: &mut render_metal::MetalRenderer =
        renderer.as_metal().ok_or("City Sphere needs Metal")?;
    play(arguments, metal)
}

/// `step_respawn` counts an eliminated player's `respawn_in_s` down by `dt_s` and asks the server
/// to rejoin once it runs out; with no countdown running it keeps asking to join until welcomed
/// (`net::Client::keep_joining`). A player waiting to respawn never asks, so they come back only
/// after the settings' `respawn.delay_s`, as on Lunar Surface.
///
/// Example: `step_respawn(&mut client, &mut respawn_in_s, dt_s)?;`
fn step_respawn(client: &mut net::Client, respawn_in_s: &mut Option<f64>, dt_s: f64) -> Result<()> {
    match respawn_in_s {
        Some(left_s) if *left_s > dt_s => *left_s -= dt_s,
        Some(_) => {
            *respawn_in_s = None;
            client.rejoin()?;
        }
        None => client.keep_joining()?,
    }
    Ok(())
}

/// `get_console_teams` lists each console's team in the server's console order.
fn get_console_teams(world: &CityWorld) -> Vec<u32> {
    world.consoles.iter().map(|console| console.team).collect()
}

/// `start_server` starts a game server for City Sphere on `port` in a thread; a new game starts
/// when Enter is pressed in the terminal unless `result_path` is set (then the won game stays won
/// and the winner is written there); it announces itself on the network if `should_announce`.
///
/// Example: `let (stop, thread) = start_server(47810, settings, &world, Vec::new(), None, true)?;`
fn start_server(
    port: u16,
    settings: ServerSettings,
    world: &CityWorld,
    game_password: Vec<u8>,
    result_path: Option<PathBuf>,
    should_announce: bool,
) -> Result<(Arc<AtomicBool>, JoinHandle<()>)> {
    let socket: std::net::UdpSocket = net::bind_server(port)?;
    let should_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let stop_flag: Arc<AtomicBool> = Arc::clone(&should_stop);
    let should_start_new_game: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    if result_path.is_none() {
        let new_game_flag: Arc<AtomicBool> = Arc::clone(&should_start_new_game);
        std::thread::spawn(move || {
            for _ in std::io::stdin().lines().map_while(std::io::Result::ok) {
                new_game_flag.store(true, Ordering::Relaxed);
            }
        });
    }
    let console_teams: Vec<u32> = get_console_teams(world);
    let thread: JoinHandle<()> = std::thread::spawn(move || {
        if let Err(error) = net::serve(
            socket,
            &stop_flag,
            MAP_NAME.to_string(),
            settings,
            console_teams,
            &should_start_new_game,
            None,
            game_password,
            result_path,
            should_announce,
        ) {
            eprintln!("The game server stopped: {error}");
        }
    });
    Ok((should_stop, thread))
}

/// `serve_only` runs a City Sphere server without a window until stopped (or `--exit-after-s`).
fn serve_only(arguments: &Arguments) -> Result<()> {
    let world: CityWorld = CityWorld::Builtin()?;
    let (should_stop, thread) = start_server(
        arguments.port,
        load_settings(arguments)?,
        &world,
        net::read_game_password(arguments.game_password_file.as_deref())?,
        arguments.level_result_file.clone(),
        true,
    )?;
    let port: u16 = arguments.port;
    let address: String =
        find_lan_address().map_or("<this computer's address>".to_string(), |a| a.to_string());
    println!("City Sphere game server on UDP port {port}, without a window. Stop it with Ctrl-C.");
    println!("Join from demos/city_sphere_rust/: make join-game SERVER={address} PORT={port}");
    println!("  (or SERVER=127.0.0.1 on this computer).");
    match &arguments.level_result_file {
        Some(path) => println!(
            "The winner goes to {} once a team has turned off the other's consoles.",
            path.display()
        ),
        None => println!(
            "A team wins by turning off the other team's consoles; then press Enter here for a new game."
        ),
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

/// `Step` is one step of a scripted route (`--play-route`) or the bot: walk to a place on a level
/// (taking lifts up and holes down as needed), turn and tilt the head, shoot, hold `E` on a
/// console until it is off, or wait; and for the rifle check, take the rifle, fire it into the
/// glass until it overheats, wait until it has cooled, or fire at the nearest enemy's head or body
/// until a player is eliminated.
#[derive(Clone, Copy, Debug)]
enum Step {
    GoTo {
        direction: DVec3,
        side: Side,
        arrive_m: f64,
    },
    WalkTo {
        direction: DVec3,
        arrive_m: f64,
    },
    Aim {
        yaw_rad: f64,
        pitch_rad: f64,
    },
    Shoot,
    HoldConsole {
        index: u8,
    },
    Wait {
        s: f64,
    },
    SelectRifle,
    Overheat,
    CoolDown,
    FireAt {
        is_head: bool,
        timeout_s: f64,
    },
}

/// `BrainOutput` is what a brain does this frame.
struct BrainOutput {
    input: CityInput,
    is_shooting: bool,
    console: Option<u8>,
    wants_rifle: bool,
}

/// `BrainSight` is what a brain knows besides the world and its player: the rifle's heat, the
/// nearest enemy's head and body centres, and how many players this player has eliminated.
#[derive(Default)]
struct BrainSight {
    is_overheated: bool,
    heat_frac: f64,
    enemy: Option<(DVec3, DVec3)>,
    kill_count: u32,
}

/// `Brain` follows steps for the route and the bot, with a little unsticking when a box is in the way.
struct Brain {
    steps: Vec<Step>,
    index: usize,
    step_s: f64,
    stuck_from: DVec3,
    stuck_s: f64,
    unstick_s: f64,
    kills_at_step_start: u32,
}

impl Brain {
    /// `New` starts on the first step.
    #[allow(non_snake_case)]
    fn New(steps: Vec<Step>) -> Brain {
        Brain {
            steps,
            index: 0,
            step_s: 0.0,
            stuck_from: DVec3::ZERO,
            stuck_s: 0.0,
            unstick_s: 0.0,
            kills_at_step_start: 0,
        }
    }

    /// `restart` goes back to the first step (after a respawn); steps already done, such as a
    /// console that is off, finish at once.
    fn restart(&mut self) {
        (self.index, self.step_s, self.unstick_s) = (0, 0.0, 0.0);
    }

    /// `is_done` says whether every step is done.
    fn is_done(&self) -> bool {
        self.index >= self.steps.len()
    }

    /// `drive` works on the current step for `dt_s`, moving to the next when it is done.
    fn drive(
        &mut self,
        world: &CityWorld,
        player: &mut CityPlayer,
        dt_s: f64,
        game: &net::GameState,
        can_shoot: bool,
        sight: &BrainSight,
    ) -> BrainOutput {
        let mut out = BrainOutput {
            input: CityInput {
                speed_frac: 1.0,
                ..CityInput::default()
            },
            is_shooting: false,
            console: None,
            wants_rifle: false,
        };
        let Some(&step) = self.steps.get(self.index) else {
            return out;
        };
        if self.step_s == 0.0 {
            self.kills_at_step_start = sight.kill_count;
        }
        self.step_s += dt_s;
        let is_finished: bool = match step {
            Step::GoTo {
                direction,
                side,
                arrive_m,
            } => self.go_to(
                world,
                player,
                (direction, side, arrive_m),
                dt_s,
                &mut out.input,
            ),
            Step::WalkTo {
                direction,
                arrive_m,
            } => walk_to(player, direction, arrive_m, dt_s, &mut out.input),
            Step::Aim { yaw_rad, pitch_rad } => {
                player.look_around(yaw_rad, 0.0);
                player.pitch_rad = pitch_rad;
                true
            }
            Step::Shoot => {
                out.is_shooting = can_shoot;
                can_shoot
            }
            Step::HoldConsole { index } => {
                out.console = Some(index);
                game.off_mask >> index & 1 == 1 || game.winner.is_some()
            }
            Step::Wait { s } => self.step_s >= s,
            Step::SelectRifle => {
                out.wants_rifle = true;
                true
            }
            Step::Overheat => {
                player.pitch_rad = OVERHEAT_PITCH_RAD;
                out.is_shooting = true;
                sight.is_overheated
            }
            Step::CoolDown => !sight.is_overheated && sight.heat_frac <= 0.0,
            Step::FireAt { is_head, timeout_s } => {
                if let Some((head, body)) = sight.enemy {
                    aim_at(player, if is_head { head } else { body });
                    out.is_shooting = true;
                }
                sight.kill_count > self.kills_at_step_start || self.step_s >= timeout_s
            }
        };
        if is_finished {
            self.index += 1;
            self.step_s = 0.0;
        }
        out
    }

    /// `go_to` walks toward a place, riding the nearest lift up or dropping through the nearest
    /// lift's hole when the place is on the other level; true once there.
    fn go_to(
        &mut self,
        world: &CityWorld,
        player: &mut CityPlayer,
        goal: (DVec3, Side, f64),
        dt_s: f64,
        input: &mut CityInput,
    ) -> bool {
        let (direction, side, arrive_m) = goal;
        let Some(glass) = world.get_glass() else {
            return true;
        };
        let r: f64 = player.feet.length();
        let side_now: Side = world.get_side(player.feet);
        let is_in_lift: bool = world.is_in_lift(player.feet);
        let target: DVec3 = direction.normalize()
            * if side == Side::Street {
                world.street_radius_m
            } else {
                glass.radius_m
            };
        if side == Side::Glass && is_in_lift {
            input.is_jumping = true;
        }
        let is_riding: bool =
            side == Side::Glass && is_in_lift && r > glass.radius_m - RIDE_START_UNDER_GLASS_M;
        let from: DVec3 = player.feet.normalize();
        let hole: DVec3 = world
            .lift_holes
            .iter()
            .map(|&(hole, _)| hole)
            .max_by(|a, b| a.dot(from).total_cmp(&b.dot(from)))
            .unwrap_or(from);
        let waypoint: DVec3 = if is_riding {
            hole * r
        } else if side_now == side {
            target
        } else {
            hole * r
        };
        let up: DVec3 = player.get_up();
        let away: DVec3 = waypoint - player.feet;
        let to: DVec3 = away - up * away.dot(up);
        if side_now == side && !is_riding && to.length() < arrive_m {
            return true;
        }
        if !is_riding || to.length() > STEER_MIN_M {
            steer(player, to, dt_s, input);
        }
        if input.forward_frac > 0.0 {
            self.stuck_s += dt_s;
            if self.stuck_s > STUCK_S {
                if (player.feet - self.stuck_from).length() < STUCK_M {
                    self.unstick_s = UNSTICK_S;
                }
                (self.stuck_from, self.stuck_s) = (player.feet, 0.0);
            }
        }
        if self.unstick_s > 0.0 {
            self.unstick_s -= dt_s;
            input.right_frac = 1.0;
            input.is_jumping = true;
        }
        false
    }
}

/// `walk_to` walks `player` straight toward the place above or below the unit `direction`, up or
/// down whatever is in the way (the stairs), whichever level it is on; true once within
/// `arrive_m` of it along the ground.
fn walk_to(
    player: &mut CityPlayer,
    direction: DVec3,
    arrive_m: f64,
    dt_s: f64,
    input: &mut CityInput,
) -> bool {
    let up: DVec3 = player.get_up();
    let away: DVec3 = direction.normalize() * player.feet.length() - player.feet;
    let to: DVec3 = away - up * away.dot(up);
    if to.length() < arrive_m {
        return true;
    }
    steer(player, to, dt_s, input);
    false
}

/// `make_stairs_route` is the walk for `team` from its spawn up its stairs onto the glass level
/// and to its pylon (`--route stairs`).
fn make_stairs_route(world: &CityWorld, team: u32) -> Result<Vec<Step>> {
    let (bottom, top) = world
        .get_stairs_ends(team)
        .ok_or("--route stairs needs a map with stairs")?;
    let console = world
        .consoles
        .iter()
        .find(|c| c.team == team)
        .ok_or("--route stairs needs the team's console")?;
    Ok(vec![
        Step::WalkTo {
            direction: bottom,
            arrive_m: STAIRS_ARRIVE_M,
        },
        Step::WalkTo {
            direction: top,
            arrive_m: STAIRS_ARRIVE_M,
        },
        Step::GoTo {
            direction: world.get_console_center(console),
            side: Side::Glass,
            arrive_m: PYLON_ARRIVE_M,
        },
        Step::Wait {
            s: STAIRS_ROUTE_END_WAIT_S,
        },
    ])
}

/// `aim_at` turns `player`'s body and tilts the head so the crosshair points at `point`.
fn aim_at(player: &mut CityPlayer, point: DVec3) {
    let aim: DVec3 = (point - player.get_eye_frame().w_axis.truncate()).normalize();
    let up: DVec3 = player.get_up();
    let flat: DVec3 = (aim - up * aim.dot(up)).normalize();
    let forward: DVec3 = -player.body.z_axis;
    player.look_around(forward.cross(flat).dot(up).atan2(forward.dot(flat)), 0.0);
    player.pitch_rad = aim.dot(up).asin();
}

/// `make_target_player` stands a player in the open on the street, `TARGET_DISTANCE_M` in front of
/// blue's first spawn, for the rifle check to shoot at.
///
/// Example: `let target = make_target_player(&world);`
fn make_target_player(world: &CityWorld) -> CityPlayer {
    let mut player: CityPlayer = world.make_player(0, 0);
    let old_up: DVec3 = player.get_up();
    let ahead: DVec3 = player.feet - player.body.z_axis * TARGET_DISTANCE_M;
    player.feet = ahead.normalize() * player.feet.length();
    let rotation: DQuat = DQuat::from_rotation_arc(old_up, player.get_up());
    player.body = DMat3::from_quat(rotation) * player.body;
    player
}

/// `make_rifle_route` is the rifle check for blue at its spawn: take the rifle, fire it into the
/// glass until it overheats, wait until it has cooled, then eliminate the nearest red player (a
/// `--bot-objective target` bot) with head hits, wait for it to come back, and eliminate it with
/// body hits.
fn make_rifle_route() -> Vec<Step> {
    vec![
        Step::SelectRifle,
        Step::Wait {
            s: RIFLE_ROUTE_SETTLE_S,
        },
        Step::Overheat,
        Step::CoolDown,
        Step::FireAt {
            is_head: true,
            timeout_s: RIFLE_ROUTE_TIMEOUT_S,
        },
        Step::Wait {
            s: RIFLE_ROUTE_RESPAWN_WAIT_S,
        },
        Step::FireAt {
            is_head: false,
            timeout_s: RIFLE_ROUTE_TIMEOUT_S,
        },
        Step::Wait {
            s: RIFLE_ROUTE_SETTLE_S,
        },
    ]
}

/// `steer` turns `player` toward the tangent direction `to` and walks when facing roughly along it.
fn steer(player: &mut CityPlayer, to: DVec3, dt_s: f64, input: &mut CityInput) {
    if to.length() < STEER_MIN_M {
        return;
    }
    let up: DVec3 = player.get_up();
    let forward: DVec3 = -player.body.z_axis;
    let want: DVec3 = to.normalize();
    let error: f64 = forward.cross(want).dot(up).atan2(forward.dot(want));
    let turn: f64 = error.clamp(-TURN_RAD_PER_S * dt_s, TURN_RAD_PER_S * dt_s);
    player.look_around(turn, 0.0);
    input.forward_frac = if error.abs() < 0.8 { 1.0 } else { 0.2 };
}

/// `make_route` is the scripted route for blue: ride a lift to the glass level, shoot four shots
/// that reflect or cross, walk across the glass to red's console (its only one) and turn it off
/// (blue wins), then wait.
fn make_route(world: &CityWorld) -> Vec<Step> {
    let console = |index: u8| {
        world
            .consoles
            .iter()
            .find(|c| c.index == index)
            .map(|c| world.get_console_center(c))
    };
    let mut steps: Vec<Step> = vec![
        Step::GoTo {
            direction: DVec3::new(0.95, 0.15, -0.25),
            side: Side::Glass,
            arrive_m: 1.0,
        },
        Step::Aim {
            yaw_rad: 0.0,
            pitch_rad: -0.45,
        },
        Step::Shoot,
        Step::Aim {
            yaw_rad: 1.3,
            pitch_rad: 0.25,
        },
        Step::Shoot,
        Step::Aim {
            yaw_rad: 1.1,
            pitch_rad: -0.2,
        },
        Step::Shoot,
        Step::Aim {
            yaw_rad: 2.0,
            pitch_rad: -0.8,
        },
        Step::Shoot,
        Step::Aim {
            yaw_rad: 0.0,
            pitch_rad: 0.0,
        },
    ];
    for target in world.consoles.iter().filter(|c| c.team == 1) {
        let index: u8 = target.index;
        if let Some(center) = console(index) {
            steps.push(Step::GoTo {
                direction: center,
                side: Side::Glass,
                arrive_m: 1.8,
            });
            steps.push(Step::HoldConsole { index });
        }
    }
    steps.push(Step::Wait { s: 3.0 });
    steps
}

/// `Beam` is a shot's corners, its team and its brightness (0 to 1) for one frame.
type Beam = (Vec<DVec3>, u32, f64);

/// `RifleLog` records the rifle check on the route: shots from cold until the rifle overheated,
/// the seconds it took to cool down, and for each player this player eliminated, the rifle's head
/// and body hits since the elimination before.
#[derive(Default)]
struct RifleLog {
    cold_shot_count: u32,
    shots_until_overheat: Option<u32>,
    overheated_at_s: Option<f64>,
    cool_down_s: Option<f64>,
    head_hit_count: u32,
    body_hit_count: u32,
    kills: Vec<[u32; 2]>,
}

/// `LiveShot` is a shot being drawn: its path, when it was fired, by which team, and whether it
/// is this window's own (drawn from the gun's muzzle).
struct LiveShot {
    shot: Shot,
    start_s: f64,
    team: u32,
    is_own: bool,
}

/// `Motion` is what a remote player's robot is doing: where its feet were and how fast it goes.
#[derive(Clone, Copy)]
struct Motion {
    feet: DVec3,
    speed_m_per_s: f64,
}

/// `Robot` is one robot to draw.
struct Robot {
    feet: DVec3,
    forward: DVec3,
    team: u32,
    clip: AvatarClip,
}

/// `make_frame` packs a frame: the scene, the consoles and pylons in their state, robots and beams.
#[allow(clippy::too_many_arguments)]
fn make_frame(
    world: &CityWorld,
    scene: &Arc<CityScene>,
    body: &AvatarModel,
    eye: DMat4,
    robots: &[Robot],
    beams: &[Beam],
    masks: (u32, u32),
    now_s: f64,
    render_px: (usize, usize),
) -> CityFrame {
    let (off_mask, disabling_mask) = masks;
    let mut dynamic = Vec::new();
    let pulse: f32 = (0.55 + 0.45 * (now_s * PULSE_RAD_PER_S).sin()) as f32;
    let mut beacons: [[f32; 4]; BEACON_SLOT_COUNT] = [[1.0, 1.0, 0.0, 0.0]; BEACON_SLOT_COUNT];
    for console in &world.consoles {
        let i: u8 = console.index;
        beacons[usize::from(i)] = if off_mask >> i & 1 == 1 {
            [0.0, BEACON_OFF_BODY_FRAC, 0.0, 0.0]
        } else if disabling_mask >> i & 1 == 1 {
            [pulse, 1.0, 0.0, 0.0]
        } else {
            [1.0, 1.0, 0.0, 0.0]
        };
    }
    let mut posed_clips: Vec<(AvatarClip, crate::mesh::MeshModel)> = Vec::new();
    for robot in robots {
        if !posed_clips.iter().any(|(clip, _)| *clip == robot.clip) {
            let duration_s: f64 = body.get_duration_s(robot.clip).max(1e-3);
            posed_clips.push((
                robot.clip,
                body.make_posed(robot.clip, now_s.rem_euclid(duration_s)),
            ));
        }
        let posed = &posed_clips
            .iter()
            .find(|(clip, _)| *clip == robot.clip)
            .expect("posed above")
            .1;
        let up: DVec3 = get_up(robot.feet);
        let tangent: DVec3 =
            (robot.forward - up * robot.forward.dot(up)).normalize_or(up.any_orthonormal_vector());
        let back: DVec3 = -tangent;
        let right: DVec3 = up.cross(back);
        let world_from_model: Mat4 = DMat4::from_cols(
            right.extend(0.0),
            up.extend(0.0),
            back.extend(0.0),
            robot.feet.extend(1.0),
        )
        .as_mat4();
        push_mesh(
            &mut dynamic,
            posed,
            body.get_materials(),
            world_from_model,
            TEAM_TINTS[robot.team as usize % 2],
        );
    }
    let camera: DVec3 = eye.w_axis.truncate();
    let mut beam_vertices = Vec::new();
    for (points, team, alpha) in beams {
        let c: [f32; 3] = world.laser_colors[*team as usize % 2];
        for pair in points.windows(2) {
            push_beam(
                &mut beam_vertices,
                pair[0],
                pair[1],
                camera,
                BEAM_WIDTH_M,
                [c[0], c[1], c[2], (*alpha * 1.4) as f32],
            );
        }
    }
    if let Some(core) = world.shells.iter().find(|s| s.is_ball()) {
        push_core_halo(
            &mut beam_vertices,
            core.radius_m * CORE_HALO_RADIUS_PER_CORE_RADIUS,
            camera,
        );
    }
    CityFrame {
        render_px,
        scene: Arc::clone(scene),
        camera_from_world: eye.inverse().as_mat4(),
        camera_m: camera.as_vec3(),
        tan_half_fov: TAN_HALF_FOV,
        beacons,
        time_s: now_s as f32,
        dynamic,
        beams: beam_vertices,
    }
}

/// `make_eye` is an eye frame at `position` looking along `forward`, its up as near `up` as can be.
fn make_eye(position: DVec3, forward: DVec3, up: DVec3) -> DMat4 {
    let back: DVec3 = -forward.normalize();
    let right: DVec3 = up.cross(back).normalize_or(back.any_orthonormal_vector());
    let true_up: DVec3 = back.cross(right);
    DMat4::from_cols(
        right.extend(0.0),
        true_up.extend(0.0),
        back.extend(0.0),
        position.extend(1.0),
    )
}

/// `CitySession` is the window's game: the level, this player and the network.
struct CitySession {
    world: CityWorld,
    scene: Arc<CityScene>,
    body: AvatarModel,
    settings: ServerSettings,
    player_settings: PerPlayerSettings,
    player_settings_path: Option<PathBuf>,
    menu: Option<menu::Menu>,
    client: net::Client,
    server: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
    player: CityPlayer,
    is_spawned: bool,
    respawn_in_s: Option<f64>,
    spawn_count: usize,
    shots: Vec<LiveShot>,
    reload_left_s: f64,
    laser_left_s: f64,
    recoil_left_s: f64,
    step_phase_rad: f64,
    run_frac: f64,
    remote_lasers: HashMap<u32, bool>,
    remote_motion: HashMap<u32, Motion>,
    disabling: Option<u8>,
    disabling_s: f64,
    start: Instant,
    last_s: f64,
    is_captured: bool,
    render_scale: f64,
    message: Option<(String, f64)>,
    brain: Option<Brain>,
    frame_ms: Vec<f64>,
    gpu_ms: Vec<f64>,
    last_frame: Option<Instant>,
    hit_count: u32,
    rifle_log: RifleLog,
    was_trigger_held: bool,
    map_base: std::cell::OnceCell<Vec<u8>>,
    hud: (f64, u32, f64),
    is_aiming: bool,
    phase_ms: Vec<[f64; PHASE_COUNT]>,
    gun: Gun,
    rifle_heat_frac: f64,
    is_overheated: bool,
    health_pulses: hud_bars::BarPulses,
    energy_pulses: hud_bars::BarPulses,
    hit_effect_frac: f64,
    last_health_frac: f64,
    slowed_s: f64,
    frame_count: u64,
}

impl CitySession {
    /// `New` connects to `--join`'s server, or starts one on `--port` and connects to it.
    #[allow(non_snake_case)]
    fn New(arguments: &Arguments) -> Result<CitySession> {
        let world: CityWorld = CityWorld::Builtin()?;
        let settings: ServerSettings = load_settings(arguments)?;
        let player_settings_path: Option<PathBuf> =
            arguments.player_settings.clone().or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(PLAYER_SETTINGS_HOME_PATH))
            });
        let player_settings: PerPlayerSettings = match &player_settings_path {
            Some(path) if arguments.player_settings.is_some() || path.exists() => {
                PerPlayerSettings::Load(path)?
            }
            _ => PerPlayerSettings::Builtin(),
        };
        let password: Vec<u8> = net::read_game_password(arguments.game_password_file.as_deref())?;
        let (server, address) = match &arguments.join {
            Some(_) => (None, get_server_address(arguments)?),
            None => (
                Some(start_server(
                    arguments.port,
                    settings.clone(),
                    &world,
                    password.clone(),
                    None,
                    !arguments.play_route,
                )?),
                SocketAddr::from((Ipv4Addr::LOCALHOST, arguments.port)),
            ),
        };
        let mut client: net::Client = net::Client::Connect(address, MAP_NAME, &password)?;
        let team: Option<u32> = arguments
            .team
            .map(TeamName::get_index)
            .or(arguments.play_route.then_some(0));
        if let Some(team) = team {
            client.join(Some(team))?;
        }
        let player: CityPlayer = world.make_player(team.unwrap_or(0), 0);
        Ok(CitySession {
            scene: Arc::new(build_scene(&world)),
            body: AvatarModel::Load(BODY_MODEL, AVATAR_HEIGHT_M)?,
            brain: match (arguments.play_route, arguments.route) {
                (false, _) => None,
                (true, RouteKind::Rifle) => Some(Brain::New(make_rifle_route())),
                (true, RouteKind::Stairs) => Some(Brain::New(make_stairs_route(&world, 0)?)),
                (true, _) => Some(Brain::New(make_route(&world))),
            },
            world,
            settings,
            player_settings,
            client,
            server,
            player,
            is_spawned: false,
            respawn_in_s: None,
            spawn_count: 0,
            shots: Vec::new(),
            reload_left_s: 0.0,
            laser_left_s: 0.0,
            recoil_left_s: 0.0,
            step_phase_rad: 0.0,
            run_frac: 0.0,
            remote_lasers: HashMap::new(),
            remote_motion: HashMap::new(),
            disabling: None,
            disabling_s: 0.0,
            start: Instant::now(),
            last_s: 0.0,
            is_captured: false,
            render_scale: arguments.render_scale.unwrap_or(1.0),
            message: None,
            frame_ms: Vec::new(),
            gpu_ms: Vec::new(),
            last_frame: None,
            hit_count: 0,
            rifle_log: RifleLog::default(),
            was_trigger_held: false,
            map_base: std::cell::OnceCell::new(),
            hud: (0.0, 0, 0.0),
            is_aiming: false,
            phase_ms: Vec::new(),
            player_settings_path,
            menu: None,
            gun: Gun::Laser,
            rifle_heat_frac: 0.0,
            is_overheated: false,
            health_pulses: hud_bars::BarPulses::New(1.0),
            energy_pulses: hud_bars::BarPulses::New(1.0),
            hit_effect_frac: 0.0,
            last_health_frac: 1.0,
            slowed_s: 0.0,
            frame_count: 0,
        })
    }

    /// `now_s` is the seconds since the window opened.
    fn now_s(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// `get_team` is this player's team (blue until the server says).
    fn get_team(&self) -> u32 {
        self.client.team.unwrap_or(0)
    }

    /// `show_message` shows a line for `MESSAGE_S`.
    fn show_message(&mut self, message: String) {
        println!("{message}");
        self.message = Some((message, self.now_s() + MESSAGE_S));
    }

    /// `get_targets` gives the other players a shot can hit, only `enemies_of`'s enemies if set.
    fn get_targets(&self, enemies_of: Option<u32>) -> Vec<Target> {
        self.client
            .players
            .iter()
            .filter(|p| {
                Some(p.id) != self.client.id && enemies_of.is_none_or(|team| p.team != team)
            })
            .map(|p| {
                let (feet, up) = CityPlayer::get_feet_up(p.frame, self.world.eye_height_m);
                Target { id: p.id, feet, up }
            })
            .collect()
    }

    /// `find_console` gives the other team's console `index` (or the nearest one if `None`) that
    /// is on and within reach.
    fn find_console(&self, index: Option<u8>) -> Option<u8> {
        let eye: DVec3 = self.player.get_eye_frame().w_axis.truncate();
        let reach_m: f64 = self.settings.consoles.reach_m + CONSOLE_REACH_SLACK_M;
        self.world
            .consoles
            .iter()
            .filter(|c| c.team != self.get_team() && self.client.game.off_mask >> c.index & 1 == 0)
            .filter(|c| index.is_none_or(|i| i == c.index))
            .map(|c| (c.index, (self.world.get_console_center(c) - eye).length()))
            .filter(|&(_, d)| d <= reach_m)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// `get_brain_sight` gives the route's brain the rifle's heat, the nearest enemy's head and
    /// body centres, and this player's eliminations.
    fn get_brain_sight(&self) -> BrainSight {
        let eye: DVec3 = self.player.get_eye_frame().w_axis.truncate();
        let team: u32 = self.get_team();
        let enemy: Option<(DVec3, DVec3)> = self
            .client
            .players
            .iter()
            .filter(|p| Some(p.id) != self.client.id && p.team != team)
            .map(|p| CityPlayer::get_feet_up(p.frame, self.world.eye_height_m))
            .min_by(|a, b| (a.0 - eye).length().total_cmp(&(b.0 - eye).length()))
            .map(|(feet, up)| (feet + up * HEAD_HEIGHT_M, feet + up * TORSO_HEIGHT_M));
        BrainSight {
            is_overheated: self.is_overheated,
            heat_frac: self.rifle_heat_frac,
            enemy,
            kill_count: self.rifle_log.kills.len() as u32,
        }
    }

    /// `fire` shoots from the eye along the crosshair, telling the server about a hit.
    fn fire(&mut self) -> Result<()> {
        let eye: DMat4 = self.player.get_eye_frame();
        let team: u32 = self.get_team();
        let targets: Vec<Target> = self.get_targets(Some(team));
        let shot: Shot = self.world.trace_shot(
            eye.w_axis.truncate(),
            -eye.z_axis.truncate(),
            self.settings.glass.range_m,
            self.settings.glass.max_bounce_count,
            &targets,
            self.client.id,
        );
        if let ShotHit::Player { id, is_head } = shot.hit {
            let hits_to_kill: u8 = match (self.gun, is_head) {
                (Gun::Laser, _) => self.settings.sniper.hits_to_eliminate,
                (Gun::Rifle, true) => self.settings.rifle.head_hits_to_eliminate,
                (Gun::Rifle, false) => self.settings.rifle.body_hits_to_eliminate,
            };
            self.client.send_hit(id, hits_to_kill)?;
            self.hit_count += 1;
            if self.gun == Gun::Rifle {
                if is_head {
                    self.rifle_log.head_hit_count += 1;
                } else {
                    self.rifle_log.body_hit_count += 1;
                }
            }
            self.show_message(format!(
                "You hit player {id} ({} bounces)",
                shot.bounce_count
            ));
        }
        self.shots.push(LiveShot {
            shot,
            start_s: self.now_s(),
            team,
            is_own: true,
        });
        match self.gun {
            Gun::Laser => {
                self.reload_left_s = self.settings.sniper.reload_s;
                self.laser_left_s = self.settings.sniper.shot_s;
            }
            Gun::Rifle => {
                let rifle = &self.settings.rifle;
                self.reload_left_s = 1.0 / rifle.shots_per_s;
                self.laser_left_s = rifle.pulse_s;
                self.rifle_heat_frac += 1.0 / f64::from(rifle.shots_until_overheat);
                self.is_overheated = self.rifle_heat_frac >= 1.0;
                self.rifle_log.cold_shot_count += 1;
                if self.is_overheated && self.rifle_log.shots_until_overheat.is_none() {
                    self.rifle_log.shots_until_overheat = Some(self.rifle_log.cold_shot_count);
                    self.rifle_log.overheated_at_s = Some(self.now_s());
                }
            }
        }
        self.recoil_left_s = RECOIL_S;
        Ok(())
    }

    /// `update` advances the game by the time since the last update: network, movement (from the
    /// held actions, or the route), shots, consoles and the robots' motion.
    fn update(&mut self, held: &[Action]) -> Result<()> {
        let now_s: f64 = self.now_s();
        let dt_s: f64 = (now_s - self.last_s).clamp(0.0, MAX_DT_S);
        self.last_s = now_s;
        for event in self.client.poll()? {
            match event {
                net::NetEvent::Joined(id) => {
                    self.player = self.world.make_player(self.get_team(), self.spawn_count);
                    self.spawn_count += 1;
                    self.is_spawned = true;
                    if let Some(brain) = &mut self.brain {
                        brain.restart();
                    }
                    let team: &str = net::TEAM_NAMES[self.get_team() as usize % 2];
                    self.show_message(format!("You are player {id}, on the {team} team"));
                }
                net::NetEvent::Eliminated {
                    victim_id,
                    shooter_id,
                    is_self,
                } => {
                    if !is_self && Some(shooter_id) == self.client.id {
                        self.rifle_log
                            .kills
                            .push([self.rifle_log.head_hit_count, self.rifle_log.body_hit_count]);
                        (self.rifle_log.head_hit_count, self.rifle_log.body_hit_count) = (0, 0);
                    }
                    if is_self {
                        self.is_spawned = false;
                        self.respawn_in_s = Some(self.settings.respawn.delay_s);
                        self.disabling = None;
                    }
                    self.show_message(format!("player {shooter_id} hit player {victim_id}"));
                }
            }
        }
        if let Some(settings) = &self.client.settings
            && *settings != self.settings
        {
            self.settings = settings.clone();
        }
        step_respawn(&mut self.client, &mut self.respawn_in_s, dt_s)?;
        self.update_hit(dt_s);
        self.reload_left_s = (self.reload_left_s - dt_s).max(0.0);
        self.laser_left_s = (self.laser_left_s - dt_s).max(0.0);
        self.recoil_left_s = (self.recoil_left_s - dt_s).max(0.0);
        if self.gun == Gun::Laser || !self.was_trigger_held || self.is_overheated {
            self.rifle_heat_frac =
                (self.rifle_heat_frac - dt_s / self.settings.rifle.cool_down_s).max(0.0);
            self.is_overheated &= self.rifle_heat_frac > 0.0;
            if let Some(at_s) = self.rifle_log.overheated_at_s
                && !self.is_overheated
                && self.rifle_log.cool_down_s.is_none()
            {
                self.rifle_log.cool_down_s = Some(now_s - at_s);
            }
        }
        let is_playing: bool = self.is_spawned && self.client.game.winner.is_none();
        let mut input = CityInput {
            speed_frac: self.get_speed_frac(),
            ..CityInput::default()
        };
        let (mut wants_shot, mut console) = (false, None);
        let sight: BrainSight = self.get_brain_sight();
        if let Some(brain) = &mut self.brain {
            if self.is_spawned {
                let out: BrainOutput = brain.drive(
                    &self.world,
                    &mut self.player,
                    dt_s,
                    &self.client.game,
                    is_playing && self.reload_left_s <= 0.0,
                    &sight,
                );
                input = CityInput {
                    speed_frac: self.get_speed_frac(),
                    ..out.input
                };
                if out.wants_rifle {
                    self.gun = Gun::Rifle;
                }
                wants_shot = out.is_shooting;
                console = out.console.and_then(|index| self.find_console(Some(index)));
            }
        } else if self.is_spawned && self.menu.is_none() {
            let is = |action: Action| held.contains(&action);
            let axis = |plus: Action, minus: Action| {
                f64::from(u8::from(is(plus))) - f64::from(u8::from(is(minus)))
            };
            input.forward_frac = axis(Action::MoveForward, Action::MoveBack);
            input.right_frac = axis(Action::MoveRight, Action::MoveLeft);
            input.is_sprinting = is(Action::Sprint);
            input.is_jumping = is(Action::Jump);
            wants_shot = is(Action::Shoot) && self.is_captured;
            if is(Action::TurnOffConsole) {
                console = self.find_console(None);
            }
        }
        if self.is_spawned {
            let before: DVec3 = self.player.feet;
            self.world
                .move_player(&mut self.player, &input, &self.settings, dt_s);
            let speed_m_per_s: f64 = (self.player.feet - before).length() / dt_s.max(1e-6);
            self.step_phase_rad += speed_m_per_s * dt_s * STEPS_RAD_PER_M;
            self.run_frac = (speed_m_per_s / self.world.get_speed_m_per_s(&self.settings, true)).min(1.0);
        }
        self.health_pulses.update(
            dt_s,
            self.client.health_frac,
            u32::from(self.settings.rifle.body_hits_to_eliminate),
            &hud_bars::PULSE_TUNING,
        );
        self.energy_pulses.update(
            dt_s,
            self.player.energy_frac,
            ENERGY_SEGMENT_COUNT,
            &hud_bars::PULSE_TUNING,
        );
        self.was_trigger_held = wants_shot;
        if wants_shot
            && is_playing
            && self.reload_left_s <= 0.0
            && !(self.gun == Gun::Rifle && self.is_overheated)
        {
            self.fire()?;
        }
        match console.filter(|_| is_playing) {
            Some(index) if self.disabling == Some(index) => self.disabling_s += dt_s,
            Some(index) => (self.disabling, self.disabling_s) = (Some(index), 0.0),
            None => (self.disabling, self.disabling_s) = (None, 0.0),
        }
        self.update_remote(now_s, dt_s);
        if self.is_spawned && self.client.id.is_some() {
            self.client.send_state(
                self.player.get_eye_frame(),
                self.laser_left_s > 0.0,
                self.gun == Gun::Rifle,
                self.disabling,
                0.0,
                PlayerWormholes::default(),
            )?;
        }
        let life_s: f64 = self.settings.sniper.shot_s + self.settings.sniper.beam_trail_s;
        self.shots.retain(|shot| now_s - shot.start_s < life_s);
        if self
            .message
            .as_ref()
            .is_some_and(|(_, until_s)| now_s > *until_s)
        {
            self.message = None;
        }
        Ok(())
    }

    /// `update_hit` starts Lunar Surface's hit effect and slows this player
    /// (`when_hit.slow_s`) when their health drops, whatever hurt them, and fades both over `dt_s`.
    fn update_hit(&mut self, dt_s: f64) {
        if self.client.health_frac < self.last_health_frac {
            self.hit_effect_frac = 1.0;
            self.slowed_s = self.settings.when_hit.slow_s;
        }
        self.last_health_frac = self.client.health_frac;
        self.hit_effect_frac =
            (self.hit_effect_frac - dt_s / self.player_settings.hit_effects.effect_s).max(0.0);
        self.slowed_s = (self.slowed_s - dt_s).max(0.0);
    }

    /// `get_speed_frac` is this player's speed as a fraction: `when_hit.slow_speed_frac` while
    /// slowed by a hit, else 1.
    fn get_speed_frac(&self) -> f64 {
        if self.slowed_s > 0.0 {
            self.settings.when_hit.slow_speed_frac
        } else {
            1.0
        }
    }

    /// `get_hit_effect` gives what the last hit on this player still does to the view, as on
    /// Lunar Surface: red edges, a blur from the edges inward, and a shake in a new direction each
    /// frame (the golden angle apart), fading over the player settings' `hit_effects.effect_s`.
    fn get_hit_effect(&self) -> render_metal::HitEffect {
        let when_hit = &self.player_settings.hit_effects;
        let shake_rad: f64 = self.frame_count as f64 * SPAWN_TURN_RAD;
        let shake_px: f64 = when_hit.shake_px * self.hit_effect_frac;
        render_metal::HitEffect {
            strength_frac: self.hit_effect_frac as f32,
            red_frac: when_hit.red_frac as f32,
            blur_px: when_hit.blur_px as f32,
            shake_px: [shake_rad.cos(), shake_rad.sin()].map(|c| (c * shake_px) as f32),
        }
    }

    /// `get_bar_rects` gives the HUD's bars in the bottom left, as on Lunar Surface
    /// (`hud_bars.rs`): this player's health (one segment per rifle body hit), the gun in hand's
    /// ammo (the sniper's one block, empty while it reloads, or the rifle's shots left before it
    /// overheats) and the sprint energy, health and energy with pulses while they change; and below
    /// the middle, the progress of turning off a console while holding `E` on one.
    fn get_bar_rects(&self) -> Vec<render_metal::OverlayRect> {
        let mut rects: Vec<render_metal::OverlayRect> = Vec::new();
        let health_segment_count: u32 = u32::from(self.settings.rifle.body_hits_to_eliminate);
        rects.extend(hud_bars::make_segmented_bar(
            HEALTH_BAR_MIN_FRAC,
            health_segment_count,
            self.client.health_frac,
            HEALTH_BAR_RGB,
            self.health_pulses.make_rect_pulse(
                HEALTH_BAR_MIN_FRAC,
                health_segment_count,
                self.client.health_frac,
                HEALTH_PULSE_RGB,
                HEALTH_BAR_RGB,
                &hud_bars::PULSE_TUNING,
            ),
        ));
        rects.extend(match self.gun {
            Gun::Laser => hud_bars::make_segmented_bar(
                HEAT_BAR_MIN_FRAC,
                1,
                if self.reload_left_s <= 0.0 { 1.0 } else { 0.0 },
                HEAT_BAR_RGB,
                render_metal::NO_RECT_PULSE,
            ),
            Gun::Rifle => hud_bars::make_segmented_bar(
                HEAT_BAR_MIN_FRAC,
                self.settings.rifle.shots_until_overheat,
                1.0 - self.rifle_heat_frac.min(1.0),
                if self.is_overheated {
                    HEAT_BAR_OVERHEATED_RGB
                } else {
                    HEAT_BAR_RGB
                },
                render_metal::NO_RECT_PULSE,
            ),
        });
        rects.extend(hud_bars::make_segmented_bar(
            ENERGY_BAR_MIN_FRAC,
            ENERGY_SEGMENT_COUNT,
            self.player.energy_frac,
            ENERGY_BAR_RGB,
            self.energy_pulses.make_rect_pulse(
                ENERGY_BAR_MIN_FRAC,
                ENERGY_SEGMENT_COUNT,
                self.player.energy_frac,
                ENERGY_PULSE_RGB,
                ENERGY_BAR_RGB,
                &hud_bars::PULSE_TUNING,
            ),
        ));
        if self.disabling.is_some() {
            rects.extend(hud_bars::make_progress_bar(
                DISABLE_BAR_MIN_FRAC,
                DISABLE_BAR_SIZE_FRAC,
                (self.disabling_s / self.settings.consoles.hold_to_disable_s).min(1.0) as f32,
                DISABLE_BAR_RGB,
            ));
        }
        rects
    }

    /// `update_remote` traces the other players' new shots and follows their robots' motion.
    fn update_remote(&mut self, now_s: f64, dt_s: f64) {
        let mut new_shots: Vec<LiveShot> = Vec::new();
        for p in self
            .client
            .players
            .iter()
            .filter(|p| Some(p.id) != self.client.id)
        {
            let was_on: bool = self
                .remote_lasers
                .insert(p.id, p.is_laser_on)
                .unwrap_or(false);
            let (feet, _) = CityPlayer::get_feet_up(p.frame, self.world.eye_height_m);
            if p.is_laser_on && !was_on {
                let targets: Vec<Target> = self
                    .get_targets(None)
                    .into_iter()
                    .filter(|t| t.id != p.id)
                    .collect();
                let shot: Shot = self.world.trace_shot(
                    p.frame.w_axis.truncate(),
                    -p.frame.z_axis.truncate(),
                    self.settings.glass.range_m,
                    self.settings.glass.max_bounce_count,
                    &targets,
                    Some(p.id),
                );
                new_shots.push(LiveShot {
                    shot,
                    start_s: now_s,
                    team: p.team,
                    is_own: false,
                });
            }
            let motion: &mut Motion = self.remote_motion.entry(p.id).or_insert(Motion {
                feet,
                speed_m_per_s: 0.0,
            });
            let speed: f64 = (feet - motion.feet).length() / dt_s.max(1e-6);
            motion.speed_m_per_s += (speed - motion.speed_m_per_s) * (dt_s * 8.0).min(1.0);
            motion.feet = feet;
        }
        self.shots.extend(new_shots);
    }

    /// `get_eye` is the camera: the player's eye, or a slow orbit over the glass level while not
    /// in the game.
    fn get_eye(&self) -> DMat4 {
        if self.is_spawned {
            return self.player.get_eye_frame();
        }
        let angle: f64 = self.now_s() * LOBBY_ORBIT_RAD_PER_S;
        let radius_m: f64 =
            self.world.get_glass().map_or(10.0, |g| g.radius_m) - self.world.eye_height_m;
        let position: DVec3 = DVec3::new(angle.cos(), 0.25, angle.sin()).normalize() * radius_m;
        let forward: DVec3 =
            (DVec3::new(-angle.sin(), 0.0, angle.cos()) * 0.6 - position.normalize()).normalize();
        make_eye(position, forward, get_up(position))
    }

    /// `pack` makes this frame at `render_px`.
    fn pack(&self, render_px: (usize, usize)) -> CityFrame {
        let eye: DMat4 = self.get_eye();
        let now_s: f64 = self.now_s();
        let robots: Vec<Robot> = self
            .client
            .players
            .iter()
            .filter(|p| Some(p.id) != self.client.id)
            .map(|p| {
                let motion: Motion = self.remote_motion.get(&p.id).copied().unwrap_or(Motion {
                    feet: p.frame.w_axis.truncate(),
                    speed_m_per_s: 0.0,
                });
                let clip: AvatarClip = if motion.speed_m_per_s > RUN_M_PER_S {
                    AvatarClip::Run
                } else if motion.speed_m_per_s > WALK_M_PER_S {
                    AvatarClip::Walk
                } else {
                    AvatarClip::Idle
                };
                Robot {
                    feet: motion.feet,
                    forward: -p.frame.z_axis.truncate(),
                    team: p.team,
                    clip,
                }
            })
            .collect();
        let life_s: f64 = self.settings.sniper.shot_s + self.settings.sniper.beam_trail_s;
        let muzzle: DVec3 = (eye * MUZZLE_M.extend(1.0)).truncate();
        let beams: Vec<Beam> = self
            .shots
            .iter()
            .map(|s| {
                let mut points: Vec<DVec3> = s.shot.points.clone();
                if s.is_own && self.is_spawned {
                    points[0] = muzzle;
                }
                (
                    points,
                    s.team,
                    (1.0 - (now_s - s.start_s) / life_s).clamp(0.0, 1.0),
                )
            })
            .collect();
        let disabling_mask: u32 = self
            .client
            .players
            .iter()
            .filter_map(|p| p.disabling_console)
            .chain(self.disabling)
            .fold(0, |mask, index| mask | 1 << index);
        let mut frame: CityFrame = make_frame(
            &self.world,
            &self.scene,
            &self.body,
            eye,
            &robots,
            &beams,
            (self.client.game.off_mask, disabling_mask),
            now_s,
            render_px,
        );
        if self.is_spawned && self.brain.is_none() && self.is_aiming {
            frame.zoom_in(self.settings.aim.sniper_zoom);
        }
        frame
    }

    /// `get_banner_lines` gives the two centred lines to show, if any: the lobby, joining, a win,
    /// "YOU DIED" with the countdown to respawning, or a message. Turning off a console shows as a
    /// bar (`get_bar_rects`), as on Lunar Surface.
    fn get_banner_lines(&self) -> Option<[String; 2]> {
        let keys: &KeyBindings = &self.player_settings.keys;
        if self.client.is_in_lobby() {
            return Some(get_lobby_lines(
                self.client.lobby_counts,
                self.client.get_server(),
                format!(
                    "{} joins blue, {} joins red",
                    keys.get_label(Action::JoinBlue),
                    keys.get_label(Action::JoinRed)
                ),
            ));
        }
        if let Some(winner) = self.client.game.winner {
            let loser: usize = (winner as usize + 1) % 2;
            return Some([
                format!(
                    "{} TEAM WINS",
                    net::TEAM_NAMES[winner as usize % 2].to_uppercase()
                ),
                format!("every {} console is off", net::TEAM_NAMES[loser]),
            ]);
        }
        if let Some(left_s) = self.respawn_in_s {
            return Some([
                "YOU DIED".to_string(),
                format!("Respawning in {:.0}", left_s.ceil().max(1.0)),
            ]);
        }
        if !self.is_spawned {
            return Some([
                "JOINING THE GAME".to_string(),
                format!("server {}", self.client.get_server()),
            ]);
        }
        self.message
            .as_ref()
            .map(|(message, _)| [message.clone(), String::new()])
    }

    /// `get_hud_title` is the window title's status line.
    fn get_hud_title(&self, render_px: (usize, usize), fps: f64, gpu_ms: f64) -> String {
        let team: &str = net::TEAM_NAMES[self.get_team() as usize % 2];
        let level: &str = match self.world.get_side(self.player.feet) {
            Side::Street => "street",
            Side::Glass => "glass level",
        };
        let on = |team: u32| {
            self.world
                .consoles
                .iter()
                .filter(|c| c.team == team && self.client.game.off_mask >> c.index & 1 == 0)
                .count()
        };
        format!(
            "{WINDOW_TITLE}: {team} | {level} | consoles on: blue {} red {} | {} | {fps:.0} fps, GPU {gpu_ms:.2} ms at {}x{}",
            on(0),
            on(1),
            match self.gun {
                Gun::Laser if self.reload_left_s > 0.0 =>
                    format!("laser reloading {:.1} s", self.reload_left_s),
                Gun::Laser => "laser ready".to_string(),
                Gun::Rifle if self.is_overheated =>
                    format!("rifle overheated {:.0}%", self.rifle_heat_frac * 100.0),
                Gun::Rifle => format!("rifle heat {:.0}%", self.rifle_heat_frac * 100.0),
            },
            render_px.0,
            render_px.1
        )
    }

    /// `record_frame` keeps a frame's time since the last and its GPU time (for the route's numbers).
    fn record_frame(&mut self, gpu_ms: f64) {
        let now: Instant = Instant::now();
        if let Some(last) = self.last_frame {
            self.frame_ms.push((now - last).as_secs_f64() * MS_PER_S);
            self.gpu_ms.push(gpu_ms);
        }
        self.last_frame = Some(now);
    }
}

impl Drop for CitySession {
    fn drop(&mut self) {
        let _ = self.client.leave();
        if let Some((should_stop, _)) = &self.server {
            should_stop.store(true, Ordering::Relaxed);
        }
    }
}

/// `get_percentile_ms` gives the `frac` percentile of `values` (0 if empty).
fn get_percentile_ms(values: &[f64], frac: f64) -> f64 {
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
        .get(((sorted.len() as f64 - 1.0) * frac).round().max(0.0) as usize)
        .copied()
        .unwrap_or(0.0)
}

/// `play` opens the window and runs the game until it closes (or the route ends).
fn play(arguments: &Arguments, metal: &mut render_metal::MetalRenderer) -> Result<()> {
    let sdl: sdl3::Sdl = sdl3::init()?;
    let video: sdl3::VideoSubsystem = sdl.video()?;
    let mut session: CitySession = CitySession::New(arguments)?;
    let mut builder: sdl3::video::WindowBuilder =
        video.window(WINDOW_TITLE, arguments.width_px, arguments.height_px);
    builder.high_pixel_density().resizable().position_centered();
    let flags: u32 = builder.window_flags() | sdl3::sys::video::SDL_WINDOW_METAL.0 as u32;
    let mut window: sdl3::video::Window = builder.set_window_flags(flags).build()?;
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
    let view: sdl3::sys::metal::SDL_MetalView =
        unsafe { sdl3::sys::metal::SDL_Metal_CreateView(window.raw()) };
    if view.is_null() {
        return Err(format!("SDL: couldn't make a Metal view: {}", sdl3::get_error()).into());
    }
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A DANGLING LAYER WILL HAPPEN
    let layer: &CAMetalLayer =
        unsafe { &*sdl3::sys::metal::SDL_Metal_GetLayer(view).cast::<CAMetalLayer>() };
    layer.setDevice(Some(metal.get_device()));
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setFramebufferOnly(false);
    layer.setDisplaySyncEnabled(!arguments.no_vsync);
    for step in render_metal::get_loading_steps(false) {
        metal.load(*step)?;
    }
    let mouse: sdl3::mouse::MouseUtil = sdl.mouse();
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let result: Result<()> = (|| loop {
        let mut phase_start: Instant = Instant::now();
        let mut phases: [f64; PHASE_COUNT] = [0.0; PHASE_COUNT];
        let mut lap = |k: usize| {
            phases[k] = phase_start.elapsed().as_secs_f64() * MS_PER_S;
            phase_start = Instant::now();
        };
        for event in event_pump.poll_iter() {
            if !handle_event(&mut session, event, &window, &mouse)? {
                return Ok(());
            }
        }
        let held: Vec<Action> = {
            let (keyboard, mouse_state) = (event_pump.keyboard_state(), event_pump.mouse_state());
            KeyBindings::get_all()
                .iter()
                .map(|(action, _)| *action)
                .filter(|action| {
                    session
                        .player_settings
                        .keys
                        .is_held(*action, &keyboard, &mouse_state)
                })
                .collect()
        };
        session.is_aiming = held.contains(&Action::Aim);
        lap(0);
        session.update(&held)?;
        lap(1);
        let now_s: f64 = session.now_s();
        if session.brain.as_ref().is_some_and(Brain::is_done)
            || now_s > ROUTE_TIMEOUT_S && session.brain.is_some()
        {
            print_route_report(&session, arguments, window.size_in_pixels());
            return Ok(());
        }
        if arguments.exit_after_s.is_some_and(|s| now_s > s) {
            return Ok(());
        }
        let window_px: (u32, u32) = window.size_in_pixels();
        layer.setDrawableSize(CGSize {
            width: f64::from(window_px.0),
            height: f64::from(window_px.1),
        });
        let render_px: (usize, usize) = (
            ((f64::from(window_px.0) * session.render_scale) as usize).max(1),
            ((f64::from(window_px.1) * session.render_scale) as usize).max(1),
        );
        let frame: CityFrame = session.pack(render_px);
        lap(2);
        let text: Option<OverlayText> = get_menu_text(&session).or_else(|| {
            session
                .get_banner_lines()
                .map(|[title, detail]| OverlayText {
                    lines: vec![(title, true), (detail, false)],
                    mask_px: DEATH_MASK_PX,
                    height_frac: BANNER_HEIGHT_FRAC,
                    is_centered: true,
                    dim_frac: if session.respawn_in_s.is_some() {
                        DEATH_DIM_FRAC
                    } else {
                        0.0
                    },
                    picture: None,
                })
        });
        let mask: Option<Vec<u8>> = text.as_ref().map(make_text_mask).transpose()?;
        let mut rects: Vec<render_metal::OverlayRect> = if session.is_spawned {
            get_crosshair_rects(window_px)
        } else {
            Vec::new()
        };
        if session.is_spawned && session.menu.is_none() {
            rects.extend(session.get_bar_rects());
        }
        if session.is_spawned && session.menu.is_none() && session.player_settings.minimap.is_shown
        {
            rects.extend(get_minimap_rects(&session, window_px));
        }
        let wand = session.is_spawned.then(|| {
            let recoil_frac: f64 = session.recoil_left_s / RECOIL_S;
            (
                make_wand(
                    session.run_frac,
                    session.step_phase_rad,
                    recoil_frac,
                    session.get_team(),
                    session.gun,
                    (0.0, 0.0),
                ),
                session.gun.get_model(),
            )
        });
        let overlay = render_metal::Overlay {
            dim_frac: text.as_ref().map_or(0.0, |t| t.dim_frac),
            mask: mask
                .as_deref()
                .zip(text.as_ref())
                .map(|(pixels, t)| (pixels, (t.mask_px.0 as usize, t.mask_px.1 as usize))),
            mask_height_frac: text.as_ref().map_or(0.0, |t| t.height_frac),
            picture: text.as_ref().and_then(|t| t.picture.as_ref()).map(|p| {
                render_metal::OverlayPicture {
                    pixels: &p.pixels,
                    size_px: (p.size_px.0 as usize, p.size_px.1 as usize),
                    min_mask_px: p.min_mask_px,
                    max_excl_mask_px: p.max_excl_mask_px,
                }
            }),
            stretch_frac: 0.0,
            minimap: None,
            rects: &rects,
            wand,
            blur_frac: 0.0,
            hit: session.get_hit_effect(),
            blueshift_frac: 0.0,
        };
        session.frame_count += 1;
        lap(3);
        let is_shown: bool =
            metal.render_to_layer(FrameData::City(&frame), layer, Some(&overlay))?;
        lap(4);
        session.phase_ms.push(phases);
        if is_shown {
            let gpu_ms: f64 = metal.last_gpu_ms;
            session.record_frame(gpu_ms);
            let (time_s, count, gpu_sum) = &mut session.hud;
            *count += 1;
            *gpu_sum += gpu_ms;
            if now_s - *time_s > HUD_PERIOD_S {
                let fps: f64 = f64::from(*count) / (now_s - *time_s);
                let mean_gpu_ms: f64 = *gpu_sum / f64::from(*count);
                session.hud = (now_s, 0, 0.0);
                window.set_title(&session.get_hud_title(render_px, fps, mean_gpu_ms))?;
            }
        }
    })();
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A LEAKED OR DOUBLE-FREED METAL VIEW WILL HAPPEN
    unsafe { sdl3::sys::metal::SDL_Metal_DestroyView(view) };
    result
}

/// `handle_event` reacts to one window event; false to close.
fn handle_event(
    session: &mut CitySession,
    event: Event,
    window: &sdl3::video::Window,
    mouse: &sdl3::mouse::MouseUtil,
) -> Result<bool> {
    match event {
        Event::Quit { .. } => return Ok(false),
        Event::MouseButtonDown { .. } if !session.is_captured => {
            session.is_captured = true;
            mouse.set_relative_mouse_mode(window, true);
            return Ok(true);
        }
        Event::MouseMotion { xrel, yrel, .. }
            if session.is_captured && session.is_spawned && session.brain.is_none() =>
        {
            let zoom: f64 = if session.is_aiming {
                session.settings.aim.sniper_zoom
            } else {
                1.0
            };
            let rad_per_px: f64 = session.player_settings.view.look_rad_per_px / zoom;
            session
                .player
                .look_around(-f64::from(xrel) * rad_per_px, -f64::from(yrel) * rad_per_px);
        }
        _ => {}
    }
    let Some(control) = key_bindings::get_pressed_control(&event) else {
        return Ok(true);
    };
    let is = |action: Action| session.player_settings.keys.matches(action, control);
    if session.menu.is_some() {
        if is(Action::OpenMenu) {
            session.menu = None;
        } else {
            handle_menu_control(session, control);
        }
    } else if is(Action::OpenMenu) {
        session.menu = Some(menu::Menu::New());
    } else if is(Action::ReleaseMouse) {
        session.is_captured = false;
        mouse.set_relative_mouse_mode(window, false);
    } else if session.client.is_in_lobby() && (is(Action::JoinBlue) || is(Action::JoinRed)) {
        let team: u32 = u32::from(is(Action::JoinRed));
        session.client.join(Some(team))?;
    } else if is(Action::Gun1) || is(Action::Gun2) || is(Action::SwitchGun) {
        session.gun = if is(Action::Gun1) {
            Gun::Laser
        } else if is(Action::Gun2) || session.gun == Gun::Laser {
            Gun::Rifle
        } else {
            Gun::Laser
        };
        session.reload_left_s = session
            .reload_left_s
            .min(1.0 / session.settings.rifle.shots_per_s);
    } else if is(Action::LowerResolution) {
        session.render_scale = (session.render_scale - RENDER_SCALE_STEP).max(MIN_RENDER_SCALE);
    } else if is(Action::RaiseResolution) {
        session.render_scale = (session.render_scale + RENDER_SCALE_STEP).min(1.0);
    }
    Ok(true)
}

/// `handle_menu_control` passes a pressed key to the open menu: it changes a setting, moves, closes
/// the menu, or saves the player settings to `player_settings_path` and says where.
///
/// Example: `handle_menu_control(&mut session, control);`
fn handle_menu_control(session: &mut CitySession, control: key_bindings::Control) {
    let menu: &mut menu::Menu = session.menu.as_mut().expect("the menu is open");
    match menu.handle(control, &mut session.player_settings) {
        menu::MenuOutcome::Stay => {}
        menu::MenuOutcome::Close => session.menu = None,
        menu::MenuOutcome::Save => {
            let message: String = match save_player_settings(session) {
                Ok(path) => format!("Saved to {path}"),
                Err(error) => format!("Couldn't save: {error}"),
            };
            session
                .menu
                .as_mut()
                .expect("the menu is open")
                .show_message(message);
        }
    }
}

/// `save_player_settings` writes the player settings to `player_settings_path` and gives the path.
///
/// Example: `let path: String = save_player_settings(&session)?;`
fn save_player_settings(session: &CitySession) -> Result<String> {
    let path: &PathBuf = session
        .player_settings_path
        .as_ref()
        .ok_or("no file to save to: pass --player-settings (HOME isn't set)")?;
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(path, session.player_settings.to_yaml())?;
    Ok(path.display().to_string())
}

/// `get_menu_text` gives the open menu's page (settings, map, scores or credits) as overlay text,
/// with the server's scoreboard on the scores page, or `None` while the menu is closed.
///
/// Example: `let text: Option<OverlayText> = get_menu_text(&session);`
fn get_menu_text(session: &CitySession) -> Option<OverlayText> {
    let menu: &menu::Menu = session.menu.as_ref()?;
    let save_path: String = match &session.player_settings_path {
        Some(path) => path.display().to_string(),
        None => "nowhere: HOME isn't set, pass --player-settings".to_string(),
    };
    let scoreboard: menu::Scoreboard = match &session.client.scores {
        None => menu::Scoreboard::Waiting,
        Some(scores) => menu::Scoreboard::Scores {
            scores,
            own_id: session.client.welcomed_id,
        },
    };
    let mut lines: Vec<(String, bool)> =
        menu.get_lines(&session.player_settings, &save_path, scoreboard);
    let picture: Option<Picture> = if menu.get_page() == menu::Page::Map {
        lines.truncate(1);
        lines.push((CITY_MAP_LEGEND.to_string(), false));
        Some(make_map_picture(session))
    } else {
        None
    };
    Some(OverlayText {
        lines,
        mask_px: MENU_MASK_PX,
        height_frac: MENU_HEIGHT_FRAC,
        is_centered: false,
        dim_frac: MENU_DIM_FRAC,
        picture,
    })
}

/// `get_map_picture_px` gives the map page's picture size: the sphere unrolled (longitude across,
/// latitude down) at 2:1, as large as the page's map area holds.
fn get_map_picture_px() -> (usize, usize) {
    let [width_px, height_px] = [0, 1]
        .map(|a| (MAP_AREA_MAX_EXCL_MASK_PX[a] - MAP_AREA_MIN_MASK_PX[a]) * PICTURE_PX_PER_MASK_PX);
    let fitted_width_px: i32 = width_px.min(height_px * 2);
    (fitted_width_px as usize, (fitted_width_px / 2) as usize)
}

/// `get_map_direction` gives the direction from the centre that the map picture's pixel `(x, y)`
/// shows.
fn get_map_direction(x: usize, y: usize, size_px: (usize, usize)) -> DVec3 {
    let longitude_rad: f64 = ((x as f64 + 0.5) / size_px.0 as f64 - 0.5) * std::f64::consts::TAU;
    let latitude_rad: f64 = (0.5 - (y as f64 + 0.5) / size_px.1 as f64) * std::f64::consts::PI;
    DVec3::new(
        latitude_rad.cos() * longitude_rad.cos(),
        latitude_rad.sin(),
        latitude_rad.cos() * longitude_rad.sin(),
    )
}

/// `get_map_pixel` gives the map picture's pixel that shows the unit direction `d` from the centre.
fn get_map_pixel(d: DVec3, size_px: (usize, usize)) -> (usize, usize) {
    let longitude_rad: f64 = d.z.atan2(d.x);
    let latitude_rad: f64 = d.y.clamp(-1.0, 1.0).asin();
    let x: f64 = (longitude_rad / std::f64::consts::TAU + 0.5) * size_px.0 as f64;
    let y: f64 = (0.5 - latitude_rad / std::f64::consts::PI) * size_px.1 as f64;
    (
        (x as usize).min(size_px.0 - 1),
        (y as usize).min(size_px.1 - 1),
    )
}

/// `make_map_base` draws the map picture's still part as seen from the centre: the street,
/// buildings, the glass level's covers and the glass's holes to the street.
fn make_map_base(world: &CityWorld, size_px: (usize, usize)) -> Vec<u8> {
    let glass_radius_m: f64 = world.get_glass().map_or(0.0, |glass| glass.radius_m);
    let mut pixels: Vec<u8> = Vec::with_capacity(size_px.0 * size_px.1 * BYTES_PER_PIXEL);
    for y in 0..size_px.1 {
        for x in 0..size_px.0 {
            let d: DVec3 = get_map_direction(x, y, size_px);
            let nearest_m: Option<f64> = world
                .boxes
                .iter()
                .filter_map(|city_box| city_box.find_hit_m(DVec3::ZERO, d).map(|(t, _)| t))
                .min_by(f64::total_cmp);
            let color: [u8; 4] = match nearest_m {
                Some(t) if t < glass_radius_m + MAP_COVER_ABOVE_GLASS_M => MAP_COVER_COLOR,
                Some(_) => MAP_BUILDING_COLOR,
                None if world.get_glass().is_some_and(|glass| glass.is_in_hole(d)) => {
                    MAP_HOLE_COLOR
                }
                None => MAP_STREET_COLOR,
            };
            pixels.extend_from_slice(&color);
        }
    }
    pixels
}

/// `get_map_picture_path` gives where `--map-picture` saves the map page's picture.
fn get_map_picture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MAP_PICTURE_PATH)
}

/// `load_map_picture` reads the rendered map picture (`--map-picture`), or gives `None` when it is
/// missing or not `size_px`, so the map page falls back to `make_map_base`'s drawing.
fn load_map_picture(size_px: (usize, usize)) -> Option<Vec<u8>> {
    let picture: image::RgbaImage = image::open(get_map_picture_path()).ok()?.to_rgba8();
    (picture.dimensions() == (size_px.0 as u32, size_px.1 as u32)).then(|| picture.into_raw())
}

/// `render_map_picture` renders the six faces of a cube around the centre (without the core, so
/// nothing blocks the view) with City Sphere's renderer, unrolls them into the map page's
/// longitude-latitude picture with `MAP_PICTURE_SUPERSAMPLE`^2 samples a pixel, and saves it to
/// `assets/map_picture.png`, with a copy and the old drawing under .artifacts/map_picture/.
fn render_map_picture(arguments: &Arguments, renderer: &mut dyn Renderer) -> Result<()> {
    let mut world: CityWorld = CityWorld::Builtin()?;
    let drawing: Vec<u8> = make_map_base(&world, get_map_picture_px());
    world
        .shells
        .retain(|shell| shell.is_walkable_inside || shell.is_reflective_inside);
    let scene: Arc<CityScene> = Arc::new(build_scene(&world));
    let body: AvatarModel = AvatarModel::Load(BODY_MODEL, AVATAR_HEIGHT_M)?;
    let face_px: usize = MAP_PICTURE_FACE_PX;
    let faces: [(DVec3, DVec3); 6] = [
        (DVec3::X, DVec3::Y),
        (DVec3::NEG_X, DVec3::Y),
        (DVec3::Y, DVec3::Z),
        (DVec3::NEG_Y, DVec3::Z),
        (DVec3::Z, DVec3::Y),
        (DVec3::NEG_Z, DVec3::Y),
    ];
    let mut face_pixels: Vec<(DMat4, Vec<u8>)> = Vec::new();
    for (forward, up) in faces {
        let eye: DMat4 = make_eye(DVec3::ZERO, forward, up);
        let mut frame: CityFrame = make_frame(
            &world,
            &scene,
            &body,
            eye,
            &[],
            &[],
            (0, 0),
            0.0,
            (face_px, face_px),
        );
        frame.tan_half_fov = 1.0;
        let mut pixels: Vec<u8> = vec![0; face_px * face_px * BYTES_PER_PIXEL];
        renderer.render(
            FrameData::City(&frame),
            (face_px as u32, face_px as u32),
            &mut pixels,
        )?;
        face_pixels.push((eye, pixels));
    }
    let size_px: (usize, usize) = get_map_picture_px();
    let n: usize = MAP_PICTURE_SUPERSAMPLE;
    let fine_px: (usize, usize) = (size_px.0 * n, size_px.1 * n);
    let mut picture: Vec<u8> = Vec::with_capacity(size_px.0 * size_px.1 * BYTES_PER_PIXEL);
    for y in 0..size_px.1 {
        for x in 0..size_px.0 {
            let mut sum: [u32; 4] = [0; 4];
            for k in 0..n * n {
                let d: DVec3 = get_map_direction(x * n + k % n, y * n + k / n, fine_px);
                let (eye, pixels) = face_pixels
                    .iter()
                    .max_by(|a, b| {
                        d.dot(-a.0.z_axis.truncate())
                            .total_cmp(&d.dot(-b.0.z_axis.truncate()))
                    })
                    .expect("six faces");
                let z: f64 = d.dot(-eye.z_axis.truncate());
                let u: f64 = d.dot(eye.x_axis.truncate()) / z;
                let v: f64 = d.dot(eye.y_axis.truncate()) / z;
                let column: usize = (((u + 1.0) / 2.0 * face_px as f64) as usize).min(face_px - 1);
                let row: usize = (((1.0 - v) / 2.0 * face_px as f64) as usize).min(face_px - 1);
                let i: usize = (row * face_px + column) * BYTES_PER_PIXEL;
                for (channel, total) in sum.iter_mut().enumerate() {
                    *total += u32::from(pixels[i + channel]);
                }
            }
            picture.extend(sum.map(|total| (total / (n * n) as u32) as u8));
        }
    }
    for pixel in picture.chunks_mut(BYTES_PER_PIXEL) {
        pixel[3] = u8::MAX;
    }
    let path: PathBuf = get_map_picture_path();
    std::fs::create_dir_all(path.parent().ok_or("the map picture's folder")?)?;
    save_png(&path, &picture, size_px.0 as u32, size_px.1 as u32)?;
    let folder: PathBuf = make_run_folder("map_picture", arguments.label.as_deref())?;
    save_png(
        &folder.join("photo.png"),
        &picture,
        size_px.0 as u32,
        size_px.1 as u32,
    )?;
    save_png(
        &folder.join("drawing.png"),
        &drawing,
        size_px.0 as u32,
        size_px.1 as u32,
    )?;
    println!(
        "Map picture in {} (copies in {})",
        path.display(),
        folder.display()
    );
    Ok(())
}

/// `make_map_picture` gives the menu's map page picture: the whole sphere unrolled (the
/// rendered `assets/map_picture.png`, or else `make_map_base`'s drawing), with consoles as squares in their team's colour (dark once off), the other
/// players as dots in their team's colour and this player in white, centred in the page's map area.
fn make_map_picture(session: &CitySession) -> Picture {
    let size_px: (usize, usize) = get_map_picture_px();
    let mut pixels: Vec<u8> = session
        .map_base
        .get_or_init(|| {
            load_map_picture(size_px).unwrap_or_else(|| make_map_base(&session.world, size_px))
        })
        .clone();
    let mut draw_square = |point: DVec3, half_px: i32, color: [f32; 4]| {
        let (center_x, center_y) = get_map_pixel(point.normalize_or(DVec3::X), size_px);
        let rgba: [u8; 4] = color.map(|c| (c * 255.0) as u8);
        for y in (center_y as i32 - half_px)..=(center_y as i32 + half_px) {
            for x in (center_x as i32 - half_px)..=(center_x as i32 + half_px) {
                if x >= 0 && y >= 0 && (x as usize) < size_px.0 && (y as usize) < size_px.1 {
                    let i: usize = (y as usize * size_px.0 + x as usize) * BYTES_PER_PIXEL;
                    pixels[i..i + BYTES_PER_PIXEL].copy_from_slice(&rgba);
                }
            }
        }
    };
    for console in &session.world.consoles {
        let is_off: bool = session.client.game.off_mask >> console.index & 1 == 1;
        let color: [f32; 4] = if is_off {
            MINIMAP_OFF_COLOR
        } else {
            TEAM_COLORS[console.team as usize]
        };
        draw_square(
            session.world.get_console_center(console),
            MAP_CONSOLE_HALF_PX,
            color,
        );
    }
    for player in session
        .client
        .players
        .iter()
        .filter(|p| Some(p.id) != session.client.id)
    {
        draw_square(
            player.frame.w_axis.truncate(),
            MAP_PLAYER_HALF_PX,
            TEAM_COLORS[player.team as usize % 2],
        );
    }
    draw_square(session.player.feet, MAP_SELF_HALF_PX, MINIMAP_SELF_COLOR);
    let size_mask_px: [i32; 2] =
        [size_px.0 as i32, size_px.1 as i32].map(|n| n / PICTURE_PX_PER_MASK_PX);
    let min_mask_px: [i32; 2] = [0, 1].map(|a| {
        MAP_AREA_MIN_MASK_PX[a]
            + (MAP_AREA_MAX_EXCL_MASK_PX[a] - MAP_AREA_MIN_MASK_PX[a] - size_mask_px[a]) / 2
    });
    Picture {
        pixels,
        size_px: (size_px.0 as u32, size_px.1 as u32),
        min_mask_px,
        max_excl_mask_px: [0, 1].map(|a| min_mask_px[a] + size_mask_px[a]),
    }
}

/// `get_minimap_rects` draws the minimap in the window's top-right corner: the whole sphere
/// unrolled around the player (straight ahead is up, the antipode is the rim), with consoles as
/// squares in their team's colour (dark once off) and players as dots, the player in white.
///
/// Example: `rects.extend(get_minimap_rects(&session, window_px));`
fn get_minimap_rects(
    session: &CitySession,
    window_px: (u32, u32),
) -> Vec<render_metal::OverlayRect> {
    let (width_px, height_px): (f64, f64) = (f64::from(window_px.0), f64::from(window_px.1));
    let radius_px: f64 = height_px * MINIMAP_RADIUS_FRAC;
    let center_px: [f64; 2] = [
        width_px - radius_px - height_px * MINIMAP_MARGIN_FRAC,
        radius_px + height_px * MINIMAP_MARGIN_FRAC,
    ];
    let outward: DVec3 = session.player.feet.normalize();
    let right: DVec3 = session.player.body.x_axis;
    let forward: DVec3 = -session.player.body.z_axis;
    let place = |point: DVec3| -> [f64; 2] {
        let direction: DVec3 = point.normalize();
        let cos_angle: f64 = outward.dot(direction).clamp(-1.0, 1.0);
        let tangent: DVec3 = (direction - outward * cos_angle).normalize_or_zero();
        let r_px: f64 = radius_px * cos_angle.acos() / std::f64::consts::PI;
        [
            center_px[0] + tangent.dot(right) * r_px,
            center_px[1] - tangent.dot(forward) * r_px,
        ]
    };
    let rect = |at_px: [f64; 2], half_px: f64, color: [f32; 4]| render_metal::OverlayRect {
        min_frac: [
            ((at_px[0] - half_px) / width_px) as f32,
            ((at_px[1] - half_px) / height_px) as f32,
        ],
        max_frac: [
            ((at_px[0] + half_px) / width_px) as f32,
            ((at_px[1] + half_px) / height_px) as f32,
        ],
        color,
        style: render_metal::FLAT_RECT_STYLE,
        pulse: render_metal::NO_RECT_PULSE,
    };
    let mut rects: Vec<render_metal::OverlayRect> =
        vec![rect(center_px, radius_px, MINIMAP_BACKGROUND_COLOR)];
    for console in &session.world.consoles {
        let is_off: bool = session.client.game.off_mask >> console.index & 1 == 1;
        let color: [f32; 4] = if is_off {
            MINIMAP_OFF_COLOR
        } else {
            TEAM_COLORS[console.team as usize]
        };
        rects.push(rect(
            place(session.world.get_console_center(console)),
            height_px * MINIMAP_CONSOLE_FRAC,
            color,
        ));
    }
    for player in session
        .client
        .players
        .iter()
        .filter(|p| Some(p.id) != session.client.welcomed_id)
    {
        rects.push(rect(
            place(player.frame.w_axis.truncate()),
            height_px * MINIMAP_DOT_FRAC,
            TEAM_COLORS[player.team as usize % 2],
        ));
    }
    rects.push(rect(
        center_px,
        height_px * MINIMAP_DOT_FRAC,
        MINIMAP_SELF_COLOR,
    ));
    rects
}

/// `print_route_report` prints the route's numbers: frame times, GPU times, the winner and shots.
fn print_route_report(session: &CitySession, arguments: &Arguments, window_px: (u32, u32)) {
    let start: usize = WARMUP_FRAME_COUNT.min(session.frame_ms.len());
    let frame_ms: &[f64] = &session.frame_ms[start..];
    let gpu_ms: &[f64] = &session.gpu_ms[start..];
    let phases: &[[f64; PHASE_COUNT]] = &session.phase_ms[start.min(session.phase_ms.len())..];
    let slow: Vec<usize> = (0..frame_ms.len().min(phases.len().saturating_sub(1)))
        .filter(|&k| frame_ms[k] > SLOW_FRAME_MS)
        .collect();
    let phase_report: serde_json::Map<String, serde_json::Value> = PHASE_NAMES
        .iter()
        .enumerate()
        .map(|(k, name)| {
            let all: Vec<f64> = phases.iter().map(|p| p[k]).collect();
            let in_slow: Vec<f64> = slow.iter().map(|&i| phases[i + 1][k]).collect();
            let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
            (name.to_string(), json!({"median_ms": get_percentile_ms(&all, 0.5), "p99_ms": get_percentile_ms(&all, 0.99), "mean_in_slow_frames_ms": mean(&in_slow)}))
        })
        .collect();
    let report = json!({
        "window_px": [window_px.0, window_px.1],
        "slow_frame_count": slow.len(),
        "phases": phase_report,
        "frame_count": frame_ms.len(),
        "frame_ms_median": get_percentile_ms(frame_ms, 0.5),
        "frame_ms_p99": get_percentile_ms(frame_ms, 0.99),
        "gpu_ms_median": get_percentile_ms(gpu_ms, 0.5),
        "gpu_ms_p99": get_percentile_ms(gpu_ms, 0.99),
        "winner": session.client.game.winner.map(|team| net::TEAM_NAMES[team as usize % 2]),
        "off_mask": session.client.game.off_mask,
        "route_done": session.brain.as_ref().is_some_and(Brain::is_done),
        "seconds": session.now_s(),
        "rifle": json!({
            "shots_until_overheat": session.rifle_log.shots_until_overheat,
            "cool_down_s": session.rifle_log.cool_down_s,
            "kills_head_and_body_hits": session.rifle_log.kills,
        }),
    });
    if arguments.json {
        println!("{report}");
    } else {
        println!("Route report: {report:#}");
    }
}

/// `BenchView` is a named camera for `--bench-views`, with robots and shots to add to the worst views.
struct BenchView {
    name: &'static str,
    eye: DMat4,
    has_crowd: bool,
}

/// `make_bench_views` gives the views: the street at blue's spawn, looking up at the glass, the
/// glass level looking across, edge-on along the glass, up a lift, and the glass level with eight
/// robots and reflected shots (the worst).
fn make_bench_views(world: &CityWorld) -> Vec<BenchView> {
    let mut street: CityPlayer = world.make_player(0, 0);
    let glass_m: f64 = world.get_glass().map_or(10.0, |g| g.radius_m);
    let eye_m: f64 = world.eye_height_m;
    let view = |player: &CityPlayer, yaw_rad: f64, pitch_rad: f64| {
        let mut p: CityPlayer = *player;
        p.look_around(yaw_rad, 0.0);
        p.pitch_rad = pitch_rad;
        p.get_eye_frame()
    };
    street.align_to_ground();
    let mut glass: CityPlayer = street;
    glass.feet = DVec3::new(0.9, 0.2, -0.3).normalize() * glass_m;
    glass.align_to_ground();
    let mut lift: CityPlayer = street;
    lift.feet = world.get_glass().map_or(DVec3::X, |g| g.holes[0].0) * world.street_radius_m;
    lift.align_to_ground();
    let _ = eye_m;
    vec![
        BenchView {
            name: "street_spawn",
            eye: view(&street, 0.0, 0.0),
            has_crowd: false,
        },
        BenchView {
            name: "street_up_at_glass",
            eye: view(&street, 0.5, 0.9),
            has_crowd: false,
        },
        BenchView {
            name: "glass_across",
            eye: view(&glass, 0.0, -0.35),
            has_crowd: false,
        },
        BenchView {
            name: "glass_edge_on",
            eye: view(&glass, 1.2, 0.0),
            has_crowd: false,
        },
        BenchView {
            name: "lift_up",
            eye: view(&lift, 0.0, 1.45),
            has_crowd: false,
        },
        BenchView {
            name: "glass_crowd_and_shots",
            eye: make_eye(
                glass.get_eye_frame().w_axis.truncate(),
                DVec3::new(-1.0, 0.25, 0.1) - glass.feet.normalize() * 0.2,
                glass.get_up(),
            ),
            has_crowd: true,
        },
    ]
}

/// `make_crowd` gives `BENCH_ROBOT_COUNT` robots across the glass level and street, and shots that
/// reflect, for the worst bench view.
fn make_crowd(world: &CityWorld) -> (Vec<Robot>, Vec<Beam>) {
    let glass_m: f64 = world.get_glass().map_or(10.0, |g| g.radius_m);
    let mut robots: Vec<Robot> = Vec::new();
    let mut beams: Vec<Beam> = Vec::new();
    for k in 0..BENCH_ROBOT_COUNT {
        let angle: f64 = k as f64 / BENCH_ROBOT_COUNT as f64 * std::f64::consts::TAU;
        let d: DVec3 = DVec3::new(-0.6, angle.cos() * 0.8, angle.sin() * 0.8).normalize();
        let feet: DVec3 = d * if k % 2 == 0 {
            glass_m
        } else {
            world.street_radius_m
        };
        robots.push(Robot {
            feet,
            forward: DVec3::X,
            team: (k % 2) as u32,
            clip: AvatarClip::Run,
        });
        let eye: DVec3 = d * (glass_m - world.eye_height_m);
        let shot: Shot = world.trace_shot(
            eye,
            (DVec3::new(0.3, 0.2, 0.1) - d).normalize(),
            90.0,
            2,
            &[],
            None,
        );
        beams.push((shot.points, (k % 2) as u32, 1.0));
    }
    (robots, beams)
}

/// `bench_views` renders each bench view `--frame-count` times at `--width-px` x `--height-px`,
/// saves the last frame of each as a PNG and prints the median and 99th-percentile GPU and frame times.
fn bench_views(arguments: &Arguments, renderer: &mut dyn Renderer) -> Result<()> {
    let world: CityWorld = CityWorld::Builtin()?;
    let scene: Arc<CityScene> = Arc::new(build_scene(&world));
    let body: AvatarModel = AvatarModel::Load(BODY_MODEL, AVATAR_HEIGHT_M)?;
    let folder: PathBuf = make_run_folder("bench_views", arguments.label.as_deref())?;
    let size: (u32, u32) = (arguments.width_px, arguments.height_px);
    let render_px: (usize, usize) = (
        ((f64::from(size.0) * arguments.render_scale.unwrap_or(1.0)) as usize).max(1),
        ((f64::from(size.1) * arguments.render_scale.unwrap_or(1.0)) as usize).max(1),
    );
    let mut pixels: Vec<u8> = vec![0; size.0 as usize * size.1 as usize * BYTES_PER_PIXEL];
    let (crowd, crowd_beams) = make_crowd(&world);
    let mut rows = Vec::new();
    for view in make_bench_views(&world) {
        let (robots, beams): (&[Robot], &[Beam]) = if view.has_crowd {
            (&crowd, &crowd_beams)
        } else {
            (&[], &[])
        };
        let mut frame_ms: Vec<f64> = Vec::new();
        let mut gpu_ms: Vec<f64> = Vec::new();
        for k in 0..=arguments.frame_count {
            let start: Instant = Instant::now();
            let frame: CityFrame = make_frame(
                &world,
                &scene,
                &body,
                view.eye,
                robots,
                beams,
                (0b0100, 0b0001),
                k as f64 / 60.0,
                render_px,
            );
            renderer.render(FrameData::City(&frame), size, &mut pixels)?;
            if k > 0 {
                frame_ms.push(start.elapsed().as_secs_f64() * MS_PER_S);
                gpu_ms.push(renderer.gpu_ms().unwrap_or(0.0));
            }
        }
        save_png(
            &folder.join(format!("{}.png", view.name)),
            &pixels,
            size.0,
            size.1,
        )?;
        let row = json!({
            "view": view.name,
            "gpu_ms_median": get_percentile_ms(&gpu_ms, 0.5),
            "gpu_ms_p99": get_percentile_ms(&gpu_ms, 0.99),
            "frame_ms_median": get_percentile_ms(&frame_ms, 0.5),
            "frame_ms_p99": get_percentile_ms(&frame_ms, 0.99),
        });
        if !arguments.json {
            println!(
                "{:<24} GPU median {:6.2} ms, p99 {:6.2} ms | frame (CPU+GPU+readback) median {:6.2} ms, p99 {:6.2} ms",
                view.name,
                row["gpu_ms_median"].as_f64().unwrap_or(0.0),
                row["gpu_ms_p99"].as_f64().unwrap_or(0.0),
                row["frame_ms_median"].as_f64().unwrap_or(0.0),
                row["frame_ms_p99"].as_f64().unwrap_or(0.0)
            );
        }
        rows.push(row);
    }
    if arguments.json {
        println!(
            "{}",
            json!({"size_px": [size.0, size.1], "render_px": [render_px.0, render_px.1], "views": rows, "run_folder": folder})
        );
    } else {
        println!(
            "{}x{} rendering {}x{}; pictures in {}",
            size.0,
            size.1,
            render_px.0,
            render_px.1,
            folder.display()
        );
    }
    Ok(())
}

/// `PhotoView` is a named camera for `--photos`, with its field of view, robots and shots.
struct PhotoView {
    name: &'static str,
    eye: DMat4,
    tan_half_fov: f32,
    robots: Vec<Robot>,
    beams: Vec<Beam>,
}

/// `get_pylon_center` gives the centre of `team`'s pylon on the glass level or on the street.
fn get_pylon_center(world: &CityWorld, team: u32, is_glass: bool) -> DVec3 {
    let glass_m: f64 = world.get_glass().map_or(0.0, |g| g.radius_m);
    world
        .boxes
        .iter()
        .filter(|b| b.group == "pylons" && b.name.starts_with(net::TEAM_NAMES[team as usize]))
        .find(|b| (b.center.length() < glass_m + 1.0) == is_glass)
        .map_or(DVec3::X, |b| b.center)
}

/// `get_level_point` gives the point `along_m` from `point` on its level (radius `radius_m`), in
/// the tangent direction `toward`.
fn get_level_point(point: DVec3, toward: DVec3, along_m: f64, radius_m: f64) -> DVec3 {
    let d: DVec3 = point.normalize();
    let t: DVec3 = (toward - d * toward.dot(d)).normalize_or(d.any_orthonormal_vector());
    (d + t * (along_m / radius_m)).normalize() * radius_m
}

/// `make_photo_views` gives `--photos`' views: the far side from near the core, the whole sphere
/// from its centre, a street fight at blue's street pylon, the glass level across to red's glass
/// pylon, and the glass level's crowd with reflected shots.
fn make_photo_views(world: &CityWorld) -> Vec<PhotoView> {
    let glass_m: f64 = world.get_glass().map_or(10.0, |g| g.radius_m);
    let street_m: f64 = world.street_radius_m;
    let eye_m: f64 = world.eye_height_m;
    let robot = |feet: DVec3, toward: DVec3, team: u32, clip: AvatarClip| Robot {
        feet,
        forward: toward - feet,
        team,
        clip,
    };
    let shot = |from: DVec3, to: DVec3, team: u32| -> Beam {
        let up: DVec3 = get_up(from);
        let origin: DVec3 = from + up * eye_m;
        (
            world
                .trace_shot(origin, (to - origin).normalize(), 90.0, 2, &[], None)
                .points,
            team,
            1.0,
        )
    };
    let blue_street: DVec3 = world
        .spawns
        .iter()
        .find(|s| s.0 == 0)
        .map_or(DVec3::X, |s| s.1.w_axis.truncate());
    let blue_glass: DVec3 = get_pylon_center(world, 0, true);
    let blue_glass_side: DVec3 = blue_glass.normalize().any_orthonormal_vector();
    let red_glass: DVec3 = get_pylon_center(world, 1, true);
    let side: DVec3 = blue_street.normalize().any_orthonormal_vector();
    let near_core: DVec3 = side * (world.street_radius_m * 0.3);
    let far_robots: Vec<Robot> = (0..4)
        .map(|k| {
            let radius_m: f64 = if k % 2 == 0 { street_m } else { glass_m };
            let feet: DVec3 = get_level_point(blue_street, side, 3.0 + 2.0 * k as f64, radius_m);
            robot(feet, blue_street, (k % 2) as u32, AvatarClip::Run)
        })
        .collect();
    let camera_street: DVec3 = get_level_point(blue_street, side, 9.0, street_m);
    let fighter_blue: DVec3 = get_level_point(blue_street, side, 2.5, street_m);
    let fighter_red: DVec3 = get_level_point(blue_street, -side, 4.0, street_m);
    let glass_side: DVec3 = red_glass.normalize().any_orthonormal_vector();
    let camera_glass: DVec3 = get_level_point(red_glass, glass_side, 7.0, glass_m);
    let glass_red: DVec3 = get_level_point(red_glass, glass_side, 2.0, glass_m);
    let glass_blue: DVec3 = get_level_point(red_glass, -glass_side, 5.0, glass_m);
    let across: DVec3 = -red_glass.normalize() * glass_m;
    let (crowd, crowd_beams) = make_crowd(world);
    vec![
        PhotoView {
            name: "far_side_from_core",
            eye: make_eye(near_core, blue_street - near_core, side),
            tan_half_fov: 0.9,
            beams: vec![
                shot(far_robots[1].feet, far_robots[0].feet, 1),
                shot(far_robots[3].feet, near_core, 1),
            ],
            robots: far_robots,
        },
        PhotoView {
            name: "sphere_from_centre",
            eye: make_eye(
                blue_street.normalize() * PHOTO_CENTRE_EYE_M,
                blue_street,
                side,
            ),
            tan_half_fov: 1.6,
            robots: Vec::new(),
            beams: Vec::new(),
        },
        PhotoView {
            name: "street_fight_under_glass",
            eye: make_eye(
                camera_street + get_up(camera_street) * eye_m,
                blue_street + get_up(blue_street) * 1.5
                    - camera_street
                    - get_up(camera_street) * eye_m,
                get_up(camera_street),
            ),
            tan_half_fov: 0.55,
            robots: vec![
                robot(fighter_blue, fighter_red, 0, AvatarClip::Run),
                robot(fighter_red, fighter_blue, 1, AvatarClip::Run),
            ],
            beams: vec![shot(
                fighter_red,
                fighter_blue + get_up(fighter_blue) * 1.2,
                1,
            )],
        },
        PhotoView {
            name: "glass_level_across_to_red_pylon",
            eye: make_eye(
                camera_glass + get_up(camera_glass) * eye_m,
                red_glass + get_up(red_glass) * 1.0 - camera_glass - get_up(camera_glass) * eye_m,
                get_up(camera_glass),
            ),
            tan_half_fov: 0.65,
            robots: vec![
                robot(glass_red, glass_blue, 1, AvatarClip::Run),
                robot(glass_blue, glass_red, 0, AvatarClip::Run),
            ],
            beams: vec![
                shot(glass_blue, across, 0),
                shot(glass_red, glass_blue + get_up(glass_blue) * 1.5, 1),
            ],
        },
        PhotoView {
            name: "blue_pylon_exposed_on_glass",
            eye: {
                let camera: DVec3 = get_level_point(blue_glass, blue_glass_side, 6.0, glass_m);
                make_eye(
                    camera + get_up(camera) * eye_m,
                    blue_glass + get_up(blue_glass) * 1.0 - camera - get_up(camera) * eye_m,
                    get_up(camera),
                )
            },
            tan_half_fov: 0.6,
            robots: vec![robot(
                get_level_point(blue_glass, -blue_glass_side, 2.0, glass_m),
                blue_glass,
                1,
                AvatarClip::Run,
            )],
            beams: vec![shot(
                get_level_point(blue_glass, -blue_glass_side, 2.0, glass_m),
                get_level_point(blue_glass, blue_glass_side, 6.0, glass_m)
                    + get_up(blue_glass) * 1.2,
                1,
            )],
        },
        PhotoView {
            name: "glass_level_crowd_and_reflections",
            eye: make_bench_views(world)
                .last()
                .map_or(DMat4::IDENTITY, |view| view.eye),
            tan_half_fov: TAN_HALF_FOV,
            robots: crowd,
            beams: crowd_beams,
        },
    ]
}

/// `render_photos` renders each of `make_photo_views` once at `--width-px` x `--height-px` and
/// saves it as a PNG under .artifacts/photos/.
fn render_photos(arguments: &Arguments, renderer: &mut dyn Renderer) -> Result<()> {
    let world: CityWorld = CityWorld::Builtin()?;
    let scene: Arc<CityScene> = Arc::new(build_scene(&world));
    let body: AvatarModel = AvatarModel::Load(BODY_MODEL, AVATAR_HEIGHT_M)?;
    let folder: PathBuf = make_run_folder("photos", arguments.label.as_deref())?;
    let size: (u32, u32) = (arguments.width_px, arguments.height_px);
    let render_px: (usize, usize) = (size.0 as usize, size.1 as usize);
    let mut pixels: Vec<u8> = vec![0; size.0 as usize * size.1 as usize * BYTES_PER_PIXEL];
    for view in make_photo_views(&world) {
        let mut frame: CityFrame = make_frame(
            &world,
            &scene,
            &body,
            view.eye,
            &view.robots,
            &view.beams,
            (0, 0),
            PHOTO_TIME_S,
            render_px,
        );
        frame.tan_half_fov = view.tan_half_fov;
        renderer.render(FrameData::City(&frame), size, &mut pixels)?;
        save_png(
            &folder.join(format!("{}.png", view.name)),
            &pixels,
            size.0,
            size.1,
        )?;
    }
    println!("Photos in {}", folder.display());
    Ok(())
}

/// `Rng` is a small xorshift generator for the bot's wandering.
struct Rng(u64);

impl Rng {
    /// `next_frac` gives a number in 0..1.
    fn next_frac(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// `run_bot` plays without a window on `--join`'s server for `--exit-after-s`: it wanders between
/// the levels shooting enemies it can hit, or with `--bot-objective consoles` holds `E` on the
/// other team's consoles until its team wins.
fn run_bot(arguments: &Arguments) -> Result<()> {
    let world: CityWorld = CityWorld::Builtin()?;
    let mut client: net::Client = net::Client::Connect(
        get_server_address(arguments)?,
        MAP_NAME,
        &net::read_game_password(arguments.game_password_file.as_deref())?,
    )?;
    client.join(arguments.team.map(TeamName::get_index))?;
    let duration_s: f64 = arguments.exit_after_s.ok_or("--bot needs --exit-after-s")?;
    let start: Instant = Instant::now();
    let mut settings: ServerSettings = ServerSettings::Builtin();
    let mut player: CityPlayer = world.make_player(0, 0);
    let (mut is_spawned, mut respawn_in_s, mut spawn_count) = (false, None::<f64>, 0);
    let (mut hit_count, mut eliminated_count, mut reload_left_s, mut laser_left_s) =
        (0u32, 0u32, 0.0, 0.0);
    let mut rng: Rng = Rng(0x9e37_79b9_7f4a_7c15
        ^ u64::from(start.elapsed().subsec_nanos())
        ^ u64::from(std::process::id()));
    let mut brain: Brain = Brain::New(Vec::new());
    let mut last_s: f64 = 0.0;
    while start.elapsed().as_secs_f64() < duration_s {
        std::thread::sleep(Duration::from_secs_f64(BOT_STEP_S));
        let now_s: f64 = start.elapsed().as_secs_f64();
        let dt_s: f64 = (now_s - last_s).min(MAX_DT_S);
        last_s = now_s;
        for event in client.poll()? {
            match event {
                net::NetEvent::Joined(_) => {
                    player = world.make_player(client.team.unwrap_or(0), spawn_count);
                    spawn_count += 1;
                    is_spawned = true;
                }
                net::NetEvent::Eliminated { is_self: true, .. } => {
                    is_spawned = false;
                    eliminated_count += 1;
                    respawn_in_s = Some(settings.respawn.delay_s);
                }
                net::NetEvent::Eliminated { .. } => {}
            }
        }
        if let Some(s) = &client.settings {
            settings = s.clone();
        }
        step_respawn(&mut client, &mut respawn_in_s, dt_s)?;
        let team: u32 = client.team.unwrap_or(0);
        if arguments.bot_objective == Some(BotObjective::Consoles) && client.game.winner.is_some() {
            break;
        }
        if !is_spawned || client.id.is_none() {
            continue;
        }
        reload_left_s = f64::max(reload_left_s - dt_s, 0.0);
        laser_left_s = f64::max(laser_left_s - dt_s, 0.0);
        let mut disabling: Option<u8> = None;
        if arguments.bot_objective == Some(BotObjective::Consoles) {
            disabling = world
                .consoles
                .iter()
                .find(|c| c.team != team && client.game.off_mask >> c.index & 1 == 0)
                .map(|c| c.index);
        } else if arguments.bot_objective == Some(BotObjective::Target) {
            player = make_target_player(&world);
        } else if client.game.winner.is_none() {
            if brain.is_done() {
                let side: Side = if rng.next_frac() < 0.5 {
                    Side::Street
                } else {
                    Side::Glass
                };
                let direction: DVec3 = DVec3::new(
                    rng.next_frac() - 0.5,
                    rng.next_frac() - 0.5,
                    rng.next_frac() - 0.5,
                );
                brain = Brain::New(vec![Step::GoTo {
                    direction: direction.normalize_or(DVec3::X),
                    side,
                    arrive_m: 2.0,
                }]);
            }
            let out: BrainOutput = brain.drive(
                &world,
                &mut player,
                dt_s,
                &client.game,
                false,
                &BrainSight::default(),
            );
            world.move_player(&mut player, &out.input, &settings, dt_s);
            let eye: DMat4 = player.get_eye_frame();
            let targets: Vec<Target> = client
                .players
                .iter()
                .filter(|p| Some(p.id) != client.id && p.team != team)
                .map(|p| {
                    let (feet, up) = CityPlayer::get_feet_up(p.frame, world.eye_height_m);
                    Target { id: p.id, feet, up }
                })
                .collect();
            if reload_left_s <= 0.0 {
                for target in &targets {
                    let origin: DVec3 = eye.w_axis.truncate();
                    let aim: DVec3 = (target.feet + target.up - origin).normalize();
                    let shot: Shot = world.trace_shot(
                        origin,
                        aim,
                        settings.glass.range_m,
                        settings.glass.max_bounce_count,
                        &targets,
                        client.id,
                    );
                    if let ShotHit::Player { id, .. } = shot.hit {
                        let up: DVec3 = player.get_up();
                        let flat: DVec3 = aim - up * aim.dot(up);
                        let forward: DVec3 = -player.body.z_axis;
                        player.look_around(
                            forward
                                .cross(flat.normalize())
                                .dot(up)
                                .atan2(forward.dot(flat.normalize())),
                            0.0,
                        );
                        player.pitch_rad = aim.dot(up).asin();
                        client.send_hit(id, settings.sniper.hits_to_eliminate)?;
                        hit_count += 1;
                        reload_left_s = settings.sniper.reload_s;
                        laser_left_s = settings.sniper.shot_s;
                        break;
                    }
                }
            }
        }
        client.send_state(
            player.get_eye_frame(),
            laser_left_s > 0.0,
            false,
            disabling,
            0.0,
            PlayerWormholes::default(),
        )?;
    }
    let winner: Option<&str> = client
        .game
        .winner
        .map(|team| net::TEAM_NAMES[team as usize % 2]);
    let report = json!({"hit_count": hit_count, "eliminated_count": eliminated_count, "winner": winner, "off_mask": client.game.off_mask});
    if arguments.json {
        println!("{report}");
    } else {
        println!(
            "The bot hit {hit_count} players and was hit {eliminated_count} times; winner: {}.",
            winner.unwrap_or("none yet")
        );
    }
    let _ = client.leave();
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the City Sphere session's scripted helpers.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Part of the rifle target aimed at from blue's spawn | head, body | `the_rifle_target_is_in_sight_of_blues_spawn` |
    //! | Map picture pixel and direction | round trip at the equator and near a pole | `map_pixels_and_directions_agree` |
    //! | When an eliminated player comes back | before the respawn delay (never), within a join retry after it, at their team's spawn | `an_eliminated_player_respawns_only_after_the_delay` |
    //! | Walk from a spawn up the stairs | each team, each of its two spawns, walking and sprinting: reaches the glass level, then its pylon, never stuck | `every_spawn_walks_up_the_stairs_to_its_pylon` |
    use super::*;

    const WALK_STEP_S: f64 = 1.0 / 60.0;
    const WALK_TIMEOUT_S: f64 = 40.0;
    const GLASS_LEVEL_TOLERANCE_M: f64 = 0.05;

    /// `walk_steps` plays `steps` for `player` with the real movement and settings, a 1/60 s step
    /// at a time, sprinting if `is_sprinting`, until the steps are done or `WALK_TIMEOUT_S` runs
    /// out; it panics if the brain ever needs to unstick the player (`STUCK_S` without moving).
    ///
    /// Example: `walk_steps(&world, &mut player, vec![Step::WalkTo { direction, arrive_m: 0.5 }], false);`
    fn walk_steps(
        world: &CityWorld,
        player: &mut CityPlayer,
        steps: Vec<Step>,
        is_sprinting: bool,
    ) -> f64 {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut brain: Brain = Brain::New(steps);
        let mut time_s: f64 = 0.0;
        while !brain.is_done() && time_s < WALK_TIMEOUT_S {
            let out: BrainOutput = brain.drive(
                world,
                player,
                WALK_STEP_S,
                &net::GameState::default(),
                false,
                &BrainSight::default(),
            );
            assert!(
                brain.unstick_s <= 0.0,
                "stuck at {:?} (radius {:.2} m) on step {} after {time_s:.1} s",
                player.feet,
                player.feet.length(),
                brain.index
            );
            let input: CityInput = CityInput {
                is_sprinting,
                ..out.input
            };
            world.move_player(player, &input, &settings, WALK_STEP_S);
            time_s += WALK_STEP_S;
        }
        assert!(brain.is_done(), "the walk took over {WALK_TIMEOUT_S} s");
        time_s
    }

    /// `every_spawn_walks_up_the_stairs_to_its_pylon` walks from each spawn of each team up the
    /// team's stairs (W-119 (City Sphere demo): "There should be like stairs going up ... you need
    /// to make it to the pylons"), checks it stands on the glass level at the top, then walks on to
    /// the team's pylon until its console is within reach.
    #[test]
    fn every_spawn_walks_up_the_stairs_to_its_pylon() {
        let world: CityWorld = CityWorld::Builtin().expect("the built-in map");
        let settings: ServerSettings = ServerSettings::Builtin();
        let glass: &crate::city_world::Shell = world.get_glass().expect("the glass");
        let glass_level_m: f64 = glass.radius_m - glass.thickness_m / 2.0;
        for (team, spawn, is_sprinting) in
            (0..8_usize).map(|k| ((k / 4) as u32, k / 2 % 2, k % 2 == 1))
        {
            let case: String = format!("team {team}, spawn {spawn}, sprinting {is_sprinting}");
            let mut player: CityPlayer = world.make_player(team, spawn);
            let mut steps: Vec<Step> = make_stairs_route(&world, team).expect("stairs");
            let to_pylon: Vec<Step> = steps.split_off(2);
            let climb_s: f64 = walk_steps(&world, &mut player, steps, is_sprinting);
            assert_eq!(world.get_side(player.feet), Side::Glass, "{case}");
            assert!(
                player.is_on_ground
                    && (player.feet.length() - glass_level_m).abs() < GLASS_LEVEL_TOLERANCE_M,
                "{case}: at radius {:.3} m after {climb_s:.1} s",
                player.feet.length()
            );
            walk_steps(&world, &mut player, to_pylon, is_sprinting);
            let console = world.consoles.iter().find(|c| c.team == team).expect("a console");
            let eye: DVec3 = player.get_eye_frame().w_axis.truncate();
            let reach_m: f64 = (world.get_console_center(console) - eye).length();
            assert!(
                reach_m <= settings.consoles.reach_m,
                "{case}: the console is {reach_m:.2} m away"
            );
        }
    }

    const TEST_RESPAWN_DELAY_S: f64 = 1.5;
    const TEST_STEP_MS: u64 = 10;
    const TEST_TIMING_SLACK_S: f64 = 0.05;
    const TEST_JOIN_RETRY_SLACK_S: f64 = 0.6;
    const TEST_TIMEOUT_S: f64 = 5.0;

    /// `poll_until` polls `client` every `TEST_STEP_MS`, stepping its respawn countdown
    /// (`step_respawn`) by the real time passed, until `is_done` says yes for an event, and gives
    /// the seconds that took; it panics after `TEST_TIMEOUT_S`.
    ///
    /// Example: `let joined_s = poll_until(&mut client, &mut respawn_in_s, |e| matches!(e, net::NetEvent::Joined(_)));`
    fn poll_until(
        client: &mut net::Client,
        respawn_in_s: &mut Option<f64>,
        is_done: impl Fn(&net::NetEvent) -> bool,
    ) -> f64 {
        let start: Instant = Instant::now();
        let mut last: Instant = start;
        while start.elapsed().as_secs_f64() < TEST_TIMEOUT_S {
            std::thread::sleep(Duration::from_millis(TEST_STEP_MS));
            let events: Vec<net::NetEvent> = client.poll().expect("the client polls");
            if events.iter().any(&is_done) {
                return start.elapsed().as_secs_f64();
            }
            step_respawn(client, respawn_in_s, last.elapsed().as_secs_f64())
                .expect("the client steps");
            last = Instant::now();
        }
        panic!("poll_until: nothing happened within {TEST_TIMEOUT_S} s");
    }

    /// `an_eliminated_player_respawns_only_after_the_delay` plays an elimination on a real server:
    /// the victim's window keeps polling and stepping its countdown, and must be welcomed back
    /// only once `respawn.delay_s` has passed (W-119 (City Sphere demo): they came back at once,
    /// after one 0.5 s join retry), and then at a spawn of their team.
    #[test]
    fn an_eliminated_player_respawns_only_after_the_delay() {
        let world: CityWorld = CityWorld::Builtin().expect("the built-in map");
        let mut settings: ServerSettings = ServerSettings::Builtin();
        settings.respawn.delay_s = TEST_RESPAWN_DELAY_S;
        let socket: std::net::UdpSocket =
            std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free local port");
        socket
            .set_read_timeout(Some(Duration::from_millis(TEST_STEP_MS)))
            .expect("a timeout");
        let address: SocketAddr = socket.local_addr().expect("an address");
        let should_stop: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let stop_flag: Arc<AtomicBool> = Arc::clone(&should_stop);
        let console_teams: Vec<u32> = get_console_teams(&world);
        let server: JoinHandle<()> = std::thread::spawn(move || {
            net::serve(
                socket,
                &stop_flag,
                MAP_NAME.to_string(),
                settings,
                console_teams,
                &AtomicBool::new(false),
                None,
                Vec::new(),
                None,
                false,
            )
            .expect("the server runs");
        });
        let is_joined = |event: &net::NetEvent| matches!(event, net::NetEvent::Joined(_));
        let mut shooter: net::Client =
            net::Client::Connect(address, MAP_NAME, b"").expect("a shooter");
        let mut victim: net::Client =
            net::Client::Connect(address, MAP_NAME, b"").expect("a victim");
        let mut no_countdown: Option<f64> = None;
        shooter.join(Some(0)).expect("the shooter joins blue");
        poll_until(&mut shooter, &mut no_countdown, is_joined);
        victim.join(Some(1)).expect("the victim joins red");
        poll_until(&mut victim, &mut no_countdown, is_joined);
        let victim_id: u32 = victim.id.expect("the victim was welcomed");
        shooter.send_hit(victim_id, 1).expect("the shooter's hit");
        poll_until(&mut victim, &mut no_countdown, |event| {
            matches!(event, net::NetEvent::Eliminated { is_self: true, .. })
        });
        let mut respawn_in_s: Option<f64> = Some(TEST_RESPAWN_DELAY_S);
        let back_after_s: f64 = poll_until(&mut victim, &mut respawn_in_s, is_joined);
        should_stop.store(true, Ordering::Relaxed);
        server.join().expect("the server stops");
        assert!(
            back_after_s >= TEST_RESPAWN_DELAY_S - TEST_TIMING_SLACK_S,
            "back after {back_after_s:.2} s, before the {TEST_RESPAWN_DELAY_S} s delay"
        );
        assert!(
            back_after_s < TEST_RESPAWN_DELAY_S + TEST_JOIN_RETRY_SLACK_S,
            "back after {back_after_s:.2} s, long after the {TEST_RESPAWN_DELAY_S} s delay"
        );
        let team: u32 = victim.team.expect("the victim has a team");
        let respawned: CityPlayer = world.make_player(team, 1);
        assert!(
            world
                .spawns
                .iter()
                .any(|&(t, frame)| t == team && frame.w_axis.truncate() == respawned.feet),
            "the victim came back at {:?}, not at a red spawn",
            respawned.feet
        );
    }

    #[test]
    fn the_rifle_target_is_in_sight_of_blues_spawn() {
        let world: CityWorld = CityWorld::Builtin().expect("the built-in map");
        let target: CityPlayer = make_target_player(&world);
        let (feet, up) = (target.feet, target.get_up());
        let targets: [Target; 1] = [Target { id: 1, feet, up }];
        for (height_m, is_head) in [(HEAD_HEIGHT_M, true), (TORSO_HEIGHT_M, false)] {
            let mut shooter: CityPlayer = world.make_player(0, 0);
            aim_at(&mut shooter, feet + up * height_m);
            let eye: DMat4 = shooter.get_eye_frame();
            let shot: Shot = world.trace_shot(
                eye.w_axis.truncate(),
                -eye.z_axis.truncate(),
                90.0,
                2,
                &targets,
                Some(0),
            );
            assert_eq!(shot.hit, ShotHit::Player { id: 1, is_head }, "{shot:?}");
        }
    }

    #[test]
    fn map_pixels_and_directions_agree() {
        let size_px: (usize, usize) = get_map_picture_px();
        for (x, y) in [(size_px.0 / 3, size_px.1 / 2), (size_px.0 / 5, 2)] {
            assert_eq!(
                get_map_pixel(get_map_direction(x, y, size_px), size_px),
                (x, y)
            );
        }
    }
}
