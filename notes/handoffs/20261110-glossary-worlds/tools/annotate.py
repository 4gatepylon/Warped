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

def annotate(picture, scale_up, to_px, marks, out_name, crop=None, font_px=30, line_px=5):
    """marks: (label, len, wid, shape, size_px, anchor); shape 'c' circle (size=radius) or 'r' rect (size=(half_w, half_h)); anchor r/l/t/b."""
    img = Image.open(picture).convert("RGB")
    if scale_up != 1:
        img = img.resize((int(img.width * scale_up), int(img.height * scale_up)), Image.LANCZOS)
    draw = ImageDraw.Draw(img)
    font = ImageFont.truetype(FONT_PATH, font_px)
    for label, length, wid, shape, size, anchor in marks:
        x, y = to_px(length, wid)
        x, y = x * scale_up, y * scale_up
        if shape == "c":
            r = size * scale_up
            draw.ellipse([x - r, y - r, x + r, y + r], outline=RED, width=line_px)
            hw = hh = r
        else:
            hw, hh = size[0] * scale_up, size[1] * scale_up
            draw.rectangle([x - hw, y - hh, x + hw, y + hh], outline=RED, width=line_px)
        box = draw.textbbox((0, 0), label, font=font, stroke_width=3)
        tw, th = box[2] - box[0], box[3] - box[1]
        pad = 8
        extra = int(anchor[1:]) - 1 if len(anchor) > 1 else 0
        anchor = anchor[0]
        if anchor in "tb":
            pad += extra * (th + 10)
        else:
            pad += extra * 40
        if anchor == "r":
            tx, ty = x + hw + pad, y - th / 2
        elif anchor == "l":
            tx, ty = x - hw - pad - tw, y - th / 2
        elif anchor == "t":
            tx, ty = x - tw / 2, y - hh - pad - th
        else:
            tx, ty = x - tw / 2, y + hh + pad
        tx = min(max(tx, 4), img.width - tw - 4)
        ty = min(max(ty, 4), img.height - th - 8)
        draw.text((tx, ty - box[1]), label, font=font, fill=(255, 255, 255), stroke_width=3, stroke_fill=(0, 0, 0))
    if crop is not None:
        img = img.crop(tuple(int(c * scale_up) for c in crop))
    OUT.mkdir(exist_ok=True)
    out = OUT / out_name
    img.save(out, "JPEG", quality=85)
    print(out, img.size, out.stat().st_size // 1024, "KiB")
    return out
