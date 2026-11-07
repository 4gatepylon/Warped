# Curved space: the math and physics behind the renderers

Notes from Claude Code (an AI agent), written on 2026-11-06 after adding curved space to Gluebox. They
say what to read and understand to follow the math and physics behind the rendering in
[`demos/`](../demos/README.md) and the spec in [`common/physics/`](../common/physics/README.md). Each
section gives the idea, the equations, and where it shows up in this repo. The
[reading list](#reading-list) at the end goes from gentle to technical. Its links were checked on that
date.

## Which demo uses what

| Demo | Space | What a light ray is | Sections |
| --- | --- | --- | --- |
| `three_js_vibecode_demo/spherical*.html` | the 3-sphere S³ | a great circle | 1–3, 5 |
| `three_js_vibecode_demo/hyperbolic*.html`, `hyperbolic_wormholes/` | hyperbolic space H³ | a geodesic of the hyperboloid | 1–3, 5 |
| `gluebox/` | a cube with glued faces: flat, spherical or hyperbolic | a straight line in a projective chart | 1, 5, 6 |
| `three_js_vibecode_demo/blackholes*.html`, `common/physics/` | Schwarzschild spacetime | a null geodesic, bent by the hole | 7 |
| `three_js_vibecode_demo/wormholes.html`, `hyperbolic_wormholes/` | "black hole foil" wormholes | a ray in a medium with a refractive index | 7 |
| `laser_gravity_demo/` | the spaces above, with gravity pulling on the beam | not a geodesic: the beam accelerates | 7 |

## 1. Surfaces: curvature, and what topology fixes

A surface's **Gaussian curvature** $K$ at a point says how circles there differ from flat ones. A
circle of radius $r$ (drawn with geodesics) has circumference

```math
C(r) = 2\pi r \left(1 - \frac{K r^2}{6} + O(r^4)\right),
```

so $K > 0$ gives short circles and $K < 0$ long ones. A sphere of radius $R$ has $K = 1/R^2$. $K$ is
intrinsic: an ant on the surface can measure it without leaving it.

The **Euler characteristic** $\chi = V - E + F$ counts the vertices, edges and faces of any
triangulation, and doesn't depend on which one. A closed orientable surface with $g$ handles has
$\chi = 2 - 2g$: 2 for the sphere, 0 for the torus. In general $\chi = \sum_i (-1)^i b_i$, where the
Betti number $b_i$ counts the independent $i$-dimensional holes.

**Gauss–Bonnet** ties the two together on a closed surface $M$:

```math
\int_M K \, dA = 2\pi \chi(M).
```

So topology fixes the *total* curvature but not where it goes. A polyhedron is the extreme case. It is
flat except at its vertices. At a vertex $v$ the **angle deficit**, $2\pi$ minus the sum of the face
angles there, acts as concentrated curvature $\delta_v$, and **Descartes' theorem** says
$\sum_v \delta_v = 2\pi\chi = 4\pi$. The **uniformization theorem** says the curvature can always be
spread out evenly: every closed surface has a metric of constant curvature, with the sign of $\chi$.

**In 3D this breaks down.**

- Every closed 3-manifold has $\chi = 0$ (by Poincaré duality), so Gauss–Bonnet gives no constraint.
- Every closed 3-manifold has metrics of negative Ricci curvature (Gao–Yau 1986, and Lohkamp 1994 in
  every dimension from 3 up). So topology fixes neither the total curvature nor its sign.
- The 3D replacement for uniformization is Thurston's **geometrization**, which Perelman proved with
  Ricci flow. Every closed orientable 3-manifold can be cut along spheres and tori into pieces that
  each carry one of eight geometries (§4).
- Only three of the eight (flat, spherical and hyperbolic) look the same in every direction. That is
  why 3D curvature has to be described per direction.

## 2. Direction-dependent curvature: neighbouring straight lines

The cleanest way to measure curvature per direction is to watch two nearby straight lines
(geodesics).

- You travel along a geodesic with unit velocity $T$.
- A friend travels along a nearby geodesic, offset from you by a small vector $J$ perpendicular to $T$.
- In flat space $J$ changes linearly. In curved space it obeys the **geodesic deviation** (Jacobi)
  equation:

```math
\frac{D^2 J}{dt^2} = -\mathcal{R}_T\, J, \qquad \mathcal{R}_T(J) = R(J, T)\,T,
```

where $R$ is the Riemann curvature tensor. The **tidal matrix** $\mathcal{R}_T$ is symmetric and acts
on the plane perpendicular to $T$, so in 3D it is a 2×2 matrix. In gravity it is exactly the tidal
force (§7.1).

Each eigenvalue $K$ of $\mathcal{R}_T$ is the **sectional curvature** of the plane spanned by $T$ and
that eigenvector. Along an eigenvector the equation becomes $J'' = -K J$. For two geodesics that leave
one point at a small angle ($J(0) = 0$, $J'(0) = 1$):

```math
J(t) = \begin{cases}
\sin(\sqrt{K}\,t)/\sqrt{K} & K > 0\text{: they meet again, as on a sphere,} \\
t & K = 0\text{: they spread linearly, as in flat space,} \\
\sinh(\sqrt{-K}\,t)/\sqrt{-K} & K < 0\text{: they spread exponentially, as in hyperbolic space.}
\end{cases}
```

Geodesics that start parallel ($J(0) = 1$, $J'(0) = 0$) follow $\cos$, $1$ and $\cosh$ instead.

**Direction-dependent** curvature means one of two things:

- the two eigenvalues of $\mathcal{R}_T$ differ, so neighbours converge one way and diverge the other;
- or the eigenvalues change when you turn to a new $T$.

**Isotropic** means $\mathcal{R}_T = \kappa I$ for every $T$. In 3D or more, a space that is isotropic
at every point has the same $\kappa$ everywhere (Schur's lemma), so it is flat, spherical or
hyperbolic.

## 3. Sectional, Ricci and scalar curvature

The sectional curvature of the plane spanned by vectors $u$ and $v$ is

```math
K(u, v) = \frac{\langle R(u, v)\,v,\ u \rangle}{\lvert u\rvert^2 \lvert v\rvert^2 - \langle u, v\rangle^2}.
```

In a space of constant curvature $\kappa$ the whole tensor collapses to
$R(u,v)w = \kappa(\langle v,w\rangle u - \langle u,w\rangle v)$, so every plane has $K = \kappa$.

Ricci and scalar curvature are traces (sums) of sectional curvatures. With $e_1, e_2, e_3$
orthonormal and $K_{ij} = K(e_i, e_j)$:

```math
\operatorname{Ric}(T, T) = \operatorname{tr} \mathcal{R}_T = K(T, e_1) + K(T, e_2) \quad (e_1, e_2 \perp T),
\qquad
S = \sum_i \operatorname{Ric}(e_i, e_i) = 2\,(K_{12} + K_{13} + K_{23}).
```

**Ricci curvature measures how a beam focuses.** Take a thin bundle of initially parallel geodesics
heading along $T$, with cross-section area $A$. It starts to shrink or grow as

```math
A''(0) = -\operatorname{Ric}(T, T)\, A(0).
```

This is a special case of the **Raychaudhuri equation**, and the idea behind Baez and Bunn's picture
of Einstein's equation (§7.1).

**Scalar curvature measures the volume of small balls:**

```math
\operatorname{Vol}(B_r) = \tfrac{4}{3}\pi r^3 \left(1 - \frac{S\, r^2}{30} + O(r^4)\right).
```

**In 3D, Ricci determines everything.** For a plane $\Pi$ with unit normal $n$,

```math
K(\Pi) = \frac{S}{2} - \operatorname{Ric}(n, n).
```

So in 3D the curvature at a point is just the symmetric 3×3 matrix $\operatorname{Ric}$, and isotropic
means $\operatorname{Ric} = (S/3)\,g$. In the basis that diagonalizes Ricci, with eigenvalues
$r_1, r_2, r_3$, this reads $K_{12} = (r_1 + r_2 - r_3)/2$.

In 4 or more dimensions this fails. The part of $R$ that Ricci misses is the **Weyl tensor**, which
carries the tides of empty space (§7.2).

For constant curvature with radius $R$: $K = \pm 1/R^2$, $\operatorname{Ric} = \pm (2/R^2)\, g$ and
$S = \pm 6/R^2$.

## 4. Examples: Thurston's eight geometries

| Geometry | Metric | Sectional curvature | Ricci eigenvalues |
| --- | --- | --- | --- |
| E³ (flat) | dx² + dy² + dz² | 0 | 0, 0, 0 |
| S³ (radius R) | the round 3-sphere | 1/R² for every plane | 2/R², 2/R², 2/R² |
| H³ (radius R) | hyperbolic space | −1/R² for every plane | −2/R², −2/R², −2/R² |
| S² × ℝ | R²(dθ² + sin²θ dφ²) + dz² | n_z²/R² for the plane with unit normal n | 1/R², 1/R², 0 |
| H² × ℝ | (hyperbolic plane of radius R) + dz² | −n_z²/R² | −1/R², −1/R², 0 |
| Nil | dx² + dy² + (dz − ½(x dy − y dx))² | xy plane −¾; planes containing z +¼ | −½, −½, +½ |
| Sol | e^(2z) dx² + e^(−2z) dy² + dz² | xy plane +1; xz and yz planes −1 | 0, 0, −2 |
| SL̃(2, ℝ) | a twisted version of H² × ℝ | (left out here) | |

What the direction-dependent ones feel like to a traveller:

- **S² × ℝ**, the geometry that S² × S¹ in Gluebox would want:
  - Walking horizontally, a neighbour offset along the sphere meets you again (sine); a neighbour
    offset vertically stays parallel.
  - Walking straight up, nothing meets again.
  - A geodesic follows a great circle at speed cos α while climbing at speed sin α, so geodesics are
    helices.
- **Sol:**
  - Moving along z, beams spread fast (Ric = −2).
  - Moving along x, the y-neighbour converges (+1) and the z-neighbour diverges (−1). A beam's volume
    is kept (Ric = 0) but its shape is sheared, as in the tides outside a black hole (§7.2).
- **Nil:** curvature is mostly negative, and slightly positive in the planes that contain the twist
  axis z.

**Why the renderers stick to constant curvature.** By **Beltrami's theorem**, a space has a chart in
which every geodesic is a straight line only if its curvature is constant. Gluebox's curved cubes rely
on such a chart (§5). The other five geometries need the geodesic equation integrated per pixel,

```math
\ddot{x}^k + \Gamma^k_{ij}\, \dot{x}^i \dot{x}^j = 0,
```

as the black-hole demos do for Schwarzschild. "Ray-marching Thurston geometries" (reading list) does
this for all eight.

## 5. Models the renderers use

Points of the curved spaces are 4-vectors and motions are 4×4 matrices, so moving, turning and going
through a portal are all matrix products.

**The 3-sphere** of radius $R$ is $\{x \in \mathbb{R}^4 : x \cdot x = R^2\}$, and its motions are the
orthogonal matrices O(4). The geodesic that leaves $x_0$ with unit velocity $u_0$ (with
$u_0 \cdot x_0 = 0$), and the distance $d$ between two points, are

```math
x(t) = \cos(t/R)\, x_0 + R \sin(t/R)\, u_0, \qquad \cos(d/R) = \frac{x \cdot y}{R^2}.
```

**Hyperbolic space** uses the Minkowski product $\langle x, y \rangle = x_1y_1 + x_2y_2 + x_3y_3 - x_4y_4$.
$H^3$ of radius $R$ is the sheet $\{\langle x, x\rangle = -R^2,\ x_4 > 0\}$ of a hyperboloid, its
motions are the Lorentz matrices $O^+(3,1)$, and

```math
x(t) = \cosh(t/R)\, x_0 + R \sinh(t/R)\, u_0, \qquad \cosh(d/R) = -\frac{\langle x, y \rangle}{R^2}.
```

**Geodesics are linear.** Both formulas say that a geodesic is where the model meets a 2-plane through
the origin of $\mathbb{R}^4$. A totally geodesic plane, such as a wall or the ground, is where the
model meets a 3-plane through the origin. That's why testing a ray against a wall is linear algebra.

**Where you can see the sign of curvature.**

- Put the ground on the plane $x_3 = 0$.
- A level shot from $x_0$ has $u_{0,3} = 0$.
- Its height coordinate is then $x_3(t) = \cos(t/R)\,x_{0,3}$ on the sphere and
  $\cosh(t/R)\,x_{0,3}$ on the hyperboloid.
- So on the sphere every level shot lands at $t = \pi R/2$, from any height, as the laser demos show.
  In hyperbolic space a level shot never lands.

**Projective charts (Gluebox).**

- Dividing by the last coordinate, $p = (x_1, x_2, x_3)/x_4$, maps half of $S^3$ (the gnomonic chart)
  and all of $H^3$ (the Beltrami–Klein model, inside the unit ball) into $\mathbb{R}^3$.
- A 2-plane through the origin becomes a line, so in the chart **geodesics are straight lines**.
- Gluebox therefore marches rays through its voxel grid in straight lines, as in flat space. Only the
  fog (which needs the true distance) and the face jumps know about the curvature.

With $R = 1$, the distance between chart points $p$ and $q$ is

```math
\cos d = \frac{1 + p \cdot q}{\sqrt{(1 + \lvert p\rvert^2)(1 + \lvert q\rvert^2)}} \ \ (\text{sphere}), \qquad
\cosh d = \frac{1 - p \cdot q}{\sqrt{(1 - \lvert p\rvert^2)(1 - \lvert q\rvert^2)}} \ \ (\text{hyperbolic}).
```

**The cube's corner angle.**

- The cube is $[-a, a]^3$ in the chart.
- The face $p_1 = a$ has normal $N = (1, 0, 0, -a)$ on the sphere and $(1, 0, 0, a)$ on the
  hyperboloid.
- The angle between two faces' normals gives the corner (dihedral) angle $\theta$:

```math
\cos\theta = -\frac{a^2}{1 + a^2} \ \ (\text{spherical}), \qquad
\cos\theta = \frac{a^2}{1 - a^2} \ \ (\text{hyperbolic}), \qquad
a^2 = \frac{\lvert\cos\theta\rvert}{1 + \cos\theta}.
```

- So 90° is flat. Wider corners are spherical: $a = 1$ gives 120°, the cube of the 8-cell, eight of
  which tile $S^3$. Sharper corners are hyperbolic.
- For a cube $n$ blocks wide, the curvature radius is $R = (n/2)/\operatorname{atan} a$ on the sphere
  and $(n/2)/\operatorname{atanh} a$ in hyperbolic space, so the centre is $n/2$ blocks from each
  face.

**Face jumps.**

- A face jump from face $i$ to its partner $j$ is the motion $G = (\text{reflection in face } j)\cdot P$.
  Here $P$ is the cube symmetry that carries face $i$ onto face $j$.
- In the chart $G$ acts projectively. A point $p$ goes to $Y_{xyz}/Y_w$ with $Y = G\,(p, 1)$.
- A direction $d$ goes to the derivative of that:

```math
v \propto \dot{Y}_{xyz}\, Y_w - Y_{xyz}\, \dot{Y}_w, \qquad \dot{Y} = G\,(d, 0).
```

The code is in `demos/gluebox/src/math/geometry.js` and `demos/gluebox/src/world/shader.js`.

## 6. Gluing a cube: when is the space smooth?

Gluing faces by motions gives a space that is smooth across the faces, so what matters is the edges.

**Edge angles.** Each class of glued edges has a total angle $\Theta$ around it. Each cube edge in the
class adds the corner angle $\theta$, and each fold line (where a face is folded onto itself) adds
180°. With $k$ cube edges and $s$ fold spokes in the class:

```math
\Theta = k\,\theta + s \cdot 180^\circ.
```

**When it's smooth.** **Poincaré's polyhedron theorem** says, roughly, that if $\Theta = 360^\circ$ for
every class, the glued space is a smooth manifold with the cube's geometry. Epstein–Petronio give the
exact statement.

**When it isn't.** If $\Theta \ne 360^\circ$, the curvature the faces don't carry is concentrated along the
edge, as at a polyhedron's vertex in §1.

- The **deficit angle** $\delta = 360^\circ - \Theta$ plays the role of curvature there.
- $\delta > 0$ is like a cone (positive curvature) and $\delta < 0$ like a saddle (negative).
- This is the idea behind **Regge calculus**, which approximates any curved space by flat pieces with
  deficits on the edges.
- Gluebox draws these edges dashed.

**Which corner angles work.** With $k$ cube edges in every class and no folds, $\Theta = 360^\circ$ needs
$\theta = 360^\circ/k$:

- $k = 4$ gives 90°: the flat 3-manifolds (9 of the 10 come from one cube).
- $k = 3$ gives 120°: the quaternionic space $S^3/Q_8$ and the lens space $L(8,3)$, both spherical.
- $k = 5$ would give 72° (hyperbolic), but the cube's 24 half-edges (each edge cut at its midpoint)
  can't be split into classes of 5.
- $\mathbb{RP}^3$ would need 180°: a degenerate cube, half of $S^3$.
- The $S^3$ preset's fold lines stay at 180° whatever $\theta$ is.

So none of those last three can be made smooth with one cube.

## 7. Physics: gravity is curvature

### 7.1 Tides are geodesic deviation

Two free-falling particles a small vector $\xi$ apart accelerate relative to each other. In Newtonian
gravity with potential $\Phi$,

```math
\ddot\xi^{\,i} = -\frac{\partial^2 \Phi}{\partial x^i\, \partial x^j}\, \xi^j,
```

and in general relativity, with $u$ the particles' 4-velocity,

```math
\frac{D^2 \xi^\mu}{d\tau^2} = -R^\mu{}_{\alpha\nu\beta}\, u^\alpha \xi^\nu u^\beta.
```

This is §2's Jacobi equation in spacetime, and in the Newtonian limit the tidal matrix is the Hessian
of $\Phi$.

- Its trace is $\nabla^2\Phi = 4\pi G\rho$, which corresponds to $\operatorname{Ric}(u, u)$.
- Einstein's equation fixes that trace from the matter present.

Baez and Bunn state it like this. A small ball of test particles, initially at rest relative to each
other, starts to change volume as

```math
\left.\frac{\ddot V}{V}\right|_{t=0} = -\frac{4\pi G}{c^2}\left(\varepsilon + P_x + P_y + P_z\right),
```

where $\varepsilon$ is the energy density and $P_x, P_y, P_z$ are the pressures at its centre.

**The demos' curved spaces are different.** They are curved *space*, fixed in time, with no matter
causing them: the stage, not a solution of Einstein's equation. They are like the spatial slices of
closed and open expanding universes. The gravity in `laser_gravity_demo/` is a separate, game-scaled
pull (§7.5).

### 7.2 Empty space still has tides

Outside a star or a black hole, Ric = 0 (vacuum). So a ball of dust keeps its volume at first, but its
shape still changes.

For Schwarzschild, for observers falling radially, the tidal matrix in the (radial, transverse,
transverse) directions is

```math
\mathcal{R} = \frac{GM}{r^3}\operatorname{diag}(-2,\ 1,\ 1).
```

- Radially, $J'' = +(2GM/r^3)\,J$: stretching.
- Sideways, $J'' = -(GM/r^3)\,J$: squeezing. Together these are spaghettification.
- The trace is zero, as Ric = 0 requires.

The part of the curvature that Ricci misses is the **Weyl tensor**. In 3D it is identically zero,
which is why §3 could say Ricci determines everything there. In 4D spacetime it carries tides and
gravitational waves through empty space.

### 7.3 Light in a static spacetime is light in glass

Take a static metric $ds^2 = -A\,c^2dt^2 + g_{ij}\,dx^i dx^j$.

- Light travels on $ds = 0$, so it takes time $c\,dt = \sqrt{g_{ij}\,dx^i dx^j / A}$.
- By **Fermat's principle**, rays make the travel time stationary. So they are geodesics of the
  **optical metric** $g_{ij}/A$.
- If the spatial metric is conformally flat, $g_{ij} = B\,\delta_{ij}$, the optical metric is
  $n^2\delta_{ij}$, with refractive index

```math
n = \sqrt{B/A},
```

and rays bend like light in glass of varying index:

```math
\frac{d}{ds}\left(n\,\frac{d\mathbf{x}}{ds}\right) = \nabla n,
\qquad\text{so the ray's curvature is } \nabla_\perp \ln n.
```

Schwarzschild in isotropic coordinates ($r = \rho\,(1 + r_s/4\rho)^2$, with $r_s = 2GM/c^2$) has

```math
A = \left(\frac{1 - r_s/4\rho}{1 + r_s/4\rho}\right)^2, \qquad
B = \left(1 + \frac{r_s}{4\rho}\right)^4, \qquad
n = \frac{(1 + r_s/4\rho)^3}{1 - r_s/4\rho} \approx 1 + \frac{r_s}{\rho}.
```

The wormhole demos use the same law with Damour and Solodukhin's "black hole foil" metric:
$n = (1 + \rho_t/\rho)^2/\sqrt{A}$ with throat $\rho_t = r_s/4$ (see
[`three_js_vibecode_demo/README.md`](../demos/three_js_vibecode_demo/README.md)).

### 7.4 Bending past a black hole

Take a light ray in Schwarzschild's equatorial plane, with $u = 1/r$ and impact parameter $b$ (how
far from the hole the ray would pass if it went straight). Then

```math
\left(\frac{du}{d\varphi}\right)^2 = \frac{1}{b^2} - u^2\,(1 - r_s u),
\qquad\text{so}\qquad
u'' + u = \tfrac{3}{2}\, r_s u^2.
```

- Light can circle the hole on the photon sphere, $r = 3r_s/2$.
- Rays with $b < (3\sqrt{3}/2)\,r_s \approx 2.6\,r_s$ fall in. They make the hole's shadow.
- Far rays bend by $\Delta\varphi \approx 2r_s/b = 4GM/(c^2 b)$, twice the Newtonian answer.

[`common/physics/README.md`](../common/physics/README.md) writes the same equation in Cartesian
coordinates for integrating with RK4. The black-hole demos trace each pixel's ray with RK4 under the
same law.

### 7.5 Beams that fall: curves that aren't geodesics

A curve that accelerates with $a$ (perpendicular to its velocity, so it only turns) satisfies
$\nabla_T T = a$; a geodesic is the case $a = 0$.

In the embedded models of §5, the covariant derivative is the ordinary second derivative minus its
part normal to the model:

```math
\ddot x = -\frac{\lvert\dot x\rvert^2}{R^2}\, x + a \ \ (\text{sphere}), \qquad
\ddot x = \frac{\langle \dot x, \dot x\rangle}{R^2}\, x + a \ \ (\text{hyperboloid}),
```

with $a$ tangent to the model ($a \cdot x = 0$, or $\langle a, x\rangle = 0$). With $a = 0$ these give
§5's $\cos$ and $\cosh$ geodesics.

`laser_gravity_demo/` integrates its beams step by step on the 3-sphere or hyperboloid (RK4). A level
beam at speed $v$ under gravity $g$ turns along a circle of radius $v^2/g$. At the demo's 20 blocks/s
and 30 blocks/s², that is about 13 blocks, as its README says.

## Reading list

**Gentle**

- Jeff Weeks, *The Shape of Space* (2nd ed., 2002). Gluing the faces of cubes, the 3-manifolds that
  come out, and their geometries, with no calculus. The closest match to Gluebox.
- Tristan Needham, *Visual Differential Geometry and Forms* (Princeton University Press, 2021).
  Curvature, geodesic deviation, Gauss–Bonnet and Einstein's equation, mostly in pictures.
- John Baez and Emory Bunn, ["The Meaning of Einstein's Equation"](https://arxiv.org/abs/gr-qc/0103044).
  The ball-of-dust form of Einstein's equation (§7.1) in a few pages.
- Wikipedia, for definitions:
  - surfaces: [Gaussian curvature](https://en.wikipedia.org/wiki/Gaussian_curvature),
    [Euler characteristic](https://en.wikipedia.org/wiki/Euler_characteristic),
    [Gauss–Bonnet theorem](https://en.wikipedia.org/wiki/Gauss%E2%80%93Bonnet_theorem),
    [Descartes' theorem](https://en.wikipedia.org/wiki/Descartes%27_theorem_on_total_angular_defect);
  - curvature in 3D and up: [sectional](https://en.wikipedia.org/wiki/Sectional_curvature),
    [Ricci](https://en.wikipedia.org/wiki/Ricci_curvature),
    [scalar](https://en.wikipedia.org/wiki/Scalar_curvature),
    [Weyl tensor](https://en.wikipedia.org/wiki/Weyl_tensor),
    [geodesic deviation](https://en.wikipedia.org/wiki/Geodesic_deviation),
    [Raychaudhuri equation](https://en.wikipedia.org/wiki/Raychaudhuri_equation);
  - 3-manifolds: [geometrization](https://en.wikipedia.org/wiki/Geometrization_conjecture),
    [Ricci flow](https://en.wikipedia.org/wiki/Ricci_flow),
    [Regge calculus](https://en.wikipedia.org/wiki/Regge_calculus);
  - models: [hyperboloid model](https://en.wikipedia.org/wiki/Hyperboloid_model),
    [Beltrami–Klein model](https://en.wikipedia.org/wiki/Beltrami%E2%80%93Klein_model),
    [gnomonic projection](https://en.wikipedia.org/wiki/Gnomonic_projection);
  - physics: [Schwarzschild metric](https://en.wikipedia.org/wiki/Schwarzschild_metric),
    [Fermat's principle](https://en.wikipedia.org/wiki/Fermat%27s_principle).

**Seeing it**

- Jeff Weeks, [Curved Spaces](https://www.geometrygames.org/CurvedSpaces/). A flight simulator for
  glued-up spherical, flat and hyperbolic spaces.
- [3-dimensional.space](http://www.3-dimensional.space). In-space views of all eight Thurston
  geometries, described in the Non-Euclidean Virtual Reality papers:
  [I: H³](https://arxiv.org/abs/1702.04004), [II: H² × E](https://arxiv.org/abs/1702.04862),
  [III: Nil](https://arxiv.org/abs/2002.00513) and [IV: Sol](https://arxiv.org/abs/2002.00369).
- Coulon, Matsumoto, Segerman and Trettel,
  ["Ray-marching Thurston geometries"](https://arxiv.org/abs/2010.15801) (Experimental Mathematics,
  2022). How to render all eight geometries and spaces glued from them: the roadmap for taking Gluebox
  past constant curvature.
- James, von Tunzelmann, Franklin and Thorne, ["Gravitational lensing by spinning black holes in
  astrophysics, and in the movie Interstellar"](https://arxiv.org/abs/1502.03808) (2015). How a
  production black-hole renderer traces its rays.
- Damour and Solodukhin, ["Wormholes as black hole foils"](https://arxiv.org/abs/0704.2667) (2007). The
  wormhole metric the wormhole demos use.

**Deeper**

- Sean Carroll, ["Lecture Notes on General Relativity"](https://arxiv.org/abs/gr-qc/9712019). Free;
  they later became the book *Spacetime and Geometry*. Curvature tensors, geodesic deviation and
  Schwarzschild orbits.
- James Hartle, *Gravity: An Introduction to Einstein's General Relativity* (2003). Physics first:
  light bending, black holes and orbits before the tensor machinery.
- William Thurston, *Three-Dimensional Geometry and Topology*, vol. 1 (1997). Geometric structures,
  gluing polyhedra, and the eight geometries.
- Peter Scott, ["The Geometries of 3-Manifolds"](https://doi.org/10.1112/blms/15.5.401) (Bulletin of
  the London Mathematical Society, 1983). The classic survey of the eight geometries.
- John Ratcliffe, *Foundations of Hyperbolic Manifolds*. The hyperboloid model and Poincaré's
  polyhedron theorem in full.
- D. B. A. Epstein and C. Petronio, "An exposition of Poincaré's polyhedron theorem" (L'Enseignement
  Mathématique, 1994). The exact conditions behind §6.
