//! `mesh.rs` is a glTF model of `common/maps/models/` placed in its structure's chart q (meters,
//! where straight lines are geodesics, so its triangles are exact geodesic faces): its triangles,
//! a bounding volume hierarchy over them for ray hits and distance bounds, and its materials.
//!
//! Interface: `MeshModel::Load`, `MeshModel::FromTriangles`, `MeshModel`, `MeshTriangle`, `BvhNode`,
//! `MeshMaterial`, `read_materials`.
//!
//! Example: `let model = MeshModel::Load("hesco_barrier", 1.4)?;`

use std::path::{Path, PathBuf};

use glam::{DVec3, Mat3, Mat4, Vec2, Vec3};

use crate::map::get_model_path;

const LEAF_TRIANGLE_COUNT: usize = 4;
const PARALLEL_DET: f64 = 1e-12;

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

/// `read_materials` reads every material of glTF document `document`, whose images are files in
/// `folder`, for model `model`.
///
/// Example: `let materials = read_materials(&document, folder, "teal_robot")?;`
pub fn read_materials(
    document: &gltf::Document,
    folder: &Path,
    model: &str,
) -> Result<Vec<MeshMaterial>, String> {
    let image_paths: Vec<PathBuf> = document
        .images()
        .map(|image| match image.source() {
            gltf::image::Source::Uri { uri, .. } => Ok(folder.join(uri)),
            gltf::image::Source::View { .. } => Err(format!("mesh: {model} embeds an image")),
        })
        .collect::<Result<_, String>>()?;
    Ok(document
        .materials()
        .map(|m| read_material(&m, &image_paths))
        .collect())
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
        let materials: Vec<MeshMaterial> = read_materials(&document, folder, model)?;
        Ok(MeshModel::FromTriangles(triangles, materials).0)
    }

    /// `FromTriangles` builds the BVH over `triangles` and gives the model, its triangles reordered
    /// so each leaf's are contiguous, and for each placed triangle its index in `triangles`.
    ///
    /// Example: `let (model, order) = MeshModel::FromTriangles(triangles, materials);`
    #[allow(non_snake_case)]
    pub fn FromTriangles(
        triangles: Vec<MeshTriangle>,
        materials: Vec<MeshMaterial>,
    ) -> (MeshModel, Vec<usize>) {
        let corners = |item: &(MeshTriangle, usize)| item.0.p;
        let mut items: Vec<(MeshTriangle, usize)> = triangles.into_iter().zip(0..).collect();
        let (lo, hi) = get_item_bounds(&items, &corners);
        let mut nodes: Vec<BvhNode> = vec![BvhNode {
            lo,
            hi,
            first: 0,
            count: items.len(),
        }];
        split_node(&mut nodes, 0, &mut items, &corners);
        let (triangles, order): (Vec<MeshTriangle>, Vec<usize>) = items.into_iter().unzip();
        let model: MeshModel = MeshModel {
            triangles,
            nodes,
            materials,
        };
        (model, order)
    }

    /// `refit` fits every BVH box around its triangles again after they moved, keeping the tree,
    /// children before their parent (each node's children come after it).
    ///
    /// Example: `posed.refit();`
    pub fn refit(&mut self) {
        for k in (0..self.nodes.len()).rev() {
            let BvhNode { first, count, .. } = self.nodes[k];
            let (lo, hi) = if count == 0 {
                let (a, b) = (self.nodes[first], self.nodes[first + 1]);
                (a.lo.min(b.lo), a.hi.max(b.hi))
            } else {
                get_bounds(&self.triangles[first..first + count])
            };
            (self.nodes[k].lo, self.nodes[k].hi) = (lo, hi);
        }
    }

    /// `find_hit_mu` gives where the line q0 + dq mu first meets a triangle within [mu_lo, mu_hi],
    /// and which triangle, or `None`.
    ///
    /// Example: `let hit = model.find_hit_mu(q0, dq, 0.0, 1.0);`
    pub fn find_hit_mu(
        &self,
        q0: DVec3,
        dq: DVec3,
        mu_lo: f64,
        mu_hi: f64,
    ) -> Option<(f64, usize)> {
        let (mut best, mut stack) = (None, vec![0usize]);
        let mut hi: f64 = mu_hi;
        while let Some(k) = stack.pop() {
            let node: BvhNode = self.nodes[k];
            let a: DVec3 = (node.lo.as_dvec3() - q0) / dq;
            let b: DVec3 = (node.hi.as_dvec3() - q0) / dq;
            if a.min(b).max_element().max(mu_lo) > a.max(b).min_element().min(hi) {
                continue;
            }
            if node.count == 0 {
                stack.extend([node.first, node.first + 1]);
                continue;
            }
            for i in node.first..node.first + node.count {
                if let Some(mu) =
                    hit_triangle(&self.triangles[i], q0, dq).filter(|&mu| mu >= mu_lo && mu < hi)
                {
                    (hi, best) = (mu, Some((mu, i)));
                }
            }
        }
        best
    }

    /// `is_within_m` says whether some triangle comes within `radius_m` of q (exactly, not by
    /// boxes), as walls stop players.
    ///
    /// Example: `if model.is_within_m(q, 0.1) { ... }`
    pub fn is_within_m(&self, q: DVec3, radius_m: f64) -> bool {
        let mut stack: Vec<usize> = vec![0];
        while let Some(k) = stack.pop() {
            let node: BvhNode = self.nodes[k];
            let gap: f64 = (node.lo.as_dvec3() - q)
                .max(q - node.hi.as_dvec3())
                .max(DVec3::ZERO)
                .length();
            if gap >= radius_m {
                continue;
            }
            if node.count == 0 {
                stack.extend([node.first, node.first + 1]);
            } else if self.triangles[node.first..node.first + node.count]
                .iter()
                .any(|t| get_triangle_distance_m(t, q) < radius_m)
            {
                return true;
            }
        }
        false
    }

    /// `get_distance_m` bounds the distance from q to the model's triangles from below, by the
    /// distance to the nearest BVH leaf box.
    ///
    /// Example: `let d_m = model.get_distance_m(DVec3::new(0.0, 5.0, 0.0));`
    pub fn get_distance_m(&self, q: DVec3) -> f64 {
        let (mut best, mut stack) = (f64::MAX, vec![0usize]);
        while let Some(k) = stack.pop() {
            let node: BvhNode = self.nodes[k];
            let gap: f64 = (node.lo.as_dvec3() - q)
                .max(q - node.hi.as_dvec3())
                .max(DVec3::ZERO)
                .length();
            if gap >= best {
                continue;
            }
            if node.count == 0 {
                stack.extend([node.first, node.first + 1]);
            } else {
                best = gap;
            }
        }
        best
    }
}

/// `get_triangle_distance_m` gives the distance from q to triangle t (Ericson's closest point).
///
/// Example: `let d_m = get_triangle_distance_m(&triangle, q);`
fn get_triangle_distance_m(t: &MeshTriangle, q: DVec3) -> f64 {
    let [a, b, c] = t.p.map(|p| p.as_dvec3());
    let (ab, ac, ap) = (b - a, c - a, q - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length();
    }
    let bp: DVec3 = q - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length();
    }
    let cp: DVec3 = q - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length();
    }
    let (vc, vb, va) = (d1 * d4 - d3 * d2, d5 * d2 - d1 * d6, d3 * d6 - d5 * d4);
    let closest: DVec3 = if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        a + ab * (d1 / (d1 - d3))
    } else if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        a + ac * (d2 / (d2 - d6))
    } else if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)))
    } else {
        let denominator: f64 = 1.0 / (va + vb + vc);
        a + ab * (vb * denominator) + ac * (vc * denominator)
    };
    (q - closest).length()
}

/// `get_bounds` gives the box around `triangles`.
///
/// Example: `let (lo, hi) = get_bounds(&triangles);`
fn get_bounds(triangles: &[MeshTriangle]) -> (Vec3, Vec3) {
    get_item_bounds(triangles, &|t: &MeshTriangle| t.p)
}

/// `get_item_bounds` gives the box around the triangles `corners` gives for `items`.
///
/// Example: `let (lo, hi) = get_item_bounds(&items, &|item| item.0.p);`
fn get_item_bounds<T>(items: &[T], corners: &impl Fn(&T) -> [Vec3; 3]) -> (Vec3, Vec3) {
    items
        .iter()
        .flat_map(corners)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
}

/// `split_node` splits node k's triangles at the median along the longest axis of their centres,
/// until leaves hold at most `LEAF_TRIANGLE_COUNT`.
///
/// Example: `split_node(&mut nodes, 0, &mut items, &|item| item.0.p);`
fn split_node<T>(
    nodes: &mut Vec<BvhNode>,
    k: usize,
    triangles: &mut [T],
    corners: &impl Fn(&T) -> [Vec3; 3],
) {
    let BvhNode { first, count, .. } = nodes[k];
    if count <= LEAF_TRIANGLE_COUNT {
        return;
    }
    let part: &mut [T] = &mut triangles[first..first + count];
    let centre = |t: &T| corners(t).into_iter().sum::<Vec3>();
    let (lo, hi) = part
        .iter()
        .map(centre)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), c| (lo.min(c), hi.max(c)));
    let axis: usize = (hi - lo).max_position();
    let half: usize = count / 2;
    part.select_nth_unstable_by(half, |a, b| centre(a)[axis].total_cmp(&centre(b)[axis]));
    let children: usize = nodes.len();
    for (start, length) in [(first, half), (first + half, count - half)] {
        let (lo, hi) = get_item_bounds(&triangles[start..start + length], corners);
        nodes.push(BvhNode {
            lo,
            hi,
            first: start,
            count: length,
        });
    }
    nodes[k] = BvhNode {
        first: children,
        count: 0,
        ..nodes[k]
    };
    split_node(nodes, children, triangles, corners);
    split_node(nodes, children + 1, triangles, corners);
}

/// `hit_triangle` gives where the line q0 + dq mu crosses triangle t (Möller-Trumbore), or `None`.
///
/// Example: `let mu = hit_triangle(&triangle, q0, dq);`
fn hit_triangle(t: &MeshTriangle, q0: DVec3, dq: DVec3) -> Option<f64> {
    let [a, b, c] = t.p.map(|p| p.as_dvec3());
    let (e1, e2) = (b - a, c - a);
    let h: DVec3 = dq.cross(e2);
    let det: f64 = e1.dot(h);
    if det.abs() < PARALLEL_DET * dq.length() {
        return None;
    }
    let s: DVec3 = q0 - a;
    let u: f64 = s.dot(h) / det;
    let g: DVec3 = s.cross(e1);
    let v: f64 = dq.dot(g) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0).then(|| e2.dot(g) / det)
}

#[cfg(test)]
mod tests {
    //! Tests for `mesh.rs` on the Hesco block (a 1.4 m cube-like block standing on the origin).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Placement | height, bottom on y = 0, centred | `the_block_is_placed` |
    //! | Ray | hits the block, passes beside it, stops short of it | `rays_hit_or_miss` |
    //! | Distance bound | far from the block, next to it | `distances_are_bounds` |
    //! | Within a radius | inside the radius of a wall, just outside it | `walls_are_near_within_their_radius` |

    use super::*;

    const HEIGHT_M: f64 = 1.4;

    #[test]
    fn the_block_is_placed() {
        let model: MeshModel = MeshModel::Load("hesco_barrier", HEIGHT_M).expect("the Hesco block");
        let (lo, hi) = get_bounds(&model.triangles);
        assert!((f64::from(hi.y - lo.y) - HEIGHT_M).abs() < 1e-4 && lo.y.abs() < 1e-4);
        assert!((lo.x + hi.x).abs() < 1e-4 && (lo.z + hi.z).abs() < 1e-4);
    }

    #[test]
    fn rays_hit_or_miss() {
        let model: MeshModel = MeshModel::Load("hesco_barrier", HEIGHT_M).expect("the Hesco block");
        let (q0, dq) = (DVec3::new(-5.0, 0.7, 0.0), DVec3::X);
        let (mu, _) = model.find_hit_mu(q0, dq, 0.0, 10.0).expect("a hit");
        assert!((4.0..5.0).contains(&mu), "hit at {mu}");
        assert!(model.find_hit_mu(q0, dq, 0.0, 3.0).is_none());
        assert!(
            model
                .find_hit_mu(DVec3::new(-5.0, 3.0, 0.0), dq, 0.0, 10.0)
                .is_none()
        );
    }

    #[test]
    fn distances_are_bounds() {
        let model: MeshModel = MeshModel::Load("hesco_barrier", HEIGHT_M).expect("the Hesco block");
        for q in [
            DVec3::new(-5.0, 0.7, 0.0),
            DVec3::new(-0.9, 0.7, 0.0),
            DVec3::new(0.0, 4.0, 0.0),
        ] {
            let dq: DVec3 = (DVec3::new(0.0, 0.7, 0.0) - q).normalize();
            let (mu, _) = model
                .find_hit_mu(q, dq, 0.0, 10.0)
                .expect("a hit toward the centre");
            assert!(
                model.get_distance_m(q) <= mu + 1e-6,
                "bound over the hit at {q}"
            );
        }
    }

    #[test]
    fn walls_are_near_within_their_radius() {
        let model: MeshModel = MeshModel::Load("hesco_barrier", HEIGHT_M).expect("the Hesco block");
        let (q0, dq) = (DVec3::new(-5.0, 0.7, 0.0), DVec3::X);
        let (mu, _) = model.find_hit_mu(q0, dq, 0.0, 10.0).expect("a hit");
        let wall: DVec3 = q0 + dq * mu;
        assert!(model.is_within_m(wall - DVec3::X * 0.09, 0.1));
        assert!(!model.is_within_m(wall - DVec3::X * 0.11, 0.1));
    }
}
