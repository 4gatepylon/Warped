import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { bundle, BUNDLE_PATH } from '../tools/build.mjs';

test('gluebox.bundle.js (used when index.html is opened from disk) matches src/', () => {
  assert.equal(readFileSync(BUNDLE_PATH, 'utf8'), bundle(), 'out of date: run `npm run build`');
});

test('the bundle is a valid classic script that loads three.js from the import map URL', () => {
  const text = bundle();
  assert.doesNotThrow(() => new vm.Script(text));
  assert.ok(!/^\s*(import|export)\s/m.test(text), 'no static import/export left');
  assert.match(text, /await import\("https:\/\/cdn\.jsdelivr\.net\/npm\/three@[\d.]+\/build\/three\.module\.js"\)/);
  // every module appears once, dependencies first
  const order = [...text.matchAll(/^\/\/ ---- src\/(.+)$/gm)].map((m) => m[1]);
  assert.equal(new Set(order).size, order.length);
  assert.ok(order.indexOf('math/cube.js') < order.indexOf('math/gluing.js'));
  assert.equal(order.at(-1), 'main.js');
});
