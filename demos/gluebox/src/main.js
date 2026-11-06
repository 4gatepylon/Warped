// Gluebox: wires the menu (choose a gluing) to the world (fly around inside it).

import { FACE_COUNT, FACE_NAMES, FACE_NICKNAMES, describeLinear, column } from './math/cube.js';
import { describeGeometry, MIN_CORNER, MAX_CORNER } from './math/geometry.js';
import { encode, decode, emptyGluing, withGlue, withBoundary, mapFromTwist, glueSelf, SELF_FOLDS, sameGluing } from './math/gluing.js';
import { analyze } from './math/topology.js';
import { PRESETS, recognize } from './catalog.js';
import { buildScene, CUBE_SIZES, DEFAULT_CUBE_SIZE } from './world/scene.js';
import { PortalSpace } from './world/space.js';
import { Player } from './world/player.js';
import { WorldRenderer } from './world/renderer.js';
import { Input } from './world/input.js';
import { Menu } from './menu/menu.js';
import { CubePreview } from './menu/preview.js';

const $ = (id) => document.getElementById(id);

const GENERIC_TIPS = [
  'Look around: every copy of the island you see is the same island, seen along a different path through the glued faces.',
  'Watch the holonomy readout as you fly through faces: it is how the space has turned (or mirrored) you so far.',
  'Turn on auto-fly (F) and let a straight line show you how the space closes up.',
];

const app = {
  mode: 'menu',
  faces: PRESETS[0].faces,
  corner: 90,         // the cube's corner angle: 90° flat, more spherical, less hyperbolic
  analysis: null,
  name: null,
  options: { size: DEFAULT_CUBE_SIZE, fog: 2.5, frame: true, seams: true, self: true, quality: 'auto' },
  world: null,        // { faces, corner, size, space, player } while a world exists
};

let renderer, preview, menu, input;

function init() {
  try {
    renderer = new WorldRenderer($('world'));
    preview = new CubePreview($('preview'));
  } catch (e) {
    $('fatal').hidden = false;
    $('fatal').textContent = `Gluebox needs WebGL 2: ${e.message}`;
    throw e;
  }
  menu = new Menu({
    onChange: (faces) => setGluing(faces),
    onPreset: (p) => setGluing(p.faces, p.corner ?? 90),
    onRandom: () => setGluing(randomGluing()),
    onCorner: (corner) => setGluing(app.faces, corner),
  });
  input = new Input($('world'), { exit: showMenu, key: onKey });
  bindOptions();
  $('enter').addEventListener('click', enterWorld);
  $('hudMenu').addEventListener('click', showMenu);
  $('tipsToggle').addEventListener('click', () => {
    const list = $('tipsList');
    list.hidden = !list.hidden;
    $('tipsToggle').textContent = list.hidden ? 'show' : 'hide';
  });
  window.addEventListener('resize', () => renderer.resize());
  window.addEventListener('hashchange', loadFromHash);
  loadFromHash();
  requestAnimationFrame(frame);
  // handle for debugging from the console
  window.__gluebox = { app, enterWorld, showMenu, setGluing, renderer, input, get player() { return app.world?.player; } };
}

// ---------------------------------------------------------------------------
// Gluing state
// ---------------------------------------------------------------------------
function setGluing(faces, corner = app.corner) {
  app.faces = faces;
  app.corner = Math.min(MAX_CORNER, Math.max(MIN_CORNER, corner));
  app.analysis = analyze(faces);
  app.name = recognize(faces, app.analysis, analyze);
  menu.show(faces, app.analysis, app.name, app.corner);
  preview.update(faces, app.analysis, app.corner);
  // URL hash: preset id or gluing code, plus "@angle" when the corners aren't the default
  const preset = app.name.preset && sameGluing(app.name.preset.faces, faces) ? app.name.preset : null;
  const base = preset ? preset.id : encode(faces);
  const hash = app.corner === (preset?.corner ?? 90) ? base : `${base}@${app.corner}`;
  if (location.hash.slice(1) !== hash) history.replaceState(null, '', `#${hash}`);
}

function loadFromHash() {
  const [h, angle] = decodeURIComponent(location.hash.slice(1)).split('@');
  const preset = PRESETS.find((p) => p.id === h);
  const corner = angle ? +angle : preset?.corner ?? 90;
  try {
    setGluing(preset ? preset.faces : h ? decode(h) : PRESETS[0].faces, Number.isFinite(corner) ? corner : 90);
  } catch (e) {
    setGluing(PRESETS[0].faces, 90);
  }
}

function randomGluing() {
  const rand = (n) => Math.floor(Math.random() * n);
  let faces = emptyGluing();
  const order = [...Array(FACE_COUNT).keys()].sort(() => Math.random() - 0.5);
  while (order.length) {
    const i = order.pop();
    const roll = Math.random();
    if (roll < 0.08) faces = withBoundary(faces, i, Math.random() < 0.5 ? 'mirror' : 'wall');
    else if (roll < 0.2 || !order.length) faces = glueSelf(faces, i, SELF_FOLDS[rand(SELF_FOLDS.length)].id);
    else {
      const j = order.splice(rand(order.length), 1)[0];
      faces = withGlue(faces, i, j, mapFromTwist(i, j, rand(4), Math.random() < 0.3));
    }
  }
  return faces;
}

function bindOptions() {
  const o = app.options;
  const size = $('optSize');
  for (const n of CUBE_SIZES) size.append(new Option(`${n} blocks`, String(n), n === o.size, n === o.size));
  size.addEventListener('change', () => { o.size = +size.value; });
  $('optFog').addEventListener('change', (e) => { o.fog = +e.target.value; });
  $('optFrame').addEventListener('change', (e) => { o.frame = e.target.checked; });
  $('optSeams').addEventListener('change', (e) => { o.seams = e.target.checked; });
  $('optSelf').addEventListener('change', (e) => { o.self = e.target.checked; });
  $('optQuality').addEventListener('change', (e) => { o.quality = e.target.value; renderer.setQuality(o.quality); });
}

// ---------------------------------------------------------------------------
// Switching between menu and world
// ---------------------------------------------------------------------------
function enterWorld() {
  const o = app.options;
  const w = app.world;
  const same = w && w.size === o.size && w.corner === app.corner && sameGluing(w.faces, app.faces);
  if (!same) {
    const grid = w && w.size === o.size ? w.space.grid : buildScene(o.size);
    const space = new PortalSpace(app.faces, grid, app.corner);
    app.world = { faces: app.faces, corner: app.corner, size: o.size, space, player: new Player(space) };
    renderer.setWorld(space, app.analysis);
  }
  renderer.setOptions({ frame: o.frame, seams: o.seams, self: o.self, fog: o.fog * o.size });
  showTips();
  $('hudTitle').textContent = app.name.title;
  $('hudMath').textContent = [app.name.math, describeGeometry(app.world.space.geometry)].filter(Boolean).join(' · ');
  app.mode = 'world';
  $('menu').hidden = true;
  $('hud').hidden = false;
  preview.setActive(false);
  input.start();
}

function showMenu() {
  if (app.mode === 'menu') return;
  app.mode = 'menu';
  input.stop();
  $('hud').hidden = true;
  $('menu').hidden = false;
  preview.setActive(true);
}

function showTips() {
  const tips = app.name.preset?.tips || GENERIC_TIPS;
  $('tipsList').replaceChildren(...tips.map((t) => Object.assign(document.createElement('li'), { textContent: t })));
}

function onKey(code) {
  const w = app.world;
  if (!w) return;
  if (code === 'KeyF') w.autoFly = !w.autoFly;
  if (code === 'KeyR') { w.player.reset(); w.autoFly = false; }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------
let last = performance.now(), hudTimer = 0, frames = 0, fpsStart = performance.now();

function frame(now) {
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  if (app.mode === 'world' && app.world) {
    stepPlayer(dt);
    renderer.render(app.world.player, app.world.space);
    frames++;
    if ((hudTimer += dt) > 0.2) { updateHud(now); hudTimer = 0; }
  }
  requestAnimationFrame(frame);
}

function stepPlayer(dt) {
  const { player } = app.world;
  const [mx, my] = input.takeMouse();
  player.look(mx, my);
  const k = (...c) => (input.held(...c) ? 1 : 0);
  let forward = k('KeyW', 'ArrowUp') - k('KeyS', 'ArrowDown');
  const right = k('KeyD') - k('KeyA');
  const up = k('KeyE', 'Space') - k('KeyQ');
  if (app.world.autoFly && !forward && !right) forward = 1;
  const turn = k('ArrowRight') - k('ArrowLeft');
  if (turn) player.turn(turn * 2 * dt);
  const len = Math.hypot(forward, right, up);
  if (len) {
    const speed = (input.held('ShiftLeft', 'ShiftRight') ? 16 : 6) * dt / len;
    player.move([right * speed, up * speed, forward * speed]);
  }
}

const AXES = ['x', 'y', 'z'];
function updateHud(now) {
  const p = app.world.player;
  $('sCross').textContent = p.crossings;
  $('sLast').textContent = p.lastFace >= 0 ? `(last: ${FACE_NAMES[p.lastFace]} ${FACE_NICKNAMES[p.lastFace]})` : '';
  $('sHand').textContent = p.mirrored ? 'a mirror image' : 'the right way round';
  const up = column(p.body, 1);
  const k = [0, 1, 2].reduce((b, i) => (Math.abs(up[i]) > Math.abs(up[b]) ? i : b), 0);
  $('sUp').textContent = `${up[k] < 0 ? '−' : '+'}${AXES[k]}${Math.abs(up[k]) < 0.9 ? ' (roughly)' : ''}`;
  $('sHol').textContent = describeLinear(p.holonomy);
  $('sFps').textContent = (frames * 1000 / (now - fpsStart)).toFixed(0);
  frames = 0; fpsStart = now;
}

init();
