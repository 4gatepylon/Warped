//! `material_surfaces.rs` says which texture set, tiling and metalness each map material uses, as
//! the voxel shader reads them; `voxel_textures.rs` in the client loads the sets themselves.
//!
//! Interface: `get_material_surface`.
//!
//! Example: `let surface = get_material_surface("hull");`

/// `MATERIAL_SURFACES` is each textured material's name, layer, blocks per repeat and metalness.
const MATERIAL_SURFACES: [(&str, usize, f32, f32); 13] = [
    ("regolith", 0, 2.6, 0.0),
    ("dark regolith", 7, 2.6, 0.0),
    ("basalt", 1, 2.0, 0.0),
    ("boulder", 1, 2.0, 0.0),
    ("hull", 2, 2.0, 0.25),
    ("metal", 3, 2.0, 1.0),
    ("solar panel", 2, 1.0, 0.5),
    ("gold foil", 2, 1.0, 1.0),
    ("red paint", 4, 2.0, 0.3),
    ("blue paint", 4, 2.0, 0.3),
    ("hazard paint", 4, 2.0, 0.3),
    ("landing pad", 5, 4.0, 0.0),
    ("floor", 6, 4.0, 0.0),
];

/// `get_material_surface` gives the shader's surface of a material: its layer (-1 for none),
/// blocks per repeat and metalness.
///
/// Example: `let surface = get_material_surface("hull");`
pub fn get_material_surface(name: &str) -> [f32; 4] {
    MATERIAL_SURFACES
        .iter()
        .find(|s| s.0 == name)
        .map_or([-1.0, 1.0, 0.0, 0.0], |s| [s.1 as f32, s.2, s.3, 0.0])
}
