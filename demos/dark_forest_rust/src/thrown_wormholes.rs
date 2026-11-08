//! `thrown_wormholes` holds the wormhole types that `net.rs` (copied verbatim from
//! `../hyperbolic_wormholes_rust/`) puts in its messages. The forest has no wormholes, so every
//! player sends `PlayerWormholes::default()`; keeping the types keeps the protocol unchanged.
//!
//! Example: `client.send_state(frame, false, true, None, 0.0, PlayerWormholes::default())?;`
//!
//! Interface: `WormholeColor`, `MouthPlace`, `PlayerWormholes`.

use glam::DVec4;

/// `WormholeColor` is which of a player's two mouths a wormhole is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WormholeColor {
    Purple,
    Green,
}

/// `MouthPlace` is where a mouth stands, in Fermi coordinates, and its yaw.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MouthPlace {
    pub u_m: f64,
    pub v_m: f64,
    pub h_m: f64,
    pub yaw_rad: f64,
}

/// `PlayerWormholes` is a player's mouths that are down or flying, and how damaged they are.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct PlayerWormholes {
    pub purple: Option<MouthPlace>,
    pub green: Option<MouthPlace>,
    pub flying: Option<(WormholeColor, DVec4)>,
    pub damage_frac: f64,
}

impl PlayerWormholes {
    /// `get_places` gives the places of the mouths that are down.
    ///
    /// Example: `let places = wormholes.get_places();`
    pub fn get_places(&self) -> Vec<MouthPlace> {
        self.purple.into_iter().chain(self.green).collect()
    }
}
