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
    int padding[5];
};

/** @brief `Scene` is one frame's rows and uniforms, with values every ray reuses. */
struct Scene {
    device const float4 *rows;
    constant Uniforms &u;
    float q_cutoff;
    float q_throat;
    float tile_klein_half_width;
    float4 tile_edges[4];
};

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
 * @brief `trace_pixels` traces one pixel's light ray backwards from the eye and writes its colour, as
 * `render_cpu.rs`'s `render_pixel` does.
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per pixel.
 * @endcode
 */
kernel void trace_pixels(device const float4 *rows [[buffer(0)]], constant Uniforms &u [[buffer(1)]],
                         texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    float inradius = acosh(cos(M_PI_F / TILES_PER_VERTEX) / sin(M_PI_F / TILE_SIDE_COUNT));
    float tile_cosh = cosh(inradius), tile_sinh = sinh(inradius);
    float half_cutoff = sinh(0.5 * u.bend_cutoff_m / u.curvature_radius_m);
    float half_throat = sinh(0.25 * log(1.0 + 2.0 * u.throat_m / u.curvature_radius_m));
    Scene scene = {rows, u, 4.0 * half_cutoff * half_cutoff, 4.0 * half_throat * half_throat, tanh(inradius),
        {float4(tile_cosh, 0.0, 0.0, tile_sinh), float4(-tile_cosh, 0.0, 0.0, tile_sinh), float4(0.0, 0.0, tile_cosh, tile_sinh), float4(0.0, 0.0, -tile_cosh, tile_sinh)}};
    float2 ndc = float2(2.0 * (float(id.x) + 0.5) / float(u.width_px) - 1.0, 1.0 - 2.0 * (float(id.y) + 0.5) / float(u.height_px));
    float r = u.curvature_radius_m, max_travel_m = MAX_TRAVEL_PER_R * r;
    float4 x = float4(0.0, 0.0, 0.0, 1.0);
    float4 v = float4(normalize(float3(ndc.x * u.tan_half_fov * u.aspect, ndc.y * u.tan_half_fov, -1.0)), 0.0);
    int row = 0;
    float travelled_m = 0.0, core = 0.0, halo = 0.0;
    BoxCache cache;
    reset_box_cache(scene, cache, x, 0.0);
    bool has_beam = u.beam_chunk_count > 0;
    float3 color = LOST_COLOR;
    for (int step = 0; step < MAX_STEP_COUNT; step++) {
        float q_nearest;
        float4 bend = get_bend(scene, row, x, v, q_nearest);
        if (q_nearest < scene.q_throat && cross_throat(scene, row, x, v)) {
            reset_box_cache(scene, cache, x, travelled_m);
            bend = get_bend(scene, row, x, v, q_nearest);
        }
        bool is_far = q_nearest > scene.q_cutoff;
        float4 vh = v;
        float tau_max, ds_m;
        if (is_far) {
            tau_max = get_cutoff_entry_tau(scene, row, x, v);
            ds_m = tau_max < 1.0 ? atanh(tau_max) * r : max_travel_m;
        } else {
            float nearest_m = get_distance_of_q(q_nearest) * r;
            ds_m = max(nearest_m * (nearest_m < NEAR_ZONE_PER_THROAT * u.throat_m ? NEAR_STEP_PER_NEAREST : FAR_STEP_PER_NEAREST), MIN_STEP_M);
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
            color = shade_hit(scene, row, hit, point);
            if (u.fog_m > 0.0) color = mix(HAZE_COLOR, color, exp(-end_m / u.fog_m));
            float spot = get_spot_glow(scene, row, point, end_m);
            halo = max(halo, spot);
            core = max(core, spot * spot);
            break;
        }
        if (tau_max >= 1.0 || travelled_m > max_travel_m) {
            if (has_beam) add_beam_glow(scene, row, x, x + vh, travelled_m, travelled_m + max_travel_m, core, halo);
            color = get_sky_color(scene, get_sky_direction(scene, fetch_matrix(scene, row, TEXEL_TO_WORLD) * (x + vh)));
            break;
        }
        float4 x_before = x;
        if (is_far) {
            advance_geodesic(x, v, atanh(tau_max));
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
    color = mix(color, LASER_HALO_COLOR, clamp(0.9 * halo, 0.0, 1.0)) + LASER_CORE_COLOR * core;
    color = color / (1.0 + max(color - 1.0, 0.0) * HIGHLIGHT_ROLLOFF);
    image.write(float4(clamp(color, 0.0, 1.0), 1.0), id);
}
