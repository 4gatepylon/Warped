// Curved (and flat) space for the cube: the three geometries of constant curvature.
//
// Points are written in homogeneous coordinates X = (x, y, z, w):
//   flat        X = (p, 1), lengths in blocks. Motions are rotations + translations.
//   spherical   X on the unit 3-sphere x²+y²+z²+w² = 1 (the 3-sphere demo's model). Motions are O(4).
//   hyperbolic  X on the hyperboloid x²+y²+z²−w² = −1, w > 0 (the hyperbolic demo's model). Motions are O(3,1).
// R is the curvature radius in blocks; a model distance of 1 is R blocks.
//
// The key fact: in the "projective chart" p = (x, y, z) / w (gnomonic for the sphere, Klein for
// hyperbolic space) every straight line of the curved space is a straight line, and every flat
// (totally geodesic) surface is a plane. So the curved cube is still the ordinary cube [−a, a]³ in
// this chart, and light still travels along straight lines through the voxel grid. Only the
// face-to-face jumps (now 4x4 projective maps) and the distances along a ray change.
//
// How curved the cube is shows up in its corner angle (the angle between two faces along an
// edge): 90° flat, more than 90° spherical, less than 90° hyperbolic. In the chart:
//   spherical   cos θ = −a²/(1 + a²)        hyperbolic   cos θ = a²/(1 − a²)
// The cube's true half-width (centre to face) is kept at n/2 blocks, which fixes R.

import { faceAxis, faceSign } from './cube.js';
import { GLUE, MIRROR } from './gluing.js';

export const FLAT = 'flat';
export const SPHERICAL = 'spherical';
export const HYPERBOLIC = 'hyperbolic';

export const MIN_CORNER = 62;   // at 60° a hyperbolic cube's corners are infinitely far away
export const MAX_CORNER = 150;  // at 180° a spherical cube fills half of the 3-sphere

/** Geometry of a cube n blocks across whose corners have the given angle (degrees). */
export function makeGeometry(cornerDeg, n) {
  const corner = Math.min(MAX_CORNER, Math.max(MIN_CORNER, cornerDeg));
  if (Math.abs(corner - 90) < 1e-9) return { kind: FLAT, corner: 90, n, a: n / 2, R: 1, sign: 0 };
  const c = Math.cos((corner * Math.PI) / 180);
  const a = Math.sqrt(Math.abs(c) / (1 + c));
  return corner > 90
    ? { kind: SPHERICAL, corner, n, a, R: n / 2 / Math.atan(a), sign: 1 }
    : { kind: HYPERBOLIC, corner, n, a, R: n / 2 / Math.atanh(a), sign: -1 };
}

export const describeGeometry = (g) => (g.kind === FLAT ? 'flat' : `${g.kind}, curvature radius ${g.R.toFixed(1)} blocks`);

// ---------------------------------------------------------------------------
// 4x4 matrices (row-major flat arrays of 16) and 4-vectors
// ---------------------------------------------------------------------------
export const I4 = Object.freeze([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

export function mul4(a, b) {
  const o = new Array(16).fill(0);
  for (let r = 0; r < 4; r++) for (let c = 0; c < 4; c++) {
    let s = 0;
    for (let k = 0; k < 4; k++) s += a[r * 4 + k] * b[k * 4 + c];
    o[r * 4 + c] = s;
  }
  return o;
}

export function apply4(m, v) {
  return [0, 1, 2, 3].map((r) => m[r * 4] * v[0] + m[r * 4 + 1] * v[1] + m[r * 4 + 2] * v[2] + m[r * 4 + 3] * v[3]);
}

export const col4 = (m, c) => [m[c], m[4 + c], m[8 + c], m[12 + c]];

export function fromCols4(cols) {
  const m = new Array(16);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) m[r * 4 + c] = cols[c][r];
  return m;
}

export function det4(m) {
  const at = (r, c) => m[r * 4 + c];
  let d = 0;
  for (let c = 0; c < 4; c++) {
    const s = [];
    for (let r = 1; r < 4; r++) for (let cc = 0; cc < 4; cc++) if (cc !== c) s.push(at(r, cc));
    const m3 = s[0] * (s[4] * s[8] - s[5] * s[7]) - s[1] * (s[3] * s[8] - s[5] * s[6]) + s[2] * (s[3] * s[7] - s[4] * s[6]);
    d += (c % 2 ? -1 : 1) * at(0, c) * m3;
  }
  return d;
}

/** Embed a 3x3 linear map (row-major 9) as a motion fixing the cube's centre. */
export function embed3(m) {
  return [m[0], m[1], m[2], 0, m[3], m[4], m[5], 0, m[6], m[7], m[8], 0, 0, 0, 0, 1];
}

/** Rotation by `angle` in the coordinate plane (i, j), i, j < 3: e_i turns towards e_j. */
export function rotation4(i, j, angle) {
  const m = I4.slice(), c = Math.cos(angle), s = Math.sin(angle);
  m[i * 4 + i] = c; m[j * 4 + j] = c; m[j * 4 + i] = s; m[i * 4 + j] = -s;
  return m;
}

// The bilinear form of the model: dot product for the sphere, Minkowski (+ + + −) for
// hyperbolic space. (Flat space is handled separately where it differs.)
export function form(g, u, v) {
  return u[0] * v[0] + u[1] * v[1] + u[2] * v[2] + (g.sign < 0 ? -1 : 1) * u[3] * v[3];
}

// ---------------------------------------------------------------------------
// Points: projective chart, voxel grid, model
// ---------------------------------------------------------------------------

/** Model point (normalised) for a chart point p. */
export function chartToModel(g, p) {
  const r2 = p[0] * p[0] + p[1] * p[1] + p[2] * p[2];
  if (g.kind === FLAT) return [p[0], p[1], p[2], 1];
  const s = 1 / Math.sqrt(g.kind === SPHERICAL ? 1 + r2 : 1 - r2);
  return [p[0] * s, p[1] * s, p[2] * s, s];
}

export const modelToChart = (X) => [X[0] / X[3], X[1] / X[3], X[2] / X[3]];

/** Voxel coordinates [0, n]³ ↔ chart [−a, a]³ (a straight rescaling). */
export const voxelToChart = (g, v) => v.map((x) => ((2 * x) / g.n - 1) * g.a);
export const chartToVoxel = (g, p) => p.map((x) => (x / g.a + 1) * (g.n / 2));

/** Homogeneous voxel coords (v, 1) → homogeneous model coords. */
function voxelFrame(g) {
  const k = (2 * g.a) / g.n;
  return [k, 0, 0, -g.a, 0, k, 0, -g.a, 0, 0, k, -g.a, 0, 0, 0, 1];
}
function voxelFrameInverse(g) {
  const k = g.n / (2 * g.a);
  return [k, 0, 0, g.n / 2, 0, k, 0, g.n / 2, 0, 0, k, g.n / 2, 0, 0, 0, 1];
}

/** A model-space map written in homogeneous voxel coordinates (what the shader uses). */
export const toVoxelMap = (g, M) => mul4(voxelFrameInverse(g), mul4(M, voxelFrame(g)));

/** True distance in blocks between two model points (homogeneous, any scale). */
export function distance(g, X, Y) {
  if (g.kind === FLAT) return Math.hypot(X[0] / X[3] - Y[0] / Y[3], X[1] / X[3] - Y[1] / Y[3], X[2] / X[3] - Y[2] / Y[3]);
  const c = form(g, X, Y) / Math.sqrt(Math.abs(form(g, X, X) * form(g, Y, Y)));
  return g.kind === SPHERICAL ? g.R * Math.acos(Math.max(-1, Math.min(1, c))) : g.R * Math.acosh(Math.max(1, -c));
}

/**
 * How a tangent vector T at the point X moves the chart point: d(xyz/w) for a model-space
 * velocity T. Linear in T, so it turns camera directions into straight chart directions.
 */
export function chartVelocity(X, T) {
  const w = X[3];
  return [0, 1, 2].map((k) => (T[k] * w - X[k] * T[3]) / (w * w));
}

// ---------------------------------------------------------------------------
// Motions
// ---------------------------------------------------------------------------

/**
 * The motion along the straight line through the model point X (normalised) in the unit
 * tangent direction U, by `blocks`. It slides X along the line and carries directions along
 * with it (parallel transport), fixing everything perpendicular to the line.
 */
export function slide(g, X, U, blocks) {
  if (g.kind === FLAT) {
    const m = I4.slice();
    for (let k = 0; k < 3; k++) m[k * 4 + 3] = U[k] * blocks;
    return m;
  }
  const s = blocks / g.R;
  const [c, sn] = g.kind === SPHERICAL ? [Math.cos(s), Math.sin(s)] : [Math.cosh(s), Math.sinh(s)];
  // write V = α X + β U + (rest); X ↦ c X + sn U and U ↦ c U ∓ sn X (− on the sphere, + hyperbolic)
  const xx = form(g, X, X); // +1 sphere, −1 hyperbolic
  const image = (V) => {
    const alpha = form(g, V, X) / xx, beta = form(g, V, U);
    const nx = alpha * c + beta * (g.kind === SPHERICAL ? -sn : sn);
    const nu = alpha * sn + beta * c;
    return V.map((v, k) => v + (nx - alpha) * X[k] + (nu - beta) * U[k]);
  };
  return fromCols4([0, 1, 2, 3].map((k) => image([0, 1, 2, 3].map((r) => (r === k ? 1 : 0)))));
}

/** The motion that slides the model point X straight to the model point Y. */
export function slideTo(g, X, Y) {
  if (g.kind === FLAT) return slide(g, X, [Y[0] / Y[3] - X[0] / X[3], Y[1] / Y[3] - X[1] / X[3], Y[2] / Y[3] - X[2] / X[3], 0], 1);
  const x = normalise(g, X), y = normalise(g, Y);
  const d = distance(g, x, y);
  if (d < 1e-12) return I4.slice();
  const xy = form(g, x, y), xx = form(g, x, x);
  const u = y.map((v, k) => v - (xy / xx) * x[k]);
  const len = Math.sqrt(form(g, u, u));
  return slide(g, x, u.map((v) => v / len), d);
}

export function normalise(g, X) {
  if (g.kind === FLAT) return X.map((v) => v / X[3]);
  const s = Math.sqrt(Math.abs(form(g, X, X))) * Math.sign(X[3] || 1);
  return X.map((v) => v / s);
}

/** Reflection in the plane of face i of the cube (fixes that face, swaps its two sides). */
export function faceReflection(g, i) {
  const k = faceAxis(i), s = faceSign(i);
  if (g.kind === FLAT) {
    const m = I4.slice();
    m[k * 4 + k] = -1;
    m[k * 4 + 3] = 2 * s * g.a;
    return m;
  }
  // the face is the hyperplane <X, N> = 0 with N = e_k + s a e_w (hyperbolic) or e_k − s a e_w (sphere)
  const N = [0, 0, 0, g.kind === SPHERICAL ? -s * g.a : s * g.a];
  N[k] = 1;
  const nn = form(g, N, N);
  const image = (V) => { const t = (2 * form(g, V, N)) / nn; return V.map((v, j) => v - t * N[j]); };
  return fromCols4([0, 1, 2, 3].map((c) => image([0, 1, 2, 3].map((r) => (r === c ? 1 : 0)))));
}

/**
 * What going through face i does, as a motion of the model: glued faces apply the face map P
 * and then reflect across the partner face (exactly as in gluing.js, which is the flat case);
 * mirrors reflect across face i itself. Returns null for walls.
 */
export function portalMotion(g, faces, i) {
  const spec = faces[i];
  if (spec.kind === MIRROR) return faceReflection(g, i);
  if (spec.kind !== GLUE) return null;
  return mul4(faceReflection(g, spec.partner), embed3(spec.P));
}

/** Gram–Schmidt for a motion's columns (right, up, back, position), keeping its handedness. */
export function tidyMotion(g, M) {
  const cols = [0, 1, 2, 3].map((c) => col4(M, c));
  if (g.kind === FLAT) {
    const out = [];
    for (let c = 0; c < 3; c++) {
      let v = cols[c].slice(0, 3);
      for (const u of out) { const d = v[0] * u[0] + v[1] * u[1] + v[2] * u[2]; v = v.map((x, k) => x - d * u[k]); }
      const l = Math.hypot(...v);
      out.push(v.map((x) => x / l));
    }
    return fromCols4([...out.map((v) => [...v, 0]), [cols[3][0] / cols[3][3], cols[3][1] / cols[3][3], cols[3][2] / cols[3][3], 1]]);
  }
  const out = [normalise(g, cols[3])];
  for (let c = 0; c < 3; c++) {
    let v = cols[c];
    for (const u of out) { const t = form(g, v, u) / form(g, u, u); v = v.map((x, k) => x - t * u[k]); }
    const l = Math.sqrt(Math.abs(form(g, v, v)));
    out.push(v.map((x) => x / l));
  }
  return fromCols4([out[1], out[2], out[3], out[0]]);
}

/**
 * Where a motion M (columns: right, up, back, position) puts you, in voxel coordinates: the
 * position, and the voxel-space displacement of one block along each body axis.
 */
export function voxelPose(g, M) {
  const X = col4(M, 3);
  const k = g.n / (2 * g.a) / g.R;
  const axis = (c) => chartVelocity(X, col4(M, c)).map((x) => x * k);
  return { pos: chartToVoxel(g, modelToChart(X)), right: axis(0), up: axis(1), back: axis(2) };
}

/** Model point for a voxel position. */
export const voxelToModel = (g, v) => chartToModel(g, voxelToChart(g, v));
