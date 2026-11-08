//! `settings.rs` reads the game's two settings files, so players can tune it without rebuilding:
//! `server_settings.yaml`, the numbers that shape play for everyone (movement, sprint energy, both
//! guns, consoles, respawning, how a hit slows, the flashlight), which the game server sends to every
//! player who joins; and `player_settings.yaml`, each player's own choices (keys, mouse speed, head
//! bob, motion blur, how a hit looks).
//!
//! Interface: `ServerSettings` (`Builtin`, `Load`, `FromJson`, `to_json`), `PerPlayerSettings`
//! (`Builtin`, `Load`) and their sections.
//!
//! Example: `let settings = ServerSettings::Load(Path::new("fast.yaml"))?;`
//!
//! Must know:
//! - Every key is required and unknown keys fail, so a typo can't silently keep a default.
//! - The built-in settings are the two files themselves, compiled in.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::key_bindings::KeyBindings;

const BUILTIN_SERVER_YAML: &str = include_str!("../server_settings.yaml");
const BUILTIN_PLAYER_YAML: &str = include_str!("../player_settings.yaml");
const SERVER_ZERO_ALLOWED_KEYS: [&str; 4] = [
    "rifle.recoil_up_rad",
    "rifle.recoil_sideways_rad",
    "rifle.recoil_settle_s",
    "respawn.delay_s",
];
const PLAYER_ZERO_ALLOWED_KEYS: [&str; 6] = [
    "view.motion_blur_s",
    "view.head_bob.walk_depth_m",
    "view.head_bob.sprint_depth_m",
    "hit_effects.red_frac",
    "hit_effects.shake_px",
    "hit_effects.blur_px",
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

/// `WhenHitSettings` is how a hit slows the player who takes it: to a fraction of their speed, for
/// a while.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct WhenHitSettings {
    pub slow_speed_frac: f64,
    pub slow_s: f64,
}

/// `HitEffectSettings` is how a hit looks to the player who takes it: the screen's edges turning
/// red, shaking and blurring inward, all fading over `effect_s`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct HitEffectSettings {
    pub effect_s: f64,
    pub red_frac: f64,
    pub shake_px: f64,
    pub blur_px: f64,
}

/// `FlashlightSettings` is the flashlight's cone (half its opening angle and the width of its soft
/// edge), the distance at which its light falls to half, and its brightness against the sun's.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashlightSettings {
    pub cone_half_angle_rad: f64,
    pub soft_edge_rad: f64,
    pub range_m: f64,
    pub brightness_frac: f64,
}

/// `ViewSettings` is how a player's view moves: how far it turns for each pixel the mouse moves, how
/// long motion smears, and the head bob.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ViewSettings {
    pub look_rad_per_px: f64,
    pub motion_blur_s: f64,
    pub head_bob: HeadBobSettings,
}

/// `PlayerFile` is `player_settings.yaml` as written: the keys by name, the view and how a hit looks.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerFile {
    keys: BTreeMap<String, Vec<String>>,
    view: ViewSettings,
    hit_effects: HitEffectSettings,
}

/// `PerPlayerSettings` is everything in `player_settings.yaml`, with the keys read into controls.
pub struct PerPlayerSettings {
    pub keys: KeyBindings,
    pub view: ViewSettings,
    pub hit_effects: HitEffectSettings,
}

/// `ServerSettings` is everything in `server_settings.yaml`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    pub movement: MovementSettings,
    pub sprint_energy: SprintEnergySettings,
    pub sniper: SniperSettings,
    pub rifle: RifleSettings,
    pub consoles: ConsoleSettings,
    pub respawn: RespawnSettings,
    pub when_hit: WhenHitSettings,
    pub flashlight: FlashlightSettings,
}

/// `check_numbers` fails, naming the key and `source`, on the first number in `value` that is
/// negative or not finite, or zero outside `zero_allowed`.
///
/// Example: `check_numbers(&serde_json::to_value(&settings)?, &SERVER_ZERO_ALLOWED_KEYS, "fast.yaml")?;`
fn check_numbers(value: &Value, zero_allowed: &[&str], source: &str) -> Result<()> {
    match find_bad_number(value, "", zero_allowed) {
        Some(key) => Err(format!(
            "settings {source}: {key} must be a number above 0 (0 is allowed only for {})",
            zero_allowed.join(", ")
        )
        .into()),
        None => Ok(()),
    }
}

/// `find_bad_number` gives the dotted key of the first number in `value` that is negative or not
/// finite, or zero outside `zero_allowed`, or `None` if every number is fine.
///
/// Example: `let bad = find_bad_number(&serde_json::to_value(&settings)?, "", &SERVER_ZERO_ALLOWED_KEYS);`
fn find_bad_number(value: &Value, key: &str, zero_allowed: &[&str]) -> Option<String> {
    match value {
        Value::Object(fields) => fields.iter().find_map(|(name, field)| {
            let child: String = if key.is_empty() {
                name.clone()
            } else {
                format!("{key}.{name}")
            };
            find_bad_number(field, &child, zero_allowed)
        }),
        Value::Number(number) => {
            let x: f64 = number.as_f64().expect("a JSON number reads as f64");
            let is_bad: bool =
                !x.is_finite() || x < 0.0 || (x == 0.0 && !zero_allowed.contains(&key));
            is_bad.then(|| key.to_string())
        }
        _ => None,
    }
}

/// `read_text` reads a settings file, naming it in errors.
///
/// Example: `let text = read_text(Path::new("fast.yaml"))?;`
fn read_text(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)
        .map_err(|error| format!("settings {}: {error}", path.display()))?)
}

impl ServerSettings {
    /// `FromYaml` reads server settings from YAML text, naming `source` in errors, and checks every
    /// number (`check_numbers`).
    ///
    /// Example: `let settings = ServerSettings::FromYaml(&text, "fast.yaml")?;`
    #[allow(non_snake_case)]
    fn FromYaml(text: &str, source: &str) -> Result<ServerSettings> {
        let settings: ServerSettings =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        check_numbers(
            &serde_json::to_value(&settings)?,
            &SERVER_ZERO_ALLOWED_KEYS,
            source,
        )?;
        Ok(settings)
    }

    /// `Builtin` gives the settings in `server_settings.yaml` as this program was built with it.
    ///
    /// Example: `let settings = ServerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> ServerSettings {
        ServerSettings::FromYaml(BUILTIN_SERVER_YAML, "server_settings.yaml (built in)")
            .expect("the built-in server_settings.yaml is valid (tested)")
    }

    /// `Load` reads server settings from a YAML file like `server_settings.yaml`.
    ///
    /// Example: `let settings = ServerSettings::Load(Path::new("fast.yaml"))?;`
    #[allow(non_snake_case)]
    pub fn Load(path: &Path) -> Result<ServerSettings> {
        ServerSettings::FromYaml(&read_text(path)?, &path.display().to_string())
    }

    /// `FromJson` reads settings sent by a game server (`to_json`), checking them as `Load` does.
    ///
    /// Example: `let settings = ServerSettings::FromJson(&bytes)?;`
    #[allow(non_snake_case)]
    pub fn FromJson(bytes: &[u8]) -> Result<ServerSettings> {
        let text: &str = std::str::from_utf8(bytes)?;
        ServerSettings::FromYaml(text, "from the game server")
    }

    /// `to_json` gives the settings as compact JSON, to send to players.
    ///
    /// Example: `let bytes = settings.to_json();`
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("settings serialize to JSON")
    }
}

impl PerPlayerSettings {
    /// `FromYaml` reads a player's settings from YAML text, naming `source` in errors: the keys
    /// (`KeyBindings::FromNames`), and every number checked as `check_numbers` does.
    ///
    /// Example: `let mine = PerPlayerSettings::FromYaml(&text, "mine.yaml")?;`
    #[allow(non_snake_case)]
    fn FromYaml(text: &str, source: &str) -> Result<PerPlayerSettings> {
        let file: PlayerFile =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        let numbers: Value =
            serde_json::json!({"view": file.view, "hit_effects": file.hit_effects});
        check_numbers(&numbers, &PLAYER_ZERO_ALLOWED_KEYS, source)?;
        Ok(PerPlayerSettings {
            keys: KeyBindings::FromNames(&file.keys, source)?,
            view: file.view,
            hit_effects: file.hit_effects,
        })
    }

    /// `Builtin` gives the settings in `player_settings.yaml` as this program was built with it.
    ///
    /// Example: `let mine = PerPlayerSettings::Builtin();`
    #[allow(non_snake_case)]
    pub fn Builtin() -> PerPlayerSettings {
        PerPlayerSettings::FromYaml(BUILTIN_PLAYER_YAML, "player_settings.yaml (built in)")
            .expect("the built-in player_settings.yaml is valid (tested)")
    }

    /// `Load` reads a player's settings from a YAML file like `player_settings.yaml`.
    ///
    /// Example: `let mine = PerPlayerSettings::Load(Path::new("mine.yaml"))?;`
    #[allow(non_snake_case)]
    pub fn Load(path: &Path) -> Result<PerPlayerSettings> {
        PerPlayerSettings::FromYaml(&read_text(path)?, &path.display().to_string())
    }
}

#[cfg(test)]
mod tests {
    //! Tests of reading and checking both settings files.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Source | the built-in files, server settings sent as JSON | `builtin_settings_read_and_survive_json` |
    //! | Keys | all present, one unknown, one missing | `builtin_settings_read_and_survive_json`, `unknown_and_missing_keys_fail` |
    //! | Numbers | above 0, 0 where allowed, 0 elsewhere, negative; in either file | `builtin_settings_read_and_survive_json`, `bad_numbers_name_their_key` |

    use super::*;

    #[test]
    fn builtin_settings_read_and_survive_json() {
        let settings: ServerSettings = ServerSettings::Builtin();
        assert_eq!(
            ServerSettings::FromJson(&settings.to_json()).expect("valid JSON"),
            settings
        );
        assert!(PerPlayerSettings::Builtin().view.look_rad_per_px > 0.0);
    }

    #[test]
    fn unknown_and_missing_keys_fail() {
        let unknown: String = BUILTIN_SERVER_YAML.replace("drain_s:", "drain_seconds:");
        assert!(ServerSettings::FromYaml(&unknown, "test").is_err());
        let missing: String = BUILTIN_SERVER_YAML.replace("  refill_s: 15.0", "");
        assert!(ServerSettings::FromYaml(&missing, "test").is_err());
        let unknown_view: String = BUILTIN_PLAYER_YAML.replace("motion_blur_s:", "blur_s:");
        assert!(PerPlayerSettings::FromYaml(&unknown_view, "test").is_err());
    }

    #[test]
    fn bad_numbers_name_their_key() {
        let zero_blur: String =
            BUILTIN_PLAYER_YAML.replace("motion_blur_s: 0.01", "motion_blur_s: 0");
        assert!(PerPlayerSettings::FromYaml(&zero_blur, "test").is_ok());
        for (old, new, key) in [
            ("drain_s: 7.5", "drain_s: 0", "sprint_energy.drain_s"),
            ("shots_per_s: 8.0", "shots_per_s: -1", "rifle.shots_per_s"),
        ] {
            let error: String =
                ServerSettings::FromYaml(&BUILTIN_SERVER_YAML.replace(old, new), "test")
                    .expect_err("a bad number fails")
                    .to_string();
            assert!(error.contains(key), "{error}");
        }
        let still_view: String =
            BUILTIN_PLAYER_YAML.replace("look_rad_per_px: 0.0025", "look_rad_per_px: 0");
        let error: String = PerPlayerSettings::FromYaml(&still_view, "test")
            .err()
            .expect("a bad number fails")
            .to_string();
        assert!(error.contains("view.look_rad_per_px"), "{error}");
    }
}
