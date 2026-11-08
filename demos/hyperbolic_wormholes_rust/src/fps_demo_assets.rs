//! `fps_demo_assets.rs` finds the fps_demo's committed assets in `demos/fps_demo/assets/`, which
//! the surface textures and the gun models come from, and fails with the command that restores one
//! that is missing.
//!
//! Interface: `get_asset_path`, `check_asset`.
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

/// `get_asset_path` gives where `relative_path` sits in the fps_demo's `assets/`.
///
/// Example: `let path: PathBuf = get_asset_path("sketchfab/sci_fi_revolver/scene.gltf");`
pub fn get_asset_path(relative_path: &str) -> PathBuf {
    get_fps_demo_folder().join("assets").join(relative_path)
}

/// `check_asset` fails, naming `path` and the command that restores the assets, when `path` is
/// missing.
///
/// Example: `check_asset(&get_asset_path("sketchfab/sci_fi_revolver/scene.gltf"))?;`
pub fn check_asset(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    Err(format!(
        "The game needs the fps_demo's assets, which are committed in demos/fps_demo/assets/, and {} is missing. Restore them with: cd {} && git checkout -- assets",
        path.display(),
        get_fps_demo_folder().display()
    )
    .into())
}
