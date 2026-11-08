//! `terrain.rs` is a map's `ground.terrain` (`common/maps/README.md`): the ground's height over
//! the world's Fermi (u, v), a crater bowl whose walls rise to an elliptical rim, roughened by
//! value noise and raised by round mounds (hills, a ridge, boulders) baked into a height grid,
//! and where a chord first meets it. `render_voxel.metal` computes the same height.
//!
//! Interface: `Terrain::FromMap`, `Terrain::get_height_m`, `Terrain::get_wall_frac`,
//! `Terrain::find_hit_s`, `get_fermi_coordinates`.
//!
//! Example: `let h_m = terrain.get_height_m(12.0, -3.5);`

use std::sync::Arc;

use glam::DVec4;

use crate::map::{MapMound, MapTerrain};

const OCTAVE_COUNT: u32 = 5;
const RIM_NOISE_M: f64 = 60.0;
const RIM_NOISE_FRAC: f64 = 0.5;
const OUTER_FLANK_PER_WALL: f64 = 3.0;
const WALL_RELIEF_GAIN: f64 = 3.0;
const RIM_SEED: u32 = 9;
const STEP_FRAC: f64 = 0.45;
const MIN_STEP_M: f64 = 0.03;
const STEP_PER_M: f64 = 0.01;
const MAX_STEPS: usize = 400;
const BISECTIONS: usize = 8;
const MOUND_CELL_M: f64 = 0.25;

/// `MoundGrid` is the mounds' summed height sampled every `cell_m` from `origin_m` (Fermi u, v)
/// over `size` points (u fastest), read bilinearly; zero outside.
pub struct MoundGrid {
    pub origin_m: [f64; 2],
    pub cell_m: f64,
    pub size: [usize; 2],
    pub heights: Vec<f32>,
}

impl MoundGrid {
    /// `FromMounds` bakes mounds, each h (1 - d²)² within its radius (d the distance over the
    /// radius), into a grid a cell wider than all of them, or `None` if there are none.
    ///
    /// Example: `let grid = MoundGrid::FromMounds(&map_terrain.mounds);`
    #[allow(non_snake_case)]
    pub fn FromMounds(mounds: &[MapMound]) -> Option<MoundGrid> {
        let edge = |a: usize, sign: f64| -> f64 {
            mounds
                .iter()
                .map(|m| sign * ([m.u_m, m.v_m][a] + sign * (m.radius_m + MOUND_CELL_M)))
                .fold(f64::NEG_INFINITY, f64::max)
                * sign
        };
        if mounds.is_empty() {
            return None;
        }
        let origin_m: [f64; 2] = [edge(0, -1.0), edge(1, -1.0)];
        let size: [usize; 2] =
            [0, 1].map(|a| ((edge(a, 1.0) - origin_m[a]) / MOUND_CELL_M).ceil() as usize + 1);
        let mut heights: Vec<f32> = Vec::with_capacity(size[0] * size[1]);
        for j in 0..size[1] {
            for i in 0..size[0] {
                let u_m: f64 = origin_m[0] + i as f64 * MOUND_CELL_M;
                let v_m: f64 = origin_m[1] + j as f64 * MOUND_CELL_M;
                let sum: f64 = mounds
                    .iter()
                    .map(|m| {
                        let d2: f64 =
                            ((u_m - m.u_m).powi(2) + (v_m - m.v_m).powi(2)) / m.radius_m.powi(2);
                        if d2 < 1.0 {
                            m.height_m * (1.0 - d2).powi(2)
                        } else {
                            0.0
                        }
                    })
                    .sum();
                heights.push(sum as f32);
            }
        }
        Some(MoundGrid {
            origin_m,
            cell_m: MOUND_CELL_M,
            size,
            heights,
        })
    }

    /// `get_height_m` reads the grid bilinearly at Fermi (u, v), as `render_voxel.metal` does.
    ///
    /// Example: `let h_m = grid.get_height_m(0.0, 0.0);`
    pub fn get_height_m(&self, u_m: f64, v_m: f64) -> f64 {
        let x: f64 = (u_m - self.origin_m[0]) / self.cell_m;
        let y: f64 = (v_m - self.origin_m[1]) / self.cell_m;
        let (i, j) = (x.floor(), y.floor());
        if i < 0.0 || j < 0.0 || i >= (self.size[0] - 1) as f64 || j >= (self.size[1] - 1) as f64 {
            return 0.0;
        }
        let k: usize = i as usize + self.size[0] * j as usize;
        let at = |d: usize| f64::from(self.heights[k + d]);
        let (fx, fy) = (x - i, y - j);
        let bottom: f64 = at(0) + (at(1) - at(0)) * fx;
        let top: f64 = at(self.size[0]) + (at(self.size[0] + 1) - at(self.size[0])) * fx;
        bottom + (top - bottom) * fy
    }

    /// `get_max_height_m` is the grid's highest sample, which bilinear reads never exceed.
    ///
    /// Example: `let top_m = grid.get_max_height_m();`
    pub fn get_max_height_m(&self) -> f64 {
        f64::from(self.heights.iter().copied().fold(0.0, f32::max))
    }
}

/// `Terrain` is a crater's shape in meters: rim semi-axes along u and v, the wall's width inward
/// from the rim along v, the rim's mean height, and noise of `relief_m` at `wavelength_m`.
#[derive(Clone)]
pub struct Terrain {
    pub rim_m: [f64; 2],
    pub wall_m: f64,
    pub rim_height_m: f64,
    pub relief_m: f64,
    pub wavelength_m: f64,
    pub seed: u32,
    pub mound_grid: Option<Arc<MoundGrid>>,
}

/// `get_fermi_coordinates` gives (u, v, h) of a point in meters, for curvature radius `r`.
///
/// Example: `let [u_m, v_m, h_m] = get_fermi_coordinates(x, 40.0);`
pub fn get_fermi_coordinates(x: DVec4, r: f64) -> [f64; 3] {
    let cosh_h: f64 = (1.0 + x.y * x.y).sqrt();
    [
        -r * (x.z / x.w).atanh(),
        r * (x.x / cosh_h).asinh(),
        r * x.y.asinh(),
    ]
}

/// `hash` gives lattice point (i, j)'s value in [0, 1] for `seed`, as `render_voxel.metal` does.
fn hash(i: i32, j: i32, seed: u32) -> f64 {
    let mut h: u32 = (i as u32).wrapping_mul(0x8da6_b343)
        ^ (j as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    f64::from(h) / f64::from(u32::MAX)
}

/// `smooth` is the cubic smoothstep of t clamped to [0, 1].
fn smooth(t: f64) -> f64 {
    let t: f64 = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `sample_noise` is smooth value noise in [0, 1] at (x, y) lattice units.
fn sample_noise(x: f64, y: f64, seed: u32) -> f64 {
    let (i, j) = (x.floor(), y.floor());
    let (fx, fy) = (smooth(x - i), smooth(y - j));
    let (i, j) = (i as i32, j as i32);
    let bottom: f64 = hash(i, j, seed) + (hash(i + 1, j, seed) - hash(i, j, seed)) * fx;
    let top: f64 = hash(i, j + 1, seed) + (hash(i + 1, j + 1, seed) - hash(i, j + 1, seed)) * fx;
    bottom + (top - bottom) * fy
}

/// `sample_fbm` sums `OCTAVE_COUNT` octaves of value noise, each twice as fine and half as
/// strong, normalized to [0, 1].
fn sample_fbm(x: f64, y: f64, seed: u32) -> f64 {
    let (mut sum, mut amplitude, mut scale) = (0.0, 0.5, 1.0);
    for octave in 0..OCTAVE_COUNT {
        sum += amplitude * sample_noise(x * scale, y * scale, seed.wrapping_add(octave));
        amplitude *= 0.5;
        scale *= 2.0;
    }
    sum / (1.0 - 0.5_f64.powi(OCTAVE_COUNT as i32))
}

impl Terrain {
    /// `FromMap` reads a map's `ground.terrain`.
    ///
    /// Example: `let terrain = map.ground.terrain.as_ref().map(Terrain::FromMap);`
    #[allow(non_snake_case)]
    pub fn FromMap(terrain: &MapTerrain) -> Terrain {
        Terrain {
            rim_m: [terrain.rim_u_m, terrain.rim_v_m],
            wall_m: terrain.wall_m,
            rim_height_m: terrain.rim_height_m,
            relief_m: terrain.relief_m,
            wavelength_m: terrain.relief_wavelength_m,
            seed: terrain.seed,
            mound_grid: MoundGrid::FromMounds(&terrain.mounds).map(Arc::new),
        }
    }

    /// `get_max_height_m` bounds `get_height_m` from above.
    ///
    /// Example: `let top_m = terrain.get_max_height_m();`
    pub fn get_max_height_m(&self) -> f64 {
        self.rim_height_m * (1.0 + RIM_NOISE_FRAC / 2.0)
            + self
                .mound_grid
                .as_ref()
                .map_or(0.0, |g| g.get_max_height_m())
            + self.relief_m * (1.0 + WALL_RELIEF_GAIN)
    }

    /// `get_mounds_height_m` is the mounds' height at (u, v), 0 without mounds.
    ///
    /// Example: `let h_m = terrain.get_mounds_height_m(0.0, 0.0);`
    fn get_mounds_height_m(&self, u_m: f64, v_m: f64) -> f64 {
        self.mound_grid
            .as_ref()
            .map_or(0.0, |g| g.get_height_m(u_m, v_m))
    }

    /// `get_height_m` gives the ground's height at Fermi (u, v): the wall rising from the floor
    /// `wall_m` inside the rim to the rim, falling away outside over `OUTER_FLANK_PER_WALL` walls,
    /// and the relief, rougher up the wall.
    ///
    /// Example: `let h_m = terrain.get_height_m(0.0, 0.0);`
    pub fn get_height_m(&self, u_m: f64, v_m: f64) -> f64 {
        let inside_m: f64 = self.get_inside_m(u_m, v_m);
        let wall: f64 = if inside_m >= 0.0 {
            smooth(1.0 - inside_m / self.wall_m)
        } else {
            1.0 - smooth(-inside_m / (OUTER_FLANK_PER_WALL * self.wall_m))
        };
        let rim_noise: f64 =
            sample_noise(u_m / RIM_NOISE_M, v_m / RIM_NOISE_M, self.seed ^ RIM_SEED);
        let rim_m: f64 = self.rim_height_m * (1.0 + RIM_NOISE_FRAC * (rim_noise - 0.5));
        let relief: f64 = sample_fbm(u_m / self.wavelength_m, v_m / self.wavelength_m, self.seed);
        rim_m * wall
            + self.get_mounds_height_m(u_m, v_m)
            + self.relief_m * relief * (1.0 + WALL_RELIEF_GAIN * wall)
    }

    /// `get_inside_m` gives how far inside the rim (u, v) is, along v's scale: negative outside.
    ///
    /// Example: `let inside_m = terrain.get_inside_m(0.0, 60.0);`
    fn get_inside_m(&self, u_m: f64, v_m: f64) -> f64 {
        let e: f64 = (u_m / self.rim_m[0]).hypot(v_m / self.rim_m[1]);
        (1.0 - e) * self.rim_m[1]
    }

    /// `get_wall_frac` gives how far up the crater's wall (u, v) is, before smoothing: 0 on the
    /// floor and at the wall's foot, 1 at the rim, more outside it.
    ///
    /// Example: `let wall_frac = terrain.get_wall_frac(0.0, 75.0);`
    pub fn get_wall_frac(&self, u_m: f64, v_m: f64) -> f64 {
        (1.0 - self.get_inside_m(u_m, v_m) / self.wall_m).max(0.0)
    }

    /// `find_hit_s` gives where the chord from x along unit tangent v first goes under the ground
    /// within `s_max` (curvature units, radius `r`), or `None`, by stepping a fraction of the gap
    /// to the ground (at least a little more each meter out) and bisecting the step that crosses.
    ///
    /// Example: `let s = terrain.find_hit_s(x, v, 2.0, 40.0);`
    pub fn find_hit_s(&self, x: DVec4, v: DVec4, s_max: f64, r: f64) -> Option<f64> {
        let top_m: f64 = self.get_max_height_m();
        let gap_m = |s: f64| -> f64 {
            let [u_m, v_m, h_m] = get_fermi_coordinates(s.cosh() * x + s.sinh() * v, r);
            h_m - self.get_height_m(u_m, v_m)
        };
        let (mut s, mut previous) = (0.0, 0.0);
        for _ in 0..MAX_STEPS {
            let gap: f64 = gap_m(s);
            if gap < 0.0 {
                let (mut lo, mut hi) = (previous, s);
                for _ in 0..BISECTIONS {
                    let mid: f64 = (lo + hi) / 2.0;
                    if gap_m(mid) < 0.0 { hi = mid } else { lo = mid }
                }
                return Some(hi);
            }
            let y: f64 = s.cosh() * x.y + s.sinh() * v.y;
            let rising: bool = s.sinh() * x.y + s.cosh() * v.y > 0.0;
            if rising && r * y.asinh() > top_m {
                return None;
            }
            previous = s;
            s += (STEP_FRAC * gap).max(MIN_STEP_M + STEP_PER_M * s * r) / r;
            if s > s_max {
                return None;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the terrain's shape and chord hits.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Where on the crater | floor, wall, rim | `the_floor_is_low_and_the_rim_high` |
    //! | Chord | down onto the floor, up into the sky | `a_chord_down_hits_and_one_up_misses` |

    use super::*;

    const R_M: f64 = 40.0;
    const CRATER: Terrain = Terrain {
        rim_m: [140.0, 84.0],
        wall_m: 26.0,
        rim_height_m: 22.0,
        relief_m: 1.0,
        wavelength_m: 24.0,
        seed: 1,
        mound_grid: None,
    };

    #[test]
    fn the_floor_is_low_and_the_rim_high() {
        let floor_m: f64 = CRATER.get_height_m(0.0, 0.0);
        let rim_m: f64 = CRATER.get_height_m(0.0, 84.0);
        assert!((0.0..=1.0).contains(&floor_m), "floor {floor_m}");
        assert!(
            rim_m > 15.0 && rim_m <= CRATER.get_max_height_m(),
            "rim {rim_m}"
        );
    }

    #[test]
    fn a_chord_down_hits_and_one_up_misses() {
        let eye: DVec4 = DVec4::new(0.0, (5.0 / R_M).sinh(), 0.0, (5.0 / R_M).cosh());
        let up_at_eye: DVec4 = DVec4::new(0.0, (5.0 / R_M).cosh(), 0.0, (5.0 / R_M).sinh());
        let down: DVec4 = -0.6 * up_at_eye - 0.8 * DVec4::Z;
        let s: f64 = CRATER.find_hit_s(eye, down, 2.0, R_M).expect("a hit");
        let [u_m, v_m, h_m] = get_fermi_coordinates(s.cosh() * eye + s.sinh() * down, R_M);
        assert!((h_m - CRATER.get_height_m(u_m, v_m)).abs() < 0.05);
        let up: DVec4 = 0.6 * up_at_eye - 0.8 * DVec4::Z;
        assert!(CRATER.find_hit_s(eye, up, 12.0, R_M).is_none());
    }
}
