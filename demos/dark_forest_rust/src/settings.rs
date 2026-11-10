//! `settings` is the game server's settings that `net.rs` sends to players in its welcome: how
//! long a console takes to switch off, how far it reaches, how long a player waits to respawn,
//! both guns (with the rifle hits that eliminate a player), aiming, movement, sprint energy, how a
//! hit slows, the flashlight, thrown wormholes and the warps' pull on walkers (`gravity`, W-144,
//! the shared built-in values when the server leaves it out); and the head bob, a player setting. The forest keeps only what it uses from `../hyperbolic_wormholes_rust/`'s much larger
//! settings, under the same names and with the same values (W-126), in its own
//! `server_settings.yaml` (built in, or `--server-settings`).
//!
//! Example: `let settings: ServerSettings = ServerSettings::Builtin();`
//!
//! Interface: `ServerSettings`, `ConsoleSettings`, `RespawnSettings`, `WormholeSettings`,
//! `SniperSettings`, `RifleSettings`, `BeamSpeckleSettings`, `AimSettings`, `MovementSettings`,
//! `SprintEnergySettings`, `WhenHitSettings`, `FlashlightSettings`, `GravitySettings`,
//! `HeadBobSettings`, `HEAD_BOB`,
//! `HitEffectSettings`, `HIT_EFFECTS`; in tests, `read_lunar_setting`. Every section but
//! `WormholeSettings` is the shared `warped_shared::play_settings` one (W-126, same play in every
//! level).

use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use warped_shared::game_settings::{parse_settings_text, read_settings_file};
pub use warped_shared::play_settings::{
    AimSettings, BeamSpeckleSettings, ConsoleSettings, FlashlightSettings, GravitySettings,
    HeadBobSettings, HitEffectSettings, MovementSettings, RespawnSettings, RifleSettings,
    SniperSettings, SprintEnergySettings, WhenHitSettings,
};

/// `WormholeSettings` is how players throw their wormhole mouths (`Q`, `thrown_wormholes.rs`) and
/// how many enemy shots through a pair destroy it: Lunar Surface's `wormholes` settings
/// (`demos/hyperbolic_wormholes_rust/server_settings.yaml`, W-126) but for the mouth's size, which
/// is the forest's own (`game::warps::THROWN_MOUTH_REACH_M` and its neighbours).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WormholeSettings {
    pub throw_speed_m_per_s: f64,
    pub throw_up_rad: f64,
    pub gravity_m_per_s_per_s: f64,
    pub max_flight_s: f64,
    pub sniper_hits_to_destroy: u8,
    pub rifle_hits_to_destroy: u8,
    pub hit_flash_s: f64,
}

/// `BUILTIN_SERVER_YAML` is `server_settings.yaml`, built into the program.
const BUILTIN_SERVER_YAML: &str = include_str!("../server_settings.yaml");

/// `HIT_EFFECTS` is the Lunar Surface demo's `hit_effects` player setting
/// (`demos/hyperbolic_wormholes_rust/player_settings.yaml`): how long a hit's red, shake and blur
/// take to fade, how red the screen's edges turn, and how far the screen shakes and the edges blur.
pub const HIT_EFFECTS: HitEffectSettings = HitEffectSettings {
    effect_s: 0.6,
    red_frac: 0.6,
    shake_px: 6.0,
    blur_px: 6.0,
};

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
    pub movement: MovementSettings,
    pub sprint_energy: SprintEnergySettings,
    pub when_hit: WhenHitSettings,
    pub flashlight: FlashlightSettings,
    #[serde(default)]
    pub gravity: GravitySettings,
}

impl ServerSettings {
    /// `Builtin` gives the settings in `server_settings.yaml` as this program was built with it.
    ///
    /// Example: `let settings = ServerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> ServerSettings {
        ServerSettings::FromText(BUILTIN_SERVER_YAML, "server_settings.yaml (built in)")
            .expect("the built-in server_settings.yaml is valid (tested)")
    }

    /// `Load` reads settings from a file like `server_settings.yaml` (YAML or JSON), every key
    /// required: `--server-settings`, such as the match server's copy for each round.
    ///
    /// Example: `let settings = ServerSettings::Load(Path::new("fast.yaml"))?;`
    #[allow(non_snake_case)]
    pub fn Load(path: &Path) -> Result<ServerSettings, Box<dyn Error>> {
        let value: Value = read_settings_file(path)?;
        ServerSettings::FromValue(value, &path.display().to_string())
    }

    /// `FromText` reads settings from the text of a settings file, naming `source` in its errors.
    ///
    /// Example: `let settings = ServerSettings::FromText(BUILTIN_SERVER_YAML, "built in")?;`
    #[allow(non_snake_case)]
    fn FromText(text: &str, source: &str) -> Result<ServerSettings, Box<dyn Error>> {
        ServerSettings::FromValue(parse_settings_text(text, source)?, source)
    }

    /// `FromValue` reads settings from a settings file's value, checking `gravity`'s ranges
    /// (`GravitySettings::check`) and naming `source` in its errors.
    ///
    /// Example: `let settings = ServerSettings::FromValue(value, "fast.yaml")?;`
    #[allow(non_snake_case)]
    fn FromValue(value: Value, source: &str) -> Result<ServerSettings, Box<dyn Error>> {
        let settings: ServerSettings =
            serde_json::from_value(value).map_err(|error| format!("{source}: {error}"))?;
        settings
            .gravity
            .check()
            .map_err(|error| format!("{source}: {error}"))?;
        Ok(settings)
    }

    /// `FromJson` reads settings sent by a game server (`to_json`), and checks `gravity`'s
    /// ranges (`GravitySettings::check`).
    ///
    /// Example: `let settings = ServerSettings::FromJson(&bytes)?;`
    #[allow(non_snake_case)]
    pub fn FromJson(bytes: &[u8]) -> Result<ServerSettings, Box<dyn Error>> {
        let settings: ServerSettings = serde_json::from_slice(bytes)?;
        settings.gravity.check()?;
        Ok(settings)
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
/// | | a file (`--server-settings`): JSON of the built-in, one key missing, no file | `test_settings_files_load_whole` |
/// | Lunar Surface file | `server_settings.yaml` | `test_guns_and_aim_match_lunar_surface` |
/// | | movement, sprint energy, when hit, flashlight | `test_movement_matches_lunar_surface` |
/// | | `player_settings.yaml` (head bob, hit effects) | `test_head_bob_matches_lunar_surface` |
/// | | consoles and respawn | `test_consoles_and_respawn_are_lunar_surface` |
/// | | thrown wormholes, but for the mouth's size | `test_thrown_wormholes_are_lunar_surface` |
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

    const FOREST_WALK_SPEED_PER_MAP_SPEED: f64 = 1.6;
    const FOREST_SPRINT_SPEED_PER_MAP_SPEED: f64 = 1.72;

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

    /// `test_movement_matches_lunar_surface` checks movement, sprint energy, how a hit slows and the
    /// flashlight are Lunar Surface's, except walking and sprinting, which the user asked slower
    /// (halfway between City Sphere's and Lunar Surface's).
    #[test]
    fn test_movement_matches_lunar_surface() {
        let settings: ServerSettings = ServerSettings::Builtin();
        let file_name: &str = "server_settings.yaml";
        assert_eq!(
            settings.movement.walk_speed_per_map_speed,
            FOREST_WALK_SPEED_PER_MAP_SPEED
        );
        assert_eq!(
            settings.movement.sprint_speed_per_map_speed,
            FOREST_SPRINT_SPEED_PER_MAP_SPEED
        );
        let lunar = |key: &str| -> f64 { read_lunar_setting(file_name, &["movement"], key) };
        let movement: MovementSettings = MovementSettings {
            walk_speed_per_map_speed: lunar("walk_speed_per_map_speed"),
            sprint_speed_per_map_speed: lunar("sprint_speed_per_map_speed"),
            ..settings.movement.clone()
        };
        assert_matches_lunar(&movement, file_name, &["movement"]);
        assert_matches_lunar(&settings.sprint_energy, file_name, &["sprint_energy"]);
        assert_matches_lunar(&settings.when_hit, file_name, &["when_hit"]);
        assert_matches_lunar(&settings.flashlight, file_name, &["flashlight"]);
    }

    /// `test_head_bob_matches_lunar_surface` checks the head bob, gun sway and a hit's look are
    /// Lunar Surface's.
    #[test]
    fn test_head_bob_matches_lunar_surface() {
        assert_matches_lunar(&HEAD_BOB, "player_settings.yaml", &["view", "head_bob"]);
        assert_matches_lunar(&HIT_EFFECTS, "player_settings.yaml", &["hit_effects"]);
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

    /// `test_thrown_wormholes_are_lunar_surface` checks every thrown-wormhole setting is Lunar
    /// Surface's.
    #[test]
    fn test_thrown_wormholes_are_lunar_surface() {
        let settings: serde_json::Value =
            serde_json::to_value(ServerSettings::Builtin().wormholes).unwrap();
        for (key, value) in settings.as_object().unwrap() {
            let lunar: f64 = read_lunar_setting("server_settings.yaml", &["wormholes"], key);
            assert_eq!(value.as_f64().unwrap(), lunar, "wormholes.{key}");
        }
    }

    /// `test_settings_files_load_whole` checks `Load` reads a whole settings file (the match
    /// server's JSON copy of the built-in ones) and refuses one with a key missing, or none.
    #[test]
    fn test_settings_files_load_whole() {
        let folder = std::env::temp_dir().join(format!("forest_settings_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut value: serde_json::Value = serde_json::to_value(ServerSettings::Builtin()).unwrap();
        value["respawn"]["delay_s"] = serde_json::json!(7.5);
        let path = folder.join("round_1_server_settings.yaml");
        std::fs::write(&path, value.to_string()).unwrap();
        assert_eq!(ServerSettings::Load(&path).unwrap().respawn.delay_s, 7.5);
        value["respawn"].as_object_mut().unwrap().remove("delay_s");
        std::fs::write(&path, value.to_string()).unwrap();
        assert!(ServerSettings::Load(&path).is_err());
        assert!(ServerSettings::Load(&folder.join("none.yaml")).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
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
