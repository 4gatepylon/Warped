//! `settings` is the game server's settings that `net.rs` sends to players in its welcome: how
//! long a console takes to switch off, how far it reaches, how long a player waits to respawn and
//! how many rifle hits eliminate a player. The forest keeps only what it uses from
//! `../hyperbolic_wormholes_rust/`'s much larger settings, under the same names and with the same
//! values (W-126).
//!
//! Example: `let settings: ServerSettings = ServerSettings::Builtin();`
//!
//! Interface: `ServerSettings`, `ConsoleSettings`, `RespawnSettings`, `WormholeSettings`,
//! `RifleSettings`; in tests, `read_lunar_setting`.

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

/// `RifleSettings` is how many rifle hits to the head (the top half meter) and to the body
/// eliminate a player.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RifleSettings {
    pub head_hits_to_eliminate: u8,
    pub body_hits_to_eliminate: u8,
}

/// `ServerSettings` is every setting the server decides for all players.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    pub consoles: ConsoleSettings,
    pub respawn: RespawnSettings,
    pub wormholes: WormholeSettings,
    pub rifle: RifleSettings,
}

impl ServerSettings {
    /// `Builtin` gives the default settings.
    ///
    /// Example: `let settings = ServerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> ServerSettings {
        ServerSettings {
            consoles: ConsoleSettings {
                hold_to_disable_s: 10.0,
                reach_m: 3.0,
            },
            respawn: RespawnSettings { delay_s: 3.0 },
            wormholes: WormholeSettings {
                sniper_hits_to_destroy: 1,
                rifle_hits_to_destroy: 3,
            },
            rifle: RifleSettings {
                head_hits_to_eliminate: 2,
                body_hits_to_eliminate: 12,
            },
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

/// `LUNAR_SURFACE_FOLDER` is where Lunar Surface's settings files are, for tests that compare.
#[cfg(test)]
const LUNAR_SURFACE_FOLDER: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../hyperbolic_wormholes_rust/");
/// `YAML_SPACES_PER_LEVEL` is how far each level of Lunar Surface's settings files is indented.
#[cfg(test)]
const YAML_SPACES_PER_LEVEL: usize = 2;

/// `read_lunar_setting` reads the number `key` in the section `section_path` of a Lunar Surface
/// settings file (`file_name` in its folder), by its lines: the forest has no YAML reader.
///
/// Example: `let reach_m: f64 = read_lunar_setting("server_settings.yaml", &["consoles"], "reach_m");`
#[cfg(test)]
pub fn read_lunar_setting(file_name: &str, section_path: &[&str], key: &str) -> f64 {
    let text: String =
        std::fs::read_to_string(format!("{LUNAR_SURFACE_FOLDER}{file_name}")).unwrap();
    let mut depth: usize = 0;
    for line in text.lines() {
        let content: &str = line.split('#').next().unwrap().trim_end();
        if content.is_empty() {
            continue;
        }
        let indent: usize = content.len() - content.trim_start().len();
        let (name, value) = content.trim_start().split_once(':').unwrap();
        if depth == section_path.len() && indent == depth * YAML_SPACES_PER_LEVEL && name == key {
            return value.trim().parse().unwrap();
        }
        if depth < section_path.len()
            && indent == depth * YAML_SPACES_PER_LEVEL
            && name == section_path[depth]
        {
            depth += 1;
        } else {
            assert!(
                indent >= depth * YAML_SPACES_PER_LEVEL || depth == 0,
                "{file_name} has no {section_path:?}.{key}"
            );
        }
    }
    panic!("{file_name} has no {section_path:?}.{key}");
}

/// `tests` checks the settings survive the welcome's JSON and are Lunar Surface's.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | source | built in, sent as JSON | `test_builtin_survives_json` |
/// | Lunar Surface's file | consoles, respawn, rifle hits | `test_builtin_is_lunar_surface` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `test_builtin_is_lunar_surface` checks the consoles' hold and reach, the respawn delay and
    /// the rifle's hits to eliminate are Lunar Surface's (`server_settings.yaml` there).
    #[test]
    fn test_builtin_is_lunar_surface() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let lunar = |section: &str, key: &str| -> f64 {
            read_lunar_setting("server_settings.yaml", &[section], key)
        };
        assert_eq!(
            settings.consoles.hold_to_disable_s,
            lunar("consoles", "hold_to_disable_s")
        );
        assert_eq!(settings.consoles.reach_m, lunar("consoles", "reach_m"));
        assert_eq!(settings.respawn.delay_s, lunar("respawn", "delay_s"));
        assert_eq!(
            f64::from(settings.rifle.head_hits_to_eliminate),
            lunar("rifle", "head_hits_to_eliminate")
        );
        assert_eq!(
            f64::from(settings.rifle.body_hits_to_eliminate),
            lunar("rifle", "body_hits_to_eliminate")
        );
    }

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
