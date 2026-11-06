// A gluing says, for each of the six faces of the cube [-1, 1]^3, what happens when you
// walk through it:
//
//   { kind: 'wall' }                     solid; you bump into it
//   { kind: 'mirror' }                   you (and light) bounce back as a mirror image
//   { kind: 'glue', partner: j, P }      you come out of face j
//
// For a glued face, P is the cube symmetry (a signed permutation matrix) whose restriction to
// face i is the identification of face i with face j: the point p on face i is the same point
// as P·p on face j. Every face map between two faces of a cube extends to exactly one cube
// symmetry, so P is a complete description. Partner faces always carry inverse maps, and a
// face glued to itself must use an involution (otherwise light would not retrace its path).

import {
  FACE_COUNT, IDENTITY, faceAxis, faceSign, faceFrame, flipAxis, quarterTurnAbout,
  matMul, matVec, matEquals, transpose, det, imageFace, oppositeFace, faceMap2D, unit,
} from './cube.js';

export const WALL = 'wall';
export const MIRROR = 'mirror';
export const GLUE = 'glue';

export const wall = () => ({ kind: WALL });
export const mirror = () => ({ kind: MIRROR });

/** The "natural" way to glue face i onto face j, which twist/flip are measured from. */
export function defaultMap(i, j) {
  if (i === j) return IDENTITY.slice();
  if (j === oppositeFace(i)) return flipAxis(faceAxis(i)); // straight across: a translation
  // Adjacent faces: the reflection that swaps them and fixes their shared edge (a fold).
  const a = faceAxis(i), b = faceAxis(j), s = faceSign(i) * faceSign(j);
  const m = IDENTITY.slice();
  m[a * 4] = 0; m[b * 4] = 0;
  m[b * 3 + a] = s; m[a * 3 + b] = s;
  return m;
}

/** Reflection of the cube that mirrors face i left-to-right (u -> -u in the face's frame). */
function mirrorOnFace(i) {
  const u = faceFrame(i).u;
  return IDENTITY.map((x, k) => x - 2 * u[Math.floor(k / 3)] * u[k % 3]);
}

/**
 * Face map from i to j: first rotate face i by `twist` quarter turns (counterclockwise as seen
 * from outside), optionally mirror it left-to-right, then apply the natural map.
 */
export function mapFromTwist(i, j, twist, flip) {
  let m = IDENTITY.slice();
  if (flip) m = mirrorOnFace(i);
  const q = quarterTurnAbout(i);
  for (let k = 0; k < ((twist % 4) + 4) % 4; k++) m = matMul(q, m);
  return matMul(defaultMap(i, j), m);
}

/** Inverse of mapFromTwist: which (twist, flip) produces P? */
export function twistOf(i, j, P) {
  for (const flip of [false, true]) {
    for (let twist = 0; twist < 4; twist++) {
      if (matEquals(mapFromTwist(i, j, twist, flip), P)) return { twist, flip };
    }
  }
  throw new Error(`map does not send face ${i} to face ${j}`);
}

// The non-trivial ways to glue a face to itself (involutions of the square), described by
// their effect in the face's own frame (u right, v up, seen from outside).
export const SELF_FOLDS = [
  { id: 'fold-h', label: 'fold top ↔ bottom', map2d: [1, 0, 0, -1] },
  { id: 'fold-v', label: 'fold left ↔ right', map2d: [-1, 0, 0, 1] },
  { id: 'fold-d', label: 'fold along ⟋ diagonal', map2d: [0, 1, 1, 0] },
  { id: 'fold-a', label: 'fold along ⟍ diagonal', map2d: [0, -1, -1, 0] },
  { id: 'half-turn', label: 'half-turn about centre', map2d: [-1, 0, 0, -1] },
];

/** The cube symmetry that glues face i to itself by the named fold. */
export function selfFoldMap(i, foldId) {
  const fold = SELF_FOLDS.find((f) => f.id === foldId);
  if (!fold) throw new Error(`unknown fold ${foldId}`);
  for (const flip of [false, true]) {
    for (let twist = 0; twist < 4; twist++) {
      const P = mapFromTwist(i, i, twist, flip);
      if (faceMap2D(P, i, i).every((x, k) => x === fold.map2d[k])) return P;
    }
  }
  throw new Error('unreachable');
}

export function selfFoldOf(i, P) {
  const m = faceMap2D(P, i, i);
  const fold = SELF_FOLDS.find((f) => f.map2d.every((x, k) => x === m[k]));
  return fold ? fold.id : null;
}

// ---------------------------------------------------------------------------
// Building and editing gluings (all functions return new objects)
// ---------------------------------------------------------------------------

export const emptyGluing = () => Array.from({ length: FACE_COUNT }, wall);

/** Glue face i to face j with cube symmetry P (and j back to i with the inverse). */
export function withGlue(faces, i, j, P) {
  const out = detach(detach(faces, i), j);
  out[i] = { kind: GLUE, partner: j, P: P.slice() };
  out[j] = { kind: GLUE, partner: i, P: transpose(P) };
  return out;
}

/** Make face i a wall or a mirror (freeing its old partner, which becomes a wall). */
export function withBoundary(faces, i, kind) {
  const out = detach(faces, i);
  out[i] = kind === MIRROR ? mirror() : wall();
  return out;
}

function detach(faces, i) {
  const out = faces.map((f) => ({ ...f }));
  const f = out[i];
  if (f.kind === GLUE && f.partner !== i) out[f.partner] = wall();
  out[i] = wall();
  return out;
}

/** Glue opposite faces (axis 0, 1, 2) with a given twist and flip. */
export function glueOpposite(faces, axis, twist = 0, flip = false) {
  const i = axis * 2, j = i + 1;
  return withGlue(faces, i, j, mapFromTwist(i, j, twist, flip));
}

export function glueSelf(faces, i, foldId) {
  return withGlue(faces, i, i, selfFoldMap(i, foldId));
}

/** Returns a list of human-readable problems; empty means the gluing is consistent. */
export function validate(faces) {
  const errors = [];
  if (faces.length !== FACE_COUNT) return ['need exactly six faces'];
  faces.forEach((f, i) => {
    if (f.kind === WALL || f.kind === MIRROR) return;
    if (f.kind !== GLUE) { errors.push(`face ${i}: unknown kind ${f.kind}`); return; }
    const { partner: j, P } = f;
    if (!Number.isInteger(j) || j < 0 || j >= FACE_COUNT) { errors.push(`face ${i}: bad partner`); return; }
    if (Math.abs(det(P)) !== 1 || !matEquals(matMul(P, transpose(P)), IDENTITY)) {
      errors.push(`face ${i}: map is not a cube symmetry`); return;
    }
    if (imageFace(P, i) !== j) { errors.push(`face ${i}: map does not land on face ${j}`); return; }
    const back = faces[j];
    if (back.kind !== GLUE || back.partner !== i || !matEquals(back.P, transpose(P))) {
      errors.push(`face ${i}: partner ${j} is not glued back with the inverse map`);
    }
    if (i === j && matEquals(P, IDENTITY)) errors.push(`face ${i}: glued to itself by the identity`);
    if (i === j && !matEquals(matMul(P, P), IDENTITY)) errors.push(`face ${i}: self-gluing must be an involution`);
  });
  return errors;
}

// ---------------------------------------------------------------------------
// Portals: what happens to a point or a light ray that leaves the cube through face i.
// The answer is an isometry h(x) = A·x + t of space that maps face i onto its partner and the
// region just outside face i to the region just inside the partner. Coordinates are those of
// the unit cube [-1, 1]^3.
// ---------------------------------------------------------------------------

export function portal(faces, i) {
  const f = faces[i];
  if (f.kind === WALL) return null;
  if (f.kind === MIRROR) {
    const a = faceAxis(i);
    return { A: flipAxis(a), t: unit(a, 2 * faceSign(i)), target: i };
  }
  const j = f.partner, a = faceAxis(j);
  // Apply the face map P (which keeps the cube in place), then reflect across the target face.
  return { A: matMul(flipAxis(a), f.P), t: unit(a, 2 * faceSign(j)), target: j };
}

export function applyPortal(h, x) {
  const y = matVec(h.A, x);
  return [y[0] + h.t[0], y[1] + h.t[1], y[2] + h.t[2]];
}

/** True when walking through every glued face keeps your handedness. */
export function preservesOrientation(faces, i) {
  const h = portal(faces, i);
  return !h || det(h.A) === 1;
}

// ---------------------------------------------------------------------------
// Compact text encoding, used for the URL hash. One token per face:
//   w = wall, m = mirror, <partner><twist>[f] for gluings, e.g. "10" or "53f".
// ---------------------------------------------------------------------------

export function encode(faces) {
  return faces.map((f, i) => {
    if (f.kind === WALL) return 'w';
    if (f.kind === MIRROR) return 'm';
    const { twist, flip } = twistOf(i, f.partner, f.P);
    return `${f.partner}${twist}${flip ? 'f' : ''}`;
  }).join('-');
}

export function decode(text) {
  const tokens = String(text).trim().split('-');
  if (tokens.length !== FACE_COUNT) throw new Error('expected six face tokens');
  let faces = emptyGluing();
  tokens.forEach((tok, i) => {
    if (tok === 'w') return;
    if (tok === 'm') { faces[i] = mirror(); return; }
    const m = /^([0-5])([0-3])(f?)$/.exec(tok);
    if (!m) throw new Error(`bad token "${tok}"`);
    const j = +m[1];
    if (j < i) return; // the lower-numbered face of each pair defines it
    faces = withGlue(faces, i, j, mapFromTwist(i, j, +m[2], m[3] === 'f'));
  });
  for (let i = 0; i < FACE_COUNT; i++) if (tokens[i] === 'm') faces[i] = mirror();
  const errors = validate(faces);
  if (errors.length) throw new Error(errors.join('; '));
  return faces;
}

/** Conjugate a gluing by a cube symmetry g (the same space, viewed after rotating/reflecting). */
export function conjugate(faces, g) {
  const gi = transpose(g);
  const out = emptyGluing();
  faces.forEach((f, i) => {
    const gi_ = imageFace(g, i);
    if (f.kind !== GLUE) { out[gi_] = { kind: f.kind }; return; }
    out[gi_] = { kind: GLUE, partner: imageFace(g, f.partner), P: matMul(g, matMul(f.P, gi)) };
  });
  return out;
}

export const sameGluing = (a, b) => a.every((f, i) => {
  const g = b[i];
  if (f.kind !== g.kind) return false;
  return f.kind !== GLUE || (f.partner === g.partner && matEquals(f.P, g.P));
});

