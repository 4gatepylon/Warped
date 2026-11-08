//! `mesh.rs` is a glTF model of `common/maps/models/` placed in its structure's chart q (meters,
//! where straight lines are geodesics, so its triangles are exact geodesic faces): its triangles,
//! a bounding volume hierarchy over them (so far only its root) and its materials.
//!
//! Interface: `MeshModel::Load`, `MeshModel`, `MeshTriangle`, `BvhNode`, `MeshMaterial`.
//!
//! Example: `let model = MeshModel::Load("hesco_barrier", 1.4)?;`

use std::path::{Path, PathBuf};

use glam::{Mat3, Mat4, Vec2, Vec3};

use crate::map::get_model_path;

/// `MeshTriangle` is one triangle: its corners, their normals and texture coordinates (u, v), and
/// its material.
#[derive(Clone, Copy)]
pub struct MeshTriangle {
    pub p: [Vec3; 3],
    pub n: [Vec3; 3],
    pub uv: [Vec2; 3],
    pub material: usize,
}

/// `BvhNode` is a box around its triangles: a leaf holds `count` triangles from `first`; an inner
/// node (count 0) has its two children at `first` and `first + 1`.
#[derive(Clone, Copy)]
pub struct BvhNode {
    pub lo: Vec3,
    pub hi: Vec3,
    pub first: usize,
    pub count: usize,
}

/// `MeshMaterial` is a glTF material's factors and its base colour, metallic-roughness, normal and
/// emissive images (paths, `None` where it has none).
#[derive(Clone)]
pub struct MeshMaterial {
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub images: [Option<PathBuf>; 4],
}

/// `MeshModel` is a placed model: triangles ordered so each BVH leaf's are contiguous, the BVH
/// (root first) and the materials.
pub struct MeshModel {
    pub triangles: Vec<MeshTriangle>,
    pub nodes: Vec<BvhNode>,
    pub materials: Vec<MeshMaterial>,
}

/// `get_globals` gives each node's model-space transform.
///
/// Example: `let globals = get_globals(&document);`
fn get_globals(document: &gltf::Document) -> Vec<Mat4> {
    fn visit(node: gltf::Node, parent: Mat4, globals: &mut [Mat4]) {
        let global: Mat4 = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        globals[node.index()] = global;
        node.children()
            .for_each(|child| visit(child, global, globals));
    }
    let mut globals: Vec<Mat4> = vec![Mat4::IDENTITY; document.nodes().len()];
    let scene = document
        .default_scene()
        .expect("a model has a default scene");
    scene
        .nodes()
        .for_each(|node| visit(node, Mat4::IDENTITY, &mut globals));
    globals
}

/// `read_material` reads one glTF material, its image paths relative to `folder`.
///
/// Example: `let material = read_material(&m, &image_paths);`
fn read_material(m: &gltf::Material, image_paths: &[PathBuf]) -> MeshMaterial {
    let pbr = m.pbr_metallic_roughness();
    let path =
        |texture: Option<gltf::Texture>| texture.map(|t| image_paths[t.source().index()].clone());
    let strength: f32 = m.emissive_strength().unwrap_or(1.0);
    MeshMaterial {
        base_color: pbr.base_color_factor(),
        emissive: m.emissive_factor().map(|e| e * strength),
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        images: [
            path(pbr.base_color_texture().map(|t| t.texture())),
            path(pbr.metallic_roughness_texture().map(|t| t.texture())),
            path(m.normal_texture().map(|t| t.texture())),
            path(m.emissive_texture().map(|t| t.texture())),
        ],
    }
}

impl MeshModel {
    /// `Load` reads model `model`, scales it evenly to `height_m` tall and puts the bottom centre
    /// of its bounding box at the origin, then builds its BVH.
    ///
    /// Example: `let model = MeshModel::Load("hesco_barrier", 1.4)?;`
    #[allow(non_snake_case)]
    pub fn Load(model: &str, height_m: f64) -> Result<MeshModel, String> {
        let path: PathBuf = get_model_path(model);
        let folder: &Path = path.parent().expect("a file has a folder");
        let (document, buffers, _) =
            gltf::import(&path).map_err(|e| format!("mesh: {}: {e}", path.display()))?;
        let globals: Vec<Mat4> = get_globals(&document);
        let mut triangles: Vec<MeshTriangle> = Vec::new();
        for node in document.nodes() {
            let Some(mesh) = node.mesh() else { continue };
            let global: Mat4 = globals[node.index()];
            let normal_turn: Mat3 = Mat3::from_mat4(global).inverse().transpose();
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                let positions: Vec<Vec3> = reader
                    .read_positions()
                    .ok_or(format!("mesh: {model} has a primitive without positions"))?
                    .map(|p| global.transform_point3(Vec3::from(p)))
                    .collect();
                let normals: Option<Vec<Vec3>> = reader.read_normals().map(|n| {
                    n.map(|n| (normal_turn * Vec3::from(n)).normalize_or_zero())
                        .collect()
                });
                let uvs: Option<Vec<Vec2>> = reader
                    .read_tex_coords(0)
                    .map(|t| t.into_f32().map(Vec2::from).collect());
                let indices: Vec<u32> = match reader.read_indices() {
                    Some(read) => read.into_u32().collect(),
                    None => (0..positions.len() as u32).collect(),
                };
                for corners in indices.as_chunks::<3>().0 {
                    let k: [usize; 3] = corners.map(|i| i as usize);
                    let p: [Vec3; 3] = k.map(|i| positions[i]);
                    let face: Vec3 = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                    triangles.push(MeshTriangle {
                        p,
                        n: k.map(|i| normals.as_ref().map_or(face, |n| n[i])),
                        uv: k.map(|i| uvs.as_ref().map_or(Vec2::ZERO, |t| t[i])),
                        material: primitive.material().index().unwrap_or(0),
                    });
                }
            }
        }
        let (lo, hi) = get_bounds(&triangles);
        let scale: f32 = height_m as f32 / (hi.y - lo.y);
        let offset: Vec3 = Vec3::new((lo.x + hi.x) / 2.0, lo.y, (lo.z + hi.z) / 2.0);
        for t in &mut triangles {
            t.p = t.p.map(|p| (p - offset) * scale);
        }
        let image_paths: Vec<PathBuf> = document
            .images()
            .map(|image| match image.source() {
                gltf::image::Source::Uri { uri, .. } => Ok(folder.join(uri)),
                gltf::image::Source::View { .. } => Err(format!("mesh: {model} embeds an image")),
            })
            .collect::<Result<_, String>>()?;
        let materials: Vec<MeshMaterial> = document
            .materials()
            .map(|m| read_material(&m, &image_paths))
            .collect();
        let (lo, hi) = get_bounds(&triangles);
        let nodes: Vec<BvhNode> = vec![BvhNode {
            lo,
            hi,
            first: 0,
            count: triangles.len(),
        }];
        Ok(MeshModel {
            triangles,
            nodes,
            materials,
        })
    }
}

/// `get_bounds` gives the box around `triangles`.
///
/// Example: `let (lo, hi) = get_bounds(&triangles);`
fn get_bounds(triangles: &[MeshTriangle]) -> (Vec3, Vec3) {
    triangles
        .iter()
        .flat_map(|t| t.p)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
}

#[cfg(test)]
mod tests {
    //! Tests for `mesh.rs` on the Hesco block (a 1.4 m cube-like block standing on the origin).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Placement | height, bottom on y = 0, centred | `the_block_is_placed` |

    use super::*;

    const HEIGHT_M: f64 = 1.4;

    #[test]
    fn the_block_is_placed() {
        let model: MeshModel = MeshModel::Load("hesco_barrier", HEIGHT_M).expect("the Hesco block");
        let (lo, hi) = get_bounds(&model.triangles);
        assert!((f64::from(hi.y - lo.y) - HEIGHT_M).abs() < 1e-4 && lo.y.abs() < 1e-4);
        assert!((lo.x + hi.x).abs() < 1e-4 && (lo.z + hi.z).abs() < 1e-4);
    }
}
