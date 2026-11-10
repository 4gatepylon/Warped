import sys, pathlib, math, collections
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from annotate import annotate, load, REPO, OUT
from PIL import Image, ImageChops

d = load("city_sphere")
def lonlat(frame):
    x, y, z = frame[12], frame[13], frame[14]
    r = math.sqrt(x * x + y * y + z * z)
    return math.degrees(math.atan2(z, x)), math.degrees(math.asin(y / r))
def make_to_px(roll_px):
    def to_px(lon, lat):
        return (496 + lon * 992 / 360 + roll_px) % 992, 248 - lat * 496 / 180
    return to_px

by_group = collections.defaultdict(list)
for b in d["boxes"]:
    by_group[b["group"]].append((b["name"], *lonlat(b["frame"])))

FLIP = {"t": "b", "b": "t", "l": "r", "r": "l"}
EXCEPTIONS = {"cover 0": "l", "block 2": "b", "stairs": "b"}
def pick_anchor(name, team, default):
    anchor = default
    for suffix, a in EXCEPTIONS.items():
        if name.endswith(suffix):
            anchor = a
    if team == "red" and name.endswith("cover 0"):
        return "l"
    if team == "red" and name.endswith("block 2"):
        return "r"
    return anchor if team == "blue" else FLIP[anchor]

def marks(team):
    m = []
    spawn = [lonlat(s["frame"]) for s in d["spawns"] if s["team"] == team]
    lon = spawn[0][0]
    m.append((f"{team} spawn", lon, 0.0, "c", 12, "b"))
    for name, lon, lat in by_group["pylons"]:
        if name.startswith(team):
            m.append((f"{team} pylon", lon, lat, "c", 9, "b"))
    for name, lon, lat in by_group["city"]:
        if name.startswith(team):
            m.append((name, lon, lat, "c", 9, pick_anchor(name, team, "t")))
    for name, lon, lat in by_group["glass level"]:
        if name.startswith(team):
            m.append((name.replace("glass cover", "cover"), lon, lat, "c", 8, pick_anchor(name.replace("glass cover", "cover"), team, "b")))
    stairs = [(lon, lat) for name, lon, lat in by_group["stairs"] if name.startswith(team)]
    lons = [s[0] if team == "blue" else (s[0] + 360) % 360 for s in stairs]
    lats = [s[1] for s in stairs]
    mid_lon = (min(lons) + max(lons)) / 2
    m.append((f"{team} stairs", mid_lon, (min(lats) + max(lats)) / 2, "r", ((max(lons) - min(lons)) * 992 / 360 / 2 + 4, (max(lats) - min(lats)) * 496 / 180 / 2 + 5), pick_anchor("stairs", team, "b")))
    return m

lifts = [(name.replace("lift pad", "lift"), lon, lat, "c", 13, "t") for name, lon, lat in by_group["lifts"]]
def in_half(mark, team):
    lon = mark[1]
    is_blue = -90.5 <= ((lon + 180) % 360 - 180) <= 90.5
    return is_blue if team == "blue" else not is_blue or abs(abs((lon + 180) % 360 - 180) - 90) < 0.5

src = REPO / "demos/city_sphere_rust/assets/map_picture.png"
rolled = OUT / "_city_rolled.png"
OUT.mkdir(exist_ok=True)
ImageChops.offset(Image.open(src).convert("RGB"), 496, 0).save(rolled)
BLUE = marks("blue") + [l for l in lifts if in_half(l, "blue")]
RED_ZONE = marks("red") + [l for l in lifts if in_half(l, "red")]
annotate(src, 3, make_to_px(0), BLUE, "city_sphere_map_1_blue_zone.jpg", crop=(228, 0, 764, 496), font_px=24, line_px=4)
annotate(rolled, 3, make_to_px(496), RED_ZONE, "city_sphere_map_2_red_zone.jpg", crop=(228, 0, 764, 496), font_px=24, line_px=4)
rolled.unlink()
print([(m[0], round(m[1]), round(m[2])) for m in BLUE])
