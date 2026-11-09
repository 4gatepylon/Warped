/**
 * @file render_voxel.metal
 * @brief `render_voxel.metal` ray-traces voxel maps of the portable map format (`common/maps/`) in H³, one thread
 * per pixel, as the Lunar Surface level's WebGL fragment shader does (`demos/levels/lunar_surface/index.html`):
 * straight geodesic chords between warp objects, kick–drift–kick steps inside their reach, a voxel walk through each
 * structure in its Klein coordinates, the ground's folded {4,5} tiling, lunar-Lambert light with soft sun shadows, lamps,
 * earthshine, a thin haze, laser beams, the dust of shots into the ground, the zone walls, stars, a planet, spaceships
 * flying over and bloom. Everything here belongs to the `lunar` appearance style.
 * @par Example
 * @code
 * dispatched by render_voxel.rs, one thread per pixel of the render size.
 * @endcode
 */
#include <metal_stdlib>
using namespace metal;

#define SCENE_BUFFERS constant VoxelUniforms &u [[buffer(16)]], device const VoxelStructure *structures [[buffer(17)]], \
    device const Shape *shapes [[buffer(18)]], device const VoxelWarp *warps [[buffer(19)]], device const float4 *warp_lists [[buffer(20)]], \
    device const VoxelLight *lights [[buffer(21)]], device const float4 *palette [[buffer(22)]], device const BeamSegment *beams [[buffer(23)]], \
    device const uchar *clearances [[buffer(24)]], device const float4 *surfaces [[buffer(25)]], device const float *mound_heights [[buffer(26)]], \
    constant float4 *means [[buffer(27)]], device const DustBurst *dust_bursts [[buffer(29)]], \
    device const DustParticle *dust_particles [[buffer(30)]], \
    texture2d_array<float> albedo_maps [[texture(4)]], texture2d_array<float> normal_maps [[texture(5)]], texture2d_array<float> arm_maps [[texture(6)]], \
    texture2d<float> milky_way [[texture(3)]], \
    device const MeshTriangle *mesh_triangles [[buffer(0)]], device const BvhNode *mesh_nodes [[buffer(1)]], \
    device const MeshMaterial *mesh_materials [[buffer(7)]], texture2d_array<float> mesh_albedo [[texture(12)]], \
    texture2d_array<float> mesh_mr [[texture(13)]], texture2d_array<float> mesh_normal [[texture(14)]], texture2d_array<float> mesh_emissive [[texture(15)]], \
    device const MeshTriangle *avatar_triangles [[buffer(8)]], device const BvhNode *avatar_nodes [[buffer(10)]]

constant int MAX_ITERATIONS = 240;
constant int MAX_SHAPE_STEPS = 96;
constant float SHAPE_HIT_M = 0.003;
constant float SHAPE_NORMAL_M = 0.002;
constant int SHAPE_BOX = 0;
constant int SHAPE_CYLINDER = 1;
constant int MAX_FOLDS = 26;
constant int OUTCOME_LOST = 0;
constant int OUTCOME_SKY = 1;
constant int OUTCOME_HIT = 2;
constant int MAT_GROUND = 255;
constant int MAT_MESH = 254;
constant int FIRST_AVATAR_ROOT_CODE = -2;
constant int CELL_MAP_MODEL = 1;
constant int CELL_AVATAR = 2;
constant int MESH_STACK_SIZE = 48;
constant float MESH_PARALLEL_DET = 1.0e-9;
constant float MESH_TEXTURE_SIDE_PX = 1024.0;
constant float BEACON_GLOW = 0.6;
constant int NO_MATERIAL = -1;
constant float NO_HIT_S = 1.0e9;
constant float LAMBDA_LIMIT = 0.99999994;
constant float TINY = 1.0e-9;
constant float FAR_S = 12.0;
constant float SHADOW_M = 80.0;
constant float SURFACE_OFFSET_M = 0.02;
constant float TERRAIN_SHADOW_START_M = 1.0;
constant float FOIL_LAMBDA_SQ = 0.25;
constant float HORIZON_CAPTURE_FRAC = 1.02;
constant float BEND_STEP_FRAC = 0.2;
constant int LATE_STEP_COUNT = 90;
constant float LATE_BEND_STEP_FRAC = 0.24;
constant float MIN_BEND_STEP_M = 0.004;
constant float MAX_BEND_STEP_M = 6.0;
constant float GROUND_DETAIL_S = 7.0;
constant float SUN_INTENSITY = 1.45;
constant float LUNAR_LAMBERT_FRAC = 0.7;
constant float TEXTURE_SIDE_PX = 2048.0;
constant float UNTEXTURED_ROUGHNESS = 0.85;
constant float MIN_ROUGHNESS = 0.045;
constant float MIN_FACING = 0.05;
constant float MIN_MEAN_ALBEDO = 0.02;
constant float3 LUMA = float3(0.2126, 0.7152, 0.0722);
constant float DISPLAY_GAMMA = 2.2;
constant float EXPOSURE = 1.3;
constant float BLOOM_STRENGTH = 0.08;
constant int BLOOM_LEVELS = __BLOOM_LEVELS__;
constant float VIGNETTE_STRENGTH = 0.28;
constant float GRADE_MIDDLE_GREY = 0.18;
constant float GRADE_CONTRAST = 1.18;
constant float GRADE_SATURATION = 1.12;
constant float3 GRADE_WHITE_BALANCE = float3(1.04, 1.0, 0.94);
constant float3 GRADE_SHADOW_TINT = float3(0.94, 1.0, 1.07);
constant float3 GRADE_HIGHLIGHT_TINT = float3(1.04, 1.0, 0.95);
constant float AMBIENT = 0.11;
constant float EARTHSHINE = 0.08;
constant float SHADOW_FLOOR = 0.0;
constant float SHADOW_PENUMBRA_TAN = 0.015;
constant float3 HAZE_COLOR = float3(0.15, 0.17, 0.21);
constant float HAZE_START_M = 60.0;
constant float HAZE_DISTANCE_M = 150.0;
constant float HAZE_MAX_FRAC = 0.28;
constant float HAZE_HORIZON_HEIGHT_FRAC = 0.08;
constant float HAZE_SUN_GLOW = 0.5;
constant float HAZE_SUN_POWER = 8.0;
constant float HAZE_SKYLIGHT = 0.2;
constant float BLOOM_KARIS_LUMA = 4.0;
constant float BLOOM_GLOW = 1.0;
constant float2 BLOOM_GLOW_LUMA_EDGES = float2(0.5, 2.0);
constant float3 EARTHSHINE_COLOR = float3(0.55, 0.7, 1.0);
constant int HAND_LIGHT_COUNT = 3;
constant int HAND_LIGHT_BUFFER_INDEX = 28;
constant float LIGHT_CUTOFF_FRAC = 3.0;
constant float LIGHT_WRAP = 0.3;
constant float BLOCK_NOISE = 0.18;
constant float TEXEL_NOISE = 0.12;
constant float EDGE_DARKEN = 0.55;
constant float EDGE_WIDTH = 0.05;
constant float SUBTEXELS = 8.0;
constant float DETAIL_FOOTPRINT_M = 0.9;
constant float GLASS_TINT = 0.8;
constant float3 GLASS_SHEEN = float3(0.05, 0.07, 0.09);
constant float BEAM_RADIUS_M = 0.02;
constant float BEAM_REACH = 4.8;
constant float SPOT_RADIUS_M = 0.15;
constant float DUST_MIN_WIDTH_PX = 1.5;
constant float DUST_CUTOFF_WIDTHS = 3.0;
constant float DUST_WRAP = 0.4;
constant float DUST_FORWARD_GLOW = 0.8;
constant float DUST_FORWARD_POWER = 6.0;
constant float DUST_EARTHSHINE_FRAC = 0.5;
constant float DUST_SMOOTH_DEPTH = 0.02;
constant int DUST_PUFF = 0;
constant float GRAIN_WRAP = 0.2;
constant float3 HASH_DIRECTION = float3(12.9898, 78.233, 37.719);
constant float HASH_SCALE = 43758.5453;
constant int FBM_OCTAVES = 4;
constant float FBM_LACUNARITY = 2.03;
constant float STAR_DENSITY_COARSE = 0.03;
constant float STAR_DENSITY_FINE = 0.012;
constant float STAR_PIXELS = 0.8;
constant float STAR_GRID_COARSE = 60.0;
constant float STAR_GRID_FINE = 170.0;
constant float MILKY_WAY_MAX_LATITUDE_RAD = 0.7853982;
constant float MILKY_WAY_DISK = 0.21;
constant float MILKY_WAY_BULGE = 0.75;
constant float MILKY_WAY_DISK_WIDTH = 0.18;
constant float MILKY_WAY_CORE_WIDENING = 0.3;
constant float2 MILKY_WAY_BULGE_SIZE = float2(0.5, 0.21);
constant float MILKY_WAY_ANTICENTRE_FRAC = 0.5;
constant float MILKY_WAY_EDGE_WARP = 0.11;
constant float MILKY_WAY_WARP_SCALE = 2.3;
constant float3 MILKY_WAY_WARP_SEEDS = float3(0.0, 5.2, 9.7);
constant float3 MILKY_WAY_STRETCH = float3(1.0, 1.0, 1.7);
constant float MILKY_WAY_STRETCH_WARP = 0.35;
constant float MILKY_WAY_NOISE_CONTRAST = 2.5;
constant float MILKY_WAY_DUST_CONTRAST = 3.0;
constant int MILKY_WAY_COARSE_OCTAVES = 3;
constant int MILKY_WAY_GRAIN_OCTAVES = 4;
constant int MILKY_WAY_DARK_OCTAVES = 6;
constant float MILKY_WAY_CLOUD_SCALE = 3.0;
constant float MILKY_WAY_CLOUD_AMOUNT = 0.9;
constant float2 MILKY_WAY_CLOUD_GAIN = float2(0.55, 0.9);
constant float2 MILKY_WAY_BULGE_CLOUD_GAIN = float2(0.75, 0.5);
constant float MILKY_WAY_GRAIN_SCALE = 16.0;
constant float MILKY_WAY_GRAIN_SEED = 4.4;
constant float MILKY_WAY_GRAIN_AMOUNT = 0.5;
constant float2 MILKY_WAY_GRAIN_GAIN = float2(0.6, 0.8);
constant float2 MILKY_WAY_BULGE_GRAIN_GAIN = float2(0.8, 0.4);
constant float MILKY_WAY_LANE_WANDER = 0.06;
constant float MILKY_WAY_LANE_WANDER_SCALE = 3.1;
constant float MILKY_WAY_LANE_WANDER_SEED = 2.0;
constant float MILKY_WAY_LANE_WIDTH_FRAC = 0.447;
constant float MILKY_WAY_RIFT_SCALE = 2.6;
constant float MILKY_WAY_RIFT_SEED = 3.1;
constant float2 MILKY_WAY_RIFT_EDGES = float2(0.35, 0.8);
constant float MILKY_WAY_RIFT_CORE_GAP = 0.45;
constant float MILKY_WAY_RIFT_DEPTH = 2.6;
constant float MILKY_WAY_DARK_SCALE = 9.0;
constant float MILKY_WAY_DARK_WARP = 1.5;
constant float MILKY_WAY_DARK_SEED = 7.7;
constant float2 MILKY_WAY_DARK_EDGES = float2(0.5, 0.92);
constant float2 MILKY_WAY_DARK_GRAIN_GAIN = float2(0.55, 0.45);
constant float MILKY_WAY_DARK_DEPTH = 1.3;
constant float3 MILKY_WAY_REDDENING = float3(0.9, 1.0, 1.12);
constant float3 MILKY_WAY_DISK_COLOR = float3(0.8, 0.88, 1.0);
constant float3 MILKY_WAY_BULGE_COLOR = float3(1.0, 0.8, 0.56);
constant float3 MILKY_WAY_NEBULA_COLOR = float3(1.0, 0.32, 0.45);
constant float MILKY_WAY_NEBULA = 0.5;
constant float MILKY_WAY_NEBULA_SCALE = 20.0;
constant float MILKY_WAY_NEBULA_SEED = 21.0;
constant float2 MILKY_WAY_NEBULA_EDGES = float2(0.72, 0.9);
constant float MILKY_WAY_STAR_BOOST = 4.0;
constant float MILKY_WAY_MICRO_STAR_GRID = 420.0;
constant float MILKY_WAY_MICRO_STAR_DENSITY = 0.3;
constant float MILKY_WAY_MICRO_STAR_BRIGHTNESS = 0.35;
constant float2 MILKY_WAY_TONE = float2(0.69, 0.46);
constant float SUN_COS_INNER = 0.99996;
constant float SUN_COS_OUTER = 0.99990;
constant float SUN_GLARE_POWER = 900.0;
constant float SUN_DISC_BRIGHTNESS = 6.0;
constant float3 OCEAN_COLOR = float3(0.07, 0.2, 0.48);
constant float3 LAND_COLOR = float3(0.32, 0.38, 0.2);
constant float3 CLOUD_COLOR = float3(0.95);
constant float3 ATMOSPHERE_COLOR = float3(0.35, 0.6, 1.0);
constant float CRATER_FLOOR_RADIUS_M = 19.0;
constant float CRATER_RIM_RADIUS_M = 24.0;
constant float CRATERLET_MAX_M = 6.0;
constant float CRATERLET_CHANCE = 0.55;
constant float EJECTA_INNER_M = 27.0;
constant float EJECTA_OUTER_M = 80.0;
constant float ZONE_STRIP_M = 0.8;
constant float ZONE_TINT_GAIN = 1.5;
constant float ZONE_TINT_FRAC = 0.07;
constant int MAX_ZONE_WALL_COUNT = __MAX_ZONE_WALL_COUNT__;
constant float ZONE_WALL_BRIGHTNESS = 0.8;
constant float ZONE_WALL_FOOT_M = 0.4;
constant float ZONE_WALL_BODY_FRAC = 0.6;
constant float ZONE_WALL_TOP_FADE_FRAC = 0.4;
constant float ZONE_WALL_SOFT_TOP_FRAC = 0.25;
constant float ZONE_WALL_END_M = 4.0;
constant float ZONE_WALL_MAX_SLANT = 4.0;
constant float ZONE_WALL_FACE_FRAC = 0.7;
constant float ZONE_WALL_SAMPLE_M = 1.0;
constant int ZONE_WALL_MAX_SAMPLES = 12;
constant float BOUNDARY_STRIP_FRAC = 0.04;
constant float CELL_HASH_SCALE = 4096.0;
constant float HASH_SHRINK = 0.001;
constant float MARIA_BASE = 0.93;
constant float MARIA_CONTRAST = 0.12;
constant float MARIA_PER_M = 0.03;
constant float FLOOR_RING_M = 3.0;
constant float FLOOR_RING_DARKEN = 0.25;
constant float EJECTA_RAY_COUNT = 9.0;
constant float2 EJECTA_RAY_EDGES = float2(0.62, 0.8);
constant float3 EJECTA_COLOR = float3(0.78, 0.78, 0.76);
constant float EJECTA_STRENGTH = 0.6;
constant float CRATERLET_CLEARANCE_M = 12.0;
constant float CRATERLET_SPACING_FRAC = 0.5;
constant float CRATERLET_HASH_SCALE = 512.0;
constant float CRATERLET_MIN_FRAC = 0.35;
constant float CRATERLET_SIZE_SEED = 7.31;
constant float CRATERLET_RIM_FRAC = 0.75;
constant float CRATERLET_RIM_BRIGHTEN = 1.18;
constant float CRATERLET_FLOOR_DARKEN = 0.72;
constant float CRATERLET_FLOOR_SLOPE = 0.1;
constant float ZONE_STRIP_GLOW = 0.55;
constant float OUTSIDE_DARKEN = 0.72;
constant float HAZARD_STRIPE_M = 1.5;
constant float3 HAZARD_DARK_COLOR = float3(0.1);
constant float GROUND_EDGE_FRAC = 0.6;
constant float CELL_SEED = 97.0;
constant float STRUCT_SEED = 17.31;
constant float TEXEL_SEED = 131.0;
constant float CONSOLE_STRIPE_DARKEN = 0.25;
constant float STRIPES_PER_BLOCK = 2.0;
constant float3 SOLAR_GRID_COLOR = float3(0.45, 0.5, 0.62);
constant float2 SOLAR_CELLS = float2(4.0, 2.0);
constant float SOLAR_GRID_EDGE = 0.88;
constant float3 HAZARD_STRIPE_COLOR = float3(0.08);
constant float FOIL_CRINKLES = 5.0;
constant float FOIL_VARIATION = 0.4;
constant float PANEL_SEAMS = 2.0;
constant float PANEL_SEAM_EDGE = 0.94;
constant float PANEL_SEAM_DARKEN = 0.12;
constant float GLOW_BRIGHTNESS = 1.2;
constant float GLOW_RIM = 0.3;
constant float DETAIL_FADE_START_FRAC = 0.3;
constant float LIGHT_FADE_START_FRAC = 0.6;
constant float HORIZON_GUARD = 1.0e-4;
constant float SUN_GLARE = 0.6;
constant float SUN_HALO_POWER = 40.0;
constant float SUN_HALO = 0.04;
constant float2 LAND_EDGES = float2(0.52, 0.56);
constant float LAND_SCALE = 2.2;
constant float3 LAND_OFFSET = float3(3.0);
constant float2 CLOUD_EDGES = float2(0.5, 0.75);
constant float CLOUD_SCALE = 4.5;
constant float3 CLOUD_OFFSET = float3(9.0, 1.0, 4.0);
constant float PLANET_SUNLIT = 1.15;
constant float PLANET_NIGHT = 0.015;
constant float LIMB_POWER = 6.0;
constant float LIMB_GLOW = 0.6;
constant float LIMB_WRAP = 0.2;
constant float HALO_RADIUS_SQ = 1.12;
constant float HALO_GLOW = 0.25;
constant float HALO_WRAP = 0.3;
constant float STAR_FAINT = 0.15;
constant float STAR_BRIGHT = 1.5;
constant float STAR_BRIGHTNESS_POWER = 4.0;
constant float STAR_JITTER = 0.6;
constant float2 STAR_SEEDS = float2(7.1, 3.7);
constant float FINE_STAR_BRIGHTNESS = 0.5;
constant float BEAM_HAZE_BRIGHTNESS = 0.45;
constant float BEAM_HAZE_WHITENESS = 0.45;
constant float BEAM_CORE_BRIGHTNESS = 1.6;
constant float BEAM_CORE_WHITENESS = 0.9;
/** @brief `ONE_COLOR_LASER_WHITENESS_FRAC` scales how white a beam's core gets on a map whose lasers share one colour (`laser_color`), so the colour shows on snow. */
constant float ONE_COLOR_LASER_WHITENESS_FRAC = 0.35;
constant float DARK_PYLON_GLOW_FRAC = 0.12;
constant float BEAM_SPECKLE_STEPS = 1023.0;
constant int THROWN_TINT_FIRST_BIT = 1;
constant int THROWN_TINT_MASK = 3;
constant int BEAM_RIFLE_BIT = 3;
constant int BEAM_SPECKLE_FIRST_BIT = 4;
constant int RIFLE_BEAM_CODE = 2;
constant int BEACON_BEAM_CODE = 4;
constant float BEACON_BEAM_RADIUS_M = 0.12;
constant float LASER_HALO_OPACITY = 0.9;
constant float LASER_CORE_WHITENESS = 0.6;
constant float MIN_SPECKLE_GLOW = 0.02;
constant uint SPECKLE_KIND_SALT = 0x9e3779b9u;
constant uint3 PIXEL_HASH_PRIMES = uint3(73856093u, 19349663u, 2654435761u);
constant uint2 PIXEL_HASH_MULTIPLIERS = uint2(0x7feb352du, 0x846ca68bu);
constant uint3 PIXEL_HASH_SHIFTS = uint3(16u, 15u, 16u);
constant uint PIXEL_HASH_DROPPED_BITS = 8u;
constant float PIXEL_HASH_SCALE = 1.0 / 16777216.0;
constant float3 BLUE_COLOR = float3(0.30, 0.62, 1.0);
constant float3 RED_COLOR = float3(1.0, 0.16, 0.08);
constant float3 SUN_COLOR = float3(1.0, 0.96, 0.88);
constant float3 FLASHLIGHT_COLOR = float3(1.0, 0.95, 0.85);
constant int TILE_CELLS_PER_HALF = 18;
constant int TERRAIN_OCTAVES = 5;
constant float TERRAIN_RIM_NOISE_M = 60.0;
constant float TERRAIN_RIM_NOISE_FRAC = 0.5;
constant float TERRAIN_OUTER_FLANK_PER_WALL = 3.0;
constant float TERRAIN_WALL_RELIEF_GAIN = 3.0;
constant uint TERRAIN_RIM_SEED = 9u;
constant float TERRAIN_STEP_FRAC = 0.45;
constant float TERRAIN_MIN_STEP_M = 0.03;
constant float TERRAIN_STEP_PER_M = 0.01;
constant int TERRAIN_MAX_STEPS = 200;
constant int TERRAIN_BISECTIONS = 8;
constant float TERRAIN_NORMAL_STEP_M = 0.1;
constant float KIND_BLACK_HOLE = 0.0;
constant float HALF_KIND = 0.5;
constant float FIRST_THROWN_KIND = 1.5;
constant float3 THROWN_TINTS[3] = {float3(__PURPLE_COLOR__), float3(__GREEN_COLOR__), float3(__HIT_FLASH_COLOR__)};
constant float THROWN_TINT_FRAC = 0.45;
constant float THROWN_TINT_FLOOR = 0.08;
constant float GLOW_FLOOR = 1.0 / 1024.0;
constant float SMOOTH_SECOND_DIFFERENCE_PER_FIRST = 1.6;
constant int REFINED_PIXELS_PER_CELL = 3;
constant int FOVEA_SHARP = 0;
constant int FOVEA_HALF = 1;
constant int FOVEA_QUARTER = 2;
constant uint FOVEA_BLOCK_PX = 4;
constant float FOVEA_SAMPLE_MARGIN_PX = 8.0;
constant float SOFT_BAND_PX = 48.0;
constant float FXAA_EDGE_THRESHOLD = 0.125;
constant float FXAA_EDGE_THRESHOLD_MIN = 0.0312;
constant int FXAA_SEARCH_STEP_COUNT = 10;
constant float FXAA_SUBPIXEL_QUALITY = 0.75;
constant uint REFINE_CELL_MASK = (1u << 24) - 1;
constant int REFINE_LEVEL_FIRST_BIT = 24;
constant int REFINE_GROUP_SIZE = __REFINE_GROUP_SIZE__;
constant int SPACESHIP_COUNT = __SPACESHIP_COUNT__;
constant int MAX_BLIZZARD_COUNT = __MAX_BLIZZARD_COUNT__;
/** @brief The snow map's look (`snow.x` the fog's meters, 0 off; `snow.w` the light snowfall's flakes per cubic meter): sky, distance fog, ground, falling snow, and the blizzards' fog and flakes. */
constant float3 SNOW_ZENITH = float3(0.42, 0.56, 0.74);
constant float3 SNOW_HORIZON = float3(0.84, 0.88, 0.93);
constant float3 SNOW_CLOUD = float3(0.9, 0.92, 0.95);
constant float SNOW_FOG_START_M = 12.0;
constant float SNOW_FLAKE_M = 18.0;
constant float SNOW_FLAKE_STEP_M = 0.9;
constant float SNOW_FOG_MAX_FRAC = 0.82;
constant float3 BLIZZARD_FOG = float3(0.86, 0.89, 0.93);
constant float BLIZZARD_SKY_M = 400.0;
constant float FLAKE_STEP_M = 0.7;
constant int FLAKE_MAX_STEPS = 40;
constant float FLAKE_RADIUS_M = 0.035;
constant float3 FLAKE_FALL_M_PER_S = float3(2.5, -2.2, 1.0);
constant float SNOW_GRAIN = 0.08;
constant int HULL_FIGHTER = 0;
constant int HULL_RING_SHIP = 1;
constant int FLAT_HULL_COUNT = 2;
constant int HULL_BATTLE_STATION = 2;
constant int HULL_CAPITAL_SHIP = 3;
constant int LIGHT_NAV = 0;
constant int LIGHT_STROBE = 1;
constant float SHIP_BOUND_HALF_LENGTHS = 1.5;
constant float SHIP_BOUND_PIXELS = 4.0;
constant float SHIP_ROUNDING_HALF_LENGTHS = 0.02;
constant float SHIP_GRADIENT_PIXELS = 0.5;
constant float SHIP_BEVEL_HALF_LENGTHS = 0.03;
constant float SHIP_BEVEL_TILT_RAD = 1.2;
constant float SHIP_SUNLIT = 1.1;
constant float3 SHIP_GROUNDSHINE_COLOR = float3(0.5, 0.48, 0.46);
constant float SHIP_PANELS_PER_HALF_LENGTH = 4.0;
constant float SHIP_LIGHT_MIN_PIXELS = 1.0;
constant float SHIP_ENGINE_BRIGHTNESS = 2.6;
constant float SHIP_PLUME_BRIGHTNESS = 0.7;
constant float SHIP_PLUME_WIDTH_PER_RADIUS = 0.8;
constant float SHIP_PLUME_HALF_LENGTHS = 0.3;
constant float3 PORT_COLOR = float3(1.6, 0.18, 0.12);
constant float3 STARBOARD_COLOR = float3(0.25, 1.6, 0.5);
constant float3 STROBE_COLOR = float3(3.0);
constant float3 WINDOW_COLOR = float3(1.0, 0.8, 0.5);
constant float RING_RADIUS_HALF_LENGTHS = 0.52;
constant float RING_HALF_WIDTH_HALF_LENGTHS = 0.055;
constant float RING_SPOKE_COUNT = 3.0;
constant float RING_SPOKE_RADIUS_HALF_LENGTHS = 0.03;
constant float3 RING_COLOR = float3(0.75, 0.74, 0.7);
constant float RING_LIGHT_COUNT = 6.0;
constant float RING_LIGHT_HALF_LENGTHS = 0.02;
constant float3 RING_LIGHT_COLOR = float3(0.8, 1.1, 1.3);
constant float GIANT_NIGHT = 0.06;
constant float STATION_GROUNDSHINE_FRAC = 0.25;
constant float CAPITAL_GROUNDSHINE_FRAC = 1.2;
constant float STATION_TILT_RAD = 0.3;
constant float2 STATION_DISH_LAT_LON_RAD = float2(0.55, -0.4);
constant float STATION_DISH_RAD = 0.3;
constant float STATION_BOWL_RADIUS_RADII = 0.45;
constant float STATION_LENS_RADIUS_RADII = 0.035;
constant float2 STATION_DISH_RING_FRACS = float2(0.45, 0.76);
constant float STATION_DISH_RING_HALF_WIDTH_RADII = 0.004;
constant float STATION_TRENCH_HALF_RAD = 0.022;
constant float STATION_BAND_RAD = 0.105;
constant float STATION_SEAM_HALF_RAD = 0.0025;
constant float STATION_SEAM_DARKEN_FRAC = 0.3;
constant float STATION_PANEL_RAD = 0.14;
constant float STATION_MIN_PANEL_COS = 0.2;
constant float STATION_PANEL_VARIATION = 0.16;
constant float3 STATION_COLOR = float3(0.6, 0.61, 0.63);
constant float3 STATION_DISH_COLOR = float3(0.5, 0.51, 0.53);
constant float3 STATION_TRENCH_COLOR = float3(0.12, 0.12, 0.13);
constant float3 STATION_LENS_COLOR = float3(0.07, 0.08, 0.09);
constant float STATION_WINDOW_FRAC = 0.1;
constant float STATION_WINDOW_SEED = 0.5;
constant float STATION_WINDOW_RADIUS_RADII = 0.005;
constant float STATION_WINDOW_BRIGHTNESS = 0.5;
constant float2 STATION_NIGHT_EDGES = float2(-0.05, 0.12);
constant float STATION_TRENCH_LIGHT_COUNT = 14.0;
constant float STATION_TRENCH_LIGHT_RADIUS_RADII = 0.007;
constant float STATION_BEACON_RADIUS_RADII = 0.012;
constant float CAPITAL_TILT_RAD = 0.9;
constant float CAPITAL_WINDOWS_PER_HALF_LENGTH = 45.0;
constant float CAPITAL_WINDOW_ROW_HALF_LENGTHS = 0.03;
constant float2 CAPITAL_WINDOW_SIZE_FRACS = float2(0.55, 0.35);
constant float CAPITAL_WINDOW_LIT_FRAC = 0.5;
constant float CAPITAL_WINDOW_BRIGHTNESS = 0.9;
constant float CAPITAL_WINDOW_SEED = 0.25;
constant float CAPITAL_EYE_DISTANCE_HALF_LENGTHS = 2.0;
constant float CAPITAL_PANELS_PER_HALF_LENGTH = 9.0;
constant float CAPITAL_PANEL_VARIATION = 0.05;
constant float CAPITAL_HALF_WIDTH_PER_HALF_LENGTH = 0.21;
constant float2 CAPITAL_EDGE_WINDOW_BAND_HALF_LENGTHS = float2(0.03, 0.09);
constant float CAPITAL_EDGE_WINDOW_END_HALF_LENGTHS = 0.5;
constant float3 CAPITAL_ENGINE_COLOR = float3(0.55, 0.8, 1.0);
constant float CAPITAL_ENGINE_BRIGHTNESS = 1.3;

/** @brief `HULL_BOXES` are the spaceships' boxes (centre, half size; in half lengths, x to the nose, y to starboard), mirrored across the keel. */
constant float4 HULL_BOXES[] = {float4(0.82, 0.0, 0.1, 0.09), float4(-0.85, 0.0, 0.08, 0.14)};
/** @brief `HULL_BOX_COLORS` are the colours of `HULL_BOXES`. */
constant float3 HULL_BOX_COLORS[] = {float3(0.7, 0.7, 0.72), float3(0.36, 0.36, 0.38)};
/** @brief `HULL_CAPSULES` are the spaceships' tapered capsules (the two ends of the axis), mirrored across the keel. */
constant float4 HULL_CAPSULES[] = {float4(-0.8, 0.0, 0.8, 0.0), float4(-0.06, 0.0, 0.06, 0.0)};
/** @brief `HULL_CAPSULE_RADII` are the radii of `HULL_CAPSULES` at their two ends. */
constant float2 HULL_CAPSULE_RADII[] = {float2(0.06, 0.05), float2(0.12, 0.12)};
/** @brief `HULL_CAPSULE_COLORS` are the colours of `HULL_CAPSULES`. */
constant float3 HULL_CAPSULE_COLORS[] = {float3(0.58, 0.58, 0.6), float3(0.66, 0.66, 0.68)};
/** @brief `HULL_PART_RANGES` gives each hull's first box, box count, first capsule and capsule count. */
constant int4 HULL_PART_RANGES[FLAT_HULL_COUNT] = {int4(0, 0, 0, 0), int4(0, 2, 0, 2)};
/** @brief `HULL_TRIANGLES` are the spaceships' flat triangles (three corners), each symmetric across the keel. */
constant float2 HULL_TRIANGLES[][3] = {{float2(1.0, 0.0), float2(-0.75, 0.82), float2(-0.75, -0.82)}};
/** @brief `HULL_TRIANGLE_COLORS` are the colours of `HULL_TRIANGLES`. */
constant float3 HULL_TRIANGLE_COLORS[] = {float3(0.66, 0.68, 0.72)};
/** @brief `HULL_INSETS` are triangles painted on a hull without changing its outline (three corners), later ones on top. */
constant float2 HULL_INSETS[][3] = {
    {float2(0.72, 0.0), float2(-0.75, 0.2), float2(-0.75, -0.2)}, {float2(0.5, 0.0), float2(0.12, 0.08), float2(0.12, -0.08)},
};
/** @brief `HULL_INSET_COLORS` are the colours of `HULL_INSETS`: a darker spine and a dark glass canopy. */
constant float3 HULL_INSET_COLORS[] = {float3(0.44, 0.46, 0.52), float3(0.14, 0.2, 0.3)};
/** @brief `HULL_TRIANGLE_RANGES` gives each hull's first triangle, triangle count, first inset and inset count. */
constant int4 HULL_TRIANGLE_RANGES[FLAT_HULL_COUNT] = {int4(0, 1, 0, 2), int4(0, 0, 0, 0)};
/** @brief `HULL_ENGINES` are the spaceships' engine nozzles (centre, radius, unused), mirrored across the keel. */
constant float4 HULL_ENGINES[] = {float4(-0.77, 0.12, 0.055, 0.0), float4(-0.95, 0.0, 0.09, 0.0)};
/** @brief `HULL_PANEL_VARIATIONS` are how much each hull's panels vary in brightness (plain on the fighters' deltas). */
constant float HULL_PANEL_VARIATIONS[FLAT_HULL_COUNT] = {0.06, 0.3};
/** @brief `HULL_ENGINE_COLORS` are each hull's engine colours. */
constant float3 HULL_ENGINE_COLORS[FLAT_HULL_COUNT] = {float3(0.45, 0.7, 1.0), float3(0.5, 0.95, 1.0)};
/** @brief `HULL_LIGHTS` are the spaceships' lights (centre, `LIGHT_*` kind or else a window, radius), mirrored across the keel. */
constant float4 HULL_LIGHTS[] = {
    float4(-0.7, 0.76, 0.0, 0.035), float4(-0.74, 0.0, 1.0, 0.04),
    float4(0.85, 0.1, 0.0, 0.025), float4(-0.85, 0.15, 1.0, 0.03), float4(0.86, 0.0, 2.0, 0.015),
};
/** @brief `HULL_GLOW_RANGES` gives each hull's first engine, engine count, first light and light count. */
constant int4 HULL_GLOW_RANGES[FLAT_HULL_COUNT] = {int4(0, 1, 0, 2), int4(1, 1, 2, 3)};
/**
 * @brief `CAPITAL_HULL_PLANES` are the capital ship's wedge (unit-free normal, offset; inside where `dot(normal, x) <=
 * offset`), in half lengths with x to the nose, y to port and z up: two sloped top faces meeting in a ridge, two
 * flatter bottom faces and the stern.
 */
constant float4 CAPITAL_HULL_PLANES[] = {
    float4(0.042, 0.2, 0.84, 0.042), float4(0.042, -0.2, 0.84, 0.042), float4(0.021, 0.1, -0.84, 0.021),
    float4(0.021, -0.1, -0.84, 0.021), float4(-1.0, 0.0, 0.0, 1.0),
};
/** @brief `CAPITAL_BOXES` are the capital ship's superstructure: a deck block, the bridge tower's neck and the bridge (lowest and highest corners, in the hull's axes). */
constant float3 CAPITAL_BOXES[][2] = {
    {float3(-0.95, -0.14, 0.0), float3(-0.35, 0.14, 0.15)}, {float3(-0.88, -0.03, 0.0), float3(-0.8, 0.03, 0.27)},
    {float3(-0.9, -0.13, 0.24), float3(-0.78, 0.13, 0.3)},
};
/** @brief `CAPITAL_BOX_COLORS` are the colours of `CAPITAL_BOXES`. */
constant float3 CAPITAL_BOX_COLORS[] = {float3(0.64, 0.65, 0.67), float3(0.56, 0.57, 0.59), float3(0.66, 0.67, 0.69)};
/** @brief `CAPITAL_HULL_COLOR` is the colour of the capital ship's wedge. */
constant float3 CAPITAL_HULL_COLOR = float3(0.58, 0.59, 0.62);
/** @brief `CAPITAL_ENGINES` are the capital ship's engine nozzles (centre in the hull's axes, radius), mirrored across the keel. */
constant float4 CAPITAL_ENGINES[] = {float4(-1.0, 0.0, 0.04, 0.04), float4(-1.0, 0.14, 0.03, 0.03)};
/** @brief `CAPITAL_LIGHTS` are the capital ship's lights (centre in the hull's axes, `LIGHT_*` kind), not mirrored. */
constant float4 CAPITAL_LIGHTS[] = {float4(-0.98, 0.41, 0.0, 0.0), float4(-0.98, -0.41, 0.0, 0.0), float4(-0.84, 0.0, 0.3, 1.0)};
constant int CAPITAL_BOX_COUNT = 3;
constant int CAPITAL_ENGINE_COUNT = 2;
constant int CAPITAL_LIGHT_COUNT = 3;
constant float CAPITAL_LIGHT_RADIUS_HALF_LENGTHS = 0.01;

/**
 * @brief `Spaceship` is one spaceship in the sky (`spaceships.rs`): its centre's direction at the origin with its half
 * length in radians, its unit heading with its hull (`HULL_*`), and its strobe's brightness, its engines' brightness
 * and its ring's turn in radians (w unused).
 */
struct Spaceship {
    float4 centre;
    float4 heading;
    float4 lights;
};

/**
 * @brief `ZoneWalls` is `zone_walls.rs`'s `GpuZoneWalls`: per wall its plane's (n_z, -n_w, edge u in meters, team),
 * then (height, opacity, thickness in meters, unused), the boundary's (u, v) semi-axes in meters (0 without one), and
 * (wall count, unused).
 */
struct ZoneWalls {
    float4 walls[MAX_ZONE_WALL_COUNT];
    float4 look;
    float4 boundary;
    int4 counts;
};

/**
 * @brief `VoxelUniforms` is one frame's camera and the map's counts, special materials, sky bodies, laser spot,
 * regions, flashlight, the spaceships, the dust (how many bursts and particles), the beams' speckle (the frame's seed
 * and the shares of speckled pixels painted black and turned white) and the zone walls (`zone_walls.rs`'s `to_gpu`) and the sharp centre (`fovea`: the radii in pixels from the image's centre
 * within which pixels are sharp and beyond which they are at a quarter resolution, then unused),
 * as `render_voxel.rs` writes them.
 */
struct VoxelUniforms {
    float4x4 camera;
    float tan_half_fov;
    float aspect;
    float pixel_angle_rad;
    float curvature_radius_m;
    int structure_count;
    int warp_count;
    int light_count;
    int beam_count;
    int width_px;
    int height_px;
    int mat_glass;
    int mat_regolith;
    int mat_dark_regolith;
    int mat_basalt;
    int mat_solar;
    int mat_hazard;
    int mat_pad;
    int mat_hull;
    int mat_gold_foil;
    int mat_console_first;
    int mat_console_count;
    int has_regions;
    int has_terrain;
    uint terrain_seed;
    float4 sun;
    float4 planet;
    float4 planet_e1;
    float4 planet_e2;
    float4 spot;
    float4 spot_color;
    float4 regions;
    float4 terrain;
    float4 relief;
    float4 mound_grid;
    float4 flashlight_position;
    float4 flashlight_direction;
    float4 flashlight;
    Spaceship spaceships[SPACESHIP_COUNT];
    int4 dust_counts;
    float4 beam_speckle;
    ZoneWalls zone_walls;
    float4 milky_way_pole;
    float4 milky_way_core;
    float4 snow;
    float4 blizzards[MAX_BLIZZARD_COUNT];
    float4 blizzard_air[MAX_BLIZZARD_COUNT];
    float4 laser_color;
    float4 fovea;
};
static_assert(sizeof(VoxelUniforms) == __BYTES_PER_VOXEL_UNIFORMS__, "render_voxel.rs's VoxelUniforms has the same layout");

/** @brief `Shape` is one smooth solid in its structure's chart (meters): centre and kind, half sizes and rounding, material and axis. */
struct Shape {
    float4 centre_kind;
    float4 half_round;
    int4 info;
};

/**
 * @brief `VoxelStructure` is one structure: the rows of its inverse frame, its bounding sphere's centre, cosh of its
 * radius, its first shape and shape count, whether its glowing shapes are dark (1: a pylon whose console is off) and
 * its mesh's root node (-1 for none;
 * `FIRST_AVATAR_ROOT_CODE - root` for a posed body in the avatar buffers), and R with whether its first shape is a console (1: a beacon)
 * and the palette entry of its body's team colour (-1 for none: not a player's or a corpse's body).
 */
struct VoxelStructure {
    float4 rows[4];
    float4 centre;
    float4 bound;
    int4 size;
    float4 scale;
};

/**
 * @brief `VoxelWarp` is one warp object: its inverse and forward frames, its centre, its kind, m and reach (curvature
 * units) and partner, where its list of structures within reach starts, how long it is, how many of its entries are
 * static structures (avatars follow) and where its clearance grid starts, and the grid's half size, cells per axis,
 * cells per unit and curvature radii per stored unit (`voxel_clearance.rs`), and a black hole's wide lens (the most it
 * turns a ray in radians, its core radius in curvature units; 0 for none). Each list entry is (structure, nearest and
 * farthest distance of its bounding sphere from the centre, unused).
 */
struct VoxelWarp {
    float4x4 inverse;
    float4x4 forward;
    float4 centre;
    float4 params;
    int4 list;
    float4 grid;
    float4 lens;
};

/** @brief `VoxelLight` is a lamp: its position, and its colour with its radius in meters. */
struct VoxelLight {
    float4 position;
    float4 color_radius;
};

/** @brief `BeamSegment` is one geodesic piece of a laser: start, unit tangent, and (length, speckle share, brightness, code: the team, 1 for red, plus `RIFLE_BEAM_CODE` for the rifle's, or `BEACON_BEAM_CODE` for a beacon's). */
struct BeamSegment {
    float4 a;
    float4 b;
    float4 info;
};

/**
 * @brief `GRAIN_PLANES` are the faces of the dust grains' polyhedra (unit outward normal, distance from the centre in
 * radii): a tetrahedron, a cube and an octahedron, their vertices 1.56, 1.2 and 1.26 radii out, so they look about
 * as big as a ball of that radius.
 */
constant float4 GRAIN_PLANES[] = {
    float4(-0.57735027, -0.57735027, -0.57735027, 0.52), float4(-0.57735027, 0.57735027, 0.57735027, 0.52),
    float4(0.57735027, -0.57735027, 0.57735027, 0.52), float4(0.57735027, 0.57735027, -0.57735027, 0.52),
    float4(1.0, 0.0, 0.0, 0.69282032), float4(-1.0, 0.0, 0.0, 0.69282032), float4(0.0, 1.0, 0.0, 0.69282032),
    float4(0.0, -1.0, 0.0, 0.69282032), float4(0.0, 0.0, 1.0, 0.69282032), float4(0.0, 0.0, -1.0, 0.69282032),
    float4(0.57735027, 0.57735027, 0.57735027, 0.72746134), float4(0.57735027, 0.57735027, -0.57735027, 0.72746134),
    float4(0.57735027, -0.57735027, 0.57735027, 0.72746134), float4(0.57735027, -0.57735027, -0.57735027, 0.72746134),
    float4(-0.57735027, 0.57735027, 0.57735027, 0.72746134), float4(-0.57735027, 0.57735027, -0.57735027, 0.72746134),
    float4(-0.57735027, -0.57735027, 0.57735027, 0.72746134), float4(-0.57735027, -0.57735027, -0.57735027, 0.72746134),
};
/** @brief `GRAIN_PLANE_RANGES` gives each `DustShape` (`dust.rs`) its first face in `GRAIN_PLANES` and face count; a puff has none. */
constant int2 GRAIN_PLANE_RANGES[] = {int2(0, 0), int2(0, 4), int2(4, 6), int2(10, 8)};

/**
 * @brief `DustBurst` is the dust of one shot into the ground (`dust.rs`): the rows of the inverse of its frame (at the
 * hit point, y up), the sun's direction in that frame with how much sunlight reaches it, where its particles start and
 * how many there are, and how far its farthest particle centre is and its largest radius, in meters.
 */
struct DustBurst {
    float4 rows[4];
    float4 sun;
    int4 range;
    float4 bound;
};

/**
 * @brief `DustParticle` is one grain or puff of mist: its centre in its burst's Klein coordinates (meters) and its
 * radius, its linear albedo and its opacity at its centre, its `DustShape` (`dust.rs`), and the unit quaternion that
 * turns it from its own axes into the burst's.
 */
struct DustParticle {
    float4 position_radius;
    float4 color_opacity;
    float4 shape;
    float4 orientation;
};

/**
 * @brief `Hit` is the first surface along a chord: distance, material, structure (-1 for the ground), face, grid point,
 * and, for a shadow ray, how far it stayed from every surface it passed: the least ratio of that gap to the distance
 * travelled times `SHADOW_PENUMBRA_TAN` (1 or more is fully lit).
 */
struct Hit {
    float s;
    int mat;
    int k;
    int axis;
    float sgn;
    float3 g;
    int3 cell;
    float lit;
};

/**
 * @brief `VoxelRayEnd` is where a ray ended, with what shading needs: the hit point (or the sky direction at the
 * origin, with w = 0), the ray's unit tangent there, the hit's grid point in its structure with the distance
 * travelled, the hit cell with its material (`MAT_GROUND` for the ground), the outcome with the structure (-1 for the
 * ground), face axis and laser and tint (bit 0 the laser's team, 1 for red; from `THROWN_TINT_FIRST_BIT`, two bits
 * for the thrown wormhole it went through, 0 for none; `BEAM_RIFLE_BIT` set for the rifle's laser, clear for the
 * sniper's blade; from `BEAM_SPECKLE_FIRST_BIT`, the laser's speckle share in steps of 1 / `BEAM_SPECKLE_STEPS`), the
 * face's side with the glass count and the laser's core and halo, and the dust and zone walls in front of it
 * (`scan_dust`, `scan_zone_walls`).
 */
struct VoxelRayEnd {
    float4 point;
    float4 view;
    float4 grid;
    int4 cell;
    int4 surface;
    float4 glow;
    float4 dust;
};
static_assert(sizeof(VoxelRayEnd) == __BYTES_PER_RAY_END__, "render_voxel.rs sizes the sample buffers by VOXEL_BYTES_PER_RAY_END");

/** @brief `MeshTriangle` is one map model triangle in its structure's chart (meters): corners with u, normals with v, and (material, unused...). */
struct MeshTriangle {
    float4 p[3];
    float4 n[3];
    int4 info;
};

/** @brief `BvhNode` is a box around model triangles, with its first child or triangle in `lo.w` and its triangle count (0 for an inner node) in `hi.w`. */
struct BvhNode {
    float4 lo;
    float4 hi;
};

/** @brief `MeshMaterial` is a model material: base colour, emissive colour with metallic, (roughness, unused...), and each map's layer (-1 for none). */
struct MeshMaterial {
    float4 base_color;
    float4 emissive_metallic;
    float4 params;
    int4 layers;
};

/** @brief `Scene` bundles a frame's buffers for the functions below. */
struct Scene {
    constant VoxelUniforms &u;
    device const VoxelStructure *structures;
    device const Shape *shapes;
    device const VoxelWarp *warps;
    device const float4 *warp_lists;
    device const VoxelLight *lights;
    device const float4 *palette;
    device const BeamSegment *beams;
    device const uchar *clearances;
    device const float4 *surfaces;
    device const float *mound_heights;
    constant float4 *means;
    texture2d_array<float> albedo_maps;
    texture2d_array<float> normal_maps;
    texture2d_array<float> arm_maps;
    device const DustBurst *dust_bursts;
    device const DustParticle *dust_particles;
    texture2d<float> milky_way;
    device const MeshTriangle *mesh_triangles;
    device const BvhNode *mesh_nodes;
    device const MeshMaterial *mesh_materials;
    texture2d_array<float> mesh_albedo;
    texture2d_array<float> mesh_mr;
    texture2d_array<float> mesh_normal;
    texture2d_array<float> mesh_emissive;
    device const MeshTriangle *avatar_triangles;
    device const BvhNode *avatar_nodes;
};

/** @brief `mdot` is the Minkowski product. @par Example @code float c = mdot(a, b); @endcode */
static float mdot(float4 a, float4 b) { return dot(a.xyz, b.xyz) - a.w * b.w; }

/** @brief `geodesic_point` gives cosh(s) x + sinh(s) v. @par Example @code float4 y = geodesic_point(x, v, 0.1); @endcode */
static float4 geodesic_point(float4 x, float4 v, float s) { return cosh(s) * x + sinh(s) * v; }

/** @brief `geodesic_tangent` gives sinh(s) x + cosh(s) v. @par Example @code float4 w = geodesic_tangent(x, v, 0.1); @endcode */
static float4 geodesic_tangent(float4 x, float4 v, float s) { return sinh(s) * x + cosh(s) * v; }

/** @brief `normalize_tangent` projects v on the tangent space at x and makes it unit. @par Example @code v = normalize_tangent(x, v); @endcode */
static float4 normalize_tangent(float4 x, float4 v) { v += mdot(v, x) * x; return v * rsqrt(max(mdot(v, v), TINY)); }

/** @brief `clamp_atanh` is atanh with its argument kept inside (-1, 1). @par Example @code float s = clamp_atanh(t); @endcode */
static float clamp_atanh(float t) { return atanh(clamp(t, -LAMBDA_LIMIT, LAMBDA_LIMIT)); }

/** @brief `measure_gap` gives the distance between nearby points from their chord. @par Example @code float d = measure_gap(a, b); @endcode */
static float measure_gap(float4 a, float4 b) { float4 d = a - b; return 2.0 * asinh(0.5 * sqrt(max(mdot(d, d), 0.0))); }

/** @brief `transport_from_centre` carries a direction at the world origin to x by parallel transport. @par Example @code float4 l = transport_from_centre(sun, x); @endcode */
static float4 transport_from_centre(float3 d, float4 x) { return float4(d, 0.0) + dot(x.xyz, d) / (1.0 + x.w) * (float4(0.0, 0.0, 0.0, 1.0) + x); }

/** @brief `hash_3d` gives a pseudo-random number in [0, 1) for a 3D point. @par Example @code float h = hash_3d(p); @endcode */
static float hash_3d(float3 p) { return fract(sin(dot(p, HASH_DIRECTION)) * HASH_SCALE); }

/** @brief `sample_noise_3d` is smooth value noise in [0, 1]. @par Example @code float n = sample_noise_3d(p); @endcode */
static float sample_noise_3d(float3 p) {
    float3 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(hash_3d(i), hash_3d(i + float3(1, 0, 0)), f.x), mix(hash_3d(i + float3(0, 1, 0)), hash_3d(i + float3(1, 1, 0)), f.x), f.y),
               mix(mix(hash_3d(i + float3(0, 0, 1)), hash_3d(i + float3(1, 0, 1)), f.x), mix(hash_3d(i + float3(0, 1, 1)), hash_3d(i + float3(1, 1, 1)), f.x), f.y), f.z);
}

/** @brief `sample_fbm_3d` sums octaves of value noise. @par Example @code float n = sample_fbm_3d(p); @endcode */
static float sample_fbm_3d(float3 p) {
    float sum = 0.0, amplitude = 0.5;
    for (int i = 0; i < FBM_OCTAVES; i++) { sum += amplitude * sample_noise_3d(p); p *= FBM_LACUNARITY; amplitude *= 0.5; }
    return sum;
}

/** @brief `to_linear` turns a display colour into a linear one. @par Example @code float3 c = to_linear(RED_COLOR); @endcode */
static float3 to_linear(float3 c) { return pow(c, DISPLAY_GAMMA); }

/** @brief `get_palette` gives material m's linear colour and glow (black for one outside the palette). @par Example @code float4 c = get_palette(scene, m); @endcode */
static float4 get_palette(thread const Scene &scene, int m) { return m >= 0 ? float4(to_linear(scene.palette[m].rgb), scene.palette[m].a) : float4(0.0); }

/** @brief `shape_distance` bounds the distance from chart point q to a shape from below (negative inside). @par Example @code float d = shape_distance(scene.shapes[0], q); @endcode */
static float shape_distance(device const Shape &sh, float3 q) {
    float3 p = q - sh.centre_kind.xyz, h = sh.half_round.xyz;
    float rounding = sh.half_round.w;
    int kind = int(sh.centre_kind.w);
    if (kind == SHAPE_BOX) {
        float3 d = abs(p) - (h - rounding);
        return length(max(d, 0.0)) + min(max(max(d.x, d.y), d.z), 0.0) - rounding;
    }
    if (kind == SHAPE_CYLINDER) {
        int a = sh.info.y, b = (a + 1) % 3, c = (a + 2) % 3;
        float2 d = float2(length(float2(p[b], p[c])) - (h[b] - rounding), abs(p[a]) - (h[a] - rounding));
        return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0) - rounding;
    }
    return (length(p / h) - 1.0) * min(min(h.x, h.y), h.z);
}

/** @brief `structure_distance` is the smallest distance bound at q over a structure's glass (or other) shapes, and which shape. @par Example @code float d = structure_distance(scene, st, q, false, index); @endcode */
static float structure_distance(thread const Scene &scene, device const VoxelStructure &st, float3 q, bool is_glass, thread int &index) {
    float best = INFINITY;
    for (int i = st.size.x; i < st.size.x + st.size.y; i++) {
        if ((scene.shapes[i].info.x == scene.u.mat_glass) != is_glass) continue;
        float d = shape_distance(scene.shapes[i], q);
        if (d < best) { best = d; index = i; }
    }
    return best;
}

/**
 * @brief `march_shapes` sphere-traces the chart line q0 + dq mu from mu to mu_hi and gives where it meets a shape (or
 * INFINITY); with `should_track_lit` (shadow rays only, since it slows the camera's rays), it lowers `lit` to the
 * least gap to a shape over the chart distance from `mu_from` times `SHADOW_PENUMBRA_TAN`.
 * @par Example
 * @code
 * float mu = march_shapes<false>(scene, st, q0, dq, 0.0, 1.0, false, index, 0.0, lit);
 * @endcode
 */
template <bool should_track_lit>
static float march_shapes(thread const Scene &scene, device const VoxelStructure &st, float3 q0, float3 dq, float mu, float mu_hi, bool is_glass, thread int &index,
                          float mu_from, thread float &lit) {
    float speed = max(length(dq), TINY);
    for (int i = 0; i < MAX_SHAPE_STEPS; i++) {
        float d = structure_distance(scene, st, q0 + dq * mu, is_glass, index);
        if (d < SHAPE_HIT_M) return mu;
        if (should_track_lit) lit = min(lit, d / max((mu - mu_from) * speed * SHADOW_PENUMBRA_TAN, TINY));
        mu += d / speed;
        if (mu > mu_hi) break;
    }
    return INFINITY;
}

/** @brief `hit_mesh_triangle` gives where the chart line q0 + dq mu crosses a triangle (Möller-Trumbore), or INFINITY. @par Example @code float mu = hit_mesh_triangle(scene.mesh_triangles[i], q0, dq); @endcode */
static float hit_mesh_triangle(device const MeshTriangle &t, float3 q0, float3 dq) {
    float3 a = t.p[0].xyz, e1 = t.p[1].xyz - a, e2 = t.p[2].xyz - a, h = cross(dq, e2), s = q0 - a, g = cross(s, e1);
    float det = dot(e1, h);
    if (abs(det) < MESH_PARALLEL_DET * length(dq)) return INFINITY;
    float b1 = dot(s, h) / det, b2 = dot(dq, g) / det;
    return b1 >= 0.0 && b2 >= 0.0 && b1 + b2 <= 1.0 ? dot(e2, g) / det : INFINITY;
}

/** @brief `march_mesh` walks a model's BVH (in `nodes`, over `triangles`) from node `root` and gives where the chart line first meets a triangle within [mu_lo, mu_hi] (or INFINITY), and which. @par Example @code float mu = march_mesh(scene.mesh_triangles, scene.mesh_nodes, st.size.w, q0, dq, 0.0, 1.0, index); @endcode */
static float march_mesh(device const MeshTriangle *triangles, device const BvhNode *nodes, int root, float3 q0, float3 dq, float mu_lo, float mu_hi, thread int &index) {
    int stack[MESH_STACK_SIZE];
    int top = 0;
    stack[top++] = root;
    float3 inv = 1.0 / dq;
    while (top > 0) {
        device const BvhNode &node = nodes[stack[--top]];
        float3 a = (node.lo.xyz - q0) * inv, b = (node.hi.xyz - q0) * inv, near = min(a, b), far = max(a, b);
        if (max(max(max(near.x, near.y), near.z), mu_lo) > min(min(min(far.x, far.y), far.z), mu_hi)) continue;
        int first = int(node.lo.w), count = int(node.hi.w);
        if (count == 0) {
            if (top + 2 <= MESH_STACK_SIZE) { stack[top++] = first; stack[top++] = first + 1; }
            continue;
        }
        for (int i = first; i < first + count; i++) {
            float mu = hit_mesh_triangle(triangles[i], q0, dq);
            if (mu >= mu_lo && mu < mu_hi) { mu_hi = mu; index = i; }
        }
    }
    return index >= 0 ? mu_hi : INFINITY;
}

/**
 * @brief `trace_structure` finds where a chord first enters a solid cell of structure k, by a voxel walk along the
 * chord's straight line in the structure's Klein coordinates (taken at the chord's closest approach for precision);
 * glass is noted in `glass_s` (and ignored by shadow rays).
 * @par Example
 * @code
 * trace_structure(scene, k, x, v, 0.0, false, hit, glass_s);
 * @endcode
 */
static void trace_structure(thread const Scene &scene, int k, float4 x, float4 v, float s_min, bool is_shadow, thread Hit &hit, thread float &glass_s) {
    device const VoxelStructure &st = scene.structures[k];
    float4 c = st.centre, info = st.bound;
    float a = -mdot(x, c), b = mdot(v, c), closest_sq = a * a - b * b;
    if (closest_sq >= info.x * info.x) return;
    float s_star = clamp_atanh(b / a), half_window = acosh(info.x * rsqrt(max(closest_sq, 1.0)));
    float s_lo = max(s_min, s_star - half_window), s_hi = min(hit.s, s_star + half_window);
    if (s_lo >= s_hi) return;
    float4 xs = geodesic_point(x, v, s_star), vs = geodesic_tangent(x, v, s_star);
    float4 p = float4(dot(st.rows[0], xs), dot(st.rows[1], xs), dot(st.rows[2], xs), dot(st.rows[3], xs));
    float4 t = float4(dot(st.rows[0], vs), dot(st.rows[1], vs), dot(st.rows[2], vs), dot(st.rows[3], vs));
    float lam_lo = tanh(s_lo - s_star), lam_hi = tanh(s_hi - s_star);
    float mu_lo = lam_lo / (p.w * (p.w + lam_lo * t.w)), mu_hi = lam_hi / (p.w * (p.w + lam_hi * t.w));
    float scale = st.scale.x;
    float3 q0 = p.xyz / p.w * scale, dq = (t.xyz * p.w - p.xyz * t.w) * scale;
    int index = st.size.x;
    float mu;
    bool is_avatar = st.size.w <= FIRST_AVATAR_ROOT_CODE, is_mesh = is_avatar || st.size.w >= 0;
    if (is_mesh) {
        index = -1;
        mu = is_avatar ? march_mesh(scene.avatar_triangles, scene.avatar_nodes, FIRST_AVATAR_ROOT_CODE - st.size.w, q0, dq, mu_lo, mu_hi, index)
                       : march_mesh(scene.mesh_triangles, scene.mesh_nodes, st.size.w, q0, dq, mu_lo, mu_hi, index);
    } else if (is_shadow) {
        float lam_from = -tanh(s_star), mu_from = lam_from / (p.w * (p.w + lam_from * t.w));
        mu = march_shapes<true>(scene, st, q0, dq, mu_lo, mu_hi, false, index, mu_from, hit.lit);
    } else {
        int glass_index;
        mu = march_shapes<false>(scene, st, q0, dq, mu_lo, mu_hi, false, index, 0.0, hit.lit);
        float mu_glass = march_shapes<false>(scene, st, q0, dq, mu_lo, min(mu, mu_hi), true, glass_index, 0.0, hit.lit);
        if (mu_glass < INFINITY) glass_s = min(glass_s, s_star + clamp_atanh(mu_glass * p.w * p.w / (1.0 - mu_glass * p.w * t.w)));
    }
    if (mu == INFINITY) return;
    float s_hit = s_star + clamp_atanh(mu * p.w * p.w / (1.0 - mu * p.w * t.w));
    if (s_hit < hit.s) {
        hit.s = s_hit; hit.mat = is_mesh ? MAT_MESH : scene.shapes[index].info.x; hit.k = k; hit.axis = 0;
        hit.sgn = 1.0; hit.g = q0 + dq * mu; hit.cell = int3(index, is_avatar ? CELL_AVATAR : is_mesh ? CELL_MAP_MODEL : 0, 0);
    }
}

/** @brief `trace_ground` intersects a chord with the ground y = 0. @par Example @code trace_ground(x, v, hit); @endcode */
static void trace_ground(float4 x, float4 v, thread Hit &hit) {
    if (x.y < 0.0 || v.y >= 0.0) return;
    float lam = x.y / -v.y;
    if (lam >= LAMBDA_LIMIT) return;
    float s = atanh(lam);
    if (s < hit.s) { hit.s = s; hit.mat = MAT_GROUND; hit.k = -1; }
}

/** @brief `terrain_hash` is lattice point (i, j)'s value in [0, 1], as terrain.rs's `hash`. @par Example @code float h = terrain_hash(3, -2, 1u); @endcode */
static float terrain_hash(int i, int j, uint seed) {
    uint h = uint(i) * 0x8da6b343u ^ uint(j) * 0xd8163841u ^ seed * 0xcb1ab31fu;
    h ^= h >> 13;
    h *= 0x5bd1e995u;
    h ^= h >> 15;
    return float(h) * (1.0 / 4294967295.0);
}

/** @brief `smooth01` is the cubic smoothstep of t clamped to [0, 1]. @par Example @code float w = smooth01(t); @endcode */
static float smooth01(float t) { t = clamp(t, 0.0, 1.0); return t * t * (3.0 - 2.0 * t); }

/** @brief `terrain_noise` is terrain.rs's smooth value noise in [0, 1]. @par Example @code float n = terrain_noise(p, 1u); @endcode */
static float terrain_noise(float2 p, uint seed) {
    float2 c = floor(p), f = float2(smooth01(p.x - c.x), smooth01(p.y - c.y));
    int i = int(c.x), j = int(c.y);
    float bottom = mix(terrain_hash(i, j, seed), terrain_hash(i + 1, j, seed), f.x);
    float top = mix(terrain_hash(i, j + 1, seed), terrain_hash(i + 1, j + 1, seed), f.x);
    return mix(bottom, top, f.y);
}

/** @brief `mound_height_m` is terrain.rs's `MoundGrid::get_height_m`: the baked mound grid (origin u, v, cells per meter, points along u in `mound_grid`, points along v in `relief.w`) read bilinearly at Fermi (u, v), 0 outside. @par Example @code float h = mound_height_m(scene, u_m, v_m); @endcode */
static float mound_height_m(thread const Scene &scene, float u_m, float v_m) {
    float4 g = scene.u.mound_grid;
    float2 x = (float2(u_m, v_m) - g.xy) * g.z, i = floor(x), f = x - i;
    if (any(i < 0.0) || i.x >= g.w - 1.0 || i.y >= scene.u.relief.w - 1.0) return 0.0;
    int k = int(i.x) + int(g.w) * int(i.y), n = int(g.w);
    float bottom = mix(scene.mound_heights[k], scene.mound_heights[k + 1], f.x);
    float top = mix(scene.mound_heights[k + n], scene.mound_heights[k + n + 1], f.x);
    return mix(bottom, top, f.y);
}

/** @brief `terrain_height_m` is terrain.rs's `get_height_m`: the ground's height at Fermi (u, v), mounds included. @par Example @code float h = terrain_height_m(scene, u_m, v_m); @endcode */
static float terrain_height_m(thread const Scene &scene, float u_m, float v_m) {
    float4 t = scene.u.terrain, rl = scene.u.relief;
    uint seed = scene.u.terrain_seed;
    float inside_m = (1.0 - length(float2(u_m / t.x, v_m / t.y))) * t.y;
    float wall = inside_m >= 0.0 ? smooth01(1.0 - inside_m / t.z) : 1.0 - smooth01(-inside_m / (TERRAIN_OUTER_FLANK_PER_WALL * t.z));
    float rim_m = t.w * (1.0 + TERRAIN_RIM_NOISE_FRAC * (terrain_noise(float2(u_m, v_m) / TERRAIN_RIM_NOISE_M, seed ^ TERRAIN_RIM_SEED) - 0.5));
    float2 p = float2(u_m, v_m) / rl.y;
    float sum = 0.0, amplitude = 0.5;
    for (uint o = 0; o < uint(TERRAIN_OCTAVES); o++) { sum += amplitude * terrain_noise(p, seed + o); amplitude *= 0.5; p *= 2.0; }
    float mounds_m = mound_height_m(scene, u_m, v_m);
    return rim_m * wall + mounds_m + rl.x * sum / (1.0 - exp2(-float(TERRAIN_OCTAVES))) * (1.0 + TERRAIN_WALL_RELIEF_GAIN * wall);
}

/** @brief `get_fermi` gives a point's Fermi (u, v, h) in meters. @par Example @code float3 f = get_fermi(x, r); @endcode */
static float3 get_fermi(float4 x, float r) {
    return float3(-r * clamp_atanh(x.z / x.w), r * asinh(x.x * rsqrt(1.0 + x.y * x.y)), r * asinh(x.y));
}

/**
 * @brief `trace_terrain` is terrain.rs's `find_hit_s` on the chord from x along v from `s_start` to `hit.s`: steps of a
 * fraction of the gap to the ground, at least a little more each meter out, then bisection of the step that went under; with
 * `should_track_lit` (shadow rays only), it lowers `hit.lit` to the least gap over the distance travelled times
 * `SHADOW_PENUMBRA_TAN`.
 * @par Example
 * @code
 * trace_terrain<false>(scene, x, v, 0.0, hit);
 * @endcode
 */
template <bool should_track_lit>
static void trace_terrain(thread const Scene &scene, float4 x, float4 v, float s_start, thread Hit &hit) {
    float r = scene.u.curvature_radius_m, top_m = scene.u.relief.z, s = s_start, previous = s_start;
    for (int i = 0; i < TERRAIN_MAX_STEPS; i++) {
        float4 p = geodesic_point(x, v, s);
        float3 f = get_fermi(p, r);
        float gap = f.z - terrain_height_m(scene, f.x, f.y);
        if (gap < 0.0) {
            float lo = previous, hi = s;
            for (int b = 0; b < TERRAIN_BISECTIONS; b++) {
                float mid = 0.5 * (lo + hi);
                float3 g = get_fermi(geodesic_point(x, v, mid), r);
                if (g.z < terrain_height_m(scene, g.x, g.y)) hi = mid; else lo = mid;
            }
            if (hi < hit.s) { hit.s = hi; hit.mat = MAT_GROUND; hit.k = -1; }
            return;
        }
        if (should_track_lit) hit.lit = min(hit.lit, gap / max(s * r * SHADOW_PENUMBRA_TAN, TINY));
        if (f.z > top_m && geodesic_tangent(x, v, s).y > 0.0) return;
        previous = s;
        s += max(TERRAIN_STEP_FRAC * gap, TERRAIN_MIN_STEP_M + TERRAIN_STEP_PER_M * s * r) / r;
        if (s > hit.s) return;
    }
}

/**
 * @brief `terrain_normal` is the unit normal of the terrain at Fermi (u, v, h): ∂h less the height's gradient along
 * ∂u and ∂v, which are orthogonal with lengths cosh h cosh v and cosh h.
 * @par Example
 * @code
 * float4 n = terrain_normal(scene, f);
 * @endcode
 */
static float4 terrain_normal(thread const Scene &scene, float3 f) {
    float r = scene.u.curvature_radius_m, d = TERRAIN_NORMAL_STEP_M;
    float g_u = (terrain_height_m(scene, f.x + d, f.y) - terrain_height_m(scene, f.x - d, f.y)) / (2.0 * d);
    float g_v = (terrain_height_m(scene, f.x, f.y + d) - terrain_height_m(scene, f.x, f.y - d)) / (2.0 * d);
    float3 c = cosh(f / r), sh = sinh(f / r);
    float4 du = float4(0.0, 0.0, -c.z * c.y * c.x, c.z * c.y * sh.x);
    float4 dv = float4(c.z * c.y, 0.0, -c.z * sh.y * sh.x, c.z * sh.y * c.x);
    float4 dh = float4(sh.z * sh.y, c.z, -sh.z * c.y * sh.x, sh.z * c.y * c.x);
    float4 n = dh - g_u / (c.z * c.z * c.y * c.y) * du - g_v / (c.z * c.z) * dv;
    return n * rsqrt(max(mdot(n, n), TINY));
}

/**
 * @brief `is_clear_of_blocks` says whether no static structure's solid cell is within `s` of the point whose spatial
 * part in warp w's frame is `local`, so a chord of length `s` from it can't reach one: its clearance grid cell's value
 * less its chart distance from the cell's centre (the metric is at most |dp|) is at least `s`. False outside the grid.
 * @par Example
 * @code
 * if (is_clear_of_blocks(scene, w, local, hit.s)) first = list.z;
 * @endcode
 */
static bool is_clear_of_blocks(thread const Scene &scene, int w, float3 local, float s) {
    device const VoxelWarp &warp = scene.warps[w];
    float4 grid = warp.grid;
    int n = int(grid.y);
    float3 g = (local + grid.x) * grid.z;
    int3 c = int3(floor(g));
    if (any(c < int3(0)) || any(c >= int3(n))) return false;
    float off_centre = length(g - float3(c) - 0.5) / grid.z;
    return float(scene.clearances[warp.list.w + c.x + n * (c.y + n * c.z)]) * grid.w - off_centre >= s;
}

/**
 * @brief `trace_scene` finds the first surface along a chord: the ground and every structure, or the structures in a
 * warp's list whose distance from its centre the chord can reach (the chord starts `rho` from the centre and is
 * `hit.s` long, so it stays within [rho - hit.s, rho + hit.s]), only its avatars where the warp's clearance grid shows
 * no static block within reach.
 * @par Example
 * @code
 * trace_scene(scene, x, v, -1, 0.0, float3(0.0), hit, glass_s);
 * @endcode
 */
static void trace_scene(thread const Scene &scene, float4 x, float4 v, int warp, float rho, float3 local, thread Hit &hit, thread float &glass_s) {
    if (scene.u.has_terrain != 0) trace_terrain<false>(scene, x, v, 0.0, hit); else trace_ground(x, v, hit);
    if (warp < 0) {
        for (int k = 0; k < scene.u.structure_count; k++) trace_structure(scene, k, x, v, 0.0, false, hit, glass_s);
    } else {
        int4 list = scene.warps[warp].list;
        float reach = hit.s;
        for (int j = is_clear_of_blocks(scene, warp, local, reach) ? list.z : 0; j < list.y; j++) {
            float4 entry = scene.warp_lists[list.x + j];
            if (rho + reach < entry.y || rho - reach > entry.z) continue;
            trace_structure(scene, int(entry.x), x, v, 0.0, false, hit, glass_s);
        }
    }
}

/**
 * @brief `trace_shadow` gives 0 if something blocks the straight path toward the sun within `SHADOW_M`, else how lit
 * x is, from 0 to 1: a path that passes close to a surface, compared with how far it has gone, is in the penumbra. The
 * terrain counts only from `TERRAIN_SHADOW_START_M` on, so a ground point doesn't shadow itself.
 * @par Example
 * @code
 * float lit = trace_shadow(scene, x, l);
 * @endcode
 */
static float trace_shadow(thread const Scene &scene, float4 x, float4 l) {
    Hit h;
    h.s = SHADOW_M / scene.u.curvature_radius_m;
    h.mat = 0;
    h.lit = 1.0;
    float glass_s = NO_HIT_S;
    if (scene.u.has_terrain != 0) {
        trace_terrain<true>(scene, x, l, TERRAIN_SHADOW_START_M / scene.u.curvature_radius_m, h);
        if (h.mat != 0) return SHADOW_FLOOR;
    }
    for (int k = 0; k < scene.u.structure_count; k++) {
        trace_structure(scene, k, x, l, 0.0, true, h, glass_s);
        if (h.mat != 0) return SHADOW_FLOOR;
    }
    return smoothstep(0.0, 1.0, h.lit);
}

/**
 * @brief `get_lens_log_index` gives a black hole's wide lens (voxel_world.rs's `get_lens_log_index`, W-107) at distance
 * `rho`: (d ln n / dρ, ln n) with ln n = h a / √(ρ² + a²) for the most it turns a ray h (`lens.x`) and its core radius
 * a (`lens.y`); none for a mouth.
 * @par Example
 * @code
 * float2 lens = get_lens_log_index(scene, w, rho);
 * @endcode
 */
static float2 get_lens_log_index(thread const Scene &scene, int w, float rho) {
    float4 lens = scene.warps[w].lens;
    if (lens.x == 0.0) return float2(0.0);
    float root = sqrt(rho * rho + lens.y * lens.y);
    return float2(-lens.x * lens.y * rho / (root * root * root), lens.x * lens.y / root);
}

/** @brief `get_index_slope` gives d ln n / dρ of a warp's index (isotropic Schwarzschild with its wide lens, or foil). @par Example @code float s = get_index_slope(scene, w, rho); @endcode */
static float get_index_slope(thread const Scene &scene, int w, float rho) {
    float4 prm = scene.warps[w].params;
    if (prm.x < HALF_KIND) { float m = prm.y, q = m / rho; return -m / (rho * rho) * (3.0 / (1.0 + q) + 1.0 / max(1.0 - q, HORIZON_GUARD)) + get_lens_log_index(scene, w, rho).x; }
    float pt = prm.y, ir = 1.0 / rho, psi = 1.0 + pt * ir, ir2 = ir / (psi * psi), rs = 4.0 * pt;
    return -2.0 * pt * ir * ir / psi - rs * psi * (1.0 - pt * ir) * ir2 * ir2 / (2.0 * (1.0 + FOIL_LAMBDA_SQ - rs * ir2));
}

/** @brief `get_log_index` gives ln n of a warp's index. @par Example @code float ln_n = get_log_index(scene, w, rho); @endcode */
static float get_log_index(thread const Scene &scene, int w, float rho) {
    float4 prm = scene.warps[w].params;
    if (prm.x < HALF_KIND) { float q = prm.y / rho; return 3.0 * log(1.0 + q) - log(max(1.0 - q, HORIZON_GUARD)) + get_lens_log_index(scene, w, rho).y; }
    float pt = prm.y, psi = 1.0 + pt / rho, r = rho * psi * psi;
    return 2.0 * log(psi) - 0.5 * log((1.0 - 4.0 * pt / r + FOIL_LAMBDA_SQ) / (1.0 + FOIL_LAMBDA_SQ));
}

/**
 * @brief `compute_gradient` gives ∇ln n of warp w at x (not yet projected across a direction), ln n tapered by
 * (1 - (ρ/reach)²)², with x's distance ρ from the warp's centre and its spatial part `local` in the warp's frame.
 * @par Example
 * @code
 * float4 g = compute_gradient(scene, w, x, rho, local);
 * @endcode
 */
static float4 compute_gradient(thread const Scene &scene, int w, float4 x, thread float &rho, thread float3 &local) {
    device const VoxelWarp &warp = scene.warps[w];
    float4 p = warp.inverse * x;
    local = p.xyz;
    rho = asinh(length(p.xyz));
    float ch = cosh(rho), sh = max(sinh(rho), TINY), reach = warp.params.z;
    float f = rho / reach, window = (1.0 - f * f) * (1.0 - f * f), window_slope = -4.0 * f * (1.0 - f * f) / reach;
    float slope = f >= 1.0 ? 0.0 : get_index_slope(scene, w, rho) * window + get_log_index(scene, w, rho) * window_slope;
    return slope * (ch * x - warp.centre) / sh;
}

/** @brief `cross_throat` carries a ray inside mouth w's throat to its partner (inversion, then diag(1, 1, -1)). @par Example @code cross_throat(scene, w, x, v); @endcode */
static void cross_throat(thread const Scene &scene, int w, thread float4 &x, thread float4 &v) {
    device const VoxelWarp &warp = scene.warps[w];
    float4 prm = warp.params, p = warp.inverse * x, t = warp.inverse * v;
    int partner = int(prm.w + 0.5);
    float len = max(length(p.xyz), TINY), rho_out = prm.y * prm.y / asinh(len);
    float3 n = p.xyz / len, flip = float3(1.0, 1.0, -1.0);
    float3 tt = t.xyz - t.w / (1.0 + p.w) * p.xyz;
    tt = (tt - 2.0 * dot(tt, n) * n) * flip;
    float4 q = float4(n * flip * sinh(rho_out), cosh(rho_out));
    float g = dot(q.xyz, tt) / (1.0 + q.w);
    x = scene.warps[partner].forward * q;
    v = normalize_tangent(x, scene.warps[partner].forward * float4(tt + g * q.xyz, g * (1.0 + q.w)));
}

/** @brief `Surface` is a shading point's material: albedo, perceptual roughness, metalness, ambient occlusion and normal. */
struct Surface {
    float3 albedo;
    float roughness;
    float metallic;
    float ao;
    float4 n;
};

/** @brief `orthonormal` makes tangent t at a point unit and perpendicular to n. @par Example @code float4 t = orthonormal(t, n); @endcode */
static float4 orthonormal(float4 t, float4 n) { t -= mdot(t, n) * n; return t * rsqrt(max(mdot(t, t), TINY)); }

/** @brief `has_texture` says whether material m has a loaded texture set. @par Example @code if (has_texture(scene, m)) ...; @endcode */
static bool has_texture(thread const Scene &scene, int m) {
    int layer = int(scene.surfaces[m].x);
    return layer >= 0 && scene.means[layer].w > 0.0;
}

/**
 * @brief `sample_surface` reads material m's texture set at uv (in blocks of block_m meters), at the level of detail of
 * the pixel's footprint: the albedo is `tint` scaled by the texture's brightness over its mean, and the normal map bends n along tangents t, b.
 * Untextured materials get `tint` with a rough, dielectric finish.
 * @par Example
 * @code
 * Surface s = sample_surface(scene, m, albedo, uv, 1.0, dist_s, x, n, t, b, v);
 * @endcode
 */
static Surface sample_surface(thread const Scene &scene, int m, float3 tint, float2 uv, float block_m, float dist_s,
                              float4 x, float4 n, float4 t, float4 b, float4 v) {
    Surface s = {tint, UNTEXTURED_ROUGHNESS, 0.0, 1.0, n};
    if (!has_texture(scene, m)) return s;
    constexpr sampler wrap(address::repeat, filter::linear, mip_filter::linear);
    float4 info = scene.surfaces[m];
    int layer = int(info.x);
    float pixel_m = scene.u.pixel_angle_rad * scene.u.curvature_radius_m * sinh(min(dist_s, FAR_S)) * rsqrt(max(abs(mdot(n, v)), MIN_FACING));
    float lod = log2(max(pixel_m * TEXTURE_SIDE_PX / (info.y * block_m), 1.0));
    float2 tex = uv / info.y;
    float3 albedo = scene.albedo_maps.sample(wrap, tex, layer, level(lod)).rgb;
    float2 bend = scene.normal_maps.sample(wrap, tex, layer, level(lod)).rg * 2.0 - 1.0;
    float3 arm = scene.arm_maps.sample(wrap, tex, layer, level(lod)).rgb;
    s.albedo = tint * dot(albedo, LUMA) / max(dot(scene.means[layer].rgb, LUMA), MIN_MEAN_ALBEDO);
    s.ao = arm.r;
    s.roughness = max(arm.g, MIN_ROUGHNESS);
    s.metallic = arm.b * info.z;
    s.n = normalize_tangent(x, t * bend.x + b * bend.y + n * sqrt(saturate(1.0 - dot(bend, bend))));
    return s;
}

/**
 * @brief `evaluate_direct_light` is the fps_demo's GGX specular plus (lunar-Lambert) diffuse for one light, from the
 * Minkowski products of the normal, view, light and half vectors at the surface point.
 * @par Example
 * @code
 * float3 c = evaluate_direct_light(s, n_dot_l, n_dot_v, n_dot_h, v_dot_h, energy);
 * @endcode
 */
static float3 evaluate_direct_light(Surface s, float n_dot_l, float n_dot_v, float n_dot_h, float v_dot_h, float3 energy) {
    if (n_dot_l <= 0.0) return float3(0.0);
    n_dot_h = saturate(n_dot_h);
    v_dot_h = saturate(v_dot_h);
    float alpha = s.roughness * s.roughness, alpha_squared = alpha * alpha;
    float d_denominator = n_dot_h * n_dot_h * (alpha_squared - 1.0) + 1.0;
    float d = alpha_squared / (M_PI_F * d_denominator * d_denominator);
    float visibility = 0.5 / max(n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - alpha_squared) + alpha_squared)
                               + n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - alpha_squared) + alpha_squared), 1e-5);
    float3 f0 = mix(float3(0.04), s.albedo, s.metallic);
    float3 f = f0 + (1.0 - f0) * pow(1.0 - v_dot_h, 5.0);
    float lunar = mix(1.0, 2.0 / (n_dot_l + n_dot_v), LUNAR_LAMBERT_FRAC);
    return ((1.0 - f) * (1.0 - s.metallic) * s.albedo / M_PI_F * lunar + d * visibility * f * energy) * n_dot_l;
}

/** @brief `light_from` is a surface's light from unit direction l, per unit of irradiance. @par Example @code float3 c = light_from(s, l, view, energy); @endcode */
static float3 light_from(Surface s, float4 l, float4 view, float3 energy) {
    float4 h = l + view;
    h *= rsqrt(max(mdot(h, h), TINY));
    return evaluate_direct_light(s, mdot(s.n, l), max(mdot(s.n, view), 1e-4), mdot(s.n, h), mdot(view, h), energy);
}

/** @brief `get_env_brdf` is Karis' fit of the split-sum specular scale and bias. @par Example @code float2 ab = get_env_brdf(0.5, n_dot_v); @endcode */
static float2 get_env_brdf(float roughness, float n_dot_v) {
    float4 r = roughness * float4(-1.0, -0.0275, -0.572, 0.022) + float4(1.0, 0.0425, 1.04, -0.04);
    float a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    return float2(-1.04, 1.04) * a004 + r.zw;
}

/** @brief `get_lamp_light` gives lamp i's light reaching x (black past its cutoff) and the unit direction toward it. @par Example @code float3 c = get_lamp_light(scene, i, x, toward); @endcode */
static float3 get_lamp_light(thread const Scene &scene, int i, float4 x, thread float4 &toward) {
    float4 lx = scene.lights[i].position;
    float d_m = measure_gap(lx, x) * scene.u.curvature_radius_m, radius_m = scene.lights[i].color_radius.w;
    toward = normalize_tangent(x, lx);
    if (d_m > LIGHT_CUTOFF_FRAC * radius_m) return float3(0.0);
    return scene.lights[i].color_radius.rgb / (1.0 + (d_m / radius_m) * (d_m / radius_m))
        * (1.0 - smoothstep(LIGHT_FADE_START_FRAC * LIGHT_CUTOFF_FRAC * radius_m, LIGHT_CUTOFF_FRAC * radius_m, d_m));
}

/** @brief `gather_lights` sums the lamps' light at a surface point. @par Example @code float3 lamps = gather_lights(scene, s, x, view, energy); @endcode */
static float3 gather_lights(thread const Scene &scene, Surface s, float4 x, float4 view, float3 energy) {
    float3 sum = float3(0.0);
    for (int i = 0; i < scene.u.light_count; i++) {
        float4 toward;
        float3 light = get_lamp_light(scene, i, x, toward);
        if (all(light == 0.0)) continue;
        float3 wrap = LIGHT_WRAP * s.albedo * (1.0 - s.metallic);
        sum += light * (wrap + (1.0 - LIGHT_WRAP) * M_PI_F * light_from(s, toward, view, energy));
    }
    return sum;
}

/**
 * @brief `light_flashlight` gives the light of the player's flashlight (`flashlight_position`, aimed along
 * `flashlight_direction`; `flashlight` holds the cosines of its cone's inner and outer edges, the distance at which it
 * falls to half, and its brightness, 0 when off) on a surface at x with normal n: full inside the cone, softening to
 * nothing at its outer edge, falling with distance, and as much as the surface faces it.
 * @par Example
 * @code
 * float3 lit = light_flashlight(scene, x, n);
 * @endcode
 */
static float3 light_flashlight(thread const Scene &scene, float4 x, float4 n) {
    float4 p = scene.u.flashlight_position, f = scene.u.flashlight;
    if (f.w <= 0.0) return float3(0.0);
    float4 toward = x + mdot(x, p) * p, back = p + mdot(p, x) * x;
    float cone = smoothstep(f.y, f.x, mdot(toward, scene.u.flashlight_direction) * rsqrt(max(mdot(toward, toward), TINY)));
    float d_m = acosh(max(-mdot(x, p), 1.0)) * scene.u.curvature_radius_m / f.z;
    float facing = max(mdot(n, back) * rsqrt(max(mdot(back, back), TINY)), 0.0);
    return SUN_INTENSITY * FLASHLIGHT_COLOR * f.w * cone * facing / (1.0 + d_m * d_m);
}

/**
 * @brief `light_surface` lights a surface point with face normal n: the sun (GGX with lunar-Lambert diffuse, shadowed),
 * earthshine, the haze's skylight and the ambient light (split-sum specular with multiple-scattering compensation), lamps
 * and the laser spot.
 * @par Example
 * @code
 * float3 c = light_surface(scene, s, x, n, v);
 * @endcode
 */
static float3 light_surface(thread const Scene &scene, Surface s, float4 x, float4 n, float4 v) {
    float4 l = transport_from_centre(scene.u.sun.xyz, x), view = -v;
    float2 env = get_env_brdf(s.roughness, max(mdot(s.n, view), 1e-4));
    float3 f0 = mix(float3(0.04), s.albedo, s.metallic);
    float3 energy = 1.0 + f0 * (1.0 / max(env.x + env.y, 1e-3) - 1.0);
    float3 color = float3(0.0);
    if (mdot(n, l) > 0.0) {
        float sun = trace_shadow(scene, geodesic_point(x, n, SURFACE_OFFSET_M / scene.u.curvature_radius_m), l);
        if (sun > 0.0) color += M_PI_F * SUN_INTENSITY * sun * SUN_COLOR * light_from(s, l, view, energy);
    }
    float earth = max(mdot(s.n, transport_from_centre(scene.u.planet.xyz, x)), 0.0);
    float3 ambient = (float3(AMBIENT) + HAZE_SKYLIGHT * HAZE_COLOR + EARTHSHINE * earth * EARTHSHINE_COLOR) * s.ao;
    color += ambient * (s.albedo * (1.0 - s.metallic) + (f0 * env.x + env.y) * energy) + gather_lights(scene, s, x, view, energy)
        + light_flashlight(scene, x, s.n) * s.albedo * (1.0 - s.metallic);
    if (scene.u.spot_color.a > 0.0) {
        float d_m = measure_gap(scene.u.spot, x) * scene.u.curvature_radius_m;
        color += scene.u.spot_color.rgb * scene.u.spot_color.a * exp(-d_m * d_m / (SPOT_RADIUS_M * SPOT_RADIUS_M));
    }
    return color;
}

/**
 * @brief `HandLight` is viewmodel.metal's `HandLight`: the light at the eye for the held gun, in camera space (+x right,
 * +y up, -z ahead): the unit directions toward the sun, the lamps and the planet (zero when absent) and each one's
 * radiance, the local up, and the radiance of the sky and of the ground.
 */
struct HandLight {
    float4 directions[HAND_LIGHT_COUNT];
    float4 colors[HAND_LIGHT_COUNT];
    float4 up;
    float4 sky;
    float4 ground;
};

/** @brief `to_camera` gives a tangent vector at the eye in camera coordinates. @par Example @code float3 d = to_camera(u, l); @endcode */
static float3 to_camera(constant VoxelUniforms &u, float4 t) { return float3(mdot(t, u.camera[0]), mdot(t, u.camera[1]), mdot(t, u.camera[2])); }

/**
 * @brief `probe_hand_light` writes the light at the eye into `hand`, as `light_surface` lights a surface there: the sun
 * (shadowed), the lamps summed into one light from their weighted mean direction (their wrapped part goes to the
 * sky), earthshine, the ambient light and the haze's skylight as the sky, and the ground (regolith) lit by the sun and the ambient light.
 * @par Example
 * @code
 * dispatched by render_voxel.rs, one thread, before the frame's rays.
 * @endcode
 */
kernel void probe_hand_light(SCENE_BUFFERS, device HandLight &hand [[buffer(HAND_LIGHT_BUFFER_INDEX)]]) {
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    float4 x = u.camera[3], l = transport_from_centre(u.sun.xyz, x), up = normalize_tangent(x, float4(0.0, 1.0, 0.0, 0.0));
    float3 sun = SUN_INTENSITY * SUN_COLOR * trace_shadow(scene, x, l), lamps = float3(0.0);
    float3 ambient = float3(AMBIENT) + HAZE_SKYLIGHT * HAZE_COLOR;
    float4 toward_lamps = float4(0.0);
    for (int i = 0; i < u.light_count; i++) {
        float4 toward;
        float3 light = get_lamp_light(scene, i, x, toward);
        lamps += light;
        toward_lamps += toward * dot(light, float3(1.0));
    }
    float4 directions[HAND_LIGHT_COUNT] = {l, toward_lamps, transport_from_centre(u.planet.xyz, x)};
    float3 colors[HAND_LIGHT_COUNT] = {sun, (1.0 - LIGHT_WRAP) * lamps, EARTHSHINE * EARTHSHINE_COLOR};
    for (int k = 0; k < HAND_LIGHT_COUNT; k++) {
        float3 d = to_camera(u, directions[k]);
        hand.directions[k] = float4(dot(d, d) > TINY ? normalize(d) : float3(0.0), 0.0);
        hand.colors[k] = float4(colors[k], 0.0);
    }
    hand.up = float4(to_camera(u, up), 0.0);
    hand.sky = float4(ambient + LIGHT_WRAP * lamps, 0.0);
    hand.ground = float4(get_palette(scene, u.mat_regolith).rgb * (sun * max(mdot(up, l), 0.0) + ambient), 0.0);
}

/** @brief `grid_normal` is the unit normal, toward +axis, of the grid plane of structure st through local point p. @par Example @code float4 t = grid_normal(st, p, 0); @endcode */
static float4 grid_normal(device const VoxelStructure &st, float4 p, int axis) {
    float c = p[axis] / p.w;
    float4 n_local = float4(0.0);
    n_local[axis] = 1.0;
    n_local.w = c;
    n_local *= rsqrt(max(1.0 - c * c, TINY));
    float4 n = st.rows[0] * n_local.x + st.rows[1] * n_local.y + st.rows[2] * n_local.z - st.rows[3] * n_local.w;
    n.w = -n.w;
    return n;
}

/** @brief `detail_fraction` gives how much per-block detail to keep at a distance. @par Example @code float d = detail_fraction(scene, s); @endcode */
static float detail_fraction(thread const Scene &scene, float dist_s) {
    float pixel_m = scene.u.pixel_angle_rad * scene.u.curvature_radius_m * sinh(min(dist_s, FAR_S));
    return 1.0 - smoothstep(DETAIL_FADE_START_FRAC * DETAIL_FOOTPRINT_M, DETAIL_FOOTPRINT_M, pixel_m);
}

/**
 * @brief `shade_ground` shades the ground at x: the {4,5} tiling folded into the central tile gives block cells, and
 * cell centres decide the regolith, crater floor, ejecta, craterlets, zone tints and the boundary.
 * @par Example
 * @code
 * float3 c = shade_ground(scene, x, v, s);
 * @endcode
 */
static float3 shade_ground(thread const Scene &scene, float4 x, float4 v, float dist_s) {
    constant VoxelUniforms &u = scene.u;
    float r = u.curvature_radius_m;
    float tile_klein_half = tanh(acosh(cos(M_PI_F / 5.0) / sin(M_PI_F / 4.0)));
    float tile_normal_scale = rsqrt(1.0 - tile_klein_half * tile_klein_half);
    float tile_inradius_s = atanh(tile_klein_half), cell_s = tile_inradius_s / float(TILE_CELLS_PER_HALF);
    float detail = detail_fraction(scene, dist_s);
    bool has_terrain = u.has_terrain != 0;
    float3 fermi = get_fermi(x, r);
    float cosh_v = cosh(fermi.y / r);
    float4 foot = has_terrain ? float4(sinh(fermi.y / r), 0.0, -cosh_v * sinh(fermi.x / r), cosh_v * cosh(fermi.x / r)) : x;
    float3 y = float3(foot.x, foot.z, foot.w);
    float3x3 N = float3x3(1.0);
    if (dist_s < GROUND_DETAIL_S) {
        for (int i = 0; i < MAX_FOLDS; i++) {
            float kx = y.x / y.z, kz = y.y / y.z, ex = abs(kx) - tile_klein_half, ez = abs(kz) - tile_klein_half;
            if (max(ex, ez) <= 0.0) break;
            float3 n = (ex > ez ? float3(sign(kx), 0.0, tile_klein_half) : float3(0.0, sign(kz), tile_klein_half)) * tile_normal_scale;
            float3 jn = float3(n.xy, -n.z);
            y -= 2.0 * dot(y, jn) * n;
            float3 Nn = N * n;
            N -= 2.0 * float3x3(Nn * jn.x, Nn * jn.y, Nn * jn.z);
        }
    }
    float2 a = float2(clamp_atanh(y.x / y.z), clamp_atanh(y.y / y.z)) / cell_s;
    float2 cell = floor(a), uv = a - cell, kc = tanh((cell + 0.5) * cell_s);
    float3 centre = N * (float3(kc, 1.0) * rsqrt(max(1.0 - dot(kc, kc), TINY)));
    if (dist_s >= GROUND_DETAIL_S) centre = y;
    if (has_terrain) centre = float3(foot.x, foot.z, foot.w);
    float u_m = -r * clamp_atanh(centre.y / centre.z), v_m = r * asinh(centre.x), r_m = acosh(max(centre.z, 1.0)) * r;
    float h = hash_3d(floor(centre * CELL_HASH_SCALE) * HASH_SHRINK);
    float3 albedo = get_palette(scene, u.mat_regolith).rgb * (MARIA_BASE + MARIA_CONTRAST * sample_fbm_3d(float3(u_m, v_m, 0.0) * MARIA_PER_M));
    bool is_snow = u.snow.x > 0.5;
    if (is_snow) {
        albedo = get_palette(scene, u.mat_regolith).rgb * (1.0 - SNOW_GRAIN + SNOW_GRAIN * sample_fbm_3d(float3(u_m, v_m, 0.0) * 0.35));
    } else if (r_m < CRATER_FLOOR_RADIUS_M) {
        albedo = mix(get_palette(scene, u.mat_dark_regolith).rgb, get_palette(scene, u.mat_basalt).rgb, FLOOR_RING_DARKEN * step(0.5, fract(r_m / FLOOR_RING_M)));
    } else if (r_m < EJECTA_OUTER_M) {
        float theta = atan2(centre.x, -centre.y);
        float ray = smoothstep(EJECTA_RAY_EDGES.x, EJECTA_RAY_EDGES.y, sample_noise_3d(float3(theta * EJECTA_RAY_COUNT, 0.0, 1.0)))
                  * (1.0 - smoothstep(EJECTA_INNER_M, EJECTA_OUTER_M, r_m));
        albedo = mix(albedo, to_linear(EJECTA_COLOR), EJECTA_STRENGTH * ray);
    }
    if (!is_snow && !has_terrain && dist_s < GROUND_DETAIL_S && r_m > CRATER_RIM_RADIUS_M + CRATERLET_CLEARANCE_M) {
        float2 local = (cell + 0.5) * cell_s, best = float2(NO_HIT_S, 0.0);
        for (int i = 0; i < 9; i++) {
            float2 cand = (float2(float(i % 3), float(i / 3)) - 1.0) * tile_inradius_s * CRATERLET_SPACING_FRAC;
            float d_m = length(local - cand) * r;
            if (d_m > CRATERLET_MAX_M) continue;
            float2 kk = tanh(cand);
            float3 cg = N * (float3(kk, 1.0) * rsqrt(1.0 - dot(kk, kk)));
            float hc = hash_3d(floor(cg * CRATERLET_HASH_SCALE) * HASH_SHRINK);
            float radius_m = CRATERLET_MAX_M * (CRATERLET_MIN_FRAC + (1.0 - CRATERLET_MIN_FRAC) * fract(hc * CRATERLET_SIZE_SEED));
            if (hc < CRATERLET_CHANCE && d_m < radius_m) best = float2(d_m / radius_m, 1.0);
        }
        if (best.y > 0.5) {
            albedo = best.x > CRATERLET_RIM_FRAC ? albedo * CRATERLET_RIM_BRIGHTEN : albedo * (CRATERLET_FLOOR_DARKEN + CRATERLET_FLOOR_SLOPE * best.x);
        }
    }
    float3 glow = float3(0.0);
    if (u.has_regions != 0) {
        float zone_edge_u_m = u.regions.x, boundary_u_m = u.regions.y, boundary_v_m = u.regions.z;
        if (abs(u_m) > zone_edge_u_m) albedo *= mix(float3(1.0), (u_m < 0.0 ? BLUE_COLOR : RED_COLOR) * ZONE_TINT_GAIN, ZONE_TINT_FRAC);
        if (u.zone_walls.counts.x == 0 && abs(abs(u_m) - zone_edge_u_m) < ZONE_STRIP_M && abs(v_m) < boundary_v_m) glow = (u_m < 0.0 ? BLUE_COLOR : RED_COLOR) * ZONE_STRIP_GLOW;
        float e = (u_m / boundary_u_m) * (u_m / boundary_u_m) + (v_m / boundary_v_m) * (v_m / boundary_v_m);
        if (has_terrain) {
        } else if (e > 1.0) {
            albedo *= OUTSIDE_DARKEN;
        } else if (e > 1.0 - BOUNDARY_STRIP_FRAC) {
            float stripes = floor(u_m / HAZARD_STRIPE_M) + floor(v_m / HAZARD_STRIPE_M);
            albedo = stripes - 2.0 * floor(stripes / 2.0) < 0.5 ? get_palette(scene, u.mat_hazard).rgb : to_linear(HAZARD_DARK_COLOR);
        }
    }
    float texel = has_texture(scene, u.mat_regolith) || is_snow ? 0.5 : hash_3d(float3(floor(uv * SUBTEXELS), h * CELL_SEED));
    float4 n = float4(0.0, 1.0, 0.0, 0.0);
    if (has_terrain) {
        n = terrain_normal(scene, fermi);
    } else if (!is_snow) {
        albedo *= 1.0 + detail * (BLOCK_NOISE * (h - 0.5) + TEXEL_NOISE * (texel - 0.5));
        float2 edge = min(uv, 1.0 - uv);
        albedo *= mix(1.0, mix(EDGE_DARKEN, 1.0, smoothstep(0.0, EDGE_WIDTH, min(edge.x, edge.y))), detail * GROUND_EDGE_FRAC);
    }
    float3 tx = N * (float3(1.0, 0.0, 0.0) + y.x * y), tz = N * (float3(0.0, 1.0, 0.0) + y.y * y);
    float4 t4 = float4(tx.x, 0.0, tx.y, tx.z), b4 = float4(tz.x, 0.0, tz.y, tz.z);
    t4 += mdot(t4, x) * x;
    b4 += mdot(b4, x) * x;
    float4 t = orthonormal(t4, n), b = -orthonormal(b4, n);
    Surface s = sample_surface(scene, u.mat_regolith, albedo, a, cell_s * r, dist_s, x, n, t, b, v);
    return light_surface(scene, s, x, n, v) + glow;
}

/**
 * @brief `shade_block` shades a block face hit at x: its material with the style's patterns (solar cells, hazard
 * stripes, foil, panel seams, consoles), per-block and per-texel noise, darkened edges, and light with the face's
 * normal; glowing materials are unlit.
 * @par Example
 * @code
 * float3 c = shade_block(scene, hit, x, v, s);
 * @endcode
 */
static float3 shade_block(thread const Scene &scene, Hit h, float4 x, float4 v, float dist_s) {
    constant VoxelUniforms &u = scene.u;
    device const VoxelStructure &st = scene.structures[h.k];
    float4 r0 = st.rows[0], r1 = st.rows[1], r2 = st.rows[2], r3 = st.rows[3];
    float4 p = float4(dot(r0, x), dot(r1, x), dot(r2, x), dot(r3, x));
    device const Shape &sh = scene.shapes[h.cell.x];
    float3 q = h.g, grad = float3(0.0);
    for (int i = 0; i < 3; i++) {
        float3 e = float3(0.0);
        e[i] = SHAPE_NORMAL_M;
        grad[i] = shape_distance(sh, q + e) - shape_distance(sh, q - e);
    }
    grad = normalize(grad);
    float4 n_local = float4(grad, dot(grad, p.xyz / p.w));
    n_local *= rsqrt(max(1.0 - n_local.w * n_local.w, TINY));
    float4 n = r0 * n_local.x + r1 * n_local.y + r2 * n_local.z - r3 * n_local.w;
    n.w = -n.w;
    float3 a_grad = abs(grad);
    int axis = a_grad.x > a_grad.y && a_grad.x > a_grad.z ? 0 : (a_grad.y > a_grad.z ? 1 : 2);
    float4 t = orthonormal(grid_normal(st, p, (axis + 1) % 3), n), b = -orthonormal(grid_normal(st, p, (axis + 2) % 3), n);
    float2 g = float2(q[(axis + 1) % 3], q[(axis + 2) % 3]), uv = fract(g);
    int m = h.mat;
    float4 entry = get_palette(scene, m);
    float3 albedo = entry.rgb;
    float glow = entry.a, block = hash_3d(float3(h.cell) + float(h.k) * STRUCT_SEED), detail = detail_fraction(scene, dist_s);
    float stripe = step(0.5, fract((uv.x + uv.y) * STRIPES_PER_BLOCK));
    if (m >= u.mat_console_first && m < u.mat_console_first + u.mat_console_count) albedo *= 1.0 - CONSOLE_STRIPE_DARKEN * (1.0 - stripe);
    if (m == u.mat_solar) albedo = mix(albedo, to_linear(SOLAR_GRID_COLOR), step(SOLAR_GRID_EDGE, max(fract(uv.x * SOLAR_CELLS.x), fract(uv.y * SOLAR_CELLS.y))));
    if (m == u.mat_hazard) albedo = mix(albedo, to_linear(HAZARD_STRIPE_COLOR), stripe);
    if (m == u.mat_gold_foil) albedo *= 1.0 - FOIL_VARIATION * (0.5 - hash_3d(float3(floor(uv * FOIL_CRINKLES), block)));
    if (m == u.mat_hull || m == u.mat_pad) albedo *= 1.0 - PANEL_SEAM_DARKEN * step(PANEL_SEAM_EDGE, max(fract(uv.x * PANEL_SEAMS), fract(uv.y * PANEL_SEAMS)));
    float texel = has_texture(scene, m) ? 0.5 : hash_3d(float3(floor(uv * SUBTEXELS), block * TEXEL_SEED));
    albedo *= 1.0 + detail * (BLOCK_NOISE * (block - 0.5) + TEXEL_NOISE * (texel - 0.5));
    if (glow > 0.5 && st.size.z != 0) { albedo *= DARK_PYLON_GLOW_FRAC; glow = 0.0; }
    if (glow > 0.5) return albedo * (GLOW_BRIGHTNESS + GLOW_RIM);
    Surface s = sample_surface(scene, m, albedo, g, u.curvature_radius_m / st.scale.x, dist_s, x, n, t, b, v);
    return light_surface(scene, s, x, n, v);
}

/** @brief `to_world` carries a local vector (c, w) of structure st to the world, by the transpose of its inverse frame's rows. @par Example @code float4 n = to_world(st, grad, 0.0); @endcode */
static float4 to_world(device const VoxelStructure &st, float3 c, float w) {
    float4 o = st.rows[0] * c.x + st.rows[1] * c.y + st.rows[2] * c.z - st.rows[3] * w;
    o.w = -o.w;
    return o;
}

/**
 * @brief `recolor_body` turns colour c of the body model into team colour `team`: it keeps c's grey part and swaps its
 * colourful part (the teal) for one of `team`'s hue with the same luminance, so grey metal and shading stay as they were.
 * @par Example
 * @code
 * base.rgb = recolor_body(base.rgb, get_palette(scene, int(st.scale.z)).rgb);
 * @endcode
 */
static float3 recolor_body(float3 c, float3 team) {
    float3 grey = float3(min(c.r, min(c.g, c.b))), hue = team - min(team.r, min(team.g, team.b));
    return grey + hue * dot(c - grey, LUMA) / max(dot(hue, LUMA), TINY);
}

/**
 * @brief `shade_mesh` shades a model triangle hit at x: its material's base colour, metallic-roughness, normal and
 * emissive maps at the hit's texture coordinates, with the interpolated normal turned toward the viewer, and a body's teal
 * turned into its team colour; a structure whose first shape is a console (a beacon) glows in the console's colour while
 * the console is on.
 * @par Example
 * @code
 * float3 c = shade_mesh(scene, hit, x, v, s);
 * @endcode
 */
static float3 shade_mesh(thread const Scene &scene, Hit h, float4 x, float4 v, float dist_s) {
    device const VoxelStructure &st = scene.structures[h.k];
    float4 r0 = st.rows[0], r1 = st.rows[1], r2 = st.rows[2], r3 = st.rows[3];
    float4 p = float4(dot(r0, x), dot(r1, x), dot(r2, x), dot(r3, x));
    device const MeshTriangle &tri = (h.cell.y == CELL_AVATAR ? scene.avatar_triangles : scene.mesh_triangles)[h.cell.x];
    device const MeshMaterial &mat = scene.mesh_materials[tri.info.x];
    float3 a = tri.p[0].xyz, e1 = tri.p[1].xyz - a, e2 = tri.p[2].xyz - a, d = h.g - a;
    float d11 = dot(e1, e1), d12 = dot(e1, e2), d22 = dot(e2, e2), det = max(d11 * d22 - d12 * d12, TINY);
    float b1 = (d22 * dot(d, e1) - d12 * dot(d, e2)) / det, b2 = (d11 * dot(d, e2) - d12 * dot(d, e1)) / det, b0 = 1.0 - b1 - b2;
    float2 uv0 = float2(tri.p[0].w, tri.n[0].w), uv1 = float2(tri.p[1].w, tri.n[1].w), uv2 = float2(tri.p[2].w, tri.n[2].w);
    float2 uv = b0 * uv0 + b1 * uv1 + b2 * uv2, du1 = uv1 - uv0, du2 = uv2 - uv0;
    float3 grad = normalize(b0 * tri.n[0].xyz + b1 * tri.n[1].xyz + b2 * tri.n[2].xyz + float3(TINY));
    float uv_det = du1.x * du2.y - du2.x * du1.y;
    float3 tangent = abs(uv_det) > TINY ? (e1 * du2.y - e2 * du1.y) / uv_det : e1, bitangent = abs(uv_det) > TINY ? (e2 * du1.x - e1 * du2.x) / uv_det : e2;
    float4 n_local = float4(grad, dot(grad, p.xyz / p.w));
    n_local *= rsqrt(max(1.0 - n_local.w * n_local.w, TINY));
    float4 n = to_world(st, n_local.xyz, n_local.w);
    if (mdot(n, v) > 0.0) n = -n;
    float4 t = orthonormal(to_world(st, tangent, 0.0), n), b = orthonormal(to_world(st, bitangent, 0.0), n);
    float area_m2 = length(cross(e1, e2)), uv_area = max(abs(uv_det), TINY);
    float pixel_m = scene.u.pixel_angle_rad * scene.u.curvature_radius_m * sinh(min(dist_s, FAR_S)) * rsqrt(max(abs(mdot(n, v)), MIN_FACING));
    float lod = log2(max(pixel_m * MESH_TEXTURE_SIDE_PX * sqrt(uv_area / max(area_m2, TINY)), 1.0));
    constexpr sampler wrap(address::repeat, filter::linear, mip_filter::linear);
    int4 layers = mat.layers;
    float4 base = mat.base_color * (layers.x >= 0 ? scene.mesh_albedo.sample(wrap, uv, layers.x, level(lod)) : float4(1.0));
    if (st.scale.z >= 0.0) base.rgb = recolor_body(base.rgb, get_palette(scene, int(st.scale.z)).rgb);
    float3 mr = layers.y >= 0 ? scene.mesh_mr.sample(wrap, uv, layers.y, level(lod)).rgb : float3(1.0);
    float3 emissive = mat.emissive_metallic.rgb * (layers.w >= 0 ? scene.mesh_emissive.sample(wrap, uv, layers.w, level(lod)).rgb : float3(1.0));
    Surface s = {base.rgb, max(mat.params.x * mr.g, MIN_ROUGHNESS), mat.emissive_metallic.w * mr.b, 1.0, n};
    if (layers.z >= 0) {
        float2 bend = scene.mesh_normal.sample(wrap, uv, layers.z, level(lod)).rg * 2.0 - 1.0;
        s.n = normalize_tangent(x, t * bend.x + b * bend.y + n * sqrt(saturate(1.0 - dot(bend, bend))));
    }
    if (st.scale.y > HALF_KIND) {
        float4 entry = get_palette(scene, scene.shapes[st.size.x].info.x);
        emissive += entry.rgb * entry.a * BEACON_GLOW;
    }
    return light_surface(scene, s, x, n, v) + emissive * GLOW_BRIGHTNESS;
}

/** @brief `sample_star_layer` draws one layer of stars from a grid on the cube around the sky. @par Example @code float s = sample_star_layer(scene, d, 60.0, 0.03); @endcode */
static float sample_star_layer(thread const Scene &scene, float3 d, float grid, float density) {
    float3 ad = abs(d);
    float2 uv;
    float face;
    if (ad.x >= ad.y && ad.x >= ad.z) { uv = d.yz / ad.x; face = d.x > 0.0 ? 0.0 : 1.0; }
    else if (ad.y >= ad.z) { uv = d.xz / ad.y; face = d.y > 0.0 ? 2.0 : 3.0; }
    else { uv = d.xy / ad.z; face = d.z > 0.0 ? 4.0 : 5.0; }
    float2 cell = floor(uv * grid);
    float h = hash_3d(float3(cell, face + grid));
    if (h > density) return 0.0;
    float2 star = (cell + 0.5 * (1.0 - STAR_JITTER) + STAR_JITTER * float2(hash_3d(float3(cell, face + STAR_SEEDS.x)), hash_3d(float3(cell, face + STAR_SEEDS.y)))) / grid;
    float angle = length(uv - star) / (1.0 + dot(uv, uv)), radius = scene.u.pixel_angle_rad * STAR_PIXELS;
    return (STAR_FAINT + STAR_BRIGHT * pow(h / density, STAR_BRIGHTNESS_POWER)) * exp(-angle * angle / (radius * radius));
}

/** @brief `sample_fbm_normalized` sums `octaves` octaves of value noise, scaled back to [0, 1]. @par Example @code float n = sample_fbm_normalized(p, 3); @endcode */
static float sample_fbm_normalized(float3 p, int octaves) {
    float sum = 0.0, amplitude = 0.5, total = 0.0;
    for (int i = 0; i < octaves; i++) { sum += amplitude * sample_noise_3d(p); total += amplitude; p *= FBM_LACUNARITY; amplitude *= 0.5; }
    return sum / total;
}

/** @brief `stretch_noise` spreads noise in [0, 1] away from its middle by `contrast`, clamped to [0, 1]. @par Example @code float n = stretch_noise(sample_noise_3d(p), 2.5); @endcode */
static float stretch_noise(float n, float contrast) { return saturate((n - 0.5) * contrast + 0.5); }

/** @brief `get_gain` gives the factor `gain.x + gain.y * x`, applied by `amount` (0 leaves 1). @par Example @code float g = get_gain(MILKY_WAY_CLOUD_GAIN, clouds, MILKY_WAY_CLOUD_AMOUNT); @endcode */
static float get_gain(float2 gain, float x, float amount) { return mix(1.0, gain.x + gain.y * x, amount); }

/**
 * @brief `get_milky_way_glow` gives the Milky Way in direction g of its own frame (x toward the core, z toward the
 * pole): a band brighter and wider toward the core with a warm bulge there, mottled by star clouds and grain, dimmed
 * and reddened by a dark rift along its middle and by dust clouds, with pink nebulae (rgb, an intensity
 * `sample_sky_bodies` turns into colour), and how much it thickens the stars (a, 0 to 1).
 * @par Example
 * @code
 * float4 glow = get_milky_way_glow(float3(1.0, 0.0, 0.0));
 * @endcode
 */
static float4 get_milky_way_glow(float3 g) {
    float lon = atan2(g.y, g.x);
    float3 warp = float3(sample_noise_3d(g * MILKY_WAY_WARP_SCALE + MILKY_WAY_WARP_SEEDS.x), sample_noise_3d(g * MILKY_WAY_WARP_SCALE + MILKY_WAY_WARP_SEEDS.y),
                         sample_noise_3d(g * MILKY_WAY_WARP_SCALE + MILKY_WAY_WARP_SEEDS.z)) - 0.5;
    float lat = g.z + MILKY_WAY_EDGE_WARP * warp.x;
    float toward_core = 0.5 + 0.5 * cos(lon);
    float bulge_shape = exp(-lon * lon / (MILKY_WAY_BULGE_SIZE.x * MILKY_WAY_BULGE_SIZE.x));
    float width = MILKY_WAY_DISK_WIDTH * (1.0 + MILKY_WAY_CORE_WIDENING * bulge_shape);
    float band = exp(-lat * lat / (width * width));
    float bulge = bulge_shape * exp(-lat * lat / (MILKY_WAY_BULGE_SIZE.y * MILKY_WAY_BULGE_SIZE.y));
    float3 stretched = g * MILKY_WAY_STRETCH + warp * MILKY_WAY_STRETCH_WARP;
    float clouds = stretch_noise(sample_fbm_normalized(stretched * MILKY_WAY_CLOUD_SCALE, MILKY_WAY_COARSE_OCTAVES), MILKY_WAY_NOISE_CONTRAST);
    float grain = stretch_noise(sample_fbm_normalized(stretched * MILKY_WAY_GRAIN_SCALE + MILKY_WAY_GRAIN_SEED, MILKY_WAY_GRAIN_OCTAVES), MILKY_WAY_NOISE_CONTRAST);
    float lane_lat = (lat - MILKY_WAY_LANE_WANDER * (sample_noise_3d(g * MILKY_WAY_LANE_WANDER_SCALE + MILKY_WAY_LANE_WANDER_SEED) - 0.5)) / (MILKY_WAY_LANE_WIDTH_FRAC * width);
    float rift_noise = stretch_noise(sample_fbm_normalized(stretched * MILKY_WAY_RIFT_SCALE + MILKY_WAY_RIFT_SEED, MILKY_WAY_COARSE_OCTAVES), MILKY_WAY_DUST_CONTRAST);
    float rift = exp(-lane_lat * lane_lat) * smoothstep(MILKY_WAY_RIFT_EDGES.x, MILKY_WAY_RIFT_EDGES.y, rift_noise) * (1.0 - MILKY_WAY_RIFT_CORE_GAP * bulge_shape);
    float3 dust_at = stretched * MILKY_WAY_DARK_SCALE + warp * MILKY_WAY_DARK_WARP + MILKY_WAY_DARK_SEED;
    float dark = band * smoothstep(MILKY_WAY_DARK_EDGES.x, MILKY_WAY_DARK_EDGES.y, stretch_noise(sample_fbm_normalized(dust_at, MILKY_WAY_DARK_OCTAVES), MILKY_WAY_DUST_CONTRAST))
        * (MILKY_WAY_DARK_GRAIN_GAIN.x + MILKY_WAY_DARK_GRAIN_GAIN.y * grain);
    float3 transmit = exp(-(MILKY_WAY_RIFT_DEPTH * rift + MILKY_WAY_DARK_DEPTH * dark) * MILKY_WAY_REDDENING);
    float disk_gain = get_gain(MILKY_WAY_CLOUD_GAIN, clouds, MILKY_WAY_CLOUD_AMOUNT) * get_gain(MILKY_WAY_GRAIN_GAIN, grain, MILKY_WAY_GRAIN_AMOUNT);
    float bulge_gain = get_gain(MILKY_WAY_BULGE_CLOUD_GAIN, clouds, MILKY_WAY_CLOUD_AMOUNT) * get_gain(MILKY_WAY_BULGE_GRAIN_GAIN, grain, MILKY_WAY_GRAIN_AMOUNT);
    float3 intensity = MILKY_WAY_DISK_COLOR * MILKY_WAY_DISK * band * mix(MILKY_WAY_ANTICENTRE_FRAC, 1.0, toward_core * toward_core) * disk_gain
                       + MILKY_WAY_BULGE_COLOR * MILKY_WAY_BULGE * bulge * bulge_gain;
    float nebula_noise = stretch_noise(sample_fbm_normalized(g * MILKY_WAY_NEBULA_SCALE + MILKY_WAY_NEBULA_SEED, MILKY_WAY_COARSE_OCTAVES), MILKY_WAY_NOISE_CONTRAST);
    intensity += MILKY_WAY_NEBULA_COLOR * MILKY_WAY_NEBULA * MILKY_WAY_DISK * band * smoothstep(MILKY_WAY_NEBULA_EDGES.x, MILKY_WAY_NEBULA_EDGES.y, nebula_noise);
    return float4(intensity * transmit, saturate(band + bulge) * dot(transmit, LUMA));
}

/**
 * @brief `bake_milky_way` draws the Milky Way (`get_milky_way_glow`) into texture 0 once, in its own frame: longitude
 * across (from −π at the left edge), latitude up to ±`MILKY_WAY_MAX_LATITUDE_RAD` (bottom to top), beyond which it is
 * too faint to matter.
 * @par Example
 * @code
 * dispatched by render_voxel.rs before the first frame, one thread per texel.
 * @endcode
 */
kernel void bake_milky_way(texture2d<float, access::write> glow [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (id.x >= glow.get_width() || id.y >= glow.get_height()) return;
    float2 uv = (float2(id) + 0.5) / float2(glow.get_width(), glow.get_height());
    float lon = (2.0 * uv.x - 1.0) * M_PI_F, lat = (2.0 * uv.y - 1.0) * MILKY_WAY_MAX_LATITUDE_RAD;
    glow.write(get_milky_way_glow(float3(cos(lat) * cos(lon), cos(lat) * sin(lon), sin(lat))), id);
}

/**
 * @brief `sample_milky_way` reads the baked Milky Way in sky direction d (in the world origin's frame), turned by the
 * map's pole and core and scaled by the player's brightness: 0 for a map without one or outside the baked latitudes.
 * @par Example
 * @code
 * float4 galaxy = sample_milky_way(scene, d);
 * @endcode
 */
static float4 sample_milky_way(thread const Scene &scene, float3 d) {
    float4 pole = scene.u.milky_way_pole;
    float3 core = scene.u.milky_way_core.xyz;
    if (pole.w <= 0.0) return float4(0.0);
    float3 g = float3(dot(d, core), dot(d, cross(pole.xyz, core)), dot(d, pole.xyz));
    float lat = asin(clamp(g.z, -1.0, 1.0));
    if (abs(lat) >= MILKY_WAY_MAX_LATITUDE_RAD) return float4(0.0);
    constexpr sampler around(filter::linear, s_address::repeat, t_address::clamp_to_edge);
    float2 uv = float2(0.5 + 0.5 * atan2(g.y, g.x) / M_PI_F, 0.5 + 0.5 * lat / MILKY_WAY_MAX_LATITUDE_RAD);
    return scene.milky_way.sample(around, uv) * pole.w;
}

/** @brief `get_sky_direction` gives an escaping ray's direction, carried to the eye, then to the origin. @par Example @code float3 d = get_sky_direction(scene, x, v); @endcode */
static float3 get_sky_direction(thread const Scene &scene, float4 x, float4 v) {
    float4 cam = scene.u.camera[3];
    float4 w = v + mdot(cam, v) / (1.0 - mdot(x, cam)) * (x + cam);
    return normalize(w.xyz - w.w / (1.0 + cam.w) * cam.xyz);
}

/** @brief `sample_sky_bodies` gives the sky in direction d at the origin: stars, Milky Way, sun, planet. @par Example @code float3 c = sample_sky_bodies(scene, d); @endcode */
static float3 sample_sky_bodies(thread const Scene &scene, float3 d) {
    constant VoxelUniforms &u = scene.u;
    float3 sun_dir = u.sun.xyz, planet = u.planet.xyz, e1 = u.planet_e1.xyz, e2 = u.planet_e2.xyz;
    float sd = dot(d, sun_dir);
    float3 sun = SUN_COLOR * (smoothstep(SUN_COS_OUTER, SUN_COS_INNER, sd) * SUN_DISC_BRIGHTNESS + pow(max(sd, 0.0), SUN_GLARE_POWER) * SUN_GLARE
                              + pow(max(sd, 0.0), SUN_HALO_POWER) * SUN_HALO);
    float2 q = float2(dot(d, e1), dot(d, e2)) / u.planet.w;
    float r2 = dot(q, q);
    if (dot(d, planet) > 0.0 && r2 < 1.0) {
        float3 nrm = q.x * e1 + q.y * e2 - sqrt(1.0 - r2) * planet;
        float lit = dot(nrm, sun_dir);
        float land = smoothstep(LAND_EDGES.x, LAND_EDGES.y, sample_fbm_3d(nrm * LAND_SCALE + LAND_OFFSET));
        float cloud = smoothstep(CLOUD_EDGES.x, CLOUD_EDGES.y, sample_fbm_3d(nrm * CLOUD_SCALE + CLOUD_OFFSET));
        float3 surface = mix(mix(OCEAN_COLOR, LAND_COLOR, land), CLOUD_COLOR, cloud);
        float3 color = surface * (max(lit, 0.0) * PLANET_SUNLIT + PLANET_NIGHT) + ATMOSPHERE_COLOR * pow(r2, LIMB_POWER) * max(lit + LIMB_WRAP, 0.0) * LIMB_GLOW;
        return color + sun;
    }
    float4 galaxy = sample_milky_way(scene, d);
    float star_boost = 1.0 + MILKY_WAY_STAR_BOOST * galaxy.a;
    float3 color = float3(sample_star_layer(scene, d, STAR_GRID_COARSE, STAR_DENSITY_COARSE * star_boost)
                          + FINE_STAR_BRIGHTNESS * sample_star_layer(scene, d, STAR_GRID_FINE, STAR_DENSITY_FINE * star_boost));
    if (galaxy.a > 0.0) {
        color += MILKY_WAY_MICRO_STAR_BRIGHTNESS * sample_star_layer(scene, d, MILKY_WAY_MICRO_STAR_GRID, MILKY_WAY_MICRO_STAR_DENSITY * galaxy.a);
        color += MILKY_WAY_TONE.x * pow(galaxy.rgb, MILKY_WAY_TONE.y);
    }
    if (dot(d, planet) > 0.0 && r2 < HALO_RADIUS_SQ) {
        color += ATMOSPHERE_COLOR * HALO_GLOW * (1.0 - smoothstep(1.0, HALO_RADIUS_SQ, r2)) * max(dot(normalize(q.x * e1 + q.y * e2), sun_dir) + HALO_WRAP, 0.0);
    }
    return color + sun;
}

/** @brief `get_box_distance_2d` gives the signed distance from p to the box centred at `box.xy` with half size `box.zw`. @par Example @code float d = get_box_distance_2d(p, HULL_BOXES[0]); @endcode */
static float get_box_distance_2d(float2 p, float4 box) {
    float2 e = abs(p - box.xy) - box.zw;
    return length(max(e, 0.0)) + min(max(e.x, e.y), 0.0);
}

/**
 * @brief `get_capsule_distance_2d` nearly gives the signed distance from p to the segment from `ends.xy` to `ends.zw`,
 * thickened by a radius running from `radii.x` to `radii.y`.
 * @par Example
 * @code
 * float d = get_capsule_distance_2d(p, HULL_CAPSULES[0], HULL_CAPSULE_RADII[0]);
 * @endcode
 */
static float get_capsule_distance_2d(float2 p, float4 ends, float2 radii) {
    float2 pa = p - ends.xy, ba = ends.zw - ends.xy;
    float t = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * t) - mix(radii.x, radii.y, t);
}

/**
 * @brief `get_triangle_distance_2d` gives the signed distance from p to the triangle with corners a, b and c (either
 * winding), negative inside.
 * @par Example
 * @code
 * float d = get_triangle_distance_2d(p, HULL_TRIANGLES[0][0], HULL_TRIANGLES[0][1], HULL_TRIANGLES[0][2]);
 * @endcode
 */
static float get_triangle_distance_2d(float2 p, float2 a, float2 b, float2 c) {
    float2 ab = b - a, bc = c - b, ca = a - c, pa = p - a, pb = p - b, pc = p - c;
    float2 qa = pa - ab * clamp(dot(pa, ab) / dot(ab, ab), 0.0, 1.0);
    float2 qb = pb - bc * clamp(dot(pb, bc) / dot(bc, bc), 0.0, 1.0);
    float2 qc = pc - ca * clamp(dot(pc, ca) / dot(ca, ca), 0.0, 1.0);
    float winding = sign(ab.x * ca.y - ab.y * ca.x);
    float2 nearest = min(min(float2(dot(qa, qa), winding * (pa.x * ab.y - pa.y * ab.x)), float2(dot(qb, qb), winding * (pb.x * bc.y - pb.y * bc.x))),
                         float2(dot(qc, qc), winding * (pc.x * ca.y - pc.y * ca.x)));
    return -sqrt(nearest.x) * sign(nearest.y);
}

/** @brief `turn_into_sector` turns p by `-turn_rad`, then into the sector of `count` around the origin that holds the +x axis. @par Example @code float2 q = turn_into_sector(p, 0.3, 6.0); @endcode */
static float2 turn_into_sector(float2 p, float turn_rad, float count) {
    float sector_rad = 2.0 * M_PI_F / count, angle_rad = atan2(p.y, p.x) - turn_rad;
    angle_rad -= sector_rad * round(angle_rad / sector_rad);
    return length(p) * float2(cos(angle_rad), sin(angle_rad));
}

/**
 * @brief `get_hull_distance` gives the signed distance from p to a spaceship's hull, in its half lengths, with the colour
 * of the nearest part. p is in the ship's own frame (x to the nose, y to starboard), and a ring ship's ring and spokes
 * are turned by `ring_rad`.
 * @par Example
 * @code
 * float d = get_hull_distance(HULL_FIGHTER, p, 0.0, albedo);
 * @endcode
 */
static float get_hull_distance(int hull, float2 p, float ring_rad, thread float3 &albedo) {
    float2 m = float2(p.x, abs(p.y));
    int4 range = HULL_PART_RANGES[hull];
    float distance = INFINITY;
    for (int i = range.x; i < range.x + range.y; i++) {
        float d = get_box_distance_2d(m, HULL_BOXES[i]) - SHIP_ROUNDING_HALF_LENGTHS;
        if (d < distance) { distance = d; albedo = HULL_BOX_COLORS[i]; }
    }
    for (int i = range.z; i < range.z + range.w; i++) {
        float d = get_capsule_distance_2d(m, HULL_CAPSULES[i], HULL_CAPSULE_RADII[i]);
        if (d < distance) { distance = d; albedo = HULL_CAPSULE_COLORS[i]; }
    }
    int4 triangles = HULL_TRIANGLE_RANGES[hull];
    for (int i = triangles.x; i < triangles.x + triangles.y; i++) {
        float d = get_triangle_distance_2d(m, HULL_TRIANGLES[i][0], HULL_TRIANGLES[i][1], HULL_TRIANGLES[i][2]);
        if (d < distance) { distance = d; albedo = HULL_TRIANGLE_COLORS[i]; }
    }
    if (hull == HULL_RING_SHIP) {
        float ring = abs(length(p) - RING_RADIUS_HALF_LENGTHS) - RING_HALF_WIDTH_HALF_LENGTHS;
        float spoke = get_capsule_distance_2d(turn_into_sector(p, ring_rad, RING_SPOKE_COUNT), float4(0.0, 0.0, RING_RADIUS_HALF_LENGTHS, 0.0),
                                              float2(RING_SPOKE_RADIUS_HALF_LENGTHS));
        float d = min(ring, spoke);
        if (d < distance) { distance = d; albedo = RING_COLOR; }
    }
    return distance;
}

/**
 * @brief `paint_hull_insets` paints a hull's insets (`HULL_INSETS`) over its colour `albedo` at p (its own frame, in
 * half lengths, with pixels `px` half lengths wide), with smooth edges.
 * @par Example
 * @code
 * albedo = paint_hull_insets(HULL_FIGHTER, p, px, albedo);
 * @endcode
 */
static float3 paint_hull_insets(int hull, float2 p, float px, float3 albedo) {
    float2 m = float2(p.x, abs(p.y));
    int4 triangles = HULL_TRIANGLE_RANGES[hull];
    for (int i = triangles.z; i < triangles.z + triangles.w; i++) {
        float d = get_triangle_distance_2d(m, HULL_INSETS[i][0], HULL_INSETS[i][1], HULL_INSETS[i][2]);
        albedo = mix(albedo, HULL_INSET_COLORS[i], clamp(0.5 - d / px, 0.0, 1.0));
    }
    return albedo;
}

/**
 * @brief `get_spaceship_glow` gives the light a spaceship's engines and lights add at p (its own frame, in half lengths,
 * with pixels `px` half lengths wide): glowing nozzles with a plume behind, red to port, green to starboard, a flashing
 * strobe, lit windows and a ring ship's ring lights.
 * @par Example
 * @code
 * color += get_spaceship_glow(ship, HULL_FIGHTER, p, px);
 * @endcode
 */
static float3 get_spaceship_glow(Spaceship ship, int hull, float2 p, float px) {
    float2 m = float2(p.x, abs(p.y));
    int4 range = HULL_GLOW_RANGES[hull];
    float3 glow = float3(0.0);
    for (int i = range.x; i < range.x + range.y; i++) {
        float2 e = m - HULL_ENGINES[i].xy;
        float r = max(HULL_ENGINES[i].z, SHIP_LIGHT_MIN_PIXELS * px), across = e.y / (r * SHIP_PLUME_WIDTH_PER_RADIUS), along = e.x / SHIP_PLUME_HALF_LENGTHS;
        float plume = e.x < 0.0 ? exp(-across * across - along * along) : 0.0;
        glow += HULL_ENGINE_COLORS[hull] * ship.lights.y * (SHIP_ENGINE_BRIGHTNESS * exp(-dot(e, e) / (r * r)) + SHIP_PLUME_BRIGHTNESS * plume);
    }
    for (int i = range.z; i < range.z + range.w; i++) {
        float4 light = HULL_LIGHTS[i];
        float2 e = m - light.xy;
        float r = max(light.w, SHIP_LIGHT_MIN_PIXELS * px);
        int kind = int(light.z);
        float3 tint = kind == LIGHT_NAV ? (p.y < 0.0 ? PORT_COLOR : STARBOARD_COLOR)
                      : kind == LIGHT_STROBE ? STROBE_COLOR * ship.lights.x : WINDOW_COLOR;
        glow += tint * exp(-dot(e, e) / (r * r));
    }
    if (hull == HULL_RING_SHIP) {
        float2 e = turn_into_sector(p, ship.lights.z, RING_LIGHT_COUNT) - float2(RING_RADIUS_HALF_LENGTHS, 0.0);
        float r = max(RING_LIGHT_HALF_LENGTHS, SHIP_LIGHT_MIN_PIXELS * px);
        glow += RING_LIGHT_COLOR * exp(-dot(e, e) / (r * r));
    }
    return glow;
}

/**
 * @brief `get_light_glow` gives the glow of a light of colour `tint` and radius `radius` at e from its centre, at least
 * `SHIP_LIGHT_MIN_PIXELS` pixels (`px` wide) across.
 * @par Example
 * @code
 * glow += get_light_glow(p - centre, CAPITAL_LIGHT_RADIUS_HALF_LENGTHS, px, STROBE_COLOR);
 * @endcode
 */
static float3 get_light_glow(float2 e, float radius, float px, float3 tint) {
    float r = max(radius, SHIP_LIGHT_MIN_PIXELS * px);
    return tint * exp(-dot(e, e) / (r * r));
}

/**
 * @brief `light_giant` gives the light on a giant ship's surface with normal n: the sun, a little light from the sunlit
 * ground below (`groundshine_frac` times what lights the small ships), and a faint glow so its night side shows against
 * the stars.
 * @par Example
 * @code
 * float3 light = light_giant(scene, n, STATION_GROUNDSHINE_FRAC);
 * @endcode
 */
static float3 light_giant(thread const Scene &scene, float3 n, float groundshine_frac) {
    return SUN_COLOR * SHIP_SUNLIT * max(dot(n, scene.u.sun.xyz), 0.0) + groundshine_frac * SHIP_GROUNDSHINE_COLOR * max(-n.y, 0.0)
           + float3(GIANT_NIGHT);
}

/**
 * @brief `draw_battle_station` draws the battle station over the sky colour `color`: a sphere `ship.centre.w` radians
 * across in the sky (p is the direction in its radii, x along the heading), its pole tilted toward the eye, with panel
 * bands, a dark trench round its equator and a concave dish with a dark lens in its upper half; small lights in the
 * trench, lit windows on its night side and a red beacon flashing on its pole.
 * @par Example
 * @code
 * color = draw_battle_station(scene, ship, p, px, color);
 * @endcode
 */
static float3 draw_battle_station(thread const Scene &scene, Spaceship ship, float2 p, float px, float3 color) {
    float r2 = dot(p, p), coverage = clamp(0.5 - (sqrt(r2) - 1.0) / px, 0.0, 1.0);
    if (coverage <= 0.0) return color;
    float3 c = ship.centre.xyz, h = ship.heading.xyz, side = cross(h, c), toward_eye = -c;
    float3 pole = side * cos(STATION_TILT_RAD) + toward_eye * sin(STATION_TILT_RAD);
    float3 front = toward_eye * cos(STATION_TILT_RAD) - side * sin(STATION_TILT_RAD);
    float2 dish_rad = STATION_DISH_LAT_LON_RAD;
    float3 dish = cos(dish_rad.x) * (cos(dish_rad.y) * front + sin(dish_rad.y) * h) + sin(dish_rad.x) * pole;
    float3 x = p.x * h + p.y * side + sqrt(max(1.0 - r2, 0.0)) * toward_eye, n = x;
    float rim = sin(STATION_DISH_RAD), bowl_offset = cos(STATION_DISH_RAD) + sqrt(STATION_BOWL_RADIUS_RADII * STATION_BOWL_RADIUS_RADII - rim * rim);
    bool is_dish = dot(x, dish) > cos(STATION_DISH_RAD);
    float3 albedo, glow = float3(0.0);
    if (is_dish) {
        float3 q = x - bowl_offset * dish;
        float b = dot(q, c);
        x += (-b + sqrt(max(b * b - dot(q, q) + STATION_BOWL_RADIUS_RADII * STATION_BOWL_RADIUS_RADII, 0.0))) * c;
        if (dot(x, x) > 1.0) return color;
        n = normalize(bowl_offset * dish - x);
        float from_axis = length(x - dot(x, dish) * dish);
        float ring = min(abs(from_axis - STATION_DISH_RING_FRACS.x * rim), abs(from_axis - STATION_DISH_RING_FRACS.y * rim));
        albedo = mix(STATION_DISH_COLOR, STATION_TRENCH_COLOR, clamp(0.5 - (ring - STATION_DISH_RING_HALF_WIDTH_RADII) / px, 0.0, 1.0));
        albedo = mix(albedo, STATION_LENS_COLOR, clamp(0.5 - (from_axis - STATION_LENS_RADIUS_RADII) / px, 0.0, 1.0));
    } else {
        float lat = asin(clamp(dot(x, pole), -1.0, 1.0)), lon = atan2(dot(x, h), dot(x, front));
        float band = floor(lat / STATION_BAND_RAD), band_lat = (band + 0.5) * STATION_BAND_RAD;
        float panel_rad = STATION_PANEL_RAD / max(cos(band_lat), STATION_MIN_PANEL_COS), column = floor(lon / panel_rad);
        albedo = STATION_COLOR * (1.0 + STATION_PANEL_VARIATION * (hash_3d(float3(band, column, float(HULL_BATTLE_STATION))) - 0.5));
        float seam = abs(lat - STATION_BAND_RAD * round(lat / STATION_BAND_RAD));
        albedo *= 1.0 - STATION_SEAM_DARKEN_FRAC * clamp(0.5 - (seam - STATION_SEAM_HALF_RAD) / px, 0.0, 1.0);
        float trench = clamp(0.5 - (abs(lat) - STATION_TRENCH_HALF_RAD) / px, 0.0, 1.0);
        albedo = mix(albedo, STATION_TRENCH_COLOR, trench);
        float window_lon = (column + 0.5) * panel_rad;
        float3 window = cos(band_lat) * (cos(window_lon) * front + sin(window_lon) * h) + sin(band_lat) * pole;
        float night = 1.0 - smoothstep(STATION_NIGHT_EDGES.x, STATION_NIGHT_EDGES.y, dot(n, scene.u.sun.xyz));
        if (hash_3d(float3(band, column, float(HULL_BATTLE_STATION) + STATION_WINDOW_SEED)) < STATION_WINDOW_FRAC && trench < 1.0) {
            glow += night * STATION_WINDOW_BRIGHTNESS * get_light_glow(float2(dot(x - window, h), dot(x - window, side)), STATION_WINDOW_RADIUS_RADII, px, WINDOW_COLOR);
        }
    }
    color = mix(color, albedo * light_giant(scene, n, STATION_GROUNDSHINE_FRAC), coverage);
    for (float i = 0.0; i < STATION_TRENCH_LIGHT_COUNT; i += 1.0) {
        float lon = 2.0 * M_PI_F * i / STATION_TRENCH_LIGHT_COUNT;
        float3 light = cos(lon) * front + sin(lon) * h;
        if (dot(light, toward_eye) > 0.0) glow += get_light_glow(p - float2(dot(light, h), dot(light, side)), STATION_TRENCH_LIGHT_RADIUS_RADII, px, RING_LIGHT_COLOR);
    }
    glow += get_light_glow(p - float2(dot(pole, h), dot(pole, side)), STATION_BEACON_RADIUS_RADII, px, PORT_COLOR * ship.lights.x);
    return color + coverage * glow;
}

/**
 * @brief `trace_capital_box` traces the ray from o along unit u through the box with corners `lo` and `hi`: the
 * distance it enters at into `entry` and the normal there into n, and returns its gap to the box (negative when it goes
 * through), measured midway between where it enters the last slab and leaves the first.
 * @par Example
 * @code
 * float gap = trace_capital_box(o, u, CAPITAL_BOXES[i][0], CAPITAL_BOXES[i][1], entry, n);
 * @endcode
 */
static float trace_capital_box(float3 o, float3 u, float3 lo, float3 hi, thread float &entry, thread float3 &n) {
    float exit = NO_HIT_S;
    entry = -NO_HIT_S;
    for (int k = 0; k < 3; k++) {
        if (abs(u[k]) < TINY) continue;
        float t_lo = (lo[k] - o[k]) / u[k], t_hi = (hi[k] - o[k]) / u[k];
        float3 axis = float3(0.0);
        axis[k] = 1.0;
        if (min(t_lo, t_hi) > entry) { entry = min(t_lo, t_hi); n = t_lo < t_hi ? -axis : axis; }
        exit = min(exit, max(t_lo, t_hi));
    }
    float3 e = abs(o + 0.5 * (entry + exit) * u - 0.5 * (lo + hi)) - 0.5 * (hi - lo);
    return length(max(e, 0.0)) + min(max(e.x, max(e.y, e.z)), 0.0);
}

/**
 * @brief `trace_capital_hull` traces the ray from o along unit u through the capital ship's wedge
 * (`CAPITAL_HULL_PLANES`), as `trace_capital_box` does a box.
 * @par Example
 * @code
 * float gap = trace_capital_hull(o, u, entry, n);
 * @endcode
 */
static float trace_capital_hull(float3 o, float3 u, thread float &entry, thread float3 &n) {
    float exit = NO_HIT_S, gap = -NO_HIT_S;
    entry = -NO_HIT_S;
    for (int i = 0; i < 5; i++) {
        float4 plane = CAPITAL_HULL_PLANES[i];
        float a = dot(plane.xyz, o) - plane.w, b = dot(plane.xyz, u);
        if (b < 0.0 && -a / b > entry) { entry = -a / b; n = normalize(plane.xyz); }
        if (b > 0.0) exit = min(exit, -a / b);
    }
    float3 middle = o + 0.5 * (entry + exit) * u;
    for (int i = 0; i < 5; i++) gap = max(gap, (dot(CAPITAL_HULL_PLANES[i].xyz, middle) - CAPITAL_HULL_PLANES[i].w) / length(CAPITAL_HULL_PLANES[i].xyz));
    return gap;
}

/**
 * @brief `get_window_light` gives how much of a pixel `px` half lengths wide, at q (along and up a superstructure's side,
 * or in from a hull edge, in half lengths), the capital ship's rows of lit windows cover: sharp windows close up, and their mean once a window
 * is under a pixel, so they do not flicker as the ship drifts.
 * @par Example
 * @code
 * color += WINDOW_COLOR * get_window_light(b.xz, px);
 * @endcode
 */
static float get_window_light(float2 q, float px) {
    float2 cells_per_half_length = float2(CAPITAL_WINDOWS_PER_HALF_LENGTH, 1.0 / CAPITAL_WINDOW_ROW_HALF_LENGTHS);
    float2 cell = q * cells_per_half_length, cell_px = px * cells_per_half_length;
    float2 inside = clamp((0.5 * CAPITAL_WINDOW_SIZE_FRACS - abs(fract(cell) - 0.5)) / cell_px + 0.5, 0.0, 1.0);
    float2 window = mix(inside, CAPITAL_WINDOW_SIZE_FRACS, clamp(cell_px, 0.0, 1.0));
    float lit = hash_3d(float3(floor(cell), float(HULL_CAPITAL_SHIP) + CAPITAL_WINDOW_SEED)) < CAPITAL_WINDOW_LIT_FRAC ? 1.0 : 0.0;
    return window.x * window.y * mix(lit, CAPITAL_WINDOW_LIT_FRAC, clamp(max(cell_px.x, cell_px.y), 0.0, 1.0));
}

/**
 * @brief `draw_capital_ship` draws the capital ship over the sky colour `color`: a long wedge `2 ship.centre.w` radians
 * long (p is the direction in its half lengths, x to the nose), seen from above at a slant so its superstructure stands
 * up toward the top of the sky, lit by the sun; with glowing engines and a plume behind, red and green lights at the
 * stern's corners, a flashing strobe on the bridge, and rows of lit windows along the superstructure's sides and the
 * hull's edges. Its superstructure is one deck block and a bridge tower.
 * @par Example
 * @code
 * color = draw_capital_ship(scene, ship, p, px, color);
 * @endcode
 */
static float3 draw_capital_ship(thread const Scene &scene, Spaceship ship, float2 p, float px, float3 color) {
    float3 c = ship.centre.xyz, h = ship.heading.xyz, side = cross(h, c);
    float3 up = -c * cos(CAPITAL_TILT_RAD) + side * sin(CAPITAL_TILT_RAD), port = side * cos(CAPITAL_TILT_RAD) + c * sin(CAPITAL_TILT_RAD);
    float2 screen_per_z = float2(sin(CAPITAL_TILT_RAD), cos(CAPITAL_TILT_RAD));
    float3 o = float3(p.x, p.y * screen_per_z.y - CAPITAL_EYE_DISTANCE_HALF_LENGTHS * screen_per_z.x, p.y * screen_per_z.x + CAPITAL_EYE_DISTANCE_HALF_LENGTHS * screen_per_z.y);
    float3 u = float3(0.0, screen_per_z.x, -screen_per_z.y);
    float entry, best_entry, coverage;
    float3 n, best_n, albedo = CAPITAL_HULL_COLOR;
    coverage = clamp(0.5 - trace_capital_hull(o, u, best_entry, best_n) / px, 0.0, 1.0);
    bool is_hull = true;
    for (int i = 0; i < CAPITAL_BOX_COUNT; i++) {
        float box_coverage = clamp(0.5 - trace_capital_box(o, u, CAPITAL_BOXES[i][0], CAPITAL_BOXES[i][1], entry, n) / px, 0.0, 1.0);
        if (box_coverage > coverage || (box_coverage > 0.0 && box_coverage >= coverage && entry < best_entry)) {
            coverage = box_coverage; best_entry = entry; best_n = n; albedo = CAPITAL_BOX_COLORS[i]; is_hull = false;
        }
    }
    float3 glow = float3(0.0);
    if (coverage > 0.0) {
        float3 b = o + best_entry * u;
        float panel = 1.0 + CAPITAL_PANEL_VARIATION * (hash_3d(float3(floor(b.xy * CAPITAL_PANELS_PER_HALF_LENGTH), float(HULL_CAPITAL_SHIP) + sign(b.z))) - 0.5);
        float3 normal = normalize(best_n.x * h + best_n.y * port + best_n.z * up);
        color = mix(color, albedo * panel * light_giant(scene, normal, CAPITAL_GROUNDSHINE_FRAC), coverage);
        float in_from_edge = CAPITAL_HALF_WIDTH_PER_HALF_LENGTH * (1.0 - b.x) - abs(b.y);
        bool is_edge_window = is_hull && best_n.z > 0.0 && b.x < CAPITAL_EDGE_WINDOW_END_HALF_LENGTHS && in_from_edge > CAPITAL_EDGE_WINDOW_BAND_HALF_LENGTHS.x
                              && in_from_edge < CAPITAL_EDGE_WINDOW_BAND_HALF_LENGTHS.y;
        float2 q = is_hull ? float2(b.x, in_from_edge) : b.xz;
        if (is_edge_window || (!is_hull && best_n.y < 0.0)) color += coverage * CAPITAL_WINDOW_BRIGHTNESS * WINDOW_COLOR * get_window_light(q, px);
    }
    for (int i = 0; i < CAPITAL_ENGINE_COUNT; i++) {
        float4 engine = CAPITAL_ENGINES[i];
        for (float mirror = -1.0; mirror <= 1.0; mirror += 2.0) {
            if (mirror < 0.0 && engine.y == 0.0) continue;
            float2 e = p - float2(engine.x, mirror * engine.y * screen_per_z.y + engine.z * screen_per_z.x);
            float r = max(engine.w, SHIP_LIGHT_MIN_PIXELS * px), across = e.y / (r * SHIP_PLUME_WIDTH_PER_RADIUS), along = e.x / SHIP_PLUME_HALF_LENGTHS;
            float plume = e.x < 0.0 ? exp(-across * across - along * along) : 0.0;
            glow += CAPITAL_ENGINE_COLOR * ship.lights.y * (CAPITAL_ENGINE_BRIGHTNESS * exp(-dot(e, e) / (r * r)) + SHIP_PLUME_BRIGHTNESS * plume);
        }
    }
    for (int i = 0; i < CAPITAL_LIGHT_COUNT; i++) {
        float4 light = CAPITAL_LIGHTS[i];
        int kind = int(light.w);
        float3 tint = kind == LIGHT_NAV ? (light.y > 0.0 ? PORT_COLOR : STARBOARD_COLOR) : kind == LIGHT_STROBE ? STROBE_COLOR * ship.lights.x : WINDOW_COLOR;
        glow += get_light_glow(p - float2(light.x, light.y * screen_per_z.y + light.z * screen_per_z.x), CAPITAL_LIGHT_RADIUS_HALF_LENGTHS, px, tint);
    }
    return color + glow;
}

/**
 * @brief `draw_spaceship` draws a spaceship over the sky colour `color` in direction d: its hull, lit by the sun on the
 * edges that face it and from below by the sunlit ground, with its engines and lights on top; the giants are drawn by
 * `draw_battle_station` and `draw_capital_ship`. Directions far from the ship return `color` after a few dot products.
 * @par Example
 * @code
 * color = draw_spaceship(scene, scene.u.spaceships[0], d, color);
 * @endcode
 */
static float3 draw_spaceship(thread const Scene &scene, Spaceship ship, float3 d, float3 color) {
    float3 c = ship.centre.xyz, h = ship.heading.xyz, side = cross(h, c);
    float facing = dot(d, c), half_length = ship.centre.w, px = scene.u.pixel_angle_rad / half_length;
    float2 p = float2(dot(d, h), dot(d, side)) / (max(facing, TINY) * half_length);
    float bound = SHIP_BOUND_HALF_LENGTHS + SHIP_BOUND_PIXELS * px;
    if (facing <= 0.0 || dot(p, p) > bound * bound) return color;
    int hull = int(ship.heading.w);
    if (hull == HULL_BATTLE_STATION) return draw_battle_station(scene, ship, p, px, color);
    if (hull == HULL_CAPITAL_SHIP) return draw_capital_ship(scene, ship, p, px, color);
    float3 albedo = float3(0.0), unused;
    float distance = get_hull_distance(hull, p, ship.lights.z, albedo), coverage = clamp(0.5 - distance / px, 0.0, 1.0);
    if (coverage > 0.0) {
        float step = SHIP_GRADIENT_PIXELS * px;
        float2 g = float2(get_hull_distance(hull, p + float2(step, 0.0), ship.lights.z, unused), get_hull_distance(hull, p + float2(0.0, step), ship.lights.z, unused)) - distance;
        g /= max(length(g), TINY);
        float tilt_rad = SHIP_BEVEL_TILT_RAD * clamp(1.0 + distance / SHIP_BEVEL_HALF_LENGTHS, 0.0, 1.0);
        float3 n = -c * cos(tilt_rad) + (g.x * h + g.y * side) * sin(tilt_rad);
        albedo = paint_hull_insets(hull, p, px, albedo);
        float panel = 1.0 + HULL_PANEL_VARIATIONS[hull] * (hash_3d(float3(floor(p * SHIP_PANELS_PER_HALF_LENGTH), float(hull) + sign(p.y))) - 0.5);
        float3 light = SUN_COLOR * SHIP_SUNLIT * max(dot(n, scene.u.sun.xyz), 0.0) + SHIP_GROUNDSHINE_COLOR * max(-n.y, 0.0);
        color = mix(color, albedo * panel * light, coverage);
    }
    return color + get_spaceship_glow(ship, hull, p, px);
}

/** @brief `sample_sky` gives the sky in direction d at the origin: stars, Milky Way, sun and planet, with the spaceships in front. @par Example @code float3 c = sample_sky(scene, d); @endcode */
static float3 sample_sky(thread const Scene &scene, float3 d) {
    if (scene.u.snow.x > 0.5) {
        float up = max(d.y, 0.0), sd = max(dot(d, scene.u.sun.xyz), 0.0);
        float3 sky = mix(SNOW_HORIZON, SNOW_ZENITH, pow(up, 0.55));
        float cloud = smoothstep(0.42, 0.78, sample_fbm_3d(float3(d.xz / (up + 0.12) * 1.6, 0.5)));
        sky = mix(sky, SNOW_CLOUD * (0.82 + 0.18 * sd), cloud * smoothstep(0.0, 0.08, up));
        return sky + SUN_COLOR * (pow(sd, 900.0) * 3.0 + pow(sd, 12.0) * 0.22);
    }
    float3 color = sample_sky_bodies(scene, d);
    for (int i = 0; i < SPACESHIP_COUNT; i++) color = draw_spaceship(scene, scene.u.spaceships[i], d, color);
    return color;
}

/** @brief `scan_beams` adds the glow of laser segments near a chord (closest approach in closed form): a core (a flat-topped blade for the sniper's, a Gaussian for the rifle's) and a soft haze (`halo`), with the brightest haze's code (team and gun) and speckle share. A beacon's beam (code at least `BEACON_BEAM_CODE`) is `BEACON_BEAM_RADIUS_M` wide. @par Example @code scan_beams(scene, x, v, s, t, core, halo, code, speckle); @endcode */
static void scan_beams(thread const Scene &scene, float4 x, float4 v, float s_end, float travelled, thread float &core, thread float &halo, thread float &code,
                       thread float &speckle) {
    float r = scene.u.curvature_radius_m, pix = scene.u.pixel_angle_rad;
    for (int i = 0; i < scene.u.beam_count; i++) {
        float4 a = scene.beams[i].a, b = scene.beams[i].b, info = scene.beams[i].info;
        int beam_code = int(info.w + HALF_KIND);
        float half_len = 0.5 * info.x, radius_m = beam_code >= BEACON_BEAM_CODE ? BEACON_BEAM_RADIUS_M : BEAM_RADIUS_M;
        float4 mid = geodesic_point(a, b, half_len);
        float am = -mdot(x, mid), bm = mdot(v, mid), reach = half_len + BEAM_REACH * max(radius_m, pix * r * sinh(min(travelled + s_end, FAR_S))) / r;
        if (am * am - bm * bm > cosh(reach) * cosh(reach)) continue;
        float al = mdot(x, a), be = mdot(v, a), ga = mdot(x, b), et = mdot(v, b);
        float k1 = 0.5 * ((al * al - ga * ga) + (be * be - et * et)), k2 = al * be - ga * et;
        float s_c = k1 > abs(k2) ? 0.5 * clamp_atanh(-k2 / k1) : (k2 < 0.0 ? s_end : 0.0);
        s_c = clamp(s_c, 0.0, min(s_end, FAR_S));
        float4 xc = geodesic_point(x, v, s_c);
        float sigma = clamp(clamp_atanh(mdot(xc, b) / -mdot(xc, a)), 0.0, info.x);
        float d_m = measure_gap(xc, geodesic_point(a, b, sigma)) * r;
        float width_m = max(radius_m, pix * r * sinh(min(travelled + s_c, FAR_S)));
        float q = d_m * d_m / (width_m * width_m);
        bool is_rifle = (beam_code & RIFLE_BEAM_CODE) != 0;
        core = max(core, info.z * exp(is_rifle ? -q : -q * q * q));
        float hl = info.z * exp(-q / BEAM_REACH);
        if (hl > halo) { halo = hl; code = info.w; speckle = info.y; }
    }
}

/**
 * @brief `get_zone_wall_depth` gives the optical depth of a zone wall seen face-on (straight through its thickness) at
 * point p, `distance_s` from the eye, for the wall on the edge at Fermi u `wall_u_m`: the map's opacity at the ground,
 * falling to about `ZONE_WALL_BODY_FRAC` of it within `ZONE_WALL_FOOT_M`, then by `ZONE_WALL_TOP_FADE_FRAC` of that up
 * to the wall's height above the ground, over whose top `ZONE_WALL_SOFT_TOP_FRAC` it fades out; it also fades out over
 * the last `ZONE_WALL_END_M` before the boundary, and thins in the haze.
 * @par Example
 * @code
 * float depth = get_zone_wall_depth(scene, p, -45.0, s);
 * @endcode
 */
static float get_zone_wall_depth(thread const Scene &scene, float4 p, float wall_u_m, float distance_s) {
    constant ZoneWalls &walls = scene.u.zone_walls;
    float r = scene.u.curvature_radius_m;
    float4 look = walls.look, boundary = walls.boundary;
    float3 f = get_fermi(p, r);
    float v_end_m = boundary.y > 0.0 ? boundary.y * sqrt(max(1.0 - (wall_u_m / boundary.x) * (wall_u_m / boundary.x), 0.0)) : NO_HIT_S;
    float above_m = max(f.z - (scene.u.has_terrain != 0 ? terrain_height_m(scene, f.x, f.y) : 0.0), 0.0);
    float height_frac = above_m / look.x;
    float profile = mix(ZONE_WALL_BODY_FRAC * (1.0 - ZONE_WALL_TOP_FADE_FRAC * height_frac), 1.0, exp(-above_m / ZONE_WALL_FOOT_M))
                  * (1.0 - smoothstep(1.0 - ZONE_WALL_SOFT_TOP_FRAC, 1.0, height_frac));
    float haze_frac = 1.0 - exp(-max(distance_s * r - HAZE_START_M, 0.0) / HAZE_DISTANCE_M);
    float opacity = look.y * profile * smoothstep(0.0, ZONE_WALL_END_M, v_end_m - abs(f.y)) * (1.0 - HAZE_MAX_FRAC * haze_frac);
    return -log(1.0 - opacity);
}

/**
 * @brief `scan_zone_walls` adds the zone walls that the chord from x along v passes through before `s_end` (`travelled`
 * from the eye) to `dust`, as glowing slabs in their team's colour. A wall is the slab within half its thickness of
 * the plane straight above its zone's edge (`zone_walls.rs`, the plane's (n_z, -n_w) in `walls[i].xy`): |⟨x, n⟩| ≤ k
 * with k the sinh of half the thickness over R. Along the chord ⟨x, n⟩ = cosh s (a + b tanh s), so the chord is
 * inside where (b² + k²) τ² + 2ab τ + a² − k² ≤ 0 for τ = tanh s, an interval found in closed form. Of the face-on
 * depth (`get_zone_wall_depth`), `ZONE_WALL_FACE_FRAC` is in the two faces, each crossed face adding its half,
 * thicker as the chord grazes it (up to `ZONE_WALL_MAX_SLANT` times), and the rest is spread through the slab,
 * summed along the chord at most `ZONE_WALL_SAMPLE_M` apart (and at most `ZONE_WALL_MAX_SAMPLES` samples): face-on
 * the wall looks as thin as the map says, and seen at a slant its tint builds up with the way through it.
 * @par Example
 * @code
 * scan_zone_walls(scene, x, v, s, t, dust);
 * @endcode
 */
static void scan_zone_walls(thread const Scene &scene, float4 x, float4 v, float s_end, float travelled, thread float4 &dust) {
    constant ZoneWalls &walls = scene.u.zone_walls;
    float r = scene.u.curvature_radius_m, thickness_m = walls.look.z;
    float k = sinh(0.5 * thickness_m / r), t_end = tanh(min(s_end, FAR_S));
    for (int i = 0; i < walls.counts.x; i++) {
        float4 wall = walls.walls[i];
        float a = x.z * wall.x + x.w * wall.y, b = v.z * wall.x + v.w * wall.y, discriminant = b * b - a * a + k * k;
        if (discriminant < 0.0) continue;
        float half_width = k * sqrt(discriminant) / (b * b + k * k), middle = -a * b / (b * b + k * k);
        float t_in = middle - half_width, t_out = middle + half_width;
        if (t_out <= 0.0 || t_in >= t_end) continue;
        float s_in = clamp_atanh(max(t_in, 0.0)), s_out = clamp_atanh(min(t_out, t_end));
        int sample_count = clamp(int(ceil((s_out - s_in) * r / ZONE_WALL_SAMPLE_M)), 1, ZONE_WALL_MAX_SAMPLES);
        float ds = (s_out - s_in) / float(sample_count), depth = 0.0;
        for (int j = 0; j < sample_count; j++) {
            float s = s_in + (float(j) + 0.5) * ds;
            depth += get_zone_wall_depth(scene, geodesic_point(x, v, s), wall.z, travelled + s) * ds * r / thickness_m;
        }
        depth *= 1.0 - ZONE_WALL_FACE_FRAC;
        float face_cos_scale = rsqrt(1.0 + k * k);
        if (t_in > 0.0) {
            float slant = min(1.0 / max(abs(a * sinh(s_in) + b * cosh(s_in)) * face_cos_scale, TINY), ZONE_WALL_MAX_SLANT);
            depth += 0.5 * ZONE_WALL_FACE_FRAC * slant * get_zone_wall_depth(scene, geodesic_point(x, v, s_in), wall.z, travelled + s_in);
        }
        if (t_out < t_end) {
            float slant = min(1.0 / max(abs(a * sinh(s_out) + b * cosh(s_out)) * face_cos_scale, TINY), ZONE_WALL_MAX_SLANT);
            depth += 0.5 * ZONE_WALL_FACE_FRAC * slant * get_zone_wall_depth(scene, geodesic_point(x, v, s_out), wall.z, travelled + s_out);
        }
        dust += float4(to_linear(wall.w > HALF_KIND ? RED_COLOR : BLUE_COLOR) * ZONE_WALL_BRIGHTNESS * depth, depth);
    }
}

/**
 * @brief `scan_dust` adds the dust of shots into the ground near a chord (x along v for s_end, `travelled` from the
 * eye) to `dust`: the summed colour times optical depth and the summed depth. Each burst the chord comes near is
 * scanned in its Klein coordinates (meters), where the chord is straight: a grain is a flat-faced polyhedron
 * (`cover_grain`) and a puff a Gaussian ball, at least `DUST_MIN_WIDTH_PX` pixels wide (and fainter as it widens),
 * and its colour is its albedo lit by the sun (more on the side facing it, glowing when seen toward it) and the
 * ambient light.
 * @par Example
 * @code
 * scan_dust(scene, x, v, s, t, dust);
 * @endcode
 */
/** @brief `rotate_by` turns v by unit quaternion q. @par Example @code float3 w = rotate_by(q, v); @endcode */
static float3 rotate_by(float4 q, float3 v) { return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v); }

/**
 * @brief `cover_grain` gives how much of a pixel a grain covers, for the ray through `p` (from the grain's centre, in
 * units of the grain's size) along unit vector u, both in the burst's axes: the polyhedron of `shape` turned by
 * `orientation`, with a pixel-wide edge (`pixels_per_size` pixels per unit), and the normal of the face the ray enters
 * (in the burst's axes) into n. The ray's gap to the polyhedron is measured midway between where it crosses into the
 * last face's inside and out of the first: inside the polyhedron when the ray goes through it, near the closest point
 * when it misses.
 * @par Example
 * @code
 * float coverage = cover_grain(2, particle.orientation, off / width_m, dir, width_m / pixel_m, n);
 * @endcode
 */
static float cover_grain(int shape, float4 orientation, float3 p, float3 u, float pixels_per_size, thread float3 &n) {
    float4 inverse = float4(-orientation.xyz, orientation.w);
    float3 p_local = rotate_by(inverse, p), u_local = rotate_by(inverse, u), n_local = float3(0.0, 1.0, 0.0);
    float entry = -NO_HIT_S, exit = NO_HIT_S, gap = -NO_HIT_S;
    int2 range = GRAIN_PLANE_RANGES[shape];
    for (int i = range.x; i < range.x + range.y; i++) {
        float4 plane = GRAIN_PLANES[i];
        float a = dot(plane.xyz, p_local) - plane.w, b = dot(plane.xyz, u_local);
        if (b < 0.0 && -a / b > entry) { entry = -a / b; n_local = plane.xyz; }
        if (b > 0.0) exit = min(exit, -a / b);
    }
    float3 middle = p_local + 0.5 * (entry + exit) * u_local;
    for (int i = range.x; i < range.x + range.y; i++) gap = max(gap, dot(GRAIN_PLANES[i].xyz, middle) - GRAIN_PLANES[i].w);
    n = rotate_by(orientation, n_local);
    return saturate(0.5 - gap * pixels_per_size);
}

static void scan_dust(thread const Scene &scene, float4 x, float4 v, float s_end, float travelled, thread float4 &dust) {
    float r = scene.u.curvature_radius_m, s_max = min(s_end, FAR_S);
    float3 ambient = float3(AMBIENT) + EARTHSHINE * DUST_EARTHSHINE_FRAC * EARTHSHINE_COLOR;
    for (int b = 0; b < scene.u.dust_counts.x; b++) {
        device const DustBurst &burst = scene.dust_bursts[b];
        float x_w = dot(burst.rows[3], x), v_w = dot(burst.rows[3], v);
        float s_c = clamp(clamp_atanh(-v_w / x_w), 0.0, s_max);
        float pixel_m = scene.u.pixel_angle_rad * r * sinh(min(travelled + s_c, FAR_S)), footprint_m = DUST_MIN_WIDTH_PX * pixel_m;
        float reach = (burst.bound.x + DUST_CUTOFF_WIDTHS * max(burst.bound.y, footprint_m)) / r;
        if (cosh(s_c) * x_w + sinh(s_c) * v_w > cosh(reach)) continue;
        float4 p = float4(dot(burst.rows[0], x), dot(burst.rows[1], x), dot(burst.rows[2], x), x_w);
        float4 e = p + tanh(s_max) * float4(dot(burst.rows[0], v), dot(burst.rows[1], v), dot(burst.rows[2], v), v_w);
        float3 q0 = p.xyz / p.w * r, d = e.xyz / max(e.w, TINY) * r - q0;
        float dd = max(dot(d, d), TINY);
        float3 dir = d * rsqrt(dd), sun = burst.sun.xyz;
        float forward = DUST_FORWARD_GLOW * pow(saturate(dot(dir, sun)), DUST_FORWARD_POWER);
        for (int i = burst.range.x; i < burst.range.x + burst.range.y; i++) {
            device const DustParticle &particle = scene.dust_particles[i];
            float4 pr = particle.position_radius, co = particle.color_opacity;
            float3 off = q0 + clamp(dot(pr.xyz - q0, d) / dd, 0.0, 1.0) * d - pr.xyz;
            float width_m = max(pr.w, footprint_m), q = dot(off, off) / (width_m * width_m);
            if (q > DUST_CUTOFF_WIDTHS * DUST_CUTOFF_WIDTHS) continue;
            int shape = int(particle.shape.x + HALF_KIND);
            float3 n = q < 1.0 ? off / width_m - sqrt(1.0 - q) * dir : off * rsqrt(max(dot(off, off), TINY));
            float coverage = shape == DUST_PUFF ? exp(-q) : cover_grain(shape, particle.orientation, off / width_m, dir, width_m / pixel_m, n);
            float depth = co.w * (pr.w / width_m) * (pr.w / width_m) * coverage;
            float wrap = shape == DUST_PUFF ? DUST_WRAP : GRAIN_WRAP;
            float facing = saturate((dot(n, sun) + wrap) / (1.0 + wrap));
            dust += float4(co.rgb * (SUN_INTENSITY * SUN_COLOR * burst.sun.w * (facing + forward) + ambient) * depth, depth);
        }
    }
}

/**
 * @brief `back_to_reach` moves a ray that has just left warp w's reach (by up to one step) back along its straight line
 * to the reach's sphere, so that an escaping ray's direction is carried to the eye from where its bending ended rather
 * than from wherever its last step happened to end, which jumps between neighbouring rays.
 * @par Example
 * @code
 * back_to_reach(scene, last_warp, x, v);
 * @endcode
 */
static void back_to_reach(thread const Scene &scene, int w, thread float4 &x, thread float4 &v) {
    float4 c = scene.warps[w].centre;
    float a = -mdot(x, c), b = -mdot(v, c), closest_sq = a * a - b * b, reach_cosh = cosh(scene.warps[w].params.z);
    if (b <= 0.0 || closest_sq >= reach_cosh * reach_cosh) return;
    float s_back = max(clamp_atanh(b / a) - acosh(reach_cosh * rsqrt(max(closest_sq, 1.0))), 0.0);
    float4 x_back = geodesic_point(x, -v, s_back);
    v = -geodesic_tangent(x, -v, s_back);
    x = x_back;
}

/**
 * @brief `trace_ray` traces a ray backwards from the eye through a point of the image (in pixels from its top left
 * corner): straight chords between warp objects, bent steps inside their reach, through wormhole throats, until a
 * surface, the sky or a horizon. After `LATE_STEP_COUNT` iterations (rays winding near a throat or the black hole,
 * which land in the lens's most compressed, aliased images) its bent steps are three times longer.
 * @par Example
 * @code
 * VoxelRayEnd end = trace_ray(scene, float2(id) + 0.5);
 * @endcode
 */
static VoxelRayEnd trace_ray(thread const Scene &scene, float2 pixel) {
    constant VoxelUniforms &u = scene.u;
    device const VoxelWarp *warps = scene.warps;
    float r = u.curvature_radius_m;
    float2 ndc = float2(2.0 * pixel.x / float(u.width_px) - 1.0, 1.0 - 2.0 * pixel.y / float(u.height_px));
    float3 d_cam = normalize(float3(ndc.x * u.tan_half_fov * u.aspect, ndc.y * u.tan_half_fov, -1.0));
    float4 x = u.camera[3], v = u.camera[0] * d_cam.x + u.camera[1] * d_cam.y + u.camera[2] * d_cam.z;
    int inside = -1;
    for (int w = 0; w < u.warp_count; w++) {
        if (-mdot(x, warps[w].centre) < cosh(warps[w].params.z)) inside = w;
    }
    float travelled = 0.0, glass = 0.0, core = 0.0, halo = 0.0, code = 0.0, speckle = 0.0;
    float4 dust = float4(0.0);
    Hit hit;
    hit.s = NO_HIT_S; hit.mat = 0; hit.k = -1; hit.axis = -1; hit.sgn = 0.0; hit.g = float3(0.0); hit.cell = int3(0);
    int outcome = OUTCOME_LOST, last_warp = -1, tint = 0;
    float4 hit_x = x, hit_v = v;
    float hit_travelled = 0.0;
    float min_step = MIN_BEND_STEP_M / r, max_step = MAX_BEND_STEP_M / r, rho = 0.0;
    float4 gradient = float4(0.0);
    float3 local = float3(0.0);
    bool has_gradient = false;
    for (int it = 0; it < MAX_ITERATIONS; it++) {
        if (inside < 0) {
            float s_end = FAR_S;
            int entering = -1;
            for (int w = 0; w < u.warp_count; w++) {
                float a = -mdot(x, warps[w].centre), b = mdot(v, warps[w].centre), closest_sq = a * a - b * b, reach_cosh = cosh(warps[w].params.z);
                if (b <= 0.0 || closest_sq >= reach_cosh * reach_cosh) continue;
                float s_in = max(clamp_atanh(b / a) - acosh(reach_cosh * rsqrt(max(closest_sq, 1.0))), 0.0);
                if (s_in < s_end) { s_end = s_in; entering = w; }
            }
            hit.s = s_end;
            hit.mat = 0;
            float glass_s = NO_HIT_S;
            trace_scene(scene, x, v, -1, 0.0, float3(0.0), hit, glass_s);
            if (glass_s < hit.s) glass += 1.0;
            if (u.beam_count > 0) scan_beams(scene, x, v, hit.s, travelled, core, halo, code, speckle);
            if (u.dust_counts.x > 0) scan_dust(scene, x, v, hit.s, travelled, dust);
            if (u.zone_walls.counts.x > 0) scan_zone_walls(scene, x, v, hit.s, travelled, dust);
            if (hit.mat != 0) { outcome = OUTCOME_HIT; hit_x = x; hit_v = v; hit_travelled = travelled; break; }
            if (entering < 0) {
                if (last_warp >= 0) back_to_reach(scene, last_warp, x, v);
                outcome = OUTCOME_SKY;
                break;
            }
            float4 xn = geodesic_point(x, v, s_end);
            v = geodesic_tangent(x, v, s_end);
            x = xn;
            travelled += s_end;
            inside = entering;
            continue;
        }
        if (!has_gradient) gradient = compute_gradient(scene, inside, x, rho, local);
        float ds = clamp((it < LATE_STEP_COUNT ? BEND_STEP_FRAC : LATE_BEND_STEP_FRAC) * rho, min_step, max_step);
        v = normalize_tangent(x, v + (gradient - mdot(gradient, v) * v) * (0.5 * ds));
        hit.s = ds;
        hit.mat = 0;
        float glass_s = NO_HIT_S;
        trace_scene(scene, x, v, inside, rho, local, hit, glass_s);
        if (glass_s < hit.s) glass += 1.0;
        if (u.beam_count > 0) scan_beams(scene, x, v, hit.s, travelled, core, halo, code, speckle);
        if (u.dust_counts.x > 0) scan_dust(scene, x, v, hit.s, travelled, dust);
        if (u.zone_walls.counts.x > 0) scan_zone_walls(scene, x, v, hit.s, travelled, dust);
        if (hit.mat != 0) { outcome = OUTCOME_HIT; hit_x = x; hit_v = v; hit_travelled = travelled; break; }
        float4 xn = geodesic_point(x, v, ds), vn = geodesic_tangent(x, v, ds);
        x = xn * rsqrt(-mdot(xn, xn));
        v = normalize_tangent(x, vn);
        travelled += ds;
        gradient = compute_gradient(scene, inside, x, rho, local);
        v = normalize_tangent(x, v + (gradient - mdot(gradient, v) * v) * (0.5 * ds));
        has_gradient = true;
        float4 prm = warps[inside].params;
        if (prm.x < HALF_KIND && rho < prm.y * HORIZON_CAPTURE_FRAC) { outcome = OUTCOME_LOST; break; }
        if (prm.x > HALF_KIND && rho < prm.y) {
            if (prm.x > FIRST_THROWN_KIND) tint = int(prm.x + 0.5) - 1;
            cross_throat(scene, inside, x, v);
            inside = int(prm.w + 0.5);
            has_gradient = false;
            continue;
        }
        if (rho > prm.z) { last_warp = inside; inside = -1; has_gradient = false; }
    }
    int beam_code = int(code + HALF_KIND);
    VoxelRayEnd end = {float4(0.0), float4(0.0), float4(0.0, 0.0, 0.0, travelled), int4(0),
                       int4(outcome, -1, -1, (beam_code & 1) | tint << THROWN_TINT_FIRST_BIT | ((beam_code & RIFLE_BEAM_CODE) != 0 ? 1 : 0) << BEAM_RIFLE_BIT
                                                 | int(round(speckle * BEAM_SPECKLE_STEPS)) << BEAM_SPECKLE_FIRST_BIT),
                       float4(0.0, glass, core, halo), dust};
    if (outcome == OUTCOME_HIT) {
        end.point = geodesic_point(hit_x, hit_v, hit.s);
        end.view = geodesic_tangent(hit_x, hit_v, hit.s);
        end.grid.w = hit_travelled + hit.s;
        end.cell.w = hit.mat;
        if (hit.mat != MAT_GROUND) {
            end.grid.xyz = hit.g;
            end.cell.xyz = hit.cell;
            end.surface.yz = int2(hit.k, hit.axis);
            end.glow.x = hit.sgn;
        }
    } else if (outcome == OUTCOME_SKY) {
        end.point = float4(get_sky_direction(scene, x, v), 0.0);
    }
    return end;
}

/**
 * @brief `add_haze` mixes a thin haze into the colour of a ray that hit a surface beyond `HAZE_START_M` (more the farther
 * it went) or reached the sky (more the nearer the horizon), brighter toward the sun; a lost ray stays as it is.
 * @par Example
 * @code
 * color = add_haze(scene, color, end);
 * @endcode
 */
static float3 add_haze(thread const Scene &scene, float3 color, VoxelRayEnd end) {
    float haze_frac, sun_cos;
    if (end.surface.x == OUTCOME_HIT) {
        haze_frac = 1.0 - exp(-max(end.grid.w * scene.u.curvature_radius_m - HAZE_START_M, 0.0) / HAZE_DISTANCE_M);
        sun_cos = mdot(end.view, transport_from_centre(scene.u.sun.xyz, end.point));
    } else if (end.surface.x == OUTCOME_SKY) {
        haze_frac = exp(-max(end.point.y, 0.0) / HAZE_HORIZON_HEIGHT_FRAC);
        sun_cos = dot(end.point.xyz, scene.u.sun.xyz);
    } else {
        return color;
    }
    if (scene.u.snow.x > 0.5) {
        float far = end.surface.x == OUTCOME_HIT ? 1.0 - exp(-max(end.grid.w * scene.u.curvature_radius_m - SNOW_FOG_START_M, 0.0) / scene.u.snow.x) : haze_frac;
        float3 fog = to_linear(SNOW_HORIZON) + SUN_COLOR * HAZE_SUN_GLOW * pow(max(sun_cos, 0.0), HAZE_SUN_POWER);
        return mix(color, fog, SNOW_FOG_MAX_FRAC * far);
    }
    float3 haze = HAZE_COLOR + SUN_COLOR * HAZE_SUN_GLOW * pow(max(sun_cos, 0.0), HAZE_SUN_POWER);
    return mix(color, haze, HAZE_MAX_FRAC * haze_frac);
}

/**
 * @brief `add_blizzards` fogs a pixel's colour by the blizzards its first ray crosses (straight in Fermi coordinates, as space is
 * nearly flat on a snow map) and adds their falling flakes: a few hashed flakes per cubic meter, one cell a step along the ray.
 * @par Example
 * @code
 * color = add_blizzards(scene, color, end, pixel);
 * @endcode
 */
static float3 add_blizzards(thread const Scene &scene, float3 color, VoxelRayEnd end, float2 pixel) {
    constant VoxelUniforms &u = scene.u;
    int count = int(u.snow.y);
    if ((count == 0 && u.snow.w <= 0.0) || end.surface.x == OUTCOME_LOST) return color;
    float r = u.curvature_radius_m;
    float2 ndc = float2(2.0 * pixel.x / float(u.width_px) - 1.0, 1.0 - 2.0 * pixel.y / float(u.height_px));
    float3 d_cam = normalize(float3(ndc.x * u.tan_half_fov * u.aspect, ndc.y * u.tan_half_fov, -1.0));
    float4 x = u.camera[3], v = u.camera[0] * d_cam.x + u.camera[1] * d_cam.y + u.camera[2] * d_cam.z;
    float3 a = get_fermi(x, r);
    float3 dir = normalize(get_fermi(geodesic_point(x, v, 1.0 / r), r) - a);
    float length_m = end.surface.x == OUTCOME_HIT ? length(get_fermi(end.point, r) - a) : BLIZZARD_SKY_M;
    float3 snow_light = to_linear(BLIZZARD_FOG);
    float transmit = 1.0;
    float3 added = float3(0.0);
    float now_s = u.snow.z;
    int light_steps = int(min(length_m, SNOW_FLAKE_M) / SNOW_FLAKE_STEP_M);
    for (int k = 0; k < light_steps; k++) {
        float t = (float(k) + 0.5) * SNOW_FLAKE_STEP_M;
        float3 p = a + dir * t - FLAKE_FALL_M_PER_S.xzy * now_s;
        float3 cell = floor(p);
        if (hash_3d(cell + 71.0) > u.snow.w) continue;
        float3 q = p - (cell + float3(hash_3d(cell + 17.0), hash_3d(cell + 31.0), hash_3d(cell + 47.0)));
        float miss = length(q - dir * dot(q, dir));
        float size_m = max(FLAKE_RADIUS_M, u.pixel_angle_rad * t * 0.7);
        added += saturate(1.0 - miss / size_m) * min(1.0, FLAKE_RADIUS_M / size_m * 2.0) * snow_light;
    }
    for (int i = 0; i < min(count, MAX_BLIZZARD_COUNT); i++) {
        float4 b = u.blizzards[i], air = u.blizzard_air[i];
        float2 o = a.xy - b.xy, dd = dir.xy;
        float qa = dot(dd, dd), qb = dot(o, dd), qc = dot(o, o) - b.z * b.z;
        float t0 = 0.0, t1 = length_m;
        if (qa > 1e-6) {
            float disc = qb * qb - qa * qc;
            if (disc <= 0.0) continue;
            t0 = max(t0, (-qb - sqrt(disc)) / qa);
            t1 = min(t1, (-qb + sqrt(disc)) / qa);
        } else if (qc >= 0.0) {
            continue;
        }
        if (abs(dir.z) > 1e-6) {
            float t_top = (b.w - a.z) / dir.z;
            if (dir.z > 0.0) t1 = min(t1, t_top); else t0 = max(t0, t_top);
        } else if (a.z >= b.w) {
            continue;
        }
        if (t1 <= t0) continue;
        int steps = min(int((t1 - t0) / FLAKE_STEP_M), FLAKE_MAX_STEPS);
        for (int k = 0; k < steps; k++) {
            float t = t0 + (float(k) + 0.5) * FLAKE_STEP_M;
            float3 p = a + dir * t - FLAKE_FALL_M_PER_S.xzy * now_s;
            float3 cell = floor(p);
            float h = hash_3d(cell);
            if (h > air.y) continue;
            float3 flake = cell + float3(hash_3d(cell + 17.0), hash_3d(cell + 31.0), hash_3d(cell + 47.0));
            float3 q = p - flake;
            float miss = length(q - dir * dot(q, dir));
            float size_m = max(FLAKE_RADIUS_M, u.pixel_angle_rad * t * 0.7);
            float cover = saturate(1.0 - miss / size_m) * min(1.0, FLAKE_RADIUS_M / size_m * 2.0);
            added += transmit * exp(-air.x * (t - t0)) * cover * snow_light;
        }
        float fade = exp(-air.x * (t1 - t0));
        added += transmit * (1.0 - fade) * snow_light;
        transmit *= fade;
    }
    return color * transmit + added;
}

/**
 * @brief `hash_pixel` gives a pseudo-random number in [0, 1) for pixel `id` and `seed`, independent from pixel to
 * pixel (a lowbias32 integer hash).
 * @par Example
 * @code
 * float roll = hash_pixel(id, uint(scene.u.beam_speckle.x));
 * @endcode
 */
static float hash_pixel(uint2 id, uint seed) {
    uint h = id.x * PIXEL_HASH_PRIMES.x ^ id.y * PIXEL_HASH_PRIMES.y ^ seed * PIXEL_HASH_PRIMES.z;
    h ^= h >> PIXEL_HASH_SHIFTS.x;
    h *= PIXEL_HASH_MULTIPLIERS.x;
    h ^= h >> PIXEL_HASH_SHIFTS.y;
    h *= PIXEL_HASH_MULTIPLIERS.y;
    h ^= h >> PIXEL_HASH_SHIFTS.z;
    return float(h >> PIXEL_HASH_DROPPED_BITS) * PIXEL_HASH_SCALE;
}

/**
 * @brief `speckle_beam` gives pixel `id`'s colour with its laser's speckle (W-84). On the share `speckle` of the pixels
 * the laser's `glow` reaches (above `MIN_SPECKLE_GLOW`), picked anew each frame with the uniforms' `beam_speckle.x`
 * seed, a second roll picks the pixel's kind in the uniforms' shares `beam_speckle.y` and `.z`: black (`lit` darkened
 * by the glow, black where it is full), white (`lit` moved toward white by the glow) or, for the rest, a dropout
 * (`bare`, the scene without the laser). Every other pixel keeps `lit`, the normal laser.
 * @par Example
 * @code
 * color = speckle_beam(scene, bare, lit, glow, 0.3, id);
 * @endcode
 */
static float3 speckle_beam(thread const Scene &scene, float3 bare, float3 lit, float glow, float speckle, uint2 id) {
    uint seed = uint(scene.u.beam_speckle.x);
    if (speckle <= 0.0 || glow <= MIN_SPECKLE_GLOW || hash_pixel(id, seed) >= speckle) return lit;
    float kind = hash_pixel(id, seed ^ SPECKLE_KIND_SALT);
    if (kind < scene.u.beam_speckle.y) return lit * (1.0 - glow);
    if (kind < scene.u.beam_speckle.y + scene.u.beam_speckle.z) return mix(lit, float3(1.0), glow);
    return bare;
}

/**
 * @brief `shade_ray_end` gives pixel `id`'s final colour from where its ray ended: the lit surface or the sky, hazed,
 * tinted by the glass it crossed, behind the dust and zone walls in front of it (as thick as their optical depth), with the laser's
 * glow (the sniper's a white blade in a soft haze, the rifle's a laser over its halo) speckled by `speckle_beam`; a
 * lost ray is black.
 * @par Example
 * @code
 * float3 color = shade_ray_end(scene, trace_ray(scene, float2(id) + 0.5), id);
 * @endcode
 */
static float3 shade_ray_end(thread const Scene &scene, VoxelRayEnd end, uint2 id) {
    float3 color = float3(0.0);
    if (end.surface.x == OUTCOME_HIT && end.cell.w == MAT_GROUND) {
        color = shade_ground(scene, end.point, end.view, end.grid.w);
    } else if (end.surface.x == OUTCOME_HIT) {
        Hit hit = {0.0, end.cell.w, end.surface.y, end.surface.z, end.glow.x, end.grid.xyz, end.cell.xyz};
        color = end.cell.w == MAT_MESH ? shade_mesh(scene, hit, end.point, end.view, end.grid.w) : shade_block(scene, hit, end.point, end.view, end.grid.w);
    } else if (end.surface.x == OUTCOME_SKY) {
        color = to_linear(sample_sky(scene, end.point.xyz));
    }
    color = add_haze(scene, color, end);
    color = add_blizzards(scene, color, end, float2(id) + 0.5);
    float glass = end.glow.y, core = end.glow.z, halo = end.glow.w;
    float3 beam_color = to_linear(scene.u.laser_color.w > 0.5 ? scene.u.laser_color.rgb : (end.surface.w & 1) != 0 ? RED_COLOR : BLUE_COLOR);
    if (glass > 0.0) color = color * pow(GLASS_TINT, glass) + to_linear(GLASS_SHEEN);
    int tint = end.surface.w >> THROWN_TINT_FIRST_BIT & THROWN_TINT_MASK;
    if (tint > 0) color = mix(color, THROWN_TINTS[tint - 1] * (dot(color, LUMA) + THROWN_TINT_FLOOR), THROWN_TINT_FRAC);
    if (end.dust.w > 0.0) color = mix(color, end.dust.rgb / end.dust.w, 1.0 - exp(-end.dust.w));
    float3 lit;
    float glow;
    float whiteness_frac = scene.u.laser_color.w > 0.5 ? ONE_COLOR_LASER_WHITENESS_FRAC : 1.0;
    if ((end.surface.w >> BEAM_RIFLE_BIT & 1) != 0) {
        lit = mix(color, beam_color, saturate(LASER_HALO_OPACITY * halo)) + mix(beam_color, float3(1.0), LASER_CORE_WHITENESS * whiteness_frac) * core;
        glow = saturate(LASER_HALO_OPACITY * halo + core);
    } else {
        lit = color + mix(beam_color, float3(1.0), BEAM_HAZE_WHITENESS * whiteness_frac) * BEAM_HAZE_BRIGHTNESS * halo;
        lit = mix(lit, mix(beam_color, float3(1.0), BEAM_CORE_WHITENESS * whiteness_frac) * BEAM_CORE_BRIGHTNESS, saturate(core));
        glow = saturate(core + BEAM_HAZE_BRIGHTNESS * halo);
    }
    color = speckle_beam(scene, color, lit, glow, float(end.surface.w >> BEAM_SPECKLE_FIRST_BIT) / BEAM_SPECKLE_STEPS, id);
    return max(color, 0.0);
}

/** @brief `write_pixel` shades a ray end into pixel `id`. @par Example @code write_pixel(scene, end, id, image); @endcode */
static void write_pixel(thread const Scene &scene, VoxelRayEnd end, uint2 id, texture2d<float, access::write> image) {
    image.write(float4(shade_ray_end(scene, end, id), 1.0), id);
}

/**
 * @brief `trace_voxel_pixels` traces every pixel's ray and writes its colour (`--sampling every-pixel`).
 * @par Example
 * @code
 * dispatched by render_voxel.rs, one thread per pixel.
 * @endcode
 */
kernel void trace_voxel_pixels(SCENE_BUFFERS, texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    write_pixel(scene, trace_ray(scene, float2(id) + 0.5), id, image);
}

/**
 * @brief `is_same_surface` says whether two samples ended the same way with no laser glow, so the pixels between them
 * can be interpolated: both on one face plane of one structure (same axis, side and layer of cells), both on the
 * ground, both in the sky or both lost, behind the same number of glass faces and nearly as much dust.
 * @par Example
 * @code
 * if (!is_same_surface(a, b)) return false;
 * @endcode
 */
static bool is_same_surface(VoxelRayEnd a, VoxelRayEnd b) {
    int axis = a.surface.z;
    bool is_inside_cell = a.surface.x == OUTCOME_HIT && a.surface.y >= 0 && axis < 0;
    return all(a.surface == b.surface) && a.glow.x == b.glow.x && a.glow.y == b.glow.y && !is_inside_cell
        && (axis < 0 || a.cell[axis] == b.cell[axis]) && b.glow.z < GLOW_FLOOR && b.glow.w < GLOW_FLOOR
        && abs(b.dust.w - a.dust.w) <= DUST_SMOOTH_DEPTH;
}

/**
 * @brief `is_line_smooth` says whether four samples in a line map screen to scene nearly linearly between the middle
 * two: the second differences, which are 8 times the midpoint's interpolation error, stay under `ratio` of the first
 * difference. For samples `h` pixels apart that bounds the error by `ratio` × h / 8 pixels.
 * @par Example
 * @code
 * bool is_smooth = is_line_smooth(p0, p1, p2, p3, SMOOTH_SECOND_DIFFERENCE_PER_FIRST);
 * @endcode
 */
static bool is_line_smooth(float4 p0, float4 p1, float4 p2, float4 p3, float ratio) {
    float limit = ratio * length(p2 - p1);
    return length(p0 - 2.0 * p1 + p2) <= limit && length(p1 - 2.0 * p2 + p3) <= limit;
}

/**
 * @brief `is_cell_smooth` says whether the cell between samples (i, j) and (i + 1, j + 1) of a grid `columns` ×
 * `grid_rows` can be interpolated: the twelve samples around it (a 4×4 block without its corners) ended on the
 * corner's surface and map nearly linearly along the cell's two rows and two columns. Border cells never are.
 * @par Example
 * @code
 * bool is_smooth = is_cell_smooth(samples, columns, sample_rows, i, j, SMOOTH_SECOND_DIFFERENCE_PER_FIRST);
 * @endcode
 */
static bool is_cell_smooth(device const VoxelRayEnd *grid, int columns, int grid_rows, int i, int j, float ratio) {
    if (i < 1 || j < 1 || i + 2 >= columns || j + 2 >= grid_rows) return false;
    VoxelRayEnd corner = grid[j * columns + i];
    float4 p[4][4];
    for (int b = 0; b < 4; b++) {
        for (int a = 0; a < 4; a++) {
            if ((a == 0 || a == 3) && (b == 0 || b == 3)) continue;
            VoxelRayEnd sample = grid[(j - 1 + b) * columns + (i - 1 + a)];
            if (!is_same_surface(corner, sample)) return false;
            p[b][a] = sample.point;
        }
    }
    return is_line_smooth(p[1][0], p[1][1], p[1][2], p[1][3], ratio) && is_line_smooth(p[2][0], p[2][1], p[2][2], p[2][3], ratio)
        && is_line_smooth(p[0][1], p[1][1], p[2][1], p[3][1], ratio) && is_line_smooth(p[0][2], p[1][2], p[2][2], p[3][2], ratio);
}

/**
 * @brief `interpolate_ray_end` blends four samples around a point of a cell (`wx` and `wy` from 0 to 1 along it) into
 * `end`: the point (back on the hyperboloid, or the unit sphere for the sky), tangent, grid point, distance and dust
 * blend, and the rest is the first sample's. On a block face the cell under the blended grid point gives the
 * material, and the result is false (trace the ray instead) unless that cell is solid and the one in front of the
 * face is open.
 * @par Example
 * @code
 * if (!interpolate_ray_end(scene, a, b, c, d, 0.5, 0.0, end)) end = trace_ray(scene, pixel);
 * @endcode
 */
static bool interpolate_ray_end(thread const Scene &scene, VoxelRayEnd a, VoxelRayEnd b, VoxelRayEnd c, VoxelRayEnd d, float wx, float wy,
                                thread VoxelRayEnd &end) {
    end = a;
    end.dust = mix(mix(a.dust, b.dust, wx), mix(c.dust, d.dust, wx), wy);
    float4 point = mix(mix(a.point, b.point, wx), mix(c.point, d.point, wx), wy);
    end.grid = mix(mix(a.grid, b.grid, wx), mix(c.grid, d.grid, wx), wy);
    if (a.surface.x == OUTCOME_SKY) end.point = float4(normalize(point.xyz), 0.0);
    if (a.surface.x != OUTCOME_HIT) return true;
    end.point = point * rsqrt(-mdot(point, point));
    end.view = normalize_tangent(end.point, mix(mix(a.view, b.view, wx), mix(c.view, d.view, wx), wy));
    if (a.surface.y < 0) return true;
    return all(int3(b.surface.y, c.surface.y, d.surface.y) == a.surface.y) && all(int3(b.cell.x, c.cell.x, d.cell.x) == a.cell.x);
}

/** @brief `get_sample_columns` gives how many samples a row of the fine grid has (one per even pixel column, one past the edge). @par Example @code int columns = get_sample_columns(u); @endcode */
static int get_sample_columns(constant VoxelUniforms &u) { return u.width_px / 2 + 2; }

/** @brief `get_coarse_columns` gives how many samples a row of the coarse grid has (one per fourth pixel column, enough past the edge to cover the fine grid). @par Example @code int columns = get_coarse_columns(u); @endcode */
static int get_coarse_columns(constant VoxelUniforms &u) { return u.width_px / 4 + 3; }

/** @brief `get_centre_distance_px` gives how far a pixel lies from the image's centre, in pixels. @par Example @code float r_px = get_centre_distance_px(u, float2(id)); @endcode */
static float get_centre_distance_px(constant VoxelUniforms &u, float2 pixel) {
    return length(pixel + 0.5 - 0.5 * float2(u.width_px, u.height_px));
}

/**
 * @brief `get_fovea_level` gives the resolution of the 4 × 4 pixel block holding a pixel, from its centre's distance to
 * the image's centre (W-136, sharp centre, soft edges): `FOVEA_SHARP` (adaptive sampling as everywhere before) within
 * `u.fovea.x`, `FOVEA_HALF` (the even pixels shaded, the rest blended from them) within `u.fovea.y`, and beyond it a
 * quarter (only every fourth pixel each way shaded, the rest blended from them).
 * @par Example
 * @code
 * int level = get_fovea_level(u, id);
 * @endcode
 */
static int get_fovea_level(constant VoxelUniforms &u, uint2 pixel) {
    float r_px = get_centre_distance_px(u, float2(pixel / FOVEA_BLOCK_PX * FOVEA_BLOCK_PX + FOVEA_BLOCK_PX / 2));
    return r_px < u.fovea.x ? FOVEA_SHARP : (r_px < u.fovea.y ? FOVEA_HALF : FOVEA_QUARTER);
}

/**
 * @brief `is_sample_needed` says whether the fine sample at an even pixel must be traced or interpolated: it may be
 * shaded or read by a block that isn't at a quarter resolution, which lies within `FOVEA_SAMPLE_MARGIN_PX` of it (the
 * half block's diagonal and the two samples `is_cell_smooth` reads beyond a cell).
 * @par Example
 * @code
 * if (!is_sample_needed(u, uint2(2 * i, 2 * j))) return;
 * @endcode
 */
static bool is_sample_needed(constant VoxelUniforms &u, uint2 pixel) {
    return get_centre_distance_px(u, float2(pixel)) - FOVEA_SAMPLE_MARGIN_PX < u.fovea.y;
}

/**
 * @brief `blend_pixels` gives the colour at pixel `id` blended bilinearly from the colours already written at the four
 * pixels `lo`, `lo` + (`spacing_px`, 0), `lo` + (0, `spacing_px`) and `lo` + (`spacing_px`, `spacing_px`); a corner past
 * the image's edge falls back to `lo`'s row or column.
 * @par Example
 * @code
 * image.write(blend_pixels(u, image, uint2(8, 4), 4, id), id);
 * @endcode
 */
static float4 blend_pixels(constant VoxelUniforms &u, texture2d<float, access::read_write> image, uint2 lo, uint spacing_px, uint2 id) {
    uint2 hi = select(lo + spacing_px, lo, lo + spacing_px >= uint2(u.width_px, u.height_px));
    float2 w = float2(id - lo) / float(spacing_px);
    float4 c00 = image.read(lo), c10 = image.read(uint2(hi.x, lo.y)), c01 = image.read(uint2(lo.x, hi.y)), c11 = image.read(hi);
    return mix(mix(c00, c10, w.x), mix(c01, c11, w.x), w.y);
}

/**
 * @brief `trace_coarse` traces the rays of every fourth pixel each way (coarse sample (i, j) is pixel (4i, 4j)), the
 * first pass of adaptive sampling, and starts this frame's lists empty.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per coarse sample.
 * @endcode
 */
kernel void trace_coarse(SCENE_BUFFERS, device VoxelRayEnd *coarse [[buffer(9)]], device atomic_uint *refine_count [[buffer(5)]],
                         device atomic_uint *fill_count [[buffer(12)]], uint2 id [[thread_position_in_grid]]) {
    int columns = get_coarse_columns(u), coarse_rows = u.height_px / 4 + 3;
    if (id.x == 0 && id.y == 0) {
        atomic_store_explicit(refine_count, 0, memory_order_relaxed);
        atomic_store_explicit(fill_count, 0, memory_order_relaxed);
    }
    if (int(id.x) >= columns || int(id.y) >= coarse_rows) return;
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    coarse[int(id.y) * columns + int(id.x)] = trace_ray(scene, 4.0 * float2(id) + 0.5);
}

/**
 * @brief `fill_samples` fills the fine grid (sample (i, j) is pixel (2i, 2j)), the second pass of adaptive sampling:
 * a sample on the coarse grid copies it, one inside a smooth coarse cell (with half the fine grid's threshold, for
 * the same error in pixels) interpolates the cell's corners, one no block needs
 * (`is_sample_needed`) copies its cell's first corner, and any other is listed for `trace_listed_samples`.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per fine sample.
 * @endcode
 */
kernel void fill_samples(SCENE_BUFFERS, device VoxelRayEnd *samples [[buffer(2)]], device const VoxelRayEnd *coarse [[buffer(9)]],
                         device uint *fill_list [[buffer(11)]], device atomic_uint *fill_count [[buffer(12)]], uint2 id [[thread_position_in_grid]]) {
    int columns = get_sample_columns(u), sample_rows = u.height_px / 2 + 2;
    int coarse_columns = get_coarse_columns(u), coarse_rows = u.height_px / 4 + 3;
    int i = int(id.x), j = int(id.y);
    if (i >= columns || j >= sample_rows) return;
    int ci = i / 2, cj = j / 2, di = i & 1, dj = j & 1, k = cj * coarse_columns + ci;
    if (di + dj == 0 || !is_sample_needed(u, 2 * id)) {
        samples[j * columns + i] = coarse[k];
        return;
    }
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    VoxelRayEnd end;
    if (is_cell_smooth(coarse, coarse_columns, coarse_rows, ci, cj, 0.5 * SMOOTH_SECOND_DIFFERENCE_PER_FIRST)
        && interpolate_ray_end(scene, coarse[k], coarse[k + di], coarse[k + dj * coarse_columns], coarse[k + dj * coarse_columns + di],
                               0.5 * float(di), 0.5 * float(dj), end)) {
        samples[j * columns + i] = end;
    } else {
        fill_list[atomic_fetch_add_explicit(fill_count, 1, memory_order_relaxed)] = uint(j * columns + i);
    }
}

/**
 * @brief `prepare_fill` sets the thread-group count of `trace_listed_samples`'s indirect dispatch from how many
 * samples `fill_samples` listed.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling as a single thread.
 * @endcode
 */
kernel void prepare_fill(device atomic_uint *fill_count [[buffer(12)]], device uint *dispatch_groups [[buffer(13)]]) {
    uint count = atomic_load_explicit(fill_count, memory_order_relaxed);
    dispatch_groups[0] = (count + REFINE_GROUP_SIZE - 1) / REFINE_GROUP_SIZE;
    dispatch_groups[1] = 1;
    dispatch_groups[2] = 1;
}

/**
 * @brief `trace_listed_samples` traces the fine samples `fill_samples` couldn't interpolate, packed densely.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling indirectly, one thread per listed sample.
 * @endcode
 */
kernel void trace_listed_samples(SCENE_BUFFERS, device VoxelRayEnd *samples [[buffer(2)]], device const uint *fill_list [[buffer(11)]],
                                 device atomic_uint *fill_count [[buffer(12)]], uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(fill_count, memory_order_relaxed)) return;
    int columns = get_sample_columns(u), k = int(fill_list[t]);
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    samples[k] = trace_ray(scene, 2.0 * float2(k % columns, k / columns) + 0.5);
}

/**
 * @brief `classify_cells` marks each cell between four fine samples smooth (1) when it lies in the sharp centre and
 * the twelve samples around it ended on one surface and map nearly linearly, and lists every other cell that covers
 * pixels, with its `get_fovea_level` from `REFINE_LEVEL_FIRST_BIT`, for `refine_pixels`.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per cell (sample (i, j) to (i + 1, j + 1)).
 * @endcode
 */
kernel void classify_cells(constant VoxelUniforms &u [[buffer(16)]], device const VoxelRayEnd *samples [[buffer(2)]],
                           device uchar *smooth_cells [[buffer(3)]], device uint *refine_cells [[buffer(4)]],
                           device atomic_uint *refine_count [[buffer(5)]], uint2 id [[thread_position_in_grid]]) {
    int columns = get_sample_columns(u), sample_rows = u.height_px / 2 + 2;
    int i = int(id.x), j = int(id.y);
    if (i >= columns || j >= sample_rows) return;
    int level = get_fovea_level(u, 2 * id);
    bool is_smooth = level == FOVEA_SHARP
        && is_cell_smooth(samples, columns, sample_rows, i, j, SMOOTH_SECOND_DIFFERENCE_PER_FIRST);
    smooth_cells[j * columns + i] = is_smooth ? 1 : 0;
    if (!is_smooth && 2 * i < u.width_px && 2 * j < u.height_px) {
        refine_cells[atomic_fetch_add_explicit(refine_count, 1, memory_order_relaxed)] = uint(j * columns + i) | uint(level) << REFINE_LEVEL_FIRST_BIT;
    }
}

/**
 * @brief `prepare_refinement` sets the thread-group count of `refine_pixels`'s indirect dispatch from how many cells
 * `classify_cells` listed.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling as a single thread.
 * @endcode
 */
kernel void prepare_refinement(device atomic_uint *refine_count [[buffer(5)]], device uint *dispatch_groups [[buffer(6)]]) {
    uint pixel_count = atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL;
    dispatch_groups[0] = (pixel_count + REFINE_GROUP_SIZE - 1) / REFINE_GROUP_SIZE;
    dispatch_groups[1] = 1;
    dispatch_groups[2] = 1;
}

/**
 * @brief `refine_pixels` traces the three odd pixels of each listed cell, packed densely so that thread groups don't
 * wait on the few pixels that need a full trace. Outside the sharp centre (W-136, sharp centre, soft edges) it blends
 * instead: from the cell's four even pixels at `FOVEA_HALF`, and from its 4 × 4 block's four corners at
 * `FOVEA_QUARTER`, where the third pixel's thread also writes the cell's even pixel unless `resolve_pixels` shaded it.
 * It only reads pixels `resolve_pixels` wrote, so its threads never race.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling indirectly, one thread per listed pixel.
 * @endcode
 */
kernel void refine_pixels(SCENE_BUFFERS, device const uint *refine_cells [[buffer(4)]], device atomic_uint *refine_count [[buffer(5)]],
                          texture2d<float, access::read_write> image [[texture(0)]], uint t [[thread_position_in_grid]]) {
    if (t >= atomic_load_explicit(refine_count, memory_order_relaxed) * REFINED_PIXELS_PER_CELL) return;
    int columns = get_sample_columns(u);
    uint entry = refine_cells[t / REFINED_PIXELS_PER_CELL];
    int cell = int(entry & REFINE_CELL_MASK), level = int(entry >> REFINE_LEVEL_FIRST_BIT), corner = int(t % REFINED_PIXELS_PER_CELL) + 1;
    uint2 lo = uint2(2 * (cell % columns), 2 * (cell / columns)), id = lo + uint2(corner & 1, corner >> 1);
    uint2 block = lo / FOVEA_BLOCK_PX * FOVEA_BLOCK_PX;
    if (level == FOVEA_QUARTER && corner == REFINED_PIXELS_PER_CELL && any(lo != block) && !is_sample_needed(u, lo)) {
        image.write(blend_pixels(u, image, block, FOVEA_BLOCK_PX, lo), lo);
    }
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    if (level != FOVEA_SHARP) {
        image.write(level == FOVEA_QUARTER ? blend_pixels(u, image, block, FOVEA_BLOCK_PX, id) : blend_pixels(u, image, lo, 2, id), id);
        return;
    }
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    image.write(float4(shade_ray_end(scene, trace_ray(scene, float2(id) + 0.5), id), 1.0), id);
}

/**
 * @brief `resolve_pixels` writes the pixels that need no listed ray: an even pixel takes its sample, and a pixel in a
 * smooth cell interpolates the cell's corners and shades the result (tracing its own ray if the blended cell isn't a
 * visible face). Outside the sharp centre it writes only even pixels whose sample is needed and every fourth pixel
 * each way. `refine_pixels` writes the rest.
 * @par Example
 * @code
 * dispatched by render_metal.rs's encode_adaptive_sampling, one thread per pixel.
 * @endcode
 */
kernel void resolve_pixels(SCENE_BUFFERS, device const VoxelRayEnd *samples [[buffer(2)]], device const uchar *smooth_cells [[buffer(3)]],
                           texture2d<float, access::write> image [[texture(0)]], uint2 id [[thread_position_in_grid]]) {
    if (int(id.x) >= u.width_px || int(id.y) >= u.height_px) return;
    int columns = get_sample_columns(u);
    int i = int(id.x) / 2, j = int(id.y) / 2, di = int(id.x) & 1, dj = int(id.y) & 1, k = j * columns + i;
    if (di + dj > 0 && smooth_cells[k] == 0) return;
    if (any(id % FOVEA_BLOCK_PX != 0) && !is_sample_needed(u, id)) return;
    Scene scene = {u, structures, shapes, warps, warp_lists, lights, palette, beams, clearances, surfaces, mound_heights, means, albedo_maps, normal_maps, arm_maps, dust_bursts, dust_particles, milky_way, mesh_triangles, mesh_nodes, mesh_materials, mesh_albedo, mesh_mr, mesh_normal, mesh_emissive, avatar_triangles, avatar_nodes};
    VoxelRayEnd end = samples[k];
    if (di + dj > 0
        && !interpolate_ray_end(scene, end, samples[k + di], samples[k + dj * columns], samples[k + dj * columns + di], 0.5 * float(di), 0.5 * float(dj), end)) {
        end = trace_ray(scene, float2(id) + 0.5);
    }
    write_pixel(scene, end, id, image);
}

/** @brief `aces_fitted` is Stephen Hill's fit of the ACES reference rendering transform (from the fps_demo). @par Example @code float3 d = aces_fitted(c); @endcode */
static float3 aces_fitted(float3 color) {
    const float3x3 input_matrix = float3x3(float3(0.59719, 0.07600, 0.02840), float3(0.35458, 0.90834, 0.13383), float3(0.04823, 0.01566, 0.83777));
    const float3x3 output_matrix = float3x3(float3(1.60475, -0.10208, -0.00327), float3(-0.53108, 1.10813, -0.07276), float3(-0.07367, -0.00605, 1.07602));
    float3 v = input_matrix * color;
    float3 a = v * (v + 0.0245786) - 0.000090537;
    float3 b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return saturate(output_matrix * (a / b));
}

/** @brief `linear_to_srgb` applies the sRGB transfer function. @par Example @code float3 e = linear_to_srgb(c); @endcode */
static float3 linear_to_srgb(float3 color) { return select(1.055 * pow(color, 1.0 / 2.4) - 0.055, color * 12.92, color <= 0.0031308); }

/** @brief `grade_scene` is the fps_demo's scene-linear grade: white balance, log contrast around middle grey, saturation. @par Example @code float3 g = grade_scene(c); @endcode */
static float3 grade_scene(float3 color) {
    color *= GRADE_WHITE_BALANCE;
    float luminance = max(dot(color, LUMA), 1e-6);
    float graded_luminance = GRADE_MIDDLE_GREY * pow(luminance / GRADE_MIDDLE_GREY, GRADE_CONTRAST);
    color *= graded_luminance / luminance;
    return max(mix(float3(graded_luminance), color, GRADE_SATURATION), 0.0);
}

/** @brief `grade_display` is the fps_demo's display grade: cool shadows and warm highlights. @par Example @code float3 g = grade_display(c); @endcode */
static float3 grade_display(float3 color) {
    float3 tint = mix(GRADE_SHADOW_TINT, GRADE_HIGHLIGHT_TINT, smoothstep(0.05, 0.6, saturate(dot(color, LUMA))));
    return saturate(color * tint);
}

/**
 * @brief `bloom_down` halves texture 1 into texture 2 for the bloom chain: four bilinear taps over a 4x4 block, each
 * damped by a weight like Karis' that keeps single, very bright pixels from blooming but lets glows and the sun bloom.
 * @par Example
 * @code
 * dispatched by render_voxel.rs once per bloom level, one thread per smaller pixel.
 * @endcode
 */
kernel void bloom_down(texture2d<float> source [[texture(1)]], texture2d<float, access::write> target [[texture(2)]], uint2 id [[thread_position_in_grid]]) {
    if (id.x >= target.get_width() || id.y >= target.get_height()) return;
    constexpr sampler edge(filter::linear, address::clamp_to_edge);
    float2 texel = 1.0 / float2(source.get_width(), source.get_height());
    float2 uv = (float2(id) + 0.5) / float2(target.get_width(), target.get_height());
    float3 sum = float3(0.0);
    float weight = 0.0;
    for (int i = 0; i < 4; i++) {
        float3 c = source.sample(edge, uv + texel * (float2(i & 1, i >> 1) * 2.0 - 1.0)).rgb;
        float w = 1.0 / (1.0 + dot(c, LUMA) / BLOOM_KARIS_LUMA);
        sum += c * w;
        weight += w;
    }
    target.write(float4(sum / weight, 1.0), id);
}

/**
 * @brief `get_spline_color` gives the HDR colour at pixel `id` as a cubic B-spline of the 4 × 4 shaded pixels around
 * it on the grid every `spacing_px` pixels each way: a smooth, ring-free blur about two grid steps wide, so a region
 * shaded at half or a quarter resolution shows no steps (W-138, no aliasing).
 * @par Example
 * @code
 * float3 c = get_spline_color(u, hdr, id, 2);
 * @endcode
 */
static float3 get_spline_color(constant VoxelUniforms &u, texture2d<float> hdr, uint2 id, uint spacing_px) {
    float2 g = float2(id) / float(spacing_px), f = fract(g), f2 = f * f, f3 = f2 * f;
    int2 base = int2(floor(g)) - 1, last = (int2(u.width_px, u.height_px) - 1) / int(spacing_px);
    float2 w[4] = {(1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0, (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0, (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0,
                   f3 / 6.0};
    float3 sum = float3(0.0);
    for (int b = 0; b < 4; b++) {
        for (int a = 0; a < 4; a++) {
            int2 k = clamp(base + int2(a, b), int2(0), last);
            sum += w[a].x * w[b].y * hdr.read(uint2(k * int(spacing_px))).rgb;
        }
    }
    return sum;
}

/**
 * @brief `get_soft_color` gives the HDR colour `tonemap_voxel` shows at pixel `id`: the pixel itself in the sharp
 * centre, blending over `SOFT_BAND_PX` into `get_spline_color` of the half-resolution grid outside it, and of the
 * quarter-resolution grid toward and beyond `u.fovea.y`.
 * @par Example
 * @code
 * float3 c = get_soft_color(u, hdr, id);
 * @endcode
 */
static float3 get_soft_color(constant VoxelUniforms &u, texture2d<float> hdr, uint2 id) {
    float3 color = hdr.read(id).rgb;
    float r_px = get_centre_distance_px(u, float2(id));
    float half_frac = r_px <= u.fovea.x ? 0.0 : smoothstep(u.fovea.x, u.fovea.x + SOFT_BAND_PX, r_px);
    float quarter_frac = r_px <= u.fovea.y - SOFT_BAND_PX ? 0.0 : smoothstep(u.fovea.y - SOFT_BAND_PX, u.fovea.y, r_px);
    if (half_frac > 0.0 && quarter_frac < 1.0) color = mix(color, get_spline_color(u, hdr, id, 2), half_frac);
    if (quarter_frac > 0.0) color = mix(color, get_spline_color(u, hdr, id, FOVEA_BLOCK_PX), quarter_frac);
    return color;
}

/**
 * @brief `tonemap_voxel` finishes the HDR image (texture 1) like the fps_demo: mixes in the bloom levels, adds the
 * bloom's bright parts as a glow, exposes, grades, tone maps with ACES, vignettes and encodes sRGB into texture 0,
 * with its luma in alpha for `fxaa_voxel`. It reads each pixel through `get_soft_color`.
 * @par Example
 * @code
 * dispatched by render_voxel.rs after tracing, one thread per pixel.
 * @endcode
 */
kernel void tonemap_voxel(constant VoxelUniforms &u [[buffer(16)]], texture2d<float> hdr [[texture(1)]],
                          array<texture2d<float>, BLOOM_LEVELS> bloom [[texture(7)]], texture2d<float, access::write> image [[texture(0)]],
                          uint2 id [[thread_position_in_grid]]) {
    if (id.x >= hdr.get_width() || id.y >= hdr.get_height()) return;
    constexpr sampler edge(filter::linear, address::clamp_to_edge);
    float2 uv = (float2(id) + 0.5) / float2(hdr.get_width(), hdr.get_height());
    float3 bloom_color = float3(0.0);
    for (int k = 0; k < BLOOM_LEVELS; k++) bloom_color += bloom[k].sample(edge, uv).rgb;
    float3 bloom_mean = bloom_color / float(BLOOM_LEVELS);
    float3 glow = bloom_mean * smoothstep(BLOOM_GLOW_LUMA_EDGES.x, BLOOM_GLOW_LUMA_EDGES.y, dot(bloom_mean, LUMA));
    float3 color = (mix(get_soft_color(u, hdr, id), bloom_mean, BLOOM_STRENGTH) + BLOOM_GLOW * glow) * EXPOSURE;
    color = grade_display(aces_fitted(grade_scene(color)));
    color *= 1.0 - VIGNETTE_STRENGTH * smoothstep(0.2, 0.85, length((uv - 0.5) * float2(1.1, 1.0)));
    float3 encoded = linear_to_srgb(saturate(color));
    image.write(float4(encoded, dot(encoded, LUMA)), id);
}

/**
 * @brief `fxaa_voxel` anti-aliases the tone-mapped image (texture 1, luma in alpha) into texture 0 with FXAA 3.11
 * (quality), as Dark Forest's `get_fxaa_color` does (W-138, no aliasing): it finds the edge through each pixel whose
 * local contrast is high enough, searches along it for its ends, and samples across it by how far the pixel is from
 * the nearer end, plus a sub-pixel blend for isolated bright or dark pixels.
 * @par Example
 * @code
 * dispatched by render_voxel.rs after tone mapping, one thread per pixel.
 * @endcode
 */
kernel void fxaa_voxel(texture2d<float> ldr [[texture(1)]], texture2d<float, access::write> image [[texture(0)]],
                       uint2 id [[thread_position_in_grid]]) {
    if (id.x >= ldr.get_width() || id.y >= ldr.get_height()) return;
    constexpr sampler edge(filter::linear, address::clamp_to_edge);
    float2 texel = 1.0 / float2(ldr.get_width(), ldr.get_height()), uv = (float2(id) + 0.5) * texel;
    float4 centre = ldr.read(id);
    float luma_centre = centre.a;
    float luma_down = ldr.sample(edge, uv + float2(0, 1) * texel).a, luma_up = ldr.sample(edge, uv + float2(0, -1) * texel).a;
    float luma_left = ldr.sample(edge, uv + float2(-1, 0) * texel).a, luma_right = ldr.sample(edge, uv + float2(1, 0) * texel).a;
    float luma_min = min(luma_centre, min(min(luma_down, luma_up), min(luma_left, luma_right)));
    float luma_max = max(luma_centre, max(max(luma_down, luma_up), max(luma_left, luma_right)));
    float luma_range = luma_max - luma_min;
    if (luma_range < max(FXAA_EDGE_THRESHOLD_MIN, luma_max * FXAA_EDGE_THRESHOLD)) {
        image.write(float4(centre.rgb, 1.0), id);
        return;
    }
    float luma_down_left = ldr.sample(edge, uv + float2(-1, 1) * texel).a, luma_up_right = ldr.sample(edge, uv + float2(1, -1) * texel).a;
    float luma_up_left = ldr.sample(edge, uv + float2(-1, -1) * texel).a, luma_down_right = ldr.sample(edge, uv + float2(1, 1) * texel).a;
    float luma_down_up = luma_down + luma_up, luma_left_right = luma_left + luma_right;
    float luma_left_corners = luma_down_left + luma_up_left, luma_down_corners = luma_down_left + luma_down_right;
    float luma_right_corners = luma_down_right + luma_up_right, luma_up_corners = luma_up_right + luma_up_left;
    float edge_horizontal = abs(-2.0 * luma_left + luma_left_corners) + abs(-2.0 * luma_centre + luma_down_up) * 2.0
        + abs(-2.0 * luma_right + luma_right_corners);
    float edge_vertical = abs(-2.0 * luma_up + luma_up_corners) + abs(-2.0 * luma_centre + luma_left_right) * 2.0
        + abs(-2.0 * luma_down + luma_down_corners);
    bool is_horizontal = edge_horizontal >= edge_vertical;
    float luma_1 = is_horizontal ? luma_up : luma_left, luma_2 = is_horizontal ? luma_down : luma_right;
    float gradient_1 = luma_1 - luma_centre, gradient_2 = luma_2 - luma_centre;
    bool is_1_steepest = abs(gradient_1) >= abs(gradient_2);
    float gradient_scaled = 0.25 * max(abs(gradient_1), abs(gradient_2));
    float step_length = (is_horizontal ? texel.y : texel.x) * (is_1_steepest ? -1.0 : 1.0);
    float luma_local_average = 0.5 * ((is_1_steepest ? luma_1 : luma_2) + luma_centre);
    float2 current_uv = uv + (is_horizontal ? float2(0.0, 0.5 * step_length) : float2(0.5 * step_length, 0.0));
    float2 offset = is_horizontal ? float2(texel.x, 0.0) : float2(0.0, texel.y);
    float2 uv_1 = current_uv - offset, uv_2 = current_uv + offset;
    float luma_end_1 = ldr.sample(edge, uv_1).a - luma_local_average, luma_end_2 = ldr.sample(edge, uv_2).a - luma_local_average;
    bool is_reached_1 = abs(luma_end_1) >= gradient_scaled, is_reached_2 = abs(luma_end_2) >= gradient_scaled;
    for (int i = 0; i < FXAA_SEARCH_STEP_COUNT && !(is_reached_1 && is_reached_2); i++) {
        float quality = i < 2 ? 1.0 : (i < 5 ? 1.5 : (i < 8 ? 2.0 : 4.0));
        if (!is_reached_1) {
            uv_1 -= offset * quality;
            luma_end_1 = ldr.sample(edge, uv_1).a - luma_local_average;
            is_reached_1 = abs(luma_end_1) >= gradient_scaled;
        }
        if (!is_reached_2) {
            uv_2 += offset * quality;
            luma_end_2 = ldr.sample(edge, uv_2).a - luma_local_average;
            is_reached_2 = abs(luma_end_2) >= gradient_scaled;
        }
    }
    float distance_1 = is_horizontal ? uv.x - uv_1.x : uv.y - uv_1.y, distance_2 = is_horizontal ? uv_2.x - uv.x : uv_2.y - uv.y;
    bool is_direction_1 = distance_1 < distance_2, is_luma_centre_smaller = luma_centre < luma_local_average;
    bool is_correct_variation = ((is_direction_1 ? luma_end_1 : luma_end_2) < 0.0) != is_luma_centre_smaller;
    float final_offset = is_correct_variation ? 0.5 - min(distance_1, distance_2) / (distance_1 + distance_2) : 0.0;
    float luma_average = (2.0 * (luma_down_up + luma_left_right) + luma_left_corners + luma_right_corners) / 12.0;
    float subpixel_1 = saturate(abs(luma_average - luma_centre) / luma_range);
    float subpixel_2 = (3.0 - 2.0 * subpixel_1) * subpixel_1 * subpixel_1;
    final_offset = max(final_offset, subpixel_2 * subpixel_2 * FXAA_SUBPIXEL_QUALITY);
    float2 final_uv = uv + (is_horizontal ? float2(0.0, final_offset * step_length) : float2(final_offset * step_length, 0.0));
    image.write(float4(ldr.sample(edge, final_uv).rgb, 1.0), id);
}
