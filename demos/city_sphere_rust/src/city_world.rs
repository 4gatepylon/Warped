//! `city_world` is City Sphere's world: the map read from `common/maps/city_sphere/`, the players'
//! movement inside the sphere (gravity outward, so up is toward the centre), and shots, which go
//! straight, stop at the street, buildings and the core, and reflect off the glass shell's inner
//! side.
//!
//! Interface: `CityWorld::Builtin`, `CityWorld::FromJson`, `CityPlayer`, `CityInput`, `Shot`,
//! `ShotHit`, `CityWorld::trace_shot`, `CityWorld::move_player`, `Side`, `get_up`.

use std::error::Error;

use glam::{DMat3, DMat4, DQuat, DVec3};
use serde::Deserialize;

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
const LIFT_M_PER_S: f64 = 6.0;
const LIFT_TOP_UNDER_GLASS_M: f64 = 1.0;
const AIR_CONTROL_FRAC: f64 = 0.35;
const SHOT_START_M: f64 = 1e-4;
const SURFACE_OFFSET_M: f64 = 1e-3;
const MAX_STEP_M: f64 = 0.2;
const MIN_PITCH_RAD: f64 = -1.5;
const MAX_PITCH_RAD: f64 = 1.5;
const GROUND_NORMAL_COS: f64 = 0.6;

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
}

/// `JsonLasers` is the map's laser colour: one for every team, or one per team.
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

/// `Shell` is a sphere about the centre: the glass (walkable on its inner side, reflecting shots
/// from that side) or the core (a solid ball). Holes are (unit direction, cosine of their angular
/// radius).
pub struct Shell {
    pub radius_m: f64,
    pub thickness_m: f64,
    pub material: u8,
    pub is_walkable_inside: bool,
    pub is_reflective_inside: bool,
    pub holes: Vec<(DVec3, f64)>,
}

impl Shell {
    /// `is_in_hole` says whether the direction of `point` from the centre is in one of the holes.
    ///
    /// Example: `let is_open = shell.is_in_hole(feet);`
    pub fn is_in_hole(&self, point: DVec3) -> bool {
        let d: DVec3 = point.normalize_or_zero();
        self.holes.iter().any(|&(hole, cos)| d.dot(hole) >= cos)
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
    pub gravity_m_per_s2: f64,
    pub eye_height_m: f64,
    pub walk_m_per_s: f64,
    pub sprint_m_per_s: f64,
    pub jump_m_per_s: f64,
    pub materials: Vec<Material>,
    pub shells: Vec<Shell>,
    pub boxes: Vec<CityBox>,
    pub consoles: Vec<Console>,
    pub spawns: Vec<(u32, DMat4)>,
    pub laser_colors: [[f32; 3]; 2],
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

/// `Target` is a player a shot can hit: id, feet and up.
#[derive(Clone, Copy)]
pub struct Target {
    pub id: u32,
    pub feet: DVec3,
    pub up: DVec3,
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
fn find_sphere_hits_m(origin: DVec3, direction: DVec3, center: DVec3, radius_m: f64) -> Option<(f64, f64)> {
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
                    .map(|h| (DVec3::from(h.direction).normalize(), h.angular_radius_rad.cos()))
                    .collect(),
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
            gravity_m_per_s2: map.gravity_m_per_s2,
            eye_height_m: map.player.eye_height_m,
            walk_m_per_s: map.player.walk_m_per_s,
            sprint_m_per_s: map.player.sprint_m_per_s,
            jump_m_per_s: map.player.jump_m_per_s,
            materials,
            shells,
            boxes,
            consoles,
            spawns,
            laser_colors: match &map.lasers {
                Some(JsonLasers { team_colors: Some(colors), .. }) => *colors,
                Some(JsonLasers { color: Some(color), .. }) => [*color; 2],
                Some(_) => return Err("map: lasers needs color or team_colors".into()),
                None => DEFAULT_LASER_COLORS,
            },
        })
    }

    /// `get_glass` gives the walkable glass shell.
    pub fn get_glass(&self) -> Option<&Shell> {
        self.shells.iter().find(|s| s.is_walkable_inside)
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

    /// `is_in_lift` says whether `feet` are over a lift pad, under a hole in the glass.
    ///
    /// Example: `let can_ride = world.is_in_lift(feet);`
    pub fn is_in_lift(&self, feet: DVec3) -> bool {
        self.get_glass().is_some_and(|glass| glass.is_in_hole(feet))
    }

    /// `trace_shot` follows a shot from `origin` along `direction` for up to `range_m`: it stops
    /// at the street, boxes, the core, a target (not `shooter_id`) or the glass from outside, and
    /// reflects off the glass's inner side about its normal at most `max_bounce_count` times (the
    /// next meeting with the glass stops it).
    ///
    /// Example: `let shot = world.trace_shot(eye, forward, 80.0, 2, &targets, Some(my_id));`
    pub fn trace_shot(
        &self,
        origin: DVec3,
        direction: DVec3,
        range_m: f64,
        max_bounce_count: u32,
        targets: &[Target],
        shooter_id: Option<u32>,
    ) -> Shot {
        let mut points: Vec<DVec3> = vec![origin];
        let (mut o, mut d, mut left_m, mut bounce_count) = (origin, direction.normalize(), range_m, 0);
        loop {
            let mut best: (f64, ShotHit, Option<DVec3>) = (left_m, ShotHit::Range, None);
            let mut consider = |t: f64, hit: ShotHit, reflect: Option<DVec3>| {
                if t > SHOT_START_M && t < best.0 {
                    best = (t, hit, reflect);
                }
            };
            if let Some((_, far)) = find_sphere_hits_m(o, d, DVec3::ZERO, self.street_radius_m) {
                consider(far, ShotHit::Surface { material: self.street_material }, None);
            }
            for shell in &self.shells {
                let Some((near, far)) = find_sphere_hits_m(o, d, DVec3::ZERO, shell.radius_m) else {
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
                        && bounce_count < max_bounce_count)
                        .then(|| d - 2.0 * d.dot(normal) * normal);
                    consider(t, ShotHit::Surface { material: shell.material }, reflect);
                    break;
                }
            }
            for city_box in &self.boxes {
                if let Some((t, _)) = city_box.find_hit_m(o, d) {
                    consider(t, ShotHit::Surface { material: city_box.material }, None);
                }
            }
            for target in targets.iter().filter(|t| Some(t.id) != shooter_id) {
                for (height_m, radius_m, is_head) in
                    [(TORSO_HEIGHT_M, TORSO_RADIUS_M, false), (HEAD_HEIGHT_M, HEAD_RADIUS_M, true)]
                {
                    let center: DVec3 = target.feet + target.up * height_m;
                    if let Some((near, _)) = find_sphere_hits_m(o, d, center, radius_m) {
                        consider(near, ShotHit::Player { id: target.id, is_head }, None);
                    }
                }
            }
            let (t, hit, reflect) = best;
            let end: DVec3 = o + d * t;
            points.push(end);
            left_m -= t;
            match reflect {
                Some(new_d) => {
                    bounce_count += 1;
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

    /// `move_player` moves `player` by `input` for `dt_s`: walking along the ground, gravity
    /// outward, jumping, riding a lift up through the glass while jump is held (hovering
    /// `LIFT_TOP_UNDER_GLASS_M` over the glass level until they walk off), and collisions with
    /// the street, the glass (from either side, except in its holes) and the boxes.
    ///
    /// Example: `world.move_player(&mut player, &input, 1.0 / 60.0);`
    pub fn move_player(&self, player: &mut CityPlayer, input: &CityInput, dt_s: f64) {
        let step_count: usize = ((player.velocity.length() * dt_s / MAX_STEP_M).ceil() as usize).clamp(1, 16);
        let step_s: f64 = dt_s / step_count as f64;
        for k in 0..step_count {
            self.step_player(player, input, step_s, k == 0);
        }
    }

    /// `step_player` is one sub-step of `move_player`.
    fn step_player(&self, player: &mut CityPlayer, input: &CityInput, dt_s: f64, is_first: bool) {
        player.align_to_ground();
        let up: DVec3 = player.get_up();
        let (right, back) = (player.body.x_axis, player.body.z_axis);
        let mut wish: DVec3 = right * input.right_frac - back * input.forward_frac;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        let speed_m_per_s: f64 = if input.is_sprinting { self.sprint_m_per_s } else { self.walk_m_per_s }
            * input.speed_frac;
        let v_up: f64 = player.velocity.dot(up);
        let tangent: DVec3 = player.velocity - up * v_up;
        let control: f64 = if player.is_on_ground { 1.0 } else { AIR_CONTROL_FRAC };
        let target: DVec3 = wish * speed_m_per_s;
        let new_tangent: DVec3 = tangent + (target - tangent) * (control * 12.0 * dt_s).min(1.0);
        let mut new_up: f64 = v_up - self.gravity_m_per_s2 * dt_s;
        if is_first && input.is_jumping && player.is_on_ground {
            new_up = self.jump_m_per_s;
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
        player.is_on_ground = false;
        self.collide(player, before);
    }

    /// `collide` pushes `player` out of the street, glass and boxes, `before` being where its feet
    /// were before this step (which side of the glass it is on).
    fn collide(&self, player: &mut CityPlayer, before: DVec3) {
        let up: DVec3 = get_up(player.feet);
        let r: f64 = player.feet.length();
        if r > self.street_radius_m {
            player.feet *= self.street_radius_m / r;
            self.land(player, up);
        }
        if let Some(glass) = self.get_glass()
            && !glass.is_in_hole(player.feet)
        {
            let half_m: f64 = glass.thickness_m / 2.0;
            let r: f64 = player.feet.length();
            let was_above: bool = before.length() <= glass.radius_m;
            if was_above && r > glass.radius_m - half_m {
                player.feet *= (glass.radius_m - half_m) / r;
                self.land(player, up);
            } else if !was_above && r - BODY_HEIGHT_M < glass.radius_m + half_m {
                player.feet *= (glass.radius_m + half_m + BODY_HEIGHT_M) / r;
                let v_up: f64 = player.velocity.dot(up);
                if v_up > 0.0 {
                    player.velocity -= up * v_up;
                }
            }
        }
        for city_box in &self.boxes {
            for height_m in [BODY_RADIUS_M, BODY_HEIGHT_M / 2.0, BODY_HEIGHT_M - BODY_RADIUS_M] {
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

    /// `land` stops a player's fall and marks them on the ground.
    fn land(&self, player: &mut CityPlayer, up: DVec3) {
        let v_up: f64 = player.velocity.dot(up);
        if v_up < 0.0 {
            player.velocity -= up * v_up;
        }
        player.is_on_ground = true;
    }

    /// `make_player` puts a new player at a team's spawn (`index` picks among them).
    ///
    /// Example: `let player = world.make_player(0, 1);`
    pub fn make_player(&self, team: u32, index: usize) -> CityPlayer {
        let spawns: Vec<&DMat4> = self.spawns.iter().filter(|(t, _)| *t == team).map(|(_, f)| f).collect();
        let frame: DMat4 = **spawns.get(index % spawns.len().max(1)).unwrap_or(&&DMat4::IDENTITY);
        CityPlayer {
            feet: frame.w_axis.truncate(),
            body: DMat3::from_mat4(frame),
            pitch_rad: 0.0,
            velocity: DVec3::ZERO,
            is_on_ground: true,
            eye_height_m: self.eye_height_m,
        }
    }
}

/// `CityInput` is what a player asks of their body this frame.
#[derive(Clone, Copy, Default)]
pub struct CityInput {
    pub forward_frac: f64,
    pub right_frac: f64,
    pub is_sprinting: bool,
    pub is_jumping: bool,
    pub speed_frac: f64,
}

/// `CityPlayer` is a player's body: feet, the body's axes (columns right, up, back; up toward the
/// centre), the head's pitch, velocity and whether it stands on something.
#[derive(Clone, Copy, Debug)]
pub struct CityPlayer {
    pub feet: DVec3,
    pub body: DMat3,
    pub pitch_rad: f64,
    pub velocity: DVec3,
    pub is_on_ground: bool,
    pub eye_height_m: f64,
}

impl CityPlayer {
    /// `get_up` is the body's up.
    pub fn get_up(&self) -> DVec3 {
        self.body.y_axis
    }

    /// `align_to_ground` turns the body the least way that makes its up point at the centre.
    pub fn align_to_ground(&mut self) {
        let rotation: DQuat = DQuat::from_rotation_arc(self.body.y_axis.normalize(), get_up(self.feet));
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

    /// `get_eye_frame` is the camera: the body's axes tilted by the pitch, at eye height (columns
    /// right, up, back, position; it looks along -z).
    ///
    /// Example: `let eye = player.get_eye_frame();`
    pub fn get_eye_frame(&self) -> DMat4 {
        let axes: DMat3 = self.body * DMat3::from_rotation_x(self.pitch_rad);
        DMat4::from_cols(
            axes.x_axis.extend(0.0),
            axes.y_axis.extend(0.0),
            axes.z_axis.extend(0.0),
            (self.feet + self.body.y_axis * self.eye_height_m).extend(1.0),
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
    //! | The map loads with 2 consoles a team and antipodal spawns | `map_has_two_consoles_a_team_and_antipodal_spawns` |
    //! | A player stands on the street and falls outward onto it | `players_fall_outward_onto_the_street` |
    //! | A lift carries a player up onto the glass level, and they stand on the glass | `lifts_carry_players_onto_the_glass` |
    //! | A shot from the glass level reflects off the glass, at most the set number of times | `shots_reflect_off_the_glass_up_to_the_limit` |
    //! | A shot from the street stops at the glass | `street_shots_stop_at_the_glass` |
    //! | A shot from the glass level goes straight across and hits a player | `glass_shots_cross_the_sphere` |
    //! | Range ends a shot | `range_ends_shots` |

    use super::*;

    fn world() -> CityWorld {
        CityWorld::Builtin().expect("the shipped map loads")
    }

    #[test]
    fn map_has_two_consoles_a_team_and_antipodal_spawns() {
        let world: CityWorld = world();
        for team in 0..2 {
            assert_eq!(world.consoles.iter().filter(|c| c.team == team).count(), 2);
        }
        let blue: DVec3 = world.spawns.iter().find(|s| s.0 == 0).unwrap().1.w_axis.truncate();
        let red: DVec3 = world.spawns.iter().find(|s| s.0 == 1).unwrap().1.w_axis.truncate();
        assert!((blue + red).length() < 1e-4);
    }

    #[test]
    fn players_fall_outward_onto_the_street() {
        let world: CityWorld = world();
        let mut player: CityPlayer = world.make_player(0, 0);
        player.feet *= 0.95;
        for _ in 0..120 {
            world.move_player(&mut player, &CityInput::default(), 1.0 / 60.0);
        }
        assert!((player.feet.length() - world.street_radius_m).abs() < 1e-3);
        assert!(player.is_on_ground);
        assert!(player.get_up().dot(-player.feet.normalize()) > 0.999);
    }

    #[test]
    fn lifts_carry_players_onto_the_glass() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let hole: DVec3 = glass.holes[0].0;
        let mut player: CityPlayer = world.make_player(0, 0);
        player.feet = hole * world.street_radius_m;
        let ride = CityInput {
            is_jumping: true,
            speed_frac: 1.0,
            ..CityInput::default()
        };
        for _ in 0..240 {
            world.move_player(&mut player, &ride, 1.0 / 60.0);
            if player.feet.length() < glass.radius_m - 1.0 {
                break;
            }
        }
        assert_eq!(world.get_side(player.feet), Side::Glass);
        let step = CityInput {
            forward_frac: 1.0,
            speed_frac: 1.0,
            ..CityInput::default()
        };
        player.body = DMat3::from_cols(player.body.x_axis, player.body.y_axis, player.body.z_axis);
        for _ in 0..90 {
            world.move_player(&mut player, &step, 1.0 / 60.0);
        }
        for _ in 0..120 {
            world.move_player(&mut player, &CityInput::default(), 1.0 / 60.0);
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
            let shot: Shot = world.trace_shot(origin, direction, 500.0, max_bounce_count, &[], None);
            assert_eq!(shot.bounce_count, max_bounce_count);
            assert_eq!(shot.points.len() as u32, max_bounce_count + 2);
        }
        let shot: Shot = world.trace_shot(origin, direction, 500.0, 1, &[], None);
        let (a, b, c) = (shot.points[0], shot.points[1], shot.points[2]);
        let normal: DVec3 = b.normalize();
        let (incoming, outgoing) = ((b - a).normalize(), (c - b).normalize());
        assert!((incoming.dot(normal) + outgoing.dot(normal)).abs() < 1e-6);
    }

    #[test]
    fn street_shots_stop_at_the_glass() {
        let world: CityWorld = world();
        let glass: &Shell = world.get_glass().unwrap();
        let mut player: CityPlayer = world.make_player(0, 0);
        player.pitch_rad = 1.4;
        let eye: DMat4 = player.get_eye_frame();
        let shot: Shot = world.trace_shot(eye.w_axis.truncate(), -eye.z_axis.truncate(), 500.0, 2, &[], None);
        assert_eq!(shot.bounce_count, 0);
        assert!((shot.points[1].length() - glass.radius_m).abs() < 1e-6 || shot.points[1].length() > glass.radius_m);
    }

    #[test]
    fn glass_shots_cross_the_sphere() {
        let world: CityWorld = world();
        let glass_r: f64 = world.get_glass().unwrap().radius_m;
        let a: DVec3 = DVec3::new(0.0, 0.0, 1.0) * (glass_r - 1.6);
        let feet: DVec3 = DVec3::new(0.0, 1.0, -0.2).normalize() * glass_r;
        let target = Target { id: 7, feet, up: get_up(feet) };
        let aim: DVec3 = (feet + target.up * TORSO_HEIGHT_M - a).normalize();
        let shot: Shot = world.trace_shot(a, aim, 500.0, 2, &[target], Some(1));
        assert_eq!(shot.hit, ShotHit::Player { id: 7, is_head: false }, "{shot:?}");
    }

    #[test]
    fn range_ends_shots() {
        let world: CityWorld = world();
        let origin: DVec3 = DVec3::new(0.0, 0.0, 10.0);
        let shot: Shot = world.trace_shot(origin, DVec3::Y, 1.0, 2, &[], None);
        assert_eq!(shot.hit, ShotHit::Range);
        assert!((shot.points[1] - origin - DVec3::Y).length() < 1e-9);
    }
}
