// Topology of the space X = cube / (face gluings).
//
// The boundary of the cube is cut into 48 triangles (each face is split by its two midlines and
// two diagonals), so that every face map sends cells to cells and a face can even be folded
// onto itself. The quotient then has a cell structure:
//   0-cells: classes of the 26 boundary lattice points (corners, edge midpoints, face centres)
//   1-cells: classes of the 72 boundary segments
//   2-cells: classes of the 48 triangles
//   3-cell : the inside of the cube
// From it we read off the Euler characteristic, H₁ (cellular homology), whether every point
// has a ball (or half-ball) neighbourhood, orientability, the boundary surfaces, and the total
// angle around every edge, which says where the flat geometry of the cube is singular.

import { FACE_COUNT, faceNormal, faceFrame, matVec, det } from './cube.js';
import { GLUE, portal } from './gluing.js';
import { homologyGroup } from './homology.js';

// ---------------------------------------------------------------------------
// The subdivided boundary of the cube (same for every gluing, built once)
// ---------------------------------------------------------------------------
const key = (p) => p.join(',');
const typeOf = (p) => p.reduce((n, x) => n + (x !== 0 ? 1 : 0), 0); // 1 centre, 2 midpoint, 3 corner

export const COMPLEX = (() => {
  const vertices = [], vIndex = new Map();
  const edges = [], eIndex = new Map();
  const triangles = [], tIndex = new Map();
  const addVertex = (p) => {
    const k = key(p);
    if (!vIndex.has(k)) { vIndex.set(k, vertices.length); vertices.push({ p, type: typeOf(p) }); }
    return vIndex.get(k);
  };
  const edgeKey = (a, b) => (a < b ? `${a}-${b}` : `${b}-${a}`);
  const addEdge = (a, b) => {
    const k = edgeKey(a, b);
    if (!eIndex.has(k)) {
      // orient every edge from the lower vertex type to the higher one; face maps preserve types
      const [tail, head] = vertices[a].type < vertices[b].type ? [a, b] : [b, a];
      const cubeEdge = vertices[a].type >= 2 && vertices[b].type >= 2;
      eIndex.set(k, edges.length);
      edges.push({ tail, head, triangles: [], dihedral: cubeEdge ? 1 : 2 }); // in units of π/2
    }
    return eIndex.get(k);
  };
  const ring = [[1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1], [0, -1], [1, -1]];
  for (let f = 0; f < FACE_COUNT; f++) {
    const n = faceNormal(f), { u, v } = faceFrame(f);
    const at = ([a, b]) => [0, 1, 2].map((k) => n[k] + a * u[k] + b * v[k]);
    const c = addVertex(n);
    for (let k = 0; k < 8; k++) {
      const p = addVertex(at(ring[k])), q = addVertex(at(ring[(k + 1) % 8]));
      const t = triangles.length;
      triangles.push({ verts: [c, p, q], face: f }); // counterclockwise seen from outside
      tIndex.set([c, p, q].sort((x, y) => x - y).join('-'), t);
      for (const [a, b] of [[c, p], [p, q], [q, c]]) edges[addEdge(a, b)].triangles.push(t);
    }
  }
  return {
    vertices, edges, triangles,
    vertexAt: (p) => vIndex.get(key(p)),
    edgeBetween: (a, b) => eIndex.get(edgeKey(a, b)),
    triangleWith: (vs) => tIndex.get(vs.slice().sort((x, y) => x - y).join('-')),
  };
})();

// ---------------------------------------------------------------------------
// Union–find
// ---------------------------------------------------------------------------
class Classes {
  constructor(n) { this.parent = Array.from({ length: n }, (_, i) => i); }
  find(i) { while (this.parent[i] !== i) i = this.parent[i] = this.parent[this.parent[i]]; return i; }
  union(a, b) { this.parent[this.find(a)] = this.find(b); }
  /** Map from element to a dense class id 0..k-1, plus k. */
  labels() {
    const ids = new Map();
    const label = this.parent.map((_, i) => {
      const r = this.find(i);
      if (!ids.has(r)) ids.set(r, ids.size);
      return ids.get(r);
    });
    return { label, count: ids.size };
  }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/** How face f's identification acts on boundary cells (null for walls and mirrors). */
function cellMaps(faces) {
  const { vertices, edges, triangles } = COMPLEX;
  return faces.map((spec, i) => {
    if (spec.kind !== GLUE) return null;
    const vmap = (v) => COMPLEX.vertexAt(matVec(spec.P, vertices[v].p));
    return {
      vertex: vmap,
      edge: (e) => COMPLEX.edgeBetween(vmap(edges[e].tail), vmap(edges[e].head)),
      triangle: (t) => COMPLEX.triangleWith(triangles[t].verts.map(vmap)),
      reversesOrientation: det(portal(faces, i).A) < 0,
    };
  });
}

export function analyze(faces) {
  const { vertices, edges, triangles } = COMPLEX;
  const maps = cellMaps(faces);
  const glued = (t) => maps[triangles[t].face] !== null;

  // --- identify cells
  const vc = new Classes(vertices.length), ec = new Classes(edges.length), tc = new Classes(triangles.length);
  triangles.forEach((T, t) => {
    const m = maps[T.face];
    if (!m) return;
    tc.union(t, m.triangle(t));
    for (const v of T.verts) vc.union(v, m.vertex(v));
  });
  edges.forEach((E, e) => {
    for (const t of E.triangles) { const m = maps[triangles[t].face]; if (m) ec.union(e, m.edge(e)); }
  });
  const V = vc.labels(), E = ec.labels(), F = tc.labels();

  // --- edge cycles: walk around each edge class, alternating "cross to the other triangle on
  // this edge" and "go through the glued face"
  const edgeCycles = edgeOrbits(maps);

  // --- vertex links: a manifold point has a sphere (inside) or disk (on the boundary) as link
  const vertexLinks = [];
  for (let c = 0; c < V.count; c++) {
    const occ = [], tris = new Set(), eds = new Set();
    let boundary = false;
    vertices.forEach((_, v) => { if (V.label[v] === c) occ.push(v); });
    triangles.forEach((T, t) => {
      if (!T.verts.some((v) => V.label[v] === c)) return;
      tris.add(F.label[t]);
      if (!glued(t)) boundary = true;
    });
    edges.forEach((Ed, e) => { if (V.label[Ed.tail] === c || V.label[Ed.head] === c) eds.add(E.label[e]); });
    const chi = eds.size - tris.size + occ.length;
    const ok = boundary ? chi === 1 : chi === 2;
    vertexLinks.push({ cls: c, type: vertices[occ[0]].type, occurrences: occ, chi, boundary, ok, link: linkName(chi, boundary) });
  }

  // --- homology
  const d1 = zeros(V.count, E.count), d2 = zeros(E.count, F.count);
  edges.forEach((Ed, e) => {
    const j = E.label[e];
    if (classRep(E.label, j) !== e) return;
    d1[V.label[Ed.head]][j] += 1; d1[V.label[Ed.tail]][j] -= 1;
  });
  triangles.forEach((T, t) => {
    const j = F.label[t];
    if (classRep(F.label, j) !== t) return;
    const [a, b, c] = T.verts;
    for (const [x, y] of [[a, b], [b, c], [c, a]]) {
      const e = COMPLEX.edgeBetween(x, y);
      d2[E.label[e]][j] += edges[e].tail === x ? 1 : -1;
    }
  });
  const H1 = homologyGroup(E.count, d1, d2);

  const closed = faces.every((f) => f.kind === GLUE);
  const orientable = faces.every((f, i) => f.kind !== GLUE || det(portal(faces, i).A) === 1);
  const manifold = vertexLinks.every((l) => l.ok);
  const singularEdges = edgeCycles.filter((c) => !c.boundary && c.angle !== 4);
  const boundary = boundarySurfaces(faces, maps, edgeCycles, V);

  return {
    counts: { vertices: V.count, edges: E.count, faces: F.count },
    euler: V.count - E.count + F.count - 1,
    H1, closed, orientable, manifold,
    flat: singularEdges.length === 0,
    edgeCycles, singularEdges, vertexLinks,
    badPoints: vertexLinks.filter((l) => !l.ok),
    boundary,
    vertexClass: V.label, edgeClass: E.label, triangleClass: F.label,
  };
}

const zeros = (r, c) => Array.from({ length: r }, () => new Array(c).fill(0));
const classRep = (labels, j) => labels.indexOf(j);

function linkName(chi, boundary) {
  if (boundary) return chi === 1 ? 'disk' : `surface with boundary, χ=${chi}`;
  if (chi === 2) return 'sphere';
  if (chi === 1) return 'projective plane';
  if (chi === 0) return 'torus or Klein bottle';
  return `closed surface, χ=${chi}`;
}

// A "flag" is an (edge, triangle) incidence. Each edge class is one orbit of flags under
// σ (swap to the other triangle on the same edge, inside the cube) and τ (go through the glued
// face). The orbit is a cycle for an interior edge and a chain for an edge on the boundary.
function edgeOrbits(maps) {
  const { edges, triangles } = COMPLEX;
  const seen = new Set();
  const flagKey = (e, t) => `${e}:${t}`;
  const otherTriangle = (e, t) => (edges[e].triangles[0] === t ? edges[e].triangles[1] : edges[e].triangles[0]);
  const cycles = [];
  edges.forEach((Ed, e0) => {
    if (seen.has(flagKey(e0, Ed.triangles[0]))) return;
    const orbitEdges = new Set();
    // Follow τ, σ, τ, σ, … from a flag. Returns null if we come back round (a cycle), or the
    // boundary triangle where the chain stops and how many mirror-image gluings it went through.
    const walk = (e, t) => {
      let flips = 0;
      for (;;) {
        seen.add(flagKey(e, t)); orbitEdges.add(e);
        const m = maps[triangles[t].face];
        if (!m) return { end: t, flips };
        if (m.reversesOrientation) flips++;
        [e, t] = [m.edge(e), m.triangle(t)];
        seen.add(flagKey(e, t)); orbitEdges.add(e);
        t = otherTriangle(e, t);
        if (seen.has(flagKey(e, t))) return null;
      }
    };
    const leg1 = walk(e0, Ed.triangles[0]);
    const leg2 = leg1 && walk(e0, Ed.triangles[1]);
    cycles.push({
      edges: [...orbitEdges],
      angle: [...orbitEdges].reduce((s, e) => s + edges[e].dihedral, 0), // units of π/2
      boundary: !!leg1,
      // boundary chains: the boundary triangles at both ends, and whether the chain between
      // them passes through an odd number of orientation-reversing gluings
      ends: leg1 ? [leg1.end, leg2.end] : null,
      flipsOrientation: leg1 ? (leg1.flips + leg2.flips) % 2 === 1 : null,
    });
  });
  return cycles;
}

// Union–find that also tracks a parity (for 2-colouring triangle orientations).
class ParityClasses {
  constructor() { this.up = new Map(); }
  find(x) {
    let p = 0;
    while (this.up.has(x)) { const { to, flip } = this.up.get(x); p ^= flip; x = to; }
    return { root: x, p };
  }
  /** Require parity(a) xor parity(b) = want. Returns false if that contradicts earlier facts. */
  relate(a, b, want) {
    const A = this.find(a), B = this.find(b);
    if (A.root === B.root) return (A.p ^ B.p) === want;
    this.up.set(A.root, { to: B.root, flip: A.p ^ B.p ^ want });
    return true;
  }
}

// Components of the boundary surface (walls and mirrors), with Euler characteristic and
// orientability, found by joining boundary triangles along boundary edge chains.
function boundarySurfaces(faces, maps, edgeCycles, V) {
  const { triangles } = COMPLEX;
  const chains = edgeCycles.filter((c) => c.boundary);
  if (!chains.length) return [];
  const comp = new Classes(triangles.length), orient = new ParityClasses();
  const conflicts = [];
  for (const c of chains) {
    const [a, b] = c.ends;
    comp.union(a, b);
    if (!orient.relate(a, b, c.flipsOrientation ? 1 : 0)) conflicts.push(a);
  }
  const groups = new Map();
  triangles.forEach((T, t) => {
    if (maps[T.face]) return;
    const r = comp.find(t);
    if (!groups.has(r)) groups.set(r, []);
    groups.get(r).push(t);
  });
  const conflictRoots = new Set(conflicts.map((t) => comp.find(t)));
  return [...groups.entries()].map(([root, tris]) => {
    const inside = new Set(tris);
    const verts = new Set();
    for (const t of tris) for (const v of triangles[t].verts) verts.add(V.label[v]);
    const edgeCount = chains.filter((c) => inside.has(c.ends[0])).length;
    const chi = verts.size - edgeCount + tris.length;
    const orientable = !conflictRoots.has(root);
    const faceList = [...new Set(tris.map((t) => triangles[t].face))];
    return { chi, orientable, name: surfaceName(chi, orientable), faces: faceList, mirror: faceList.some((f) => faces[f].kind === 'mirror') };
  });
}

export function surfaceName(chi, orientable) {
  if (orientable) {
    if (chi === 2) return 'sphere';
    if (chi === 0) return 'torus';
    if (chi < 0 && chi % 2 === 0) return `genus-${1 - chi / 2} surface`;
  } else {
    if (chi === 1) return 'projective plane';
    if (chi === 0) return 'Klein bottle';
    if (chi < 1) return `non-orientable surface (χ=${chi})`;
  }
  return `surface with χ=${chi}`;
}

/**
 * Classes of the 24 half-edges of the cube's 12 edges, for drawing the cube's frame.
 * Half-edge index = cubeEdge * 2 + half, cubeEdge = axis * 4 + ia * 2 + ib, where the edge runs
 * along `axis` at the other two coordinates (ia, ib ∈ {0: −1, 1: +1}) and half 0 is the end at
 * the negative side of `axis`. For each: `full` numbers the classes of whole cube edges, `end`
 * (0/1) says which end of its class it is (so the frame can be drawn like an arrow), and
 * `singular` marks edges whose total angle is not 360°.
 */
export function cubeEdgeClasses(analysis) {
  const cycleOf = new Map();
  analysis.edgeCycles.forEach((c) => c.edges.forEach((e) => cycleOf.set(e, c)));
  const halves = [];
  for (let axis = 0; axis < 3; axis++) {
    const a = (axis + 1) % 3, b = (axis + 2) % 3;
    for (let ia = 0; ia < 2; ia++) for (let ib = 0; ib < 2; ib++) for (let h = 0; h < 2; h++) {
      const mid = [0, 0, 0];
      mid[a] = ia ? 1 : -1; mid[b] = ib ? 1 : -1;
      const corner = mid.slice();
      corner[axis] = h ? 1 : -1;
      const e = COMPLEX.edgeBetween(COMPLEX.vertexAt(mid), COMPLEX.vertexAt(corner));
      const cyc = cycleOf.get(e);
      halves.push({ cubeEdge: axis * 4 + ia * 2 + ib, cls: analysis.edgeClass[e], singular: !cyc.boundary && cyc.angle !== 4, boundary: cyc.boundary });
    }
  }
  // whole cube edges are in the same class when any of their halves are
  const whole = new Classes(12);
  const firstEdgeOfClass = new Map();
  for (const h of halves) {
    if (firstEdgeOfClass.has(h.cls)) whole.union(h.cubeEdge, firstEdgeOfClass.get(h.cls));
    else firstEdgeOfClass.set(h.cls, h.cubeEdge);
  }
  const { label } = whole.labels();
  const endOf = new Map();   // half-edge class -> 0 or 1 within its whole-edge class
  const endCount = new Map(); // whole-edge class -> number of half-edge classes in it (1 or 2)
  const out = halves.map((h) => {
    const full = label[h.cubeEdge];
    if (!endOf.has(h.cls)) {
      const n = endCount.get(full) || 0;
      endOf.set(h.cls, n);
      endCount.set(full, n + 1);
    }
    return { ...h, full, end: endOf.get(h.cls) };
  });
  return out.map((h) => ({ ...h, ends: endCount.get(h.full) }));
}

/** How many of the 12 cube edges, and how many fold lines inside faces, are singular. */
export function singularSummary(analysis) {
  const halves = cubeEdgeClasses(analysis);
  const cubeEdges = new Set(halves.filter((h) => h.singular).map((h) => h.cubeEdge)).size;
  const halfCount = halves.filter((h) => h.singular).length;
  const others = analysis.singularEdges.filter((c) => c.edges.some((e) => COMPLEX.edges[e].dihedral === 2)).length;
  return { cubeEdges, halfCount, foldLines: others };
}
