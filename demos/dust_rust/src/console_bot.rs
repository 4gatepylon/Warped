//! `console_bot.rs` walks bots on foot as a player does (`Navigator`): with the level's own
//! movement (`VoxelPlayer::step`, called by the bot loop), around wormhole mouths, black holes and
//! the level's other dangers, jumping and then sidestepping when a block stops it. A
//! `--bot-objective consoles` bot (`ConsoleWalker`) walks to the other team's consoles and holds `E`
//! on each only once its view meets it within reach (`ConsolePlace::is_held_from`, the server's own
//! test), so a bot win shows that every pylon can be reached on foot (W-126). A fighting bot
//! (`Fighter`) walks to the nearest enemy until it sees them, then faces them and fires.
//!
//! Interface: `Navigator`, `ConsoleWalker`, `Fighter` (each `New`, `step` or `walk_toward`, and
//! `restart` after a respawn),
//! `WalkStep`, `FightStep`, `Danger`, `find_warp_dangers`.
//!
//! Example: `let walk = walker.step(&world, &mut player, off_mask, team, 3.0, now_s, 1.0 / 60.0);`
//!
//! Must know:
//! - Limitations: it steers by sight of its goal, without a map of the level's blocks: along a wall
//!   it sidesteps ever further, to alternating sides, until its way toward the console is clear,
//!   so a long wall costs it time; it gives a console up after `GIVE_UP_S` and says where it stood.

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
const FIRE_RANGE_M: f64 = 80.0;
const SIGHT_SLACK_M: f64 = 1.0;
const CLOSE_M: f64 = 10.0;
const LINE_SAMPLE_M: f64 = 1.0;
const SCAN_TURNS_PER_S: f64 = 1.5;
const SCAN_GROWTH_RAD_PER_S: f64 = 0.03;
const MAX_SCAN_RAD: f64 = 0.12;

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

/// `FightStep` is what a fighting bot does this step: the keys it holds and whether its laser is on.
#[derive(Clone, Copy, Default)]
pub struct FightStep {
    pub input: Input,
    pub is_laser_on: bool,
}

/// `Navigator` walks a bot on foot toward one goal after another: the dangers it keeps out of, how
/// long it has been stuck, its sidestep (until when, to which side, how long the next one lasts and
/// how many so far), the closest it has come to its goal since it last got closer, and where its
/// body was last step.
pub struct Navigator {
    dangers: Vec<Danger>,
    stuck_s: f64,
    sidestep_until_s: f64,
    sidestep_sign: f64,
    sidestep_s: f64,
    sidestep_count: u32,
    closest_m: f64,
    last_point: Option<DVec4>,
}

impl Navigator {
    /// `New` makes a navigator that keeps out of `dangers`.
    ///
    /// Example: `let navigator = Navigator::New(find_warp_dangers(&world));`
    #[allow(non_snake_case)]
    pub fn New(dangers: Vec<Danger>) -> Navigator {
        Navigator {
            dangers,
            stuck_s: 0.0,
            sidestep_until_s: 0.0,
            sidestep_sign: 1.0,
            sidestep_s: FIRST_SIDESTEP_S,
            sidestep_count: 0,
            closest_m: f64::MAX,
            last_point: None,
        }
    }

    /// `start_goal` forgets the last goal's sidesteps and progress, for a new goal.
    ///
    /// Example: `navigator.start_goal();`
    pub fn start_goal(&mut self) {
        (self.sidestep_s, self.sidestep_count, self.closest_m) = (FIRST_SIDESTEP_S, 0, f64::MAX);
    }

    /// `stand` tells the navigator the bot stood still this step, so standing doesn't count as
    /// stuck.
    ///
    /// Example: `navigator.stand();`
    pub fn stand(&mut self) {
        (self.stuck_s, self.last_point) = (0.0, None);
    }

    /// `walk_toward` turns `player` to walk toward `goal` this step at time `now_s` (`dt_s` after
    /// the last) and gives the keys: around the dangers, sprinting while more than `SPRINT_FROM_M`
    /// away, jumping once stuck, and sidestepping along a wall (`update_stuck`) until the way
    /// toward the goal is clear for `PROBE_M`.
    ///
    /// Example: `let input = navigator.walk_toward(&world, &mut player, foot, now_s, 1.0 / 60.0);`
    pub fn walk_toward(
        &mut self,
        world: &VoxelWorld,
        player: &mut VoxelPlayer,
        goal: DVec4,
        now_s: f64,
        dt_s: f64,
    ) -> Input {
        let body_point: DVec4 = player.body.col(AXIS_W);
        self.update_stuck(world, body_point, player.is_sprinting, dt_s, now_s);
        let goal_m: f64 = get_distance(body_point, goal) * world.r;
        if goal_m < self.closest_m - PROGRESS_M {
            (self.sidestep_s, self.closest_m) = (FIRST_SIDESTEP_S, goal_m);
        }
        let mut heading_rad: f64 = self.choose_heading_rad(world.r, player.body, goal);
        let toward_goal: DMat4 = player.body * make_rotation(AXIS_X, AXIS_Z, heading_rad);
        if now_s < self.sidestep_until_s && world.is_way_clear(toward_goal, player.h_m, PROBE_M) {
            self.sidestep_until_s = now_s;
        }
        if now_s < self.sidestep_until_s {
            heading_rad += self.sidestep_sign * SIDESTEP_RAD;
        }
        player.look(heading_rad / LOOK_RAD_PER_PX, 0.0);
        Input {
            forward: true,
            up: self.stuck_s > JUMP_AFTER_STUCK_S,
            sprint: goal_m > SPRINT_FROM_M && player.energy_frac > MIN_SPRINT_ENERGY_FRAC,
            ..Input::default()
        }
    }

    /// `update_stuck` counts how long the bot has moved less than `STUCK_MOVED_FRAC` of its speed,
    /// and starts a sidestep once that passes `SIDESTEP_AFTER_STUCK_S`: to alternating sides, each
    /// `SIDESTEP_GROWTH` times as long as the last (up to `MAX_SIDESTEP_S`), so the bot searches ever
    /// further along a wall until its way toward the goal is clear (`walk_toward` ends it there).
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
        let moved_m: f64 = self.last_point.map_or(speed_m_per_s * dt_s, |last| {
            get_distance(last, body_point) * world.r
        });
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

    /// `crosses_danger` says whether the straight line from `eye` to `target`, on a level of
    /// curvature radius `r_m`, comes within a danger's radius (a shot through a blizzard or a warp
    /// doesn't reach, and walking it leads in).
    ///
    /// Example: `if !navigator.crosses_danger(world.r, eye, enemy) { ... }`
    pub fn crosses_danger(&self, r_m: f64, eye: DVec4, target: DVec4) -> bool {
        let length_m: f64 = get_distance(eye, target) * r_m;
        let toward: DVec4 = normalize_tangent(eye, target);
        let sample_count: usize = (length_m / LINE_SAMPLE_M).ceil() as usize;
        (0..=sample_count).any(|i| {
            let s: f64 = length_m * i as f64 / sample_count.max(1) as f64 / r_m;
            let point: DVec4 = s.cosh() * eye + s.sinh() * toward;
            self.dangers
                .iter()
                .any(|danger| get_distance(point, danger.point) * r_m < danger.radius_m)
        })
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

/// `ConsoleWalker` is a walking bot's plan for the consoles: every console's place, its navigator,
/// the console it walks to, since when and whether it reached it, and the consoles it gave up.
pub struct ConsoleWalker {
    places: Vec<ConsolePlace>,
    navigator: Navigator,
    target: Option<(usize, f64)>,
    is_target_reached: bool,
    given_up: Vec<usize>,
}

impl ConsoleWalker {
    /// `New` makes a walker for a level with consoles at `places` and `dangers` to keep out of.
    ///
    /// Example: `let walker = ConsoleWalker::New(places, find_warp_dangers(&world));`
    #[allow(non_snake_case)]
    pub fn New(places: Vec<ConsolePlace>, dangers: Vec<Danger>) -> ConsoleWalker {
        ConsoleWalker {
            places,
            navigator: Navigator::New(dangers),
            target: None,
            is_target_reached: false,
            given_up: Vec::new(),
        }
    }

    /// `restart` forgets the console the walker was at, for a bot that has just come back at its
    /// spawn: it chooses again and walks the whole way.
    ///
    /// Example: `walker.restart();`
    pub fn restart(&mut self) {
        self.target = None;
        self.navigator.stand();
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
            self.is_target_reached = false;
            self.navigator.start_goal();
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
                    self.navigator.sidestep_count
                );
            }
            turn_toward(player, place.get_foot());
            self.navigator.stand();
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
        WalkStep {
            input: self
                .navigator
                .walk_toward(world, player, place.get_foot(), now_s, dt_s),
            console: None,
        }
    }
}

/// `Fighter` is a fighting bot's plan: its navigator, the enemy it goes after, how long it has
/// fired without a hit (`scan_s`), and the aim offset (right and up, radians) of its last hit.
pub struct Fighter {
    navigator: Navigator,
    target_id: Option<u32>,
    scan_s: f64,
    hit_offset_rad: Option<(f64, f64)>,
}

impl Fighter {
    /// `New` makes a fighter for a level with `dangers` to keep out of.
    ///
    /// Example: `let fighter = Fighter::New(find_warp_dangers(&world));`
    #[allow(non_snake_case)]
    pub fn New(dangers: Vec<Danger>) -> Fighter {
        Fighter {
            navigator: Navigator::New(dangers),
            target_id: None,
            scan_s: 0.0,
            hit_offset_rad: None,
        }
    }

    /// `restart` forgets the fighter's way, for a bot that has just come back at its spawn.
    ///
    /// Example: `fighter.restart();`
    pub fn restart(&mut self) {
        self.target_id = None;
        self.navigator.stand();
    }

    /// `note_shot` tells the fighter whether its laser hit the enemy this step (`dt_s` long): it
    /// keeps the aim offset of a hit, and otherwise scans further around the enemy, as wind or a
    /// warp may bend the laser off a straight aim.
    ///
    /// Example: `fighter.note_shot(view.hit_player_id.is_some(), 1.0 / 60.0);`
    pub fn note_shot(&mut self, is_hit: bool, dt_s: f64) {
        match is_hit {
            true => self.hit_offset_rad = Some(self.get_scan_offset_rad()),
            false => (self.hit_offset_rad, self.scan_s) = (None, self.scan_s + dt_s),
        }
    }

    /// `get_scan_offset_rad` gives the aim offset (right, up; radians) from a straight aim at the
    /// enemy: the last hit's, or a point on a spiral that widens up to `MAX_SCAN_RAD` the longer
    /// the laser misses.
    ///
    /// Example: `let (right_rad, up_rad) = fighter.get_scan_offset_rad();`
    fn get_scan_offset_rad(&self) -> (f64, f64) {
        if let Some(offset_rad) = self.hit_offset_rad {
            return offset_rad;
        }
        let radius_rad: f64 = (SCAN_GROWTH_RAD_PER_S * self.scan_s).min(MAX_SCAN_RAD);
        let angle_rad: f64 = std::f64::consts::TAU * SCAN_TURNS_PER_S * self.scan_s;
        (radius_rad * angle_rad.cos(), radius_rad * angle_rad.sin())
    }

    /// `step` turns `player` and gives this step's keys and laser at time `now_s` (`dt_s` after the
    /// last), against the nearest of `enemies` (each one's id and eye point): it walks to them
    /// around blocks and dangers (`Navigator`) until it sees them within `FIRE_RANGE_M` with no
    /// danger between, then faces them (offset by `get_scan_offset_rad`), fires, and closes in to
    /// `CLOSE_M`.
    ///
    /// Example: `let fight = fighter.step(&world, &mut player, &enemies, now_s, 1.0 / 60.0);`
    pub fn step(
        &mut self,
        world: &VoxelWorld,
        player: &mut VoxelPlayer,
        enemies: &[(u32, DVec4)],
        now_s: f64,
        dt_s: f64,
    ) -> FightStep {
        let eye: DVec4 = player.get_camera_frame(world).col(AXIS_W);
        let Some(&(id, target)) = enemies
            .iter()
            .min_by(|a, b| get_distance(eye, a.1).total_cmp(&get_distance(eye, b.1)))
        else {
            self.target_id = None;
            self.navigator.stand();
            return FightStep::default();
        };
        if self.target_id != Some(id) {
            self.target_id = Some(id);
            self.navigator.start_goal();
            (self.scan_s, self.hit_offset_rad) = (0.0, None);
        }
        let distance_m: f64 = get_distance(eye, target) * world.r;
        let toward: DVec4 = normalize_tangent(eye, target);
        let is_seen: bool = distance_m < FIRE_RANGE_M
            && world.find_clear_m(eye, toward, distance_m) > distance_m - SIGHT_SLACK_M
            && !self.navigator.crosses_danger(world.r, eye, target);
        if !is_seen {
            let input: Input = self
                .navigator
                .walk_toward(world, player, target, now_s, dt_s);
            aim_pitch(player, player.get_camera_frame(world), target);
            return FightStep {
                input,
                is_laser_on: false,
            };
        }
        turn_toward(player, target);
        aim_pitch(player, player.get_camera_frame(world), target);
        let (right_rad, up_rad) = self.get_scan_offset_rad();
        player.look(right_rad / LOOK_RAD_PER_PX, -up_rad / LOOK_RAD_PER_PX);
        self.navigator.stand();
        FightStep {
            input: Input {
                forward: distance_m > CLOSE_M,
                ..Input::default()
            },
            is_laser_on: true,
        }
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
    player.look(
        0.0,
        -local.normalize().y.clamp(-1.0, 1.0).asin() / LOOK_RAD_PER_PX,
    );
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
    //! | A window's hold on a built-in map's pylon | from each side, near and at the edge of reach, standing (the server takes every hold the window's own aim makes) | `the_server_takes_every_hold_a_window_makes` |

    use clap::ValueEnum;

    use super::*;
    use crate::console_reach::find_console_places;
    use crate::geometry::{AXIS_X, AXIS_Y, AXIS_Z, make_boost, make_rotation};
    use crate::map::{Map, MapName};

    const R_M: f64 = 40.0;
    const REACH_M: f64 = 3.0;
    const GOAL_M: f64 = 30.0;
    const ANGLE_TOLERANCE_RAD: f64 = 1e-9;
    const SIDE_COUNT: usize = 12;
    const GAPS_M: [f64; 3] = [0.5, 1.5, 2.9];
    const PYLON_RADIUS_M: f64 = 1.35;
    const EYE_HEIGHT_M: f64 = 1.6;

    /// `point_at` gives the ground point `along_m` ahead (-z) and `right_m` to the right (+x) of
    /// the origin.
    fn point_at(along_m: f64, right_m: f64) -> DVec4 {
        (make_boost(AXIS_Z, -along_m / R_M) * make_boost(AXIS_X, right_m / R_M)).col(AXIS_W)
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
            Navigator::New(dangers).choose_heading_rad(R_M, DMat4::IDENTITY, goal)
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
        assert!(
            bent_rad < 0.0,
            "a danger a little to the right sends the bot left"
        );
        let around: Danger = Danger {
            point: point_at(-1.0, 0.0),
            radius_m: 5.0,
        };
        assert!(heading(vec![around]).abs() < ANGLE_TOLERANCE_RAD);
    }

    #[test]
    fn the_server_takes_every_hold_a_window_makes() {
        for name in MapName::value_variants() {
            let map: Map = Map::Builtin(name.get_folder_name()).expect("a built-in map");
            if map.space.curvature_radius_m.is_none() || map.structures.is_empty() {
                continue;
            }
            let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a world");
            let places: Vec<ConsolePlace> = find_console_places(&map).expect("console places");
            let mut window_hold_count: usize = 0;
            for (index, place) in places.iter().enumerate() {
                for side in 0..SIDE_COUNT {
                    for gap_m in GAPS_M {
                        let turn_rad: f64 = std::f64::consts::TAU * side as f64 / SIDE_COUNT as f64;
                        let eye: DMat4 = place.frame
                            * make_rotation(AXIS_X, AXIS_Z, turn_rad)
                            * make_boost(AXIS_Z, (PYLON_RADIUS_M + gap_m) / world.r)
                            * make_boost(AXIS_Y, EYE_HEIGHT_M / world.r);
                        let aimed: Option<u8> = world.find_aimed_material(eye, REACH_M);
                        if aimed == Some(world.consoles[index].material) {
                            window_hold_count += 1;
                            assert!(
                                place.is_held_from(eye, REACH_M),
                                "map {:?}, console {index}, side {side}, gap {gap_m} m",
                                map.name
                            );
                        }
                    }
                }
            }
            assert!(
                window_hold_count > 0,
                "map {:?}: no window aim met a console",
                map.name
            );
        }
    }
}
