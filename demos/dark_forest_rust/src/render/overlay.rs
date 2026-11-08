//! `overlay` holds the HUD's overlay rectangles, copied from the Lunar Surface demo's
//! `render_metal.rs` (`demos/hyperbolic_wormholes_rust/src/render_metal.rs`) so the HUD looks the
//! same in every level: `OverlayRect`, `RectStyle`, `RectPulse`, `FLAT_RECT_STYLE` and
//! `NO_RECT_PULSE`. `shaders/overlay.metal` draws them over the frame.
//!
//! Example: `let rects: Vec<OverlayRect> = hud_bars::make_progress_bar(min, size, 0.5, rgb);`

/// `OverlayRect` is the shader's `OverlayRect`: corners as fractions of the screen (top left 0, 0),
/// an RGBA colour (A the opacity), how it looks beyond its colour (`FLAT_RECT_STYLE` for a
/// plain rectangle) and the pulses it carries for its bar (`NO_RECT_PULSE` for none).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayRect {
    pub min_frac: [f32; 2],
    pub max_frac: [f32; 2],
    pub color: [f32; 4],
    pub style: RectStyle,
    pub pulse: RectPulse,
}

/// `RectPulse` is the shader's `RectPulse`, up to `MAX_RECT_PULSE_COUNT` soft glows running along
/// a HUD bar, carried by the bar's first `OverlayRect`: their middles' x as fractions of the
/// screen's width (measured along the rect's slant, at its middle row), their opacities (0 for
/// none), the x span they show in (the bar's filled part; they fade out just inside its ends), their
/// half width (of the screen's width) and half height (of its height), and their RGBA colour (A
/// unused).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectPulse {
    pub centers_x_frac: [f32; MAX_RECT_PULSE_COUNT],
    pub opacities: [f32; MAX_RECT_PULSE_COUNT],
    pub min_x_frac: f32,
    pub max_x_frac: f32,
    pub half_width_frac: f32,
    pub half_height_frac: f32,
    pub color: [f32; 4],
}

/// `MAX_RECT_PULSE_COUNT` is how many pulses one `RectPulse` carries, the shader's.
pub const MAX_RECT_PULSE_COUNT: usize = 4;

/// `NO_RECT_PULSE` is no pulse: every opacity 0, which the shader skips.
pub const NO_RECT_PULSE: RectPulse = RectPulse {
    centers_x_frac: [0.0; MAX_RECT_PULSE_COUNT],
    opacities: [0.0; MAX_RECT_PULSE_COUNT],
    min_x_frac: 0.0,
    max_x_frac: 0.0,
    half_width_frac: 0.0,
    half_height_frac: 0.0,
    color: [0.0; 4],
};

/// `RectStyle` is the shader's `RectStyle`, how an `OverlayRect` looks beyond its colour, with
/// lengths as fractions of the output's height so it looks the same at any size: its corners'
/// radius (at most half its shorter side), an inner outline's width, a gloss highlight's strength
/// over its top half (0 to 1), a soft drop shadow's blur, a slant (how far its sides lean right
/// per unit of height, like `/` when positive), a glow's radius in its own colour, how much
/// lighter its top and darker its bottom are (0 for even), a zero, and the outline's RGBA colour.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectStyle {
    pub corner_frac: f32,
    pub outline_frac: f32,
    pub gloss_frac: f32,
    pub shadow_frac: f32,
    pub slant_x_per_y: f32,
    pub glow_frac: f32,
    pub shade_frac: f32,
    pub padding: f32,
    pub outline_color: [f32; 4],
}

/// `FLAT_RECT_STYLE` is a plain rectangle: square corners, no outline, gloss, shadow, slant, glow
/// or shading.
pub const FLAT_RECT_STYLE: RectStyle = RectStyle {
    corner_frac: 0.0,
    outline_frac: 0.0,
    gloss_frac: 0.0,
    shadow_frac: 0.0,
    slant_x_per_y: 0.0,
    glow_frac: 0.0,
    shade_frac: 0.0,
    padding: 0.0,
    outline_color: [0.0; 4],
};

const BYTES_PER_OVERLAY_RECT_ACCORDING_TO_METAL_LAYOUT: usize = 144;
const _: () = assert!(
    std::mem::size_of::<OverlayRect>() == BYTES_PER_OVERLAY_RECT_ACCORDING_TO_METAL_LAYOUT,
    "OverlayRect must match the shader's 144-byte OverlayRect"
);

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

/// `HitEffect` is what a recent hit does to the scene (the shader's `read_hit_scene`): how much of
/// the effect is left (1 just after the hit, 0 for none), how red the edges turn, how wide their
/// blur is and how far the picture is shifted this frame. It matches `post.metal`'s `HitEffect`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct HitEffect {
    pub strength_frac: f32,
    pub red_frac: f32,
    pub blur_px: f32,
    pub shake_px: [f32; 2],
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
