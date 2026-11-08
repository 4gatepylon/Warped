/**
 * @file render_voxel.metal
 * @brief `render_voxel.metal` ray-traces voxel maps of the portable map format (`common/maps/`) in H³, one thread
 * per pixel, as the Lunar Surface level's WebGL fragment shader does (`demos/levels/lunar_surface/index.html`):
 * straight geodesic chords between warp objects, kick–drift–kick steps inside their reach, a voxel walk through each
 * structure in its Klein coordinates, the ground's folded {4,5} tiling, lunar-Lambert light with sun shadows, lamps,
 * earthshine, laser beams, stars and a planet. Everything here belongs to the `lunar` appearance style.
 * @par Example
 * @code
 * dispatched by render_voxel.rs, one thread per pixel of the render size.
 * @endcode
 */
#include <metal_stdlib>
using namespace metal;

#define SCENE_BUFFERS constant VoxelUniforms &u [[buffer(16)]], device const VoxelStructure *structures [[buffer(17)]], \
    device const uchar *voxels [[buffer(18)]], device const VoxelWarp *warps [[buffer(19)]], device const float4 *warp_lists [[buffer(20)]], \
    device const VoxelLight *lights [[buffer(21)]], device const float4 *palette [[buffer(22)]], device const BeamSegment *beams [[buffer(23)]], \
    device const uchar *clearances [[buffer(24)]]

constant int MAX_ITERATIONS = 240;
constant int MAX_DDA_STEPS = 80;
constant int MAX_FOLDS = 26;
constant int OUTCOME_LOST = 0;
constant int OUTCOME_SKY = 1;
constant int OUTCOME_HIT = 2;
constant int MAT_GROUND = 255;
constant int NO_MATERIAL = -1;
constant float NO_HIT_S = 1.0e9;
constant float LAMBDA_LIMIT = 0.99999994;
constant float TINY = 1.0e-9;
constant float FAR_S = 12.0;
constant float SHADOW_M = 80.0;
constant float SURFACE_OFFSET_M = 0.02;
constant float FOIL_LAMBDA_SQ = 0.25;
constant float HORIZON_CAPTURE_FRAC = 1.02;
constant float BEND_STEP_FRAC = 0.08;
constant int LATE_STEP_COUNT = 90;
constant float LATE_BEND_STEP_FRAC = 0.24;
constant float MIN_BEND_STEP_M = 0.004;
constant float MAX_BEND_STEP_M = 3.0;
constant float GROUND_DETAIL_S = 7.0;
constant float SUN_INTENSITY = 1.45;
constant float LUNAR_LAMBERT_FRAC = 0.7;
constant float AMBIENT = 0.11;
constant float EARTHSHINE = 0.08;
constant float SHADOW_FLOOR = 0.0;
constant float3 EARTHSHINE_COLOR = float3(0.55, 0.7, 1.0);
constant float LIGHT_CUTOFF_FRAC = 3.0;
constant float LIGHT_WRAP = 0.3;
constant float BLOCK_NOISE = 0.18;
constant float TEXEL_NOISE = 0.12;
constant float EDGE_DARKEN = 0.55;
constant float EDGE_WIDTH = 0.05;
constant float SUBTEXELS = 8.0;
constant float DETAIL_FOOTPRINT_M = 0.9;
constant float GLASS_TINT = 0.8;
constant float3 GLASS_SHEEN = float3(0.05, 0.07, 0.09);
constant float BEAM_RADIUS_M = 0.02;
constant float BEAM_REACH = 4.8;
constant float SPOT_RADIUS_M = 0.15;
constant float3 HASH_DIRECTION = float3(12.9898, 78.233, 37.719);
constant float HASH_SCALE = 43758.5453;
constant int FBM_OCTAVES = 4;
constant float FBM_LACUNARITY = 2.03;
constant float STAR_DENSITY_COARSE = 0.03;
constant float STAR_DENSITY_FINE = 0.012;
constant float STAR_PIXELS = 0.8;
constant float STAR_GRID_COARSE = 60.0;
constant float STAR_GRID_FINE = 170.0;
constant float3 MILKY_WAY_NORMAL = float3(0.35, 0.55, 0.76);
constant float MILKY_WAY_WIDTH = 0.2;
constant float SUN_COS_INNER = 0.99996;
constant float SUN_COS_OUTER = 0.99990;
constant float SUN_GLARE_POWER = 900.0;
constant float SUN_DISC_BRIGHTNESS = 6.0;
constant float3 OCEAN_COLOR = float3(0.07, 0.2, 0.48);
constant float3 LAND_COLOR = float3(0.32, 0.38, 0.2);
constant float3 CLOUD_COLOR = float3(0.95);
constant float3 ATMOSPHERE_COLOR = float3(0.35, 0.6, 1.0);
constant float CRATER_FLOOR_RADIUS_M = 19.0;
constant float CRATER_RIM_RADIUS_M = 24.0;
constant float CRATERLET_MAX_M = 6.0;
constant float CRATERLET_CHANCE = 0.55;
constant float EJECTA_INNER_M = 27.0;
constant float EJECTA_OUTER_M = 80.0;
constant float ZONE_STRIP_M = 0.8;
constant float ZONE_TINT_GAIN = 1.5;
constant float ZONE_TINT_FRAC = 0.07;
constant float BOUNDARY_STRIP_FRAC = 0.04;
constant float CELL_HASH_SCALE = 4096.0;
constant float HASH_SHRINK = 0.001;
constant float MARIA_BASE = 0.93;
constant float MARIA_CONTRAST = 0.12;
constant float MARIA_PER_M = 0.03;
constant float FLOOR_RING_M = 3.0;
constant float FLOOR_RING_DARKEN = 0.25;
constant float EJECTA_RAY_COUNT = 9.0;
constant float2 EJECTA_RAY_EDGES = float2(0.62, 0.8);
constant float3 EJECTA_COLOR = float3(0.78, 0.78, 0.76);
constant float EJECTA_STRENGTH = 0.6;
constant float CRATERLET_CLEARANCE_M = 12.0;
constant float CRATERLET_SPACING_FRAC = 0.5;
constant float CRATERLET_HASH_SCALE = 512.0;
constant float CRATERLET_MIN_FRAC = 0.35;
constant float CRATERLET_SIZE_SEED = 7.31;
constant float CRATERLET_RIM_FRAC = 0.75;
constant float CRATERLET_RIM_BRIGHTEN = 1.18;
constant float CRATERLET_FLOOR_DARKEN = 0.72;
constant float CRATERLET_FLOOR_SLOPE = 0.1;
constant float ZONE_STRIP_GLOW = 0.55;
constant float OUTSIDE_DARKEN = 0.72;
constant float HAZARD_STRIPE_M = 1.5;
constant float3 HAZARD_DARK_COLOR = float3(0.1);
constant float GROUND_EDGE_FRAC = 0.6;
constant float CELL_SEED = 97.0;
constant float STRUCT_SEED = 17.31;
constant float TEXEL_SEED = 131.0;
constant float CONSOLE_STRIPE_DARKEN = 0.25;
constant float STRIPES_PER_BLOCK = 2.0;
constant float3 SOLAR_GRID_COLOR = float3(0.45, 0.5, 0.62);
constant float2 SOLAR_CELLS = float2(4.0, 2.0);
constant float SOLAR_GRID_EDGE = 0.88;
constant float3 HAZARD_STRIPE_COLOR = float3(0.08);
constant float FOIL_CRINKLES = 5.0;
constant float FOIL_VARIATION = 0.4;
constant float PANEL_SEAMS = 2.0;
constant float PANEL_SEAM_EDGE = 0.94;
constant float PANEL_SEAM_DARKEN = 0.12;
constant float GLOW_BRIGHTNESS = 1.2;
constant float GLOW_RIM = 0.3;
constant float DETAIL_FADE_START_FRAC = 0.3;
constant float LIGHT_FADE_START_FRAC = 0.6;
constant float DDA_NUDGE = 1.0e-5;
constant float HORIZON_GUARD = 1.0e-4;
constant float SUN_GLARE = 0.6;
constant float SUN_HALO_POWER = 40.0;
constant float SUN_HALO = 0.04;
constant float2 LAND_EDGES = float2(0.52, 0.56);
constant float LAND_SCALE = 2.2;
constant float3 LAND_OFFSET = float3(3.0);
constant float2 CLOUD_EDGES = float2(0.5, 0.75);
constant float CLOUD_SCALE = 4.5;
constant float3 CLOUD_OFFSET = float3(9.0, 1.0, 4.0);
constant float PLANET_SUNLIT = 1.15;
constant float PLANET_NIGHT = 0.015;
constant float LIMB_POWER = 6.0;
constant float LIMB_GLOW = 0.6;
constant float LIMB_WRAP = 0.2;
constant float HALO_RADIUS_SQ = 1.12;
constant float HALO_GLOW = 0.25;
constant float HALO_WRAP = 0.3;
constant float STAR_FAINT = 0.15;
constant float STAR_BRIGHT = 1.5;
constant float STAR_BRIGHTNESS_POWER = 4.0;
constant float STAR_JITTER = 0.6;
constant float2 STAR_SEEDS = float2(7.1, 3.7);
constant float FINE_STAR_BRIGHTNESS = 0.5;
constant float3 MILKY_WAY_COLOR = float3(0.75, 0.8, 1.0);
constant float MILKY_WAY_BRIGHTNESS = 0.05;
constant float MILKY_WAY_DETAIL = 6.0;
constant float BEAM_HALO_OPACITY = 0.9;
constant float BEAM_CORE_WHITENESS = 0.6;
constant float TONEMAP_KNEE = 0.6;
constant float3 BLUE_COLOR = float3(0.30, 0.62, 1.0);
constant float3 RED_COLOR = float3(1.0, 0.16, 0.08);
constant float3 SUN_COLOR = float3(1.0, 0.96, 0.88);
constant float3 FLASHLIGHT_COLOR = float3(1.0, 0.95, 0.85);
constant int TILE_CELLS_PER_HALF = 18;
constant float KIND_BLACK_HOLE = 0.0;
constant float HALF_KIND = 0.5;
constant float GLOW_FLOOR = 1.0 / 1024.0;
constant float SMOOTH_SECOND_DIFFERENCE_PER_FIRST = 1.6;
constant int REFINED_PIXELS_PER_CELL = 3;
constant int REFINE_GROUP_SIZE = __REFINE_GROUP_SIZE__;

/**
 * @brief `VoxelUniforms` is one frame's camera and the map's counts, special materials, sky bodies, laser spot and
 * regions, as `render_voxel.rs` writes them.
 */
struct VoxelUniforms {
    float4x4 camera;
    float tan_half_fov;
    float aspect;
    float pixel_angle_rad;
    float curvature_radius_m;
    int structure_count;
    int warp_count;
    int light_count;
    int beam_count;
    int width_px;
    int height_px;
    int mat_glass;
    int mat_regolith;
    int mat_dark_regolith;
    int mat_basalt;
    int mat_solar;
    int mat_hazard;
    int mat_pad;
    int mat_hull;
    int mat_gold_foil;
    int mat_console_first;
    int mat_console_count;
    int has_regions;
    int padding[2];
    float4 sun;
    float4 planet;
    float4 planet_e1;
    float4 planet_e2;
    float4 spot;
    float4 spot_color;
    float4 regions;
    float4 flashlight_position;
    float4 flashlight_direction;
    float4 flashlight;
};

/**
 * @brief `VoxelStructure` is one structure: the rows of its inverse frame, its bounding sphere's centre, cosh of its
 * radius with the grid's origin, its size with the offset of its cells, and R over its cell size.
 */
struct VoxelStructure {
    float4 rows[4];
    float4 centre;
    float4 bound;
    int4 size;
    float4 scale;
};

/**
 * @brief `VoxelWarp` is one warp object: its inverse and forward frames, its centre, its kind, m and reach (curvature
 * units) and partner, where its list of structures within reach starts, how long it is, how many of its entries are
 * static structures (avatars follow) and where its clearance grid starts, and the grid's half size, cells per axis,
 * cells per unit and curvature radii per stored unit (`voxel_clearance.rs`). Each list entry is (structure, nearest
 * and farthest distance of its bounding sphere from the centre, unused).
 */
struct VoxelWarp {
    float4x4 inverse;
    float4x4 forward;
    float4 centre;
    float4 params;
    int4 list;
    float4 grid;
};

/** @brief `VoxelLight` is a lamp: its position, and its colour with its radius in meters. */
struct VoxelLight {
    float4 position;
    float4 color_radius;
};

/** @brief `BeamSegment` is one geodesic piece of a laser: start, unit tangent, and (length, unused, brightness, team). */
struct BeamSegment {
    float4 a;
    float4 b;
    float4 info;
};

/** @brief `Hit` is the first surface along a chord: distance, material, structure (-1 for the ground), face, grid point. */
struct Hit {
    float s;
    int mat;
    int k;
    int axis;
    float sgn;
    float3 g;
    int3 cell;
};

/**
 * @brief `VoxelRayEnd` is where a ray ended, with what shading needs: the hit point (or the sky direction at the
 * origin, with w = 0), the ray's unit tangent there, the hit's grid point in its structure with the distance
 * travelled, the hit cell with its material (`MAT_GROUND` for the ground), the outcome with the structure (-1 for the
 * ground), face axis and laser team (1 for red), and the face's side with the glass count and the laser's core and
 * halo.
 */
struct VoxelRayEnd {
    float4 point;
    float4 view;
    float4 grid;
    int4 cell;
    int4 surface;
    float4 glow;
};
static_assert(sizeof(VoxelRayEnd) == __BYTES_PER_RAY_END__, "render_voxel.rs sizes the sample buffers by VOXEL_BYTES_PER_RAY_END");

/** @brief `Scene` bundles a frame's buffers for the functions below. */
struct Scene {
    constant VoxelUniforms &u;
    device const VoxelStructure *structures;
    device const uchar *voxels;
    device const VoxelWarp *warps;
    device const float4 *warp_lists;
    device const VoxelLight *lights;
    device const float4 *palette;
    device const BeamSegment *beams;
    device const uchar *clearances;
};

/** @brief `mdot` is the Minkowski product. @par Example @code float c = mdot(a, b); @endcode */
static float mdot(float4 a, float4 b) { return dot(a.xyz, b.xyz) - a.w * b.w; }

/** @brief `geodesic_point` gives cosh(s) x + sinh(s) v. @par Example @code float4 y = geodesic_point(x, v, 0.1); @endcode */
static float4 geodesic_point(float4 x, float4 v, float s) { return cosh(s) * x + sinh(s) * v; }

/** @brief `geodesic_tangent` gives sinh(s) x + cosh(s) v. @par Example @code float4 w = geodesic_tangent(x, v, 0.1); @endcode */
static float4 geodesic_tangent(float4 x, float4 v, float s) { return sinh(s) * x + cosh(s) * v; }

/** @brief `normalize_tangent` projects v on the tangent space at x and makes it unit. @par Example @code v = normalize_tangent(x, v); @endcode */
static float4 normalize_tangent(float4 x, float4 v) { v += mdot(v, x) * x; return v * rsqrt(max(mdot(v, v), TINY)); }

/** @brief `clamp_atanh` is atanh with its argument kept inside (-1, 1). @par Example @code float s = clamp_atanh(t); @endcode */
static float clamp_atanh(float t) { return atanh(clamp(t, -LAMBDA_LIMIT, LAMBDA_LIMIT)); }

/** @brief `measure_gap` gives the distance between nearby points from their chord. @par Example @code float d = measure_gap(a, b); @endcode */
static float measure_gap(float4 a, float4 b) { float4 d = a - b; return 2.0 * asinh(0.5 * sqrt(max(mdot(d, d), 0.0))); }

/** @brief `transport_from_centre` carries a direction at the world origin to x by parallel transport. @par Example @code float4 l = transport_from_centre(sun, x); @endcode */
static float4 transport_from_centre(float3 d, float4 x) { return float4(d, 0.0) + dot(x.xyz, d) / (1.0 + x.w) * (float4(0.0, 0.0, 0.0, 1.0) + x); }

/** @brief `hash_3d` gives a pseudo-random number in [0, 1) for a 3D point. @par Example @code float h = hash_3d(p); @endcode */
static float hash_3d(float3 p) { return fract(sin(dot(p, HASH_DIRECTION)) * HASH_SCALE); }

/** @brief `sample_noise_3d` is smooth value noise in [0, 1]. @par Example @code float n = sample_noise_3d(p); @endcode */
static float sample_noise_3d(float3 p) {
    float3 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(hash_3d(i), hash_3d(i + float3(1, 0, 0)), f.x), mix(hash_3d(i + float3(0, 1, 0)), hash_3d(i + float3(1, 1, 0)), f.x), f.y),
               mix(mix(hash_3d(i + float3(0, 0, 1)), hash_3d(i + float3(1, 0, 1)), f.x), mix(hash_3d(i + float3(0, 1, 1)), hash_3d(i + float3(1, 1, 1)), f.x), f.y), f.z);
}

/** @brief `sample_fbm_3d` sums octaves of value noise. @par Example @code float n = sample_fbm_3d(p); @endcode */
static float sample_fbm_3d(float3 p) {
    float sum = 0.0, amplitude = 0.5;
    for (int i = 0; i < FBM_OCTAVES; i++) { sum += amplitude * sample_noise_3d(p); p *= FBM_LACUNARITY; amplitude *= 0.5; }
    return sum;
}

/** @brief `get_palette` gives a material's colour and glow, black for one outside the palette. @par Example @code float4 c = get_palette(scene, m); @endcode */
static float4 get_palette(thread const Scene &scene, int m) { return m >= 0 ? scene.palette[m] : float4(0.0); }

/**
 * @brief `trace_structure` finds where a chord first enters a solid cell of structure k, by a voxel walk along the
 * chord's straight line in the structure's Klein coordinates (taken at the chord's closest approach for precision);
 * glass is noted in `glass_s` (and ignored by shadow rays).
 * @par Example
 * @code
 * trace_structure(scene, k, x, v, 0.0, false, hit, glass_s);
 * @endcode
 */
static void trace_structure(thread const Scene &scene, int k, float4 x, float4 v, float s_min, bool is_shadow, thread Hit &hit, thread float &glass_s) {
    device const VoxelStructure &st = scene.structures[k];
    float4 c = st.centre, info = st.bound;
    float a = -mdot(x, c), b = mdot(v, c), closest_sq = a * a - b * b;
    if (closest_sq >= info.x * info.x) return;
    float s_star = clamp_atanh(b / a), half_window = acosh(info.x * rsqrt(max(closest_sq, 1.0)));
    float s_lo = max(s_min, s_star - half_window), s_hi = min(hit.s, s_star + half_window);
    if (s_lo >= s_hi) return;
    float4 xs = geodesic_point(x, v, s_star), vs = geodesic_tangent(x, v, s_star);
    float4 p = float4(dot(st.rows[0], xs), dot(st.rows[1], xs), dot(st.rows[2], xs), dot(st.rows[3], xs));
    float4 t = float4(dot(st.rows[0], vs), dot(st.rows[1], vs), dot(st.rows[2], vs), dot(st.rows[3], vs));
    float lam_lo = tanh(s_lo - s_star), lam_hi = tanh(s_hi - s_star);
    float mu_lo = lam_lo / (p.w * (p.w + lam_lo * t.w)), mu_hi = lam_hi / (p.w * (p.w + lam_hi * t.w));
    float scale = st.scale.x;
    float3 g0 = p.xyz / p.w * scale + info.yzw, d = (t.xyz * p.w - p.xyz * t.w) * scale;
    d = select(d, float3(TINY), abs(d) < TINY);
    float3 inv_d = 1.0 / d, size = float3(st.size.xyz);
    float3 m0 = -g0 * inv_d, m1 = (size - g0) * inv_d, m_near = min(m0, m1), m_far = max(m0, m1);
    float mu_in = max(max(m_near.x, m_near.y), m_near.z);
    int axis = mu_in == m_near.x ? 0 : (mu_in == m_near.y ? 1 : 2);
    if (mu_lo > mu_in) { mu_in = mu_lo; axis = -1; }
    float mu_out = min(min(min(m_far.x, m_far.y), m_far.z), mu_hi);
    if (mu_in >= mu_out) return;
    int3 dims = st.size.xyz, step_dir = int3(sign(d));
    int3 cell = clamp(int3(floor(g0 + d * mu_in + sign(d) * DDA_NUDGE)), int3(0), dims - 1);
    float3 mu_next = (float3(cell) + step(float3(0.0), d) - g0) * inv_d, mu_delta = abs(inv_d);
    float mu = mu_in;
    bool in_glass = false;
    device const uchar *cells = scene.voxels + st.size.w;
    for (int i = 0; i < MAX_DDA_STEPS; i++) {
        int m = int(cells[cell.x + dims.x * (cell.y + dims.y * cell.z)]);
        if (m != 0) {
            float lam = mu * p.w * p.w / (1.0 - mu * p.w * t.w), s_hit = s_star + clamp_atanh(lam);
            if (m == scene.u.mat_glass && !is_shadow) {
                if (!in_glass) glass_s = min(glass_s, s_hit);
                in_glass = true;
            } else if (m != scene.u.mat_glass) {
                if (s_hit < hit.s) {
                    hit.s = s_hit; hit.mat = m; hit.k = k; hit.axis = axis;
                    hit.sgn = axis >= 0 ? -float(step_dir[axis]) : 0.0; hit.g = g0 + d * mu; hit.cell = cell;
                }
                return;
            }
        } else {
            in_glass = false;
        }
        if (mu_next.x < mu_next.y && mu_next.x < mu_next.z) { axis = 0; mu = mu_next.x; cell.x += step_dir.x; mu_next.x += mu_delta.x; }
        else if (mu_next.y < mu_next.z) { axis = 1; mu = mu_next.y; cell.y += step_dir.y; mu_next.y += mu_delta.y; }
        else { axis = 2; mu = mu_next.z; cell.z += step_dir.z; mu_next.z += mu_delta.z; }
        if (mu > mu_out || any(cell < int3(0)) || any(cell >= dims)) return;
    }
}

/** @brief `trace_ground` intersects a chord with the ground y = 0. @par Example @code trace_ground(x, v, hit); @endcode */
static void trace_ground(float4 x, float4 v, thread Hit &hit) {
    if (x.y < 0.0 || v.y >= 0.0) return;
    float lam = x.y / -v.y;
    if (lam >= LAMBDA_LIMIT) return;
    float s = atanh(lam);
    if (s < hit.s) { hit.s = s; hit.mat = MAT_GROUND; hit.k = -1; }
}

/**
 * @brief `is_clear_of_blocks` says whether no static structure's solid cell is within `s` of the point whose spatial
 * part in warp w's frame is `local`, so a chord of length `s` from it can't reach one: its clearance grid cell's value
 * less its chart distance from the cell's centre (the metric is at most |dp|) is at least `s`. False outside the grid.
 * @par Example
 * @code
 * if (is_clear_of_blocks(scene, w, local, hit.s)) first = list.z;
 * @endcode
 */
static bool is_clear_of_blocks(thread const Scene &scene, int w, float3 local, float s) {
    device const VoxelWarp &warp = scene.warps[w];
    float4 grid = warp.grid;
    int n = int(grid.y);
    float3 g = (local + grid.x) * grid.z;
    int3 c = int3(floor(g));
    if (any(c < int3(0)) || any(c >= int3(n))) return false;
    float off_centre = length(g - float3(c) - 0.5) / grid.z;
    return float(scene.clearances[warp.list.w + c.x + n * (c.y + n * c.z)]) * grid.w - off_centre >= s;
}

/**
 * @brief `trace_scene` finds the first surface along a chord: the ground and every structure, or the structures in a
 * warp's list whose distance from its centre the chord can reach (the chord starts `rho` from the centre and is
 * `hit.s` long, so it stays within [rho - hit.s, rho + hit.s]), only its avatars where the warp's clearance grid shows
 * no static block within reach.
 * @par Example
 * @code
 * trace_scene(scene, x, v, -1, 0.0, float3(0.0), hit, glass_s);
 * @endcode
 */
static void trace_scene(thread const Scene &scene, float4 x, float4 v, int warp, float rho, float3 local, thread Hit &hit, thread float &glass_s) {
    trace_ground(x, v, hit);
    if (warp < 0) {
        for (int k = 0; k < scene.u.structure_count; k++) trace_structure(scene, k, x, v, 0.0, false, hit, glass_s);
    } else {
        int4 list = scene.warps[warp].list;
        float reach = hit.s;
        for (int j = is_clear_of_blocks(scene, warp, local, reach) ? list.z : 0; j < list.y; j++) {
            float4 entry = scene.warp_lists[list.x + j];
            if (rho + reach < entry.y || rho - reach > entry.z) continue;
            trace_structure(scene, int(entry.x), x, v, 0.0, false, hit, glass_s);
        }
    }
}

/** @brief `trace_shadow` gives 1 if nothing blocks the straight path toward the sun within `SHADOW_M`, else 0. @par Example @code float lit = trace_shadow(scene, x, l); @endcode */
static float trace_shadow(thread const Scene &scene, float4 x, float4 l) {
    Hit h;
    h.s = SHADOW_M / scene.u.curvature_radius_m;
    h.mat = 0;
    float glass_s = NO_HIT_S;
    for (int k = 0; k < scene.u.structure_count; k++) {
        trace_structure(scene, k, x, l, 0.0, true, h, glass_s);
        if (h.mat != 0) return SHADOW_FLOOR;
    }
    return 1.0;
}

/** @brief `get_index_slope` gives d ln n / dρ of a warp's index (isotropic Schwarzschild or foil). @par Example @code float s = get_index_slope(scene, w, rho); @endcode */
static float get_index_slope(thread const Scene &scene, int w, float rho) {
    float4 prm = scene.warps[w].params;
    if (prm.x < HALF_KIND) { float m = prm.y, q = m / rho; return -m / (rho * rho) * (3.0 / (1.0 + q) + 1.0 / max(1.0 - q, HORIZON_GUARD)); }
    float pt = prm.y, ir = 1.0 / rho, psi = 1.0 + pt * ir, ir2 = ir / (psi * psi), rs = 4.0 * pt;
    return -2.0 * pt * ir * ir / psi - rs * psi * (1.0 - pt * ir) * ir2 * ir2 / (2.0 * (1.0 + FOIL_LAMBDA_SQ - rs * ir2));
}

/** @brief `get_log_index` gives ln n of a warp's index. @par Example @code float ln_n = get_log_index(scene, w, rho); @endcode */
static float get_log_index(thread const Scene &scene, int w, float rho) {
    float4 prm = scene.warps[w].params;
    if (prm.x < HALF_KIND) { float q = prm.y / rho; return 3.0 * log(1.0 + q) - log(max(1.0 - q, HORIZON_GUARD)); }
    float pt = prm.y, psi = 1.0 + pt / rho, r = rho * psi * psi;
    return 2.0 * log(psi) - 0.5 * log((1.0 - 4.0 * pt / r + FOIL_LAMBDA_SQ) / (1.0 + FOIL_LAMBDA_SQ));
}

/**
 * @brief `compute_gradient` gives ∇ln n of warp w at x (not yet projected across a direction), ln n tapered by
 * (1 - (ρ/reach)²)², with x's distance ρ from the warp's centre and its spatial part `local` in the warp's frame.
 * @par Example
 * @code
 * float4 g = compute_gradient(scene, w, x, rho, local);
 * @endcode
 */
static float4 compute_gradient(thread const Scene &scene, int w, float4 x, thread float &rho, thread float3 &local) {
    device const VoxelWarp &warp = scene.warps[w];
    float4 p = warp.inverse * x;
    local = p.xyz;
    rho = asinh(length(p.xyz));
    float ch = cosh(rho), sh = max(sinh(rho), TINY), reach = warp.params.z;
    float f = rho / reach, window = (1.0 - f * f) * (1.0 - f * f), window_slope = -4.0 * f * (1.0 - f * f) / reach;
    float slope = f >= 1.0 ? 0.0 : get_index_slope(scene, w, rho) * window + get_log_index(scene, w, rho) * window_slope;
    return slope * (ch * x - warp.centre) / sh;
}

/** @brief `cross_throat` carries a ray inside mouth w's throat to its partner (inversion, then diag(1, 1, -1)). @par Example @code cross_throat(scene, w, x, v); @endcode */
static void cross_throat(thread const Scene &scene, int w, thread float4 &x, thread float4 &v) {
    device const VoxelWarp &warp = scene.warps[w];
    float4 prm = warp.params, p = warp.inverse * x, t = warp.inverse * v;
    int partner = int(prm.w + 0.5);
    float len = max(length(p.xyz), TINY), rho_out = prm.y * prm.y / asinh(len);
    float3 n = p.xyz / len, flip = float3(1.0, 1.0, -1.0);
    float3 tt = t.xyz - t.w / (1.0 + p.w) * p.xyz;
    tt = (tt - 2.0 * dot(tt, n) * n) * flip;
    float4 q = float4(n * flip * sinh(rho_out), cosh(rho_out));
    float g = dot(q.xyz, tt) / (1.0 + q.w);
    x = scene.warps[partner].forward * q;
    v = normalize_tangent(x, scene.warps[partner].forward * float4(tt + g * q.xyz, g * (1.0 + q.w)));
}

/** @brief `gather_lights` sums the lamps' light at a surface point. @par Example @code float3 lamps = gather_lights(scene, x, n); @endcode */
static float3 gather_lights(thread const Scene &scene, float4 x, float4 n) {
    float3 sum = float3(0.0);
    for (int i = 0; i < scene.u.light_count; i++) {
        float4 lx = scene.lights[i].position;
        float d_m = measure_gap(lx, x) * scene.u.curvature_radius_m, radius_m = scene.lights[i].color_radius.w;
        if (d_m > LIGHT_CUTOFF_FRAC * radius_m) continue;
        float4 toward = lx + mdot(lx, x) * x;
        float facing = mdot(n, toward) * rsqrt(max(mdot(toward, toward), TINY));
        float falloff = 1.0 / (1.0 + (d_m / radius_m) * (d_m / radius_m))
                      * (1.0 - smoothstep(LIGHT_FADE_START_FRAC * LIGHT_CUTOFF_FRAC * radius_m, LIGHT_CUTOFF_FRAC * radius_m, d_m));
        sum += scene.lights[i].color_radius.rgb * falloff * (LIGHT_WRAP + (1.0 - LIGHT_WRAP) * max(facing, 0.0));
    }
    return sum;
}

/** @brief `light_surface` lights a surface point: the sun (lunar-Lambert, shadowed), earthshine, lamps and the laser spot. @par Example @code float3 c = light_surface(scene, albedo, x, n, v); @endcode */
/**
 * @brief `light_flashlight` gives the light of the player's flashlight (`flashlight_position`, aimed along
 * `flashlight_direction`; `flashlight` holds the cosines of its cone's inner and outer edges, the distance at which it
 * falls to half, and its brightness, 0 when off) on a surface at x with normal n: full inside the cone, softening to
 * nothing at its outer edge, falling with distance, and as much as the surface faces it.
 * @par Example
 * @code
 * float3 lit = light_flashlight(scene, x, n);
 * @endcode
 */
static float3 light_flashlight(thread const Scene &scene, float4 x, float4 n) {
    float4 p = scene.u.flashlight_position, f = scene.u.flashlight;
    if (f.w <= 0.0) return float3(0.0);
    float4 toward = x + mdot(x, p) * p, back = p + mdot(p, x) * x;
    float cone = smoothstep(f.y, f.x, mdot(toward, scene.u.flashlight_direction) * rsqrt(max(mdot(toward, toward), TINY)));
    float d_m = acosh(max(-mdot(x, p), 1.0)) * scene.u.curvature_radius_m / f.z;
    float facing = max(mdot(n, back) * rsqrt(max(mdot(back, back), TINY)), 0.0);
    return SUN_INTENSITY * FLASHLIGHT_COLOR * f.w * cone * facing / (1.0 + d_m * d_m);
}

static float3 light_surface(thread const Scene &scene, float3 albedo, float4 x, float4 n, float4 v) {
    float4 l = transport_from_centre(scene.u.sun.xyz, x);
    float mu0 = mdot(n, l), mu = max(-mdot(n, v), 0.0), sun = 0.0;
    if (mu0 > 0.0) {
        sun = mix(mu0, 2.0 * mu0 / (mu0 + mu), LUNAR_LAMBERT_FRAC)
            * trace_shadow(scene, geodesic_point(x, n, SURFACE_OFFSET_M / scene.u.curvature_radius_m), l);
    }
    float earth = max(mdot(n, transport_from_centre(scene.u.planet.xyz, x)), 0.0);
    float3 light = float3(AMBIENT) + EARTHSHINE * earth * EARTHSHINE_COLOR + SUN_INTENSITY * sun * SUN_COLOR + gather_lights(scene, x, n)
        + light_flashlight(scene, x, n);
    float3 color = albedo * light;
    if (scene.u.spot_color.a > 0.0) {
        float d_m = measure_gap(scene.u.spot, x) * scene.u.curvature_radius_m;
        color += scene.u.spot_color.rgb * scene.u.spot_color.a * exp(-d_m * d_m / (SPOT_RADIUS_M * SPOT_RADIUS_M));
    }
    return color;
}

/** @brief `detail_fraction` gives how much per-block detail to keep at a distance. @par Example @code float d = detail_fraction(scene, s); @endcode */
static float detail_fraction(thread const Scene &scene, float dist_s) {
    float pixel_m = scene.u.pixel_angle_rad * scene.u.curvature_radius_m * sinh(min(dist_s, FAR_S));
    return 1.0 - smoothstep(DETAIL_FADE_START_FRAC * DETAIL_FOOTPRINT_M, DETAIL_FOOTPRINT_M, pixel_m);
}

/**
 * @brief `shade_ground` shades the ground at x: the {4,5} tiling folded into the central tile gives block cells, and
 * cell centres decide the regolith, crater floor, ejecta, craterlets, zone tints and the boundary.
 * @par Example
 * @code
 * float3 c = shade_ground(scene, x, v, s);
 * @endcode
 */
static float3 shade_ground(thread const Scene &scene, float4 x, float4 v, float dist_s) {
    constant VoxelUniforms &u = scene.u;
    float r = u.curvature_radius_m;
    float tile_klein_half = tanh(acosh(cos(M_PI_F / 5.0) / sin(M_PI_F / 4.0)));
    float tile_normal_scale = rsqrt(1.0 - tile_klein_half * tile_klein_half);
    float tile_inradius_s = atanh(tile_klein_half), cell_s = tile_inradius_s / float(TILE_CELLS_PER_HALF);
    float detail = detail_fraction(scene, dist_s);
    float3 y = float3(x.x, x.z, x.w);
    float3x3 N = float3x3(1.0);
    if (dist_s < GROUND_DETAIL_S) {
        for (int i = 0; i < MAX_FOLDS; i++) {
            float kx = y.x / y.z, kz = y.y / y.z, ex = abs(kx) - tile_klein_half, ez = abs(kz) - tile_klein_half;
            if (max(ex, ez) <= 0.0) break;
            float3 n = (ex > ez ? float3(sign(kx), 0.0, tile_klein_half) : float3(0.0, sign(kz), tile_klein_half)) * tile_normal_scale;
            float3 jn = float3(n.xy, -n.z);
            y -= 2.0 * dot(y, jn) * n;
            float3 Nn = N * n;
            N -= 2.0 * float3x3(Nn * jn.x, Nn * jn.y, Nn * jn.z);
        }
    }
    float2 a = float2(clamp_atanh(y.x / y.z), clamp_atanh(y.y / y.z)) / cell_s;
    float2 cell = floor(a), uv = a - cell, kc = tanh((cell + 0.5) * cell_s);
    float3 centre = N * (float3(kc, 1.0) * rsqrt(max(1.0 - dot(kc, kc), TINY)));
    if (dist_s >= GROUND_DETAIL_S) centre = y;
    float u_m = -r * clamp_atanh(centre.y / centre.z), v_m = r * asinh(centre.x), r_m = acosh(max(centre.z, 1.0)) * r;
    float h = hash_3d(floor(centre * CELL_HASH_SCALE) * HASH_SHRINK);
    float3 albedo = get_palette(scene, u.mat_regolith).rgb * (MARIA_BASE + MARIA_CONTRAST * sample_fbm_3d(float3(u_m, v_m, 0.0) * MARIA_PER_M));
    if (r_m < CRATER_FLOOR_RADIUS_M) {
        albedo = mix(get_palette(scene, u.mat_dark_regolith).rgb, get_palette(scene, u.mat_basalt).rgb, FLOOR_RING_DARKEN * step(0.5, fract(r_m / FLOOR_RING_M)));
    } else if (r_m < EJECTA_OUTER_M) {
        float theta = atan2(centre.x, -centre.y);
        float ray = smoothstep(EJECTA_RAY_EDGES.x, EJECTA_RAY_EDGES.y, sample_noise_3d(float3(theta * EJECTA_RAY_COUNT, 0.0, 1.0)))
                  * (1.0 - smoothstep(EJECTA_INNER_M, EJECTA_OUTER_M, r_m));
        albedo = mix(albedo, EJECTA_COLOR, EJECTA_STRENGTH * ray);
    }
    if (dist_s < GROUND_DETAIL_S && r_m > CRATER_RIM_RADIUS_M + CRATERLET_CLEARANCE_M) {
        float2 local = (cell + 0.5) * cell_s, best = float2(NO_HIT_S, 0.0);
        for (int i = 0; i < 9; i++) {
            float2 cand = (float2(float(i % 3), float(i / 3)) - 1.0) * tile_inradius_s * CRATERLET_SPACING_FRAC;
            float d_m = length(local - cand) * r;
            if (d_m > CRATERLET_MAX_M) continue;
            float2 kk = tanh(cand);
            float3 cg = N * (float3(kk, 1.0) * rsqrt(1.0 - dot(kk, kk)));
            float hc = hash_3d(floor(cg * CRATERLET_HASH_SCALE) * HASH_SHRINK);
            float radius_m = CRATERLET_MAX_M * (CRATERLET_MIN_FRAC + (1.0 - CRATERLET_MIN_FRAC) * fract(hc * CRATERLET_SIZE_SEED));
            if (hc < CRATERLET_CHANCE && d_m < radius_m) best = float2(d_m / radius_m, 1.0);
        }
        if (best.y > 0.5) {
            albedo = best.x > CRATERLET_RIM_FRAC ? albedo * CRATERLET_RIM_BRIGHTEN : albedo * (CRATERLET_FLOOR_DARKEN + CRATERLET_FLOOR_SLOPE * best.x);
        }
    }
    float3 glow = float3(0.0);
    if (u.has_regions != 0) {
        float zone_edge_u_m = u.regions.x, boundary_u_m = u.regions.y, boundary_v_m = u.regions.z;
        if (abs(u_m) > zone_edge_u_m) albedo *= mix(float3(1.0), (u_m < 0.0 ? BLUE_COLOR : RED_COLOR) * ZONE_TINT_GAIN, ZONE_TINT_FRAC);
        if (abs(abs(u_m) - zone_edge_u_m) < ZONE_STRIP_M && abs(v_m) < boundary_v_m) glow = (u_m < 0.0 ? BLUE_COLOR : RED_COLOR) * ZONE_STRIP_GLOW;
        float e = (u_m / boundary_u_m) * (u_m / boundary_u_m) + (v_m / boundary_v_m) * (v_m / boundary_v_m);
        if (e > 1.0) {
            albedo *= OUTSIDE_DARKEN;
        } else if (e > 1.0 - BOUNDARY_STRIP_FRAC) {
            float stripes = floor(u_m / HAZARD_STRIPE_M) + floor(v_m / HAZARD_STRIPE_M);
            albedo = stripes - 2.0 * floor(stripes / 2.0) < 0.5 ? get_palette(scene, u.mat_hazard).rgb : HAZARD_DARK_COLOR;
        }
    }
    float texel = hash_3d(float3(floor(uv * SUBTEXELS), h * CELL_SEED));
    albedo *= 1.0 + detail * (BLOCK_NOISE * (h - 0.5) + TEXEL_NOISE * (texel - 0.5));
    float2 edge = min(uv, 1.0 - uv);
    albedo *= mix(1.0, mix(EDGE_DARKEN, 1.0, smoothstep(0.0, EDGE_WIDTH, min(edge.x, edge.y))), detail * GROUND_EDGE_FRAC);
    return light_surface(scene, albedo, x, float4(0.0, 1.0, 0.0, 0.0), v) + glow;
}

/**
 * @brief `shade_block` shades a block face hit at x: its material with the style's patterns (solar cells, hazard
 * stripes, foil, panel seams, consoles), per-block and per-texel noise, darkened edges, and light with the face's
 * normal; glowing materials are unlit.
 * @par Example
 * @code
 * float3 c = shade_block(scene, hit, x, v, s);
 * @endcode
 */
static float3 shade_block(thread const Scene &scene, Hit h, float4 x, float4 v, float dist_s) {
    constant VoxelUniforms &u = scene.u;
    device const VoxelStructure &st = scene.structures[h.k];
    float4 r0 = st.rows[0], r1 = st.rows[1], r2 = st.rows[2], r3 = st.rows[3];
    float4 p = float4(dot(r0, x), dot(r1, x), dot(r2, x), dot(r3, x));
    int axis = h.axis < 0 ? 1 : h.axis;
    float c = p[axis] / p.w;
    float4 n_local = float4(0.0);
    n_local[axis] = 1.0;
    n_local.w = c;
    n_local *= (h.axis < 0 ? 1.0 : h.sgn) * rsqrt(max(1.0 - c * c, TINY));
    float4 n = r0 * n_local.x + r1 * n_local.y + r2 * n_local.z - r3 * n_local.w;
    n.w = -n.w;
    float2 uv = fract(float2(h.g[(axis + 1) % 3], h.g[(axis + 2) % 3]));
    int m = h.mat;
    float4 entry = get_palette(scene, m);
    float3 albedo = entry.rgb;
    float glow = entry.a, block = hash_3d(float3(h.cell) + float(h.k) * STRUCT_SEED), detail = detail_fraction(scene, dist_s);
    float stripe = step(0.5, fract((uv.x + uv.y) * STRIPES_PER_BLOCK));
    if (m >= u.mat_console_first && m < u.mat_console_first + u.mat_console_count) albedo *= 1.0 - CONSOLE_STRIPE_DARKEN * (1.0 - stripe);
    if (m == u.mat_solar) albedo = mix(albedo, SOLAR_GRID_COLOR, step(SOLAR_GRID_EDGE, max(fract(uv.x * SOLAR_CELLS.x), fract(uv.y * SOLAR_CELLS.y))));
    if (m == u.mat_hazard) albedo = mix(albedo, HAZARD_STRIPE_COLOR, stripe);
    if (m == u.mat_gold_foil) albedo *= 1.0 - FOIL_VARIATION * (0.5 - hash_3d(float3(floor(uv * FOIL_CRINKLES), block)));
    if (m == u.mat_hull || m == u.mat_pad) albedo *= 1.0 - PANEL_SEAM_DARKEN * step(PANEL_SEAM_EDGE, max(fract(uv.x * PANEL_SEAMS), fract(uv.y * PANEL_SEAMS)));
    float texel = hash_3d(float3(floor(uv * SUBTEXELS), block * TEXEL_SEED));
    albedo *= 1.0 + detail * (BLOCK_NOISE * (block - 0.5) + TEXEL_NOISE * (texel - 0.5));
    float2 edge = min(uv, 1.0 - uv);
    albedo *= mix(1.0, mix(EDGE_DARKEN, 1.0, smoothstep(0.0, EDGE_WIDTH, min(edge.x, edge.y))), detail * (1.0 - glow));
    if (glow > 0.5) return albedo * (GLOW_BRIGHTNESS + GLOW_RIM * smoothstep(0.0, GLOW_RIM, min(edge.x, edge.y)));
    return light_surface(scene, albedo, x, n, v);
}

/** @brief `sample_star_layer` draws one layer of stars from a grid on the cube around the sky. @par Example @code float s = sample_star_layer(scene, d, 60.0, 0.03); @endcode */
static float sample_star_layer(thread const Scene &scene, float3 d, float grid, float density) {
    float3 ad = abs(d);
    float2 uv;
    float face;
    if (ad.x >= ad.y && ad.x >= ad.z) { uv = d.yz / ad.x; face = d.x > 0.0 ? 0.0 : 1.0; }
    else if (ad.y >= ad.z) { uv = d.xz / ad.y; face = d.y > 0.0 ? 2.0 : 3.0; }
    else { uv = d.xy / ad.z; face = d.z > 0.0 ? 4.0 : 5.0; }
    float2 cell = floor(uv * grid);
    float h = hash_3d(float3(cell, face + grid));
    if (h > density) return 0.0;
    float2 star = (cell + 0.5 * (1.0 - STAR_JITTER) + STAR_JITTER * float2(hash_3d(float3(cell, face + STAR_SEEDS.x)), hash_3d(float3(cell, face + STAR_SEEDS.y)))) / grid;
    float angle = length(uv - star) / (1.0 + dot(uv, uv)), radius = scene.u.pixel_angle_rad * STAR_PIXELS;
    return (STAR_FAINT + STAR_BRIGHT * pow(h / density, STAR_BRIGHTNESS_POWER)) * exp(-angle * angle / (radius * radius));
}

/** @brief `get_sky_direction` gives an escaping ray's direction, carried to the eye, then to the origin. @par Example @code float3 d = get_sky_direction(scene, x, v); @endcode */
static float3 get_sky_direction(thread const Scene &scene, float4 x, float4 v) {
    float4 cam = scene.u.camera[3];
    float4 w = v + mdot(cam, v) / (1.0 - mdot(x, cam)) * (x + cam);
    return normalize(w.xyz - w.w / (1.0 + cam.w) * cam.xyz);
}

/** @brief `sample_sky` gives the sky in direction d at the origin: stars, Milky Way, sun, planet. @par Example @code float3 c = sample_sky(scene, d); @endcode */
static float3 sample_sky(thread const Scene &scene, float3 d) {
    constant VoxelUniforms &u = scene.u;
    float3 sun_dir = u.sun.xyz, planet = u.planet.xyz, e1 = u.planet_e1.xyz, e2 = u.planet_e2.xyz;
    float sd = dot(d, sun_dir);
    float3 sun = SUN_COLOR * (smoothstep(SUN_COS_OUTER, SUN_COS_INNER, sd) * SUN_DISC_BRIGHTNESS + pow(max(sd, 0.0), SUN_GLARE_POWER) * SUN_GLARE
                              + pow(max(sd, 0.0), SUN_HALO_POWER) * SUN_HALO);
    float2 q = float2(dot(d, e1), dot(d, e2)) / u.planet.w;
    float r2 = dot(q, q);
    if (dot(d, planet) > 0.0 && r2 < 1.0) {
        float3 nrm = q.x * e1 + q.y * e2 - sqrt(1.0 - r2) * planet;
        float lit = dot(nrm, sun_dir);
        float land = smoothstep(LAND_EDGES.x, LAND_EDGES.y, sample_fbm_3d(nrm * LAND_SCALE + LAND_OFFSET));
        float cloud = smoothstep(CLOUD_EDGES.x, CLOUD_EDGES.y, sample_fbm_3d(nrm * CLOUD_SCALE + CLOUD_OFFSET));
        float3 surface = mix(mix(OCEAN_COLOR, LAND_COLOR, land), CLOUD_COLOR, cloud);
        float3 color = surface * (max(lit, 0.0) * PLANET_SUNLIT + PLANET_NIGHT) + ATMOSPHERE_COLOR * pow(r2, LIMB_POWER) * max(lit + LIMB_WRAP, 0.0) * LIMB_GLOW;
        return color + sun;
    }
    float3 color = float3(sample_star_layer(scene, d, STAR_GRID_COARSE, STAR_DENSITY_COARSE) + FINE_STAR_BRIGHTNESS * sample_star_layer(scene, d, STAR_GRID_FINE, STAR_DENSITY_FINE));
    float band = exp(-pow(dot(d, normalize(MILKY_WAY_NORMAL)) / MILKY_WAY_WIDTH, 2.0));
    color += MILKY_WAY_COLOR * band * MILKY_WAY_BRIGHTNESS * sample_fbm_3d(d * MILKY_WAY_DETAIL);
    if (dot(d, planet) > 0.0 && r2 < HALO_RADIUS_SQ) {
        color += ATMOSPHERE_COLOR * HALO_GLOW * (1.0 - smoothstep(1.0, HALO_RADIUS_SQ, r2)) * max(dot(normalize(q.x * e1 + q.y * e2), sun_dir) + HALO_WRAP, 0.0);
    }
    return color + sun;
}

/** @brief `scan_beams` adds the glow of laser segments near a chord (closest approach in closed form), with the brightest halo's team. @par Example @code scan_beams(scene, x, v, s, t, core, halo, team); @endcode */
static void scan_beams(thread const Scene &scene, float4 x, float4 v, float s_end, float travelled, thread float &core, thread float &halo, thread float &team) {
    float r = scene.u.curvature_radius_m, pix = scene.u.pixel_angle_rad;
    for (int i = 0; i < scene.u.beam_count; i++) {
        float4 a = scene.beams[i].a, b = scene.beams[i].b, info = scene.beams[i].info;
        float half_len = 0.5 * info.x;
        float4 mid = geodesic_point(a, b, half_len);
        float am = -mdot(x, mid), bm = mdot(v, mid), reach = half_len + BEAM_REACH * max(BEAM_RADIUS_M, pix * r * sinh(min(travelled + s_end, FAR_S))) / r;
        if (am * am - bm * bm > cosh(reach) * cosh(reach)) continue;
        float al = mdot(x, a), be = mdot(v, a), ga = mdot(x, b), et = mdot(v, b);
        float k1 = 0.5 * ((al * al - ga * ga) + (be * be - et * et)), k2 = al * be - ga * et;
        float s_c = k1 > abs(k2) ? 0.5 * clamp_atanh(-k2 / k1) : (k2 < 0.0 ? s_end : 0.0);
        s_c = clamp(s_c, 0.0, min(s_end, FAR_S));
        float4 xc = geodesic_point(x, v, s_c);
        float sigma = clamp(clamp_atanh(mdot(xc, b) / -mdot(xc, a)), 0.0, info.x);
        float d_m = measure_gap(xc, geodesic_point(a, b, sigma)) * r;
        float width_m = max(BEAM_RADIUS_M, pix * r * sinh(min(travelled + s_c, FAR_S)));
        float q = d_m * d_m / (width_m * width_m);
        core = max(core, info.z * exp(-q));
        float hl = info.z * exp(-q / BEAM_REACH);
        if (hl > halo) { halo = hl; team = info.w; }
    }
}

/**
 * @brief `back_to_reach` moves a ray that has just left warp w's reach (by up to one step) back along its straight line
 * to the reach's sphere, so that an escaping ray's direction is carried to the eye from where its bending ended rather
 * than from wherever its last step happened to end, which jumps between neighbouring rays.
 * @par Example
 * @code
 * back_to_reach(scene, last_warp, x, v);
 * @endcode
 */
static void back_to_reach(thread const Scene &scene, int w, thread float4 &x, thread float4 &v) {
    float4 c = scene.warps[w].centre;
    float a = -mdot(x, c), b = -mdot(v, c), closest_sq = a * a - b * b, reach_cosh = cosh(scene.warps[w].params.z);
    if (b <= 0.0 || closest_sq >= reach_cosh * reach_cosh) return;
    float s_back = max(clamp_atanh(b / a) - acosh(reach_cosh * rsqrt(max(closest_sq, 1.0))), 0.0);
    float4 x_back = geodesic_point(x, -v, s_back);
    v = -geodesic_tangent(x, -v, s_back);
    x = x_back;
}

/**
 * @brief `trace_ray` traces a ray backwards from the eye through a point of the image (in pixels from its top left
 * corner): straight chords between warp objects, bent steps inside their reach, through wormhole throats, until a
 * surface, the sky or a horizon. After `LATE_STEP_COUNT` iterations (rays winding near a throat or the black hole,
 * which land in the lens's most compressed, aliased images) its bent steps are three times longer.
 * @par Example
 * @code
 * VoxelRayEnd end = trace_ray(scene, float2(id) + 0.5);
 * @endcode
 */
static VoxelRayEnd trace_ray(thread const Scene &scene, float2 pixel) {
    constant VoxelUniforms &u = scene.u;
    device const VoxelWarp *warps = scene.warps;
    float r = u.curvature_radius_m;
    float2 ndc = float2(2.0 * pixel.x / float(u.width_px) - 1.0, 1.0 - 2.0 * pixel.y / float(u.height_px));
    float3 d_cam = normalize(float3(ndc.x * u.tan_half_fov * u.aspect, ndc.y * u.tan_half_fov, -1.0));
    float4 x = u.camera[3], v = u.camera[0] * d_cam.x + u.camera[1] * d_cam.y + u.camera[2] * d_cam.z;
    int inside = -1;
    for (int w = 0; w < u.warp_count; w++) {
        if (-mdot(x, warps[w].centre) < cosh(warps[w].params.z)) inside = w;
    }
    float travelled = 0.0, glass = 0.0, core = 0.0, halo = 0.0, team = 0.0;
    Hit hit;
    hit.s = NO_HIT_S; hit.mat = 0; hit.k = -1; hit.axis = -1; hit.sgn = 0.0; hit.g = float3(0.0); hit.cell = int3(0);
    int outcome = OUTCOME_LOST, last_warp = -1;
    float4 hit_x = x, hit_v = v;
    float hit_travelled = 0.0;
    float min_step = MIN_BEND_STEP_M / r, max_step = MAX_BEND_STEP_M / r, rho = 0.0;
    float4 gradient = float4(0.0);
    float3 local = float3(0.0);
    bool has_gradient = false;
    for (int it = 0; it < MAX_ITERATIONS; it++) {
        if (inside < 0) {
            float s_end = FAR_S;
            int entering = -1;
            for (int w = 0; w < u.warp_count; w++) {
                float a = -mdot(x, warps[w].centre), b = mdot(v, warps[w].centre), closest_sq = a * a - b * b, reach_cosh = cosh(warps[w].params.z);
                if (b <= 0.0 || closest_sq >= reach_cosh * reach_cosh) continue;
                float s_in = max(clamp_atanh(b / a) - acosh(reach_cosh * rsqrt(max(closest_sq, 1.0))), 0.0);
                if (s_in < s_end) { s_end = s_in; entering = w; }
            }
            hit.s = s_end;
            hit.mat = 0;
            float glass_s = NO_HIT_S;
            trace_scene(scene, x, v, -1, 0.0, float3(0.0), hit, glass_s);
            if (glass_s < hit.s) glass += 1.0;
            if (u.beam_count > 0) scan_beams(scene, x, v, hit.s, travelled, core, halo, team);
            if (hit.mat != 0) { outcome = OUTCOME_HIT; hit_x = x; hit_v = v; hit_travelled = travelled; break; }
            if (entering < 0) {
                if (last_warp >= 0) back_to_reach(scene, last_warp, x, v);
                outcome = OUTCOME_SKY;
                break;
            }
            float4 xn = geodesic_point(x, v, s_end);
            v = geodesic_tangent(x, v, s_end);
            x = xn;
            travelled += s_end;
            inside = entering;
            continue;
        }
        if (!has_gradient) gradient = compute_gradient(scene, inside, x, rho, local);
        float ds = clamp((it < LATE_STEP_COUNT ? BEND_STEP_FRAC : LATE_BEND_STEP_FRAC) * rho, min_step, max_step);
        v = normalize_tangent(x, v + (gradient - mdot(gradient, v) * v) * (0.5 * ds));
        hit.s = ds;
        hit.mat = 0;
        float glass_s = NO_HIT_S;
        trace_scene(scene, x, v, inside, rho, local, hit, glass_s);
        if (glass_s < hit.s) glass += 1.0;
        if (u.beam_count > 0) scan_beams(scene, x, v, hit.s, travelled, core, halo, team);
        if (hit.mat != 0) { outcome = OUTCOME_HIT; hit_x = x; hit_v = v; hit_travelled = travelled; break; }
        float4 xn = geodesic_point(x, v, ds), vn = geodesic_tangent(x, v, ds);
        x = xn * rsqrt(-mdot(xn, xn));
        v = normalize_tangent(x, vn);
        travelled += ds;
        gradient = compute_gradient(scene, inside, x, rho, local);
        v = normalize_tangent(x, v + (gradient - mdot(gradient, v) * v) * (0.5 * ds));
        has_gradient = true;
        float4 prm = warps[inside].params;
        if (prm.x < HALF_KIND && rho < prm.y * HORIZON_CAPTURE_FRAC) { outcome = OUTCOME_LOST; break; }
        if (prm.x > HALF_KIND && rho < prm.y) { cross_throat(scene, inside, x, v); inside = int(prm.w + 0.5); has_gradient = false; continue; }
        if (rho > prm.z) { last_warp = inside; inside = -1; has_gradient = false; }
    }
    VoxelRayEnd end = {float4(0.0), float4(0.0), float4(0.0, 0.0, 0.0, travelled), int4(0), int4(outcome, -1, -1, team > HALF_KIND ? 1 : 0),
                       float4(0.0, glass, core, halo)};
    if (outcome == OUTCOME_HIT) {
        end.point = geodesic_point(hit_x, hit_v, hit.s);
        end.view = geodesic_tangent(hit_x, hit_v, hit.s);
        end.grid.w = hit_travelled + hit.s;
        end.cell.w = hit.mat;
        if (hit.mat != MAT_GROUND) {
            end.grid.xyz = hit.g;
            end.cell.xyz = hit.cell;
            end.surface.yz = int2(hit.k, hit.axis);
            end.glow.x = hit.sgn;
        }
    } else if (outcome == OUTCOME_SKY) {
        end.point = float4(get_sky_direction(scene, x, v), 0.0);
    }
    return end;
}

/**
 * @brief `shade_ray_end` gives a ray's final colour from where it ended: the lit surface or the sky, tinted by the
 * glass it crossed, with the laser's glow and the highlight roll-off; a lost ray is black.
 * @par Example
 * @code
 * float3 color = shade_ray_end(scene, trace_ray(scene, float2(id) + 0.5));
 * @endcode
 */
static float3 shade_ray_end(thread const Scene &scene, VoxelRayEnd end) {
    float3 color = float3(0.0);
    if (end.surface.x == OUTCOME_HIT && end.cell.w == MAT_GROUND) {
        color = shade_ground(scene, end.point, end.view, end.grid.w);
    } else if (end.surface.x == OUTCOME_HIT) {
        Hit hit = {0.0, end.cell.w, end.surface.y, end.surface.z, end.glow.x, end.grid.xyz, end.cell.xyz};
        color = shade_block(scene, hit, end.point, end.view, end.grid.w);
    } else if (end.surface.x == OUTCOME_SKY) {
        color = sample_sky(scene, end.point.xyz);
    }
    float glass = end.glow.y, core = end.glow.z, halo = end.glow.w;
    float3 beam_color = end.surface.w > 0 ? RED_COLOR : BLUE_COLOR;
    if (glass > 0.0) color = color * pow(GLASS_TINT, glass) + GLASS_SHEEN;
    color = mix(color, beam_color, clamp(BEAM_HALO_OPACITY * halo, 0.0, 1.0)) + mix(beam_color, float3(1.0), BEAM_CORE_WHITENESS) * core;
    color = color / (1.0 + max(color - 1.0, 0.0) * TONEMAP_KNEE);
    return clamp(color, 0.0, 1.0);
}

/** @brief `write_pixel` shades a ray end into pixel `id`. @par Example @code write_pixel(scene, end, id, image); @endcode */
static void write_pixel(thread const Scene &scene, VoxelRayEnd end, uint2 id, texture2d<float, access::write> image) {
    image.write(float4(shade_ray_end(scene, end), 1.0), id);
}

/**
 * @brief `trace_voxel_pixels` traces every pixel's ray and writes its colour (`--sampling every-pixel`).
 * @par Example
 * @code
 * dispatched by render_voxel.rs, one thread per pixel.
 * @endcode
 */
kernel void trace_voxel_pixels(SCENE_BUFFERS, texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = {u, structures, voxels, warps, warp_lists, lights, palette, beams, clearances};
    write_pixel(scene, trace_ray(scene, float2(id) + 0.5), id, image);
}

/**
 * @brief `is_same_surface` says whether two samples ended the same way with no laser glow, so the pixels between them
 * can be interpolated: both on one face plane of one structure (same axis, side and layer of cells), both on the
 * ground, both in the sky or both lost, behind the same number of glass faces.
 * @par Example
 * @code
 * if (!is_same_surface(a, b)) return false;
 * @endcode
 */
static bool is_same_surface(VoxelRayEnd a, VoxelRayEnd b) {
    int axis = a.surface.z;
    bool is_inside_cell = a.surface.x == OUTCOME_HIT && a.surface.y >= 0 && axis < 0;
    return all(a.surface.xyz == b.surface.xyz) && a.glow.x == b.glow.x && a.glow.y == b.glow.y && !is_inside_cell
        && (axis < 0 || a.cell[axis] == b.cell[axis]) && b.glow.z < GLOW_FLOOR && b.glow.w < GLOW_FLOOR;
}

/**
 * @brief `is_line_smooth` says whether four samples in a line map screen to scene nearly linearly between the middle
 * two: the second differences, which are 8 times the midpoint's interpolation error, stay under `ratio` of the first
 * difference. For samples `h` pixels apart that bounds the error by `ratio` × h / 8 pixels.
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
 * @brief `is_cell_smooth` says whether the cell between samples (i, j) and (i + 1, j + 1) of a grid `columns` ×
 * `grid_rows` can be interpolated: the twelve samples around it (a 4×4 block without its corners) ended on the
 * corner's surface and map nearly linearly along the cell's two rows and two columns. Border cells never are.
 * @par Example
 * @code
 * bool is_smooth = is_cell_smooth(samples, columns, sample_rows, i, j, SMOOTH_SECOND_DIFFERENCE_PER_FIRST);
 * @endcode
 */
static bool is_cell_smooth(device const VoxelRayEnd *grid, int columns, int grid_rows, int i, int j, float ratio) {
    if (i < 1 || j < 1 || i + 2 >= columns || j + 2 >= grid_rows) return false;
    VoxelRayEnd corner = grid[j * columns + i];
    float4 p[4][4];
    for (int b = 0; b < 4; b++) {
        for (int a = 0; a < 4; a++) {
            if ((a == 0 || a == 3) && (b == 0 || b == 3)) continue;
            VoxelRayEnd sample = grid[(j - 1 + b) * columns + (i - 1 + a)];
            if (!is_same_surface(corner, sample)) return false;
            p[b][a] = sample.point;
        }
    }
    return is_line_smooth(p[1][0], p[1][1], p[1][2], p[1][3], ratio) && is_line_smooth(p[2][0], p[2][1], p[2][2], p[2][3], ratio)
        && is_line_smooth(p[0][1], p[1][1], p[2][1], p[3][1], ratio) && is_line_smooth(p[0][2], p[1][2], p[2][2], p[3][2], ratio);
}

/**
 * @brief `interpolate_ray_end` blends four samples around a point of a cell (`wx` and `wy` from 0 to 1 along it) into
 * `end`: the point (back on the hyperboloid, or the unit sphere for the sky), tangent, grid point and distance blend,
 * and the rest is the first sample's. On a block face the cell under the blended grid point gives the material, and
 * the result is false (trace the ray instead) unless that cell is solid and the one in front of the face is open.
 * @par Example
 * @code
 * if (!interpolate_ray_end(scene, a, b, c, d, 0.5, 0.0, end)) end = trace_ray(scene, pixel);
 * @endcode
 */
static bool interpolate_ray_end(thread const Scene &scene, VoxelRayEnd a, VoxelRayEnd b, VoxelRayEnd c, VoxelRayEnd d, float wx, float wy,
                                thread VoxelRayEnd &end) {
    end = a;
    float4 point = mix(mix(a.point, b.point, wx), mix(c.point, d.point, wx), wy);
    end.grid = mix(mix(a.grid, b.grid, wx), mix(c.grid, d.grid, wx), wy);
    if (a.surface.x == OUTCOME_SKY) end.point = float4(normalize(point.xyz), 0.0);
    if (a.surface.x != OUTCOME_HIT) return true;
    end.point = point * rsqrt(-mdot(point, point));
    end.view = normalize_tangent(end.point, mix(mix(a.view, b.view, wx), mix(c.view, d.view, wx), wy));
    if (a.surface.y < 0) return true;
    device const VoxelStructure &st = scene.structures[a.surface.y];
    int axis = a.surface.z;
    int3 dims = st.size.xyz, cell = clamp(int3(floor(end.grid.xyz)), int3(0), dims - 1), front = cell;
    cell[axis] = a.cell[axis];
    front[axis] = cell[axis] + int(a.glow.x);
    device const uchar *cells = scene.voxels + st.size.w;
    int m = int(cells[cell.x + dims.x * (cell.y + dims.y * cell.z)]);
    bool is_front_open = any(front < int3(0)) || any(front >= dims) || cells[front.x + dims.x * (front.y + dims.y * front.z)] == 0;
    end.cell = int4(cell, m);
    return m != 0 && m != scene.u.mat_glass && is_front_open;
}

/** @brief `get_sample_columns` gives how many samples a row of the fine grid has (one per even pixel column, one past the edge). @par Example @code int columns = get_sample_columns(u); @endcode */
static int get_sample_columns(constant VoxelUniforms &u) { return u.width_px / 2 + 2; }

/** @brief `get_coarse_columns` gives how many samples a row of the coarse grid has (one per fourth pixel column, enough past the edge to cover the fine grid). @par Example @code int columns = get_coarse_columns(u); @endcode */
static int get_coarse_columns(constant VoxelUniforms &u) { return u.width_px / 4 + 3; }

/**
 * @brief `trace_coarse` traces the rays of every fourth pixel each way (coarse sample (i, j) is pixel (4i, 4j)), the
 * first pass of adaptive sampling, and starts this frame's lists empty.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per coarse sample.
 * @endcode
 */
kernel void trace_coarse(SCENE_BUFFERS, device VoxelRayEnd *coarse [[buffer(9)]], device atomic_uint *refine_count [[buffer(5)]],
                         device atomic_uint *fill_count [[buffer(12)]], uint2 id [[thread_position_in_grid]]) {
    int columns = get_coarse_columns(u), coarse_rows = u.height_px / 4 + 3;
    if (id.x == 0 && id.y == 0) {
        atomic_store_explicit(refine_count, 0, memory_order_relaxed);
        atomic_store_explicit(fill_count, 0, memory_order_relaxed);
    }
    if (int(id.x) >= columns || int(id.y) >= coarse_rows) return;
    Scene scene = {u, structures, voxels, warps, warp_lists, lights, palette, beams, clearances};
    coarse[int(id.y) * columns + int(id.x)] = trace_ray(scene, 4.0 * float2(id) + 0.5);
}

/**
 * @brief `fill_samples` fills the fine grid (sample (i, j) is pixel (2i, 2j)), the second pass of adaptive sampling:
 * a sample on the coarse grid copies it, one inside a smooth coarse cell (with half the fine grid's threshold, for
 * the same error in pixels) interpolates the cell's corners, and any other is listed for `trace_listed_samples`.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per fine sample.
 * @endcode
 */
kernel void fill_samples(SCENE_BUFFERS, device VoxelRayEnd *samples [[buffer(2)]], device const VoxelRayEnd *coarse [[buffer(9)]],
                         device uint *fill_list [[buffer(11)]], device atomic_uint *fill_count [[buffer(12)]], uint2 id [[thread_position_in_grid]]) {
    int columns = get_sample_columns(u), sample_rows = u.height_px / 2 + 2;
    int coarse_columns = get_coarse_columns(u), coarse_rows = u.height_px / 4 + 3;
    int i = int(id.x), j = int(id.y);
    if (i >= columns || j >= sample_rows) return;
    int ci = i / 2, cj = j / 2, di = i & 1, dj = j & 1, k = cj * coarse_columns + ci;
    if (di + dj == 0) {
        samples[j * columns + i] = coarse[k];
        return;
    }
    Scene scene = {u, structures, voxels, warps, warp_lists, lights, palette, beams, clearances};
    VoxelRayEnd end;
    if (is_cell_smooth(coarse, coarse_columns, coarse_rows, ci, cj, 0.5 * SMOOTH_SECOND_DIFFERENCE_PER_FIRST)
        && interpolate_ray_end(scene, coarse[k], coarse[k + di], coarse[k + dj * coarse_columns], coarse[k + dj * coarse_columns + di],
                               0.5 * float(di), 0.5 * float(dj), end)) {
        samples[j * columns + i] = end;
    } else {
        fill_list[atomic_fetch_add_explicit(fill_count, 1, memory_order_relaxed)] = uint(j * columns + i);
    }
}

/**
 * @brief `prepare_fill` sets the thread-group count of `trace_listed_samples`'s indirect dispatch from how many
 * samples `fill_samples` listed.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling as a single thread.
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
 * dispatched by render_metal.rs's encode_adaptive_sampling indirectly, one thread per listed sample.
 * @endcode
 */
kernel void trace_listed_samples(SCENE_BUFFERS, device VoxelRayEnd *samples [[buffer(2)]], device const uint *fill_list [[buffer(11)]],
                                 device atomic_uint *fill_count [[buffer(12)]], uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(fill_count, memory_order_relaxed)) return;
    int columns = get_sample_columns(u), k = int(fill_list[t]);
    Scene scene = {u, structures, voxels, warps, warp_lists, lights, palette, beams, clearances};
    samples[k] = trace_ray(scene, 2.0 * float2(k % columns, k / columns) + 0.5);
}

/**
 * @brief `classify_cells` marks each cell between four fine samples smooth (1) when the twelve samples around it
 * ended on one surface and map nearly linearly, and lists every other cell that covers pixels, for `refine_pixels`.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per cell (sample (i, j) to (i + 1, j + 1)).
 * @endcode
 */
kernel void classify_cells(constant VoxelUniforms &u [[buffer(16)]], device const VoxelRayEnd *samples [[buffer(2)]],
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
 * @brief `prepare_refinement` sets the thread-group count of `refine_pixels`'s indirect dispatch from how many cells
 * `classify_cells` listed.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling as a single thread.
 * @endcode
 */
kernel void prepare_refinement(device atomic_uint *refine_count [[buffer(5)]], device uint *dispatch_groups [[buffer(6)]]) {
    uint pixel_count = atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL;
    dispatch_groups[0] = (pixel_count + REFINE_GROUP_SIZE - 1) / REFINE_GROUP_SIZE;
    dispatch_groups[1] = 1;
    dispatch_groups[2] = 1;
}

/**
 * @brief `refine_pixels` traces the three odd pixels of each listed cell, packed densely so that thread groups don't
 * wait on the few pixels that need a full trace.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling indirectly, one thread per listed pixel.
 * @endcode
 */
kernel void refine_pixels(SCENE_BUFFERS, device const uint *refine_cells [[buffer(4)]], device atomic_uint *refine_count [[buffer(5)]],
                          texture2d<float, access::write> image [[texture(0)]], uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL) return;
    int columns = get_sample_columns(u);
    int cell = int(refine_cells[t / REFINED_PIXELS_PER_CELL]), corner = int(t % REFINED_PIXELS_PER_CELL) + 1;
    uint2 id = uint2(2 * (cell % columns) + (corner & 1), 2 * (cell / columns) + (corner >> 1));
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = {u, structures, voxels, warps, warp_lists, lights, palette, beams, clearances};
    write_pixel(scene, trace_ray(scene, float2(id) + 0.5), id, image);
}

/**
 * @brief `resolve_pixels` writes the pixels that need no listed ray: an even pixel takes its sample, and a pixel in a
 * smooth cell interpolates the cell's corners and shades the result (tracing its own ray if the blended cell isn't a
 * visible face). `refine_pixels` writes the rest.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per pixel.
 * @endcode
 */
kernel void resolve_pixels(SCENE_BUFFERS, device const VoxelRayEnd *samples [[buffer(2)]], device const uchar *smooth_cells [[buffer(3)]],
                           texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    int columns = get_sample_columns(u);
    int i = int(id.x) / 2, j = int(id.y) / 2, di = int(id.x) & 1, dj = int(id.y) & 1, k = j * columns + i;
    if (di + dj > 0 && smooth_cells[k] == 0) return;
    Scene scene = {u, structures, voxels, warps, warp_lists, lights, palette, beams, clearances};
    VoxelRayEnd end = samples[k];
    if (di + dj > 0
        && !interpolate_ray_end(scene, end, samples[k + di], samples[k + dj * columns], samples[k + dj * columns + di], 0.5 * float(di), 0.5 * float(dj), end)) {
        end = trace_ray(scene, float2(id) + 0.5);
    }
    write_pixel(scene, end, id, image);
}
