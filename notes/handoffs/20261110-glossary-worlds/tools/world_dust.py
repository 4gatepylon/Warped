import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from annotate import annotate, REPO
from zone_objects import collect

singles, walls, warps, spawns, zone = collect("dust", 50, 48)
def to_px(u, v):
    return 496 - 3.75 * u, 256 + 3.75 * v

def pick(name, z):
    return sorted([(u, v) for n, u, v in singles + warps + spawns if n == name and zone(u) == z], key=lambda t: (-t[0], t[1]))
def wall_rects(z, order):
    ws = sorted([w for w in walls if zone(w[1]) == z], key=order)
    return [(f"{z} hesco wall {i + 1}", w[1], w[2], "r", (max(w[3] * 3.75 / 2, 4) + 5, max(w[4] * 3.75 / 2, 4) + 6)) for i, w in enumerate(ws)]

def C(label, pos, r, anchor="r"):
    return (label, pos[0], pos[1], "c", r, anchor)

def team_marks(team, sign):
    z = team
    by_left = sorted(pick(f"{team} pylon", z), key=lambda t: sign * t[1])
    m = [
        C(f"{team} spawn", pick(f"{team} spawn", z)[0], 9, "t" if team == "blue" else "b"),
        C(f"{team} bunker", pick(f"{team} kazemat", z)[0], 13, "l" if team == "blue" else "r"),
        C(f"{team} left pylon", by_left[0], 8, "r" if team == "blue" else "l"),
        C(f"{team} mid pylon", by_left[1], 8, "b" if team == "blue" else "t"),
        C(f"{team} right pylon", by_left[2], 8, "r" if team == "blue" else "l"),
        C(f"{team} boulder", pick("boulder", z)[0], 6, "t"),
        C(f"{team} pillar", pick("sandstone pillar", z)[0], 7, "t"),
        C(f"{team} mesa", pick("sandstone mesa", z)[0], 14, "t" if team == "blue" else "b"),
        C(f"{team} mouth", pick("wormhole mouth A" if team == "blue" else "wormhole mouth B", z)[0], 8, "b" if team == "blue" else "t"),
    ]
    for label, u, v, shape, size in wall_rects(z, (lambda w: -w[1]) if team == "blue" else (lambda w: w[1])):
        m.append((label, u, v, shape, size, "b"))
    return m

BLUE = team_marks("blue", 1)
RED_ZONE = team_marks("red", -1)
MID = []
for kind, label, r in [("ruin wall", "ruin wall", 7), ("boulder", "boulder", 6), ("sandstone pillar", "pillar", 7)]:
    for i, pos in enumerate(pick(kind, "middle")):
        MID.append(C(f"{label} {i + 1}", pos, r, "b"))
mesas = pick("sandstone mesa", "middle")
MID += [C("blue-side mesa", mesas[0], 14, "t"), C("red-side mesa", mesas[1], 14, "b")]
arches = sorted(pick("sandstone arch", "middle"), key=lambda t: t[1])
MID += [C("top arch", arches[0], 9, "t"), C("bottom arch", arches[1], 9, "b")]
MID += [C("blue-side field bunker", pick("blue field bunker", "middle")[0], 12, "t"), C("red-side field bunker", pick("red field bunker", "middle")[0], 12, "b")]

pic = REPO / "demos/dust_rust/map_pictures/dust_r50m.png"
annotate(pic, 3, to_px, BLUE, "dust_map_1_blue_zone.jpg", crop=(0, 0, 330, 512), font_px=26, line_px=4)
annotate(pic, 3, to_px, MID, "dust_map_2_middle_zone.jpg", crop=(290, 0, 702, 512), font_px=26, line_px=4)
annotate(pic, 3, to_px, RED_ZONE, "dust_map_3_red_zone.jpg", crop=(662, 0, 992, 512), font_px=26, line_px=4)
for name, marks in [("BLUE", BLUE), ("MID", MID), ("RED", RED_ZONE)]:
    print(name, [(m[0], round(m[1]), round(m[2])) for m in marks])
