//! `hud` lays out the heads-up display: the Lunar Surface demo's bars and crosshair as overlay
//! rectangles (`build_overlay_rects`, with `hud_bars`: health, ammo and sprint energy in the
//! bottom left, with their pulses, and the console bar), and as flat rectangles an optional
//! frame-time readout in seven-segment digits and the tint while dead (`build_hud`).
//!
//! Example:
//!     let rects: Vec<HudRect> = build_hud(&HudState { .. }, width_px, height_px);
//!     let bars: Vec<OverlayRect> = build_overlay_rects(&state, &pulses, (2560, 1440));

use glam::Vec4;

use crate::hud_bars::{BarPulses, PULSE_TUNING, make_progress_bar, make_segmented_bar};
use crate::render::HudRect;
use crate::render::overlay::{FLAT_RECT_STYLE, NO_RECT_PULSE, OverlayRect};

/// `REFERENCE_HEIGHT_PX` is the window height at which the HUD is drawn at scale 1.
const REFERENCE_HEIGHT_PX: f32 = 1080.0;

/// `HudState` is what the flat HUD rectangles show.
pub struct HudState {
    pub is_dead: bool,
    pub frame_ms: Option<f32>,
}

/// `HudBuilder` collects rectangles in a scaled coordinate space.
struct HudBuilder {
    rects: Vec<HudRect>,
    scale: f32,
}

impl HudBuilder {
    /// `rect` adds a rectangle; coordinates are in reference pixels.
    fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, color_rgba: Vec4) {
        self.rects.push(HudRect {
            x_px: (x * self.scale).round(),
            y_px: (y * self.scale).round(),
            width_px: (width * self.scale).max(1.0).round(),
            height_px: (height * self.scale).max(1.0).round(),
            color_rgba,
        });
    }

    /// `digit` draws a seven-segment digit with its top-left corner at (x, y).
    fn digit(&mut self, x: f32, y: f32, size: f32, value: u32, color_rgba: Vec4) {
        const SEGMENTS: [u8; 10] = [
            0b0111111, 0b0000110, 0b1011011, 0b1001111, 0b1100110, 0b1101101, 0b1111101, 0b0000111,
            0b1111111, 0b1101111,
        ];
        let bits: u8 = SEGMENTS[(value % 10) as usize];
        let thickness: f32 = size * 0.16;
        let width: f32 = size * 0.6;
        let half: f32 = size * 0.5;
        let segments: [(f32, f32, f32, f32); 7] = [
            (x + thickness, y, width - 2.0 * thickness, thickness),
            (
                x + width - thickness,
                y + thickness,
                thickness,
                half - thickness * 1.5,
            ),
            (
                x + width - thickness,
                y + half + thickness * 0.5,
                thickness,
                half - thickness * 1.5,
            ),
            (
                x + thickness,
                y + size - thickness,
                width - 2.0 * thickness,
                thickness,
            ),
            (
                x,
                y + half + thickness * 0.5,
                thickness,
                half - thickness * 1.5,
            ),
            (x, y + thickness, thickness, half - thickness * 1.5),
            (
                x + thickness,
                y + half - thickness * 0.5,
                width - 2.0 * thickness,
                thickness,
            ),
        ];
        for (index, (sx, sy, sw, sh)) in segments.iter().enumerate() {
            if bits & (1 << index) != 0 {
                self.rect(*sx, *sy, *sw, *sh, color_rgba);
            }
        }
    }

    /// `number` draws a number right-aligned so that it ends at x; returns its left edge.
    fn number(&mut self, right_x: f32, y: f32, size: f32, value: u32, color_rgba: Vec4) -> f32 {
        let digits: Vec<u32> = value
            .to_string()
            .bytes()
            .map(|b| (b - b'0') as u32)
            .collect();
        let advance: f32 = size * 0.78;
        let mut x: f32 = right_x - advance * digits.len() as f32;
        let left_x: f32 = x;
        for digit in digits {
            self.digit(x, y, size, digit, color_rgba);
            x += advance;
        }
        left_x
    }
}

/// `build_hud` lays out what the HUD draws as flat rectangles, for a window of the given size in
/// pixels: the frame-time readout, and the dark tint while dead. The bars and the crosshair are
/// overlay rectangles (`build_overlay_rects`).
///
/// Example:
///     let rects: Vec<HudRect> = build_hud(&state, 3024.0, 1964.0);
pub fn build_hud(state: &HudState, width_px: f32, height_px: f32) -> Vec<HudRect> {
    let scale: f32 = height_px / REFERENCE_HEIGHT_PX;
    let mut hud: HudBuilder = HudBuilder {
        rects: Vec::new(),
        scale,
    };
    let width: f32 = width_px / scale;
    let height: f32 = REFERENCE_HEIGHT_PX;
    let white: Vec4 = Vec4::new(0.95, 0.97, 1.0, 0.92);
    let margin: f32 = 48.0;
    if let Some(frame_ms) = state.frame_ms {
        let tenths: u32 = (frame_ms * 10.0).round() as u32;
        let left: f32 = hud.number(margin + 60.0, margin, 26.0, tenths / 10, white.with_w(0.7));
        let _ = left;
        hud.rect(margin + 64.0, margin + 22.0, 4.0, 4.0, white.with_w(0.7));
        hud.number(margin + 92.0, margin, 26.0, tenths % 10, white.with_w(0.7));
    }
    if state.is_dead {
        hud.rect(0.0, 0.0, width, height, Vec4::new(0.08, 0.0, 0.0, 0.35));
    }
    hud.rects
}

/// `HEALTH_BAR_MIN_FRAC` to `CROSSHAIR_COLOR` are the Lunar Surface demo's HUD layout and colours
/// (`demos/hyperbolic_wormholes_rust/src/main.rs`), copied so the bars sit and look the same in
/// every level: each bar's top left corner as fractions of the window, its colour and its pulses'
/// colour, the energy bar's segment count, the console bar's size, and the crosshair's gap, arm
/// length and thickness as fractions of the window's height.
const HEAT_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.9];
const HEAT_BAR_RGB: [f32; 3] = [1.0, 0.6, 0.2];
const HEAT_BAR_OVERHEATED_RGB: [f32; 3] = [1.0, 0.15, 0.1];
const HEALTH_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.87];
const HEALTH_BAR_RGB: [f32; 3] = [0.35, 0.95, 0.45];
const HEALTH_PULSE_RGB: [f32; 3] = [1.0, 0.45, 0.2];
const ENERGY_SEGMENT_COUNT: u32 = 10;
const ENERGY_BAR_MIN_FRAC: [f32; 2] = [0.03, 0.93];
const ENERGY_BAR_RGB: [f32; 3] = [0.5, 0.82, 1.0];
const ENERGY_PULSE_RGB: [f32; 3] = [0.8, 0.94, 1.0];
const DISABLE_BAR_MIN_FRAC: [f32; 2] = [0.4, 0.62];
const DISABLE_BAR_SIZE_FRAC: [f32; 2] = [0.2, 0.02];
const DISABLE_BAR_RGB: [f32; 3] = [1.0, 0.85, 0.3];
const CROSSHAIR_GAP_FRAC: f32 = 0.006;
const CROSSHAIR_ARM_FRAC: f32 = 0.012;
const CROSSHAIR_THICKNESS_FRAC: f32 = 0.0025;
const CROSSHAIR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.85];

/// `OverlayHudState` is what the HUD's bars show: health in segments of one body hit each, the
/// gun's ammo in segments of one shot each (drawn in the overheated colour while it reloads), the
/// sprint energy, how far an enemy console has been held toward off (if one is), and whether text
/// covers the view (which hides the crosshair).
pub struct OverlayHudState {
    pub health_frac: f32,
    pub health_segment_count: u32,
    pub ammo_frac: f32,
    pub ammo_segment_count: u32,
    pub is_reloading: bool,
    pub energy_frac: f32,
    pub console_hold_frac: Option<f32>,
    pub is_view_covered: bool,
}

/// `BarPulsePair` is the health and energy bars' pulses, which run along a bar while its value
/// changes (`hud_bars::BarPulses`).
pub struct BarPulsePair {
    health: BarPulses,
    energy: BarPulses,
}

impl BarPulsePair {
    /// `New` is both bars full, without pulses.
    #[allow(non_snake_case)]
    pub fn New() -> BarPulsePair {
        BarPulsePair {
            health: BarPulses::New(1.0),
            energy: BarPulses::New(1.0),
        }
    }

    /// `update` moves both bars' pulses on by `dt_s` toward the state's health and energy, as the
    /// Lunar Surface demo's `update_bar_pulses` does.
    ///
    /// Example: `pulses.update(dt_s, &state);`
    pub fn update(&mut self, dt_s: f32, state: &OverlayHudState) {
        self.health.update(
            f64::from(dt_s),
            f64::from(state.health_frac),
            state.health_segment_count,
            &PULSE_TUNING,
        );
        self.energy.update(
            f64::from(dt_s),
            f64::from(state.energy_frac),
            ENERGY_SEGMENT_COUNT,
            &PULSE_TUNING,
        );
    }
}

/// `build_overlay_rects` gives the HUD's overlay rectangles as the Lunar Surface demo's
/// `get_overlay_rects` builds them: the crosshair (unless text covers the view) and, in the bottom
/// left, the health bar, the gun's ammo bar and the sprint energy bar, with health and energy
/// carrying their pulses; below the middle, the progress of turning off a console while `E` is
/// held on one. `window_px` is the window's size in pixels.
///
/// Example: `let rects: Vec<OverlayRect> = build_overlay_rects(&state, &pulses, (2560, 1440));`
pub fn build_overlay_rects(
    state: &OverlayHudState,
    pulses: &BarPulsePair,
    window_px: (u32, u32),
) -> Vec<OverlayRect> {
    let mut rects: Vec<OverlayRect> = Vec::new();
    if !state.is_view_covered {
        rects.extend(get_crosshair_rects(window_px));
    }
    let health_frac: f64 = f64::from(state.health_frac);
    rects.extend(make_segmented_bar(
        HEALTH_BAR_MIN_FRAC,
        state.health_segment_count,
        health_frac,
        HEALTH_BAR_RGB,
        pulses.health.make_rect_pulse(
            HEALTH_BAR_MIN_FRAC,
            state.health_segment_count,
            health_frac,
            HEALTH_PULSE_RGB,
            HEALTH_BAR_RGB,
            &PULSE_TUNING,
        ),
    ));
    rects.extend(make_segmented_bar(
        HEAT_BAR_MIN_FRAC,
        state.ammo_segment_count,
        f64::from(state.ammo_frac),
        if state.is_reloading {
            HEAT_BAR_OVERHEATED_RGB
        } else {
            HEAT_BAR_RGB
        },
        NO_RECT_PULSE,
    ));
    if let Some(hold_frac) = state.console_hold_frac {
        rects.extend(make_progress_bar(
            DISABLE_BAR_MIN_FRAC,
            DISABLE_BAR_SIZE_FRAC,
            hold_frac.min(1.0),
            DISABLE_BAR_RGB,
        ));
    }
    let energy_frac: f64 = f64::from(state.energy_frac);
    rects.extend(make_segmented_bar(
        ENERGY_BAR_MIN_FRAC,
        ENERGY_SEGMENT_COUNT,
        energy_frac,
        ENERGY_BAR_RGB,
        pulses.energy.make_rect_pulse(
            ENERGY_BAR_MIN_FRAC,
            ENERGY_SEGMENT_COUNT,
            energy_frac,
            ENERGY_PULSE_RGB,
            ENERGY_BAR_RGB,
            &PULSE_TUNING,
        ),
    ));
    rects
}

/// `get_crosshair_rects` is the Lunar Surface demo's crosshair, copied as it is: four thin white
/// arms around the middle of a window of `window_px` pixels, sized by its height.
///
/// Example: `let rects = get_crosshair_rects((2560, 1440));`
fn get_crosshair_rects(window_px: (u32, u32)) -> Vec<OverlayRect> {
    let x_per_y: f32 = window_px.1 as f32 / window_px.0.max(1) as f32;
    let (near, far, half) = (
        CROSSHAIR_GAP_FRAC,
        CROSSHAIR_GAP_FRAC + CROSSHAIR_ARM_FRAC,
        CROSSHAIR_THICKNESS_FRAC / 2.0,
    );
    [(near, far), (-far, -near)]
        .into_iter()
        .flat_map(|(lo, hi)| {
            [
                ([lo * x_per_y, -half], [hi * x_per_y, half]),
                ([-half * x_per_y, lo], [half * x_per_y, hi]),
            ]
        })
        .map(|([x0, y0], [x1, y1])| OverlayRect {
            min_frac: [0.5 + x0, 0.5 + y0],
            max_frac: [0.5 + x1, 0.5 + y1],
            color: CROSSHAIR_COLOR,
            style: FLAT_RECT_STYLE,
            pulse: NO_RECT_PULSE,
        })
        .collect()
}
