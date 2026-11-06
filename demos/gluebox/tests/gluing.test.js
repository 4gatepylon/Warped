import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  CUBE_SYMMETRIES, IDENTITY, det, matMul, matVec, imageFace, faceNormal, oppositeFace, faceMap2D,
} from '../src/math/cube.js';
import {
  emptyGluing, glueOpposite, glueSelf, withGlue, withBoundary, mapFromTwist, twistOf, defaultMap,
  SELF_FOLDS, selfFoldMap, validate, portal, applyPortal, encode, decode, conjugate, sameGluing,
} from '../src/math/gluing.js';

const close = (a, b) => a.every((x, i) => Math.abs(x - b[i]) < 1e-12);

test('the cube has 48 symmetries, all distinct signed permutations', () => {
  assert.equal(CUBE_SYMMETRIES.length, 48);
  assert.equal(new Set(CUBE_SYMMETRIES.map((m) => m.join(','))).size, 48);
  for (const m of CUBE_SYMMETRIES) assert.equal(Math.abs(det(m)), 1);
});

test('each face pair has exactly 8 face maps, and twist/flip reaches each once', () => {
  for (let i = 0; i < 6; i++) {
    for (let j = 0; j < 6; j++) {
      const maps = new Set();
      for (let t = 0; t < 4; t++) {
        for (const f of [false, true]) {
          const P = mapFromTwist(i, j, t, f);
          assert.equal(imageFace(P, i), j);
          maps.add(P.join(','));
          assert.deepEqual(twistOf(i, j, P), { twist: t, flip: f });
        }
      }
      assert.equal(maps.size, 8);
    }
  }
});

test('the default map sends a face straight across, or folds it about the shared edge', () => {
  // opposite faces: a translation portal
  const f = glueOpposite(emptyGluing(), 0);
  assert.deepEqual(portal(f, 0), { A: IDENTITY.slice(), t: [-2, 0, 0], target: 1 });
  // adjacent faces +x and +y: points on their shared edge stay put
  const P = defaultMap(0, 2);
  assert.deepEqual(matVec(P, [1, 1, 0.5]), [1, 1, 0.5]);
  assert.equal(imageFace(P, 0), 2);
});

test('self folds are the five non-trivial involutions of the square', () => {
  for (let i = 0; i < 6; i++) {
    const seen = new Set();
    for (const fold of SELF_FOLDS) {
      const P = selfFoldMap(i, fold.id);
      assert.deepEqual(matMul(P, P), IDENTITY);
      assert.notDeepEqual(P, IDENTITY);
      assert.equal(imageFace(P, i), i);
      assert.deepEqual(faceMap2D(P, i, i), fold.map2d);
      seen.add(P.join(','));
    }
    assert.equal(seen.size, 5);
  }
});

test('portals map the face onto its partner and the cube onto the neighbouring cube', () => {
  const faces = glueSelf(withGlue(glueOpposite(emptyGluing(), 2, 1, true), 0, 4, mapFromTwist(0, 4, 3, false)), 1, 'fold-d');
  assert.deepEqual(validate(faces), []);
  for (let i = 0; i < 6; i++) {
    const h = portal(faces, i);
    if (!h) continue;
    const j = h.target;
    // the centre of face i goes to the centre of face j
    assert.ok(close(applyPortal(h, faceNormal(i)), faceNormal(j)));
    // the cube's centre goes to the centre of the cube on the far side of face j
    assert.ok(close(applyPortal(h, [0, 0, 0]), faceNormal(j).map((x) => 2 * x)));
    // a point just outside face i ends up just inside face j
    const out = faceNormal(i).map((x) => 1.01 * x);
    const inn = applyPortal(h, out);
    assert.ok(Math.max(...inn.map(Math.abs)) < 1);
    // and going back through the partner undoes it
    const back = portal(faces, j);
    assert.ok(close(applyPortal(back, applyPortal(h, [0.3, -0.2, 0.9])), [0.3, -0.2, 0.9]));
  }
});

test('the antipodal gluing really glues every boundary point to its antipode (RP³)', () => {
  let faces = emptyGluing();
  for (let a = 0; a < 3; a++) faces = glueOpposite(faces, a, 2);
  for (let i = 0; i < 6; i++) {
    const p = faceNormal(i).map((x, k) => (x !== 0 ? x : [0.3, -0.7, 0.45][k]));
    assert.deepEqual(matVec(faces[i].P, p), p.map((x) => -x));
  }
});

test('editing keeps the gluing consistent', () => {
  let faces = glueOpposite(emptyGluing(), 0);
  faces = withGlue(faces, 0, 2, mapFromTwist(0, 2, 1, true)); // steals +x from -x
  assert.equal(faces[1].kind, 'wall');
  assert.deepEqual(validate(faces), []);
  faces = withBoundary(faces, 2, 'mirror');
  assert.equal(faces[0].kind, 'wall');
  assert.equal(faces[2].kind, 'mirror');
  assert.deepEqual(validate(faces), []);
});

test('validation catches broken gluings', () => {
  const faces = glueOpposite(emptyGluing(), 0);
  faces[1] = { ...faces[1], P: IDENTITY.slice() };
  assert.ok(validate(faces).length > 0);
  const self = emptyGluing();
  self[0] = { kind: 'glue', partner: 0, P: mapFromTwist(0, 0, 1, false) }; // quarter turn: not an involution
  assert.ok(validate(self).some((e) => e.includes('involution')));
});

test('encode / decode round-trips', () => {
  const faces = glueSelf(glueOpposite(withBoundary(emptyGluing(), 4, 'mirror'), 0, 3, true), 2, 'half-turn');
  const code = encode(faces);
  assert.equal(code, '13f-01f-22-w-m-w');
  assert.ok(sameGluing(decode(code), faces));
  assert.throws(() => decode('nonsense'));
});

test('conjugating by a symmetry gives a consistent gluing', () => {
  const faces = glueOpposite(glueOpposite(emptyGluing(), 0, 1), 2, 0, true);
  for (const g of CUBE_SYMMETRIES) assert.deepEqual(validate(conjugate(faces, g)), []);
  assert.ok(sameGluing(conjugate(faces, IDENTITY), faces));
  assert.equal(oppositeFace(4), 5);
});
