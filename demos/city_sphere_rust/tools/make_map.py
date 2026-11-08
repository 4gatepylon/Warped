"""`make_map.py` writes City Sphere's map, `common/maps/city_sphere/map.json`, from the layout below.

Euclidean space, metres, the sphere's centre at the origin. The street is the inside of a sphere of
`STREET_RADIUS_M`; gravity points outward onto it, so a player's up is towards the centre. The glass
shell of `GLASS_RADIUS_M` roofs the street, and players can stand on its inner (centre-facing) side,
the glass level, which has no roof. Lifts are holes in the glass over lift pads. Blue's half is +x,
red's is -x, and every red thing is the antipode of a blue one.

Example:
    python3 tools/make_map.py
"""

import json
import math
from pathlib import Path

import numpy as np

STREET_RADIUS_M: float = 24.0
GLASS_RADIUS_M: float = 17.0
GLASS_THICKNESS_M: float = 0.25
CORE_RADIUS_M: float = 2.5
LIFT_HOLE_RAD: float = 0.11
OUTPUT: Path = Path(__file__).resolve().parents[3] / "common" / "maps" / "city_sphere" / "map.json"
MATERIALS: list[tuple[str, tuple[float, float, float], str]] = [
    ("air", (0.0, 0.0, 0.0), "empty"),
    ("street", (0.32, 0.32, 0.34), "solid"),
    ("building", (0.62, 0.6, 0.58), "solid"),
    ("brick", (0.55, 0.36, 0.3), "solid"),
    ("glass", (0.55, 0.75, 0.95), "glass"),
    ("glass obstacle", (0.45, 0.47, 0.52), "solid"),
    ("core", (1.0, 0.75, 1.0), "light"),
    ("lift", (0.7, 0.55, 1.0), "light"),
    ("blue pylon", (0.25, 0.45, 1.0), "light"),
    ("red pylon", (1.0, 0.28, 0.22), "light"),
    ("blue console 0", (0.3, 0.55, 1.0), "light"),
    ("blue console 1", (0.3, 0.55, 1.0), "light"),
    ("red console 0", (1.0, 0.35, 0.3), "light"),
    ("red console 1", (1.0, 0.35, 0.3), "light"),
]


def material_id(name: str) -> int:
    """`material_id` gives the id of the material called `name`.

    Example:
        >>> material_id("air")
        0
    """
    return [m[0] for m in MATERIALS].index(name)


def direction(longitude_rad: float, latitude_rad: float) -> np.ndarray:
    """`direction` is the unit vector at a longitude (from +x towards -z) and latitude (towards +y).

    Example:
        >>> direction(0.0, 0.0).tolist()
        [1.0, 0.0, 0.0]
    """
    return np.array(
        [
            math.cos(latitude_rad) * math.cos(longitude_rad),
            math.sin(latitude_rad),
            -math.cos(latitude_rad) * math.sin(longitude_rad),
        ]
    )


def make_frame(d: np.ndarray, radius_m: float, heading_rad: float = 0.0) -> list[float]:
    """`make_frame` is the rigid frame at `radius_m` along `d`, its +y towards the centre and turned
    `heading_rad` about it, column-major.

    Example:
        >>> make_frame(np.array([1.0, 0.0, 0.0]), 24.0)[12:]
        [24.0, 0.0, 0.0, 1.0]
    """
    up: np.ndarray = -d / np.linalg.norm(d)
    helper: np.ndarray = np.array([0.0, 1.0, 0.0]) if abs(up[1]) < 0.9 else np.array([0.0, 0.0, 1.0])
    right: np.ndarray = np.cross(helper, up)
    right /= np.linalg.norm(right)
    back: np.ndarray = np.cross(right, up)
    c, s = math.cos(heading_rad), math.sin(heading_rad)
    right, back = c * right - s * back, s * right + c * back
    origin: np.ndarray = d / np.linalg.norm(d) * radius_m
    columns = [list(right) + [0.0], list(up) + [0.0], list(back) + [0.0], list(origin) + [1.0]]
    return [round(float(v), 6) for column in columns for v in column]


def make_box(name: str, group: str, d: np.ndarray, base_radius_m: float, half: tuple, material: str,
             heading_rad: float = 0.0) -> dict:
    """`make_box` is a box standing on the surface of radius `base_radius_m` (its bottom there).

    Example:
        >>> make_box("a", "city", np.array([1.0, 0, 0]), 24.0, (1, 1, 1), "building")["half_size_m"]
        [1, 1, 1]
    """
    return {
        "name": name,
        "group": group,
        "frame": make_frame(d, base_radius_m - half[1], heading_rad),
        "half_size_m": list(half),
        "material": material_id(material),
    }


def make_team_half(sign: float, team: str) -> tuple[list[dict], list[dict]]:
    """`make_team_half` gives one team's boxes and spawns; red's (sign -1) are blue's antipodes.

    Example:
        >>> len(make_team_half(1.0, "blue")[1])
        2
    """
    s = lambda d: sign * d
    boxes: list[dict] = []
    city = [
        (0.35, 0.45, (2.2, 2.8, 1.6), "building", 0.3),
        (0.35, -0.45, (1.8, 3.5, 2.0), "brick", -0.2),
        (0.75, 0.2, (2.5, 2.2, 1.5), "building", 0.9),
        (0.7, -0.3, (1.5, 4.0, 1.5), "building", 0.0),
        (1.1, 0.55, (2.0, 3.0, 2.0), "brick", 0.5),
        (1.15, -0.1, (2.8, 2.5, 1.2), "building", 1.2),
        (0.2, 0.85, (2.5, 3.2, 2.0), "building", 0.0),
        (0.25, -0.9, (2.0, 2.6, 2.4), "brick", 0.4),
        (1.25, -0.65, (1.6, 3.8, 1.6), "building", 0.7),
        (0.95, 0.95, (2.2, 2.4, 1.8), "brick", 0.2),
    ]
    for k, (lon, lat, half, material, heading) in enumerate(city):
        boxes.append(make_box(f"{team} block {k}", "city", s(direction(lon, lat)), STREET_RADIUS_M, half, material, heading))
    glass = [
        (0.3, 0.3, (1.2, 1.1, 0.4), 0.0),
        (0.55, -0.25, (0.5, 1.4, 1.6), 0.4),
        (0.85, 0.45, (1.5, 1.0, 0.4), 1.0),
        (1.0, -0.35, (0.4, 1.2, 1.4), 0.2),
        (1.3, 0.15, (1.6, 1.3, 0.4), 0.8),
        (0.6, 0.85, (0.5, 1.2, 1.5), 0.0),
    ]
    for k, (lon, lat, half, heading) in enumerate(glass):
        boxes.append(make_box(f"{team} glass cover {k}", "glass level", s(direction(lon, lat)), GLASS_RADIUS_M, half, "glass obstacle", heading))
    boxes.append(make_box(f"{team} pylon street", "pylons", s(direction(0.5, 0.0)), STREET_RADIUS_M, (0.45, 2.6, 0.45), f"{team} pylon"))
    boxes.append(make_box(f"{team} console street", "consoles", s(direction(0.5, 0.0) + np.array([0, 0, -0.06])), STREET_RADIUS_M, (0.4, 0.55, 0.25), f"{team} console 0"))
    boxes.append(make_box(f"{team} pylon glass", "pylons", s(direction(0.9, -0.2)), GLASS_RADIUS_M, (0.4, 2.0, 0.4), f"{team} pylon"))
    boxes.append(make_box(f"{team} console glass", "consoles", s(direction(0.9, -0.2) + np.array([0, 0, -0.08])), GLASS_RADIUS_M, (0.35, 0.5, 0.22), f"{team} console 1"))
    spawns = [
        {"team": team, "frame": make_frame(s(direction(0.0, 0.12)), STREET_RADIUS_M, math.pi / 2)},
        {"team": team, "frame": make_frame(s(direction(0.0, -0.12)), STREET_RADIUS_M, math.pi / 2)},
    ]
    return boxes, spawns


def get_lift_directions() -> list[np.ndarray]:
    """`get_lift_directions` gives the lifts' directions: two per team near its spawn, two in the
    middle, every one with its antipode.

    Example:
        >>> len(get_lift_directions())
        6
    """
    blue: list[np.ndarray] = [direction(0.15, 0.4), direction(0.15, -0.4), direction(math.pi / 2, 0.0)]
    return blue + [-d for d in blue]


def main() -> None:
    """`main` writes the map.

    Example:
        main()
    """
    boxes: list[dict] = []
    spawns: list[dict] = []
    for sign, team in ((1.0, "blue"), (-1.0, "red")):
        team_boxes, team_spawns = make_team_half(sign, team)
        boxes += team_boxes
        spawns += team_spawns
    for k, d in enumerate(get_lift_directions()):
        boxes.append(make_box(f"lift pad {k}", "lifts", d, STREET_RADIUS_M, (0.9, 0.05, 0.9), "lift"))
    manifest: dict = {
        "format": "warped-map",
        "version": 1,
        "name": "city_sphere",
        "description": "A satellite city inside a hollow sphere in Euclidean space: streets on the inner surface (gravity outward), a glass shell roofing them that is the open glass level, lifts up to it, and a glowing core.",
        "source": "demos/levels/city_sphere/index.html",
        "space": {"kind": "euclidean"},
        "gravity_m_per_s2": 9.8,
        "player": {"eye_height_m": 1.6, "walk_m_per_s": 5.0, "sprint_m_per_s": 8.5, "jump_m_per_s": 5.0, "fly_m_per_s": 12.0},
        "materials": [{"id": k, "name": n, "color": list(c), "kind": kind} for k, (n, c, kind) in enumerate(MATERIALS)],
        "ground": {"material": material_id("street"), "sphere": {"radius_m": STREET_RADIUS_M}},
        "shells": [
            {
                "name": "glass",
                "radius_m": GLASS_RADIUS_M,
                "thickness_m": GLASS_THICKNESS_M,
                "material": material_id("glass"),
                "walkable_side": "inside",
                "laser": "reflect_inside",
                "holes": [{"direction": [round(float(v), 6) for v in d], "angular_radius_rad": LIFT_HOLE_RAD} for d in get_lift_directions()],
            },
            {"name": "core", "radius_m": CORE_RADIUS_M, "thickness_m": CORE_RADIUS_M, "material": material_id("core"), "walkable_side": "none", "laser": "stop", "holes": []},
        ],
        "boxes": boxes,
        "lights": [{"position": [0.0, 0.0, 0.0, 1.0], "color": [1.0, 0.82, 1.0], "radius_m": 30.0}],
        "sky": {"transport": "parallel", "sun": {"direction": [0.0, 1.0, 0.0], "color": [1.0, 0.85, 1.0]}, "stars": False, "clouds": False, "zenith_color": [0.05, 0.03, 0.09], "horizon_color": [0.12, 0.08, 0.18]},
        "spawns": spawns,
        "appearance": {"style": "city_sphere", "lighting": {"model": "ambient_sun", "ambient": 0.35, "sun": 0.65}},
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(manifest, indent=1) + "\n")
    print(f"Wrote {OUTPUT}")


if __name__ == "__main__":
    main()
