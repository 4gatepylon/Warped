//! `app_bundle.rs` finds the folders of files the game reads while it runs, the fps_demo's assets
//! and the maps' models: inside `Warped.app` when the game runs from it, else in the checkout.
//!
//! Interface: `ResourceFolder` (`get_path`), `ASSETS`, `MODELS`, `get_model_path`,
//! `get_app_resources_folder`, `get_level_slug`, `get_app_level_program`, `get_app_logs_folder`.
//!
//! Example: `let rifle: PathBuf = ASSETS.get_path().join("models/bolt_action_rifle_7_62");`
//!
//! Must know:
//! - `make package-mac-app` copies each folder into the app's `Contents/Resources/<name>/`, and
//!   `build.rs` names the same folders for the models built into the program.

use std::path::{Path, PathBuf};

const LOGS_HOME_PATH: &str = "Library/Logs/Warped";
const LEVELS_FOLDER_IN_APP: &str = "levels";

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

/// `get_level_slug` gives a level's name as a file name: lower case, spaces as `_`.
///
/// Example: `assert_eq!(get_level_slug("Snow Planet"), "snow_planet");`
pub fn get_level_slug(name: &str) -> String {
    name.to_lowercase().replace(' ', "_")
}

/// `get_app_level_program` gives the program of the level named `name` inside `Warped.app`
/// (`Contents/MacOS/levels/<slug>`, as `make package-mac-app` puts it), or `None` outside an app.
///
/// Example: `let program = get_app_level_program("Dust");`
pub fn get_app_level_program(name: &str) -> Option<PathBuf> {
    let resources: PathBuf = get_app_resources_folder()?;
    Some(
        resources
            .with_file_name("MacOS")
            .join(LEVELS_FOLDER_IN_APP)
            .join(get_level_slug(name)),
    )
}

/// `get_app_logs_folder` gives `~/Library/Logs/Warped/` when this program runs from `Warped.app`
/// (the checkout's `.artifacts/` isn't there on another Mac), or `None` outside an app or without
/// a home folder.
///
/// Example: `let folder = get_app_logs_folder().map(|logs| logs.join("serve_match"));`
pub fn get_app_logs_folder() -> Option<PathBuf> {
    get_app_resources_folder()?;
    let home: String = std::env::var("HOME").ok()?;
    Some(Path::new(&home).join(LOGS_HOME_PATH))
}

#[cfg(test)]
mod tests {
    //! Unit tests for `app_bundle.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Where the program runs | the checkout (tests): no app folders | `outside_an_app_there_are_no_app_folders` |
    //! | Level names | one word, several words, capitals | `level_slugs_are_file_names` |

    use super::*;

    #[test]
    fn outside_an_app_there_are_no_app_folders() {
        assert_eq!(get_app_resources_folder(), None);
        assert_eq!(get_app_level_program("Dust"), None);
        assert_eq!(get_app_logs_folder(), None);
    }

    #[test]
    fn level_slugs_are_file_names() {
        assert_eq!(get_level_slug("Dust"), "dust");
        assert_eq!(get_level_slug("Lunar Surface"), "lunar_surface");
        assert_eq!(get_level_slug("CITY Sphere"), "city_sphere");
    }
}
