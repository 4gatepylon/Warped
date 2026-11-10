"""Scratch tool: draw red marks and names on a level's map picture."""
import json, math, pathlib
from PIL import Image, ImageDraw, ImageFont

FONT_CANDIDATES = ["/System/Library/Fonts/Supplemental/Arial Bold.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf"]
FONT_PATH = next(f for f in FONT_CANDIDATES if pathlib.Path(f).exists())
RED = (255, 35, 35)
REPO = pathlib.Path(__file__).resolve().parents[4]
OUT = pathlib.Path(__file__).resolve().parent.parent / "pictures"

def load(level):
    return json.loads((REPO / f"common/maps/{level}/map.json").read_text())

def flat_pos(frame):
    return frame[14], frame[12]

def fermi_pos(frame, radius_m):
    x, _, z, w = frame[12:16]
    return radius_m * math.atanh(z / w), radius_m * math.asinh(x)

def _overlap(a, b):
    w = min(a[2], b[2]) - max(a[0], b[0])
    h = min(a[3], b[3]) - max(a[1], b[1])
    return w * h if w > 0 and h > 0 else 0

def _label_box(anchor, extra, x, y, hw, hh, tw, th):
    pad = 8 + extra * (th + 10 if anchor in "tb" else 40)
    if anchor == "r":
        return x + hw + pad, y - th / 2
    if anchor == "l":
        return x - hw - pad - tw, y - th / 2
    if anchor == "t":
        return x - tw / 2, y - hh - pad - th
    if anchor == "b":
        return x - tw / 2, y + hh + pad
    dx = hw * 0.7 + 6 + extra * 30
    dy = hh * 0.7 + 6 + extra * (th + 6)
    return (x + dx if anchor[1] == "r" else x - dx - tw), (y - dy - th if anchor[0] == "t" else y + dy)

def annotate(picture, scale_up, to_px, marks, out_name, crop=None, font_px=30, line_px=5, bottom_margin_px=8):
    """marks: (label, len, wid, shape, size_px, anchor); shape 'c' circle (size=radius) or 'r' rect (size=(half_w, half_h)); anchor r/l/t/b/tr/tl/br/bl[N] is the preferred side, and a trailing "!" pins the label there.

    Each label goes on the first side, in preference order, where it stays inside the crop and off the labels placed
    before it and off every other mark; a label pushed away from its mark gets a thin line back to it.
    """
    img = Image.open(picture).convert("RGB")
    if scale_up != 1:
        img = img.resize((int(img.width * scale_up), int(img.height * scale_up)), Image.LANCZOS)
    draw = ImageDraw.Draw(img)
    font = ImageFont.truetype(FONT_PATH, font_px)
    bounds = tuple(c * scale_up for c in crop) if crop is not None else (0, 0, img.width, img.height)
    shapes = []
    for label, length, wid, shape, size, anchor in marks:
        x, y = to_px(length, wid)
        x, y = x * scale_up, y * scale_up
        hw, hh = (size * scale_up, size * scale_up) if shape == "c" else (size[0] * scale_up, size[1] * scale_up)
        shapes.append((x, y, hw, hh))
        if shape == "c":
            draw.ellipse([x - hw, y - hh, x + hw, y + hh], outline=RED, width=line_px)
        else:
            draw.rectangle([x - hw, y - hh, x + hw, y + hh], outline=RED, width=line_px)
    placed = []
    texts = []
    for i in sorted(range(len(marks)), key=lambda i: not marks[i][5].endswith("!")):
        label, anchor = marks[i][0], marks[i][5]
        is_pinned = anchor.endswith("!")
        anchor = anchor.rstrip("!")
        x, y, hw, hh = shapes[i]
        box = draw.textbbox((0, 0), label, font=font, stroke_width=3)
        tw, th = box[2] - box[0], box[3] - box[1]
        side = anchor.rstrip("0123456789")
        first = (side, int(anchor[len(side):]) - 1 if len(anchor) > len(side) else 0)
        candidates = [first] if is_pinned else [first] + [(a, e) for e in range(3) for a in ("r", "l", "t", "b", "tr", "tl", "br", "bl")]
        best = None
        for rank, (a, e) in enumerate(candidates):
            tx, ty = _label_box(a, e, x, y, hw, hh, tw, th)
            rect = (tx - 3, ty - 3, tx + tw + 3, ty + th + 3)
            outside = max(0, bounds[0] + 4 - rect[0]) + max(0, rect[2] - bounds[2] + 4) + max(0, bounds[1] + 4 - rect[1]) + max(0, rect[3] - bounds[3] + bottom_margin_px)
            cost = outside * 1000 + sum(_overlap(rect, p) for p in placed) * 10
            cost += sum(_overlap(rect, (sx - shw, sy - shh, sx + shw, sy + shh)) for j, (sx, sy, shw, shh) in enumerate(shapes) if j != i) * 3
            if best is None or cost < best[0]:
                best = (cost, tx, ty, rect, a, e)
            if cost == 0:
                break
        _, tx, ty, rect, a, e = best
        placed.append(rect)
        if e > 0 or len(a) == 2:
            lx = min(max(x, rect[0]), rect[2])
            ly = min(max(y, rect[1]), rect[3])
            dist = math.hypot(lx - x, ly - y)
            if dist > max(hw, hh) + 4:
                k = min(hw, hh) / dist
                draw.line([x + (lx - x) * k, y + (ly - y) * k, lx, ly], fill=RED, width=max(2, line_px - 2))
        texts.append((tx, ty - box[1], label))
    for tx, ty, label in texts:
        draw.text((tx, ty), label, font=font, fill=(255, 255, 255), stroke_width=3, stroke_fill=(0, 0, 0))
    if crop is not None:
        img = img.crop(tuple(int(c * scale_up) for c in crop))
    OUT.mkdir(exist_ok=True)
    out = OUT / out_name
    img.save(out, "JPEG", quality=85)
    print(out, img.size, out.stat().st_size // 1024, "KiB")
    return out
