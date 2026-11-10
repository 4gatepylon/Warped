/**
 * @file
 * @brief `boundary_planes.metal` is how the team boundary planes look in every level (W-148, team boundary planes in
 * every level; W-149, shimmer through boundary planes): their colours and glow, how opaque they are from the ground
 * up (`get_boundary_profile`, Lunar Surface's W-64 (zone planes) look), and the heat haze where they cover the screen
 * (`get_boundary_haze_px`), a screen-space effect each level applies in its last full-screen pass. Each level finds
 * where its planes cover the screen (from its map's team zones) and calls these. `boundary_planes.rs` holds the haze's
 * clock and the tests.
 *
 * @par Interface
 * - Constants: `BOUNDARY_BLUE_COLOR`, `BOUNDARY_RED_COLOR`, `BOUNDARY_BRIGHTNESS`, `BOUNDARY_HAZE_PX`.
 * - Functions: `get_boundary_color`, `get_boundary_profile`, `get_boundary_haze_px`.
 *
 * @par Example
 * @code
 * float2 move_px = get_boundary_haze_px(float2(id), height_px, clock_s, LEVEL_HAZE_STYLE) * weight;
 * @endcode
 *
 * @par Must know
 * - Gotchas:
 *   - A level prepends this file to its own shader source (`BOUNDARY_PLANES_METAL`), so it includes the library itself.
 */
#include <metal_stdlib>
using namespace metal;

constant float3 BOUNDARY_BLUE_COLOR = float3(0.30, 0.62, 1.0);
constant float3 BOUNDARY_RED_COLOR = float3(1.0, 0.16, 0.08);
constant float BOUNDARY_BRIGHTNESS = 0.8;
constant float BOUNDARY_FOOT_M = 0.4;
constant float BOUNDARY_BODY_FRAC = 0.6;
constant float BOUNDARY_TOP_FADE_FRAC = 0.4;
constant float BOUNDARY_SOFT_TOP_FRAC = 0.25;
constant float BOUNDARY_HAZE_PX = 8.0;
constant float BOUNDARY_HAZE_REFERENCE_HEIGHT_PX = 1440.0;
constant int BOUNDARY_HAZE_SPACE_CELLS = 1024;
constant int BOUNDARY_HAZE_TIME_CELLS = 512;
constant float BOUNDARY_HAZE_S_PER_TIME_CELL = 0.5;
constant float BOUNDARY_HAZE_TALL_FRAC = 0.45;
constant float BOUNDARY_HAZE_UP_SHARE = 0.35;
constant float BOUNDARY_HAZE_OCTAVE_GAIN = 0.5;
constant float BOUNDARY_HAZE_SPACE_SCALE = 2.03;
constant int BOUNDARY_HAZE_OCTAVE_COUNT = 3;
constant float3 BOUNDARY_HAZE_SECOND_SEED = float3(37.0, 91.0, 0.0);
constant float BOUNDARY_HAZE_TIME_SCALE = 2.0;
constant uint3 BOUNDARY_HAZE_HASH_PRIMES = uint3(73856093u, 19349663u, 83492791u);
constant uint2 BOUNDARY_HAZE_HASH_MULTIPLIERS = uint2(0x7feb352du, 0x846ca68bu);
constant uint3 BOUNDARY_HAZE_HASH_SHIFTS = uint3(16u, 15u, 16u);
constant uint BOUNDARY_HAZE_HASH_DROPPED_BITS = 8u;
constant float BOUNDARY_HAZE_HASH_SCALE = 1.0 / 16777216.0;

/**
 * @brief `get_boundary_color` gives a boundary plane's colour by its team (0 blue, 1 red), in sRGB.
 * @param[in] team The team whose zone the plane bounds.
 * @return The colour.
 * @par Example
 * @code
 * float3 c = to_linear(get_boundary_color(1));
 * @endcode
 */
static float3 get_boundary_color(int team) {
    return team == 0 ? BOUNDARY_BLUE_COLOR : BOUNDARY_RED_COLOR;
}

/**
 * @brief `get_boundary_profile` gives how opaque a boundary plane is at `above_m` over the ground, as a share of its
 * opacity at the foot: 1 at the ground, falling to about `BOUNDARY_BODY_FRAC` within `BOUNDARY_FOOT_M`, then by
 * `BOUNDARY_TOP_FADE_FRAC` of that up to `height_m`, over whose top `BOUNDARY_SOFT_TOP_FRAC` it fades out.
 * @param[in] above_m The height over the ground, at least 0.
 * @param[in] height_m The plane's height.
 * @return The share, 0 to 1.
 * @par Example
 * @code
 * float opacity = look.y * get_boundary_profile(above_m, look.x);
 * @endcode
 */
static float get_boundary_profile(float above_m, float height_m) {
    float height_frac = above_m / height_m;
    return mix(BOUNDARY_BODY_FRAC * (1.0 - BOUNDARY_TOP_FADE_FRAC * height_frac), 1.0, exp(-above_m / BOUNDARY_FOOT_M))
           * (1.0 - smoothstep(1.0 - BOUNDARY_SOFT_TOP_FRAC, 1.0, height_frac));
}

/**
 * @brief `get_haze_hash` gives a pseudo-random number in [0, 1) for a lattice point, the lattice wrapping every
 * `BOUNDARY_HAZE_SPACE_CELLS` cells across and up and `BOUNDARY_HAZE_TIME_CELLS` in time, so the haze repeats only
 * after `boundary_planes.rs`'s `SHIMMER_CLOCK_PERIOD_S` and its clock wraps without a jump.
 * @param[in] cell The lattice point (across, up, time).
 * @return The number.
 * @par Example
 * @code
 * float n = get_haze_hash(int3(4, 7, 2));
 * @endcode
 */
static float get_haze_hash(int3 cell) {
    int2 space = (cell.xy % BOUNDARY_HAZE_SPACE_CELLS + BOUNDARY_HAZE_SPACE_CELLS) % BOUNDARY_HAZE_SPACE_CELLS;
    int time = (cell.z % BOUNDARY_HAZE_TIME_CELLS + BOUNDARY_HAZE_TIME_CELLS) % BOUNDARY_HAZE_TIME_CELLS;
    uint h = uint(space.x) * BOUNDARY_HAZE_HASH_PRIMES.x ^ uint(space.y) * BOUNDARY_HAZE_HASH_PRIMES.y
             ^ uint(time) * BOUNDARY_HAZE_HASH_PRIMES.z;
    h ^= h >> BOUNDARY_HAZE_HASH_SHIFTS.x;
    h *= BOUNDARY_HAZE_HASH_MULTIPLIERS.x;
    h ^= h >> BOUNDARY_HAZE_HASH_SHIFTS.y;
    h *= BOUNDARY_HAZE_HASH_MULTIPLIERS.y;
    h ^= h >> BOUNDARY_HAZE_HASH_SHIFTS.z;
    return float(h >> BOUNDARY_HAZE_HASH_DROPPED_BITS) * BOUNDARY_HAZE_HASH_SCALE;
}

/**
 * @brief `get_haze_noise` is smooth value noise in [-1, 1] at `p` (across and up in lattice cells, then time in time
 * cells): the eight lattice corners' numbers blended with a quintic, so it has no seams and changes smoothly in time.
 * @param[in] p The point.
 * @return The noise.
 * @par Example
 * @code
 * float n = get_haze_noise(float3(4.5, 2.25, 10.0));
 * @endcode
 */
static float get_haze_noise(float3 p) {
    int3 i = int3(floor(p));
    float3 f = p - floor(p);
    float3 w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    float x00 = mix(get_haze_hash(i), get_haze_hash(i + int3(1, 0, 0)), w.x);
    float x10 = mix(get_haze_hash(i + int3(0, 1, 0)), get_haze_hash(i + int3(1, 1, 0)), w.x);
    float x01 = mix(get_haze_hash(i + int3(0, 0, 1)), get_haze_hash(i + int3(1, 0, 1)), w.x);
    float x11 = mix(get_haze_hash(i + int3(0, 1, 1)), get_haze_hash(i + int3(1, 1, 1)), w.x);
    return 2.0 * mix(mix(x00, x10, w.y), mix(x01, x11, w.y), w.z) - 1.0;
}

/**
 * @brief `get_boundary_haze_px` gives how far, in pixels, to move the picture's sample where a team boundary plane
 * covers the screen (W-149, shimmer through boundary planes), like heat haze over railway tracks (the user's reference):
 * wavy, mostly sideways ripples in tall streaks that rise and keep changing, from `BOUNDARY_HAZE_OCTAVE_COUNT` octaves
 * of value noise through time, each finer one rising and changing faster. It is screen-space, needn't match the world,
 * and changes a little every frame; at a 1440-pixel-high picture it moves up to about `BOUNDARY_HAZE_PX` times the
 * level's strength. Its cells are many pixels wide, so it never flickers from pixel to pixel.
 * @param[in] screen_px The pixel, in pixels from the top left.
 * @param[in] height_px The picture's height in pixels.
 * @param[in] clock_s The haze's clock (`boundary_planes.rs`'s `get_shimmer_clock_s`), in seconds.
 * @param[in] style The level's haze: (strength, cell height as a share of the picture's height, how many cells a second
 * it rises, a whole number; how fast it changes, a whole number of time cells per `BOUNDARY_HAZE_S_PER_TIME_CELL`).
 * @return The move, in pixels (x right, y down).
 * @par Example
 * @code
 * float2 move_px = get_boundary_haze_px(float2(id), height_px, clock_s, float4(1.0, 0.03, 3.0, 1.0)) * weight;
 * @endcode
 */
static float2 get_boundary_haze_px(float2 screen_px, float height_px, float clock_s, float4 style) {
    float2 p = screen_px / (style.y * height_px) * float2(1.0, BOUNDARY_HAZE_TALL_FRAC);
    float2 move = float2(0.0);
    float amplitude = 1.0, total = 0.0, space_scale = 1.0, time_scale = 1.0;
    for (int k = 0; k < BOUNDARY_HAZE_OCTAVE_COUNT; k++) {
        float3 q = float3(p.x * space_scale, p.y * space_scale + style.z * time_scale * clock_s,
                          clock_s / BOUNDARY_HAZE_S_PER_TIME_CELL * style.w * time_scale);
        move += amplitude * float2(get_haze_noise(q), BOUNDARY_HAZE_UP_SHARE * get_haze_noise(q + BOUNDARY_HAZE_SECOND_SEED));
        total += amplitude;
        amplitude *= BOUNDARY_HAZE_OCTAVE_GAIN;
        space_scale *= BOUNDARY_HAZE_SPACE_SCALE;
        time_scale *= BOUNDARY_HAZE_TIME_SCALE;
    }
    return move / total * BOUNDARY_HAZE_PX * style.x * height_px / BOUNDARY_HAZE_REFERENCE_HEIGHT_PX;
}
