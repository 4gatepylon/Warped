//! `voxel_clearance.rs` gives each warp a clearance grid: over the warp's reach, in its local
//! chart (the spatial part p of a point in the warp's frame), a lower bound on the distance from
//! each cell's centre to any shape of a static structure. A point's distance to a shape is at least
//! its cell's value less its chart distance |p - centre| from the cell's centre (the hyperbolic
//! metric is at most |dp|), so a bent step in the renderer whose chord is shorter than that can't
//! reach a shape: it skips the static structures and tests only the ground and avatars.
//!
//! Interface: `ClearanceGrids::Build`, `ClearanceGrid`, `STORED_M_PER_UNIT`.
//!
//! Example: `let grids = ClearanceGrids::Build(&world);`

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::get_distance;
use crate::voxel_world::{BEND_STEP_FRAC, VoxelWorld};

pub const STORED_M_PER_UNIT: f64 = 0.1;
const CELL_M: f64 = 0.5;
const CAP_MARGIN_M: f64 = 0.5;
const REACH_MARGIN_FRAC: f64 = 1.1;

/// `ClearanceGrid` is one warp's grid: where its values start, its cells per axis, its half size
/// in the chart (cells span p from -`half_size` to `half_size` on each axis), and how many entries
/// of the warp's list are static structures (avatars come after them).
#[derive(Clone, Copy)]
pub struct ClearanceGrid {
    pub offset: usize,
    pub cell_count: usize,
    pub half_size: f64,
    pub static_entry_count: usize,
}

/// `ClearanceGrids` is every warp's `ClearanceGrid` and all their values, one byte per cell
/// (x fastest, then y, then z), each a lower bound in `STORED_M_PER_UNIT` steps on the distance
/// from the cell's centre to a block, capped a little above the warp's longest step.
#[derive(Default)]
pub struct ClearanceGrids {
    pub grids: Vec<ClearanceGrid>,
    pub values: Vec<u8>,
}

impl ClearanceGrids {
    /// `Build` makes every warp's grid from the world's static structures.
    ///
    /// Example: `let grids = ClearanceGrids::Build(&world);`
    #[allow(non_snake_case)]
    pub fn Build(world: &VoxelWorld) -> ClearanceGrids {
        let mut grids = ClearanceGrids::default();
        for w in 0..world.warps.len() {
            let (cell_count, half_size, values) = build_grid(world, w);
            grids.grids.push(ClearanceGrid {
                offset: grids.values.len(),
                cell_count,
                half_size,
                static_entry_count: world.warps[w].list.len(),
            });
            grids.values.extend(values);
        }
        grids
    }
}

/// `build_grid` makes warp w's grid. Every cell starts at the cap, and each static structure in
/// the warp's list lowers the cells in its window to its shapes' distance bound at their centre, in
/// its chart q = R p / p.w: the Klein metric is at least |dk|, so that is at most the hyperbolic
/// distance. Cells outside its window are at least the cap away, as the metric is at least
/// |dp| / cosh ρ out to the radius ρ of the further point. The structures are shared out among
/// threads, whose grids are merged by minimum. It returns the cells per axis, the half size and
/// the values.
///
/// Example: `let (n, half_size, values) = build_grid(&world, 0);`
fn build_grid(world: &VoxelWorld, w: usize) -> (usize, f64, Vec<u8>) {
    let warp = &world.warps[w];
    let extent: f64 = warp.reach * REACH_MARGIN_FRAC;
    let half_size: f64 = extent.sinh();
    let n: usize = (2.0 * half_size * world.r / CELL_M).ceil() as usize;
    let side: f64 = 2.0 * half_size / n as f64;
    let cap_m: f64 = BEND_STEP_FRAC * warp.reach * world.r + CAP_MARGIN_M;
    let cap_unit: u8 = (cap_m / STORED_M_PER_UNIT).min(f64::from(u8::MAX)) as u8;
    let cap: f64 = cap_m / world.r;
    let near: Vec<usize> = warp
        .list
        .iter()
        .copied()
        .filter(|&k| {
            let st = &world.structures[k];
            get_distance(st.centre, warp.centre) < extent + cap + st.radius
        })
        .collect();
    let thread_count: usize = std::thread::available_parallelism().map_or(1, usize::from);
    let chunk_size: usize = near.len().div_ceil(thread_count).max(1);
    let partial_grids: Vec<Vec<u8>> = std::thread::scope(|scope| {
        let handles: Vec<_> = near
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    let mut values: Vec<u8> = vec![cap_unit; n * n * n];
                    for &k in chunk {
                        let st = &world.structures[k];
                        let to_structure: DMat4 = st.inverse * warp.frame;
                        let centre_p: DVec3 = (warp.inverse * st.centre).truncate();
                        let window: f64 = (cap + st.radius) * (extent + cap + st.radius).cosh();
                        let lo: DVec3 = ((centre_p - window + half_size) / side)
                            .floor()
                            .max(DVec3::ZERO);
                        let hi: DVec3 = ((centre_p + window + half_size) / side)
                            .floor()
                            .min(DVec3::splat(n as f64 - 1.0));
                        for kz in lo.z as usize..=hi.z as usize {
                            for j in lo.y as usize..=hi.y as usize {
                                for i in lo.x as usize..=hi.x as usize {
                                    let p: DVec3 =
                                        (DVec3::new(i as f64, j as f64, kz as f64) + 0.5) * side
                                            - half_size;
                                    let local: DVec4 =
                                        to_structure * p.extend((1.0 + p.length_squared()).sqrt());
                                    let q: DVec3 = local.truncate() / local.w * world.r;
                                    let d_m: f64 = world.get_distance_m(k, q);
                                    let unit: u8 = (d_m.max(0.0) / STORED_M_PER_UNIT)
                                        .floor()
                                        .min(f64::from(cap_unit))
                                        as u8;
                                    let index: usize = i + n * (j + n * kz);
                                    values[index] = values[index].min(unit);
                                }
                            }
                        }
                    }
                    values
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a clearance grid thread panicked"))
            .collect()
    });
    let mut values: Vec<u8> = vec![cap_unit; n * n * n];
    for partial in partial_grids {
        for (value, other) in values.iter_mut().zip(partial) {
            *value = (*value).min(other);
        }
    }
    (n, half_size, values)
}

#[cfg(test)]
mod tests {
    //! Tests for `voxel_clearance.rs`: the shader's bound (a grid value less a point's chart
    //! distance from its cell's centre) against the distance to the nearest shape, measured by
    //! rays cast from the point in many directions (so a measured distance is at least the true
    //! one).
    //!
    //! Partitions of the input, a point inside a warp's reach and the warp:
    //! - the point's measured distance to a block: under the warp's cap (where its steps can reach
    //!   blocks), over it;
    //! - the warp: the black hole, mouth A, mouth B.
    //!
    //! | Test | Distance | Warp |
    //! | --- | --- | --- |
    //! | `bounds_stay_under_the_distance_to_blocks` | both | all three |
    //! | `bounds_near_blocks_are_tight` | under the cap | all three |

    use super::*;
    use crate::geometry::get_distance;
    use crate::map::{Map, get_map_folder};
    use crate::voxel_world::map_klein;

    const SAMPLE_COUNT_PER_WARP: usize = 300;
    const SEED_STEP: f64 = 0.618_033_988_749_895;
    const DIRECTION_COUNT: usize = 400;
    const MEASURE_REACH_M: f64 = 12.0;
    const TIGHT_SLACK_M: f64 = 1.5;

    /// `load_lunar_surface` loads the Lunar Surface map's world.
    ///
    /// Example: `let world = load_lunar_surface();`
    fn load_lunar_surface() -> VoxelWorld {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        VoxelWorld::FromMap(&map).expect("a voxel world")
    }

    /// `is_inside_a_shape` tells whether x is inside a static shape near warp w.
    ///
    /// Example: `if is_inside_a_shape(&world, 0, x) { ... }`
    fn is_inside_a_shape(world: &VoxelWorld, w: usize, x: DVec4) -> bool {
        world.warps[w].list.iter().any(|&k| {
            let local: DVec4 = world.structures[k].inverse * x;
            let q: DVec3 = local.truncate() / local.w * world.r;
            world.get_distance_m(k, q) < 0.0
        })
    }

    /// `measure_distance_m` gives the distance from x to the nearest static shape surface near
    /// warp w met by rays cast from x in `DIRECTION_COUNT` directions in each structure's chart
    /// (`f64::MAX` if none within `reach_m`), so it is at least the true distance.
    ///
    /// Example: `let d = measure_distance_m(&world, 0, x, 10.0);`
    fn measure_distance_m(world: &VoxelWorld, w: usize, x: DVec4, reach_m: f64) -> f64 {
        let mut best: f64 = f64::MAX;
        for &k in &world.warps[w].list {
            let st = &world.structures[k];
            if get_distance(st.centre, x) > reach_m / world.r + st.radius {
                continue;
            }
            let frame: DMat4 = crate::geometry::invert_lorentz(st.inverse);
            let local: DVec4 = st.inverse * x;
            let q0: DVec3 = local.truncate() / local.w * world.r;
            for i in 0..DIRECTION_COUNT {
                let u: [f64; 2] = [1.0, 2.0].map(|a| ((i as f64 + 0.5) * SEED_STEP * a).fract());
                let (z, phi) = (2.0 * u[0] - 1.0, std::f64::consts::TAU * u[1]);
                let ring: f64 = (1.0 - z * z).sqrt();
                let direction: DVec3 = DVec3::new(ring * phi.cos(), z, ring * phi.sin());
                let hit = match st.mesh {
                    Some(m) => world.meshes[m].find_hit_mu(q0, direction, 0.0, 2.0 * reach_m),
                    None => crate::shapes::find_hit_mu(
                        world.get_shapes(k),
                        q0,
                        direction,
                        0.0,
                        2.0 * reach_m,
                        None,
                    ),
                };
                if let Some((mu, _)) = hit {
                    let point: DVec4 = frame * map_klein((q0 + direction * mu) / world.r);
                    best = best.min(get_distance(point, x) * world.r);
                }
            }
        }
        best
    }

    /// `get_bound_m` gives the shader's bound at x for warp w, in meters, or `None` outside the grid.
    ///
    /// Example: `let bound = get_bound_m(&world, 0, x);`
    fn get_bound_m(world: &VoxelWorld, w: usize, x: DVec4) -> Option<f64> {
        let grid: ClearanceGrid = world.clearance.grids[w];
        let n: usize = grid.cell_count;
        let side: f64 = 2.0 * grid.half_size / n as f64;
        let p: DVec3 = (world.warps[w].inverse * x).truncate();
        let g: DVec3 = (p + grid.half_size) / side;
        if g.min_element() < 0.0 || g.max_element() >= n as f64 {
            return None;
        }
        let cell: DVec3 = g.floor();
        let index: usize = cell.x as usize + n * (cell.y as usize + n * cell.z as usize);
        let centre: DVec3 = (cell + 0.5) * side - grid.half_size;
        Some(
            f64::from(world.clearance.values[grid.offset + index]) * STORED_M_PER_UNIT
                - (p - centre).length() * world.r,
        )
    }

    /// `sample_points` gives deterministic points spread through warp w's reach.
    ///
    /// Example: `let points = sample_points(&world, 0);`
    fn sample_points(world: &VoxelWorld, w: usize) -> Vec<DVec4> {
        let warp = &world.warps[w];
        (0..SAMPLE_COUNT_PER_WARP)
            .map(|i| {
                let u: [f64; 3] =
                    [1.0, 2.0, 3.0].map(|a| ((i as f64 + 1.0) * SEED_STEP * a).fract());
                let direction: DVec3 =
                    DVec3::new(2.0 * u[0] - 1.0, 2.0 * u[1] - 1.0, 2.0 * u[2] - 1.0)
                        .normalize_or(DVec3::X);
                let rho: f64 = warp.reach * u[0].max(u[2]);
                warp.frame * (direction * rho.sinh()).extend(rho.cosh())
            })
            .collect()
    }

    #[test]
    fn bounds_stay_under_the_distance_to_blocks() {
        let world: VoxelWorld = load_lunar_surface();
        for w in 0..world.warps.len() {
            for x in sample_points(&world, w) {
                let bound_m: f64 = get_bound_m(&world, w, x).expect("a point inside the reach");
                let measured_m: f64 = measure_distance_m(&world, w, x, MEASURE_REACH_M);
                assert!(
                    bound_m <= measured_m,
                    "warp {w}: bound {bound_m} m over the measured {measured_m} m"
                );
            }
        }
    }

    #[test]
    fn bounds_near_blocks_are_tight() {
        let world: VoxelWorld = load_lunar_surface();
        let mut near_count: usize = 0;
        for w in 0..world.warps.len() {
            let cap_m: f64 = BEND_STEP_FRAC * world.warps[w].reach * world.r + CAP_MARGIN_M;
            for x in sample_points(&world, w) {
                let measured_m: f64 = measure_distance_m(&world, w, x, MEASURE_REACH_M);
                if measured_m > cap_m || is_inside_a_shape(&world, w, x) {
                    continue;
                }
                near_count += 1;
                let bound_m: f64 = get_bound_m(&world, w, x).expect("a point inside the reach");
                assert!(
                    measured_m - bound_m < TIGHT_SLACK_M,
                    "warp {w}: bound {bound_m} m, measured {measured_m} m"
                );
            }
        }
        assert!(
            near_count > 0,
            "no sample point came within a cap of a block"
        );
    }
}
