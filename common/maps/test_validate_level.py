"""`test_validate_level.py` tests `validate_map.py`'s level checks on Lunar Surface's map with a level file and a
levels manifest written for it, broken one way at a time.

| Partitioning | Parts | Tests |
| --- | --- | --- |
| Level against its schema | valid, a required field missing, an unknown field | `test_a_valid_level_passes`, `test_bad_levels_are_caught` |
| Name | the map's, another | `test_a_valid_level_passes`, `test_bad_levels_are_caught` |
| Engine | draws everything, unknown, can't draw the space, the style, a map feature or a level feature, misses a required map feature | `test_a_valid_level_passes`, `test_bad_engines_are_caught` |
| Growing black hole | none, a black hole, a wormhole mouth, a missing warp | `test_a_valid_level_passes`, `test_bad_levels_are_caught` |
| Laser colour | none, in the level only, in the map and the level | `test_a_valid_level_passes`, `test_bad_levels_are_caught`, `test_a_laser_colour_in_the_level_passes` |
| Pylons | as many for each team, fewer for one team, none | `test_a_valid_level_passes`, `test_bad_maps_are_caught` |
| Spawns | for both teams, none for one | `test_a_valid_level_passes`, `test_bad_maps_are_caught` |
| Map picture | none, present, missing | `test_a_valid_level_passes`, `test_map_pictures_are_checked`, `test_bad_levels_are_caught` |
| Manifest | valid, a schema error, an even match, a level twice, a level without a level file, a missing crate | `test_a_valid_level_passes`, `test_bad_manifests_are_caught` |
| Map features | every name is a path of the map schema | `test_map_features_are_map_fields` |

Example:
    uv run pytest test_validate_level.py
"""

import json
import shutil
from collections.abc import Callable
from pathlib import Path

import pytest

from validate_map import LEVEL_NAME, LEVELS_SCHEMA_PATH, MANIFEST_NAME, SCHEMA_PATH, _validate_level

MAPS_FOLDER: Path = Path(__file__).parent
LEVEL: dict = {"format": "warped-level", "version": 1, "name": "lunar_surface", "title": "Lunar Surface",
               "engine": "voxel_lunar", "black_hole_growth": {"warp": "black hole"}}  # fmt: skip
ENGINE: dict = {"crate": "demos/hyperbolic_wormholes_rust", "binary": "hyperbolic-wormholes", "spaces": ["hyperbolic"],
                "styles": ["lunar"], "lighting_models": ["lunar_lambert"], "required_map_features": [],
                "map_features": ["ground.tiling", "ground.terrain", "structures", "structures.mesh", "warps", "lights",
                                 "sky.planet", "sky.milky_way", "regions.zones", "regions.boundary", "regions.zone_walls"],
                "level_features": ["black_hole_growth", "laser_color"]}  # fmt: skip
MATCH_LEVELS: list[str] = ["lunar_surface", "second", "third"]
Change = Callable[[dict], object]


def _make_level(tmp_path: Path, level: Change = dict, map_: Change = dict, levels: Change = dict) -> Path:
    """`_make_level` writes Lunar Surface as a level in `tmp_path`, with `levels.json` beside it, each file changed.

    Args:
        tmp_path: pytest's temporary folder.
        level: Edits the level file in place.
        map_: Edits the map in place.
        levels: Edits the levels manifest in place.

    Returns:
        The level's folder.

    Example:
        >>> folder = _make_level(tmp_path, level=lambda level: level.pop("engine"))
    """
    folder: Path = tmp_path / "lunar_surface"
    shutil.copytree(MAPS_FOLDER / "lunar_surface", folder)
    documents: dict[str, dict] = {
        LEVEL_NAME: json.loads(json.dumps(LEVEL)),
        MANIFEST_NAME: json.loads((folder / MANIFEST_NAME).read_text()),
        "levels.json": {"format": "warped-levels", "version": 1, "engines": {"voxel_lunar": json.loads(json.dumps(ENGINE))},
                        "matches": {"default": {"levels": list(MATCH_LEVELS)}}},
    }  # fmt: skip
    for name, change in ((LEVEL_NAME, level), (MANIFEST_NAME, map_), ("levels.json", levels)):
        change(documents[name])
    (folder / LEVEL_NAME).write_text(json.dumps(documents[LEVEL_NAME]))
    (folder / MANIFEST_NAME).write_text(json.dumps(documents[MANIFEST_NAME]))
    (tmp_path / "levels.json").write_text(json.dumps(documents["levels.json"]))
    for name in MATCH_LEVELS[1:]:
        (tmp_path / name).mkdir()
        (tmp_path / name / LEVEL_NAME).write_text("{}")
    return folder


def _rename_material(map_: dict, old_name: str, new_name: str) -> None:
    """`_rename_material` renames the map's material `old_name`.

    Example:
        >>> _rename_material(map_, "red console 1", "blue console 4")
    """
    next(material for material in map_["materials"] if material["name"] == old_name)["name"] = new_name


def test_a_valid_level_passes(tmp_path: Path) -> None:
    assert _validate_level(_make_level(tmp_path), tmp_path / "levels.json") == []


def test_a_laser_colour_in_the_level_passes(tmp_path: Path) -> None:
    folder: Path = _make_level(tmp_path, level=lambda level: level.update(laser_color=[1.0, 0.8, 0.3]))
    assert _validate_level(folder, tmp_path / "levels.json") == []


@pytest.mark.parametrize(
    ("change", "expected"),
    [
        (lambda level: level.pop("engine"), "'engine' is a required property"),
        (lambda level: level.update(speed=1), "Additional properties"),
        (lambda level: level.update(name="dust"), "isn't the map's"),
        (lambda level: level.update(black_hole_growth={"warp": "wormhole mouth A"}), "a black hole"),
        (lambda level: level.update(black_hole_growth={"warp": "no such warp"}), "a black hole"),
        (lambda level: level.update(map_picture="missing.png"), "isn't in"),
    ],
    ids=["engine missing", "unknown field", "another name", "a mouth grows", "a missing warp grows", "missing picture"],
)
def test_bad_levels_are_caught(change: Change, expected: str, tmp_path: Path) -> None:
    errors: list[str] = _validate_level(_make_level(tmp_path, level=change), tmp_path / "levels.json")
    assert any(expected in error for error in errors), errors


def test_map_pictures_are_checked(tmp_path: Path) -> None:
    folder: Path = _make_level(tmp_path, level=lambda level: level.update(map_picture="map_picture.png"))
    (folder / "map_picture.png").write_bytes(b"")
    assert _validate_level(folder, tmp_path / "levels.json") == []


@pytest.mark.parametrize(
    ("change", "expected"),
    [
        (lambda map_: map_["appearance"].update(laser_color=[1.0, 0.8, 0.3]), "set it once"),
        (lambda map_: _rename_material(map_, "red console 1", "blue console 4"), "as many as the other"),
        (lambda map_: [m.update(name=f"plain {m['id']}") for m in map_["materials"]], "at least 1"),
        (lambda map_: map_.update(spawns=[s for s in map_["spawns"] if s["team"] != "red"]), "none for red"),
    ],
    ids=["two laser colours", "fewer red pylons", "no pylons", "no red spawn"],
)
def test_bad_maps_are_caught(change: Change, expected: str, tmp_path: Path) -> None:
    level: Change = (lambda level: level.update(laser_color=[1.0, 0.8, 0.3])) if "set it once" in expected else dict
    errors: list[str] = _validate_level(_make_level(tmp_path, level=level, map_=change), tmp_path / "levels.json")
    assert any(expected in error for error in errors), errors


@pytest.mark.parametrize(
    ("change", "expected"),
    [
        (lambda engines: engines.update(other=engines.pop("voxel_lunar")), "isn't one of"),
        (lambda engines: engines["voxel_lunar"].update(spaces=["euclidean"]), "the space 'hyperbolic'"),
        (lambda engines: engines["voxel_lunar"].update(styles=["meadow"]), "the style 'lunar'"),
        (lambda engines: engines["voxel_lunar"]["map_features"].remove("warps"), "the map feature 'warps'"),
        (lambda engines: engines["voxel_lunar"].update(level_features=[]), "the level feature 'black_hole_growth'"),
        (lambda engines: engines["voxel_lunar"].update(required_map_features=["ladders"]), "needs the map feature"),
    ],
    ids=["unknown engine", "space", "style", "map feature", "level feature", "required map feature"],
)
def test_bad_engines_are_caught(change: Change, expected: str, tmp_path: Path) -> None:
    folder: Path = _make_level(tmp_path, levels=lambda levels: change(levels["engines"]))
    errors: list[str] = _validate_level(folder, tmp_path / "levels.json")
    assert any(expected in error for error in errors), errors


@pytest.mark.parametrize(
    ("change", "expected"),
    [
        (lambda levels: levels.update(version=2), "version"),
        (lambda levels: levels["matches"]["default"]["levels"].pop(), "odd number"),
        (lambda levels: levels["matches"]["default"].update(levels=["lunar_surface"] * 3), "distinct"),
        (lambda levels: levels["matches"]["default"]["levels"].append("missing"), "'missing' has no level.json"),
        (lambda levels: levels["engines"]["voxel_lunar"].update(crate="demos/no_such_crate"), "isn't a folder"),
    ],
    ids=["schema", "even match", "a level twice", "no level file", "missing crate"],
)
def test_bad_manifests_are_caught(change: Change, expected: str, tmp_path: Path) -> None:
    errors: list[str] = _validate_level(_make_level(tmp_path, levels=change), tmp_path / "levels.json")
    assert any(expected in error for error in errors), errors


def test_map_features_are_map_fields() -> None:
    map_schema: dict = json.loads(SCHEMA_PATH.read_text())
    for path in json.loads(LEVELS_SCHEMA_PATH.read_text())["$defs"]["map_feature"]["enum"]:
        node: dict = map_schema
        for key in path.split("."):
            node = node["items"] if node["type"] == "array" else node
            node = node["properties"][key]
