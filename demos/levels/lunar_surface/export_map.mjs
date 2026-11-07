/**
 * `export_map.mjs` exports the Lunar Surface level to the portable map format of `common/maps/` (see its README): it
 * runs the level's own layout code from `index.html` (everything before the WebGL set-up, plus the functions that
 * find where a player stands) in a Node VM, then writes `map.json` and `voxels.bin` into
 * `common/maps/lunar_surface/`, replacing what is there. The map files are generated: rerun this after changing the
 * level's layout.
 *
 * Interface: `node demos/levels/lunar_surface/export_map.mjs [--output-folder <path>]`.
 *
 * @example node demos/levels/lunar_surface/export_map.mjs
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const LEVEL_FOLDER = path.dirname(fileURLToPath(import.meta.url));
const REPOSITORY_FOLDER = path.resolve(LEVEL_FOLDER, '../../..');
const DEFAULT_OUTPUT_FOLDER = path.join(REPOSITORY_FOLDER, 'common/maps/lunar_surface');
const LAYOUT_START = 'const CURVATURE_RADIUS_M';
const LAYOUT_END = "const canvas = document.getElementById('canvas');";
const STANDING_START = 'function find_nearby_structures';
const STANDING_END = 'function move_optically';
const CELL_M = 1;
const SUN_COLOR = [1.0, 0.96, 0.88];
const LIGHTING = { model: 'lunar_lambert', lommel_seeliger_frac: 0.7, ambient: 0.11, earthshine: 0.08, sun_intensity: 1.45, shadows: true };
const SPACE_COLOR = [0, 0, 0];
const TEAM_NAMES = ['blue', 'red'];

/**
 * `cut_between` returns the text of `source` from the start of `start` to the start of `end`.
 * @param {string} source The text.
 * @param {string} start Where the piece starts.
 * @param {string} end Where it stops (not included).
 * @return {string} The piece.
 * @example const code = cut_between(html, 'const CURVATURE_RADIUS_M', "const canvas");
 */
function cut_between(source, start, end) {
  const i = source.indexOf(start), j = source.indexOf(end, i);
  if (i < 0 || j < 0) throw new Error(`cut_between: index.html no longer has ${JSON.stringify(i < 0 ? start : end)}`);
  return source.slice(i, j);
}

/**
 * `run_level` runs the level's layout code and returns what the map needs from it.
 * @param {string} html The level's index.html.
 * @return {Object} The level's structures, warps, lamps, constants and helpers.
 * @example const level = run_level(readFileSync('index.html', 'utf8'));
 */
function run_level(html) {
  const code = cut_between(html, LAYOUT_START, LAYOUT_END) + cut_between(html, STANDING_START, STANDING_END);
  const exported = `;({ structures, warps, lamps, MATERIALS, MAT, SPAWNS, CURVATURE_RADIUS_M, GRAVITY_M_PER_S_PER_S, EYE_HEIGHT_M,
    WALK_SPEED_M_PER_S, SPRINT_SPEED_M_PER_S, JUMP_SPEED_M_PER_S, FLY_SPEED_M_PER_S, BLACK_HOLE_RS_M, BLACK_HOLE_REACH_M,
    WORMHOLE_RS_M, WORMHOLE_REACH_M, FOIL_LAMBDA_SQ, TILE_CELLS_PER_HALF, SUN_ELEVATION_RAD, SUN_AZIMUTH_RAD,
    PLANET_ELEVATION_RAD, PLANET_AZIMUTH_RAD, PLANET_RADIUS_RAD, ZONE_EDGE_U_M, BOUNDARY_U_M, BOUNDARY_V_M, KIND_MOUTH,
    make_frame, make_boost, multiply_matrices, get_facing_yaw, find_standing_height, Y })`;
  return vm.runInNewContext(code + exported, {}, { filename: 'lunar_surface/index.html' });
}

/**
 * `make_sky_direction` gives a unit direction at the level's centre from an elevation and an azimuth (from +u toward
 * +v), as the level's `make_sky_direction` does.
 * @param {number} elevation_rad Angle above the ground.
 * @param {number} azimuth_rad Angle from +u toward +v.
 * @return {Array<number>} The direction (x is +v, z is -u).
 * @example const sun = make_sky_direction(0.3, -2.1);
 */
function make_sky_direction(elevation_rad, azimuth_rad) {
  return [Math.cos(elevation_rad) * Math.sin(azimuth_rad), Math.sin(elevation_rad), -Math.cos(elevation_rad) * Math.cos(azimuth_rad)];
}

/**
 * `make_map` turns the level into a map manifest and its voxel bytes.
 * @param {Object} level What `run_level` returned.
 * @return {{manifest: Object, voxels: Uint8Array}} The map.
 * @example const { manifest, voxels } = make_map(level);
 */
function make_map(level) {
  const R = level.CURVATURE_RADIUS_M;
  const materials = level.MATERIALS.map(([name, color, glow], id) => ({
    id, name, color, kind: id === 0 ? 'empty' : id === level.MAT.GLASS ? 'glass' : glow > 0 ? 'light' : 'solid'
  }));
  let offset_bytes = 0;
  const structures = level.structures.map(structure => {
    const voxels = { size: structure.dims, cell_m: CELL_M, origin: structure.off, offset_bytes };
    offset_bytes += structure.cells.length;
    return { name: structure.name, frame: Array.from(structure.frame), voxels };
  });
  const voxels = new Uint8Array(offset_bytes);
  level.structures.reduce((at, structure) => { voxels.set(structure.cells, at); return at + structure.cells.length; }, 0);
  const warps = level.warps.map(warp => {
    const is_mouth = warp.kind === level.KIND_MOUTH;
    return {
      kind: is_mouth ? 'wormhole_mouth' : 'black_hole', name: warp.name, frame: Array.from(warp.frame),
      law: is_mouth ? 'foil' : 'isotropic_schwarzschild',
      schwarzschild_radius_m: is_mouth ? level.WORMHOLE_RS_M : level.BLACK_HOLE_RS_M,
      ...(is_mouth ? { lambda_squared: level.FOIL_LAMBDA_SQ } : {}),
      radial_coordinate: 'distance', reach_m: is_mouth ? level.WORMHOLE_REACH_M : level.BLACK_HOLE_REACH_M, falloff: 'taper',
      ...(is_mouth ? { partner: warp.partner } : {})
    };
  });
  const spawns = level.SPAWNS.map((spawn, team) => {
    const ground = level.make_frame(spawn.u_m, spawn.v_m, level.get_facing_yaw(spawn.u_m, spawn.v_m, 0, 0));
    const feet_m = level.find_standing_height(ground) - level.EYE_HEIGHT_M;
    return { team: TEAM_NAMES[team], frame: Array.from(level.multiply_matrices(ground, level.make_boost(level.Y, feet_m / R))) };
  });
  const manifest = {
    format: 'warped-map', version: 1, name: 'lunar_surface',
    description: 'A lunar base on a regolith plain in H³: a crater with a black hole over it, a habitat row, a ridge with a wormhole to the red landing pad.',
    source: 'demos/levels/lunar_surface/index.html',
    space: { kind: 'hyperbolic', curvature_radius_m: R },
    gravity_m_per_s2: level.GRAVITY_M_PER_S_PER_S,
    player: {
      eye_height_m: level.EYE_HEIGHT_M, walk_m_per_s: level.WALK_SPEED_M_PER_S, sprint_m_per_s: level.SPRINT_SPEED_M_PER_S,
      jump_m_per_s: level.JUMP_SPEED_M_PER_S, fly_m_per_s: level.FLY_SPEED_M_PER_S
    },
    materials,
    ground: { material: level.MAT.REGOLITH, tiling: { kind: 'square_order_5', cells_per_side: 2 * level.TILE_CELLS_PER_HALF } },
    structures, warps,
    lights: level.lamps.map(lamp => ({ position: Array.from(lamp.x), color: lamp.color, radius_m: lamp.radius_m })),
    sky: {
      transport: 'parallel', sun: { direction: make_sky_direction(level.SUN_ELEVATION_RAD, level.SUN_AZIMUTH_RAD), color: SUN_COLOR },
      planet: { direction: make_sky_direction(level.PLANET_ELEVATION_RAD, level.PLANET_AZIMUTH_RAD), angular_radius_rad: level.PLANET_RADIUS_RAD },
      stars: true, clouds: false, zenith_color: SPACE_COLOR, horizon_color: SPACE_COLOR
    },
    spawns,
    regions: {
      zones: [
        { team: 'blue', u_min_m: null, u_max_m: -level.ZONE_EDGE_U_M },
        { team: 'middle', u_min_m: -level.ZONE_EDGE_U_M, u_max_m: level.ZONE_EDGE_U_M },
        { team: 'red', u_min_m: level.ZONE_EDGE_U_M, u_max_m: null }
      ],
      boundary: { kind: 'fermi_ellipse', u_m: level.BOUNDARY_U_M, v_m: level.BOUNDARY_V_M }
    },
    appearance: { style: 'lunar', lighting: LIGHTING }
  };
  return { manifest, voxels };
}

/**
 * `main` reads the command line, exports the level and says where it wrote the map.
 * @example main();
 */
function main() {
  const args = process.argv.slice(2);
  const is_flag = args[0] === '--output-folder';
  if (args.length !== 0 && !(is_flag && args.length === 2)) {
    console.error('Usage: node demos/levels/lunar_surface/export_map.mjs [--output-folder <path>]');
    process.exit(2);
  }
  const output_folder = is_flag ? path.resolve(args[1]) : DEFAULT_OUTPUT_FOLDER;
  const { manifest, voxels } = make_map(run_level(readFileSync(path.join(LEVEL_FOLDER, 'index.html'), 'utf8')));
  mkdirSync(output_folder, { recursive: true });
  writeFileSync(path.join(output_folder, 'map.json'), JSON.stringify(manifest, null, 1) + '\n');
  writeFileSync(path.join(output_folder, 'voxels.bin'), voxels);
  console.log(`Wrote ${manifest.structures.length} structures (${voxels.length} voxel bytes), ${manifest.warps.length} warps and ${manifest.lights.length} lights to ${output_folder}`);
}

main();
