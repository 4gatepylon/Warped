// The player: a motion of space (see math/geometry.js) that carries the origin to where you are
// and the coordinate axes to your body axes (columns: right, up, back, position), plus a
// separate pitch angle - the same set-up as the curved-space demos.
//
// Turning rotates the motion about your own up axis, so you stay level relative to your own
// "up". Walking slides you along straight lines of the (possibly curved) space. Going through a
// portal applies the portal's motion, so your up can end up pointing anywhere in the cube, and
// you can even come back as a mirror image.

import { IDENTITY, matMul, fromColumns } from '../math/cube.js';
import {
  mul4, apply4, col4, det4, embed3, rotation4, chartVelocity, slide, slideTo, distance, tidyMotion,
  voxelToModel, voxelPose, modelToChart, chartToVoxel,
} from '../math/geometry.js';
import { spawnPoint } from './scene.js';

export const RADIUS = 0.3;
export const LOOK_SENSITIVITY = 0.0025;
const MAX_PITCH = 1.55;
const MAX_STEP = 0.1;
const ORIGIN = [0, 0, 0, 1];

const normalize = (v) => { const l = Math.hypot(v[0], v[1], v[2]); return v.map((x) => x / l); };

export class Player {
  constructor(space) {
    this.space = space;
    this.g = space.geometry;
    this.reset();
  }

  reset() {
    const s = spawnPoint(this.space.n);
    this.place(s.pos, IDENTITY);
    this.turn(s.yaw);
    this.pitch = s.pitch;
    this.holonomy = IDENTITY.slice(); // product of every portal's cube symmetry since spawning
    this.crossings = 0;
    this.lastFace = -1;
  }

  /** Stand at voxel position v with body axes given (relative to the cube) by a 3x3 matrix. */
  place(v, body) {
    this.motion = mul4(slideTo(this.g, ORIGIN, voxelToModel(this.g, v)), embed3(body));
  }

  /** Position in voxel coordinates. */
  get pos() { return chartToVoxel(this.g, modelToChart(col4(this.motion, 3))); }

  /** Body axes (right, up, back) as directions in the cube, as the columns of a 3x3 matrix. */
  get body() {
    const X = col4(this.motion, 3);
    return fromColumns(...[0, 1, 2].map((c) => normalize(chartVelocity(X, col4(this.motion, c)))));
  }

  get mirrored() { return det4(this.motion) < 0; }

  /** Mouse look: dx turns right, dy looks down (screen pixels). */
  look(dx, dy) {
    this.turn(dx * LOOK_SENSITIVITY);
    this.pitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, this.pitch - dy * LOOK_SENSITIVITY));
  }

  turn(angle) { this.motion = mul4(this.motion, rotation4(0, 2, angle)); }

  /** The camera's motion: the body tilted by the pitch. */
  camera() { return mul4(this.motion, rotation4(1, 2, this.pitch)); }

  /** Camera position and axes in voxel coordinates (see geometry.voxelPose). */
  view() { return voxelPose(this.g, this.camera()); }

  /**
   * Move by a displacement in blocks given in body coordinates (right, up, forward), going
   * through portals. Each small step goes along a true straight line. If a block or wall is in
   * the way, the step is split along the cube's three axis directions and only the free parts
   * are taken, so you slide along walls instead of stopping dead.
   */
  move(local) {
    const len = Math.hypot(local[0], local[1], local[2]);
    if (!len) return;
    const steps = Math.ceil(len / MAX_STEP);
    const direction = [local[0] / len, local[1] / len, -local[2] / len, 0];
    for (let s = 0; s < steps; s++) {
      const X = col4(this.motion, 3);
      const v = this.pos;
      const end = apply4(slide(this.g, X, apply4(this.motion, direction), len / steps), X);
      const target = chartToVoxel(this.g, modelToChart(end));
      const dv = target.map((x, i) => x - v[i]);
      const free = dv.map((d, axis) => Math.abs(d) < 1e-12 || !this.space.isSolid(this.probe(v, axis, d)));
      if (free.every(Boolean)) {
        this.moveTo(target);
      } else {
        let w = v;
        dv.forEach((d, axis) => {
          if (!free[axis] || Math.abs(d) < 1e-12) return;
          w = w.slice();
          w[axis] += d;
          this.moveTo(w);
        });
      }
      this.passPortals();
    }
    this.motion = tidyMotion(this.g, this.motion);
  }

  /** The point RADIUS blocks beyond a move of d along one axis from v (for collisions). */
  probe(v, axis, d) {
    const target = v.slice();
    target[axis] += d;
    const p = v.slice();
    p[axis] += d * (1 + RADIUS / this.trueDistance(v, target));
    return p;
  }

  trueDistance(v, w) { return distance(this.g, voxelToModel(this.g, v), voxelToModel(this.g, w)); }

  /** Slide straight to a voxel position, carrying the body axes along. */
  moveTo(v) {
    const slideMotion = slideTo(this.g, col4(this.motion, 3), voxelToModel(this.g, v));
    this.motion = mul4(slideMotion, this.motion);
  }

  passPortals() {
    for (let guard = 0; guard < 4; guard++) {
      const f = this.space.outsideFace(this.pos);
      if (f < 0) return;
      const h = this.space.portals[f];
      if (!h) { this.moveTo(this.pos.map((x) => Math.max(1e-3, Math.min(this.space.n - 1e-3, x)))); return; }
      this.motion = mul4(h.G, this.motion);
      this.holonomy = matMul(h.A, this.holonomy);
      this.crossings++;
      this.lastFace = f;
    }
  }
}
