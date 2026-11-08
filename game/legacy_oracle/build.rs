//! `build.rs` writes `$OUT_DIR/embedded_assets.rs` for the demo's `fps_demo_assets.rs`: an empty
//! table of files built into the program, so the oracle reads every model from disk.
//!
//! Interface: `EMBEDDED_ASSETS`, the table the demo's `build.rs` fills for the demo itself.
//!
//! Example: `cargo build -p legacy-oracle` runs it.
//!
//! Must know:
//! - The demo's `build.rs` can't run here: it finds the fps_demo's assets relative to the demo's
//!   folder, and the oracle simulates without them.

use std::env;
use std::fs;
use std::path::PathBuf;

const EMPTY_TABLE: &str = "/// `EMBEDDED_ASSETS` is empty in the oracle.\n\
                           pub const EMBEDDED_ASSETS: &[(&str, &[u8])] = &[];\n";

/// `main` writes the empty table.
///
/// Example: `main()`
fn main() {
    let out_folder: PathBuf = PathBuf::from(env::var("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(out_folder.join("embedded_assets.rs"), EMPTY_TABLE).expect("the table is written");
}
