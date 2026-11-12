//! `render_mesh.rs` uploads a world's map models (`mesh.rs`) to the GPU once for
//! `render_voxel.metal`: every model's triangles, BVH nodes and materials in shared buffers, and
//! their base colour, metallic-roughness, normal and emissive images in one texture array each.
//!
//! Each frame's posed player bodies (`voxel_world.rs`'s `avatar_meshes`) are packed apart
//! (`write_avatar_buffers`) into buffers of their own, whose roots (`get_avatar_roots`) a
//! structure gives as `FIRST_AVATAR_ROOT_CODE - root`.
//!
//! Interface: `MeshGpu::New`, `MeshGpu`, `get_mesh_roots`, `get_first_materials`,
//! `get_avatar_roots`, `get_avatar_bytes`, `write_avatar_buffers`, `GpuTriangle`, `GpuNode`,
//! `NO_MESH`, `FIRST_AVATAR_ROOT_CODE`.
//!
//! Example: `let gpu = MeshGpu::New(&device, &world.meshes)?;`

use std::error::Error;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Arc;

use image::RgbaImage;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLBuffer, MTLComputeCommandEncoder, MTLDevice, MTLResourceOptions, MTLTexture};
use rayon::prelude::*;

use crate::mesh::{BvhNode, MeshModel, MeshTriangle};
use crate::texture_arrays::{make_array, make_mips};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub const NO_MESH: i32 = -1;
pub const FIRST_AVATAR_ROOT_CODE: i32 = -2;
const TEXTURE_SIDE_PX: u32 = 1024;
const BUFFER_INDICES: [usize; 3] = [0, 1, 7];
const FIRST_TEXTURE_INDEX: usize = 12;
const SLOT_FORMATS: [objc2_metal::MTLPixelFormat; 4] = [
    objc2_metal::MTLPixelFormat::RGBA8Unorm_sRGB,
    objc2_metal::MTLPixelFormat::RGBA8Unorm,
    objc2_metal::MTLPixelFormat::RGBA8Unorm,
    objc2_metal::MTLPixelFormat::RGBA8Unorm_sRGB,
];

/// `GpuTriangle` is the shader's `MeshTriangle`: corners with u, normals with v, and (material,
/// unused...).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GpuTriangle {
    p: [[f32; 4]; 3],
    n: [[f32; 4]; 3],
    info: [i32; 4],
}

/// `GpuNode` is the shader's `BvhNode`: its box, with its first child or triangle in `lo.w` and
/// its triangle count (0 for an inner node) in `hi.w`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GpuNode {
    lo: [f32; 4],
    hi: [f32; 4],
}

/// `GpuMaterial` is the shader's `MeshMaterial`: base colour, emissive colour with metallic,
/// (roughness, unused...), and the layer of each map (-1 where it has none).
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuMaterial {
    base_color: [f32; 4],
    emissive_metallic: [f32; 4],
    params: [f32; 4],
    layers: [i32; 4],
}

/// `MeshGpu` is the uploaded models: the buffers of triangles, nodes and materials, and the four
/// texture arrays.
pub struct MeshGpu {
    buffers: [Retained<ProtocolObject<dyn MTLBuffer>>; 3],
    maps: Vec<Retained<ProtocolObject<dyn MTLTexture>>>,
}

/// `get_mesh_roots` gives each model's root node in the uploaded node buffer.
///
/// Example: `let roots = get_mesh_roots(&world.meshes);`
pub fn get_mesh_roots(meshes: &[MeshModel]) -> Vec<i32> {
    meshes
        .iter()
        .scan(0, |first, model| {
            let root: i32 = *first;
            *first += model.nodes.len() as i32;
            Some(root)
        })
        .collect()
}

/// `get_first_materials` gives where each model's materials start in the uploaded material buffer.
///
/// Example: `let firsts = get_first_materials(&world.meshes);`
pub fn get_first_materials(meshes: &[MeshModel]) -> Vec<i32> {
    meshes
        .iter()
        .scan(0, |first, model| {
            let start: i32 = *first;
            *first += model.materials.len() as i32;
            Some(start)
        })
        .collect()
}

/// `make_gpu_triangle` gives a model's triangle as the shader reads it, its material counted from
/// `first_material`.
///
/// Example: `let packed: GpuTriangle = make_gpu_triangle(&model.triangles[0], 0);`
fn make_gpu_triangle(t: &MeshTriangle, first_material: i32) -> GpuTriangle {
    GpuTriangle {
        p: [0, 1, 2].map(|i| t.p[i].extend(t.uv[i].x).to_array()),
        n: [0, 1, 2].map(|i| t.n[i].extend(t.uv[i].y).to_array()),
        info: [first_material + t.material as i32, 0, 0, 0],
    }
}

/// `make_gpu_node` gives a model's BVH node as the shader reads it, in buffers where the model's
/// triangles start at `first_triangle` and its nodes at `root`.
///
/// Example: `let packed: GpuNode = make_gpu_node(&model.nodes[0], 0.0, 0.0);`
fn make_gpu_node(node: &BvhNode, first_triangle: f32, root: f32) -> GpuNode {
    let first: f32 = if node.count == 0 {
        root
    } else {
        first_triangle
    };
    GpuNode {
        lo: node.lo.extend(first + node.first as f32).to_array(),
        hi: node.hi.extend(node.count as f32).to_array(),
    }
}

/// `pack_model` appends `model`'s triangles and nodes to `triangles` and `nodes`, its root at
/// `nodes.len()` and its materials from `first_material`.
///
/// Example: `pack_model(&model, 0, &mut triangles, &mut nodes);`
fn pack_model(
    model: &MeshModel,
    first_material: i32,
    triangles: &mut Vec<GpuTriangle>,
    nodes: &mut Vec<GpuNode>,
) {
    let (first_triangle, root) = (triangles.len() as f32, nodes.len() as f32);
    triangles.extend(
        model
            .triangles
            .iter()
            .map(|t| make_gpu_triangle(t, first_material)),
    );
    nodes.extend(
        model
            .nodes
            .iter()
            .map(|node| make_gpu_node(node, first_triangle, root)),
    );
}

/// `get_avatar_roots` gives the root node of each of this frame's posed bodies in the bodies'
/// node buffer, where `write_avatar_buffers` puts them one after another.
///
/// Example: `let roots: Vec<i32> = get_avatar_roots(&world.avatar_meshes);`
pub fn get_avatar_roots(meshes: &[Arc<MeshModel>]) -> Vec<i32> {
    meshes
        .iter()
        .scan(0, |root, model| {
            let start: i32 = *root;
            *root += model.nodes.len() as i32;
            Some(start)
        })
        .collect()
}

/// `get_avatar_bytes` gives how many bytes of triangles and of nodes this frame's posed bodies
/// take in the bodies' buffers.
///
/// Example: `let [triangle_bytes, node_bytes] = get_avatar_bytes(&frame.avatar_meshes);`
pub fn get_avatar_bytes(meshes: &[Arc<MeshModel>]) -> [usize; 2] {
    let triangle_count: usize = meshes.iter().map(|model| model.triangles.len()).sum();
    let node_count: usize = meshes.iter().map(|model| model.nodes.len()).sum();
    [
        triangle_count * std::mem::size_of::<GpuTriangle>(),
        node_count * std::mem::size_of::<GpuNode>(),
    ]
}

/// `BodyPart` is one posed body with where it goes among a frame's bodies: the index of its first
/// triangle, its root node's index, and its own stretches of the triangles and of the nodes.
struct BodyPart<'a> {
    model: &'a MeshModel,
    first_triangle: usize,
    root: usize,
    triangles: &'a mut [GpuTriangle],
    nodes: &'a mut [GpuNode],
}

/// `write_avatar_meshes` packs this frame's posed bodies, whose materials start at
/// `first_material`, into the start of `triangles` and `nodes`, one body after another and the
/// bodies in parallel: ten bodies are 18 MB of triangles a frame, most of a frame's time on one
/// thread (W-155, play against bots).
///
/// Example: `write_avatar_meshes(&world.avatar_meshes, 12, &mut triangles, &mut nodes);`
fn write_avatar_meshes(
    meshes: &[Arc<MeshModel>],
    first_material: i32,
    triangles: &mut [GpuTriangle],
    nodes: &mut [GpuNode],
) {
    let mut parts: Vec<BodyPart> = Vec::with_capacity(meshes.len());
    let (mut later_triangles, mut later_nodes) = (triangles, nodes);
    let (mut first_triangle, mut root) = (0, 0);
    for model in meshes {
        let (triangles, rest) = later_triangles.split_at_mut(model.triangles.len());
        later_triangles = rest;
        let (nodes, rest) = later_nodes.split_at_mut(model.nodes.len());
        later_nodes = rest;
        parts.push(BodyPart {
            model,
            first_triangle,
            root,
            triangles,
            nodes,
        });
        first_triangle += model.triangles.len();
        root += model.nodes.len();
    }
    parts.into_par_iter().for_each(|part| {
        for (packed, t) in part.triangles.iter_mut().zip(&part.model.triangles) {
            *packed = make_gpu_triangle(t, first_material);
        }
        for (packed, node) in part.nodes.iter_mut().zip(&part.model.nodes) {
            *packed = make_gpu_node(node, part.first_triangle as f32, part.root as f32);
        }
    });
}

/// `write_avatar_buffers` packs this frame's posed bodies (`write_avatar_meshes`) straight into
/// the shared buffers the shader reads them from, which hold at least `get_avatar_bytes`.
///
/// Example: `write_avatar_buffers(&frame.avatar_meshes, 12, &triangles_device, &nodes_device);`
pub fn write_avatar_buffers(
    meshes: &[Arc<MeshModel>],
    first_material: i32,
    triangles_device: &ProtocolObject<dyn MTLBuffer>,
    nodes_device: &ProtocolObject<dyn MTLBuffer>,
) {
    let [triangle_bytes, node_bytes] = get_avatar_bytes(meshes);
    assert!(
        triangles_device.length() >= triangle_bytes && nodes_device.length() >= node_bytes,
        "write_avatar_buffers: buffers of {} and {} bytes for {triangle_bytes} and {node_bytes}",
        triangles_device.length(),
        nodes_device.length()
    );
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A WRITE PAST THE BODIES' BUFFERS WILL HAPPEN
    let (triangles, nodes) = unsafe {
        (
            std::slice::from_raw_parts_mut(
                triangles_device.contents().as_ptr().cast::<GpuTriangle>(),
                triangle_bytes / std::mem::size_of::<GpuTriangle>(),
            ),
            std::slice::from_raw_parts_mut(
                nodes_device.contents().as_ptr().cast::<GpuNode>(),
                node_bytes / std::mem::size_of::<GpuNode>(),
            ),
        )
    };
    write_avatar_meshes(meshes, first_material, triangles, nodes);
}

/// `make_buffer` copies `items` into a new shared buffer.
///
/// Example: `let buffer = make_buffer(device, &nodes)?;`
fn make_buffer<T>(
    device: &ProtocolObject<dyn MTLDevice>,
    items: &[T],
) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>> {
    if items.is_empty() {
        return Ok(device
            .newBufferWithLength_options(1, MTLResourceOptions::StorageModeShared)
            .ok_or("Metal: no memory for the map models")?);
    }
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY PAST THE ITEMS WILL HAPPEN
    let buffer = unsafe {
        device.newBufferWithBytes_length_options(
            NonNull::new(items.as_ptr().cast_mut().cast()).expect("a slice isn't null"),
            std::mem::size_of_val(items),
            MTLResourceOptions::StorageModeShared,
        )
    };
    Ok(buffer.ok_or("Metal: no memory for the map models")?)
}

impl MeshGpu {
    /// `New` uploads `meshes`, decoding their images in parallel.
    ///
    /// Example: `let gpu = MeshGpu::New(&device, &world.meshes)?;`
    #[allow(non_snake_case)]
    pub fn New(device: &ProtocolObject<dyn MTLDevice>, meshes: &[MeshModel]) -> Result<MeshGpu> {
        let (mut triangles, mut nodes, mut materials) = (Vec::new(), Vec::new(), Vec::new());
        let mut slot_images: [Vec<PathBuf>; 4] = Default::default();
        for model in meshes {
            pack_model(model, materials.len() as i32, &mut triangles, &mut nodes);
            for m in &model.materials {
                let layers: [i32; 4] = std::array::from_fn(|slot| {
                    m.images[slot].as_ref().map_or(NO_MESH, |path| {
                        let images: &mut Vec<PathBuf> = &mut slot_images[slot];
                        images.iter().position(|p| p == path).unwrap_or_else(|| {
                            images.push(path.clone());
                            images.len() - 1
                        }) as i32
                    })
                });
                materials.push(GpuMaterial {
                    base_color: m.base_color,
                    emissive_metallic: [m.emissive[0], m.emissive[1], m.emissive[2], m.metallic],
                    params: [m.roughness, 0.0, 0.0, 0.0],
                    layers,
                });
            }
        }
        let maps = slot_images
            .iter()
            .zip(SLOT_FORMATS)
            .map(|(paths, format)| {
                let layers: Vec<Vec<RgbaImage>> = paths
                    .par_iter()
                    .map(|path| make_mips(path, TEXTURE_SIDE_PX))
                    .collect::<std::result::Result<_, String>>()?;
                make_array(device, format, 4, &layers)
            })
            .collect::<Result<_>>()?;
        Ok(MeshGpu {
            buffers: [
                make_buffer(device, &triangles)?,
                make_buffer(device, &nodes)?,
                make_buffer(device, &materials)?,
            ],
            maps,
        })
    }

    /// `bind` sets the models' buffers and texture arrays on `encoder` for the shader.
    ///
    /// Example: `mesh_gpu.bind(&encoder);`
    pub fn bind(&self, encoder: &ProtocolObject<dyn MTLComputeCommandEncoder>) {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN THE SHADER READING THE WRONG BUFFERS WILL HAPPEN
        unsafe {
            for (buffer, index) in self.buffers.iter().zip(BUFFER_INDICES) {
                encoder.setBuffer_offset_atIndex(Some(buffer), 0, index);
            }
            for (k, map) in self.maps.iter().enumerate() {
                encoder.setTexture_atIndex(Some(map), FIRST_TEXTURE_INDEX + k);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for `render_mesh.rs`'s packing of posed bodies, on small made-up models.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Bodies | none, one, several of different sizes | `bodies_pack_as_models_packed_in_turn` |
    //! | Node | inner (points at nodes from the body's root), leaf (points at the body's triangles) | `bodies_pack_as_models_packed_in_turn` |
    //! | Room in the buffers | exactly the bodies', more (the rest untouched) | `bodies_pack_as_models_packed_in_turn` |

    use glam::{Vec2, Vec3};

    use super::*;

    const FIRST_MATERIAL: i32 = 12;
    const SPARE_COUNT: usize = 3;
    const UNTOUCHED: f32 = -7.0;

    /// `make_model` gives a model of `leaf_count` leaves of two triangles each under one root,
    /// its numbers all different and set by `seed`.
    ///
    /// Example: `let model: MeshModel = make_model(3, 0.5);`
    fn make_model(leaf_count: usize, seed: f32) -> MeshModel {
        let triangles: Vec<MeshTriangle> = (0..leaf_count * 2)
            .map(|k| {
                let corner: Vec3 = Vec3::new(k as f32, seed, k as f32 * seed);
                MeshTriangle {
                    p: [corner, corner + Vec3::X, corner + Vec3::Y],
                    n: [Vec3::Z, Vec3::X, Vec3::Y],
                    uv: [Vec2::new(seed, k as f32), Vec2::X, Vec2::Y],
                    material: k % 3,
                }
            })
            .collect();
        let root: BvhNode = BvhNode {
            lo: Vec3::splat(-seed),
            hi: Vec3::splat(seed),
            first: 1,
            count: 0,
        };
        let leaves = (0..leaf_count).map(|k| BvhNode {
            lo: Vec3::splat(k as f32),
            hi: Vec3::splat(k as f32 + seed),
            first: k * 2,
            count: 2,
        });
        MeshModel {
            triangles,
            nodes: std::iter::once(root).chain(leaves).collect(),
            materials: Vec::new(),
        }
    }

    /// `get_numbers` gives packed triangles and nodes as plain numbers, to compare them.
    ///
    /// Example: `assert_eq!(get_numbers(&triangles, &nodes), get_numbers(&others, &other_nodes));`
    fn get_numbers(triangles: &[GpuTriangle], nodes: &[GpuNode]) -> (Vec<f32>, Vec<i32>) {
        let floats: Vec<f32> = triangles
            .iter()
            .flat_map(|t| t.p.into_iter().chain(t.n).flatten())
            .chain(nodes.iter().flat_map(|node| [node.lo, node.hi]).flatten())
            .collect();
        (floats, triangles.iter().flat_map(|t| t.info).collect())
    }

    #[test]
    fn bodies_pack_as_models_packed_in_turn() {
        let spare_triangle: GpuTriangle = GpuTriangle {
            p: [[UNTOUCHED; 4]; 3],
            n: [[UNTOUCHED; 4]; 3],
            info: [0; 4],
        };
        let spare_node: GpuNode = GpuNode {
            lo: [UNTOUCHED; 4],
            hi: [UNTOUCHED; 4],
        };
        for leaf_counts in [vec![], vec![2], vec![3, 1, 5, 2]] {
            let meshes: Vec<Arc<MeshModel>> = leaf_counts
                .iter()
                .enumerate()
                .map(|(k, &leaf_count)| Arc::new(make_model(leaf_count, 0.5 + k as f32)))
                .collect();
            let (mut expected_triangles, mut expected_nodes) = (Vec::new(), Vec::new());
            let mut expected_roots: Vec<i32> = Vec::new();
            for model in &meshes {
                expected_roots.push(expected_nodes.len() as i32);
                pack_model(
                    model,
                    FIRST_MATERIAL,
                    &mut expected_triangles,
                    &mut expected_nodes,
                );
            }
            assert_eq!(get_avatar_roots(&meshes), expected_roots);
            assert_eq!(
                get_avatar_bytes(&meshes),
                [
                    std::mem::size_of_val(expected_triangles.as_slice()),
                    std::mem::size_of_val(expected_nodes.as_slice())
                ]
            );
            for spare_count in [0, SPARE_COUNT] {
                let mut triangles: Vec<GpuTriangle> =
                    vec![spare_triangle; expected_triangles.len() + spare_count];
                let mut nodes: Vec<GpuNode> = vec![spare_node; expected_nodes.len() + spare_count];
                write_avatar_meshes(&meshes, FIRST_MATERIAL, &mut triangles, &mut nodes);
                let (written_triangles, spare_triangles) =
                    triangles.split_at(expected_triangles.len());
                let (written_nodes, spare_nodes) = nodes.split_at(expected_nodes.len());
                assert_eq!(
                    get_numbers(written_triangles, written_nodes),
                    get_numbers(&expected_triangles, &expected_nodes),
                    "{leaf_counts:?} with {spare_count} spare"
                );
                assert_eq!(
                    get_numbers(spare_triangles, spare_nodes),
                    get_numbers(
                        &vec![spare_triangle; spare_count],
                        &vec![spare_node; spare_count]
                    ),
                    "{leaf_counts:?}: the room after the bodies changed"
                );
            }
        }
    }
}
