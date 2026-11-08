/**
 * @file
 * @brief `overlay.metal` draws the HUD's overlay rectangles (`render/overlay.rs`) over the finished
 * frame: the bars with their slant, gloss, outline, shadow, glow and pulses, and the crosshair.
 * Everything down to `get_rect_layer` is copied from the Lunar Surface demo's `render_metal.metal`
 * (`demos/hyperbolic_wormholes_rust/src/render_metal.metal`), so the HUD looks the same in every
 * level; `overlay_rects_fragment` blends them in the same order as its `apply_overlay`.
 *
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, overlay_rects_fragment) with premultiplied blending into the drawable
 * @endcode
 */

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
 * @brief `overlay_rects_fragment` blends `rect_count` overlay rectangles over the drawable in
 * `apply_overlay`'s order: first all their shadows and glows (`get_rect_halo`, the strongest of
 * each), then the rectangles themselves in order (`get_rect_layer`), then the bars' pulses
 * (`get_pulse_glow`, tinting toward their colour and adding `RECT_PULSE_LIGHT_GAIN` of it as light).
 * Each step maps the colour below to `color * keep + add`, so the fragment returns `add` and
 * `1 - keep` for premultiplied blending.
 * @par Example
 * @code
 * pipeline(fullscreen_vertex, overlay_rects_fragment) with premultiplied blending into the drawable
 * @endcode
 */
fragment float4 overlay_rects_fragment(FullscreenVertexOut in [[stage_in]],
                                       constant OverlayRect* rects [[buffer(0)]],
                                       constant uint& rect_count [[buffer(1)]],
                                       constant float2& size_px [[buffer(2)]]) {
    float2 at_px = in.position.xy;
    float shadow_a = 0.0;
    float3 glow = float3(0.0);
    for (uint i = 0; i < rect_count; i++) {
        float4 halo = get_rect_halo(rects[i], at_px, size_px);
        shadow_a = max(shadow_a, halo.a);
        glow = max(glow, halo.rgb);
    }
    float keep = 1.0 - shadow_a;
    float3 add = glow;
    for (uint i = 0; i < rect_count; i++) {
        float4 layer = get_rect_layer(rects[i], at_px, size_px);
        keep *= 1.0 - layer.a;
        add = add * (1.0 - layer.a) + layer.rgb;
    }
    for (uint i = 0; i < rect_count; i++) {
        float pulse_glow = get_pulse_glow(rects[i], at_px, size_px);
        float3 pulse_rgb = rects[i].pulse.color.rgb;
        keep *= 1.0 - pulse_glow;
        add = add * (1.0 - pulse_glow) + pulse_rgb * pulse_glow * (1.0 + RECT_PULSE_LIGHT_GAIN);
    }
    return float4(add, 1.0 - keep);
}
