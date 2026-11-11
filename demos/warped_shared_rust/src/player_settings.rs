//! `player_settings.rs` reads, checks and saves `player_settings.yaml`, each player's own choices
//! (keys, mouse speed, head bob, motion blur, the Milky Way's brightness, how a hit looks, the
//! minimap, the picture's visuals), the same file every level reads; and lists the numbers a settings page edits
//! (`PLAYER_NUMBER_ROWS`). The one-window app (`../warped_mvp_rust/`) edits it from every menu.
//!
//! Interface: `PerPlayerSettings` (`Builtin`, `Load`, `FromYaml`, `FromValue`, `to_value`,
//! `to_yaml`, `save`), `get_saved_player_settings_path`, `PLAYER_NUMBER_ROWS`,
//! `BUILTIN_PLAYER_YAML`, and the generic file helpers `check_numbers`, `drop_removed_keys`,
//! `add_missing_keys`.
//!
//! Example: `let mine = PerPlayerSettings::Load(&get_saved_player_settings_path()?)?;`
//!
//! Must know:
//! - Gotchas: this is the levels' `settings.rs` player half (Lunar Surface's, the reference),
//!   moved here; until the levels use it (after W-140, hold Tab for the map, Esc for the menu),
//!   a change to the file's keys changes both, with `REMOVED_PLAYER_KEYS` and
//!   `ADDED_PLAYER_KEYS` (the `compatible-changes` skill).
//! - Gotchas: every key is required and unknown keys fail, except the named removed and added
//!   ones, so a typo can't silently keep a default.

use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::key_bindings::{self, KeyBindings};
use crate::play_settings::{HitEffectSettings, MainMapSettings, MinimapSettings, ViewSettings};
use crate::visuals::{VISUALS_ROWS, VisualsSettings};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `BUILTIN_PLAYER_YAML` is Lunar Surface's `player_settings.yaml`, which every level's copy
/// equals (W-126, same play in every level), compiled in.
pub const BUILTIN_PLAYER_YAML: &str =
    include_str!("../../hyperbolic_wormholes_rust/player_settings.yaml");
const PLAYER_SETTINGS_HOME_PATH: &str = ".config/warped/player_settings.yaml";
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
const ADDED_PLAYER_KEYS: [&str; 8] = [
    "visuals",
    "keys.switch_view",
    "view.milky_way_brightness_frac",
    "view.head_bob.gun_walk_sway_m",
    "view.head_bob.gun_sprint_sway_m",
    "view.head_bob.rifle_walk_swing_rad",
    "view.head_bob.rifle_sprint_swing_rad",
    "main_map",
];
const AT_MOST_1_KEYS: [&str; 2] = ["view.wormhole_tint_frac", "view.milky_way_brightness_frac"];

/// `PLAYER_NUMBER_ROWS` is the numbers a settings page edits, the in-level menu's: each one's
/// dotted key in the file, its label, step, smallest and largest value.
pub const PLAYER_NUMBER_ROWS: [(&str, &str, f64, f64, f64); 13] = [
    (
        "view.look_rad_per_px",
        "mouse look rad/px",
        0.00025,
        0.0005,
        0.01,
    ),
    ("view.motion_blur_s", "motion blur s", 0.005, 0.0, 0.05),
    (
        "view.head_bob.walk_depth_m",
        "head bob walking m",
        0.01,
        0.0,
        0.3,
    ),
    (
        "view.head_bob.sprint_depth_m",
        "head bob sprinting m",
        0.01,
        0.0,
        0.4,
    ),
    ("hit_effects.red_frac", "hit red 0-1", 0.1, 0.0, 1.0),
    ("hit_effects.shake_px", "hit shake px", 1.0, 0.0, 20.0),
    ("hit_effects.blur_px", "hit blur px", 1.0, 0.0, 20.0),
    (
        "view.wormhole_tint_frac",
        "wormhole tint 0-1",
        0.1,
        0.0,
        1.0,
    ),
    (
        "view.milky_way_brightness_frac",
        "milky way 0-1",
        0.1,
        0.0,
        1.0,
    ),
    get_visuals_number_row("visuals.brightness", 0),
    get_visuals_number_row("visuals.contrast", 1),
    get_visuals_number_row("visuals.saturation", 2),
    get_visuals_number_row("visuals.gamma", 3),
];

/// `get_visuals_number_row` gives `PLAYER_NUMBER_ROWS`' row for `VISUALS_ROWS[index]` (W-51, basic
/// visuals), at the dotted `key` of its setting.
///
/// Example: `get_visuals_number_row("visuals.gamma", 3)`
const fn get_visuals_number_row(
    key: &'static str,
    index: usize,
) -> (&'static str, &'static str, f64, f64, f64) {
    let (_, label, step, low, high) = VISUALS_ROWS[index];
    (key, label, step, low, high)
}

/// `PlayerFile` is `player_settings.yaml` as written: the keys by name, the view, how a hit looks
/// and the minimap.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlayerFile {
    keys: BTreeMap<String, Vec<String>>,
    view: ViewSettings,
    hit_effects: HitEffectSettings,
    minimap: MinimapSettings,
    main_map: MainMapSettings,
    visuals: VisualsSettings,
}

/// `PerPlayerSettings` is everything in `player_settings.yaml`, with the keys read into controls.
pub struct PerPlayerSettings {
    pub keys: KeyBindings,
    pub view: ViewSettings,
    pub hit_effects: HitEffectSettings,
    pub minimap: MinimapSettings,
    pub main_map: MainMapSettings,
    pub visuals: VisualsSettings,
}

/// `get_saved_player_settings_path` gives where the menus save a player's settings and every
/// game loads them from: `~/.config/warped/player_settings.yaml`.
///
/// Example: `let path = get_saved_player_settings_path()?;`
pub fn get_saved_player_settings_path() -> Result<PathBuf> {
    let home: String =
        std::env::var("HOME").map_err(|error| format!("HOME isn't set ({error})"))?;
    Ok(PathBuf::from(home).join(PLAYER_SETTINGS_HOME_PATH))
}

/// `check_numbers` fails, naming the key and `source`, on the first number in `value` that is
/// negative or not finite, or zero outside `zero_allowed`.
///
/// Example: `check_numbers(&serde_json::to_value(&settings)?, &ZERO_ALLOWED_KEYS, "mine.yaml")?;`
pub fn check_numbers(value: &Value, zero_allowed: &[&str], source: &str) -> Result<()> {
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
/// Example: `let bad = find_bad_number(&value, "", &ZERO_ALLOWED_KEYS);`
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

/// `drop_removed_keys` deletes from a settings file's YAML each dotted key of `removed` (such as
/// `view.wormhole_tint_s`) that it has, so a file an older version saved still loads; every other
/// unknown key still fails.
///
/// Example: `drop_removed_keys(&mut yaml, &REMOVED_PLAYER_KEYS);`
pub fn drop_removed_keys(yaml: &mut serde_yaml::Value, removed: &[&str]) {
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
pub fn add_missing_keys(
    yaml: &mut serde_yaml::Value,
    defaults: &serde_yaml::Value,
    added: &[&str],
) {
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

impl PerPlayerSettings {
    /// `FromYaml` reads a player's settings from YAML text, naming `source` in errors: the keys
    /// removed since saves began are dropped and those added since take their built-in values,
    /// then `FromValue` checks the rest.
    ///
    /// Example: `let mine = PerPlayerSettings::FromYaml(&text, "mine.yaml")?;`
    #[allow(non_snake_case)]
    pub fn FromYaml(text: &str, source: &str) -> Result<PerPlayerSettings> {
        let mut yaml: serde_yaml::Value =
            serde_yaml::from_str(text).map_err(|error| format!("settings {source}: {error}"))?;
        drop_removed_keys(&mut yaml, &REMOVED_PLAYER_KEYS);
        let defaults: serde_yaml::Value = serde_yaml::from_str(BUILTIN_PLAYER_YAML)
            .expect("the built-in player_settings.yaml is YAML (tested)");
        add_missing_keys(&mut yaml, &defaults, &ADDED_PLAYER_KEYS);
        let value: Value =
            serde_json::to_value(&yaml).map_err(|error| format!("settings {source}: {error}"))?;
        PerPlayerSettings::FromValue(&value, source)
    }

    /// `FromValue` reads a player's settings from the file's keys as a JSON value (what a
    /// settings page edits), naming `source` in errors: the keys (`KeyBindings::FromNames`, after
    /// `key_bindings::upgrade_saved_names` upgrades keys saved before W-140), and
    /// every number checked as `check_numbers` does, with the tint and the Milky Way at most 1, and
    /// the visuals in their ranges (`VisualsSettings::check`).
    ///
    /// Example: `let mine = PerPlayerSettings::FromValue(&edited, "the settings page")?;`
    #[allow(non_snake_case)]
    pub fn FromValue(value: &Value, source: &str) -> Result<PerPlayerSettings> {
        let mut file: PlayerFile = serde_json::from_value(value.clone())
            .map_err(|error| format!("settings {source}: {error}"))?;
        key_bindings::upgrade_saved_names(&mut file.keys);
        let numbers: Value = serde_json::json!({"view": file.view, "hit_effects": file.hit_effects, "minimap": file.minimap, "main_map": file.main_map});
        check_numbers(&numbers, &PLAYER_ZERO_ALLOWED_KEYS, source)?;
        for key in AT_MOST_1_KEYS {
            let value_frac: f64 = key
                .split('.')
                .try_fold(&numbers, |node, part| node.get(part))
                .and_then(Value::as_f64)
                .expect("AT_MOST_1_KEYS name numbers of the file");
            if value_frac > 1.0 {
                return Err(format!(
                    "settings {source}: {key} must be at most 1, not {value_frac}"
                )
                .into());
            }
        }
        file.visuals.check(source)?;
        Ok(PerPlayerSettings {
            keys: KeyBindings::FromNames(&file.keys, source)?,
            view: file.view,
            hit_effects: file.hit_effects,
            minimap: file.minimap,
            main_map: file.main_map,
            visuals: file.visuals,
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
        let text: String = std::fs::read_to_string(path)
            .map_err(|error| format!("settings {}: {error}", path.display()))?;
        PerPlayerSettings::FromYaml(&text, &path.display().to_string())
    }

    /// `to_value` gives the settings as the file's keys in a JSON value, which `FromValue` reads.
    ///
    /// Example: `let mut edited = mine.to_value();`
    pub fn to_value(&self) -> Value {
        let file: PlayerFile = PlayerFile {
            keys: self.keys.to_names(),
            view: self.view.clone(),
            hit_effects: self.hit_effects.clone(),
            minimap: self.minimap.clone(),
            main_map: self.main_map,
            visuals: self.visuals,
        };
        serde_json::to_value(&file).expect("player settings serialize to JSON")
    }

    /// `to_yaml` gives the settings as YAML text that `Load` reads back (without the notes).
    ///
    /// Example: `std::fs::write(path, mine.to_yaml())?;`
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(&self.to_value()).expect("player settings serialize to YAML")
    }

    /// `save` writes the settings to `path`, making its folder, and fails naming the path.
    ///
    /// Example: `mine.save(&get_saved_player_settings_path()?)?;`
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)
                .map_err(|error| format!("can't make {}: {error}", folder.display()))?;
        }
        std::fs::write(path, self.to_yaml())
            .map_err(|error| format!("can't save {}: {error}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Tests of reading, checking and saving `player_settings.yaml`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Source | the built-in file, a saved file, an edited JSON value | `saved_settings_load_back`, `edited_values_are_checked` |
    //! | Keys | all present, one unknown, one missing (not an added one) | `saved_settings_load_back`, `unknown_and_missing_keys_fail` |
    //! | Numbers | above 0, 0 where allowed, 0 elsewhere, above 1 where at most 1 | `edited_values_are_checked` |
    //! | Removed key (`view.wormhole_tint_s`) | absent, present in an old save (dropped) | `old_saves_load` |
    //! | Added key (`keys.switch_view`, the `visuals` section) | present, missing in an old save (its built-in value) | `old_saves_load` |
    //! | Visuals (W-51, basic visuals) | in range (built in), out of range (fails, naming the key); rows match `VISUALS_ROWS` | `edited_values_are_checked`, `number_rows_name_numbers_in_range` |
    //! | Keys from before W-140 (`release_mouse`, `open_menu` on Tab) | upgraded to `show_map` and `open_menu` on Escape | `old_saves_load` |
    //! | Number rows | every row's key is a number of the file, inside its row's range | `number_rows_name_numbers_in_range` |
    //! | Same file as the levels | the match's copy of `player_settings.yaml` | `the_levels_use_the_same_file` |

    use super::*;
    use crate::key_bindings::{Action, Control};
    use sdl3::keyboard::Scancode;

    /// `get_number` gives the number at a dotted `key` of `value`.
    ///
    /// Example: `let x = get_number(&value, "view.motion_blur_s");`
    fn get_number(value: &Value, key: &str) -> Option<f64> {
        key.split('.')
            .try_fold(value, |node, part| node.get(part))
            .and_then(Value::as_f64)
    }

    #[test]
    fn saved_settings_load_back() {
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        mine.view.motion_blur_s = 0.0;
        mine.keys
            .set_controls(Action::Jump, vec![Control::Key(Scancode::J)])
            .expect("J can jump");
        let path: PathBuf = std::env::temp_dir().join(format!(
            "warped_player_settings_{}.yaml",
            std::process::id()
        ));
        mine.save(&path).expect("a save");
        let loaded: PerPlayerSettings = PerPlayerSettings::Load(&path).expect("a load");
        std::fs::remove_file(&path).expect("a removal");
        assert_eq!(loaded.to_yaml(), mine.to_yaml());
        assert_eq!(loaded.keys.get_names(Action::Jump), "J");
    }

    #[test]
    fn unknown_and_missing_keys_fail() {
        let unknown: String = BUILTIN_PLAYER_YAML.replace("motion_blur_s:", "blur_s:");
        assert!(PerPlayerSettings::FromYaml(&unknown, "test").is_err());
        let missing: String = BUILTIN_PLAYER_YAML.replace("  flashlight: [F]", "");
        let error: String = PerPlayerSettings::FromYaml(&missing, "test")
            .err()
            .expect("a missing key fails")
            .to_string();
        assert!(error.contains("flashlight"), "{error}");
    }

    #[test]
    fn edited_values_are_checked() {
        let builtin: Value = PerPlayerSettings::Builtin().to_value();
        let with = |key: &str, x: f64| -> Value {
            let mut value: Value = builtin.clone();
            let (section, name) = key.rsplit_once('.').expect("a dotted key");
            let parent: &mut Value = section
                .split('.')
                .try_fold(&mut value, |node, part| node.get_mut(part))
                .expect("the section exists");
            parent[name] = serde_json::json!(x);
            value
        };
        assert!(PerPlayerSettings::FromValue(&with("view.motion_blur_s", 0.0), "test").is_ok());
        assert!(PerPlayerSettings::FromValue(&with("view.motion_blur_s", 0.02), "test").is_ok());
        for (key, x) in [
            ("view.look_rad_per_px", 0.0),
            ("view.wormhole_tint_frac", 1.5),
            ("hit_effects.red_frac", -0.1),
            ("visuals.brightness", 9.0),
            ("visuals.saturation", -0.5),
        ] {
            let error: String = PerPlayerSettings::FromValue(&with(key, x), "test")
                .err()
                .expect("a bad number fails")
                .to_string();
            assert!(error.contains(key), "{error}");
        }
    }

    #[test]
    fn old_saves_load() {
        let old_tint: String = BUILTIN_PLAYER_YAML.replacen(
            "\n  wormhole_tint_frac: 1.0",
            "\n  wormhole_tint_frac: 1.0\n  wormhole_tint_s: 1.5",
            1,
        );
        assert_ne!(old_tint, BUILTIN_PLAYER_YAML);
        let without_switch_view: String = BUILTIN_PLAYER_YAML.replace("  switch_view: [V]", "");
        let before_held_map: String = BUILTIN_PLAYER_YAML
            .replacen("  show_map: [Tab]", "  release_mouse: [Escape]", 1)
            .replacen("  open_menu: [Escape]", "  open_menu: [Tab]", 1);
        assert!(!before_held_map.contains("show_map"));
        let without_visuals: String = BUILTIN_PLAYER_YAML
            .split("\nvisuals:")
            .next()
            .expect("a first part")
            .to_string();
        assert!(!without_visuals.contains("brightness:"));
        for old_save in [
            old_tint,
            without_switch_view,
            before_held_map,
            without_visuals,
        ] {
            let loaded: PerPlayerSettings =
                PerPlayerSettings::FromYaml(&old_save, "test").expect("an old save loads");
            assert_eq!(loaded.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
        }
    }

    #[test]
    fn number_rows_name_numbers_in_range() {
        let builtin: Value = PerPlayerSettings::Builtin().to_value();
        for (key, label, step, low, high) in PLAYER_NUMBER_ROWS {
            let x: f64 = get_number(&builtin, key).unwrap_or_else(|| panic!("{key} is a number"));
            assert!(low <= x && x <= high && step > 0.0, "{label}: {x}");
        }
        for (knob, key, ..) in VISUALS_ROWS {
            let row = PLAYER_NUMBER_ROWS
                .iter()
                .find(|row| row.0 == format!("visuals.{key}"))
                .unwrap_or_else(|| panic!("a row for visuals.{key}"));
            assert_eq!(row.1, key, "{knob:?}");
        }
    }

    #[test]
    fn the_levels_use_the_same_file() {
        let match_copy: &str = include_str!("../../warped_match_rust/player_settings.yaml");
        assert_eq!(match_copy, BUILTIN_PLAYER_YAML);
    }
}
