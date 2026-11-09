//! `app_bundle.rs` finds the folders of files the game reads while it runs, the fps_demo's assets
//! and the maps' models: inside `Warped.app` when the game runs from it, else in the checkout.
//!
//! Interface: `ResourceFolder` (`get_path`), `ASSETS`, `MODELS`, `get_model_path`,
//! `get_app_resources_folder`.
//!
//! Example: `let rifle: PathBuf = ASSETS.get_path().join("models/bolt_action_rifle_7_62");`
//!
//! Must know:
//! - `make package-mac-app` copies each folder into the app's `Contents/Resources/<name>/`, and
//!   `build.rs` names the same folders for the models built into the program.

use std::path::{Path, PathBuf};

/// `ResourceFolder` is a folder the game reads files from: its `name` inside the app's
/// `Contents/Resources/`, and its `checkout_folder` relative to this crate's folder.
pub struct ResourceFolder {
    pub name: &'static str,
    checkout_folder: &'static str,
}

/// `ASSETS` is the fps_demo's assets: surface textures and gun models.
pub const ASSETS: ResourceFolder = ResourceFolder {
    name: "assets",
    checkout_folder: "../fps_demo/assets",
};

/// `MODELS` is the maps' models: the structures and the players' body.
pub const MODELS: ResourceFolder = ResourceFolder {
    name: "models",
    checkout_folder: "../../common/maps/models",
};

impl ResourceFolder {
    /// `get_path` gives the folder: inside the app when the game runs from one, else in the
    /// checkout.
    ///
    /// Example: `let models: PathBuf = MODELS.get_path();`
    pub fn get_path(&self) -> PathBuf {
        match get_app_resources_folder() {
            Some(resources) => resources.join(self.name),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).join(self.checkout_folder),
        }
    }
}

/// `get_model_path` gives the `scene.gltf` of the model folder `model` in `common/maps/models/`, or in
/// the app's copy of it when the game runs from `Warped.app`.
///
/// Example: `let path: PathBuf = get_model_path("hesco_barrier");`
pub fn get_model_path(model: &str) -> PathBuf {
    MODELS.get_path().join(model).join("scene.gltf")
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
