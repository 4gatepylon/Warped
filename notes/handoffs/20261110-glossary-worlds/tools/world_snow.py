import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from annotate import annotate, REPO

def to_px(length, wid):
    return 1024 - 16 * length, 704 + 16 * wid

M = [
    ("blue spawn", 48, 0, "c", 34, "t"),
    ("blue left pylon", 34, -16, "c", 40, "r"),
    ("blue right pylon", 34, 16, "c", 40, "l"),
    ("blue mouth", 46, -20, "c", 62, "b"),
    ("blue mid mound", 27, 6, "c", 48, "r"),
    ("blue right mound", 44, 27, "c", 48, "t"),
    ("blue black hole", 22, 20, "c", 62, "b"),
    ("top mound", 14, -31, "c", 48, "r"),
    ("top black hole", 6, -21, "c", 62, "b"),
    ("blizzard", 0, 0, "c", 165, "t"),
    ("bottom black hole", 1, 28, "c", 62, "t"),
    ("bottom mound", -14, 31, "c", 48, "b"),
    ("red spawn", -48, 0, "c", 34, "t"),
    ("red left pylon", -34, 16, "c", 40, "t"),
    ("red right pylon", -34, -16, "c", 40, "r"),
    ("red mouth", -46, 20, "c", 62, "t"),
    ("red mid mound", -27, -6, "c", 48, "b"),
    ("red right mound", -44, -27, "c", 48, "t"),
    ("red left black hole", -22, 18, "c", 62, "b"),
    ("red right black hole", -21, -26, "c", 62, "t"),
]
annotate(REPO / "demos/snow_planet_rust/map_pictures/snow_planet.png", 1, to_px, M, "snow_planet_map.jpg", font_px=34)
