"""`test_make_world_doc.py` tests `world_doc_schema.py` and `make_world_doc.py` on a small made-up level.

Partitionings and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| Shape kind | circle | test_shape_contains_inside_outside_and_edge |
| | ellipse | test_shape_contains_inside_outside_and_edge |
| | rect | test_shape_contains_inside_outside_and_edge |
| | polygon, convex | test_shape_contains_inside_outside_and_edge |
| | polygon, concave | test_polygon_centre_is_its_centroid |
| Point vs shape | inside | test_shape_contains_inside_outside_and_edge |
| | outside | test_shape_contains_inside_outside_and_edge |
| | on the edge (not polygon) | test_shape_contains_inside_outside_and_edge |
| Shape values | valid | test_world_accepts_a_valid_tree |
| | rect corners swapped, flat polygon | test_shape_rejects_degenerate_values |
| World tree | valid | test_world_accepts_a_valid_tree |
| | name both zone and object | test_world_rejects_a_broken_tree[shared name] |
| | child not defined | test_world_rejects_a_broken_tree[undefined child] |
| | child in two zones | test_world_rejects_a_broken_tree[two parents] |
| | zone the root can't reach | test_world_rejects_a_broken_tree[unreachable zone] |
| | root listed as a child | test_world_rejects_a_broken_tree[root as child] |
| | object listed in the root | test_world_rejects_a_broken_tree[object in root] |
| | object's centre outside its zone | test_world_rejects_a_broken_tree[object outside zone] |
| | object's centre in a sub-zone | test_world_rejects_a_broken_tree[object in sub-zone] |
| | shape outside the picture | test_world_rejects_a_shape_outside_the_picture |
| Zone depth | one level | test_world_accepts_a_valid_tree |
| | several levels, breadth first | test_zone_sections_are_breadth_first_with_paths |
| Template slots | each known slot once | test_cli_writes_then_checks_up_to_date |
| | unknown slot | test_template_rejects_bad_slots[unknown] |
| | slot repeated | test_template_rejects_bad_slots[repeated] |
| Label place | beside the mark | test_label_goes_beside_a_free_mark |
| | away from the mark | test_label_moves_away_when_beside_is_taken |
| | nowhere | test_label_fails_when_the_picture_is_full |
| CLI run | write, then check | test_cli_writes_then_checks_up_to_date |
| | check with a changed or extra file | test_cli_check_fails_on_changed_and_extra_files |
| | --check with --dry-run, missing source | test_cli_rejects_bad_arguments |
"""

import copy
from pathlib import Path

import pytest
import yaml
from click.testing import CliRunner, Result
from PIL import Image
from pydantic import TypeAdapter, ValidationError

from make_world_doc import _LabelPlacer, hydrate_template, main, render_zone_sections
from world_doc_schema import Shape, World

PICTURE_SIZE_PX: tuple[int, int] = (400, 200)
TEMPLATE: str = "# Test Level\n\n{{map}}\n\n{{map_zones}}\n\n{{world_tree}}\n\n{{map_objects}}\n\n{{zone_sections}}\n"
WORLD: dict = {
    "level": "Test Level",
    "root": "level",
    "zones": {
        "level": {"shape": {"kind": "rect", "min_px": [0, 0], "max_px": [400, 200]}, "description": "All of it.",
                  "zones": ["left zone", "right zone"]},
        "left zone": {"shape": {"kind": "rect", "min_px": [0, 0], "max_px": [200, 200]}, "description": "The left.",
                      "objects": ["left rock"]},
        "right zone": {"shape": {"kind": "rect", "min_px": [200, 0], "max_px": [400, 200]}, "description": "The right.",
                       "zones": ["right top"], "objects": ["right rock"]},
        "right top": {"shape": {"kind": "polygon", "points_px": [[250, 10], [350, 10], [300, 80]]},
                      "description": "A | triangle.", "objects": ["top post"]},
    },
    "objects": {
        "left rock": {"shape": {"kind": "circle", "centre_px": [100, 100], "radius_px": 10},
                      "description": "A rock.", "layout_name": "boulder"},
        "right rock": {"shape": {"kind": "ellipse", "centre_px": [300, 150], "radii_px": [20, 10]},
                       "description": "Another rock.", "layout_name": "boulder"},
        "top post": {"shape": {"kind": "rect", "min_px": [295, 30], "max_px": [305, 40]},
                     "description": "A post.", "layout_name": "post"},
    },
}  # fmt: skip
SHAPE_ADAPTER: TypeAdapter = TypeAdapter(Shape)


def _make_level(folder: Path, world: dict = WORLD, template: str = TEMPLATE) -> Path:
    """`_make_level` writes a level folder with a plain grey picture, `world` and `template`.

    Example:
        >>> level_folder = _make_level(tmp_path)
    """
    source: Path = folder / "test_level" / "source"
    source.mkdir(parents=True)
    Image.new("RGB", PICTURE_SIZE_PX, (128, 128, 128)).save(source / "map.png")
    (source / "world.yaml").write_text(yaml.safe_dump(world))
    (source / "template.md").write_text(template)
    return folder / "test_level"


@pytest.mark.parametrize(
    "shape, inside_px, outside_px, edge_px",
    [
        ({"kind": "circle", "centre_px": [0, 0], "radius_px": 5}, (3, 3), (4, 4), (5, 0)),
        ({"kind": "ellipse", "centre_px": [0, 0], "radii_px": [10, 5]}, (9, 0), (9, 3), (0, 5)),
        ({"kind": "rect", "min_px": [0, 0], "max_px": [10, 5]}, (5, 2), (11, 2), (10, 5)),
        ({"kind": "polygon", "points_px": [[0, 0], [10, 0], [10, 10], [0, 10]]}, (5, 5), (15, 5), None),
    ],
)
def test_shape_contains_inside_outside_and_edge(shape: dict, inside_px, outside_px, edge_px) -> None:
    """`test_shape_contains_inside_outside_and_edge` checks each shape kind's `containsPx`."""
    parsed = SHAPE_ADAPTER.validate_python(shape)
    assert parsed.containsPx(inside_px)
    assert not parsed.containsPx(outside_px)
    if edge_px is not None:
        assert parsed.containsPx(edge_px)


def test_polygon_centre_is_its_centroid() -> None:
    """`test_polygon_centre_is_its_centroid` checks an L shape's centre, which its corners' average would get wrong."""
    shape = SHAPE_ADAPTER.validate_python(
        {"kind": "polygon", "points_px": [[0, 0], [20, 0], [20, 10], [10, 10], [10, 30], [0, 30]]}
    )
    assert shape.getCentrePx() == pytest.approx((7.5, 12.5))


@pytest.mark.parametrize(
    "shape",
    [
        {"kind": "rect", "min_px": [10, 10], "max_px": [5, 20]},
        {"kind": "polygon", "points_px": [[0, 0], [1, 1], [2, 2]]},
    ],
)
def test_shape_rejects_degenerate_values(shape: dict) -> None:
    """`test_shape_rejects_degenerate_values` checks that a swapped rect and a flat polygon fail."""
    with pytest.raises(ValidationError):
        SHAPE_ADAPTER.validate_python(shape)


def test_world_accepts_a_valid_tree() -> None:
    """`test_world_accepts_a_valid_tree` checks that the test level loads."""
    world: World = World.model_validate(WORLD)
    assert world.getZonePath("left zone") == ["left zone"]


def _break_world(change: str) -> dict:
    """`_break_world` gives a copy of the test world with one thing wrong.

    Example:
        >>> _break_world("two parents")
    """
    world: dict = copy.deepcopy(WORLD)
    zones: dict = world["zones"]
    if change == "shared name":
        world["objects"]["right top"] = world["objects"].pop("top post")
        zones["right top"]["objects"] = ["right top"]
    elif change == "undefined child":
        zones["left zone"]["objects"].append("ghost")
    elif change == "two parents":
        zones["left zone"]["objects"].append("right rock")
    elif change == "unreachable zone":
        zones["level"]["zones"].remove("left zone")
    elif change == "root as child":
        zones["left zone"]["zones"] = ["level"]
    elif change == "object in root":
        zones["level"]["objects"] = ["left rock"]
        zones["left zone"]["objects"] = []
    elif change == "object outside zone":
        world["objects"]["left rock"]["shape"]["centre_px"] = [300, 150]
    elif change == "object in sub-zone":
        zones["right top"]["objects"] = []
        zones["right zone"]["objects"].append("top post")
    return world


@pytest.mark.parametrize(
    "change",
    [
        "shared name",
        "undefined child",
        "two parents",
        "unreachable zone",
        "root as child",
        "object in root",
        "object outside zone",
        "object in sub-zone",
    ],
)
def test_world_rejects_a_broken_tree(change: str) -> None:
    """`test_world_rejects_a_broken_tree` checks that each kind of broken tree fails to load."""
    with pytest.raises(ValidationError):
        World.model_validate(_break_world(change))


def test_world_rejects_a_shape_outside_the_picture(tmp_path: Path) -> None:
    """`test_world_rejects_a_shape_outside_the_picture` checks the picture-size check of `World.FromYaml`."""
    level_folder: Path = _make_level(tmp_path)
    with pytest.raises(ValueError, match="outside the 300x200 map picture"):
        World.FromYaml(level_folder / "source" / "world.yaml", (300, 200))


def test_zone_sections_are_breadth_first_with_paths() -> None:
    """`test_zone_sections_are_breadth_first_with_paths` checks section order, headings, links and escaping."""
    sections: str = render_zone_sections(World.model_validate(WORLD))
    headings: list[str] = [line for line in sections.splitlines() if line.startswith("## ")]
    assert headings == ["## Left zone", "## Right zone", "## Right zone > right top"]
    assert "[**right top**](#right-zone--right-top)" in sections
    assert "A \\| triangle." in sections


@pytest.mark.parametrize(
    "template", ["{{world_tree}} {{wrold_tree}}", "{{map}}\n{{ map }}"], ids=["unknown", "repeated"]
)
def test_template_rejects_bad_slots(template: str) -> None:
    """`test_template_rejects_bad_slots` checks that an unknown or repeated slot fails."""
    with pytest.raises(ValueError, match="template"):
        hydrate_template(template, {"map": "", "world_tree": ""})


def test_label_goes_beside_a_free_mark() -> None:
    """`test_label_goes_beside_a_free_mark` checks that a label with room goes right of its mark, with no line."""
    placer: _LabelPlacer = _LabelPlacer(PICTURE_SIZE_PX, [(100, 100, 110, 110)])
    box, is_beside = placer.placeLabel((40, 10), (100, 100, 110, 110), False)
    assert is_beside and box[0] > 110


def test_label_moves_away_when_beside_is_taken() -> None:
    """`test_label_moves_away_when_beside_is_taken` checks that a crowded mark's label goes farther, with a line."""
    crowd: list = [(60, 60, 160, 160)]
    placer: _LabelPlacer = _LabelPlacer(PICTURE_SIZE_PX, crowd)
    _, is_beside = placer.placeLabel((40, 10), (105, 105, 115, 115), False)
    assert not is_beside


def test_label_fails_when_the_picture_is_full() -> None:
    """`test_label_fails_when_the_picture_is_full` checks that a label with no room raises."""
    placer: _LabelPlacer = _LabelPlacer(PICTURE_SIZE_PX, [(0, 0, *PICTURE_SIZE_PX)])
    with pytest.raises(ValueError, match="no free place"):
        placer.placeLabel((40, 10), (100, 100, 110, 110), False)


def test_cli_writes_then_checks_up_to_date(tmp_path: Path) -> None:
    """`test_cli_writes_then_checks_up_to_date` checks a run's files, and that `--check` then passes."""
    level_folder: Path = _make_level(tmp_path)
    runner: CliRunner = CliRunner()
    assert runner.invoke(main, [str(level_folder)]).exit_code == 0
    names: set[str] = {path.name for path in (level_folder / "generated").iterdir()}
    assert names == {
        "test_level.md", "map.png", "map_objects.png", "map_zones.png",
        "zone_left_zone.png", "zone_right_zone.png", "zone_right_top.png",
    }  # fmt: skip
    assert "{{" not in (level_folder / "generated" / "test_level.md").read_text()
    assert runner.invoke(main, ["--check", str(level_folder)]).exit_code == 0


def test_cli_check_fails_on_changed_and_extra_files(tmp_path: Path) -> None:
    """`test_cli_check_fails_on_changed_and_extra_files` checks `--check` on stale output, and that a run fixes it."""
    level_folder: Path = _make_level(tmp_path)
    runner: CliRunner = CliRunner()
    runner.invoke(main, [str(level_folder)])
    generated: Path = level_folder / "generated"
    Image.new("RGB", PICTURE_SIZE_PX, (0, 0, 0)).save(generated / "map_zones.png")
    (generated / "old.png").write_text("stale")
    result: Result = runner.invoke(main, ["--check", "--json", str(level_folder)])
    assert result.exit_code == 1
    assert "map_zones.png" in result.output and "old.png" in result.output
    assert runner.invoke(main, [str(level_folder)]).exit_code == 0
    assert not (generated / "old.png").exists()
    assert runner.invoke(main, ["--check", str(level_folder)]).exit_code == 0


def test_cli_rejects_bad_arguments(tmp_path: Path) -> None:
    """`test_cli_rejects_bad_arguments` checks usage errors: two modes at once, and a folder with no source."""
    level_folder: Path = _make_level(tmp_path)
    runner: CliRunner = CliRunner()
    assert runner.invoke(main, ["--check", "--dry-run", str(level_folder)]).exit_code == 2
    (tmp_path / "empty").mkdir()
    assert runner.invoke(main, [str(tmp_path / "empty")]).exit_code == 2
