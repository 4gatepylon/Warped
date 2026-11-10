//! `city_world` is City Sphere's world: the map read from `common/maps/city_sphere/`, the players'
//! movement inside the sphere (gravity outward, so up is toward the centre), and shots, which go
//! straight, stop at the street, buildings and the core, and reflect off the glass shell's inner
//! side.
//!
//! Interface: `CityWorld::Builtin`, `CityWorld::FromJson`, `TeamDome`, `CityWorld::get_team_domes`, `CityPlayer`,
//! `CityInput`, `Shot`, `Bounces`, `ShotHit`, `CityWorld::trace_shot`, `CityWorld::move_player`,
//! `Side`, `get_up`.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;

use glam::{DMat3, DMat4, DQuat, DVec3};
use serde::Deserialize;

use warped_shared::gravity::{FlatWarp, get_flat_pull_m_per_s};
use warped_shared::laser_drop::{DROP_PIECE_M, turn_flat};
use warped_shared::movement::{MoveInput, MoveRules, MoveState};

use crate::play_settings::HeadBobSettings;
use crate::settings::{GlassSettings, ServerSettings};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const MAP_JSON: &str = include_str!("../../../common/maps/city_sphere/map.json");
const TEAM_NAMES: [&str; 2] = ["blue", "red"];
pub const BODY_HEIGHT_M: f64 = 1.8;
pub const BODY_RADIUS_M: f64 = 0.4;
const HEAD_RADIUS_M: f64 = 0.25;
const TORSO_RADIUS_M: f64 = 0.42;
pub const TORSO_HEIGHT_M: f64 = 1.0;
pub const HEAD_HEIGHT_M: f64 = 1.55;
/// `WARP_PULL_HEIGHT_M` is where warps pull a body (W-144): its middle above the feet, where a
/// mouth's disc carries it through (`city::BODY_MIDDLE_M`).
const WARP_PULL_HEIGHT_M: f64 = 0.9;
const LIFT_M_PER_S: f64 = 6.0;
const LIFT_TOP_UNDER_GLASS_M: f64 = 1.0;
const AIR_CONTROL_FRAC: f64 = 0.35;
const SHOT_START_M: f64 = 1e-4;
const SURFACE_OFFSET_M: f64 = 1e-3;
const MAX_STEP_M: f64 = 0.2;
const MAX_STEP_UP_M: f64 = 0.2;
const MIN_PITCH_RAD: f64 = -1.5;
const MAX_PITCH_RAD: f64 = 1.5;
const GROUND_NORMAL_COS: f64 = 0.6;
const SPAWN_PITCH_RAD: f64 = 0.25;
const LIFT_GROUP: &str = "lifts";
const STAIRS_GROUP: &str = "stairs";
const PYLON_GROUP: &str = "pylons";
const PATH_CELL_RAD: f64 = 0.025;
const PATH_CLEARANCE_M: f64 = 0.35;
const PATH_HOLE_CLEARANCE_M: f64 = 1.5;
const PATH_COST_PER_RAD: f64 = 1e6;
/// `CONSOLE_REACH_SLACK_M` is how much further than `consoles.reach_m` the server lets a hold
/// count, so a window that turns a console off at the edge of its reach isn't refused.
pub const CONSOLE_REACH_SLACK_M: f64 = 0.6;

#[derive(Deserialize)]
struct JsonMaterial {
    id: u8,
    name: String,
    color: [f32; 3],
    kind: String,
}

#[derive(Deserialize)]
struct JsonSphere {
    radius_m: f64,
}

#[derive(Deserialize)]
struct JsonGround {
    material: u8,
    sphere: JsonSphere,
}

#[derive(Deserialize)]
struct JsonHole {
    direction: [f64; 3],
    angular_radius_rad: f64,
}

#[derive(Deserialize)]
struct JsonShell {
    radius_m: f64,
    thickness_m: f64,
    material: u8,
    walkable_side: String,
    laser: String,
    holes: Vec<JsonHole>,
}

#[derive(Deserialize)]
struct JsonBox {
    name: String,
    group: String,
    frame: [f64; 16],
    half_size_m: [f64; 3],
    material: u8,
}

#[derive(Deserialize)]
struct JsonSpawn {
    team: String,
    frame: [f64; 16],
}

#[derive(Deserialize)]
struct JsonPlayer {
    eye_height_m: f64,
    walk_m_per_s: f64,
    sprint_m_per_s: f64,
    jump_m_per_s: f64,
}

/// `DEFAULT_LASER_COLORS` are the shots' colours on a map without `lasers`: blue's, then red's.
pub const DEFAULT_LASER_COLORS: [[f32; 3]; 2] = [[0.3, 0.55, 1.0], [1.0, 0.3, 0.25]];

#[derive(Deserialize)]
struct JsonMap {
    name: String,
    space: JsonSpace,
    gravity_m_per_s2: f64,
    player: JsonPlayer,
    materials: Vec<JsonMaterial>,
    ground: JsonGround,
    shells: Vec<JsonShell>,
    boxes: Vec<JsonBox>,
    spawns: Vec<JsonSpawn>,
    #[serde(default)]
    lasers: Option<JsonLasers>,
    #[serde(default)]
    variation: Option<JsonVariation>,
    #[serde(default)]
    regions: Option<JsonRegions>,
}

/// `JsonRegions` is the map's team zones, bands of Fermi v (+x here), and how their walls look.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonRegions {
    zones: Vec<JsonZone>,
    zone_walls: Option<JsonZoneWalls>,
}

/// `JsonZone` is one team's band of v, `None` for no edge on that side. City Sphere reads only
/// the team: its team areas are domes over the pylons (`TeamDome`), not bands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonZone {
    team: String,
    #[serde(rename = "v_min_m")]
    _v_min_m: Option<f64>,
    #[serde(rename = "v_max_m")]
    _v_max_m: Option<f64>,
}

/// `JsonZoneWalls` is how the zones' walls look.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonZoneWalls {
    height_m: f64,
    thickness_m: f64,
    opacity_frac: f64,
}

/// `TeamDome` is one team's area in City Sphere and its boundary (W-148, team boundary planes in
/// every level; the user: "for city sphere in particular they are domes, NOT planes"): the ball
/// of `radius_m` around `centre`, the foot of the team `team`'s pylon on the glass level, where it
/// is above the glass. Its surface is drawn `opacity_frac` opaque at its foot; it stops neither
/// players nor lasers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamDome {
    pub centre: DVec3,
    pub radius_m: f64,
    pub team: u32,
    pub opacity_frac: f64,
}

/// `get_dome_opacity_frac` gives how opaque the map draws its team boundaries at their foot
/// (`regions.zone_walls.opacity_frac`), or `None` for a map without zone walls; it fails on a
/// zone of a team that isn't blue, red or middle, or zone walls that aren't positive.
///
/// Example: `let opacity_frac = get_dome_opacity_frac(map.regions.as_ref())?;`
fn get_dome_opacity_frac(regions: Option<&JsonRegions>) -> Result<Option<f64>> {
    let Some(regions) = regions else {
        return Ok(None);
    };
    for zone in &regions.zones {
        if zone.team != "middle" && !TEAM_NAMES.contains(&zone.team.as_str()) {
            return Err(format!("map: zone team {:?} isn't blue, red or middle", zone.team).into());
        }
    }
    let Some(walls) = &regions.zone_walls else {
        return Ok(None);
    };
    if walls.height_m <= 0.0
        || walls.thickness_m <= 0.0
        || !(walls.opacity_frac > 0.0 && walls.opacity_frac < 1.0)
    {
        return Err(
            "map: zone_walls needs a positive height and thickness and an opacity between 0 and 1"
                .into(),
        );
    }
    Ok(Some(walls.opacity_frac))
}

/// `JsonLasers` is the map's laser colour: one for every team, or one per team.
#[derive(Deserialize)]
struct JsonVariation {
    seed: u32,
}

#[derive(Deserialize)]
struct JsonLasers {
    color: Option<[f32; 3]>,
    team_colors: Option<[[f32; 3]; 2]>,
}

#[derive(Deserialize)]
struct JsonSpace {
    kind: String,
}

/// `Material` is a map material: its name, colour and whether it glows.
#[derive(Clone)]
pub struct Material {
    pub name: String,
    pub color: [f32; 3],
    pub is_light: bool,
}

/// `Hole` is a hole in a shell: the directions within its angular radius of `direction` (`cos` is
/// the radius's cosine) and, for a stairs' hole, only those over its stairs: within
/// `half_width_m` of the plane through the centre along the flight, measured at the shell's
/// radius (`side` is that plane's unit normal, the steps' right axis). A round hole's
/// `half_width_m` is infinite.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hole {
    pub direction: DVec3,
    pub cos: f64,
    pub side: DVec3,
    pub half_width_m: f64,
}

/// `Shell` is a sphere about the centre: the glass (walkable on its inner side, reflecting shots
/// from that side) or the core (a solid ball), with its holes.
pub struct Shell {
    pub radius_m: f64,
    pub thickness_m: f64,
    pub material: u8,
    pub is_walkable_inside: bool,
    pub is_reflective_inside: bool,
    pub holes: Vec<Hole>,
}

impl Shell {
    /// `is_in_hole` says whether the direction of `point` from the centre is in one of the holes.
    ///
    /// Example: `let is_open = shell.is_in_hole(feet);`
    pub fn is_in_hole(&self, point: DVec3) -> bool {
        self.is_in_widened_hole(point, 0.0)
    }

    /// `is_in_widened_hole` is `is_in_hole` with every stairs' hole `side_margin_m` wider on each
    /// side (a round hole is unchanged).
    ///
    /// Example: `let can_climb = shell.is_in_widened_hole(feet, BODY_RADIUS_M);`
    pub fn is_in_widened_hole(&self, point: DVec3, side_margin_m: f64) -> bool {
        let d: DVec3 = point.normalize_or_zero();
        self.holes.iter().any(|hole| {
            d.dot(hole.direction) >= hole.cos
                && d.dot(hole.side).abs() * self.radius_m <= hole.half_width_m + side_margin_m
        })
    }

    /// `is_ball` says whether the shell is solid to its centre (the core).
    pub fn is_ball(&self) -> bool {
        self.thickness_m >= self.radius_m
    }
}

/// `CityBox` is an oriented box: its centre, its axes (columns: right, up, back) and half sizes.
pub struct CityBox {
    pub name: String,
    pub group: String,
    pub center: DVec3,
    pub axes: DMat3,
    pub half_m: DVec3,
    pub material: u8,
}

impl CityBox {
    /// `to_local` gives `point` in the box's axes, from its centre.
    pub fn to_local(&self, point: DVec3) -> DVec3 {
        self.axes.transpose() * (point - self.center)
    }

    /// `find_hit_m` gives the distance along the unit `direction` from `origin` to where the ray
    /// enters the box (after `SHOT_START_M`), and the outward normal there.
    ///
    /// Example: `let hit = city_box.find_hit_m(eye, forward);`
    pub fn find_hit_m(&self, origin: DVec3, direction: DVec3) -> Option<(f64, DVec3)> {
        let o: DVec3 = self.to_local(origin);
        let d: DVec3 = self.axes.transpose() * direction;
        let (mut t_min, mut t_max, mut axis, mut sign) = (f64::NEG_INFINITY, f64::INFINITY, 0, 1.0);
        for k in 0..3 {
            if d[k].abs() < 1e-12 {
                if o[k].abs() > self.half_m[k] {
                    return None;
                }
                continue;
            }
            let t0: f64 = (-self.half_m[k] - o[k]) / d[k];
            let t1: f64 = (self.half_m[k] - o[k]) / d[k];
            let (near, far) = if t0 < t1 { (t0, t1) } else { (t1, t0) };
            if near > t_min {
                (t_min, axis, sign) = (near, k, -d[k].signum());
            }
            t_max = t_max.min(far);
        }
        if t_min > t_max || t_min < SHOT_START_M {
            return None;
        }
        let mut normal: DVec3 = DVec3::ZERO;
        normal[axis] = sign;
        Some((t_min, self.axes * normal))
    }
}

/// `Console` is a pylon's console: its team, its index in the server's console list (the order
/// of the `<team> console <n>` materials) and its box.
pub struct Console {
    pub team: u32,
    pub index: u8,
    pub box_index: usize,
}

/// `CityWorld` is the whole level.
pub struct CityWorld {
    pub street_radius_m: f64,
    pub street_material: u8,
    /// `map_gravity_m_per_s2` is the map's gravity, before `server_settings.yaml`'s `movement`
    /// speeds up the jumps (`get_motion`).
    pub map_gravity_m_per_s2: f64,
    pub eye_height_m: f64,
    /// `map_walk_m_per_s` is the map's walking speed, which `server_settings.yaml`'s
    /// `movement.walk_speed_per_map_speed` multiplies, as on Lunar Surface.
    pub map_walk_m_per_s: f64,
    /// `map_sprint_m_per_s` is the map's sprinting speed, which `movement.sprint_speed_per_map_speed`
    /// multiplies.
    pub map_sprint_m_per_s: f64,
    /// `map_jump_m_per_s` is the map's jump speed, which with `map_gravity_m_per_s2` gives the
    /// map's jump time.
    pub map_jump_m_per_s: f64,
    pub materials: Vec<Material>,
    pub shells: Vec<Shell>,
    pub boxes: Vec<CityBox>,
    pub consoles: Vec<Console>,
    pub spawns: Vec<(u32, DMat4)>,
    /// `lift_holes` are the glass's holes over a lift pad (a box in the group `lifts`); the
    /// stairs' holes have none.
    pub lift_holes: Vec<Hole>,
    pub laser_colors: [[f32; 3]; 2],
    /// `variation_seed` seeds the map's small look variation (panel tint), 0 without one.
    pub variation_seed: u32,
    /// `mouths` are the linked thrown mouths in the game now, which pull walkers along the ground
    /// (W-144, `move_player`); the window and the bots set them each frame, as Lunar Surface's
    /// `VoxelWorld::set_thrown_wormholes` adds them to its warps.
    pub mouths: Vec<FlatWarp>,
    /// `dome_opacity_frac` is how opaque the team domes are at their foot (W-148), from the map's
    /// zone walls; `None`: the map has none, so no domes are drawn.
    pub dome_opacity_frac: Option<f64>,
}

/// `Bounces` is how a shot meets the glass's inner side (`CityWorld::trace_shot`): at most
/// `max_count` reflections, each keeping `reflectivity_frac` of the range left.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Bounces {
    pub max_count: u32,
    pub reflectivity_frac: f64,
}

impl Bounces {
    /// `NONE` stops a shot at the glass's first meeting.
    pub const NONE: Bounces = Bounces {
        max_count: 0,
        reflectivity_frac: 1.0,
    };

    /// `Perfect` reflects at most `max_count` times off a perfect mirror.
    ///
    /// Example: `let bounces = Bounces::Perfect(2);`
    #[allow(non_snake_case)]
    pub fn Perfect(max_count: u32) -> Bounces {
        Bounces {
            max_count,
            reflectivity_frac: 1.0,
        }
    }

    /// `FromGlass` reflects as the server settings' `glass` section says.
    ///
    /// Example: `let bounces = Bounces::FromGlass(&settings.glass);`
    #[allow(non_snake_case)]
    pub fn FromGlass(glass: &GlassSettings) -> Bounces {
        Bounces {
            max_count: glass.max_bounce_count,
            reflectivity_frac: glass.reflectivity_frac,
        }
    }
}

/// `Side` is which level a point is on: the street, under the glass, or the glass level, on the
/// glass's inner side (toward the centre).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Street,
    Glass,
}

/// `ShotHit` is what a shot ended on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ShotHit {
    Player { id: u32, is_head: bool },
    Surface { material: u8 },
    Range,
}

/// `Shot` is a shot's path, its corners from the muzzle to its end, with how many times it
/// reflected and what it ended on.
#[derive(Clone, Debug)]
pub struct Shot {
    pub points: Vec<DVec3>,
    pub bounce_count: u32,
    pub hit: ShotHit,
}

/// `Target` is a player a shot can hit: id, feet, up, and height as a share of standing (less
/// while crouched, W-137 (crouched robots sink): the torso and head spheres come down with it).
#[derive(Clone, Copy)]
pub struct Target {
    pub id: u32,
    pub feet: DVec3,
    pub up: DVec3,
    pub height_frac: f64,
}

/// `get_up` is the up direction at `point`: toward the centre.
///
/// Example: `let up = get_up(feet);`
pub fn get_up(point: DVec3) -> DVec3 {
    let up: DVec3 = -point.normalize_or_zero();
    if up == DVec3::ZERO { DVec3::Y } else { up }
}

/// `find_sphere_hits_m` gives the distances along the unit `direction` from `origin` where the
/// ray meets the sphere of `radius_m` about the centre, nearest first.
fn find_sphere_hits_m(
    origin: DVec3,
    direction: DVec3,
    center: DVec3,
    radius_m: f64,
) -> Option<(f64, f64)> {
    let o: DVec3 = origin - center;
    let b: f64 = o.dot(direction);
    let c: f64 = o.length_squared() - radius_m * radius_m;
    let discriminant: f64 = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    let root: f64 = discriminant.sqrt();
    Some((-b - root, -b + root))
}

/// `get_material_kind` maps a material kind to (is it a light).
fn parse_frame(values: &[f64; 16]) -> DMat4 {
    DMat4::from_cols_array(values)
}

/// `fit_hole_to_stairs` reads the map's round hole `json` and, when a step of stairs (a box in
/// the group `stairs`) stands in it, narrows it to that flight's width, so the glass beside the
/// stairs stays whole (W-132 (City Sphere stairs hole)); a hole without a step stays round.
///
/// Example: `let hole = fit_hole_to_stairs(&map.shells[0].holes[6], &boxes);`
fn fit_hole_to_stairs(json: &JsonHole, boxes: &[CityBox]) -> Hole {
    let direction: DVec3 = DVec3::from(json.direction).normalize();
    let cos: f64 = json.angular_radius_rad.cos();
    let step: Option<&CityBox> = boxes
        .iter()
        .find(|b| b.group == STAIRS_GROUP && b.center.normalize().dot(direction) >= cos);
    Hole {
        direction,
        cos,
        side: step.map_or(DVec3::ZERO, |b| b.axes.x_axis),
        half_width_m: step.map_or(f64::INFINITY, |b| b.half_m.x),
    }
}

impl CityWorld {
    /// `Builtin` reads the shipped map, `common/maps/city_sphere/map.json`.
    ///
    /// Example: `let world = CityWorld::Builtin()?;`
    #[allow(non_snake_case)]
    pub fn Builtin() -> Result<CityWorld> {
        CityWorld::FromJson(MAP_JSON)
    }

    /// `FromJson` reads a City Sphere manifest: Euclidean, with a sphere ground and shells.
    ///
    /// Example: `let world = CityWorld::FromJson(&std::fs::read_to_string(path)?)?;`
    #[allow(non_snake_case)]
    pub fn FromJson(text: &str) -> Result<CityWorld> {
        let map: JsonMap = serde_json::from_str(text)?;
        if map.space.kind != "euclidean" {
            return Err(format!("city map {}: the space must be euclidean", map.name).into());
        }
        let mut materials: Vec<Material> = vec![
            Material {
                name: String::new(),
                color: [0.0; 3],
                is_light: false
            };
            256
        ];
        for material in &map.materials {
            materials[material.id as usize] = Material {
                name: material.name.clone(),
                color: material.color,
                is_light: material.kind == "light",
            };
        }
        let console_materials: Vec<(u8, u32)> = map
            .materials
            .iter()
            .filter_map(|m| {
                TEAM_NAMES
                    .iter()
                    .position(|team| m.name.starts_with(&format!("{team} console ")))
                    .map(|team| (m.id, team as u32))
            })
            .collect();
        let boxes: Vec<CityBox> = map
            .boxes
            .iter()
            .map(|b| {
                let frame: DMat4 = parse_frame(&b.frame);
                CityBox {
                    name: b.name.clone(),
                    group: b.group.clone(),
                    center: frame.w_axis.truncate(),
                    axes: DMat3::from_mat4(frame),
                    half_m: DVec3::from(b.half_size_m),
                    material: b.material,
                }
            })
            .collect();
        let mut consoles: Vec<Console> = Vec::new();
        for (index, &(material, team)) in console_materials.iter().enumerate() {
            let box_index: usize = boxes
                .iter()
                .position(|b| b.material == material)
                .ok_or(format!("city map: no box is console material {material}"))?;
            consoles.push(Console {
                team,
                index: index as u8,
                box_index,
            });
        }
        let shells: Vec<Shell> = map
            .shells
            .iter()
            .map(|s| Shell {
                radius_m: s.radius_m,
                thickness_m: s.thickness_m,
                material: s.material,
                is_walkable_inside: s.walkable_side == "inside",
                is_reflective_inside: s.laser == "reflect_inside",
                holes: s
                    .holes
                    .iter()
                    .map(|h| fit_hole_to_stairs(h, &boxes))
                    .collect(),
            })
            .collect();
        let lift_holes: Vec<Hole> = shells
            .iter()
            .filter(|s| s.is_walkable_inside)
            .flat_map(|s| s.holes.iter().copied())
            .filter(|hole| {
                boxes.iter().any(|b| {
                    b.group == LIFT_GROUP && b.center.normalize().dot(hole.direction) >= hole.cos
                })
            })
            .collect();
        let spawns: Vec<(u32, DMat4)> = map
            .spawns
            .iter()
            .map(|s| {
                (
                    TEAM_NAMES.iter().position(|t| *t == s.team).unwrap_or(0) as u32,
                    parse_frame(&s.frame),
                )
            })
            .collect();
        Ok(CityWorld {
            street_radius_m: map.ground.sphere.radius_m,
            street_material: map.ground.material,
            map_gravity_m_per_s2: map.gravity_m_per_s2,
            eye_height_m: map.player.eye_height_m,
            map_walk_m_per_s: map.player.walk_m_per_s,
            map_sprint_m_per_s: map.player.sprint_m_per_s,
            map_jump_m_per_s: map.player.jump_m_per_s,
            materials,
            shells,
            boxes,
            consoles,
            spawns,
            lift_holes,
            variation_seed: map.variation.as_ref().map_or(0, |v| v.seed),
            mouths: Vec::new(),
            dome_opacity_frac: get_dome_opacity_frac(map.regions.as_ref())?,
            laser_colors: match &map.lasers {
                Some(JsonLasers {
                    team_colors: Some(colors),
                    ..
                }) => *colors,
                Some(JsonLasers {
                    color: Some(color), ..
                }) => [*color; 2],
                Some(_) => return Err("map: lasers needs color or team_colors".into()),
                None => DEFAULT_LASER_COLORS,
            },
        })
    }

    /// `get_glass` gives the walkable glass shell.
    pub fn get_glass(&self) -> Option<&Shell> {
        self.shells.iter().find(|s| s.is_walkable_inside)
    }

    /// `get_team_domes` gives each team's dome (W-148): around the foot of each of the team's
    /// pylons that stand on the glass level, with the radius that makes the dome span
    /// `circumference_frac` of the way round the glass level (0.2: 72 degrees, 21.4 m across on
    /// a 17 m glass level). It gives none on a map without zone walls or without a glass level.
    ///
    /// Example: `let domes: Vec<TeamDome> = world.get_team_domes(settings.team_domes.circumference_frac);`
    pub fn get_team_domes(&self, circumference_frac: f64) -> Vec<TeamDome> {
        let (Some(opacity_frac), Some(glass)) = (self.dome_opacity_frac, self.get_glass()) else {
            return Vec::new();
        };
        let glass_m: f64 = glass.radius_m;
        let radius_m: f64 = 2.0 * glass_m * (std::f64::consts::PI * circumference_frac / 2.0).sin();
        let mut domes: Vec<TeamDome> = Vec::new();
        for (team, name) in TEAM_NAMES.iter().enumerate() {
            let on_glass = self.boxes.iter().filter(|b| {
                b.group == PYLON_GROUP && b.name.starts_with(name) && b.center.length() < glass_m
            });
            domes.extend(on_glass.map(|pylon| TeamDome {
                centre: pylon.center.normalize() * glass_m,
                radius_m,
                team: team as u32,
                opacity_frac,
            }));
        }
        domes
    }

    /// `get_side` gives which level `feet` are on: the glass level if they are inside the glass.
    ///
    /// Example: `let side = world.get_side(player.feet);`
    pub fn get_side(&self, feet: DVec3) -> Side {
        match self.get_glass() {
            Some(glass) if feet.length() < glass.radius_m + SURFACE_OFFSET_M * 10.0 => Side::Glass,
            _ => Side::Street,
        }
    }

    /// `get_console_center` gives a console's centre.
    pub fn get_console_center(&self, console: &Console) -> DVec3 {
        self.boxes[console.box_index].center
    }

    /// `get_stairs_ends` gives where `team`'s stairs start on the street (the middle of the bottom
    /// step's back edge) and end on the glass level (the middle of the top step's far edge), as
    /// unit directions from the centre, or `None` on a map without them.
    ///
    /// Example: `let (bottom, top) = world.get_stairs_ends(0).expect("stairs");`
    pub fn get_stairs_ends(&self, team: u32) -> Option<(DVec3, DVec3)> {
        let steps: Vec<&CityBox> = self
            .boxes
            .iter()
            .filter(|b| b.group == STAIRS_GROUP && b.name.starts_with(TEAM_NAMES[team as usize]))
            .collect();
        let (first, last) = (steps.first()?, steps.last()?);
        let bottom: DVec3 = first.center + first.axes.z_axis * first.half_m.z;
        let top: DVec3 = last.center - last.axes.z_axis * last.half_m.z;
        Some((bottom.normalize(), top.normalize()))
    }

    /// `is_console_aimed` says whether a player whose eye frame is `eye` aims at console `index`
    /// within `reach_m`, as on Lunar Surface (`find_aimed_material`): the first box the ray along
    /// their crosshair meets is the console, or the pylon around it, at most `reach_m` away. An
    /// index that isn't a console (from the network) isn't aimed at.
    ///
    /// Example: `if world.is_console_aimed(player.get_eye_frame(), 1, 3.0) { ... }`
    pub fn is_console_aimed(&self, eye: DMat4, index: u8, reach_m: f64) -> bool {
        let Some(console) = self.consoles.iter().find(|c| c.index == index) else {
            return false;
        };
        let console_center: DVec3 = self.boxes[console.box_index].center;
        let (origin, direction) = (eye.w_axis.truncate(), -eye.z_axis.truncate().normalize());
        let nearest: Option<(f64, usize)> = self
            .boxes
            .iter()
            .enumerate()
            .filter_map(|(k, b)| b.find_hit_m(origin, direction).map(|(t, _)| (t, k)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        nearest.is_some_and(|(t, k)| {
            let hit: &CityBox = &self.boxes[k];
            let is_pylon_around: bool = hit.group == PYLON_GROUP
                && hit.to_local(console_center).abs().cmple(hit.half_m).all();
            t <= reach_m && (k == console.box_index || is_pylon_around)
        })
    }

    /// `into_console_check` turns the world into the game server's console check: a hold counts
    /// while the player aims at the console within `reach_m` plus `CONSOLE_REACH_SLACK_M`
    /// (`is_console_aimed`).
    ///
    /// Example: `let check = CityWorld::Builtin()?.into_console_check(3.0);`
    pub fn into_console_check(self, reach_m: f64) -> Box<dyn Fn(DMat4, u8) -> bool + Send> {
        Box::new(move |eye, index| {
            self.is_console_aimed(eye, index, reach_m + CONSOLE_REACH_SLACK_M)
        })
    }

    /// `find_glass_path` plans a walk on the glass level from the unit direction `from` toward
    /// the unit direction `goal`, avoiding the glass's holes and whatever stands on the glass level
    /// (covers, pylons) by a body radius and `PATH_CLEARANCE_M` (`PATH_HOLE_CLEARANCE_M` from a
    /// hole, since a walker that overshoots a turn there falls through): an A* search on a
    /// longitude-latitude grid of `PATH_CELL_RAD` cells, from the free cell nearest `from` (the
    /// first waypoint) to the free cell nearest `goal`, then pulled taut into the fewest straight
    /// legs. It gives the legs' ends as unit directions, or `None` when no walk exists.
    ///
    /// Example: `let waypoints = world.find_glass_path(top, console_direction).expect("a path");`
    pub fn find_glass_path(&self, from: DVec3, goal: DVec3) -> Option<Vec<DVec3>> {
        let glass: &Shell = self.get_glass()?;
        let level_m: f64 = glass.radius_m - glass.thickness_m / 2.0;
        let lat_count: usize = (std::f64::consts::PI / PATH_CELL_RAD).round() as usize;
        let lon_count: usize = (std::f64::consts::TAU / PATH_CELL_RAD).round() as usize;
        let to_direction = |cell: usize| -> DVec3 {
            let (i, j) = (cell / lon_count, cell % lon_count);
            let lat: f64 = (i as f64 + 0.5) / lat_count as f64 * std::f64::consts::PI
                - std::f64::consts::FRAC_PI_2;
            let lon: f64 = j as f64 / lon_count as f64 * std::f64::consts::TAU;
            DVec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin())
        };
        let clearance_m: f64 = BODY_RADIUS_M + PATH_CLEARANCE_M;
        let standing: Vec<&CityBox> = self
            .boxes
            .iter()
            .filter(|b| {
                let (near_m, far_m) = (
                    b.center.length() - b.half_m.length(),
                    b.center.length() + b.half_m.length(),
                );
                near_m < level_m && far_m > level_m - BODY_HEIGHT_M
            })
            .filter(|b| b.group != STAIRS_GROUP)
            .collect();
        let is_free = |d: DVec3| -> bool {
            let is_near_hole: bool = glass.holes.iter().any(|hole| {
                d.dot(hole.direction).clamp(-1.0, 1.0).acos()
                    < hole.cos.acos() + (BODY_RADIUS_M + PATH_HOLE_CLEARANCE_M) / level_m
            });
            !is_near_hole
                && standing.iter().all(|b| {
                    [
                        BODY_RADIUS_M,
                        BODY_HEIGHT_M / 2.0,
                        BODY_HEIGHT_M - BODY_RADIUS_M,
                    ]
                    .iter()
                    .all(|&height_m| {
                        let local: DVec3 = b.to_local(d * (level_m - height_m));
                        (local - local.clamp(-b.half_m, b.half_m)).length() >= clearance_m
                    })
                })
        };
        let cell_count: usize = lat_count * lon_count;
        let free: Vec<bool> = (0..cell_count)
            .map(|cell| is_free(to_direction(cell)))
            .collect();
        let find_nearest_free = |d: DVec3| -> Option<usize> {
            (0..cell_count)
                .filter(|&cell| free[cell])
                .min_by(|&a, &b| d.dot(to_direction(b)).total_cmp(&d.dot(to_direction(a))))
        };
        let start: usize = find_nearest_free(from)?;
        let end: usize = find_nearest_free(goal)?;
        let angle =
            |a: usize, b: usize| to_direction(a).dot(to_direction(b)).clamp(-1.0, 1.0).acos();
        let mut cost: Vec<f64> = vec![f64::INFINITY; cell_count];
        let mut came_from: Vec<usize> = vec![usize::MAX; cell_count];
        let mut open: BinaryHeap<Reverse<(u64, usize)>> = BinaryHeap::new();
        cost[start] = 0.0;
        open.push(Reverse((
            (angle(start, end) * PATH_COST_PER_RAD) as u64,
            start,
        )));
        while let Some(Reverse((_, cell))) = open.pop() {
            if cell == end {
                break;
            }
            let (i, j) = (cell / lon_count, cell % lon_count);
            for (di, dj) in [
                (-1, -1),
                (-1, 0),
                (-1, 1),
                (0, -1),
                (0, 1),
                (1, -1),
                (1, 0),
                (1, 1),
            ] {
                let ni: i64 = i as i64 + di;
                if ni < 0 || ni >= lat_count as i64 {
                    continue;
                }
                let nj: usize = (j as i64 + dj).rem_euclid(lon_count as i64) as usize;
                let next: usize = ni as usize * lon_count + nj;
                let next_cost: f64 = cost[cell] + angle(cell, next);
                if free[next] && next_cost < cost[next] {
                    cost[next] = next_cost;
                    came_from[next] = cell;
                    let priority: f64 = (next_cost + angle(next, end)) * PATH_COST_PER_RAD;
                    open.push(Reverse((priority as u64, next)));
                }
            }
        }
        if !cost[end].is_finite() {
            return None;
        }
        let mut cells: Vec<usize> = vec![end];
        while *cells.last().expect("the path has a cell") != start {
            cells.push(came_from[*cells.last().expect("the path has a cell")]);
        }
        cells.reverse();
        let is_leg_free = |a: DVec3, b: DVec3| -> bool {
            let sample_count: usize =
                (a.dot(b).clamp(-1.0, 1.0).acos() / (PATH_CELL_RAD / 2.0)).ceil() as usize;
            (1..sample_count)
                .all(|k| is_free(a.lerp(b, k as f64 / sample_count as f64).normalize()))
        };
        let mut waypoints: Vec<DVec3> = vec![to_direction(start)];
        let mut at: usize = 0;
        while at + 1 < cells.len() {
            let reach: usize = (at + 1..cells.len())
                .rev()
                .find(|&k| is_leg_free(to_direction(cells[at]), to_direction(cells[k])))
                .unwrap_or(at + 1);
            waypoints.push(to_direction(cells[reach]));
            at = reach;
        }
        Some(waypoints)
    }

    /// `is_in_lift` says whether `feet` are over a lift pad, under its hole in the glass (not in
    /// the stairs' holes, where holding jump does nothing).
    ///
    /// Example: `let can_ride = world.is_in_lift(feet);`
    pub fn is_in_lift(&self, feet: DVec3) -> bool {
        let d: DVec3 = feet.normalize_or_zero();
        self.lift_holes
            .iter()
            .any(|hole| d.dot(hole.direction) >= hole.cos)
    }

    /// `trace_shot` follows a shot from `origin` along `direction` for up to `range_m`: it stops
    /// at the street, boxes, the core, a target (not `shooter_id`) or the glass from outside, and
    /// reflects off the glass's inner side about its normal at most `bounces.max_count` times (the
    /// next meeting with the glass stops it), each reflection keeping `bounces.reflectivity_frac`
    /// of the range left.
    ///
    /// Example: `let shot = world.trace_shot(eye, forward, 80.0, Bounces::Perfect(2), &targets, Some(my_id));`
    pub fn trace_shot(
        &self,
        origin: DVec3,
        direction: DVec3,
        range_m: f64,
        bounces: Bounces,
        targets: &[Target],
        shooter_id: Option<u32>,
    ) -> Shot {
        self.trace_falling_shot(
            origin, direction, range_m, bounces, targets, shooter_id, 0.0,
        )
    }

    /// `trace_falling_shot` is `trace_shot` for a shot that falls toward the street (outward)
    /// by `drop_per_m` per meter (`laser_drop::get_drop_curvature_per_m`, W-157 (bullet drop, by
    /// game setting)): it goes in straight pieces of at most `DROP_PIECE_M`, turning down between
    /// them (`turn_flat`), each piece a point of the shot; 0 is `trace_shot`'s straight shot.
    ///
    /// Example: `let shot = world.trace_falling_shot(eye, forward, 80.0, Bounces::Perfect(2), &targets, Some(my_id), 0.00016);`
    #[allow(clippy::too_many_arguments)]
    pub fn trace_falling_shot(
        &self,
        origin: DVec3,
        direction: DVec3,
        range_m: f64,
        bounces: Bounces,
        targets: &[Target],
        shooter_id: Option<u32>,
        drop_per_m: f64,
    ) -> Shot {
        let mut points: Vec<DVec3> = vec![origin];
        let (mut o, mut d, mut left_m, mut bounce_count) =
            (origin, direction.normalize(), range_m, 0);
        loop {
            let piece_m: f64 = if drop_per_m > 0.0 {
                left_m.min(DROP_PIECE_M)
            } else {
                left_m
            };
            let mut best: (f64, ShotHit, Option<DVec3>) = (piece_m, ShotHit::Range, None);
            let mut consider = |t: f64, hit: ShotHit, reflect: Option<DVec3>| {
                if t > SHOT_START_M && t < best.0 {
                    best = (t, hit, reflect);
                }
            };
            if let Some((_, far)) = find_sphere_hits_m(o, d, DVec3::ZERO, self.street_radius_m) {
                consider(
                    far,
                    ShotHit::Surface {
                        material: self.street_material,
                    },
                    None,
                );
            }
            for shell in &self.shells {
                let Some((near, far)) = find_sphere_hits_m(o, d, DVec3::ZERO, shell.radius_m)
                else {
                    continue;
                };
                for t in [near, far] {
                    let point: DVec3 = o + d * t;
                    if t <= SHOT_START_M || shell.is_in_hole(point) {
                        continue;
                    }
                    let is_from_inside: bool = o.length() < shell.radius_m;
                    let normal: DVec3 = -point.normalize();
                    let reflect: Option<DVec3> = (shell.is_reflective_inside
                        && is_from_inside
                        && !shell.is_ball()
                        && bounce_count < bounces.max_count)
                        .then(|| d - 2.0 * d.dot(normal) * normal);
                    consider(
                        t,
                        ShotHit::Surface {
                            material: shell.material,
                        },
                        reflect,
                    );
                    break;
                }
            }
            for city_box in &self.boxes {
                if let Some((t, _)) = city_box.find_hit_m(o, d) {
                    consider(
                        t,
                        ShotHit::Surface {
                            material: city_box.material,
                        },
                        None,
                    );
                }
            }
            for target in targets.iter().filter(|t| Some(t.id) != shooter_id) {
                for (height_m, radius_m, is_head) in [
                    (TORSO_HEIGHT_M, TORSO_RADIUS_M, false),
                    (HEAD_HEIGHT_M, HEAD_RADIUS_M, true),
                ] {
                    let center: DVec3 = target.feet + target.up * (height_m * target.height_frac);
                    if let Some((near, _)) = find_sphere_hits_m(o, d, center, radius_m) {
                        consider(
                            near,
                            ShotHit::Player {
                                id: target.id,
                                is_head,
                            },
                            None,
                        );
                    }
                }
            }
            let (t, hit, reflect) = best;
            let end: DVec3 = o + d * t;
            points.push(end);
            left_m -= t;
            if matches!(hit, ShotHit::Range) && reflect.is_none() && left_m > 0.0 {
                d = turn_flat(d, end.normalize_or(DVec3::Y), drop_per_m, t);
                o = end;
                continue;
            }
            match reflect {
                Some(new_d) => {
                    bounce_count += 1;
                    left_m *= bounces.reflectivity_frac;
                    d = new_d.normalize();
                    o = end + d * SURFACE_OFFSET_M;
                }
                None => {
                    return Shot {
                        points,
                        bounce_count,
                        hit,
                    };
                }
            }
        }
    }

    /// `get_speed_m_per_s` gives the walking or sprinting speed under `settings`: the map's times
    /// `movement`'s multiplier, as on Lunar Surface.
    ///
    /// Example: `let sprint_m_per_s = world.get_speed_m_per_s(&settings, true);`
    pub fn get_speed_m_per_s(&self, settings: &ServerSettings, is_sprinting: bool) -> f64 {
        if is_sprinting {
            self.map_sprint_m_per_s * settings.movement.sprint_speed_per_map_speed
        } else {
            self.map_walk_m_per_s * settings.movement.walk_speed_per_map_speed
        }
    }

    /// `get_jump` gives the jump speed and the gravity under `settings`, as Lunar Surface's
    /// `VoxelWorld::set_settings` makes them: a jump rises `movement.jump_height_m` in
    /// `movement.jump_time_per_map_jump_time` of the map's time to the top of a jump.
    ///
    /// Example: `let (jump_m_per_s, gravity_m_per_s2) = world.get_jump(&settings);`
    pub fn get_jump(&self, settings: &ServerSettings) -> (f64, f64) {
        let movement = &settings.movement;
        let half_jump_s: f64 = self.map_jump_m_per_s / self.map_gravity_m_per_s2
            * movement.jump_time_per_map_jump_time;
        let jump_m_per_s: f64 = 2.0 * movement.jump_height_m / half_jump_s;
        (jump_m_per_s, jump_m_per_s / half_jump_s)
    }

    /// `move_player` moves `player` by `input` for `dt_s` under `settings` and the head bob
    /// `head_bob`: first the movement rules every level shares (`MoveState::step`: crouching,
    /// sprint energy, the head bob's steps, a hit's and crouching's slow), then walking along the
    /// ground at the speed they allow, pulled along the ground toward each of `mouths` (W-144,
    /// `warped_shared::gravity`: faster toward one, slower away, drifting toward one standing
    /// still), gravity outward, jumping, riding a lift up through the
    /// glass while jump is held (hovering `LIFT_TOP_UNDER_GLASS_M` over the glass level until they
    /// walk off), and collisions with the street, the glass (from either side, except in its
    /// holes) and the boxes.
    ///
    /// Example: `world.move_player(&mut player, &input, &settings, &head_bob, 1.0 / 60.0);`
    pub fn move_player(
        &self,
        player: &mut CityPlayer,
        input: &CityInput,
        settings: &ServerSettings,
        head_bob: &HeadBobSettings,
        dt_s: f64,
    ) {
        let move_input: MoveInput = MoveInput {
            is_moving: input.forward_frac != 0.0 || input.right_frac != 0.0,
            is_sprint_held: input.is_sprinting,
            is_crouch_held: input.is_crouching,
            is_flying: false,
            is_on_ground: player.is_on_ground,
        };
        let rules: MoveRules = MoveRules {
            movement: &settings.movement,
            energy: &settings.sprint_energy,
            when_hit: &settings.when_hit,
            head_bob,
        };
        let slow_frac: f64 = player.move_state.step(&move_input, &rules, dt_s);
        player.crouch_drop_m = player.move_state.crouch_frac
            * player.eye_height_m
            * (1.0 - settings.movement.crouch_eye_height_frac);
        let speed_m_per_s: f64 =
            self.get_speed_m_per_s(settings, player.move_state.is_sprinting) * slow_frac;
        let (jump_m_per_s, gravity_m_per_s2) = self.get_jump(settings);
        let up: DVec3 = get_up(player.feet);
        let pull_m_per_s: DVec3 = get_flat_pull_m_per_s(
            &settings.gravity,
            self.get_speed_m_per_s(settings, false),
            player.feet + up * WARP_PULL_HEIGHT_M,
            up,
            &self.mouths,
        );
        let motion: StepMotion = StepMotion {
            speed_m_per_s,
            jump_m_per_s,
            gravity_m_per_s2,
            pull_m_per_s,
        };
        let step_count: usize =
            ((player.velocity.length() * dt_s / MAX_STEP_M).ceil() as usize).clamp(1, 16);
        let step_s: f64 = dt_s / step_count as f64;
        for k in 0..step_count {
            self.step_player(player, input, &motion, step_s, k == 0);
        }
    }

    /// `step_player` is one sub-step of `move_player`, walking at up to `motion`'s speed.
    fn step_player(
        &self,
        player: &mut CityPlayer,
        input: &CityInput,
        motion: &StepMotion,
        dt_s: f64,
        is_first: bool,
    ) {
        let speed_m_per_s: f64 = motion.speed_m_per_s;
        player.align_to_ground();
        let up: DVec3 = player.get_up();
        let (right, back) = (player.body.x_axis, player.body.z_axis);
        let mut wish: DVec3 = right * input.right_frac - back * input.forward_frac;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        let v_up: f64 = player.velocity.dot(up);
        let tangent: DVec3 = player.velocity - up * v_up;
        let control: f64 = if player.is_on_ground {
            1.0
        } else {
            AIR_CONTROL_FRAC
        };
        let target: DVec3 = if motion.pull_m_per_s == DVec3::ZERO {
            wish * speed_m_per_s
        } else {
            wish * speed_m_per_s + motion.pull_m_per_s
        };
        let new_tangent: DVec3 = tangent + (target - tangent) * (control * 12.0 * dt_s).min(1.0);
        let mut new_up: f64 = v_up - motion.gravity_m_per_s2 * dt_s;
        if is_first && input.is_jumping && player.is_on_ground {
            new_up = motion.jump_m_per_s - motion.gravity_m_per_s2 * dt_s;
        }
        let glass_m: f64 = self.get_glass().map_or(0.0, |glass| glass.radius_m);
        let top_m: f64 = glass_m - LIFT_TOP_UNDER_GLASS_M;
        let r: f64 = player.feet.length();
        if input.is_jumping && self.is_in_lift(player.feet) {
            new_up = if r > top_m {
                LIFT_M_PER_S.min((r - top_m) / dt_s)
            } else {
                new_up.max(0.0)
            };
        }
        player.velocity = new_tangent + up * new_up;
        let before: DVec3 = player.feet;
        player.feet += player.velocity * dt_s;
        let was_on_ground: bool = player.is_on_ground;
        player.is_on_ground = false;
        self.collide(player, before, was_on_ground);
    }

    /// `collide` pushes `player` out of the street, glass and boxes, `before` being where its feet
    /// were before this step (which side of the glass it is on). A player who was on the ground
    /// steps up onto a box whose top is at most `MAX_STEP_UP_M` above their feet (a stair's step)
    /// instead of being pushed back by its edge. A player under the glass passes through a stairs'
    /// hole a body radius wider on each side, so the glass doesn't push down one climbing at a
    /// step's edge; on the glass level, the glass holds a player up to the stairs' edge.
    fn collide(&self, player: &mut CityPlayer, before: DVec3, was_on_ground: bool) {
        let up: DVec3 = get_up(player.feet);
        let r: f64 = player.feet.length();
        if r > self.street_radius_m {
            player.feet *= self.street_radius_m / r;
            self.land(player, up);
        }
        if let Some(glass) = self.get_glass() {
            let was_above: bool = before.length() <= glass.radius_m;
            let side_margin_m: f64 = if was_above { 0.0 } else { BODY_RADIUS_M };
            let is_in_hole: bool = glass.is_in_widened_hole(player.feet, side_margin_m);
            let half_m: f64 = glass.thickness_m / 2.0;
            let r: f64 = player.feet.length();
            if !is_in_hole && was_above && r > glass.radius_m - half_m {
                player.feet *= (glass.radius_m - half_m) / r;
                self.land(player, up);
            } else if !is_in_hole && !was_above && r - BODY_HEIGHT_M < glass.radius_m + half_m {
                player.feet *= (glass.radius_m + half_m + BODY_HEIGHT_M) / r;
                let v_up: f64 = player.velocity.dot(up);
                if v_up > 0.0 {
                    player.velocity -= up * v_up;
                }
            }
        }
        for city_box in &self.boxes {
            if was_on_ground {
                self.step_up(player, city_box, up);
            }
            for height_m in [
                BODY_RADIUS_M,
                BODY_HEIGHT_M / 2.0,
                BODY_HEIGHT_M - BODY_RADIUS_M,
            ] {
                let point: DVec3 = player.feet + up * height_m;
                let local: DVec3 = city_box.to_local(point);
                let closest: DVec3 = local.clamp(-city_box.half_m, city_box.half_m);
                let away: DVec3 = local - closest;
                let distance_m: f64 = away.length();
                let push_local: DVec3 = if distance_m > 1e-9 {
                    if distance_m >= BODY_RADIUS_M {
                        continue;
                    }
                    away / distance_m * (BODY_RADIUS_M - distance_m)
                } else {
                    let depth: DVec3 = city_box.half_m - local.abs();
                    let axis: usize = if depth.x < depth.y && depth.x < depth.z {
                        0
                    } else if depth.y < depth.z {
                        1
                    } else {
                        2
                    };
                    let mut push: DVec3 = DVec3::ZERO;
                    push[axis] = (depth[axis] + BODY_RADIUS_M) * local[axis].signum();
                    push
                };
                let push: DVec3 = city_box.axes * push_local;
                player.feet += push;
                let normal: DVec3 = push.normalize_or_zero();
                if normal.dot(up) > GROUND_NORMAL_COS {
                    self.land(player, up);
                } else {
                    let into: f64 = player.velocity.dot(normal);
                    if into < 0.0 {
                        player.velocity -= normal * into;
                    }
                }
            }
        }
    }

    /// `step_up` lifts `player`'s feet onto `city_box`'s top when the box stands upright under
    /// them (its +y along `up`), its top is above the feet by at most `MAX_STEP_UP_M`, and the feet
    /// are within a body radius of its footprint.
    fn step_up(&self, player: &mut CityPlayer, city_box: &CityBox, up: DVec3) {
        if city_box.axes.y_axis.dot(up) < GROUND_NORMAL_COS {
            return;
        }
        let local: DVec3 = city_box.to_local(player.feet);
        let rise_m: f64 = city_box.half_m.y - local.y;
        let outside_m: f64 = DVec3::new(
            (local.x.abs() - city_box.half_m.x).max(0.0),
            0.0,
            (local.z.abs() - city_box.half_m.z).max(0.0),
        )
        .length();
        if rise_m > 0.0 && rise_m <= MAX_STEP_UP_M && outside_m < BODY_RADIUS_M {
            player.feet += city_box.axes.y_axis * rise_m;
            self.land(player, up);
        }
    }

    /// `land` stops a player's fall and marks them on the ground.
    fn land(&self, player: &mut CityPlayer, up: DVec3) {
        let v_up: f64 = player.velocity.dot(up);
        if v_up < 0.0 {
            player.velocity -= up * v_up;
        }
        player.is_on_ground = true;
    }

    /// `find_clear_spawn` gives the index of `team`'s spawn farthest from the nearest of `others`
    /// (other players' feet), from `first` on in turn when none are near, so players who join or
    /// come back together don't stand on top of each other.
    ///
    /// Example: `let player = world.make_player(0, world.find_clear_spawn(0, &others, 1));`
    pub fn find_clear_spawn(&self, team: u32, others: &[DVec3], first: usize) -> usize {
        let spawns: Vec<DVec3> = self
            .spawns
            .iter()
            .filter(|(t, _)| *t == team)
            .map(|(_, frame)| frame.w_axis.truncate())
            .collect();
        let gap_m = |k: usize| -> f64 {
            others
                .iter()
                .map(|other| (*other - spawns[k]).length())
                .fold(f64::INFINITY, f64::min)
        };
        (0..spawns.len())
            .map(|k| (first + k) % spawns.len())
            .fold(None, |best: Option<(usize, f64)>, k| match best {
                Some((_, best_m)) if gap_m(k) <= best_m => best,
                _ => Some((k, gap_m(k))),
            })
            .map_or(0, |(k, _)| k)
    }

    /// `make_player` puts a new player at a team's spawn (`index` picks among them), looking a
    /// little up (`SPAWN_PITCH_RAD`), since level ground inside the sphere rises ahead and would
    /// fill the view.
    ///
    /// Example: `let player = world.make_player(0, 1);`
    pub fn make_player(&self, team: u32, index: usize) -> CityPlayer {
        let spawns: Vec<&DMat4> = self
            .spawns
            .iter()
            .filter(|(t, _)| *t == team)
            .map(|(_, f)| f)
            .collect();
        let frame: DMat4 = **spawns
            .get(index % spawns.len().max(1))
            .unwrap_or(&&DMat4::IDENTITY);
        CityPlayer {
            feet: frame.w_axis.truncate(),
            body: DMat3::from_mat4(frame),
            pitch_rad: SPAWN_PITCH_RAD,
            velocity: DVec3::ZERO,
            is_on_ground: true,
            eye_height_m: self.eye_height_m,
            crouch_drop_m: 0.0,
            move_state: MoveState {
                crouch_frac: 0.0,
                energy_frac: 1.0,
                is_sprinting: false,
                bob_phase_rad: 0.0,
                bob_frac: 0.0,
                slowed_s: 0.0,
            },
        }
    }
}

/// `StepMotion` is how a player moves under the server settings: their speed this frame, the jump
/// speed and the gravity (`CityWorld::get_jump`), and the mouths' pull along the ground.
struct StepMotion {
    speed_m_per_s: f64,
    jump_m_per_s: f64,
    gravity_m_per_s2: f64,
    pull_m_per_s: DVec3,
}

/// `CityInput` is what a player asks of their body this frame: walking (forward and right, -1
/// to 1), sprint, jump and crouch held.
#[derive(Clone, Copy, Default)]
pub struct CityInput {
    pub forward_frac: f64,
    pub right_frac: f64,
    pub is_sprinting: bool,
    pub is_jumping: bool,
    pub is_crouching: bool,
}

/// `CityPlayer` is a player's body: feet, the body's axes (columns right, up, back; up toward the
/// centre), the head's pitch, velocity, whether it stands on something, its standing eye height
/// and how far crouching lowers it now, and the state of the movement rules every level shares
/// (`move_state`: crouch, sprint energy, sprinting, the head bob, a hit's slow), as Lunar
/// Surface's `VoxelPlayer` keeps them.
#[derive(Clone, Copy, Debug)]
pub struct CityPlayer {
    pub feet: DVec3,
    pub body: DMat3,
    pub pitch_rad: f64,
    pub velocity: DVec3,
    pub is_on_ground: bool,
    pub eye_height_m: f64,
    pub crouch_drop_m: f64,
    pub move_state: MoveState,
}

impl CityPlayer {
    /// `get_up` is the body's up.
    pub fn get_up(&self) -> DVec3 {
        self.body.y_axis
    }

    /// `align_to_ground` turns the body the least way that makes its up point at the centre.
    pub fn align_to_ground(&mut self) {
        let rotation: DQuat =
            DQuat::from_rotation_arc(self.body.y_axis.normalize(), get_up(self.feet));
        let m: DMat3 = DMat3::from_quat(rotation) * self.body;
        let y: DVec3 = m.y_axis.normalize();
        let x: DVec3 = (m.x_axis - y * m.x_axis.dot(y)).normalize();
        self.body = DMat3::from_cols(x, y, x.cross(y));
    }

    /// `look_around` turns the body about its up by `yaw_rad` and tilts the head by `pitch_rad`.
    ///
    /// Example: `player.look_around(0.01, -0.02);`
    pub fn look_around(&mut self, yaw_rad: f64, pitch_rad: f64) {
        self.body = DMat3::from_quat(DQuat::from_axis_angle(self.body.y_axis, yaw_rad)) * self.body;
        self.pitch_rad = (self.pitch_rad + pitch_rad).clamp(MIN_PITCH_RAD, MAX_PITCH_RAD);
    }

    /// `get_eye_frame` is the camera: the body's axes tilted by the pitch, at eye height, lowered
    /// by crouching (columns right, up, back, position; it looks along -z).
    ///
    /// Example: `let eye = player.get_eye_frame();`
    pub fn get_eye_frame(&self) -> DMat4 {
        let axes: DMat3 = self.body * DMat3::from_rotation_x(self.pitch_rad);
        DMat4::from_cols(
            axes.x_axis.extend(0.0),
            axes.y_axis.extend(0.0),
            axes.z_axis.extend(0.0),
            (self.feet + self.body.y_axis * (self.eye_height_m - self.crouch_drop_m)).extend(1.0),
        )
    }

    /// `FromEyeFrame` rebuilds a remote player's feet and up from their eye frame.
    ///
    /// Example: `let (feet, up) = CityPlayer::get_feet_up(eye, 1.6);`
    pub fn get_feet_up(eye: DMat4, eye_height_m: f64) -> (DVec3, DVec3) {
        let up: DVec3 = get_up(eye.w_axis.truncate());
        (eye.w_axis.truncate() - up * eye_height_m, up)
    }
}

#[cfg(test)]
mod tests {
    //! | Behaviour | Test |
    //! | --- | --- |
    //! | The map loads with 2 consoles a team and antipodal spawns | `map_has_one_console_a_team_and_antipodal_spawns` |
    //! | A player stands on the street and falls outward onto it | `players_fall_outward_onto_the_street` |
    //! | A player spawns where no other player stands (none near: the next in turn; one on the first: the other) | `spawns_keep_clear_of_other_players` |
    //! | A lift carries a player up onto the glass level, and they stand on the glass | `lifts_carry_players_onto_the_glass` |
    //! | A shot from the glass level reflects off the glass, at most the set number of times, each reflection keeping the reflectivity's share of the range (0, 0.5) | `shots_reflect_off_the_glass_up_to_the_limit` |
    //! | A shot from the street stops at the glass | `street_shots_stop_at_the_glass` |
    //! | A shot from the glass level goes straight across and hits a player | `glass_shots_cross_the_sphere` |
    //! | Range ends a shot | `range_ends_shots` |
    //! | Where a stairs' hole is open, across it (W-132 (City Sphere stairs hole)): its middle, just inside its stairs' edge, just outside it (inside the map's round hole), a body radius wider for a climber; a lift's hole stays round | `stairs_holes_open_only_over_their_stairs` |
    //! | A shot at the glass beside a stairs' hole: from the street (stops), from the glass level (reflects); straight down into it (ends on a step) | `shots_beside_the_stairs_hole_meet_the_glass` |
    //! | A player on the glass level beside a stairs' hole: walking its whole length just past the edge (stays on the glass); standing over the stairs (falls in) | `players_walk_on_the_glass_beside_the_stairs_hole` |
    //! | A player climbing each team's stairs at the edge of the steps, either side: reaches the glass level | `players_climb_the_stairs_at_their_edge` |
    //! | The team domes (W-148): the map's (one per team, centred on the glass under its pylon, spanning the share of the circumference asked for: 0.05, 0.2, 0.5; the two apart); a map without zone walls (none); zone walls read with and without zones; a zone of another team (fails); no regions | `each_team_has_a_dome_over_its_pylon` |
    //! | A linked mouth near a walker (W-144): walking toward it (farther in the same time), away (less far, still away), standing (drifts toward it); past the pull's reach (each the same as with no mouth, bit for bit) | `mouths_pull_walkers_toward_them` |

    use super::*;

    #[test]
    fn each_team_has_a_dome_over_its_pylon() {
        let world: CityWorld = CityWorld::Builtin().expect("the map");
        let glass_m: f64 = world.get_glass().expect("a glass level").radius_m;
        let domes: Vec<TeamDome> = world.get_team_domes(0.2);
        assert_eq!(
            domes.iter().map(|dome| dome.team).collect::<Vec<u32>>(),
            [0, 1]
        );
        for dome in &domes {
            let pylon: &CityBox = world
                .boxes
                .iter()
                .find(|b| {
                    b.group == PYLON_GROUP && b.name.starts_with(TEAM_NAMES[dome.team as usize])
                })
                .expect("the team's pylon");
            assert!((dome.centre.length() - glass_m).abs() < 1e-9);
            assert!(dome.centre.normalize().dot(pylon.center.normalize()) > 1.0 - 1e-9);
            assert!((dome.centre - pylon.center).length() < dome.radius_m);
            let rim_angle_rad: f64 = 2.0 * (dome.radius_m / (2.0 * glass_m)).asin();
            assert!((2.0 * rim_angle_rad / std::f64::consts::TAU - 0.2).abs() < 1e-9);
            assert!((dome.radius_m - 10.506).abs() < 1e-3 && dome.opacity_frac < 1.0);
        }
        assert!((domes[0].centre - domes[1].centre).length() > 2.0 * domes[0].radius_m);
        let small: f64 = world.get_team_domes(0.05)[0].radius_m;
        let half: f64 = world.get_team_domes(0.5)[0].radius_m;
        assert!(small < domes[0].radius_m && (half - glass_m * 2.0_f64.sqrt()).abs() < 1e-9);
        let mut without_walls: CityWorld = CityWorld::Builtin().expect("the map");
        without_walls.dome_opacity_frac = None;
        assert_eq!(without_walls.get_team_domes(0.2), []);
        let read = |text: &str| -> Result<Option<f64>> {
            let regions: JsonRegions = serde_json::from_str(text)?;
            get_dome_opacity_frac(Some(&regions))
        };
        let blue_zone: &str = r#"{"team": "blue", "v_min_m": 4.0, "v_max_m": null}"#;
        let walls: &str = r#"{"height_m": 6.0, "thickness_m": 0.6, "opacity_frac": 0.25}"#;
        let without_walls: String = format!(r#"{{"zones": [{blue_zone}], "zone_walls": null}}"#);
        assert_eq!(read(&without_walls).expect("no walls"), None);
        let with_walls: String = format!(r#"{{"zones": [], "zone_walls": {walls}}}"#);
        assert_eq!(read(&with_walls).expect("walls"), Some(0.25));
        let green: String = format!(
            r#"{{"zones": [{}], "zone_walls": {walls}}}"#,
            blue_zone.replace("blue", "green")
        );
        assert!(read(&green).is_err());
        assert_eq!(get_dome_opacity_frac(None).expect("no regions"), None);
    }

    const EDGE_M: f64 = 0.1;
    const BESIDE_STAIRS_M: f64 = 0.8;
    const SHOOTER_SIDE_M: f64 = 4.0;
    const ALONG_HOLE_M: f64 = 3.0;
    const FALL_S: f64 = 1.0;
    const CLIMB_S: f64 = 20.0;
    const LEAD_IN_M: f64 = 1.0;
    const GLASS_LEVEL_TOLERANCE_M: f64 = 0.05;
    const STEP_S: f64 = 1.0 / 60.0;
    const PULL_WALK_S: f64 = 0.5;
    const NEAR_MOUTH_M: f64 = 8.0;
    const FAR_MOUTH_M: f64 = 40.0;

    fn world() -> CityWorld {
        CityWorld::Builtin().expect("the shipped map loads")
    }

    /// `get_stairs_holes` gives the glass's stairs' holes, one a team.
    fn get_stairs_holes(glass: &Shell) -> Vec<Hole> {
        let holes: Vec<Hole> = glass
            .holes
            .iter()
            .copied()
            .filter(|hole| hole.half_width_m.is_finite())
            .collect();
        assert_eq!(holes.len(), TEAM_NAMES.len());
        holes
    }

    /// `get_beside` gives the point `side_m` to the side of `hole`'s middle along its stairs'
    /// side axis, `along_m` along the stairs, at `radius_m` from the centre.
    fn get_beside(hole: &Hole, side_m: f64, along_m: f64, radius_m: f64) -> DVec3 {
        let along: DVec3 = hole.side.cross(hole.direction);
        (hole.direction * radius_m + hole.side * side_m + along * along_m).normalize() * radius_m
    }

    /// `walk_toward` walks `player` toward `target` for one step of `STEP_S`, turned to face it.
    fn walk_toward(world: &CityWorld, player: &mut CityPlayer, target: DVec3) {
        let up: DVec3 = get_up(player.feet);
        let to: DVec3 = target - player.feet;
        let back: DVec3 = -(to - up * to.dot(up)).normalize();
        player.body = DMat3::from_cols(up.cross(back), up, back);
        let input: CityInput = CityInput {
            forward_frac: 1.0,
            ..CityInput::default()
        };
        world.move_player(player, &input, &settings(), &head_bob(), STEP_S);
    }

    /// `head_bob` is the built-in player settings' head bob.
    fn head_bob() -> HeadBobSettings {
        crate::settings::PerPlayerSettings::Builtin().view.head_bob
    }

    /// `settings` is the built-in `server_settings.yaml`.
    fn settings() -> ServerSettings {
        ServerSettings::Builtin()
    }

    /// `walk_by_a_mouth` walks blue's first spawn with `input` for `PULL_WALK_S`, with one linked
    /// mouth `mouth_m` ahead at the body's middle (none for `None`), and gives where the feet end
    /// up and how far they went toward it.
    fn walk_by_a_mouth(input: &CityInput, mouth_m: Option<f64>) -> (DVec3, f64) {
        let mut world: CityWorld = world();
        let mut player: CityPlayer = world.make_player(0, 0);
        let (start, ahead) = (player.feet, -player.body.z_axis);
        let middle: DVec3 = start + get_up(start) * WARP_PULL_HEIGHT_M;
        world.mouths = mouth_m
            .map(|ahead_m| FlatWarp {
                centre_m: middle + ahead * ahead_m,
                radius_m: 1.0,
                is_mouth: true,
            })
            .into_iter()
            .collect();
        for _ in 0..(PULL_WALK_S / STEP_S).round() as usize {
            world.move_player(&mut player, input, &settings(), &head_bob(), STEP_S);
        }
        (player.feet, (player.feet - start).dot(ahead))
    }

    #[test]
    fn mouths_pull_walkers_toward_them() {
        let forward: CityInput = CityInput {
            forward_frac: 1.0,
            ..CityInput::default()
        };
        let back: CityInput = CityInput {
            forward_frac: -1.0,
            ..CityInput::default()
        };
        let still: CityInput = CityInput::default();
        let (_, free_m) = walk_by_a_mouth(&forward, None);
        let (_, toward_m) = walk_by_a_mouth(&forward, Some(NEAR_MOUTH_M));
        assert!(toward_m > free_m, "{toward_m} against {free_m}");
        let (_, free_back_m) = walk_by_a_mouth(&back, None);
        let (_, away_m) = walk_by_a_mouth(&back, Some(NEAR_MOUTH_M));
        assert!(
            free_back_m < away_m && away_m < 0.0,
            "{away_m} against {free_back_m}"
        );
        let (_, drift_m) = walk_by_a_mouth(&still, Some(NEAR_MOUTH_M));
        assert!(drift_m > 0.0, "{drift_m}");
        for input in [&forward, &back, &still] {
            assert_eq!(
                walk_by_a_mouth(input, Some(FAR_MOUTH_M)).0,
                walk_by_a_mouth(input, None).0
            );
        }
    }

    #[test]
    fn map_has_one_console_a_team_and_antipodal_spawns() {
        let world: CityWorld = world();
        for team in 0..2 {
            assert_eq!(world.consoles.iter().filter(|c| c.team == team).count(), 1);
        }
        let blue: DVec3 = world
            .spawns
            .iter()
            .find(|s| s.0 == 0)
            .unwrap()
            .1
            .w_axis
            .truncate();
        let red: DVec3 = world
            .spawns
            .iter()
            .find(|s| s.0 == 1)
            .unwrap()
            .1
            .w_axis
            .truncate();
        assert!((blue + red).length() < 1e-4);
    }

    /// `spawns_keep_clear_of_other_players` picks the first spawn with nobody around, and the
    /// other spawn when a player stands on the first.
    #[test]
    fn spawns_keep_clear_of_other_players() {
        let world: CityWorld = world();
        assert_eq!(world.find_clear_spawn(0, &[], 0), 0);
        assert_eq!(world.find_clear_spawn(0, &[], 1), 1);
        let on_first: DVec3 = world.make_player(0, 0).feet;
        assert_eq!(world.find_clear_spawn(0, &[on_first], 0), 1);
    }

    #[test]
    fn players_fall_outward_onto_the_street() {
        let world: CityWorld = world();
        let mut player: CityPlayer = world.make_player(0, 0);
        player.feet *= 0.95;
        for _ in 0..120 {
            world.move_player(
                &mut player,
                &CityInput::default(),
                &settings(),
                &head_bob(),
                1.0 / 60.0,
            );
        }
        assert!((player.feet.length() - world.street_radius_m).abs() < 1e-3);
        assert!(player.is_on_ground);
        assert!(player.get_up().dot(-player.feet.normalize()) > 0.999);
    }

    #[test]
    fn lifts_carry_players_onto_the_glass() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let hole: DVec3 = glass.holes[0].direction;
        let mut player: CityPlayer = world.make_player(0, 0);
        player.feet = hole * world.street_radius_m;
        let ride = CityInput {
            is_jumping: true,
            ..CityInput::default()
        };
        for _ in 0..240 {
            world.move_player(&mut player, &ride, &settings(), &head_bob(), 1.0 / 60.0);
            if player.feet.length() < glass.radius_m - 1.0 {
                break;
            }
        }
        assert_eq!(world.get_side(player.feet), Side::Glass);
        let step = CityInput {
            forward_frac: 1.0,
            ..CityInput::default()
        };
        player.body = DMat3::from_cols(player.body.x_axis, player.body.y_axis, player.body.z_axis);
        for _ in 0..90 {
            world.move_player(&mut player, &step, &settings(), &head_bob(), 1.0 / 60.0);
        }
        for _ in 0..120 {
            world.move_player(
                &mut player,
                &CityInput::default(),
                &settings(),
                &head_bob(),
                1.0 / 60.0,
            );
        }
        assert_eq!(world.get_side(player.feet), Side::Glass);
        assert!(player.is_on_ground);
    }

    #[test]
    fn shots_reflect_off_the_glass_up_to_the_limit() {
        let world: CityWorld = world();
        let glass_r: f64 = world.get_glass().unwrap().radius_m;
        let origin: DVec3 = DVec3::new(0.3, 0.2, 1.0).normalize() * (glass_r - 1.6);
        let direction: DVec3 = DVec3::new(0.2, 1.0, 0.4).normalize();
        for max_bounce_count in 0..=2 {
            let shot: Shot = world.trace_shot(
                origin,
                direction,
                500.0,
                Bounces::Perfect(max_bounce_count),
                &[],
                None,
            );
            assert_eq!(shot.bounce_count, max_bounce_count);
            assert_eq!(shot.points.len() as u32, max_bounce_count + 2);
        }
        let shot: Shot = world.trace_shot(origin, direction, 500.0, Bounces::Perfect(1), &[], None);
        let (a, b, c) = (shot.points[0], shot.points[1], shot.points[2]);
        let normal: DVec3 = b.normalize();
        let (incoming, outgoing) = ((b - a).normalize(), (c - b).normalize());
        assert!((incoming.dot(normal) + outgoing.dot(normal)).abs() < 1e-6);
        let first_leg_m: f64 = (b - a).length();
        let range_m: f64 = first_leg_m + 4.0;
        for reflectivity_frac in [0.0, 0.5] {
            let bounces: Bounces = Bounces {
                max_count: 1,
                reflectivity_frac,
            };
            let dimmed: Shot = world.trace_shot(origin, direction, range_m, bounces, &[], None);
            let second_leg_m: f64 = (dimmed.points[2] - dimmed.points[1]).length();
            assert_eq!(dimmed.hit, ShotHit::Range, "{dimmed:?}");
            assert!(
                (second_leg_m - SURFACE_OFFSET_M - 4.0 * reflectivity_frac).abs() < 1e-6,
                "{second_leg_m}"
            );
        }
    }

    #[test]
    fn street_shots_stop_at_the_glass() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let mut player: CityPlayer = world.make_player(0, 0);
        player.pitch_rad = 1.4;
        let eye: DMat4 = player.get_eye_frame();
        let shot: Shot = world.trace_shot(
            eye.w_axis.truncate(),
            -eye.z_axis.truncate(),
            500.0,
            Bounces::Perfect(2),
            &[],
            None,
        );
        assert_eq!(shot.bounce_count, 0);
        assert!(
            (shot.points[1].length() - glass.radius_m).abs() < 1e-6
                || shot.points[1].length() > glass.radius_m
        );
    }

    #[test]
    fn glass_shots_cross_the_sphere() {
        let world: CityWorld = world();
        let glass_r: f64 = world.get_glass().unwrap().radius_m;
        let a: DVec3 = DVec3::new(0.0, 0.0, 1.0) * (glass_r - 1.6);
        let feet: DVec3 = DVec3::new(0.0, 1.0, -0.2).normalize() * glass_r;
        let target = Target {
            id: 7,
            feet,
            up: get_up(feet),
            height_frac: 1.0,
        };
        let aim: DVec3 = (feet + target.up * TORSO_HEIGHT_M - a).normalize();
        let shot: Shot = world.trace_shot(a, aim, 500.0, Bounces::Perfect(2), &[target], Some(1));
        assert_eq!(
            shot.hit,
            ShotHit::Player {
                id: 7,
                is_head: false
            },
            "{shot:?}"
        );
    }

    #[test]
    fn range_ends_shots() {
        let world: CityWorld = world();
        let origin: DVec3 = DVec3::new(0.0, 0.0, 10.0);
        let shot: Shot = world.trace_shot(origin, DVec3::Y, 1.0, Bounces::Perfect(2), &[], None);
        assert_eq!(shot.hit, ShotHit::Range);
        assert!((shot.points[1] - origin - DVec3::Y).length() < 1e-9);
    }

    /// `stairs_holes_open_only_over_their_stairs` checks across each stairs' hole that it is open
    /// over its stairs and shut beside them, though the map's round hole reaches there, except a
    /// body radius wider for a player climbing; and that the lifts' holes stay round.
    #[test]
    fn stairs_holes_open_only_over_their_stairs() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        for hole in get_stairs_holes(glass) {
            let at = |side_m: f64| get_beside(&hole, side_m, 0.0, glass.radius_m);
            let (inside_m, outside_m) = (hole.half_width_m - EDGE_M, hole.half_width_m + EDGE_M);
            let widened_outside_m: f64 = outside_m + BODY_RADIUS_M;
            assert!(glass.is_in_hole(at(0.0)));
            assert!(glass.is_in_hole(at(inside_m)) && glass.is_in_hole(at(-inside_m)));
            assert!(at(widened_outside_m).normalize().dot(hole.direction) >= hole.cos);
            assert!(!glass.is_in_hole(at(outside_m)) && !glass.is_in_hole(at(-outside_m)));
            assert!(glass.is_in_widened_hole(at(outside_m), BODY_RADIUS_M));
            assert!(!glass.is_in_widened_hole(at(widened_outside_m), BODY_RADIUS_M));
        }
        assert_eq!(world.lift_holes.len(), glass.holes.len() - TEAM_NAMES.len());
        for hole in &world.lift_holes {
            let sin: f64 = (1.0 - hole.cos * hole.cos).sqrt() * (1.0 - EDGE_M / glass.radius_m);
            let edge: DVec3 = hole.direction * (1.0 - sin * sin).sqrt()
                + hole.direction.any_orthonormal_vector() * sin;
            assert!(hole.half_width_m.is_infinite() && glass.is_in_hole(edge));
        }
    }

    /// `shots_beside_the_stairs_hole_meet_the_glass` shoots at the glass just beside each stairs'
    /// hole, where the map's round hole reaches: from the street the shot stops on the glass, and
    /// from the glass level it reflects there; a shot straight down into the hole ends on a step.
    #[test]
    fn shots_beside_the_stairs_hole_meet_the_glass() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let level_m: f64 = glass.radius_m - glass.thickness_m / 2.0;
        let stairs_material: u8 = world
            .boxes
            .iter()
            .find(|b| b.group == STAIRS_GROUP)
            .unwrap()
            .material;
        for hole in get_stairs_holes(glass) {
            let beside_m: f64 = hole.half_width_m + BESIDE_STAIRS_M;
            let beside: DVec3 = get_beside(&hole, beside_m, 0.0, glass.radius_m);
            assert!(beside.normalize().dot(hole.direction) >= hole.cos);
            let street_eye: DVec3 = get_beside(
                &hole,
                SHOOTER_SIDE_M,
                0.0,
                world.street_radius_m - world.eye_height_m,
            );
            let glass_eye: DVec3 =
                get_beside(&hole, SHOOTER_SIDE_M, 0.0, level_m - world.eye_height_m);
            for (eye, bounce_count) in [(street_eye, 0), (glass_eye, 1)] {
                let shot: Shot = world.trace_shot(
                    eye,
                    (beside - eye).normalize(),
                    500.0,
                    Bounces::Perfect(1),
                    &[],
                    None,
                );
                assert!(
                    (shot.points[1] - beside).length() < EDGE_M
                        && shot.bounce_count == bounce_count,
                    "{shot:?}"
                );
            }
            let eye: DVec3 = hole.direction * (level_m - world.eye_height_m);
            let down: Shot =
                world.trace_shot(eye, hole.direction, 500.0, Bounces::Perfect(1), &[], None);
            assert_eq!(
                down.hit,
                ShotHit::Surface {
                    material: stairs_material
                }
            );
        }
    }

    /// `players_walk_on_the_glass_beside_the_stairs_hole` walks a player on the glass level along
    /// each stairs' hole, past its whole length, with their feet just beyond the stairs' edge where
    /// the map's round hole reaches: they stay on the glass level. A player standing over the
    /// stairs falls in.
    #[test]
    fn players_walk_on_the_glass_beside_the_stairs_hole() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let level_m: f64 = glass.radius_m - glass.thickness_m / 2.0;
        for hole in get_stairs_holes(glass) {
            for side_m in [hole.half_width_m + EDGE_M, -hole.half_width_m - EDGE_M] {
                let mut player: CityPlayer = world.make_player(0, 0);
                player.feet = get_beside(&hole, side_m, -ALONG_HOLE_M, level_m);
                let target: DVec3 = get_beside(&hole, side_m, ALONG_HOLE_M, level_m);
                while (target - player.feet).length() > EDGE_M {
                    walk_toward(&world, &mut player, target);
                    assert!(
                        (player.feet.length() - level_m).abs() < GLASS_LEVEL_TOLERANCE_M,
                        "fell in at {:?}",
                        player.feet
                    );
                }
            }
            let mut player: CityPlayer = world.make_player(0, 0);
            player.feet = hole.direction * level_m;
            for _ in 0..(FALL_S / STEP_S) as usize {
                world.move_player(
                    &mut player,
                    &CityInput::default(),
                    &settings(),
                    &head_bob(),
                    STEP_S,
                );
            }
            assert!(player.feet.length() > glass.radius_m + glass.thickness_m);
        }
    }

    /// `players_climb_the_stairs_at_their_edge` walks up each team's stairs with the feet just
    /// beyond the steps' edge, on either side: the glass, a body radius away, doesn't push the
    /// player down off the stairs, and they reach the glass level.
    #[test]
    fn players_climb_the_stairs_at_their_edge() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let level_m: f64 = glass.radius_m - glass.thickness_m / 2.0;
        for team in 0..TEAM_NAMES.len() as u32 {
            let (bottom, top) = world.get_stairs_ends(team).unwrap();
            let step: &CityBox = world
                .boxes
                .iter()
                .find(|b| b.group == STAIRS_GROUP && b.name.starts_with(TEAM_NAMES[team as usize]))
                .unwrap();
            let up_stairs: DVec3 = (top - bottom * top.dot(bottom)).normalize();
            for side_m in [step.half_m.x + EDGE_M, -step.half_m.x - EDGE_M] {
                let at = |d: DVec3, radius_m: f64| {
                    (d * radius_m + step.axes.x_axis * side_m).normalize() * radius_m
                };
                let mut player: CityPlayer = world.make_player(team, 0);
                let start: DVec3 = bottom - up_stairs * (LEAD_IN_M / world.street_radius_m);
                player.feet = at(start.normalize(), world.street_radius_m);
                let target: DVec3 = at(top, level_m);
                let mut time_s: f64 = 0.0;
                while (target - player.feet).length() > EDGE_M && time_s < CLIMB_S {
                    walk_toward(&world, &mut player, target);
                    time_s += STEP_S;
                }
                assert!(
                    (player.feet.length() - level_m).abs() < GLASS_LEVEL_TOLERANCE_M,
                    "team {team}, side {side_m} m: stopped at radius {:.2} m",
                    player.feet.length()
                );
            }
        }
    }
}
