// The glued cube as a place you can be in: a voxel grid of n blocks per side, occupying
// [0, n]^3, whose faces are walls, mirrors or portals (see math/gluing.js).

import { IDENTITY, matMul, matVec } from '../math/cube.js';
import { portal, MIRROR } from '../math/gluing.js';

export class PortalSpace {
  constructor(faces, grid) {
    this.faces = faces;
    this.grid = grid;
    this.n = grid.n;
    const L = this.n / 2, c = [L, L, L];
    // Rescale each portal h(x) = A x + t from the unit cube [-1,1]^3 to block coordinates:
    // x_blocks = L (x_unit + 1), so h_blocks(x) = A (x - c) + c + L t.
    this.portals = faces.map((spec, i) => {
      const h = portal(faces, i);
      if (!h) return null;
      const Ac = matVec(h.A, c);
      return { A: h.A, t: [0, 1, 2].map((k) => c[k] - Ac[k] + L * h.t[k]), target: h.target, mirror: spec.kind === MIRROR };
    });
  }

  through(face, p) {
    const h = this.portals[face];
    const y = matVec(h.A, p);
    return [y[0] + h.t[0], y[1] + h.t[1], y[2] + h.t[2]];
  }

  /** The face that p is furthest outside of, or -1 if p is in the cube. */
  outsideFace(p) {
    let face = -1, worst = 0;
    for (let k = 0; k < 3; k++) {
      if (-p[k] > worst) { worst = -p[k]; face = k * 2 + 1; }
      if (p[k] - this.n > worst) { worst = p[k] - this.n; face = k * 2; }
    }
    return face;
  }

  /**
   * Follow portals until p is back inside the cube. Returns the point, the accumulated linear
   * part A (how directions turn), the faces crossed, and whether a wall got in the way.
   */
  wrap(p) {
    let A = IDENTITY;
    const crossed = [];
    for (let guard = 0; guard < 8; guard++) {
      const f = this.outsideFace(p);
      if (f < 0) break;
      if (!this.portals[f]) return { p, A, crossed, wall: f };
      p = this.through(f, p);
      A = matMul(this.portals[f].A, A);
      crossed.push(f);
    }
    return { p, A, crossed, wall: -1 };
  }

  isSolid(p) {
    const w = this.wrap(p);
    if (w.wall >= 0) return true;
    return this.grid.get(Math.floor(w.p[0]), Math.floor(w.p[1]), Math.floor(w.p[2])) > 0;
  }
}
