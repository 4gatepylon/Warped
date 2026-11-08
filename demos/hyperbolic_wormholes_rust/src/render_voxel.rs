//! `render_voxel.rs` draws voxel maps on the GPU with `render_voxel.metal`: it packs a
//! `VoxelWorld` (with any avatars), the laser beams and the dust of shots into the ground
//! (`dust.rs`) into buffers each frame and traces them into the render texture that
//! `render_metal.rs` upscales and presents, every pixel or with the house's adaptive sampling
//! (`render_metal.rs`'s `encode_adaptive_sampling`).
//!
//! Interface: `VoxelUniforms`, `VoxelFrame::Build`, `VoxelGpu`.
//!
//! Example: `let frame = VoxelFrame::Build(&world, camera, &beams, spot, &[], (1280, 800));`

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

use crate::dust::DustCloud;
use crate::geometry::{AXIS_W, AXIS_Z, get_distance, invert_lorentz};
use crate::render_metal::{
    AdaptivePipelines, Pipeline, REFINE_GROUP_SIZE, SampleBuffers, VOXEL_BYTES_PER_RAY_END,
    encode_adaptive_sampling, make_pipeline,
};
use crate::terrain::MoundGrid;
use crate::voxel_clearance::{ClearanceGrid, ClearanceGrids, STORED_M_PER_UNIT};
use crate::voxel_textures::SurfaceTextures;
use crate::voxel_world::{BeamSegment, MAX_MATERIAL_COUNT, VoxelWorld, clip_beam_to_map};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SHADER_SOURCE: &str = include_str!("render_voxel.metal");
const KERNEL_NAME: &str = "trace_voxel_pixels";
const UNIFORMS_BUFFER_INDEX: usize = 16;
const SCENE_BUFFER_COUNT: usize = 10;
const MEANS_BUFFER_INDEX: usize = 27;
const DUST_BUFFER_INDICES: [usize; 2] = [28, 29];
const NO_MOUNDS: [f32; 1] = [0.0];
const FIRST_SURFACE_TEXTURE_INDEX: usize = 4;
const BLOOM_LEVELS: usize = 5;
const FIRST_BLOOM_TEXTURE_INDEX: usize = 7;
const THREADGROUP_SIDE: usize = 8;
const HALF_FOV_RAD: f64 = 37.5 * std::f64::consts::PI / 180.0;
const SPOT_BRIGHTNESS: f32 = 4.0;
const BEAM_BRIGHTNESS: f32 = 1.0;
const MIN_BUFFER_BYTES: usize = 256;
const BYTES_PER_FLOAT4: usize = 16;
const FLOAT4S_PER_DUST_BURST: usize = 7;
const FLOAT4S_PER_DUST_PARTICLE: usize = 3;

/// `VoxelUniforms` is the shader's `VoxelUniforms`.
#[repr(C)]
#[derive(Clone, Copy)]
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
    pub has_terrain: i32,
    pub terrain_seed: u32,
    pub sun: [f32; 4],
    pub planet: [f32; 4],
    pub planet_e1: [f32; 4],
    pub planet_e2: [f32; 4],
    pub spot: [f32; 4],
    pub spot_color: [f32; 4],
    pub regions: [f32; 4],
    pub terrain: [f32; 4],
    pub relief: [f32; 4],
    pub mound_grid: [f32; 4],
    pub flashlight_position: [f32; 4],
    pub flashlight_direction: [f32; 4],
    pub flashlight: [f32; 4],
    pub dust_counts: [i32; 4],
}

const _: () = assert!(
    std::mem::size_of::<VoxelUniforms>() <= 4096,
    "VoxelUniforms fits setBytes"
);

const _: () = assert!(
    std::mem::size_of::<VoxelUniforms>().is_multiple_of(BYTES_PER_FLOAT4),
    "VoxelUniforms ends on a float4 boundary, as Metal pads it"
);

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

/// `GpuShape` is the shader's `Shape`: centre and kind, half sizes and rounding, material and axis.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuShape {
    centre_kind: [f32; 4],
    half_round: [f32; 4],
    info: [i32; 4],
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

/// `GpuDustBurst` is the shader's `DustBurst`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuDustBurst {
    rows: [[f32; 4]; 4],
    sun: [f32; 4],
    range: [i32; 4],
    bound: [f32; 4],
}

/// `GpuDustParticle` is the shader's `DustParticle`.
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuDustParticle {
    position_radius: [f32; 4],
    color_opacity: [f32; 4],
    shape: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<GpuDustBurst>() == FLOAT4S_PER_DUST_BURST * BYTES_PER_FLOAT4
        && std::mem::size_of::<GpuDustParticle>() == FLOAT4S_PER_DUST_PARTICLE * BYTES_PER_FLOAT4,
    "GpuDustBurst and GpuDustParticle match the shader's DustBurst and DustParticle"
);

/// `VoxelFrame` is one frame's packed voxel scene: the uniforms and every buffer's contents.
pub struct VoxelFrame {
    pub uniforms: VoxelUniforms,
    structures: Vec<GpuStructure>,
    shapes: Vec<GpuShape>,
    warps: Vec<GpuWarp>,
    lists: Vec<[f32; 4]>,
    lights: Vec<GpuLight>,
    palette: Vec<[f32; 4]>,
    beams: Vec<GpuBeam>,
    clearance: Arc<ClearanceGrids>,
    surfaces: Vec<[f32; 4]>,
    mound_grid: Option<Arc<MoundGrid>>,
    dust_bursts: Vec<GpuDustBurst>,
    dust_particles: Vec<GpuDustParticle>,
}

/// `get_mound_grid` gives a terrain's mound height grid, if it has mounds.
///
/// Example: `let grid = get_mound_grid(&world.terrain);`
fn get_mound_grid(terrain: &Option<crate::terrain::Terrain>) -> Option<&Arc<MoundGrid>> {
    terrain.as_ref().and_then(|t| t.mound_grid.as_ref())
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

/// `pack_dust` gives the shader's dust bursts and their particles for `clouds`: each burst's
/// inverse frame rows, its sun, where its particles start and how many there are, and how far its
/// farthest particle centre is (and its largest radius), in meters.
///
/// Example: `let (bursts, particles) = pack_dust(&clouds);`
fn pack_dust(clouds: &[DustCloud]) -> (Vec<GpuDustBurst>, Vec<GpuDustParticle>) {
    let mut particles: Vec<GpuDustParticle> = Vec::new();
    let bursts: Vec<GpuDustBurst> = clouds
        .iter()
        .map(|cloud| {
            let first: usize = particles.len();
            particles.extend(cloud.particles.iter().map(|p| GpuDustParticle {
                position_radius: p.position_m.extend(p.radius_m).as_vec4().to_array(),
                color_opacity: [p.albedo[0], p.albedo[1], p.albedo[2], p.opacity_frac as f32],
                shape: [p.softness_frac as f32, 0.0, 0.0, 0.0],
            }));
            let inverse: DMat4 = invert_lorentz(cloud.frame);
            let farthest_m: f64 = cloud
                .particles
                .iter()
                .map(|p| p.position_m.length())
                .fold(0.0, f64::max);
            let largest_m: f64 = cloud
                .particles
                .iter()
                .map(|p| p.radius_m)
                .fold(0.0, f64::max);
            GpuDustBurst {
                rows: [0, 1, 2, 3].map(|i| to_f32(inverse.row(i))),
                sun: cloud.sun.extend(cloud.sun_frac).as_vec4().to_array(),
                range: [first as i32, cloud.particles.len() as i32, 0, 0],
                bound: [farthest_m as f32, largest_m as f32, 0.0, 0.0],
            }
        })
        .collect();
    (bursts, particles)
}

/// `to_columns` converts a double matrix to float columns.
///
/// Example: `let m = to_columns(frame);`
fn to_columns(m: DMat4) -> [[f32; 4]; 4] {
    [0, 1, 2, 3].map(|c| to_f32(m.col(c)))
}

impl VoxelFrame {
    /// `Build` packs `world` seen from `camera` at a render size, with laser segments of each team
    /// (`beams`: segments and team, 0 blue or 1 red; drawn as far as `clip_beam_to_map` allows),
    /// an optional laser spot and the dust of shots into the ground.
    ///
    /// Example: `let frame = VoxelFrame::Build(&world, camera, &beams, spot, &clouds, (1280, 800));`
    #[allow(non_snake_case)]
    pub fn Build(
        world: &VoxelWorld,
        camera: DMat4,
        beams: &[(Vec<BeamSegment>, u32)],
        spot: Option<(DVec4, u32)>,
        dust: &[DustCloud],
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
                    bound: [s.radius.cosh() as f32, 0.0, 0.0, 0.0],
                    size: [s.first_shape as i32, s.shape_count as i32, 0, 0],
                    scale: [r as f32, 0.0, 0.0, 0.0],
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
                clip_beam_to_map(segments)
                    .into_iter()
                    .map(move |s| GpuBeam {
                        a: to_f32(s.a),
                        b: to_f32(s.b),
                        info: [s.length as f32, 0.0, BEAM_BRIGHTNESS, *team as f32],
                    })
            })
            .collect();
        let (dust_bursts, dust_particles) = pack_dust(dust);
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
            has_terrain: i32::from(world.terrain.is_some()),
            terrain_seed: world.terrain.as_ref().map_or(0, |t| t.seed),
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
            terrain: world.terrain.as_ref().map_or([0.0; 4], |t| {
                [t.rim_m[0], t.rim_m[1], t.wall_m, t.rim_height_m].map(|x| x as f32)
            }),
            relief: world.terrain.as_ref().map_or([0.0; 4], |t| {
                [
                    t.relief_m,
                    t.wavelength_m,
                    t.get_max_height_m(),
                    t.mound_grid.as_ref().map_or(0.0, |g| g.size[1] as f64),
                ]
                .map(|x| x as f32)
            }),
            mound_grid: get_mound_grid(&world.terrain).map_or([0.0; 4], |g| {
                [
                    g.origin_m[0],
                    g.origin_m[1],
                    1.0 / g.cell_m,
                    g.size[0] as f64,
                ]
                .map(|x| x as f32)
            }),
            flashlight_position: world
                .flashlight_eye
                .map_or([0.0, 0.0, 0.0, 1.0], |eye| to_f32(eye.col(AXIS_W))),
            flashlight_direction: world
                .flashlight_eye
                .map_or([0.0; 4], |eye| to_f32(-eye.col(AXIS_Z))),
            flashlight: world.flashlight_eye.map_or([0.0; 4], |_| {
                let light = &world.settings.flashlight;
                [
                    light.cone_half_angle_rad.cos() as f32,
                    (light.cone_half_angle_rad + light.soft_edge_rad).cos() as f32,
                    light.range_m as f32,
                    light.brightness_frac as f32,
                ]
            }),
            dust_counts: [dust_bursts.len() as i32, dust_particles.len() as i32, 0, 0],
        };
        assert_eq!(
            world.palette.len(),
            MAX_MATERIAL_COUNT,
            "VoxelFrame: the palette has 256 entries"
        );
        VoxelFrame {
            uniforms,
            structures,
            shapes: world
                .shapes
                .iter()
                .map(|sh| GpuShape {
                    centre_kind: [
                        sh.centre.x as f32,
                        sh.centre.y as f32,
                        sh.centre.z as f32,
                        sh.kind as i32 as f32,
                    ],
                    half_round: [
                        sh.half.x as f32,
                        sh.half.y as f32,
                        sh.half.z as f32,
                        sh.round_m as f32,
                    ],
                    info: [i32::from(sh.material), sh.axis as i32, 0, 0],
                })
                .collect(),
            warps,
            lists,
            lights,
            palette: world.palette.clone(),
            beams: gpu_beams,
            clearance: Arc::clone(&world.clearance),
            surfaces: world.surfaces.clone(),
            mound_grid: get_mound_grid(&world.terrain).cloned(),
            dust_bursts,
            dust_particles,
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
            buffers: (0..SCENE_BUFFER_COUNT + DUST_BUFFER_INDICES.len())
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
            as_bytes(&frame.shapes),
            as_bytes(&frame.warps),
            as_bytes(&frame.lists),
            as_bytes(&frame.lights),
            as_bytes(&frame.palette),
            as_bytes(&frame.beams),
            &frame.clearance.values,
            as_bytes(&frame.surfaces),
            frame
                .mound_grid
                .as_ref()
                .map_or(as_bytes(&NO_MOUNDS), |g| as_bytes(&g.heights)),
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
        let dust: [&[u8]; DUST_BUFFER_INDICES.len()] = [
            as_bytes(&frame.dust_bursts),
            as_bytes(&frame.dust_particles),
        ];
        for (k, bytes) in dust.iter().enumerate() {
            let buffer: &ProtocolObject<dyn MTLBuffer> =
                self.buffers[SCENE_BUFFER_COUNT + k].write(device, bytes)?;
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
            unsafe { encoder.setBuffer_offset_atIndex(Some(buffer), 0, DUST_BUFFER_INDICES[k]) };
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
