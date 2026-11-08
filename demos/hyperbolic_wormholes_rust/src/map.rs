//! `map.rs` reads a map in the portable format of `common/maps/` (see its README), `map.json`, into
//! typed values, failing at once with the field's name when a map breaks the format.
//!
//! Interface: `Map::Load`, `get_map_folder`, and the types of the manifest's parts.
//!
//! Example: `let map = Map::Load(&get_map_folder("lunar_surface"))?;`
//!
//! Must know:
//! - Every field of the format is read, so any valid map parses (`deny_unknown_fields`), even the
//!   ones this demo doesn't use; hence `dead_code` is allowed here.

#![allow(dead_code)]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use glam::{DMat4, DVec3, DVec4};
use serde::Deserialize;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const MAPS_FOLDER: &str = "../../common/maps";
const MANIFEST_NAME: &str = "map.json";
const FORMAT: &str = "warped-map";
const VERSION: u32 = 1;
const SPACE_HYPERBOLIC: &str = "hyperbolic";

/// `Space` is the map's space; this demo reads hyperbolic maps only.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Space {
    pub kind: String,
    pub curvature_radius_m: Option<f64>,
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

/// `Structure` is a group of smooth shapes sharing a frame.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub name: String,
    pub frame: [f64; 16],
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

/// `Sky` is how the sky looks and how a ray finds its place in it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sky {
    pub transport: String,
    pub sun: Sun,
    pub planet: Option<Planet>,
    pub stars: bool,
    pub clouds: bool,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub fog_m: Option<f64>,
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

/// `Regions` is the map's zones, boundary and zone walls.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regions {
    pub zones: Option<Vec<Zone>>,
    pub boundary: Option<Boundary>,
    pub zone_walls: Option<ZoneWalls>,
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

/// `get_map_folder` gives a shipped map's folder, `common/maps/<name>/`, found from the crate's folder.
///
/// Example: `let folder = get_map_folder("house_demo");`
pub fn get_map_folder(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(MAPS_FOLDER)
        .join(name)
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

/// `to_direction` turns 3 numbers into a 3-vector.
///
/// Example: `let d = to_direction(&map.sky.sun.direction);`
pub fn to_direction(values: &[f64; 3]) -> DVec3 {
    DVec3::from_array(*values)
}

impl Map {
    /// `Load` reads the map in `folder`, checking the format's name and version and that the
    /// space is hyperbolic.
    ///
    /// Example: `let map = Map::Load(&get_map_folder("house_demo"))?;`
    #[allow(non_snake_case)]
    pub fn Load(folder: &Path) -> Result<Map> {
        let manifest_path: PathBuf = folder.join(MANIFEST_NAME);
        let text: String = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("map: can't read {}: {error}", manifest_path.display()))?;
        let map: Map = serde_json::from_str(&text).map_err(|error| {
            format!(
                "map: {} breaks the format: {error}",
                manifest_path.display()
            )
        })?;
        if map.format != FORMAT || map.version != VERSION {
            return Err(format!(
                "map: {} is {:?} version {}, not {FORMAT:?} version {VERSION}",
                manifest_path.display(),
                map.format,
                map.version
            )
            .into());
        }
        if map.space.kind != SPACE_HYPERBOLIC || map.space.curvature_radius_m.is_none() {
            return Err(format!(
                "map: {} is in {} space; this demo draws hyperbolic maps only",
                map.name, map.space.kind
            )
            .into());
        }
        Ok(map)
    }

    /// `get_curvature_radius_m` gives the space's curvature radius (`Load` checked it is there).
    ///
    /// Example: `let r = map.get_curvature_radius_m();`
    pub fn get_curvature_radius_m(&self) -> f64 {
        self.space
            .curvature_radius_m
            .expect("Map::Load checked that the space has a curvature radius")
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
