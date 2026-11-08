//! `scene_data.rs` packs one frame's scene for the renderers: rows of float 4-vectors, one row in
//! the camera's coordinates and one in each mouth's, plus the frame's `Uniforms`. A ray that goes
//! through a mouth continues in its partner's row, so coordinates near the ray stay small enough
//! for 32-bit floats. The layout is the same for the CPU renderer and the Metal shader.
//!
//! Interface: the `TEXEL_*` layout constants, `Uniforms`, `Beam`, `SceneData::Build`.
//!
//! Example: `let data = SceneData::Build(&world, camera_frame, &beam, size, time_s, true);`

use glam::{DMat4, DVec3, DVec4, Vec4};

use crate::geometry::{
    AXIS_W, AXIS_Y, UP_VECTOR, get_distance, get_local_direction, invert_lorentz, normalize_tangent,
};
use crate::world::{MAX_BOX_COUNT, MAX_CLUSTER_COUNT, MAX_MOUTH_COUNT, World};

pub const ROW_COUNT: usize = 1 + MAX_MOUTH_COUNT;
pub const TEXEL_TO_WORLD: usize = 0;
pub const TEXEL_GROUND: usize = 4;
pub const TEXEL_SUN: usize = 5;
pub const TEXEL_MOUTHS: usize = 6;
pub const TEXEL_GLUINGS: usize = TEXEL_MOUTHS + MAX_MOUTH_COUNT;
pub const TEXELS_PER_GLUING: usize = 4;
pub const TEXEL_BOXES: usize = TEXEL_GLUINGS + TEXELS_PER_GLUING * MAX_MOUTH_COUNT;
pub const TEXELS_PER_BOX: usize = 6;
pub const TEXEL_CLUSTERS: usize = TEXEL_BOXES + TEXELS_PER_BOX * MAX_BOX_COUNT;
pub const TEXELS_PER_CLUSTER: usize = 6;
pub const TEXEL_CLUSTER_INFO: usize = 5;
pub const TEXEL_CLUSTER_ORDER: usize = TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * MAX_CLUSTER_COUNT;
pub const TEXEL_SPOT: usize = TEXEL_CLUSTER_ORDER + MAX_CLUSTER_COUNT;
pub const MAX_BEAM_CHUNK_COUNT: usize = 64;
pub const TEXEL_BEAM_CHUNKS: usize = TEXEL_SPOT + 1;
pub const TEXELS_PER_BEAM_CHUNK: usize = 2;
pub const MAX_BEAM_POINT_COUNT: usize = 256;
pub const TEXEL_BEAM_POINTS: usize =
    TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * MAX_BEAM_CHUNK_COUNT;
pub const TEXEL_MOUTH_FIELDS: usize = TEXEL_BEAM_POINTS + MAX_BEAM_POINT_COUNT;
pub const TEXELS_PER_MOUTH_FIELD: usize = 5;
pub const ROW_TEXEL_COUNT: usize = TEXEL_MOUTH_FIELDS + TEXELS_PER_MOUTH_FIELD * MAX_MOUTH_COUNT;
pub const BEAM_POINTS_PER_CHUNK: usize = 4;
pub const HALF_FOV_RAD: f64 = 37.5 * std::f64::consts::PI / 180.0;
pub const FOG_PER_R: f64 = 3.0;
const MIN_TILE_CELL_COUNT: i32 = 3;
const MAX_TILE_CELL_COUNT: i32 = 15;
const TILE_SIDE_COUNT: f64 = 4.0;
const TILES_PER_VERTEX: f64 = 5.0;
pub const SUN_DIRECTION: DVec3 = DVec3::new(0.45, 0.62, 0.35);

/// `Uniforms` is everything per frame that isn't in the rows. Its layout matches the Metal
/// shader's `Uniforms` struct, so it is `repr(C)` and padded to a multiple of 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Uniforms {
    pub world_to_camera: [[f32; 4]; 4],
    pub sky_basis: [[f32; 4]; 3],
    pub sun_ideal_world: [f32; 4],
    pub next_rows: [i32; MAX_MOUTH_COUNT],
    pub row_tints: [[f32; 4]; MAX_MOUTH_COUNT],
    pub curvature_radius_m: f32,
    pub tan_half_fov: f32,
    pub aspect: f32,
    pub throat_m: f32,
    pub bend_cutoff_m: f32,
    pub fog_m: f32,
    pub time_s: f32,
    pub ground_edge_elevation_rad: f32,
    pub pixel_angle_rad: f32,
    pub mouth_count: i32,
    pub cluster_count: i32,
    pub tile_cell_count: i32,
    pub beam_chunk_count: i32,
    pub width_px: i32,
    pub height_px: i32,
    pub camera_mouth: i32,
    pub camera_distance_m: f32,
    pub jump_m: f32,
    pub padding: [i32; 2],
}

const _: () = assert!(std::mem::size_of::<Uniforms>().is_multiple_of(16));

/// `Beam` is the laser's path in world coordinates: runs of points (a new run starts after each
/// wormhole), each point with how far along the beam it is, and the spot where it hit something.
#[derive(Clone, Default)]
pub struct Beam {
    pub runs: Vec<Vec<(DVec4, f64)>>,
    pub spot: Option<DVec4>,
}

/// `SceneData` is one frame's packed scene: `rows` holds `ROW_COUNT` rows of `ROW_TEXEL_COUNT`
/// 4-vectors.
pub struct SceneData {
    pub rows: Vec<Vec4>,
    pub uniforms: Uniforms,
}

impl SceneData {
    /// `Build` packs the scene as seen from `camera_frame` at a render size, with the laser beam.
    ///
    /// Example: `let data = SceneData::Build(&world, frame, &Beam::default(), (640, 400), 0.0, true);`
    #[allow(non_snake_case)]
    pub fn Build(
        world: &World,
        camera_frame: DMat4,
        beam: &Beam,
        size_px: (u32, u32),
        time_s: f64,
        is_fog_on: bool,
    ) -> SceneData {
        let mut rows: Vec<Vec4> = vec![Vec4::ZERO; ROW_COUNT * ROW_TEXEL_COUNT];
        let beam_chunk_count: usize = fill_row(&mut rows, 0, world, camera_frame, beam);
        for (k, mouth) in world.mouths.iter().enumerate() {
            fill_row(&mut rows, 1 + k, world, mouth.frame, beam);
        }
        let uniforms: Uniforms = make_uniforms(
            world,
            camera_frame,
            beam_chunk_count,
            size_px,
            time_s,
            is_fog_on,
        );
        SceneData { rows, uniforms }
    }
}

/// `to_vec4` converts a double-precision 4-vector to single precision.
///
/// Example: `let v = to_vec4(point);`
fn to_vec4(v: DVec4) -> Vec4 {
    v.as_vec4()
}

/// `fill_row` writes the whole scene, in the coordinates of `frame`, into row `row`: the map back
/// to the world, ground, sun, mouths with their gluing maps into the partner's row, boxes,
/// clusters, the laser spot and the beam (in Klein coordinates, x/w, where geodesics are straight).
/// It returns how many beam chunks it wrote, the same for every row.
///
/// Example: `let beam_chunk_count = fill_row(&mut rows, 0, &world, camera_frame, &beam);`
fn fill_row(rows: &mut [Vec4], row: usize, world: &World, frame: DMat4, beam: &Beam) -> usize {
    let to_frame: DMat4 = invert_lorentz(frame);
    let r: f64 = world.curvature_radius_m;
    let sun: DVec4 = SUN_DIRECTION.normalize().extend(1.0);
    let base: usize = row * ROW_TEXEL_COUNT;
    let mut write = |texel: usize, value: Vec4| rows[base + texel] = value;
    for c in 0..4 {
        write(TEXEL_TO_WORLD + c, to_vec4(frame.col(c)));
    }
    write(TEXEL_GROUND, to_vec4(to_frame * UP_VECTOR));
    write(TEXEL_SUN, to_vec4(to_frame * sun));
    for (k, mouth) in world.mouths.iter().enumerate() {
        write(TEXEL_MOUTHS + k, to_vec4(to_frame * mouth.center));
        let gluing: DMat4 = mouth.flip * mouth.to_local * frame;
        for c in 0..4 {
            write(
                TEXEL_GLUINGS + TEXELS_PER_GLUING * k + c,
                to_vec4(gluing.col(c)),
            );
        }
    }
    let box_slabs = world
        .boxes
        .iter()
        .enumerate()
        .map(|(b, hyperbolic_box)| (TEXEL_BOXES + TEXELS_PER_BOX * b, hyperbolic_box));
    let cluster_slabs = world
        .clusters
        .iter()
        .enumerate()
        .map(|(k, cluster)| (TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k, &cluster.bound));
    for (first, hyperbolic_box) in box_slabs.chain(cluster_slabs) {
        for i in 0..3 {
            write(first + i, to_vec4(to_frame * hyperbolic_box.normals[i]));
        }
        let h: [f64; 3] = hyperbolic_box.half_m;
        let sigma: [f32; 3] = h.map(|half_m| (half_m / r).sinh() as f32);
        write(
            first + 3,
            Vec4::new(sigma[0], sigma[1], sigma[2], hyperbolic_box.material as f32),
        );
        write(
            first + 4,
            Vec4::new(h[0] as f32, h[1] as f32, h[2] as f32, 0.0),
        );
    }
    for (b, hyperbolic_box) in world.boxes.iter().enumerate() {
        let color: [f32; 3] = hyperbolic_box.color;
        write(
            TEXEL_BOXES + TEXELS_PER_BOX * b + 5,
            Vec4::new(color[0], color[1], color[2], 0.0),
        );
    }
    for (k, cluster) in world.clusters.iter().enumerate() {
        write(
            TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k + TEXEL_CLUSTER_INFO,
            Vec4::new(
                cluster.first_box as f32,
                cluster.box_count as f32,
                cluster.top_m as f32,
                0.0,
            ),
        );
    }
    for (i, (k, distance_m)) in order_clusters(world, frame.col(AXIS_W))
        .into_iter()
        .enumerate()
    {
        write(
            TEXEL_CLUSTER_ORDER + i,
            Vec4::new(k as f32, distance_m as f32, 0.0, 0.0),
        );
    }
    let to_klein = |point: DVec4| -> Vec4 {
        let local: DVec4 = to_frame * point;
        (local.truncate() / local.w).as_vec3().extend(0.0)
    };
    write(
        TEXEL_SPOT,
        match beam.spot {
            Some(point) => to_klein(point).truncate().extend(1.0),
            None => Vec4::ZERO,
        },
    );
    let mut chunk_count: usize = 0;
    let mut point_count: usize = 0;
    for run in &beam.runs {
        let mut start: usize = 0;
        while start + 1 < run.len() && chunk_count < MAX_BEAM_CHUNK_COUNT {
            let end_incl: usize = (start + BEAM_POINTS_PER_CHUNK).min(run.len() - 1);
            let count: usize = end_incl - start + 1;
            if point_count + count > MAX_BEAM_POINT_COUNT {
                break;
            }
            let points: Vec<Vec4> = run[start..=end_incl]
                .iter()
                .map(|(point, travelled_m)| to_klein(*point).truncate().extend(*travelled_m as f32))
                .collect();
            let center: glam::Vec3 =
                points.iter().map(|p| p.truncate()).sum::<glam::Vec3>() / count as f32;
            let radius: f32 = points
                .iter()
                .map(|p| (p.truncate() - center).length())
                .fold(0.0, f32::max);
            for (j, point) in points.iter().enumerate() {
                write(TEXEL_BEAM_POINTS + point_count + j, *point);
            }
            write(
                TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * chunk_count,
                center.extend(radius),
            );
            write(
                TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * chunk_count + 1,
                Vec4::new(point_count as f32, count as f32, 0.0, 0.0),
            );
            point_count += count;
            chunk_count += 1;
            start = end_incl;
        }
    }
    chunk_count
}

/// `order_clusters` lists the clusters by a distance each is at least from `origin` (past its
/// bounding box's slabs, and no less than the origin's height above its top),
/// nearest first. A ray that starts at the origin can't reach cluster k before travelling that far,
/// so the shader only looks at clusters in this order as its ray travels.
///
/// Example: `let order = order_clusters(&world, camera_frame.col(AXIS_W));`
fn order_clusters(world: &World, origin: DVec4) -> Vec<(usize, f64)> {
    let r: f64 = world.curvature_radius_m;
    let height_m: f64 = r * origin[AXIS_Y].asinh();
    let mut order: Vec<(usize, f64)> = world
        .clusters
        .iter()
        .enumerate()
        .map(|(k, cluster)| {
            let shape_m: f64 = world.get_box_lower_bound_m(&cluster.bound, origin);
            (k, shape_m.max(height_m - cluster.top_m).max(0.0))
        })
        .collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1));
    order
}

/// `get_tile_cell_count` picks how many cells each side of a ground tile has: about one per
/// block, odd so that the checker pattern is the same however a tile is reached.
///
/// Example: `let cell_count = get_tile_cell_count(8.0);`
pub fn get_tile_cell_count(curvature_radius_m: f64) -> i32 {
    let half_edge: f64 = ((std::f64::consts::PI / TILE_SIDE_COUNT).cos()
        / (std::f64::consts::PI / TILES_PER_VERTEX).sin())
    .acosh();
    let edge_m: f64 = 2.0 * half_edge * curvature_radius_m;
    (2 * ((edge_m - 1.0) / 2.0).round() as i32 + 1).clamp(MIN_TILE_CELL_COUNT, MAX_TILE_CELL_COUNT)
}

/// `make_uniforms` fills the per-frame uniforms, including what the sky needs: a level basis at
/// the camera and the elevation of the ground's edge, which in H³ lies well below eye level (the
/// angle of parallelism 2 atan(e^(-h/R)) from straight down).
///
/// Example: `let uniforms = make_uniforms(&world, frame, 0, (640, 400), 0.0, true);`
fn make_uniforms(
    world: &World,
    camera_frame: DMat4,
    beam_chunk_count: usize,
    size_px: (u32, u32),
    time_s: f64,
    is_fog_on: bool,
) -> Uniforms {
    let r: f64 = world.curvature_radius_m;
    let x: DVec4 = camera_frame.col(AXIS_W);
    let up: DVec3 = get_local_direction(camera_frame, normalize_tangent(x, UP_VECTOR)).normalize();
    let mut right: DVec3 = DVec3::X - up.x * up;
    if right.length() < 1e-6 {
        right = DVec3::Z - up.z * up;
    }
    let right: DVec3 = right.normalize();
    let back: DVec3 = right.cross(up);
    let height_m: f64 = r * x[AXIS_Y].max(0.0).asinh();
    let mut next_rows: [i32; MAX_MOUTH_COUNT] = [0; MAX_MOUTH_COUNT];
    let mut row_tints: [[f32; 4]; MAX_MOUTH_COUNT] = [[0.0; 4]; MAX_MOUTH_COUNT];
    for (k, mouth) in world.mouths.iter().enumerate() {
        next_rows[k] = (1 + mouth.partner_index) as i32;
        if let Some(color) = world.mouths[mouth.partner_index].color {
            let [r, g, b] = color.get_rgb();
            row_tints[k] = [r, g, b, 1.0];
        }
    }
    let tan_half_fov: f64 = HALF_FOV_RAD.tan();
    let (mut camera_mouth, mut camera_distance_m): (i32, f64) = (-1, 0.0);
    for (k, mouth) in world.mouths.iter().enumerate() {
        let distance_m: f64 = get_distance(x, mouth.center) * r;
        if distance_m < world.get_jump_radius_m()
            && (camera_mouth < 0 || distance_m < camera_distance_m)
        {
            (camera_mouth, camera_distance_m) = (k as i32, distance_m);
        }
    }
    Uniforms {
        world_to_camera: invert_lorentz(camera_frame).as_mat4().to_cols_array_2d(),
        sky_basis: [right, up, back].map(|v| v.as_vec3().extend(0.0).to_array()),
        sun_ideal_world: SUN_DIRECTION.normalize().extend(1.0).as_vec4().to_array(),
        next_rows,
        row_tints,
        curvature_radius_m: r as f32,
        tan_half_fov: tan_half_fov as f32,
        aspect: size_px.0 as f32 / size_px.1 as f32,
        throat_m: world.get_throat_m() as f32,
        bend_cutoff_m: world.bend_cutoff_m as f32,
        fog_m: if is_fog_on {
            (FOG_PER_R * r) as f32
        } else {
            0.0
        },
        time_s: time_s as f32,
        ground_edge_elevation_rad: (2.0 * (-height_m / r).exp().atan()
            - std::f64::consts::FRAC_PI_2) as f32,
        pixel_angle_rad: (2.0 * tan_half_fov / f64::from(size_px.1)) as f32,
        mouth_count: world.mouths.len() as i32,
        cluster_count: world.clusters.len() as i32,
        tile_cell_count: get_tile_cell_count(r),
        beam_chunk_count: beam_chunk_count as i32,
        width_px: size_px.0 as i32,
        height_px: size_px.1 as i32,
        camera_mouth,
        camera_distance_m: camera_distance_m as f32,
        jump_m: world.get_jump_radius_m() as f32,
        padding: [0; 2],
    }
}
