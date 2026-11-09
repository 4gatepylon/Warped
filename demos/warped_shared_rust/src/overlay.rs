//! `overlay.rs` is what a level draws over a frame before showing it: the HUD's rectangles, the
//! held gun's wand, the minimap, the text mask with a picture under it, and a recent hit's effect.
//! Each level's renderer (`render_metal.rs`) draws an `Overlay` with its shader's `apply_overlay`,
//! so the `#[repr(C)]` structs here are laid out as that shader reads them.
//!
//! Interface: `Overlay`, `OverlayMinimap`, `Minimap`, `OverlayPicture`, `HitEffect`, `OverlayWand`,
//! `OverlayRect`, `RectPulse`, `RectStyle`, `FLAT_RECT_STYLE`, `NO_RECT_PULSE`,
//! `MAX_RECT_PULSE_COUNT`, `MAX_MINIMAP_MARK_COUNT`, `MAX_MINIMAP_LINE_COUNT`.
//!
//! Example: `let rect = OverlayRect { min_frac: [0.1, 0.9], max_frac: [0.3, 0.92], color: [1.0; 4], style: FLAT_RECT_STYLE, pulse: NO_RECT_PULSE };`
//!
//! Must know:
//! - Gotchas: every level's `render_metal.metal` declares these structs too; a change here changes
//!   them in every level's shader in the same commit.

use crate::viewmodel::GunModel;

/// `MAX_MINIMAP_MARK_COUNT` is how many marks (other players) one `Minimap` carries, the shader's.
pub const MAX_MINIMAP_MARK_COUNT: usize = 16;
/// `MAX_MINIMAP_LINE_COUNT` is how many lines (shots) one `Minimap` carries, the shader's.
pub const MAX_MINIMAP_LINE_COUNT: usize = 96;

/// `Overlay` is what to draw over a frame before it is shown: how much to darken the frame (while a
/// player is out after a hit), a text mask with its size (one byte a pixel, rows top first; see the
/// shader's `apply_overlay` for what its levels mean), up to the renderer's
/// `MAX_OVERLAY_RECT_COUNT` rectangles (the HUD's bars), the player's wand with the gun model drawn in its place, how much of the
/// previous frame's scene to blend into this one's (motion blur; 0 for none), what a recent hit
/// does to the scene, how much of the frame's height the mask's text may take, a picture under
/// the text, how far the scene stretches at the map's soft edge (the shader's
/// `read_stretched_scene`; 0 for none), the minimap, and how far the scene blueshifts inside a
/// wormhole mouth (the shader's `blueshift_scene`: 1 for walking straight ahead at the throat,
/// negative for a redshift, 0 for none).
pub struct Overlay<'a> {
    pub dim_frac: f32,
    pub mask: Option<(&'a [u8], (usize, usize))>,
    pub rects: &'a [OverlayRect],
    pub wand: Option<(OverlayWand, GunModel)>,
    pub blur_frac: f32,
    pub hit: HitEffect,
    pub mask_height_frac: f32,
    pub picture: Option<OverlayPicture<'a>>,
    pub stretch_frac: f32,
    pub minimap: Option<OverlayMinimap<'a>>,
    pub blueshift_frac: f32,
}

/// `OverlayMinimap` is the minimap: the map's picture from above without labels (RGBA, rows top
/// first, `map_layout.rs`) with its size, and the shader's `Minimap`.
pub struct OverlayMinimap<'a> {
    pub map_pixels: &'a [u8],
    pub map_size_px: (usize, usize),
    pub minimap: Minimap,
}

/// `Minimap` is the shader's `Minimap` (each member a multiple of 16 bytes, as Metal lays it
/// out): the frame that takes the minimap's plane (its y = 0, with −z up the minimap) into the
/// world; where it sits (the centre's x and y and the radius in pixels, then the Poincaré disk's
/// radius at its rim, tanh of half its range in curvature radii); where the map's picture lies
/// (the Fermi u and v of its top left corner, meters per pixel, then the curvature radius); how
/// many marks and lines it has, then its kind (0 for the hyperbolic ground; a level's own shader
/// may read others, as City Sphere's `MINIMAP_KIND_SPHERE` does) and a zero; the marks (other
/// players: a point in the minimap's unit disk, x right and y down, then a radius in pixels and a
/// zero; and a colour); and the lines (shots: two ends in the unit disk, then a colour).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Minimap {
    pub ground_frame: [[f32; 4]; 4],
    pub placement: [f32; 4],
    pub map: [f32; 4],
    pub counts: [i32; 4],
    pub marks: [[[f32; 4]; 2]; MAX_MINIMAP_MARK_COUNT],
    pub lines: [[[f32; 4]; 2]; MAX_MINIMAP_LINE_COUNT],
}

impl Minimap {
    /// `Hidden` is no minimap: a radius of 0, which the shader skips.
    ///
    /// Example: `let minimap = Minimap::Hidden();`
    #[allow(non_snake_case)]
    pub fn Hidden() -> Minimap {
        Minimap {
            ground_frame: [[0.0; 4]; 4],
            placement: [0.0; 4],
            map: [0.0; 4],
            counts: [0; 4],
            marks: [[[0.0; 4]; 2]; MAX_MINIMAP_MARK_COUNT],
            lines: [[[0.0; 4]; 2]; MAX_MINIMAP_LINE_COUNT],
        }
    }
}

/// `OverlayPicture` is an RGBA picture (rows top first) of `size_px`, drawn under the overlay's
/// text and stretched between two corners given in the text mask's pixels.
pub struct OverlayPicture<'a> {
    pub pixels: &'a [u8],
    pub size_px: (usize, usize),
    pub min_mask_px: [i32; 2],
    pub max_excl_mask_px: [i32; 2],
}

/// `HitEffect` is what a recent hit does to the scene (the shader's `read_hit_scene`): how much of
/// the effect is left (1 just after the hit, 0 for none), how red the edges turn, how wide their
/// blur is and how far the picture is shifted this frame.
#[derive(Clone, Copy, Default)]
pub struct HitEffect {
    pub strength_frac: f32,
    pub red_frac: f32,
    pub blur_px: f32,
    pub shake_px: [f32; 2],
}

/// `OverlayWand` is the shader's `OverlayWand`: the wand's shaft centre in camera space (x right,
/// y up, -z ahead, meters; w unused), its axes as columns (its length along its own -z), its
/// shaft's half sizes and RGB colour, and its tip's half sizes and RGB colour.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OverlayWand {
    pub center_m: [f32; 4],
    pub axes: [[f32; 4]; 3],
    pub shaft_half_m: [f32; 4],
    pub shaft_color: [f32; 4],
    pub tip_half_m: [f32; 4],
    pub tip_color: [f32; 4],
}

impl OverlayWand {
    /// `get_tip_m` gives the front of the wand's tip in camera space, where the gun's muzzle is.
    ///
    /// Example: `let muzzle_m = wand.get_tip_m();`
    pub fn get_tip_m(&self) -> glam::Vec3 {
        let reach_m: f32 = self.shaft_half_m[2] + 2.0 * self.tip_half_m[2];
        glam::Vec4::from(self.center_m).truncate()
            - glam::Vec4::from(self.axes[2]).truncate() * reach_m
    }
}

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
