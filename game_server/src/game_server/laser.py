"""`laser.py` traces a laser past a black hole, and checks what a laser touches.

The black hole is a Schwarzschild black hole, and the map shows its Schwarzschild coordinates r and
φ as flat polar coordinates around it. A laser then follows a null geodesic, whose orbit equation
u'' + u = (3/2) r_s u² (u = 1/r, ' = d/dφ) is the same as this equation in the map's x and y:

    d²x/dλ² = -(3/2) r_s h² x / |x|⁵,  where h = |x × dx/dλ| is constant and x is relative to the
    black hole.

Interface:
    POINT_SPACING_M: The distance along the path between the points `trace_path` returns.
    trace_path: Returns the points of a laser's path, from where it starts to where it ends.
    touches_square: Checks whether a path touches an axis-aligned square.

Example:
    >>> points = trace_path((2.0, 12.0), (1.0, 0.0), (12.0, 12.0), 1.2, 24.0, 48.0)
"""

import math

POINT_SPACING_M: float = 0.25
_STEP_M: float = 0.02

_Vector = tuple[float, float]


def _acceleration(x: float, y: float, k: float) -> _Vector:
    """`_acceleration` returns d²x/dλ² at (x, y) relative to the black hole, for k = (3/2) r_s h².

    Example:
        >>> _acceleration(3.0, 0.0, 1.5)
    """
    scale: float = -k / math.hypot(x, y) ** 5
    return (scale * x, scale * y)


def _rk4_step(x: float, y: float, vx: float, vy: float, k: float, dt: float) -> tuple[float, ...]:
    """`_rk4_step` advances the geodesic equation by `dt` with the classic Runge-Kutta method.

    Example:
        >>> x, y, vx, vy = _rk4_step(3.0, 0.0, 0.0, 1.0, 1.5, 0.02)
    """
    a1: _Vector = _acceleration(x, y, k)
    a2: _Vector = _acceleration(x + vx * dt / 2, y + vy * dt / 2, k)
    a3: _Vector = _acceleration(
        x + (vx + a1[0] * dt / 2) * dt / 2, y + (vy + a1[1] * dt / 2) * dt / 2, k
    )
    a4: _Vector = _acceleration(x + (vx + a2[0] * dt / 2) * dt, y + (vy + a2[1] * dt / 2) * dt, k)
    return (
        x + dt * (vx + dt * (a1[0] + a2[0] + a3[0]) / 6),
        y + dt * (vy + dt * (a1[1] + a2[1] + a3[1]) / 6),
        vx + dt * (a1[0] + 2 * a2[0] + 2 * a3[0] + a4[0]) / 6,
        vy + dt * (a1[1] + 2 * a2[1] + 2 * a3[1] + a4[1]) / 6,
    )


def trace_path(
    start_m: _Vector,
    direction: _Vector,
    black_hole_m: _Vector,
    schwarzschild_radius_m: float,
    map_size_m: float,
    max_length_m: float,
) -> list[_Vector]:
    """`trace_path` follows a laser from `start_m` until it falls in, leaves the map or is too long.

    Args:
        start_m: Where the laser starts.
        direction: Which way it starts; any nonzero length.
        black_hole_m: Where the black hole is.
        schwarzschild_radius_m: The black hole's Schwarzschild radius r_s.
        map_size_m: The map's side; the map runs from 0 to `map_size_m` in x and y.
        max_length_m: The longest the path may be.

    Returns:
        Points about `POINT_SPACING_M` apart along the path, starting with `start_m`.

    Raises:
        ValueError: `direction` is zero.

    Example:
        >>> points = trace_path((2.0, 12.0), (1.0, 0.0), (12.0, 12.0), 1.2, 24.0, 48.0)
    """
    length: float = math.hypot(*direction)
    if length == 0:
        raise ValueError("direction must not be zero")
    x: float = start_m[0] - black_hole_m[0]
    y: float = start_m[1] - black_hole_m[1]
    vx: float = direction[0] / length
    vy: float = direction[1] / length
    k: float = 1.5 * schwarzschild_radius_m * (x * vy - y * vx) ** 2
    points: list[_Vector] = [start_m]
    path_length_m: float = 0.0
    since_last_point_m: float = 0.0
    while path_length_m < max_length_m:
        next_x, next_y, vx, vy = _rk4_step(x, y, vx, vy, k, _STEP_M)
        step_length_m: float = math.hypot(next_x - x, next_y - y)
        x, y = next_x, next_y
        path_length_m += step_length_m
        since_last_point_m += step_length_m
        map_x_m: float = x + black_hole_m[0]
        map_y_m: float = y + black_hole_m[1]
        is_inside_map: bool = 0 <= map_x_m <= map_size_m and 0 <= map_y_m <= map_size_m
        if math.hypot(x, y) <= schwarzschild_radius_m or not is_inside_map:
            points.append((map_x_m, map_y_m))
            break
        if since_last_point_m >= POINT_SPACING_M:
            points.append((map_x_m, map_y_m))
            since_last_point_m = 0.0
    return points


def _segment_touches_square(
    start: _Vector, end: _Vector, center: _Vector, half_size_m: float
) -> bool:
    """`_segment_touches_square` checks a segment against a square with the slab method.

    Example:
        >>> _segment_touches_square((0.0, 0.0), (2.0, 0.0), (1.0, 0.0), 0.4)
        True
    """
    enter: float = 0.0
    leave: float = 1.0
    for axis in (0, 1):
        delta: float = end[axis] - start[axis]
        low: float = center[axis] - half_size_m
        high: float = center[axis] + half_size_m
        if delta == 0:
            if not low <= start[axis] <= high:
                return False
            continue
        t_low: float = (low - start[axis]) / delta
        t_high: float = (high - start[axis]) / delta
        enter = max(enter, min(t_low, t_high))
        leave = min(leave, max(t_low, t_high))
    return enter <= leave


def touches_square(points: list[_Vector], center: _Vector, half_size_m: float) -> bool:
    """`touches_square` checks whether the path through `points` touches a square.

    Example:
        >>> touches_square([(0.0, 0.0), (2.0, 0.0)], (1.0, 0.0), 0.4)
        True
    """
    return any(
        _segment_touches_square(points[i], points[i + 1], center, half_size_m)
        for i in range(len(points) - 1)
    )
