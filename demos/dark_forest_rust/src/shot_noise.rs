//! `shot_noise` holds the beam speckle levels that `net.rs` (copied verbatim from
//! `../hyperbolic_wormholes_rust/`) quantizes. The forest's rifle has no speckle, so players
//! send 0; keeping these keeps the protocol unchanged.
//!
//! Example: `let frac: f64 = shot_noise::round_speckle_frac(0.5);`
//!
//! Interface: `BEAM_SPECKLE_LEVEL_COUNT`, `round_speckle_frac`.

/// `BEAM_SPECKLE_LEVEL_COUNT` is how many levels a beam's speckle share is sent in.
pub const BEAM_SPECKLE_LEVEL_COUNT: u8 = 30;

/// `round_speckle_frac` rounds a speckle share in [0, 1] to the nearest level.
///
/// Example: `assert_eq!(round_speckle_frac(0.0), 0.0);`
pub fn round_speckle_frac(speckle_frac: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&speckle_frac),
        "round_speckle_frac: speckle_frac must be in [0, 1], not {speckle_frac}"
    );
    let level_count: f64 = f64::from(BEAM_SPECKLE_LEVEL_COUNT);
    (speckle_frac * level_count).round() / level_count
}
