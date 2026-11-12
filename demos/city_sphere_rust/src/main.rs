//! `city-sphere` runs City Sphere alone, in a window of its own: the program is the `city_sphere_level`
//! library (`lib.rs`), which the one-window app plays too (W-141, one window for the whole game).
//!
//! Interface: the command line in `lib.rs`'s docstring; without arguments, the main menu.
//!
//! Example: `cargo run --release -- --map city-sphere --join 192.168.1.5`

use std::process::ExitCode;

/// `main` runs the program as its command line asks (`city_sphere_level::run_alone`).
///
/// Example: `city-sphere --bench-views --width-px 2560 --height-px 1600`
fn main() -> ExitCode {
    city_sphere_level::run_alone()
}
