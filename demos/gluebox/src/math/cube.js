// The cube [-1, 1]^3, its six faces and its 48 symmetries.
//
// Everything here is exact integer arithmetic: cube symmetries are signed permutation
// matrices, stored row-major as flat arrays of 9 numbers.

export const FACE_COUNT = 6;

// Face i lies on the plane x[axis] = sign. Order: +x, -x, +y, -y, +z, -z.
export const faceAxis = (i) => i >> 1;
export const faceSign = (i) => (i & 1 ? -1 : 1);
export const faceIndex = (axis, sign) => axis * 2 + (sign > 0 ? 0 : 1);
export const oppositeFace = (i) => i ^ 1;
export const FACE_NAMES = ['+x', '−x', '+y', '−y', '+z', '−z'];
export const FACE_NICKNAMES = ['east', 'west', 'ceiling', 'floor', 'south', 'north'];

export function unit(axis, sign = 1) {
  const v = [0, 0, 0];
  v[axis] = sign;
  return v;
}

export const faceNormal = (i) => unit(faceAxis(i), faceSign(i));

// A 2D frame on every face, as seen from OUTSIDE the cube: u points right, v points up,
// and u × v is the outward normal. Side faces keep +y as "up"; the top and bottom faces
// use -z as "up" (north).
const FACE_FRAMES = [
  { u: [0, 0, -1], v: [0, 1, 0] },  // +x
  { u: [0, 0, 1], v: [0, 1, 0] },   // -x
  { u: [1, 0, 0], v: [0, 0, -1] },  // +y
  { u: [-1, 0, 0], v: [0, 0, -1] }, // -y
  { u: [1, 0, 0], v: [0, 1, 0] },   // +z
  { u: [-1, 0, 0], v: [0, 1, 0] },  // -z
];
export const faceFrame = (i) => FACE_FRAMES[i];

// ---------------------------------------------------------------------------
// 3x3 matrices (row-major flat arrays) and vectors
// ---------------------------------------------------------------------------
export const IDENTITY = Object.freeze([1, 0, 0, 0, 1, 0, 0, 0, 1]);

export function matMul(a, b) {
  const o = new Array(9).fill(0);
  for (let r = 0; r < 3; r++) {
    for (let c = 0; c < 3; c++) {
      let s = 0;
      for (let k = 0; k < 3; k++) s += a[r * 3 + k] * b[k * 3 + c];
      o[r * 3 + c] = s;
    }
  }
  return o;
}

export function matVec(m, v) {
  return [
    m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
    m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
    m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
  ];
}

export function transpose(m) {
  return [m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]];
}

export function det(m) {
  return (
    m[0] * (m[4] * m[8] - m[5] * m[7]) -
    m[1] * (m[3] * m[8] - m[5] * m[6]) +
    m[2] * (m[3] * m[7] - m[4] * m[6])
  );
}

export const matEquals = (a, b) => a.every((x, i) => x === b[i]);

/** Build a matrix from its three columns. */
export function fromColumns(c0, c1, c2) {
  return [c0[0], c1[0], c2[0], c0[1], c1[1], c2[1], c0[2], c1[2], c2[2]];
}

export const column = (m, c) => [m[c], m[3 + c], m[6 + c]];

/** Diagonal matrix that flips one axis (reflection in the plane x[axis] = 0). */
export function flipAxis(axis) {
  const m = IDENTITY.slice();
  m[axis * 4] = -1;
  return m;
}

/** Quarter turn about an outward face normal (right-hand rule), as a cube symmetry. */
export function quarterTurnAbout(face) {
  const n = faceNormal(face);
  // Rotation by +90° about unit vector n: v -> (n·v)n + n × v
  const cols = [0, 1, 2].map((a) => {
    const e = unit(a);
    const nd = n[0] * e[0] + n[1] * e[1] + n[2] * e[2];
    const cr = cross(n, e);
    return [nd * n[0] + cr[0], nd * n[1] + cr[1], nd * n[2] + cr[2]];
  });
  return fromColumns(cols[0], cols[1], cols[2]);
}

export function cross(a, b) {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

export const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/** Which face does the outward normal n (a signed unit axis vector) belong to? */
export function faceOfNormal(n) {
  for (let a = 0; a < 3; a++) if (n[a] !== 0) return faceIndex(a, n[a]);
  throw new Error('not an axis vector');
}

/** The face that cube symmetry P sends face i to. */
export const imageFace = (P, i) => faceOfNormal(matVec(P, faceNormal(i)));

// All 48 signed permutation matrices = the full symmetry group of the cube.
export const CUBE_SYMMETRIES = (() => {
  const perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
  const out = [];
  for (const p of perms) {
    for (let s = 0; s < 8; s++) {
      const m = new Array(9).fill(0);
      for (let r = 0; r < 3; r++) m[r * 3 + p[r]] = (s >> r) & 1 ? -1 : 1;
      out.push(m);
    }
  }
  return out;
})();

/**
 * The 2x2 matrix of a face map in face frames: if P sends face i to face j, then a point
 * with face coordinates (u, v) on face i lands at M·(u, v) on face j. Row-major [a, b, c, d].
 */
export function faceMap2D(P, i, j) {
  const fi = faceFrame(i), fj = faceFrame(j);
  const Pu = matVec(P, fi.u), Pv = matVec(P, fi.v);
  return [dot(fj.u, Pu), dot(fj.u, Pv), dot(fj.v, Pu), dot(fj.v, Pv)];
}

/** Human readable "(x, y, z) ↦ (−x, y, z−2)"-style description of a linear map. */
export function describeLinear(m) {
  const names = ['x', 'y', 'z'];
  const parts = [0, 1, 2].map((r) => {
    for (let c = 0; c < 3; c++) {
      const v = m[r * 3 + c];
      if (Math.abs(v) > 0.5) return (v < 0 ? '−' : '') + names[c];
    }
    return '0';
  });
  return `(${parts.join(', ')})`;
}
