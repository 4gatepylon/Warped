//! `level_library.rs` finds the code that plays a level of the levels manifest in the app's own
//! window: each level crate is a library linked into `warped`, and the manifest names the crate by
//! its folder (`LevelEntry::folder`), so a level's entry point (`app::play_level_fn_t`) is found
//! from the manifest, not from its name.
//!
//! Interface: `get_play_level`.
//!
//! Example: `if let Some(play) = get_play_level(&level) { let end = play(&mut window, &join)?; }`
//!
//! Must know:
//! - Gotchas: a new level crate needs its library added to `Cargo.toml` and a row in
//!   `LEVEL_LIBRARIES`; a level without one plays the stand-in screen.

use warped_shared::app::play_level_fn_t;
use warped_shared::match_protocol::LevelEntry;

/// `LEVEL_LIBRARIES` is each level crate's folder (as the manifest names it) and its entry point.
const LEVEL_LIBRARIES: [(&str, play_level_fn_t); 5] = [
    (
        "demos/hyperbolic_wormholes_rust",
        lunar_surface_level::play_level,
    ),
    ("demos/dust_rust", dust_level::play_level),
    ("demos/snow_planet_rust", snow_planet_level::play_level),
    ("demos/city_sphere_rust", city_sphere_level::play_level),
    ("demos/dark_forest_rust", dark_forest_level::play_level),
];

/// `get_play_level` gives the entry point of `level`'s library, or `None` if the app has none
/// for its folder.
///
/// Example: `let play = get_play_level(&levels[0]);`
pub fn get_play_level(level: &LevelEntry) -> Option<play_level_fn_t> {
    LEVEL_LIBRARIES
        .iter()
        .find(|(folder, _)| *folder == level.folder)
        .map(|(_, play)| *play)
}

#[cfg(test)]
mod tests {
    //! Unit tests for `level_library.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Level | every level of the shipped manifest (a library each, all different), an unknown folder | `every_manifest_level_has_a_library` |

    use super::*;
    use crate::levels_manifest::{get_levels_manifest_path, load_levels};

    #[test]
    fn every_manifest_level_has_a_library() {
        let levels: Vec<LevelEntry> = load_levels(&get_levels_manifest_path()).unwrap();
        let plays: Vec<usize> = levels
            .iter()
            .map(|level| {
                get_play_level(level).unwrap_or_else(|| panic!("{} has no library", level.name))
                    as usize
            })
            .collect();
        let mut distinct: Vec<usize> = plays.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), plays.len());
        let mut unknown: LevelEntry = levels[0].clone();
        unknown.folder = "demos/no_such_level".to_string();
        assert!(get_play_level(&unknown).is_none());
    }
}
