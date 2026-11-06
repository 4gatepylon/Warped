// The player: a position in the cube and a body frame that is carried through portals.
//
// As in the curved-space demos, orientation is a body frame B (columns: right, up, back) plus a
// separate pitch angle. Turning left/right rotates B about its own up axis, so you stay level
// relative to your own "up" - but going through a portal applies the portal's linear map to B,
// so your up can end up pointing anywhere in the cube, and B can even become a mirror image.

import { IDENTITY, matMul, matVec, det, column, fromColumns, cross } from '../math/cube.js';
import { spawnPoint } from './scene.js';

export const RADIUS = 0.3;
export const LOOK_SENSITIVITY = 0.0025;
const MAX_PITCH = 1.55;
const MAX_STEP = 0.1;

const rotY = (a) => { const c = Math.cos(a), s = Math.sin(a); return [c, 0, s, 0, 1, 0, -s, 0, c]; };
const rotX = (a) => { const c = Math.cos(a), s = Math.sin(a); return [1, 0, 0, 0, c, -s, 0, s, c]; };
const normalize = (v) => { const l = Math.hypot(v[0], v[1], v[2]); return v.map((x) => x / l); };

export class Player {
  constructor(space) {
    this.space = space;
    this.reset();
  }

  reset() {
    const s = spawnPoint(this.space.n);
    this.pos = s.pos.slice();
    this.pitch = s.pitch;
    this.body = IDENTITY.slice();
    this.turn(s.yaw);
    this.holonomy = IDENTITY.slice(); // product of every portal's linear part since spawning
    this.crossings = 0;
    this.lastFace = -1;
  }

  /** Mouse look: dx turns right, dy looks down (screen pixels). */
  look(dx, dy) {
    this.turn(dx * LOOK_SENSITIVITY);
    this.pitch = Math.max(-MAX_PITCH, Math.min(MAX_PITCH, this.pitch - dy * LOOK_SENSITIVITY));
  }

  turn(angle) { this.body = matMul(this.body, rotY(-angle)); }

  /** Camera axes in cube coordinates. */
  camera() {
    const C = matMul(this.body, rotX(this.pitch));
    const back = column(C, 2);
    return { right: column(C, 0), up: column(C, 1), forward: back.map((x) => -x), frame: C };
  }

  get mirrored() { return det(this.body) < 0; }

  /**
   * Move by a displacement given in body coordinates (right, up, forward), sliding along
   * blocks and walls and going through portals.
   */
  move(local) {
    const len = Math.hypot(local[0], local[1], local[2]);
    if (!len) return;
    const steps = Math.ceil(len / MAX_STEP);
    const bodyStep = [local[0] / steps, local[1] / steps, -local[2] / steps];
    for (let s = 0; s < steps; s++) {
      const d = matVec(this.body, bodyStep);
      for (let k = 0; k < 3; k++) {
        if (!d[k]) continue;
        const trial = this.pos.slice();
        trial[k] += d[k];
        const probe = trial.slice();
        probe[k] += Math.sign(d[k]) * RADIUS;
        if (!this.space.isSolid(probe)) this.pos = trial;
      }
      this.passPortals();
    }
    this.orthonormalize();
  }

  passPortals() {
    for (let guard = 0; guard < 4; guard++) {
      const f = this.space.outsideFace(this.pos);
      if (f < 0) return;
      const h = this.space.portals[f];
      if (!h) { this.clampInside(); return; }
      this.pos = this.space.through(f, this.pos);
      this.body = matMul(h.A, this.body);
      this.holonomy = matMul(h.A, this.holonomy);
      this.crossings++;
      this.lastFace = f;
    }
  }

  clampInside() {
    const n = this.space.n;
    this.pos = this.pos.map((x) => Math.max(1e-3, Math.min(n - 1e-3, x)));
  }

  // Keep the body frame exactly orthonormal (with its current handedness) despite rounding.
  orthonormalize() {
    const sign = det(this.body) < 0 ? -1 : 1;
    const r = normalize(column(this.body, 0));
    let u = column(this.body, 1);
    const ru = r[0] * u[0] + r[1] * u[1] + r[2] * u[2];
    u = normalize(u.map((x, i) => x - ru * r[i]));
    const b = cross(r, u).map((x) => x * sign);
    this.body = fromColumns(r, u, b);
  }
}
