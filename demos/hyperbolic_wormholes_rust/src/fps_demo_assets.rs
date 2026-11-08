//! `fps_demo_assets.rs` finds the fps_demo's committed assets in `demos/fps_demo/assets/`, which
//! the surface textures and the revolver come from, or in the app's `Contents/Resources/assets/`
//! when the game runs from `Warped.app`, and fails with how to restore one that is missing.
//!
//! Interface: `get_app_resources_folder`, `get_asset_path`, `check_asset`.
//!
//! Example: `check_asset(&get_asset_path("textures/moon_01/moon_01_diff_2k.jpg"))?;`

use std::error::Error;
use std::path::{Path, PathBuf};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `get_fps_demo_folder` gives `demos/fps_demo/`.
///
/// Example: `let folder: PathBuf = get_fps_demo_folder();`
fn get_fps_demo_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the demo's folder sits in demos/")
        .join("fps_demo")
}

/// `get_app_resources_folder` gives the app's `Contents/Resources/` when this program runs from an
/// app bundle (`Contents/MacOS/`), or `None`.
///
/// Example: `let resources: Option<PathBuf> = get_app_resources_folder();`
pub fn get_app_resources_folder() -> Option<PathBuf> {
    let program: PathBuf = std::env::current_exe().ok()?;
    let folder: &Path = program.parent()?;
    (folder.file_name()? == "MacOS").then(|| folder.with_file_name("Resources"))
}

/// `get_asset_path` gives where `relative_path` sits in the fps_demo's `assets/`: the copy inside
/// the app when the game runs from one, else the checkout's.
///
/// Example: `let path: PathBuf = get_asset_path("sketchfab/sci_fi_revolver/scene.gltf");`
pub fn get_asset_path(relative_path: &str) -> PathBuf {
    let assets_folder: PathBuf = match get_app_resources_folder() {
        Some(resources) => resources.join("assets"),
        None => get_fps_demo_folder().join("assets"),
    };
    assets_folder.join(relative_path)
}

/// `check_asset` fails, naming `path` and the command that restores the assets, when `path` is
/// missing.
///
/// Example: `check_asset(&get_asset_path("sketchfab/sci_fi_revolver/scene.gltf"))?;`
pub fn check_asset(path: &Path) -> Result<()> {
    if path.exists() {
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
