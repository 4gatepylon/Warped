//! `zone_walls.rs` is a map's `regions.zone_walls` (`common/maps/README.md`): the walls that mark
//! the edges of the blue and red zones. Each wall is a slab around the set of points straight above
//! one edge (their Fermi u is the edge's), which in H³ is a geodesic plane standing on the ground:
//! the points within half the wall's thickness of that plane. `render_voxel.metal` draws it from
//! the ground up to the map's height, faintly in the team's colour; it blocks neither players nor
//! lasers.
//!
//! Interface: `ZoneWalls::FromRegions`, `ZoneWalls::to_gpu`, `ZoneWall::get_plane`,
//! `GpuZoneWalls`, `MAX_ZONE_WALL_COUNT`.
//!
//! Example: `let walls = ZoneWalls::FromRegions(regions)?;`

use glam::DVec4;

use crate::map::Regions;
use crate::net::TEAM_NAMES;

/// `Result` is this module's result type, with `VoxelWorld::FromMap`'s errors.
type Result<T> = std::result::Result<T, String>;

pub const MAX_ZONE_WALL_COUNT: usize = 4;
const MIDDLE_TEAM: &str = "middle";

/// `ZoneWall` is one wall: the Fermi u of the edge it stands on, and the team whose zone the edge
/// bounds (an index into `net::TEAM_NAMES`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneWall {
    pub u_m: f64,
    pub team: u32,
}

impl ZoneWall {
    /// `get_plane` gives the unit normal n of the plane straight above the wall's edge, on a map
    /// of curvature radius `r`: a point x is on that plane where ⟨x, n⟩ = 0, and otherwise
    /// ⟨x, n⟩ is the sinh of its distance from it, positive on the side of smaller u.
    ///
    /// Example: `let n = wall.get_plane(40.0);`
    pub fn get_plane(&self, r: f64) -> DVec4 {
        let u: f64 = self.u_m / r;
        DVec4::new(0.0, 0.0, u.cosh(), -u.sinh())
    }
}

/// `GpuZoneWalls` is `render_voxel.metal`'s `ZoneWalls`: per wall its plane's (n_z, -n_w, edge u
/// in meters, team) (the rest zero), then (height, opacity, thickness, the shimmer's clock in s,
/// which `render_voxel.rs` sets each frame), then the boundary's
/// (u, v) semi-axes (0 without a boundary), then (wall count, 0, 0, 0).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuZoneWalls {
    pub walls: [[f32; 4]; MAX_ZONE_WALL_COUNT],
    pub look: [f32; 4],
    pub boundary: [f32; 4],
    pub counts: [i32; 4],
}

/// `ZoneWalls` is a map's zone walls: one per finite edge of each blue or red zone, how high
/// they rise above the ground, how thick they are, how opaque they are at its foot, and the
/// playable boundary's semi-axes (Fermi u, v), which they stop at, if the map has one.
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneWalls {
    pub walls: Vec<ZoneWall>,
    pub height_m: f64,
    pub thickness_m: f64,
    pub opacity_frac: f64,
    pub boundary_m: Option<[f64; 2]>,
}

impl ZoneWalls {
    /// `FromRegions` reads a map's zone walls from its regions: `None` when the map has no
    /// `zone_walls`, else a wall for each finite `u_min_m` and `u_max_m` of each blue or red zone
    /// (the middle zone's edges get none of their own).
    ///
    /// Errors: a zone's team isn't blue, red or middle; the height or the thickness isn't
    /// positive; the opacity isn't between 0 and 1 (both excluded); more than
    /// `MAX_ZONE_WALL_COUNT` walls.
    ///
    /// Example: `let walls = ZoneWalls::FromRegions(map.regions.as_ref().unwrap())?;`
    #[allow(non_snake_case)]
    pub fn FromRegions(regions: &Regions) -> Result<Option<ZoneWalls>> {
        let Some(look) = regions.zone_walls else {
            return Ok(None);
        };
        for (name, value_m) in [
            ("height_m", look.height_m),
            ("thickness_m", look.thickness_m),
        ] {
            if value_m <= 0.0 {
                return Err(format!(
                    "map: regions.zone_walls.{name} is {value_m}, not positive"
                ));
            }
        }
        if !(look.opacity_frac > 0.0 && look.opacity_frac < 1.0) {
            return Err(format!(
                "map: regions.zone_walls.opacity_frac is {}, not between 0 and 1",
                look.opacity_frac
            ));
        }
        let mut walls: Vec<ZoneWall> = Vec::new();
        for zone in regions.zones.iter().flatten() {
            if zone.team == MIDDLE_TEAM {
                continue;
            }
            let team: usize = TEAM_NAMES
                .iter()
                .position(|&name| name == zone.team)
                .ok_or_else(|| {
                    format!(
                        "map: a zone's team is {:?}, not blue, red or middle",
                        zone.team
                    )
                })?;
            for u_m in [zone.u_min_m, zone.u_max_m].into_iter().flatten() {
                walls.push(ZoneWall {
                    u_m,
                    team: team as u32,
                });
            }
        }
        if walls.len() > MAX_ZONE_WALL_COUNT {
            return Err(format!(
                "map: its zones have {} edges with walls, more than {MAX_ZONE_WALL_COUNT}",
                walls.len()
            ));
        }
        Ok(Some(ZoneWalls {
            walls,
            height_m: look.height_m,
            thickness_m: look.thickness_m,
            opacity_frac: look.opacity_frac,
            boundary_m: regions.boundary.as_ref().map(|b| [b.u_m, b.v_m]),
        }))
    }

    /// `to_gpu` gives the walls as `render_voxel.metal`'s `VoxelUniforms` holds them, on a map
    /// of curvature radius `r` (see `GpuZoneWalls`).
    ///
    /// Example: `let gpu_zone_walls = zone_walls.to_gpu(world.r);`
    pub fn to_gpu(&self, r: f64) -> GpuZoneWalls {
        let mut walls: [[f32; 4]; MAX_ZONE_WALL_COUNT] = [[0.0; 4]; MAX_ZONE_WALL_COUNT];
        for (gpu_wall, wall) in walls.iter_mut().zip(&self.walls) {
            let n: DVec4 = wall.get_plane(r);
            *gpu_wall = [n.z, -n.w, wall.u_m, f64::from(wall.team)].map(|x| x as f32);
        }
        let [boundary_u_m, boundary_v_m] = self.boundary_m.unwrap_or([0.0; 2]);
        GpuZoneWalls {
            walls,
            look: [self.height_m, self.opacity_frac, self.thickness_m, 0.0].map(|x| x as f32),
            boundary: [boundary_u_m, boundary_v_m, 0.0, 0.0].map(|x| x as f32),
            counts: [self.walls.len() as i32, 0, 0, 0],
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of reading the zone walls, of where their planes lie, and of the span of a geodesic
    //! inside a wall's slab as `render_voxel.metal`'s `scan_zone_walls` finds it.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | `zone_walls` in the map | absent (no walls), present | `lunar_surface_has_a_wall_per_team`, `maps_without_zone_walls_have_none` |
    //! | A zone's edges | one finite (Lunar Surface's blue and red), both finite, none (both null) | `lunar_surface_has_a_wall_per_team`, `each_finite_edge_of_a_team_zone_gets_a_wall` |
    //! | A zone's team | blue, red, middle (no walls), another name (fails) | `lunar_surface_has_a_wall_per_team`, `bad_zone_walls_fail` |
    //! | Wall count | 0 to `MAX_ZONE_WALL_COUNT`, more (fails) | `each_finite_edge_of_a_team_zone_gets_a_wall`, `bad_zone_walls_fail` |
    //! | Height, thickness and opacity | in range, height or thickness not positive, opacity 0 or 1 (each fails) | `lunar_surface_has_a_wall_per_team`, `bad_zone_walls_fail` |
    //! | Boundary | present (passed on), absent (zeros on the GPU) | `lunar_surface_has_a_wall_per_team`, `each_finite_edge_of_a_team_zone_gets_a_wall` |
    //! | Point against a wall's plane | straight above the edge at any v and height (on it), along the u axis on either side (sinh of the distance, positive at smaller u) | `the_wall_is_straight_above_its_edge` |
    //! | Geodesic against a slab | crossing it (in one face, out the other), dipping in and out of one face, passing by, starting inside, running inside along the plane | `the_slab_span_matches_brute_force` |

    use super::*;
    use crate::geometry::{make_fermi_point, minkowski_dot};
    use crate::map::{Boundary, Map, Zone, ZoneWalls as MapZoneWalls, get_map_folder};

    const R_M: f64 = 40.0;
    const EDGE_U_M: f64 = 45.0;
    const HEIGHT_M: f64 = 8.0;
    const THICKNESS_M: f64 = 0.6;
    const OPACITY_FRAC: f64 = 0.2;
    const TOLERANCE: f64 = 1e-9;
    const RED_TEAM: u32 = 1;
    const BRUTE_FORCE_S_LIMIT: f64 = 3.0;
    const BRUTE_FORCE_STEP_COUNT: i32 = 6000;
    const SPAN_TOLERANCE: f64 = 1e-3;

    /// `make_zone` makes a zone of `team` between `u_min_m` and `u_max_m` (None for unbounded).
    ///
    /// Example: `let zone = make_zone("red", Some(45.0), None);`
    fn make_zone(team: &str, u_min_m: Option<f64>, u_max_m: Option<f64>) -> Zone {
        Zone {
            team: team.to_string(),
            u_min_m,
            u_max_m,
        }
    }

    /// `make_regions` makes regions with `zones`, no boundary, and walls of `height_m`,
    /// `THICKNESS_M` and `opacity_frac`.
    ///
    /// Example: `let regions = make_regions(vec![], 8.0, 0.2);`
    fn make_regions(zones: Vec<Zone>, height_m: f64, opacity_frac: f64) -> Regions {
        Regions {
            zones: Some(zones),
            boundary: None,
            zone_walls: Some(MapZoneWalls {
                height_m,
                thickness_m: THICKNESS_M,
                opacity_frac,
            }),
        }
    }

    /// `get_slab_tanh_span` is `scan_zone_walls`'s span of a geodesic x cosh s + v sinh s inside
    /// the slab |⟨x, n⟩| ≤ k, with a = ⟨x, n⟩ and b = ⟨v, n⟩: the (tanh s) interval where
    /// (b² + k²) τ² + 2ab τ + a² − k² ≤ 0, or `None` when the geodesic stays outside.
    ///
    /// Example: `let span = get_slab_tanh_span(0.1, 0.5, 0.0075);`
    fn get_slab_tanh_span(a: f64, b: f64, k: f64) -> Option<(f64, f64)> {
        let discriminant: f64 = b * b - a * a + k * k;
        if discriminant < 0.0 {
            return None;
        }
        let root: f64 = k * discriminant.sqrt();
        let denominator: f64 = b * b + k * k;
        Some(((-a * b - root) / denominator, (-a * b + root) / denominator))
    }

    #[test]
    fn lunar_surface_has_a_wall_per_team() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).unwrap();
        let regions: &Regions = map.regions.as_ref().unwrap();
        let zone_walls: ZoneWalls = ZoneWalls::FromRegions(regions).unwrap().unwrap();
        let expected: Vec<ZoneWall> = vec![
            ZoneWall {
                u_m: -EDGE_U_M,
                team: 0,
            },
            ZoneWall {
                u_m: EDGE_U_M,
                team: RED_TEAM,
            },
        ];
        assert_eq!(zone_walls.walls, expected);
        assert!(zone_walls.height_m > 0.0 && zone_walls.thickness_m > 0.0);
        assert!(zone_walls.opacity_frac > 0.0 && zone_walls.opacity_frac < 1.0);
        let boundary: &Boundary = regions.boundary.as_ref().unwrap();
        assert_eq!(zone_walls.boundary_m, Some([boundary.u_m, boundary.v_m]));
        let gpu: GpuZoneWalls = zone_walls.to_gpu(R_M);
        assert_eq!(gpu.counts, [2, 0, 0, 0]);
        assert_eq!(gpu.look[2], zone_walls.thickness_m as f32);
        assert_eq!(
            gpu.boundary,
            [boundary.u_m as f32, boundary.v_m as f32, 0.0, 0.0]
        );
        assert_eq!(gpu.walls[1][2..], [EDGE_U_M as f32, RED_TEAM as f32]);
        assert_eq!(gpu.walls[2], [0.0; 4]);
    }

    #[test]
    fn maps_without_zone_walls_have_none() {
        let map: Map = Map::Load(&get_map_folder("house_demo")).unwrap();
        assert!(map.regions.is_none());
        let regions: Regions = Regions {
            zones: Some(vec![make_zone("blue", None, Some(-EDGE_U_M))]),
            boundary: None,
            zone_walls: None,
        };
        assert_eq!(ZoneWalls::FromRegions(&regions).unwrap(), None);
    }

    #[test]
    fn each_finite_edge_of_a_team_zone_gets_a_wall() {
        let zones: Vec<Zone> = vec![
            make_zone("red", Some(-10.0), Some(10.0)),
            make_zone("middle", Some(10.0), Some(EDGE_U_M)),
            make_zone("blue", Some(EDGE_U_M), Some(2.0 * EDGE_U_M)),
            make_zone("blue", None, None),
        ];
        let zone_walls: ZoneWalls =
            ZoneWalls::FromRegions(&make_regions(zones, HEIGHT_M, OPACITY_FRAC))
                .unwrap()
                .unwrap();
        let edges: Vec<(f64, u32)> = zone_walls.walls.iter().map(|w| (w.u_m, w.team)).collect();
        assert_eq!(
            edges,
            [
                (-10.0, RED_TEAM),
                (10.0, RED_TEAM),
                (EDGE_U_M, 0),
                (2.0 * EDGE_U_M, 0)
            ]
        );
        assert_eq!(zone_walls.boundary_m, None);
        let gpu: GpuZoneWalls = zone_walls.to_gpu(R_M);
        assert_eq!(
            gpu.look,
            [
                HEIGHT_M as f32,
                OPACITY_FRAC as f32,
                THICKNESS_M as f32,
                0.0
            ]
        );
        assert_eq!(gpu.boundary, [0.0; 4]);
        assert_eq!(gpu.counts[0], MAX_ZONE_WALL_COUNT as i32);
    }

    #[test]
    fn bad_zone_walls_fail() {
        let one_edge = || vec![make_zone("red", Some(EDGE_U_M), None)];
        let too_many: Vec<Zone> = (0..=MAX_ZONE_WALL_COUNT)
            .map(|i| make_zone("red", Some(i as f64), None))
            .collect();
        let mut thin: Regions = make_regions(one_edge(), HEIGHT_M, OPACITY_FRAC);
        thin.zone_walls.as_mut().unwrap().thickness_m = 0.0;
        let cases: [(Regions, &str); 6] = [
            (make_regions(one_edge(), 0.0, OPACITY_FRAC), "height_m"),
            (thin, "thickness_m"),
            (make_regions(one_edge(), HEIGHT_M, 0.0), "opacity_frac"),
            (make_regions(one_edge(), HEIGHT_M, 1.0), "opacity_frac"),
            (
                make_regions(
                    vec![make_zone("green", None, Some(0.0))],
                    HEIGHT_M,
                    OPACITY_FRAC,
                ),
                "green",
            ),
            (make_regions(too_many, HEIGHT_M, OPACITY_FRAC), "more than"),
        ];
        for (regions, expected) in cases {
            let error: String = ZoneWalls::FromRegions(&regions).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn the_wall_is_straight_above_its_edge() {
        let n: DVec4 = ZoneWall {
            u_m: EDGE_U_M,
            team: 0,
        }
        .get_plane(R_M);
        assert!((minkowski_dot(n, n) - 1.0).abs() < TOLERANCE);
        for v_m in [-60.0, 0.0, 25.0] {
            for h_m in [0.0, 3.0, 40.0] {
                let x: DVec4 = make_fermi_point(EDGE_U_M, v_m, h_m, R_M);
                assert!(minkowski_dot(x, n).abs() < TOLERANCE, "v {v_m} h {h_m}");
            }
        }
        for offset_m in [-30.0, -1.0, 1.0, 30.0] {
            let x: DVec4 = make_fermi_point(EDGE_U_M + offset_m, 0.0, 0.0, R_M);
            let expected: f64 = (-offset_m / R_M).sinh();
            assert!(
                (minkowski_dot(x, n) - expected).abs() < TOLERANCE,
                "{offset_m}"
            );
        }
    }

    #[test]
    fn the_slab_span_matches_brute_force() {
        let k: f64 = (THICKNESS_M / (2.0 * R_M)).sinh();
        let cases: [(&str, f64, f64, bool); 5] = [
            ("crossing", 0.1, -0.5, true),
            ("dipping in", 0.02, -0.0195, true),
            ("passing by", 0.05, -0.01, false),
            ("starting inside", 0.5 * k, 0.3, true),
            ("along the plane", 0.0, 0.0, true),
        ];
        for (name, a, b, is_met) in cases {
            let span: Option<(f64, f64)> = get_slab_tanh_span(a, b, k);
            assert_eq!(span.is_some(), is_met, "{name}");
            for i in -BRUTE_FORCE_STEP_COUNT..=BRUTE_FORCE_STEP_COUNT {
                let s: f64 = BRUTE_FORCE_S_LIMIT * f64::from(i) / f64::from(BRUTE_FORCE_STEP_COUNT);
                let gap: f64 = (a * s.cosh() + b * s.sinh()).abs() - k;
                let tau: f64 = s.tanh();
                let is_in_span: bool = span.is_some_and(|(lo, hi)| tau >= lo && tau <= hi);
                if gap.abs() > SPAN_TOLERANCE * k {
                    assert_eq!(gap < 0.0, is_in_span, "{name} at s {s}");
                }
            }
        }
    }
}
