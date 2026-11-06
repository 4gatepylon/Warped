import { test } from 'node:test';
import assert from 'node:assert/strict';
import { det, matMul, IDENTITY } from '../src/math/cube.js';
import { PRESETS } from '../src/catalog.js';
import { buildScene, spawnPoint, CUBE_SIZES, VoxelGrid } from '../src/world/scene.js';
import { PortalSpace } from '../src/world/space.js';
import { Player, RADIUS } from '../src/world/player.js';

const preset = (id) => PRESETS.find((p) => p.id === id).faces;
const emptyGrid = (n) => new VoxelGrid(n);
const near = (a, b, eps = 1e-9) => a.every((x, i) => Math.abs(x - b[i]) < eps);
// body frames (columns right, up, back) for facing east (+x) and west (-x)
const FACING_EAST = [0, 0, -1, 0, 1, 0, 1, 0, 0];
const FACING_WEST = [0, 0, 1, 0, 1, 0, -1, 0, 0];

test('the scene fits in every cube size and leaves room to spawn', () => {
  for (const n of CUBE_SIZES) {
    const g = buildScene(n);
    const b = g.bounds();
    assert.ok(b.min.every((x) => x >= 1) && b.max.every((x) => x <= n), `scene fits in ${n}`);
    const s = spawnPoint(n);
    assert.equal(g.get(...s.pos.map(Math.floor)), 0, 'spawn point is empty');
  }
});

test('flying east in the 3-torus comes back in from the west, unchanged', () => {
  const space = new PortalSpace(preset('t3'), emptyGrid(16));
  const p = new Player(space);
  p.place([15.5, 8, 8], FACING_EAST.slice());
  p.pitch = 0;
  p.move([0, 0, 1]);
  assert.ok(near(p.pos, [0.5, 8, 8], 1e-6), `got ${p.pos}`);
  assert.equal(p.crossings, 1);
  assert.deepEqual(p.holonomy, IDENTITY);
  assert.ok(!p.mirrored);
});

test('flying north in Klein bottle × circle mirrors you, twice restores you', () => {
  const space = new PortalSpace(preset('amphicosm1'), emptyGrid(16));
  const p = new Player(space);
  p.place([5, 8, 0.4], IDENTITY.slice()); // facing -z = north
  p.pitch = 0;
  p.move([0, 0, 1]);
  assert.equal(p.crossings, 1);
  assert.ok(p.mirrored);
  assert.ok(near(p.pos, [11, 8, 15.4], 1e-6), `x is mirrored: got ${p.pos}`);
  p.move([0, 0, 16]);
  assert.equal(p.crossings, 2);
  assert.ok(!p.mirrored);
  assert.ok(near(p.pos, [5, 8, 15.4], 1e-6), `got ${p.pos}`);
});

test('in the half-turn space, going north turns you upside down', () => {
  const space = new PortalSpace(preset('dicosm'), emptyGrid(16));
  const p = new Player(space);
  p.place([8, 8, 0.2], IDENTITY.slice());
  p.move([0, 0, 0.5]);
  assert.equal(p.crossings, 1);
  assert.ok(!p.mirrored);
  assert.ok(p.body[4] < -0.99, 'up now points along -y');
});

test('walls stop you, mirrors bounce you', () => {
  const walls = new PortalSpace(preset('solid-torus'), emptyGrid(16));
  const p = new Player(walls);
  p.place([1, 8, 8], FACING_WEST.slice());
  p.move([0, 0, 5]);
  assert.ok(Math.abs(p.pos[0] - RADIUS) < 0.11, `stopped at the wall: ${p.pos}`);
  assert.equal(p.crossings, 0);

  const mirrors = new PortalSpace(preset('kaleidoscope'), emptyGrid(16));
  const q = new Player(mirrors);
  q.place([1, 8, 8], FACING_WEST.slice());
  q.move([0, 0, 2]);
  assert.ok(near(q.pos, [1, 8, 8], 1e-6), `bounced back: ${q.pos}`);
  assert.ok(q.mirrored);
});

test('blocks are solid, also when seen through a portal', () => {
  const g = emptyGrid(16);
  g.set(0, 8, 8, 1); // a block just inside the west face
  const space = new PortalSpace(preset('t3'), g);
  assert.ok(space.isSolid([0.5, 8.5, 8.5]));
  assert.ok(space.isSolid([16.5, 8.5, 8.5]), 'east of the east face is the block by the west face');
  const p = new Player(space);
  p.place([15, 8.5, 8.5], FACING_EAST.slice());
  p.move([0, 0, 3]);
  assert.equal(p.crossings, 0, 'blocked by the block on the other side');
});

test('every portal is an isometry that the matching portal undoes', () => {
  for (const { id, faces } of PRESETS) {
    const space = new PortalSpace(faces, emptyGrid(12));
    space.portals.forEach((h, i) => {
      if (!h) return;
      assert.equal(Math.abs(det(h.A)), 1);
      const back = space.portals[h.target];
      const x = [3.3, 7.1, 11.9];
      assert.ok(near(space.through(h.target, space.through(i, x)), x), `${id} face ${i}`);
      assert.deepEqual(matMul(back.A, h.A), IDENTITY);
    });
  }
});
