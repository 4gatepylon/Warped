//! `render_cpu.rs` ray-traces the scene on the CPU, one pixel per ray, in 32-bit floats, spread
//! over every core with rayon. It is a port of the WebGL demo's fragment shader, plus the laser
//! beam: each straight piece of a ray is tested against the beam's pieces, in Klein coordinates.
//!
//! Interface: `Scene`, `TraceEnd`, `trace_ray`, `render`.
//!
//! Example: `render_cpu::render(&data, &mut pixels);`
//!
//! Must know:
//! - Gotchas: positions are 4-vectors on the hyperboloid; `mdot` is the Minkowski product.
//! - Performance: far from mouths a ray is one geodesic tested exactly; near a mouth it takes
//!   short midpoint steps, which is where most of the time goes.

use glam::{Mat3, Mat4, Vec2, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
use rayon::prelude::*;

use crate::scene_data::{
    ROW_TEXEL_COUNT, SceneData, TEXEL_BEAM_CHUNKS, TEXEL_BEAM_POINTS, TEXEL_BOXES, TEXEL_CLUSTERS,
    TEXEL_GLUINGS, TEXEL_GROUND, TEXEL_MOUTHS, TEXEL_SPOT, TEXEL_SUN, TEXEL_TO_WORLD,
    TEXELS_PER_BEAM_CHUNK, TEXELS_PER_BOX, TEXELS_PER_CLUSTER, TEXELS_PER_GLUING, Uniforms,
};
use crate::world::{MATERIAL_GLASS, MATERIAL_GLOW, MAX_MOUTH_COUNT};

const LAMBDA_SQUARED: f32 = 0.25;
const MAX_TRAVEL_PER_R: f32 = 24.0;
const MAX_STEP_COUNT: usize = 700;
const MAX_FOLD_COUNT: usize = 64;
const NEAR_STEP_PER_NEAREST: f32 = 0.12;
const FAR_STEP_PER_NEAREST: f32 = 0.25;
const NEAR_ZONE_PER_THROAT: f32 = 10.0;
const MIN_STEP_M: f32 = 0.005;
const CUTOFF_ENTRY_SHRINK: f32 = 0.98;
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
const SKY_HORIZON_COLOR: Vec3 = Vec3::new(0.78, 0.86, 0.95);
const HAZE_COLOR: Vec3 = SKY_HORIZON_COLOR;
const SKY_ZENITH_COLOR: Vec3 = Vec3::new(0.30, 0.52, 0.88);
const CLOUD_COLOR: Vec3 = Vec3::ONE;
const SUN_COLOR: Vec3 = Vec3::new(1.0, 0.92, 0.75);
const LOST_COLOR: Vec3 = Vec3::new(0.02, 0.02, 0.03);
const ROAD_COLOR: Vec3 = Vec3::new(0.58, 0.58, 0.60);
const DIRT_COLOR: Vec3 = Vec3::new(0.55, 0.42, 0.27);
const GRASS_DARK_COLOR: Vec3 = Vec3::new(0.36, 0.60, 0.25);
const GRASS_LIGHT_COLOR: Vec3 = Vec3::new(0.40, 0.65, 0.28);
const GLASS_SHEEN_COLOR: Vec3 = Vec3::splat(0.95);
const LASER_HALO_COLOR: Vec3 = Vec3::new(1.0, 0.06, 0.03);
const LASER_CORE_COLOR: Vec3 = Vec3::new(1.0, 0.62, 0.52);
const DIRT_FRACTION: f32 = 0.04;
const TEXTURE_CELLS_PER_BLOCK: f32 = 8.0;
const TEXTURE_MIN: f32 = 0.86;
const TEXTURE_RANGE: f32 = 0.22;
const EDGE_DARKEST: f32 = 0.55;
const EDGE_WIDTH: f32 = 0.045;
const AMBIENT_LIGHT: f32 = 0.68;
const SUN_LIGHT: f32 = 0.32;
const TOP_SHADE: f32 = 1.0;
const BOTTOM_SHADE: f32 = 0.5;
const X_SIDE_SHADE: f32 = 0.82;
const Z_SIDE_SHADE: f32 = 0.68;
const GLASS_SHEEN: f32 = 0.2;
const GLOW_BRIGHTNESS: f32 = 1.25;
const SUN_DISK_INNER_COSINE: f32 = 0.99955;
const SUN_DISK_OUTER_COSINE: f32 = 0.9998;
const SUN_DISK_BRIGHTNESS: f32 = 3.0;
const SUN_GLOW_POWER: f32 = 300.0;
const SUN_GLOW_BRIGHTNESS: f32 = 0.4;
const SKY_GRADIENT_POWER: f32 = 0.55;
const CLOUD_SCALE: f32 = 1.6;
const CLOUD_DRIFT_PER_S: f32 = 0.01;
const HIGHLIGHT_ROLLOFF: f32 = 0.6;
const BEAM_RADIUS_M: f32 = 0.012;
const BEAM_REACH: f32 = 4.8;
const BEAM_FADE_M: f32 = 260.0;
const SPOT_RADIUS_M: f32 = 0.07;
const SPOT_PIXELS: f32 = 2.5;
const NO_HIT: i32 = -2;
const GROUND_HIT: i32 = -1;
const TILE_SIDE_COUNT: f32 = 4.0;
const TILES_PER_VERTEX: f32 = 5.0;
const BYTES_PER_PIXEL: usize = 4;

/// `Scene` is one frame's packed scene, with the uniforms unpacked into vector types.
pub struct Scene<'a> {
    rows: &'a [Vec4],
    u: &'a Uniforms,
    world_to_camera: Mat4,
    sky_basis: Mat3,
    sun_ideal_world: Vec4,
    tile_edges: [Vec4; 4],
    tile_klein_half_width: f32,
    q_cutoff: f32,
}

/// `Reach` is cosh and sinh of how far a straight piece of a ray reaches, in curvature radii, so a
/// box can be ruled out without an `asinh`: a point is farther than `reach` past a slab of
/// half-thickness h exactly when |⟨x, N⟩| > sinh(h/R + reach).
#[derive(Clone, Copy)]
struct Reach {
    cosh: f32,
    sinh: f32,
}

/// `Hit` is the nearest surface found so far along one straight piece of a ray: `tau` is tanh of
/// its distance in curvature radii, `index` a box index, `GROUND_HIT` or `NO_HIT`.
struct Hit {
    tau: f32,
    index: i32,
    face: usize,
    side: f32,
}

/// `TraceEnd` is how a ray ended: on a surface (in a row's coordinates), in the sky (at a point at
/// infinity, as a light-like 4-vector), or lost after too many steps.
pub enum TraceEnd {
    Surface {
        row: usize,
        index: i32,
        face: usize,
        side: f32,
        point: Vec4,
        travelled_m: f32,
    },
    Sky {
        row: usize,
        ideal: Vec4,
    },
    Lost,
}

/// `Glow` is how strongly the laser's core and halo show in one pixel.
#[derive(Default)]
struct Glow {
    core: f32,
    halo: f32,
}

/// `mdot` computes the Minkowski product ax bx + ay by + az bz - aw bw.
///
/// Example: `let cosh_distance = -mdot(x, c);`
pub fn mdot(a: Vec4, b: Vec4) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z - a.w * b.w
}

/// `to_tangent` projects a vector onto the tangent space at point `x` and scales it to unit length.
///
/// Example: `let up = to_tangent(x, ground);`
fn to_tangent(x: Vec4, w: Vec4) -> Vec4 {
    let t: Vec4 = w + mdot(w, x) * x;
    t / mdot(t, t).max(1e-30).sqrt()
}

/// `to_point` puts a 4-vector back on the hyperboloid.
///
/// Example: `x = to_point(x);`
fn to_point(x: Vec4) -> Vec4 {
    x / (-mdot(x, x)).max(1e-30).sqrt()
}

/// `advance_geodesic` moves a point and its unit direction `s` curvature radii along their
/// geodesic.
///
/// Example: `advance_geodesic(&mut x, &mut v, ds_m / r);`
fn advance_geodesic(x: &mut Vec4, v: &mut Vec4, s: f32) {
    let (c, sh) = (s.cosh(), s.sinh());
    let moved: Vec4 = c * *x + sh * *v;
    *v = sh * *x + c * *v;
    *x = moved;
}

/// `get_q` gives q = 4 sinh²(d/2) for the distance d between two points, from their difference,
/// which stays accurate when they are close. It grows with d, so comparing q against a threshold
/// compares distances without an `asinh`.
///
/// Example: `let q = get_q(x, c);`
#[inline]
fn get_q(a: Vec4, b: Vec4) -> f32 {
    let d: Vec4 = a - b;
    mdot(d, d).max(0.0)
}

/// `get_q_of_distance` gives q = 4 sinh²(d/2) for a distance d in curvature radii.
///
/// Example: `let q_cutoff = get_q_of_distance(2.0);`
fn get_q_of_distance(d: f32) -> f32 {
    let half: f32 = 2.0 * (0.5 * d).sinh();
    half * half
}

/// `get_distance_of_q` gives the distance in curvature radii for q = 4 sinh²(d/2).
///
/// Example: `let d = get_distance_of_q(q);`
#[inline]
fn get_distance_of_q(q: f32) -> f32 {
    2.0 * (0.5 * q.sqrt()).asinh()
}

/// `get_dln_n_flat` gives d ln n / dρ for the flat "black hole foil" wormhole (Damour &
/// Solodukhin 2007), n = (1 + ρ_t/ρ)² / sqrt(A), in isotropic coordinates.
///
/// Example: `let g = get_dln_n_flat(3.0, 0.35);`
fn get_dln_n_flat(rho_m: f32, throat_m: f32) -> f32 {
    let psi: f32 = 1.0 + throat_m / rho_m;
    let r_m: f32 = rho_m * psi * psi;
    let rs_m: f32 = 4.0 * throat_m;
    let a: f32 = (1.0 - rs_m / r_m + LAMBDA_SQUARED) / (1.0 + LAMBDA_SQUARED);
    -2.0 * throat_m / (rho_m * rho_m * psi)
        - rs_m * psi * (1.0 - throat_m / rho_m) / (2.0 * r_m * r_m * (1.0 + LAMBDA_SQUARED) * a)
}

/// `hash_3` gives a pseudo-random number in [0, 1) for a 3-vector.
///
/// Example: `let r = hash_3(Vec3::new(1.0, 2.0, 3.0));`
fn hash_3(p: Vec3) -> f32 {
    (p.dot(Vec3::new(12.9898, 78.233, 37.719)).sin() * 43758.547)
        .fract()
        .abs()
}

/// `hash_2` gives a pseudo-random number in [0, 1) for a 2-vector.
///
/// Example: `let r = hash_2(Vec2::new(1.0, 2.0));`
fn hash_2(p: Vec2) -> f32 {
    (p.dot(Vec2::new(127.1, 311.7)).sin() * 43758.547)
        .fract()
        .abs()
}

/// `get_value_noise` gives smooth 2D value noise in [0, 1).
///
/// Example: `let n = get_value_noise(uv * 4.0);`
fn get_value_noise(p: Vec2) -> f32 {
    let i: Vec2 = p.floor();
    let mut f: Vec2 = p - i;
    f = f * f * (3.0 - 2.0 * f);
    let bottom: f32 = hash_2(i) + (hash_2(i + Vec2::X) - hash_2(i)) * f.x;
    let top: f32 = hash_2(i + Vec2::Y) + (hash_2(i + Vec2::ONE) - hash_2(i + Vec2::Y)) * f.x;
    bottom + (top - bottom) * f.y
}

/// `get_fractal_noise` sums five octaves of value noise, for clouds.
///
/// Example: `let cloud = get_fractal_noise(uv);`
fn get_fractal_noise(mut p: Vec2) -> f32 {
    let (mut sum, mut amplitude) = (0.0, 0.5);
    for _ in 0..5 {
        sum += amplitude * get_value_noise(p);
        p *= 2.03;
        amplitude *= 0.5;
    }
    sum
}

/// `smoothstep` is GLSL's smoothstep.
///
/// Example: `let t = smoothstep(0.0, 1.0, x);`
fn smoothstep(edge_0: f32, edge_1: f32, x: f32) -> f32 {
    let t: f32 = ((x - edge_0) / (edge_1 - edge_0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `get_block_texture` gives a block face's brightness pattern: speckles in an 8 × 8 grid and dark
/// block edges.
///
/// Example: `base *= get_block_texture(uv, 3.0);`
fn get_block_texture(uv: Vec2, seed: f32) -> f32 {
    let f: Vec2 = uv - uv.floor();
    let cell: Vec2 = (f * TEXTURE_CELLS_PER_BLOCK).floor() + uv.floor() * 17.0;
    let speckle: f32 = TEXTURE_MIN + TEXTURE_RANGE * hash_3(cell.extend(seed));
    let edge: Vec2 = f.min(Vec2::ONE - f);
    speckle
        * (EDGE_DARKEST + (1.0 - EDGE_DARKEST) * smoothstep(0.0, EDGE_WIDTH, edge.x.min(edge.y)))
}

/// `distance_point_segment` gives the distance from a point to a segment.
///
/// Example: `let d = distance_point_segment(p, a, b);`
fn distance_point_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab: Vec3 = b - a;
    let t: f32 = ((p - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0);
    (p - a - ab * t).length()
}

/// `get_segment_distance_squared` gives the squared distance between segments p1-q1 and p2-q2, and
/// where along p1-q1 (0 to 1) it is closest.
///
/// Example: `let (d2, s) = get_segment_distance_squared(a, b, p, q);`
fn get_segment_distance_squared(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f32, f32) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (mut s, mut t): (f32, f32);
    if e < 1e-10 {
        t = 0.0;
        s = if a > 1e-10 {
            (-d1.dot(r) / a).clamp(0.0, 1.0)
        } else {
            0.0
        };
    } else if a < 1e-10 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let (b, c) = (d1.dot(d2), d1.dot(r));
        let denominator: f32 = a * e - b * b;
        s = if denominator > 1e-7 * a * e {
            ((b * f - c * e) / denominator).clamp(0.0, 1.0)
        } else {
            0.0
        };
        t = (b * s + f) / e;
        if t < 0.0 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else if t > 1.0 {
            t = 1.0;
            s = ((b - c) / a).clamp(0.0, 1.0);
        }
    }
    let w: Vec3 = p1 + d1 * s - p2 - d2 * t;
    (w.dot(w), s)
}

impl<'a> Scene<'a> {
    /// `New` unpacks one frame's scene data for tracing.
    ///
    /// Example: `let scene = Scene::New(&data);`
    #[allow(non_snake_case)]
    pub fn New(data: &'a SceneData) -> Scene<'a> {
        let u: &Uniforms = &data.uniforms;
        let inradius: f32 = ((std::f32::consts::PI / TILES_PER_VERTEX).cos()
            / (std::f32::consts::PI / TILE_SIDE_COUNT).sin())
        .acosh();
        let (c, s) = (inradius.cosh(), inradius.sinh());
        let basis: [Vec3; 3] = u.sky_basis.map(|row| Vec4::from_array(row).truncate());
        Scene {
            rows: &data.rows,
            u,
            world_to_camera: Mat4::from_cols_array_2d(&u.world_to_camera),
            sky_basis: Mat3::from_cols(basis[0], basis[1], basis[2]).transpose(),
            sun_ideal_world: Vec4::from_array(u.sun_ideal_world),
            tile_edges: [
                Vec4::new(c, 0.0, 0.0, s),
                Vec4::new(-c, 0.0, 0.0, s),
                Vec4::new(0.0, 0.0, c, s),
                Vec4::new(0.0, 0.0, -c, s),
            ],
            tile_klein_half_width: inradius.tanh(),
            q_cutoff: get_q_of_distance(u.bend_cutoff_m / u.curvature_radius_m),
        }
    }

    /// `fetch` reads one texel of a row.
    ///
    /// Example: `let ground = scene.fetch(row, TEXEL_GROUND);`
    #[inline]
    fn fetch(&self, row: usize, texel: usize) -> Vec4 {
        self.rows[row * ROW_TEXEL_COUNT + texel]
    }

    /// `fetch_matrix` reads four texels of a row as a matrix's columns.
    ///
    /// Example: `let to_world = scene.fetch_matrix(row, TEXEL_TO_WORLD);`
    fn fetch_matrix(&self, row: usize, first_texel: usize) -> Mat4 {
        Mat4::from_cols(
            self.fetch(row, first_texel),
            self.fetch(row, first_texel + 1),
            self.fetch(row, first_texel + 2),
            self.fetch(row, first_texel + 3),
        )
    }

    /// `get_bend` gives how fast a ray at `x` moving along `v` turns (per block, at right angles to
    /// `v`), summed over mouths within the cutoff, and q of the nearest mouth (see `get_q`). Each
    /// mouth uses the flat law at the effective radius R e^ρ sinh ρ.
    ///
    /// Example: `let (bend, q_nearest) = scene.get_bend(row, x, v);`
    fn get_bend(&self, row: usize, x: Vec4, v: Vec4) -> (Vec4, f32) {
        let r: f32 = self.u.curvature_radius_m;
        let mut g: Vec4 = Vec4::ZERO;
        let mut q_nearest: f32 = f32::MAX;
        for k in 0..self.u.mouth_count as usize {
            let c: Vec4 = self.fetch(row, TEXEL_MOUTHS + k);
            let q: f32 = get_q(x, c);
            q_nearest = q_nearest.min(q);
            if q > self.q_cutoff || q < 1e-12 {
                continue;
            }
            let (cosh_rho, sinh_rho) = (1.0 + 0.5 * q, (q * (1.0 + 0.25 * q)).sqrt());
            let e_rho: f32 = cosh_rho + sinh_rho;
            g += get_dln_n_flat(r * sinh_rho * e_rho, self.u.throat_m)
                * e_rho
                * e_rho
                * (cosh_rho * x - c)
                / sinh_rho;
        }
        (g - mdot(g, v) * v, q_nearest)
    }

    /// `get_cutoff_entry_tau` finds where a geodesic first comes within (slightly less than) the
    /// bending cutoff of any mouth, as tanh of the distance, or 1.0 if it never does.
    ///
    /// Example: `let tau_max = scene.get_cutoff_entry_tau(row, x, v);`
    fn get_cutoff_entry_tau(&self, row: usize, x: Vec4, v: Vec4) -> f32 {
        let k_cosh: f32 =
            (CUTOFF_ENTRY_SHRINK * self.u.bend_cutoff_m / self.u.curvature_radius_m).cosh();
        let mut best: f32 = 1.0;
        for k in 0..self.u.mouth_count as usize {
            let c: Vec4 = self.fetch(row, TEXEL_MOUTHS + k);
            let (a, b) = (-mdot(x, c), -mdot(v, c));
            let discriminant: f32 = k_cosh * k_cosh + b * b - a * a;
            if b >= 0.0 || discriminant < 0.0 {
                continue;
            }
            let tau: f32 = (-a * b - k_cosh * discriminant.sqrt()) / (b * b + k_cosh * k_cosh);
            if tau > 0.0 {
                best = best.min(tau);
            }
        }
        best
    }

    /// `cross_throat` moves a ray through a throat if it is inside one: reflected through the
    /// throat sphere and carried into the partner mouth's row, where that mouth is at the origin.
    ///
    /// Example: `if scene.cross_throat(&mut row, &mut x, &mut v) { clear_until_m = -1.0; }`
    fn cross_throat(&self, row: &mut usize, x: &mut Vec4, v: &mut Vec4) -> bool {
        let r: f32 = self.u.curvature_radius_m;
        for k in 0..self.u.mouth_count as usize {
            let c: Vec4 = self.fetch(*row, TEXEL_MOUTHS + k);
            let q: f32 = get_q(*x, c);
            let (sinh_rho, cosh_rho) = ((q * (1.0 + 0.25 * q)).sqrt(), 1.0 + 0.5 * q);
            let rho_effective_m: f32 = r * sinh_rho * (cosh_rho + sinh_rho);
            if rho_effective_m >= self.u.throat_m {
                continue;
            }
            let outward: Vec4 = (*x - cosh_rho * c) / sinh_rho;
            let radial: Vec4 = (cosh_rho * *x - c) / sinh_rho;
            let along: f32 = mdot(*v, radial);
            let gluing: Mat4 = self.fetch_matrix(*row, TEXEL_GLUINGS + TEXELS_PER_GLUING * k);
            let new_rho: f32 =
                0.5 * (1.0 + 2.0 * self.u.throat_m * self.u.throat_m / rho_effective_m / r).ln();
            let (new_outward, sideways) = (gluing * outward, gluing * (*v - along * radial));
            let (new_cosh, new_sinh) = (new_rho.cosh(), new_rho.sinh());
            *x = to_point(new_cosh * Vec4::W + new_sinh * new_outward);
            *v = to_tangent(
                *x,
                sideways - along * (new_sinh * Vec4::W + new_cosh * new_outward),
            );
            *row = self.u.next_rows[k] as usize;
            return true;
        }
        false
    }

    /// `test_box` finds where a geodesic first enters a box within `reach`, lowering `hit` if
    /// nearer. Along x cosh s + v sinh s, with τ = tanh s, each slab |⟨x, N⟩| ≤ σ is a quadratic
    /// inequality in τ, so the box is an interval of τ.
    ///
    /// Example: `let lower_m = scene.test_box(row, b, x, v, ds_m, &mut hit);`
    fn test_box(&self, row: usize, b: usize, x: Vec4, v: Vec4, reach: Reach, hit: &mut Hit) -> f32 {
        let r: f32 = self.u.curvature_radius_m;
        let first: usize = TEXEL_BOXES + TEXELS_PER_BOX * b;
        let sigma: Vec4 = self.fetch(row, first + 3);
        let normals: [Vec4; 3] = [
            self.fetch(row, first),
            self.fetch(row, first + 1),
            self.fetch(row, first + 2),
        ];
        let a: Vec3 = Vec3::new(
            mdot(x, normals[0]),
            mdot(x, normals[1]),
            mdot(x, normals[2]),
        );
        for i in 0..3 {
            let sinh_limit: f32 =
                sigma[i] * reach.cosh + (1.0 + sigma[i] * sigma[i]).sqrt() * reach.sinh;
            if a[i].abs() > sinh_limit {
                return r * a[i].abs().asinh() - self.fetch(row, first + 4)[i];
            }
        }
        let half_m: Vec4 = self.fetch(row, first + 4);
        let lower_m: f32 = (Vec3::new(a.x.abs().asinh(), a.y.abs().asinh(), a.z.abs().asinh()) * r
            - half_m.truncate())
        .max_element();
        let (mut tau_in, mut tau_out, mut side, mut face) = (-1.0f32, 1.0f32, 1.0f32, 0usize);
        for i in 0..3 {
            let (ai, bi, s) = (a[i], mdot(v, normals[i]), sigma[i]);
            let discriminant: f32 = s * s + bi * bi - ai * ai;
            if discriminant < 0.0 {
                return lower_m;
            }
            let (root, denominator) = (s * discriminant.sqrt(), bi * bi + s * s);
            let (tau_0, tau_1) = (
                (-ai * bi - root) / denominator,
                (-ai * bi + root) / denominator,
            );
            if tau_0 > tau_in {
                tau_in = tau_0;
                face = i;
                side = if ai + bi * tau_0 >= 0.0 { 1.0 } else { -1.0 };
            }
            tau_out = tau_out.min(tau_1);
        }
        if tau_in <= tau_out && tau_in >= 0.0 && tau_in < hit.tau {
            *hit = Hit {
                tau: tau_in,
                index: b as i32,
                face,
                side,
            };
        }
        lower_m
    }

    /// `test_cluster` tests a geodesic piece against one cluster's boxes, skipping them all if the
    /// cluster's sphere is out of reach (a one-box cluster uses the box's own tighter bound), and
    /// returns a lower bound on the distance to the cluster.
    ///
    /// Example: `let lower_m = scene.test_cluster(row, k, x, vh, ds_m, &mut hit);`
    fn test_cluster(
        &self,
        row: usize,
        k: usize,
        x: Vec4,
        v: Vec4,
        reach_m: f32,
        hit: &mut Hit,
    ) -> f32 {
        let info: Vec4 = self.fetch(row, TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k + 1);
        let (first_box, box_count) = (info.y as usize, info.z as usize);
        if box_count > 1 {
            let distance: f32 = get_distance_of_q(get_q(
                x,
                self.fetch(row, TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k),
            ));
            let sphere_m: f32 = distance * self.u.curvature_radius_m - info.x;
            if sphere_m > reach_m {
                return sphere_m;
            }
        }
        let reach: Reach = Reach {
            cosh: (reach_m / self.u.curvature_radius_m).cosh(),
            sinh: (reach_m / self.u.curvature_radius_m).sinh(),
        };
        (first_box..first_box + box_count)
            .map(|b| self.test_box(row, b, x, v, reach, hit))
            .fold(1e9, f32::min)
    }

    /// `test_boxes` tests a geodesic piece against every cluster and returns a lower bound on the
    /// distance to every box, so the ray can skip box tests until it has travelled that far.
    ///
    /// Example: `let clear_m = scene.test_boxes(row, x, vh, ds_m, &mut hit);`
    fn test_boxes(&self, row: usize, x: Vec4, v: Vec4, reach_m: f32, hit: &mut Hit) -> f32 {
        (0..self.u.cluster_count as usize)
            .map(|k| self.test_cluster(row, k, x, v, reach_m, hit))
            .fold(1e9, f32::min)
    }

    /// `get_sky_direction` gives where the sky's colours are looked up for a point at infinity:
    /// its direction from the camera, with the elevation stretched so the ground's edge is the
    /// horizon.
    ///
    /// Example: `let d = scene.get_sky_direction(ideal_world);`
    fn get_sky_direction(&self, ideal_world: Vec4) -> Vec3 {
        let d: Vec3 = self.sky_basis * (self.world_to_camera * ideal_world).truncate().normalize();
        let elevation_rad: f32 = d.y.clamp(-1.0, 1.0).asin();
        let edge: f32 = self.u.ground_edge_elevation_rad;
        let height: f32 = ((elevation_rad - edge) / (HALF_PI - edge)).clamp(0.0, 1.0) * HALF_PI;
        let horizontal: Vec2 = if d.xz().length() > 1e-6 {
            d.xz().normalize()
        } else {
            Vec2::X
        };
        Vec3::new(
            height.cos() * horizontal.x,
            height.sin(),
            height.cos() * horizontal.y,
        )
    }

    /// `get_sky_color` gives the sky's colour in a direction from `get_sky_direction`, with clouds
    /// and the sun.
    ///
    /// Example: `let sky = scene.get_sky_color(d);`
    fn get_sky_color(&self, d: Vec3) -> Vec3 {
        let mut c: Vec3 =
            SKY_HORIZON_COLOR.lerp(SKY_ZENITH_COLOR, d.y.max(0.0).powf(SKY_GRADIENT_POWER));
        if d.y > 0.01 {
            let uv: Vec2 =
                d.xz() / (d.y + 0.1) * 0.9 + Vec2::new(self.u.time_s * CLOUD_DRIFT_PER_S, 0.0);
            let cloud: f32 = smoothstep(0.52, 0.78, get_fractal_noise(uv * CLOUD_SCALE))
                * 0.85
                * smoothstep(0.01, 0.15, d.y);
            c = c.lerp(CLOUD_COLOR, cloud);
        }
        let sun_cosine: f32 = d.dot(self.get_sky_direction(self.sun_ideal_world));
        c + SUN_COLOR
            * (smoothstep(SUN_DISK_INNER_COSINE, SUN_DISK_OUTER_COSINE, sun_cosine)
                * SUN_DISK_BRIGHTNESS
                + sun_cosine.max(0.0).powf(SUN_GLOW_POWER) * SUN_GLOW_BRIGHTNESS)
    }

    /// `get_ground_color` gives the ground's colour at a point of the {4,5} tiling: the point is
    /// folded into the central square by reflections, the reflections run backwards give its
    /// tile's centre (for a per-tile tint), and its Klein coordinates pick a cell.
    ///
    /// Example: `let c = scene.get_ground_color(to_world * xh);`
    fn get_ground_color(&self, mut p: Vec4) -> Vec3 {
        let mut edges: [usize; MAX_FOLD_COUNT] = [0; MAX_FOLD_COUNT];
        let mut fold_count: usize = 0;
        while fold_count < MAX_FOLD_COUNT {
            let (mut worst, mut edge) = (0.0f32, usize::MAX);
            for (e, n) in self.tile_edges.iter().enumerate() {
                let d: f32 = mdot(p, *n);
                if d > worst {
                    worst = d;
                    edge = e;
                }
            }
            if edge == usize::MAX {
                break;
            }
            p -= 2.0 * worst * self.tile_edges[edge];
            edges[fold_count] = edge;
            fold_count += 1;
        }
        let mut center: Vec4 = Vec4::W;
        for &edge in edges[..fold_count].iter().rev() {
            let n: Vec4 = self.tile_edges[edge];
            center -= 2.0 * mdot(center, n) * n;
        }
        let tile_seed: f32 = hash_3((center.truncate() * 64.0 + 0.5).floor());
        let cell_count: f32 = self.u.tile_cell_count as f32;
        let cell_position: Vec2 =
            (p.xz() / p.w / self.tile_klein_half_width * 0.5 + 0.5) * cell_count;
        let cell: Vec2 = cell_position
            .floor()
            .clamp(Vec2::ZERO, Vec2::splat(cell_count - 1.0));
        let is_road: bool = cell.x == 0.0
            || cell.y == 0.0
            || cell.x == cell_count - 1.0
            || cell.y == cell_count - 1.0;
        let r: f32 = hash_3(cell.extend(tile_seed * 97.0));
        let c: Vec3 = if is_road {
            ROAD_COLOR * (0.93 + 0.1 * r)
        } else if r < DIRT_FRACTION {
            DIRT_COLOR
        } else {
            let grass: Vec3 = if (cell.x + cell.y) % 2.0 > 0.5 {
                GRASS_DARK_COLOR
            } else {
                GRASS_LIGHT_COLOR
            };
            grass * (0.95 + 0.1 * tile_seed) * (0.95 + 0.08 * r)
        };
        c * get_block_texture(cell_position, tile_seed)
    }

    /// `shade_hit` gives the lit colour of the surface a ray hit, before fog.
    ///
    /// Example: `let c = scene.shade_hit(row, index, face, side, point);`
    fn shade_hit(&self, row: usize, index: i32, face: usize, side: f32, xh: Vec4) -> Vec3 {
        let r: f32 = self.u.curvature_radius_m;
        let up: Vec4 = to_tangent(xh, self.fetch(row, TEXEL_GROUND));
        let sun: Vec4 = to_tangent(xh, self.fetch(row, TEXEL_SUN));
        let (base, n, shade): (Vec3, Vec4, f32) = if index == GROUND_HIT {
            (
                self.get_ground_color(self.fetch_matrix(row, TEXEL_TO_WORLD) * xh),
                up,
                TOP_SHADE,
            )
        } else {
            let first: usize = TEXEL_BOXES + TEXELS_PER_BOX * index as usize;
            let (material, half_m, color) = (
                self.fetch(row, first + 3),
                self.fetch(row, first + 4),
                self.fetch(row, first + 5).truncate(),
            );
            let n: Vec4 = side * to_tangent(xh, self.fetch(row, first + face));
            let (axis_a, axis_b) = (if face == 0 { 1 } else { 0 }, if face == 2 { 1 } else { 2 });
            let uv: Vec2 = Vec2::new(
                r * mdot(xh, self.fetch(row, first + axis_a)).asinh() + half_m[axis_a],
                r * mdot(xh, self.fetch(row, first + axis_b)).asinh() + half_m[axis_b],
            );
            let material_id: u32 = material.w.round() as u32;
            if material_id == MATERIAL_GLOW {
                return color * GLOW_BRIGHTNESS * (0.7 + 0.3 * get_block_texture(uv, 0.0));
            }
            let mut base: Vec3 = color * get_block_texture(uv, (index * 3) as f32 + face as f32);
            if material_id == MATERIAL_GLASS {
                base = base.lerp(GLASS_SHEEN_COLOR, GLASS_SHEEN);
            }
            let up_dot: f32 = mdot(n, up);
            let shade: f32 = if up_dot > 0.5 {
                TOP_SHADE
            } else if up_dot < -0.5 {
                BOTTOM_SHADE
            } else if face == 0 {
                X_SIDE_SHADE
            } else {
                Z_SIDE_SHADE
            };
            (base, n, shade)
        };
        base * shade * (AMBIENT_LIGHT + SUN_LIGHT * mdot(n, sun).max(0.0))
    }

    /// `add_beam_glow` adds how much the laser beam shows near one straight piece of a ray, from
    /// `a` (where the ray had travelled `t0_m`) to `b` (at `t1_m`), in Klein coordinates.
    ///
    /// Example: `scene.add_beam_glow(row, a, b, 0.0, 3.0, &mut glow);`
    fn add_beam_glow(&self, row: usize, a: Vec4, b: Vec4, t0_m: f32, t1_m: f32, glow: &mut Glow) {
        let r: f32 = self.u.curvature_radius_m;
        let (ka, kb) = (a.truncate() / a.w, b.truncate() / b.w);
        let beam_radius: f32 = BEAM_RADIUS_M / r;
        let reach: f32 = BEAM_REACH * beam_radius.max(self.u.pixel_angle_rad);
        for c in 0..self.u.beam_chunk_count as usize {
            let chunk: Vec4 = self.fetch(row, TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * c);
            if distance_point_segment(chunk.truncate(), ka, kb) - chunk.w > reach {
                continue;
            }
            let info: Vec4 = self.fetch(row, TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * c + 1);
            let (first, count) = (info.x as usize, info.y as usize);
            let mut q0: Vec4 = self.fetch(row, TEXEL_BEAM_POINTS + first);
            for j in 1..count {
                let q1: Vec4 = self.fetch(row, TEXEL_BEAM_POINTS + first + j);
                let (d2, s) = get_segment_distance_squared(ka, kb, q0.truncate(), q1.truncate());
                let t_m: f32 = t0_m + s * (t1_m - t0_m);
                let sigma: f32 = beam_radius.max(self.u.pixel_angle_rad * (t_m / r).tanh());
                let fade: f32 = (-q0.w / BEAM_FADE_M).exp();
                glow.core = glow.core.max(fade * (-d2 / (sigma * sigma)).exp());
                glow.halo = glow.halo.max(fade * (-d2 / (4.8 * sigma * sigma)).exp());
                q0 = q1;
            }
        }
    }

    /// `get_spot_glow` gives how much the laser's hit spot shows at a hit point.
    ///
    /// Example: `let g = scene.get_spot_glow(row, xh, 4.0);`
    fn get_spot_glow(&self, row: usize, xh: Vec4, t_m: f32) -> f32 {
        let spot: Vec4 = self.fetch(row, TEXEL_SPOT);
        if spot.w <= 0.0 {
            return 0.0;
        }
        let r: f32 = self.u.curvature_radius_m;
        let radius: f32 =
            (SPOT_RADIUS_M / r).max(SPOT_PIXELS * self.u.pixel_angle_rad * (t_m / r).tanh());
        let d: Vec3 = xh.truncate() / xh.w - spot.truncate();
        spot.w * (-d.dot(d) / (radius * radius)).exp() * (-t_m / BEAM_FADE_M).exp()
    }
}

/// `trace_ray` follows one light ray backwards from `start` along `direction` (both in the camera
/// row's coordinates), calling `visit(row, a, b, t0_m, t1_m)` for each straight piece. Far from
/// every mouth a ray is one geodesic tested exactly; near a mouth it takes midpoint steps that
/// bend it and can carry it into the partner mouth's row.
///
/// Example: `let end = trace_ray(&scene, Vec4::W, v, |_, _, _, _, _| {});`
pub fn trace_ray(
    scene: &Scene,
    start: Vec4,
    direction: Vec4,
    mut visit: impl FnMut(usize, Vec4, Vec4, f32, f32),
) -> TraceEnd {
    let u: &Uniforms = scene.u;
    let r: f32 = u.curvature_radius_m;
    let max_travel_m: f32 = MAX_TRAVEL_PER_R * r;
    let (mut x, mut v, mut row) = (start, direction, 0usize);
    let (mut travelled_m, mut box_clear_until_m) = (0.0f32, -1.0f32);
    for _ in 0..MAX_STEP_COUNT {
        let (bend, q_nearest) = scene.get_bend(row, x, v);
        let is_far: bool = q_nearest > scene.q_cutoff;
        let (vh, tau_max, ds_m) = if is_far {
            let tau: f32 = scene.get_cutoff_entry_tau(row, x, v);
            (
                v,
                tau,
                if tau < 1.0 {
                    tau.atanh() * r
                } else {
                    max_travel_m
                },
            )
        } else {
            let nearest_m: f32 = get_distance_of_q(q_nearest) * r;
            let fraction: f32 = if nearest_m < NEAR_ZONE_PER_THROAT * u.throat_m {
                NEAR_STEP_PER_NEAREST
            } else {
                FAR_STEP_PER_NEAREST
            };
            let ds: f32 = (nearest_m * fraction).max(MIN_STEP_M);
            (to_tangent(x, v + bend * (0.5 * ds)), (ds / r).tanh(), ds)
        };
        let mut hit: Hit = Hit {
            tau: tau_max,
            index: NO_HIT,
            face: 0,
            side: 1.0,
        };
        let ground: Vec4 = scene.fetch(row, TEXEL_GROUND);
        let (ground_a, ground_b) = (mdot(x, ground), mdot(vh, ground));
        if ground_a > 0.0 && ground_b < 0.0 && -ground_a / ground_b < hit.tau {
            hit = Hit {
                tau: -ground_a / ground_b,
                index: GROUND_HIT,
                face: 0,
                side: 1.0,
            };
        }
        if travelled_m + ds_m > box_clear_until_m {
            box_clear_until_m = travelled_m + scene.test_boxes(row, x, vh, ds_m, &mut hit);
        }
        if hit.index != NO_HIT {
            let point: Vec4 = to_point(x + hit.tau * vh);
            let end_m: f32 = travelled_m + hit.tau.atanh() * r;
            visit(row, x, point, travelled_m, end_m);
            return TraceEnd::Surface {
                row,
                index: hit.index,
                face: hit.face,
                side: hit.side,
                point,
                travelled_m: end_m,
            };
        }
        if tau_max >= 1.0 || travelled_m > max_travel_m {
            visit(row, x, x + vh, travelled_m, travelled_m + max_travel_m);
            return TraceEnd::Sky { row, ideal: x + vh };
        }
        let x_before: Vec4 = x;
        if is_far {
            advance_geodesic(&mut x, &mut v, tau_max.atanh());
        } else {
            let (mut xm, mut vm) = (x, vh);
            advance_geodesic(&mut xm, &mut vm, 0.5 * ds_m / r);
            let (bend_middle, _) = scene.get_bend(row, xm, vm);
            v = vh;
            advance_geodesic(&mut x, &mut v, ds_m / r);
            v += (bend_middle - 0.5 * bend) * ds_m;
        }
        x = to_point(x);
        v = to_tangent(x, v);
        visit(row, x_before, x, travelled_m, travelled_m + ds_m);
        travelled_m += ds_m;
        if scene.cross_throat(&mut row, &mut x, &mut v) {
            box_clear_until_m = -1.0;
        }
    }
    TraceEnd::Lost
}

/// `render_pixel` gives one pixel's colour for a ray through normalized device coordinates `ndc`.
///
/// Example: `let c = render_pixel(&scene, Vec2::ZERO);`
fn render_pixel(scene: &Scene, ndc: Vec2) -> Vec3 {
    let u: &Uniforms = scene.u;
    let direction: Vec3 = Vec3::new(
        ndc.x * u.tan_half_fov * u.aspect,
        ndc.y * u.tan_half_fov,
        -1.0,
    )
    .normalize();
    let has_beam: bool = u.beam_chunk_count > 0;
    let mut glow: Glow = Glow::default();
    let end: TraceEnd = trace_ray(
        scene,
        Vec4::W,
        direction.extend(0.0),
        |row, a, b, t0, t1| {
            if has_beam {
                scene.add_beam_glow(row, a, b, t0, t1, &mut glow);
            }
        },
    );
    let mut color: Vec3 = match end {
        TraceEnd::Surface {
            row,
            index,
            face,
            side,
            point,
            travelled_m,
        } => {
            let mut c: Vec3 = scene.shade_hit(row, index, face, side, point);
            if u.fog_m > 0.0 {
                c = HAZE_COLOR.lerp(c, (-travelled_m / u.fog_m).exp());
            }
            let spot: f32 = scene.get_spot_glow(row, point, travelled_m);
            glow.halo = glow.halo.max(spot);
            glow.core = glow.core.max(spot * spot);
            c
        }
        TraceEnd::Sky { row, ideal } => scene.get_sky_color(
            scene.get_sky_direction(scene.fetch_matrix(row, TEXEL_TO_WORLD) * ideal),
        ),
        TraceEnd::Lost => LOST_COLOR,
    };
    color = color.lerp(LASER_HALO_COLOR, (0.9 * glow.halo).clamp(0.0, 1.0))
        + LASER_CORE_COLOR * glow.core;
    color / (Vec3::ONE + (color - Vec3::ONE).max(Vec3::ZERO) * HIGHLIGHT_ROLLOFF)
}

/// `render` ray-traces the whole image into `pixels` (RGBA, 8 bits per channel, top row first),
/// one image row per rayon task.
///
/// Example: `render(&data, &mut pixels);`
pub fn render(data: &SceneData, pixels: &mut [u8]) {
    let scene: Scene = Scene::New(data);
    let (width, height) = (
        data.uniforms.width_px as usize,
        data.uniforms.height_px as usize,
    );
    assert_eq!(
        pixels.len(),
        width * height * BYTES_PER_PIXEL,
        "render: the pixel buffer doesn't match {width}x{height}"
    );
    assert!(data.uniforms.mouth_count as usize <= MAX_MOUTH_COUNT);
    pixels
        .par_chunks_mut(width * BYTES_PER_PIXEL)
        .enumerate()
        .for_each(|(y, line)| {
            let ndc_y: f32 = 1.0 - 2.0 * (y as f32 + 0.5) / height as f32;
            for x in 0..width {
                let ndc_x: f32 = 2.0 * (x as f32 + 0.5) / width as f32 - 1.0;
                let c: Vec3 = render_pixel(&scene, Vec2::new(ndc_x, ndc_y))
                    .clamp(Vec3::ZERO, Vec3::ONE)
                    * 255.0;
                line[x * BYTES_PER_PIXEL..(x + 1) * BYTES_PER_PIXEL]
                    .copy_from_slice(&[c.x as u8, c.y as u8, c.z as u8, 255]);
            }
        });
}
