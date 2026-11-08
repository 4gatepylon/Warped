// Smooth-shape versions of index.html's voxel builders, for export_map.mjs. It runs this after index.html's level
// code, so these declarations replace the voxel ones: every model becomes rounded boxes, cylinders and ellipsoids
// in its local chart (meters, y up from the ground), placed on the crater terrain. Shapes resting on the ground
// reach down below the terrain so nothing floats on uneven ground.

const ROUND_M = 0.12;
const PLACED_MODELS = [];
const MOUND_HEIGHT_SCALE = 0.45, MOUND_WIDTH_SCALE = 1.4, MOUND_RADIUS_PER_PEAK_RADIUS = 1.6, RIDGE_MOUND_SPACING_M = 6, RIDGE_MOUND_RADIUS_M = 10, MOUND_SEED = 17;
const BOULDER_MIN_RADIUS_M = 2.5, BOULDER_EXTRA_RADIUS_M = 2.5, BOULDER_MIN_RISE = 0.25, BOULDER_EXTRA_RISE = 0.25;
const PLINTH_MARGIN_M = 0.5;
const STAND_PROBE_TOP_M = 30;
const STAND_PROBE_STEP_M = 0.02;
const TERRAIN_OCTAVES = 5, TERRAIN_RIM_NOISE_M = 60, TERRAIN_RIM_NOISE_FRAC = 0.5, TERRAIN_OUTER_FLANK = 3;
const TERRAIN_WALL_RELIEF_GAIN = 3, TERRAIN_RIM_SEED = 9;
const WINDOW_Y = 2, WALL_ROUND_M = 0.3;
const KAZEMAT_HEIGHT_M = 3.5, GARAGE_HEIGHT_M = 6, FIELD_BUNKER_HEIGHT_M = 3.5, BEACON_HEIGHT_M = 4, HESCO_HEIGHT_M = 1.4;
const HESCO_WIDTH_M = 1.55, HESCO_GAP_M = 3, BEACON_RADIUS_M = 0.45, RED_SPAWN_U_M = 112;
SPAWNS[RED].u_m = RED_SPAWN_U_M;

/** `ShapeModel` collects one structure's shapes, lamps and consoles, like `VoxelModel`. */
class ShapeModel {
  constructor(name) { this.name = name; this.shapes = []; this.lamps = []; this.consoles = []; this.mesh = null; this.parts = []; }
  /** `box` adds a rounded box over the voxel-style inclusive cell range [x0, x1] x [y0, y1] x [z0, z1]. */
  box(x0, y0, z0, x1, y1, z1, material, round_m = ROUND_M) {
    const lo = [x0, y0, z0], hi = [x1 + 1, y1 + 1, z1 + 1];
    this.add('box', lo.map((l, a) => (l + hi[a]) / 2), lo.map((l, a) => (hi[a] - l) / 2), material, round_m);
  }
  /** `cylinder` adds a rounded cylinder along `axis` (0 x, 1 y, 2 z) from its centre, radius and half length. */
  cylinder(centre_m, radius_m, half_length_m, material, axis = 1, round_m = ROUND_M) {
    const half_m = [radius_m, radius_m, radius_m]; half_m[axis] = half_length_m;
    this.add('cylinder', centre_m, half_m, material, round_m, axis);
  }
  ellipsoid(centre_m, half_m, material) { this.add('ellipsoid', centre_m, half_m, material, 0); }
  add(kind, centre_m, half_m, material, round_m, axis = 1) {
    round_m = Math.min(round_m, ...half_m.map(h => h * 0.99));
    this.shapes.push({ kind, centre_m, half_m, round_m, axis, material });
  }
  addLamp(q_m, color, radius_m) { this.lamps.push({ q_m, color, radius_m }); }
}

function hash_terrain(i, j, seed) {
  let h = (Math.imul(i, 0x8da6b343) ^ Math.imul(j, 0xd8163841) ^ Math.imul(seed, 0xcb1ab31f)) >>> 0;
  h = (h ^ (h >>> 13)) >>> 0;
  h = Math.imul(h, 0x5bd1e995) >>> 0;
  h = (h ^ (h >>> 15)) >>> 0;
  return h / 0xffffffff;
}

function smooth_terrain(t) { t = Math.min(1, Math.max(0, t)); return t * t * (3 - 2 * t); }

function sample_terrain_noise(x, y, seed) {
  const i = Math.floor(x), j = Math.floor(y), fx = smooth_terrain(x - i), fy = smooth_terrain(y - j);
  const bottom = hash_terrain(i, j, seed) + (hash_terrain(i + 1, j, seed) - hash_terrain(i, j, seed)) * fx;
  const top = hash_terrain(i, j + 1, seed) + (hash_terrain(i + 1, j + 1, seed) - hash_terrain(i, j + 1, seed)) * fx;
  return bottom + (top - bottom) * fy;
}

/** `get_terrain_height_m` is terrain.rs's `get_height_m` for the map's `TERRAIN`: the ground's height at (u, v). */
function get_terrain_height_m(u_m, v_m) {
  const t = TERRAIN, e = Math.hypot(u_m / t.rim_u_m, v_m / t.rim_v_m), inside_m = (1 - e) * t.rim_v_m;
  const wall = inside_m >= 0 ? smooth_terrain(1 - inside_m / t.wall_m) : 1 - smooth_terrain(-inside_m / (TERRAIN_OUTER_FLANK * t.wall_m));
  const rim_noise = sample_terrain_noise(u_m / TERRAIN_RIM_NOISE_M, v_m / TERRAIN_RIM_NOISE_M, (t.seed ^ TERRAIN_RIM_SEED) >>> 0);
  const rim_m = t.rim_height_m * (1 + TERRAIN_RIM_NOISE_FRAC * (rim_noise - 0.5));
  let sum = 0, amplitude = 0.5, scale = 1;
  for (let octave = 0; octave < TERRAIN_OCTAVES; octave++) {
    sum += amplitude * sample_terrain_noise(u_m / t.relief_wavelength_m * scale, v_m / t.relief_wavelength_m * scale, (t.seed + octave) >>> 0);
    amplitude *= 0.5; scale *= 2;
  }
  const relief = sum / (1 - 0.5 ** TERRAIN_OCTAVES);
  return rim_m * wall + get_mounds_height_m(u_m, v_m) + t.relief_m * relief * (1 + TERRAIN_WALL_RELIEF_GAIN * wall);
}

/** `get_mounds_height_m` is terrain.rs's `get_mounds_height_m`: the summed mounds h (1 - d^2)^2 at (u, v), d = distance / radius. */
function get_mounds_height_m(u_m, v_m) {
  let sum = 0;
  for (const m of TERRAIN.mounds) {
    const d2 = ((u_m - m.u_m) ** 2 + (v_m - m.v_m) ** 2) / m.radius_m ** 2;
    if (d2 < 1) sum += m.height_m * (1 - d2) ** 2;
  }
  return sum;
}

/** `make_mounds` turns the central peak, the ridge and the boulder groups into terrain mounds. */
function make_mounds() {
  const mounds = [{ u_m: 0, v_m: 0, radius_m: CENTRAL_PEAK_RADIUS_M * MOUND_RADIUS_PER_PEAK_RADIUS, height_m: CENTRAL_PEAK_HEIGHT_M }];
  const ridge_count = Math.round(2 * RIDGE_HALF_LENGTH_M / RIDGE_MOUND_SPACING_M) + 1;
  for (let k = 0; k < ridge_count; k++) {
    const u_m = -RIDGE_HALF_LENGTH_M + k * RIDGE_MOUND_SPACING_M, taper = Math.min(1, (RIDGE_HALF_LENGTH_M - Math.abs(u_m)) / RIDGE_END_TAPER_M + 0.3);
    const v_m = RIDGE_V_M + RIDGE_MEANDER_M * Math.sin(u_m / RIDGE_MEANDER_LENGTH_M);
    mounds.push({ u_m, v_m, radius_m: RIDGE_MOUND_RADIUS_M, height_m: RIDGE_CREST_M * taper * RIDGE_MOUND_SPACING_M / (RIDGE_MOUND_RADIUS_M * 16 / 15) });
  }
  const rng = make_rng(MOUND_SEED), groups = [[LAYOUT.blue_boulders, 4, 5], [LAYOUT.crater_boulders, 2, 2], [LAYOUT.flank_boulders, 3, 4]];
  for (const [centres, count, spread_m] of groups) for (const [u0, v0] of centres) for (let i = 0; i < count; i++) {
    const radius_m = BOULDER_MIN_RADIUS_M + rng() * BOULDER_EXTRA_RADIUS_M;
    mounds.push({ u_m: u0 + (rng() * 2 - 1) * spread_m, v_m: v0 + (rng() * 2 - 1) * spread_m, radius_m, height_m: radius_m * (BOULDER_MIN_RISE + rng() * BOULDER_EXTRA_RISE) });
  }
  const flatten = m => ({ ...m, radius_m: m.radius_m * MOUND_WIDTH_SCALE, height_m: m.height_m * MOUND_HEIGHT_SCALE });
  return mounds.map(flatten).map(m => Object.fromEntries(Object.entries(m).map(([k, x]) => [k, Math.round(x * 1000) / 1000])));
}
TERRAIN.mounds = make_mounds();

/** `get_shape_distance_m` bounds the distance from local point q to a shape from below, as shapes.rs does. */
function get_shape_distance_m(shape, q) {
  const p = q.map((c, a) => c - shape.centre_m[a]), h = shape.half_m, r = shape.round_m;
  if (shape.kind === 'box') {
    const d = p.map((c, a) => Math.abs(c) - (h[a] - r));
    return Math.hypot(...d.map(c => Math.max(c, 0))) + Math.min(Math.max(...d), 0) - r;
  }
  if (shape.kind === 'cylinder') {
    const a = shape.axis, b = (a + 1) % 3, c = (a + 2) % 3;
    const d = [Math.hypot(p[b], p[c]) - (h[b] - r), Math.abs(p[a]) - (h[a] - r)];
    return Math.hypot(Math.max(d[0], 0), Math.max(d[1], 0)) + Math.min(Math.max(d[0], d[1]), 0) - r;
  }
  return (Math.hypot(...p.map((c, i) => c / h[i])) - 1) * Math.min(...h);
}

function place_shapes(name, frame, shapes, model_lamps, mesh = null) {
  const R = CURVATURE_RADIUS_M, lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const s of shapes) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a], s.centre_m[a] - s.half_m[a]); hi[a] = Math.max(hi[a], s.centre_m[a] + s.half_m[a]); }
  const centre_local = map_klein(lo.map((l, a) => (l + hi[a]) / (2 * R)));
  let radius = 0;
  for (let c = 0; c < 8; c++) radius = Math.max(radius, measure_distance(map_klein([0, 1, 2].map(a => ((c >> a) & 1 ? hi[a] : lo[a]) / R)), centre_local));
  if (structures.length >= MAX_STRUCTS) throw new Error(`place_shapes: more than ${MAX_STRUCTS} structures`);
  structures.push({ name, frame, inv: invert_lorentz(frame), shapes, mesh, centre: apply_matrix(frame, centre_local), radius: radius + BOUND_MARGIN_M / R });
  for (const lamp of model_lamps) lamps.push({ x: map_local(frame, lamp.q_m), color: lamp.color.map(c => c * LAMP_BRIGHTNESS), radius_m: lamp.radius_m });
  return structures.length - 1;
}

/**
 * `place_model` sets a model on the terrain at (u, v), turned by `yaw_rad`, and each of its parts (`{model, x_m}`)
 * `x_m` along its local x, each as its own structure.
 */
function place_model(model, u_m, v_m, yaw_rad) {
  const base = make_frame(u_m, v_m, yaw_rad);
  for (const part of model.parts) place_model_at(part.model, multiply_matrices(base, make_boost(X, part.x_m / CURVATURE_RADIUS_M)));
  return place_model_at(model, base);
}

/**
 * `place_model_at` sets a model on the terrain at frame `base`: lifted to the highest ground under it (a model drawn
 * as a mesh to the lowest, so it sinks in rather than floats), its ground shapes extended down to the lowest.
 */
function place_model_at(model, base) {
  if (!model.shapes.length) return -1;
  const R = CURVATURE_RADIUS_M, heights = [], { u_m, v_m } = get_fermi_coordinates(get_column(base, W));
  for (const s of model.shapes) for (const sx of [-1, 0, 1]) for (const sz of [-1, 0, 1]) {
    const q = [s.centre_m[0] + sx * s.half_m[0], 0, s.centre_m[2] + sz * s.half_m[2]], g = get_fermi_coordinates(map_local(base, q));
    heights.push(get_terrain_height_m(g.u_m, g.v_m));
  }
  const top_m = model.mesh ? Math.min(...heights) : Math.max(...heights), drop_m = top_m - Math.min(...heights) + PLINTH_MARGIN_M;
  const shapes = model.shapes.map(s => {
    if (s.centre_m[1] - s.half_m[1] > 0.01) return s;
    const bottom_m = Math.min(s.centre_m[1] - s.half_m[1], -drop_m), top = s.centre_m[1] + s.half_m[1];
    if (s.kind === 'ellipsoid') return { ...s, centre_m: [s.centre_m[0], s.centre_m[1] - drop_m / 2, s.centre_m[2]], half_m: [s.half_m[0], s.half_m[1] + drop_m / 2, s.half_m[2]] };
    return { ...s, centre_m: [s.centre_m[0], (bottom_m + top) / 2, s.centre_m[2]], half_m: [s.half_m[0], (top - bottom_m) / 2, s.half_m[2]] };
  });
  let reach_m = 0;
  for (const s of model.shapes) reach_m = Math.max(reach_m, Math.hypot(Math.abs(s.centre_m[0]) + s.half_m[0], Math.abs(s.centre_m[2]) + s.half_m[2]));
  PLACED_MODELS.push({ u_m, v_m, reach_m, top_m });
  return place_shapes(model.name, multiply_matrices(base, make_boost(Y, top_m / R)), shapes, model.lamps, model.mesh);
}

/** `get_ground_lift_m` gives the height a point's ground is raised to: the top of a placed model under it, or else the terrain. */
function get_ground_lift_m(u_m, v_m) {
  const under = PLACED_MODELS.filter(m => Math.hypot(u_m - m.u_m, v_m - m.v_m) < m.reach_m).map(m => m.top_m);
  return under.length ? Math.max(...under) : get_terrain_height_m(u_m, v_m);
}

/** `add_warp` registers a warp object as index.html's does, raised with the ground under it (terrain or a placed model). */
function add_warp(warp) {
  if (warps.length >= MAX_WARPS) throw new Error(`add_warp: more than ${MAX_WARPS} warp objects`);
  const g = get_fermi_coordinates(get_column(warp.frame, W));
  const frame = multiply_matrices(warp.frame, make_boost(Y, get_ground_lift_m(g.u_m, g.v_m) / CURVATURE_RADIUS_M));
  warps.push({ ...warp, frame, centre: get_column(frame, W), inv: invert_lorentz(frame), list: [] });
  return warps.length - 1;
}

/** `add_terrain_chunk` drops the voxel rim, central peak and ridge: the terrain has them (`make_mounds`). */
function add_terrain_chunk() { return -1; }

/** `find_standing_height` gives the eye height of a body frame standing on the terrain or the shapes above it. */
function find_standing_height(B) {
  const R = CURVATURE_RADIUS_M, g = get_fermi_coordinates(get_column(B, W)), ground_m = get_terrain_height_m(g.u_m, g.v_m);
  for (let h_m = ground_m + STAND_PROBE_TOP_M; h_m > ground_m; h_m -= STAND_PROBE_STEP_M) {
    const x = get_column(multiply_matrices(B, make_boost(Y, h_m / R)), W);
    for (const st of structures) {
      if (measure_distance(x, st.centre) > st.radius) continue;
      const local = apply_matrix(st.inv, x), q = [0, 1, 2].map(a => local[a] / local[3] * R);
      if (st.shapes.some(s => get_shape_distance_m(s, q) < 0)) return h_m + EYE_HEIGHT_M;
    }
  }
  return ground_m + EYE_HEIGHT_M;
}

/**
 * `make_mesh_model` gives a model drawn as the map model `mesh_model`, `height_m` tall, whose players are stopped by
 * boxes (each its centre and half sizes in meters).
 */
function make_mesh_model(name, mesh_model, height_m, boxes, material) {
  const model = new ShapeModel(name);
  model.mesh = { model: mesh_model, height_m };
  for (const [centre_m, half_m] of boxes) model.add('box', centre_m, half_m, material, 0);
  return model;
}

function build_lander() {
  const model = make_mesh_model('blue kazemat', 'stekelvarken_kazemat', KAZEMAT_HEIGHT_M, [[[0, 1.5, 0], [3, 1.5, 2]]], MAT.HULL);
  model.addLamp([0, 6, -4], TEAM_COLORS[BLUE], 14);
  return model;
}

function build_pylon(team) {
  const t = TEAMS[team], model = new ShapeModel(`${t.name} pylon consoles`);
  for (let i = 0; i < CONSOLES_PER_PYLON; i++) {
    const angle_rad = Math.PI / 2 + i * 2 * Math.PI / CONSOLES_PER_PYLON;
    const x = Math.floor(PYLON_CONSOLE_RADIUS_M * Math.cos(angle_rad)), z = Math.floor(PYLON_CONSOLE_RADIUS_M * Math.sin(angle_rad));
    model.cylinder([x + 0.5, 0.5, z + 0.5], 0.35, 0.5, MAT.METAL);
    model.box(x, 1, z, x, 1, z, t.console + i, 0.25);
    model.consoles.push([x + 0.5, 1.5, z + 0.5]);
  }
  const beacon = make_mesh_model(`${t.name} pylon`, 'scifi_beacon', BEACON_HEIGHT_M, [], MAT.METAL);
  beacon.cylinder([0, BEACON_HEIGHT_M / 2, 0], BEACON_RADIUS_M, BEACON_HEIGHT_M / 2, t.paint);
  beacon.addLamp([0, PYLON_CORE_HEIGHT_M, 0], t.color, 10);
  model.parts.push({ model: beacon, x_m: 0 });
  return model;
}

/**
 * `add_wall_around_windows` adds a hull wall one cell thick with a hole in row `WINDOW_Y` at each window, for a glass
 * pane to fill: a piece below the row, a piece above it, and a pillar between each pair of holes that reaches one
 * cell into both pieces, so no seam shows.
 * @param {ShapeModel} model The model to add the wall to.
 * @param {number} x The wall's cell column.
 * @param {number} top_y The wall's top cell row.
 * @param {number} z0 The wall's first cell along z.
 * @param {number} z1 Its last cell along z (included).
 * @param {function(number): boolean} is_window Whether cell z of row `WINDOW_Y` is a window.
 * @example add_wall_around_windows(model, -3, 4, -6, 5, z => z % 3 === 0);
 */
function add_wall_around_windows(model, x, top_y, z0, z1, is_window) {
  model.box(x, 0, z0, x, WINDOW_Y - 1, z1, MAT.HULL, WALL_ROUND_M);
  model.box(x, WINDOW_Y + 1, z0, x, top_y, z1, MAT.HULL, WALL_ROUND_M);
  for (let z = z0; z <= z1; z++) {
    if (is_window(z) || (z > z0 && !is_window(z - 1))) continue;
    let z_end_incl = z;
    while (z_end_incl < z1 && !is_window(z_end_incl + 1)) z_end_incl++;
    model.box(x, WINDOW_Y - 1, z, x, WINDOW_Y + 1, z_end_incl, MAT.HULL, WALL_ROUND_M);
  }
}

function build_habitat_shapes(name, length_m, stripe, light) {
  const model = new ShapeModel(name), z0 = -Math.floor(length_m / 2), z1 = z0 + length_m - 1, window_period = 3;
  const is_window = z => z > z0 && z < z1 && (z - z0) % window_period === 1;
  model.box(-3, 0, z0, 2, 0, z1, MAT.FLOOR, 0.1);
  for (const x of [-3, 2]) {
    add_wall_around_windows(model, x, 4, z0, z1, is_window);
    model.box(x, 1, z0 + 1, x, 1, z1 - 1, stripe, 0.1); model.shapes.at(-1).half_m[0] = 0.55;
    for (let z = z0 + 1; z < z1; z++) if (is_window(z)) { model.box(x, WINDOW_Y, z, x, WINDOW_Y, z, MAT.GLASS, 0.15); model.shapes.at(-1).half_m[0] = 0.56; }
  }
  model.box(-3, 4, z0, 2, 4, z1, MAT.HULL, 0.4);
  for (let z = z0 + 1; z < z1; z += 2) { model.box(-2, 5, z, 1, 5, z, MAT.SOLAR, 0.05); model.shapes.at(-1).half_m[1] = 0.08; model.shapes.at(-1).centre_m[1] = 5.05; }
  for (const z of [z0, z1]) {
    model.box(-3, 0, z, -2, 4, z, MAT.HULL, 0.2); model.box(1, 0, z, 2, 4, z, MAT.HULL, 0.2); model.box(-1, 3, z, 0, 4, z, MAT.HULL, 0.2);
    for (const x of [-3, 2]) model.ellipsoid([x + 0.5, 3.5, z + (z === z0 ? 0 : 1)], [0.3, 0.3, 0.15], light);
  }
  return model;
}

function build_habitat(name, length_m, stripe, light) {
  if (name !== 'derelict habitat module') return build_habitat_shapes(name, length_m, stripe, light);
  return make_mesh_model('derelict field bunker', 'ww2_field_bunker', FIELD_BUNKER_HEIGHT_M, [[[0, 1.75, 2.45], [3.6, 1.75, 3.75]], [[-0.65, 1.1, -3.75], [2.95, 1.1, 2.45]]], MAT.HULL);
}

function build_dome(name, radius_m, light) {
  const model = new ShapeModel(name), r = radius_m, glass_frac = 0.62, light_count = 6;
  model.cylinder([0, glass_frac * r / 2, 0], r, glass_frac * r / 2, MAT.HULL, 1, 0.5);
  model.ellipsoid([0, 0, 0], [r - 0.15, r, r - 0.15], MAT.GLASS);
  model.cylinder([0, glass_frac * r + 0.3, 0], 1, 0.3, light);
  for (let i = 0; i < light_count; i++) {
    const a = i * 2 * Math.PI / light_count;
    model.ellipsoid([r * Math.cos(a), 1, r * Math.sin(a)], [0.3, 0.3, 0.3], light);
  }
  for (const s of [-1, 1]) model.box(-1, 0, s > 0 ? r - 1 : -r - 1, 0, 2, s > 0 ? r : -r, MAT.HULL, 0.3);
  return model;
}

function build_antenna(name, height_m, light) {
  const model = new ShapeModel(name), dish_r = 2.5;
  model.box(-1, 0, -1, 1, 0, 1, MAT.METAL, 0.2);
  model.cylinder([0.5, (height_m + 2) / 2, 0.5], 0.22, height_m / 2, MAT.METAL);
  model.cylinder([0.5, height_m - 1.5, 0.5], 0.12, dish_r, MAT.METAL, 0);
  model.cylinder([0.5, height_m + 1.6, 0.5], dish_r, 0.25, MAT.HULL, 1, 0.2);
  model.ellipsoid([0.5, height_m + 3.4, 0.5], [0.35, 0.35, 0.35], light);
  return model;
}

function build_walkway(name, length_m) {
  const model = new ShapeModel(name), z0 = -Math.floor(length_m / 2), z1 = z0 + length_m - 1;
  const is_window = z => (z - z0) % 2 === 1;
  model.box(-1, 0, z0, 1, 0, z1, MAT.METAL, 0.1);
  for (const x of [-2, 2]) add_wall_around_windows(model, x, 3, z0, z1, is_window);
  model.box(-2, 3, z0, 2, 3, z1, MAT.HULL, 0.4);
  for (let z = z0; z <= z1; z++) for (const x of [-2, 2]) if (is_window(z)) { model.box(x, WINDOW_Y, z, x, WINDOW_Y, z, MAT.GLASS, 0.15); model.shapes.at(-1).half_m[0] = 0.56; }
  return model;
}

function build_landing_pad(name, team) {
  const t = TEAMS[team], model = new ShapeModel(name), half = 7;
  model.box(-half, 0, -half, half - 1, 0, half - 1, MAT.HAZARD, 0.1);
  model.box(-half + 1, 0, -half + 1, half - 2, 0, half - 2, MAT.PAD, 0.1); model.shapes.at(-1).half_m[1] += 0.02;
  model.cylinder([0, 0.53, 0], 4.5, 0.5, t.paint, 1, 0.05);
  model.cylinder([0, 0.55, 0], 3.5, 0.5, MAT.PAD, 1, 0.05);
  for (const [x, z] of [[-half, -half], [half - 1, -half], [-half, half - 1], [half - 1, half - 1]]) {
    model.cylinder([x + 0.5, 2, z + 0.5], 0.25, 1, MAT.METAL);
    model.ellipsoid([x + 0.5, 3.4, z + 0.5], [0.35, 0.35, 0.35], t.light);
  }
  model.box(3, 1, 4, 4, 2, 5, MAT.METAL, 0.3);
  model.box(-5, 1, 4, -5, 1, 4, MAT.HAZARD, 0.3);
  model.addLamp([0, 3, 0], t.color, 9);
  return model;
}

function build_watchtower(name, team) {
  const t = TEAMS[team], model = new ShapeModel(name), top = 6;
  for (const [x, z] of [[-2, -2], [2, -2], [-2, 2], [2, 2]]) model.cylinder([x + 0.5, (top + 1) / 2, z + 0.5], 0.3, (top + 1) / 2, MAT.METAL);
  model.box(-2, top, -2, 2, top, 2, MAT.METAL, 0.2);
  model.box(-2, top + 1, -2, 2, top + 1, -2, MAT.HULL, 0.15); model.box(-2, top + 1, 2, 2, top + 1, 2, MAT.HULL, 0.15);
  model.box(-2, top + 1, -2, -2, top + 1, 2, MAT.HULL, 0.15); model.box(2, top + 1, -2, 2, top + 1, 0, MAT.HULL, 0.15);
  for (const [x, z] of [[-2, -2], [2, -2], [-2, 2], [2, 2]]) model.ellipsoid([x + 0.5, top + 2.3, z + 0.5], [0.3, 0.3, 0.3], t.light);
  for (let k = 0; k <= top; k++) model.box(3, k, k - 4, 3, k, k - 4, MAT.METAL, 0.1);
  model.addLamp([0, top + 2, 0], t.color, 9);
  return model;
}

function build_hangar(name, team) {
  const model = make_mesh_model(name, 'desert_bunker_garage', GARAGE_HEIGHT_M, [[[0, 1.6, 0], [9.1, 1.6, 4.6]], [[0, 4, 0], [9.1, 0.8, 3.2]]], MAT.HULL);
  model.addLamp([0, 3, -8], TEAMS[team].color, 12);
  return model;
}

function build_barrier(name, length_m, team) {
  const model = new ShapeModel(name), side_count = Math.floor((length_m - HESCO_GAP_M) / 2 / HESCO_WIDTH_M);
  for (const sign of [-1, 1]) for (let k = 0; k < side_count; k++) {
    const block = make_mesh_model('hesco block', 'hesco_barrier', HESCO_HEIGHT_M, [[[0, HESCO_HEIGHT_M / 2, 0], [HESCO_WIDTH_M / 2, HESCO_HEIGHT_M / 2, HESCO_WIDTH_M / 2]]], MAT.METAL);
    model.parts.push({ model: block, x_m: sign * (HESCO_GAP_M / 2 + (k + 0.5) * HESCO_WIDTH_M) });
  }
  return model;
}

function build_crates(name, rng, count) {
  const model = new ShapeModel(name), spread = 6, kinds = [MAT.METAL, MAT.HAZARD, MAT.HULL], heights = new Map();
  for (let i = 0; i < count; i++) {
    const size = rng() < 0.4 ? 2 : 1, x = Math.floor(rng() * spread) - spread / 2, z = Math.floor(rng() * spread) - spread / 2;
    let y = 0;
    for (let a = x; a < x + size; a++) for (let b = z; b < z + size; b++) y = Math.max(y, heights.get(`${a},${b}`) || 0);
    for (let a = x; a < x + size; a++) for (let b = z; b < z + size; b++) heights.set(`${a},${b}`, y + size);
    model.box(x, y, z, x + size - 1, y + size - 1, z + size - 1, kinds[Math.floor(rng() * kinds.length)], 0.1);
  }
  return model;
}

function build_solar_array(name, rows, length_m) {
  const model = new ShapeModel(name), row_pitch = 4, post_period = 4, x0 = -Math.floor(length_m / 2), x1 = length_m - Math.floor(length_m / 2) - 1;
  for (let r = 0; r < rows; r++) {
    const z = r * row_pitch - Math.floor(rows * row_pitch / 2);
    model.box(x0, 1, z, x1, 1, z + 1, MAT.SOLAR, 0.05); model.shapes.at(-1).half_m[1] = 0.1; model.shapes.at(-1).centre_m[1] = 1.4;
    for (let x = x0; x <= x1; x++) if (((x % post_period) + post_period) % post_period === 0) model.cylinder([x + 0.5, 0.65, z + 1], 0.12, 0.65, MAT.METAL);
  }
  return model;
}

/** `build_boulders` builds nothing: boulders are terrain mounds (`make_mounds`). */
function build_boulders(name) { return new ShapeModel(name); }
