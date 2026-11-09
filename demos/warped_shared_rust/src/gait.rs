//! `gait.rs` is how every level animates another player's robot from what it sees of them (W-126,
//! same play in every level; W-106, Teal robot player model): standing (`Idle`), walking or
//! running by their speed over the ground with the stride kept in step with it, and in the air
//! (`Jump`, from takeoff). A level measures the ground it covers and whether it is supported in
//! its own space; these are the rules.
//!
//! Interface: `Gait` (`New`, `follow`, `get_pose`), `choose_gait`, `MIN_WALK_M_PER_S`,
//! `STALE_SNAPSHOT_S`, `MAX_STRIDES_PER_S`, `MIN_AIR_S`.
//!
//! Example: `gait.follow(Some(0.15), true, 14.0, now_s, 1.0 / 60.0); let (clip, time_s) = gait.get_pose(14.0, now_s, |clip| durations[clip]);`

use crate::avatar_model::AvatarClip;

const SPEED_SMOOTHING_S: f64 = 0.12;
/// `STALE_SNAPSHOT_S` is how long a pose may stand still before the player counts as stopped.
pub const STALE_SNAPSHOT_S: f64 = 0.25;
/// `MIN_WALK_M_PER_S` is the slowest speed over the ground that walks rather than stands.
pub const MIN_WALK_M_PER_S: f64 = 0.8;
const WALK_CYCLE_M: f64 = 1.6;
const RUN_CYCLE_M: f64 = 2.4;
/// `MAX_STRIDES_PER_S` is the fastest the stride loops.
pub const MAX_STRIDES_PER_S: f64 = 2.5;
/// `MIN_AIR_S` is how long a player must be off the ground before the jump shows.
pub const MIN_AIR_S: f64 = 0.08;

/// `Gait` is what a window remembers of one player's motion: when they last moved, their smoothed
/// speed over the ground, where they are in the stride (fraction of a loop), and when they left
/// the ground (`None` while on it).
#[derive(Clone, Copy, Debug)]
pub struct Gait {
    moved_s: f64,
    speed_m_per_s: f64,
    stride_frac: f64,
    left_ground_s: Option<f64>,
}

/// `choose_gait` gives the clip for moving at `speed_m_per_s` over the ground: idle below
/// `MIN_WALK_M_PER_S`, walking below `run_from_m_per_s`, running from there.
///
/// Example: `let clip = choose_gait(10.0, 14.0);`
pub fn choose_gait(speed_m_per_s: f64, run_from_m_per_s: f64) -> AvatarClip {
    if speed_m_per_s < MIN_WALK_M_PER_S {
        AvatarClip::Idle
    } else if speed_m_per_s < run_from_m_per_s {
        AvatarClip::Walk
    } else {
        AvatarClip::Run
    }
}

impl Gait {
    /// `New` is a player first seen at `now_s`, standing still on the ground.
    ///
    /// Example: `let gait = Gait::New(12.5);`
    #[allow(non_snake_case)]
    pub fn New(now_s: f64) -> Gait {
        Gait {
            moved_s: now_s,
            speed_m_per_s: 0.0,
            stride_frac: 0.0,
            left_ground_s: None,
        }
    }

    /// `follow` takes what this window sees of the player at `now_s`, `dt_s` after the last frame:
    /// how far over the ground they went since the last pose it saw (`moved_m`, `None` when their
    /// pose hasn't changed), and whether they stand on something. It smooths their speed over
    /// `SPEED_SMOOTHING_S` (zero once the pose has stood still for `STALE_SNAPSHOT_S`, as poses
    /// don't come every frame), moves the stride on by it (one loop per `WALK_CYCLE_M` or
    /// `RUN_CYCLE_M`, at most `MAX_STRIDES_PER_S`, so the feet slide at the game's fast speeds
    /// rather than blur), and notes when they leave the ground.
    ///
    /// Example: `gait.follow(Some(0.15), true, 14.0, 12.5, 0.016);`
    pub fn follow(
        &mut self,
        moved_m: Option<f64>,
        is_on_ground: bool,
        run_from_m_per_s: f64,
        now_s: f64,
        dt_s: f64,
    ) {
        match moved_m {
            Some(ground_m) => {
                let since_s: f64 = (now_s - self.moved_s).max(dt_s).max(f64::EPSILON);
                let blend_frac: f64 = 1.0 - (-since_s / SPEED_SMOOTHING_S).exp();
                self.speed_m_per_s += (ground_m / since_s - self.speed_m_per_s) * blend_frac;
                self.moved_s = now_s;
            }
            None if now_s - self.moved_s > STALE_SNAPSHOT_S => self.speed_m_per_s = 0.0,
            None => {}
        }
        let cycle_m: f64 = match choose_gait(self.speed_m_per_s, run_from_m_per_s) {
            AvatarClip::Run => RUN_CYCLE_M,
            _ => WALK_CYCLE_M,
        };
        let rate_per_s: f64 = (self.speed_m_per_s / cycle_m).min(MAX_STRIDES_PER_S);
        self.stride_frac = (self.stride_frac + rate_per_s * dt_s).fract();
        self.left_ground_s = match (is_on_ground, self.left_ground_s) {
            (true, _) => None,
            (false, None) => Some(now_s),
            (false, Some(left_s)) => Some(left_s),
        };
    }

    /// `get_pose` gives the clip and the time in it for the player now: in the air for
    /// `MIN_AIR_S` or more, jumping (from then); otherwise by `choose_gait`, standing on the
    /// clock and stepping at its place in the stride times the clip's length
    /// (`get_duration_s`), so walking and running hand over in step.
    ///
    /// Example: `let (clip, time_s) = gait.get_pose(14.0, 12.5, |clip| body.get_duration_s(clip));`
    pub fn get_pose(
        &self,
        run_from_m_per_s: f64,
        now_s: f64,
        get_duration_s: impl Fn(AvatarClip) -> f64,
    ) -> (AvatarClip, f64) {
        if let Some(left_s) = self.left_ground_s
            && now_s - left_s >= MIN_AIR_S
        {
            return (AvatarClip::Jump, now_s - left_s - MIN_AIR_S);
        }
        match choose_gait(self.speed_m_per_s, run_from_m_per_s) {
            AvatarClip::Idle => (AvatarClip::Idle, now_s),
            clip => (clip, self.stride_frac * get_duration_s(clip)),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the gait rules.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Speed over the ground | still, walking speed, running speed | `the_gait_follows_the_speed` |
    //! | Ground | on it, just left (under `MIN_AIR_S`), in the air (`Jump` from takeoff) | `jumping_starts_in_the_air` |
    //! | Poses seen | every frame, none for longer than `STALE_SNAPSHOT_S` (idle) | `the_gait_follows_the_speed` |

    use super::*;

    const FRAME_S: f64 = 1.0 / 60.0;
    const RUN_FROM_M_PER_S: f64 = 14.0;
    const DURATION_S: f64 = 1.0;

    /// `run_at` follows a player moving at `speed_m_per_s` on the ground for `duration_s` from
    /// `start_s`, and gives the time it ended at.
    fn run_at(gait: &mut Gait, speed_m_per_s: f64, start_s: f64, duration_s: f64) -> f64 {
        let mut now_s: f64 = start_s;
        for _ in 0..(duration_s / FRAME_S).round() as usize {
            now_s += FRAME_S;
            let moved_m: Option<f64> = (speed_m_per_s > 0.0).then_some(speed_m_per_s * FRAME_S);
            gait.follow(moved_m, true, RUN_FROM_M_PER_S, now_s, FRAME_S);
        }
        now_s
    }

    #[test]
    fn the_gait_follows_the_speed() {
        let mut gait: Gait = Gait::New(0.0);
        assert_eq!(gait.get_pose(RUN_FROM_M_PER_S, 0.0, |_| DURATION_S).0, AvatarClip::Idle);
        let now_s: f64 = run_at(&mut gait, 5.0, 0.0, 1.0);
        let (clip, time_s) = gait.get_pose(RUN_FROM_M_PER_S, now_s, |_| DURATION_S);
        assert_eq!(clip, AvatarClip::Walk);
        assert!((0.0..DURATION_S).contains(&time_s));
        let now_s: f64 = run_at(&mut gait, 20.0, now_s, 1.0);
        assert_eq!(gait.get_pose(RUN_FROM_M_PER_S, now_s, |_| DURATION_S).0, AvatarClip::Run);
        let now_s: f64 = run_at(&mut gait, 0.0, now_s, 2.0 * STALE_SNAPSHOT_S);
        assert_eq!(gait.get_pose(RUN_FROM_M_PER_S, now_s, |_| DURATION_S).0, AvatarClip::Idle);
    }

    #[test]
    fn jumping_starts_in_the_air() {
        let mut gait: Gait = Gait::New(0.0);
        gait.follow(None, false, RUN_FROM_M_PER_S, FRAME_S, FRAME_S);
        assert_eq!(gait.get_pose(RUN_FROM_M_PER_S, FRAME_S, |_| DURATION_S).0, AvatarClip::Idle);
        let later_s: f64 = FRAME_S + MIN_AIR_S + 0.1;
        gait.follow(None, false, RUN_FROM_M_PER_S, later_s, FRAME_S);
        let (clip, time_s) = gait.get_pose(RUN_FROM_M_PER_S, later_s, |_| DURATION_S);
        assert_eq!(clip, AvatarClip::Jump);
        assert!((time_s - 0.1).abs() < 1e-9);
        gait.follow(None, true, RUN_FROM_M_PER_S, later_s + FRAME_S, FRAME_S);
        assert_ne!(gait.get_pose(RUN_FROM_M_PER_S, later_s + FRAME_S, |_| DURATION_S).0, AvatarClip::Jump);
    }
}
