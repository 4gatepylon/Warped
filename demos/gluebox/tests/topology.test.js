import { test } from 'node:test';
import assert from 'node:assert/strict';
import { analyze, COMPLEX, cubeEdgeClasses } from '../src/math/topology.js';
import { invariantFactors, homologyGroup, formatGroup } from '../src/math/homology.js';
import { emptyGluing, decode } from '../src/math/gluing.js';
import { PRESETS, matchPreset, recognize } from '../src/catalog.js';
import { CUBE_SYMMETRIES } from '../src/math/cube.js';
import { conjugate } from '../src/math/gluing.js';

const preset = (id) => PRESETS.find((p) => p.id === id);

test('the subdivided cube boundary is a sphere', () => {
  assert.equal(COMPLEX.vertices.length, 26);
  assert.equal(COMPLEX.edges.length, 72);
  assert.equal(COMPLEX.triangles.length, 48);
  assert.equal(26 - 72 + 48, 2);
  for (const e of COMPLEX.edges) assert.equal(e.triangles.length, 2);
});

test('Smith normal form', () => {
  assert.deepEqual(invariantFactors([[2, 4, 4], [-6, 6, 12], [10, -4, -16]]), [2, 6, 12]);
  assert.deepEqual(invariantFactors([[0, 0], [0, 0]]), []);
  assert.deepEqual(homologyGroup(3, [[0, 0, 0]], [[2], [0], [0]]), { free: 2, torsion: [2] });
  assert.equal(formatGroup({ free: 1, torsion: [2, 2] }), 'ℤ ⊕ (ℤ/2)²');
  assert.equal(formatGroup({ free: 0, torsion: [] }), '0');
});

test('an unglued cube is a ball', () => {
  const a = analyze(emptyGluing());
  assert.equal(a.euler, 1);
  assert.equal(formatGroup(a.H1), '0');
  assert.ok(a.manifold && !a.closed);
  assert.deepEqual(a.boundary.map((b) => b.name), ['sphere']);
});

// Expected values from the classification of flat 3-manifolds (Conway & Rossetti, "Describing
// the platycosms") and standard facts about S³, RP³, S³/Q₈, S²×S¹ and the bounded examples.
const EXPECTED = {
  t3: { H1: 'ℤ³', orientable: true, flat: true },
  dicosm: { H1: 'ℤ ⊕ (ℤ/2)²', orientable: true, flat: true },
  tetracosm: { H1: 'ℤ ⊕ ℤ/2', orientable: true, flat: true },
  tricosm: { H1: 'ℤ ⊕ ℤ/3', orientable: true, flat: true },
  didicosm: { H1: '(ℤ/4)²', orientable: true, flat: true },
  amphicosm1: { H1: 'ℤ² ⊕ ℤ/2', orientable: false, flat: true },
  amphicosm2: { H1: 'ℤ²', orientable: false, flat: true },
  amphidicosm1: { H1: 'ℤ ⊕ (ℤ/2)²', orientable: false, flat: true },
  amphidicosm2: { H1: 'ℤ ⊕ ℤ/4', orientable: false, flat: true },
  s3: { H1: '0', orientable: true, flat: false },
  rp3: { H1: 'ℤ/2', orientable: true, flat: false },
  quaternion: { H1: '(ℤ/2)²', orientable: true, flat: false },
  s2xs1: { H1: 'ℤ', orientable: true, flat: false },
  'solid-torus': { H1: 'ℤ', orientable: true, flat: true, boundary: ['torus'] },
  t2xi: { H1: 'ℤ²', orientable: true, flat: true, boundary: ['torus', 'torus'] },
  kxi: { H1: 'ℤ ⊕ ℤ/2', orientable: false, flat: true, boundary: ['Klein bottle', 'Klein bottle'] },
  'solid-klein': { H1: 'ℤ', orientable: false, flat: true, boundary: ['Klein bottle'] },
  kaleidoscope: { H1: '0', orientable: true, flat: true, boundary: ['sphere'] },
};

for (const [id, want] of Object.entries(EXPECTED)) {
  test(`preset ${id}: ${want.H1}`, () => {
    const a = analyze(preset(id).faces);
    assert.equal(formatGroup(a.H1), want.H1);
    assert.equal(a.orientable, want.orientable);
    assert.equal(a.flat, want.flat);
    assert.ok(a.manifold, 'should be a manifold');
    if (want.boundary) {
      assert.ok(!a.closed);
      assert.deepEqual(a.boundary.map((b) => b.name).sort(), want.boundary.slice().sort());
      // a compact 3-manifold has χ = χ(boundary) / 2
      assert.equal(a.euler, a.boundary.reduce((s, b) => s + b.chi, 0) / 2);
    } else {
      assert.ok(a.closed);
      assert.equal(a.euler, 0, 'closed 3-manifolds have Euler characteristic 0');
      assert.deepEqual(a.boundary, []);
    }
  });
}

test('flat closed presets have 360° around every edge; RP³ has 180°, S³/Q₈ has 270°', () => {
  for (const id of ['t3', 'didicosm', 'tricosm']) {
    assert.ok(analyze(preset(id).faces).edgeCycles.every((c) => c.angle === 4));
  }
  const rp3 = analyze(preset('rp3').faces);
  assert.ok(rp3.edgeCycles.filter((c) => c.edges.some((e) => COMPLEX.edges[e].dihedral === 1)).every((c) => c.angle === 2));
  const q8 = analyze(preset('quaternion').faces);
  assert.ok(q8.edgeCycles.filter((c) => c.edges.some((e) => COMPLEX.edges[e].dihedral === 1)).every((c) => c.angle === 3));
});

test('a face folded by a half-turn is not a manifold point (cone on RP²)', () => {
  const a = analyze(decode('w-w-w-w-42-w'));
  assert.ok(!a.manifold);
  assert.deepEqual(a.badPoints.map((p) => [p.type, p.link]), [[1, 'projective plane']]);
  assert.ok(!analyze(preset('inversions').faces).manifold);
});

test('in a closed gluing every triangle is glued to exactly one other', () => {
  for (const p of PRESETS.filter((q) => q.faces.every((f) => f.kind === 'glue'))) {
    const a = analyze(p.faces);
    const sum = new Map();
    COMPLEX.triangles.forEach((T, t) => {
      const cls = a.triangleClass[t];
      sum.set(cls, (sum.get(cls) || 0) + 1);
    });
    assert.ok([...sum.values()].every((n) => n === 2), `${p.id}: every triangle class has two sides`);
  }
});

test('the cube-edge colouring groups whole edges into classes', () => {
  const t3 = cubeEdgeClasses(analyze(preset('t3').faces));
  assert.equal(new Set(t3.map((h) => h.full)).size, 3);           // x-, y- and z-edges
  assert.ok(t3.every((h) => h.ends === 2 && !h.singular));
  const s3 = cubeEdgeClasses(analyze(preset('s3').faces));
  assert.ok(s3.some((h) => h.singular));
});

test('presets are recognised even after rotating or mirroring the cube', () => {
  for (const p of PRESETS) {
    const g = CUBE_SYMMETRIES[(p.id.length * 7) % 48];
    const turned = conjugate(p.faces, g);
    assert.equal(matchPreset(turned)?.id, p.id);
    assert.equal(formatGroup(analyze(turned).H1), formatGroup(analyze(p.faces).H1));
  }
});

test('an unnamed flat closed gluing is named from its invariants', () => {
  // flips on two pairs of faces: not a preset (even after turning the cube), but still K × S¹
  const faces = decode('10-00-30f-20f-50f-40f');
  assert.equal(matchPreset(faces), null);
  const a = analyze(faces);
  assert.ok(a.flat && a.manifold && a.closed);
  const name = recognize(faces, a, analyze);
  assert.ok(name.exact);
  assert.match(name.title, /Klein bottle × circle/);
});
