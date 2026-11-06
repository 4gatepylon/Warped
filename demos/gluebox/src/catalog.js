// Named spaces you can build from one cube, and how to recognise a gluing as one of them.

import { CUBE_SYMMETRIES } from './math/cube.js';
import { decode, conjugate, sameGluing } from './math/gluing.js';
import { groupKey, formatGroup } from './math/homology.js';
import { singularSummary, singularCycles } from './math/topology.js';

// Gluing codes: one token per face (+x, −x, +y, −y, +z, −z). "w" wall, "m" mirror, otherwise
// <partner face><quarter turns of twist>[f = mirror flip]. See gluing.js.
export const PRESETS = [
  // --- The flat 3-manifolds ("platycosms"). Nine of the ten can be made from a single cube.
  {
    id: 't3', group: 'Flat universes', name: '3-torus', math: 'T³ (torocosm)', code: '10-00-30-20-50-40',
    blurb: 'Every pair of opposite faces is glued straight across. The 3D version of a video game screen that wraps around.',
    tips: [
      'Look in any direction: the island repeats forever on a cubic grid, every copy facing the same way.',
      'Fly out of any face and you come straight back in through the opposite one.',
      'Look straight ahead and wait: the figure in the distance is you, seen from behind.',
    ],
  },
  {
    id: 'dicosm', group: 'Flat universes', name: 'Half-turn space', math: 'dicosm', code: '10-00-30-20-52-42',
    blurb: 'Like the 3-torus, but north and south are glued with a half turn.',
    tips: [
      'Look north: every other island is rotated 180° about the north–south line, so it hangs upside down.',
      'Fly out through the north wall and you come back in upside down. Do it twice and you are the right way up again.',
      'East–west and up–down still wrap straight across.',
    ],
  },
  {
    id: 'tetracosm', group: 'Flat universes', name: 'Quarter-turn space', math: 'tetracosm', code: '10-00-30-20-51-41',
    blurb: 'North and south are glued with a quarter turn.',
    tips: [
      'Look north: each copy is turned a further 90°, so it takes four copies to line up again.',
      'Fly north once and "up" now points sideways. Watch the holonomy readout.',
    ],
  },
  {
    id: 'tricosm', group: 'Flat universes', name: 'Third-turn space', math: 'tricosm', code: '21-51-01-41-31-11',
    blurb: 'Adjacent faces are glued in pairs so that the cube turns 120° about its long diagonal. Usually built from a hexagonal prism, but a cube works too.',
    tips: [
      'Fly through any face: the red, green and blue arms of the axis marker trade places.',
      'Three trips through the same kind of face bring you back to your original orientation.',
    ],
  },
  {
    id: 'didicosm', group: 'Flat universes', name: 'Hantzsche–Wendt space', math: 'didicosm', code: '12-02-42-52-22-32',
    blurb: 'East–west glued with a half turn; the floor is glued to the north wall and the ceiling to the south wall, also with half turns. The only flat space with finite first homology: there is no direction that simply wraps around.',
    tips: [
      'Every trip through a face comes back turned half way round, about a different axis for each pair of faces.',
      'Fly east twice and you are the right way up again: two half turns about the same axis cancel.',
    ],
  },
  {
    id: 'amphicosm1', group: 'Flat universes', name: 'Klein bottle × circle', math: 'first amphicosm', code: '10-00-30-20-50f-40f',
    blurb: 'North and south are glued with a mirror flip. The space has no consistent left and right.',
    tips: [
      'Fly out through north: you come back as your own mirror image. Check the axis marker: it is now left-handed.',
      'Look north: every other copy of the island is a mirror image (look at the chimney and the door).',
      'Go through again and you are back to normal.',
    ],
  },
  {
    id: 'amphicosm2', group: 'Flat universes', name: 'Second amphicosm', math: 'second amphicosm', code: '10-00-30-20-51f-43f',
    blurb: 'North and south are glued with a mirror flip across a diagonal of the face, so a trip north or south swaps east–west with up–down, and mirrors you.',
    tips: [
      'Fly north: the floor is now on your right, and the axis marker looks left-handed.',
      'Compare with Klein bottle × circle: the mirror now runs diagonally, which changes the topology (H₁ loses its ℤ/2).',
    ],
  },
  {
    id: 'amphidicosm1', group: 'Flat universes', name: 'First amphidicosm', math: 'first amphidicosm', code: '10-00-30f-20f-52-42',
    blurb: 'Floor–ceiling glued with a mirror flip, north–south with a half turn.',
    tips: ['Going up mirrors you; going north turns you upside down. Combine the two and see what the holonomy says.'],
  },
  {
    id: 'amphidicosm2', group: 'Flat universes', name: 'Second amphidicosm', math: 'second amphidicosm', code: '11f-03f-30f-20f-50f-40f',
    blurb: 'All three pairs are glued with mirror flips, and east–west flips across a diagonal.',
    tips: ['Every one of the six faces mirrors you.'],
  },

  // --- Closed spaces whose natural geometry is not flat. Made from a flat cube they have edges
  // where the angles do not add up to 360°.
  {
    id: 's3', group: 'Curved spaces', name: '3-sphere', math: 'S³', code: '02f-12f-30-20-42f-52f',
    blurb: 'Floor glued to ceiling, and each side wall folded onto itself along its horizontal midline. That zips the surface of the cube shut like a purse, which gives the 3-sphere.',
    tips: [
      'Fly into a side wall just above its midline: you come back out just below it, heading back into the room upside down.',
      'There is no repeating grid here: the space is finite and every loop can be shrunk to a point.',
      'Collapsing the whole surface of the cube to a single point also gives S³, but that cannot be walked through, so Gluebox uses this zip instead.',
    ],
  },
  {
    id: 'rp3', group: 'Curved spaces', name: 'Projective space', math: 'RP³', code: '12-02-32-22-52-42',
    blurb: 'Every face is glued to the opposite face with a half turn, so each point on the surface is glued to the point directly opposite it through the centre.',
    tips: [
      'Fly out of any face: you come back in through the opposite face, turned half way round.',
      'The total angle around every cube edge is only 180°, so things look oddly doubled near the edges.',
    ],
  },
  {
    id: 'quaternion', group: 'Curved spaces', name: 'Quaternionic space', math: 'S³/Q₈', code: '11-01-31-21-51-41', corner: 120,
    blurb: 'All three pairs of opposite faces are glued with a quarter turn in the same direction. On a spherical cube with 120° corners (eight of which fill the 3-sphere) this is perfectly smooth.',
    tips: [
      'Space here is curved like the 3-sphere: distant islands shrink, then grow again.',
      'Set the curvature back to flat in the menu: the cube edges turn dashed, because only 270° of cube meets around each one.',
    ],
  },
  {
    id: 'lens83', group: 'Curved spaces', name: 'Lens space', math: 'L(8,3)', code: '11-01-43-53-23-33', corner: 120,
    blurb: 'East–west glued with a quarter turn; the ceiling is glued to the south wall and the floor to the north wall, with three-quarter turns. On a 120° spherical cube this is smooth: the lens space L(8,3), the 3-sphere wrapped around eight times.',
    tips: ['Fly in a straight line for about 80 blocks (once round the 3-sphere): you are back where you started, the right way round. Some directions bring you back sooner.'],
  },
  {
    id: 's2xs1', group: 'Curved spaces', name: 'Sphere × circle', math: 'S² × S¹', code: '02f-12f-30-20-50-40',
    blurb: 'Floor–ceiling and north–south glued straight across; the east and west walls are each folded onto themselves.',
    tips: ['Compare with the 3-sphere: only the north and south walls changed, from folded to straight across.'],
  },

  // --- Spaces with a boundary you can bump into.
  {
    id: 'solid-torus', group: 'With walls', name: 'Solid torus', math: 'D² × S¹', code: 'w-w-w-w-50-40',
    blurb: 'Only north and south are glued; the other four faces are solid walls. An endless corridor.',
    tips: ['Look north or south: an endless tunnel of islands.'],
  },
  {
    id: 't2xi', group: 'With walls', name: 'Thickened torus', math: 'T² × I', code: '10-00-w-w-50-40',
    blurb: 'A floor and a ceiling, with the side walls glued straight across: a flat world that wraps around in both directions.',
    tips: ['Fly up to the ceiling and look around: an infinite field of islands.'],
  },
  {
    id: 'kxi', group: 'With walls', name: 'Klein bottle × interval', math: 'K × I', code: '10-00-w-w-50f-40f',
    blurb: 'A floor-and-ceiling world where going north–south mirrors you.',
    tips: ['Walk north: the islands in that direction alternate between normal and mirror image.'],
  },
  {
    id: 'solid-klein', group: 'With walls', name: 'Solid Klein bottle', math: 'D² ×~ S¹', code: 'w-w-w-w-50f-40f',
    blurb: 'An endless corridor that mirrors you every time you go around.',
    tips: ['Its boundary (the walls) is a Klein bottle.'],
  },

  // --- Mirrors and spaces that are not manifolds.
  {
    id: 'kaleidoscope', group: 'Mirrors & oddities', name: 'Kaleidoscope', math: 'B³ (mirrored)', code: 'm-m-m-m-m-m',
    blurb: 'Six mirrors. Topologically just a ball, but light bounces forever.',
    tips: ['The lattice looks like the 3-torus, except neighbouring copies are mirror images.'],
  },
  {
    id: 'inversions', group: 'Mirrors & oddities', name: 'Inversion box', math: 'not a manifold', code: '02-12-22-32-42-52',
    blurb: 'Every face is glued to itself with a half turn about its centre. The six face centres, and the two points where the corners end up, do not look like ordinary space: their neighbourhoods are cones on a projective plane.',
    tips: ['Each face reflects you through its centre point, which mirrors you in all three directions at once.'],
  },
].map((p) => ({ ...p, faces: decode(p.code) }));

export const PRESET_GROUPS = [...new Set(PRESETS.map((p) => p.group))];

// The ten closed flat 3-manifolds are told apart by orientability and first homology.
const PLATYCOSMS = {
  'or Z3': 'the 3-torus', 'or Z1+2+2': 'the half-turn space (dicosm)', 'or Z1+3': 'the third-turn space (tricosm)',
  'or Z1+2': 'the quarter-turn space (tetracosm)', 'or Z1': 'the sixth-turn space (hexacosm)',
  'or Z0+4+4': 'the Hantzsche–Wendt space (didicosm)',
  'non Z2+2': 'Klein bottle × circle (first amphicosm)', 'non Z2': 'the second amphicosm',
  'non Z1+2+2': 'the first amphidicosm', 'non Z1+4': 'the second amphidicosm',
};

/** Which preset is this gluing, up to rotating or reflecting the cube? */
export function matchPreset(faces) {
  for (const p of PRESETS) {
    for (const g of CUBE_SYMMETRIES) if (sameGluing(conjugate(p.faces, g), faces)) return p;
  }
  return null;
}

const signature = (a) => [a.closed, a.orientable, a.manifold, groupKey(a.H1), a.boundary.map((b) => b.name).sort().join('+')].join('|');

let presetSignatures = null;

/**
 * Name a gluing from its analysis (see topology.js). Returns { title, exact, detail } where
 * exact says whether the name is certain (a known preset, or a flat closed manifold, which are
 * classified) or only a guess from matching invariants.
 */
export function recognize(faces, analysis, analyzeFn) {
  const preset = matchPreset(faces);
  if (preset) return { title: preset.name, math: preset.math, exact: true, preset };
  if (analysis.closed && analysis.manifold && analysis.flat) {
    const k = `${analysis.orientable ? 'or' : 'non'} ${groupKey(analysis.H1)}`;
    if (PLATYCOSMS[k]) return { title: capitalise(PLATYCOSMS[k]), math: 'flat 3-manifold', exact: true };
  }
  if (analyzeFn) {
    presetSignatures ||= new Map(PRESETS.map((p) => [signature(analyzeFn(p.faces)), p]));
    const twin = presetSignatures.get(signature(analysis));
    if (twin && analysis.manifold) {
      return { title: `Unnamed space`, math: `same invariants as ${twin.math}`, exact: false, twin };
    }
  }
  return { title: 'Unnamed space', math: analysis.manifold ? '' : 'not a manifold', exact: false };
}

const capitalise = (s) => s.replace(/^the /, '').replace(/^./, (c) => c.toUpperCase());

/** Summary of an analysis for the menu, for a cube with the given corner angle: chips, H₁, χ. */
export function describe(analysis, corner = 90) {
  const a = analysis;
  const sing = singularSummary(a, corner);
  const smooth = singularCycles(a, corner).length === 0;
  const geometry = corner === 90 ? 'flat' : corner > 90 ? 'spherical' : 'hyperbolic';
  const edges = `${sing.cubeEdges} cube edge${sing.cubeEdges === 1 ? '' : 's'}`;
  const chips = [
    { text: a.closed ? 'closed' : 'has walls', tone: a.closed ? 'good' : '' },
    { text: a.manifold ? 'manifold' : 'not a manifold', tone: a.manifold ? 'good' : 'bad' },
    { text: a.orientable ? 'orientable' : 'non-orientable', tone: '' },
    smooth
      ? { text: `${geometry} everywhere`, tone: 'good' }
      : { text: `angles off along ${edges}${sing.foldLines ? ' + fold lines' : ''}`, tone: 'warn' },
  ];
  return { chips, H1: formatGroup(a.H1), euler: a.euler };
}
