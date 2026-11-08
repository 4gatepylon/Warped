//! `hud` lays out the heads-up display as rectangles: crosshair, hit marker, health bar,
//! ammo counter and pips, reload bar, kill counter, and an optional frame-time readout. Digits
//! are drawn as seven-segment glyphs.
//!
//! Example:
//!     let rects: Vec<HudRect> = build_hud(&HudState { .. }, width_px, height_px);

use glam::Vec4;

use crate::render::HudRect;

/// `REFERENCE_HEIGHT_PX` is the window height at which the HUD is drawn at scale 1.
const REFERENCE_HEIGHT_PX: f32 = 1080.0;

/// `HudState` is everything the HUD shows.
pub struct HudState {
    pub health_frac: f32,
    pub ammo: u32,
    pub magazine_size: u32,
    pub reload_frac: Option<f32>,
    pub spread_frac: f32,
    pub ads_frac: f32,
    pub hit_marker_frac: f32,
    pub kill_marker_frac: f32,
    pub kill_count: u32,
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

/// `build_hud` lays out the HUD for a window of the given size in pixels.
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
    let (center_x, center_y) = (width * 0.5, height * 0.5);
    let white: Vec4 = Vec4::new(0.95, 0.97, 1.0, 0.92);
    let shadow: Vec4 = Vec4::new(0.0, 0.0, 0.0, 0.45);
    let accent: Vec4 = Vec4::new(0.35, 0.85, 1.0, 0.95);
    let danger: Vec4 = Vec4::new(1.0, 0.25, 0.2, 0.95);

    if !state.is_dead {
        let crosshair_alpha: f32 = 1.0 - state.ads_frac;
        if crosshair_alpha > 0.02 {
            let gap: f32 = 7.0 + state.spread_frac * 26.0;
            let length: f32 = 11.0;
            let thickness: f32 = 2.0;
            let color: Vec4 = white.with_w(white.w * crosshair_alpha);
            let outline: Vec4 = shadow.with_w(shadow.w * crosshair_alpha);
            let arms: [(f32, f32, f32, f32); 4] = [
                (
                    center_x - gap - length,
                    center_y - thickness * 0.5,
                    length,
                    thickness,
                ),
                (
                    center_x + gap,
                    center_y - thickness * 0.5,
                    length,
                    thickness,
                ),
                (
                    center_x - thickness * 0.5,
                    center_y - gap - length,
                    thickness,
                    length,
                ),
                (
                    center_x - thickness * 0.5,
                    center_y + gap,
                    thickness,
                    length,
                ),
            ];
            for (x, y, w, h) in arms {
                hud.rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, outline);
                hud.rect(x, y, w, h, color);
            }
            hud.rect(center_x - 1.0, center_y - 1.0, 2.0, 2.0, color);
        }
        if state.hit_marker_frac > 0.0 {
            let color: Vec4 = if state.kill_marker_frac > 0.0 {
                danger
            } else {
                white
            }
            .with_w(state.hit_marker_frac);
            for step in 0..5 {
                let offset: f32 = 8.0 + step as f32 * 2.5;
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    hud.rect(
                        center_x + sx * offset - 1.5,
                        center_y + sy * offset - 1.5,
                        3.0,
                        3.0,
                        color,
                    );
                }
            }
        }
    }

    let margin: f32 = 48.0;
    let bar_width: f32 = 320.0;
    let bar_y: f32 = height - margin - 14.0;
    let health_color: Vec4 = if state.health_frac < 0.3 {
        danger
    } else {
        white
    };
    hud.rect(margin - 3.0, bar_y - 3.0, bar_width + 6.0, 20.0, shadow);
    hud.rect(
        margin,
        bar_y,
        bar_width * state.health_frac.clamp(0.0, 1.0),
        14.0,
        health_color,
    );
    hud.rect(margin, bar_y - 52.0, 10.0, 34.0, health_color);
    hud.rect(margin - 12.0, bar_y - 40.0, 34.0, 10.0, health_color);
    hud.number(
        margin + 150.0,
        bar_y - 54.0,
        38.0,
        (state.health_frac * 100.0).ceil() as u32,
        health_color,
    );

    let right: f32 = width - margin;
    let ammo_color: Vec4 = if state.ammo <= 3 { danger } else { white };
    let left_x: f32 = hud.number(right - 70.0, bar_y - 62.0, 64.0, state.ammo, ammo_color);
    let _ = left_x;
    hud.number(
        right,
        bar_y - 34.0,
        30.0,
        state.magazine_size,
        white.with_w(0.55),
    );
    let pip_width: f32 = 9.0;
    let pip_gap: f32 = 5.0;
    for pip in 0..state.magazine_size {
        let x: f32 = right - (state.magazine_size - pip) as f32 * (pip_width + pip_gap) + pip_gap;
        let color: Vec4 = if pip < state.ammo { accent } else { shadow };
        hud.rect(x, bar_y, pip_width, 14.0, color);
    }
    if let Some(frac) = state.reload_frac {
        let reload_width: f32 = 180.0;
        hud.rect(
            center_x - reload_width * 0.5 - 2.0,
            center_y + 58.0,
            reload_width + 4.0,
            10.0,
            shadow,
        );
        hud.rect(
            center_x - reload_width * 0.5,
            center_y + 60.0,
            reload_width * frac,
            6.0,
            accent,
        );
    }

    hud.number(right, margin, 34.0, state.kill_count, accent);
    for index in 0..3 {
        hud.rect(
            right - 140.0 + index as f32 * 12.0,
            margin + 6.0,
            6.0,
            22.0,
            accent,
        );
    }
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
