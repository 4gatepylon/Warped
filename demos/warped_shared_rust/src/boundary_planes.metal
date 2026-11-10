/**
 * @file
 * @brief `boundary_planes.metal` is how the team boundary planes look in every level (W-148, team boundary planes in
 * every level; W-149, shimmer through boundary planes): their colours and glow, how opaque they are from the ground
 * up (`get_boundary_profile`, Lunar Surface's W-64 (zone planes) look), and the shimmer of what is seen through them
 * (`get_boundary_shimmer`). Each level finds where its planes are (its map's team zones) and calls these.
 * `boundary_planes.rs` holds the same shimmer on the CPU, its clock, and the tests.
 *
 * @par Interface
 * - Constants: `BOUNDARY_BLUE_COLOR`, `BOUNDARY_RED_COLOR`, `BOUNDARY_BRIGHTNESS`, `BOUNDARY_SHIMMER_RAD`.
 * - Functions: `get_boundary_color`, `get_boundary_profile`, `get_boundary_shimmer`.
 *
 * @par Example
 * @code
 * float2 bend_rad = get_boundary_shimmer(on_plane_m, clock_s, footprint_m) * weight;
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
constant float BOUNDARY_SHIMMER_RAD = 0.0025;
constant float BOUNDARY_SHIMMER_SHORTEST_WAVE_M = 0.6;
constant float4 BOUNDARY_SHIMMER_WAVES_PER_M = float4(1.1, 0.83, 0.71, 1.27);
constant float4 BOUNDARY_SHIMMER_HZ = float4(0.9, 0.7, 1.1, 0.8);
constant float BOUNDARY_TWO_PI = 6.2831853;

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
 * @brief `get_boundary_shimmer` gives how far what is seen through a boundary plane is turned, in radians across and up
 * the plane, where the view crosses it at `on_plane_m` (two coordinates along the plane, in meters): a few slow waves
 * drifting over the plane, at most `BOUNDARY_SHIMMER_RAD`. Where one pixel covers more than about half the shortest
 * wave (`footprint_m`) it fades out, so it never flickers from pixel to pixel (W-138, no aliasing).
 * @param[in] on_plane_m Where the view crosses the plane, along it, in meters.
 * @param[in] clock_s The shimmer's clock (`boundary_planes.rs`'s `get_shimmer_clock_s`), in seconds.
 * @param[in] footprint_m How wide one pixel is on the plane there, in meters.
 * @return The turn, in radians, each within ±`BOUNDARY_SHIMMER_RAD`.
 * @par Example
 * @code
 * float2 bend_rad = get_boundary_shimmer(float2(v_m, above_m), clock_s, pixel_m);
 * @endcode
 */
static float2 get_boundary_shimmer(float2 on_plane_m, float clock_s, float footprint_m) {
    float4 phase = BOUNDARY_TWO_PI * (BOUNDARY_SHIMMER_WAVES_PER_M * float4(on_plane_m.x + on_plane_m.y, on_plane_m.x - on_plane_m.y,
                                                                             on_plane_m.y, on_plane_m.x)
                                      + BOUNDARY_SHIMMER_HZ * clock_s);
    float2 wave = 0.5 * float2(sin(phase.x) + sin(phase.y), sin(phase.z) + cos(phase.w));
    float fade = 1.0 - smoothstep(0.25 * BOUNDARY_SHIMMER_SHORTEST_WAVE_M, 0.5 * BOUNDARY_SHIMMER_SHORTEST_WAVE_M, footprint_m);
    return BOUNDARY_SHIMMER_RAD * fade * wave;
}
