// Integer linear algebra for cellular homology: rank and Smith normal form invariants.

/** Invariant factors (positive, each dividing the next) of an integer matrix given as rows. */
export function invariantFactors(rows) {
  const A = rows.map((r) => r.slice());
  const m = A.length, n = m ? A[0].length : 0;
  const factors = [];
  for (let t = 0; t < Math.min(m, n); t++) {
    if (!movePivot(A, t)) break;
    for (;;) {
      clearPivotColumn(A, t);
      clearPivotRow(A, t);
      if (!pivotColumnOrRowDirty(A, t) && !fixDivisibility(A, t)) break;
    }
    factors.push(Math.abs(A[t][t]));
  }
  return factors;
}

export const rank = (rows) => invariantFactors(rows).length;

// Bring the entry of smallest nonzero absolute value in the lower-right block to (t, t).
function movePivot(A, t) {
  let best = 0, bi = -1, bj = -1;
  for (let i = t; i < A.length; i++) {
    for (let j = t; j < A[i].length; j++) {
      const v = Math.abs(A[i][j]);
      if (v && (!best || v < best)) { best = v; bi = i; bj = j; }
    }
  }
  if (bi < 0) return false;
  [A[t], A[bi]] = [A[bi], A[t]];
  for (const row of A) [row[t], row[bj]] = [row[bj], row[t]];
  return true;
}

function clearPivotColumn(A, t) {
  for (let i = t + 1; i < A.length; i++) {
    const q = Math.trunc(A[i][t] / A[t][t]);
    if (q) for (let j = t; j < A[i].length; j++) A[i][j] -= q * A[t][j];
  }
}

function clearPivotRow(A, t) {
  for (let j = t + 1; j < A[t].length; j++) {
    const q = Math.trunc(A[t][j] / A[t][t]);
    if (q) for (let i = t; i < A.length; i++) A[i][j] -= q * A[i][t];
  }
}

// If a remainder survived in the pivot's row or column, make it the new pivot and go again.
function pivotColumnOrRowDirty(A, t) {
  let dirty = false;
  for (let i = t + 1; i < A.length; i++) if (A[i][t]) dirty = true;
  for (let j = t + 1; j < A[t].length; j++) if (A[t][j]) dirty = true;
  if (dirty) movePivotWithin(A, t);
  return dirty;
}

function movePivotWithin(A, t) {
  // smallest nonzero entry in pivot row/column
  let best = Math.abs(A[t][t]), bi = t, bj = t;
  for (let i = t + 1; i < A.length; i++) if (A[i][t] && Math.abs(A[i][t]) < best) { best = Math.abs(A[i][t]); bi = i; bj = t; }
  for (let j = t + 1; j < A[t].length; j++) if (A[t][j] && Math.abs(A[t][j]) < best) { best = Math.abs(A[t][j]); bi = t; bj = j; }
  [A[t], A[bi]] = [A[bi], A[t]];
  for (const row of A) [row[t], row[bj]] = [row[bj], row[t]];
}

// Smith form needs the pivot to divide everything below-right of it.
function fixDivisibility(A, t) {
  const p = A[t][t];
  for (let i = t + 1; i < A.length; i++) {
    for (let j = t + 1; j < A[i].length; j++) {
      if (A[i][j] % p !== 0) {
        for (let k = t; k < A[t].length; k++) A[t][k] += A[i][k];
        return true;
      }
    }
  }
  return false;
}

/**
 * H_k = ker(∂_k) / im(∂_{k+1}) for boundary matrices ∂_k : C_k -> C_{k-1} given as
 * (rows = cells of dimension k-1, columns = cells of dimension k).
 * Returns { free, torsion } with torsion the list of invariant factors > 1.
 */
export function homologyGroup(dimCk, boundaryK, boundaryK1) {
  const rk = boundaryK ? rank(boundaryK) : 0;
  const f = boundaryK1 ? invariantFactors(boundaryK1) : [];
  return { free: dimCk - rk - f.length, torsion: f.filter((d) => d > 1) };
}

/** Pretty "ℤ³ ⊕ ℤ/2" style formatting. */
export function formatGroup({ free, torsion }) {
  const sup = { 2: '²', 3: '³', 4: '⁴', 5: '⁵', 6: '⁶' };
  const parts = [];
  if (free === 1) parts.push('ℤ');
  else if (free > 1) parts.push('ℤ' + (sup[free] || `^${free}`));
  const counts = new Map();
  for (const d of torsion) counts.set(d, (counts.get(d) || 0) + 1);
  for (const [d, c] of counts) parts.push(c === 1 ? `ℤ/${d}` : `(ℤ/${d})${sup[c] || `^${c}`}`);
  return parts.length ? parts.join(' ⊕ ') : '0';
}

/** Canonical key for comparing groups, e.g. "Z1+2+2". */
export const groupKey = ({ free, torsion }) => `Z${free}` + torsion.slice().sort((a, b) => a - b).map((d) => `+${d}`).join('');
