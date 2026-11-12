//! `app_bundle.rs` finds the folders of files the game reads while it runs, the fps_demo's assets,
//! the maps' models and each level's own files: inside `Warped.app` when the game runs from it,
//! else in the checkout.
//!
//! Interface: `ResourceFolder` (`get_path`), `ASSETS`, `MODELS`, `get_model_path`,
//! `get_app_resources_folder`, `get_level_folder`, `get_level_slug`, `get_app_level_program`,
//! `get_app_logs_folder`.
//!
//! Example: `let rifle: PathBuf = ASSETS.get_path().join("models/bolt_action_rifle_7_62");`
//!
//! Must know:
//! - `make package-mac-app` copies each folder into the app's `Contents/Resources/<name>/`, and
//!   `build.rs` names the same folders for the models built into the program.
//! - The app's programs are `Contents/MacOS/<program>` and, for the levels,
//!   `Contents/MacOS/levels/<slug>`. The app's own program draws a level too, so a level's own
//!   files are found by its crate folder's name, not by the program: they are in
//!   `Contents/Resources/repository/demos/<its crate's folder>/`, laid out as in the repository, so
//!   a path from a level's crate folder to the repository's root still works there.

use std::path::{Path, PathBuf};

const LOGS_HOME_PATH: &str = "Library/Logs/Warped";
const LEVELS_FOLDER_IN_APP: &str = "levels";
const PROGRAMS_FOLDER_IN_APP: &str = "MacOS";
const RESOURCES_FOLDER_IN_APP: &str = "Resources";
const REPOSITORY_FOLDER_IN_APP: &str = "repository";
const DEMOS_FOLDER: &str = "demos";

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
    find_resources_folder(&std::env::current_exe().ok()?)
}

/// `find_resources_folder` gives the `Contents/Resources/` of the app that `program` is in, as
/// `Contents/MacOS/<program>` or as a level, `Contents/MacOS/levels/<slug>`; or `None`.
///
/// Example: `let resources = find_resources_folder(Path::new("/Warped.app/Contents/MacOS/levels/dust"));`
fn find_resources_folder(program: &Path) -> Option<PathBuf> {
    let folder: &Path = program.parent()?;
    let programs_folder: &Path = if folder.file_name()? == LEVELS_FOLDER_IN_APP {
        folder.parent()?
    } else {
        folder
    };
    (programs_folder.file_name()? == PROGRAMS_FOLDER_IN_APP)
        .then(|| programs_folder.with_file_name(RESOURCES_FOLDER_IN_APP))
}

/// `get_level_folder` gives the folder a level reads its own files from (its textures, its map
/// picture): `crate_folder`, the level's `env!("CARGO_MANIFEST_DIR")`, in the checkout, and the
/// app's copy of it when the level runs from `Warped.app`, where the checkout isn't.
///
/// Example: `let picture: PathBuf = get_level_folder(env!("CARGO_MANIFEST_DIR")).join("assets/map_picture.png");`
pub fn get_level_folder(crate_folder: &str) -> PathBuf {
    let crate_folder: &Path = Path::new(crate_folder);
    std::env::current_exe()
        .ok()
        .and_then(|program| find_app_level_folder(&program, crate_folder))
        .unwrap_or_else(|| crate_folder.to_path_buf())
}

/// `find_app_level_folder` gives the copy of the level's `crate_folder` in the app that `program`
/// is in, `Contents/Resources/repository/demos/<crate folder's name>/`; or `None` outside an app.
///
/// Example: `let folder = find_app_level_folder(&program, Path::new("/repo/demos/dust_rust"));`
fn find_app_level_folder(program: &Path, crate_folder: &Path) -> Option<PathBuf> {
    Some(
        find_resources_folder(program)?
            .join(REPOSITORY_FOLDER_IN_APP)
            .join(DEMOS_FOLDER)
            .join(crate_folder.file_name()?),
    )
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
            .with_file_name(PROGRAMS_FOLDER_IN_APP)
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
    //! | Where a program sits | the app's programs folder; its levels folder; a levels folder outside an app; a build folder | `the_apps_programs_and_levels_find_its_resources` |
    //! | Where a level's own files are | the app's program and a level's program in the app: the app's copy; a checkout: the crate's folder | `a_level_in_the_app_reads_the_apps_copy_of_its_files` |
    //! | Level names | one word, several words, capitals | `level_slugs_are_file_names` |

    use super::*;

    #[test]
    fn outside_an_app_there_are_no_app_folders() {
        assert_eq!(get_app_resources_folder(), None);
        assert_eq!(get_app_level_program("Dust"), None);
        assert_eq!(get_app_logs_folder(), None);
    }

    #[test]
    fn the_apps_programs_and_levels_find_its_resources() {
        let resources: Option<PathBuf> = Some(PathBuf::from("/Warped.app/Contents/Resources"));
        assert_eq!(
            find_resources_folder(Path::new("/Warped.app/Contents/MacOS/Warped")),
            resources
        );
        assert_eq!(
            find_resources_folder(Path::new("/Warped.app/Contents/MacOS/levels/dust")),
            resources
        );
        assert_eq!(find_resources_folder(Path::new("/games/levels/dust")), None);
        assert_eq!(
            find_resources_folder(Path::new("/repo/demos/dust_rust/.build/release/dust")),
            None
        );
    }

    #[test]
    fn a_level_in_the_app_reads_the_apps_copy_of_its_files() {
        let crate_folder: &Path = Path::new("/repo/demos/dust_rust");
        let in_app: Option<PathBuf> = Some(PathBuf::from(
            "/Warped.app/Contents/Resources/repository/demos/dust_rust",
        ));
        assert_eq!(
            find_app_level_folder(Path::new("/Warped.app/Contents/MacOS/Warped"), crate_folder),
            in_app
        );
        assert_eq!(
            find_app_level_folder(
                Path::new("/Warped.app/Contents/MacOS/levels/dust"),
                crate_folder
            ),
            in_app
        );
        assert_eq!(
            find_app_level_folder(
                Path::new("/repo/demos/dust_rust/.build/release/dust"),
                crate_folder
            ),
            None
        );
        assert_eq!(
            get_level_folder("/repo/demos/dust_rust"),
            PathBuf::from("/repo/demos/dust_rust")
        );
    }

    #[test]
    fn level_slugs_are_file_names() {
        assert_eq!(get_level_slug("Dust"), "dust");
        assert_eq!(get_level_slug("Lunar Surface"), "lunar_surface");
        assert_eq!(get_level_slug("CITY Sphere"), "city_sphere");
    }
}
