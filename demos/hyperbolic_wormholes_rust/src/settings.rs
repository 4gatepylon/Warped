//! `settings.rs` reads `game_settings.yaml`, the numbers that tune how the game plays (movement,
//! sprint energy, head bob, both guns, consoles, respawning, motion blur), so players can tune
//! their game without rebuilding it. The game server sends its settings to every player who joins.
//!
//! Interface: `GameSettings` (`Builtin`, `Load`, `FromJson`, `to_json`) and its sections.
//!
//! Example: `let settings = GameSettings::Load(Path::new("fast.yaml"))?;`
//!
//! Must know:
//! - Every key is required and unknown keys fail, so a typo can't silently keep a default.
//! - The built-in settings are `game_settings.yaml` itself, compiled in.

use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const BUILTIN_YAML: &str = include_str!("../game_settings.yaml");
const ZERO_ALLOWED_KEYS: [&str; 10] = [
    "when_hit.red_frac",
    "when_hit.shake_px",
    "when_hit.blur_px",
    "head_bob.walk_depth_m",
    "head_bob.sprint_depth_m",
    "rifle.recoil_up_rad",
    "rifle.recoil_sideways_rad",
    "rifle.recoil_settle_s",
    "respawn.delay_s",
    "display.motion_blur_s",
];

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `MovementSettings` is walking and sprinting speed as multiples of the map's, a jump's height and
/// time in the air as a fraction of the map's jump (gravity follows from the two), and crouching:
/// its speed and eye height as fractions of walking's, and how long it takes to crouch or stand.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct MovementSettings {
    pub walk_speed_per_map_speed: f64,
    pub sprint_speed_per_map_speed: f64,
    pub jump_height_m: f64,
    pub jump_time_per_map_jump_time: f64,
    pub crouch_speed_frac: f64,
    pub crouch_eye_height_frac: f64,
    pub crouch_s: f64,
}

/// `SprintEnergySettings` is how long a full sprint bar lasts and how long an empty one takes to
/// refill.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SprintEnergySettings {
    pub drain_s: f64,
    pub refill_s: f64,
}

/// `HeadBobSettings` is how deep the eye dips at each step, how many steps a second, and how long
/// the bob takes to start or stop.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HeadBobSettings {
    pub walk_depth_m: f64,
    pub sprint_depth_m: f64,
    pub walk_steps_per_s: f64,
    pub sprint_steps_per_s: f64,
    pub fade_s: f64,
}

/// `SniperSettings` is gun 1, the laser wand: how long a click's shot hits, how many hits
/// eliminate a player, and how its beam flies (speed, and how long each point stays lit).
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct SniperSettings {
    pub shot_s: f64,
    pub hits_to_eliminate: u8,
    pub beam_speed_m_per_s: f64,
    pub beam_trail_s: f64,
}

/// `RifleSettings` is gun 2: its fire rate, how long a shot hits, how its beam flies, overheating
/// and cooling, the hits that eliminate a player, and each shot's recoil.
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
}

/// `ConsoleSettings` is how long holding `E` takes to turn off a console, and from how far.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ConsoleSettings {
    pub hold_to_disable_s: f64,
    pub reach_m: f64,
}

/// `RespawnSettings` is how long a player who was hit waits before coming back.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct RespawnSettings {
    pub delay_s: f64,
}

/// `WhenHitSettings` is what a player feels each time a hit takes some of their health: slowed
/// to a fraction of their speed for a while, and the screen's edges turning red, shaking and
/// blurring inward, all fading over `effect_s`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WhenHitSettings {
    pub slow_speed_frac: f64,
    pub slow_s: f64,
    pub effect_s: f64,
    pub red_frac: f64,
    pub shake_px: f64,
    pub blur_px: f64,
}

/// `DisplaySettings` is how long motion smears in the window.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct DisplaySettings {
    pub motion_blur_s: f64,
}

/// `GameSettings` is everything in `game_settings.yaml`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct GameSettings {
    pub movement: MovementSettings,
    pub sprint_energy: SprintEnergySettings,
    pub head_bob: HeadBobSettings,
    pub sniper: SniperSettings,
    pub rifle: RifleSettings,
    pub consoles: ConsoleSettings,
    pub respawn: RespawnSettings,
    pub when_hit: WhenHitSettings,
    pub display: DisplaySettings,
}

/// `find_bad_number` gives the dotted key of the first number in `value` that is negative or not
/// finite, or zero outside `ZERO_ALLOWED_KEYS`, or `None` if every number is fine.
///
/// Example: `let bad = find_bad_number(&serde_json::to_value(&settings)?, "");`
fn find_bad_number(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(fields) => fields.iter().find_map(|(name, field)| {
            let child: String = if key.is_empty() {
                name.clone()
            } else {
                format!("{key}.{name}")
            };
            find_bad_number(field, &child)
        }),
        Value::Number(number) => {
            let x: f64 = number.as_f64().expect("a JSON number reads as f64");
            let is_bad: bool =
                !x.is_finite() || x < 0.0 || (x == 0.0 && !ZERO_ALLOWED_KEYS.contains(&key));
            is_bad.then(|| key.to_string())
        }
        _ => None,
    }
}

impl GameSettings {
    /// `FromYaml` reads settings from YAML text, naming `source` in errors, and checks every number
    /// (`find_bad_number`).
    ///
    /// Example: `let settings = GameSettings::FromYaml(&text, "fast.yaml")?;`
    #[allow(non_snake_case)]
    fn FromYaml(text: &str, source: &str) -> Result<GameSettings> {
        let settings: GameSettings =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        if let Some(key) = find_bad_number(&serde_json::to_value(&settings)?, "") {
            return Err(format!(
                "settings {source}: {key} must be a number above 0 (0 is allowed only for {})",
                ZERO_ALLOWED_KEYS.join(", ")
            )
            .into());
        }
        Ok(settings)
    }

    /// `Builtin` gives the settings in `game_settings.yaml` as this program was built with it.
    ///
    /// Example: `let settings = GameSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> GameSettings {
        GameSettings::FromYaml(BUILTIN_YAML, "game_settings.yaml (built in)")
            .expect("the built-in game_settings.yaml is valid (tested)")
    }

    /// `Load` reads settings from a YAML file like `game_settings.yaml`.
    ///
    /// Example: `let settings = GameSettings::Load(Path::new("fast.yaml"))?;`
    #[allow(non_snake_case)]
    pub fn Load(path: &Path) -> Result<GameSettings> {
        let text: String = std::fs::read_to_string(path)
            .map_err(|error| format!("settings {}: {error}", path.display()))?;
        GameSettings::FromYaml(&text, &path.display().to_string())
    }

    /// `FromJson` reads settings sent by a game server (`to_json`), checking them as `Load` does.
    ///
    /// Example: `let settings = GameSettings::FromJson(&bytes)?;`
    #[allow(non_snake_case)]
    pub fn FromJson(bytes: &[u8]) -> Result<GameSettings> {
        let text: &str = std::str::from_utf8(bytes)?;
        GameSettings::FromYaml(text, "from the game server")
    }

    /// `to_json` gives the settings as compact JSON, to send to players.
    ///
    /// Example: `let bytes = settings.to_json();`
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("settings serialize to JSON")
    }
}

#[cfg(test)]
mod tests {
    //! Tests of reading and checking game settings.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Source | the built-in file, JSON from a server | `builtin_settings_survive_json` |
    //! | Keys | all present, one unknown, one missing | `builtin_settings_survive_json`, `unknown_and_missing_keys_fail` |
    //! | Numbers | above 0, 0 where allowed, 0 elsewhere, negative | `builtin_settings_survive_json`, `bad_numbers_name_their_key` |

    use super::*;

    #[test]
    fn builtin_settings_survive_json() {
        let settings: GameSettings = GameSettings::Builtin();
        assert_eq!(
            GameSettings::FromJson(&settings.to_json()).expect("valid JSON"),
            settings
        );
    }

    #[test]
    fn unknown_and_missing_keys_fail() {
        let unknown: String = BUILTIN_YAML.replace("drain_s:", "drain_seconds:");
        assert!(GameSettings::FromYaml(&unknown, "test").is_err());
        let missing: String = BUILTIN_YAML.replace("  refill_s: 15.0", "");
        assert!(GameSettings::FromYaml(&missing, "test").is_err());
    }

    #[test]
    fn bad_numbers_name_their_key() {
        let zero_blur: String = BUILTIN_YAML.replace("motion_blur_s: 0.01", "motion_blur_s: 0");
        assert!(GameSettings::FromYaml(&zero_blur, "test").is_ok());
        for (old, new, key) in [
            ("drain_s: 7.5", "drain_s: 0", "sprint_energy.drain_s"),
            ("shots_per_s: 8.0", "shots_per_s: -1", "rifle.shots_per_s"),
        ] {
            let error: String = GameSettings::FromYaml(&BUILTIN_YAML.replace(old, new), "test")
                .expect_err("a bad number fails")
                .to_string();
            assert!(error.contains(key), "{error}");
        }
    }
}
