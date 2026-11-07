//! `multiplayer.rs` brings the other players into the game. Each is drawn as a box around their
//! eye, added to a copy of the world every frame, so the ray tracer bends them through wormholes
//! like everything else; everyone's laser is traced and drawn; and this player's laser reaching
//! another player is a hit. It also has the windowless bot that `--bot` runs to test a game.
//!
//! Interface: `get_spawn_player`, `SharedView`, `pack_shared_view`, `run_bot`.
//!
//! Example: `let view = multiplayer::pack_shared_view(&world, frame, true, &client.players, (1280, 800), 0.0);`

use std::error::Error;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, make_boost, make_rotation};
use crate::net::{Client, NetEvent, RemotePlayer};
use crate::physics::{self, Input, LaserReport, Player};
use crate::scene_data::{Beam, SceneData};
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
        shared.add_avatar(get_avatar_frame(world, player.frame), AVATAR_HALF_M, color);
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

#[cfg(test)]
mod tests {
    //! Tests of where players start and of lasers hitting players.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Player id | each id up to `MAX_PLAYER_COUNT` | `spawn_points_are_apart_and_clear_of_mouths` |
    //! | Laser aim | at another player, past them | `a_laser_hits_the_player_it_points_at` |

    use super::*;
    use crate::net::MAX_PLAYER_COUNT;
    use crate::world::DEFAULT_CURVATURE_RADIUS_M;

    const MIN_SPAWN_GAP_M: f64 = 1.0;

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
}
