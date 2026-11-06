// The turnable cube in the menu: each face painted with its pair colour and F glyphs, each
// edge coloured by which edges end up glued together.

import * as THREE from 'three';
import { FACE_COUNT, FACE_NAMES, FACE_NICKNAMES, faceNormal, faceFrame } from '../math/cube.js';
import { cubeEdgeClasses } from '../math/topology.js';
import { glyphMaps } from '../world/renderer.js';
import { faceColors, edgeColor, toCss } from '../palette.js';

const TEX = 512;
const GLYPH_AT = [0.55, 0.22], GLYPH_SIZE = 0.2; // keep in sync with shader.js

export class CubePreview {
  constructor(canvas) {
    this.canvas = canvas;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    this.scene = new THREE.Scene();
    this.camera = new THREE.PerspectiveCamera(32, 1, 0.1, 50);
    this.camera.position.set(0, 0, 8.2);
    this.cube = new THREE.Group();
    this.cube.rotation.set(0.45, -0.65, 0);
    this.scene.add(this.cube);
    this.faces = this.makeFaces();
    this.edgeGroup = new THREE.Group();
    this.cube.add(this.edgeGroup);
    this.addAxes();
    this.enableDragging();
    this.running = false;
    this.setActive(true);
  }

  makeFaces() {
    return Array.from({ length: FACE_COUNT }, (_, i) => {
      const canvas = document.createElement('canvas');
      canvas.width = canvas.height = TEX;
      const texture = new THREE.CanvasTexture(canvas);
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.anisotropy = 4;
      const mesh = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), new THREE.MeshBasicMaterial({ map: texture }));
      const n = new THREE.Vector3(...faceNormal(i)), { u, v } = faceFrame(i);
      mesh.matrixAutoUpdate = false;
      mesh.matrix.makeBasis(new THREE.Vector3(...u), new THREE.Vector3(...v), n).setPosition(n);
      this.cube.add(mesh);
      return { canvas, texture, mesh };
    });
  }

  // Small x/y/z arrows from a corner of the cube, so face names can be checked.
  addAxes() {
    const origin = new THREE.Vector3(-1.25, -1.25, -1.25);
    [[1, 0, 0, 0xff5b5b], [0, 1, 0, 0x4fdc6a], [0, 0, 1, 0x4d8dff]].forEach(([x, y, z, color]) => {
      this.cube.add(new THREE.ArrowHelper(new THREE.Vector3(x, y, z), origin, 0.7, color, 0.18, 0.1));
    });
  }

  /** Repaint for a new gluing, its analysis (from topology.js) and the cube's corner angle. */
  update(faces, analysis, corner = 90) {
    const colors = faceColors(faces);
    faces.forEach((spec, i) => {
      paintFace(this.faces[i].canvas.getContext('2d'), i, spec, colors[i], glyphMaps(faces, i));
      this.faces[i].texture.needsUpdate = true;
    });
    this.rebuildEdges(analysis, corner);
  }

  rebuildEdges(analysis, corner) {
    for (const child of [...this.edgeGroup.children]) { child.geometry.dispose(); child.material.dispose(); this.edgeGroup.remove(child); }
    cubeEdgeClasses(analysis, corner).forEach((h, idx) => {
      const edge = idx >> 1, half = idx & 1, axis = edge >> 2, a = (axis + 1) % 3, b = (axis + 2) % 3;
      const mid = [0, 0, 0];
      mid[a] = (edge >> 1) & 1 ? 1 : -1; mid[b] = edge & 1 ? 1 : -1;
      const end = mid.slice(); end[axis] = half ? 1 : -1;
      const color = new THREE.Color(...edgeColor(h.full, h.ends === 2 ? h.end : 0.5));
      const pieces = h.singular ? [[0, 0.2], [0.4, 0.6], [0.8, 1]] : [[0, 1]];
      for (const [s, t] of pieces) {
        const p = new THREE.Vector3(...mid).lerp(new THREE.Vector3(...end), s);
        const q = new THREE.Vector3(...mid).lerp(new THREE.Vector3(...end), t);
        this.edgeGroup.add(cylinder(p, q, 0.045, color));
      }
    });
  }

  enableDragging() {
    let last = null;
    this.canvas.addEventListener('pointerdown', (e) => { last = [e.clientX, e.clientY]; this.canvas.setPointerCapture(e.pointerId); });
    this.canvas.addEventListener('pointermove', (e) => {
      if (!last) return;
      const dx = e.clientX - last[0], dy = e.clientY - last[1];
      last = [e.clientX, e.clientY];
      const q = new THREE.Quaternion()
        .setFromAxisAngle(new THREE.Vector3(0, 1, 0), dx * 0.01)
        .multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), dy * 0.01));
      this.cube.quaternion.premultiply(q);
    });
    const stop = () => { last = null; };
    this.canvas.addEventListener('pointerup', stop);
    this.canvas.addEventListener('pointercancel', stop);
  }

  resize() {
    const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
    if (!w || !h) return;
    if (this.canvas.width !== Math.round(w * this.renderer.getPixelRatio()) || this.canvas.height !== Math.round(h * this.renderer.getPixelRatio())) {
      this.renderer.setSize(w, h, false);
      this.camera.aspect = w / h;
      this.camera.position.z = 8.2 * Math.max(1, 0.9 / this.camera.aspect);
      this.camera.updateProjectionMatrix();
    }
  }

  /** Pause rendering while the menu is hidden. */
  setActive(active) {
    if (active === this.running) return;
    this.running = active;
    const loop = () => {
      if (!this.running) return;
      this.resize();
      this.renderer.render(this.scene, this.camera);
      requestAnimationFrame(loop);
    };
    if (active) requestAnimationFrame(loop);
  }
}

function cylinder(p, q, r, color) {
  const dir = new THREE.Vector3().subVectors(q, p);
  const geo = new THREE.CylinderGeometry(r, r, dir.length(), 10);
  const mesh = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color }));
  mesh.position.copy(p).add(q).multiplyScalar(0.5);
  mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.normalize());
  return mesh;
}

// Draw one face. Canvas pixel (px, py) is face-frame point (u, v) = (2px/S - 1, 1 - 2py/S).
function paintFace(ctx, i, spec, rgb, glyphs) {
  const S = TEX;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  if (spec.kind === 'mirror') {
    const g = ctx.createLinearGradient(0, 0, S, S);
    g.addColorStop(0, '#dfe8f5'); g.addColorStop(0.5, '#9fb2cc'); g.addColorStop(1, '#e8eef8');
    ctx.fillStyle = g;
  } else {
    ctx.fillStyle = toCss(rgb, spec.kind === 'wall' ? 0.75 : 1);
  }
  ctx.fillRect(0, 0, S, S);
  // grid
  ctx.strokeStyle = 'rgba(0,0,0,0.12)';
  ctx.lineWidth = 2;
  for (let k = 1; k < 8; k++) {
    ctx.beginPath(); ctx.moveTo((k * S) / 8, 0); ctx.lineTo((k * S) / 8, S); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(0, (k * S) / 8); ctx.lineTo(S, (k * S) / 8); ctx.stroke();
  }
  ctx.strokeStyle = 'rgba(0,0,0,0.35)'; ctx.lineWidth = 6; ctx.strokeRect(3, 3, S - 6, S - 6);
  // label
  ctx.fillStyle = 'rgba(0,0,0,0.55)';
  ctx.font = 'bold 34px ui-monospace, Menlo, monospace';
  ctx.textAlign = 'left'; ctx.textBaseline = 'top';
  ctx.fillText(`${FACE_NAMES[i]} ${FACE_NICKNAMES[i]}`, 18, 16);
  if (spec.kind !== 'glue') {
    ctx.font = 'bold 44px ui-monospace, Menlo, monospace';
    ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.fillStyle = 'rgba(0,0,0,0.35)';
    ctx.fillText(spec.kind === 'wall' ? 'WALL' : 'MIRROR', S / 2, S / 2);
    return;
  }
  // F glyphs, each drawn through its 2x2 face map M: canvas = T ∘ M (u, v)
  ctx.fillStyle = toCss(rgb, 0.35);
  for (const [a, b, c, d] of glyphs) {
    ctx.setTransform((S / 2) * a, -(S / 2) * c, (S / 2) * b, -(S / 2) * d, S / 2, S / 2);
    drawF(ctx);
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
}

// The letter F in face coordinates (v up), matching inGlyph() in shader.js.
function drawF(ctx) {
  const [ox, oy] = GLYPH_AT, s = GLYPH_SIZE;
  const rect = (x0, x1, y0, y1) => ctx.fillRect(ox + s * x0, oy + s * y0, s * (x1 - x0), s * (y1 - y0));
  rect(-0.5, -0.2, -0.8, 0.8);
  rect(-0.5, 0.6, 0.5, 0.8);
  rect(-0.5, 0.3, -0.05, 0.25);
}
