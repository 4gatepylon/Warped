//! `overlay` gives the shared overlay types every level draws (`warped_shared::overlay`, so the HUD
//! looks the same in every level): `OverlayRect`, a HUD rectangle with its style and pulses, and
//! `HitEffect`, a recent hit's effect; and which parts of the window the rectangles change
//! (`get_overlay_regions_px`). `shaders/overlay.metal` draws the rectangles over the frame and
//! `shaders/post.metal` the hit's effect.
//!
//! Example: `let rects: Vec<OverlayRect> = hud_bars::make_progress_bar(min, size, 0.5, rgb);`

pub use warped_shared::overlay::{HitEffect, OverlayRect};

/// `OVERLAY_REACH_PER_HEIGHT` is how far past its box, as a fraction of the window's height, a
/// rectangle's shadow, glow or pulses can reach (the pulses reach furthest: 3 half heights of 1.1
/// bar heights of 0.016), with room to spare.
const OVERLAY_REACH_PER_HEIGHT: f32 = 0.06;

/// `get_overlay_regions_px` gives the parts of a window of `size_px` pixels that `rects` can
/// change: each rectangle's box, grown by its reach and slant, merged where they overlap, as
/// (left, top, right, bottom) pixels clamped to the window. The overlay pass shades only these.
///
/// Example: `let regions = get_overlay_regions_px(&rects, (2560, 1440));`
pub fn get_overlay_regions_px(rects: &[OverlayRect], size_px: (u32, u32)) -> Vec<[u32; 4]> {
    let (width_px, height_px) = (size_px.0 as f32, size_px.1 as f32);
    let mut regions: Vec<[f32; 4]> = rects
        .iter()
        .map(|rect| {
            let lean_px: f32 =
                rect.style.slant_x_per_y.abs() * (rect.max_frac[1] - rect.min_frac[1]) * height_px;
            let reach_px: f32 = OVERLAY_REACH_PER_HEIGHT * height_px;
            [
                rect.min_frac[0] * width_px - reach_px - lean_px,
                rect.min_frac[1] * height_px - reach_px,
                rect.max_frac[0] * width_px + reach_px + lean_px,
                rect.max_frac[1] * height_px + reach_px,
            ]
        })
        .collect();
    let mut is_merging: bool = true;
    while is_merging {
        is_merging = false;
        'pairs: for i in 0..regions.len() {
            for j in i + 1..regions.len() {
                let (a, b) = (regions[i], regions[j]);
                if a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3] {
                    regions[i] = [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[2].max(b[2]),
                        a[3].max(b[3]),
                    ];
                    regions.swap_remove(j);
                    is_merging = true;
                    break 'pairs;
                }
            }
        }
    }
    regions
        .iter()
        .map(|region| {
            [
                region[0].clamp(0.0, width_px) as u32,
                region[1].clamp(0.0, height_px) as u32,
                region[2].clamp(0.0, width_px).ceil() as u32,
                region[3].clamp(0.0, height_px).ceil() as u32,
            ]
        })
        .filter(|region| region[2] > region[0] && region[3] > region[1])
        .collect()
}

/// `tests` checks the regions the overlay pass shades.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | rects' reaches | far apart, overlapping | `test_regions_merge_only_overlapping_rects` |
/// | a rect at the window's edge | inside, past it | `test_regions_merge_only_overlapping_rects` |
#[cfg(test)]
mod tests {
    use super::*;
    use warped_shared::overlay::{FLAT_RECT_STYLE, NO_RECT_PULSE};

    /// `rect` is a plain rectangle from `min_frac` to `max_frac`.
    fn rect(min_frac: [f32; 2], max_frac: [f32; 2]) -> OverlayRect {
        OverlayRect {
            min_frac,
            max_frac,
            color: [1.0; 4],
            style: FLAT_RECT_STYLE,
            pulse: NO_RECT_PULSE,
        }
    }

    /// `test_regions_merge_only_overlapping_rects` checks two bars a little apart become one region
    /// around both, a far crosshair arm its own, and a rect at the corner is clamped to the window.
    #[test]
    fn test_regions_merge_only_overlapping_rects() {
        let rects: [OverlayRect; 4] = [
            rect([0.03, 0.87], [0.23, 0.886]),
            rect([0.03, 0.9], [0.23, 0.916]),
            rect([0.5, 0.49], [0.51, 0.5]),
            rect([0.0, 0.0], [0.01, 0.01]),
        ];
        let reach_px: f32 = OVERLAY_REACH_PER_HEIGHT * 1000.0;
        let regions: Vec<[u32; 4]> = get_overlay_regions_px(&rects, (2000, 1000));
        assert_eq!(regions.len(), 3, "{regions:?}");
        let bars: [u32; 4] = [
            (60.0 - reach_px) as u32,
            (870.0 - reach_px) as u32,
            (460.0 + reach_px) as u32,
            (916.0 + reach_px) as u32,
        ];
        assert!(regions.contains(&bars), "{regions:?}");
        assert!(
            regions
                .iter()
                .any(|region| region[0] == 0 && region[1] == 0)
        );
    }
}
