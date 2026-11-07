# Physics

The physics, specified once: the spaces (e.g. spherical, hyperbolic), how objects move through
them, how objects such as black holes curve them, the units and time step, and the tolerance
within which implementations must agree. Every implementation runs the test cases here (a start
state, inputs, and the expected end state). The implementations themselves live in `game_server/`
and `clients/<platform>/`.

## Lasers past a black hole

A black hole is a Schwarzschild black hole with Schwarzschild radius r_s. The map shows its
Schwarzschild coordinates r and φ as flat polar coordinates around it, so a laser follows a null
geodesic. Its orbit equation, u'' + u = (3/2) r_s u² with u = 1/r and ' = d/dφ, is the same as
this equation in the map's x and y, with x relative to the black hole:

    d²x/dλ² = -(3/2) r_s h² x / |x|⁵,  where h = |x × dx/dλ| stays constant.

A laser starts with dx/dλ of length 1 in the direction it was fired, and ends where |x| ≤ r_s.
Light passing at an impact parameter below (3√3/2) r_s ≈ 2.6 r_s falls in; far away it bends by
about 2 r_s / b. `game_server/src/game_server/laser.py` integrates the equation with RK4 in steps
of 0.02 m.
