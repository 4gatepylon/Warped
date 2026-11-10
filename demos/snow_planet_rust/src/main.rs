//! `snow-planet` runs Snow Planet alone, in a window of its own: the program is the `snow_planet_level`
//! library (`lib.rs`), which the one-window app plays too (W-141, one window for the whole game).
//!
//! Interface: the command line in `lib.rs`'s docstring; without arguments, the title screen.
//!
//! Example: `cargo run --release -- --map snow-planet --join 192.168.1.5`

use std::process::ExitCode;

/// `main` runs the program as its command line asks (`snow_planet_level::run_alone`).
///
/// Example: `snow-planet --bench-views --width-px 2560 --height-px 1600`
fn main() -> ExitCode {
    snow_planet_level::run_alone()
}
