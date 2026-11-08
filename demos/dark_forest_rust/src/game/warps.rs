//! `warps` gives the map's black holes and wormhole mouths their effect on bodies: a black hole
//! pulls everything within its reach and kills what crosses its horizon, and walking into a
//! wormhole mouth's throat steps out of its partner on the far side, still moving the same way.
//!
//! Example: `let warps: Warps = Warps::from_map(&map.warps);`

use glam::Vec3;

use crate::map::{self, Warp};

/// `HORIZON_PULL_M_PER_S2` is a black hole's pull at its horizon, before the taper to its reach.
const HORIZON_PULL_M_PER_S2: f32 = 30.0;
/// `THROAT_RADIUS_PER_SCHWARZSCHILD_RADIUS_FRAC` is how big a mouth's walk-in throat is.
const THROAT_RADIUS_PER_SCHWARZSCHILD_RADIUS_FRAC: f32 = 0.5;
/// `EXIT_MARGIN_M` is how far outside the partner's throat a traveller steps out.
const EXIT_MARGIN_M: f32 = 0.4;

/// `BlackHole` is one black hole: its centre, horizon and reach.
#[derive(Clone, Debug)]
pub struct BlackHole {
    pub centre_m: Vec3,
    pub horizon_radius_m: f32,
    pub reach_m: f32,
}

/// `WormholeMouth` is one end of a wormhole; `partner` indexes the other end in `Warps::mouths`.
#[derive(Clone, Debug)]
pub struct WormholeMouth {
    pub centre_m: Vec3,
    pub throat_radius_m: f32,
    pub partner: usize,
}

/// `Warps` is every black hole and wormhole mouth of a map.
#[derive(Clone, Debug, Default)]
pub struct Warps {
    pub black_holes: Vec<BlackHole>,
    pub mouths: Vec<WormholeMouth>,
}

impl Warps {
    /// `from_map` reads the map's warps; a mouth's `partner` must be another mouth.
    ///
    /// Example: `let warps: Warps = Warps::from_map(&map.warps);`
    pub fn from_map(warps: &[Warp]) -> Warps {
        let mouth_warp_indices: Vec<usize> = (0..warps.len())
            .filter(|index| warps[*index].kind == "wormhole_mouth")
            .collect();
        let mut result: Warps = Warps::default();
        for warp in warps {
            let centre_m: Vec3 = map::frame_translation_m(&warp.frame);
            match warp.kind.as_str() {
                "black_hole" => result.black_holes.push(BlackHole {
                    centre_m,
                    horizon_radius_m: warp.schwarzschild_radius_m,
                    reach_m: warp.reach_m,
                }),
                "wormhole_mouth" => {
                    let partner_warp_index: usize =
                        warp.partner.expect("a wormhole mouth names its partner");
                    let partner: usize = mouth_warp_indices
                        .iter()
                        .position(|index| *index == partner_warp_index)
                        .expect("a wormhole mouth's partner is a wormhole mouth");
                    result.mouths.push(WormholeMouth {
                        centre_m,
                        throat_radius_m: warp.schwarzschild_radius_m
                            * THROAT_RADIUS_PER_SCHWARZSCHILD_RADIUS_FRAC,
                        partner,
                    });
                }
                other => panic!("warp {:?} has unknown kind {other:?}", warp.name),
            }
        }
        result
    }

    /// `get_pull_m_per_s2` is the black holes' pull at `x`: inverse square from the horizon,
    /// tapering to nothing at each hole's reach.
    ///
    /// Example: `velocity_m_s += warps.get_pull_m_per_s2(x) * dt_s;`
    pub fn get_pull_m_per_s2(&self, x: Vec3) -> Vec3 {
        self.black_holes
            .iter()
            .map(|hole| {
                let offset_m: Vec3 = hole.centre_m - x;
                let r_m: f32 = offset_m.length();
                if r_m >= hole.reach_m || r_m < hole.horizon_radius_m * 0.1 {
                    return Vec3::ZERO;
                }
                let taper_frac: f32 = 1.0 - r_m / hole.reach_m;
                offset_m / r_m
                    * HORIZON_PULL_M_PER_S2
                    * (hole.horizon_radius_m / r_m).powi(2)
                    * taper_frac
            })
            .sum()
    }

    /// `is_inside_horizon` tells whether `x` has fallen into a black hole.
    ///
    /// Example: `if warps.is_inside_horizon(eye_m) { player.take_damage(f32::MAX); }`
    pub fn is_inside_horizon(&self, x: Vec3) -> bool {
        self.black_holes
            .iter()
            .any(|hole| hole.centre_m.distance(x) < hole.horizon_radius_m)
    }

    /// `get_mouth_exit_feet_m` is where a body whose middle is at `body_centre_m` and feet at
    /// `feet_m` steps out if it is inside a mouth's throat: across the partner from where it came
    /// in, so it keeps walking away from the partner.
    ///
    /// Example: `if let Some(exit_m) = warps.get_mouth_exit_feet_m(feet_m, centre_m) { ... }`
    pub fn get_mouth_exit_feet_m(&self, feet_m: Vec3, body_centre_m: Vec3) -> Option<Vec3> {
        let mouth: &WormholeMouth = self
            .mouths
            .iter()
            .find(|mouth| mouth.centre_m.distance(body_centre_m) < mouth.throat_radius_m)?;
        let partner: &WormholeMouth = &self.mouths[mouth.partner];
        let came_from: Vec3 = (body_centre_m - mouth.centre_m)
            .with_y(0.0)
            .try_normalize()
            .unwrap_or(Vec3::X);
        let exit_m: Vec3 = partner.centre_m - came_from * (partner.throat_radius_m + EXIT_MARGIN_M);
        Some(exit_m.with_y(feet_m.y + partner.centre_m.y - mouth.centre_m.y))
    }
}

/// `tests` checks the pull, the horizon and the passage through a mouth.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | distance from a black hole | inside reach, outside reach, inside horizon | `test_black_hole_pulls_within_reach_and_kills_inside` |
/// | body vs a mouth | outside throat, inside throat | `test_walking_into_a_mouth_steps_out_beyond_its_partner` |
#[cfg(test)]
mod tests {
    use super::*;

    fn warps() -> Warps {
        Warps {
            black_holes: vec![BlackHole {
                centre_m: Vec3::new(0.0, 10.0, 0.0),
                horizon_radius_m: 2.0,
                reach_m: 8.0,
            }],
            mouths: vec![
                WormholeMouth {
                    centre_m: Vec3::new(20.0, 1.0, 0.0),
                    throat_radius_m: 1.5,
                    partner: 1,
                },
                WormholeMouth {
                    centre_m: Vec3::new(-20.0, 1.0, 0.0),
                    throat_radius_m: 1.5,
                    partner: 0,
                },
            ],
        }
    }

    /// `test_black_hole_pulls_within_reach_and_kills_inside` checks the pull points at the hole,
    /// is zero beyond reach, and the horizon kills.
    #[test]
    fn test_black_hole_pulls_within_reach_and_kills_inside() {
        let warps: Warps = warps();
        let pull: Vec3 = warps.get_pull_m_per_s2(Vec3::new(4.0, 10.0, 0.0));
        assert!(pull.x < -1.0 && pull.y.abs() < 1e-4, "{pull:?}");
        assert_eq!(
            warps.get_pull_m_per_s2(Vec3::new(9.0, 10.0, 0.0)),
            Vec3::ZERO
        );
        assert!(warps.is_inside_horizon(Vec3::new(1.0, 10.0, 0.0)));
        assert!(!warps.is_inside_horizon(Vec3::new(3.0, 10.0, 0.0)));
    }

    /// `test_walking_into_a_mouth_steps_out_beyond_its_partner` checks a body entering mouth 0
    /// from +x steps out on the -x side of mouth 1, outside its throat.
    #[test]
    fn test_walking_into_a_mouth_steps_out_beyond_its_partner() {
        let warps: Warps = warps();
        let outside_feet_m: Vec3 = Vec3::new(22.0, 0.0, 0.0);
        assert!(
            warps
                .get_mouth_exit_feet_m(outside_feet_m, outside_feet_m + Vec3::Y)
                .is_none()
        );
        let feet_m: Vec3 = Vec3::new(21.0, 0.0, 0.0);
        let exit_m: Vec3 = warps
            .get_mouth_exit_feet_m(feet_m, feet_m + Vec3::Y)
            .expect("inside the throat");
        assert!(
            (exit_m - Vec3::new(-21.9, 0.0, 0.0)).length() < 1e-4,
            "{exit_m:?}"
        );
    }
}
