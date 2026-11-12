"""`make_world_doc.py` writes each level's world doc and its map pictures from the level's source of truth.

A level folder `<level>/` holds `source/`, written by people and agents, and `generated/`, written only by this
script. From `source/map.png` (the game's map picture without labels), `source/world.yaml` (zones and objects, see
`world_doc_schema.py`) and `source/template.md` (prose with slots), it writes to `generated/`:

- `<level>.md`: the template with its slots filled.
- `map.png`: a copy of the map picture.
- `map_objects.png`: the map with every object outlined and named.
- `map_zones.png`: the map with the root's zones outlined and named.
- `zone_<zone>.png`, one per zone: the whole map, blurred and darkened outside the zone, with the zone's sub-zones
  and its own objects outlined and named.

The template's slots are `{{map}}`, `{{map_objects}}`, `{{map_zones}}`, `{{world_tree}}` and `{{zone_sections}}`;
each may appear at most once. `{{zone_sections}}` gives every zone below the root a section, breadth first, headed
by its path from the root (`## blue zone > blue top`).

Interface:
    LEVEL_FOLDERS: one or more level folders, each with a `source/` folder.
    --check: write nothing; list every file in `generated/` that differs from what the script would write, and exit
        1 if any does.
    --dry-run: write nothing; list the files the script would write and remove.
    --json: print the results as one JSON object.

Example:
    $ uv run python make_world_doc.py lunar_surface
    $ uv run python make_world_doc.py --check lunar_surface

Must know:
    Side effects:
        - A run without `--check` or `--dry-run` replaces `generated/` and removes any file in it that it didn't write.
    Gotchas:
        - `--check` compares pictures pixel by pixel, not byte by byte, so PNG compression may differ by machine.
"""

import json
import re
from functools import cache
from pathlib import Path

import click
from PIL import Image, ImageDraw, ImageEnhance, ImageFilter, ImageFont
from pydantic import ValidationError

from world_doc_schema import Shape, World, bounds_px_t, point_px_t

SOURCE_FOLDER_NAME: str = "source"
GENERATED_FOLDER_NAME: str = "generated"
MAP_PICTURE_NAME: str = "map.png"
WORLD_NAME: str = "world.yaml"
TEMPLATE_NAME: str = "template.md"
OBJECTS_PICTURE_NAME: str = "map_objects.png"
ZONES_PICTURE_NAME: str = "map_zones.png"
GENERATED_DOC_NOTE: str = "<!-- game_design/worlds/make_world_doc.py writes this file from ../source/. Edit those files, not this one. -->\n\n"
SLOT_PATTERN: re.Pattern = re.compile(r"\{\{\s*([a-z_]+)\s*\}\}")
SLOTS: tuple[str, ...] = ("map", "map_objects", "map_zones", "world_tree", "zone_sections")
SUPERSAMPLE: int = 2
OUTSIDE_ZONE_BLUR_PX: float = 8
OUTSIDE_ZONE_DIM_FRAC: float = 0.45
OBJECT_COLOR: tuple[int, int, int] = (255, 64, 64)
ZONE_COLOR: tuple[int, int, int] = (255, 255, 255)
SUB_ZONE_COLOR: tuple[int, int, int] = (255, 214, 0)
TEXT_COLOR: tuple[int, int, int] = (255, 255, 255)
TEXT_STROKE_COLOR: tuple[int, int, int] = (0, 0, 0)
OBJECT_LINE_WIDTH_PX: int = 2
ZONE_LINE_WIDTH_PX: int = 3
LEADER_LINE_WIDTH_PX: int = 1
TEXT_STROKE_WIDTH_PX: int = 3
OBJECT_FONT_SIZE_PX: int = 22
ZONE_FONT_SIZE_PX: int = 34
LABEL_GAP_PX: float = 3
LABEL_MARGIN_PX: float = 2
LABEL_SEARCH_STEP_PX: float = 10
LABEL_SEARCH_MAX_PX: float = 400
LABEL_SEARCH_DIRECTIONS: tuple[tuple[float, float], ...] = (
    (1, 0), (0.92, -0.38), (0.71, -0.71), (0.38, -0.92), (0, -1), (-0.38, -0.92), (-0.71, -0.71), (-0.92, -0.38),
    (-1, 0), (-0.92, 0.38), (-0.71, 0.71), (-0.38, 0.92), (0, 1), (0.38, 0.92), (0.71, 0.71), (0.92, 0.38),
)  # fmt: skip
TREE_BRANCH: str = "├── "
TREE_LAST_BRANCH: str = "└── "
TREE_PIPE: str = "│   "
TREE_SPACE: str = "    "


@cache
def _get_font(size_px: int) -> ImageFont.FreeTypeFont:
    """`_get_font` gives Pillow's own font at a size, so pictures don't depend on the machine's fonts.

    Args:
        size_px: The font's size, in pixels.

    Example:
        >>> _get_font(30)
    """
    return ImageFont.load_default(size=size_px)


def _boxes_overlap(a: bounds_px_t, b: bounds_px_t) -> bool:
    """`_boxes_overlap` says whether two boxes, each (min x, min y, max x, max y), overlap.

    Args:
        a: One box.
        b: The other box.

    Example:
        >>> _boxes_overlap((0, 0, 2, 2), (1, 1, 3, 3))
        True
    """
    return a[0] < b[2] and b[0] < a[2] and a[1] < b[3] and b[1] < a[3]


def _clamp_to_box(point_px: point_px_t, box: bounds_px_t) -> point_px_t:
    """`_clamp_to_box` gives the point of a box nearest to a point.

    Args:
        point_px: The point.
        box: The box, (min x, min y, max x, max y).

    Example:
        >>> _clamp_to_box((5, 5), (0, 0, 2, 2))
        (2, 2)
    """
    return (min(max(point_px[0], box[0]), box[2]), min(max(point_px[1], box[1]), box[3]))


class _LabelPlacer:
    """`_LabelPlacer` finds a place for each label where it overlaps no mark and no label placed before it.

    Args:
        picture_size_px: The picture's (width, height).
        obstacles: Boxes no label may cover, such as the objects' marks.

    Example:
        >>> placer = _LabelPlacer((1862, 1072), [(10, 10, 20, 20)])
        >>> box, is_beside = placer.placeLabel((60, 14), (10, 10, 20, 20), False)
    """

    def __init__(self, picture_size_px: tuple[int, int], obstacles: list[bounds_px_t]) -> None:
        self.picture_size_px_: tuple[int, int] = picture_size_px
        self.occupied_: list[bounds_px_t] = list(obstacles)

    def _is_free(self, box: bounds_px_t) -> bool:
        """`_is_free` says whether a label box lies inside the picture and clear of everything placed so far.

        Args:
            box: The label's box.

        Example:
            >>> placer._is_free((0, 0, 10, 10))
        """
        width_px, height_px = self.picture_size_px_
        if box[0] < LABEL_MARGIN_PX or box[1] < LABEL_MARGIN_PX:
            return False
        if box[2] > width_px - LABEL_MARGIN_PX or box[3] > height_px - LABEL_MARGIN_PX:
            return False
        padded: bounds_px_t = (
            box[0] - LABEL_GAP_PX,
            box[1] - LABEL_GAP_PX,
            box[2] + LABEL_GAP_PX,
            box[3] + LABEL_GAP_PX,
        )
        return not any(_boxes_overlap(padded, other) for other in self.occupied_)

    def placeLabel(
        self, label_size_px: tuple[float, float], anchor: bounds_px_t, should_centre: bool
    ) -> tuple[bounds_px_t, bool]:
        """`placeLabel` places one label for the mark whose box is `anchor`, and keeps its place free from now on.

        It tries the spots right beside the mark, then spots ever farther from the mark's centre. With
        `should_centre`, as for a zone's name, it tries the mark's centre first instead of the spots beside it, and
        the label never needs a line.

        Args:
            label_size_px: The label's (width, height).
            anchor: The box of the mark the label names.
            should_centre: Whether to start from the middle of the mark, as for a zone's name.

        Returns:
            The label's box and whether it sits beside its mark; one that doesn't needs a line to it.

        Example:
            >>> box, is_beside = placer.placeLabel((60, 14), (10, 10, 20, 20), False)
        """
        w, h = label_size_px
        x0, y0, x1, y1 = anchor
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        g: float = LABEL_GAP_PX
        beside: list[point_px_t] = [
            (x1 + g, cy - h / 2), (x0 - g - w, cy - h / 2), (cx - w / 2, y0 - g - h), (cx - w / 2, y1 + g),
            (x1 + g, y0 - g - h), (x0 - g - w, y0 - g - h), (x1 + g, y1 + g), (x0 - g - w, y1 + g),
        ]  # fmt: skip
        if should_centre:
            beside = [(cx - w / 2, cy - h / 2)]
        far: list[point_px_t] = []
        distance_px: float = LABEL_SEARCH_STEP_PX
        while distance_px <= LABEL_SEARCH_MAX_PX:
            far.extend(
                (cx + dx * distance_px - w / 2, cy + dy * distance_px - h / 2) for dx, dy in LABEL_SEARCH_DIRECTIONS
            )
            distance_px += LABEL_SEARCH_STEP_PX
        for index, (left, top) in enumerate(beside + far):
            box: bounds_px_t = (left, top, left + w, top + h)
            if self._is_free(box):
                self.occupied_.append(box)
                return box, should_centre or index < len(beside)
        raise ValueError(f"no free place within {LABEL_SEARCH_MAX_PX} px for a {w:.0f}x{h:.0f} label at {anchor}")


class _Overlay:
    """`_Overlay` draws outlines, lines and labels at `SUPERSAMPLE` times a picture's size, for smooth edges.

    Args:
        picture_size_px: The map picture's (width, height); coordinates passed in are in its pixels.

    Example:
        >>> overlay = _Overlay((1862, 1072))
        >>> overlay.drawShape(shape, (255, 0, 0), 2)
        >>> picture = overlay.compositeOnto(picture)
    """

    def __init__(self, picture_size_px: tuple[int, int]) -> None:
        self.picture_size_px: tuple[int, int] = picture_size_px
        width_px, height_px = picture_size_px
        self.image_: Image.Image = Image.new("RGBA", (width_px * SUPERSAMPLE, height_px * SUPERSAMPLE), (0, 0, 0, 0))
        self.draw_: ImageDraw.ImageDraw = ImageDraw.Draw(self.image_)

    def drawShape(self, shape: Shape, color: tuple[int, int, int], width_px: int) -> None:
        """`drawShape` outlines a shape.

        Args:
            shape: The shape.
            color: The outline's colour.
            width_px: The outline's width.

        Example:
            >>> overlay.drawShape(shape, (255, 0, 0), 2)
        """
        shape.draw(self.draw_, SUPERSAMPLE, None, color, width_px * SUPERSAMPLE)

    def drawLine(self, start_px: point_px_t, end_px: point_px_t, color: tuple[int, int, int], width_px: int) -> None:
        """`drawLine` draws a straight line.

        Args:
            start_px: One end.
            end_px: The other end.
            color: The line's colour.
            width_px: The line's width.

        Example:
            >>> overlay.drawLine((0, 0), (10, 10), (255, 0, 0), 1)
        """
        points: list[float] = [v * SUPERSAMPLE for v in (*start_px, *end_px)]
        self.draw_.line(points, fill=color, width=width_px * SUPERSAMPLE)

    def measureText(self, text: str, font_size_px: int) -> tuple[float, float]:
        """`measureText` gives the (width, height) a label takes, outline included.

        Args:
            text: The label.
            font_size_px: The font's size.

        Example:
            >>> overlay.measureText("blue spawn", 15)
        """
        left, top, right, bottom = self.draw_.textbbox(
            (0, 0), text, font=_get_font(font_size_px * SUPERSAMPLE), stroke_width=TEXT_STROKE_WIDTH_PX * SUPERSAMPLE
        )
        return ((right - left) / SUPERSAMPLE, (bottom - top) / SUPERSAMPLE)

    def drawText(self, text: str, top_left_px: point_px_t, font_size_px: int) -> None:
        """`drawText` writes a label, white with a dark outline, with its box's top left corner at `top_left_px`.

        Args:
            text: The label.
            top_left_px: Where its box starts.
            font_size_px: The font's size.

        Example:
            >>> overlay.drawText("blue spawn", (100, 40), 15)
        """
        font: ImageFont.FreeTypeFont = _get_font(font_size_px * SUPERSAMPLE)
        stroke_width: int = TEXT_STROKE_WIDTH_PX * SUPERSAMPLE
        left, top, _, _ = self.draw_.textbbox((0, 0), text, font=font, stroke_width=stroke_width)
        xy: tuple[float, float] = (top_left_px[0] * SUPERSAMPLE - left, top_left_px[1] * SUPERSAMPLE - top)
        self.draw_.text(xy, text, font=font, fill=TEXT_COLOR, stroke_width=stroke_width, stroke_fill=TEXT_STROKE_COLOR)

    def labelShapes(
        self,
        named_shapes: list[tuple[str, Shape]],
        placer: _LabelPlacer,
        font_size_px: int,
        line_color: tuple[int, int, int],
        should_centre: bool,
    ) -> None:
        """`labelShapes` names each shape, joining a name that had to sit away from its shape to it with a line.

        Args:
            named_shapes: (name, shape) pairs, in the order to place their labels.
            placer: What finds free places for the labels.
            font_size_px: The labels' font size.
            line_color: The colour of the lines from labels to shapes.
            should_centre: Whether to try each label in the middle of its shape first.

        Example:
            >>> overlay.labelShapes([("blue spawn", shape)], placer, 15, (255, 0, 0), False)
        """
        labels: list[tuple[str, bounds_px_t]] = []
        for name, shape in named_shapes:
            anchor: bounds_px_t = shape.getBoundsPx()
            box, is_beside = placer.placeLabel(self.measureText(name, font_size_px), anchor, should_centre)
            if not is_beside:
                label_end_px: point_px_t = _clamp_to_box(shape.getCentrePx(), box)
                self.drawLine(_clamp_to_box(label_end_px, anchor), label_end_px, line_color, LEADER_LINE_WIDTH_PX)
            labels.append((name, box))
        for name, box in labels:
            self.drawText(name, box[:2], font_size_px)

    def compositeOnto(self, picture: Image.Image) -> Image.Image:
        """`compositeOnto` shrinks the overlay to the picture's size and lays it over the picture.

        Args:
            picture: The picture, in RGB.

        Returns:
            A new RGB picture.

        Example:
            >>> picture = overlay.compositeOnto(picture)
        """
        shrunk: Image.Image = self.image_.convert("RGBa").resize(self.picture_size_px, Image.Resampling.LANCZOS)
        return Image.alpha_composite(picture.convert("RGBA"), shrunk.convert("RGBA")).convert("RGB")


def _get_slug(name: str) -> str:
    """`_get_slug` turns a zone's name into a file name part: lowercase, with runs of other characters as `_`.

    Args:
        name: The zone's name.

    Example:
        >>> _get_slug("blue zone")
        'blue_zone'
    """
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")


def _get_heading_anchor(heading: str) -> str:
    """`_get_heading_anchor` gives the link anchor GitHub makes for a Markdown heading.

    Args:
        heading: The heading's text.

    Example:
        >>> _get_heading_anchor("blue zone > blue top")
        'blue-zone--blue-top'
    """
    return re.sub(r"[^a-z0-9 _-]", "", heading.lower()).replace(" ", "-")


def _escape_cell(text: str) -> str:
    """`_escape_cell` makes text safe for a Markdown table cell.

    Args:
        text: The text.

    Example:
        >>> _escape_cell("a | b")
        'a \\\\| b'
    """
    return text.replace("|", "\\|").replace("\n", " ")


def render_world_tree(world: World) -> str:
    """`render_world_tree` draws the world's zones and objects as a text tree, each zone's zones before its objects.

    Args:
        world: The world.

    Returns:
        The tree, in a fenced `text` block.

    Example:
        >>> print(render_world_tree(world))
    """
    lines: list[str] = [world.level]

    def add_children(zone_name: str, prefix: str) -> None:
        """`add_children` adds one line per child of a zone, and its children's lines under each child zone.

        Args:
            zone_name: The zone.
            prefix: What goes before this level's branches.

        Example:
            >>> add_children(world.root, "")
        """
        zone = world.zones[zone_name]
        children: list[tuple[str, bool]] = [(name, True) for name in zone.zones] + [
            (name, False) for name in zone.objects
        ]
        for index, (name, is_zone) in enumerate(children):
            is_last: bool = index == len(children) - 1
            lines.append(prefix + (TREE_LAST_BRANCH if is_last else TREE_BRANCH) + name)
            if is_zone:
                add_children(name, prefix + (TREE_SPACE if is_last else TREE_PIPE))

    add_children(world.root, "")
    return "```text\n" + "\n".join(lines) + "\n```"


def render_zone_sections(world: World) -> str:
    """`render_zone_sections` writes one section per zone below the root, breadth first.

    Each section has the zone's path from the root as its heading, its description, its picture, a table of its
    sub-zones (when it has any) and a table of its own objects (when it has any).

    Args:
        world: The world.

    Returns:
        The sections, as Markdown.

    Example:
        >>> print(render_zone_sections(world))
    """
    sections: list[str] = []
    for zone_name in world.getZonesBreadthFirst():
        zone = world.zones[zone_name]
        path: list[str] = world.getZonePath(zone_name)
        caption: str = (
            f"{world.level}'s {zone_name}: the whole map, blurred and darkened outside the zone, with the zone's"
            " sub-zones and objects outlined and named"
        )
        heading: str = " > ".join(path)
        parts: list[str] = [
            f"## {heading[0].upper()}{heading[1:]}",
            zone.description,
            f"![{caption}](zone_{_get_slug(zone_name)}.png)",
        ]
        if zone.zones:
            rows: list[str] = ["| Zone | Where it is |", "| --- | --- |"]
            for child in zone.zones:
                anchor: str = _get_heading_anchor(" > ".join([*path, child]))
                rows.append(f"| [**{child}**](#{anchor}) | {_escape_cell(world.zones[child].description)} |")
            parts.append("\n".join(rows))
        if zone.objects:
            rows = ["| Name | What it is | Name in the level layout |", "| --- | --- | --- |"]
            for child in zone.objects:
                thing = world.objects[child]
                rows.append(f"| **{child}** | {_escape_cell(thing.description)} | {_escape_cell(thing.layout_name)} |")
            parts.append("\n".join(rows))
        sections.append("\n\n".join(parts))
    return "\n\n".join(sections)


def hydrate_template(template: str, slot_texts: dict[str, str]) -> str:
    """`hydrate_template` fills each `{{slot}}` in a template with its text.

    Args:
        template: The template.
        slot_texts: The text for each slot in `SLOTS`.

    Returns:
        The filled template.

    Raises:
        ValueError: The template has an unknown slot or a slot more than once.

    Example:
        >>> hydrate_template("# Dust\\n\\n{{world_tree}}\\n", {"world_tree": "..."})
    """
    names: list[str] = SLOT_PATTERN.findall(template)
    unknown: list[str] = sorted(set(names) - set(SLOTS))
    if unknown:
        raise ValueError(f"template: unknown slots {unknown}; the slots are {list(SLOTS)}")
    repeated: list[str] = sorted({name for name in names if names.count(name) > 1})
    if repeated:
        raise ValueError(f"template: slots {repeated} appear more than once; each may appear at most once")
    return SLOT_PATTERN.sub(lambda match: slot_texts[match.group(1)], template)


def _render_objects_picture(world: World, picture: Image.Image) -> Image.Image:
    """`_render_objects_picture` outlines and names every object on the map picture.

    Args:
        world: The world.
        picture: The map picture, in RGB.

    Example:
        >>> _render_objects_picture(world, picture).save("map_objects.png")
    """
    overlay: _Overlay = _Overlay(picture.size)
    named_shapes: list[tuple[str, Shape]] = [(name, thing.shape) for name, thing in world.objects.items()]
    for _, shape in named_shapes:
        overlay.drawShape(shape, OBJECT_COLOR, OBJECT_LINE_WIDTH_PX)
    placer: _LabelPlacer = _LabelPlacer(picture.size, [shape.getBoundsPx() for _, shape in named_shapes])
    overlay.labelShapes(named_shapes, placer, OBJECT_FONT_SIZE_PX, OBJECT_COLOR, False)
    return overlay.compositeOnto(picture)


def _render_zones_picture(world: World, picture: Image.Image) -> Image.Image:
    """`_render_zones_picture` outlines and names the root's zones on the map picture.

    Args:
        world: The world.
        picture: The map picture, in RGB.

    Example:
        >>> _render_zones_picture(world, picture).save("map_zones.png")
    """
    overlay: _Overlay = _Overlay(picture.size)
    named_shapes: list[tuple[str, Shape]] = [(name, world.zones[name].shape) for name in world.zones[world.root].zones]
    for _, shape in named_shapes:
        overlay.drawShape(shape, ZONE_COLOR, ZONE_LINE_WIDTH_PX)
    placer: _LabelPlacer = _LabelPlacer(picture.size, [thing.shape.getBoundsPx() for thing in world.objects.values()])
    overlay.labelShapes(named_shapes, placer, ZONE_FONT_SIZE_PX, ZONE_COLOR, True)
    return overlay.compositeOnto(picture)


def _render_zone_picture(world: World, picture: Image.Image, zone_name: str) -> Image.Image:
    """`_render_zone_picture` shows one zone on the whole map: the rest blurred and darkened, its parts named.

    Args:
        world: The world.
        picture: The map picture, in RGB.
        zone_name: The zone to show.

    Example:
        >>> _render_zone_picture(world, picture, "blue zone").save("zone_blue_zone.png")
    """
    zone = world.zones[zone_name]
    width_px, height_px = picture.size
    mask: Image.Image = Image.new("L", (width_px * SUPERSAMPLE, height_px * SUPERSAMPLE), 0)
    zone.shape.draw(ImageDraw.Draw(mask), SUPERSAMPLE, 255, None, 0)
    mask = mask.resize(picture.size, Image.Resampling.LANCZOS)
    outside: Image.Image = picture.filter(ImageFilter.GaussianBlur(OUTSIDE_ZONE_BLUR_PX))
    outside = ImageEnhance.Brightness(outside).enhance(1 - OUTSIDE_ZONE_DIM_FRAC)
    focused: Image.Image = Image.composite(picture, outside, mask)
    overlay: _Overlay = _Overlay(picture.size)
    overlay.drawShape(zone.shape, ZONE_COLOR, ZONE_LINE_WIDTH_PX)
    sub_zones: list[tuple[str, Shape]] = [(name, world.zones[name].shape) for name in zone.zones]
    objects: list[tuple[str, Shape]] = [(name, world.objects[name].shape) for name in zone.objects]
    for _, shape in sub_zones:
        overlay.drawShape(shape, SUB_ZONE_COLOR, ZONE_LINE_WIDTH_PX)
    for _, shape in objects:
        overlay.drawShape(shape, OBJECT_COLOR, OBJECT_LINE_WIDTH_PX)
    placer: _LabelPlacer = _LabelPlacer(picture.size, [shape.getBoundsPx() for _, shape in objects])
    overlay.labelShapes(sub_zones, placer, ZONE_FONT_SIZE_PX, SUB_ZONE_COLOR, True)
    overlay.labelShapes(objects, placer, OBJECT_FONT_SIZE_PX, OBJECT_COLOR, False)
    return overlay.compositeOnto(focused)


def _validate_level_folder(level_folder: Path) -> None:
    """`_validate_level_folder` checks that a level folder has the three source files.

    Args:
        level_folder: The level folder.

    Raises:
        click.BadParameter: A source file is missing.

    Example:
        >>> _validate_level_folder(Path("lunar_surface"))
    """
    for name in (MAP_PICTURE_NAME, WORLD_NAME, TEMPLATE_NAME):
        if not (level_folder / SOURCE_FOLDER_NAME / name).is_file():
            raise click.BadParameter(
                f"{level_folder} has no {SOURCE_FOLDER_NAME}/{name}; a level folder needs {SOURCE_FOLDER_NAME}/"
                f"{MAP_PICTURE_NAME}, {WORLD_NAME} and {TEMPLATE_NAME}. Example: make_world_doc.py lunar_surface",
                param_hint="LEVEL_FOLDERS",
            )


def make_outputs(level_folder: Path) -> dict[str, str | Image.Image]:
    """`make_outputs` makes every file of a level's `generated/` folder, in memory.

    Args:
        level_folder: The level folder.

    Returns:
        Each file's contents by file name: text for the doc, a picture for each PNG.

    Raises:
        pydantic.ValidationError: `world.yaml` doesn't follow the schema or its tree is wrong.
        ValueError: A shape lies outside the map picture, the template is wrong, or a label has no free place.

    Example:
        >>> outputs = make_outputs(Path("lunar_surface"))
    """
    source: Path = level_folder / SOURCE_FOLDER_NAME
    with Image.open(source / MAP_PICTURE_NAME) as opened:
        picture: Image.Image = opened.convert("RGB")
    world: World = World.FromYaml(source / WORLD_NAME, picture.size)
    slot_texts: dict[str, str] = {
        "map": f"![{world.level}'s map, without labels]({MAP_PICTURE_NAME})",
        "map_objects": f"![{world.level}'s map, with every object outlined and named]({OBJECTS_PICTURE_NAME})",
        "map_zones": f"![{world.level}'s map, with its zones outlined and named]({ZONES_PICTURE_NAME})",
        "world_tree": render_world_tree(world),
        "zone_sections": render_zone_sections(world),
    }
    doc: str = GENERATED_DOC_NOTE + hydrate_template((source / TEMPLATE_NAME).read_text(), slot_texts)
    outputs: dict[str, str | Image.Image] = {
        f"{level_folder.resolve().name}.md": doc,
        MAP_PICTURE_NAME: picture,
        OBJECTS_PICTURE_NAME: _render_objects_picture(world, picture),
        ZONES_PICTURE_NAME: _render_zones_picture(world, picture),
    }
    for zone_name in world.getZonesBreadthFirst():
        outputs[f"zone_{_get_slug(zone_name)}.png"] = _render_zone_picture(world, picture, zone_name)
    return outputs


def _is_file_current(path: Path, contents: str | Image.Image) -> bool:
    """`_is_file_current` says whether a file already holds what `make_outputs` made for it.

    Args:
        path: The file.
        contents: Its text, or its picture, compared pixel by pixel.

    Example:
        >>> _is_file_current(Path("lunar_surface/generated/map.png"), picture)
    """
    if not path.is_file():
        return False
    if isinstance(contents, str):
        return path.read_text() == contents
    with Image.open(path) as existing:
        return existing.size == contents.size and existing.convert("RGB").tobytes() == contents.tobytes()


@click.command(epilog="Example: uv run python make_world_doc.py lunar_surface")
@click.argument("level_folders", nargs=-1, required=True, type=click.Path(file_okay=False, exists=True, path_type=Path))
@click.option("--check", "should_check", is_flag=True, help="Write nothing; exit 1 if generated/ is out of date.")
@click.option("--dry-run", is_flag=True, help="Write nothing; list the files that would be written and removed.")
@click.option("--json", "should_print_json", is_flag=True, help="Print the results as one JSON object.")
def main(level_folders: tuple[Path, ...], should_check: bool, dry_run: bool, should_print_json: bool) -> None:
    """`main` writes the world doc and pictures of each level folder to its `generated/` folder.

    Args:
        level_folders: The level folders.
        should_check: Whether to only report out-of-date files.
        dry_run: Whether to only list what would change.
        should_print_json: Whether to print JSON.

    Raises:
        click.UsageError: `--check` and `--dry-run` were both given.
        click.ClickException: A level's source is wrong (exit code 1).

    Example:
        >>> main(["lunar_surface"])
    """
    if should_check and dry_run:
        raise click.UsageError(
            "--check and --dry-run can't be used together. Example: make_world_doc.py --check lunar_surface"
        )
    for level_folder in level_folders:
        _validate_level_folder(level_folder)
    outputs_by_level: dict[Path, dict[str, str | Image.Image]] = {}
    for level_folder in level_folders:
        try:
            outputs_by_level[level_folder] = make_outputs(level_folder)
        except (ValidationError, ValueError) as error:
            raise click.ClickException(f"{level_folder}: {error}") from error
    results: list[dict[str, object]] = []
    for level_folder, outputs in outputs_by_level.items():
        generated: Path = level_folder / GENERATED_FOLDER_NAME
        existing: set[str] = {path.name for path in generated.iterdir()} if generated.is_dir() else set()
        stale: list[str] = sorted(name for name in existing - outputs.keys())
        changed: list[str] = sorted(
            name for name, contents in outputs.items() if not _is_file_current(generated / name, contents)
        )
        if not should_check and not dry_run:
            generated.mkdir(exist_ok=True)
            for name in stale:
                (generated / name).unlink()
            for name in changed:
                contents: str | Image.Image = outputs[name]
                if isinstance(contents, str):
                    (generated / name).write_text(contents)
                else:
                    contents.save(generated / name, format="PNG")
        results.append(
            {
                "level_folder": str(level_folder),
                "changed": [str(generated / name) for name in changed],
                "removed": [str(generated / name) for name in stale],
            }
        )
    is_out_of_date: bool = any(result["changed"] or result["removed"] for result in results)
    if should_print_json:
        click.echo(json.dumps({"levels": results, "is_out_of_date": is_out_of_date}, indent=2))
    else:
        is_writing: bool = not should_check and not dry_run
        for result in results:
            click.echo(f"{result['level_folder']}:")
            for path in result["changed"]:
                click.echo(f"  {'wrote' if is_writing else 'would write'} {path}")
            for path in result["removed"]:
                click.echo(f"  {'removed' if is_writing else 'would remove'} {path}")
            if not result["changed"] and not result["removed"]:
                click.echo("  up to date")
    if should_check and is_out_of_date:
        raise click.ClickException("generated/ is out of date; run `make generate-world-docs` and commit the result")


if __name__ == "__main__":
    main()
