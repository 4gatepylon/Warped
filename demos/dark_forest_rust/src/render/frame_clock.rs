//! `frame_clock` turns the measured time between frame loops into the step the game simulates,
//! so motion stays even when the loop's wake-ups jitter: it averages the last few frames, snaps
//! the average to whole display refreshes when it is close to one, and pays time it held back or
//! ran ahead into later steps, so game time stays within half a step of the wall clock.
//!
//! Interface: `FrameClock`.
//!
//! Example:
//!     let mut clock: FrameClock = FrameClock::default();
//!     let dt_s: f32 = clock.step_s(measured_s, Some(1.0 / 100.0));

use std::collections::VecDeque;

/// `AVERAGED_FRAME_COUNT` is how many of the last frames the step averages.
const AVERAGED_FRAME_COUNT: usize = 4;
/// `SNAP_FRAC` of a refresh interval is how close the average must be to a whole number of
/// refreshes to be snapped to it.
const SNAP_FRAC: f32 = 0.15;
/// `PAYBACK_FRAC` of the time owed beyond half a step is paid back each frame.
const PAYBACK_FRAC: f32 = 0.1;
/// `OWED_SLACK_FRAC` of a step may stay owed without being paid back, so that wake-up jitter,
/// which evens out by itself, doesn't make the steps jitter.
const OWED_SLACK_FRAC: f32 = 0.5;
/// `MAX_PAYBACK_FRAC` of the step is the most that one frame pays back.
const MAX_PAYBACK_FRAC: f32 = 0.25;

/// `FrameClock` remembers the last measured frame times and the time owed to the game.
#[derive(Default)]
pub struct FrameClock {
    measured_s: VecDeque<f32>,
    owed_s: f32,
}

impl FrameClock {
    /// `step_s` takes the measured time since the last frame and returns the step to simulate:
    /// the average of the last `AVERAGED_FRAME_COUNT` measurements, snapped to a whole number of
    /// `refresh_interval_s` (None when frames don't wait for the display) when within `SNAP_FRAC`
    /// of one, plus part of the time owed from earlier steps beyond `OWED_SLACK_FRAC` of a step.
    ///
    /// Example: `let dt_s: f32 = clock.step_s(0.0103, Some(0.01));`
    pub fn step_s(&mut self, measured_s: f32, refresh_interval_s: Option<f32>) -> f32 {
        assert!(
            measured_s >= 0.0,
            "a frame takes no negative time, not {measured_s} s"
        );
        if self.measured_s.len() == AVERAGED_FRAME_COUNT {
            self.measured_s.pop_front();
        }
        self.measured_s.push_back(measured_s);
        let mean_s: f32 = self.measured_s.iter().sum::<f32>() / self.measured_s.len() as f32;
        let target_s: f32 = match refresh_interval_s {
            Some(interval_s) => {
                let refresh_count: f32 = (mean_s / interval_s).round().max(1.0);
                let snapped_s: f32 = refresh_count * interval_s;
                if (mean_s - snapped_s).abs() <= SNAP_FRAC * interval_s {
                    snapped_s
                } else {
                    mean_s
                }
            }
            None => mean_s,
        };
        self.owed_s += measured_s - target_s;
        let slack_s: f32 = target_s * OWED_SLACK_FRAC;
        let beyond_slack_s: f32 = self.owed_s.signum() * (self.owed_s.abs() - slack_s).max(0.0);
        let max_payback_s: f32 = target_s * MAX_PAYBACK_FRAC;
        let payback_s: f32 = (beyond_slack_s * PAYBACK_FRAC).clamp(-max_payback_s, max_payback_s);
        self.owed_s -= payback_s;
        target_s + payback_s
    }
}

/// `tests` checks the step for steady, jittery, changing and stalled frame times.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | measured times | steady at the refresh, jittering around it | `test_jitter_around_the_refresh_gives_even_steps` |
/// | measured times | steady between refreshes (no snap) | `test_times_between_refreshes_are_kept` |
/// | measured times | one long stall | `test_a_stall_is_spread_and_paid_back` |
/// | refresh interval | known, unknown (no vsync) | `test_jitter_around_the_refresh_gives_even_steps`, `test_without_a_refresh_the_average_is_used` |
/// | game time vs wall time | equal in the long run | `test_a_stall_is_spread_and_paid_back`, `test_jitter_around_the_refresh_gives_even_steps` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `REFRESH_S` is a 100 Hz display's refresh interval.
    const REFRESH_S: f32 = 0.01;

    /// `run` steps a clock through `measured_s` and returns the steps.
    fn run(measured_s: &[f32], refresh_interval_s: Option<f32>) -> Vec<f32> {
        let mut clock: FrameClock = FrameClock::default();
        measured_s
            .iter()
            .map(|&s| clock.step_s(s, refresh_interval_s))
            .collect()
    }

    /// `test_jitter_around_the_refresh_gives_even_steps` checks that frames shown every refresh
    /// but measured with ±1.5 ms of wake-up jitter give steps of one refresh once the average is
    /// full, and that game time stays with wall time.
    #[test]
    fn test_jitter_around_the_refresh_gives_even_steps() {
        let measured_s: Vec<f32> = (0..400)
            .map(|i| REFRESH_S + if i % 2 == 0 { 0.0015 } else { -0.0015 })
            .collect();
        let steps_s: Vec<f32> = run(&measured_s, Some(REFRESH_S));
        assert!(
            steps_s[AVERAGED_FRAME_COUNT..]
                .iter()
                .all(|&s| (s - REFRESH_S).abs() < 1e-6),
            "{steps_s:?}"
        );
        let drift_s: f32 = steps_s.iter().sum::<f32>() - measured_s.iter().sum::<f32>();
        assert!(drift_s.abs() <= REFRESH_S * OWED_SLACK_FRAC, "{drift_s}");
    }

    /// `test_times_between_refreshes_are_kept` checks that a steady 13 ms frame, between one and
    /// two refreshes, is simulated as 13 ms.
    #[test]
    fn test_times_between_refreshes_are_kept() {
        let steps_s: Vec<f32> = run(&[0.013; 20], Some(REFRESH_S));
        assert!(steps_s.iter().all(|&s| (s - 0.013).abs() < 1e-6));
    }

    /// `test_a_stall_is_spread_and_paid_back` checks that one 50 ms stall among 10 ms frames is
    /// spread over the next steps (no step jumps by the whole stall) and paid back to within
    /// half a step.
    #[test]
    fn test_a_stall_is_spread_and_paid_back() {
        let mut measured_s: Vec<f32> = vec![REFRESH_S; 300];
        measured_s[100] = 0.05;
        let steps_s: Vec<f32> = run(&measured_s, Some(REFRESH_S));
        assert!(steps_s.iter().all(|&s| s < 0.03), "{steps_s:?}");
        let drift_s: f32 = steps_s.iter().sum::<f32>() - measured_s.iter().sum::<f32>();
        assert!(
            drift_s.abs() <= REFRESH_S * OWED_SLACK_FRAC + 1e-5,
            "{drift_s}"
        );
    }

    /// `test_without_a_refresh_the_average_is_used` checks that without vsync the step is the
    /// running average, not snapped.
    #[test]
    fn test_without_a_refresh_the_average_is_used() {
        let steps_s: Vec<f32> = run(&[0.004, 0.006, 0.004, 0.006], None);
        assert!((steps_s[3] - 0.005).abs() < 1e-6, "{steps_s:?}");
    }
}
