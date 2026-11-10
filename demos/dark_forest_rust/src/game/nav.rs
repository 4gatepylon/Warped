//! `nav` finds robots' walking paths across the forest's levels: a 2.5D grid of standing spots
//! (on the ground, decks and bridges, `CELL_M` apart) joined to neighbours a step up or down and
//! through ladders, searched with A*.
//!
//! Interface: `NavGraph` (`build`, `find_path`, `find_path_near`), `NavStep`.
//!
//! Example: `let path: Option<Vec<NavStep>> = nav.find_path(bot_feet_m, player_feet_m);`
//!
//! Must know:
//! - A spot stands on the top of a solid under its cell's centre, below `MAX_STAND_HEIGHT_M`, with
//!   room for a body (`BODY_RADIUS_M`, `BODY_HEIGHT_M`) above it. Spots in a wormhole's throat are
//!   left out, so robots walk around wormholes.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use glam::{Vec2, Vec3};

use super::collision::Aabb;
use super::warps::Warps;
use super::world::LadderVolume;

/// `CELL_M` is the grid's spacing.
const CELL_M: f32 = 0.5;
/// `BODY_RADIUS_M` and `BODY_HEIGHT_M` are the room a robot needs (its radius plus a margin).
const BODY_RADIUS_M: f32 = 0.45;
const BODY_HEIGHT_M: f32 = 1.85;
/// `BODY_CENTRE_ABOVE_FEET_M` is where a wormhole acts on a body.
const BODY_CENTRE_ABOVE_FEET_M: f32 = 0.9;
/// `STEP_M` is the highest step between neighbouring spots.
const STEP_M: f32 = 0.45;
/// `MAX_STAND_HEIGHT_M` leaves out the tops of the bamboo.
const MAX_STAND_HEIGHT_M: f32 = 13.0;
/// `LADDER_REACH_M` is how far from a ladder's foot or top its spots may be.
const LADDER_REACH_M: f32 = 1.6;
/// `LADDER_FOOT_BACK_M` and `LADDER_TOP_IN_M` place a ladder's foot behind it and its top on the
/// deck it leads to.
const LADDER_FOOT_BACK_M: f32 = 0.5;
const LADDER_TOP_IN_M: f32 = 0.9;
/// `ENDPOINT_REACH_M` is how far a path's ends may be from the nearest spot.
const ENDPOINT_REACH_M: f32 = 3.0;
/// `HEIGHT_WEIGHT` makes height count more than ground distance in finding the nearest spot.
const HEIGHT_WEIGHT: f32 = 3.0;
/// `MOUTH_KEEP_OUT_M` keeps spots this far outside a wormhole's walk-in throat.
const MOUTH_KEEP_OUT_M: f32 = 0.6;

/// `NavStep` is one spot of a path, with the ladder (an index into the arena's ladders) climbed to
/// reach it, if any.
#[derive(Clone, Copy, Debug)]
pub struct NavStep {
    pub position_m: Vec3,
    pub ladder: Option<usize>,
}

/// `NavGraph` is the standing spots and how they join.
pub struct NavGraph {
    positions_m: Vec<Vec3>,
    edges: Vec<Vec<(u32, Option<usize>)>>,
    cell_nodes: Vec<Vec<u32>>,
    column_count: usize,
    row_count: usize,
    min_m: Vec2,
}

/// `Frontier` is an A* queue entry, ordered so the heap pops the lowest estimate first.
#[derive(Clone, Copy, PartialEq)]
struct Frontier {
    estimate_m: f32,
    node: u32,
}

impl Eq for Frontier {}

impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> Ordering {
        other.estimate_m.total_cmp(&self.estimate_m)
    }
}

impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NavGraph {
    /// `build` finds the standing spots among `solids` within the bounds, joins neighbours and the
    /// ends of every ladder, and leaves out the wormholes' throats.
    ///
    /// Example: `let nav = NavGraph::build(&solids, &arena.ladders, &arena.warps, low_m, high_m);`
    pub fn build(
        solids: &[Aabb],
        ladders: &[LadderVolume],
        warps: &Warps,
        bounds_min_m: Vec3,
        bounds_max_m: Vec3,
    ) -> NavGraph {
        let min_m: Vec2 = Vec2::new(bounds_min_m.x, bounds_min_m.z);
        let size_m: Vec2 = Vec2::new(bounds_max_m.x, bounds_max_m.z) - min_m;
        let column_count: usize = (size_m.x / CELL_M).ceil() as usize;
        let row_count: usize = (size_m.y / CELL_M).ceil() as usize;
        let cell_count: usize = column_count * row_count;
        let get_range = |low_m: f32, high_m: f32, origin_m: f32, count: usize| -> (usize, usize) {
            (
                ((low_m - origin_m) / CELL_M).floor().max(0.0) as usize,
                (((high_m - origin_m) / CELL_M).ceil().max(0.0) as usize).min(count),
            )
        };
        let mut blockers: Vec<Vec<(f32, f32)>> = vec![Vec::new(); cell_count];
        let mut floors: Vec<Vec<f32>> = vec![Vec::new(); cell_count];
        for solid in solids {
            let (i_low, i_high) = get_range(
                solid.min_m.x - BODY_RADIUS_M,
                solid.max_m.x + BODY_RADIUS_M,
                min_m.x,
                column_count,
            );
            let (j_low, j_high) = get_range(
                solid.min_m.z - BODY_RADIUS_M,
                solid.max_m.z + BODY_RADIUS_M,
                min_m.y,
                row_count,
            );
            for j in j_low..j_high {
                for i in i_low..i_high {
                    let cell: usize = j * column_count + i;
                    let centre_m: Vec2 = min_m + (Vec2::new(i as f32, j as f32) + 0.5) * CELL_M;
                    blockers[cell].push((solid.min_m.y, solid.max_m.y));
                    let is_under: bool = centre_m.x > solid.min_m.x
                        && centre_m.x < solid.max_m.x
                        && centre_m.y > solid.min_m.z
                        && centre_m.y < solid.max_m.z;
                    if is_under && solid.max_m.y < MAX_STAND_HEIGHT_M {
                        floors[cell].push(solid.max_m.y);
                    }
                }
            }
        }
        let mut positions_m: Vec<Vec3> = Vec::new();
        let mut cell_nodes: Vec<Vec<u32>> = vec![Vec::new(); cell_count];
        for cell in 0..cell_count {
            let centre_m: Vec2 = min_m
                + (Vec2::new((cell % column_count) as f32, (cell / column_count) as f32) + 0.5)
                    * CELL_M;
            let mut tops_m: Vec<f32> = std::mem::take(&mut floors[cell]);
            tops_m.sort_by(f32::total_cmp);
            tops_m.dedup_by(|a, b| (*a - *b).abs() < 0.05);
            for top_m in tops_m {
                let position_m: Vec3 = Vec3::new(centre_m.x, top_m, centre_m.y);
                let has_room: bool = blockers[cell].iter().all(|&(low_m, high_m)| {
                    high_m <= top_m + 0.05 || low_m >= top_m + BODY_HEIGHT_M
                });
                let is_in_mouth: bool = warps.mouths.iter().any(|mouth| {
                    mouth
                        .centre_m
                        .distance(position_m + Vec3::Y * BODY_CENTRE_ABOVE_FEET_M)
                        < mouth.throat_radius_m + MOUTH_KEEP_OUT_M
                });
                if has_room && !is_in_mouth {
                    cell_nodes[cell].push(positions_m.len() as u32);
                    positions_m.push(position_m);
                }
            }
        }
        let mut edges: Vec<Vec<(u32, Option<usize>)>> = vec![Vec::new(); positions_m.len()];
        for cell in 0..cell_count {
            let (i, j) = ((cell % column_count) as i64, (cell / column_count) as i64);
            for (di, dj) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
                let (a, b) = (i + di, j + dj);
                if a < 0 || b < 0 || a as usize >= column_count || b as usize >= row_count {
                    continue;
                }
                let other: usize = b as usize * column_count + a as usize;
                for &n in &cell_nodes[cell] {
                    for &m in &cell_nodes[other] {
                        if (positions_m[n as usize].y - positions_m[m as usize].y).abs() <= STEP_M {
                            edges[n as usize].push((m, None));
                            edges[m as usize].push((n, None));
                        }
                    }
                }
            }
        }
        let mut graph: NavGraph = NavGraph {
            positions_m,
            edges,
            cell_nodes,
            column_count,
            row_count,
            min_m,
        };
        for (index, ladder) in ladders.iter().enumerate() {
            let facing: Vec3 = (-ladder.frame.z_axis.truncate())
                .with_y(0.0)
                .normalize_or_zero();
            let centre_m: Vec3 = (ladder.aabb.min_m + ladder.aabb.max_m) * 0.5;
            let foot: Option<u32> = graph.find_nearest(
                centre_m.with_y(ladder.aabb.min_m.y) - facing * LADDER_FOOT_BACK_M,
                LADDER_REACH_M,
            );
            let top: Option<u32> = graph.find_nearest(
                centre_m.with_y(ladder.aabb.max_m.y) + facing * LADDER_TOP_IN_M,
                LADDER_REACH_M,
            );
            if let (Some(foot), Some(top)) = (foot, top)
                && graph.positions_m[top as usize].y - graph.positions_m[foot as usize].y > STEP_M
            {
                graph.edges[foot as usize].push((top, Some(index)));
                graph.edges[top as usize].push((foot, Some(index)));
            }
        }
        graph
    }

    /// `find_nearest` is the spot nearest `position_m` within `reach_m` across the ground and
    /// within a step or so in height, height counting `HEIGHT_WEIGHT` times as much.
    fn find_nearest(&self, position_m: Vec3, reach_m: f32) -> Option<u32> {
        let reach_cell_count: i64 = (reach_m / CELL_M).ceil() as i64;
        let i: i64 = ((position_m.x - self.min_m.x) / CELL_M).floor() as i64;
        let j: i64 = ((position_m.z - self.min_m.y) / CELL_M).floor() as i64;
        let mut best: Option<(f32, u32)> = None;
        for b in
            (j - reach_cell_count).max(0)..=(j + reach_cell_count).min(self.row_count as i64 - 1)
        {
            for a in (i - reach_cell_count).max(0)
                ..=(i + reach_cell_count).min(self.column_count as i64 - 1)
            {
                for &node in &self.cell_nodes[b as usize * self.column_count + a as usize] {
                    let offset_m: Vec3 = self.positions_m[node as usize] - position_m;
                    let across_m: f32 = offset_m.with_y(0.0).length();
                    if across_m > reach_m || offset_m.y.abs() > reach_m {
                        continue;
                    }
                    let score: f32 = across_m + offset_m.y.abs() * HEIGHT_WEIGHT;
                    if best.is_none_or(|(best_score, _)| score < best_score) {
                        best = Some((score, node));
                    }
                }
            }
        }
        best.map(|(_, node)| node)
    }

    /// `find_path` is the shortest path of spots from near `from_m` to near `to_m`, or `None` if
    /// either end is far from every spot or no path joins them.
    ///
    /// Example: `let path = nav.find_path(bot.feet_m, player.feet_m).unwrap_or_default();`
    pub fn find_path(&self, from_m: Vec3, to_m: Vec3) -> Option<Vec<NavStep>> {
        let goal: u32 = self.find_nearest(to_m, ENDPOINT_REACH_M)?;
        let goal_m: Vec3 = self.positions_m[goal as usize];
        self.search(from_m, goal_m, 0.0, |node: u32| node == goal)
    }

    /// `find_path_near` is the shortest path of spots from near `from_m` to any spot within
    /// `radius_m` of `to_m` across the ground and `height_m` up or down, or `None` if no such spot
    /// can be reached; the nearest spot to `to_m` may be cut off (inside a hut whose door is too
    /// narrow for a robot), where a spot around it is not.
    ///
    /// Example: `let path = nav.find_path_near(feet_m, console_foot_m, 3.0, 1.0).unwrap_or_default();`
    pub fn find_path_near(
        &self,
        from_m: Vec3,
        to_m: Vec3,
        radius_m: f32,
        height_m: f32,
    ) -> Option<Vec<NavStep>> {
        self.search(from_m, to_m, radius_m, |node: u32| {
            let offset_m: Vec3 = self.positions_m[node as usize] - to_m;
            offset_m.with_y(0.0).length() <= radius_m && offset_m.y.abs() <= height_m
        })
    }

    /// `search` runs A* from the spot nearest `from_m` to the first spot where `is_goal` holds,
    /// estimating the way left as the distance to `goal_m` less `slack_m` (so it never guesses
    /// too much), and gives the path's spots after the start.
    ///
    /// Example: `let path = self.search(from_m, goal_m, 0.0, |node| node == goal);`
    fn search(
        &self,
        from_m: Vec3,
        goal_m: Vec3,
        slack_m: f32,
        is_goal: impl Fn(u32) -> bool,
    ) -> Option<Vec<NavStep>> {
        let start: u32 = self.find_nearest(from_m, ENDPOINT_REACH_M)?;
        let estimate =
            |node: u32| (self.positions_m[node as usize].distance(goal_m) - slack_m).max(0.0);
        let mut cost_m: Vec<f32> = vec![f32::INFINITY; self.positions_m.len()];
        let mut came_from: Vec<(u32, Option<usize>)> =
            vec![(u32::MAX, None); self.positions_m.len()];
        let mut frontier: BinaryHeap<Frontier> = BinaryHeap::new();
        cost_m[start as usize] = 0.0;
        frontier.push(Frontier {
            estimate_m: estimate(start),
            node: start,
        });
        let mut reached: Option<u32> = None;
        while let Some(Frontier { estimate_m, node }) = frontier.pop() {
            if is_goal(node) {
                reached = Some(node);
                break;
            }
            let here_m: Vec3 = self.positions_m[node as usize];
            if estimate_m > cost_m[node as usize] + estimate(node) + 1e-3 {
                continue;
            }
            for &(next, ladder) in &self.edges[node as usize] {
                let next_m: Vec3 = self.positions_m[next as usize];
                let new_cost_m: f32 = cost_m[node as usize] + here_m.distance(next_m);
                if new_cost_m < cost_m[next as usize] {
                    cost_m[next as usize] = new_cost_m;
                    came_from[next as usize] = (node, ladder);
                    frontier.push(Frontier {
                        estimate_m: new_cost_m + estimate(next),
                        node: next,
                    });
                }
            }
        }
        let mut node: u32 = reached?;
        let mut steps: Vec<NavStep> = Vec::new();
        while node != start {
            let (previous, ladder) = came_from[node as usize];
            steps.push(NavStep {
                position_m: self.positions_m[node as usize],
                ladder,
            });
            node = previous;
        }
        steps.reverse();
        Some(steps)
    }
}

/// `tests` checks paths and a robot's climb on the real forest.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | path's levels | ground to deck (a ladder), ground to ground across the maze | `test_path_climbs_to_the_treehouse`, `test_path_crosses_the_labyrinth` |
/// | goal's nearest spot | reachable, cut off (inside the treehouse's pylon hut: a spot around it) | `test_path_reaches_near_the_deck_pylon` |
/// | robot following a path | up a ladder | `test_robot_climbs_onto_the_treehouse_deck` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::bots::{Bot, BotContext};
    use crate::game::world::Arena;

    fn forest() -> (Arena, Vec<Aabb>, NavGraph) {
        let arena: Arena = Arena::from_map(&crate::forest_layout::build());
        let solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let nav: NavGraph = NavGraph::build(
            &solids,
            &arena.ladders,
            &arena.warps,
            arena.bounds_min_m,
            arena.bounds_max_m,
        );
        (arena, solids, nav)
    }

    /// `test_path_climbs_to_the_treehouse` checks the path from red's spawn to its treehouse
    /// deck climbs a ladder and ends on the deck.
    #[test]
    fn test_path_climbs_to_the_treehouse() {
        let (_, _, nav) = forest();
        let path: Vec<NavStep> = nav
            .find_path(Vec3::new(0.0, 0.0, 52.0), Vec3::new(14.0, 7.0, 44.0))
            .expect("a path to the treehouse");
        assert!(path.iter().any(|step| step.ladder.is_some()));
        assert!(path.last().expect("steps").position_m.y > 6.5);
    }

    /// `test_path_reaches_near_the_deck_pylon` checks no path reaches the pylon in red's
    /// treehouse hut itself (a bamboo clump narrows its door for robots), but one reaches the deck
    /// within 3.5 m of it, a player's reach.
    #[test]
    fn test_path_reaches_near_the_deck_pylon() {
        let (_, _, nav) = forest();
        let (spawn_m, pylon_m) = (Vec3::new(-6.0, 0.0, 52.0), Vec3::new(17.7, 7.0, 42.0));
        assert!(nav.find_path(spawn_m, pylon_m).is_none());
        let path: Vec<NavStep> = nav
            .find_path_near(spawn_m, pylon_m, 3.5, 1.0)
            .expect("a path near the pylon");
        let end_m: Vec3 = path.last().expect("steps").position_m;
        assert!(
            (end_m - pylon_m).with_y(0.0).length() <= 3.5 && (end_m.y - pylon_m.y).abs() <= 1.0
        );
    }

    /// `test_path_crosses_the_labyrinth` checks red's spawn reaches blue's on foot.
    #[test]
    fn test_path_crosses_the_labyrinth() {
        let (_, _, nav) = forest();
        assert!(
            nav.find_path(Vec3::new(0.0, 0.0, 52.0), Vec3::new(0.0, 0.0, -52.0))
                .is_some()
        );
    }

    /// `test_robot_climbs_onto_the_treehouse_deck` runs a robot at red's treehouse ladder towards
    /// a player on the deck for 30 s and checks it gets up.
    #[test]
    fn test_robot_climbs_onto_the_treehouse_deck() {
        let (arena, solids, nav) = forest();
        let mut bot: Bot = Bot::new(Vec3::new(8.0, 0.0, 50.0), 3);
        let context: BotContext = BotContext {
            solids: &solids,
            get_warp_pull_m_per_s: &|_| Vec3::ZERO,
            nav: &nav,
            ladders: &arena.ladders,
            player_eye_m: Vec3::new(14.0, 8.7, 44.0),
            player_velocity_m_s: Vec3::ZERO,
            is_player_alive: true,
            is_player_crouched: false,
        };
        let dt_s: f32 = 1.0 / 60.0;
        let mut highest_m: f32 = 0.0;
        for _ in 0..(30.0 / dt_s) as usize {
            let _ = bot.update(&context, dt_s);
            highest_m = highest_m.max(bot.feet_m.y);
        }
        assert!(
            highest_m > 6.5,
            "the robot got to {highest_m} m, at {:?}",
            bot.feet_m
        );
    }
}
