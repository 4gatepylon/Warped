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

use crate::geometry::{
    AXIS_W, AXIS_X, AXIS_Y, AXIS_Z, get_distance, invert_lorentz, make_boost, make_rotation,
};

pub const DEFAULT_CURVATURE_RADIUS_M: f64 = 8.0;
pub const HORIZON_RADIUS_M: f64 = 1.4;
pub const THROAT_PER_HORIZON: f64 = 0.25;
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

/// `Cluster` is a run of boxes inside one sphere, which a ray can skip at once.
#[derive(Clone)]
pub struct Cluster {
    pub center: DVec4,
    pub radius_m: f64,
    pub first_box: usize,
    pub box_count: usize,
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
pub struct World {
    pub curvature_radius_m: f64,
    pub boxes: Vec<HyperbolicBox>,
    pub clusters: Vec<Cluster>,
    pub mouths: Vec<Mouth>,
    pub spawn_frame: DMat4,
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
        };
        world.spawn_frame = world.make_ground_frame(SPAWN_X_M, SPAWN_Z_M, SPAWN_HEADING_RAD);
        world.build_rails();
        let house_frame: DMat4 = world.get_house_frame();
        world.group_boxes(|w| w.build_house(house_frame));
        let blue_frame: DMat4 =
            world.make_frame_from_spawn(BLUE_PYLON_BEARING_RAD, BLUE_PYLON_DISTANCE_M);
        world.group_boxes(|w| w.build_pylon(blue_frame, TEAM_BLUE));
        let red_frame: DMat4 = world.get_red_pylon_frame();
        world.group_boxes(|w| w.build_pylon(red_frame, TEAM_RED));
        let pairs: Vec<(DMat4, DMat4)> = world.make_default_wormholes();
        world.set_wormholes(&pairs);
        world.place_trees();
        world
    }

    /// `get_throat_m` gives the throat's radius in the lensing law's coordinates: a quarter of the
    /// mouth size.
    ///
    /// Example: `if rho_effective_m < world.get_throat_m() { cross(); }`
    pub fn get_throat_m(&self) -> f64 {
        THROAT_PER_HORIZON * HORIZON_RADIUS_M
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

    /// `group_boxes` runs a builder and records the boxes it adds as one cluster, with a sphere
    /// around its first box's centre that holds them all.
    ///
    /// Example: `world.group_boxes(|w| w.build_house(frame));`
    fn group_boxes(&mut self, build: impl FnOnce(&mut World)) {
        assert!(
            self.clusters.len() < MAX_CLUSTER_COUNT,
            "group_boxes: too many clusters"
        );
        let (r, first_box): (f64, usize) = (self.curvature_radius_m, self.boxes.len());
        build(self);
        assert!(
            self.boxes.len() > first_box,
            "group_boxes: the builder added no boxes"
        );
        let center: DVec4 = self.boxes[first_box].center;
        let radius_m: f64 = self.boxes[first_box..]
            .iter()
            .map(|b| {
                let box_radius_m: f64 = r * b
                    .half_m
                    .iter()
                    .map(|h| (h / r).cosh())
                    .product::<f64>()
                    .acosh();
                get_distance(center, b.center) * r + box_radius_m
            })
            .fold(0.0, f64::max);
        self.clusters.push(Cluster {
            center,
            radius_m,
            first_box,
            box_count: self.boxes.len() - first_box,
        });
    }

    /// `get_box_lower_bound_m` gives a distance a point is at least from a box: the largest of its
    /// distances past each slab (negative inside).
    ///
    /// Example: `if world.get_box_lower_bound_m(b, point) < 3.0 { skip(); }`
    pub fn get_box_lower_bound_m(&self, b: &HyperbolicBox, x: DVec4) -> f64 {
        let r: f64 = self.curvature_radius_m;
        (0..3)
            .map(|i| {
                r * crate::geometry::minkowski_dot(x, b.normals[i])
                    .abs()
                    .asinh()
                    - b.half_m[i]
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// `build_rails` adds the straight cobblestone wall ahead of spawn and seven straight walls
    /// leaving it at right angles, which would stay parallel in flat space.
    ///
    /// Example: `world.build_rails();`
    fn build_rails(&mut self) {
        let base_frame: DMat4 = self.make_ground_frame(0.0, -RAIL_START_M, 0.0);
        self.group_boxes(|w| {
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
            self.group_boxes(|w| {
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
            self.group_boxes(|w| w.build_tree(frame, height_m));
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
