//! `frame_clock` turns the measured time between frame loops into the step the game simulates,
//! so motion stays even when the loop's wake-ups jitter: it averages the last few frames, snaps
//! the average to whole display refreshes when it is close to one, and pays time it held back or
//! ran ahead into later steps, so game time stays within half a step of the wall clock.
//!
//! It also picks how many display refreshes each frame stays on screen
//! (`choose_refreshes_per_frame`), so frames that the GPU can't finish within one refresh are
//! shown at an even cadence (every second refresh, say) instead of a mix of one and two.
//!
//! Interface: `FrameClock`, `choose_refreshes_per_frame`, `CadenceHold`, `MIN_CADENCE_SAMPLE_COUNT`.
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

/// `MIN_CADENCE_SAMPLE_COUNT` is how many shown frames (their intervals and GPU times)
/// `choose_refreshes_per_frame` needs since the cadence last changed before it changes it again.
pub const MIN_CADENCE_SAMPLE_COUNT: usize = 30;
/// `MAX_REFRESHES_PER_FRAME` is the slowest cadence: every third refresh (33 fps at 100 Hz).
const MAX_REFRESHES_PER_FRAME: u32 = 3;
/// `LATE_FRAC` of the shown frames may stay up longer than planned (by half a refresh or more)
/// before the cadence slows down.
const LATE_FRAC: f64 = 0.1;
/// `SPEED_UP_FRAC` of one refresh fewer is what the GPU work (90th percentile) must fit in before
/// the cadence speeds up, so it doesn't flip back and forth.
const SPEED_UP_FRAC: f64 = 0.85;
/// `MIN_HOLD_S` and `MAX_HOLD_S` bound how long the cadence waits before speeding up again after a
/// speed-up that slowed down again within `FAILED_SPEED_UP_S` (`CadenceHold`): the wait doubles
/// with each such failure and starts over after a speed-up that holds.
const MIN_HOLD_S: f64 = 2.0;
const MAX_HOLD_S: f64 = 16.0;
const FAILED_SPEED_UP_S: f64 = 3.0;

/// `CadenceHold` keeps the cadence from speeding up again and again when the GPU work sits just
/// inside one refresh fewer: after a speed-up that had to slow down again soon, it waits
/// `MIN_HOLD_S`, then twice as long after each further failure, up to `MAX_HOLD_S`.
///
/// Example:
///     let mut hold: CadenceHold = CadenceHold::New();
///     let refreshes: u32 = choose_refreshes_per_frame(2, &shown_ms, &gpu_ms, 10.0, hold.can_speed_up(now_s));
///     hold.note_change(2, refreshes, now_s);
pub struct CadenceHold {
    hold_s: f64,
    held_until_s: f64,
    last_speed_up_s: Option<f64>,
}

impl CadenceHold {
    /// `New` gives a hold that lets the cadence speed up at once.
    ///
    /// Example: `let hold: CadenceHold = CadenceHold::New();`
    #[allow(non_snake_case)]
    pub fn New() -> CadenceHold {
        CadenceHold {
            hold_s: MIN_HOLD_S,
            held_until_s: f64::MIN,
            last_speed_up_s: None,
        }
    }

    /// `can_speed_up` says whether the cadence may speed up at `now_s` (seconds on any clock that
    /// only goes forward).
    ///
    /// Example: `if hold.can_speed_up(now_s) { ... }`
    pub fn can_speed_up(&self, now_s: f64) -> bool {
        now_s >= self.held_until_s
    }

    /// `note_change` records the cadence going from `from` to `to` refreshes per frame at `now_s`.
    ///
    /// Example: `hold.note_change(1, 2, now_s);`
    pub fn note_change(&mut self, from: u32, to: u32, now_s: f64) {
        if to < from {
            self.last_speed_up_s = Some(now_s);
        } else if to > from {
            let has_failed: bool = self
                .last_speed_up_s
                .is_some_and(|speed_up_s| now_s - speed_up_s < FAILED_SPEED_UP_S);
            if has_failed {
                self.held_until_s = now_s + self.hold_s;
                self.hold_s = (self.hold_s * 2.0).min(MAX_HOLD_S);
            } else {
                self.held_until_s = now_s;
                self.hold_s = MIN_HOLD_S;
            }
            self.last_speed_up_s = None;
        }
    }
}

/// `choose_refreshes_per_frame` is how many refreshes of `refresh_ms` each frame should stay on
/// screen, given the current cadence and, since it was set, the intervals at which frames were
/// shown and the frames' GPU times (at least `MIN_CADENCE_SAMPLE_COUNT` of each): one step slower
/// when more than `LATE_FRAC` of the frames stayed up half a refresh or more longer than planned,
/// one step faster, if `can_speed_up` (`CadenceHold`), when the GPU's 90th percentile fits in
/// `SPEED_UP_FRAC` of one refresh fewer.
///
/// Example: `let refreshes: u32 = choose_refreshes_per_frame(1, &shown_ms, &gpu_ms, 10.0, true);`
pub fn choose_refreshes_per_frame(
    current: u32,
    shown_intervals_ms: &[f64],
    gpu_ms: &[f64],
    refresh_ms: f64,
    can_speed_up: bool,
) -> u32 {
    assert!(
        shown_intervals_ms.len() >= MIN_CADENCE_SAMPLE_COUNT
            && gpu_ms.len() >= MIN_CADENCE_SAMPLE_COUNT,
        "the cadence needs {MIN_CADENCE_SAMPLE_COUNT} shown intervals and GPU times, not {} and {}",
        shown_intervals_ms.len(),
        gpu_ms.len()
    );
    let late_ms: f64 = (f64::from(current) + 0.5) * refresh_ms;
    let late_count: usize = shown_intervals_ms
        .iter()
        .filter(|&&ms| ms > late_ms)
        .count();
    let mut sorted_ms: Vec<f64> = gpu_ms.to_vec();
    sorted_ms.sort_by(f64::total_cmp);
    let p90_ms: f64 = sorted_ms[(sorted_ms.len() - 1) * 9 / 10];
    if late_count as f64 > LATE_FRAC * shown_intervals_ms.len() as f64 {
        (current + 1).min(MAX_REFRESHES_PER_FRAME)
    } else if can_speed_up
        && current > 1
        && p90_ms < f64::from(current - 1) * refresh_ms * SPEED_UP_FRAC
    {
        current - 1
    } else {
        current
    }
}

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
/// | shown frames vs cadence | on time, a 10/20 ms mix, late at two refreshes, late at the slowest, GPU well inside a refresh fewer (with and without margin, held back or not) | `test_cadence_follows_the_shown_frames` |
/// | speed-ups that fail | none, one soon after another, several in a row, then one that holds | `test_failed_speed_ups_wait_longer_each_time` |
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

    /// `test_cadence_follows_the_shown_frames` checks that at 100 Hz frames shown on time every
    /// refresh keep that cadence, a mix of 10 and 20 ms slows it to every second refresh, frames
    /// late at that cadence slow it to every third and no further, and GPU work well inside one
    /// refresh speeds it up again.
    #[test]
    fn test_cadence_follows_the_shown_frames() {
        let refresh_ms: f64 = 10.0;
        let on_time = |refreshes: f64| vec![refresh_ms * refreshes; MIN_CADENCE_SAMPLE_COUNT];
        let mixed: Vec<f64> = (0..MIN_CADENCE_SAMPLE_COUNT)
            .map(|i| if i % 3 == 0 { 20.0 } else { 10.0 })
            .collect();
        let gpu = |ms: f64| vec![ms; MIN_CADENCE_SAMPLE_COUNT];
        assert_eq!(
            choose_refreshes_per_frame(1, &on_time(1.0), &gpu(9.0), refresh_ms, true),
            1
        );
        assert_eq!(
            choose_refreshes_per_frame(1, &mixed, &gpu(13.0), refresh_ms, true),
            2
        );
        assert_eq!(
            choose_refreshes_per_frame(2, &on_time(2.0), &gpu(13.0), refresh_ms, true),
            2
        );
        assert_eq!(
            choose_refreshes_per_frame(2, &on_time(3.0), &gpu(25.0), refresh_ms, true),
            3
        );
        assert_eq!(
            choose_refreshes_per_frame(3, &on_time(4.0), &gpu(35.0), refresh_ms, true),
            3
        );
        assert_eq!(
            choose_refreshes_per_frame(2, &on_time(2.0), &gpu(9.0), refresh_ms, true),
            2
        );
        assert_eq!(
            choose_refreshes_per_frame(2, &on_time(2.0), &gpu(8.0), refresh_ms, true),
            1
        );
        assert_eq!(
            choose_refreshes_per_frame(2, &on_time(2.0), &gpu(8.0), refresh_ms, false),
            2
        );
    }

    /// `test_failed_speed_ups_wait_longer_each_time` checks that a slow-down long after a
    /// speed-up holds nothing back, one soon after waits `MIN_HOLD_S`, a second twice that, up
    /// to `MAX_HOLD_S`, and a speed-up that holds starts the waits over.
    #[test]
    fn test_failed_speed_ups_wait_longer_each_time() {
        let mut hold: CadenceHold = CadenceHold::New();
        assert!(hold.can_speed_up(0.0));
        hold.note_change(2, 1, 0.0);
        hold.note_change(1, 2, 10.0);
        assert!(hold.can_speed_up(10.0));
        let mut now_s: f64 = 20.0;
        let mut waits_s: Vec<f64> = Vec::new();
        for _ in 0..5 {
            hold.note_change(2, 1, now_s);
            hold.note_change(1, 2, now_s + 1.0);
            now_s += 1.0;
            let wait_s: f64 = (0..=40)
                .map(|tenth| f64::from(tenth) * 0.5)
                .find(|&wait_s| hold.can_speed_up(now_s + wait_s))
                .expect("the hold ends");
            waits_s.push(wait_s);
            now_s += wait_s;
        }
        assert_eq!(waits_s, [2.0, 4.0, 8.0, 16.0, 16.0]);
        hold.note_change(2, 1, now_s);
        hold.note_change(1, 2, now_s + FAILED_SPEED_UP_S + 1.0);
        assert!(hold.can_speed_up(now_s + FAILED_SPEED_UP_S + 1.0));
    }

    /// `test_without_a_refresh_the_average_is_used` checks that without vsync the step is the
    /// running average, not snapped.
    #[test]
    fn test_without_a_refresh_the_average_is_used() {
        let steps_s: Vec<f32> = run(&[0.004, 0.006, 0.004, 0.006], None);
        assert!((steps_s[3] - 0.005).abs() < 1e-6, "{steps_s:?}");
    }
}
