//! `voxel_world.rs` is a voxel map of the portable format (`common/maps/`, such as Lunar Surface) on
//! the CPU, ported from the Lunar Surface level's JavaScript: structures with bounding spheres, warp
//! objects with the structures in their reach, a voxel walk, the laser's light path, and walking on
//! the map (collisions, stepping up blocks, gravity, jumping, flying, and moving along light paths
//! near a wormhole mouth). `render_voxel.metal` draws the same world, with `voxel_clearance.rs`'s grids.
//!
//! Interface: `VoxelWorld::FromMap`, `VoxelWorld::with_avatars`, `VoxelWorld::light_consoles`,
//! `VoxelWorld::find_aimed_material`, `Console`, `VoxelPlayer`, `Shot`, `BeamSegment`.
//!
//! Example: `let world = VoxelWorld::FromMap(&Map::Load(&get_map_folder("lunar_surface"))?)?;`

use std::sync::Arc;

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, invert_lorentz, make_boost, make_rotation,
    minkowski_dot, normalize_tangent,
};
use crate::map::{Map, PlayerSettings, to_direction, to_frame, to_point};
use crate::net::{MAX_CONSOLE_COUNT, TEAM_NAMES};
use crate::physics::Input;
use crate::settings::GameSettings;
use crate::voxel_clearance::ClearanceGrids;

pub const MAX_MATERIAL_COUNT: usize = 256;
pub const AVATAR_FIRST_MATERIAL: usize = 240;
const BOUND_MARGIN_M: f64 = 0.2;
pub(crate) const BEND_STEP_FRAC: f64 = 0.08;
const CONSOLE_PULSE_HZ: f64 = 1.0;
const CONSOLE_DISABLING_PULSE_PER_PULSE: f64 = 2.0;
const CONSOLE_PULSE_MIN_FRAC: f32 = 0.6;
const CONSOLE_PULSE_MAX_FRAC: f32 = 2.0;
const CONSOLE_OFF_FRAC: f32 = 0.12;
const CONSOLE_PROBE_M: f64 = 0.05;
pub const SPAWN_TURN_RAD: f64 = 2.399_963_229_728_653;
pub const MIN_SPAWN_GAP_M: f64 = 1.5;
const SPAWN_RING_M: f64 = 3.0;
const SPAWN_RING_STEP_M: f64 = 2.0;
const SPAWN_SLOTS_PER_RING: u32 = 8;
const SPAWN_RING_COUNT: u32 = 3;
const MIN_BEND_STEP_M: f64 = 0.004;
const MAX_BEND_STEP_M: f64 = 3.0;
const HORIZON_CAPTURE_FRAC: f64 = 1.02;
const FALL_IN_PER_HORIZON: f64 = 2.0;
const SUPPORTED_LAMBDA_SQUARED: f64 = 0.25;
const BODY_RADIUS_M: f64 = 0.25;
const HEAD_CLEARANCE_M: f64 = 0.15;
const PROBE_LOW_M: f64 = 0.02;
const PROBE_KNEE_M: f64 = 0.3;
const PROBE_WAIST_M: f64 = 1.1;
const MIN_FLY_HEIGHT_M: f64 = 0.4;
const MAX_FLY_HEIGHT_M: f64 = 240.0;
const UNSTICK_STEP_M: f64 = 0.25;
const UNSTICK_MAX_STEPS: usize = 160;
const PUSH_OUT_STEP_M: f64 = 0.05;
const MAX_WALK_RISE_M: f64 = 0.3;
const PUSH_OUT_STEP_COUNT: usize = 6;
const PUSH_OUT_DIRECTION_COUNT: usize = 16;
const LANDING_BISECTIONS: usize = 10;
const STAND_SEARCH_STEP_M: f64 = 0.05;
const STAND_SEARCH_MAX_STEPS: usize = 400;
const MAX_FALL_SPEED_M_PER_S: f64 = 40.0;
const FLY_CLIMB_PER_S: f64 = 0.8;
const MIN_FLY_CLIMB_M_PER_S: f64 = 5.0;
const FLY_SPRINT_PER_FLY: f64 = 40.0 / 14.0;
const NEARBY_REACH_M: f64 = 6.0;
const LASER_MAX_LENGTH_M: f64 = 480.0;
const LASER_MAX_ITERATIONS: usize = 4000;
const LASER_MUZZLE_M: f64 = 3.0;
pub const MUZZLE_M: DVec3 = DVec3::new(0.187, -0.157, -0.555);
const BEAM_TOLERANCE_M: f64 = 0.02;
pub const LOOK_RAD_PER_PX: f64 = 0.0025;
const MAX_PITCH_RAD: f64 = 1.55;
const TINY: f64 = 1e-12;
const AVATAR_SIZE: [usize; 3] = [1, 4, 1];
const AVATAR_CELL_M: f64 = 0.5;
const AVATAR_ORIGIN: [f64; 3] = [0.5, 0.0, 0.5];

/// `VoxelStructure` is one structure: its frame (and inverse), its grid, where its cells start in
/// the voxel bytes, and its bounding sphere (radius in curvature units).
#[derive(Clone)]
pub struct VoxelStructure {
    pub name: String,
    pub inverse: DMat4,
    pub size: [usize; 3],
    pub origin: [f64; 3],
    pub cell_m: f64,
    pub offset: usize,
    pub centre: DVec4,
    pub radius: f64,
}

/// `VoxelWarp` is a black hole or a wormhole mouth: its frame, m and reach in curvature units,
/// its partner and the structures within its reach.
#[derive(Clone)]
pub struct VoxelWarp {
    pub name: String,
    pub is_mouth: bool,
    pub frame: DMat4,
    pub inverse: DMat4,
    pub centre: DVec4,
    pub m: f64,
    pub reach: f64,
    pub partner: usize,
    pub list: Vec<usize>,
}

/// `SpecialMaterials` is the ids of the materials the `lunar` style draws in its own way (-1 when
/// the map lacks one).
#[derive(Clone, Copy)]
pub struct SpecialMaterials {
    pub glass: i32,
    pub regolith: i32,
    pub dark_regolith: i32,
    pub basalt: i32,
    pub solar: i32,
    pub hazard: i32,
    pub pad: i32,
    pub hull: i32,
    pub gold_foil: i32,
    pub console_first: i32,
    pub console_count: i32,
}

/// `Console` is one console of a team's pylon: its material, its team (an index into
/// `TEAM_NAMES`) and the map's colour for it.
#[derive(Clone, Copy)]
pub struct Console {
    pub material: u8,
    pub team: u32,
    color: [f32; 3],
}

/// `find_consoles` lists a map's consoles, the materials named "<team> console <n>" (such as
/// "red console 2"), in the map's material order, which numbers them for the game server.
///
/// Example: `let teams: Vec<u32> = find_consoles(&map).iter().map(|c| c.team).collect();`
pub fn find_consoles(map: &Map) -> Vec<Console> {
    let consoles: Vec<Console> = map
        .materials
        .iter()
        .filter_map(|m| {
            let team: usize = TEAM_NAMES
                .iter()
                .position(|name| m.name.starts_with(&format!("{name} console ")))?;
            Some(Console {
                material: m.id,
                team: team as u32,
                color: m.color,
            })
        })
        .collect();
    assert!(
        consoles.len() <= MAX_CONSOLE_COUNT,
        "map {}: {} consoles, more than the game's {MAX_CONSOLE_COUNT}",
        map.name,
        consoles.len()
    );
    consoles
}

/// `VoxelWorld` is a whole voxel map, ready to trace and to walk on.
#[derive(Clone)]
pub struct VoxelWorld {
    pub name: String,
    pub r: f64,
    pub structures: Vec<VoxelStructure>,
    pub voxels: Vec<u8>,
    pub warps: Vec<VoxelWarp>,
    pub lights: Vec<(DVec4, [f32; 3], f64)>,
    pub palette: Vec<[f32; 4]>,
    pub special: SpecialMaterials,
    pub regions: Option<[f64; 3]>,
    pub sun: DVec3,
    pub planet: Option<(DVec3, f64)>,
    pub player: PlayerSettings,
    pub gravity_m_per_s2: f64,
    map_player: PlayerSettings,
    map_gravity_m_per_s2: f64,
    pub settings: GameSettings,
    pub spawns: Vec<DMat4>,
    pub static_structure_count: usize,
    pub consoles: Vec<Console>,
    pub clearance: Arc<ClearanceGrids>,
}

/// `Hit` is where a light path met something: the structure (or `None` for the ground) and the point.
#[derive(Clone, Copy)]
pub struct Hit {
    pub structure: Option<usize>,
    pub s: f64,
}

/// `BeamSegment` is one geodesic piece of a laser: start, unit tangent and length (curvature units).
#[derive(Clone, Copy)]
pub struct BeamSegment {
    pub a: DVec4,
    pub b: DVec4,
    pub length: f64,
}

/// `Shot` is a laser's path: its drawn segments, where it hit (and what), and the mouths it went through.
pub struct Shot {
    pub segments: Vec<BeamSegment>,
    pub spot: Option<DVec4>,
    pub hit_structure: Option<usize>,
    pub is_hit: bool,
    pub length_m: f64,
    pub through: Vec<String>,
}

/// `clip_beam` gives the part of a beam (`segments`, in a world of curvature radius `r`) from
/// `from_m` to `to_m` along it, meters from its start.
///
/// Example: `let flying = clip_beam(&shot.segments, world.r, 20.0, 35.0);`
pub fn clip_beam(segments: &[BeamSegment], r: f64, from_m: f64, to_m: f64) -> Vec<BeamSegment> {
    let mut start_m: f64 = 0.0;
    let mut clipped: Vec<BeamSegment> = Vec::new();
    for segment in segments {
        let length_m: f64 = segment.length * r;
        let (lo_m, hi_m) = (from_m.max(start_m), to_m.min(start_m + length_m));
        if lo_m < hi_m {
            let s: f64 = (lo_m - start_m) / r;
            clipped.push(BeamSegment {
                a: geodesic_point(segment.a, segment.b, s),
                b: geodesic_tangent(segment.a, segment.b, s),
                length: (hi_m - lo_m) / r,
            });
        }
        start_m += length_m;
    }
    clipped
}

/// `normalize_minkowski` scales a vector to unit Minkowski length (either sign).
///
/// Example: `let g = normalize_minkowski(g);`
fn normalize_minkowski(a: DVec4) -> DVec4 {
    a / minkowski_dot(a, a).abs().sqrt()
}

/// `geodesic_point` gives cosh(s) x + sinh(s) v.
///
/// Example: `let y = geodesic_point(x, v, 0.1);`
fn geodesic_point(x: DVec4, v: DVec4, s: f64) -> DVec4 {
    s.cosh() * x + s.sinh() * v
}

/// `geodesic_tangent` gives sinh(s) x + cosh(s) v.
///
/// Example: `let w = geodesic_tangent(x, v, 0.1);`
fn geodesic_tangent(x: DVec4, v: DVec4, s: f64) -> DVec4 {
    s.sinh() * x + s.cosh() * v
}

/// `map_klein` maps Klein coordinates (|k| < 1) to the hyperboloid.
///
/// Example: `let p = map_klein(DVec3::new(0.1, 0.0, 0.0));`
pub(crate) fn map_klein(k: DVec3) -> DVec4 {
    let f: f64 = 1.0 / (1.0 - k.length_squared()).sqrt();
    (k * f).extend(f)
}

/// `map_exp` maps a local offset in meters to the hyperboloid by the exponential map at the origin.
///
/// Example: `let probe = map_exp(DVec3::new(0.25, 0.0, 0.0), 40.0);`
fn map_exp(q_m: DVec3, r: f64) -> DVec4 {
    let n: f64 = q_m.length() / r;
    if n < TINY {
        return DVec4::W;
    }
    (q_m * (n.sinh() / (n * r))).extend(n.cosh())
}

/// `cross_ground` gives the ground's cross product of two ground vectors (the level's `cross_ground`).
///
/// Example: `let right = cross_ground(t, g);`
fn cross_ground(a: DVec4, b: DVec4) -> DVec4 {
    DVec4::new(
        a.z * b.w - a.w * b.z,
        0.0,
        a.w * b.x - a.x * b.w,
        -(a.x * b.z - a.z * b.x),
    )
}

/// `make_ground_frame` gives the orientation-preserving isometry fixing e_y that sends the origin
/// to ground point `g` and local +z toward `toward`.
///
/// Example: `let frame = make_ground_frame(g, -heading);`
fn make_ground_frame(g: DVec4, toward: DVec4) -> DMat4 {
    let g: DVec4 = normalize_minkowski(g);
    let t: DVec4 = normalize_minkowski(toward + minkowski_dot(toward, g) * g);
    let r: DVec4 = normalize_minkowski(cross_ground(t, g));
    let frame: DMat4 = DMat4::from_cols(r, DVec4::Y, t, g);
    if frame.determinant() < 0.0 {
        DMat4::from_cols(-r, DVec4::Y, t, g)
    } else {
        frame
    }
}

/// `renormalize_frame` removes rounding drift from a body frame, keeping it on the ground with e_y up.
///
/// Example: `body = renormalize_frame(body);`
fn renormalize_frame(body: DMat4) -> DMat4 {
    let mut g: DVec4 = body.col(AXIS_W);
    g.y = 0.0;
    g = normalize_minkowski(g);
    if g.w < 0.0 {
        g = -g;
    }
    let mut t: DVec4 = body.col(AXIS_Z);
    t.y = 0.0;
    t = normalize_minkowski(t + minkowski_dot(t, g) * g);
    let mut r: DVec4 = normalize_minkowski(cross_ground(t, g));
    let old: DVec4 = body.col(AXIS_X);
    if r.x * old.x + r.z * old.z + r.w * old.w < 0.0 {
        r = -r;
    }
    DMat4::from_cols(r, DVec4::Y, t, g)
}

/// `find_material` gives the id of the material with `name`, or -1.
///
/// Example: `let glass = find_material(map, "glass");`
fn find_material(map: &Map, name: &str) -> i32 {
    map.materials
        .iter()
        .find(|m| m.name == name)
        .map_or(-1, |m| i32::from(m.id))
}

impl VoxelWorld {
    /// `FromMap` builds the world from a voxel map: each structure's bounding sphere (around its
    /// grid's corners, plus `BOUND_MARGIN_M`), each warp's list of structures within reach, the
    /// palette (with a glow for `light` materials) and the `lunar` style's special materials.
    /// Warps must use the `distance` radius and the `taper` falloff, and foils λ² = 0.25. The
    /// player moves by the built-in game settings (`set_settings`), so the map's gravity and jump
    /// speed must be above 0.
    ///
    /// Example: `let world = VoxelWorld::FromMap(&map)?;`
    #[allow(non_snake_case)]
    pub fn FromMap(map: &Map) -> Result<VoxelWorld, String> {
        let r: f64 = map.get_curvature_radius_m();
        if map.gravity_m_per_s2 <= 0.0 || map.player.jump_m_per_s <= 0.0 {
            return Err(format!(
                "map {}: the voxel game needs gravity and a jump speed above 0, not {} m/s² and {} m/s",
                map.name, map.gravity_m_per_s2, map.player.jump_m_per_s
            ));
        }
        let structures: Vec<VoxelStructure> = map
            .structures
            .iter()
            .map(|s| {
                let frame: DMat4 = to_frame(&s.frame);
                let size: [usize; 3] = s.voxels.size.map(|n| n as usize);
                let origin: [f64; 3] = s.voxels.origin.map(f64::from);
                let cell_m: f64 = s.voxels.cell_m;
                let lo: DVec3 = DVec3::from_array(origin.map(|o| -o * cell_m));
                let hi: DVec3 =
                    DVec3::from_array([0, 1, 2].map(|a| (size[a] as f64 - origin[a]) * cell_m));
                let centre_local: DVec4 = map_klein((lo + hi) / (2.0 * r));
                let radius: f64 = (0..8)
                    .map(|c: usize| {
                        let q: DVec3 = DVec3::from_array(
                            [0, 1, 2].map(|a| if c >> a & 1 == 1 { hi[a] } else { lo[a] }),
                        );
                        get_distance(map_klein(q / r), centre_local)
                    })
                    .fold(0.0, f64::max);
                VoxelStructure {
                    name: s.name.clone(),
                    inverse: invert_lorentz(frame),
                    size,
                    origin,
                    cell_m,
                    offset: s.voxels.offset_bytes,
                    centre: frame * centre_local,
                    radius: radius + BOUND_MARGIN_M / r,
                }
            })
            .collect();
        let mut warps: Vec<VoxelWarp> = Vec::new();
        for warp in &map.warps {
            let is_supported: bool = warp.radial_coordinate == "distance"
                && warp.falloff == "taper"
                && (warp.law != "foil" || warp.lambda_squared == Some(SUPPORTED_LAMBDA_SQUARED));
            if !is_supported {
                return Err(format!(
                    "map {}: the voxel renderer draws warps with the distance radius, a taper and λ² = 0.25; {:?} differs",
                    map.name, warp.name
                ));
            }
            let frame: DMat4 = to_frame(&warp.frame);
            let centre: DVec4 = frame.col(AXIS_W);
            let reach: f64 = warp.reach_m / r;
            warps.push(VoxelWarp {
                name: warp.name.clone(),
                is_mouth: warp.kind == "wormhole_mouth",
                frame,
                inverse: invert_lorentz(frame),
                centre,
                m: warp.schwarzschild_radius_m / 4.0 / r,
                reach,
                partner: warp.partner.unwrap_or(usize::MAX),
                list: (0..structures.len())
                    .filter(|&k| {
                        get_distance(structures[k].centre, centre) < structures[k].radius + reach
                    })
                    .collect(),
            });
        }
        let mut palette: Vec<[f32; 4]> = vec![[0.0; 4]; MAX_MATERIAL_COUNT];
        for material in &map.materials {
            let glow: f32 = if material.kind == "light" { 1.0 } else { 0.0 };
            palette[usize::from(material.id)] = [
                material.color[0],
                material.color[1],
                material.color[2],
                glow,
            ];
        }
        let consoles: Vec<i32> = map
            .materials
            .iter()
            .filter(|m| m.name.contains("console"))
            .map(|m| i32::from(m.id))
            .collect();
        let special: SpecialMaterials = SpecialMaterials {
            glass: map
                .materials
                .iter()
                .find(|m| m.kind == "glass")
                .map_or(-1, |m| i32::from(m.id)),
            regolith: i32::from(map.ground.material),
            dark_regolith: find_material(map, "dark regolith"),
            basalt: find_material(map, "basalt"),
            solar: find_material(map, "solar panel"),
            hazard: find_material(map, "hazard paint"),
            pad: find_material(map, "landing pad"),
            hull: find_material(map, "hull"),
            gold_foil: find_material(map, "gold foil"),
            console_first: consoles.first().copied().unwrap_or(-1),
            console_count: consoles.len() as i32,
        };
        let regions: Option<[f64; 3]> = map.regions.as_ref().and_then(|regions| {
            let edge_m: f64 = regions
                .zones
                .as_ref()?
                .iter()
                .filter_map(|z| z.u_max_m)
                .fold(f64::NEG_INFINITY, f64::max);
            let boundary = regions.boundary.as_ref()?;
            Some([edge_m, boundary.u_m, boundary.v_m])
        });
        let mut world: VoxelWorld = VoxelWorld {
            name: map.name.clone(),
            r,
            static_structure_count: structures.len(),
            consoles: find_consoles(map),
            structures,
            voxels: map.voxels.clone(),
            warps,
            lights: map
                .lights
                .iter()
                .map(|l| (to_point(&l.position), l.color, l.radius_m))
                .collect(),
            palette,
            special,
            regions,
            sun: to_direction(&map.sky.sun.direction),
            planet: map
                .sky
                .planet
                .as_ref()
                .map(|p| (to_direction(&p.direction), p.angular_radius_rad)),
            player: map.player,
            gravity_m_per_s2: map.gravity_m_per_s2,
            map_player: map.player,
            map_gravity_m_per_s2: map.gravity_m_per_s2,
            settings: GameSettings::Builtin(),
            spawns: map.spawns.iter().map(|s| to_frame(&s.frame)).collect(),
            clearance: Arc::new(ClearanceGrids::default()),
        };
        world.set_settings(GameSettings::Builtin());
        world.clearance = Arc::new(ClearanceGrids::Build(&world));
        Ok(world)
    }

    /// `set_settings` makes `settings` the world's game settings and moves the player by them:
    /// walking and sprinting at multiples of the map's speeds, and a jump that rises
    /// `jump_height_m` in `jump_time_per_map_jump_time` of the map's jump time (gravity follows).
    ///
    /// Example: `world.set_settings(GameSettings::Load(Path::new("fast.yaml"))?);`
    pub fn set_settings(&mut self, settings: GameSettings) {
        let movement = &settings.movement;
        let half_jump_s: f64 = self.map_player.jump_m_per_s / self.map_gravity_m_per_s2
            * movement.jump_time_per_map_jump_time;
        let jump_m_per_s: f64 = 2.0 * movement.jump_height_m / half_jump_s;
        self.player = PlayerSettings {
            walk_m_per_s: self.map_player.walk_m_per_s * movement.walk_speed_per_map_speed,
            sprint_m_per_s: self.map_player.sprint_m_per_s * movement.sprint_speed_per_map_speed,
            jump_m_per_s,
            ..self.map_player
        };
        self.gravity_m_per_s2 = jump_m_per_s / half_jump_s;
        self.settings = settings;
    }

    /// `with_avatars` gives a copy of the world with a body for each of `bodies` (a feet frame
    /// and an avatar colour index): a 1 × 4 grid of half-meter cells in its own material, so it is
    /// drawn, lensed and hit by lasers like any structure.
    ///
    /// Example: `let shared = world.with_avatars(&[(feet, 2)]);`
    pub fn with_avatars(&self, bodies: &[(DMat4, usize)], colors: &[[f32; 3]]) -> VoxelWorld {
        let mut shared: VoxelWorld = self.clone();
        for &(feet, color_index) in bodies {
            let material: usize = AVATAR_FIRST_MATERIAL + color_index % colors.len();
            let color: [f32; 3] = colors[color_index % colors.len()];
            shared.palette[material] = [color[0], color[1], color[2], 0.0];
            let offset: usize = shared.voxels.len();
            shared.voxels.extend(std::iter::repeat_n(
                material as u8,
                AVATAR_SIZE.iter().product(),
            ));
            let r: f64 = self.r;
            let lo: DVec3 = DVec3::from_array(AVATAR_ORIGIN.map(|o| -o * AVATAR_CELL_M));
            let hi: DVec3 = DVec3::from_array(
                [0, 1, 2].map(|a| (AVATAR_SIZE[a] as f64 - AVATAR_ORIGIN[a]) * AVATAR_CELL_M),
            );
            let centre_local: DVec4 = map_klein((lo + hi) / (2.0 * r));
            shared.structures.push(VoxelStructure {
                name: "player".to_string(),
                inverse: invert_lorentz(feet),
                size: AVATAR_SIZE,
                origin: AVATAR_ORIGIN,
                cell_m: AVATAR_CELL_M,
                offset,
                centre: feet * centre_local,
                radius: get_distance(map_klein(hi / r), centre_local) + BOUND_MARGIN_M / r,
            });
            for warp in &mut shared.warps {
                let k: usize = shared.structures.len() - 1;
                if get_distance(shared.structures[k].centre, warp.centre)
                    < shared.structures[k].radius + warp.reach
                {
                    warp.list.push(k);
                }
            }
        }
        shared
    }

    /// `light_consoles` sets each console's light at `time_s`: dark (and unlit) once its bit in
    /// `off_mask` is set, otherwise pulsing `CONSOLE_PULSE_HZ` times a second between
    /// `CONSOLE_PULSE_MIN_FRAC` and `CONSOLE_PULSE_MAX_FRAC` of its colour, and
    /// `CONSOLE_DISABLING_PULSE_PER_PULSE` times as fast while its bit in `disabling_mask` is set.
    ///
    /// Example: `world.light_consoles(game.off_mask, 0b100, 12.5);`
    pub fn light_consoles(&mut self, off_mask: u32, disabling_mask: u32, time_s: f64) {
        for (i, console) in self.consoles.iter().enumerate() {
            let entry: [f32; 4] = if off_mask >> i & 1 == 1 {
                let [r, g, b] = console.color.map(|c| c * CONSOLE_OFF_FRAC);
                [r, g, b, 0.0]
            } else {
                let hz: f64 = if disabling_mask >> i & 1 == 1 {
                    CONSOLE_PULSE_HZ * CONSOLE_DISABLING_PULSE_PER_PULSE
                } else {
                    CONSOLE_PULSE_HZ
                };
                let wave: f32 = (0.5 + 0.5 * (std::f64::consts::TAU * hz * time_s).sin()) as f32;
                let level: f32 = CONSOLE_PULSE_MIN_FRAC
                    + (CONSOLE_PULSE_MAX_FRAC - CONSOLE_PULSE_MIN_FRAC) * wave;
                let [r, g, b] = console.color.map(|c| c * level);
                [r, g, b, 1.0]
            };
            self.palette[usize::from(console.material)] = entry;
        }
    }

    /// `find_aimed_material` gives the material of the block straight ahead of `camera`'s eye
    /// within `reach_m`, or `None` when the ground or nothing is there. The ray isn't bent: over a
    /// few meters only a mouth's throat would bend it much.
    ///
    /// Example: `let material = world.find_aimed_material(camera, 3.0);`
    pub fn find_aimed_material(&self, camera: DMat4, reach_m: f64) -> Option<u8> {
        let (x, v) = (camera.col(AXIS_W), -camera.col(AXIS_Z));
        let hit: Hit = self.trace_scene(x, v, reach_m / self.r, None)?;
        let inside: DVec4 = geodesic_point(x, v, hit.s + CONSOLE_PROBE_M / self.r);
        Some(self.get_material_at(inside, &[hit.structure?]))
    }

    /// `is_head_hit` says whether point x, on an avatar (structure `k`, from `with_avatars`), is in
    /// its top cell, its head.
    ///
    /// Example: `let is_head = shared.is_head_hit(k, spot);`
    pub fn is_head_hit(&self, k: usize, x: DVec4) -> bool {
        let s: &VoxelStructure = &self.structures[k];
        let p: DVec4 = s.inverse * x;
        p.y / p.w * self.r / s.cell_m + s.origin[1] >= (AVATAR_SIZE[1] - 1) as f64
    }

    /// `get_material_at` gives the material of the cell holding point x among `candidates`, or 0.
    ///
    /// Example: `let m = world.get_material_at(x, &nearby);`
    fn get_material_at(&self, x: DVec4, candidates: &[usize]) -> u8 {
        for &k in candidates {
            let s: &VoxelStructure = &self.structures[k];
            let p: DVec4 = s.inverse * x;
            let cell: [f64; 3] =
                [0, 1, 2].map(|a| (p[a] / p.w * self.r / s.cell_m + s.origin[a]).floor());
            if (0..3).any(|a| cell[a] < 0.0 || cell[a] >= s.size[a] as f64) {
                continue;
            }
            let [i, j, k_] = cell.map(|c| c as usize);
            let material: u8 = self.voxels[s.offset + i + s.size[0] * (j + s.size[1] * k_)];
            if material != 0 {
                return material;
            }
        }
        0
    }

    /// `find_nearby_structures` lists the structures whose bounding sphere comes within `reach_m` of x.
    ///
    /// Example: `let nearby = world.find_nearby_structures(x, 6.0);`
    fn find_nearby_structures(&self, x: DVec4, reach_m: f64) -> Vec<usize> {
        let reach: f64 = reach_m / self.r;
        (0..self.structures.len())
            .filter(|&k| {
                -minkowski_dot(x, self.structures[k].centre)
                    < (self.structures[k].radius + reach).cosh()
            })
            .collect()
    }

    /// `get_fermi_coordinates` gives (u, v, h) of a point in meters.
    ///
    /// Example: `let [u_m, v_m, h_m] = world.get_fermi_coordinates(x);`
    fn get_fermi_coordinates(&self, x: DVec4) -> [f64; 3] {
        let cosh_h: f64 = (1.0 + x.y * x.y).sqrt();
        [
            -self.r * (x.z / x.w).atanh(),
            self.r * (x.x / cosh_h).asinh(),
            self.r * x.y.asinh(),
        ]
    }

    /// `is_outside_boundary` says whether a point is outside the playable ellipse (never, without one).
    ///
    /// Example: `if world.is_outside_boundary(x) { ... }`
    fn is_outside_boundary(&self, x: DVec4) -> bool {
        match self.regions {
            Some([_, u_max_m, v_max_m]) => {
                let [u_m, v_m, _] = self.get_fermi_coordinates(x);
                (u_m / u_max_m).powi(2) + (v_m / v_max_m).powi(2) > 1.0
            }
            None => false,
        }
    }

    /// `is_body_blocked` says whether a body standing on `body` with its eye `h_m` up touches a
    /// block or the ground, by probe points at its feet, knees, waist and head.
    ///
    /// Example: `if world.is_body_blocked(body, h_m, &nearby) { ... }`
    fn is_body_blocked(&self, body: DMat4, h_m: f64, nearby: &[usize]) -> bool {
        let eye_m: f64 = self.player.eye_height_m;
        let probes: [[f64; 3]; 10] = [
            [0.0, PROBE_LOW_M, 0.0],
            [BODY_RADIUS_M, PROBE_KNEE_M, 0.0],
            [-BODY_RADIUS_M, PROBE_KNEE_M, 0.0],
            [0.0, PROBE_KNEE_M, BODY_RADIUS_M],
            [0.0, PROBE_KNEE_M, -BODY_RADIUS_M],
            [BODY_RADIUS_M, PROBE_WAIST_M, 0.0],
            [-BODY_RADIUS_M, PROBE_WAIST_M, 0.0],
            [0.0, PROBE_WAIST_M, BODY_RADIUS_M],
            [0.0, PROBE_WAIST_M, -BODY_RADIUS_M],
            [0.0, eye_m + HEAD_CLEARANCE_M, 0.0],
        ];
        probes.iter().any(|&[dx, dy, dz]| {
            let y_m: f64 = h_m - eye_m + dy;
            y_m < 0.0
                || self.get_material_at(
                    body * make_boost(AXIS_Y, y_m / self.r)
                        * map_exp(DVec3::new(dx, 0.0, dz), self.r),
                    nearby,
                ) != 0
        })
    }

    /// `push_out_sideways` gives the nearest body, moved along the ground by up to
    /// `PUSH_OUT_STEP_COUNT` × `PUSH_OUT_STEP_M` in one of `PUSH_OUT_DIRECTION_COUNT` directions,
    /// that is clear of blocks at eye height `h_m`, or `None`. A body grazing a wall (turned so a
    /// probe swings into it) gets out this way instead of being lifted to the wall's top.
    ///
    /// Example: `if let Some(body) = world.push_out_sideways(body, h_m, &nearby) { ... }`
    fn push_out_sideways(&self, body: DMat4, h_m: f64, nearby: &[usize]) -> Option<DMat4> {
        (1..=PUSH_OUT_STEP_COUNT).find_map(|n| {
            let distance: f64 = n as f64 * PUSH_OUT_STEP_M / self.r;
            (0..PUSH_OUT_DIRECTION_COUNT).find_map(|k| {
                let angle_rad: f64 =
                    std::f64::consts::TAU * k as f64 / PUSH_OUT_DIRECTION_COUNT as f64;
                let moved: DMat4 = renormalize_frame(
                    body * make_boost(AXIS_Z, angle_rad.sin() * distance)
                        * make_boost(AXIS_X, angle_rad.cos() * distance),
                );
                (!self.is_body_blocked(moved, h_m, nearby)).then_some(moved)
            })
        })
    }

    /// `find_walk_rise_m` gives the smallest lift, up to `MAX_WALK_RISE_M`, that clears a body
    /// standing on `body` with eye height `h_m`, or `None` if that much doesn't. Walking on a
    /// structure needs it: its cells' tops are geodesic planes, which rise a little against the
    /// ground's height away from the structure's centre, but a lift this small never climbs a
    /// block.
    ///
    /// Example: `if let Some(rise_m) = world.find_walk_rise_m(body, h_m, &nearby) { ... }`
    fn find_walk_rise_m(&self, body: DMat4, h_m: f64, nearby: &[usize]) -> Option<f64> {
        if self.is_body_blocked(body, h_m + MAX_WALK_RISE_M, nearby) {
            return None;
        }
        let (mut lo_m, mut hi_m) = (0.0, MAX_WALK_RISE_M);
        for _ in 0..LANDING_BISECTIONS {
            let mid_m: f64 = (lo_m + hi_m) / 2.0;
            if self.is_body_blocked(body, h_m + mid_m, nearby) {
                lo_m = mid_m;
            } else {
                hi_m = mid_m;
            }
        }
        Some(hi_m)
    }

    /// `find_standing_height` gives the eye height where a body standing on `body` is clear of blocks.
    ///
    /// Example: `let h_m = world.find_standing_height(body);`
    fn find_standing_height(&self, body: DMat4) -> f64 {
        let nearby: Vec<usize> = self.find_nearby_structures(body.col(AXIS_W), NEARBY_REACH_M);
        let mut h_m: f64 = self.player.eye_height_m;
        for _ in 0..STAND_SEARCH_MAX_STEPS {
            if !self.is_body_blocked(body, h_m, &nearby) {
                break;
            }
            h_m += STAND_SEARCH_STEP_M;
        }
        h_m
    }

    /// `compute_bend` gives warp w's bending at (x, v), ln n tapered to zero at its reach, and the
    /// distance to its centre.
    ///
    /// Example: `let (bend, rho) = world.compute_bend(0, x, v);`
    fn compute_bend(&self, w: usize, x: DVec4, v: DVec4) -> (DVec4, f64) {
        let warp: &VoxelWarp = &self.warps[w];
        let p: DVec4 = warp.inverse * x;
        let rho: f64 = p.truncate().length().asinh();
        let (ch, sh) = (rho.cosh(), rho.sinh().max(TINY));
        let f: f64 = rho / warp.reach;
        let window: f64 = (1.0 - f * f).powi(2);
        let window_slope: f64 = -4.0 * f * (1.0 - f * f) / warp.reach;
        let (slope_n, log_n) = if warp.is_mouth {
            let (pt, ir) = (warp.m, 1.0 / rho);
            let psi: f64 = 1.0 + pt * ir;
            let ir2: f64 = ir / (psi * psi);
            let rs: f64 = 4.0 * pt;
            let slope: f64 = -2.0 * pt * ir * ir / psi
                - rs * psi * (1.0 - pt * ir) * ir2 * ir2
                    / (2.0 * (1.0 + SUPPORTED_LAMBDA_SQUARED - rs * ir2));
            let r_s: f64 = rho * psi * psi;
            let log: f64 = 2.0 * psi.ln()
                - 0.5
                    * ((1.0 - 4.0 * pt / r_s + SUPPORTED_LAMBDA_SQUARED)
                        / (1.0 + SUPPORTED_LAMBDA_SQUARED))
                        .ln();
            (slope, log)
        } else {
            let q: f64 = warp.m / rho;
            let guard: f64 = (1.0 - q).max(1e-4);
            (
                -warp.m / (rho * rho) * (3.0 / (1.0 + q) + 1.0 / guard),
                3.0 * (1.0 + q).ln() - guard.ln(),
            )
        };
        let slope: f64 = if f >= 1.0 {
            0.0
        } else {
            slope_n * window + log_n * window_slope
        };
        let g: DVec4 = slope * (ch * x - warp.centre) / sh;
        (g - minkowski_dot(g, v) * v, rho)
    }

    /// `step_bent` advances a ray one kick–drift–kick step inside warp w, returning the chord's
    /// start direction, the new state and the distance to the centre.
    ///
    /// Example: `let (v0, x1, v1, rho) = world.step_bent(0, x, v, 0.01);`
    fn step_bent(&self, w: usize, x: DVec4, v: DVec4, ds: f64) -> (DVec4, DVec4, DVec4, f64) {
        let v0: DVec4 = normalize_tangent(x, v + 0.5 * ds * self.compute_bend(w, x, v).0);
        let x1: DVec4 = normalize_minkowski(geodesic_point(x, v0, ds));
        let v1: DVec4 = normalize_tangent(x1, geodesic_tangent(x, v0, ds));
        let (bend, rho) = self.compute_bend(w, x1, v1);
        (v0, x1, normalize_tangent(x1, v1 + 0.5 * ds * bend), rho)
    }

    /// `choose_bend_step` gives the step inside a warp: a fraction of the distance to its centre.
    ///
    /// Example: `let ds = world.choose_bend_step(rho);`
    fn choose_bend_step(&self, rho: f64) -> f64 {
        (BEND_STEP_FRAC * rho).clamp(MIN_BEND_STEP_M / self.r, MAX_BEND_STEP_M / self.r)
    }

    /// `find_warp_inside` gives the warp whose reach sphere holds x.
    ///
    /// Example: `let inside = world.find_warp_inside(eye);`
    fn find_warp_inside(&self, x: DVec4) -> Option<usize> {
        self.warps
            .iter()
            .position(|w| -minkowski_dot(x, w.centre) < w.reach.cosh())
    }

    /// `find_warp_entry` finds where a straight ray first enters a warp's reach sphere.
    ///
    /// Example: `let (s_end, entering) = world.find_warp_entry(x, v, 12.0);`
    fn find_warp_entry(&self, x: DVec4, v: DVec4, s_max: f64) -> (f64, Option<usize>) {
        let (mut s_end, mut entering) = (s_max, None);
        for (k, w) in self.warps.iter().enumerate() {
            let (a, b) = (-minkowski_dot(x, w.centre), minkowski_dot(v, w.centre));
            let (closest_sq, reach_cosh) = (a * a - b * b, w.reach.cosh());
            if b <= 0.0 || closest_sq >= reach_cosh * reach_cosh {
                continue;
            }
            let s_in: f64 =
                ((b / a).atanh() - (reach_cosh / closest_sq.max(1.0).sqrt()).acosh()).max(0.0);
            if s_in < s_end {
                (s_end, entering) = (s_in, Some(k));
            }
        }
        (s_end, entering)
    }

    /// `cross_throat` carries a point inside mouth w's throat, and a tangent at it, to the partner.
    ///
    /// Example: `let (x, v) = world.cross_throat(1, x, v);`
    fn cross_throat(&self, w: usize, x: DVec4, v: DVec4) -> (DVec4, DVec4) {
        let warp: &VoxelWarp = &self.warps[w];
        let partner: &VoxelWarp = &self.warps[warp.partner];
        let (p, t) = (warp.inverse * x, warp.inverse * v);
        let len: f64 = p.truncate().length().max(TINY);
        let n: DVec3 = p.truncate() / len;
        let rho_out: f64 = warp.m * warp.m / len.asinh();
        let flip: DVec3 = DVec3::new(1.0, 1.0, -1.0);
        let mut tt: DVec3 = t.truncate() - t.w / (1.0 + p.w) * p.truncate();
        tt = (tt - 2.0 * tt.dot(n) * n) * flip;
        let q: DVec4 = (n * flip * rho_out.sinh()).extend(rho_out.cosh());
        let g: f64 = q.truncate().dot(tt) / (1.0 + q.w);
        let x_out: DVec4 = partner.frame * q;
        (
            x_out,
            normalize_tangent(
                x_out,
                partner.frame * (tt + g * q.truncate()).extend(g * (1.0 + q.w)),
            ),
        )
    }

    /// `trace_structure` finds where a chord first enters a solid cell of structure k, by a voxel walk.
    ///
    /// Example: `let s = world.trace_structure(3, x, v, 0.0, 1.0);`
    fn trace_structure(&self, k: usize, x: DVec4, v: DVec4, s_min: f64, s_max: f64) -> Option<f64> {
        let st: &VoxelStructure = &self.structures[k];
        let (a, b) = (-minkowski_dot(x, st.centre), minkowski_dot(v, st.centre));
        let (closest_sq, bound_cosh) = (a * a - b * b, st.radius.cosh());
        if closest_sq >= bound_cosh * bound_cosh {
            return None;
        }
        let s_star: f64 = (b / a).atanh();
        let half_window: f64 = (bound_cosh / closest_sq.max(1.0).sqrt()).acosh();
        let (s_lo, s_hi) = (
            s_min.max(s_star - half_window),
            s_max.min(s_star + half_window),
        );
        if s_lo >= s_hi {
            return None;
        }
        let p: DVec4 = st.inverse * geodesic_point(x, v, s_star);
        let t: DVec4 = st.inverse * geodesic_tangent(x, v, s_star);
        let mu_of = |s: f64| -> f64 {
            let lam: f64 = (s - s_star).tanh();
            lam / (p.w * (p.w + lam * t.w))
        };
        let scale: f64 = self.r / st.cell_m;
        let g0: [f64; 3] = [0, 1, 2].map(|i| p[i] / p.w * scale + st.origin[i]);
        let d: [f64; 3] = [0, 1, 2].map(|i| {
            let c: f64 = (t[i] * p.w - p[i] * t.w) * scale;
            if c.abs() < TINY { TINY } else { c }
        });
        let (mut mu_in, mut mu_out) = (mu_of(s_lo), mu_of(s_hi));
        for i in 0..3 {
            let (m0, m1) = (-g0[i] / d[i], (st.size[i] as f64 - g0[i]) / d[i]);
            mu_in = mu_in.max(m0.min(m1));
            mu_out = mu_out.min(m0.max(m1));
        }
        if mu_in >= mu_out {
            return None;
        }
        let mut cell: [i64; 3] = [0, 1, 2].map(|i| {
            let nudge: f64 = if d[i] > 0.0 { 1e-9 } else { -1e-9 };
            ((g0[i] + d[i] * mu_in + nudge).floor() as i64).clamp(0, st.size[i] as i64 - 1)
        });
        let step: [i64; 3] = d.map(|c| if c > 0.0 { 1 } else { -1 });
        let mut mu_next: [f64; 3] =
            [0, 1, 2].map(|i| (cell[i] as f64 + if d[i] > 0.0 { 1.0 } else { 0.0 } - g0[i]) / d[i]);
        let mu_delta: [f64; 3] = d.map(|c| (1.0 / c).abs());
        let mut mu: f64 = mu_in;
        for _ in 0..st.size.iter().sum::<usize>() + 3 {
            let index: usize =
                cell[0] as usize + st.size[0] * (cell[1] as usize + st.size[1] * cell[2] as usize);
            if self.voxels[st.offset + index] != 0 {
                let lam: f64 = mu * p.w * p.w / (1.0 - mu * p.w * t.w);
                return Some(s_star + lam.atanh());
            }
            let i: usize = if mu_next[0] < mu_next[1] {
                if mu_next[0] < mu_next[2] { 0 } else { 2 }
            } else if mu_next[1] < mu_next[2] {
                1
            } else {
                2
            };
            mu = mu_next[i];
            cell[i] += step[i];
            mu_next[i] += mu_delta[i];
            if mu > mu_out || cell[i] < 0 || cell[i] >= st.size[i] as i64 {
                return None;
            }
        }
        None
    }

    /// `trace_scene` finds the first thing a chord hits: the ground or a block (among all
    /// structures, or warp w's list).
    ///
    /// Example: `let hit = world.trace_scene(x, v, 1.0, None);`
    fn trace_scene(&self, x: DVec4, v: DVec4, s_max: f64, warp: Option<usize>) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        if x.y >= 0.0 && v.y < 0.0 && x.y / -v.y < 1.0 {
            let s: f64 = (x.y / -v.y).atanh();
            if s < s_max {
                best = Some(Hit { structure: None, s });
            }
        }
        let all: Vec<usize>;
        let candidates: &[usize] = match warp {
            Some(w) => &self.warps[w].list,
            None => {
                all = (0..self.structures.len()).collect();
                &all
            }
        };
        for &k in candidates {
            let limit: f64 = best.map_or(s_max, |hit| hit.s);
            if let Some(s) = self.trace_structure(k, x, v, 0.0, limit) {
                best = Some(Hit {
                    structure: Some(k),
                    s,
                });
            }
        }
        best
    }

    /// `trace_laser` follows a shot from `camera`'s eye along its crosshair: straight between warps,
    /// bent inside them, through throats, to a block, the ground, a horizon or `LASER_MAX_LENGTH_M`.
    /// The drawn beam starts at the muzzle (`MUZZLE_M` in camera space, where the window draws the
    /// tip of the gun in hand) and joins the path `LASER_MUZZLE_M` ahead,
    /// simplified into geodesic segments within `BEAM_TOLERANCE_M`.
    ///
    /// Example: `let shot = world.trace_laser(player.get_camera_frame(&world));`
    pub fn trace_laser(&self, camera: DMat4) -> Shot {
        let max_s: f64 = LASER_MAX_LENGTH_M / self.r;
        let (mut x, mut v) = (camera.col(AXIS_W), -camera.col(AXIS_Z));
        let (mut travelled, mut inside) = (0.0, self.find_warp_inside(x));
        let mut pieces: Vec<Vec<DVec4>> = vec![vec![x]];
        let (mut hit, mut through): (Option<Hit>, Vec<String>) = (None, Vec::new());
        for _ in 0..LASER_MAX_ITERATIONS {
            if travelled >= max_s {
                break;
            }
            let Some(w) = inside else {
                let (s_end, entering) = self.find_warp_entry(x, v, max_s - travelled);
                if let Some(found) = self.trace_scene(x, v, s_end, None) {
                    pieces
                        .last_mut()
                        .expect("a piece")
                        .push(geodesic_point(x, v, found.s));
                    travelled += found.s;
                    hit = Some(found);
                    break;
                }
                let xn: DVec4 = geodesic_point(x, v, s_end);
                v = geodesic_tangent(x, v, s_end);
                x = xn;
                travelled += s_end;
                pieces.last_mut().expect("a piece").push(x);
                if entering.is_none() {
                    break;
                }
                inside = entering;
                continue;
            };
            let ds: f64 = self.choose_bend_step(self.compute_bend(w, x, v).1);
            let (v0, x1, v1, rho) = self.step_bent(w, x, v, ds);
            if let Some(found) = self.trace_scene(x, v0, ds, Some(w)) {
                pieces
                    .last_mut()
                    .expect("a piece")
                    .push(geodesic_point(x, v0, found.s));
                travelled += found.s;
                hit = Some(found);
                break;
            }
            (x, v) = (x1, v1);
            travelled += ds;
            pieces.last_mut().expect("a piece").push(x);
            let warp: &VoxelWarp = &self.warps[w];
            if !warp.is_mouth && rho < warp.m * HORIZON_CAPTURE_FRAC {
                break;
            }
            if warp.is_mouth && rho < warp.m {
                (x, v) = self.cross_throat(w, x, v);
                through.push(warp.name.clone());
                pieces.push(vec![x]);
                inside = Some(warp.partner);
                continue;
            }
            if rho > warp.reach {
                inside = None;
            }
        }
        let gun: DVec4 = camera * map_exp(MUZZLE_M, self.r);
        let first: &mut Vec<DVec4> = &mut pieces[0];
        if first.len() > 1 && get_distance(first[0], first[1]) * self.r > LASER_MUZZLE_M {
            let t: DVec4 = first[1] + minkowski_dot(first[1], first[0]) * first[0];
            let muzzle: DVec4 =
                geodesic_point(first[0], normalize_minkowski(t), LASER_MUZZLE_M / self.r);
            first.splice(0..1, [gun, muzzle]);
        } else {
            first[0] = gun;
        }
        let spot: Option<DVec4> = hit.map(|_| {
            *pieces
                .last()
                .and_then(|p| p.last())
                .expect("a hit ends a piece")
        });
        Shot {
            segments: pieces
                .iter()
                .flat_map(|piece| self.simplify_path(piece))
                .collect(),
            spot,
            hit_structure: hit.and_then(|h| h.structure),
            is_hit: hit.is_some(),
            length_m: travelled * self.r,
            through,
        }
    }

    /// `simplify_path` turns a traced polyline into geodesic segments within `BEAM_TOLERANCE_M` of it.
    ///
    /// Example: `let segments = world.simplify_path(&points);`
    fn simplify_path(&self, points: &[DVec4]) -> Vec<BeamSegment> {
        let make_segment = |p: DVec4, q: DVec4| -> Option<BeamSegment> {
            let length: f64 = get_distance(p, q);
            (length > 1e-9).then(|| BeamSegment {
                a: p,
                b: (q - length.cosh() * p) / length.sinh(),
                length,
            })
        };
        let offset = |p: DVec4, segment: &BeamSegment| -> f64 {
            let perp: DVec4 = p + minkowski_dot(p, segment.a) * segment.a
                - minkowski_dot(p, segment.b) * segment.b;
            minkowski_dot(perp, perp).max(0.0).sqrt().asinh()
        };
        let mut segments: Vec<BeamSegment> = Vec::new();
        let mut i: usize = 0;
        while i + 1 < points.len() {
            let mut j: usize = i + 1;
            while j + 1 < points.len() {
                let Some(trial) = make_segment(points[i], points[j + 1]) else {
                    break;
                };
                if points[i + 1..=j]
                    .iter()
                    .any(|p| offset(*p, &trial) * self.r > BEAM_TOLERANCE_M)
                {
                    break;
                }
                j += 1;
            }
            if let Some(segment) = make_segment(points[i], points[j]) {
                segments.push(segment);
            }
            i = j;
        }
        segments
    }
}

/// `VoxelPlayer` is a player on a voxel map, as in the level: a body frame on the ground plane, the
/// eye's height above it, the view's pitch, the vertical speed, whether they fly or stand, their
/// sprint energy (1 full, 0 empty) and whether they sprinted on the last step, and the head bob's
/// phase and how much of it shows (0 standing still, 1 walking).
#[derive(Clone, Copy)]
pub struct VoxelPlayer {
    pub body: DMat4,
    pub h_m: f64,
    pub pitch_rad: f64,
    pub vertical_m_per_s: f64,
    pub is_flying: bool,
    pub is_on_ground: bool,
    pub energy_frac: f64,
    pub is_sprinting: bool,
    pub bob_phase_rad: f64,
    pub bob_frac: f64,
}

impl VoxelPlayer {
    /// `AtSpawn` makes a player standing at spawn point `index` (modulo the map's spawns), facing
    /// its way.
    ///
    /// Example: `let player = VoxelPlayer::AtSpawn(&world, 0);`
    #[allow(non_snake_case)]
    pub fn AtSpawn(world: &VoxelWorld, index: usize) -> VoxelPlayer {
        let feet: DMat4 = world.spawns[index % world.spawns.len()];
        let body: DMat4 = renormalize_frame(feet);
        VoxelPlayer {
            body,
            h_m: world.find_standing_height(body),
            pitch_rad: 0.0,
            vertical_m_per_s: 0.0,
            is_flying: false,
            is_on_ground: true,
            energy_frac: 1.0,
            is_sprinting: false,
            bob_phase_rad: 0.0,
            bob_frac: 0.0,
        }
    }

    /// `NearSpawn` makes player `id` standing near their team's spawn point (ids alternate between
    /// the map's spawns), clear of blocks, at the first slot at least `MIN_SPAWN_GAP_M` from every
    /// eye in `others`: slots go around rings `SPAWN_RING_STEP_M` apart from `SPAWN_RING_M`,
    /// `SPAWN_TURN_RAD` apart, starting from the id's own. If all are taken it uses the last.
    ///
    /// Example: `let player = VoxelPlayer::NearSpawn(&world, 3, &other_eyes);`
    #[allow(non_snake_case)]
    pub fn NearSpawn(world: &VoxelWorld, id: u32, others: &[DVec4]) -> VoxelPlayer {
        let spawn: DMat4 = VoxelPlayer::AtSpawn(world, id as usize).body;
        let slot_count: u32 = SPAWN_SLOTS_PER_RING * SPAWN_RING_COUNT;
        let mut player: Option<VoxelPlayer> = None;
        for k in 0..slot_count {
            let turn: DMat4 = make_rotation(AXIS_X, AXIS_Z, f64::from(id + k) * SPAWN_TURN_RAD);
            let ring_m: f64 =
                SPAWN_RING_M + SPAWN_RING_STEP_M * f64::from(k / SPAWN_SLOTS_PER_RING);
            let body: DMat4 = spawn * turn * make_boost(AXIS_Z, -ring_m / world.r) * turn.inverse();
            let candidate: VoxelPlayer = VoxelPlayer::Standing(world, body);
            let eye: DVec4 = candidate.get_camera_frame(world).col(AXIS_W);
            if others
                .iter()
                .all(|&x| get_distance(eye, x) * world.r >= MIN_SPAWN_GAP_M)
            {
                return candidate;
            }
            player = Some(candidate);
        }
        player.expect("there is at least one spawn slot")
    }

    /// `Standing` makes a player standing on `body` (a ground frame), clear of blocks.
    ///
    /// Example: `let player = VoxelPlayer::Standing(&world, body);`
    #[allow(non_snake_case)]
    pub fn Standing(world: &VoxelWorld, body: DMat4) -> VoxelPlayer {
        let body: DMat4 = renormalize_frame(body);
        VoxelPlayer {
            body,
            h_m: world.find_standing_height(body),
            pitch_rad: 0.0,
            vertical_m_per_s: 0.0,
            is_flying: false,
            is_on_ground: true,
            energy_frac: 1.0,
            is_sprinting: false,
            bob_phase_rad: 0.0,
            bob_frac: 0.0,
        }
    }

    /// `AtView` puts the player at Fermi coordinates (u, v), facing ground point (u', v'), with the eye
    /// `h_m` up (or standing, for `None`) and a pitch, as the level's viewpoints do.
    ///
    /// Example: `let player = VoxelPlayer::AtView(&world, [-107.0, 0.0], [0.0, 0.0], None, 0.0);`
    #[allow(non_snake_case)]
    pub fn AtView(
        world: &VoxelWorld,
        at_m: [f64; 2],
        target_m: [f64; 2],
        h_m: Option<f64>,
        pitch_rad: f64,
    ) -> VoxelPlayer {
        let make_frame = |[u_m, v_m]: [f64; 2], yaw_rad: f64| -> DMat4 {
            make_boost(AXIS_Z, -u_m / world.r)
                * make_boost(AXIS_X, v_m / world.r)
                * make_rotation(AXIS_X, AXIS_Z, yaw_rad)
        };
        let p: DVec4 =
            invert_lorentz(make_frame(at_m, 0.0)) * make_frame(target_m, 0.0).col(AXIS_W);
        let body: DMat4 = make_frame(at_m, p.x.atan2(-p.z));
        VoxelPlayer {
            body,
            h_m: h_m.unwrap_or_else(|| world.find_standing_height(body)),
            pitch_rad,
            vertical_m_per_s: 0.0,
            is_flying: h_m.is_some(),
            is_on_ground: h_m.is_none(),
            energy_frac: 1.0,
            is_sprinting: false,
            bob_phase_rad: 0.0,
            bob_frac: 0.0,
        }
    }

    /// `FromEye` makes the body of a player whose eye frame is `eye` (from the network), for drawing
    /// them: the body frame on the ground below the eye, facing their way.
    ///
    /// Example: `let feet = VoxelPlayer::FromEye(&world, eye).get_feet_frame(&world);`
    #[allow(non_snake_case)]
    pub fn FromEye(world: &VoxelWorld, eye: DMat4) -> VoxelPlayer {
        let x: DVec4 = eye.col(AXIS_W);
        let cosh_h: f64 = (1.0 + x.y * x.y).sqrt();
        let g: DVec4 = DVec4::new(x.x / cosh_h, 0.0, x.z / cosh_h, x.w / cosh_h);
        let up: DVec4 = x.y * g + cosh_h * DVec4::Y;
        let back: DVec4 = eye.col(AXIS_Z);
        let heading: DVec4 = back - minkowski_dot(back, up) * up;
        let body: DMat4 = if minkowski_dot(heading, heading) > TINY {
            make_ground_frame(g, heading)
        } else {
            renormalize_frame(eye)
        };
        VoxelPlayer {
            body,
            h_m: world.r * x.y.asinh(),
            pitch_rad: 0.0,
            vertical_m_per_s: 0.0,
            is_flying: false,
            is_on_ground: false,
            energy_frac: 1.0,
            is_sprinting: false,
            bob_phase_rad: 0.0,
            bob_frac: 0.0,
        }
    }

    /// `get_camera_frame` gives the eye's frame: the body, raised to the eye (with the head bob)
    /// and pitched.
    ///
    /// Example: `let camera = player.get_camera_frame(&world);`
    pub fn get_camera_frame(&self, world: &VoxelWorld) -> DMat4 {
        self.body
            * make_boost(AXIS_Y, (self.h_m + self.get_bob_m(world)) / world.r)
            * make_rotation(AXIS_Z, AXIS_Y, -self.pitch_rad)
    }

    /// `get_bob_m` gives the head bob: the eye dips at each footstep, deeper when sprinting, by
    /// `world`'s settings.
    ///
    /// Example: `let bob_m = player.get_bob_m(&world);`
    pub fn get_bob_m(&self, world: &VoxelWorld) -> f64 {
        let bob = &world.settings.head_bob;
        let depth_m: f64 = if self.is_sprinting {
            bob.sprint_depth_m
        } else {
            bob.walk_depth_m
        };
        -self.bob_frac * depth_m * self.bob_phase_rad.sin().abs()
    }

    /// `get_feet_frame` gives the frame at the player's feet (for their avatar).
    ///
    /// Example: `let feet = player.get_feet_frame(&world);`
    pub fn get_feet_frame(&self, world: &VoxelWorld) -> DMat4 {
        self.body * make_boost(AXIS_Y, (self.h_m - world.player.eye_height_m) / world.r)
    }

    /// `look` turns the view: yaw turns the body, pitch tilts the camera.
    ///
    /// Example: `player.look(12.0, -3.0);`
    pub fn look(&mut self, dx_px: f64, dy_px: f64) {
        self.body *= make_rotation(AXIS_X, AXIS_Z, dx_px * LOOK_RAD_PER_PX);
        self.pitch_rad =
            (self.pitch_rad - dy_px * LOOK_RAD_PER_PX).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);
    }

    /// `move_optically` moves the body `distance_m` along light paths (bent by warp w and through
    /// its throat), returning the new body, eye height and the mouth gone through, if any.
    ///
    /// Example: `let (body, h_m, crossed) = player.move_optically(&world, [0.0, -1.0], 0.1, 1);`
    fn move_optically(
        &self,
        world: &VoxelWorld,
        dir: [f64; 2],
        distance_m: f64,
        warp: usize,
    ) -> (DMat4, f64, Option<String>) {
        let c0: DMat4 = self.body * make_boost(AXIS_Y, self.h_m / world.r);
        let (mut x, mut t) = (c0.col(AXIS_W), c0 * DVec4::new(dir[0], 0.0, dir[1], 0.0));
        let (mut left, mut inside, mut crossed) = (distance_m / world.r, Some(warp), None);
        for _ in 0..LASER_MAX_ITERATIONS {
            if left <= TINY {
                break;
            }
            let Some(w) = inside else {
                let xn: DVec4 = geodesic_point(x, t, left);
                t = geodesic_tangent(x, t, left);
                x = xn;
                break;
            };
            let ds: f64 = left.min(world.choose_bend_step(world.compute_bend(w, x, t).1));
            let (_, x1, v1, rho) = world.step_bent(w, x, t, ds);
            (x, t) = (x1, v1);
            left -= ds;
            let mouth: &VoxelWarp = &world.warps[w];
            if mouth.is_mouth && rho < mouth.m {
                (x, t) = world.cross_throat(w, x, t);
                crossed = Some(mouth.name.clone());
                inside = Some(mouth.partner);
            } else if rho > mouth.reach {
                inside = None;
            }
        }
        let (cosh_h, sinh_h) = ((1.0 + x.y * x.y).sqrt(), x.y);
        let g: DVec4 = DVec4::new(x.x / cosh_h, 0.0, x.z / cosh_h, x.w / cosh_h);
        let up: DVec4 = sinh_h * g + cosh_h * DVec4::Y;
        let heading: DVec4 = normalize_minkowski(t - minkowski_dot(t, up) * up);
        let frame: DMat4 = make_ground_frame(g, -heading);
        let body: DMat4 =
            renormalize_frame(frame * make_rotation(AXIS_X, AXIS_Z, -dir[0].atan2(-dir[1])));
        (body, world.r * sinh_h.asinh(), crossed)
    }

    /// `step` moves the player one time step as the level does: walking or sprinting (or flying;
    /// sprinting only while there is energy, which refills only while `Shift` isn't held to move),
    /// moving along light paths inside a mouth's reach, falling into a black hole (back to spawn),
    /// colliding with blocks and the boundary (rising up to `MAX_WALK_RISE_M` where a floor's
    /// cells rise a little), jumping (the only way onto a block) and falling. A
    /// body that starts the step inside a block is first pushed out sideways, and lifted (as the
    /// level does) only if that fails. It returns messages for the HUD.
    ///
    /// Example: `for message in player.step(&world, &input, 1.0 / 60.0, 0) { ... }`
    pub fn step(
        &mut self,
        world: &VoxelWorld,
        input: &Input,
        dt_s: f64,
        spawn_index: usize,
    ) -> Vec<String> {
        let r: f64 = world.r;
        let forward: f64 = f64::from(u8::from(input.forward)) - f64::from(u8::from(input.back));
        let side: f64 = f64::from(u8::from(input.right)) - f64::from(u8::from(input.left));
        let len: f64 = forward.hypot(side);
        let settings: PlayerSettings = world.player;
        let (energy, bob) = (&world.settings.sprint_energy, &world.settings.head_bob);
        let wants_to_sprint: bool = input.sprint && len > 0.0;
        self.is_sprinting = wants_to_sprint && self.energy_frac > 0.0;
        self.energy_frac = (self.energy_frac
            + if wants_to_sprint {
                -dt_s / energy.drain_s
            } else {
                dt_s / energy.refill_s
            })
        .clamp(0.0, 1.0);
        let is_walking: bool = len > 0.0 && !self.is_flying && self.is_on_ground;
        let bob_target: f64 = f64::from(u8::from(is_walking));
        self.bob_frac += (bob_target - self.bob_frac).clamp(-dt_s / bob.fade_s, dt_s / bob.fade_s);
        if is_walking {
            let hz: f64 = if self.is_sprinting {
                bob.sprint_steps_per_s
            } else {
                bob.walk_steps_per_s
            };
            self.bob_phase_rad =
                (self.bob_phase_rad + std::f64::consts::TAU * hz * dt_s) % std::f64::consts::TAU;
        }
        let speed_m_per_s: f64 = match (self.is_flying, self.is_sprinting) {
            (true, true) => settings.fly_m_per_s * FLY_SPRINT_PER_FLY,
            (true, false) => settings.fly_m_per_s,
            (false, true) => settings.sprint_m_per_s,
            (false, false) => settings.walk_m_per_s,
        };
        let eye: DVec4 = (self.body * make_boost(AXIS_Y, self.h_m / r)).col(AXIS_W);
        let warp: Option<usize> = world.find_warp_inside(eye);
        if let Some(w) = warp
            && !world.warps[w].is_mouth
            && world.compute_bend(w, eye, self.body.col(AXIS_X)).1
                < FALL_IN_PER_HORIZON * world.warps[w].m
        {
            *self = VoxelPlayer::NearSpawn(world, spawn_index as u32, &[]);
            return vec!["You fell into the black hole".to_string()];
        }
        let (mut body_new, mut h_new, mut crossed) = (self.body, self.h_m, None);
        let has_moved: bool = len > 0.0;
        if has_moved {
            let dir: [f64; 2] = [side / len, -forward / len];
            let distance_m: f64 = speed_m_per_s * dt_s;
            if let Some(w) = warp.filter(|&w| world.warps[w].is_mouth) {
                (body_new, h_new, crossed) = self.move_optically(world, dir, distance_m, w);
            } else {
                body_new = renormalize_frame(
                    self.body
                        * make_boost(AXIS_Z, dir[1] * distance_m / r)
                        * make_boost(AXIS_X, dir[0] * distance_m / r),
                );
            }
        }
        let messages: Vec<String> = crossed
            .iter()
            .map(|name| format!("Through {name}"))
            .collect();
        if self.is_flying {
            (self.body, self.h_m) = (body_new, h_new);
            let climb_m_per_s: f64 = MIN_FLY_CLIMB_M_PER_S.max(self.h_m) * FLY_CLIMB_PER_S;
            if input.up {
                self.h_m += climb_m_per_s * dt_s;
            }
            if input.down {
                self.h_m -= climb_m_per_s * dt_s;
            }
            self.h_m = self.h_m.clamp(MIN_FLY_HEIGHT_M, MAX_FLY_HEIGHT_M);
            return messages;
        }
        let nearby: Vec<usize> =
            world.find_nearby_structures(self.body.col(AXIS_W), NEARBY_REACH_M);
        if world.is_body_blocked(self.body, self.h_m, &nearby)
            && let Some(body) = world.push_out_sideways(self.body, self.h_m, &nearby)
        {
            self.body = body;
        }
        for _ in 0..UNSTICK_MAX_STEPS {
            if !world.is_body_blocked(self.body, self.h_m, &nearby) {
                break;
            }
            self.h_m += UNSTICK_STEP_M;
            self.vertical_m_per_s = 0.0;
        }
        if crossed.is_some() {
            (self.body, self.h_m) = (body_new, h_new);
        } else if has_moved && !world.is_outside_boundary(body_new.col(AXIS_W)) {
            let nearby_new: Vec<usize> =
                world.find_nearby_structures(body_new.col(AXIS_W), NEARBY_REACH_M);
            if !world.is_body_blocked(body_new, h_new, &nearby_new) {
                (self.body, self.h_m) = (body_new, h_new);
            } else if self.is_on_ground
                && let Some(rise_m) = world.find_walk_rise_m(body_new, h_new, &nearby_new)
            {
                (self.body, self.h_m) = (body_new, h_new + rise_m);
            }
        }
        if input.up && self.is_on_ground {
            self.vertical_m_per_s = settings.jump_m_per_s;
        }
        self.vertical_m_per_s =
            (self.vertical_m_per_s - world.gravity_m_per_s2 * dt_s).max(-MAX_FALL_SPEED_M_PER_S);
        let nearby_now: Vec<usize> =
            world.find_nearby_structures(self.body.col(AXIS_W), NEARBY_REACH_M);
        let (h0, dh) = (self.h_m, self.vertical_m_per_s * dt_s);
        self.h_m = h0 + dh;
        self.is_on_ground = false;
        if world.is_body_blocked(self.body, self.h_m, &nearby_now) {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..LANDING_BISECTIONS {
                let mid: f64 = (lo + hi) / 2.0;
                if world.is_body_blocked(self.body, h0 + dh * mid, &nearby_now) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            self.h_m = h0 + dh * lo;
            self.is_on_ground = dh < 0.0;
            self.vertical_m_per_s = 0.0;
        }
        messages
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the voxel world against the Lunar Surface level's own numbers.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Where a player starts | on a structure (the blue lander), on the ground (a viewpoint) | `players_stand_where_the_level_puts_them` |
    //! | Laser path | through a wormhole to a structure | `the_laser_into_the_wormhole_hits_the_red_pylon` |
    //! | Players spawning | up to the player limit, and a late id whose own slot is next to player 1's (it moves on) | `players_spawn_apart` |
    //! | Beam clipped to | its middle (through the wormhole), before its start, past its end | `a_clipped_beam_keeps_its_middle` |
    //! | Obstacle walked into | one block (stops the player), one block while jumping (climbed), a wall turned along after touching it at an angle (not climbed) | `walking_into_a_block_does_not_climb_it`, `jumping_climbs_onto_a_block`, `turning_along_a_wall_does_not_climb_it` |
    //! | Jump on open ground | rises the settings' jump height and lands in their fraction of the map's jump time | `a_jump_is_low_and_quick` |
    //! | Sprint energy | full (sprints at sprint speed, drains), empty with `Shift` held (walks, stays empty), `Shift` released (refills) | `sprinting_uses_energy` |
    //! | Floor walked on | the ground, a structure's deck above the ground (its cells' tops rise a little against the ground's height) | `walking_into_a_block_does_not_climb_it` (walks on the ground first), `walking_on_a_deck_moves` |

    use super::*;
    use crate::map::{Map, get_map_folder};

    const LANDER_DECK_M: f64 = 1.0;
    const STAND_TOLERANCE_M: f64 = 0.06;
    const PYLON_DISTANCE_M: f64 = 42.4;
    const DISTANCE_TOLERANCE_M: f64 = 0.5;
    const WALL_CELLS: usize = 3;
    const CLEAR_CELLS: usize = 4;
    const START_OUT_CELLS: f64 = 0.6;
    const DIAGONAL_CELLS: f64 = 1.5;
    const STEP_S: f64 = 1.0 / 60.0;
    const WALK_STEP_COUNT: usize = 90;
    const MAX_RISE_ALONG_WALL_M: f64 = 0.5;
    const MIN_RISE_ONTO_BLOCK_M: f64 = 0.9;
    const MAX_RISE_ONTO_BLOCK_M: f64 = 1.2;
    const DECK_WALK_STEP_COUNT: usize = 30;
    const MIN_DECK_WALK_FRAC: f64 = 0.5;
    const QUARTER_TURN_RAD: f64 = std::f64::consts::FRAC_PI_4;
    const EMPTY_SPRINT_STEP_COUNT: usize = 10;
    const FAR_ID_NEAR_ONE: u32 = 35;
    const MAX_JUMP_STEP_COUNT: usize = 120;
    const JUMP_HEIGHT_TOLERANCE_M: f64 = 0.1;
    const JUMP_TIME_TOLERANCE_S: f64 = 0.05;

    /// `find_columns` finds, on structures standing upright on the ground, columns of cells (x, z)
    /// solid from the ground up to `solid_cells` (and, with `is_top_open`, empty for
    /// `CLEAR_CELLS` above), the same at (x, z + 1), with the `CLEAR_CELLS` columns beyond their +x
    /// faces empty from the ground up (outside the grid counts as empty). For each it gives a map
    /// point on the ground `START_OUT_CELLS` cells out from the face and one a player there faces,
    /// toward the face and `along_cells` cells along it (map points are (u, v) in meters, as
    /// `VoxelPlayer::AtView` takes them).
    ///
    /// Example: `let walls = find_columns(&world, 3, false, 1.5);`
    fn find_columns(
        world: &VoxelWorld,
        solid_cells: usize,
        is_top_open: bool,
        along_cells: f64,
    ) -> Vec<([f64; 2], [f64; 2])> {
        let height: usize = solid_cells + CLEAR_CELLS;
        let mut found: Vec<([f64; 2], [f64; 2])> = Vec::new();
        for st in &world.structures[..world.static_structure_count] {
            let frame: DMat4 = invert_lorentz(st.inverse);
            let is_upright: bool = frame.col(AXIS_W).y.abs() < TINY
                && (frame.col(AXIS_Y) - DVec4::Y).length() < TINY
                && st.origin[1] == 0.0;
            if !is_upright {
                continue;
            }
            let [nx, ny, nz] = st.size;
            let is_solid = |x: usize, y: usize, z: usize| {
                x < nx && y < ny && z < nz && world.voxels[st.offset + x + nx * (y + ny * z)] != 0
            };
            let to_map = |g: DVec3| -> [f64; 2] {
                let p: DVec4 =
                    frame * map_klein((g - DVec3::from_array(st.origin)) * st.cell_m / world.r);
                let v_m: f64 = world.r * p.x.asinh();
                [-world.r * (p.z / (v_m / world.r).cosh()).asinh(), v_m]
            };
            for z in 0..nz.saturating_sub(1) {
                for x in 0..nx {
                    let is_column = |z: usize| {
                        (0..solid_cells).all(|y| is_solid(x, y, z))
                            && (!is_top_open || (solid_cells..height).all(|y| !is_solid(x, y, z)))
                    };
                    let is_clear: bool = (1..=CLEAR_CELLS).all(|dx| {
                        (0..height).all(|y| !is_solid(x + dx, y, z) && !is_solid(x + dx, y, z + 1))
                    });
                    if is_column(z) && is_column(z + 1) && is_clear {
                        let face: f64 = (x + 1) as f64;
                        let at_m: [f64; 2] =
                            to_map(DVec3::new(face + START_OUT_CELLS, 0.0, z as f64 + 0.5));
                        let target_m: [f64; 2] = to_map(DVec3::new(
                            face + START_OUT_CELLS - DIAGONAL_CELLS,
                            0.0,
                            z as f64 + 0.5 + along_cells,
                        ));
                        found.push((at_m, target_m));
                    }
                }
            }
        }
        found
    }

    /// `place_on_ground` makes a player at the first of `spots` (from `find_columns`) where they
    /// stand on the ground, not on a block.
    ///
    /// Example: `let player = place_on_ground(&world, &walls);`
    fn place_on_ground(world: &VoxelWorld, spots: &[([f64; 2], [f64; 2])]) -> VoxelPlayer {
        spots
            .iter()
            .map(|&(at_m, target_m)| VoxelPlayer::AtView(world, at_m, target_m, None, 0.0))
            .find(|player| (player.h_m - world.player.eye_height_m).abs() < STAND_TOLERANCE_M)
            .expect("a spot on the ground beside the column")
    }

    /// `walk_forward` steps a player walking forward `count` times and gives the highest eye height
    /// reached.
    ///
    /// Example: `let top_m = walk_forward(&world, &mut player, 90);`
    fn walk_forward(world: &VoxelWorld, player: &mut VoxelPlayer, count: usize) -> f64 {
        let input: Input = Input {
            forward: true,
            ..Input::default()
        };
        (0..count).fold(player.h_m, |top_m, _| {
            player.step(world, &input, STEP_S, 0);
            top_m.max(player.h_m)
        })
    }

    /// `load_lunar_surface` loads the shipped Lunar Surface map as a voxel world.
    ///
    /// Example: `let world = load_lunar_surface();`
    fn load_lunar_surface() -> VoxelWorld {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        VoxelWorld::FromMap(&map).expect("a voxel world")
    }

    #[test]
    fn players_stand_where_the_level_puts_them() {
        let world: VoxelWorld = load_lunar_surface();
        let eye_m: f64 = world.player.eye_height_m;
        let on_lander: VoxelPlayer = VoxelPlayer::AtSpawn(&world, 0);
        assert!((on_lander.h_m - (LANDER_DECK_M + eye_m)).abs() < STAND_TOLERANCE_M);
        let on_ground: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        assert!((on_ground.h_m - eye_m).abs() < STAND_TOLERANCE_M);
    }

    #[test]
    fn the_laser_into_the_wormhole_hits_the_red_pylon() {
        let world: VoxelWorld = load_lunar_surface();
        let player: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-22.0, 40.0], [-14.0, 40.0], None, 0.0);
        let shot: Shot = world.trace_laser(player.get_camera_frame(&world));
        assert_eq!(shot.through, vec!["wormhole mouth A".to_string()]);
        let hit: usize = shot.hit_structure.expect("the laser hits a structure");
        assert_eq!(world.structures[hit].name, "red pylon");
        assert!((shot.length_m - PYLON_DISTANCE_M).abs() < DISTANCE_TOLERANCE_M);
    }

    #[test]
    fn players_spawn_apart() {
        let world: VoxelWorld = load_lunar_surface();
        let mut eyes: Vec<DVec4> = Vec::new();
        for id in (1..=crate::net::MAX_PLAYER_COUNT as u32).chain([FAR_ID_NEAR_ONE]) {
            let player: VoxelPlayer = VoxelPlayer::NearSpawn(&world, id, &eyes);
            let nearby: Vec<usize> =
                world.find_nearby_structures(player.body.col(AXIS_W), NEARBY_REACH_M);
            assert!(
                !world.is_body_blocked(player.body, player.h_m, &nearby),
                "player {id}"
            );
            let eye: DVec4 = player.get_camera_frame(&world).col(AXIS_W);
            for other in &eyes {
                assert!(
                    get_distance(eye, *other) * world.r >= MIN_SPAWN_GAP_M,
                    "player {id}"
                );
            }
            eyes.push(eye);
        }
    }

    #[test]
    fn a_clipped_beam_keeps_its_middle() {
        let world: VoxelWorld = load_lunar_surface();
        let player: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-22.0, 40.0], [-14.0, 40.0], None, 0.0);
        let shot: Shot = world.trace_laser(player.get_camera_frame(&world));
        let length_m = |segments: &[BeamSegment]| -> f64 {
            segments.iter().map(|s| s.length).sum::<f64>() * world.r
        };
        let full_m: f64 = length_m(&shot.segments);
        let (from_m, to_m) = (full_m / 4.0, full_m / 2.0);
        let middle: Vec<BeamSegment> = clip_beam(&shot.segments, world.r, from_m, to_m);
        assert!((length_m(&middle) - (to_m - from_m)).abs() < DISTANCE_TOLERANCE_M);
        assert!(clip_beam(&shot.segments, world.r, -2.0, -1.0).is_empty());
        assert!(clip_beam(&shot.segments, world.r, full_m + 1.0, full_m + 2.0).is_empty());
    }

    #[test]
    fn walking_into_a_block_does_not_climb_it() {
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer = place_on_ground(&world, &find_columns(&world, 1, true, 0.0));
        let start_m: f64 = player.h_m;
        assert!(
            walk_forward(&world, &mut player, WALK_STEP_COUNT) - start_m < MAX_RISE_ALONG_WALL_M
        );
    }

    #[test]
    fn jumping_climbs_onto_a_block() {
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer = place_on_ground(&world, &find_columns(&world, 1, true, 0.0));
        let start_m: f64 = player.h_m;
        let input: Input = Input {
            forward: true,
            up: true,
            ..Input::default()
        };
        let has_stood_on_block: bool = (0..WALK_STEP_COUNT).any(|_| {
            player.step(&world, &input, STEP_S, 0);
            let rise_m: f64 = player.h_m - start_m;
            player.is_on_ground && rise_m > MIN_RISE_ONTO_BLOCK_M && rise_m < MAX_RISE_ONTO_BLOCK_M
        });
        assert!(has_stood_on_block);
    }

    #[test]
    fn turning_along_a_wall_does_not_climb_it() {
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer = place_on_ground(
            &world,
            &find_columns(&world, WALL_CELLS, false, DIAGONAL_CELLS),
        );
        let start_m: f64 = player.h_m;
        walk_forward(&world, &mut player, WALK_STEP_COUNT);
        player.look(QUARTER_TURN_RAD / LOOK_RAD_PER_PX, 0.0);
        let top_m: f64 = walk_forward(&world, &mut player, WALK_STEP_COUNT);
        assert!(
            top_m - start_m < MAX_RISE_ALONG_WALL_M,
            "rose {} m along a wall",
            top_m - start_m
        );
    }

    #[test]
    fn walking_on_a_deck_moves() {
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer = VoxelPlayer::AtSpawn(&world, 0);
        let start: DVec4 = player.body.col(AXIS_W);
        walk_forward(&world, &mut player, DECK_WALK_STEP_COUNT);
        let moved_m: f64 = get_distance(start, player.body.col(AXIS_W)) * world.r;
        let expected_m: f64 = world.player.walk_m_per_s * STEP_S * DECK_WALK_STEP_COUNT as f64;
        assert!(
            moved_m > MIN_DECK_WALK_FRAC * expected_m,
            "moved {moved_m} m of {expected_m} m"
        );
    }

    #[test]
    fn sprinting_uses_energy() {
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        let sprint: Input = Input {
            forward: true,
            sprint: true,
            ..Input::default()
        };
        player.step(&world, &sprint, STEP_S, 0);
        assert!(player.is_sprinting && player.energy_frac < 1.0);
        player.energy_frac = 0.0;
        for _ in 0..EMPTY_SPRINT_STEP_COUNT {
            player.step(&world, &sprint, STEP_S, 0);
            assert!(!player.is_sprinting && player.energy_frac == 0.0);
        }
        let walk: Input = Input {
            sprint: false,
            ..sprint
        };
        player.step(&world, &walk, STEP_S, 0);
        assert!(!player.is_sprinting && player.energy_frac > 0.0);
        let drain_s: f64 = world.settings.sprint_energy.drain_s;
        let full_drain_steps: usize = (drain_s / STEP_S).ceil() as usize;
        player.energy_frac = 1.0;
        for _ in 0..full_drain_steps {
            player.step(
                &world,
                &Input {
                    sprint: true,
                    ..sprint
                },
                STEP_S,
                0,
            );
        }
        assert!(player.energy_frac < STEP_S / drain_s);
    }

    #[test]
    fn a_jump_is_low_and_quick() {
        let map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the map loads");
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        let start_m: f64 = player.h_m;
        let jump: Input = Input {
            up: true,
            ..Input::default()
        };
        player.step(&world, &jump, STEP_S, 0);
        let mut apex_m: f64 = player.h_m - start_m;
        let mut air_step_count: usize = 1;
        while !player.is_on_ground {
            assert!(
                air_step_count < MAX_JUMP_STEP_COUNT,
                "the jump never landed"
            );
            player.step(&world, &Input::default(), STEP_S, 0);
            apex_m = apex_m.max(player.h_m - start_m);
            air_step_count += 1;
        }
        let map_jump_s: f64 = 2.0 * map.player.jump_m_per_s / map.gravity_m_per_s2;
        let jump_s: f64 = air_step_count as f64 * STEP_S;
        assert!(
            (apex_m - world.settings.movement.jump_height_m).abs() < JUMP_HEIGHT_TOLERANCE_M,
            "apex {apex_m} m"
        );
        assert!(
            (jump_s - map_jump_s * world.settings.movement.jump_time_per_map_jump_time).abs()
                < JUMP_TIME_TOLERANCE_S,
            "jump took {jump_s} s"
        );
    }
}
