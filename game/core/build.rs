//! `build.rs` builds into the game the models whose licenses forbid shipping them as files others
//! could copy out (the Sketchfab Standard license): it writes `$OUT_DIR/embedded_assets.rs`, a
//! table of `include_bytes!` that `fps_demo_assets.rs` reads instead of those files.
//!
//! Interface: `EMBEDDED_FOLDERS`, each a key prefix (`<resource folder>/<path>`, as
//! `app_bundle.rs` names the folders) and that folder relative to this crate.
//!
//! Example: `cargo build --release` runs it.
//!
//! Must know:
//! - `make package-mac-app` leaves these folders out of the app, and the CREDITS page names them.
//! - A license file in a folder isn't built in.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const EMBEDDED_FOLDERS: [(&str, &str); 2] = [
    (
        "assets/sketchfab/sci_fi_revolver",
        "../../demos/fps_demo/assets/sketchfab/sci_fi_revolver",
    ),
    ("models/teal_robot", "../../common/maps/models/teal_robot"),
];
const LICENSE_FILE: &str = "license.txt";

/// `list_files` gives every file under `folder`, sorted, but its license file.
///
/// Example: `let files: Vec<PathBuf> = list_files(Path::new("../../common/maps/models/teal_robot"));`
fn list_files(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    let entries = fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("build.rs: can't read {}: {error}", folder.display()));
    for entry in entries {
        let path: PathBuf = entry.expect("a folder entry").path();
        if path.is_dir() {
            files.extend(list_files(&path));
        } else if path.file_name().is_some_and(|name| name != LICENSE_FILE) {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// `main` writes the table of built-in files and asks Cargo to rerun when those folders change.
///
/// Example: `main()`
fn main() {
    let crate_folder: PathBuf =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let mut entries: String = String::new();
    for (key_prefix, relative_folder) in EMBEDDED_FOLDERS {
        let folder: PathBuf = crate_folder.join(relative_folder);
        println!("cargo:rerun-if-changed={}", folder.display());
        for path in list_files(&folder) {
            let relative: &Path = path.strip_prefix(&folder).expect("a file under its folder");
            let key: String = format!("{key_prefix}/{}", relative.display());
            entries.push_str(&format!(
                "    ({key:?}, include_bytes!({:?})),\n",
                path.display().to_string()
            ));
        }
    }
    let table: String = format!(
        "/// `EMBEDDED_ASSETS` is every file built into the game, by `<resource folder>/<path>`.\n\
         pub const EMBEDDED_ASSETS: &[(&str, &[u8])] = &[\n{entries}];\n"
    );
    let out_folder: PathBuf = PathBuf::from(env::var("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(out_folder.join("embedded_assets.rs"), table).expect("the table is written");
}
