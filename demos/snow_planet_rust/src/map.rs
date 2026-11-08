//! `map.rs` reads a map in the portable format of `common/maps/` (see its README), `map.json`, into
//! typed values, failing at once with the field's name when a map breaks the format.
//!
//! Interface: `Map::Builtin`, `Map::Load`, `Map::scale_curvature`, `MapName`, `get_console_team`,
//! `get_map_folder`, and the types of the manifest's parts.
//!
//! Example: `let map = Map::Builtin("lunar_surface")?;`
//!
//! Must know:
//! - Every field of the format is read, so any valid map parses (`deny_unknown_fields`), even the
//!   ones this demo doesn't use; hence `dead_code` is allowed here.
//! - The game plays the maps built into the program (`Map::Builtin`), so it runs without the
//!   repository; `Map::Load` reads a map from a folder, for tests and tools.

#![allow(dead_code)]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use glam::{DMat4, DQuat, DVec3, DVec4};
use serde::Deserialize;

use crate::app_bundle::MODELS;
use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, invert_lorentz, make_boost, make_fermi_point,
};
use crate::net::TEAM_NAMES;
use crate::terrain::get_fermi_coordinates;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const MAPS_FOLDER: &str = "../../common/maps";
const MANIFEST_NAME: &str = "map.json";
const FORMAT: &str = "warped-map";
const VERSION: u32 = 1;
const SPACE_HYPERBOLIC: &str = "hyperbolic";
const SPACE_EUCLIDEAN: &str = "euclidean";
/// `FLAT_CURVATURE_RADIUS_M` is the curvature radius a Euclidean map is drawn at (`Map::curve_flat`):
/// so large that across the map lengths change by under 2 %, so small that the renderer's single
/// precision still separates centimetres.
pub const FLAT_CURVATURE_RADIUS_M: f64 = 160.0;
const DIRECTION_TOLERANCE: f64 = 1e-6;
const BUILTIN_MAPS: [(&str, &str); 3] = [
    (
        "house_demo",
        include_str!("../../../common/maps/house_demo/map.json"),
    ),
    (
        "lunar_surface",
        include_str!("../../../common/maps/lunar_surface/map.json"),
    ),
    (
        "snow_planet",
        include_str!("../../../common/maps/snow_planet/map.json"),
    ),
];

/// `Space` is the map's space: hyperbolic, or Euclidean with the ground's two axes (Fermi u and v)
/// periodic when it has `torus_periods_m`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Space {
    pub kind: String,
    pub curvature_radius_m: Option<f64>,
    pub torus_periods_m: Option<[f64; 2]>,
}

/// `PlayerSettings` is how a player moves on the map.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct PlayerSettings {
    pub eye_height_m: f64,
    pub walk_m_per_s: f64,
    pub sprint_m_per_s: f64,
    pub jump_m_per_s: f64,
    pub fly_m_per_s: f64,
}

/// `Material` is one palette entry.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub id: u8,
    pub name: String,
    pub color: [f32; 3],
    pub kind: String,
}

/// `Tiling` is how the ground is tiled for texture.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tiling {
    pub kind: String,
    pub cells_per_side: u32,
}

/// `MapTerrain` is the ground's relief, a `crater` in the world's Fermi coordinates.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapTerrain {
    pub kind: String,
    pub rim_u_m: f64,
    pub rim_v_m: f64,
    pub wall_m: f64,
    pub rim_height_m: f64,
    pub relief_m: f64,
    pub relief_wavelength_m: f64,
    pub seed: u32,
    #[serde(default)]
    pub mounds: Vec<MapMound>,
}

/// `MapMound` is a round hill on the terrain: centre (u, v), radius and height in meters.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapMound {
    pub u_m: f64,
    pub v_m: f64,
    pub radius_m: f64,
    pub height_m: f64,
}

/// `Ground` is the ground's material, tiling and relief.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ground {
    pub material: u8,
    pub tiling: Option<Tiling>,
    pub terrain: Option<MapTerrain>,
}

/// `MapShape` is one smooth solid of a structure, in its local chart (meters): a kind (`box`,
/// `cylinder` or `ellipsoid`), centre, half sizes, edge rounding, a cylinder's axis and a material.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapShape {
    pub kind: String,
    pub centre_m: [f64; 3],
    pub half_m: [f64; 3],
    pub round_m: Option<f64>,
    pub axis: Option<u8>,
    pub material: u8,
}

/// `MapMesh` is the glTF model a structure is drawn as: its folder in `common/maps/models/` and
/// its height in meters.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapMesh {
    pub model: String,
    pub height_m: f64,
}

/// `Structure` is a group of smooth shapes sharing a frame, drawn as its `mesh` when it has one.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub name: String,
    pub frame: [f64; 16],
    pub mesh: Option<MapMesh>,
    pub shapes: Vec<MapShape>,
}

/// `MapBox` is a box: a frame, half sizes and a material.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapBox {
    pub name: String,
    pub group: String,
    pub frame: [f64; 16],
    pub half_size_m: [f64; 3],
    pub material: u8,
}

/// `Warp` is a black hole or a wormhole mouth.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Warp {
    pub kind: String,
    pub name: String,
    pub frame: [f64; 16],
    pub law: String,
    pub schwarzschild_radius_m: f64,
    pub lambda_squared: Option<f64>,
    pub radial_coordinate: String,
    pub reach_m: f64,
    pub falloff: String,
    pub partner: Option<usize>,
}

/// `Light` is a lamp.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub position: [f64; 4],
    pub color: [f32; 3],
    pub radius_m: f64,
}

/// `Sun` is the sun's direction (at the world origin) and colour.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sun {
    pub direction: [f64; 3],
    pub color: [f32; 3],
}

/// `Planet` is another body in the sky.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Planet {
    pub direction: [f64; 3],
    pub angular_radius_rad: f64,
}

/// `MilkyWay` is where the galaxy's band lies in the sky: the great circle 90° from
/// `pole_direction`, brightest at `core_direction` (a unit direction perpendicular to it).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MilkyWay {
    pub pole_direction: [f64; 3],
    pub core_direction: [f64; 3],
}

/// `Sky` is how the sky looks and how a ray finds its place in it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sky {
    pub transport: String,
    pub sun: Sun,
    pub planet: Option<Planet>,
    pub milky_way: Option<MilkyWay>,
    pub stars: bool,
    pub clouds: bool,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub fog_m: Option<f64>,
    pub snow_per_m3: Option<f64>,
}

/// `Spawn` is where a player of a team starts.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    pub team: String,
    pub frame: [f64; 16],
}

/// `Zone` is a team's band of the world's Fermi coordinate u.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub team: String,
    pub u_min_m: Option<f64>,
    pub u_max_m: Option<f64>,
}

/// `Boundary` is the playable area's edge.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub kind: String,
    pub u_m: f64,
    pub v_m: f64,
}

/// `ZoneWalls` is how the walls along the team zones' edges look: their height above the ground,
/// their thickness and their opacity at the ground.
#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct ZoneWalls {
    pub height_m: f64,
    pub thickness_m: f64,
    pub opacity_frac: f64,
}

/// `Blizzard` is a snowstorm: an upright cylinder at Fermi (u, v) on the ground. Inside it a player
/// moves at `speed_frac` of their speed and loses `freeze_health_per_s` of their health a second, a
/// shot goes at most `shot_reach_m` inside blizzards, and the air holds `fog_per_m` of fog (optical
/// depth per meter) and `snow_per_m3` flakes per cubic meter.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Blizzard {
    pub name: String,
    pub u_m: f64,
    pub v_m: f64,
    pub radius_m: f64,
    pub height_m: f64,
    pub speed_frac: f64,
    pub freeze_health_per_s: f64,
    pub shot_reach_m: f64,
    pub fog_per_m: f64,
    pub snow_per_m3: f64,
}

impl Blizzard {
    /// `contains` says whether the point at Fermi coordinates `[u_m, v_m, h_m]` is inside this
    /// blizzard.
    ///
    /// Example: `if blizzard.contains([0.0, 0.0, 1.6]) { ... }`
    pub fn contains(&self, [u_m, v_m, h_m]: [f64; 3]) -> bool {
        (u_m - self.u_m).hypot(v_m - self.v_m) < self.radius_m && h_m < self.height_m
    }
}

/// `Regions` is the map's zones, boundary, zone walls and blizzards.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regions {
    pub zones: Option<Vec<Zone>>,
    pub boundary: Option<Boundary>,
    pub zone_walls: Option<ZoneWalls>,
    #[serde(default)]
    pub blizzards: Vec<Blizzard>,
}

/// `Lighting` is the lighting model and its numbers (those of the model; the others are absent).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    pub model: String,
    pub ambient: f64,
    pub sun: Option<f64>,
    pub lommel_seeliger_frac: Option<f64>,
    pub earthshine: Option<f64>,
    pub sun_intensity: Option<f64>,
    pub shadows: Option<bool>,
}

/// `Appearance` is the renderer's style for surface detail, and the lighting.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Appearance {
    pub style: String,
    pub lighting: Lighting,
    pub laser_color: Option<[f32; 3]>,
}

/// `Map` is a whole map: the manifest's parts and the voxel cells.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub source: String,
    pub space: Space,
    pub gravity_m_per_s2: f64,
    pub player: PlayerSettings,
    pub materials: Vec<Material>,
    pub ground: Ground,
    #[serde(default)]
    pub structures: Vec<Structure>,
    #[serde(default)]
    pub boxes: Vec<MapBox>,
    #[serde(default)]
    pub warps: Vec<Warp>,
    #[serde(default)]
    pub lights: Vec<Light>,
    pub sky: Sky,
    pub spawns: Vec<Spawn>,
    pub regions: Option<Regions>,
    pub appearance: Appearance,
}

/// `MapName` is which map to play, from `common/maps/`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum MapName {
    HouseDemo,
    LunarSurface,
    SnowPlanet,
}

impl MapName {
    /// `get_folder_name` gives the map's folder name in `common/maps/`.
    ///
    /// Example: `let name = MapName::HouseDemo.get_folder_name();`
    pub fn get_folder_name(self) -> &'static str {
        match self {
            MapName::HouseDemo => "house_demo",
            MapName::LunarSurface => "lunar_surface",
            MapName::SnowPlanet => "snow_planet",
        }
    }
}

/// `get_console_team` gives the team (an index into `TEAM_NAMES`) of a console's material, one
/// named "<team> console <n>" such as "red console 2", or `None` for any other material.
///
/// Example: `let teams: Vec<u32> = map.materials.iter().filter_map(get_console_team).collect();`
pub fn get_console_team(material: &Material) -> Option<u32> {
    TEAM_NAMES
        .iter()
        .position(|name| material.name.starts_with(&format!("{name} console ")))
        .map(|team| team as u32)
}

/// `get_map_folder` gives a shipped map's folder, `common/maps/<name>/`, found from the crate's folder.
///
/// Example: `let folder = get_map_folder("house_demo");`
pub fn get_map_folder(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MAPS_FOLDER)
        .join(name)
}

/// `get_model_path` gives the `scene.gltf` of the model folder `model` in `common/maps/models/`, or in
/// the app's copy of it when the game runs from `Warped.app` (`app_bundle.rs`).
///
/// Example: `let path: PathBuf = get_model_path("hesco_barrier");`
pub fn get_model_path(model: &str) -> PathBuf {
    MODELS.get_path().join(model).join("scene.gltf")
}

/// `to_frame` turns a frame's 16 numbers (column-major) into a matrix.
///
/// Example: `let frame = to_frame(&spawn.frame);`
pub fn to_frame(values: &[f64; 16]) -> DMat4 {
    DMat4::from_cols_array(values)
}

/// `to_point` turns 4 numbers into a 4-vector.
///
/// Example: `let x = to_point(&light.position);`
pub fn to_point(values: &[f64; 4]) -> DVec4 {
    DVec4::from_array(*values)
}

/// `make_fermi_frame` gives the frame at Fermi coordinates (u, v, h) in meters on a map of curvature
/// radius `r_m`, B_z(−u/R) · B_x(v/R) · B_y(h/R): its origin is `make_fermi_point`'s, and its axes are
/// the Fermi axes there.
///
/// Example: `let frame = make_fermi_frame([12.0, -3.0, 0.0], 40.0);`
fn make_fermi_frame(fermi_m: [f64; 3], r_m: f64) -> DMat4 {
    make_boost(AXIS_Z, -fermi_m[0] / r_m)
        * make_boost(AXIS_X, fermi_m[1] / r_m)
        * make_boost(AXIS_Y, fermi_m[2] / r_m)
}

/// `refit_frame` gives the frame on a map of curvature radius `to_r_m` at the same Fermi
/// coordinates in meters as `frame` on a map of curvature radius `from_r_m`, turned the same way
/// against the Fermi axes there. The turn is made an exact rotation first, so the map file's
/// rounding doesn't grow with the new frame's distance from the origin.
///
/// Example: `let frame = refit_frame(frame, 40.0, 20.0);`
fn refit_frame(frame: DMat4, from_r_m: f64, to_r_m: f64) -> DMat4 {
    let fermi_m: [f64; 3] = get_fermi_coordinates(frame.col(AXIS_W), from_r_m);
    let turn: DMat4 = invert_lorentz(make_fermi_frame(fermi_m, from_r_m)) * frame;
    make_fermi_frame(fermi_m, to_r_m) * DMat4::from_quat(DQuat::from_mat4(&turn).normalize())
}

/// `refit_frame_values` is `refit_frame` on a frame's 16 numbers (column-major).
///
/// Example: `spawn.frame = refit_frame_values(&spawn.frame, 40.0, 20.0);`
fn refit_frame_values(values: &[f64; 16], from_r_m: f64, to_r_m: f64) -> [f64; 16] {
    refit_frame(to_frame(values), from_r_m, to_r_m).to_cols_array()
}

/// `to_direction` turns 3 numbers into a 3-vector.
///
/// Example: `let d = to_direction(&map.sky.sun.direction);`
pub fn to_direction(values: &[f64; 3]) -> DVec3 {
    DVec3::from_array(*values)
}

/// `check_milky_way` says what is wrong with a Milky Way, if anything: its pole or core isn't a
/// unit direction, or the core isn't perpendicular to the pole (within `DIRECTION_TOLERANCE`, as
/// `common/maps/validate_map.py` checks).
///
/// Example: `check_milky_way(&milky_way)?;`
fn check_milky_way(milky_way: &MilkyWay) -> std::result::Result<(), String> {
    let pole: DVec3 = to_direction(&milky_way.pole_direction);
    let core: DVec3 = to_direction(&milky_way.core_direction);
    for (name, direction) in [("pole_direction", pole), ("core_direction", core)] {
        if (direction.length() - 1.0).abs() > DIRECTION_TOLERANCE {
            return Err(format!("{name} must be a unit direction, not {direction}"));
        }
    }
    if pole.dot(core).abs() > DIRECTION_TOLERANCE {
        return Err(format!(
            "core_direction must be perpendicular to pole_direction, not at a dot product of {}",
            pole.dot(core)
        ));
    }
    Ok(())
}

impl Map {
    /// `Builtin` gives the map called `name` (its folder's name in `common/maps/`) from the copy
    /// built into the program, checked as `Load` checks a map.
    ///
    /// Example: `let map = Map::Builtin("house_demo")?;`
    #[allow(non_snake_case)]
    pub fn Builtin(name: &str) -> Result<Map> {
        let Some(&(_, text)) = BUILTIN_MAPS.iter().find(|(builtin, _)| *builtin == name) else {
            let names: Vec<&str> = BUILTIN_MAPS.iter().map(|(builtin, _)| *builtin).collect();
            return Err(format!(
                "map: no built-in map {name:?}; built-in maps: {}",
                names.join(", ")
            )
            .into());
        };
        Map::Parse(text, &format!("the built-in {name}"))
    }

    /// `Load` reads the map in `folder`, checked as `Parse` checks it.
    ///
    /// Example: `let map = Map::Load(&get_map_folder("house_demo"))?;`
    #[allow(non_snake_case)]
    pub fn Load(folder: &Path) -> Result<Map> {
        let manifest_path: PathBuf = folder.join(MANIFEST_NAME);
        let text: String = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("map: can't read {}: {error}", manifest_path.display()))?;
        Map::Parse(&text, &manifest_path.display().to_string())
    }

    /// `Parse` reads a map from its `map.json` text, checking the format's name and version, that
    /// the space is hyperbolic and that a Milky Way's pole and core are perpendicular unit
    /// directions; `source` names the text in errors.
    ///
    /// Example: `let map = Map::Parse(&text, "house_demo/map.json")?;`
    #[allow(non_snake_case)]
    fn Parse(text: &str, source: &str) -> Result<Map> {
        let map: Map = serde_json::from_str(text)
            .map_err(|error| format!("map: {source} breaks the format: {error}"))?;
        if map.format != FORMAT || map.version != VERSION {
            return Err(format!(
                "map: {source} is {:?} version {}, not {FORMAT:?} version {VERSION}",
                map.format, map.version
            )
            .into());
        }
        let is_hyperbolic: bool =
            map.space.kind == SPACE_HYPERBOLIC && map.space.curvature_radius_m.is_some();
        let is_euclidean: bool =
            map.space.kind == SPACE_EUCLIDEAN && map.space.curvature_radius_m.is_none();
        if !is_hyperbolic && !is_euclidean {
            return Err(format!(
                "map: {} is in {} space; this demo draws hyperbolic and Euclidean maps only",
                map.name, map.space.kind
            )
            .into());
        }
        if let Some(milky_way) = &map.sky.milky_way {
            check_milky_way(milky_way)
                .map_err(|problem| format!("map: {}: sky.milky_way {problem}", map.name))?;
        }
        Ok(map)
    }

    /// `is_euclidean` says whether the map is in Euclidean space (until `curve_flat` draws it in
    /// nearly flat hyperbolic space).
    ///
    /// Example: `if map.is_euclidean() { map.curve_flat(FLAT_CURVATURE_RADIUS_M); }`
    pub fn is_euclidean(&self) -> bool {
        self.space.kind == SPACE_EUCLIDEAN
    }

    /// `curve_flat` turns a Euclidean map into a hyperbolic one of curvature radius `r_m` that the
    /// renderer draws: each frame and lamp goes to the Fermi coordinates (u, v, h) = (−z, x, y) of
    /// its Euclidean place, turned the same way against the Fermi axes there. Sizes in meters
    /// (shapes, warps' reach, regions) stay. The map keeps `torus_periods_m`.
    ///
    /// Example: `map.curve_flat(FLAT_CURVATURE_RADIUS_M);`
    pub fn curve_flat(&mut self, r_m: f64) {
        let curve = |values: &[f64; 16]| -> [f64; 16] {
            let frame: DMat4 = to_frame(values);
            let t: DVec4 = frame.col(AXIS_W);
            let mut turn: DMat4 = frame;
            turn.w_axis = DVec4::W;
            let rotation: DQuat = DQuat::from_mat4(&turn).normalize();
            (make_fermi_frame([-t.z, t.x, t.y], r_m) * DMat4::from_quat(rotation)).to_cols_array()
        };
        for structure in &mut self.structures {
            structure.frame = curve(&structure.frame);
        }
        for map_box in &mut self.boxes {
            map_box.frame = curve(&map_box.frame);
        }
        for warp in &mut self.warps {
            warp.frame = curve(&warp.frame);
        }
        for spawn in &mut self.spawns {
            spawn.frame = curve(&spawn.frame);
        }
        for light in &mut self.lights {
            let [x, y, z, _] = light.position;
            light.position = make_fermi_point(-z, x, y, r_m).to_array();
        }
        self.space.kind = SPACE_HYPERBOLIC.to_string();
        self.space.curvature_radius_m = Some(r_m);
    }

    /// `get_blizzards` gives the map's blizzards (none without regions).
    ///
    /// Example: `let blizzards = map.get_blizzards();`
    pub fn get_blizzards(&self) -> Vec<Blizzard> {
        self.regions
            .as_ref()
            .map_or(Vec::new(), |regions| regions.blizzards.clone())
    }

    /// `get_curvature_radius_m` gives the space's curvature radius (`Load` checked it is there).
    ///
    /// Example: `let r = map.get_curvature_radius_m();`
    pub fn get_curvature_radius_m(&self) -> f64 {
        self.space
            .curvature_radius_m
            .expect("Map::Load checked that the space has a curvature radius")
    }

    /// `scale_curvature` makes space `curvature_per_map_curvature` times as curved: it divides the
    /// curvature radius by it and moves every structure, box, warp, lamp and spawn to the same Fermi
    /// coordinates in meters, turned the same way against the Fermi axes there. Sizes in meters
    /// (shapes in their charts, boxes, warps' reach, the terrain, regions) stay, so ground distances
    /// along u and across it stay; distances along u away from v = 0 grow (by cosh(v/R)). It fails if
    /// a structure's shapes then reach the edge of its chart (|q| = R).
    ///
    /// Example: `map.scale_curvature(settings.space.curvature_per_map_curvature)?;`
    pub fn scale_curvature(&mut self, curvature_per_map_curvature: f64) -> Result<()> {
        let from_r_m: f64 = self.get_curvature_radius_m();
        let to_r_m: f64 = from_r_m / curvature_per_map_curvature;
        for structure in &self.structures {
            let reach_m: f64 = structure
                .shapes
                .iter()
                .map(|shape| {
                    (to_direction(&shape.centre_m).abs() + to_direction(&shape.half_m)).length()
                })
                .fold(0.0, f64::max);
            if reach_m >= to_r_m {
                return Err(format!(
                    "map {}: structure {:?} reaches {reach_m:.1} m from its frame's origin, past the edge of its chart at a curvature radius of {to_r_m} m; use a curvature_per_map_curvature below {:.2}",
                    self.name,
                    structure.name,
                    from_r_m / reach_m
                )
                .into());
            }
        }
        for structure in &mut self.structures {
            structure.frame = refit_frame_values(&structure.frame, from_r_m, to_r_m);
        }
        for map_box in &mut self.boxes {
            map_box.frame = refit_frame_values(&map_box.frame, from_r_m, to_r_m);
        }
        for warp in &mut self.warps {
            warp.frame = refit_frame_values(&warp.frame, from_r_m, to_r_m);
        }
        for spawn in &mut self.spawns {
            spawn.frame = refit_frame_values(&spawn.frame, from_r_m, to_r_m);
        }
        for light in &mut self.lights {
            let [u_m, v_m, h_m] = get_fermi_coordinates(to_point(&light.position), from_r_m);
            light.position = make_fermi_point(u_m, v_m, h_m, to_r_m).to_array();
        }
        self.space.curvature_radius_m = Some(to_r_m);
        Ok(())
    }

    /// `get_material` gives the palette entry with id `id`.
    ///
    /// Example: `let material = map.get_material(box.material)?;`
    pub fn get_material(&self, id: u8) -> Result<&Material> {
        self.materials
            .iter()
            .find(|material| material.id == id)
            .ok_or_else(|| {
                format!(
                    "map: {} uses material {id}, which isn't in its palette",
                    self.name
                )
                .into()
            })
    }
}

#[cfg(test)]
mod tests {
    //! Tests of curving a map's space more (`Map::scale_curvature`) and of checking a map's Milky
    //! Way (`check_milky_way`).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Built-in map asked for | one built in (`house_demo`, `lunar_surface`), one not built in | `builtin_maps_match_their_files`, `unknown_names_fail_listing_the_builtins` |
    //! | Milky Way in the map | none (the house), valid (Lunar Surface) | `shipped_milky_ways_load` |
    //! | Milky Way's directions | perpendicular unit directions, pole not unit, core not unit, not perpendicular | `milky_way_needs_perpendicular_unit_directions` |
    //! | Factor | 1 (nothing moves), between 1 and 2 (the house at 1.25), 2 (Lunar Surface: the radius halves) | `scaling_by_1_moves_nothing`, `scaling_keeps_fermi_coordinates_and_turns` |
    //! | What moves | structures and spawns (Lunar Surface), boxes and mouths (the house), black holes and lamps (Lunar Surface) | `scaling_keeps_fermi_coordinates_and_turns` |
    //! | Distance in meters | along the middle line v = 0 (stays), across it at the same u (stays), along u away from it (grows) | `distances_on_and_off_the_middle_line` |
    //! | Shapes against the new chart | inside it (Lunar Surface at 2), past its edge (at 3: fails, map unchanged) | `scaling_keeps_fermi_coordinates_and_turns`, `shapes_past_the_chart_fail` |

    use super::*;
    use crate::geometry::{get_distance, minkowski_dot};

    const FRAME_TOLERANCE: f64 = 1e-9;
    const ROUNDING_PER_W_SQUARED: f64 = 1e-12;
    const FERMI_TOLERANCE_M: f64 = 1e-6;
    const TWICE: f64 = 2.0;
    const HOUSE_FACTOR: f64 = 1.25;
    const THRICE: f64 = 3.0;
    const OFF_LINE_GROWTH_FRAC: f64 = 2.0;

    /// `load` loads a shipped map.
    fn load(name: &str) -> Map {
        Map::Load(&get_map_folder(name)).expect("a shipped map loads")
    }

    /// `get_frames` gives every frame of a map: structures, boxes, warps and spawns.
    fn get_frames(map: &Map) -> Vec<DMat4> {
        let structures = map.structures.iter().map(|s| &s.frame);
        let boxes = map.boxes.iter().map(|b| &b.frame);
        let warps = map.warps.iter().map(|w| &w.frame);
        let spawns = map.spawns.iter().map(|s| &s.frame);
        structures
            .chain(boxes)
            .chain(warps)
            .chain(spawns)
            .map(to_frame)
            .collect()
    }

    /// `get_turn` gives a frame's turn against the Fermi axes at its origin.
    fn get_turn(frame: DMat4, r_m: f64) -> DMat4 {
        let fermi_m: [f64; 3] = get_fermi_coordinates(frame.col(AXIS_W), r_m);
        invert_lorentz(make_fermi_frame(fermi_m, r_m)) * frame
    }

    /// `get_fermi_m` gives a point's Fermi coordinates as a vector.
    fn get_fermi_m(x: DVec4, r_m: f64) -> DVec3 {
        DVec3::from(get_fermi_coordinates(x, r_m))
    }

    /// `get_ground_distance_m` gives the distance between two ground points at Fermi (u, v).
    fn get_ground_distance_m(map: &Map, a_m: [f64; 2], b_m: [f64; 2]) -> f64 {
        let r_m: f64 = map.get_curvature_radius_m();
        let a: DVec4 = make_fermi_point(a_m[0], a_m[1], 0.0, r_m);
        get_distance(a, make_fermi_point(b_m[0], b_m[1], 0.0, r_m)) * r_m
    }

    #[test]
    fn shipped_milky_ways_load() {
        assert!(load("house_demo").sky.milky_way.is_none());
        let milky_way: MilkyWay = load("lunar_surface")
            .sky
            .milky_way
            .expect("Lunar Surface has a Milky Way");
        assert_eq!(check_milky_way(&milky_way), Ok(()));
    }

    #[test]
    fn milky_way_needs_perpendicular_unit_directions() {
        const UP: [f64; 3] = [0.0, 1.0, 0.0];
        const ALONG_X: [f64; 3] = [1.0, 0.0, 0.0];
        const LONG_X: [f64; 3] = [2.0, 0.0, 0.0];
        const SLANTED: [f64; 3] = [0.6, 0.8, 0.0];
        let check = |pole_direction: [f64; 3], core_direction: [f64; 3]| {
            check_milky_way(&MilkyWay {
                pole_direction,
                core_direction,
            })
        };
        assert_eq!(check(UP, ALONG_X), Ok(()));
        for (pole, core, problem) in [
            (LONG_X, UP, "pole_direction must be a unit direction"),
            (UP, LONG_X, "core_direction must be a unit direction"),
            (UP, SLANTED, "core_direction must be perpendicular"),
        ] {
            let error: String = check(pole, core).expect_err(problem);
            assert!(error.contains(problem), "{error}");
        }
    }

    #[test]
    fn scaling_by_1_moves_nothing() {
        let mut map: Map = load("lunar_surface");
        let before: Vec<DMat4> = get_frames(&map);
        map.scale_curvature(1.0).expect("the map's own curvature");
        let own_r_m: f64 = load("lunar_surface").get_curvature_radius_m();
        assert_eq!(map.get_curvature_radius_m(), own_r_m);
        for (old, new) in before.iter().zip(get_frames(&map)) {
            assert!(old.abs_diff_eq(new, FRAME_TOLERANCE), "{old} became {new}");
        }
    }

    #[test]
    fn scaling_keeps_fermi_coordinates_and_turns() {
        for (name, factor) in [("lunar_surface", TWICE), ("house_demo", HOUSE_FACTOR)] {
            let original: Map = load(name);
            let mut map: Map = load(name);
            map.scale_curvature(factor).expect("a stronger curvature");
            let from_r_m: f64 = original.get_curvature_radius_m();
            let to_r_m: f64 = map.get_curvature_radius_m();
            assert_eq!(to_r_m, from_r_m / factor);
            for (old, new) in get_frames(&original).into_iter().zip(get_frames(&map)) {
                let old_fermi_m: DVec3 = get_fermi_m(old.col(AXIS_W), from_r_m);
                let new_fermi_m: DVec3 = get_fermi_m(new.col(AXIS_W), to_r_m);
                assert!(
                    old_fermi_m.abs_diff_eq(new_fermi_m, FERMI_TOLERANCE_M),
                    "{name}"
                );
                let tolerance: f64 =
                    FRAME_TOLERANCE.max(ROUNDING_PER_W_SQUARED * new.w_axis.w.powi(2));
                let old_turn: DMat4 = get_turn(old, from_r_m);
                let new_turn: DMat4 = get_turn(new, to_r_m);
                assert!(old_turn.abs_diff_eq(new_turn, tolerance), "{name}");
                for (i, j) in [(0, 0), (1, 2), (3, 3), (0, 3)] {
                    let expected: f64 = minkowski_dot(old.col(i), old.col(j));
                    let product: f64 = minkowski_dot(new.col(i), new.col(j));
                    assert!((product - expected).abs() < tolerance, "{name}");
                }
            }
            for (old, new) in original.lights.iter().zip(&map.lights) {
                let old_fermi_m: DVec3 = get_fermi_m(to_point(&old.position), from_r_m);
                let new_fermi_m: DVec3 = get_fermi_m(to_point(&new.position), to_r_m);
                assert!(
                    old_fermi_m.abs_diff_eq(new_fermi_m, FERMI_TOLERANCE_M),
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn distances_on_and_off_the_middle_line() {
        let original: Map = load("lunar_surface");
        let mut map: Map = load("lunar_surface");
        map.scale_curvature(TWICE).expect("twice the curvature");
        let (blue_m, red_m) = ([-107.0, 0.0], [120.0, 0.0]);
        let middle_line_m: f64 = get_ground_distance_m(&map, blue_m, red_m);
        let original_middle_line_m: f64 = get_ground_distance_m(&original, blue_m, red_m);
        assert!((middle_line_m - original_middle_line_m).abs() < FERMI_TOLERANCE_M);
        let (near_m, far_m) = ([-30.0, -30.0], [-30.0, -40.0]);
        let across_m: f64 = get_ground_distance_m(&map, near_m, far_m);
        let original_across_m: f64 = get_ground_distance_m(&original, near_m, far_m);
        assert!((across_m - original_across_m).abs() < FERMI_TOLERANCE_M);
        let (west_m, east_m) = ([-22.0, 40.0], [-14.0, 40.0]);
        let along_m: f64 = get_ground_distance_m(&map, west_m, east_m);
        let original_along_m: f64 = get_ground_distance_m(&original, west_m, east_m);
        assert!(along_m > OFF_LINE_GROWTH_FRAC * original_along_m);
    }

    #[test]
    fn shapes_past_the_chart_fail() {
        let mut map: Map = load("lunar_surface");
        let before: Vec<DMat4> = get_frames(&map);
        let error: String = map
            .scale_curvature(THRICE)
            .expect_err("the relay mast reaches past a 13.3 m chart")
            .to_string();
        assert!(
            error.contains("relay mast") && error.contains("chart"),
            "{error}"
        );
        let own_r_m: f64 = load("lunar_surface").get_curvature_radius_m();
        assert_eq!(map.get_curvature_radius_m(), own_r_m);
        assert_eq!(get_frames(&map), before);
    }

    #[test]
    fn builtin_maps_match_their_files() {
        for (name, _) in BUILTIN_MAPS {
            let builtin: Map = Map::Builtin(name).expect("the built-in map parses");
            let loaded: Map = Map::Load(&get_map_folder(name)).expect("the map's file parses");
            assert_eq!(builtin.name, loaded.name);
            assert_eq!(builtin.materials.len(), loaded.materials.len());
            assert_eq!(builtin.spawns.len(), loaded.spawns.len());
        }
    }

    #[test]
    fn unknown_names_fail_listing_the_builtins() {
        let error: String = Map::Builtin("moon_base")
            .err()
            .expect("no such map")
            .to_string();
        assert!(error.contains("moon_base"), "{error}");
        assert!(error.contains("house_demo, lunar_surface"), "{error}");
    }
}
