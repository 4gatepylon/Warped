// The contents of the cube: a small floating island made of blocks. Everything on it is
// deliberately lopsided (door on one side, chimney at the back right, a right-handed red/green/
// blue axis marker), so that rotated and mirrored copies are easy to tell apart.

export const MATERIALS = [
  { name: 'air', color: [0, 0, 0] },
  { name: 'grass', color: [0.40, 0.65, 0.28] },
  { name: 'dirt', color: [0.55, 0.40, 0.26] },
  { name: 'stone', color: [0.55, 0.55, 0.57] },
  { name: 'planks', color: [0.74, 0.57, 0.34] },
  { name: 'log', color: [0.42, 0.31, 0.19] },
  { name: 'leaves', color: [0.24, 0.52, 0.20] },
  { name: 'glass', color: [0.70, 0.87, 0.96] },
  { name: 'roof', color: [0.64, 0.25, 0.20] },
  { name: 'brick', color: [0.58, 0.30, 0.25] },
  { name: 'white', color: [0.93, 0.93, 0.93] },
  { name: 'red', color: [0.86, 0.18, 0.16] },
  { name: 'green', color: [0.20, 0.75, 0.25] },
  { name: 'blue', color: [0.18, 0.35, 0.90] },
  { name: 'gold', color: [0.95, 0.78, 0.25] },
  { name: 'door', color: [0.36, 0.24, 0.13] },
];
const M = Object.fromEntries(MATERIALS.map((m, i) => [m.name, i]));

export const CUBE_SIZES = [12, 16, 20, 24];
export const DEFAULT_CUBE_SIZE = 20;

export class VoxelGrid {
  constructor(n) {
    this.n = n;
    this.data = new Uint8Array(n * n * n);
  }
  inside(x, y, z) { const n = this.n; return x >= 0 && y >= 0 && z >= 0 && x < n && y < n && z < n; }
  get(x, y, z) { return this.inside(x, y, z) ? this.data[x + this.n * (y + this.n * z)] : 0; }
  set(x, y, z, m) { if (this.inside(x, y, z)) this.data[x + this.n * (y + this.n * z)] = m; }
  /** Smallest box [min, max) containing every solid block (or null if empty). */
  bounds() {
    const n = this.n, min = [n, n, n], max = [0, 0, 0];
    for (let z = 0; z < n; z++) for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
      if (!this.get(x, y, z)) continue;
      const p = [x, y, z];
      for (let k = 0; k < 3; k++) { min[k] = Math.min(min[k], p[k]); max[k] = Math.max(max[k], p[k] + 1); }
    }
    return min[0] < max[0] ? { min, max } : null;
  }
}

/** Where the island's grass surface is, for a cube of n blocks. */
export const islandTop = (n) => Math.max(4, Math.floor(n / 2) - 3);

export function buildScene(n) {
  const g = new VoxelGrid(n);
  const c = Math.floor(n / 2), y0 = islandTop(n);
  const at = (dx, y, dz, m) => g.set(c + dx, y, c + dz, m);

  // Floating island, 8×8 on top, tapering underneath.
  const layers = [['grass', 0], ['dirt', 1], ['stone', 2], ['stone', 3]];
  for (const [mat, inset] of layers) {
    for (let dx = -4 + inset; dx <= 3 - inset; dx++) {
      for (let dz = -4 + inset; dz <= 3 - inset; dz++) at(dx, y0 - inset, dz, M[mat]);
    }
  }

  // Cottage in the back-left quarter, door facing +z, chimney at the back right.
  const base = y0 + 1;
  for (let dx = -4; dx <= 0; dx++) {
    for (let dz = -4; dz <= 0; dz++) {
      const edgeX = dx === -4 || dx === 0, edgeZ = dz === -4 || dz === 0;
      if (!edgeX && !edgeZ) continue;
      for (let h = 0; h < 3; h++) at(dx, base + h, dz, edgeX && edgeZ ? M.log : M.planks);
    }
  }
  at(-2, base, 0, M.door); at(-2, base + 1, 0, M.door);
  at(-4, base + 1, -2, M.glass); at(0, base + 1, -2, M.glass);
  for (let k = 0; k < 3; k++) {          // gable roof, ridge along x at dz = -2
    for (let dx = -5; dx <= 1; dx++) { at(dx, base + 3 + k, -k, M.roof); at(dx, base + 3 + k, -4 + k, M.roof); }
  }
  for (let dz = -3; dz <= -1; dz++) { at(-4, base + 3, dz, M.planks); at(0, base + 3, dz, M.planks); }
  at(-4, base + 4, -2, M.planks); at(0, base + 4, -2, M.planks);
  for (let h = 3; h <= 6; h++) at(-1, base + h, -3, M.brick);

  // Tree at the back right.
  const tx = 2, tz = -2;
  for (let h = 0; h < 4; h++) at(tx, base + h, tz, M.log);
  for (let dx = -1; dx <= 1; dx++) for (let dz = -1; dz <= 1; dz++) {
    for (let h = 3; h <= 4; h++) if (dx || dz || h === 4) at(tx + dx, base + h, tz + dz, M.leaves);
  }
  at(tx, base + 5, tz, M.leaves);

  // Right-handed axis marker at the front right: red +x, green +y, blue +z.
  const ox = 1, oz = 1;
  at(ox, base, oz, M.white);
  for (let k = 1; k <= 3; k++) {
    at(ox + k, base, oz, M.red);
    at(ox, base + k, oz, M.green);
    at(ox, base, oz + k, M.blue);
  }
  at(ox, base + 4, oz, M.gold);
  return g;
}

/**
 * Where the player starts: up and to the front left of the island, looking at the cottage door.
 * yaw is the turn to the right from facing -z (north).
 */
export function spawnPoint(n) {
  const c = n / 2;
  const pos = [c - 0.2 * n, islandTop(n) + 3.5, c + 0.42 * n];
  const target = [c - 1, islandTop(n) + 1.5, c - 1];
  const d = target.map((x, i) => x - pos[i]);
  return { pos, yaw: Math.atan2(d[0], -d[2]), pitch: Math.atan2(d[1], Math.hypot(d[0], d[2])) };
}
