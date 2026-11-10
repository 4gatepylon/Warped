//! `hyperbolic-wormholes` runs Dust alone, in a window of its own: the program is the `dust_level`
//! library (`lib.rs`), which the one-window app plays too (W-141, one window for the whole game).
//!
//! Interface: the command line in `lib.rs`'s docstring; without arguments, the title screen.
//!
//! Example: `cargo run --release -- --map dust --join 192.168.1.5`

use std::process::ExitCode;

/// `main` runs the program as its command line asks (`dust_level::run_alone`).
///
/// Example: `hyperbolic-wormholes --bench-views --width-px 2560 --height-px 1600`
fn main() -> ExitCode {
    dust_level::run_alone()
}
