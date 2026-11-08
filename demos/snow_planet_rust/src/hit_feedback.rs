//! `hit_feedback.rs` decides what a fall in this player's health shows and does, W-126 (same play
//! in every level). A shot (a fall of most of a hit's worth or more at once) shows the hit effect
//! (red edges, blur and shake) at full strength and slows the player, as on Lunar Surface. Damage
//! that comes a little at a time (freezing in a blizzard) adds up, and each hit's worth of it shows
//! the effect once, as a hit of that size would; it doesn't slow the player, since the blizzard
//! slows them itself.
//!
//! Interface: `HitFeedback`, `HitResponse`.
//!
//! Example: `let response = feedback.take_health(client.health_frac, hit_frac);`
//!
//! Must know: the server sends health as a byte, so a hit's fall can read up to 1/255 short;
//! `SHOT_SHARE_OF_HIT_FRAC` leaves room for that.

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

#[cfg(test)]
mod tests {
    //! Tests of `HitFeedback`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
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
}
