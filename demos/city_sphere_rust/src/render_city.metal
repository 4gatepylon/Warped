// `render_city.metal` rasterizes City Sphere: lit, textured streets and buildings, glowing lights,
// the see-through glass shell (with its lift holes cut out) and additive laser beams.
#include <metal_stdlib>
using namespace metal;

constant uint MAX_HOLE_COUNT = 8;
constant float BEACON_FLAG = 2.0;
constant float METAL_SPEC = 0.45;
constant float METAL_SATURATION = 0.3;
constant float METAL_SPEC_POWER = 48.0;
constant float STREET_LAYER = 0.0;
constant float PANEL_LINE_WIDTH = 0.012;
constant float3 PANEL_LINE_COLOR = float3(0.25, 0.7, 0.9);
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
    }
    float4 beacon = is_beacon ? u.beacons[min(uint(in.color.a - BEACON_FLAG + 0.5), BEACON_SLOT_COUNT - 1)] : float4(0.0);
    base *= is_beacon ? beacon.y : 1.0;
    if (!is_beacon && in.color.a > 0.5) {
        return float4(encode_srgb(apply_fog(base * 1.6, in.world, u)), 1.0);
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
    if (abs(in.layer - STREET_LAYER) < 0.5) {
        float2 cell = fract(in.uv);
        float2 edge = min(cell, 1.0 - cell);
        lit += PANEL_LINE_COLOR * (1.0 - smoothstep(0.0, PANEL_LINE_WIDTH, min(edge.x, edge.y)));
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
    float fresnel = 0.05 + 0.55 * pow(1.0 - facing, 4.0);
    float2 cell = fract(in.uv);
    float2 edge = min(cell, 1.0 - cell);
    float line = 1.0 - smoothstep(0.0, 0.025, min(edge.x, edge.y));
    float rim = 0.0;
    for (uint i = 0; i < hole_count && i < MAX_HOLE_COUNT; i++) {
        float c = dot(d, u.holes[i].xyz);
        rim = max(rim, smoothstep(u.holes[i].w - 0.004, u.holes[i].w, c));
    }
    float alpha = clamp(0.07 + fresnel + 0.35 * line + 0.6 * rim, 0.0, 0.9);
    float3 tint = in.color.rgb * (0.6 + 0.4 * line) + float3(0.8, 0.6, 1.0) * rim;
    return float4(encode_srgb(tint) * alpha, alpha);
}

fragment float4 beam_fragment(Fragment in [[stage_in]]) {
    float across = 1.0 - abs(in.uv.x * 2.0 - 1.0);
    float glow = pow(across, 1.5) * in.color.a;
    float3 core = mix(in.color.rgb, float3(1.0), pow(across, 6.0));
    return float4(encode_srgb(core) * glow, 0.0);
}
