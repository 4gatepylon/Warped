//! `health.rs` is a player's health as every level counts it (W-126, same play in every level),
//! Lunar Surface's: the game server adds 1 / `hits_to_kill` of damage per hit (`add_damage`,
//! `get_hit_damage_frac`) and eliminates a player at full damage (`is_eliminated`); the window
//! decides what a fall in its health shows and does (`HitFeedback`) and fades the hit effect
//! (`fade_effect_frac`). A shot (a fall of most of a hit's worth or more at once) shows the hit
//! effect (red edges, blur and shake) at full strength and slows the player. Damage that comes a
//! little at a time (freezing in Snow Planet's blizzard) adds up, and each hit's worth of it shows
//! the effect once, as a hit of that size would; it doesn't slow the player, since the blizzard
//! slows them itself.
//!
//! Interface: `DAMAGE_TOLERANCE_FRAC`, `get_hit_damage_frac`, `add_damage`, `is_eliminated`,
//! `fade_effect_frac`, `HitFeedback`, `HitResponse`.
//!
//! Example: `let response = feedback.take_health(client.health_frac, hit_frac);`
//!
//! Must know: the server sends health as a byte, so a hit's fall can read up to 1/255 short;
//! `SHOT_SHARE_OF_HIT_FRAC` leaves room for that.

/// `DAMAGE_TOLERANCE_FRAC` is how far below full damage still eliminates, so that `hits_to_kill`
/// hits of 1 / `hits_to_kill` each eliminate despite rounding.
pub const DAMAGE_TOLERANCE_FRAC: f64 = 1e-9;

/// `SHOT_SHARE_OF_HIT_FRAC` is how much of a hit's worth one fall must be to count as a shot.
const SHOT_SHARE_OF_HIT_FRAC: f64 = 0.9;

/// `HitResponse` is what one fall in health does: whether it shows the hit effect and whether it
/// slows the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct HitResponse {
    pub should_show: bool,
    pub should_slow: bool,
}

/// `HitFeedback` keeps the last health this player had and the damage not shown yet.
pub struct HitFeedback {
    last_health_frac: f64,
    unshown_damage_frac: f64,
}

impl HitFeedback {
    /// `New` starts at full health with nothing to show.
    ///
    /// Example: `let feedback = HitFeedback::New();`
    #[allow(non_snake_case)]
    pub fn New() -> HitFeedback {
        HitFeedback {
            last_health_frac: 1.0,
            unshown_damage_frac: 0.0,
        }
    }

    /// `take_health` takes the health the server last sent and gives what its fall since the last
    /// call does, for hits of `hit_frac` of the health (the smallest a gun deals). A rise (a
    /// respawn) forgets the damage not shown.
    ///
    /// Example: `if feedback.take_health(0.9, 1.0 / 12.0).should_show { ... }`
    pub fn take_health(&mut self, health_frac: f64, hit_frac: f64) -> HitResponse {
        let fall_frac: f64 = self.last_health_frac - health_frac;
        self.last_health_frac = health_frac;
        if fall_frac <= 0.0 {
            if fall_frac < 0.0 {
                self.unshown_damage_frac = 0.0;
            }
            return HitResponse::default();
        }
        let is_shot: bool = fall_frac >= hit_frac * SHOT_SHARE_OF_HIT_FRAC;
        self.unshown_damage_frac += fall_frac;
        let should_show: bool = is_shot || self.unshown_damage_frac >= hit_frac;
        if should_show {
            self.unshown_damage_frac = 0.0;
        }
        HitResponse {
            should_show,
            should_slow: is_shot,
        }
    }
}

/// `get_hit_damage_frac` gives the damage one hit adds when `hits_to_kill` such hits eliminate.
///
/// Example: `let damage_frac: f64 = get_hit_damage_frac(12);`
pub fn get_hit_damage_frac(hits_to_kill: u8) -> f64 {
    1.0 / f64::from(hits_to_kill)
}

/// `add_damage` adds `added_frac` to a player's `damage_frac` and says whether that eliminates
/// them (`is_eliminated`).
///
/// Example: `if add_damage(&mut slot.damage_frac, get_hit_damage_frac(hits_to_kill)) { ... }`
pub fn add_damage(damage_frac: &mut f64, added_frac: f64) -> bool {
    *damage_frac += added_frac;
    is_eliminated(*damage_frac)
}

/// `is_eliminated` says whether `damage_frac` of damage eliminates a player: full damage, within
/// `DAMAGE_TOLERANCE_FRAC`.
///
/// Example: `let is_out: bool = is_eliminated(slot.damage_frac);`
pub fn is_eliminated(damage_frac: f64) -> bool {
    damage_frac >= 1.0 - DAMAGE_TOLERANCE_FRAC
}

/// `fade_effect_frac` fades a hit effect's strength (1 just after a hit) by `dt_s` of the
/// `effect_s` it takes to fade fully, to no less than 0.
///
/// Example: `self.hit_effect_frac = fade_effect_frac(self.hit_effect_frac, dt_s, 0.6);`
pub fn fade_effect_frac(effect_frac: f64, dt_s: f64, effect_s: f64) -> f64 {
    (effect_frac - dt_s / effect_s).max(0.0)
}

#[cfg(test)]
mod tests {
    //! Tests of the damage a hit adds and of `HitFeedback`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Hits added | fewer than `hits_to_kill` (not eliminated), exactly `hits_to_kill` of every gun's (eliminated despite rounding) | `hits_to_kill_hits_eliminate` |
    //! | Fading | part way, past the end (0) | `the_hit_effect_fades` |
    //! | Fall | none, a shot (a hit's worth, read a byte short), small falls (freezing) | `shots_show_and_slow`, `freezing_shows_once_per_hit_without_slowing` |
    //! | Health after | lower, the same, higher (a respawn) | `a_respawn_forgets_unshown_damage` |

    use super::*;

    const HIT_FRAC: f64 = 1.0 / 12.0;
    const BYTE_FRAC: f64 = 1.0 / 255.0;
    const FREEZE_PER_S: f64 = 0.08;
    const SNAPSHOT_S: f64 = 1.0 / 60.0;

    #[test]
    fn shots_show_and_slow() {
        let mut feedback: HitFeedback = HitFeedback::New();
        let shot: HitResponse = HitResponse {
            should_show: true,
            should_slow: true,
        };
        assert_eq!(
            feedback.take_health(1.0 - HIT_FRAC + BYTE_FRAC, HIT_FRAC),
            shot
        );
        assert_eq!(
            feedback.take_health(1.0 - HIT_FRAC + BYTE_FRAC, HIT_FRAC),
            HitResponse::default()
        );
        assert_eq!(feedback.take_health(0.5, HIT_FRAC), shot);
    }

    #[test]
    fn freezing_shows_once_per_hit_without_slowing() {
        let mut feedback: HitFeedback = HitFeedback::New();
        let mut health_frac: f64 = 1.0;
        let mut shown_count: usize = 0;
        let seconds: f64 = 3.0;
        for _ in 0..(seconds / SNAPSHOT_S) as usize {
            health_frac -= FREEZE_PER_S * SNAPSHOT_S;
            let sent_frac: f64 = (health_frac * 255.0).round() / 255.0;
            let response: HitResponse = feedback.take_health(sent_frac, HIT_FRAC);
            assert!(!response.should_slow);
            shown_count += usize::from(response.should_show);
        }
        assert_eq!(shown_count, (seconds * FREEZE_PER_S / HIT_FRAC) as usize);
    }

    #[test]
    fn a_respawn_forgets_unshown_damage() {
        let mut feedback: HitFeedback = HitFeedback::New();
        assert!(
            !feedback
                .take_health(1.0 - HIT_FRAC / 2.0, HIT_FRAC)
                .should_show
        );
        assert!(!feedback.take_health(1.0, HIT_FRAC).should_show);
        assert!(
            !feedback
                .take_health(1.0 - HIT_FRAC / 2.0, HIT_FRAC)
                .should_show
        );
    }

    #[test]
    fn hits_to_kill_hits_eliminate() {
        const HITS_TO_KILL: [u8; 3] = [1, 2, 12];
        for hits_to_kill in HITS_TO_KILL {
            let mut damage_frac: f64 = 0.0;
            for _ in 1..hits_to_kill {
                assert!(!add_damage(
                    &mut damage_frac,
                    get_hit_damage_frac(hits_to_kill)
                ));
            }
            assert!(add_damage(
                &mut damage_frac,
                get_hit_damage_frac(hits_to_kill)
            ));
        }
    }

    #[test]
    fn the_hit_effect_fades() {
        const EFFECT_S: f64 = 0.5;
        assert_eq!(fade_effect_frac(1.0, EFFECT_S / 2.0, EFFECT_S), 0.5);
        assert_eq!(fade_effect_frac(0.25, EFFECT_S, EFFECT_S), 0.0);
    }
}
