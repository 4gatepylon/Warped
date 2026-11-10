/**
 * @file
 * @brief `visuals.metal` applies a player's basic visuals (W-51, basic visuals) to the finished, sRGB-encoded picture
 * in each level's last full-screen pass: brightness, contrast, saturation and gamma, in screen space only.
 * `visuals.rs` holds the settings that fill `PlayerVisuals` and the same maths on the CPU
 * (`apply_player_visuals`), which its tests compare with this file's.
 *
 * @par Interface
 * - Struct: `PlayerVisuals`.
 * - Function: `apply_player_visuals`.
 *
 * @par Example
 * @code
 * encoded = apply_player_visuals(encoded, visuals);
 * @endcode
 *
 * @par Must know
 * - Gotchas:
 *   - A level prepends this file to its own shader source (`PLAYER_VISUALS_METAL`), so it includes the library itself.
 */
#include <metal_stdlib>
using namespace metal;

constant float3 PLAYER_VISUALS_LUMA = float3(0.2126, 0.7152, 0.0722);
constant float PLAYER_VISUALS_MID_GREY = 0.5;

/**
 * @brief `PlayerVisuals` matches `GpuPlayerVisuals` in `visuals.rs`: the picture's brightness, contrast, saturation
 * and gamma, each 1 for none.
 */
struct PlayerVisuals {
    float brightness;
    float contrast;
    float saturation;
    float gamma;
};

/**
 * @brief `apply_player_visuals` gives an sRGB-encoded colour as `visuals` show it: scaled by the brightness, spread
 * from mid grey by the contrast, moved away from its grey by the saturation, clamped to 0 to 1, and raised to the
 * power 1 / gamma (a gamma above 1 lifts the dark and middle tones). With every setting 1 it returns `encoded`
 * untouched, so the default picture is unchanged to the bit.
 * @param[in] encoded The colour, sRGB-encoded, each channel 0 to 1.
 * @param[in] visuals The player's visuals.
 * @return The colour to show, each channel 0 to 1.
 * @par Example
 * @code
 * float3 shown = apply_player_visuals(linear_to_srgb(saturate(color)), visuals);
 * @endcode
 */
static float3 apply_player_visuals(float3 encoded, constant PlayerVisuals& visuals) {
    if (visuals.brightness == 1.0 && visuals.contrast == 1.0 && visuals.saturation == 1.0 && visuals.gamma == 1.0) {
        return encoded;
    }
    float3 c = encoded * visuals.brightness;
    c = (c - PLAYER_VISUALS_MID_GREY) * visuals.contrast + PLAYER_VISUALS_MID_GREY;
    float luma = dot(c, PLAYER_VISUALS_LUMA);
    c = luma + (c - luma) * visuals.saturation;
    return pow(saturate(c), 1.0 / visuals.gamma);
}
