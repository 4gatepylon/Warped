//! `forest_layout` builds the Dark Forest map: a bounded Euclidean forest, red's base at +z and
//! blue's at −z (a half turn of red's), with treehouses, rope ladders and bridges between the
//! trees, and two pylons per team (one in a treehouse, one in a palisade on the ground) with one
//! console each. `--export-map` writes it to `common/maps/dark_forest/map.json`, which the game
//! then loads; the layout lives in that file, this module only makes it.
//!
//! Example:
//!     let map: MapFile = forest_layout::build();
//!
//! Interface: `build`, `HALF_WIDTH_M`, `HALF_LENGTH_M`, `DECK_TOP_M`, `LOOKOUT_TOP_M`.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::map::{
    Appearance, Ground, Ladder, LaserLook, Light, MapBox, MapFile, Material, PlayerSpec, Shape,
    Sky, Space, Spawn, Structure, Sun, Warp, translation_frame, yaw_frame,
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
/// `BORDER_HEIGHT_M` is the height of the rock wall around the forest.
const BORDER_HEIGHT_M: f32 = 4.0;
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
const LEAF_SPRAY_COUNT: usize = 6;
const LEAF_SPRAY_DEPTH_M: f32 = 3.0;
const LEAF_HALF_LENGTH_M: f32 = 0.9;
const LEAF_HALF_WIDTH_M: f32 = 0.14;
const LEAF_HALF_THICKNESS_M: f32 = 0.03;

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
            let culm_count: usize = 3 + (radius_m * 6.0) as usize;
            for i in 0..culm_count {
                let angle_rad: f32 = (i as f32 + 0.6 * random_frac()) / culm_count as f32 * TAU;
                let spread_m: f32 = radius_m * CLUMP_SPREAD_FRAC * (0.35 + 0.65 * random_frac());
                let culm_radius_m: f32 = THIN_CULM_RADIUS_M * (0.75 + 0.5 * random_frac());
                let culm_height_m: f32 = height_m * (0.7 + 0.3 * random_frac());
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
            for spray in 0..LEAF_SPRAY_COUNT {
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
                frame: frame.to_cols_array(),
                shapes,
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

    /// `add_deck` adds a square deck whose top is at `top_m`.
    fn add_deck(&mut self, group: &str, centre: Vec2, half: Vec2, top_m: f32) {
        self.add_box(
            group,
            Vec3::new(centre.x, top_m - DECK_THICKNESS_M * 0.5, centre.y),
            Vec3::new(half.x, DECK_THICKNESS_M * 0.5, half.y),
            PLANKS,
        );
        self.keep_clear(centre, half);
        self.map.lights.push(Light {
            position: [centre.x, top_m + 2.4, centre.y, 1.0],
            color: [1.0, 0.62, 0.3],
            radius_m: 5.0,
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

    /// `add_pylon` adds a team's pylon: a painted plinth, its glowing console and a light.
    fn add_pylon(&mut self, side: Side, index: usize, foot_m: Vec3) {
        let group: String = format!("{} pylon {index}", side.team);
        self.add_box(
            &group,
            foot_m + Vec3::Y * 0.5,
            Vec3::new(0.4, 0.5, 0.4),
            side.paint,
        );
        self.add_box(
            &group,
            foot_m + Vec3::Y * 1.15,
            Vec3::new(0.3, 0.15, 0.3),
            side.console,
        );
        self.add_box(
            &group,
            foot_m + Vec3::Y * 1.8,
            Vec3::new(0.06, 0.5, 0.06),
            side.paint,
        );
        let color: [f32; 3] = if side.sign > 0.0 {
            [1.0, 0.2, 0.12]
        } else {
            [0.2, 0.4, 1.0]
        };
        self.map.lights.push(Light {
            position: [foot_m.x, foot_m.y + 1.9, foot_m.z, 1.0],
            color,
            radius_m: 3.5,
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

/// `BLACK_HOLE_CENTRE_M` is red's black hole, high over the open ground west of its ground pylon;
/// blue's is turned by 180°.
const BLACK_HOLE_CENTRE_M: Vec3 = Vec3::new(-14.0, 10.0, 36.0);
const BLACK_HOLE_SCHWARZSCHILD_RADIUS_M: f32 = 2.0;
const BLACK_HOLE_REACH_M: f32 = 8.0;
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

/// `add_warps` adds the two black holes and the three wormholes, and keeps trees out of their
/// reach.
fn add_warps(layout: &mut Layout) {
    for (name, centre_m) in [
        ("red's black hole", BLACK_HOLE_CENTRE_M),
        (
            "blue's black hole",
            BLUE.at(
                BLACK_HOLE_CENTRE_M.x,
                BLACK_HOLE_CENTRE_M.y,
                BLACK_HOLE_CENTRE_M.z,
            ),
        ),
    ] {
        layout.keep_clear(
            Vec2::new(centre_m.x, centre_m.z),
            Vec2::splat(BLACK_HOLE_REACH_M),
        );
        layout.map.warps.push(Warp {
            kind: "black_hole".to_string(),
            name: name.to_string(),
            frame: Mat4::from_translation(centre_m).to_cols_array(),
            law: "isotropic_schwarzschild".to_string(),
            schwarzschild_radius_m: BLACK_HOLE_SCHWARZSCHILD_RADIUS_M,
            lambda_squared: None,
            radial_coordinate: "distance".to_string(),
            reach_m: BLACK_HOLE_REACH_M,
            falloff: "taper".to_string(),
            partner: None,
        });
    }
    for (pair_index, pair) in MOUTH_PAIRS_XZ_M.iter().enumerate() {
        let first_index: usize = layout.map.warps.len();
        for (end_index, xz_m) in pair.iter().enumerate() {
            layout.keep_clear(*xz_m, Vec2::splat(MOUTH_REACH_M));
            layout.map.warps.push(Warp {
                kind: "wormhole_mouth".to_string(),
                name: format!("gate {pair_index} mouth {end_index}"),
                frame: Mat4::from_translation(Vec3::new(xz_m.x, MOUTH_HEIGHT_M, xz_m.y))
                    .to_cols_array(),
                law: "foil".to_string(),
                schwarzschild_radius_m: MOUTH_SCHWARZSCHILD_RADIUS_M,
                lambda_squared: Some(MOUTH_LAMBDA_SQUARED),
                radial_coordinate: "distance".to_string(),
                reach_m: MOUTH_REACH_M,
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
        let centre: Vec2 = flat(side.at(x, 0.0, z));
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
        let centre: Vec3 = side.at(x, half * 0.6, z);
        layout.add_box(
            &format!("{team} rock {index}"),
            centre,
            Vec3::new(half, half * 0.6, half * 0.8),
            ROCK,
        );
        layout.keep_clear(Vec2::new(centre.x, centre.z), Vec2::splat(half));
    }
}

/// `add_decorative_trees` scatters trees on red's half where nothing else stands and mirrors
/// them onto blue's half, so both teams see the same forest.
fn add_decorative_trees(layout: &mut Layout) {
    let mut count: usize = 0;
    let step_m: f32 = 6.5;
    let mut z: f32 = 3.0;
    while z < HALF_LENGTH_M - 2.0 {
        let mut x: f32 = -HALF_WIDTH_M + 3.0;
        while x < HALF_WIDTH_M - 2.0 {
            let jitter: Vec2 =
                Vec2::new(layout.random_frac() - 0.5, layout.random_frac() - 0.5) * 3.5;
            let point: Vec2 = Vec2::new(x, z) + jitter;
            let radius_m: f32 = 0.35 + layout.random_frac() * 0.45;
            let should_grow: bool = layout.random_frac() < 0.6;
            let inside: bool = point.x.abs() < HALF_WIDTH_M - 1.5
                && point.y < HALF_LENGTH_M - 1.5
                && point.y > 1.5;
            if should_grow && inside && layout.is_clear(point, radius_m * CLUMP_SPREAD_FRAC + 0.4) {
                let height_m: f32 = 18.0 + layout.random_frac() * 8.0;
                for (side, sign) in [("red", 1.0_f32), ("blue", -1.0)] {
                    let foot_m: Vec3 = Vec3::new(point.x * sign, 0.0, point.y * sign);
                    layout.add_tree(&format!("{side} tree {count}"), foot_m, radius_m, height_m);
                }
                layout.keep_clear(point, Vec2::splat(radius_m * CLUMP_SPREAD_FRAC));
                count += 1;
            }
            x += step_m;
        }
        z += step_m;
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
    ]
}

/// `get_clump_seed` gives a bamboo clump's random seed from where it stands, the same for red's
/// clump and blue's clump turned by 180°.
fn get_clump_seed(foot_m: Vec3) -> u32 {
    let x: u32 = (foot_m.x.abs() * 64.0) as u32;
    let z: u32 = (foot_m.z.abs() * 64.0) as u32;
    (x.wrapping_mul(73_856_093) ^ z.wrapping_mul(19_349_663)) | 1
}

/// `unit_direction` scales a direction to length 1, in f64 so the map's validator agrees.
fn unit_direction(direction: [f64; 3]) -> [f64; 3] {
    let length: f64 = direction.iter().map(|x| x * x).sum::<f64>().sqrt();
    direction.map(|x| x / length)
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
            description: "Dark Forest: a bounded Euclidean forest with treehouses, rope ladders and bridges, two pylons per team"
                .to_string(),
            source: "demos/dark_forest_rust/src/forest_layout.rs".to_string(),
            space: Space {
                kind: "euclidean".to_string(),
            },
            gravity_m_per_s2: 16.0,
            player: PlayerSpec {
                eye_height_m: 1.68,
                walk_m_per_s: 4.4,
                sprint_m_per_s: 7.0,
                jump_m_per_s: 5.4,
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
            laser: Some(LaserLook {
                rgb: GOLDEN_WHITE_LASER_RGB,
            }),
            lights: Vec::new(),
            sky: Sky {
                transport: "parallel".to_string(),
                sun: Sun {
                    direction: unit_direction([0.45, 0.35, -0.82]),
                    color: [1.0, 0.72, 0.45],
                },
                stars: false,
                clouds: true,
                zenith_color: [0.28, 0.36, 0.52],
                horizon_color: [0.62, 0.52, 0.42],
                fog_m: Some(70.0),
            },
            spawns: Vec::new(),
            appearance: Appearance {
                style: "forest".to_string(),
            lighting: serde_json::json!({"model": "ambient_sun", "ambient": 0.6, "sun": 3.7}),
        },
        },
        footprints: Vec::new(),
        random_state: 0x5eed_f0e5,
    };
    let margin_m: f32 = 12.0;
    layout.add_box_between(
        "the forest floor",
        Vec3::new(-HALF_WIDTH_M - margin_m, -0.5, -HALF_LENGTH_M - margin_m),
        Vec3::new(HALF_WIDTH_M + margin_m, 0.0, HALF_LENGTH_M + margin_m),
        FOREST_FLOOR,
    );
    let (w, l, t) = (HALF_WIDTH_M, HALF_LENGTH_M, 1.0);
    for (a, b) in [
        (
            Vec3::new(-w - t, 0.0, -l - t),
            Vec3::new(w + t, BORDER_HEIGHT_M, -l),
        ),
        (
            Vec3::new(-w - t, 0.0, l),
            Vec3::new(w + t, BORDER_HEIGHT_M, l + t),
        ),
        (
            Vec3::new(-w - t, 0.0, -l),
            Vec3::new(-w, BORDER_HEIGHT_M, l),
        ),
        (Vec3::new(w, 0.0, -l), Vec3::new(w + t, BORDER_HEIGHT_M, l)),
    ] {
        layout.add_box_between("the border rocks", a, b, ROCK);
    }
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
    add_decorative_trees(&mut layout);
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
    layout.map
}

/// `tests` checks the layout's invariants.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | teams | red, blue | `test_each_team_has_two_consoles_and_spawns` |
/// | ladders | to a deck, to a lookout | `test_ladders_reach_decks` |
/// | symmetry | red's half against blue's | `test_halves_mirror` |
/// | warps | 2 black holes and 3 paired wormholes, each reach clear of boxes, shapes, ladders and spawns | `test_warps_are_paired_and_clear` |
#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::frame_translation_m;

    /// `test_each_team_has_two_consoles_and_spawns` checks the objective's pieces exist.
    #[test]
    fn test_each_team_has_two_consoles_and_spawns() {
        let map: MapFile = build();
        for (console, team) in [(RED_CONSOLE, "red"), (BLUE_CONSOLE, "blue")] {
            assert_eq!(
                map.boxes.iter().filter(|b| b.material == console).count(),
                2
            );
            assert_eq!(map.spawns.iter().filter(|s| s.team == team).count(), 4);
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

    /// `test_warps_are_paired_and_clear` checks the map has 2 black holes and 3 wormholes whose
    /// mouths point at each other, and that nothing but the ground is within any warp's reach.
    #[test]
    fn test_warps_are_paired_and_clear() {
        let map: MapFile = build();
        let black_hole_count: usize = map.warps.iter().filter(|w| w.kind == "black_hole").count();
        assert_eq!((black_hole_count, map.warps.len()), (2, 8));
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
        for structure in &map.structures {
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
