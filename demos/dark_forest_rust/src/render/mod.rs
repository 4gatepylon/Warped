//! `render` is the narrow interface between the game and a GPU backend: the game uploads meshes,
//! textures and materials once, then describes each frame as plain data in a `FrameDescription`.
//! Only `render::metal` talks to the GPU, so a Vulkan backend would implement `Renderer` too.
//!
//! Interface: `Renderer`, `Vertex`, `SkinVertex`, `MeshData`, `TextureData`, `MaterialData`,
//! `EnvironmentData`, `FrameDescription` and the types it holds, and `RenderSettings`.
//!
//! Example: `let mesh: MeshHandle = renderer.create_mesh(&mesh_data);`

pub mod camera_math;
pub mod culling;
pub mod frame_clock;
pub mod frame_trace;
pub mod metal;
pub mod overlay;
pub mod tiles;

use glam::{Mat4, Vec3, Vec4};

/// `Vertex` is one vertex of a mesh, laid out exactly as the shaders read it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
}

/// `SkinVertex` is the joints and weights of one vertex of a skinned mesh.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SkinVertex {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

/// `MeshData` is the geometry of one mesh, in its own model space.
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub skin_vertices: Option<Vec<SkinVertex>>,
    pub indices: Vec<u32>,
}

/// `TextureFormat` is how a `TextureData`'s pixels are stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextureFormat {
    Rgba8Srgb,
    Rgba8Linear,
    Rg8Linear,
}

/// `TextureData` is one 2D texture's top mip level; the backend makes the other mip levels.
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub pixels: Vec<u8>,
}

/// `AlphaMode` is how a material's base color alpha is used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

/// `MaterialData` is a metallic-roughness PBR material. `occlusion_roughness_metal_texture`
/// packs ambient occlusion, roughness and metalness in R, G and B, as Poly Haven's ARM maps do.
#[derive(Clone)]
pub struct MaterialData {
    pub base_color_texture: Option<TextureHandle>,
    pub normal_texture: Option<TextureHandle>,
    pub occlusion_roughness_metal_texture: Option<TextureHandle>,
    pub emissive_texture: Option<TextureHandle>,
    pub base_color_factor: Vec4,
    pub roughness_factor: f32,
    pub metallic_factor: f32,
    pub emissive_factor: Vec3,
    pub has_occlusion_in_red: bool,
    pub alpha_mode: AlphaMode,
}

impl Default for MaterialData {
    /// `default` is a white, fully rough, non-metallic material without textures.
    fn default() -> Self {
        MaterialData {
            base_color_texture: None,
            normal_texture: None,
            occlusion_roughness_metal_texture: None,
            emissive_texture: None,
            base_color_factor: Vec4::ONE,
            roughness_factor: 1.0,
            metallic_factor: 0.0,
            emissive_factor: Vec3::ZERO,
            has_occlusion_in_red: false,
            alpha_mode: AlphaMode::Opaque,
        }
    }
}

/// `EnvironmentData` is the equirectangular HDR sky (linear RGB, 3 floats per pixel) with the
/// sun removed, plus the analytic sun that replaces it.
pub struct EnvironmentData {
    pub width: u32,
    pub height: u32,
    pub rgb_pixels: Vec<f32>,
    pub irradiance_sh9: [Vec3; 9],
    pub sun_direction: Vec3,
    pub sun_illuminance_rgb: Vec3,
    pub sun_angular_radius_rad: f32,
    pub atmosphere: Atmosphere,
}

/// `Atmosphere` is the art-directed look that goes with a sky: exposure, fog, and bounce light.
///
/// Example:
///     let atmosphere: Atmosphere = Atmosphere { exposure: 1.0, ..atmosphere };
#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    pub exposure: f32,
    pub ibl_intensity_frac: f32,
    pub fog_color_rgb: Vec3,
    pub fog_density_per_m: f32,
    pub fog_height_falloff_per_m: f32,
    pub fog_base_height_m: f32,
    pub fog_sun_inscatter_frac: f32,
    pub fog_max_opacity_frac: f32,
    pub bounce_color_rgb: Vec3,
    pub bloom_mix_frac: f32,
}

/// `OccluderBox` is one axis-aligned box that blocks sky light, for the sky visibility volume.
#[derive(Clone, Copy, Debug)]
pub struct OccluderBox {
    pub min_m: Vec3,
    pub max_m: Vec3,
}

/// `MeshHandle` names a mesh uploaded with `Renderer::create_mesh`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MeshHandle(pub u32);

/// `TextureHandle` names a texture uploaded with `Renderer::create_texture`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextureHandle(pub u32);

/// `MaterialHandle` names a material made with `Renderer::create_material`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MaterialHandle(pub u32);

/// `DrawItem` draws one mesh with one material. `joint_offset` is the first of the mesh's joint
/// matrices in `FrameDescription::joint_matrices`, for skinned meshes only. `body_flash_frac` marks
/// a player's body (the teal robot): its teal turns into `tint`'s team colour as on Lunar Surface
/// (`recolor_body` in `scene.metal`) instead of being multiplied by it, and it reddens by the
/// share given (1 just after a hit).
#[derive(Clone, Copy, Debug)]
pub struct DrawItem {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub model: Mat4,
    pub tint: Vec4,
    pub joint_offset: Option<u32>,
    pub bounds_center_m: Vec3,
    pub bounds_radius_m: f32,
    pub does_cast_shadow: bool,
    pub body_flash_frac: Option<f32>,
}

/// `Camera` is where the player looks from.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view: Mat4,
    pub position_m: Vec3,
    pub fov_y_rad: f32,
    pub viewmodel_fov_y_rad: f32,
    pub near_m: f32,
    pub far_m: f32,
}

/// `PointLight` is a light that falls off to zero at `radius_m`.
#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub position_m: Vec3,
    pub radius_m: f32,
    pub color_rgb: Vec3,
}

/// `Decal` is a bullet hole: a square of side `size_m` on a surface.
#[derive(Clone, Copy, Debug)]
pub struct Decal {
    pub position_m: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub size_m: f32,
    pub seed: f32,
}

/// `ParticleKind` is how a particle is shaded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum ParticleKind {
    Spark = 0,
    Smoke = 1,
    MuzzleFlash = 2,
    Tracer = 3,
}

/// `Particle` is a camera-facing sprite, stretched along `stretch_m` (for sparks and tracers).
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub position_m: Vec3,
    pub stretch_m: Vec3,
    pub size_m: f32,
    pub color_rgba: Vec4,
    pub kind: ParticleKind,
    pub seed: f32,
}

/// `Beam` is one straight piece of a laser drawn as Lunar Surface draws it: the sniper's a white
/// blade in a soft haze, the rifle's (`is_rifle`) a laser over its halo, in `rgb` (linear, 0 to 1),
/// with a share `speckle_frac` of its pixels speckling each frame. `has_previous` and `has_next`
/// say whether the laser goes on past `start_m` and `end_m`, so the joint is drawn once.
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    pub start_m: Vec3,
    pub end_m: Vec3,
    pub rgb: Vec3,
    pub is_rifle: bool,
    pub speckle_frac: f32,
    pub has_previous: bool,
    pub has_next: bool,
}

/// `HudRect` is a filled rectangle on the screen, in pixels from the top left of the window.
#[derive(Clone, Copy, Debug)]
pub struct HudRect {
    pub x_px: f32,
    pub y_px: f32,
    pub width_px: f32,
    pub height_px: f32,
    pub color_rgba: Vec4,
}

/// `HudImage` is an RGBA picture (8 bits a channel, sRGB, rows top first) drawn over the HUD's
/// rectangles with its top left at `x_px`, `y_px`, each of its pixels `scale` window pixels wide.
#[derive(Clone, Copy, Debug)]
pub struct HudImage<'a> {
    pub pixels_rgba: &'a [u8],
    pub width_px: u32,
    pub height_px: u32,
    pub x_px: f32,
    pub y_px: f32,
    pub scale: f32,
}

/// `FrameDescription` is everything the backend draws in one frame. The first
/// `static_draw_count` of `world_draws` are the same every frame (the arena), so the backend may
/// keep their shadows between frames. `beams` are the lasers in flight, drawn after the lit pass,
/// whose speckled pixels are black and white in `beam_speckle_shares` (the rest drop out).
/// `lens_draws` and
/// `lens_particles` are the robots and laser effects among `world_draws` and `world_particles`,
/// which wormhole lenses also show live (the lens atlas holds only the static world).
/// `thrown_lens_mouths` are the players' thrown mouths, in linked pairs; the backend draws each
/// one's view of the static world over a few frames once it appears, and shows its lens from then.
/// `overlay_rects` are the HUD's bars and crosshair, in the Lunar Surface demo's look, drawn over
/// `hud_rects` and under `hud_images`.
pub struct FrameDescription<'a> {
    pub camera: Camera,
    pub world_draws: &'a [DrawItem],
    pub static_draw_count: usize,
    pub viewmodel_draws: &'a [DrawItem],
    pub joint_matrices: &'a [Mat4],
    pub point_lights: &'a [PointLight],
    pub decals: &'a [Decal],
    pub world_particles: &'a [Particle],
    pub viewmodel_particles: &'a [Particle],
    pub hud_rects: &'a [HudRect],
    pub hud_images: &'a [HudImage<'a>],
    pub time_s: f32,
    pub hit_effect: overlay::HitEffect,
    pub lens_draws: &'a [DrawItem],
    pub lens_particles: &'a [Particle],
    pub thrown_lens_mouths: &'a [ThrownLensMouth],
    pub overlay_rects: &'a [overlay::OverlayRect],
    pub beams: &'a [Beam],
    pub beam_speckle_shares: glam::Vec2,
}

/// `LensMouth` is one wormhole mouth as the lens pass sees it: its lens throat (where rays cross),
/// reach, foil λ², the floor height under it, and its partner's index in `WormholeLens::mouths`.
#[derive(Clone, Copy, Debug)]
pub struct LensMouth {
    pub centre_m: Vec3,
    pub lens_throat_m: f32,
    pub reach_m: f32,
    pub lambda_squared: f32,
    pub floor_m: f32,
    pub partner: usize,
}

/// `ThrownLensMouth` is a player's thrown mouth as the lens pass sees it: `mouth`, whose `partner`
/// indexes the frame's `thrown_lens_mouths`, and the colour (linear RGB) that tints what shows
/// through it (its own, or red while it flashes from a hit).
#[derive(Clone, Copy, Debug)]
pub struct ThrownLensMouth {
    pub mouth: LensMouth,
    pub tint_rgb: Vec3,
}

/// `MAX_THROWN_LENS_MOUTH_COUNT` is how many thrown mouths the lens shows at once (every player's
/// pair), and `THROWN_LENS_FACE_PX` the side of each face of a thrown mouth's view of the world.
pub const MAX_THROWN_LENS_MOUTH_COUNT: usize = 16;
pub const THROWN_LENS_FACE_PX: usize = 128;

/// `WormholeLens` is what the lens pass needs: every mouth, and `atlas`, the world seen from each
/// mouth's centre: one row per mouth of six square 90° faces, oriented as `LENS_FACES`.
pub struct WormholeLens {
    pub mouths: Vec<LensMouth>,
    pub atlas: TextureData,
}

/// `LENS_FACES` are the lens atlas's cube faces' (forward, up) directions, in atlas order; the
/// shader's `sample_lens_atlas` matches them.
pub const LENS_FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::Y),
    (Vec3::NEG_X, Vec3::Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
    (Vec3::Z, Vec3::Y),
    (Vec3::NEG_Z, Vec3::Y),
];

/// `MAX_LENS_MOUTH_COUNT` is how many mouths the lens pass takes, the map's and the thrown ones;
/// `post.metal`'s `LensUniforms` agrees.
pub const MAX_LENS_MOUTH_COUNT: usize = 32;
/// `LENS_DEPTH_NEAR_M` and `LENS_DEPTH_FAR_M` bound the lens atlas's alpha: the static world's
/// distance along each face's axis, log-encoded from 0 at the near bound to 1 at the far bound (and
/// the sky); `post.metal` decodes it with the same bounds.
pub const LENS_DEPTH_NEAR_M: f32 = 0.25;
pub const LENS_DEPTH_FAR_M: f32 = 128.0;
/// `MAX_LIVE_LENS_SLOT_COUNT` is how many mouths' surroundings the lens shows live at once.
pub const MAX_LIVE_LENS_SLOT_COUNT: usize = 4;
/// `LIVE_LENS_RANGE_M` is how near a mouth a robot or laser must come to show through the lens.
pub const LIVE_LENS_RANGE_M: f32 = 50.0;

/// `encode_lens_depth` is the lens atlas's alpha byte for a pixel of reverse-Z `depth` (0 for the
/// sky) drawn with near plane `near_m`.
///
/// Example: `let alpha: u8 = encode_lens_depth(0.05, depth);`
pub fn encode_lens_depth(near_m: f32, depth: f32) -> u8 {
    let along_m: f32 = if depth > 0.0 {
        near_m / depth
    } else {
        LENS_DEPTH_FAR_M
    };
    let frac: f32 =
        (along_m / LENS_DEPTH_NEAR_M).ln() / (LENS_DEPTH_FAR_M / LENS_DEPTH_NEAR_M).ln();
    (frac.clamp(0.0, 1.0) * f32::from(u8::MAX)).round() as u8
}

/// `MAX_LENS_GLOW_SEGMENT_COUNT` is how many laser and glow segments the lens pass looks for along
/// its rays; `post.metal`'s `LensUniforms` agrees.
pub const MAX_LENS_GLOW_SEGMENT_COUNT: usize = 24;
/// `GLOW_JOIN_COS` is how parallel two tracer dashes must be (the cosine of the angle between them)
/// to be joined into one segment.
const GLOW_JOIN_COS: f32 = 0.999;

/// `get_glow_segments` turns the particles the lens shows into segments (start, end, radius): runs
/// of overlapping, parallel tracer dashes (a laser beam, cut into dashes) joined into one segment
/// each, and every other particle as its own (a point, for eye glows and sparks).
///
/// Example: `let glows: Vec<(Vec3, Vec3, f32)> = get_glow_segments(&frame.lens_particles);`
pub fn get_glow_segments(particles: &[Particle]) -> Vec<(Vec3, Vec3, f32)> {
    let mut segments: Vec<(Vec3, Vec3, f32)> = Vec::new();
    let mut is_last_tracer: bool = false;
    for particle in particles {
        let half_m: Vec3 = particle.stretch_m * 0.5;
        let (start_m, end_m) = (particle.position_m - half_m, particle.position_m + half_m);
        let is_tracer: bool = particle.kind == ParticleKind::Tracer;
        if let Some(last) = segments.last_mut()
            && is_tracer
            && is_last_tracer
            && last.1.distance(start_m) <= particle.stretch_m.length()
            && (last.1 - last.0)
                .normalize_or_zero()
                .dot(particle.stretch_m.normalize_or_zero())
                >= GLOW_JOIN_COS
        {
            last.1 = end_m;
        } else {
            segments.push((start_m, end_m, particle.size_m));
        }
        is_last_tracer = is_tracer;
    }
    segments
}

/// `get_segment_distance_m` is how near the segment from `a_m` to `b_m` comes to `point_m`.
pub fn get_segment_distance_m(point_m: Vec3, a_m: Vec3, b_m: Vec3) -> f32 {
    let ab: Vec3 = b_m - a_m;
    let along_frac: f32 = (point_m - a_m).dot(ab) / ab.length_squared().max(1e-12);
    point_m.distance(a_m + ab * along_frac.clamp(0.0, 1.0))
}

/// `pick_live_lens_mouths` chooses whose surroundings the lens shows live this frame, in slot
/// order: for each mouth in `is_visible`, nearest the camera first, its partner (seen through it)
/// and then itself (seen bent around it), each only if a mover (a segment) comes within
/// `LIVE_LENS_RANGE_M` of it, at most `MAX_LIVE_LENS_SLOT_COUNT` mouths.
///
/// Example: `let slots: Vec<usize> = pick_live_lens_mouths(&mouths, camera_m, &is_visible, &movers);`
pub fn pick_live_lens_mouths(
    mouths: &[LensMouth],
    camera_m: Vec3,
    is_visible: &[bool],
    movers: &[(Vec3, Vec3)],
) -> Vec<usize> {
    let mut visible: Vec<usize> = (0..mouths.len()).filter(|&i| is_visible[i]).collect();
    visible.sort_by(|&a, &b| {
        let a_m: f32 = mouths[a].centre_m.distance_squared(camera_m);
        a_m.total_cmp(&mouths[b].centre_m.distance_squared(camera_m))
    });
    let mut slots: Vec<usize> = Vec::new();
    for index in visible.into_iter().flat_map(|i| [mouths[i].partner, i]) {
        let centre_m: Vec3 = mouths[index].centre_m;
        let is_near: bool = movers
            .iter()
            .any(|&(a_m, b_m)| get_segment_distance_m(centre_m, a_m, b_m) < LIVE_LENS_RANGE_M);
        if is_near && !slots.contains(&index) && slots.len() < MAX_LIVE_LENS_SLOT_COUNT {
            slots.push(index);
        }
    }
    slots
}

/// `MAX_BOUNDARY_PLANE_COUNT` is how many team boundary planes the tone map draws, `post.metal`'s.
pub const MAX_BOUNDARY_PLANE_COUNT: usize = 4;

/// `GpuBoundaryPlanes` matches `post.metal`'s `BoundaryPlanes` (W-148, team boundary planes in
/// every level; W-149, shimmer through boundary planes): per plane (z in meters, team, 0, 0), then
/// (height in meters, opacity at the foot, the shimmer's clock in seconds, 0), the planes' extent
/// (x from, x to, the ground's height, 0), and (plane count, 0, 0, 0).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuBoundaryPlanes {
    pub planes: [[f32; 4]; MAX_BOUNDARY_PLANE_COUNT],
    pub look: [f32; 4],
    pub extent: [f32; 4],
    pub counts: [i32; 4],
}

/// `RenderSettings` are the quality switches that can change while the demo runs.
#[derive(Clone, Copy, Debug)]
pub struct RenderSettings {
    pub render_scale_frac: f32,
    pub is_ssao_enabled: bool,
    pub is_bloom_enabled: bool,
    pub is_fxaa_enabled: bool,
    pub is_color_grade_enabled: bool,
    pub is_volumetric_haze_enabled: bool,
    pub is_material_response_enabled: bool,
    pub is_vsync_enabled: bool,
    pub is_foveation_enabled: bool,
    pub shadow_distance_m: f32,
}

/// `FrameTiming` is how long the GPU took for a frame finished earlier, how long this frame's
/// encoding waited and took, and this frame's index (for `frame_trace`).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTiming {
    pub gpu_ms: f64,
    pub slot_wait_ms: f64,
    pub drawable_wait_ms: f64,
    pub cpu_render_ms: f64,
    pub frame_index: usize,
}

/// `Renderer` is the whole backend interface.
pub trait Renderer {
    /// `create_texture` uploads a texture and builds its mip levels.
    fn create_texture(&mut self, texture: &TextureData) -> TextureHandle;
    /// `create_mesh` uploads a mesh.
    fn create_mesh(&mut self, mesh: &MeshData) -> MeshHandle;
    /// `create_material` stores a material.
    fn create_material(&mut self, material: &MaterialData) -> MaterialHandle;
    /// `set_environment` builds the sky, the image-based lighting and the sun from an HDRI.
    fn set_environment(&mut self, environment: &EnvironmentData);
    /// `set_occluders` bakes how much sky each point of the arena sees, from boxes that block it.
    fn set_occluders(&mut self, occluders: &[OccluderBox], bounds_min_m: Vec3, bounds_max_m: Vec3);
    /// `set_boundary_planes` sets the team boundary planes the tone map draws (W-148).
    fn set_boundary_planes(&mut self, planes: GpuBoundaryPlanes);
    /// `render_frame` draws and presents one frame at the window's current size in pixels.
    /// `resize` sets the size, in pixels, of the window's drawable.
    fn resize(&mut self, width_px: u32, height_px: u32);
    fn render_frame(&mut self, frame: &FrameDescription, settings: &RenderSettings) -> FrameTiming;
    /// `read_frame_rgba` waits for the frames in flight and gives the last one's picture before
    /// the HUD, at the internal resolution: (width_px, height_px, RGBA bytes).
    fn read_frame_rgba(&mut self) -> (u32, u32, Vec<u8>);
    /// `read_frame_depth` is `read_frame_rgba`'s depth: reverse Z, near plane over distance
    /// along the view axis, 0 for the sky (opaque surfaces only).
    fn read_frame_depth(&mut self) -> (u32, u32, Vec<f32>);
    /// `set_wormhole_lens` bends the view through these wormhole mouths from now on (None stops).
    fn set_wormhole_lens(&mut self, lens: Option<&WormholeLens>);
    /// `set_is_capturing` leaves the vignette and grain out of frames, for pictures that become
    /// textures.
    fn set_is_capturing(&mut self, is_capturing: bool);
    /// `set_is_capturing_hud` draws frames, HUD included, into a picture instead of the window
    /// (from the next frame on, at the window's current size), for `read_hud_frame_rgba`.
    fn set_is_capturing_hud(&mut self, is_capturing_hud: bool);
    /// `read_hud_frame_rgba` waits for the frames in flight and gives the last one with its HUD,
    /// as the window would show it: (width_px, height_px, RGBA bytes).
    fn read_hud_frame_rgba(&mut self) -> (u32, u32, Vec<u8>);
}

/// `tests` checks which mouths the lens shows live, the atlas's depth encoding, and that a robot
/// beyond a gate is on the traced ray through it.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | mover vs visible mouth | near its partner only | `test_robot_beyond_a_gate_is_found_through_it` |
/// | mover vs visible mouth | near it and its partner | `test_live_mouths_take_partner_then_self` |
/// | mover vs visible mouth | near neither | `test_live_mouths_need_a_mover_and_a_visible_mouth` |
/// | mouth visibility | visible, not visible | `test_live_mouths_need_a_mover_and_a_visible_mouth` |
/// | candidate count | at most the slots, more than the slots | `test_live_mouths_take_partner_then_self`, `test_live_mouths_keep_the_nearest_when_full` |
/// | mover shape | a point (robot), a long segment (laser) | `test_live_mouths_need_a_mover_and_a_visible_mouth` |
/// | depth | sky, nearer than the near bound, between the bounds | `test_lens_depth_encoding` |
/// | glow particles | one beam's dashes, two beams' dashes back to back, a point glow between dashes | `test_beam_dashes_join_into_segments` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::warps::{BeamPath, Warps, WormholeMouth};

    /// `CENTRE_GATE_M` are the Dark Forest's centre gate's mouths, `GATE_THROAT_M` and `GATE_REACH_M`
    /// their walk-in throat and reach, and `GATE_LAMBDA_SQUARED` their foil's λ².
    const CENTRE_GATE_M: [Vec3; 2] = [Vec3::new(-2.0, 1.7, 24.0), Vec3::new(2.0, 1.7, -24.0)];
    const GATE_THROAT_M: f32 = 1.6;
    const GATE_REACH_M: f32 = 5.0;
    const GATE_LAMBDA_SQUARED: f32 = 0.25;

    /// `lens_mouth` is a mouth at `centre_m` paired with mouth `partner`, sized as a ground gate.
    fn lens_mouth(centre_m: Vec3, partner: usize) -> LensMouth {
        LensMouth {
            centre_m,
            lens_throat_m: GATE_THROAT_M * 0.5,
            reach_m: GATE_REACH_M,
            lambda_squared: GATE_LAMBDA_SQUARED,
            floor_m: 0.0,
            partner,
        }
    }

    /// `test_robot_beyond_a_gate_is_found_through_it` checks that an eye ray into the centre gate,
    /// traced as the lens traces it (`Warps::trace_beam`), comes out of the partner and meets a
    /// robot standing 6 m beyond it, and that the partner, not the mouth in view, gets the live slot.
    #[test]
    fn test_robot_beyond_a_gate_is_found_through_it() {
        let warps: Warps = Warps {
            black_holes: Vec::new(),
            mouths: (0..2)
                .map(|index| WormholeMouth {
                    centre_m: CENTRE_GATE_M[index],
                    throat_radius_m: GATE_THROAT_M,
                    reach_m: GATE_REACH_M,
                    lambda_squared: GATE_LAMBDA_SQUARED,
                    partner: 1 - index,
                })
                .collect(),
        };
        let robot_centre_m: Vec3 = Vec3::new(2.0, 1.0, -30.0);
        let robot_radius_m: f32 = 0.5;
        let eye_m: Vec3 = CENTRE_GATE_M[0] + Vec3::new(0.0, -0.1, 6.5);
        let path: BeamPath = warps.trace_beam(eye_m, Vec3::NEG_Z, 60.0, |x, v, length_m| {
            let oc: Vec3 = x - robot_centre_m;
            let b: f32 = oc.dot(v);
            let disc: f32 = b * b - (oc.length_squared() - robot_radius_m * robot_radius_m);
            let t: f32 = -b - disc.max(0.0).sqrt();
            (disc > 0.0 && (0.0..length_m).contains(&t)).then_some(t)
        });
        assert!(
            path.end_m.distance(robot_centre_m) <= robot_radius_m + 1e-3,
            "the ray ends at {:?}",
            path.end_m
        );
        let mouths: Vec<LensMouth> = vec![
            lens_mouth(CENTRE_GATE_M[0], 1),
            lens_mouth(CENTRE_GATE_M[1], 0),
        ];
        let slots: Vec<usize> = pick_live_lens_mouths(
            &mouths,
            eye_m,
            &[true, false],
            &[(robot_centre_m, robot_centre_m)],
        );
        assert_eq!(slots, vec![1]);
    }

    /// `test_live_mouths_take_partner_then_self` checks that a visible mouth with movers near both
    /// ends gives its partner the first slot and itself the second.
    #[test]
    fn test_live_mouths_take_partner_then_self() {
        let mouths: Vec<LensMouth> = vec![
            lens_mouth(Vec3::new(0.0, 1.7, 0.0), 1),
            lens_mouth(Vec3::new(200.0, 1.7, 0.0), 0),
        ];
        let movers: Vec<(Vec3, Vec3)> = vec![
            (Vec3::new(3.0, 1.0, 0.0), Vec3::new(3.0, 1.0, 0.0)),
            (Vec3::new(203.0, 1.0, 0.0), Vec3::new(203.0, 1.0, 0.0)),
        ];
        let slots: Vec<usize> =
            pick_live_lens_mouths(&mouths, Vec3::new(0.0, 1.7, 10.0), &[true, false], &movers);
        assert_eq!(slots, vec![1, 0]);
    }

    /// `test_live_mouths_need_a_mover_and_a_visible_mouth` checks that no mouth is live without a
    /// visible mouth or without a mover in range, and that a laser crossing a mouth counts even
    /// though both its ends are far away.
    #[test]
    fn test_live_mouths_need_a_mover_and_a_visible_mouth() {
        let mouths: Vec<LensMouth> = vec![
            lens_mouth(Vec3::new(0.0, 1.7, 0.0), 1),
            lens_mouth(Vec3::new(200.0, 1.7, 0.0), 0),
        ];
        let camera_m: Vec3 = Vec3::new(0.0, 1.7, 10.0);
        let far_robot: (Vec3, Vec3) = (Vec3::new(100.0, 1.0, 0.0), Vec3::new(100.0, 1.0, 0.0));
        let near_robot: (Vec3, Vec3) = (Vec3::new(203.0, 1.0, 0.0), Vec3::new(203.0, 1.0, 0.0));
        let laser: (Vec3, Vec3) = (Vec3::new(200.0, 1.7, 100.0), Vec3::new(200.0, 1.7, -100.0));
        assert!(pick_live_lens_mouths(&mouths, camera_m, &[true, false], &[far_robot]).is_empty());
        assert!(
            pick_live_lens_mouths(&mouths, camera_m, &[false, false], &[near_robot]).is_empty()
        );
        assert_eq!(
            pick_live_lens_mouths(&mouths, camera_m, &[true, false], &[laser]),
            vec![1]
        );
    }

    /// `test_live_mouths_keep_the_nearest_when_full` checks that with more live candidates than
    /// slots, the mouths seen nearest the camera keep theirs.
    #[test]
    fn test_live_mouths_keep_the_nearest_when_full() {
        let mouths: Vec<LensMouth> = (0..6)
            .map(|index| lens_mouth(Vec3::new(index as f32 * 10.0, 1.7, 0.0), index ^ 1))
            .collect();
        let movers: Vec<(Vec3, Vec3)> = mouths
            .iter()
            .map(|mouth| (mouth.centre_m, mouth.centre_m))
            .collect();
        let slots: Vec<usize> =
            pick_live_lens_mouths(&mouths, Vec3::new(55.0, 1.7, 0.0), &[true; 6], &movers);
        assert_eq!(slots, vec![4, 5, 2, 3]);
    }

    /// `test_beam_dashes_join_into_segments` checks that a beam's overlapping dashes become one
    /// segment, that a second beam turning off from its end and a point glow stay apart.
    #[test]
    fn test_beam_dashes_join_into_segments() {
        let dash = |position_m: Vec3, direction: Vec3| Particle {
            position_m,
            stretch_m: direction * 0.3,
            size_m: 0.02,
            color_rgba: Vec4::ONE,
            kind: ParticleKind::Tracer,
            seed: 0.0,
        };
        let glow: Particle = Particle {
            stretch_m: Vec3::ZERO,
            size_m: 0.07,
            kind: ParticleKind::MuzzleFlash,
            ..dash(Vec3::new(9.0, 1.0, 0.0), Vec3::X)
        };
        let mut particles: Vec<Particle> = (0..10)
            .map(|i| dash(Vec3::new(i as f32 * 0.15, 1.0, 0.0), Vec3::X))
            .collect();
        particles.extend((0..4).map(|i| dash(Vec3::new(1.5, 1.0 + i as f32 * 0.15, 0.0), Vec3::Y)));
        particles.push(glow);
        let segments: Vec<(Vec3, Vec3, f32)> = get_glow_segments(&particles);
        assert_eq!(segments.len(), 3, "{segments:?}");
        assert!(segments[0].0.distance(Vec3::new(-0.15, 1.0, 0.0)) < 1e-5);
        assert!(segments[0].1.distance(Vec3::new(1.5, 1.0, 0.0)) < 1e-5);
        assert!(segments[1].1.distance(Vec3::new(1.5, 1.6, 0.0)) < 1e-5);
        assert_eq!(segments[2], (glow.position_m, glow.position_m, glow.size_m));
    }

    /// `test_lens_depth_encoding` checks the sky and the far bound encode to 255, anything nearer
    /// than the near bound to 0, and a distance in between to within one step of its value.
    #[test]
    fn test_lens_depth_encoding() {
        let near_m: f32 = 0.05;
        assert_eq!(encode_lens_depth(near_m, 0.0), u8::MAX);
        assert_eq!(
            encode_lens_depth(near_m, near_m / LENS_DEPTH_FAR_M),
            u8::MAX
        );
        assert_eq!(encode_lens_depth(near_m, near_m / 0.1), 0);
        let along_m: f32 = 9.0;
        let frac: f32 = f32::from(encode_lens_depth(near_m, near_m / along_m)) / 255.0;
        let decoded_m: f32 = LENS_DEPTH_NEAR_M * (LENS_DEPTH_FAR_M / LENS_DEPTH_NEAR_M).powf(frac);
        assert!((decoded_m / along_m - 1.0).abs() < 0.025, "{decoded_m}");
    }
}
