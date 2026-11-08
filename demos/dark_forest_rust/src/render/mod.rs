//! `render` is the narrow interface between the game and a GPU backend: the game uploads meshes,
//! textures and materials once, then describes each frame as plain data in a `FrameDescription`.
//! Only `render::metal` talks to the GPU, so a Vulkan backend would implement `Renderer` too.
//!
//! Interface: `Renderer`, `Vertex`, `SkinVertex`, `MeshData`, `TextureData`, `MaterialData`,
//! `EnvironmentData`, `FrameDescription` and the types it holds, and `RenderSettings`.
//!
//! Example: `let mesh: MeshHandle = renderer.create_mesh(&mesh_data);`

pub mod culling;
pub mod metal;

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
/// matrices in `FrameDescription::joint_matrices`, for skinned meshes only.
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

/// `FrameDescription` is everything the backend draws in one frame. `lens_draws` and
/// `lens_particles` are the robots and laser effects among `world_draws` and `world_particles`,
/// which wormhole lenses also show live (the lens atlas holds only the static world).
pub struct FrameDescription<'a> {
    pub camera: Camera,
    pub world_draws: &'a [DrawItem],
    pub viewmodel_draws: &'a [DrawItem],
    pub joint_matrices: &'a [Mat4],
    pub point_lights: &'a [PointLight],
    pub decals: &'a [Decal],
    pub world_particles: &'a [Particle],
    pub viewmodel_particles: &'a [Particle],
    pub hud_rects: &'a [HudRect],
    pub hud_images: &'a [HudImage<'a>],
    pub time_s: f32,
    pub damage_flash_frac: f32,
    pub lens_draws: &'a [DrawItem],
    pub lens_particles: &'a [Particle],
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

/// `MAX_LENS_MOUTH_COUNT` is how many mouths the lens pass takes.
pub const MAX_LENS_MOUTH_COUNT: usize = 16;
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

/// `get_segment_distance_m` is how near the segment from `a_m` to `b_m` comes to `point_m`.
fn get_segment_distance_m(point_m: Vec3, a_m: Vec3, b_m: Vec3) -> f32 {
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
    pub shadow_distance_m: f32,
}

/// `FrameTiming` is how long the GPU took for a frame finished earlier.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameTiming {
    pub gpu_ms: f64,
    pub slot_wait_ms: f64,
    pub drawable_wait_ms: f64,
    pub cpu_render_ms: f64,
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
}
