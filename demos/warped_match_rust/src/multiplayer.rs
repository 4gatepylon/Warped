//! `multiplayer.rs` brings the other players into the game. Each is drawn as a box around their
//! eye, added to a copy of the world every frame, so the ray tracer bends them through wormholes
//! like everything else; everyone's laser is traced and drawn; and this player's laser reaching
//! another player is a hit. It also has the windowless bot that `--bot` runs to test a game.
//!
//! On voxel maps the same happens with `VoxelWorld`: bodies are voxel structures and lasers are
//! traced through the map (`pack_shared_voxel_view`, `run_voxel_bot`).
//!
//! Interface: `get_spawn_player`, `SharedView`, `pack_shared_view`, `run_bot`,
//! `SharedVoxelView`, `get_body`, `with_bodies`, `pack_shared_voxel_view`, `run_voxel_bot`.
//!
//! Example: `let view = multiplayer::pack_shared_view(&world, frame, Some(frame), &client.players, &[], (1280, 800), 0.0);`

use std::error::Error;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3, DVec4};

use crate::BotObjective;
use crate::avatar_model::AvatarClip;
use crate::black_hole_growth::BlackHoleGrowth;
use crate::dust::DustCloud;
use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, get_local_direction, make_boost, make_rotation,
    normalize_tangent,
};
use crate::minimap::TEAM_COLORS;
use crate::net::{Client, NetEvent, RemotePlayer};
use crate::physics::{self, Input, LaserReport, Player};
use crate::render_voxel::{LaserBeam, VoxelFrame};
use crate::scene_data::{Beam, SceneData};
use crate::settings::{ServerSettings, SpaceSettings};
use crate::thrown_wormholes::{Marker, PlayerWormholes};
use crate::voxel_world::{
    LOOK_RAD_PER_PX, MARKER_NAME, MIN_SPAWN_GAP_M, SPAWN_TURN_RAD, Shot, VoxelPlayer, VoxelWorld,
};
use crate::world::{Color, EYE_HEIGHT_M, World};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const AVATAR_HALF_M: [f64; 3] = [0.3, 0.95, 0.3];
const AVATAR_DROP_M: f64 = 0.75;
const SPAWN_RING_M: f64 = 3.0;
const IDLE_POSE: (AvatarClip, f64) = (AvatarClip::Idle, 0.0);
const BOT_STEP_S: f64 = 1.0 / 60.0;
const BOT_HITS_TO_KILL: u8 = 1;
const BOT_RANGE_M: f64 = 12.0;
const BOT_WANDER_PX_PER_S: f64 = 120.0;
const BOT_REJOIN_S: f64 = 2.0;
const BOT_STUCK_FRAC: f64 = 0.5;
const BOT_VIEW_PX: (u32, u32) = (1280, 800);
const VOXEL_SPAWN_RING_M: f64 = 3.0;
const SPAWN_SLOT_COUNT: u32 = 16;
const VOXEL_BOT_LASER_RANGE_M: f64 = 120.0;

/// `get_spawn_player` makes player `id`'s starting player on the house: on a ring around spawn,
/// facing the middle, at the first bearing (starting from the id's own) at least
/// `MIN_SPAWN_GAP_M` from every eye in `others`, or the last one tried.
///
/// Example: `let player = multiplayer::get_spawn_player(&world, 3, &other_eyes);`
pub fn get_spawn_player(world: &World, id: u32, others: &[DVec4]) -> Player {
    let mut player: Player = Player::AtSpawn(world);
    for k in 0..SPAWN_SLOT_COUNT {
        player.frame = world
            .make_frame_from_spawn(f64::from(id + k) * SPAWN_TURN_RAD, SPAWN_RING_M)
            * make_boost(AXIS_Y, EYE_HEIGHT_M / world.curvature_radius_m)
            * make_rotation(AXIS_X, AXIS_Z, std::f64::consts::PI);
        let eye: DVec4 = player.frame.col(AXIS_W);
        if others
            .iter()
            .all(|&x| get_distance(eye, x) * world.curvature_radius_m >= MIN_SPAWN_GAP_M)
        {
            break;
        }
    }
    player
}

/// `get_avatar_frame` gives the frame of the box drawn for a player whose eye is at `eye`: upright
/// (whatever their pitch), facing their way, its centre `AVATAR_DROP_M` below the eye.
///
/// Example: `let frame = get_avatar_frame(&world, player.frame);`
fn get_avatar_frame(world: &World, eye: DMat4) -> DMat4 {
    let up: DVec3 = physics::get_up_local(eye);
    let back: DVec3 = (DVec3::Z - up * up.z).try_normalize().unwrap_or(DVec3::Y);
    let level: DMat4 = DMat4::from_cols(
        up.cross(back).extend(0.0),
        up.extend(0.0),
        back.extend(0.0),
        DVec4::W,
    );
    eye * level * make_boost(AXIS_Y, -AVATAR_DROP_M / world.curvature_radius_m)
}

/// `SharedView` is one frame's packed scene with the other players and every laser, this
/// player's laser report, the player its laser hit, if any, and the marker it hit (an index into
/// the markers given), if any.
pub struct SharedView {
    pub data: SceneData,
    pub report: Option<LaserReport>,
    pub hit_player_id: Option<u32>,
    pub hit_marker: Option<usize>,
}

/// `get_team_colors` gives each team's colour for players' bodies, the same as the minimap's and
/// the map page's marks (`minimap::TEAM_COLORS`), indexed by team.
///
/// Example: `let blue = get_team_colors()[0];`
fn get_team_colors() -> Vec<Color> {
    TEAM_COLORS.iter().map(|&[r, g, b, _]| [r, g, b]).collect()
}

/// `get_avatar_bodies` gives each of `players`' bodies on a voxel map: its feet frame and the
/// index of its colour in `get_team_colors`, which is its team.
///
/// Example: `let bodies = get_avatar_bodies(&world, &players);`
fn get_avatar_bodies(world: &VoxelWorld, players: &[RemotePlayer]) -> Vec<(DMat4, usize)> {
    players
        .iter()
        .map(|p| get_body(world, p.frame, p.team))
        .collect()
}

/// `pack_shared_view` packs the scene seen from `frame` with a box for each of `players` and a
/// glowing cube for each of `markers`, this player's laser (if on, fired from `laser_aim`: `frame`
/// turned by the shot's noise) and the lasers of the players who have theirs on, each traced
/// through the same scene, so beams stop at players and bend through wormholes.
///
/// Example: `let view = multiplayer::pack_shared_view(&world, frame, Some(frame), &players, &[], (1280, 800), 0.0);`
pub fn pack_shared_view(
    world: &World,
    frame: DMat4,
    laser_aim: Option<DMat4>,
    players: &[RemotePlayer],
    markers: &[Marker],
    render_px: (u32, u32),
    time_s: f64,
) -> SharedView {
    let mut shared: World = world.clone();
    let first_avatar_box: usize = shared.boxes.len();
    for player in players {
        let color: Color = get_team_colors()[player.team as usize];
        let name: String = format!("player {}", player.id);
        shared.add_avatar(
            get_avatar_frame(world, player.frame),
            &name,
            AVATAR_HALF_M,
            color,
        );
    }
    shared.add_markers(markers);
    let trace_from = |eye: DMat4| {
        let data: SceneData =
            SceneData::Build(&shared, eye, &Beam::default(), render_px, time_s, true);
        physics::trace_laser(&shared, &data)
    };
    let (mut beam, mut report, mut hit_player_id, mut hit_marker) =
        (Beam::default(), None, None, None);
    if let Some(aim) = laser_aim {
        let (own_beam, mut own_report) = trace_from(aim);
        let past_avatars: Option<usize> = own_report
            .hit_box
            .and_then(|b| b.checked_sub(first_avatar_box));
        hit_player_id = past_avatars
            .and_then(|k| players.get(k))
            .map(|player| player.id);
        hit_marker = past_avatars.and_then(|k| k.checked_sub(players.len()));
        if let Some(id) = hit_player_id {
            own_report.hit = Some(format!("player {id}"));
        }
        (beam, report) = (own_beam, Some(own_report));
    }
    for player in players.iter().filter(|player| player.is_laser_on) {
        beam.runs.extend(trace_from(player.frame).0.runs);
    }
    SharedView {
        data: SceneData::Build(&shared, frame, &beam, render_px, time_s, true),
        report,
        hit_player_id,
        hit_marker,
    }
}

/// `check_bot_space` fails if the server that welcomed `client` curves space other than the
/// built-in settings do, which a bot builds its world with.
///
/// Example: `check_bot_space(&client)?;`
fn check_bot_space(client: &Client) -> Result<()> {
    let space: &SpaceSettings = &client
        .settings
        .as_ref()
        .expect("a welcome brings the server's settings")
        .space;
    let builtin: SpaceSettings = ServerSettings::Builtin().space;
    if *space != builtin {
        return Err(format!(
            "the bot builds the map at the built-in space.curvature_per_map_curvature ({}), but the server uses {}; give the server the built-in value",
            builtin.curvature_per_map_curvature, space.curvature_per_map_curvature
        )
        .into());
    }
    Ok(())
}

/// `run_bot` plays for `duration_s` seconds without a window, to test a game: it walks and turns
/// until another player is within `BOT_RANGE_M`, then turns toward them with its laser on; when
/// its laser hits someone it reports the hit and calls `save_view` with its view and the victim's
/// id; when it is hit it waits `BOT_REJOIN_S` and joins again. It returns how many players it hit
/// and how often it was hit.
///
/// Example: `let (hit_count, eliminated_count) = multiplayer::run_bot(&world, &mut client, 30.0, |data, id| Ok(()))?;`
pub fn run_bot(
    world: &World,
    client: &mut Client,
    duration_s: f64,
    mut save_view: impl FnMut(&SceneData, u32) -> Result<()>,
) -> Result<(u32, u32)> {
    let start: Instant = Instant::now();
    let mut player: Player = Player::AtSpawn(world);
    let (mut hit_count, mut eliminated_count) = (0, 0);
    let mut rejoin_at: Option<Instant> = None;
    let mut last_victim: Option<u32> = None;
    while start.elapsed().as_secs_f64() < duration_s {
        let tick: Instant = Instant::now();
        for event in client.poll()? {
            match event {
                NetEvent::Joined(id) => {
                    let others: Vec<DVec4> =
                        client.players.iter().map(|p| p.frame.col(AXIS_W)).collect();
                    player = get_spawn_player(world, id, &others);
                    println!("bot joined as player {id}");
                }
                NetEvent::Eliminated {
                    victim_id,
                    shooter_id,
                    is_self,
                } => {
                    println!("player {victim_id} was hit by player {shooter_id}");
                    if is_self {
                        eliminated_count += 1;
                        rejoin_at = Some(Instant::now() + Duration::from_secs_f64(BOT_REJOIN_S));
                    }
                }
            }
        }
        if let Some(time) = rejoin_at {
            if Instant::now() >= time {
                rejoin_at = None;
                client.rejoin()?;
            }
        } else if client.id.is_some() {
            let x: DVec4 = player.frame.col(AXIS_W);
            let target: Option<&RemotePlayer> = client
                .players
                .iter()
                .map(|other| (other, get_distance(x, other.frame.col(AXIS_W))))
                .filter(|(_, distance)| distance * world.curvature_radius_m < BOT_RANGE_M)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(other, _)| other);
            let mut input: Input = Input::default();
            if let Some(other) = target {
                physics::turn_toward(&mut player.frame, other.frame.col(AXIS_W));
            } else {
                input.forward = true;
                physics::look_around(&mut player.frame, BOT_WANDER_PX_PER_S * BOT_STEP_S, 0.0);
            }
            physics::update_player(world, &mut player, &input, BOT_STEP_S);
            let is_laser_on: bool = target.is_some();
            let view: SharedView = pack_shared_view(
                world,
                player.frame,
                is_laser_on.then_some(player.frame),
                &client.players,
                &[],
                BOT_VIEW_PX,
                start.elapsed().as_secs_f64(),
            );
            if let Some(victim_id) = view.hit_player_id {
                client.send_hit(victim_id, BOT_HITS_TO_KILL)?;
                if last_victim != Some(victim_id) {
                    hit_count += 1;
                    println!("bot hit player {victim_id}");
                    save_view(&view.data, victim_id)?;
                }
            }
            last_victim = view.hit_player_id;
            client.send_state(
                player.frame,
                is_laser_on,
                false,
                None,
                0.0,
                PlayerWormholes::default(),
            )?;
        } else {
            client.keep_joining()?;
        }
        std::thread::sleep(Duration::from_secs_f64(BOT_STEP_S).saturating_sub(tick.elapsed()));
    }
    client.leave()?;
    Ok((hit_count, eliminated_count))
}

/// `SharedVoxelView` is one frame of a voxel map with the other players, every laser, this
/// player's laser shot and the player it hit, if any, and whether it hit their head.
pub struct SharedVoxelView {
    pub frame: VoxelFrame,
    pub shot: Option<Shot>,
    pub hit_player_id: Option<u32>,
    pub hit_marker: Option<usize>,
    pub is_head_hit: bool,
    pub other_shots: Vec<(u32, Shot)>,
}

/// `Tracers` is what to draw for lasers instead of every laser's whole path: beam pieces with
/// their teams and speckle shares, an impact glow, and the dust of shots into the ground.
pub struct Tracers<'a> {
    pub beams: &'a [LaserBeam],
    pub spot: Option<(DVec4, u32)>,
    pub dust: &'a [DustCloud],
}

/// `get_body` gives the body drawn for a player of `team` whose eye frame is `eye`: its feet frame
/// and the index of its colour in `get_team_colors`, which is its team, as every window draws them.
///
/// Example: `let body = multiplayer::get_body(&world, player.frame, player.team);`
pub fn get_body(world: &VoxelWorld, eye: DMat4, team: u32) -> (DMat4, usize) {
    (
        VoxelPlayer::FromEye(world, eye).get_feet_frame(world),
        team as usize,
    )
}

/// `with_bodies` gives a copy of `world` with the body of each of `players` (`get_body`), as
/// `pack_shared_voxel_view` draws them and traces this player's laser.
///
/// Example: `let shared = multiplayer::with_bodies(&world, &client.players);`
pub fn with_bodies(world: &VoxelWorld, players: &[RemotePlayer]) -> VoxelWorld {
    world.with_avatars(&get_avatar_bodies(world, players), &get_team_colors())
}

/// `trace_other_voxel_lasers` traces the laser of each of `players` who has theirs on (their
/// `bodies` given), through `world` with every other player's body but not their own (their gun
/// is inside it, so their beam would stop where it starts), and with `own_body` (this player's,
/// when drawn), giving each player's id and shot.
///
/// Example: `let shots = trace_other_voxel_lasers(&world, &players, &bodies, None, &colors);`
fn trace_other_voxel_lasers(
    world: &VoxelWorld,
    players: &[RemotePlayer],
    bodies: &[(DMat4, usize)],
    own_body: Option<(DMat4, usize)>,
    colors: &[[f32; 3]],
) -> Vec<(u32, Shot)> {
    players
        .iter()
        .enumerate()
        .filter(|(_, player)| player.is_laser_on)
        .map(|(i, player)| {
            let others: Vec<(DMat4, usize)> = bodies
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, &body)| body)
                .chain(own_body)
                .collect();
            (
                player.id,
                world
                    .with_avatars(&others, colors)
                    .trace_laser(player.frame),
            )
        })
        .collect()
}

/// `pack_shared_voxel_view` packs a voxel map seen from `camera` with a body for each of
/// `players` (each in its pose in `poses`, as the body model) and a glowing ball for each of `markers`, this player's laser (if on, fired from
/// `laser_aim`, the eye turned by the shot's noise, for team `team`) and the lasers of the players
/// who have theirs on, all traced through the same map, so beams stop at players and bend like
/// light. In third person it also draws this player's own body (`own_body`, from `get_body`),
/// which the other players' lasers stop at but this player's own passes, as it leaves from inside
/// it. It draws every laser's whole path (this player's as the sniper's, without speckle; the
/// others' in their gun's style, with their shots' speckle), or `tracers` instead when given. `own_body` takes the pose after the players' in `poses`, and
/// the `corpses` still lying (a feet frame, team and pose each) are drawn last, where lasers pass
/// through them.
///
/// Example: `let view = multiplayer::pack_shared_voxel_view(&world, camera, Some(camera), 1, &players, &poses, &[], None, &[], None, (1280, 800));`
#[allow(clippy::too_many_arguments)]
pub fn pack_shared_voxel_view(
    world: &VoxelWorld,
    camera: DMat4,
    laser_aim: Option<DMat4>,
    team: u32,
    players: &[RemotePlayer],
    poses: &[(AvatarClip, f64)],
    markers: &[Marker],
    own_body: Option<(DMat4, usize)>,
    corpses: &[(DMat4, usize, (AvatarClip, f64))],
    tracers: Option<Tracers>,
    render_px: (u32, u32),
) -> SharedVoxelView {
    let colors: Vec<[f32; 3]> = get_team_colors();
    let bodies: Vec<(DMat4, usize)> = get_avatar_bodies(world, players);
    let own_bodies: Vec<(DMat4, usize)> = own_body.into_iter().collect();
    assert_eq!(
        poses.len(),
        bodies.len() + own_bodies.len(),
        "pack_shared_voxel_view: {} poses for {} players and {} own body",
        poses.len(),
        bodies.len(),
        own_bodies.len()
    );
    let (player_poses, own_poses) = poses.split_at(bodies.len());
    let mut shared: VoxelWorld = world.with_posed_avatars(&bodies, player_poses, &[], &colors);
    shared.add_markers(markers);
    let mut beams: Vec<LaserBeam> = Vec::new();
    let (mut shot, mut hit_player_id, mut spot, mut is_head_hit) = (None, None, None, false);
    let mut hit_marker: Option<usize> = None;
    if let Some(aim) = laser_aim {
        let own: Shot = shared.trace_laser(aim);
        let past_static: Option<usize> = own
            .hit_structure
            .and_then(|k| k.checked_sub(shared.static_structure_count));
        hit_player_id = past_static
            .and_then(|k| players.get(k))
            .map(|player| player.id);
        hit_marker = past_static.and_then(|k| k.checked_sub(players.len()));
        is_head_hit = hit_player_id.is_some()
            && own
                .hit_structure
                .zip(own.spot)
                .is_some_and(|(k, x)| shared.is_head_hit(k, x));
        spot = own.spot.map(|x| (x, team));
        beams.push(LaserBeam {
            segments: own.segments.clone(),
            team,
            is_rifle: false,
            speckle_frac: 0.0,
        });
        shot = Some(own);
    }
    let other_shots: Vec<(u32, Shot)> =
        trace_other_voxel_lasers(world, players, &bodies, own_body, &colors);
    for (id, other) in &other_shots {
        let shooter: Option<&RemotePlayer> = players.iter().find(|p| p.id == *id);
        beams.push(LaserBeam {
            segments: other.segments.clone(),
            team: shooter.map_or(0, |p| p.team),
            is_rifle: shooter.is_some_and(|p| p.is_rifle),
            speckle_frac: shooter.map_or(0.0, |p| p.beam_speckle_frac),
        });
    }
    let no_dust: &[DustCloud] = &[];
    let (beams, spot, dust) = match tracers {
        Some(tracers) => (tracers.beams, tracers.spot, tracers.dust),
        None => (beams.as_slice(), spot, no_dust),
    };
    let drawn: VoxelWorld = shared.with_posed_avatars(&own_bodies, own_poses, corpses, &colors);
    SharedVoxelView {
        frame: VoxelFrame::Build(&drawn, camera, beams, spot, dust, render_px),
        shot,
        hit_player_id,
        hit_marker,
        is_head_hit,
        other_shots,
    }
}

/// `describe_shot` turns a voxel laser shot traced in `world` (without avatars) into one line for
/// the HUD or the results.
///
/// Example: `let text = describe_shot(&world, &shot);`
pub fn describe_shot(world: &VoxelWorld, shot: &Shot) -> String {
    let what: String = match (shot.is_hit, shot.hit_structure) {
        (true, Some(k)) => world
            .structures
            .get(k)
            .unwrap_or_else(|| panic!("describe_shot: structure {k} isn't in this world (an avatar?); use describe_voxel_laser"))
            .name
            .clone(),
        (true, None) => "the ground".to_string(),
        (false, _) => "nothing".to_string(),
    };
    format_shot(shot, &what)
}

/// `format_shot` gives the HUD's line for `shot`, which hit `what`: the warps it went through,
/// what it hit and how far it went.
///
/// Example: `let text = format_shot(&shot, "the ground");`
fn format_shot(shot: &Shot, what: &str) -> String {
    let through: String = shot
        .through
        .iter()
        .map(|name| format!("through {name}, "))
        .collect();
    format!("{through}hits {what} after {:.1} m", shot.length_m)
}

/// `describe_voxel_laser` gives the HUD's line for this player's laser in a shared view: the
/// player it hit, a thrown wormhole's marker, or what else it hit in `world` (the map without
/// avatars or markers), or `None` with the laser off. A shot that hit an avatar or a marker never
/// hit the map, so the map's structures are only looked up for shots that hit the map.
///
/// Example: `let line = multiplayer::describe_voxel_laser(&world, &view);`
pub fn describe_voxel_laser(world: &VoxelWorld, view: &SharedVoxelView) -> Option<String> {
    match (view.hit_player_id, view.shot.as_ref()) {
        (Some(victim_id), _) => Some(format!("hits player {victim_id}")),
        (None, Some(shot)) if view.hit_marker.is_some() => Some(format_shot(shot, MARKER_NAME)),
        (None, Some(shot)) => Some(describe_shot(world, shot)),
        (None, None) => None,
    }
}

/// `run_voxel_bot` is `run_bot` on a voxel map: it walks toward the nearest other player (jumping
/// when a block stops it), faces them, fires when within `VOXEL_BOT_LASER_RANGE_M`, reports hits (calling `save_view` with its
/// view) and rejoins `BOT_REJOIN_S` after being hit. The black hole grows with the server's
/// consoles, or to `black_hole_strength` when given, and the bot's own time slows near it. It returns how many players it hit and how often it was hit,
/// and fails if the server curves space other than the built-in settings (`check_bot_space`).
///
/// Example: `let (hit_count, eliminated_count) = multiplayer::run_voxel_bot(&world, &mut client, 30.0, None, |frame, id| Ok(()))?;`
pub fn run_voxel_bot(
    world: &VoxelWorld,
    client: &mut Client,
    duration_s: f64,
    black_hole_strength: Option<f64>,
    objective: BotObjective,
    mut save_view: impl FnMut(&VoxelFrame, u32) -> Result<()>,
) -> Result<(u32, u32)> {
    let (mut grown_world, mut black_hole_growth) =
        (world.clone(), BlackHoleGrowth::New(black_hole_strength));
    let start: Instant = Instant::now();
    let mut player: VoxelPlayer = VoxelPlayer::AtSpawn(world, 0);
    let (mut hit_count, mut eliminated_count) = (0, 0);
    let mut rejoin_at: Option<Instant> = None;
    let mut last_victim: Option<u32> = None;
    let mut is_stuck: bool = false;
    while start.elapsed().as_secs_f64() < duration_s {
        let tick: Instant = Instant::now();
        let now_s: f64 = start.elapsed().as_secs_f64();
        black_hole_growth.update(&mut grown_world, client.game.off_mask, now_s);
        let world: &VoxelWorld = &grown_world;
        for event in client.poll()? {
            match event {
                NetEvent::Joined(id) => {
                    let others: Vec<DVec4> =
                        client.players.iter().map(|p| p.frame.col(AXIS_W)).collect();
                    let team: u32 = client.team.expect("a welcome brings the team");
                    check_bot_space(client)?;
                    player = VoxelPlayer::NearSpawn(world, team, id, &others);
                    println!("bot joined as player {id}");
                }
                NetEvent::Eliminated {
                    victim_id,
                    shooter_id,
                    is_self,
                } => {
                    println!("player {victim_id} was hit by player {shooter_id}");
                    if is_self {
                        eliminated_count += 1;
                        rejoin_at = Some(Instant::now() + Duration::from_secs_f64(BOT_REJOIN_S));
                    }
                }
            }
        }
        if let Some(time) = rejoin_at {
            if Instant::now() >= time {
                rejoin_at = None;
                client.rejoin()?;
            }
        } else if client.id.is_some() {
            let camera: DMat4 = player.get_camera_frame(world);
            let eye: DVec4 = camera.col(AXIS_W);
            let target: Option<(&RemotePlayer, f64)> = client
                .players
                .iter()
                .map(|other| (other, get_distance(eye, other.frame.col(AXIS_W)) * world.r))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let mut input: Input = Input::default();
            let mut is_laser_on: bool = false;
            if let Some((other, distance_m)) = target {
                let local: DVec3 =
                    get_local_direction(camera, normalize_tangent(eye, other.frame.col(AXIS_W)));
                player.look(
                    local.x.atan2(-local.z) / LOOK_RAD_PER_PX,
                    -local.y.clamp(-1.0, 1.0).asin() / LOOK_RAD_PER_PX,
                );
                input.forward = distance_m > VOXEL_SPAWN_RING_M * 2.0;
                is_laser_on = distance_m < VOXEL_BOT_LASER_RANGE_M;
            } else {
                input.forward = true;
                player.look(BOT_WANDER_PX_PER_S * BOT_STEP_S, 0.0);
            }
            input.up = input.forward && is_stuck;
            let before: DVec4 = player.body.col(AXIS_W);
            let team: u32 = client.team.unwrap_or(0);
            let own_step_s: f64 =
                BOT_STEP_S * world.get_time_rate_frac(player.get_camera_frame(world).col(AXIS_W));
            player.step(world, &input, own_step_s, team as usize);
            is_stuck = input.forward
                && get_distance(before, player.body.col(AXIS_W)) * world.r
                    < BOT_STUCK_FRAC * world.player.walk_m_per_s * own_step_s;
            let camera: DMat4 = player.get_camera_frame(world);
            let view: SharedVoxelView = pack_shared_voxel_view(
                world,
                camera,
                is_laser_on.then_some(camera),
                team,
                &client.players,
                &vec![IDLE_POSE; client.players.len()],
                &[],
                None,
                &[],
                None,
                BOT_VIEW_PX,
            );
            if let Some(victim_id) = view.hit_player_id {
                client.send_hit(victim_id, BOT_HITS_TO_KILL)?;
                if last_victim != Some(victim_id) {
                    hit_count += 1;
                    println!("bot hit player {victim_id}");
                    save_view(&view.frame, victim_id)?;
                }
            }
            last_victim = view.hit_player_id;
            let console: Option<u8> = match objective {
                BotObjective::Fight => None,
                BotObjective::Consoles => get_console_to_disable(world, client, team),
            };
            client.send_state(
                camera,
                is_laser_on && console.is_none(),
                false,
                console,
                0.0,
                PlayerWormholes::default(),
            )?;
        } else {
            client.keep_joining()?;
        }
        std::thread::sleep(Duration::from_secs_f64(BOT_STEP_S).saturating_sub(tick.elapsed()));
    }
    client.leave()?;
    Ok((hit_count, eliminated_count))
}

/// `get_console_to_disable` gives the first console of the other team (than `team`) that is still
/// on, for a `--bot-objective consoles` bot to hold `E` on, or `None` once the game is won.
///
/// Example: `let console = get_console_to_disable(&world, &client, 0);`
fn get_console_to_disable(world: &VoxelWorld, client: &Client, team: u32) -> Option<u8> {
    if client.game.winner.is_some() {
        return None;
    }
    (0..world.consoles.len())
        .find(|&i| world.consoles[i].team != team && client.game.off_mask >> i & 1 == 0)
        .map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    //! Tests of where players start and of lasers hitting players.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Player id | each id up to `MAX_PLAYER_COUNT` | `spawn_points_are_apart_and_clear_of_mouths` |
    //! | Laser aim | at another player, past them, strayed past them by shot noise | `a_laser_hits_the_player_it_points_at`, `a_strayed_shot_misses_alike_in_both_windows` |
    //! | What a shot hits past the players | a thrown wormhole's marker (named by its index) | `a_shot_at_a_marker_names_it` |
    //! | Map | the house, Lunar Surface (a hit on another player's body, described for the HUD) | `a_laser_hits_the_player_it_points_at`, `a_lunar_laser_hit_on_a_player_is_described` |
    //! | Body colour | players of both teams, with ids whose old per-id colours differ from their team's | `players_are_drawn_in_their_teams_colours` |
    //! | Whose laser | this player's, another player's on Lunar Surface (drawn from their gun, out of their own body; strayed as in its shooter's window) | the tests above, `other_players_lasers_leave_their_bodies`, `a_strayed_shot_misses_alike_in_both_windows` |
    //! | This player's own body | not drawn (first person), drawn (third person: others' lasers stop at it, this player's own passes it) | `an_own_body_stops_others_lasers_only` |

    use super::*;
    use crate::net::MAX_PLAYER_COUNT;
    use crate::shot_noise::ShotOffset;
    use crate::world::DEFAULT_CURVATURE_RADIUS_M;

    const LUNAR_SHOOTER_M: [f64; 2] = [-27.0, 0.0];
    const LUNAR_TARGET_M: [f64; 2] = [-20.0, 0.0];
    const MARKER_AHEAD_M: f64 = 3.0;
    const MARKER_RADIUS_M: f64 = 0.5;
    const MIN_OTHER_BEAM_M: f64 = 1.0;
    const STRAY_PAST_TARGET: ShotOffset = ShotOffset {
        angle_rad: 0.15,
        direction_rad: -std::f64::consts::FRAC_PI_4,
    };
    const SAME_SPOT_M: f64 = 1e-6;

    #[test]
    fn spawn_points_are_apart_and_clear_of_mouths() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let r: f64 = world.curvature_radius_m;
        let mut points: Vec<DVec4> = Vec::new();
        for id in 1..=MAX_PLAYER_COUNT as u32 {
            points.push(get_spawn_player(&world, id, &points).frame.col(AXIS_W));
        }
        for (i, a) in points.iter().enumerate() {
            for b in &points[i + 1..] {
                assert!(get_distance(*a, *b) * r >= MIN_SPAWN_GAP_M);
            }
            for mouth in &world.mouths {
                assert!(get_distance(*a, mouth.center) * r > world.get_jump_radius_m());
            }
        }
    }

    #[test]
    fn a_laser_hits_the_player_it_points_at() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let shooter: Player = get_spawn_player(&world, 1, &[]);
        let mut target: Player = get_spawn_player(&world, 2, &[]);
        physics::turn_toward(&mut target.frame, shooter.frame.col(AXIS_W));
        let mut aimed: DMat4 = shooter.frame;
        physics::turn_toward(&mut aimed, target.frame.col(AXIS_W));
        let other: RemotePlayer = RemotePlayer {
            id: 2,
            frame: target.frame,
            is_laser_on: false,
            is_rifle: false,
            team: 0,
            disabling_console: None,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let view: SharedView =
            pack_shared_view(&world, aimed, Some(aimed), &[other], &[], (64, 40), 0.0);
        assert_eq!(view.hit_player_id, Some(2));
        let away: DMat4 = aimed * make_rotation(AXIS_X, AXIS_Z, std::f64::consts::PI);
        let view: SharedView =
            pack_shared_view(&world, away, Some(away), &[other], &[], (64, 40), 0.0);
        assert_eq!(view.hit_player_id, None);
    }

    #[test]
    fn a_shot_at_a_marker_names_it() {
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let shooter: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_SHOOTER_M, LUNAR_TARGET_M, None, 0.0);
        let camera: DMat4 = shooter.get_camera_frame(&world);
        let ahead: DVec4 = (camera * make_boost(AXIS_Z, -MARKER_AHEAD_M / world.r)).col(AXIS_W);
        let marker: Marker = Marker {
            x: ahead,
            rgb: [1.0; 3],
            radius_m: MARKER_RADIUS_M,
            is_mouth: true,
        };
        let view: SharedVoxelView = pack_shared_voxel_view(
            &world,
            camera,
            Some(camera),
            0,
            &[],
            &[],
            &[marker],
            None,
            &[],
            None,
            (64, 40),
        );
        assert_eq!((view.hit_player_id, view.hit_marker), (None, Some(0)));
        let line: String = describe_voxel_laser(&world, &view).expect("the laser is on");
        assert!(line.contains(MARKER_NAME), "{line}");
    }

    #[test]
    fn a_lunar_laser_hit_on_a_player_is_described() {
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let shooter: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_SHOOTER_M, LUNAR_TARGET_M, None, 0.0);
        let target: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_TARGET_M, LUNAR_SHOOTER_M, None, 0.0);
        let other: RemotePlayer = RemotePlayer {
            id: 2,
            frame: target.get_camera_frame(&world),
            is_laser_on: false,
            is_rifle: false,
            team: 0,
            disabling_console: None,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let camera: DMat4 = shooter.get_camera_frame(&world);
        let view: SharedVoxelView = pack_shared_voxel_view(
            &world,
            camera,
            Some(camera),
            0,
            &[other],
            &[IDLE_POSE],
            &[],
            None,
            &[],
            None,
            (64, 40),
        );
        assert_eq!(view.hit_player_id, Some(2));
        assert_eq!(
            describe_voxel_laser(&world, &view),
            Some("hits player 2".to_string())
        );
    }

    #[test]
    fn a_strayed_shot_misses_alike_in_both_windows() {
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let shooter: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_SHOOTER_M, LUNAR_TARGET_M, None, 0.0);
        let target: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_TARGET_M, LUNAR_SHOOTER_M, None, 0.0);
        let camera: DMat4 = shooter.get_camera_frame(&world);
        let aim: DMat4 = STRAY_PAST_TARGET.aim(camera);
        let remote = |id: u32, frame: DMat4, is_laser_on: bool| RemotePlayer {
            id,
            frame,
            is_laser_on,
            is_rifle: false,
            team: 0,
            disabling_console: None,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let target_camera: DMat4 = target.get_camera_frame(&world);
        let shooters_view: SharedVoxelView = pack_shared_voxel_view(
            &world,
            camera,
            Some(aim),
            0,
            &[remote(2, target_camera, false)],
            &[IDLE_POSE],
            &[],
            None,
            &[],
            None,
            (64, 40),
        );
        assert_eq!(shooters_view.hit_player_id, None);
        let own_spot: DVec4 = shooters_view
            .shot
            .and_then(|shot| shot.spot)
            .expect("the strayed shot lands on the ground");
        let targets_view: SharedVoxelView = pack_shared_voxel_view(
            &world,
            target_camera,
            None,
            0,
            &[remote(1, aim, true)],
            &[IDLE_POSE],
            &[],
            None,
            &[],
            None,
            (64, 40),
        );
        let seen_spot: DVec4 = targets_view.other_shots[0]
            .1
            .spot
            .expect("the target's window sees the shot land");
        assert!(get_distance(own_spot, seen_spot) * world.r < SAME_SPOT_M);
    }

    #[test]
    fn other_players_lasers_leave_their_bodies() {
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let target: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_TARGET_M, LUNAR_SHOOTER_M, None, 0.0);
        let firing: RemotePlayer = RemotePlayer {
            id: 2,
            frame: target.get_camera_frame(&world),
            is_laser_on: true,
            is_rifle: false,
            team: 0,
            disabling_console: None,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let bodies: Vec<(DMat4, usize)> = get_avatar_bodies(&world, std::slice::from_ref(&firing));
        let shots = trace_other_voxel_lasers(&world, &[firing], &bodies, None, &get_team_colors());
        assert!(shots[0].1.length_m > MIN_OTHER_BEAM_M);
    }

    /// `players_are_drawn_in_their_teams_colours` checks that a player's body takes their team's
    /// colour, the minimap's, whatever their id (W-99, players in team colours).
    #[test]
    fn players_are_drawn_in_their_teams_colours() {
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let at: VoxelPlayer =
            VoxelPlayer::AtView(&world, LUNAR_TARGET_M, LUNAR_SHOOTER_M, None, 0.0);
        let players: Vec<RemotePlayer> = [(1, 1), (2, 0), (9, 1), (8, 0)]
            .map(|(id, team)| RemotePlayer {
                id,
                frame: at.get_camera_frame(&world),
                is_laser_on: false,
                is_rifle: false,
                team,
                disabling_console: None,
                disabling_frac: 0.0,
                beam_speckle_frac: 0.0,
                wormholes: PlayerWormholes::default(),
            })
            .to_vec();
        let colors: Vec<Color> = get_team_colors();
        for (player, (_, color_index)) in players.iter().zip(get_avatar_bodies(&world, &players)) {
            let [r, g, b, _] = TEAM_COLORS[player.team as usize];
            assert_eq!(colors[color_index], [r, g, b], "player {}", player.id);
        }
    }

    #[test]
    fn an_own_body_stops_others_lasers_only() {
        const OWN_TEAM: u32 = 1;
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let eye: DMat4 = VoxelPlayer::AtView(&world, LUNAR_SHOOTER_M, LUNAR_TARGET_M, None, 0.0)
            .get_camera_frame(&world);
        let firing: RemotePlayer = RemotePlayer {
            id: 2,
            frame: VoxelPlayer::AtView(&world, LUNAR_TARGET_M, LUNAR_SHOOTER_M, None, 0.0)
                .get_camera_frame(&world),
            is_laser_on: true,
            is_rifle: false,
            team: 0,
            disabling_console: None,
            disabling_frac: 0.0,
            beam_speckle_frac: 0.0,
            wormholes: PlayerWormholes::default(),
        };
        let pack = |own_body: Option<(DMat4, usize)>| -> SharedVoxelView {
            pack_shared_voxel_view(
                &world,
                eye,
                Some(eye),
                0,
                std::slice::from_ref(&firing),
                &vec![IDLE_POSE; 1 + usize::from(own_body.is_some())],
                &[],
                own_body,
                &[],
                None,
                (64, 40),
            )
        };
        let first_person: SharedVoxelView = pack(None);
        let third_person: SharedVoxelView = pack(Some(get_body(&world, eye, OWN_TEAM)));
        assert_eq!(
            third_person.frame.uniforms.structure_count,
            first_person.frame.uniforms.structure_count + 1,
            "the own body is drawn"
        );
        assert_eq!(
            third_person.hit_player_id,
            Some(2),
            "the own shot passes it"
        );
        let apart_m: f64 = get_distance(eye.col(AXIS_W), firing.frame.col(AXIS_W)) * world.r;
        assert!(third_person.other_shots[0].1.length_m < apart_m);
        assert!(first_person.other_shots[0].1.length_m > apart_m);
    }
}
