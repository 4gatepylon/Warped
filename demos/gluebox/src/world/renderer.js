// three.js plumbing for the ray tracer: one full-screen triangle pair with the shader from
// shader.js, a 3D texture holding the blocks, and uniforms describing the gluing and the
// geometry (flat, spherical or hyperbolic).

import * as THREE from 'three';
import { vertexShader, fragmentShader, MAX_SELF_IMAGES } from './shader.js';
import { MATERIALS } from './scene.js';
import { FACE_COUNT, faceFrame, faceMap2D, fromColumns } from '../math/cube.js';
import { GLUE } from '../math/gluing.js';
import { FLAT, SPHERICAL, mul4, voxelPose } from '../math/geometry.js';
import { cubeEdgeClasses } from '../math/topology.js';
import { faceColors, edgeColor } from '../palette.js';

const FOG_COLOR = new THREE.Vector3(0.70, 0.80, 0.90);
const SUN = new THREE.Vector3(0.45, 0.75, 0.35).normalize();
const FOV_DEG = 75;

const array = (n, make) => Array.from({ length: n }, make);
const toMatrix3 = (m) => new THREE.Matrix3().set(...m); // row-major in, as three.js expects
const toMatrix4 = (m) => new THREE.Matrix4().set(...m);

export class WorldRenderer {
  constructor(canvas) {
    this.canvas = canvas;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: false, powerPreference: 'high-performance' });
    this.quality = 'auto';
    this.uniforms = {
      uVox: { value: null }, uN: { value: 16 },
      uBoxMin: { value: new THREE.Vector3() }, uBoxMax: { value: new THREE.Vector3() },
      uPalette: { value: array(16, (_, i) => new THREE.Vector3(...(MATERIALS[i]?.color || [1, 0, 1]))) },
      uCamPos: { value: new THREE.Vector3() }, uCamRight: { value: new THREE.Vector3() },
      uCamUp: { value: new THREE.Vector3() }, uCamFwd: { value: new THREE.Vector3() },
      uTanHalf: { value: Math.tan((FOV_DEG / 2) * Math.PI / 180) }, uAspect: { value: 1 },
      uFaceKind: { value: array(6, () => 0) },
      uPortal: { value: array(6, () => new THREE.Matrix4()) },
      uTarget: { value: array(6, () => 0) },
      uFaceColor: { value: array(6, () => new THREE.Vector3()) },
      uFaceU: { value: array(6, (_, i) => new THREE.Vector3(...faceFrame(i).u)) },
      uFaceV: { value: array(6, (_, i) => new THREE.Vector3(...faceFrame(i).v)) },
      uGlyphCount: { value: array(6, () => 0) },
      uGlyphInv: { value: array(12, () => new THREE.Vector4(1, 0, 0, 1)) },
      uEdgeColor: { value: array(24, () => new THREE.Vector3(1, 1, 1)) },
      uEdgeSingular: { value: array(24, () => 0) },
      uSelfPos: { value: array(MAX_SELF_IMAGES, () => new THREE.Vector3()) },
      uSelfFrame: { value: array(MAX_SELF_IMAGES, () => new THREE.Matrix3()) },
      uSelfInverse: { value: array(MAX_SELF_IMAGES, () => new THREE.Matrix3()) },
      uSelfCount: { value: 0 },
      uGeometry: { value: 0 }, uChartA: { value: 8 }, uCurvatureR: { value: 1 },
      uShowFrame: { value: 1 }, uShowSeams: { value: 1 }, uShowSelf: { value: 1 },
      uFog: { value: 40 }, uFogColor: { value: FOG_COLOR }, uSun: { value: SUN },
    };
    const material = new THREE.RawShaderMaterial({
      uniforms: this.uniforms, vertexShader, fragmentShader, glslVersion: THREE.GLSL3, depthTest: false, depthWrite: false,
    });
    const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material);
    quad.frustumCulled = false;
    this.scene = new THREE.Scene();
    this.scene.add(quad);
    this.camera = new THREE.Camera();
    this.resize();
  }

  /** Upload the blocks and the gluing (call when either changes). */
  setWorld(space, analysis) {
    const u = this.uniforms, grid = space.grid, n = grid.n;
    if (u.uVox.value) u.uVox.value.dispose();
    const tex = new THREE.Data3DTexture(grid.data, n, n, n);
    tex.format = THREE.RedFormat; tex.type = THREE.UnsignedByteType;
    tex.minFilter = tex.magFilter = THREE.NearestFilter; tex.unpackAlignment = 1; tex.needsUpdate = true;
    u.uVox.value = tex;
    u.uN.value = n;
    const box = grid.bounds() || { min: [0, 0, 0], max: [0, 0, 0] };
    u.uBoxMin.value.set(...box.min); u.uBoxMax.value.set(...box.max);

    const colors = faceColors(space.faces);
    for (let i = 0; i < FACE_COUNT; i++) {
      const spec = space.faces[i], h = space.portals[i];
      u.uFaceKind.value[i] = spec.kind === GLUE ? 1 : spec.kind === 'mirror' ? 2 : 0;
      if (h) u.uPortal.value[i] = toMatrix4(h.H);
      u.uTarget.value[i] = h ? h.target : i;
      u.uFaceColor.value[i].set(...colors[i]);
    }
    const g = space.geometry;
    u.uGeometry.value = g.kind === FLAT ? 0 : g.kind === SPHERICAL ? 1 : -1;
    u.uChartA.value = g.a;
    u.uCurvatureR.value = g.R;
    this.setGlyphs(space.faces);
    for (const [i, h] of cubeEdgeClasses(analysis, g.corner).entries()) {
      u.uEdgeColor.value[i].set(...edgeColor(h.full, h.ends === 2 ? h.end : 0.5));
      u.uEdgeSingular.value[i] = h.singular ? 1 : 0;
    }
  }

  // The letter F drawn on glued faces: on the lower-numbered face of each pair as is, on its
  // partner as the image under the gluing, so the two Fs mark the same points.
  setGlyphs(faces) {
    const u = this.uniforms;
    for (let i = 0; i < FACE_COUNT; i++) {
      const maps = glyphMaps(faces, i);
      u.uGlyphCount.value[i] = maps.length;
      maps.forEach((m, k) => {
        const inv = [m[0], m[2], m[1], m[3]]; // signed permutations: inverse = transpose
        u.uGlyphInv.value[i * 2 + k].set(...inv);
      });
    }
  }

  setOptions({ frame, seams, self, fog }) {
    const u = this.uniforms;
    if (frame !== undefined) u.uShowFrame.value = frame ? 1 : 0;
    if (seams !== undefined) u.uShowSeams.value = seams ? 1 : 0;
    if (self !== undefined) u.uShowSelf.value = self ? 1 : 0;
    if (fog !== undefined) u.uFog.value = fog;
  }

  setQuality(q) { this.quality = q; this.resize(); }

  resize() {
    const dpr = window.devicePixelRatio || 1, w = window.innerWidth, h = window.innerHeight;
    const q = this.quality === 'auto' ? Math.min(1, Math.sqrt(1300000 / (w * h * dpr * dpr))) : +this.quality;
    this.renderer.setPixelRatio(dpr * q);
    this.renderer.setSize(w, h, false);
    this.uniforms.uAspect.value = w / h;
  }

  /** Draw the view of a player (see player.js) in a space (see space.js). */
  render(player, space) {
    const u = this.uniforms, view = player.view();
    u.uCamPos.value.set(...view.pos);
    u.uCamRight.value.set(...view.right);
    u.uCamUp.value.set(...view.up);
    u.uCamFwd.value.set(...view.back.map((x) => -x));
    const images = selfImages(player, space);
    u.uSelfCount.value = images.length;
    images.forEach((pose, i) => {
      const F = toMatrix3(fromColumns(pose.right, pose.up, pose.back));
      u.uSelfPos.value[i].set(...pose.pos);
      u.uSelfFrame.value[i] = F;
      u.uSelfInverse.value[i] = F.clone().invert();
    });
    this.renderer.render(this.scene, this.camera);
  }

  dispose() { this.renderer.dispose(); }
}

/** 2x2 maps (face frame coordinates) of the F glyphs drawn on face i. */
export function glyphMaps(faces, i) {
  const spec = faces[i];
  if (spec.kind !== GLUE) return [];
  const j = spec.partner;
  if (j === i) return [[1, 0, 0, 1], faceMap2D(spec.P, i, i)];
  return i < j ? [[1, 0, 0, 1]] : [faceMap2D(faces[j].P, j, i)];
}

// You (your body, without the pitch), plus the copies of you that show up just past nearby
// portals, so you can see the part of yourself sticking through a face. Voxel poses.
function selfImages(player, space) {
  const g = space.geometry, out = [voxelPose(g, player.motion)];
  const pos = out[0].pos;
  for (let f = 0; f < FACE_COUNT && out.length < MAX_SELF_IMAGES; f++) {
    const h = space.portals[f];
    if (!h) continue;
    const k = f >> 1, bound = f & 1 ? 0 : space.n;
    if (Math.abs(pos[k] - bound) > 1.3) continue;
    out.push(voxelPose(g, mul4(h.G, player.motion)));
  }
  return out;
}

