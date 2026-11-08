//! `render_voxel.rs` draws voxel maps on the GPU with `render_voxel.metal`: it packs a
//! `VoxelWorld` (with any avatars) and the laser beams into buffers each frame and traces them
//! into the render texture that `render_metal.rs` upscales and presents, every pixel or with the
//! house's adaptive sampling (`render_metal.rs`'s `encode_adaptive_sampling`).
//!
//! Interface: `VoxelUniforms`, `VoxelFrame::Build`, `VoxelGpu`.
//!
//! Example: `let frame = VoxelFrame::Build(&world, camera, &shots, (1280, 800));`

use std::error::Error;
use std::ptr::NonNull;
use std::sync::Arc;

use glam::{DMat4, DVec4};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBuffer, MTLComputeCommandEncoder, MTLDevice, MTLLibrary, MTLPixelFormat, MTLResourceOptions,
    MTLSize, MTLStorageMode, MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

use crate::geometry::get_distance;
use crate::render_metal::{
    AdaptivePipelines, Pipeline, REFINE_GROUP_SIZE, SampleBuffers, VOXEL_BYTES_PER_RAY_END,
    encode_adaptive_sampling, make_pipeline,
};
use crate::voxel_clearance::{ClearanceGrid, ClearanceGrids, STORED_M_PER_UNIT};
use crate::voxel_textures::SurfaceTextures;
use crate::voxel_world::{BeamSegment, MAX_MATERIAL_COUNT, VoxelWorld};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SHADER_SOURCE: &str = include_str!("render_voxel.metal");
const KERNEL_NAME: &str = "trace_voxel_pixels";
const UNIFORMS_BUFFER_INDEX: usize = 16;
const SCENE_BUFFER_COUNT: usize = 9;
const MEANS_BUFFER_INDEX: usize = 26;
const FIRST_SURFACE_TEXTURE_INDEX: usize = 4;
const BLOOM_LEVELS: usize = 5;
const FIRST_BLOOM_TEXTURE_INDEX: usize = 7;
const THREADGROUP_SIDE: usize = 8;
const HALF_FOV_RAD: f64 = 37.5 * std::f64::consts::PI / 180.0;
const SPOT_BRIGHTNESS: f32 = 4.0;
const BEAM_BRIGHTNESS: f32 = 1.0;
const MIN_BUFFER_BYTES: usize = 256;

/// `VoxelUniforms` is the shader's `VoxelUniforms`.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VoxelUniforms {
    pub camera: [[f32; 4]; 4],
    pub tan_half_fov: f32,
    pub aspect: f32,
    pub pixel_angle_rad: f32,
    pub curvature_radius_m: f32,
    pub structure_count: i32,
    pub warp_count: i32,
    pub light_count: i32,
    pub beam_count: i32,
    pub width_px: i32,
    pub height_px: i32,
    pub mat_glass: i32,
    pub mat_regolith: i32,
    pub mat_dark_regolith: i32,
    pub mat_basalt: i32,
    pub mat_solar: i32,
    pub mat_hazard: i32,
    pub mat_pad: i32,
    pub mat_hull: i32,
    pub mat_gold_foil: i32,
    pub mat_console_first: i32,
    pub mat_console_count: i32,
    pub has_regions: i32,
    pub padding: [i32; 2],
    pub sun: [f32; 4],
    pub planet: [f32; 4],
    pub planet_e1: [f32; 4],
    pub planet_e2: [f32; 4],
    pub spot: [f32; 4],
    pub spot_color: [f32; 4],
    pub regions: [f32; 4],
}

/// `GpuStructure` is the shader's `VoxelStructure`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuStructure {
    rows: [[f32; 4]; 4],
    centre: [f32; 4],
    bound: [f32; 4],
    size: [i32; 4],
    scale: [f32; 4],
}

/// `GpuWarp` is the shader's `VoxelWarp`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuWarp {
    inverse: [[f32; 4]; 4],
    forward: [[f32; 4]; 4],
    centre: [f32; 4],
    params: [f32; 4],
    list: [i32; 4],
    grid: [f32; 4],
}

/// `GpuLight` is the shader's `VoxelLight`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuLight {
    position: [f32; 4],
    color_radius: [f32; 4],
}

/// `GpuBeam` is the shader's `BeamSegment`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuBeam {
    a: [f32; 4],
    b: [f32; 4],
    info: [f32; 4],
}

/// `VoxelFrame` is one frame's packed voxel scene: the uniforms and every buffer's contents.
pub struct VoxelFrame {
    pub uniforms: VoxelUniforms,
    structures: Vec<GpuStructure>,
    voxels: Vec<u8>,
    warps: Vec<GpuWarp>,
    lists: Vec<[f32; 4]>,
    lights: Vec<GpuLight>,
    palette: Vec<[f32; 4]>,
    beams: Vec<GpuBeam>,
    clearance: Arc<ClearanceGrids>,
    surfaces: Vec<[f32; 4]>,
}

/// `to_f32` converts a double 4-vector to floats.
///
/// Example: `let v = to_f32(x);`
fn to_f32(v: DVec4) -> [f32; 4] {
    v.as_vec4().to_array()
}

/// `pack_grid` gives the shader's `VoxelWarp.grid` for a clearance grid: its half size, cells per
/// axis, cells per unit of the chart, and curvature radii per stored unit.
///
/// Example: `let grid = pack_grid(&world.clearance.grids[0], world.r);`
fn pack_grid(grid: &ClearanceGrid, r: f64) -> [f32; 4] {
    let n: f64 = grid.cell_count as f64;
    [
        grid.half_size as f32,
        n as f32,
        (n / (2.0 * grid.half_size)) as f32,
        (STORED_M_PER_UNIT / r) as f32,
    ]
}

/// `to_columns` converts a double matrix to float columns.
///
/// Example: `let m = to_columns(frame);`
fn to_columns(m: DMat4) -> [[f32; 4]; 4] {
    [0, 1, 2, 3].map(|c| to_f32(m.col(c)))
}

impl VoxelFrame {
    /// `Build` packs `world` seen from `camera` at a render size, with laser segments of each team
    /// (`beams`: segments and team, 0 blue or 1 red) and an optional laser spot.
    ///
    /// Example: `let frame = VoxelFrame::Build(&world, camera, &beams, spot, (1280, 800));`
    #[allow(non_snake_case)]
    pub fn Build(
        world: &VoxelWorld,
        camera: DMat4,
        beams: &[(Vec<BeamSegment>, u32)],
        spot: Option<(DVec4, u32)>,
        size_px: (u32, u32),
    ) -> VoxelFrame {
        let r: f64 = world.r;
        let structures: Vec<GpuStructure> = world
            .structures
            .iter()
            .map(|s| {
                let rows: [[f32; 4]; 4] = [0, 1, 2, 3].map(|i| to_f32(s.inverse.row(i)));
                GpuStructure {
                    rows,
                    centre: to_f32(s.centre),
                    bound: [
                        s.radius.cosh() as f32,
                        s.origin[0] as f32,
                        s.origin[1] as f32,
                        s.origin[2] as f32,
                    ],
                    size: [
                        s.size[0] as i32,
                        s.size[1] as i32,
                        s.size[2] as i32,
                        s.offset as i32,
                    ],
                    scale: [(r / s.cell_m) as f32, 0.0, 0.0, 0.0],
                }
            })
            .collect();
        let mut lists: Vec<[f32; 4]> = Vec::new();
        let warps: Vec<GpuWarp> = world
            .warps
            .iter()
            .zip(&world.clearance.grids)
            .map(|(w, grid)| {
                let first: usize = lists.len();
                lists.extend(w.list.iter().map(|&k| {
                    let structure = &world.structures[k];
                    let distance: f64 = get_distance(structure.centre, w.centre);
                    [
                        k as f32,
                        (distance - structure.radius).max(0.0) as f32,
                        (distance + structure.radius) as f32,
                        0.0,
                    ]
                }));
                GpuWarp {
                    inverse: to_columns(w.inverse),
                    forward: to_columns(w.frame),
                    centre: to_f32(w.centre),
                    params: [
                        f32::from(u8::from(w.is_mouth)),
                        w.m as f32,
                        w.reach as f32,
                        if w.is_mouth { w.partner as f32 } else { -1.0 },
                    ],
                    list: [
                        first as i32,
                        w.list.len() as i32,
                        grid.static_entry_count as i32,
                        grid.offset as i32,
                    ],
                    grid: pack_grid(grid, r),
                }
            })
            .collect();
        let lights: Vec<GpuLight> = world
            .lights
            .iter()
            .map(|&(x, color, radius_m)| GpuLight {
                position: to_f32(x),
                color_radius: [color[0], color[1], color[2], radius_m as f32],
            })
            .collect();
        let gpu_beams: Vec<GpuBeam> = beams
            .iter()
            .flat_map(|(segments, team)| {
                segments.iter().map(move |s| GpuBeam {
                    a: to_f32(s.a),
                    b: to_f32(s.b),
                    info: [s.length as f32, 0.0, BEAM_BRIGHTNESS, *team as f32],
                })
            })
            .collect();
        let tan_half_fov: f64 = HALF_FOV_RAD.tan();
        let (planet, e1, e2) = match world.planet {
            Some((d, radius_rad)) => {
                let e1 = d.cross(glam::DVec3::Y).normalize();
                ((d, radius_rad.sin()), e1, e1.cross(d))
            }
            None => ((glam::DVec3::Y, 0.0), glam::DVec3::X, glam::DVec3::Z),
        };
        let special = world.special;
        let team_colors: [[f32; 3]; 2] = [[0.30, 0.62, 1.0], [1.0, 0.16, 0.08]];
        let uniforms: VoxelUniforms = VoxelUniforms {
            camera: to_columns(camera),
            tan_half_fov: tan_half_fov as f32,
            aspect: size_px.0 as f32 / size_px.1 as f32,
            pixel_angle_rad: (2.0 * tan_half_fov / f64::from(size_px.1)) as f32,
            curvature_radius_m: r as f32,
            structure_count: structures.len() as i32,
            warp_count: warps.len() as i32,
            light_count: lights.len() as i32,
            beam_count: gpu_beams.len() as i32,
            width_px: size_px.0 as i32,
            height_px: size_px.1 as i32,
            mat_glass: special.glass,
            mat_regolith: special.regolith,
            mat_dark_regolith: special.dark_regolith,
            mat_basalt: special.basalt,
            mat_solar: special.solar,
            mat_hazard: special.hazard,
            mat_pad: special.pad,
            mat_hull: special.hull,
            mat_gold_foil: special.gold_foil,
            mat_console_first: special.console_first,
            mat_console_count: special.console_count,
            has_regions: i32::from(world.regions.is_some()),
            padding: [0; 2],
            sun: world.sun.as_vec3().extend(0.0).to_array(),
            planet: planet.0.as_vec3().extend(planet.1 as f32).to_array(),
            planet_e1: e1.as_vec3().extend(0.0).to_array(),
            planet_e2: e2.as_vec3().extend(0.0).to_array(),
            spot: spot.map_or([0.0, 0.0, 0.0, 1.0], |(x, _)| to_f32(x)),
            spot_color: spot.map_or([0.0; 4], |(_, team)| {
                let c: [f32; 3] = team_colors[team as usize % 2];
                [c[0], c[1], c[2], SPOT_BRIGHTNESS]
            }),
            regions: world.regions.map_or([0.0; 4], |[edge, u_m, v_m]| {
                [edge as f32, u_m as f32, v_m as f32, 0.0]
            }),
        };
        assert_eq!(
            world.palette.len(),
            MAX_MATERIAL_COUNT,
            "VoxelFrame: the palette has 256 entries"
        );
        VoxelFrame {
            uniforms,
            structures,
            voxels: world.voxels.clone(),
            warps,
            lists,
            lights,
            palette: world.palette.clone(),
            beams: gpu_beams,
            clearance: Arc::clone(&world.clearance),
            surfaces: world.surfaces.clone(),
        }
    }

    /// `get_render_size` gives the frame's render size in pixels.
    ///
    /// Example: `let (w, h) = frame.get_render_size();`
    pub fn get_render_size(&self) -> (usize, usize) {
        (
            self.uniforms.width_px as usize,
            self.uniforms.height_px as usize,
        )
    }
}

/// `GrowableBuffer` is a shared GPU buffer rewritten each frame, grown when its contents outgrow it.
struct GrowableBuffer {
    buffer: Option<Retained<ProtocolObject<dyn MTLBuffer>>>,
}

impl GrowableBuffer {
    /// `write` copies `bytes` into the buffer (making a bigger one first if needed) and returns it.
    ///
    /// Example: `let buffer = self.voxels.write(device, &frame.voxels)?;`
    fn write(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        bytes: &[u8],
    ) -> Result<&ProtocolObject<dyn MTLBuffer>> {
        let needed: usize = bytes.len().max(MIN_BUFFER_BYTES);
        if self.buffer.as_ref().is_none_or(|b| b.length() < needed) {
            self.buffer = Some(
                device
                    .newBufferWithLength_options(
                        needed.next_power_of_two(),
                        MTLResourceOptions::StorageModeShared,
                    )
                    .ok_or("Metal: no memory for a voxel buffer")?,
            );
        }
        let buffer: &ProtocolObject<dyn MTLBuffer> =
            self.buffer.as_ref().expect("the buffer was just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A WRITE PAST THE BUFFER WILL HAPPEN
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                buffer.contents().as_ptr().cast::<u8>(),
                bytes.len(),
            )
        };
        Ok(buffer)
    }
}

/// `as_bytes` views a slice of plain values as bytes.
///
/// Example: `let bytes = as_bytes(&frame.lights);`
fn as_bytes<T: Copy>(values: &[T]) -> &[u8] {
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN READING PAST THE SLICE WILL HAPPEN
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

/// `Texture` is a Metal texture.
type Texture = Retained<ProtocolObject<dyn MTLTexture>>;

/// `PostTextures` is the HDR image the rays are traced into, and its bloom chain (each level half
/// the last).
struct PostTextures {
    hdr: Texture,
    bloom: Vec<Texture>,
}

/// `make_hdr_texture` makes a private half-float texture that kernels read and write.
///
/// Example: `let hdr = make_hdr_texture(device, (1280, 800))?;`
fn make_hdr_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    size: (usize, usize),
) -> Result<Texture> {
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TEXTURE OF THE WRONG FORMAT OR SIZE WILL HAPPEN
    let descriptor: Retained<MTLTextureDescriptor> = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            MTLPixelFormat::RGBA16Float,
            size.0.max(1),
            size.1.max(1),
            false,
        )
    };
    descriptor.setUsage(MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite);
    descriptor.setStorageMode(MTLStorageMode::Private);
    Ok(device
        .newTextureWithDescriptor(&descriptor)
        .ok_or("Metal: no memory for the HDR texture")?)
}

/// `dispatch` runs a kernel over a size, one thread per pixel.
///
/// Example: `dispatch(encoder, &self.tonemap, (1280, 800));`
fn dispatch(
    encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
    pipeline: &Pipeline,
    size: (usize, usize),
) {
    encoder.setComputePipelineState(pipeline);
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

/// `VoxelGpu` is the voxel renderer's pipelines (every pixel, adaptive sampling's and the
/// post-processing's), buffers, surface textures and HDR textures.
pub struct VoxelGpu {
    trace_pixels: Pipeline,
    adaptive: AdaptivePipelines,
    bloom_down: Pipeline,
    tonemap: Pipeline,
    buffers: Vec<GrowableBuffer>,
    textures: SurfaceTextures,
    post: Option<PostTextures>,
}

impl VoxelGpu {
    /// `New` compiles `render_voxel.metal` and loads the surface textures on `device`.
    ///
    /// Example: `let gpu = VoxelGpu::New(&device)?;`
    #[allow(non_snake_case)]
    pub fn New(device: &ProtocolObject<dyn MTLDevice>) -> Result<VoxelGpu> {
        let source: String = SHADER_SOURCE
            .replace("__REFINE_GROUP_SIZE__", &REFINE_GROUP_SIZE.to_string())
            .replace(
                "__BYTES_PER_RAY_END__",
                &VOXEL_BYTES_PER_RAY_END.to_string(),
            )
            .replace("__BLOOM_LEVELS__", &BLOOM_LEVELS.to_string());
        let library: Retained<ProtocolObject<dyn MTLLibrary>> = device
            .newLibraryWithSource_options_error(&NSString::from_str(&source), None)
            .map_err(|error| {
                format!(
                    "Metal: the voxel shader didn't compile: {}",
                    error.localizedDescription()
                )
            })?;
        Ok(VoxelGpu {
            trace_pixels: make_pipeline(device, &library, KERNEL_NAME)?,
            adaptive: AdaptivePipelines::FromLibrary(device, &library)?,
            bloom_down: make_pipeline(device, &library, "bloom_down")?,
            tonemap: make_pipeline(device, &library, "tonemap_voxel")?,
            buffers: (0..SCENE_BUFFER_COUNT)
                .map(|_| GrowableBuffer { buffer: None })
                .collect(),
            textures: SurfaceTextures::Load(device)?,
            post: None,
        })
    }

    /// `encode` uploads a frame and traces it into the HDR texture (with `samples`, by adaptive
    /// sampling into those buffers, otherwise one ray per pixel), then blooms, grades and tone maps
    /// it into `target`.
    ///
    /// Example: `gpu.encode(&device, &encoder, &frame, None, &render_texture)?;`
    pub(crate) fn encode(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>,
        frame: &VoxelFrame,
        samples: Option<&SampleBuffers>,
        target: &ProtocolObject<dyn MTLTexture>,
    ) -> Result<()> {
        let contents: [&[u8]; SCENE_BUFFER_COUNT] = [
            as_bytes(&frame.structures),
            &frame.voxels,
            as_bytes(&frame.warps),
            as_bytes(&frame.lists),
            as_bytes(&frame.lights),
            as_bytes(&frame.palette),
            as_bytes(&frame.beams),
            &frame.clearance.values,
            as_bytes(&frame.surfaces),
        ];
        let means: &[[f32; 4]] = &self.textures.means;
        let maps = [
            &self.textures.albedo,
            &self.textures.normal,
            &self.textures.arm,
        ];
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG TEXTURES WILL HAPPEN
        unsafe {
            encoder.setBytes_length_atIndex(
                NonNull::from(means).cast(),
                std::mem::size_of_val(means),
                MEANS_BUFFER_INDEX,
            );
            for (k, map) in maps.iter().enumerate() {
                encoder.setTexture_atIndex(Some(map), FIRST_SURFACE_TEXTURE_INDEX + k);
            }
        }
        let uniforms: NonNull<VoxelUniforms> = NonNull::from(&frame.uniforms);
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            encoder.setBytes_length_atIndex(
                uniforms.cast(),
                std::mem::size_of::<VoxelUniforms>(),
                UNIFORMS_BUFFER_INDEX,
            )
        };
        for (k, bytes) in contents.iter().enumerate() {
            let buffer: &ProtocolObject<dyn MTLBuffer> = self.buffers[k].write(device, bytes)?;
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
            unsafe {
                encoder.setBuffer_offset_atIndex(Some(buffer), 0, UNIFORMS_BUFFER_INDEX + k + 1)
            };
        }
        let (width, height) = frame.get_render_size();
        if self
            .post
            .as_ref()
            .is_none_or(|p| (p.hdr.width(), p.hdr.height()) != (width, height))
        {
            self.post = Some(PostTextures {
                hdr: make_hdr_texture(device, (width, height))?,
                bloom: (1..=BLOOM_LEVELS)
                    .map(|k| make_hdr_texture(device, (width >> k, height >> k)))
                    .collect::<Result<_>>()?,
            });
        }
        let post: &PostTextures = self.post.as_ref().expect("the HDR textures were just made");
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER WRITING THE WRONG TEXTURE WILL HAPPEN
        unsafe { encoder.setTexture_atIndex(Some(&post.hdr), 0) };
        match samples {
            Some(buffers) => encode_adaptive_sampling(
                encoder,
                &self.adaptive,
                buffers,
                VOXEL_BYTES_PER_RAY_END,
                (width, height),
            ),
            None => dispatch(encoder, &self.trace_pixels, (width, height)),
        }
        for (k, level) in post.bloom.iter().enumerate() {
            let source: &Texture = if k == 0 {
                &post.hdr
            } else {
                &post.bloom[k - 1]
            };
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN BLOOM FROM THE WRONG LEVEL WILL HAPPEN
            unsafe {
                encoder.setTexture_atIndex(Some(source), 1);
                encoder.setTexture_atIndex(Some(level), 2);
                encoder.setTexture_atIndex(Some(level), FIRST_BLOOM_TEXTURE_INDEX + k);
            }
            dispatch(encoder, &self.bloom_down, (level.width(), level.height()));
        }
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN TONE MAPPING THE WRONG TEXTURE WILL HAPPEN
        unsafe {
            encoder.setTexture_atIndex(Some(&post.hdr), 1);
            encoder.setTexture_atIndex(Some(target), 0);
        }
        dispatch(encoder, &self.tonemap, (width, height));
        Ok(())
    }
}
