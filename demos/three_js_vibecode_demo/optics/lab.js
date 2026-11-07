// Optics lab: the code shared by the demo pages in this folder. Each page loads three.js as a module into
// window.THREE and then this file as an ordinary deferred script, so the pages also work when opened straight from
// disk. The page's <body data-demo="..."> picks the demo.
(() => {
'use strict';
document.body.insertAdjacentHTML('afterbegin', `
<canvas id="c"></canvas>
<div id="cross"></div>

<div id="controls" class="panel">
  <h1>Optics lab (flat space)</h1>
  <div>Demo: <a href="mirrors.html">mirrors</a> · <a href="splitters.html">splitters</a> · <a href="diffraction.html">diffraction</a> · <a href="heat_haze.html">heat haze</a> · <a href="refraction.html">refraction</a> · <a href="refraction_glow.html">refraction + glow</a></div>
  <div class="muted">Click the scene to capture the mouse (<kbd>Esc</kbd> releases), or drag to look.</div>
  <div class="row"><kbd>WASD</kbd> fly · <kbd>E</kbd>/<kbd>Q</kbd> up/down · <kbd>Shift</kbd> fast</div>
  <div class="row laser"><b>Laser</b> · <kbd>L</kbd> on/off<br>
    <kbd>Click</kbd> (mouse captured) or <kbd>R</kbd>: leave an emitter here that keeps firing<br>
    <kbd>Z</kbd> remove all emitters</div>
  <div class="row"><b>Panels</b> · <kbd>1</kbd> place a mirror<span class="f-split"> · <kbd>2</kbd> a beam splitter</span><span class="f-grat"> · <kbd>3</kbd> a grating</span><span class="f-glass"> · <kbd>5</kbd> a glass block · <kbd>6</kbd> a prism</span><br>
    Aim at a panel, then hold <kbd>T</kbd> or the right mouse button and move the mouse to turn it.
    <kbd>J</kbd>/<kbd>K</kbd> turn 5°, <kbd>U</kbd>/<kbd>I</kbd> tilt 5° (with <kbd>Shift</kbd>: 1°).
    <kbd>X</kbd> delete it · <kbd>C</kbd> delete all panels</div>
  <div class="row f-heat"><b>Heaters</b> · <kbd>4</kbd> put a heater on the block you aim at, or take away the heater you aim at</div>
  <div class="row f-grat">
    <label>Laser colour <select id="colour">
      <option value="650">red (650 nm)</option>
      <option value="532">green (532 nm)</option>
      <option value="450">blue (450 nm)</option>
      <option value="0">white</option>
    </select> <kbd>N</kbd></label>
    <label>Gratings: <b id="lpmval">600</b> lines per mm</label>
    <input id="lpm" type="range" min="200" max="1200" step="50" value="600">
  </div>
  <div class="row">
    <label>Render quality <select id="quality"><option value="auto" selected>auto</option><option value="0.35">low</option><option value="0.5">medium</option><option value="0.75">high</option><option value="1">full</option></select></label>
  </div>
  <div class="row">
    <button id="bLaser" class="on">Laser: on</button>
    <button id="bPin">Leave emitter</button>
    <button id="bUnpin">Remove emitters</button>
  </div>
  <div class="row">
    <button id="bScene">Reset scene</button>
    <button id="bView">Reset view</button>
  </div>
</div>

<div id="stats" class="panel">
  <div>Panels: <b id="sP">0</b> · emitters: <b id="sE">0</b></div>
  <div>Laser: <span id="sLaser">–</span></div>
  <div class="muted">Aimed at: <span id="sAim">–</span></div>
  <div id="sMsg"></div>
  <div class="muted"><span id="sFps">–</span> fps</div>
</div>

<div id="tips" class="panel">
  <button id="tipsToggle">hide</button>
  <h1>Things to try</h1>
  <div id="tipsBody"></div>
</div>
<div id="err"></div>
`);
if (!window.THREE) {
  const el = document.getElementById('err'); el.style.display = 'flex';
  el.textContent = 'three.js could not be loaded from cdn.jsdelivr.net, so the lab cannot start.';
  throw new Error('three.js is missing');
}

// ---------------------------------------------------------------------------
// Six demos, one page each, each adding an optical element to the previous one.
// ---------------------------------------------------------------------------
const MODES = {
  mirrors: { splitter: false, grating: false },
  splitters: { splitter: true, grating: false },
  diffraction: { splitter: true, grating: true },
  heat_haze: { splitter: true, grating: true, heat: true },
  refraction: { splitter: true, grating: true, heat: true, glass: true },
  refraction_glow: { splitter: true, grating: true, heat: true, glass: true },
};
const TIPS = {
  mirrors: `<ol>
    <li><b>Angle in = angle out.</b> Aim your laser at a mirror and turn it (hold <kbd>T</kbd> and move the mouse): turning a mirror by some angle turns the reflected beam by twice that.</li>
    <li><b>Zigzag</b> (left): three mirrors at 45° walk a beam across the floor and onto the white screen.</li>
    <li><b>Periscope</b> (right): two mirrors lift a beam over the low wall.</li>
    <li><b>Hall of mirrors</b> (middle): two facing mirrors. A beam fired in at an angle ricochets down the gap. Stand between them and look sideways to see reflections of reflections.</li>
    <li><b>Corner reflector</b> (front right, on the floor): three mirrors at right angles. Any beam that goes in comes straight back out, parallel to itself, offset by twice how far it went in from the corner. Aim your laser into it.</li>
    <li>Mirrors are real reflections in the renderer too, so you can see beams (and yourself... almost) in them.</li>
  </ol>`,
  splitters: `<ol>
    <li><b>A beam splitter</b> is a half-silvered mirror: half the light reflects, half goes straight through. Every split halves the brightness.</li>
    <li><b>Cascade</b> (left): three splitters and a mirror turn one beam into four, at 50%, 25%, 12.5% and 12.5% (the mirror loses a little).</li>
    <li><b>Mach–Zehnder interferometer</b> (right): a beam splits, the two halves take different paths, and a second splitter recombines them into two outputs.</li>
    <li>Looking through a splitter you see both what's behind it and a reflection, half each.</li>
    <li>Limitation: beams here are rays, not waves, so the interferometer can't show interference. In a real one, the two outputs get brighter and darker as you change one path's length by fractions of a wavelength.</li>
  </ol>`,
  diffraction: `<ol>
    <li><b>A diffraction grating</b> (fine parallel lines) splits a beam into "orders" at angles given by sin θ<sub>out</sub> = sin θ<sub>in</sub> + m·λ/d, where λ is the wavelength, d the line spacing and m = 0, ±1, ±2...</li>
    <li><b>Colour matters.</b> Red light (long λ) spreads more than blue. Switch your laser colour (<kbd>N</kbd>) and watch the spots move.</li>
    <li><b>White light</b> (top row on the screen) stays white straight through, but every other order fans out into a rainbow.</li>
    <li><b>Orders can disappear.</b> If sin θ would exceed 1 that order doesn't exist. Slide the lines per mm up, or tilt a grating, and watch outer orders vanish.</li>
    <li>Brightness per order (a thin sinusoidal phase grating): about 30% straight through, 30% in each first order, 4.6% in each second order, less beyond.</li>
    <li>Limitation: looking <i>through</i> a grating you only see the straight-through view, not the shifted rainbow copies a real one shows.</li>
  </ol>`,
  heat_haze: `<ol>
    <li><b>Hot air bends light.</b> Warm air is thinner, so its refractive index is lower. Light bends toward the cooler, denser air: outward from the middle of the plume rising off a heater, and upward, because the air is hottest near the heater.</li>
    <li><b>Turbulence spreads the beam.</b> The rising air churns, so different parts of the beam are bent different ways, and it leaves as a widening, soft glow (left: a beam through the air just above a heater). Its spot is wide and dim: the same light is spread over a bigger area.</li>
    <li><b>Higher up is cooler:</b> the beam above it crosses the same plume higher up and spreads much less.</li>
    <li><b>Off-centre</b> (right, green): a beam through the edge of a plume is mostly pushed aside rather than spread.</li>
    <li>Look through the hot air: the world behind it shimmers too. <kbd>4</kbd> puts a heater where you aim, or takes one away.</li>
    <li>Exaggerated: real hot air changes the refractive index by about 0.0001, so real heat shimmer bends light by hundredths of a degree. Here the change is about 1000× bigger, so you can see it across the lab.</li>
  </ol>`,
  refraction: `<ol>
    <li><b>Snell's law.</b> Light is slower in glass, by its refractive index (about 1.63 here), so a beam entering at an angle bends toward the face's normal, and away from it on the way out: sin θ<sub>air</sub> = n sin θ<sub>glass</sub>.</li>
    <li><b>Glass block</b> (left): the beam leaves parallel to how it came in, shifted sideways. Turn the block (hold <kbd>T</kbd>) and the shift grows with the angle.</li>
    <li><b>Prism</b> (middle): blue light is slowed more than red (dispersion), so it bends more, and white light fans out into a rainbow on the screen.</li>
    <li><b>Total internal reflection</b> (right): inside glass, a beam meeting a face more than about 38° from its normal can't get out and is reflected completely. The long bar guides the beam along like an optical fibre.</li>
    <li>Some light always reflects off a glass face: about 6% head-on, more at a glancing angle. Look for the faint reflected beams.</li>
    <li><kbd>5</kbd> places a glass block, <kbd>6</kbd> a prism. Shine red, green and blue lasers (<kbd>N</kbd>) through a prism and compare.</li>
  </ol>`,
  refraction_glow: `<ol>
    <li><b>Hot air, then glass.</b> Each beam first crosses the hot air above a heater, which spreads it into a widening soft glow, then goes through glass, which bends the glow like any other beam.</li>
    <li><b>Soft rainbow</b> (left): white light softened by hot air goes through a prism. Each colour is bent by a different amount, but each is now a wide glow, so the colours overlap into a blurred rainbow on the screen.</li>
    <li><b>Glowing bar</b> (right): a softened beam enters a long glass bar and is trapped by total internal reflection, so the bar fills with glow.</li>
    <li>Aim at a heater and press <kbd>4</kbd> to take it away and compare with a sharp beam. <kbd>4</kbd> on the floor puts one back.</li>
  </ol>`,
};

// ---------------------------------------------------------------------------
// Voxel world (stored in a 3D texture that the ray tracer reads)
// ---------------------------------------------------------------------------
const NX = 64, NY = 24, NZ = 64;
const M = { air: 0, grass: 1, dirt: 2, stone: 3, planks: 4, log: 5, leaves: 6, glass: 7, roof: 8, brick: 9, quartz: 10, obsidian: 11,
  red: 12, blue: 13, yellow: 14, purple: 15, orange: 16, gold: 17, cobble: 18, heater: 19 };
const MAT_NAME = Object.fromEntries(Object.entries(M).map(([k, v]) => [v, k]));
const vox = new Uint8Array(NX * NY * NZ);
const inGrid = (x, y, z) => x >= 0 && y >= 0 && z >= 0 && x < NX && y < NY && z < NZ;
const set = (x, y, z, m) => { if (inGrid(x, y, z)) vox[x + y * NX + z * NX * NY] = m; };
const get = (x, y, z) => inGrid(x, y, z) ? vox[x + y * NX + z * NX * NY] : (y < 1 ? M.grass : 0);
function mulberry32(a) { return function () { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }

function buildWorld() {
  vox.fill(0);
  for (let x = 0; x < NX; x++) for (let z = 0; z < NZ; z++) set(x, 0, z, M.grass);
  // lab floor: pale tiles with a darker line every 4 blocks, for judging distances
  for (let x = 8; x < 56; x++) for (let z = 14; z < 60; z++) set(x, 0, z, (x % 4 === 0 || z % 4 === 0) ? M.stone : M.quartz);
  // a tall white screen to catch the beams
  for (let x = 4; x <= 59; x++) for (let y = 1; y <= 9; y++) set(x, y, 12, (x === 4 || x === 59 || y === 9) ? M.cobble : M.quartz);
  // a low wall for the periscope
  for (let x = 40; x <= 48; x++) for (let y = 1; y <= 3; y++) set(x, y, 21, M.cobble);
  // coloured posts at the sides of the lab
  const cols = [M.red, M.orange, M.yellow, M.blue, M.purple];
  for (let k = 0; k < 5; k++) for (let y = 1; y <= 4; y++) { set(6, y, 20 + k * 8, cols[k]); set(57, y, 20 + k * 8, cols[4 - k]); }
  // trees around the outside
  const rng = mulberry32(7);
  let placed = 0;
  for (let tries = 0; tries < 600 && placed < 22; tries++) {
    const tx = 2 + Math.floor(rng() * (NX - 4)), tz = 2 + Math.floor(rng() * (NZ - 4));
    if (tx > 3 && tx < 60 && tz > 8) continue;   // keep the lab clear: trees only along the sides and behind the screen
    const h = 4 + Math.floor(rng() * 2);
    for (let y = h - 1; y <= h; y++) for (let x = -2; x <= 2; x++) for (let z = -2; z <= 2; z++) { if (Math.abs(x) === 2 && Math.abs(z) === 2 && rng() < 0.7) continue; set(tx + x, y, tz + z, M.leaves); }
    for (let x = -1; x <= 1; x++) for (let z = -1; z <= 1; z++) set(tx + x, h + 1, tz + z, M.leaves);
    for (let y = 1; y < h; y++) set(tx, y, tz, M.log);
    placed++;
  }
}
buildWorld();

// ---------------------------------------------------------------------------
// Limits shared by the CPU tracer and the shader
// ---------------------------------------------------------------------------
const MAXP = 16, MAXSEG = 256, MAXG = MAXSEG / 8, MAXS = 48, MAXE = 16, MAXPINS = 12, MAXH = 8;
// Hot-air plume above a heater: index drop A at the bottom of a plume of radius R (blocks) that cools with height
// scale H and ends at height top. turb sets how much turbulence wobbles the view through it, and glow how fast a
// beam that crossed it widens (its width grows by glow × the index drop it crossed, per block travelled).
const PLUME = { R: 1, H: 3.5, top: 10, A: 0.08, turb: 2.2, glow: 1.3 };
// Glass (panel types 3 and 4): dense flint, refractive index n(λ) = A + B/λ² (λ in nm), so blue bends more than red.
// d is a glass block's half thickness; the view through glass uses the single index `view`.
const GLASS = { A: 1.6, B: 9500, d: 0.4, view: 1.625 };
const FRAME = 0.06;                     // panel frame width (blocks); the frame is opaque
const TYPE_NAME = ['mirror', 'beam splitter', 'grating', 'glass block', 'prism'];

// ---------------------------------------------------------------------------
// Ray tracer. Each pixel's ray walks the voxel grid. Panels (mirrors, splitters, gratings) are flat
// rectangles: a mirror sends the ray on reflected, a splitter sends it both ways at half weight (a
// small stack of pending rays), a grating lets it through. Every straight piece of a pixel's ray path
// is tested against the laser segments, so beams also show up in mirrors. Rays passing hot air are bent
// where they pass closest to the plume's axis (see hitPlume).
// ---------------------------------------------------------------------------
const vert = `
precision highp float;
in vec3 position; out vec2 vNdc;
void main() { vNdc = position.xy; gl_Position = vec4(position.xy, 0.0, 1.0); }`;
const frag = `
precision highp float; precision highp int; precision highp sampler3D; precision highp sampler2D;
uniform sampler3D uVox; uniform ivec3 uDim;
uniform vec3 uCamPos, uCamRight, uCamUp, uCamFwd; uniform float uTanHalf, uAspect, uTime; uniform vec3 uSun;
uniform vec4 uPC[${MAXP}]; uniform vec4 uPN[${MAXP}]; uniform vec4 uPU[${MAXP}]; uniform int uNP, uSel;   // panel centre+type, normal+half height, u axis+half width
uniform sampler2D uSeg; uniform vec4 uGS[${MAXG}]; uniform int uNSeg, uNG;                              // laser segments, bounding sphere per 8
uniform vec4 uSpot[${MAXS}]; uniform vec4 uSpotC[${MAXS}]; uniform int uNS;                             // where beams touch things
uniform vec4 uEm[${MAXE}]; uniform int uNE;                                                             // emitter boxes
uniform vec4 uHt[${MAXH}]; uniform int uNH;                                                             // plume axis x, base y, axis z
uniform float uBeamR, uPix;
in vec2 vNdc; out vec4 outColor;

const vec3 HAZE = vec3(0.78, 0.86, 0.95);
const float FRAME = ${FRAME}, REACH = 4.8, MAXT = 700.0;
const float GLASS_N = ${GLASS.view.toFixed(3)}, GLASS_D = ${GLASS.d.toFixed(3)};
const float PR = ${PLUME.R.toFixed(3)}, PH = ${PLUME.H.toFixed(3)}, PTOP = ${PLUME.top.toFixed(3)}, PA = ${PLUME.A.toFixed(3)}, PTURB = ${PLUME.turb.toFixed(3)};
const vec3 PAL[20] = vec3[20](vec3(0.0), vec3(0.40,0.65,0.28), vec3(0.55,0.40,0.26), vec3(0.55,0.55,0.57), vec3(0.74,0.57,0.34),
  vec3(0.42,0.31,0.19), vec3(0.24,0.52,0.20), vec3(0.70,0.87,0.96), vec3(0.64,0.25,0.20), vec3(0.58,0.30,0.25), vec3(0.90,0.89,0.86),
  vec3(0.13,0.11,0.17), vec3(0.80,0.20,0.18), vec3(0.20,0.35,0.80), vec3(0.92,0.80,0.20), vec3(0.55,0.25,0.70), vec3(0.92,0.50,0.15),
  vec3(0.95,0.78,0.25), vec3(0.50,0.50,0.50), vec3(1.0,0.42,0.12));

float hash3(vec3 p) { return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453); }
float hash2(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) { vec2 i = floor(p), f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(hash2(i), hash2(i+vec2(1,0)), f.x), mix(hash2(i+vec2(0,1)), hash2(i+vec2(1,1)), f.x), f.y); }
float fbm(vec2 p) { float s = 0.0, a = 0.5; for (int i = 0; i < 5; i++) { s += a * vnoise(p); p *= 2.03; a *= 0.5; } return s; }
vec3 spectrum(float h) { return clamp(abs(fract(h + vec3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, 0.0, 1.0); }

int voxAt(ivec3 c) {
  if (c.y < 0) return 1;
  if (c.x < 0 || c.z < 0 || c.x >= uDim.x || c.y >= uDim.y || c.z >= uDim.z) return c.y < 1 ? 1 : 0;   // endless grass plane
  return int(texelFetch(uVox, c, 0).r * 255.0 + 0.5);
}
vec3 sky(vec3 d) {
  vec3 hor = HAZE;
  vec3 c = mix(hor, vec3(0.30, 0.52, 0.88), pow(max(d.y, 0.0), 0.55));
  if (d.y < 0.0) c = mix(hor, vec3(0.42, 0.56, 0.38), min(-d.y * 5.0, 1.0));
  if (d.y > 0.01) {
    vec2 uv = d.xz / (d.y + 0.1) * 0.9 + vec2(uTime * 0.01, 0.0);
    c = mix(c, vec3(1.0), smoothstep(0.52, 0.78, fbm(uv * 1.6)) * 0.85 * smoothstep(0.01, 0.15, d.y));
  }
  float sd = dot(d, uSun);
  c += vec3(1.0, 0.92, 0.75) * (smoothstep(0.99955, 0.9998, sd) * 3.0 + pow(max(sd, 0.0), 300.0) * 0.4);
  return c;
}
vec3 shadeVox(int m, vec3 hp, vec3 n, ivec3 cell) {
  vec3 base = PAL[m];
  if (m == 19) return base * (1.15 + 0.1 * sin(uTime * 2.3 + float(cell.x)));   // a heater glows
  if (m == 1 && n.y < 0.5) base = fract(hp.y) > 0.8 ? PAL[1] : PAL[2];
  vec2 uv = abs(n.x) > 0.5 ? hp.zy : (abs(n.y) > 0.5 ? hp.xz : hp.xy);
  vec2 f = fract(uv);
  base *= 0.86 + 0.22 * hash3(vec3(floor(f * 8.0), float(cell.x * 7 + cell.y * 131 + cell.z * 29)));
  vec2 e2 = min(f, 1.0 - f);
  base *= mix(0.6, 1.0, smoothstep(0.0, 0.04, min(e2.x, e2.y)));
  float face = n.y > 0.5 ? 1.0 : (n.y < -0.5 ? 0.5 : (abs(n.x) > 0.5 ? 0.82 : 0.68));
  return base * face * (0.78 + 0.22 * max(dot(n, uSun), 0.0));
}

// First solid voxel along o + t d for t < tmax (0 if none). Walks the grid; outside it, only the
// endless grass plane (top at y = 1) can be hit.
int traceVox(vec3 o, vec3 d, float tmax, out float tHit, out vec3 nrm, out ivec3 hc) {
  tHit = tmax; nrm = vec3(0.0, 1.0, 0.0); hc = ivec3(0);
  vec3 sd = vec3(d.x >= 0.0 ? 1.0 : -1.0, d.y >= 0.0 ? 1.0 : -1.0, d.z >= 0.0 ? 1.0 : -1.0);
  vec3 inv = sd / max(abs(d), vec3(1e-9));          // keeps its sign even for tiny components
  vec3 ta = -o * inv, tb = (vec3(uDim) - o) * inv;
  vec3 tmn = min(ta, tb), tmx = max(ta, tb);
  float tn = max(max(tmn.x, tmn.y), tmn.z), tf = min(min(tmx.x, tmx.y), tmx.z);
  if (tf > max(tn, 0.0) && tn < tmax) {
    float ts = max(tn, 0.0);
    ivec3 c = clamp(ivec3(floor(o + d * (ts + 1e-4))), ivec3(0), uDim - 1);
    ivec3 st = ivec3(sd);
    vec3 nxt = (vec3(c) + step(0.0, sd) - o) * inv, dl = abs(inv);
    if (tn > 0.0) {                    // entering the grid from outside: test the first cell
      int m = voxAt(c);
      if (m > 0) {
        tHit = tn; hc = c;
        nrm = tn == tmn.x ? vec3(-float(st.x), 0.0, 0.0) : (tn == tmn.y ? vec3(0.0, -float(st.y), 0.0) : vec3(0.0, 0.0, -float(st.z)));
        return m;
      }
    }
    for (int i = 0; i < 256; i++) {
      float t; vec3 n3;
      if (nxt.x < nxt.y && nxt.x < nxt.z) { t = nxt.x; c.x += st.x; nxt.x += dl.x; n3 = vec3(-float(st.x), 0.0, 0.0); }
      else if (nxt.y < nxt.z) { t = nxt.y; c.y += st.y; nxt.y += dl.y; n3 = vec3(0.0, -float(st.y), 0.0); }
      else { t = nxt.z; c.z += st.z; nxt.z += dl.z; n3 = vec3(0.0, 0.0, -float(st.z)); }
      if (t > tmax) return 0;
      if (any(lessThan(c, ivec3(0))) || any(greaterThanEqual(c, uDim))) break;
      int m = voxAt(c);
      if (m > 0) { tHit = t; hc = c; nrm = n3; return m; }
    }
  }
  if (d.y < -1e-6) {
    float tg = (1.0 - o.y) / d.y;
    if (tg > 0.0 && tg < tmax) { tHit = tg; nrm = vec3(0.0, 1.0, 0.0); hc = ivec3(floor(o + d * tg - vec3(0.0, 0.5, 0.0))); return 1; }
  }
  return 0;
}
// Glass solids are convex. A block (type 3) has 6 faces; a prism (type 4) is a triangle of side 2·hu pointing along n,
// with 2 caps. Face k of panel i: outward normal, and its distance from the centre.
vec4 solidFace(int i, int k) {
  vec3 n = uPN[i].xyz, u = uPU[i].xyz, v = cross(n, u);
  if (k < 2) return vec4(k == 0 ? v : -v, uPN[i].w);
  if (int(uPC[i].w + 0.5) == 3) return k < 4 ? vec4(k == 2 ? u : -u, uPU[i].w) : vec4(k == 4 ? n : -n, GLASS_D);
  float r = uPU[i].w / 1.7320508;                     // the triangle's inradius
  return k == 2 ? vec4(-n, r) : vec4(n * 0.5 + u * (k == 3 ? 0.8660254 : -0.8660254), r);
}
// From outside: where the ray enters solid i. From inside: where it leaves. n = that face's outward normal.
float solidHit(int i, vec3 o, vec3 d, bool inside, out vec3 n) {
  float t0 = -1e9, t1 = 1e9; vec3 n0 = vec3(0.0), n1 = vec3(0.0);
  int nf = int(uPC[i].w + 0.5) == 3 ? 6 : 5;
  for (int k = 0; k < 6; k++) {
    if (k >= nf) break;
    vec4 f = solidFace(i, k);
    float dn = dot(d, f.xyz), dist = f.w - dot(o - uPC[i].xyz, f.xyz);
    if (abs(dn) < 1e-9) { if (dist < 0.0) t1 = -1e9; continue; }
    float t = dist / dn;
    if (dn < 0.0) { if (t > t0) { t0 = t; n0 = f.xyz; } } else if (t < t1) { t1 = t; n1 = f.xyz; }
  }
  n = inside ? n1 : n0;
  return inside ? t1 : (t0 < t1 && t0 > 1e-4 ? t0 : -1.0);
}
// Snell's law into or out of glass across a face with outward normal n. R = Fresnel reflectance (unpolarised); R = 1
// is total internal reflection.
vec3 refractF(vec3 d, vec3 n, bool goingIn, out float R) {
  float eta = goingIn ? 1.0 / GLASS_N : GLASS_N;
  vec3 nf = goingIn ? n : -n;
  float ci = -dot(d, nf), k = 1.0 - eta * eta * (1.0 - ci * ci);
  if (k < 0.0) { R = 1.0; return reflect(d, nf); }
  float ct = sqrt(k), m = 1.0 / eta, rs = (ci - m * ct) / (ci + m * ct), rp = (m * ci - ct) / (m * ci + ct);
  R = 0.5 * (rs * rs + rp * rp);
  return eta * d + (eta * ci - ct) * nf;
}
// Nearest panel along the ray (not counting the one it just left). ab = position on the panel.
int hitPanel(vec3 o, vec3 d, int skip, float tmax, out float tHit, out vec2 ab) {
  int best = -1; tHit = tmax; ab = vec2(0.0);
  for (int i = 0; i < ${MAXP}; i++) {
    if (i >= uNP) break;
    if (i == skip) continue;
    if (int(uPC[i].w + 0.5) >= 3) { vec3 gn; float t = solidHit(i, o, d, false, gn); if (t > 0.0 && t < tHit) { best = i; tHit = t; ab = vec2(0.0); } continue; }
    vec3 n = uPN[i].xyz; float den = dot(d, n);
    if (abs(den) < 1e-7) continue;
    float t = dot(uPC[i].xyz - o, n) / den;
    if (t <= 1e-5 || t >= tHit) continue;
    vec3 q = o + d * t - uPC[i].xyz; vec3 u = uPU[i].xyz;
    float a = dot(q, u), b = dot(q, cross(n, u));
    if (abs(a) <= uPU[i].w && abs(b) <= uPN[i].w) { best = i; tHit = t; ab = vec2(a, b); }
  }
  return best;
}
int hitEmitter(vec3 o, vec3 d, float tmax, out float tHit, out vec3 nrm) {   // small dark boxes the emitters sit in
  int best = -1; tHit = tmax; nrm = vec3(0.0, 1.0, 0.0);
  vec3 inv = vec3(abs(d.x) > 1e-9 ? 1.0 / d.x : 1e9, abs(d.y) > 1e-9 ? 1.0 / d.y : 1e9, abs(d.z) > 1e-9 ? 1.0 / d.z : 1e9);
  for (int i = 0; i < ${MAXE}; i++) {
    if (i >= uNE) break;
    vec3 ta = (uEm[i].xyz - uEm[i].w - o) * inv, tb = (uEm[i].xyz + uEm[i].w - o) * inv;
    vec3 tmn = min(ta, tb), tmx = max(ta, tb);
    float tn = max(max(tmn.x, tmn.y), tmn.z), tf = min(min(tmx.x, tmx.y), tmx.z);
    if (tf > max(tn, 0.0) && tn > 1e-4 && tn < tHit) {
      best = i; tHit = tn;
      nrm = tn == tmn.x ? vec3(-sign(d.x), 0.0, 0.0) : (tn == tmn.y ? vec3(0.0, -sign(d.y), 0.0) : vec3(0.0, 0.0, -sign(d.z)));
    }
  }
  return best;
}

// Hot air above heaters. A plume is narrow, so a ray is bent all at once where it passes closest to the plume's axis
// (a thin lens): outward from the hot core and upward, both from the refractive index n = 1 - PA exp(-y/PH - b²/PR²)
// integrated along the ray, plus a wobble for turbulence. The plume sways, so the view through it shimmers.
vec2 plumeSway(int i, float y) { return vec2(sin(1.9 * uTime + 1.1 * y + float(i)), cos(1.3 * uTime + 0.9 * y + 2.0 * float(i))) * (0.12 * PR); }
int hitPlume(vec3 o, vec3 d, int skip, float tmax, out float tHit, out vec3 nd, out float s) {
  int best = -1; tHit = tmax; nd = d; s = 0.0;
  float hl = length(d.xz);
  if (hl < 1e-3) return -1;
  for (int i = 0; i < ${MAXH}; i++) {
    if (i >= uNH) break;
    if (100 + i == skip) continue;
    vec3 c = uHt[i].xyz;
    float t = dot(c.xz - o.xz, d.xz) / (hl * hl);
    if (t <= 1e-4 || t >= tHit) continue;
    vec3 p = o + d * t; float y = p.y - c.y;
    if (y < 0.0 || y > PTOP) continue;
    vec2 q = p.xz - c.xz - plumeSway(i, y); float b = length(q);
    if (b > 2.5 * PR) continue;
    float si = PA * exp(-y / PH - b * b / (PR * PR)) / hl;            // index drop integrated along the chord
    vec2 q1 = b > 1e-5 ? q / b : vec2(0.0);
    vec3 wob = vec3(sin(7.0 * p.z + 3.1 * uTime + 5.0 * y), 0.6 * sin(5.0 * p.x - 2.7 * uTime + 4.0 * y), sin(6.0 * p.x + 2.3 * uTime - 3.0 * y));
    nd = normalize(d + vec3(q1.x, 0.0, q1.y) * (3.545 * si * b / PR) + vec3(0.0, 1.772 * si * PR / PH, 0.0) + wob * (0.25 * PTURB * si));
    best = i; tHit = t; s = si;
  }
  return best;
}

// --- laser helpers
float distPS(vec3 p, vec3 a, vec3 b) {
  vec3 ab = b - a; float t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-12), 0.0, 1.0);
  return length(p - a - ab * t);
}
float segSeg(vec3 p1, vec3 q1, vec3 p2, vec3 q2, out float s, out float t) {   // squared distance between two segments
  vec3 d1 = q1 - p1, d2 = q2 - p2, r = p1 - p2;
  float a = dot(d1, d1), e = dot(d2, d2), f = dot(d2, r);
  s = 0.0;
  if (e < 1e-10) { t = 0.0; s = a > 1e-10 ? clamp(-dot(d1, r) / a, 0.0, 1.0) : 0.0; }
  else if (a < 1e-10) { t = clamp(f / e, 0.0, 1.0); }
  else {
    float b = dot(d1, d2), c = dot(d1, r), den = a * e - b * b;
    s = den > 1e-7 * a * e ? clamp((b * f - c * e) / den, 0.0, 1.0) : 0.0;
    t = (b * s + f) / e;
    if (t < 0.0) { t = 0.0; s = clamp(-c / a, 0.0, 1.0); }
    else if (t > 1.0) { t = 1.0; s = clamp((b - c) / a, 0.0, 1.0); }
  }
  vec3 w = p1 + d1 * s - p2 - d2 * t;
  return dot(w, w);
}
// Glow from laser segments near the straight piece a->b of a pixel's ray (which has travelled t0 so far).
void beamGlow(vec3 a, vec3 b, float t0, float w, inout float hA, inout vec4 hC, inout vec3 core) {
  float len = length(b - a);
  float reach = REACH * max(uBeamR, uPix * (t0 + len));
  vec3 rm = 0.5 * (a + b);
  for (int g = 0; g < ${MAXG}; g++) {
    if (g >= uNG) break;
    if (distPS(uGS[g].xyz, a, b) > uGS[g].w + reach) continue;
    for (int j = 0; j < 8; j++) {
      int i = g * 8 + j;
      if (i >= uNSeg) break;
      vec4 A = texelFetch(uSeg, ivec2(i, 0), 0), B = texelFetch(uSeg, ivec2(i, 1), 0), C = texelFetch(uSeg, ivec2(i, 2), 0);
      float sl = length(B.xyz - A.xyz);
      if (length(0.5 * (A.xyz + B.xyz) - rm) > 0.5 * (sl + len) + reach + C.w + B.w * sl) continue;
      float s, sb; float d2 = segSeg(a, b, A.xyz, B.xyz, s, sb);
      float tc = t0 + s * len, wb = C.w + B.w * sb * sl, sg = max(max(uBeamR, uPix * tc), wb);
      bool soft = wb > uBeamR;                         // widened by hot air: a soft glow, dimmer the wider it gets
      float f = A.w * w * exp(-tc / 300.0) * (soft ? min(1.0, 0.3 / wb) : 1.0);
      float h = f * exp(-d2 / ((soft ? 1.0 : 4.8) * sg * sg));
      if (h < 0.002) continue;
      hA = max(hA, h); hC += vec4(C.rgb, 1.0) * h;       // halo colour: overlapping beams blend by strength
      core = max(core, C.rgb * f * (soft ? 0.0 : exp(-d2 / (sg * sg))));
    }
  }
}
void spotGlow(vec3 hp, float t, float w, inout float hA, inout vec4 hC, inout vec3 core) {   // where beams touch a surface
  float rr0 = max(0.07, 2.5 * uPix * t), fade = w * exp(-t / 300.0);
  for (int k = 0; k < ${MAXS}; k++) {
    if (k >= uNS) break;
    vec3 d = hp - uSpot[k].xyz; float rr = max(rr0, uSpotC[k].w);    // a soft beam's spot is as wide as the beam
    float g = uSpot[k].w * fade * min(1.0, 0.8 / rr) * exp(-dot(d, d) / (rr * rr));
    hA = max(hA, g); hC += vec4(uSpotC[k].rgb, 1.0) * g;
    core = max(core, uSpotC[k].rgb * g * g);
  }
}

struct Ray { vec3 o; vec3 d; vec3 w; float t; int skip; };

void main() {
  vec3 dir = normalize(uCamFwd + vNdc.x * uTanHalf * uAspect * uCamRight + vNdc.y * uTanHalf * uCamUp);
  Ray st[6];
  st[0] = Ray(uCamPos, dir, vec3(1.0), 0.0, -1);
  int sp = 1;
  vec3 col = vec3(0.0), core = vec3(0.0); vec4 hC = vec4(0.0); float hA = 0.0;
  for (int k = 0; k < 10; k++) {
    if (sp <= 0) break;
    sp--;
    Ray r = st[sp];
    float wm = max(r.w.r, max(r.w.g, r.w.b));
    if (r.skip >= 200) {                               // inside glass: on to the far face, then out or reflected back in
      int gi = r.skip - 200; vec3 gn; float R;
      float tg = solidHit(gi, r.o, r.d, true, gn);
      if (uNSeg > 0) beamGlow(r.o, r.o + r.d * tg, r.t, wm, hA, hC, core);
      vec3 hp = r.o + r.d * tg, tr = refractF(r.d, gn, false, R);
      if (R > 0.999) { if (sp < 6) { st[sp] = Ray(hp, tr, r.w, r.t + tg, r.skip); sp++; } continue; }
      if (R * wm > 0.03 && sp < 5) { st[sp] = Ray(hp, reflect(r.d, gn), r.w * R, r.t + tg, r.skip); sp++; }
      if (sp < 6) { st[sp] = Ray(hp, tr, r.w * (1.0 - R), r.t + tg, gi); sp++; }
      continue;
    }
    float tp; vec2 ab;
    int ip = hitPanel(r.o, r.d, r.skip, MAXT, tp, ab);
    float te; vec3 en;
    int ie = hitEmitter(r.o, r.d, tp, te, en);
    float lim = ie >= 0 ? te : tp;
    float th, hs; vec3 hd;
    int ih = uNH > 0 ? hitPlume(r.o, r.d, r.skip, lim, th, hd, hs) : -1;
    if (ih >= 0) lim = th;
    float tv; vec3 nrm; ivec3 hc;
    int m = traceVox(r.o, r.d, lim, tv, nrm, hc);
    float tEnd = m > 0 ? tv : lim;
    if (uNSeg > 0) beamGlow(r.o, r.o + r.d * tEnd, r.t, wm, hA, hC, core);
    float tt = r.t + tEnd, fog = exp(-tt / 170.0);
    if (m > 0) {
      vec3 hp = r.o + r.d * tv;
      if (uNS > 0) spotGlow(hp, tt, wm, hA, hC, core);
      col += r.w * mix(HAZE, shadeVox(m, hp, nrm, hc), fog);
    } else if (ih >= 0) {                              // hot air: a faint warm haze, then on with the bent ray
      col += r.w * vec3(1.0, 0.55, 0.25) * min(1.2 * hs, 0.12) * fog;
      if (sp < 6) { st[sp] = Ray(r.o + r.d * th, hd, r.w, tt, 100 + ih); sp++; }
    } else if (ie >= 0) {
      vec3 ec = vec3(0.16, 0.16, 0.19) * (0.6 + 0.4 * max(dot(en, uSun), 0.0));
      col += r.w * mix(HAZE, ec, fog);
    } else if (ip >= 0) {
      vec3 hp = r.o + r.d * tp, n = uPN[ip].xyz;
      int type = int(uPC[ip].w + 0.5);
      if (uNS > 0) spotGlow(hp, tt, wm, hA, hC, core);
      if (type >= 3) {                                   // glass: partly reflected, the rest refracted in
        vec3 gn; float R; solidHit(ip, r.o, r.d, false, gn);
        vec3 tr = refractF(r.d, gn, true, R), tint = ip == uSel ? vec3(1.0, 0.85, 0.5) : vec3(0.92, 0.98, 0.96);
        if (R * wm > 0.03 && sp < 5) { st[sp] = Ray(hp, reflect(r.d, gn), r.w * R, tt, ip); sp++; }
        if (sp < 6) { st[sp] = Ray(hp, tr, r.w * (1.0 - R) * tint, tt, 200 + ip); sp++; }
        continue;
      }
      if (abs(ab.x) > uPU[ip].w - FRAME || abs(ab.y) > uPN[ip].w - FRAME) {     // the frame is opaque
        vec3 fc = ip == uSel ? vec3(1.0, 0.78, 0.25) : (type == 0 ? vec3(0.34, 0.35, 0.38) : (type == 1 ? vec3(0.25, 0.48, 0.58) : vec3(0.5, 0.32, 0.6)));
        col += r.w * mix(HAZE, fc * (0.65 + 0.35 * abs(dot(n, uSun))), fog);
        continue;
      }
      vec3 rd = reflect(r.d, n);
      if (type == 0) {                                   // mirror
        if (sp < 6) { st[sp] = Ray(hp, rd, r.w * vec3(0.9, 0.92, 0.95), tt, ip); sp++; }
      } else if (type == 1) {                            // half-silvered: both ways, half each
        vec3 w2 = r.w * vec3(0.47, 0.49, 0.5);
        if (sp < 6) { st[sp] = Ray(hp, r.d, w2, tt, ip); sp++; } else col += w2 * HAZE;
        if (sp < 6) { st[sp] = Ray(hp, rd, w2, tt, ip); sp++; } else col += w2 * HAZE;
      } else {                                           // grating: straight-through view plus a rainbow sheen
        col += r.w * 0.14 * spectrum(fract(dot(rd, uPU[ip].xyz) * 2.5 + 0.5)) * fog;
        if (sp < 6) { st[sp] = Ray(hp, r.d, r.w * 0.82, tt, ip); sp++; }
      }
    } else {
      col += r.w * sky(r.d);
    }
  }
  for (int i = 0; i < 6; i++) { if (i >= sp) break; col += st[i].w * HAZE; }   // out of budget: approximate
  col = mix(col, hC.rgb / max(hC.a, 1e-6), clamp(0.9 * hA, 0.0, 1.0));                                // laser halo
  col += mix(core, vec3(max(core.r, max(core.g, core.b))), 0.6);               // white-hot core
  col = col / (1.0 + max(col - 1.0, 0.0) * 0.6);
  outColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}`;

// ---------------------------------------------------------------------------
// Three.js setup
// ---------------------------------------------------------------------------
const canvas = document.getElementById('c');
let renderer;
try { renderer = new THREE.WebGLRenderer({ canvas, antialias: false }); }
catch (e) { const el = document.getElementById('err'); el.style.display = 'flex'; el.textContent = 'WebGL2 is required: ' + e.message; throw e; }
const tex = new THREE.Data3DTexture(vox, NX, NY, NZ);
tex.format = THREE.RedFormat; tex.type = THREE.UnsignedByteType;
tex.minFilter = tex.magFilter = THREE.NearestFilter; tex.unpackAlignment = 1; tex.needsUpdate = true;
const segData = new Float32Array(MAXSEG * 3 * 4);
const segTex = new THREE.DataTexture(segData, MAXSEG, 3, THREE.RGBAFormat, THREE.FloatType);
segTex.minFilter = segTex.magFilter = THREE.NearestFilter; segTex.needsUpdate = true;
const v4s = n => Array.from({ length: n }, () => new THREE.Vector4());
const uniforms = {
  uVox: { value: tex }, uDim: { value: new Int32Array([NX, NY, NZ]) },
  uCamPos: { value: new THREE.Vector3() }, uCamRight: { value: new THREE.Vector3() }, uCamUp: { value: new THREE.Vector3() }, uCamFwd: { value: new THREE.Vector3() },
  uTanHalf: { value: Math.tan(37.5 * Math.PI / 180) }, uAspect: { value: 1 }, uTime: { value: 0 }, uSun: { value: new THREE.Vector3(0.45, 0.62, 0.35).normalize() },
  uPC: { value: v4s(MAXP) }, uPN: { value: v4s(MAXP) }, uPU: { value: v4s(MAXP) }, uNP: { value: 0 }, uSel: { value: -1 },
  uSeg: { value: segTex }, uGS: { value: v4s(MAXG) }, uNSeg: { value: 0 }, uNG: { value: 0 },
  uSpot: { value: v4s(MAXS) }, uSpotC: { value: v4s(MAXS) }, uNS: { value: 0 },
  uEm: { value: v4s(MAXE) }, uNE: { value: 0 }, uHt: { value: v4s(MAXH) }, uNH: { value: 0 },
  uBeamR: { value: 0.012 }, uPix: { value: 0.002 }
};
const mat = new THREE.RawShaderMaterial({ uniforms, vertexShader: vert, fragmentShader: frag, glslVersion: THREE.GLSL3, depthTest: false, depthWrite: false });
const quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2), mat); quad.frustumCulled = false;
const scene = new THREE.Scene(); scene.add(quad);
const cam = new THREE.Camera();

let quality = 'auto';
const bufSize = new THREE.Vector2();
function resize() {
  const dpr = window.devicePixelRatio || 1, w = window.innerWidth, h = window.innerHeight;
  const q = quality === 'auto' ? Math.min(1, Math.sqrt(520000 / (w * h * dpr * dpr))) : +quality;
  renderer.setPixelRatio(dpr * q);
  renderer.setSize(w, h, false);
  uniforms.uAspect.value = w / h;
  renderer.getDrawingBufferSize(bufSize);
  uniforms.uPix.value = 2 * uniforms.uTanHalf.value / Math.max(1, bufSize.y);
}
window.addEventListener('resize', resize);

// ---------------------------------------------------------------------------
// Small vector helpers (plain arrays)
// ---------------------------------------------------------------------------
const add3 = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub3 = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const mul3 = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const dot3 = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross3 = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm3 = a => mul3(a, 1 / Math.hypot(a[0], a[1], a[2]));
const reflect3 = (d, n) => sub3(d, mul3(n, 2 * dot3(d, n)));
const DEG = Math.PI / 180;

// ---------------------------------------------------------------------------
// Optics. Wavelengths in nm (0 = white light). Gratings are thin sinusoidal phase gratings: order m
// carries a fraction J_m(φ)² of the light (J = Bessel function), here with φ = 1.43, which puts about
// 30% straight through, 30% into each first order and 4.6% into each second order.
// ---------------------------------------------------------------------------
const WHITE = [420, 460, 500, 540, 580, 620, 660];
const WHITE_COL = [1, 0.96, 0.88];
function waveRGB(l) {                    // rough visible-spectrum colour, brightest channel = 1
  let r = 0, g = 0, b = 0;
  if (l < 440) { r = (440 - l) / 60; b = 1; } else if (l < 490) { g = (l - 440) / 50; b = 1; }
  else if (l < 510) { g = 1; b = (510 - l) / 20; } else if (l < 580) { r = (l - 510) / 70; g = 1; }
  else if (l < 645) { r = 1; g = (645 - l) / 65; } else r = 1;
  return [r, g, b];
}
const laserRGB = l => l ? (l >= 640 ? [1, 0.06, 0.03] : waveRGB(l)) : WHITE_COL;
function besselJ(m, x) { let s = 0, f = 1; for (let k = 0; k < 20; k++) { if (k > 0) f *= k; let g = 1; for (let i = 2; i <= k + m; i++) g *= i; s += (k % 2 ? -1 : 1) * Math.pow(x / 2, 2 * k + m) / (f * g); } return s; }
const EFF = [0, 1, 2, 3, 4].map(m => besselJ(m, 1.43) ** 2);
const LASER = { maxLen: 400, minI: 0.012, maxDepth: 40, mirror: 0.95 };
const GUN = { right: 0.22, up: -0.2, fwd: 0.3 };      // where your beam leaves, relative to your eye

// Panels: flat rectangles. Orientation as yaw/pitch of the normal (same convention as the camera);
// u is the horizontal edge direction (and the direction a grating spreads light along), v the other edge.
function orient(p) {
  const cp = Math.cos(p.pitch), sp = Math.sin(p.pitch), cy = Math.cos(p.yaw), sy = Math.sin(p.yaw);
  p.n = [sy * cp, sp, -cy * cp]; p.u = [cy, 0, sy]; p.v = cross3(p.n, p.u);
  if (p.sideways) { const u = p.u; p.u = p.v; p.v = mul3(u, -1); }   // grating lines run the other way: spreads up/down
  return p;
}
function makePanel(type, c, n, hu = 0.8, hv = 0.8) {
  n = norm3(n);
  return orient({ type, c, hu, hv, pitch: Math.asin(Math.max(-1, Math.min(1, n[1]))), yaw: Math.atan2(n[0], -n[2]) });
}
// Glass solids, as in the shader: solidFaces lists [outward normal, distance from the centre].
const glassN = lam => GLASS.A + GLASS.B / (lam || 550) ** 2;
function solidFaces(p) {
  const f = [[p.v, p.hv], [mul3(p.v, -1), p.hv]], r = p.hu / Math.sqrt(3);
  if (p.type === 3) return f.concat([[p.u, p.hu], [mul3(p.u, -1), p.hu], [p.n, GLASS.d], [mul3(p.n, -1), GLASS.d]]);
  return f.concat([[mul3(p.n, -1), r], [add3(mul3(p.n, 0.5), mul3(p.u, 0.8660254)), r], [add3(mul3(p.n, 0.5), mul3(p.u, -0.8660254)), r]]);
}
function solidHit(p, o, d, inside) {   // from outside: where the ray enters; from inside: where it leaves
  let t0 = -1e9, t1 = 1e9, n0 = null, n1 = null;
  for (const [N, w] of solidFaces(p)) {
    const dn = dot3(d, N), dist = w - dot3(sub3(o, p.c), N);
    if (Math.abs(dn) < 1e-12) { if (dist < 0) t1 = -1e9; continue; }
    const t = dist / dn;
    if (dn < 0) { if (t > t0) { t0 = t; n0 = N; } } else if (t < t1) { t1 = t; n1 = N; }
  }
  return inside ? { t: t1, n: n1 } : (t0 < t1 && t0 > 1e-4 ? { t: t0, n: n0 } : null);
}
function refractF(d, n, goingIn, N) {   // Snell's law and Fresnel reflectance R; dir is null for total internal reflection
  const eta = goingIn ? 1 / N : N, nf = goingIn ? n : mul3(n, -1), ci = -dot3(d, nf), k = 1 - eta * eta * (1 - ci * ci);
  if (k < 0) return { dir: null, R: 1 };
  const ct = Math.sqrt(k), m = 1 / eta, rs = (ci - m * ct) / (ci + m * ct), rp = (m * ci - ct) / (m * ci + ct);
  return { dir: norm3(add3(mul3(d, eta), mul3(nf, eta * ci - ct))), R: (rs * rs + rp * rp) / 2 };
}
function nearestPanel(o, d, skip, tmax) {
  let best = null;
  state.panels.forEach((p, i) => {
    if (i === skip) return;
    if (p.type >= 3) { const h = solidHit(p, o, d, false); if (h && h.t < (best ? best.t : tmax)) best = { i, t: h.t, a: 0, b: 0 }; return; }
    const den = dot3(d, p.n);
    if (Math.abs(den) < 1e-9) return;
    const t = dot3(sub3(p.c, o), p.n) / den;
    if (t <= 1e-5 || t >= (best ? best.t : tmax)) return;
    const q = sub3(add3(o, mul3(d, t)), p.c), a = dot3(q, p.u), b = dot3(q, p.v);
    if (Math.abs(a) <= p.hu && Math.abs(b) <= p.hv) best = { i, t, a, b };
  });
  return best;
}
function voxelRay(o, d, tmax) {                      // same walk as the shader's traceVox
  const inv = d.map(x => (x >= 0 ? 1 : -1) / Math.max(Math.abs(x), 1e-12)), B = [NX, NY, NZ];   // keeps its sign even for tiny components
  let tn = -Infinity, tf = Infinity;
  for (let k = 0; k < 3; k++) { const a = -o[k] * inv[k], b = (B[k] - o[k]) * inv[k]; tn = Math.max(tn, Math.min(a, b)); tf = Math.min(tf, Math.max(a, b)); }
  if (tf > Math.max(tn, 0) && tn < tmax) {
    const ts = Math.max(tn, 0);
    const c = [0, 1, 2].map(k => Math.min(B[k] - 1, Math.max(0, Math.floor(o[k] + d[k] * (ts + 1e-4)))));
    const st = d.map(x => x >= 0 ? 1 : -1), dl = inv.map(Math.abs);
    const nxt = [0, 1, 2].map(k => (c[k] + (d[k] >= 0 ? 1 : 0) - o[k]) * inv[k]);
    if (tn > 0) { const m = get(c[0], c[1], c[2]); if (m) return { t: tn, m }; }
    for (let i = 0; i < 400; i++) {
      const k = nxt[0] < nxt[1] && nxt[0] < nxt[2] ? 0 : (nxt[1] < nxt[2] ? 1 : 2);
      const t = nxt[k]; c[k] += st[k]; nxt[k] += dl[k];
      if (t > tmax) return null;
      if (c[k] < 0 || c[k] >= B[k]) break;
      const m = get(c[0], c[1], c[2]);
      if (m) return { t, m };
    }
  }
  if (d[1] < -1e-9) { const tg = (1 - o[1]) / d[1]; if (tg > 0 && tg < tmax) return { t: tg, m: M.grass }; }
  return null;
}
function diffract(d, p, lam, m) {              // direction of order m through grating p, or null if it doesn't exist
  const du = dot3(d, p.u) + m * lam * state.lpm * 1e-6, dv = dot3(d, p.v), dn = dot3(d, p.n);
  const r = 1 - du * du - dv * dv;
  if (r <= 1e-6) return null;
  return add3(add3(mul3(p.u, du), mul3(p.v, dv)), mul3(p.n, Math.sign(dn) * Math.sqrt(r)));
}
// Hot air (the shader's hitPlume does the same for the view): where a beam passes closest to a plume's axis it is
// bent outward and upward. The index change is about 1000x real hot air's, so the effect shows across the lab.
const sway = (i, y, t) => [Math.sin(1.9 * t + 1.1 * y + i) * 0.12 * PLUME.R, Math.cos(1.3 * t + 0.9 * y + 2 * i) * 0.12 * PLUME.R];
function nearestPlume(o, d, skip, tmax) {
  const hl = Math.hypot(d[0], d[2]);
  if (hl < 1e-3) return null;
  let best = null;
  state.heaters.forEach((c, i) => {
    if (100 + i === skip) return;
    const ax = c[0] + 0.5, az = c[2] + 0.5, t = ((ax - o[0]) * d[0] + (az - o[2]) * d[2]) / (hl * hl);
    if (t <= 1e-4 || t >= (best ? best.t : tmax)) return;
    const p = add3(o, mul3(d, t)), y = p[1] - c[1] - 1;
    if (y < 0 || y > PLUME.top) return;
    const [sx, sz] = sway(i, y, state.time), qx = p[0] - ax - sx, qz = p[2] - az - sz, b = Math.hypot(qx, qz);
    if (b > 2.5 * PLUME.R) return;
    const si = PLUME.A * Math.exp(-y / PLUME.H - b * b / PLUME.R ** 2) / hl, q1 = b > 1e-5 ? [qx / b, 0, qz / b] : [0, 0, 0];
    best = { i, t, s: si, bend: add3(mul3(q1, 3.545 * si * b / PLUME.R), [0, 1.772 * si * PLUME.R / PLUME.H, 0]) };
  });
  return best;
}
// Follow one laser through mirrors, splitters, gratings, hot air and glass. Returns the straight segments, the points
// where light touches something (spots), and where each branch ends (leaves).
function traceLaser(o, d, lam) {
  const segs = [], spots = [], leaves = [];
  let full = false;
  const stack = [{ o, d: norm3(d), I: 1, lam, col: laserRGB(lam), len: 0, depth: 0, skip: -1 }];
  while (stack.length) {
    if (segs.length >= MAXSEG) { full = true; break; }
    const r = stack.pop();
    if (r.glass >= 0) {          // inside glass: on to the far face, then out (refracted) and partly reflected back in
      const g = state.panels[r.glass], h = solidHit(g, r.o, r.d, true), hp = add3(r.o, mul3(r.d, h.t)), len = r.len + h.t;
      const w1 = (r.w0 || 0) + (r.g || 0) * h.t;
      segs.push({ a: r.o, b: hp, I: r.I, col: r.col, w0: r.w0 || 0, g: r.g || 0 });
      const f = refractF(r.d, h.n, false, glassN(r.lam)), more = r.depth < LASER.maxDepth;
      if (more && r.I * f.R >= LASER.minI) stack.push({ ...r, o: hp, d: reflect3(r.d, h.n), I: r.I * f.R, len, depth: r.depth + 1, w0: w1 });
      if (more && f.dir && r.I * (1 - f.R) >= LASER.minI) stack.push({ ...r, o: hp, d: f.dir, I: r.I * (1 - f.R), len, depth: r.depth + 1, glass: -1, skip: r.glass, w0: w1 });
      continue;
    }
    const P = nearestPanel(r.o, r.d, r.skip, LASER.maxLen - r.len);
    const H = nearestPlume(r.o, r.d, r.skip, P ? P.t : LASER.maxLen - r.len);
    const lim = H ? H.t : (P ? P.t : LASER.maxLen - r.len), V = voxelRay(r.o, r.d, lim);
    const t = V ? V.t : lim, hp = add3(r.o, mul3(r.d, t)), len = r.len + t, w1 = (r.w0 || 0) + (r.g || 0) * t;
    segs.push({ a: r.o, b: hp, I: r.I, col: r.col, w0: r.w0 || 0, g: r.g || 0 });
    const leaf = what => { leaves.push({ I: r.I, depth: r.depth, len, what }); };
    if (V) { spots.push({ p: hp, I: r.I, col: r.col, w: w1 }); leaf(MAT_NAME[V.m]); continue; }
    if (H) {      // bent, and widened by turbulence into a soft glow (spreads from several plumes add in quadrature)
      if (r.depth >= LASER.maxDepth) { leaf('hot air'); continue; }
      const e1 = norm3(cross3(r.d, [0, 1, 0])), e2 = cross3(e1, r.d), a = 0.7 * state.time, wob = 0.3 * PLUME.turb * H.s;
      const dir = norm3(add3(add3(r.d, H.bend), add3(mul3(e1, wob * Math.cos(a)), mul3(e2, wob * Math.sin(a)))));
      stack.push({ ...r, o: hp, d: dir, len, depth: r.depth + 1, skip: 100 + H.i, w0: w1, g: Math.hypot(r.g || 0, PLUME.glow * H.s) });
      continue;
    }
    if (!P) { leaf(null); continue; }
    const p = state.panels[P.i];
    if (Math.abs(P.a) > p.hu - FRAME || Math.abs(P.b) > p.hv - FRAME) { spots.push({ p: hp, I: r.I, col: r.col, w: w1 }); leaf('a panel frame'); continue; }
    spots.push({ p: hp, I: r.I * 0.3, col: r.col, w: w1 });
    const next = (dir, I, extra) => {
      if (I < LASER.minI || r.depth >= LASER.maxDepth) return;
      stack.push({ ...r, o: hp, d: dir, I, len, depth: r.depth + 1, skip: P.i, w0: w1, ...extra });
    };
    if (p.type === 0) next(reflect3(r.d, p.n), r.I * LASER.mirror);
    else if (p.type === 1) { next(r.d, r.I * 0.5); next(reflect3(r.d, p.n), r.I * 0.5); }
    else if (p.type >= 3) {      // glass: partly reflected; the rest refracted in, white light splitting by colour
      const h = solidHit(p, r.o, r.d, false);
      next(reflect3(r.d, h.n), r.I * refractF(r.d, h.n, true, glassN(r.lam)).R);
      for (const l of r.lam ? [r.lam] : WHITE) {
        const f = refractF(r.d, h.n, true, glassN(l));
        next(f.dir, r.I * (1 - f.R) / (r.lam ? 1 : WHITE.length), { glass: P.i, lam: l, col: laserRGB(l) });
      }
    } else {
      for (let m = -4; m <= 4; m++) {
        const e = r.I * EFF[Math.abs(m)];
        if (e < LASER.minI) continue;
        if (m === 0) { next(r.d, e); continue; }                       // straight through: every colour together
        for (const l of r.lam ? [r.lam] : WHITE) {                      // white light fans out by colour
          const dd = diffract(r.d, p, l, m);
          if (dd) next(dd, e, { lam: l, col: laserRGB(l) });
        }
      }
    }
  }
  return { segs, spots, leaves, full };
}

// Hand the beams to the shader: segments (texture, bounding sphere per 8), spots, emitter boxes.
// Displayed brightness is intensity^0.6, so faint branches stay visible.
function uploadLasers(results, emitters) {
  let ns = 0, nsp = 0;
  const wid = [];                                     // each segment's widest point, for the bounding spheres
  for (const res of results) {
    for (const s of res.segs) {
      if (ns >= MAXSEG) break;
      const I = Math.pow(s.I, 0.6);
      segData.set([s.a[0], s.a[1], s.a[2], I], ns * 4);
      segData.set([s.b[0], s.b[1], s.b[2], s.g], (MAXSEG + ns) * 4);              // width grows by g per block
      segData.set([s.col[0], s.col[1], s.col[2], s.w0], (2 * MAXSEG + ns) * 4);   // from w0 at the start
      wid[ns++] = s.w0 + s.g * Math.hypot(...sub3(s.b, s.a));
    }
    for (const s of res.spots) {
      if (nsp >= MAXS) break;
      uniforms.uSpot.value[nsp].set(s.p[0], s.p[1], s.p[2], Math.pow(s.I, 0.6));
      uniforms.uSpotC.value[nsp].set(s.col[0], s.col[1], s.col[2], s.w || 0);
      nsp++;
    }
  }
  const ng = Math.ceil(ns / 8);
  for (let g = 0; g < ng; g++) {
    const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity], pts = [];
    for (let i = g * 8; i < Math.min(ns, g * 8 + 8); i++) for (const row of [0, 1]) {
      const o = (row * MAXSEG + i) * 4, q = [segData[o], segData[o + 1], segData[o + 2]];
      pts.push(q); for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], q[k]); hi[k] = Math.max(hi[k], q[k]); }
    }
    const c = mul3(add3(lo, hi), 0.5);
    let rad = 0; for (const q of pts) rad = Math.max(rad, Math.hypot(...sub3(q, c)));
    rad += Math.max(...wid.slice(g * 8, g * 8 + 8));
    uniforms.uGS.value[g].set(c[0], c[1], c[2], rad + 1e-3);
  }
  const ne = Math.min(MAXE, emitters.length);
  for (let i = 0; i < ne; i++) { const e = emitters[i], c = sub3(e.o, mul3(e.d, 0.17)); uniforms.uEm.value[i].set(c[0], c[1], c[2], 0.14); }
  uniforms.uNSeg.value = ns; uniforms.uNG.value = ng; uniforms.uNS.value = nsp; uniforms.uNE.value = ne;
  segTex.needsUpdate = true;
}
function setHeaters(list) {          // heater blocks go in the voxel grid, their plumes in a uniform array
  for (const c of state.heaters) set(c[0], c[1], c[2], M.air);
  state.heaters = list.slice(0, MAXH);
  state.heaters.forEach((c, i) => { set(c[0], c[1], c[2], M.heater); uniforms.uHt.value[i].set(c[0] + 0.5, c[1] + 1, c[2] + 0.5, 0); });
  uniforms.uNH.value = state.heaters.length;
  tex.needsUpdate = true;
}
function toggleHeater() {            // put a heater on the block you aim at, or take away the heater you aim at
  const { fwd } = camBasis(), o = arr(state.pos), d = arr(fwd), V = voxelRay(o, d, 30);
  if (!V) return;
  const cell = t => o.map((x, k) => Math.floor(x + d[k] * t)), hit = cell(V.t + 1e-3), c = cell(V.t - 1e-3);
  const i = state.heaters.findIndex(h => h.every((x, k) => x === hit[k]));
  if (i >= 0) { setHeaters(state.heaters.filter((_, j) => j !== i)); return; }
  if (state.heaters.length >= MAXH) { say(`At most ${MAXH} heaters: aim at one and press 4 to take it away.`); return; }
  if (!inGrid(c[0], c[1], c[2]) || get(c[0], c[1], c[2])) return;
  setHeaters([...state.heaters, c]);
  say('Placed a heater. Shine a laser through the air just above it.');
}
function uploadPanels() {
  const n = Math.min(MAXP, state.panels.length);
  for (let i = 0; i < n; i++) {
    const p = state.panels[i];
    uniforms.uPC.value[i].set(p.c[0], p.c[1], p.c[2], p.type);
    uniforms.uPN.value[i].set(p.n[0], p.n[1], p.n[2], p.hv);
    uniforms.uPU.value[i].set(p.u[0], p.u[1], p.u[2], p.hu);
  }
  uniforms.uNP.value = n;
}

// ---------------------------------------------------------------------------
// Scenes. Beams run at y = 2.5 unless noted; the white screen is at z = 12.
// ---------------------------------------------------------------------------
function loadScene(mode) {
  const P = [], E = [], H = [];
  const at = p => [p[0] + 0.5, p[1], p[2] + 0.5];      // keep beams in the middle of blocks, not on their edges
  const mir = (c, n, hu, hv) => P.push(makePanel(0, at(c), n, hu, hv));
  const spl = (c, n) => P.push(makePanel(1, at(c), n));
  const grat = (c, n) => P.push(makePanel(2, at(c), n, 0.8, 0.8));
  const emit = (o, d, lam = 650) => E.push({ o: at(o), d: norm3(d), lam });
  if (mode === 'mirrors') {
    // zigzag of 45° mirrors ending on the screen
    emit([14, 2.5, 46], [1, 0, 0]);
    mir([20, 2.5, 46], [-1, 0, -1]); mir([20, 2.5, 40], [1, 0, 1]); mir([26, 2.5, 40], [-1, 0, -1]);
    // periscope over the low wall
    emit([44, 2, 28], [0, 0, -1]);
    mir([44, 2, 24], [0, 1, 1], 0.6, 0.6); mir([44, 6, 24], [0, -1, -1], 0.6, 0.6);
    // hall of mirrors: two big facing mirrors and a beam fired in at an angle
    mir([29, 2.5, 29], [1, 0, 0], 7, 1.5); mir([35, 2.5, 29], [-1, 0, 0], 7, 1.5);
    emit([30, 2.2, 37], [1, 0, -0.3]);
    // corner reflector on the floor, open toward the spawn point
    const C = [46.5, 1.05, 36.5], s = 2.6;
    mir([C[0], C[1] + s / 2, C[2] + s / 2], [1, 0, 0], s / 2, s / 2);
    mir([C[0] - s / 2, C[1], C[2] + s / 2], [0, 1, 0], s / 2, s / 2);
    mir([C[0] - s / 2, C[1] + s / 2, C[2]], [0, 0, 1], s / 2, s / 2);
  } else if (mode === 'splitters') {
    // cascade: one beam into four
    emit([10, 2.5, 46], [1, 0, 0]);
    spl([14, 2.5, 46], [-1, 0, -1]); spl([19, 2.5, 46], [-1, 0, -1]); spl([24, 2.5, 46], [-1, 0, -1]);
    mir([29, 2.5, 46], [-1, 0, -1]);
    // Mach–Zehnder interferometer: split, two paths, recombine into two outputs (screen and a side post)
    emit([32, 2.5, 34], [1, 0, 0]);
    spl([35, 2.5, 34], [-1, 0, -1]); mir([39, 2.5, 34], [-1, 0, -1]);
    mir([35, 2.5, 28], [1, 0, 1]); spl([39, 2.5, 28], [1, 0, 1]);
  } else if (mode === 'diffraction') {
    // a red laser through a grating, and white light through another (higher up, so the patterns don't overlap)
    emit([18, 2.5, 46], [0, 0, -1], 650); grat([18, 2.5, 40], [0, 0, 1]);
    emit([44, 5, 46], [0, 0, -1], 0); grat([44, 5, 40], [0, 0, 1]);
    // a green laser through a grating turned on its side, which spreads light up and down instead
    emit([31, 2, 30], [0, 0, -1], 532);
    const g = makePanel(2, at([31, 2, 26]), [0, 0, 1]); g.sideways = true; P.push(orient(g));
  } else if (mode === 'heat_haze') {
    // a beam through the hot air just above a heater, one higher up the same plume, and a green one off-centre
    H.push([18, 1, 32], [36, 1, 32]);
    emit([18, 2.4, 44], [0, 0, -1]); emit([18, 6, 44], [0, 0, -1]); emit([37, 2.4, 44], [0, 0, -1], 532);
  } else if (mode === 'refraction') {
    // a glass block turned 60° to the beam; white light through a prism at minimum deviation; a long bar that guides
    // a beam by total internal reflection
    emit([12, 2.5, 46], [0, 0, -1]); P.push(makePanel(3, at([12, 2.5, 40]), [0.866, 0, 0.5], 1.2, 1));
    emit([24.69, 3, 36.57], [0.758, 0, -0.652], 0); P.push(makePanel(4, at([30, 3, 32]), [0.909, 0, 0.417], 1, 1));
    emit([37.85, 3.46, 30], [0.766, -0.643, 0]); P.push(makePanel(3, at([44, 2.5, 30]), [0, 0, 1], 5, 0.4));
  } else {
    // white light softened by hot air, then through a prism at minimum deviation; a softened beam into a glass bar
    H.push([16, 1, 31], [37, 1, 28]);
    emit([16, 6, 36], [0, 0, -1], 0); P.push(makePanel(4, at([16, 6, 26]), [-0.909, 0, -0.417], 1, 1));
    emit([35.94, 5.07, 28], [0.766, -0.643, 0]); P.push(makePanel(3, at([44, 2.5, 28]), [0, 0, 1], 5, 0.4));
  }
  setHeaters(H);
  state.panels = P;
  state.pins = E;
}

// ---------------------------------------------------------------------------
// State, camera
// ---------------------------------------------------------------------------
const V3 = THREE.Vector3;
const state = { mode: 'mirrors', pos: new V3(), yaw: 0, pitch: 0, laser: true, pins: [], panels: [], lam: 650, lpm: 600, heaters: [], time: 0, grab: -1, aimed: -1, msg: '', msgT: 0 };
function resetView() { state.pos.set(32.5, 7, 55); state.yaw = 0; state.pitch = -0.32; }
function camBasis() {
  const cp = Math.cos(state.pitch), sp = Math.sin(state.pitch), cy = Math.cos(state.yaw), sy = Math.sin(state.yaw);
  const fwd = new V3(sy * cp, sp, -cy * cp), right = new V3(cy, 0, sy), up = new V3().crossVectors(right, fwd);
  return { fwd, right, up };
}
const arr = v => [v.x, v.y, v.z];
function gunPose() {
  const { fwd, right, up } = camBasis();
  return { o: arr(state.pos.clone().addScaledVector(right, GUN.right).addScaledVector(up, GUN.up).addScaledVector(fwd, GUN.fwd)), d: arr(fwd) };
}
function aimedPanel() {
  const { fwd } = camBasis(), o = arr(state.pos), d = arr(fwd);
  const P = nearestPanel(o, d, -1, 80);
  if (!P) return -1;
  return voxelRay(o, d, P.t) ? -1 : P.i;
}
function say(text) { state.msg = text; state.msgT = 3; }
function placePanel(type) {
  if (type === 1 && !MODES[state.mode].splitter) return;
  if (type === 2 && !MODES[state.mode].grating) return;
  if (type >= 3 && !MODES[state.mode].glass) return;
  if (state.panels.length >= MAXP) { say(`At most ${MAXP} panels: delete one first (X).`); return; }
  const { fwd } = camBasis(), o = arr(state.pos), d = arr(fwd);
  const P = nearestPanel(o, d, -1, 10), V = voxelRay(o, d, 10);
  const free = Math.min(P ? P.t : 10, V ? V.t : 10);
  const dist = Math.max(1.2, Math.min(3.5, free - 0.9));
  state.panels.push(orient({ type, c: add3(o, mul3(d, dist)), hu: 0.8, hv: 0.8, yaw: state.yaw + Math.PI, pitch: -state.pitch }));   // facing you
  say(`Placed a ${TYPE_NAME[type]} facing you: aim at it and hold T (or the right mouse button) to turn it.`);
}
function rotatePanel(i, dyaw, dpitch) {
  const p = state.panels[i];
  if (!p) return;
  p.yaw += dyaw; p.pitch = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, p.pitch + dpitch));
  orient(p);
}
function pinLaser() {
  const g = gunPose();
  state.pins.push({ o: g.o, d: g.d, lam: state.lam });
  while (state.pins.length > MAXPINS) state.pins.shift();
}
function setLaser(on) { state.laser = on; $('bLaser').textContent = 'Laser: ' + (on ? 'on' : 'off'); $('bLaser').classList.toggle('on', on); }
function setMode(mode) {
  if (!MODES[mode]) mode = 'mirrors';
  state.mode = mode;
  document.body.classList.toggle('no-split', !MODES[mode].splitter);
  document.body.classList.toggle('no-grat', !MODES[mode].grating);
  document.body.classList.toggle('no-heat', !MODES[mode].heat);
  document.body.classList.toggle('no-glass', !MODES[mode].glass);
  document.querySelector(`a[href="${mode}.html"]`).classList.add('cur');
  $('tipsBody').innerHTML = TIPS[mode];
  if (!MODES[mode].grating) { state.lam = 650; $('colour').value = '650'; }
  loadScene(mode); resetView();
}
let liveRes = null;
function laserText(r) {
  if (!r) return 'off';
  const L = r.leaves;
  if (!L.length) return 'fades out';
  const end = l => l.what ? `hits ${l.what} after ${l.len.toFixed(1)} blocks` : 'flies off into the sky';
  if (L.length === 1) { const n = L[0].depth; return `${n ? `${n} bounce${n > 1 ? 's' : ''}, then ` : ''}${end(L[0])}`; }
  const b = L.reduce((a, l) => l.I > a.I ? l : a, L[0]);
  return `splits into ${L.length} beams${r.full ? ' (beam budget reached)' : ''}; the brightest (${(100 * b.I).toFixed(0)}%) ${end(b)}`;
}
function aimText(i) {
  const p = state.panels[i];
  if (!p) return '–';
  const yaw = ((p.yaw / DEG) % 360 + 360) % 360;
  return `${TYPE_NAME[p.type]} · turned ${yaw.toFixed(0)}°, tilted ${(p.pitch / DEG).toFixed(0)}°`;
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------
const keys = {};
const $ = id => document.getElementById(id);
let rmb = false;
function startGrab() { if (state.grab < 0) state.grab = state.aimed; }
function endGrab() { if (!keys.KeyT && !rmb) state.grab = -1; }
window.addEventListener('keydown', e => {
  if (e.target.tagName === 'INPUT' || e.target.tagName === 'SELECT') return;
  keys[e.code] = true;
  const step = (e.shiftKey ? 1 : 5) * DEG;
  if (e.code === 'KeyJ') rotatePanel(state.aimed, -step, 0);
  if (e.code === 'KeyK') rotatePanel(state.aimed, step, 0);
  if (e.code === 'KeyU') rotatePanel(state.aimed, 0, -step);
  if (e.code === 'KeyI') rotatePanel(state.aimed, 0, step);
  if (e.repeat) return;
  if (e.code === 'KeyT') startGrab();
  if (e.code === 'Digit1') placePanel(0);
  if (e.code === 'Digit2') placePanel(1);
  if (e.code === 'Digit3') placePanel(2);
  if (e.code === 'KeyX' && state.aimed >= 0) { state.panels.splice(state.aimed, 1); state.grab = -1; }
  if (e.code === 'KeyC') { state.panels = []; state.grab = -1; }
  if (e.code === 'KeyL') setLaser(!state.laser);
  if (e.code === 'KeyR') pinLaser();
  if (e.code === 'KeyZ') state.pins = [];
  if (e.code === 'Digit4' && MODES[state.mode].heat) toggleHeater();
  if (e.code === 'Digit5') placePanel(3);
  if (e.code === 'Digit6') placePanel(4);
  if (e.code === 'KeyN' && MODES[state.mode].grating) { const s = $('colour'); s.selectedIndex = (s.selectedIndex + 1) % s.options.length; state.lam = +s.value; }
  if (['Space', 'ArrowUp', 'ArrowDown'].includes(e.code)) e.preventDefault();
});
window.addEventListener('keyup', e => { keys[e.code] = false; if (e.code === 'KeyT') endGrab(); });
window.addEventListener('blur', () => { for (const k in keys) keys[k] = false; rmb = false; state.grab = -1; });
const SENS = 0.0025; let dragging = false;
function look(dx, dy) {
  if (state.grab >= 0) { rotatePanel(state.grab, dx * 0.004, -dy * 0.004); return; }   // turning a panel instead of looking
  state.yaw += dx * SENS; state.pitch = Math.max(-1.55, Math.min(1.55, state.pitch - dy * SENS));
}
canvas.addEventListener('contextmenu', e => e.preventDefault());
canvas.addEventListener('pointerdown', e => {
  if (e.button === 2) { rmb = true; startGrab(); }
  if (document.pointerLockElement === canvas) { if (e.button === 0) pinLaser(); return; }   // captured: click leaves an emitter
  dragging = true; try { canvas.setPointerCapture(e.pointerId); } catch (_) {}
  if (e.pointerType === 'mouse' && e.button === 0) { try { const p = canvas.requestPointerLock(); if (p && p.catch) p.catch(() => {}); } catch (_) {} }
});
window.addEventListener('pointerup', e => { if (e.button === 2) { rmb = false; endGrab(); } dragging = false; });
canvas.addEventListener('pointermove', e => { if (document.pointerLockElement === canvas || dragging) look(e.movementX || 0, e.movementY || 0); });

$('colour').addEventListener('change', e => { state.lam = +e.target.value; });
$('lpm').addEventListener('input', e => { state.lpm = +e.target.value; $('lpmval').textContent = state.lpm; });
$('quality').addEventListener('change', e => { quality = e.target.value; resize(); });
$('bLaser').addEventListener('click', () => setLaser(!state.laser));
$('bPin').addEventListener('click', pinLaser);
$('bUnpin').addEventListener('click', () => { state.pins = []; });
$('bScene').addEventListener('click', () => loadScene(state.mode));
$('bView').addEventListener('click', resetView);
$('tipsToggle').addEventListener('click', () => { const b = $('tipsBody'), hidden = b.style.display === 'none'; b.style.display = hidden ? '' : 'none'; $('tipsToggle').textContent = hidden ? 'hide' : 'show'; });

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------
let last = performance.now(), hudT = 0, frames = 0, t0 = performance.now();
function frame(now) {
  const dt = Math.min(0.05, (now - last) / 1000); last = now;
  const { right } = camBasis();
  const flat = new V3(Math.sin(state.yaw), 0, -Math.cos(state.yaw));
  const speed = (keys.ShiftLeft || keys.ShiftRight) ? 18 : 7;
  const mv = new V3();
  if (keys.KeyW || keys.ArrowUp) mv.add(flat); if (keys.KeyS || keys.ArrowDown) mv.sub(flat);
  if (keys.KeyD) mv.add(right); if (keys.KeyA) mv.sub(right);
  if (keys.KeyE || keys.Space) mv.y += 1; if (keys.KeyQ) mv.y -= 1;
  if (keys.ArrowLeft) state.yaw -= 2 * dt; if (keys.ArrowRight) state.yaw += 2 * dt;
  if (mv.lengthSq() > 0) state.pos.addScaledVector(mv.normalize(), speed * dt);

  state.aimed = aimedPanel();
  uploadPanels();
  liveRes = null;
  state.time = now / 1000;
  if (state.laser) { const g = gunPose(); liveRes = traceLaser(g.o, g.d, state.lam); }
  const results = liveRes ? [liveRes] : [];
  for (let i = state.pins.length - 1; i >= 0; i--) results.push(traceLaser(state.pins[i].o, state.pins[i].d, state.pins[i].lam));
  uploadLasers(results, state.pins);

  const cb = camBasis();
  uniforms.uCamPos.value.copy(state.pos); uniforms.uCamFwd.value.copy(cb.fwd); uniforms.uCamRight.value.copy(cb.right); uniforms.uCamUp.value.copy(cb.up);
  uniforms.uSel.value = state.grab >= 0 ? state.grab : state.aimed;
  uniforms.uTime.value = now / 1000;
  renderer.render(scene, cam);

  frames++;
  state.msgT -= dt;
  if ((hudT += dt) > 0.2) {
    hudT = 0;
    $('sP').textContent = state.panels.length; $('sE').textContent = state.pins.length;
    $('sLaser').textContent = laserText(liveRes);
    $('sAim').textContent = aimText(state.grab >= 0 ? state.grab : state.aimed);
    $('sMsg').textContent = state.msgT > 0 ? state.msg : '';
    if (now - t0 > 500) { $('sFps').textContent = (frames * 1000 / (now - t0)).toFixed(0); frames = 0; t0 = now; }
  }
  requestAnimationFrame(frame);
}
setMode(document.body.dataset.demo);
resize();
requestAnimationFrame(frame);
window.__demo = { state, traceLaser, loadScene, setMode, setHeaters, toggleHeater, placePanel, rotatePanel, pinLaser, resetView, camBasis, uniforms, renderer, scene, cam, frame, EFF };
})();
