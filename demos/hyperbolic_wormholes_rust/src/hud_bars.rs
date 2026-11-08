//! `hud_bars.rs` builds the HUD's bars as overlay rectangles in their look (D85): segments that
//! lean like `/`, empty ones dark glass outlined in the bar's colour, filled ones translucent,
//! glossy, outlined and glowing in it; and soft glows that run along a bar's filled part while
//! its value changes (D89): down from the filled part's end while it falls (a hit, sprinting),
//! up toward it while it rises (refilling), as strong as the value's smoothed rate of change.
//! The shader's `get_rect_layer`, `get_rect_halo` and `get_pulse_glow` draw them.
//!
//! Interface: `make_segmented_bar`, `make_progress_bar`, `BarPulses`, `PulseTuning`,
//! `PULSE_TUNING`, `BAR_SIZE_FRAC`.
//!
//! Example: `let rects = make_segmented_bar([0.03, 0.87], 12, 0.5, [0.35, 0.95, 0.45], pulse);`
//!
//! Must know:
//! - Gotchas: the CPU renderer's window draws these rectangles flat, without their style or pulses.

use crate::render_metal::{MAX_RECT_PULSE_COUNT, NO_RECT_PULSE, OverlayRect, RectPulse, RectStyle};

pub(crate) const BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.016];
const MAX_SEGMENT_COUNT: u32 = 20;
const SEGMENT_GAP_FRAC: f32 = 0.0035;
const FILL_TOLERANCE: f64 = 0.05;
const FILL_OPACITY: f32 = 0.6;
const SLOT_COLOR: [f32; 4] = [0.02, 0.04, 0.08, 0.38];
const SLANT_X_PER_Y: f32 = 0.45;
const CORNER_FRAC: f32 = 0.0015;
const FILL_OUTLINE_FRAC: f32 = 0.0012;
const SLOT_OUTLINE_FRAC: f32 = 0.001;
const FILL_OUTLINE_OPACITY: f32 = 0.85;
const SLOT_OUTLINE_OPACITY: f32 = 0.35;
const FILL_OUTLINE_WHITE_FRAC: f32 = 0.35;
const FILL_GLOSS_FRAC: f32 = 0.3;
const SLOT_GLOSS_FRAC: f32 = 0.15;
const FILL_GLOW_FRAC: f32 = 0.006;
const FILL_SHADE_FRAC: f32 = 0.2;
const REFILL_PULSE_WHITE_FRAC: f32 = 0.65;
const MIN_PULSE_STRENGTH_FRAC: f64 = 0.05;
const PULSE_END_MARGIN_PER_HALF_WIDTH: f64 = 3.0;
const WHITE_RGB: [f32; 3] = [1.0, 1.0, 1.0];

/// `PULSE_TUNING` is how the HUD's bars pulse: the `PulseTuning` the user picked.
pub(crate) const PULSE_TUNING: PulseTuning = PulseTuning {
    smoothing_s: 0.2,
    full_rate_frac_per_s: 0.12,
    use_opacity: 0.7,
    refill_opacity: 0.45,
    slow_speed_frac_per_s: 2.5,
    fast_speed_frac_per_s: 2.5,
    slow_period_s: 0.3,
    fast_period_s: 0.3,
    half_width_frac: 0.01,
    half_height_per_bar_height: 0.8,
};

/// `PulseTuning` is how a bar's pulses behave: the time constant of the moving average whose rate
/// of change drives them; the rate (fractions of the bar per second) at which they are at full
/// strength; their opacity at full strength while the value falls (use) and while it rises
/// (refill); their speed along the bar (fractions of its width per second) and the time between
/// them, at zero and at full strength (in between, in proportion); their half width (of the
/// screen's width) and their half height (of the bar's height).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PulseTuning {
    pub smoothing_s: f64,
    pub full_rate_frac_per_s: f64,
    pub use_opacity: f32,
    pub refill_opacity: f32,
    pub slow_speed_frac_per_s: f64,
    pub fast_speed_frac_per_s: f64,
    pub slow_period_s: f64,
    pub fast_period_s: f64,
    pub half_width_frac: f32,
    pub half_height_per_bar_height: f32,
}

/// `PulseKind` is why pulses run along a bar: its value falls (use: down, from the filled part's
/// end toward the bar's start, in the use colour) or rises (refill: up, from the bar's start
/// toward the filled part's end, in the bar's own colour lightened).
#[derive(Clone, Copy, Debug, PartialEq)]
enum PulseKind {
    Use,
    Refill,
}

/// `BarPulses` is the pulses on one HUD bar: the moving average of its value and that average's
/// rate of change (fractions of the bar per second, negative while it falls), the kind of the
/// pulses running, the time until the next one starts, and where each runs along the bar (0 its
/// start, 1 its end; none for a free slot).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BarPulses {
    smoothed_frac: f64,
    rate_frac_per_s: f64,
    kind: Option<PulseKind>,
    until_next_s: f64,
    positions_frac: [Option<f64>; MAX_RECT_PULSE_COUNT],
}

impl BarPulses {
    /// `New` is a bar at `value_frac` with no pulses, its average settled there.
    ///
    /// Example: `let pulses = BarPulses::New(1.0);`
    #[allow(non_snake_case)]
    pub(crate) fn New(value_frac: f64) -> BarPulses {
        BarPulses {
            smoothed_frac: value_frac,
            rate_frac_per_s: 0.0,
            kind: None,
            until_next_s: 0.0,
            positions_frac: [None; MAX_RECT_PULSE_COUNT],
        }
    }

    /// `update` moves the bar's average `dt_s` toward `value_frac` (an exponential moving average
    /// with `tuning.smoothing_s`), takes its rate of change, moves the running pulses along at the
    /// speed for that strength (`get_strength_frac`), drops those past their end, and starts a new
    /// one each period while the strength is at least `MIN_PULSE_STRENGTH_FRAC`: a use pulse at
    /// the filled part's end of a bar of `segment_count` segments, a refill pulse at its start.
    /// A change of kind drops the pulses of the old one.
    ///
    /// Example: `self.energy_pulses.update(dt_s, energy_frac, 10, &PULSE_TUNING);`
    pub(crate) fn update(
        &mut self,
        dt_s: f64,
        value_frac: f64,
        segment_count: u32,
        tuning: &PulseTuning,
    ) {
        self.smoothed_frac +=
            (value_frac - self.smoothed_frac) * (1.0 - (-dt_s / tuning.smoothing_s).exp());
        self.rate_frac_per_s = (value_frac - self.smoothed_frac) / tuning.smoothing_s;
        let strength_frac: f64 = self.get_strength_frac(tuning);
        let wanted: Option<PulseKind> = if strength_frac < MIN_PULSE_STRENGTH_FRAC {
            None
        } else if self.rate_frac_per_s < 0.0 {
            Some(PulseKind::Use)
        } else {
            Some(PulseKind::Refill)
        };
        if wanted.is_some() && wanted != self.kind {
            (self.kind, self.until_next_s) = (wanted, 0.0);
            self.positions_frac = [None; MAX_RECT_PULSE_COUNT];
        }
        let Some(kind) = self.kind else {
            return;
        };
        let filled_end_frac: f64 = f64::from(get_filled_end_frac(segment_count, value_frac));
        let margin_frac: f64 =
            PULSE_END_MARGIN_PER_HALF_WIDTH * f64::from(tuning.half_width_frac / BAR_SIZE_FRAC[0]);
        let step_frac: f64 = dt_s
            * lerp(
                tuning.slow_speed_frac_per_s,
                tuning.fast_speed_frac_per_s,
                strength_frac,
            );
        for position in self.positions_frac.iter_mut() {
            *position = position
                .map(|x| match kind {
                    PulseKind::Use => x - step_frac,
                    PulseKind::Refill => x + step_frac,
                })
                .filter(|x| *x >= -margin_frac && *x <= filled_end_frac + margin_frac);
        }
        self.until_next_s -= dt_s;
        if wanted.is_none() || self.until_next_s > 0.0 {
            return;
        }
        self.until_next_s = lerp(tuning.slow_period_s, tuning.fast_period_s, strength_frac);
        if let Some(slot) = self.positions_frac.iter_mut().find(|x| x.is_none()) {
            *slot = Some(match kind {
                PulseKind::Use => filled_end_frac,
                PulseKind::Refill => 0.0,
            });
        }
    }

    /// `get_strength_frac` gives how strong the pulses are: the average's rate of change over
    /// `tuning.full_rate_frac_per_s`, at most 1.
    ///
    /// Example: `let strength_frac = self.get_strength_frac(&PULSE_TUNING);`
    fn get_strength_frac(&self, tuning: &PulseTuning) -> f64 {
        (self.rate_frac_per_s.abs() / tuning.full_rate_frac_per_s).min(1.0)
    }

    /// `make_rect_pulse` gives the pulses for the bar starting at `bar_min_frac`, of
    /// `segment_count` segments at `value_frac`: each running one at the strength's share of its
    /// kind's opacity, shown over the filled part only, in `use_rgb` while the value falls or
    /// `bar_rgb` lightened while it rises; `NO_RECT_PULSE` when none runs.
    ///
    /// Example: `let pulse = pulses.make_rect_pulse([0.03, 0.87], 12, 0.75, use_rgb, rgb, &PULSE_TUNING);`
    pub(crate) fn make_rect_pulse(
        &self,
        bar_min_frac: [f32; 2],
        segment_count: u32,
        value_frac: f64,
        use_rgb: [f32; 3],
        bar_rgb: [f32; 3],
        tuning: &PulseTuning,
    ) -> RectPulse {
        let Some(kind) = self.kind else {
            return NO_RECT_PULSE;
        };
        if self.positions_frac.iter().all(Option::is_none) {
            return NO_RECT_PULSE;
        }
        let (rgb, full_opacity) = match kind {
            PulseKind::Use => (use_rgb, tuning.use_opacity),
            PulseKind::Refill => (
                mix_rgb(bar_rgb, WHITE_RGB, REFILL_PULSE_WHITE_FRAC),
                tuning.refill_opacity,
            ),
        };
        let opacity: f32 = full_opacity * self.get_strength_frac(tuning) as f32;
        let x_frac = |along_frac: f64| bar_min_frac[0] + along_frac as f32 * BAR_SIZE_FRAC[0];
        RectPulse {
            centers_x_frac: self.positions_frac.map(|x| x.map_or(0.0, x_frac)),
            opacities: self.positions_frac.map(|x| x.map_or(0.0, |_| opacity)),
            min_x_frac: bar_min_frac[0],
            max_x_frac: x_frac(f64::from(get_filled_end_frac(segment_count, value_frac))),
            half_width_frac: tuning.half_width_frac,
            half_height_frac: tuning.half_height_per_bar_height * BAR_SIZE_FRAC[1],
            color: [rgb[0], rgb[1], rgb[2], 1.0],
        }
    }
}

/// `get_filled_count` gives how many of a bar's `segment_count` segments (at least 1, at most
/// `MAX_SEGMENT_COUNT`) are filled at `filled_frac`: a segment counts once it is more than
/// `FILL_TOLERANCE` full.
///
/// Example: `let filled_count = get_filled_count(12, 0.5);`
fn get_filled_count(segment_count: u32, filled_frac: f64) -> u32 {
    let n: u32 = segment_count.clamp(1, MAX_SEGMENT_COUNT);
    ((filled_frac * f64::from(n) - FILL_TOLERANCE).ceil()).clamp(0.0, f64::from(n)) as u32
}

/// `get_segment_w_frac` gives the width of each of a bar's `segment_count` segments, as a
/// fraction of the screen's width.
///
/// Example: `let segment_w = get_segment_w_frac(12);`
fn get_segment_w_frac(segment_count: u32) -> f32 {
    let n: u32 = segment_count.clamp(1, MAX_SEGMENT_COUNT);
    (BAR_SIZE_FRAC[0] - SEGMENT_GAP_FRAC * (n - 1) as f32) / n as f32
}

/// `get_filled_end_frac` gives where a bar of `segment_count` segments at `filled_frac` stops
/// being filled: the right edge of its last filled segment, as a fraction of the bar's width (0
/// when none is).
///
/// Example: `let end_frac = get_filled_end_frac(12, 0.75);`
fn get_filled_end_frac(segment_count: u32, filled_frac: f64) -> f32 {
    let filled_count: u32 = get_filled_count(segment_count, filled_frac);
    if filled_count == 0 {
        return 0.0;
    }
    let segment_w: f32 = get_segment_w_frac(segment_count);
    (filled_count as f32 * (segment_w + SEGMENT_GAP_FRAC) - SEGMENT_GAP_FRAC) / BAR_SIZE_FRAC[0]
}

/// `make_segmented_bar` gives a HUD bar of `segment_count` separate segments (at most
/// `MAX_SEGMENT_COUNT`) from `min_frac`, `BAR_SIZE_FRAC` in all: enough of them filled in
/// `fill_rgb` to cover `filled_frac` of the bar (`get_filled_count`), the rest empty slots
/// (`make_glass_rect`), the first carrying the bar's `pulse`.
///
/// Example: `let rects = make_segmented_bar([0.03, 0.87], 12, 0.5, [0.35, 0.95, 0.45], pulse);`
pub(crate) fn make_segmented_bar(
    min_frac: [f32; 2],
    segment_count: u32,
    filled_frac: f64,
    fill_rgb: [f32; 3],
    pulse: RectPulse,
) -> Vec<OverlayRect> {
    let n: u32 = segment_count.clamp(1, MAX_SEGMENT_COUNT);
    let filled_count: u32 = get_filled_count(n, filled_frac);
    let segment_w: f32 = get_segment_w_frac(n);
    (0..n)
        .map(|i| {
            let x: f32 = min_frac[0] + i as f32 * (segment_w + SEGMENT_GAP_FRAC);
            make_glass_rect(
                [x, min_frac[1]],
                [x + segment_w, min_frac[1] + BAR_SIZE_FRAC[1]],
                fill_rgb,
                i < filled_count,
                if i == 0 { pulse } else { NO_RECT_PULSE },
            )
        })
        .collect()
}

/// `make_progress_bar` gives a continuous bar from `min_frac` of `size_frac`: an empty slot over
/// all of it and, over its left `filled_frac`, a filled part in `fill_rgb` (none at 0), in the
/// segments' look (`make_glass_rect`).
///
/// Example: `let rects = make_progress_bar([0.4, 0.62], [0.2, 0.02], 0.5, [1.0, 0.85, 0.3]);`
pub(crate) fn make_progress_bar(
    min_frac: [f32; 2],
    size_frac: [f32; 2],
    filled_frac: f32,
    fill_rgb: [f32; 3],
) -> Vec<OverlayRect> {
    let ([x, y], [w, h]) = (min_frac, size_frac);
    let mut rects: Vec<OverlayRect> = vec![make_glass_rect(
        min_frac,
        [x + w, y + h],
        fill_rgb,
        false,
        NO_RECT_PULSE,
    )];
    if filled_frac > 0.0 {
        rects.push(make_glass_rect(
            min_frac,
            [x + w * filled_frac, y + h],
            fill_rgb,
            true,
            NO_RECT_PULSE,
        ));
    }
    rects
}

/// `make_glass_rect` gives one bar rectangle from `min_frac` to `max_frac`, leaning by
/// `SLANT_X_PER_Y`: filled (`is_filled`), `rgb` at `FILL_OPACITY`, shaded, glossy, glowing and
/// outlined in `rgb` lightened; or an empty slot of dark glass (`SLOT_COLOR`) outlined faintly in
/// `rgb`; carrying `pulse`.
///
/// Example: `let rect = make_glass_rect([0.03, 0.87], [0.05, 0.886], rgb, true, NO_RECT_PULSE);`
fn make_glass_rect(
    min_frac: [f32; 2],
    max_frac: [f32; 2],
    rgb: [f32; 3],
    is_filled: bool,
    pulse: RectPulse,
) -> OverlayRect {
    let (color, style) = if is_filled {
        let outline_rgb: [f32; 3] = mix_rgb(rgb, WHITE_RGB, FILL_OUTLINE_WHITE_FRAC);
        (
            [rgb[0], rgb[1], rgb[2], FILL_OPACITY],
            RectStyle {
                corner_frac: CORNER_FRAC,
                outline_frac: FILL_OUTLINE_FRAC,
                gloss_frac: FILL_GLOSS_FRAC,
                shadow_frac: 0.0,
                slant_x_per_y: SLANT_X_PER_Y,
                glow_frac: FILL_GLOW_FRAC,
                shade_frac: FILL_SHADE_FRAC,
                padding: 0.0,
                outline_color: [
                    outline_rgb[0],
                    outline_rgb[1],
                    outline_rgb[2],
                    FILL_OUTLINE_OPACITY,
                ],
            },
        )
    } else {
        (
            SLOT_COLOR,
            RectStyle {
                corner_frac: CORNER_FRAC,
                outline_frac: SLOT_OUTLINE_FRAC,
                gloss_frac: SLOT_GLOSS_FRAC,
                shadow_frac: 0.0,
                slant_x_per_y: SLANT_X_PER_Y,
                glow_frac: 0.0,
                shade_frac: 0.0,
                padding: 0.0,
                outline_color: [rgb[0], rgb[1], rgb[2], SLOT_OUTLINE_OPACITY],
            },
        )
    };
    OverlayRect {
        min_frac,
        max_frac,
        color,
        style,
        pulse,
    }
}

/// `mix_rgb` gives the colour `t_frac` of the way from `a` to `b`.
///
/// Example: `let pale = mix_rgb(rgb, [1.0, 1.0, 1.0], 0.5);`
fn mix_rgb(a: [f32; 3], b: [f32; 3], t_frac: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t_frac)
}

/// `lerp` gives the value `t_frac` of the way from `a` to `b`.
///
/// Example: `let speed = lerp(1.0, 3.0, 0.5);`
fn lerp(a: f64, b: f64, t_frac: f64) -> f64 {
    a + (b - a) * t_frac
}

#[cfg(test)]
mod tests {
    //! Tests of building the bars and running their pulses.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Segmented bar's filled fraction | none, a sliver, all but one (after rounding to a byte), all | `segmented_bars_count_whole_segments` |
    //! | Segmented bar's segment count | one block, several | `segmented_bars_count_whole_segments` |
    //! | Which segment carries the pulse | the first, the others | `segmented_bars_count_whole_segments` |
    //! | Filled part's end | none filled, part, all | `the_filled_part_ends_after_its_last_segment` |
    //! | Progress bar's filled fraction | none (slot only), part, all | `progress_bars_fill_from_the_left` |
    //! | Value over time | steady, a step down (a hit), falling steadily (sprinting), rising steadily (refilling) | `pulses_follow_the_smoothed_rate` |
    //! | Pulse kind | use (from the filled end, down), refill (from the start, up, paler) | `pulses_follow_the_smoothed_rate`, `pulses_show_only_over_the_filled_part` |
    //! | Pulse's place | running, past its end (dropped) | `pulses_follow_the_smoothed_rate` |
    //! | Strength | none (no pulse), part (in between speed and period), full | `stronger_changes_pulse_faster_and_more_often` |

    use super::*;

    const RGB: [f32; 3] = [0.35, 0.95, 0.45];
    const USE_RGB: [f32; 3] = [1.0, 0.45, 0.2];
    const BAR_MIN_FRAC: [f32; 2] = [0.03, 0.87];
    const SEGMENT_COUNT: u32 = 12;
    const DT_S: f64 = 1.0 / 60.0;
    const TOLERANCE: f64 = 1e-6;

    #[test]
    fn segmented_bars_count_whole_segments() {
        let filled = |frac: f64| -> usize {
            make_segmented_bar([0.0, 0.0], SEGMENT_COUNT, frac, RGB, NO_RECT_PULSE)
                .iter()
                .filter(|rect| rect.color[3] == FILL_OPACITY)
                .count()
        };
        let eleven_twelfths_frac: f64 =
            (11.0 / 12.0 * f64::from(u8::MAX)).round() / f64::from(u8::MAX);
        assert_eq!(
            [0.0, 0.01, eleven_twelfths_frac, 1.0].map(filled),
            [0, 1, 11, 12]
        );
        for (frac, color) in [
            (0.0, SLOT_COLOR),
            (1.0, [RGB[0], RGB[1], RGB[2], FILL_OPACITY]),
        ] {
            let block = make_segmented_bar([0.0, 0.0], 1, frac, RGB, NO_RECT_PULSE);
            assert_eq!(block.len(), 1);
            assert_eq!(block[0].color, color);
            assert_eq!(block[0].max_frac[0], BAR_SIZE_FRAC[0]);
            assert_eq!(block[0].style.slant_x_per_y, SLANT_X_PER_Y);
        }
        let pulse: RectPulse = RectPulse {
            opacities: [1.0; MAX_RECT_PULSE_COUNT],
            ..NO_RECT_PULSE
        };
        let rects: Vec<OverlayRect> =
            make_segmented_bar([0.0, 0.0], SEGMENT_COUNT, 0.5, RGB, pulse);
        assert_eq!(rects[0].pulse, pulse);
        assert!(rects[1..].iter().all(|rect| rect.pulse == NO_RECT_PULSE));
    }

    #[test]
    fn the_filled_part_ends_after_its_last_segment() {
        assert_eq!(get_filled_end_frac(SEGMENT_COUNT, 0.0), 0.0);
        assert!((get_filled_end_frac(SEGMENT_COUNT, 1.0) - 1.0).abs() < 1e-6);
        let segment_w: f32 = get_segment_w_frac(SEGMENT_COUNT);
        let expected_frac: f32 = (6.0 * segment_w + 5.0 * SEGMENT_GAP_FRAC) / BAR_SIZE_FRAC[0];
        assert!((get_filled_end_frac(SEGMENT_COUNT, 0.5) - expected_frac).abs() < 1e-6);
    }

    #[test]
    fn progress_bars_fill_from_the_left() {
        const SIZE_FRAC: [f32; 2] = [0.2, 0.02];
        assert_eq!(make_progress_bar([0.4, 0.6], SIZE_FRAC, 0.0, RGB).len(), 1);
        for filled_frac in [0.5, 1.0] {
            let rects: Vec<OverlayRect> =
                make_progress_bar([0.4, 0.6], SIZE_FRAC, filled_frac, RGB);
            assert_eq!(rects.len(), 2);
            assert_eq!(rects[0].color, SLOT_COLOR);
            assert_eq!(rects[0].max_frac[0], 0.4 + SIZE_FRAC[0]);
            assert_eq!(rects[1].min_frac, [0.4, 0.6]);
            assert_eq!(rects[1].max_frac[0], 0.4 + SIZE_FRAC[0] * filled_frac);
            assert_eq!(rects[1].color[3], FILL_OPACITY);
        }
    }

    /// `run` updates `pulses` for `duration_s` at `DT_S` steps with the value `value_at(t_s)`.
    ///
    /// Example: `run(&mut pulses, 0.5, |_| 1.0);`
    fn run(pulses: &mut BarPulses, duration_s: f64, value_at: impl Fn(f64) -> f64) {
        for i in 0..(duration_s / DT_S).round() as usize {
            pulses.update(
                DT_S,
                value_at(i as f64 * DT_S),
                SEGMENT_COUNT,
                &PULSE_TUNING,
            );
        }
    }

    #[test]
    fn pulses_follow_the_smoothed_rate() {
        let mut pulses: BarPulses = BarPulses::New(1.0);
        run(&mut pulses, 1.0, |_| 1.0);
        assert_eq!(pulses, BarPulses::New(1.0), "steady");
        let after_hit_frac: f64 = 11.0 / 12.0;
        pulses.update(DT_S, after_hit_frac, SEGMENT_COUNT, &PULSE_TUNING);
        let end_frac: f64 = f64::from(get_filled_end_frac(SEGMENT_COUNT, after_hit_frac));
        assert_eq!(pulses.kind, Some(PulseKind::Use));
        assert_eq!(
            pulses.positions_frac[0],
            Some(end_frac),
            "starts at the last hit point"
        );
        run(&mut pulses, 0.1, |_| after_hit_frac);
        let x_frac: f64 = pulses.positions_frac[0].expect("still running");
        assert!(x_frac < end_frac, "runs down: {x_frac}");
        run(&mut pulses, 3.0, |_| after_hit_frac);
        assert!(
            pulses.get_strength_frac(&PULSE_TUNING) < MIN_PULSE_STRENGTH_FRAC,
            "a hit is a burst"
        );
        assert!(
            pulses.positions_frac.iter().all(Option::is_none),
            "all past the start"
        );
        let drain_frac_per_s: f64 = 0.1;
        run(&mut pulses, 2.0, |t_s| 0.9 - drain_frac_per_s * t_s);
        assert_eq!(pulses.kind, Some(PulseKind::Use));
        assert!(
            (pulses.rate_frac_per_s + drain_frac_per_s).abs() < 0.01,
            "a steady drain gives a steady rate: {}",
            pulses.rate_frac_per_s
        );
        run(&mut pulses, 2.0, |t_s| 0.3 + drain_frac_per_s * t_s);
        assert_eq!(pulses.kind, Some(PulseKind::Refill));
        assert!(pulses.positions_frac.iter().flatten().all(|x| *x >= 0.0));
    }

    #[test]
    fn pulses_show_only_over_the_filled_part() {
        let mut pulses: BarPulses = BarPulses::New(0.25);
        assert_eq!(
            pulses.make_rect_pulse(
                BAR_MIN_FRAC,
                SEGMENT_COUNT,
                0.25,
                USE_RGB,
                RGB,
                &PULSE_TUNING
            ),
            NO_RECT_PULSE
        );
        run(&mut pulses, 0.5, |t_s| 0.25 + 0.2 * t_s);
        let value_frac: f64 = 0.35;
        let pulse: RectPulse = pulses.make_rect_pulse(
            BAR_MIN_FRAC,
            SEGMENT_COUNT,
            value_frac,
            USE_RGB,
            RGB,
            &PULSE_TUNING,
        );
        assert_eq!(pulse.min_x_frac, BAR_MIN_FRAC[0]);
        let end_frac: f32 = get_filled_end_frac(SEGMENT_COUNT, value_frac);
        assert!((pulse.max_x_frac - (BAR_MIN_FRAC[0] + end_frac * BAR_SIZE_FRAC[0])).abs() < 1e-6);
        assert!(pulse.opacities.iter().any(|a| *a > 0.0));
        assert!(
            pulse
                .opacities
                .iter()
                .all(|a| *a <= PULSE_TUNING.refill_opacity)
        );
        assert!(
            (0..3).all(|k| pulse.color[k] >= RGB[k]),
            "refill pulses are paler"
        );
    }

    #[test]
    fn stronger_changes_pulse_faster_and_more_often() {
        let tuning: PulseTuning = PulseTuning {
            slow_speed_frac_per_s: 1.0,
            fast_speed_frac_per_s: 3.0,
            slow_period_s: 0.6,
            fast_period_s: 0.2,
            ..PULSE_TUNING
        };
        let mut pulses: BarPulses = BarPulses::New(0.5);
        pulses.rate_frac_per_s = -tuning.full_rate_frac_per_s / 2.0;
        pulses.smoothed_frac = 0.5 - pulses.rate_frac_per_s * tuning.smoothing_s;
        pulses.update(DT_S, 0.5, SEGMENT_COUNT, &tuning);
        let strength_frac: f64 = pulses.get_strength_frac(&tuning);
        assert!(
            strength_frac > 0.0 && strength_frac < 1.0,
            "{strength_frac}"
        );
        assert!(
            (pulses.until_next_s - lerp(0.6, 0.2, strength_frac)).abs() < TOLERANCE,
            "period in between"
        );
        let start_frac: f64 = pulses.positions_frac[0].expect("just started");
        pulses.update(DT_S, 0.5, SEGMENT_COUNT, &tuning);
        let moved_frac: f64 = start_frac - pulses.positions_frac[0].expect("running");
        let speed_frac_per_s: f64 = moved_frac / DT_S;
        assert!(
            speed_frac_per_s > 1.0 && speed_frac_per_s < 3.0,
            "{speed_frac_per_s}"
        );
    }
}
