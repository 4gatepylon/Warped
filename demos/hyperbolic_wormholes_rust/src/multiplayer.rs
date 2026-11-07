//! `multiplayer.rs` brings the other players into the game. Each is drawn as a box around their
//! eye, added to a copy of the world every frame, so the ray tracer bends them through wormholes
//! like everything else; everyone's laser is traced and drawn; and this player's laser reaching
//! another player is a hit. It also has the windowless bot that `--bot` runs to test a game.
//!
//! On voxel maps the same happens with `VoxelWorld`: bodies are voxel structures and lasers are
//! traced through the map (`pack_shared_voxel_view`, `get_voxel_spawn`, `run_voxel_bot`).
//!
//! Interface: `get_spawn_player`, `SharedView`, `pack_shared_view`, `run_bot`,
//! `get_voxel_spawn`, `SharedVoxelView`, `pack_shared_voxel_view`, `run_voxel_bot`.
//!
//! Example: `let view = multiplayer::pack_shared_view(&world, frame, true, &client.players, (1280, 800), 0.0);`

use std::error::Error;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, get_local_direction, make_boost, make_rotation,
    normalize_tangent,
};
use crate::net::{Client, NetEvent, RemotePlayer};
use crate::physics::{self, Input, LaserReport, Player};
use crate::render_voxel::VoxelFrame;
use crate::scene_data::{Beam, SceneData};
use crate::voxel_world::{BeamSegment, LOOK_RAD_PER_PX, Shot, VoxelPlayer, VoxelWorld};
use crate::world::{Color, EYE_HEIGHT_M, World};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const AVATAR_HALF_M: [f64; 3] = [0.3, 0.95, 0.3];
const AVATAR_DROP_M: f64 = 0.75;
const SPAWN_RING_M: f64 = 3.0;
const SPAWN_TURN_RAD: f64 = 2.399_963_229_728_653;
const AVATAR_COLORS: [Color; 8] = [
    [0.95, 0.25, 0.2],
    [0.2, 0.55, 0.95],
    [0.95, 0.8, 0.15],
    [0.6, 0.3, 0.9],
    [0.2, 0.8, 0.45],
    [0.95, 0.5, 0.1],
    [0.9, 0.3, 0.7],
    [0.2, 0.85, 0.85],
];
const BOT_STEP_S: f64 = 1.0 / 60.0;
const BOT_RANGE_M: f64 = 12.0;
const BOT_WANDER_PX_PER_S: f64 = 120.0;
const BOT_REJOIN_S: f64 = 2.0;
const BOT_VIEW_PX: (u32, u32) = (1280, 800);
const VOXEL_SPAWN_RING_M: f64 = 3.0;
const VOXEL_BOT_LASER_RANGE_M: f64 = 120.0;
const TEAM_COUNT: u32 = 2;

/// `get_spawn_player` makes player `id`'s starting player: on a ring around spawn, each id at
/// another bearing, facing the middle.
///
/// Example: `let player = multiplayer::get_spawn_player(&world, 3);`
pub fn get_spawn_player(world: &World, id: u32) -> Player {
    let mut player: Player = Player::AtSpawn(world);
    player.frame = world.make_frame_from_spawn(f64::from(id) * SPAWN_TURN_RAD, SPAWN_RING_M)
        * make_boost(AXIS_Y, EYE_HEIGHT_M / world.curvature_radius_m)
        * make_rotation(AXIS_X, AXIS_Z, std::f64::consts::PI);
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
/// player's laser report, and the player its laser hit, if any.
pub struct SharedView {
    pub data: SceneData,
    pub report: Option<LaserReport>,
    pub hit_player_id: Option<u32>,
}

/// `pack_shared_view` packs the scene seen from `frame` with a box for each of `players`, this
/// player's laser (if on) and the lasers of the players who have theirs on, each traced through the
/// same scene, so beams stop at players and bend through wormholes.
///
/// Example: `let view = multiplayer::pack_shared_view(&world, frame, true, &players, (1280, 800), 0.0);`
pub fn pack_shared_view(
    world: &World,
    frame: DMat4,
    is_laser_on: bool,
    players: &[RemotePlayer],
    render_px: (u32, u32),
    time_s: f64,
) -> SharedView {
    let mut shared: World = world.clone();
    let first_avatar_box: usize = shared.boxes.len();
    for player in players {
        let color: Color = AVATAR_COLORS[player.id as usize % AVATAR_COLORS.len()];
        let name: String = format!("player {}", player.id);
        shared.add_avatar(
            get_avatar_frame(world, player.frame),
            &name,
            AVATAR_HALF_M,
            color,
        );
    }
    let trace_from = |eye: DMat4| {
        let data: SceneData =
            SceneData::Build(&shared, eye, &Beam::default(), render_px, time_s, true);
        physics::trace_laser(&shared, &data)
    };
    let (mut beam, mut report, mut hit_player_id) = (Beam::default(), None, None);
    if is_laser_on {
        let (own_beam, mut own_report) = trace_from(frame);
        hit_player_id = own_report
            .hit_box
            .and_then(|b| b.checked_sub(first_avatar_box))
            .map(|k| players[k].id);
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
    }
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
                    player = get_spawn_player(world, id);
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
                is_laser_on,
                &client.players,
                BOT_VIEW_PX,
                start.elapsed().as_secs_f64(),
            );
            if let Some(victim_id) = view.hit_player_id {
                client.send_hit(victim_id)?;
                if last_victim != Some(victim_id) {
                    hit_count += 1;
                    println!("bot hit player {victim_id}");
                    save_view(&view.data, victim_id)?;
                }
            }
            last_victim = view.hit_player_id;
            client.send_state(player.frame, is_laser_on)?;
        } else {
            client.keep_joining()?;
        }
        std::thread::sleep(Duration::from_secs_f64(BOT_STEP_S).saturating_sub(tick.elapsed()));
    }
    client.leave()?;
    Ok((hit_count, eliminated_count))
}

/// `get_voxel_spawn` gives player `id`'s starting player on a voxel map: on their team's spawn
/// (ids alternate between the map's spawn points), moved a few meters around it so that players of
/// a team don't start inside each other, standing where they are clear of blocks.
///
/// Example: `let player = multiplayer::get_voxel_spawn(&world, 3);`
pub fn get_voxel_spawn(world: &VoxelWorld, id: u32) -> VoxelPlayer {
    let spawn: VoxelPlayer = VoxelPlayer::AtSpawn(world, id as usize);
    let turn: DMat4 = make_rotation(AXIS_X, AXIS_Z, f64::from(id) * SPAWN_TURN_RAD);
    let body: DMat4 =
        spawn.body * turn * make_boost(AXIS_Z, -VOXEL_SPAWN_RING_M / world.r) * turn.inverse();
    VoxelPlayer::Standing(world, body)
}

/// `get_team` gives a player's team for the beam colour: 0 (blue) or 1 (red), as their spawn.
///
/// Example: `let team = get_team(3);`
fn get_team(id: u32) -> u32 {
    id % TEAM_COUNT
}

/// `SharedVoxelView` is one frame of a voxel map with the other players, every laser, this
/// player's laser shot and the player it hit, if any.
pub struct SharedVoxelView {
    pub frame: VoxelFrame,
    pub shot: Option<Shot>,
    pub hit_player_id: Option<u32>,
}

/// `trace_other_voxel_lasers` traces the laser of each of `players` who has theirs on (their
/// `bodies` given), through `world` with every other player's body but not their own (their gun
/// is inside it, so their beam would stop where it starts), giving each shot and team.
///
/// Example: `let shots = trace_other_voxel_lasers(&world, &players, &bodies, &colors);`
fn trace_other_voxel_lasers(
    world: &VoxelWorld,
    players: &[RemotePlayer],
    bodies: &[(DMat4, usize)],
    colors: &[[f32; 3]],
) -> Vec<(Shot, u32)> {
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
                .collect();
            (
                world
                    .with_avatars(&others, colors)
                    .trace_laser(player.frame),
                get_team(player.id),
            )
        })
        .collect()
}

/// `pack_shared_voxel_view` packs a voxel map seen from `camera` with a body for each of
/// `players`, this player's laser (if on, for team `team`) and the lasers of the players who have
/// theirs on, all traced through the same map, so beams stop at players and bend like light.
///
/// Example: `let view = multiplayer::pack_shared_voxel_view(&world, camera, true, 1, &players, (1280, 800));`
pub fn pack_shared_voxel_view(
    world: &VoxelWorld,
    camera: DMat4,
    is_laser_on: bool,
    team: u32,
    players: &[RemotePlayer],
    render_px: (u32, u32),
) -> SharedVoxelView {
    let colors: Vec<[f32; 3]> = AVATAR_COLORS.to_vec();
    let bodies: Vec<(DMat4, usize)> = players
        .iter()
        .map(|p| {
            (
                VoxelPlayer::FromEye(world, p.frame).get_feet_frame(world),
                p.id as usize,
            )
        })
        .collect();
    let shared: VoxelWorld = world.with_avatars(&bodies, &colors);
    let mut beams: Vec<(Vec<BeamSegment>, u32)> = Vec::new();
    let (mut shot, mut hit_player_id, mut spot) = (None, None, None);
    if is_laser_on {
        let own: Shot = shared.trace_laser(camera);
        hit_player_id = own
            .hit_structure
            .and_then(|k| k.checked_sub(shared.static_structure_count))
            .map(|k| players[k].id);
        spot = own.spot.map(|x| (x, team));
        beams.push((own.segments.clone(), team));
        shot = Some(own);
    }
    for (other, team) in trace_other_voxel_lasers(world, players, &bodies, &colors) {
        beams.push((other.segments, team));
    }
    SharedVoxelView {
        frame: VoxelFrame::Build(&shared, camera, &beams, spot, render_px),
        shot,
        hit_player_id,
    }
}

/// `describe_shot` turns a voxel laser shot traced in `world` (without avatars) into one line for
/// the HUD or the results.
///
/// Example: `let text = describe_shot(&world, &shot);`
pub fn describe_shot(world: &VoxelWorld, shot: &Shot) -> String {
    let through: String = shot
        .through
        .iter()
        .map(|name| format!("through {name}, "))
        .collect();
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
    format!("{through}hits {what} after {:.1} m", shot.length_m)
}

/// `describe_voxel_laser` gives the HUD's line for this player's laser in a shared view: the
/// player it hit, or what else it hit in `world` (the map without avatars), or `None` with the
/// laser off. A shot that hit an avatar is always a hit on a player, so the map's structures are
/// only looked up for shots that hit the map.
///
/// Example: `let line = multiplayer::describe_voxel_laser(&world, &view);`
pub fn describe_voxel_laser(world: &VoxelWorld, view: &SharedVoxelView) -> Option<String> {
    match (view.hit_player_id, view.shot.as_ref()) {
        (Some(victim_id), _) => Some(format!("hits player {victim_id}")),
        (None, Some(shot)) => Some(describe_shot(world, shot)),
        (None, None) => None,
    }
}

/// `run_voxel_bot` is `run_bot` on a voxel map: it walks toward the nearest other player, faces
/// them, fires when within `VOXEL_BOT_LASER_RANGE_M`, reports hits (calling `save_view` with its
/// view) and rejoins `BOT_REJOIN_S` after being hit. It returns how many players it hit and how
/// often it was hit.
///
/// Example: `let (hit_count, eliminated_count) = multiplayer::run_voxel_bot(&world, &mut client, 30.0, |frame, id| Ok(()))?;`
pub fn run_voxel_bot(
    world: &VoxelWorld,
    client: &mut Client,
    duration_s: f64,
    mut save_view: impl FnMut(&VoxelFrame, u32) -> Result<()>,
) -> Result<(u32, u32)> {
    let start: Instant = Instant::now();
    let mut player: VoxelPlayer = VoxelPlayer::AtSpawn(world, 0);
    let (mut hit_count, mut eliminated_count, mut my_id) = (0, 0, 0);
    let mut rejoin_at: Option<Instant> = None;
    let mut last_victim: Option<u32> = None;
    while start.elapsed().as_secs_f64() < duration_s {
        let tick: Instant = Instant::now();
        for event in client.poll()? {
            match event {
                NetEvent::Joined(id) => {
                    (player, my_id) = (get_voxel_spawn(world, id), id);
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
            player.step(world, &input, BOT_STEP_S, my_id as usize);
            let camera: DMat4 = player.get_camera_frame(world);
            let view: SharedVoxelView = pack_shared_voxel_view(
                world,
                camera,
                is_laser_on,
                get_team(my_id),
                &client.players,
                BOT_VIEW_PX,
            );
            if let Some(victim_id) = view.hit_player_id {
                client.send_hit(victim_id)?;
                if last_victim != Some(victim_id) {
                    hit_count += 1;
                    println!("bot hit player {victim_id}");
                    save_view(&view.frame, victim_id)?;
                }
            }
            last_victim = view.hit_player_id;
            client.send_state(camera, is_laser_on)?;
        } else {
            client.keep_joining()?;
        }
        std::thread::sleep(Duration::from_secs_f64(BOT_STEP_S).saturating_sub(tick.elapsed()));
    }
    client.leave()?;
    Ok((hit_count, eliminated_count))
}

#[cfg(test)]
mod tests {
    //! Tests of where players start and of lasers hitting players.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Player id | each id up to `MAX_PLAYER_COUNT` | `spawn_points_are_apart_and_clear_of_mouths` |
    //! | Laser aim | at another player, past them | `a_laser_hits_the_player_it_points_at` |
    //! | Map | the house, Lunar Surface (a hit on another player's body, described for the HUD) | `a_laser_hits_the_player_it_points_at`, `a_lunar_laser_hit_on_a_player_is_described` |
    //! | Whose laser | this player's, another player's on Lunar Surface (drawn from their gun, out of their own body) | the tests above, `other_players_lasers_leave_their_bodies` |

    use super::*;
    use crate::net::MAX_PLAYER_COUNT;
    use crate::world::DEFAULT_CURVATURE_RADIUS_M;

    const MIN_SPAWN_GAP_M: f64 = 1.0;
    const LUNAR_SHOOTER_M: [f64; 2] = [-27.0, 0.0];
    const LUNAR_TARGET_M: [f64; 2] = [-20.0, 0.0];
    const MIN_OTHER_BEAM_M: f64 = 1.0;

    #[test]
    fn spawn_points_are_apart_and_clear_of_mouths() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let r: f64 = world.curvature_radius_m;
        let points: Vec<DVec4> = (1..=MAX_PLAYER_COUNT as u32)
            .map(|id| get_spawn_player(&world, id).frame.col(AXIS_W))
            .collect();
        for (i, a) in points.iter().enumerate() {
            for b in &points[i + 1..] {
                assert!(get_distance(*a, *b) * r > MIN_SPAWN_GAP_M);
            }
            for mouth in &world.mouths {
                assert!(get_distance(*a, mouth.center) * r > world.get_jump_radius_m());
            }
        }
    }

    #[test]
    fn a_laser_hits_the_player_it_points_at() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let shooter: Player = get_spawn_player(&world, 1);
        let mut target: Player = get_spawn_player(&world, 2);
        physics::turn_toward(&mut target.frame, shooter.frame.col(AXIS_W));
        let mut aimed: DMat4 = shooter.frame;
        physics::turn_toward(&mut aimed, target.frame.col(AXIS_W));
        let other: RemotePlayer = RemotePlayer {
            id: 2,
            frame: target.frame,
            is_laser_on: false,
        };
        let view: SharedView = pack_shared_view(&world, aimed, true, &[other], (64, 40), 0.0);
        assert_eq!(view.hit_player_id, Some(2));
        let away: DMat4 = aimed * make_rotation(AXIS_X, AXIS_Z, std::f64::consts::PI);
        let view: SharedView = pack_shared_view(&world, away, true, &[other], (64, 40), 0.0);
        assert_eq!(view.hit_player_id, None);
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
        };
        let camera: DMat4 = shooter.get_camera_frame(&world);
        let view: SharedVoxelView =
            pack_shared_voxel_view(&world, camera, true, 0, &[other], (64, 40));
        assert_eq!(view.hit_player_id, Some(2));
        assert_eq!(
            describe_voxel_laser(&world, &view),
            Some("hits player 2".to_string())
        );
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
        };
        let bodies: Vec<(DMat4, usize)> = vec![(
            VoxelPlayer::FromEye(&world, firing.frame).get_feet_frame(&world),
            2,
        )];
        let shots = trace_other_voxel_lasers(&world, &[firing], &bodies, &AVATAR_COLORS);
        assert!(shots[0].0.length_m > MIN_OTHER_BEAM_M);
    }
}
