// GLSL for the portal ray tracer.
//
// Every pixel is a light ray traced backwards from the eye. Everything happens in voxel
// coordinates, a rescaled projective chart in which light travels in straight lines even when
// space is curved (see math/geometry.js). When a ray reaches a glued face it continues from the
// partner face, its position and direction mapped by that face's 4x4 portal map, so what you
// see is exactly what an inhabitant of the glued-up space would see. Walls stop the ray, mirrors
// reflect it. True distances (for the fog) use the curved metric.

export const MAX_SELF_IMAGES = 4;

export const vertexShader = /* glsl */ `
precision highp float;
in vec3 position;
out vec2 vNdc;
void main() { vNdc = position.xy; gl_Position = vec4(position.xy, 0.0, 1.0); }`;

export const fragmentShader = /* glsl */ `
precision highp float; precision highp int; precision highp sampler3D;

uniform sampler3D uVox;
uniform float uN;
uniform vec3 uBoxMin, uBoxMax;                     // bounding box of all blocks
uniform vec3 uPalette[16];

uniform vec3 uCamPos, uCamRight, uCamUp, uCamFwd;
uniform float uTanHalf, uAspect;

// faces: 0 +x, 1 -x, 2 +y, 3 -y, 4 +z, 5 -z
uniform float uFaceKind[6];                        // 0 wall, 1 glued, 2 mirror
uniform mat4 uPortal[6];                           // homogeneous voxel coordinates
uniform float uTarget[6];                          // the face a ray comes back in through
uniform vec3 uFaceColor[6];
uniform vec3 uFaceU[6], uFaceV[6];                 // face frames (seen from outside)
uniform float uGlyphCount[6];
uniform vec4 uGlyphInv[12];                        // 2 per face: inverse 2x2 maps (row-major)

uniform vec3 uEdgeColor[24];                       // per half-edge of the cube frame
uniform float uEdgeSingular[24];

uniform vec3 uSelfPos[${MAX_SELF_IMAGES}];
uniform mat3 uSelfFrame[${MAX_SELF_IMAGES}];       // voxel displacement per block along right, up, back
uniform mat3 uSelfInverse[${MAX_SELF_IMAGES}];
uniform int uSelfCount;

uniform float uGeometry;                           // 0 flat, 1 spherical, -1 hyperbolic
uniform float uChartA, uCurvatureR;                // cube half-width in the chart; radius in blocks

uniform float uShowFrame, uShowSeams, uShowSelf;
uniform float uFog;                                // e-folding distance in blocks
uniform vec3 uFogColor, uSun;

in vec2 vNdc;
out vec4 outColor;

const int MAX_SEGMENTS = 64;
const int MAX_DDA = 72;
const float BEAM_R = 0.09;
const vec3 SELF_HALF = vec3(0.26, 0.28, 0.26);
const vec2 GLYPH_AT = vec2(0.55, 0.22);
const float GLYPH_SIZE = 0.2;

// True distance in blocks between two points given in voxel coordinates.
float trueDistance(vec3 p, vec3 q) {
  if (uGeometry == 0.0) return length(p - q);
  vec4 P = vec4((p * (2.0 / uN) - 1.0) * uChartA, 1.0), Q = vec4((q * (2.0 / uN) - 1.0) * uChartA, 1.0);
  if (uGeometry > 0.0) return uCurvatureR * acos(clamp(dot(P, Q) / (length(P) * length(Q)), -1.0, 1.0));
  float pq = dot(P.xyz, Q.xyz) - P.w * Q.w, pp = dot(P.xyz, P.xyz) - P.w * P.w, qq = dot(Q.xyz, Q.xyz) - Q.w * Q.w;
  return uCurvatureR * acosh(max(1.0, -pq / sqrt(pp * qq)));
}

float hash3(vec3 p) { return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453); }

vec3 safeInv(vec3 v) {
  return vec3(abs(v.x) > 1e-9 ? 1.0 / v.x : 1e30, abs(v.y) > 1e-9 ? 1.0 / v.y : 1e30, abs(v.z) > 1e-9 ? 1.0 / v.z : 1e30);
}

float faceLight(vec3 n) {
  float face = n.y > 0.5 ? 1.0 : (n.y < -0.5 ? 0.55 : (abs(n.x) > 0.5 ? 0.82 : 0.68));
  return face * (0.8 + 0.2 * max(dot(n, uSun), 0.0));
}

// ---- blocks ---------------------------------------------------------------
int voxelAt(ivec3 c) { return int(texelFetch(uVox, c, 0).r * 255.0 + 0.5); }

vec3 shadeVoxel(int m, vec3 hp, vec3 n, ivec3 cell) {
  vec3 base = uPalette[m];
  if (m == 1 && n.y < 0.5) base = fract(hp.y) > 0.8 ? uPalette[1] : uPalette[2];   // grass block sides
  vec2 uv = abs(n.x) > 0.5 ? hp.zy : (abs(n.y) > 0.5 ? hp.xz : hp.xy);
  vec2 f = fract(uv);
  base *= 0.86 + 0.22 * hash3(vec3(floor(f * 8.0), float(cell.x * 7 + cell.y * 131 + cell.z * 29)));
  vec2 e2 = min(f, 1.0 - f);
  base *= mix(0.6, 1.0, smoothstep(0.0, 0.04, min(e2.x, e2.y)));
  if (m == 7) base = mix(base, vec3(0.95), 0.2);
  return base * faceLight(n);
}

// Ray against the block bounding box: (tNear, tFar), entry axis in .z (-1 if starting inside).
vec3 boxHit(vec3 p, vec3 inv, vec3 bmin, vec3 bmax) {
  vec3 t0 = (bmin - p) * inv, t1 = (bmax - p) * inv;
  vec3 tn = min(t0, t1), tf = max(t0, t1);
  float tNear = max(max(tn.x, tn.y), tn.z), tFar = min(min(tf.x, tf.y), tf.z);
  float axis = tn.x >= tn.y && tn.x >= tn.z ? 0.0 : (tn.y >= tn.z ? 1.0 : 2.0);
  return vec3(tNear, tFar, tNear > 0.0 ? axis : -1.0);
}

// March the voxel grid along the ray between tStart and tEnd. Returns hit distance (or -1).
float marchBlocks(vec3 p, vec3 v, float tStart, float tEnd, int entryAxis, out vec3 colour) {
  vec3 inv = safeInv(v);
  vec3 q = p + v * tStart;
  ivec3 cell = clamp(ivec3(floor(q + v * 1e-4)), ivec3(0), ivec3(int(uN) - 1));
  ivec3 stp = ivec3(sign(v));
  vec3 tMax = (vec3(cell) + step(0.0, v) - p) * inv;
  vec3 tDelta = abs(inv);
  float t = tStart;
  int axis = entryAxis;
  for (int i = 0; i < MAX_DDA; i++) {
    int m = voxelAt(cell);
    if (m > 0) {
      vec3 hp = p + v * t;
      vec3 n = vec3(0.0);
      if (axis >= 0) n[axis] = -float(stp[axis]);
      colour = axis >= 0 ? shadeVoxel(m, hp, n, cell) : uPalette[m] * 0.3;
      return t;
    }
    if (tMax.x < tMax.y && tMax.x < tMax.z) { t = tMax.x; tMax.x += tDelta.x; cell.x += stp.x; axis = 0; }
    else if (tMax.y < tMax.z) { t = tMax.y; tMax.y += tDelta.y; cell.y += stp.y; axis = 1; }
    else { t = tMax.z; tMax.z += tDelta.z; cell.z += stp.z; axis = 2; }
    if (t > tEnd) break;
    if (any(lessThan(cell, ivec3(0))) || any(greaterThanEqual(cell, ivec3(int(uN))))) break;
  }
  return -1.0;
}

// ---- the cube's frame: a thin beam along each of the 12 edges -------------
float frameHit(vec3 p, vec3 v, float tMaxHit, out vec3 colour) {
  float best = -1.0;
  for (int e = 0; e < 12; e++) {
    int k = e / 4, a = (k + 1) % 3, b = (k + 2) % 3;
    vec2 q = vec2(p[a] - (((e / 2) % 2 == 1) ? uN : 0.0), p[b] - ((e % 2 == 1) ? uN : 0.0));
    vec2 d = vec2(v[a], v[b]);
    float A = dot(d, d), B = dot(q, d), C = dot(q, q) - BEAM_R * BEAM_R;
    if (C < 0.0 || A < 1e-12) continue;          // we're inside this beam: ignore it
    float disc = B * B - A * C;
    if (disc < 0.0) continue;
    float t = (-B - sqrt(disc)) / A;
    if (t <= 0.0 || t >= tMaxHit || (best >= 0.0 && t >= best)) continue;
    best = t;
    vec3 hp = p + v * t;
    vec2 n2 = normalize(q + d * t);
    vec3 n = vec3(0.0); n[a] = n2.x; n[b] = n2.y;
    int half_ = hp[k] > 0.5 * uN ? 1 : 0;
    int idx = e * 2 + half_;
    vec3 c = uEdgeColor[idx];
    if (uEdgeSingular[idx] > 0.5 && fract(hp[k] * 0.75) < 0.5) c *= 0.25;    // dashed = singular
    colour = c * (0.55 + 0.45 * max(dot(n, uSun), 0.0) + 0.15 * n.y);
  }
  return best;
}

// ---- yourself: a little block figure. Its right side is red and its left side blue, so a
// mirror image of you is easy to spot. Local frame: x right, y up, z back (camera at the head).
float localBox(vec3 o, vec3 d, vec3 centre, vec3 half_, out vec3 normal, out vec3 local) {
  vec3 inv = safeInv(d);
  vec3 t0 = (centre - half_ - o) * inv, t1 = (centre + half_ - o) * inv;
  vec3 tn = min(t0, t1), tf = max(t0, t1);
  float tNear = max(max(tn.x, tn.y), tn.z), tFar = min(min(tf.x, tf.y), tf.z);
  if (tNear > tFar || tNear <= 0.0) return -1.0;
  int ax = tn.x >= tn.y && tn.x >= tn.z ? 0 : (tn.y >= tn.z ? 1 : 2);
  normal = vec3(0.0); normal[ax] = -sign(d[ax]);
  local = o + d * tNear - centre;
  return tNear;
}

vec3 headColour(vec3 n, vec3 lp) {
  if (n.x > 0.5) return vec3(0.9, 0.2, 0.2);      // right: red
  if (n.x < -0.5) return vec3(0.2, 0.4, 0.95);    // left: blue
  if (n.y > 0.5) return vec3(0.95, 0.8, 0.3);
  if (n.y < -0.5) return vec3(0.4);
  if (n.z > 0.5) return vec3(0.35, 0.28, 0.25);   // back of the head
  vec2 f = lp.xy;                                 // face, with a lopsided smile
  if (length(f - vec2(-0.09, 0.06)) < 0.04 || length(f - vec2(0.09, 0.06)) < 0.04) return vec3(0.1);
  if (abs(f.x + 0.03) < 0.1 && abs(f.y + 0.11) < 0.022) return vec3(0.6, 0.15, 0.15);
  return vec3(0.96, 0.8, 0.66);
}

vec3 bodyColour(vec3 n) {
  if (n.x > 0.5) return vec3(0.85, 0.25, 0.22);
  if (n.x < -0.5) return vec3(0.22, 0.38, 0.9);
  return vec3(0.25, 0.65, 0.62);
}

float selfHit(vec3 p, vec3 v, float tMaxHit, bool skipFirst, out vec3 colour) {
  float best = -1.0;
  for (int i = 0; i < ${MAX_SELF_IMAGES}; i++) {
    if (i >= uSelfCount) break;
    if (i == 0 && skipFirst) continue;   // that's the copy you are looking out of
    mat3 F = uSelfFrame[i], Fi = uSelfInverse[i];
    vec3 o = Fi * (p - uSelfPos[i]), d = Fi * v, n, lp;
    float limit = best >= 0.0 ? best : tMaxHit;
    float t = localBox(o, d, vec3(0.0), SELF_HALF, n, lp);
    if (t > 0.0 && t < limit) { best = limit = t; colour = headColour(n, lp) * faceLight(normalize(F * n)); }
    t = localBox(o, d, vec3(0.0, -0.78, 0.04), vec3(0.3, 0.46, 0.17), n, lp);
    if (t > 0.0 && t < limit) { best = t; colour = bodyColour(n) * faceLight(normalize(F * n)); }
  }
  return best;
}

// ---- faces of the cube ------------------------------------------------------
vec2 faceCoords(int f, vec3 hp) {          // [-1, 1]^2 in the face's frame
  vec3 c = hp - vec3(0.5 * uN);
  return vec2(dot(c, uFaceU[f]), dot(c, uFaceV[f])) / (0.5 * uN);
}

bool inGlyph(vec2 g) {                     // the letter F, in its own coordinates
  vec2 l = (g - GLYPH_AT) / GLYPH_SIZE;
  bool stem = l.x > -0.5 && l.x < -0.2 && l.y > -0.8 && l.y < 0.8;
  bool top = l.x > -0.5 && l.x < 0.6 && l.y > 0.5 && l.y < 0.8;
  bool mid = l.x > -0.5 && l.x < 0.3 && l.y > -0.05 && l.y < 0.25;
  return stem || top || mid;
}

bool glyphAt(int f, vec2 uv) {
  for (int k = 0; k < 2; k++) {
    if (float(k) >= uGlyphCount[f]) break;
    vec4 m = uGlyphInv[f * 2 + k];
    if (inGlyph(vec2(m.x * uv.x + m.y * uv.y, m.z * uv.x + m.w * uv.y))) return true;
  }
  return false;
}

float gridLine(vec2 blocks, float w) {
  vec2 g = abs(fract(blocks) - 0.5);
  return 1.0 - smoothstep(0.5 - w, 0.5, max(g.x, g.y));
}

vec3 shadeWall(int f, vec3 hp) {
  vec2 uv = faceCoords(f, hp);
  vec2 blocks = (uv + 1.0) * 0.5 * uN;
  vec3 base = f == 3 ? vec3(0.42, 0.62, 0.30) : mix(vec3(0.72, 0.72, 0.74), uFaceColor[f], 0.25);
  base *= 0.9 + 0.1 * hash3(vec3(floor(blocks), float(f)));
  base *= mix(1.0, 0.7, gridLine(blocks, 0.06));
  int axis = f / 2;
  vec3 n = vec3(0.0); n[axis] = (f % 2 == 0) ? -1.0 : 1.0;   // facing into the cube
  return base * faceLight(n);
}

// ---------------------------------------------------------------------------
void main() {
  vec3 p = uCamPos;
  vec3 v = normalize(uCamFwd + vNdc.x * uTanHalf * uAspect * uCamRight + vNdc.y * uTanHalf * uCamUp);
  vec3 acc = vec3(0.0);       // light already collected from translucent seams
  float trans = 1.0;          // how much of what lies further on still gets through
  float dist = 0.0;           // distance travelled so far (for fog)
  vec3 surface = uFogColor;
  int entryAxis = -1;

  for (int seg = 0; seg < MAX_SEGMENTS; seg++) {
    vec3 inv = safeInv(v);
    vec3 tFace = (step(0.0, v) * uN - p) * inv;
    float tExit = max(min(min(tFace.x, tFace.y), tFace.z), 0.0);
    int exitAxis = tFace.x <= tFace.y && tFace.x <= tFace.z ? 0 : (tFace.y <= tFace.z ? 1 : 2);

    float tHit = 1e30; vec3 hitColour = vec3(0.0), c;
    vec3 bh = boxHit(p, inv, uBoxMin, uBoxMax);
    if (bh.x <= bh.y && bh.y > 0.0 && bh.x < tExit) {
      float t0 = max(bh.x, 0.0);
      float t = marchBlocks(p, v, t0, min(bh.y, tExit), bh.x > 0.0 ? int(bh.z) : entryAxis, c);
      if (t >= 0.0) { tHit = t; hitColour = c; }
    }
    if (uShowFrame > 0.5) {
      float t = frameHit(p, v, min(tHit, tExit), c);
      if (t >= 0.0) { tHit = t; hitColour = c; }
    }
    if (uShowSelf > 0.5) {
      float t = selfHit(p, v, min(tHit, tExit), seg == 0, c);
      if (t >= 0.0) { tHit = t; hitColour = c; }
    }
    if (tHit < 1e29) { dist += trueDistance(p, p + v * tHit); surface = hitColour; break; }

    // reached a face of the cube
    dist += trueDistance(p, p + v * tExit);
    p += v * tExit;
    int face = exitAxis * 2 + (v[exitAxis] > 0.0 ? 0 : 1);
    float fogLeft = exp(-dist / uFog);
    if (trans * fogLeft < 0.01) { surface = uFogColor; break; }
    float kind = uFaceKind[face];
    if (kind < 0.5) { surface = shadeWall(face, p); break; }
    if (kind > 1.5) {
      vec3 tint = vec3(0.75, 0.85, 1.0);
      acc += trans * 0.06 * mix(uFogColor, tint, fogLeft);
      trans *= 0.86;
    } else if (uShowSeams > 0.5) {
      vec2 uv = faceCoords(face, p);
      float a = 0.035 + 0.1 * gridLine((uv + 1.0) * 0.5 * uN, 0.04);
      if (glyphAt(face, uv)) a = 0.55;
      acc += trans * a * mix(uFogColor, uFaceColor[face], fogLeft);
      trans *= 1.0 - a;
    }
    // go through: continue from the partner face. The portal map is projective, so map the
    // point, and differentiate it to map the direction.
    vec4 Y = uPortal[face] * vec4(p, 1.0);
    vec4 dY = uPortal[face] * vec4(v, 0.0);
    p = clamp(Y.xyz / Y.w, vec3(0.0), vec3(uN));
    v = normalize(dY.xyz * Y.w - Y.xyz * dY.w);
    entryAxis = int(uTarget[face]) / 2;
    if (seg == MAX_SEGMENTS - 1) surface = uFogColor;
  }

  vec3 col = acc + trans * mix(uFogColor, surface, exp(-dist / uFog));
  outColor = vec4(col, 1.0);
}`;
