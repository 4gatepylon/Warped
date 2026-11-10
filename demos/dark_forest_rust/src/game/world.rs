//! `world` turns a portable map (`common/maps/dark_forest/map.json`) into the arena: its
//! meshes (boxes, tree trunks, canopies, logs and rope ladders, with world-space UVs), its
//! collision boxes, its ladders, its pylon consoles, its lights and its spawns.
//!
//! Example:
//!     let arena: Arena = Arena::from_map(&map::load(&path)?);
//!     let meshes: Vec<(Surface, MeshData)> = arena.surface_meshes();

use crate::game::warps::Warps;
use glam::{Mat3, Mat4, Vec2, Vec3};

use super::collision::Aabb;
use crate::map::{self, MapFile};
use crate::render::{MeshData, Vertex};

/// `Surface` is one of the arena's PBR materials.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Surface {
    Ground,
    Bark,
    Planks,
    Leaves,
    Rock,
    Rope,
    RedPaint,
    BluePaint,
    RedConsole,
    BlueConsole,
    Horizon,
    Accretion,
    Throat,
    Wax,
    Flame,
}

impl Surface {
    /// `ALL` lists every surface; the first `TEXTURED_COUNT` have their own texture sets, in
    /// the order of `asset_manifest::SURFACE_TEXTURES`.
    pub const ALL: [Surface; 15] = [
        Surface::Ground,
        Surface::Bark,
        Surface::Planks,
        Surface::Leaves,
        Surface::Rock,
        Surface::Rope,
        Surface::RedPaint,
        Surface::BluePaint,
        Surface::RedConsole,
        Surface::BlueConsole,
        Surface::Horizon,
        Surface::Accretion,
        Surface::Throat,
        Surface::Wax,
        Surface::Flame,
    ];
    /// `TEXTURED_COUNT` is how many surfaces have their own texture sets.
    pub const TEXTURED_COUNT: usize = 5;

    /// `tile_size_m` is how many meters one repeat of the surface's texture covers.
    pub fn tile_size_m(self) -> f32 {
        match self {
            Surface::Ground => 4.0,
            Surface::Bark => 2.0,
            Surface::Planks | Surface::RedPaint | Surface::BluePaint => 2.0,
            Surface::Leaves => 3.0,
            Surface::Rock => 2.5,
            Surface::Rope => 0.5,
            Surface::RedConsole | Surface::BlueConsole => 1.0,
            Surface::Horizon | Surface::Accretion | Surface::Throat => 1.0,
            Surface::Wax | Surface::Flame => 1.0,
        }
    }

    /// `from_material_name` maps a map palette name to a surface (planks for unknown names).
    pub fn from_material_name(name: &str) -> Surface {
        match name {
            "forest floor" => Surface::Ground,
            "bamboo" | "bark" => Surface::Bark,
            "leaves" => Surface::Leaves,
            "rock" => Surface::Rock,
            "rope" => Surface::Rope,
            "red paint" => Surface::RedPaint,
            "blue paint" => Surface::BluePaint,
            "red console" => Surface::RedConsole,
            "blue console" => Surface::BlueConsole,
            "candle wax" => Surface::Wax,
            "candle flame" => Surface::Flame,
            _ => Surface::Planks,
        }
    }
}

/// `WorldBox` is one solid box of the arena and its surface.
#[derive(Clone, Copy, Debug)]
pub struct WorldBox {
    pub aabb: Aabb,
    pub surface: Surface,
    pub is_drawn: bool,
}

/// `LightPlacement` is one point light and its color (in multiples of the light scale).
#[derive(Clone, Copy, Debug)]
pub struct LightPlacement {
    pub position_m: Vec3,
    pub radius_m: f32,
    pub color_rgb: Vec3,
    pub flicker_frac: f32,
}

/// `ShapeKind` is the kind of a smooth shape that is drawn as a mesh.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShapeKind {
    Cylinder { axis: usize },
    Ellipsoid,
}

/// `WorldShape` is one trunk, log or canopy, its centre and half size in world axes.
#[derive(Clone, Copy, Debug)]
pub struct WorldShape {
    pub kind: ShapeKind,
    pub centre_m: Vec3,
    pub half_m: Vec3,
    pub local_half_m: Vec3,
    pub rotation: Mat3,
    pub surface: Surface,
}

/// `LadderVolume` is a climbable rope ladder: where a player climbs, and which way they face.
#[derive(Clone, Copy, Debug)]
pub struct LadderVolume {
    pub aabb: Aabb,
    pub frame: Mat4,
    pub half_m: Vec3,
}

/// `Console` is one pylon's console: its team (0 blue, 1 red, as `net::TEAM_NAMES`) and centre.
#[derive(Clone, Copy, Debug)]
pub struct Console {
    pub team: u32,
    pub centre_m: Vec3,
}

/// `Pylon` is a pylon drawn as the shared beacon model; `console_index` is its console.
#[derive(Clone, Copy, Debug)]
pub struct Pylon {
    pub foot_m: Vec3,
    pub height_m: f32,
    pub console_index: usize,
}

/// `Arena` is the whole level; `map_player` and `map_gravity_m_per_s2` are how players move on
/// its map before the server settings (`player::Movement`).
pub struct Arena {
    pub map_player: crate::map::PlayerSpec,
    pub map_gravity_m_per_s2: f32,
    pub pylons: Vec<Pylon>,
    pub warps: Warps,
    pub boxes: Vec<WorldBox>,
    pub shapes: Vec<WorldShape>,
    pub ladders: Vec<LadderVolume>,
    pub consoles: Vec<Console>,
    pub laser_rgb: Option<Vec3>,
    pub lights: Vec<LightPlacement>,
    pub player_spawns: Vec<(Vec3, f32)>,
    pub team_spawns: [Vec<(Vec3, f32)>; 2],
    pub bot_spawns: Vec<Vec3>,
    pub bounds_min_m: Vec3,
    pub bounds_max_m: Vec3,
    /// `boundary_planes` are the team boundary planes (W-148), from the map's zones.
    pub boundary_planes: Vec<BoundaryPlane>,
}

/// `BoundaryPlane` is one team boundary plane (W-148, team boundary planes in every level): the
/// plane z = `z_m` (Fermi u = −z) on a finite edge of team `team`'s zone (0 blue, 1 red),
/// `height_m` tall over the ground and `opacity_frac` opaque at its foot. It stops neither players
/// nor shots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryPlane {
    pub z_m: f32,
    pub team: usize,
    pub height_m: f32,
    pub opacity_frac: f32,
}

/// `get_boundary_planes` gives a plane on each finite edge of the blue and red zones of the map's
/// regions, if it has zone walls; a zone of another team panics (the map is the forest's own).
///
/// Example: `let planes: Vec<BoundaryPlane> = get_boundary_planes(map.regions.as_ref());`
fn get_boundary_planes(regions: Option<&crate::map::Regions>) -> Vec<BoundaryPlane> {
    let Some(crate::map::Regions {
        zones,
        zone_walls: Some(walls),
    }) = regions
    else {
        return Vec::new();
    };
    zones
        .iter()
        .filter(|zone| zone.team != "middle")
        .flat_map(|zone| {
            let team: usize = match zone.team.as_str() {
                "blue" => 0,
                "red" => 1,
                other => panic!("map: zone team {other:?} isn't blue, red or middle"),
            };
            [zone.u_min_m, zone.u_max_m]
                .into_iter()
                .flatten()
                .map(move |u_m| BoundaryPlane {
                    z_m: -u_m,
                    team,
                    height_m: walls.height_m,
                    opacity_frac: walls.opacity_frac,
                })
        })
        .collect()
}

/// `CYLINDER_SEGMENT_COUNT` is how many sides a trunk or log mesh has.
const CYLINDER_SEGMENT_COUNT: usize = 14;
/// `ACCRETION_DISK_RADIUS_FRAC` is a black hole's glowing disk radius per horizon radius.
const ACCRETION_DISK_RADIUS_FRAC: f32 = 2.6;
/// `ELLIPSOID_RING_COUNT` and `ELLIPSOID_SEGMENT_COUNT` are a leaf spray's latitude and longitude
/// divisions.
const ELLIPSOID_RING_COUNT: usize = 4;
const ELLIPSOID_SEGMENT_COUNT: usize = 6;
/// `WARP_RING_COUNT` and `WARP_SEGMENT_COUNT` divide the smooth spheres and disks of warps.
const WARP_RING_COUNT: usize = 16;
const WARP_SEGMENT_COUNT: usize = 32;
/// `BAMBOO_TILE_FRAC_AROUND_CULM` is how much of the bamboo texture's width (a row of about 30
/// culms) wraps once around a culm, so each culm shows about two of the texture's.
const BAMBOO_TILE_FRAC_AROUND_CULM: f32 = 0.06;
/// `TRUNK_COLLISION_FRAC` is the share of a round shape's radius its collision box covers.
const TRUNK_COLLISION_FRAC: f32 = 0.82;
/// `CANOPY_COLLISION_FRAC` is the share of a canopy's half size its collision box covers.
const CANOPY_COLLISION_FRAC: f32 = 0.6;
/// `RUNG_SPACING_M` is the distance between a rope ladder's rungs.
const RUNG_SPACING_M: f32 = 0.38;

/// `team_index` maps a map team name to `net::TEAM_NAMES`' index (blue 0, red 1).
fn team_index(team: &str) -> Option<u32> {
    match team {
        "blue" => Some(0),
        "red" => Some(1),
        _ => None,
    }
}

impl Arena {
    /// `from_map` builds the arena from a Euclidean map.
    ///
    /// Example: `let arena: Arena = Arena::from_map(&map);`
    pub fn from_map(map: &MapFile) -> Arena {
        let surface_of = |material: u8| -> Surface {
            map.materials
                .iter()
                .find(|entry| entry.id == material)
                .map_or(Surface::Planks, |entry| {
                    Surface::from_material_name(&entry.name)
                })
        };
        let mut boxes: Vec<WorldBox> = Vec::new();
        let mut consoles: Vec<Console> = Vec::new();
        for map_box in &map.boxes {
            let centre_m: Vec3 = map::frame_translation_m(&map_box.frame);
            let half_m: Vec3 =
                map::frame_half_extent_m(&map_box.frame, Vec3::from(map_box.half_size_m));
            let surface: Surface = surface_of(map_box.material);
            boxes.push(WorldBox {
                aabb: Aabb::from_center(centre_m, half_m),
                surface,
                is_drawn: true,
            });
            match surface {
                Surface::RedConsole => consoles.push(Console { team: 1, centre_m }),
                Surface::BlueConsole => consoles.push(Console { team: 0, centre_m }),
                _ => {}
            }
        }
        let mut shapes: Vec<WorldShape> = Vec::new();
        let mut pylons: Vec<Pylon> = Vec::new();
        for structure in &map.structures {
            let frame: Mat4 = Mat4::from_cols_array(&structure.frame);
            if let Some(mesh) = &structure.mesh {
                assert_eq!(
                    mesh.model, "scifi_beacon",
                    "structure {:?} draws a model this demo lacks",
                    structure.name
                );
                for shape in &structure.shapes {
                    let centre_m: Vec3 = frame.transform_point3(Vec3::from(shape.centre_m));
                    let half_m: Vec3 =
                        map::frame_half_extent_m(&structure.frame, Vec3::from(shape.half_m));
                    let surface: Surface = surface_of(shape.material);
                    boxes.push(WorldBox {
                        aabb: Aabb::from_center(centre_m, half_m),
                        surface,
                        is_drawn: false,
                    });
                    let team: u32 = match surface {
                        Surface::RedConsole => 1,
                        Surface::BlueConsole => 0,
                        _ => continue,
                    };
                    pylons.push(Pylon {
                        foot_m: map::frame_translation_m(&structure.frame),
                        height_m: mesh.height_m,
                        console_index: consoles.len(),
                    });
                    consoles.push(Console { team, centre_m });
                }
                continue;
            }
            for shape in &structure.shapes {
                let centre_m: Vec3 = frame.transform_point3(Vec3::from(shape.centre_m));
                let half_m: Vec3 =
                    map::frame_half_extent_m(&structure.frame, Vec3::from(shape.half_m));
                let surface: Surface = surface_of(shape.material);
                let (kind, collision_half_m) = match shape.kind.as_str() {
                    "cylinder" => {
                        let axis: usize = usize::from(shape.axis.min(2));
                        let mut collision: Vec3 = half_m * TRUNK_COLLISION_FRAC;
                        collision[axis] = half_m[axis];
                        (ShapeKind::Cylinder { axis }, collision)
                    }
                    "ellipsoid" => (ShapeKind::Ellipsoid, half_m * CANOPY_COLLISION_FRAC),
                    _ => {
                        boxes.push(WorldBox {
                            aabb: Aabb::from_center(centre_m, half_m),
                            surface,
                            is_drawn: true,
                        });
                        continue;
                    }
                };
                shapes.push(WorldShape {
                    kind,
                    centre_m,
                    half_m,
                    local_half_m: Vec3::from(shape.half_m),
                    rotation: Mat3::from_mat4(frame),
                    surface,
                });
                if !matches!(surface, Surface::Leaves | Surface::Wax | Surface::Flame) {
                    boxes.push(WorldBox {
                        aabb: Aabb::from_center(centre_m, collision_half_m),
                        surface,
                        is_drawn: false,
                    });
                }
            }
        }
        let ladders: Vec<LadderVolume> = map
            .ladders
            .iter()
            .map(|ladder| {
                let centre_m: Vec3 = map::frame_translation_m(&ladder.frame);
                LadderVolume {
                    aabb: Aabb::from_center(
                        centre_m,
                        map::frame_half_extent_m(&ladder.frame, Vec3::from(ladder.half_size_m)),
                    ),
                    frame: Mat4::from_cols_array(&ladder.frame),
                    half_m: Vec3::from(ladder.half_size_m),
                }
            })
            .collect();
        let lights: Vec<LightPlacement> = map
            .lights
            .iter()
            .map(|light| LightPlacement {
                position_m: Vec3::new(light.position[0], light.position[1], light.position[2]),
                radius_m: light.radius_m,
                color_rgb: Vec3::from(light.color),
                flicker_frac: light.flicker_frac.unwrap_or(0.0),
            })
            .collect();
        let warps: Warps = Warps::from_map(&map.warps);
        let warp_shape = |centre_m: Vec3, half_m: Vec3, surface: Surface| WorldShape {
            kind: ShapeKind::Ellipsoid,
            centre_m,
            half_m,
            local_half_m: half_m,
            rotation: Mat3::IDENTITY,
            surface,
        };
        for hole in &warps.black_holes {
            let r_m: f32 = hole.horizon_radius_m;
            shapes.push(warp_shape(
                hole.centre_m,
                Vec3::splat(r_m),
                Surface::Horizon,
            ));
            shapes.push(warp_shape(
                hole.centre_m,
                Vec3::new(
                    r_m * ACCRETION_DISK_RADIUS_FRAC,
                    0.04,
                    r_m * ACCRETION_DISK_RADIUS_FRAC,
                ),
                Surface::Accretion,
            ));
        }
        let mut team_spawns: [Vec<(Vec3, f32)>; 2] = [Vec::new(), Vec::new()];
        let mut player_spawns: Vec<(Vec3, f32)> = Vec::new();
        for spawn in &map.spawns {
            let pose: (Vec3, f32) = (
                map::frame_translation_m(&spawn.frame),
                map::frame_yaw_rad(&spawn.frame),
            );
            if let Some(team) = team_index(&spawn.team) {
                team_spawns[team as usize].push(pose);
            }
            player_spawns.push(pose);
        }
        let bot_spawns: Vec<Vec3> = team_spawns[0].iter().map(|(p, _)| *p).collect();
        let (mut bounds_min_m, mut bounds_max_m) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for world_box in &boxes {
            bounds_min_m = bounds_min_m.min(world_box.aabb.min_m);
            bounds_max_m = bounds_max_m.max(world_box.aabb.max_m);
        }
        for shape in &shapes {
            bounds_min_m = bounds_min_m.min(shape.centre_m - shape.half_m);
            bounds_max_m = bounds_max_m.max(shape.centre_m + shape.half_m);
        }
        Arena {
            map_player: map.player.clone(),
            map_gravity_m_per_s2: map.gravity_m_per_s2,
            boxes,
            shapes,
            ladders,
            consoles,
            laser_rgb: map.laser.as_ref().map(|laser| Vec3::from(laser.rgb)),
            lights,
            player_spawns,
            team_spawns,
            bot_spawns,
            bounds_min_m,
            bounds_max_m,
            warps,
            pylons,
            boundary_planes: get_boundary_planes(map.regions.as_ref()),
        }
    }

    /// `surface_meshes` builds one mesh per surface from the arena's boxes, shapes and ladders.
    /// The collision boxes of round shapes are not drawn.
    ///
    /// Example:
    ///     for (surface, mesh) in arena.surface_meshes() { ... }
    pub fn surface_meshes(&self) -> Vec<(Surface, MeshData)> {
        let mut solid_boxes: Vec<(Surface, Aabb)> = self
            .boxes
            .iter()
            .filter(|b| b.is_drawn)
            .map(|b| (b.surface, b.aabb))
            .collect();
        for ladder in &self.ladders {
            solid_boxes.extend(
                ladder_parts(ladder)
                    .into_iter()
                    .map(|aabb| (Surface::Rope, aabb)),
            );
        }
        Surface::ALL
            .iter()
            .filter_map(|surface: &Surface| {
                let mut mesh: MeshData = MeshData {
                    vertices: Vec::new(),
                    skin_vertices: None,
                    indices: Vec::new(),
                };
                for (_, aabb) in solid_boxes.iter().filter(|(s, _)| s == surface) {
                    append_box(&mut mesh, aabb, surface.tile_size_m());
                }
                for shape in self.shapes.iter().filter(|s| s.surface == *surface) {
                    match shape.kind {
                        ShapeKind::Cylinder { axis } => {
                            append_cylinder(&mut mesh, shape, axis, surface.tile_size_m())
                        }
                        ShapeKind::Ellipsoid => {
                            append_ellipsoid(&mut mesh, shape, surface.tile_size_m())
                        }
                    }
                }
                (!mesh.indices.is_empty()).then_some((*surface, mesh))
            })
            .collect()
    }

    /// `occluders` are the boxes that block sky light: everything solid but the ground.
    ///
    /// Example: `let boxes: Vec<Aabb> = arena.occluders();`
    pub fn occluders(&self) -> Vec<Aabb> {
        self.boxes.iter().skip(1).map(|b| b.aabb).collect()
    }
}

/// `ladder_parts` are the boxes that draw a rope ladder: two side ropes and the rungs.
fn ladder_parts(ladder: &LadderVolume) -> Vec<Aabb> {
    let frame: Mat4 = ladder.frame;
    let to_world = |centre: Vec3, half: Vec3| -> Aabb {
        let world_half: Vec3 = frame.x_axis.truncate().abs() * half.x
            + frame.y_axis.truncate().abs() * half.y
            + frame.z_axis.truncate().abs() * half.z;
        Aabb::from_center(frame.transform_point3(centre), world_half)
    };
    let half_height_m: f32 = ladder.half_m.y;
    let rope_x_m: f32 = 0.32;
    let mut parts: Vec<Aabb> = vec![
        to_world(
            Vec3::new(-rope_x_m, 0.0, 0.0),
            Vec3::new(0.025, half_height_m, 0.025),
        ),
        to_world(
            Vec3::new(rope_x_m, 0.0, 0.0),
            Vec3::new(0.025, half_height_m, 0.025),
        ),
    ];
    let mut y_m: f32 = -half_height_m + 0.3;
    while y_m < half_height_m - 0.9 {
        parts.push(to_world(
            Vec3::new(0.0, y_m, 0.0),
            Vec3::new(rope_x_m, 0.025, 0.035),
        ));
        y_m += RUNG_SPACING_M;
    }
    parts
}

/// `append_cylinder` adds a capped cylinder along an axis of its structure's frame, its UVs around
/// and along it.
fn append_cylinder(mesh: &mut MeshData, shape: &WorldShape, axis: usize, tile_size_m: f32) {
    let rotation: Mat3 = shape.rotation;
    let along: Vec3 = Vec3::AXES[axis];
    let (a, b) = (Vec3::AXES[(axis + 1) % 3], Vec3::AXES[(axis + 2) % 3]);
    let radius_a_m: f32 = shape.local_half_m.dot(a);
    let radius_b_m: f32 = shape.local_half_m.dot(b);
    let half_length_m: f32 = shape.local_half_m.dot(along);
    let circumference_m: f32 = std::f32::consts::TAU * (radius_a_m + radius_b_m) * 0.5;
    let around_tile_count: f32 = match shape.surface {
        Surface::Bark => BAMBOO_TILE_FRAC_AROUND_CULM,
        _ => circumference_m / tile_size_m,
    };
    let base: u32 = mesh.vertices.len() as u32;
    for segment in 0..=CYLINDER_SEGMENT_COUNT {
        let angle: f32 = segment as f32 / CYLINDER_SEGMENT_COUNT as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let radial: Vec3 = a * cos * radius_a_m + b * sin * radius_b_m;
        let normal: Vec3 = rotation * (a * cos / radius_a_m + b * sin / radius_b_m).normalize();
        let tangent: Vec3 = rotation * (b * cos - a * sin).normalize();
        for end in [-1.0_f32, 1.0] {
            let position: Vec3 = shape.centre_m + rotation * (radial + along * half_length_m * end);
            let u: f32 = segment as f32 / CYLINDER_SEGMENT_COUNT as f32 * around_tile_count;
            let v: f32 = -(position.dot(rotation * along)) / tile_size_m;
            mesh.vertices.push(Vertex {
                position: position.to_array(),
                normal: normal.to_array(),
                tangent: tangent.extend(1.0).to_array(),
                uv: [u, v],
            });
        }
    }
    for segment in 0..CYLINDER_SEGMENT_COUNT as u32 {
        let (i0, i1, i2, i3) = (
            base + segment * 2,
            base + segment * 2 + 1,
            base + segment * 2 + 2,
            base + segment * 2 + 3,
        );
        push_outward(mesh, [i0, i2, i1], shape.centre_m);
        push_outward(mesh, [i1, i2, i3], shape.centre_m);
    }
    for end in [-1.0_f32, 1.0] {
        let normal: Vec3 = rotation * along * end;
        let centre_index: u32 = mesh.vertices.len() as u32;
        let centre: Vec3 = shape.centre_m + rotation * along * half_length_m * end;
        mesh.vertices.push(Vertex {
            position: centre.to_array(),
            normal: normal.to_array(),
            tangent: (rotation * a).extend(1.0).to_array(),
            uv: [0.0, 0.0],
        });
        for segment in 0..=CYLINDER_SEGMENT_COUNT {
            let angle: f32 = segment as f32 / CYLINDER_SEGMENT_COUNT as f32 * std::f32::consts::TAU;
            let offset: Vec3 = a * angle.cos() * radius_a_m + b * angle.sin() * radius_b_m;
            mesh.vertices.push(Vertex {
                position: (centre + rotation * offset).to_array(),
                normal: normal.to_array(),
                tangent: (rotation * a).extend(1.0).to_array(),
                uv: [offset.dot(a) / tile_size_m, offset.dot(b) / tile_size_m],
            });
        }
        for segment in 0..CYLINDER_SEGMENT_COUNT as u32 {
            let triangle: [u32; 3] = [
                centre_index,
                centre_index + 1 + segment,
                centre_index + 2 + segment,
            ];
            push_facing(mesh, triangle, normal);
        }
    }
}

/// `append_ellipsoid` adds a latitude-longitude ellipsoid, turned with its structure's frame.
fn append_ellipsoid(mesh: &mut MeshData, shape: &WorldShape, tile_size_m: f32) {
    let (ring_count, segment_count): (usize, usize) = if shape.surface == Surface::Leaves {
        (ELLIPSOID_RING_COUNT, ELLIPSOID_SEGMENT_COUNT)
    } else {
        (WARP_RING_COUNT, WARP_SEGMENT_COUNT)
    };
    let (rotation, half_m) = (shape.rotation, shape.local_half_m);
    let base: u32 = mesh.vertices.len() as u32;
    let row_length: u32 = segment_count as u32 + 1;
    let around_m: f32 = std::f32::consts::TAU * (half_m.x + half_m.z) * 0.5;
    for ring in 0..=ring_count {
        let polar: f32 = ring as f32 / ring_count as f32 * std::f32::consts::PI;
        for segment in 0..=segment_count {
            let azimuth: f32 = segment as f32 / segment_count as f32 * std::f32::consts::TAU;
            let unit: Vec3 = Vec3::new(
                polar.sin() * azimuth.cos(),
                polar.cos(),
                polar.sin() * azimuth.sin(),
            );
            let position: Vec3 = shape.centre_m + rotation * (unit * half_m);
            let normal: Vec3 = rotation * (unit / half_m).normalize();
            let tangent: Vec3 = rotation * Vec3::new(-azimuth.sin(), 0.0, azimuth.cos());
            mesh.vertices.push(Vertex {
                position: position.to_array(),
                normal: normal.to_array(),
                tangent: tangent.extend(1.0).to_array(),
                uv: [
                    segment as f32 / segment_count as f32 * around_m / tile_size_m,
                    polar * half_m.y / tile_size_m,
                ],
            });
        }
    }
    for ring in 0..ring_count as u32 {
        for segment in 0..segment_count as u32 {
            let i0: u32 = base + ring * row_length + segment;
            let (i1, i2, i3) = (i0 + 1, i0 + row_length, i0 + row_length + 1);
            if ring > 0 {
                push_outward(mesh, [i0, i1, i2], shape.centre_m);
            }
            if ring + 1 < ring_count as u32 {
                push_outward(mesh, [i1, i3, i2], shape.centre_m);
            }
        }
    }
}

/// `push_outward` adds a triangle wound counter-clockwise as seen from away from a centre.
fn push_outward(mesh: &mut MeshData, triangle: [u32; 3], centre_m: Vec3) {
    let p: [Vec3; 3] = triangle.map(|index| Vec3::from(mesh.vertices[index as usize].position));
    let mid: Vec3 = (p[0] + p[1] + p[2]) / 3.0;
    let outward: Vec3 =
        Vec3::from(mesh.vertices[triangle[0] as usize].normal) + (mid - centre_m) * 1e-3;
    push_facing(mesh, triangle, outward);
}

/// `push_facing` adds a triangle wound counter-clockwise as seen from along `normal`.
fn push_facing(mesh: &mut MeshData, triangle: [u32; 3], normal: Vec3) {
    let p: [Vec3; 3] = triangle.map(|index| Vec3::from(mesh.vertices[index as usize].position));
    if (p[1] - p[0]).cross(p[2] - p[0]).dot(normal) >= 0.0 {
        mesh.indices.extend(triangle);
    } else {
        mesh.indices.extend([triangle[0], triangle[2], triangle[1]]);
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

/// `tests` checks the arena's box meshes and the Dark Forest map's spawns.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | box face | each of the six faces | `test_box_faces_wind_outward` |
/// | spawn | each of both teams' (on the ground, the body clear, facing open forest) | `test_every_spawn_stands_clear_facing_open_forest` |
/// | team boundary planes (W-148) | the map's (one per team, each between the middle and its team's spawns) | `test_each_team_has_a_boundary_plane_before_its_spawns` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::collision::cast_ray;

    /// `test_each_team_has_a_boundary_plane_before_its_spawns` checks the forest's map gives one
    /// boundary plane per team, between the middle and that team's spawns.
    #[test]
    fn test_each_team_has_a_boundary_plane_before_its_spawns() {
        let map = crate::map::load(&crate::map::default_map_path()).expect("the map loads");
        let arena: Arena = Arena::from_map(&map);
        assert_eq!(arena.boundary_planes.len(), 2);
        for plane in &arena.boundary_planes {
            let spawns: &[(Vec3, f32)] = &arena.team_spawns[plane.team];
            assert!(!spawns.is_empty());
            for z_m in spawns.iter().map(|(at, _)| at.z) {
                assert!(z_m * plane.z_m > plane.z_m * plane.z_m, "{plane:?}: {z_m}");
            }
            assert!(plane.height_m > 0.0 && plane.opacity_frac > 0.0 && plane.opacity_frac < 1.0);
        }
    }

    const BODY_RADIUS_M: f32 = 0.3;
    const BODY_HEIGHT_M: f32 = 1.8;
    const EYE_HEIGHT_M: f32 = 1.68;
    const LIFT_M: f32 = 0.05;
    const GROUND_PROBE_M: f32 = 0.2;
    const MIN_OPEN_VIEW_M: f32 = 3.0;

    /// `test_every_spawn_stands_clear_facing_open_forest` checks each team spawn of the Dark Forest
    /// map stands on something solid, with room for a body, and faces at least `MIN_OPEN_VIEW_M`
    /// of open forest rather than a wall.
    #[test]
    fn test_every_spawn_stands_clear_facing_open_forest() {
        let map = crate::map::load(&crate::map::default_map_path()).expect("the map loads");
        let arena: Arena = Arena::from_map(&map);
        let solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let spawns: Vec<(Vec3, f32)> = arena.team_spawns.concat();
        assert_eq!(spawns.len(), 8);
        for (feet_m, yaw_rad) in spawns {
            let below = cast_ray(
                &solids,
                feet_m + Vec3::Y * LIFT_M,
                Vec3::NEG_Y,
                GROUND_PROBE_M,
            );
            assert!(below.is_some(), "the spawn at {feet_m} floats");
            let body: Aabb = Aabb {
                min_m: feet_m - Vec3::new(BODY_RADIUS_M, -LIFT_M, BODY_RADIUS_M),
                max_m: feet_m + Vec3::new(BODY_RADIUS_M, BODY_HEIGHT_M, BODY_RADIUS_M),
            };
            assert!(
                !solids.iter().any(|solid| solid.overlaps(&body)),
                "the spawn at {feet_m} is inside something"
            );
            let ahead: Vec3 = Vec3::new(-yaw_rad.sin(), 0.0, -yaw_rad.cos());
            let eye_m: Vec3 = feet_m + Vec3::Y * EYE_HEIGHT_M;
            let wall = cast_ray(&solids, eye_m, ahead, MIN_OPEN_VIEW_M);
            assert!(wall.is_none(), "the spawn at {feet_m} faces a wall");
        }
    }

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
