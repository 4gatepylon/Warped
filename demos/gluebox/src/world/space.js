// The glued cube as a place you can be in: a voxel grid of n blocks per side, occupying
// [0, n]³ in voxel coordinates, whose faces are walls, mirrors or portals (see math/gluing.js),
// in flat, spherical or hyperbolic space (see math/geometry.js).
//
// Voxel coordinates are a rescaled projective chart, so straight lines stay straight lines in
// them whatever the curvature. Each portal is therefore a 4x4 projective map H on homogeneous
// voxel coordinates (an ordinary rotation + translation in flat space).

import { IDENTITY, matMul } from '../math/cube.js';
import { portal, MIRROR } from '../math/gluing.js';
import { makeGeometry, portalMotion, toVoxelMap, apply4 } from '../math/geometry.js';

export class PortalSpace {
  /** @param corner the cube's corner angle in degrees (90 = flat). */
  constructor(faces, grid, corner = 90) {
    this.faces = faces;
    this.grid = grid;
    this.n = grid.n;
    this.geometry = makeGeometry(corner, this.n);
    this.portals = faces.map((spec, i) => {
      const h = portal(faces, i);
      if (!h) return null;
      const G = portalMotion(this.geometry, faces, i);
      return {
        G,                                   // the motion, in model coordinates
        H: toVoxelMap(this.geometry, G),     // the same, in homogeneous voxel coordinates
        A: h.A,                              // how it turns directions, as a cube symmetry
        target: h.target,
        mirror: spec.kind === MIRROR,
      };
    });
  }

  /** Where a point just past face `face` really is: just inside the partner face. */
  through(face, v) {
    const y = apply4(this.portals[face].H, [v[0], v[1], v[2], 1]);
    return [y[0] / y[3], y[1] / y[3], y[2] / y[3]];
  }

  /** The face that v is furthest outside of, or -1 if v is in the cube. */
  outsideFace(v) {
    let face = -1, worst = 0;
    for (let k = 0; k < 3; k++) {
      if (-v[k] > worst) { worst = -v[k]; face = k * 2 + 1; }
      if (v[k] - this.n > worst) { worst = v[k] - this.n; face = k * 2; }
    }
    return face;
  }

  /**
   * Follow portals until v is back inside the cube. Returns the point, the accumulated cube
   * symmetry A (how directions got turned), the faces crossed, and whether a wall was in the way.
   */
  wrap(v) {
    let A = IDENTITY;
    const crossed = [];
    for (let guard = 0; guard < 8; guard++) {
      const f = this.outsideFace(v);
      if (f < 0) break;
      if (!this.portals[f]) return { p: v, A, crossed, wall: f };
      v = this.through(f, v);
      A = matMul(this.portals[f].A, A);
      crossed.push(f);
    }
    return { p: v, A, crossed, wall: -1 };
  }

  isSolid(v) {
    const w = this.wrap(v);
    if (w.wall >= 0) return true;
    return this.grid.get(Math.floor(w.p[0]), Math.floor(w.p[1]), Math.floor(w.p[2])) > 0;
  }
}
