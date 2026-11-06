// Try every way of gluing all six faces of the cube (in pairs, or folded onto themselves) and
// tabulate what comes out. This is where the "which spaces can one cube make" list in the
// README comes from.
//
//   node tools/enumerate.mjs            # pairs only (7,680 gluings, a second or two)
//   node tools/enumerate.mjs --folds    # also faces folded onto themselves (170,305, ~1 min)

import { emptyGluing, withGlue, glueSelf, mapFromTwist, SELF_FOLDS, encode } from '../src/math/gluing.js';
import { analyze, smoothCorner } from '../src/math/topology.js';
import { MIN_CORNER, MAX_CORNER } from '../src/math/geometry.js';
import { formatGroup } from '../src/math/homology.js';
import { recognize } from '../src/catalog.js';

const withFolds = process.argv.includes('--folds');

function* gluings(faces, i) {
  while (i < 6 && faces[i].kind === 'glue') i++;
  if (i === 6) { yield faces; return; }
  if (withFolds) for (const f of SELF_FOLDS) yield* gluings(glueSelf(faces, i, f.id), i + 1);
  for (let j = i + 1; j < 6; j++) {
    if (faces[j].kind === 'glue') continue;
    for (let twist = 0; twist < 4; twist++) {
      for (const flip of [false, true]) yield* gluings(withGlue(faces, i, j, mapFromTwist(i, j, twist, flip)), i + 1);
    }
  }
}

const table = new Map();
let total = 0;
for (const faces of gluings(emptyGluing(), 0)) {
  total++;
  const a = analyze(faces);
  // a manifold that isn't flat may still be smooth on a curved cube (see math/geometry.js)
  const corner = smoothCorner(a);
  const curvedSmooth = corner !== null && corner !== 90 && corner >= MIN_CORNER && corner <= MAX_CORNER;
  const kind = !a.manifold ? 'not a manifold'
    : a.flat ? 'flat manifold'
      : curvedSmooth ? `smooth at ${corner}° corners` : 'manifold, singular edges';
  const key = `${kind.padEnd(25)} ${(a.orientable ? 'orientable' : 'non-orientable').padEnd(15)} H1 = ${formatGroup(a.H1)}`;
  if (!table.has(key)) {
    const name = a.manifold && (a.flat || curvedSmooth) ? recognize(faces, a).title : '';
    table.set(key, { count: 0, example: encode(faces), name });
  }
  table.get(key).count++;
}

console.log(`${total} gluings of all six faces${withFolds ? ' (including folds)' : ''}\n`);
for (const [key, { count, example, name }] of [...table].sort()) {
  console.log(`${key.padEnd(62)} ${String(count).padStart(7)}  e.g. ${example.padEnd(24)} ${name}`);
}
