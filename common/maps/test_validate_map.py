"""`test_validate_map.py` tests `validate_map.py` on the shipped maps and on copies broken one way at a time.

| Partitioning | Parts | Tests |
| --- | --- | --- |
| Shipped map | boxes only (house_demo), shapes and mounds (lunar_surface) | `test_shipped_maps_are_valid` |
| Manifest against the schema | valid, a required field missing, a wrong type | `test_shipped_maps_are_valid`, `test_schema_errors_are_reported` |
| Frame | an isometry, not one | `test_shipped_maps_are_valid`, `test_a_bent_frame_is_caught` |
| Shapes | valid, an unknown material, rounding too big, an unknown kind | `test_shipped_maps_are_valid`, `test_bad_shapes_are_caught` |
| Mouth partners | paired, unpaired | `test_shipped_maps_are_valid`, `test_unpaired_mouths_are_caught` |
| Tapered reaches | apart, overlapping | `test_shipped_maps_are_valid`, `test_overlapping_tapered_reaches_are_caught` |
| Name | matches the folder, doesn't | `test_shipped_maps_are_valid`, `test_a_wrong_name_is_caught` |

Example:
    uv run pytest
"""

import json
import shutil
from collections.abc import Callable
from pathlib import Path

import pytest

from validate_map import MANIFEST_NAME, SCHEMA_PATH, _validate_map

MAPS_FOLDER: Path = Path(__file__).parent
SCHEMA: dict = json.loads(SCHEMA_PATH.read_text())


def _copy_map(name: str, tmp_path: Path, change: Callable[[dict], None] | None = None) -> Path:
    """`_copy_map` copies a shipped map into `tmp_path/<name>` and applies `change` to its manifest.

    Args:
        name: The shipped map's folder name.
        tmp_path: pytest's temporary folder.
        change: Edits the manifest in place, or None.

    Returns:
        The copy's folder.

    Example:
        >>> folder = _copy_map("house_demo", tmp_path, lambda m: m.pop("sky"))
    """
    folder: Path = tmp_path / name
    shutil.copytree(MAPS_FOLDER / name, folder)
    if change is not None:
        manifest: dict = json.loads((folder / MANIFEST_NAME).read_text())
        change(manifest)
        (folder / MANIFEST_NAME).write_text(json.dumps(manifest))
    return folder


@pytest.mark.parametrize("name", ["house_demo", "lunar_surface"])
def test_shipped_maps_are_valid(name: str) -> None:
    assert _validate_map(MAPS_FOLDER / name, SCHEMA) == []


@pytest.mark.parametrize(
    "change",
    [lambda m: m.pop("sky"), lambda m: m.update(gravity_m_per_s2="down")],
    ids=["missing sky", "wrong gravity type"],
)
def test_schema_errors_are_reported(change: Callable[[dict], None], tmp_path: Path) -> None:
    assert _validate_map(_copy_map("house_demo", tmp_path, change), SCHEMA) != []


def test_a_bent_frame_is_caught(tmp_path: Path) -> None:
    def bend(manifest: dict) -> None:
        manifest["boxes"][0]["frame"][0] = 2.0

    errors: list[str] = _validate_map(_copy_map("house_demo", tmp_path, bend), SCHEMA)
    assert any("not an isometry" in error for error in errors)


@pytest.mark.parametrize(
    ("damage", "expected"),
    [
        (lambda shape: shape.update(material=254), "isn't in the palette"),
        (lambda shape: shape.update(round_m=100.0), "exceeds its smallest half size"),
        (lambda shape: shape.update(kind="torus"), "torus"),
    ],
    ids=["unknown material", "rounding too big", "unknown kind"],
)
def test_bad_shapes_are_caught(damage: Callable[[dict], None], expected: str, tmp_path: Path) -> None:
    errors: list[str] = _validate_map(
        _copy_map("lunar_surface", tmp_path, lambda manifest: damage(manifest["structures"][0]["shapes"][0])), SCHEMA
    )
    assert any(expected in error for error in errors)


def test_unpaired_mouths_are_caught(tmp_path: Path) -> None:
    def unpair(manifest: dict) -> None:
        manifest["warps"][1]["partner"] = 1

    errors: list[str] = _validate_map(_copy_map("lunar_surface", tmp_path, unpair), SCHEMA)
    assert any("partner" in error for error in errors)


def test_overlapping_tapered_reaches_are_caught(tmp_path: Path) -> None:
    def overlap(manifest: dict) -> None:
        manifest["warps"][0]["reach_m"] = 1000.0

    errors: list[str] = _validate_map(_copy_map("lunar_surface", tmp_path, overlap), SCHEMA)
    assert any("overlap" in error for error in errors)


def test_a_wrong_name_is_caught(tmp_path: Path) -> None:
    errors: list[str] = _validate_map(_copy_map("house_demo", tmp_path, lambda m: m.update(name="other")), SCHEMA)
    assert any("doesn't match" in error for error in errors)
