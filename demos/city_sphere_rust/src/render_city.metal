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
constant float RIB_HALF_WIDTH_M = 0.07;
constant float RIB_ALPHA = 0.85;
constant float3 RIB_COLOR = float3(0.16, 0.17, 0.2);
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
constant float METAL_SATURATION = 0.3;
constant float METAL_SPEC_POWER = 48.0;
constant float STREET_LAYER = 0.0;
constant float PANEL_LINE_HALF_WIDTH_M = 0.03;
constant float3 PANEL_LINE_COLOR = float3(0.05, 0.14, 0.18);
constant uint BEACON_SLOT_COUNT = 8;
constant float BEACON_GLOW_BRIGHTNESS = 2.0;
constant float BEACON_BODY_GLOW = 0.72;

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

// `apply_fog` mixes in the sky colour with distance.
static float3 apply_fog(float3 c, float3 world, constant CityUniforms& u) {
    float d = length(u.camera_m.xyz - world);
    return mix(u.sky_color.rgb, c, exp(-d / u.glass.w));
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
            float3 cell = floor(in.world * WEAR_CELLS_PER_M);
            float wear = fract(sin(dot(cell, float3(12.9898, 78.233, 37.719))) * 43758.5453);
            base *= 1.0 - WEAR_FRAC * wear * wear;
        }
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
    float spec = pow(max(dot(n, h), 0.0), METAL_SPEC_POWER) * METAL_SPEC;
    float3 light = u.ambient.rgb * wrap + u.core_light.rgb * (lambert * falloff) + spec * u.core_light.rgb;
    float3 lit = base * light;
    if (!is_beacon && in.layer >= 0.0) {
        float3 mirror = reflect(-view, n);
        float core_frac = dot(mirror, l);
        float3 env = mix(ENV_FLOOR_COLOR, u.core_light.rgb * ENV_CORE_GAIN, smoothstep(-0.3, 1.0, core_frac))
            + u.core_light.rgb * pow(max(core_frac, 0.0), ENV_HIGHLIGHT_POWER) * ENV_HIGHLIGHT_GAIN;
        float fresnel = METAL_REFLECT_MIN + (1.0 - METAL_REFLECT_MIN) * pow(1.0 - max(dot(n, view), 0.0), 5.0);
        lit += env * mix(base, float3(1.0), 0.3) * fresnel * METAL_REFLECT;
    }
    if (abs(in.layer - STREET_LAYER) < 0.5) {
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
    float rib_light = 0.3 + 0.4 * pow(max(dot(reflect(-view, n), l), 0.0), 8.0);
    float3 tint = mix(in.color.rgb * GLASS_TINT_FRAC, RIB_COLOR * rib_light, line) + float3(0.8, 0.6, 1.0) * rim;
    return float4(encode_srgb(tint) * alpha, alpha);
}

fragment float4 beam_fragment(Fragment in [[stage_in]]) {
    float across = 1.0 - abs(in.uv.x * 2.0 - 1.0);
    float glow = pow(across, 1.5) * in.color.a;
    float3 core = mix(in.color.rgb, float3(1.0), pow(across, 6.0));
    return float4(encode_srgb(core) * glow, 0.0);
}
