//! `world` lays out the arena: an asphalt yard inside concrete walls, a corrugated-iron
//! warehouse with a loading dock, a brick block, shipping containers, and props. It builds the
//! arena's meshes (with world-space UVs) and its collision boxes.
//!
//! Example:
//!     let arena: Arena = Arena::new();
//!     let meshes: Vec<(Surface, MeshData)> = arena.surface_meshes();

use glam::{Vec2, Vec3};

use super::collision::Aabb;
use crate::render::{MeshData, Vertex};

/// `Surface` is one of the arena's PBR materials.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Surface {
    Asphalt,
    ConcreteFloor,
    Brick,
    ConcreteWall,
    Corrugated,
    MetalPlate,
    RustyMetal,
    Pavement,
}

impl Surface {
    /// `ALL` lists every surface, in the order of `asset_manifest::SURFACE_TEXTURES`.
    pub const ALL: [Surface; 8] = [
        Surface::Asphalt,
        Surface::ConcreteFloor,
        Surface::Brick,
        Surface::ConcreteWall,
        Surface::Corrugated,
        Surface::MetalPlate,
        Surface::RustyMetal,
        Surface::Pavement,
    ];

    /// `tile_size_m` is the world size that one repeat of the surface's texture covers.
    fn tile_size_m(self) -> f32 {
        match self {
            Surface::Asphalt => 4.0,
            Surface::ConcreteFloor => 3.0,
            Surface::Brick => 2.5,
            Surface::ConcreteWall => 3.0,
            Surface::Corrugated => 2.0,
            Surface::MetalPlate => 2.0,
            Surface::RustyMetal => 2.5,
            Surface::Pavement => 2.0,
        }
    }
}

/// `WorldBox` is one solid box of the arena and its surface.
#[derive(Clone, Copy, Debug)]
pub struct WorldBox {
    pub aabb: Aabb,
    pub surface: Surface,
}

/// `PropPlacement` is one placed prop model.
#[derive(Clone, Copy, Debug)]
pub struct PropPlacement {
    pub model_id: &'static str,
    pub position_m: Vec3,
    pub yaw_rad: f32,
    pub is_solid: bool,
}

/// `LightPlacement` is one point light and its color (in multiples of the light scale).
#[derive(Clone, Copy, Debug)]
pub struct LightPlacement {
    pub position_m: Vec3,
    pub radius_m: f32,
    pub color_rgb: Vec3,
}

/// `Arena` is the whole level.
pub struct Arena {
    pub boxes: Vec<WorldBox>,
    pub props: Vec<PropPlacement>,
    pub lights: Vec<LightPlacement>,
    pub player_spawns: Vec<(Vec3, f32)>,
    pub bot_spawns: Vec<Vec3>,
    pub bounds_min_m: Vec3,
    pub bounds_max_m: Vec3,
}

/// `HALF_SIZE_M` is half the width of the walled yard.
const HALF_SIZE_M: f32 = 34.0;
/// `PERIMETER_HEIGHT_M` is the height of the yard's walls.
const PERIMETER_HEIGHT_M: f32 = 5.0;
/// `WAREHOUSE_HEIGHT_M` is the height of the warehouse's walls.
const WAREHOUSE_HEIGHT_M: f32 = 6.5;
/// `CONTAINER_SIZE_M` is the size of a 20-foot shipping container.
const CONTAINER_SIZE_M: Vec3 = Vec3::new(6.06, 2.59, 2.44);
/// `DOOR_HEIGHT_M` is the height of the warehouse doors.
const DOOR_HEIGHT_M: f32 = 3.6;

/// `boxed` makes a world box from its corners.
fn boxed(min: [f32; 3], max: [f32; 3], surface: Surface) -> WorldBox {
    WorldBox {
        aabb: Aabb {
            min_m: Vec3::from(min),
            max_m: Vec3::from(max),
        },
        surface,
    }
}

/// `container` makes a shipping container at a floor center, long along x or z.
fn container(center: Vec2, base_y_m: f32, is_along_x: bool) -> WorldBox {
    let size: Vec3 = if is_along_x {
        CONTAINER_SIZE_M
    } else {
        Vec3::new(CONTAINER_SIZE_M.z, CONTAINER_SIZE_M.y, CONTAINER_SIZE_M.x)
    };
    let min: Vec3 = Vec3::new(center.x - size.x * 0.5, base_y_m, center.y - size.z * 0.5);
    WorldBox {
        aabb: Aabb {
            min_m: min,
            max_m: min + size,
        },
        surface: Surface::RustyMetal,
    }
}

/// `prop` makes a prop placement.
fn prop(model_id: &'static str, x: f32, z: f32, yaw_rad: f32, is_solid: bool) -> PropPlacement {
    PropPlacement {
        model_id,
        position_m: Vec3::new(x, 0.0, z),
        yaw_rad,
        is_solid,
    }
}

impl Arena {
    /// `new` builds the arena.
    ///
    /// Example:
    ///     let arena: Arena = Arena::new();
    pub fn new() -> Arena {
        let h: f32 = HALF_SIZE_M;
        let wall: f32 = 1.0;
        let mut boxes: Vec<WorldBox> = vec![
            boxed(
                [-h - 30.0, -1.0, -h - 30.0],
                [h + 30.0, 0.0, h + 30.0],
                Surface::Asphalt,
            ),
            boxed(
                [-h - wall, 0.0, -h - wall],
                [h + wall, PERIMETER_HEIGHT_M, -h],
                Surface::ConcreteWall,
            ),
            boxed(
                [-h - wall, 0.0, h],
                [h + wall, PERIMETER_HEIGHT_M, h + wall],
                Surface::ConcreteWall,
            ),
            boxed(
                [-h - wall, 0.0, -h],
                [-h, PERIMETER_HEIGHT_M, h],
                Surface::ConcreteWall,
            ),
            boxed(
                [h, 0.0, -h],
                [h + wall, PERIMETER_HEIGHT_M, h],
                Surface::ConcreteWall,
            ),
        ];

        let (wx0, wx1, wz0, wz1) = (-28.0_f32, -6.0_f32, -28.0_f32, -6.0_f32);
        let t: f32 = 0.4;
        let top: f32 = WAREHOUSE_HEIGHT_M;
        boxes.extend([
            boxed([wx0, 0.0, wz0], [wx1, 0.06, wz1], Surface::ConcreteFloor),
            boxed([wx0, 0.0, wz0], [wx1, top, wz0 + t], Surface::Corrugated),
            boxed([wx0, 0.0, wz0], [wx0 + t, top, wz1], Surface::Corrugated),
            boxed([wx0, 0.0, wz1 - t], [-22.0, top, wz1], Surface::Corrugated),
            boxed([-14.0, 0.0, wz1 - t], [wx1, top, wz1], Surface::Corrugated),
            boxed(
                [-22.0, DOOR_HEIGHT_M + 1.0, wz1 - t],
                [-14.0, top, wz1],
                Surface::Corrugated,
            ),
            boxed([wx1 - t, 0.0, wz0], [wx1, top, -19.0], Surface::Corrugated),
            boxed([wx1 - t, 0.0, -15.0], [wx1, top, wz1], Surface::Corrugated),
            boxed(
                [wx1 - t, DOOR_HEIGHT_M, -19.0],
                [wx1, top, -15.0],
                Surface::Corrugated,
            ),
            boxed(
                [wx0, top, wz0],
                [wx1, top + 0.35, -19.5],
                Surface::MetalPlate,
            ),
            boxed(
                [wx0, top, -16.5],
                [wx1, top + 0.35, wz1],
                Surface::MetalPlate,
            ),
            boxed(
                [-22.0, 0.0, -22.5],
                [-19.0, 1.2, -21.0],
                Surface::MetalPlate,
            ),
            boxed([-13.0, 0.0, -25.0], [-9.0, 2.4, -23.6], Surface::RustyMetal),
            boxed([-26.0, 0.0, -12.0], [-24.6, 1.6, -8.0], Surface::MetalPlate),
        ]);

        let (dx0, dx1, dz0, dz1) = (-6.0_f32, 0.0_f32, -27.0_f32, -21.0_f32);
        boxes.push(boxed(
            [dx0, 0.0, dz0],
            [dx1, 1.2, dz1],
            Surface::ConcreteWall,
        ));
        for step in 0..3 {
            let step_height_m: f32 = 0.3 * (step + 1) as f32;
            let step_x_m: f32 = dx1 + 0.4 * (3 - step) as f32;
            boxes.push(boxed(
                [dx1, 0.0, dz0 + 1.5],
                [step_x_m, step_height_m, dz0 + 4.0],
                Surface::ConcreteWall,
            ));
        }

        let (bx0, bx1, bz0, bz1) = (10.0_f32, 26.0_f32, 12.0_f32, 26.0_f32);
        boxes.extend([
            boxed(
                [bx0 - 2.0, 0.0, bz0 - 2.0],
                [bx1, 0.15, bz0],
                Surface::Pavement,
            ),
            boxed([bx0 - 2.0, 0.0, bz0], [bx0, 0.15, bz1], Surface::Pavement),
            boxed([bx0, 0.0, bz0], [bx1, 9.0, bz1], Surface::Brick),
            boxed(
                [bx0 + 2.0, 9.0, bz0 + 2.0],
                [bx0 + 6.0, 10.5, bz0 + 5.0],
                Surface::MetalPlate,
            ),
            boxed([-4.0, 0.0, 6.0], [4.0, 1.3, 6.6], Surface::Brick),
            boxed([-12.0, 0.0, 14.0], [-11.4, 1.3, 22.0], Surface::Brick),
            boxed([18.0, 0.0, -2.0], [24.0, 1.3, -1.4], Surface::Brick),
        ]);

        boxes.extend([
            container(Vec2::new(8.0, -20.0), 0.0, true),
            container(Vec2::new(8.0, -20.0), CONTAINER_SIZE_M.y, true),
            container(Vec2::new(8.6, -16.8), 0.0, true),
            container(Vec2::new(24.0, -24.0), 0.0, false),
            container(Vec2::new(-20.0, 8.0), 0.0, true),
            container(Vec2::new(-24.0, 20.0), 0.0, false),
            container(Vec2::new(28.0, 6.0), 0.0, false),
            container(Vec2::new(0.0, 22.0), 0.0, true),
        ]);

        let props: Vec<PropPlacement> = vec![
            prop("concrete_road_barrier", 2.0, -6.0, 0.1, true),
            prop("concrete_road_barrier", 5.5, -5.0, -0.3, true),
            prop("concrete_road_barrier", -3.0, 12.0, 1.4, true),
            prop("concrete_road_barrier", 14.0, 4.0, 0.0, true),
            prop("concrete_road_barrier", 20.0, -12.0, 1.57, true),
            prop("concrete_road_barrier", -14.0, 2.0, 0.6, true),
            prop("Barrel_01", -8.0, -4.0, 0.0, true),
            prop("Barrel_01", -7.2, -3.4, 1.0, true),
            prop("barrel_03", -26.5, -26.5, 0.4, true),
            prop("barrel_03", -25.6, -26.8, 2.0, true),
            prop("barrel_03", 12.4, -22.4, 0.0, true),
            prop("old_military_crate", -16.0, -24.0, 0.2, true),
            prop("old_military_crate", -18.0, -12.0, 1.2, true),
            prop("wooden_crate_02", -10.0, -10.0, 0.3, true),
            prop("wooden_crate_02", -10.2, -8.6, -0.2, true),
            prop("wooden_crate_02", 4.0, 18.0, 0.7, true),
            prop("metal_trash_can", 9.0, 9.5, 0.0, true),
            prop("metal_trash_can", 7.6, 10.4, 1.0, true),
            prop("utility_box_01", 9.6, 18.0, -1.57, true),
            prop("fire_hydrant", 7.0, 13.0, 0.5, true),
            prop("street_lamp_01", 7.0, 2.0, 0.0, true),
            prop("street_lamp_01", -4.0, -1.0, std::f32::consts::PI, true),
            prop("street_lamp_01", 16.0, -8.0, 1.57, true),
            prop("exterior_aircon_unit", 9.6, 0.0, -1.57, false),
            prop("old_tyre", 22.0, -20.0, 0.0, false),
            prop("old_tyre", -2.0, 26.0, 0.0, false),
            prop("portable_generator", -4.0, -24.0, 0.8, true),
            prop("propane_tank", -9.0, -26.0, 0.0, true),
            prop("propane_tank", -8.2, -26.2, 0.0, true),
            prop("cardboard_box_01", -12.0, -19.0, 0.4, false),
            prop("cardboard_box_01", -20.0, -24.0, 1.1, false),
            prop("cardboard_box_01", 26.0, 9.0, 0.2, false),
        ];

        let fluorescent: Vec3 = Vec3::new(1.0, 0.95, 0.85);
        let sodium: Vec3 = Vec3::new(1.0, 0.62, 0.3);
        let mut lights: Vec<LightPlacement> = Vec::new();
        for x in [-23.0, -11.0] {
            for z in [-23.0, -11.0] {
                lights.push(LightPlacement {
                    position_m: Vec3::new(x, top - 0.6, z),
                    radius_m: 14.0,
                    color_rgb: fluorescent,
                });
            }
        }
        Arena {
            boxes,
            props,
            lights,
            player_spawns: vec![
                (Vec3::new(20.0, 0.0, -6.0), 2.4),
                (Vec3::new(-2.0, 0.0, 30.0), 0.0),
                (Vec3::new(30.0, 0.0, 28.0), 0.8),
            ],
            bot_spawns: vec![
                Vec3::new(-18.0, 0.06, -18.0),
                Vec3::new(-28.0, 0.0, 28.0),
                Vec3::new(-3.0, 1.2, -24.0),
                Vec3::new(30.0, 0.0, -30.0),
                Vec3::new(-30.0, 0.0, -2.0),
                Vec3::new(4.0, 0.0, 30.0),
            ],
            bounds_min_m: Vec3::new(-h - wall, -1.0, -h - wall),
            bounds_max_m: Vec3::new(h + wall, 16.0, h + wall),
        }
        .with_street_lights(sodium)
    }

    /// `with_street_lights` adds a sodium lamp at the head of every street-lamp prop.
    fn with_street_lights(mut self, color_rgb: Vec3) -> Arena {
        for placement in self.props.iter().filter(|p| p.model_id == "street_lamp_01") {
            self.lights.push(LightPlacement {
                position_m: placement.position_m + Vec3::Y * 5.2,
                radius_m: 16.0,
                color_rgb,
            });
        }
        self
    }

    /// `surface_meshes` builds one mesh per surface from the arena's boxes.
    ///
    /// Example:
    ///     for (surface, mesh) in arena.surface_meshes() { ... }
    pub fn surface_meshes(&self) -> Vec<(Surface, MeshData)> {
        Surface::ALL
            .iter()
            .filter_map(|surface: &Surface| {
                let mut mesh: MeshData = MeshData {
                    vertices: Vec::new(),
                    skin_vertices: None,
                    indices: Vec::new(),
                };
                for world_box in self.boxes.iter().filter(|b| b.surface == *surface) {
                    append_box(&mut mesh, &world_box.aabb, surface.tile_size_m());
                }
                (!mesh.indices.is_empty()).then_some((*surface, mesh))
            })
            .collect()
    }
}

/// `append_box` adds a box's six faces, wound counter-clockwise from outside, with UVs in
/// world space so that textures line up across boxes.
fn append_box(mesh: &mut MeshData, aabb: &Aabb, tile_size_m: f32) {
    let axes: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
    for (axis, axis_direction) in axes.iter().enumerate() {
        for sign in [-1.0_f32, 1.0] {
            let normal: Vec3 = *axis_direction * sign;
            let (u_axis, v_axis) = match axis {
                0 => (Vec3::Z * -sign, Vec3::NEG_Y),
                1 => (Vec3::X, Vec3::Z * sign),
                _ => (Vec3::X * sign, Vec3::NEG_Y),
            };
            let center: Vec3 = aabb.center() + normal * (aabb.max_m - aabb.min_m)[axis] * 0.5;
            let half: Vec3 = (aabb.max_m - aabb.min_m) * 0.5;
            let u_half: f32 = half.dot(u_axis.abs());
            let v_half: f32 = half.dot(v_axis.abs());
            let corners: [Vec2; 4] = [
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, -1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(-1.0, 1.0),
            ];
            let base: u32 = mesh.vertices.len() as u32;
            for corner in corners {
                let position: Vec3 =
                    center + u_axis * corner.x * u_half + v_axis * corner.y * v_half;
                let uv: Vec2 = Vec2::new(position.dot(u_axis), position.dot(v_axis)) / tile_size_m;
                let handedness: f32 = if normal.cross(u_axis).dot(v_axis) < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                mesh.vertices.push(Vertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                    tangent: u_axis.extend(handedness).to_array(),
                    uv: uv.to_array(),
                });
            }
            let is_ccw: bool = u_axis.cross(v_axis).dot(normal) > 0.0;
            let order: [u32; 6] = if is_ccw {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            mesh.indices.extend(order.iter().map(|index| base + index));
        }
    }
}

/// `tests` checks the arena's box meshes.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | box face | each of the six faces | `test_box_faces_wind_outward` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_box_faces_wind_outward` checks every triangle faces along its vertex normal.
    #[test]
    fn test_box_faces_wind_outward() {
        let mut mesh: MeshData = MeshData {
            vertices: Vec::new(),
            skin_vertices: None,
            indices: Vec::new(),
        };
        append_box(&mut mesh, &Aabb::from_center(Vec3::ZERO, Vec3::ONE), 1.0);
        assert_eq!(mesh.indices.len(), 36);
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let p: Vec<Vec3> = triangle
                .iter()
                .map(|index| Vec3::from(mesh.vertices[*index as usize].position))
                .collect();
            let face_normal: Vec3 = (p[1] - p[0]).cross(p[2] - p[0]);
            let normal: Vec3 = Vec3::from(mesh.vertices[triangle[0] as usize].normal);
            assert!(face_normal.dot(normal) > 0.0);
        }
    }
}
