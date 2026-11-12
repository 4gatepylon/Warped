//! `forest_layout` builds the Dark Forest map: a Euclidean forest, red's base at +z and blue's at
//! −z (a half turn of red's), with treehouses, rope ladders and bridges between the trees, and two
//! pylons per team (one in a treehouse, one in a palisade on the ground) with one console each.
//! `--export-map` writes it to `common/maps/dark_forest/map.json`, which the game then loads; the
//! layout lives in that file, this module only makes it.
//!
//! No wall closes the playable rectangle (W-145, no walls at map edges): past it, a band
//! `SOFT_EDGE_M` wide slows players without end (the map's `regions.boundary`), and the bamboo
//! goes on `OUTER_FOREST_DEPTH_M` past the edge (`add_outer_forest`), so a player walking off the
//! map always sees forest ahead.
//!
//! Example:
//!     let map: MapFile = forest_layout::build();
//!
//! Interface: `build`, `HALF_WIDTH_M`, `HALF_LENGTH_M`, `SOFT_EDGE_M`, `OUTER_FOREST_DEPTH_M`,
//! `DECK_TOP_M`, `LOOKOUT_TOP_M`.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::map::{
    Appearance, Boundary, Ground, Ladder, LaserLook, Light, MapBox, MapFile, Material, PlayerSpec,
    Regions, Scatter, Shape, Sky, Space, Spawn, Structure, StructureMesh, Sun, Warp, Zone,
    ZoneWalls, translation_frame, yaw_frame,
};

/// `HALF_WIDTH_M` is half the forest's extent along x.
pub const HALF_WIDTH_M: f32 = 30.0;
/// `HALF_LENGTH_M` is half the forest's extent along z, from blue's end to red's.
pub const HALF_LENGTH_M: f32 = 56.0;
/// `DECK_TOP_M` is the height of every treehouse deck and bridge.
pub const DECK_TOP_M: f32 = 7.0;
/// `LOOKOUT_TOP_M` is the height of the two lookouts on the central tree.
pub const LOOKOUT_TOP_M: f32 = 12.0;
/// `DECK_THICKNESS_M` is how thick decks and bridges are.
const DECK_THICKNESS_M: f32 = 0.3;
/// `RAIL_HEIGHT_M` is how high a bridge's top rope is above its deck.
const RAIL_HEIGHT_M: f32 = 1.0;
/// `SOFT_EDGE_M` is how far past the playable rectangle players' speed would reach 0: the band
/// where each step outward is cut more and more (`warped_shared::map_edge`).
pub const SOFT_EDGE_M: f32 = 12.0;
/// `OUTER_FOREST_DEPTH_M` is how far past the playable rectangle the bamboo goes on.
pub const OUTER_FOREST_DEPTH_M: f32 = 30.0;
/// `EDGE_DETAIL_DEPTH_M` is how far past the rectangle the outer forest's clumps are like the
/// labyrinth's with a branch per culm (`TreeDetail::Edge`); beyond it, where nobody walks, they are
/// wider clumps of bare culms under one low-detail canopy (`TreeDetail::Far`).
const EDGE_DETAIL_DEPTH_M: f32 = 14.0;
/// `FLOOR_PAST_FOREST_M` is how far the forest floor reaches past the outer forest.
const FLOOR_PAST_FOREST_M: f32 = 10.0;
/// `EDGE_CLUMP_SPACING_M` and `FAR_CLUMP_SPACING_M` are the grid steps of the outer forest's
/// clumps before jitter, near the edge and beyond it, and `OUTER_CLUMP_JITTER_FRAC` how far, as a
/// share of the step, each clump moves off its grid point.
const EDGE_CLUMP_SPACING_M: f32 = 3.2;
const FAR_CLUMP_SPACING_M: f32 = 2.6;
const OUTER_CLUMP_JITTER_FRAC: f32 = 0.4;
/// `FAR_CLUMP_MIN_RADIUS_M` and `FAR_CLUMP_RADIUS_SPREAD_M` size the far clumps, which grow
/// 2 + 4 × radius culms each (`add_tree`).
const FAR_CLUMP_MIN_RADIUS_M: f32 = 0.6;
const FAR_CLUMP_RADIUS_SPREAD_M: f32 = 0.35;
/// `FAR_CANOPY_TOP_FRAC` is how high a far clump's canopy sits, as a share of its height;
/// `FAR_CANOPY_REACH_M` how far it reaches past the clump's culms, and `FAR_CANOPY_HALF_HEIGHT_M`
/// its half thickness.
const FAR_CANOPY_TOP_FRAC: f32 = 0.85;
const FAR_CANOPY_REACH_M: f32 = 1.4;
const FAR_CANOPY_HALF_HEIGHT_M: f32 = 1.2;
/// `OUTER_FOREST_SALT` seeds the outer forest's own random sequence, so the forest inside the
/// rectangle stays as it was.
const OUTER_FOREST_SALT: u32 = 0x0e7e_d9e5;
/// `OUTER_TREE_RADIUS_M` is the footprint kept clear around each of the big outer trees.
const OUTER_TREE_RADIUS_M: f32 = 2.0;
/// `SUPPORT_RADIUS_M` is the smallest tree radius that becomes a thick bundle of bamboo (it holds
/// up a deck or a ladder) rather than a loose clump.
const SUPPORT_RADIUS_M: f32 = 1.0;
/// `SUPPORT_CULM_RADIUS_FRAC` is a bundle's culm radius per bundle radius.
const SUPPORT_CULM_RADIUS_FRAC: f32 = 0.3;
/// `SUPPORT_CULM_GAP_FRAC` is the spacing of a bundle's ring of culms per culm radius (2 touch).
const SUPPORT_CULM_GAP_FRAC: f32 = 2.1;
/// `CLUMP_SPREAD_FRAC` is how far a loose clump's culms stand from its middle per tree radius.
const CLUMP_SPREAD_FRAC: f32 = 2.2;
const THIN_CULM_RADIUS_M: f32 = 0.13;
const MAX_CULM_LEAN_RAD: f32 = 0.08;
/// `LEAF_SPRAY_COUNT` thin leaf sprays point out from each culm over its top `LEAF_SPRAY_DEPTH_M`.
const LEAF_SPRAY_COUNT: usize = 3;
const LEAF_SPRAY_DEPTH_M: f32 = 4.5;
const LEAF_HALF_LENGTH_M: f32 = 1.3;
const LEAF_HALF_WIDTH_M: f32 = 0.22;
const LEAF_HALF_THICKNESS_M: f32 = 0.03;
/// `BAMBOO_SEED` seeds the generator that places, sizes and leans the decorative bamboo.
const BAMBOO_SEED: u32 = 0x5eed_f0e5;
/// `PROP_JITTER_M` is how far a log or rock may move from its listed spot; both halves move alike.
const PROP_JITTER_M: f32 = 0.6;
/// `LOG_JITTER_SALT` and `ROCK_JITTER_SALT` give logs and rocks different jitter streams.
const LOG_JITTER_SALT: u32 = 0x1065;
const ROCK_JITTER_SALT: u32 = 0x60c4;
/// `SPAWN_JITTER_SALT` gives the back row of spawns its own jitter stream, and
/// `BACK_SPAWN_ROW_Z_M` puts that row behind the front one (at 52 m), inside the spawn's clearing,
/// so each team has six spawns (W-155, play against bots: a 5v5 and a spare player).
const SPAWN_JITTER_SALT: u32 = 0x5a0e;
const BACK_SPAWN_ROW_Z_M: f32 = 55.0;
/// `BRANCH_MIN_COUNT` to `BRANCH_MIN_COUNT + BRANCH_EXTRA_COUNT` branches fork from each culm's
/// top into a leafy crown; the crowns together are the canopy that shades the forest.
const BRANCH_MIN_COUNT: usize = 3;
const BRANCH_EXTRA_COUNT: usize = 2;
const BRANCH_RADIUS_M: f32 = 0.05;
const BRANCH_FORK_DEPTH_M: f32 = 2.5;
const BRANCH_MIN_TILT_RAD: f32 = 0.6;
const BRANCH_TILT_SPREAD_RAD: f32 = 0.5;
const BRANCH_MIN_LENGTH_M: f32 = 2.5;
const BRANCH_LENGTH_SPREAD_M: f32 = 1.5;
/// `CROWN_LEAF_COUNT` broad leaves hang off each branch's outer part.
const CROWN_LEAF_COUNT: usize = 7;
const CROWN_LEAF_HALF_LENGTH_M: f32 = 1.2;
const CROWN_LEAF_HALF_WIDTH_M: f32 = 0.55;

const AIR: u8 = 0;
const FOREST_FLOOR: u8 = 1;
const BARK: u8 = 2;
const PLANKS: u8 = 3;
const LEAVES: u8 = 4;
const ROCK: u8 = 5;
const ROPE: u8 = 6;
const RED_PAINT: u8 = 7;
const BLUE_PAINT: u8 = 8;
const RED_CONSOLE: u8 = 9;
const BLUE_CONSOLE: u8 = 10;
const CANDLE_WAX: u8 = 11;
/// `PYLON_HEIGHT_M` and `PYLON_RADIUS_M` size a pylon, in the hyperbolic demo's beacon's proportions.
const PYLON_HEIGHT_M: f32 = 3.2;
const PYLON_RADIUS_M: f32 = 0.36;
const CANDLE_FLAME: u8 = 12;
/// `CANDLE_RADIUS_M` and `CANDLE_HALF_HEIGHT_M` size a candle's wax.
const CANDLE_RADIUS_M: f32 = 0.05;
const CANDLE_HALF_HEIGHT_M: f32 = 0.1;
/// `FLAME_RADIUS_M` and `FLAME_HALF_HEIGHT_M` size a candle's flame.
const FLAME_RADIUS_M: f32 = 0.025;
const FLAME_HALF_HEIGHT_M: f32 = 0.06;
/// `CANDLE_LIGHT_RGB`, `CANDLE_LIGHT_RADIUS_M` and `CANDLE_FLICKER_FRAC` are a candle's light.
const CANDLE_LIGHT_RGB: [f32; 3] = [1.0, 0.5, 0.18];
const CANDLE_LIGHT_RADIUS_M: f32 = 3.5;
const CANDLE_FLICKER_FRAC: f32 = 0.35;
/// `CANDLE_OFFSET_PER_SUPPORT_RADIUS_FRAC` is how far from a support bundle's middle its candle stands.
const CANDLE_OFFSET_PER_SUPPORT_RADIUS_FRAC: f32 = 1.6;

/// `Side` is one team's half of the forest: red's is as written, blue's is its half turn.
#[derive(Clone, Copy)]
struct Side {
    team: &'static str,
    sign: f32,
    paint: u8,
    console: u8,
}

impl Side {
    /// `at` maps a point written for red's side to this side.
    fn at(self, x: f32, y: f32, z: f32) -> Vec3 {
        Vec3::new(x * self.sign, y, z * self.sign)
    }

    /// `yaw` maps a yaw written for red's side to this side.
    fn yaw(self, yaw_rad: f32) -> f32 {
        if self.sign > 0.0 {
            yaw_rad
        } else {
            yaw_rad + PI
        }
    }
}

const RED: Side = Side {
    team: "red",
    sign: 1.0,
    paint: RED_PAINT,
    console: RED_CONSOLE,
};
const BLUE: Side = Side {
    team: "blue",
    sign: -1.0,
    paint: BLUE_PAINT,
    console: BLUE_CONSOLE,
};

/// `TreeDetail` is how much of a bamboo clump is drawn: every branch (`Full`, inside the map), one
/// branch a culm (`Edge`, in the band players walk into) or bare culms under one canopy for the
/// whole clump (`Far`, beyond it), so the outer forest costs less to draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TreeDetail {
    Full,
    Edge,
    Far,
}

impl TreeDetail {
    /// `get_leaf_spray_count` is how many leaf sprays each culm has.
    fn get_leaf_spray_count(self) -> usize {
        match self {
            TreeDetail::Full | TreeDetail::Edge => LEAF_SPRAY_COUNT,
            TreeDetail::Far => 0,
        }
    }

    /// `get_branch_count` is how many branches a culm forks into, for a random `random_frac`.
    fn get_branch_count(self, random_frac: f32) -> usize {
        match self {
            TreeDetail::Full => {
                BRANCH_MIN_COUNT + (random_frac * (BRANCH_EXTRA_COUNT + 1) as f32) as usize
            }
            TreeDetail::Edge => 1,
            TreeDetail::Far => 0,
        }
    }
}

/// `Layout` collects the map while it is built, and the ground footprints that the decorative
/// trees keep out of.
struct Layout {
    map: MapFile,
    footprints: Vec<(Vec2, Vec2)>,
    random_state: u32,
}

impl Layout {
    /// `add_box` adds a box from its centre and half size.
    fn add_box(&mut self, group: &str, centre_m: Vec3, half_m: Vec3, material: u8) {
        let name: String = format!("box {}", self.map.boxes.len());
        self.map.boxes.push(MapBox {
            name,
            group: group.to_string(),
            frame: translation_frame(centre_m),
            half_size_m: half_m.to_array(),
            material,
        });
    }

    /// `add_box_between` adds a box from two corners.
    fn add_box_between(&mut self, group: &str, a_m: Vec3, b_m: Vec3, material: u8) {
        self.add_box(group, (a_m + b_m) * 0.5, (b_m - a_m).abs() * 0.5, material);
    }

    /// `keep_clear` marks a ground rectangle (with a margin) where no decorative tree grows.
    fn keep_clear(&mut self, centre: Vec2, half: Vec2) {
        self.footprints.push((centre, half + Vec2::splat(1.6)));
    }

    /// `add_tree` adds a bamboo clump where a tree of `radius_m` and `height_m` would stand: a
    /// bundle of thick, upright culms as wide as the trunk if it holds up a deck or ladder
    /// (`radius_m` at least `SUPPORT_RADIUS_M`), otherwise a loose clump of thin, leaning culms.
    /// Every culm has leaf sprays near its top. Blue's clumps are red's turned by 180°.
    fn add_tree(&mut self, name: &str, foot_m: Vec3, radius_m: f32, height_m: f32) {
        self.add_tree_in_detail(name, foot_m, radius_m, height_m, TreeDetail::Full);
    }

    /// `add_tree_in_detail` is `add_tree` drawn in `detail`.
    fn add_tree_in_detail(
        &mut self,
        name: &str,
        foot_m: Vec3,
        radius_m: f32,
        height_m: f32,
        detail: TreeDetail,
    ) {
        let is_turned: bool = foot_m.z < 0.0 || (foot_m.z == 0.0 && foot_m.x < 0.0);
        let turn: f32 = if is_turned { -1.0 } else { 1.0 };
        let mut random_state: u32 = get_clump_seed(foot_m);
        let mut random_frac = || {
            random_state ^= random_state << 13;
            random_state ^= random_state >> 17;
            random_state ^= random_state << 5;
            (random_state >> 8) as f32 / (1u32 << 24) as f32
        };
        let mut culms: Vec<(Vec2, f32, f32, f32)> = Vec::new();
        if radius_m >= SUPPORT_RADIUS_M {
            self.add_candle(
                &format!("{name} candle"),
                foot_m + Vec3::X * (radius_m * CANDLE_OFFSET_PER_SUPPORT_RADIUS_FRAC * turn),
            );
            let culm_radius_m: f32 = radius_m * SUPPORT_CULM_RADIUS_FRAC;
            let ring_radius_m: f32 = radius_m - culm_radius_m;
            let ring_count: usize =
                (TAU * ring_radius_m / (SUPPORT_CULM_GAP_FRAC * culm_radius_m)) as usize;
            for i in 0..ring_count {
                let angle_rad: f32 = i as f32 / ring_count as f32 * TAU;
                let culm_height_m: f32 = height_m * (0.8 + 0.2 * random_frac());
                culms.push((
                    Vec2::from_angle(angle_rad) * ring_radius_m,
                    culm_radius_m,
                    culm_height_m,
                    0.0,
                ));
            }
            let core_radius_m: f32 = (ring_radius_m - culm_radius_m).max(culm_radius_m);
            culms.push((Vec2::ZERO, core_radius_m, height_m, 0.0));
        } else {
            let culm_count: usize = 2 + (radius_m * 4.0) as usize;
            for i in 0..culm_count {
                let angle_rad: f32 = (i as f32 + 0.6 * random_frac()) / culm_count as f32 * TAU;
                let spread_m: f32 = radius_m * CLUMP_SPREAD_FRAC * (0.35 + 0.65 * random_frac());
                let culm_radius_m: f32 = THIN_CULM_RADIUS_M * (0.6 + 0.9 * random_frac());
                let culm_height_m: f32 = height_m * (0.6 + 0.4 * random_frac());
                culms.push((
                    Vec2::from_angle(angle_rad) * spread_m,
                    culm_radius_m,
                    culm_height_m,
                    MAX_CULM_LEAN_RAD * random_frac(),
                ));
            }
        }
        for (index, (offset, culm_radius_m, culm_height_m, lean_rad)) in
            culms.into_iter().enumerate()
        {
            let outward: Vec2 = offset.try_normalize().unwrap_or(Vec2::X) * turn;
            let tilt: Quat = Quat::from_axis_angle(Vec3::new(outward.y, 0.0, -outward.x), lean_rad);
            let yaw_rad: f32 = random_frac() * TAU + if is_turned { PI } else { 0.0 };
            let culm_foot_m: Vec3 = foot_m + Vec3::new(offset.x, 0.0, offset.y) * turn;
            let frame: Mat4 = Mat4::from_translation(culm_foot_m)
                * Mat4::from_quat(tilt * Quat::from_rotation_y(yaw_rad));
            let mut shapes: Vec<Shape> = vec![Shape {
                kind: "cylinder".to_string(),
                centre_m: [0.0, culm_height_m * 0.5, 0.0],
                half_m: [culm_radius_m, culm_height_m * 0.5, culm_radius_m],
                round_m: 0.0,
                axis: 1,
                material: BARK,
            }];
            for spray in 0..detail.get_leaf_spray_count() {
                let sign: f32 = if spray % 4 < 2 { 1.0 } else { -1.0 };
                let y_m: f32 =
                    culm_height_m - LEAF_SPRAY_DEPTH_M * spray as f32 / LEAF_SPRAY_COUNT as f32;
                let reach_m: f32 = culm_radius_m + LEAF_HALF_LENGTH_M * 0.9;
                let (centre_m, half_m) = if spray % 2 == 0 {
                    (
                        [sign * reach_m, y_m, 0.0],
                        [LEAF_HALF_LENGTH_M, LEAF_HALF_THICKNESS_M, LEAF_HALF_WIDTH_M],
                    )
                } else {
                    (
                        [0.0, y_m, sign * reach_m],
                        [LEAF_HALF_WIDTH_M, LEAF_HALF_THICKNESS_M, LEAF_HALF_LENGTH_M],
                    )
                };
                shapes.push(Shape {
                    kind: "ellipsoid".to_string(),
                    centre_m,
                    half_m,
                    round_m: 0.0,
                    axis: 1,
                    material: LEAVES,
                });
            }
            self.map.structures.push(Structure {
                name: format!("{name} culm {index}"),
                mesh: None,
                frame: frame.to_cols_array(),
                shapes,
            });
            let branch_count: usize = detail.get_branch_count(random_frac());
            for branch in 0..branch_count {
                let azimuth_rad: f32 =
                    (branch as f32 + 0.6 * random_frac()) / branch_count as f32 * TAU;
                let tilt_rad: f32 = BRANCH_MIN_TILT_RAD + BRANCH_TILT_SPREAD_RAD * random_frac();
                let length_m: f32 = BRANCH_MIN_LENGTH_M + BRANCH_LENGTH_SPREAD_M * random_frac();
                let fork_y_m: f32 = culm_height_m - BRANCH_FORK_DEPTH_M * random_frac();
                let branch_frame: Mat4 = frame
                    * Mat4::from_translation(Vec3::new(0.0, fork_y_m, 0.0))
                    * Mat4::from_quat(
                        Quat::from_rotation_y(azimuth_rad) * Quat::from_rotation_z(-tilt_rad),
                    );
                let mut branch_shapes: Vec<Shape> = vec![Shape {
                    kind: "cylinder".to_string(),
                    centre_m: [0.0, length_m * 0.5, 0.0],
                    half_m: [BRANCH_RADIUS_M, length_m * 0.5, BRANCH_RADIUS_M],
                    round_m: 0.0,
                    axis: 1,
                    material: BARK,
                }];
                for leaf in 0..CROWN_LEAF_COUNT {
                    let along_frac: f32 = 0.4 + 0.6 * leaf as f32 / (CROWN_LEAF_COUNT - 1) as f32;
                    let sign: f32 = if leaf % 2 == 0 { 1.0 } else { -1.0 };
                    let reach_m: f32 = CROWN_LEAF_HALF_LENGTH_M * 0.8;
                    let (centre_m, half_m) = if leaf % 3 == 2 {
                        (
                            [sign * reach_m, along_frac * length_m, 0.0],
                            [
                                CROWN_LEAF_HALF_LENGTH_M,
                                LEAF_HALF_THICKNESS_M,
                                CROWN_LEAF_HALF_WIDTH_M,
                            ],
                        )
                    } else {
                        (
                            [0.0, along_frac * length_m, sign * reach_m],
                            [
                                CROWN_LEAF_HALF_WIDTH_M,
                                LEAF_HALF_THICKNESS_M,
                                CROWN_LEAF_HALF_LENGTH_M,
                            ],
                        )
                    };
                    branch_shapes.push(Shape {
                        kind: "ellipsoid".to_string(),
                        centre_m,
                        half_m,
                        round_m: 0.0,
                        axis: 1,
                        material: LEAVES,
                    });
                }
                self.map.structures.push(Structure {
                    name: format!("{name} culm {index} branch {branch}"),
                    mesh: None,
                    frame: branch_frame.to_cols_array(),
                    shapes: branch_shapes,
                });
            }
        }
        if detail == TreeDetail::Far {
            let reach_m: f32 = radius_m * CLUMP_SPREAD_FRAC + FAR_CANOPY_REACH_M;
            self.map.structures.push(Structure {
                name: format!("{name} canopy"),
                mesh: None,
                frame: translation_frame(foot_m + Vec3::Y * (height_m * FAR_CANOPY_TOP_FRAC)),
                shapes: vec![Shape {
                    kind: "ellipsoid".to_string(),
                    centre_m: [0.0, 0.0, 0.0],
                    half_m: [reach_m, FAR_CANOPY_HALF_HEIGHT_M, reach_m],
                    round_m: 0.0,
                    axis: 1,
                    material: LEAVES,
                }],
            });
        }
    }

    /// `add_log` adds a fallen log along x or z, lying on the ground.
    fn add_log(
        &mut self,
        name: &str,
        centre: Vec2,
        half_length_m: f32,
        radius_m: f32,
        is_along_x: bool,
    ) {
        let (axis, half_m) = if is_along_x {
            (0, [half_length_m, radius_m, radius_m])
        } else {
            (2, [radius_m, radius_m, half_length_m])
        };
        self.map.structures.push(Structure {
            name: name.to_string(),
            mesh: None,
            frame: translation_frame(Vec3::new(centre.x, radius_m, centre.y)),
            shapes: vec![Shape {
                kind: "cylinder".to_string(),
                centre_m: [0.0, 0.0, 0.0],
                half_m,
                round_m: 0.0,
                axis,
                material: BARK,
            }],
        });
    }

    /// `add_candle` adds a candle standing at `foot_m`, with a flickering warm light above it.
    fn add_candle(&mut self, name: &str, foot_m: Vec3) {
        let flame_y_m: f32 = 2.0 * CANDLE_HALF_HEIGHT_M + FLAME_HALF_HEIGHT_M;
        self.map.structures.push(Structure {
            name: name.to_string(),
            mesh: None,
            frame: translation_frame(foot_m),
            shapes: vec![
                Shape {
                    kind: "cylinder".to_string(),
                    centre_m: [0.0, CANDLE_HALF_HEIGHT_M, 0.0],
                    half_m: [CANDLE_RADIUS_M, CANDLE_HALF_HEIGHT_M, CANDLE_RADIUS_M],
                    round_m: 0.0,
                    axis: 1,
                    material: CANDLE_WAX,
                },
                Shape {
                    kind: "ellipsoid".to_string(),
                    centre_m: [0.0, flame_y_m, 0.0],
                    half_m: [FLAME_RADIUS_M, FLAME_HALF_HEIGHT_M, FLAME_RADIUS_M],
                    round_m: 0.0,
                    axis: 1,
                    material: CANDLE_FLAME,
                },
            ],
        });
        self.map.lights.push(Light {
            position: (foot_m + Vec3::Y * (flame_y_m + 2.0 * FLAME_HALF_HEIGHT_M))
                .extend(1.0)
                .to_array(),
            color: CANDLE_LIGHT_RGB,
            radius_m: CANDLE_LIGHT_RADIUS_M,
            flicker_frac: Some(CANDLE_FLICKER_FRAC),
        });
    }

    /// `add_deck` adds a square deck whose top is at `top_m`.
    fn add_deck(&mut self, group: &str, centre: Vec2, half: Vec2, top_m: f32) {
        self.add_box(
            group,
            Vec3::new(centre.x, top_m - DECK_THICKNESS_M * 0.5, centre.y),
            Vec3::new(half.x, DECK_THICKNESS_M * 0.5, half.y),
            PLANKS,
        );
        self.keep_clear(centre, half);
        for (index, corner) in [Vec2::new(0.8, 0.8), Vec2::new(-0.8, -0.8)]
            .into_iter()
            .enumerate()
        {
            let point: Vec2 = centre + half * corner * get_half_turn(centre);
            self.add_candle(
                &format!("{group} candle {index}"),
                Vec3::new(point.x, top_m, point.y),
            );
        }
        self.map.lights.push(Light {
            position: [centre.x, top_m + 2.4, centre.y, 1.0],
            color: [1.0, 0.62, 0.3],
            radius_m: 5.0,
            flicker_frac: None,
        });
    }

    /// `add_bridge` adds a plank bridge with rope rails at deck height between two points on the
    /// same x or the same z.
    fn add_bridge(&mut self, group: &str, from: Vec2, to: Vec2) {
        let is_along_x: bool = (to.x - from.x).abs() > (to.y - from.y).abs();
        let half_width_m: f32 = 1.0;
        let (low, high) = (from.min(to), from.max(to));
        let across: Vec2 = if is_along_x { Vec2::Y } else { Vec2::X };
        let low_corner: Vec2 = low - across * half_width_m;
        let high_corner: Vec2 = high + across * half_width_m;
        self.add_box_between(
            group,
            Vec3::new(low_corner.x, DECK_TOP_M - 0.12, low_corner.y),
            Vec3::new(high_corner.x, DECK_TOP_M, high_corner.y),
            PLANKS,
        );
        self.keep_clear((low + high) * 0.5, (high_corner - low_corner) * 0.5);
        for side in [-1.0_f32, 1.0] {
            let offset: Vec2 = across * (half_width_m + 0.04) * side;
            for (bottom_m, top_m) in [(0.5, 0.55), (RAIL_HEIGHT_M - 0.05, RAIL_HEIGHT_M)] {
                let a: Vec2 = low + offset - across * 0.03;
                let b: Vec2 = high + offset + across * 0.03;
                self.add_box_between(
                    group,
                    Vec3::new(a.x, DECK_TOP_M + bottom_m, a.y),
                    Vec3::new(b.x, DECK_TOP_M + top_m, b.y),
                    ROPE,
                );
            }
            let length_m: f32 = (high - low).length();
            let post_count: usize = (length_m / 3.0).ceil() as usize;
            for index in 0..=post_count {
                let point: Vec2 = low.lerp(high, index as f32 / post_count as f32) + offset;
                self.add_box(
                    group,
                    Vec3::new(point.x, DECK_TOP_M + RAIL_HEIGHT_M * 0.5, point.y),
                    Vec3::new(0.06, RAIL_HEIGHT_M * 0.5, 0.06),
                    PLANKS,
                );
            }
        }
        let middle_on_path: Vec2 = low.lerp(high, 0.5);
        let middle: Vec2 =
            middle_on_path + across * (half_width_m + 0.04) * get_half_turn(middle_on_path);
        self.add_candle(
            &format!("{group} candle"),
            Vec3::new(middle.x, DECK_TOP_M + RAIL_HEIGHT_M, middle.y),
        );
    }

    /// `add_ladder` adds a rope ladder from `bottom_m` to `top_m`, hanging at `centre` with its
    /// climber facing `yaw_rad`.
    fn add_ladder(&mut self, name: &str, centre: Vec2, yaw_rad: f32, bottom_m: f32, top_m: f32) {
        let half_height_m: f32 = (top_m - bottom_m + 1.0) * 0.5;
        self.map.ladders.push(Ladder {
            name: name.to_string(),
            frame: yaw_frame(
                Vec3::new(centre.x, bottom_m + half_height_m, centre.y),
                yaw_rad,
            ),
            half_size_m: [0.5, half_height_m, 0.3],
        });
        self.keep_clear(centre, Vec2::splat(1.0));
    }

    /// `add_pylon` adds a team's pylon: the shared `scifi_beacon` model, whose round collider is
    /// its console, and a light.
    fn add_pylon(&mut self, side: Side, index: usize, foot_m: Vec3) {
        self.map.structures.push(Structure {
            name: format!("{} pylon {index}", side.team),
            mesh: Some(StructureMesh {
                model: "scifi_beacon".to_string(),
                height_m: PYLON_HEIGHT_M,
            }),
            frame: translation_frame(foot_m),
            shapes: vec![Shape {
                kind: "cylinder".to_string(),
                centre_m: [0.0, PYLON_HEIGHT_M * 0.5, 0.0],
                half_m: [PYLON_RADIUS_M, PYLON_HEIGHT_M * 0.5, PYLON_RADIUS_M],
                round_m: 0.0,
                axis: 1,
                material: side.console,
            }],
        });
        let color: [f32; 3] = if side.sign > 0.0 {
            [1.0, 0.2, 0.12]
        } else {
            [0.2, 0.4, 1.0]
        };
        self.map.lights.push(Light {
            position: [foot_m.x, foot_m.y + PYLON_HEIGHT_M + 0.3, foot_m.z, 1.0],
            color,
            radius_m: 3.5,
            flicker_frac: None,
        });
    }

    /// `random_frac` is a pseudo-random number in [0, 1), the same on every run.
    fn random_frac(&mut self) -> f32 {
        self.random_state ^= self.random_state << 13;
        self.random_state ^= self.random_state >> 17;
        self.random_state ^= self.random_state << 5;
        (self.random_state >> 8) as f32 / (1u32 << 24) as f32
    }

    /// `is_clear` tells whether a tree of this radius may grow at a ground point.
    fn is_clear(&self, point: Vec2, radius_m: f32) -> bool {
        self.footprints.iter().all(|(centre, half)| {
            let gap: Vec2 = ((point - *centre).abs() - *half).max(Vec2::ZERO);
            gap.length() > radius_m
        })
    }
}

/// `GOLDEN_WHITE_LASER_RGB` is every team's laser colour here, the house map's warm white core
/// turned golden (the user: yellow lasers for Dust and Dark Forest).
const GOLDEN_WHITE_LASER_RGB: [f32; 3] = [1.0, 0.86, 0.6];

/// `MOUTH_HEIGHT_M` puts a mouth's centre at eye height, so walking into it carries the eye
/// through its throat.
const MOUTH_HEIGHT_M: f32 = 1.7;
const MOUTH_SCHWARZSCHILD_RADIUS_M: f32 = 3.2;
const MOUTH_REACH_M: f32 = 5.0;
const MOUTH_LAMBDA_SQUARED: f32 = 0.25;
/// `MOUTH_PAIRS_XZ_M` are the wormholes' mouths on the ground: red's flank gate, blue's flank
/// gate (red's turned by 180°) and the centre gate (its own turn); every mouth's −z faces blue, so
/// walking toward the enemy through one comes out still heading toward the enemy.
const MOUTH_PAIRS_XZ_M: [[Vec2; 2]; 3] = [
    [Vec2::new(-22.0, 20.0), Vec2::new(22.0, -8.0)],
    [Vec2::new(22.0, -20.0), Vec2::new(-22.0, 8.0)],
    [Vec2::new(-2.0, 24.0), Vec2::new(2.0, -24.0)],
];

/// `LIFT_MOUTHS_M` is red's lift, a wormhole from the ground to the open corner of its treehouse
/// deck; blue's is turned by 180°. `LIFT_SCHWARZSCHILD_RADIUS_M` and `LIFT_REACH_M` keep its
/// mouths small enough to fit beside the deck's tree and bridge.
const LIFT_MOUTHS_M: [Vec3; 2] = [
    Vec3::new(-4.0, MOUTH_HEIGHT_M, 36.0),
    Vec3::new(10.3, DECK_TOP_M + MOUTH_HEIGHT_M, 38.8),
];
const LIFT_SCHWARZSCHILD_RADIUS_M: f32 = 2.4;
const LIFT_REACH_M: f32 = 3.0;

/// `add_warps` adds the five wormholes (three across the ground, each team's lift) and keeps trees
/// out of their reach.
fn add_warps(layout: &mut Layout) {
    let mut gates: Vec<([Vec3; 2], f32, f32)> = MOUTH_PAIRS_XZ_M
        .iter()
        .map(|pair| {
            (
                pair.map(|xz_m| Vec3::new(xz_m.x, MOUTH_HEIGHT_M, xz_m.y)),
                MOUTH_SCHWARZSCHILD_RADIUS_M,
                MOUTH_REACH_M,
            )
        })
        .collect();
    for side in [RED, BLUE] {
        gates.push((
            LIFT_MOUTHS_M.map(|m| side.at(m.x, m.y, m.z)),
            LIFT_SCHWARZSCHILD_RADIUS_M,
            LIFT_REACH_M,
        ));
    }
    for (pair_index, (ends_m, schwarzschild_radius_m, reach_m)) in gates.into_iter().enumerate() {
        let first_index: usize = layout.map.warps.len();
        for (end_index, centre_m) in ends_m.into_iter().enumerate() {
            layout.keep_clear(Vec2::new(centre_m.x, centre_m.z), Vec2::splat(reach_m));
            layout.map.warps.push(Warp {
                kind: "wormhole_mouth".to_string(),
                name: format!("gate {pair_index} mouth {end_index}"),
                frame: Mat4::from_translation(centre_m).to_cols_array(),
                law: "foil".to_string(),
                schwarzschild_radius_m,
                lambda_squared: Some(MOUTH_LAMBDA_SQUARED),
                radial_coordinate: "distance".to_string(),
                reach_m,
                falloff: "taper".to_string(),
                partner: Some(first_index + 1 - end_index),
            });
        }
    }
}

/// `add_team_side` adds one team's base: spawns, its home treehouse with a pylon, the deck
/// chain toward the centre, and the palisade with its ground pylon.
fn add_team_side(layout: &mut Layout, side: Side) {
    let team: &str = side.team;
    for x in [-6.0, -2.0, 2.0, 6.0] {
        layout.map.spawns.push(Spawn {
            team: team.to_string(),
            frame: yaw_frame(side.at(x, 0.0, 52.0), side.yaw(0.0)),
        });
    }
    for (index, x) in [-4.0, 4.0].into_iter().enumerate() {
        let jitter_m: Vec2 = get_prop_jitter_m(SPAWN_JITTER_SALT, index);
        layout.map.spawns.push(Spawn {
            team: team.to_string(),
            frame: yaw_frame(
                side.at(x + jitter_m.x, 0.0, BACK_SPAWN_ROW_Z_M + jitter_m.y),
                side.yaw(0.0),
            ),
        });
    }
    let flat = |point: Vec3| Vec2::new(point.x, point.z);
    layout.keep_clear(flat(side.at(0.0, 0.0, 52.0)), Vec2::new(8.0, 3.0));

    let home: Vec3 = side.at(14.0, 0.0, 42.0);
    layout.add_tree(&format!("{team} home tree"), home, 1.4, 24.0);
    let group: String = format!("{team} treehouse");
    layout.add_deck(&group, flat(home), Vec2::splat(5.0), DECK_TOP_M);
    let hut_low: Vec3 = side.at(16.4, DECK_TOP_M, 39.0);
    let hut_high: Vec3 = side.at(18.9, DECK_TOP_M + 2.4, 45.0);
    let (low, high) = (hut_low.min(hut_high), hut_low.max(hut_high));
    let wall_m: f32 = 0.12;
    let door_low: f32 = side.at(0.0, 0.0, 41.0).z.min(side.at(0.0, 0.0, 43.0).z);
    let door_high: f32 = side.at(0.0, 0.0, 41.0).z.max(side.at(0.0, 0.0, 43.0).z);
    let inner_x: f32 = if side.sign > 0.0 { low.x } else { high.x };
    let outer_x: f32 = if side.sign > 0.0 { high.x } else { low.x };
    layout.add_box_between(
        &group,
        Vec3::new(outer_x - wall_m * side.sign, low.y, low.z),
        Vec3::new(outer_x, high.y, high.z),
        PLANKS,
    );
    for z in [low.z, high.z - wall_m] {
        layout.add_box_between(
            &group,
            Vec3::new(low.x, low.y, z),
            Vec3::new(high.x, high.y, z + wall_m),
            PLANKS,
        );
    }
    for (z0, z1) in [(low.z, door_low), (door_high, high.z)] {
        layout.add_box_between(
            &group,
            Vec3::new(inner_x, low.y, z0),
            Vec3::new(inner_x + wall_m * side.sign, high.y, z1),
            PLANKS,
        );
    }
    layout.add_box_between(
        &group,
        Vec3::new(inner_x, DECK_TOP_M + 2.0, door_low),
        Vec3::new(inner_x + wall_m * side.sign, high.y, door_high),
        PLANKS,
    );
    layout.add_box_between(
        &group,
        Vec3::new(low.x - 0.2, high.y, low.z - 0.2),
        Vec3::new(high.x + 0.2, high.y + 0.15, high.z + 0.2),
        side.paint,
    );
    layout.add_pylon(side, 1, side.at(17.7, DECK_TOP_M, 42.0));
    layout.add_ladder(
        &format!("{team} treehouse ladder"),
        flat(side.at(11.0, 0.0, 47.3)),
        side.yaw(0.0),
        0.0,
        DECK_TOP_M,
    );

    let middle: Vec3 = side.at(14.0, 0.0, 22.0);
    layout.add_tree(&format!("{team} middle tree"), middle, 1.0, 22.0);
    layout.add_deck(
        &format!("{team} middle deck"),
        flat(middle),
        Vec2::splat(3.5),
        DECK_TOP_M,
    );
    layout.add_bridge(
        &format!("{team} home bridge"),
        flat(side.at(14.0, 0.0, 37.0)),
        flat(side.at(14.0, 0.0, 25.5)),
    );
    layout.add_ladder(
        &format!("{team} middle ladder"),
        flat(side.at(10.2, 0.0, 22.0)),
        side.yaw(-FRAC_PI_2),
        0.0,
        DECK_TOP_M,
    );

    let flank: Vec3 = side.at(14.0, 0.0, 0.0);
    layout.add_tree(&format!("{team} flank tree"), flank, 1.0, 22.0);
    layout.add_deck(
        &format!("{team} flank deck"),
        flat(flank),
        Vec2::splat(3.0),
        DECK_TOP_M,
    );
    layout.add_bridge(
        &format!("{team} flank bridge"),
        flat(side.at(14.0, 0.0, 18.5)),
        flat(side.at(14.0, 0.0, 3.0)),
    );
    layout.add_bridge(
        &format!("{team} centre bridge"),
        flat(side.at(11.0, 0.0, 0.0)),
        flat(side.at(4.5, 0.0, 0.0)),
    );
    layout.add_ladder(
        &format!("{team} flank ladder"),
        flat(side.at(17.3, 0.0, 0.0)),
        side.yaw(FRAC_PI_2),
        0.0,
        DECK_TOP_M,
    );
    layout.add_deck(
        &format!("{team} lookout"),
        flat(side.at(0.0, 0.0, 3.5)),
        Vec2::new(2.5, 1.2),
        LOOKOUT_TOP_M,
    );
    layout.add_ladder(
        &format!("{team} lookout ladder"),
        flat(side.at(2.8, 0.0, 3.5)),
        side.yaw(FRAC_PI_2),
        DECK_TOP_M,
        LOOKOUT_TOP_M,
    );
    layout.add_ladder(
        &format!("{team} centre ladder"),
        flat(side.at(-2.5, 0.0, 4.8)),
        side.yaw(0.0),
        0.0,
        DECK_TOP_M,
    );

    let palisade: String = format!("{team} palisade");
    let (x0, x1, z0, z1) = (-18.0_f32, -10.0_f32, 40.0_f32, 48.0_f32);
    let wall_height_m: f32 = 2.6;
    let segments: [(f32, f32, f32, f32); 5] = [
        (x0, z1 - 0.4, x1, z1),
        (x0, z0, x0 + 0.4, z1),
        (x1 - 0.4, z0, x1, z1),
        (x0, z0, -15.5, z0 + 0.4),
        (-12.5, z0, x1, z0 + 0.4),
    ];
    for (ax, az, bx, bz) in segments {
        layout.add_box_between(
            &palisade,
            side.at(ax, 0.0, az),
            side.at(bx, wall_height_m, bz),
            BARK,
        );
    }
    layout.keep_clear(flat(side.at(-14.0, 0.0, 44.0)), Vec2::splat(4.0));
    layout.add_pylon(side, 0, side.at(-14.0, 0.0, 44.0));
    layout.map.lights.push(Light {
        position: side.at(-14.0, 3.2, 41.0).extend(1.0).to_array(),
        color: [1.0, 0.62, 0.3],
        radius_m: 5.0,
        flicker_frac: None,
    });

    for (index, (x, z, is_along_x)) in [
        (-4.0, 30.0, true),
        (6.0, 14.0, false),
        (-16.0, 2.0, true),
        (22.0, 32.0, false),
    ]
    .into_iter()
    .enumerate()
    {
        let jitter_m: Vec2 = get_prop_jitter_m(LOG_JITTER_SALT, index);
        let centre: Vec2 = flat(side.at(x + jitter_m.x, 0.0, z + jitter_m.y));
        layout.add_log(
            &format!("{team} log {index}"),
            centre,
            2.6,
            0.45,
            is_along_x,
        );
        let half: Vec2 = if is_along_x {
            Vec2::new(2.6, 0.45)
        } else {
            Vec2::new(0.45, 2.6)
        };
        layout.keep_clear(centre, half);
    }
    for (index, (x, z, half)) in [
        (-8.0, 20.0, 1.2),
        (22.0, 12.0, 1.0),
        (-22.0, 28.0, 1.4),
        (4.0, 38.0, 0.9),
    ]
    .into_iter()
    .enumerate()
    {
        let jitter_m: Vec2 = get_prop_jitter_m(ROCK_JITTER_SALT, index);
        let centre: Vec3 = side.at(x + jitter_m.x, half * 0.6, z + jitter_m.y);
        layout.add_box(
            &format!("{team} rock {index}"),
            centre,
            Vec3::new(half, half * 0.6, half * 0.8),
            ROCK,
        );
        layout.keep_clear(Vec2::new(centre.x, centre.z), Vec2::splat(half));
    }
}

/// `MAZE_CELL_M` is the side of a cell of the ground labyrinth: a corridor and its wall.
const MAZE_CELL_M: f32 = 4.0;
/// `MAZE_LOOP_FRAC` is the share of a spanning maze's walls opened anyway, so the labyrinth has
/// loops as well as dead ends.
const MAZE_LOOP_FRAC: f32 = 0.12;
/// `MAZE_TURN_FRAC` is how often carving the maze turns rather than going straight, so corridors
/// wind.
const MAZE_TURN_FRAC: f32 = 0.7;
/// `MAZE_HALFWAY_OPEN_FRAC` is the share of the walls on the halfway line left open.
const MAZE_HALFWAY_OPEN_FRAC: f32 = 0.35;
/// `MAZE_CLUMPS_PER_WALL` bamboo clumps stand along each closed cell wall, `MAZE_WALL_JITTER_M`
/// off its line at most.
const MAZE_CLUMPS_PER_WALL: usize = 4;
const MAZE_WALL_JITTER_M: f32 = 0.25;
const MAZE_CLUMP_MIN_RADIUS_M: f32 = 0.25;
const MAZE_CLUMP_RADIUS_SPREAD_M: f32 = 0.15;

/// `add_labyrinth` carves red's half of the ground into a seeded maze of `MAZE_CELL_M` cells (a
/// winding depth-first spanning tree plus some loops), grows a bamboo wall on every closed cell
/// side where nothing else stands, and mirrors it onto blue's half. Walls on the halfway line are
/// red's for its west half, so their 180° turns close the east half.
fn add_labyrinth(layout: &mut Layout) {
    let column_count: usize = (2.0 * HALF_WIDTH_M / MAZE_CELL_M) as usize;
    let row_count: usize = (HALF_LENGTH_M / MAZE_CELL_M) as usize;
    let index = |i: usize, j: usize| j * column_count + i;
    let mut is_open_east: Vec<bool> = vec![false; column_count * row_count];
    let mut is_open_north: Vec<bool> = vec![false; column_count * row_count];
    let mut is_visited: Vec<bool> = vec![false; column_count * row_count];
    let start: (usize, usize) = (column_count / 2, row_count - 1);
    is_visited[index(start.0, start.1)] = true;
    let mut stack: Vec<(usize, usize, Option<usize>)> = vec![(start.0, start.1, None)];
    while let Some(&(i, j, last_direction)) = stack.last() {
        let mut options: Vec<(usize, usize, usize)> = Vec::new();
        if i + 1 < column_count {
            options.push((i + 1, j, 0));
        }
        if i > 0 {
            options.push((i - 1, j, 1));
        }
        if j + 1 < row_count {
            options.push((i, j + 1, 2));
        }
        if j > 0 {
            options.push((i, j - 1, 3));
        }
        options.retain(|&(a, b, _)| !is_visited[index(a, b)]);
        if options.is_empty() {
            stack.pop();
            continue;
        }
        let turns: Vec<(usize, usize, usize)> = options
            .iter()
            .copied()
            .filter(|option| Some(option.2) != last_direction)
            .collect();
        let pool: &[(usize, usize, usize)] =
            if !turns.is_empty() && layout.random_frac() < MAZE_TURN_FRAC {
                &turns
            } else {
                &options
            };
        let (a, b, direction) =
            pool[((layout.random_frac() * pool.len() as f32) as usize).min(pool.len() - 1)];
        match direction {
            0 => is_open_east[index(i, j)] = true,
            1 => is_open_east[index(a, b)] = true,
            2 => is_open_north[index(i, j)] = true,
            _ => is_open_north[index(a, b)] = true,
        }
        is_visited[index(a, b)] = true;
        stack.push((a, b, Some(direction)));
    }
    let corner = |i: usize, j: usize| {
        Vec2::new(
            -HALF_WIDTH_M + i as f32 * MAZE_CELL_M,
            j as f32 * MAZE_CELL_M,
        )
    };
    let mut walls: Vec<(Vec2, Vec2)> = Vec::new();
    for j in 0..row_count {
        for i in 0..column_count {
            let is_inner_east: bool = i + 1 < column_count;
            if is_inner_east && !is_open_east[index(i, j)] && layout.random_frac() >= MAZE_LOOP_FRAC
            {
                walls.push((corner(i + 1, j), corner(i + 1, j + 1)));
            }
            let is_inner_north: bool = j + 1 < row_count;
            if is_inner_north
                && !is_open_north[index(i, j)]
                && layout.random_frac() >= MAZE_LOOP_FRAC
            {
                walls.push((corner(i, j + 1), corner(i + 1, j + 1)));
            }
        }
    }
    for i in 0..column_count / 2 {
        if layout.random_frac() >= MAZE_HALFWAY_OPEN_FRAC {
            walls.push((corner(i, 0), corner(i + 1, 0)));
        }
    }
    let mut count: usize = 0;
    for (a, b) in walls {
        let along: Vec2 = (b - a) / MAZE_CLUMPS_PER_WALL as f32;
        let across: Vec2 = along.perp().normalize();
        for clump in 0..MAZE_CLUMPS_PER_WALL {
            let along_frac: f32 = clump as f32 + 0.5 + (layout.random_frac() - 0.5) * 0.6;
            let point: Vec2 = a
                + along * along_frac
                + across * ((layout.random_frac() - 0.5) * 2.0 * MAZE_WALL_JITTER_M);
            let radius_m: f32 =
                MAZE_CLUMP_MIN_RADIUS_M + MAZE_CLUMP_RADIUS_SPREAD_M * layout.random_frac();
            let height_m: f32 = 16.0 + layout.random_frac() * 12.0;
            let is_inside: bool =
                point.x.abs() < HALF_WIDTH_M - 1.0 && point.y.abs() < HALF_LENGTH_M - 1.0;
            if !is_inside || !layout.is_clear(point, radius_m * CLUMP_SPREAD_FRAC + 0.3) {
                continue;
            }
            for (side, sign) in [("red", 1.0_f32), ("blue", -1.0)] {
                let foot_m: Vec3 = Vec3::new(point.x * sign, 0.0, point.y * sign);
                layout.add_tree(&format!("{side} tree {count}"), foot_m, radius_m, height_m);
            }
            count += 1;
        }
    }
}

/// `add_outer_forest` grows bamboo past the playable rectangle out to `OUTER_FOREST_DEPTH_M`, clear
/// of the rectangle (by each clump's spread) and of the big outer trees: clumps like the
/// labyrinth's (`TreeDetail::Edge`) out to `EDGE_DETAIL_DEPTH_M`, then wider, denser clumps of bare
/// culms under one canopy each (`TreeDetail::Far`), so that looking out from anywhere a player can
/// reach shows bamboo, not the open beyond. It grows red's half (z > 0) and turns it half a turn
/// onto blue's, as the labyrinth does, with its own seeded sequence; the row on the halfway line
/// is red's west of the middle, so its half turn fills the east and no lane runs along that line.
fn add_outer_forest(layout: &mut Layout) {
    let outer_feet_m: Vec<Vec3> = layout
        .map
        .structures
        .iter()
        .filter(|tree| tree.name.starts_with("outer ") && tree.name.ends_with(" culm 0"))
        .map(|tree| crate::map::frame_translation_m(&tree.frame))
        .collect();
    for foot_m in outer_feet_m {
        layout.keep_clear(
            Vec2::new(foot_m.x, foot_m.z),
            Vec2::splat(OUTER_TREE_RADIUS_M),
        );
    }
    let mut random_state: u32 = BAMBOO_SEED ^ OUTER_FOREST_SALT;
    let mut count: usize = 0;
    for (detail, spacing_m, depth_m, min_radius_m, radius_spread_m) in [
        (
            TreeDetail::Edge,
            EDGE_CLUMP_SPACING_M,
            [0.0, EDGE_DETAIL_DEPTH_M],
            MAZE_CLUMP_MIN_RADIUS_M,
            MAZE_CLUMP_RADIUS_SPREAD_M,
        ),
        (
            TreeDetail::Far,
            FAR_CLUMP_SPACING_M,
            [EDGE_DETAIL_DEPTH_M, OUTER_FOREST_DEPTH_M],
            FAR_CLUMP_MIN_RADIUS_M,
            FAR_CLUMP_RADIUS_SPREAD_M,
        ),
    ] {
        let reach_m: Vec2 = Vec2::new(HALF_WIDTH_M, HALF_LENGTH_M) + OUTER_FOREST_DEPTH_M;
        let column_count: usize = (2.0 * reach_m.x / spacing_m).ceil() as usize;
        let row_count: usize = (reach_m.y / spacing_m).ceil() as usize + 1;
        for j in 0..row_count {
            for i in 0..column_count {
                let mut random_frac = || {
                    random_state ^= random_state << 13;
                    random_state ^= random_state >> 17;
                    random_state ^= random_state << 5;
                    (random_state >> 8) as f32 / (1u32 << 24) as f32
                };
                let jitter: Vec2 = Vec2::new(random_frac() - 0.5, random_frac() - 0.5)
                    * 2.0
                    * OUTER_CLUMP_JITTER_FRAC
                    * spacing_m;
                let point: Vec2 = Vec2::new(
                    -reach_m.x + (i as f32 + 0.5) * spacing_m,
                    j as f32 * spacing_m,
                ) + jitter;
                let radius_m: f32 = min_radius_m + radius_spread_m * random_frac();
                let height_m: f32 = 16.0 + random_frac() * 12.0;
                let past_m: f32 = Vec2::new(
                    (point.x.abs() - HALF_WIDTH_M).max(0.0),
                    (point.y.abs() - HALF_LENGTH_M).max(0.0),
                )
                .length();
                let spread_m: f32 = radius_m * CLUMP_SPREAD_FRAC + 0.3;
                if (j == 0 && point.x >= 0.0)
                    || past_m < depth_m[0].max(spread_m)
                    || past_m >= depth_m[1]
                    || !layout.is_clear(point, spread_m)
                {
                    continue;
                }
                for (side, sign) in [("red", 1.0_f32), ("blue", -1.0)] {
                    let foot_m: Vec3 = Vec3::new(point.x * sign, 0.0, point.y * sign);
                    layout.add_tree_in_detail(
                        &format!("{side} outer forest {count}"),
                        foot_m,
                        radius_m,
                        height_m,
                        detail,
                    );
                }
                count += 1;
            }
        }
    }
}

/// `materials` is the map's palette.
fn materials() -> Vec<Material> {
    let entry = |id: u8, name: &str, color: [f32; 3], kind: &str| Material {
        id,
        name: name.to_string(),
        color,
        kind: kind.to_string(),
    };
    vec![
        entry(AIR, "air", [0.0, 0.0, 0.0], "empty"),
        entry(FOREST_FLOOR, "forest floor", [0.24, 0.19, 0.12], "solid"),
        entry(BARK, "bamboo", [0.42, 0.36, 0.22], "solid"),
        entry(PLANKS, "planks", [0.52, 0.38, 0.24], "solid"),
        entry(LEAVES, "leaves", [0.16, 0.3, 0.1], "solid"),
        entry(ROCK, "rock", [0.4, 0.4, 0.37], "solid"),
        entry(ROPE, "rope", [0.62, 0.52, 0.34], "solid"),
        entry(RED_PAINT, "red paint", [0.62, 0.12, 0.08], "solid"),
        entry(BLUE_PAINT, "blue paint", [0.1, 0.22, 0.62], "solid"),
        entry(RED_CONSOLE, "red console", [1.0, 0.22, 0.12], "light"),
        entry(BLUE_CONSOLE, "blue console", [0.2, 0.45, 1.0], "light"),
        entry(CANDLE_WAX, "candle wax", [0.85, 0.8, 0.65], "solid"),
        entry(CANDLE_FLAME, "candle flame", [1.0, 0.6, 0.25], "light"),
    ]
}

/// `get_clump_seed` gives a bamboo clump's random seed from where it stands, the same for red's
/// clump and blue's clump turned by 180°.
fn get_clump_seed(foot_m: Vec3) -> u32 {
    let x: u32 = (foot_m.x.abs() * 64.0) as u32;
    let z: u32 = (foot_m.z.abs() * 64.0) as u32;
    (x.wrapping_mul(73_856_093) ^ z.wrapping_mul(19_349_663)) | 1
}

/// `get_prop_jitter_m` is the seeded offset of prop `index` of a kind, written for red's side so
/// mirroring gives blue the same offset.
fn get_prop_jitter_m(salt: u32, index: usize) -> Vec2 {
    let mut state: u32 = (BAMBOO_SEED
        ^ salt.wrapping_mul(0x9e37_79b9)
        ^ (index as u32 + 1).wrapping_mul(0x85eb_ca6b))
        | 1;
    let mut next_frac = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    Vec2::new(next_frac() - 0.5, next_frac() - 0.5) * 2.0 * PROP_JITTER_M
}

/// `get_half_turn` is 1 for a point on red's half and -1 on blue's, so a detail placed with it
/// turns half a turn with its half.
fn get_half_turn(point: Vec2) -> f32 {
    if point.y < 0.0 || (point.y == 0.0 && point.x < 0.0) {
        -1.0
    } else {
        1.0
    }
}

/// `unit_direction` scales a direction to length 1, in f64 so the map's validator agrees.
fn unit_direction(direction: [f64; 3]) -> [f64; 3] {
    let length: f64 = direction.iter().map(|x| x * x).sum::<f64>().sqrt();
    direction.map(|x| x / length)
}

/// `ZONE_EDGE_U_M` is where each team's zone begins, along Fermi u (−z): blue's spawns and pylons
/// are beyond +24 m, red's beyond −24 m, and the middle of the forest is between.
const ZONE_EDGE_U_M: f32 = 24.0;
/// `ZONE_WALL_HEIGHT_M` is how tall the team boundary planes are: above the treehouses' decks
/// (7 m) and a player's eye on them.
const ZONE_WALL_HEIGHT_M: f32 = 10.0;
/// `ZONE_WALL_THICKNESS_M` is every map's zone walls' thickness (Lunar Surface's).
const ZONE_WALL_THICKNESS_M: f32 = 0.6;
/// `ZONE_WALL_OPACITY_FRAC` is how opaque the planes are at their foot: fainter than the other
/// maps' 0.25, which hides the dark forest behind a plane.
const ZONE_WALL_OPACITY_FRAC: f32 = 0.1;

/// `make_regions` gives the forest's team zones (blue toward −z, red toward +z) and their walls,
/// the team boundary planes (W-148, team boundary planes in every level).
///
/// Example: `let regions: Regions = make_regions();`
fn make_regions() -> Regions {
    let zone = |team: &str, u_min_m: Option<f32>, u_max_m: Option<f32>| Zone {
        team: team.to_string(),
        u_min_m,
        u_max_m,
    };
    Regions {
        zones: vec![
            zone("blue", Some(ZONE_EDGE_U_M), None),
            zone("middle", Some(-ZONE_EDGE_U_M), Some(ZONE_EDGE_U_M)),
            zone("red", None, Some(-ZONE_EDGE_U_M)),
        ],
        boundary: None,
        zone_walls: Some(ZoneWalls {
            height_m: ZONE_WALL_HEIGHT_M,
            thickness_m: ZONE_WALL_THICKNESS_M,
            opacity_frac: ZONE_WALL_OPACITY_FRAC,
        }),
    }
}

/// `build` makes the whole Dark Forest map.
///
/// Example: `let map: MapFile = build();`
pub fn build() -> MapFile {
    let mut layout: Layout = Layout {
        map: MapFile {
            format: "warped-map".to_string(),
            version: 1,
            name: "dark_forest".to_string(),
            description: "Dark Forest: a Euclidean forest with treehouses, rope ladders and bridges, two pylons per team, slowing players without end past its edge"
                .to_string(),
            source: "demos/dark_forest_rust/src/forest_layout.rs".to_string(),
            space: Space {
                kind: "euclidean".to_string(),
            },
            gravity_m_per_s2: 6.0,
            player: PlayerSpec {
                eye_height_m: 1.68,
                walk_m_per_s: 5.0,
                sprint_m_per_s: 9.0,
                jump_m_per_s: 5.2,
                fly_m_per_s: 6.0,
            },
            materials: materials(),
            ground: Ground {
                material: FOREST_FLOOR,
            },
            structures: Vec::new(),
            boxes: Vec::new(),
            ladders: Vec::new(),
            warps: Vec::new(),
            scatter: Some(Scatter {
                seed: BAMBOO_SEED,
                spacing_m: MAZE_CELL_M,
                grow_frac: 1.0 - MAZE_LOOP_FRAC,
            }),
            laser: Some(LaserLook {
                rgb: GOLDEN_WHITE_LASER_RGB,
            }),
            lights: Vec::new(),
            sky: Sky {
                transport: "parallel".to_string(),
                sun: Sun {
                    direction: unit_direction([0.45, 0.2, -0.82]),
                    color: [0.62, 0.78, 0.55],
                },
                stars: false,
                clouds: true,
                zenith_color: [0.18, 0.24, 0.22],
                horizon_color: [0.36, 0.4, 0.32],
                fog_m: Some(40.0),
            },
            spawns: Vec::new(),
            regions: Some(Regions {
                boundary: Some(Boundary {
                    kind: "fermi_rectangle".to_string(),
                    u_m: HALF_LENGTH_M,
                    v_m: HALF_WIDTH_M,
                    soft_edge_m: Some(SOFT_EDGE_M),
                }),
                ..make_regions()
            }),
            appearance: Appearance {
                style: "forest".to_string(),
            lighting: serde_json::json!({"model": "ambient_sun", "ambient": 0.6, "sun": 3.7}),
        },
        },
        footprints: Vec::new(),
        random_state: BAMBOO_SEED,
    };
    let margin_m: f32 = OUTER_FOREST_DEPTH_M + FLOOR_PAST_FOREST_M;
    layout.add_box_between(
        "the forest floor",
        Vec3::new(-HALF_WIDTH_M - margin_m, -0.5, -HALF_LENGTH_M - margin_m),
        Vec3::new(HALF_WIDTH_M + margin_m, 0.0, HALF_LENGTH_M + margin_m),
        FOREST_FLOOR,
    );
    let (w, l) = (HALF_WIDTH_M, HALF_LENGTH_M);
    layout.add_tree("the great tree", Vec3::ZERO, 2.2, 30.0);
    layout.add_deck(
        "the great tree deck",
        Vec2::ZERO,
        Vec2::splat(4.5),
        DECK_TOP_M,
    );
    for side in [RED, BLUE] {
        add_team_side(&mut layout, side);
    }
    add_warps(&mut layout);
    add_labyrinth(&mut layout);
    for index in 0..10 {
        let t_frac: f32 = index as f32 / 10.0;
        for (sign_x, sign_z) in [(1.0_f32, 1.0_f32), (-1.0, -1.0)] {
            let x: f32 = (-w + 4.0 + t_frac * 2.0 * (w - 4.0)) * sign_x;
            let z: f32 = (l + 5.0) * sign_z;
            layout.add_tree(
                &format!("outer tree {index} {sign_z}"),
                Vec3::new(x, 0.0, z),
                0.9,
                24.0,
            );
            let side_z: f32 = (-l + 6.0 + t_frac * 2.0 * (l - 6.0)) * sign_z;
            layout.add_tree(
                &format!("outer side tree {index} {sign_x}"),
                Vec3::new((w + 5.0) * sign_x, 0.0, side_z),
                0.9,
                24.0,
            );
        }
    }
    add_outer_forest(&mut layout);
    layout.map
}

/// `tests` checks the layout's invariants.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | teams | red, blue | `test_each_team_has_two_consoles_and_spawns` |
/// | ladders | to a deck, to a lookout | `test_ladders_reach_decks` |
/// | symmetry | red's half against blue's | `test_halves_mirror` |
/// | warps | no black holes and 5 paired wormholes, each reach clear of boxes, shapes, ladders and spawns | `test_warps_are_paired_and_clear` |
/// | walking into a mouth | each of the 10 mouths, from 16 directions | `test_every_mouth_lets_out_onto_solid_ground` |
/// | the map file | as this layout exports it, to rounding | `test_the_map_file_is_this_layout` |
/// | the map's edge (W-145, no walls at map edges) | each of the 4 sides: no box past the rectangle, mostly open ground just past it, a soft-edge boundary in the map | `test_no_wall_closes_the_map` |
/// | looking out at eye height, against the culms | from the edge and from where the slowing stops players, on each of the 4 sides, from 60° left to 60° right of straight out | `test_looking_off_the_map_shows_forest` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::collision::Aabb;
    use crate::game::world::Arena;
    use crate::map::frame_translation_m;

    /// `EDGE_SAMPLE_STEP_M` is the spacing of the points sampled along each side of the rectangle.
    const EDGE_SAMPLE_STEP_M: f32 = 1.0;
    /// `BODY_HALF_M` is a standing player's collision box's half size.
    const BODY_HALF_M: Vec3 = Vec3::new(0.3, 0.9, 0.3);
    /// `MIN_OPEN_EDGE_FRAC` is the least share of points just past an edge where a player fits.
    const MIN_OPEN_EDGE_FRAC: f32 = 0.5;
    /// `EYE_HEIGHT_M` is a standing player's eye height, and `LOOK_STEP_DEG` and `MAX_LOOK_DEG` the
    /// directions looked in, either side of straight out.
    const EYE_HEIGHT_M: f32 = 1.68;
    const LOOK_STEP_DEG: f32 = 10.0;
    const MAX_LOOK_DEG: f32 = 60.0;
    /// `MAX_SEE_THROUGH_FRAC` is the most of the looks out that may pass the whole outer forest.
    const MAX_SEE_THROUGH_FRAC: f32 = 0.03;

    /// `get_edge_points` gives points along the 4 sides of the rectangle grown by `past_m`, every
    /// `EDGE_SAMPLE_STEP_M`, each with the side's outward direction.
    fn get_edge_points(past_m: f32) -> Vec<(Vec3, Vec3)> {
        let (w, l) = (HALF_WIDTH_M + past_m, HALF_LENGTH_M + past_m);
        let mut points: Vec<(Vec3, Vec3)> = Vec::new();
        for sign in [1.0_f32, -1.0] {
            let along_x_count: usize = (2.0 * HALF_WIDTH_M / EDGE_SAMPLE_STEP_M) as usize;
            for i in 0..=along_x_count {
                let x: f32 = -HALF_WIDTH_M + i as f32 * EDGE_SAMPLE_STEP_M;
                points.push((Vec3::new(x, 0.0, l * sign), Vec3::Z * sign));
            }
            let along_z_count: usize = (2.0 * HALF_LENGTH_M / EDGE_SAMPLE_STEP_M) as usize;
            for i in 0..=along_z_count {
                let z: f32 = -HALF_LENGTH_M + i as f32 * EDGE_SAMPLE_STEP_M;
                points.push((Vec3::new(w * sign, 0.0, z), Vec3::X * sign));
            }
        }
        points
    }

    /// `MAP_FILE_TOLERANCE` is how far a number in the map file may be from this layout's: debug
    /// and release builds round the layout's trigonometry differently in the last bits.
    const MAP_FILE_TOLERANCE: f64 = 1e-5;

    /// `find_mismatch` gives the path of the first place where JSON values `a` and `b` differ
    /// beyond `MAP_FILE_TOLERANCE`, if any.
    fn find_mismatch(a: &serde_json::Value, b: &serde_json::Value, path: &str) -> Option<String> {
        use serde_json::Value;
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => {
                let (x, y) = (x.as_f64()?, y.as_f64()?);
                ((x - y).abs() > MAP_FILE_TOLERANCE).then(|| format!("{path}: {x} against {y}"))
            }
            (Value::Array(xs), Value::Array(ys)) if xs.len() == ys.len() => xs
                .iter()
                .zip(ys)
                .enumerate()
                .find_map(|(i, (x, y))| find_mismatch(x, y, &format!("{path}/{i}"))),
            (Value::Object(xs), Value::Object(ys)) if xs.len() == ys.len() => {
                xs.iter().find_map(|(key, x)| match ys.get(key) {
                    Some(y) => find_mismatch(x, y, &format!("{path}/{key}")),
                    None => Some(format!("{path}/{key}: missing")),
                })
            }
            _ => (a != b).then(|| path.to_string()),
        }
    }

    #[test]
    fn test_the_map_file_is_this_layout() {
        let file: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::map::default_map_path()).expect("the map file"),
        )
        .expect("the map file's JSON");
        let layout: serde_json::Value = serde_json::to_value(build()).expect("map JSON");
        let mismatch: Option<String> = find_mismatch(&file, &layout, "");
        assert!(
            mismatch.is_none(),
            "{mismatch:?}: run `make export-map` to write this layout to the map file"
        );
    }

    #[test]
    fn test_no_wall_closes_the_map() {
        let map: MapFile = build();
        let boundary: &Boundary = map
            .regions
            .as_ref()
            .and_then(|regions| regions.boundary.as_ref())
            .expect("the map's boundary");
        assert_eq!(
            (boundary.kind.as_str(), boundary.u_m, boundary.v_m),
            ("fermi_rectangle", HALF_LENGTH_M, HALF_WIDTH_M)
        );
        assert_eq!(boundary.soft_edge_m, Some(SOFT_EDGE_M));
        for map_box in &map.boxes[1..] {
            let (low_m, high_m) = world_bounds(
                Mat4::from_cols_array(&map_box.frame),
                Vec3::from(map_box.half_size_m),
            );
            assert!(
                low_m.x >= -HALF_WIDTH_M
                    && high_m.x <= HALF_WIDTH_M
                    && low_m.z >= -HALF_LENGTH_M
                    && high_m.z <= HALF_LENGTH_M,
                "{} ({}) reaches past the edge",
                map_box.name,
                map_box.group
            );
        }
        let solids: Vec<Aabb> = Arena::from_map(&map)
            .boxes
            .iter()
            .map(|world_box| world_box.aabb)
            .collect();
        let points: Vec<(Vec3, Vec3)> = get_edge_points(BODY_HALF_M.x + 0.05);
        for side in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
            let side_points: Vec<Vec3> = points
                .iter()
                .filter(|(_, out)| *out == side)
                .map(|(point_m, _)| *point_m)
                .collect();
            let open_count: usize = side_points
                .iter()
                .filter(|point_m| {
                    let body: Aabb = Aabb::from_center(
                        **point_m + Vec3::Y * (BODY_HALF_M.y + 0.05),
                        BODY_HALF_M,
                    );
                    !solids.iter().any(|solid| solid.overlaps(&body))
                })
                .count();
            let open_frac: f32 = open_count as f32 / side_points.len() as f32;
            assert!(open_frac > MIN_OPEN_EDGE_FRAC, "side {side}: {open_frac}");
        }
    }

    /// `get_culms_at_height` gives where every upright culm (a cylinder along its frame's y) of
    /// `map` crosses the height `y_m`, and its radius: what blocks a level look at that height.
    fn get_culms_at_height(map: &MapFile, y_m: f32) -> Vec<(Vec2, f32)> {
        map.structures
            .iter()
            .filter_map(|structure| {
                let shape: &Shape = structure.shapes.first()?;
                let is_culm: bool = shape.kind == "cylinder" && shape.axis == 1;
                let frame: Mat4 = Mat4::from_cols_array(&structure.frame);
                let foot_m: Vec3 = frame.transform_point3(Vec3::ZERO);
                let up: Vec3 = frame.transform_vector3(Vec3::Y);
                let length_m: f32 = 2.0 * shape.half_m[1];
                let along_m: f32 = (y_m - foot_m.y) / up.y;
                let crossing_m: Vec3 = foot_m + up * along_m;
                (is_culm && up.y > 0.9 && along_m >= 0.0 && along_m <= length_m)
                    .then(|| (Vec2::new(crossing_m.x, crossing_m.z), shape.half_m[0]))
            })
            .collect()
    }

    /// `is_circle_on_ray` says whether the ray from `origin` along the unit `direction` meets the
    /// circle at `centre` of `radius_m` within `length_m`.
    fn is_circle_on_ray(
        origin: Vec2,
        direction: Vec2,
        length_m: f32,
        centre: Vec2,
        radius_m: f32,
    ) -> bool {
        let along_m: f32 = (centre - origin).dot(direction).clamp(0.0, length_m);
        (origin + direction * along_m).distance(centre) <= radius_m
    }

    #[test]
    fn test_looking_off_the_map_shows_forest() {
        let culms_at_eye: Vec<(Vec2, f32)> = get_culms_at_height(&build(), EYE_HEIGHT_M);
        let look_count: usize = (2.0 * MAX_LOOK_DEG / LOOK_STEP_DEG) as usize + 1;
        for past_m in [0.0, SOFT_EDGE_M] {
            let (mut see_through_count, mut look_total) = (0_usize, 0_usize);
            for (point_m, out) in get_edge_points(past_m) {
                let eye_m: Vec3 = point_m + Vec3::Y * EYE_HEIGHT_M;
                for k in 0..look_count {
                    let turn_rad: f32 = (-MAX_LOOK_DEG + k as f32 * LOOK_STEP_DEG).to_radians();
                    let direction: Vec3 = Quat::from_rotation_y(turn_rad) * out;
                    let out_of_forest_m: f32 = (OUTER_FOREST_DEPTH_M - past_m) / turn_rad.cos();
                    look_total += 1;
                    let is_blocked: bool = culms_at_eye.iter().any(|&(centre, radius_m)| {
                        is_circle_on_ray(
                            Vec2::new(eye_m.x, eye_m.z),
                            Vec2::new(direction.x, direction.z),
                            out_of_forest_m,
                            centre,
                            radius_m,
                        )
                    });
                    see_through_count += usize::from(!is_blocked);
                }
            }
            let see_through_frac: f32 = see_through_count as f32 / look_total as f32;
            println!(
                "from {past_m} m past the edge: {see_through_frac} of the looks pass the forest"
            );
            assert!(
                see_through_frac <= MAX_SEE_THROUGH_FRAC,
                "from {past_m} m past the edge: {see_through_frac}"
            );
        }
    }

    /// `test_each_team_has_two_consoles_and_spawns` checks the objective's pieces exist.
    #[test]
    fn test_each_team_has_two_consoles_and_spawns() {
        let map: MapFile = build();
        for (console, team) in [(RED_CONSOLE, "red"), (BLUE_CONSOLE, "blue")] {
            let pylon_count: usize = map
                .structures
                .iter()
                .filter(|s| {
                    s.mesh.is_some() && s.shapes.iter().any(|shape| shape.material == console)
                })
                .count();
            assert_eq!(pylon_count, 2);
            assert_eq!(map.spawns.iter().filter(|s| s.team == team).count(), 6);
        }
    }

    /// `test_ladders_reach_decks` checks every ladder's top rises a meter above a deck height.
    #[test]
    fn test_ladders_reach_decks() {
        let map: MapFile = build();
        assert_eq!(map.ladders.len(), 10);
        for ladder in &map.ladders {
            let top_m: f32 = frame_translation_m(&ladder.frame).y + ladder.half_size_m[1];
            assert!(
                (top_m - DECK_TOP_M - 1.0).abs() < 1e-3
                    || (top_m - LOOKOUT_TOP_M - 1.0).abs() < 1e-3,
                "{} tops out at {top_m}",
                ladder.name
            );
        }
    }

    /// `test_halves_mirror` checks every red-named structure has a blue twin a half turn away.
    /// `world_bounds` gives the world-space bounds of a box of half size `half_m` in `frame`.
    fn world_bounds(frame: Mat4, half_m: Vec3) -> (Vec3, Vec3) {
        let corners = (0..8).map(|i: u32| {
            let sign: Vec3 = Vec3::new(
                if i & 1 == 0 { -1.0 } else { 1.0 },
                if i & 2 == 0 { -1.0 } else { 1.0 },
                if i & 4 == 0 { -1.0 } else { 1.0 },
            );
            frame.transform_point3(sign * half_m)
        });
        corners.fold(
            (Vec3::INFINITY, Vec3::NEG_INFINITY),
            |(low, high), corner| (low.min(corner), high.max(corner)),
        )
    }

    /// `test_every_mouth_lets_out_onto_solid_ground` walks a body into every mouth from 16
    /// directions and checks it steps out of the partner with ground or a deck under its feet
    /// (within 0.6 m), without overlapping a solid.
    #[test]
    fn test_every_mouth_lets_out_onto_solid_ground() {
        use crate::game::collision::Aabb;
        let arena: crate::game::world::Arena = crate::game::world::Arena::from_map(&build());
        let solids: Vec<Aabb> = arena.boxes.iter().map(|world_box| world_box.aabb).collect();
        let body_centre_above_feet_m: f32 = 0.9;
        let body_half_width_m: f32 = 0.4;
        let ground_reach_m: f32 = 0.6;
        let step_m: f32 = 0.02;
        let direction_count: usize = 16;
        let mut problems: Vec<String> = Vec::new();
        for (index, mouth) in arena.warps.mouths.iter().enumerate() {
            let floor_y_m: f32 = mouth.centre_m.y - MOUTH_HEIGHT_M;
            let walk_m: f32 = mouth.throat_radius_m + 1.0;
            for direction in 0..direction_count {
                let angle_rad: f32 = TAU * direction as f32 / direction_count as f32;
                let toward: Vec3 = Vec3::new(angle_rad.cos(), 0.0, angle_rad.sin());
                let start_m: Vec3 = (mouth.centre_m - toward * walk_m).with_y(floor_y_m);
                let exit_feet_m: Option<Vec3> =
                    (0..(2.0 * walk_m / step_m) as usize).find_map(|step| {
                        let feet_m: Vec3 = start_m + toward * (step as f32 * step_m);
                        let centre_m: Vec3 = feet_m + Vec3::Y * body_centre_above_feet_m;
                        arena
                            .warps
                            .get_mouth_exit_feet_m(feet_m, centre_m)
                            .map(|(exit_m, _)| exit_m)
                    });
                let Some(exit_feet_m) = exit_feet_m else {
                    problems.push(format!(
                        "mouth {index} from {angle_rad:.2} rad: never entered"
                    ));
                    continue;
                };
                let half_width: Vec3 = Vec3::new(body_half_width_m, 0.0, body_half_width_m);
                let under_feet: Aabb = Aabb {
                    min_m: exit_feet_m - half_width - Vec3::Y * ground_reach_m,
                    max_m: exit_feet_m + half_width,
                };
                let has_ground: bool = exit_feet_m.y < ground_reach_m
                    || solids.iter().any(|solid| solid.overlaps(&under_feet));
                let body: Aabb = Aabb {
                    min_m: exit_feet_m - half_width + Vec3::Y * 0.3,
                    max_m: exit_feet_m + half_width + Vec3::Y * 1.8,
                };
                if !has_ground || solids.iter().any(|solid| solid.overlaps(&body)) {
                    problems.push(format!(
                        "mouth {index} from {angle_rad:.2} rad: out at {exit_feet_m:.2?}, ground under it: {has_ground}"
                    ));
                }
            }
        }
        assert!(problems.is_empty(), "{problems:#?}");
    }

    /// `test_warps_are_paired_and_clear` checks the map has no black holes and 5 wormholes whose
    /// mouths point at each other, and that nothing but the ground is within any warp's reach.
    #[test]
    fn test_warps_are_paired_and_clear() {
        let map: MapFile = build();
        let black_hole_count: usize = map.warps.iter().filter(|w| w.kind == "black_hole").count();
        assert_eq!((black_hole_count, map.warps.len()), (0, 10));
        for (index, warp) in map.warps.iter().enumerate() {
            if let Some(partner) = warp.partner {
                assert_eq!(map.warps[partner].partner, Some(index), "{}", warp.name);
            }
        }
        let mut bounds: Vec<(String, (Vec3, Vec3))> = Vec::new();
        for map_box in map.boxes.iter().filter(|b| b.group != "the forest floor") {
            let frame: Mat4 = Mat4::from_cols_array(&map_box.frame);
            bounds.push((
                map_box.name.clone(),
                world_bounds(frame, Vec3::from(map_box.half_size_m)),
            ));
        }
        for structure in map.structures.iter().filter(|s| !s.name.contains("candle")) {
            let frame: Mat4 = Mat4::from_cols_array(&structure.frame);
            for shape in &structure.shapes {
                let shape_frame: Mat4 = frame * Mat4::from_translation(Vec3::from(shape.centre_m));
                bounds.push((
                    structure.name.clone(),
                    world_bounds(shape_frame, Vec3::from(shape.half_m)),
                ));
            }
        }
        for ladder in &map.ladders {
            let frame: Mat4 = Mat4::from_cols_array(&ladder.frame);
            bounds.push((
                ladder.name.clone(),
                world_bounds(frame, Vec3::from(ladder.half_size_m)),
            ));
        }
        let mut crowded_warps: Vec<String> = Vec::new();
        for warp in &map.warps {
            let centre_m: Vec3 = Mat4::from_cols_array(&warp.frame).w_axis.truncate();
            for (name, (low, high)) in &bounds {
                let is_floor_below: bool = centre_m.x > low.x
                    && centre_m.x < high.x
                    && centre_m.z > low.z
                    && centre_m.z < high.z
                    && centre_m.y - high.y > 0.0
                    && centre_m.y - high.y < MOUTH_HEIGHT_M + 0.1;
                if is_floor_below {
                    continue;
                }
                let gap_m: f32 = (centre_m.clamp(*low, *high) - centre_m).length();
                if gap_m <= warp.reach_m {
                    crowded_warps.push(format!("{name} ({gap_m:.1} m) in {}", warp.name));
                }
            }
            for spawn in &map.spawns {
                let spawn_m: Vec3 = Mat4::from_cols_array(&spawn.frame).w_axis.truncate();
                assert!(
                    spawn_m.distance(centre_m) > warp.reach_m + 2.0,
                    "a spawn is by {}",
                    warp.name
                );
            }
            for other in &map.warps {
                let other_m: Vec3 = Mat4::from_cols_array(&other.frame).w_axis.truncate();
                let distance_m: f32 = other_m.distance(centre_m);
                assert!(distance_m == 0.0 || distance_m > warp.reach_m + other.reach_m);
            }
        }
        assert!(crowded_warps.is_empty(), "{crowded_warps:#?}");
    }

    /// `test_labyrinth_links_every_ground_goal` floods a walk grid from red's first spawn, around
    /// every box and trunk a body can't pass at walking height, and checks that every spawn,
    /// ground pylon, ladder foot and ground wormhole mouth is reached.
    #[test]
    fn test_labyrinth_links_every_ground_goal() {
        const CELL_M: f32 = 0.25;
        const BODY_RADIUS_M: f32 = 0.35;
        const STEP_M: f32 = 0.4;
        const HEAD_M: f32 = 1.8;
        const GOAL_TOLERANCE_M: f32 = 1.2;
        let map: MapFile = build();
        let column_count: usize = (2.0 * HALF_WIDTH_M / CELL_M) as usize;
        let row_count: usize = (2.0 * HALF_LENGTH_M / CELL_M) as usize;
        let to_cell = |x_m: f32, z_m: f32| -> (usize, usize) {
            (
                (((x_m + HALF_WIDTH_M) / CELL_M).max(0.0) as usize).min(column_count - 1),
                (((z_m + HALF_LENGTH_M) / CELL_M).max(0.0) as usize).min(row_count - 1),
            )
        };
        let mut is_blocked: Vec<bool> = vec![false; column_count * row_count];
        let mut block = |low: Vec3, high: Vec3| {
            if high.y < STEP_M || low.y > HEAD_M {
                return;
            }
            let (i_low, j_low) = to_cell(low.x - BODY_RADIUS_M, low.z - BODY_RADIUS_M);
            let (i_high, j_high) = to_cell(high.x + BODY_RADIUS_M, high.z + BODY_RADIUS_M);
            for j in j_low..=j_high {
                for i in i_low..=i_high {
                    is_blocked[j * column_count + i] = true;
                }
            }
        };
        for map_box in map.boxes.iter().filter(|b| b.group != "the forest floor") {
            let (low, high) = world_bounds(
                Mat4::from_cols_array(&map_box.frame),
                Vec3::from(map_box.half_size_m),
            );
            block(low, high);
        }
        for structure in &map.structures {
            let frame: Mat4 = Mat4::from_cols_array(&structure.frame);
            for shape in structure
                .shapes
                .iter()
                .filter(|s| s.kind == "cylinder" && s.axis == 1)
            {
                let shape_frame: Mat4 = frame * Mat4::from_translation(Vec3::from(shape.centre_m));
                let foot_m: Vec3 =
                    shape_frame.transform_point3(Vec3::new(0.0, -shape.half_m[1], 0.0));
                let top_m: Vec3 =
                    shape_frame.transform_point3(Vec3::new(0.0, shape.half_m[1], 0.0));
                let t: f32 = ((1.0 - foot_m.y) / (top_m.y - foot_m.y)).clamp(0.0, 1.0);
                let waist_m: Vec3 = foot_m.lerp(top_m, t);
                let radius: Vec3 = Vec3::new(shape.half_m[0], 0.0, shape.half_m[2]);
                block(
                    Vec3::new(waist_m.x, foot_m.y, waist_m.z) - radius,
                    Vec3::new(waist_m.x, top_m.y.min(HEAD_M), waist_m.z) + radius,
                );
            }
        }
        let start_m: Vec3 = Mat4::from_cols_array(&map.spawns[0].frame)
            .w_axis
            .truncate();
        let mut is_reached: Vec<bool> = vec![false; column_count * row_count];
        let (i_start, j_start) = to_cell(start_m.x, start_m.z);
        assert!(
            !is_blocked[j_start * column_count + i_start],
            "the first spawn is blocked"
        );
        let mut queue: std::collections::VecDeque<(usize, usize)> = [(i_start, j_start)].into();
        is_reached[j_start * column_count + i_start] = true;
        while let Some((i, j)) = queue.pop_front() {
            let neighbours = [
                (i.wrapping_sub(1), j),
                (i + 1, j),
                (i, j.wrapping_sub(1)),
                (i, j + 1),
            ];
            for (a, b) in neighbours {
                if a < column_count && b < row_count {
                    let cell: usize = b * column_count + a;
                    if !is_blocked[cell] && !is_reached[cell] {
                        is_reached[cell] = true;
                        queue.push_back((a, b));
                    }
                }
            }
        }
        let mut goals: Vec<(String, Vec3)> = Vec::new();
        for spawn in &map.spawns {
            goals.push((
                format!("{} spawn", spawn.team),
                Mat4::from_cols_array(&spawn.frame).w_axis.truncate(),
            ));
        }
        for structure in map.structures.iter().filter(|s| s.mesh.is_some()) {
            goals.push((
                structure.name.clone(),
                frame_translation_m(&structure.frame),
            ));
        }
        for ladder in &map.ladders {
            let (low, _) = world_bounds(
                Mat4::from_cols_array(&ladder.frame),
                Vec3::from(ladder.half_size_m),
            );
            goals.push((
                ladder.name.clone(),
                Vec3::new(ladder.frame[12], low.y, ladder.frame[14]),
            ));
        }
        for warp in &map.warps {
            goals.push((
                warp.name.clone(),
                Mat4::from_cols_array(&warp.frame).w_axis.truncate(),
            ));
        }
        let tolerance_cell_count: i32 = (GOAL_TOLERANCE_M / CELL_M) as i32;
        let unreached: Vec<&String> = goals
            .iter()
            .filter(|(_, goal_m)| goal_m.y < MOUTH_HEIGHT_M + 1.0)
            .filter(|(_, goal_m)| {
                let (i, j) = to_cell(goal_m.x, goal_m.z);
                let is_near_reached = (-tolerance_cell_count..=tolerance_cell_count).any(|dj| {
                    (-tolerance_cell_count..=tolerance_cell_count).any(|di| {
                        let (a, b) = (i as i32 + di, j as i32 + dj);
                        a >= 0
                            && b >= 0
                            && (a as usize) < column_count
                            && (b as usize) < row_count
                            && is_reached[b as usize * column_count + a as usize]
                    })
                });
                !is_near_reached
            })
            .map(|(name, _)| name)
            .collect();
        assert!(unreached.is_empty(), "unreached: {unreached:?}");
    }

    #[test]
    fn test_halves_mirror() {
        let map: MapFile = build();
        let origins = |prefix: &str| -> Vec<Vec3> {
            map.structures
                .iter()
                .filter(|s| s.name.starts_with(prefix))
                .map(|s| frame_translation_m(&s.frame) * Vec3::new(1.0, 1.0, 1.0))
                .collect()
        };
        let red: Vec<Vec3> = origins("red ");
        let blue: Vec<Vec3> = origins("blue ");
        assert_eq!(red.len(), blue.len());
        for (r, b) in red.iter().zip(&blue) {
            assert!((*r * Vec3::new(-1.0, 1.0, -1.0) - *b).length() < 1e-3);
        }
    }
}
