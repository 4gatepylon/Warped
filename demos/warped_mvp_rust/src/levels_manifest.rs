//! `levels_manifest.rs` is the app's one door to the levels manifest, the file listing the
//! game's levels (name, program, map, map picture), so the app never names a level itself. Today
//! the manifest is the match's `match_levels.yaml`; when the declarative level definitions land
//! (`claude/level-definitions`), only this file changes.
//!
//! Interface: `get_levels_manifest_path`, `load_levels`, `load_map_pictures`, `Picture`.
//!
//! Example: `let levels = load_levels(&get_levels_manifest_path())?;`
//!
//! Must know:
//! - Gotchas: a level's `map_picture` is a path from the manifest's folder; a picture that isn't
//!   there (a level not built yet) is `None`, drawn as an empty box.

use std::error::Error;
use std::path::{Path, PathBuf};

use warped_shared::app_bundle::get_app_resources_folder;
use warped_shared::match_protocol::{LevelEntry, LoadLevels};

/// `MANIFEST_NAME` is the manifest's file name, in the app's `Contents/Resources/` or in the
/// match's folder of the checkout (`CHECKOUT_MANIFEST_FOLDER`, from this crate's folder).
const MANIFEST_NAME: &str = "match_levels.yaml";
const CHECKOUT_MANIFEST_FOLDER: &str = "../warped_match_rust";

/// `Picture` is an RGBA picture (rows top first) and its size in pixels.
pub type Picture = (Vec<u8>, (u32, u32));

/// `get_levels_manifest_path` gives the manifest: inside `Warped.app` when the app runs from
/// one, else the checkout's.
///
/// Example: `let path = get_levels_manifest_path();`
pub fn get_levels_manifest_path() -> PathBuf {
    match get_app_resources_folder() {
        Some(resources) => resources.join(MANIFEST_NAME),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(CHECKOUT_MANIFEST_FOLDER)
            .join(MANIFEST_NAME),
    }
}

/// `load_levels` reads the levels from the manifest at `path`, in their order on the map (blue's
/// deepest first).
///
/// Example: `let levels = load_levels(&path)?;`
pub fn load_levels(path: &Path) -> Result<Vec<LevelEntry>, Box<dyn Error>> {
    LoadLevels(path)
}

/// `find_picture_in_checkout` gives a level's map picture in its own crate folder when the
/// manifest's path (`relative`, through the match's `.levels/` links, which only `make levels`
/// makes) isn't there: the part of `relative` after the level's `crate_folder`, under the checkout
/// (two folders up from the manifest's `folder`); `None` if that isn't there either.
///
/// Example: `let path = find_picture_in_checkout(folder, "demos/dust_rust", ".levels/dust/demos/dust_rust/map_pictures/dust_r50m.png");`
fn find_picture_in_checkout(folder: &Path, crate_folder: &str, relative: &str) -> Option<PathBuf> {
    let (_, inside_crate) = relative.split_once(&format!("{crate_folder}/"))?;
    let path: PathBuf = folder.join("../..").join(crate_folder).join(inside_crate);
    path.exists().then_some(path)
}

/// `load_map_pictures` gives each level's map picture, read from the manifest at
/// `manifest_path`'s folder (or from the level's crate folder, `find_picture_in_checkout`), or
/// `None` for a level without one or whose file isn't there.
///
/// Example: `let pictures = load_map_pictures(&path, &levels)?;`
pub fn load_map_pictures(
    manifest_path: &Path,
    levels: &[LevelEntry],
) -> Result<Vec<Option<Picture>>, Box<dyn Error>> {
    let folder: &Path = manifest_path
        .parent()
        .ok_or_else(|| format!("{} has no folder", manifest_path.display()))?;
    levels
        .iter()
        .map(|level| {
            let Some(relative) = &level.map_picture else {
                return Ok(None);
            };
            let path: PathBuf = match folder.join(relative) {
                path if path.exists() => path,
                _ => match find_picture_in_checkout(folder, &level.folder, relative) {
                    Some(path) => path,
                    None => return Ok(None),
                },
            };
            let image: image::RgbaImage = image::open(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?
                .to_rgba8();
            let size_px: (u32, u32) = image.dimensions();
            Ok(Some((image.into_raw(), size_px)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Unit tests for `levels_manifest.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Manifest | the checkout's (five levels, each named once) | `the_checkout_manifest_loads` |
    //! | Pictures | a level with a map picture (found in its crate folder without `.levels/`), one without | `the_checkout_manifest_loads` |

    use super::*;

    #[test]
    fn the_checkout_manifest_loads() {
        let path: PathBuf = get_levels_manifest_path();
        let levels: Vec<LevelEntry> = load_levels(&path).unwrap();
        assert_eq!(levels.len(), 5);
        let mut names: Vec<&str> = levels.iter().map(|level| level.name.as_str()).collect();
        names.dedup();
        assert_eq!(names.len(), levels.len());
        let pictures: Vec<Option<Picture>> = load_map_pictures(&path, &levels).unwrap();
        assert_eq!(pictures.len(), levels.len());
        for (level, picture) in levels.iter().zip(&pictures) {
            assert_eq!(picture.is_some(), level.map_picture.is_some(), "{}", level.name);
        }
    }
}
