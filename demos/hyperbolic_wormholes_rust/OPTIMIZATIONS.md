# Making the Metal ray tracer fast

This file explains, in order and with the maths, every optimization tried on the Rust + Metal hyperbolic wormholes
renderer, kept or not, and what each one measured. The code is in PR 72, which builds on the Rust port (PR 69) and
the Metal port (PR 70). [`DECISIONS.md`](DECISIONS.md) records the same choices in the repo's decision template; this
file is about the mathematics behind them.

All times are milliseconds per frame on an Apple M3 Pro (macOS 15.7.3, Rust 1.98.1, release build), shared with other
work, so differences under about 15% are noise. Unless a line says otherwise, the time is the median frame for the
spawn view at 2560×1600, tracing every pixel at full resolution. "px" is how far image features moved against a
reference, defined in [section 4](#4-how-accuracy-is-measured).

## 1. What a frame computes

### 1.1 Space, points and distances

Space is hyperbolic space H³ with curvature radius $R = 8$ blocks, in the hyperboloid model. A point is a 4-vector
$x = (x_1, x_2, x_3, x_w)$ with $\langle x, x \rangle = -1$ and $x_w > 0$, where

```math
\langle a, b \rangle = a_1 b_1 + a_2 b_2 + a_3 b_3 - a_w b_w .
```

The distance between two points is $d = R \operatorname{arcosh}(-\langle a, b \rangle)$. A unit tangent vector $v$ at
$x$ has $\langle v, v \rangle = 1$ and $\langle x, v \rangle = 0$, and the geodesic from $x$ along $v$ is

```math
x(s) = x \cosh(s/R) + v \sinh(s/R) .
```

With $\tau = \tanh(s/R)$, the point $x + \tau v$ is $x(s)$ up to a positive factor, so a geodesic is a straight line
in these coordinates, and every intersection below is solved in $\tau$.

Comparing distances needs no `asinh`. The squared Minkowski length of the difference,

```math
q(a, b) = \langle a - b, a - b \rangle = -2 - 2 \langle a, b \rangle = 2(\cosh(d/R) - 1) = 4 \sinh^2\!\left(\frac{d}{2R}\right),
```

grows with $d$, so $d < d_0$ exactly when $q < 4 \sinh^2(d_0/2R)$. Computing it from the difference $a - b$ instead of
from $\langle a, b \rangle$ keeps its precision in 32-bit floats when the points are close.

### 1.2 Light paths

Each mouth $k$, at centre $c_k$, bends light like a lens. At distance $\rho$ from $c_k$ (in curvature radii), with
$\cosh \rho = 1 + q/2$ and $\sinh \rho = \sqrt{q(1 + q/4)}$, the lensing law works in an effective flat radius

```math
\rho_\text{eff} = R \sinh\rho \, e^{\rho} = \tfrac{R}{2}\left(e^{2\rho} - 1\right),
```

and the gradient of the log refractive index is

```math
g_k(x) = f(\rho_\text{eff}) \, e^{2\rho} \, \hat r_k, \qquad
\hat r_k = \frac{\cosh\rho \, x - c_k}{\sinh\rho},
```

where $\hat r_k$ is the unit tangent at $x$ pointing away from $c_k$, and $f = d \ln n / d\rho_\text{eff}$ is the WebGL
demo's law for throat radius $t = 0.35$ blocks: with $\psi = 1 + t/\rho$, $r = \rho \psi^2$, $r_s = 4t$,
$\lambda^2 = 1/4$ and $A = (1 - r_s/r + \lambda^2)/(1 + \lambda^2)$,

```math
f(\rho) = -\frac{2t}{\rho^2 \psi} - \frac{r_s \, \psi \, (1 - t/\rho)}{2 r^2 (1 + \lambda^2) A} .
```

A mouth adds nothing beyond the bending cutoff, $2R = 16$ blocks. The total gradient is $g = \sum_k g_k$, and its part
across the ray, the bending, is

```math
b(x, v) = g - \langle g, v \rangle v .
```

A light path with unit speed obeys

```math
x' = \frac{v}{R}, \qquad v' = \frac{x}{R} + b(x, v),
```

where the $x/R$ term alone gives the geodesics of 1.1.

### 1.3 Throats

The throat of mouth $k$ is the sphere $\rho_\text{eff} = t$. A ray that steps inside it, with radial speed
$\alpha = \langle v, \hat r_k \rangle$ and outward direction $\hat u = (x - \cosh\rho \, c_k)/\sinh\rho$ at $c_k$, comes
out of the partner mouth: in the partner's coordinates (its centre at the origin $o$, reached through the gluing
matrix $G_k$) it is at $\rho'$ with $\rho'_\text{eff} = t^2/\rho_\text{eff}$, that is
$\rho' = \tfrac12 \ln(1 + 2t^2/(R \rho_\text{eff}))$, and

```math
x' = \cosh\rho' \, o + \sinh\rho' \, G_k \hat u, \qquad
v' = G_k (v - \alpha \hat r_k) - \alpha \left(\sinh\rho' \, o + \cosh\rho' \, G_k \hat u\right),
```

so the sideways motion carries over and the inward motion turns outward.

### 1.4 The scene rows and the stepping loop of PR 70

Each frame packs the scene into rows of 4-vectors: one row in the eye's coordinates and one per mouth in that mouth's
partner's coordinates, so a ray that crosses a throat switches rows and keeps small coordinates. A ray starts at the
eye ($o$) and repeats:

1. Sum the bending of every mouth within the cutoff, and find the nearest mouth's $q$.
2. Cross a throat if the ray is inside one.
3. Choose a step $h$: if no mouth is within the cutoff, follow the exact geodesic to where it enters the nearest
   cutoff sphere, a quadratic in $\tau$: with $a = -\langle x, c \rangle$, $b = -\langle v, c \rangle$ and
   $K = \cosh(0.98 \cdot 16/R)$, $\tau = (-ab - K\sqrt{K^2 + b^2 - a^2})/(b^2 + K^2)$. Otherwise
   $h = 0.12 \, d_\text{nearest}$ within 10 throat radii of a mouth and $0.25 \, d_\text{nearest}$ beyond, at least
   0.005 blocks.
4. Test the ground and the boxes along the step (below), and stop at the first hit.
5. Advance with the explicit midpoint method on the geodesic flow:

```math
v_h = \mathrm{unit}\!\left(v + \tfrac{h}{2} b(x, v)\right), \quad
x_m = \exp_x\!\left(\tfrac{h}{2} v_h\right), \quad
x_1 = \exp_x(h \, v_h), \quad
v_1 = T(v_h) + h\left(b(x_m, v_m) - \tfrac12 b(x, v)\right),
```

where $T$ moves $v_h$ along its geodesic. Its error per unit length is $O(h^2)$.

Hits are tested along the geodesic from $x$ in direction $v_h$ for length $h$. A box is three pairs of planes: unit
normals $N_i$ and half sizes $h_i$, $|\langle x, N_i \rangle| \le \sigma_i = \sinh(h_i/R)$. Along $x + \tau v$,
normalized, each slab is

```math
(\alpha_i + \tau \beta_i)^2 \le \sigma_i^2 (1 - \tau^2), \qquad \alpha_i = \langle x, N_i \rangle, \ \beta_i = \langle v, N_i \rangle,
```

a quadratic inequality, so each slab is an interval of $\tau$ and the box is their intersection. The ground is the
plane $\langle x, N_\text{ground} \rangle = 0$, hit at $\tau = -\alpha/\beta$.

A pixel's cost is the number of steps times the bending sum over the mouths plus the box tests. In PR 70 the spawn view
took 81 ms.

## 2. Exact speed-ups: the same pixels, less work

Every change in this section left every pixel the same, on both renderers, checked image against image.

### 2.1 Distances through q (PR 69, the CPU port)

The first CPU profile spent 38% of its time in `asinhf`, from distance checks. Each check of a mouth, throat, box or
zone now compares $q$ or $\sinh$ against a threshold computed once per frame (1.1).

A per-ray cache of nearby box clusters was also tried there and dropped: no measurable gain.

### 2.2 Check throats only when a mouth is that close (PR 70)

The bending sum already computes every mouth's $q$, so the throat check runs only when
$q_\text{nearest} < 4 \sinh^2(\rho_t/2)$, $\rho_t = \tfrac12 \ln(1 + 2t/R)$ (the throat's own distance). Metal got
about 30% faster.

### 2.3 Skipping boxes a ray cannot reach (PR 72)

The scene has clusters of boxes: the house (22 boxes in a sphere 11 blocks across next to spawn), the pylons and the
trees. Each step of PR 70 looped over every cluster, skipped one only when the step's start was farther from its
bounding sphere than the step's length, and otherwise tested all its boxes. Five changes, in this order, took the
spawn view from 81 to 30 ms:

| Change | Spawn, ms |
| --- | ---: |
| PR 70 | 81 |
| skip a cluster whose bounding sphere the step's line misses | 65 |
| per-ray candidate mask | 47 |
| skip clusters the ray climbs away from | 46 |
| activate clusters in the row's distance order | 44 |
| bounding boxes instead of spheres | 30 |

**Candidate mask.** For cluster $k$ with bounding slabs $(N_i^k, H_i^k)$ and top height $z_k$, a point at height $z$
is at least

```math
\ell_k(x) = \max\left(z - z_k, \ \max_i \left[R \operatorname{arsinh}|\langle x, N_i^k \rangle| - H_i^k\right]\right)
```

from it, since the distance to a plane is $R \operatorname{arsinh}|\langle x, N \rangle|$ and the cluster lies within
$H_i^k$ of it. A path of length $L$ moves at most $L$ from where it starts. So a full pass over the clusters marks those
with $\ell_k \le W$ ($W = 3$ blocks, or the step's length if longer) in a 32-bit mask, and records the distance
$\min_{k \text{ unmarked}} \ell_k$ the ray can travel before any other cluster could matter. Until then each step
tests only the marked clusters, and only once the nearest marked bound is within reach. Windows from 1 to 6 blocks
gave the same times; 12 and 24 were slower.

**Climbing skip.** Along a geodesic, the height above the ground satisfies

```math
\sinh(z(s)/R) = a \cosh(s/R) + b \sinh(s/R), \qquad a = \langle x, N_\text{ground} \rangle, \ b = \langle v, N_\text{ground} \rangle,
```

which only grows when $a \ge 0$ and $b \ge 0$, so a step that starts above a cluster's top and climbs skips it.

**Distance order.** Each row stores the clusters sorted by a lower bound $D_k$ on their distance from the row's
origin. A ray keeps $s_\text{row}$ such that $s - s_\text{row} \ge d(o, x)$ (set when it enters the row, by the
triangle inequality), and the full pass looks only at clusters with $D_k \le s - s_\text{row} + W$.

**Bounding boxes.** Each cluster is bounded by a box in its own frame (centre $C$, normals $N_i$) instead of a sphere.
Its half sizes need a bound on $|\langle x, n \rangle|$ over every member box, for each normal $n$. In a member box's
coordinates $p_i = \langle x, N_i \rangle$, the box is $|p_i| \le \sigma_i$ and $x = \sum_i p_i N_i + p_w C$ with
$p_w = \sqrt{1 + |p|^2}$, so

```math
\langle x, n \rangle = a \cdot p + a_w \sqrt{1 + |p|^2}, \qquad a_i = \langle N_i, n \rangle, \ a_w = \langle C, n \rangle .
```

$\sqrt{1 + |p|^2}$ is convex, so this is convex for one sign of $a_w$ and concave for the other. The convex sign peaks
at one of the 8 corners. The concave sign stays below $\sum_i |a_i| \sigma_i - |a_w|$, as $p_w \ge 1$. The larger of
the two is a guaranteed bound, exact on the convex side, and a test checks that every box lies inside its cluster's
bound. The ray tests the bounding box with the same slab test as a box (1.4) and skips the cluster's boxes unless it
enters the bound before its nearest hit so far. A slab is out of reach of a step of length $h$ when
$|\alpha_i| > \sigma_i \cosh(h/R) + \sqrt{1 + \sigma_i^2}\,\sinh(h/R) = \sinh(\operatorname{arsinh}\sigma_i + h/R)$.

After these, a ray at spawn tests about 1.5 boxes per frame instead of 77. Turning box tests off entirely gains only
another 10 ms, which goes to SIMD groups waiting on their slowest ray, so a deeper hierarchy would gain little.

### 2.4 Laser beam chunks

The laser is a light ray traced on the CPU each frame, a polyline of up to 256 points drawn by testing each step of
each pixel's ray against its segments in Klein coordinates, where geodesics are straight lines. Segments are grouped
in chunks with bounding spheres. Chunks of 4 segments instead of 16 took the laser view from 55 to 43 ms (8 gave 44, 2
gave 48).

### 2.5 Presenting straight into the window

PR 70 wrote RGBA into a shared buffer, waited for the GPU, copied the pixels out and handed them to SDL. Now the
shader writes a texture that is copied into the window's `CAMetalLayer` drawable and presented without waiting. That
was about 0.5 ms at 1280×800, worth it once frames got down to a few milliseconds.

### 2.6 Tried, no gain

- 16×16 or 32×32 thread groups instead of 8×8: slower.
- The scene rows in the `constant` address space, or the mouths' centres kept in registers: no gain, or slower.
- Compiling the laser code out when it is off, or skipping the glow's `exp` for far beam pieces: no gain.
- A full pass over the clusters in $\sinh$ space, one `asinh` instead of three per cluster: 4% slower.
- A 300-step cap per ray instead of 700: no change.
- A lookup table for each mouth's bending factor, indexed by $\log q$, instead of computing $f$: no faster.

## 3. Approximations: fewer, cheaper rays

These change pixels, so each was measured against the reference of section 4. They are in the Metal shader only;
the CPU renderer still traces PR 70's light paths and is the reference.

### 3.1 Adaptive sampling

Most neighbouring rays end on the same surface at nearby points. The shader traces a coarse grid and interpolates
the ray ends where that is provably close:

1. Trace every fourth pixel each way: sample $S_{i,j}$ is pixel $(4i, 4j)$.
2. For every second pixel, look at the cell of four coarse samples around it, and the twelve samples of the
   surrounding 4×4 block without its corners. If all twelve ended on the same surface (same kind, row, face and
   side), with no laser glow, and each of the cell's two rows and two columns of four ray ends $p_0, \dots, p_3$
   satisfies

   ```math
   |p_0 - 2p_1 + p_2| \le \kappa |p_2 - p_1| \quad \text{and} \quad |p_1 - 2p_2 + p_3| \le \kappa |p_2 - p_1|,
   ```

   interpolate bilinearly in the ambient coordinates and project back onto the hyperboloid (or the unit sphere,
   for sky directions). Otherwise trace it.
3. Do the same for the remaining pixels from the grid of step 2.

The ray end $p$ is the hit point, or the sky direction for rays that leave. For a quadratic $p(u)$ sampled $h$
pixels apart, the second difference is $p'' h^2$ and the midpoint interpolation error is $p'' h^2/8$, so the
condition bounds the error by $\kappa h/8$ pixels of the local first difference. With $\kappa = 0.8$ at $h = 4$ and
$\kappa = 1.6$ at $h = 2$, both levels are within about 0.4 pixel.

Interpolated pixels are still shaded one by one (textures, fog, lighting), so textures stay sharp. Rays that can't be
interpolated are appended with atomics to lists and traced in separate passes with indirect dispatches of 64-thread
groups, so a thread group doesn't wait on rays its neighbours don't have.

Result: spawn 22 → 8.5 ms of GPU time at 2560×1600. Against tracing every pixel, features move under 0.01 pixel
(median), at most 0.9% of pixels change by more than 30 of 765 (the view 2.5 blocks from a mouth; 0.2% elsewhere),
and SSIM is 0.98 to 0.998.

Tried and dropped: one level (every second pixel; 9.4 instead of 7.7 ms at 2560×1440); $\kappa$ = 0.4 (0.1 pixel,
but the near view 29 instead of 20 ms) or 0.8 at the fine level; tracing non-smooth pixels in the same pass (about 9
ms slower, as a group waits on its slowest ray); interpolating per row or column (no faster); interpolating ray states
where they leave a mouth's near zone (it applied to only 4 to 6% of cells).

### 3.2 Fourth-order steps

The midpoint method needs short steps for its $O(h^2)$ error. From 3 throat radii of a mouth out to the cutoff, a ray
now takes classical Runge–Kutta steps of the system in 1.2, $y = (x, v)$, $y' = F(y)$:

```math
k_1 = F(y), \ k_2 = F\!\left(y + \tfrac{h}{2} k_1\right), \ k_3 = F\!\left(y + \tfrac{h}{2} k_2\right), \ k_4 = F(y + h k_3), \quad
y_1 = y + \tfrac{h}{6}(k_1 + 2k_2 + 2k_3 + k_4),
```

then puts $x_1$ back on the hyperboloid and $v_1$ in its tangent space. Its error per unit length is $O(h^4)$. The
steps are $h = 0.5 \, d_\text{nearest}$ from 3 to 10 throat radii (instead of midpoint steps of 0.12) and
$h = 0.35 \, d_\text{nearest}$ beyond (instead of 0.25; 3.4). Within 10 throat radii, stages 2 to 4 re-evaluate only
the nearest mouth $m$ and keep the others' gradient from the step's start:

```math
g(x_\text{stage}) \approx \sum_{k \ne m} g_k(x) + g_m(x_\text{stage}),
```

which was 10% faster near a mouth with no measured change in accuracy (but see section 6). Hits are tested along the
geodesic from $x$ to $x_1$, the step's chord.

Result: 10% faster, and at least as accurate as the midpoint steps it replaces (spawn: 11.5% of pixels changed
against the four-times reference, 12.3% before).

Tried and dropped:

- RK4 all the way to the throat: its stages land inside the throat sphere, and 21% of the throat view's pixels changed
  however small the steps.
- A kick–drift–kick (Heun-like) scheme that reuses each step's end bending as the next step's start, so it sums the
  bending once per step instead of twice: 17% faster at spawn, but 43% of the throat view's pixels changed (10% for
  midpoint), and crossing the throat before the end kick lost rays at the throat's edge.
- RK4 steps of 0.5 or 0.7 of the distance out to the cutoff: no faster, or faster with 18 to 36% of pixels changed.
- Keeping the other mouths' gradient fixed within midpoint steps: 22% of the aimed view changed instead of 10%.
- Growing the midpoint steps as $\sqrt{d}$ beyond 10 throat radii, up to 0.5 or 0.8 of it: 10% faster, but 16 to 22%
  of pixels changed instead of 12%.
- Summing only the nearest mouth's bending inside its near zone: slower, and 24% of the throat view changed.

### 3.3 Jumping across each mouth's inner sphere

Near a mouth, steps must be short ($0.12\,d$), and rays that wind around the throat take hundreds. But within the jump
radius $J = 4.5\,t = 1.575$ blocks of a mouth's centre there is nothing but the mouth (a test checks that no box or
ground reaches into any jump sphere), and the mouth's own field is the same about every axis through its centre. So a
ray's path inside lies in the plane through $c$, $x$ and $v$, and depends only on the angle $\theta$ at which it
enters. The shader replaces the whole crossing with one table lookup.

**Plane frame.** For a ray at $x$, at distance $\rho$ from $c$, the frame at $c$ is

```math
\hat a = \frac{x - \cosh\rho \, c}{\sinh\rho}, \quad
\hat b = \frac{v_\perp}{|v_\perp|}, \quad v_\perp = v - \alpha \hat r, \ \alpha = \langle v, \hat r \rangle, \quad
\theta = \operatorname{atan2}(|v_\perp|, -\alpha),
```

and $\hat n$ completes it. $v_\perp$ is orthogonal to $x$, $c$ and $\hat r$, so it is a tangent at $c$ as well.

**Throat table.** For 4096 angles $\theta \in [0, \pi/2]$, a GPU kernel traces, once at start-up, a ray from the
sphere's edge with fine midpoint steps ($0.01\,\rho$) in the mouth's field alone, crossing the throat into a copy of
the same mouth, until it leaves the sphere. It stores the angle $\varphi$ swept around the centre, the path length
$L$, the exit angle $\beta$ from straight out, and the parity (whether it crossed; $-1$ if it never left). At runtime
the exit is

```math
e = \cos\varphi \, \hat a + \sin\varphi \, \hat b, \quad
x_\text{out} = \cosh(J/R) \, c + \sinh(J/R) \, e, \quad
v_\text{out} = \cos\beta \left(\sinh(J/R) \, c + \cosh(J/R) \, e\right) + \sin\beta \left(-\sin\varphi \, \hat a + \cos\varphi \, \hat b\right),
```

with $c, \hat a, \hat b$ replaced by $o, G\hat a, G\hat b$ in the partner's row when the ray crossed. Entries are
linearly interpolated in $\theta$, unless the two neighbours differ in parity.

**Camera table.** When the eye is inside a jump sphere, its rays start inside. A second table has 64 rows for eye
distances $d_j = 0.9\,t \, (J/0.9t)^{j/63}$ by 2048 angles in $[0, \pi]$, blended between rows.

**The other mouths, to first order.** The other mouths' gradient bends rays inside the sphere too, and the throat
magnifies small deflections. Each entry also stores the response of its exit to a uniform extra field $\epsilon$
(the ambient vector projected onto the tangent space, $\epsilon + \langle \epsilon, x \rangle x$), from six extra rays
with a field of 0.004 per block along $\hat a$, $\hat b$ and $\hat n$, before and after the throat:
$\partial\varphi/\partial\epsilon$ and $\partial\beta/\partial\epsilon$ in the plane, and the out-of-plane tilts
$\partial\eta_x/\partial\epsilon_n$, $\partial\eta_v/\partial\epsilon_n$ of the exit point and direction. At runtime
$\epsilon_\text{in} = \sum_{k \ne m} g_k(c)$ in the frame $(\hat a, \hat b, \hat n)$, $\epsilon_\text{out}$ the same at
the partner's centre in its row, and

```math
\varphi = \varphi_0 + \frac{\partial\varphi}{\partial\epsilon_\text{in}} \cdot \epsilon_\text{in}^{ab} + \frac{\partial\varphi}{\partial\epsilon_\text{out}} \cdot \epsilon_\text{out}^{ab},
\qquad \beta \ \text{likewise},
\qquad e \leftarrow \mathrm{unit}(e + \eta_x \hat n), \quad v_\text{out} \leftarrow v_\text{out} + \eta_v \hat n .
```

A response is zero where a probe ray changes parity.

**Entering.** A step that would end inside the sphere is shortened by secant interpolation of $\rho$ along it, to
$h \cdot (\rho_0 - J)/(\rho_0 - \rho_1)$ plus 2 mm, and retaken, so the ray enters near the edge where the table
starts.

Result: the view 2.5 blocks from a mouth 65 → 30 ms (every pixel), and the images as close to the reference as
stepping through: at the throat 8.5% of pixels changed (SSIM 0.873) against 10.1% (0.865); 2.5 blocks away 18.4%
(0.702) against 17.0% (0.699). Both tables take 0.4 s at start-up.

Tried and dropped:

- No other-mouth correction: 27% of the near view's pixels changed instead of 17%, and 17% of the throat view's
  instead of 10%. Leaving them out even within 1.5 throat radii lost accuracy.
- Two half kicks by the other mouths' bending around the jump instead of the response: 40% changed.
- Tracing the camera table each frame for the eye's exact distance: 2 to 6 ms a frame.
- A four times finer throat table: no change in accuracy.

### 3.4 Step sizes beyond 10 throat radii

With RK4 in place, the outer steps were set for accuracy rather than speed: RK4 steps of $0.35\,d_\text{nearest}$
instead of PR 70's midpoint steps of $0.25\,d_\text{nearest}$ brought features at spawn from 3.2 to 1.4 pixels of the
converged paths, for 23% more GPU time when tracing every pixel. Midpoint steps of 0.125, or RK4 steps of 0.25, gave
1.1 pixels for 50% more. Shorter RK4 steps near a mouth (0.35 or 0.25 instead of 0.5) took the view 2.5 blocks from
1A only from 2.8 to 2.2 or 2.1 pixels, for 12 to 25% more time there.

## 4. How accuracy is measured

The reference is the CPU renderer's light paths (PR 70's midpoint method) with steps four and eight times smaller,
rendered by the Metal shader tracing every pixel (`.artifacts/bench_views/*-ref-fine7` and `*-ref-fine8`). The two
references differ by 0.03 to 0.2 pixel (median) in every view but the throat view (1 pixel), so the eight-times one
has converged to well under a pixel outside the lens's strongest magnification.

- **Feature shift.** The image is cut into 48×48-pixel patches; patches with a brightness standard deviation under 4
  are skipped. For each patch, the peak of the phase correlation (Hann window, normalized cross-power spectrum) with
  sub-pixel parabolic refinement gives its shift. Reported as the median, at 2560×1600; 1 pixel is about 0.05° of
  view.
- **Changed pixels.** The share of pixels whose RGB values differ by more than 30 of 765 in total.
- **SSIM** on brightness, with a Gaussian window of σ = 1.5 pixels.

## 5. Resolution when a frame is too slow

The window ray-traces at render scale $s \le 1$ per side and upscales with MetalFX's spatial scaler. One scaler is made
for the window's size with a variable input region, and run once during the first frame, so changing $s$ costs
nothing (each first use cost 20 ms or more). GPU time grows with the pixel count, $s^2$, so after a frame rendered at
scale $s_\text{used}$ in $t$ ms of GPU time,

```math
s^* = s_\text{used} \sqrt{T / t}, \quad T = 14 \text{ ms}, \qquad
s_\text{next} = \begin{cases} \max(s^*, \ 0.8 \, s_\text{used}) & s^* < s \\ \min(1.02 \, s, \ s^*) & \text{otherwise} \end{cases}
```

clamped to $[0.5, 1]$, and the frame uses $s$ rounded down to a multiple of 0.05. So it drops at once (by up to a
fifth a frame) and climbs back by at most 2% a frame; 14 ms leaves room for the CPU's part of a 60 Hz frame. Moving 2%
a frame both ways made 1.3 to 1.5% of the mouth tour's frames late; dropping all the way at once fell to 0.5 and kept
full resolution in only 92.5% of the legs' frames. MetalFX's temporal scaler needs motion vectors, which a ray tracer
through wormholes doesn't have in closed form.

At 0.8 scale, features stay within 0.2 to 0.4 pixel of full resolution, with SSIM 0.85 to 0.96.

## 6. Where it stands, and what is left

Median ms at full resolution, 2560×1600, adaptive sampling:

| View | PR 70 | Now |
| --- | ---: | ---: |
| spawn | 81 | 11 |
| aimed at mouth 1A | 64 | 10 |
| 2.5 blocks from 1A | - | 23 |
| 0.8 blocks from 1A's throat | 134 | 12 |
| above the rails | 25 | 6.2 |
| high above spawn | 52 | 9.1 |
| laser through 1A | 88 | 13 |

In a full-screen window (2560×1336) with the automatic scale, the scripted legs ran at 155 fps on average with 1 of
719 frames over 1/60 s and 94.9% of frames at full resolution, and a tour circling every mouth at 106 fps with 5 of
1795 frames over 1/60 s and 83% at full resolution (never below 0.65).

Feature shift against the eight-times reference (median, px): spawn 1.4, aimed at 1A 1.4, above the rails 0.1, at
the throat 2.3, 2.5 blocks from 1A 2.6 (PR 70: 3.2, 3.1, 1.0, 2.5 and 2.2).

Left to do after PR 72: views dominated by a mouth's lens were still 2 to 3 pixels from the converged paths, and
the frames within about a block of a mouth, looking past it, were the ones the window rendered below full
resolution. Section 7 takes up the first.

## 7. Accuracy fixes (the layer after PR 72)

### 7.1 A converged reference

The references of section 4 were rebuilt with the same tracer and steps 16 and 32 times smaller than PR 70's (no RK4,
no jump, every pixel; `*-ref-fine16`, `*-ref-fine32`). They agree to 0.01 to 0.12 pixel (median) in every view, and
the eight-times reference is within 0.02 to 0.28 pixel of the 32-times one, so the 1 pixel at the throat in section
4 was the four-times reference's error. From here on, shifts are against the 32-times reference, over the patches
where the 16- and 32-times references agree within 0.5 pixel (61% of the throat view's textured patches, 90 to
99.8% elsewhere), so a view is judged only where the truth is known to that accuracy.

Against it, PR 72 was at: spawn 1.38, aimed at 1A 1.34, high above spawn 1.04, laser 1.35, above the rails 0.13, 2.5
blocks from 1A 2.64 and at the throat 1.99 pixels.

### 7.2 Hits tested along chords

A step tests the ground and boxes along the geodesic chord from its start to its end, not along the curved path.
A path bending by $\kappa = |b|$ per block strays from a chord $h$ long by up to $\kappa h^2/8$. Seen from the eye,
a hit $L$ blocks away is off by about $\kappa h^2/(8L)$, and a hit in the eye's first step, $t$ along it, by
$\kappa (h - t)/2$, half the step's turn, however close the hit. Both are bounded by

```math
\frac{\kappa h^2}{2(4L + h)} \le \varepsilon
\quad\Longleftrightarrow\quad
h \le \lambda(L) = \frac{\varepsilon + \sqrt{\varepsilon^2 + 8\varepsilon\kappa L}}{\kappa} .
```

Capping every step at $\lambda$ with $\varepsilon = 10^{-4}$ (0.2 pixel) took spawn from 1.38 to 0.07 pixels and
aimed at 1A from 1.34 to 0.04, but doubled the time, as it shortens steps that hit nothing. So only the step that
hits is redone: when its chord hits at $L^*$ and the step is longer than $\lambda(L^*)$, the ray steps again from
the step's start to $0.75\,\lambda$ before the hit and then in steps of at most $\lambda$ until $2\lambda$ past it, up
to three times per ray. That gave spawn 0.39, aimed at 1A 0.10 and the throat view 1.42, at no measurable cost at
first; with the later changes, turning it off saves about 1.1 of spawn's 13.4 ms of GPU time.

Tried: capping steps at 0.3 of the distance travelled (spawn 1.05, +35% time); a looser $\varepsilon = 2 \cdot
10^{-4}$ (spawn 0.33); testing the hitting step along up to 16 shorter chords of its cubic Hermite curve instead of
re-stepping (as accurate, but slower: every hit ran up to 16 box tests).

### 7.3 Outer steps and the curvature scale

The view from high above stayed at 1.03 pixels. RK4 steps of $0.35\,d$ grow to 5.6 blocks near the 16-block cutoff,
but the field there varies on the scale of the curvature radius too (its $e^{2\rho}$ factor doubles every $R/2 = 4$
blocks), which the step rule doesn't see. Capping RK4 steps at 2 blocks took the high view to 0.06 and spawn to 0.23
pixels (3 blocks: 0.13 and 0.36), for about 10% more time. Ending steps exactly on the cutoff spheres, where the field drops to zero, gave
nothing.

### 7.4 Near a mouth: whole RK4 steps

Within 10 throat radii, RK4 stages re-evaluated only the nearest mouth (3.2). Summing every mouth in every stage, and
steps of $0.35\,d$ instead of $0.5\,d$ from 3 throat radii out (so one rule from there to the cutoff), took the view
2.5 blocks from 1A from 2.06 to 1.59 pixels (with 7.5's changes), and the ordinary views to 0.05 to 0.14.

### 7.5 The jump

Replacing the jump by fine stepping showed it was most of what was left near a mouth: the throat view went from 1.43
to 0.18 pixels and the near view from 2.22 to 1.44. Stepping through the sphere with the other mouths' field frozen
at its value at the centre (the table's assumption) gave 0.99 and 1.57, so that assumption was the error; adding the
field's first-order change across the sphere gave 0.12 and 1.07.

**Tidal response.** The other mouths' gradient near the centre is $g(c) + T\,\xi$, where $\xi$ is the offset from
the centre ($\xi_j = \langle x, e_j \rangle$ in an orthonormal frame at $c$) and $T_{ij} = \langle \partial_j g, e_i
\rangle$ is symmetric. A ray in the plane $(\hat a, \hat b)$ only feels, to first order, $T_{aa}$, $T_{bb}$ and
$T_{ab}$ in its plane and $T_{na}$, $T_{nb}$ across it. Each table entry now also stores the response of $\varphi$ and
$\beta$ to the three in-plane parts and of the two tilts to the two across parts, on each side of the throat, from
ten more probe rays with $T$ = 0.02 (0.005 and 0.08 gave the same images, so the response is linear): 10 texels per
entry instead of 4. Each frame a small kernel stores, for each mouth in each row, $g(c)$ and a matrix $M$ with
$M e = \sum_j D_j \langle e_j, e \rangle$, where $D_j$ is the central difference of $g$ along $e_j$ (0.01 curvature
radii), so a jump reads $T_{ab} = \tfrac12(\langle M\hat b, \hat a\rangle + \langle M\hat a, \hat b\rangle)$ and the rest
with two matrix products.

With the camera table, the far side's tidal term took the throat view from 1.38 to 1.03 pixels. With the throat
table it moved the near view the wrong way (1.61 to 1.98), while the near side's term helped (1.44); the cause isn't
found, so the throat table uses the near side's term only.

**Entering.** The step that ends inside the sphere is cut by three rounds of regula falsi on $\rho$, each a new RK4
step from the step's start, so the ray enters within a fraction of a millimetre of the sphere where the table
starts. With one round the near view was 2.02 pixels instead of 1.50. Taking the entry point and direction from the
step's Hermite curve instead was slower and less accurate: regula falsi on the cubic sometimes stopped deep inside.

Also tried: a four times larger throat table, or table rays with four times smaller steps (no change); the other
mouths' field averaged over the entry, centre and exit points instead of the centre (no change).

### 7.6 Rays that never come out

Just outside a throat, a band of directions around the tangent one stays trapped: traced with 20,000 fine steps and
ten times the usual path length, those rays still circle the throat. The demo's lensing law holds them there, so
they carry no light and the band is black (a wide black band, looking sideways inside a mouth). The shader now
paints those pixels from the image around them, with a pull–push fill. Level 0 is the image, with weight $W = 0$
for lost pixels and 1 for the rest; each coarser level averages $2\times2$ blocks by weight,

```math
P_{k+1} = \frac{\sum P_k}{\sum W_k}\, W_{k+1}, \qquad W_{k+1} = \min\!\left(1, \sum W_k\right),
```

with colours premultiplied by weight, down to $1\times1$; then, coarsest first, each level fills its gaps from the one
above, sampled bilinearly, $C_k = P_k + (1 - W_k)\, C_{k+1}$, and lost pixels take $C_1$. The band becomes a soft blend
of the colours around it. It is about 24 small dispatches a frame and costs no measurable time.

### 7.7 Where it stands

Feature shift against the 32-times reference, over converged patches (pixels):

| View | PR 72 | Now |
| --- | ---: | ---: |
| spawn | 1.38 | 0.14 |
| aimed at mouth 1A | 1.34 | 0.07 |
| 2.5 blocks from 1A | 2.64 | 1.50 |
| 0.8 blocks from 1A's throat | 1.99 | 1.02 |
| above the rails | 0.13 | 0.08 |
| high above spawn | 1.04 | 0.05 |
| laser through 1A | 1.35 | 0.07 |

It costs GPU time: at 2560×1600, spawn 9.7 → 13.6 ms of GPU time, 2.5 blocks from 1A 21.6 → 33.3 and at the throat
10.6 → 14.5 (measured alternately, under load). In a full-screen window the automatic scale keeps about the same
share of frames under 1/60 s (scripted legs 5 of 724 against 4; mouth tour 30 of 1795 against 15), but renders more
frames near a mouth below full resolution (legs 77% at full resolution against 93%, mouth tour 44% against 69%).

## 8. Lunar Surface: adaptive sampling on voxel faces

The Lunar Surface renderer (`render_voxel.metal`) keeps the level's light paths (D34): kick–drift–kick steps of 8%
of the distance to the warp, each step's chord tested against the structures in the warp's reach. Near the black hole
and the wormhole mouths every ray takes 30 to 120 such steps, so at 2560×1600 those views took 33 to 57 ms of GPU
time, and the window upscaled them from 0.53 to 0.62 of full resolution.

### 8.1 Where the time went

Measured by turning parts off, at 2560×1600 tracing every pixel:

- Shading (materials, the sun's shadow ray, lamps, the ground's tiling) is 3 to 6 ms of each view; the sun's shadow
  rays alone cost no measurable time.
- Testing structures along the bent steps is about half of a view near a warp (the black hole 41.7 → 21.8 ms without
  it). A cheaper test per structure (the step's start must lie within the structure's bounding sphere plus the step,
  $-\langle x, c\rangle \le \cosh R \cosh s + \sinh R \sinh s$) and one call site for straight and bent chords were
  no faster, so the cost is in the voxel walks themselves.
- Per-pixel step counts: in the black hole view every ray takes 30 to 40 steps, as the eye is inside the hole's reach;
  at the wormhole exit 94% of the steps are in rays taking over 40 steps, which wind near the throat.

### 8.2 Adaptive sampling

The house's adaptive sampling (3.1) now runs here too, with the same passes and buffers (`encode_adaptive_sampling`
in `render_metal.rs`). A ray end $E$ (`VoxelRayEnd`) is the hit point and tangent, the hit's grid point
$g \in \mathbb{R}^3$ in its structure's cell coordinates, the cell and its material, the path length, the glass
count and the laser glow; or the escaping direction carried back to the origin, for the sky. Two samples are on the
same surface when both are on one face plane, that is one structure $k$, one face axis $a$, one side
$\sigma = \pm 1$ and the same layer of cells $\mathrm{cell}_a$ (a plane of the structure's Klein coordinates, so a
geodesic plane); or both on the ground, both in the sky or both lost; behind the same number of glass faces, with no
laser glow. With that, the twelve-sample test of 3.1 decides each cell, and a pixel in a smooth cell blends its four
corners: the point and tangent bilinearly, put back on the hyperboloid and into its tangent space; $g$ and the path
length bilinearly.

A face plane may have holes or bumps smaller than the 4-pixel spacing. So an interpolated block hit takes its cell
from the blended grid point, $\mathrm{cell}_b = \lfloor g_b \rfloor$ for $b \ne a$, and is kept only if that cell is
solid (and not glass) and the cell in front of it, $\mathrm{cell} + \sigma e_a$, is empty or outside the grid; else
the ray is traced. The material comes from that cell, so textures change at the right edges. Each pixel is still
shaded on its own (sub-block texels, edges, stars), so interpolation doesn't blur them.

Result, GPU ms at 2560×1600:

| View | Every pixel | Adaptive |
| --- | ---: | ---: |
| black hole | 41.9 | 16.6 |
| Einstein ring | 44.3 | 15.6 |
| wormhole | 33.4 | 17.5 |
| wormhole exit | 57.6 | 29.2 |
| laser into the wormhole | 44.0 | 26.6 |
| overview | 21.9 | 12.9 |
| blue spawn | 5.8 | 8.4 |
| overlook | 5.3 | 6.5 |

Views with little bending pay about 1 ms for the extra passes, and still run far above 60 fps. Against tracing every
pixel, 0.05 to 1.4% of pixels change by more than 30 of 765 and SSIM is 0.976 to 0.999; every change is texture
noise where texels are smaller than a pixel (near the horizon and in lensed rims), where both images alias. Against
a reference with steps eight times smaller, the median feature shift is the same as tracing every pixel's within
0.05 pixel on every view but the wormhole exit (4.1 against 3.7 pixels, in that aliasing).

### 8.3 Tried: jumping across each warp's empty middle

Inside the largest sphere around a warp with no block or ground in it (7.5 m around the black hole, 1.34 m around the
mouths, limited by the red landing pad and the ground), the index $n(\rho)$ is spherically symmetric, so a ray stays
in the plane through the centre and its tangent, and $L = n(\rho) \sinh\rho \sin\alpha$ is conserved ($\alpha$ from
the radial direction). Where it leaves then depends only on $\sin\alpha$ at entry. A table of 2,048 entries per warp,
traced at load with steps four times smaller than the renderer's, stored the outcome (back out, through the throat,
captured), the angle swept about the centre, the exit's tangential component and the path length. The shader landed
each entering ray on the sphere, built the rotation $R = [\hat n, \hat e_\varphi, \hat n \times \hat e_\varphi]$
taking the table's ray (entering on the $x$ axis in the plane $z = 0$) to this one, and placed the exit at
$R(\cos\varphi, \sin\varphi, 0)$, or $F R F$ with $F = \mathrm{diag}(1, 1, -1)$ in the partner's frame after the
throat's mirror. Entries whose neighbours disagree or sweep more than 0.02 rad (near an orbit) stepped as before.

It cut the mean steps per ray from 42 to 33 (black hole) and 48 to 29 (Einstein ring), and tracing every pixel got
7 to 30% faster. With adaptive sampling it was 10 to 35% slower: a jumped ray follows the fine path, while its
neighbours just outside the sphere, or in the bands near an orbit, carry the level's step error (up to 2 pixels), so
cells along those edges stopped being smooth and were traced. Jumping pays only if the stepping around it is as
accurate as the table, which the level's steps are not; it was dropped.
