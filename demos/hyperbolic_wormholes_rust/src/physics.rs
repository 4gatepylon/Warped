//! `physics.rs` moves the player in double precision: walking with gravity toward the ground
//! (jump, jetpack) or flying, along paths that bend near mouths like light and go through throats.
//! It also aims through a mouth and traces the laser, whose path is a light ray.
//!
//! Interface: `Player`, `Input`, `update_player`, `look_around`, `level_out`, `aim_through_mouth`,
//! `find_aimed_mouth`, `trace_laser`, `LaserReport`.
//!
//! Example: `let messages = update_player(&world, &mut player, &input, 1.0 / 60.0);`
//!
//! Must know:
//! - Limitations: you collide with the ground only; walls and other boxes don't stop you.
//! - Limitations: mouths bend your path like light but don't pull you otherwise.

use glam::{DMat4, DVec3, DVec4, Vec4};

use crate::geometry::{
    AXIS_W, AXIS_Y, UP_VECTOR, get_distance, get_local_direction, make_boost, make_boost_along,
    make_rotation_about, make_rotation_between, minkowski_dot, normalize_point, normalize_tangent,
    orthonormalize_frame,
};
use crate::render_cpu::{Scene, TraceEnd, trace_ray};
use crate::scene_data::{Beam, ROW_TEXEL_COUNT, SceneData, TEXEL_TO_WORLD};
use crate::world::{EYE_HEIGHT_M, Mouth, World};

pub const GRAVITY_M_PER_S2: f64 = 30.0;
const FALL_GRAVITY_FACTOR: f64 = 1.6;
pub const JUMP_SPEED_M_PER_S: f64 = 7.0;
const JETPACK_M_PER_S2: f64 = 34.0;
pub const WALK_SPEED_M_PER_S: f64 = 6.0;
pub const SPRINT_SPEED_M_PER_S: f64 = 14.0;
const AIR_CONTROL_PER_S: f64 = 4.0;
const ON_GROUND_TOLERANCE_M: f64 = 0.02;
const MAX_SUBSTEP_COUNT: usize = 400;
const MAX_STEP_M: f64 = 0.5;
const STEP_PER_NEAREST: f64 = 0.05;
const LOOK_RAD_PER_PX: f64 = 0.0025;
const MAX_PITCH_RAD: f64 = 1.55;
const LEVEL_OUT_RATE_PER_S: f64 = 2.5;
const LEVEL_OUT_MAX_UP_DOT: f64 = 0.98;
const AIM_MAX_ITERATION_COUNT: usize = 10;
const AIM_TOLERANCE_M: f64 = 1e-3;
const AIM_FINITE_DIFFERENCE: f64 = 1e-3;
const AIM_MAX_TURN: f64 = 0.2;
const AIM_MAX_STEP_COUNT: usize = 4000;
const AIM_MAX_TRAVEL_M: f64 = 120.0;
const AIM_GIVE_UP_PAST_CLOSEST_M: f64 = 1.0;
const SHADOW_PER_HORIZON: f64 = 2.598_076_211_353_316;
const HAND_OFFSET_M: DVec3 = DVec3::new(0.25, -0.22, -0.35);
const LASER_FOCUS_M: f64 = 10.0;
const LASER_MAX_M: f64 = 150.0;
const MIN_SPEED_M_PER_S: f64 = 1e-6;

/// `Player` is you: a camera frame (columns right, up, back, eye position), a velocity (a world
/// tangent vector at the eye, blocks per second) and whether you fly or stand on the ground.
#[derive(Clone)]
pub struct Player {
    pub frame: DMat4,
    pub velocity: DVec4,
    pub is_flying: bool,
    pub is_on_ground: bool,
}

/// `Input` is which movement keys are held this frame.
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

/// `LaserReport` describes the laser's path: how many wormholes it went through, what it hit and
/// how long it is.
#[derive(Clone, Default)]
pub struct LaserReport {
    pub wormhole_count: usize,
    pub hit: Option<String>,
    pub hit_box: Option<usize>,
    pub length_m: f64,
}

impl Player {
    /// `AtSpawn` makes a player standing at spawn, looking along the spawn heading.
    ///
    /// Example: `let player = Player::AtSpawn(&world);`
    #[allow(non_snake_case)]
    pub fn AtSpawn(world: &World) -> Player {
        Player {
            frame: world.spawn_frame * make_boost(AXIS_Y, EYE_HEIGHT_M / world.curvature_radius_m),
            velocity: DVec4::ZERO,
            is_flying: false,
            is_on_ground: true,
        }
    }

    /// `get_position` gives the eye's position.
    ///
    /// Example: `let x = player.get_position();`
    pub fn get_position(&self) -> DVec4 {
        self.frame.col(AXIS_W)
    }

    /// `get_height_m` gives the eye's height above the ground.
    ///
    /// Example: `let h = player.get_height_m(&world);`
    pub fn get_height_m(&self, world: &World) -> f64 {
        world.curvature_radius_m * self.get_position()[AXIS_Y].asinh()
    }
}

/// `get_light_bend` gives how fast a light ray at `x` moving along unit `v` turns (a tangent
/// vector, per block, at right angles to `v`), summed over mouths within the cutoff, and the
/// distance to the nearest mouth. It is the same law as the renderer's.
///
/// Example: `let (bend, nearest_m) = get_light_bend(&world, x, v);`
pub fn get_light_bend(world: &World, x: DVec4, v: DVec4) -> (DVec4, f64) {
    let r: f64 = world.curvature_radius_m;
    let (cutoff_m, throat_m) = (world.bend_cutoff_m, world.get_throat_m());
    let mut acceleration: DVec4 = DVec4::ZERO;
    let mut nearest_m: f64 = f64::INFINITY;
    for mouth in &world.mouths {
        let rho: f64 = get_distance(x, mouth.center);
        nearest_m = nearest_m.min(rho * r);
        if rho * r > cutoff_m || rho < 1e-12 {
            continue;
        }
        let gradient: f64 =
            get_dln_n_flat(world.get_effective_radius_m(rho), throat_m) * (2.0 * rho).exp();
        let radial: DVec4 = (rho.cosh() * x - mouth.center) / rho.sinh();
        acceleration += gradient * radial;
    }
    (acceleration - minkowski_dot(acceleration, v) * v, nearest_m)
}

/// `get_dln_n_flat` gives d ln n / dρ for the flat "black hole foil" wormhole.
///
/// Example: `let g = get_dln_n_flat(3.0, 0.35);`
fn get_dln_n_flat(rho_m: f64, throat_m: f64) -> f64 {
    const LAMBDA_SQUARED: f64 = 0.25;
    let psi: f64 = 1.0 + throat_m / rho_m;
    let r_m: f64 = rho_m * psi * psi;
    let rs_m: f64 = 4.0 * throat_m;
    let a: f64 = (1.0 - rs_m / r_m + LAMBDA_SQUARED) / (1.0 + LAMBDA_SQUARED);
    -2.0 * throat_m / (rho_m * rho_m * psi)
        - rs_m * psi * (1.0 - throat_m / rho_m) / (2.0 * r_m * r_m * (1.0 + LAMBDA_SQUARED) * a)
}

/// `get_space_stretch` gives how many blocks of proper length one block of the lensing
/// coordinates is at `x` (ψ²), so moving at a steady speed slows down near a mouth.
///
/// Example: `let stretch = get_space_stretch(&world, x);`
fn get_space_stretch(world: &World, x: DVec4) -> f64 {
    let throat_m: f64 = world.get_throat_m();
    let psi: f64 = 1.0
        + world
            .mouths
            .iter()
            .map(|m| throat_m / world.get_effective_radius_m(get_distance(x, m.center)))
            .sum::<f64>();
    psi * psi
}

/// `cross_throat_if_inside` moves a frame through a throat if its position is inside one, and
/// returns the mouth entered: the position is reflected through the throat sphere and carried to
/// the partner, and each axis keeps its sideways part and flips its part along the radius.
///
/// Example: `if let Some(mouth) = cross_throat_if_inside(&world, &mut frame) { say(mouth); }`
pub fn cross_throat_if_inside<'a>(world: &'a World, frame: &mut DMat4) -> Option<&'a Mouth> {
    let throat_m: f64 = world.get_throat_m();
    let x: DVec4 = frame.col(AXIS_W);
    for mouth in &world.mouths {
        let rho: f64 = get_distance(x, mouth.center);
        let rho_effective_m: f64 = world.get_effective_radius_m(rho);
        if rho_effective_m >= throat_m {
            continue;
        }
        let (c, s) = (rho.cosh(), rho.sinh());
        let radial: DVec4 = (c * x - mouth.center) / s;
        let outward: DVec4 = (x - c * mouth.center) / s;
        let new_rho: f64 =
            0.5 * (2.0 * throat_m * throat_m / rho_effective_m / world.curvature_radius_m).ln_1p();
        let (new_center, new_outward) = (mouth.gluing * mouth.center, mouth.gluing * outward);
        let new_x: DVec4 = new_rho.cosh() * new_center + new_rho.sinh() * new_outward;
        let new_radial: DVec4 = new_rho.sinh() * new_center + new_rho.cosh() * new_outward;
        let axes: [DVec4; 3] = [0, 1, 2].map(|i| {
            let axis: DVec4 = frame.col(i);
            let along: f64 = minkowski_dot(axis, radial);
            mouth.gluing * (axis - along * radial) - along * new_radial
        });
        *frame = orthonormalize_frame(DMat4::from_cols(axes[0], axes[1], axes[2], new_x));
        return Some(mouth);
    }
    None
}

/// `get_up_local` gives the direction straight up, away from the ground, in a frame's axes.
///
/// Example: `let up_local = get_up_local(player.frame);`
pub fn get_up_local(frame: DMat4) -> DVec3 {
    get_local_direction(frame, normalize_tangent(frame.col(AXIS_W), UP_VECTOR)).normalize()
}

/// `format_crossing` describes going through a mouth, for the HUD.
///
/// Example: `messages.push(format_crossing(&world, mouth));`
fn format_crossing(world: &World, mouth: &Mouth) -> String {
    format!(
        "Through wormhole {}: out of mouth {}",
        mouth.pair_index + 1,
        world.mouths[mouth.partner_index].name
    )
}

/// `update_player` moves the player for one frame and returns messages about wormholes crossed.
/// Flying moves along light rays with the view turning with the path; walking moves with gravity
/// toward the ground, a jump and a jetpack, along paths that bend like light near mouths.
///
/// Example: `let messages = update_player(&world, &mut player, &input, 1.0 / 60.0);`
pub fn update_player(world: &World, player: &mut Player, input: &Input, dt_s: f64) -> Vec<String> {
    let speed_m_per_s: f64 = if input.sprint {
        SPRINT_SPEED_M_PER_S
    } else {
        WALK_SPEED_M_PER_S
    };
    let up_local: DVec3 = get_up_local(player.frame);
    if player.is_flying {
        let mut direction: DVec3 = DVec3::ZERO;
        direction +=
            DVec3::NEG_Z * (f64::from(u8::from(input.forward)) - f64::from(u8::from(input.back)));
        direction +=
            DVec3::X * (f64::from(u8::from(input.right)) - f64::from(u8::from(input.left)));
        direction += up_local * (f64::from(u8::from(input.up)) - f64::from(u8::from(input.down)));
        player.velocity = DVec4::ZERO;
        if direction.length() < 1e-9 {
            return Vec::new();
        }
        return fly_along_light(world, player, direction.normalize(), speed_m_per_s * dt_s);
    }
    let x: DVec4 = player.get_position();
    let up: DVec4 = normalize_tangent(x, UP_VECTOR);
    let flatten = |w: DVec4| -> Option<DVec4> {
        let flat: DVec4 = w - minkowski_dot(w, up) * up;
        (minkowski_dot(flat, flat) > 1e-9).then(|| flat / minkowski_dot(flat, flat).sqrt())
    };
    let forward: DVec4 = flatten(-player.frame.col(2))
        .or_else(|| flatten(player.frame.col(1)))
        .unwrap_or(DVec4::ZERO);
    let right: DVec4 = flatten(player.frame.col(0)).unwrap_or(DVec4::ZERO);
    let mut wish: DVec4 = forward
        * (f64::from(u8::from(input.forward)) - f64::from(u8::from(input.back)))
        + right * (f64::from(u8::from(input.right)) - f64::from(u8::from(input.left)));
    if minkowski_dot(wish, wish) > 1e-9 {
        wish *= speed_m_per_s / minkowski_dot(wish, wish).sqrt();
    }
    let mut vertical_m_per_s: f64 = minkowski_dot(player.velocity, up);
    let horizontal: DVec4 = player.velocity - vertical_m_per_s * up;
    let new_horizontal: DVec4 = if player.is_on_ground {
        wish
    } else {
        horizontal + (wish - horizontal) * (AIR_CONTROL_PER_S * dt_s).min(1.0)
    };
    if input.up && player.is_on_ground {
        vertical_m_per_s = vertical_m_per_s.max(JUMP_SPEED_M_PER_S);
        player.is_on_ground = false;
    }
    player.velocity = new_horizontal + vertical_m_per_s * up;
    move_with_gravity(world, player, input.up, dt_s)
}

/// `move_with_gravity` integrates a walking player's motion over `dt_s` in short steps: gravity
/// (stronger on the way down) and the jetpack change the velocity, the path bends like light near
/// mouths, throats carry the player through, and the ground stops the fall.
///
/// Example: `let messages = move_with_gravity(&world, &mut player, false, 1.0 / 60.0);`
fn move_with_gravity(
    world: &World,
    player: &mut Player,
    is_jetpack_on: bool,
    dt_s: f64,
) -> Vec<String> {
    let r: f64 = world.curvature_radius_m;
    let mut messages: Vec<String> = Vec::new();
    let mut time_left_s: f64 = dt_s;
    for _ in 0..MAX_SUBSTEP_COUNT {
        if time_left_s <= 1e-9 {
            break;
        }
        let x: DVec4 = player.get_position();
        let up: DVec4 = normalize_tangent(x, UP_VECTOR);
        let vertical_m_per_s: f64 = minkowski_dot(player.velocity, up);
        let gravity_m_per_s2: f64 = GRAVITY_M_PER_S2
            * if vertical_m_per_s < 0.0 {
                FALL_GRAVITY_FACTOR
            } else {
                1.0
            };
        let thrust_m_per_s2: f64 = if is_jetpack_on && !player.is_on_ground {
            JETPACK_M_PER_S2
        } else {
            0.0
        };
        let speed_m_per_s: f64 = minkowski_dot(player.velocity, player.velocity)
            .max(0.0)
            .sqrt();
        if speed_m_per_s < MIN_SPEED_M_PER_S {
            if player.is_on_ground {
                break;
            }
            let step_s: f64 = time_left_s.min(MAX_STEP_M / GRAVITY_M_PER_S2);
            player.velocity += (thrust_m_per_s2 - gravity_m_per_s2) * step_s * up;
            time_left_s -= step_s;
            continue;
        }
        let direction: DVec4 = player.velocity / speed_m_per_s;
        let (bend, nearest_m) = get_light_bend(world, x, direction);
        let stretch: f64 = get_space_stretch(world, x);
        let step_m: f64 = (speed_m_per_s * time_left_s / stretch)
            .min(STEP_PER_NEAREST * nearest_m)
            .min(MAX_STEP_M);
        let step_s: f64 = step_m * stretch / speed_m_per_s;
        let local: DVec3 = get_local_direction(player.frame, direction).normalize();
        player.frame = orthonormalize_frame(player.frame * make_boost_along(local, step_m / r));
        let new_x: DVec4 = player.get_position();
        let new_up: DVec4 = normalize_tangent(new_x, UP_VECTOR);
        let transported: DVec4 = speed_m_per_s * (player.frame * local.extend(0.0));
        let bent: DVec4 = transported + speed_m_per_s * step_m * bend;
        turn_with_path(&mut player.frame, transported, bent);
        let velocity: DVec4 = bent + (thrust_m_per_s2 - gravity_m_per_s2) * step_s * new_up;
        player.velocity = velocity + minkowski_dot(velocity, new_x) * new_x;
        let velocity_local: DVec3 = get_local_direction(player.frame, player.velocity);
        if let Some(mouth) = cross_throat_if_inside(world, &mut player.frame) {
            player.velocity = player.frame * velocity_local.extend(0.0);
            messages.push(format_crossing(world, mouth));
        }
        stand_on_ground(world, player);
        time_left_s -= step_s;
    }
    messages
}

/// `turn_with_path` turns the view left or right by the angle the path's level direction turned
/// from `before` to `after` (world tangent vectors at the frame's position), so that walking keeps
/// heading where the bent path goes, as light does.
///
/// Example: `turn_with_path(&mut player.frame, transported, bent);`
fn turn_with_path(frame: &mut DMat4, before: DVec4, after: DVec4) {
    let up_local: DVec3 = get_up_local(*frame);
    let flatten = |w: DVec4| -> DVec3 {
        let local: DVec3 = get_local_direction(*frame, w);
        local - local.dot(up_local) * up_local
    };
    let (from, to) = (flatten(before), flatten(after));
    if from.length() < 1e-9 || to.length() < 1e-9 {
        return;
    }
    let angle_rad: f64 = from.cross(to).dot(up_local).atan2(from.dot(to));
    *frame *= make_rotation_about(up_local, angle_rad);
}

/// `stand_on_ground` keeps the eye at least `EYE_HEIGHT_M` above the ground: below it, the player
/// is lifted and stops falling.
///
/// Example: `stand_on_ground(&world, &mut player);`
fn stand_on_ground(world: &World, player: &mut Player) {
    let r: f64 = world.curvature_radius_m;
    let height_m: f64 = player.get_height_m(world);
    if height_m > EYE_HEIGHT_M + ON_GROUND_TOLERANCE_M {
        player.is_on_ground = false;
        return;
    }
    if height_m < EYE_HEIGHT_M {
        player.frame = orthonormalize_frame(
            player.frame
                * make_boost_along(get_up_local(player.frame), (EYE_HEIGHT_M - height_m) / r),
        );
    }
    let x: DVec4 = player.get_position();
    let up: DVec4 = normalize_tangent(x, UP_VECTOR);
    let vertical_m_per_s: f64 = minkowski_dot(player.velocity, up);
    if vertical_m_per_s < 0.0 {
        player.velocity -= vertical_m_per_s * up;
    }
    player.velocity += minkowski_dot(player.velocity, x) * x;
    player.is_on_ground = true;
}

/// `fly_along_light` moves the camera the way light moves: the path bends near mouths and goes
/// through throats, and the view turns with the path, so the crosshair shows where you go.
///
/// Example: `fly_along_light(&world, &mut player, DVec3::NEG_Z, 0.1);`
pub fn fly_along_light(
    world: &World,
    player: &mut Player,
    local_direction: DVec3,
    distance_m: f64,
) -> Vec<String> {
    let r: f64 = world.curvature_radius_m;
    let m: DVec3 = local_direction.normalize();
    let mut messages: Vec<String> = Vec::new();
    let mut left_m: f64 = distance_m;
    for _ in 0..MAX_SUBSTEP_COUNT {
        if left_m <= 1e-6 {
            break;
        }
        let x: DVec4 = player.get_position();
        let stretch: f64 = get_space_stretch(world, x);
        let v: DVec4 = player.frame * m.extend(0.0);
        let (first, nearest_m) = get_light_bend(world, x, v);
        let step_m: f64 = (left_m / stretch)
            .min(STEP_PER_NEAREST * nearest_m)
            .min(MAX_STEP_M);
        let midpoint_direction: DVec4 = normalize_tangent(x, v + 0.5 * step_m * first);
        player.frame = player.frame
            * make_rotation_between(
                m,
                get_local_direction(player.frame, midpoint_direction).normalize(),
            );
        let middle_frame: DMat4 = player.frame * make_boost_along(m, 0.5 * step_m / r);
        let (middle, _) = get_light_bend(
            world,
            middle_frame.col(AXIS_W),
            middle_frame * m.extend(0.0),
        );
        player.frame *= make_boost_along(m, step_m / r);
        let end_x: DVec4 = player.get_position();
        let end_direction: DVec4 = normalize_tangent(
            end_x,
            player.frame * m.extend(0.0) + step_m * middle - 0.5 * step_m * first,
        );
        player.frame = orthonormalize_frame(
            player.frame
                * make_rotation_between(
                    m,
                    get_local_direction(player.frame, end_direction).normalize(),
                ),
        );
        left_m -= step_m * stretch;
        if let Some(mouth) = cross_throat_if_inside(world, &mut player.frame) {
            messages.push(format_crossing(world, mouth));
        }
    }
    messages
}

/// `remove_roll` turns the camera about its view direction to remove a fraction of its roll,
/// unless it looks almost straight up or down.
///
/// Example: `remove_roll(&mut player.frame, 1.0);`
fn remove_roll(frame: &mut DMat4, fraction: f64) {
    let up_local: DVec3 = get_up_local(*frame);
    if up_local.z.abs() > LEVEL_OUT_MAX_UP_DOT {
        return;
    }
    let roll_rad: f64 = (-up_local.x).atan2(up_local.y);
    *frame *= make_rotation_about(DVec3::Z, roll_rad * fraction);
}

/// `level_out` removes the camera's roll a little at a time.
///
/// Example: `level_out(&mut player.frame, 1.0 / 60.0);`
pub fn level_out(frame: &mut DMat4, dt_s: f64) {
    remove_roll(frame, 1.0 - (-LEVEL_OUT_RATE_PER_S * dt_s).exp());
}

/// `look_around` turns the view left and right about the up direction and up and down about the
/// camera's right axis, never past straight up or down.
///
/// Example: `look_around(&mut player.frame, dx, dy);`
pub fn look_around(frame: &mut DMat4, dx_px: f64, dy_px: f64) {
    let up_local: DVec3 = get_up_local(*frame);
    *frame *= make_rotation_about(up_local, -dx_px * LOOK_RAD_PER_PX);
    let pitch_rad: f64 = (-up_local.z).clamp(-1.0, 1.0).asin();
    let new_pitch_rad: f64 =
        (pitch_rad - dy_px * LOOK_RAD_PER_PX).clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);
    *frame =
        orthonormalize_frame(*frame * make_rotation_about(DVec3::X, new_pitch_rad - pitch_rad));
}

/// `turn_toward` turns the view to look straight (along the geodesic, ignoring the bending) at
/// `target`: first about the up direction, then up or down, within the usual pitch limits.
///
/// Example: `physics::turn_toward(&mut player.frame, other.frame.col(AXIS_W));`
pub fn turn_toward(frame: &mut DMat4, target: DVec4) {
    let to_target = |frame: DMat4| -> DVec3 {
        get_local_direction(frame, normalize_tangent(frame.col(AXIS_W), target)).normalize()
    };
    let up_local: DVec3 = get_up_local(*frame);
    let forward: DVec3 = DVec3::NEG_Z - up_local * -up_local.z;
    let toward: DVec3 = to_target(*frame);
    let toward_level: DVec3 = toward - up_local * toward.dot(up_local);
    let yaw_rad: f64 = up_local
        .dot(forward.cross(toward_level))
        .atan2(forward.dot(toward_level));
    *frame *= make_rotation_about(up_local, yaw_rad);
    let up_local: DVec3 = get_up_local(*frame);
    let pitch_rad: f64 = (-up_local.z).clamp(-1.0, 1.0).asin();
    let wanted_pitch_rad: f64 = to_target(*frame)
        .dot(up_local)
        .clamp(-1.0, 1.0)
        .asin()
        .clamp(-MAX_PITCH_RAD, MAX_PITCH_RAD);
    *frame =
        orthonormalize_frame(*frame * make_rotation_about(DVec3::X, wanted_pitch_rad - pitch_rad));
}

/// `trace_miss` follows the light ray leaving the camera along a local direction and measures by
/// how much it misses a mouth's centre (in blocks, in the mouth's axes, at right angles to the
/// ray): where it crosses the throat if it falls in (interpolated, so the miss changes smoothly
/// with the aim), else at its closest approach.
///
/// Example: `let miss = trace_miss(&world, frame, DVec3::NEG_Z, &world.mouths[0]);`
fn trace_miss(world: &World, frame: DMat4, local_direction: DVec3, target: &Mouth) -> DVec3 {
    let r: f64 = world.curvature_radius_m;
    let throat_m: f64 = world.get_throat_m();
    let mut x: DVec4 = frame.col(AXIS_W);
    let mut v: DVec4 = frame * local_direction.normalize().extend(0.0);
    let (mut closest_m, mut closest_miss) = (f64::INFINITY, DVec3::ZERO);
    let mut previous: Option<(DVec3, f64)> = None;
    let mut travelled_m: f64 = 0.0;
    for _ in 0..AIM_MAX_STEP_COUNT {
        if travelled_m > AIM_MAX_TRAVEL_M {
            break;
        }
        let rho: f64 = get_distance(x, target.center);
        let excess_m: f64 = world.get_effective_radius_m(rho) - throat_m;
        let (local_x, local_v) = (
            (target.to_local * x).truncate(),
            (target.to_local * v).truncate(),
        );
        let miss: DVec3 = r * (local_x - local_x.dot(local_v) / local_v.dot(local_v) * local_v);
        if excess_m < 0.0 {
            return match previous {
                None => miss,
                Some((previous_miss, previous_excess_m)) => {
                    previous_miss
                        + previous_excess_m / (previous_excess_m - excess_m)
                            * (miss - previous_miss)
                }
            };
        }
        if rho * r < closest_m {
            closest_m = rho * r;
            closest_miss = miss;
        } else if rho * r > closest_m + AIM_GIVE_UP_PAST_CLOSEST_M {
            break;
        }
        previous = Some((miss, excess_m));
        let (first, nearest_m) = get_light_bend(world, x, v);
        let step_m: f64 = MAX_STEP_M.min(STEP_PER_NEAREST * nearest_m);
        let s: f64 = step_m / r;
        let vh: DVec4 = normalize_tangent(x, v + 0.5 * step_m * first);
        let (xm, vm) = (
            (0.5 * s).cosh() * x + (0.5 * s).sinh() * vh,
            (0.5 * s).sinh() * x + (0.5 * s).cosh() * vh,
        );
        let (middle, _) = get_light_bend(world, xm, vm);
        let xn: DVec4 = normalize_point(s.cosh() * x + s.sinh() * vh);
        let vn: DVec4 = s.sinh() * x + s.cosh() * vh;
        v = normalize_tangent(xn, vn + step_m * middle - 0.5 * step_m * first);
        x = xn;
        travelled_m += step_m;
    }
    closest_miss
}

/// `aim_through_mouth` turns the view so the light ray through the crosshair passes through the
/// exact middle of a mouth: straight at its centre with the horizon level, then Gauss-Newton on
/// the view direction, damped so it never turns far at once.
///
/// Example: `aim_through_mouth(&world, &mut player.frame, &world.mouths[0]);`
pub fn aim_through_mouth(world: &World, frame: &mut DMat4, target: &Mouth) {
    let x: DVec4 = frame.col(AXIS_W);
    let toward: DVec3 =
        get_local_direction(*frame, normalize_tangent(x, target.center)).normalize();
    *frame = orthonormalize_frame(*frame * make_rotation_between(DVec3::NEG_Z, toward));
    remove_roll(frame, 1.0);
    let (mut alpha, mut beta) = (0.0f64, 0.0f64);
    let direction_of = |a: f64, b: f64| DVec3::new(a, b, -1.0);
    for _ in 0..AIM_MAX_ITERATION_COUNT {
        let miss: DVec3 = trace_miss(world, *frame, direction_of(alpha, beta), target);
        if miss.length() < AIM_TOLERANCE_M {
            break;
        }
        let j_alpha: DVec3 = (trace_miss(
            world,
            *frame,
            direction_of(alpha + AIM_FINITE_DIFFERENCE, beta),
            target,
        ) - miss)
            / AIM_FINITE_DIFFERENCE;
        let j_beta: DVec3 = (trace_miss(
            world,
            *frame,
            direction_of(alpha, beta + AIM_FINITE_DIFFERENCE),
            target,
        ) - miss)
            / AIM_FINITE_DIFFERENCE;
        let (a11, a12, a22) = (
            j_alpha.dot(j_alpha),
            j_alpha.dot(j_beta),
            j_beta.dot(j_beta),
        );
        let (b1, b2) = (-j_alpha.dot(miss), -j_beta.dot(miss));
        let determinant: f64 = a11 * a22 - a12 * a12;
        if determinant.abs() < 1e-12 {
            break;
        }
        let (d_alpha, d_beta) = (
            (a22 * b1 - a12 * b2) / determinant,
            (a11 * b2 - a12 * b1) / determinant,
        );
        let damping: f64 = (AIM_MAX_TURN / d_alpha.hypot(d_beta)).min(1.0);
        alpha += damping * d_alpha;
        beta += damping * d_beta;
    }
    *frame = orthonormalize_frame(
        *frame * make_rotation_between(DVec3::NEG_Z, direction_of(alpha, beta).normalize()),
    );
}

/// `find_aimed_mouth` finds the mouth whose dark disk is closest to the crosshair: the angle to
/// its centre less the disk's apparent radius is smallest.
///
/// Example: `if let Some(k) = find_aimed_mouth(&world, player.frame) { aim(k); }`
pub fn find_aimed_mouth(world: &World, frame: DMat4) -> Option<usize> {
    let x: DVec4 = frame.col(AXIS_W);
    let mut best: Option<(usize, f64)> = None;
    for (k, mouth) in world.mouths.iter().enumerate() {
        let distance: f64 = get_distance(x, mouth.center);
        if distance < 1e-9 {
            continue;
        }
        let local: DVec3 =
            get_local_direction(frame, normalize_tangent(x, mouth.center)).normalize();
        let angle_rad: f64 = (-local.z).clamp(-1.0, 1.0).acos();
        let disk_rad: f64 = (SHADOW_PER_HORIZON * world.get_throat_m()
            / crate::world::THROAT_PER_HORIZON
            / (world.curvature_radius_m * distance.sinh()))
        .atan();
        if best.is_none_or(|(_, score)| angle_rad - disk_rad < score) {
            best = Some((k, angle_rad - disk_rad));
        }
    }
    best.map(|(k, _)| k)
}

/// `trace_laser` traces the laser from the hand toward the point `LASER_FOCUS_M` straight ahead of
/// the eye (in hyperbolic space a beam parallel to the line of sight would drift away from the
/// crosshair), as a light ray through the frame's packed scene (so it meets exactly what the
/// renderer draws), and returns its path in world coordinates and a report.
///
/// Example: `let (beam, report) = trace_laser(&world, &data);`
pub fn trace_laser(world: &World, data: &SceneData) -> (Beam, LaserReport) {
    let r: f64 = world.curvature_radius_m;
    let scene: Scene = Scene::New(data);
    let hand_distance: f64 = HAND_OFFSET_M.length() / r;
    let hand: DVec4 =
        (HAND_OFFSET_M.normalize() * hand_distance.sinh()).extend(hand_distance.cosh());
    let focus_distance: f64 = LASER_FOCUS_M / r;
    let focus: DVec4 = DVec4::new(0.0, 0.0, -focus_distance.sinh(), focus_distance.cosh());
    let direction: DVec4 = normalize_tangent(hand, focus);
    let to_world = |row: usize, point: Vec4| -> DVec4 {
        let first: usize = row * ROW_TEXEL_COUNT + TEXEL_TO_WORLD;
        let m: DMat4 = DMat4::from_cols(
            data.rows[first].as_dvec4(),
            data.rows[first + 1].as_dvec4(),
            data.rows[first + 2].as_dvec4(),
            data.rows[first + 3].as_dvec4(),
        );
        m * point.as_dvec4()
    };
    let mut beam: Beam = Beam::default();
    let mut current_row: usize = usize::MAX;
    let mut report: LaserReport = LaserReport::default();
    let end: TraceEnd = trace_ray(
        &scene,
        hand.as_vec4(),
        direction.as_vec4(),
        |row, a, b, t0_m, t1_m| {
            if f64::from(t0_m) > LASER_MAX_M {
                return;
            }
            let a_world: DVec4 = normalize_point(to_world(row, a));
            let b_world: DVec4 = if b.w.abs() > 0.0 && (-crate::render_cpu::mdot(b, b)).abs() > 1e-6
            {
                normalize_point(to_world(row, b))
            } else {
                let tangent: DVec4 = normalize_tangent(a_world, to_world(row, b) - a_world);
                let s: f64 = (LASER_MAX_M - f64::from(t0_m)).max(0.0) / r;
                s.cosh() * a_world + s.sinh() * tangent
            };
            let t1: f64 = f64::from(t1_m).min(LASER_MAX_M);
            if row != current_row {
                if current_row != usize::MAX {
                    report.wormhole_count += 1;
                }
                beam.runs.push(vec![(a_world, f64::from(t0_m))]);
                current_row = row;
            }
            beam.runs
                .last_mut()
                .expect("trace_laser: a run was just started")
                .push((b_world, t1));
            report.length_m = t1;
        },
    );
    if let TraceEnd::Surface {
        row, index, point, ..
    } = end
    {
        beam.spot = Some(normalize_point(to_world(row, point)));
        report.hit_box = usize::try_from(index).ok();
        report.hit = Some(if index < 0 {
            "the ground".to_string()
        } else {
            describe_box(world, index as usize)
        });
    }
    (beam, report)
}

/// `describe_box` names what a box is part of, for the laser report.
///
/// Example: `let name = describe_box(&world, 3);`
pub fn describe_box(world: &World, index: usize) -> String {
    world
        .clusters
        .iter()
        .find(|c| index >= c.first_box && index < c.first_box + c.box_count)
        .expect("describe_box: every box is in a cluster")
        .name
        .clone()
}

#[cfg(test)]
mod tests {
    //! Tests of moving through the starting wormholes.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | How you move | walking, flying | `walking_into_each_entrance_goes_through`, `flying_into_each_entrance_goes_through` |
    //! | Which entrance | near (1A), behind (2A), far left (3A) | both tests above, which try all three |

    use super::*;

    const DT_S: f64 = 1.0 / 60.0;
    const MAX_S: f64 = 10.0;

    /// `move_into` aims at a mouth from spawn and holds forward until a crossing, returning the
    /// crossing message (or the closest approach in blocks, for the failure message).
    fn move_into(world: &World, name: &str, is_flying: bool) -> std::result::Result<String, f64> {
        let mut player: Player = Player::AtSpawn(world);
        player.is_flying = is_flying;
        let target: &Mouth = world
            .mouths
            .iter()
            .find(|m| m.name == name)
            .expect("the mouth exists");
        aim_through_mouth(world, &mut player.frame, target);
        let input: Input = Input {
            forward: true,
            ..Input::default()
        };
        let mut closest_m: f64 = f64::INFINITY;
        let mut time_s: f64 = 0.0;
        while time_s < MAX_S {
            let messages: Vec<String> = update_player(world, &mut player, &input, DT_S);
            level_out(&mut player.frame, DT_S);
            closest_m = closest_m
                .min(get_distance(player.get_position(), target.center) * world.curvature_radius_m);
            if let Some(message) = messages.first() {
                return Ok(message.clone());
            }
            time_s += DT_S;
        }
        Err(closest_m)
    }

    /// `walking_into_each_entrance_goes_through` walks from spawn into 1A, 2A and 3A.
    #[test]
    fn walking_into_each_entrance_goes_through() {
        let world: World = World::Build(crate::world::DEFAULT_CURVATURE_RADIUS_M);
        for (name, wormhole) in [("1A", 1), ("2A", 2), ("3A", 3)] {
            let result = move_into(&world, name, false);
            assert!(
                result
                    .as_ref()
                    .is_ok_and(|m| m.starts_with(&format!("Through wormhole {wormhole}:"))),
                "walking into {name}: {result:?}"
            );
        }
    }

    /// `flying_into_each_entrance_goes_through` flies from spawn into 1A, 2A and 3A.
    #[test]
    fn flying_into_each_entrance_goes_through() {
        let world: World = World::Build(crate::world::DEFAULT_CURVATURE_RADIUS_M);
        for (name, wormhole) in [("1A", 1), ("2A", 2), ("3A", 3)] {
            let result = move_into(&world, name, true);
            assert!(
                result
                    .as_ref()
                    .is_ok_and(|m| m.starts_with(&format!("Through wormhole {wormhole}:"))),
                "flying into {name}: {result:?}"
            );
        }
    }
}
