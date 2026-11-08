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
use crate::geometry::{AXIS_W, AXIS_Y, AXIS_Z, get_distance, invert_lorentz};
use crate::mesh::MeshModel;
use crate::render_mesh::{
    FIRST_AVATAR_ROOT_CODE, GpuNode, GpuTriangle, MeshGpu, NO_MESH, get_first_materials,
    get_mesh_roots, pack_avatar_meshes,
};
use crate::render_metal::{
    AdaptivePipelines, Pipeline, REFINE_GROUP_SIZE, SampleBuffers, VOXEL_BYTES_PER_RAY_END,
    encode_adaptive_sampling, make_pipeline,
};
use crate::spaceships::{GpuSpaceship, SPACESHIP_COUNT, get_spaceships};
use crate::terrain::MoundGrid;
use crate::thrown_wormholes::{GREEN_COLOR, HIT_FLASH_COLOR, PURPLE_COLOR, WormholeColor};
use crate::viewmodel::HAND_LIGHT_BYTES;
use crate::voxel_clearance::{ClearanceGrid, ClearanceGrids, STORED_M_PER_UNIT};
use crate::voxel_textures::SurfaceTextures;
use crate::voxel_world::{
    BeamSegment, MAX_MATERIAL_COUNT, VoxelWarp, VoxelWorld, clip_beam_to_map,
};
use crate::zone_walls::{GpuZoneWalls, MAX_ZONE_WALL_COUNT};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const NO_CLEARANCE_GRID: ClearanceGrid = ClearanceGrid {
    offset: 0,
    cell_count: 0,
    half_size: 1.0,
    static_entry_count: 0,
};
const SHADER_SOURCE: &str = include_str!("render_voxel.metal");
const KERNEL_NAME: &str = "trace_voxel_pixels";
const NO_BODY_MATERIAL: f32 = -1.0;
const UNIFORMS_BUFFER_INDEX: usize = 16;
const SCENE_BUFFER_COUNT: usize = 10;
const MEANS_BUFFER_INDEX: usize = 27;
const DUST_BUFFER_INDICES: [usize; 2] = [29, 30];
const AVATAR_BUFFER_INDICES: [usize; 2] = [8, 10];
const HAND_LIGHT_BUFFER_INDEX: usize = 28;
const HAND_LIGHT_GRID: (usize, usize) = (1, 1);
const NO_MOUNDS: [f32; 1] = [0.0];
const FIRST_SURFACE_TEXTURE_INDEX: usize = 4;
const MILKY_WAY_TEXTURE_INDEX: usize = 3;
const MILKY_WAY_TEXTURE_PX: (usize, usize) = (4096, 1024);
const BAKE_TARGET_TEXTURE_INDEX: usize = 0;
const BLOOM_LEVELS: usize = 5;
const FIRST_BLOOM_TEXTURE_INDEX: usize = 7;
const THREADGROUP_SIDE: usize = 8;
const HALF_FOV_RAD: f64 = 37.5 * std::f64::consts::PI / 180.0;
const SPOT_BRIGHTNESS: f32 = 4.0;
const BEAM_BRIGHTNESS: f32 = 1.0;
const BEACON_BEAM_M: f64 = 400.0;
const RIFLE_BEAM_CODE: u32 = 2;
const BEACON_BEAM_CODE: u32 = 4;
const BEAM_SEED_PERIOD_MS: u64 = 1 << 24;
const MS_PER_S: f64 = 1000.0;
/// `FOG_DEPTHS` is how many of a fog's e-folding distances past its start a ray goes before it
/// ends in the fog (5: the fog hides all but 0.7% of what lies beyond).
const FOG_DEPTHS: f64 = 5.0;
/// `SAND_PERIOD_S` is how often the blowing sand's clock wraps, short enough to keep the shader's
/// float positions precise.
const SAND_PERIOD_S: f64 = 600.0;
/// `DEFAULT_SAND` is the blowing sand of a fog whose map gives no `atmosphere.sand`.
const DEFAULT_SAND: crate::map::Sand = crate::map::Sand {
    gust_frac: 0.3,
    streak_frac: 0.15,
    sheet_frac: 0.0,
};
const MIN_BUFFER_BYTES: usize = 256;
const BYTES_PER_FLOAT4: usize = 16;
const FLOAT4S_PER_DUST_BURST: usize = 7;
const FLOAT4S_PER_DUST_PARTICLE: usize = 4;

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
    pub spaceships: [GpuSpaceship; SPACESHIP_COUNT],
    pub dust_counts: [i32; 4],
    pub beam_speckle: [f32; 4],
    pub zone_walls: GpuZoneWalls,
    pub milky_way_pole: [f32; 4],
    pub milky_way_core: [f32; 4],
    pub wind: [f32; 4],
    pub atmosphere: [f32; 4],
    pub fog_color: [f32; 4],
    pub sand: [f32; 4],
    pub overhead: [f32; 4],
    pub laser_color: [f32; 4],
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
    lens: [f32; 4],
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
    orientation: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<GpuDustBurst>() == FLOAT4S_PER_DUST_BURST * BYTES_PER_FLOAT4
        && std::mem::size_of::<GpuDustParticle>() == FLOAT4S_PER_DUST_PARTICLE * BYTES_PER_FLOAT4,
    "GpuDustBurst and GpuDustParticle match the shader's DustBurst and DustParticle"
);

/// `LaserBeam` is a laser shot's beam to draw: its geodesic segments, the shooter's team (0 blue,
/// 1 red), whether the rifle fired it (drawn as a laser; the sniper's is a lightsaber blade), and
/// the share of its pixels that speckle each frame.
#[derive(Clone)]
pub struct LaserBeam {
    pub segments: Vec<BeamSegment>,
    pub team: u32,
    pub is_rifle: bool,
    pub speckle_frac: f64,
}

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
    meshes: Arc<Vec<MeshModel>>,
    avatar_triangles: Vec<GpuTriangle>,
    avatar_nodes: Vec<GpuNode>,
}

/// `get_beacon_beams` gives each beacon's beam: from the top of the model of each structure drawn
/// as a mesh whose first shape is a console, straight up for `BEACON_BEAM_M` (as far as
/// `clip_beam_to_map` allows), in its console's team colour and `BEACON_BEAM_CODE` (so the shader
/// draws it wider), bright while its console is on and gone once it is off.
///
/// Example: `let beams = get_beacon_beams(&world);`
fn get_beacon_beams(world: &VoxelWorld) -> Vec<GpuBeam> {
    let mut beams: Vec<GpuBeam> = Vec::new();
    for st in &world.structures {
        let Some(m) = st.mesh else { continue };
        let Some(console) = world
            .consoles
            .iter()
            .find(|c| st.shape_count > 0 && c.material == world.shapes[st.first_shape].material)
        else {
            continue;
        };
        let on_frac: f32 = if world.palette[usize::from(console.material)][3] > 0.0 {
            1.0
        } else {
            0.0
        };
        let frame: DMat4 = invert_lorentz(st.inverse);
        let top: f64 = f64::from(world.meshes[m].nodes[0].hi.y) / world.r;
        let (x, v) = (frame.col(AXIS_W), frame.col(AXIS_Y));
        let segment: BeamSegment = BeamSegment {
            a: top.cosh() * x + top.sinh() * v,
            b: top.sinh() * x + top.cosh() * v,
            length: BEACON_BEAM_M / world.r,
        };
        let code: u32 = console.team + BEACON_BEAM_CODE;
        beams.extend(
            clip_beam_to_map(&[segment], world.r)
                .into_iter()
                .map(|s| GpuBeam {
                    a: to_f32(s.a),
                    b: to_f32(s.b),
                    info: [s.length as f32, 0.0, BEAM_BRIGHTNESS * on_frac, code as f32],
                }),
        );
    }
    beams
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
                shape: [p.shape as i32 as f32, 0.0, 0.0, 0.0],
                orientation: p.orientation.as_quat().to_array(),
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

/// `format_rgb` writes a colour as the arguments of a Metal `float3`.
///
/// Example: `let text = format_rgb(PURPLE_COLOR);`
fn format_rgb([r, g, b]: [f32; 3]) -> String {
    format!("{r:?}, {g:?}, {b:?}")
}

/// `get_warp_kind` gives the shader's kind of a warp: 0 for a black hole, 1 for a map's wormhole
/// mouth, 2 and 3 for a thrown purple and green mouth, and 4 for a thrown mouth flashing red.
///
/// Example: `let kind = get_warp_kind(&world.warps[0]);`
fn get_warp_kind(warp: &VoxelWarp) -> f32 {
    match (warp.is_mouth, warp.color) {
        (false, _) => 0.0,
        (true, None) => 1.0,
        (true, Some(_)) if warp.is_flashing => 4.0,
        (true, Some(WormholeColor::Purple)) => 2.0,
        (true, Some(WormholeColor::Green)) => 3.0,
    }
}

/// `to_columns` converts a double matrix to float columns.
///
/// Example: `let m = to_columns(frame);`
fn to_columns(m: DMat4) -> [[f32; 4]; 4] {
    [0, 1, 2, 3].map(|c| to_f32(m.col(c)))
}

impl VoxelFrame {
    /// `Build` packs `world` seen from `camera` at a render size, with laser segments of each team
    /// (`beams`, each drawn in its gun's style as far as `clip_beam_to_map` allows), an optional
    /// laser spot and the dust of shots into the ground. The speckle picks its pixels anew each
    /// millisecond of `world.sky_time_s`, and their kinds in the shares of
    /// `world.settings.beam_speckle`.
    ///
    /// Example: `let frame = VoxelFrame::Build(&world, camera, &beams, spot, &clouds, (1280, 800));`
    #[allow(non_snake_case)]
    pub fn Build(
        world: &VoxelWorld,
        camera: DMat4,
        beams: &[LaserBeam],
        spot: Option<(DVec4, u32)>,
        dust: &[DustCloud],
        size_px: (u32, u32),
    ) -> VoxelFrame {
        let r: f64 = world.r;
        let mesh_roots: Vec<i32> = get_mesh_roots(&world.meshes);
        let body_first_material: i32 = get_first_materials(&world.meshes)[world.body_mesh];
        let (avatar_triangles, avatar_nodes, avatar_roots) =
            pack_avatar_meshes(&world.avatar_meshes, body_first_material);
        let structures: Vec<GpuStructure> = world
            .structures
            .iter()
            .map(|s| {
                let rows: [[f32; 4]; 4] = [0, 1, 2, 3].map(|i| to_f32(s.inverse.row(i)));
                GpuStructure {
                    rows,
                    centre: to_f32(s.centre),
                    bound: [s.radius.cosh() as f32, 0.0, 0.0, 0.0],
                    size: [
                        s.first_shape as i32,
                        s.shape_count as i32,
                        i32::from(s.is_dark),
                        match (s.mesh, s.avatar_mesh) {
                            (Some(m), _) => mesh_roots[m],
                            (None, Some(a)) => FIRST_AVATAR_ROOT_CODE - avatar_roots[a],
                            (None, None) => NO_MESH,
                        },
                    ],
                    scale: [
                        r as f32,
                        f32::from(u8::from(
                            s.shape_count > 0
                                && world
                                    .consoles
                                    .iter()
                                    .any(|c| c.material == world.shapes[s.first_shape].material),
                        )),
                        s.body_material.map_or(NO_BODY_MATERIAL, f32::from),
                        0.0,
                    ],
                }
            })
            .collect();
        let mut lists: Vec<[f32; 4]> = Vec::new();
        let warps: Vec<GpuWarp> = world
            .warps
            .iter()
            .enumerate()
            .map(|(k, w)| {
                let grid: &ClearanceGrid = match w.color {
                    Some(_) => &NO_CLEARANCE_GRID,
                    None => &world.clearance.grids[k],
                };
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
                        get_warp_kind(w),
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
                    lens: [w.lens_rad as f32, w.lens_core as f32, 0.0, 0.0],
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
        let mut gpu_beams: Vec<GpuBeam> = beams
            .iter()
            .flat_map(|beam| {
                let code: u32 = beam.team + RIFLE_BEAM_CODE * u32::from(beam.is_rifle);
                clip_beam_to_map(&beam.segments, world.r)
                    .into_iter()
                    .map(move |s| GpuBeam {
                        a: to_f32(s.a),
                        b: to_f32(s.b),
                        info: [
                            s.length as f32,
                            beam.speckle_frac as f32,
                            BEAM_BRIGHTNESS,
                            code as f32,
                        ],
                    })
            })
            .collect();
        gpu_beams.extend(get_beacon_beams(world));
        let (dust_bursts, dust_particles) = pack_dust(dust);
        let zone_walls: GpuZoneWalls = world.zone_walls.as_ref().map_or(
            GpuZoneWalls {
                walls: [[0.0; 4]; MAX_ZONE_WALL_COUNT],
                look: [0.0; 4],
                boundary: [0.0; 4],
                counts: [0; 4],
            },
            |walls| walls.to_gpu(r),
        );
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
                let c: [f32; 3] = world.laser_color.unwrap_or(team_colors[team as usize % 2]);
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
            spaceships: get_spaceships(world.sky_time_s),
            dust_counts: [dust_bursts.len() as i32, dust_particles.len() as i32, 0, 0],
            beam_speckle: [
                ((world.sky_time_s * MS_PER_S) as u64 % BEAM_SEED_PERIOD_MS) as f32,
                world.settings.beam_speckle.black_frac as f32,
                world.settings.beam_speckle.white_frac as f32,
                0.0,
            ],
            zone_walls,
            milky_way_pole: world.milky_way.map_or([0.0; 4], |(pole, _)| {
                pole.as_vec3()
                    .extend(world.milky_way_brightness_frac as f32)
                    .to_array()
            }),
            milky_way_core: world
                .milky_way
                .map_or([0.0; 4], |(_, core)| core.as_vec3().extend(0.0).to_array()),
            wind: world.wind.map_or([0.0; 4], |wind| {
                let n = wind.normal;
                [n.x as f32, n.y as f32, n.z as f32, wind.k as f32]
            }),
            atmosphere: world.fog.map_or([0.0; 4], |fog| {
                [
                    world.wind.map_or(0.0, |wind| wind.segment_s) as f32,
                    ((fog.start_m + FOG_DEPTHS * fog.distance_m) / r) as f32,
                    fog.start_m as f32,
                    fog.distance_m as f32,
                ]
            }),
            fog_color: world.fog.map_or([0.0; 4], |fog| {
                [
                    fog.color[0],
                    fog.color[1],
                    fog.color[2],
                    fog.max_frac as f32,
                ]
            }),
            sand: world.fog.map_or([0.0; 4], |_| {
                let sand: crate::map::Sand = world.sand.unwrap_or(DEFAULT_SAND);
                [
                    (world.sky_time_s % SAND_PERIOD_S) as f32,
                    sand.gust_frac,
                    sand.streak_frac,
                    sand.sheet_frac,
                ]
            }),
            overhead: [0.0; 4],
            laser_color: world
                .laser_color
                .map_or([0.0; 4], |[r, g, b]| [r, g, b, 1.0]),
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
            meshes: Arc::clone(&world.meshes),
            avatar_triangles,
            avatar_nodes,
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

/// `VoxelGpu` is the voxel renderer's pipelines (every pixel's, adaptive sampling's, the
/// post-processing's, the held gun's light probe and the Milky Way's bake), buffers, surface
/// textures, HDR textures, and the Milky Way's texture with whether it is baked yet.
pub struct VoxelGpu {
    trace_pixels: Pipeline,
    probe_hand_light: Pipeline,
    hand_light: Retained<ProtocolObject<dyn MTLBuffer>>,
    adaptive: AdaptivePipelines,
    bloom_down: Pipeline,
    tonemap: Pipeline,
    bake_milky_way: Pipeline,
    buffers: Vec<GrowableBuffer>,
    textures: SurfaceTextures,
    meshes: Option<(*const Vec<MeshModel>, MeshGpu)>,
    post: Option<PostTextures>,
    milky_way: Texture,
    is_milky_way_baked: bool,
}

impl VoxelGpu {
    /// `New` compiles `render_voxel.metal` on `device` and keeps the surface `textures`.
    ///
    /// Example: `let gpu = VoxelGpu::New(&device, SurfaceTextures::Load(&device)?)?;`
    #[allow(non_snake_case)]
    pub fn New(
        device: &ProtocolObject<dyn MTLDevice>,
        textures: SurfaceTextures,
    ) -> Result<VoxelGpu> {
        let source: String = SHADER_SOURCE
            .replace("__REFINE_GROUP_SIZE__", &REFINE_GROUP_SIZE.to_string())
            .replace(
                "__BYTES_PER_RAY_END__",
                &VOXEL_BYTES_PER_RAY_END.to_string(),
            )
            .replace("__BLOOM_LEVELS__", &BLOOM_LEVELS.to_string())
            .replace("__SPACESHIP_COUNT__", &SPACESHIP_COUNT.to_string())
            .replace("__MAX_ZONE_WALL_COUNT__", &MAX_ZONE_WALL_COUNT.to_string())
            .replace(
                "__BYTES_PER_VOXEL_UNIFORMS__",
                &std::mem::size_of::<VoxelUniforms>().to_string(),
            )
            .replace("__PURPLE_COLOR__", &format_rgb(PURPLE_COLOR))
            .replace("__GREEN_COLOR__", &format_rgb(GREEN_COLOR))
            .replace("__HIT_FLASH_COLOR__", &format_rgb(HIT_FLASH_COLOR));
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
            probe_hand_light: make_pipeline(device, &library, "probe_hand_light")?,
            hand_light: device
                .newBufferWithLength_options(
                    HAND_LIGHT_BYTES,
                    MTLResourceOptions::StorageModePrivate,
                )
                .ok_or("Metal: no memory for the hand light")?,
            adaptive: AdaptivePipelines::FromLibrary(device, &library)?,
            bloom_down: make_pipeline(device, &library, "bloom_down")?,
            tonemap: make_pipeline(device, &library, "tonemap_voxel")?,
            bake_milky_way: make_pipeline(device, &library, "bake_milky_way")?,
            buffers: (0..SCENE_BUFFER_COUNT
                + DUST_BUFFER_INDICES.len()
                + AVATAR_BUFFER_INDICES.len())
                .map(|_| GrowableBuffer { buffer: None })
                .collect(),
            textures,
            meshes: None,
            post: None,
            milky_way: make_hdr_texture(device, MILKY_WAY_TEXTURE_PX)?,
            is_milky_way_baked: false,
        })
    }

    /// `get_hand_light` gives the buffer where `encode` has the light at the eye written, for the
    /// held gun (viewmodel.rs).
    ///
    /// Example: `revolver.encode(&device, &command_buffer, &wand, size, 0.6, Some(gpu.get_hand_light()))?;`
    pub(crate) fn get_hand_light(&self) -> &ProtocolObject<dyn MTLBuffer> {
        &self.hand_light
    }

    /// `encode` uploads a frame, probes the light at the eye for the held gun, and traces it into
    /// the HDR texture (with `samples`, by adaptive sampling into those buffers, otherwise one ray
    /// per pixel), then blooms, grades and tone maps it into `target`. The first call bakes the
    /// Milky Way into its texture first; later frames only read it.
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
        if !self.is_milky_way_baked {
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE BAKE WRITING THE WRONG TEXTURE WILL HAPPEN
            unsafe { encoder.setTexture_atIndex(Some(&self.milky_way), BAKE_TARGET_TEXTURE_INDEX) };
            dispatch(encoder, &self.bake_milky_way, MILKY_WAY_TEXTURE_PX);
            self.is_milky_way_baked = true;
        }
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
            encoder.setTexture_atIndex(Some(&self.milky_way), MILKY_WAY_TEXTURE_INDEX);
        }
        let mesh_key: *const Vec<MeshModel> = Arc::as_ptr(&frame.meshes);
        if self.meshes.as_ref().is_none_or(|(key, _)| *key != mesh_key) {
            self.meshes = Some((mesh_key, MeshGpu::New(device, &frame.meshes)?));
        }
        self.meshes
            .as_ref()
            .expect("the meshes were just uploaded")
            .1
            .bind(encoder);
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
        let avatars: [&[u8]; AVATAR_BUFFER_INDICES.len()] = [
            as_bytes(&frame.avatar_triangles),
            as_bytes(&frame.avatar_nodes),
        ];
        for (k, bytes) in avatars.iter().enumerate() {
            let slot: usize = SCENE_BUFFER_COUNT + DUST_BUFFER_INDICES.len() + k;
            let buffer: &ProtocolObject<dyn MTLBuffer> = self.buffers[slot].write(device, bytes)?;
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
            unsafe { encoder.setBuffer_offset_atIndex(Some(buffer), 0, AVATAR_BUFFER_INDICES[k]) };
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
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER WRITING THE WRONG BUFFER WILL HAPPEN
        unsafe {
            encoder.setBuffer_offset_atIndex(Some(&self.hand_light), 0, HAND_LIGHT_BUFFER_INDEX)
        };
        dispatch(encoder, &self.probe_hand_light, HAND_LIGHT_GRID);
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

#[cfg(test)]
mod tests {
    //! Tests of packing a frame's laser beams and the beacons' beams for the shader.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Gun | the sniper's (code = team), the rifle's (code = team + `RIFLE_BEAM_CODE`) | `beams_carry_their_gun_team_and_speckle` |
    //! | Team | blue, red | `beams_carry_their_gun_team_and_speckle` |
    //! | Speckle share | none, some; the kinds' shares from the settings | `beams_carry_their_gun_team_and_speckle` |
    //! | A beacon's console | on (its beam is bright), off (its beam goes, the others stay) | `beacon_beams_go_with_their_consoles` |

    use super::*;
    use crate::map::{Map, get_map_folder};
    use crate::net::MAX_CONSOLE_COUNT;
    use crate::voxel_world::{Shot, VoxelPlayer, VoxelWorld};

    const SPECKLE_FRAC: f64 = 0.4;
    const RENDER_PX: (u32, u32) = (64, 40);

    #[test]
    fn beams_carry_their_gun_team_and_speckle() {
        let map: crate::map::Map =
            crate::map::Map::Load(&crate::map::get_map_folder("lunar_surface"))
                .expect("the Lunar Surface map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let camera: DMat4 = VoxelPlayer::AtSpawn(&world, 0).get_camera_frame(&world);
        let shot: Shot = world.trace_laser(camera);
        let beam = |team: u32, is_rifle: bool, speckle_frac: f64| LaserBeam {
            segments: shot.segments.clone(),
            team,
            is_rifle,
            speckle_frac,
        };
        let beams: [LaserBeam; 2] = [beam(0, false, 0.0), beam(1, true, SPECKLE_FRAC)];
        let frame: VoxelFrame = VoxelFrame::Build(&world, camera, &beams, None, &[], RENDER_PX);
        let sniper: [f32; 2] = [0.0, 0.0];
        let rifle: [f32; 2] = [SPECKLE_FRAC as f32, (1 + RIFLE_BEAM_CODE) as f32];
        let laser_beam_count: usize = frame.beams.len() - get_beacon_beams(&world).len();
        let packed: Vec<[f32; 2]> = frame.beams[..laser_beam_count]
            .iter()
            .map(|b| [b.info[1], b.info[3]])
            .collect();
        assert!(packed.contains(&sniper) && packed.contains(&rifle));
        assert!(packed.iter().all(|p| *p == sniper || *p == rifle));
        let kinds: &crate::settings::BeamSpeckleSettings = &world.settings.beam_speckle;
        assert_eq!(
            frame.uniforms.beam_speckle[1..3],
            [kinds.black_frac as f32, kinds.white_frac as f32]
        );
    }

    #[test]
    fn beacon_beams_go_with_their_consoles() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        let mut world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let beacon_count: usize = world.consoles.len();
        let get_brightness = |world: &mut VoxelWorld, off_mask: u32| -> Vec<f32> {
            world.light_consoles(off_mask, &[0.0; MAX_CONSOLE_COUNT], 0.0);
            get_beacon_beams(world).iter().map(|b| b.info[2]).collect()
        };
        assert_eq!(
            get_brightness(&mut world, 0),
            vec![BEAM_BRIGHTNESS; beacon_count]
        );
        let brightness: Vec<f32> = get_brightness(&mut world, 1);
        assert_eq!(
            brightness.iter().filter(|&&b| b == 0.0).count(),
            1,
            "{brightness:?}"
        );
        assert_eq!(
            brightness.iter().filter(|&&b| b == BEAM_BRIGHTNESS).count(),
            beacon_count - 1
        );
    }
}
