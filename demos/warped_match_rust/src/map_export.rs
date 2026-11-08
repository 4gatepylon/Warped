//! `map_export.rs` writes the demo's world as a map in the portable format of `common/maps/` (see
//! its README): the boxes with a material per colour, the wormhole mouths with the demo's lensing
//! law, the sky and the spawn point, so other programs can load the house demo.
//!
//! Interface: `export_map`.
//!
//! Example: `let path = map_export::export_map(&world, Path::new("../../common/maps/house_demo"))?;`

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use glam::DMat4;
use serde_json::{Value, json};

use crate::physics::{
    self, GRAVITY_M_PER_S2, JUMP_SPEED_M_PER_S, SPRINT_SPEED_M_PER_S, WALK_SPEED_M_PER_S,
};
use crate::scene_data::{FOG_PER_R, SUN_DIRECTION, get_tile_cell_count};
use crate::world::{
    Color, EYE_HEIGHT_M, FOIL_LAMBDA_SQUARED, MATERIAL_GLASS, MATERIAL_GLOW, NAMED_COLORS,
    THROAT_PER_HORIZON, World,
};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const MAP_NAME: &str = "house_demo";
const MANIFEST_NAME: &str = "map.json";
const GROUND_MATERIAL_ID: usize = 1;
const GROUND_COLOR: Color = [0.38, 0.62, 0.26];
const SUN_COLOR: Color = [1.0, 0.92, 0.75];
const ZENITH_COLOR: Color = [0.30, 0.52, 0.88];
const HORIZON_COLOR: Color = [0.78, 0.86, 0.95];
const AMBIENT_LIGHT: f64 = 0.68;
const SUN_LIGHT: f64 = 0.32;
const SCHWARZSCHILD_PER_THROAT: f64 = 1.0 / THROAT_PER_HORIZON;

/// `get_frame_values` gives a frame's 16 numbers in column-major order.
///
/// Example: `let values = get_frame_values(world.spawn_frame);`
fn get_frame_values(frame: DMat4) -> Vec<f64> {
    frame.to_cols_array().to_vec()
}

/// `get_material_kind` names a box material's kind in the map format.
///
/// Example: `let kind = get_material_kind(MATERIAL_GLASS);`
fn get_material_kind(material: u32) -> &'static str {
    match material {
        MATERIAL_GLASS => "glass",
        MATERIAL_GLOW => "light",
        _ => "solid",
    }
}

/// `export_map` writes `map.json` for the world into `folder` (made if missing) and returns its
/// path.
///
/// Example: `let path = map_export::export_map(&world, Path::new("house_demo"))?;`
pub fn export_map(world: &World, folder: &Path) -> Result<PathBuf> {
    let r: f64 = world.curvature_radius_m;
    let mut palette: Vec<(Color, &'static str)> = Vec::new();
    let mut boxes: Vec<Value> = Vec::new();
    for (index, hyperbolic_box) in world.boxes.iter().enumerate() {
        let key: (Color, &'static str) = (
            hyperbolic_box.color,
            get_material_kind(hyperbolic_box.material),
        );
        let position: usize = match palette.iter().position(|entry| *entry == key) {
            Some(position) => position,
            None => {
                palette.push(key);
                palette.len() - 1
            }
        };
        let frame: DMat4 = DMat4::from_cols(
            hyperbolic_box.normals[0],
            hyperbolic_box.normals[1],
            hyperbolic_box.normals[2],
            hyperbolic_box.center,
        );
        boxes.push(json!({
            "name": format!("box {index}"),
            "group": physics::describe_box(world, index),
            "frame": get_frame_values(frame),
            "half_size_m": hyperbolic_box.half_m,
            "material": GROUND_MATERIAL_ID + 1 + position,
        }));
    }
    let mut materials: Vec<Value> = vec![
        json!({"id": 0, "name": "air", "color": [0.0, 0.0, 0.0], "kind": "empty"}),
        json!({"id": GROUND_MATERIAL_ID, "name": "grass", "color": GROUND_COLOR, "kind": "solid"}),
    ];
    for (position, (color, kind)) in palette.iter().enumerate() {
        let name: String = NAMED_COLORS
            .iter()
            .find(|(_, named)| named == color)
            .map_or(format!("colour {position}"), |(name, _)| name.to_string());
        materials.push(json!({
            "id": GROUND_MATERIAL_ID + 1 + position,
            "name": format!("{name} ({kind})"),
            "color": color,
            "kind": kind,
        }));
    }
    let warps: Vec<Value> = world
        .mouths
        .iter()
        .map(|mouth| {
            json!({
                "kind": "wormhole_mouth",
                "name": mouth.name,
                "frame": get_frame_values(mouth.frame),
                "law": "foil",
                "schwarzschild_radius_m": SCHWARZSCHILD_PER_THROAT * world.get_throat_m(),
                "lambda_squared": FOIL_LAMBDA_SQUARED,
                "radial_coordinate": "flat_effective",
                "reach_m": world.bend_cutoff_m,
                "falloff": "cutoff",
                "partner": mouth.partner_index,
            })
        })
        .collect();
    let manifest: Value = json!({
        "format": "warped-map",
        "version": 1,
        "name": MAP_NAME,
        "description": "The Rust wormholes demo: a house, rails, two pylons and trees in H³, with three wormholes between them.",
        "source": "demos/hyperbolic_wormholes_rust/src/world.rs",
        "space": {"kind": "hyperbolic", "curvature_radius_m": r},
        "gravity_m_per_s2": GRAVITY_M_PER_S2,
        "player": {
            "eye_height_m": EYE_HEIGHT_M,
            "walk_m_per_s": WALK_SPEED_M_PER_S,
            "sprint_m_per_s": SPRINT_SPEED_M_PER_S,
            "jump_m_per_s": JUMP_SPEED_M_PER_S,
            "fly_m_per_s": WALK_SPEED_M_PER_S,
        },
        "materials": materials,
        "ground": {
            "material": GROUND_MATERIAL_ID,
            "tiling": {"kind": "square_order_5", "cells_per_side": get_tile_cell_count(r)},
        },
        "boxes": boxes,
        "warps": warps,
        "sky": {
            "transport": "ideal",
            "sun": {"direction": SUN_DIRECTION.normalize().to_array(), "color": SUN_COLOR},
            "stars": false,
            "clouds": true,
            "zenith_color": ZENITH_COLOR,
            "horizon_color": HORIZON_COLOR,
            "fog_m": FOG_PER_R * r,
        },
        "spawns": [{"team": "any", "frame": get_frame_values(world.spawn_frame)}],
        "appearance": {
            "style": "meadow",
            "lighting": {"model": "ambient_sun", "ambient": AMBIENT_LIGHT, "sun": SUN_LIGHT},
        },
    });
    fs::create_dir_all(folder)?;
    let path: PathBuf = folder.join(MANIFEST_NAME);
    fs::write(&path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    Ok(path)
}
