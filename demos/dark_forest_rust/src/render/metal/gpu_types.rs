//! `gpu_types` holds the uniform and instance layouts that the Metal shaders read; each one must
//! match the struct of the same name in `common.metal` or `ibl.metal`.
//!
//! Example:
//!     let rect: GpuHudRect = GpuHudRect { rect_px: Vec4::ZERO, color: Vec4::ONE };

use glam::{Mat4, UVec4, Vec4};

use crate::render::culling::CASCADE_COUNT;

/// `GpuFrameUniforms` is everything every pass needs to know about the frame.
///
/// Example:
///     let frame: GpuFrameUniforms = GpuFrameUniforms::default();
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuFrameUniforms {
    pub view_proj: Mat4,
    pub view: Mat4,
    pub proj: Mat4,
    pub inv_view_proj: Mat4,
    pub inv_proj: Mat4,
    pub viewmodel_view_proj: Mat4,
    pub cascade_view_proj: [Mat4; CASCADE_COUNT],
    pub cascade_split_far_m: Vec4,
    pub cascade_texel_size_m: Vec4,
    pub camera_position_m: Vec4,
    pub sun_direction: Vec4,
    pub sun_illuminance: Vec4,
    pub sh9: [Vec4; 9],
    pub fog_color: Vec4,
    pub fog_params: Vec4,
    pub sky_visibility_origin_m: Vec4,
    pub sky_visibility_inv_size_m: Vec4,
    pub screen_size: Vec4,
    pub render_params: Vec4,
    pub post_params: Vec4,
    pub bounce_color: Vec4,
    pub effect_params: Vec4,
}

/// `GpuLensUniforms` is the wormhole lens pass's mouths: (centre, lens throat) and (reach, λ²,
/// partner, floor height) each, and (mouth count, 1 while capturing, atlas rows, unused); then
/// what it shows live: each mouth's live slot plus 1 (0 for none, negative when a robot is within
/// the mouth's reach; four mouths a vector), the robots' bounding spheres (centre, radius), and
/// (unused, sphere count, near plane, glow segment count); then the lasers' and other glows'
/// segments, each a start and an end (w: the glow's radius on both).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuLensUniforms {
    pub centres: [Vec4; 16],
    pub shapes: [Vec4; 16],
    pub params: Vec4,
    pub live_slots: [Vec4; 4],
    pub movers: [Vec4; 16],
    pub live_params: Vec4,
    pub glow_starts: [Vec4; crate::render::MAX_LENS_GLOW_SEGMENT_COUNT],
    pub glow_ends: [Vec4; crate::render::MAX_LENS_GLOW_SEGMENT_COUNT],
}

/// `GpuPassUniforms` is the projection that one pass draws with.
///
/// Example:
///     let pass: GpuPassUniforms = GpuPassUniforms { view_proj, flags: Vec4::ZERO };
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuPassUniforms {
    pub view_proj: Mat4,
    pub flags: Vec4,
}

/// `GpuDrawUniforms` is one draw's transform, tint, and flags (y: is a viewmodel draw).
///
/// Example:
///     let draw: GpuDrawUniforms = GpuDrawUniforms { model, tint: Vec4::ONE, flags: UVec4::ZERO };
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuDrawUniforms {
    pub model: Mat4,
    pub tint: Vec4,
    pub flags: UVec4,
}

/// `GpuMaterialUniforms` is a material's factors (params: roughness, metallic, alpha cutoff,
/// whether occlusion is in the red channel).
///
/// Example:
///     let material: GpuMaterialUniforms = GpuMaterialUniforms::default();
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuMaterialUniforms {
    pub base_color_factor: Vec4,
    pub emissive_factor: Vec4,
    pub params: Vec4,
}

/// `GpuPointLight` is a point light: position and radius, then color.
///
/// Example:
///     let light: GpuPointLight = GpuPointLight { position_radius, color };
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuPointLight {
    pub position_radius: Vec4,
    pub color: Vec4,
}

/// `GpuDecal` is one bullet hole: position and size, normal and seed, tangent.
///
/// Example:
///     let decal: GpuDecal = GpuDecal::default();
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuDecal {
    pub position_size: Vec4,
    pub normal_seed: Vec4,
    pub tangent: Vec4,
}

/// `GpuParticle` is one particle: position and size, stretch and kind, color, seed.
///
/// Example:
///     let particle: GpuParticle = GpuParticle::default();
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuParticle {
    pub position_size: Vec4,
    pub stretch_kind: Vec4,
    pub color: Vec4,
    pub seed: Vec4,
}

/// `GpuHudRect` is one HUD rectangle in window pixels (x, y, width, height) and its color.
///
/// Example:
///     let rect: GpuHudRect = GpuHudRect { rect_px: Vec4::ZERO, color: Vec4::ONE };
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuHudRect {
    pub rect_px: Vec4,
    pub color: Vec4,
}

/// `GpuHudImage` is one HUD picture's window rectangle (x, y, width, height) and its size in
/// pixels; its pixels follow it in the frame's buffer.
///
/// Example:
///     let image: GpuHudImage = GpuHudImage { rect_px: [0.0; 4], size_px: [1, 1], padding: [0; 2] };
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuHudImage {
    pub rect_px: [f32; 4],
    pub size_px: [u32; 2],
    pub padding: [u32; 2],
}

/// `GpuOccluderBox` is one axis-aligned box that blocks the sky.
///
/// Example:
///     let occluder: GpuOccluderBox = GpuOccluderBox { min_m: Vec4::ZERO, max_m: Vec4::ONE };
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuOccluderBox {
    pub min_m: Vec4,
    pub max_m: Vec4,
}

/// `GpuSkyVisibilityParams` is the sky-visibility grid: origin, cell size, cell and box counts.
///
/// Example:
///     let params: GpuSkyVisibilityParams = GpuSkyVisibilityParams::default();
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpuSkyVisibilityParams {
    pub origin_m: Vec4,
    pub cell_size_m: Vec4,
    pub counts: UVec4,
}

/// `tests` checks that the GPU structs match the shaders' layouts.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | struct | each struct shared with MSL | `test_layouts_match_shaders` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_layouts_match_shaders` checks the sizes that the shaders' structs have.
    #[test]
    fn test_layouts_match_shaders() {
        assert_eq!(std::mem::size_of::<GpuFrameUniforms>(), 64 * 10 + 16 * 23);
        assert_eq!(std::mem::size_of::<GpuDrawUniforms>(), 96);
        assert_eq!(std::mem::size_of::<GpuParticle>(), 64);
        assert_eq!(std::mem::size_of::<crate::render::Vertex>(), 48);
        assert_eq!(std::mem::size_of::<crate::render::SkinVertex>(), 24);
    }
}
