/**
 * @file
 * @brief `common.metal` holds the structs shared with Rust and the lighting functions that the
 * other shader files use. The Rust side concatenates every `.metal` file into one library.
 *
 * @par Interface
 * - Structs: `Vertex`, `SkinVertex`, `FrameUniforms`, `PassUniforms`, `DrawUniforms`,
 *   `MaterialUniforms`, `GpuPointLight`, `GpuDecal`, `GpuParticle`, `GpuHudRect`.
 * - Functions: `shade_surface`, `apply_fog`, `apply_shadowed_fog`, `equirect_uv_from_direction`,
 *   and their helpers.
 *
 * @par Example
 * @code
 * float3 color = shade_surface(frame, surface, textures);
 * @endcode
 */
#include <metal_stdlib>
using namespace metal;

constant float PI = 3.14159265358979;
constant uint CASCADE_COUNT = 4;
constant uint MAX_POINT_LIGHT_COUNT = 64;
constant float MIN_ROUGHNESS = 0.045;
constant float SKY_VISIBILITY_NORMAL_OFFSET_M = 0.6;
constant float SHADOW_NORMAL_OFFSET_TEXELS = 1.5;
constant float SHADOW_FILTER_RADIUS_TEXELS = 1.6;
constant float CASCADE_BLEND_FRAC = 0.12;
constant uint SHADOW_TAP_COUNT = 12;
constant float FOG_MAX_DISTANCE_M = 2000.0;
constant float FOG_SUN_LOBE_G = 0.7;
constant float3 FOG_EXTINCTION_TINT = float3(0.82, 1.0, 1.24);
constant uint HAZE_STEP_COUNT = 12;
constant float HAZE_MAX_DISTANCE_M = 64.0;
constant float DUST_DENSITY_PER_M = 0.05;
constant float DUST_AMBIENT_FRAC = 0.2;
constant float DUST_SUN_LOBE_G = 0.45;

constant float2 POISSON_DISK[SHADOW_TAP_COUNT] = {
    float2(-0.326, -0.406), float2(-0.840, -0.074), float2(-0.696, 0.457),
    float2(-0.203, 0.621),  float2(0.962, -0.195),  float2(0.473, -0.480),
    float2(0.519, 0.767),   float2(0.185, -0.893),  float2(0.507, 0.064),
    float2(0.896, 0.412),   float2(-0.322, -0.933), float2(-0.792, -0.598)};

constant bool IS_SKINNED [[function_constant(0)]];

/** @brief `Vertex` matches `render::Vertex` in Rust. */
struct Vertex {
    packed_float3 position;
    packed_float3 normal;
    packed_float4 tangent;
    packed_float2 uv;
};

/** @brief `SkinVertex` matches `render::SkinVertex` in Rust. */
struct SkinVertex {
    ushort4 joints;
    packed_float4 weights;
};

/** @brief `FrameUniforms` matches `GpuFrameUniforms` in `render/metal/mod.rs`. */
struct FrameUniforms {
    float4x4 view_proj;
    float4x4 view;
    float4x4 proj;
    float4x4 inv_view_proj;
    float4x4 inv_proj;
    float4x4 viewmodel_view_proj;
    float4x4 cascade_view_proj[CASCADE_COUNT];
    float4 cascade_split_far_m;
    float4 cascade_texel_size_m;
    float4 camera_position_m;
    float4 sun_direction;
    float4 sun_illuminance;
    float4 sh9[9];
    float4 fog_color;
    float4 fog_params;
    float4 sky_visibility_origin_m;
    float4 sky_visibility_inv_size_m;
    float4 screen_size;
    float4 render_params;
    float4 post_params;
    float4 bounce_color;
    float4 effect_params;
};

/** @brief `PassUniforms` is the view-projection matrix of one pass (camera or shadow cascade). */
struct PassUniforms {
    float4x4 view_proj;
    float4 flags;
};

/** @brief `DrawUniforms` matches `GpuDrawUniforms`: one draw's model matrix and tint. */
struct DrawUniforms {
    float4x4 model;
    float4 tint;
    uint4 flags;
};

/** @brief `MaterialUniforms` matches `GpuMaterialUniforms`: one material's constant factors. */
struct MaterialUniforms {
    float4 base_color_factor;
    float4 emissive_factor;
    float4 params;
};

/** @brief `GpuPointLight` matches the Rust struct of the same name. */
struct GpuPointLight {
    float4 position_radius;
    float4 color;
};

/** @brief `GpuDecal` matches the Rust struct of the same name. */
struct GpuDecal {
    float4 position_size;
    float4 normal_seed;
    float4 tangent;
};

/** @brief `GpuParticle` matches the Rust struct of the same name. */
struct GpuParticle {
    float4 position_size;
    float4 stretch_kind;
    float4 color;
    float4 seed;
};

/** @brief `GpuHudRect` matches the Rust struct of the same name. */
struct GpuHudRect {
    float4 rect_px;
    float4 color;
};

/** @brief `SceneVertexOut` is what the scene vertex shaders hand to the fragment shaders. */
struct SceneVertexOut {
    float4 position [[position, invariant]];
    float3 world_position;
    float3 normal;
    float4 tangent;
    float2 uv;
};

/** @brief `SceneTextures` are the textures that `shade_surface` reads. */
struct SceneTextures {
    depth2d_array<float> shadow_map;
    texture3d<float> sky_visibility;
    texturecube<float> prefiltered;
    texture2d<float> brdf_lut;
    texture2d<float> ssao;
};

/** @brief `Surface` is one shaded point: its position, normal and material. */
struct Surface {
    float3 world_position;
    float3 normal;
    float3 base_color;
    float roughness;
    float metallic;
    float occlusion;
    float3 emissive;
    float2 screen_position;
    bool should_use_ssao;
};

/**
 * @brief `interleaved_gradient_noise` returns a per-pixel noise value in [0, 1).
 * @param pixel The pixel position.
 * @par Example
 * @code
 * float angle = 2.0 * PI * interleaved_gradient_noise(in.position.xy);
 * @endcode
 */
inline float interleaved_gradient_noise(float2 pixel) {
    return fract(52.9829189 * fract(dot(pixel, float2(0.06711056, 0.00583715))));
}

/**
 * @brief `equirect_uv_from_direction` maps a direction to equirectangular texture coordinates.
 * @param direction A unit direction, +Y up.
 * @return u around the horizon and v from the zenith (0) to the nadir (1).
 * @par Example
 * @code
 * float2 uv = equirect_uv_from_direction(normalize(d));
 * @endcode
 */
inline float2 equirect_uv_from_direction(float3 direction) {
    return float2(0.5 + atan2(direction.x, -direction.z) / (2.0 * PI),
                  acos(clamp(direction.y, -1.0, 1.0)) / PI);
}

/**
 * @brief `evaluate_sh9` returns the irradiance divided by pi for a normal, from SH9 coefficients.
 * @param sh The coefficients, already convolved with the clamped cosine.
 * @param n The unit normal.
 * @par Example
 * @code
 * float3 diffuse_light = evaluate_sh9(frame.sh9, n);
 * @endcode
 */
inline float3 evaluate_sh9(constant float4* sh, float3 n) {
    float3 result = sh[0].rgb * 0.282095;
    result += sh[1].rgb * 0.488603 * n.y;
    result += sh[2].rgb * 0.488603 * n.z;
    result += sh[3].rgb * 0.488603 * n.x;
    result += sh[4].rgb * 1.092548 * n.x * n.y;
    result += sh[5].rgb * 1.092548 * n.y * n.z;
    result += sh[6].rgb * 0.315392 * (3.0 * n.z * n.z - 1.0);
    result += sh[7].rgb * 1.092548 * n.x * n.z;
    result += sh[8].rgb * 0.546274 * (n.x * n.x - n.y * n.y);
    return max(result, 0.0);
}

/**
 * @brief `fresnel_schlick_roughness` is Schlick's Fresnel, damped for rough surfaces.
 * @param cos_theta The cosine between the view direction and the normal.
 * @param f0 The reflectance at normal incidence.
 * @param roughness The perceptual roughness.
 * @par Example
 * @code
 * float3 f = fresnel_schlick_roughness(n_dot_v, f0, roughness);
 * @endcode
 */
inline float3 fresnel_schlick_roughness(float cos_theta, float3 f0, float roughness) {
    return f0 + (max(float3(1.0 - roughness), f0) - f0) * pow(1.0 - cos_theta, 5.0);
}

/**
 * @brief `evaluate_direct_light` is the GGX specular plus Lambert diffuse for one light.
 * @param n The normal.
 * @param v The direction to the eye.
 * @param l The direction to the light.
 * @param base_color The albedo.
 * @param roughness The perceptual roughness.
 * @param metallic The metalness.
 * @param energy_compensation The multiple-scattering boost of the specular lobe (1 for none).
 * @return The reflected radiance per unit of light irradiance.
 * @par Example
 * @code
 * float3 radiance = evaluate_direct_light(n, v, l, albedo, 0.5, 0.0, float3(1.0)) * irradiance;
 * @endcode
 */
inline float3 evaluate_direct_light(float3 n, float3 v, float3 l, float3 base_color,
                                    float roughness, float metallic, float3 energy_compensation) {
    float3 h = normalize(v + l);
    float n_dot_l = saturate(dot(n, l));
    float n_dot_v = max(dot(n, v), 1e-4);
    float n_dot_h = saturate(dot(n, h));
    float v_dot_h = saturate(dot(v, h));
    float alpha = roughness * roughness;
    float alpha_squared = alpha * alpha;
    float d_denominator = n_dot_h * n_dot_h * (alpha_squared - 1.0) + 1.0;
    float d = alpha_squared / (PI * d_denominator * d_denominator);
    float visibility_v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - alpha_squared) + alpha_squared);
    float visibility_l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - alpha_squared) + alpha_squared);
    float visibility = 0.5 / max(visibility_v + visibility_l, 1e-5);
    float3 f0 = mix(float3(0.04), base_color, metallic);
    float3 f = f0 + (1.0 - f0) * pow(1.0 - v_dot_h, 5.0);
    float3 specular = d * visibility * f * energy_compensation;
    float3 diffuse = (1.0 - f) * (1.0 - metallic) * base_color / PI;
    return (diffuse + specular) * n_dot_l;
}

/**
 * @brief `sample_cascade` filters one shadow cascade with a rotated Poisson disk.
 * @param frame The frame uniforms.
 * @param shadow_map The cascades.
 * @param cascade The cascade to read.
 * @param world_position The (normal-offset) point to test.
 * @param rotation_angle The per-pixel rotation of the disk.
 * @return 1 when lit, 0 when in shadow.
 * @par Example
 * @code
 * float lit = sample_cascade(frame, shadow_map, 0, p, angle);
 * @endcode
 */
inline float sample_cascade(constant FrameUniforms& frame, depth2d_array<float> shadow_map,
                            uint cascade, float3 world_position, float rotation_angle) {
    constexpr sampler shadow_sampler(coord::normalized, filter::linear, address::clamp_to_edge,
                                     compare_func::less_equal);
    float4 shadow_clip = frame.cascade_view_proj[cascade] * float4(world_position, 1.0);
    float3 shadow_ndc = shadow_clip.xyz / shadow_clip.w;
    float2 shadow_uv = shadow_ndc.xy * float2(0.5, -0.5) + 0.5;
    if (any(shadow_uv < 0.0) || any(shadow_uv > 1.0) || shadow_ndc.z > 1.0) {
        return 1.0;
    }
    float2x2 rotation = float2x2(float2(cos(rotation_angle), sin(rotation_angle)),
                                 float2(-sin(rotation_angle), cos(rotation_angle)));
    float texel_uv = frame.post_params.y;
    float lit = 0.0;
    for (uint i = 0; i < SHADOW_TAP_COUNT; ++i) {
        float2 offset = rotation * POISSON_DISK[i] * SHADOW_FILTER_RADIUS_TEXELS * texel_uv;
        lit += shadow_map.sample_compare(shadow_sampler, shadow_uv + offset, cascade,
                                         shadow_ndc.z);
    }
    return lit / float(SHADOW_TAP_COUNT);
}

/**
 * @brief `sample_sun_shadow` picks the cascade for a point and blends into the next one at the
 * cascade's far edge.
 * @param frame The frame uniforms.
 * @param shadow_map The cascades.
 * @param world_position The shaded point.
 * @param normal The surface normal, for the normal-offset bias.
 * @param screen_position The pixel, for the noise rotation.
 * @return 1 when lit, 0 when in shadow.
 * @par Example
 * @code
 * float lit = sample_sun_shadow(frame, textures.shadow_map, p, n, in.position.xy);
 * @endcode
 */
inline float sample_sun_shadow(constant FrameUniforms& frame, depth2d_array<float> shadow_map,
                               float3 world_position, float3 normal, float2 screen_position) {
    float view_depth_m = distance(world_position, frame.camera_position_m.xyz);
    uint cascade = 0;
    while (cascade < CASCADE_COUNT && view_depth_m > frame.cascade_split_far_m[cascade]) {
        cascade++;
    }
    if (cascade >= CASCADE_COUNT) {
        return 1.0;
    }
    float3 l = frame.sun_direction.xyz;
    float n_dot_l = saturate(dot(normal, l));
    float slope = sqrt(1.0 - n_dot_l * n_dot_l);
    float rotation_angle = 2.0 * PI * interleaved_gradient_noise(screen_position);
    float3 offset_position =
        world_position + normal * frame.cascade_texel_size_m[cascade] *
                             SHADOW_NORMAL_OFFSET_TEXELS * (0.5 + slope);
    float lit = sample_cascade(frame, shadow_map, cascade, offset_position, rotation_angle);
    float cascade_near_m = cascade == 0 ? 0.0 : frame.cascade_split_far_m[cascade - 1];
    float cascade_far_m = frame.cascade_split_far_m[cascade];
    float blend_start_m = cascade_far_m - (cascade_far_m - cascade_near_m) * CASCADE_BLEND_FRAC;
    if (view_depth_m > blend_start_m && cascade + 1 < CASCADE_COUNT) {
        float3 next_offset_position =
            world_position + normal * frame.cascade_texel_size_m[cascade + 1] *
                                 SHADOW_NORMAL_OFFSET_TEXELS * (0.5 + slope);
        float next_lit =
            sample_cascade(frame, shadow_map, cascade + 1, next_offset_position, rotation_angle);
        lit = mix(lit, next_lit, smoothstep(blend_start_m, cascade_far_m, view_depth_m));
    }
    return lit;
}

/**
 * @brief `sample_sky_visibility` reads the baked fraction of the sky a point sees.
 * @param frame The frame uniforms.
 * @param sky_visibility The baked volume.
 * @param world_position The shaded point.
 * @param normal The surface normal; the volume is read one cell out along it.
 * @par Example
 * @code
 * float visibility = sample_sky_visibility(frame, textures.sky_visibility, p, n);
 * @endcode
 */
inline float sample_sky_visibility(constant FrameUniforms& frame, texture3d<float> sky_visibility,
                                   float3 world_position, float3 normal) {
    constexpr sampler volume_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float3 sample_position = world_position + normal * SKY_VISIBILITY_NORMAL_OFFSET_M;
    float3 uvw = (sample_position - frame.sky_visibility_origin_m.xyz) *
                 frame.sky_visibility_inv_size_m.xyz;
    return sky_visibility.sample(volume_sampler, uvw).r;
}

/**
 * @brief `shade_surface` lights a surface with the sun, the point lights and the sky.
 * @param frame The frame uniforms.
 * @param surface The point to shade.
 * @param textures The shadow, sky visibility, IBL and SSAO textures.
 * @param point_lights The point lights; `frame.post_params.w` says how many there are.
 * @return The outgoing radiance toward the eye.
 * @par Example
 * @code
 * float3 color = shade_surface(frame, surface, textures, point_lights);
 * @endcode
 */
inline float3 shade_surface(constant FrameUniforms& frame, Surface surface,
                            SceneTextures textures, device const GpuPointLight* point_lights) {
    constexpr sampler linear_sampler(coord::normalized, filter::linear, mip_filter::linear,
                                     address::clamp_to_edge);
    float3 n = surface.normal;
    float3 v = normalize(frame.camera_position_m.xyz - surface.world_position);
    float roughness = clamp(surface.roughness, MIN_ROUGHNESS, 1.0);
    float n_dot_v = max(dot(n, v), 1e-4);
    bool is_material_response_enabled = frame.effect_params.z > 0.5;
    float3 f0 = mix(float3(0.04), surface.base_color, surface.metallic);
    float2 brdf = textures.brdf_lut.sample(linear_sampler, float2(n_dot_v, roughness)).rg;
    float3 energy_compensation =
        is_material_response_enabled ? 1.0 + f0 * (1.0 / max(brdf.x + brdf.y, 1e-3) - 1.0)
                                     : float3(1.0);

    float sun_lit = sample_sun_shadow(frame, textures.shadow_map, surface.world_position, n,
                                      surface.screen_position);
    float3 color = evaluate_direct_light(n, v, frame.sun_direction.xyz, surface.base_color,
                                         roughness, surface.metallic, energy_compensation) *
                   frame.sun_illuminance.rgb * sun_lit;

    uint point_light_count = min(uint(frame.post_params.w), MAX_POINT_LIGHT_COUNT);
    for (uint i = 0; i < point_light_count; ++i) {
        float3 to_light = point_lights[i].position_radius.xyz - surface.world_position;
        float distance_squared = max(dot(to_light, to_light), 1e-4);
        float radius_m = point_lights[i].position_radius.w;
        float falloff = saturate(1.0 - pow(distance_squared / (radius_m * radius_m), 2.0));
        float attenuation = falloff * falloff / (distance_squared + 1.0);
        if (attenuation > 0.0) {
            float3 l = to_light * rsqrt(distance_squared);
            color += evaluate_direct_light(n, v, l, surface.base_color, roughness,
                                           surface.metallic, energy_compensation) *
                     point_lights[i].color.rgb * attenuation;
        }
    }

    float ssao = 1.0;
    if (surface.should_use_ssao && frame.render_params.z > 0.5) {
        float2 ssao_uv = surface.screen_position * frame.screen_size.zw;
        ssao = textures.ssao.sample(linear_sampler, ssao_uv).r;
    }
    float occlusion = surface.occlusion * ssao;
    float sky_visibility =
        sample_sky_visibility(frame, textures.sky_visibility, surface.world_position, n);
    float ibl_intensity = frame.post_params.z;
    float3 f = fresnel_schlick_roughness(n_dot_v, f0, roughness);
    float3 single_scatter = f * brdf.x + brdf.y;
    float3 multi_scatter = float3(0.0);
    float3 diffuse_occlusion = float3(occlusion);
    if (is_material_response_enabled) {
        float energy = brdf.x + brdf.y;
        float3 average_fresnel = f0 + (1.0 - f0) / 21.0;
        multi_scatter = single_scatter * average_fresnel * (1.0 - energy) /
                        (1.0 - (1.0 - energy) * average_fresnel);
        float3 a = 2.0404 * surface.base_color - 0.3324;
        float3 b = -4.7951 * surface.base_color + 0.6417;
        float3 c = 2.7552 * surface.base_color + 0.6903;
        diffuse_occlusion = max(occlusion, ((occlusion * a + b) * occlusion + c) * occlusion);
    }
    float3 diffuse_weight = is_material_response_enabled
                                ? (1.0 - single_scatter - multi_scatter) * (1.0 - surface.metallic) *
                                      surface.base_color
                                : (1.0 - f) * (1.0 - surface.metallic) * surface.base_color;
    float3 sky_irradiance = evaluate_sh9(frame.sh9, n) * ibl_intensity;
    float3 bounce = frame.bounce_color.rgb * (0.6 + 0.4 * saturate(n.y * 0.5 + 0.5));
    float3 ambient_irradiance = mix(bounce, sky_irradiance, sky_visibility);
    color += (diffuse_weight + multi_scatter) * ambient_irradiance * diffuse_occlusion;

    float3 r = reflect(-v, n);
    float mip_count = frame.render_params.w;
    float3 prefiltered =
        textures.prefiltered.sample(linear_sampler, r, level(roughness * (mip_count - 1.0))).rgb *
        ibl_intensity;
    float specular_occlusion = saturate(pow(n_dot_v + occlusion, exp2(-16.0 * roughness - 1.0)) -
                                        1.0 + occlusion);
    float3 reflected = mix(bounce, prefiltered, sky_visibility * sky_visibility);
    float3 specular_weight = is_material_response_enabled ? single_scatter : f0 * brdf.x + brdf.y;
    color += reflected * specular_weight * specular_occlusion;
    color += surface.emissive;
    return color;
}

/**
 * @brief `fog_opacity` is the fraction of light that height fog scatters out of a view ray, per
 * color channel (blue is scattered more, as in a real atmosphere).
 * @param frame The frame uniforms.
 * @param ray_direction The unit direction of the ray from the camera.
 * @param ray_length_m The length of the ray.
 * @return One minus the transmittance along the ray.
 * @par Example
 * @code
 * float3 opacity = fog_opacity(frame, normalize(ray), length(ray));
 * @endcode
 */
inline float3 fog_opacity(constant FrameUniforms& frame, float3 ray_direction,
                          float ray_length_m) {
    float3 camera_position = frame.camera_position_m.xyz;
    float density = frame.fog_color.w;
    float falloff = frame.fog_params.x;
    float base_height_m = frame.fog_params.y;
    float height_delta_m = ray_direction.y * ray_length_m;
    float camera_density = density * exp(-falloff * (camera_position.y - base_height_m));
    float integral_factor = abs(falloff * height_delta_m) > 1e-3
                                ? (1.0 - exp(-falloff * height_delta_m)) / (falloff * height_delta_m)
                                : 1.0;
    float optical_depth = camera_density * integral_factor * ray_length_m;
    float3 tint = frame.effect_params.y > 0.5 ? FOG_EXTINCTION_TINT : float3(1.0);
    return 1.0 - exp(-optical_depth * tint);
}

/**
 * @brief `fog_sun_phase` is the Henyey-Greenstein phase function toward the sun.
 * @param frame The frame uniforms.
 * @param ray_direction The unit direction of the ray from the camera.
 * @param g The anisotropy: 0 scatters evenly, near 1 scatters forward.
 * @par Example
 * @code
 * float3 sun_inscatter = frame.sun_illuminance.rgb * fog_sun_phase(frame, ray_direction, 0.7);
 * @endcode
 */
inline float fog_sun_phase(constant FrameUniforms& frame, float3 ray_direction, float g) {
    float cos_sun = dot(ray_direction, frame.sun_direction.xyz);
    return (1.0 - g * g) / (4.0 * PI * pow(1.0 + g * g - 2.0 * g * cos_sun, 1.5));
}

/**
 * @brief `apply_fog` adds exponential height fog, brighter toward the sun.
 * @param frame The frame uniforms.
 * @param color The radiance before fog.
 * @param world_position The far end of the view ray.
 * @return The radiance after fog.
 * @par Example
 * @code
 * color = apply_fog(frame, color, in.world_position);
 * @endcode
 */
inline float3 apply_fog(constant FrameUniforms& frame, float3 color, float3 world_position) {
    float3 camera_position = frame.camera_position_m.xyz;
    float3 ray = world_position - camera_position;
    float ray_length_m = min(length(ray), FOG_MAX_DISTANCE_M);
    float3 ray_direction = normalize(ray);
    float3 fog_amount = min(fog_opacity(frame, ray_direction, ray_length_m), frame.fog_params.w);
    float3 inscatter = frame.fog_color.rgb * frame.post_params.z +
                       frame.sun_illuminance.rgb * fog_sun_phase(frame, ray_direction, FOG_SUN_LOBE_G) *
                           frame.fog_params.z;
    return mix(color, inscatter, fog_amount);
}

/**
 * @brief `apply_shadowed_fog` is `apply_fog` with the sun's in-scattering ray-marched through
 * the shadow cascades, so the haze shows light shafts and is dark where the sun is blocked.
 * Sheltered air (low sky visibility, such as inside the warehouse) also holds dust, which
 * makes sunbeams through openings visible.
 * @param frame The frame uniforms.
 * @param color The radiance before fog.
 * @param world_position The far end of the view ray.
 * @param shadow_map The cascades.
 * @param sky_visibility The baked sky-visibility volume, for the dust.
 * @param screen_position The pixel, for the march's jitter.
 * @return The radiance after fog.
 * @par Interface
 * `frame.effect_params.y` is 0 to fall back to `apply_fog`. The march covers the first
 * `HAZE_MAX_DISTANCE_M`; the rest of the ray is treated as lit.
 * @par Example
 * @code
 * color = apply_shadowed_fog(frame, color, p, shadow_map, sky_visibility, in.position.xy);
 * @endcode
 */
inline float3 apply_shadowed_fog(constant FrameUniforms& frame, float3 color,
                                 float3 world_position, depth2d_array<float> shadow_map,
                                 texture3d<float> sky_visibility, float2 screen_position) {
    if (frame.effect_params.y < 0.5) {
        return apply_fog(frame, color, world_position);
    }
    constexpr sampler shadow_sampler(coord::normalized, filter::linear, address::clamp_to_edge,
                                     compare_func::less_equal);
    constexpr sampler volume_sampler(coord::normalized, filter::linear, address::clamp_to_edge);
    float3 camera_position = frame.camera_position_m.xyz;
    float3 ray = world_position - camera_position;
    float ray_length_m = min(length(ray), FOG_MAX_DISTANCE_M);
    float3 ray_direction = normalize(ray);
    float3 fog_amount = min(fog_opacity(frame, ray_direction, ray_length_m), frame.fog_params.w);
    float march_length_m = min(ray_length_m, HAZE_MAX_DISTANCE_M);
    float dt = march_length_m / float(HAZE_STEP_COUNT);
    float jitter = interleaved_gradient_noise(screen_position);
    float3 lit_opacity = float3(0.0);
    float3 previous_opacity = float3(0.0);
    float dust_transmittance = 1.0;
    float dust_lit_opacity = 0.0;
    for (uint i = 0; i < HAZE_STEP_COUNT; ++i) {
        float t = (float(i) + 1.0) * dt;
        float3 opacity = fog_opacity(frame, ray_direction, t);
        float3 p = camera_position + ray_direction * ((float(i) + jitter) * dt);
        uint cascade = 0;
        while (cascade < CASCADE_COUNT - 1 && t > frame.cascade_split_far_m[cascade]) {
            cascade++;
        }
        float4 shadow_clip = frame.cascade_view_proj[cascade] * float4(p, 1.0);
        float3 shadow_ndc = shadow_clip.xyz / shadow_clip.w;
        float2 shadow_uv = shadow_ndc.xy * float2(0.5, -0.5) + 0.5;
        bool is_outside_cascade = any(shadow_uv < 0.0) || any(shadow_uv > 1.0) || shadow_ndc.z > 1.0;
        float lit = is_outside_cascade
                        ? 1.0
                        : shadow_map.sample_compare(shadow_sampler, shadow_uv, cascade, shadow_ndc.z);
        lit_opacity += (opacity - previous_opacity) * lit;
        previous_opacity = opacity;
        float3 uvw = (p - frame.sky_visibility_origin_m.xyz) * frame.sky_visibility_inv_size_m.xyz;
        float shelter = 1.0 - sky_visibility.sample(volume_sampler, uvw).r;
        float dust_opacity = 1.0 - exp(-DUST_DENSITY_PER_M * shelter * shelter * dt);
        dust_lit_opacity += dust_transmittance * dust_opacity * lit;
        dust_transmittance *= 1.0 - dust_opacity;
    }
    lit_opacity += max(fog_amount - previous_opacity, 0.0);
    float3 ambient = frame.fog_color.rgb * frame.post_params.z;
    float3 sun = frame.sun_illuminance.rgb *
                 fog_sun_phase(frame, ray_direction, FOG_SUN_LOBE_G) * frame.fog_params.z;
    float3 dust_sun = frame.sun_illuminance.rgb *
                      fog_sun_phase(frame, ray_direction, DUST_SUN_LOBE_G);
    float3 fogged = color * (1.0 - fog_amount) + ambient * fog_amount + sun * lit_opacity;
    float3 dust_inscatter = frame.bounce_color.rgb * DUST_AMBIENT_FRAC * (1.0 - dust_transmittance) +
                            dust_sun * dust_lit_opacity;
    return fogged * dust_transmittance + dust_inscatter * (1.0 - fog_amount);
}
