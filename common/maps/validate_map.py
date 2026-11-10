"""`validate_map.py` checks that map folders follow the map format of `common/maps/README.md`.

It checks `map.json` against `map.schema.json`, then what the schema can't say: the name matches the folder,
material ids are unique with 0 (and only 0) empty, every material used exists, frames are isometries of the map's
space, points lie in the space, shapes' rounding fits inside them, wormhole mouths pair up, tapered warps'
reach spheres don't overlap, directions are unit vectors, the Milky Way's core is perpendicular to its pole, and
only a map without terrain has a soft edge. A folder that also holds a `level.json` is a level, which it checks as `LEVELS.md` says: the level and `levels.json`
against their schemas, and the map against the level's engine, pylons, spawns and laser colour. It prints one line
per map (or `--json`) and exits 1 if any map is invalid.

Interface:
    uv run python validate_map.py <map folder>... [--json]

Example:
    uv run python validate_map.py house_demo lunar_surface
"""

import json
import re
from pathlib import Path

import click
import jsonschema
import numpy as np

SCHEMA_PATH: Path = Path(__file__).parent / "map.schema.json"
MANIFEST_NAME: str = "map.json"
MODELS_FOLDER: Path = Path(__file__).parent / "models"
TOLERANCE: float = 1e-6
EMPTY_ID: int = 0
MINKOWSKI: np.ndarray = np.diag([1.0, 1.0, 1.0, -1.0])
CURVED_KINDS: tuple[str, ...] = ("hyperbolic", "spherical")
LEVEL_NAME: str = "level.json"
LEVEL_SCHEMA_PATH: Path = Path(__file__).parent / "level.schema.json"
LEVELS_PATH: Path = Path(__file__).parent / "levels.json"
LEVELS_SCHEMA_PATH: Path = Path(__file__).parent / "levels.schema.json"
REPOSITORY_FOLDER: Path = Path(__file__).resolve().parents[2]
TEAMS: tuple[str, ...] = ("blue", "red")
CONSOLE_NAME_PATTERN: re.Pattern = re.compile(r"^(blue|red) console( [0-9]+)?$")
LASER_COLOR_PATHS: tuple[str, ...] = ("laser", "lasers", "appearance.laser_color")
MIN_MATCH_LEVEL_COUNT: int = 3


def _get_frame_errors(name: str, values: list[float], space_kind: str) -> list[str]:
    """`_get_frame_errors` checks that a frame (16 numbers, column-major) is an isometry of the space.

    Args:
        name: What the frame belongs to, for the messages.
        values: The frame.
        space_kind: `hyperbolic`, `spherical` or `euclidean`.

    Returns:
        One message per problem; empty if the frame is an isometry.

    Example:
        >>> _get_frame_errors("box 3", list(np.eye(4).flatten()), "hyperbolic")
        []
    """
    frame: np.ndarray = np.array(values, dtype=np.float64).reshape(4, 4).T
    if space_kind == "hyperbolic":
        is_isometry: bool = np.allclose(frame.T @ MINKOWSKI @ frame, MINKOWSKI, atol=TOLERANCE)
        is_isometry = is_isometry and frame[3, 3] > 0.0
    elif space_kind == "spherical":
        is_isometry = np.allclose(frame.T @ frame, np.eye(4), atol=TOLERANCE)
    else:
        rotation: np.ndarray = frame[:3, :3]
        is_isometry = np.allclose(rotation.T @ rotation, np.eye(3), atol=TOLERANCE) and np.allclose(
            frame[3], [0.0, 0.0, 0.0, 1.0], atol=TOLERANCE
        )
    return [] if is_isometry else [f"{name}: its frame is not an isometry of {space_kind} space"]


def _get_point_errors(name: str, point: list[float], space_kind: str) -> list[str]:
    """`_get_point_errors` checks that a 4-vector is a point of the space.

    Args:
        name: What the point belongs to, for the messages.
        point: The point (x, y, z, w).
        space_kind: `hyperbolic`, `spherical` or `euclidean`.

    Returns:
        One message if the point is off the space; else empty.

    Example:
        >>> _get_point_errors("light 0", [0.0, 0.0, 0.0, 1.0], "hyperbolic")
        []
    """
    p: np.ndarray = np.array(point, dtype=np.float64)
    if space_kind == "hyperbolic":
        is_on: bool = abs(p @ MINKOWSKI @ p + 1.0) < TOLERANCE and p[3] > 0.0
    elif space_kind == "spherical":
        is_on = abs(p @ p - 1.0) < TOLERANCE
    else:
        is_on = abs(p[3] - 1.0) < TOLERANCE
    return [] if is_on else [f"{name}: its position is not a point of {space_kind} space"]


def _get_distance(a: list[float], b: list[float], space_kind: str, radius_m: float) -> float:
    """`_get_distance` gives the distance in meters between two points of the space.

    Args:
        a: A point.
        b: Another point.
        space_kind: `hyperbolic`, `spherical` or `euclidean`.
        radius_m: The curvature radius (unused in euclidean space).

    Returns:
        The distance.

    Example:
        >>> _get_distance([0, 0, 0, 1], [0, 0, 0, 1], "hyperbolic", 8.0)
        0.0
    """
    p, q = np.array(a, dtype=np.float64), np.array(b, dtype=np.float64)
    if space_kind == "hyperbolic":
        return float(radius_m * np.arccosh(max(1.0, -(p @ MINKOWSKI @ q))))
    if space_kind == "spherical":
        return float(radius_m * np.arccos(np.clip(p @ q, -1.0, 1.0)))
    return float(np.linalg.norm(p[:3] - q[:3]))


def _get_material_errors(manifest: dict) -> list[str]:
    """`_get_material_errors` checks the palette: unique ids, 0 and only 0 empty, and every material used exists.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        One message per problem.

    Example:
        >>> _get_material_errors({"materials": [{"id": 0, "kind": "empty"}], "ground": {"material": 0}})
        []
    """
    errors: list[str] = []
    ids: list[int] = [material["id"] for material in manifest["materials"]]
    if len(ids) != len(set(ids)):
        errors.append("materials: an id is used twice")
    empty_ids: list[int] = [m["id"] for m in manifest["materials"] if m["kind"] == "empty"]
    if empty_ids != [EMPTY_ID]:
        errors.append(f"materials: exactly material {EMPTY_ID} must be empty, got {empty_ids}")
    used: list[tuple[str, int]] = [("ground", manifest["ground"]["material"])]
    if "boxes" in manifest:
        used += [(f"box {box['name']!r}", box["material"]) for box in manifest["boxes"]]
    for structure in manifest["structures"] if "structures" in manifest else []:
        used += [(f"structure {structure['name']!r}", shape["material"]) for shape in structure["shapes"]]
    errors += [f"{owner}: material {id_} isn't in the palette" for owner, id_ in used if id_ not in set(ids)]
    return errors


def _get_shape_errors(manifest: dict) -> list[str]:
    """`_get_shape_errors` checks that each structure shape's rounding fits inside its half sizes.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        One message per problem.

    Example:
        >>> _get_shape_errors({"structures": []})
        []
    """
    return [
        f"structure {structure['name']!r}: shape {k}'s round_m {shape['round_m']} exceeds its smallest half size"
        for structure in (manifest["structures"] if "structures" in manifest else [])
        for k, shape in enumerate(structure["shapes"])
        if "round_m" in shape and shape["round_m"] > min(shape["half_m"])
    ]


def _get_mesh_errors(manifest: dict) -> list[str]:
    """`_get_mesh_errors` checks that each structure's model is in `models/`.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        One message per problem.

    Example:
        >>> _get_mesh_errors({"structures": []})
        []
    """
    return [
        f"structure {structure['name']!r}: model {structure['mesh']['model']!r} isn't in {MODELS_FOLDER}"
        for structure in (manifest["structures"] if "structures" in manifest else [])
        if "mesh" in structure and not (MODELS_FOLDER / structure["mesh"]["model"] / "scene.gltf").exists()
    ]


def _get_warp_errors(manifest: dict) -> list[str]:
    """`_get_warp_errors` checks that wormhole mouths pair up, that `flat_effective` is only used in hyperbolic space,
    and that a tapered warp's reach sphere overlaps no other warp's.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        One message per problem.

    Example:
        >>> _get_warp_errors({"space": {"kind": "euclidean"}})
        []
    """
    warps: list[dict] = manifest["warps"] if "warps" in manifest else []
    space: dict = manifest["space"]
    radius_m: float = space["curvature_radius_m"] if space["kind"] in CURVED_KINDS else 1.0
    errors: list[str] = []
    for k, warp in enumerate(warps):
        if warp["radial_coordinate"] == "flat_effective" and space["kind"] != "hyperbolic":
            errors.append(f"warp {warp['name']!r}: flat_effective needs hyperbolic space")
        if warp["kind"] != "wormhole_mouth":
            continue
        partner: int = warp["partner"]
        is_paired: bool = partner != k and partner < len(warps) and warps[partner]["kind"] == "wormhole_mouth"
        if not is_paired or warps[partner]["partner"] != k:
            errors.append(f"warp {warp['name']!r}: its partner {partner} must be another mouth whose partner is {k}")
    centres: list[list[float]] = [list(np.array(w["frame"]).reshape(4, 4)[3]) for w in warps]
    for i in range(len(warps)):
        for j in range(i + 1, len(warps)):
            if "taper" not in (warps[i]["falloff"], warps[j]["falloff"]):
                continue
            gap_m: float = _get_distance(centres[i], centres[j], space["kind"], radius_m)
            if gap_m < warps[i]["reach_m"] + warps[j]["reach_m"]:
                errors.append(f"warps {warps[i]['name']!r} and {warps[j]['name']!r}: tapered reach spheres overlap")
    return errors


def _get_geometry_errors(manifest: dict) -> list[str]:
    """`_get_geometry_errors` checks every frame, light position and sky direction.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        One message per problem.

    Example:
        >>> _get_geometry_errors(json.loads((Path("house_demo") / "map.json").read_text()))
        []
    """
    kind: str = manifest["space"]["kind"]
    errors: list[str] = []
    for key in ("structures", "boxes", "warps", "spawns"):
        for k, item in enumerate(manifest[key] if key in manifest else []):
            label: str = item["name"] if "name" in item else str(k)
            errors += _get_frame_errors(f"{key[:-1]} {label!r}", item["frame"], kind)
    for k, light in enumerate(manifest["lights"] if "lights" in manifest else []):
        errors += _get_point_errors(f"light {k}", light["position"], kind)
    sky: dict = manifest["sky"]
    bodies: list[tuple[str, list[float]]] = [("sun", sky["sun"]["direction"])]
    if "planet" in sky:
        bodies.append(("planet", sky["planet"]["direction"]))
    if "milky_way" in sky:
        bodies += [
            (f"Milky Way's {key[: -len('_direction')]}", sky["milky_way"][key])
            for key in ("pole_direction", "core_direction")
        ]
        if abs(np.dot(sky["milky_way"]["pole_direction"], sky["milky_way"]["core_direction"])) > TOLERANCE:
            errors.append("sky: the Milky Way's core is not perpendicular to its pole")
    for name, direction in bodies:
        if abs(np.linalg.norm(direction) - 1.0) > TOLERANCE:
            errors.append(f"sky: the {name}'s direction is not a unit vector")
    return errors


def _get_edge_errors(manifest: dict) -> list[str]:
    """`_get_edge_errors` checks that a soft edge (`regions.boundary.soft_edge_m`) is on a map without terrain.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        One message if a map with terrain has a soft edge (its edge is up the crater's wall); else empty.

    Example:
        >>> _get_edge_errors(json.loads((Path("snow_planet") / "map.json").read_text()))
        []
    """
    regions: dict = manifest["regions"] if "regions" in manifest else {}
    boundary: dict = regions["boundary"] if "boundary" in regions else {}
    if "soft_edge_m" in boundary and "terrain" in manifest["ground"]:
        return ["regions: a map with terrain has its edge up the crater's wall, not a boundary's soft_edge_m"]
    return []


def _get_schema_errors(document: dict, schema: dict) -> list[str]:
    """`_get_schema_errors` checks a JSON document against a JSON schema.

    Args:
        document: The document.
        schema: The schema.

    Returns:
        One message per problem, each starting with the path of the value at fault.

    Example:
        >>> _get_schema_errors({}, {"required": ["name"]})
        ["top level: 'name' is a required property"]
    """
    return [
        f"{'/'.join(str(p) for p in error.absolute_path) or 'top level'}: {error.message}"
        for error in jsonschema.Draft202012Validator(schema).iter_errors(document)
    ]


def _has_path(value: object, keys: list[str]) -> bool:
    """`_has_path` says whether a JSON value has the nested keys `keys`, where a list has them if any item does.

    Args:
        value: The JSON value.
        keys: The keys from the outside in, such as `["lights", "flicker_frac"]`.

    Returns:
        Whether the keys are there.

    Example:
        >>> _has_path({"lights": [{"flicker_frac": 0.3}]}, ["lights", "flicker_frac"])
        True
    """
    if not keys:
        return True
    if isinstance(value, list):
        return any(_has_path(item, keys) for item in value)
    return isinstance(value, dict) and keys[0] in value and _has_path(value[keys[0]], keys[1:])


def _count_pylons(manifest: dict) -> dict[str, int]:
    """`_count_pylons` counts each team's pylons: the structures and boxes made of a console material.

    Args:
        manifest: The map's manifest (already valid against the schema).

    Returns:
        The number of pylons of each team in `TEAMS`.

    Example:
        >>> _count_pylons(json.loads((Path("lunar_surface") / "map.json").read_text()))
        {'blue': 3, 'red': 3}
    """
    console_teams: dict[int, str] = {
        material["id"]: match.group(1)
        for material in manifest["materials"]
        if (match := CONSOLE_NAME_PATTERN.match(material["name"])) is not None
    }
    items: list[set[int]] = [
        {shape["material"] for shape in structure["shapes"]}
        for structure in (manifest["structures"] if "structures" in manifest else [])
    ] + [{box["material"]} for box in (manifest["boxes"] if "boxes" in manifest else [])]
    counts: dict[str, int] = {team: 0 for team in TEAMS}
    for materials in items:
        for team in {console_teams[id_] for id_ in materials if id_ in console_teams}:
            counts[team] += 1
    return counts


def _get_levels_errors(levels: dict, levels_schema: dict, maps_folder: Path) -> list[str]:
    """`_get_levels_errors` checks the levels manifest: its schema, that each engine's crate exists, and that each
    match is an odd number of distinct levels, at least `MIN_MATCH_LEVEL_COUNT`, each a folder with a `level.json`.

    Args:
        levels: The manifest.
        levels_schema: The manifest's JSON schema.
        maps_folder: The folder that holds the levels' folders.

    Returns:
        One message per problem.

    Example:
        >>> _get_levels_errors(json.loads(LEVELS_PATH.read_text()), json.loads(LEVELS_SCHEMA_PATH.read_text()), Path("."))
        []
    """
    errors: list[str] = _get_schema_errors(levels, levels_schema)
    if errors:
        return errors
    for name, match in levels["matches"].items():
        names: list[str] = match["levels"]
        if len(names) < MIN_MATCH_LEVEL_COUNT or len(names) % 2 == 0 or len(set(names)) != len(names):
            errors.append(
                f"match {name!r}: needs an odd number of distinct levels, at least {MIN_MATCH_LEVEL_COUNT}, got {names}"
            )
        errors += [
            f"match {name!r}: {level!r} has no {LEVEL_NAME}"
            for level in names
            if not (maps_folder / level / LEVEL_NAME).exists()
        ]
    return errors + [
        f"engine {name!r}: its crate {engine['crate']} isn't a folder of the repository"
        for name, engine in levels["engines"].items()
        if not (REPOSITORY_FOLDER / engine["crate"]).is_dir()
    ]


def _get_level_errors(folder: Path, manifest: dict, levels: dict, schemas: tuple[dict, dict]) -> list[str]:
    """`_get_level_errors` checks a level's `level.json` against its schema and its map: the same name, an engine
    that can draw the map's space, style, lighting and features and the level's features, the black hole that grows,
    one laser colour at most, as many pylons for each team (at least 1), a spawn for each team, and the map picture.

    Args:
        folder: The level's folder.
        manifest: The level's map (already valid).
        levels: The levels manifest (already valid).
        schemas: The level's and the levels manifest's JSON schemas.

    Returns:
        One message per problem.

    Example:
        >>> _get_level_errors(Path("dust"), dust_map, levels, (level_schema, levels_schema))
        []
    """
    level: dict = json.loads((folder / LEVEL_NAME).read_text())
    errors: list[str] = [f"{LEVEL_NAME}: {error}" for error in _get_schema_errors(level, schemas[0])]
    if errors:
        return errors
    if level["name"] != manifest["name"]:
        errors.append(f"{LEVEL_NAME}: name {level['name']!r} isn't the map's {manifest['name']!r}")
    if level["engine"] not in levels["engines"]:
        return errors + [f"{LEVEL_NAME}: engine {level['engine']!r} isn't one of {sorted(levels['engines'])}"]
    engine: dict = levels["engines"][level["engine"]]
    uses: list[tuple[str, str, list[str]]] = [
        ("space", manifest["space"]["kind"], engine["spaces"]),
        ("style", manifest["appearance"]["style"], engine["styles"]),
        ("lighting model", manifest["appearance"]["lighting"]["model"], engine["lighting_models"]),
    ]
    uses += [
        ("map feature", path, engine["map_features"])
        for path in schemas[1]["$defs"]["map_feature"]["enum"]
        if _has_path(manifest, path.split("."))
    ]
    uses += [
        ("level feature", key, engine["level_features"])
        for key in schemas[1]["$defs"]["level_feature"]["enum"]
        if key in level
    ]
    errors += [
        f"engine {level['engine']!r} can't draw the {what} {value!r}"
        for what, value, known in uses
        if value not in known
    ]
    errors += [
        f"engine {level['engine']!r} needs the map feature {path!r}"
        for path in engine["required_map_features"]
        if not _has_path(manifest, path.split("."))
    ]
    if "black_hole_growth" in level:
        hole_name: str = level["black_hole_growth"]["warp"]
        warps: list[dict] = manifest["warps"] if "warps" in manifest else []
        if [w["kind"] for w in warps if w["name"] == hole_name] != ["black_hole"]:
            errors.append(f"black_hole_growth: the map needs exactly one warp named {hole_name!r}, a black hole")
    laser_sources: list[str] = [p for p in LASER_COLOR_PATHS if _has_path(manifest, p.split("."))]
    laser_sources += [f"{LEVEL_NAME}'s laser_color"] if "laser_color" in level else []
    if len(laser_sources) > 1:
        errors.append(f"laser colour: set in {laser_sources}; set it once, in {LEVEL_NAME}'s laser_color")
    pylon_counts: dict[str, int] = _count_pylons(manifest)
    if pylon_counts["blue"] != pylon_counts["red"] or pylon_counts["blue"] == 0:
        errors.append(f"pylons: each team needs as many as the other, at least 1, got {pylon_counts}")
    spawn_teams: set[str] = {spawn["team"] for spawn in manifest["spawns"]}
    errors += [f"spawns: none for {team}" for team in TEAMS if not spawn_teams & {team, "any"}]
    if "map_picture" in level and not (folder / level["map_picture"]).is_file():
        errors.append(f"map_picture: {level['map_picture']} isn't in {folder}")
    return errors


def _validate_level(folder: Path, levels_path: Path) -> list[str]:
    """`_validate_level` checks a level folder, one with a `map.json` and a `level.json` (`_get_level_errors`), and
    the levels manifest at `levels_path` that names its engine (`_get_levels_errors`).

    Args:
        folder: The level's folder.
        levels_path: The levels manifest.

    Returns:
        One message per problem; empty if the level is valid.

    Example:
        >>> _validate_level(Path("dust"), LEVELS_PATH)
        []
    """
    map_errors: list[str] = _validate_map(folder, json.loads(SCHEMA_PATH.read_text()))
    if map_errors:
        return map_errors
    schemas: tuple[dict, dict] = (json.loads(LEVEL_SCHEMA_PATH.read_text()), json.loads(LEVELS_SCHEMA_PATH.read_text()))
    levels: dict = json.loads(levels_path.read_text())
    levels_errors: list[str] = _get_levels_errors(levels, schemas[1], levels_path.parent)
    if levels_errors:
        return [f"{levels_path.name}: {error}" for error in levels_errors]
    manifest: dict = json.loads((folder / MANIFEST_NAME).read_text())
    return _get_level_errors(folder, manifest, levels, schemas)


def _validate_map(folder: Path, schema: dict) -> list[str]:
    """`_validate_map` checks one map folder.

    Args:
        folder: The map's folder.
        schema: The manifest's JSON schema.

    Returns:
        One message per problem; empty if the map is valid.

    Example:
        >>> _validate_map(Path("lunar_surface"), json.loads(SCHEMA_PATH.read_text()))
        []
    """
    path: Path = folder / MANIFEST_NAME
    if not path.exists():
        return [f"{MANIFEST_NAME}: missing in {folder}"]
    manifest: dict = json.loads(path.read_text())
    schema_errors: list[str] = _get_schema_errors(manifest, schema)
    if schema_errors:
        return schema_errors
    errors: list[str] = []
    if manifest["name"] != folder.resolve().name:
        errors.append(f"name: {manifest['name']!r} doesn't match the folder {folder.resolve().name!r}")
    return (
        errors
        + _get_material_errors(manifest)
        + _get_shape_errors(manifest)
        + _get_mesh_errors(manifest)
        + _get_warp_errors(manifest)
        + _get_geometry_errors(manifest)
        + _get_edge_errors(manifest)
    )


@click.command(epilog="Example: uv run python validate_map.py house_demo lunar_surface")
@click.argument("folders", nargs=-1, required=True, type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.option("--json", "is_json", is_flag=True, help="Print the results as one JSON object.")
def main(folders: tuple[Path, ...], is_json: bool) -> None:
    """`main` validates each map folder, as a level if it has a `level.json`, and prints whether it is valid."""
    schema: dict = json.loads(SCHEMA_PATH.read_text())
    results: dict[str, list[str]] = {
        str(folder): _validate_level(folder, LEVELS_PATH)
        if (folder / LEVEL_NAME).exists()
        else _validate_map(folder, schema)
        for folder in folders
    }
    if is_json:
        click.echo(json.dumps({"maps": results, "is_valid": not any(results.values())}))
    else:
        for folder, errors in results.items():
            click.echo(f"{folder}: valid" if not errors else f"{folder}: {len(errors)} problems")
            for error in errors:
                click.echo(f"  {error}")
    if any(results.values()):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
