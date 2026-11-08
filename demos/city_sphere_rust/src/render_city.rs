//! `render_city` draws City Sphere with Metal rasterization (light goes straight in its Euclidean
//! space, so no ray marching is needed): the street sphere and buildings, textured with the
//! fps_demo's CC0 Poly Haven textures and lit by the glowing core; consoles, pylons and robots
//! (per frame); the see-through glass shell with its lift holes; and laser beams.
//!
//! Interface: `CityVertex`, `CityScene`, `build_scene`, `CityFrame`, `CityGpu`, `push_box`,
//! `push_mesh`, `push_beam`, `MAX_HOLE_COUNT`.

use std::error::Error;
use std::ptr::NonNull;
use std::sync::Arc;

use glam::{DMat3, DVec3, Mat4, Vec3};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCompareFunction, MTLCullMode,
    MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice, MTLLibrary, MTLLoadAction, MTLPixelFormat,
    MTLPrimitiveType, MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResourceOptions, MTLScissorRect, MTLStorageMode, MTLStoreAction, MTLTexture,
    MTLTextureDescriptor, MTLTextureUsage, MTLViewport,
};
use rayon::prelude::*;

use crate::city_world::{CityWorld, get_up};
use crate::fps_demo_assets::get_asset_path;
use crate::mesh::{MeshMaterial, MeshModel};
use crate::voxel_textures::{make_array, make_mips};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Texture = Retained<ProtocolObject<dyn MTLTexture>>;
type Pipeline = Retained<ProtocolObject<dyn MTLRenderPipelineState>>;
type Buffer = Retained<ProtocolObject<dyn MTLBuffer>>;

const SHADER_SOURCE: &str = include_str!("render_city.metal");
pub const MAX_HOLE_COUNT: usize = 8;
const TEXTURE_SIDE_PX: u32 = 1024;
pub const BEACON_SLOT_COUNT: usize = 8;
const BEACON_TEAM_COLORS: [[f32; 3]; 2] = [[0.3, 0.55, 1.0], [1.0, 0.3, 0.25]];
const BEACON_FLAG: f32 = 2.0;
const BEACON_LAYER: usize = TEXTURE_IDS.len();
const BEACON_MODEL: &str = "scifi_beacon";
const TEXTURE_IDS: [&str; 4] = ["metal_plate", "metal_plate_02", "blue_metal_plate", "metal_plate"];
const LAYER_STREET: f32 = 0.0;
const LAYER_CONCRETE: f32 = 1.0;
const LAYER_BRICK: f32 = 2.0;
const LAYER_METAL: f32 = 3.0;
const NO_LAYER: f32 = -1.0;
const NEAR_M: f32 = 0.05;
const FAR_M: f32 = 200.0;
const STREET_TILE_M: f64 = 4.0;
const WALL_TILE_M: f64 = 3.0;
const GLASS_CELL_M: f64 = 2.5;
const SPHERE_LON_COUNT: usize = 128;
const SPHERE_LAT_COUNT: usize = 64;
const CORE_LON_COUNT: usize = 32;
const CORE_LAT_COUNT: usize = 16;
const SKY_COLOR: [f32; 4] = [0.05, 0.03, 0.08, 1.0];
const CORE_LIGHT: [f32; 4] = [1.0, 0.86, 1.0, 1.0];
const AMBIENT: [f32; 4] = [0.374, 0.34, 0.476, 0.0];
const FOG_M: f32 = 90.0;
const GLASS_TINT: [f32; 4] = [0.55, 0.75, 1.0, 0.0];

/// `CityVertex` is one corner: position and normal (metres, world), texture coordinates, colour
/// (alpha 1 for glowing, 0 for lit) and texture layer (-1 for none). The shader's `CityVertex`
/// matches it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CityVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub layer: f32,
}

/// `CityUniforms` is the frame's camera and lighting; the shader's `CityUniforms` matches it.
#[repr(C)]
#[derive(Clone, Copy)]
struct CityUniforms {
    clip_from_world: [[f32; 4]; 4],
    camera_m: [f32; 4],
    core_light: [f32; 4],
    ambient: [f32; 4],
    holes: [[f32; 4]; MAX_HOLE_COUNT],
    glass: [f32; 4],
    sky_color: [f32; 4],
    beacons: [[f32; 4]; BEACON_SLOT_COUNT],
}

/// `CityScene` is the geometry that never changes: opaque triangles and the glass's, with an id
/// so the GPU copy is made once.
pub struct CityScene {
    pub id: u64,
    pub opaque: Vec<CityVertex>,
    pub glass: Vec<CityVertex>,
    pub holes: Vec<[f32; 4]>,
    pub glass_radius_m: f32,
}

/// `CityFrame` is one frame to draw: the scene, the camera (world to camera, the vertical
/// half-angle tangent), and this frame's opaque triangles (consoles, pylons, robots) and beams.
pub struct CityFrame {
    pub render_px: (usize, usize),
    pub scene: Arc<CityScene>,
    pub camera_from_world: Mat4,
    pub camera_m: Vec3,
    pub tan_half_fov: f32,
    pub beacons: [[f32; 4]; BEACON_SLOT_COUNT],
    pub dynamic: Vec<CityVertex>,
    pub beams: Vec<CityVertex>,
}

impl CityFrame {
    /// `get_render_size` gives the frame's render size in pixels.
    pub fn get_render_size(&self) -> (usize, usize) {
        self.render_px
    }

    /// `zoom_in` narrows the view by `zoom` (aiming).
    pub fn zoom_in(&mut self, zoom: f64) {
        self.tan_half_fov /= zoom.max(1.0) as f32;
    }

    /// `get_clip_from_world` is the perspective onto Metal's 0..1 depth times the camera.
    fn get_clip_from_world(&self) -> Mat4 {
        let aspect: f32 = self.render_px.0 as f32 / self.render_px.1.max(1) as f32;
        let depth: f32 = FAR_M / (NEAR_M - FAR_M);
        let clip_from_camera: Mat4 = Mat4::from_cols_array_2d(&[
            [1.0 / (self.tan_half_fov * aspect), 0.0, 0.0, 0.0],
            [0.0, 1.0 / self.tan_half_fov, 0.0, 0.0],
            [0.0, 0.0, depth, -1.0],
            [0.0, 0.0, depth * NEAR_M, 0.0],
        ]);
        clip_from_camera * self.camera_from_world
    }
}

/// `to_f32` turns a vector into a shader triple.
fn to_f32(v: DVec3) -> [f32; 3] {
    v.as_vec3().to_array()
}

/// `push_quad` adds two triangles a, b, c and a, c, d with one normal and corner uvs.
fn push_quad(out: &mut Vec<CityVertex>, corners: [DVec3; 4], uvs: [[f32; 2]; 4], normal: DVec3, color: [f32; 4], layer: f32) {
    for k in [0, 1, 2, 0, 2, 3] {
        out.push(CityVertex {
            position: to_f32(corners[k]),
            normal: to_f32(normal),
            uv: uvs[k],
            color,
            layer,
        });
    }
}

/// `push_box` adds a box's six faces, textures tiled every `WALL_TILE_M`.
///
/// Example: `push_box(&mut vertices, center, axes, half_m, [1.0; 4], -1.0);`
pub fn push_box(out: &mut Vec<CityVertex>, center: DVec3, axes: DMat3, half_m: DVec3, color: [f32; 4], layer: f32) {
    for axis in 0..3 {
        for sign in [-1.0, 1.0] {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let n: DVec3 = axes.col(axis) * sign;
            let du: DVec3 = axes.col(u) * half_m[u];
            let dv: DVec3 = axes.col(v) * half_m[v];
            let c: DVec3 = center + n * half_m[axis];
            let (su, sv) = ((2.0 * half_m[u] / WALL_TILE_M) as f32, (2.0 * half_m[v] / WALL_TILE_M) as f32);
            let corners: [DVec3; 4] = if sign > 0.0 {
                [c - du - dv, c + du - dv, c + du + dv, c - du + dv]
            } else {
                [c - du - dv, c - du + dv, c + du + dv, c + du - dv]
            };
            let uvs: [[f32; 2]; 4] = if sign > 0.0 {
                [[0.0, 0.0], [su, 0.0], [su, sv], [0.0, sv]]
            } else {
                [[0.0, 0.0], [0.0, sv], [su, sv], [su, 0.0]]
            };
            push_quad(out, corners, uvs, n, color, layer);
        }
    }
}

/// `push_sphere` adds a latitude-longitude sphere about the centre, its normals inward if
/// `is_inward`, textures tiled every `tile_m` metres along the surface.
fn push_sphere(out: &mut Vec<CityVertex>, radius_m: f64, is_inward: bool, counts: (usize, usize), tile_m: f64, color: [f32; 4], layer: f32) {
    let (lon_count, lat_count) = counts;
    let point = |i: usize, j: usize| -> DVec3 {
        let lon: f64 = i as f64 / lon_count as f64 * std::f64::consts::TAU;
        let lat: f64 = (j as f64 / lat_count as f64 - 0.5) * std::f64::consts::PI;
        DVec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin())
    };
    let uv = |i: usize, j: usize| -> [f32; 2] {
        let around_m: f64 = i as f64 / lon_count as f64 * std::f64::consts::TAU * radius_m;
        let up_m: f64 = j as f64 / lat_count as f64 * std::f64::consts::PI * radius_m;
        [(around_m / tile_m) as f32, (up_m / tile_m) as f32]
    };
    for i in 0..lon_count {
        for j in 0..lat_count {
            let ids: [(usize, usize); 4] = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
            let ids: [(usize, usize); 4] = if is_inward { ids } else { [ids[0], ids[3], ids[2], ids[1]] };
            for k in [0, 1, 2, 0, 2, 3] {
                let (a, b) = ids[k];
                let d: DVec3 = point(a, b);
                out.push(CityVertex {
                    position: to_f32(d * radius_m),
                    normal: to_f32(if is_inward { -d } else { d }),
                    uv: uv(a, b),
                    color,
                    layer,
                });
            }
        }
    }
}

/// `get_layer` picks a material's texture by its name.
fn get_layer(name: &str) -> f32 {
    match name {
        "street" => LAYER_STREET,
        "building" => LAYER_CONCRETE,
        "brick" => LAYER_BRICK,
        "glass obstacle" => LAYER_METAL,
        _ => NO_LAYER,
    }
}

/// `build_scene` makes the static geometry: the street, every box but the consoles and pylons
/// (drawn per frame, since they change colour), the core and the glass.
///
/// Example: `let scene = build_scene(&world);`
pub fn build_scene(world: &CityWorld) -> CityScene {
    let mut opaque: Vec<CityVertex> = Vec::new();
    let street = &world.materials[world.street_material as usize];
    let street_color: [f32; 4] = [street.color[0] * 2.4, street.color[1] * 2.4, street.color[2] * 2.4, 0.0];
    push_sphere(&mut opaque, world.street_radius_m, true, (SPHERE_LON_COUNT, SPHERE_LAT_COUNT), STREET_TILE_M, street_color, LAYER_STREET);
    for city_box in world.boxes.iter().filter(|b| b.group != "consoles" && b.group != "pylons") {
        let material = &world.materials[city_box.material as usize];
        let layer: f32 = get_layer(&material.name);
        let tint: f32 = if layer >= 0.0 { 1.6 } else { 1.0 };
        let c = material.color;
        let color: [f32; 4] = [c[0] * tint, c[1] * tint, c[2] * tint, if material.is_light { 1.0 } else { 0.0 }];
        push_box(&mut opaque, city_box.center, city_box.axes, city_box.half_m, color, layer);
    }
    let mut glass: Vec<CityVertex> = Vec::new();
    let mut holes: Vec<[f32; 4]> = Vec::new();
    let mut glass_radius_m: f32 = 0.0;
    for shell in &world.shells {
        let material = &world.materials[shell.material as usize];
        if shell.is_ball() {
            let c = material.color;
            push_sphere(&mut opaque, shell.radius_m, false, (CORE_LON_COUNT, CORE_LAT_COUNT), 1.0, [c[0], c[1], c[2], 1.0], NO_LAYER);
        } else {
            push_sphere(&mut glass, shell.radius_m, true, (SPHERE_LON_COUNT, SPHERE_LAT_COUNT), GLASS_CELL_M, GLASS_TINT, NO_LAYER);
            glass_radius_m = shell.radius_m as f32;
            holes = shell
                .holes
                .iter()
                .take(MAX_HOLE_COUNT)
                .map(|&(d, cos)| [d.x as f32, d.y as f32, d.z as f32, cos as f32])
                .collect();
        }
    }
    push_pylon_beacons(&mut opaque, world);
    CityScene {
        id: 1,
        opaque,
        glass,
        holes,
        glass_radius_m,
    }
}

/// `push_mesh` adds a posed model's triangles placed by `world_from_model`, its colours times `tint`.
///
/// Example: `push_mesh(&mut vertices, &posed, body.get_materials(), frame, [0.4, 0.6, 1.0]);`
pub fn push_mesh(out: &mut Vec<CityVertex>, model: &MeshModel, materials: &[MeshMaterial], world_from_model: Mat4, tint: [f32; 3]) {
    let normal_matrix: glam::Mat3 = glam::Mat3::from_mat4(world_from_model);
    for triangle in &model.triangles {
        let Some(material) = materials.get(triangle.material) else {
            continue;
        };
        let base = material.base_color;
        let glow: bool = material.emissive.iter().sum::<f32>() > 0.5;
        let color: [f32; 4] = [base[0] * tint[0], base[1] * tint[1], base[2] * tint[2], if glow { 1.0 } else { 0.0 }];
        for k in 0..3 {
            out.push(CityVertex {
                position: world_from_model.transform_point3(triangle.p[k]).to_array(),
                normal: (normal_matrix * triangle.n[k]).to_array(),
                uv: [0.0, 0.0],
                color,
                layer: NO_LAYER,
            });
        }
    }
}

/// `get_texture_path` gives texture `id`'s 2k diffuse image: City Sphere's own (`make
/// download-assets`, in `assets/textures/`) or else the fps demo's.
fn get_texture_path(id: &str) -> std::path::PathBuf {
    let own: std::path::PathBuf = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("assets/textures/{id}/{id}_diff_2k.jpg"));
    if own.exists() { own } else { get_asset_path(&format!("textures/{id}/{id}_diff_2k.jpg")) }
}

/// `push_beacon` adds a pylon's beacon (the hyperbolic demo's `scifi_beacon` model, as on Lunar
/// Surface) to the still scene with its base colour and emissive textures, recoloured in
/// `team_color`; the shader reads its console's state from the uniforms' `beacons[slot]` (strip
/// glow, body brightness), so it pulses while a player turns the console off and darkens once off.
///
/// Example: `push_beacon(&mut opaque, &beacon, world_from_model, [0.3, 0.55, 1.0], 0);`
pub fn push_beacon(out: &mut Vec<CityVertex>, model: &MeshModel, world_from_model: Mat4, team_color: [f32; 3], slot: usize) {
    assert!(slot < BEACON_SLOT_COUNT, "beacon slot {slot} out of range");
    let normal_matrix: glam::Mat3 = glam::Mat3::from_mat4(world_from_model);
    let color: [f32; 4] = [team_color[0], team_color[1], team_color[2], BEACON_FLAG + slot as f32];
    for triangle in &model.triangles {
        for k in 0..3 {
            out.push(CityVertex {
                position: world_from_model.transform_point3(triangle.p[k]).to_array(),
                normal: (normal_matrix * triangle.n[k]).normalize_or_zero().to_array(),
                uv: triangle.uv[k].to_array(),
                color,
                layer: BEACON_LAYER as f32,
            });
        }
    }
}

/// `push_pylon_beacons` adds a beacon (`push_beacon`) on every pylon box, standing on its level
/// and as tall as the box, in the slot of its team's nearest console.
fn push_pylon_beacons(out: &mut Vec<CityVertex>, world: &CityWorld) {
    let beacon: &MeshModel = get_beacon();
    for b in world.boxes.iter().filter(|b| b.group == "pylons") {
        let team: u32 = u32::from(b.name.starts_with("red"));
        let Some(console) = world
            .consoles
            .iter()
            .filter(|c| c.team == team)
            .min_by(|x, y| (world.get_console_center(x) - b.center).length().total_cmp(&(world.get_console_center(y) - b.center).length()))
        else {
            continue;
        };
        let up: DVec3 = get_up(b.center);
        let height_m: f64 = 2.0 * b.half_m.y;
        let back: DVec3 = (b.axes.z_axis - up * b.axes.z_axis.dot(up)).normalize_or(up.any_orthonormal_vector());
        let right: DVec3 = up.cross(back);
        let world_from_model: Mat4 = glam::DMat4::from_cols(
            (right * height_m).extend(0.0),
            (up * height_m).extend(0.0),
            (back * height_m).extend(0.0),
            (b.center - up * b.half_m.y).extend(1.0),
        )
        .as_mat4();
        push_beacon(out, beacon, world_from_model, BEACON_TEAM_COLORS[team as usize % 2], usize::from(console.index));
    }
}

/// `get_beacon` gives the pylons' model, the hyperbolic demo's `scifi_beacon` (as on Lunar
/// Surface), 1 m tall, loaded once.
///
/// Example: `let beacon = get_beacon();`
pub fn get_beacon() -> &'static MeshModel {
    static BEACON: std::sync::OnceLock<MeshModel> = std::sync::OnceLock::new();
    BEACON.get_or_init(|| MeshModel::Load(BEACON_MODEL, 1.0).expect("the scifi_beacon model in common/maps/models/"))
}

/// `push_beam` adds a laser beam from `a` to `b`, `width_m` wide, facing `camera_m`, with colour
/// and brightness (alpha).
///
/// Example: `push_beam(&mut beams, muzzle, hit, eye, 0.05, [1.0, 0.3, 0.2, 1.0]);`
pub fn push_beam(out: &mut Vec<CityVertex>, a: DVec3, b: DVec3, camera_m: DVec3, width_m: f64, color: [f32; 4]) {
    let along: DVec3 = b - a;
    let side: DVec3 = along.cross(camera_m - a).normalize_or_zero() * (width_m / 2.0);
    if side == DVec3::ZERO {
        return;
    }
    push_quad(out, [a - side, a + side, b + side, b - side], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], DVec3::ZERO, color, NO_LAYER);
}

/// `CityGpu` holds City Sphere's Metal state: pipelines, the textures and the scene's buffers.
pub struct CityGpu {
    opaque: Pipeline,
    glass: Pipeline,
    beam: Pipeline,
    depth_write: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_test: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    albedo: Texture,
    scene_id: u64,
    scene_buffers: Option<(Buffer, usize, Buffer, usize)>,
    depth: Option<(Texture, (usize, usize))>,
}

/// `make_buffer` copies `items` into a shared buffer.
fn make_buffer<T: Copy>(device: &ProtocolObject<dyn MTLDevice>, items: &[T]) -> Result<Buffer> {
    let dummy: [u8; 16] = [0; 16];
    let (pointer, length) = if items.is_empty() {
        (dummy.as_ptr().cast::<std::ffi::c_void>(), dummy.len())
    } else {
        (items.as_ptr().cast::<std::ffi::c_void>(), std::mem::size_of_val(items))
    };
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE ITEMS WILL HAPPEN
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            NonNull::new(pointer.cast_mut()).expect("a slice isn't null"),
            length,
            MTLResourceOptions::StorageModeShared,
        )
    };
    Ok(buffer.ok_or("Metal: no memory for City Sphere's vertices")?)
}

/// `make_pipeline` builds one of the pipelines: `blend` is None (opaque), Some(false) (premultiplied
/// alpha) or Some(true) (additive).
fn make_pipeline(device: &ProtocolObject<dyn MTLDevice>, library: &ProtocolObject<dyn MTLLibrary>, fragment: &str, blend: Option<bool>) -> Result<Pipeline> {
    let function = |name: &str| library.newFunctionWithName(&NSString::from_str(name)).ok_or(format!("Metal: no {name}"));
    let descriptor: Retained<MTLRenderPipelineDescriptor> = MTLRenderPipelineDescriptor::new();
    descriptor.setVertexFunction(Some(&*function("city_vertex")?));
    descriptor.setFragmentFunction(Some(&*function(fragment)?));
    let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    if let Some(is_additive) = blend {
        attachment.setBlendingEnabled(true);
        attachment.setSourceRGBBlendFactor(MTLBlendFactor::One);
        attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        let destination: MTLBlendFactor = if is_additive { MTLBlendFactor::One } else { MTLBlendFactor::OneMinusSourceAlpha };
        attachment.setDestinationRGBBlendFactor(destination);
        attachment.setDestinationAlphaBlendFactor(destination);
    }
    descriptor.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
    Ok(device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .map_err(|e| format!("Metal: the City Sphere pipeline failed: {}", e.localizedDescription()))?)
}

/// `make_depth_state` tests depth (less) and writes it if `is_write`.
fn make_depth_state(device: &ProtocolObject<dyn MTLDevice>, is_write: bool) -> Result<Retained<ProtocolObject<dyn MTLDepthStencilState>>> {
    let descriptor: Retained<MTLDepthStencilDescriptor> = MTLDepthStencilDescriptor::new();
    descriptor.setDepthCompareFunction(MTLCompareFunction::Less);
    descriptor.setDepthWriteEnabled(is_write);
    Ok(device.newDepthStencilStateWithDescriptor(&descriptor).ok_or("Metal: no depth state")?)
}

impl CityGpu {
    /// `New` compiles the shaders and loads the textures.
    ///
    /// Example: `let gpu = CityGpu::New(&device)?;`
    #[allow(non_snake_case)]
    pub fn New(device: &ProtocolObject<dyn MTLDevice>) -> Result<CityGpu> {
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(SHADER_SOURCE), None)
            .map_err(|e| format!("Metal: render_city.metal failed: {}", e.localizedDescription()))?;
        let mut layers: Vec<Vec<image::RgbaImage>> = TEXTURE_IDS
            .par_iter()
            .map(|id| make_mips(&get_texture_path(id), TEXTURE_SIDE_PX))
            .collect::<std::result::Result<_, String>>()?;
        let beacon_material: &MeshMaterial = get_beacon().materials.first().ok_or("the beacon has no material")?;
        for image in [&beacon_material.images[0], &beacon_material.images[3]] {
            layers.push(make_mips(image.as_ref().ok_or("the beacon lacks its base colour or emissive image")?, TEXTURE_SIDE_PX)?);
        }
        Ok(CityGpu {
            opaque: make_pipeline(device, &library, "city_fragment", None)?,
            glass: make_pipeline(device, &library, "glass_fragment", Some(false))?,
            beam: make_pipeline(device, &library, "beam_fragment", Some(true))?,
            depth_write: make_depth_state(device, true)?,
            depth_test: make_depth_state(device, false)?,
            albedo: make_array(device, MTLPixelFormat::RGBA8Unorm_sRGB, 4, &layers)?,
            scene_id: 0,
            scene_buffers: None,
            depth: None,
        })
    }

    /// `encode` draws `frame` into the top-left `frame.render_px` of `target`.
    ///
    /// Example: `gpu.encode(&device, &command_buffer, &frame, &render_texture)?;`
    pub fn encode(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        frame: &CityFrame,
        target: &ProtocolObject<dyn MTLTexture>,
    ) -> Result<()> {
        if self.scene_id != frame.scene.id || self.scene_buffers.is_none() {
            self.scene_buffers = Some((
                make_buffer(device, &frame.scene.opaque)?,
                frame.scene.opaque.len(),
                make_buffer(device, &frame.scene.glass)?,
                frame.scene.glass.len(),
            ));
            self.scene_id = frame.scene.id;
        }
        let target_px: (usize, usize) = (target.width(), target.height());
        if self.depth.as_ref().is_none_or(|(_, size)| *size != target_px) {
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A DEPTH TEXTURE OF THE WRONG SIZE WILL HAPPEN
            let descriptor: Retained<MTLTextureDescriptor> = unsafe {
                MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                    MTLPixelFormat::Depth32Float,
                    target_px.0,
                    target_px.1,
                    false,
                )
            };
            descriptor.setUsage(MTLTextureUsage::RenderTarget);
            descriptor.setStorageMode(MTLStorageMode::Private);
            let texture = device.newTextureWithDescriptor(&descriptor).ok_or("Metal: no memory for the depth")?;
            self.depth = Some((texture, target_px));
        }
        let mut holes: [[f32; 4]; MAX_HOLE_COUNT] = [[0.0; 4]; MAX_HOLE_COUNT];
        for (slot, hole) in holes.iter_mut().zip(&frame.scene.holes) {
            *slot = *hole;
        }
        let uniforms = CityUniforms {
            clip_from_world: frame.get_clip_from_world().to_cols_array_2d(),
            camera_m: frame.camera_m.extend(1.0).to_array(),
            core_light: CORE_LIGHT,
            ambient: AMBIENT,
            holes,
            glass: [frame.scene.glass_radius_m, frame.scene.holes.len() as f32, 0.0, FOG_M],
            sky_color: SKY_COLOR,
            beacons: frame.beacons,
        };
        let dynamic: Buffer = make_buffer(device, &frame.dynamic)?;
        let beams: Buffer = make_buffer(device, &frame.beams)?;
        let pass: Retained<MTLRenderPassDescriptor> = MTLRenderPassDescriptor::new();
        let color = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        color.setTexture(Some(target));
        color.setLoadAction(MTLLoadAction::Clear);
        color.setStoreAction(MTLStoreAction::Store);
        color.setClearColor(MTLClearColor {
            red: f64::from(SKY_COLOR[0]),
            green: f64::from(SKY_COLOR[1]),
            blue: f64::from(SKY_COLOR[2]),
            alpha: 1.0,
        });
        let depth = pass.depthAttachment();
        depth.setTexture(Some(&self.depth.as_ref().expect("made above").0));
        depth.setLoadAction(MTLLoadAction::Clear);
        depth.setStoreAction(MTLStoreAction::DontCare);
        depth.setClearDepth(1.0);
        let encoder = command_buffer.renderCommandEncoderWithDescriptor(&pass).ok_or("Metal: no render encoder")?;
        encoder.setCullMode(MTLCullMode::None);
        encoder.setViewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: frame.render_px.0 as f64,
            height: frame.render_px.1 as f64,
            znear: 0.0,
            zfar: 1.0,
        });
        encoder.setScissorRect(MTLScissorRect {
            x: 0,
            y: 0,
            width: frame.render_px.0.min(target_px.0),
            height: frame.render_px.1.min(target_px.1),
        });
        let (opaque, opaque_count, glass, glass_count) = self.scene_buffers.as_ref().expect("made above");
        let draws: [(&Pipeline, bool, &Buffer, usize); 4] = [
            (&self.opaque, true, opaque, *opaque_count),
            (&self.opaque, true, &dynamic, frame.dynamic.len()),
            (&self.glass, false, glass, *glass_count),
            (&self.beam, false, &beams, frame.beams.len()),
        ];
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS OR DRAWING PAST THEM WILL HAPPEN
        unsafe {
            encoder.setVertexBytes_length_atIndex(NonNull::from(&uniforms).cast(), std::mem::size_of::<CityUniforms>(), 1);
            encoder.setFragmentBytes_length_atIndex(NonNull::from(&uniforms).cast(), std::mem::size_of::<CityUniforms>(), 1);
            encoder.setFragmentTexture_atIndex(Some(&self.albedo), 0);
            for (pipeline, is_depth_write, buffer, count) in draws {
                if count == 0 {
                    continue;
                }
                encoder.setRenderPipelineState(pipeline);
                encoder.setDepthStencilState(Some(if is_depth_write { &self.depth_write } else { &self.depth_test }));
                encoder.setVertexBuffer_offset_atIndex(Some(buffer), 0, 0);
                encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, count);
            }
        }
        encoder.endEncoding();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! | Behaviour | Test |
    //! | --- | --- |
    //! | The vertex layout matches the shader's (13 floats) | `vertex_is_13_floats` |
    //! | The scene has the street, boxes, core and glass, and the lift holes | `scene_has_every_part` |

    use super::*;

    #[test]
    fn vertex_is_13_floats() {
        assert_eq!(std::mem::size_of::<CityVertex>(), 13 * 4);
    }

    #[test]
    fn scene_has_every_part() {
        let world: CityWorld = CityWorld::Builtin().unwrap();
        let scene: CityScene = build_scene(&world);
        assert!(scene.opaque.len() > SPHERE_LON_COUNT * SPHERE_LAT_COUNT * 6);
        assert_eq!(scene.glass.len(), SPHERE_LON_COUNT * SPHERE_LAT_COUNT * 6);
        assert_eq!(scene.holes.len(), 6);
    }
}
