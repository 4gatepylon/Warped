//! `render_metal.rs` ray-traces the scene on the GPU with Metal: it compiles `render_metal.metal`
//! (a port of `render_cpu.rs`) at start-up, copies each frame's scene rows and uniforms to the
//! GPU, runs one thread per pixel into a texture, and, when the output is larger than the render
//! size, upscales it with MetalFX's spatial scaler. The result is either read back (for the
//! benchmark's PNGs) or copied straight into a window's `CAMetalLayer` drawable.
//!
//! Interface: `MetalRenderer::New`, `MetalRenderer::render_to_pixels`,
//! `MetalRenderer::render_to_layer`, `MetalRenderer::get_device`, `MetalRenderer::last_gpu_ms`.
//!
//! Example: `let mut renderer = MetalRenderer::New()?; renderer.render_to_pixels(&data, size, &mut pixels)?;`
//!
//! Must know:
//! - Performance: it waits for each frame to finish, so the GPU is idle while the CPU packs the
//!   next frame; `last_gpu_ms` is the GPU's own time for the last frame, upscaling included.

use std::error::Error;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder,
    MTLCommandQueue, MTLComputeCommandEncoder, MTLComputePipelineState,
    MTLCreateSystemDefaultDevice, MTLDevice, MTLLibrary, MTLOrigin, MTLPixelFormat, MTLRegion,
    MTLResourceOptions, MTLSize, MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};
use objc2_metal_fx::{MTLFXSpatialScaler, MTLFXSpatialScalerBase, MTLFXSpatialScalerDescriptor};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

use crate::render_voxel::{VoxelFrame, VoxelGpu};
use crate::scene_data::{
    HALF_FOV_RAD, ROW_COUNT, ROW_TEXEL_COUNT, SceneData, TEXEL_BEAM_CHUNKS, TEXEL_BEAM_POINTS,
    TEXEL_BOXES, TEXEL_CLUSTER_INFO, TEXEL_CLUSTER_ORDER, TEXEL_CLUSTERS, TEXEL_GLUINGS,
    TEXEL_GROUND, TEXEL_MOUTH_FIELDS, TEXEL_MOUTHS, TEXEL_SPOT, TEXEL_SUN, TEXEL_TO_WORLD,
    TEXELS_PER_BEAM_CHUNK, TEXELS_PER_BOX, TEXELS_PER_CLUSTER, TEXELS_PER_GLUING,
    TEXELS_PER_MOUTH_FIELD, Uniforms,
};
use crate::viewmodel::Viewmodel;
use crate::world::{MATERIAL_GLASS, MATERIAL_GLOW, MAX_CLUSTER_COUNT, MAX_MOUTH_COUNT};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `Texture` is a Metal texture and its size in pixels.
type Texture = (Retained<ProtocolObject<dyn MTLTexture>>, (usize, usize));

/// `Overlay` is what to draw over a frame before it is shown: how much to darken the frame (while a
/// player is out after a hit), a text mask with its size (one byte a pixel, rows top first; see the
/// shader's `apply_overlay` for what its levels mean), up to `MAX_OVERLAY_RECT_COUNT` rectangles
/// (the HUD's bars), the player's wand, how much of the previous frame's scene to blend into
/// this one's (motion blur; 0 for none), what a recent hit does to the scene, how much of the
/// frame's height the mask's text may take, a picture under the text, how far the scene
/// stretches at the map's soft edge (the shader's `read_stretched_scene`; 0 for none), the
/// minimap, and how far into the blueshift after a wormhole the scene is (the shader's
/// `blueshift_scene`: 1 at its peak, 0 for none).
pub struct Overlay<'a> {
    pub dim_frac: f32,
    pub mask: Option<(&'a [u8], (usize, usize))>,
    pub rects: &'a [OverlayRect],
    pub wand: Option<OverlayWand>,
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
/// many marks and lines it has (then two zeros); the marks (other players: a point in the
/// minimap's unit disk, x right and y down, then a radius in pixels and a zero; and a colour);
/// and the lines (shots: two ends in the unit disk, then a colour).
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

/// `OverlayRect` is the shader's `OverlayRect`: corners as fractions of the screen (top left 0, 0),
/// an RGBA colour (A the opacity), how it looks beyond its colour (`FLAT_RECT_STYLE` for a
/// plain rectangle) and the pulse across it (`NO_RECT_PULSE` for none).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayRect {
    pub min_frac: [f32; 2],
    pub max_frac: [f32; 2],
    pub color: [f32; 4],
    pub style: RectStyle,
    pub pulse: RectPulse,
}

/// `RectPulse` is the shader's `RectPulse`, a band of light across an `OverlayRect` (a pulse
/// running along a HUD bar): its middle's x and its half width as fractions of the screen's width
/// (measured along the rect's slant, at its middle row), two zeros, and its RGBA colour, A its
/// opacity at its middle (0 for none).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectPulse {
    pub center_x_frac: f32,
    pub half_width_frac: f32,
    pub padding: [f32; 2],
    pub color: [f32; 4],
}

/// `NO_RECT_PULSE` is no pulse: an opacity of 0, which the shader skips.
pub const NO_RECT_PULSE: RectPulse = RectPulse {
    center_x_frac: 0.0,
    half_width_frac: 0.0,
    padding: [0.0; 2],
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

const BYTES_PER_OVERLAY_RECT_ACCORDING_TO_METAL_LAYOUT: usize = 112;
const _: () = assert!(
    std::mem::size_of::<OverlayRect>() == BYTES_PER_OVERLAY_RECT_ACCORDING_TO_METAL_LAYOUT,
    "OverlayRect must match the shader's 112-byte OverlayRect"
);

/// `OverlayParams` is the shader's `OverlayParams` (8-byte aligned, as its `int2` makes it).
#[repr(C, align(8))]
struct OverlayParams {
    dim_frac: f32,
    scale: i32,
    origin: [i32; 2],
    rect_count: i32,
    has_mask: i32,
    has_wand: i32,
    tan_half_fov: f32,
    blur_frac: f32,
    hit_frac: f32,
    hit_red_frac: f32,
    hit_blur_px: f32,
    shake_px: [f32; 2],
    picture_min_mask_px: [i32; 2],
    picture_max_excl_mask_px: [i32; 2],
    stretch_frac: f32,
    blueshift_frac: f32,
}

/// `Scaler` is a MetalFX spatial scaler with the input and output sizes it was made for.
type Scaler = (
    Retained<ProtocolObject<dyn MTLFXSpatialScaler>>,
    (usize, usize),
    (usize, usize),
);

/// `Frame` is an encoded frame: its command buffer (not yet committed) and the texture that will
/// hold its final image.
type Frame = (
    Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    Retained<ProtocolObject<dyn MTLTexture>>,
);

/// `Pipeline` is a compiled compute kernel.
pub(crate) type Pipeline = Retained<ProtocolObject<dyn MTLComputePipelineState>>;

/// `SampleBuffers` holds adaptive sampling's buffers for render sizes up to one size: the coarse samples,
/// the list of fine samples to trace and its length (followed by that pass's indirect dispatch
/// arguments), the fine samples, the cell marks, the list of cells to refine and its length (also
/// followed by dispatch arguments), the largest fine grid they hold, and the size of one sample.
pub(crate) struct SampleBuffers {
    coarse: Retained<ProtocolObject<dyn MTLBuffer>>,
    fill_list: Retained<ProtocolObject<dyn MTLBuffer>>,
    fill_count: Retained<ProtocolObject<dyn MTLBuffer>>,
    samples: Retained<ProtocolObject<dyn MTLBuffer>>,
    cells: Retained<ProtocolObject<dyn MTLBuffer>>,
    refine_cells: Retained<ProtocolObject<dyn MTLBuffer>>,
    refine_count: Retained<ProtocolObject<dyn MTLBuffer>>,
    grid: (usize, usize),
    bytes_per_ray_end: usize,
}

/// `AdaptivePipelines` is adaptive sampling's passes, compiled from a shader that has kernels of
/// these names using the buffer indices `encode_adaptive_sampling` sets (the house's and the voxel
/// renderer's shaders both do).
pub(crate) struct AdaptivePipelines {
    trace_coarse: Pipeline,
    fill_samples: Pipeline,
    prepare_fill: Pipeline,
    trace_listed_samples: Pipeline,
    classify_cells: Pipeline,
    prepare_refinement: Pipeline,
    resolve_pixels: Pipeline,
    refine_pixels: Pipeline,
}

impl AdaptivePipelines {
    /// `FromLibrary` compiles adaptive sampling's kernels from a shader library.
    ///
    /// Example: `let adaptive = AdaptivePipelines::FromLibrary(&device, &library)?;`
    #[allow(non_snake_case)]
    pub(crate) fn FromLibrary(
        device: &ProtocolObject<dyn MTLDevice>,
        library: &ProtocolObject<dyn MTLLibrary>,
    ) -> Result<AdaptivePipelines> {
        let make = |name: &str| make_pipeline(device, library, name);
        Ok(AdaptivePipelines {
            trace_coarse: make("trace_coarse")?,
            fill_samples: make("fill_samples")?,
            prepare_fill: make("prepare_fill")?,
            trace_listed_samples: make("trace_listed_samples")?,
            classify_cells: make("classify_cells")?,
            prepare_refinement: make("prepare_refinement")?,
            resolve_pixels: make("resolve_pixels")?,
            refine_pixels: make("refine_pixels")?,
        })
    }
}

/// `make_pipeline` compiles the kernel `name` of a shader library.
///
/// Example: `let trace_pixels = make_pipeline(&device, &library, "trace_pixels")?;`
pub(crate) fn make_pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    library: &ProtocolObject<dyn MTLLibrary>,
    name: &str,
) -> Result<Pipeline> {
    let function = library
        .newFunctionWithName(&NSString::from_str(name))
        .ok_or(format!("Metal: the shader has no kernel named {name}"))?;
    Ok(device
        .newComputePipelineStateWithFunction_error(&function)
        .map_err(|error| {
            format!(
                "Metal: the {name} pipeline failed: {}",
                error.localizedDescription()
            )
        })?)
}

/// `HoleBuffers` holds the hole filling's buffers for render sizes up to one size: one byte per
/// pixel saying whether its ray was lost, the pyramid (every level above the image, half the size
/// of the one below), and the largest render size they hold.
struct HoleBuffers {
    lost: Retained<ProtocolObject<dyn MTLBuffer>>,
    pyramid: Retained<ProtocolObject<dyn MTLBuffer>>,
    max_px: (usize, usize),
}

/// `PyramidLevels` is the shader's `PyramidLevels`: the sizes of a finer and a coarser pyramid level
/// and where each starts in the pyramid buffer.
#[repr(C)]
struct PyramidLevels {
    low_size: [i32; 2],
    high_size: [i32; 2],
    low_offset: i32,
    high_offset: i32,
}

const SHADER_SOURCE: &str = include_str!("render_metal.metal");
const BYTES_PER_RAY_END: usize = 48;
pub(crate) const VOXEL_BYTES_PER_RAY_END: usize = 96;
const BYTES_PER_CELL_MARK: usize = 1;
const BYTES_PER_CELL_INDEX: usize = 4;
const REFINE_COUNT_BYTES: usize = 16;
const DISPATCH_ARGUMENTS_BYTES: usize = 12;
pub(crate) const REFINE_GROUP_SIZE: usize = 64;
const THROAT_TABLE_SIZE: usize = 4096;
const CAMERA_TABLE_SIZE: usize = 2048;
const CAMERA_TABLE_ROWS: usize = 64;
const TEXELS_PER_TABLE_ENTRY: usize = 10;
const BYTES_PER_PIXEL: usize = 4;
const BYTES_PER_TEXEL: usize = 16;
const BYTES_PER_LOST_FLAG: usize = 1;
const BYTES_PER_PYRAMID_TEXEL: usize = 8;
const THREADGROUP_SIDE: usize = 8;
pub(crate) const OVERLAY_MAX_WIDTH_FRAC: f64 = 0.8;
pub(crate) const MAX_OVERLAY_RECT_COUNT: usize = 64;
pub(crate) const MAX_MINIMAP_MARK_COUNT: usize = 16;
pub(crate) const MAX_MINIMAP_LINE_COUNT: usize = 96;
/// `REVOLVER_WAND` is `has_wand` when the revolver (viewmodel.rs) is drawn in the wand's place.
const REVOLVER_WAND: i32 = 2;
const EMPTY_MASK_PX: (usize, usize) = (1, 1);
const BYTES_PER_PICTURE_PIXEL: usize = 4;
const MS_PER_S: f64 = 1000.0;
const PIXEL_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;
const _: () = assert!(
    MAX_CLUSTER_COUNT <= 32,
    "the shader keeps candidate clusters in a 32-bit mask"
);

/// `FrameData` is one frame to draw: the house demo's packed rows, or a voxel map's frame.
#[derive(Clone, Copy)]
pub enum FrameData<'a> {
    House(&'a SceneData),
    Voxel(&'a VoxelFrame),
}

impl FrameData<'_> {
    /// `get_render_size` gives the frame's render size in pixels.
    ///
    /// Example: `let render_px = frame_data.get_render_size();`
    pub fn get_render_size(&self) -> (usize, usize) {
        match self {
            FrameData::House(data) => (
                data.uniforms.width_px as usize,
                data.uniforms.height_px as usize,
            ),
            FrameData::Voxel(frame) => frame.get_render_size(),
        }
    }
}

/// `MetalRenderer` holds the GPU, the compiled ray tracer, the scene buffer, and the textures and
/// MetalFX scaler for the current render and output sizes.
pub struct MetalRenderer {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    trace_pixels: Pipeline,
    adaptive: AdaptivePipelines,
    build_throat_table: Pipeline,
    build_camera_table: Pipeline,
    build_mouth_fields: Pipeline,
    pull_holes_from_image: Pipeline,
    pull_holes: Pipeline,
    push_holes: Pipeline,
    fill_holes: Pipeline,
    apply_overlay: Pipeline,
    overlay_target: Option<Texture>,
    overlay_mask: Option<Texture>,
    overlay_picture: Option<Texture>,
    overlay_map: Option<Texture>,
    blur_histories: Option<(Texture, Texture)>,
    hole_buffers: Option<HoleBuffers>,
    voxel_gpu: Option<VoxelGpu>,
    viewmodel: Option<Viewmodel>,
    throat_table: Retained<ProtocolObject<dyn MTLBuffer>>,
    camera_table: Retained<ProtocolObject<dyn MTLBuffer>>,
    has_throat_table: bool,
    is_scaler_warm: bool,
    is_adaptive: bool,
    sample_buffers: Option<SampleBuffers>,
    rows: Retained<ProtocolObject<dyn MTLBuffer>>,
    render_texture: Option<Texture>,
    output_texture: Option<Texture>,
    scaler: Option<Scaler>,
    readback: Option<(Retained<ProtocolObject<dyn MTLBuffer>>, usize)>,
    pub last_gpu_ms: f64,
}

/// `fill_shader_constants` replaces each placeholder in the shader source (a name between double
/// underscores) with the Rust constant of that name, so the shader's row layout always matches
/// `scene_data.rs`.
///
/// Example: `let source = fill_shader_constants(SHADER_SOURCE);`
fn fill_shader_constants(source: &str) -> String {
    let constants: [(&str, usize); 28] = [
        ("TEXEL_MOUTH_FIELDS", TEXEL_MOUTH_FIELDS),
        ("TEXELS_PER_MOUTH_FIELD", TEXELS_PER_MOUTH_FIELD),
        ("TEXELS_PER_TABLE_ENTRY", TEXELS_PER_TABLE_ENTRY),
        ("CAMERA_TABLE_SIZE", CAMERA_TABLE_SIZE),
        ("CAMERA_TABLE_ROWS", CAMERA_TABLE_ROWS),
        ("THROAT_TABLE_SIZE", THROAT_TABLE_SIZE),
        ("REFINE_GROUP_SIZE", REFINE_GROUP_SIZE),
        ("BYTES_PER_RAY_END", BYTES_PER_RAY_END),
        ("MAX_MOUTH_COUNT", MAX_MOUTH_COUNT),
        ("ROW_TEXEL_COUNT", ROW_TEXEL_COUNT),
        ("TEXEL_TO_WORLD", TEXEL_TO_WORLD),
        ("TEXEL_GROUND", TEXEL_GROUND),
        ("TEXEL_SUN", TEXEL_SUN),
        ("TEXEL_MOUTHS", TEXEL_MOUTHS),
        ("TEXEL_GLUINGS", TEXEL_GLUINGS),
        ("TEXELS_PER_GLUING", TEXELS_PER_GLUING),
        ("TEXEL_BOXES", TEXEL_BOXES),
        ("TEXELS_PER_BOX", TEXELS_PER_BOX),
        ("TEXEL_CLUSTERS", TEXEL_CLUSTERS),
        ("TEXELS_PER_CLUSTER", TEXELS_PER_CLUSTER),
        ("TEXEL_CLUSTER_INFO", TEXEL_CLUSTER_INFO),
        ("TEXEL_CLUSTER_ORDER", TEXEL_CLUSTER_ORDER),
        ("TEXEL_SPOT", TEXEL_SPOT),
        ("TEXEL_BEAM_CHUNKS", TEXEL_BEAM_CHUNKS),
        ("TEXELS_PER_BEAM_CHUNK", TEXELS_PER_BEAM_CHUNK),
        ("TEXEL_BEAM_POINTS", TEXEL_BEAM_POINTS),
        ("MATERIAL_GLASS", MATERIAL_GLASS as usize),
        ("MATERIAL_GLOW", MATERIAL_GLOW as usize),
    ];
    let mut filled: String = source.to_string();
    for (name, value) in constants {
        filled = filled.replace(&format!("__{name}__"), &value.to_string());
    }
    assert!(
        !filled.contains("__"),
        "render_metal.metal has a placeholder render_metal.rs doesn't fill"
    );
    filled
}

/// `upload_texture` copies `bytes` (rows top first, `bytes_per_pixel` a pixel) into the texture in
/// `slot`, first making a new one of `pixel_format` if the slot is empty or holds another size.
///
/// Example: `upload_texture(&device, &mut self.overlay_mask, mask, (240, 24), MTLPixelFormat::R8Unorm, 1)?;`
fn upload_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    slot: &mut Option<Texture>,
    bytes: &[u8],
    size_px: (usize, usize),
    pixel_format: MTLPixelFormat,
    bytes_per_pixel: usize,
) -> Result<()> {
    assert_eq!(
        bytes.len(),
        size_px.0 * size_px.1 * bytes_per_pixel,
        "upload_texture: the bytes aren't {}x{} pixels of {bytes_per_pixel} bytes",
        size_px.0,
        size_px.1
    );
    if slot.as_ref().is_none_or(|(_, size)| *size != size_px) {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TEXTURE OF THE WRONG FORMAT OR SIZE WILL HAPPEN
        let descriptor: Retained<MTLTextureDescriptor> = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                pixel_format,
                size_px.0,
                size_px.1,
                false,
            )
        };
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        descriptor.setStorageMode(MTLStorageMode::Shared);
        let texture = device
            .newTextureWithDescriptor(&descriptor)
            .ok_or("Metal: no memory for an overlay texture")?;
        *slot = Some((texture, size_px));
    }
    let (texture, _) = slot.as_ref().expect("the texture was just made");
    let region: MTLRegion = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width: size_px.0,
            height: size_px.1,
            depth: 1,
        },
    };
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE BYTES WILL HAPPEN
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            region,
            0,
            NonNull::new(bytes.as_ptr() as *mut std::ffi::c_void).expect("a texture's bytes"),
            size_px.0 * bytes_per_pixel,
        )
    };
    Ok(())
}

/// `dispatch_grid` runs the encoder's current kernel once per point of a 2D grid, in square
/// thread groups.
///
/// Example: `dispatch_grid(&encoder, (1280, 800));`
fn dispatch_grid(encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>, size: (usize, usize)) {
    encoder.dispatchThreads_threadsPerThreadgroup(
        MTLSize {
            width: size.0,
            height: size.1,
            depth: 1,
        },
        MTLSize {
            width: THREADGROUP_SIDE,
            height: THREADGROUP_SIDE,
            depth: 1,
        },
    );
}

/// `get_sample_grid` gives the size of adaptive sampling's sample grid for a render size: one
/// sample per even pixel each way, and one more past each far edge, so every pixel lies between
/// samples (the shader's `get_sample_columns` matches it).
///
/// Example: `let grid = get_sample_grid((2560, 1600));`
fn get_sample_grid(render_px: (usize, usize)) -> (usize, usize) {
    (render_px.0 / 2 + 2, render_px.1 / 2 + 2)
}

/// `get_coarse_grid` gives the size of adaptive sampling's coarse grid for a render size: one
/// sample per fourth pixel each way, and enough past each far edge to cover the fine grid (the
/// shader's `get_coarse_columns` matches it).
///
/// Example: `let coarse_grid = get_coarse_grid((2560, 1600));`
fn get_coarse_grid(render_px: (usize, usize)) -> (usize, usize) {
    (render_px.0 / 4 + 3, render_px.1 / 4 + 3)
}

/// `get_pyramid_levels` gives the hole-filling pyramid's levels for a render size, as (size, offset
/// in the pyramid buffer): level 0 is the image itself (offset 0, unused), and each level above
/// is half the one below, rounded up, down to 1×1, stored one after another from level 1.
///
/// Example: `let levels = get_pyramid_levels((2560, 1600));`
fn get_pyramid_levels(render_px: (usize, usize)) -> Vec<((usize, usize), usize)> {
    let mut levels: Vec<((usize, usize), usize)> = vec![(render_px, 0)];
    let mut offset: usize = 0;
    let mut size: (usize, usize) = render_px;
    while size != (1, 1) {
        size = (size.0.div_ceil(2), size.1.div_ceil(2));
        levels.push((size, offset));
        offset += size.0 * size.1;
    }
    levels
}

/// `encode_adaptive_sampling` encodes adaptive sampling's passes into an encoder that already has
/// the scene's buffers and the render texture: trace every fourth pixel, fill in every second
/// pixel (interpolating where that is exact enough, tracing the rest), mark the smooth cells and
/// list the others, size the refinement, write the pixels that need no new ray, and trace the
/// listed ones. The buffers must hold samples of the shader's ray end size, `bytes_per_ray_end`.
///
/// Example: `encode_adaptive_sampling(&encoder, &self.adaptive, buffers, BYTES_PER_RAY_END, (2560, 1600));`
pub(crate) fn encode_adaptive_sampling(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    pipelines: &AdaptivePipelines,
    buffers: &SampleBuffers,
    bytes_per_ray_end: usize,
    render_px: (usize, usize),
) {
    assert_eq!(
        buffers.bytes_per_ray_end, bytes_per_ray_end,
        "encode_adaptive_sampling: the sample buffers were made for another ray end size"
    );
    let grid: (usize, usize) = get_sample_grid(render_px);
    assert!(
        buffers.grid.0 >= grid.0 && buffers.grid.1 >= grid.1,
        "encode_adaptive_sampling: buffers for {:?} samples can't hold {grid:?}",
        buffers.grid
    );
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
    unsafe {
        encoder.setBuffer_offset_atIndex(Some(&buffers.samples), 0, 2);
        encoder.setBuffer_offset_atIndex(Some(&buffers.cells), 0, 3);
        encoder.setBuffer_offset_atIndex(Some(&buffers.refine_cells), 0, 4);
        encoder.setBuffer_offset_atIndex(Some(&buffers.refine_count), 0, 5);
        encoder.setBuffer_offset_atIndex(Some(&buffers.refine_count), REFINE_COUNT_BYTES, 6);
        encoder.setBuffer_offset_atIndex(Some(&buffers.coarse), 0, 9);
        encoder.setBuffer_offset_atIndex(Some(&buffers.fill_list), 0, 11);
        encoder.setBuffer_offset_atIndex(Some(&buffers.fill_count), 0, 12);
        encoder.setBuffer_offset_atIndex(Some(&buffers.fill_count), REFINE_COUNT_BYTES, 13);
    }
    encoder.setComputePipelineState(&pipelines.trace_coarse);
    dispatch_grid(encoder, get_coarse_grid(render_px));
    encoder.setComputePipelineState(&pipelines.fill_samples);
    dispatch_grid(encoder, grid);
    encoder.setComputePipelineState(&pipelines.prepare_fill);
    dispatch_grid(encoder, (1, 1));
    encoder.setComputePipelineState(&pipelines.trace_listed_samples);
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A DISPATCH SIZED FROM THE WRONG BYTES WILL HAPPEN
    unsafe {
        encoder.dispatchThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerThreadgroup(
            &buffers.fill_count,
            REFINE_COUNT_BYTES,
            MTLSize {
                width: REFINE_GROUP_SIZE,
                height: 1,
                depth: 1,
            },
        );
    }
    encoder.setComputePipelineState(&pipelines.classify_cells);
    dispatch_grid(encoder, grid);
    encoder.setComputePipelineState(&pipelines.prepare_refinement);
    dispatch_grid(encoder, (1, 1));
    encoder.setComputePipelineState(&pipelines.resolve_pixels);
    dispatch_grid(encoder, render_px);
    encoder.setComputePipelineState(&pipelines.refine_pixels);
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A DISPATCH SIZED FROM THE WRONG BYTES WILL HAPPEN
    unsafe {
        encoder.dispatchThreadgroupsWithIndirectBuffer_indirectBufferOffset_threadsPerThreadgroup(
            &buffers.refine_count,
            REFINE_COUNT_BYTES,
            MTLSize {
                width: REFINE_GROUP_SIZE,
                height: 1,
                depth: 1,
            },
        );
    }
}

impl MetalRenderer {
    /// `New` opens the default GPU, compiles the ray tracer and allocates the scene buffer. With
    /// `is_adaptive`, it traces every other pixel each way and interpolates where that is exact
    /// enough (see `resolve_pixels` in the shader); otherwise it traces every pixel.
    ///
    /// Example: `let renderer = MetalRenderer::New(true)?;`
    #[allow(non_snake_case)]
    pub fn New(is_adaptive: bool) -> Result<MetalRenderer> {
        let device: Retained<ProtocolObject<dyn MTLDevice>> =
            MTLCreateSystemDefaultDevice().ok_or("Metal: this machine has no GPU Metal can use")?;
        let source: Retained<NSString> = NSString::from_str(&fill_shader_constants(SHADER_SOURCE));
        let library: Retained<ProtocolObject<dyn MTLLibrary>> = device
            .newLibraryWithSource_options_error(&source, None)
            .map_err(|error| {
                format!(
                    "Metal: the shader didn't compile: {}",
                    error.localizedDescription()
                )
            })?;
        let make_pipeline = |name: &str| make_pipeline(&device, &library, name);
        let trace_pixels: Pipeline = make_pipeline("trace_pixels")?;
        let adaptive: AdaptivePipelines = AdaptivePipelines::FromLibrary(&device, &library)?;
        let (build_throat_table, build_camera_table) = (
            make_pipeline("build_throat_table")?,
            make_pipeline("build_camera_table")?,
        );
        let (pull_holes_from_image, pull_holes) = (
            make_pipeline("pull_holes_from_image")?,
            make_pipeline("pull_holes")?,
        );
        let (push_holes, fill_holes) = (make_pipeline("push_holes")?, make_pipeline("fill_holes")?);
        let apply_overlay: Pipeline = make_pipeline("apply_overlay")?;
        let build_mouth_fields: Pipeline = make_pipeline("build_mouth_fields")?;
        let make_table = |entry_count: usize| {
            device
                .newBufferWithLength_options(
                    entry_count * BYTES_PER_TEXEL,
                    MTLResourceOptions::StorageModePrivate,
                )
                .ok_or("Metal: no memory for a jump table")
        };
        let (throat_table, camera_table) = (
            make_table(THROAT_TABLE_SIZE * TEXELS_PER_TABLE_ENTRY)?,
            make_table(CAMERA_TABLE_SIZE * CAMERA_TABLE_ROWS * TEXELS_PER_TABLE_ENTRY)?,
        );
        let queue: Retained<ProtocolObject<dyn MTLCommandQueue>> =
            device.newCommandQueue().ok_or("Metal: no command queue")?;
        let rows: Retained<ProtocolObject<dyn MTLBuffer>> = device
            .newBufferWithLength_options(
                ROW_COUNT * ROW_TEXEL_COUNT * BYTES_PER_TEXEL,
                MTLResourceOptions::StorageModeShared,
            )
            .ok_or("Metal: no memory for the scene rows")?;
        Ok(MetalRenderer {
            device,
            queue,
            trace_pixels,
            adaptive,
            build_throat_table,
            build_camera_table,
            build_mouth_fields,
            pull_holes_from_image,
            pull_holes,
            push_holes,
            fill_holes,
            apply_overlay,
            overlay_target: None,
            blur_histories: None,
            overlay_mask: None,
            overlay_picture: None,
            overlay_map: None,
            hole_buffers: None,
            voxel_gpu: None,
            viewmodel: None,
            throat_table,
            camera_table,
            has_throat_table: false,
            is_scaler_warm: false,
            is_adaptive,
            sample_buffers: None,
            rows,
            render_texture: None,
            output_texture: None,
            scaler: None,
            readback: None,
            last_gpu_ms: 0.0,
        })
    }

    /// `get_device` gives the GPU, so a window's layer can use the same one.
    ///
    /// Example: `layer.setDevice(Some(renderer.get_device()));`
    pub fn get_device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.device
    }

    /// `make_texture` makes a private BGRA texture of a size with the given uses.
    ///
    /// Example: `let texture = self.make_texture((640, 400), MTLTextureUsage::ShaderWrite)?;`
    fn make_texture(&self, size_px: (usize, usize), usage: MTLTextureUsage) -> Result<Texture> {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TEXTURE OF THE WRONG FORMAT OR SIZE WILL HAPPEN
        let descriptor: Retained<MTLTextureDescriptor> = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                PIXEL_FORMAT,
                size_px.0,
                size_px.1,
                false,
            )
        };
        descriptor.setUsage(usage);
        descriptor.setStorageMode(MTLStorageMode::Private);
        let texture = self
            .device
            .newTextureWithDescriptor(&descriptor)
            .ok_or(format!(
                "Metal: no memory for a {}x{} texture",
                size_px.0, size_px.1
            ))?;
        Ok((texture, size_px))
    }

    /// `prepare_upscaling` makes the MetalFX scaler and its output texture for an output size, if
    /// the current ones are for another. The scaler takes any input up to that size (its input
    /// content size is set each frame), so changing the render scale needs no new scaler.
    ///
    /// Example: `self.prepare_upscaling((2560, 1600))?;`
    fn prepare_upscaling(&mut self, output_px: (usize, usize)) -> Result<()> {
        if self
            .scaler
            .as_ref()
            .is_some_and(|(_, _, output)| *output == output_px)
        {
            return Ok(());
        }
        let descriptor: Retained<MTLFXSpatialScalerDescriptor> =
            unsafe { MTLFXSpatialScalerDescriptor::new() };
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A SCALER THAT DOESN'T MATCH ITS TEXTURES WILL HAPPEN
        let scaler = unsafe {
            descriptor.setInputWidth(output_px.0);
            descriptor.setInputHeight(output_px.1);
            descriptor.setOutputWidth(output_px.0);
            descriptor.setOutputHeight(output_px.1);
            descriptor.setColorTextureFormat(PIXEL_FORMAT);
            descriptor.setOutputTextureFormat(PIXEL_FORMAT);
            descriptor.newSpatialScalerWithDevice(&self.device)
        }
        .ok_or("MetalFX: this GPU has no spatial scaler")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN AN OUTPUT TEXTURE THE SCALER CAN'T WRITE WILL HAPPEN
        let usage: MTLTextureUsage =
            unsafe { scaler.outputTextureUsage() } | MTLTextureUsage::ShaderRead;
        self.output_texture = Some(self.make_texture(output_px, usage)?);
        self.scaler = Some((scaler, output_px, output_px));
        Ok(())
    }

    /// `encode_frame` encodes one frame: the scene rows are copied to the GPU, the ray tracer runs
    /// at the scene's render size, and MetalFX upscales the result when `output_px` is larger.
    ///
    /// Example: `let (command_buffer, texture) = self.encode_frame(&data, (2560, 1600))?;`
    fn encode_frame(&mut self, frame_data: FrameData, output_px: (usize, usize)) -> Result<Frame> {
        let render_px: (usize, usize) = frame_data.get_render_size();
        let is_upscaled: bool = output_px != render_px;
        assert!(
            render_px.0 <= output_px.0 && render_px.1 <= output_px.1,
            "Metal: the render size {render_px:?} is larger than the output {output_px:?}"
        );
        if self
            .render_texture
            .as_ref()
            .is_none_or(|(_, size)| *size != output_px)
        {
            self.render_texture = Some(self.make_texture(
                output_px,
                MTLTextureUsage::ShaderWrite | MTLTextureUsage::ShaderRead,
            )?);
        }
        self.prepare_upscaling(output_px)?;
        if let FrameData::Voxel(voxel_frame) = frame_data {
            return self.encode_voxel_frame(voxel_frame, output_px);
        }
        let FrameData::House(data) = frame_data else {
            unreachable!("encode_frame: the voxel frame returned above")
        };
        if self.is_adaptive {
            self.prepare_sample_buffers(output_px, BYTES_PER_RAY_END)?;
        }
        self.prepare_hole_buffers(output_px)?;
        let row_bytes: usize = data.rows.len() * BYTES_PER_TEXEL;
        assert!(
            row_bytes <= self.rows.length(),
            "Metal: the scene rows don't fit their buffer"
        );
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A WRITE PAST THE ROWS BUFFER WILL HAPPEN
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.rows.as_ptr().cast::<u8>(),
                self.rows.contents().as_ptr().cast::<u8>(),
                row_bytes,
            )
        };
        let command_buffer = self
            .queue
            .commandBuffer()
            .ok_or("Metal: no command buffer")?;
        let encoder = command_buffer
            .computeCommandEncoder()
            .ok_or("Metal: no compute encoder")?;
        let uniforms: NonNull<Uniforms> = NonNull::from(&data.uniforms);
        let should_build_table: bool = !self.has_throat_table;
        self.has_throat_table = true;
        let (render_texture, _) = self
            .render_texture
            .as_ref()
            .expect("the render texture was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&self.rows), 0, 0);
            encoder.setBytes_length_atIndex(uniforms.cast(), std::mem::size_of::<Uniforms>(), 1);
            encoder.setTexture_atIndex(Some(render_texture), 0);
            encoder.setBuffer_offset_atIndex(Some(&self.throat_table), 0, 7);
            encoder.setBuffer_offset_atIndex(Some(&self.camera_table), 0, 8);
        }
        let hole_buffers: &HoleBuffers = self
            .hole_buffers
            .as_ref()
            .expect("prepare_hole_buffers just made them");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&hole_buffers.lost), 0, 14);
            encoder.setBuffer_offset_atIndex(Some(&hole_buffers.pyramid), 0, 15);
        }
        encoder.setComputePipelineState(&self.build_mouth_fields);
        dispatch_grid(&encoder, (MAX_MOUTH_COUNT, ROW_COUNT));
        if should_build_table {
            encoder.setComputePipelineState(&self.build_throat_table);
            dispatch_grid(&encoder, (THROAT_TABLE_SIZE, 1));
            encoder.setComputePipelineState(&self.build_camera_table);
            dispatch_grid(&encoder, (CAMERA_TABLE_SIZE, CAMERA_TABLE_ROWS));
        }
        if self.is_adaptive {
            let buffers: &SampleBuffers = self
                .sample_buffers
                .as_ref()
                .expect("prepare_sample_buffers just made them");
            encode_adaptive_sampling(
                &encoder,
                &self.adaptive,
                buffers,
                BYTES_PER_RAY_END,
                render_px,
            );
        } else {
            encoder.setComputePipelineState(&self.trace_pixels);
            dispatch_grid(&encoder, render_px);
        }
        self.encode_hole_filling(&encoder, render_px);
        encoder.endEncoding();
        let texture: Retained<ProtocolObject<dyn MTLTexture>> = render_texture.clone();
        self.finish_encoding(command_buffer, texture, render_px, is_upscaled)
    }

    /// `encode_voxel_frame` encodes a voxel map's frame: the voxel kernel into the render texture,
    /// then upscaling if needed.
    ///
    /// Example: `let (command_buffer, texture) = self.encode_voxel_frame(&frame, (2560, 1600))?;`
    fn encode_voxel_frame(
        &mut self,
        frame: &VoxelFrame,
        output_px: (usize, usize),
    ) -> Result<Frame> {
        let render_px: (usize, usize) = frame.get_render_size();
        if self.voxel_gpu.is_none() {
            self.voxel_gpu = Some(VoxelGpu::New(&self.device)?);
        }
        if self.is_adaptive {
            self.prepare_sample_buffers(output_px, VOXEL_BYTES_PER_RAY_END)?;
        }
        let command_buffer = self
            .queue
            .commandBuffer()
            .ok_or("Metal: no command buffer")?;
        let encoder = command_buffer
            .computeCommandEncoder()
            .ok_or("Metal: no compute encoder")?;
        let (render_texture, _) = self
            .render_texture
            .as_ref()
            .expect("encode_frame made the render texture");
        self.voxel_gpu
            .as_mut()
            .expect("the voxel pipeline was just made")
            .encode(
                &self.device,
                &encoder,
                frame,
                self.sample_buffers.as_ref(),
                render_texture,
            )?;
        encoder.endEncoding();
        let texture: Retained<ProtocolObject<dyn MTLTexture>> = render_texture.clone();
        self.finish_encoding(command_buffer, texture, render_px, output_px != render_px)
    }

    /// `finish_encoding` adds MetalFX upscaling to a frame's command buffer when the render size is
    /// smaller than the output (or once, to warm the scaler up), and gives the texture to show.
    ///
    /// Example: `self.finish_encoding(command_buffer, texture, render_px, is_upscaled)`
    fn finish_encoding(
        &mut self,
        command_buffer: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
        render_texture: Retained<ProtocolObject<dyn MTLTexture>>,
        render_px: (usize, usize),
        is_upscaled: bool,
    ) -> Result<Frame> {
        if !is_upscaled && self.is_scaler_warm {
            return Ok((command_buffer, render_texture));
        }
        let (scaler, _, _) = self.scaler.as_ref().expect("the scaler was just made");
        let (output_texture, _) = self
            .output_texture
            .as_ref()
            .expect("the output texture was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN MetalFX READING OR WRITING THE WRONG TEXTURE WILL HAPPEN
        unsafe {
            scaler.setInputContentWidth(render_px.0);
            scaler.setInputContentHeight(render_px.1);
            scaler.setColorTexture(Some(&render_texture));
            scaler.setOutputTexture(Some(output_texture));
            scaler.encodeToCommandBuffer(&command_buffer);
        }
        let output: Retained<ProtocolObject<dyn MTLTexture>> = if is_upscaled {
            output_texture.clone()
        } else {
            render_texture
        };
        self.is_scaler_warm = true;
        Ok((command_buffer, output))
    }

    /// `encode_hole_filling` encodes the passes that paint lost pixels (rays trapped circling a
    /// throat) from the pixels around them instead of black: average the other pixels into a
    /// pyramid of ever coarser levels, fill each level's gaps from the one above, coarsest first,
    /// then paint the lost pixels from the first level.
    ///
    /// Example: `self.encode_hole_filling(&encoder, (2560, 1600));`
    fn encode_hole_filling(
        &self,
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
        render_px: (usize, usize),
    ) {
        let levels: Vec<((usize, usize), usize)> = get_pyramid_levels(render_px);
        let set_levels = |low: ((usize, usize), usize), high: ((usize, usize), usize)| {
            let step: PyramidLevels = PyramidLevels {
                low_size: [low.0.0 as i32, low.0.1 as i32],
                high_size: [high.0.0 as i32, high.0.1 as i32],
                low_offset: low.1 as i32,
                high_offset: high.1 as i32,
            };
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG LEVELS WILL HAPPEN
            unsafe {
                encoder.setBytes_length_atIndex(
                    NonNull::from(&step).cast(),
                    std::mem::size_of::<PyramidLevels>(),
                    16,
                )
            };
        };
        for k in 0..levels.len() - 1 {
            set_levels(levels[k], levels[k + 1]);
            encoder.setComputePipelineState(if k == 0 {
                &self.pull_holes_from_image
            } else {
                &self.pull_holes
            });
            dispatch_grid(encoder, levels[k + 1].0);
        }
        for k in (1..levels.len() - 1).rev() {
            set_levels(levels[k], levels[k + 1]);
            encoder.setComputePipelineState(&self.push_holes);
            dispatch_grid(encoder, levels[k].0);
        }
        set_levels(levels[0], levels[1]);
        encoder.setComputePipelineState(&self.fill_holes);
        dispatch_grid(encoder, render_px);
    }

    /// `prepare_hole_buffers` makes the hole filling's buffers big enough for any render size up to
    /// `max_px`, if the current ones are for another size.
    ///
    /// Example: `self.prepare_hole_buffers((2560, 1600))?;`
    fn prepare_hole_buffers(&mut self, max_px: (usize, usize)) -> Result<()> {
        if self
            .hole_buffers
            .as_ref()
            .is_some_and(|buffers| buffers.max_px == max_px)
        {
            return Ok(());
        }
        let (top_size, top_offset): ((usize, usize), usize) = *get_pyramid_levels(max_px)
            .last()
            .expect("a pyramid has at least the image");
        let pyramid_texel_count: usize = top_offset + top_size.0 * top_size.1;
        let make_buffer = |bytes: usize| {
            self.device
                .newBufferWithLength_options(bytes, MTLResourceOptions::StorageModePrivate)
                .ok_or(format!(
                    "Metal: no memory to fill holes at {}x{}",
                    max_px.0, max_px.1
                ))
        };
        self.hole_buffers = Some(HoleBuffers {
            lost: make_buffer(max_px.0 * max_px.1 * BYTES_PER_LOST_FLAG)?,
            pyramid: make_buffer(pyramid_texel_count * BYTES_PER_PYRAMID_TEXEL)?,
            max_px,
        });
        Ok(())
    }

    /// `prepare_sample_buffers` makes adaptive sampling's buffers big enough for any render size up
    /// to `max_px`, if the current ones are for another size.
    ///
    /// Example: `self.prepare_sample_buffers((2560, 1600), BYTES_PER_RAY_END)?;`
    fn prepare_sample_buffers(
        &mut self,
        max_px: (usize, usize),
        bytes_per_ray_end: usize,
    ) -> Result<()> {
        let grid: (usize, usize) = get_sample_grid(max_px);
        if self.sample_buffers.as_ref().is_some_and(|buffers| {
            buffers.grid == grid && buffers.bytes_per_ray_end == bytes_per_ray_end
        }) {
            return Ok(());
        }
        let make_buffer = |bytes: usize| {
            self.device
                .newBufferWithLength_options(bytes, MTLResourceOptions::StorageModePrivate)
                .ok_or(format!(
                    "Metal: no memory for a {}x{} sample grid",
                    grid.0, grid.1
                ))
        };
        let cell_count: usize = grid.0 * grid.1;
        let coarse_grid: (usize, usize) = get_coarse_grid(max_px);
        self.sample_buffers = Some(SampleBuffers {
            coarse: make_buffer(coarse_grid.0 * coarse_grid.1 * bytes_per_ray_end)?,
            fill_list: make_buffer(cell_count * BYTES_PER_CELL_INDEX)?,
            fill_count: make_buffer(REFINE_COUNT_BYTES + DISPATCH_ARGUMENTS_BYTES)?,
            samples: make_buffer(cell_count * bytes_per_ray_end)?,
            cells: make_buffer(cell_count * BYTES_PER_CELL_MARK)?,
            refine_cells: make_buffer(cell_count * BYTES_PER_CELL_INDEX)?,
            refine_count: make_buffer(REFINE_COUNT_BYTES + DISPATCH_ARGUMENTS_BYTES)?,
            grid,
            bytes_per_ray_end,
        });
        Ok(())
    }

    /// `finish` commits a command buffer, waits for it, checks it worked and records its GPU time.
    ///
    /// Example: `self.finish(&command_buffer)?;`
    fn finish(&mut self, command_buffer: &ProtocolObject<dyn MTLCommandBuffer>) -> Result<()> {
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        if command_buffer.status() != MTLCommandBufferStatus::Completed {
            let reason: String = command_buffer
                .error()
                .map_or("unknown".to_string(), |error| {
                    error.localizedDescription().to_string()
                });
            return Err(format!("Metal: the frame failed: {reason}").into());
        }
        self.last_gpu_ms = (command_buffer.GPUEndTime() - command_buffer.GPUStartTime()) * MS_PER_S;
        Ok(())
    }

    /// `render_to_pixels` draws one frame at `output_px` (upscaled with MetalFX if larger than the
    /// render size) and reads it back into `pixels` (RGBA, top row first).
    ///
    /// Example: `renderer.render_to_pixels(&data, (2560, 1600), &mut pixels)?;`
    pub fn render_to_pixels(
        &mut self,
        data: FrameData,
        output_px: (usize, usize),
        pixels: &mut [u8],
    ) -> Result<()> {
        let pixel_bytes: usize = output_px.0 * output_px.1 * BYTES_PER_PIXEL;
        if pixels.len() != pixel_bytes {
            return Err(format!(
                "Metal: the pixel buffer has {} bytes, not {pixel_bytes}",
                pixels.len()
            )
            .into());
        }
        if self
            .readback
            .as_ref()
            .is_none_or(|(_, bytes)| *bytes != pixel_bytes)
        {
            let buffer = self
                .device
                .newBufferWithLength_options(pixel_bytes, MTLResourceOptions::StorageModeShared)
                .ok_or("Metal: no memory to read the pixels back")?;
            self.readback = Some((buffer, pixel_bytes));
        }
        let (command_buffer, texture) = self.encode_frame(data, output_px)?;
        let (readback, _) = self
            .readback
            .as_ref()
            .expect("the readback buffer was just made");
        let blit = command_buffer
            .blitCommandEncoder()
            .ok_or("Metal: no blit encoder")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE READBACK BUFFER WILL HAPPEN
        unsafe {
            blit.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toBuffer_destinationOffset_destinationBytesPerRow_destinationBytesPerImage(
                &texture,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width: output_px.0, height: output_px.1, depth: 1 },
                readback,
                0,
                output_px.0 * BYTES_PER_PIXEL,
                pixel_bytes,
            );
        }
        blit.endEncoding();
        self.finish(&command_buffer)?;
        let (readback, _) = self
            .readback
            .as_ref()
            .expect("the readback buffer was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE READBACK BUFFER WILL HAPPEN
        unsafe {
            std::ptr::copy_nonoverlapping(
                readback.contents().as_ptr().cast::<u8>(),
                pixels.as_mut_ptr(),
                pixel_bytes,
            )
        };
        for pixel in pixels.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
            pixel.swap(0, 2);
        }
        Ok(())
    }

    /// `encode_overlay` encodes drawing `overlay` over a finished frame into a texture of its own,
    /// which it returns: the frame blended with the previous frame's scene (kept in one of two
    /// history textures, swapped each frame; none after a resize) and darkened, the mask (if any)
    /// scaled by a whole number to at most `mask_height_frac` of the frame's height (and at most
    /// `OVERLAY_MAX_WIDTH_FRAC` of its width), centred, over the picture (if any), and the
    /// rectangles.
    ///
    /// Example: `let shown = self.encode_overlay(&command_buffer, &frame, &overlay, (2560, 1600))?;`
    fn encode_overlay(
        &mut self,
        command_buffer: &ProtocolObject<dyn MTLCommandBuffer>,
        frame: &ProtocolObject<dyn MTLTexture>,
        overlay: &Overlay,
        output_px: (usize, usize),
    ) -> Result<Retained<ProtocolObject<dyn MTLTexture>>> {
        assert!(
            overlay.rects.len() <= MAX_OVERLAY_RECT_COUNT,
            "encode_overlay: {} rectangles, more than {MAX_OVERLAY_RECT_COUNT}",
            overlay.rects.len()
        );
        let empty_mask: [u8; 1] = [0];
        let (mask_bytes, (mask_w, mask_h)) = overlay.mask.unwrap_or((&empty_mask, EMPTY_MASK_PX));
        if self
            .overlay_target
            .as_ref()
            .is_none_or(|(_, size)| *size != output_px)
        {
            self.overlay_target = Some(self.make_texture(
                output_px,
                MTLTextureUsage::ShaderWrite | MTLTextureUsage::ShaderRead,
            )?);
        }
        let is_blur_history_valid: bool = self
            .blur_histories
            .as_ref()
            .is_some_and(|((_, size), _)| *size == output_px);
        if !is_blur_history_valid {
            let usage: MTLTextureUsage = MTLTextureUsage::ShaderWrite | MTLTextureUsage::ShaderRead;
            self.blur_histories = Some((
                self.make_texture(output_px, usage)?,
                self.make_texture(output_px, usage)?,
            ));
        }
        let empty_picture: [u8; BYTES_PER_PICTURE_PIXEL] = [0; BYTES_PER_PICTURE_PIXEL];
        let (picture_bytes, picture_px, picture_corners_mask_px) = match &overlay.picture {
            Some(picture) => (
                picture.pixels,
                picture.size_px,
                [picture.min_mask_px, picture.max_excl_mask_px],
            ),
            None => (&empty_picture[..], EMPTY_MASK_PX, [[0; 2]; 2]),
        };
        upload_texture(
            &self.device,
            &mut self.overlay_mask,
            mask_bytes,
            (mask_w, mask_h),
            MTLPixelFormat::R8Unorm,
            1,
        )?;
        upload_texture(
            &self.device,
            &mut self.overlay_picture,
            picture_bytes,
            picture_px,
            MTLPixelFormat::RGBA8Unorm,
            BYTES_PER_PICTURE_PIXEL,
        )?;
        let (mask, _) = self.overlay_mask.as_ref().expect("the mask was just made");
        let minimap: Minimap = match &overlay.minimap {
            Some(minimap) => {
                let is_uploaded: bool = self
                    .overlay_map
                    .as_ref()
                    .is_some_and(|(_, size)| *size == minimap.map_size_px);
                if !is_uploaded {
                    upload_texture(
                        &self.device,
                        &mut self.overlay_map,
                        minimap.map_pixels,
                        minimap.map_size_px,
                        MTLPixelFormat::RGBA8Unorm,
                        BYTES_PER_PICTURE_PIXEL,
                    )?;
                }
                minimap.minimap
            }
            None => Minimap::Hidden(),
        };
        let (picture, _) = self
            .overlay_picture
            .as_ref()
            .expect("the picture was just made");
        let map: &ProtocolObject<dyn MTLTexture> = match &self.overlay_map {
            Some((map, _)) => map,
            None => picture,
        };
        let tan_half_fov: f32 = HALF_FOV_RAD.tan() as f32;
        let revolver_image: Option<Retained<ProtocolObject<dyn MTLTexture>>> = match overlay.wand {
            Some(wand) => {
                if self.viewmodel.is_none() {
                    self.viewmodel = Some(Viewmodel::Load(&self.device)?);
                }
                let revolver: &mut Viewmodel = self
                    .viewmodel
                    .as_mut()
                    .expect("the revolver was just loaded");
                Some(revolver.encode(
                    &self.device,
                    command_buffer,
                    &wand,
                    output_px,
                    tan_half_fov,
                )?)
            }
            None => None,
        };
        let scale: usize = ((output_px.1 as f32 * overlay.mask_height_frac) as usize / mask_h)
            .min((output_px.0 as f64 * OVERLAY_MAX_WIDTH_FRAC) as usize / mask_w)
            .max(1);
        let params: OverlayParams = OverlayParams {
            dim_frac: overlay.dim_frac,
            scale: scale as i32,
            origin: [
                (output_px.0 as i32 - (mask_w * scale) as i32) / 2,
                (output_px.1 as i32 - (mask_h * scale) as i32) / 2,
            ],
            rect_count: overlay.rects.len() as i32,
            has_mask: i32::from(overlay.mask.is_some()),
            has_wand: if revolver_image.is_some() {
                REVOLVER_WAND
            } else {
                i32::from(overlay.wand.is_some())
            },
            tan_half_fov,
            blur_frac: if is_blur_history_valid {
                overlay.blur_frac
            } else {
                0.0
            },
            hit_frac: overlay.hit.strength_frac,
            hit_red_frac: overlay.hit.red_frac,
            hit_blur_px: overlay.hit.blur_px,
            shake_px: overlay.hit.shake_px,
            picture_min_mask_px: picture_corners_mask_px[0],
            picture_max_excl_mask_px: picture_corners_mask_px[1],
            stretch_frac: overlay.stretch_frac,
            blueshift_frac: overlay.blueshift_frac,
        };
        let wand: OverlayWand = overlay.wand.unwrap_or(OverlayWand {
            center_m: [0.0; 4],
            axes: [[0.0; 4]; 3],
            shaft_half_m: [0.0; 4],
            shaft_color: [0.0; 4],
            tip_half_m: [0.0; 4],
            tip_color: [0.0; 4],
        });
        let mut rects: [OverlayRect; MAX_OVERLAY_RECT_COUNT] = [OverlayRect {
            min_frac: [0.0; 2],
            max_frac: [0.0; 2],
            color: [0.0; 4],
            style: FLAT_RECT_STYLE,
            pulse: NO_RECT_PULSE,
        }; MAX_OVERLAY_RECT_COUNT];
        rects[..overlay.rects.len()].copy_from_slice(overlay.rects);
        let (target, _) = self
            .overlay_target
            .as_ref()
            .expect("the target was just made");
        let ((history_in, _), (history_out, _)) = self
            .blur_histories
            .as_ref()
            .expect("the histories were just made");
        let encoder = command_buffer
            .computeCommandEncoder()
            .ok_or("Metal: no compute encoder")?;
        encoder.setComputePipelineState(&self.apply_overlay);
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG TEXTURES WILL HAPPEN
        unsafe {
            encoder.setTexture_atIndex(Some(frame), 0);
            encoder.setTexture_atIndex(Some(mask), 1);
            encoder.setTexture_atIndex(Some(target), 2);
            encoder.setTexture_atIndex(Some(history_in), 3);
            encoder.setTexture_atIndex(Some(history_out), 4);
            encoder.setTexture_atIndex(Some(revolver_image.as_deref().unwrap_or(frame)), 5);
            encoder.setTexture_atIndex(Some(picture), 6);
            encoder.setBytes_length_atIndex(
                NonNull::from(&params).cast(),
                std::mem::size_of::<OverlayParams>(),
                0,
            );
            encoder.setBytes_length_atIndex(
                NonNull::from(&rects).cast(),
                std::mem::size_of_val(&rects),
                1,
            );
            encoder.setBytes_length_atIndex(
                NonNull::from(&wand).cast(),
                std::mem::size_of::<OverlayWand>(),
                2,
            );
            encoder.setBytes_length_atIndex(
                NonNull::from(&minimap).cast(),
                std::mem::size_of::<Minimap>(),
                3,
            );
            encoder.setTexture_atIndex(Some(map), 7);
        }
        dispatch_grid(&encoder, output_px);
        encoder.endEncoding();
        let target: Retained<ProtocolObject<dyn MTLTexture>> = target.clone();
        self.blur_histories = self.blur_histories.take().map(|(a, b)| (b, a));
        Ok(target)
    }

    /// `render_to_layer` draws one frame straight into the next drawable of a window's
    /// `CAMetalLayer`, upscaled with MetalFX to the drawable's size, with `overlay` drawn over it if
    /// given, and presents it. It returns false if the layer had no drawable to give.
    ///
    /// Example: `renderer.render_to_layer(&data, &layer, None)?;`
    pub fn render_to_layer(
        &mut self,
        data: FrameData,
        layer: &CAMetalLayer,
        overlay: Option<&Overlay>,
    ) -> Result<bool> {
        let Some(drawable) = layer.nextDrawable() else {
            return Ok(false);
        };
        let target = drawable.texture();
        let output_px: (usize, usize) = (target.width(), target.height());
        let (command_buffer, frame_texture) = self.encode_frame(data, output_px)?;
        let texture: Retained<ProtocolObject<dyn MTLTexture>> = match overlay {
            Some(overlay) => {
                self.encode_overlay(&command_buffer, &frame_texture, overlay, output_px)?
            }
            None => frame_texture,
        };
        let blit = command_buffer
            .blitCommandEncoder()
            .ok_or("Metal: no blit encoder")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY BETWEEN TEXTURES OF DIFFERENT SIZES WILL HAPPEN
        unsafe { blit.copyFromTexture_toTexture(&texture, &target) };
        blit.endEncoding();
        command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
        self.finish(&command_buffer)?;
        Ok(true)
    }
}
