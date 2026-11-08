//! `fps_demo_assets.rs` finds the fps_demo's committed assets in `demos/fps_demo/assets/`, which
//! the surface textures and the gun models come from, and fails with the command that restores one
//! that is missing. It also reads every asset file the game loads (`read_asset_file`), the maps'
//! models too, from the copy built into the program for the models whose licenses ask for that
//! (`build.rs`), else from disk.
//!
//! Interface: `get_asset_path`, `check_asset`, `read_asset_file`, `import_gltf`.
//!
//! Example: `let (document, buffers) = import_gltf(&get_asset_path("sketchfab/sci_fi_revolver/scene.gltf"))?;`
//!
//! Must know:
//! - Inside `Warped.app` the folders are the app's own (`app_bundle.rs`), so the game runs without
//!   the repository.

use std::borrow::Cow;
use std::error::Error;
use std::path::{Path, PathBuf};

use crate::app_bundle::{ASSETS, MODELS, ResourceFolder, get_app_resources_folder};

include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BUFFER_ALIGNMENT_BYTES: usize = 4;

/// `get_fps_demo_folder` gives `demos/fps_demo/`.
///
/// Example: `let folder: PathBuf = get_fps_demo_folder();`
fn get_fps_demo_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the demo's folder sits in demos/")
        .join("fps_demo")
}

/// `get_asset_path` gives where `relative_path` sits in the fps_demo's `assets/`: the copy inside
/// the app when the game runs from one, else the checkout's.
///
/// Example: `let path: PathBuf = get_asset_path("sketchfab/sci_fi_revolver/scene.gltf");`
pub fn get_asset_path(relative_path: &str) -> PathBuf {
    ASSETS.get_path().join(relative_path)
}

/// `find_embedded` gives the bytes of `path` when it is a file built into the program, keyed by
/// its resource folder's name and its path in it, or `None`.
///
/// Example: `let bytes: Option<&[u8]> = find_embedded(&get_asset_path("sketchfab/sci_fi_revolver/scene.gltf"));`
fn find_embedded(path: &Path) -> Option<&'static [u8]> {
    let folders: [&ResourceFolder; 2] = [&ASSETS, &MODELS];
    let key: String = folders.iter().find_map(|folder| {
        let relative: &Path = path.strip_prefix(folder.get_path()).ok()?;
        Some(format!("{}/{}", folder.name, relative.display()))
    })?;
    EMBEDDED_ASSETS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|&(_, bytes)| bytes)
}

/// `read_asset_file` gives the bytes of the asset file at `path`: the copy built into the program,
/// or else the file on disk; it fails naming the path.
///
/// Example: `let bytes = read_asset_file(&get_asset_path("textures/moon_01/moon_01_diff_2k.jpg"))?;`
pub fn read_asset_file(path: &Path) -> std::result::Result<Cow<'static, [u8]>, String> {
    if let Some(bytes) = find_embedded(path) {
        return Ok(Cow::Borrowed(bytes));
    }
    std::fs::read(path)
        .map(Cow::Owned)
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// `import_gltf` reads the glTF at `path` and its buffers through `read_asset_file`, like
/// `gltf::import` without the images (loaders read those themselves, also through it).
///
/// Example: `let (document, buffers) = import_gltf(&get_asset_path("sketchfab/sci_fi_revolver/scene.gltf"))?;`
pub fn import_gltf(
    path: &Path,
) -> std::result::Result<(gltf::Document, Vec<gltf::buffer::Data>), String> {
    let folder: &Path = path.parent().expect("a file has a folder");
    let gltf::Gltf { document, mut blob } = gltf::Gltf::from_slice(&read_asset_file(path)?)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let buffers: Vec<gltf::buffer::Data> = document
        .buffers()
        .map(|buffer| {
            let mut data: Vec<u8> = match buffer.source() {
                gltf::buffer::Source::Uri(uri) => read_asset_file(&folder.join(uri))?.into_owned(),
                gltf::buffer::Source::Bin => blob
                    .take()
                    .ok_or_else(|| format!("{}: no binary chunk", path.display()))?,
            };
            data.resize(data.len().next_multiple_of(BUFFER_ALIGNMENT_BYTES), 0);
            Ok(gltf::buffer::Data(data))
        })
        .collect::<std::result::Result<_, String>>()?;
    Ok((document, buffers))
}

/// `check_asset` fails, naming `path` and how to restore it, when `path` is neither on disk nor
/// built into the program.
///
/// Example: `check_asset(&get_asset_path("sketchfab/sci_fi_revolver/scene.gltf"))?;`
pub fn check_asset(path: &Path) -> Result<()> {
    if path.exists() || find_embedded(path).is_some() {
        return Ok(());
    }
    if get_app_resources_folder().is_some() {
        return Err(format!(
            "Warped.app is missing {}; download and install the app again.",
            path.display()
        )
        .into());
    }
    Err(format!(
        "The game needs the fps_demo's assets, which are committed in demos/fps_demo/assets/, and {} is missing. Restore them with: cd {} && git checkout -- assets",
        path.display(),
        get_fps_demo_folder().display()
    )
    .into())
}
