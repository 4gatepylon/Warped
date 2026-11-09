//! `hud` lays out the heads-up display: the shared HUD every level draws (`warped_shared::hud`,
//! Lunar Surface's) as overlay rectangles (`build_overlay_rects`: the crosshair; health, ammo and
//! sprint energy in the bottom left, with their pulses; the console bar), and as flat rectangles
//! an optional frame-time readout in seven-segment digits and the tint while dead (`build_hud`).
//!
//! Example:
//!     let rects: Vec<HudRect> = build_hud(&HudState { .. }, width_px, height_px);
//!     let bars = build_overlay_rects(&bars, &ammo, &settings.rifle, &pulses, false, (2560, 1440));

use glam::Vec4;

use warped_shared::hud::{AmmoBar, HudBars, HudPulses, get_crosshair_rects, make_ammo_bar_rects};

use crate::render::HudRect;
use crate::render::overlay::OverlayRect;
use crate::settings::RifleSettings;

/// `REFERENCE_HEIGHT_PX` is the window height at which the HUD is drawn at scale 1.
const REFERENCE_HEIGHT_PX: f32 = 1080.0;
/// `DEATH_DIM_FRAC` is how much the view darkens while dead, under "YOU DIED" (the Lunar Surface
/// demo's).
const DEATH_DIM_FRAC: f32 = 0.55;

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
        hud.rect(
            0.0,
            0.0,
            width,
            height,
            Vec4::new(0.0, 0.0, 0.0, DEATH_DIM_FRAC),
        );
    }
    hud.rects
}

/// `build_overlay_rects` gives the HUD's overlay rectangles as Lunar Surface draws them, from the
/// shared HUD (`warped_shared::hud`): the crosshair unless text covers the view
/// (`is_view_covered`), then the bars of `bars` and `ammo` with `pulses` (`make_ammo_bar_rects`).
/// `window_px` is the window's size in pixels.
///
/// Example: `let rects = build_overlay_rects(&bars, &ammo, &settings.rifle, &pulses, false, (2560, 1440));`
pub fn build_overlay_rects(
    bars: &HudBars,
    ammo: &AmmoBar,
    rifle: &RifleSettings,
    pulses: &HudPulses,
    is_view_covered: bool,
    window_px: (u32, u32),
) -> Vec<OverlayRect> {
    let mut rects: Vec<OverlayRect> = Vec::new();
    if !is_view_covered {
        rects.extend(get_crosshair_rects(window_px));
    }
    rects.extend(make_ammo_bar_rects(bars, ammo, rifle, pulses));
    rects
}

/// `tests` checks which overlay rectangles the HUD draws.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | view | clear, covered (menu or "YOU DIED") | `test_crosshair_only_over_a_clear_view` |
/// | console | not held, held | `test_console_bar_only_while_held` |
/// | bars | health, ammo and energy, each in its Lunar Surface colour | `test_console_bar_only_while_held` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ServerSettings;
    use warped_shared::hud::{
        CROSSHAIR_COLOR, DISABLE_BAR_RGB, ENERGY_BAR_RGB, HEALTH_BAR_RGB, HEAT_BAR_RGB,
    };

    /// `BARS` is a player at full health and energy, not holding `E` on a console.
    const BARS: HudBars = HudBars {
        health_frac: Some(1.0),
        energy_frac: Some(1.0),
        console_hold_frac: None,
    };
    /// `AMMO` is the sniper, loaded.
    const AMMO: AmmoBar = AmmoBar {
        frac: 1.0,
        segment_count: 1,
        is_overheated: false,
    };
    const WINDOW_PX: (u32, u32) = (2560, 1440);

    /// `count_colored` counts the rectangles whose colour starts with `rgb`.
    fn count_colored(rects: &[OverlayRect], rgb: [f32; 3]) -> usize {
        rects
            .iter()
            .filter(|rect| rect.color[..3] == rgb[..])
            .count()
    }

    /// `test_crosshair_only_over_a_clear_view` checks the four crosshair arms show over a clear
    /// view and not while text covers it.
    #[test]
    fn test_crosshair_only_over_a_clear_view() {
        let (pulses, rifle) = (HudPulses::New(), ServerSettings::Builtin().rifle);
        let crosshair_rgb: [f32; 3] = [CROSSHAIR_COLOR[0], CROSSHAIR_COLOR[1], CROSSHAIR_COLOR[2]];
        let clear: Vec<OverlayRect> =
            build_overlay_rects(&BARS, &AMMO, &rifle, &pulses, false, WINDOW_PX);
        assert_eq!(count_colored(&clear, crosshair_rgb), 4);
        let hidden: Vec<OverlayRect> =
            build_overlay_rects(&BARS, &AMMO, &rifle, &pulses, true, WINDOW_PX);
        assert_eq!(count_colored(&hidden, crosshair_rgb), 0);
    }

    /// `test_console_bar_only_while_held` checks the health, ammo and energy bars always show in
    /// their colours, and the console bar only while `E` is held on a console.
    #[test]
    fn test_console_bar_only_while_held() {
        let (pulses, rifle) = (HudPulses::New(), ServerSettings::Builtin().rifle);
        let idle: Vec<OverlayRect> =
            build_overlay_rects(&BARS, &AMMO, &rifle, &pulses, false, WINDOW_PX);
        for rgb in [HEALTH_BAR_RGB, HEAT_BAR_RGB, ENERGY_BAR_RGB] {
            assert!(count_colored(&idle, rgb) > 0, "{rgb:?}");
        }
        assert_eq!(count_colored(&idle, DISABLE_BAR_RGB), 0);
        let holding: HudBars = HudBars {
            console_hold_frac: Some(0.5),
            ..BARS
        };
        let held: Vec<OverlayRect> =
            build_overlay_rects(&holding, &AMMO, &rifle, &pulses, false, WINDOW_PX);
        assert!(count_colored(&held, DISABLE_BAR_RGB) > 0);
    }
}
