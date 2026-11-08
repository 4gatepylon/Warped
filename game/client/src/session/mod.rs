//! `session` is one game window's play: the `Session` holds the voxel map, the player, the other
//! players, the guns and thrown wormholes, turns input into movement and shots each frame, and
//! packs the frame to render. Its children split the methods: `weapons` (guns, thrown wormholes,
//! hits), `online` (the game server's messages and connection problems) and `hud` (the overlay's
//! text, rectangles, minimap and gun model).
//!
//! Interface: `Session`, `PackedFrame`, `scale_size`, `make_text_mask`, `get_text_positions`,
//! `OverlayText`.
//!
//! Example: `let mut session: Session = Session::New(&arguments)?;`

use std::fs;
use std::path::PathBuf;
use std::process::Child;
use std::time::Instant;

use glam::{DMat4, DVec4};
use sdl3::event::Event;

use crate::bench::{VoxelView, make_voxel_bench_views};
use crate::black_hole_growth::BlackHoleGrowth;
use crate::geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, make_boost, make_rotation};
use crate::key_bindings::{Action, Control};
use crate::map::MapName;
use crate::player::Input;
use crate::player_settings::PerPlayerSettings;
use crate::render_metal::FrameData;
use crate::render_voxel::VoxelFrame;
use crate::settings::ServerSettings;
use crate::thrown_wormholes::{Marker, MouthPlace, ThrownWormholes};
use crate::voxel_world::{VoxelPlayer, VoxelWorld};

use crate::{
    Arguments, ENERGY_SEGMENT_COUNT, MIN_IMAGE_SIDE_PX, MS_PER_S, Result, avatar_animation,
    avatar_model, dust, exit_prompt, hud_bars, key_bindings, map, map_layout, menu, multiplayer,
    net, render_voxel, shot_noise, spaceships, third_person, viewmodel, voxel_world,
};

mod hud;
mod online;
mod weapons;

use crate::cli::RendererKind;
use crate::cli::RouteKind;
use crate::cli::get_default_render_scale;
use crate::server_process::get_server_address;
use crate::server_process::load_map;
use crate::server_process::load_settings;
use crate::server_process::start_server_process;
pub(crate) use hud::{OverlayText, get_text_positions, make_text_mask};
use hud::{get_blueshift_frac, get_map_picture_max_px};
use online::ConnectionProblem;
use weapons::{get_aim_zoom, take_hit_weight};

const MIN_AUTO_RENDER_SCALE: f64 = 0.5;
const AUTO_SCALE_TARGET_GPU_MS: f64 = 14.0;
const AUTO_SCALE_MAX_STEP: f64 = 1.02;
const AUTO_SCALE_MAX_DROP: f64 = 0.8;
const RENDER_SCALE_QUANTUM: f64 = 0.05;
const FULL_RESOLUTION_FROM_SCALE: f64 = 0.95;
const RENDER_SCALE_ROUNDING: f64 = 1e-6;
const RENDER_SCALE_STEP: f64 = 1.25;
const MIN_RENDER_SCALE: f64 = 0.05;
const MAX_FRAME_DT_S: f64 = 0.05;
const HUD_PERIOD_S: f64 = 0.25;
const ROUTE_WARMUP_FRAME_COUNT: usize = 5;
const TOUR_YAW_RAD: f64 = 0.8;
const TOUR_PITCH_RAD: f64 = 0.4;
const TOUR_YAW_TURNS: f64 = 2.0;
const TOUR_PITCH_TURNS: f64 = 3.0;
const WARP_TOUR_FRAMES_PER_WARP: usize = 600;
const WARP_TOUR_NEAR_PER_REACH: f64 = 0.1;
const WARP_TOUR_FAR_PER_REACH: f64 = 0.8;
const FRAME_BUDGET_MS: f64 = 1000.0 / 60.0;
const MESSAGE_DURATION_S: f64 = 3.0;
const SKY_TRACER_M: f64 = 80.0;
const PLAYER_SETTINGS_HOME_PATH: &str = ".config/warped/player_settings.yaml";

/// `RouteFrame` is one frame of a `--play-route` route: the camera, and whether the laser fires.
struct RouteFrame {
    frame: DMat4,
    is_laser_on: bool,
}

/// `make_warp_tour` makes a tour of a voxel map's warp objects: for each, the eye circles it once
/// at its centre's height, from `WARP_TOUR_FAR_PER_REACH` of its reach to `WARP_TOUR_NEAR_PER_REACH`
/// and back (19 m to 2.4 m around the black hole, 12 m to 1.5 m around a mouth), looking at it with
/// the view swinging as in `make_mouth_tour`, over `WARP_TOUR_FRAMES_PER_WARP` frames.
///
/// Example: `let tour = make_warp_tour(&world);`
fn make_warp_tour(world: &VoxelWorld) -> Vec<RouteFrame> {
    let mut frames: Vec<RouteFrame> = Vec::new();
    for warp in world.warps.iter() {
        for n in 0..WARP_TOUR_FRAMES_PER_WARP {
            let turn: f64 =
                std::f64::consts::TAU * n as f64 / (WARP_TOUR_FRAMES_PER_WARP - 1) as f64;
            let distance: f64 = warp.reach
                * (WARP_TOUR_NEAR_PER_REACH
                    + (WARP_TOUR_FAR_PER_REACH - WARP_TOUR_NEAR_PER_REACH)
                        * (0.5 + 0.5 * turn.cos()));
            let frame: DMat4 = warp.frame
                * make_rotation(AXIS_X, AXIS_Z, turn)
                * make_boost(AXIS_Z, distance)
                * make_rotation(AXIS_X, AXIS_Z, TOUR_YAW_RAD * (TOUR_YAW_TURNS * turn).sin())
                * make_rotation(
                    AXIS_Y,
                    AXIS_Z,
                    TOUR_PITCH_RAD * (TOUR_PITCH_TURNS * turn).sin(),
                );
            frames.push(RouteFrame {
                frame,
                is_laser_on: false,
            });
        }
    }
    frames
}

/// `Session` is the state of a played window: the map it loaded, its game (`voxel`), the toggles
/// and the HUD's running averages.
pub(crate) struct Session {
    map_name: MapName,
    render_scale: f64,
    last_used_scale: f64,
    is_auto_scale: bool,
    exit_after_s: Option<f64>,
    route: Vec<RouteFrame>,
    route_index: usize,
    route_eye: DMat4,
    frame_times_ms: Vec<f64>,
    render_scales: Vec<f64>,
    total_frame_count: u64,
    total_gpu_ms: f64,
    last_output_px: (u32, u32),
    is_laser_on: bool,
    shot_time_s: f64,
    aim_frac: f64,
    sniper_reload_s: f64,
    since_movement_key_s: f64,
    settings: ServerSettings,
    player_settings: PerPlayerSettings,
    player_settings_path: Option<PathBuf>,
    menu: Option<menu::Menu>,
    last_connection_problem: Option<ConnectionProblem>,
    exit_prompt: exit_prompt::ExitPrompt,
    map_layout: map_layout::MapLayout,
    pub(crate) gun: Gun,
    is_trigger_held: bool,
    is_flashlight_on: bool,
    view_switch: third_person::ViewSwitch,
    is_own_body_shown: bool,
    rifle_next_shot_s: f64,
    rifle_heat_frac: f64,
    is_overheated: bool,
    has_rifle_shot_hit: bool,
    tracers: Vec<Tracer>,
    dust_bursts: Vec<dust::DustBurst>,
    was_firing: bool,
    remote_firing_ids: Vec<u32>,
    avatars: avatar_animation::AvatarAnimator,
    rifle_shot_count: u64,
    recoil_frac: f64,
    shot_noise: shot_noise::ShotNoise,
    shot_offset: shot_noise::ShotOffset,
    beam_speckle_frac: f64,
    gun_phase_rad: f64,
    step_frac: f64,
    run_pose_frac: f64,
    pub(crate) motion_blur_frac: f32,
    hit_effect_frac: f64,
    pub(crate) blueshift_frac: f32,
    last_health_frac: f64,
    health_pulses: hud_bars::BarPulses,
    energy_pulses: hud_bars::BarPulses,
    is_captured: bool,
    message: String,
    message_time_s: f64,
    laser: Option<String>,
    fps: f64,
    render_ms: f64,
    gpu_ms: Option<f64>,
    start: Instant,
    last: Instant,
    hud_time_s: f64,
    pub(crate) hud_frame_count: u32,
    hud_render_ms: f64,
    multiplayer: Option<Multiplayer>,
    voxel: VoxelState,
    thrown: ThrownWormholes,
    thrown_pairs: Vec<[MouthPlace; 2]>,
    thrown_pair_owners: Vec<Option<u32>>,
    wormhole_damages: Vec<(Option<u32>, f64)>,
    wormhole_flashes: Vec<(Option<u32>, f64)>,
    has_shot_hit_wormholes: bool,
}

/// `VoxelState` is a window's game on a voxel map: the map, the player, their spawn point and the
/// black hole's growth.
struct VoxelState {
    world: VoxelWorld,
    player: VoxelPlayer,
    spawn_index: usize,
    black_hole_growth: BlackHoleGrowth,
}

/// `PackedFrame` is one frame packed for the renderer: a voxel map's frame.
pub(crate) enum PackedFrame {
    Voxel(Box<VoxelFrame>),
}

impl PackedFrame {
    /// `zoom_in` narrows the camera's field of view so the world looks `zoom` times closer (1 for
    /// none), for aiming: the renderers read the view's width from `tan_half_fov` and the size of
    /// a pixel's view from `pixel_angle_rad`.
    ///
    /// Example: `packed.zoom_in(2.5);`
    pub(crate) fn zoom_in(&mut self, zoom: f64) {
        let zoom: f32 = zoom as f32;
        match self {
            PackedFrame::Voxel(frame) => {
                frame.uniforms.tan_half_fov /= zoom;
                frame.uniforms.pixel_angle_rad /= zoom;
            }
        }
    }

    /// `as_frame_data` lends the frame to a renderer.
    ///
    /// Example: `renderer.render(packed.as_frame_data(), size, &mut pixels)?;`
    pub(crate) fn as_frame_data(&self) -> FrameData<'_> {
        match self {
            PackedFrame::Voxel(frame) => FrameData::Voxel(frame),
        }
    }
}

/// `Tracer` is a fired laser drawn as a beam flying out along its path (`segments`, traced when it
/// was fired, `length_m` long): its front leaves the muzzle at `fired_s` (seconds into the session)
/// and moves at `speed_m_per_s`, each point stays lit for `trail_s` after the front passes it, and
/// `spot` glows where it hit while the trail is there. It is drawn in its gun's style (`is_rifle`),
/// and a share `speckle_frac` of its pixels speckles each frame (the shot's noise). This player's
/// own shots (`is_own`) stay joined to the gun in hand while their trail still reaches the muzzle.
struct Tracer {
    segments: Vec<voxel_world::BeamSegment>,
    length_m: f64,
    spot: Option<glam::DVec4>,
    team: u32,
    is_rifle: bool,
    speckle_frac: f64,
    fired_s: f64,
    speed_m_per_s: f64,
    trail_s: f64,
    is_own: bool,
}

impl Tracer {
    /// `New` makes a tracer for `shot`, fired at `fired_s` by a player of `team` with the rifle
    /// (`is_rifle`) or the sniper, whose beam speed and trail come from `settings`, and whose
    /// beam speckles a share `speckle_frac` of its pixels. A shot that hit nothing flies only its
    /// first `SKY_TRACER_M`: farther out over the sky it is under a pixel wide, and the shader's
    /// single precision makes its glow noisy (D64, D66).
    ///
    /// Example: `let tracer = Tracer::New(&shot, 1, 12.5, false, 0.1, &settings, world.r);`
    #[allow(non_snake_case)]
    fn New(
        shot: &voxel_world::Shot,
        team: u32,
        fired_s: f64,
        is_rifle: bool,
        speckle_frac: f64,
        settings: &ServerSettings,
        r: f64,
    ) -> Tracer {
        let (speed_m_per_s, trail_s) = if is_rifle {
            (
                settings.rifle.beam_speed_m_per_s,
                settings.rifle.beam_trail_s,
            )
        } else {
            (
                settings.sniper.beam_speed_m_per_s,
                settings.sniper.beam_trail_s,
            )
        };
        let segments: Vec<voxel_world::BeamSegment> = if shot.is_hit {
            shot.segments.clone()
        } else {
            voxel_world::clip_beam(&shot.segments, r, 0.0, SKY_TRACER_M)
        };
        Tracer {
            length_m: segments.iter().map(|s| s.length).sum::<f64>() * r,
            segments,
            spot: shot.spot,
            team,
            is_rifle,
            speckle_frac,
            fired_s,
            speed_m_per_s,
            trail_s,
            is_own: false,
        }
    }

    /// `get_lit_m` gives the lit stretch of the path at `now_s`, as meters from its start: from the
    /// trail's end to the front.
    ///
    /// Example: `let (tail_m, head_m) = tracer.get_lit_m(13.0);`
    fn get_lit_m(&self, now_s: f64) -> (f64, f64) {
        let head_m: f64 = self.speed_m_per_s * (now_s - self.fired_s);
        (head_m - self.speed_m_per_s * self.trail_s, head_m)
    }

    /// `get_arrival_s` gives when the beam's front reaches the end of its path, in seconds into
    /// the session.
    ///
    /// Example: `let start_s = tracer.get_arrival_s();`
    fn get_arrival_s(&self) -> f64 {
        self.fired_s + self.length_m / self.speed_m_per_s
    }
}

/// `add_dust_burst` adds the dust burst of `shot`, fired as `tracer` by the rifle (`is_rifle`) or
/// the sniper, to `bursts` when it hit the ground of `world` (`DustBurst::FromShot`), starting when
/// its beam front arrives, and drops the oldest bursts beyond `MAX_DUST_BURST_COUNT`.
///
/// Example: `add_dust_burst(&mut self.dust_bursts, &voxel.world, shot, &tracer, false);`
fn add_dust_burst(
    bursts: &mut Vec<dust::DustBurst>,
    world: &VoxelWorld,
    shot: &voxel_world::Shot,
    tracer: &Tracer,
    is_rifle: bool,
) {
    let Some(burst) = dust::DustBurst::FromShot(world, shot, tracer.get_arrival_s(), is_rifle)
    else {
        return;
    };
    bursts.push(burst);
    let excess_count: usize = bursts.len().saturating_sub(dust::MAX_DUST_BURST_COUNT);
    bursts.drain(..excess_count);
}

/// `Gun` is the weapon a player holds: gun 1, the laser (a quarter-second shot per click; one
/// hit eliminates), or gun 2, the design's rifle (fires while the button is held, its beam
/// pulsing; two hits to the head or twelve to the body eliminate; overheats).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gun {
    Laser,
    Rifle,
}

impl Gun {
    /// `get_model` gives the model drawn in hand for this gun: the bolt-action rifle for the laser
    /// (the sniper) and the energy revolver for the rifle.
    ///
    /// Example: `let model: viewmodel::GunModel = Gun::Rifle.get_model();`
    pub(crate) fn get_model(self) -> viewmodel::GunModel {
        match self {
            Gun::Laser => viewmodel::GunModel::BoltActionRifle,
            Gun::Rifle => viewmodel::GunModel::Revolver,
        }
    }
}

/// `Multiplayer` is a window's game on the local network: its connection, the server process it
/// started with `--host`, whether this player was hit and waits to rejoin, the number of the game
/// it plays, and the console this player holds `E` on, with the seconds held.
struct Multiplayer {
    client: net::Client,
    server: Option<Child>,
    is_eliminated: bool,
    respawn_in_s: f64,
    game_number: u32,
    disabling: Option<(u8, f64)>,
}

impl Drop for Multiplayer {
    /// `drop` tells the server this player is leaving and stops the hosted server, if any.
    ///
    /// Example: `drop(multiplayer);`
    fn drop(&mut self) {
        if let Err(error) = self.client.leave() {
            eprintln!("Couldn't tell the server this player left: {error}");
        }
        if let Some(mut server) = self.server.take()
            && let Err(error) = server.kill().and_then(|()| server.wait().map(drop))
        {
            eprintln!("Couldn't stop the game server: {error}");
        }
    }
}

impl Session {
    /// `New` starts a session at spawn with the command line's render scale.
    ///
    /// Example: `let mut session = Session::New(&arguments);`
    #[allow(non_snake_case)]
    pub(crate) fn New(arguments: &Arguments) -> Result<Session> {
        let settings: ServerSettings = load_settings(arguments)?;
        let map: map::Map = load_map(arguments.map, &settings)?;
        let player_settings_path: Option<PathBuf> =
            arguments.player_settings.clone().or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(PLAYER_SETTINGS_HOME_PATH))
            });
        let player_settings: PerPlayerSettings = match &player_settings_path {
            Some(path) if arguments.player_settings.is_some() || path.exists() => {
                eprintln!("Player settings: {}", path.display());
                PerPlayerSettings::Load(path)?
            }
            _ => PerPlayerSettings::Builtin(),
        };
        let mut world: VoxelWorld = VoxelWorld::FromMap(&map)?;
        world.set_settings(settings.clone());
        world.head_bob = player_settings.view.head_bob.clone();
        world.milky_way_brightness_frac = player_settings.view.milky_way_brightness_frac;
        let player: VoxelPlayer = match &arguments.view {
            Some(name) => {
                let view: VoxelView = make_voxel_bench_views()
                    .into_iter()
                    .find(|v| v.name == name)
                    .ok_or_else(|| {
                        format!("--view {name:?} isn't a viewpoint; see --bench-views")
                    })?;
                VoxelPlayer::AtView(&world, view.at_m, view.target_m, view.h_m, view.pitch_rad)
            }
            None => VoxelPlayer::AtSpawn(&world, 0),
        };
        let voxel: VoxelState = VoxelState {
            world,
            player,
            spawn_index: 0,
            black_hole_growth: BlackHoleGrowth::New(arguments.black_hole_strength),
        };
        let route: Vec<RouteFrame> = match (arguments.play_route, arguments.route) {
            (false, _) => Vec::new(),
            (true, RouteKind::WarpTour) => make_warp_tour(&voxel.world),
        };
        let multiplayer: Option<Multiplayer> = if arguments.host || arguments.join.is_some() {
            let server: Option<Child> = if arguments.host {
                Some(start_server_process(arguments)?)
            } else {
                None
            };
            Some(Multiplayer {
                client: net::Client::Connect(
                    get_server_address(arguments)?,
                    arguments.map.get_folder_name(),
                    &net::read_game_password(arguments.game_password_file.as_deref())?,
                )?,
                server,
                is_eliminated: false,
                respawn_in_s: 0.0,
                game_number: 0,
                disabling: None,
            })
        } else {
            None
        };
        Ok(Session {
            map_name: arguments.map,
            settings,
            player_settings,
            player_settings_path,
            menu: None,
            last_connection_problem: None,
            exit_prompt: exit_prompt::ExitPrompt::New(),
            map_layout: map_layout::MapLayout::FromMap(&map, get_map_picture_max_px())?,
            render_scale: arguments
                .render_scale
                .unwrap_or(get_default_render_scale(arguments.renderer)),
            last_used_scale: 1.0,
            is_auto_scale: arguments.renderer == RendererKind::Metal
                && arguments.render_scale.is_none(),
            exit_after_s: arguments.exit_after_s,
            route,
            route_index: 0,
            route_eye: DMat4::IDENTITY,
            frame_times_ms: Vec::new(),
            render_scales: Vec::new(),
            total_frame_count: 0,
            total_gpu_ms: 0.0,
            last_output_px: (0, 0),
            is_laser_on: false,
            shot_time_s: 0.0,
            aim_frac: 0.0,
            sniper_reload_s: 0.0,
            since_movement_key_s: f64::INFINITY,
            gun: Gun::Rifle,
            is_trigger_held: false,
            is_flashlight_on: false,
            view_switch: third_person::ViewSwitch::FirstPerson(),
            is_own_body_shown: false,
            rifle_next_shot_s: 0.0,
            rifle_heat_frac: 0.0,
            is_overheated: false,
            has_rifle_shot_hit: false,
            tracers: Vec::new(),
            dust_bursts: Vec::new(),
            was_firing: false,
            remote_firing_ids: Vec::new(),
            avatars: avatar_animation::AvatarAnimator::New(),
            rifle_shot_count: 0,
            recoil_frac: 0.0,
            shot_noise: shot_noise::ShotNoise::FromClock(),
            shot_offset: shot_noise::ShotOffset::NONE,
            beam_speckle_frac: 0.0,
            gun_phase_rad: 0.0,
            step_frac: 0.0,
            run_pose_frac: 0.0,
            motion_blur_frac: 0.0,
            hit_effect_frac: 0.0,
            blueshift_frac: 0.0,
            last_health_frac: 1.0,
            health_pulses: hud_bars::BarPulses::New(1.0),
            energy_pulses: hud_bars::BarPulses::New(1.0),
            is_captured: false,
            message: String::new(),
            message_time_s: 0.0,
            laser: None,
            fps: 0.0,
            render_ms: 0.0,
            gpu_ms: None,
            start: Instant::now(),
            last: Instant::now(),
            hud_time_s: 0.0,
            hud_frame_count: 0,
            hud_render_ms: 0.0,
            multiplayer,
            voxel,
            thrown: ThrownWormholes::New(),
            thrown_pairs: Vec::new(),
            thrown_pair_owners: Vec::new(),
            wormhole_damages: Vec::new(),
            wormhole_flashes: Vec::new(),
            has_shot_hit_wormholes: false,
        })
    }

    /// `handle_event` reacts to one window event and returns false when the player quits: the
    /// window's close button quits, Esc releases the mouse (or asks whether to exit if it is free,
    /// and while it asks every pressed control answers it), a click captures it (and once captured,
    /// a left click fires a shot), the mouse looks (more slowly while aiming zooms in, by the same
    /// factor), `1`, `2` and the wheel switch guns, `V` switches between first and third person,
    /// and - and = change the render scale. Players can't respawn by choice.
    ///
    /// Example: `if !session.handle_event(event, &window, &mouse) { break; }`
    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        window: &sdl3::video::Window,
        mouse: &sdl3::mouse::MouseUtil,
    ) -> bool {
        match event {
            Event::Quit { .. } => return false,
            Event::MouseButtonDown { .. } if !self.is_captured && !self.exit_prompt.is_shown() => {
                self.is_captured = true;
                mouse.set_relative_mouse_mode(window, true);
                return true;
            }
            Event::MouseMotion { xrel, yrel, .. } if self.is_captured && self.menu.is_none() => {
                let speed: f64 = self.player_settings.view.look_rad_per_px
                    / voxel_world::LOOK_RAD_PER_PX
                    / self.get_zoom();
                let (dx_px, dy_px) = (f64::from(xrel) * speed, f64::from(yrel) * speed);
                self.voxel.player.look(dx_px, dy_px)
            }
            _ => {}
        }
        match key_bindings::get_pressed_control(&event) {
            Some(control) if self.exit_prompt.is_shown() => {
                self.exit_prompt.answer(control) != exit_prompt::ExitOutcome::Exit
            }
            Some(control) if self.menu.is_some() => {
                self.handle_menu_control(control);
                true
            }
            Some(control) if self.player_settings.keys.matches(Action::OpenMenu, control) => {
                self.menu = Some(menu::Menu::New());
                true
            }
            Some(control) => {
                self.handle_control(control, window, mouse);
                true
            }
            None => true,
        }
    }

    /// `handle_menu_control` passes a pressed control to the open menu, closes it or saves when
    /// the menu says so, and puts the changed head bob and Milky Way brightness into the world.
    ///
    /// Example: `self.handle_menu_control(Control::Key(Scancode::Down));`
    fn handle_menu_control(&mut self, control: Control) {
        let menu: &mut menu::Menu = self.menu.as_mut().expect("the menu is open");
        match menu.handle(control, &mut self.player_settings) {
            menu::MenuOutcome::Stay => {}
            menu::MenuOutcome::Close => self.menu = None,
            menu::MenuOutcome::Save => {
                let message: String = match self.save_player_settings() {
                    Ok(path) => format!("Saved to {path}"),
                    Err(error) => format!("Couldn't save: {error}"),
                };
                self.menu
                    .as_mut()
                    .expect("the menu is open")
                    .show_message(message);
            }
        }
        {
            self.voxel.world.head_bob = self.player_settings.view.head_bob.clone();
            self.voxel.world.milky_way_brightness_frac =
                self.player_settings.view.milky_way_brightness_frac;
        }
    }

    /// `save_player_settings` writes this player's settings to their file (`--player-settings`, or
    /// `~/.config/warped/player_settings.yaml`, which later games load), giving its path.
    ///
    /// Example: `let path = self.save_player_settings()?;`
    fn save_player_settings(&self) -> Result<String> {
        let path: &PathBuf = self
            .player_settings_path
            .as_ref()
            .ok_or("no file to save to: pass --player-settings (HOME isn't set)")?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(path, self.player_settings.to_yaml())?;
        Ok(path.display().to_string())
    }

    /// `handle_control` does what a pressed control (key, mouse button or wheel) is bound to:
    /// releasing the mouse (or asking whether to exit when it is free), the render resolution,
    /// choosing a team in the lobby (before the guns, which may share its keys), the guns, the
    /// view (first or third person), and firing the sniper.
    ///
    /// Example: `self.handle_control(control, window, mouse);`
    fn handle_control(
        &mut self,
        control: Control,
        window: &sdl3::video::Window,
        mouse: &sdl3::mouse::MouseUtil,
    ) {
        let actions: Vec<Action> = self.player_settings.keys.get_actions(control);
        let is = |action: Action| actions.contains(&action);
        if is(Action::ReleaseMouse) {
            if self.exit_prompt.press_release_mouse(self.is_captured)
                == exit_prompt::ExitOutcome::ReleaseMouse
            {
                self.is_captured = false;
                mouse.set_relative_mouse_mode(window, false);
            }
        } else if is(Action::LowerResolution) || is(Action::RaiseResolution) {
            self.is_auto_scale = false;
            self.render_scale = if is(Action::LowerResolution) {
                (self.render_scale / RENDER_SCALE_STEP).max(MIN_RENDER_SCALE)
            } else {
                (self.render_scale * RENDER_SCALE_STEP).min(1.0)
            };
        } else if self.is_in_lobby() && (is(Action::JoinBlue) || is(Action::JoinRed)) {
            let team: u32 = u32::from(is(Action::JoinRed));
            let multiplayer: &mut Multiplayer = self.multiplayer.as_mut().expect("in the lobby");
            if let Err(error) = multiplayer.client.join(Some(team)) {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
        } else if is(Action::Gun1) {
            self.switch_gun(Gun::Laser);
        } else if is(Action::Gun2) {
            self.switch_gun(Gun::Rifle);
        } else if is(Action::SwitchGun) {
            self.switch_gun(match self.gun {
                Gun::Laser => Gun::Rifle,
                Gun::Rifle => Gun::Laser,
            });
        } else if is(Action::SwitchView) {
            self.view_switch.toggle();
        }
        if is(Action::ThrowWormhole) && self.can_throw() {
            self.throw_wormhole();
        }
        if is(Action::Shoot)
            && self.is_captured
            && self.gun == Gun::Laser
            && self.sniper_reload_s <= 0.0
            && self.can_shoot()
            && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated)
        {
            self.shot_time_s = self.settings.sniper.shot_s;
            self.has_shot_hit_wormholes = false;
            self.sniper_reload_s = self.settings.sniper.reload_s;
            self.shot_offset = self.shot_noise.sample_sniper(&self.settings.sniper);
            self.beam_speckle_frac = self.shot_noise.sample_sniper_speckle(&self.settings.sniper);
        }
    }

    /// `update` moves the player by the held keys for the time since the last frame, and fails if
    /// the game server can't be read (it plays another map, say). Near a black hole the player's
    /// own time runs slower (W-107, `get_time_rate_frac`): moving, turning off a console and the
    /// guns go by that time, while looking around, the menus and respawning keep the window's.
    ///
    /// Example: `session.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;`
    pub(crate) fn update(
        &mut self,
        keyboard: &sdl3::keyboard::KeyboardState,
        mouse: &sdl3::mouse::MouseState,
    ) -> Result<()> {
        let frame_s: f64 = self.last.elapsed().as_secs_f64();
        let dt_s: f64 = frame_s.min(MAX_FRAME_DT_S);
        let own_dt_s: f64 = dt_s * self.get_time_rate_frac();
        let blur_s: f64 = self.player_settings.view.motion_blur_s;
        self.hit_effect_frac =
            (self.hit_effect_frac - dt_s / self.player_settings.hit_effects.effect_s).max(0.0);
        self.update_bar_pulses(dt_s);
        self.view_switch.update(dt_s);
        self.blueshift_frac = 0.0;
        self.motion_blur_frac = if blur_s > 0.0 {
            (-dt_s / blur_s).exp() as f32
        } else {
            0.0
        };
        self.last = Instant::now();
        if self.route_index < self.route.len() {
            self.frame_times_ms.push(frame_s * MS_PER_S);
            let step: &RouteFrame = &self.route[self.route_index];
            (self.route_eye, self.is_laser_on) = (step.frame, step.is_laser_on);
            self.route_index += 1;
            self.hud_time_s += dt_s;
            return Ok(());
        }
        self.shot_time_s = (self.shot_time_s - own_dt_s).max(0.0);
        self.sniper_reload_s = (self.sniper_reload_s - own_dt_s).max(0.0);
        if let Some(multiplayer) = &mut self.multiplayer
            && multiplayer.is_eliminated
        {
            multiplayer.respawn_in_s -= dt_s;
            if multiplayer.respawn_in_s <= 0.0 {
                multiplayer.is_eliminated = false;
                if let Err(error) = multiplayer.client.rejoin() {
                    self.message = format!("Couldn't rejoin: {error}");
                    self.message_time_s = MESSAGE_DURATION_S;
                }
            }
        }
        let is_out: bool = self.update_multiplayer()?;
        self.report_connection_problem();
        if is_out {
            self.message_time_s -= dt_s;
            self.hud_time_s += dt_s;
            return Ok(());
        }
        let is_playing: bool = self.menu.is_none() && !self.exit_prompt.is_shown();
        let held = |action: Action| {
            is_playing && self.player_settings.keys.is_held(action, keyboard, mouse)
        };
        let input: Input = Input {
            forward: held(Action::MoveForward),
            back: held(Action::MoveBack),
            left: held(Action::MoveLeft),
            right: held(Action::MoveRight),
            up: held(Action::Jump),
            down: false,
            sprint: held(Action::Sprint),
            crouch: held(Action::Crouch),
        };
        self.is_trigger_held = self.is_captured && held(Action::Shoot);
        self.is_flashlight_on = self.is_captured && held(Action::Flashlight);
        let is_aiming: bool = self.is_captured
            && held(Action::Aim)
            && !self.multiplayer.as_ref().is_some_and(|m| m.is_eliminated);
        let aim_step: f64 = dt_s / self.settings.aim.zoom_s;
        self.aim_frac =
            (self.aim_frac + if is_aiming { aim_step } else { -aim_step }).clamp(0.0, 1.0);
        let is_holding_e: bool = held(Action::TurnOffConsole);
        let team: usize = self.get_team() as usize;
        let report: voxel_world::StepReport =
            self.voxel
                .player
                .step(&self.voxel.world, &input, own_dt_s, team);
        let pitch_rad: f64 = self.voxel.player.pitch_rad;
        let view_m_per_s: f64 = report.forward_m_per_s * pitch_rad.cos()
            + self.voxel.player.vertical_m_per_s * pitch_rad.sin();
        let eye: DVec4 = self
            .voxel
            .player
            .get_camera_frame(&self.voxel.world)
            .col(AXIS_W);
        self.blueshift_frac = get_blueshift_frac(
            self.voxel.world.get_mouth_depth_frac(eye),
            view_m_per_s,
            self.voxel.world.player.walk_m_per_s,
            self.player_settings.view.wormhole_tint_frac,
        );
        for message in report.messages {
            self.message = message;
            self.message_time_s = MESSAGE_DURATION_S;
        }
        self.update_wormholes();
        self.update_disabling(is_holding_e, own_dt_s);
        self.update_gun(&input, own_dt_s);
        self.update_rifle(own_dt_s);
        let eye: DMat4 = self.get_eye_frame();
        let laser_aim: Option<DMat4> = self.get_laser_aim(self.get_shot_eye());
        if let Some(multiplayer) = &mut self.multiplayer
            && let Err(error) = multiplayer.client.send_state(
                laser_aim.unwrap_or(eye),
                laser_aim.is_some(),
                self.gun == Gun::Rifle,
                multiplayer.disabling.map(|(console, _)| console),
                self.beam_speckle_frac,
                self.thrown.placed,
            )
        {
            self.message = format!("Couldn't reach the server: {error}");
            self.message_time_s = MESSAGE_DURATION_S;
        }
        self.message_time_s -= dt_s;
        self.hud_time_s += dt_s;
        Ok(())
    }

    /// `update_bar_pulses` moves the pulses running along the HUD's bars on by `dt_s`
    /// (`hud_bars.rs`), driven by how fast each bar's value changes: this player's health in a
    /// game on a server, and the sprint energy on a voxel map.
    ///
    /// Example: `self.update_bar_pulses(dt_s);`
    fn update_bar_pulses(&mut self, dt_s: f64) {
        if let Some(multiplayer) = &self.multiplayer {
            self.health_pulses.update(
                dt_s,
                multiplayer.client.health_frac,
                u32::from(self.settings.rifle.body_hits_to_eliminate),
                &hud_bars::PULSE_TUNING,
            );
        }
        {
            self.energy_pulses.update(
                dt_s,
                self.voxel.player.energy_frac,
                ENERGY_SEGMENT_COUNT,
                &hud_bars::PULSE_TUNING,
            );
        }
    }

    /// `get_team` gives this player's team (an index into `net::TEAM_NAMES`): the one the server
    /// put them on, or the first without a server or before joining.
    ///
    /// Example: `let team = self.get_team();`
    fn get_team(&self) -> u32 {
        self.multiplayer
            .as_ref()
            .and_then(|m| m.client.team)
            .unwrap_or(0)
    }

    /// `is_in_lobby` says whether this player is on a server and hasn't chosen a team yet.
    ///
    /// Example: `if self.is_in_lobby() { ... }`
    fn is_in_lobby(&self) -> bool {
        self.multiplayer
            .as_ref()
            .is_some_and(|m| m.client.is_in_lobby())
    }

    /// `get_eye_frame` gives the eye's frame: the voxel player's camera.
    ///
    /// Example: `let eye = session.get_eye_frame();`
    fn get_eye_frame(&self) -> DMat4 {
        self.voxel.player.get_camera_frame(&self.voxel.world)
    }

    /// `show_message` shows a line in the HUD for `MESSAGE_DURATION_S`.
    ///
    /// Example: `self.show_message("player 3 was hit by player 2".to_string());`
    fn show_message(&mut self, message: String) {
        self.message = message;
        self.message_time_s = MESSAGE_DURATION_S;
    }

    /// `get_render_size` gives the size to ray-trace for a window of `window_px` pixels. With
    /// automatic resolution, a scale of `FULL_RESOLUTION_FROM_SCALE` or more renders at full
    /// resolution, as MetalFX costs about as much as those scales save and loses as much detail as
    /// lower ones; others round down to steps of `RENDER_SCALE_QUANTUM`.
    ///
    /// Example: `let render_px = session.get_render_size((2560, 1600));`
    pub(crate) fn get_render_size(&mut self, window_px: (u32, u32)) -> (u32, u32) {
        self.last_output_px = window_px;
        let quantized_scale: f64 = if !self.is_auto_scale {
            self.render_scale
        } else if self.render_scale >= FULL_RESOLUTION_FROM_SCALE {
            1.0
        } else {
            ((self.render_scale / RENDER_SCALE_QUANTUM + RENDER_SCALE_ROUNDING).floor()
                * RENDER_SCALE_QUANTUM)
                .max(MIN_AUTO_RENDER_SCALE)
        };
        self.last_used_scale = quantized_scale;
        if !self.route.is_empty() {
            self.render_scales.push(quantized_scale);
        }
        scale_size(window_px, quantized_scale)
    }

    /// `pack` packs this frame's scene (and laser) at a render size.
    ///
    /// Example: `let data = session.pack((1280, 800));`
    pub(crate) fn pack(&mut self, render_px: (u32, u32)) -> PackedFrame {
        let is_firing: bool = self.is_firing();
        let owned_markers: Vec<(Option<u32>, Marker)> = self.get_wormhole_markers();
        let markers: Vec<Marker> = owned_markers.iter().map(|&(_, marker)| marker).collect();
        let (off_mask, disabling_fracs) = self.get_console_states();
        let time_s: f64 = self.start.elapsed().as_secs_f64();
        {
            self.voxel
                .world
                .light_consoles(off_mask, &disabling_fracs, time_s);
            self.voxel.world.sky_time_s = spaceships::get_utc_s();
            self.voxel
                .black_hole_growth
                .update(&mut self.voxel.world, off_mask, time_s);
            let eye: DMat4 = self.voxel.player.get_camera_frame(&self.voxel.world);
            self.voxel.world.flashlight_eye = self.is_flashlight_on.then_some(eye);
        }
        {
            let eye: DMat4 = if self.route.is_empty() {
                self.voxel.player.get_camera_frame(&self.voxel.world)
            } else {
                self.route_eye
            };
            let third_person_view: Option<(third_person::ThirdPersonCamera, DMat4)> =
                self.get_third_person_view();
            let camera: DMat4 = third_person_view.map_or(eye, |(placed, _)| placed.frame);
            let laser_aim: Option<DMat4> =
                self.get_laser_aim(third_person_view.map_or(eye, |(_, shot_eye)| shot_eye));
            let team: u32 = self.get_team();
            let own_body: Option<(DMat4, usize)> = third_person_view
                .filter(|(placed, _)| placed.is_body_shown)
                .map(|_| multiplayer::get_body(&self.voxel.world, laser_aim.unwrap_or(eye), team));
            self.is_own_body_shown = own_body.is_some();
            let players: Vec<net::RemotePlayer> = self
                .multiplayer
                .as_ref()
                .map_or(Vec::new(), |m| m.client.players.clone());
            let (now_s, r) = (self.start.elapsed().as_secs_f64(), self.voxel.world.r);
            let mut eyes: Vec<(u32, DMat4)> = players.iter().map(|p| (p.id, p.frame)).collect();
            eyes.push((avatar_animation::OWN_TRACK_ID, laser_aim.unwrap_or(eye)));
            let mut poses: Vec<(avatar_model::AvatarClip, f64)> =
                self.avatars.update(&self.voxel.world, &eyes, now_s);
            poses.truncate(players.len() + usize::from(own_body.is_some()));
            let corpses: Vec<(DMat4, usize, (avatar_model::AvatarClip, f64))> = self
                .avatars
                .get_corpses(now_s, self.settings.respawn.delay_s)
                .into_iter()
                .map(|(corpse, pose)| (corpse.feet, corpse.team as usize, pose))
                .collect();
            self.tracers
                .retain(|tracer| tracer.get_lit_m(now_s).0 < tracer.length_m);
            self.dust_bursts.retain(|burst| !burst.is_over(now_s));
            let clouds: Vec<dust::DustCloud> = self
                .dust_bursts
                .iter()
                .filter_map(|burst| burst.get_cloud(now_s))
                .collect();
            let beams: Vec<render_voxel::LaserBeam> = self.get_lit_beams(r);
            let spot: Option<(glam::DVec4, u32)> = self
                .tracers
                .iter()
                .rev()
                .find(|tracer| tracer.get_lit_m(now_s).1 >= tracer.length_m)
                .and_then(|tracer| tracer.spot.map(|x| (x, tracer.team)));
            let view: multiplayer::SharedVoxelView = multiplayer::pack_shared_voxel_view(
                &self.voxel.world,
                camera,
                laser_aim,
                team,
                &players,
                &poses,
                &markers,
                own_body,
                &corpses,
                self.route.is_empty().then_some(multiplayer::Tracers {
                    beams: &beams,
                    spot,
                    dust: &clouds,
                }),
                render_px,
            );
            if is_firing
                && !self.was_firing
                && let Some(shot) = &view.shot
            {
                let is_rifle: bool = self.gun == Gun::Rifle;
                let speckle_frac: f64 = self.beam_speckle_frac;
                let tracer = Tracer {
                    is_own: true,
                    ..Tracer::New(shot, team, now_s, is_rifle, speckle_frac, &self.settings, r)
                };
                add_dust_burst(
                    &mut self.dust_bursts,
                    &self.voxel.world,
                    shot,
                    &tracer,
                    is_rifle,
                );
                self.tracers.push(tracer);
            }
            self.was_firing = is_firing;
            for (id, shot) in &view.other_shots {
                if !self.remote_firing_ids.contains(id) {
                    let shooter: Option<&net::RemotePlayer> = players.iter().find(|p| p.id == *id);
                    let (team, is_rifle, speckle_frac) = shooter.map_or((0, false, 0.0), |p| {
                        (p.team, p.is_rifle, p.beam_speckle_frac)
                    });
                    let tracer =
                        Tracer::New(shot, team, now_s, is_rifle, speckle_frac, &self.settings, r);
                    add_dust_burst(
                        &mut self.dust_bursts,
                        &self.voxel.world,
                        shot,
                        &tracer,
                        is_rifle,
                    );
                    self.tracers.push(tracer);
                }
            }
            self.remote_firing_ids = view.other_shots.iter().map(|(id, _)| *id).collect();
            self.laser = multiplayer::describe_voxel_laser(&self.voxel.world, &view);
            let first: usize = self.voxel.world.static_warp_count;
            if let Some(victim_id) = view.hit_player_id
                && let Some(hits_to_kill) = take_hit_weight(
                    &self.settings,
                    self.gun,
                    &mut self.has_rifle_shot_hit,
                    view.is_head_hit,
                )
                && let Some(multiplayer) = &mut self.multiplayer
                && let Err(error) = multiplayer.client.send_hit(victim_id, hits_to_kill)
            {
                self.show_message(format!("Couldn't reach the server: {error}"));
            }
            let entered: Vec<usize> = view.shot.as_ref().map_or(Vec::new(), |shot| {
                shot.through_warps
                    .iter()
                    .filter_map(|w| w.checked_sub(first).map(|k| k / 2))
                    .collect()
            });
            self.report_wormhole_hits(&entered, view.hit_marker, &owned_markers);
            PackedFrame::Voxel(Box::new(view.frame))
        }
    }

    /// `record_frame` adds one frame's render time to the HUD's averages, which refresh every
    /// `HUD_PERIOD_S`. With automatic resolution, each frame also sets the render scale to the one
    /// whose GPU time would be `AUTO_SCALE_TARGET_GPU_MS` (time grows with the pixel count, the
    /// scale's square), by down to `AUTO_SCALE_MAX_DROP` a frame when that is lower and by at most
    /// `AUTO_SCALE_MAX_STEP` a frame when it is higher, between `MIN_AUTO_RENDER_SCALE` and 1; the
    /// render size follows it,
    /// rounded down to steps of `RENDER_SCALE_QUANTUM`.
    ///
    /// Example: `session.record_frame(12.5, renderer.gpu_ms());`
    pub(crate) fn record_frame(&mut self, render_ms: f64, gpu_ms: Option<f64>) {
        self.hud_frame_count += 1;
        self.hud_render_ms += render_ms;
        self.gpu_ms = gpu_ms;
        self.total_frame_count += 1;
        self.total_gpu_ms += gpu_ms.unwrap_or(render_ms);
        if let Some(ms) = gpu_ms
            && self.is_auto_scale
        {
            let wanted: f64 = self.last_used_scale * (AUTO_SCALE_TARGET_GPU_MS / ms).sqrt();
            let raised: f64 = (self.render_scale * AUTO_SCALE_MAX_STEP).min(wanted);
            let lowered: f64 = wanted.max(self.last_used_scale * AUTO_SCALE_MAX_DROP);
            self.render_scale = if wanted < self.render_scale {
                lowered
            } else {
                raised
            }
            .clamp(MIN_AUTO_RENDER_SCALE, 1.0);
        }
        if self.hud_time_s <= HUD_PERIOD_S {
            return;
        }
        self.fps = f64::from(self.hud_frame_count) / self.hud_time_s;
        self.render_ms = self.hud_render_ms / f64::from(self.hud_frame_count);
        (self.hud_time_s, self.hud_frame_count, self.hud_render_ms) = (0.0, 0, 0.0);
    }

    /// `print_route_statistics` prints how the window kept up while playing the route, after its
    /// first `ROUTE_WARMUP_FRAME_COUNT` frames: the time between frames (median, 99th percentile,
    /// worst, and how many took longer than a 60 Hz frame) and the render scale it used.
    ///
    /// Example: `session.print_route_statistics();`
    fn print_route_statistics(&self) {
        let mut times_ms: Vec<f64> = self.frame_times_ms[ROUTE_WARMUP_FRAME_COUNT..].to_vec();
        times_ms.sort_by(f64::total_cmp);
        let scales: &[f64] = &self.render_scales[ROUTE_WARMUP_FRAME_COUNT..];
        let percentile =
            |fraction: f64| times_ms[((times_ms.len() - 1) as f64 * fraction).round() as usize];
        let over_count: usize = times_ms.iter().filter(|&&ms| ms > FRAME_BUDGET_MS).count();
        println!(
            "{} route frames in a {}x{} window: {:.1} fps on average; frame times median {:.1} ms, 99th percentile {:.1} ms, worst {:.1} ms; {} frames ({:.1}%) over {:.1} ms; render scale mean {:.2}, lowest {:.2}, full in {:.1}% of frames",
            times_ms.len(),
            self.last_output_px.0,
            self.last_output_px.1,
            MS_PER_S * times_ms.len() as f64 / times_ms.iter().sum::<f64>(),
            percentile(0.5),
            percentile(0.99),
            times_ms[times_ms.len() - 1],
            over_count,
            100.0 * over_count as f64 / times_ms.len() as f64,
            FRAME_BUDGET_MS,
            scales.iter().sum::<f64>() / scales.len() as f64,
            scales.iter().copied().fold(1.0, f64::min),
            100.0 * scales.iter().filter(|&&scale| scale >= 1.0).count() as f64
                / scales.len() as f64,
        );
    }

    /// `is_finished` says whether `--exit-after-s` has run out; if so it prints the window's
    /// average frame rate, GPU time and last render scale.
    ///
    /// Example: `if session.is_finished() { return Ok(()); }`
    pub(crate) fn is_finished(&self) -> bool {
        if !self.route.is_empty() {
            if self.route_index < self.route.len() {
                return false;
            }
            self.print_route_statistics();
            return true;
        }
        let Some(limit_s) = self.exit_after_s else {
            return false;
        };
        let elapsed_s: f64 = self.start.elapsed().as_secs_f64();
        if elapsed_s < limit_s {
            return false;
        }
        let frame_count: f64 = self.total_frame_count.max(1) as f64;
        println!(
            "{} frames in {elapsed_s:.1} s: {:.1} fps, {:.1} ms per frame on the GPU, render scale {:.2} of a {}x{} window",
            self.total_frame_count,
            frame_count / elapsed_s,
            self.total_gpu_ms / frame_count,
            self.render_scale,
            self.last_output_px.0,
            self.last_output_px.1
        );
        true
    }
}

/// `scale_size` scales a size by a factor, keeping each side at least `MIN_IMAGE_SIDE_PX`.
///
/// Example: `let render_px = scale_size((2560, 1600), 0.5);`
pub(crate) fn scale_size(size_px: (u32, u32), scale: f64) -> (u32, u32) {
    (
        ((f64::from(size_px.0) * scale).round() as u32).max(MIN_IMAGE_SIDE_PX),
        ((f64::from(size_px.1) * scale).round() as u32).max(MIN_IMAGE_SIDE_PX),
    )
}

/// `get_eyes` gives where other players' eyes are, so spawning players can keep clear of them.
///
/// Example: `let others = get_eyes(&client.players);`
fn get_eyes(players: &[net::RemotePlayer]) -> Vec<DVec4> {
    players
        .iter()
        .map(|player| player.frame.col(AXIS_W))
        .collect()
}

/// `get_points_and_teams` gives where other players are (their eyes) and their teams (indices
/// into `net::TEAM_NAMES`), for the map page.
///
/// Example: `let others = get_points_and_teams(&client.players);`
fn get_points_and_teams(players: &[net::RemotePlayer]) -> Vec<(DVec4, u32)> {
    players
        .iter()
        .map(|player| (player.frame.col(AXIS_W), player.team))
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests of the guns' hit rules and models in hand, aiming's zoom, where overlay text goes and
    //! the tint after a wormhole.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Depth in a mouth | at the throat (1), part way, outside (0) | `wormhole_shift_follows_depth_and_speed` |
    //! | Speed along the view | forward (blueshift), backward (redshift), still (none), past the cap (held at it) | `wormhole_shift_follows_depth_and_speed` |
    //! | Tint strength setting | 1, between 0 and 1, 0 | `wormhole_shift_follows_depth_and_speed` |
    //! | Gun | laser (every frame's hit counts, one eliminates), rifle (one hit per shot) | `hits_weigh_by_gun_and_place` |
    //! | Where a rifle hits | the head, the body | `hits_weigh_by_gun_and_place` |
    //! | Gun in hand | laser (the bolt-action rifle), rifle (the revolver) | `each_gun_holds_its_model` |
    //! | Map loading | loads every gun model | `each_gun_holds_its_model` |
    //! | Aim fraction | not aiming, half way (eased, half the extra zoom), fully aimed, past either end (clamped) | `aiming_zooms_in_eased` |
    //! | Gun model swaying | the bolt-action rifle (full swing), the revolver (its swing scaled by its length) | `guns_sway_with_the_steps_by_their_length` |
    //! | Stepping fraction | standing (0: no sway), easing in (0.5: half), stepping (1) | `guns_sway_with_the_steps_by_their_length` |
    //! | Run pose | walking (0: the walking sway), sprinting (1: the sprinting sway), half way (eased between) | `guns_sway_with_the_steps_by_their_length` |
    //! | Step phase | between steps (0: centred), one step (a quarter turn: right), the next (three quarters: left) | `guns_sway_with_the_steps_by_their_length` |
    //! | Overlay text | a centred two-line banner, the exit prompt's banner uncut, the lobby's banner connecting to the longest address and connected with full teams, the left-aligned menu with a line too long for its mask, the scores page with more players than it shows and five-digit kills and deaths | `overlay_text_fits_its_mask` |

    use super::hud::*;
    use super::weapons::*;
    use super::*;
    use std::net::SocketAddr;

    use crate::*;

    #[test]
    fn wormhole_shift_follows_depth_and_speed() {
        const WALK_M_PER_S: f64 = 10.0;
        const SPRINT_M_PER_S: f64 = 18.0;
        const HALF_FRAC: f64 = 0.5;
        const WEAK_STRENGTH_FRAC: f64 = 0.4;
        let shift = |depth_frac: f64, view_m_per_s: f64, strength_frac: f64| -> f64 {
            f64::from(get_blueshift_frac(
                depth_frac,
                view_m_per_s,
                WALK_M_PER_S,
                strength_frac,
            ))
        };
        assert_eq!(shift(1.0, WALK_M_PER_S, 1.0), 1.0);
        assert_eq!(shift(1.0, -WALK_M_PER_S, 1.0), -1.0);
        assert_eq!(shift(1.0, 0.0, 1.0), 0.0);
        assert_eq!(shift(0.0, WALK_M_PER_S, 1.0), 0.0);
        assert_eq!(shift(1.0, WALK_M_PER_S, 0.0), 0.0);
        let part_way: f64 = shift(HALF_FRAC, -WALK_M_PER_S, WEAK_STRENGTH_FRAC);
        assert!((part_way + HALF_FRAC * WEAK_STRENGTH_FRAC).abs() < 1e-6);
        assert!((shift(1.0, SPRINT_M_PER_S, 1.0) - MAX_BLUESHIFT_FRAC).abs() < 1e-6);
        assert!((shift(1.0, -SPRINT_M_PER_S, 1.0) + MAX_BLUESHIFT_FRAC).abs() < 1e-6);
    }

    #[test]
    fn guns_sway_with_the_steps_by_their_length() {
        const TOLERANCE: f64 = 1e-6;
        const HALF_FRAC: f64 = 0.5;
        let head_bob: settings::HeadBobSettings = PerPlayerSettings::Builtin().view.head_bob;
        let right_rad: f64 = std::f64::consts::FRAC_PI_2;
        let left_rad: f64 = 3.0 * std::f64::consts::FRAC_PI_2;
        let sway =
            |model: viewmodel::GunModel, step_frac: f64, run_pose_frac: f64, phase_rad: f64| {
                let (sway_m, swing_rad) =
                    get_sway(model, step_frac, run_pose_frac, phase_rad, &head_bob);
                (f64::from(sway_m), f64::from(swing_rad))
            };
        let is_near = |a: (f64, f64), b: (f64, f64)| {
            (a.0 - b.0).abs() < TOLERANCE && (a.1 - b.1).abs() < TOLERANCE
        };
        let rifle = viewmodel::GunModel::BoltActionRifle;
        let revolver = viewmodel::GunModel::Revolver;
        let (walk_m, sprint_m) = (head_bob.gun_walk_sway_m, head_bob.gun_sprint_sway_m);
        let (walk_rad, sprint_rad) = (
            head_bob.rifle_walk_swing_rad,
            head_bob.rifle_sprint_swing_rad,
        );
        assert!(walk_m > 0.0 && sprint_m > walk_m && walk_rad > 0.0 && sprint_rad > walk_rad);
        assert!(is_near(sway(rifle, 0.0, 1.0, right_rad), (0.0, 0.0)));
        assert!(is_near(sway(rifle, 1.0, 0.0, 0.0), (0.0, 0.0)));
        assert!(is_near(
            sway(rifle, 1.0, 0.0, right_rad),
            (walk_m, walk_rad)
        ));
        assert!(is_near(
            sway(rifle, 1.0, 0.0, left_rad),
            (-walk_m, -walk_rad)
        ));
        assert!(is_near(
            sway(rifle, 1.0, 1.0, right_rad),
            (sprint_m, sprint_rad)
        ));
        assert!(is_near(
            sway(rifle, HALF_FRAC, 0.0, right_rad),
            (HALF_FRAC * walk_m, HALF_FRAC * walk_rad)
        ));
        let half_way: (f64, f64) = sway(rifle, 1.0, HALF_FRAC, right_rad);
        assert!(is_near(
            half_way,
            (
                (walk_m + sprint_m) * HALF_FRAC,
                (walk_rad + sprint_rad) * HALF_FRAC
            )
        ));
        let length_frac: f64 =
            f64::from(revolver.get_modelled_length_m() / rifle.get_modelled_length_m());
        assert!(length_frac < 1.0);
        assert!(is_near(
            sway(revolver, 1.0, 1.0, right_rad),
            (sprint_m, sprint_rad * length_frac)
        ));
    }

    #[test]
    fn each_gun_holds_its_model() {
        assert_eq!(Gun::Laser.get_model(), viewmodel::GunModel::BoltActionRifle);
        assert_eq!(Gun::Rifle.get_model(), viewmodel::GunModel::Revolver);
        let steps: &[render_metal::LoadingStep] = render_metal::get_loading_steps();
        for gun in [Gun::Laser, Gun::Rifle] {
            let step: render_metal::LoadingStep = render_metal::LoadingStep::Gun(gun.get_model());
            assert_eq!(steps.iter().filter(|s| **s == step).count(), 1);
        }
    }

    #[test]
    fn hits_weigh_by_gun_and_place() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let mut has_hit: bool = false;
        for _ in 0..2 {
            assert_eq!(
                take_hit_weight(&settings, Gun::Laser, &mut has_hit, false),
                Some(settings.sniper.hits_to_eliminate)
            );
        }
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, true),
            Some(settings.rifle.head_hits_to_eliminate)
        );
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false),
            None
        );
        has_hit = false;
        assert_eq!(
            take_hit_weight(&settings, Gun::Rifle, &mut has_hit, false),
            Some(settings.rifle.body_hits_to_eliminate)
        );
    }

    #[test]
    fn overlay_text_fits_its_mask() {
        let banner: OverlayText = OverlayText {
            lines: vec![
                ("YOU DIED".to_string(), true),
                ("Respawning in 3".to_string(), false),
            ],
            mask_px: DEATH_MASK_PX,
            height_frac: BANNER_HEIGHT_FRAC,
            is_centered: true,
            dim_frac: DEATH_DIM_FRAC,
            picture: None,
        };
        let placed = get_text_positions(&banner);
        let title_px: f32 = DEBUG_GLYPH_PX * "YOU DIED".len() as f32;
        assert_eq!(placed[0].1.0 * 2.0 + title_px, DEATH_MASK_PX.0 as f32);
        assert!(placed[0].2 && !placed[1].2);
        let long_path: String = "/very/long".repeat(20);
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        let scores_menu: menu::Menu = menu::Menu::New();
        let mut settings_menu: menu::Menu = menu::Menu::New();
        settings_menu.handle(Control::Key(sdl3::keyboard::Scancode::E), &mut mine);
        let mut credits_menu: menu::Menu = menu::Menu::New();
        credits_menu.handle(Control::Key(sdl3::keyboard::Scancode::Q), &mut mine);
        let scores: Vec<net::Score> = (0..3 * net::MAX_PLAYER_COUNT as u32)
            .map(|id| net::Score {
                id: u32::MAX - id,
                team: id % net::TEAM_NAMES.len() as u32,
                kills: u32::from(u16::MAX),
                deaths: u32::from(u16::MAX),
            })
            .collect();
        let scoreboard = menu::Scoreboard::Scores {
            scores: &scores,
            own_id: Some(u32::MAX),
        };
        let max_char_count: usize = (MENU_MASK_PX.0 as f32 / DEBUG_GLYPH_PX) as usize;
        for line in scores_menu.get_lines(&mine, &long_path, scoreboard) {
            assert!(line.0.chars().count() <= max_char_count, "{}", line.0);
        }
        for menu in [settings_menu, scores_menu, credits_menu] {
            let menu_text: OverlayText = OverlayText {
                lines: menu.get_lines(&mine, &long_path, scoreboard),
                mask_px: MENU_MASK_PX,
                height_frac: MENU_HEIGHT_FRAC,
                is_centered: false,
                dim_frac: MENU_DIM_FRAC,
                picture: None,
            };
            for (line, (x, _), _) in get_text_positions(&menu_text) {
                assert_eq!(x, MENU_LEFT_PX);
                assert!(line.chars().count() <= max_char_count, "{line}");
            }
        }
        let banner_char_count: usize = (DEATH_MASK_PX.0 as f32 / DEBUG_GLYPH_PX) as usize;
        for line in exit_prompt::get_lines() {
            assert!(line.chars().count() <= banner_char_count, "{line}");
        }
        let longest_server: SocketAddr = "255.255.255.255:65535".parse().expect("an address");
        let full_counts: [u32; 2] = [net::MAX_PLAYER_COUNT as u32; 2];
        let keys_line: String = "1 joins blue, 2 joins red".to_string();
        let connecting = get_lobby_lines(None, longest_server, keys_line.clone());
        let connected = get_lobby_lines(Some(full_counts), longest_server, keys_line);
        assert!(connecting[0].starts_with("CONNECTING") && connected[0].starts_with("CONNECTED "));
        for line in connecting.iter().chain(&connected) {
            assert!(line.chars().count() <= banner_char_count, "{line}");
        }
    }

    #[test]
    fn aiming_zooms_in_eased() {
        const FULL_ZOOM: f64 = 2.5;
        assert_eq!(get_aim_zoom(0.0, FULL_ZOOM), 1.0);
        assert_eq!(get_aim_zoom(0.5, FULL_ZOOM), 1.0 + (FULL_ZOOM - 1.0) / 2.0);
        assert_eq!(get_aim_zoom(1.0, FULL_ZOOM), FULL_ZOOM);
        assert_eq!(get_aim_zoom(-1.0, FULL_ZOOM), 1.0);
        assert_eq!(get_aim_zoom(2.0, FULL_ZOOM), FULL_ZOOM);
        assert!(
            get_aim_zoom(0.1, FULL_ZOOM) - 1.0 < 0.1 * (FULL_ZOOM - 1.0),
            "eased in"
        );
    }
}
