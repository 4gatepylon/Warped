//! `voxel_world.rs` is a voxel map of the portable format (`common/maps/`, such as Lunar Surface) on
//! the CPU, ported from the Lunar Surface level's JavaScript: structures with bounding spheres, warp
//! objects with the structures in their reach, a voxel walk, the laser's light path, and walking on
//! the map (collisions, stepping up blocks, gravity, jumping, flying, and moving along light paths
//! near a wormhole mouth). `render_voxel.metal` draws the same world, with `voxel_clearance.rs`'s grids.
//! On a map with team consoles (W-68), the black hole grows as consoles turn off, and time runs
//! slower near it (W-107, `get_time_rate_frac`).
//!
//! Interface: `VoxelWorld::FromMap`, `VoxelWorld::with_avatars`, `VoxelWorld::with_posed_avatars`,
//! `VoxelWorld::light_consoles`,
//! `VoxelWorld::set_thrown_wormholes`, `VoxelWorld::can_place_mouth`, `VoxelWorld::find_hit_s`,
//! `VoxelWorld::add_markers`, `VoxelWorld::find_aimed_material`, `VoxelWorld::find_clear_m`,
//! `VoxelWorld::find_crosshair_point`, `VoxelWorld::is_sunlit`, `VoxelWorld::is_eye_supported`, `VoxelWorld::get_mouth_depth_frac`,
//! `VoxelWorld::can_grow_black_hole`, `VoxelWorld::grow_black_hole`, `VoxelWorld::get_time_rate_frac`,
//! `Console`, `VoxelPlayer`, `StepReport`, `Shot`, `BeamSegment`, `MARKER_NAME`.
//!
//! Example: `let world = VoxelWorld::FromMap(&Map::Load(&get_map_folder("lunar_surface"))?)?;`

use std::sync::Arc;

use glam::{DMat4, DVec3, DVec4};
use warped_shared::map_edge;

use crate::avatar_model::{AvatarClip, AvatarModel, BODY_MODEL};
use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, invert_lorentz, make_boost, make_rotation,
    minkowski_dot, normalize_tangent, orthonormalize_frame, transport_from_centre,
};
use crate::map::{
    Fog, Gust, Map, PlayerSettings, Wind, get_console_team, to_direction, to_frame, to_point,
};
use crate::mesh::MeshModel;
use crate::net::MAX_CONSOLE_COUNT;
use crate::physics::Input;
use crate::settings::{HeadBobSettings, PerPlayerSettings, ServerSettings};
use crate::shapes::{Shape, ShapeKind, find_hit_mu};
use crate::terrain::{Terrain, get_fermi_coordinates};
use crate::thrown_wormholes::{Marker, MouthPlace, WormholeColor};
use crate::voxel_clearance::ClearanceGrids;
use crate::voxel_textures::get_material_surface;
use crate::zone_walls::ZoneWalls;
use warped_shared::movement::{MoveInput, MoveRules, MoveState, get_crouched_height_frac};

pub const MAX_MATERIAL_COUNT: usize = 256;
pub const AVATAR_FIRST_MATERIAL: usize = 240;
pub const MARKER_NAME: &str = "a thrown wormhole";
/// `FOG_DEPTHS` is how many of a fog's e-folding distances past its start a ray goes before it
/// ends in the fog (5: the fog hides all but 0.7% of what lies beyond).
pub const FOG_DEPTHS: f64 = 5.0;
const MAX_MARKER_COUNT: usize = 16;
const FIRST_GAME_MATERIAL: usize = AVATAR_FIRST_MATERIAL - MAX_MARKER_COUNT;
const BOUND_MARGIN_M: f64 = 0.2;
pub(crate) const BEND_STEP_FRAC: f64 = 0.16;
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
const MESH_WALL_M: f64 = 0.1;
const HEAD_CLEARANCE_M: f64 = 0.15;
const PROBE_LOW_M: f64 = 0.02;
const PROBE_KNEE_M: f64 = 0.3;
const PROBE_WAIST_M: f64 = 1.1;
const MIN_FLY_HEIGHT_M: f64 = 0.4;
const MAX_FLY_HEIGHT_M: f64 = 240.0;
const UNSTICK_STEP_M: f64 = 0.25;
const UNSTICK_MAX_STEPS: usize = 160;
const PUSH_OUT_STEP_M: f64 = 0.05;
const MAX_WALK_RISE_M: f64 = 0.5;
const RING_PROBE_COUNT: usize = 8;
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
const WAY_PROBE_STEP_M: f64 = 0.25;
const INSIDE_MOUTH_PER_THROAT: f64 = 5.0;
const INSIDE_MOUTH_SCALE: f64 = 1.8;
const LASER_MAX_LENGTH_M: f64 = 480.0;
const LASER_MAX_ITERATIONS: usize = 4000;
const LASER_MUZZLE_M: f64 = 3.0;
pub use warped_shared::weapons::MUZZLE_M;
const BEAM_TOLERANCE_M: f64 = 0.02;
const MAX_DRAWN_BEAM_DISTANCE_M: f64 = 160.0;
const BEAM_CLIP_BISECTION_COUNT: usize = 40;
pub const LOOK_RAD_PER_PX: f64 = 0.0025;
const MAX_PITCH_RAD: f64 = 1.55;
const TINY: f64 = 1e-12;
const AVATAR_HEAD_FROM_M: f64 = 1.5;
const AVATAR_HEIGHT_M: f64 = 2.0;
const GROUND_PROBE_M: f64 = 0.1;
/// `AvatarBody` is a player's body as `with_avatars` adds it: its feet frame, the index of its
/// colour, and its height as a share of standing (less while crouched, W-137 (crouched robots
/// sink)).
pub type AvatarBody = (DMat4, usize, f64);
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
/// the voxel bytes, its map model (`mesh`, which lasers hit) or posed body (`avatar_mesh`, an index
/// into the world's `avatar_meshes`, only drawn: lasers hit its shapes), its bounding sphere
/// (radius in curvature units), whether its glowing shapes are dark (`is_dark`: a pylon whose
/// console is off), the palette entry of its body's team colour (`body_material`, for a
/// player's or a corpse's body: `render_voxel.metal` turns the body model's teal into it), and a
/// player's body's height as a share of standing (`avatar_height_frac`: less while crouched; 1
/// for anything else).
#[derive(Clone)]
pub struct VoxelStructure {
    pub name: String,
    pub inverse: DMat4,
    pub first_shape: usize,
    pub shape_count: usize,
    pub mesh: Option<usize>,
    pub avatar_mesh: Option<usize>,
    pub centre: DVec4,
    pub radius: f64,
    pub is_dark: bool,
    pub body_material: Option<u8>,
    pub avatar_height_frac: f64,
}

/// `VoxelWarp` is a black hole or a wormhole mouth: its frame, m and reach in curvature units,
/// its partner, the structures within its reach, its colour if a player threw it, whether it
/// flashes red (just hit), and a black hole's wide lens (`get_lens_log_index`): the most it turns
/// a ray (radians, 0 for none) and its core radius (curvature units).
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
    pub color: Option<WormholeColor>,
    pub is_flashing: bool,
    pub lens_rad: f64,
    pub lens_core: f64,
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
    pylon: Option<usize>,
    lamp: Option<(usize, [f32; 3])>,
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
            let team: u32 = get_console_team(m)?;
            Some(Console {
                material: m.id,
                team,
                color: m.color,
                pulse: ConsolePulse::Resting(),
                pylon: None,
                lamp: None,
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
/// shining this frame, the time the sky shows (`sky_time_s`: the UTC clock in the window, 0 for
/// fixed views; `spaceships.rs`), and how bright this player sees the Milky Way (`milky_way`: its
/// pole and core directions, if the map has one), from their `view.milky_way_brightness_frac`. `body` is the players' body model, whose materials
/// are map model `body_mesh` (with no triangles), and `avatar_meshes` are this frame's posed bodies.
#[derive(Clone)]
pub struct VoxelWorld {
    pub name: String,
    pub r: f64,
    pub structures: Vec<VoxelStructure>,
    pub shapes: Vec<Shape>,
    pub meshes: Arc<Vec<MeshModel>>,
    pub body: Arc<AvatarModel>,
    pub body_mesh: usize,
    pub avatar_meshes: Vec<Arc<MeshModel>>,
    pub warps: Vec<VoxelWarp>,
    pub lights: Vec<(DVec4, [f32; 3], f64)>,
    pub palette: Vec<[f32; 4]>,
    pub surfaces: Vec<[f32; 4]>,
    pub special: SpecialMaterials,
    pub regions: Option<[f64; 3]>,
    pub zone_walls: Option<ZoneWalls>,
    pub terrain: Option<Terrain>,
    pub wind: Option<WindField>,
    pub fog: Option<Fog>,
    pub sand: Option<crate::map::Sand>,
    pub laser_color: Option<[f32; 3]>,
    pub sun: DVec3,
    pub planet: Option<(DVec3, f64)>,
    pub milky_way: Option<(DVec3, DVec3)>,
    pub milky_way_brightness_frac: f64,
    pub player: PlayerSettings,
    pub gravity_m_per_s2: f64,
    map_player: PlayerSettings,
    map_gravity_m_per_s2: f64,
    pub settings: ServerSettings,
    pub head_bob: HeadBobSettings,
    pub spawns: Vec<DMat4>,
    pub static_structure_count: usize,
    pub static_warp_count: usize,
    pub consoles: Vec<Console>,
    pub flashlight_eye: Option<DMat4>,
    pub sky_time_s: f64,
    pub clearance: Arc<ClearanceGrids>,
    growing_hole: Option<(usize, VoxelWarp)>,
}

/// `WindField` is a map's wind (`map::Wind`) in curvature units: the plane's unit normal at the
/// origin (w = 0), ln n's gradient per curvature radius and the chord length.
#[derive(Clone, Copy, Debug)]
pub struct WindField {
    pub normal: DVec4,
    pub k: f64,
    pub segment_s: f64,
    pub gust: Option<GustField>,
    pub time_s: f64,
    pub laser_k_per_k: f64,
}

/// `GUST_WAVE_SIZES` gives each gust wave's wavelengths along and across the wind, in the map's
/// `size_m`; `GUST_WAVE_PERIODS_S` how often each changes shape as it drifts.
const GUST_WAVE_SIZES: [[f64; 2]; GUST_WAVE_COUNT] = [[1.0, 2.2], [1.6, -1.2], [0.6, 0.9]];
const GUST_WAVE_PERIODS_S: [f64; GUST_WAVE_COUNT] = [7.0, 11.0, 5.0];
pub const GUST_WAVE_COUNT: usize = 3;

/// `GustField` is a map's `Gust` in curvature units: each wave's wavevector in the hyperboloid's
/// x, y, z (per curvature radius), its seeded phase and how fast its phase turns.
#[derive(Clone, Copy, Debug)]
pub struct GustField {
    pub strength_frac: f64,
    pub waves: [DVec3; GUST_WAVE_COUNT],
    phases_rad: [f64; GUST_WAVE_COUNT],
    rad_per_s: [f64; GUST_WAVE_COUNT],
}

impl GustField {
    /// `FromMap` builds the gust waves of `gust` for a wind blowing along `direction`, curvature
    /// radius `r` meters.
    ///
    /// Example: `let gust = GustField::FromMap(&map_gust, DVec3::X, 50.0);`
    #[allow(non_snake_case)]
    pub fn FromMap(gust: &Gust, direction: DVec3, r: f64) -> GustField {
        let across: DVec3 = direction.cross(DVec3::Y).normalize();
        let mut seed: u64 = gust.seed;
        let mut waves: [DVec3; GUST_WAVE_COUNT] = [DVec3::ZERO; GUST_WAVE_COUNT];
        let mut phases_rad: [f64; GUST_WAVE_COUNT] = [0.0; GUST_WAVE_COUNT];
        let mut rad_per_s: [f64; GUST_WAVE_COUNT] = [0.0; GUST_WAVE_COUNT];
        for i in 0..GUST_WAVE_COUNT {
            let [along, side] = GUST_WAVE_SIZES[i];
            let k_per_m: DVec3 = std::f64::consts::TAU
                * (direction / (along * gust.size_m) + across / (side * gust.size_m));
            waves[i] = k_per_m * r;
            phases_rad[i] = std::f64::consts::TAU * (next_splitmix64(&mut seed) >> 11) as f64
                / (1u64 << 53) as f64;
            rad_per_s[i] = std::f64::consts::TAU / GUST_WAVE_PERIODS_S[i]
                - k_per_m.dot(direction) * gust.speed_m_per_s;
        }
        GustField {
            strength_frac: gust.strength_frac,
            waves,
            phases_rad,
            rad_per_s,
        }
    }

    /// `get_phases_rad` is each wave's phase at `time_s` (seconds on the shared UTC clock), in
    /// [0, 2π): the shader's `u.gust_waves[i].w`.
    ///
    /// Example: `let phases = gust.get_phases_rad(world.sky_time_s);`
    pub fn get_phases_rad(&self, time_s: f64) -> [f64; GUST_WAVE_COUNT] {
        std::array::from_fn(|i| {
            (self.phases_rad[i] + self.rad_per_s[i] * time_s).rem_euclid(std::f64::consts::TAU)
        })
    }

    /// `get_strength` is how many times the wind's base strength blows at x at `time_s`: 1 plus
    /// `strength_frac` times the mean of the waves, as the shader's `gust_strength`.
    ///
    /// Example: `let strength = gust.get_strength(x, world.sky_time_s);`
    pub fn get_strength(&self, x: DVec4, time_s: f64) -> f64 {
        let phases_rad: [f64; GUST_WAVE_COUNT] = self.get_phases_rad(time_s);
        let wave_sum: f64 = (0..GUST_WAVE_COUNT)
            .map(|i| (self.waves[i].dot(x.truncate()) + phases_rad[i]).sin())
            .sum();
        1.0 + self.strength_frac * wave_sum / GUST_WAVE_COUNT as f64
    }
}

/// `next_splitmix64` steps `state` and returns the next number of the SplitMix64 sequence.
///
/// Example: `let n = next_splitmix64(&mut seed);`
fn next_splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z: u64 = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl WindField {
    /// `FromMap` converts a map's wind for curvature radius `r` meters.
    ///
    /// Example: `let wind = WindField::FromMap(&map_wind, 50.0);`
    #[allow(non_snake_case)]
    pub fn FromMap(wind: &Wind, r: f64) -> WindField {
        let [x, y, z] = wind.direction;
        WindField {
            normal: DVec4::new(x, y, z, 0.0),
            k: wind.log_index_gradient_per_m * r,
            segment_s: wind.segment_m / r,
            gust: wind
                .gust
                .map(|gust| GustField::FromMap(&gust, DVec3::new(x, y, z), r)),
            time_s: 0.0,
            laser_k_per_k: wind.laser_gradient_per_view_gradient.unwrap_or(1.0),
        }
    }

    /// `get_laser_wind` is the wind lasers feel: `laser_k_per_k` times as strong as the light's,
    /// so a beam visibly curves downwind of what the eye sees.
    ///
    /// Example: `let laser_wind = wind.get_laser_wind();`
    pub fn get_laser_wind(&self) -> WindField {
        WindField {
            k: self.k * self.laser_k_per_k,
            ..*self
        }
    }

    /// `get_gradient` is ∇ ln n at x: k, scaled by the gust strength at x at `time_s`, times the
    /// unit gradient of the signed distance to the plane, (N + ⟨x, N⟩ x) / √(1 + ⟨x, N⟩²).
    ///
    /// Example: `let g = wind.get_gradient(x);`
    pub fn get_gradient(&self, x: DVec4) -> DVec4 {
        let f: f64 = minkowski_dot(x, self.normal);
        let strength: f64 = self
            .gust
            .map_or(1.0, |gust| gust.get_strength(x, self.time_s));
        strength * self.k * (self.normal + f * x) / (1.0 + f * f).sqrt()
    }

    /// `kick` turns direction v at x by the gradient's part across it over `ds`, as the shader's
    /// `wind_kick` does.
    ///
    /// Example: `let v = wind.kick(x, v, 0.5 * ds);`
    pub fn kick(&self, x: DVec4, v: DVec4, ds: f64) -> DVec4 {
        let g: DVec4 = self.get_gradient(x);
        normalize_tangent(x, v + (g - minkowski_dot(g, v) * v) * ds)
    }
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

impl BeamSegment {
    /// `Between` gives the geodesic segment from `p` to `q`, or `None` if they are the same point.
    ///
    /// Example: `let segment = BeamSegment::Between(muzzle, end);`
    #[allow(non_snake_case)]
    pub fn Between(p: DVec4, q: DVec4) -> Option<BeamSegment> {
        let length: f64 = get_distance(p, q);
        (length > 1e-9).then(|| BeamSegment {
            a: p,
            b: (q - length.cosh() * p) / length.sinh(),
            length,
        })
    }
}

/// `Shot` is a laser's path: its drawn segments, where it hit (and what), and the mouths it went
/// through, by name and by index in the world's warps.
pub struct Shot {
    pub segments: Vec<BeamSegment>,
    pub spot: Option<DVec4>,
    pub hit_structure: Option<usize>,
    pub is_hit: bool,
    pub length_m: f64,
    pub through: Vec<String>,
    pub through_warps: Vec<usize>,
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
/// `MAX_DRAWN_BEAM_DISTANCE_M` from the map's origin, on a map of curvature radius `r`. Past
/// there, out over the sky, the shader's single-precision maths of a beam's glow turns to noise
/// (coordinates grow as cosh of that distance) that floods the sky; Lunar Surface's playable area
/// lies within 128 m, at any curvature.
///
/// Example: `let drawn = clip_beam_to_map(&shot.segments, world.r);`
pub fn clip_beam_to_map(segments: &[BeamSegment], r: f64) -> Vec<BeamSegment> {
    let max_w: f64 = (MAX_DRAWN_BEAM_DISTANCE_M / r).cosh();
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

/// `get_lens_log_index` gives a black hole's wide lens at distance `rho` from its centre (W-107):
/// d ln n / dρ and ln n, with ln n = h a / √(ρ² + a²) for the most it turns a ray h
/// (`lens_rad`) and its core radius a (`lens_core`). It bends light over its whole core, most at
/// a ray passing a from the centre, and adds no horizon, so the black part doesn't grow with it;
/// both are 0 without a lens.
///
/// Example: `let (slope, log_n) = get_lens_log_index(&world.warps[0], 0.2);`
fn get_lens_log_index(warp: &VoxelWarp, rho: f64) -> (f64, f64) {
    if warp.lens_rad == 0.0 {
        return (0.0, 0.0);
    }
    let a: f64 = warp.lens_core;
    let root: f64 = (rho * rho + a * a).sqrt();
    (
        -warp.lens_rad * a * rho / (root * root * root),
        warp.lens_rad * a / root,
    )
}

/// `map_exp` maps a local offset in meters to the hyperboloid by the exponential map at the origin.
///
/// Example: `let probe = map_exp(DVec3::new(0.25, 0.0, 0.0), 40.0);`
pub fn map_exp(q_m: DVec3, r: f64) -> DVec4 {
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

/// `make_terrain` reads a map's terrain and bakes its height everywhere a ray from inside the
/// map's boundary reaches before the fog ends it (`FOG_DEPTHS`); a map without a boundary or fog
/// keeps it unbaked.
///
/// Example: `let terrain = make_terrain(&map);`
fn make_terrain(map: &Map) -> Option<Terrain> {
    let terrain: Terrain = Terrain::FromMap(map.ground.terrain.as_ref()?);
    let boundary = map.regions.as_ref().and_then(|r| r.boundary.as_ref());
    let fog: Option<Fog> = map.atmosphere.and_then(|a| a.fog);
    Some(match (boundary, fog) {
        (Some(boundary), Some(fog)) => {
            let reach_m: f64 = fog.start_m + FOG_DEPTHS * fog.distance_m;
            terrain.bake_heights([boundary.u_m + reach_m, boundary.v_m + reach_m])
        }
        _ => terrain,
    })
}

/// `make_structure` gives a structure of `shapes` (starting at `first_shape` in the world's list)
/// drawn as `mesh` (its index in the world's meshes and the model) if it has one, in `frame`,
/// with a bounding sphere around the corners of their boxes plus `BOUND_MARGIN_M`.
///
/// Example: `let st = make_structure("crate", frame, 0, &shapes, None, 40.0);`
fn make_structure(
    name: &str,
    frame: DMat4,
    first_shape: usize,
    shapes: &[Shape],
    mesh: Option<(usize, &MeshModel)>,
    r: f64,
) -> VoxelStructure {
    let mesh_box: Option<(DVec3, DVec3)> =
        mesh.map(|(_, model)| (model.nodes[0].lo.as_dvec3(), model.nodes[0].hi.as_dvec3()));
    let lo: DVec3 = shapes
        .iter()
        .map(|sh| sh.centre - sh.half)
        .chain(mesh_box.map(|b| b.0))
        .fold(DVec3::MAX, DVec3::min);
    let hi: DVec3 = shapes
        .iter()
        .map(|sh| sh.centre + sh.half)
        .chain(mesh_box.map(|b| b.1))
        .fold(DVec3::MIN, DVec3::max);
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
        mesh: mesh.map(|(index, _)| index),
        avatar_mesh: None,
        centre: frame * centre_local,
        radius: radius + BOUND_MARGIN_M / r,
        is_dark: false,
        body_material: None,
        avatar_height_frac: 1.0,
    }
}

impl VoxelWorld {
    /// `set_sky_time_s` sets the UTC clock the sky, the blowing sand and the wind's gusts follow,
    /// so every window computes the same gusts at the same moment.
    ///
    /// Example: `world.set_sky_time_s(spaceships::get_utc_s());`
    pub fn set_sky_time_s(&mut self, time_s: f64) {
        self.sky_time_s = time_s;
        if let Some(wind) = &mut self.wind {
            wind.time_s = time_s;
        }
    }

    /// `get_shapes` gives structure k's shapes.
    ///
    /// Example: `let shapes = world.get_shapes(0);`
    pub fn get_shapes(&self, k: usize) -> &[Shape] {
        let st: &VoxelStructure = &self.structures[k];
        &self.shapes[st.first_shape..st.first_shape + st.shape_count]
    }

    /// `find_hit_mu` gives where the chart line q0 + dq mu first meets structure k within
    /// [mu_lo, mu_hi] and which shape or triangle: its mesh's triangles if it has one, else its
    /// shapes but those of material `see_through` (glass, as light and lasers pass it), if any.
    ///
    /// Example: `let hit = world.find_hit_mu(3, q0, dq, 0.0, 1.0, None);`
    pub fn find_hit_mu(
        &self,
        k: usize,
        q0: DVec3,
        dq: DVec3,
        mu_lo: f64,
        mu_hi: f64,
        see_through: Option<u8>,
    ) -> Option<(f64, usize)> {
        match self.structures[k].mesh {
            Some(m) => self.meshes[m].find_hit_mu(q0, dq, mu_lo, mu_hi),
            None => find_hit_mu(self.get_shapes(k), q0, dq, mu_lo, mu_hi, see_through),
        }
    }

    /// `get_distance_m` bounds the distance from chart point q to what light meets in structure
    /// k from below: its mesh if it has one, else its shapes.
    ///
    /// Example: `let d_m = world.get_distance_m(3, q);`
    pub fn get_distance_m(&self, k: usize, q: DVec3) -> f64 {
        match self.structures[k].mesh {
            Some(m) => self.meshes[m].get_distance_m(q),
            None => crate::shapes::get_distance_m(self.get_shapes(k), q).0,
        }
    }

    /// `FromMap` builds the world from a voxel map: each structure's bounding sphere (around its
    /// grid's corners, plus `BOUND_MARGIN_M`), each warp's list of structures within reach, the
    /// palette (with a glow for `light` materials) and the `lunar` style's special materials.
    /// Warps must use the `distance` radius and the `taper` falloff, and foils λ² = 0.25. The
    /// player moves by the built-in game settings (`set_settings`), so the map's gravity and jump
    /// speed must be above 0. On a map with team consoles and one black hole, the black hole grows
    /// (`can_grow_black_hole`).
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
        let (mut meshes, mut mesh_keys) = (Vec::<MeshModel>::new(), Vec::<(String, f64)>::new());
        for s in &map.structures {
            let first_shape: usize = shapes.len();
            for shape in &s.shapes {
                shapes.push(Shape::FromMap(shape).map_err(|e| format!("map {}: {e}", map.name))?);
            }
            let mesh: Option<usize> = match &s.mesh {
                None => None,
                Some(mesh) => {
                    let key: (String, f64) = (mesh.model.clone(), mesh.height_m);
                    if !mesh_keys.contains(&key) {
                        meshes.push(MeshModel::Load(&mesh.model, mesh.height_m)?);
                        mesh_keys.push(key.clone());
                    }
                    mesh_keys.iter().position(|k| *k == key)
                }
            };
            let frame: DMat4 = to_frame(&s.frame);
            structures.push(make_structure(
                &s.name,
                frame,
                first_shape,
                &shapes[first_shape..],
                mesh.map(|m| (m, &meshes[m])),
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
                color: None,
                is_flashing: false,
                lens_rad: 0.0,
                lens_core: 0.0,
            });
        }
        if let Some(material) = map
            .materials
            .iter()
            .find(|m| usize::from(m.id) >= FIRST_GAME_MATERIAL)
        {
            return Err(format!(
                "map {}: material {:?} has id {}; ids from {FIRST_GAME_MATERIAL} are the game's",
                map.name, material.name, material.id
            ));
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
            dark_regolith: find_material(map, "dark regolith").max(find_material(map, "dark sand")),
            basalt: find_material(map, "basalt").max(find_material(map, "sandstone")),
            solar: find_material(map, "solar panel"),
            hazard: find_material(map, "hazard paint"),
            pad: find_material(map, "landing pad"),
            hull: find_material(map, "hull"),
            gold_foil: find_material(map, "gold foil"),
            console_first: consoles.first().copied().unwrap_or(-1),
            console_count: consoles.len() as i32,
        };
        let team_consoles: Vec<Console> = find_consoles(map);
        let black_holes: Vec<usize> = (0..warps.len()).filter(|&w| !warps[w].is_mouth).collect();
        let growing_hole: Option<(usize, VoxelWarp)> = match black_holes[..] {
            [w] if !team_consoles.is_empty() => Some((w, warps[w].clone())),
            _ => None,
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
        let body: AvatarModel = AvatarModel::Load(BODY_MODEL, AVATAR_HEIGHT_M)?;
        let body_mesh: usize = meshes.len();
        meshes.push(MeshModel {
            triangles: Vec::new(),
            nodes: Vec::new(),
            materials: body.get_materials().to_vec(),
        });
        let mut world: VoxelWorld = VoxelWorld {
            name: map.name.clone(),
            r,
            terrain: make_terrain(map),
            wind: map
                .atmosphere
                .and_then(|a| a.wind)
                .map(|wind| WindField::FromMap(&wind, r)),
            fog: map.atmosphere.and_then(|a| a.fog),
            sand: map.atmosphere.and_then(|a| a.sand),
            laser_color: map.appearance.laser_color,
            static_structure_count: structures.len(),
            static_warp_count: warps.len(),
            consoles: team_consoles,
            flashlight_eye: None,
            growing_hole,
            sky_time_s: 0.0,
            structures,
            shapes,
            meshes: Arc::new(meshes),
            body: Arc::new(body),
            body_mesh,
            avatar_meshes: Vec::new(),
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
            zone_walls: match &map.regions {
                Some(regions) => ZoneWalls::FromRegions(regions)?,
                None => None,
            },
            sun: to_direction(&map.sky.sun.direction),
            planet: map
                .sky
                .planet
                .as_ref()
                .map(|p| (to_direction(&p.direction), p.angular_radius_rad)),
            milky_way: map.sky.milky_way.as_ref().map(|m| {
                (
                    to_direction(&m.pole_direction),
                    to_direction(&m.core_direction),
                )
            }),
            milky_way_brightness_frac: PerPlayerSettings::Builtin().view.milky_way_brightness_frac,
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
        world.find_console_pylons();
        Ok(world)
    }

    /// `find_console_pylons` gives each console its pylon, the structure with the console's
    /// shape, and that pylon's lamp, the light nearest its centre within its bounding sphere, so
    /// `light_consoles` can darken the whole pylon when its console is off.
    ///
    /// Example: `world.find_console_pylons();`
    fn find_console_pylons(&mut self) {
        for console in &mut self.consoles {
            let pylon: Option<usize> = self.structures.iter().position(|s| {
                self.shapes[s.first_shape..s.first_shape + s.shape_count]
                    .iter()
                    .any(|shape| shape.material == console.material)
            });
            console.pylon = pylon;
            console.lamp = pylon.and_then(|k| {
                let structure: &VoxelStructure = &self.structures[k];
                self.lights
                    .iter()
                    .enumerate()
                    .map(|(i, &(x, color, _))| (i, color, get_distance(x, structure.centre)))
                    .filter(|&(_, _, distance)| distance < structure.radius)
                    .min_by(|a, b| a.2.total_cmp(&b.2))
                    .map(|(i, color, _)| (i, color))
            });
        }
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

    /// `with_avatars` gives a copy of the world with a body for each of `bodies` (a feet frame,
    /// an avatar colour index and a height share): a cylinder and a head in its own material,
    /// squashed to the height share, so it is lensed and hit by lasers like any structure, and
    /// drawn as such.
    ///
    /// Example: `let shared = world.with_avatars(&[(feet, 2)], &colors);`
    pub fn with_avatars(&self, bodies: &[AvatarBody], colors: &[[f32; 3]]) -> VoxelWorld {
        let mut shared: VoxelWorld = self.clone();
        for &(feet, color_index, height_frac) in bodies {
            shared.add_avatar(feet, color_index, colors, None, height_frac);
        }
        shared
    }

    /// `with_posed_avatars` is `with_avatars`, but draws each body as the body model in its pose
    /// in `poses` (a clip and a time in it, one per body), squashed to its height share; lasers
    /// still hit the shapes. After the
    /// bodies come `corpses` (a feet frame, an avatar colour index and a pose each), only drawn:
    /// lasers pass through them.
    ///
    /// Example: `let shared = world.with_posed_avatars(&[(feet, 2)], &[(AvatarClip::Idle, 0.0)], &[], &colors);`
    pub fn with_posed_avatars(
        &self,
        bodies: &[AvatarBody],
        poses: &[(AvatarClip, f64)],
        corpses: &[(DMat4, usize, (AvatarClip, f64))],
        colors: &[[f32; 3]],
    ) -> VoxelWorld {
        assert_eq!(
            bodies.len(),
            poses.len(),
            "with_posed_avatars: {} bodies but {} poses",
            bodies.len(),
            poses.len()
        );
        let mut shared: VoxelWorld = self.clone();
        for (&(feet, color_index, height_frac), &(clip, time_s)) in bodies.iter().zip(poses) {
            let posed: MeshModel = self.body.make_squashed(clip, time_s, height_frac);
            shared.add_avatar(feet, color_index, colors, Some(posed), height_frac);
        }
        for &(feet, color_index, (clip, time_s)) in corpses {
            let posed: MeshModel = self.body.make_posed(clip, time_s);
            let drawn: Option<(usize, &MeshModel)> = Some((shared.avatar_meshes.len(), &posed));
            let mut corpse: VoxelStructure =
                make_structure("corpse", feet, shared.shapes.len(), &[], drawn, self.r);
            corpse.avatar_mesh = corpse.mesh.take();
            corpse.body_material = Some(shared.set_avatar_color(color_index, colors));
            shared.avatar_meshes.push(Arc::new(posed));
            shared.push_structure(corpse);
        }
        shared
    }

    /// `add_avatar` adds a body standing on `feet` in avatar colour `color_index`, its shapes
    /// squashed toward the feet to `height_frac` of their height, drawn as `posed` if given, and
    /// puts it in the list of every warp it comes within reach of.
    ///
    /// Example: `shared.add_avatar(feet, 2, &colors, None, 1.0);`
    fn add_avatar(
        &mut self,
        feet: DMat4,
        color_index: usize,
        colors: &[[f32; 3]],
        posed: Option<MeshModel>,
        height_frac: f64,
    ) {
        let material: u8 = self.set_avatar_color(color_index, colors);
        let first_shape: usize = self.shapes.len();
        let squash: DVec3 = DVec3::new(1.0, height_frac, 1.0);
        self.shapes.extend(AVATAR_SHAPES.map(|shape| Shape {
            material,
            centre: shape.centre * squash,
            half: shape.half * squash,
            round_m: shape.round_m.min(shape.half.y * height_frac),
            ..shape
        }));
        let body: &[Shape] = &self.shapes[first_shape..];
        let drawn: Option<(usize, &MeshModel)> = posed
            .as_ref()
            .map(|model| (self.avatar_meshes.len(), model));
        let mut avatar: VoxelStructure =
            make_structure("player", feet, first_shape, body, drawn, self.r);
        avatar.avatar_mesh = avatar.mesh.take();
        avatar.body_material = Some(material);
        avatar.avatar_height_frac = height_frac;
        if let Some(model) = posed {
            self.avatar_meshes.push(Arc::new(model));
        }
        self.push_structure(avatar);
    }

    /// `set_avatar_color` puts avatar colour `color_index` of `colors` in its palette entry and
    /// gives that entry.
    ///
    /// Example: `let material: u8 = shared.set_avatar_color(1, &colors);`
    fn set_avatar_color(&mut self, color_index: usize, colors: &[[f32; 3]]) -> u8 {
        let material: usize = AVATAR_FIRST_MATERIAL + color_index % colors.len();
        let color: [f32; 3] = colors[color_index % colors.len()];
        self.palette[material] = [color[0], color[1], color[2], 0.0];
        u8::try_from(material).expect("avatar materials fit the 256-entry palette")
    }

    /// `add_structure` adds a structure of `shapes` in `frame` after the others, in the list of
    /// every warp it comes within reach of.
    ///
    /// Example: `world.add_structure("a thrown wormhole", frame, &[ball]);`
    fn add_structure(&mut self, name: &str, frame: DMat4, shapes: &[Shape]) {
        let first_shape: usize = self.shapes.len();
        self.shapes.extend_from_slice(shapes);
        let structure: VoxelStructure = make_structure(
            name,
            frame,
            first_shape,
            &self.shapes[first_shape..],
            None,
            self.r,
        );
        self.push_structure(structure);
    }

    /// `push_structure` adds `structure` after the others, in the list of every warp it comes
    /// within reach of.
    ///
    /// Example: `shared.push_structure(avatar);`
    fn push_structure(&mut self, structure: VoxelStructure) {
        let k: usize = self.structures.len();
        for warp in &mut self.warps {
            if get_distance(structure.centre, warp.centre) < structure.radius + warp.reach {
                warp.list.push(k);
            }
        }
        self.structures.push(structure);
    }

    /// `add_markers` adds a glowing ball for each of `markers` (at most `MAX_MARKER_COUNT`, each in
    /// a material of its own): a thrown mouth in the air, or one down whose partner isn't. Add them
    /// after every avatar, so the avatars' structures keep their places.
    ///
    /// Example: `shared.add_markers(&wormholes.get_markers(world.r));`
    pub fn add_markers(&mut self, markers: &[Marker]) {
        assert!(
            markers.len() <= MAX_MARKER_COUNT,
            "add_markers: {} markers, more than {MAX_MARKER_COUNT}",
            markers.len()
        );
        for (i, marker) in markers.iter().enumerate() {
            let material: usize = FIRST_GAME_MATERIAL + i;
            let [r, g, b] = marker.rgb;
            self.palette[material] = [r, g, b, 1.0];
            let (x, radius_m) = (marker.x, marker.radius_m);
            let ball: Shape = Shape {
                kind: ShapeKind::Ellipsoid,
                centre: DVec3::ZERO,
                half: DVec3::splat(radius_m),
                round_m: 0.0,
                axis: 1,
                material: material as u8,
            };
            let frame: DMat4 =
                orthonormalize_frame(DMat4::from_cols(DVec4::X, DVec4::Y, DVec4::Z, x));
            self.add_structure(MARKER_NAME, frame, &[ball]);
        }
    }

    /// `get_thrown_size` gives a thrown mouth's m and reach, in curvature units, by the settings.
    ///
    /// Example: `let (m, reach) = world.get_thrown_size();`
    fn get_thrown_size(&self) -> (f64, f64) {
        let wormholes = &self.settings.wormholes;
        (
            wormholes.schwarzschild_radius_m / 4.0 / self.r,
            wormholes.reach_m / self.r,
        )
    }

    /// `set_thrown_wormholes` replaces the thrown mouths with `pairs`, each a purple and a green
    /// place linked to each other, after the map's own warps and before any avatar is added.
    ///
    /// Example: `world.set_thrown_wormholes(&[[purple, green]]);`
    pub fn set_thrown_wormholes(&mut self, pairs: &[[MouthPlace; 2]]) {
        assert_eq!(
            self.structures.len(),
            self.static_structure_count,
            "set_thrown_wormholes: the world has avatars"
        );
        self.warps.truncate(self.static_warp_count);
        let (m, reach) = self.get_thrown_size();
        for pair in pairs {
            let first: usize = self.warps.len();
            for (side, (place, color)) in pair
                .iter()
                .zip([WormholeColor::Purple, WormholeColor::Green])
                .enumerate()
            {
                let frame: DMat4 = place.get_frame(self.r);
                let centre: DVec4 = frame.col(AXIS_W);
                self.warps.push(VoxelWarp {
                    name: format!("the {} wormhole", color.get_name().to_lowercase()),
                    is_mouth: true,
                    frame,
                    inverse: invert_lorentz(frame),
                    centre,
                    m,
                    reach,
                    partner: first + 1 - side,
                    list: (0..self.structures.len())
                        .filter(|&k| {
                            let structure: &VoxelStructure = &self.structures[k];
                            get_distance(structure.centre, centre) < structure.radius + reach
                        })
                        .collect(),
                    color: Some(color),
                    is_flashing: false,
                    lens_rad: 0.0,
                    lens_core: 0.0,
                });
            }
        }
    }

    /// `set_flashing_pairs` makes the thrown pairs whose entry in `flashing` is true (in the order
    /// `set_thrown_wormholes` was given them) flash red.
    ///
    /// Example: `world.set_flashing_pairs(&[false, true]);`
    pub fn set_flashing_pairs(&mut self, flashing: &[bool]) {
        let first: usize = self.static_warp_count;
        for (k, warp) in self.warps[first..].iter_mut().enumerate() {
            warp.is_flashing = flashing[k / 2];
        }
    }

    /// `can_place_mouth` says whether a thrown mouth may hover at `place`: inside the map, with its
    /// reach clear of every map warp's and of those of the thrown mouths at `others` (reaches that
    /// taper may not overlap).
    ///
    /// Example: `if world.can_place_mouth(&place, &others) { ... }`
    pub fn can_place_mouth(&self, place: &MouthPlace, others: &[MouthPlace]) -> bool {
        let centre: DVec4 = place.get_centre(self.r);
        let (_, reach) = self.get_thrown_size();
        !self.is_outside_boundary(centre)
            && self.warps[..self.static_warp_count]
                .iter()
                .all(|w| get_distance(centre, w.centre) >= reach + w.reach)
            && others
                .iter()
                .all(|other| get_distance(centre, other.get_centre(self.r)) >= 2.0 * reach)
    }

    /// `find_hit_s` gives how far along a straight chord from x along unit v (at most `s_max`, in
    /// curvature units) the ground or a structure is, for a thrown mouth's flight.
    ///
    /// Example: `let s = world.find_hit_s(x, v, 0.01);`
    pub fn find_hit_s(&self, x: DVec4, v: DVec4, s_max: f64) -> Option<f64> {
        self.trace_scene(x, v, s_max, None).map(|hit| hit.s)
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
            let is_off: bool = off_mask >> i & 1 == 1;
            let entry: [f32; 4] = if is_off {
                let [r, g, b] = console.color.map(|c| c * CONSOLE_OFF_FRAC);
                [r, g, b, 0.0]
            } else {
                let level: f32 = console.pulse.get_level_frac();
                let [r, g, b] = console.color.map(|c| c * level);
                [r, g, b, 1.0]
            };
            self.palette[usize::from(console.material)] = entry;
            if let Some(k) = console.pylon {
                self.structures[k].is_dark = is_off;
            }
            if let Some((light, color)) = console.lamp {
                self.lights[light].1 = if is_off {
                    color.map(|c| c * CONSOLE_OFF_FRAC)
                } else {
                    color
                };
            }
        }
    }

    /// `can_grow_black_hole` says whether this map's black hole grows as consoles turn off (W-68):
    /// it has team consoles and one black hole.
    ///
    /// Example: `if world.can_grow_black_hole() { world.grow_black_hole(0.4) }`
    pub fn can_grow_black_hole(&self) -> bool {
        self.growing_hole.is_some()
    }

    /// `grow_black_hole` grows the black hole to `strength_frac` (0 to 1) by the settings'
    /// `black_hole`: its Schwarzschild radius and reach grow from the map's to the settings'
    /// maximum in proportion to the strength, its wide lens from none to the settings'
    /// `max_lens_rad` (with a core of `lens_core_per_reach` of its reach), and it stays where the
    /// map has it.
    ///
    /// Example: `world.grow_black_hole(0.4);`
    pub fn grow_black_hole(&mut self, strength_frac: f64) {
        let (index, hole) = self.growing_hole.clone().unwrap_or_else(|| {
            panic!(
                "grow_black_hole: map {} has no black hole that grows",
                self.name
            )
        });
        let settings = &self.settings.black_hole;
        let full_strength_mass: f64 = settings.max_schwarzschild_radius_m / 4.0 / self.r;
        let full_strength_reach: f64 = settings.max_reach_m / self.r;
        let reach: f64 = hole.reach + (full_strength_reach - hole.reach) * strength_frac;
        self.warps[index] = VoxelWarp {
            m: hole.m + (full_strength_mass - hole.m) * strength_frac,
            reach,
            lens_rad: settings.max_lens_rad * strength_frac,
            lens_core: settings.lens_core_per_reach * reach,
            ..hole
        };
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

    /// `is_way_clear` says whether a player's body `body`, its eye `h_m` up, could walk
    /// `distance_m` straight ahead as `VoxelPlayer::step` moves it: at every `WAY_PROBE_STEP_M` of
    /// the way, inside the boundary and free of blocks or only a walkable rise up. A walking bot
    /// probes its way with it.
    ///
    /// Example: `if world.is_way_clear(player.body, player.h_m, 1.5) { ... }`
    pub fn is_way_clear(&self, body: DMat4, h_m: f64, distance_m: f64) -> bool {
        let step_count: usize = (distance_m / WAY_PROBE_STEP_M).ceil() as usize;
        (1..=step_count).all(|i| {
            let along_m: f64 = distance_m * i as f64 / step_count as f64;
            let ahead: DMat4 = renormalize_frame(body * make_boost(AXIS_Z, -along_m / self.r));
            let nearby: Vec<usize> = self.find_nearby_structures(ahead.col(AXIS_W), NEARBY_REACH_M);
            !self.is_outside_boundary(ahead.col(AXIS_W))
                && (!self.is_body_blocked(ahead, h_m, &nearby)
                    || self.find_walk_rise_m(ahead, h_m, &nearby).is_some())
        })
    }

    /// `raise_crouched_eye` gives the eye frame a player whose eye frame is `eye` (from the
    /// network) and who is `crouch_frac` of the way down a crouch would have standing: raised
    /// along the vertical by the drop crouching gave it, so the body under it stands on the
    /// ground (W-137 (crouched robots sink)). It is `eye` itself when not crouched.
    ///
    /// Example: `let standing_eye = world.raise_crouched_eye(player.frame, player.crouch_frac);`
    pub fn raise_crouched_eye(&self, eye: DMat4, crouch_frac: f64) -> DMat4 {
        let drop_m: f64 = self.player.eye_height_m
            * (1.0 - get_crouched_height_frac(crouch_frac, &self.settings.movement));
        if drop_m == 0.0 {
            return eye;
        }
        let seen: VoxelPlayer = VoxelPlayer::FromEye(self, eye);
        let at_eye: DMat4 = seen.body * make_boost(AXIS_Y, seen.h_m / self.r);
        at_eye * make_boost(AXIS_Y, drop_m / self.r) * invert_lorentz(at_eye) * eye
    }

    /// `is_eye_supported` says whether a player whose eye frame is `eye` (from the network) stands
    /// on something: their body, `GROUND_PROBE_M` lower, would meet the ground or a block.
    ///
    /// Example: `let is_on_ground = world.is_eye_supported(player.frame);`
    pub fn is_eye_supported(&self, eye: DMat4) -> bool {
        let player: VoxelPlayer = VoxelPlayer::FromEye(self, eye);
        let nearby: Vec<usize> = self.find_nearby_structures(eye.col(AXIS_W), NEARBY_REACH_M);
        self.is_body_blocked(player.body, player.h_m - GROUND_PROBE_M, &nearby)
    }

    /// `find_clear_m` gives how far the straight chord from point x along unit tangent v goes, up to
    /// `max_m`, before it meets the ground or a block, glass included: a camera can't pass a pane.
    ///
    /// Example: `let clear_m = world.find_clear_m(eye.col(AXIS_W), eye.col(AXIS_Z), 3.0);`
    pub fn find_clear_m(&self, x: DVec4, v: DVec4, max_m: f64) -> f64 {
        self.trace_scene_through(x, v, max_m / self.r, None, None)
            .map_or(max_m, |hit| hit.s * self.r)
    }

    /// `find_crosshair_point` gives the point the straight ray from `camera`'s eye along its
    /// crosshair first meets, ignoring the warps' bending: the ground or a block (through glass, as
    /// the laser goes), else the end of the laser's range. Two nearby rays aimed at it, as from the
    /// eye and from a camera behind it, bend nearly alike on the way.
    ///
    /// Example: `let target = world.find_crosshair_point(camera);`
    pub fn find_crosshair_point(&self, camera: DMat4) -> DVec4 {
        let (x, v) = (camera.col(AXIS_W), -camera.col(AXIS_Z));
        let max_s: f64 = LASER_MAX_LENGTH_M / self.r;
        let s: f64 = self
            .trace_scene(x, v, max_s, None)
            .map_or(max_s, |hit| hit.s);
        geodesic_point(x, v, s)
    }

    /// `is_head_hit` says whether point x, on an avatar (structure `k`, from `with_avatars`), is in
    /// its top cell, its head (lower by the body's height share while crouched).
    ///
    /// Example: `let is_head = shared.is_head_hit(k, spot);`
    pub fn is_head_hit(&self, k: usize, x: DVec4) -> bool {
        let p: DVec4 = self.structures[k].inverse * x;
        p.y / p.w * self.r >= AVATAR_HEAD_FROM_M * self.structures[k].avatar_height_frac
    }

    /// `get_material_at` gives the material of the cell holding point x among `candidates`, or 0.
    /// A structure drawn as a mesh holds the points within `MESH_WALL_M` of its triangles, in the
    /// material of its first shape.
    ///
    /// Example: `let m = world.get_material_at(x, &nearby);`
    fn get_material_at(&self, x: DVec4, candidates: &[usize]) -> u8 {
        for &k in candidates {
            let p: DVec4 = self.structures[k].inverse * x;
            let q: DVec3 = p.truncate() / p.w * self.r;
            if let Some(m) = self.structures[k].mesh {
                if self.meshes[m].is_within_m(q, MESH_WALL_M) {
                    return self.get_shapes(k)[0].material;
                }
                continue;
            }
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
        map_edge::get_edge_frac(
            terrain.get_wall_frac(u_m, v_m) - edge.start_wall_frac,
            edge.stop_wall_frac - edge.start_wall_frac,
        )
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
    /// block or the ground, by probe points at its feet and head and rings of `RING_PROBE_COUNT`
    /// at its knees and waist, so turning in place doesn't swing a probe into a wall.
    ///
    /// Example: `if world.is_body_blocked(body, h_m, &nearby) { ... }`
    fn is_body_blocked(&self, body: DMat4, h_m: f64, nearby: &[usize]) -> bool {
        let eye_m: f64 = self.player.eye_height_m;
        let ring = (0..RING_PROBE_COUNT)
            .map(|i| std::f64::consts::TAU * i as f64 / RING_PROBE_COUNT as f64);
        let probes: Vec<[f64; 3]> = [
            [0.0, PROBE_LOW_M, 0.0],
            [0.0, eye_m + HEAD_CLEARANCE_M, 0.0],
        ]
        .into_iter()
        .chain([PROBE_KNEE_M, PROBE_WAIST_M].into_iter().flat_map(|y_m| {
            ring.clone()
                .map(move |a: f64| [BODY_RADIUS_M * a.cos(), y_m, BODY_RADIUS_M * a.sin()])
        }))
        .collect();
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
            let (lens_slope, lens_log) = get_lens_log_index(warp, rho);
            (
                -warp.m / (rho * rho) * (3.0 / (1.0 + q) + 1.0 / guard) + lens_slope,
                3.0 * (1.0 + q).ln() - guard.ln() + lens_log,
            )
        };
        let slope: f64 = if f >= 1.0 {
            0.0
        } else {
            slope_n * window + log_n * window_slope
        };
        let mut g: DVec4 = slope * (ch * x - warp.centre) / sh;
        if let Some(wind) = &self.wind {
            g += wind.get_gradient(x);
        }
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

    /// `get_time_rate_frac` gives how fast time runs at x (W-107), as a share of time far from every
    /// black hole. Near each black hole it is general relativity's rate for someone standing still,
    /// (1 − q) / (1 + q) in isotropic coordinates, with q = k m / ρ: ρ the distance to its centre,
    /// m its horizon's radius in those coordinates, and k the settings'
    /// `black_hole.time_dilation_per_general_relativity` (1 for real). Its wide lens slows time
    /// too, as a weak field does: the rate's log falls by k times half the lens's ln n. The rate's
    /// log fades to 0 at the black hole's reach along the window its light bending uses, and the
    /// rate never drops below `black_hole.min_time_rate_frac`.
    ///
    /// Example: `let rate_frac = world.get_time_rate_frac(eye.col(AXIS_W));`
    pub fn get_time_rate_frac(&self, x: DVec4) -> f64 {
        let settings = &self.settings.black_hole;
        let log_rate: f64 = self
            .warps
            .iter()
            .filter(|warp| !warp.is_mouth)
            .map(|warp| {
                let rho: f64 = (warp.inverse * x).truncate().length().asinh();
                let f: f64 = rho / warp.reach;
                let k: f64 = settings.time_dilation_per_general_relativity;
                let q: f64 = k * warp.m / rho;
                let lens_log: f64 = get_lens_log_index(warp, rho).1;
                match (f < 1.0, q < 1.0) {
                    (false, _) => 0.0,
                    (true, false) => f64::NEG_INFINITY,
                    (true, true) => {
                        (((1.0 - q) / (1.0 + q)).ln() - k * lens_log / 2.0) * (1.0 - f * f).powi(2)
                    }
                }
            })
            .sum();
        log_rate.exp().max(settings.min_time_rate_frac)
    }

    /// `get_mouth_depth_frac` gives how deep x is inside a wormhole mouth: 1 within
    /// `INSIDE_MOUTH_SCALE` throat radii (ρ ≤ `INSIDE_MOUTH_SCALE` m, with ρ the distance to the
    /// mouth's centre), falling along the smoothstep 1 − t²(3 − 2t) to 0 at `INSIDE_MOUTH_SCALE` ×
    /// `INSIDE_MOUTH_PER_THROAT` throat radii, where the mouth bends space strongly; 0 farther out,
    /// and near a black hole. `INSIDE_MOUTH_SCALE` scales both radii together, and with them how
    /// long a walk through shows the tint.
    ///
    /// Must know:
    /// - Gotchas: walking through mouth A on Lunar Surface, the eye passes 1.2 throat radii from
    ///   the centre, not through it, so scaling 1.8 (not 2) is what doubles the tint's time.
    ///
    /// Example: `let depth_frac = world.get_mouth_depth_frac(eye);`
    pub fn get_mouth_depth_frac(&self, x: DVec4) -> f64 {
        match self.find_warp_inside(x) {
            Some(w) if self.warps[w].is_mouth => {
                let warp: &VoxelWarp = &self.warps[w];
                let rho: f64 = (-minkowski_dot(x, warp.centre)).max(1.0).acosh();
                let full_rho: f64 = INSIDE_MOUTH_SCALE * warp.m;
                let t: f64 = ((rho - full_rho) / ((INSIDE_MOUTH_PER_THROAT - 1.0) * full_rho))
                    .clamp(0.0, 1.0);
                1.0 - t * t * (3.0 - 2.0 * t)
            }
            _ => 0.0,
        }
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
    /// The chord passes through shapes of material `see_through`, if any (glass, as light and
    /// lasers do).
    ///
    /// Example: `let s = world.trace_structure(3, x, v, 0.0, 1.0, None);`
    fn trace_structure(
        &self,
        k: usize,
        x: DVec4,
        v: DVec4,
        s_min: f64,
        s_max: f64,
        see_through: Option<u8>,
    ) -> Option<f64> {
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
        let (mu, _) = self.find_hit_mu(k, q0, dq, mu_of(s_lo), mu_of(s_hi), see_through)?;
        let lam: f64 = mu * p.w * p.w / (1.0 - mu * p.w * t.w);
        Some(s_star + lam.atanh())
    }

    /// `trace_scene` finds the first thing a chord hits: the ground or a block (among all
    /// structures, or warp w's list), passing through glass as light and lasers do.
    ///
    /// Example: `let hit = world.trace_scene(x, v, 1.0, None);`
    fn trace_scene(&self, x: DVec4, v: DVec4, s_max: f64, warp: Option<usize>) -> Option<Hit> {
        let glass: Option<u8> = u8::try_from(self.special.glass).ok();
        self.trace_scene_through(x, v, s_max, warp, glass)
    }

    /// `trace_scene_through` is `trace_scene` passing through shapes of material `see_through`
    /// only, if any.
    ///
    /// Example: `let hit = world.trace_scene_through(x, v, 1.0, None, None);`
    fn trace_scene_through(
        &self,
        x: DVec4,
        v: DVec4,
        s_max: f64,
        warp: Option<usize>,
        see_through: Option<u8>,
    ) -> Option<Hit> {
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
            if let Some(s) = self.trace_structure(k, x, v, 0.0, limit, see_through) {
                best = Some(Hit {
                    structure: Some(k),
                    s,
                });
            }
        }
        best
    }

    /// `trace_laser` follows a shot from `camera`'s eye along its crosshair: straight between the
    /// warps, bent inside them, through throats, to a block, the ground, a horizon or
    /// `LASER_MAX_LENGTH_M` (through glass). The drawn beam starts at the muzzle (`MUZZLE_M` in camera space, where
    /// the window draws the tip of the gun in hand) and joins the path `LASER_MUZZLE_M` ahead,
    /// simplified into geodesic segments within `BEAM_TOLERANCE_M`.
    ///
    /// Example: `let shot = world.trace_laser(player.get_camera_frame(&world));`
    pub fn trace_laser(&self, camera: DMat4) -> Shot {
        let max_s: f64 = LASER_MAX_LENGTH_M / self.r;
        let laser_wind: Option<WindField> = self.wind.map(|wind| wind.get_laser_wind());
        let (mut x, mut v) = (camera.col(AXIS_W), -camera.col(AXIS_Z));
        let (mut travelled, mut inside) = (0.0, self.find_warp_inside(x));
        let mut pieces: Vec<Vec<DVec4>> = vec![vec![x]];
        let mut hit: Option<Hit> = None;
        let mut through: Vec<String> = Vec::new();
        let mut through_warps: Vec<usize> = Vec::new();
        for _ in 0..LASER_MAX_ITERATIONS {
            if travelled >= max_s {
                break;
            }
            let Some(w) = inside else {
                let segment_s: f64 = laser_wind.map_or(f64::INFINITY, |wind| wind.segment_s);
                let s_max: f64 = (max_s - travelled).min(segment_s);
                if let Some(wind) = &laser_wind {
                    v = wind.kick(x, v, 0.5 * s_max);
                }
                let (s_end, entering) = self.find_warp_entry(x, v, s_max);
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
                if let Some(wind) = &laser_wind {
                    v = wind.kick(x, v, 0.5 * s_end);
                    if entering.is_none() {
                        continue;
                    }
                }
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
                through_warps.push(w);
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
            through_warps,
        }
    }

    /// `simplify_path` turns a traced polyline into geodesic segments within `BEAM_TOLERANCE_M` of it.
    ///
    /// Example: `let segments = world.simplify_path(&points);`
    fn simplify_path(&self, points: &[DVec4]) -> Vec<BeamSegment> {
        let make_segment = BeamSegment::Between;
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
/// and how fast the player walked along the way they face (negative walking backwards; 0 standing
/// still, only stepping sideways, or blocked).
pub struct StepReport {
    pub messages: Vec<String>,
    pub forward_m_per_s: f64,
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
    /// level does) only if that fails. It returns messages for the HUD and how fast the player
    /// walked along the way they face.
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
        let mut state: MoveState = MoveState {
            crouch_frac: self.crouch_frac,
            energy_frac: self.energy_frac,
            is_sprinting: self.is_sprinting,
            bob_phase_rad: self.bob_phase_rad,
            bob_frac: self.bob_frac,
            slowed_s: self.slowed_s,
        };
        let move_input: MoveInput = MoveInput {
            is_moving: len > 0.0,
            is_sprint_held: input.sprint,
            is_crouch_held: input.crouch,
            is_flying: self.is_flying,
            is_on_ground: self.is_on_ground,
        };
        let rules: MoveRules = MoveRules {
            movement: &world.settings.movement,
            energy: &world.settings.sprint_energy,
            when_hit: &world.settings.when_hit,
            head_bob: &world.head_bob,
        };
        let slow_frac: f64 = state.step(&move_input, &rules, dt_s);
        MoveState {
            crouch_frac: self.crouch_frac,
            energy_frac: self.energy_frac,
            is_sprinting: self.is_sprinting,
            bob_phase_rad: self.bob_phase_rad,
            bob_frac: self.bob_frac,
            slowed_s: self.slowed_s,
        } = state;
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
                forward_m_per_s: 0.0,
            };
        }
        let (mut body_new, mut h_new, mut crossed) = (self.body, self.h_m, None);
        let has_moved: bool = len > 0.0;
        let walked_m_per_s: f64 = if has_moved {
            speed_m_per_s * forward / len
        } else {
            0.0
        };
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
                body_new = map_edge::take_step(distance_m, edge_frac, move_m, |body: &DMat4| {
                    world.get_edge_frac(body.col(AXIS_W))
                });
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
            let [u_m, v_m, _] = world.get_fermi_coordinates(self.body.col(AXIS_W));
            let floor_m: f64 = world
                .terrain
                .as_ref()
                .map_or(0.0, |t| t.get_height_m(u_m, v_m));
            self.h_m = self.h_m.clamp(floor_m + MIN_FLY_HEIGHT_M, MAX_FLY_HEIGHT_M);
            return StepReport {
                messages,
                forward_m_per_s: walked_m_per_s,
            };
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
        StepReport {
            messages,
            forward_m_per_s: if self.body == body_new {
                walked_m_per_s
            } else {
                0.0
            },
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the voxel world against the Lunar Surface level's own numbers.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Where a player starts | on the ground at the blue spawn, beside the blue kazemat, and at a viewpoint | `players_stand_where_the_level_puts_them` |
    //! | Laser path | through a wormhole to a structure | `the_laser_into_the_wormhole_hits_the_red_pylon` |
    //! | Laser at a window (the red habitat module's) | shot from inside touching the pane, close (the muzzle past the pane), across the room, slantwise (each leaves); from outside (comes in to the floor) | `a_laser_passes_through_a_window` |
    //! | What a window's pane stops | lasers (no), players (yes: its middle is solid) | `a_laser_passes_through_a_window` |
    //! | Players spawning | up to the player limit, and a late id whose own slot is next to player 1's (it moves on) | `players_spawn_apart` |
    //! | Beam drawn | inside the map (whole), out into the sky (stops 160 m from the origin), from the far end of the map at twice the curvature (drawn) | `beams_into_the_sky_stop_near_the_map` |
    //! | Beam clipped to | its middle (through the wormhole), before its start, past its end | `a_clipped_beam_keeps_its_middle` |
    //! | Obstacle walked into | one block (stops the player), one block while jumping (climbed), a wall turned along after touching it at an angle (not climbed) | `walking_into_a_block_does_not_climb_it`, `jumping_climbs_onto_a_block`, `turning_along_a_wall_does_not_climb_it` |
    //! | Jump on open ground | rises the settings' jump height and lands in their fraction of the map's jump time | `a_jump_is_low_and_quick` |
    //! | Walking | upright, crouched (half as far, eye fully lowered) | `crouching_lowers_the_eye_and_slows_walking` |
    //! | Speed a step reports | standing still (0), forward (walking speed), backward (minus it), sideways (0), forward and sideways (its forward part), into a block (0) | `steps_report_speed_along_the_facing` |
    //! | Mouth depth at | a mouth's centre and 1.8 throat radii (1), 5.4 throat radii (1/2), the inside's edge at 9 (0), halfway out the reach (0), inside the black hole's reach (0) | `mouth_depth_is_only_near_the_throat` |
    //! | Sprint energy | full (sprints at sprint speed, drains), empty with `Shift` held (walks, stays empty), `Shift` released (refills) | `sprinting_uses_energy` |
    //! | Refilling energy | crouched (at the crouched rate), walking upright (at the walking rate) | `crouching_refills_energy_faster` |
    //! | Map edge | walking out up the crater's wall (ever slower, never past the stop), walking back in (full speed) | `the_crater_wall_slows_without_end` |
    //! | Low way walked out of | a habitat module's doors (each module, each way), a walkway's ends (each walkway, each way) | `players_walk_out_of_modules_and_walkways` |
    //! | Turning while pressed on a bunker wall | the garage's corners (straight, diagonal), the field bunker's forecourt corner (diagonal); each walks away | `players_turning_at_bunker_walls_get_away` |
    //! | Bunker model walked out of | the kazemat's door, each of the garage's two doorways, the field bunker's room, forecourt and steps | `players_walk_out_of_bunkers` |
    //! | Floor walked on | the ground, a structure's deck above the ground (its cells' tops rise a little against the ground's height) | `walking_into_a_block_does_not_climb_it` (walks on the ground first), `walking_on_a_deck_moves` |
    //! | Black hole growth | 1 (the settings' maximum, with its wide lens), back to 0 (the map's, no lens) | `the_black_hole_grows_in_place` |
    //! | Distance from the black hole (time rate) | at its centre and inside k horizons (the floor), within its reach (between, slower closer in), at and past its reach (1) | `time_slows_near_the_black_hole` |
    //! | Black hole strength (time rate) | the map's, full (slower at the same place) | `time_slows_near_the_black_hole` |
    //! | Time dilation setting k | 0 (1 everywhere), 1 (general relativity's rate, tapered; with the wide lens at full strength) | `time_dilation_follows_the_setting` |
    //! | Warp | black hole (slows time), wormhole mouth (doesn't) | `time_dilation_follows_the_setting` |
    //! | Progress of turning off a console | none (the normal rate), part way (rising, faster each step), done (the frenzy), past done (fails) | `the_pulse_ramps_up_with_progress`, `progress_past_done_fails` |
    //! | Target rate against the pulse's own | higher (reached at once), the same (kept), lower (eases down: barely after a frame, nearly there after a second, never below) | `the_pulse_eases_back_down` |
    //! | Rate while the light is watched | steady, ramping up over a whole hold, dropping when the hold stops (the light moves only as fast as the pulse, never jumps) | `the_light_never_jumps` |
    //! | Console | off (dark, unlit), on and not held (the normal pulse), on and held (its own faster pulse) | `consoles_pulse_by_their_own_progress` |
    //! | Pylon of a console | console off (the whole pylon and its lamp dark), back on (lit again), another console's (untouched) | `a_pylon_goes_dark_with_its_console` |
    //! | Thrown pair | walked into (the walker comes out of the other mouth), shot into (the laser goes through, naming the mouth's warp), flashing (both mouths) | `a_thrown_pair_carries_walkers_and_lasers` |
    //! | Thrown mouth's place | clear of every warp, in a map warp's reach, in another thrown mouth's reach | `thrown_mouths_keep_clear_of_other_warps` |
    //! | Marker | inside a thrown mouth's reach (in its list, hit by a laser) | `a_marker_is_listed_and_hit` |
    //! | Thrown pair on Dust | shot into across terrain and wind (the laser goes through) | `a_dust_throw_carries_the_laser_through_the_wind` |
    //! | Dust's laser wind | laser against light across the wind (2 to 6 m farther downwind at 50 m) | `the_laser_bends_more_than_the_view` |
    //! | Dust's gusts | two places, two times (strength differs, direction stays downwind), worlds loaded apart at one UTC time, also years from 1970 (same gradient, same laser spot) | `gusts_change_strength_not_direction`, `two_worlds_agree_on_the_gusts` |

    use super::*;
    use crate::geometry::orthonormalize_frame;
    use crate::map::{Map, get_map_folder};
    use crate::settings::MAX_CURVATURE_PER_MAP_CURVATURE;
    use crate::thrown_wormholes::GREEN_COLOR;

    const STAND_TOLERANCE_M: f64 = 0.06;
    const PYLON_DISTANCE_M: f64 = 41.5;
    const PYLON_AIM_M: [f64; 2] = [-14.0, 40.05];
    const PYLON_AIM_PITCH_RAD: f64 = -0.004;
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
    const OPEN_U_M: f64 = -60.0;
    const OPEN_V_M: f64 = -30.0;
    const OPEN_GAP_M: f64 = 30.0;
    const WALK_IN_M: f64 = 4.0;
    const WALK_THROUGH_STEP_COUNT: usize = 120;
    const BLACK_HOLE_NEIGHBOUR_U_M: f64 = -20.0;
    const MARKER_RADIUS_M: f64 = 0.3;

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
        world.structures = vec![make_structure(
            "block",
            frame,
            0,
            &world.shapes,
            None,
            world.r,
        )];
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

    /// `make_open_world` gives the Lunar Surface world with flat ground and no structures, and a
    /// purple and a green place far from its warps, 30 m apart across the line v = -30 m.
    ///
    /// Example: `let (world, [purple, green]) = make_open_world();`
    fn make_open_world() -> (VoxelWorld, [MouthPlace; 2]) {
        let mut world: VoxelWorld = load_lunar_surface();
        world.terrain = None;
        (world.structures, world.shapes, world.static_structure_count) =
            (Vec::new(), Vec::new(), 0);
        for warp in &mut world.warps {
            warp.list.clear();
        }
        let eye_m: f64 = world.player.eye_height_m;
        let place = |v_m: f64| MouthPlace {
            u_m: OPEN_U_M,
            v_m,
            h_m: eye_m,
            yaw_rad: 0.0,
        };
        (world, [place(OPEN_V_M), place(OPEN_V_M - OPEN_GAP_M)])
    }

    #[test]
    fn a_thrown_pair_carries_walkers_and_lasers() {
        let (mut world, pair) = make_open_world();
        world.set_thrown_wormholes(&[pair]);
        let start: [f64; 2] = [OPEN_U_M - WALK_IN_M, OPEN_V_M];
        let mut player: VoxelPlayer =
            VoxelPlayer::AtView(&world, start, [OPEN_U_M, OPEN_V_M], None, 0.0);
        let shot: Shot = world.trace_laser(player.get_camera_frame(&world));
        assert_eq!(shot.through, vec!["the purple wormhole".to_string()]);
        assert_eq!(shot.through_warps, vec![world.static_warp_count]);
        world.set_flashing_pairs(&[true]);
        assert!(
            world.warps[world.static_warp_count..]
                .iter()
                .all(|w| w.is_flashing)
        );
        let input: Input = Input {
            forward: true,
            ..Input::default()
        };
        let messages: Vec<String> = (0..WALK_THROUGH_STEP_COUNT)
            .flat_map(|_| player.step(&world, &input, STEP_S, 0).messages)
            .collect();
        assert!(
            messages.contains(&"Through the purple wormhole".to_string()),
            "{messages:?}"
        );
        let [_, v_m, _] = world.get_fermi_coordinates(player.body.col(AXIS_W));
        assert!((v_m - pair[1].v_m).abs() < WALK_IN_M, "{v_m}");
    }

    #[test]
    fn a_marker_is_listed_and_hit() {
        let (mut world, pair) = make_open_world();
        world.set_thrown_wormholes(&[pair]);
        let player: VoxelPlayer = VoxelPlayer::AtView(
            &world,
            [OPEN_U_M - WALK_IN_M, OPEN_V_M + WALK_IN_M],
            [OPEN_U_M, OPEN_V_M + WALK_IN_M],
            None,
            0.0,
        );
        let camera: DMat4 = player.get_camera_frame(&world);
        let ahead: DVec4 = (camera * make_boost(AXIS_Z, -WALK_IN_M / 2.0 / world.r)).col(AXIS_W);
        world.add_markers(&[Marker {
            x: ahead,
            rgb: GREEN_COLOR,
            radius_m: MARKER_RADIUS_M,
            is_mouth: true,
        }]);
        assert!(world.warps[world.static_warp_count].list.contains(&0));
        assert_eq!(world.trace_laser(camera).hit_structure, Some(0));
    }

    #[test]
    fn thrown_mouths_keep_clear_of_other_warps() {
        let (world, [purple, green]) = make_open_world();
        assert!(world.can_place_mouth(&purple, &[green]));
        let near_black_hole: MouthPlace = MouthPlace {
            u_m: BLACK_HOLE_NEIGHBOUR_U_M,
            v_m: 0.0,
            ..purple
        };
        assert!(!world.can_place_mouth(&near_black_hole, &[]));
        let near_green: MouthPlace = MouthPlace {
            v_m: green.v_m + WALK_IN_M,
            ..green
        };
        assert!(!world.can_place_mouth(&near_green, &[green]));
    }

    #[test]
    fn players_stand_where_the_level_puts_them() {
        let world: VoxelWorld = load_lunar_surface();
        let eye_m: f64 = world.player.eye_height_m;
        let at_spawn: VoxelPlayer = VoxelPlayer::AtSpawn(&world, 0);
        let feet_m: f64 = get_eye_above_ground_m(&world, &at_spawn) - eye_m;
        assert!(
            feet_m.abs() < STAND_TOLERANCE_M,
            "the spawn stands {feet_m} m above the ground"
        );
        let on_ground: VoxelPlayer =
            VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        assert!((get_eye_above_ground_m(&world, &on_ground) - eye_m).abs() < STAND_TOLERANCE_M);
    }

    #[test]
    fn the_laser_into_the_wormhole_hits_the_red_pylon() {
        let world: VoxelWorld = load_lunar_surface();
        let player: VoxelPlayer = VoxelPlayer::AtView(
            &world,
            [-22.0, 40.5],
            PYLON_AIM_M,
            None,
            PYLON_AIM_PITCH_RAD,
        );
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
    fn the_black_hole_grows_in_place() {
        let world: VoxelWorld = load_lunar_surface();
        let mut grown: VoxelWorld = world.clone();
        grown.grow_black_hole(1.0);
        let (map_hole, hole) = (&world.warps[0], &grown.warps[0]);
        assert_eq!(hole.centre, map_hole.centre);
        let settings = &world.settings.black_hole;
        assert!((hole.m * 4.0 * grown.r - settings.max_schwarzschild_radius_m).abs() < 1e-9);
        assert!((hole.reach * grown.r - settings.max_reach_m).abs() < 1e-9);
        assert_eq!(hole.lens_rad, settings.max_lens_rad);
        assert!((hole.lens_core - settings.lens_core_per_reach * hole.reach).abs() < 1e-12);
        grown.grow_black_hole(0.0);
        assert_eq!(
            (
                grown.warps[0].m,
                grown.warps[0].reach,
                grown.warps[0].lens_rad
            ),
            (map_hole.m, map_hole.reach, 0.0)
        );
    }

    fn get_point_from_hole(world: &VoxelWorld, w: usize, distance_m: f64) -> DVec4 {
        let offset_m: DVec3 = DVec3::new(distance_m, 0.0, 0.0);
        world.warps[w].frame * map_exp(offset_m, world.r)
    }

    #[test]
    fn time_slows_near_the_black_hole() {
        let world: VoxelWorld = load_lunar_surface();
        let settings = &world.settings.black_hole;
        let (min_frac, hole) = (settings.min_time_rate_frac, &world.warps[0]);
        assert!(!hole.is_mouth);
        let rate_at = |world: &VoxelWorld, distance_m: f64| -> f64 {
            world.get_time_rate_frac(get_point_from_hole(world, 0, distance_m))
        };
        let reach_m: f64 = hole.reach * world.r;
        let horizon_m: f64 = hole.m * world.r * settings.time_dilation_per_general_relativity;
        assert_eq!(rate_at(&world, 0.0), min_frac);
        assert_eq!(rate_at(&world, 0.9 * horizon_m), min_frac);
        let near_frac: f64 = rate_at(&world, 0.25 * reach_m);
        let far_frac: f64 = rate_at(&world, 0.75 * reach_m);
        assert!(min_frac < near_frac && near_frac < far_frac && far_frac < 1.0);
        assert_eq!(rate_at(&world, reach_m), 1.0);
        assert_eq!(rate_at(&world, 2.0 * reach_m), 1.0);
        let mut grown: VoxelWorld = world.clone();
        grown.grow_black_hole(1.0);
        assert!(rate_at(&grown, 0.25 * reach_m) < near_frac);
    }

    #[test]
    fn time_dilation_follows_the_setting() {
        let mut world: VoxelWorld = load_lunar_surface();
        world
            .settings
            .black_hole
            .time_dilation_per_general_relativity = 0.0;
        assert_eq!(
            world.get_time_rate_frac(get_point_from_hole(&world, 0, 1.0)),
            1.0
        );
        world
            .settings
            .black_hole
            .time_dilation_per_general_relativity = 1.0;
        world.grow_black_hole(1.0);
        let hole: &VoxelWarp = &world.warps[0];
        let rho: f64 = hole.reach / 2.0;
        let q: f64 = hole.m / rho;
        let lens_log_n: f64 = hole.lens_rad * hole.lens_core / rho.hypot(hole.lens_core);
        let window: f64 = (1.0 - 0.25_f64).powi(2);
        let expected_frac: f64 = ((((1.0 - q) / (1.0 + q)).ln() - lens_log_n / 2.0) * window).exp();
        let at_half_reach: DVec4 = get_point_from_hole(&world, 0, rho * world.r);
        assert!((world.get_time_rate_frac(at_half_reach) - expected_frac).abs() < 1e-9);
        let mouth: usize = (0..world.warps.len())
            .find(|&w| world.warps[w].is_mouth)
            .expect("Lunar Surface has a wormhole mouth");
        let near_mouth: DVec4 =
            get_point_from_hole(&world, mouth, 2.0 * world.warps[mouth].m * world.r);
        assert_eq!(world.get_time_rate_frac(near_mouth), 1.0);
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
    fn steps_report_speed_along_the_facing() {
        let world: VoxelWorld = load_lunar_surface();
        let start: VoxelPlayer = VoxelPlayer::AtView(&world, [-27.0, 0.0], [40.0, 0.0], None, 0.0);
        let walk_m_per_s: f64 = world.player.walk_m_per_s;
        for (input, expected_m_per_s) in [
            (Input::default(), 0.0),
            (
                Input {
                    forward: true,
                    ..Input::default()
                },
                walk_m_per_s,
            ),
            (
                Input {
                    back: true,
                    ..Input::default()
                },
                -walk_m_per_s,
            ),
            (
                Input {
                    right: true,
                    ..Input::default()
                },
                0.0,
            ),
            (
                Input {
                    forward: true,
                    left: true,
                    ..Input::default()
                },
                walk_m_per_s / std::f64::consts::SQRT_2,
            ),
        ] {
            let mut player: VoxelPlayer = start;
            let report: StepReport = player.step(&world, &input, STEP_S, 0);
            assert!((report.forward_m_per_s - expected_m_per_s).abs() < 1e-9);
        }
        let (block_world, at_m, target_m) = make_block_world(1.0, 0.0);
        let mut blocked: VoxelPlayer = VoxelPlayer::AtView(&block_world, at_m, target_m, None, 0.0);
        walk_forward(&block_world, &mut blocked, WALK_STEP_COUNT);
        let into_block: Input = Input {
            forward: true,
            ..Input::default()
        };
        let report: StepReport = blocked.step(&block_world, &into_block, STEP_S, 0);
        assert_eq!(report.forward_m_per_s, 0.0);
    }

    #[test]
    fn mouth_depth_is_only_near_the_throat() {
        const FULL_PER_THROAT: f64 = 1.8;
        const HALF_WAY_PER_THROAT: f64 = 5.4;
        const EDGE_PER_THROAT: f64 = 9.0;
        const HALF_WAY_DEPTH_FRAC: f64 = 0.5;
        let world: VoxelWorld = load_lunar_surface();
        let at = |warp: &VoxelWarp, rho: f64| -> DVec4 {
            (warp.frame * make_boost(AXIS_X, rho)).col(AXIS_W)
        };
        let mouth: &VoxelWarp = world
            .warps
            .iter()
            .find(|warp| warp.is_mouth)
            .expect("Lunar Surface has a mouth");
        assert!((world.get_mouth_depth_frac(mouth.centre) - 1.0).abs() < 1e-9);
        let full: f64 = world.get_mouth_depth_frac(at(mouth, FULL_PER_THROAT * mouth.m));
        assert!((full - 1.0).abs() < 1e-9);
        let half_way: f64 = world.get_mouth_depth_frac(at(mouth, HALF_WAY_PER_THROAT * mouth.m));
        assert!((half_way - HALF_WAY_DEPTH_FRAC).abs() < 1e-6);
        let edge: DVec4 = at(mouth, EDGE_PER_THROAT * mouth.m);
        assert!(world.get_mouth_depth_frac(edge) < 1e-6);
        assert_eq!(
            world.get_mouth_depth_frac(at(mouth, mouth.reach / 2.0)),
            0.0
        );
        let black_hole: &VoxelWarp = world
            .warps
            .iter()
            .find(|warp| !warp.is_mouth)
            .expect("Lunar Surface has a black hole");
        assert_eq!(
            world.get_mouth_depth_frac(at(black_hole, black_hole.reach / 2.0)),
            0.0
        );
    }

    #[test]
    fn beams_into_the_sky_stop_near_the_map() {
        let world: VoxelWorld = load_lunar_surface();
        let into_wormhole: Shot = world.trace_laser(
            VoxelPlayer::AtView(&world, [-22.0, 40.0], [-14.0, 40.0], None, 0.0)
                .get_camera_frame(&world),
        );
        assert_eq!(
            clip_beam_to_map(&into_wormhole.segments, world.r).len(),
            into_wormhole.segments.len()
        );
        let into_sky: Shot = world.trace_laser(
            VoxelPlayer::AtView(&world, [-107.0, 0.0], [0.0, 0.0], None, SKY_PITCH_RAD)
                .get_camera_frame(&world),
        );
        let max_w: f64 = (MAX_DRAWN_BEAM_DISTANCE_M / world.r).cosh();
        let drawn: Vec<BeamSegment> = clip_beam_to_map(&into_sky.segments, world.r);
        let end: &BeamSegment = drawn.last().expect("the beam starts inside the map");
        let end_w: f64 = geodesic_point(end.a, end.b, end.length).w;
        assert!(into_sky.hit_structure.is_none() && end_w <= max_w && end_w > max_w * 0.99);
        let mut map: Map = Map::Load(&get_map_folder("lunar_surface")).expect("the map");
        map.scale_curvature(MAX_CURVATURE_PER_MAP_CURVATURE)
            .expect("Lunar Surface takes twice its curvature");
        let curved: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let from_red_spawn: Shot = curved.trace_laser(
            VoxelPlayer::AtView(&curved, [115.0, 0.0], [0.0, 0.0], None, 0.0)
                .get_camera_frame(&curved),
        );
        assert!(!clip_beam_to_map(&from_red_spawn.segments, curved.r).is_empty());
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
                "red habitat module" => Some((k, MODULE_OUT_M)),
                "red walkway" | "crater walkway" => Some((k, WALKWAY_OUT_M)),
                _ => None,
            })
            .collect();
        assert_eq!(ways.len(), 3, "one habitat module and two walkways");
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

    /// `BunkerRoute` is a walk out of a bunker: the structure's name, the start in its chart
    /// (x, z), the turns before each leg, and the chart (x, z) the walk must end past.
    struct BunkerRoute {
        name: &'static str,
        start_m: [f64; 2],
        turns_rad: &'static [f64],
        out_m: [f64; 2],
    }

    /// `players_walk_out_of_bunkers` checks that a player inside each bunker model walks out of
    /// its door (the field bunker's through its forecourt and down its steps), as the mesh's
    /// triangles, not boxes, stop players.
    #[test]
    fn players_walk_out_of_bunkers() {
        const LEG_S: f64 = 6.0;
        let world: VoxelWorld = load_lunar_surface();
        let route = |name: &'static str,
                     start_m: [f64; 2],
                     turns_rad: &'static [f64],
                     out_m: [f64; 2]| BunkerRoute {
            name,
            start_m,
            turns_rad,
            out_m,
        };
        let routes: [BunkerRoute; 4] = [
            route(
                "blue kazemat",
                [-0.2, 0.0],
                &[std::f64::consts::PI],
                [f64::MIN, 4.2],
            ),
            route(
                "red spawn hangar",
                [-7.4, 0.0],
                &[std::f64::consts::PI],
                [f64::MIN, 7.8],
            ),
            route(
                "red spawn hangar",
                [1.5, 0.0],
                &[std::f64::consts::PI],
                [f64::MIN, 7.8],
            ),
            route(
                "derelict field bunker",
                [0.0, 3.85],
                &[0.0, std::f64::consts::FRAC_PI_2],
                [6.0, f64::MIN],
            ),
        ];
        for BunkerRoute {
            name,
            start_m,
            turns_rad,
            out_m,
        } in routes
        {
            let k: usize = world
                .structures
                .iter()
                .position(|st| st.name == name)
                .expect(name);
            let frame: DMat4 = invert_lorentz(world.structures[k].inverse);
            let start: DVec4 = frame * map_klein(DVec3::new(start_m[0], 0.0, start_m[1]) / world.r);
            let body: DMat4 = orthonormalize_frame(DMat4::from_cols(
                frame.col(0),
                frame.col(1),
                frame.col(2),
                start,
            ));
            let mut player: VoxelPlayer = VoxelPlayer::Standing(&world, body);
            for &turn_rad in turns_rad {
                player.body *= make_rotation(AXIS_X, AXIS_Z, turn_rad);
                walk_forward(&world, &mut player, (LEG_S / STEP_S) as usize);
            }
            let p: DVec4 = world.structures[k].inverse * player.body.col(AXIS_W);
            let (x_m, z_m) = (p.x / p.w * world.r, p.z / p.w * world.r);
            assert!(
                x_m > out_m[0] && z_m > out_m[1],
                "{name} from {start_m:?}: stopped at ({x_m}, {z_m})"
            );
        }
    }

    /// `players_turning_at_bunker_walls_get_away` checks that a player who sprints into a bunker
    /// wall and turns while pressing on it can still walk away: each probe ring has
    /// `RING_PROBE_COUNT` probes, so turning doesn't swing one into the wall.
    #[test]
    fn players_turning_at_bunker_walls_get_away() {
        const TURN_RAD: f64 = 0.3;
        const TURN_COUNT: usize = 12;
        const TURN_FRAME_COUNT: usize = 10;
        const MIN_ESCAPE_M: f64 = 0.5;
        const FRAME_S: f64 = 1.0 / 30.0;
        let world: VoxelWorld = load_lunar_surface();
        let escapes: [Input; 4] = [
            Input {
                back: true,
                ..Input::default()
            },
            Input {
                left: true,
                ..Input::default()
            },
            Input {
                right: true,
                ..Input::default()
            },
            Input {
                forward: true,
                ..Input::default()
            },
        ];
        let eighth = |n: f64, radius_m: f64| -> [f64; 2] {
            let angle_rad: f64 = n * std::f64::consts::FRAC_PI_4;
            [radius_m * angle_rad.cos(), radius_m * angle_rad.sin()]
        };
        for (name, start_m, is_diagonal) in [
            ("red spawn hangar", eighth(7.0, 16.0), false),
            ("red spawn hangar", eighth(0.0, 16.0), true),
            ("derelict field bunker", eighth(5.0, 11.0), true),
        ] {
            let sprint: Input = Input {
                forward: true,
                left: is_diagonal,
                sprint: true,
                ..Input::default()
            };
            let k: usize = world
                .structures
                .iter()
                .position(|st| st.name == name)
                .expect(name);
            let frame: DMat4 = invert_lorentz(world.structures[k].inverse);
            let start: DVec4 = frame * map_klein(DVec3::new(start_m[0], 0.0, start_m[1]) / world.r);
            let body: DMat4 = orthonormalize_frame(DMat4::from_cols(
                frame.col(0),
                frame.col(1),
                frame.col(2),
                start,
            ));
            let facing: DMat4 = make_rotation(AXIS_X, AXIS_Z, (-start_m[0]).atan2(start_m[1]));
            let mut player: VoxelPlayer = VoxelPlayer::Standing(&world, body * facing);
            for _ in 0..(4.0 / FRAME_S) as usize {
                player.step(&world, &sprint, FRAME_S, 0);
            }
            for turn in 0..TURN_COUNT {
                player.body *= make_rotation(AXIS_X, AXIS_Z, TURN_RAD);
                for _ in 0..TURN_FRAME_COUNT {
                    player.step(&world, &sprint, FRAME_S, 0);
                }
                let escape_m: f64 = escapes
                    .iter()
                    .map(|input| {
                        let mut moved: VoxelPlayer = player;
                        for _ in 0..(1.0 / FRAME_S) as usize {
                            moved.step(&world, input, FRAME_S, 0);
                        }
                        get_distance(moved.body.col(AXIS_W), player.body.col(AXIS_W)) * world.r
                    })
                    .fold(0.0, f64::max);
                assert!(
                    escape_m > MIN_ESCAPE_M,
                    "{name} from {start_m:?}, turn {turn}: got only {escape_m} m away"
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
    fn a_pylon_goes_dark_with_its_console() {
        let mut world: VoxelWorld = load_lunar_surface();
        let fracs: [f64; MAX_CONSOLE_COUNT] = [0.0; MAX_CONSOLE_COUNT];
        let pylons: Vec<usize> = world
            .consoles
            .iter()
            .map(|c| c.pylon.expect("a pylon"))
            .collect();
        let lamps: Vec<(usize, [f32; 3])> = world
            .consoles
            .iter()
            .map(|c| c.lamp.expect("a lamp"))
            .collect();
        world.light_consoles(1 << OFF_CONSOLE, &fracs, STEP_S);
        for (i, (&k, &(light, color))) in pylons.iter().zip(&lamps).enumerate() {
            let is_off: bool = i == OFF_CONSOLE;
            assert_eq!(world.structures[k].is_dark, is_off, "pylon of console {i}");
            let expected: [f32; 3] = if is_off {
                color.map(|c| c * CONSOLE_OFF_FRAC)
            } else {
                color
            };
            assert_eq!(world.lights[light].1, expected, "lamp of console {i}");
        }
        world.light_consoles(0, &fracs, 2.0 * STEP_S);
        assert!(!world.structures[pylons[OFF_CONSOLE]].is_dark);
        assert_eq!(world.lights[lamps[OFF_CONSOLE].0].1, lamps[OFF_CONSOLE].1);
    }

    #[test]
    fn consoles_pulse_by_their_own_progress() {
        let mut world: VoxelWorld = load_lunar_surface();
        assert!(world.consoles.len() > OFF_CONSOLE);
        let mut disabling_fracs: [f64; MAX_CONSOLE_COUNT] = [0.0; MAX_CONSOLE_COUNT];
        disabling_fracs[HELD_CONSOLE] = 1.0;
        world.light_consoles(1 << OFF_CONSOLE, &disabling_fracs, STEP_S);
        let rates_hz: Vec<f64> = world.consoles.iter().map(|c| c.pulse.hz).collect();
        assert_eq!(rates_hz[OFF_CONSOLE], CONSOLE_PULSE_HZ);
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

    /// `the_wind_bends_light_toward_its_direction` checks Dust's wind field (W-121): its gradient
    /// at the origin is k along the wind, it turns a ray across the wind toward it, and leaves a
    /// ray along the wind alone.
    #[test]
    fn the_wind_bends_light_toward_its_direction() {
        const K: f64 = 0.06;
        let wind: WindField = WindField {
            normal: DVec4::new(1.0, 0.0, 0.0, 0.0),
            k: K,
            segment_s: 0.24,
            gust: None,
            time_s: 0.0,
            laser_k_per_k: 1.0,
        };
        let origin: DVec4 = DVec4::new(0.0, 0.0, 0.0, 1.0);
        let gradient: DVec4 = wind.get_gradient(origin);
        assert!(
            (gradient - DVec4::new(K, 0.0, 0.0, 0.0)).length() < 1e-12,
            "{gradient}"
        );
        let across: DVec4 = wind.kick(origin, DVec4::new(0.0, 0.0, -1.0, 0.0), 0.5);
        assert!(across.x > 0.0 && across.z < 0.0, "{across}");
        let along: DVec4 = wind.kick(origin, DVec4::new(1.0, 0.0, 0.0, 0.0), 0.5);
        assert!(
            (along - DVec4::new(1.0, 0.0, 0.0, 0.0)).length() < 1e-12,
            "{along}"
        );
    }

    /// `get_beam_point_m` gives the Fermi coordinates (u, v, h) of the point `s_m` along `shot`'s beam
    /// in `world`.
    fn get_beam_point_m(world: &VoxelWorld, shot: &Shot, s_m: f64) -> [f64; 3] {
        let mut gone_m: f64 = 0.0;
        for segment in &shot.segments {
            let length_m: f64 = segment.length * world.r;
            if gone_m + length_m >= s_m {
                let x: DVec4 = geodesic_point(segment.a, segment.b, (s_m - gone_m) / world.r);
                return world.get_fermi_coordinates(x);
            }
            gone_m += length_m;
        }
        panic!("the beam is {gone_m} m long, shorter than {s_m} m");
    }

    /// `ACROSS_WIND_AT_M` and `ACROSS_WIND_TARGET_M` are a clear lane across Dust's wind (which
    /// blows along v), and `BEND_AT_M` how far along it the bend is measured.
    const ACROSS_WIND_AT_M: [f64; 2] = [-70.0, -20.0];
    const ACROSS_WIND_TARGET_M: [f64; 2] = [70.0, -20.0];
    const BEND_AT_M: f64 = 50.0;

    /// `get_downwind_m` gives how far downwind (along v) `world`'s laser is `BEND_AT_M` along the
    /// lane across the wind.
    fn get_downwind_m(world: &VoxelWorld) -> f64 {
        let camera: DMat4 =
            VoxelPlayer::AtView(world, ACROSS_WIND_AT_M, ACROSS_WIND_TARGET_M, None, 0.0)
                .get_camera_frame(world);
        get_beam_point_m(world, &world.trace_laser(camera), BEND_AT_M)[1]
    }

    /// `the_dust_laser_bends_downwind` checks that on Dust a laser fired across the wind goes
    /// downwind of where it goes without the wind.
    #[test]
    fn the_dust_laser_bends_downwind() {
        let map: Map = Map::Load(&crate::map::get_map_folder("dust")).expect("the Dust map");
        let windy: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let calm: VoxelWorld = VoxelWorld {
            wind: None,
            ..windy.clone()
        };
        let (windy_m, calm_m) = (get_downwind_m(&windy), get_downwind_m(&calm));
        assert!(windy_m > calm_m, "{windy_m} vs {calm_m}");
    }

    /// `a_dust_throw_carries_the_laser_through_the_wind` checks that on Dust, with its terrain and
    /// wind, a laser aimed at a thrown purple mouth goes through it.
    #[test]
    fn a_dust_throw_carries_the_laser_through_the_wind() {
        const MOUTH_U_M: f64 = -20.0;
        const GREEN_V_M: f64 = -20.0;
        let map: Map = Map::Load(&get_map_folder("dust")).expect("the Dust map");
        let mut world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let eye_m: f64 = world.player.eye_height_m;
        let terrain: &Terrain = world.terrain.as_ref().expect("Dust's terrain");
        let place = |v_m: f64| MouthPlace {
            u_m: MOUTH_U_M,
            v_m,
            h_m: terrain.get_height_m(MOUTH_U_M, v_m) + eye_m,
            yaw_rad: 0.0,
        };
        let pair: [MouthPlace; 2] = [place(0.0), place(GREEN_V_M)];
        world.set_thrown_wormholes(&[pair]);
        let camera: DMat4 = VoxelPlayer::AtView(&world, [-30.0, 0.0], [MOUTH_U_M, 0.0], None, 0.0)
            .get_camera_frame(&world);
        let shot: Shot = world.trace_laser(camera);
        assert_eq!(
            shot.through.first().map(String::as_str),
            Some("the purple wormhole")
        );
    }

    /// `APPARENT_BEND_M` is the range the user asked for of a laser's bend as its shooter sees it,
    /// against the bent light, `BEND_AT_M` across the wind.
    const APPARENT_BEND_M: std::ops::Range<f64> = 2.0..6.0;

    /// `the_laser_bends_more_than_the_view` checks that on Dust a laser fired across the wind goes
    /// farther downwind than light does, by `APPARENT_BEND_M`, so its shooter sees it curve.
    #[test]
    fn the_laser_bends_more_than_the_view() {
        let map: Map = Map::Load(&crate::map::get_map_folder("dust")).expect("the Dust map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let wind: WindField = world.wind.expect("Dust's wind");
        let as_light: VoxelWorld = VoxelWorld {
            wind: Some(WindField {
                laser_k_per_k: 1.0,
                ..wind
            }),
            ..world.clone()
        };
        let apparent_bend_m: f64 = get_downwind_m(&world) - get_downwind_m(&as_light);
        assert!(
            APPARENT_BEND_M.contains(&apparent_bend_m),
            "{apparent_bend_m}"
        );
    }

    /// `gusts_change_strength_not_direction` checks that Dust's gusts make the wind stronger and
    /// weaker across places and times, while ∇ ln n keeps the calm wind's direction.
    #[test]
    fn gusts_change_strength_not_direction() {
        const TIMES_S: [f64; 2] = [0.0, 3.0];
        const PLACES_M: [[f64; 2]; 2] = [[0.0, 0.0], [30.0, -20.0]];
        let map: Map = Map::Load(&get_map_folder("dust")).expect("the Dust map");
        let world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel world");
        let mut wind: WindField = world.wind.expect("Dust's wind");
        let gust: GustField = wind.gust.expect("Dust's gusts");
        let calm: WindField = WindField { gust: None, ..wind };
        let mut strengths: Vec<f64> = Vec::new();
        for time_s in TIMES_S {
            wind.time_s = time_s;
            for [u_m, v_m] in PLACES_M {
                let x: DVec4 = VoxelPlayer::AtView(&world, [u_m, v_m], [u_m + 1.0, v_m], None, 0.0)
                    .get_camera_frame(&world)
                    .col(3);
                let (g, g_calm) = (wind.get_gradient(x), calm.get_gradient(x));
                let strength: f64 = gust.get_strength(x, time_s);
                assert!((g - strength * g_calm).length() < 1e-12 * g_calm.length());
                strengths.push(strength);
            }
        }
        assert!(
            strengths
                .iter()
                .all(|&s| (s - 1.0).abs() <= gust.strength_frac)
        );
        assert!(
            strengths
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() > 1e-3)
        );
    }

    /// `two_worlds_agree_on_the_gusts` checks that two windows (or a window and a server) loading
    /// Dust apart compute the same wind and the same laser spot at one UTC time, today and far on.
    #[test]
    fn two_worlds_agree_on_the_gusts() {
        const UTC_TIMES_S: [f64; 2] = [1.79e9, 4.1e9];
        let map: Map = Map::Load(&get_map_folder("dust")).expect("the Dust map");
        let mut worlds: Vec<VoxelWorld> = (0..2)
            .map(|_| VoxelWorld::FromMap(&map).expect("a voxel world"))
            .collect();
        for time_s in UTC_TIMES_S {
            for world in &mut worlds {
                world.set_sky_time_s(time_s);
            }
            let camera: DMat4 =
                VoxelPlayer::AtView(&worlds[0], [-60.0, 0.0], [60.0, 0.0], None, -0.05)
                    .get_camera_frame(&worlds[0]);
            let x: DVec4 = camera.col(3);
            let [a, b] = [&worlds[0], &worlds[1]].map(|w| w.wind.expect("wind").get_gradient(x));
            assert_eq!(a, b);
            let [spot_a, spot_b] = [&worlds[0], &worlds[1]].map(|w| w.trace_laser(camera).spot);
            assert_eq!(spot_a, spot_b);
        }
    }
}

#[cfg(test)]
mod parity_tests {
    //! Tests that Dust plays as Lunar Surface does, W-126 (same play in every level).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Level | Dust, Lunar Surface (the reference) | `a_jump_matches_lunar_surface` |
    //! | Console | each of the six (3 pylons a team) | `every_console_can_be_turned_off_on_foot` |
    //! | Side of a pylon | each of `SIDE_COUNT` directions around it | `every_console_can_be_turned_off_on_foot` |
    //! | Settings file | the server's, the player's | `the_settings_files_match_lunar_surface` |

    use super::*;
    use crate::map::{Map, get_map_folder};

    const STEP_S: f64 = 1.0 / 60.0;
    const MAX_JUMP_STEP_COUNT: usize = 120;
    const JUMP_HEIGHT_TOLERANCE_M: f64 = 0.05;
    const SIDE_COUNT: usize = 8;
    const GAP_TO_PYLON_M: f64 = 1.0;
    const CONSOLE_COUNT: usize = 6;
    const LUNAR_SETTINGS_PATH: &str = "../hyperbolic_wormholes_rust/server_settings.yaml";
    const DUST_ONLY_SETTINGS: [&str; 2] =
        ["jump_time_per_map_jump_time", "curvature_per_map_curvature"];
    const DUST_JUMP_AT_M: [f64; 2] = [-95.0, 6.0];
    const LUNAR_JUMP_AT_M: [f64; 2] = [-27.0, 0.0];

    /// `load_dust` builds Dust's world as the game does, with Dust's server settings.
    fn load_dust() -> VoxelWorld {
        VoxelWorld::FromMap(&Map::Load(&get_map_folder("dust")).expect("the Dust map"))
            .expect("a voxel world")
    }

    /// `measure_jump` gives a standing jump's height in meters and its time in the air in steps,
    /// at Fermi (u, v).
    fn measure_jump(world: &VoxelWorld, at_m: [f64; 2]) -> (f64, usize) {
        let mut player: VoxelPlayer =
            VoxelPlayer::AtView(world, at_m, [at_m[0] + 1.0, at_m[1]], None, 0.0);
        let start_m: f64 = player.h_m;
        let jump: Input = Input {
            up: true,
            ..Input::default()
        };
        player.step(world, &jump, STEP_S, 0);
        let (mut apex_m, mut air_step_count): (f64, usize) = (player.h_m - start_m, 1);
        while !player.is_on_ground {
            assert!(
                air_step_count < MAX_JUMP_STEP_COUNT,
                "the jump never landed"
            );
            player.step(world, &Input::default(), STEP_S, 0);
            apex_m = apex_m.max(player.h_m - start_m);
            air_step_count += 1;
        }
        (apex_m, air_step_count)
    }

    #[test]
    fn a_jump_matches_lunar_surface() {
        let lunar_map: Map =
            Map::Load(&get_map_folder("lunar_surface")).expect("the Lunar Surface map");
        let mut lunar: VoxelWorld = VoxelWorld::FromMap(&lunar_map).expect("a voxel world");
        lunar.set_settings(
            ServerSettings::Load(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(LUNAR_SETTINGS_PATH),
            )
            .expect("Lunar Surface's server settings"),
        );
        let (lunar_apex_m, lunar_step_count) = measure_jump(&lunar, LUNAR_JUMP_AT_M);
        let (dust_apex_m, dust_step_count) = measure_jump(&load_dust(), DUST_JUMP_AT_M);
        assert!(
            (dust_apex_m - lunar_apex_m).abs() < JUMP_HEIGHT_TOLERANCE_M,
            "Dust {dust_apex_m} m, Lunar Surface {lunar_apex_m} m"
        );
        assert!(
            dust_step_count.abs_diff(lunar_step_count) <= 1,
            "Dust {dust_step_count} steps in the air, Lunar Surface {lunar_step_count}"
        );
    }

    #[test]
    fn every_console_can_be_turned_off_on_foot() {
        let world: VoxelWorld = load_dust();
        assert_eq!(world.consoles.len(), CONSOLE_COUNT);
        for console in &world.consoles {
            let k: usize = console.pylon.expect("each console has its pylon");
            let [u_m, v_m, _] = get_fermi_coordinates(world.structures[k].centre, world.r);
            let gap_m: f64 = world.get_shapes(k)[0].half.x + GAP_TO_PYLON_M;
            for side in 0..SIDE_COUNT {
                let angle_rad: f64 = std::f64::consts::TAU * side as f64 / SIDE_COUNT as f64;
                let at_m: [f64; 2] = [u_m + gap_m * angle_rad.cos(), v_m + gap_m * angle_rad.sin()];
                let player: VoxelPlayer = VoxelPlayer::AtView(&world, at_m, [u_m, v_m], None, 0.0);
                let nearby: Vec<usize> =
                    world.find_nearby_structures(player.body.col(AXIS_W), NEARBY_REACH_M);
                assert!(
                    !world.is_body_blocked(player.body, player.h_m, &nearby),
                    "console {}: no room to stand at {at_m:?}",
                    console.material
                );
                let aimed: Option<u8> = world.find_aimed_material(
                    player.get_camera_frame(&world),
                    world.settings.consoles.reach_m,
                );
                assert_eq!(aimed, Some(console.material), "from {at_m:?}");
            }
        }
    }

    #[test]
    fn the_settings_files_match_lunar_surface() {
        let root: &std::path::Path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |path: &str| -> Vec<String> {
            std::fs::read_to_string(root.join(path))
                .expect("a settings file")
                .lines()
                .filter(|line| {
                    !DUST_ONLY_SETTINGS
                        .iter()
                        .any(|key| line.trim_start().starts_with(key))
                })
                .map(str::to_string)
                .collect()
        };
        assert_eq!(read("server_settings.yaml"), read(LUNAR_SETTINGS_PATH));
        assert_eq!(
            read("player_settings.yaml"),
            read("../hyperbolic_wormholes_rust/player_settings.yaml")
        );
    }
}

#[cfg(test)]
mod play_tests {
    //! Scripted play on Dust: spawns, walking everywhere from them, the level's wormhole and a
    //! thrown pair, W-126 (same play in every level).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Team | blue, red | `players_spawn_on_open_sand`, `walking_never_sticks_sinks_or_leaves` |
    //! | Players at a spawn | the first, up to `SPAWN_PLAYER_COUNT` on its rings | `players_spawn_on_open_sand` |
    //! | Heading from a spawn | each of `HEADING_COUNT` | `walking_never_sticks_sinks_or_leaves` |
    //! | Gait | walking, sprinting, jumping, crouching | `walking_never_sticks_sinks_or_leaves` |
    //! | Ridge (the dunes along v = ±25 m) | the north one, the south one, along each line of `RIDGE_CROSSING_U_M` (clear of props) | `walkers_cross_the_ridges` |
    //! | Wormhole | the level's pair, a thrown pair | `the_level_wormhole_carries_walkers_and_lasers`, `a_thrown_pair_carries_walkers_on_dust` |
    //! | Way into a level mouth | each of `HEADING_COUNT` | `the_level_wormhole_carries_walkers_and_lasers` |

    use super::*;
    use crate::map::{Map, get_map_folder};

    const STEP_S: f64 = 1.0 / 60.0;
    const SPAWN_PLAYER_COUNT: u32 = 8;
    const CLEAR_AHEAD_M: f64 = 20.0;
    const STAND_TOLERANCE_M: f64 = 0.05;
    const HEADING_COUNT: usize = 12;
    const WALK_STEP_COUNT: usize = 900;
    const GAIT_PERIOD_STEP_COUNT: usize = 60;
    const MIN_FIRST_SECOND_M: f64 = 5.0;
    const WALK_IN_M: f64 = 6.0;
    const WALK_THROUGH_STEP_COUNT: usize = 240;
    const NEAR_EXIT_M: f64 = 10.0;
    const MIN_WALK_ON_M: f64 = 3.0;
    const PURPLE_M: [f64; 2] = [-70.0, 30.0];
    const RIDGE_CROSSING_U_M: [f64; 6] = [-42.0, -30.0, -10.0, 10.0, 18.0, 40.0];
    const RIDGE_FROM_V_M: f64 = 12.0;
    const RIDGE_TO_V_M: f64 = 34.0;
    const RIDGE_WALK_STEP_COUNT: usize = 300;
    const MIN_RIDGE_CLIMB_M: f64 = 2.0;
    const GREEN_M: [f64; 2] = [70.0, -30.0];

    fn load_dust() -> VoxelWorld {
        VoxelWorld::FromMap(&Map::Load(&get_map_folder("dust")).expect("the Dust map"))
            .expect("a voxel world")
    }

    fn get_place_m(world: &VoxelWorld, player: &VoxelPlayer) -> [f64; 2] {
        let [u_m, v_m, _] = world.get_fermi_coordinates(player.body.col(AXIS_W));
        [u_m, v_m]
    }

    /// `get_eye_above_ground_m` gives how high a player's eye is above the terrain under them.
    fn get_eye_above_ground_m(world: &VoxelWorld, player: &VoxelPlayer) -> f64 {
        let [u_m, v_m] = get_place_m(world, player);
        player.h_m
            - world
                .terrain
                .as_ref()
                .expect("Dust's terrain")
                .get_height_m(u_m, v_m)
    }

    /// `walk` walks `player` forward until a step reports a message or `step_count` steps pass,
    /// and gives the messages.
    fn walk(world: &VoxelWorld, player: &mut VoxelPlayer, step_count: usize) -> Vec<String> {
        let input: Input = Input {
            forward: true,
            ..Input::default()
        };
        for _ in 0..step_count {
            let messages: Vec<String> = player.step(world, &input, STEP_S, 0).messages;
            if !messages.is_empty() {
                return messages;
            }
        }
        Vec::new()
    }

    #[test]
    fn players_spawn_on_open_sand() {
        let world: VoxelWorld = load_dust();
        let eye_m: f64 = world.player.eye_height_m;
        for team in 0..2 {
            let mut eyes: Vec<DVec4> = Vec::new();
            for id in 0..SPAWN_PLAYER_COUNT {
                let player: VoxelPlayer = VoxelPlayer::NearSpawn(&world, team, id, &eyes);
                let camera: DMat4 = player.get_camera_frame(&world);
                let [u_m, v_m, _] = world.get_fermi_coordinates(camera.col(AXIS_W));
                let at: String = format!("team {team}, player {id} at ({u_m:.1}, {v_m:.1})");
                let above_m: f64 = get_eye_above_ground_m(&world, &player);
                assert!(
                    (above_m - eye_m).abs() < STAND_TOLERANCE_M,
                    "{at}: eye {above_m} m up"
                );
                let nearby: Vec<usize> =
                    world.find_nearby_structures(player.body.col(AXIS_W), NEARBY_REACH_M);
                assert!(
                    !world.is_body_blocked(player.body, player.h_m, &nearby),
                    "{at}"
                );
                assert!(!world.is_outside_boundary(player.body.col(AXIS_W)), "{at}");
                let clear_m: f64 =
                    world.find_clear_m(camera.col(AXIS_W), -camera.col(AXIS_Z), CLEAR_AHEAD_M);
                assert!(clear_m >= CLEAR_AHEAD_M, "{at}: a wall {clear_m} m ahead");
                eyes.push(camera.col(AXIS_W));
            }
        }
    }

    #[test]
    fn walking_never_sticks_sinks_or_leaves() {
        let world: VoxelWorld = load_dust();
        let crouch_eye_m: f64 =
            world.player.eye_height_m * world.settings.movement.crouch_eye_height_frac;
        for team in 0..2 {
            for heading in 0..HEADING_COUNT {
                let spawn: VoxelPlayer = VoxelPlayer::AtSpawn(&world, team);
                let [u_m, v_m] = get_place_m(&world, &spawn);
                let angle_rad: f64 = std::f64::consts::TAU * heading as f64 / HEADING_COUNT as f64;
                let target_m: [f64; 2] = [u_m + angle_rad.cos(), v_m + angle_rad.sin()];
                let mut player: VoxelPlayer =
                    VoxelPlayer::AtView(&world, [u_m, v_m], target_m, None, 0.0);
                for i in 0..WALK_STEP_COUNT {
                    let phase: usize = i / GAIT_PERIOD_STEP_COUNT % 4;
                    let input: Input = Input {
                        forward: true,
                        sprint: phase == 1,
                        up: phase == 2 && i % GAIT_PERIOD_STEP_COUNT == 0,
                        crouch: phase == 3,
                        ..Input::default()
                    };
                    let messages: Vec<String> = player.step(&world, &input, STEP_S, team).messages;
                    let at: String =
                        format!("team {team}, heading {heading}, step {i}, {messages:?}");
                    let nearby: Vec<usize> =
                        world.find_nearby_structures(player.body.col(AXIS_W), NEARBY_REACH_M);
                    assert!(
                        !messages.is_empty()
                            || !world.is_body_blocked(player.body, player.h_m, &nearby),
                        "{at}: blocked with the eye {} m up",
                        player.h_m
                    );
                    let above_m: f64 = get_eye_above_ground_m(&world, &player);
                    assert!(
                        above_m > crouch_eye_m - STAND_TOLERANCE_M,
                        "{at}: {above_m}"
                    );
                    assert!(!world.is_outside_boundary(player.body.col(AXIS_W)), "{at}");
                    if i + 1 == GAIT_PERIOD_STEP_COUNT {
                        let [u_now_m, v_now_m] = get_place_m(&world, &player);
                        let moved_m: f64 = (u_now_m - u_m).hypot(v_now_m - v_m);
                        assert!(moved_m > MIN_FIRST_SECOND_M, "{at}: stuck, {moved_m} m");
                    }
                }
            }
        }
    }

    #[test]
    fn walkers_cross_the_ridges() {
        let world: VoxelWorld = load_dust();
        for side in [1.0, -1.0] {
            for u_m in RIDGE_CROSSING_U_M {
                let start_m: [f64; 2] = [side * u_m, side * RIDGE_FROM_V_M];
                let target_m: [f64; 2] = [side * u_m, side * RIDGE_TO_V_M];
                let mut player: VoxelPlayer =
                    VoxelPlayer::AtView(&world, start_m, target_m, None, 0.0);
                let start_h_m: f64 = player.h_m;
                let mut top_h_m: f64 = start_h_m;
                for i in 0..RIDGE_WALK_STEP_COUNT {
                    walk(&world, &mut player, 1);
                    top_h_m = top_h_m.max(player.h_m);
                    let nearby: Vec<usize> =
                        world.find_nearby_structures(player.body.col(AXIS_W), NEARBY_REACH_M);
                    assert!(
                        !world.is_body_blocked(player.body, player.h_m, &nearby),
                        "from {start_m:?}, step {i}: blocked"
                    );
                    if side * get_place_m(&world, &player)[1] >= RIDGE_TO_V_M {
                        break;
                    }
                }
                let [u_end_m, v_end_m] = get_place_m(&world, &player);
                assert!(
                    side * v_end_m >= RIDGE_TO_V_M,
                    "from {start_m:?}, stopped at ({u_end_m}, {v_end_m})"
                );
                assert!(
                    top_h_m - start_h_m > MIN_RIDGE_CLIMB_M,
                    "from {start_m:?}, climbed only {} m",
                    top_h_m - start_h_m
                );
            }
        }
    }

    fn find_mouth_m(world: &VoxelWorld, name: &str) -> [f64; 2] {
        let mouth: &VoxelWarp = world
            .warps
            .iter()
            .find(|warp| warp.name == name)
            .expect("the level's mouth");
        let [u_m, v_m, _] = world.get_fermi_coordinates(mouth.centre);
        [u_m, v_m]
    }

    #[test]
    fn the_level_wormhole_carries_walkers_and_lasers() {
        let world: VoxelWorld = load_dust();
        let mouths_m: [[f64; 2]; 2] = [
            find_mouth_m(&world, "wormhole mouth A"),
            find_mouth_m(&world, "wormhole mouth B"),
        ];
        for (from, [into_m, out_m]) in [mouths_m, [mouths_m[1], mouths_m[0]]]
            .into_iter()
            .enumerate()
        {
            let name: String = format!("wormhole mouth {}", ["A", "B"][from]);
            for angle_index in 0..HEADING_COUNT {
                let angle_rad: f64 =
                    std::f64::consts::TAU * angle_index as f64 / HEADING_COUNT as f64;
                let start_m: [f64; 2] = [
                    into_m[0] + WALK_IN_M * angle_rad.cos(),
                    into_m[1] + WALK_IN_M * angle_rad.sin(),
                ];
                let mut player: VoxelPlayer =
                    VoxelPlayer::AtView(&world, start_m, into_m, None, 0.0);
                let shot: Shot = world.trace_laser(player.get_camera_frame(&world));
                assert_eq!(shot.through.first(), Some(&name), "shot from {start_m:?}");
                let messages: Vec<String> = walk(&world, &mut player, WALK_THROUGH_STEP_COUNT);
                assert_eq!(
                    messages,
                    vec![format!("Through {name}")],
                    "from {start_m:?}"
                );
                let [u_m, v_m] = get_place_m(&world, &player);
                assert!(
                    (u_m - out_m[0]).hypot(v_m - out_m[1]) < NEAR_EXIT_M,
                    "from {start_m:?}, came out at ({u_m}, {v_m})"
                );
                let exit_m: [f64; 2] = [u_m, v_m];
                walk(&world, &mut player, WALK_THROUGH_STEP_COUNT);
                let [u_end_m, v_end_m] = get_place_m(&world, &player);
                assert!(
                    (u_end_m - exit_m[0]).hypot(v_end_m - exit_m[1]) > MIN_WALK_ON_M,
                    "from {start_m:?}, stuck at ({u_end_m}, {v_end_m}) after the throat"
                );
            }
        }
    }

    #[test]
    fn a_thrown_pair_carries_walkers_on_dust() {
        let mut world: VoxelWorld = load_dust();
        let terrain: &Terrain = world.terrain.as_ref().expect("Dust's terrain");
        let eye_m: f64 = world.player.eye_height_m;
        let place = |at_m: [f64; 2]| MouthPlace {
            u_m: at_m[0],
            v_m: at_m[1],
            h_m: terrain.get_height_m(at_m[0], at_m[1]) + eye_m,
            yaw_rad: 0.0,
        };
        let (purple, green) = (place(PURPLE_M), place(GREEN_M));
        assert!(world.can_place_mouth(&purple, &[]));
        assert!(world.can_place_mouth(&green, &[purple]));
        world.set_thrown_wormholes(&[[purple, green]]);
        let start_m: [f64; 2] = [PURPLE_M[0] - WALK_IN_M, PURPLE_M[1]];
        let mut player: VoxelPlayer = VoxelPlayer::AtView(&world, start_m, PURPLE_M, None, 0.0);
        let shot: Shot = world.trace_laser(player.get_camera_frame(&world));
        assert_eq!(
            shot.through.first(),
            Some(&"the purple wormhole".to_string())
        );
        let messages: Vec<String> = walk(&world, &mut player, WALK_THROUGH_STEP_COUNT);
        assert!(
            messages.contains(&"Through the purple wormhole".to_string()),
            "{messages:?}"
        );
        let [u_m, v_m] = get_place_m(&world, &player);
        assert!(
            (u_m - GREEN_M[0]).hypot(v_m - GREEN_M[1]) < NEAR_EXIT_M,
            "came out at ({u_m}, {v_m})"
        );
    }
}
