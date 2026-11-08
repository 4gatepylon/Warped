// `render_city.metal` rasterizes City Sphere: lit, textured streets and buildings, glowing lights,
// the see-through glass shell (with its lift holes cut out) and additive laser beams.
#include <metal_stdlib>
using namespace metal;

constant uint MAX_HOLE_COUNT = 8;
constant float BEACON_FLAG = 2.0;
constant float METAL_SPEC = 0.45;
constant float METAL_REFLECT = 0.6;
constant float WEAR_CELLS_PER_M = 0.5;
constant float WEAR_FRAC = 0.3;
constant float METAL_REFLECT_MIN = 0.25;
constant float3 ENV_FLOOR_COLOR = float3(0.05, 0.06, 0.09);
constant float ENV_CORE_GAIN = 0.35;
constant float ENV_HIGHLIGHT_POWER = 48.0;
constant float ENV_HIGHLIGHT_GAIN = 1.5;
constant float RIB_HALF_WIDTH_M = 0.05;
constant float RIB_ALPHA = 0.9;
constant float3 RIB_COLOR = float3(1.0, 1.0, 1.0);
constant float RIB_GLOW = 1.4;
constant float GLASS_BASE_ALPHA = 0.035;
constant float GLASS_FRESNEL_GAIN = 0.35;
constant float GLASS_TINT_FRAC = 0.5;
constant float SCREEN_LAYER = -2.0;
constant float LED_LAYER = -3.0;
constant float GLOW_PHASE_SPAN = 0.45;
constant float GLOW_GAIN = 1.6;
constant float2 SCREEN_CELLS = float2(8.0, 6.0);
constant float SCREEN_LINE_COUNT = 24.0;
constant float SCREEN_REFRESH_HZ = 1.5;
constant float LED_BLINK_HZ = 1.2;
constant float LED_DUTY_FRAC = 0.6;
constant float LED_OFF_FRAC = 0.12;
constant float CORE_HALO_POWER = 2.0;
constant float METAL_SATURATION = 0.3;
constant float STREET_LAYER = 0.0;
constant float PANEL_LINE_HALF_WIDTH_M = 0.03;
constant float3 PANEL_LINE_COLOR = float3(0.02, 0.055, 0.07);
constant uint BEACON_SLOT_COUNT = 8;
constant float BEACON_GLOW_BRIGHTNESS = 2.0;
constant float BEACON_BODY_GLOW = 0.72;
constant float ROUGH_MIN = 0.1;
constant float ROUGH_MAX = 0.85;
constant float SMOOTH_SPEC_POWER = 96.0;
constant float ROUGH_SPEC_POWER = 10.0;
constant float FRAME_WIDTH_M = 0.16;
constant float FRAME_GROOVE_M = 0.025;
constant float FRAME_BRIGHT = 1.6;
constant float FRAME_GROOVE_DARK = 0.6;
constant float FRAME_ROUGH = 0.2;
constant float WALL_PANEL_M = 3.0;
constant float GRIME_HEIGHT_M = 1.2;
constant float GRIME_FRAC = 0.45;
constant float GRIME_MOTTLE_FRAC = 0.18;
constant float HAZARD_GAP_M = 0.25;
constant float HAZARD_WIDTH_M = 0.7;
constant float HAZARD_STRIPE_M = 0.45;
constant float3 HAZARD_YELLOW = float3(0.95, 0.62, 0.06);
constant float3 HAZARD_DARK = float3(0.05, 0.05, 0.06);

struct CityVertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float2 uv;
    packed_float4 color;
    float layer;
};

struct CityUniforms {
    float4x4 clip_from_world;
    float4 camera_m;
    float4 core_light;
    float4 ambient;
    float4 holes[MAX_HOLE_COUNT];
    float4 glass;
    float4 sky_color;
    float4 beacons[BEACON_SLOT_COUNT];
};

struct Fragment {
    float4 position [[position]];
    float3 world;
    float3 normal;
    float2 uv;
    float4 color;
    float layer;
};

vertex Fragment city_vertex(uint id [[vertex_id]], const device CityVertex* vertices [[buffer(0)]],
                            constant CityUniforms& u [[buffer(1)]]) {
    CityVertex v = vertices[id];
    Fragment out;
    out.world = float3(v.position);
    out.position = u.clip_from_world * float4(out.world, 1.0);
    out.normal = float3(v.normal);
    out.uv = float2(v.uv);
    out.color = float4(v.color);
    out.layer = v.layer;
    return out;
}

// `encode_srgb` turns linear colour into the render texture's sRGB-encoded values.
static float3 encode_srgb(float3 c) {
    return pow(clamp(c, 0.0, 1.0), float3(1.0 / 2.2));
}

// `hash13` gives a pseudo-random number in [0, 1) for a point.
static float hash13(float3 p) {
    return fract(sin(dot(p, float3(12.9898, 78.233, 37.719))) * 43758.5453);
}

// `apply_fog` mixes in the sky colour with distance.
static float3 apply_fog(float3 c, float3 world, constant CityUniforms& u) {
    float d = length(u.camera_m.xyz - world);
    return mix(u.sky_color.rgb, c, exp(-d / u.glass.w));
}

// `apply_hazard` paints diagonal yellow and black stripes on the street in a ring around the foot
// of every lift (under each hole in the glass), a little outside the lift.
static float3 apply_hazard(float3 base, float3 world, constant CityUniforms& u) {
    float3 d = normalize(world);
    float street_r = u.ambient.w;
    for (uint i = 0; i < uint(u.glass.y) && i < MAX_HOLE_COUNT; i++) {
        float3 axis = u.holes[i].xyz;
        float c = dot(d, axis);
        float lift_m = u.glass.x * sqrt(max(1.0 - u.holes[i].w * u.holes[i].w, 0.0));
        float rho_m = street_r * sqrt(max(1.0 - c * c, 0.0));
        float ring_m = rho_m - lift_m - HAZARD_GAP_M;
        if (c <= 0.0 || ring_m < -0.1 || ring_m > HAZARD_WIDTH_M + 0.1) {
            continue;
        }
        float3 t1 = normalize(cross(axis, abs(axis.y) < 0.9 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0)));
        float3 t2 = cross(axis, t1);
        float angle = atan2(dot(d, t2), dot(d, t1));
        float stripe_count = round(6.2832 * (lift_m + HAZARD_GAP_M) / (2.0 * HAZARD_STRIPE_M));
        float stripe = step(0.5, fract(angle / 6.2832 * stripe_count + rho_m / (2.0 * HAZARD_STRIPE_M)));
        float aa_m = fwidth(ring_m);
        float inside = smoothstep(-aa_m, aa_m, ring_m) * (1.0 - smoothstep(HAZARD_WIDTH_M - aa_m, HAZARD_WIDTH_M + aa_m, ring_m));
        return mix(base, mix(HAZARD_DARK, HAZARD_YELLOW, stripe), inside);
    }
    return base;
}

fragment float4 city_fragment(Fragment in [[stage_in]], constant CityUniforms& u [[buffer(1)]],
                              texture2d_array<float> albedo [[texture(0)]]) {
    constexpr sampler s(address::repeat, filter::linear, mip_filter::linear, max_anisotropy(8));
    float3 base = in.color.rgb;
    bool is_beacon = in.color.a > BEACON_FLAG - 0.5;
    if (in.layer >= 0.0) {
        float3 texel = albedo.sample(s, in.uv, uint(in.layer)).rgb;
        base *= is_beacon ? texel : mix(float3(dot(texel, float3(0.3, 0.59, 0.11))), texel, METAL_SATURATION);
        if (!is_beacon) {
            float wear = hash13(floor(in.world * WEAR_CELLS_PER_M));
            base *= 1.0 - WEAR_FRAC * wear * wear;
        }
    }
    bool is_street = abs(in.layer - STREET_LAYER) < 0.5;
    float rough = 0.5;
    if (!is_beacon && in.layer >= 0.0) {
        float2 panel = is_street ? float2(0.0) : floor(in.uv);
        rough = mix(ROUGH_MIN, ROUGH_MAX, hash13(float3(panel, dot(in.color.rgb, float3(97.0, 57.0, 31.0)))));
        if (!is_street) {
            float2 to_edge = min(fract(in.uv), 1.0 - fract(in.uv));
            float edge_m = min(to_edge.x, to_edge.y) * WALL_PANEL_M;
            float edge_aa_m = fwidth(edge_m);
            float frame = 1.0 - smoothstep(FRAME_WIDTH_M - edge_aa_m, FRAME_WIDTH_M + edge_aa_m, edge_m);
            float groove = 1.0 - smoothstep(FRAME_GROOVE_M - edge_aa_m, FRAME_GROOVE_M + edge_aa_m, edge_m);
            base = mix(base, base * FRAME_BRIGHT, frame) * (1.0 - FRAME_GROOVE_DARK * groove);
            rough = mix(rough, FRAME_ROUGH, frame);
        }
        if (is_street) {
            base = apply_hazard(base, in.world, u);
        }
        float above_m = u.ambient.w - length(in.world);
        float mottle = 0.5 + 0.25 * sin(dot(in.world, float3(0.83, 1.31, 0.57))) * sin(dot(in.world, float3(-1.13, 0.37, 1.49)))
            + 0.25 * sin(dot(in.world, float3(2.9, -2.3, 1.7)) + 1.3);
        float grime = GRIME_MOTTLE_FRAC * mottle
            + (is_street ? 0.0 : GRIME_FRAC * (1.0 - smoothstep(0.0, GRIME_HEIGHT_M, above_m)) * (0.6 + 0.4 * mottle));
        base *= 1.0 - grime;
        rough = mix(rough, ROUGH_MAX, grime);
    }
    float4 beacon = is_beacon ? u.beacons[min(uint(in.color.a - BEACON_FLAG + 0.5), BEACON_SLOT_COUNT - 1)] : float4(0.0);
    base *= is_beacon ? beacon.y : 1.0;
    if (!is_beacon && in.color.a > 0.5) {
        float glow = GLOW_GAIN;
        float phase = (in.color.a - 1.0) / GLOW_PHASE_SPAN;
        float t = u.glass.z;
        if (abs(in.layer - SCREEN_LAYER) < 0.5) {
            float2 cell = floor(in.uv * SCREEN_CELLS) + floor(t * SCREEN_REFRESH_HZ + phase * 7.0);
            float text = step(0.45, fract(sin(dot(cell, float2(12.9898, 78.233))) * 43758.5453));
            float scan = 0.8 + 0.2 * sin((in.uv.y * SCREEN_LINE_COUNT - t * 2.0) * 6.2832);
            float flicker = 0.92 + 0.08 * sin(t * 37.0 + phase * 60.0) * sin(t * 13.0 + phase * 20.0);
            glow *= (0.35 + 0.65 * text) * scan * flicker;
        } else if (abs(in.layer - LED_LAYER) < 0.5) {
            float on = step(fract(t * LED_BLINK_HZ * (0.6 + phase) + phase), LED_DUTY_FRAC);
            glow *= LED_OFF_FRAC + (1.0 - LED_OFF_FRAC) * on;
        }
        return float4(encode_srgb(apply_fog(base * glow, in.world, u)), 1.0);
    }
    float3 n = normalize(in.normal);
    float3 to_core = -in.world;
    float r = max(length(to_core), 1.0);
    float3 l = to_core / r;
    float lambert = max(dot(n, l), 0.0);
    float wrap = 0.5 + 0.5 * dot(n, l);
    float falloff = u.core_light.w * 300.0 / (r * r + 150.0);
    float3 view = normalize(u.camera_m.xyz - in.world);
    float3 h = normalize(view + l);
    float spec = pow(max(dot(n, h), 0.0), mix(SMOOTH_SPEC_POWER, ROUGH_SPEC_POWER, rough)) * METAL_SPEC * (1.5 - rough);
    float3 light = u.ambient.rgb * wrap + u.core_light.rgb * (lambert * falloff) + spec * u.core_light.rgb;
    float3 lit = base * light;
    if (!is_beacon && in.layer >= 0.0) {
        float3 mirror = reflect(-view, n);
        float core_frac = dot(mirror, l);
        float3 env = mix(ENV_FLOOR_COLOR, u.core_light.rgb * ENV_CORE_GAIN, smoothstep(-0.3, 1.0, core_frac))
            + u.core_light.rgb * pow(max(core_frac, 0.0), ENV_HIGHLIGHT_POWER * (1.0 - 0.8 * rough)) * ENV_HIGHLIGHT_GAIN * (1.0 - 0.6 * rough);
        float fresnel = METAL_REFLECT_MIN + (1.0 - METAL_REFLECT_MIN) * pow(1.0 - max(dot(n, view), 0.0), 5.0);
        lit += env * mix(base, float3(1.0), 0.3) * fresnel * METAL_REFLECT * (1.3 - rough);
    }
    if (is_street) {
        float edge_m = -in.color.a;
        float edge_aa_m = fwidth(edge_m);
        lit += PANEL_LINE_COLOR * (1.0 - smoothstep(PANEL_LINE_HALF_WIDTH_M - edge_aa_m, PANEL_LINE_HALF_WIDTH_M + edge_aa_m, edge_m));
    }
    if (is_beacon) {
        float3 strips = albedo.sample(s, in.uv, uint(in.layer) + 1).rgb;
        float glow = beacon.x;
        lit += (strips * mix(in.color.rgb, float3(1.0), 0.5) * BEACON_GLOW_BRIGHTNESS + in.color.rgb * BEACON_BODY_GLOW) * glow;
    }
    return float4(encode_srgb(apply_fog(lit, in.world, u)), 1.0);
}

fragment float4 glass_fragment(Fragment in [[stage_in]], constant CityUniforms& u [[buffer(1)]]) {
    float3 d = normalize(in.world);
    uint hole_count = uint(u.glass.y);
    for (uint i = 0; i < hole_count && i < MAX_HOLE_COUNT; i++) {
        if (dot(d, u.holes[i].xyz) >= u.holes[i].w) {
            discard_fragment();
        }
    }
    float3 n = normalize(in.normal);
    float3 view = normalize(u.camera_m.xyz - in.world);
    float facing = abs(dot(n, view));
    float fresnel = 0.03 + GLASS_FRESNEL_GAIN * pow(1.0 - facing, 4.0);
    float edge_aa_m = fwidth(in.uv.x);
    float line = 1.0 - smoothstep(RIB_HALF_WIDTH_M - edge_aa_m, RIB_HALF_WIDTH_M + edge_aa_m, in.uv.x);
    float rim = 0.0;
    for (uint i = 0; i < hole_count && i < MAX_HOLE_COUNT; i++) {
        float c = dot(d, u.holes[i].xyz);
        rim = max(rim, smoothstep(u.holes[i].w - 0.004, u.holes[i].w, c));
    }
    float alpha = clamp(GLASS_BASE_ALPHA + fresnel * (1.0 - line) + RIB_ALPHA * line + 0.6 * rim, 0.0, 0.95);
    float3 l = normalize(-in.world);
    float rib_light = RIB_GLOW + 0.3 * pow(max(dot(reflect(-view, n), l), 0.0), 8.0);
    float3 tint = mix(in.color.rgb * GLASS_TINT_FRAC, RIB_COLOR * rib_light, line) + float3(0.8, 0.6, 1.0) * rim;
    return float4(encode_srgb(tint) * alpha, alpha);
}

fragment float4 beam_fragment(Fragment in [[stage_in]]) {
    if (length(in.normal) > 0.5) {
        float halo = pow(saturate(1.0 - length(in.uv * 2.0 - 1.0)), CORE_HALO_POWER) * in.color.a;
        return float4(encode_srgb(in.color.rgb) * halo, 0.0);
    }
    float across = 1.0 - abs(in.uv.x * 2.0 - 1.0);
    float glow = pow(across, 1.5) * in.color.a;
    float3 core = mix(in.color.rgb, float3(1.0), pow(across, 6.0));
    return float4(encode_srgb(core) * glow, 0.0);
}
