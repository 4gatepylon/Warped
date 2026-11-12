/**
 * @file
 * @brief `scene.metal` draws the 3D scene: shadow and depth-normal prepass vertex shaders, the
 * forward PBR shader, the sky, bullet-hole decals and particles.
 *
 * @par Interface
 * - Vertex: `scene_vertex` (static or skinned via `IS_SKINNED`), `sky_vertex`, `decal_vertex`,
 *   `particle_vertex`.
 * - Fragment: `prepass_fragment`, `forward_fragment`, `sky_fragment`, `decal_fragment`,
 *   `particle_fragment`.
 *
 * @par Example
 * @code
 * pipeline(scene_vertex, forward_fragment) draws one mesh with one material
 * @endcode
 */

constant float DECAL_HOLE_RADIUS_FRAC = 0.16;
constant float DECAL_SCORCH_RADIUS_FRAC = 0.48;
constant float DECAL_NORMAL_OFFSET_M = 0.004;
constant float SUN_DISK_RADIANCE_SCALE = 1.0;
constant float SUN_GLOW_STRENGTH = 0.04;

/** @brief `SceneFragmentTextures` are the material textures, bound at slots 0 to 3. */
struct MaterialTextures {
    texture2d<float> base_color [[texture(0)]];
    texture2d<float> normal [[texture(1)]];
    texture2d<float> occlusion_roughness_metal [[texture(2)]];
    texture2d<float> emissive [[texture(3)]];
};

/**
 * @brief `scene_vertex` transforms one vertex by its draw's model matrix, or by its joints when
 * `IS_SKINNED`, and projects it with the pass's view-projection matrix.
 * @par Example
 * @code
 * setVertexBuffer(vertices, 0); setVertexBytes(draw, 3); setVertexBytes(pass, 2)
 * @endcode
 */
vertex SceneVertexOut scene_vertex(uint vertex_id [[vertex_id]],
                                   device const Vertex* vertices [[buffer(0)]],
                                   constant FrameUniforms& frame [[buffer(1)]],
                                   constant PassUniforms& pass [[buffer(2)]],
                                   constant DrawUniforms& draw [[buffer(3)]],
                                   device const SkinVertex* skin_vertices
                                       [[buffer(4), function_constant(IS_SKINNED)]],
                                   device const float4x4* joints
                                       [[buffer(5), function_constant(IS_SKINNED)]]) {
    Vertex in_vertex = vertices[vertex_id];
    float4x4 model = draw.model;
    if (IS_SKINNED) {
        SkinVertex skin = skin_vertices[vertex_id];
        float4 weights = float4(skin.weights);
        model = joints[skin.joints.x] * weights.x + joints[skin.joints.y] * weights.y +
                joints[skin.joints.z] * weights.z + joints[skin.joints.w] * weights.w;
    }
    float4 world_position = model * float4(float3(in_vertex.position), 1.0);
    float3x3 normal_matrix = float3x3(model[0].xyz, model[1].xyz, model[2].xyz);
    SceneVertexOut out;
    out.position = pass.view_proj * world_position;
    out.world_position = world_position.xyz;
    out.normal = normalize(normal_matrix * float3(in_vertex.normal));
    float4 tangent = float4(in_vertex.tangent);
    out.tangent = float4(normalize(normal_matrix * tangent.xyz), tangent.w);
    out.uv = float2(in_vertex.uv);
    return out;
}

/**
 * @brief `prepass_fragment` writes the view-space normal for SSAO.
 * @par Example
 * @code
 * pipeline(scene_vertex, prepass_fragment) with a depth attachment and an RGBA16F target
 * @endcode
 */
fragment float4 prepass_fragment(SceneVertexOut in [[stage_in]],
                                 constant FrameUniforms& frame [[buffer(1)]],
                                 bool is_front_face [[front_facing]]) {
    float3 n = normalize(in.normal) * (is_front_face ? 1.0 : -1.0);
    float3 view_normal = normalize((frame.view * float4(n, 0.0)).xyz);
    return float4(view_normal, 1.0);
}

/** @brief `MATERIAL_MAX_ANISOTROPY` is how many samples along a surface seen at a slant its textures take (Metal's most), so bridge planks and decks seen edge-on don't shimmer (W-138, no aliasing). */
constant int MATERIAL_MAX_ANISOTROPY = 16;

/**
 * @brief `read_surface` samples a material at one fragment; the caller fills in its screen position and SSAO
 * coordinates.
 * @par Example
 * @code
 * Surface surface = read_surface(in, material, textures, is_front_face);
 * @endcode
 */
inline Surface read_surface(SceneVertexOut in, constant MaterialUniforms& material,
                            constant DrawUniforms& draw, MaterialTextures textures,
                            bool is_front_face, thread float& alpha) {
    constexpr sampler material_sampler(coord::normalized, filter::linear, mip_filter::linear,
                                       address::repeat, max_anisotropy(MATERIAL_MAX_ANISOTROPY));
    float4 texel = textures.base_color.sample(material_sampler, in.uv) * material.base_color_factor;
    float4 base_color = texel * draw.tint;
    if (draw.flags.z != 0) {
        float flash = as_type<float>(draw.flags.w);
        base_color = float4(recolor_body(texel.rgb, draw.tint.rgb) * (1.0 + flash * BODY_FLASH_RGB), texel.a);
    }
    float2 normal_xy = textures.normal.sample(material_sampler, in.uv).rg * 2.0 - 1.0;
    float3 normal_tangent = float3(normal_xy, sqrt(saturate(1.0 - dot(normal_xy, normal_xy))));
    float4 orm = textures.occlusion_roughness_metal.sample(material_sampler, in.uv);
    float3 emissive = textures.emissive.sample(material_sampler, in.uv).rgb *
                      material.emissive_factor.rgb;
    float face_sign = is_front_face ? 1.0 : -1.0;
    float3 n = normalize(in.normal) * face_sign;
    float3 t = in.tangent.xyz - n * dot(n, in.tangent.xyz);
    float t_length = length(t);
    if (t_length > 1e-4) {
        t /= t_length;
        float3 b = cross(n, t) * in.tangent.w;
        n = normalize(t * normal_tangent.x + b * normal_tangent.y + n * normal_tangent.z);
    }
    Surface surface;
    surface.world_position = in.world_position;
    surface.normal = n;
    surface.base_color = base_color.rgb;
    surface.roughness = orm.g * material.params.x;
    surface.metallic = orm.b * material.params.y;
    surface.occlusion = material.params.w > 0.5 ? orm.r : 1.0;
    surface.emissive = emissive;
    surface.should_use_ssao = draw.flags.y == 0;
    alpha = base_color.a;
    return surface;
}

// FORWARD_FRAGMENT_PARAMS is here because only forward_fragment and forward_masked_fragment below use it.
#define FORWARD_FRAGMENT_PARAMS                                                                 \
    SceneVertexOut in [[stage_in]], constant FrameUniforms &frame [[buffer(1)]],                \
        constant DrawUniforms &draw [[buffer(3)]],                                              \
        constant MaterialUniforms &material [[buffer(4)]],                                      \
        device const GpuPointLight *point_lights [[buffer(5)]],                                 \
        MaterialTextures material_textures, depth2d_array<float> shadow_map [[texture(4)]],     \
        texture3d<float> sky_visibility [[texture(5)]],                                         \
        texturecube<float> prefiltered [[texture(6)]], texture2d<float> brdf_lut [[texture(7)]], \
        texture2d<float> ssao [[texture(8)]], bool is_front_face [[front_facing]]

/**
 * @brief `shade_forward` shades one forward fragment, optionally cutting out masked texels.
 * @par Example
 * @code
 * return shade_forward(in, frame, draw, material, point_lights, material_textures, textures,
 *                      is_front_face, false);
 * @endcode
 */
inline float4 shade_forward(SceneVertexOut in, constant FrameUniforms& frame,
                            constant DrawUniforms& draw, constant MaterialUniforms& material,
                            device const GpuPointLight* point_lights,
                            MaterialTextures material_textures, SceneTextures textures,
                            bool is_front_face, bool should_alpha_test) {
    float alpha = 1.0;
    Surface surface = read_surface(in, material, draw, material_textures, is_front_face, alpha);
    if (should_alpha_test && alpha < material.params.z) {
        discard_fragment();
    }
    surface.screen_position = get_noise_px(frame, in.position.xy);
    surface.ssao_uv = in.position.xy * frame.foveation.xy;
    float3 color = shade_surface(frame, surface, textures, point_lights);
    color = apply_shadowed_fog(frame, color, in.world_position, textures.shadow_map,
                               textures.sky_visibility, surface.screen_position);
    return float4(color, alpha);
}

/**
 * @brief `forward_fragment` shades one fragment of an opaque or blended mesh.
 * @par Example
 * @code
 * pipeline(scene_vertex, forward_fragment) into the HDR target
 * @endcode
 */
fragment float4 forward_fragment(FORWARD_FRAGMENT_PARAMS) {
    SceneTextures textures = {shadow_map, sky_visibility, prefiltered, brdf_lut, ssao};
    return shade_forward(in, frame, draw, material, point_lights, material_textures, textures,
                         is_front_face, false);
}

/**
 * @brief `forward_masked_fragment` shades one fragment of an alpha-masked mesh, discarding texels
 * below the material's alpha cutoff.
 * @par Example
 * @code
 * pipeline(scene_vertex, forward_masked_fragment) into the HDR target
 * @endcode
 */
fragment float4 forward_masked_fragment(FORWARD_FRAGMENT_PARAMS) {
    SceneTextures textures = {shadow_map, sky_visibility, prefiltered, brdf_lut, ssao};
    return shade_forward(in, frame, draw, material, point_lights, material_textures, textures,
                         is_front_face, true);
}

/**
 * @brief `shadow_mask_fragment` cuts alpha-masked casters out of the shadow map.
 * @par Example
 * @code
 * pipeline(scene_vertex, shadow_mask_fragment) with only a depth attachment
 * @endcode
 */
fragment void shadow_mask_fragment(SceneVertexOut in [[stage_in]],
                                   constant DrawUniforms& draw [[buffer(3)]],
                                   constant MaterialUniforms& material [[buffer(4)]],
                                   texture2d<float> base_color [[texture(0)]]) {
    constexpr sampler material_sampler(coord::normalized, filter::linear, mip_filter::linear,
                                       address::repeat);
    float alpha = base_color.sample(material_sampler, in.uv).a * material.base_color_factor.a *
                  draw.tint.a;
    if (alpha < material.params.z) {
        discard_fragment();
    }
}

/** @brief `SkyVertexOut` is a full-screen triangle's vertex with its clip-space position. */
struct SkyVertexOut {
    float4 position [[position]];
    float2 ndc;
};

/**
 * @brief `sky_vertex` makes a full-screen triangle at the far plane (depth 0 with reverse Z).
 * @par Example
 * @code
 * drawPrimitives(triangle, 0, 3)
 * @endcode
 */
vertex SkyVertexOut sky_vertex(uint vertex_id [[vertex_id]]) {
    float2 ndc = float2((vertex_id << 1) & 2, vertex_id & 2) * 2.0 - 1.0;
    SkyVertexOut out;
    out.position = float4(ndc, 0.0, 1.0);
    out.ndc = ndc;
    return out;
}

/**
 * @brief `sky_fragment` draws the sky cube map, the analytic sun disk, and the horizon fog.
 * @par Example
 * @code
 * pipeline(sky_vertex, sky_fragment) with depth test greater-or-equal at depth 0
 * @endcode
 */
fragment float4 sky_fragment(SkyVertexOut in [[stage_in]],
                             constant FrameUniforms& frame [[buffer(1)]],
                             texturecube<float> environment [[texture(0)]],
                             depth2d_array<float> shadow_map [[texture(4)]],
                             texture3d<float> sky_visibility [[texture(5)]]) {
    constexpr sampler cube_sampler(coord::normalized, filter::linear, mip_filter::linear);
    float4 world = frame.inv_view_proj * float4(in.ndc, 1.0, 1.0);
    float3 direction = normalize(world.xyz / world.w - frame.camera_position_m.xyz);
    float3 color = environment.sample(cube_sampler, direction, level(0.0)).rgb *
                   frame.post_params.z;
    float cos_sun = dot(direction, frame.sun_direction.xyz);
    float sun_radius_rad = frame.sun_direction.w;
    float cos_sun_radius = cos(sun_radius_rad);
    float sun_solid_angle = 2.0 * PI * (1.0 - cos_sun_radius);
    float disk = smoothstep(cos_sun_radius - 1e-4, cos_sun_radius + 1e-4, cos_sun);
    color += frame.sun_illuminance.rgb / sun_solid_angle * disk * SUN_DISK_RADIANCE_SCALE;
    color += frame.sun_illuminance.rgb * SUN_GLOW_STRENGTH * pow(saturate(cos_sun), 600.0);
    float3 far_point = frame.camera_position_m.xyz + direction * FOG_MAX_DISTANCE_M;
    color = apply_shadowed_fog(frame, color, far_point, shadow_map, sky_visibility,
                               get_noise_px(frame, in.position.xy));
    return float4(color, 1.0);
}

/** @brief `DecalVertexOut` is one corner of a decal quad. */
struct DecalVertexOut {
    float4 position [[position]];
    float3 world_position;
    float3 normal;
    float3 tangent;
    float2 local;
    float seed;
};

/**
 * @brief `decal_vertex` places one corner of a bullet-hole quad on its surface.
 * @par Example
 * @code
 * drawPrimitives(triangle_strip, 0, 4, decal_count)
 * @endcode
 */
vertex DecalVertexOut decal_vertex(uint vertex_id [[vertex_id]],
                                   uint instance_id [[instance_id]],
                                   constant FrameUniforms& frame [[buffer(1)]],
                                   device const GpuDecal* decals [[buffer(0)]]) {
    GpuDecal decal = decals[instance_id];
    float2 corner = float2(vertex_id & 1, vertex_id >> 1) * 2.0 - 1.0;
    float3 n = decal.normal_seed.xyz;
    float3 t = decal.tangent.xyz;
    float3 b = cross(n, t);
    float half_size_m = decal.position_size.w * 0.5;
    float3 world_position = decal.position_size.xyz + n * DECAL_NORMAL_OFFSET_M +
                            (t * corner.x + b * corner.y) * half_size_m;
    DecalVertexOut out;
    out.position = frame.view_proj * float4(world_position, 1.0);
    out.world_position = world_position;
    out.normal = n;
    out.tangent = t;
    out.local = corner;
    out.seed = decal.normal_seed.w;
    return out;
}

/**
 * @brief `decal_fragment` shades a bullet hole: a dark hole, a scorched ring and chipped edges,
 * lit like the surface it sits on.
 * @par Example
 * @code
 * pipeline(decal_vertex, decal_fragment) with alpha blending and no depth write
 * @endcode
 */
fragment float4 decal_fragment(DecalVertexOut in [[stage_in]],
                               constant FrameUniforms& frame [[buffer(1)]],
                               device const GpuPointLight* point_lights [[buffer(5)]],
                               depth2d_array<float> shadow_map [[texture(4)]],
                               texture3d<float> sky_visibility [[texture(5)]],
                               texturecube<float> prefiltered [[texture(6)]],
                               texture2d<float> brdf_lut [[texture(7)]],
                               texture2d<float> ssao [[texture(8)]]) {
    float radius = length(in.local);
    float angle = atan2(in.local.y, in.local.x);
    float jagged = 0.08 * sin(angle * 7.0 + in.seed * 40.0) + 0.05 * sin(angle * 13.0 + in.seed * 17.0);
    float hole = 1.0 - smoothstep(DECAL_HOLE_RADIUS_FRAC * 0.7, DECAL_HOLE_RADIUS_FRAC + jagged * 0.3, radius);
    float scorch = 1.0 - smoothstep(DECAL_HOLE_RADIUS_FRAC, DECAL_SCORCH_RADIUS_FRAC + jagged, radius);
    float cracks = smoothstep(0.92, 1.0, sin(angle * 9.0 + in.seed * 23.0)) *
                   (1.0 - smoothstep(0.2, 0.9, radius));
    float alpha = saturate(max(max(hole, scorch * 0.85), cracks * 0.6));
    if (alpha < 0.01) {
        discard_fragment();
    }
    float3 b = cross(in.normal, in.tangent);
    float rim = smoothstep(DECAL_HOLE_RADIUS_FRAC * 0.6, DECAL_HOLE_RADIUS_FRAC * 1.4, radius) *
                (1.0 - smoothstep(DECAL_HOLE_RADIUS_FRAC * 1.4, DECAL_SCORCH_RADIUS_FRAC, radius));
    float2 radial = radius > 1e-3 ? in.local / radius : float2(0.0);
    float3 n = normalize(in.normal + (in.tangent * radial.x + b * radial.y) * rim * 0.8);
    Surface surface;
    surface.world_position = in.world_position;
    surface.normal = n;
    surface.base_color = mix(float3(0.06, 0.055, 0.05), float3(0.008), hole);
    surface.roughness = 0.85;
    surface.metallic = 0.0;
    surface.occlusion = 1.0 - hole * 0.9;
    surface.emissive = 0.0;
    surface.screen_position = get_noise_px(frame, in.position.xy);
    surface.ssao_uv = in.position.xy * frame.foveation.xy;
    surface.should_use_ssao = true;
    SceneTextures textures = {shadow_map, sky_visibility, prefiltered, brdf_lut, ssao};
    float3 color = shade_surface(frame, surface, textures, point_lights);
    color = apply_shadowed_fog(frame, color, in.world_position, textures.shadow_map,
                               textures.sky_visibility, surface.screen_position);
    return float4(color, alpha);
}

/** @brief `ParticleVertexOut` is one corner of a particle sprite. */
struct ParticleVertexOut {
    float4 position [[position]];
    float2 local;
    float4 color;
    uint kind [[flat]];
    float seed;
};

/**
 * @brief `particle_vertex` makes a camera-facing sprite, stretched along the particle's motion.
 * @par Example
 * @code
 * drawPrimitives(triangle_strip, 0, 4, particle_count)
 * @endcode
 */
vertex ParticleVertexOut particle_vertex(uint vertex_id [[vertex_id]],
                                         uint instance_id [[instance_id]],
                                         constant FrameUniforms& frame [[buffer(1)]],
                                         constant PassUniforms& pass [[buffer(2)]],
                                         device const GpuParticle* particles [[buffer(0)]]) {
    GpuParticle particle = particles[instance_id];
    float2 corner = float2(vertex_id & 1, vertex_id >> 1) * 2.0 - 1.0;
    float3 center = particle.position_size.xyz;
    float3 to_camera = normalize(frame.camera_position_m.xyz - center);
    float3 stretch = particle.stretch_kind.xyz;
    float stretch_length_m = length(stretch);
    float size_m = particle.position_size.w;
    float3 axis_u;
    float3 axis_v;
    if (stretch_length_m > 1e-4) {
        axis_u = stretch / stretch_length_m;
        axis_v = normalize(cross(axis_u, to_camera));
        axis_u *= stretch_length_m * 0.5 + size_m;
        axis_v *= size_m;
    } else {
        float rotation = particle.seed.x * 2.0 * PI;
        float3 up = abs(to_camera.y) < 0.99 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
        float3 right = normalize(cross(up, to_camera));
        float3 camera_up = cross(to_camera, right);
        axis_u = (right * cos(rotation) + camera_up * sin(rotation)) * size_m;
        axis_v = (camera_up * cos(rotation) - right * sin(rotation)) * size_m;
    }
    float3 world_position = center + axis_u * corner.x + axis_v * corner.y;
    ParticleVertexOut out;
    out.position = pass.view_proj * float4(world_position, 1.0);
    out.local = corner;
    out.color = particle.color;
    out.kind = uint(particle.stretch_kind.w);
    out.seed = particle.seed.x;
    return out;
}

/**
 * @brief A glowing ball (particle kind 4, a thrown wormhole's marker) is solid out to between `GLOW_BALL_EDGE.x` and
 * `.y` of the sprite's half width, in a halo falling off as exp(−`GLOW_HALO_FALLOFF` r²) at `GLOW_HALO_FRAC` of its
 * brightness that fades to nothing between `GLOW_FADE.x` and `.y`, so the sprite's square edge never shows.
 */
constant float2 GLOW_BALL_EDGE = float2(0.42, 0.5);
constant float GLOW_HALO_FALLOFF = 5.0;
constant float GLOW_HALO_FRAC = 0.5;
constant float2 GLOW_FADE = float2(0.6, 1.0);

/**
 * @brief `particle_fragment` shades sparks, smoke, muzzle flashes, tracers and glowing balls procedurally.
 * Additive kinds return alpha 0 so that one premultiplied blend state serves every kind.
 * @par Example
 * @code
 * pipeline(particle_vertex, particle_fragment) with premultiplied alpha blending
 * @endcode
 */
fragment float4 particle_fragment(ParticleVertexOut in [[stage_in]],
                                  constant FrameUniforms& frame [[buffer(1)]]) {
    float radius = length(in.local);
    float4 color = in.color;
    if (in.kind == 1) {
        float puff = (1.0 - smoothstep(0.2, 1.0, radius)) *
                     (0.75 + 0.25 * sin(in.local.x * 5.0 + in.seed * 30.0) *
                                 sin(in.local.y * 4.0 + in.seed * 11.0));
        float3 lit = color.rgb * (evaluate_sh9(frame.sh9, float3(0.0, 1.0, 0.0)) *
                                      frame.post_params.z +
                                  frame.sun_illuminance.rgb * 0.15);
        float alpha = color.a * puff;
        return float4(lit * alpha, alpha);
    }
    if (in.kind == 2) {
        float angle = atan2(in.local.y, in.local.x);
        float star = pow(abs(cos(angle * 2.0 + in.seed * 6.0)), 18.0) * (1.0 - radius) +
                     pow(abs(cos(angle * 3.0 + in.seed * 3.0)), 30.0) * (1.0 - radius) * 0.6;
        float core = exp(-radius * radius * 9.0);
        float intensity = saturate(core + star) * (1.0 - smoothstep(0.85, 1.0, radius));
        return float4(color.rgb * intensity, 0.0);
    }
    if (in.kind == 4) {
        float ball = 1.0 - smoothstep(GLOW_BALL_EDGE.x, GLOW_BALL_EDGE.y, radius);
        float halo = exp(-radius * radius * GLOW_HALO_FALLOFF) * (1.0 - smoothstep(GLOW_FADE.x, GLOW_FADE.y, radius));
        return float4(color.rgb * max(ball, halo * GLOW_HALO_FRAC), 0.0);
    }
    float glow = exp(-dot(in.local, in.local) * (in.kind == 3 ? 6.0 : 4.0));
    if (in.kind == 3) {
        glow *= exp(-in.local.y * in.local.y * 8.0);
    }
    return float4(color.rgb * glow, 0.0);
}
