/**
 * @file
 * @brief `render_metal.metal` ray-traces the hyperbolic wormholes scene on the GPU, one thread per
 * pixel. It is a port of `render_cpu.rs` (itself a port of the WebGL demo's fragment shader), with
 * the same scene rows, constants and laser.
 *
 * @par Interface
 * - Kernel `trace_pixels`: buffer 0 holds the scene rows (`ROW_COUNT` rows of `ROW_TEXEL_COUNT`
 *   float4s), buffer 1 the `Uniforms`, texture 0 the output image, top row first.
 * - `render_metal.rs` replaces each placeholder (a constant's name between double underscores) with the
 *   Rust constant of that name before compiling.
 *
 * @par Example
 * @code
 * encoder.dispatchThreads_threadsPerThreadgroup(width x height, 8 x 8);
 * @endcode
 *
 * @par Must know
 * - Gotchas: positions are 4-vectors on the hyperboloid; `mdot` is the Minkowski product.
 */

#include <metal_stdlib>
using namespace metal;

constant int MAX_MOUTH_COUNT = __MAX_MOUTH_COUNT__;
constant int ROW_TEXEL_COUNT = __ROW_TEXEL_COUNT__;
constant int TEXEL_TO_WORLD = __TEXEL_TO_WORLD__;
constant int TEXEL_GROUND = __TEXEL_GROUND__;
constant int TEXEL_SUN = __TEXEL_SUN__;
constant int TEXEL_MOUTHS = __TEXEL_MOUTHS__;
constant int TEXEL_GLUINGS = __TEXEL_GLUINGS__;
constant int TEXELS_PER_GLUING = __TEXELS_PER_GLUING__;
constant int TEXEL_BOXES = __TEXEL_BOXES__;
constant int TEXELS_PER_BOX = __TEXELS_PER_BOX__;
constant int TEXEL_CLUSTERS = __TEXEL_CLUSTERS__;
constant int TEXELS_PER_CLUSTER = __TEXELS_PER_CLUSTER__;
constant int TEXEL_CLUSTER_INFO = __TEXEL_CLUSTER_INFO__;
constant int TEXEL_CLUSTER_ORDER = __TEXEL_CLUSTER_ORDER__;
constant int TEXEL_SPOT = __TEXEL_SPOT__;
constant int TEXEL_BEAM_CHUNKS = __TEXEL_BEAM_CHUNKS__;
constant int TEXELS_PER_BEAM_CHUNK = __TEXELS_PER_BEAM_CHUNK__;
constant int TEXEL_BEAM_POINTS = __TEXEL_BEAM_POINTS__;
constant int MATERIAL_GLASS = __MATERIAL_GLASS__;
constant int MATERIAL_GLOW = __MATERIAL_GLOW__;
constant float LAMBDA_SQUARED = 0.25;
constant float MAX_TRAVEL_PER_R = 24.0;
constant int MAX_STEP_COUNT = 700;
constant int MAX_FOLD_COUNT = 64;
constant float NEAR_STEP_PER_NEAREST = 0.12;
constant float FAR_STEP_PER_NEAREST = 0.25;
constant float NEAR_ZONE_PER_THROAT = 10.0;
constant float MIN_STEP_M = 0.005;
constant float RK4_STEP_PER_NEAREST = 0.5;
constant float RK4_ZONE_START_PER_THROAT = 3.0;
constant float JUMP_OVERSHOOT_M = 0.002;
constant int THROAT_TABLE_SIZE = __THROAT_TABLE_SIZE__;
constant int TEXELS_PER_TABLE_ENTRY = 4;
constant float TABLE_FIELD_PROBE = 0.004;
constant float THROAT_TABLE_STEP_PER_NEAREST = 0.01;
constant float CAMERA_TABLE_STEP_PER_NEAREST = 0.015;
constant int CAMERA_TABLE_SIZE = __CAMERA_TABLE_SIZE__;
constant int CAMERA_TABLE_ROWS = __CAMERA_TABLE_ROWS__;
constant float CAMERA_TABLE_MIN_PER_THROAT = 0.9;
constant float TABLE_MIN_STEP_PER_STEP = 0.07;
constant int THROAT_TABLE_MAX_STEP_COUNT = 40000;
constant int CAMERA_TABLE_MAX_STEP_COUNT = 20000;
constant float CUTOFF_ENTRY_SHRINK = 0.98;
constant float HALF_PI = 1.5707963;
constant float3 SKY_HORIZON_COLOR = float3(0.78, 0.86, 0.95);
constant float3 HAZE_COLOR = SKY_HORIZON_COLOR;
constant float3 SKY_ZENITH_COLOR = float3(0.30, 0.52, 0.88);
constant float3 CLOUD_COLOR = float3(1.0);
constant float3 SUN_COLOR = float3(1.0, 0.92, 0.75);
constant float3 LOST_COLOR = float3(0.02, 0.02, 0.03);
constant float3 ROAD_COLOR = float3(0.58, 0.58, 0.60);
constant float3 DIRT_COLOR = float3(0.55, 0.42, 0.27);
constant float3 GRASS_DARK_COLOR = float3(0.36, 0.60, 0.25);
constant float3 GRASS_LIGHT_COLOR = float3(0.40, 0.65, 0.28);
constant float3 GLASS_SHEEN_COLOR = float3(0.95);
constant float3 LASER_HALO_COLOR = float3(1.0, 0.06, 0.03);
constant float3 LASER_CORE_COLOR = float3(1.0, 0.62, 0.52);
constant float DIRT_FRACTION = 0.04;
constant float TEXTURE_CELLS_PER_BLOCK = 8.0;
constant float TEXTURE_MIN = 0.86;
constant float TEXTURE_RANGE = 0.22;
constant float EDGE_DARKEST = 0.55;
constant float EDGE_WIDTH = 0.045;
constant float AMBIENT_LIGHT = 0.68;
constant float SUN_LIGHT = 0.32;
constant float TOP_SHADE = 1.0;
constant float BOTTOM_SHADE = 0.5;
constant float X_SIDE_SHADE = 0.82;
constant float Z_SIDE_SHADE = 0.68;
constant float GLASS_SHEEN = 0.2;
constant float GLOW_BRIGHTNESS = 1.25;
constant float SUN_DISK_INNER_COSINE = 0.99955;
constant float SUN_DISK_OUTER_COSINE = 0.9998;
constant float SUN_DISK_BRIGHTNESS = 3.0;
constant float SUN_GLOW_POWER = 300.0;
constant float SUN_GLOW_BRIGHTNESS = 0.4;
constant float SKY_GRADIENT_POWER = 0.55;
constant float CLOUD_SCALE = 1.6;
constant float CLOUD_DRIFT_PER_S = 0.01;
constant float HIGHLIGHT_ROLLOFF = 0.6;
constant float BEAM_RADIUS_M = 0.012;
constant float BEAM_REACH = 4.8;
constant float BEAM_FADE_M = 260.0;
constant float SPOT_RADIUS_M = 0.07;
constant float SPOT_PIXELS = 2.5;
constant float CANDIDATE_WINDOW_M = 3.0;
constant int NO_HIT = -2;
constant int GROUND_HIT = -1;
constant int SKY_HIT = -3;
constant int LOST_HIT = -4;
constant int BYTES_PER_RAY_END = __BYTES_PER_RAY_END__;
constant float GLOW_FLOOR = 1.0 / 1024.0;
constant float SMOOTH_SECOND_DIFFERENCE_PER_FIRST = 1.6;
constant int REFINED_PIXELS_PER_CELL = 3;
constant int REFINE_GROUP_SIZE = __REFINE_GROUP_SIZE__;
constant float TILE_SIDE_COUNT = 4.0;
constant float TILES_PER_VERTEX = 5.0;

/** @brief `Uniforms` matches the Rust `Uniforms` struct field by field (240 bytes). */
struct Uniforms {
    float4x4 world_to_camera;
    float4 sky_basis[3];
    float4 sun_ideal_world;
    int next_rows[MAX_MOUTH_COUNT];
    float curvature_radius_m;
    float tan_half_fov;
    float aspect;
    float throat_m;
    float bend_cutoff_m;
    float fog_m;
    float time_s;
    float ground_edge_elevation_rad;
    float pixel_angle_rad;
    int mouth_count;
    int cluster_count;
    int tile_cell_count;
    int beam_chunk_count;
    int width_px;
    int height_px;
    int camera_mouth;
    float camera_distance_m;
    float jump_m;
    int padding[2];
};

/** @brief `Scene` is one frame's rows and uniforms, with values every ray reuses. */
struct Scene {
    device const float4 *rows;
    constant Uniforms &u;
    device const float4 *throat_table;
    device const float4 *camera_table;
    float q_cutoff;
    float q_throat;
    float q_near_zone;
    float q_rk4_start;
    float q_jump;
    float tile_klein_half_width;
    float4 tile_edges[4];
};

/**
 * @brief `RayEnd` is where one pixel's ray ended: the surface it hit (`kind` is a box index, `GROUND_HIT`,
 * `SKY_HIT` or `LOST_HIT`) in row `row`, the hit point in that row's coordinates (for the sky, the sky lookup
 * direction), how far the ray travelled, and the laser glow gathered along it.
 */
struct RayEnd {
    float4 point;
    float end_m;
    float core;
    float halo;
    int kind;
    int row;
    int face;
    float side;
};
static_assert(sizeof(RayEnd) == BYTES_PER_RAY_END, "render_metal.rs sizes the sample buffer by BYTES_PER_RAY_END");

/** @brief `Hit` is the nearest surface so far along one straight piece of a ray. */
struct Hit {
    float tau;
    int index;
    int face;
    float side;
};

/**
 * @brief `mdot` computes the Minkowski product.
 * @return ax bx + ay by + az bz - aw bw.
 * @par Example
 * @code
 * float cosh_d = -mdot(x, c);
 * @endcode
 */
static float mdot(float4 a, float4 b) { return dot(a.xyz, b.xyz) - a.w * b.w; }

/**
 * @brief `to_tangent` projects `w` onto the tangent space at point `x` and scales it to unit length.
 * @par Example
 * @code
 * float4 up = to_tangent(x, ground);
 * @endcode
 */
static float4 to_tangent(float4 x, float4 w) { float4 t = w + mdot(w, x) * x; return t * rsqrt(max(mdot(t, t), 1e-30)); }

/**
 * @brief `to_point` puts a 4-vector back on the hyperboloid.
 * @par Example
 * @code
 * x = to_point(x);
 * @endcode
 */
static float4 to_point(float4 x) { return x * rsqrt(max(-mdot(x, x), 1e-30)); }

/**
 * @brief `advance_geodesic` moves a point and its unit direction `s` curvature radii along their geodesic.
 * @par Example
 * @code
 * advance_geodesic(x, v, ds_m / r);
 * @endcode
 */
static void advance_geodesic(thread float4 &x, thread float4 &v, float s) {
    float c = cosh(s), sh = sinh(s);
    float4 moved = c * x + sh * v;
    v = sh * x + c * v;
    x = moved;
}

/**
 * @brief `get_q` gives q = 4 sinh²(d/2) for the distance between two points, from their difference.
 * @par Example
 * @code
 * float q = get_q(x, c);
 * @endcode
 */
static float get_q(float4 a, float4 b) { float4 d = a - b; return max(mdot(d, d), 0.0); }

/**
 * @brief `get_distance_of_q` gives the distance in curvature radii for q = 4 sinh²(d/2).
 * @par Example
 * @code
 * float d = get_distance_of_q(q);
 * @endcode
 */
static float get_distance_of_q(float q) { return 2.0 * asinh(0.5 * sqrt(q)); }

/**
 * @brief `get_dln_n_flat` gives d ln n / dρ for the flat "black hole foil" wormhole (Damour & Solodukhin 2007).
 * @par Example
 * @code
 * float g = get_dln_n_flat(3.0, 0.35);
 * @endcode
 */
static float get_dln_n_flat(float rho_m, float throat_m) {
    float psi = 1.0 + throat_m / rho_m, r_m = rho_m * psi * psi, rs_m = 4.0 * throat_m;
    float a = (1.0 - rs_m / r_m + LAMBDA_SQUARED) / (1.0 + LAMBDA_SQUARED);
    return -2.0 * throat_m / (rho_m * rho_m * psi) - rs_m * psi * (1.0 - throat_m / rho_m) / (2.0 * r_m * r_m * (1.0 + LAMBDA_SQUARED) * a);
}

/**
 * @brief `fetch` reads one texel of a row.
 * @par Example
 * @code
 * float4 ground = fetch(scene, row, TEXEL_GROUND);
 * @endcode
 */
static float4 fetch(thread const Scene &scene, int row, int texel) { return scene.rows[row * ROW_TEXEL_COUNT + texel]; }

/**
 * @brief `fetch_matrix` reads four texels of a row as a matrix's columns.
 * @par Example
 * @code
 * float4x4 to_world = fetch_matrix(scene, row, TEXEL_TO_WORLD);
 * @endcode
 */
static float4x4 fetch_matrix(thread const Scene &scene, int row, int first) {
    return float4x4(fetch(scene, row, first), fetch(scene, row, first + 1), fetch(scene, row, first + 2), fetch(scene, row, first + 3));
}

/**
 * @brief `get_bend` gives how fast a ray turns (per block, at right angles to `v`), summed over mouths
 * within the cutoff, and sets `q_nearest` to q of the nearest mouth.
 * @par Example
 * @code
 * float q_nearest; float4 bend = get_bend(scene, row, x, v, q_nearest);
 * @endcode
 */
static float4 get_bend(thread const Scene &scene, int row, float4 x, float4 v, thread float &q_nearest) {
    float r = scene.u.curvature_radius_m;
    float4 g = float4(0.0);
    q_nearest = 3.4e38;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float4 c = fetch(scene, row, TEXEL_MOUTHS + k);
        float q = get_q(x, c);
        q_nearest = min(q_nearest, q);
        if (q > scene.q_cutoff || q < 1e-12) continue;
        float cosh_rho = 1.0 + 0.5 * q, sinh_rho = sqrt(q * (1.0 + 0.25 * q)), e_rho = cosh_rho + sinh_rho;
        g += get_dln_n_flat(r * sinh_rho * e_rho, scene.u.throat_m) * e_rho * e_rho * (cosh_rho * x - c) / sinh_rho;
    }
    return g - mdot(g, v) * v;
}

/**
 * @brief `get_mouth_gradient` gives one mouth's part of the gradient of ln n at `x` (unprojected), or zero
 * outside the cutoff.
 * @par Example
 * @code
 * float4 g = get_mouth_gradient(scene, x, c);
 * @endcode
 */
static float4 get_mouth_gradient(thread const Scene &scene, float4 x, float4 c) {
    float q = get_q(x, c);
    if (q > scene.q_cutoff || q < 1e-12) return float4(0.0);
    float cosh_rho = 1.0 + 0.5 * q, sinh_rho = sqrt(q * (1.0 + 0.25 * q)), e_rho = cosh_rho + sinh_rho;
    return get_dln_n_flat(scene.u.curvature_radius_m * sinh_rho * e_rho, scene.u.throat_m) * e_rho * e_rho * (cosh_rho * x - c) / sinh_rho;
}

/**
 * @brief `get_bend_split` is `get_bend`, and also gives the nearest mouth's index and the other mouths' part of
 * the gradient (unprojected), for `step_rk4`.
 * @par Example
 * @code
 * float4 bend = get_bend_split(scene, row, x, v, q_nearest, nearest, g_others);
 * @endcode
 */
static float4 get_bend_split(thread const Scene &scene, int row, float4 x, float4 v, thread float &q_nearest, thread int &nearest,
                             thread float4 &g_others) {
    float4 g = float4(0.0), g_nearest = float4(0.0);
    q_nearest = 3.4e38;
    nearest = 0;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float4 c = fetch(scene, row, TEXEL_MOUTHS + k);
        float4 gk = get_mouth_gradient(scene, x, c);
        float q = get_q(x, c);
        if (q < q_nearest) { q_nearest = q; nearest = k; g_nearest = gk; }
        g += gk;
    }
    g_others = g - g_nearest;
    return g - mdot(g, v) * v;
}

/**
 * @brief `step_rk4` moves a ray `ds_m` blocks with the classical fourth-order Runge-Kutta method, for
 * x' = v / R and v' = x / R + bend(x, v) (a geodesic of H³ plus the bending), starting from the bending
 * `bend` already known at `x`, and puts the result back on the hyperboloid. The later stages re-evaluate only
 * the nearest mouth (centre `c`), keeping the other mouths' gradient `g_others` from the start, as they are at
 * least a few blocks away.
 * @par Example
 * @code
 * step_rk4(scene, row, x, v, bend, g_others, c, ds_m);
 * @endcode
 */
static void step_rk4(thread const Scene &scene, int row, thread float4 &x, thread float4 &v, float4 bend, float4 g_others, float4 c, float ds_m) {
    float r = scene.u.curvature_radius_m;
    float4 kx1 = v / r, kv1 = x / r + bend;
    float4 x2 = x + 0.5 * ds_m * kx1, v2 = v + 0.5 * ds_m * kv1;
    float4 g2 = g_others + get_mouth_gradient(scene, x2, c);
    float4 kx2 = v2 / r, kv2 = x2 / r + g2 - mdot(g2, v2) * v2;
    float4 x3 = x + 0.5 * ds_m * kx2, v3 = v + 0.5 * ds_m * kv2;
    float4 g3 = g_others + get_mouth_gradient(scene, x3, c);
    float4 kx3 = v3 / r, kv3 = x3 / r + g3 - mdot(g3, v3) * v3;
    float4 x4 = x + ds_m * kx3, v4 = v + ds_m * kv3;
    float4 g4 = g_others + get_mouth_gradient(scene, x4, c);
    float4 kx4 = v4 / r, kv4 = x4 / r + g4 - mdot(g4, v4) * v4;
    x = to_point(x + ds_m / 6.0 * (kx1 + 2.0 * kx2 + 2.0 * kx3 + kx4));
    v = to_tangent(x, v + ds_m / 6.0 * (kv1 + 2.0 * kv2 + 2.0 * kv3 + kv4));
}

/**
 * @brief `get_cutoff_entry_tau` finds where a geodesic first comes within the bending cutoff of any mouth,
 * as tanh of the distance, or 1 if it never does.
 * @par Example
 * @code
 * float tau_max = get_cutoff_entry_tau(scene, row, x, v);
 * @endcode
 */
static float get_cutoff_entry_tau(thread const Scene &scene, int row, float4 x, float4 v) {
    float k_cosh = cosh(CUTOFF_ENTRY_SHRINK * scene.u.bend_cutoff_m / scene.u.curvature_radius_m), best = 1.0;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float4 c = fetch(scene, row, TEXEL_MOUTHS + k);
        float a = -mdot(x, c), b = -mdot(v, c), discriminant = k_cosh * k_cosh + b * b - a * a;
        if (b >= 0.0 || discriminant < 0.0) continue;
        float tau = (-a * b - k_cosh * sqrt(discriminant)) / (b * b + k_cosh * k_cosh);
        if (tau > 0.0) best = min(best, tau);
    }
    return best;
}

/**
 * @brief `cross_throat` moves a ray through a throat if it is inside one, into the partner mouth's row.
 * @return Whether the ray went through.
 * @par Example
 * @code
 * if (cross_throat(scene, row, x, v)) cache.global_clear_until_m = -1.0;
 * @endcode
 */
static bool cross_throat(thread const Scene &scene, thread int &row, thread float4 &x, thread float4 &v) {
    float r = scene.u.curvature_radius_m;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float4 c = fetch(scene, row, TEXEL_MOUTHS + k);
        float q = get_q(x, c);
        float sinh_rho = sqrt(q * (1.0 + 0.25 * q)), cosh_rho = 1.0 + 0.5 * q;
        float rho_effective_m = r * sinh_rho * (cosh_rho + sinh_rho);
        if (rho_effective_m >= scene.u.throat_m) continue;
        float4 outward = (x - cosh_rho * c) / sinh_rho;
        float4 radial = (cosh_rho * x - c) / sinh_rho;
        float along = mdot(v, radial);
        float4x4 gluing = fetch_matrix(scene, row, TEXEL_GLUINGS + TEXELS_PER_GLUING * k);
        float new_rho = 0.5 * log(1.0 + 2.0 * scene.u.throat_m * scene.u.throat_m / rho_effective_m / r);
        float4 new_outward = gluing * outward, sideways = gluing * (v - along * radial);
        float4 origin = float4(0.0, 0.0, 0.0, 1.0);
        x = to_point(cosh(new_rho) * origin + sinh(new_rho) * new_outward);
        v = to_tangent(x, sideways - along * (sinh(new_rho) * origin + cosh(new_rho) * new_outward));
        row = scene.u.next_rows[k];
        return true;
    }
    return false;
}

/**
 * @brief `test_slabs` intersects a geodesic with a box stored as a box's first five texels (three normals,
 * sinh of the half sizes, half sizes), within reach (cosh and sinh of the reach in curvature radii). Along
 * x cosh s + v sinh s, with τ = tanh s, each slab |⟨x, N⟩| ≤ σ is a quadratic inequality in τ, so the box
 * is an interval of τ, which it sets (`tau_in` > `tau_out` when the geodesic misses or is out of reach),
 * with the face and side entered.
 * @return A lower bound on the start's distance from the box, in blocks.
 * @par Example
 * @code
 * float lower_m = test_slabs(scene, row, first, x, v, reach_cosh, reach_sinh, tau_in, tau_out, face, side);
 * @endcode
 */
static float test_slabs(thread const Scene &scene, int row, int first, float4 x, float4 v, float reach_cosh, float reach_sinh,
                        thread float &tau_in, thread float &tau_out, thread int &face, thread float &side) {
    float r = scene.u.curvature_radius_m;
    float4 sigma = fetch(scene, row, first + 3);
    float4 normals[3] = {fetch(scene, row, first), fetch(scene, row, first + 1), fetch(scene, row, first + 2)};
    float3 a = float3(mdot(x, normals[0]), mdot(x, normals[1]), mdot(x, normals[2]));
    tau_in = 1.0;
    tau_out = -1.0;
    for (int i = 0; i < 3; i++) {
        float sinh_limit = sigma[i] * reach_cosh + sqrt(1.0 + sigma[i] * sigma[i]) * reach_sinh;
        if (abs(a[i]) > sinh_limit) return r * asinh(abs(a[i])) - fetch(scene, row, first + 4)[i];
    }
    float4 half_m = fetch(scene, row, first + 4);
    float3 lower = r * asinh(abs(a)) - half_m.xyz;
    float lower_m = max(lower.x, max(lower.y, lower.z));
    float temp_in = -1.0, temp_out = 1.0;
    for (int i = 0; i < 3; i++) {
        float ai = a[i], bi = mdot(v, normals[i]), s = sigma[i];
        float discriminant = s * s + bi * bi - ai * ai;
        if (discriminant < 0.0) return lower_m;
        float root = s * sqrt(discriminant), denominator = bi * bi + s * s;
        float tau_0 = (-ai * bi - root) / denominator, tau_1 = (-ai * bi + root) / denominator;
        if (tau_0 > temp_in) { temp_in = tau_0; face = i; side = ai + bi * tau_0 >= 0.0 ? 1.0 : -1.0; }
        temp_out = min(temp_out, tau_1);
    }
    tau_in = temp_in;
    tau_out = temp_out;
    return lower_m;
}

/**
 * @brief `test_box` finds where a geodesic first enters a box within reach, lowering `hit` if nearer.
 * @return A lower bound on the start's distance from the box, in blocks.
 * @par Example
 * @code
 * float lower_m = test_box(scene, row, b, x, v, reach_cosh, reach_sinh, hit);
 * @endcode
 */
static float test_box(thread const Scene &scene, int row, int b, float4 x, float4 v, float reach_cosh, float reach_sinh, thread Hit &hit) {
    float tau_in, tau_out, side = 1.0;
    int face = 0;
    float lower_m = test_slabs(scene, row, TEXEL_BOXES + TEXELS_PER_BOX * b, x, v, reach_cosh, reach_sinh, tau_in, tau_out, face, side);
    if (tau_in <= tau_out && tau_in >= 0.0 && tau_in < hit.tau) { hit.tau = tau_in; hit.index = b; hit.face = face; hit.side = side; }
    return lower_m;
}

/**
 * @brief `test_cluster` tests a geodesic piece against one cluster's boxes, skipping them all unless the piece
 * reaches the cluster's bounding box before the nearest hit so far (a one-box cluster tests its box directly).
 * @return A lower bound on the distance to the cluster, in blocks.
 * @par Example
 * @code
 * float lower_m = test_cluster(scene, row, k, x, vh, reach_cosh, reach_sinh, hit);
 * @endcode
 */
static float test_cluster(thread const Scene &scene, int row, int k, float4 x, float4 v, float reach_cosh, float reach_sinh, thread Hit &hit) {
    int first = TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k;
    float4 info = fetch(scene, row, first + TEXEL_CLUSTER_INFO);
    int first_box = int(info.x + 0.5), box_count = int(info.y + 0.5);
    if (box_count > 1) {
        float tau_in, tau_out, side;
        int face;
        float lower_m = test_slabs(scene, row, first, x, v, reach_cosh, reach_sinh, tau_in, tau_out, face, side);
        if (tau_in > tau_out || tau_out < 0.0 || tau_in >= hit.tau) return lower_m;
    }
    float nearest_m = 1e9;
    for (int b = first_box; b < first_box + box_count; b++) nearest_m = min(nearest_m, test_box(scene, row, b, x, v, reach_cosh, reach_sinh, hit));
    return nearest_m;
}

/**
 * @brief `get_cluster_lower_bound` gives a distance a point at `height_m` is at least from a cluster: past its
 * bounding box's slabs, and no less than its height above the cluster's top.
 * @return The lower bound, in blocks (negative inside).
 * @par Example
 * @code
 * float lower_m = get_cluster_lower_bound(scene, row, k, x, height_m);
 * @endcode
 */
static float get_cluster_lower_bound(thread const Scene &scene, int row, int k, float4 x, float height_m) {
    float r = scene.u.curvature_radius_m;
    int first = TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k;
    float above_top_m = height_m - fetch(scene, row, first + TEXEL_CLUSTER_INFO).z;
    float3 a = float3(mdot(x, fetch(scene, row, first)), mdot(x, fetch(scene, row, first + 1)), mdot(x, fetch(scene, row, first + 2)));
    float3 lower = r * asinh(abs(a)) - fetch(scene, row, first + 4).xyz;
    return max(above_top_m, max(lower.x, max(lower.y, lower.z)));
}

/**
 * @brief `BoxCache` is one ray's memory of the clusters near it: how many clusters of the row's distance order
 * it has reached (`activated_count`), where its row's distances are measured from (`row_start_m`, how far along
 * the ray the row's origin effectively was), a bit mask of the candidates from the last full pass, and how far
 * the ray can go before another full pass (`global_clear_until_m`) or before a candidate could be reached
 * (`local_clear_until_m`).
 */
struct BoxCache {
    int activated_count;
    float row_start_m;
    uint candidates;
    float global_clear_until_m;
    float local_clear_until_m;
};

/**
 * @brief `reset_box_cache` starts a ray's memory of nearby clusters afresh in a row, at point `x`.
 * @par Example
 * @code
 * reset_box_cache(scene, cache, x, travelled_m);
 * @endcode
 */
static void reset_box_cache(thread const Scene &scene, thread BoxCache &cache, float4 x, float travelled_m) {
    cache.activated_count = 0;
    cache.row_start_m = travelled_m - get_distance_of_q(get_q(x, float4(0.0, 0.0, 0.0, 1.0))) * scene.u.curvature_radius_m;
    cache.candidates = 0;
    cache.global_clear_until_m = -1.0;
    cache.local_clear_until_m = -1.0;
}

/**
 * @brief `test_boxes` tests a geodesic piece starting `travelled_m` along the ray against the boxes it might
 * reach. A full pass looks only at clusters the ray could have reached by now, by the row's distance order
 * (cluster k is at least D_k from the row's origin, so it is out of reach until the ray has gone D_k), bounds
 * their distances, keeps those within `CANDIDATE_WINDOW_M` (or the piece's reach) in the mask, and notes when
 * any other could matter; until then only the candidates are tested, and
 * only once one could be within reach. A piece that climbs from above a cluster's top skips that cluster: along
 * a geodesic sinh(height) = a cosh s + b sinh s, which only grows when a and b are not negative.
 * @par Example
 * @code
 * test_boxes(scene, row, x, vh, travelled_m, ds_m, hit, cache);
 * @endcode
 */
static void test_boxes(thread const Scene &scene, int row, float4 x, float4 v, float travelled_m, float reach_m, thread Hit &hit, thread BoxCache &cache) {
    if (travelled_m + reach_m <= cache.local_clear_until_m) return;
    float4 ground = fetch(scene, row, TEXEL_GROUND);
    float ground_a = mdot(x, ground), ground_b = mdot(v, ground);
    float height_m = scene.u.curvature_radius_m * asinh(ground_a);
    bool is_climbing = ground_a > 0.0 && ground_b >= 0.0;
    if (travelled_m + reach_m > cache.global_clear_until_m) {
        float window_m = max(reach_m, CANDIDATE_WINDOW_M), row_travel_m = travelled_m - cache.row_start_m, other_m = 1e9;
        while (cache.activated_count < scene.u.cluster_count && fetch(scene, row, TEXEL_CLUSTER_ORDER + cache.activated_count).y <= row_travel_m + window_m) {
            cache.activated_count++;
        }
        if (cache.activated_count < scene.u.cluster_count) other_m = fetch(scene, row, TEXEL_CLUSTER_ORDER + cache.activated_count).y - row_travel_m;
        cache.candidates = 0;
        for (int i = 0; i < cache.activated_count; i++) {
            int k = int(fetch(scene, row, TEXEL_CLUSTER_ORDER + i).x + 0.5);
            float lower_m = get_cluster_lower_bound(scene, row, k, x, height_m);
            if (lower_m <= window_m) cache.candidates |= 1u << k;
            else other_m = min(other_m, lower_m);
        }
        cache.global_clear_until_m = travelled_m + other_m;
    }
    float r = scene.u.curvature_radius_m, nearest_m = 1e9;
    float reach_cosh = cosh(reach_m / r), reach_sinh = sinh(reach_m / r);
    for (uint left = cache.candidates; left != 0; left &= left - 1) {
        int k = int(ctz(left));
        float above_top_m = height_m - fetch(scene, row, TEXEL_CLUSTERS + TEXELS_PER_CLUSTER * k + TEXEL_CLUSTER_INFO).z;
        if (is_climbing && above_top_m > 0.0) { nearest_m = min(nearest_m, above_top_m); continue; }
        nearest_m = min(nearest_m, test_cluster(scene, row, k, x, v, reach_cosh, reach_sinh, hit));
    }
    cache.local_clear_until_m = min(travelled_m + nearest_m, cache.global_clear_until_m);
}

/** @brief `hash_3` gives a pseudo-random number in [0, 1) for a 3-vector. @par Example @code float h = hash_3(p); @endcode */
static float hash_3(float3 p) { return abs(fract(sin(dot(p, float3(12.9898, 78.233, 37.719))) * 43758.547)); }

/** @brief `hash_2` gives a pseudo-random number in [0, 1) for a 2-vector. @par Example @code float h = hash_2(p); @endcode */
static float hash_2(float2 p) { return abs(fract(sin(dot(p, float2(127.1, 311.7))) * 43758.547)); }

/** @brief `get_value_noise` gives smooth 2D value noise. @par Example @code float n = get_value_noise(uv); @endcode */
static float get_value_noise(float2 p) {
    float2 i = floor(p), f = p - i;
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash_2(i), hash_2(i + float2(1, 0)), f.x), mix(hash_2(i + float2(0, 1)), hash_2(i + float2(1, 1)), f.x), f.y);
}

/** @brief `get_fractal_noise` sums five octaves of value noise. @par Example @code float c = get_fractal_noise(uv); @endcode */
static float get_fractal_noise(float2 p) {
    float sum = 0.0, amplitude = 0.5;
    for (int i = 0; i < 5; i++) { sum += amplitude * get_value_noise(p); p *= 2.03; amplitude *= 0.5; }
    return sum;
}

/**
 * @brief `get_block_texture` gives a block face's speckles and dark edges.
 * @par Example
 * @code
 * base *= get_block_texture(uv, 3.0);
 * @endcode
 */
static float get_block_texture(float2 uv, float seed) {
    float2 f = uv - floor(uv);
    float speckle = TEXTURE_MIN + TEXTURE_RANGE * hash_3(float3(floor(f * TEXTURE_CELLS_PER_BLOCK) + floor(uv) * 17.0, seed));
    float2 edge = min(f, 1.0 - f);
    return speckle * mix(EDGE_DARKEST, 1.0, smoothstep(0.0, EDGE_WIDTH, min(edge.x, edge.y)));
}

/**
 * @brief `get_sky_direction` gives the sky lookup direction for a point at infinity, with the elevation
 * stretched so the ground's edge is the horizon.
 * @par Example
 * @code
 * float3 d = get_sky_direction(scene, ideal_world);
 * @endcode
 */
static float3 get_sky_direction(thread const Scene &scene, float4 ideal_world) {
    float3 seen = normalize((scene.u.world_to_camera * ideal_world).xyz);
    float3 d = float3(dot(scene.u.sky_basis[0].xyz, seen), dot(scene.u.sky_basis[1].xyz, seen), dot(scene.u.sky_basis[2].xyz, seen));
    float elevation_rad = asin(clamp(d.y, -1.0, 1.0));
    float edge = scene.u.ground_edge_elevation_rad;
    float height = clamp((elevation_rad - edge) / (HALF_PI - edge), 0.0, 1.0) * HALF_PI;
    float2 horizontal = length(d.xz) > 1e-6 ? normalize(d.xz) : float2(1.0, 0.0);
    return float3(cos(height) * horizontal.x, sin(height), cos(height) * horizontal.y);
}

/**
 * @brief `get_sky_color` gives the sky's colour with clouds and the sun.
 * @par Example
 * @code
 * float3 sky = get_sky_color(scene, d);
 * @endcode
 */
static float3 get_sky_color(thread const Scene &scene, float3 d) {
    float3 c = mix(SKY_HORIZON_COLOR, SKY_ZENITH_COLOR, pow(max(d.y, 0.0), SKY_GRADIENT_POWER));
    if (d.y > 0.01) {
        float2 uv = d.xz / (d.y + 0.1) * 0.9 + float2(scene.u.time_s * CLOUD_DRIFT_PER_S, 0.0);
        c = mix(c, CLOUD_COLOR, smoothstep(0.52, 0.78, get_fractal_noise(uv * CLOUD_SCALE)) * 0.85 * smoothstep(0.01, 0.15, d.y));
    }
    float sun_cosine = dot(d, get_sky_direction(scene, scene.u.sun_ideal_world));
    return c + SUN_COLOR * (smoothstep(SUN_DISK_INNER_COSINE, SUN_DISK_OUTER_COSINE, sun_cosine) * SUN_DISK_BRIGHTNESS
        + pow(max(sun_cosine, 0.0), SUN_GLOW_POWER) * SUN_GLOW_BRIGHTNESS);
}

/**
 * @brief `get_ground_color` gives the colour of the {4,5}-tiled ground at a world point.
 * @par Example
 * @code
 * float3 c = get_ground_color(scene, to_world * xh);
 * @endcode
 */
static float3 get_ground_color(thread const Scene &scene, float4 p) {
    int edges[MAX_FOLD_COUNT];
    int fold_count = 0;
    while (fold_count < MAX_FOLD_COUNT) {
        float worst = 0.0;
        int edge = -1;
        for (int e = 0; e < 4; e++) { float d = mdot(p, scene.tile_edges[e]); if (d > worst) { worst = d; edge = e; } }
        if (edge < 0) break;
        p -= 2.0 * worst * scene.tile_edges[edge];
        edges[fold_count] = edge;
        fold_count++;
    }
    float4 center = float4(0.0, 0.0, 0.0, 1.0);
    for (int i = fold_count - 1; i >= 0; i--) { float4 n = scene.tile_edges[edges[i]]; center -= 2.0 * mdot(center, n) * n; }
    float tile_seed = hash_3(floor(center.xyz * 64.0 + 0.5));
    float cell_count = float(scene.u.tile_cell_count);
    float2 cell_position = (p.xz / p.w / scene.tile_klein_half_width * 0.5 + 0.5) * cell_count;
    float2 cell = clamp(floor(cell_position), float2(0.0), float2(cell_count - 1.0));
    bool is_road = cell.x == 0.0 || cell.y == 0.0 || cell.x == cell_count - 1.0 || cell.y == cell_count - 1.0;
    float r = hash_3(float3(cell, tile_seed * 97.0));
    float3 c;
    if (is_road) c = ROAD_COLOR * (0.93 + 0.1 * r);
    else if (r < DIRT_FRACTION) c = DIRT_COLOR;
    else c = (fmod(cell.x + cell.y, 2.0) > 0.5 ? GRASS_DARK_COLOR : GRASS_LIGHT_COLOR) * (0.95 + 0.1 * tile_seed) * (0.95 + 0.08 * r);
    return c * get_block_texture(cell_position, tile_seed);
}

/**
 * @brief `shade_hit` gives the lit colour of the surface a ray hit, before fog.
 * @par Example
 * @code
 * float3 c = shade_hit(scene, row, hit, xh);
 * @endcode
 */
static float3 shade_hit(thread const Scene &scene, int row, Hit hit, float4 xh) {
    float r = scene.u.curvature_radius_m;
    float4 up = to_tangent(xh, fetch(scene, row, TEXEL_GROUND));
    float4 sun = to_tangent(xh, fetch(scene, row, TEXEL_SUN));
    float3 base;
    float4 n;
    float shade = TOP_SHADE;
    if (hit.index == GROUND_HIT) {
        base = get_ground_color(scene, fetch_matrix(scene, row, TEXEL_TO_WORLD) * xh);
        n = up;
    } else {
        int first = TEXEL_BOXES + TEXELS_PER_BOX * hit.index;
        float4 material = fetch(scene, row, first + 3), half_m = fetch(scene, row, first + 4);
        float3 color = fetch(scene, row, first + 5).xyz;
        n = hit.side * to_tangent(xh, fetch(scene, row, first + hit.face));
        int axis_a = hit.face == 0 ? 1 : 0, axis_b = hit.face == 2 ? 1 : 2;
        float2 uv = float2(r * asinh(mdot(xh, fetch(scene, row, first + axis_a))) + half_m[axis_a],
                           r * asinh(mdot(xh, fetch(scene, row, first + axis_b))) + half_m[axis_b]);
        int material_id = int(material.w + 0.5);
        if (material_id == MATERIAL_GLOW) return color * GLOW_BRIGHTNESS * (0.7 + 0.3 * get_block_texture(uv, 0.0));
        base = color * get_block_texture(uv, float(hit.index * 3 + hit.face));
        if (material_id == MATERIAL_GLASS) base = mix(base, GLASS_SHEEN_COLOR, GLASS_SHEEN);
        float up_dot = mdot(n, up);
        shade = up_dot > 0.5 ? TOP_SHADE : (up_dot < -0.5 ? BOTTOM_SHADE : (hit.face == 0 ? X_SIDE_SHADE : Z_SIDE_SHADE));
    }
    return base * shade * (AMBIENT_LIGHT + SUN_LIGHT * max(mdot(n, sun), 0.0));
}

/**
 * @brief `get_segment_distance_squared` gives the squared distance between segments p1-q1 and p2-q2, and
 * sets `s` to where along p1-q1 (0 to 1) it is closest.
 * @par Example
 * @code
 * float s; float d2 = get_segment_distance_squared(a, b, p, q, s);
 * @endcode
 */
static float get_segment_distance_squared(float3 p1, float3 q1, float3 p2, float3 q2, thread float &s) {
    float3 d1 = q1 - p1, d2 = q2 - p2, r = p1 - p2;
    float a = dot(d1, d1), e = dot(d2, d2), f = dot(d2, r), t;
    s = 0.0;
    if (e < 1e-10) { t = 0.0; s = a > 1e-10 ? clamp(-dot(d1, r) / a, 0.0, 1.0) : 0.0; }
    else if (a < 1e-10) { t = clamp(f / e, 0.0, 1.0); }
    else {
        float b = dot(d1, d2), c = dot(d1, r), denominator = a * e - b * b;
        s = denominator > 1e-7 * a * e ? clamp((b * f - c * e) / denominator, 0.0, 1.0) : 0.0;
        t = (b * s + f) / e;
        if (t < 0.0) { t = 0.0; s = clamp(-c / a, 0.0, 1.0); }
        else if (t > 1.0) { t = 1.0; s = clamp((b - c) / a, 0.0, 1.0); }
    }
    float3 w = p1 + d1 * s - p2 - d2 * t;
    return dot(w, w);
}

/**
 * @brief `add_beam_glow` adds how much the laser shows near one straight piece of a ray, from `a` (where
 * the ray had travelled `t0_m`) to `b` (at `t1_m`), in Klein coordinates.
 * @par Example
 * @code
 * add_beam_glow(scene, row, a, b, t0, t1, core, halo);
 * @endcode
 */
static void add_beam_glow(thread const Scene &scene, int row, float4 a, float4 b, float t0_m, float t1_m, thread float &core, thread float &halo) {
    float r = scene.u.curvature_radius_m;
    float3 ka = a.xyz / a.w, kb = b.xyz / b.w;
    float beam_radius = BEAM_RADIUS_M / r;
    float reach = BEAM_REACH * max(beam_radius, scene.u.pixel_angle_rad);
    float3 ab = kb - ka;
    for (int c = 0; c < scene.u.beam_chunk_count; c++) {
        float4 chunk = fetch(scene, row, TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * c);
        float t = clamp(dot(chunk.xyz - ka, ab) / max(dot(ab, ab), 1e-12), 0.0, 1.0);
        if (length(chunk.xyz - ka - ab * t) - chunk.w > reach) continue;
        float4 info = fetch(scene, row, TEXEL_BEAM_CHUNKS + TEXELS_PER_BEAM_CHUNK * c + 1);
        int first = int(info.x + 0.5), count = int(info.y + 0.5);
        float4 q0 = fetch(scene, row, TEXEL_BEAM_POINTS + first);
        for (int j = 1; j < count; j++) {
            float4 q1 = fetch(scene, row, TEXEL_BEAM_POINTS + first + j);
            float s;
            float d2 = get_segment_distance_squared(ka, kb, q0.xyz, q1.xyz, s);
            float t_m = t0_m + s * (t1_m - t0_m);
            float sigma = max(beam_radius, scene.u.pixel_angle_rad * tanh(t_m / r));
            float fade = exp(-q0.w / BEAM_FADE_M);
            core = max(core, fade * exp(-d2 / (sigma * sigma)));
            halo = max(halo, fade * exp(-d2 / (4.8 * sigma * sigma)));
            q0 = q1;
        }
    }
}

/**
 * @brief `get_spot_glow` gives how much the laser's hit spot shows at a hit point.
 * @par Example
 * @code
 * float g = get_spot_glow(scene, row, xh, 4.0);
 * @endcode
 */
static float get_spot_glow(thread const Scene &scene, int row, float4 xh, float t_m) {
    float4 spot = fetch(scene, row, TEXEL_SPOT);
    if (spot.w <= 0.0) return 0.0;
    float r = scene.u.curvature_radius_m;
    float radius = max(SPOT_RADIUS_M / r, SPOT_PIXELS * scene.u.pixel_angle_rad * tanh(t_m / r));
    float3 d = xh.xyz / xh.w - spot.xyz;
    return spot.w * exp(-dot(d, d) / (radius * radius)) * exp(-t_m / BEAM_FADE_M);
}

/**
 * @brief `get_nearest_mouth` gives the index of the mouth nearest a point, and sets `q_nearest` to its q.
 * @par Example
 * @code
 * float q; int k = get_nearest_mouth(scene, row, x, q);
 * @endcode
 */
static int get_nearest_mouth(thread const Scene &scene, int row, float4 x, thread float &q_nearest) {
    int nearest = 0;
    q_nearest = 3.4e38;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float q = get_q(x, fetch(scene, row, TEXEL_MOUTHS + k));
        if (q < q_nearest) { q_nearest = q; nearest = k; }
    }
    return nearest;
}

/**
 * @brief `get_jump_entry_m` finds how far a geodesic goes before it enters any mouth's jump sphere (within
 * the uniforms' `jump_m` of its centre), in blocks, or a large distance if it never does.
 * @par Example
 * @code
 * ds_m = min(ds_m, get_jump_entry_m(scene, row, x, v) + JUMP_OVERSHOOT_M);
 * @endcode
 */
static float get_jump_entry_m(thread const Scene &scene, int row, float4 x, float4 v) {
    float k_cosh = 1.0 + 0.5 * scene.q_jump, best = 1.0;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float4 c = fetch(scene, row, TEXEL_MOUTHS + k);
        float a = -mdot(x, c), b = -mdot(v, c), discriminant = k_cosh * k_cosh + b * b - a * a;
        if (b >= 0.0 || discriminant < 0.0) continue;
        float tau = (-a * b - k_cosh * sqrt(discriminant)) / (b * b + k_cosh * k_cosh);
        if (tau > 0.0) best = min(best, tau);
    }
    return best < 1.0 ? atanh(best) * scene.u.curvature_radius_m : 3.4e38;
}

/**
 * @brief `TableEntry` is one (interpolated) jump-table entry: its texels as `write_table_entry` writes them.
 */
struct TableEntry {
    float4 base;
    float4 response_in;
    float4 response_out;
    float4 tilt_response;
};

/**
 * @brief `read_table_entry` reads entry `i` of a jump table row, blended with entry `i + 1` by `f` unless one
 * crosses the throat and the other doesn't.
 * @par Example
 * @code
 * TableEntry entry = read_table_entry(row, 12, 0.3);
 * @endcode
 */
static TableEntry read_table_entry(device const float4 *row, int i, float f) {
    device const float4 *low = row + i * TEXELS_PER_TABLE_ENTRY, *high = low + TEXELS_PER_TABLE_ENTRY;
    if (low[0].w != high[0].w) {
        device const float4 *nearest = f < 0.5 ? low : high;
        return {nearest[0], nearest[1], nearest[2], nearest[3]};
    }
    return {mix(low[0], high[0], f), mix(low[1], high[1], f), mix(low[2], high[2], f), mix(low[3], high[3], f)};
}

/**
 * @brief `blend_table_entries` blends two jump-table entries by `g` unless one crosses the throat and the other
 * doesn't.
 * @par Example
 * @code
 * TableEntry entry = blend_table_entries(first, second, 0.4);
 * @endcode
 */
static TableEntry blend_table_entries(TableEntry first, TableEntry second, float g) {
    if (first.base.w != second.base.w) return g < 0.5 ? first : second;
    return {mix(first.base, second.base, g), mix(first.response_in, second.response_in, g), mix(first.response_out, second.response_out, g),
            mix(first.tilt_response, second.tilt_response, g)};
}

/**
 * @brief `look_up_jump_table` reads a jump table for a ray at a mouth's centre `c`, point `x` and direction
 * `v`, and sets the plane's axes: `a` from the centre toward `x`, and `b` toward the ray's sideways motion. The
 * table has `row_count` rows of `size` entries spread over angles 0 to `angle_range` from straight in; the ray
 * reads row `row_position` (blending the two nearest rows).
 * @return The entry, with base.w = 2 if the ray's angle is past `angle_range`.
 * @par Example
 * @code
 * TableEntry entry = look_up_jump_table(scene.throat_table, THROAT_TABLE_SIZE, 1, 0.0, M_PI_2_F, c, x, v, a, b);
 * @endcode
 */
static TableEntry look_up_jump_table(device const float4 *table, int size, int row_count, float row_position, float angle_range,
                                     float4 c, float4 x, float4 v, thread float4 &a, thread float4 &b) {
    float q = get_q(x, c);
    float cosh_rho = 1.0 + 0.5 * q, sinh_rho = sqrt(q * (1.0 + 0.25 * q));
    float4 radial = (cosh_rho * x - c) / sinh_rho;
    a = (x - cosh_rho * c) / sinh_rho;
    float along = mdot(v, radial);
    float4 sideways = v - along * radial;
    float sideways_length = sqrt(max(mdot(sideways, sideways), 0.0));
    if (sideways_length > 1e-6) {
        b = sideways / sideways_length;
    } else {
        b = abs(a.x) < 0.9 ? float4(1.0, 0.0, 0.0, 0.0) : float4(0.0, 1.0, 0.0, 0.0);
        b = b + mdot(b, c) * c - mdot(b, a) * a;
        b = b * rsqrt(mdot(b, b));
    }
    float angle = atan2(sideways_length, -along);
    if (angle >= angle_range) return {float4(0.0, 0.0, 0.0, 2.0), float4(0.0), float4(0.0), float4(0.0)};
    float position = angle / angle_range * float(size - 1);
    int i = min(int(position), size - 2);
    float f = position - float(i);
    int row_low = min(int(row_position), max(row_count - 2, 0)), row_high = min(row_low + 1, row_count - 1);
    float g = clamp(row_position - float(row_low), 0.0, 1.0);
    TableEntry first = read_table_entry(table + row_low * size * TEXELS_PER_TABLE_ENTRY, i, f);
    TableEntry second = read_table_entry(table + row_high * size * TEXELS_PER_TABLE_ENTRY, i, f);
    return blend_table_entries(first, second, g);
}

/**
 * @brief `get_plane_normal` gives a unit vector at `c` at right angles to the tangent vectors `a` and `b` there.
 * @par Example
 * @code
 * float4 n = get_plane_normal(c, a, b);
 * @endcode
 */
static float4 get_plane_normal(float4 c, float4 a, float4 b) {
    float4 best = float4(0.0);
    for (int k = 0; k < 3; k++) {
        float4 w = float4(0.0);
        w[k] = 1.0;
        w = w + mdot(w, c) * c - mdot(w, a) * a - mdot(w, b) * b;
        if (mdot(w, w) > mdot(best, best)) best = w;
    }
    return best * rsqrt(mdot(best, best));
}

/**
 * @brief `get_others_gradient` gives the gradient of ln n at `x` from every mouth except `skip`, unprojected.
 * @par Example
 * @code
 * float4 g = get_others_gradient(scene, row, c, nearest);
 * @endcode
 */
static float4 get_others_gradient(thread const Scene &scene, int row, float4 x, int skip) {
    float4 g = float4(0.0);
    for (int k = 0; k < scene.u.mouth_count; k++) {
        if (k != skip) g += get_mouth_gradient(scene, x, fetch(scene, row, TEXEL_MOUTHS + k));
    }
    return g;
}

/**
 * @brief `jump_through_mouth` carries a ray inside the nearest mouth's jump sphere straight to where it leaves
 * the sphere (through the throat into the partner mouth's row, or back out), from a jump table: the field
 * inside is the mouth's alone, which is the same about every axis through its centre, so the path depends only
 * on the angle. The other mouths' bending inside is added to first order: their gradient at the centre, on each
 * side of the throat, times the table's response to a uniform field. The laser glow inside is taken along
 * straight chords through the centre.
 * @return Whether the ray jumped (not if its angle is past the table's); `is_lost` is set if the table says
 * the ray never leaves.
 * @par Example
 * @code
 * if (jump_through_mouth(scene, scene.throat_table, THROAT_TABLE_SIZE, 1, 0.0, M_PI_2_F, row, x, v, travelled_m, core, halo, has_beam, is_lost)) { ... }
 * @endcode
 */
static bool jump_through_mouth(thread const Scene &scene, device const float4 *table, int size, int row_count, float row_position,
                               float angle_range, thread int &row, thread float4 &x, thread float4 &v, thread float &travelled_m,
                               thread float &core, thread float &halo, bool has_beam, thread bool &is_lost) {
    float q_nearest;
    int nearest = get_nearest_mouth(scene, row, x, q_nearest);
    float4 c = fetch(scene, row, TEXEL_MOUTHS + nearest), a, b;
    TableEntry entry = look_up_jump_table(table, size, row_count, row_position, angle_range, c, x, v, a, b);
    if (entry.base.w > 1.5) return false;
    if (entry.base.w < 0.0) { is_lost = true; return true; }
    float4 n = get_plane_normal(c, a, b), g_in = get_others_gradient(scene, row, c, nearest);
    float4 c_out = c, a_out = a, b_out = b, n_out = n;
    float3 field_out = float3(0.0);
    int row_out = row;
    if (entry.base.w > 0.5) {
        float4x4 gluing = fetch_matrix(scene, row, TEXEL_GLUINGS + TEXELS_PER_GLUING * nearest);
        c_out = float4(0.0, 0.0, 0.0, 1.0);
        a_out = gluing * a;
        b_out = gluing * b;
        n_out = gluing * n;
        row_out = scene.u.next_rows[nearest];
        float unused_q;
        float4 g_out = get_others_gradient(scene, row_out, c_out, get_nearest_mouth(scene, row_out, c_out, unused_q));
        field_out = float3(mdot(g_out, a_out), mdot(g_out, b_out), mdot(g_out, n_out));
    }
    float3 field_in = float3(mdot(g_in, a), mdot(g_in, b), mdot(g_in, n));
    float angle = entry.base.x + dot(entry.response_in.xy, field_in.xy) + dot(entry.response_out.xy, field_out.xy);
    float exit_angle = entry.base.z + dot(entry.response_in.zw, field_in.xy) + dot(entry.response_out.zw, field_out.xy);
    float tilt_position = entry.tilt_response.x * field_in.z + entry.tilt_response.z * field_out.z;
    float tilt_direction = entry.tilt_response.y * field_in.z + entry.tilt_response.w * field_out.z;
    float jump_cosh = 1.0 + 0.5 * scene.q_jump, jump_sinh = sqrt(scene.q_jump * (1.0 + 0.25 * scene.q_jump));
    float4 e = cos(angle) * a_out + sin(angle) * b_out + tilt_position * n_out;
    e = e * rsqrt(mdot(e, e));
    float4 x_out = to_point(jump_cosh * c_out + jump_sinh * e);
    float4 v_out = to_tangent(x_out, cos(exit_angle) * (jump_sinh * c_out + jump_cosh * e) + sin(exit_angle) * (-sin(angle) * a_out + cos(angle) * b_out)
                                     + tilt_direction * n_out);
    if (has_beam) {
        if (entry.base.w > 0.5) {
            add_beam_glow(scene, row, x, c, travelled_m, travelled_m + 0.5 * entry.base.y, core, halo);
            add_beam_glow(scene, row_out, c_out, x_out, travelled_m + 0.5 * entry.base.y, travelled_m + entry.base.y, core, halo);
        } else {
            add_beam_glow(scene, row, x, x_out, travelled_m, travelled_m + entry.base.y, core, halo);
        }
    }
    x = x_out;
    v = v_out;
    row = row_out;
    travelled_m += entry.base.y;
    return true;
}

/**
 * @brief `TableExit` is where a jump-table ray left the sphere: the angle it swept around the centre, its path
 * length in blocks, its exit angle from straight out, its crossing parity (-1 if it never left), and how far its
 * exit point and direction tilted out of the starting plane.
 */
struct TableExit {
    float angle;
    float travelled_m;
    float exit_angle;
    float parity;
    float tilt_position;
    float tilt_direction;
};

/**
 * @brief `get_table_exit_values` gives, in a table ray's frame, the exit angle from straight out and the
 * out-of-plane tilts of a point and direction.
 * @par Example
 * @code
 * float3 values = get_table_exit_values(x, v);
 * @endcode
 */
static float3 get_table_exit_values(float4 x, float4 v) {
    float4 c = float4(0.0, 0.0, 0.0, 1.0);
    float q = get_q(x, c);
    float cosh_rho = 1.0 + 0.5 * q, sinh_rho = sqrt(q * (1.0 + 0.25 * q));
    float4 radial = (cosh_rho * x - c) / sinh_rho, outward = (x - cosh_rho * c) / sinh_rho;
    float in_plane = max(length(outward.xy), 1e-6);
    float4 tangential = float4(-outward.y / in_plane, outward.x / in_plane, 0.0, 0.0);
    return float3(atan2(mdot(v, tangential), mdot(v, radial)), outward.z, v.z);
}

/**
 * @brief `trace_table_ray` traces one jump-table ray: it starts `start_m` blocks from a mouth's centre at
 * `angle_in` from straight in, in the mouth's field plus a uniform extra field (`field_in` before it crosses the
 * throat, `field_out` after, both as vectors at the centre), with midpoint steps of `step_per_nearest` of the
 * distance to the centre (at most `max_step_count`), crossing the throat as `cross_throat` does (into a copy of
 * the same mouth), until it leaves the jump sphere (radius `jump_m`), where its exit is interpolated.
 * @par Example
 * @code
 * TableExit exit = trace_table_ray(u.curvature_radius_m, u.throat_m, u.jump_m, u.jump_m, 0.3, 0.01, 40000, float4(0.0), float4(0.0));
 * @endcode
 */
static TableExit trace_table_ray(float r, float throat_m, float jump_m, float start_m, float angle_in, float step_per_nearest,
                                 int max_step_count, float4 field_in, float4 field_out) {
    float jump_rho = jump_m / r, start_rho = start_m / r;
    float4 c = float4(0.0, 0.0, 0.0, 1.0), a = float4(1.0, 0.0, 0.0, 0.0), b = float4(0.0, 1.0, 0.0, 0.0);
    float4 x = cosh(start_rho) * c + sinh(start_rho) * a;
    float4 v = -cos(angle_in) * (sinh(start_rho) * c + cosh(start_rho) * a) + sin(angle_in) * b;
    float travelled_m = 0.0, angle = 0.0, parity = 0.0;
    float4 x_before = x, v_before = v;
    float rho_before = start_rho, angle_before = 0.0, travelled_before_m = 0.0;
    for (int n = 0; n < max_step_count; n++) {
        float q = get_q(x, c);
        float cosh_rho = 1.0 + 0.5 * q, sinh_rho = sqrt(q * (1.0 + 0.25 * q)), e_rho = cosh_rho + sinh_rho;
        float rho_effective_m = r * sinh_rho * e_rho;
        float4 radial = (cosh_rho * x - c) / sinh_rho, outward = (x - cosh_rho * c) / sinh_rho;
        float along = mdot(v, radial);
        if (rho_effective_m < throat_m) {
            float new_rho = 0.5 * log(1.0 + 2.0 * throat_m * throat_m / rho_effective_m / r);
            x = to_point(cosh(new_rho) * c + sinh(new_rho) * outward);
            v = to_tangent(x, v - along * radial - along * (sinh(new_rho) * c + cosh(new_rho) * outward));
            parity = 1.0 - parity;
            x_before = x;
            v_before = v;
            rho_before = new_rho;
            continue;
        }
        float rho = get_distance_of_q(q);
        if (rho >= jump_rho && along > 0.0 && n > 0) {
            float f = clamp((jump_rho - rho_before) / max(rho - rho_before, 1e-9), 0.0, 1.0);
            float3 values = mix(get_table_exit_values(x_before, v_before), get_table_exit_values(x, v), f);
            return {mix(angle_before, angle, f), mix(travelled_before_m, travelled_m, f), values.x, parity, values.y, values.z};
        }
        float4 field = parity < 0.5 ? field_in : field_out;
        float ds_m = max(step_per_nearest * rho * r, TABLE_MIN_STEP_PER_STEP * step_per_nearest * throat_m);
        float4 g = get_dln_n_flat(rho_effective_m, throat_m) * e_rho * e_rho * radial + field + mdot(field, x) * x;
        float4 bend = g - mdot(g, v) * v;
        float4 vh = to_tangent(x, v + bend * (0.5 * ds_m));
        float4 xm = x, vm = vh;
        advance_geodesic(xm, vm, 0.5 * ds_m / r);
        float qm = get_q(xm, c);
        float cosh_m = 1.0 + 0.5 * qm, sinh_m = sqrt(qm * (1.0 + 0.25 * qm)), e_m = cosh_m + sinh_m;
        float4 gm = get_dln_n_flat(r * sinh_m * e_m, throat_m) * e_m * e_m * (cosh_m * xm - c) / sinh_m + field + mdot(field, xm) * xm;
        x_before = x;
        v_before = v;
        rho_before = rho;
        angle_before = angle;
        travelled_before_m = travelled_m;
        v = vh;
        advance_geodesic(x, v, ds_m / r);
        v += (gm - mdot(gm, vm) * vm - 0.5 * bend) * ds_m;
        x = to_point(x);
        v = to_tangent(x, v);
        float4 p0 = x_before - (1.0 + 0.5 * get_q(x_before, c)) * c, p1 = x - (1.0 + 0.5 * get_q(x, c)) * c;
        angle += atan2(p0.x * p1.y - p0.y * p1.x, p0.x * p1.x + p0.y * p1.y);
        travelled_m += ds_m;
    }
    return {0.0, 0.0, 0.0, -1.0, 0.0, 0.0};
}

/**
 * @brief `write_table_entry` traces one jump-table entry and its first-order response to a uniform extra field,
 * and writes it as `TEXELS_PER_TABLE_ENTRY` texels: (angle swept, path length, exit angle, parity); the changes
 * of the swept and exit angles per unit field along the plane's two axes before the throat; the same after it;
 * and the exit point's and direction's tilts per unit field along the plane's normal, before and after. The
 * responses come from extra rays with a small field (`TABLE_FIELD_PROBE` per block) and are zero where a
 * probe changes whether the ray crosses.
 * @par Example
 * @code
 * write_table_entry(throat_table + i * TEXELS_PER_TABLE_ENTRY, u.curvature_radius_m, u.throat_m, u.jump_m, u.jump_m, 0.3, 0.01, 40000);
 * @endcode
 */
static void write_table_entry(device float4 *entry, float r, float throat_m, float jump_m, float start_m, float angle_in,
                              float step_per_nearest, int max_step_count) {
    float4 zero = float4(0.0);
    TableExit base = trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, zero, zero);
    entry[0] = float4(base.angle, base.travelled_m, base.exit_angle, base.parity);
    float responses[12];
    for (int k = 0; k < 6; k++) {
        float4 field = float4(0.0);
        field[k % 3] = TABLE_FIELD_PROBE;
        TableExit probe = k < 3 ? trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, field, zero)
                                : trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, zero, field);
        bool is_valid = base.parity >= 0.0 && probe.parity == base.parity;
        float2 change = is_valid ? float2(probe.angle - base.angle, probe.exit_angle - base.exit_angle) / TABLE_FIELD_PROBE : float2(0.0);
        float2 tilt = is_valid ? float2(probe.tilt_position, probe.tilt_direction) / TABLE_FIELD_PROBE : float2(0.0);
        responses[2 * k] = k % 3 == 2 ? tilt.x : change.x;
        responses[2 * k + 1] = k % 3 == 2 ? tilt.y : change.y;
    }
    entry[1] = float4(responses[0], responses[2], responses[1], responses[3]);
    entry[2] = float4(responses[6], responses[8], responses[7], responses[9]);
    entry[3] = float4(responses[4], responses[5], responses[10], responses[11]);
}

/**
 * @brief `build_throat_table` fills the throat table once: rays entering a jump sphere from its edge at angles
 * 0 (straight in) to π/2 (grazing).
 * @par Example
 * @code
 * dispatched by render_metal.rs once, one thread per entry.
 * @endcode
 */
kernel void build_throat_table(constant Uniforms &u [[buffer(1)]], device float4 *throat_table [[buffer(7)]], uint i [[thread_position_in_grid]]) {
    if (int(i) >= THROAT_TABLE_SIZE) return;
    write_table_entry(throat_table + int(i) * TEXELS_PER_TABLE_ENTRY, u.curvature_radius_m, u.throat_m, u.jump_m, u.jump_m,
                      M_PI_2_F * float(i) / float(THROAT_TABLE_SIZE - 1), THROAT_TABLE_STEP_PER_NEAREST, THROAT_TABLE_MAX_STEP_COUNT);
}

/**
 * @brief `get_camera_table_distance_m` gives the eye's distance from a mouth's centre that row `row_position`
 * of the camera table is for: rows are spaced evenly in log distance from just inside the throat to the jump
 * sphere's edge.
 * @par Example
 * @code
 * float distance_m = get_camera_table_distance_m(u, 10.0);
 * @endcode
 */
static float get_camera_table_distance_m(constant Uniforms &u, float row_position) {
    float min_m = CAMERA_TABLE_MIN_PER_THROAT * u.throat_m;
    return min_m * pow(u.jump_m / min_m, row_position / float(CAMERA_TABLE_ROWS - 1));
}

/**
 * @brief `get_camera_table_row` gives the (fractional) camera table row for the eye's distance from its mouth.
 * @par Example
 * @code
 * float row_position = get_camera_table_row(u);
 * @endcode
 */
static float get_camera_table_row(constant Uniforms &u) {
    float min_m = CAMERA_TABLE_MIN_PER_THROAT * u.throat_m;
    return clamp(log(u.camera_distance_m / min_m) / log(u.jump_m / min_m) * float(CAMERA_TABLE_ROWS - 1), 0.0, float(CAMERA_TABLE_ROWS - 1));
}

/**
 * @brief `build_camera_table` fills the camera table once: for `CAMERA_TABLE_ROWS` distances of the eye from a
 * mouth's centre inside its jump sphere, rays leaving the eye at angles 0 (straight in toward the mouth) to π
 * (straight out).
 * @par Example
 * @code
 * dispatched by render_metal.rs once, one thread per entry (angle, row).
 * @endcode
 */
kernel void build_camera_table(constant Uniforms &u [[buffer(1)]], device float4 *camera_table [[buffer(8)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= CAMERA_TABLE_SIZE || int(id.y) >= CAMERA_TABLE_ROWS) return;
    write_table_entry(camera_table + (int(id.y) * CAMERA_TABLE_SIZE + int(id.x)) * TEXELS_PER_TABLE_ENTRY, u.curvature_radius_m, u.throat_m,
                      u.jump_m, get_camera_table_distance_m(u, float(id.y)), M_PI_F * float(id.x) / float(CAMERA_TABLE_SIZE - 1),
                      CAMERA_TABLE_STEP_PER_NEAREST, CAMERA_TABLE_MAX_STEP_COUNT);
}

/**
 * @brief `make_scene` wraps one frame's rows, uniforms and jump tables with the values every ray reuses.
 * @par Example
 * @code
 * Scene scene = make_scene(rows, u, throat_table, camera_table);
 * @endcode
 */
static Scene make_scene(device const float4 *rows, constant Uniforms &u, device const float4 *throat_table, device const float4 *camera_table) {
    float inradius = acosh(cos(M_PI_F / TILES_PER_VERTEX) / sin(M_PI_F / TILE_SIDE_COUNT));
    float tile_cosh = cosh(inradius), tile_sinh = sinh(inradius);
    float half_cutoff = sinh(0.5 * u.bend_cutoff_m / u.curvature_radius_m);
    float half_near_zone = sinh(0.5 * NEAR_ZONE_PER_THROAT * u.throat_m / u.curvature_radius_m);
    float half_rk4_start = sinh(0.5 * RK4_ZONE_START_PER_THROAT * u.throat_m / u.curvature_radius_m);
    float half_jump = sinh(0.5 * u.jump_m / u.curvature_radius_m);
    float half_throat = sinh(0.25 * log(1.0 + 2.0 * u.throat_m / u.curvature_radius_m));
    return {rows, u, throat_table, camera_table, 4.0 * half_cutoff * half_cutoff, 4.0 * half_throat * half_throat, 4.0 * half_near_zone * half_near_zone,
        4.0 * half_rk4_start * half_rk4_start, 4.0 * half_jump * half_jump, tanh(inradius),
        {float4(tile_cosh, 0.0, 0.0, tile_sinh), float4(-tile_cosh, 0.0, 0.0, tile_sinh), float4(0.0, 0.0, tile_cosh, tile_sinh), float4(0.0, 0.0, -tile_cosh, tile_sinh)}};
}

/**
 * @brief `get_ndc` gives a pixel position's normalized device coordinates (-1 to 1, y up), for any pixel
 * position, inside the image or not.
 * @par Example
 * @code
 * float2 ndc = get_ndc(scene, float2(id) + 0.5);
 * @endcode
 */
static float2 get_ndc(thread const Scene &scene, float2 pixel) {
    return float2(2.0 * pixel.x / float(scene.u.width_px) - 1.0, 1.0 - 2.0 * pixel.y / float(scene.u.height_px));
}

/**
 * @brief `trace_ray` follows the light ray through a point of the screen backwards from the eye, as
 * `render_cpu.rs`'s `render_pixel` does, and says where it ended.
 * @par Example
 * @code
 * RayEnd end = trace_ray(scene, get_ndc(scene, float2(id) + 0.5));
 * @endcode
 */
static RayEnd trace_ray(thread const Scene &scene, float2 ndc) {
    constant Uniforms &u = scene.u;
    float r = u.curvature_radius_m, max_travel_m = MAX_TRAVEL_PER_R * r;
    float4 x = float4(0.0, 0.0, 0.0, 1.0);
    float4 v = float4(normalize(float3(ndc.x * u.tan_half_fov * u.aspect, ndc.y * u.tan_half_fov, -1.0)), 0.0);
    int row = 0;
    float travelled_m = 0.0, core = 0.0, halo = 0.0;
    BoxCache cache;
    reset_box_cache(scene, cache, x, 0.0);
    bool has_beam = u.beam_chunk_count > 0, was_outside_jump = false;
    RayEnd end = {float4(0.0), 0.0, 0.0, 0.0, LOST_HIT, 0, 0, 1.0};
    if (u.camera_mouth >= 0) {
        bool is_lost = false;
        jump_through_mouth(scene, scene.camera_table, CAMERA_TABLE_SIZE, CAMERA_TABLE_ROWS, get_camera_table_row(u), M_PI_F, row, x, v,
                           travelled_m, core, halo, has_beam, is_lost);
        if (is_lost) return end;
        reset_box_cache(scene, cache, x, travelled_m);
    }
    for (int step = 0; step < MAX_STEP_COUNT; step++) {
        float q_nearest;
        int nearest;
        float4 g_others;
        float4 bend = get_bend_split(scene, row, x, v, q_nearest, nearest, g_others);
        if (q_nearest < scene.q_throat && cross_throat(scene, row, x, v)) {
            reset_box_cache(scene, cache, x, travelled_m);
            bend = get_bend_split(scene, row, x, v, q_nearest, nearest, g_others);
        }
        if (q_nearest < scene.q_jump && was_outside_jump) {
            bool is_lost = false;
            if (jump_through_mouth(scene, scene.throat_table, THROAT_TABLE_SIZE, 1, 0.0, M_PI_2_F, row, x, v, travelled_m, core, halo, has_beam, is_lost)) {
                if (is_lost) break;
                reset_box_cache(scene, cache, x, travelled_m);
                bend = get_bend_split(scene, row, x, v, q_nearest, nearest, g_others);
            }
        }
        was_outside_jump = q_nearest >= scene.q_jump;
        bool is_far = q_nearest > scene.q_cutoff;
        float4 vh = v;
        float tau_max, ds_m;
        bool is_near = q_nearest < scene.q_near_zone && q_nearest > scene.q_rk4_start;
        float4 x_next = x, v_next = v;
        if (is_far) {
            tau_max = get_cutoff_entry_tau(scene, row, x, v);
            ds_m = tau_max < 1.0 ? atanh(tau_max) * r : max_travel_m;
        } else if (is_near) {
            ds_m = max(get_distance_of_q(q_nearest) * r * RK4_STEP_PER_NEAREST, MIN_STEP_M);
            if (q_nearest >= scene.q_jump) ds_m = min(ds_m, get_jump_entry_m(scene, row, x, v) + JUMP_OVERSHOOT_M);
            step_rk4(scene, row, x_next, v_next, bend, g_others, fetch(scene, row, TEXEL_MOUTHS + nearest), ds_m);
            float q_next;
            get_nearest_mouth(scene, row, x_next, q_next);
            if (q_nearest >= scene.q_jump && q_next < scene.q_jump) {
                float rho_now = get_distance_of_q(q_nearest), rho_next = get_distance_of_q(q_next), rho_jump = get_distance_of_q(scene.q_jump);
                ds_m = ds_m * clamp((rho_now - rho_jump) / max(rho_now - rho_next, 1e-6), 0.0, 1.0) + JUMP_OVERSHOOT_M;
                x_next = x;
                v_next = v;
                step_rk4(scene, row, x_next, v_next, bend, g_others, fetch(scene, row, TEXEL_MOUTHS + nearest), ds_m);
            }
            vh = to_tangent(x, x_next);
            tau_max = tanh(get_distance_of_q(get_q(x, x_next)));
        } else {
            float nearest_m = get_distance_of_q(q_nearest) * r;
            ds_m = max(nearest_m * (q_nearest < scene.q_near_zone ? NEAR_STEP_PER_NEAREST : FAR_STEP_PER_NEAREST), MIN_STEP_M);
            vh = to_tangent(x, v + bend * (0.5 * ds_m));
            tau_max = tanh(ds_m / r);
        }
        Hit hit = {tau_max, NO_HIT, 0, 1.0};
        float4 ground = fetch(scene, row, TEXEL_GROUND);
        float ground_a = mdot(x, ground), ground_b = mdot(vh, ground);
        if (ground_a > 0.0 && ground_b < 0.0 && -ground_a / ground_b < hit.tau) { hit.tau = -ground_a / ground_b; hit.index = GROUND_HIT; }
        test_boxes(scene, row, x, vh, travelled_m, ds_m, hit, cache);
        if (hit.index != NO_HIT) {
            float4 point = to_point(x + hit.tau * vh);
            float end_m = travelled_m + atanh(hit.tau) * r;
            if (has_beam) add_beam_glow(scene, row, x, point, travelled_m, end_m, core, halo);
            end = {point, end_m, core, halo, hit.index, row, hit.face, hit.side};
            return end;
        }
        if (tau_max >= 1.0 || travelled_m > max_travel_m) {
            if (has_beam) add_beam_glow(scene, row, x, x + vh, travelled_m, travelled_m + max_travel_m, core, halo);
            float3 direction = get_sky_direction(scene, fetch_matrix(scene, row, TEXEL_TO_WORLD) * (x + vh));
            end = {float4(direction, 0.0), travelled_m, core, halo, SKY_HIT, row, 0, 1.0};
            return end;
        }
        float4 x_before = x;
        if (is_far) {
            advance_geodesic(x, v, atanh(tau_max));
        } else if (is_near) {
            x = x_next;
            v = v_next;
        } else {
            float4 xm = x, vm = vh;
            advance_geodesic(xm, vm, 0.5 * ds_m / r);
            float unused_q;
            float4 bend_middle = get_bend(scene, row, xm, vm, unused_q);
            v = vh;
            advance_geodesic(x, v, ds_m / r);
            v += (bend_middle - 0.5 * bend) * ds_m;
        }
        x = to_point(x);
        v = to_tangent(x, v);
        if (has_beam) add_beam_glow(scene, row, x_before, x, travelled_m, travelled_m + ds_m, core, halo);
        travelled_m += ds_m;
    }
    end.core = core;
    end.halo = halo;
    return end;
}

/**
 * @brief `shade_ray_end` gives the final colour of a ray from where it ended: the lit surface with fog and
 * the laser's spot, or the sky, with the laser's glow and the highlight roll-off.
 * @par Example
 * @code
 * float3 color = shade_ray_end(scene, trace_ray(scene, ndc));
 * @endcode
 */
static float3 shade_ray_end(thread const Scene &scene, RayEnd end) {
    float3 color = LOST_COLOR;
    float core = end.core, halo = end.halo;
    if (end.kind == SKY_HIT) {
        color = get_sky_color(scene, end.point.xyz);
    } else if (end.kind != LOST_HIT) {
        Hit hit = {0.0, end.kind, end.face, end.side};
        color = shade_hit(scene, end.row, hit, end.point);
        if (scene.u.fog_m > 0.0) color = mix(HAZE_COLOR, color, exp(-end.end_m / scene.u.fog_m));
        float spot = get_spot_glow(scene, end.row, end.point, end.end_m);
        halo = max(halo, spot);
        core = max(core, spot * spot);
    }
    color = mix(color, LASER_HALO_COLOR, clamp(0.9 * halo, 0.0, 1.0)) + LASER_CORE_COLOR * core;
    color = color / (1.0 + max(color - 1.0, 0.0) * HIGHLIGHT_ROLLOFF);
    return clamp(color, 0.0, 1.0);
}

/**
 * @brief `trace_pixels` traces every pixel's ray and writes its colour.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per pixel.
 * @endcode
 */
kernel void trace_pixels(device const float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]], device const float4 *throat_table [[buffer(7)]],
                         device const float4 *camera_table [[buffer(8)]],
                         texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    image.write(float4(shade_ray_end(scene, trace_ray(scene, get_ndc(scene, float2(id) + 0.5))), 1.0), id);
}

/**
 * @brief `get_sample_columns` gives how many samples a row of the sample grid has: one per even pixel
 * column, and one more past the image's edge, so every pixel lies between samples.
 * @par Example
 * @code
 * int columns = get_sample_columns(u);
 * @endcode
 */
static int get_sample_columns(constant Uniforms &u) { return u.width_px / 2 + 2; }

/**
 * @brief `is_same_surface` says whether two samples ended on the same surface, in the same row, without
 * laser glow, so the pixels between them can be interpolated.
 * @par Example
 * @code
 * if (!is_same_surface(a, b)) return false;
 * @endcode
 */
static bool is_same_surface(RayEnd a, RayEnd b) {
    return a.kind == b.kind && a.row == b.row && a.face == b.face && a.side == b.side && b.kind != LOST_HIT
        && b.halo < GLOW_FLOOR && b.core < GLOW_FLOOR;
}

/**
 * @brief `is_line_smooth` says whether four samples in a line map screen to scene nearly linearly between the
 * middle two: the second differences, which are 8 times the midpoint's interpolation error, stay under `ratio`
 * of the first difference. For samples `h` pixels apart that bounds the error by `ratio` × h / 8 pixels.
 * @par Example
 * @code
 * bool is_smooth = is_line_smooth(p0, p1, p2, p3, SMOOTH_SECOND_DIFFERENCE_PER_FIRST);
 * @endcode
 */
static bool is_line_smooth(float4 p0, float4 p1, float4 p2, float4 p3, float ratio) {
    float limit = ratio * length(p2 - p1);
    return length(p0 - 2.0 * p1 + p2) <= limit && length(p1 - 2.0 * p2 + p3) <= limit;
}

/**
 * @brief `is_cell_smooth` says whether the cell between samples (i, j) and (i + 1, j + 1) of a grid
 * `columns` × `grid_rows` can be interpolated: the twelve samples around it (a 4×4 block without its corners)
 * ended on the corner's surface and map nearly linearly along the cell's two rows and two columns. Cells at the
 * grid's border never are.
 * @par Example
 * @code
 * bool is_smooth = is_cell_smooth(samples, columns, sample_rows, i, j, SMOOTH_SECOND_DIFFERENCE_PER_FIRST);
 * @endcode
 */
static bool is_cell_smooth(device const RayEnd *grid, int columns, int grid_rows, int i, int j, float ratio) {
    if (i < 1 || j < 1 || i + 2 >= columns || j + 2 >= grid_rows) return false;
    RayEnd corner = grid[j * columns + i];
    float4 p[4][4];
    for (int b = 0; b < 4; b++) {
        for (int a = 0; a < 4; a++) {
            if ((a == 0 || a == 3) && (b == 0 || b == 3)) continue;
            RayEnd s = grid[(j - 1 + b) * columns + (i - 1 + a)];
            if (!is_same_surface(corner, s)) return false;
            p[b][a] = s.point;
        }
    }
    return is_line_smooth(p[1][0], p[1][1], p[1][2], p[1][3], ratio) && is_line_smooth(p[2][0], p[2][1], p[2][2], p[2][3], ratio)
        && is_line_smooth(p[0][1], p[1][1], p[2][1], p[3][1], ratio) && is_line_smooth(p[0][2], p[1][2], p[2][2], p[3][2], ratio);
}

/**
 * @brief `interpolate_ray_end` blends four samples around a point of a cell (`wx` and `wy` from 0 to 1 along
 * it) into the ray end there: the hit point (or sky direction) and distance blend, and the rest is the first
 * sample's.
 * @par Example
 * @code
 * RayEnd end = interpolate_ray_end(a, b, c, d, 0.5, 0.0);
 * @endcode
 */
static RayEnd interpolate_ray_end(RayEnd a, RayEnd b, RayEnd c, RayEnd d, float wx, float wy) {
    RayEnd end = a;
    float4 point = mix(mix(a.point, b.point, wx), mix(c.point, d.point, wx), wy);
    end.point = a.kind == SKY_HIT ? float4(normalize(point.xyz), 0.0) : to_point(point);
    end.end_m = mix(mix(a.end_m, b.end_m, wx), mix(c.end_m, d.end_m, wx), wy);
    return end;
}

/**
 * @brief `get_coarse_columns` gives how many samples a row of the coarse grid has: one per fourth pixel column,
 * and enough past the image's edge to cover every sample of the fine grid.
 * @par Example
 * @code
 * int coarse_columns = get_coarse_columns(u);
 * @endcode
 */
static int get_coarse_columns(constant Uniforms &u) { return u.width_px / 4 + 3; }

/**
 * @brief `trace_coarse` traces the rays of every fourth pixel each way (coarse sample (i, j) is pixel (4i, 4j)),
 * the first pass of adaptive sampling, and starts this frame's lists empty.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per coarse sample.
 * @endcode
 */
kernel void trace_coarse(device const float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]], device const float4 *throat_table [[buffer(7)]],
                         device const float4 *camera_table [[buffer(8)]], device RayEnd *coarse [[buffer(9)]],
                         device atomic_uint *refine_count [[buffer(5)]], device atomic_uint *fill_count [[buffer(12)]],
                         uint2 id [[thread_position_in_grid]]) {
    int columns = get_coarse_columns(u), coarse_rows = u.height_px / 4 + 3;
    if (id.x == 0 && id.y == 0) {
        atomic_store_explicit(refine_count, 0, memory_order_relaxed);
        atomic_store_explicit(fill_count, 0, memory_order_relaxed);
    }
    if (int(id.x) >= columns || int(id.y) >= coarse_rows) return;
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    coarse[int(id.y) * columns + int(id.x)] = trace_ray(scene, get_ndc(scene, 4.0 * float2(id) + 0.5));
}

/**
 * @brief `fill_samples` fills the fine grid (sample (i, j) is pixel (2i, 2j)), the second pass of adaptive
 * sampling: a sample on the coarse grid copies it, one inside a smooth coarse cell (with half the fine grid's
 * threshold, for the same error in pixels) interpolates the cell's corners, and any other is listed for
 * `trace_listed_samples`.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per fine sample.
 * @endcode
 */
kernel void fill_samples(constant Uniforms &u [[buffer(1)]], device RayEnd *samples [[buffer(2)]], device const RayEnd *coarse [[buffer(9)]],
                         device uint *fill_list [[buffer(11)]], device atomic_uint *fill_count [[buffer(12)]],
                         uint2 id [[thread_position_in_grid]]) {
    int columns = get_sample_columns(u), sample_rows = u.height_px / 2 + 2;
    int coarse_columns = get_coarse_columns(u), coarse_rows = u.height_px / 4 + 3;
    int i = int(id.x), j = int(id.y);
    if (i >= columns || j >= sample_rows) return;
    int ci = i / 2, cj = j / 2, di = i & 1, dj = j & 1;
    if (di + dj == 0) {
        samples[j * columns + i] = coarse[cj * coarse_columns + ci];
    } else if (is_cell_smooth(coarse, coarse_columns, coarse_rows, ci, cj, 0.5 * SMOOTH_SECOND_DIFFERENCE_PER_FIRST)) {
        int k = cj * coarse_columns + ci;
        samples[j * columns + i] = interpolate_ray_end(coarse[k], coarse[k + di], coarse[k + dj * coarse_columns], coarse[k + dj * coarse_columns + di],
                                                       0.5 * float(di), 0.5 * float(dj));
    } else {
        fill_list[atomic_fetch_add_explicit(fill_count, 1, memory_order_relaxed)] = uint(j * columns + i);
    }
}

/**
 * @brief `prepare_fill` sets the thread-group count of `trace_listed_samples`'s indirect dispatch from how many
 * samples `fill_samples` listed.
 * @par Example
 * @code
 * dispatched by render_metal.rs as a single thread.
 * @endcode
 */
kernel void prepare_fill(device atomic_uint *fill_count [[buffer(12)]], device uint *dispatch_groups [[buffer(13)]]) {
    uint count = atomic_load_explicit(fill_count, memory_order_relaxed);
    dispatch_groups[0] = (count + REFINE_GROUP_SIZE - 1) / REFINE_GROUP_SIZE;
    dispatch_groups[1] = 1;
    dispatch_groups[2] = 1;
}

/**
 * @brief `trace_listed_samples` traces the fine samples `fill_samples` couldn't interpolate, packed densely.
 * @par Example
 * @code
 * dispatched by render_metal.rs indirectly, one thread per listed sample.
 * @endcode
 */
kernel void trace_listed_samples(device const float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]], device const float4 *throat_table [[buffer(7)]],
                                 device const float4 *camera_table [[buffer(8)]], device RayEnd *samples [[buffer(2)]],
                                 device const uint *fill_list [[buffer(11)]], device atomic_uint *fill_count [[buffer(12)]],
                                 uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(fill_count, memory_order_relaxed)) return;
    int columns = get_sample_columns(u), k = int(fill_list[t]);
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    samples[k] = trace_ray(scene, get_ndc(scene, 2.0 * float2(k % columns, k / columns) + 0.5));
}

/**
 * @brief `classify_cells` marks each cell between four samples smooth (1) when the twelve samples around it
 * ended on one surface and map nearly linearly, the second pass of adaptive sampling, and lists every other
 * cell that covers pixels, for `refine_pixels`. Cells at the grid's border are never smooth.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per cell (sample (i, j) to (i + 1, j + 1)).
 * @endcode
 */
kernel void classify_cells(constant Uniforms &u [[buffer(1)]], device const RayEnd *samples [[buffer(2)]],
                           device uchar *smooth_cells [[buffer(3)]], device uint *refine_cells [[buffer(4)]],
                           device atomic_uint *refine_count [[buffer(5)]], uint2 id [[thread_position_in_grid]]) {
    int columns = get_sample_columns(u), sample_rows = u.height_px / 2 + 2;
    int i = int(id.x), j = int(id.y);
    if (i >= columns || j >= sample_rows) return;
    uchar is_smooth = is_cell_smooth(samples, columns, sample_rows, i, j, SMOOTH_SECOND_DIFFERENCE_PER_FIRST) ? 1 : 0;
    smooth_cells[j * columns + i] = is_smooth;
    if (is_smooth == 0 && 2 * i < u.width_px && 2 * j < u.height_px) {
        refine_cells[atomic_fetch_add_explicit(refine_count, 1, memory_order_relaxed)] = uint(j * columns + i);
    }
}

/**
 * @brief `prepare_refinement` sets the thread-group count of `refine_pixels`'s indirect dispatch from how many
 * cells `classify_cells` listed.
 * @par Example
 * @code
 * dispatched by render_metal.rs as a single thread.
 * @endcode
 */
kernel void prepare_refinement(device atomic_uint *refine_count [[buffer(5)]], device uint *dispatch_groups [[buffer(6)]]) {
    uint pixel_count = atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL;
    dispatch_groups[0] = (pixel_count + REFINE_GROUP_SIZE - 1) / REFINE_GROUP_SIZE;
    dispatch_groups[1] = 1;
    dispatch_groups[2] = 1;
}

/**
 * @brief `refine_pixels` traces the three odd pixels of each listed cell, the last pass of adaptive sampling,
 * packed densely so that the GPU's thread groups don't wait on the few pixels that need a full trace.
 * @par Example
 * @code
 * dispatched by render_metal.rs indirectly, one thread per listed pixel.
 * @endcode
 */
kernel void refine_pixels(device const float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]], device const float4 *throat_table [[buffer(7)]],
                         device const float4 *camera_table [[buffer(8)]],
                          device const uint *refine_cells [[buffer(4)]], device atomic_uint *refine_count [[buffer(5)]],
                          texture2d<float, access::write> image [[texture(0)]], uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL) return;
    int columns = get_sample_columns(u);
    int cell = int(refine_cells[t / REFINED_PIXELS_PER_CELL]), corner = int(t % REFINED_PIXELS_PER_CELL) + 1;
    uint2 id = uint2(2 * (cell % columns) + (corner & 1), 2 * (cell / columns) + (corner >> 1));
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    image.write(float4(shade_ray_end(scene, trace_ray(scene, get_ndc(scene, float2(id) + 0.5))), 1.0), id);
}

/**
 * @brief `resolve_pixels` writes the pixels that need no new ray, the third pass of adaptive sampling: an even
 * pixel takes its sample, and a pixel in a smooth cell interpolates the cell's corner samples and shades the
 * result. `refine_pixels` writes the rest.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per pixel.
 * @endcode
 */
kernel void resolve_pixels(device const float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]], device const float4 *throat_table [[buffer(7)]],
                         device const float4 *camera_table [[buffer(8)]],
                           device const RayEnd *samples [[buffer(2)]], device const uchar *smooth_cells [[buffer(3)]],
                           texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    int columns = get_sample_columns(u);
    int i = int(id.x) / 2, j = int(id.y) / 2, di = int(id.x) & 1, dj = int(id.y) & 1;
    RayEnd end = samples[j * columns + i];
    if (di + dj > 0) {
        if (smooth_cells[j * columns + i] != 0) {
            end = interpolate_ray_end(end, samples[j * columns + i + di], samples[(j + dj) * columns + i], samples[(j + dj) * columns + i + di],
                                      0.5 * float(di), 0.5 * float(dj));
        } else {
            return;
        }
    }
    image.write(float4(shade_ray_end(scene, end), 1.0), id);
}
