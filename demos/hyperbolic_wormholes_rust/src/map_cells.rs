//! `map_cells.rs` cuts the ground into square cells of Fermi (u, v) for the bent ray tracer (W-68,
//! W-103; `render_voxel.metal`'s `trace_map_chord`), each with a bound on the terrain's height and
//! a mask of the structures within `MAP_CELL_REACH_M` and a margin of its square. Fermi
//! coordinates change by at most the distance moved (ds² = cosh² h cosh² v du² + cosh² h dv² +
//! dh²), so a chord of the map no longer than the reach can only meet the terrain where it comes
//! down to that bound, and only the structures in the mask, of the cell it starts in.
//!
//! Interface: `MAP_CELL_M`, `MAP_CELL_COUNT`, `MAP_CELL_REACH_M`, `MAX_MASKED_STRUCTURE_COUNT`,
//! `make_terrain_tops_m`, `make_map_cells`.
//!
//! Example: `let cells = make_map_cells(&world);`

use crate::terrain::{Terrain, get_fermi_coordinates};
use crate::voxel_world::VoxelWorld;

pub const MAP_CELL_M: f64 = 4.0;
pub const MAP_CELL_COUNT: usize = 88;
pub const MAP_CELL_REACH_M: f64 = 10.0;
pub const MAX_MASKED_STRUCTURE_COUNT: usize = 64;
const MARGIN_M: f64 = 0.05;
const STRUCTURES_PER_WORD: usize = 32;

/// `get_cells` gives the cells on the grid (a range of indices along u or v, maybe empty) whose
/// squares meet the range `lo_m..=hi_m` of u (or v).
///
/// Example: `for i in get_cells(-3.0, 5.0) { ... }`
fn get_cells(lo_m: f64, hi_m: f64) -> std::ops::Range<usize> {
    let to_cell = |at_m: f64| (at_m / MAP_CELL_M + MAP_CELL_COUNT as f64 / 2.0).floor();
    let clamp = |cell: f64| cell.clamp(0.0, MAP_CELL_COUNT as f64) as usize;
    clamp(to_cell(lo_m))..clamp(to_cell(hi_m) + 1.0)
}

/// `make_terrain_tops_m` gives each cell's (u fastest) bound on `terrain`'s height within the reach
/// and the margin of its square: the bowl's bound over the widened square, plus the highest mound
/// grid sample a bilinear read there can use (each sample counts in the cells within a grid step
/// of it, then each cell takes the highest within the widening), plus the margin. It panics if the
/// mounds reach past the cells.
///
/// Example: `let tops_m = make_terrain_tops_m(&terrain);`
pub fn make_terrain_tops_m(terrain: &Terrain) -> Vec<f32> {
    let n: usize = MAP_CELL_COUNT;
    let widen_m: f64 = MAP_CELL_REACH_M + MARGIN_M;
    let mut mound_tops_m: Vec<f64> = vec![0.0; n * n];
    if let Some(grid) = &terrain.mound_grid {
        let inner_m: f64 = n as f64 / 2.0 * MAP_CELL_M - widen_m;
        let far_m = |a: usize| grid.origin_m[a] + (grid.size[a] - 1) as f64 * grid.cell_m;
        assert!(
            (0..2).all(|a| grid.origin_m[a] > -inner_m && far_m(a) < inner_m),
            "make_terrain_tops_m: mounds from {:?} to ({}, {}) m, expected them within {inner_m} m of the middle",
            grid.origin_m,
            far_m(0),
            far_m(1)
        );
        for (k, &height_m) in grid.heights.iter().enumerate() {
            let at_m = |a: usize, i: usize| grid.origin_m[a] + i as f64 * grid.cell_m;
            let (u_m, v_m) = (at_m(0, k % grid.size[0]), at_m(1, k / grid.size[0]));
            for j in get_cells(v_m - grid.cell_m, v_m + grid.cell_m) {
                for i in get_cells(u_m - grid.cell_m, u_m + grid.cell_m) {
                    mound_tops_m[i + n * j] = mound_tops_m[i + n * j].max(f64::from(height_m));
                }
            }
        }
    }
    let widen: usize = (widen_m / MAP_CELL_M).ceil() as usize;
    let near = |i: usize| i.saturating_sub(widen)..=(i + widen).min(n - 1);
    let range_m = |i: usize| {
        let lo_m: f64 = (i as f64 - n as f64 / 2.0) * MAP_CELL_M;
        [lo_m - widen_m, lo_m + MAP_CELL_M + widen_m]
    };
    let mound_tops_m: &[f64] = &mound_tops_m;
    (0..n * n)
        .map(|c| {
            let (i, j) = (c % n, c / n);
            let mounds_m: f64 = near(j)
                .flat_map(|jj| near(i).map(move |ii| mound_tops_m[ii + n * jj]))
                .fold(0.0, f64::max);
            let bowl_m: f64 = terrain.get_max_bowl_height_m(range_m(i), range_m(j));
            (bowl_m + mounds_m + MARGIN_M) as f32
        })
        .collect()
}

/// `make_map_cells` gives the shader's cells (u fastest), each the bits of its terrain top (from
/// `world.map_terrain_tops_m`, 0 without terrain) and a mask of the structures (bit k of word
/// k / 32 for structure k) whose bounding sphere comes within the reach and the margin of its
/// square; empty when the world has more than `MAX_MASKED_STRUCTURE_COUNT` structures.
///
/// Example: `let cells = make_map_cells(&world);`
pub fn make_map_cells(world: &VoxelWorld) -> Vec<[u32; 4]> {
    if world.structures.len() > MAX_MASKED_STRUCTURE_COUNT {
        return Vec::new();
    }
    let n: usize = MAP_CELL_COUNT;
    let mut cells: Vec<[u32; 4]> = (0..n * n)
        .map(|c| match world.terrain {
            Some(_) => [world.map_terrain_tops_m[c].to_bits(), 0, 0, 0],
            None => [0; 4],
        })
        .collect();
    for (k, structure) in world.structures.iter().enumerate() {
        let [u_m, v_m, _] = get_fermi_coordinates(structure.centre, world.r);
        let reach_m: f64 = structure.radius * world.r + MAP_CELL_REACH_M + MARGIN_M;
        for j in get_cells(v_m - reach_m, v_m + reach_m) {
            for i in get_cells(u_m - reach_m, u_m + reach_m) {
                let gap = |at_m: f64, i: usize| {
                    let lo_m: f64 = (i as f64 - n as f64 / 2.0) * MAP_CELL_M;
                    (lo_m - at_m).max(at_m - lo_m - MAP_CELL_M).max(0.0)
                };
                if gap(u_m, i).hypot(gap(v_m, j)) < reach_m {
                    cells[i + n * j][1 + k / STRUCTURES_PER_WORD] |= 1 << (k % STRUCTURES_PER_WORD);
                }
            }
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    //! Tests of the map cells' terrain tops and structure masks.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Where the cell is | crater floor with mounds, wall, rim, outside the rim | `tops_bound_the_ground_within_reach` |
    //! | Chord against a structure | starts in its bounding sphere, enters it from outside | `masks_hold_every_structure_a_short_chord_meets` |
    //! | Chord length | short, the full reach | `masks_hold_every_structure_a_short_chord_meets` |
    //! | Structure count | at most `MAX_MASKED_STRUCTURE_COUNT`, more | `masks_hold_every_structure_a_short_chord_meets`, `too_many_structures_give_no_cells` |

    use glam::{DMat4, DVec4};

    use super::*;
    use crate::geometry::{get_distance, make_fermi_point, normalize_tangent};
    use crate::map::{Map, get_map_folder};

    const SAMPLES_PER_SIDE: usize = 9;
    const CHORD_SAMPLE_COUNT: usize = 40;
    const CHORD_DIRECTION_COUNT: usize = 12;
    const CHORD_HEIGHTS_M: [f64; 3] = [0.5, 4.0, 15.0];
    const SHORT_CHORD_M: f64 = 2.0;

    fn load_lunar_surface() -> VoxelWorld {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        VoxelWorld::FromMap(&map).expect("a voxel world")
    }

    fn get_cell(x: DVec4, r: f64) -> usize {
        let [u_m, v_m, _] = get_fermi_coordinates(x, r);
        let first = |at_m: f64| get_cells(at_m, at_m).start;
        first(u_m) + MAP_CELL_COUNT * first(v_m)
    }

    #[test]
    fn tops_bound_the_ground_within_reach() {
        let world: VoxelWorld = load_lunar_surface();
        let terrain: &Terrain = world.terrain.as_ref().expect("Lunar Surface has terrain");
        for [u_m, v_m] in [
            [0.0, 0.0],
            [0.0, 56.0],
            [0.0, 75.0],
            [0.0, 90.0],
            [0.0, 110.0],
            [150.0, 30.0],
        ] {
            let c: usize = get_cell(make_fermi_point(u_m, v_m, 0.0, world.r), world.r);
            let top_m: f64 = f64::from(world.map_terrain_tops_m[c]);
            for a in 0..SAMPLES_PER_SIDE * SAMPLES_PER_SIDE {
                let at_m = |k: usize, middle_m: f64| {
                    let t: f64 = k as f64 / (SAMPLES_PER_SIDE - 1) as f64 - 0.5;
                    middle_m + t * (MAP_CELL_M + 2.0 * MAP_CELL_REACH_M)
                };
                let middle_m =
                    |i: usize| (i as f64 + 0.5 - MAP_CELL_COUNT as f64 / 2.0) * MAP_CELL_M;
                let (su_m, sv_m) = (
                    at_m(a % SAMPLES_PER_SIDE, middle_m(c % MAP_CELL_COUNT)),
                    at_m(a / SAMPLES_PER_SIDE, middle_m(c / MAP_CELL_COUNT)),
                );
                let h_m: f64 = terrain.get_height_m(su_m, sv_m);
                assert!(
                    h_m <= top_m,
                    "at ({su_m}, {sv_m}): {h_m} m above the top {top_m} m of the cell at ({u_m}, {v_m})"
                );
            }
        }
        let rim_m: f64 = terrain.get_height_m(0.0, terrain.rim_m[1]);
        let floor_top_m: f64 = f64::from(
            world.map_terrain_tops_m
                [get_cell(make_fermi_point(-30.0, -30.0, 0.0, world.r), world.r)],
        );
        assert!(
            floor_top_m < rim_m / 2.0,
            "a floor cell's top {floor_top_m} m should be well below the rim's {rim_m} m"
        );
    }

    #[test]
    fn masks_hold_every_structure_a_short_chord_meets() {
        let world: VoxelWorld = load_lunar_surface();
        let cells: Vec<[u32; 4]> = make_map_cells(&world);
        let mut met_count: usize = 0;
        for (k, structure) in world.structures.iter().enumerate() {
            let [u_m, v_m, _] = get_fermi_coordinates(structure.centre, world.r);
            let radius_m: f64 = structure.radius * world.r;
            for (d, &h_m) in (0..CHORD_DIRECTION_COUNT).zip(CHORD_HEIGHTS_M.iter().cycle()) {
                let angle: f64 = std::f64::consts::TAU * d as f64 / CHORD_DIRECTION_COUNT as f64;
                let from_outside = (radius_m + 0.5 * MAP_CELL_REACH_M, MAP_CELL_REACH_M);
                for (start_m, length_m) in [from_outside, (0.5 * radius_m, SHORT_CHORD_M)] {
                    let (du_m, dv_m) = (start_m * angle.cos(), start_m * angle.sin());
                    let x: DVec4 = make_fermi_point(u_m + du_m, v_m + dv_m, h_m, world.r);
                    let v: DVec4 = normalize_tangent(x, structure.centre - x);
                    let meets: bool = (0..=CHORD_SAMPLE_COUNT).any(|i| {
                        let s: f64 = length_m / world.r * i as f64 / CHORD_SAMPLE_COUNT as f64;
                        get_distance(s.cosh() * x + s.sinh() * v, structure.centre)
                            < structure.radius
                    });
                    let word: u32 = cells[get_cell(x, world.r)][1 + k / STRUCTURES_PER_WORD];
                    assert!(
                        !meets || word >> (k % STRUCTURES_PER_WORD) & 1 == 1,
                        "structure {k}: a chord from {x} meets it"
                    );
                    met_count += usize::from(meets);
                }
            }
        }
        assert!(
            met_count > world.structures.len(),
            "only {met_count} chords met a structure"
        );
    }

    #[test]
    fn too_many_structures_give_no_cells() {
        let world: VoxelWorld = load_lunar_surface();
        assert_eq!(
            make_map_cells(&world).len(),
            MAP_CELL_COUNT * MAP_CELL_COUNT
        );
        let bodies: Vec<(DMat4, usize)> = vec![(DMat4::IDENTITY, 0); MAX_MASKED_STRUCTURE_COUNT];
        assert!(make_map_cells(&world.with_avatars(&bodies, &[[1.0, 0.0, 0.0]])).is_empty());
    }
}
