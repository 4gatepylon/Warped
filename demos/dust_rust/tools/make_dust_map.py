"""`make_dust_map.py` writes Dust's map, `common/maps/dust/map.json`, from the layout below.

Dust is a point-symmetric desert basin in hyperbolic space (R = 50 m): blue spawns at u = -104 m,
red at +104 m, three pylons a team, twin sandstone ridges with an arch each, mesas and pillars in
the middle, and one fixed wormhole pair across the flanks, centred at eye height so players walk
through them (W-126, same play in every level), moved up to `MOUTH_JITTER_M` by `MOUTH_SEED` with
mouth B mirroring A; no black hole. Heights follow `src/terrain.rs` so things stand on the ground.
Standard library only.

Usage (from demos/dust_rust/): python3 tools/make_dust_map.py [output path]
"""

import json
import math
import random
import sys
from pathlib import Path

R_M = 50.0
LASER_COLOR = [1.0, 0.8, 0.3]
JITTER_SEED = 121
JITTER_M = 1.5
JITTER_YAW_RAD = 0.3
JITTER_SCALE_FRAC = 0.12
EYE_HEIGHT_M = 1.6
MOUTH_A_SPOT_M = (-50.0, -40.0)
MOUTH_YAW_RAD = 0.6
MOUTH_JITTER_M = 1.0
MOUTH_SEED = JITTER_SEED + 1
WIND_LOG_INDEX_GRADIENT_PER_M = 0.00144
GUST_STRENGTH_FRAC = 0.2
GUST_SIZE_M = 3000
LASER_GRADIENT_PER_VIEW_GRADIENT = 2.4
OUTPUT = Path(__file__).resolve().parents[3] / "common/maps/dust/map.json"
TERRAIN = {"kind": "crater", "rim_u_m": 170, "rim_v_m": 85, "wall_m": 25, "rim_height_m": 16,
           "relief_m": 0.7, "relief_wavelength_m": 22, "seed": 5}
MATERIALS = [
    ("air", [0, 0, 0], "empty"), ("sand", [0.78, 0.62, 0.42], "solid"),
    ("dark sand", [0.62, 0.5, 0.36], "solid"), ("sandstone", [0.72, 0.52, 0.36], "solid"),
    ("boulder", [0.6, 0.45, 0.32], "solid"), ("hull", [0.8, 0.76, 0.68], "solid"),
    ("metal", [0.46, 0.46, 0.46], "solid"), ("glass", [0.55, 0.75, 0.85], "glass"),
    ("red light", [1, 0.16, 0.1], "light"), ("blue light", [0.25, 0.55, 1], "light"),
    ("red paint", [0.72, 0.12, 0.1], "solid"), ("blue paint", [0.14, 0.3, 0.74], "solid"),
    ("white light", [1, 0.94, 0.82], "light"), ("blue pylon core", [0.4, 0.75, 1], "light"),
    ("red pylon core", [1, 0.3, 0.22], "light"),
    ("blue console 1", [0.3, 0.65, 1], "light"), ("blue console 2", [0.3, 0.65, 1], "light"),
    ("blue console 3", [0.3, 0.65, 1], "light"), ("red console 1", [1, 0.28, 0.18], "light"),
    ("red console 2", [1, 0.28, 0.18], "light"), ("red console 3", [1, 0.28, 0.18], "light"),
    ("sandstone blocks", [0.74, 0.6, 0.44], "solid"),
]
MAT = {name: i for i, (name, _, _) in enumerate(MATERIALS)}
TEAM_LIGHT = {"blue": [0.165, 0.341, 0.55], "red": [0.55, 0.088, 0.044]}


def get_hash(i: int, j: int, seed: int) -> float:
    """`get_hash` is `terrain.rs`'s `hash`: lattice point (i, j)'s value in [0, 1]."""
    m = 0xFFFFFFFF
    h = ((i & m) * 0x8DA6B343 & m) ^ ((j & m) * 0xD8163841 & m) ^ (seed * 0xCB1AB31F & m)
    h ^= h >> 13
    h = h * 0x5BD1E995 & m
    h ^= h >> 15
    return h / m


def smooth(t: float) -> float:
    t = min(max(t, 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def sample_noise(x: float, y: float, seed: int) -> float:
    i, j = math.floor(x), math.floor(y)
    fx, fy = smooth(x - i), smooth(y - j)
    bottom = get_hash(i, j, seed) + (get_hash(i + 1, j, seed) - get_hash(i, j, seed)) * fx
    top = get_hash(i, j + 1, seed) + (get_hash(i + 1, j + 1, seed) - get_hash(i, j + 1, seed)) * fx
    return bottom + (top - bottom) * fy


def sample_fbm(x: float, y: float, seed: int) -> float:
    total, amplitude, scale = 0.0, 0.5, 1.0
    for octave in range(5):
        total += amplitude * sample_noise(x * scale, y * scale, seed + octave)
        amplitude *= 0.5
        scale *= 2.0
    return total / (1.0 - 0.5**5)


def get_height_m(u: float, v: float, mounds: list[dict]) -> float:
    """`get_height_m` is `Terrain::get_height_m` with the mounds summed exactly."""
    t = TERRAIN
    inside = (1.0 - math.hypot(u / t["rim_u_m"], v / t["rim_v_m"])) * t["rim_v_m"]
    wall = smooth(1.0 - inside / t["wall_m"]) if inside >= 0 else 1.0 - smooth(-inside / (3.0 * t["wall_m"]))
    rim = t["rim_height_m"] * (1.0 + 0.5 * (sample_noise(u / 60.0, v / 60.0, t["seed"] ^ 9) - 0.5))
    relief = sample_fbm(u / t["relief_wavelength_m"], v / t["relief_wavelength_m"], t["seed"])
    heap = sum(m["height_m"] * (1 - d2) ** 2 for m in mounds
               if (d2 := ((u - m["u_m"]) ** 2 + (v - m["v_m"]) ** 2) / m["radius_m"] ** 2) < 1.0)
    return rim * wall + heap + t["relief_m"] * relief * (1.0 + 3.0 * wall)


def multiply(a: list[list[float]], b: list[list[float]]) -> list[list[float]]:
    return [[sum(a[r][k] * b[k][c] for k in range(4)) for c in range(4)] for r in range(4)]


def boost(axis: int, s: float) -> list[list[float]]:
    m = [[float(r == c) for c in range(4)] for r in range(4)]
    m[axis][axis] = m[3][3] = math.cosh(s)
    m[axis][3] = m[3][axis] = math.sinh(s)
    return m


def make_frame(u: float, v: float, h: float, yaw: float = 0.0) -> list[float]:
    """`make_frame` is the Fermi frame at (u, v, h) m turned `yaw` about up, column-major.

    Yaw 0 faces +u (red's end); yaw pi faces -u.
    """
    c, s = math.cos(yaw), math.sin(yaw)
    turn = [[c, 0, s, 0], [0, 1, 0, 0], [-s, 0, c, 0], [0, 0, 0, 1]]
    m = multiply(multiply(multiply(boost(2, -u / R_M), boost(0, v / R_M)), boost(1, h / R_M)), turn)
    return [m[r][col] for col in range(4) for r in range(4)]


def make_point(u: float, v: float, h: float) -> list[float]:
    return make_frame(u, v, h)[12:]


def box(centre: list[float], half: list[float], material: str, round_m: float = 0.0, kind: str = "box") -> dict:
    return {"kind": kind, "centre_m": centre, "half_m": half, "round_m": round_m, "axis": 1, "material": MAT[material]}


def mirror(team: str) -> str:
    return {"blue": "red", "red": "blue"}.get(team, team)


def scale_shapes(shapes: list[dict], s: float) -> list[dict]:
    """`scale_shapes` scales the centres, half sizes and rounding of `shapes` by `s` about the frame's origin."""
    return [dict(sh, centre_m=[c * s for c in sh["centre_m"]], half_m=[c * s for c in sh["half_m"]],
                 round_m=sh["round_m"] * s) for sh in shapes]


def main() -> None:
    jitter = random.Random(JITTER_SEED)

    def nudge(u: float, v: float, yaw: float) -> tuple[float, float, float, float]:
        """`nudge` jitters a natural prop's place, turn and size from `JITTER_SEED`; `place` mirrors the result."""
        return (u + jitter.uniform(-JITTER_M, JITTER_M), v + jitter.uniform(-JITTER_M, JITTER_M),
                yaw + jitter.uniform(-JITTER_YAW_RAD, JITTER_YAW_RAD),
                1.0 + jitter.uniform(-JITTER_SCALE_FRAC, JITTER_SCALE_FRAC))

    mounds: list[dict] = []
    dunes = [(u, 25 + 2.5 * math.sin(u / 7.0), 8.5, 3.4) for u in [-40, -32, -24, -16, -9, 9, 16, 24, 32, 40]]
    dunes += [(-70, 50, 16, 4.0), (-30, 52, 14, 3.2), (10, 55, 18, 4.5), (60, 50, 15, 3.6),
              (-95, 30, 10, 1.8), (-62, -6, 7, 1.4), (-40, -52, 13, 3.0), (-8, -44, 9, 2.0)]
    for u, v, r, h in dunes:
        u, v, _, s = nudge(u, v, 0.0)
        for sign in (1, -1):
            mounds.append({"u_m": sign * u, "v_m": sign * v, "radius_m": r * s, "height_m": h * s})
    ground = lambda u, v: get_height_m(u, v, mounds)  # noqa: E731
    structures: list[dict] = []
    lights: list[dict] = []

    def place(name, u, v, yaw, shapes, mesh=None, sink_m=0.0):
        for k, (su, sv, team) in enumerate([(u, v, name), (-u, -v, None)]):
            label = name if k == 0 else " ".join(mirror(w) for w in name.split(" "))
            entry = {"name": label, "frame": make_frame(su, sv, ground(su, sv) - sink_m, yaw + math.pi * k)}
            if mesh:
                entry["mesh"] = mesh
            red_shapes = [dict(s, material=MAT[mirror_material(MATERIALS[s["material"]][0])]) for s in shapes]
            entry["shapes"] = shapes if k == 0 else red_shapes
            structures.append(entry)

    def mirror_material(name: str) -> str:
        words = name.split(" ")
        return " ".join(mirror(w) for w in words)

    for i, (u, v) in enumerate([(-84, -26), (-92, 6), (-78, 30)]):
        place("blue pylon", u, v, 0.0, [box([0, 5.654, 0], [1.35, 6.346, 1.35], f"blue console {i + 1}", kind="cylinder")],
              {"model": "scifi_beacon", "height_m": 12})
        for su, sv, team in [(u, v, "blue"), (-u, -v, "red")]:
            lights.append({"position": make_point(su, sv, ground(su, sv) + 12.6), "color": TEAM_LIGHT[team], "radius_m": 6})
    place("blue kazemat", -94, -12, 0.0, [box([0, 2.163, 0], [5.143, 2.98, 3.429], "hull")],
          {"model": "stekelvarken_kazemat", "height_m": 6})
    place("blue field bunker", -30, -32, 0.35, [box([0, 2.21, 3.85], [5.657, 3.29, 5.893], "hull"),
                                                 box([-1.021, 1.188, -5.893], [4.636, 2.269, 3.85], "hull")],
          {"model": "ww2_field_bunker", "height_m": 5.5})
    for u, v, yaw, n in [(-58, -14, 0.0, 4), (-60, 18, 0.2, 4), (-72, -42, -0.4, 3), (-100, 22, 0.0, 3)]:
        for k in range(n):
            du = (k - (n - 1) / 2) * 1.55
            place("hesco block", u - du * math.sin(yaw), v + du * math.cos(yaw), yaw,
                  [box([0, 0.381, 0], [0.775, 1.019, 0.775], "metal")], {"model": "hesco_barrier", "height_m": 1.4})
    for u in (-1.0,):
        v = 25.0
        place("sandstone arch", 0.0, v, 0.0, [box([-3.6, 2.6, 0], [1.3, 3.6, 1.6], "sandstone", 0.4),
                                             box([3.6, 2.6, 0], [1.3, 3.6, 1.6], "sandstone", 0.4),
                                             box([0, 6.9, 0], [4.9, 1.0, 1.7], "sandstone", 0.5)], sink_m=1.0)
    def place_natural(name, u, v, yaw, shapes, sink_m=0.0):
        u, v, yaw, s = nudge(u, v, yaw)
        place(name, u, v, yaw, scale_shapes(shapes, s), sink_m=sink_m * s)

    place_natural("sandstone mesa", -18, 9, 0.4, [box([0, 2.5, 0], [5.0, 3.5, 4.0], "sandstone", 1.2)], sink_m=1.0)
    place_natural("sandstone mesa", -52, 34, -0.3, [box([0, 3.0, 0], [4.0, 4.0, 6.0], "sandstone", 1.0)], sink_m=1.0)
    for u, v, rad, h in [(-40, 0, 1.3, 6.0), (-10, -12, 1.1, 5.0), (-36, -18, 1.5, 7.0), (-66, 4, 1.0, 4.5), (-5, 40, 1.4, 6.5)]:
        place_natural("sandstone pillar", u, v, 0.0, [box([0, h / 2 - 0.5, 0], [rad, h / 2 + 0.5, rad], "sandstone", 0.3, "cylinder")])
    for u, v, yaw, length in [(-24, -6, 0.5, 3.0), (-12, 36, 1.2, 2.5), (-36, 12, -0.2, 3.5), (-48, -22, 1.0, 2.5)]:
        place_natural("ruin wall", u, v, yaw, [box([0, 0.7, 0], [length, 1.6, 0.4], "sandstone blocks", 0.05)], sink_m=0.3)
    for u, v, rad in [(-26, 18, 1.6), (-44, 46, 2.2), (-14, -30, 1.4), (-88, -40, 2.0)]:
        place_natural("boulder", u, v, 0.0, [box([0, rad * 0.4, 0], [rad, rad * 0.8, rad * 1.2], "boulder", kind="ellipsoid")])
    mouth_jitter = random.Random(MOUTH_SEED)
    du, dv = (mouth_jitter.uniform(-MOUTH_JITTER_M, MOUTH_JITTER_M) for _ in range(2))
    a_u, a_v = MOUTH_A_SPOT_M[0] + du, MOUTH_A_SPOT_M[1] + dv
    mouths = [(a_u, a_v, MOUTH_YAW_RAD), (-a_u, -a_v, MOUTH_YAW_RAD + math.pi)]
    warps = [{"kind": "wormhole_mouth", "name": f"wormhole mouth {'AB'[k]}",
              "frame": make_frame(u, v, ground(u, v) + EYE_HEIGHT_M, yaw), "law": "foil", "schwarzschild_radius_m": 1.6,
              "lambda_squared": 0.25, "radial_coordinate": "distance", "reach_m": 15, "falloff": "taper",
              "partner": 1 - k} for k, (u, v, yaw) in enumerate(mouths)]
    sun = [0.55, 0.62, -0.42]
    norm = math.sqrt(sum(c * c for c in sun))
    manifest = {
        "format": "warped-map", "version": 1, "name": "dust",
        "description": "Dust: a desert basin in hyperbolic space (R = 50 m) under a constant sandstorm whose wind bends light, three pylons a team, fixed and thrown wormholes, no black hole.",
        "source": f"demos/dust_rust/tools/make_dust_map.py (layout after demos/levels/dust/index.html; natural props jittered from seed {JITTER_SEED}, mirrored for fairness)",
        "space": {"kind": "hyperbolic", "curvature_radius_m": R_M},
        "gravity_m_per_s2": 9.81,
        "player": {"eye_height_m": EYE_HEIGHT_M, "walk_m_per_s": 5, "sprint_m_per_s": 9, "jump_m_per_s": 5.2, "fly_m_per_s": 14},
        "materials": [{"id": i, "name": n, "color": c, "kind": k} for i, (n, c, k) in enumerate(MATERIALS)],
        "ground": {"material": MAT["sand"], "tiling": {"kind": "square_order_5", "cells_per_side": 36},
                   "terrain": dict(TERRAIN, mounds=mounds)},
        "structures": structures, "warps": warps, "lights": lights,
        "sky": {"transport": "parallel", "sun": {"direction": [c / norm for c in sun], "color": [1, 0.9, 0.74]},
                "stars": False, "clouds": False, "zenith_color": [0.62, 0.5, 0.36], "horizon_color": [0.8, 0.66, 0.48],
                "fog_m": 150},
        "atmosphere": {
            "fog": {"color": [0.74, 0.55, 0.35], "start_m": 4, "distance_m": 50, "max_frac": 0.94},
            "sand": {"gust_frac": 0.5, "streak_frac": 0.45, "sheet_frac": 0.35},
            "wind": {"direction": [1, 0, 0], "log_index_gradient_per_m": WIND_LOG_INDEX_GRADIENT_PER_M, "segment_m": 48,
                     "gust": {"seed": JITTER_SEED, "strength_frac": GUST_STRENGTH_FRAC, "size_m": GUST_SIZE_M, "speed_m_per_s": 14},
                     "laser_gradient_per_view_gradient": LASER_GRADIENT_PER_VIEW_GRADIENT},
        },
        "spawns": [{"team": "blue", "frame": make_frame(-104, 0, ground(-104, 0), 0.0)},
                   {"team": "red", "frame": make_frame(104, 0, ground(104, 0), math.pi)}],
        "regions": {"zones": [{"team": "blue", "u_min_m": None, "u_max_m": -48},
                              {"team": "middle", "u_min_m": -48, "u_max_m": 48},
                              {"team": "red", "u_min_m": 48, "u_max_m": None}],
                    "boundary": {"kind": "fermi_ellipse", "u_m": 128, "v_m": 64},
                    "zone_walls": {"height_m": 6, "thickness_m": 0.6, "opacity_frac": 0.25}},
        "appearance": {"style": "desert", "laser_color": LASER_COLOR, "lighting": {"model": "lunar_lambert", "lommel_seeliger_frac": 0.2, "ambient": 0.26,
                                                       "earthshine": 0.0, "sun_intensity": 1.25, "shadows": True}},
    }
    output = Path(sys.argv[1]) if len(sys.argv) > 1 else OUTPUT
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(manifest, indent=1) + "\n")
    print(f"Wrote {output}: {len(structures)} structures, {len(mounds)} mounds")


if __name__ == "__main__":
    main()
