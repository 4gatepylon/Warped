//! `warps` gives the map's black holes and wormhole mouths their effect on bodies and lasers: a
//! black hole pulls everything within its reach and kills what crosses its horizon; a wormhole
//! mouth's foil lens bends lasers within its reach, and a body or laser entering its throat comes
//! out of its partner's, through the partner's mirror in z (the hyperbolic demo's gluing).
//!
//! Players' thrown mouths (`thrown_wormholes.rs`) join the map's as more pairs
//! (`Warps::with_thrown_pairs`), sized as the forest's lift mouths, with the same world-fixed
//! mirror: a heuristic that keeps one gluing for the shader and the walkers.
//!
//! Example: `let warps: Warps = Warps::from_map(&map.warps);`

use glam::{DVec3, Mat3, Vec3};
use warped_shared::gravity::FlatWarp;
use warped_shared::laser_drop::{DROP_PIECE_M, turn_flat};
use warped_shared::pull_dynamics::{WarpPulls, get_flat_warp_pulls, step_drift};

use crate::game::bots::BODY_CENTRE_HEIGHT_M;
use crate::game::player::Player;
use crate::map::{self, Warp};
use crate::settings::GravitySettings;

/// `THROAT_RADIUS_PER_SCHWARZSCHILD_RADIUS_FRAC` is how big a mouth's walk-in throat is.
const THROAT_RADIUS_PER_SCHWARZSCHILD_RADIUS_FRAC: f32 = 0.5;
/// `EXIT_MARGIN_M` is how far outside the partner's throat a traveller steps out.
const EXIT_MARGIN_M: f32 = 0.4;
/// `MIRROR` glues a mouth's throat to its partner's (diag(1, 1, -1), as in the hyperbolic demo), so
/// whatever heads −z into one mouth heads −z out of the other.
pub const MIRROR: Vec3 = Vec3::new(1.0, 1.0, -1.0);
/// `LENS_THROAT_PER_THROAT_FRAC` is the lens's throat (where rays cross) per walk-in throat: the
/// hyperbolic demo's ρ = r_s/4, so rays heading within about the walk-in throat cross.
pub const LENS_THROAT_PER_THROAT_FRAC: f32 = 0.5;
/// `LENS_STEP_COUNT` is how many bent steps a laser takes across a mouth's reach.
const LENS_STEP_COUNT: f32 = 32.0;
/// `MAX_BEAM_STEP_COUNT` caps a laser's steps, for one that circles a throat.
const MAX_BEAM_STEP_COUNT: usize = 600;

/// `THROWN_MOUTH_SCHWARZSCHILD_RADIUS_M`, `THROWN_MOUTH_REACH_M` and `THROWN_MOUTH_LAMBDA_SQUARED`
/// size a thrown mouth as the forest's lift mouths (the map's smaller gates), so a corridor holds it.
pub const THROWN_MOUTH_SCHWARZSCHILD_RADIUS_M: f32 = 2.4;
pub const THROWN_MOUTH_REACH_M: f32 = 3.0;
const THROWN_MOUTH_LAMBDA_SQUARED: f32 = 0.25;

/// `BeamPath` is a laser's path: its pieces (straight, or one bent step each), where it stopped,
/// and the mouths whose throats it went into, in order.
#[derive(Clone, Debug)]
pub struct BeamPath {
    pub pieces: Vec<(Vec3, Vec3)>,
    pub end_m: Vec3,
    pub crossed_mouths: Vec<usize>,
}

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
    pub reach_m: f32,
    pub lambda_squared: f32,
    pub partner: usize,
}

impl WormholeMouth {
    /// `get_log_index` gives (d ln n / dρ, ln n) of the mouth's foil lens at `rho_m` from its
    /// centre: the hyperbolic demo's `get_index_slope` and `get_log_index` in flat space, with the
    /// throat at ρ = `throat_radius_m`.
    fn get_log_index(&self, rho_m: f32) -> (f32, f32) {
        let pt: f32 = self.get_lens_throat_m();
        let ir: f32 = 1.0 / rho_m;
        let psi: f32 = 1.0 + pt * ir;
        let ir2: f32 = ir / (psi * psi);
        let rs: f32 = 4.0 * pt;
        let lambda_sq: f32 = self.lambda_squared;
        let slope: f32 = -2.0 * pt * ir * ir / psi
            - rs * psi * (1.0 - pt * ir) * ir2 * ir2 / (2.0 * (1.0 + lambda_sq - rs * ir2));
        let ln_n: f32 =
            2.0 * psi.ln() - 0.5 * ((1.0 - rs * ir2 + lambda_sq) / (1.0 + lambda_sq)).ln();
        (slope, ln_n)
    }

    /// `get_lens_throat_m` is the radius at which the lens carries rays to the partner.
    pub fn get_lens_throat_m(&self) -> f32 {
        self.throat_radius_m * LENS_THROAT_PER_THROAT_FRAC
    }

    /// `get_bend` is ∇ln n of the lens at `x`, tapered by (1 − (ρ/reach)²)² to nothing at the
    /// reach; a ray's direction v turns by (∇ln n − (v·∇ln n) v) per meter.
    ///
    /// Example: `v = (v + (g - g.dot(v) * v) * step_m).normalize();`
    pub fn get_bend(&self, x: Vec3) -> Vec3 {
        let offset_m: Vec3 = x - self.centre_m;
        let rho_m: f32 = offset_m.length().max(1e-4);
        let f: f32 = rho_m / self.reach_m;
        if f >= 1.0 {
            return Vec3::ZERO;
        }
        let window: f32 = (1.0 - f * f) * (1.0 - f * f);
        let window_slope: f32 = -4.0 * f * (1.0 - f * f) / self.reach_m;
        let (slope, ln_n) = self.get_log_index(rho_m);
        offset_m / rho_m * (slope * window + ln_n * window_slope)
    }
}

/// `Warps` is every black hole and wormhole mouth of a map, and the players' thrown pairs after the
/// map's mouths.
#[derive(Clone, Debug, Default)]
pub struct Warps {
    pub black_holes: Vec<BlackHole>,
    pub mouths: Vec<WormholeMouth>,
}

impl Warps {
    /// `with_thrown_pairs` gives these warps with the mouths of `pairs` (each a purple and a green
    /// centre) after their own: pair k's purple at index `self.mouths.len() + 2k`, its green next.
    ///
    /// Example: `let warps: Warps = map_warps.with_thrown_pairs(&[[purple_m, green_m]]);`
    pub fn with_thrown_pairs(&self, pairs: &[[Vec3; 2]]) -> Warps {
        let first: usize = self.mouths.len();
        let mut warps: Warps = self.clone();
        for (pair, centres_m) in pairs.iter().enumerate() {
            for (side, centre_m) in centres_m.iter().enumerate() {
                warps.mouths.push(WormholeMouth {
                    centre_m: *centre_m,
                    throat_radius_m: THROWN_MOUTH_SCHWARZSCHILD_RADIUS_M
                        * THROAT_RADIUS_PER_SCHWARZSCHILD_RADIUS_FRAC,
                    reach_m: THROWN_MOUTH_REACH_M,
                    lambda_squared: THROWN_MOUTH_LAMBDA_SQUARED,
                    partner: first + 2 * pair + 1 - side,
                });
            }
        }
        warps
    }

    /// `is_clear_for_thrown_mouth` says whether a thrown mouth may hover at `centre_m`: its reach
    /// clear of every mouth's here and of every black hole's, and of the reaches of thrown mouths at
    /// `others_m`, so no two lenses overlap.
    ///
    /// Example: `if warps.is_clear_for_thrown_mouth(centre_m, &others_m) { place(); }`
    pub fn is_clear_for_thrown_mouth(&self, centre_m: Vec3, others_m: &[Vec3]) -> bool {
        self.mouths
            .iter()
            .all(|mouth| mouth.centre_m.distance(centre_m) >= mouth.reach_m + THROWN_MOUTH_REACH_M)
            && self
                .black_holes
                .iter()
                .all(|hole| hole.centre_m.distance(centre_m) >= hole.reach_m + THROWN_MOUTH_REACH_M)
            && others_m
                .iter()
                .all(|other_m| other_m.distance(centre_m) >= 2.0 * THROWN_MOUTH_REACH_M)
    }

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
                        reach_m: warp.reach_m,
                        lambda_squared: warp
                            .lambda_squared
                            .expect("a wormhole mouth's foil names its lambda_squared"),
                        partner,
                    });
                }
                other => panic!("warp {:?} has unknown kind {other:?}", warp.name),
            }
        }
        result
    }

    /// `get_walk_pulls` gives what every black hole and mouth, the map's and the linked thrown
    /// ones, does to a body whose middle is at `body_centre_m` (W-202, pull toward wormholes and
    /// black holes in the game settings; `warped_shared::pull_dynamics`), as in every level: a
    /// horizon or a walk-in throat is each warp's radius.
    ///
    /// Example: `let pulls: WarpPulls = warps.get_walk_pulls(&settings.gravity, centre_m);`
    pub fn get_walk_pulls(&self, gravity: &GravitySettings, body_centre_m: Vec3) -> WarpPulls {
        let holes = self.black_holes.iter().map(|hole| FlatWarp {
            centre_m: hole.centre_m.as_dvec3(),
            radius_m: f64::from(hole.horizon_radius_m),
            is_mouth: false,
        });
        let mouths = self.mouths.iter().map(|mouth| FlatWarp {
            centre_m: mouth.centre_m.as_dvec3(),
            radius_m: f64::from(mouth.throat_radius_m),
            is_mouth: true,
        });
        let warps: Vec<FlatWarp> = holes.chain(mouths).collect();
        get_flat_warp_pulls(gravity, body_centre_m.as_dvec3(), DVec3::Y, &warps)
    }

    /// `pull_player` moves `player`'s drift, lift and slowing on by `dt_s` under the warps' pull,
    /// as every level does (W-202): the pull along the ground builds the drift
    /// (`Player::pull_m_per_s`), which is braked out of every warp's reach; a black hole also
    /// lifts, slows the legs (`Player::legs_speed_frac`), holds the player out of its minimum
    /// radius, and darkens the view by how deep the player is (`Player::black_hole_depth_frac`).
    /// The owner calls it before each `Player::update`.
    ///
    /// Example: `warps.pull_player(&settings.gravity, &mut player, 1.0 / 60.0);`
    pub fn pull_player(&self, gravity: &GravitySettings, player: &mut Player, dt_s: f32) {
        let centre_m: Vec3 = player.feet_m + Vec3::Y * BODY_CENTRE_HEIGHT_M;
        let pulls: WarpPulls = self.get_walk_pulls(gravity, centre_m);
        let along_ground: Option<DVec3> = pulls
            .pull_m_per_s_per_s
            .map(|pull| DVec3::new(pull.x, 0.0, pull.z));
        let drift: DVec3 = pulls.hold_out(step_drift(
            player.pull_m_per_s.as_dvec3(),
            along_ground,
            gravity.brake_m_per_s_per_s,
            f64::from(dt_s),
        ));
        player.pull_m_per_s = Vec3::new(drift.x as f32, 0.0, drift.z as f32);
        player.legs_speed_frac = pulls.legs_speed_frac as f32;
        player.black_hole_depth_frac = pulls.black_hole_depth_frac as f32;
        let lift_m_per_s_per_s: f64 = pulls.pull_m_per_s_per_s.map_or(0.0, |pull| pull.y);
        let vertical_m_per_s: f64 =
            f64::from(player.velocity_m_s.y) + lift_m_per_s_per_s * f64::from(dt_s);
        player.velocity_m_s.y = pulls.hold_out(DVec3::Y * vertical_m_per_s).y as f32;
    }

    /// `is_inside_horizon` tells whether `x` has fallen into a black hole.
    ///
    /// Example: `if warps.is_inside_horizon(eye_m) { player.take_damage(f32::MAX); }`
    pub fn is_inside_horizon(&self, x: Vec3) -> bool {
        self.black_holes
            .iter()
            .any(|hole| hole.centre_m.distance(x) < hole.horizon_radius_m)
    }

    /// `cross_throat` carries a point `x` inside mouth `index`'s lens throat, heading `v`, to its
    /// partner: inversion through the throat, then the partner's mirror. It returns the partner's
    /// index, the point (just outside the partner's throat) and the direction.
    ///
    /// Example: `let (index, x, v) = warps.cross_throat(index, x, v);`
    pub fn cross_throat(&self, index: usize, x: Vec3, v: Vec3) -> (usize, Vec3, Vec3) {
        let mouth: &WormholeMouth = &self.mouths[index];
        let partner: &WormholeMouth = &self.mouths[mouth.partner];
        let offset_m: Vec3 = x - mouth.centre_m;
        let rho_m: f32 = offset_m.length().max(1e-4);
        let n: Vec3 = offset_m / rho_m;
        let rho_out_m: f32 = partner.get_lens_throat_m() * mouth.get_lens_throat_m() / rho_m;
        let turned: Vec3 = (v - 2.0 * v.dot(n) * n) * MIRROR;
        (
            mouth.partner,
            partner.centre_m + n * MIRROR * rho_out_m,
            turned,
        )
    }

    /// `get_mouth_exit_feet_m` gives, for a body whose middle is at `body_centre_m` and feet at
    /// `feet_m` inside a mouth's throat, where its feet step out of the partner (as
    /// `cross_throat` carries the point where it came in) and the turn to apply to its velocity
    /// and facing.
    ///
    /// Example: `if let Some((feet_m, turn)) = warps.get_mouth_exit_feet_m(feet_m, centre_m) { ... }`
    pub fn get_mouth_exit_feet_m(&self, feet_m: Vec3, body_centre_m: Vec3) -> Option<(Vec3, Mat3)> {
        let mouth: &WormholeMouth = self
            .mouths
            .iter()
            .find(|mouth| mouth.centre_m.distance(body_centre_m) < mouth.throat_radius_m)?;
        let partner: &WormholeMouth = &self.mouths[mouth.partner];
        let n: Vec3 = (body_centre_m - mouth.centre_m)
            .try_normalize()
            .unwrap_or(Vec3::X);
        let reflection: Mat3 = Mat3::IDENTITY - 2.0 * Mat3::from_cols(n * n.x, n * n.y, n * n.z);
        let turn: Mat3 = Mat3::from_diagonal(MIRROR) * reflection;
        let exit_centre_m: Vec3 =
            partner.centre_m + n * MIRROR * (partner.throat_radius_m + EXIT_MARGIN_M);
        Some((exit_centre_m + (feet_m - body_centre_m), turn))
    }

    /// `get_next_reach_entry_m` is how far a ray from `x` heading `v` goes before it enters a
    /// mouth's reach, if it does.
    fn get_next_reach_entry_m(&self, x: Vec3, v: Vec3) -> Option<f32> {
        self.mouths
            .iter()
            .filter_map(|mouth| {
                let offset_m: Vec3 = x - mouth.centre_m;
                let b: f32 = offset_m.dot(v);
                let discriminant: f32 =
                    b * b - (offset_m.length_squared() - mouth.reach_m * mouth.reach_m);
                let entry_m: f32 = -b - discriminant.max(0.0).sqrt();
                (discriminant > 0.0 && entry_m > 0.0).then_some(entry_m)
            })
            .min_by(f32::total_cmp)
    }

    /// `trace_beam` follows a laser from `origin_m` heading `direction` for `range_m`: straight
    /// outside every mouth's reach (falling by `drop_per_m` per meter in pieces of `DROP_PIECE_M`,
    /// W-157 (bullet drop, by game setting); 0 for straight), bent by its lens inside, and through
    /// a throat to its partner.
    /// `hit(start, direction, length)` gives how far along a straight piece the laser first hits
    /// something, if it does; the laser stops there.
    ///
    /// Example: `let path: BeamPath = warps.trace_beam(eye_m, forward, 200.0, 0.0, |x, v, l| None);`
    pub fn trace_beam(
        &self,
        origin_m: Vec3,
        direction: Vec3,
        range_m: f32,
        drop_per_m: f32,
        mut hit: impl FnMut(Vec3, Vec3, f32) -> Option<f32>,
    ) -> BeamPath {
        let mut pieces: Vec<(Vec3, Vec3)> = Vec::new();
        let mut crossed_mouths: Vec<usize> = Vec::new();
        let mut x: Vec3 = origin_m;
        let mut v: Vec3 = direction.normalize();
        let mut left_m: f32 = range_m;
        for _ in 0..MAX_BEAM_STEP_COUNT {
            if left_m <= 0.0 {
                break;
            }
            let inside: Option<usize> = self
                .mouths
                .iter()
                .position(|mouth| mouth.centre_m.distance(x) < mouth.reach_m);
            let step_m: f32 = match inside {
                Some(index) => {
                    let mouth: &WormholeMouth = &self.mouths[index];
                    let step_m: f32 = (mouth.reach_m / LENS_STEP_COUNT).min(left_m);
                    let g: Vec3 = mouth.get_bend(x);
                    v = (v + (g - g.dot(v) * v) * step_m).normalize();
                    step_m
                }
                None if drop_per_m > 0.0 => self
                    .get_next_reach_entry_m(x, v)
                    .map_or(left_m, |entry_m| entry_m + 0.01)
                    .min(left_m)
                    .min(DROP_PIECE_M as f32),
                None => self
                    .get_next_reach_entry_m(x, v)
                    .map_or(left_m, |entry_m| entry_m + 0.01)
                    .min(left_m),
            };
            if let Some(distance_m) = hit(x, v, step_m) {
                let end_m: Vec3 = x + v * distance_m;
                pieces.push((x, end_m));
                return BeamPath {
                    pieces,
                    end_m,
                    crossed_mouths,
                };
            }
            let next_m: Vec3 = x + v * step_m;
            pieces.push((x, next_m));
            x = next_m;
            left_m -= step_m;
            if inside.is_none() && drop_per_m > 0.0 {
                v = turn_flat(
                    v.as_dvec3(),
                    DVec3::NEG_Y,
                    f64::from(drop_per_m),
                    f64::from(step_m),
                )
                .as_vec3();
            }
            if let Some(index) = inside
                && self.mouths[index].centre_m.distance(x) < self.mouths[index].get_lens_throat_m()
            {
                let (_, crossed_m, crossed_v) = self.cross_throat(index, x, v);
                crossed_mouths.push(index);
                x = crossed_m;
                v = crossed_v.normalize();
            }
        }
        BeamPath {
            pieces,
            end_m: x,
            crossed_mouths,
        }
    }
}

/// `tests` checks the pull, the horizon and the passage through a mouth.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | distance from a black hole | inside reach, outside reach, inside horizon | `test_black_hole_pulls_within_reach_and_kills_inside` |
/// | body vs a mouth | outside throat, inside throat | `test_walking_into_a_mouth_steps_out_beyond_its_partner` |
/// | laser vs a mouth | through its centre, beside it within reach | `test_laser_through_a_mouth_comes_out_of_its_partner`, `test_laser_beside_a_mouth_bends_towards_it` |
/// | thrown pair | walked into (out of its partner), shot into (out of its partner, named) | `test_a_thrown_pair_carries_walkers_and_lasers` |
/// | thrown mouth's place | clear, in a map mouth's reach, in another thrown mouth's reach | `test_thrown_mouths_keep_clear_of_other_reaches` |
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
                    reach_m: 5.0,
                    lambda_squared: 0.25,
                    partner: 1,
                },
                WormholeMouth {
                    centre_m: Vec3::new(-20.0, 1.0, 0.0),
                    throat_radius_m: 1.5,
                    reach_m: 5.0,
                    lambda_squared: 0.25,
                    partner: 0,
                },
            ],
        }
    }

    /// `test_black_hole_pulls_within_reach_and_kills_inside` checks the pull points at the hole
    /// with half the mouths' strength, is none beyond twice the pull's radius, holds a player out
    /// of the minimum radius, and the horizon kills.
    #[test]
    fn test_black_hole_pulls_within_reach_and_kills_inside() {
        let warps: Warps = warps();
        let gravity: GravitySettings = GravitySettings::default();
        let pulls: WarpPulls = warps.get_walk_pulls(&gravity, Vec3::new(6.0, 10.0, 0.0));
        let pull: DVec3 = pulls.pull_m_per_s_per_s.expect("in reach");
        let expected: f64 = -0.5 * gravity.acceleration_m_per_s_per_s;
        assert!(
            (pull.x - expected).abs() < 1e-6 && pull.y.abs() < 1e-6,
            "{pull:?}"
        );
        assert_eq!(pulls.legs_speed_frac, gravity.black_hole_speed_frac);
        assert_eq!(
            warps.get_walk_pulls(&gravity, Vec3::new(11.0, 10.0, 0.0)),
            WarpPulls::NONE
        );
        let mut player: Player = Player::new(Vec3::new(2.0, 10.0 - BODY_CENTRE_HEIGHT_M, 0.0), 0.0);
        player.pull_m_per_s = Vec3::new(-4.0, 0.0, 0.0);
        warps.pull_player(&gravity, &mut player, 1.0 / 60.0);
        assert_eq!(player.pull_m_per_s, Vec3::ZERO);
        assert_eq!(player.black_hole_depth_frac, 1.0);
        assert!(warps.is_inside_horizon(Vec3::new(1.0, 10.0, 0.0)));
        assert!(!warps.is_inside_horizon(Vec3::new(3.0, 10.0, 0.0)));
    }

    /// `test_beams_fall_by_their_drop` checks a beam far from every warp goes straight without a
    /// drop and, with one, falls about a t² / 2 over its length (W-157 (bullet drop, by game
    /// setting)).
    #[test]
    fn test_beams_fall_by_their_drop() {
        const RANGE_M: f32 = 100.0;
        const DROP_M_PER_S_PER_S: f64 = 100.0;
        const BEAM_SPEED_M_PER_S: f64 = 250.0;
        let warps: Warps = warps();
        let start_m: Vec3 = Vec3::new(0.0, 50.0, -100.0);
        let straight: BeamPath =
            warps.trace_beam(start_m, Vec3::NEG_Z, RANGE_M, 0.0, |_, _, _| None);
        assert_eq!(straight.end_m, start_m + Vec3::NEG_Z * RANGE_M);
        let drop_per_m: f32 = warped_shared::laser_drop::get_drop_curvature_per_m(
            DROP_M_PER_S_PER_S,
            BEAM_SPEED_M_PER_S,
        ) as f32;
        let falling: BeamPath =
            warps.trace_beam(start_m, Vec3::NEG_Z, RANGE_M, drop_per_m, |_, _, _| None);
        let t_s: f32 = RANGE_M / BEAM_SPEED_M_PER_S as f32;
        let expected_m: f32 = 0.5 * DROP_M_PER_S_PER_S as f32 * t_s * t_s;
        let fallen_m: f32 = start_m.y - falling.end_m.y;
        assert!(
            (fallen_m - expected_m).abs() < 0.15 * expected_m,
            "{fallen_m} vs {expected_m}"
        );
        assert!(falling.pieces.len() > straight.pieces.len());
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
        let (exit_m, turn) = warps
            .get_mouth_exit_feet_m(feet_m, feet_m + Vec3::Y)
            .expect("inside the throat");
        assert!(
            (exit_m - Vec3::new(-18.1, 0.0, 0.0)).length() < 1e-4,
            "{exit_m:?}"
        );
        assert!((turn * Vec3::NEG_X - Vec3::X).length() < 1e-4);
    }

    /// `test_laser_through_a_mouth_comes_out_of_its_partner` checks a laser fired at mouth 0's
    /// centre from +x leaves mouth 1 on its +x side, heading +x.
    #[test]
    fn test_laser_through_a_mouth_comes_out_of_its_partner() {
        let warps: Warps = warps();
        let path: BeamPath = warps.trace_beam(
            Vec3::new(30.0, 1.0, 0.0),
            Vec3::NEG_X,
            30.0,
            0.0,
            |_, _, _| None,
        );
        let (start_m, end_m) = *path.pieces.last().expect("a path");
        assert!(
            path.end_m.x > -18.0 && path.end_m.x < 10.0,
            "{:?}",
            path.end_m
        );
        assert!((end_m - start_m).normalize().x > 0.99);
    }

    /// `test_laser_beside_a_mouth_bends_towards_it` checks a laser passing 4.2 m from mouth 0's
    /// centre leaves its reach turned towards it, without crossing.
    #[test]
    fn test_laser_beside_a_mouth_bends_towards_it() {
        let warps: Warps = warps();
        let path: BeamPath = warps.trace_beam(
            Vec3::new(30.0, 1.0, 4.2),
            Vec3::NEG_X,
            20.0,
            0.0,
            |_, _, _| None,
        );
        let (start_m, end_m) = *path.pieces.last().expect("a path");
        let direction: Vec3 = (end_m - start_m).normalize();
        assert!(
            direction.z < -0.02 && path.end_m.x > 5.0,
            "{direction:?} {:?}",
            path.end_m
        );
    }

    /// `test_a_thrown_pair_carries_walkers_and_lasers` checks a body walking into a thrown purple
    /// mouth steps out beyond its green partner, and a laser fired into it comes out of the green
    /// one, naming the purple mouth it entered.
    #[test]
    fn test_a_thrown_pair_carries_walkers_and_lasers() {
        let purple_m: Vec3 = Vec3::new(0.0, 1.7, 20.0);
        let green_m: Vec3 = Vec3::new(10.0, 1.7, 40.0);
        let warps: Warps = warps().with_thrown_pairs(&[[purple_m, green_m]]);
        assert_eq!((warps.mouths[2].partner, warps.mouths[3].partner), (3, 2));
        let feet_m: Vec3 = purple_m + Vec3::new(0.0, -1.7, 0.5);
        let (exit_m, _) = warps
            .get_mouth_exit_feet_m(feet_m, feet_m + Vec3::Y * 0.9)
            .expect("inside the purple throat");
        assert!(exit_m.distance(green_m) < 3.0, "{exit_m:?}");
        let path: BeamPath = warps.trace_beam(
            purple_m + Vec3::Z * 10.0,
            Vec3::NEG_Z,
            30.0,
            0.0,
            |_, _, _| None,
        );
        assert_eq!(path.crossed_mouths, [2]);
        assert!(
            (path.end_m.x - green_m.x).abs() < 0.5 && path.end_m.z < green_m.z,
            "{:?}",
            path.end_m
        );
    }

    /// `test_thrown_mouths_keep_clear_of_other_reaches` checks a thrown mouth is placed in the
    /// open, but not within a map mouth's reach plus its own, nor within two reaches of another
    /// thrown mouth.
    #[test]
    fn test_thrown_mouths_keep_clear_of_other_reaches() {
        let warps: Warps = warps();
        let open_m: Vec3 = Vec3::new(0.0, 1.7, 30.0);
        assert!(warps.is_clear_for_thrown_mouth(open_m, &[Vec3::new(0.0, 1.7, 20.0)]));
        assert!(!warps.is_clear_for_thrown_mouth(Vec3::new(14.0, 1.7, 0.0), &[]));
        assert!(!warps.is_clear_for_thrown_mouth(open_m, &[Vec3::new(0.0, 1.7, 25.0)]));
    }
}
