// Colours shared by the menu preview and the world, so a face or edge looks the same in both.

import { FACE_COUNT } from './math/cube.js';

const PAIR_COLORS = [
  [0.93, 0.42, 0.38], // coral
  [0.35, 0.62, 0.95], // blue
  [0.98, 0.78, 0.30], // amber
  [0.55, 0.82, 0.45], // green
  [0.78, 0.52, 0.92], // violet
  [0.35, 0.82, 0.80], // teal
];
export const WALL_COLOR = [0.62, 0.62, 0.65];
export const MIRROR_COLOR = [0.80, 0.86, 0.94];

const EDGE_COLORS = [
  [1.0, 0.36, 0.36], [1.0, 0.82, 0.24], [0.30, 0.86, 0.45], [0.28, 0.60, 1.0],
  [0.80, 0.45, 1.0], [1.0, 0.58, 0.18], [0.25, 0.92, 0.90], [1.0, 0.45, 0.78],
  [0.70, 0.90, 0.30], [0.55, 0.55, 1.0], [0.95, 0.95, 0.95], [0.60, 0.40, 0.25],
];

/** One colour per face: glued faces share their pair's colour. */
export function faceColors(faces) {
  const out = new Array(FACE_COUNT);
  let next = 0;
  faces.forEach((f, i) => {
    if (f.kind === 'wall') out[i] = WALL_COLOR;
    else if (f.kind === 'mirror') out[i] = MIRROR_COLOR;
    else if (f.partner >= i) out[i] = PAIR_COLORS[next++ % PAIR_COLORS.length];
  });
  faces.forEach((f, i) => { if (f.kind === 'glue' && f.partner < i) out[i] = out[f.partner]; });
  return out;
}

/** Colour of a class of cube edges; `end` 0 or 1 shades the two halves (like an arrow), 0.5 for neither. */
export function edgeColor(cls, end) {
  const c = EDGE_COLORS[cls % EDGE_COLORS.length];
  const k = end === 0 ? 0.55 : end === 1 ? 1.0 : 0.8;
  return c.map((x) => x * k);
}

export const toCss = (rgb, k = 1) => `rgb(${rgb.map((x) => Math.round(Math.min(1, x * k) * 255)).join(',')})`;
