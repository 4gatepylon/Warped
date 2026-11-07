//! `world.rs` builds the demo's scene in H³, in double precision: hyperbolic boxes (rails, a
//! house, two pylons, trees) grouped into clusters, and three wormholes whose mouths stand at eye
//! height, so you can walk into them.
//!
//! Interface: `World::Build`, `World`, `HyperbolicBox`, `Cluster`, `Mouth`, the layout constants.
//!
//! Example: `let world = World::Build(DEFAULT_CURVATURE_RADIUS_M);`
//!
//! Must know:
//! - Gotchas: a box's faces are surfaces at a constant distance from three perpendicular planes
//!   through its centre, so a box standing on the ground keeps its height however long it is.

use glam::{DMat4, DVec4};

use crate::map::{Map, Spawn, Warp, to_frame};

use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, invert_lorentz, make_boost, make_rotation,
    minkowski_dot,
};

pub const DEFAULT_CURVATURE_RADIUS_M: f64 = 8.0;
pub const HORIZON_RADIUS_M: f64 = 1.4;
pub const THROAT_PER_HORIZON: f64 = 0.25;
pub const BEND_CUTOFF_PER_R: f64 = 2.0;
pub const FOIL_LAMBDA_SQUARED: f64 = 0.25;
pub const JUMP_RADIUS_PER_THROAT: f64 = 4.5;
pub const EYE_HEIGHT_M: f64 = 1.7;
pub const MAX_MOUTH_COUNT: usize = 8;
pub const MAX_BOX_COUNT: usize = 96;
pub const MAX_CLUSTER_COUNT: usize = 32;
pub const MATERIAL_SOLID: u32 = 0;
pub const MATERIAL_GLASS: u32 = 1;
pub const MATERIAL_GLOW: u32 = 2;

const SPAWN_X_M: f64 = 0.0;
const SPAWN_Z_M: f64 = 3.0;
const SPAWN_HEADING_RAD: f64 = -0.3;
const MOUTH_HEIGHT_M: f64 = EYE_HEIGHT_M;
const HIGH_EXIT_HEIGHT_M: f64 = 12.0;
const HIGH_EXIT_PITCH_DOWN_RAD: f64 = 1.1;
const RAIL_START_M: f64 = 5.0;
const RAIL_SPACING_M: f64 = 3.0;
const RAIL_LENGTH_M: f64 = 50.0;
const RAIL_HALF_COUNT: i32 = 3;
const RAIL_HALF_WIDTH_M: f64 = 0.5;
const RAIL_HEIGHT_M: f64 = 1.0;
const BASE_WALL_HALF_LENGTH_M: f64 = 15.0;
const HOUSE_DISTANCE_M: f64 = 14.0;
const RED_PYLON_DISTANCE_M: f64 = 42.0;
const RED_PYLON_OFFSET_M: f64 = 8.0;
const BLUE_PYLON_BEARING_RAD: f64 = 2.3;
const BLUE_PYLON_DISTANCE_M: f64 = 13.0;
const CONSOLES_PER_PYLON: usize = 5;
const CONSOLE_RING_RADIUS_M: f64 = 2.6;
const TREE_SEED: u32 = 99;
const TREE_COUNT: usize = 9;
const TREE_ATTEMPT_COUNT: usize = 400;
const TREE_MIN_DISTANCE_M: f64 = 7.0;
const TREE_MAX_DISTANCE_M: f64 = 32.0;
const TREE_CLEARANCE_FROM_BOXES_M: f64 = 3.5;
const TREE_CLEARANCE_FROM_POINTS_M: f64 = 8.0;
const TREE_MIN_HEIGHT_M: f64 = 4.0;
const TREE_HEIGHT_CHOICE_COUNT: f64 = 2.0;
const FULL_TURN_RAD: f64 = std::f64::consts::TAU;
const STANDING_TOLERANCE: f64 = 1e-9;
const BOX_CORNER_COUNT: usize = 8;
const QUARTER_TURN_RAD: f64 = std::f64::consts::FRAC_PI_2;
/// `DEFAULT_ENTRANCES` is each starting wormhole's entrance: its bearing from the spawn view and
/// its distance along the ground from spawn.
const DEFAULT_ENTRANCES: [(f64, f64); 3] = [(-0.5, 7.5), (2.7, 9.0), (-1.5, 12.0)];

/// `Color` is a linear RGB colour.
pub type Color = [f32; 3];

const PLANKS: Color = [0.74, 0.57, 0.34];
const LOG: Color = [0.42, 0.31, 0.19];
const COBBLE: Color = [0.55, 0.55, 0.55];
const GLASS: Color = [0.70, 0.87, 0.96];
const ROOF: Color = [0.64, 0.25, 0.20];
const BRICK: Color = [0.58, 0.30, 0.25];
const LEAVES: Color = [0.24, 0.52, 0.20];
const STONE: Color = [0.62, 0.62, 0.64];
const CONSOLE: Color = [0.78, 0.80, 0.84];
const TEAM_BLUE: Color = [0.30, 0.60, 1.00];
const TEAM_RED: Color = [1.00, 0.32, 0.26];
const RED: Color = [0.80, 0.20, 0.18];
const BLUE: Color = [0.20, 0.35, 0.80];
const GOLD: Color = [0.95, 0.78, 0.25];
pub const NAMED_COLORS: [(&str, Color); 14] = [
    ("planks", PLANKS),
    ("log", LOG),
    ("cobblestone", COBBLE),
    ("glass", GLASS),
    ("roof", ROOF),
    ("brick", BRICK),
    ("leaves", LEAVES),
    ("stone", STONE),
    ("console", CONSOLE),
    ("team blue", TEAM_BLUE),
    ("team red", TEAM_RED),
    ("red", RED),
    ("blue", BLUE),
    ("gold", GOLD),
];
const RAIL_COLORS: [Color; 7] = [
    [0.80, 0.20, 0.18],
    [0.92, 0.50, 0.15],
    [0.92, 0.80, 0.20],
    [0.45, 0.80, 0.20],
    [0.20, 0.70, 0.75],
    [0.20, 0.35, 0.80],
    [0.55, 0.25, 0.70],
];

/// `HyperbolicBox` is the set of points within `half_m[i]` blocks of the planes through `center`
/// whose unit normals are `normals[i]`.
#[derive(Clone)]
pub struct HyperbolicBox {
    pub normals: [DVec4; 3],
    pub center: DVec4,
    pub half_m: [f64; 3],
    pub color: Color,
    pub material: u32,
}

/// `Cluster` is a run of boxes inside one bounding box (`bound`, the box itself for a one-box
/// cluster), which a ray can skip at once.
#[derive(Clone)]
pub struct Cluster {
    pub name: String,
    pub bound: HyperbolicBox,
    pub first_box: usize,
    pub box_count: usize,
    pub top_m: f64,
}

/// `Mouth` is one end of a wormhole. Its frame's columns are right, up, back and centre; you pass
/// through cleanly moving along -back. `gluing` maps the space around it to the space around its
/// partner, with the back axis flipped (or every axis, for a mouth without a partner).
#[derive(Clone)]
pub struct Mouth {
    pub frame: DMat4,
    pub to_local: DMat4,
    pub flip: DMat4,
    pub gluing: DMat4,
    pub center: DVec4,
    pub partner_index: usize,
    pub pair_index: usize,
    pub name: String,
}

/// `World` is the whole scene at one curvature radius.
#[derive(Clone)]
pub struct World {
    pub curvature_radius_m: f64,
    pub boxes: Vec<HyperbolicBox>,
    pub clusters: Vec<Cluster>,
    pub mouths: Vec<Mouth>,
    pub spawn_frame: DMat4,
    pub throat_m: f64,
    pub bend_cutoff_m: f64,
}

impl World {
    /// `Build` builds the scene for a curvature radius: rails, house, pylons, wormholes and trees.
    ///
    /// Example: `let world = World::Build(8.0);`
    #[allow(non_snake_case)]
    pub fn Build(curvature_radius_m: f64) -> World {
        let mut world: World = World {
            curvature_radius_m,
            boxes: Vec::new(),
            clusters: Vec::new(),
            mouths: Vec::new(),
            spawn_frame: DMat4::IDENTITY,
            throat_m: THROAT_PER_HORIZON * HORIZON_RADIUS_M,
            bend_cutoff_m: BEND_CUTOFF_PER_R * curvature_radius_m,
        };
        world.spawn_frame = world.make_ground_frame(SPAWN_X_M, SPAWN_Z_M, SPAWN_HEADING_RAD);
        world.build_rails();
        let house_frame: DMat4 = world.get_house_frame();
        world.group_boxes(house_frame, "the house", |w| w.build_house(house_frame));
        let blue_frame: DMat4 =
            world.make_frame_from_spawn(BLUE_PYLON_BEARING_RAD, BLUE_PYLON_DISTANCE_M);
        world.group_boxes(blue_frame, "the blue pylon", |w| {
            w.build_pylon(blue_frame, TEAM_BLUE)
        });
        let red_frame: DMat4 = world.get_red_pylon_frame();
        world.group_boxes(red_frame, "the red pylon", |w| {
            w.build_pylon(red_frame, TEAM_RED)
        });
        let pairs: Vec<(DMat4, DMat4)> = world.make_default_wormholes();
        world.set_wormholes(&pairs);
        world.place_trees();
        world
    }

    /// `FromMap` builds the world from a map in the portable format (`common/maps/`): its boxes,
    /// grouped into clusters by `group` (each bounded around its first box's frame), its wormhole
    /// mouths and its first spawn point. This world draws boxes and `foil` mouths with the
    /// `flat_effective` radius and a `cutoff`, all alike, so a map with voxel structures, black
    /// holes or other laws fails.
    ///
    /// Example: `let world = World::FromMap(&Map::Load(&get_map_folder("house_demo"))?)?;`
    #[allow(non_snake_case)]
    pub fn FromMap(map: &Map) -> Result<World, String> {
        let unsupported = |what: &str| format!("map {}: this renderer draws boxes and foil mouths; {what}", map.name);
        if !map.structures.is_empty() {
            return Err(unsupported("it has voxel structures"));
        }
        let mouth: &Warp = map.warps.first().ok_or_else(|| unsupported("it has no wormhole mouths"))?;
        for warp in &map.warps {
            let is_like_first: bool = warp.kind == "wormhole_mouth"
                && warp.law == "foil"
                && warp.radial_coordinate == "flat_effective"
                && warp.falloff == "cutoff"
                && warp.lambda_squared == Some(FOIL_LAMBDA_SQUARED)
                && warp.schwarzschild_radius_m == mouth.schwarzschild_radius_m
                && warp.reach_m == mouth.reach_m;
            if !is_like_first {
                return Err(unsupported(&format!("warp {:?} isn't a mouth like the first one", warp.name)));
            }
        }
        let spawn: &Spawn = map.spawns.first().ok_or_else(|| unsupported("it has no spawn point"))?;
        let mut world: World = World {
            curvature_radius_m: map.get_curvature_radius_m(),
            boxes: Vec::new(),
            clusters: Vec::new(),
            mouths: Vec::new(),
            spawn_frame: to_frame(&spawn.frame),
            throat_m: mouth.schwarzschild_radius_m * THROAT_PER_HORIZON,
            bend_cutoff_m: mouth.reach_m,
        };
        let mut groups: Vec<&str> = Vec::new();
        for map_box in &map.boxes {
            if !groups.contains(&map_box.group.as_str()) {
                groups.push(&map_box.group);
            }
        }
        for group in groups {
            let first_box: usize = world.boxes.len();
            for map_box in map.boxes.iter().filter(|b| b.group == group) {
                let material: &crate::map::Material = map.get_material(map_box.material).map_err(|e| e.to_string())?;
                let kind: u32 = match material.kind.as_str() {
                    "glass" => MATERIAL_GLASS,
                    "light" => MATERIAL_GLOW,
                    "solid" => MATERIAL_SOLID,
                    other => return Err(format!("map {}: box {:?} is made of {other:?}", map.name, map_box.name)),
                };
                world.add_box(to_frame(&map_box.frame), map_box.half_size_m, material.color, kind);
            }
            let frame: DMat4 = DMat4::from_cols(
                world.boxes[first_box].normals[0],
                world.boxes[first_box].normals[1],
                world.boxes[first_box].normals[2],
                world.boxes[first_box].center,
            );
            world.close_cluster(frame, group, first_box);
        }
        world.mouths = map
            .warps
            .iter()
            .enumerate()
            .map(|(k, warp)| {
                let partner_index: usize = warp.partner.expect("the format requires a mouth's partner");
                let frame: DMat4 = to_frame(&warp.frame);
                let mut flip: DMat4 = DMat4::IDENTITY;
                flip.col_mut(AXIS_Z)[AXIS_Z] = -1.0;
                Mouth {
                    frame,
                    to_local: invert_lorentz(frame),
                    flip,
                    gluing: DMat4::IDENTITY,
                    center: frame.col(AXIS_W),
                    partner_index,
                    pair_index: k.min(partner_index) / 2,
                    name: warp.name.clone(),
                }
            })
            .collect();
        for k in 0..world.mouths.len() {
            let partner_frame: DMat4 = world.mouths[world.mouths[k].partner_index].frame;
            world.mouths[k].gluing = partner_frame * world.mouths[k].flip * world.mouths[k].to_local;
        }
        Ok(world)
    }

    /// `get_throat_m` gives the throat's radius in the lensing law's coordinates: a quarter of the
    /// mouth size.
    ///
    /// Example: `if rho_effective_m < world.get_throat_m() { cross(); }`
    pub fn get_throat_m(&self) -> f64 {
        self.throat_m
    }

    /// `get_jump_radius_m` gives the radius of the sphere around each mouth that the Metal shader
    /// crosses in one jump: nothing but the mouth may lie inside it.
    ///
    /// Example: `let jump_m = world.get_jump_radius_m();`
    pub fn get_jump_radius_m(&self) -> f64 {
        JUMP_RADIUS_PER_THROAT * self.get_throat_m()
    }

    /// `get_effective_radius_m` gives (R/2)(e^(2ρ) - 1) for a distance ρ in curvature radii: the
    /// flat radius whose 1/radius is H³'s point-mass potential (coth ρ - 1)/R.
    ///
    /// Example: `let rho_effective_m = world.get_effective_radius_m(0.5);`
    pub fn get_effective_radius_m(&self, rho: f64) -> f64 {
        self.curvature_radius_m * rho.sinh() * rho.exp()
    }

    /// `make_ground_frame` makes a frame on the ground `x_m` blocks along the x axis, then `z_m`
    /// blocks at right angles to it, turned by `heading_rad` from north toward east.
    ///
    /// Example: `let frame = world.make_ground_frame(14.0, 0.0, 0.0);`
    pub fn make_ground_frame(&self, x_m: f64, z_m: f64, heading_rad: f64) -> DMat4 {
        let r: f64 = self.curvature_radius_m;
        make_boost(AXIS_X, x_m / r)
            * make_boost(AXIS_Z, z_m / r)
            * make_rotation(AXIS_X, AXIS_Z, heading_rad)
    }

    /// `offset_frame` moves a frame by local offsets: along x, then z, then up.
    ///
    /// Example: `let lintel = world.offset_frame(front, 0.0, 3.0, 0.0);`
    pub fn offset_frame(&self, frame: DMat4, x_m: f64, y_m: f64, z_m: f64) -> DMat4 {
        let r: f64 = self.curvature_radius_m;
        frame
            * make_boost(AXIS_X, x_m / r)
            * make_boost(AXIS_Z, z_m / r)
            * make_boost(AXIS_Y, y_m / r)
    }

    /// `make_frame_from_spawn` makes the ground frame reached by turning `bearing_rad` at spawn
    /// and walking `distance_m` straight on; its forward axis continues the walk.
    ///
    /// Example: `let frame = world.make_frame_from_spawn(2.3, 13.0);`
    pub fn make_frame_from_spawn(&self, bearing_rad: f64, distance_m: f64) -> DMat4 {
        self.spawn_frame
            * make_rotation(AXIS_X, AXIS_Z, bearing_rad)
            * make_boost(AXIS_Z, -distance_m / self.curvature_radius_m)
    }

    /// `add_avatar` adds another player's body: one box around `frame`'s centre and axes, in a
    /// cluster of its own.
    ///
    /// Example: `world.add_avatar(frame, "player 2", [0.3, 0.95, 0.3], [0.9, 0.3, 0.3]);`
    pub fn add_avatar(&mut self, frame: DMat4, name: &str, half_m: [f64; 3], color: Color) {
        self.group_boxes(frame, name, |world| {
            world.add_box(frame, half_m, color, MATERIAL_SOLID)
        });
    }

    /// `add_box` adds a box around `frame`'s centre and axes.
    ///
    /// Example: `world.add_box(frame, [0.5, 1.0, 25.0], RED, MATERIAL_SOLID);`
    fn add_box(&mut self, frame: DMat4, half_m: [f64; 3], color: Color, material: u32) {
        assert!(
            self.boxes.len() < MAX_BOX_COUNT,
            "add_box: the scene already has {MAX_BOX_COUNT} boxes"
        );
        self.boxes.push(HyperbolicBox {
            normals: [frame.col(0), frame.col(1), frame.col(2)],
            center: frame.col(AXIS_W),
            half_m,
            color,
            material,
        });
    }

    /// `add_standing_box` adds a box standing on the ground, `height_m` tall everywhere, whose
    /// hidden half lies below the ground.
    ///
    /// Example: `world.add_standing_box(frame, 0.0, -3.0, [4.6, 4.0, 0.5], PLANKS, MATERIAL_SOLID);`
    fn add_standing_box(
        &mut self,
        frame: DMat4,
        x_m: f64,
        z_m: f64,
        half_m: [f64; 3],
        color: Color,
        material: u32,
    ) {
        let box_frame: DMat4 = self.offset_frame(frame, x_m, 0.0, z_m);
        self.add_box(box_frame, half_m, color, material);
    }

    /// `add_floating_box` adds a box centred above the ground at local offsets (x, height, z).
    ///
    /// Example: `world.add_floating_box(frame, [0.0, 3.0, 0.0], [0.5, 1.0, 0.5], PLANKS, MATERIAL_SOLID);`
    fn add_floating_box(
        &mut self,
        frame: DMat4,
        center_m: [f64; 3],
        half_m: [f64; 3],
        color: Color,
        material: u32,
    ) {
        let box_frame: DMat4 = self.offset_frame(frame, center_m[0], center_m[1], center_m[2]);
        self.add_box(box_frame, half_m, color, material);
    }

    /// `group_boxes` runs a builder and records the boxes it adds as one cluster, bounded by a box
    /// around `frame`'s planes that holds them all (or by its only box).
    ///
    /// Example: `world.group_boxes(frame, "the house", |w| w.build_house(frame));`
    fn group_boxes(&mut self, frame: DMat4, name: &str, build: impl FnOnce(&mut World)) {
        let first_box: usize = self.boxes.len();
        build(self);
        self.close_cluster(frame, name, first_box);
    }

    /// `close_cluster` records the boxes from `first_box` on as one cluster named `name`, bounded
    /// by a box around `frame`'s planes that holds them all (or by its only box).
    ///
    /// Example: `world.close_cluster(frame, "the house", first_box);`
    fn close_cluster(&mut self, frame: DMat4, name: &str, first_box: usize) {
        assert!(
            self.clusters.len() < MAX_CLUSTER_COUNT,
            "close_cluster: too many clusters"
        );
        let boxes: &[HyperbolicBox] = &self.boxes[first_box..];
        assert!(!boxes.is_empty(), "close_cluster: the cluster has no boxes");
        let bound: HyperbolicBox = if boxes.len() == 1 {
            boxes[0].clone()
        } else {
            let half_m: [f64; 3] = [AXIS_X, AXIS_Y, AXIS_Z].map(|axis| {
                let sinh_max: f64 = boxes
                    .iter()
                    .map(|b| self.get_slab_sinh(b, frame.col(axis)))
                    .fold(0.0, f64::max);
                self.curvature_radius_m * sinh_max.asinh()
            });
            HyperbolicBox {
                normals: [frame.col(AXIS_X), frame.col(AXIS_Y), frame.col(AXIS_Z)],
                center: frame.col(AXIS_W),
                half_m,
                color: [0.0; 3],
                material: MATERIAL_SOLID,
            }
        };
        let top_m: f64 = boxes
            .iter()
            .map(|b| self.get_box_top_m(b))
            .fold(0.0, f64::max);
        self.clusters.push(Cluster {
            name: name.to_string(),
            bound,
            first_box,
            box_count: self.boxes.len() - first_box,
            top_m,
        });
    }

    /// `get_slab_sinh` gives a bound on |⟨x, n⟩| over the points x of a box, for a plane's unit
    /// normal n: the sinh of the box's greatest distance from the plane, in curvature radii. In the
    /// box's own coordinates p, where the box is |p_i| ≤ sinh(h_i/R) and p_w = √(1 + |p|²),
    /// ⟨x, n⟩ = a·p + a_w p_w is convex for one sign and concave for the other. The convex sign
    /// peaks at a corner, and the concave sign stays below Σ|a_i| sinh(h_i/R) - |a_w|, as p_w ≥ 1.
    ///
    /// Example: `let sinh_max = world.get_slab_sinh(&world.boxes[0], frame.col(AXIS_Y));`
    fn get_slab_sinh(&self, b: &HyperbolicBox, n: DVec4) -> f64 {
        let r: f64 = self.curvature_radius_m;
        let a: [f64; 3] = b.normals.map(|normal| minkowski_dot(normal, n));
        let a_w: f64 = minkowski_dot(b.center, n);
        let sinh_half: [f64; 3] = b.half_m.map(|h| (h / r).sinh());
        let linear_max: f64 = (0..3).map(|i| a[i].abs() * sinh_half[i]).sum();
        let corner_max: f64 = (0..BOX_CORNER_COUNT)
            .map(|corner: usize| {
                let p: [f64; 3] = [0, 1, 2].map(|i| {
                    if corner >> i & 1 == 1 {
                        sinh_half[i]
                    } else {
                        -sinh_half[i]
                    }
                });
                let p_w: f64 = (1.0 + p.iter().map(|c| c * c).sum::<f64>()).sqrt();
                ((0..3).map(|i| a[i] * p[i]).sum::<f64>() + a_w * p_w).abs()
            })
            .fold(0.0, f64::max);
        corner_max.max(linear_max - a_w.abs())
    }

    /// `get_box_top_m` gives a height above the ground that no point of a box exceeds: exactly its
    /// height for a box standing on the ground (its top is at a constant height), and its centre's
    /// height plus its bounding radius otherwise.
    ///
    /// Example: `let top_m = world.get_box_top_m(&world.boxes[0]);`
    fn get_box_top_m(&self, b: &HyperbolicBox) -> f64 {
        let r: f64 = self.curvature_radius_m;
        if b.center[AXIS_Y].abs() < STANDING_TOLERANCE {
            return b.half_m[1];
        }
        let box_radius_m: f64 = r * b
            .half_m
            .iter()
            .map(|h| (h / r).cosh())
            .product::<f64>()
            .acosh();
        r * b.center[AXIS_Y].asinh() + box_radius_m
    }

    /// `get_box_lower_bound_m` gives a distance a point is at least from a box: the largest of its
    /// distances past each slab (negative inside).
    ///
    /// Example: `if world.get_box_lower_bound_m(b, point) < 3.0 { skip(); }`
    pub fn get_box_lower_bound_m(&self, b: &HyperbolicBox, x: DVec4) -> f64 {
        let r: f64 = self.curvature_radius_m;
        (0..3)
            .map(|i| r * minkowski_dot(x, b.normals[i]).abs().asinh() - b.half_m[i])
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// `build_rails` adds the straight cobblestone wall ahead of spawn and seven straight walls
    /// leaving it at right angles, which would stay parallel in flat space.
    ///
    /// Example: `world.build_rails();`
    fn build_rails(&mut self) {
        let base_frame: DMat4 = self.make_ground_frame(0.0, -RAIL_START_M, 0.0);
        self.group_boxes(base_frame, "the cobblestone wall", |w| {
            w.add_standing_box(
                base_frame,
                0.0,
                0.0,
                [BASE_WALL_HALF_LENGTH_M, RAIL_HEIGHT_M, RAIL_HALF_WIDTH_M],
                COBBLE,
                MATERIAL_SOLID,
            )
        });
        for k in -RAIL_HALF_COUNT..=RAIL_HALF_COUNT {
            let rail_frame: DMat4 =
                self.offset_frame(base_frame, f64::from(k) * RAIL_SPACING_M, 0.0, 0.0);
            let half_length_m: f64 = RAIL_LENGTH_M / 2.0;
            let color: Color = RAIL_COLORS[(k + RAIL_HALF_COUNT) as usize];
            let name: String = format!("rail {}", k + RAIL_HALF_COUNT + 1);
            self.group_boxes(rail_frame, &name, |w| {
                w.add_standing_box(
                    rail_frame,
                    0.0,
                    -(RAIL_HALF_WIDTH_M + half_length_m),
                    [RAIL_HALF_WIDTH_M, RAIL_HEIGHT_M, half_length_m],
                    color,
                    MATERIAL_SOLID,
                )
            });
        }
    }

    /// `get_house_frame` gives the house's frame, with its door facing spawn.
    ///
    /// Example: `let frame = world.get_house_frame();`
    fn get_house_frame(&self) -> DMat4 {
        self.make_ground_frame(HOUSE_DISTANCE_M, 0.0, QUARTER_TURN_RAD)
    }

    /// `get_red_pylon_frame` gives the red pylon's frame: 42 blocks down the middle rail, then 8
    /// blocks to its right.
    ///
    /// Example: `let frame = world.get_red_pylon_frame();`
    fn get_red_pylon_frame(&self) -> DMat4 {
        let r: f64 = self.curvature_radius_m;
        self.make_ground_frame(0.0, -RAIL_START_M, 0.0)
            * make_boost(AXIS_Z, -RED_PYLON_DISTANCE_M / r)
            * make_boost(AXIS_X, RED_PYLON_OFFSET_M / r)
    }

    /// `build_house` adds a small house whose door faces the frame's +z axis; each wall's pieces
    /// are placed along the wall's own line, so they stay in one plane.
    ///
    /// Example: `world.build_house(frame);`
    fn build_house(&mut self, frame: DMat4) {
        let front: DMat4 = self.offset_frame(frame, 0.0, 0.0, 3.0);
        let back: DMat4 = self.offset_frame(frame, 0.0, 0.0, -3.0);
        let sides: [DMat4; 2] = [
            self.offset_frame(frame, -4.0, 0.0, 0.0),
            self.offset_frame(frame, 4.0, 0.0, 0.0),
        ];
        self.add_standing_box(frame, 0.0, 0.0, [4.6, 0.1, 3.6], PLANKS, MATERIAL_SOLID);
        self.add_standing_box(back, 0.0, 0.0, [4.6, 4.0, 0.5], PLANKS, MATERIAL_SOLID);
        for side in sides {
            self.add_standing_box(side, 0.0, 0.0, [0.5, 4.0, 3.6], PLANKS, MATERIAL_SOLID);
        }
        self.add_standing_box(front, -2.6, 0.0, [2.1, 4.0, 0.5], PLANKS, MATERIAL_SOLID);
        self.add_standing_box(front, 2.6, 0.0, [2.1, 4.0, 0.5], PLANKS, MATERIAL_SOLID);
        self.add_floating_box(
            front,
            [0.0, 3.0, 0.0],
            [0.5, 1.0, 0.5],
            PLANKS,
            MATERIAL_SOLID,
        );
        for sign in [-1.0, 1.0] {
            self.add_floating_box(
                front,
                [sign * 2.3, 2.5, 0.0],
                [0.6, 0.6, 0.56],
                GLASS,
                MATERIAL_GLASS,
            );
            self.add_floating_box(
                back,
                [sign * 2.3, 2.5, 0.0],
                [0.6, 0.6, 0.56],
                GLASS,
                MATERIAL_GLASS,
            );
        }
        for side in sides {
            self.add_floating_box(
                side,
                [0.0, 2.5, 0.0],
                [0.56, 0.6, 0.6],
                GLASS,
                MATERIAL_GLASS,
            );
        }
        for (step, half_depth_m) in [4.4, 3.4, 2.4, 1.4, 0.5].iter().enumerate() {
            self.add_floating_box(
                frame,
                [0.0, 4.5 + step as f64, 0.0],
                [5.4, 0.5, *half_depth_m],
                ROOF,
                MATERIAL_SOLID,
            );
        }
        self.add_floating_box(
            frame,
            [2.0, 8.0, -1.5],
            [0.5, 2.0, 0.5],
            BRICK,
            MATERIAL_SOLID,
        );
        self.add_standing_box(frame, -2.0, -1.5, [0.5, 1.0, 0.5], RED, MATERIAL_SOLID);
        self.add_standing_box(frame, 2.0, -1.5, [0.5, 1.0, 0.5], BLUE, MATERIAL_SOLID);
        self.add_standing_box(frame, 0.0, -2.0, [0.5, 2.0, 0.5], GOLD, MATERIAL_SOLID);
    }

    /// `build_tree` adds a blocky tree: a trunk, a wide layer of leaves and a smaller one on top.
    ///
    /// Example: `world.build_tree(frame, 4.0);`
    fn build_tree(&mut self, frame: DMat4, height_m: f64) {
        self.add_standing_box(frame, 0.0, 0.0, [0.5, height_m, 0.5], LOG, MATERIAL_SOLID);
        self.add_floating_box(
            frame,
            [0.0, height_m - 0.5, 0.0],
            [2.5, 1.0, 2.5],
            LEAVES,
            MATERIAL_SOLID,
        );
        self.add_floating_box(
            frame,
            [0.0, height_m + 1.0, 0.0],
            [1.5, 0.5, 1.5],
            LEAVES,
            MATERIAL_SOLID,
        );
    }

    /// `build_pylon` adds a team's pylon: a glowing core on a stone base, ringed by five consoles.
    ///
    /// Example: `world.build_pylon(frame, TEAM_RED);`
    fn build_pylon(&mut self, frame: DMat4, team_color: Color) {
        self.add_standing_box(frame, 0.0, 0.0, [1.5, 0.4, 1.5], STONE, MATERIAL_SOLID);
        self.add_standing_box(frame, 0.0, 0.0, [0.6, 6.0, 0.6], team_color, MATERIAL_GLOW);
        for i in 0..CONSOLES_PER_PYLON {
            let angle_rad: f64 = i as f64 * FULL_TURN_RAD / CONSOLES_PER_PYLON as f64;
            let (s, c) = angle_rad.sin_cos();
            self.add_standing_box(
                frame,
                CONSOLE_RING_RADIUS_M * s,
                CONSOLE_RING_RADIUS_M * c,
                [0.4, 1.1, 0.4],
                CONSOLE,
                MATERIAL_SOLID,
            );
        }
    }

    /// `make_mouth_frame` makes a mouth's frame from a ground frame, local offsets and the tilt of
    /// its forward axis below horizontal.
    ///
    /// Example: `let frame = world.make_mouth_frame(ground, 0.0, 1.7, 8.0, 0.0);`
    fn make_mouth_frame(
        &self,
        frame: DMat4,
        x_m: f64,
        y_m: f64,
        z_m: f64,
        pitch_down_rad: f64,
    ) -> DMat4 {
        self.offset_frame(frame, x_m, y_m, z_m) * make_rotation(AXIS_Z, AXIS_Y, pitch_down_rad)
    }

    /// `make_default_wormholes` makes the three starting wormholes: ahead-left of spawn to the
    /// house door, behind spawn (by the blue pylon) to the red pylon, and far left to high above
    /// the rails. Each entrance faces along a straight line from spawn.
    ///
    /// Example: `let pairs = world.make_default_wormholes();`
    fn make_default_wormholes(&self) -> Vec<(DMat4, DMat4)> {
        let base_frame: DMat4 = self.make_ground_frame(0.0, -RAIL_START_M, 0.0);
        let exits: [DMat4; 3] = [
            self.make_mouth_frame(self.get_house_frame(), 0.0, MOUTH_HEIGHT_M, 8.0, 0.0),
            self.make_mouth_frame(self.get_red_pylon_frame(), 0.0, MOUTH_HEIGHT_M, 10.0, 0.0),
            self.make_mouth_frame(
                base_frame,
                0.0,
                HIGH_EXIT_HEIGHT_M,
                -6.0,
                HIGH_EXIT_PITCH_DOWN_RAD,
            ),
        ];
        DEFAULT_ENTRANCES
            .iter()
            .zip(exits)
            .map(|(&(bearing_rad, distance_m), exit)| {
                let entrance: DMat4 = self.offset_frame(
                    self.make_frame_from_spawn(bearing_rad, distance_m),
                    0.0,
                    MOUTH_HEIGHT_M,
                    0.0,
                );
                (entrance, exit)
            })
            .collect()
    }

    /// `set_wormholes` turns wormhole pairs into the list of mouths, each with its gluing map.
    ///
    /// Example: `world.set_wormholes(&pairs);`
    fn set_wormholes(&mut self, pairs: &[(DMat4, DMat4)]) {
        assert!(
            pairs.len() * 2 <= MAX_MOUTH_COUNT,
            "set_wormholes: at most {} pairs",
            MAX_MOUTH_COUNT / 2
        );
        self.mouths.clear();
        for (pair_index, (a, b)) in pairs.iter().enumerate() {
            for (side, frame) in [(0usize, *a), (1usize, *b)] {
                let mut flip: DMat4 = DMat4::IDENTITY;
                flip.col_mut(AXIS_Z)[AXIS_Z] = -1.0;
                self.mouths.push(Mouth {
                    frame,
                    to_local: invert_lorentz(frame),
                    flip,
                    gluing: DMat4::IDENTITY,
                    center: frame.col(AXIS_W),
                    partner_index: 2 * pair_index + 1 - side,
                    pair_index,
                    name: format!("{}{}", pair_index + 1, if side == 0 { 'A' } else { 'B' }),
                });
            }
        }
        for k in 0..self.mouths.len() {
            let partner_frame: DMat4 = self.mouths[self.mouths[k].partner_index].frame;
            self.mouths[k].gluing = partner_frame * self.mouths[k].flip * self.mouths[k].to_local;
        }
    }

    /// `place_trees` adds trees at seeded random spots near spawn, clear of the boxes, spawn and
    /// the mouths.
    ///
    /// Example: `world.place_trees();`
    fn place_trees(&mut self) {
        let r: f64 = self.curvature_radius_m;
        let mut random: Mulberry32 = Mulberry32 { state: TREE_SEED };
        let mut avoided: Vec<DVec4> = vec![self.spawn_frame.col(AXIS_W)];
        avoided.extend(self.mouths.iter().map(|m| m.center));
        let mut placed_count: usize = 0;
        for _ in 0..TREE_ATTEMPT_COUNT {
            if placed_count >= TREE_COUNT {
                break;
            }
            let angle_rad: f64 = random.next_unit() * FULL_TURN_RAD;
            let distance_m: f64 = TREE_MIN_DISTANCE_M
                + random.next_unit() * (TREE_MAX_DISTANCE_M - TREE_MIN_DISTANCE_M);
            let spin_rad: f64 = random.next_unit() * FULL_TURN_RAD;
            let height_m: f64 =
                TREE_MIN_HEIGHT_M + (random.next_unit() * TREE_HEIGHT_CHOICE_COUNT).floor();
            let frame: DMat4 = make_rotation(AXIS_X, AXIS_Z, angle_rad)
                * make_boost(AXIS_Z, distance_m / r)
                * make_rotation(AXIS_X, AXIS_Z, spin_rad);
            let point: DVec4 = frame.col(AXIS_W);
            if avoided
                .iter()
                .any(|p| get_distance(*p, point) * r < TREE_CLEARANCE_FROM_POINTS_M)
            {
                continue;
            }
            if self
                .boxes
                .iter()
                .any(|b| self.get_box_lower_bound_m(b, point) < TREE_CLEARANCE_FROM_BOXES_M)
            {
                continue;
            }
            let name: String = format!("tree {}", placed_count + 1);
            self.group_boxes(frame, &name, |w| w.build_tree(frame, height_m));
            placed_count += 1;
        }
    }
}

/// `Mulberry32` is a small seeded random number generator, the same as the WebGL demo's.
struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// `next_unit` returns the next number in [0, 1).
    ///
    /// Example: `let r = random.next_unit();`
    fn next_unit(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut t: u32 = (self.state ^ (self.state >> 15)).wrapping_mul(1 | self.state);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the clusters' bounding boxes.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Box against a bounding plane | straddling it, wholly on one side | `slab_bound_is_exact_for_a_centred_box`, `every_box_is_inside_its_cluster_bound` |
    //! | Box orientation against the plane | same frame, rotated and moved | `slab_bound_is_exact_for_a_centred_box`, `slab_bound_is_tight_for_a_moved_box` |
    //! | Cluster | house, pylon, tree | `every_box_is_inside_its_cluster_bound` |
    //! | Mouth | each of the demo's mouths | `jump_spheres_hold_no_boxes_or_ground` |
    //! | World's source | built in code, loaded from `common/maps/house_demo` | `the_house_map_loads_the_built_world` |

    use super::*;
    use crate::geometry::{make_rotation_about, minkowski_dot};
    use glam::DVec3;

    const SAMPLE_COUNT_PER_SIDE: usize = 9;
    const TOLERANCE: f64 = 1e-9;
    const TIGHTNESS: f64 = 0.02;

    /// `sample_box` gives points spread over a box, corners and faces included, from its own
    /// coordinates p (the box is |p_i| ≤ sinh(h_i/R)).
    fn sample_box(b: &HyperbolicBox, r: f64) -> Vec<DVec4> {
        let sinh_half: [f64; 3] = b.half_m.map(|h| (h / r).sinh());
        let fraction =
            |n: usize| -> f64 { 2.0 * n as f64 / (SAMPLE_COUNT_PER_SIDE - 1) as f64 - 1.0 };
        let mut points: Vec<DVec4> = Vec::new();
        for i in 0..SAMPLE_COUNT_PER_SIDE {
            for j in 0..SAMPLE_COUNT_PER_SIDE {
                for k in 0..SAMPLE_COUNT_PER_SIDE {
                    let p: [f64; 3] = [
                        fraction(i) * sinh_half[0],
                        fraction(j) * sinh_half[1],
                        fraction(k) * sinh_half[2],
                    ];
                    let p_w: f64 = (1.0 + p.iter().map(|c| c * c).sum::<f64>()).sqrt();
                    points.push(
                        p[0] * b.normals[0]
                            + p[1] * b.normals[1]
                            + p[2] * b.normals[2]
                            + p_w * b.center,
                    );
                }
            }
        }
        points
    }

    /// `get_sampled_slab_sinh` gives the largest |⟨x, n⟩| over a box's sample points.
    fn get_sampled_slab_sinh(b: &HyperbolicBox, n: DVec4, r: f64) -> f64 {
        sample_box(b, r)
            .iter()
            .map(|x| minkowski_dot(*x, n).abs())
            .fold(0.0, f64::max)
    }

    /// `make_box` gives a box in `frame` with half sizes `half_m`.
    fn make_box(frame: DMat4, half_m: [f64; 3]) -> HyperbolicBox {
        HyperbolicBox {
            normals: [frame.col(AXIS_X), frame.col(AXIS_Y), frame.col(AXIS_Z)],
            center: frame.col(AXIS_W),
            half_m,
            color: [0.0; 3],
            material: MATERIAL_SOLID,
        }
    }

    /// `every_box_is_inside_its_cluster_bound` checks that every sample point of every box of each
    /// cluster in the demo's world is inside the cluster's bounding box.
    #[test]
    fn every_box_is_inside_its_cluster_bound() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let r: f64 = world.curvature_radius_m;
        for cluster in world.clusters.iter().filter(|c| c.box_count > 1) {
            for b in &world.boxes[cluster.first_box..cluster.first_box + cluster.box_count] {
                for x in sample_box(b, r) {
                    assert!(world.get_box_lower_bound_m(&cluster.bound, x) <= TOLERANCE);
                }
            }
        }
    }

    /// `jump_spheres_hold_no_boxes_or_ground` checks that every box and the ground lie outside each
    /// mouth's jump sphere, which the Metal shader crosses without testing them.
    #[test]
    fn jump_spheres_hold_no_boxes_or_ground() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let jump_m: f64 = world.get_jump_radius_m();
        for mouth in &world.mouths {
            assert!(world.curvature_radius_m * mouth.center[AXIS_Y].asinh() > jump_m);
            for b in &world.boxes {
                assert!(world.get_box_lower_bound_m(b, mouth.center) > jump_m);
            }
        }
    }

    /// `slab_bound_is_exact_for_a_centred_box` checks that a box centred on the planes, in their
    /// own frame, is bounded by exactly its own half sizes.
    #[test]
    fn slab_bound_is_exact_for_a_centred_box() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let r: f64 = world.curvature_radius_m;
        let frame: DMat4 = world.spawn_frame;
        let b: HyperbolicBox = make_box(frame, [4.6, 4.0, 0.5]);
        for (axis, half_m) in [AXIS_X, AXIS_Y, AXIS_Z].into_iter().zip(b.half_m) {
            let sinh_max: f64 = world.get_slab_sinh(&b, frame.col(axis));
            assert!((sinh_max - (half_m / r).sinh()).abs() < TOLERANCE);
        }
    }

    /// `slab_bound_is_tight_for_a_moved_box` checks a rotated box moved off the planes on each
    /// side: the bound holds every sample point and is no more than `TIGHTNESS` above them.
    #[test]
    fn slab_bound_is_tight_for_a_moved_box() {
        let world: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let r: f64 = world.curvature_radius_m;
        let frame: DMat4 = world.spawn_frame;
        for offset_m in [[3.0, 5.0, -2.0], [-4.0, -1.0, 6.0]] {
            let box_frame: DMat4 = world.offset_frame(frame, offset_m[0], offset_m[1], offset_m[2])
                * make_rotation_about(DVec3::new(0.3, 0.8, 0.5).normalize(), 0.9);
            let b: HyperbolicBox = make_box(box_frame, [2.5, 1.0, 0.6]);
            for axis in [AXIS_X, AXIS_Y, AXIS_Z] {
                let n: DVec4 = frame.col(axis);
                let (bound, sampled) =
                    (world.get_slab_sinh(&b, n), get_sampled_slab_sinh(&b, n, r));
                assert!(bound >= sampled - TOLERANCE);
                assert!(bound <= sampled + TIGHTNESS);
            }
        }
    }

    #[test]
    fn the_house_map_loads_the_built_world() {
        let built: World = World::Build(DEFAULT_CURVATURE_RADIUS_M);
        let map: Map = Map::Load(&crate::map::get_map_folder("house_demo")).expect("the house map");
        let loaded: World = World::FromMap(&map).expect("a world of boxes and mouths");
        assert_eq!(loaded.curvature_radius_m, built.curvature_radius_m);
        assert_eq!(loaded.spawn_frame, built.spawn_frame);
        assert_eq!((loaded.throat_m, loaded.bend_cutoff_m), (built.throat_m, built.bend_cutoff_m));
        assert_eq!(loaded.boxes.len(), built.boxes.len());
        for (a, b) in loaded.boxes.iter().zip(&built.boxes) {
            assert_eq!((a.normals, a.center, a.half_m, a.color, a.material), (b.normals, b.center, b.half_m, b.color, b.material));
        }
        let members = |w: &World| -> Vec<(String, usize, usize)> {
            w.clusters.iter().map(|c| (c.name.clone(), c.first_box, c.box_count)).collect()
        };
        assert_eq!(members(&loaded), members(&built));
        for (a, b) in loaded.mouths.iter().zip(&built.mouths) {
            assert_eq!((a.frame, a.gluing, a.partner_index, a.pair_index), (b.frame, b.gluing, b.partner_index, b.pair_index));
        }
    }
}
