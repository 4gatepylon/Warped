// The menu: preset list, per-face gluing controls and the analysis of the current gluing.
// It owns no state of its own beyond the DOM: every edit produces a new gluing that is passed
// to onChange, and the app calls show() with whatever it decides the current gluing is.

import { FACE_COUNT, FACE_NAMES, FACE_NICKNAMES } from '../math/cube.js';
import {
  GLUE, WALL, MIRROR, SELF_FOLDS, withGlue, withBoundary, glueSelf, mapFromTwist, twistOf, selfFoldOf,
} from '../math/gluing.js';
import { PRESETS, PRESET_GROUPS, describe } from '../catalog.js';
import { faceColors, toCss } from '../palette.js';

const $ = (id) => document.getElementById(id);
const el = (tag, props = {}, children = []) => {
  const e = Object.assign(document.createElement(tag), props);
  for (const c of [].concat(children)) e.append(c);
  return e;
};
const faceLabel = (i) => `${FACE_NAMES[i]} ${FACE_NICKNAMES[i]}`;

export class Menu {
  /**
   * @param {object} handlers
   *   onChange(faces)  – the user edited the gluing
   *   onPreset(preset) – the user picked a preset
   *   onRandom()       – "surprise me"
   */
  constructor(handlers) {
    this.handlers = handlers;
    this.faces = null;
    this.buildPresetList();
    this.rows = this.buildFaceTable();
    $('randomize').addEventListener('click', () => handlers.onRandom());
  }

  buildPresetList() {
    const list = $('presetList');
    this.presetButtons = new Map();
    for (const group of PRESET_GROUPS) {
      const box = el('div', { className: 'group' }, el('div', { className: 'group-title', textContent: group }));
      for (const p of PRESETS.filter((q) => q.group === group)) {
        const b = el('button', { title: p.blurb }, [p.name + ' ', el('span', { className: 'math', textContent: p.math })]);
        b.addEventListener('click', () => this.handlers.onPreset(p));
        this.presetButtons.set(p.id, b);
        box.append(b);
      }
      list.append(box);
    }
  }

  buildFaceTable() {
    const body = $('faceTable').querySelector('tbody');
    return Array.from({ length: FACE_COUNT }, (_, i) => {
      const swatch = el('span', { className: 'swatch' });
      const target = el('select', { title: 'What happens when you go through this face' });
      target.append(el('option', { value: WALL, textContent: 'wall' }), el('option', { value: MIRROR, textContent: 'mirror' }));
      for (let j = 0; j < FACE_COUNT; j++) {
        target.append(el('option', { value: String(j), textContent: j === i ? 'itself (fold)' : faceLabel(j) }));
      }
      const twist = el('select', { title: 'Quarter turns before gluing' });
      ['0°', '90°', '180°', '270°'].forEach((t, k) => twist.append(el('option', { value: String(k), textContent: t })));
      const flip = el('input', { type: 'checkbox', title: 'Mirror the face left-to-right before gluing' });
      const fold = el('select', { title: 'How the face is folded onto itself' });
      for (const f of SELF_FOLDS) fold.append(el('option', { value: f.id, textContent: f.label }));

      const twistCell = el('td', {}, twist), flipCell = el('td', {}, flip), foldCell = el('td', { className: 'fold', colSpan: 2 }, fold);
      const row = el('tr', {}, [
        el('td', { className: 'face-name' }, [swatch, FACE_NAMES[i] + ' ', el('span', { className: 'nick', textContent: FACE_NICKNAMES[i] })]),
        el('td', {}, target), twistCell, flipCell, foldCell,
      ]);
      body.append(row);

      target.addEventListener('change', () => {
        const v = target.value;
        if (v === WALL || v === MIRROR) return this.edit(withBoundary(this.faces, i, v));
        const j = +v;
        if (j === i) return this.edit(glueSelf(this.faces, i, SELF_FOLDS[0].id));
        this.edit(withGlue(this.faces, i, j, mapFromTwist(i, j, 0, false)));
      });
      const retwist = () => {
        const j = this.faces[i].partner;
        this.edit(withGlue(this.faces, i, j, mapFromTwist(i, j, +twist.value, flip.checked)));
      };
      twist.addEventListener('change', retwist);
      flip.addEventListener('change', retwist);
      fold.addEventListener('change', () => this.edit(glueSelf(this.faces, i, fold.value)));
      return { row, swatch, target, twist, flip, fold, twistCell, flipCell, foldCell };
    });
  }

  edit(faces) { this.handlers.onChange(faces); }

  /** Show a gluing, its analysis and its name (from catalog.recognize). */
  show(faces, analysis, name) {
    this.faces = faces;
    const colors = faceColors(faces);
    faces.forEach((spec, i) => {
      const r = this.rows[i];
      r.swatch.style.background = toCss(colors[i]);
      const glued = spec.kind === GLUE, self = glued && spec.partner === i;
      r.target.value = glued ? String(spec.partner) : spec.kind;
      r.twistCell.hidden = r.flipCell.hidden = self;
      r.foldCell.hidden = !self;
      r.twist.disabled = r.flip.disabled = !glued;
      if (glued && !self) {
        const { twist, flip } = twistOf(i, spec.partner, spec.P);
        r.twist.value = String(twist);
        r.flip.checked = flip;
      } else {
        r.twist.value = '0';
        r.flip.checked = false;
      }
      if (self) r.fold.value = selfFoldOf(i, spec.P);
    });
    for (const [id, b] of this.presetButtons) b.classList.toggle('selected', name.preset?.id === id);
    this.showAnalysis(analysis, name);
  }

  showAnalysis(a, name) {
    $('resultTitle').textContent = name.title;
    $('resultMath').textContent = name.math || '';
    $('resultBlurb').textContent = name.preset?.blurb || (name.exact
      ? 'A flat 3-manifold: Gluebox recognises it from its invariants.'
      : 'A gluing of your own. Its invariants are below.');

    const summary = describe(a);
    $('resultChips').replaceChildren(...summary.chips.map((c) => el('span', { className: `chip ${c.tone}`, textContent: c.text })));

    const facts = [
      ['H₁', summary.H1],
      ['χ', String(summary.euler)],
      ['cells', `${a.counts.vertices} vertices, ${a.counts.edges} edges, ${a.counts.faces} triangles, 1 cube (after cutting each face into 8 triangles)`],
    ];
    if (a.boundary.length) facts.push(['boundary', a.boundary.map((b) => b.name + (b.mirror ? ' (mirrored)' : '')).join(' + ')]);
    $('resultFacts').replaceChildren(...facts.flatMap(([k, v]) => [el('dt', { textContent: k }), el('dd', { textContent: v })]));

    const notes = [];
    if (!a.manifold) {
      const kinds = new Map();
      for (const p of a.badPoints) kinds.set(p.link, (kinds.get(p.link) || 0) + 1);
      for (const [link, count] of kinds) {
        notes.push({ warn: true, text: `${count} point${count > 1 ? 's' : ''} where space is pinched: the neighbourhood is a cone on a ${link}, not a ball.` });
      }
    }
    if (!a.flat) {
      const angles = [...new Set(a.singularEdges.map((c) => c.angle * 90))].sort((x, y) => x - y).map((d) => `${d}°`);
      notes.push({ text: `Around the dashed edges the cube's corners add up to ${angles.join(' or ')} instead of 360°. A flat cube can't make this space smoothly, so you'll see doubled or missing wedges near them.` });
    }
    if (!a.orientable && a.manifold) notes.push({ text: 'Non-orientable: some trips bring you back as your own mirror image.' });
    if (!a.closed) notes.push({ text: 'Walls are solid; mirrors bounce you (and light) back.' });
    if (name.twin) notes.push({ text: `This gluing has the same invariants as the ${name.twin.name} preset, so it may well be the same space, but Gluebox can't prove it.` });
    $('resultNotes').replaceChildren(...notes.map((n) => el('li', { className: n.warn ? 'warn' : '', textContent: n.text })));
  }
}
