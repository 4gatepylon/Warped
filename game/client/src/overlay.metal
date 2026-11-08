/**
 * @file
 * @brief `overlay.metal` draws over a finished frame on the GPU, one thread per output pixel: motion
 * blur, the wormhole blueshift, the hit effect, text, pictures, the wand or a gun model, the minimap
 * and the HUD's rectangles.
 *
 * @par Interface
 * - Kernel `apply_overlay`: the buffers and textures `render_metal.rs`'s `encode_overlay` sets.
 *
 * @par Example
 * @code
 * encoder.setComputePipelineState(&self.apply_overlay);
 * @endcode
 */

#include <metal_stdlib>
using namespace metal;

constant float OVERLAY_TITLE_LEVEL = 0.75;
constant float OVERLAY_TEXT_LEVEL = 0.25;
constant float4 OVERLAY_TITLE_COLOR = float4(1.0, 0.35, 0.3, 1.0);
constant float4 OVERLAY_TEXT_COLOR = float4(0.92, 0.94, 0.98, 1.0);
constant float3 HIT_RED_COLOR = float3(0.85, 0.05, 0.03);
constant float HIT_EDGE_REACH_FRAC = 0.7;
constant int HIT_BLUR_TAP_COUNT = 8;
constant float3 WAND_LIGHT_DIRECTION = float3(-0.42, 0.78, 0.46);
constant float WAND_AMBIENT = 0.3;
constant float WAND_SPECULAR = 0.45;
constant float WAND_SHININESS = 28.0;
constant float WAND_EDGE_M = 0.003;
constant float WAND_EDGE_DARKEN = 0.55;
constant float WAND_TIP_GLOW = 1.25;
constant float WAND_HALO_M = 0.012;
constant float WAND_HALO_STRENGTH = 0.6;
constant int WAND_SAMPLES_PER_SIDE = 2;
constant int REVOLVER_WAND = 2;
constant int STRETCH_TAP_COUNT = 12;
constant float STRETCH_ZOOM_GAIN = 1.5;
constant float STRETCH_SMEAR_FRAC = 0.35;
constant float STRETCH_FRINGE_FRAC = 0.03;
constant float3 BLUESHIFT_CHANNEL_NM = float3(610.0, 550.0, 465.0);
constant float BLUESHIFT_INFRARED_NM = 700.0;
constant float BLUESHIFT_ULTRAVIOLET_NM = 400.0;
constant float BLUESHIFT_WALK_SPEED_PER_LIGHT_SPEED = 0.09;
constant float REDSHIFT_WALK_SPEED_PER_LIGHT_SPEED = 0.3;
constant float BLUESHIFT_ANGLE_FRAC = 0.5;
constant float BLUESHIFT_BEAMING_POWER = 2.0;
constant float REDSHIFT_BEAMING_POWER = 1.0;
constant float3 BLUESHIFT_GLOW_COLOR = float3(0.0, 0.12, 0.4);
constant float3 REDSHIFT_GLOW_COLOR = float3(0.4, 0.04, 0.0);
constant float3 BLUESHIFT_LUMA_PER_CHANNEL = float3(0.2126, 0.7152, 0.0722);
constant float BLUESHIFT_PALE_FRAC = 0.3;
constant float BLUESHIFT_WHITE_FRAC = 0.6;
constant float3 BLUESHIFT_TINT = float3(224.0 / 255.0, 1.0, 1.0);
constant float BLUESHIFT_GAIN = 1.5;
constant float BLUESHIFT_SHOULDER_KNEE = 0.8;
constant float BLUESHIFT_LIFT_FRAC = 0.0;
constant float BLUESHIFT_MAX_GREEN_PER_BLUE = 0.95;
constant float REDSHIFT_GAIN = 0.7;
constant int MAX_MINIMAP_MARK_COUNT = 16;
constant int MAX_MINIMAP_LINE_COUNT = 96;
constant float MINIMAP_OPACITY = 0.88;
constant float MINIMAP_RIM_PX = 2.0;
constant float4 MINIMAP_RIM_COLOR = float4(0.9, 0.92, 0.95, 0.9);
constant float3 MINIMAP_OFF_MAP_COLOR = float3(0.03, 0.03, 0.04);
constant float MINIMAP_GRID_M = 10.0;
constant float MINIMAP_GRID_HALF_WIDTH_PX = 0.6;
constant float4 MINIMAP_GRID_COLOR = float4(1.0, 1.0, 1.0, 0.16);
constant float MINIMAP_LINE_HALF_WIDTH_PX = 1.2;
constant float MINIMAP_YOU_PX = 14.0;
constant float MINIMAP_YOU_BASE_FRAC = 0.6;
constant float MINIMAP_YOU_SPREAD = 0.5;
constant float3 MINIMAP_YOU_COLOR = float3(1.0, 1.0, 1.0);
constant float MINIMAP_TINY = 1e-6;
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
 * @brief `OverlayParams` is how `apply_overlay` draws: how much to darken, mask pixels to screen pixels, the mask's top
 * left, how many rectangles there are, whether there is a mask and a wand, the camera's tan(half vertical fov), how
 * much of the previous frame's scene to blend in (motion blur), and how far the scene blueshifts inside a wormhole
 * mouth (`blueshift_scene`: 1 for walking straight ahead at the throat, negative for a redshift, 0 for none).
 */
struct OverlayParams {
    float dim_frac;
    int scale;
    int2 origin;
    int rect_count;
    int has_mask;
    int has_wand;
    float tan_half_fov;
    float blur_frac;
    float hit_frac;
    float hit_red_frac;
    float hit_blur_px;
    float2 shake_px;
    int2 picture_min_mask_px;
    int2 picture_max_excl_mask_px;
    float stretch_frac;
    float blueshift_frac;
};

/**
 * @brief `sample_spectrum` gives a colour's light at wavelength `nm`, reading its red, green and blue as samples of a
 * spectrum at `BLUESHIFT_CHANNEL_NM` joined by straight lines, falling to 0 at the eye's limits,
 * `BLUESHIFT_ULTRAVIOLET_NM` and `BLUESHIFT_INFRARED_NM` (a picture holds no light beyond them for a shift to bring in).
 * @par Example
 * @code
 * float red_seen = sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.r * D);
 * @endcode
 */
static float sample_spectrum(float3 rgb, float nm) {
    float3 at_nm = BLUESHIFT_CHANNEL_NM;
    if (nm <= at_nm.b) return rgb.b * saturate((nm - BLUESHIFT_ULTRAVIOLET_NM) / (at_nm.b - BLUESHIFT_ULTRAVIOLET_NM));
    if (nm <= at_nm.g) return mix(rgb.b, rgb.g, (nm - at_nm.b) / (at_nm.g - at_nm.b));
    if (nm <= at_nm.r) return mix(rgb.g, rgb.r, (nm - at_nm.g) / (at_nm.r - at_nm.g));
    return rgb.r * saturate((BLUESHIFT_INFRARED_NM - nm) / (BLUESHIFT_INFRARED_NM - at_nm.r));
}

/**
 * @brief `roll_off_shoulder` scales a colour so its brightest channel, above `BLUESHIFT_SHOULDER_KNEE`, rolls off
 * smoothly toward 1 without reaching it (knee + (1 - knee)(1 - e^(-(m - knee) / (1 - knee))) for a brightest channel
 * m), which keeps the hue and texture of bright pixels instead of clipping them to flat white.
 * @par Example
 * @code
 * float3 kept = roll_off_shoulder(rgb * BLUESHIFT_GAIN);
 * @endcode
 */
static float3 roll_off_shoulder(float3 rgb) {
    float brightest = max(rgb.r, max(rgb.g, rgb.b));
    if (brightest <= BLUESHIFT_SHOULDER_KNEE) return rgb;
    float span = 1.0 - BLUESHIFT_SHOULDER_KNEE;
    float rolled = BLUESHIFT_SHOULDER_KNEE + span * (1.0 - exp(-(brightest - BLUESHIFT_SHOULDER_KNEE) / span));
    return rgb * (rolled / brightest);
}

/**
 * @brief `blueshift_scene` gives a pixel's colour as seen moving along the view, `blueshift_frac` giving how fast
 * (1 for walking at a mouth's throat; negative backward; 0 for none). The relativistic Doppler factor
 * D = sqrt(1 - β²) / (1 - β cos θ), with θ the angle between the pixel's ray and straight ahead, makes each channel
 * show the light the scene sends at D times its wavelength (`sample_spectrum`), D^power times as bright (relativistic
 * beaming), plus a glow times how far that gain is from 1, so the black sky turns too.
 * - Forward (option 8, "toward light cyan", the user's pick): red fades and green takes red's light, with
 *   `BLUESHIFT_BEAMING_POWER` and `BLUESHIFT_GLOW_COLOR`. The colour then pales `BLUESHIFT_PALE_FRAC` of the way
 *   toward its grey, moves `BLUESHIFT_WHITE_FRAC` of the way toward `BLUESHIFT_TINT` at the same luminance,
 *   brightens by `BLUESHIFT_GAIN` through `roll_off_shoulder`, and lifts `BLUESHIFT_LIFT_FRAC` of the way toward the
 *   tint. Last, green moves into blue and is held to `BLUESHIFT_MAX_GREEN_PER_BLUE` of it, so nothing turns green.
 * - Backward (R2, "darker", the user's pick): blue fades and greys turn red, with `REDSHIFT_BEAMING_POWER` and
 *   `REDSHIFT_GLOW_COLOR`, then all of it darkens by `REDSHIFT_GAIN`.
 * - Speed: β is `BLUESHIFT_WALK_SPEED_PER_LIGHT_SPEED` (backward, `REDSHIFT_WALK_SPEED_PER_LIGHT_SPEED`) at
 *   |`blueshift_frac`| = 1 and grows with it beyond; below 1, the shifted colour fades in by |`blueshift_frac`|
 *   instead, since a smaller shift would first turn greys greenish (or yellowish).
 * - Angle: θ keeps only `BLUESHIFT_ANGLE_FRAC` of its effect (cos θ is taken that much of the way from 1), so the
 *   corners shift nearly as much as the middle.
 * @par Example
 * @code
 * float3 seen = blueshift_scene(scene, id, float2(out.get_width(), out.get_height()), p);
 * @endcode
 * @par Must know
 * - Limitations:
 *   - `blueshift_scene` takes the motion to be along the view, wherever the player really goes.
 *   - Real beaming changes brightness by D³ or more; the gentler powers keep scenes from washing out.
 */
static float3 blueshift_scene(float3 rgb, uint2 id, float2 size, constant OverlayParams &p) {
    if (p.blueshift_frac == 0.0) return rgb;
    bool is_forward = p.blueshift_frac > 0.0;
    float2 ndc = 2.0 * (float2(id) + 0.5) / size - 1.0;
    float3 ray = normalize(float3(ndc.x * p.tan_half_fov * size.x / size.y, ndc.y * p.tan_half_fov, -1.0));
    float speed_frac = abs(p.blueshift_frac);
    float fade_frac = min(speed_frac, 1.0);
    float walk_beta = is_forward ? BLUESHIFT_WALK_SPEED_PER_LIGHT_SPEED : -REDSHIFT_WALK_SPEED_PER_LIGHT_SPEED;
    float beta = walk_beta * max(speed_frac, 1.0);
    float D = sqrt(1.0 - beta * beta) / (1.0 - beta * mix(1.0, -ray.z, BLUESHIFT_ANGLE_FRAC));
    float3 shifted = float3(sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.r * D), sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.g * D),
                            sample_spectrum(rgb, BLUESHIFT_CHANNEL_NM.b * D));
    if (!is_forward) {
        float beaming = pow(D, REDSHIFT_BEAMING_POWER);
        float3 seen = (shifted * beaming + REDSHIFT_GLOW_COLOR * abs(beaming - 1.0)) * REDSHIFT_GAIN;
        return mix(rgb, seen, fade_frac);
    }
    float beaming = pow(D, BLUESHIFT_BEAMING_POWER);
    float3 seen = shifted * beaming + BLUESHIFT_GLOW_COLOR * abs(beaming - 1.0);
    seen = mix(seen, float3(dot(seen, BLUESHIFT_LUMA_PER_CHANNEL)), BLUESHIFT_PALE_FRAC);
    float3 tint_per_luma = BLUESHIFT_TINT / dot(BLUESHIFT_TINT, BLUESHIFT_LUMA_PER_CHANNEL);
    seen = mix(seen, dot(seen, BLUESHIFT_LUMA_PER_CHANNEL) * tint_per_luma, BLUESHIFT_WHITE_FRAC);
    seen = roll_off_shoulder(seen * BLUESHIFT_GAIN);
    seen = mix(seen, BLUESHIFT_TINT, BLUESHIFT_LIFT_FRAC);
    float3 faded = mix(rgb, seen, fade_frac);
    faded.b = mix(faded.b, max(faded.b, faded.g), fade_frac);
    faded.g = mix(faded.g, min(faded.g, BLUESHIFT_MAX_GREEN_PER_BLUE * faded.b), fade_frac);
    return faded;
}

/**
 * @brief `read_hit_scene` reads a frame's pixel as a player who was just hit sees it (`hit_frac` of the effect left, 1
 * just after the hit): shifted by `shake_px`, and toward the edges blurred over `hit_blur_px` and turned red by
 * `hit_red_frac`, reaching further in the fresher the hit; with no hit left, the pixel as it is.
 * @par Example
 * @code
 * float3 scene = read_hit_scene(frame, id, p);
 * @endcode
 */
static float3 read_hit_scene(texture2d<float, access::read> frame, uint2 id, constant OverlayParams &p) {
    int2 size = int2(frame.get_width(), frame.get_height());
    int2 at = clamp(int2(id) + int2(round(p.shake_px)), int2(0), size - 1);
    float3 sharp = frame.read(uint2(at)).rgb;
    if (p.hit_frac <= 0.0) return sharp;
    float corner = length((float2(id) + 0.5) / float2(size) - 0.5) * M_SQRT2_F;
    float edge = p.hit_frac * smoothstep(1.0 - HIT_EDGE_REACH_FRAC * p.hit_frac, 1.0, corner);
    float3 sum = sharp;
    for (int k = 0; k < HIT_BLUR_TAP_COUNT; k++) {
        float angle = 2.0 * M_PI_F * float(k) / float(HIT_BLUR_TAP_COUNT);
        int2 tap = at + int2(round(p.hit_blur_px * edge * float2(cos(angle), sin(angle))));
        sum += frame.read(uint2(clamp(tap, int2(0), size - 1))).rgb;
    }
    float3 blurred = mix(sharp, sum / float(HIT_BLUR_TAP_COUNT + 1), edge);
    return mix(blurred, HIT_RED_COLOR, edge * p.hit_red_frac);
}

/**
 * @brief `read_stretched_scene` reads a frame's pixel as a player slowed at the map's soft edge sees it (`stretch_frac`
 * of the way to its stop, 0 for none): the scene pulled out from the middle of the screen (magnified up to
 * 1 + `STRETCH_ZOOM_GAIN` times at the middle, not at all at the corners) and smeared along lines from the middle over
 * `STRETCH_SMEAR_FRAC` of the way in (`STRETCH_TAP_COUNT` taps), red pulled a little further and blue a little less
 * (`STRETCH_FRINGE_FRAC`); each tap is read as `read_hit_scene` reads it.
 * @par Example
 * @code
 * float3 scene = read_stretched_scene(frame, id, p);
 * @endcode
 */
static float3 read_stretched_scene(texture2d<float, access::read> frame, uint2 id, constant OverlayParams &p) {
    if (p.stretch_frac <= 0.0) return read_hit_scene(frame, id, p);
    float2 size = float2(frame.get_width(), frame.get_height());
    float2 middle = size * 0.5;
    float2 offset = float2(id) + 0.5 - middle;
    float zoom = pow(1.0 + STRETCH_ZOOM_GAIN * p.stretch_frac, 1.0 - length(offset) / length(middle));
    float fringe = STRETCH_FRINGE_FRAC * p.stretch_frac;
    float3 sum = float3(0.0);
    for (int k = 0; k < STRETCH_TAP_COUNT; k++) {
        float pull = 1.0 - STRETCH_SMEAR_FRAC * p.stretch_frac * float(k) / float(STRETCH_TAP_COUNT - 1);
        float3 pulls = pull * float3(1.0 + fringe, 1.0, 1.0 - fringe);
        for (int c = 0; c < 3; c++) {
            float2 source = clamp(middle + offset / zoom * pulls[c], float2(0.0), size - 1.0);
            sum[c] += read_hit_scene(frame, uint2(source), p)[c];
        }
    }
    return sum / float(STRETCH_TAP_COUNT);
}

/**
 * @brief `OverlayWand` is the wand in camera space (x right, y up, -z ahead, meters): its shaft's centre, its axes (its
 * length along its own -z), its shaft's half sizes and colour, and its tip's half sizes and colour (the tip sits at the
 * shaft's front end).
 */
struct OverlayWand {
    float4 center_m;
    float4 axes[3];
    float4 shaft_half_m;
    float4 shaft_color;
    float4 tip_half_m;
    float4 tip_color;
};

/**
 * @brief `hit_box` gives the distance along a ray from the eye (unit direction `d`) to a box (centre, axes, half sizes),
 * or -1 if it misses, with the hit face's normal and how far the hit is from that face's nearest edge.
 * @par Example
 * @code
 * float t = hit_box(d, center, axes, half_m, normal, edge_m);
 * @endcode
 */
static float hit_box(float3 d, float3 center, float3x3 axes, float3 half_m, thread float3 &normal, thread float &edge_m) {
    float3 o = transpose(axes) * -center, ld = transpose(axes) * d;
    float3 inv = 1.0 / select(ld, float3(1e-7), abs(ld) < 1e-7);
    float3 t0 = (-half_m - o) * inv, t1 = (half_m - o) * inv;
    float3 t_lo = min(t0, t1), t_hi = max(t0, t1);
    float t_near = max(max(t_lo.x, t_lo.y), t_lo.z), t_far = min(min(t_hi.x, t_hi.y), t_hi.z);
    if (t_near > t_far || t_near < 0.0) return -1.0;
    float3 face = select(float3(0.0), -sign(ld), t_lo == t_near);
    float3 gap = half_m - abs(o + t_near * ld);
    edge_m = min(min(face.x != 0.0 ? 1e3 : gap.x, face.y != 0.0 ? 1e3 : gap.y), face.z != 0.0 ? 1e3 : gap.z);
    normal = axes * face;
    return t_near;
}

/**
 * @brief `shade_wand` gives the wand's colour and coverage (0 or 1) along one ray from the eye: a lit, dark shaft with a
 * highlight and darkened edges, and a glowing tip; past them, the tip's halo with coverage 0.
 * @par Example
 * @code
 * float4 c = shade_wand(d, wand);
 * @endcode
 */
static float4 shade_wand(float3 d, constant OverlayWand &wand) {
    float3x3 axes = float3x3(wand.axes[0].xyz, wand.axes[1].xyz, wand.axes[2].xyz);
    float3 tip_center = wand.center_m.xyz + axes * float3(0.0, 0.0, -(wand.shaft_half_m.z + wand.tip_half_m.z));
    float3 shaft_normal, tip_normal;
    float shaft_edge_m, tip_edge_m;
    float t_shaft = hit_box(d, wand.center_m.xyz, axes, wand.shaft_half_m.xyz, shaft_normal, shaft_edge_m);
    float t_tip = hit_box(d, tip_center, axes, wand.tip_half_m.xyz, tip_normal, tip_edge_m);
    float3 light = normalize(WAND_LIGHT_DIRECTION);
    if (t_tip > 0.0 && (t_shaft < 0.0 || t_tip < t_shaft)) {
        return float4(wand.tip_color.rgb * (WAND_TIP_GLOW + WAND_AMBIENT * max(dot(tip_normal, light), 0.0)), 1.0);
    }
    if (t_shaft > 0.0) {
        float diffuse = max(dot(shaft_normal, light), 0.0);
        float highlight = pow(max(dot(reflect(-light, shaft_normal), -d), 0.0), WAND_SHININESS);
        float edge = mix(WAND_EDGE_DARKEN, 1.0, smoothstep(0.0, WAND_EDGE_M, shaft_edge_m));
        return float4(wand.shaft_color.rgb * (WAND_AMBIENT + diffuse) * edge + WAND_SPECULAR * highlight, 1.0);
    }
    float miss_m = length(cross(d, tip_center));
    return float4(wand.tip_color.rgb * WAND_HALO_STRENGTH * exp(-miss_m / WAND_HALO_M), 0.0);
}

/** @brief `MinimapMark` is a mark on the minimap: a point in its unit disk (x right, y down), a radius in pixels and a zero; and a colour. */
struct MinimapMark {
    float4 disk;
    float4 color;
};

/** @brief `MinimapLine` is a line on the minimap: its two ends in the unit disk (x right, y down), and a colour. */
struct MinimapLine {
    float4 ends;
    float4 color;
};

/**
 * @brief `Minimap` is render_metal.rs's `Minimap`: the frame taking the minimap's plane (y = 0, −z up the minimap) into
 * the world; its centre and radius in pixels and the Poincaré disk's radius at its rim; the map picture's top left corner
 * (Fermi u, v), meters per pixel and the curvature radius; the counts of marks and lines; the marks and the lines.
 */
struct Minimap {
    float4x4 ground_frame;
    float4 placement;
    float4 map;
    int4 counts;
    MinimapMark marks[MAX_MINIMAP_MARK_COUNT];
    MinimapLine lines[MAX_MINIMAP_LINE_COUNT];
};

/**
 * @brief `draw_minimap` draws the minimap over `color` at pixel `id`: a disk of radius `placement.z` around
 * `placement.xy`, a Poincaré disk of the ground around the player (out to `placement.w`, tanh of half its range in
 * curvature radii), so geodesics away from the player bend into arcs. Each pixel's disk point goes onto the hyperboloid,
 * through `ground_frame` into the world, to its Fermi (u, v) and so to the map's picture; then come faint lines every
 * `MINIMAP_GRID_M` of u and v (as wide as a pixel, from the disk's metric 2R / (1 − |q|²)), the shots' lines, the other
 * players' marks, and a white triangle for the player pointing up; inside a rim.
 * @par Example
 * @code
 * color.rgb = draw_minimap(color.rgb, id, minimap, map);
 * @endcode
 */
static float3 draw_minimap(float3 color, uint2 id, constant Minimap &m, texture2d<float, access::read> map) {
    float radius_px = m.placement.z;
    if (radius_px <= 0.0) return color;
    float2 offset_px = float2(id) + 0.5 - m.placement.xy;
    float distance_px = length(offset_px);
    if (distance_px > radius_px + MINIMAP_RIM_PX) return color;
    if (distance_px > radius_px) return mix(color, MINIMAP_RIM_COLOR.rgb, MINIMAP_RIM_COLOR.a);
    float2 q = offset_px / radius_px * m.placement.w;
    float qq = dot(q, q);
    float4 local = float4(2.0 * q.x, 0.0, 2.0 * q.y, 1.0 + qq) / (1.0 - qq);
    float4 x = m.ground_frame * local;
    float r = m.map.w;
    float2 fermi_m = float2(-r * atanh(x.z / x.w), r * asinh(x.x / sqrt(1.0 + x.y * x.y)));
    float2 at_px = (fermi_m - m.map.xy) / m.map.z;
    float3 ground = MINIMAP_OFF_MAP_COLOR;
    if (all(at_px >= 0.0) && all(at_px < float2(map.get_width(), map.get_height()))) ground = map.read(uint2(at_px)).rgb;
    float m_per_px = 2.0 * r * m.placement.w / radius_px / (1.0 - qq);
    float2 to_grid_m = abs(fract(fermi_m / MINIMAP_GRID_M + 0.5) - 0.5) * MINIMAP_GRID_M;
    if (min(to_grid_m.x, to_grid_m.y) < MINIMAP_GRID_HALF_WIDTH_PX * m_per_px) {
        ground = mix(ground, MINIMAP_GRID_COLOR.rgb, MINIMAP_GRID_COLOR.a);
    }
    for (int i = 0; i < m.counts.y; i++) {
        float2 a = m.lines[i].ends.xy * radius_px;
        float2 ab = m.lines[i].ends.zw * radius_px - a;
        float t = clamp(dot(offset_px - a, ab) / max(dot(ab, ab), MINIMAP_TINY), 0.0, 1.0);
        if (length(offset_px - a - t * ab) < MINIMAP_LINE_HALF_WIDTH_PX) ground = m.lines[i].color.rgb;
    }
    for (int i = 0; i < m.counts.x; i++) {
        if (length(offset_px - m.marks[i].disk.xy * radius_px) < m.marks[i].disk.z) ground = m.marks[i].color.rgb;
    }
    float2 you = offset_px / MINIMAP_YOU_PX;
    if (you.y > -1.0 && you.y < MINIMAP_YOU_BASE_FRAC && abs(you.x) < (you.y + 1.0) * MINIMAP_YOU_SPREAD) {
        ground = MINIMAP_YOU_COLOR;
    }
    return mix(color, ground, MINIMAP_OPACITY);
}

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
 * @brief `apply_overlay` copies a finished frame into `out`, blended with the previous frame's scene (`history_in`, by
 * `blur_frac`; the blend is kept in `history_out` for the next frame), blueshifted (`blueshift_scene`) and darkened by `dim_frac`, with a mask's text drawn over it at
 * `scale` screen pixels per mask pixel from `origin` (the title's colour where the mask is above `OVERLAY_TITLE_LEVEL`,
 * the text's above `OVERLAY_TEXT_LEVEL`) over a picture (blended by its alpha) stretched from `picture_min_mask_px` to
 * `picture_max_excl_mask_px` in mask pixels (none when that box is empty), the wand (`shade_wand`, 2 × 2 rays a pixel)
 * when `has_wand` is set (or the revolver from viewmodel.rs, `revolver`, when it is `REVOLVER_WAND`), the minimap
 * (`draw_minimap`, from `minimap` and the map's picture `minimap_map`), and `rects` over that: first all their shadows
 * and glows (`get_rect_halo`, the strongest of each), then the rectangles themselves blended in order (`get_rect_layer`),
 * then the bars' pulses (`get_pulse_glow`: tinting toward their colour and adding `RECT_PULSE_LIGHT_GAIN` of it as
 * light) (render_metal.rs's `Overlay`: e.g. "YOU DIED" and a countdown, and the
 * energy bar).
 * @par Example
 * @code
 * dispatched by render_metal.rs, one thread per output pixel.
 * @endcode
 */
kernel void apply_overlay(texture2d<float, access::read> frame [[texture(0)]], texture2d<float, access::read> revolver [[texture(5)]], texture2d<float, access::read> mask [[texture(1)]],
                          texture2d<float, access::write> out [[texture(2)]], constant OverlayParams &p [[buffer(0)]],
                          constant OverlayRect *rects [[buffer(1)]], constant OverlayWand &wand [[buffer(2)]],
                          texture2d<float, access::read> history_in [[texture(3)]],
                          texture2d<float, access::write> history_out [[texture(4)]],
                          texture2d<float, access::read> picture [[texture(6)]], constant Minimap &minimap [[buffer(3)]],
                          texture2d<float, access::read> minimap_map [[texture(7)]], uint2 id [[thread_position_in_grid]]) {
    if (id.x >= out.get_width() || id.y >= out.get_height()) return;
    float3 scene = mix(read_stretched_scene(frame, id, p), history_in.read(id).rgb, p.blur_frac);
    history_out.write(float4(scene, 1.0), id);
    float4 color = float4(blueshift_scene(scene, id, float2(out.get_width(), out.get_height()), p) * (1.0 - p.dim_frac), 1.0);
    int2 offset = int2(id) - p.origin;
    int2 picture_size_mask_px = p.picture_max_excl_mask_px - p.picture_min_mask_px;
    if (all(picture_size_mask_px > int2(0)) && all(offset >= int2(0))) {
        float2 picture_frac = (float2(offset) / float(p.scale) - float2(p.picture_min_mask_px)) / float2(picture_size_mask_px);
        if (all(picture_frac >= 0.0) && all(picture_frac < 1.0)) {
            float4 pixel = picture.read(uint2(picture_frac * float2(picture.get_width(), picture.get_height())));
            color.rgb = mix(color.rgb, pixel.rgb, pixel.a);
        }
    }
    if (p.has_mask != 0 && all(offset >= int2(0))) {
        uint2 cell = uint2(offset / p.scale);
        if (cell.x < mask.get_width() && cell.y < mask.get_height()) {
            float level = mask.read(cell).r;
            if (level > OVERLAY_TITLE_LEVEL) color = OVERLAY_TITLE_COLOR;
            else if (level > OVERLAY_TEXT_LEVEL) color = OVERLAY_TEXT_COLOR;
        }
    }
    float2 size = float2(out.get_width(), out.get_height());
    if (p.has_wand == REVOLVER_WAND) {
        float4 held = revolver.read(id);
        color.rgb = color.rgb * (1.0 - held.a) + held.rgb;
    } else if (p.has_wand != 0) {
        float3 glow = float3(0.0);
        float4 sum = float4(0.0);
        for (int k = 0; k < WAND_SAMPLES_PER_SIDE * WAND_SAMPLES_PER_SIDE; k++) {
            float2 sub = (float2(k % WAND_SAMPLES_PER_SIDE, k / WAND_SAMPLES_PER_SIDE) + 0.5) / float(WAND_SAMPLES_PER_SIDE);
            float2 ndc = float2(2.0, -2.0) * ((float2(id) + sub) / size) + float2(-1.0, 1.0);
            float4 ray_color = shade_wand(normalize(float3(ndc.x * p.tan_half_fov * size.x / size.y, ndc.y * p.tan_half_fov, -1.0)), wand);
            if (ray_color.a > 0.0) sum += ray_color;
            else glow += ray_color.rgb;
        }
        float samples = float(WAND_SAMPLES_PER_SIDE * WAND_SAMPLES_PER_SIDE);
        color.rgb = mix(color.rgb + glow / samples, sum.a > 0.0 ? sum.rgb / sum.a : color.rgb, sum.a / samples);
    }
    color.rgb = draw_minimap(color.rgb, id, minimap, minimap_map);
    float2 at_px = float2(id) + 0.5;
    float shadow_a = 0.0;
    float3 glow = float3(0.0);
    for (int i = 0; i < p.rect_count; i++) {
        float4 halo = get_rect_halo(rects[i], at_px, size);
        shadow_a = max(shadow_a, halo.a);
        glow = max(glow, halo.rgb);
    }
    color.rgb = color.rgb * (1.0 - shadow_a) + glow;
    for (int i = 0; i < p.rect_count; i++) {
        float4 layer = get_rect_layer(rects[i], at_px, size);
        color.rgb = color.rgb * (1.0 - layer.a) + layer.rgb;
    }
    for (int i = 0; i < p.rect_count; i++) {
        float pulse_glow = get_pulse_glow(rects[i], at_px, size);
        float3 pulse_rgb = rects[i].pulse.color.rgb;
        color.rgb = mix(color.rgb, pulse_rgb, pulse_glow) + pulse_rgb * pulse_glow * RECT_PULSE_LIGHT_GAIN;
    }
    out.write(color, id);
}
