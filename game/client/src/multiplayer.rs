//! `multiplayer.rs` brings the other players into the game. Each is drawn as a box around their
//! eye, added to a copy of the world every frame, so the ray tracer bends them through wormholes
//! like everything else; everyone's laser is traced and drawn; and this player's laser reaching
//! another player is a hit. It also has the windowless bot that `--bot` runs to test a game.
//!
//! Bodies are voxel structures and lasers are traced through the map (`pack_shared_voxel_view`,
//! `run_voxel_bot`).
//!
//! Interface:
//! `SharedVoxelView`, `get_body`, `with_bodies`, `pack_shared_voxel_view`, `run_voxel_bot`.
//!
//! Example: `let world = multiplayer::with_bodies(&world, &client.players);`

use std::error::Error;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3, DVec4};

use crate::avatar_model::AvatarClip;
use crate::black_hole_growth::BlackHoleGrowth;
use crate::dust::DustCloud;
use crate::geometry::{AXIS_W, get_distance, get_local_direction, normalize_tangent};
use crate::minimap::TEAM_COLORS;
use crate::net::{Client, NetEvent, RemotePlayer};
use crate::physics::Input;
use crate::render_voxel::{LaserBeam, VoxelFrame};
use crate::settings::{ServerSettings, SpaceSettings};
use crate::thrown_wormholes::{Marker, PlayerWormholes};
use crate::voxel_world::{LOOK_RAD_PER_PX, MARKER_NAME, Shot, VoxelPlayer, VoxelWorld};

/// `Color` is a linear RGB colour.
type Color = [f32; 3];

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const IDLE_POSE: (AvatarClip, f64) = (AvatarClip::Idle, 0.0);
const BOT_STEP_S: f64 = 1.0 / 60.0;
const BOT_HITS_TO_KILL: u8 = 1;
const BOT_WANDER_PX_PER_S: f64 = 120.0;
const BOT_REJOIN_S: f64 = 2.0;
const BOT_STUCK_FRAC: f64 = 0.5;
const BOT_VIEW_PX: (u32, u32) = (1280, 800);
const VOXEL_SPAWN_RING_M: f64 = 3.0;
const VOXEL_BOT_LASER_RANGE_M: f64 = 120.0;

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
            client.send_state(
                camera,
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

#[cfg(test)]
mod tests {
    //! Tests of where players start and of lasers hitting players.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Player id | each id up to `MAX_PLAYER_COUNT` | `spawn_points_are_apart_and_clear_of_mouths` |
    //! | Laser aim | at another player, past them, strayed past them by shot noise | `a_laser_hits_the_player_it_points_at`, `a_strayed_shot_misses_alike_in_both_windows` |
    //! | What a shot hits past the players | a thrown wormhole's marker (named by its index) | `a_shot_at_a_marker_names_it` |
    //! | Map | Lunar Surface (a hit on another player's body, described for the HUD) | `a_lunar_laser_hit_on_a_player_is_described` |
    //! | Body colour | players of both teams, with ids whose old per-id colours differ from their team's | `players_are_drawn_in_their_teams_colours` |
    //! | Whose laser | this player's, another player's on Lunar Surface (drawn from their gun, out of their own body; strayed as in its shooter's window) | the tests above, `other_players_lasers_leave_their_bodies`, `a_strayed_shot_misses_alike_in_both_windows` |
    //! | This player's own body | not drawn (first person), drawn (third person: others' lasers stop at it, this player's own passes it) | `an_own_body_stops_others_lasers_only` |

    use super::*;
    use crate::geometry::{AXIS_Z, make_boost};
    use crate::shot_noise::ShotOffset;

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
