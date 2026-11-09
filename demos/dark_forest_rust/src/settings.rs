//! `settings` is the game server's settings that `net.rs` sends to players in its welcome: how
//! long a console takes to switch off, how far it reaches, how long a player waits to respawn,
//! both guns (with the rifle hits that eliminate a player) and aiming; and the head bob, a player
//! setting. The forest keeps only what it uses from `../hyperbolic_wormholes_rust/`'s much larger
//! settings, under the same names and with the same values (W-126).
//!
//! Example: `let settings: ServerSettings = ServerSettings::Builtin();`
//!
//! Interface: `ServerSettings`, `ConsoleSettings`, `RespawnSettings`, `WormholeSettings`,
//! `SniperSettings`, `RifleSettings`, `BeamSpeckleSettings`, `AimSettings`, `HeadBobSettings`,
//! `HEAD_BOB`; in tests, `read_lunar_setting`.

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

/// `HeadBobSettings` is how deep the eye dips at each step, how many steps a second, how long the
/// bob takes to start or stop, and how the gun in hand sways with the steps while walking and
/// while sprinting: how far it slides left and right, and how far a gun as long as the bolt-action
/// rifle swings (its muzzle lagging the slide; shorter guns swing in proportion).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeadBobSettings {
    pub walk_depth_m: f64,
    pub sprint_depth_m: f64,
    pub walk_steps_per_s: f64,
    pub sprint_steps_per_s: f64,
    pub fade_s: f64,
    pub gun_walk_sway_m: f64,
    pub gun_sprint_sway_m: f64,
    pub rifle_walk_swing_rad: f64,
    pub rifle_sprint_swing_rad: f64,
}

/// `SniperSettings` is gun 1, the laser wand: how long a click's shot hits, how long it then takes
/// to reload, how many hits eliminate a player, how its beam flies (speed, and how long each
/// point stays lit), how far a shot strays from the crosshair (the mean of an exponential
/// distribution, and a cap; `shot_noise.rs`), and the share of its beam's pixels that speckle each
/// frame (the same: a mean and a cap).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SniperSettings {
    pub shot_s: f64,
    pub reload_s: f64,
    pub hits_to_eliminate: u8,
    pub beam_speed_m_per_s: f64,
    pub beam_trail_s: f64,
    pub noise_mean_rad: f64,
    pub noise_max_rad: f64,
    pub beam_speckle_mean_frac: f64,
    pub beam_speckle_max_frac: f64,
}

/// `RifleSettings` is gun 2: its fire rate, how long a shot hits, how its beam flies, overheating
/// and cooling, the hits that eliminate a player, each shot's recoil, the most a shot strays
/// from the crosshair, cold and at full heat, and the share of its beam's pixels that speckle each
/// frame, cold and at full heat (`shot_noise.rs`).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RifleSettings {
    pub shots_per_s: f64,
    pub pulse_s: f64,
    pub beam_speed_m_per_s: f64,
    pub beam_trail_s: f64,
    pub shots_until_overheat: u32,
    pub cool_down_s: f64,
    pub head_hits_to_eliminate: u8,
    pub body_hits_to_eliminate: u8,
    pub recoil_up_rad: f64,
    pub recoil_sideways_rad: f64,
    pub recoil_settle_s: f64,
    pub noise_start_rad: f64,
    pub noise_max_rad: f64,
    pub beam_speckle_start_frac: f64,
    pub beam_speckle_max_frac: f64,
}

/// `BeamSpeckleSettings` is how a laser beam's speckled pixels look, with both guns: the shares of
/// them painted black, turned white and dropping out (showing what is behind), which add up to 1.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct BeamSpeckleSettings {
    pub black_frac: f64,
    pub white_frac: f64,
    pub dropout_frac: f64,
}

/// `AimSettings` is aiming (holding the aim control): how much closer things look with each gun at
/// full zoom, and how long zooming fully in or out takes. The mouse turns the view more slowly by
/// the same factor.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct AimSettings {
    pub sniper_zoom: f64,
    pub rifle_zoom: f64,
    pub zoom_s: f64,
}

/// `HEAD_BOB` is the Lunar Surface demo's `view.head_bob` player setting
/// (`demos/hyperbolic_wormholes_rust/player_settings.yaml`): the eye's dip and the gun's sway with
/// each step.
pub const HEAD_BOB: HeadBobSettings = HeadBobSettings {
    walk_depth_m: 0.08,
    sprint_depth_m: 0.14,
    walk_steps_per_s: 2.0,
    sprint_steps_per_s: 2.8,
    fade_s: 0.1667,
    gun_walk_sway_m: 0.02,
    gun_sprint_sway_m: 0.04,
    rifle_walk_swing_rad: 0.03,
    rifle_sprint_swing_rad: 0.06,
};

/// `ServerSettings` is every setting the server decides for all players. The guns, their beams and
/// aiming are the Lunar Surface demo's (`demos/hyperbolic_wormholes_rust/server_settings.yaml`).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    pub consoles: ConsoleSettings,
    pub respawn: RespawnSettings,
    pub wormholes: WormholeSettings,
    pub sniper: SniperSettings,
    pub rifle: RifleSettings,
    pub beam_speckle: BeamSpeckleSettings,
    pub aim: AimSettings,
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
            sniper: SniperSettings {
                shot_s: 0.25,
                reload_s: 2.0,
                hits_to_eliminate: 1,
                beam_speed_m_per_s: 250.0,
                beam_trail_s: 0.25,
                noise_mean_rad: 0.003,
                noise_max_rad: 0.015,
                beam_speckle_mean_frac: 0.2,
                beam_speckle_max_frac: 0.5,
            },
            rifle: RifleSettings {
                shots_per_s: 8.0,
                pulse_s: 0.06,
                beam_speed_m_per_s: 250.0,
                beam_trail_s: 0.05,
                shots_until_overheat: 20,
                cool_down_s: 2.0,
                head_hits_to_eliminate: 2,
                body_hits_to_eliminate: 12,
                recoil_up_rad: 0.0075,
                recoil_sideways_rad: 0.0025,
                recoil_settle_s: 0.08,
                noise_start_rad: 0.002,
                noise_max_rad: 0.02,
                beam_speckle_start_frac: 0.0,
                beam_speckle_max_frac: 0.5,
            },
            beam_speckle: BeamSpeckleSettings {
                black_frac: 0.1,
                white_frac: 0.1,
                dropout_frac: 0.8,
            },
            aim: AimSettings {
                sniper_zoom: 2.5,
                rifle_zoom: 1.5,
                zoom_s: 0.15,
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

/// `tests` checks the settings survive the welcome's JSON, and that the guns, aiming and head bob
/// are the Lunar Surface demo's, number for number.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | source | built in, sent as JSON | `test_builtin_survives_json` |
/// | Lunar Surface file | `server_settings.yaml` | `test_guns_and_aim_match_lunar_surface` |
/// | | `player_settings.yaml` | `test_head_bob_matches_lunar_surface` |
/// | | consoles and respawn | `test_consoles_and_respawn_are_lunar_surface` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `read_lunar_section` reads the numbers in one section of a Lunar Surface settings file, by
    /// its lines (the forest has no YAML reader), as (key, number) pairs.
    ///
    /// Example: `read_lunar_section("player_settings.yaml", &["view", "head_bob"])`
    fn read_lunar_section(file_name: &str, section_path: &[&str]) -> Vec<(String, f64)> {
        let text: String =
            std::fs::read_to_string(format!("{LUNAR_SURFACE_FOLDER}{file_name}")).unwrap();
        let mut depth: usize = 0;
        let mut numbers: Vec<(String, f64)> = Vec::new();
        for line in text.lines() {
            let content: &str = line.split('#').next().unwrap().trim_end();
            if content.is_empty() {
                continue;
            }
            let indent: usize = content.len() - content.trim_start().len();
            let (key, value) = content.trim_start().split_once(':').unwrap();
            if depth == section_path.len() {
                if indent < depth * YAML_SPACES_PER_LEVEL {
                    break;
                }
                numbers.push((key.to_string(), value.trim().parse().unwrap()));
            } else if indent == depth * YAML_SPACES_PER_LEVEL && key == section_path[depth] {
                depth += 1;
            } else {
                assert!(
                    indent >= depth * YAML_SPACES_PER_LEVEL,
                    "{file_name} has no {section_path:?}"
                );
            }
        }
        assert!(!numbers.is_empty(), "{file_name} has no {section_path:?}");
        numbers
    }

    /// `assert_matches_lunar` checks `forest_section` has exactly the keys and numbers of Lunar
    /// Surface's section.
    ///
    /// Example: `assert_matches_lunar(&settings.aim, "server_settings.yaml", &["aim"]);`
    fn assert_matches_lunar<T: Serialize>(forest_section: &T, file_name: &str, path: &[&str]) {
        let forest_value: serde_json::Value = serde_json::to_value(forest_section).unwrap();
        let forest_numbers: &serde_json::Map<String, serde_json::Value> =
            forest_value.as_object().unwrap();
        let lunar_numbers: Vec<(String, f64)> = read_lunar_section(file_name, path);
        assert_eq!(forest_numbers.len(), lunar_numbers.len(), "{path:?}");
        for (key, lunar_number) in lunar_numbers {
            assert_eq!(
                forest_numbers[key.as_str()].as_f64().unwrap(),
                lunar_number,
                "{path:?}.{key}"
            );
        }
    }

    /// `test_guns_and_aim_match_lunar_surface` checks both guns, their beams' speckle and aiming
    /// are Lunar Surface's.
    #[test]
    fn test_guns_and_aim_match_lunar_surface() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let file_name: &str = "server_settings.yaml";
        assert_matches_lunar(&settings.sniper, file_name, &["sniper"]);
        assert_matches_lunar(&settings.rifle, file_name, &["rifle"]);
        assert_matches_lunar(&settings.beam_speckle, file_name, &["beam_speckle"]);
        assert_matches_lunar(&settings.aim, file_name, &["aim"]);
    }

    /// `test_head_bob_matches_lunar_surface` checks the head bob and gun sway are Lunar Surface's.
    #[test]
    fn test_head_bob_matches_lunar_surface() {
        assert_matches_lunar(&HEAD_BOB, "player_settings.yaml", &["view", "head_bob"]);
    }

    /// `test_consoles_and_respawn_are_lunar_surface` checks the consoles' hold and reach and the
    /// respawn delay are Lunar Surface's.
    #[test]
    fn test_consoles_and_respawn_are_lunar_surface() {
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
