//! `console_bot.rs` walks a `--bot-objective consoles` bot to the other team's consoles on foot and
//! turns them off one by one, as a player does: it moves with the level's own movement
//! (`VoxelPlayer::step`, called by the bot loop), steers around wormhole mouths, black holes and
//! the level's other dangers, jumps and then sidesteps when a block stops it, and holds `E` only
//! once its view meets the console within reach (`ConsolePlace::is_held_from`, the server's own
//! test). A bot win so shows that every pylon can be reached on foot (W-126).
//!
//! Interface: `ConsoleWalker` (`New`, `step`), `WalkStep`, `Danger`, `find_warp_dangers`.
//!
//! Example: `let walk = walker.step(&world, &mut player, client.game.off_mask, team, 3.0, now_s);`
//!
//! Must know:
//! - Limitations: it steers by sight of its goal, without a map of the level's blocks, so a pylon
//!   behind a long wall may take it several sidesteps; it gives a console up after
//!   `GIVE_UP_S` and says where it stood.

use glam::{DMat4, DVec3, DVec4};

use crate::console_reach::ConsolePlace;
use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Z, get_distance, get_local_direction, make_rotation, normalize_tangent,
};
use crate::physics::Input;
use crate::terrain::get_fermi_coordinates;
use crate::voxel_world::{LOOK_RAD_PER_PX, VoxelPlayer, VoxelWorld};

const MOUTH_DANGER_M: f64 = 6.0;
const BLACK_HOLE_DANGER_M: f64 = 5.0;
const LOOKAHEAD_M: f64 = 10.0;
const SPRINT_FROM_M: f64 = 12.0;
const MIN_SPRINT_ENERGY_FRAC: f64 = 0.2;
const STUCK_MOVED_FRAC: f64 = 0.3;
const JUMP_AFTER_STUCK_S: f64 = 0.25;
const SIDESTEP_AFTER_STUCK_S: f64 = 1.0;
const FIRST_SIDESTEP_S: f64 = 1.0;
const MAX_SIDESTEP_S: f64 = 16.0;
const SIDESTEP_GROWTH: f64 = 2.0;
const SIDESTEP_RAD: f64 = std::f64::consts::FRAC_PI_2;
const PROBE_M: f64 = 2.5;
const PROGRESS_M: f64 = 3.0;
const GIVE_UP_S: f64 = 120.0;

/// `Danger` is a place a walking bot keeps out of: within `radius_m` of `point`.
#[derive(Clone, Copy, Debug)]
pub struct Danger {
    pub point: DVec4,
    pub radius_m: f64,
}

/// `WalkStep` is what a walking bot does this step: the keys it holds and the console it holds
/// `E` on, if any.
#[derive(Clone, Copy, Default)]
pub struct WalkStep {
    pub input: Input,
    pub console: Option<u8>,
}

/// `ConsoleWalker` is a walking bot's plan: every console's place, the dangers to keep out of,
/// the console it walks to, since when and whether it reached it, the consoles it gave up, how
/// long it has been stuck, its sidestep (until when, to which side, how long the next one lasts
/// and how many so far), and the closest it has come to its console since it last got closer.
pub struct ConsoleWalker {
    places: Vec<ConsolePlace>,
    dangers: Vec<Danger>,
    target: Option<(usize, f64)>,
    is_target_reached: bool,
    sidestep_count: u32,
    given_up: Vec<usize>,
    stuck_s: f64,
    sidestep_until_s: f64,
    sidestep_sign: f64,
    sidestep_s: f64,
    closest_m: f64,
    last_point: Option<DVec4>,
}

impl ConsoleWalker {
    /// `New` makes a walker for a level with consoles at `places` and `dangers` to keep out of.
    ///
    /// Example: `let walker = ConsoleWalker::New(places, find_warp_dangers(&world));`
    #[allow(non_snake_case)]
    pub fn New(places: Vec<ConsolePlace>, dangers: Vec<Danger>) -> ConsoleWalker {
        ConsoleWalker {
            places,
            dangers,
            target: None,
            is_target_reached: false,
            sidestep_count: 0,
            given_up: Vec::new(),
            stuck_s: 0.0,
            sidestep_until_s: 0.0,
            sidestep_sign: 1.0,
            sidestep_s: FIRST_SIDESTEP_S,
            closest_m: f64::MAX,
            last_point: None,
        }
    }

    /// `step` turns `player` (of `team`) and gives this step's keys and console at time `now_s`
    /// (seconds, `dt_s` after the last step): toward the nearest of the other team's consoles still
    /// on (bit i of `off_mask` is console i), holding `E` once its view meets it within `reach_m`.
    ///
    /// Example: `let walk = walker.step(&world, &mut player, off_mask, 0, 3.0, now_s, 1.0 / 60.0);`
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        world: &VoxelWorld,
        player: &mut VoxelPlayer,
        off_mask: u32,
        team: u32,
        reach_m: f64,
        now_s: f64,
        dt_s: f64,
    ) -> WalkStep {
        let body_point: DVec4 = player.body.col(AXIS_W);
        let is_on = |i: usize| self.places[i].team != team && off_mask >> i & 1 == 0;
        if self.target.is_none_or(|(i, _)| !is_on(i)) {
            self.target = (0..self.places.len())
                .filter(|&i| is_on(i) && !self.given_up.contains(&i))
                .min_by(|&a, &b| {
                    let distance = |i: usize| get_distance(body_point, self.places[i].get_foot());
                    distance(a).total_cmp(&distance(b))
                })
                .map(|i| (i, now_s));
            (self.is_target_reached, self.sidestep_count) = (false, 0);
            (self.sidestep_s, self.closest_m) = (FIRST_SIDESTEP_S, f64::MAX);
            if let Some((i, _)) = self.target {
                let distance_m: f64 = get_distance(body_point, self.places[i].get_foot()) * world.r;
                println!("bot walks to console {i}, {distance_m:.1} m away");
            }
        }
        let Some((index, since_s)) = self.target else {
            return WalkStep::default();
        };
        let place: ConsolePlace = self.places[index];
        let camera: DMat4 = player.get_camera_frame(world);
        aim_pitch(player, camera, place.get_aim_point(camera.col(AXIS_W)));
        if place.is_held_from(player.get_camera_frame(world), reach_m) {
            if !self.is_target_reached {
                self.is_target_reached = true;
                println!(
                    "bot reached console {index} on foot in {:.1} s, after {} sidesteps",
                    now_s - since_s,
                    self.sidestep_count
                );
            }
            turn_toward(player, place.get_foot());
            self.stuck_s = 0.0;
            self.last_point = None;
            return WalkStep {
                input: Input::default(),
                console: Some(index as u8),
            };
        }
        if now_s - since_s > GIVE_UP_S {
            let [u_m, v_m, h_m] = get_fermi_coordinates(body_point, world.r);
            let distance_m: f64 = get_distance(body_point, place.get_foot()) * world.r;
            println!(
                "bot gives up console {index}: stuck at u {u_m:.1} m, v {v_m:.1} m, h {h_m:.1} m, {distance_m:.1} m from it"
            );
            self.given_up.push(index);
            self.target = None;
            return WalkStep::default();
        }
        self.update_stuck(world, body_point, player.is_sprinting, dt_s, now_s);
        let goal_m: f64 = get_distance(body_point, place.get_foot()) * world.r;
        if goal_m < self.closest_m - PROGRESS_M {
            (self.sidestep_s, self.closest_m) = (FIRST_SIDESTEP_S, goal_m);
        }
        let mut heading_rad: f64 = self.choose_heading_rad(world.r, player.body, place.get_foot());
        let toward_goal: DMat4 = player.body * make_rotation(AXIS_X, AXIS_Z, heading_rad);
        if now_s < self.sidestep_until_s && world.is_way_clear(toward_goal, player.h_m, PROBE_M) {
            self.sidestep_until_s = now_s;
        }
        if now_s < self.sidestep_until_s {
            heading_rad += self.sidestep_sign * SIDESTEP_RAD;
        }
        player.look(heading_rad / LOOK_RAD_PER_PX, 0.0);
        let input: Input = Input {
            forward: true,
            up: self.stuck_s > JUMP_AFTER_STUCK_S,
            sprint: goal_m > SPRINT_FROM_M && player.energy_frac > MIN_SPRINT_ENERGY_FRAC,
            ..Input::default()
        };
        WalkStep {
            input,
            console: None,
        }
    }

    /// `update_stuck` counts how long the bot has moved less than `STUCK_MOVED_FRAC` of its speed,
    /// and starts a sidestep once that passes `SIDESTEP_AFTER_STUCK_S`: to alternating sides, each
    /// `SIDESTEP_GROWTH` times as long as the last (up to `MAX_SIDESTEP_S`), so the bot searches ever
    /// further along a wall until its way toward the console is clear (`step` ends it there).
    ///
    /// Example: `self.update_stuck(world, body_point, false, 1.0 / 60.0, now_s);`
    fn update_stuck(
        &mut self,
        world: &VoxelWorld,
        body_point: DVec4,
        is_sprinting: bool,
        dt_s: f64,
        now_s: f64,
    ) {
        let speed_m_per_s: f64 = match is_sprinting {
            true => world.player.sprint_m_per_s,
            false => world.player.walk_m_per_s,
        };
        let moved_m: f64 = self
            .last_point
            .map_or(speed_m_per_s * dt_s, |last| get_distance(last, body_point) * world.r);
        self.last_point = Some(body_point);
        self.stuck_s = match moved_m < STUCK_MOVED_FRAC * speed_m_per_s * dt_s {
            true => self.stuck_s + dt_s,
            false => 0.0,
        };
        if self.stuck_s > SIDESTEP_AFTER_STUCK_S && now_s >= self.sidestep_until_s {
            self.sidestep_until_s = now_s + self.sidestep_s;
            self.sidestep_sign = -self.sidestep_sign;
            self.sidestep_s = (self.sidestep_s * SIDESTEP_GROWTH).min(MAX_SIDESTEP_S);
            self.sidestep_count += 1;
            self.stuck_s = 0.0;
        }
    }

    /// `choose_heading_rad` gives the turn (radians, positive to the right) from the way `body`
    /// faces toward `goal`, on a level of curvature radius `r_m`, bent around each danger the straight
    /// way would come within its radius of (to the side nearer the goal's way), or straight away from
    /// a danger the bot is inside.
    ///
    /// Example: `let turn_rad = self.choose_heading_rad(world.r, player.body, foot);`
    fn choose_heading_rad(&self, r_m: f64, body: DMat4, goal: DVec4) -> f64 {
        let goal_rad: f64 = get_heading_rad(body, goal);
        let goal_m: f64 = get_distance(body.col(AXIS_W), goal) * r_m;
        let mut heading_rad: f64 = goal_rad;
        for danger in &self.dangers {
            let distance_m: f64 = get_distance(body.col(AXIS_W), danger.point) * r_m;
            if distance_m > danger.radius_m + LOOKAHEAD_M || distance_m > goal_m + danger.radius_m {
                continue;
            }
            let danger_rad: f64 = get_heading_rad(body, danger.point);
            if distance_m <= danger.radius_m {
                return wrap_rad(danger_rad + std::f64::consts::PI);
            }
            let half_rad: f64 = (danger.radius_m / distance_m).asin();
            let off_rad: f64 = wrap_rad(heading_rad - danger_rad);
            if off_rad.abs() < half_rad {
                heading_rad = wrap_rad(danger_rad + half_rad.copysign(off_rad));
            }
        }
        heading_rad
    }
}

/// `get_heading_rad` gives the turn (radians, positive to the right) from the way `body` faces to
/// the way toward `point`, along the ground.
///
/// Example: `let turn_rad = get_heading_rad(player.body, foot);`
fn get_heading_rad(body: DMat4, point: DVec4) -> f64 {
    let local: DVec3 = get_local_direction(body, normalize_tangent(body.col(AXIS_W), point));
    local.x.atan2(-local.z)
}

/// `wrap_rad` brings an angle into (-π, π].
///
/// Example: `assert_eq!(wrap_rad(4.0), 4.0 - std::f64::consts::TAU);`
fn wrap_rad(angle_rad: f64) -> f64 {
    let wrapped: f64 = angle_rad.rem_euclid(std::f64::consts::TAU);
    match wrapped > std::f64::consts::PI {
        true => wrapped - std::f64::consts::TAU,
        false => wrapped,
    }
}

/// `turn_toward` turns `player`'s body to face `point` along the ground.
///
/// Example: `turn_toward(&mut player, foot);`
fn turn_toward(player: &mut VoxelPlayer, point: DVec4) {
    player.look(get_heading_rad(player.body, point) / LOOK_RAD_PER_PX, 0.0);
}

/// `aim_pitch` tilts `player`'s view, whose camera frame is `camera`, up or down to `point`.
///
/// Example: `aim_pitch(&mut player, camera, aim_point);`
fn aim_pitch(player: &mut VoxelPlayer, camera: DMat4, point: DVec4) {
    let local: DVec3 = get_local_direction(camera, normalize_tangent(camera.col(AXIS_W), point));
    player.look(0.0, -local.normalize().y.clamp(-1.0, 1.0).asin() / LOOK_RAD_PER_PX);
}

/// `find_warp_dangers` gives each of `world`'s wormhole mouths and black holes as a danger: a
/// mouth's throat moves a player elsewhere and a black hole's horizon sends them back to their
/// spawn.
///
/// Example: `let dangers = find_warp_dangers(&world);`
pub fn find_warp_dangers(world: &VoxelWorld) -> Vec<Danger> {
    world
        .warps
        .iter()
        .map(|warp| Danger {
            point: warp.centre,
            radius_m: match warp.is_mouth {
                true => MOUTH_DANGER_M,
                false => BLACK_HOLE_DANGER_M,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Tests of the walking bot's steering.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Angle to wrap | inside (-π, π], above it, below it | `angles_wrap_into_one_turn` |
    //! | A danger on the way | none, beside the way (kept), across the way (bent around), around the bot (away) | `the_way_bends_around_dangers` |

    use super::*;
    use crate::geometry::{AXIS_X, AXIS_Z, make_boost};

    const R_M: f64 = 40.0;
    const GOAL_M: f64 = 30.0;
    const ANGLE_TOLERANCE_RAD: f64 = 1e-9;

    /// `point_at` gives the ground point `along_m` ahead (-z) and `right_m` to the right (+x) of
    /// the origin.
    fn point_at(along_m: f64, right_m: f64) -> DVec4 {
        (make_boost(AXIS_Z, -along_m / R_M) * make_boost(AXIS_X, right_m / R_M)).col(AXIS_W)
    }

    /// `make_walker` gives a walker with no consoles and `dangers`.
    fn make_walker(dangers: Vec<Danger>) -> ConsoleWalker {
        ConsoleWalker::New(Vec::new(), dangers)
    }

    #[test]
    fn angles_wrap_into_one_turn() {
        assert!((wrap_rad(1.0) - 1.0).abs() < ANGLE_TOLERANCE_RAD);
        assert!((wrap_rad(4.0) - (4.0 - std::f64::consts::TAU)).abs() < ANGLE_TOLERANCE_RAD);
        assert!((wrap_rad(-4.0) - (std::f64::consts::TAU - 4.0)).abs() < ANGLE_TOLERANCE_RAD);
    }

    #[test]
    fn the_way_bends_around_dangers() {
        let goal: DVec4 = point_at(GOAL_M, 0.0);
        let heading = |dangers: Vec<Danger>| {
            make_walker(dangers).choose_heading_rad(R_M, DMat4::IDENTITY, goal)
        };
        assert!(heading(Vec::new()).abs() < ANGLE_TOLERANCE_RAD);
        let beside: Danger = Danger {
            point: point_at(10.0, 8.0),
            radius_m: 5.0,
        };
        assert!(heading(vec![beside]).abs() < ANGLE_TOLERANCE_RAD);
        let across: Danger = Danger {
            point: point_at(10.0, 1.0),
            radius_m: 5.0,
        };
        let bent_rad: f64 = heading(vec![across]);
        assert!(bent_rad < 0.0, "a danger a little to the right sends the bot left");
        let around: Danger = Danger {
            point: point_at(-1.0, 0.0),
            radius_m: 5.0,
        };
        assert!(heading(vec![around]).abs() < ANGLE_TOLERANCE_RAD);
    }
}
