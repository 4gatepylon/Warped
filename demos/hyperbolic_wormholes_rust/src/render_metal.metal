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

constant float OVERLAY_TITLE_LEVEL = 0.75;
constant float OVERLAY_TEXT_LEVEL = 0.25;
constant float4 OVERLAY_TITLE_COLOR = float4(1.0, 0.35, 0.3, 1.0);
constant float4 OVERLAY_TEXT_COLOR = float4(0.92, 0.94, 0.98, 1.0);
constant float3 HIT_RED_COLOR = float3(0.85, 0.05, 0.03);
constant float HIT_EDGE_REACH_FRAC = 0.7;
constant int HIT_BLUR_TAP_COUNT = 8;
constant float3 WAND_LIGHT_DIRECTION = float3(-0.42, 0.78, 0.46);
constant float WAND_AMBIENT = 0.3;
constant float WAND_SPECULAR = 0.45;
constant float WAND_SHININESS = 28.0;
constant float WAND_EDGE_M = 0.003;
constant float WAND_EDGE_DARKEN = 0.55;
constant float WAND_TIP_GLOW = 1.25;
constant float WAND_HALO_M = 0.012;
constant float WAND_HALO_STRENGTH = 0.6;
constant int WAND_SAMPLES_PER_SIDE = 2;
constant int REVOLVER_WAND = 2;
constant int STRETCH_TAP_COUNT = 12;
constant float STRETCH_ZOOM_GAIN = 1.5;
constant float STRETCH_SMEAR_FRAC = 0.35;
constant float STRETCH_FRINGE_FRAC = 0.03;
constant float3 BLUESHIFT_CHANNEL_NM = float3(610.0, 550.0, 465.0);
constant float BLUESHIFT_INFRARED_NM = 700.0;
constant float BLUESHIFT_ULTRAVIOLET_NM = 400.0;
constant float BLUESHIFT_WALK_SPEED_PER_LIGHT_SPEED = 0.05;
constant float BLUESHIFT_WHITE_FRAC = 0.85;
constant float3 BLUESHIFT_WHITE_COLOR = float3(0.86, 0.93, 1.0);
constant float3 REDSHIFT_WHITE_COLOR = float3(1.0, 0.92, 0.84);
constant int MAX_MINIMAP_MARK_COUNT = 16;
constant int MAX_MINIMAP_LINE_COUNT = 96;
constant float MINIMAP_OPACITY = 0.88;
constant float MINIMAP_RIM_PX = 2.0;
constant float4 MINIMAP_RIM_COLOR = float4(0.9, 0.92, 0.95, 0.9);
constant float3 MINIMAP_OFF_MAP_COLOR = float3(0.03, 0.03, 0.04);
constant float MINIMAP_GRID_M = 10.0;
constant float MINIMAP_GRID_HALF_WIDTH_PX = 0.6;
constant float4 MINIMAP_GRID_COLOR = float4(1.0, 1.0, 1.0, 0.16);
constant float MINIMAP_LINE_HALF_WIDTH_PX = 1.2;
constant float MINIMAP_YOU_PX = 14.0;
constant float MINIMAP_YOU_BASE_FRAC = 0.6;
constant float MINIMAP_YOU_SPREAD = 0.5;
constant float3 MINIMAP_YOU_COLOR = float3(1.0, 1.0, 1.0);
constant float MINIMAP_TINY = 1e-6;
constant float RECT_SHADOW_OPACITY = 0.55;
constant float RECT_SHADOW_DROP_PER_BLUR = 0.45;
constant float RECT_GLOW_OPACITY = 0.7;
constant float RECT_GLOW_REACH_PER_RADIUS = 4.0;
constant float RECT_GLOSS_END_FRAC = 0.5;
constant float RECT_GLOSS_SOFT_FRAC = 0.08;
constant float RECT_GLOSS_TOP_OPACITY = 0.6;
constant float RECT_GLOSS_MIDDLE_OPACITY = 0.15;
constant float RECT_GLOSS_INSET_PER_HEIGHT = 0.12;
constant float RECT_EDGE_SOFT_PX = 0.5;
constant int MAX_RECT_PULSE_COUNT = 4;
constant float RECT_PULSE_REACH_PER_HALF_SIZE = 3.0;
constant float RECT_PULSE_EDGE_SOFT_PER_HALF_WIDTH = 0.5;
constant float RECT_PULSE_LIGHT_GAIN = 0.6;

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
constant int TEXEL_MOUTH_FIELDS = __TEXEL_MOUTH_FIELDS__;
constant int TEXELS_PER_MOUTH_FIELD = __TEXELS_PER_MOUTH_FIELD__;
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
constant float RK4_STEP_PER_NEAREST = 0.35;
constant float MAX_RK4_STEP_M = 2.0;
constant float RK4_ZONE_START_PER_THROAT = 3.0;
constant float JUMP_OVERSHOOT_M = 0.0005;
constant int JUMP_ENTRY_ITERATION_COUNT = 3;
constant float CHORD_ANGLE_TOLERANCE_RAD = 1e-4;
constant int MAX_HIT_REFINE_COUNT = 3;
constant float FINE_LEAD_PER_STEP = 0.75;
constant float FINE_TRAIL_PER_STEP = 2.0;
constant int THROAT_TABLE_SIZE = __THROAT_TABLE_SIZE__;
constant int TEXELS_PER_TABLE_ENTRY = __TEXELS_PER_TABLE_ENTRY__;
constant int TEXELS_PER_TIDAL_RESPONSE = 6;
constant float TIDAL_FIELD_PROBE = 0.02;
constant float TIDAL_STEP_PER_R = 0.01;
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
constant float3 TINT_LUMA = float3(0.2126, 0.7152, 0.0722);
constant float THROWN_TINT_FRAC = 0.45;
constant float THROWN_TINT_FLOOR = 0.08;
constant float3 ROAD_COLOR = float3(0.58, 0.58, 0.60);
constant float3 DIRT_COLOR = float3(0.55, 0.42, 0.27);
constant float3 GRASS_DARK_COLOR = float3(0.36, 0.60, 0.25);
constant float3 GRASS_LIGHT_COLOR = float3(0.40, 0.65, 0.28);
constant float3 GLASS_SHEEN_COLOR = float3(0.95);
constant float3 LASER_HALO_COLOR = float3(1.0, 0.06, 0.03);
constant float3 LASER_CORE_COLOR = float3(1.0, 0.92, 0.9);
constant float LASER_HAZE_BRIGHTNESS = 0.45;
constant float LASER_HAZE_WHITENESS = 0.45;
constant float LASER_CORE_BRIGHTNESS = 1.6;
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

/**
 * @brief `Uniforms` matches the Rust `Uniforms` struct field by field (688 bytes). `row_tints[k]` is the colour of
 * the thrown mouth a ray went through to come out in row k + 1, with w = 1 (w = 0 for a map's mouth).
 */
struct Uniforms {
    float4x4 world_to_camera;
    float4 sky_basis[3];
    float4 sun_ideal_world;
    int next_rows[MAX_MOUTH_COUNT];
    float4 row_tints[MAX_MOUTH_COUNT];
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
 * @brief `get_bend_nearest` is `get_bend`, and also gives the nearest mouth's index.
 * @par Example
 * @code
 * float4 bend = get_bend_nearest(scene, row, x, v, q_nearest, nearest);
 * @endcode
 */
static float4 get_bend_nearest(thread const Scene &scene, int row, float4 x, float4 v, thread float &q_nearest, thread int &nearest) {
    float4 g = float4(0.0);
    q_nearest = 3.4e38;
    nearest = 0;
    for (int k = 0; k < scene.u.mouth_count; k++) {
        float4 c = fetch(scene, row, TEXEL_MOUTHS + k);
        float q = get_q(x, c);
        if (q < q_nearest) { q_nearest = q; nearest = k; }
        g += get_mouth_gradient(scene, x, c);
    }
    return g - mdot(g, v) * v;
}

/**
 * @brief `step_rk4` moves a ray `ds_m` blocks with the classical fourth-order Runge-Kutta method, for
 * x' = v / R and v' = x / R + bend(x, v) (a geodesic of H³ plus the bending), starting from the bending
 * `bend` already known at `x`, summing every mouth's bending at every stage, and puts the result back on the
 * hyperboloid.
 * @par Example
 * @code
 * step_rk4(scene, row, x, v, bend, ds_m);
 * @endcode
 */
static void step_rk4(thread const Scene &scene, int row, thread float4 &x, thread float4 &v, float4 bend, float ds_m) {
    float r = scene.u.curvature_radius_m, unused_q;
    float4 kx1 = v / r, kv1 = x / r + bend;
    float4 x2 = x + 0.5 * ds_m * kx1, v2 = v + 0.5 * ds_m * kv1;
    float4 kx2 = v2 / r, kv2 = x2 / r + get_bend(scene, row, x2, v2, unused_q);
    float4 x3 = x + 0.5 * ds_m * kx2, v3 = v + 0.5 * ds_m * kv2;
    float4 kx3 = v3 / r, kv3 = x3 / r + get_bend(scene, row, x3, v3, unused_q);
    float4 x4 = x + ds_m * kx3, v4 = v + ds_m * kv3;
    float4 kx4 = v4 / r, kv4 = x4 / r + get_bend(scene, row, x4, v4, unused_q);
    x = to_point(x + ds_m / 6.0 * (kx1 + 2.0 * kx2 + 2.0 * kx3 + kx4));
    v = to_tangent(x, v + ds_m / 6.0 * (kv1 + 2.0 * kv2 + 2.0 * kv3 + kv4));
}

/**
 * @brief `get_chord_limit_m` gives the longest step whose chord, tested for hits in place of the curved path,
 * puts a hit `travelled_m` blocks from the eye at most `CHORD_ANGLE_TOLERANCE_RAD` off as seen from the eye: a
 * path bending by |bend| per block strays from a chord h long by up to |bend| h² / 8, and the bound
 * |bend| h² / (2 (4L + h)) ≤ ε covers both a hit far away (L ≫ h) and one in the first step (L = 0).
 * @par Example
 * @code
 * float limit_m = get_chord_limit_m(bend, travelled_m + hit_m);
 * @endcode
 */
static float get_chord_limit_m(float4 bend, float travelled_m) {
    float curvature = sqrt(max(mdot(bend, bend), 0.0)), e = CHORD_ANGLE_TOLERANCE_RAD;
    return curvature > 0.0 ? (e + sqrt(e * e + 8.0 * e * curvature * travelled_m)) / curvature : 3.4e38;
}

/**
 * @brief `get_capped_step_m` shortens a step around a hit being refined: up to `fine_start_m` along the ray it
 * lands exactly there, and from there to `fine_end_m` it is at most `fine_step_m`.
 * @par Example
 * @code
 * ds_m = get_capped_step_m(ds_m, travelled_m, fine_start_m, fine_end_m, fine_step_m);
 * @endcode
 */
static float get_capped_step_m(float ds_m, float travelled_m, float fine_start_m, float fine_end_m, float fine_step_m) {
    if (travelled_m < fine_start_m - MIN_STEP_M) return min(ds_m, fine_start_m - travelled_m);
    if (travelled_m < fine_end_m) return max(min(ds_m, fine_step_m), MIN_STEP_M);
    return ds_m;
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
            float q = d2 / (sigma * sigma);
            core = max(core, fade * exp(-q * q * q));
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
    float4 tidal[TEXELS_PER_TIDAL_RESPONSE];
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
    float g = low[0].w != high[0].w ? step(0.5, f) : f;
    TableEntry entry = {mix(low[0], high[0], g), mix(low[1], high[1], g), mix(low[2], high[2], g), mix(low[3], high[3], g)};
    for (int k = 0; k < TEXELS_PER_TIDAL_RESPONSE; k++) entry.tidal[k] = mix(low[4 + k], high[4 + k], g);
    return entry;
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
    if (first.base.w != second.base.w) g = step(0.5, g);
    TableEntry entry = {mix(first.base, second.base, g), mix(first.response_in, second.response_in, g),
                        mix(first.response_out, second.response_out, g), mix(first.tilt_response, second.tilt_response, g)};
    for (int k = 0; k < TEXELS_PER_TIDAL_RESPONSE; k++) entry.tidal[k] = mix(first.tidal[k], second.tidal[k], g);
    return entry;
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
 * @brief `get_others_tidal` gives how the other mouths' gradient changes across mouth `mouth`'s centre within a
 * ray's plane (axes `a` and `b`, normal `n`), from the matrix `build_mouth_fields` stored: `in_plane` is its
 * in-plane part (aa, bb and the symmetric ab component) and `across` is how its component along `n` changes
 * along `a` and `b`.
 * @par Example
 * @code
 * get_others_tidal(scene, row, nearest, a, b, n, tidal_in, across_in);
 * @endcode
 */
static void get_others_tidal(thread const Scene &scene, int row, int mouth, float4 a, float4 b, float4 n, thread float3 &in_plane,
                             thread float2 &across) {
    int first = TEXEL_MOUTH_FIELDS + TEXELS_PER_MOUTH_FIELD * mouth;
    float4x4 change = float4x4(fetch(scene, row, first + 1), fetch(scene, row, first + 2), fetch(scene, row, first + 3), fetch(scene, row, first + 4));
    float4 da = change * a, db = change * b;
    in_plane = float3(mdot(da, a), mdot(db, b), 0.5 * (mdot(db, a) + mdot(da, b)));
    across = float2(mdot(da, n), mdot(db, n));
}

/**
 * @brief `jump_through_mouth` carries a ray inside the nearest mouth's jump sphere straight to where it leaves
 * the sphere (through the throat into the partner mouth's row, or back out), from a jump table: the field
 * inside is the mouth's alone, which is the same about every axis through its centre, so the path depends only
 * on the angle. The other mouths' bending inside is added to first order: their gradient at the centre, on each
 * side of the throat, times the table's response to a uniform field, and that gradient's change across the
 * centre (`get_others_tidal`) times the response to a linear field. The far side's linear part is used only with
 * `has_far_tidal` (the camera table), as it moved images further from the converged paths with the throat table.
 * The laser glow inside is taken along straight chords through the centre.
 * @return Whether the ray jumped (not if its angle is past the table's); `is_lost` is set if the table says
 * the ray never leaves.
 * @par Example
 * @code
 * if (jump_through_mouth(scene, scene.throat_table, THROAT_TABLE_SIZE, 1, 0.0, false, M_PI_2_F, row, x, v, travelled_m, core, halo, has_beam, is_lost)) { ... }
 * @endcode
 */
static bool jump_through_mouth(thread const Scene &scene, device const float4 *table, int size, int row_count, float row_position, bool has_far_tidal,
                               float angle_range, thread int &row, thread float4 &x, thread float4 &v, thread float &travelled_m,
                               thread float &core, thread float &halo, bool has_beam, thread bool &is_lost) {
    float q_nearest;
    int nearest = get_nearest_mouth(scene, row, x, q_nearest);
    float4 c = fetch(scene, row, TEXEL_MOUTHS + nearest), a, b;
    TableEntry entry = look_up_jump_table(table, size, row_count, row_position, angle_range, c, x, v, a, b);
    if (entry.base.w > 1.5) return false;
    if (entry.base.w < 0.0) { is_lost = true; return true; }
    float4 n = get_plane_normal(c, a, b), g_in = fetch(scene, row, TEXEL_MOUTH_FIELDS + TEXELS_PER_MOUTH_FIELD * nearest);
    float4 c_out = c, a_out = a, b_out = b, n_out = n;
    float3 field_out = float3(0.0), tidal_in, tidal_out = float3(0.0);
    float2 across_in, across_out = float2(0.0);
    get_others_tidal(scene, row, nearest, a, b, n, tidal_in, across_in);
    int row_out = row;
    if (entry.base.w > 0.5) {
        float4x4 gluing = fetch_matrix(scene, row, TEXEL_GLUINGS + TEXELS_PER_GLUING * nearest);
        c_out = float4(0.0, 0.0, 0.0, 1.0);
        a_out = gluing * a;
        b_out = gluing * b;
        n_out = gluing * n;
        row_out = scene.u.next_rows[nearest];
        float unused_q;
        int nearest_out = get_nearest_mouth(scene, row_out, c_out, unused_q);
        float4 g_out = fetch(scene, row_out, TEXEL_MOUTH_FIELDS + TEXELS_PER_MOUTH_FIELD * nearest_out);
        field_out = float3(mdot(g_out, a_out), mdot(g_out, b_out), mdot(g_out, n_out));
        if (has_far_tidal) get_others_tidal(scene, row_out, nearest_out, a_out, b_out, n_out, tidal_out, across_out);
    }
    float3 field_in = float3(mdot(g_in, a), mdot(g_in, b), mdot(g_in, n));
    float angle = entry.base.x + dot(entry.response_in.xy, field_in.xy) + dot(entry.response_out.xy, field_out.xy)
                + dot(entry.tidal[0].xyz, tidal_in) + dot(entry.tidal[3].xyz, tidal_out);
    float exit_angle = entry.base.z + dot(entry.response_in.zw, field_in.xy) + dot(entry.response_out.zw, field_out.xy)
                     + dot(entry.tidal[1].xyz, tidal_in) + dot(entry.tidal[4].xyz, tidal_out);
    float tilt_position = entry.tilt_response.x * field_in.z + entry.tilt_response.z * field_out.z + dot(entry.tidal[2].xy, across_in)
                        + dot(entry.tidal[5].xy, across_out);
    float tilt_direction = entry.tilt_response.y * field_in.z + entry.tilt_response.w * field_out.z + dot(entry.tidal[2].zw, across_in)
                         + dot(entry.tidal[5].zw, across_out);
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
 * @brief `get_table_field` gives a table ray's extra field at `x`: a uniform part `field` plus a linear part
 * `tidal` times x's spatial coordinates (its offset from the centre), as a tangent vector at `x`.
 * @par Example
 * @code
 * float4 g = get_table_field(field, tidal, x);
 * @endcode
 */
static float4 get_table_field(float4 field, float3x3 tidal, float4 x) {
    float4 f = field + float4(tidal * x.xyz, 0.0);
    return f + mdot(f, x) * x;
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
                                 int max_step_count, float4 field_in, float4 field_out, float3x3 tidal_in, float3x3 tidal_out) {
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
        float3x3 tidal = parity < 0.5 ? tidal_in : tidal_out;
        float ds_m = max(step_per_nearest * rho * r, TABLE_MIN_STEP_PER_STEP * step_per_nearest * throat_m);
        float4 g = get_dln_n_flat(rho_effective_m, throat_m) * e_rho * e_rho * radial + get_table_field(field, tidal, x);
        float4 bend = g - mdot(g, v) * v;
        float4 vh = to_tangent(x, v + bend * (0.5 * ds_m));
        float4 xm = x, vm = vh;
        advance_geodesic(xm, vm, 0.5 * ds_m / r);
        float qm = get_q(xm, c);
        float cosh_m = 1.0 + 0.5 * qm, sinh_m = sqrt(qm * (1.0 + 0.25 * qm)), e_m = cosh_m + sinh_m;
        float4 gm = get_dln_n_flat(r * sinh_m * e_m, throat_m) * e_m * e_m * (cosh_m * xm - c) / sinh_m + get_table_field(field, tidal, xm);
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
 * and the exit point's and direction's tilts per unit field along the plane's normal, before and after. Then, for
 * each side, three texels of the response to a linear field (zero at the centre, along the plane's axes a, b and
 * normal n): the swept angle's and the exit angle's per unit of its aa, bb and ab parts, and the tilts per unit
 * of its na and nb parts. The responses come from extra rays with a small field (`TABLE_FIELD_PROBE` per block,
 * `TIDAL_FIELD_PROBE` per curvature radius for the linear ones) and are zero where a probe changes whether the
 * ray crosses.
 * @par Example
 * @code
 * write_table_entry(throat_table + i * TEXELS_PER_TABLE_ENTRY, u.curvature_radius_m, u.throat_m, u.jump_m, u.jump_m, 0.3, 0.01, 40000);
 * @endcode
 */
static void write_table_entry(device float4 *entry, float r, float throat_m, float jump_m, float start_m, float angle_in,
                              float step_per_nearest, int max_step_count) {
    float4 zero = float4(0.0);
    float3x3 none = float3x3(0.0);
    TableExit base = trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, zero, zero, none, none);
    entry[0] = float4(base.angle, base.travelled_m, base.exit_angle, base.parity);
    float responses[12];
    for (int k = 0; k < 6; k++) {
        float4 field = float4(0.0);
        field[k % 3] = TABLE_FIELD_PROBE;
        TableExit probe = k < 3 ? trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, field, zero, none, none)
                                : trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, zero, field, none, none);
        bool is_valid = base.parity >= 0.0 && probe.parity == base.parity;
        float2 change = is_valid ? float2(probe.angle - base.angle, probe.exit_angle - base.exit_angle) / TABLE_FIELD_PROBE : float2(0.0);
        float2 tilt = is_valid ? float2(probe.tilt_position, probe.tilt_direction) / TABLE_FIELD_PROBE : float2(0.0);
        responses[2 * k] = k % 3 == 2 ? tilt.x : change.x;
        responses[2 * k + 1] = k % 3 == 2 ? tilt.y : change.y;
    }
    entry[1] = float4(responses[0], responses[2], responses[1], responses[3]);
    entry[2] = float4(responses[6], responses[8], responses[7], responses[9]);
    entry[3] = float4(responses[4], responses[5], responses[10], responses[11]);
    float3 e0 = float3(1.0, 0.0, 0.0), e1 = float3(0.0, 1.0, 0.0), e2 = float3(0.0, 0.0, 1.0), o = float3(0.0);
    float3x3 tidal_probes[5] = {float3x3(e0, o, o), float3x3(o, e1, o), float3x3(e1, e0, o), float3x3(e2, o, e0), float3x3(o, e2, e1)};
    for (int side = 0; side < 2; side++) {
        float tidal[10];
        for (int k = 0; k < 5; k++) {
            float3x3 probe_tidal = tidal_probes[k] * TIDAL_FIELD_PROBE;
            TableExit probe = side == 0 ? trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, zero, zero, probe_tidal, none)
                                        : trace_table_ray(r, throat_m, jump_m, start_m, angle_in, step_per_nearest, max_step_count, zero, zero, none, probe_tidal);
            bool is_valid = base.parity >= 0.0 && probe.parity == base.parity;
            float2 change = is_valid ? (k < 3 ? float2(probe.angle - base.angle, probe.exit_angle - base.exit_angle) : float2(probe.tilt_position, probe.tilt_direction))
                                           / TIDAL_FIELD_PROBE
                                     : float2(0.0);
            tidal[2 * k] = change.x;
            tidal[2 * k + 1] = change.y;
        }
        entry[4 + 3 * side] = float4(tidal[0], tidal[2], tidal[4], 0.0);
        entry[5 + 3 * side] = float4(tidal[1], tidal[3], tidal[5], 0.0);
        entry[6 + 3 * side] = float4(tidal[6], tidal[8], tidal[7], tidal[9]);
    }
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
 * `render_cpu.rs`'s `render_pixel` does, and says where it ended. Where a step's chord finds a hit further off the
 * curved path than `get_chord_limit_m` allows, it steps again from the step's start, landing just before the hit
 * and taking short steps across it.
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
        jump_through_mouth(scene, scene.camera_table, CAMERA_TABLE_SIZE, CAMERA_TABLE_ROWS, get_camera_table_row(u), true, M_PI_F, row, x, v,
                           travelled_m, core, halo, has_beam, is_lost);
        if (is_lost) return end;
        reset_box_cache(scene, cache, x, travelled_m);
    }
    float fine_start_m = -1.0, fine_end_m = -1.0, fine_step_m = 3.4e38;
    int refine_count = 0;
    for (int step = 0; step < MAX_STEP_COUNT; step++) {
        float q_nearest;
        int nearest;
        float4 bend = get_bend_nearest(scene, row, x, v, q_nearest, nearest);
        if (q_nearest < scene.q_throat && cross_throat(scene, row, x, v)) {
            reset_box_cache(scene, cache, x, travelled_m);
            bend = get_bend_nearest(scene, row, x, v, q_nearest, nearest);
        }
        if (q_nearest < scene.q_jump && was_outside_jump) {
            bool is_lost = false;
            if (jump_through_mouth(scene, scene.throat_table, THROAT_TABLE_SIZE, 1, 0.0, false, M_PI_2_F, row, x, v, travelled_m, core, halo, has_beam, is_lost)) {
                if (is_lost) break;
                reset_box_cache(scene, cache, x, travelled_m);
                bend = get_bend_nearest(scene, row, x, v, q_nearest, nearest);
            }
        }
        was_outside_jump = q_nearest >= scene.q_jump;
        bool is_far = q_nearest > scene.q_cutoff;
        float4 vh = v;
        float tau_max, ds_m;
        bool is_near = q_nearest > scene.q_rk4_start;
        float4 x_next = x, v_next = v;
        if (is_far) {
            tau_max = get_cutoff_entry_tau(scene, row, x, v);
            ds_m = tau_max < 1.0 ? atanh(tau_max) * r : max_travel_m;
        } else if (is_near) {
            ds_m = max(get_distance_of_q(q_nearest) * r * RK4_STEP_PER_NEAREST, MIN_STEP_M);
            ds_m = get_capped_step_m(ds_m, travelled_m, fine_start_m, fine_end_m, fine_step_m);
            ds_m = min(ds_m, MAX_RK4_STEP_M);
            if (q_nearest >= scene.q_jump) ds_m = min(ds_m, get_jump_entry_m(scene, row, x, v) + JUMP_OVERSHOOT_M);
            step_rk4(scene, row, x_next, v_next, bend, ds_m);
            float q_next;
            get_nearest_mouth(scene, row, x_next, q_next);
            if (q_nearest >= scene.q_jump && q_next < scene.q_jump) {
                float4 c = fetch(scene, row, TEXEL_MOUTHS + nearest);
                float rho_jump = get_distance_of_q(scene.q_jump), outside_m = 0.0, inside_m = ds_m;
                float rho_outside = get_distance_of_q(q_nearest), rho_inside = get_distance_of_q(q_next);
                for (int i = 0; i < JUMP_ENTRY_ITERATION_COUNT; i++) {
                    float try_m = mix(outside_m, inside_m, clamp((rho_outside - rho_jump) / max(rho_outside - rho_inside, 1e-6), 0.0, 1.0));
                    x_next = x;
                    v_next = v;
                    step_rk4(scene, row, x_next, v_next, bend, try_m);
                    float rho_try = get_distance_of_q(get_q(x_next, c));
                    if (rho_try < rho_jump) { inside_m = try_m; rho_inside = rho_try; } else { outside_m = try_m; rho_outside = rho_try; }
                }
                ds_m = inside_m + JUMP_OVERSHOOT_M;
                x_next = x;
                v_next = v;
                step_rk4(scene, row, x_next, v_next, bend, ds_m);
            }
            vh = to_tangent(x, x_next);
            tau_max = tanh(get_distance_of_q(get_q(x, x_next)));
        } else {
            float nearest_m = get_distance_of_q(q_nearest) * r;
            ds_m = max(nearest_m * (q_nearest < scene.q_near_zone ? NEAR_STEP_PER_NEAREST : FAR_STEP_PER_NEAREST), MIN_STEP_M);
            ds_m = get_capped_step_m(ds_m, travelled_m, fine_start_m, fine_end_m, fine_step_m);
            vh = to_tangent(x, v + bend * (0.5 * ds_m));
            tau_max = tanh(ds_m / r);
        }
        Hit hit = {tau_max, NO_HIT, 0, 1.0};
        float4 ground = fetch(scene, row, TEXEL_GROUND);
        float ground_a = mdot(x, ground), ground_b = mdot(vh, ground);
        if (ground_a > 0.0 && ground_b < 0.0 && -ground_a / ground_b < hit.tau) { hit.tau = -ground_a / ground_b; hit.index = GROUND_HIT; }
        test_boxes(scene, row, x, vh, travelled_m, ds_m, hit, cache);
        if (hit.index != NO_HIT && !is_far && refine_count < MAX_HIT_REFINE_COUNT) {
            float hit_m = atanh(hit.tau) * r, limit_m = get_chord_limit_m(bend, travelled_m + hit_m);
            if (ds_m > limit_m) {
                refine_count++;
                fine_start_m = travelled_m + max(hit_m - FINE_LEAD_PER_STEP * limit_m, 0.0);
                fine_end_m = travelled_m + hit_m + FINE_TRAIL_PER_STEP * limit_m;
                fine_step_m = limit_m;
                continue;
            }
        }
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
    float4 tint = end.row > 0 ? scene.u.row_tints[end.row - 1] : float4(0.0);
    if (tint.w > 0.0) color = mix(color, tint.rgb * (dot(color, TINT_LUMA) + THROWN_TINT_FLOOR), THROWN_TINT_FRAC);
    color += mix(LASER_HALO_COLOR, float3(1.0), LASER_HAZE_WHITENESS) * LASER_HAZE_BRIGHTNESS * halo;
    color = mix(color, LASER_CORE_COLOR * LASER_CORE_BRIGHTNESS, saturate(core));
    color = color / (1.0 + max(color - 1.0, 0.0) * HIGHLIGHT_ROLLOFF);
    return clamp(color, 0.0, 1.0);
}

/**
 * @brief `write_pixel` shades a ray end into pixel `id` of the image, and marks in `lost` (one byte per pixel of
 * the render size) whether the ray was lost, for `fill_holes`.
 * @par Example
 * @code
 * write_pixel(scene, trace_ray(scene, ndc), id, lost, image);
 * @endcode
 */
static void write_pixel(thread const Scene &scene, RayEnd end, uint2 id, device uchar *lost, texture2d<float, access::write> image) {
    image.write(float4(shade_ray_end(scene, end), 1.0), id);
    lost[int(id.y) * scene.u.width_px + int(id.x)] = end.kind == LOST_HIT ? 1 : 0;
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
                         device uchar *lost [[buffer(14)]], texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    write_pixel(scene, trace_ray(scene, get_ndc(scene, float2(id) + 0.5)), id, lost, image);
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
                          device uchar *lost [[buffer(14)]], texture2d<float, access::write> image [[texture(0)]], uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL) return;
    int columns = get_sample_columns(u);
    int cell = int(refine_cells[t / REFINED_PIXELS_PER_CELL]), corner = int(t % REFINED_PIXELS_PER_CELL) + 1;
    uint2 id = uint2(2 * (cell % columns) + (corner & 1), 2 * (cell / columns) + (corner >> 1));
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = make_scene(rows, u, throat_table, camera_table);
    write_pixel(scene, trace_ray(scene, get_ndc(scene, float2(id) + 0.5)), id, lost, image);
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
                           device uchar *lost [[buffer(14)]], texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
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
    write_pixel(scene, end, id, lost, image);
}

/**
 * @brief `PyramidLevels` is one step of the hole-filling pyramid: the sizes in pixels of a finer level (`low`) and
 * the next coarser one (`high`, half the size, rounded up), and where each starts in the pyramid buffer (level 1
 * starts at 0; level 0 is the image). Each pyramid texel is a colour premultiplied by its weight, and the weight.
 */
struct PyramidLevels {
    int2 low_size;
    int2 high_size;
    int low_offset;
    int high_offset;
};

/**
 * @brief `read_pyramid` reads texel `p` of a pyramid level, clamped to the level's edges.
 * @par Example
 * @code
 * float4 texel = read_pyramid(pyramid, levels.high_offset, levels.high_size, int2(3, 4));
 * @endcode
 */
static float4 read_pyramid(device const half4 *pyramid, int offset, int2 size, int2 p) {
    p = clamp(p, int2(0), size - 1);
    return float4(pyramid[offset + p.y * size.x + p.x]);
}

/**
 * @brief `sample_pyramid` samples a pyramid level bilinearly at a position in its texels (texel centres at whole
 * numbers).
 * @par Example
 * @code
 * float4 coarse = sample_pyramid(pyramid, levels.high_offset, levels.high_size, (float2(id) + 0.5) * 0.5 - 0.5);
 * @endcode
 */
static float4 sample_pyramid(device const half4 *pyramid, int offset, int2 size, float2 position) {
    float2 base = floor(position), f = position - base;
    int2 p = int2(base);
    return mix(mix(read_pyramid(pyramid, offset, size, p), read_pyramid(pyramid, offset, size, p + int2(1, 0)), f.x),
               mix(read_pyramid(pyramid, offset, size, p + int2(0, 1)), read_pyramid(pyramid, offset, size, p + int2(1, 1)), f.x), f.y);
}

/**
 * @brief `average_children` turns the sum of up to four premultiplied texels (colour times weight, and weight)
 * into their parent: their weighted average colour, with a weight of their total weight, at most 1.
 * @par Example
 * @code
 * pyramid[index] = average_children(sum);
 * @endcode
 */
static half4 average_children(float4 sum) {
    float weight = min(sum.a, 1.0);
    return half4(sum.a > 0.0 ? float4(sum.rgb / sum.a * weight, weight) : float4(0.0));
}

/**
 * @brief `pull_holes_from_image` makes pyramid level 1 from the image: each texel averages the pixels of its 2×2
 * block whose rays weren't lost.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per texel of level 1.
 * @endcode
 */
kernel void pull_holes_from_image(constant Uniforms &u [[buffer(1)]], device const uchar *lost [[buffer(14)]], device half4 *pyramid [[buffer(15)]],
                                  constant PyramidLevels &levels [[buffer(16)]], texture2d<float, access::read> image [[texture(0)]],
                                  uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= levels.high_size.x || int(id.y) >= levels.high_size.y) return;
    float4 sum = float4(0.0);
    for (int k = 0; k < 4; k++) {
        int2 p = int2(id) * 2 + int2(k & 1, k >> 1);
        if (p.x < levels.low_size.x && p.y < levels.low_size.y && lost[p.y * u.width_px + p.x] == 0) sum += float4(image.read(uint2(p)).rgb, 1.0);
    }
    pyramid[levels.high_offset + int(id.y) * levels.high_size.x + int(id.x)] = average_children(sum);
}

/**
 * @brief `pull_holes` makes the next coarser pyramid level: each texel averages its 2×2 block by weight.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per texel of the coarser level.
 * @endcode
 */
kernel void pull_holes(device half4 *pyramid [[buffer(15)]], constant PyramidLevels &levels [[buffer(16)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= levels.high_size.x || int(id.y) >= levels.high_size.y) return;
    float4 sum = float4(0.0);
    for (int k = 0; k < 4; k++) {
        int2 p = int2(id) * 2 + int2(k & 1, k >> 1);
        if (p.x < levels.low_size.x && p.y < levels.low_size.y) sum += float4(pyramid[levels.low_offset + p.y * levels.low_size.x + p.x]);
    }
    pyramid[levels.high_offset + int(id.y) * levels.high_size.x + int(id.x)] = average_children(sum);
}

/**
 * @brief `push_holes` fills in a finer pyramid level from the coarser one already filled: a texel of weight w keeps
 * its own colour and takes 1 - w of the coarser level's, sampled bilinearly, so holes fade smoothly into the
 * colours around them.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per texel of the finer level, from the coarsest level down.
 * @endcode
 */
kernel void push_holes(device half4 *pyramid [[buffer(15)]], constant PyramidLevels &levels [[buffer(16)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= levels.low_size.x || int(id.y) >= levels.low_size.y) return;
    int index = levels.low_offset + int(id.y) * levels.low_size.x + int(id.x);
    float4 own = float4(pyramid[index]);
    if (own.a >= 1.0) return;
    float4 coarse = sample_pyramid(pyramid, levels.high_offset, levels.high_size, (float2(id) + 0.5) * 0.5 - 0.5);
    pyramid[index] = half4(float4(own.rgb + (1.0 - own.a) * coarse.rgb, 1.0));
}

/**
 * @brief `fill_holes` paints each pixel whose ray was lost (trapped circling a throat) with filled pyramid level 1,
 * sampled bilinearly, instead of black.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per pixel of the render size.
 * @endcode
 */
kernel void fill_holes(constant Uniforms &u [[buffer(1)]], device const uchar *lost [[buffer(14)]], device const half4 *pyramid [[buffer(15)]],
                       constant PyramidLevels &levels [[buffer(16)]], texture2d<float, access::write> image [[texture(0)]],
                       uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px || lost[int(id.y) * u.width_px + int(id.x)] == 0) return;
    float4 coarse = sample_pyramid(pyramid, levels.high_offset, levels.high_size, (float2(id) + 0.5) * 0.5 - 0.5);
    image.write(float4(coarse.rgb, 1.0), id);
}

/**
 * @brief `build_mouth_fields` stores, each frame, for each mouth `id.x` in each row `id.y`, the other mouths'
 * gradient at its centre and the matrix that gives that gradient's change along any tangent vector there (by
 * central differences along an orthonormal basis), for the jumps' first-order corrections.
 * @par Example
 * @code
 * dispatched by render_metal.rs first in each frame, one thread per mouth and row.
 * @endcode
 */
kernel void build_mouth_fields(device float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]], uint2 id [[thread_position_in_grid]]) {
    int mouth = int(id.x), row = int(id.y);
    if (mouth >= u.mouth_count || row > u.mouth_count) return;
    Scene scene = make_scene(rows, u, rows, rows);
    float4 c = fetch(scene, row, TEXEL_MOUTHS + mouth);
    device float4 *field = rows + row * ROW_TEXEL_COUNT + TEXEL_MOUTH_FIELDS + TEXELS_PER_MOUTH_FIELD * mouth;
    float step_cosh = cosh(TIDAL_STEP_PER_R), step_sinh = sinh(TIDAL_STEP_PER_R);
    float4x4 change = float4x4(0.0);
    float4 basis[3];
    for (int j = 0; j < 3; j++) {
        float4 e = float4(0.0);
        e[j] = 1.0;
        e = e + mdot(e, c) * c;
        for (int i = 0; i < j; i++) e = e - mdot(e, basis[i]) * basis[i];
        basis[j] = e * rsqrt(mdot(e, e));
        float4 d = (get_others_gradient(scene, row, step_cosh * c + step_sinh * basis[j], mouth)
                    - get_others_gradient(scene, row, step_cosh * c - step_sinh * basis[j], mouth)) / (2.0 * step_sinh);
        change = change + float4x4(d * basis[j].x, d * basis[j].y, d * basis[j].z, -d * basis[j].w);
    }
    field[0] = get_others_gradient(scene, row, c, mouth);
    for (int i = 0; i < 4; i++) field[1 + i] = change[i];
}

/**
 * @brief `OverlayParams` is how `apply_overlay` draws: how much to darken, mask pixels to screen pixels, the mask's top
 * left, how many rectangles there are, whether there is a mask and a wand, the camera's tan(half vertical fov), how
 * much of the previous frame's scene to blend in (motion blur), and how far the scene blueshifts inside a wormhole
 * mouth (`blueshift_scene`: 1 for walking straight ahead at the throat, negative for a redshift, 0 for none).
 */
struct OverlayParams {
    float dim_frac;
    int scale;
    int2 origin;
    int rect_count;
    int has_mask;
    int has_wand;
    float tan_half_fov;
    float blur_frac;
    float hit_frac;
    float hit_red_frac;
    float hit_blur_px;
    float2 shake_px;
    int2 picture_min_mask_px;
    int2 picture_max_excl_mask_px;
    float stretch_frac;
    float blueshift_frac;
};

/**
 * @brief `sample_spectrum` gives a colour's light at wavelength `nm`, reading its red, green and blue as samples of a
 * spectrum at `BLUESHIFT_CHANNEL_NM` joined by straight lines, falling to 0 at the eye's limits,
 * `BLUESHIFT_ULTRAVIOLET_NM` and `BLUESHIFT_INFRARED_NM` (a picture holds no light beyond them for a shift to bring in).
 * @par Example
 * @code
 * float red_seen = sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.r * D);
 * @endcode
 */
static float sample_spectrum(float3 rgb, float nm) {
    float3 at_nm = BLUESHIFT_CHANNEL_NM;
    if (nm <= at_nm.b) return rgb.b * saturate((nm - BLUESHIFT_ULTRAVIOLET_NM) / (at_nm.b - BLUESHIFT_ULTRAVIOLET_NM));
    if (nm <= at_nm.g) return mix(rgb.b, rgb.g, (nm - at_nm.b) / (at_nm.g - at_nm.b));
    if (nm <= at_nm.r) return mix(rgb.g, rgb.r, (nm - at_nm.g) / (at_nm.r - at_nm.g));
    return rgb.r * saturate((BLUESHIFT_INFRARED_NM - nm) / (BLUESHIFT_INFRARED_NM - at_nm.r));
}

/**
 * @brief `blueshift_scene` gives a pixel's colour as seen moving along the view, `blueshift_frac` giving how fast
 * (1 for walking at a mouth's throat; negative backward; 0 for none), as a flash close to white: a small relativistic
 * Doppler shift, then most of the way to a pale tinted white.
 * - Shift: the Doppler factor D = sqrt(1 - β²) / (1 - β cos θ), with θ the angle between the pixel's ray and
 *   straight ahead and β = `BLUESHIFT_WALK_SPEED_PER_LIGHT_SPEED` × max(|`blueshift_frac`|, 1) (negative backward),
 *   makes each channel show the light the scene sends at D times its wavelength (`sample_spectrum`), D times as
 *   bright (relativistic beaming).
 * - White: that colour goes `BLUESHIFT_WHITE_FRAC` of the way to `BLUESHIFT_WHITE_COLOR` (a pale blue-white) moving
 *   forward, or to `REDSHIFT_WHITE_COLOR` (a warm white) moving backward, as the forward view whitens at speeds near
 *   light's.
 * - Strength: below |`blueshift_frac`| = 1, the result fades in by |`blueshift_frac`|.
 * @par Example
 * @code
 * float3 seen = blueshift_scene(scene, id, float2(out.get_width(), out.get_height()), p);
 * @endcode
 * @par Must know
 * - Limitations:
 *   - `blueshift_scene` takes the motion to be along the view, wherever the player really goes.
 */
static float3 blueshift_scene(float3 rgb, uint2 id, float2 size, constant OverlayParams &p) {
    if (p.blueshift_frac == 0.0) return rgb;
    float2 ndc = 2.0 * (float2(id) + 0.5) / size - 1.0;
    float3 ray = normalize(float3(ndc.x * p.tan_half_fov * size.x / size.y, ndc.y * p.tan_half_fov, -1.0));
    float speed_frac = abs(p.blueshift_frac);
    float beta = sign(p.blueshift_frac) * BLUESHIFT_WALK_SPEED_PER_LIGHT_SPEED * max(speed_frac, 1.0);
    float D = sqrt(1.0 - beta * beta) / (1.0 + beta * ray.z);
    float3 shifted = D * float3(sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.r * D), sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.g * D),
                                sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.b * D));
    float3 white = p.blueshift_frac > 0.0 ? BLUESHIFT_WHITE_COLOR : REDSHIFT_WHITE_COLOR;
    return mix(rgb, mix(shifted, white, BLUESHIFT_WHITE_FRAC), min(speed_frac, 1.0));
}

/**
 * @brief `read_hit_scene` reads a frame's pixel as a player who was just hit sees it (`hit_frac` of the effect left, 1
 * just after the hit): shifted by `shake_px`, and toward the edges blurred over `hit_blur_px` and turned red by
 * `hit_red_frac`, reaching further in the fresher the hit; with no hit left, the pixel as it is.
 * @par Example
 * @code
 * float3 scene = read_hit_scene(frame, id, p);
 * @endcode
 */
static float3 read_hit_scene(texture2d<float, access::read> frame, uint2 id, constant OverlayParams &p) {
    int2 size = int2(frame.get_width(), frame.get_height());
    int2 at = clamp(int2(id) + int2(round(p.shake_px)), int2(0), size - 1);
    float3 sharp = frame.read(uint2(at)).rgb;
    if (p.hit_frac <= 0.0) return sharp;
    float corner = length((float2(id) + 0.5) / float2(size) - 0.5) * M_SQRT2_F;
    float edge = p.hit_frac * smoothstep(1.0 - HIT_EDGE_REACH_FRAC * p.hit_frac, 1.0, corner);
    float3 sum = sharp;
    for (int k = 0; k < HIT_BLUR_TAP_COUNT; k++) {
        float angle = 2.0 * M_PI_F * float(k) / float(HIT_BLUR_TAP_COUNT);
        int2 tap = at + int2(round(p.hit_blur_px * edge * float2(cos(angle), sin(angle))));
        sum += frame.read(uint2(clamp(tap, int2(0), size - 1))).rgb;
    }
    float3 blurred = mix(sharp, sum / float(HIT_BLUR_TAP_COUNT + 1), edge);
    return mix(blurred, HIT_RED_COLOR, edge * p.hit_red_frac);
}

/**
 * @brief `read_stretched_scene` reads a frame's pixel as a player slowed at the map's soft edge sees it (`stretch_frac`
 * of the way to its stop, 0 for none): the scene pulled out from the middle of the screen (magnified up to
 * 1 + `STRETCH_ZOOM_GAIN` times at the middle, not at all at the corners) and smeared along lines from the middle over
 * `STRETCH_SMEAR_FRAC` of the way in (`STRETCH_TAP_COUNT` taps), red pulled a little further and blue a little less
 * (`STRETCH_FRINGE_FRAC`); each tap is read as `read_hit_scene` reads it.
 * @par Example
 * @code
 * float3 scene = read_stretched_scene(frame, id, p);
 * @endcode
 */
static float3 read_stretched_scene(texture2d<float, access::read> frame, uint2 id, constant OverlayParams &p) {
    if (p.stretch_frac <= 0.0) return read_hit_scene(frame, id, p);
    float2 size = float2(frame.get_width(), frame.get_height());
    float2 middle = size * 0.5;
    float2 offset = float2(id) + 0.5 - middle;
    float zoom = pow(1.0 + STRETCH_ZOOM_GAIN * p.stretch_frac, 1.0 - length(offset) / length(middle));
    float fringe = STRETCH_FRINGE_FRAC * p.stretch_frac;
    float3 sum = float3(0.0);
    for (int k = 0; k < STRETCH_TAP_COUNT; k++) {
        float pull = 1.0 - STRETCH_SMEAR_FRAC * p.stretch_frac * float(k) / float(STRETCH_TAP_COUNT - 1);
        float3 pulls = pull * float3(1.0 + fringe, 1.0, 1.0 - fringe);
        for (int c = 0; c < 3; c++) {
            float2 source = clamp(middle + offset / zoom * pulls[c], float2(0.0), size - 1.0);
            sum[c] += read_hit_scene(frame, uint2(source), p)[c];
        }
    }
    return sum / float(STRETCH_TAP_COUNT);
}

/**
 * @brief `OverlayWand` is the wand in camera space (x right, y up, -z ahead, meters): its shaft's centre, its axes (its
 * length along its own -z), its shaft's half sizes and colour, and its tip's half sizes and colour (the tip sits at the
 * shaft's front end).
 */
struct OverlayWand {
    float4 center_m;
    float4 axes[3];
    float4 shaft_half_m;
    float4 shaft_color;
    float4 tip_half_m;
    float4 tip_color;
};

/**
 * @brief `hit_box` gives the distance along a ray from the eye (unit direction `d`) to a box (centre, axes, half sizes),
 * or -1 if it misses, with the hit face's normal and how far the hit is from that face's nearest edge.
 * @par Example
 * @code
 * float t = hit_box(d, center, axes, half_m, normal, edge_m);
 * @endcode
 */
static float hit_box(float3 d, float3 center, float3x3 axes, float3 half_m, thread float3 &normal, thread float &edge_m) {
    float3 o = transpose(axes) * -center, ld = transpose(axes) * d;
    float3 inv = 1.0 / select(ld, float3(1e-7), abs(ld) < 1e-7);
    float3 t0 = (-half_m - o) * inv, t1 = (half_m - o) * inv;
    float3 t_lo = min(t0, t1), t_hi = max(t0, t1);
    float t_near = max(max(t_lo.x, t_lo.y), t_lo.z), t_far = min(min(t_hi.x, t_hi.y), t_hi.z);
    if (t_near > t_far || t_near < 0.0) return -1.0;
    float3 face = select(float3(0.0), -sign(ld), t_lo == t_near);
    float3 gap = half_m - abs(o + t_near * ld);
    edge_m = min(min(face.x != 0.0 ? 1e3 : gap.x, face.y != 0.0 ? 1e3 : gap.y), face.z != 0.0 ? 1e3 : gap.z);
    normal = axes * face;
    return t_near;
}

/**
 * @brief `shade_wand` gives the wand's colour and coverage (0 or 1) along one ray from the eye: a lit, dark shaft with a
 * highlight and darkened edges, and a glowing tip; past them, the tip's halo with coverage 0.
 * @par Example
 * @code
 * float4 c = shade_wand(d, wand);
 * @endcode
 */
static float4 shade_wand(float3 d, constant OverlayWand &wand) {
    float3x3 axes = float3x3(wand.axes[0].xyz, wand.axes[1].xyz, wand.axes[2].xyz);
    float3 tip_center = wand.center_m.xyz + axes * float3(0.0, 0.0, -(wand.shaft_half_m.z + wand.tip_half_m.z));
    float3 shaft_normal, tip_normal;
    float shaft_edge_m, tip_edge_m;
    float t_shaft = hit_box(d, wand.center_m.xyz, axes, wand.shaft_half_m.xyz, shaft_normal, shaft_edge_m);
    float t_tip = hit_box(d, tip_center, axes, wand.tip_half_m.xyz, tip_normal, tip_edge_m);
    float3 light = normalize(WAND_LIGHT_DIRECTION);
    if (t_tip > 0.0 && (t_shaft < 0.0 || t_tip < t_shaft)) {
        return float4(wand.tip_color.rgb * (WAND_TIP_GLOW + WAND_AMBIENT * max(dot(tip_normal, light), 0.0)), 1.0);
    }
    if (t_shaft > 0.0) {
        float diffuse = max(dot(shaft_normal, light), 0.0);
        float highlight = pow(max(dot(reflect(-light, shaft_normal), -d), 0.0), WAND_SHININESS);
        float edge = mix(WAND_EDGE_DARKEN, 1.0, smoothstep(0.0, WAND_EDGE_M, shaft_edge_m));
        return float4(wand.shaft_color.rgb * (WAND_AMBIENT + diffuse) * edge + WAND_SPECULAR * highlight, 1.0);
    }
    float miss_m = length(cross(d, tip_center));
    return float4(wand.tip_color.rgb * WAND_HALO_STRENGTH * exp(-miss_m / WAND_HALO_M), 0.0);
}

/** @brief `MinimapMark` is a mark on the minimap: a point in its unit disk (x right, y down), a radius in pixels and a zero; and a colour. */
struct MinimapMark {
    float4 disk;
    float4 color;
};

/** @brief `MinimapLine` is a line on the minimap: its two ends in the unit disk (x right, y down), and a colour. */
struct MinimapLine {
    float4 ends;
    float4 color;
};

/**
 * @brief `Minimap` is render_metal.rs's `Minimap`: the frame taking the minimap's plane (y = 0, −z up the minimap) into
 * the world; its centre and radius in pixels and the Poincaré disk's radius at its rim; the map picture's top left corner
 * (Fermi u, v), meters per pixel and the curvature radius; the counts of marks and lines; the marks and the lines.
 */
struct Minimap {
    float4x4 ground_frame;
    float4 placement;
    float4 map;
    int4 counts;
    MinimapMark marks[MAX_MINIMAP_MARK_COUNT];
    MinimapLine lines[MAX_MINIMAP_LINE_COUNT];
};

/**
 * @brief `draw_minimap` draws the minimap over `color` at pixel `id`: a disk of radius `placement.z` around
 * `placement.xy`, a Poincaré disk of the ground around the player (out to `placement.w`, tanh of half its range in
 * curvature radii), so geodesics away from the player bend into arcs. Each pixel's disk point goes onto the hyperboloid,
 * through `ground_frame` into the world, to its Fermi (u, v) and so to the map's picture; then come faint lines every
 * `MINIMAP_GRID_M` of u and v (as wide as a pixel, from the disk's metric 2R / (1 − |q|²)), the shots' lines, the other
 * players' marks, and a white triangle for the player pointing up; inside a rim.
 * @par Example
 * @code
 * color.rgb = draw_minimap(color.rgb, id, minimap, map);
 * @endcode
 */
static float3 draw_minimap(float3 color, uint2 id, constant Minimap &m, texture2d<float, access::read> map) {
    float radius_px = m.placement.z;
    if (radius_px <= 0.0) return color;
    float2 offset_px = float2(id) + 0.5 - m.placement.xy;
    float distance_px = length(offset_px);
    if (distance_px > radius_px + MINIMAP_RIM_PX) return color;
    if (distance_px > radius_px) return mix(color, MINIMAP_RIM_COLOR.rgb, MINIMAP_RIM_COLOR.a);
    float2 q = offset_px / radius_px * m.placement.w;
    float qq = dot(q, q);
    float4 local = float4(2.0 * q.x, 0.0, 2.0 * q.y, 1.0 + qq) / (1.0 - qq);
    float4 x = m.ground_frame * local;
    float r = m.map.w;
    float2 fermi_m = float2(-r * atanh(x.z / x.w), r * asinh(x.x / sqrt(1.0 + x.y * x.y)));
    float2 at_px = (fermi_m - m.map.xy) / m.map.z;
    float3 ground = MINIMAP_OFF_MAP_COLOR;
    if (all(at_px >= 0.0) && all(at_px < float2(map.get_width(), map.get_height()))) ground = map.read(uint2(at_px)).rgb;
    float m_per_px = 2.0 * r * m.placement.w / radius_px / (1.0 - qq);
    float2 to_grid_m = abs(fract(fermi_m / MINIMAP_GRID_M + 0.5) - 0.5) * MINIMAP_GRID_M;
    if (min(to_grid_m.x, to_grid_m.y) < MINIMAP_GRID_HALF_WIDTH_PX * m_per_px) {
        ground = mix(ground, MINIMAP_GRID_COLOR.rgb, MINIMAP_GRID_COLOR.a);
    }
    for (int i = 0; i < m.counts.y; i++) {
        float2 a = m.lines[i].ends.xy * radius_px;
        float2 ab = m.lines[i].ends.zw * radius_px - a;
        float t = clamp(dot(offset_px - a, ab) / max(dot(ab, ab), MINIMAP_TINY), 0.0, 1.0);
        if (length(offset_px - a - t * ab) < MINIMAP_LINE_HALF_WIDTH_PX) ground = m.lines[i].color.rgb;
    }
    for (int i = 0; i < m.counts.x; i++) {
        if (length(offset_px - m.marks[i].disk.xy * radius_px) < m.marks[i].disk.z) ground = m.marks[i].color.rgb;
    }
    float2 you = offset_px / MINIMAP_YOU_PX;
    if (you.y > -1.0 && you.y < MINIMAP_YOU_BASE_FRAC && abs(you.x) < (you.y + 1.0) * MINIMAP_YOU_SPREAD) {
        ground = MINIMAP_YOU_COLOR;
    }
    return mix(color, ground, MINIMAP_OPACITY);
}

/**
 * @brief `RectStyle` is render_metal.rs's `RectStyle`, how an `OverlayRect` looks beyond its colour (lengths as fractions
 * of the output's height; all zero for a flat rectangle): its corners' radius, an inner outline's width, a gloss
 * highlight's strength over its top half, a soft drop shadow's blur, a slant (how far its sides lean right per unit of
 * height), a glow's radius in its own colour, how much lighter its top and darker its bottom are, a zero, and the
 * outline's colour.
 */
struct RectStyle {
    float corner_frac;
    float outline_frac;
    float gloss_frac;
    float shadow_frac;
    float slant_x_per_y;
    float glow_frac;
    float shade_frac;
    float padding;
    float4 outline_color;
};

/**
 * @brief `RectPulse` is render_metal.rs's `RectPulse`, up to `MAX_RECT_PULSE_COUNT` soft glows running along a HUD bar,
 * carried by the bar's first `OverlayRect`: their middles' x as fractions of the screen's width (measured along the rect's
 * slant, at its middle row), their opacities (0 for none), the x span they show in (the bar's filled part, fading out
 * over `RECT_PULSE_EDGE_SOFT_PER_HALF_WIDTH` of a half width, at most half the span, inside each end), their half width
 * (of the screen's width) and half height (of its height), and their colour (`.a` unused).
 */
struct RectPulse {
    float4 centers_x_frac;
    float4 opacities;
    float min_x_frac;
    float max_x_frac;
    float half_width_frac;
    float half_height_frac;
    float4 color;
};

/**
 * @brief `OverlayRect` is a rectangle drawn over the frame: its corners as fractions of the screen (top left 0, 0), its
 * colour and opacity, its style and the pulses it carries for its bar.
 */
struct OverlayRect {
    float2 min_frac;
    float2 max_frac;
    float4 color;
    RectStyle style;
    RectPulse pulse;
};

/**
 * @brief `get_rounded_box_distance` gives the signed distance from `p` to a box of half sizes `half_size` around the origin
 * with corners rounded by `radius` (negative inside), the usual rounded-box distance.
 * @par Example
 * @code
 * float d = get_rounded_box_distance(p - center, half_size, 4.0);
 * @endcode
 */
static float get_rounded_box_distance(float2 p, float2 half_size, float radius) {
    float2 q = abs(p) - half_size + radius;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
}

/**
 * @brief `get_rect_distance_px` gives the signed distance in pixels from pixel centre `at_px` to `rect`'s shape on an
 * output `size_px`: its box, slanted and with rounded corners, moved down by `drop_px` (for a shadow).
 * @par Example
 * @code
 * float d_px = get_rect_distance_px(rect, at_px, size_px, 0.0);
 * @endcode
 */
static float get_rect_distance_px(constant OverlayRect &rect, float2 at_px, float2 size_px, float drop_px) {
    float2 min_px = rect.min_frac * size_px;
    float2 max_px = rect.max_frac * size_px;
    float2 half_size = (max_px - min_px) * 0.5;
    float2 p = at_px - (min_px + half_size) - float2(0.0, drop_px);
    p.x += rect.style.slant_x_per_y * p.y;
    float radius = min(rect.style.corner_frac * size_px.y, min(half_size.x, half_size.y));
    return get_rounded_box_distance(p, half_size, radius);
}

/**
 * @brief `is_near_rect` tells whether pixel centre `at_px` lies within `reach_px` of `rect`'s box (widened by its slant),
 * so the rect loops can skip far rectangles cheaply.
 * @par Example
 * @code
 * if (!is_near_rect(rect, at_px, size_px, 1.0)) continue;
 * @endcode
 */
static bool is_near_rect(constant OverlayRect &rect, float2 at_px, float2 size_px, float reach_px) {
    float2 min_px = rect.min_frac * size_px;
    float2 max_px = rect.max_frac * size_px;
    float reach_x_px = reach_px + abs(rect.style.slant_x_per_y) * (max_px.y - min_px.y) * 0.5;
    return at_px.x >= min_px.x - reach_x_px && at_px.x <= max_px.x + reach_x_px && at_px.y >= min_px.y - reach_px &&
           at_px.y <= max_px.y + reach_px;
}

/**
 * @brief `get_rect_halo` gives what `rect` casts around itself at pixel centre `at_px`, outside its shape: its drop
 * shadow's opacity (`RECT_SHADOW_OPACITY` at most, `RECT_SHADOW_DROP_PER_BLUR` of the blur below it) in `.a` and its
 * glow's light (its colour fading over `glow_frac` of the height) in `.rgb`.
 * @par Example
 * @code
 * float4 halo = get_rect_halo(rects[i], at_px, size_px);
 * @endcode
 */
static float4 get_rect_halo(constant OverlayRect &rect, float2 at_px, float2 size_px) {
    float shadow_px = rect.style.shadow_frac * size_px.y;
    float glow_px = rect.style.glow_frac * size_px.y;
    float reach_px = max(shadow_px * (1.0 + RECT_SHADOW_DROP_PER_BLUR), glow_px * RECT_GLOW_REACH_PER_RADIUS);
    if (reach_px <= 0.0 || !is_near_rect(rect, at_px, size_px, reach_px)) return float4(0.0);
    float d_px = get_rect_distance_px(rect, at_px, size_px, 0.0);
    float outside = smoothstep(-RECT_EDGE_SOFT_PX, RECT_EDGE_SOFT_PX, d_px);
    float4 halo = float4(0.0);
    if (shadow_px > 0.0) {
        float shadow_d_px = get_rect_distance_px(rect, at_px, size_px, shadow_px * RECT_SHADOW_DROP_PER_BLUR);
        halo.a = RECT_SHADOW_OPACITY * rect.color.a * (1.0 - smoothstep(-shadow_px, shadow_px, shadow_d_px)) * outside;
    }
    if (glow_px > 0.0) {
        halo.rgb = rect.color.rgb * RECT_GLOW_OPACITY * rect.color.a * exp(-max(d_px, 0.0) / glow_px) * outside;
    }
    return halo;
}

/**
 * @brief `get_pulse_glow` gives how much of `rect`'s pulse colour covers pixel centre `at_px` (0 to 1): the sum of its
 * pulses' Gaussians along the bar (leaning with its slant) times a Gaussian across it, faded out smoothly inside both
 * ends of its span, so it never reaches the empty slots. The glow covers the segments and the gaps between them alike.
 * @par Example
 * @code
 * color.rgb = mix(color.rgb, rect.pulse.color.rgb, get_pulse_glow(rect, at_px, size_px));
 * @endcode
 */
static float get_pulse_glow(constant OverlayRect &rect, float2 at_px, float2 size_px) {
    RectPulse pulse = rect.pulse;
    if (all(pulse.opacities <= 0.0)) return 0.0;
    float half_width_px = max(pulse.half_width_frac * size_px.x, 1.0);
    float half_height_px = max(pulse.half_height_frac * size_px.y, 1.0);
    float across_px = at_px.y - (rect.min_frac.y + rect.max_frac.y) * 0.5 * size_px.y;
    if (abs(across_px) > RECT_PULSE_REACH_PER_HALF_SIZE * half_height_px) return 0.0;
    float along_px = at_px.x + rect.style.slant_x_per_y * across_px;
    float min_px = pulse.min_x_frac * size_px.x;
    float max_px = pulse.max_x_frac * size_px.x;
    float soft_px = max(min(RECT_PULSE_EDGE_SOFT_PER_HALF_WIDTH * half_width_px, (max_px - min_px) * 0.5), 1.0);
    float span = smoothstep(min_px, min_px + soft_px, along_px) * (1.0 - smoothstep(max_px - soft_px, max_px, along_px));
    if (span <= 0.0) return 0.0;
    float sum = 0.0;
    for (int k = 0; k < MAX_RECT_PULSE_COUNT; k++) {
        float u = (along_px - pulse.centers_x_frac[k] * size_px.x) / half_width_px;
        sum += pulse.opacities[k] * exp(-u * u);
    }
    float v = across_px / half_height_px;
    return min(sum, 1.0) * exp(-v * v) * span;
}

/**
 * @brief `get_rect_layer` gives `rect` at pixel centre `at_px` as premultiplied colour and opacity (zero outside it, soft
 * over half a pixel at its edge): its colour shaded from top to bottom by `shade_frac`, a gloss highlight over its top
 * half (inset from its edge by `RECT_GLOSS_INSET_PER_HEIGHT` of its height), then its inner outline.
 * @par Example
 * @code
 * float4 layer = get_rect_layer(rects[i], at_px, size_px);
 * color.rgb = color.rgb * (1.0 - layer.a) + layer.rgb;
 * @endcode
 */
static float4 get_rect_layer(constant OverlayRect &rect, float2 at_px, float2 size_px) {
    if (!is_near_rect(rect, at_px, size_px, 1.0)) return float4(0.0);
    float d_px = get_rect_distance_px(rect, at_px, size_px, 0.0);
    float coverage = 1.0 - smoothstep(-RECT_EDGE_SOFT_PX, RECT_EDGE_SOFT_PX, d_px);
    if (coverage <= 0.0) return float4(0.0);
    RectStyle s = rect.style;
    float height_px = (rect.max_frac.y - rect.min_frac.y) * size_px.y;
    float t = clamp((at_px.y - rect.min_frac.y * size_px.y) / max(height_px, 1.0), 0.0, 1.0);
    float3 rgb = rect.color.rgb * (1.0 + s.shade_frac * (1.0 - 2.0 * t));
    float4 layer = float4(rgb * rect.color.a, rect.color.a);
    if (s.gloss_frac > 0.0) {
        float inset = smoothstep(0.0, RECT_GLOSS_INSET_PER_HEIGHT * height_px, -d_px - s.outline_frac * size_px.y);
        float top = 1.0 - smoothstep(RECT_GLOSS_END_FRAC - RECT_GLOSS_SOFT_FRAC, RECT_GLOSS_END_FRAC, t);
        float gloss_a = s.gloss_frac * inset * top * mix(RECT_GLOSS_TOP_OPACITY, RECT_GLOSS_MIDDLE_OPACITY, t / RECT_GLOSS_END_FRAC);
        layer = float4(gloss_a, gloss_a, gloss_a, gloss_a) + layer * (1.0 - gloss_a);
    }
    if (s.outline_frac > 0.0) {
        float half_width_px = s.outline_frac * size_px.y * 0.5;
        float stroke = clamp(half_width_px - abs(d_px + half_width_px) + RECT_EDGE_SOFT_PX, 0.0, 1.0);
        float outline_a = stroke * s.outline_color.a;
        layer = float4(s.outline_color.rgb * outline_a, outline_a) + layer * (1.0 - outline_a);
    }
    return layer * coverage;
}

/**
 * @brief `apply_overlay` copies a finished frame into `out`, blended with the previous frame's scene (`history_in`, by
 * `blur_frac`; the blend is kept in `history_out` for the next frame), blueshifted (`blueshift_scene`) and darkened by `dim_frac`, with a mask's text drawn over it at
 * `scale` screen pixels per mask pixel from `origin` (the title's colour where the mask is above `OVERLAY_TITLE_LEVEL`,
 * the text's above `OVERLAY_TEXT_LEVEL`) over a picture (blended by its alpha) stretched from `picture_min_mask_px` to
 * `picture_max_excl_mask_px` in mask pixels (none when that box is empty), the wand (`shade_wand`, 2 × 2 rays a pixel)
 * when `has_wand` is set (or the revolver from viewmodel.rs, `revolver`, when it is `REVOLVER_WAND`), the minimap
 * (`draw_minimap`, from `minimap` and the map's picture `minimap_map`), and `rects` over that: first all their shadows
 * and glows (`get_rect_halo`, the strongest of each), then the rectangles themselves blended in order (`get_rect_layer`),
 * then the bars' pulses (`get_pulse_glow`: tinting toward their colour and adding `RECT_PULSE_LIGHT_GAIN` of it as
 * light) (render_metal.rs's `Overlay`: e.g. "YOU DIED" and a countdown, and the
 * energy bar).
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per output pixel.
 * @endcode
 */
kernel void apply_overlay(texture2d<float, access::read> frame [[texture(0)]], texture2d<float, access::read> revolver [[texture(5)]], texture2d<float, access::read> mask [[texture(1)]],
                          texture2d<float, access::write> out [[texture(2)]], constant OverlayParams &p [[buffer(0)]],
                          constant OverlayRect *rects [[buffer(1)]], constant OverlayWand &wand [[buffer(2)]],
                          texture2d<float, access::read> history_in [[texture(3)]],
                          texture2d<float, access::write> history_out [[texture(4)]],
                          texture2d<float, access::read> picture [[texture(6)]], constant Minimap &minimap [[buffer(3)]],
                          texture2d<float, access::read> minimap_map [[texture(7)]], uint2 id [[thread_position_in_grid]]) {
    if (id.x >= out.get_width() || id.y >= out.get_height()) return;
    float3 scene = mix(read_stretched_scene(frame, id, p), history_in.read(id).rgb, p.blur_frac);
    history_out.write(float4(scene, 1.0), id);
    float4 color = float4(blueshift_scene(scene, id, float2(out.get_width(), out.get_height()), p) * (1.0 - p.dim_frac), 1.0);
    int2 offset = int2(id) - p.origin;
    int2 picture_size_mask_px = p.picture_max_excl_mask_px - p.picture_min_mask_px;
    if (all(picture_size_mask_px > int2(0)) && all(offset >= int2(0))) {
        float2 picture_frac = (float2(offset) / float(p.scale) - float2(p.picture_min_mask_px)) / float2(picture_size_mask_px);
        if (all(picture_frac >= 0.0) && all(picture_frac < 1.0)) {
            float4 pixel = picture.read(uint2(picture_frac * float2(picture.get_width(), picture.get_height())));
            color.rgb = mix(color.rgb, pixel.rgb, pixel.a);
        }
    }
    if (p.has_mask != 0 && all(offset >= int2(0))) {
        uint2 cell = uint2(offset / p.scale);
        if (cell.x < mask.get_width() && cell.y < mask.get_height()) {
            float level = mask.read(cell).r;
            if (level > OVERLAY_TITLE_LEVEL) color = OVERLAY_TITLE_COLOR;
            else if (level > OVERLAY_TEXT_LEVEL) color = OVERLAY_TEXT_COLOR;
        }
    }
    float2 size = float2(out.get_width(), out.get_height());
    if (p.has_wand == REVOLVER_WAND) {
        float4 held = revolver.read(id);
        color.rgb = color.rgb * (1.0 - held.a) + held.rgb;
    } else if (p.has_wand != 0) {
        float3 glow = float3(0.0);
        float4 sum = float4(0.0);
        for (int k = 0; k < WAND_SAMPLES_PER_SIDE * WAND_SAMPLES_PER_SIDE; k++) {
            float2 sub = (float2(k % WAND_SAMPLES_PER_SIDE, k / WAND_SAMPLES_PER_SIDE) + 0.5) / float(WAND_SAMPLES_PER_SIDE);
            float2 ndc = float2(2.0, -2.0) * ((float2(id) + sub) / size) + float2(-1.0, 1.0);
            float4 ray_color = shade_wand(normalize(float3(ndc.x * p.tan_half_fov * size.x / size.y, ndc.y * p.tan_half_fov, -1.0)), wand);
            if (ray_color.a > 0.0) sum += ray_color;
            else glow += ray_color.rgb;
        }
        float samples = float(WAND_SAMPLES_PER_SIDE * WAND_SAMPLES_PER_SIDE);
        color.rgb = mix(color.rgb + glow / samples, sum.a > 0.0 ? sum.rgb / sum.a : color.rgb, sum.a / samples);
    }
    color.rgb = draw_minimap(color.rgb, id, minimap, minimap_map);
    float2 at_px = float2(id) + 0.5;
    float shadow_a = 0.0;
    float3 glow = float3(0.0);
    for (int i = 0; i < p.rect_count; i++) {
        float4 halo = get_rect_halo(rects[i], at_px, size);
        shadow_a = max(shadow_a, halo.a);
        glow = max(glow, halo.rgb);
    }
    color.rgb = color.rgb * (1.0 - shadow_a) + glow;
    for (int i = 0; i < p.rect_count; i++) {
        float4 layer = get_rect_layer(rects[i], at_px, size);
        color.rgb = color.rgb * (1.0 - layer.a) + layer.rgb;
    }
    for (int i = 0; i < p.rect_count; i++) {
        float pulse_glow = get_pulse_glow(rects[i], at_px, size);
        float3 pulse_rgb = rects[i].pulse.color.rgb;
        color.rgb = mix(color.rgb, pulse_rgb, pulse_glow) + pulse_rgb * pulse_glow * RECT_PULSE_LIGHT_GAIN;
    }
    out.write(color, id);
}
