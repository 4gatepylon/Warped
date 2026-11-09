//! `gltf_model` loads glTF models into renderer meshes and materials: one merged mesh per
//! material, either flattened to its rest pose or skinned to a palette for animation.
//!
//! Rigid parts of an animated model (meshes hanging off animated nodes) become skinned vertices
//! bound with full weight to their node, so a whole animated model draws as one skinned mesh per
//! material.
//!
//! Example:
//!     let model: Model = load_model(&mut renderer, &path, &ModelOptions::default());

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use gltf::animation::util::ReadOutputs;
use gltf::animation::{Interpolation, Property};

use super::textures::{TextureRequest, TextureSource, decode_textures};
use crate::render::{
    AlphaMode, MaterialData, MaterialHandle, MeshData, MeshHandle, Renderer, SkinVertex,
    TextureFormat, TextureHandle, Vertex,
};

/// `TRANSMISSION_OPACITY_FRAC` is how opaque a fully transmissive (glass) surface is drawn.
const TRANSMISSION_OPACITY_FRAC: f32 = 0.18;
/// `DEFAULT_MAX_TEXTURE_SIZE_PX` is the default texture size limit.
const DEFAULT_MAX_TEXTURE_SIZE_PX: u32 = 2048;
/// `MIN_TANGENT_LENGTH` is the length below which a tangent or UV determinant is degenerate.
const MIN_TANGENT_LENGTH: f32 = 1e-8;

/// `ModelOptions` says how to load a model; `should_keep_positions` keeps its vertices' positions
/// in `Model::positions_m` (the guns, to find their muzzles).
///
/// Example:
///     let options = ModelOptions { is_animated: true, ..ModelOptions::default() };
pub struct ModelOptions<'a> {
    pub should_include_mesh_node: &'a dyn Fn(&str) -> bool,
    pub max_texture_size_px: u32,
    pub is_animated: bool,
    pub should_recenter: bool,
    pub emissive_scale: f32,
    pub min_x_m: f32,
    pub base_color_tint_rgb: Vec3,
    pub should_keep_positions: bool,
}

impl Default for ModelOptions<'_> {
    fn default() -> Self {
        ModelOptions {
            should_include_mesh_node: &|_: &str| true,
            max_texture_size_px: DEFAULT_MAX_TEXTURE_SIZE_PX,
            is_animated: false,
            should_recenter: true,
            emissive_scale: 1.0,
            min_x_m: f32::MIN,
            base_color_tint_rgb: Vec3::ONE,
            should_keep_positions: false,
        }
    }
}

/// `ModelPart` is one material's merged mesh.
#[derive(Clone, Copy)]
pub struct ModelPart {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub material_index: Option<usize>,
}

/// `Trs` is a node's local translation, rotation, and scale.
#[derive(Clone, Copy)]
pub struct Trs {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Trs {
    /// `matrix` returns the transform as a matrix.
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// `Channel` is one animated property of one node.
struct Channel {
    node: usize,
    property: Property,
    times_s: Vec<f32>,
    values: Vec<Vec4>,
    is_step: bool,
}

/// `Animation` is a named clip.
pub struct Animation {
    pub name: String,
    pub duration_s: f32,
    channels: Vec<Channel>,
}

/// `Skeleton` is an animated model's node hierarchy, its skinning palette, and its clips.
///
/// Example:
///     let pose: Vec<Trs> = skeleton.sample(idle, time_s);
///     skeleton.write_palette(&pose, model_matrix, &mut joint_matrices);
pub struct Skeleton {
    parents: Vec<Option<usize>>,
    rest: Vec<Trs>,
    order: Vec<usize>,
    palette: Vec<(usize, Mat4)>,
    pub animations: Vec<Animation>,
}

impl Skeleton {
    /// `animation_index` finds a clip by name.
    pub fn animation_index(&self, name: &str) -> Option<usize> {
        self.animations
            .iter()
            .position(|animation: &Animation| animation.name == name)
    }

    /// `sample` returns every node's local transform at a time in a clip.
    pub fn sample(&self, animation_index: usize, time_s: f32) -> Vec<Trs> {
        let mut pose: Vec<Trs> = self.rest.clone();
        let animation: &Animation = &self.animations[animation_index];
        for channel in &animation.channels {
            let value: Vec4 = sample_channel(channel, time_s);
            let trs: &mut Trs = &mut pose[channel.node];
            match channel.property {
                Property::Translation => trs.translation = value.truncate(),
                Property::Rotation => trs.rotation = Quat::from_vec4(value).normalize(),
                Property::Scale => trs.scale = value.truncate(),
                Property::MorphTargetWeights => {}
            }
        }
        pose
    }

    /// `write_palette` appends the world-space joint matrices of a pose.
    pub fn write_palette(&self, pose: &[Trs], model: Mat4, joint_matrices: &mut Vec<Mat4>) {
        let mut globals: Vec<Mat4> = vec![Mat4::IDENTITY; pose.len()];
        for &node in &self.order {
            let local: Mat4 = pose[node].matrix();
            globals[node] = match self.parents[node] {
                Some(parent) => globals[parent] * local,
                None => local,
            };
        }
        joint_matrices.extend(
            self.palette
                .iter()
                .map(|(node, inverse_bind)| model * globals[*node] * *inverse_bind),
        );
    }
}

/// `sample_channel` interpolates a channel at a time, clamped to its keys.
fn sample_channel(channel: &Channel, time_s: f32) -> Vec4 {
    let times: &[f32] = &channel.times_s;
    if times.len() == 1 || time_s <= times[0] {
        return channel.values[0];
    }
    let last: usize = times.len() - 1;
    if time_s >= times[last] {
        return channel.values[last];
    }
    let next: usize = times.partition_point(|key_s: &f32| *key_s <= time_s);
    let previous: usize = next - 1;
    if channel.is_step {
        return channel.values[previous];
    }
    let frac: f32 = (time_s - times[previous]) / (times[next] - times[previous]);
    if channel.property == Property::Rotation {
        let a: Quat = Quat::from_vec4(channel.values[previous]);
        let b: Quat = Quat::from_vec4(channel.values[next]);
        return Vec4::from(a.slerp(b, frac));
    }
    channel.values[previous].lerp(channel.values[next], frac)
}

/// `Model` is a loaded model; `positions_m` holds its vertices' positions when its options keep
/// them, and is empty otherwise.
pub struct Model {
    pub parts: Vec<ModelPart>,
    pub bounds_min_m: Vec3,
    pub bounds_max_m: Vec3,
    pub skeleton: Option<Skeleton>,
    pub material_names: Vec<String>,
    pub positions_m: Vec<Vec3>,
}

/// `MeshBuilder` accumulates one material's vertices.
#[derive(Default)]
struct MeshBuilder {
    vertices: Vec<Vertex>,
    skin_vertices: Vec<SkinVertex>,
    indices: Vec<u32>,
}

/// `compute_tangents` fills tangents from positions and UVs, or any perpendicular if no UVs.
fn compute_tangents(vertices: &mut [Vertex], indices: &[u32]) {
    let mut tangents: Vec<Vec3> = vec![Vec3::ZERO; vertices.len()];
    let mut bitangents: Vec<Vec3> = vec![Vec3::ZERO; vertices.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let [a, b, c] = [
            triangle[0] as usize,
            triangle[1] as usize,
            triangle[2] as usize,
        ];
        let p0: Vec3 = Vec3::from(vertices[a].position);
        let edge_1: Vec3 = Vec3::from(vertices[b].position) - p0;
        let edge_2: Vec3 = Vec3::from(vertices[c].position) - p0;
        let uv0: Vec2 = Vec2::from(vertices[a].uv);
        let delta_uv_1: Vec2 = Vec2::from(vertices[b].uv) - uv0;
        let delta_uv_2: Vec2 = Vec2::from(vertices[c].uv) - uv0;
        let determinant: f32 = delta_uv_1.x * delta_uv_2.y - delta_uv_2.x * delta_uv_1.y;
        if determinant.abs() < MIN_TANGENT_LENGTH {
            continue;
        }
        let inverse: f32 = 1.0 / determinant;
        let tangent: Vec3 = (edge_1 * delta_uv_2.y - edge_2 * delta_uv_1.y) * inverse;
        let bitangent: Vec3 = (edge_2 * delta_uv_1.x - edge_1 * delta_uv_2.x) * inverse;
        for index in [a, b, c] {
            tangents[index] += tangent;
            bitangents[index] += bitangent;
        }
    }
    for (index, vertex) in vertices.iter_mut().enumerate() {
        let normal: Vec3 = Vec3::from(vertex.normal);
        let mut tangent: Vec3 = tangents[index] - normal * normal.dot(tangents[index]);
        if tangent.length_squared() < MIN_TANGENT_LENGTH {
            tangent = normal.any_orthogonal_vector();
        }
        let tangent: Vec3 = tangent.normalize();
        let handedness: f32 = if normal.cross(tangent).dot(bitangents[index]) < 0.0 {
            -1.0
        } else {
            1.0
        };
        vertex.tangent = tangent.extend(handedness).to_array();
    }
}

/// `TextureKey` identifies one decoded texture: a glTF image and the format it's decoded to.
type TextureKey = (usize, TextureFormatKey);

/// `TextureFormatKey` is a hashable [`TextureFormat`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum TextureFormatKey {
    Srgb,
    Linear,
    Normal,
}

impl TextureFormatKey {
    /// `format` returns the texture format the key decodes to.
    fn format(self) -> TextureFormat {
        match self {
            TextureFormatKey::Srgb => TextureFormat::Rgba8Srgb,
            TextureFormatKey::Linear => TextureFormat::Rgba8Linear,
            TextureFormatKey::Normal => TextureFormat::Rg8Linear,
        }
    }
}

/// `image_source` finds an image's bytes: a file next to the glTF, or a buffer view.
fn image_source(
    image: &gltf::Image,
    base_dir: &Path,
    buffers: &[gltf::buffer::Data],
) -> TextureSource {
    match image.source() {
        gltf::image::Source::Uri { uri, .. } => {
            let decoded: String = uri.replace("%20", " ");
            TextureSource::File(base_dir.join(PathBuf::from(decoded)))
        }
        gltf::image::Source::View { view, .. } => {
            let buffer: &[u8] = &buffers[view.buffer().index()];
            TextureSource::Bytes(buffer[view.offset()..view.offset() + view.length()].to_vec())
        }
    }
}

/// `create_materials` decodes the textures of every material and creates the materials.
fn create_materials(
    renderer: &mut dyn Renderer,
    document: &gltf::Document,
    base_dir: &Path,
    buffers: &[gltf::buffer::Data],
    max_texture_size_px: u32,
    emissive_scale: f32,
    base_color_tint_rgb: Vec3,
) -> (Vec<MaterialHandle>, MaterialHandle) {
    let mut keys: Vec<TextureKey> = Vec::new();
    let mut add_key = |texture: Option<gltf::texture::Texture>, format: TextureFormatKey| {
        if let Some(texture) = texture {
            let key: TextureKey = (texture.source().index(), format);
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    };
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        add_key(
            pbr.base_color_texture().map(|info| info.texture()),
            TextureFormatKey::Srgb,
        );
        add_key(
            pbr.metallic_roughness_texture().map(|info| info.texture()),
            TextureFormatKey::Linear,
        );
        add_key(
            material.normal_texture().map(|info| info.texture()),
            TextureFormatKey::Normal,
        );
        add_key(
            material.emissive_texture().map(|info| info.texture()),
            TextureFormatKey::Srgb,
        );
    }
    let images: Vec<gltf::Image> = document.images().collect();
    let requests: Vec<TextureRequest> = keys
        .iter()
        .map(|(image_index, format)| TextureRequest {
            source: image_source(&images[*image_index], base_dir, buffers),
            format: format.format(),
            max_size_px: max_texture_size_px,
        })
        .collect();
    let handles: HashMap<TextureKey, TextureHandle> = keys
        .iter()
        .copied()
        .zip(
            decode_textures(&requests)
                .iter()
                .map(|data| renderer.create_texture(data)),
        )
        .collect();
    let lookup = |texture: Option<gltf::texture::Texture>, format: TextureFormatKey| {
        texture.and_then(|texture| handles.get(&(texture.source().index(), format)).copied())
    };
    let mut materials: Vec<MaterialHandle> = Vec::new();
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        let metallic_roughness = pbr.metallic_roughness_texture().map(|info| info.texture());
        let has_occlusion_in_red: bool = match (&metallic_roughness, material.occlusion_texture()) {
            (Some(mr), Some(occlusion)) => {
                mr.source().index() == occlusion.texture().source().index()
            }
            _ => false,
        };
        let mut base_color_factor: Vec4 =
            Vec4::from(pbr.base_color_factor()) * base_color_tint_rgb.extend(1.0);
        let mut alpha_mode: AlphaMode = match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask,
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        };
        if let Some(transmission) = material.transmission() {
            let opacity: f32 =
                1.0 - transmission.transmission_factor() * (1.0 - TRANSMISSION_OPACITY_FRAC);
            base_color_factor.w *= opacity;
            alpha_mode = AlphaMode::Blend;
        }
        let emissive_strength: f32 = material.emissive_strength().unwrap_or(1.0) * emissive_scale;
        materials.push(renderer.create_material(&MaterialData {
            base_color_texture: lookup(
                pbr.base_color_texture().map(|info| info.texture()),
                TextureFormatKey::Srgb,
            ),
            normal_texture: lookup(
                material.normal_texture().map(|info| info.texture()),
                TextureFormatKey::Normal,
            ),
            occlusion_roughness_metal_texture: lookup(metallic_roughness, TextureFormatKey::Linear),
            emissive_texture: lookup(
                material.emissive_texture().map(|info| info.texture()),
                TextureFormatKey::Srgb,
            ),
            base_color_factor,
            roughness_factor: pbr.roughness_factor(),
            metallic_factor: pbr.metallic_factor(),
            emissive_factor: Vec3::from(material.emissive_factor()) * emissive_strength,
            has_occlusion_in_red,
            alpha_mode,
        }));
    }
    let default_material: MaterialHandle = renderer.create_material(&MaterialData::default());
    (materials, default_material)
}

/// `read_animations` reads every clip of a document.
fn read_animations(document: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Vec<Animation> {
    document
        .animations()
        .map(|animation| {
            let mut channels: Vec<Channel> = Vec::new();
            for channel in animation.channels() {
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                let times_s: Vec<f32> = match reader.read_inputs() {
                    Some(inputs) => inputs.collect(),
                    None => continue,
                };
                let mut values: Vec<Vec4> = match reader.read_outputs() {
                    Some(ReadOutputs::Translations(outputs)) => {
                        outputs.map(|value| Vec3::from(value).extend(0.0)).collect()
                    }
                    Some(ReadOutputs::Scales(outputs)) => {
                        outputs.map(|value| Vec3::from(value).extend(0.0)).collect()
                    }
                    Some(ReadOutputs::Rotations(outputs)) => {
                        outputs.into_f32().map(Vec4::from).collect()
                    }
                    _ => continue,
                };
                let interpolation: Interpolation = channel.sampler().interpolation();
                if interpolation == Interpolation::CubicSpline {
                    values = values
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|triple: &[Vec4; 3]| triple[1])
                        .collect();
                }
                if values.len() != times_s.len() || times_s.is_empty() {
                    continue;
                }
                channels.push(Channel {
                    node: channel.target().node().index(),
                    property: channel.target().property(),
                    times_s,
                    values,
                    is_step: interpolation == Interpolation::Step,
                });
            }
            let duration_s: f32 = channels
                .iter()
                .filter_map(|channel: &Channel| channel.times_s.last().copied())
                .fold(0.0, f32::max);
            Animation {
                name: animation.name().unwrap_or_default().to_string(),
                duration_s,
                channels,
            }
        })
        .collect()
}

/// `load_model` loads a glTF or GLB file and uploads its meshes and materials.
///
/// Example:
///     let model: Model = load_model(&mut renderer, &path, &ModelOptions::default());
pub fn load_model(renderer: &mut dyn Renderer, path: &Path, options: &ModelOptions) -> Model {
    let gltf::Gltf { document, blob } = gltf::Gltf::open(path)
        .unwrap_or_else(|error| panic!("cannot open {}: {error}", path.display()));
    let base_dir: &Path = path.parent().unwrap_or(Path::new("."));
    let buffers: Vec<gltf::buffer::Data> = gltf::import_buffers(&document, Some(base_dir), blob)
        .unwrap_or_else(|error| panic!("cannot read buffers of {}: {error}", path.display()));
    let (materials, default_material) = create_materials(
        renderer,
        &document,
        base_dir,
        &buffers,
        options.max_texture_size_px,
        options.emissive_scale,
        options.base_color_tint_rgb,
    );

    let node_count: usize = document.nodes().count();
    let mut parents: Vec<Option<usize>> = vec![None; node_count];
    for node in document.nodes() {
        for child in node.children() {
            parents[child.index()] = Some(node.index());
        }
    }
    let rest: Vec<Trs> = document
        .nodes()
        .map(|node| {
            let (translation, rotation, scale) = node.transform().decomposed();
            Trs {
                translation: Vec3::from(translation),
                rotation: Quat::from_array(rotation),
                scale: Vec3::from(scale),
            }
        })
        .collect();
    let mut order: Vec<usize> = Vec::with_capacity(node_count);
    let mut stack: Vec<usize> = (0..node_count)
        .filter(|node| parents[*node].is_none())
        .collect();
    while let Some(node) = stack.pop() {
        order.push(node);
        stack.extend(
            document
                .nodes()
                .nth(node)
                .into_iter()
                .flat_map(|n| n.children())
                .map(|c| c.index()),
        );
    }
    let mut globals: Vec<Mat4> = vec![Mat4::IDENTITY; node_count];
    for &node in &order {
        let local: Mat4 = rest[node].matrix();
        globals[node] = parents[node].map_or(local, |parent| globals[parent] * local);
    }

    let mut palette: Vec<(usize, Mat4)> = Vec::new();
    let mut palette_index = |node: usize, inverse_bind: Mat4| -> u16 {
        let entry: (usize, Mat4) = (node, inverse_bind);
        let index: usize = palette
            .iter()
            .position(|existing| existing.0 == node && existing.1 == inverse_bind)
            .unwrap_or_else(|| {
                palette.push(entry);
                palette.len() - 1
            });
        index as u16
    };
    let mut builders: HashMap<Option<usize>, MeshBuilder> = HashMap::new();
    let mut bounds_min_m: Vec3 = Vec3::splat(f32::MAX);
    let mut bounds_max_m: Vec3 = Vec3::splat(f32::MIN);
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        if !(options.should_include_mesh_node)(node.name().unwrap_or_default()) {
            continue;
        }
        let skin_joints: Option<(Vec<usize>, Vec<Mat4>)> = node.skin().map(|skin| {
            let joints: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
            let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
            let inverse_binds: Vec<Mat4> = reader
                .read_inverse_bind_matrices()
                .map(|matrices| matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect())
                .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);
            (joints, inverse_binds)
        });
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let positions: Vec<Vec3> = positions.map(Vec3::from).collect();
            let normals: Vec<Vec3> = reader
                .read_normals()
                .map(|normals| normals.map(Vec3::from).collect())
                .unwrap_or_else(|| vec![Vec3::Y; positions.len()]);
            let uvs: Vec<Vec2> = reader
                .read_tex_coords(0)
                .map(|uvs| uvs.into_f32().map(Vec2::from).collect())
                .unwrap_or_else(|| vec![Vec2::ZERO; positions.len()]);
            let tangents: Option<Vec<Vec4>> = reader
                .read_tangents()
                .map(|tangents| tangents.map(Vec4::from).collect());
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let joints: Option<Vec<[u16; 4]>> = reader
                .read_joints(0)
                .map(|joints| joints.into_u16().collect());
            let weights: Option<Vec<[f32; 4]>> = reader
                .read_weights(0)
                .map(|weights| weights.into_f32().collect());

            let mut skin_vertices: Vec<SkinVertex> = Vec::new();
            let mut rest_matrices: Vec<Mat4> = Vec::new();
            match (&skin_joints, &joints, &weights) {
                (Some((skin_nodes, inverse_binds)), Some(joints), Some(weights)) => {
                    for (vertex_joints, vertex_weights) in joints.iter().zip(weights) {
                        let mut mapped: [u16; 4] = [0; 4];
                        let mut rest_matrix: Mat4 = Mat4::ZERO;
                        for slot in 0..4 {
                            let joint: usize = vertex_joints[slot] as usize;
                            let inverse_bind: Mat4 = inverse_binds[joint];
                            mapped[slot] = palette_index(skin_nodes[joint], inverse_bind);
                            rest_matrix +=
                                globals[skin_nodes[joint]] * inverse_bind * vertex_weights[slot];
                        }
                        skin_vertices.push(SkinVertex {
                            joints: mapped,
                            weights: *vertex_weights,
                        });
                        rest_matrices.push(rest_matrix);
                    }
                }
                _ => {
                    let joint: u16 = palette_index(node.index(), Mat4::IDENTITY);
                    skin_vertices = vec![
                        SkinVertex {
                            joints: [joint; 4],
                            weights: [1.0, 0.0, 0.0, 0.0]
                        };
                        positions.len()
                    ];
                    rest_matrices = vec![globals[node.index()]; positions.len()];
                }
            }

            let builder: &mut MeshBuilder =
                builders.entry(primitive.material().index()).or_default();
            let base_index: u32 = builder.vertices.len() as u32;
            let mut primitive_vertices: Vec<Vertex> = Vec::with_capacity(positions.len());
            let mut rest_xs_m: Vec<f32> = Vec::with_capacity(positions.len());
            for index in 0..positions.len() {
                let rest_matrix: Mat4 = rest_matrices[index];
                let rest_position: Vec3 = rest_matrix.transform_point3(positions[index]);
                rest_xs_m.push(rest_position.x);
                if rest_position.x >= options.min_x_m {
                    bounds_min_m = bounds_min_m.min(rest_position);
                    bounds_max_m = bounds_max_m.max(rest_position);
                }
                let (position, normal, tangent) = if options.is_animated {
                    (
                        positions[index],
                        normals[index],
                        tangents.as_ref().map(|t| t[index]),
                    )
                } else {
                    let normal_matrix: Mat4 = rest_matrix.inverse().transpose();
                    let tangent: Option<Vec4> = tangents.as_ref().map(|t| {
                        rest_matrix
                            .transform_vector3(t[index].truncate())
                            .normalize()
                            .extend(t[index].w * rest_matrix.determinant().signum())
                    });
                    (
                        rest_position,
                        normal_matrix
                            .transform_vector3(normals[index])
                            .normalize_or(Vec3::Y),
                        tangent,
                    )
                };
                primitive_vertices.push(Vertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                    tangent: tangent.unwrap_or(Vec4::ZERO).to_array(),
                    uv: uvs[index].to_array(),
                });
            }
            let mut flipped_indices: Vec<u32> = indices
                .as_chunks::<3>()
                .0
                .iter()
                .filter(|triangle| {
                    triangle
                        .iter()
                        .map(|&index| rest_xs_m[index as usize])
                        .sum::<f32>()
                        / 3.0
                        >= options.min_x_m
                })
                .flatten()
                .copied()
                .collect();
            if !options.is_animated && rest_matrices.first().is_some_and(|m| m.determinant() < 0.0)
            {
                for triangle in flipped_indices.as_chunks_mut::<3>().0 {
                    triangle.swap(1, 2);
                }
            }
            if tangents.is_none() {
                compute_tangents(&mut primitive_vertices, &flipped_indices);
            }
            builder.vertices.extend(primitive_vertices);
            builder.skin_vertices.extend(skin_vertices);
            builder
                .indices
                .extend(flipped_indices.iter().map(|index| index + base_index));
        }
    }

    let offset: Vec3 = if options.should_recenter && !options.is_animated {
        let center: Vec3 = (bounds_min_m + bounds_max_m) * 0.5;
        Vec3::new(-center.x, -bounds_min_m.y, -center.z)
    } else {
        Vec3::ZERO
    };
    let positions_m: Vec<Vec3> = if options.should_keep_positions {
        builders
            .values()
            .flat_map(|builder| builder.vertices.iter())
            .map(|vertex| Vec3::from(vertex.position) + offset)
            .collect()
    } else {
        Vec::new()
    };
    let mut material_indices: Vec<Option<usize>> = builders.keys().copied().collect();
    material_indices.sort();
    let parts: Vec<ModelPart> = material_indices
        .into_iter()
        .map(|material_index| {
            let builder: MeshBuilder = builders.remove(&material_index).expect("key exists");
            let mut vertices: Vec<Vertex> = builder.vertices;
            for vertex in &mut vertices {
                vertex.position = (Vec3::from(vertex.position) + offset).to_array();
            }
            let mesh: MeshHandle = renderer.create_mesh(&MeshData {
                vertices,
                skin_vertices: options.is_animated.then_some(builder.skin_vertices),
                indices: builder.indices,
            });
            let material: MaterialHandle =
                material_index.map_or(default_material, |index| materials[index]);
            ModelPart {
                mesh,
                material,
                material_index,
            }
        })
        .collect();
    let skeleton: Option<Skeleton> = options.is_animated.then(|| Skeleton {
        parents,
        rest,
        order,
        palette,
        animations: read_animations(&document, &buffers),
    });
    Model {
        parts,
        bounds_min_m: bounds_min_m + offset,
        bounds_max_m: bounds_max_m + offset,
        skeleton,
        material_names: document
            .materials()
            .map(|material| material.name().unwrap_or_default().to_string())
            .collect(),
        positions_m,
    }
}
