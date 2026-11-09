//! `movement.rs` is the rules of moving that every level plays alike (W-126, same play in every
//! level), apart from the space it moves in: crouching (its share of the way down), sprint energy
//! (draining while sprinting, refilling faster while crouched), the head bob's steps, and how much
//! a hit and crouching slow a player. A level turns the speed this gives into motion in its own
//! space, with its own collisions (the `VoxelPlayer::step` of Lunar Surface, Dust, Snow Planet
//! and the match, and City Sphere's `CityWorld::move_player`).
//!
//! Interface: `MoveState` (`step`), `MoveInput`, `MoveRules`.
//!
//! Example: `let slow_frac: f64 = state.step(&input, &rules, 1.0 / 60.0);`
//!
//! Must know:
//! - Gotchas: `step` keeps the order of the levels' own arithmetic, so moving it here changed no
//!   player's path by a bit; keep it so, or check the levels' play traces.

use crate::play_settings::{
    HeadBobSettings, MovementSettings, SprintEnergySettings, WhenHitSettings,
};

/// `MoveState` is what the rules keep between steps: how far down a crouch is (0 standing, 1
/// crouched), the sprint energy (0 to 1), whether the player sprints, the head bob's phase and
/// how much of it shows (0 to 1), and how long a hit still slows the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveState {
    pub crouch_frac: f64,
    pub energy_frac: f64,
    pub is_sprinting: bool,
    pub bob_phase_rad: f64,
    pub bob_frac: f64,
    pub slowed_s: f64,
}

/// `MoveInput` is one step's movement keys as the rules need them (a move key held, `Shift` to
/// sprint, `Ctrl` to crouch), and where the player is: flying, or standing on the ground.
#[derive(Clone, Copy, Debug)]
pub struct MoveInput {
    pub is_moving: bool,
    pub is_sprint_held: bool,
    pub is_crouch_held: bool,
    pub is_flying: bool,
    pub is_on_ground: bool,
}

/// `MoveRules` is the settings the rules follow: movement and sprint energy, how a hit slows,
/// and the head bob.
#[derive(Clone, Copy)]
pub struct MoveRules<'a> {
    pub movement: &'a MovementSettings,
    pub energy: &'a SprintEnergySettings,
    pub when_hit: &'a WhenHitSettings,
    pub head_bob: &'a HeadBobSettings,
}

impl MoveState {
    /// `step` moves the rules on by `dt_s`: the crouch toward held or not (over
    /// `movement.crouch_s`), sprinting (held with a move key, not crouched, while there is
    /// energy) and the energy, the head bob's steps (while walking on the ground), and the slow
    /// after a hit counting down. It gives the share of full speed this step allows: a hit's
    /// `when_hit.slow_speed_frac` while slowed, times `movement.crouch_speed_frac` while crouched
    /// on foot.
    ///
    /// Example: `let slow_frac: f64 = state.step(&input, &rules, 1.0 / 60.0);`
    pub fn step(&mut self, input: &MoveInput, rules: &MoveRules, dt_s: f64) -> f64 {
        let (energy, bob) = (rules.energy, rules.head_bob);
        let crouch_step: f64 = dt_s / rules.movement.crouch_s;
        let crouch_target: f64 = f64::from(u8::from(input.is_crouch_held));
        self.crouch_frac += (crouch_target - self.crouch_frac).clamp(-crouch_step, crouch_step);
        let wants_to_sprint: bool =
            input.is_sprint_held && !input.is_crouch_held && input.is_moving;
        self.is_sprinting = wants_to_sprint && self.energy_frac > 0.0;
        let energy_change_per_s: f64 = match (wants_to_sprint, input.is_crouch_held) {
            (true, _) => -1.0 / energy.drain_s,
            (false, true) => 1.0 / energy.refill_crouching_s,
            (false, false) => 1.0 / energy.refill_walking_s,
        };
        self.energy_frac = (self.energy_frac + energy_change_per_s * dt_s).clamp(0.0, 1.0);
        let is_walking: bool = input.is_moving && !input.is_flying && input.is_on_ground;
        let bob_target: f64 = f64::from(u8::from(is_walking));
        self.bob_frac += (bob_target - self.bob_frac).clamp(-dt_s / bob.fade_s, dt_s / bob.fade_s);
        if is_walking {
            let hz: f64 = if self.is_sprinting {
                bob.sprint_steps_per_s
            } else {
                bob.walk_steps_per_s
            };
            self.bob_phase_rad =
                (self.bob_phase_rad + std::f64::consts::TAU * hz * dt_s) % std::f64::consts::TAU;
        }
        let slow_frac: f64 = if self.slowed_s > 0.0 {
            rules.when_hit.slow_speed_frac
        } else {
            1.0
        } * if input.is_crouch_held && !input.is_flying {
            rules.movement.crouch_speed_frac
        } else {
            1.0
        };
        self.slowed_s = (self.slowed_s - dt_s).max(0.0);
        slow_frac
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the movement rules.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Sprint key | held while moving (sprints while there is energy, drains it over `drain_s`), held standing still (no sprint), released (refills) | `sprint_drains_and_refills` |
    //! | Crouch key | held (crouches over `crouch_s`, refills energy faster, slows to `crouch_speed_frac`), released (stands) | `crouching_takes_its_time_and_slows`, `sprint_drains_and_refills` |
    //! | Where | on the ground (the head bob steps), in the air or flying (no steps, no crouch slow when flying) | `bob_steps_only_on_the_ground` |
    //! | After a hit | slowed (for `slow_s`), not | `a_hit_slows_for_a_while` |

    use super::*;
    use serde::Deserialize;

    const LUNAR_SERVER_YAML: &str =
        include_str!("../../hyperbolic_wormholes_rust/server_settings.yaml");
    const LUNAR_PLAYER_YAML: &str =
        include_str!("../../hyperbolic_wormholes_rust/player_settings.yaml");
    const STEP_S: f64 = 1.0 / 60.0;
    const STILL: MoveState = MoveState {
        crouch_frac: 0.0,
        energy_frac: 1.0,
        is_sprinting: false,
        bob_phase_rad: 0.0,
        bob_frac: 0.0,
        slowed_s: 0.0,
    };

    /// `ServerSettings` is the movement sections of Lunar Surface's `server_settings.yaml`.
    #[derive(Deserialize)]
    struct ServerSettings {
        movement: MovementSettings,
        sprint_energy: SprintEnergySettings,
        when_hit: WhenHitSettings,
    }

    /// `PlayerView` is the head bob of Lunar Surface's `player_settings.yaml`.
    #[derive(Deserialize)]
    struct PlayerView {
        head_bob: HeadBobSettings,
    }

    /// `PlayerFile` is the view of Lunar Surface's `player_settings.yaml`.
    #[derive(Deserialize)]
    struct PlayerFile {
        view: PlayerView,
    }

    /// `read_settings` reads Lunar Surface's built-in movement settings and head bob.
    fn read_settings() -> (ServerSettings, HeadBobSettings) {
        let server: ServerSettings =
            serde_yaml::from_str(LUNAR_SERVER_YAML).expect("Lunar Surface's server settings");
        let player: PlayerFile =
            serde_yaml::from_str(LUNAR_PLAYER_YAML).expect("Lunar Surface's player settings");
        (server, player.view.head_bob)
    }

    /// `walking` is a move key held on the ground, with sprint and crouch as given.
    fn walking(is_sprint_held: bool, is_crouch_held: bool) -> MoveInput {
        MoveInput {
            is_moving: true,
            is_sprint_held,
            is_crouch_held,
            is_flying: false,
            is_on_ground: true,
        }
    }

    /// `run_for` steps `state` for `duration_s` with `input`, giving the last step's slow share.
    fn run_for(
        state: &mut MoveState,
        input: &MoveInput,
        rules: &MoveRules,
        duration_s: f64,
    ) -> f64 {
        let mut slow_frac: f64 = 1.0;
        for _ in 0..(duration_s / STEP_S).round() as usize {
            slow_frac = state.step(input, rules, STEP_S);
        }
        slow_frac
    }

    #[test]
    fn sprint_drains_and_refills() {
        let (server, head_bob) = read_settings();
        let rules: MoveRules = MoveRules {
            movement: &server.movement,
            energy: &server.sprint_energy,
            when_hit: &server.when_hit,
            head_bob: &head_bob,
        };
        let mut state: MoveState = STILL;
        run_for(
            &mut state,
            &walking(true, false),
            &rules,
            server.sprint_energy.drain_s / 2.0,
        );
        assert!(state.is_sprinting && (state.energy_frac - 0.5).abs() < 1e-9);
        run_for(
            &mut state,
            &walking(true, false),
            &rules,
            server.sprint_energy.drain_s,
        );
        assert!(
            !state.is_sprinting && state.energy_frac == 0.0,
            "out of energy"
        );
        let standing: MoveInput = MoveInput {
            is_moving: false,
            ..walking(true, false)
        };
        run_for(
            &mut state,
            &standing,
            &rules,
            server.sprint_energy.refill_walking_s / 2.0,
        );
        assert!(!state.is_sprinting && (state.energy_frac - 0.5).abs() < 1e-9);
        run_for(
            &mut state,
            &walking(false, true),
            &rules,
            server.sprint_energy.refill_crouching_s / 2.0,
        );
        assert!(
            (state.energy_frac - 1.0).abs() < 1e-9,
            "crouching refills faster"
        );
    }

    #[test]
    fn crouching_takes_its_time_and_slows() {
        let (server, head_bob) = read_settings();
        let rules: MoveRules = MoveRules {
            movement: &server.movement,
            energy: &server.sprint_energy,
            when_hit: &server.when_hit,
            head_bob: &head_bob,
        };
        let mut state: MoveState = STILL;
        let slow_frac: f64 = run_for(
            &mut state,
            &walking(false, true),
            &rules,
            server.movement.crouch_s / 2.0,
        );
        assert!((state.crouch_frac - 0.5).abs() < 0.1);
        assert_eq!(slow_frac, server.movement.crouch_speed_frac);
        run_for(
            &mut state,
            &walking(false, true),
            &rules,
            server.movement.crouch_s,
        );
        assert_eq!(state.crouch_frac, 1.0);
        let slow_frac: f64 = run_for(
            &mut state,
            &walking(false, false),
            &rules,
            server.movement.crouch_s * 2.0,
        );
        assert_eq!((state.crouch_frac, slow_frac), (0.0, 1.0));
        let flying: MoveInput = MoveInput {
            is_flying: true,
            ..walking(false, true)
        };
        assert_eq!(
            state.step(&flying, &rules, STEP_S),
            1.0,
            "no crouch slow when flying"
        );
    }

    #[test]
    fn bob_steps_only_on_the_ground() {
        let (server, head_bob) = read_settings();
        let rules: MoveRules = MoveRules {
            movement: &server.movement,
            energy: &server.sprint_energy,
            when_hit: &server.when_hit,
            head_bob: &head_bob,
        };
        let mut state: MoveState = STILL;
        run_for(
            &mut state,
            &walking(false, false),
            &rules,
            head_bob.fade_s * 2.0,
        );
        assert_eq!(state.bob_frac, 1.0);
        assert!(state.bob_phase_rad > 0.0);
        let in_the_air: MoveInput = MoveInput {
            is_on_ground: false,
            ..walking(false, false)
        };
        let phase_rad: f64 = state.bob_phase_rad;
        run_for(&mut state, &in_the_air, &rules, head_bob.fade_s * 2.0);
        assert_eq!((state.bob_frac, state.bob_phase_rad), (0.0, phase_rad));
    }

    #[test]
    fn a_hit_slows_for_a_while() {
        let (server, head_bob) = read_settings();
        let rules: MoveRules = MoveRules {
            movement: &server.movement,
            energy: &server.sprint_energy,
            when_hit: &server.when_hit,
            head_bob: &head_bob,
        };
        let mut state: MoveState = MoveState {
            slowed_s: server.when_hit.slow_s,
            ..STILL
        };
        let slow_frac: f64 = run_for(
            &mut state,
            &walking(false, false),
            &rules,
            server.when_hit.slow_s / 2.0,
        );
        assert_eq!(slow_frac, server.when_hit.slow_speed_frac);
        let slow_frac: f64 = run_for(
            &mut state,
            &walking(false, false),
            &rules,
            server.when_hit.slow_s,
        );
        assert_eq!((slow_frac, state.slowed_s), (1.0, 0.0));
    }
}
