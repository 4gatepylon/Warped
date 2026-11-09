//! `render_mesh.rs` uploads a world's map models (`mesh.rs`) to the GPU once for
//! `render_voxel.metal`: every model's triangles, BVH nodes and materials in shared buffers, and
//! their base colour, metallic-roughness, normal and emissive images in one texture array each.
//!
//! Each frame's posed player bodies (`voxel_world.rs`'s `avatar_meshes`) are packed apart
//! (`pack_avatar_meshes`) into buffers of their own, whose roots a structure gives as
//! `FIRST_AVATAR_ROOT_CODE - root`.
//!
//! Interface: `MeshGpu::New`, `MeshGpu`, `get_mesh_roots`, `get_first_materials`,
//! `pack_avatar_meshes`, `GpuTriangle`, `GpuNode`, `NO_MESH`, `FIRST_AVATAR_ROOT_CODE`.
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

use crate::mesh::MeshModel;
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
    triangles.extend(model.triangles.iter().map(|t| GpuTriangle {
        p: [0, 1, 2].map(|i| t.p[i].extend(t.uv[i].x).to_array()),
        n: [0, 1, 2].map(|i| t.n[i].extend(t.uv[i].y).to_array()),
        info: [first_material + t.material as i32, 0, 0, 0],
    }));
    nodes.extend(model.nodes.iter().map(|node| {
        let first: f32 = if node.count == 0 {
            root
        } else {
            first_triangle
        };
        GpuNode {
            lo: node.lo.extend(first + node.first as f32).to_array(),
            hi: node.hi.extend(node.count as f32).to_array(),
        }
    }));
}

/// `pack_avatar_meshes` packs this frame's posed bodies, whose materials start at
/// `first_material`, giving their triangles, nodes and each body's root node.
///
/// Example: `let (triangles, nodes, roots) = pack_avatar_meshes(&world.avatar_meshes, 12);`
pub fn pack_avatar_meshes(
    meshes: &[Arc<MeshModel>],
    first_material: i32,
) -> (Vec<GpuTriangle>, Vec<GpuNode>, Vec<i32>) {
    let (mut triangles, mut nodes, mut roots) = (Vec::new(), Vec::new(), Vec::new());
    for model in meshes {
        roots.push(nodes.len() as i32);
        pack_model(model, first_material, &mut triangles, &mut nodes);
    }
    (triangles, nodes, roots)
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
