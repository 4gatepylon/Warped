//! `gravity.rs` is the warps as the pull sees them, and how a level adds the drift the pull gives
//! to a player's walk (W-202, pull toward wormholes and black holes in the game settings): a black
//! hole or a wormhole mouth in flat space (`FlatWarp`) or in H³ (`HyperbolicWarp`), and
//! `add_pull`, which adds the drift to the walk. The pull itself, the drift and the sprint's ramps
//! are in `pull_dynamics.rs`; its numbers are `play_settings::GravitySettings`.
//!
//! Interface: `FlatWarp`, `HyperbolicWarp`, `add_pull`.
//!
//! Example: `if let Some((dir, speed_m_per_s)) = add_pull([0.0, -1.0], 10.0, [drift.x, drift.z]) { ... }`
//!
//! Must know:
//! - Gotchas: with no drift `add_pull` gives the walk itself, bit for bit, so levels move as
//!   before away from every warp (their play traces there stay the same).

use glam::{DVec3, DVec4};

/// `FlatWarp` is a black hole or wormhole mouth in flat space: its centre, the radius of its
/// horizon or throat, and whether it is a mouth.
#[derive(Clone, Copy, Debug)]
pub struct FlatWarp {
    pub centre_m: DVec3,
    pub radius_m: f64,
    pub is_mouth: bool,
}

/// `HyperbolicWarp` is a black hole or wormhole mouth in H³ (the hyperboloid model,
/// `geometry.rs`): its centre, the radius of its horizon or throat, and whether it is a mouth.
#[derive(Clone, Copy, Debug)]
pub struct HyperbolicWarp {
    pub centre: DVec4,
    pub radius_m: f64,
    pub is_mouth: bool,
}

/// `add_pull` adds a drift (`pull_m_per_s`, the velocity the warps' pull has built up:
/// `pull_dynamics::step_drift`) to a walk along the unit direction `walk_dir` (or [0, 0]
/// standing still) at `walk_m_per_s`, both in the same two axes along the ground, and gives the
/// unit direction and speed of the move, or `None` if the player doesn't move. With no pull it
/// gives the walk itself, bit for bit.
///
/// Example: `if let Some((dir, speed_m_per_s)) = add_pull([0.0, -1.0], 10.0, pull_m_per_s) { ... }`
pub fn add_pull(
    walk_dir: [f64; 2],
    walk_m_per_s: f64,
    pull_m_per_s: [f64; 2],
) -> Option<([f64; 2], f64)> {
    let is_walking: bool = walk_dir != [0.0, 0.0];
    if pull_m_per_s == [0.0, 0.0] {
        return is_walking.then_some((walk_dir, walk_m_per_s));
    }
    let x_m_per_s: f64 = walk_dir[0] * walk_m_per_s + pull_m_per_s[0];
    let z_m_per_s: f64 = walk_dir[1] * walk_m_per_s + pull_m_per_s[1];
    let speed_m_per_s: f64 = x_m_per_s.hypot(z_m_per_s);
    (speed_m_per_s > 0.0).then(|| {
        (
            [x_m_per_s / speed_m_per_s, z_m_per_s / speed_m_per_s],
            speed_m_per_s,
        )
    })
}

#[cfg(test)]
mod tests {
    //! Tests of adding a drift to a walk, and of the pull's settings.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Walk and drift | standing, no drift (no move); walking, no drift (the walk, bit for bit); drifting the way of the walk (faster); against it (slower); standing, drifting (moves with the drift) | `walking_with_the_pull` |
    //! | Settings | built in (valid); a radius of 0, a negative strength or braking; a black hole's speed share of 0 or above 1, a darkness above 1; a file with one key (the rest built in), with an unknown key, with a key of the earlier pull (fails) | `settings_are_checked_and_default` |

    use super::*;
    use crate::play_settings::GravitySettings;

    const WALK_M_PER_S: f64 = 10.0;

    #[test]
    fn walking_with_the_pull() {
        let forward: [f64; 2] = [0.0, -1.0];
        assert_eq!(add_pull([0.0, 0.0], WALK_M_PER_S, [0.0, 0.0]), None);
        let walk_dir: [f64; 2] = [0.6, -0.8];
        assert_eq!(
            add_pull(walk_dir, WALK_M_PER_S, [0.0, 0.0]),
            Some((walk_dir, WALK_M_PER_S))
        );
        let pull_m_per_s: f64 = 0.75 * WALK_M_PER_S;
        let toward = add_pull(forward, WALK_M_PER_S, [0.0, -pull_m_per_s]).expect("moves");
        assert_eq!(toward, (forward, WALK_M_PER_S + pull_m_per_s));
        let away = add_pull(forward, WALK_M_PER_S, [0.0, pull_m_per_s]).expect("moves");
        assert_eq!(away, (forward, WALK_M_PER_S - pull_m_per_s));
        let drift = add_pull([0.0, 0.0], WALK_M_PER_S, [pull_m_per_s, 0.0]).expect("drifts");
        assert_eq!(drift, ([1.0, 0.0], pull_m_per_s));
    }

    #[test]
    fn settings_are_checked_and_default() {
        let gravity: GravitySettings = GravitySettings::default();
        assert_eq!(gravity.check(), Ok(()));
        let with = |change: fn(&mut GravitySettings)| {
            let mut changed: GravitySettings = GravitySettings::default();
            change(&mut changed);
            changed.check()
        };
        assert!(with(|g| g.radius_m = 0.0).is_err());
        assert!(with(|g| g.acceleration_m_per_s_per_s = -1.0).is_err());
        assert!(with(|g| g.brake_m_per_s_per_s = -1.0).is_err());
        assert_eq!(with(|g| g.acceleration_m_per_s_per_s = 0.0), Ok(()));
        assert!(with(|g| g.black_hole_speed_frac = 0.0).is_err());
        assert!(with(|g| g.black_hole_speed_frac = 1.5).is_err());
        assert!(with(|g| g.black_hole_darkness_frac = 1.5).is_err());
        assert_eq!(with(|g| g.black_hole_min_radius_m = 0.0), Ok(()));
        let one_key: GravitySettings = serde_yaml::from_str("radius_m: 12.0").expect("reads");
        assert_eq!(
            one_key,
            GravitySettings {
                radius_m: 12.0,
                ..gravity.clone()
            }
        );
        assert!(serde_yaml::from_str::<GravitySettings>("radius_mm: 12.0").is_err());
        assert!(serde_yaml::from_str::<GravitySettings>("reach_m: 24.0").is_err());
    }
}
