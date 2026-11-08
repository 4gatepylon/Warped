//! `viewmodel.rs` draws a gun model from the fps_demo's assets in the player's hand where the wand
//! is, rasterised with 4x MSAA into a texture of its own that `apply_overlay` lays over the frame:
//! the energy revolver (the Sketchfab "Sci-fi revolver" glTF's railgun variant) or Poly Haven's
//! bolt-action rifle, with their PBR textures.
//!
//! Interface: `GunModel`, `check_gun_models`, `Viewmodel::Load`, `Viewmodel::encode`.
//!
//! Example: `let rifle = Viewmodel::Load(&device, GunModel::BoltActionRifle)?;`

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use glam::{Mat3, Mat4, Vec3};
use image::RgbaImage;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLBuffer, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder,
    MTLCompareFunction, MTLCullMode, MTLDepthStencilDescriptor, MTLDepthStencilState, MTLDevice,
    MTLIndexType, MTLLibrary, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType,
    MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineDescriptor,
    MTLRenderPipelineState, MTLResourceOptions, MTLStorageMode, MTLStoreAction, MTLTexture,
    MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};
use rayon::prelude::*;

use crate::fps_demo_assets::{check_asset, get_asset_path, import_gltf};
use crate::render_metal::OverlayWand;
use crate::voxel_textures::{make_array, make_mips};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Texture = Retained<ProtocolObject<dyn MTLTexture>>;
type Pipeline = Retained<ProtocolObject<dyn MTLRenderPipelineState>>;

const SHADER_SOURCE: &str = include_str!("viewmodel.metal");
const REVOLVER_GLTF: &str = "sketchfab/sci_fi_revolver/scene.gltf";
const BOLT_ACTION_RIFLE_GLTF: &str = "models/bolt_action_rifle_7_62/bolt_action_rifle_7_62_2k.gltf";
/// `RAILGUN_NODE_PARTS` are name parts of the revolver's railgun variant; the file also holds a
/// ballistic one beside it.
const RAILGUN_NODE_PARTS: [&str; 8] = [
    "Drum",
    "BulletsC",
    "Extractor",
    "Receiver_1",
    "RailGun",
    "Glass",
    "Collimator",
    "VATS",
];
const TEXTURE_SIDE_PX: u32 = 1024;
const SAMPLE_COUNT: usize = 4;
const NEAR_M: f32 = 0.01;
const FAR_M: f32 = 4.0;
/// `MUZZLE_SLICE_M` is how far behind a gun model's front its vertices count as the muzzle's face.
const MUZZLE_SLICE_M: f32 = 0.01;
/// `REVOLVER_SCALE` is how many times its modelled size the revolver is drawn.
const REVOLVER_SCALE: f32 = 1.6;
/// `REVOLVER_MODELLED_LENGTH_M` is the revolver's length in its glTF (the railgun's parts), from
/// its grip's back to its muzzle: about a real revolver's.
const REVOLVER_MODELLED_LENGTH_M: f32 = 0.302;
/// `BOLT_ACTION_RIFLE_MODELLED_LENGTH_M` is the bolt-action rifle's length in its glTF, from its
/// stock's end to its muzzle: a real rifle's.
pub const BOLT_ACTION_RIFLE_MODELLED_LENGTH_M: f32 = 1.231;
/// `BOLT_ACTION_RIFLE_SCALE` is how many times its modelled size (1.23 m long) the rifle is drawn:
/// small enough that its scope, bolt and stock come into the corner of the view with its muzzle at
/// the wand's tip, where a full-size rifle shows only its barrel.
const BOLT_ACTION_RIFLE_SCALE: f32 = 0.28;
/// `BOLT_ACTION_RIFLE_TURN_RAD` turns the rifle about its up axis so that its barrel, which lies
/// along +x in its glTF, points along +z like the revolver's.
const BOLT_ACTION_RIFLE_TURN_RAD: f32 = -std::f32::consts::FRAC_PI_2;
/// `TRANSMISSION_OPACITY` is how opaque the transmissive glass is drawn.
const TRANSMISSION_OPACITY: f32 = 0.18;
/// `SLOT_FORMATS` are the formats of a material's base colour, metallic-roughness, normal and
/// emissive maps, and `SLOT_FALLBACKS` the pixel used when one is missing.
const SLOT_FORMATS: [MTLPixelFormat; 4] = [
    MTLPixelFormat::RGBA8Unorm_sRGB,
    MTLPixelFormat::RGBA8Unorm,
    MTLPixelFormat::RGBA8Unorm,
    MTLPixelFormat::RGBA8Unorm_sRGB,
];
const HAND_LIGHT_COUNT: usize = 3;
const HAND_LIGHT_BUFFER_INDEX: usize = 1;
const SLOT_FALLBACKS: [[u8; 4]; 4] = [[255; 4], [255; 4], [128, 128, 255, 255], [255; 4]];

/// `GunModel` is a model drawn in the player's hand: the bolt-action rifle (the sniper, gun 1) or
/// the energy revolver (the rifle, gun 2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GunModel {
    Revolver,
    BoltActionRifle,
}

/// `ModelSpec` is where a gun model's glTF is in the fps_demo's `assets/`, which of its nodes are
/// drawn (name parts, or `None` for all), how far it turns about its up axis so its barrel points
/// along +z, and how many times its modelled size it is drawn.
struct ModelSpec {
    gltf_path: &'static str,
    node_parts: Option<&'static [&'static str]>,
    turn_rad: f32,
    scale: f32,
}

impl GunModel {
    /// `ALL` is every gun model, for loading them all before the first frame.
    pub const ALL: [GunModel; 2] = [GunModel::Revolver, GunModel::BoltActionRifle];

    /// `get_label` names the model, for the loading screen and the console.
    ///
    /// Example: `let label: &str = GunModel::BoltActionRifle.get_label();`
    pub fn get_label(self) -> &'static str {
        match self {
            GunModel::Revolver => "revolver",
            GunModel::BoltActionRifle => "rifle",
        }
    }

    /// `get_modelled_length_m` gives how long the model is in its glTF, its real size, before it is
    /// drawn smaller or larger in hand.
    ///
    /// Example: `let length_m: f32 = GunModel::Revolver.get_modelled_length_m();`
    pub fn get_modelled_length_m(self) -> f32 {
        match self {
            GunModel::Revolver => REVOLVER_MODELLED_LENGTH_M,
            GunModel::BoltActionRifle => BOLT_ACTION_RIFLE_MODELLED_LENGTH_M,
        }
    }

    /// `get_spec` gives where the model's glTF is and how it is drawn.
    ///
    /// Example: `let spec: ModelSpec = GunModel::Revolver.get_spec();`
    fn get_spec(self) -> ModelSpec {
        match self {
            GunModel::Revolver => ModelSpec {
                gltf_path: REVOLVER_GLTF,
                node_parts: Some(&RAILGUN_NODE_PARTS),
                turn_rad: 0.0,
                scale: REVOLVER_SCALE,
            },
            GunModel::BoltActionRifle => ModelSpec {
                gltf_path: BOLT_ACTION_RIFLE_GLTF,
                node_parts: None,
                turn_rad: BOLT_ACTION_RIFLE_TURN_RAD,
                scale: BOLT_ACTION_RIFLE_SCALE,
            },
        }
    }
}

/// `Vertex` is the shader's `Vertex`: a glTF vertex with its node's transform applied.
#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 4],
    normal: [f32; 4],
    tangent: [f32; 4],
    uv: [f32; 4],
}

/// `DrawUniforms` is the shader's `DrawUniforms`.
#[repr(C)]
#[derive(Clone, Copy)]
struct DrawUniforms {
    clip_from_model: [[f32; 4]; 4],
    camera_from_model: [[f32; 4]; 4],
}

/// `MaterialUniforms` is the shader's `MaterialUniforms`: colour and emission factors, and
/// metallic, roughness, opacity and whether it blends.
#[repr(C)]
#[derive(Clone, Copy)]
struct MaterialUniforms {
    base_color: [f32; 4],
    emissive: [f32; 4],
    params: [f32; 4],
}

/// `HandLight` is the shader's `HandLight` (and render_voxel.metal's): the light on the gun model
/// in camera space, as directions toward three lights with their radiance, the local up, and the
/// radiance of the sky and of the ground.
#[repr(C)]
#[derive(Clone, Copy)]
struct HandLight {
    directions: [[f32; 4]; HAND_LIGHT_COUNT],
    colors: [[f32; 4]; HAND_LIGHT_COUNT],
    up: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
}

/// `HAND_LIGHT_BYTES` is the size of a `HandLight`, for the buffer `probe_hand_light` writes.
pub(crate) const HAND_LIGHT_BYTES: usize = std::mem::size_of::<HandLight>();

/// `FIXED_HAND_LIGHT` lights the gun model on maps whose renderer doesn't probe their light (the
/// house): a key light from the upper left, lit regolith below and black space above.
const FIXED_HAND_LIGHT: HandLight = HandLight {
    directions: [[-0.4208, 0.7814, 0.4608, 0.0], [0.0; 4], [0.0; 4]],
    colors: [[2.4, 2.35, 2.25, 0.0], [0.0; 4], [0.0; 4]],
    up: [0.0, 1.0, 0.0, 0.0],
    sky: [0.03, 0.035, 0.05, 0.0],
    ground: [0.32, 0.31, 0.3, 0.0],
};

/// `Material` is a glTF material's uniforms and its four maps.
struct Material {
    uniforms: MaterialUniforms,
    maps: [Texture; 4],
    is_blended: bool,
}

/// `Part` is a run of the index buffer drawn with one material.
struct Part {
    first_index: usize,
    index_count: usize,
    material: usize,
}

/// `Targets` are the multisampled colour and depth and the resolved image, for one size.
struct Targets {
    size: (usize, usize),
    color: Texture,
    depth: Texture,
    resolved: Texture,
}

/// `Viewmodel` is a gun model's mesh and materials on the GPU, the transform that puts its muzzle
/// at the origin pointing along -z, its pipelines and targets.
pub struct Viewmodel {
    vertices: Retained<ProtocolObject<dyn MTLBuffer>>,
    indices: Retained<ProtocolObject<dyn MTLBuffer>>,
    parts: Vec<Part>,
    materials: Vec<Material>,
    muzzle_from_gltf: Mat4,
    opaque: Pipeline,
    blended: Pipeline,
    depth_write: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    depth_test: Retained<ProtocolObject<dyn MTLDepthStencilState>>,
    targets: Option<Targets>,
}

/// `set_globals` fills each node's model-space transform below `node`.
fn set_globals(node: gltf::Node, parent: Mat4, globals: &mut [Mat4]) {
    let global: Mat4 = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    globals[node.index()] = global;
    for child in node.children() {
        set_globals(child, global, globals);
    }
}

/// `make_buffer` copies `items` into a new shared buffer.
fn make_buffer<T>(
    device: &ProtocolObject<dyn MTLDevice>,
    items: &[T],
) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>> {
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE ITEMS WILL HAPPEN
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            NonNull::new(items.as_ptr().cast_mut().cast()).expect("a slice isn't null"),
            std::mem::size_of_val(items).max(1),
            MTLResourceOptions::StorageModeShared,
        )
    };
    Ok(buffer.ok_or("Metal: no memory for the gun model")?)
}

/// `make_target` makes a private render target, multisampled when `sample_count` is above 1.
fn make_target(
    device: &ProtocolObject<dyn MTLDevice>,
    format: MTLPixelFormat,
    size: (usize, usize),
    sample_count: usize,
) -> Result<Texture> {
    let descriptor: Retained<MTLTextureDescriptor> = MTLTextureDescriptor::new();
    descriptor.setTextureType(if sample_count > 1 {
        MTLTextureType::Type2DMultisample
    } else {
        MTLTextureType::Type2D
    });
    descriptor.setPixelFormat(format);
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TARGET OF THE WRONG SIZE WILL HAPPEN
    unsafe {
        descriptor.setWidth(size.0.max(1));
        descriptor.setHeight(size.1.max(1));
        descriptor.setSampleCount(sample_count);
    }
    descriptor.setUsage(if sample_count > 1 {
        MTLTextureUsage::RenderTarget
    } else {
        MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead
    });
    descriptor.setStorageMode(MTLStorageMode::Private);
    Ok(device
        .newTextureWithDescriptor(&descriptor)
        .ok_or("Metal: no memory for the gun model's targets")?)
}

/// `make_pipeline` builds the gun models' pipeline, blending premultiplied colour if `is_blended`.
fn make_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    is_blended: bool,
) -> Result<Pipeline> {
    let function = |name: &str| {
        library
            .newFunctionWithName(&NSString::from_str(name))
            .ok_or(format!("Metal: no {name}"))
    };
    let descriptor: Retained<MTLRenderPipelineDescriptor> = MTLRenderPipelineDescriptor::new();
    descriptor.setVertexFunction(Some(&*function("viewmodel_vertex")?));
    descriptor.setFragmentFunction(Some(&*function("viewmodel_fragment")?));
    descriptor.setRasterSampleCount(SAMPLE_COUNT);
    let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
    attachment.setPixelFormat(MTLPixelFormat::RGBA8Unorm);
    if is_blended {
        attachment.setBlendingEnabled(true);
        attachment.setSourceRGBBlendFactor(MTLBlendFactor::One);
        attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
    }
    descriptor.setDepthAttachmentPixelFormat(MTLPixelFormat::Depth32Float);
    Ok(device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .map_err(|e| {
            format!(
                "Metal: the gun model pipeline failed: {}",
                e.localizedDescription()
            )
        })?)
}

/// `make_depth_state` tests depth (less) and writes it if `is_write`.
fn make_depth_state(
    device: &ProtocolObject<dyn MTLDevice>,
    is_write: bool,
) -> Result<Retained<ProtocolObject<dyn MTLDepthStencilState>>> {
    let descriptor: Retained<MTLDepthStencilDescriptor> = MTLDepthStencilDescriptor::new();
    descriptor.setDepthCompareFunction(MTLCompareFunction::Less);
    descriptor.setDepthWriteEnabled(is_write);
    Ok(device
        .newDepthStencilStateWithDescriptor(&descriptor)
        .ok_or("Metal: no depth state")?)
}

/// `get_clip_from_camera` is a right-handed perspective onto Metal's 0..1 depth, for a vertical
/// half-angle tangent `tan_half_fov` and width over height `aspect`.
fn get_clip_from_camera(tan_half_fov: f32, aspect: f32) -> Mat4 {
    let depth: f32 = FAR_M / (NEAR_M - FAR_M);
    Mat4::from_cols_array_2d(&[
        [1.0 / (tan_half_fov * aspect), 0.0, 0.0, 0.0],
        [0.0, 1.0 / tan_half_fov, 0.0, 0.0],
        [0.0, 0.0, depth, -1.0],
        [0.0, 0.0, depth * NEAR_M, 0.0],
    ])
}

/// `get_map_images` gives a material's base colour, metallic-roughness, normal and emissive images.
fn get_map_images(material: &gltf::Material) -> [Option<usize>; 4] {
    let pbr = material.pbr_metallic_roughness();
    [
        pbr.base_color_texture()
            .map(|t| t.texture().source().index()),
        pbr.metallic_roughness_texture()
            .map(|t| t.texture().source().index()),
        material
            .normal_texture()
            .map(|t| t.texture().source().index()),
        material
            .emissive_texture()
            .map(|t| t.texture().source().index()),
    ]
}

/// `check_gun_models` fails like `check_asset` when a gun model's glTF is missing.
///
/// Example: `check_gun_models()?;`
pub fn check_gun_models() -> Result<()> {
    GunModel::ALL
        .iter()
        .try_for_each(|model| check_asset(&get_asset_path(model.get_spec().gltf_path)))
}

/// `Mesh` is a gun model's drawn triangles, with its nodes' transforms and its turn applied, and
/// the material of each run of indices.
struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    parts: Vec<Part>,
}

/// `GltfFile` is a glTF document read from the fps_demo's `assets/`, its buffers and its folder.
struct GltfFile {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    folder: PathBuf,
}

/// `open_gltf` reads `spec`'s glTF and its buffers, and fails like `check_asset` when it is missing.
///
/// Example: `let file: GltfFile = open_gltf(&GunModel::Revolver.get_spec())?;`
fn open_gltf(spec: &ModelSpec) -> Result<GltfFile> {
    let path: PathBuf = get_asset_path(spec.gltf_path);
    check_asset(&path)?;
    let (document, buffers) = import_gltf(&path)?;
    let folder: PathBuf = path.parent().expect("a file has a folder").to_path_buf();
    Ok(GltfFile {
        document,
        buffers,
        folder,
    })
}

/// `read_mesh` gathers the triangles of `file`'s nodes that `spec` draws, in model space turned by
/// `spec.turn_rad` about the up axis.
///
/// Example: `let mesh: Mesh = read_mesh(&file, &spec);`
fn read_mesh(file: &GltfFile, spec: &ModelSpec) -> Mesh {
    let document: &gltf::Document = &file.document;
    let mut globals: Vec<Mat4> = vec![Mat4::IDENTITY; document.nodes().len()];
    for scene in document.scenes() {
        for node in scene.nodes() {
            set_globals(node, Mat4::from_rotation_y(spec.turn_rad), &mut globals);
        }
    }
    let (mut vertices, mut indices, mut parts) =
        (Vec::<Vertex>::new(), Vec::<u32>::new(), Vec::<Part>::new());
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        let name: &str = node.name().unwrap_or_default();
        if let Some(node_parts) = spec.node_parts
            && !node_parts.iter().any(|part| name.contains(part))
        {
            continue;
        }
        let global: Mat4 = globals[node.index()];
        let (turn, normal_turn) = (
            Mat3::from_mat4(global),
            Mat3::from_mat4(global).inverse().transpose(),
        );
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&file.buffers[buffer.index()]));
            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_default();
            let tangents: Vec<[f32; 4]> = reader
                .read_tangents()
                .map(|t| t.collect())
                .unwrap_or_default();
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .unwrap_or_default();
            let first_vertex: u32 = vertices.len() as u32;
            for (i, position) in positions.enumerate() {
                let position: Vec3 = global.transform_point3(Vec3::from(position));
                let normal: Vec3 =
                    normal_turn * Vec3::from(normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]));
                let tangent: [f32; 4] = tangents.get(i).copied().unwrap_or([0.0; 4]);
                let uv: [f32; 2] = uvs.get(i).copied().unwrap_or([0.0; 2]);
                vertices.push(Vertex {
                    position: position.extend(1.0).to_array(),
                    normal: normal.normalize_or_zero().extend(0.0).to_array(),
                    tangent: (turn * Vec3::new(tangent[0], tangent[1], tangent[2]))
                        .normalize_or_zero()
                        .extend(tangent[3])
                        .to_array(),
                    uv: [uv[0], uv[1], 0.0, 0.0],
                });
            }
            let first_index: usize = indices.len();
            match reader.read_indices() {
                Some(read) => indices.extend(read.into_u32().map(|i| i + first_vertex)),
                None => indices.extend(first_vertex..vertices.len() as u32),
            }
            let material: usize = primitive.material().index().unwrap_or(0);
            parts.push(Part {
                first_index,
                index_count: indices.len() - first_index,
                material,
            });
        }
    }
    Mesh {
        vertices,
        indices,
        parts,
    }
}

/// `get_muzzle_m` gives a gun model's muzzle in model space: the middle of its front face (the
/// vertices within `MUZZLE_SLICE_M` of its frontmost, the largest z), on that front.
///
/// Example: `let muzzle_m: Vec3 = get_muzzle_m(&mesh.vertices);`
fn get_muzzle_m(vertices: &[Vertex]) -> Vec3 {
    let positions = || {
        vertices
            .iter()
            .map(|v| Vec3::new(v.position[0], v.position[1], v.position[2]))
    };
    let front_z_m: f32 = positions().map(|p| p.z).fold(f32::MIN, f32::max);
    let (face_min_m, face_max_m) = positions()
        .filter(|p| p.z > front_z_m - MUZZLE_SLICE_M)
        .fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(p), hi.max(p)),
        );
    ((face_min_m + face_max_m) * 0.5).with_z(front_z_m)
}

/// `get_muzzle_from_model` takes a gun model's space to its muzzle's: `muzzle_m` to the origin,
/// the barrel along -z (the camera's ahead), drawn `scale` times its modelled size.
///
/// Example: `let muzzle_from_model: Mat4 = get_muzzle_from_model(muzzle_m, 1.6);`
fn get_muzzle_from_model(muzzle_m: Vec3, scale: f32) -> Mat4 {
    Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-muzzle_m)
}

impl Viewmodel {
    /// `Load` reads `model` from the fps_demo's `assets/` onto `device`, and fails like
    /// `check_asset` when it is missing.
    ///
    /// Example: `let rifle = Viewmodel::Load(&device, GunModel::BoltActionRifle)?;`
    #[allow(non_snake_case)]
    pub fn Load(device: &ProtocolObject<dyn MTLDevice>, model: GunModel) -> Result<Viewmodel> {
        let spec: ModelSpec = model.get_spec();
        let file: GltfFile = open_gltf(&spec)?;
        let Mesh {
            vertices,
            indices,
            parts,
        } = read_mesh(&file, &spec);
        let (document, base_dir): (&gltf::Document, &Path) = (&file.document, &file.folder);
        let image_paths: Vec<Option<PathBuf>> = document
            .images()
            .map(|image| match image.source() {
                gltf::image::Source::Uri { uri, .. } => Some(base_dir.join(uri)),
                gltf::image::Source::View { .. } => None,
            })
            .collect();
        let mut jobs: Vec<(usize, usize)> = document
            .materials()
            .filter(|m| parts.iter().any(|p| Some(p.material) == m.index()))
            .flat_map(|m| {
                get_map_images(&m)
                    .into_iter()
                    .enumerate()
                    .filter_map(|(slot, image)| image.map(|image| (image, slot)))
                    .collect::<Vec<_>>()
            })
            .collect();
        jobs.sort_unstable();
        jobs.dedup();
        let decoded: Vec<((usize, usize), Vec<RgbaImage>)> = jobs
            .par_iter()
            .map(|&(image, slot)| {
                let path: &PathBuf = image_paths[image]
                    .as_ref()
                    .ok_or_else(|| format!("an embedded {} image", model.get_label()))?;
                Ok(((image, slot), make_mips(path, TEXTURE_SIDE_PX)?))
            })
            .collect::<std::result::Result<_, String>>()?;
        let mut maps: HashMap<(usize, usize), Texture> = HashMap::new();
        for (key, mips) in &decoded {
            maps.insert(
                *key,
                make_array(device, SLOT_FORMATS[key.1], 4, std::slice::from_ref(mips))?,
            );
        }
        let fallbacks: Vec<Texture> = (0..4)
            .map(|slot| {
                make_array(
                    device,
                    SLOT_FORMATS[slot],
                    4,
                    &[vec![RgbaImage::from_pixel(
                        1,
                        1,
                        image::Rgba(SLOT_FALLBACKS[slot]),
                    )]],
                )
            })
            .collect::<Result<_>>()?;
        let materials: Vec<Material> = document
            .materials()
            .map(|m| {
                let pbr = m.pbr_metallic_roughness();
                let is_blended: bool = m.alpha_mode() == gltf::material::AlphaMode::Blend;
                let emissive: Vec3 =
                    Vec3::from(m.emissive_factor()) * m.emissive_strength().unwrap_or(1.0);
                let images: [Option<usize>; 4] = get_map_images(&m);
                Material {
                    uniforms: MaterialUniforms {
                        base_color: pbr.base_color_factor(),
                        emissive: emissive.extend(0.0).to_array(),
                        params: [
                            pbr.metallic_factor(),
                            pbr.roughness_factor(),
                            if m.transmission().is_some() {
                                TRANSMISSION_OPACITY
                            } else {
                                1.0
                            },
                            f32::from(u8::from(is_blended)),
                        ],
                    },
                    maps: std::array::from_fn(|slot| {
                        images[slot]
                            .and_then(|image| maps.get(&(image, slot)).cloned())
                            .unwrap_or_else(|| fallbacks[slot].clone())
                    }),
                    is_blended,
                }
            })
            .collect();
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(SHADER_SOURCE), None)
            .map_err(|e| {
                format!(
                    "Metal: the gun model shader didn't compile: {}",
                    e.localizedDescription()
                )
            })?;
        println!(
            "Loaded the {}: {} triangles, {} maps.",
            model.get_label(),
            indices.len() / 3,
            maps.len()
        );
        Ok(Viewmodel {
            vertices: make_buffer(device, &vertices)?,
            indices: make_buffer(device, &indices)?,
            parts,
            materials,
            muzzle_from_gltf: get_muzzle_from_model(get_muzzle_m(&vertices), spec.scale),
            opaque: make_pipeline(device, &library, false)?,
            blended: make_pipeline(device, &library, true)?,
            depth_write: make_depth_state(device, true)?,
            depth_test: make_depth_state(device, false)?,
            targets: None,
        })
    }

    /// `encode` draws the gun model posed like `wand`, its muzzle at the wand's tip, into a `size`
    /// image (premultiplied sRGB, clear where it isn't) for a camera of vertical half-angle tangent `tan_half_fov`, lit by
    /// `hand_light` (a GPU `HandLight` written earlier in `command_buffer`) or, if it is None, by
    /// `FIXED_HAND_LIGHT`, and returns it.
    ///
    /// Example: `let image = viewmodel.encode(&device, &command_buffer, &wand, (2560, 1600), 0.6, None)?;`
    pub fn encode(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        wand: &OverlayWand,
        size: (usize, usize),
        tan_half_fov: f32,
        hand_light: Option<&ProtocolObject<dyn MTLBuffer>>,
    ) -> Result<Texture> {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = Some(Targets {
                size,
                color: make_target(device, MTLPixelFormat::RGBA8Unorm, size, SAMPLE_COUNT)?,
                depth: make_target(device, MTLPixelFormat::Depth32Float, size, SAMPLE_COUNT)?,
                resolved: make_target(device, MTLPixelFormat::RGBA8Unorm, size, 1)?,
            });
        }
        let targets: &Targets = self.targets.as_ref().expect("the targets were just made");
        let axes: Mat3 = Mat3::from_cols_array_2d(&wand.axes.map(|a| [a[0], a[1], a[2]]));
        let camera_from_model: Mat4 =
            Mat4::from_mat3_translation(axes, wand.get_tip_m()) * self.muzzle_from_gltf;
        let aspect: f32 = size.0 as f32 / size.1.max(1) as f32;
        let draw = DrawUniforms {
            clip_from_model: (get_clip_from_camera(tan_half_fov, aspect) * camera_from_model)
                .to_cols_array_2d(),
            camera_from_model: camera_from_model.to_cols_array_2d(),
        };
        let pass: Retained<MTLRenderPassDescriptor> = MTLRenderPassDescriptor::new();
        let color = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        color.setTexture(Some(&targets.color));
        color.setResolveTexture(Some(&targets.resolved));
        color.setLoadAction(MTLLoadAction::Clear);
        color.setStoreAction(MTLStoreAction::MultisampleResolve);
        color.setClearColor(MTLClearColor {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 0.0,
        });
        let depth = pass.depthAttachment();
        depth.setTexture(Some(&targets.depth));
        depth.setLoadAction(MTLLoadAction::Clear);
        depth.setStoreAction(MTLStoreAction::DontCare);
        depth.setClearDepth(1.0);
        let encoder = command_buffer
            .renderCommandEncoderWithDescriptor(&pass)
            .ok_or("Metal: no render encoder")?;
        encoder.setCullMode(MTLCullMode::None);
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&self.vertices), 0, 0);
            encoder.setVertexBytes_length_atIndex(
                NonNull::from(&draw).cast(),
                std::mem::size_of::<DrawUniforms>(),
                1,
            );
            match hand_light {
                Some(buffer) => encoder.setFragmentBuffer_offset_atIndex(
                    Some(buffer),
                    0,
                    HAND_LIGHT_BUFFER_INDEX,
                ),
                None => encoder.setFragmentBytes_length_atIndex(
                    NonNull::from(&FIXED_HAND_LIGHT).cast(),
                    std::mem::size_of::<HandLight>(),
                    HAND_LIGHT_BUFFER_INDEX,
                ),
            }
        }
        for is_blended in [false, true] {
            encoder.setRenderPipelineState(if is_blended {
                &self.blended
            } else {
                &self.opaque
            });
            encoder.setDepthStencilState(Some(if is_blended {
                &self.depth_test
            } else {
                &self.depth_write
            }));
            for part in self
                .parts
                .iter()
                .filter(|p| self.materials[p.material].is_blended == is_blended)
            {
                let material: &Material = &self.materials[part.material];
                // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A DRAW PAST THE INDICES WILL HAPPEN
                unsafe {
                    encoder.setFragmentBytes_length_atIndex(
                        NonNull::from(&material.uniforms).cast(),
                        std::mem::size_of::<MaterialUniforms>(),
                        0,
                    );
                    for (index, map) in material.maps.iter().enumerate() {
                        encoder.setFragmentTexture_atIndex(Some(map), index);
                    }
                    encoder
                        .drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                            MTLPrimitiveType::Triangle,
                            part.index_count,
                            MTLIndexType::UInt32,
                            &self.indices,
                            part.first_index * std::mem::size_of::<u32>(),
                        );
                }
            }
        }
        encoder.endEncoding();
        Ok(targets.resolved.clone())
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the gun models held in hand: their files, which way they point, their size and
    //! where their muzzle sits.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Gun model | the revolver, the bolt-action rifle | `every_gun_model_is_found`, `the_muzzle_is_the_front_of_the_barrel`, `every_gun_model_points_ahead` |
    //! | Model file | present, missing (fails naming the restore command) | `every_gun_model_is_found`, `a_missing_model_fails_with_the_fix` |
    //! | Where a vertex is along the barrel | on the muzzle's face (z = 0), behind it (z > 0); none ahead of it | `the_muzzle_is_the_front_of_the_barrel` |
    //! | Turn about the up axis | none (the revolver), a quarter turn (the rifle, modelled along +x) | `every_gun_model_points_ahead`, `every_gun_model_is_drawn_at_its_scale` |
    //! | Modelled length | the revolver (0.30 m), the bolt-action rifle (1.23 m): each matches its mesh | `every_gun_model_is_drawn_at_its_scale` |

    use super::*;

    const LENGTH_TOLERANCE_M: f32 = 0.01;
    const POSITION_TOLERANCE_M: f32 = 1e-5;
    /// `MIN_LENGTH_PER_WIDTH` is how many times longer than wide a gun model is at least.
    const MIN_LENGTH_PER_WIDTH: f32 = 3.0;
    const RESTORE_COMMAND: &str = "checkout -- assets";

    /// `get_positions_from_muzzle_m` gives `model`'s drawn vertices in its muzzle's space (the
    /// muzzle at the origin, the barrel along -z), at its drawn size.
    fn get_positions_from_muzzle_m(model: GunModel) -> Vec<Vec3> {
        let spec: ModelSpec = model.get_spec();
        let mesh: Mesh = read_mesh(&open_gltf(&spec).expect("the model is committed"), &spec);
        let muzzle_from_model: Mat4 =
            get_muzzle_from_model(get_muzzle_m(&mesh.vertices), spec.scale);
        mesh.vertices
            .iter()
            .map(|v| {
                muzzle_from_model.transform_point3(Vec3::new(
                    v.position[0],
                    v.position[1],
                    v.position[2],
                ))
            })
            .collect()
    }

    /// `get_extent_m` gives the smallest and largest corners around `positions_m`.
    fn get_extent_m(positions_m: &[Vec3]) -> (Vec3, Vec3) {
        positions_m.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        )
    }

    #[test]
    fn every_gun_model_is_found() {
        check_gun_models().expect("every gun model is committed in the fps_demo's assets");
        for model in GunModel::ALL {
            assert!(
                !get_positions_from_muzzle_m(model).is_empty(),
                "{model:?} draws nothing"
            );
        }
    }

    #[test]
    fn a_missing_model_fails_with_the_fix() {
        let spec = ModelSpec {
            gltf_path: "models/no_such_gun/no_such_gun.gltf",
            node_parts: None,
            turn_rad: 0.0,
            scale: 1.0,
        };
        let error: String = open_gltf(&spec)
            .err()
            .expect("a missing model fails")
            .to_string();
        assert!(error.contains("no_such_gun.gltf"), "{error}");
        assert!(error.contains(RESTORE_COMMAND), "{error}");
    }

    #[test]
    fn the_muzzle_is_the_front_of_the_barrel() {
        for model in GunModel::ALL {
            let positions_m: Vec<Vec3> = get_positions_from_muzzle_m(model);
            assert!(
                positions_m.iter().all(|p| p.z >= -POSITION_TOLERANCE_M),
                "{model:?} has vertices ahead of its muzzle"
            );
            let face_m: Vec<Vec3> = positions_m
                .iter()
                .copied()
                .filter(|p| p.z < MUZZLE_SLICE_M * model.get_spec().scale)
                .collect();
            let (face_min_m, face_max_m) = get_extent_m(&face_m);
            assert!(
                face_min_m.x < 0.0
                    && face_max_m.x > 0.0
                    && face_min_m.y < 0.0
                    && face_max_m.y > 0.0,
                "{model:?}'s muzzle {face_min_m}..{face_max_m} doesn't surround the origin"
            );
        }
    }

    #[test]
    fn every_gun_model_points_ahead() {
        for model in GunModel::ALL {
            let (min_m, max_m) = get_extent_m(&get_positions_from_muzzle_m(model));
            let size_m: Vec3 = max_m - min_m;
            assert!(
                size_m.z > MIN_LENGTH_PER_WIDTH * size_m.x,
                "{model:?} is {size_m} across: its barrel isn't along z"
            );
        }
    }

    #[test]
    fn every_gun_model_is_drawn_at_its_scale() {
        for model in GunModel::ALL {
            let (_, max_m) = get_extent_m(&get_positions_from_muzzle_m(model));
            let drawn_length_m: f32 = model.get_modelled_length_m() * model.get_spec().scale;
            assert!(
                (max_m.z - drawn_length_m).abs() < LENGTH_TOLERANCE_M,
                "{model:?} reaches {} m behind its muzzle, not {drawn_length_m} m",
                max_m.z
            );
        }
    }
}
