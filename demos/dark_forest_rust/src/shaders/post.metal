/**
 * @file
 * @brief `post.metal` holds the screen-space passes: SSAO and its blur, bloom, ACES tone mapping
 * with grading, FXAA, and the HUD.
 *
 * @par Interface
 * - Vertex: `fullscreen_vertex`, `hud_vertex`.
 * - Fragment: `ssao_fragment`, `ssao_blur_fragment`, `bloom_downsample_fragment`,
 *   `bloom_upsample_fragment`, `tonemap_fragment`, `fxaa_fragment`, `hud_fragment`,
 *   `hud_image_vertex`, `hud_image_fragment`.
 *
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, tonemap_fragment) into the LDR target
 * @endcode
 */

constant uint SSAO_SAMPLE_COUNT = 12;
constant float SSAO_RADIUS_M = 0.55;
constant float SSAO_BIAS_M = 0.02;
constant float SSAO_POWER = 1.6;
constant int SSAO_BLUR_RADIUS_TEXELS = 4;
constant float SSAO_BLUR_DEPTH_SHARPNESS = 12.0;
constant float VIGNETTE_STRENGTH = 0.28;
constant float GRAIN_STRENGTH = 0.025;
constant float GRADE_MIDDLE_GREY = 0.18;
constant float GRADE_CONTRAST = 1.18;
constant float GRADE_SATURATION = 1.12;
constant float3 GRADE_WHITE_BALANCE = float3(1.04, 1.0, 0.94);
constant float3 GRADE_SHADOW_TINT = float3(0.94, 1.0, 1.07);
constant float3 GRADE_HIGHLIGHT_TINT = float3(1.04, 1.0, 0.95);
constant float FXAA_EDGE_THRESHOLD = 0.125;
constant float FXAA_EDGE_THRESHOLD_MIN = 0.0312;
constant uint FXAA_SEARCH_STEP_COUNT = 10;
constant float FXAA_SUBPIXEL_QUALITY = 0.75;

/** @brief `FullscreenVertexOut` is a full-screen triangle's vertex with its texture coordinates. */
struct FullscreenVertexOut {
    float4 position [[position]];
    float2 uv;
};

/**
 * @brief `fullscreen_vertex` makes a triangle that covers the whole target.
 * @par Example
 * @code
 * drawPrimitives(triangle, 0, 3)
 * @endcode
 */
vertex FullscreenVertexOut fullscreen_vertex(uint vertex_id [[vertex_id]]) {
    float2 uv = float2((vertex_id << 1) & 2, vertex_id & 2);
    FullscreenVertexOut out;
    out.position = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

/**
 * @brief `view_position_from_depth` turns a depth-buffer value back into a view-space position.
 * @par Example
 * @code
 * float3 p = view_position_from_depth(frame, uv, depth.sample(s, uv));
 * @endcode
 */
inline float3 view_position_from_depth(constant FrameUniforms& frame, float2 uv, float depth) {
    float4 ndc = float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    float4 view = frame.inv_proj * ndc;
    return view.xyz / view.w;
}

/**
 * @brief `ssao_fragment` estimates ambient occlusion from the depth and normal prepass.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, ssao_fragment) into a half-resolution R8 target
 * @endcode
 */
fragment float4 ssao_fragment(FullscreenVertexOut in [[stage_in]],
                              constant FrameUniforms& frame [[buffer(1)]],
                              depth2d<float> depth [[texture(0)]],
                              texture2d<float> normals [[texture(1)]]) {
    constexpr sampler point_sampler(coord::normalized, filter::nearest, address::clamp_to_edge);
    float center_depth = depth.sample(point_sampler, in.uv);
    if (center_depth <= 0.0) {
        return float4(1.0);
    }
    float3 p = view_position_from_depth(frame, in.uv, center_depth);
    float3 n = normalize(normals.sample(point_sampler, in.uv).xyz);
    float noise = interleaved_gradient_noise(in.position.xy);
    float3 random_vector = normalize(float3(cos(noise * 2.0 * PI), sin(noise * 2.0 * PI), 0.3));
    float3 t = normalize(random_vector - n * dot(random_vector, n));
    float3 b = cross(n, t);
    float occlusion = 0.0;
    for (uint i = 0; i < SSAO_SAMPLE_COUNT; ++i) {
        float fi = float(i) + noise;
        float cos_theta = 1.0 - fi / float(SSAO_SAMPLE_COUNT);
        float sin_theta = sqrt(saturate(1.0 - cos_theta * cos_theta));
        float phi = fi * 2.39996323;
        float3 hemisphere = float3(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);
        float scale = mix(0.15, 1.0, (fi / float(SSAO_SAMPLE_COUNT)) * (fi / float(SSAO_SAMPLE_COUNT)));
        float3 sample_position =
            p + (t * hemisphere.x + b * hemisphere.y + n * hemisphere.z) * SSAO_RADIUS_M * scale;
        float4 clip = frame.proj * float4(sample_position, 1.0);
        float2 sample_uv = clip.xy / clip.w * float2(0.5, -0.5) + 0.5;
        float scene_depth = depth.sample(point_sampler, sample_uv);
        float scene_z = view_position_from_depth(frame, sample_uv, scene_depth).z;
        float range_check = smoothstep(0.0, 1.0, SSAO_RADIUS_M / max(abs(p.z - scene_z), 1e-4));
        occlusion += (scene_z >= sample_position.z + SSAO_BIAS_M ? 1.0 : 0.0) * range_check;
    }
    float ao = pow(saturate(1.0 - occlusion / float(SSAO_SAMPLE_COUNT)), SSAO_POWER);
    return float4(ao);
}

/**
 * @brief `ssao_blur_fragment` blurs the occlusion along one axis, without crossing depth edges.
 * @param direction (1, 0) for the horizontal pass, (0, 1) for the vertical one.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, ssao_blur_fragment), run twice
 * @endcode
 */
fragment float4 ssao_blur_fragment(FullscreenVertexOut in [[stage_in]],
                                   constant FrameUniforms& frame [[buffer(1)]],
                                   constant float2& direction [[buffer(0)]],
                                   texture2d<float> ao [[texture(0)]],
                                   depth2d<float> depth [[texture(1)]]) {
    constexpr sampler point_sampler(coord::normalized, filter::nearest, address::clamp_to_edge);
    float2 texel = 1.0 / float2(ao.get_width(), ao.get_height());
    float center_z = view_position_from_depth(frame, in.uv, depth.sample(point_sampler, in.uv)).z;
    float sum = 0.0;
    float weight_sum = 0.0;
    for (int i = -SSAO_BLUR_RADIUS_TEXELS; i <= SSAO_BLUR_RADIUS_TEXELS; ++i) {
        float2 uv = in.uv + direction * texel * float(i);
        float z = view_position_from_depth(frame, uv, depth.sample(point_sampler, uv)).z;
        float depth_weight = exp(-abs(z - center_z) * SSAO_BLUR_DEPTH_SHARPNESS);
        float spatial_weight = exp(-float(i * i) / 8.0);
        float weight = depth_weight * spatial_weight;
        sum += ao.sample(point_sampler, uv).r * weight;
        weight_sum += weight;
    }
    return float4(sum / max(weight_sum, 1e-4));
}

/**
 * @brief `karis_weight` damps very bright texels so that single fireflies don't bloom.
 * @par Example
 * @code
 * float w = karis_weight(color);
 * @endcode
 */
inline float karis_weight(float3 color) {
    return 1.0 / (1.0 + dot(color, float3(0.2126, 0.7152, 0.0722)));
}

/**
 * @brief `bloom_downsample_fragment` is the 13-tap downsample of Jimenez's bloom.
 * @param is_first_level 1 for the first level, which uses the Karis average.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, bloom_downsample_fragment) into each smaller mip
 * @endcode
 */
fragment float4 bloom_downsample_fragment(FullscreenVertexOut in [[stage_in]],
                                          constant float& is_first_level [[buffer(0)]],
                                          texture2d<float> source [[texture(0)]]) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 t = 1.0 / float2(source.get_width(), source.get_height());
    float2 uv = in.uv;
    float3 a = source.sample(linear_sampler, uv + t * float2(-2, -2)).rgb;
    float3 b = source.sample(linear_sampler, uv + t * float2(0, -2)).rgb;
    float3 c = source.sample(linear_sampler, uv + t * float2(2, -2)).rgb;
    float3 d = source.sample(linear_sampler, uv + t * float2(-2, 0)).rgb;
    float3 e = source.sample(linear_sampler, uv).rgb;
    float3 f = source.sample(linear_sampler, uv + t * float2(2, 0)).rgb;
    float3 g = source.sample(linear_sampler, uv + t * float2(-2, 2)).rgb;
    float3 h = source.sample(linear_sampler, uv + t * float2(0, 2)).rgb;
    float3 i = source.sample(linear_sampler, uv + t * float2(2, 2)).rgb;
    float3 j = source.sample(linear_sampler, uv + t * float2(-1, -1)).rgb;
    float3 k = source.sample(linear_sampler, uv + t * float2(1, -1)).rgb;
    float3 l = source.sample(linear_sampler, uv + t * float2(-1, 1)).rgb;
    float3 m = source.sample(linear_sampler, uv + t * float2(1, 1)).rgb;
    if (is_first_level > 0.5) {
        float3 g0 = (a + b + d + e) * 0.25;
        float3 g1 = (b + c + e + f) * 0.25;
        float3 g2 = (d + e + g + h) * 0.25;
        float3 g3 = (e + f + h + i) * 0.25;
        float3 g4 = (j + k + l + m) * 0.25;
        float w0 = karis_weight(g0), w1 = karis_weight(g1), w2 = karis_weight(g2);
        float w3 = karis_weight(g3), w4 = karis_weight(g4);
        float3 sum = (g0 * w0 + g1 * w1 + g2 * w2 + g3 * w3) * 0.125 + g4 * w4 * 0.5;
        float weight_sum = (w0 + w1 + w2 + w3) * 0.125 + w4 * 0.5;
        return float4(sum / weight_sum, 1.0);
    }
    float3 color = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 +
                   (j + k + l + m) * 0.125;
    return float4(color, 1.0);
}

/**
 * @brief `bloom_upsample_fragment` is a 3x3 tent filter, blended additively into the next mip.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, bloom_upsample_fragment) with additive blending
 * @endcode
 */
fragment float4 bloom_upsample_fragment(FullscreenVertexOut in [[stage_in]],
                                        texture2d<float> source [[texture(0)]]) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 t = 1.0 / float2(source.get_width(), source.get_height());
    float3 sum = source.sample(linear_sampler, in.uv).rgb * 4.0;
    sum += (source.sample(linear_sampler, in.uv + t * float2(0, -1)).rgb +
            source.sample(linear_sampler, in.uv + t * float2(-1, 0)).rgb +
            source.sample(linear_sampler, in.uv + t * float2(1, 0)).rgb +
            source.sample(linear_sampler, in.uv + t * float2(0, 1)).rgb) * 2.0;
    sum += source.sample(linear_sampler, in.uv + t * float2(-1, -1)).rgb +
           source.sample(linear_sampler, in.uv + t * float2(1, -1)).rgb +
           source.sample(linear_sampler, in.uv + t * float2(-1, 1)).rgb +
           source.sample(linear_sampler, in.uv + t * float2(1, 1)).rgb;
    return float4(sum / 16.0, 1.0);
}

/**
 * @brief `aces_fitted` is Stephen Hill's fit of the ACES reference rendering transform.
 * @par Example
 * @code
 * float3 display = aces_fitted(scene_linear * exposure);
 * @endcode
 */
inline float3 aces_fitted(float3 color) {
    const float3x3 input_matrix = float3x3(float3(0.59719, 0.07600, 0.02840),
                                           float3(0.35458, 0.90834, 0.13383),
                                           float3(0.04823, 0.01566, 0.83777));
    const float3x3 output_matrix = float3x3(float3(1.60475, -0.10208, -0.00327),
                                            float3(-0.53108, 1.10813, -0.07276),
                                            float3(-0.07367, -0.00605, 1.07602));
    float3 v = input_matrix * color;
    float3 a = v * (v + 0.0245786) - 0.000090537;
    float3 b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return saturate(output_matrix * (a / b));
}

/**
 * @brief `linear_to_srgb` applies the sRGB transfer function.
 * @par Example
 * @code
 * float3 encoded = linear_to_srgb(display_linear);
 * @endcode
 */
inline float3 linear_to_srgb(float3 color) {
    return select(1.055 * pow(color, 1.0 / 2.4) - 0.055, color * 12.92, color <= 0.0031308);
}

/**
 * @brief `grade_scene` is the scene-linear part of the color grade: white balance, contrast
 * around middle grey in log space, and saturation.
 * @par Example
 * @code
 * float3 graded = grade_scene(color * exposure);
 * @endcode
 */
inline float3 grade_scene(float3 color) {
    color *= GRADE_WHITE_BALANCE;
    float luminance = max(dot(color, float3(0.2126, 0.7152, 0.0722)), 1e-6);
    float graded_luminance = GRADE_MIDDLE_GREY * pow(luminance / GRADE_MIDDLE_GREY, GRADE_CONTRAST);
    color *= graded_luminance / luminance;
    return max(mix(float3(graded_luminance), color, GRADE_SATURATION), 0.0);
}

/**
 * @brief `grade_display` is the display-linear part of the color grade: cool shadows and warm
 * highlights.
 * @par Example
 * @code
 * float3 graded = grade_display(aces_fitted(color));
 * @endcode
 */
inline float3 grade_display(float3 color) {
    float luminance = saturate(dot(color, float3(0.2126, 0.7152, 0.0722)));
    float3 tint = mix(GRADE_SHADOW_TINT, GRADE_HIGHLIGHT_TINT, smoothstep(0.05, 0.6, luminance));
    return saturate(color * tint);
}

/**
 * @brief `tonemap_fragment` adds bloom, exposes, tone maps with ACES, grades, and stores luma
 * in alpha for FXAA.
 * @par Interface
 * `frame.effect_params.x` turns the color grade on, and `frame.effect_params.w` is the number of
 * bloom levels summed into `bloom`.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, tonemap_fragment) into the RGBA8 LDR target
 * @endcode
 */
fragment float4 tonemap_fragment(FullscreenVertexOut in [[stage_in]],
                                 constant FrameUniforms& frame [[buffer(1)]],
                                 texture2d<float> hdr [[texture(0)]],
                                 texture2d<float> bloom [[texture(1)]]) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float3 color = hdr.sample(linear_sampler, in.uv).rgb;
    float3 bloom_color = bloom.sample(linear_sampler, in.uv).rgb / frame.effect_params.w;
    color = mix(color, bloom_color, frame.render_params.y);
    color *= frame.render_params.x;
    bool is_color_grade_enabled = frame.effect_params.x > 0.5;
    if (is_color_grade_enabled) {
        color = grade_display(aces_fitted(grade_scene(color)));
    } else {
        float luminance = dot(color, float3(0.2126, 0.7152, 0.0722));
        color = aces_fitted(max(mix(float3(luminance), color, 1.08), 0.0));
    }
    float2 centered = in.uv - 0.5;
    float vignette = 1.0 - VIGNETTE_STRENGTH * smoothstep(0.2, 0.85, length(centered * float2(1.1, 1.0)));
    color *= vignette;
    float damage = frame.post_params.x;
    float edge = smoothstep(0.25, 0.75, length(centered));
    color = mix(color, float3(0.45, 0.02, 0.01), damage * edge * 0.8);
    float3 encoded = linear_to_srgb(saturate(color));
    float grain = interleaved_gradient_noise(in.position.xy + fract(frame.camera_position_m.w) * 97.0) - 0.5;
    encoded = saturate(encoded + grain * GRAIN_STRENGTH);
    float luma = dot(encoded, float3(0.299, 0.587, 0.114));
    return float4(encoded, luma);
}

/**
 * @brief `fxaa_fragment` is FXAA 3.11 (quality), reading luma from alpha; it also scales the
 * internal resolution up to the window.
 * @param is_enabled 0 copies the image without anti-aliasing.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, fxaa_fragment) into the drawable
 * @endcode
 */
fragment float4 fxaa_fragment(FullscreenVertexOut in [[stage_in]],
                              constant float& is_enabled [[buffer(0)]],
                              texture2d<float> ldr [[texture(0)]]) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 texel = 1.0 / float2(ldr.get_width(), ldr.get_height());
    float2 uv = in.uv;
    float4 center = ldr.sample(linear_sampler, uv);
    if (is_enabled < 0.5) {
        return float4(center.rgb, 1.0);
    }
    float luma_center = center.a;
    float luma_down = ldr.sample(linear_sampler, uv + float2(0, 1) * texel).a;
    float luma_up = ldr.sample(linear_sampler, uv + float2(0, -1) * texel).a;
    float luma_left = ldr.sample(linear_sampler, uv + float2(-1, 0) * texel).a;
    float luma_right = ldr.sample(linear_sampler, uv + float2(1, 0) * texel).a;
    float luma_min = min(luma_center, min(min(luma_down, luma_up), min(luma_left, luma_right)));
    float luma_max = max(luma_center, max(max(luma_down, luma_up), max(luma_left, luma_right)));
    float luma_range = luma_max - luma_min;
    if (luma_range < max(FXAA_EDGE_THRESHOLD_MIN, luma_max * FXAA_EDGE_THRESHOLD)) {
        return float4(center.rgb, 1.0);
    }
    float luma_down_left = ldr.sample(linear_sampler, uv + float2(-1, 1) * texel).a;
    float luma_up_right = ldr.sample(linear_sampler, uv + float2(1, -1) * texel).a;
    float luma_up_left = ldr.sample(linear_sampler, uv + float2(-1, -1) * texel).a;
    float luma_down_right = ldr.sample(linear_sampler, uv + float2(1, 1) * texel).a;
    float luma_down_up = luma_down + luma_up;
    float luma_left_right = luma_left + luma_right;
    float luma_left_corners = luma_down_left + luma_up_left;
    float luma_down_corners = luma_down_left + luma_down_right;
    float luma_right_corners = luma_down_right + luma_up_right;
    float luma_up_corners = luma_up_right + luma_up_left;
    float edge_horizontal = abs(-2.0 * luma_left + luma_left_corners) +
                            abs(-2.0 * luma_center + luma_down_up) * 2.0 +
                            abs(-2.0 * luma_right + luma_right_corners);
    float edge_vertical = abs(-2.0 * luma_up + luma_up_corners) +
                          abs(-2.0 * luma_center + luma_left_right) * 2.0 +
                          abs(-2.0 * luma_down + luma_down_corners);
    bool is_horizontal = edge_horizontal >= edge_vertical;
    float luma_1 = is_horizontal ? luma_up : luma_left;
    float luma_2 = is_horizontal ? luma_down : luma_right;
    float gradient_1 = luma_1 - luma_center;
    float gradient_2 = luma_2 - luma_center;
    bool is_1_steepest = abs(gradient_1) >= abs(gradient_2);
    float gradient_scaled = 0.25 * max(abs(gradient_1), abs(gradient_2));
    float step_length = is_horizontal ? texel.y : texel.x;
    float luma_local_average;
    if (is_1_steepest) {
        step_length = -step_length;
        luma_local_average = 0.5 * (luma_1 + luma_center);
    } else {
        luma_local_average = 0.5 * (luma_2 + luma_center);
    }
    float2 current_uv = uv;
    if (is_horizontal) {
        current_uv.y += step_length * 0.5;
    } else {
        current_uv.x += step_length * 0.5;
    }
    float2 offset = is_horizontal ? float2(texel.x, 0.0) : float2(0.0, texel.y);
    float2 uv_1 = current_uv - offset;
    float2 uv_2 = current_uv + offset;
    float luma_end_1 = ldr.sample(linear_sampler, uv_1).a - luma_local_average;
    float luma_end_2 = ldr.sample(linear_sampler, uv_2).a - luma_local_average;
    bool is_reached_1 = abs(luma_end_1) >= gradient_scaled;
    bool is_reached_2 = abs(luma_end_2) >= gradient_scaled;
    for (uint i = 0; i < FXAA_SEARCH_STEP_COUNT && !(is_reached_1 && is_reached_2); ++i) {
        float quality = i < 2 ? 1.0 : (i < 5 ? 1.5 : (i < 8 ? 2.0 : 4.0));
        if (!is_reached_1) {
            uv_1 -= offset * quality;
            luma_end_1 = ldr.sample(linear_sampler, uv_1).a - luma_local_average;
            is_reached_1 = abs(luma_end_1) >= gradient_scaled;
        }
        if (!is_reached_2) {
            uv_2 += offset * quality;
            luma_end_2 = ldr.sample(linear_sampler, uv_2).a - luma_local_average;
            is_reached_2 = abs(luma_end_2) >= gradient_scaled;
        }
    }
    float distance_1 = is_horizontal ? (uv.x - uv_1.x) : (uv.y - uv_1.y);
    float distance_2 = is_horizontal ? (uv_2.x - uv.x) : (uv_2.y - uv.y);
    bool is_direction_1 = distance_1 < distance_2;
    float distance_final = min(distance_1, distance_2);
    float edge_thickness = distance_1 + distance_2;
    bool is_luma_center_smaller = luma_center < luma_local_average;
    bool is_correct_variation_1 = (luma_end_1 < 0.0) != is_luma_center_smaller;
    bool is_correct_variation_2 = (luma_end_2 < 0.0) != is_luma_center_smaller;
    bool is_correct_variation = is_direction_1 ? is_correct_variation_1 : is_correct_variation_2;
    float pixel_offset = -distance_final / edge_thickness + 0.5;
    float final_offset = is_correct_variation ? pixel_offset : 0.0;
    float luma_average = (1.0 / 12.0) * (2.0 * (luma_down_up + luma_left_right) +
                                         luma_left_corners + luma_right_corners);
    float subpixel_offset_1 = saturate(abs(luma_average - luma_center) / luma_range);
    float subpixel_offset_2 = (-2.0 * subpixel_offset_1 + 3.0) * subpixel_offset_1 * subpixel_offset_1;
    float subpixel_offset = subpixel_offset_2 * subpixel_offset_2 * FXAA_SUBPIXEL_QUALITY;
    final_offset = max(final_offset, subpixel_offset);
    float2 final_uv = uv;
    if (is_horizontal) {
        final_uv.y += final_offset * step_length;
    } else {
        final_uv.x += final_offset * step_length;
    }
    return float4(ldr.sample(linear_sampler, final_uv).rgb, 1.0);
}

/** @brief `HudVertexOut` is one corner of a HUD rectangle. */
struct HudVertexOut {
    float4 position [[position]];
    float4 color;
};

/**
 * @brief `hud_vertex` places one corner of a HUD rectangle, given in window pixels.
 * @par Example
 * @code
 * drawPrimitives(triangle_strip, 0, 4, rect_count)
 * @endcode
 */
vertex HudVertexOut hud_vertex(uint vertex_id [[vertex_id]], uint instance_id [[instance_id]],
                               device const GpuHudRect* rects [[buffer(0)]],
                               constant float2& window_size_px [[buffer(1)]]) {
    GpuHudRect rect = rects[instance_id];
    float2 corner = float2(vertex_id & 1, vertex_id >> 1);
    float2 pixel = rect.rect_px.xy + corner * rect.rect_px.zw;
    HudVertexOut out;
    out.position = float4(pixel / window_size_px * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    out.color = rect.color;
    return out;
}

/**
 * @brief `hud_fragment` fills a HUD rectangle with its color, in sRGB.
 * @par Example
 * @code
 * pipeline(hud_vertex, hud_fragment) with alpha blending into the drawable
 * @endcode
 */
fragment float4 hud_fragment(HudVertexOut in [[stage_in]]) {
    return in.color;
}

/** @brief `GpuHudImage` is where a HUD picture goes, in window pixels, and its size in pixels. */
struct GpuHudImage {
    float4 rect_px;
    uint2 size_px;
    uint2 padding;
};

/** @brief `HudImageVertexOut` is one corner of a HUD picture and where it is in the picture. */
struct HudImageVertexOut {
    float4 position [[position]];
    float2 uv;
};

/**
 * @brief `hud_image_vertex` places one corner of a HUD picture, given in window pixels.
 * @par Example
 * @code
 * drawPrimitives(triangle_strip, 0, 4)
 * @endcode
 */
vertex HudImageVertexOut hud_image_vertex(uint vertex_id [[vertex_id]],
                                          constant GpuHudImage& image [[buffer(0)]],
                                          constant float2& window_size_px [[buffer(1)]]) {
    float2 corner = float2(vertex_id & 1, vertex_id >> 1);
    float2 pixel = image.rect_px.xy + corner * image.rect_px.zw;
    HudImageVertexOut out;
    out.position = float4(pixel / window_size_px * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    out.uv = corner;
    return out;
}

/**
 * @brief `hud_image_fragment` reads the HUD picture's nearest pixel from the frame's buffer.
 * @par Example
 * @code
 * pipeline(hud_image_vertex, hud_image_fragment) with alpha blending into the drawable
 * @endcode
 */
fragment float4 hud_image_fragment(HudImageVertexOut in [[stage_in]],
                                   constant GpuHudImage& image [[buffer(0)]],
                                   device const uchar4* pixels [[buffer(1)]]) {
    uint2 pixel = min(uint2(in.uv * float2(image.size_px)), image.size_px - 1);
    return float4(pixels[pixel.y * image.size_px.x + pixel.x]) / 255.0;
}
