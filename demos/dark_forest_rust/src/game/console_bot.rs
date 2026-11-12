//! `console_bot` walks a bot as a player walks (`Player::update`), along the forest's paths
//! (`nav.rs`: the ground, decks, bridges and rope ladders): to the other team's consoles in turn
//! (`--bot-objective consoles`), holding `E` on each once its view meets it within reach
//! (`ConsolePlace::is_held_from`, the game server's own test), or toward a player it hunts
//! (`BotWalker::walk_to`). A bot win so shows that every pylon can be reached on foot (W-126), as
//! Lunar Surface's `console_bot.rs` shows it there.
//!
//! Interface: `BotWalker` (`New`, `walk_to`, `step_consoles`), `WalkStep`.
//!
//! Example: `let walk: WalkStep = walker.step_consoles(&player, &nav, &ladders, &places, &off, 0, 3.0, now_s, dt_s);`
//!
//! Must know:
//! - Limitations: it follows the robots' grid of standing spots, so it walks around wormhole
//!   mouths rather than through them; it gives a console up after `GIVE_UP_S` and says where it
//!   stood.

use glam::{DMat4, Vec2, Vec3};

use super::nav::{NavGraph, NavStep};
#[cfg(test)]
use super::player::Movement;
use super::player::{Player, PlayerInput};
use super::world::LadderVolume;
use crate::console_reach::{ConsolePlace, REACH_TOLERANCE_M};

/// `WAYPOINT_REACH_M` and `WAYPOINT_HEIGHT_M` are how near a path's spot the bot comes, across and
/// up, before heading for the next.
const WAYPOINT_REACH_M: f32 = 0.6;
const WAYPOINT_HEIGHT_M: f32 = 0.6;
/// `REPLAN_S` is how often the bot finds a new path, and `GOAL_MOVED_M` how far its goal may move
/// before it finds one at once.
const REPLAN_S: f32 = 3.0;
const GOAL_MOVED_M: f32 = 2.0;
/// `STUCK_M_PER_S` is the speed below which the bot counts as stuck; it jumps after
/// `JUMP_AFTER_STUCK_S` stuck and finds a new path after `REPLAN_AFTER_STUCK_S`.
const STUCK_M_PER_S: f32 = 0.5;
const JUMP_AFTER_STUCK_S: f32 = 0.6;
const REPLAN_AFTER_STUCK_S: f32 = 2.0;
/// `AIM_LEVEL_M` is how far below or above a console's foot the bot's feet may be for it to turn
/// to the console and walk straight at it, once within reach across.
const AIM_LEVEL_M: f32 = 1.0;
/// `SPRINT_FROM_M` is how far from its goal the bot sprints, while it has `MIN_SPRINT_ENERGY_FRAC`.
const SPRINT_FROM_M: f32 = 12.0;
const MIN_SPRINT_ENERGY_FRAC: f64 = 0.2;
/// `GIVE_UP_S` is how long the bot walks to a console before it gives it up.
const GIVE_UP_S: f32 = 120.0;
/// `LADDER_FRONT_M` is how far in front of a ladder's middle the bot lines up to climb it (clear of
/// the deck above, which would stop its head), `LADDER_STEP_OFF_M` how far in front of it the bot
/// aims to step off a deck onto it (and behind it, on the deck, to line up first), and
/// `LADDER_BOTTOM_M` how near its foot's height the bot hops off it.
const LADDER_FRONT_M: f32 = 0.7;
const LADDER_STEP_OFF_M: f32 = 0.8;
const LADDER_BOTTOM_M: f32 = 0.3;
/// `LADDER_LINE_M` is how far beside a ladder's middle line a bot on a deck may be before it steps
/// back from the edge into line with the ladder, clear of the railings beside it.
const LADDER_LINE_M: f32 = 0.15;
/// `LADDER_HOP_M` is how near a ladder's foot the bot keeps jumping, so the ladder lets go of it
/// while it walks on (above a jump's height, so a hop doesn't grab the ladder again), and
/// `BODY_RADIUS_M` is a player's half width (`player.rs`).
const LADDER_HOP_M: f32 = 2.0;
const BODY_RADIUS_M: f32 = 0.3;
/// `CLIMB_UP_PITCH_RAD` is how far up the bot looks while it climbs, so forward climbs up.
const CLIMB_UP_PITCH_RAD: f32 = 0.2;

/// `WalkStep` is what the bot does this frame: the controls it holds, and the console it holds
/// `E` on, if any.
#[derive(Clone, Copy, Debug, Default)]
pub struct WalkStep {
    pub input: PlayerInput,
    pub console: Option<usize>,
}

/// `BotWalker` is a walking bot's plan: the console it walks to and since when, whether it reached
/// it, the consoles it gave up, its path (from `nav.rs`) with the next step, the goal and time it
/// was found for, and how long it has been stuck where it last stood.
pub struct BotWalker {
    target: Option<(usize, f32)>,
    is_target_reached: bool,
    given_up: Vec<usize>,
    path: Vec<NavStep>,
    path_index: usize,
    path_goal_m: Vec3,
    planned_s: f32,
    last_feet_m: Vec3,
    stuck_s: f32,
}

impl BotWalker {
    /// `New` makes a walker with no plan yet.
    ///
    /// Example: `let walker: BotWalker = BotWalker::New();`
    #[allow(non_snake_case)]
    pub fn New() -> BotWalker {
        BotWalker {
            target: None,
            is_target_reached: false,
            given_up: Vec::new(),
            path: Vec::new(),
            path_index: 0,
            path_goal_m: Vec3::splat(f32::MAX),
            planned_s: f32::MIN,
            last_feet_m: Vec3::ZERO,
            stuck_s: 0.0,
        }
    }

    /// `step_consoles` walks `player` (of `team`) toward the nearest of the other team's consoles
    /// still on (`places`, with `console_off`), and holds `E` once its view meets it within
    /// `reach_m`, at `now_s` (seconds into the match) and `dt_s` after the last step.
    ///
    /// Example: `let walk = walker.step_consoles(&player, &nav, &ladders, &places, &off, 0, 3.0, 12.0, 1.0 / 60.0);`
    #[allow(clippy::too_many_arguments)]
    pub fn step_consoles(
        &mut self,
        player: &Player,
        nav: &NavGraph,
        ladders: &[LadderVolume],
        places: &[ConsolePlace],
        console_off: &[bool],
        team: u32,
        reach_m: f64,
        now_s: f32,
        dt_s: f32,
    ) -> WalkStep {
        let is_on =
            |i: usize| places[i].team != team && !console_off.get(i).copied().unwrap_or(false);
        if self.target.is_none_or(|(i, _)| !is_on(i)) {
            let feet_m: Vec3 = player.feet_m;
            self.target = (0..places.len())
                .filter(|&i| is_on(i) && !self.given_up.contains(&i))
                .min_by(|&a, &b| {
                    let distance = |i: usize| get_foot_m(&places[i]).distance(feet_m);
                    distance(a).total_cmp(&distance(b))
                })
                .map(|i| (i, now_s));
            self.is_target_reached = false;
            if let Some((i, _)) = self.target {
                let distance_m: f32 = get_foot_m(&places[i]).distance(feet_m);
                println!("bot walks to console {i}, {distance_m:.1} m away");
            }
        }
        let Some((index, since_s)) = self.target else {
            return WalkStep::default();
        };
        let place: &ConsolePlace = &places[index];
        let centre_m: Vec3 = place.get_centre_m().as_vec3();
        let eye: DMat4 = player.camera_world().as_dmat4();
        if player.is_alive() && place.is_held_from(eye, reach_m) {
            if !self.is_target_reached {
                self.is_target_reached = true;
                println!(
                    "bot reached console {index} on foot in {:.1} s",
                    now_s - since_s
                );
            }
            return WalkStep {
                input: look_at(player, centre_m),
                console: Some(index),
            };
        }
        if now_s - since_s > GIVE_UP_S {
            println!(
                "bot gives up console {index}: stuck at {:.1?}, {:.1} m from it",
                player.feet_m,
                player.feet_m.distance(get_foot_m(place))
            );
            self.given_up.push(index);
            self.target = None;
            return WalkStep::default();
        }
        let foot_m: Vec3 = get_foot_m(place);
        let near_m: f32 = (reach_m + REACH_TOLERANCE_M) as f32;
        let is_in_reach: bool = (foot_m - player.feet_m).with_y(0.0).length() <= near_m
            && (foot_m.y - player.feet_m.y).abs() <= AIM_LEVEL_M
            && !player.is_climbing;
        let input: PlayerInput = if is_in_reach {
            PlayerInput {
                move_axes: Vec2::Y,
                ..look_at(player, centre_m)
            }
        } else {
            self.walk_to(player, nav, ladders, foot_m, near_m, now_s, dt_s)
        };
        WalkStep {
            input,
            console: None,
        }
    }

    /// `walk_to` steers `player` toward `goal_m`, or to any spot within `near_m` of it across
    /// (`NavGraph::find_path_near`) when `near_m` is above 0, along a path of `nav`, finding a new
    /// one every `REPLAN_S`, when the goal moves or when stuck, and climbing `ladders` where the
    /// path does: up by walking into the ladder, down by stepping off the deck onto it and holding
    /// crouch, and hopping (a jump lets go of a ladder) while near its foot once the path leaves it.
    ///
    /// Example: `let input: PlayerInput = walker.walk_to(&player, &nav, &ladders, enemy_feet_m, 0.0, 12.0, 1.0 / 60.0);`
    #[allow(clippy::too_many_arguments)]
    pub fn walk_to(
        &mut self,
        player: &Player,
        nav: &NavGraph,
        ladders: &[LadderVolume],
        goal_m: Vec3,
        near_m: f32,
        now_s: f32,
        dt_s: f32,
    ) -> PlayerInput {
        let moved_m_per_s: f32 = player.feet_m.distance(self.last_feet_m) / dt_s.max(1e-4);
        self.last_feet_m = player.feet_m;
        self.stuck_s = if moved_m_per_s < STUCK_M_PER_S {
            self.stuck_s + dt_s
        } else {
            0.0
        };
        let should_replan: bool = !player.is_climbing
            && (goal_m.distance(self.path_goal_m) > GOAL_MOVED_M
                || now_s - self.planned_s > REPLAN_S
                || self.stuck_s > REPLAN_AFTER_STUCK_S);
        if should_replan {
            self.path = if near_m > 0.0 {
                nav.find_path_near(player.feet_m, goal_m, near_m, AIM_LEVEL_M)
            } else {
                nav.find_path(player.feet_m, goal_m)
            }
            .unwrap_or_default();
            self.path_index = 0;
            self.path_goal_m = goal_m;
            self.planned_s = now_s;
            if self.stuck_s > REPLAN_AFTER_STUCK_S {
                self.stuck_s = 0.0;
            }
        }
        while let Some(step) = self.path.get(self.path_index) {
            let is_reached: bool = (step.position_m - player.feet_m).with_y(0.0).length()
                < WAYPOINT_REACH_M
                && (step.position_m.y - player.feet_m.y).abs() < WAYPOINT_HEIGHT_M;
            if !is_reached {
                break;
            }
            self.path_index += 1;
        }
        let left_m: f32 = self.path[self.path_index.min(self.path.len())..]
            .iter()
            .fold((player.feet_m, 0.0), |(from_m, sum_m), step| {
                (step.position_m, sum_m + from_m.distance(step.position_m))
            })
            .1;
        let should_sprint: bool =
            left_m > SPRINT_FROM_M && player.moves.energy_frac > MIN_SPRINT_ENERGY_FRAC;
        let is_jump_pressed: bool = self.stuck_s > JUMP_AFTER_STUCK_S;
        let Some(step) = self.path.get(self.path_index).copied() else {
            return PlayerInput {
                is_jump_pressed,
                ..walk_toward(player, goal_m, false)
            };
        };
        let is_at_ladder_foot: bool = is_at_ladder_foot(player, ladders);
        let Some(ladder_index) = step.ladder else {
            return PlayerInput {
                is_jump_pressed: is_jump_pressed || is_at_ladder_foot,
                ..walk_toward(player, step.position_m, should_sprint)
            };
        };
        let ladder: &LadderVolume = &ladders[ladder_index];
        let facing: Vec3 = (-ladder.frame.z_axis.truncate())
            .with_y(0.0)
            .normalize_or_zero();
        let centre_m: Vec3 =
            ((ladder.aabb.min_m + ladder.aabb.max_m) * 0.5).with_y(player.feet_m.y);
        let is_going_up: bool = step.position_m.y > player.feet_m.y;
        let front_m: Vec3 = centre_m - facing * LADDER_FRONT_M;
        let from_centre_m: Vec3 = player.feet_m - centre_m;
        let is_beside: bool =
            (from_centre_m - facing * from_centre_m.dot(facing)).length() > LADDER_LINE_M;
        let is_near_foot: bool = player.feet_m.y < ladder.aabb.min_m.y + LADDER_HOP_M;
        let is_under_deck: bool = from_centre_m.dot(facing) > -LADDER_FRONT_M * 0.5;
        match (player.is_climbing, is_going_up) {
            (true, true) if is_near_foot && is_under_deck => PlayerInput {
                is_jump_pressed: true,
                ..walk_toward(player, front_m, false)
            },
            (true, true) => PlayerInput {
                move_axes: Vec2::Y,
                ..look_along(player, facing, CLIMB_UP_PITCH_RAD)
            },
            (true, false) if player.feet_m.y - step.position_m.y < LADDER_BOTTOM_M => PlayerInput {
                is_jump_pressed: true,
                ..walk_toward(player, step.position_m, false)
            },
            (true, false) => PlayerInput {
                is_crouch_held: true,
                ..look_along(player, facing, 0.0)
            },
            (false, true) if (front_m - player.feet_m).length() < LADDER_LINE_M * 2.0 => {
                PlayerInput {
                    move_axes: Vec2::Y,
                    is_jump_pressed,
                    ..look_along(player, facing, CLIMB_UP_PITCH_RAD)
                }
            }
            (false, true) => PlayerInput {
                is_jump_pressed: is_jump_pressed || is_at_ladder_foot,
                ..walk_toward(player, front_m, false)
            },
            (false, false) if is_beside => PlayerInput {
                is_jump_pressed,
                ..walk_toward(player, centre_m + facing * LADDER_STEP_OFF_M, false)
            },
            (false, false) => PlayerInput {
                is_jump_pressed,
                ..walk_toward(player, centre_m - facing * LADDER_STEP_OFF_M, false)
            },
        }
    }
}

/// `is_at_ladder_foot` says whether `player`'s body is in one of `ladders` within `LADDER_HOP_M`
/// of its foot, where the bot keeps jumping while it walks, so the ladder doesn't grab it.
///
/// Example: `let should_hop: bool = is_at_ladder_foot(&player, &ladders);`
fn is_at_ladder_foot(player: &Player, ladders: &[LadderVolume]) -> bool {
    let reach_m: Vec3 = Vec3::new(BODY_RADIUS_M, 0.0, BODY_RADIUS_M);
    ladders.iter().any(|ladder| {
        let (low_m, high_m) = (ladder.aabb.min_m - reach_m, ladder.aabb.max_m + reach_m);
        let feet_m: Vec3 = player.feet_m;
        feet_m.x > low_m.x
            && feet_m.x < high_m.x
            && feet_m.z > low_m.z
            && feet_m.z < high_m.z
            && feet_m.y < ladder.aabb.min_m.y + LADDER_HOP_M
    })
}

/// `get_foot_m` gives where a console's box meets the floor under its middle, the goal of a walk
/// to it.
///
/// Example: `let foot_m: Vec3 = get_foot_m(&place);`
fn get_foot_m(place: &ConsolePlace) -> Vec3 {
    place.get_centre_m().as_vec3().with_y(place.min_m.y as f32)
}

/// `look_along` turns `player`'s view to `direction` (across) and `pitch_rad` this frame.
///
/// Example: `let input: PlayerInput = look_along(&player, Vec3::NEG_Z, 0.0);`
fn look_along(player: &Player, direction: Vec3, pitch_rad: f32) -> PlayerInput {
    let yaw_rad: f32 = (-direction.x).atan2(-direction.z);
    let yaw_delta_rad: f32 = (player.yaw_rad - yaw_rad + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    PlayerInput {
        look_delta_rad: Vec2::new(yaw_delta_rad, player.pitch_rad - pitch_rad),
        ..PlayerInput::default()
    }
}

/// `look_at` turns `player`'s view to `point_m` from their eye this frame.
///
/// Example: `let input: PlayerInput = look_at(&player, console_centre_m);`
fn look_at(player: &Player, point_m: Vec3) -> PlayerInput {
    let to_point: Vec3 = point_m - player.eye_position_m();
    let across_m: f32 = to_point.with_y(0.0).length();
    look_along(player, to_point.with_y(0.0), to_point.y.atan2(across_m))
}

/// `walk_toward` turns `player` level toward `point_m` and walks there, sprinting when
/// `should_sprint`.
///
/// Example: `let input: PlayerInput = walk_toward(&player, step_m, true);`
fn walk_toward(player: &Player, point_m: Vec3, should_sprint: bool) -> PlayerInput {
    PlayerInput {
        move_axes: Vec2::Y,
        is_sprint_held: should_sprint,
        ..look_along(player, point_m - player.feet_m, 0.0)
    }
}

/// `tests` walks a bot of each team from its spawn to every console of the other team on the real
/// forest, with the player's own movement, and checks it holds each within reach.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | team | blue (to red's pylons), red (to blue's) | `test_bots_walk_to_every_enemy_pylon` |
/// | start | each spawn of its own team, each of the other team's (beside their ground pylon) | `test_bots_walk_to_every_enemy_pylon` |
/// | pylon | on the ground, on a treehouse deck (up a ladder, and down again after) | `test_bots_walk_to_every_enemy_pylon` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::console_reach::find_console_places;
    use crate::game::collision::Aabb;
    use crate::game::world::Arena;
    use crate::map::{self, MapFile};

    const DT_S: f32 = 1.0 / 60.0;
    const REACH_M: f64 = 3.0;
    const MAX_WALK_S: f32 = 90.0;
    const HOLD_S: f32 = 1.0;

    /// `test_bots_walk_to_every_enemy_pylon` walks a bot of each team from every spawn of both teams
    /// (so it meets the pylons in both orders) and checks it reaches every console of the other team
    /// in turn, each within `MAX_WALK_S`.
    #[test]
    fn test_bots_walk_to_every_enemy_pylon() {
        let map: MapFile = map::load(&map::default_map_path()).expect("the map loads");
        let arena: Arena = Arena::from_map(&map);
        let places: Vec<ConsolePlace> = find_console_places(&map);
        let solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let nav: NavGraph = NavGraph::build(
            &solids,
            &arena.ladders,
            &arena.warps,
            arena.playable_min_m,
            arena.playable_max_m,
        );
        let starts: Vec<(Vec3, f32)> = arena.team_spawns.concat();
        let movement: Movement = Movement::New(
            &arena.map_player,
            arena.map_gravity_m_per_s2,
            &crate::settings::ServerSettings::Builtin(),
        );
        for (team, &(spawn_m, yaw_rad)) in
            (0..2_u32).flat_map(|team| starts.iter().map(move |start| (team, start)))
        {
            let mut player: Player = Player::new(spawn_m, yaw_rad);
            let mut walker: BotWalker = BotWalker::New();
            let mut console_off: Vec<bool> = vec![false; places.len()];
            let (mut now_s, mut held_s, mut walk_s) = (0.0_f32, 0.0_f32, 0.0_f32);
            while places
                .iter()
                .enumerate()
                .any(|(i, p)| p.team != team && !console_off[i])
            {
                let walk: WalkStep = walker.step_consoles(
                    &player,
                    &nav,
                    &arena.ladders,
                    &places,
                    &console_off,
                    team,
                    REACH_M,
                    now_s,
                    DT_S,
                );
                player.update(
                    &walk.input,
                    &movement,
                    &solids,
                    &arena.ladders,
                    arena.soft_edge.as_ref(),
                    DT_S,
                );
                (now_s, walk_s) = (now_s + DT_S, walk_s + DT_S);
                held_s = if walk.console.is_some() {
                    held_s + DT_S
                } else {
                    0.0
                };
                if let Some(index) = walk.console
                    && held_s > HOLD_S
                {
                    console_off[index] = true;
                    (held_s, walk_s) = (0.0, 0.0);
                }
                assert!(
                    walk_s < MAX_WALK_S,
                    "team {team}'s bot from {spawn_m} is stuck at {:?}",
                    player.feet_m
                );
            }
        }
    }
}
