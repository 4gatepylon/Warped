/**
 * @file viewmodel.metal
 * @brief Draws the held energy revolver (viewmodel.rs): a vertex shader into camera space and a PBR fragment shader with
 * the glTF's base colour, metallic-roughness, normal and emissive maps, lit by a `HandLight` (the scene's light at the
 * eye on voxel maps, `probe_hand_light` in render_voxel.metal; a fixed key light on the house), tone mapped with ACES into premultiplied sRGB for `apply_overlay` to lay over the frame.
 */
#include <metal_stdlib>
using namespace metal;

constant int HAND_LIGHT_COUNT = 3;
constant float EXPOSURE = 1.3;
constant float MIN_ROUGHNESS = 0.045;
constant float MIN_TANGENT_SQUARED = 1e-8;

/** @brief `Vertex` is viewmodel.rs's `Vertex`: a camera-ready glTF vertex (model space, metres). */
struct Vertex {
    float4 position;
    float4 normal;
    float4 tangent;
    float4 uv;
};

/** @brief `DrawUniforms` is viewmodel.rs's `DrawUniforms`: model to clip and to camera space. */
struct DrawUniforms {
    float4x4 clip_from_model;
    float4x4 camera_from_model;
};

/** @brief `MaterialUniforms` is viewmodel.rs's `MaterialUniforms`: colour and emission factors, and metallic, roughness, opacity and whether it blends. */
struct MaterialUniforms {
    float4 base_color;
    float4 emissive;
    float4 params;
};

/**
 * @brief `HandLight` is viewmodel.rs's `HandLight`: in camera space, the unit directions toward up to three lights (zero
 * when absent) and each one's radiance, the local up, and the radiance of the sky and of the ground.
 */
struct HandLight {
    float4 directions[HAND_LIGHT_COUNT];
    float4 colors[HAND_LIGHT_COUNT];
    float4 up;
    float4 sky;
    float4 ground;
};

/** @brief `Fragment` is a vertex in clip space with its camera-space position, frame and texture coordinates. */
struct Fragment {
    float4 position [[position]];
    float3 camera_m;
    float3 normal;
    float4 tangent;
    float2 uv;
};

/** @brief `viewmodel_vertex` puts a vertex in clip and camera space. @par Example @code pipeline(viewmodel_vertex, viewmodel_fragment) @endcode */
vertex Fragment viewmodel_vertex(device const Vertex *vertices [[buffer(0)]], constant DrawUniforms &draw [[buffer(1)]], uint vid [[vertex_id]]) {
    Vertex v = vertices[vid];
    Fragment f;
    f.position = draw.clip_from_model * v.position;
    f.camera_m = (draw.camera_from_model * v.position).xyz;
    f.normal = (draw.camera_from_model * float4(v.normal.xyz, 0.0)).xyz;
    f.tangent = float4((draw.camera_from_model * float4(v.tangent.xyz, 0.0)).xyz, v.tangent.w);
    f.uv = v.uv.xy;
    return f;
}

/** @brief `aces_fitted` is Stephen Hill's fit of the ACES transform (as in the fps_demo). @par Example @code float3 d = aces_fitted(c); @endcode */
static float3 aces_fitted(float3 color) {
    const float3x3 input_matrix = float3x3(float3(0.59719, 0.07600, 0.02840), float3(0.35458, 0.90834, 0.13383), float3(0.04823, 0.01566, 0.83777));
    const float3x3 output_matrix = float3x3(float3(1.60475, -0.10208, -0.00327), float3(-0.53108, 1.10813, -0.07276), float3(-0.07367, -0.00605, 1.07602));
    float3 v = input_matrix * color;
    float3 a = v * (v + 0.0245786) - 0.000090537;
    float3 b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return saturate(output_matrix * (a / b));
}

/** @brief `get_environment` is the light from direction r in camera space: the ground below, the sky above. @par Example @code float3 e = get_environment(hand, r); @endcode */
static float3 get_environment(constant HandLight &hand, float3 r) { return mix(hand.ground.rgb, hand.sky.rgb, smoothstep(-0.2, 0.3, dot(r, hand.up.xyz))); }

/** @brief `get_env_brdf` is Karis' fit of the split-sum environment BRDF (scale, bias on F0). @par Example @code float2 e = get_env_brdf(0.5, 0.8); @endcode */
static float2 get_env_brdf(float roughness, float n_dot_v) {
    const float4 c0 = float4(-1.0, -0.0275, -0.572, 0.022), c1 = float4(1.0, 0.0425, 1.04, -0.04);
    float4 r = roughness * c0 + c1;
    float a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    return float2(-1.04, 1.04) * a004 + r.zw;
}

/** @brief `viewmodel_fragment` shades the revolver with GGX and its maps, in premultiplied display sRGB. @par Example @code pipeline(viewmodel_vertex, viewmodel_fragment) @endcode */
fragment float4 viewmodel_fragment(Fragment f [[stage_in]], bool is_front [[front_facing]], constant MaterialUniforms &m [[buffer(0)]],
                                   constant HandLight &hand [[buffer(1)]],
                                   texture2d_array<float> base_map [[texture(0)]], texture2d_array<float> mr_map [[texture(1)]],
                                   texture2d_array<float> normal_map [[texture(2)]], texture2d_array<float> emissive_map [[texture(3)]]) {
    constexpr sampler wrap(address::repeat, filter::linear, mip_filter::linear, max_anisotropy(8));
    float4 base = base_map.sample(wrap, f.uv, 0) * m.base_color;
    float3 mr = mr_map.sample(wrap, f.uv, 0).rgb;
    float roughness = max(mr.g * m.params.y, MIN_ROUGHNESS), metallic = saturate(mr.b * m.params.x);
    float3 n = normalize(f.normal) * (is_front ? 1.0 : -1.0);
    float3 t = f.tangent.xyz - n * dot(n, f.tangent.xyz);
    if (dot(t, t) > MIN_TANGENT_SQUARED) {
        t = normalize(t);
        float3 bend = normal_map.sample(wrap, f.uv, 0).rgb * 2.0 - 1.0;
        n = normalize(t * bend.x + cross(n, t) * f.tangent.w * bend.y + n * bend.z);
    }
    float3 v = normalize(-f.camera_m), f0 = mix(float3(0.04), base.rgb, metallic), direct = float3(0.0);
    float n_dot_v = max(dot(n, v), 1e-4), alpha = roughness * roughness, alpha_squared = alpha * alpha;
    for (int k = 0; k < HAND_LIGHT_COUNT; k++) {
        float3 l = hand.directions[k].xyz, h = normalize(v + l);
        float n_dot_l = saturate(dot(n, l)), n_dot_h = saturate(dot(n, h)), v_dot_h = saturate(dot(v, h));
        float d_denominator = n_dot_h * n_dot_h * (alpha_squared - 1.0) + 1.0;
        float visibility = 0.5 / max(n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - alpha_squared) + alpha_squared)
                                   + n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - alpha_squared) + alpha_squared), 1e-5);
        float3 fresnel = f0 + (1.0 - f0) * pow(1.0 - v_dot_h, 5.0);
        direct += ((1.0 - fresnel) * (1.0 - metallic) * base.rgb + alpha_squared / (d_denominator * d_denominator) * visibility * fresnel)
            * n_dot_l * hand.colors[k].rgb;
    }
    float2 env = get_env_brdf(roughness, n_dot_v);
    float3 ambient = get_environment(hand, n) * base.rgb * (1.0 - metallic) + get_environment(hand, reflect(-v, n)) * (f0 * env.x + env.y);
    float3 color = direct + ambient + emissive_map.sample(wrap, f.uv, 0).rgb * m.emissive.rgb;
    float3 display = aces_fitted(color * EXPOSURE);
    display = select(1.055 * pow(display, 1.0 / 2.4) - 0.055, display * 12.92, display <= 0.0031308);
    float opacity = m.params.w > 0.5 ? saturate(base.a * m.params.z) : 1.0;
    return float4(display * opacity, opacity);
}
