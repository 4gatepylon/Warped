//! `level_link.rs` is the app's side of a level playing in its window (`app::MatchLink`): once a
//! frame the level asks it whether the round is over, and it polls the network there, so the
//! match server keeps hearing this player; it gives the level's menu the match's status. With a
//! `Tour` (`warped --tour-levels`), it also times the level's frames and presses the keys that
//! show the map, open the menu and leave the level. With a `FrameTimer` (a person playing), it
//! times the level's frames for the play log (`play_log.rs`).
//!
//! Interface: `AppLink` (`New`), `Tour` (`New`, `on_frame`, `get_frame_times`), `TourStep`,
//! `FrameTimer` (`New`, `on_frame`, `get_frame_times`), `summarise_frame_times`, `FrameSummary`,
//! `format_frame_row`, `FRAME_TABLE_HEADER`.
//!
//! Example: `let mut link = AppLink::New(&mut app, Some(&mut tour), None);`

use std::time::Instant;

use sdl3::keyboard::Scancode;
use sdl3::sys::events::{SDL_EVENT_KEY_DOWN, SDL_Event, SDL_KeyboardEvent, SDL_PushEvent};
use warped_shared::app::MatchLink;
use warped_shared::match_protocol::MatchView;

use crate::app_state::App;

const MS_PER_S: f64 = 1000.0;
const PERCENT: f64 = 100.0;
const P99_PCT: f64 = 99.0;
const TARGET_FRAME_MS: f64 = 1000.0 / 60.0;

/// `FRAME_TABLE_HEADER` is the head of a table of `format_frame_row`'s rows.
pub const FRAME_TABLE_HEADER: &str = "| Level | Frames | Median ms | p99 ms | Max ms | p99 under 16.7 ms |\n| --- | --- | --- | --- | --- | --- |\n";

/// `TourStep` is what a toured level does next, after how long in the level: wait while it loads
/// and settles, time its frames, open the menu, then leave by its QUIT page. (It can't hold the
/// map key: a level reads that from the keyboard's state, which a queued key event doesn't change.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TourStep {
    Settle,
    Time,
    OpenMenu,
    Leave,
    Done,
}

/// `Tour` is a scripted visit of a level: how long each step lasts, when the level started, the
/// frame times measured in the `Time` step, and the step reached.
pub struct Tour {
    settle_s: f64,
    time_s: f64,
    step_s: f64,
    started_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    frame_times_ms: Vec<f64>,
    step: TourStep,
}

impl Tour {
    /// `New` makes a tour that settles `settle_s`, times frames for `time_s`, and spends `step_s`
    /// on each key step.
    ///
    /// Example: `let tour = Tour::New(8.0, 10.0, 1.0);`
    #[allow(non_snake_case)]
    pub fn New(settle_s: f64, time_s: f64, step_s: f64) -> Tour {
        Tour {
            settle_s,
            time_s,
            step_s,
            started_at: None,
            last_frame_at: None,
            frame_times_ms: Vec::new(),
            step: TourStep::Settle,
        }
    }

    /// `get_frame_times` gives the frame times measured, in ms.
    ///
    /// Example: `let times = tour.get_frame_times();`
    pub fn get_frame_times(&self) -> &[f64] {
        &self.frame_times_ms
    }

    /// `get_step` gives the step reached.
    ///
    /// Example: `if tour.get_step() == TourStep::Done { ... }`
    #[cfg(test)]
    fn get_step(&self) -> TourStep {
        self.step
    }

    /// `get_step_at` gives the step due `elapsed_s` into the level.
    ///
    /// Example: `assert_eq!(tour.get_step_at(0.0), TourStep::Settle);`
    fn get_step_at(&self, elapsed_s: f64) -> TourStep {
        let time_end_s: f64 = self.settle_s + self.time_s;
        match elapsed_s {
            s if s < self.settle_s => TourStep::Settle,
            s if s < time_end_s => TourStep::Time,
            s if s < time_end_s + self.step_s => TourStep::OpenMenu,
            s if s < time_end_s + 2.0 * self.step_s => TourStep::Leave,
            _ => TourStep::Done,
        }
    }

    /// `on_frame` counts a frame at `now`: it records its time in the `Time` step and, on entering
    /// a key step, presses that step's keys (Escape for the menu; Q and Y on the menu's QUIT page
    /// to leave). It gives the step entered, if one began.
    ///
    /// Example: `if let Some(step) = tour.on_frame(Instant::now()) { ... }`
    pub fn on_frame(&mut self, now: Instant) -> Option<TourStep> {
        let started_at: Instant = *self.started_at.get_or_insert(now);
        let step: TourStep = self.get_step_at(now.duration_since(started_at).as_secs_f64());
        if step == TourStep::Time
            && let Some(last) = self.last_frame_at
        {
            self.frame_times_ms
                .push(now.duration_since(last).as_secs_f64() * MS_PER_S);
        }
        self.last_frame_at = Some(now);
        if step == self.step {
            return None;
        }
        self.step = step;
        match step {
            TourStep::OpenMenu => push_key_down(Scancode::Escape),
            TourStep::Leave => {
                push_key_down(Scancode::Q);
                push_key_down(Scancode::Y);
            }
            TourStep::Settle | TourStep::Time | TourStep::Done => {}
        }
        Some(step)
    }
}

/// `push_key_down` queues `scancode` going down, as the keyboard would, through SDL's own
/// `SDL_PushEvent`.
///
/// Example: `push_key_down(Scancode::Escape);`
fn push_key_down(scancode: Scancode) {
    let key: SDL_KeyboardEvent = SDL_KeyboardEvent {
        r#type: SDL_EVENT_KEY_DOWN,
        scancode: scancode.into(),
        down: true,
        repeat: false,
        ..Default::default()
    };
    let mut event: SDL_Event = SDL_Event { key };
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN SDL READING AN EVENT AS THE WRONG TYPE WILL HAPPEN
    let is_pushed: bool = unsafe { SDL_PushEvent(&mut event) };
    assert!(is_pushed, "SDL didn't queue the key: {}", sdl3::get_error());
}

/// `FrameSummary` is a level's frame times: how many, the median, the 99th percentile and the
/// slowest, in ms.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FrameSummary {
    pub frame_count: usize,
    pub median_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// `summarise_frame_times` gives the summary of `times_ms`, or `None` for no frames.
///
/// Example: `let summary = summarise_frame_times(&[10.0, 11.0]);`
pub fn summarise_frame_times(times_ms: &[f64]) -> Option<FrameSummary> {
    if times_ms.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = times_ms.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |pct: f64| sorted[((sorted.len() - 1) as f64 * pct / PERCENT).round() as usize];
    Some(FrameSummary {
        frame_count: sorted.len(),
        median_ms: at(PERCENT / 2.0),
        p99_ms: at(P99_PCT),
        max_ms: sorted[sorted.len() - 1],
    })
}

/// `format_frame_row` gives the table row of the level `name`'s frame times, ending in a newline:
/// its `summary`, and whether its 99th percentile is inside 60 fps; or a row saying it had no
/// frames.
///
/// Example: `let row: String = format_frame_row("Dust", summarise_frame_times(&[10.0]));`
pub fn format_frame_row(name: &str, summary: Option<FrameSummary>) -> String {
    match summary {
        Some(s) => format!(
            "| {name} | {} | {:.2} | {:.2} | {:.2} | {} |\n",
            s.frame_count,
            s.median_ms,
            s.p99_ms,
            s.max_ms,
            if s.p99_ms < TARGET_FRAME_MS {
                "yes"
            } else {
                "NO"
            }
        ),
        None => format!("| {name} | 0 | - | - | - | no frames |\n"),
    }
}

/// `FrameTimer` times the frames of a level a person plays: how long the level loads and settles
/// before frames count, when it started, and the frame times since, in ms.
pub struct FrameTimer {
    settle_s: f64,
    started_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    frame_times_ms: Vec<f64>,
}

impl FrameTimer {
    /// `New` makes a timer that counts frames from `settle_s` into the level.
    ///
    /// Example: `let mut timer = FrameTimer::New(5.0);`
    #[allow(non_snake_case)]
    pub fn New(settle_s: f64) -> FrameTimer {
        FrameTimer {
            settle_s,
            started_at: None,
            last_frame_at: None,
            frame_times_ms: Vec::new(),
        }
    }

    /// `on_frame` counts a frame at `now`, recording the time since the frame before it once the
    /// level has settled.
    ///
    /// Example: `timer.on_frame(Instant::now());`
    pub fn on_frame(&mut self, now: Instant) {
        let started_at: Instant = *self.started_at.get_or_insert(now);
        if now.duration_since(started_at).as_secs_f64() >= self.settle_s
            && let Some(last) = self.last_frame_at
        {
            self.frame_times_ms
                .push(now.duration_since(last).as_secs_f64() * MS_PER_S);
        }
        self.last_frame_at = Some(now);
    }

    /// `get_frame_times` gives the frame times measured, in ms.
    ///
    /// Example: `let times = timer.get_frame_times();`
    pub fn get_frame_times(&self) -> &[f64] {
        &self.frame_times_ms
    }
}

/// `AppLink` is the `MatchLink` of a level the app plays: the app, the tour if one runs, and the
/// timer of a person's play.
pub struct AppLink<'a> {
    app: &'a mut App,
    tour: Option<&'a mut Tour>,
    timer: Option<&'a mut FrameTimer>,
}

impl<'a> AppLink<'a> {
    /// `New` links a level to `app`, with `tour` scripting it and `timer` timing it if given.
    ///
    /// Example: `let mut link = AppLink::New(&mut app, None, Some(&mut timer));`
    #[allow(non_snake_case)]
    pub fn New(
        app: &'a mut App,
        tour: Option<&'a mut Tour>,
        timer: Option<&'a mut FrameTimer>,
    ) -> AppLink<'a> {
        AppLink { app, tour, timer }
    }
}

impl MatchLink for AppLink<'_> {
    /// `is_round_over` polls the network and says whether the round the level plays has ended;
    /// a tour also counts the frame and presses its keys here, and a timer counts the frame.
    ///
    /// Example: `if link.is_round_over() { ... }`
    fn is_round_over(&mut self) -> bool {
        let now: Instant = Instant::now();
        if let Some(tour) = &mut self.tour {
            tour.on_frame(now);
        }
        if let Some(timer) = &mut self.timer {
            timer.on_frame(now);
        }
        self.app.poll_network();
        self.app.is_level_round_over()
    }

    /// `get_match_view` gives the match's latest view.
    ///
    /// Example: `let view = link.get_match_view();`
    fn get_match_view(&self) -> Option<&MatchView> {
        self.app.get_level_view()
    }

    /// `get_match_lines` gives the level menu's MATCH page.
    ///
    /// Example: `menu.set_match_lines(link.get_match_lines());`
    fn get_match_lines(&self) -> Option<Vec<(String, bool)>> {
        self.app.get_match_lines()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `level_link.rs` (the keys a tour presses need SDL, so the steps are tested
    //! through `get_step_at` and the frame times through a tour that stays in `Time`).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Tour steps | before settling, timing, each key step, done | `tour_steps_follow_the_clock` |
    //! | Frame times | recorded only while timing | `frames_are_timed_only_while_timing` |
    //! | Summary | none, one, many (median, p99, max) | `frame_times_summarise` |
    //! | A person's frames | while the level settles (not timed), after it (timed) | `a_persons_frames_are_timed_once_the_level_settled` |
    //! | Table row | inside 60 fps, outside it, no frames | `rows_say_whether_frames_are_inside_60_fps` |

    use super::*;
    use std::time::Duration;

    const TOUR_FRAME_STEP: Duration = Duration::from_millis(10);

    #[test]
    fn tour_steps_follow_the_clock() {
        let tour: Tour = Tour::New(2.0, 3.0, 1.0);
        for (elapsed_s, step) in [
            (0.0, TourStep::Settle),
            (2.5, TourStep::Time),
            (5.5, TourStep::OpenMenu),
            (6.5, TourStep::Leave),
            (7.5, TourStep::Done),
        ] {
            assert_eq!(tour.get_step_at(elapsed_s), step, "{elapsed_s}");
        }
    }

    #[test]
    fn frames_are_timed_only_while_timing() {
        let mut tour: Tour = Tour::New(0.05, 100.0, 1.0);
        let start: Instant = Instant::now();
        for i in 0..20 {
            tour.on_frame(start + TOUR_FRAME_STEP * i);
        }
        assert_eq!(tour.get_step(), TourStep::Time);
        assert!(tour.get_frame_times().len() >= 13 && tour.get_frame_times().len() <= 15);
        assert!(
            tour.get_frame_times()
                .iter()
                .all(|&ms| (ms - 10.0).abs() < 1e-6)
        );
    }

    #[test]
    fn a_persons_frames_are_timed_once_the_level_settled() {
        let mut timer: FrameTimer = FrameTimer::New(0.05);
        let start: Instant = Instant::now();
        for i in 0..5 {
            timer.on_frame(start + TOUR_FRAME_STEP * i);
        }
        assert!(timer.get_frame_times().is_empty());
        for i in 5..20 {
            timer.on_frame(start + TOUR_FRAME_STEP * i);
        }
        assert_eq!(timer.get_frame_times().len(), 15);
        assert!(
            timer
                .get_frame_times()
                .iter()
                .all(|&ms| (ms - 10.0).abs() < 1e-6)
        );
    }

    #[test]
    fn rows_say_whether_frames_are_inside_60_fps() {
        assert_eq!(
            format_frame_row("Dust", summarise_frame_times(&[10.0, 12.0])),
            "| Dust | 2 | 12.00 | 12.00 | 12.00 | yes |\n"
        );
        assert_eq!(
            format_frame_row("Dust", summarise_frame_times(&[20.0])),
            "| Dust | 1 | 20.00 | 20.00 | 20.00 | NO |\n"
        );
        assert_eq!(
            format_frame_row("Dust", None),
            "| Dust | 0 | - | - | - | no frames |\n"
        );
    }

    #[test]
    fn frame_times_summarise() {
        assert_eq!(summarise_frame_times(&[]), None);
        let one: FrameSummary = summarise_frame_times(&[12.0]).unwrap();
        assert_eq!((one.median_ms, one.p99_ms, one.max_ms), (12.0, 12.0, 12.0));
        let times: Vec<f64> = (1..=100).map(f64::from).collect();
        let many: FrameSummary = summarise_frame_times(&times).unwrap();
        assert_eq!(many.frame_count, 100);
        assert_eq!(many.median_ms, 51.0);
        assert_eq!(many.p99_ms, 99.0);
        assert_eq!(many.max_ms, 100.0);
    }
}
