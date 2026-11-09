//! `map` reads and writes the portable map format of `common/maps/` (spec in its README) for a
//! Euclidean map: the subset this demo uses (axis-aligned boxes, cylinder and ellipsoid shapes,
//! ladders, lights, spawns) plus the optional `ladders` field this level added.
//!
//! Example:
//!     let map: MapFile = map::load(Path::new("../../common/maps/dark_forest/map.json"))?;
//!
//! Interface: `MapFile` and its parts, `load`, `translation_frame`, `yaw_frame`,
//! `frame_translation_m`, `frame_yaw_rad`.

use std::path::Path;

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// `MapFile` is one `map.json` manifest.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapFile {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub source: String,
    pub space: Space,
    pub gravity_m_per_s2: f32,
    pub player: PlayerSpec,
    pub materials: Vec<Material>,
    pub ground: Ground,
    #[serde(default)]
    pub structures: Vec<Structure>,
    #[serde(default)]
    pub boxes: Vec<MapBox>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ladders: Vec<Ladder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warps: Vec<Warp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub laser: Option<LaserLook>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scatter: Option<Scatter>,
    #[serde(default)]
    pub lights: Vec<Light>,
    pub sky: Sky,
    pub spawns: Vec<Spawn>,
    pub appearance: Appearance,
}

/// `Scatter` records how a generator scattered the map's decorative plants: its seed, the grid
/// step before jitter and the chance that a clear grid point grows a plant.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Scatter {
    pub seed: u32,
    pub spacing_m: f32,
    pub grow_frac: f32,
}

/// `Space` is the map's geometry; this demo reads only `euclidean`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Space {
    pub kind: String,
}

/// `PlayerSpec` is how players move on the map.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerSpec {
    pub eye_height_m: f32,
    pub walk_m_per_s: f32,
    pub sprint_m_per_s: f32,
    pub jump_m_per_s: f32,
    pub fly_m_per_s: f32,
}

/// `Material` is one palette entry.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Material {
    pub id: u8,
    pub name: String,
    pub color: [f32; 3],
    pub kind: String,
}

/// `Ground` is the ground plane's material.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ground {
    pub material: u8,
}

/// `Structure` is a solid made of smooth shapes in its frame.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Structure {
    pub name: String,
    pub frame: [f32; 16],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<StructureMesh>,
    pub shapes: Vec<Shape>,
}

/// `StructureMesh` draws a structure as the glTF model `models/<model>/scene.gltf`, scaled to
/// `height_m` tall, instead of its shapes.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StructureMesh {
    pub model: String,
    pub height_m: f32,
}

/// `Shape` is one box, cylinder or ellipsoid of a structure, in the structure's frame.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Shape {
    pub kind: String,
    pub centre_m: [f32; 3],
    pub half_m: [f32; 3],
    #[serde(default, skip_serializing_if = "is_zero")]
    pub round_m: f32,
    #[serde(default = "default_axis")]
    pub axis: u8,
    pub material: u8,
}

/// `is_zero` tells serde to leave out a zero rounding.
fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

/// `default_axis` is a cylinder's default axis, y.
fn default_axis() -> u8 {
    1
}

/// `MapBox` is one box: |q_a| ≤ `half_size_m[a]` in its frame.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapBox {
    pub name: String,
    pub group: String,
    pub frame: [f32; 16],
    pub half_size_m: [f32; 3],
    pub material: u8,
}

/// `Ladder` is a climbable volume (|q_a| ≤ `half_size_m[a]` in its frame) that stops neither
/// players nor lasers; a rope ladder hangs in its middle, its rungs along local x, and the
/// frame's −z faces the climber.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ladder {
    pub name: String,
    pub frame: [f32; 16],
    pub half_size_m: [f32; 3],
}

/// `Warp` is a black hole or a wormhole mouth (the format README's "Warps"): its centre is its
/// frame's origin, and a mouth's `partner` is the index of the other mouth of its pair.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Warp {
    pub kind: String,
    pub name: String,
    pub frame: [f32; 16],
    pub law: String,
    pub schwarzschild_radius_m: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda_squared: Option<f32>,
    pub radial_coordinate: String,
    pub reach_m: f32,
    pub falloff: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partner: Option<usize>,
}

/// `LaserLook` is the colour every team's lasers share on this map (absent: team colours).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaserLook {
    pub rgb: [f32; 3],
}

/// `Light` is one lamp.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Light {
    pub position: [f32; 4],
    pub color: [f32; 3],
    pub radius_m: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flicker_frac: Option<f32>,
}

/// `Sky` is the sky and the sun.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sky {
    pub transport: String,
    pub sun: Sun,
    pub stars: bool,
    pub clouds: bool,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fog_m: Option<f32>,
}

/// `Sun` is the sun's direction and colour.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sun {
    pub direction: [f64; 3],
    pub color: [f32; 3],
}

/// `Spawn` is where a player of a team starts.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Spawn {
    pub team: String,
    pub frame: [f32; 16],
}

/// `Appearance` names the renderer's style for the map.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Appearance {
    pub style: String,
    pub lighting: serde_json::Value,
}

/// `load` reads a Euclidean map file.
///
/// Example: `let map: MapFile = load(&path)?;`
pub fn load(path: &Path) -> Result<MapFile, String> {
    let text: String = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read map {}: {error}", path.display()))?;
    let map: MapFile = serde_json::from_str(&text)
        .map_err(|error| format!("map {} is not a valid map: {error}", path.display()))?;
    if map.space.kind != "euclidean" {
        return Err(format!(
            "map {} is {}, but this demo draws only euclidean maps",
            path.display(),
            map.space.kind
        ));
    }
    Ok(map)
}

/// `get_console_team` gives the team (0 blue, 1 red, as `net::TEAM_NAMES`) of a console made of
/// `material`, or `None` when the material isn't named `blue console` or `red console`.
///
/// Example: `let team: Option<u32> = get_console_team(&map, shape.material);`
pub fn get_console_team(map: &MapFile, material: u8) -> Option<u32> {
    match map
        .materials
        .iter()
        .find(|m| m.id == material)?
        .name
        .as_str()
    {
        "blue console" => Some(0),
        "red console" => Some(1),
        _ => None,
    }
}

/// `default_map_path` is `common/maps/dark_forest/map.json` in this repository.
///
/// Example: `let map: MapFile = load(&default_map_path())?;`
pub fn default_map_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../common/maps/dark_forest/map.json")
}

/// `translation_frame` is the frame at a point, axes along the world's.
///
/// Example: `let frame = translation_frame(Vec3::new(1.0, 0.0, 2.0));`
pub fn translation_frame(position_m: Vec3) -> [f32; 16] {
    Mat4::from_translation(position_m).to_cols_array()
}

/// `yaw_frame` is the frame at a point turned by a yaw about +y (yaw 0 faces −z).
///
/// Example: `let frame = yaw_frame(Vec3::ZERO, std::f32::consts::PI);`
pub fn yaw_frame(position_m: Vec3, yaw_rad: f32) -> [f32; 16] {
    Mat4::from_rotation_translation(Quat::from_rotation_y(yaw_rad), position_m).to_cols_array()
}

/// `frame_translation_m` is a frame's origin.
///
/// Example: `let origin_m: Vec3 = frame_translation_m(&spawn.frame);`
pub fn frame_translation_m(frame: &[f32; 16]) -> Vec3 {
    Vec3::new(frame[12], frame[13], frame[14])
}

/// `frame_yaw_rad` is the yaw of a frame's −z axis about +y (0 when it faces −z).
///
/// Example: `let yaw_rad: f32 = frame_yaw_rad(&spawn.frame);`
pub fn frame_yaw_rad(frame: &[f32; 16]) -> f32 {
    let forward: Vec3 = -Vec3::new(frame[8], frame[9], frame[10]);
    (-forward.x).atan2(-forward.z)
}

/// `frame_half_extent_m` is the half size of a box's world-axis bounding box, given its frame's
/// rotation and its local half size.
///
/// Example: `let half_m: Vec3 = frame_half_extent_m(&map_box.frame, Vec3::ONE);`
pub fn frame_half_extent_m(frame: &[f32; 16], half_m: Vec3) -> Vec3 {
    let matrix: Mat4 = Mat4::from_cols_array(frame);
    let axes: [Vec3; 3] = [
        matrix.x_axis.truncate(),
        matrix.y_axis.truncate(),
        matrix.z_axis.truncate(),
    ];
    axes[0].abs() * half_m.x + axes[1].abs() * half_m.y + axes[2].abs() * half_m.z
}

/// `tests` checks frame helpers.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | yaw | 0, a quarter turn, a half turn | `test_yaw_round_trips` |
/// | frame rotation | none, a quarter turn | `test_half_extent_follows_rotation` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_yaw_round_trips` checks `frame_yaw_rad` inverts `yaw_frame`.
    #[test]
    fn test_yaw_round_trips() {
        for yaw_rad in [0.0_f32, std::f32::consts::FRAC_PI_2, 3.0] {
            let frame = yaw_frame(Vec3::new(1.0, 2.0, 3.0), yaw_rad);
            assert!((frame_yaw_rad(&frame) - yaw_rad).abs() < 1e-5);
            assert_eq!(frame_translation_m(&frame), Vec3::new(1.0, 2.0, 3.0));
        }
    }

    /// `test_half_extent_follows_rotation` checks a quarter turn swaps x and z.
    #[test]
    fn test_half_extent_follows_rotation() {
        let half_m: Vec3 = Vec3::new(1.0, 2.0, 3.0);
        let straight = frame_half_extent_m(&translation_frame(Vec3::ZERO), half_m);
        assert!((straight - half_m).length() < 1e-5);
        let turned =
            frame_half_extent_m(&yaw_frame(Vec3::ZERO, std::f32::consts::FRAC_PI_2), half_m);
        assert!((turned - Vec3::new(3.0, 2.0, 1.0)).length() < 1e-5);
    }
}
