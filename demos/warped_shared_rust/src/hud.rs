//! `hud.rs` is the parts of the heads-up display every level draws alike (W-126, same play in
//! every level), as `OverlayRect`s for each level's renderer: the crosshair.
//!
//! Interface: `get_crosshair_rects`.
//!
//! Example: `let rects: Vec<OverlayRect> = get_crosshair_rects((2560, 1440));`

use crate::overlay::{FLAT_RECT_STYLE, NO_RECT_PULSE, OverlayRect};

pub const CROSSHAIR_GAP_FRAC: f32 = 0.006;
pub const CROSSHAIR_ARM_FRAC: f32 = 0.012;
pub const CROSSHAIR_THICKNESS_FRAC: f32 = 0.0025;
pub const CROSSHAIR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.85];

/// `get_crosshair_rects` gives the crosshair in the middle of a window of `window_px` pixels: four
/// arms `CROSSHAIR_ARM_FRAC` long, `CROSSHAIR_GAP_FRAC` from the middle, in fractions of the window's
/// height so they look the same at any size.
///
/// Example: `let rects = get_crosshair_rects((2560, 1600));`
pub fn get_crosshair_rects(window_px: (u32, u32)) -> Vec<OverlayRect> {
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
