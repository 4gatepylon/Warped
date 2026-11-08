//! `hud_bars.rs` builds the HUD's bars as overlay rectangles in their look (D85): segments that
//! lean like `/`, empty ones dark glass outlined in the bar's colour, filled ones translucent,
//! glossy, outlined and glowing in it; and a pulse that runs along a bar while it is used (a hit
//! on health, sprinting on energy: down, right to left) or refills (up, left to right, fainter).
//! The shader's `get_rect_layer` and `get_rect_halo` draw them.
//!
//! Interface: `make_segmented_bar`, `make_progress_bar`, `BarPulse`, `PulseKind`,
//! `get_health_pulse_kind`, `get_energy_pulse_kind`, `BAR_SIZE_FRAC`.
//!
//! Example: `let rects = make_segmented_bar([0.03, 0.87], 12, 0.5, [0.35, 0.95, 0.45], pulse);`
//!
//! Must know:
//! - Gotchas: the CPU renderer's window draws these rectangles flat, without their style or pulse.

use crate::render_metal::{NO_RECT_PULSE, OverlayRect, RectPulse, RectStyle};

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
const USE_PULSE_TRAVEL_S: f64 = 0.5;
const USE_PULSE_PERIOD_S: f64 = 0.8;
const USE_PULSE_OPACITY: f32 = 0.8;
const REFILL_PULSE_TRAVEL_S: f64 = 1.0;
const REFILL_PULSE_PERIOD_S: f64 = 1.6;
const REFILL_PULSE_OPACITY: f32 = 0.4;
const REFILL_PULSE_WHITE_FRAC: f32 = 0.65;
const PULSE_HALF_WIDTH_FRAC: f32 = 0.02;
const PULSE_MARGIN_PER_HALF_WIDTH: f32 = 2.5;
const WHITE_RGB: [f32; 3] = [1.0, 1.0, 1.0];

/// `PulseKind` is why a pulse runs along a bar: the bar is being used up (down, right to left,
/// in the use colour) or refills (up, left to right, fainter, in the bar's own colour lightened).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PulseKind {
    Use,
    Refill,
}

impl PulseKind {
    /// `get_travel_s` gives how long a pulse of this kind takes to cross its bar.
    ///
    /// Example: `let travel_s = PulseKind::Use.get_travel_s();`
    fn get_travel_s(self) -> f64 {
        match self {
            PulseKind::Use => USE_PULSE_TRAVEL_S,
            PulseKind::Refill => REFILL_PULSE_TRAVEL_S,
        }
    }

    /// `get_period_s` gives how long after a pulse of this kind starts the next can start: its
    /// travel and then a pause.
    ///
    /// Example: `let period_s = PulseKind::Refill.get_period_s();`
    fn get_period_s(self) -> f64 {
        match self {
            PulseKind::Use => USE_PULSE_PERIOD_S,
            PulseKind::Refill => REFILL_PULSE_PERIOD_S,
        }
    }
}

/// `BarPulse` is the pulse on one HUD bar: its kind (none when no pulse runs) and how long ago it
/// started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BarPulse {
    kind: Option<PulseKind>,
    age_s: f64,
}

impl BarPulse {
    /// `Idle` is a bar with no pulse.
    ///
    /// Example: `let pulse = BarPulse::Idle();`
    #[allow(non_snake_case)]
    pub(crate) fn Idle() -> BarPulse {
        BarPulse {
            kind: None,
            age_s: 0.0,
        }
    }

    /// `start` starts a pulse of `kind` at once, replacing any pulse on its way (each hit sends
    /// one down the health bar).
    ///
    /// Example: `self.health_pulse.start(PulseKind::Use);`
    pub(crate) fn start(&mut self, kind: PulseKind) {
        (self.kind, self.age_s) = (Some(kind), 0.0);
    }

    /// `update` ages the pulse by `dt_s`. Once its period is over, or at once when `wanted` is a
    /// use pulse and a refill pulse runs, the next pulse is `wanted`'s (none if it is `None`), so
    /// pulses repeat for as long as their cause lasts.
    ///
    /// Example: `self.energy_pulse.update(dt_s, Some(PulseKind::Use));`
    pub(crate) fn update(&mut self, dt_s: f64, wanted: Option<PulseKind>) {
        if let Some(kind) = self.kind {
            self.age_s += dt_s;
            let is_over: bool = self.age_s >= kind.get_period_s();
            let is_overtaken: bool = kind == PulseKind::Refill && wanted == Some(PulseKind::Use);
            if !is_over && !is_overtaken {
                return;
            }
            *self = BarPulse::Idle();
        }
        if let Some(kind) = wanted {
            self.start(kind);
        }
    }

    /// `make_rect_pulse` gives the pulse for the rectangles of a bar starting at `bar_min_x_frac`
    /// across the screen: a band `PULSE_HALF_WIDTH_FRAC` wide, crossing the bar from
    /// `PULSE_MARGIN_PER_HALF_WIDTH` half widths outside one end to as far outside the other in
    /// its kind's travel time, in `use_rgb` (a use pulse) or `bar_rgb` lightened (a refill pulse);
    /// `NO_RECT_PULSE` when none is crossing.
    ///
    /// Example: `let pulse = self.health_pulse.make_rect_pulse(0.03, [1.0, 0.38, 0.12], rgb);`
    pub(crate) fn make_rect_pulse(
        &self,
        bar_min_x_frac: f32,
        use_rgb: [f32; 3],
        bar_rgb: [f32; 3],
    ) -> RectPulse {
        let Some(kind) = self.kind else {
            return NO_RECT_PULSE;
        };
        let travel_frac: f32 = (self.age_s / kind.get_travel_s()) as f32;
        if travel_frac >= 1.0 {
            return NO_RECT_PULSE;
        }
        let margin_frac: f32 = PULSE_HALF_WIDTH_FRAC * PULSE_MARGIN_PER_HALF_WIDTH;
        let span_frac: f32 = BAR_SIZE_FRAC[0] + 2.0 * margin_frac;
        let (along_frac, rgb, opacity) = match kind {
            PulseKind::Use => (
                BAR_SIZE_FRAC[0] + margin_frac - travel_frac * span_frac,
                use_rgb,
                USE_PULSE_OPACITY,
            ),
            PulseKind::Refill => (
                travel_frac * span_frac - margin_frac,
                mix_rgb(bar_rgb, WHITE_RGB, REFILL_PULSE_WHITE_FRAC),
                REFILL_PULSE_OPACITY,
            ),
        };
        RectPulse {
            center_x_frac: bar_min_x_frac + along_frac,
            half_width_frac: PULSE_HALF_WIDTH_FRAC,
            padding: [0.0; 2],
            color: [rgb[0], rgb[1], rgb[2], opacity],
        }
    }
}

/// `get_health_pulse_kind` gives the pulse a change of health from `last_health_frac` to
/// `health_frac` starts: a use pulse when it fell (a hit), a refill pulse when it rose (a
/// respawn or a new game), none when it stayed.
///
/// Example: `let kind = get_health_pulse_kind(0.75, 1.0);`
pub(crate) fn get_health_pulse_kind(health_frac: f64, last_health_frac: f64) -> Option<PulseKind> {
    if health_frac < last_health_frac {
        Some(PulseKind::Use)
    } else if health_frac > last_health_frac {
        Some(PulseKind::Refill)
    } else {
        None
    }
}

/// `get_energy_pulse_kind` gives the pulse the sprint energy wants: a use pulse while the player
/// sprints, a refill pulse while the energy rises from `last_energy_frac` to `energy_frac`, none
/// otherwise (full, or empty with sprint still held).
///
/// Example: `let kind = get_energy_pulse_kind(false, 0.5, 0.49);`
pub(crate) fn get_energy_pulse_kind(
    is_sprinting: bool,
    energy_frac: f64,
    last_energy_frac: f64,
) -> Option<PulseKind> {
    if is_sprinting {
        Some(PulseKind::Use)
    } else if energy_frac > last_energy_frac {
        Some(PulseKind::Refill)
    } else {
        None
    }
}

/// `make_segmented_bar` gives a HUD bar of `segment_count` separate segments (at most
/// `MAX_SEGMENT_COUNT`) from `min_frac`, `BAR_SIZE_FRAC` in all: enough of them filled in
/// `fill_rgb` to cover `filled_frac` of the bar (a segment counts once it is more than
/// `FILL_TOLERANCE` full), the rest empty slots (`make_glass_rect`), all with `pulse` across them.
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
    let filled_count: f64 = (filled_frac * f64::from(n) - FILL_TOLERANCE).ceil();
    let [w, h] = BAR_SIZE_FRAC;
    let segment_w: f32 = (w - SEGMENT_GAP_FRAC * (n - 1) as f32) / n as f32;
    (0..n)
        .map(|i| {
            let x: f32 = min_frac[0] + i as f32 * (segment_w + SEGMENT_GAP_FRAC);
            make_glass_rect(
                [x, min_frac[1]],
                [x + segment_w, min_frac[1] + h],
                fill_rgb,
                f64::from(i) < filled_count,
                pulse,
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
/// `rgb`; with `pulse` across it.
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

#[cfg(test)]
mod tests {
    //! Tests of building the bars and running their pulses.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Segmented bar's filled fraction | none, a sliver, all but one (after rounding to a byte), all | `segmented_bars_count_whole_segments` |
    //! | Segmented bar's segment count | one block, several | `segmented_bars_count_whole_segments` |
    //! | Progress bar's filled fraction | none (slot only), part, all | `progress_bars_fill_from_the_left` |
    //! | Pulse kind | use (right to left), refill (left to right, fainter) | `use_pulses_run_down_and_refill_pulses_run_up` |
    //! | Pulse's age | crossing, crossed (in its pause), period over | `pulses_repeat_while_wanted_and_stop_when_not` |
    //! | Wanted after a pulse | same kind (repeats), none (stops), use over a running refill (at once), refill over a running use (after it) | `pulses_repeat_while_wanted_and_stop_when_not`, `use_pulses_overtake_refill_pulses` |
    //! | Started while running | another hit (restarts) | `use_pulses_overtake_refill_pulses` |
    //! | Health change | fell, rose, stayed | `health_and_energy_changes_pick_pulses` |
    //! | Energy | sprinting, rising, staying or falling | `health_and_energy_changes_pick_pulses` |

    use super::*;

    const RGB: [f32; 3] = [0.35, 0.95, 0.45];
    const USE_RGB: [f32; 3] = [1.0, 0.38, 0.12];
    const BAR_MIN_X_FRAC: f32 = 0.03;
    const STEP_S: f64 = 0.1;

    #[test]
    fn segmented_bars_count_whole_segments() {
        const SEGMENT_COUNT: u32 = 12;
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

    #[test]
    fn use_pulses_run_down_and_refill_pulses_run_up() {
        assert_eq!(
            BarPulse::Idle().make_rect_pulse(BAR_MIN_X_FRAC, USE_RGB, RGB),
            NO_RECT_PULSE
        );
        let bar_middle_x_frac: f32 = BAR_MIN_X_FRAC + BAR_SIZE_FRAC[0] / 2.0;
        for (kind, is_down) in [(PulseKind::Use, true), (PulseKind::Refill, false)] {
            let mut pulse: BarPulse = BarPulse::Idle();
            pulse.start(kind);
            let centers_x_frac: Vec<f32> = [0.0, 0.5, 0.99]
                .map(|travel_frac| {
                    pulse.age_s = travel_frac * kind.get_travel_s();
                    pulse
                        .make_rect_pulse(BAR_MIN_X_FRAC, USE_RGB, RGB)
                        .center_x_frac
                })
                .to_vec();
            assert!((centers_x_frac[1] - bar_middle_x_frac).abs() < 1e-6);
            let is_first_right: bool = centers_x_frac[0] > BAR_MIN_X_FRAC + BAR_SIZE_FRAC[0];
            let is_last_left: bool = centers_x_frac[2] < BAR_MIN_X_FRAC;
            assert_eq!(
                (is_first_right, is_last_left),
                (is_down, is_down),
                "{kind:?}"
            );
        }
        let mut pulse: BarPulse = BarPulse::Idle();
        pulse.start(PulseKind::Use);
        assert_eq!(
            pulse.make_rect_pulse(BAR_MIN_X_FRAC, USE_RGB, RGB).color,
            [USE_RGB[0], USE_RGB[1], USE_RGB[2], USE_PULSE_OPACITY]
        );
        pulse.start(PulseKind::Refill);
        let refill_color: [f32; 4] = pulse.make_rect_pulse(BAR_MIN_X_FRAC, USE_RGB, RGB).color;
        assert!(refill_color[3] < USE_PULSE_OPACITY);
        assert!((0..3).all(|k| refill_color[k] >= RGB[k]));
    }

    #[test]
    fn pulses_repeat_while_wanted_and_stop_when_not() {
        let mut pulse: BarPulse = BarPulse::Idle();
        pulse.update(STEP_S, Some(PulseKind::Use));
        assert_eq!(pulse.kind, Some(PulseKind::Use));
        pulse.update(USE_PULSE_TRAVEL_S, Some(PulseKind::Use));
        assert_eq!(pulse.kind, Some(PulseKind::Use));
        assert_eq!(
            pulse.make_rect_pulse(BAR_MIN_X_FRAC, USE_RGB, RGB),
            NO_RECT_PULSE,
            "crossed, in its pause"
        );
        pulse.update(USE_PULSE_PERIOD_S, Some(PulseKind::Use));
        assert_eq!((pulse.kind, pulse.age_s), (Some(PulseKind::Use), 0.0));
        pulse.update(USE_PULSE_PERIOD_S, None);
        assert_eq!(pulse.kind, None);
        pulse.update(STEP_S, None);
        assert_eq!(pulse, BarPulse::Idle());
    }

    #[test]
    fn use_pulses_overtake_refill_pulses() {
        let mut pulse: BarPulse = BarPulse::Idle();
        pulse.start(PulseKind::Refill);
        pulse.update(STEP_S, Some(PulseKind::Use));
        assert_eq!((pulse.kind, pulse.age_s), (Some(PulseKind::Use), 0.0));
        pulse.update(STEP_S, Some(PulseKind::Refill));
        assert_eq!(
            pulse.kind,
            Some(PulseKind::Use),
            "a use pulse finishes first"
        );
        pulse.start(PulseKind::Use);
        assert_eq!(pulse.age_s, 0.0, "another hit restarts it");
    }

    #[test]
    fn health_and_energy_changes_pick_pulses() {
        assert_eq!(get_health_pulse_kind(0.75, 1.0), Some(PulseKind::Use));
        assert_eq!(get_health_pulse_kind(1.0, 0.0), Some(PulseKind::Refill));
        assert_eq!(get_health_pulse_kind(0.5, 0.5), None);
        assert_eq!(get_energy_pulse_kind(true, 0.4, 0.5), Some(PulseKind::Use));
        assert_eq!(
            get_energy_pulse_kind(false, 0.5, 0.4),
            Some(PulseKind::Refill)
        );
        assert_eq!(get_energy_pulse_kind(false, 1.0, 1.0), None);
        assert_eq!(get_energy_pulse_kind(false, 0.0, 0.1), None);
    }
}
