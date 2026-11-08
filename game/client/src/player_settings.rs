//! `player_settings.rs` reads `player_settings.yaml`, each player's own choices (keys, mouse speed,
//! head bob, motion blur, the Milky Way's brightness, how a hit looks), so players can tune them
//! without rebuilding.
//!
//! Interface: `PerPlayerSettings` (`Builtin`, `Load`, `to_yaml`).
//!
//! Example: `let mine = PerPlayerSettings::Load(Path::new("mine.yaml"))?;`
//!
//! Must know:
//! - Every key is required and unknown keys fail, so a typo can't silently keep a default. The only
//!   exceptions are named: player keys removed since saves began (`REMOVED_PLAYER_KEYS`) are
//!   dropped, and those added since (`ADDED_PLAYER_KEYS`) take their built-in values when missing.
//! - The built-in settings are the file itself, compiled in.
//! - The tests here cover both settings files, since only the client reads both.

#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
use std::error::Error;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(target_os = "macos")]
use crate::key_bindings::KeyBindings;
use crate::settings::{
    BUILTIN_PLAYER_YAML, HitEffectSettings, MinimapSettings, ViewSettings, check_numbers, read_text,
};

#[cfg(target_os = "macos")]
const PLAYER_ZERO_ALLOWED_KEYS: [&str; 13] = [
    "view.motion_blur_s",
    "view.edge_stretch_frac",
    "view.wormhole_tint_frac",
    "view.milky_way_brightness_frac",
    "view.head_bob.walk_depth_m",
    "view.head_bob.sprint_depth_m",
    "view.head_bob.gun_walk_sway_m",
    "view.head_bob.gun_sprint_sway_m",
    "view.head_bob.rifle_walk_swing_rad",
    "view.head_bob.rifle_sprint_swing_rad",
    "hit_effects.red_frac",
    "hit_effects.shake_px",
    "hit_effects.blur_px",
];
const REMOVED_PLAYER_KEYS: [&str; 1] = ["view.wormhole_tint_s"];
const ADDED_PLAYER_KEYS: [&str; 6] = [
    "keys.switch_view",
    "view.milky_way_brightness_frac",
    "view.head_bob.gun_walk_sway_m",
    "view.head_bob.gun_sprint_sway_m",
    "view.head_bob.rifle_walk_swing_rad",
    "view.head_bob.rifle_sprint_swing_rad",
];

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `PlayerFile` is `player_settings.yaml` as written: the keys by name, the view and how a hit looks.
#[cfg(target_os = "macos")]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerFile {
    keys: BTreeMap<String, Vec<String>>,
    view: ViewSettings,
    hit_effects: HitEffectSettings,
    minimap: MinimapSettings,
}

/// `PerPlayerSettings` is everything in `player_settings.yaml`, with the keys read into controls.
#[cfg(target_os = "macos")]
pub struct PerPlayerSettings {
    pub keys: KeyBindings,
    pub view: ViewSettings,
    pub hit_effects: HitEffectSettings,
    pub minimap: MinimapSettings,
}

/// `drop_removed_keys` deletes from a settings file's YAML each dotted key of `removed` (such as
/// `view.wormhole_tint_s`) that it has, so a file an older version saved still loads; every other
/// unknown key still fails.
///
/// Example: `drop_removed_keys(&mut yaml, &REMOVED_PLAYER_KEYS);`
fn drop_removed_keys(yaml: &mut serde_yaml::Value, removed: &[&str]) {
    for key in removed {
        let mut parts: Vec<&str> = key.split('.').collect();
        let name: &str = parts.pop().expect("a key has at least one part");
        let parent: Option<&mut serde_yaml::Value> = parts
            .iter()
            .try_fold(&mut *yaml, |node, part| node.get_mut(*part));
        if let Some(serde_yaml::Value::Mapping(fields)) = parent {
            fields.remove(name);
        }
    }
}

/// `add_missing_keys` gives a settings file's YAML each dotted key of `added` (such as
/// `keys.switch_view`, added after players began saving their settings) that it lacks, with its
/// value in `defaults`, so a file an older version saved still loads; every other missing key
/// still fails. A key of `added` that `defaults` lacks is a bug, and panics.
///
/// Example: `add_missing_keys(&mut yaml, &defaults, &ADDED_PLAYER_KEYS);`
fn add_missing_keys(yaml: &mut serde_yaml::Value, defaults: &serde_yaml::Value, added: &[&str]) {
    for key in added {
        let mut parts: Vec<&str> = key.split('.').collect();
        let default: &serde_yaml::Value = parts
            .iter()
            .try_fold(defaults, |node, part| node.get(*part))
            .unwrap_or_else(|| panic!("add_missing_keys: the defaults have no {key}"));
        let name: &str = parts.pop().expect("a key has at least one part");
        let parent: Option<&mut serde_yaml::Value> = parts
            .iter()
            .try_fold(&mut *yaml, |node, part| node.get_mut(*part));
        if let Some(serde_yaml::Value::Mapping(fields)) = parent
            && !fields.contains_key(name)
        {
            fields.insert(name.into(), default.clone());
        }
    }
}

#[cfg(target_os = "macos")]
impl PerPlayerSettings {
    /// `FromYaml` reads a player's settings from YAML text, naming `source` in errors: the keys
    /// (`KeyBindings::FromNames`), and every number checked as `check_numbers` does, with
    /// `view.wormhole_tint_frac` and `view.milky_way_brightness_frac` at most 1. The keys in
    /// `REMOVED_PLAYER_KEYS`, which older versions saved, are dropped (`drop_removed_keys`), and those in `ADDED_PLAYER_KEYS`, which
    /// older versions didn't save, take their built-in values when missing (`add_missing_keys`).
    ///
    /// Example: `let mine = PerPlayerSettings::FromYaml(&text, "mine.yaml")?;`
    #[allow(non_snake_case)]
    fn FromYaml(text: &str, source: &str) -> Result<PerPlayerSettings> {
        let mut yaml: serde_yaml::Value =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        drop_removed_keys(&mut yaml, &REMOVED_PLAYER_KEYS);
        let defaults: serde_yaml::Value = serde_yaml::from_str(BUILTIN_PLAYER_YAML)
            .expect("the built-in player_settings.yaml is YAML (tested)");
        add_missing_keys(&mut yaml, &defaults, &ADDED_PLAYER_KEYS);
        let file: PlayerFile =
            serde_yaml::from_value(yaml).map_err(|error| format!("settings {source}: {error}"))?;
        let numbers: Value = serde_json::json!({"view": file.view, "hit_effects": file.hit_effects, "minimap": file.minimap});
        check_numbers(&numbers, &PLAYER_ZERO_ALLOWED_KEYS, source)?;
        for (key, value_frac) in [
            ("view.wormhole_tint_frac", file.view.wormhole_tint_frac),
            (
                "view.milky_way_brightness_frac",
                file.view.milky_way_brightness_frac,
            ),
        ] {
            if value_frac > 1.0 {
                return Err(format!(
                    "settings {source}: {key} must be at most 1, not {value_frac}"
                )
                .into());
            }
        }
        Ok(PerPlayerSettings {
            keys: KeyBindings::FromNames(&file.keys, source)?,
            view: file.view,
            hit_effects: file.hit_effects,
            minimap: file.minimap,
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

    /// `to_yaml` gives the settings as YAML text that `Load` reads back (without the notes).
    ///
    /// Example: `std::fs::write(path, mine.to_yaml())?;`
    pub fn to_yaml(&self) -> String {
        let file: PlayerFile = PlayerFile {
            keys: self.keys.to_names(),
            view: self.view.clone(),
            hit_effects: self.hit_effects.clone(),
            minimap: self.minimap.clone(),
        };
        serde_yaml::to_string(&file).expect("player settings serialize to YAML")
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
    //! | Source | the built-in files, server settings sent as JSON, player settings the menu saved | `builtin_settings_read_and_survive_json`, `saved_player_settings_load_back` |
    //! | Keys | all present, one unknown, one missing | `builtin_settings_read_and_survive_json`, `unknown_and_missing_keys_fail` |
    //! | Numbers | above 0, 0 where allowed, 0 elsewhere, negative; in either file | `builtin_settings_read_and_survive_json`, `bad_numbers_name_their_key` |
    //! | Numbers that bound each other | in order (built in), out of order (a zoom below 1, a map edge that stops before it starts, a rifle's noise or beam speckle that shrinks with heat, a speckle share above 1, speckle kinds that don't add up to 1) | `builtin_settings_read_and_survive_json`, `bad_numbers_name_their_key` |
    //! | Wormhole tint strength | 0, between 0 and 1, 1 (built in), above 1 | `wormhole_tint_takes_0_to_1`, `builtin_settings_read_and_survive_json` |
    //! | Removed key (`view.wormhole_tint_s`) | absent (built in), present in an old save (dropped, loads), a similar key never removed (fails) | `builtin_settings_read_and_survive_json`, `removed_keys_are_dropped_and_others_still_fail` |
    //! | Curvature per map curvature | below 1, 1, between 1 and 2 (1.5, built in), 2, above 2 | `curvature_takes_1_to_2`, `builtin_settings_read_and_survive_json` |
    //! | Added key (`keys.switch_view`, `view.milky_way_brightness_frac`, the four `view.head_bob` sway and swing keys) | present (built in), missing in an old save (gets its built-in value), another key missing (fails) | `builtin_settings_read_and_survive_json`, `added_keys_take_their_defaults_and_others_still_fail` |
    //! | Milky Way brightness | 0, between 0 and 1, 1 (built in), above 1 | `milky_way_brightness_takes_0_to_1`, `builtin_settings_read_and_survive_json` |
    //! | Slowest time rate near the black hole | 0, between 0 and 1 (0.1, built in), 1, above 1 | `min_time_rate_takes_above_0_to_1`, `builtin_settings_read_and_survive_json` |
    //! | Time dilation per general relativity | 0 (allowed: none), 15 (built in) | `min_time_rate_takes_above_0_to_1`, `builtin_settings_read_and_survive_json` |

    use super::*;
    use crate::key_bindings::{Action, Control};
    use crate::settings::*;
    use sdl3::keyboard::Scancode;

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
    fn saved_player_settings_load_back() {
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        mine.view.motion_blur_s = 0.0;
        mine.keys
            .set_controls(Action::Jump, vec![Control::Key(Scancode::J)])
            .expect("J can jump");
        let saved: String = mine.to_yaml();
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&saved, "test").expect("saved settings load");
        assert_eq!(loaded.to_yaml(), saved);
        assert_eq!(loaded.keys.get_names(Action::Jump), "J");
    }

    #[test]
    fn unknown_and_missing_keys_fail() {
        let unknown: String = BUILTIN_SERVER_YAML.replace("drain_s:", "drain_seconds:");
        assert!(ServerSettings::FromYaml(&unknown, "test").is_err());
        let missing: String = BUILTIN_SERVER_YAML.replace("  refill_walking_s: 15.0", "");
        assert!(ServerSettings::FromYaml(&missing, "test").is_err());
        let unknown_view: String = BUILTIN_PLAYER_YAML.replace("motion_blur_s:", "blur_s:");
        assert!(PerPlayerSettings::FromYaml(&unknown_view, "test").is_err());
    }

    #[test]
    fn bad_numbers_name_their_key() {
        let zero_blur: String =
            BUILTIN_PLAYER_YAML.replace("motion_blur_s: 0.01", "motion_blur_s: 0");
        assert!(PerPlayerSettings::FromYaml(&zero_blur, "test").is_ok());
        let no_reload: String = BUILTIN_SERVER_YAML.replace("reload_s: 2.0", "reload_s: 0");
        assert!(ServerSettings::FromYaml(&no_reload, "test").is_ok());
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
        let shrinking_aim: String =
            BUILTIN_SERVER_YAML.replace("rifle_zoom: 1.5", "rifle_zoom: 0.5");
        let error: String = ServerSettings::FromYaml(&shrinking_aim, "test")
            .expect_err("a zoom below 1 fails")
            .to_string();
        assert!(error.contains("aim"), "{error}");
        let shrinking_noise: String =
            BUILTIN_SERVER_YAML.replace("noise_start_rad: 0.002", "noise_start_rad: 0.5");
        let error: String = ServerSettings::FromYaml(&shrinking_noise, "test")
            .expect_err("a rifle's noise that shrinks with heat fails")
            .to_string();
        assert!(error.contains("noise_start_rad"), "{error}");
        for bad in [
            BUILTIN_SERVER_YAML.replace(
                "beam_speckle_start_frac: 0.0",
                "beam_speckle_start_frac: 0.7",
            ),
            BUILTIN_SERVER_YAML.replace("beam_speckle_max_frac: 0.5", "beam_speckle_max_frac: 1.5"),
            BUILTIN_SERVER_YAML
                .replace("beam_speckle_mean_frac: 0.2", "beam_speckle_mean_frac: 1.5"),
            BUILTIN_SERVER_YAML.replace("dropout_frac: 0.8", "dropout_frac: 0.7"),
        ] {
            let error: String = ServerSettings::FromYaml(&bad, "test")
                .expect_err("a bad beam speckle fails")
                .to_string();
            assert!(error.contains("beam speckles"), "{error}");
        }
        let all_dropout: String = BUILTIN_SERVER_YAML
            .replace("black_frac: 0.1", "black_frac: 0")
            .replace("white_frac: 0.1", "white_frac: 0")
            .replace("dropout_frac: 0.8", "dropout_frac: 1.0");
        assert!(ServerSettings::FromYaml(&all_dropout, "test").is_ok());
        let backward_edge: String =
            BUILTIN_SERVER_YAML.replace("start_wall_frac: 0.0", "start_wall_frac: 0.6");
        let error: String = ServerSettings::FromYaml(&backward_edge, "test")
            .expect_err("a stop below the start fails")
            .to_string();
        assert!(error.contains("map_edge"), "{error}");
        let still_view: String =
            BUILTIN_PLAYER_YAML.replace("look_rad_per_px: 0.0025", "look_rad_per_px: 0");
        let error: String = PerPlayerSettings::FromYaml(&still_view, "test")
            .err()
            .expect("a bad number fails")
            .to_string();
        assert!(error.contains("view.look_rad_per_px"), "{error}");
    }

    #[test]
    fn wormhole_tint_takes_0_to_1() {
        let none: String =
            BUILTIN_PLAYER_YAML.replace("wormhole_tint_frac: 1.0", "wormhole_tint_frac: 0");
        assert!(PerPlayerSettings::FromYaml(&none, "test").is_ok());
        let half: String =
            BUILTIN_PLAYER_YAML.replace("wormhole_tint_frac: 1.0", "wormhole_tint_frac: 0.5");
        assert!(PerPlayerSettings::FromYaml(&half, "test").is_ok());
        let too_strong: String =
            BUILTIN_PLAYER_YAML.replace("wormhole_tint_frac: 1.0", "wormhole_tint_frac: 1.5");
        let error: String = PerPlayerSettings::FromYaml(&too_strong, "test")
            .err()
            .expect("a tint above 1 fails")
            .to_string();
        assert!(error.contains("view.wormhole_tint_frac"), "{error}");
    }

    #[test]
    fn milky_way_brightness_takes_0_to_1() {
        const BUILTIN_LINE: &str = "milky_way_brightness_frac: 1.0";
        let with_brightness = |value: &str| -> String {
            BUILTIN_PLAYER_YAML
                .replace(BUILTIN_LINE, &format!("milky_way_brightness_frac: {value}"))
        };
        assert!(BUILTIN_PLAYER_YAML.contains(BUILTIN_LINE));
        for value in ["0", "0.5"] {
            let loaded: PerPlayerSettings =
                PerPlayerSettings::FromYaml(&with_brightness(value), "test").expect("0 to 1 loads");
            assert_eq!(
                loaded.view.milky_way_brightness_frac.to_string(),
                value.to_string()
            );
        }
        let error: String = PerPlayerSettings::FromYaml(&with_brightness("1.5"), "test")
            .err()
            .expect("a brightness above 1 fails")
            .to_string();
        assert!(error.contains("view.milky_way_brightness_frac"), "{error}");
    }

    #[test]
    fn removed_keys_are_dropped_and_others_still_fail() {
        const OLD_TINT_LINE: &str = "\n  wormhole_tint_s: 1.5  # removed by W-82";
        let with_tint_line = |line: &str| -> String {
            let tint_frac_line: &str = "\n  wormhole_tint_frac: 1.0";
            BUILTIN_PLAYER_YAML.replacen(tint_frac_line, &format!("{tint_frac_line}{line}"), 1)
        };
        let old_save: String = with_tint_line(OLD_TINT_LINE);
        assert_ne!(old_save, BUILTIN_PLAYER_YAML);
        let loaded: PerPlayerSettings =
            PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
        assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        let typo: String = with_tint_line("\n  wormhole_tint_seconds: 1.5");
        let error: String = PerPlayerSettings::FromYaml(&typo, "test")
            .err()
            .expect("a key that was never removed fails")
            .to_string();
        assert!(error.contains("wormhole_tint_seconds"), "{error}");
    }

    #[test]
    fn curvature_takes_1_to_2() {
        const BUILTIN_CURVATURE_PER_MAP_CURVATURE: f64 = 1.5;
        let builtin: f64 = ServerSettings::Builtin().space.curvature_per_map_curvature;
        assert_eq!(builtin, BUILTIN_CURVATURE_PER_MAP_CURVATURE);
        for (factor, is_valid) in [("1.0", true), ("2.0", true), ("0.5", false), ("2.5", false)] {
            let yaml: String = BUILTIN_SERVER_YAML.replace(
                "curvature_per_map_curvature: 1.5",
                &format!("curvature_per_map_curvature: {factor}"),
            );
            let result: Result<ServerSettings> = ServerSettings::FromYaml(&yaml, "test");
            assert_eq!(result.is_ok(), is_valid, "{factor}");
            if let Err(error) = result {
                assert!(
                    error
                        .to_string()
                        .contains("space.curvature_per_map_curvature"),
                    "{error}"
                );
            }
        }
    }

    #[test]
    fn min_time_rate_takes_above_0_to_1() {
        let with_values = |rate_frac: &str, dilation: &str| -> Result<ServerSettings> {
            let yaml: String = BUILTIN_SERVER_YAML
                .replace(
                    "min_time_rate_frac: 0.1",
                    &format!("min_time_rate_frac: {rate_frac}"),
                )
                .replace(
                    "time_dilation_per_general_relativity: 15.0",
                    &format!("time_dilation_per_general_relativity: {dilation}"),
                );
            ServerSettings::FromYaml(&yaml, "test")
        };
        for (rate_frac, is_valid) in [("0.0", false), ("1.0", true), ("1.5", false)] {
            let result: Result<ServerSettings> = with_values(rate_frac, "15.0");
            assert_eq!(result.is_ok(), is_valid, "{rate_frac}");
            if let Err(error) = result {
                assert!(
                    error.to_string().contains("black_hole.min_time_rate_frac"),
                    "{error}"
                );
            }
        }
        assert!(with_values("0.1", "0.0").is_ok());
    }

    #[test]
    fn added_keys_take_their_defaults_and_others_still_fail() {
        let without_key = |name: &str| -> String {
            let prefix: String = format!("{name}:");
            let lines: Vec<&str> = BUILTIN_PLAYER_YAML
                .lines()
                .filter(|line| !line.trim_start().starts_with(&prefix))
                .collect();
            lines.join("\n")
        };
        for name in [
            "switch_view",
            "milky_way_brightness_frac",
            "gun_walk_sway_m",
            "gun_sprint_sway_m",
            "rifle_walk_swing_rad",
            "rifle_sprint_swing_rad",
        ] {
            let old_save: String = without_key(name);
            assert!(!old_save.contains(name));
            let loaded: PerPlayerSettings =
                PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
            assert!(
                loaded
                    .keys
                    .matches(Action::SwitchView, Control::Key(Scancode::V))
            );
            assert_eq!(loaded.view.milky_way_brightness_frac, 1.0);
            assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        }
        let error: String = PerPlayerSettings::FromYaml(&without_key("flashlight"), "test")
            .err()
            .expect("a key that wasn't added later still fails when missing")
            .to_string();
        assert!(error.contains("flashlight"), "{error}");
    }
}
