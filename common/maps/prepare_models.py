"""`prepare_models.py` downloads the Sketchfab models the maps use and writes small copies into `models/<name>/`.

Each model is downloaded with a Sketchfab API token into `.build/sketchfab/<name>/` (skipped when already there),
then copied to `models/<name>/` with its geometry unchanged but for parts left out (the garage's closed doors, so
players can go in), its textures shrunk to at most `--max-texture-px` on a
side (JPEG unless the texture's alpha is used), spec-gloss materials turned into metallic-roughness ones, and its
`license.txt`. A model in `_ANIMATION_SCRIPTS` first goes through its Blender script (`teal_robot`, the players' body,
gets a clean rig and its clips from `animate_teal_blender.py`), which writes into `.build/animated/<name>/`.
`models/` is committed, so players don't need a token; rerun this to change the models.

Interface:
    --max-texture-px: The longest texture side to keep (default 1024).
    --blender-path: The Blender executable that runs the animation scripts.
    --dry-run: Print what would be downloaded and written, and change nothing.
    SKETCHFAB_API_TOKEN: The Sketchfab API token, needed only for models not yet in `.build/sketchfab/`.

Example:
    $ SKETCHFAB_API_TOKEN=... uv run python prepare_models.py

Must know:
    Side effects:
        - `prepare_models.py` replaces each model's folder in `models/`, which it generated.
"""

import io
import json
import os
import shutil
import subprocess
import urllib.request
import zipfile
from pathlib import Path

import click
from PIL import Image

_MAPS_FOLDER: Path = Path(__file__).resolve().parent
_DOWNLOAD_FOLDER: Path = _MAPS_FOLDER / ".build" / "sketchfab"
_MODELS_FOLDER: Path = _MAPS_FOLDER / "models"
_ANIMATED_FOLDER: Path = _MAPS_FOLDER / ".build" / "animated"
_DEFAULT_BLENDER_PATH: str = "/Applications/Blender.app/Contents/MacOS/Blender"
_DOWNLOAD_API_URL: str = "https://api.sketchfab.com/v3/models/{uid}/download"
_TOKEN_ENV: str = "SKETCHFAB_API_TOKEN"
_JPEG_QUALITY: int = 90
_DEFAULT_MAX_TEXTURE_PX: int = 1024
_OPAQUE_ALPHA: int = 255
_SPEC_GLOSS: str = "KHR_materials_pbrSpecularGlossiness"
_REMOVED_NODE_NAMES: dict[str, list[str]] = {"desert_bunker_garage": ["GarageDoor_windows and metal_0"]}
MODELS: dict[str, str] = {
    "stekelvarken_kazemat": "f409a77deaaf4e3888d47f76ec48dbdd",
    "desert_bunker_garage": "ebe3177a1c7947b98328d8cab6aa8600",
    "ww2_field_bunker": "6ea5cebdaa82455aa3aad26baa6e0375",
    "scifi_beacon": "a1d91ebb3e2d41bba31c02b11423d97f",
    "hesco_barrier": "ab21217481094e9a84f989e71fd56a59",
    "teal_robot": "d6a54ff9468c451bbdbd879afa03f20a",
}
_ANIMATION_SCRIPTS: dict[str, str] = {"teal_robot": "animate_teal_blender.py"}


def _download_model(name: str, uid: str, token: str) -> None:
    """`_download_model` downloads one model's glTF archive and unzips it into `.build/sketchfab/<name>/`.

    Args:
        name: The model's folder name.
        uid: The model's Sketchfab id.
        token: The Sketchfab API token.

    Example:
        >>> _download_model("hesco_barrier", MODELS["hesco_barrier"], token)
    """
    request = urllib.request.Request(_DOWNLOAD_API_URL.format(uid=uid), headers={"Authorization": f"Token {token}"})
    with urllib.request.urlopen(request) as response:
        archive_url: str = json.load(response)["gltf"]["url"]
    with urllib.request.urlopen(archive_url) as response:
        archive_bytes: bytes = response.read()
    temp_folder: Path = _DOWNLOAD_FOLDER / f"{name}.part"
    shutil.rmtree(temp_folder, ignore_errors=True)
    zipfile.ZipFile(io.BytesIO(archive_bytes)).extractall(temp_folder)
    temp_folder.rename(_DOWNLOAD_FOLDER / name)


def _convert_spec_gloss(gltf: dict, source: Path, target: Path, max_texture_px: int) -> None:
    """`_convert_spec_gloss` turns spec-gloss materials into metallic-roughness ones, in place in `gltf`.

    The diffuse map becomes the base colour, and a new roughness map is 1 - glossiness (the spec-gloss map's alpha),
    with no metal.

    Args:
        gltf: The parsed glTF, whose materials and images change.
        source: The downloaded model's folder.
        target: The folder the new roughness maps go in.
        max_texture_px: The longest texture side to keep.

    Example:
        >>> _convert_spec_gloss(gltf, Path(".build/sketchfab/stekelvarken_kazemat"), target, 1024)
    """
    for material in gltf["materials"]:
        if "extensions" not in material or _SPEC_GLOSS not in material["extensions"]:
            continue
        spec_gloss: dict = material["extensions"].pop(_SPEC_GLOSS)
        glossiness: float = spec_gloss["glossinessFactor"] if "glossinessFactor" in spec_gloss else 1.0
        pbr: dict = {"metallicFactor": 0.0, "roughnessFactor": 1.0 - glossiness}
        if "diffuseFactor" in spec_gloss:
            pbr["baseColorFactor"] = spec_gloss["diffuseFactor"]
        if "diffuseTexture" in spec_gloss:
            pbr["baseColorTexture"] = spec_gloss["diffuseTexture"]
        if "specularGlossinessTexture" in spec_gloss:
            texture: dict = gltf["textures"][spec_gloss["specularGlossinessTexture"]["index"]]
            gloss: Image.Image = Image.open(source / gltf["images"][texture["source"]]["uri"]).convert("RGBA")
            roughness: Image.Image = Image.eval(gloss.getchannel("A"), lambda a: _OPAQUE_ALPHA - a)
            black: Image.Image = Image.new("L", gloss.size, 0)
            temp_uri: str = f"textures/{material['name']}_metallicRoughness.jpeg"
            image: Image.Image = Image.merge("RGB", (black, roughness, black))
            image.thumbnail((max_texture_px, max_texture_px))
            image.save(target / temp_uri, quality=_JPEG_QUALITY)
            gltf["images"].append({"uri": temp_uri, "mimeType": "image/jpeg"})
            gltf["textures"].append({"source": len(gltf["images"]) - 1})
            pbr["roughnessFactor"] = 1.0
            pbr["metallicRoughnessTexture"] = {"index": len(gltf["textures"]) - 1}
        material["pbrMetallicRoughness"] = pbr
        if not material["extensions"]:
            del material["extensions"]
    for key in ("extensionsUsed", "extensionsRequired"):
        if key in gltf:
            gltf[key] = [extension for extension in gltf[key] if extension != _SPEC_GLOSS]


def _shrink_images(gltf: dict, source: Path, target: Path, max_texture_px: int) -> None:
    """`_shrink_images` writes each of the downloaded model's images shrunk into `target`, as JPEG unless it has alpha.

    Args:
        gltf: The parsed glTF, whose image URIs change to the written files.
        source: The downloaded model's folder.
        target: The folder the images go in.
        max_texture_px: The longest texture side to keep.

    Example:
        >>> _shrink_images(gltf, source, target, 1024)
    """
    for image_entry in gltf["images"]:
        if (target / image_entry["uri"]).exists():
            continue
        image: Image.Image = Image.open(source / image_entry["uri"])
        image.thumbnail((max_texture_px, max_texture_px))
        has_alpha: bool = image.mode in ("RGBA", "LA") and image.getchannel("A").getextrema()[0] < _OPAQUE_ALPHA
        uri: Path = Path(image_entry["uri"]).with_suffix(".png" if has_alpha else ".jpeg")
        if has_alpha:
            image.save(target / uri)
        else:
            image.convert("RGB").save(target / uri, quality=_JPEG_QUALITY)
        image_entry["uri"] = uri.as_posix()
        image_entry["mimeType"] = "image/png" if has_alpha else "image/jpeg"


def _animate_model(name: str, blender_path: Path) -> None:
    """`_animate_model` runs model `name`'s Blender script on its download, writing `.build/animated/<name>/`.

    Args:
        name: The model's folder name, a key of `_ANIMATION_SCRIPTS`.
        blender_path: The Blender executable.

    Example:
        >>> _animate_model("teal_robot", Path(_DEFAULT_BLENDER_PATH))
    """
    target: Path = _ANIMATED_FOLDER / name
    shutil.rmtree(target, ignore_errors=True)
    script: Path = _MAPS_FOLDER / _ANIMATION_SCRIPTS[name]
    command: list[str] = [str(blender_path), "-b", "--factory-startup", "-P", str(script), "--"]
    subprocess.run([*command, str(_DOWNLOAD_FOLDER / name / "scene.gltf"), str(target)], check=True)
    shutil.copy(_DOWNLOAD_FOLDER / name / "license.txt", target / "license.txt")


def _write_model(name: str, max_texture_px: int) -> None:
    """`_write_model` writes `models/<name>/` from the downloaded model, without its nodes in `_REMOVED_NODE_NAMES`.

    Args:
        name: The model's folder name.
        max_texture_px: The longest texture side to keep.

    Example:
        >>> _write_model("hesco_barrier", 1024)
    """
    source: Path = (_ANIMATED_FOLDER if name in _ANIMATION_SCRIPTS else _DOWNLOAD_FOLDER) / name
    target: Path = _MODELS_FOLDER / name
    shutil.rmtree(target, ignore_errors=True)
    (target / "textures").mkdir(parents=True)
    gltf: dict = json.loads((source / "scene.gltf").read_text())
    for node in gltf["nodes"]:
        if name in _REMOVED_NODE_NAMES and node["name"] in _REMOVED_NODE_NAMES[name]:
            del node["mesh"]
    _convert_spec_gloss(gltf, source, target, max_texture_px)
    _shrink_images(gltf, source, target, max_texture_px)
    (target / "scene.gltf").write_text(json.dumps(gltf, indent=1))
    shutil.copy(source / "scene.bin", target / "scene.bin")
    shutil.copy(source / "license.txt", target / "license.txt")


@click.command(epilog="Example: SKETCHFAB_API_TOKEN=... uv run python prepare_models.py --dry-run")
@click.option(
    "--max-texture-px",
    type=click.IntRange(min=1),
    default=_DEFAULT_MAX_TEXTURE_PX,
    show_default=True,
    help="The longest texture side to keep.",
)
@click.option(
    "--blender-path",
    type=click.Path(exists=True, dir_okay=False, path_type=Path),
    default=_DEFAULT_BLENDER_PATH,
    show_default=True,
    help="The Blender executable that runs the animation scripts.",
)
@click.option("--dry-run", is_flag=True, help="Print what would be downloaded and written, and change nothing.")
def main(max_texture_px: int, blender_path: Path, dry_run: bool) -> None:
    """`main` downloads the missing models, then writes every model's small copy into `models/`.

    Args:
        max_texture_px: The longest texture side to keep.
        blender_path: The Blender executable that runs the animation scripts.
        dry_run: Whether to only print the plan.

    Example:
        >>> main(["--dry-run"])
    """
    missing_names: list[str] = [name for name in MODELS if not (_DOWNLOAD_FOLDER / name / "scene.gltf").exists()]
    if missing_names and _TOKEN_ENV not in os.environ:
        raise click.UsageError(
            f"{_TOKEN_ENV} is not set, and these models aren't downloaded yet: "
            f"{', '.join(missing_names)}. Example: {_TOKEN_ENV}=... uv run python prepare_models.py"
        )
    for name in missing_names:
        click.echo(f"download {name} into {_DOWNLOAD_FOLDER / name}", err=True)
    for name in _ANIMATION_SCRIPTS:
        click.echo(f"animate {name} into {_ANIMATED_FOLDER / name} with {blender_path}", err=True)
    for name in MODELS:
        click.echo(f"write {_MODELS_FOLDER / name}", err=True)
    if dry_run:
        return
    _DOWNLOAD_FOLDER.mkdir(parents=True, exist_ok=True)
    for name in missing_names:
        _download_model(name, MODELS[name], os.environ[_TOKEN_ENV])
    for name in _ANIMATION_SCRIPTS:
        _animate_model(name, blender_path)
    for name in MODELS:
        _write_model(name, max_texture_px)
    click.echo(_MODELS_FOLDER)


if __name__ == "__main__":
    main()
