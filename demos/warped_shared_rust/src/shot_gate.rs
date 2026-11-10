//! `shot_gate.rs` is when a player may shoot after moving, the same in every level and for bots
//! (W-147 (shooting while moving, by game setting)): a move the game's `shooting` settings don't
//! allow shooting during (jumping, walking upright, sprinting) holds a shot until
//! `movement.shoot_after_moving_s` after it ends; crouched walking, standing and falling never
//! hold one.
//!
//! Interface: `ShotMoves`, `ShotGate` (`New`, `note_moves`, `is_open`).
//!
//! Example: `gate.note_moves(&moves, &settings.shooting, dt_s); if gate.is_open(&settings.movement) { ... }`

use crate::play_settings::{MovementSettings, ShootingSettings};

/// `ShotMoves` is what a player does this step that may hold a shot: jumping, walking upright
/// (moving, not crouched) and, while walking upright, sprinting.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShotMoves {
    pub is_jumping: bool,
    pub is_walking_upright: bool,
    pub is_sprinting: bool,
}

impl ShotMoves {
    /// `is_holding_shot` says whether these moves hold a shot under `shooting`: a jump unless
    /// shooting while jumping is on, a sprint unless shooting while sprinting is on, and other
    /// upright walking unless shooting while walking is on.
    ///
    /// Example: `if moves.is_holding_shot(&settings.shooting) { ... }`
    pub fn is_holding_shot(&self, shooting: &ShootingSettings) -> bool {
        let is_sprinting: bool = self.is_walking_upright && self.is_sprinting;
        let is_walking: bool = self.is_walking_upright && !self.is_sprinting;
        (self.is_jumping && !shooting.can_shoot_while_jumping)
            || (is_sprinting && !shooting.can_shoot_while_sprinting)
            || (is_walking && !shooting.can_shoot_while_walking)
    }
}

/// `ShotGate` is how long ago a move last held a shot (`ShotMoves::is_holding_shot`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotGate {
    pub since_holding_move_s: f64,
}

impl ShotGate {
    /// `New` is a gate no move has held yet, open at once.
    ///
    /// Example: `let gate = ShotGate::New();`
    #[allow(non_snake_case)]
    pub fn New() -> ShotGate {
        ShotGate {
            since_holding_move_s: f64::INFINITY,
        }
    }

    /// `note_moves` counts `dt_s` more since a holding move, or starts again from 0 when `moves`
    /// hold a shot under `shooting`.
    ///
    /// Example: `gate.note_moves(&moves, &settings.shooting, 1.0 / 60.0);`
    pub fn note_moves(&mut self, moves: &ShotMoves, shooting: &ShootingSettings, dt_s: f64) {
        self.since_holding_move_s = if moves.is_holding_shot(shooting) {
            0.0
        } else {
            self.since_holding_move_s + dt_s
        };
    }

    /// `is_open` says whether a shot may go now: `movement.shoot_after_moving_s` or more since a
    /// holding move.
    ///
    /// Example: `if gate.is_open(&settings.movement) { ... }`
    pub fn is_open(&self, movement: &MovementSettings) -> bool {
        self.since_holding_move_s >= movement.shoot_after_moving_s
    }
}

#[cfg(test)]
mod tests {
    //! Tests of when moving holds a shot.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Move | none, crouched walking, walking upright, sprinting, jumping, sprinting while jumping | `each_move_holds_a_shot_unless_its_switch_is_on` |
    //! | Switches | all off (the default), each one on alone, all on | `each_move_holds_a_shot_unless_its_switch_is_on` |
    //! | Time since a holding move | never, just held, less than `shoot_after_moving_s`, exactly it, more | `the_gate_opens_after_the_wait` |

    use super::*;

    const SHOOT_AFTER_MOVING_S: f64 = 0.25;
    const STEP_S: f64 = 0.1;

    /// `make_movement` gives movement settings that hold a shot for `SHOOT_AFTER_MOVING_S`.
    fn make_movement() -> MovementSettings {
        MovementSettings {
            walk_speed_per_map_speed: 1.0,
            sprint_speed_per_map_speed: 1.0,
            sprint_up_m_per_s_per_s: 15.0,
            sprint_down_m_per_s_per_s: 15.0,
            jump_height_m: 1.0,
            jump_time_per_map_jump_time: 1.0,
            crouch_speed_frac: 0.5,
            crouch_eye_height_frac: 0.5,
            crouch_s: 0.1,
            shoot_after_moving_s: SHOOT_AFTER_MOVING_S,
        }
    }

    #[test]
    fn each_move_holds_a_shot_unless_its_switch_is_on() {
        let still: ShotMoves = ShotMoves::default();
        let walking: ShotMoves = ShotMoves {
            is_walking_upright: true,
            ..still
        };
        let sprinting: ShotMoves = ShotMoves {
            is_sprinting: true,
            ..walking
        };
        let jumping: ShotMoves = ShotMoves {
            is_jumping: true,
            ..still
        };
        let sprint_jumping: ShotMoves = ShotMoves {
            is_jumping: true,
            ..sprinting
        };
        let crouch_sprint_key: ShotMoves = ShotMoves {
            is_sprinting: true,
            ..still
        };
        let off: ShootingSettings = ShootingSettings::default();
        let held = |moves: ShotMoves, shooting: &ShootingSettings| moves.is_holding_shot(shooting);
        assert!(!held(still, &off) && !held(crouch_sprint_key, &off));
        assert!(held(walking, &off) && held(sprinting, &off) && held(jumping, &off));
        let walk_on: ShootingSettings = ShootingSettings {
            can_shoot_while_walking: true,
            ..off.clone()
        };
        assert!(!held(walking, &walk_on) && held(sprinting, &walk_on) && held(jumping, &walk_on));
        let sprint_on: ShootingSettings = ShootingSettings {
            can_shoot_while_sprinting: true,
            ..off.clone()
        };
        assert!(held(walking, &sprint_on) && !held(sprinting, &sprint_on));
        assert!(held(sprint_jumping, &sprint_on));
        let jump_on: ShootingSettings = ShootingSettings {
            can_shoot_while_jumping: true,
            ..off.clone()
        };
        assert!(!held(jumping, &jump_on) && held(sprint_jumping, &jump_on));
        let all_on: ShootingSettings = ShootingSettings {
            can_shoot_while_jumping: true,
            can_shoot_while_walking: true,
            can_shoot_while_sprinting: true,
            ..off
        };
        for moves in [still, walking, sprinting, jumping, sprint_jumping] {
            assert!(!held(moves, &all_on), "{moves:?}");
        }
    }

    #[test]
    fn the_gate_opens_after_the_wait() {
        let movement: MovementSettings = make_movement();
        let shooting: ShootingSettings = ShootingSettings::default();
        let mut gate: ShotGate = ShotGate::New();
        assert!(gate.is_open(&movement));
        let walking: ShotMoves = ShotMoves {
            is_walking_upright: true,
            ..ShotMoves::default()
        };
        gate.note_moves(&walking, &shooting, STEP_S);
        assert!(!gate.is_open(&movement));
        gate.note_moves(&ShotMoves::default(), &shooting, STEP_S);
        assert!(!gate.is_open(&movement));
        gate.note_moves(
            &ShotMoves::default(),
            &shooting,
            SHOOT_AFTER_MOVING_S - STEP_S,
        );
        assert!(gate.is_open(&movement));
        gate.note_moves(&ShotMoves::default(), &shooting, STEP_S);
        assert!(gate.is_open(&movement));
    }
}
