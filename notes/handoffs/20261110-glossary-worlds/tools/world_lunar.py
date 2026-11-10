import sys, pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from annotate import annotate, load, fermi_pos, OUT, FONT_PATH
from zone_objects import collect
from PIL import Image, ImageDraw, ImageFont

R, EDGE, S = 40, 45, 10
W, H = 2 * 132 * S, 2 * 76 * S
def to_px(u, v):
    return W / 2 - S * u, H / 2 + S * v

d = load("lunar_surface")
singles, walls, warps, spawns, zone = collect("lunar_surface", R, EDGE)
base = Image.new("RGB", (W, H), (22, 22, 26))
draw = ImageDraw.Draw(base)
draw.ellipse([W / 2 - 128 * S, H / 2 - 72 * S, W / 2 + 128 * S, H / 2 + 72 * S], fill=(92, 92, 98))
for u, colour in [(EDGE, (90, 140, 255)), (-EDGE, (255, 110, 100))]:
    x = to_px(u, 0)[0]
    draw.line([x, 40, x, H - 40], fill=colour, width=4)
bx, by = to_px(0, 0)
draw.ellipse([bx - 24 * S, by - 24 * S, bx + 24 * S, by + 24 * S], outline=(60, 60, 66), width=3)
draw.ellipse([bx - 20, by - 20, bx + 20, by + 20], fill=(0, 0, 0))
SIZES = {"kazemat": 26, "hangar": 30, "dome": 22, "habitat": 24, "bunker": 20, "pad": 30, "watchtower": 12, "walkway": 22, "array": 16, "crates": 12, "mast": 8, "antenna": 8}
for s in d["structures"]:
    u, v = fermi_pos(s["frame"], R)
    x, y = to_px(u, v)
    name = s["name"]
    if name == "hesco block":
        draw.rectangle([x - 6, y - 6, x + 6, y + 6], fill=(170, 160, 140))
    elif "pylon" in name:
        colour = (90, 150, 255) if name.startswith("blue") else (255, 90, 80)
        draw.ellipse([x - 12, y - 12, x + 12, y + 12], fill=colour)
    else:
        half = next((size for key, size in SIZES.items() if key in name), 14)
        draw.rectangle([x - half, y - half, x + half, y + half], fill=(150, 150, 158), outline=(40, 40, 44), width=2)
for name, u, v in warps:
    if "mouth" in name:
        x, y = to_px(u, v)
        draw.ellipse([x - 18, y - 18, x + 18, y + 18], outline=(200, 230, 255), width=5)
for name, u, v in spawns:
    x, y = to_px(u, v)
    colour = (90, 150, 255) if name.startswith("blue") else (255, 90, 80)
    draw.polygon([(x, y - 14), (x + 12, y + 10), (x - 12, y + 10)], fill=colour)
base_path = OUT / "_lunar_base.png"
OUT.mkdir(exist_ok=True)
base.save(base_path)

def pick(name, z):
    return sorted([(u, v) for n, u, v in singles + warps + spawns if n == name and zone(u) == z], key=lambda t: (-t[0], t[1]))
def C(label, pos, r, anchor="r"):
    return (label, pos[0], pos[1], "c", r, anchor)
WALL_ANCHORS = {"blue": ["b", "t", "b", "r", "r", "r", "r"], "red": ["t", "b"]}
def wall_marks(team, order):
    ws = sorted([w for w in walls if zone(w[1]) == team], key=order)
    return [(f"{team} hesco wall {i + 1}", w[1], w[2], "r", (w[3] * S / 2 + 12, w[4] * S / 2 + 12), WALL_ANCHORS[team][i]) for i, w in enumerate(ws)]

bp = sorted(pick("blue pylon", "blue"), key=lambda t: t[1])
bc = pick("blue supply crates", "blue")
BLUE = [
    C("blue spawn", pick("blue spawn", "blue")[0], 20, "b"), C("blue bunker", pick("blue kazemat", "blue")[0], 34, "t"),
    C("blue relay mast", pick("blue relay mast", "blue")[0], 16, "b"), C("blue solar array", pick("blue solar array", "blue")[0], 24, "b"),
    C("blue left pylon", bp[0], 18, "t"), C("blue mid pylon", bp[1], 18, "t"), C("blue right pylon", bp[2], 18, "l"),
    C("blue crates 1", bc[0], 18, "l"), C("blue crates 2", bc[1], 18, "t"),
] + wall_marks("blue", lambda w: (-w[1], w[2]))
der = pick("derelict field bunker", "middle")
MID = [
    C("black hole", (0, 0), 30, "b"), C("ridge mouth", pick("wormhole mouth A", "middle")[0], 26, "b"),
    C("blue-side derelict bunker", der[0], 26, "b"), C("red-side derelict bunker", der[1], 26, "b"),
    C("mid relay mast", pick("relay mast", "middle")[0], 16, "b"), C("comms dome", pick("comms dome", "middle")[0], 28, "b"),
    C("crater walkway", pick("crater walkway", "middle")[0], 28, "t"), C("mid crates", pick("crates", "middle")[0], 18, "t"),
    C("mid solar array", pick("solar array", "middle")[0], 24, "t"),
]
rp = sorted(pick("red pylon", "red"), key=lambda t: -t[1])
RED_ZONE = [
    C("red spawn", pick("red spawn", "red")[0], 20, "b"), C("red hangar", pick("red spawn hangar", "red")[0], 38, "t"),
    C("red left pylon", rp[0], 18, "b"), C("red mid pylon", rp[1], 18, "t"), C("red right pylon", rp[2], 18, "t"),
    C("red habitat", pick("red habitat module", "red")[0], 30, "b"), C("red landing pad, red pad mouth", pick("red landing pad", "red")[0], 38, "b"),
    C("red solar array", pick("red solar array", "red")[0], 24, "t"), C("red crates", pick("red crates", "red")[0], 18, "b"),
    C("red watchtower", pick("red watchtower", "red")[0], 18, "t"), C("red dome", pick("red dome", "red")[0], 28, "b"),
    C("red antenna", pick("red antenna", "red")[0], 16, "b"), C("red walkway", pick("red walkway", "red")[0], 28, "t"),
] + wall_marks("red", lambda w: (w[1], w[2]))
annotate(base_path, 1, to_px, BLUE, "lunar_surface_map_1_blue_zone.jpg", crop=(0, 0, 1180, H), font_px=26, line_px=4, bottom_margin_px=44)
annotate(base_path, 1, to_px, MID, "lunar_surface_map_2_middle_zone.jpg", crop=(820, 0, 1820, H), font_px=26, line_px=4, bottom_margin_px=44)
annotate(base_path, 1, to_px, RED_ZONE, "lunar_surface_map_3_red_zone.jpg", crop=(1740, 0, W, H), font_px=26, line_px=4, bottom_margin_px=44)
base_path.unlink()
caption_font = ImageFont.truetype(FONT_PATH, 22)
for out_name in ("lunar_surface_map_1_blue_zone.jpg", "lunar_surface_map_2_middle_zone.jpg", "lunar_surface_map_3_red_zone.jpg"):
    pic = Image.open(OUT / out_name)
    ImageDraw.Draw(pic).text((10, H - 34), "Drawn from the level layout, not a game screenshot", font=caption_font, fill=(190, 190, 190))
    pic.save(OUT / out_name, "JPEG", quality=85)
for nm, ms in [("BLUE", BLUE), ("MID", MID), ("RED", RED_ZONE)]:
    print(nm, [(m[0], round(m[1]), round(m[2])) for m in ms])
