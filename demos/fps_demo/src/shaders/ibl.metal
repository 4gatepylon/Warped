/**
 * @file
 * @brief `ibl.metal` holds the compute kernels that run once at startup: the sky cube map, the
 * prefiltered specular cube map, the split-sum BRDF table and the sky visibility volume.
 *
 * @par Interface
 * - Kernels: `convert_equirect_to_cube`, `prefilter_specular`, `integrate_brdf`,
 *   `bake_sky_visibility`.
 *
 * @par Example
 * @code
 * dispatch convert_equirect_to_cube over (face_size, face_size, 6)
 * @endcode
 */

constant uint PREFILTER_SAMPLE_COUNT = 192;
constant uint BRDF_SAMPLE_COUNT = 256;
constant uint SKY_VISIBILITY_RAY_COUNT = 96;
constant float SKY_VISIBILITY_MAX_DISTANCE_M = 60.0;
constant float SKY_VISIBILITY_RAY_START_M = 0.05;

/** @brief `OccluderBoxGpu` matches `GpuOccluderBox` in Rust. */
struct OccluderBoxGpu {
    float4 min_m;
    float4 max_m;
};

/** @brief `SkyVisibilityParams` matches `GpuSkyVisibilityParams` in Rust. */
struct SkyVisibilityParams {
    float4 origin_m;
    float4 cell_size_m;
    uint4 counts;
};

/**
 * @brief `direction_from_cube_texel` returns the direction through a texel of a cube face.
 * @param face The face, in Metal's +X, -X, +Y, -Y, +Z, -Z order.
 * @param texel The texel on the face.
 * @param face_size The face's side in texels.
 * @par Example
 * @code
 * float3 d = direction_from_cube_texel(face, gid.xy, size);
 * @endcode
 */
inline float3 direction_from_cube_texel(uint face, uint2 texel, uint face_size) {
    float2 uv = (float2(texel) + 0.5) / float(face_size) * 2.0 - 1.0;
    float3 direction;
    switch (face) {
        case 0: direction = float3(1.0, -uv.y, -uv.x); break;
        case 1: direction = float3(-1.0, -uv.y, uv.x); break;
        case 2: direction = float3(uv.x, 1.0, uv.y); break;
        case 3: direction = float3(uv.x, -1.0, -uv.y); break;
        case 4: direction = float3(uv.x, -uv.y, 1.0); break;
        default: direction = float3(-uv.x, -uv.y, -1.0); break;
    }
    return normalize(direction);
}

/**
 * @brief `hammersley` returns the i-th of n low-discrepancy points in the unit square.
 * @param i The point's index.
 * @param n How many points there are.
 * @par Example
 * @code
 * float2 xi = hammersley(i, SAMPLE_COUNT);
 * @endcode
 */
inline float2 hammersley(uint i, uint n) {
    return float2(float(i) / float(n), float(reverse_bits(i)) * 2.3283064365386963e-10);
}

/**
 * @brief `importance_sample_ggx` returns a GGX-distributed half vector around a normal.
 * @param xi A point in the unit square.
 * @param n The normal.
 * @param roughness The perceptual roughness.
 * @par Example
 * @code
 * float3 h = importance_sample_ggx(hammersley(i, count), n, 0.5);
 * @endcode
 */
inline float3 importance_sample_ggx(float2 xi, float3 n, float roughness) {
    float alpha = roughness * roughness;
    float phi = 2.0 * PI * xi.x;
    float cos_theta = sqrt((1.0 - xi.y) / (1.0 + (alpha * alpha - 1.0) * xi.y));
    float sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    float3 h_tangent = float3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta);
    float3 up = abs(n.z) < 0.999 ? float3(0.0, 0.0, 1.0) : float3(1.0, 0.0, 0.0);
    float3 tangent = normalize(cross(up, n));
    float3 bitangent = cross(n, tangent);
    return normalize(tangent * h_tangent.x + bitangent * h_tangent.y + n * h_tangent.z);
}

/**
 * @brief `convert_equirect_to_cube` resamples the equirectangular sky into a cube map.
 * @param equirect The sky, linear RGB.
 * @param cube The cube map to write.
 * @param gid The texel and face.
 * @par Example
 * @code
 * dispatch over (face_size, face_size, 6)
 * @endcode
 */
kernel void convert_equirect_to_cube(texture2d<float, access::sample> equirect [[texture(0)]],
                                     texturecube<half, access::write> cube [[texture(1)]],
                                     uint3 gid [[thread_position_in_grid]]) {
    constexpr sampler equirect_sampler(coord::normalized, filter::linear, address::repeat);
    uint face_size = cube.get_width();
    if (gid.x >= face_size || gid.y >= face_size) {
        return;
    }
    float3 direction = direction_from_cube_texel(gid.z, gid.xy, face_size);
    float2 uv = equirect_uv_from_direction(direction);
    float3 radiance = equirect.sample(equirect_sampler, uv, level(0.0)).rgb;
    cube.write(half4(half3(radiance), 1.0h), gid.xy, gid.z);
}

/**
 * @brief `prefilter_specular` convolves the sky with GGX lobes for one roughness (one mip).
 * @param environment The sky cube map with its mips.
 * @param prefiltered The mip of the prefiltered cube map to write.
 * @param roughness The roughness this mip stands for.
 * @param gid The texel and face.
 * @par Example
 * @code
 * dispatch once per mip over (mip_size, mip_size, 6)
 * @endcode
 */
kernel void prefilter_specular(texturecube<float, access::sample> environment [[texture(0)]],
                               texturecube<half, access::write> prefiltered [[texture(1)]],
                               constant float& roughness [[buffer(0)]],
                               uint3 gid [[thread_position_in_grid]]) {
    constexpr sampler cube_sampler(coord::normalized, filter::linear, mip_filter::linear);
    uint face_size = prefiltered.get_width();
    if (gid.x >= face_size || gid.y >= face_size) {
        return;
    }
    float3 n = direction_from_cube_texel(gid.z, gid.xy, face_size);
    if (roughness < 1e-3) {
        prefiltered.write(half4(half3(environment.sample(cube_sampler, n, level(0.0)).rgb), 1.0h),
                          gid.xy, gid.z);
        return;
    }
    float environment_size = float(environment.get_width());
    float texel_solid_angle = 4.0 * PI / (6.0 * environment_size * environment_size);
    float alpha = roughness * roughness;
    float3 sum = 0.0;
    float weight_sum = 0.0;
    for (uint i = 0; i < PREFILTER_SAMPLE_COUNT; ++i) {
        float3 h = importance_sample_ggx(hammersley(i, PREFILTER_SAMPLE_COUNT), n, roughness);
        float3 l = normalize(2.0 * dot(n, h) * h - n);
        float n_dot_l = dot(n, l);
        if (n_dot_l <= 0.0) {
            continue;
        }
        float n_dot_h = saturate(dot(n, h));
        float d_denominator = n_dot_h * n_dot_h * (alpha * alpha - 1.0) + 1.0;
        float d = alpha * alpha / (PI * d_denominator * d_denominator);
        float pdf = d / 4.0 + 1e-4;
        float sample_solid_angle = 1.0 / (float(PREFILTER_SAMPLE_COUNT) * pdf);
        float mip = 0.5 * log2(sample_solid_angle / texel_solid_angle) + 1.0;
        sum += environment.sample(cube_sampler, l, level(max(mip, 0.0))).rgb * n_dot_l;
        weight_sum += n_dot_l;
    }
    prefiltered.write(half4(half3(sum / max(weight_sum, 1e-4)), 1.0h), gid.xy, gid.z);
}

/**
 * @brief `integrate_brdf` fills the split-sum table: scale and bias on F0 by (n.v, roughness).
 * @param lut The table to write.
 * @param gid The texel.
 * @par Example
 * @code
 * dispatch over (lut_size, lut_size)
 * @endcode
 */
kernel void integrate_brdf(texture2d<half, access::write> lut [[texture(0)]],
                           uint2 gid [[thread_position_in_grid]]) {
    uint size = lut.get_width();
    if (gid.x >= size || gid.y >= size) {
        return;
    }
    float n_dot_v = max((float(gid.x) + 0.5) / float(size), 1e-3);
    float roughness = (float(gid.y) + 0.5) / float(size);
    float3 v = float3(sqrt(1.0 - n_dot_v * n_dot_v), 0.0, n_dot_v);
    float3 n = float3(0.0, 0.0, 1.0);
    float alpha = roughness * roughness;
    float k = alpha / 2.0;
    float scale = 0.0;
    float bias = 0.0;
    for (uint i = 0; i < BRDF_SAMPLE_COUNT; ++i) {
        float3 h = importance_sample_ggx(hammersley(i, BRDF_SAMPLE_COUNT), n, roughness);
        float3 l = normalize(2.0 * dot(v, h) * h - v);
        float n_dot_l = saturate(l.z);
        float n_dot_h = saturate(h.z);
        float v_dot_h = saturate(dot(v, h));
        if (n_dot_l > 0.0) {
            float g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);
            float g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);
            float g_visibility = g_v * g_l * v_dot_h / (n_dot_h * n_dot_v);
            float fresnel_weight = pow(1.0 - v_dot_h, 5.0);
            scale += (1.0 - fresnel_weight) * g_visibility;
            bias += fresnel_weight * g_visibility;
        }
    }
    lut.write(half4(half(scale / BRDF_SAMPLE_COUNT), half(bias / BRDF_SAMPLE_COUNT), 0.0h, 1.0h),
              gid);
}

/**
 * @brief `intersect_ray_box` tells whether a ray hits a box before `max_distance_m`.
 * @param origin The ray's start.
 * @param inverse_direction One over each component of the ray's direction.
 * @param box The box.
 * @param max_distance_m How far the ray goes.
 * @par Example
 * @code
 * bool is_hit = intersect_ray_box(p, 1.0 / d, boxes[i], 60.0);
 * @endcode
 */
inline bool intersect_ray_box(float3 origin, float3 inverse_direction, OccluderBoxGpu box,
                              float max_distance_m) {
    float3 t0 = (box.min_m.xyz - origin) * inverse_direction;
    float3 t1 = (box.max_m.xyz - origin) * inverse_direction;
    float3 t_near = min(t0, t1);
    float3 t_far = max(t0, t1);
    float t_enter = max(max(t_near.x, t_near.y), max(t_near.z, 0.0));
    float t_exit = min(min(t_far.x, t_far.y), min(t_far.z, max_distance_m));
    return t_enter <= t_exit;
}

/**
 * @brief `bake_sky_visibility` stores, per cell, the cosine-weighted fraction of the upper
 * hemisphere that no occluder box blocks.
 * @param volume The volume to write.
 * @param boxes The occluder boxes.
 * @param params The volume's origin, cell size and counts (`counts.w` is the box count).
 * @param gid The cell.
 * @par Example
 * @code
 * dispatch over (counts.x, counts.y, counts.z)
 * @endcode
 */
kernel void bake_sky_visibility(texture3d<half, access::write> volume [[texture(0)]],
                                device const OccluderBoxGpu* boxes [[buffer(0)]],
                                constant SkyVisibilityParams& params [[buffer(1)]],
                                uint3 gid [[thread_position_in_grid]]) {
    if (any(gid >= params.counts.xyz)) {
        return;
    }
    float3 cell_center = params.origin_m.xyz + (float3(gid) + 0.5) * params.cell_size_m.xyz;
    uint box_count = params.counts.w;
    for (uint b = 0; b < box_count; ++b) {
        if (all(cell_center > boxes[b].min_m.xyz) && all(cell_center < boxes[b].max_m.xyz)) {
            volume.write(half4(0.0h), gid);
            return;
        }
    }
    uint visible_count = 0;
    for (uint i = 0; i < SKY_VISIBILITY_RAY_COUNT; ++i) {
        float2 xi = hammersley(i, SKY_VISIBILITY_RAY_COUNT);
        float phi = 2.0 * PI * xi.y;
        float cos_theta = sqrt(1.0 - xi.x);
        float sin_theta = sqrt(xi.x);
        float3 direction = float3(sin_theta * cos(phi), cos_theta, sin_theta * sin(phi));
        float3 origin = cell_center + direction * SKY_VISIBILITY_RAY_START_M;
        float3 inverse_direction = 1.0 / direction;
        bool is_blocked = false;
        for (uint b = 0; b < box_count && !is_blocked; ++b) {
            is_blocked = intersect_ray_box(origin, inverse_direction, boxes[b],
                                           SKY_VISIBILITY_MAX_DISTANCE_M);
        }
        visible_count += is_blocked ? 0 : 1;
    }
    volume.write(half4(half(float(visible_count) / float(SKY_VISIBILITY_RAY_COUNT))), gid);
}
