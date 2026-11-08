//! `voxel_world.rs` is a voxel map of the portable format (`common/maps/`, such as Lunar Surface) on
//! the CPU, ported from the Lunar Surface level's JavaScript: structures with bounding spheres, warp
//! objects with the structures in their reach, a voxel walk, the laser's light path, and walking on
//! the map (collisions, stepping up blocks, gravity, jumping, flying, and moving along light paths
//! near a wormhole mouth). `render_voxel.metal` draws the same world, with `voxel_clearance.rs`'s grids.
//!
//! Interface: `VoxelWorld::FromMap`, `VoxelWorld::with_avatars`, `VoxelWorld::light_consoles`,
//! `VoxelWorld::find_aimed_material`, `VoxelWorld::is_sunlit`, `Console`, `VoxelPlayer`, `StepReport`,
//! `Shot`, `BeamSegment`.
//!
//! Example: `let world = VoxelWorld::FromMap(&Map::Load(&get_map_folder("lunar_surface"))?)?;`

use std::sync::Arc;

use glam::{DMat4, DVec3, DVec4};

use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, invert_lorentz, make_boost, make_rotation,
    minkowski_dot, normalize_tangent, transport_from_centre,
};
use crate::map::{Map, PlayerSettings, to_direction, to_frame, to_point};
use crate::net::{MAX_CONSOLE_COUNT, TEAM_NAMES};
use crate::physics::Input;
use crate::settings::{HeadBobSettings, PerPlayerSettings, ServerSettings};
use crate::shapes::{Shape, ShapeKind, find_hit_mu};
use crate::terrain::{Terrain, get_fermi_coordinates};
use crate::voxel_clearance::ClearanceGrids;
use crate::voxel_textures::get_material_surface;

pub const MAX_MATERIAL_COUNT: usize = 256;
pub const AVATAR_FIRST_MATERIAL: usize = 240;
const BOUND_MARGIN_M: f64 = 0.2;
pub(crate) const BEND_STEP_FRAC: f64 = 0.08;
const CONSOLE_PULSE_HZ: f64 = 1.0;
const CONSOLE_FRENZY_PULSE_HZ: f64 = 8.0;
const CONSOLE_PULSE_EASE_TIME_CONSTANT_S: f64 = 0.3;
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
const WALK_RISE_STEP_M: f64 = 0.02;
const SUN_SHADOW_M: f64 = 80.0;
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
const MAX_DRAWN_BEAM_DISTANCE_PER_CURVATURE_RADIUS: f64 = 4.0;
const BEAM_CLIP_BISECTION_COUNT: usize = 40;
pub const LOOK_RAD_PER_PX: f64 = 0.0025;
const MAX_PITCH_RAD: f64 = 1.55;
const TINY: f64 = 1e-12;
const AVATAR_HEAD_FROM_M: f64 = 1.5;
const AVATAR_SHAPES: [Shape; 2] = [
    Shape {
        kind: ShapeKind::Cylinder,
        centre: DVec3::new(0.0, 0.75, 0.0),
        half: DVec3::new(0.25, 0.75, 0.25),
        round_m: 0.2,
        axis: 1,
        material: 0,
    },
    Shape {
        kind: ShapeKind::Ellipsoid,
        centre: DVec3::new(0.0, 1.75, 0.0),
        half: DVec3::new(0.22, 0.25, 0.22),
        round_m: 0.0,
        axis: 1,
        material: 0,
    },
];

/// `VoxelStructure` is one structure: its frame (and inverse), its grid, where its cells start in
/// the voxel bytes, and its bounding sphere (radius in curvature units).
#[derive(Clone)]
pub struct VoxelStructure {
    pub name: String,
    pub inverse: DMat4,
    pub first_shape: usize,
    pub shape_count: usize,
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
/// `TEAM_NAMES`), the map's colour for it, and its pulse.
#[derive(Clone, Copy)]
pub struct Console {
    pub material: u8,
    pub team: u32,
    color: [f32; 3],
    pulse: ConsolePulse,
}

/// `ConsolePulse` is a console's pulse at `time_s`: how far through one pulse it is
/// (`phase_frac`, from 0 to 1) and how many pulses a second it makes now (`hz`).
#[derive(Clone, Copy)]
struct ConsolePulse {
    phase_frac: f64,
    hz: f64,
    time_s: f64,
}

impl ConsolePulse {
    /// `Resting` is a pulse at the normal rate, `CONSOLE_PULSE_HZ`, at the start of a pulse at
    /// time 0.
    ///
    /// Example: `let pulse = ConsolePulse::Resting();`
    #[allow(non_snake_case)]
    fn Resting() -> ConsolePulse {
        ConsolePulse {
            phase_frac: 0.0,
            hz: CONSOLE_PULSE_HZ,
            time_s: 0.0,
        }
    }

    /// `advance` moves the pulse on to `time_s`, toward the rate `target_hz`: the rate rises to a
    /// higher target at once, and eases down to a lower one exponentially with the time constant
    /// `CONSOLE_PULSE_EASE_TIME_CONSTANT_S` (within 4% of the way in 1 s). The phase adds up the
    /// rate over the time, so it never jumps when the rate changes. `advance` panics if `time_s`
    /// is before the pulse's own time.
    ///
    /// Example: `console.pulse = console.pulse.advance(get_console_pulse_hz(0.5), 12.5);`
    fn advance(self, target_hz: f64, time_s: f64) -> ConsolePulse {
        let dt: f64 = time_s - self.time_s;
        assert!(
            dt >= 0.0,
            "ConsolePulse::advance: time_s {time_s} is before the pulse's {}",
            self.time_s
        );
        let hz: f64 = if target_hz >= self.hz {
            target_hz
        } else {
            target_hz + (self.hz - target_hz) * (-dt / CONSOLE_PULSE_EASE_TIME_CONSTANT_S).exp()
        };
        ConsolePulse {
            phase_frac: (self.phase_frac + hz * dt).fract(),
            hz,
            time_s,
        }
    }

    /// `get_level_frac` gives the console's brightness now, as a fraction of its colour: a sine
    /// of the phase between `CONSOLE_PULSE_MIN_FRAC` and `CONSOLE_PULSE_MAX_FRAC`.
    ///
    /// Example: `let level = console.pulse.get_level_frac();`
    fn get_level_frac(self) -> f32 {
        let wave: f32 = (0.5 + 0.5 * (std::f64::consts::TAU * self.phase_frac).sin()) as f32;
        CONSOLE_PULSE_MIN_FRAC + (CONSOLE_PULSE_MAX_FRAC - CONSOLE_PULSE_MIN_FRAC) * wave
    }
}

/// `get_console_pulse_hz` gives how many times a second a console pulses when it is
/// `disabling_frac` of the way to being turned off (0 when nobody holds `E` on it): from
/// `CONSOLE_PULSE_HZ` at 0 up to `CONSOLE_FRENZY_PULSE_HZ` at 1, by the same factor for each
/// equal step of progress, so it speeds up fastest right before the console turns off. It panics
/// if `disabling_frac` isn't between 0 and 1.
///
/// Example: `let hz = get_console_pulse_hz(0.5);`
fn get_console_pulse_hz(disabling_frac: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&disabling_frac),
        "get_console_pulse_hz: disabling_frac must be between 0 and 1, got {disabling_frac}"
    );
    CONSOLE_PULSE_HZ * (CONSOLE_FRENZY_PULSE_HZ / CONSOLE_PULSE_HZ).powf(disabling_frac)
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
                pulse: ConsolePulse::Resting(),
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

/// `VoxelWorld` is a whole voxel map, ready to trace and to walk on, with the eye of the flashlight
/// shining this frame and the time the sky shows (`sky_time_s`: the UTC clock in the window, 0 for
/// fixed views; `spaceships.rs`).
#[derive(Clone)]
pub struct VoxelWorld {
    pub name: String,
    pub r: f64,
    pub structures: Vec<VoxelStructure>,
    pub shapes: Vec<Shape>,
    pub warps: Vec<VoxelWarp>,
    pub lights: Vec<(DVec4, [f32; 3], f64)>,
    pub palette: Vec<[f32; 4]>,
    pub surfaces: Vec<[f32; 4]>,
    pub special: SpecialMaterials,
    pub regions: Option<[f64; 3]>,
    pub terrain: Option<Terrain>,
    pub sun: DVec3,
    pub planet: Option<(DVec3, f64)>,
    pub player: PlayerSettings,
    pub gravity_m_per_s2: f64,
    map_player: PlayerSettings,
    map_gravity_m_per_s2: f64,
    pub settings: ServerSettings,
    pub head_bob: HeadBobSettings,
    pub spawns: Vec<DMat4>,
    pub static_structure_count: usize,
    pub consoles: Vec<Console>,
    pub flashlight_eye: Option<DMat4>,
    pub sky_time_s: f64,
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

/// `clip_beam_to_map` gives a beam up to where it first gets farther than
/// `MAX_DRAWN_BEAM_DISTANCE_PER_CURVATURE_RADIUS` curvature radii from the map's origin. Past
/// there, out over the sky, the shader's single-precision maths of a beam's glow turns to noise
/// (coordinates grow as cosh of that distance) that floods the sky; the map lies well inside.
///
/// Example: `let drawn = clip_beam_to_map(&shot.segments);`
pub fn clip_beam_to_map(segments: &[BeamSegment]) -> Vec<BeamSegment> {
    let max_w: f64 = MAX_DRAWN_BEAM_DISTANCE_PER_CURVATURE_RADIUS.cosh();
    let mut drawn: Vec<BeamSegment> = Vec::new();
    for segment in segments {
        let get_w = |s: f64| geodesic_point(segment.a, segment.b, s).w;
        if get_w(0.0) > max_w {
            break;
        }
        if get_w(segment.length) <= max_w {
            drawn.push(*segment);
            continue;
        }
        let (mut lo, mut hi) = (0.0, segment.length);
        for _ in 0..BEAM_CLIP_BISECTION_COUNT {
            let mid: f64 = (lo + hi) / 2.0;
            if get_w(mid) > max_w {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        drawn.push(BeamSegment {
            length: lo,
            ..*segment
        });
        break;
    }
    drawn
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
pub(crate) fn geodesic_point(x: DVec4, v: DVec4, s: f64) -> DVec4 {
    s.cosh() * x + s.sinh() * v
}

/// `geodesic_tangent` gives sinh(s) x + cosh(s) v.
///
/// Example: `let w = geodesic_tangent(x, v, 0.1);`
pub(crate) fn geodesic_tangent(x: DVec4, v: DVec4, s: f64) -> DVec4 {
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
pub(crate) fn make_ground_frame(g: DVec4, toward: DVec4) -> DMat4 {
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

/// `make_structure` gives a structure of `shapes` (starting at `first_shape` in the world's list)
/// in `frame`, with a bounding sphere around the corners of their boxes plus `BOUND_MARGIN_M`.
///
/// Example: `let st = make_structure("crate", frame, 0, &shapes, 40.0);`
fn make_structure(
    name: &str,
    frame: DMat4,
    first_shape: usize,
    shapes: &[Shape],
    r: f64,
) -> VoxelStructure {
    let lo: DVec3 = shapes
        .iter()
        .fold(DVec3::MAX, |lo, sh| lo.min(sh.centre - sh.half));
    let hi: DVec3 = shapes
        .iter()
        .fold(DVec3::MIN, |hi, sh| hi.max(sh.centre + sh.half));
    let centre_local: DVec4 = map_klein((lo + hi) / (2.0 * r));
    let radius: f64 = (0..8)
        .map(|c: usize| {
            let q: DVec3 =
                DVec3::from_array([0, 1, 2].map(|a| if c >> a & 1 == 1 { hi[a] } else { lo[a] }));
            get_distance(map_klein(q / r), centre_local)
        })
        .fold(0.0, f64::max);
    VoxelStructure {
        name: name.to_string(),
        inverse: invert_lorentz(frame),
        first_shape,
        shape_count: shapes.len(),
        centre: frame * centre_local,
        radius: radius + BOUND_MARGIN_M / r,
    }
}

impl VoxelWorld {
    /// `get_shapes` gives structure k's shapes.
    ///
    /// Example: `let shapes = world.get_shapes(0);`
    pub fn get_shapes(&self, k: usize) -> &[Shape] {
        let st: &VoxelStructure = &self.structures[k];
        &self.shapes[st.first_shape..st.first_shape + st.shape_count]
    }

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
        let (mut structures, mut shapes) = (Vec::new(), Vec::new());
        for s in &map.structures {
            let first_shape: usize = shapes.len();
            for shape in &s.shapes {
                shapes.push(Shape::FromMap(shape).map_err(|e| format!("map {}: {e}", map.name))?);
            }
            let frame: DMat4 = to_frame(&s.frame);
            structures.push(make_structure(
                &s.name,
                frame,
                first_shape,
                &shapes[first_shape..],
                r,
            ));
        }
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
        let mut surfaces: Vec<[f32; 4]> = vec![get_material_surface(""); MAX_MATERIAL_COUNT];
        for material in &map.materials {
            surfaces[usize::from(material.id)] = get_material_surface(&material.name);
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
            terrain: map.ground.terrain.as_ref().map(Terrain::FromMap),
            static_structure_count: structures.len(),
            consoles: find_consoles(map),
            flashlight_eye: None,
            sky_time_s: 0.0,
            structures,
            shapes,
            warps,
            lights: map
                .lights
                .iter()
                .map(|l| (to_point(&l.position), l.color, l.radius_m))
                .collect(),
            palette,
            surfaces,
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
            settings: ServerSettings::Builtin(),
            head_bob: PerPlayerSettings::Builtin().view.head_bob,
            spawns: map.spawns.iter().map(|s| to_frame(&s.frame)).collect(),
            clearance: Arc::new(ClearanceGrids::default()),
        };
        world.set_settings(ServerSettings::Builtin());
        world.clearance = Arc::new(ClearanceGrids::Build(&world));
        Ok(world)
    }

    /// `set_settings` makes `settings` the world's game settings and moves the player by them:
    /// walking and sprinting at multiples of the map's speeds, and a jump that rises
    /// `jump_height_m` in `jump_time_per_map_jump_time` of the map's jump time (gravity follows).
    ///
    /// Example: `world.set_settings(ServerSettings::Load(Path::new("fast.yaml"))?);`
    pub fn set_settings(&mut self, settings: ServerSettings) {
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

    /// `get_map_gravity_m_per_s2` gives the map's own gravity, before the game settings speed up
    /// the players' jumps (`set_settings`).
    ///
    /// Example: `let g_m_per_s2 = world.get_map_gravity_m_per_s2();`
    pub fn get_map_gravity_m_per_s2(&self) -> f64 {
        self.map_gravity_m_per_s2
    }

    /// `is_sunlit` says whether sunlight reaches point x: nothing (the ground or a block) lies
    /// within `SUN_SHADOW_M` of it toward the sun, as `render_voxel.metal`'s `trace_shadow` checks.
    ///
    /// Example: `let is_lit = world.is_sunlit(x);`
    pub fn is_sunlit(&self, x: DVec4) -> bool {
        let toward_sun: DVec4 = transport_from_centre(self.sun, x);
        self.trace_scene(x, toward_sun, SUN_SHADOW_M / self.r, None)
            .is_none()
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
            let first_shape: usize = shared.shapes.len();
            shared.shapes.extend(AVATAR_SHAPES.map(|shape| Shape {
                material: material as u8,
                ..shape
            }));
            let body: &[Shape] = &shared.shapes[first_shape..];
            let avatar: VoxelStructure = make_structure("player", feet, first_shape, body, self.r);
            shared.structures.push(avatar);
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

    /// `light_consoles` sets each console's light at `time_s` (seconds, never going back): dark
    /// (and unlit) once its bit in `off_mask` is set, otherwise pulsing between
    /// `CONSOLE_PULSE_MIN_FRAC` and `CONSOLE_PULSE_MAX_FRAC` of its colour. Console i pulses at
    /// `get_console_pulse_hz(disabling_fracs[i])`, faster and faster as it is turned off, and
    /// eases back down when that stops (`ConsolePulse::advance`).
    ///
    /// Example: `world.light_consoles(game.off_mask, &[0.0; MAX_CONSOLE_COUNT], 12.5);`
    pub fn light_consoles(
        &mut self,
        off_mask: u32,
        disabling_fracs: &[f64; MAX_CONSOLE_COUNT],
        time_s: f64,
    ) {
        for (i, console) in self.consoles.iter_mut().enumerate() {
            console.pulse = console
                .pulse
                .advance(get_console_pulse_hz(disabling_fracs[i]), time_s);
            let entry: [f32; 4] = if off_mask >> i & 1 == 1 {
                let [r, g, b] = console.color.map(|c| c * CONSOLE_OFF_FRAC);
                [r, g, b, 0.0]
            } else {
                let level: f32 = console.pulse.get_level_frac();
                let [r, g, b] = console.color.map(|c| c * level);
                [r, g, b, 1.0]
            };
            self.palette[usize::from(console.material)] = entry;
        }
    }

    /// `find_aimed_material` gives the material of the block straight ahead of `camera`'s eye
    /// within `reach_m`, or `None` when the ground or nothing is there. The ray passes through glass
    /// and isn't bent: over a few meters only a mouth's throat would bend it much.
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
        let p: DVec4 = self.structures[k].inverse * x;
        p.y / p.w * self.r >= AVATAR_HEAD_FROM_M
    }

    /// `get_material_at` gives the material of the cell holding point x among `candidates`, or 0.
    ///
    /// Example: `let m = world.get_material_at(x, &nearby);`
    fn get_material_at(&self, x: DVec4, candidates: &[usize]) -> u8 {
        for &k in candidates {
            let p: DVec4 = self.structures[k].inverse * x;
            let q: DVec3 = p.truncate() / p.w * self.r;
            if let Some(shape) = self
                .get_shapes(k)
                .iter()
                .find(|sh| sh.get_distance_m(q) <= 0.0)
            {
                return shape.material;
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
        get_fermi_coordinates(x, self.r)
    }

    /// `is_under_ground` says whether point x is below the ground (the terrain, or h = 0 without).
    ///
    /// Example: `if world.is_under_ground(x) { ... }`
    fn is_under_ground(&self, x: DVec4) -> bool {
        let [u_m, v_m, h_m] = self.get_fermi_coordinates(x);
        h_m < self
            .terrain
            .as_ref()
            .map_or(0.0, |t| t.get_height_m(u_m, v_m))
    }

    /// `get_edge_frac` gives how far a point is into the map's soft edge (`map_edge` in the
    /// settings): 0 before players start to slow on the crater's wall, 1 where their speed would
    /// reach 0; always 0 on a map without terrain.
    ///
    /// Example: `let edge_frac = world.get_edge_frac(eye);`
    pub fn get_edge_frac(&self, x: DVec4) -> f64 {
        let Some(terrain) = &self.terrain else {
            return 0.0;
        };
        let [u_m, v_m, _] = self.get_fermi_coordinates(x);
        let edge = &self.settings.map_edge;
        ((terrain.get_wall_frac(u_m, v_m) - edge.start_wall_frac)
            / (edge.stop_wall_frac - edge.start_wall_frac))
            .clamp(0.0, 1.0)
    }

    /// `is_outside_boundary` says whether a point is past the map's edge: on a map with terrain,
    /// where the soft edge stops players (`get_edge_frac` reaches 1); otherwise outside the
    /// playable ellipse (never, without one).
    ///
    /// Example: `if world.is_outside_boundary(x) { ... }`
    fn is_outside_boundary(&self, x: DVec4) -> bool {
        if self.terrain.is_some() {
            return self.get_edge_frac(x) >= 1.0;
        }
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
            let probe: DVec4 =
                body * make_boost(AXIS_Y, y_m / self.r) * map_exp(DVec3::new(dx, 0.0, dz), self.r);
            self.is_under_ground(probe) || self.get_material_at(probe, nearby) != 0
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
    /// standing on `body` with eye height `h_m`, or `None` if none does. Walking on a structure
    /// needs it: its floors are geodesic planes, which rise a little against the ground's height
    /// away from the structure's centre, but a lift this small never climbs a block. It tries lifts
    /// upward in `WALK_RISE_STEP_M` steps and bisects below the first that clears, rather than
    /// trying the largest first: under a low ceiling (a doorway's lintel, a walkway's roof) the
    /// largest lift puts the head into it while a small one clears the floor.
    ///
    /// Example: `if let Some(rise_m) = world.find_walk_rise_m(body, h_m, &nearby) { ... }`
    fn find_walk_rise_m(&self, body: DMat4, h_m: f64, nearby: &[usize]) -> Option<f64> {
        let step_count: usize = (MAX_WALK_RISE_M / WALK_RISE_STEP_M).ceil() as usize;
        let first_clear_m: f64 = (1..=step_count)
            .map(|i| (i as f64 * WALK_RISE_STEP_M).min(MAX_WALK_RISE_M))
            .find(|&rise_m| !self.is_body_blocked(body, h_m + rise_m, nearby))?;
        let (mut lo_m, mut hi_m) = ((first_clear_m - WALK_RISE_STEP_M).max(0.0), first_clear_m);
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

    /// `trace_structure` finds where a chord first enters a shape of structure k, by sphere tracing.
    /// The chord passes through glass, as light and lasers do.
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
        let q0: DVec3 = p.truncate() / p.w * self.r;
        let dq: DVec3 = (t.truncate() * p.w - p.truncate() * t.w) * self.r;
        let glass: Option<u8> = u8::try_from(self.special.glass).ok();
        let (mu, _) = find_hit_mu(self.get_shapes(k), q0, dq, mu_of(s_lo), mu_of(s_hi), glass)?;
        let lam: f64 = mu * p.w * p.w / (1.0 - mu * p.w * t.w);
        Some(s_star + lam.atanh())
    }

    /// `trace_scene` finds the first thing a chord hits: the ground or a block (among all
    /// structures, or warp w's list).
    ///
    /// Example: `let hit = world.trace_scene(x, v, 1.0, None);`
    fn trace_scene(&self, x: DVec4, v: DVec4, s_max: f64, warp: Option<usize>) -> Option<Hit> {
        let ground_s: Option<f64> = match &self.terrain {
            Some(terrain) => terrain.find_hit_s(x, v, s_max, self.r),
            None if x.y >= 0.0 && v.y < 0.0 && x.y / -v.y < 1.0 => Some((x.y / -v.y).atanh()),
            None => None,
        };
        let mut best: Option<Hit> = ground_s
            .filter(|&s| s < s_max)
            .map(|s| Hit { structure: None, s });
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
    /// bent inside them, through throats and glass, to a block, the ground, a horizon or
    /// `LASER_MAX_LENGTH_M`.
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

/// `StepReport` is what one `VoxelPlayer::step` did that the window shows: messages for the HUD,
/// and whether the player went through a wormhole mouth.
pub struct StepReport {
    pub messages: Vec<String>,
    pub has_crossed_mouth: bool,
}

/// `VoxelPlayer` is a player on a voxel map, as in the level: a body frame on the ground plane, the
/// eye's height above it, the view's pitch, the vertical speed, whether they fly or stand, their
/// sprint energy (1 full, 0 empty) and whether they sprinted on the last step, and the head bob's
/// phase and how much of it shows (0 standing still, 1 walking), and the seconds left of being
/// slowed after a hit (moving at the settings' `when_hit.slow_speed_frac`), and how far they are
/// crouched (0 standing, 1 crouched).
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
    pub slowed_s: f64,
    pub crouch_frac: f64,
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
            slowed_s: 0.0,
            crouch_frac: 0.0,
        }
    }

    /// `NearSpawn` makes player `id` standing near the spawn point of `team` (the map's spawn of
    /// that index), clear of blocks, at the first slot at least `MIN_SPAWN_GAP_M` from every eye in
    /// `others`: slots go around rings `SPAWN_RING_STEP_M` apart from `SPAWN_RING_M`,
    /// `SPAWN_TURN_RAD` apart, starting from the id's own. If all are taken it uses the last.
    ///
    /// Example: `let player = VoxelPlayer::NearSpawn(&world, 1, 3, &other_eyes);`
    #[allow(non_snake_case)]
    pub fn NearSpawn(world: &VoxelWorld, team: u32, id: u32, others: &[DVec4]) -> VoxelPlayer {
        let spawn: DMat4 = VoxelPlayer::AtSpawn(world, team as usize).body;
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
            slowed_s: 0.0,
            crouch_frac: 0.0,
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
            slowed_s: 0.0,
            crouch_frac: 0.0,
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
            slowed_s: 0.0,
            crouch_frac: 0.0,
        }
    }

    /// `get_camera_frame` gives the eye's frame: the body, raised to the eye (with the head bob)
    /// and pitched.
    ///
    /// Example: `let camera = player.get_camera_frame(&world);`
    pub fn get_camera_frame(&self, world: &VoxelWorld) -> DMat4 {
        self.body
            * make_boost(
                AXIS_Y,
                (self.h_m + self.get_bob_m(world) - self.get_crouch_drop_m(world)) / world.r,
            )
            * make_rotation(AXIS_Z, AXIS_Y, -self.pitch_rad)
    }

    /// `get_crouch_drop_m` gives how far crouching lowers the eye now: up to the settings'
    /// `crouch_eye_height_frac` of the standing eye height.
    ///
    /// Example: `let drop_m = player.get_crouch_drop_m(&world);`
    pub fn get_crouch_drop_m(&self, world: &VoxelWorld) -> f64 {
        let crouched_frac: f64 = world.settings.movement.crouch_eye_height_frac;
        self.crouch_frac * world.player.eye_height_m * (1.0 - crouched_frac)
    }

    /// `get_bob_m` gives the head bob: the eye dips at each footstep, deeper when sprinting, by
    /// `world`'s settings.
    ///
    /// Example: `let bob_m = player.get_bob_m(&world);`
    pub fn get_bob_m(&self, world: &VoxelWorld) -> f64 {
        let bob = &world.head_bob;
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
    /// moving along light paths inside a mouth's reach, falling into a black hole (back to the spawn
    /// of index `spawn_index`, the player's team),
    /// slowing down without end on the way out up the crater's wall (`get_edge_frac`: each step
    /// that goes further out is cut by the share of the way already come), colliding with blocks
    /// and the boundary (rising up to `MAX_WALK_RISE_M` where a floor's
    /// cells rise a little), jumping (the only way onto a block) and falling. A
    /// body that starts the step inside a block is first pushed out sideways, and lifted (as the
    /// level does) only if that fails. It returns messages for the HUD and whether the player went
    /// through a mouth.
    ///
    /// Example: `for message in player.step(&world, &input, 1.0 / 60.0, 0).messages { ... }`
    pub fn step(
        &mut self,
        world: &VoxelWorld,
        input: &Input,
        dt_s: f64,
        spawn_index: usize,
    ) -> StepReport {
        let r: f64 = world.r;
        let forward: f64 = f64::from(u8::from(input.forward)) - f64::from(u8::from(input.back));
        let side: f64 = f64::from(u8::from(input.right)) - f64::from(u8::from(input.left));
        let len: f64 = forward.hypot(side);
        let settings: PlayerSettings = world.player;
        let (energy, bob) = (&world.settings.sprint_energy, &world.head_bob);
        let crouch_step: f64 = dt_s / world.settings.movement.crouch_s;
        let crouch_target: f64 = f64::from(u8::from(input.crouch));
        self.crouch_frac += (crouch_target - self.crouch_frac).clamp(-crouch_step, crouch_step);
        let wants_to_sprint: bool = input.sprint && !input.crouch && len > 0.0;
        self.is_sprinting = wants_to_sprint && self.energy_frac > 0.0;
        let energy_change_per_s: f64 = match (wants_to_sprint, input.crouch) {
            (true, _) => -1.0 / energy.drain_s,
            (false, true) => 1.0 / energy.refill_crouching_s,
            (false, false) => 1.0 / energy.refill_walking_s,
        };
        self.energy_frac = (self.energy_frac + energy_change_per_s * dt_s).clamp(0.0, 1.0);
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
        let slow_frac: f64 = if self.slowed_s > 0.0 {
            world.settings.when_hit.slow_speed_frac
        } else {
            1.0
        } * if input.crouch && !self.is_flying {
            world.settings.movement.crouch_speed_frac
        } else {
            1.0
        };
        self.slowed_s = (self.slowed_s - dt_s).max(0.0);
        let speed_m_per_s: f64 = slow_frac
            * match (self.is_flying, self.is_sprinting) {
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
            *self = VoxelPlayer::NearSpawn(world, spawn_index as u32, 0, &[]);
            return StepReport {
                messages: vec!["You fell into the black hole".to_string()],
                has_crossed_mouth: false,
            };
        }
        let (mut body_new, mut h_new, mut crossed) = (self.body, self.h_m, None);
        let has_moved: bool = len > 0.0;
        if has_moved {
            let dir: [f64; 2] = [side / len, -forward / len];
            let distance_m: f64 = speed_m_per_s * dt_s;
            if let Some(w) = warp.filter(|&w| world.warps[w].is_mouth) {
                (body_new, h_new, crossed) = self.move_optically(world, dir, distance_m, w);
            } else {
                let move_m = |step_m: f64| -> DMat4 {
                    renormalize_frame(
                        self.body
                            * make_boost(AXIS_Z, dir[1] * step_m / r)
                            * make_boost(AXIS_X, dir[0] * step_m / r),
                    )
                };
                let edge_frac: f64 = world.get_edge_frac(self.body.col(AXIS_W));
                body_new = move_m(distance_m);
                if world.get_edge_frac(body_new.col(AXIS_W)) > edge_frac {
                    body_new = move_m(distance_m * (1.0 - edge_frac));
                }
            }
        }
        let report: StepReport = StepReport {
            messages: crossed
                .iter()
                .map(|name| format!("Through {name}"))
                .collect(),
            has_crossed_mouth: crossed.is_some(),
        };
        if self.is_flying {
            (self.body, self.h_m) = (body_new, h_new);
            let climb_m_per_s: f64 = MIN_FLY_CLIMB_M_PER_S.max(self.h_m) * FLY_CLIMB_PER_S;
            if input.up {
                self.h_m += climb_m_per_s * dt_s;
            }
            if input.down {
                self.h_m -= climb_m_per_s * dt_s;
            }
            let [u_m, v_m, _] = world.get_fermi_coordinates(self.body.col(AXIS_W));
            let floor_m: f64 = world
                .terrain
                .as_ref()
                .map_or(0.0, |t| t.get_height_m(u_m, v_m));
            self.h_m = self.h_m.clamp(floor_m + MIN_FLY_HEIGHT_M, MAX_FLY_HEIGHT_M);
            return report;
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
        report
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
    //! | Laser at a window (the red habitat module's) | shot from inside touching the pane, close (the muzzle past the pane), across the room, slantwise (each leaves); from outside (comes in to the floor) | `a_laser_passes_through_a_window` |
    //! | What a window's pane stops | lasers (no), players (yes: its middle is solid) | `a_laser_passes_through_a_window` |
    //! | Players spawning | up to the player limit, and a late id whose own slot is next to player 1's (it moves on) | `players_spawn_apart` |
    //! | Beam drawn | inside the map (whole), out into the sky (stops 4 curvature radii from the origin) | `beams_into_the_sky_stop_near_the_map` |
    //! | Beam clipped to | its middle (through the wormhole), before its start, past its end | `a_clipped_beam_keeps_its_middle` |
    //! | Obstacle walked into | one block (stops the player), one block while jumping (climbed), a wall turned along after touching it at an angle (not climbed) | `walking_into_a_block_does_not_climb_it`, `jumping_climbs_onto_a_block`, `turning_along_a_wall_does_not_climb_it` |
    //! | Jump on open ground | rises the settings' jump height and lands in their fraction of the map's jump time | `a_jump_is_low_and_quick` |
    //! | Walking | upright, crouched (half as far, eye fully lowered) | `crouching_lowers_the_eye_and_slows_walking` |
    //! | Step through a mouth | not yet (no crossing, no message), the step that goes through (one crossing and its message) | `walking_into_a_mouth_reports_the_crossing` |
    //! | Sprint energy | full (sprints at sprint speed, drains), empty with `Shift` held (walks, stays empty), `Shift` released (refills) | `sprinting_uses_energy` |
    //! | Refilling energy | crouched (at the crouched rate), walking upright (at the walking rate) | `crouching_refills_energy_faster` |
    //! | Map edge | walking out up the crater's wall (ever slower, never past the stop), walking back in (full speed) | `the_crater_wall_slows_without_end` |
    //! | Low way walked out of | a habitat module's doors (each module, each way), a walkway's ends (each walkway, each way) | `players_walk_out_of_modules_and_walkways` |
    //! | Floor walked on | the ground, a structure's deck above the ground (its cells' tops rise a little against the ground's height) | `walking_into_a_block_does_not_climb_it` (walks on the ground first), `walking_on_a_deck_moves` |
    //! | Progress of turning off a console | none (the normal rate), part way (rising, faster each step), done (the frenzy), past done (fails) | `the_pulse_ramps_up_with_progress`, `progress_past_done_fails` |
    //! | Target rate against the pulse's own | higher (reached at once), the same (kept), lower (eases down: barely after a frame, nearly there after a second, never below) | `the_pulse_eases_back_down` |
    //! | Rate while the light is watched | steady, ramping up over a whole hold, dropping when the hold stops (the light moves only as fast as the pulse, never jumps) | `the_light_never_jumps` |
    //! | Console | off (dark, unlit), on and not held (the normal pulse), on and held (its own faster pulse) | `consoles_pulse_by_their_own_progress` |

    use super::*;
    use crate::geometry::orthonormalize_frame;
    use crate::map::{Map, get_map_folder};

    const LANDER_DECK_M: f64 = 1.0;
    const STAND_TOLERANCE_M: f64 = 0.06;
    const MAX_PLINTH_M: f64 = 1.5;
    const PYLON_DISTANCE_M: f64 = 34.0;
    const DISTANCE_TOLERANCE_M: f64 = 0.5;
    const BLOCK_U_M: f64 = -80.0;
    const BLOCK_DEPTH_M: f64 = 6.0;
    const BLOCK_HALF_WIDTH_M: f64 = 6.0;
    const WALL_M: f64 = 3.0;
    const START_OUT_M: f64 = 0.6;
    const DIAGONAL_M: f64 = 1.5;
    const STEP_S: f64 = 1.0 / 60.0;
    const WALK_STEP_COUNT: usize = 90;
    const MAX_RISE_ALONG_WALL_M: f64 = 0.5;
    const MIN_RISE_ONTO_BLOCK_M: f64 = 0.9;
    const MAX_RISE_ONTO_BLOCK_M: f64 = 1.2;
    const MIN_CLEARANCE_OVER_GROUND_M: f64 = 0.2;
    const DECK_WALK_STEP_COUNT: usize = 30;
    const MIN_DECK_WALK_FRAC: f64 = 0.5;
    const QUARTER_TURN_RAD: f64 = std::f64::consts::FRAC_PI_4;
    const EMPTY_SPRINT_STEP_COUNT: usize = 10;
    const FAR_ID_NEAR_ONE: u32 = 35;
    const SPAWN_TEAM_COUNT: u32 = 2;
    const SKY_PITCH_RAD: f64 = 0.5;
    const CROUCH_STEP_COUNT: usize = 30;
    const CROUCH_TOLERANCE_FRAC: f64 = 0.02;
    const MAX_JUMP_STEP_COUNT: usize = 120;
    const JUMP_HEIGHT_TOLERANCE_M: f64 = 0.1;
    const JUMP_TIME_TOLERANCE_S: f64 = 0.05;
    const HZ_TOLERANCE: f64 = 1e-9;
    const RAMP_STEP_COUNT: usize = 10;
    const HALF_FRAC: f64 = 0.5;
    const PAST_DONE_FRAC: f64 = 1.5;
    const EASE_STEP_COUNT: usize = 60;
    const MIN_KEPT_AFTER_A_FRAME_FRAC: f64 = 0.9;
    const MAX_LEFT_AFTER_A_SECOND_FRAC: f64 = 0.05;
    const HOLD_S: f64 = 10.0;
    const FINE_STEP_S: f64 = 0.001;
    const STEADY_S: f64 = 1.0;
    const LEVEL_TOLERANCE_FRAC: f64 = 1e-6;
    const HELD_CONSOLE: usize = 1;
    const OFF_CONSOLE: usize = 2;

    /// `make_block_world` gives the Lunar Surface world with flat ground and nothing on it but a
    /// box `height_m` tall, 1 m deep and 6 m long at u = -80 m, and a spot on the ground
    /// `START_OUT_M` from its +x face facing toward it and `along_m` along it (map points are
    /// (u, v) in meters, as `VoxelPlayer::AtView` takes them).
    ///
    /// Example: `let (world, at_m, target_m) = make_block_world(1.0, 0.0);`
    fn make_block_world(height_m: f64, along_m: f64) -> (VoxelWorld, [f64; 2], [f64; 2]) {
        let mut world: VoxelWorld = load_lunar_surface();
        let frame: DMat4 = make_boost(AXIS_Z, -BLOCK_U_M / world.r);
        world.terrain = None;
        world.shapes = vec![Shape {
            kind: ShapeKind::Box,
            centre: DVec3::new(0.5 - BLOCK_DEPTH_M / 2.0, height_m / 2.0, 0.0),
            half: DVec3::new(BLOCK_DEPTH_M / 2.0, height_m / 2.0, BLOCK_HALF_WIDTH_M),
            round_m: 0.0,
            axis: 1,
            material: 1,
        }];
        world.structures = vec![make_structure("block", frame, 0, &world.shapes, world.r)];
        world.static_structure_count = 1;
        for warp in &mut world.warps {
            warp.list.clear();
        }
        let to_map = |q: DVec3| -> [f64; 2] {
            let p: DVec4 = frame * map_klein(q / world.r);
            let v_m: f64 = world.r * p.x.asinh();
            [-world.r * (p.z / (v_m / world.r).cosh()).asinh(), v_m]
        };
        let at_m: [f64; 2] = to_map(DVec3::new(0.5 + START_OUT_M, 0.0, 0.0));
        let target_m: [f64; 2] = to_map(DVec3::new(0.5 + START_OUT_M - DIAGONAL_M, 0.0, along_m));
        (world, at_m, target_m)
    }

    /// `get_eye_above_ground_m` gives a player's eye height above the ground under them.
    ///
    /// Example: `let eye_m = get_eye_above_ground_m(&world, &player);`
    fn get_eye_above_ground_m(world: &VoxelWorld, player: &VoxelPlayer) -> f64 {
        let [u_m, v_m, _] = world.get_fermi_coordinates(player.body.col(AXIS_W));
        player.h_m
            - world
                .terrain
                .as_ref()
                .map_or(0.0, |t| t.get_height_m(u_m, v_m))
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

    #[test]
    fn the_crater_wall_slows_without_end() {
        const START_V_M: f64 = 50.0;
        const OUT_STEP_COUNT: usize = 3600;
        const BACK_STEP_COUNT: usize = 60;
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer =
            VoxelPlayer::AtView(&world, [0.0, START_V_M], [0.0, 2.0 * START_V_M], None, 0.0);
        let get_v_m =
            |player: &VoxelPlayer| world.get_fermi_coordinates(player.body.col(AXIS_W))[1];
        let mut steps_m: Vec<f64> = Vec::new();
        for _ in 0..OUT_STEP_COUNT {
            let before_m: f64 = get_v_m(&player);
            walk_forward(&world, &mut player, 1);
            steps_m.push(get_v_m(&player) - before_m);
        }
        let edge_frac: f64 = world.get_edge_frac(player.body.col(AXIS_W));
        assert!(edge_frac > 0.9 && edge_frac < 1.0, "{edge_frac}");
        assert!(
            steps_m[OUT_STEP_COUNT - 1] < 0.1 * steps_m[0],
            "{steps_m:?}"
        );
        player.look(std::f64::consts::PI / LOOK_RAD_PER_PX, 0.0);
        let before_m: f64 = get_v_m(&player);
        walk_forward(&world, &mut player, BACK_STEP_COUNT);
        let walked_m: f64 = before_m - get_v_m(&player);
        let expected_m: f64 = world.player.walk_m_per_s * STEP_S * BACK_STEP_COUNT as f64;
        assert!(
            (walked_m - expected_m).abs() < DISTANCE_TOLERANCE_M,
            "{walked_m} of {expected_m}"
        );
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
        let deck_m: f64 = get_eye_above_ground_m(&world, &on_lander) - eye_m;
        assert!(
            deck_m > LANDER_DECK_M - STAND_TOLERANCE_M && deck_m < LANDER_DECK_M + MAX_PLINTH_M,
            "the spawn stands {deck_m} m above the ground"
        );
        let on_ground: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        assert!((get_eye_above_ground_m(&world, &on_ground) - eye_m).abs() < STAND_TOLERANCE_M);
    }

    #[test]
    fn the_laser_into_the_wormhole_hits_the_red_pylon() {
        let world: VoxelWorld = load_lunar_surface();
        let player: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-22.0, 40.5], [-14.0, 40.0], None, 0.0);
        let shot: Shot = world.trace_laser(player.get_camera_frame(&world));
        assert_eq!(shot.through, vec!["wormhole mouth A".to_string()]);
        let hit: usize = shot.hit_structure.expect("the laser hits a structure");
        assert_eq!(world.structures[hit].name, "red pylon");
        assert!((shot.length_m - PYLON_DISTANCE_M).abs() < DISTANCE_TOLERANCE_M);
    }

    /// `aim_camera` gives a camera at point `from_m` of structure k's local chart, looking along the
    /// chart's straight line (a geodesic) toward point `to_m`, upright along the chart's y.
    ///
    /// Example: `let camera = aim_camera(&world, k, DVec3::new(0.0, 2.5, 0.0), DVec3::new(-6.0, 2.5, 0.0));`
    fn aim_camera(world: &VoxelWorld, k: usize, from_m: DVec3, to_m: DVec3) -> DMat4 {
        let frame: DMat4 = invert_lorentz(world.structures[k].inverse);
        let to_world = |q_m: DVec3| -> DVec4 { frame * map_klein(q_m / world.r) };
        let basis: DMat4 = orthonormalize_frame(DMat4::from_cols(
            to_world(2.0 * from_m - to_m),
            to_world(from_m + DVec3::Y),
            to_world(from_m + (to_m - from_m).cross(DVec3::Y)),
            to_world(from_m),
        ));
        DMat4::from_cols(basis.col(2), basis.col(1), basis.col(0), basis.col(3))
    }

    /// `a_laser_passes_through_a_window` checks that a shot at a window of the red habitat module
    /// passes through its glass pane however close to it the shooter stands: from inside it leaves
    /// the module, and from outside it comes in to the floor. The pane still stops players (W-70).
    #[test]
    fn a_laser_passes_through_a_window() {
        const TOUCHING_M: f64 = 0.05;
        const CLOSE_M: f64 = 0.3;
        const ACROSS_ROOM_M: f64 = 3.5;
        const OUTSIDE_M: f64 = 2.5;
        const AIM_PAST_M: f64 = 4.0;
        const SLANT_FROM_M: f64 = 0.4;
        const SLANT_TO_M: f64 = -1.5;
        const FLOOR_IN_M: f64 = 3.0;
        const FLOOR_DOWN_M: f64 = 1.5;
        const MIN_INSIDE_M: f64 = 1.0;
        let world: VoxelWorld = load_lunar_surface();
        let k: usize = world
            .structures
            .iter()
            .position(|st| st.name == "red habitat module")
            .expect("the red habitat module");
        let pane: Shape = *world
            .get_shapes(k)
            .iter()
            .filter(|sh| i32::from(sh.material) == world.special.glass && sh.centre.x < 0.0)
            .min_by(|a, b| a.centre.z.abs().total_cmp(&b.centre.z.abs()))
            .expect("a window in the module's -x wall");
        let (inner_x_m, outer_x_m) = (pane.centre.x + pane.half.x, pane.centre.x - pane.half.x);
        let at = |x_m: f64, along_m: f64| -> DVec3 {
            DVec3::new(x_m, pane.centre.y, pane.centre.z + along_m)
        };
        let out_m: DVec3 = at(pane.centre.x - AIM_PAST_M, 0.0);
        for (from_m, to_m) in [
            (at(inner_x_m + TOUCHING_M, 0.0), out_m),
            (at(inner_x_m + CLOSE_M, 0.0), out_m),
            (at(inner_x_m + ACROSS_ROOM_M, 0.0), out_m),
            (
                at(inner_x_m + CLOSE_M, SLANT_FROM_M),
                at(pane.centre.x - AIM_PAST_M, SLANT_TO_M),
            ),
        ] {
            let shot: Shot = world.trace_laser(aim_camera(&world, k, from_m, to_m));
            assert_ne!(shot.hit_structure, Some(k), "shot from {from_m} stays in");
            assert!(
                shot.length_m > from_m.x - outer_x_m,
                "shot from {from_m} stops after {} m, before the pane's outer face",
                shot.length_m
            );
        }
        let floor_m: DVec3 = pane.centre + DVec3::new(FLOOR_IN_M, -FLOOR_DOWN_M, 0.0);
        let outside_m: DVec3 = pane.centre - (floor_m - pane.centre).normalize() * OUTSIDE_M;
        let shot: Shot = world.trace_laser(aim_camera(&world, k, outside_m, floor_m));
        assert_eq!(shot.hit_structure, Some(k), "the shot from outside");
        let p: DVec4 = world.structures[k].inverse * shot.spot.expect("a hit's spot");
        let hit_m: DVec3 = p.truncate() / p.w * world.r;
        assert!(
            hit_m.x > inner_x_m + MIN_INSIDE_M,
            "the shot from outside stops at {hit_m}, not inside the room"
        );
        let pane_x: DVec4 =
            invert_lorentz(world.structures[k].inverse) * map_klein(pane.centre / world.r);
        assert_eq!(
            i32::from(world.get_material_at(pane_x, &[k])),
            world.special.glass,
            "the pane's middle is solid for players"
        );
    }

    #[test]
    fn players_spawn_apart() {
        let world: VoxelWorld = load_lunar_surface();
        let mut eyes: Vec<DVec4> = Vec::new();
        for id in (1..=crate::net::MAX_PLAYER_COUNT as u32).chain([FAR_ID_NEAR_ONE]) {
            let team: u32 = id % SPAWN_TEAM_COUNT;
            let player: VoxelPlayer = VoxelPlayer::NearSpawn(&world, team, id, &eyes);
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
    fn crouching_lowers_the_eye_and_slows_walking() {
        let world: VoxelWorld = load_lunar_surface();
        let start: VoxelPlayer = VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        let walk: Input = Input {
            forward: true,
            ..Input::default()
        };
        let walk_for = |input: &Input| -> VoxelPlayer {
            let mut player: VoxelPlayer = start;
            for _ in 0..CROUCH_STEP_COUNT {
                player.step(&world, input, STEP_S, 0);
            }
            player
        };
        let moved_m = |player: &VoxelPlayer| -> f64 {
            get_distance(start.body.col(AXIS_W), player.body.col(AXIS_W)) * world.r
        };
        let (upright, crouched) = (
            walk_for(&walk),
            walk_for(&Input {
                crouch: true,
                ..walk
            }),
        );
        let movement = &world.settings.movement;
        let speed_frac: f64 = moved_m(&crouched) / moved_m(&upright);
        assert!((speed_frac - movement.crouch_speed_frac).abs() < CROUCH_TOLERANCE_FRAC);
        let full_drop_m: f64 = world.player.eye_height_m * (1.0 - movement.crouch_eye_height_frac);
        assert_eq!(upright.get_crouch_drop_m(&world), 0.0);
        assert!((crouched.get_crouch_drop_m(&world) - full_drop_m).abs() < 1e-9);
    }

    #[test]
    fn walking_into_a_mouth_reports_the_crossing() {
        const MOUTH_A_VIEW_M: [f64; 2] = [-22.0, 40.0];
        const MOUTH_A_TARGET_M: [f64; 2] = [-14.0, 40.0];
        const THROUGH_STEP_COUNT: usize = 150;
        let world: VoxelWorld = load_lunar_surface();
        let mut player: VoxelPlayer =
            VoxelPlayer::AtView(&world, MOUTH_A_VIEW_M, MOUTH_A_TARGET_M, None, 0.0);
        let walk: Input = Input {
            forward: true,
            ..Input::default()
        };
        let reports: Vec<StepReport> = (0..THROUGH_STEP_COUNT)
            .map(|_| player.step(&world, &walk, STEP_S, 0))
            .collect();
        let (crossings, others): (Vec<&StepReport>, Vec<&StepReport>) =
            reports.iter().partition(|report| report.has_crossed_mouth);
        assert_eq!(crossings.len(), 1);
        assert_eq!(crossings[0].messages, vec!["Through wormhole mouth A"]);
        assert!(others.iter().all(|report| report.messages.is_empty()));
    }

    #[test]
    fn beams_into_the_sky_stop_near_the_map() {
        let world: VoxelWorld = load_lunar_surface();
        let into_wormhole: Shot = world.trace_laser(
            VoxelPlayer::AtView(&world, [-22.0, 40.0], [-14.0, 40.0], None, 0.0)
                .get_camera_frame(&world),
        );
        assert_eq!(
            clip_beam_to_map(&into_wormhole.segments).len(),
            into_wormhole.segments.len()
        );
        let into_sky: Shot = world.trace_laser(
            VoxelPlayer::AtView(&world, [-107.0, 0.0], [0.0, 0.0], None, SKY_PITCH_RAD)
                .get_camera_frame(&world),
        );
        let max_w: f64 = MAX_DRAWN_BEAM_DISTANCE_PER_CURVATURE_RADIUS.cosh();
        let drawn: Vec<BeamSegment> = clip_beam_to_map(&into_sky.segments);
        let end: &BeamSegment = drawn.last().expect("the beam starts inside the map");
        let end_w: f64 = geodesic_point(end.a, end.b, end.length).w;
        assert!(into_sky.hit_structure.is_none() && end_w <= max_w && end_w > max_w * 0.99);
    }

    #[test]
    fn walking_into_a_block_does_not_climb_it() {
        let (world, at_m, target_m) = make_block_world(1.0, 0.0);
        let mut player: VoxelPlayer = VoxelPlayer::AtView(&world, at_m, target_m, None, 0.0);
        let start_m: f64 = player.h_m;
        assert!(
            walk_forward(&world, &mut player, WALK_STEP_COUNT) - start_m < MAX_RISE_ALONG_WALL_M
        );
    }

    #[test]
    fn jumping_climbs_onto_a_block() {
        let (world, at_m, target_m) = make_block_world(1.0, 0.0);
        let mut player: VoxelPlayer = VoxelPlayer::AtView(&world, at_m, target_m, None, 0.0);
        let input: Input = Input {
            forward: true,
            up: true,
            ..Input::default()
        };
        let has_stood_on_block: bool = (0..WALK_STEP_COUNT).any(|_| {
            player.step(&world, &input, STEP_S, 0);
            let feet_m: f64 = player.h_m - world.player.eye_height_m;
            let is_on_block: bool = get_eye_above_ground_m(&world, &player)
                > world.player.eye_height_m + MIN_CLEARANCE_OVER_GROUND_M;
            player.is_on_ground
                && is_on_block
                && feet_m > MIN_RISE_ONTO_BLOCK_M
                && feet_m < MAX_RISE_ONTO_BLOCK_M
        });
        assert!(has_stood_on_block);
    }

    #[test]
    fn turning_along_a_wall_does_not_climb_it() {
        let (world, at_m, target_m) = make_block_world(WALL_M, DIAGONAL_M);
        let mut player: VoxelPlayer = VoxelPlayer::AtView(&world, at_m, target_m, None, 0.0);
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

    /// `players_walk_out_of_modules_and_walkways` checks that a player standing in the middle of
    /// a habitat module's floor walks out through either of its doors, and one standing in a
    /// walkway walks out of either end, though both are only 2 m high (W-85).
    #[test]
    fn players_walk_out_of_modules_and_walkways() {
        const MODULE_OUT_M: f64 = 6.5;
        const WALKWAY_OUT_M: f64 = 5.5;
        const EXIT_WALK_M: f64 = 9.0;
        let world: VoxelWorld = load_lunar_surface();
        let step_count: usize =
            (EXIT_WALK_M / (world.player.walk_m_per_s * STEP_S)).ceil() as usize;
        let ways: Vec<(usize, f64)> = world
            .structures
            .iter()
            .enumerate()
            .filter_map(|(k, st)| match st.name.as_str() {
                "red habitat module" | "derelict habitat module" => Some((k, MODULE_OUT_M)),
                "red walkway" | "crater walkway" => Some((k, WALKWAY_OUT_M)),
                _ => None,
            })
            .collect();
        assert_eq!(ways.len(), 5, "three habitat modules and two walkways");
        for (k, out_m) in ways {
            let frame: DMat4 = invert_lorentz(world.structures[k].inverse);
            for turn_rad in [0.0, std::f64::consts::PI] {
                let mut player: VoxelPlayer =
                    VoxelPlayer::Standing(&world, frame * make_rotation(AXIS_X, AXIS_Z, turn_rad));
                walk_forward(&world, &mut player, step_count);
                let p: DVec4 = world.structures[k].inverse * player.body.col(AXIS_W);
                let z_m: f64 = p.z / p.w * world.r;
                assert!(
                    z_m.abs() > out_m,
                    "{} #{k}, turned {turn_rad} rad: stopped at z = {z_m} m",
                    world.structures[k].name
                );
            }
        }
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
    fn crouching_refills_energy_faster() {
        const REFILL_STEP_COUNT: usize = 60;
        let world: VoxelWorld = load_lunar_surface();
        let energy = &world.settings.sprint_energy;
        let refill_s: f64 = STEP_S * REFILL_STEP_COUNT as f64;
        for (crouch, full_refill_s) in [
            (true, energy.refill_crouching_s),
            (false, energy.refill_walking_s),
        ] {
            let mut player: VoxelPlayer =
                VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
            player.energy_frac = 0.0;
            let input: Input = Input {
                forward: true,
                crouch,
                ..Input::default()
            };
            for _ in 0..REFILL_STEP_COUNT {
                player.step(&world, &input, STEP_S, 0);
            }
            let expected_frac: f64 = refill_s / full_refill_s;
            assert!(
                (player.energy_frac - expected_frac).abs() < 1e-9,
                "crouch {crouch}: {} of {expected_frac}",
                player.energy_frac
            );
        }
        assert_eq!(energy.refill_crouching_s, energy.drain_s);
        assert_eq!(energy.refill_walking_s, 2.0 * energy.drain_s);
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

    #[test]
    fn the_pulse_ramps_up_with_progress() {
        assert_eq!(get_console_pulse_hz(0.0), CONSOLE_PULSE_HZ);
        let half_hz: f64 = get_console_pulse_hz(HALF_FRAC);
        let middle_hz: f64 = (CONSOLE_PULSE_HZ * CONSOLE_FRENZY_PULSE_HZ).sqrt();
        assert!((half_hz - middle_hz).abs() < HZ_TOLERANCE, "{half_hz}");
        assert!((get_console_pulse_hz(1.0) - CONSOLE_FRENZY_PULSE_HZ).abs() < HZ_TOLERANCE);
        let rates_hz: Vec<f64> = (0..=RAMP_STEP_COUNT)
            .map(|k| get_console_pulse_hz(k as f64 / RAMP_STEP_COUNT as f64))
            .collect();
        let rises_hz: Vec<f64> = rates_hz.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            rises_hz.iter().all(|&rise_hz| rise_hz > 0.0),
            "{rates_hz:?}"
        );
        assert!(rises_hz.windows(2).all(|w| w[1] > w[0]), "{rates_hz:?}");
    }

    #[test]
    #[should_panic(expected = "disabling_frac must be between 0 and 1")]
    fn progress_past_done_fails() {
        get_console_pulse_hz(PAST_DONE_FRAC);
    }

    #[test]
    fn the_pulse_eases_back_down() {
        let frenzied: ConsolePulse =
            ConsolePulse::Resting().advance(CONSOLE_FRENZY_PULSE_HZ, STEP_S);
        assert_eq!(frenzied.hz, CONSOLE_FRENZY_PULSE_HZ);
        let kept: ConsolePulse = frenzied.advance(CONSOLE_FRENZY_PULSE_HZ, 2.0 * STEP_S);
        assert_eq!(kept.hz, CONSOLE_FRENZY_PULSE_HZ);
        let mut pulse: ConsolePulse = kept;
        let rates_hz: Vec<f64> = (1..=EASE_STEP_COUNT)
            .map(|k| {
                pulse = pulse.advance(CONSOLE_PULSE_HZ, kept.time_s + k as f64 * STEP_S);
                pulse.hz
            })
            .collect();
        let gap_hz: f64 = CONSOLE_FRENZY_PULSE_HZ - CONSOLE_PULSE_HZ;
        assert!(rates_hz[0] > CONSOLE_PULSE_HZ + MIN_KEPT_AFTER_A_FRAME_FRAC * gap_hz);
        assert!(
            rates_hz
                .windows(2)
                .all(|w| w[1] < w[0] && w[1] > CONSOLE_PULSE_HZ),
            "{rates_hz:?}"
        );
        let last_hz: f64 = rates_hz[EASE_STEP_COUNT - 1];
        assert!(last_hz < CONSOLE_PULSE_HZ + MAX_LEFT_AFTER_A_SECOND_FRAC * gap_hz);
    }

    #[test]
    fn the_light_never_jumps() {
        let ramp_step_count: usize = (HOLD_S / FINE_STEP_S) as usize;
        let steady_step_count: usize = (STEADY_S / FINE_STEP_S) as usize;
        let max_step_frac: f64 = f64::from(CONSOLE_PULSE_MAX_FRAC - CONSOLE_PULSE_MIN_FRAC)
            * std::f64::consts::PI
            * CONSOLE_FRENZY_PULSE_HZ
            * FINE_STEP_S
            + LEVEL_TOLERANCE_FRAC;
        let mut pulse: ConsolePulse = ConsolePulse::Resting();
        let mut level_frac: f32 = pulse.get_level_frac();
        let mut has_dropped: bool = false;
        for k in 1..=steady_step_count + ramp_step_count + steady_step_count {
            let held_count: usize = k.saturating_sub(steady_step_count);
            let target_hz: f64 = if held_count == 0 || held_count > ramp_step_count {
                CONSOLE_PULSE_HZ
            } else {
                get_console_pulse_hz(held_count as f64 / ramp_step_count as f64)
            };
            let previous_hz: f64 = pulse.hz;
            pulse = pulse.advance(target_hz, k as f64 * FINE_STEP_S);
            has_dropped |= target_hz < previous_hz;
            let next_frac: f32 = pulse.get_level_frac();
            let step_frac: f64 = f64::from((next_frac - level_frac).abs());
            assert!(step_frac <= max_step_frac, "step {k}: {step_frac}");
            level_frac = next_frac;
        }
        assert!(has_dropped);
    }

    #[test]
    fn consoles_pulse_by_their_own_progress() {
        let mut world: VoxelWorld = load_lunar_surface();
        assert!(world.consoles.len() > OFF_CONSOLE);
        let mut disabling_fracs: [f64; MAX_CONSOLE_COUNT] = [0.0; MAX_CONSOLE_COUNT];
        disabling_fracs[HELD_CONSOLE] = 1.0;
        world.light_consoles(1 << OFF_CONSOLE, &disabling_fracs, STEP_S);
        let rates_hz: Vec<f64> = world.consoles.iter().map(|c| c.pulse.hz).collect();
        assert_eq!(rates_hz[0], CONSOLE_PULSE_HZ);
        assert_eq!(rates_hz[HELD_CONSOLE], CONSOLE_FRENZY_PULSE_HZ);
        for (i, console) in world.consoles.iter().enumerate() {
            let entry: [f32; 4] = world.palette[usize::from(console.material)];
            let (level_frac, lit_frac) = if i == OFF_CONSOLE {
                (CONSOLE_OFF_FRAC, 0.0)
            } else {
                (console.pulse.get_level_frac(), 1.0)
            };
            assert_eq!(
                entry,
                [
                    console.color[0] * level_frac,
                    console.color[1] * level_frac,
                    console.color[2] * level_frac,
                    lit_frac
                ]
            );
        }
    }
}
