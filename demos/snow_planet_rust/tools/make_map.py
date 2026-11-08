"""`make_map.py` writes Snow Planet's map, `common/maps/snow_planet/map.json`, from the layout below.

The map is Euclidean (E³, x right, y up, z back) with the ground's two axes periodic (`space.torus_periods_m`). It is
drawn in Fermi coordinates: u runs along -z (the teams' axis: blue at u < 0, red at u > 0) and v along +x. The red
half is the blue half turned half a turn about the vertical through the origin, so the teams' sides match.

Example: `python3 tools/make_map.py` (or `make map`) from the demo's folder.
"""

import json
import math
from pathlib import Path

MAP_PATH: Path = Path(__file__).resolve().parents[3] / "common" / "maps" / "snow_planet" / "map.json"
PERIOD_U_M: float = 112.0
PERIOD_V_M: float = 72.0
PYLON_HEIGHT_M: float = 12.0
PYLON_RADIUS_M: float = 1.35
PYLON_CORE_M: float = 5.654
ICE, SNOW, PACKED, ROCK, METAL = 3, 1, 2, 4, 5
BLUE_PAINT, RED_PAINT = 6, 7
CONSOLES: dict[str, list[int]] = {"blue": [11, 12], "red": [13, 14]}
TEAM_LIGHT: dict[str, list[float]] = {"blue": [0.25, 0.55, 1.0], "red": [1.0, 0.28, 0.18]}


def make_frame(u_m: float, v_m: float, h_m: float = 0.0, yaw_rad: float = 0.0) -> list[float]:
    """`make_frame` gives a frame's 16 numbers (column-major) at Fermi (u, v, h), turned `yaw_rad` to the left.

    Example:
        >>> make_frame(0.0, 0.0)[12:]
        [0.0, 0.0, -0.0, 1.0]
    """
    c, s = math.cos(yaw_rad), math.sin(yaw_rad)
    return [c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, v_m, h_m, -u_m, 1.0]


def make_box(name: str, u_m: float, v_m: float, half_m: list[float], material: int, yaw_rad: float = 0.0,
             kind: str = "box") -> dict:
    """`make_box` gives a structure of one shape standing on the ground at (u, v).

    Example:
        >>> make_box("bank", 1.0, 2.0, [1.0, 0.5, 1.0], 2)["shapes"][0]["centre_m"]
        [0.0, 0.5, 0.0]
    """
    shape: dict = {"kind": kind, "centre_m": [0.0, half_m[1], 0.0], "half_m": half_m, "material": material}
    if kind == "box":
        shape["round_m"] = min(0.25, min(half_m) / 2)
    if kind == "cylinder":
        shape["axis"] = 1
    return {"name": name, "frame": make_frame(u_m, v_m, 0.0, yaw_rad), "shapes": [shape]}


# Blue's half: (name, u, v, half sizes, material, yaw, kind). Red's is the same turned half a turn.
BLUE_COVER: list[tuple] = [
    ("spawn snow wall", -40.0, 0.0, [4.0, 1.1, 0.6], PACKED, math.pi / 2, "box"),
    ("ice wall", -27.0, 0.0, [3.5, 1.6, 0.35], ICE, math.pi / 2, "box"),
    ("snow bank", -30.0, 9.0, [2.6, 0.8, 1.1], PACKED, 0.3, "box"),
    ("snow bank", -30.0, -9.0, [2.6, 0.8, 1.1], PACKED, -0.3, "box"),
    ("ice pillar", -18.0, 7.0, [1.1, 2.6, 1.1], ICE, 0.0, "cylinder"),
    ("ice pillar", -18.0, -7.0, [1.1, 2.6, 1.1], ICE, 0.0, "cylinder"),
    ("rock", -12.0, 26.0, [3.2, 2.0, 2.6], ROCK, 0.4, "ellipsoid"),
    ("rock", -34.0, -26.0, [2.8, 1.8, 2.2], ROCK, -0.6, "ellipsoid"),
    ("ice block", -22.0, 18.0, [1.6, 1.4, 1.6], ICE, 0.5, "box"),
    ("snow bank", -8.0, -12.0, [2.2, 0.9, 1.0], PACKED, 1.2, "box"),
    ("pylon wall", -34.0, 11.0, [2.5, 1.2, 0.4], PACKED, 0.0, "box"),
    ("pylon wall", -34.0, -11.0, [2.5, 1.2, 0.4], PACKED, 0.0, "box"),
]
BLIZZARDS: list[dict] = [
    {"name": "white-out", "u_m": 0.0, "v_m": 0.0, "radius_m": 10.0},
    {"name": "north squall", "u_m": -14.0, "v_m": 24.0, "radius_m": 7.0},
    {"name": "south squall", "u_m": 14.0, "v_m": -24.0, "radius_m": 7.0},
]
BLIZZARD_NUMBERS: dict = {
    "height_m": 16.0,
    "speed_frac": 0.5,
    "freeze_health_per_s": 0.08,
    "shot_reach_m": 8.0,
    "fog_per_m": 0.12,
    "snow_per_m3": 0.6,
}


def mirror(u_m: float, v_m: float, yaw_rad: float) -> tuple[float, float, float]:
    """`mirror` turns a place on blue's half into red's: half a turn about the origin's vertical.

    Example:
        >>> mirror(1.0, 2.0, 0.0)
        (-1.0, -2.0, 3.141592653589793)
    """
    return -u_m, -v_m, yaw_rad + math.pi


def make_pylon(team: str, index: int, u_m: float, v_m: float) -> dict:
    """`make_pylon` gives a team's pylon `index` (its console's material) at (u, v): the beacon model on a cylinder.

    Example:
        >>> make_pylon("blue", 0, -34.0, 16.0)["shapes"][0]["material"]
        11
    """
    return {
        "name": f"{team} pylon",
        "frame": make_frame(u_m, v_m),
        "mesh": {"model": "scifi_beacon", "height_m": PYLON_HEIGHT_M},
        "shapes": [{"kind": "cylinder", "centre_m": [0.0, PYLON_CORE_M, 0.0],
                    "half_m": [PYLON_RADIUS_M, PYLON_HEIGHT_M - PYLON_CORE_M, PYLON_RADIUS_M], "round_m": 0.0,
                    "axis": 1, "material": CONSOLES[team][index]}],
    }


def make_map() -> dict:
    """`make_map` gives the whole manifest.

    Example:
        >>> make_map()["space"]["kind"]
        'euclidean'
    """
    structures: list[dict] = []
    for name, u_m, v_m, half_m, material, yaw_rad, kind in BLUE_COVER:
        structures.append(make_box(f"blue {name}", u_m, v_m, half_m, material, yaw_rad, kind))
        ru, rv, ryaw = mirror(u_m, v_m, yaw_rad)
        structures.append(make_box(f"red {name}", ru, rv, half_m, material, ryaw, kind))
    lights: list[dict] = []
    for team, sign in (("blue", -1.0), ("red", 1.0)):
        for index, v_m in enumerate((16.0, -16.0)):
            u_m = sign * 34.0
            structures.append(make_pylon(team, index, u_m, sign * v_m))
            lights.append({"position": [sign * v_m, 8.0, -u_m, 1.0], "color": TEAM_LIGHT[team], "radius_m": 8.0})
    warps: list[dict] = []
    for name, u_m, v_m in (("black hole north", 0.0, 17.0), ("black hole south", 0.0, -17.0)):
        warps.append({"kind": "black_hole", "name": name, "frame": make_frame(u_m, v_m, 6.0),
                      "law": "isotropic_schwarzschild", "schwarzschild_radius_m": 1.1, "radial_coordinate": "distance",
                      "reach_m": 11.0, "falloff": "taper"})
    for name, u_m, v_m, yaw_rad, partner in (("wormhole mouth A", -22.0, -27.0, 0.0, 3),
                                              ("wormhole mouth B", 22.0, 27.0, math.pi, 2)):
        warps.append({"kind": "wormhole_mouth", "name": name, "frame": make_frame(u_m, v_m, 2.4, yaw_rad),
                      "law": "foil", "schwarzschild_radius_m": 1.6, "lambda_squared": 0.25,
                      "radial_coordinate": "distance", "reach_m": 9.0, "falloff": "taper", "partner": partner})
    materials: list[dict] = [
        {"id": 0, "name": "air", "color": [0, 0, 0], "kind": "empty"},
        {"id": SNOW, "name": "snow", "color": [0.86, 0.9, 0.95], "kind": "solid"},
        {"id": PACKED, "name": "packed snow", "color": [0.78, 0.83, 0.9], "kind": "solid"},
        {"id": ICE, "name": "ice", "color": [0.62, 0.82, 0.95], "kind": "glass"},
        {"id": ROCK, "name": "rock", "color": [0.32, 0.33, 0.36], "kind": "solid"},
        {"id": METAL, "name": "metal", "color": [0.46, 0.48, 0.52], "kind": "solid"},
        {"id": BLUE_PAINT, "name": "blue paint", "color": [0.14, 0.3, 0.74], "kind": "solid"},
        {"id": RED_PAINT, "name": "red paint", "color": [0.72, 0.12, 0.1], "kind": "solid"},
        {"id": 8, "name": "blue light", "color": [0.25, 0.55, 1.0], "kind": "light"},
        {"id": 9, "name": "red light", "color": [1.0, 0.16, 0.1], "kind": "light"},
        {"id": 10, "name": "white light", "color": [1.0, 0.94, 0.82], "kind": "light"},
    ]
    for team, ids in CONSOLES.items():
        for index, material in enumerate(ids):
            materials.append({"id": material, "name": f"{team} console {index + 1}", "color": TEAM_LIGHT[team],
                              "kind": "light"})
    sun: list[float] = [-0.55, 0.42, 0.72]
    norm: float = math.sqrt(sum(c * c for c in sun))
    return {
        "format": "warped-map",
        "version": 1,
        "name": "snow_planet",
        "description": "A snow field on a torus (T² × ℝ): two pylons a team, two black holes, a wormhole pair, "
        "ice cover and three blizzards.",
        "source": "demos/snow_planet_rust/tools/make_map.py (after demos/levels/snow_planet/index.html)",
        "space": {"kind": "euclidean", "torus_periods_m": [PERIOD_U_M, PERIOD_V_M]},
        "gravity_m_per_s2": 9.81,
        "player": {"eye_height_m": 1.6, "walk_m_per_s": 5.0, "sprint_m_per_s": 9.0, "jump_m_per_s": 5.2,
                   "fly_m_per_s": 14.0},
        "materials": materials,
        "ground": {"material": SNOW},
        "structures": structures,
        "warps": warps,
        "lights": lights,
        "sky": {"transport": "parallel", "sun": {"direction": [c / norm for c in sun], "color": [1.0, 0.95, 0.88]},
                "stars": False, "clouds": True, "zenith_color": [0.42, 0.56, 0.74],
                "horizon_color": [0.82, 0.87, 0.93], "fog_m": 90.0},
        "spawns": [{"team": "blue", "frame": make_frame(-48.0, 0.0)},
                   {"team": "red", "frame": make_frame(48.0, 0.0, 0.0, math.pi)}],
        "regions": {
            "zones": [{"team": "blue", "u_min_m": None, "u_max_m": -20.0},
                      {"team": "middle", "u_min_m": -20.0, "u_max_m": 20.0},
                      {"team": "red", "u_min_m": 20.0, "u_max_m": None}],
            "boundary": {"kind": "fermi_ellipse", "u_m": 60.0, "v_m": 40.0},
            "zone_walls": {"height_m": 6.0, "thickness_m": 0.6, "opacity_frac": 0.25},
            "blizzards": [{**blizzard, **BLIZZARD_NUMBERS} for blizzard in BLIZZARDS],
        },
        "appearance": {"style": "snow", "lighting": {"model": "lunar_lambert", "lommel_seeliger_frac": 0.2,
                                                     "ambient": 0.3, "earthshine": 0.0, "sun_intensity": 1.2,
                                                     "shadows": True}},
    }


if __name__ == "__main__":
    MAP_PATH.write_text(json.dumps(make_map(), indent=1) + "\n")
    print(f"Wrote {MAP_PATH}")
