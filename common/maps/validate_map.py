"""`validate_map.py` checks that map folders follow the map format of `common/maps/README.md`.

It checks `map.json` against `map.schema.json`, then what the schema can't say: the name matches the folder,
material ids are unique with 0 (and only 0) empty, every material used exists, frames are isometries of the map's
space, points lie in the space, shapes' rounding fits inside them, wormhole mouths pair up, tapered warps'
reach spheres don't overlap, directions are unit vectors, the Milky Way's core is perpendicular to its pole, and
only a map without terrain has a soft edge. It prints one line per map (or `--json`) and exits 1 if any map is
invalid.

Interface:
    uv run python validate_map.py <map folder>... [--json]

Example:
    uv run python validate_map.py house_demo lunar_surface
"""

import json
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
    schema_errors: list[str] = [
        f"{'/'.join(str(p) for p in error.absolute_path) or 'map'}: {error.message}"
        for error in jsonschema.Draft202012Validator(schema).iter_errors(manifest)
    ]
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
    """`main` validates each map folder and prints whether it follows the map format."""
    schema: dict = json.loads(SCHEMA_PATH.read_text())
    results: dict[str, list[str]] = {str(folder): _validate_map(folder, schema) for folder in folders}
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
