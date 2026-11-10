import sys, pathlib, collections
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from annotate import annotate, load, flat_pos, REPO

d = load("dark_forest")
def to_px(length, wid):
    return 420 + 10 * wid, 680 + 10 * length

groups = collections.defaultdict(list)
for b in d["boxes"]:
    L, W = flat_pos(b["frame"])
    groups[b["group"]].append((L, W, b["half_size_m"][2], b["half_size_m"][0]))
def rect(name):
    ps = groups[name]
    lo_l = min(p[0] - p[2] for p in ps); hi_l = max(p[0] + p[2] for p in ps)
    lo_w = min(p[1] - p[3] for p in ps); hi_w = max(p[1] + p[3] for p in ps)
    return (lo_l + hi_l) / 2, (lo_w + hi_w) / 2, ((hi_w - lo_w) * 5 + 6, (hi_l - lo_l) * 5 + 6)
def R(label, group, anchor):
    L, W, size = rect(group)
    return (label, L, W, "r", size, anchor)
pyl = {s["name"]: flat_pos(s["frame"]) for s in d["structures"] if "pylon" in s["name"]}
lad = {l["name"]: flat_pos(l["frame"]) for l in d["ladders"]}
warp = {w["name"]: flat_pos(w["frame"]) for w in d["warps"]}
def C(label, pos, r, anchor):
    return (label, pos[0], pos[1], "c", r, anchor)

BLUE = [
    C("blue spawn", (-53, 0), 26, "t"),
    R("blue palisade", "blue palisade", "r"),
    C("blue left pylon", pyl["blue pylon 0"], 14, "b"),
    R("blue treehouse", "blue treehouse", "t"),
    C("blue right pylon", pyl["blue pylon 1"], 14, "l"),
    C("blue treehouse ladder", lad["blue treehouse ladder"], 10, "r"),
    R("blue home bridge", "blue home bridge", "l"),
    C("blue rock 3", groups["blue rock 3"][0][:2], 14, "t"),
    C("blue rock 2", groups["blue rock 2"][0][:2], 14, "r"),
    C("blue home gate, mouth A", warp["gate 4 mouth 0"], 22, "r"),
    C("blue home gate, mouth B", warp["gate 4 mouth 1"], 22, "b"),
    C("mid gate, blue mouth", warp["gate 2 mouth 1"], 34, "r"),
]
MID = [
    R("blue middle deck", "blue middle deck", "l"),
    C("blue middle ladder", lad["blue middle ladder"], 10, "r"),
    C("blue rock 0", groups["blue rock 0"][0][:2], 14, "r"),
    C("cross gate B, blue-side mouth", warp["gate 1 mouth 0"], 34, "t"),
    R("blue flank bridge", "blue flank bridge", "r"),
    C("blue rock 1", groups["blue rock 1"][0][:2], 14, "t"),
    C("cross gate A, blue-side mouth", warp["gate 0 mouth 1"], 34, "t"),
    R("blue lookout", "blue lookout", "t"),
    R("blue centre bridge", "blue centre bridge", "b"),
    R("great tree deck", "the great tree deck", "t2"),
    R("red centre bridge", "red centre bridge", "t"),
    R("blue flank deck", "blue flank deck", "l"),
    R("red flank deck", "red flank deck", "r"),
    R("red lookout", "red lookout", "b"),
    R("red flank bridge", "red flank bridge", "l"),
    C("cross gate B, red-side mouth", warp["gate 1 mouth 1"], 34, "b"),
    C("red rock 1", groups["red rock 1"][0][:2], 14, "b"),
    C("cross gate A, red-side mouth", warp["gate 0 mouth 0"], 34, "b"),
    C("red rock 0", groups["red rock 0"][0][:2], 14, "l"),
    R("red middle deck", "red middle deck", "r"),
    C("red middle ladder", lad["red middle ladder"], 10, "l"),
]
RED_ZONE = [
    C("mid gate, red mouth", warp["gate 2 mouth 0"], 34, "l"),
    R("red home bridge", "red home bridge", "r"),
    C("red rock 2", groups["red rock 2"][0][:2], 14, "l"),
    C("red rock 3", groups["red rock 3"][0][:2], 14, "b"),
    C("red home gate, mouth A", warp["gate 3 mouth 0"], 22, "l"),
    C("red home gate, mouth B", warp["gate 3 mouth 1"], 22, "t"),
    R("red treehouse", "red treehouse", "b"),
    C("red right pylon", pyl["red pylon 1"], 14, "r"),
    R("red palisade", "red palisade", "l"),
    C("red left pylon", pyl["red pylon 0"], 14, "t"),
    C("red treehouse ladder", lad["red treehouse ladder"], 10, "l"),
    C("red spawn", (53, 0), 26, "b"),
]
pic = REPO / "demos/dark_forest_rust/assets/map_picture.png"
annotate(pic, 2, to_px, BLUE, "dark_forest_map_1_blue_zone.jpg", crop=(0, 80, 840, 480), font_px=30)
annotate(pic, 2, to_px, MID, "dark_forest_map_2_middle_zone.jpg", crop=(0, 400, 840, 960), font_px=30)
annotate(pic, 2, to_px, RED_ZONE, "dark_forest_map_3_red_zone.jpg", crop=(0, 880, 840, 1280), font_px=30)
