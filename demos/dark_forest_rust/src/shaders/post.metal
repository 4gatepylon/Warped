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
 * @brief `LensUniforms` matches `GpuLensUniforms`: each mouth's (centre, lens throat) and (reach, λ²,
 * partner, floor height), and (mouth count, 1 while capturing, atlas rows, unused); then each
 * mouth's live slot plus 1 (0 for none, negative when a robot is within its reach), the robots'
 * bounding spheres, (unused, sphere count, near plane, glow segment count), and the lasers' and other
 * glows' segments (start and end, w their radius).
 */
struct LensUniforms {
    float4 centres[16];
    float4 shapes[16];
    float4 params;
    float4 live_slots[4];
    float4 movers[16];
    float4 live_params;
    float4 glow_starts[24];
    float4 glow_ends[24];
};

/**
 * @brief `LENS_DEPTH_NEAR_M` and `LENS_DEPTH_FAR_M` bound the atlas's log-encoded alpha (the static
 * world's distance along each face's axis); they match `render::LENS_DEPTH_NEAR_M` and
 * `LENS_DEPTH_FAR_M`. `LENS_DEPTH_SLACK_FRAC` lets a robot right against a wall still show.
 */
constant float LENS_DEPTH_NEAR_M = 0.25;
constant float LENS_DEPTH_FAR_M = 128.0;
constant float LENS_DEPTH_SLACK_FRAC = 0.03;
/**
 * @brief `LENS_MOVER_THICKNESS_M` is how far behind a robot's surface, seen from a mouth's centre, a
 * ray still hits it; `LENS_MOVER_STEP_COUNT` samples a ray across each robot's bounding sphere, and
 * `LENS_MOVER_FAR_M` is how far past a reach the lens looks for robots.
 */
constant float LENS_MOVER_THICKNESS_M = 0.6;
constant int LENS_MOVER_STEP_COUNT = 12;
constant float LENS_MOVER_FAR_M = 80.0;

/** @brief `LENS_VISIBLE_FRAC` of a mouth's reach shows the lens; it fades in from `LENS_FADE_FRAC`. */
constant float LENS_VISIBLE_FRAC = 0.8;
constant float LENS_FADE_FRAC = 0.6;
/** @brief `LENS_PROXY_M` is the distance of the world around a mouth that its atlas lookups assume. */
constant float LENS_PROXY_M = 12.0;
constant int LENS_MAX_STEP_COUNT = 64;
/**
 * @brief From inside a mouth's visible sphere, how much a ray shows the lens depends on how much it bends, not on
 * how near it passes the centre: none below `LENS_INSIDE_BEND_MIN_RAD`, fully from `LENS_INSIDE_BEND_FULL_RAD`
 * (and fully through the throat), so no ray shows a faint lens picture over the direct view; this takes over
 * across the last `LENS_INSIDE_BLEND_M` inside the sphere.
 */
constant float LENS_INSIDE_BEND_MIN_RAD = 0.02;
constant float LENS_INSIDE_BEND_FULL_RAD = 0.07;
constant float LENS_INSIDE_BLEND_M = 0.5;

/**
 * @brief `get_lens_bend` is ∇ln n of mouth w's foil lens at x, tapered by (1 − (ρ/reach)²)² to
 * nothing at its reach: the hyperbolic demo's `compute_gradient` in flat space.
 * @par Example
 * @code
 * float3 g = get_lens_bend(lens, w, x);
 * @endcode
 */
static float3 get_lens_bend(constant LensUniforms& lens, int w, float3 x) {
    float3 offset = x - lens.centres[w].xyz;
    float rho = max(length(offset), 1e-4);
    float reach = lens.shapes[w].x;
    float f = rho / reach;
    if (f >= 1.0) {
        return float3(0.0);
    }
    float pt = lens.centres[w].w, lambda_sq = lens.shapes[w].y;
    float ir = 1.0 / rho, psi = 1.0 + pt * ir, ir2 = ir / (psi * psi), rs = 4.0 * pt;
    float slope = -2.0 * pt * ir * ir / psi - rs * psi * (1.0 - pt * ir) * ir2 * ir2 / (2.0 * (1.0 + lambda_sq - rs * ir2));
    float ln_n = 2.0 * log(psi) - 0.5 * log((1.0 - rs * ir2 + lambda_sq) / (1.0 + lambda_sq));
    float window = (1.0 - f * f) * (1.0 - f * f), window_slope = -4.0 * f * (1.0 - f * f) / reach;
    return offset * ir * (slope * window + ln_n * window_slope);
}

/**
 * @brief `get_lens_face_uv` is where direction d falls in a row of six cube faces (oriented as
 * `LENS_FACES`, each `face_px` wide): u across the row, v down the face; `axis_cos` is the cosine
 * between d and its face's axis.
 * @par Example
 * @code
 * float2 uv = get_lens_face_uv(d, face_px, axis_cos);
 * @endcode
 */
static float2 get_lens_face_uv(float3 d, float face_px, thread float& axis_cos) {
    float3 a = abs(d);
    int face;
    float3 forward, up;
    if (a.x >= a.y && a.x >= a.z) {
        face = d.x > 0.0 ? 0 : 1;
        forward = float3(sign(d.x), 0.0, 0.0);
        up = float3(0.0, 1.0, 0.0);
    } else if (a.y >= a.z) {
        face = d.y > 0.0 ? 2 : 3;
        forward = float3(0.0, sign(d.y), 0.0);
        up = float3(0.0, 0.0, sign(d.y));
    } else {
        face = d.z > 0.0 ? 4 : 5;
        forward = float3(0.0, 0.0, sign(d.z));
        up = float3(0.0, 1.0, 0.0);
    }
    float3 right = cross(forward, up);
    axis_cos = dot(d, forward) / length(d);
    float2 st = float2(dot(d, right), dot(d, up)) / dot(d, forward);
    float2 uv_face = clamp(float2(0.5 + 0.5 * st.x, 0.5 - 0.5 * st.y), 0.5 / face_px, 1.0 - 0.5 / face_px);
    return float2((float(face) + uv_face.x) / 6.0, uv_face.y);
}

/**
 * @brief `sample_lens_atlas` is the world seen from mouth w's centre in direction d (its atlas row's
 * six faces, oriented as `LENS_FACES`).
 * @par Example
 * @code
 * float3 color = sample_lens_atlas(lens, atlas, w, d);
 * @endcode
 */
static float3 sample_lens_atlas(constant LensUniforms& lens, texture2d<float> atlas, int w, float3 d) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float axis_cos;
    float2 uv = get_lens_face_uv(d, float(atlas.get_width()) / 6.0, axis_cos);
    uv.y = (float(w) + uv.y) / lens.params.z;
    return atlas.sample(linear_sampler, uv, level(0.0)).rgb;
}

/**
 * @brief `get_static_distance_m` is how far the static world is from mouth w's centre in direction
 * d, from its atlas row's alpha.
 * @par Example
 * @code
 * float wall_m = get_static_distance_m(atlas, w, d);
 * @endcode
 */
static float get_static_distance_m(texture2d<float> atlas, int w, float3 d) {
    float face_px = float(atlas.get_width()) / 6.0;
    float axis_cos;
    float2 uv = get_lens_face_uv(d, face_px, axis_cos);
    uint2 texel = uint2(uv.x * float(atlas.get_width()), (float(w) + uv.y) * face_px);
    float encoded = atlas.read(texel).a;
    return LENS_DEPTH_NEAR_M * pow(LENS_DEPTH_FAR_M / LENS_DEPTH_NEAR_M, encoded) / axis_cos;
}

/**
 * @brief `get_live_slot` is the live slot that shows the robots and lasers around mouth w (−1 for
 * none this frame).
 * @par Example
 * @code
 * int slot = get_live_slot(lens, w);
 * @endcode
 */
static int get_live_slot(constant LensUniforms& lens, int w) {
    return int(abs(lens.live_slots[w / 4][w % 4]) + 0.5) - 1;
}

/**
 * @brief `get_inner_live_slot` is mouth w's live slot if a robot stands within its reach, where
 * rays bend (−1 otherwise), so the bent steps only look for robots where one can be.
 * @par Example
 * @code
 * int slot = get_inner_live_slot(lens, current);
 * @endcode
 */
static int get_inner_live_slot(constant LensUniforms& lens, int w) {
    float encoded = lens.live_slots[w / 4][w % 4];
    return encoded < 0.0 ? int(-encoded + 0.5) - 1 : -1;
}

/**
 * @brief `get_live_distance_m` is how far the nearest robot drawn in live slot `slot` is from its
 * mouth's centre in direction d (1e30 for none).
 * @par Example
 * @code
 * float robot_m = get_live_distance_m(lens, live_depth, slot, d);
 * @endcode
 */
static float get_live_distance_m(constant LensUniforms& lens, depth2d_array<float> live_depth, int slot,
                                 float3 d) {
    float face_px = float(live_depth.get_width()) / 6.0;
    float axis_cos;
    float2 uv = get_lens_face_uv(d, face_px, axis_cos);
    float depth = live_depth.read(uint2(uv.x * float(live_depth.get_width()), uv.y * face_px), uint(slot));
    return depth > 0.0 ? lens.live_params.z / depth / axis_cos : 1e30;
}

/**
 * @brief `is_on_live_mover` says whether the point at `offset` from the centre of live slot `slot`'s
 * mouth is inside a robot drawn there: at most `depth_m` behind its surface seen from that centre.
 * @par Example
 * @code
 * bool is_hit = is_on_live_mover(lens, live_depth, slot, x - c, LENS_MOVER_THICKNESS_M);
 * @endcode
 */
static bool is_on_live_mover(constant LensUniforms& lens, depth2d_array<float> live_depth, int slot,
                             float3 offset, float depth_m) {
    float r = length(offset);
    float robot_m = get_live_distance_m(lens, live_depth, slot, offset);
    return r >= robot_m && r - robot_m < depth_m;
}

/**
 * @brief `find_live_mover_m` is how far along the straight ray from x heading v (in the
 * surroundings of mouth w, live slot `slot`, up to `far_m`) it first enters a robot (−1 for none):
 * it samples the ray across each robot's bounding sphere.
 * @par Example
 * @code
 * float t = find_live_mover_m(lens, live_depth, w, slot, x, v, far_m);
 * @endcode
 */
static float find_live_mover_m(constant LensUniforms& lens, depth2d_array<float> live_depth, int w,
                               int slot, float3 x, float3 v, float far_m) {
    float3 c = lens.centres[w].xyz;
    float best_m = far_m;
    bool is_found = false;
    int mover_count = int(lens.live_params.y + 0.5);
    for (int m = 0; m < mover_count; m++) {
        float3 oc = x - lens.movers[m].xyz;
        float b = dot(oc, v);
        float radius = lens.movers[m].w;
        float disc = b * b - (dot(oc, oc) - radius * radius);
        if (disc <= 0.0) {
            continue;
        }
        float t_start = max(-b - sqrt(disc), 0.0);
        float t_end = min(-b + sqrt(disc), best_m);
        for (int i = 0; i <= LENS_MOVER_STEP_COUNT && t_start <= t_end; i++) {
            float t = mix(t_start, t_end, float(i) / float(LENS_MOVER_STEP_COUNT));
            if (is_on_live_mover(lens, live_depth, slot, x + v * t - c, LENS_MOVER_THICKNESS_M)) {
                best_m = t;
                is_found = true;
                break;
            }
        }
    }
    return is_found ? best_m : -1.0;
}

/**
 * @brief `find_live_glow_offset` says whether the straight ray from x heading v (from `reach_m` behind x, where
 * the bent part is close to straight, to `far_m` ahead) passes a laser or other glow segment within its radius
 * plus a live face's texel at that distance, and gives the offset from mouth w's centre of the point on the ray
 * nearest the first one it passes.
 * @par Example
 * @code
 * bool has_glow = find_live_glow_offset(lens, w, face_px, x, v, reach, far_m, glow_offset);
 * @endcode
 */
static bool find_live_glow_offset(constant LensUniforms& lens, int w, float face_px, float3 x, float3 v,
                                  float reach_m, float far_m, thread float3& glow_offset) {
    float3 c = lens.centres[w].xyz;
    float3 ray_start = x - v * reach_m;
    float ray_length_m = reach_m + far_m;
    float best_t = 1e30;
    int glow_count = int(lens.live_params.w + 0.5);
    for (int i = 0; i < glow_count; i++) {
        float3 a = lens.glow_starts[i].xyz;
        float3 ab = lens.glow_ends[i].xyz - a;
        float3 r = ray_start - a;
        float ab_sq = dot(ab, ab), v_ab = dot(v, ab), v_r = dot(v, r), ab_r = dot(ab, r);
        float denominator = ab_sq - v_ab * v_ab;
        float t = denominator > 1e-8 ? clamp((v_ab * ab_r - ab_sq * v_r) / denominator, 0.0, ray_length_m)
                                     : clamp(-v_r, 0.0, ray_length_m);
        float s = ab_sq > 1e-8 ? clamp((dot(ab, ray_start + v * t - a)) / ab_sq, 0.0, 1.0) : 0.0;
        t = clamp(dot(a + ab * s - ray_start, v), 0.0, ray_length_m);
        float3 on_ray = ray_start + v * t;
        float reach = lens.glow_starts[i].w + 2.0 * length(on_ray - c) / face_px;
        if (distance(on_ray, a + ab * s) < reach && t < best_t) {
            best_t = t;
            glow_offset = on_ray - c;
        }
    }
    return best_t < 1e30;
}

/**
 * @brief `get_graded_color` exposes a scene-linear color, tone maps it with ACES and grades it
 * (`frame.effect_params.x` turns the grade on): the display-linear color before any vignette.
 * @par Example
 * @code
 * float3 encoded = linear_to_srgb(saturate(get_graded_color(frame, hdr)));
 * @endcode
 */
static float3 get_graded_color(constant FrameUniforms& frame, float3 color) {
    color *= frame.render_params.x;
    if (frame.effect_params.x > 0.5) {
        return grade_display(aces_fitted(grade_scene(color)));
    }
    float luminance = dot(color, float3(0.2126, 0.7152, 0.0722));
    return aces_fitted(max(mix(float3(luminance), color, 1.08), 0.0));
}

/**
 * @brief `get_display_color` is `get_graded_color` encoded as sRGB.
 * @par Example
 * @code
 * float3 encoded = get_display_color(frame, hdr);
 * @endcode
 */
static float3 get_display_color(constant FrameUniforms& frame, float3 color) {
    return linear_to_srgb(saturate(get_graded_color(frame, color)));
}

/**
 * @brief `add_live_color` puts what live slot `slot` shows over the atlas color `color` of mouth w:
 * the robot at `hit_offset` from w's centre if the ray hit one that the static world doesn't hide
 * (`is_hit`), otherwise, if the ray passes a laser or glow (`has_glow`), its light at `glow_offset` from w's
 * centre, unless the static world hides it.
 * @par Example
 * @code
 * color = add_live_color(frame, atlas, live_color, live_depth, lens, w, slot, is_hit, hit_offset, has_glow,
 *                        glow_offset, color);
 * @endcode
 */
static float3 add_live_color(constant FrameUniforms& frame, texture2d<float> atlas,
                             texture2d_array<float> live_color, depth2d_array<float> live_depth,
                             constant LensUniforms& lens, int w, int slot, bool is_hit,
                             float3 hit_offset, bool has_glow, float3 glow_offset, float3 color) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float face_px = float(live_color.get_width()) / 6.0;
    float axis_cos;
    if (is_hit) {
        float robot_m = get_live_distance_m(lens, live_depth, slot, hit_offset);
        if (robot_m > get_static_distance_m(atlas, w, hit_offset) * (1.0 + LENS_DEPTH_SLACK_FRAC)) {
            return color;
        }
        float2 uv = get_lens_face_uv(hit_offset, face_px, axis_cos);
        float4 robot = live_color.sample(linear_sampler, uv, uint(slot), level(0.0));
        float coverage = saturate(robot.a);
        return mix(color, get_display_color(frame, robot.rgb / max(coverage, 1e-4)), coverage);
    }
    if (!has_glow || length(glow_offset) > get_static_distance_m(atlas, w, glow_offset) * (1.0 + LENS_DEPTH_SLACK_FRAC)) {
        return color;
    }
    float2 uv = get_lens_face_uv(glow_offset, face_px, axis_cos);
    float4 glow = live_color.sample(linear_sampler, uv, uint(slot), level(0.0));
    float3 light = glow.rgb * (1.0 - saturate(glow.a));
    if (all(light <= 0.0)) {
        return color;
    }
    return 1.0 - (1.0 - color) * (1.0 - get_display_color(frame, light));
}

/**
 * @brief `find_lens_mouth` is the mouth whose visible lens the eye ray through `uv` meets first
 * (−1 for none), with how far away it meets it, how close the ray passes its centre and the ray.
 * @par Example
 * @code
 * int w = find_lens_mouth(frame, lens, uv, entry_m, closest_m, dir);
 * @endcode
 */
static int find_lens_mouth(constant FrameUniforms& frame, constant LensUniforms& lens, float2 uv,
                           thread float& entry_m, thread float& closest_m, thread float3& dir) {
    int count = int(lens.params.x + 0.5);
    float3 eye = frame.camera_position_m.xyz;
    float2 ndc = float2(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    float4 near_point = frame.inv_view_proj * float4(ndc, 1.0, 1.0);
    dir = normalize(near_point.xyz / near_point.w - eye);
    int w = -1;
    entry_m = 1e30;
    closest_m = 0.0;
    for (int i = 0; i < count; i++) {
        float3 oc = eye - lens.centres[i].xyz;
        float b = dot(oc, dir);
        float closest_sq = dot(oc, oc) - b * b;
        float visible = lens.shapes[i].x * LENS_VISIBLE_FRAC;
        if (closest_sq >= visible * visible) {
            continue;
        }
        float half_chord = sqrt(visible * visible - closest_sq);
        if (-b + half_chord < 0.0) {
            continue;
        }
        float visible_entry = max(-b - half_chord, 0.0);
        if (visible_entry < entry_m) {
            entry_m = visible_entry;
            w = i;
            closest_m = sqrt(closest_sq);
        }
    }
    return w;
}

/**
 * @brief `get_lens_color` traces the eye ray through `uv` into the nearest wormhole mouth: bent
 * by its lens, through its throat to the partner (inversion, then the mirror diag(1, 1, -1)),
 * until it leaves a reach or meets the floor; it returns the atlas color there (lined up on the
 * floor or a `LENS_PROXY_M` sphere) and how much it covers the scene, before any depth test. A ray that reaches
 * the mouth from below its floor (a deck's mouth seen from the ground) shows no lens, since the floor hides the
 * mouth from there (W-139).
 * Where the ray ends near a mouth with a live slot, it also shows the first robot on its way (bent
 * or straight, unless the static world hides it) and the lasers drawn there. From inside a mouth's visible
 * sphere, the lens covers the scene as far as it bends the ray (fully through the throat).
 * @par Example
 * @code
 * float4 lensed = get_lens_color(frame, lens, atlas, live_color, live_depth, in.uv);
 * @endcode
 */
static float4 get_lens_color(constant FrameUniforms& frame, constant LensUniforms& lens,
                             texture2d<float> atlas, texture2d_array<float> live_color,
                             depth2d_array<float> live_depth, float2 uv) {
    float entry_m, closest_m;
    float3 dir;
    int w = find_lens_mouth(frame, lens, uv, entry_m, closest_m, dir);
    if (w < 0) {
        return float4(0.0);
    }
    float3 eye = frame.camera_position_m.xyz;
    float reach = lens.shapes[w].x;
    float3 oc = eye - lens.centres[w].xyz;
    float b = dot(oc, dir);
    float start_m = max(-b - sqrt(max(reach * reach - (dot(oc, oc) - b * b), 0.0)), 0.0);
    float3 x = eye + dir * start_m, v = dir;
    if (x.y < lens.shapes[w].w) {
        return float4(0.0);
    }
    int current = w;
    bool is_hit = false;
    bool has_crossed = false;
    for (int k = 0; k < LENS_MAX_STEP_COUNT && !is_hit; k++) {
        float3 offset = x - lens.centres[current].xyz;
        float rho = length(offset);
        float pt = lens.centres[current].w, reach_now = lens.shapes[current].x;
        if (rho >= reach_now && dot(offset, v) > 0.0) {
            break;
        }
        if (x.y < lens.shapes[current].w) {
            break;
        }
        if (rho < pt) {
            int partner = int(lens.shapes[current].z + 0.5);
            float3 n = offset / max(rho, 1e-4);
            float3 mirror = float3(1.0, 1.0, -1.0);
            v = normalize((v - 2.0 * dot(v, n) * n) * mirror);
            x = lens.centres[partner].xyz + n * mirror * (lens.centres[partner].w * pt / max(rho, 1e-4));
            current = partner;
            has_crossed = true;
            continue;
        }
        float h = clamp(0.3 * (rho - 0.5 * pt), 0.05, reach_now / 8.0);
        float3 g = get_lens_bend(lens, current, x);
        v = normalize(v + (g - dot(g, v) * v) * h);
        x += v * h;
        int slot_now = get_inner_live_slot(lens, current);
        is_hit = slot_now >= 0 && is_on_live_mover(lens, live_depth, slot_now, x - lens.centres[current].xyz,
                                                   max(LENS_MOVER_THICKNESS_M, h));
    }
    float3 c = lens.centres[current].xyz;
    float3 oc_proxy = x - c;
    float b_proxy = dot(oc_proxy, v);
    float t = -b_proxy + sqrt(max(b_proxy * b_proxy - (dot(oc_proxy, oc_proxy) - LENS_PROXY_M * LENS_PROXY_M), 0.0));
    float floor_t = LENS_MOVER_FAR_M;
    if (v.y < -1e-4) {
        floor_t = (lens.shapes[current].w - x.y) / v.y;
        t = min(t, floor_t);
    }
    float3 d = normalize(x + v * max(t, 0.0) - c);
    float3 color = sample_lens_atlas(lens, atlas, current, d);
    int slot = get_live_slot(lens, current);
    if (slot >= 0) {
        float3 hit_offset = x - c;
        if (!is_hit && floor_t > 0.0) {
            float mover_t = find_live_mover_m(lens, live_depth, current, slot, x, v, min(floor_t, LENS_MOVER_FAR_M));
            is_hit = mover_t >= 0.0;
            hit_offset = x + v * mover_t - c;
        }
        float3 glow_offset = float3(0.0);
        bool has_glow = !is_hit && find_live_glow_offset(lens, current, float(live_color.get_width()) / 6.0, x, v,
                                                         lens.shapes[current].x, min(floor_t, LENS_MOVER_FAR_M),
                                                         glow_offset);
        color = add_live_color(frame, atlas, live_color, live_depth, lens, current, slot, is_hit, hit_offset,
                               has_glow, glow_offset, color);
    }
    float weight = 1.0 - smoothstep(LENS_FADE_FRAC * reach, LENS_VISIBLE_FRAC * reach, closest_m);
    float inside_frac = smoothstep(LENS_VISIBLE_FRAC * reach, LENS_VISIBLE_FRAC * reach - LENS_INSIDE_BLEND_M,
                                   length(oc));
    float bend_frac = has_crossed ? 1.0 : smoothstep(LENS_INSIDE_BEND_MIN_RAD, LENS_INSIDE_BEND_FULL_RAD,
                                                     acos(clamp(dot(dir, v), -1.0, 1.0)));
    return float4(color, mix(weight, bend_frac, inside_frac));
}

/**
 * @brief `lens_fragment` draws the wormhole lens at half resolution (color and coverage); the
 * tonemap pass tests it against the scene's depth at full resolution.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, lens_fragment) into the half-resolution lens target
 * @endcode
 */
fragment float4 lens_fragment(FullscreenVertexOut in [[stage_in]],
                              constant FrameUniforms& frame [[buffer(1)]],
                              constant LensUniforms& lens [[buffer(2)]],
                              texture2d<float> atlas [[texture(0)]],
                              texture2d_array<float> live_color [[texture(1)]],
                              depth2d_array<float> live_depth [[texture(2)]]) {
    return get_lens_color(frame, lens, atlas, live_color, live_depth, in.uv);
}

/**
 * @brief `sample_spline` reads `picture` at `uv` through a cubic B-spline of its 4 × 4 nearest texels, in four bilinear
 * taps: a smooth upsampling without steps or ringing, for the half-resolution lens picture (W-138, no aliasing).
 * @par Example
 * @code
 * float4 c = sample_spline(lens_picture, in.uv);
 * @endcode
 */
static float4 sample_spline(texture2d<float> picture, float2 uv) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 size = float2(picture.get_width(), picture.get_height());
    float2 p = uv * size - 0.5, i = floor(p), f = p - i, f2 = f * f, f3 = f2 * f;
    float2 w0 = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0, w1 = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
    float2 w2 = (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0, w3 = f3 / 6.0;
    float2 g0 = w0 + w1, g1 = w2 + w3;
    float2 h0 = (i - 0.5 + w1 / g0) / size, h1 = (i + 1.5 + w3 / g1) / size;
    return g0.y * (g0.x * picture.sample(linear_sampler, h0) + g1.x * picture.sample(linear_sampler, float2(h1.x, h0.y)))
        + g1.y * (g0.x * picture.sample(linear_sampler, float2(h0.x, h1.y)) + g1.x * picture.sample(linear_sampler, h1));
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
                                 texture2d<float> bloom [[texture(1)]],
                                 constant LensUniforms& lens [[buffer(2)]],
                                 depth2d<float> depth [[texture(2)]],
                                 texture2d<float> lens_picture [[texture(3)]]) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float3 color = hdr.sample(linear_sampler, in.uv).rgb;
    float is_kept_plain = lens.params.y;
    float3 bloom_color = bloom.sample(linear_sampler, in.uv).rgb / frame.effect_params.w;
    color = get_graded_color(frame, mix(color, bloom_color, frame.render_params.y));
    float2 centered = in.uv - 0.5;
    float vignette = 1.0 - VIGNETTE_STRENGTH * (1.0 - is_kept_plain) * smoothstep(0.2, 0.85, length(centered * float2(1.1, 1.0)));
    color *= vignette;
    float3 encoded = linear_to_srgb(saturate(color));
    float grain = interleaved_gradient_noise(in.position.xy + fract(frame.camera_position_m.w) * 97.0) - 0.5;
    encoded = saturate(encoded + grain * GRAIN_STRENGTH * (1.0 - is_kept_plain));
    float4 lensed = float4(0.0);
    float entry_m, closest_m;
    float3 dir;
    if (lens.params.x > 0.5 && find_lens_mouth(frame, lens, in.uv, entry_m, closest_m, dir) >= 0) {
        constexpr sampler point_sampler(coord::normalized, filter::nearest, address::clamp_to_edge);
        float scene_depth = depth.sample(point_sampler, in.uv);
        float scene_m = 1e30;
        if (scene_depth > 0.0) {
            float2 ndc = float2(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
            float4 p = frame.inv_view_proj * float4(ndc, scene_depth, 1.0);
            scene_m = length(p.xyz / p.w - frame.camera_position_m.xyz);
        }
        if (entry_m < scene_m) {
            lensed = sample_spline(lens_picture, in.uv);
        }
    }
    encoded = mix(encoded, lensed.rgb, lensed.a);
    float luma = dot(encoded, float3(0.299, 0.587, 0.114));
    return float4(encoded, luma);
}

/**
 * @brief `HitEffect` matches `render::overlay::HitEffect`: how much of a recent hit's effect is left
 * (1 just after it, 0 for none), how red the edges turn, how wide their blur is and how far the
 * picture shifts this frame, in window pixels.
 */
struct HitEffect {
    float strength_frac;
    float red_frac;
    float blur_px;
    packed_float2 shake_px;
};

/**
 * @brief `HIT_RED_COLOR`, `HIT_EDGE_REACH_FRAC` and `HIT_BLUR_TAP_COUNT` are the Lunar Surface demo's
 * (`demos/hyperbolic_wormholes_rust/src/render_metal.metal`): the colour the edges turn, how far in
 * a fresh hit reaches, and the blur's taps.
 */
constant float3 HIT_RED_COLOR = float3(0.85, 0.05, 0.03);
constant float HIT_EDGE_REACH_FRAC = 0.7;
constant int HIT_BLUR_TAP_COUNT = 8;

static float3 get_fxaa_color(texture2d<float> ldr, float2 uv, float is_enabled);

/**
 * @brief `fxaa_fragment` shows the frame in the window as a player who was just hit sees it, as the
 * Lunar Surface demo's `read_hit_scene` does: shifted by `hit.shake_px`, and toward the edges
 * blurred over `hit.blur_px` and turned red by `hit.red_frac`, reaching further in the fresher the
 * hit. Each pixel is FXAA 3.11 of the internal-resolution frame (`get_fxaa_color`).
 * @param is_enabled 0 copies the image without anti-aliasing.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, fxaa_fragment) into the drawable
 * @endcode
 */
fragment float4 fxaa_fragment(FullscreenVertexOut in [[stage_in]],
                              constant float& is_enabled [[buffer(0)]],
                              constant HitEffect& hit [[buffer(1)]],
                              constant float2& window_px [[buffer(2)]],
                              texture2d<float> ldr [[texture(0)]]) {
    if (hit.strength_frac <= 0.0) {
        return float4(get_fxaa_color(ldr, in.uv, is_enabled), 1.0);
    }
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 at_px = clamp(in.position.xy + round(float2(hit.shake_px)), float2(0.5), window_px - 0.5);
    float3 sharp = get_fxaa_color(ldr, at_px / window_px, is_enabled);
    float corner = length(in.position.xy / window_px - 0.5) * M_SQRT2_F;
    float edge = hit.strength_frac * smoothstep(1.0 - HIT_EDGE_REACH_FRAC * hit.strength_frac, 1.0, corner);
    float3 sum = sharp;
    for (int k = 0; k < HIT_BLUR_TAP_COUNT; k++) {
        float angle = 2.0 * M_PI_F * float(k) / float(HIT_BLUR_TAP_COUNT);
        float2 tap_px = at_px + round(hit.blur_px * edge * float2(cos(angle), sin(angle)));
        sum += ldr.sample(linear_sampler, clamp(tap_px, float2(0.5), window_px - 0.5) / window_px).rgb;
    }
    float3 blurred = mix(sharp, sum / float(HIT_BLUR_TAP_COUNT + 1), edge);
    return float4(mix(blurred, HIT_RED_COLOR, edge * hit.red_frac), 1.0);
}

/**
 * @brief `get_fxaa_color` is FXAA 3.11 (quality) of `ldr` at `uv`, reading luma from alpha; it also
 * scales the internal resolution up to the window. `is_enabled` 0 copies the pixel as it is.
 * @par Example
 * @code
 * float3 color = get_fxaa_color(ldr, in.uv, is_enabled);
 * @endcode
 */
static float3 get_fxaa_color(texture2d<float> ldr, float2 uv, float is_enabled) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float2 texel = 1.0 / float2(ldr.get_width(), ldr.get_height());
    float4 center = ldr.sample(linear_sampler, uv);
    if (is_enabled < 0.5) {
        return center.rgb;
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
        return center.rgb;
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
    return ldr.sample(linear_sampler, final_uv).rgb;
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

/** @brief `GpuBeam` matches `gpu_types::GpuBeam`: (start, radius), (end, speckle share), (colour, 1 for the rifle's), (goes on before the start, goes on past the end, -, -). */
struct GpuBeam {
    float4 start_radius;
    float4 end_speckle;
    float4 color_kind;
    float4 joints;
};

/** @brief `BeamPass` matches `gpu_types::GpuBeamPass`: (radians per pixel, black share, white share, frame seed). */
struct BeamPass {
    float4 params;
};

/**
 * @brief Lunar Surface's beam look (`render_voxel.metal`): a beam is at least a pixel wide; its haze reaches
 * `BEAM_REACH` widths; the sniper's blade and haze and the rifle's halo opacity and core whiteness; speckle only
 * where the glow passes `MIN_SPECKLE_GLOW`. `BEAM_QUAD_REACH` is how many widths around a beam its quad covers,
 * `BEAM_NEAR_M` where a beam behind the camera is cut, and `BEAM_HDR_PER_DISPLAY` scales Lunar's display
 * colours into this demo's HDR, so its tone map shows them about as bright as Lunar does.
 */
constant float BEAM_REACH = 4.8;
constant float BEAM_HAZE_BRIGHTNESS = 0.45;
constant float BEAM_HAZE_WHITENESS = 0.45;
constant float BEAM_CORE_BRIGHTNESS = 1.6;
constant float BEAM_CORE_WHITENESS = 0.9;
constant float LASER_HALO_OPACITY = 0.9;
constant float LASER_CORE_WHITENESS = 0.6;
constant float MIN_SPECKLE_GLOW = 0.02;
constant float BEAM_QUAD_REACH = 8.0;
constant float BEAM_NEAR_M = 0.05;
constant float BEAM_HDR_PER_DISPLAY = 1.0;
/** @brief `hash_pixel`'s lowbias32 constants and the speckle kind's salt, Lunar Surface's. */
constant uint SPECKLE_KIND_SALT = 0x9e3779b9u;
constant uint3 PIXEL_HASH_PRIMES = uint3(73856093u, 19349663u, 2654435761u);
constant uint2 PIXEL_HASH_MULTIPLIERS = uint2(0x7feb352du, 0x846ca68bu);
constant uint3 PIXEL_HASH_SHIFTS = uint3(16u, 15u, 16u);
constant uint PIXEL_HASH_DROPPED_BITS = 8u;
constant float PIXEL_HASH_SCALE = 1.0 / 16777216.0;

/** @brief `BeamVertexOut` is a beam quad's corner and which beam it belongs to. */
struct BeamVertexOut {
    float4 position [[position]];
    uint beam [[flat]];
};

/**
 * @brief `beam_vertex` makes the screen quad around beam `instance_id`: the segment cut where it goes behind the
 * camera, projected, and widened by `BEAM_QUAD_REACH` times its width in pixels (a pixel, or the beam's radius if
 * wider) on every side, so it covers the haze whichever way the beam points.
 * @par Example
 * @code
 * pipeline(beam_vertex, beam_fragment), a triangle-strip quad per beam
 * @endcode
 */
vertex BeamVertexOut beam_vertex(uint vertex_id [[vertex_id]], uint instance_id [[instance_id]],
                                 constant GpuBeam* beams [[buffer(0)]],
                                 constant FrameUniforms& frame [[buffer(1)]],
                                 constant BeamPass& pass [[buffer(2)]]) {
    GpuBeam beam = beams[instance_id];
    float3 a = beam.start_radius.xyz, b = beam.end_speckle.xyz;
    float za = -(frame.view * float4(a, 1.0)).z, zb = -(frame.view * float4(b, 1.0)).z;
    BeamVertexOut out;
    out.beam = instance_id;
    if (za < BEAM_NEAR_M && zb < BEAM_NEAR_M) {
        out.position = float4(2.0, 2.0, 0.0, 1.0);
        return out;
    }
    if (za < BEAM_NEAR_M) {
        a = mix(a, b, (BEAM_NEAR_M - za) / (zb - za));
        za = BEAM_NEAR_M;
    } else if (zb < BEAM_NEAR_M) {
        b = mix(b, a, (BEAM_NEAR_M - zb) / (za - zb));
        zb = BEAM_NEAR_M;
    }
    float4 ca = frame.view_proj * float4(a, 1.0), cb = frame.view_proj * float4(b, 1.0);
    float2 half_size_px = 0.5 * frame.screen_size.xy;
    float2 pa = ca.xy / ca.w * half_size_px, pb = cb.xy / cb.w * half_size_px;
    float pixel_rad = pass.params.x, radius_m = beam.start_radius.w;
    float reach_px = BEAM_QUAD_REACH * max(max(radius_m / (pixel_rad * za), radius_m / (pixel_rad * zb)), 1.0);
    float2 along = pb - pa;
    float length_px = length(along);
    float2 axis = length_px > 1e-3 ? along / length_px : float2(1.0, 0.0);
    float2 side = float2(-axis.y, axis.x);
    bool is_end = (vertex_id & 2u) != 0;
    float2 corner = (is_end ? pb + axis * reach_px : pa - axis * reach_px) + side * ((vertex_id & 1u) != 0 ? reach_px : -reach_px);
    out.position = float4(corner / half_size_px, 0.0, 1.0);
    return out;
}

/**
 * @brief `hash_pixel` gives a pseudo-random number in [0, 1) for pixel `id` and `seed`, independent from pixel to
 * pixel (a lowbias32 integer hash), as Lunar Surface's.
 * @par Example
 * @code
 * float roll = hash_pixel(uint2(in.position.xy), seed);
 * @endcode
 */
static float hash_pixel(uint2 id, uint seed) {
    uint h = id.x * PIXEL_HASH_PRIMES.x ^ id.y * PIXEL_HASH_PRIMES.y ^ seed * PIXEL_HASH_PRIMES.z;
    h ^= h >> PIXEL_HASH_SHIFTS.x;
    h *= PIXEL_HASH_MULTIPLIERS.x;
    h ^= h >> PIXEL_HASH_SHIFTS.y;
    h *= PIXEL_HASH_MULTIPLIERS.y;
    h ^= h >> PIXEL_HASH_SHIFTS.z;
    return float(h >> PIXEL_HASH_DROPPED_BITS) * PIXEL_HASH_SCALE;
}

/**
 * @brief `beam_fragment` draws one beam over the lit scene as Lunar Surface's `scan_beams` and `shade_ray_end` do:
 * the pixel's ray meets the beam closest before it hits the scene (the depth prepass), at a gap measured in the
 * beam's width (its radius, or a pixel far away); the sniper's is a flat-topped white blade in a soft haze, the
 * rifle's a Gaussian laser over its halo. A share of the pixels its glow reaches speckles each frame, as
 * `speckle_beam`: black, white, or dropping out (the scene without the beam). It returns premultiplied colour,
 * with alpha how much of the scene it covers.
 * @par Example
 * @code
 * pipeline(beam_vertex, beam_fragment) with premultiplied blending into the HDR target, after the lit pass
 * @endcode
 */
fragment float4 beam_fragment(BeamVertexOut in [[stage_in]], constant GpuBeam* beams [[buffer(0)]],
                              constant FrameUniforms& frame [[buffer(1)]],
                              constant BeamPass& pass [[buffer(2)]],
                              depth2d<float> depth [[texture(0)]]) {
    GpuBeam beam = beams[in.beam];
    float2 uv = in.position.xy * frame.screen_size.zw;
    float3 eye = frame.camera_position_m.xyz;
    float4 near_point = frame.inv_view_proj * float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 1.0, 1.0);
    float3 v = normalize(near_point.xyz / near_point.w - eye);
    float scene_depth = depth.read(uint2(in.position.xy));
    float ray_end_m = 1e30;
    if (scene_depth > 0.0) {
        float4 p = frame.inv_view_proj * float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, scene_depth, 1.0);
        ray_end_m = length(p.xyz / p.w - eye);
    }
    float3 a = beam.start_radius.xyz, ab = beam.end_speckle.xyz - a, r = eye - a;
    float ab_sq = max(dot(ab, ab), 1e-8), v_ab = dot(v, ab), v_r = dot(v, r), ab_r = dot(ab, r);
    float denominator = ab_sq - v_ab * v_ab;
    float t = denominator > 1e-8 ? (v_ab * ab_r - ab_sq * v_r) / denominator : -v_r;
    t = clamp(t, 0.0, ray_end_m);
    float s = clamp(dot(eye + v * t - a, ab) / ab_sq, 0.0, 1.0);
    t = clamp(dot(a + ab * s - eye, v), 0.0, ray_end_m);
    s = clamp(dot(eye + v * t - a, ab) / ab_sq, 0.0, 1.0);
    if (s >= 1.0 && beam.joints.y > 0.5) {
        discard_fragment();
    }
    float gap_m = distance(eye + v * t, a + ab * s);
    float width_m = max(beam.start_radius.w, pass.params.x * t);
    float q = gap_m * gap_m / (width_m * width_m);
    bool is_rifle = beam.color_kind.w > 0.5;
    float core = is_rifle ? exp(-q) : exp(-q * q * q);
    float halo = exp(-q / BEAM_REACH);
    float3 color = beam.color_kind.rgb;
    float3 light;
    float cover, glow;
    if (is_rifle) {
        cover = saturate(LASER_HALO_OPACITY * halo);
        light = color * cover + mix(color, float3(1.0), LASER_CORE_WHITENESS) * core;
        glow = saturate(LASER_HALO_OPACITY * halo + core);
    } else {
        cover = saturate(core);
        float3 haze = mix(color, float3(1.0), BEAM_HAZE_WHITENESS) * BEAM_HAZE_BRIGHTNESS * halo;
        light = haze * (1.0 - cover) + mix(color, float3(1.0), BEAM_CORE_WHITENESS) * BEAM_CORE_BRIGHTNESS * cover;
        glow = saturate(core + BEAM_HAZE_BRIGHTNESS * halo);
    }
    uint seed = uint(pass.params.w);
    uint2 id = uint2(in.position.xy);
    float speckle = beam.end_speckle.w;
    if (speckle > 0.0 && glow > MIN_SPECKLE_GLOW && hash_pixel(id, seed) < speckle) {
        float kind = hash_pixel(id, seed ^ SPECKLE_KIND_SALT);
        if (kind < pass.params.y) {
            light *= 1.0 - glow;
            cover = 1.0 - (1.0 - cover) * (1.0 - glow);
        } else if (kind < pass.params.y + pass.params.z) {
            light = light * (1.0 - glow) + glow;
            cover = 1.0 - (1.0 - cover) * (1.0 - glow);
        } else {
            light = float3(0.0);
            cover = 0.0;
        }
    }
    float hdr_scale = BEAM_HDR_PER_DISPLAY / max(frame.render_params.x, 1e-6);
    return float4(light * hdr_scale, cover);
}
