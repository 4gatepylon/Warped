//! `avatar_model.rs` is the players' body, W-106 (Teal robot player model): the skinned glTF model
//! `teal_robot` of `common/maps/models/` with its clips, posed on the CPU into the triangles and
//! BVH of a `MeshModel` for `render_voxel.metal` to ray-trace. The BVH is built once, on the first
//! idle pose, and only refitted for each pose.
//!
//! Interface: `AvatarModel::Load`, `AvatarModel`, `AvatarClip`, `BODY_MODEL`.
//!
//! Example: `let posed = AvatarModel::Load(BODY_MODEL, 2.0)?.make_posed(AvatarClip::Walk, 0.3);`
//!
//! Must know:
//! - A posed model is in its structure's chart, in meters: feet at the origin, up +y, facing -z
//!   (the way a player looks), and its triangles' materials index `get_materials`.
//! - The ray tracer draws every triangle opaque, so `Load` leaves out see-through (`BLEND`)
//!   primitives: the robot's face glass, which would hide its glowing eyes.

use std::path::{Path, PathBuf};

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use gltf::animation::Property;
use gltf::animation::util::ReadOutputs;

use crate::fps_demo_assets::import_gltf;
use crate::map::get_model_path;
use crate::mesh::{MeshMaterial, MeshModel, MeshTriangle, read_materials};

/// `BODY_MODEL` is the folder of the players' body model in `common/maps/models/`.
pub const BODY_MODEL: &str = "teal_robot";
const JOINTS_PER_VERTEX: usize = 4;

/// `AvatarClip` is one of the body's animations, named as in the model.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AvatarClip {
    Idle,
    Walk,
    Run,
    Jump,
    Death,
}

/// `ALL_CLIPS` lists every clip, in the order `AvatarModel` keeps them.
const ALL_CLIPS: [AvatarClip; 5] = [
    AvatarClip::Idle,
    AvatarClip::Walk,
    AvatarClip::Run,
    AvatarClip::Jump,
    AvatarClip::Death,
];

impl AvatarClip {
    /// `get_name` gives the clip's animation name in the model.
    ///
    /// Example: `assert_eq!(AvatarClip::Walk.get_name(), "Walk");`
    fn get_name(self) -> &'static str {
        match self {
            AvatarClip::Idle => "Idle",
            AvatarClip::Walk => "Walk",
            AvatarClip::Run => "Run",
            AvatarClip::Jump => "Jump",
            AvatarClip::Death => "Death",
        }
    }

    /// `is_looping` says whether the clip repeats (idle, walk, run) rather than holding its last
    /// pose (jump, death).
    ///
    /// Example: `if clip.is_looping() { ... }`
    pub fn is_looping(self) -> bool {
        matches!(self, AvatarClip::Idle | AvatarClip::Walk | AvatarClip::Run)
    }
}

/// `Channel` is one animated property of one node: key times and values (a translation or scale
/// in xyz, a rotation as a quaternion xyzw), linearly interpolated.
struct Channel {
    node: usize,
    property: Property,
    times_s: Vec<f32>,
    values: Vec<Vec4>,
}

/// `Clip` is one animation: its channels and length.
struct Clip {
    channels: Vec<Channel>,
    duration_s: f32,
}

/// `SkinVertex` is one vertex in the model's own space: position, normal, and up to four joints
/// (indices into the skin) with their weights.
#[derive(Clone, Copy)]
struct SkinVertex {
    p: Vec3,
    n: Vec3,
    joints: [usize; JOINTS_PER_VERTEX],
    weights: [f32; JOINTS_PER_VERTEX],
}

/// `AvatarModel` is the loaded body: the node tree with each node's rest transform, the skin, the
/// vertices, the clips, and the placed model whose triangles each pose moves.
pub struct AvatarModel {
    parents: Vec<Option<usize>>,
    parent_first: Vec<usize>,
    rest: Vec<(Vec3, Quat, Vec3)>,
    joints: Vec<usize>,
    inverse_binds: Vec<Mat4>,
    vertices: Vec<SkinVertex>,
    corners: Vec<[usize; 3]>,
    clips: Vec<Clip>,
    to_chart: Mat4,
    placed: MeshModel,
}

/// `read_clip` reads the animation `name` of `document`.
///
/// Example: `let clip = read_clip(&document, &buffers, "Walk")?;`
fn read_clip(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    name: &str,
) -> Result<Clip, String> {
    let animation = document
        .animations()
        .find(|a| a.name() == Some(name))
        .ok_or(format!("avatar_model: the model has no clip {name}"))?;
    let mut channels: Vec<Channel> = Vec::new();
    for channel in animation.channels() {
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
        let times_s: Vec<f32> = reader
            .read_inputs()
            .ok_or(format!(
                "avatar_model: clip {name} has a channel without times"
            ))?
            .collect();
        let values: Vec<Vec4> = match reader.read_outputs() {
            Some(ReadOutputs::Translations(t)) => t.map(|v| Vec3::from(v).extend(0.0)).collect(),
            Some(ReadOutputs::Scales(s)) => s.map(|v| Vec3::from(v).extend(0.0)).collect(),
            Some(ReadOutputs::Rotations(r)) => r.into_f32().map(Vec4::from).collect(),
            _ => return Err(format!("avatar_model: clip {name} animates morph weights")),
        };
        channels.push(Channel {
            node: channel.target().node().index(),
            property: channel.target().property(),
            times_s,
            values,
        });
    }
    let duration_s: f32 = channels
        .iter()
        .filter_map(|c| c.times_s.last().copied())
        .fold(0.0, f32::max);
    Ok(Clip {
        channels,
        duration_s,
    })
}

/// `sample_channel` gives channel c's value at `time_s` (held before its first key and after its
/// last), interpolating a rotation along the shorter arc.
///
/// Example: `let v = sample_channel(&channel, 0.4);`
fn sample_channel(c: &Channel, time_s: f32) -> Vec4 {
    let after: usize = c.times_s.partition_point(|&t| t <= time_s);
    if after == 0 || after == c.times_s.len() {
        return c.values[after.saturating_sub(1)];
    }
    let (t0, t1) = (c.times_s[after - 1], c.times_s[after]);
    let f: f32 = (time_s - t0) / (t1 - t0);
    let (a, b) = (c.values[after - 1], c.values[after]);
    match c.property {
        Property::Rotation => Vec4::from(Quat::from_vec4(a).slerp(Quat::from_vec4(b), f)),
        _ => a.lerp(b, f),
    }
}

impl AvatarModel {
    /// `Load` reads model `model` (skinned, with every `AvatarClip`), scales it evenly so its first
    /// idle pose is `height_m` tall with its feet at the origin, turns it to face -z, and builds
    /// its BVH on that pose.
    ///
    /// Example: `let body = AvatarModel::Load(BODY_MODEL, 2.0)?;`
    #[allow(non_snake_case)]
    pub fn Load(model: &str, height_m: f64) -> Result<AvatarModel, String> {
        let path: PathBuf = get_model_path(model);
        let folder: &Path = path.parent().expect("a file has a folder");
        let (document, buffers) = import_gltf(&path).map_err(|e| format!("avatar_model: {e}"))?;
        let mut parents: Vec<Option<usize>> = vec![None; document.nodes().len()];
        for node in document.nodes() {
            node.children()
                .for_each(|child| parents[child.index()] = Some(node.index()));
        }
        let mut parent_first: Vec<usize> = (0..parents.len()).collect();
        let depth = |mut k: usize| -> usize {
            let mut d: usize = 0;
            while let Some(parent) = parents[k] {
                (k, d) = (parent, d + 1);
            }
            d
        };
        parent_first.sort_by_key(|&k| depth(k));
        let rest: Vec<(Vec3, Quat, Vec3)> = document
            .nodes()
            .map(|node| {
                let (t, r, s) = node.transform().decomposed();
                (Vec3::from(t), Quat::from_array(r), Vec3::from(s))
            })
            .collect();
        let skin = document
            .skins()
            .next()
            .ok_or(format!("avatar_model: {model} has no skin"))?;
        let joints: Vec<usize> = skin.joints().map(|joint| joint.index()).collect();
        let inverse_binds: Vec<Mat4> = skin
            .reader(|buffer| Some(&buffers[buffer.index()]))
            .read_inverse_bind_matrices()
            .ok_or(format!(
                "avatar_model: {model}'s skin has no inverse bind matrices"
            ))?
            .map(|m| Mat4::from_cols_array_2d(&m))
            .collect();
        let (mut vertices, mut corners, mut uvs, mut materials) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for node in document.nodes().filter(|node| node.skin().is_some()) {
            let mesh = node
                .mesh()
                .ok_or("avatar_model: a skinned node has no mesh".to_string())?;
            for primitive in mesh
                .primitives()
                .filter(|p| p.material().alpha_mode() != gltf::material::AlphaMode::Blend)
            {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                let first: usize = vertices.len();
                let missing =
                    |what: &str| format!("avatar_model: {model} has a primitive without {what}");
                let positions = reader.read_positions().ok_or(missing("positions"))?;
                let normals = reader.read_normals().ok_or(missing("normals"))?;
                let skin_joints = reader.read_joints(0).ok_or(missing("joints"))?.into_u16();
                let weights = reader.read_weights(0).ok_or(missing("weights"))?.into_f32();
                for (((p, n), j), w) in positions.zip(normals).zip(skin_joints).zip(weights) {
                    vertices.push(SkinVertex {
                        p: Vec3::from(p),
                        n: Vec3::from(n),
                        joints: j.map(usize::from),
                        weights: w,
                    });
                }
                uvs.extend(
                    reader
                        .read_tex_coords(0)
                        .ok_or(missing("texture coordinates"))?
                        .into_f32()
                        .map(Vec2::from),
                );
                let indices: Vec<u32> = reader
                    .read_indices()
                    .ok_or(missing("indices"))?
                    .into_u32()
                    .collect();
                for triangle in indices.as_chunks::<3>().0 {
                    corners.push(triangle.map(|i| first + i as usize));
                    materials.push(primitive.material().index().unwrap_or(0));
                }
            }
        }
        let clips: Vec<Clip> = ALL_CLIPS
            .iter()
            .map(|clip| read_clip(&document, &buffers, clip.get_name()))
            .collect::<Result<_, String>>()?;
        let mut body: AvatarModel = AvatarModel {
            parents,
            parent_first,
            rest,
            joints,
            inverse_binds,
            vertices,
            corners,
            clips,
            to_chart: Mat4::from_rotation_y(std::f32::consts::PI),
            placed: MeshModel::FromTriangles(Vec::new(), Vec::new()).0,
        };
        let (positions, _) = body.skin(AvatarClip::Idle, 0.0);
        let (lo, hi) = positions
            .iter()
            .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), &p| {
                (lo.min(p), hi.max(p))
            });
        let scale: f32 = height_m as f32 / (hi.y - lo.y);
        body.to_chart = Mat4::from_scale(Vec3::splat(scale))
            * Mat4::from_translation(Vec3::new(0.0, -lo.y, 0.0))
            * body.to_chart;
        let (positions, normals) = body.skin(AvatarClip::Idle, 0.0);
        let triangles: Vec<MeshTriangle> = body
            .corners
            .iter()
            .zip(&materials)
            .map(|(c, &material)| MeshTriangle {
                p: c.map(|i| positions[i]),
                n: c.map(|i| normals[i]),
                uv: c.map(|i| uvs[i]),
                material,
            })
            .collect();
        let (placed, order) =
            MeshModel::FromTriangles(triangles, read_materials(&document, folder, model)?);
        body.corners = order.iter().map(|&k| body.corners[k]).collect();
        body.placed = placed;
        Ok(body)
    }

    /// `get_materials` gives the materials the posed triangles index.
    ///
    /// Example: `let materials = body.get_materials();`
    pub fn get_materials(&self) -> &[MeshMaterial] {
        &self.placed.materials
    }

    /// `get_duration_s` gives the length of clip `clip` in seconds.
    ///
    /// Example: `let loop_s = body.get_duration_s(AvatarClip::Walk);`
    pub fn get_duration_s(&self, clip: AvatarClip) -> f64 {
        f64::from(self.get_clip(clip).duration_s)
    }

    /// `get_clip` gives clip `clip`'s keys.
    ///
    /// Example: `let keys = body.get_clip(AvatarClip::Walk);`
    fn get_clip(&self, clip: AvatarClip) -> &Clip {
        &self.clips[ALL_CLIPS
            .iter()
            .position(|&c| c == clip)
            .expect("every clip is listed")]
    }

    /// `get_joint_matrices` gives each skin joint's matrix (its node's transform times its inverse
    /// bind matrix) in clip `clip` at `time_s`, wrapped for a looping clip and held at the ends
    /// otherwise.
    ///
    /// Example: `let matrices = body.get_joint_matrices(AvatarClip::Run, 0.25);`
    fn get_joint_matrices(&self, clip: AvatarClip, time_s: f64) -> Vec<Mat4> {
        let clip_data: &Clip = self.get_clip(clip);
        let t_s: f32 = if clip.is_looping() {
            (time_s as f32).rem_euclid(clip_data.duration_s)
        } else {
            (time_s as f32).clamp(0.0, clip_data.duration_s)
        };
        let mut locals: Vec<(Vec3, Quat, Vec3)> = self.rest.clone();
        for channel in &clip_data.channels {
            let value: Vec4 = sample_channel(channel, t_s);
            let local: &mut (Vec3, Quat, Vec3) = &mut locals[channel.node];
            match channel.property {
                Property::Translation => local.0 = value.truncate(),
                Property::Rotation => local.1 = Quat::from_vec4(value).normalize(),
                Property::Scale => local.2 = value.truncate(),
                Property::MorphTargetWeights => unreachable!("read_clip rejects morph weights"),
            }
        }
        let mut globals: Vec<Mat4> = vec![Mat4::IDENTITY; self.rest.len()];
        for &k in &self.parent_first {
            let (t, r, s) = locals[k];
            let local: Mat4 = Mat4::from_scale_rotation_translation(s, r, t);
            globals[k] = self.parents[k].map_or(local, |parent| globals[parent] * local);
        }
        self.joints
            .iter()
            .zip(&self.inverse_binds)
            .map(|(&joint, inverse_bind)| self.to_chart * globals[joint] * *inverse_bind)
            .collect()
    }

    /// `skin` gives every vertex's position and normal in the chart, in clip `clip` at `time_s`.
    ///
    /// Example: `let (positions, normals) = body.skin(AvatarClip::Idle, 0.0);`
    fn skin(&self, clip: AvatarClip, time_s: f64) -> (Vec<Vec3>, Vec<Vec3>) {
        let matrices: Vec<Mat4> = self.get_joint_matrices(clip, time_s);
        self.vertices
            .iter()
            .map(|v| {
                let blend: Mat4 = (0..JOINTS_PER_VERTEX)
                    .map(|i| matrices[v.joints[i]] * v.weights[i])
                    .fold(Mat4::ZERO, |sum, m| sum + m);
                (
                    blend.transform_point3(v.p),
                    blend.transform_vector3(v.n).normalize_or_zero(),
                )
            })
            .unzip()
    }

    /// `make_posed` gives the body in clip `clip` at `time_s`: the placed model's triangles moved
    /// to the pose and its BVH refitted around them.
    ///
    /// Example: `let posed = body.make_posed(AvatarClip::Walk, 0.3);`
    pub fn make_posed(&self, clip: AvatarClip, time_s: f64) -> MeshModel {
        let (positions, normals) = self.skin(clip, time_s);
        let mut posed: MeshModel = MeshModel {
            triangles: self.placed.triangles.clone(),
            nodes: self.placed.nodes.clone(),
            materials: Vec::new(),
        };
        for (triangle, c) in posed.triangles.iter_mut().zip(&self.corners) {
            triangle.p = c.map(|i| positions[i]);
            triangle.n = c.map(|i| normals[i]);
        }
        posed.refit();
        posed
    }
}

#[cfg(test)]
mod tests {
    //! Tests for `avatar_model.rs` on the Teal robot scaled to 2 m.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Clip | idle, walk, run, jump, death | `every_clip_poses_inside_its_boxes` |
    //! | Time in a looping clip | within the first loop, a whole loop later | `looping_clips_repeat` |
    //! | Time in a held clip | before the end, after the end (held) | `held_clips_end_on_their_last_pose` |
    //! | Pose's shape | standing (2 m tall, feet on y = 0, facing -z), lying (death's end, low) | `the_body_stands_and_lies` |

    use super::*;

    const HEIGHT_M: f64 = 2.0;
    const TOLERANCE_M: f32 = 1e-3;
    const MAX_LYING_HEIGHT_M: f32 = 0.6;

    /// `get_bounds` gives the box around a posed model's triangles.
    ///
    /// Example: `let (lo, hi) = get_bounds(&posed);`
    fn get_bounds(posed: &MeshModel) -> (Vec3, Vec3) {
        posed
            .triangles
            .iter()
            .flat_map(|t| t.p)
            .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)))
    }

    #[test]
    fn every_clip_poses_inside_its_boxes() {
        let body: AvatarModel = AvatarModel::Load(BODY_MODEL, HEIGHT_M).expect("the body");
        for clip in ALL_CLIPS {
            let posed: MeshModel = body.make_posed(clip, 0.3);
            for node in posed.nodes.iter().filter(|node| node.count > 0) {
                for t in &posed.triangles[node.first..node.first + node.count] {
                    for p in t.p {
                        assert!(
                            p.cmpge(node.lo - TOLERANCE_M).all()
                                && p.cmple(node.hi + TOLERANCE_M).all(),
                            "{clip:?}: a corner at {p} is outside its leaf"
                        );
                    }
                }
            }
            let (lo, hi) = get_bounds(&posed);
            assert!(
                posed.nodes[0].lo.abs_diff_eq(lo, TOLERANCE_M)
                    && posed.nodes[0].hi.abs_diff_eq(hi, TOLERANCE_M)
            );
        }
    }

    #[test]
    fn looping_clips_repeat() {
        let body: AvatarModel = AvatarModel::Load(BODY_MODEL, HEIGHT_M).expect("the body");
        for clip in [AvatarClip::Idle, AvatarClip::Walk, AvatarClip::Run] {
            let duration_s: f64 = body.get_duration_s(clip);
            let (a, b) = (
                body.make_posed(clip, 0.2),
                body.make_posed(clip, 0.2 + duration_s),
            );
            let gap: f32 = a
                .triangles
                .iter()
                .zip(&b.triangles)
                .map(|(s, t)| (s.p[0] - t.p[0]).length())
                .fold(0.0, f32::max);
            assert!(gap < TOLERANCE_M, "{clip:?} moves {gap} m over a loop");
        }
    }

    #[test]
    fn held_clips_end_on_their_last_pose() {
        let body: AvatarModel = AvatarModel::Load(BODY_MODEL, HEIGHT_M).expect("the body");
        for clip in [AvatarClip::Jump, AvatarClip::Death] {
            let (early, end, late) = (
                body.make_posed(clip, 0.1),
                body.make_posed(clip, 10.0),
                body.make_posed(clip, 20.0),
            );
            let moved: f32 = early
                .triangles
                .iter()
                .zip(&end.triangles)
                .map(|(s, t)| (s.p[0] - t.p[0]).length())
                .fold(0.0, f32::max);
            let held: f32 = end
                .triangles
                .iter()
                .zip(&late.triangles)
                .map(|(s, t)| (s.p[0] - t.p[0]).length())
                .fold(0.0, f32::max);
            assert!(
                moved > 0.1 && held < TOLERANCE_M,
                "{clip:?}: moved {moved} m, then {held} m"
            );
        }
    }

    #[test]
    fn the_body_stands_and_lies() {
        let body: AvatarModel = AvatarModel::Load(BODY_MODEL, HEIGHT_M).expect("the body");
        let standing: MeshModel = body.make_posed(AvatarClip::Idle, 0.0);
        let (lo, hi) = get_bounds(&standing);
        assert!((hi.y - lo.y - HEIGHT_M as f32).abs() < TOLERANCE_M && lo.y.abs() < TOLERANCE_M);
        let glow: usize = body
            .get_materials()
            .iter()
            .position(|m| m.emissive.iter().sum::<f32>() > 0.0)
            .expect("the glowing eyes");
        let eyes_z_m: Vec<f32> = standing
            .triangles
            .iter()
            .filter(|t| t.material == glow && t.p[0].y > 0.8 * HEIGHT_M as f32)
            .map(|t| t.p[0].z)
            .collect();
        assert!(
            !eyes_z_m.is_empty() && eyes_z_m.iter().sum::<f32>() < 0.0,
            "the eyes look toward +z"
        );
        let lying: MeshModel = body.make_posed(AvatarClip::Death, 10.0);
        let (lo, hi) = get_bounds(&lying);
        assert!(
            hi.y - lo.y < MAX_LYING_HEIGHT_M,
            "dead, the body is {} m tall",
            hi.y - lo.y
        );
    }
}
