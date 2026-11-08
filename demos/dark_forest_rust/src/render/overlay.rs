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
