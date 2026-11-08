//! `settings` is the game server's settings that `net.rs` sends to players in its welcome: how
//! long a console takes to switch off, how far it reaches, and how long a player waits to
//! respawn. The forest keeps only what it uses from `../hyperbolic_wormholes_rust/`'s much larger
//! settings, under the same names.
//!
//! Example: `let settings: ServerSettings = ServerSettings::Builtin();`
//!
//! Interface: `ServerSettings`, `ConsoleSettings`, `RespawnSettings`, `WormholeSettings`.

use std::error::Error;

use serde::{Deserialize, Serialize};

/// `ConsoleSettings` is how long a player holds `E` to switch a console off, and from how far.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ConsoleSettings {
    pub hold_to_disable_s: f64,
    pub reach_m: f64,
}

/// `RespawnSettings` is how long a player who was eliminated waits before coming back.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RespawnSettings {
    pub delay_s: f64,
}

/// `WormholeSettings` is how many hits destroy thrown wormholes; the forest has none, but the
/// protocol's hit messages carry the counts.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WormholeSettings {
    pub sniper_hits_to_destroy: u8,
    pub rifle_hits_to_destroy: u8,
}

/// `ServerSettings` is every setting the server decides for all players.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    pub consoles: ConsoleSettings,
    pub respawn: RespawnSettings,
    pub wormholes: WormholeSettings,
    pub rifle_hits_to_kill: u8,
}

impl ServerSettings {
    /// `Builtin` gives the default settings.
    ///
    /// Example: `let settings = ServerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> ServerSettings {
        ServerSettings {
            consoles: ConsoleSettings {
                hold_to_disable_s: 3.0,
                reach_m: 2.5,
            },
            respawn: RespawnSettings { delay_s: 3.0 },
            wormholes: WormholeSettings {
                sniper_hits_to_destroy: 1,
                rifle_hits_to_destroy: 3,
            },
            rifle_hits_to_kill: 4,
        }
    }

    /// `FromJson` reads settings sent by a game server (`to_json`).
    ///
    /// Example: `let settings = ServerSettings::FromJson(&bytes)?;`
    #[allow(non_snake_case)]
    pub fn FromJson(bytes: &[u8]) -> Result<ServerSettings, Box<dyn Error>> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// `to_json` gives the settings as compact JSON, to send to players.
    ///
    /// Example: `let bytes = settings.to_json();`
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("settings serialize to JSON")
    }
}

/// `tests` checks the settings survive the welcome's JSON.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | source | built in, sent as JSON | `test_builtin_survives_json` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_builtin_survives_json` checks `FromJson` inverts `to_json`.
    #[test]
    fn test_builtin_survives_json() {
        let settings: ServerSettings = ServerSettings::Builtin();
        assert_eq!(
            ServerSettings::FromJson(&settings.to_json()).unwrap(),
            settings
        );
    }
}
