// `render_city.metal` rasterizes City Sphere: lit, textured streets and buildings, glowing lights,
// the see-through glass shell (with its lift and stairs holes cut out) and additive laser beams.
#include <metal_stdlib>
using namespace metal;

constant uint MAX_HOLE_COUNT = 8;
constant float HOLE_RIM_COS = 0.004;
constant float MIN_SPECKLE_GLOW = 0.02;
constant float3 FLASHLIGHT_COLOR = float3(1.0, 0.95, 0.85);
constant float FLASHLIGHT_INTENSITY = 1.45;
constant float FLASHLIGHT_TINY_M = 1e-3;
constant float BEAM_HALF_WIDTH_M = 0.08;
constant float DUST_GRAIN_EDGE = 0.6;
constant float DUST_PUFF_FALLOFF = 3.0;
constant float3 DUST_LIGHT = float3(0.95, 0.85, 1.0);
constant float BEAM_RADIUS_M = 0.02;
constant float BEAM_REACH = 4.8;
constant float RIFLE_BEAM_MARK = 0.25;
constant float BEAM_HAZE_BRIGHTNESS = 0.45;
constant float BEAM_HAZE_WHITENESS = 0.45;
constant float BEAM_CORE_BRIGHTNESS = 1.6;
constant float BEAM_CORE_WHITENESS = 0.9;
constant float LASER_HALO_OPACITY = 0.9;
constant float LASER_CORE_WHITENESS = 0.6;
constant uint SPECKLE_KIND_SALT = 0x9e3779b9u;
constant uint3 PIXEL_HASH_PRIMES = uint3(73856093u, 19349663u, 2654435761u);
constant uint2 PIXEL_HASH_MULTIPLIERS = uint2(0x7feb352du, 0x846ca68bu);
constant uint3 PIXEL_HASH_SHIFTS = uint3(16u, 15u, 16u);
constant uint PIXEL_HASH_DROPPED_BITS = 8u;
constant float PIXEL_HASH_SCALE = 1.0 / 16777216.0;
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
constant float BOLT_INSET_M = 0.08;
constant float STRIP_LOW_M = 0.25;
constant float STRIP_HIGH_M = 0.31;
constant float3 STRIP_COLOR = float3(0.25, 0.85, 1.0);
constant float STRIP_GAIN = 1.6;
constant float STRIP_PULSE_HZ = 0.25;
constant float STRIP_PULSE_FRAC = 0.25;
constant float BOLT_RADIUS_M = 0.03;
constant float BOLT_BRIGHT = 1.8;
constant float BOLT_SHADOW_DARK = 0.55;
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
    float4 hazard;
    float4 beam_speckle;
    float4 flashlight_position;
    float4 flashlight_direction;
    float4 flashlight;
    float4 hole_sides[MAX_HOLE_COUNT];
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
// of every lift (under each of the first `hazard.x` holes in the glass, the lifts'), a little
// outside the lift.
static float3 apply_hazard(float3 base, float3 world, constant CityUniforms& u) {
    float3 d = normalize(world);
    float street_r = u.ambient.w;
    for (uint i = 0; i < uint(u.hazard.x) && i < MAX_HOLE_COUNT; i++) {
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

// `light_flashlight` gives the light of the player's flashlight on a surface at `world` with normal
// `n`, as on Lunar Surface: from `u.flashlight_position` along `u.flashlight_direction`, full inside
// the cone and softening to nothing at its outer edge (`u.flashlight.x` and `.y`, their cosines),
// falling to half at `u.flashlight.z` meters, as much as the surface faces it, times its brightness
// `u.flashlight.w` (0 when off).
static float3 light_flashlight(float3 world, float3 n, constant CityUniforms& u) {
    float4 f = u.flashlight;
    if (f.w <= 0.0) return float3(0.0);
    float3 toward = world - u.flashlight_position.xyz;
    float d_m = max(length(toward), FLASHLIGHT_TINY_M);
    float cone = smoothstep(f.y, f.x, dot(toward / d_m, u.flashlight_direction.xyz));
    float facing = max(dot(n, -toward / d_m), 0.0);
    float falloff = d_m / f.z;
    return FLASHLIGHT_INTENSITY * FLASHLIGHT_COLOR * f.w * cone * facing / (1.0 + falloff * falloff);
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
            float2 in_panel_m = fract(in.uv) * WALL_PANEL_M;
            float bolt_m = length(min(in_panel_m, WALL_PANEL_M - in_panel_m) - BOLT_INSET_M);
            float bolt = 1.0 - smoothstep(BOLT_RADIUS_M - edge_aa_m, BOLT_RADIUS_M + edge_aa_m, bolt_m);
            float bolt_shadow = (1.0 - smoothstep(BOLT_RADIUS_M, BOLT_RADIUS_M * 1.8, bolt_m)) * (1.0 - bolt);
            base = mix(base, base * BOLT_BRIGHT, bolt) * (1.0 - BOLT_SHADOW_DARK * bolt_shadow);
        }
        if (is_street) {
            base = apply_hazard(base, in.world, u);
        }
        float r_m = length(in.world);
        float above_m = (r_m > u.glass.x ? u.ambient.w : u.glass.x) - r_m;
        float mottle = 0.5 + 0.25 * sin(dot(in.world, float3(0.83, 1.31, 0.57))) * sin(dot(in.world, float3(-1.13, 0.37, 1.49)))
            + 0.25 * sin(dot(in.world, float3(2.9, -2.3, 1.7)) + 1.3);
        float grime = GRIME_MOTTLE_FRAC * mottle
            + (is_street ? 0.0 : GRIME_FRAC * (1.0 - smoothstep(0.0, GRIME_HEIGHT_M, above_m)) * (0.6 + 0.4 * mottle));
        base *= 1.0 - grime;
        rough = mix(rough, ROUGH_MAX, grime);
    }
    float strip = 0.0;
    if (!is_beacon && !is_street && in.layer >= 0.0 && abs(dot(normalize(in.normal), normalize(in.world))) < 0.5) {
        float r_m = length(in.world);
        float above_m = (r_m > u.glass.x ? u.ambient.w : u.glass.x) - r_m;
        float aa_m = fwidth(above_m);
        strip = smoothstep(STRIP_LOW_M - aa_m, STRIP_LOW_M + aa_m, above_m) * (1.0 - smoothstep(STRIP_HIGH_M - aa_m, STRIP_HIGH_M + aa_m, above_m));
        strip *= 1.0 - STRIP_PULSE_FRAC + STRIP_PULSE_FRAC * sin(6.2832 * (u.glass.z * STRIP_PULSE_HZ + dot(in.world, float3(0.05))));
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
    float3 light = u.ambient.rgb * wrap + u.core_light.rgb * (lambert * falloff) + spec * u.core_light.rgb
        + light_flashlight(in.world, n, u);
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
    lit += STRIP_COLOR * STRIP_GAIN * strip;
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
        if (dot(d, u.holes[i].xyz) >= u.holes[i].w && abs(dot(d, u.hole_sides[i].xyz)) <= u.hole_sides[i].w) {
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
        float side_rim_width = HOLE_RIM_COS / sqrt(1.0 - u.holes[i].w * u.holes[i].w);
        float side_rim = 1.0 - smoothstep(u.hole_sides[i].w, u.hole_sides[i].w + side_rim_width, abs(dot(d, u.hole_sides[i].xyz)));
        rim = max(rim, min(smoothstep(u.holes[i].w - HOLE_RIM_COS, u.holes[i].w, c), side_rim));
    }
    float alpha = clamp(GLASS_BASE_ALPHA + fresnel * (1.0 - line) + RIB_ALPHA * line + 0.6 * rim, 0.0, 0.95);
    float3 l = normalize(-in.world);
    float rib_light = RIB_GLOW + 0.3 * pow(max(dot(reflect(-view, n), l), 0.0), 8.0);
    float3 tint = mix(in.color.rgb * GLASS_TINT_FRAC, RIB_COLOR * rib_light, line) + float3(0.8, 0.6, 1.0) * rim;
    return float4(encode_srgb(tint) * alpha, alpha);
}

// `hash_pixel` gives a pseudo-random number in [0, 1) for pixel `id` and `seed`, independent from
// pixel to pixel: Lunar Surface's (`render_voxel.metal`), so the speckle looks the same.
static float hash_pixel(uint2 id, uint seed) {
    uint h = id.x * PIXEL_HASH_PRIMES.x ^ id.y * PIXEL_HASH_PRIMES.y ^ seed * PIXEL_HASH_PRIMES.z;
    h ^= h >> PIXEL_HASH_SHIFTS.x;
    h *= PIXEL_HASH_MULTIPLIERS.x;
    h ^= h >> PIXEL_HASH_SHIFTS.y;
    h *= PIXEL_HASH_MULTIPLIERS.y;
    h ^= h >> PIXEL_HASH_SHIFTS.z;
    return float(h >> PIXEL_HASH_DROPPED_BITS) * PIXEL_HASH_SCALE;
}

// `beam_fragment` draws a laser beam (or, with a normal along z, the core's halo; along x, a dust
// particle, `push_dust_particle`) over the scene as premultiplied light, in Lunar Surface's two looks (`render_voxel.metal`'s
// `scan_beams` and `shade_ray_end`), its brightness `in.color.a`, from how far the pixel is from
// the beam's line against `BEAM_RADIUS_M` (or a pixel, if wider): the sniper's a white blade
// (`exp(-q^3)`) in a soft team-coloured haze, and the rifle's (a normal with y `RIFLE_BEAM_MARK`) a
// team-coloured Gaussian laser over a halo that tints the scene. On the share `in.layer` of the
// beam's pixels, picked anew each frame with `u.beam_speckle.x`, a pixel speckles as on Lunar
// Surface: black (the scene darkened by the glow, through alpha), white (white light by the glow)
// in the shares `u.beam_speckle.y` and `.z`, or else a dropout (the scene shows through).
fragment float4 beam_fragment(Fragment in [[stage_in]], constant CityUniforms& u [[buffer(1)]]) {
    if (in.normal.x > 0.5) {
        float radial = length(in.uv * 2.0 - 1.0);
        float shape = in.layer > 0.5 ? 1.0 - smoothstep(DUST_GRAIN_EDGE, 1.0, radial) : exp(-radial * radial * DUST_PUFF_FALLOFF);
        float cover = saturate(in.color.a) * shape * step(radial, 1.0);
        return float4(encode_srgb(in.color.rgb * DUST_LIGHT) * cover, cover);
    }
    if (length(in.normal) > 0.5) {
        float halo = pow(saturate(1.0 - length(in.uv * 2.0 - 1.0)), CORE_HALO_POWER) * in.color.a;
        return float4(encode_srgb(in.color.rgb) * halo, 0.0);
    }
    float d_m = abs(in.uv.x * 2.0 - 1.0) * BEAM_HALF_WIDTH_M;
    float width_m = max(BEAM_RADIUS_M, fwidth(d_m));
    float q = d_m * d_m / (width_m * width_m);
    float brightness = saturate(in.color.a);
    float3 beam = in.color.rgb;
    bool is_rifle = in.normal.y > RIFLE_BEAM_MARK * 0.5;
    float halo = brightness * exp(-q / BEAM_REACH);
    float core = brightness * exp(is_rifle ? -q : -q * q * q);
    float3 light;
    float cover;
    float glow;
    if (is_rifle) {
        light = beam * LASER_HALO_OPACITY * halo + mix(beam, float3(1.0), LASER_CORE_WHITENESS) * core;
        cover = saturate(LASER_HALO_OPACITY * halo);
        glow = saturate(LASER_HALO_OPACITY * halo + core);
    } else {
        float c = saturate(core);
        light = (1.0 - c) * mix(beam, float3(1.0), BEAM_HAZE_WHITENESS) * BEAM_HAZE_BRIGHTNESS * halo
            + c * mix(beam, float3(1.0), BEAM_CORE_WHITENESS) * BEAM_CORE_BRIGHTNESS;
        cover = c;
        glow = saturate(core + BEAM_HAZE_BRIGHTNESS * halo);
    }
    uint2 id = uint2(in.position.xy);
    uint seed = uint(u.beam_speckle.x);
    if (in.layer > 0.0 && glow > MIN_SPECKLE_GLOW && hash_pixel(id, seed) < in.layer) {
        float kind = hash_pixel(id, seed ^ SPECKLE_KIND_SALT);
        float strength = saturate(glow);
        if (kind < u.beam_speckle.y) return float4(0.0, 0.0, 0.0, strength);
        if (kind < u.beam_speckle.y + u.beam_speckle.z) return float4(float3(strength), 0.0);
        return float4(0.0);
    }
    return float4(encode_srgb(light), cover);
}

constant float BOUNDARY_MAX_SLANT = 4.0;
constant float BOUNDARY_MIN_FACE_COS = 0.001;
constant float BOUNDARY_SRGB_GAMMA = 2.2;
constant uint MAX_HAZE_PLANE_COUNT = 4;
constant float HAZE_FOOT_M = 0.3;
constant float HAZE_RAY_DEPTH = 0.5;
constant float4 CITY_HAZE_STYLE = float4(1.0, 0.06, 1.0, 1.0);

/**
 * @brief `boundary_fragment` draws a team boundary plane (W-148, team boundary planes in every level) over what is
 * behind it (`behind`, read back by programmable blending): its glow in its team's colour, as opaque as the map says at
 * each floor's foot (the street and the glass) and fading up its height (`get_boundary_profile`), thicker the more it
 * is seen edge-on (up to `BOUNDARY_MAX_SLANT` times). Each team's plane shows only from its own side (`in.normal`), so
 * the one disc between the hemispheres is blue from blue's half and red from red's.
 * @par Example
 * @code
 * drawn after the glass with the boundary pipeline (no blending, depth tested)
 * @endcode
 */
fragment float4 boundary_fragment(Fragment in [[stage_in]], float4 behind [[color(0)]], constant CityUniforms& u [[buffer(1)]]) {
    float3 view = u.camera_m.xyz - in.world;
    if (dot(view, in.normal) < 0.0) discard_fragment();
    float r_m = length(in.world);
    float floor_m = r_m > u.glass.x ? u.ambient.w : u.glass.x;
    float shown_frac = get_boundary_profile(max(floor_m - r_m, 0.0), in.uv.x);
    float slant = min(1.0 / max(abs(normalize(view).x), BOUNDARY_MIN_FACE_COS), BOUNDARY_MAX_SLANT);
    float alpha = 1.0 - pow(1.0 - in.color.a * shown_frac, slant);
    float3 glow = encode_srgb(pow(get_boundary_color(int(in.layer + 0.5)), BOUNDARY_SRGB_GAMMA) * BOUNDARY_BRIGHTNESS);
    return float4(mix(behind.rgb, glow, alpha), behind.a);
}

/**
 * @brief `CityHaze` matches `render_city.rs`'s `CityHaze`: the inverse view and projection, the camera, the street's and
 * glass's radii with the shimmer's clock in `.w`, the render size and the target's texel size, each boundary plane's
 * (x, side, height, team) and their count.
 */
struct CityHaze {
    float4x4 world_from_clip;
    float4 camera_m;
    float4 radii;
    float4 size;
    float4 planes[MAX_HAZE_PLANE_COUNT];
    int4 counts;
};

/** @brief `HazeVertexOut` is a vertex of `haze_vertex`'s full-screen triangle. */
struct HazeVertexOut {
    float4 position [[position]];
};

/**
 * @brief `haze_vertex` makes a triangle that covers the whole render area, for `haze_fragment`.
 * @par Example
 * @code
 * drawPrimitives(triangle, 0, 3) with the haze pipeline
 * @endcode
 */
vertex HazeVertexOut haze_vertex(uint id [[vertex_id]]) {
    float2 uv = float2((id << 1) & 2, id & 2);
    HazeVertexOut out;
    out.position = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return out;
}

/**
 * @brief `haze_fragment` makes the picture shimmer like heat haze where a team boundary plane covers the screen (W-149,
 * shimmer through boundary planes): it finds where the eye's ray through the pixel crosses a plane seen from its own
 * side, between the core and the street, and how much the plane shows there (`get_boundary_profile` above the floor
 * below it), then samples the frame's copy `drawn` moved by `get_boundary_haze_px` times that, in City Sphere's glassy
 * style (`CITY_HAZE_STYLE`: wide, slow wobbles). It is screen-space: what is in front of the plane shimmers too.
 * @par Example
 * @code
 * a render pass over the resolved frame, with the frame's copy at texture 0 and `CityHaze` at fragment buffer 0
 * @endcode
 */
fragment float4 haze_fragment(HazeVertexOut in [[stage_in]], texture2d<float> drawn [[texture(0)]],
                              constant CityHaze& haze [[buffer(0)]]) {
    constexpr sampler edge(filter::linear, address::clamp_to_edge);
    float2 pixel = in.position.xy;
    float2 ndc = float2(2.0 * pixel.x / haze.size.x - 1.0, 1.0 - 2.0 * pixel.y / haze.size.y);
    float4 far_point = haze.world_from_clip * float4(ndc, HAZE_RAY_DEPTH, 1.0);
    float3 eye = haze.camera_m.xyz;
    float3 dir = normalize(far_point.xyz / far_point.w - eye);
    float weight = 0.0;
    for (int i = 0; i < haze.counts.x; i++) {
        float4 plane = haze.planes[i];
        float t_m = (plane.x - eye.x) / dir.x;
        if ((eye.x - plane.x) * plane.y <= 0.0 || !(t_m > 0.0)) continue;
        float r_m = length(eye + t_m * dir);
        float floor_m = r_m > haze.radii.y ? haze.radii.x : haze.radii.y;
        float above_m = floor_m - r_m;
        weight = max(weight, get_boundary_profile(max(above_m, 0.0), plane.z) * smoothstep(-HAZE_FOOT_M, 0.0, above_m));
    }
    if (weight <= 0.0) return drawn.sample(edge, pixel * haze.size.zw);
    float2 move_px = get_boundary_haze_px(pixel, haze.size.y, haze.radii.w, CITY_HAZE_STYLE) * weight;
    return drawn.sample(edge, (pixel + move_px) * haze.size.zw);
}

/** @brief `VisualsVertexOut` is a vertex of `visuals_vertex`'s full-screen triangle. */
struct VisualsVertexOut {
    float4 position [[position]];
};

/**
 * @brief `visuals_vertex` makes a triangle that covers the whole render area, for `visuals_fragment`.
 * @par Example
 * @code
 * drawPrimitives(triangle, 0, 3) with the visuals pipeline
 * @endcode
 */
vertex VisualsVertexOut visuals_vertex(uint id [[vertex_id]]) {
    float2 uv = float2((id << 1) & 2, id & 2);
    VisualsVertexOut out;
    out.position = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return out;
}

/**
 * @brief `visuals_fragment` shows a pixel of the finished, resolved frame in the player's visuals (W-51, basic
 * visuals): it reads the pixel already there (`drawn`, programmable blending) and gives it back through
 * `apply_player_visuals` (`visuals.metal`, prepended by render_city.rs).
 * @par Example
 * @code
 * a render pass that loads the resolved frame, with the visuals pipeline and `PlayerVisuals` at fragment buffer 0
 * @endcode
 */
fragment float4 visuals_fragment(VisualsVertexOut in [[stage_in]], float4 drawn [[color(0)]],
                                 constant PlayerVisuals& visuals [[buffer(0)]]) {
    return float4(apply_player_visuals(drawn.rgb, visuals), drawn.a);
}
