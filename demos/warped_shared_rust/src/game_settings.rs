//! `game_settings.rs` is the game master's settings (W-158, per-map game settings): the match
//! server keeps each level's default server settings (its settings file) and the game master's
//! overrides of them, one shared section laid over every level that has the key, then one section
//! per level; it answers the clients that ask, and writes each round's level its merged settings.
//!
//! Interface: `GameSettings` (`New`, `Load`, `get_shared_keys`, `get_level_keys`, `get_default`,
//! `get_value`, `apply`, `get_overrides`, `find_problem`, `to_setting_value`,
//! `get_level_settings`, `to_answer`, `FromAnswer`), `SettingsScope`, `SettingsMessage`,
//! `Overrides`, `Refusal`, `make_settings_message`, `parse_settings_message`,
//! `read_settings_file`, `parse_settings_text`.
//!
//! Example: `settings.apply(&parse_settings_message(b"set shared respawn.delay_s 5", 5)?)?;`
//!
//! Must know:
//! - The game master saves every override at once (`SettingsMessage::Save`): the match server
//!   takes the save only if every level's settings then keep the rules of `setting_rules.rs`, and
//!   otherwise keeps what it had and answers with the reason (`GameSettings::refusal`). `Set` and
//!   `Reset`, one key at a time and unchecked beyond the number itself, stay for older apps.
//! - Gotchas: a key is a dotted path to a number or a switch (`true` or `false`, set with 1 or
//!   0) in a settings file (`movement.jump_height_m`, `shooting.is_friendly_fire_on`); a key at
//!   least two levels' files have is shared, any other is its level's own.
//! - Gotchas: a client asks with `settings?`, `set <scope> <key> <value>` or `reset <scope> <key>`
//!   (scope `shared` or a level's index), and only those clients get `settings <json>` back, so a
//!   client older than this module never sees one.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::match_protocol::LevelEntry;
use crate::setting_rules::check_settings;
use crate::settings_editor::{get_at, set_at};

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const SETTINGS_REQUEST: &str = "settings?";
const SETTINGS_ANSWER_PREFIX: &str = "settings ";
const SETTINGS_SAVE_PREFIX: &str = "save ";
const SHARED_SCOPE: &str = "shared";
const SERVER_SETTINGS_FLAG: &str = "--server-settings";
const MIN_LEVELS_PER_SHARED_KEY: usize = 2;

/// `SettingsScope` is which section of the game settings an override is in: the shared one, laid
/// over every level that has the key, or one level's (its index in the levels file).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsScope {
    Shared,
    Level(usize),
}

/// `SettingsMessage` is what a client sends about the game settings: a request for them, an
/// override of one key in a scope, or a reset of one key there (back to the layer below).
#[derive(Clone, PartialEq, Debug)]
pub enum SettingsMessage {
    Request,
    Set {
        scope: SettingsScope,
        key: String,
        value: f64,
    },
    Reset {
        scope: SettingsScope,
        key: String,
    },
    Save(Overrides),
}

/// `Overrides` is every override of the game settings at once, as the game master saves them:
/// the shared ones and each level's own, each a dotted key to its value.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Overrides {
    pub shared: BTreeMap<String, Value>,
    pub levels: Vec<BTreeMap<String, Value>>,
}

/// `Refusal` is why the match server didn't take the last save: the overrides it refused (so a
/// client knows the refusal is of its own save), the level whose settings would break a rule (its
/// index in the levels file; `None` when the save itself was wrong), and the reason.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Refusal {
    pub overrides: Overrides,
    pub level: Option<usize>,
    pub reason: String,
}

/// `format_scope` gives a scope as messages write it: `shared` or the level's index.
///
/// Example: `assert_eq!(format_scope(SettingsScope::Level(2)), "2");`
fn format_scope(scope: SettingsScope) -> String {
    match scope {
        SettingsScope::Shared => SHARED_SCOPE.to_string(),
        SettingsScope::Level(level) => level.to_string(),
    }
}

/// `parse_scope` reads a scope word, checking a level's index against `level_count`.
///
/// Example: `assert_eq!(parse_scope("shared", 5)?, SettingsScope::Shared);`
fn parse_scope(word: &str, level_count: usize) -> Result<SettingsScope> {
    if word == SHARED_SCOPE {
        return Ok(SettingsScope::Shared);
    }
    match word.parse::<usize>() {
        Ok(level) if level < level_count => Ok(SettingsScope::Level(level)),
        _ => Err(format!(
            "settings scope {word:?}: expected {SHARED_SCOPE} or a level's index below {level_count}"
        )
        .into()),
    }
}

/// `make_settings_message` gives the datagram `parse_settings_message` reads for `message`.
///
/// Example: `socket.send_to(&make_settings_message(&SettingsMessage::Request), server)?;`
pub fn make_settings_message(message: &SettingsMessage) -> Vec<u8> {
    match message {
        SettingsMessage::Request => SETTINGS_REQUEST.to_string(),
        SettingsMessage::Set { scope, key, value } => {
            format!("set {} {key} {value}", format_scope(*scope))
        }
        SettingsMessage::Reset { scope, key } => format!("reset {} {key}", format_scope(*scope)),
        SettingsMessage::Save(overrides) => format!(
            "{SETTINGS_SAVE_PREFIX}{}",
            serde_json::to_string(overrides).expect("overrides serialize to JSON")
        ),
    }
    .into_bytes()
}

/// `parse_settings_message` reads a client's `settings?`, `set <scope> <key> <value>`,
/// `reset <scope> <key>` or `save <overrides as JSON>` datagram for a game of `level_count`
/// levels; any other datagram is an error.
///
/// Example: `let message = parse_settings_message(b"reset 4 glass.range_m", 5)?;`
pub fn parse_settings_message(datagram: &[u8], level_count: usize) -> Result<SettingsMessage> {
    let text: &str = std::str::from_utf8(datagram).map_err(|_| "not a settings message")?;
    if let Some(json) = text.strip_prefix(SETTINGS_SAVE_PREFIX) {
        let overrides: Overrides =
            serde_json::from_str(json).map_err(|error| format!("save: {error}"))?;
        if overrides.levels.len() != level_count {
            return Err(format!(
                "save: overrides for {} levels, but the game has {level_count}",
                overrides.levels.len()
            )
            .into());
        }
        return Ok(SettingsMessage::Save(overrides));
    }
    let words: Vec<&str> = text.split(' ').collect();
    match words.as_slice() {
        [SETTINGS_REQUEST] => Ok(SettingsMessage::Request),
        ["set", scope, key, value] => Ok(SettingsMessage::Set {
            scope: parse_scope(scope, level_count)?,
            key: key.to_string(),
            value: value
                .parse()
                .map_err(|_| format!("set {key}: {value:?} isn't a number"))?,
        }),
        ["reset", scope, key] => Ok(SettingsMessage::Reset {
            scope: parse_scope(scope, level_count)?,
            key: key.to_string(),
        }),
        _ => Err("not a settings message".into()),
    }
}

/// `read_settings_file` reads a server settings file (YAML, or JSON, which is YAML too) as a JSON
/// value.
///
/// Example: `let defaults: Value = read_settings_file(Path::new("server_settings.yaml"))?;`
pub fn read_settings_file(path: &Path) -> Result<Value> {
    let text: String = fs::read_to_string(path)
        .map_err(|error| format!("can't read the settings file {}: {error}", path.display()))?;
    parse_settings_text(&text, &path.display().to_string())
}

/// `parse_settings_text` reads the text of a server settings file from `source` (named in its
/// errors) as a JSON value, failing on anything but a map of sections.
///
/// Example: `let builtin: Value = parse_settings_text(BUILTIN_YAML, "server_settings.yaml")?;`
pub fn parse_settings_text(text: &str, source: &str) -> Result<Value> {
    let value: Value = serde_yaml::from_str(text)
        .map_err(|error| format!("{source} isn't a settings file: {error}"))?;
    if !value.is_object() {
        return Err(format!("{source} isn't a settings file: not a map of sections").into());
    }
    Ok(value)
}

/// `push_number_keys` adds the dotted path of every number in `value` under `prefix` to `keys`.
///
/// Example: `push_number_keys(&defaults, "", &mut keys);`
fn push_number_keys(value: &Value, prefix: &str, keys: &mut Vec<String>) {
    match value {
        Value::Number(_) | Value::Bool(_) => keys.push(prefix.to_string()),
        Value::Object(fields) => {
            for (name, field) in fields {
                let key: String = match prefix {
                    "" => name.clone(),
                    _ => format!("{prefix}.{name}"),
                };
                push_number_keys(field, &key, keys);
            }
        }
        _ => {}
    }
}

/// `get_number_keys` gives the dotted path of every number in `value`, sorted.
///
/// Example: `let keys = get_number_keys(&defaults);`
fn get_number_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    push_number_keys(value, "", &mut keys);
    keys.sort();
    keys
}

/// `GameSettings` is the game's settings as the match server keeps them: each level's default
/// server settings (`None` for a level without a settings file), the shared overrides, each
/// level's own overrides, each a dotted key to a number, and why the last save was refused, if it
/// was.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct GameSettings {
    pub defaults: Vec<Option<Value>>,
    pub shared: BTreeMap<String, Value>,
    pub levels: Vec<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Refusal>,
}

impl GameSettings {
    /// `New` keeps `defaults`, one per level, with no overrides.
    ///
    /// Example: `let settings = GameSettings::New(vec![Some(json!({"respawn": {"delay_s": 3.0}})), None]);`
    #[allow(non_snake_case)]
    pub fn New(defaults: Vec<Option<Value>>) -> GameSettings {
        let level_count: usize = defaults.len();
        GameSettings {
            defaults,
            shared: BTreeMap::new(),
            levels: vec![BTreeMap::new(); level_count],
            refusal: None,
        }
    }

    /// `get_overrides` gives every override as a save carries them.
    ///
    /// Example: `let overrides: Overrides = settings.get_overrides();`
    pub fn get_overrides(&self) -> Overrides {
        Overrides {
            shared: self.shared.clone(),
            levels: self.levels.clone(),
        }
    }

    /// `find_problem` gives the first level whose settings, with the overrides laid over its
    /// defaults, break a rule (`setting_rules::check_settings`), and the reason; `None` when
    /// every level's settings are fine.
    ///
    /// Example: `if let Some((level, reason)) = settings.find_problem() { ... }`
    pub fn find_problem(&self) -> Option<(usize, String)> {
        (0..self.defaults.len()).find_map(|level| {
            let defaults: &Value = self.defaults[level].as_ref()?;
            let merged: Value = self.get_level_settings(level)?;
            check_settings(&merged, defaults)
                .err()
                .map(|reason| (level, reason))
        })
    }

    /// `to_setting_value` gives `value`, a number a client sent for `key`, as the settings hold
    /// it beside `default`: a switch for a switch (0 or 1), a whole number where the default is
    /// whole, else the number; it fails on a number that isn't finite and at least 0.
    ///
    /// Example: `let held: Value = GameSettings::to_setting_value("glass.max_bounce_count", &json!(2), 3.0)?;`
    pub fn to_setting_value(key: &str, default: &Value, value: f64) -> Result<Value> {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("{key} must be a number of at least 0, not {value}").into());
        }
        match (default.is_boolean(), default.is_u64()) {
            (true, _) if value != 0.0 && value != 1.0 => {
                Err(format!("{key} is a switch: 0 (off) or 1 (on), not {value}").into())
            }
            (true, _) => Ok(Value::Bool(value == 1.0)),
            (false, true) if value.fract() != 0.0 => {
                Err(format!("{key} must be a whole number, not {value}").into())
            }
            (false, true) => Ok(Value::from(value as u64)),
            (false, false) => Ok(Value::from(value)),
        }
    }

    /// `save` replaces every override with `overrides` if each names a key its scope has and
    /// every level's settings then keep the rules; otherwise it changes nothing but `refusal`,
    /// which says why, and fails with the reason.
    ///
    /// Example: `settings.save(&overrides)?;`
    fn save(&mut self, overrides: &Overrides) -> Result<()> {
        let mut candidate: GameSettings = self.clone();
        candidate.shared = overrides.shared.clone();
        candidate.levels = overrides.levels.clone();
        candidate.refusal = None;
        let shared_keys: Vec<String> = self.get_shared_keys();
        let unknown: Option<String> = overrides
            .shared
            .keys()
            .find(|key| !shared_keys.contains(key))
            .cloned()
            .or_else(|| {
                overrides
                    .levels
                    .iter()
                    .enumerate()
                    .find_map(|(level, own)| {
                        let keys: Vec<String> = self.defaults[level]
                            .as_ref()
                            .map_or(Vec::new(), get_number_keys);
                        own.keys().find(|key| !keys.contains(key)).cloned()
                    })
            });
        let refusal: Option<Refusal> = match unknown {
            Some(key) => Some(Refusal {
                overrides: overrides.clone(),
                level: None,
                reason: format!("the game settings have no {key} there"),
            }),
            None => candidate.find_problem().map(|(level, reason)| Refusal {
                overrides: overrides.clone(),
                level: Some(level),
                reason,
            }),
        };
        match refusal {
            Some(refusal) => {
                let reason: String = refusal.reason.clone();
                self.refusal = Some(refusal);
                Err(reason.into())
            }
            None => {
                *self = candidate;
                Ok(())
            }
        }
    }

    /// `Load` reads each level's `server_settings` file, a path from `manifest_folder` (the
    /// levels file's folder). It fails on a file that doesn't read, and on a level that also
    /// passes `--server-settings` in its `server_args` (it would get the flag twice).
    ///
    /// Example: `let settings = GameSettings::Load(&levels, Path::new("."))?;`
    #[allow(non_snake_case)]
    pub fn Load(levels: &[LevelEntry], manifest_folder: &Path) -> Result<GameSettings> {
        let mut defaults: Vec<Option<Value>> = Vec::new();
        for level in levels {
            let Some(path) = &level.server_settings else {
                defaults.push(None);
                continue;
            };
            if level
                .server_args
                .iter()
                .any(|arg| arg == SERVER_SETTINGS_FLAG)
            {
                return Err(format!(
                    "{} has both server_settings and {SERVER_SETTINGS_FLAG} in its server_args; keep one",
                    level.name
                )
                .into());
            }
            defaults.push(Some(read_settings_file(&manifest_folder.join(path))?));
        }
        Ok(GameSettings::New(defaults))
    }

    /// `get_shared_keys` gives the keys at least two levels' defaults have, sorted.
    ///
    /// Example: `let keys = settings.get_shared_keys();`
    pub fn get_shared_keys(&self) -> Vec<String> {
        let mut level_counts: BTreeMap<String, usize> = BTreeMap::new();
        for defaults in self.defaults.iter().flatten() {
            for key in get_number_keys(defaults) {
                *level_counts.entry(key).or_default() += 1;
            }
        }
        level_counts
            .into_iter()
            .filter(|(_, count)| *count >= MIN_LEVELS_PER_SHARED_KEY)
            .map(|(key, _)| key)
            .collect()
    }

    /// `get_level_keys` gives the keys only `level`'s defaults have, sorted: its section of the
    /// game settings page (empty for a level with none of its own, or no settings file).
    ///
    /// Example: `let keys = settings.get_level_keys(2);`
    pub fn get_level_keys(&self, level: usize) -> Vec<String> {
        let shared: Vec<String> = self.get_shared_keys();
        self.defaults[level]
            .as_ref()
            .map_or(Vec::new(), get_number_keys)
            .into_iter()
            .filter(|key| !shared.contains(key))
            .collect()
    }

    /// `get_default` gives `key`'s default in `scope`: the level's, or for the shared scope the
    /// value most levels with the key have (the first such level's on a tie); `None` if no level
    /// in the scope has it.
    ///
    /// Example: `let walk = settings.get_default(SettingsScope::Shared, "movement.walk_speed_per_map_speed");`
    pub fn get_default(&self, scope: SettingsScope, key: &str) -> Option<&Value> {
        let values: Vec<&Value> = match scope {
            SettingsScope::Shared => self
                .defaults
                .iter()
                .flatten()
                .filter_map(|defaults| get_at(defaults, key))
                .collect(),
            SettingsScope::Level(level) => self.defaults[level]
                .as_ref()
                .and_then(|defaults| get_at(defaults, key))
                .into_iter()
                .collect(),
        };
        values
            .iter()
            .max_by_key(|value| {
                let count: usize = values.iter().filter(|other| other == value).count();
                let first: usize = values.iter().position(|other| other == *value).unwrap_or(0);
                (count, usize::MAX - first)
            })
            .copied()
    }

    /// `get_value` gives `key` in `scope` as the game plays it: the scope's override, or its
    /// default.
    ///
    /// Example: `let delay = settings.get_value(SettingsScope::Shared, "respawn.delay_s");`
    pub fn get_value(&self, scope: SettingsScope, key: &str) -> Option<&Value> {
        let overrides: &BTreeMap<String, Value> = match scope {
            SettingsScope::Shared => &self.shared,
            SettingsScope::Level(level) => &self.levels[level],
        };
        overrides.get(key).or_else(|| self.get_default(scope, key))
    }

    /// `apply` carries out a `Set`, a `Reset` or a `Save` (a `Request` changes nothing). A set
    /// must name a key the scope has (a shared key, or any key of the level's), with a finite
    /// number of at least 0, whole where the default is; a save is taken whole or not at all
    /// (`save`).
    ///
    /// Example: `settings.apply(&SettingsMessage::Reset { scope: SettingsScope::Shared, key })?;`
    pub fn apply(&mut self, message: &SettingsMessage) -> Result<()> {
        match message {
            SettingsMessage::Request => Ok(()),
            SettingsMessage::Save(overrides) => self.save(overrides),
            SettingsMessage::Reset { scope, key } => {
                match scope {
                    SettingsScope::Shared => self.shared.remove(key),
                    SettingsScope::Level(level) => self.levels[*level].remove(key),
                };
                Ok(())
            }
            SettingsMessage::Set { scope, key, value } => {
                let is_in_scope: bool = match scope {
                    SettingsScope::Shared => self.get_shared_keys().contains(key),
                    SettingsScope::Level(level) => self.defaults[*level]
                        .as_ref()
                        .is_some_and(|defaults| get_number_keys(defaults).contains(key)),
                };
                let default: &Value = self
                    .get_default(*scope, key)
                    .filter(|_| is_in_scope)
                    .ok_or_else(|| format!("the game settings have no {key} there"))?;
                let number: Value = GameSettings::to_setting_value(key, default, *value)?;
                match scope {
                    SettingsScope::Shared => self.shared.insert(key.clone(), number),
                    SettingsScope::Level(level) => self.levels[*level].insert(key.clone(), number),
                };
                Ok(())
            }
        }
    }

    /// `get_level_settings` gives the settings `level` plays by: its defaults with the shared
    /// overrides of keys it has laid over them, then its own; `None` for a level without a
    /// settings file.
    ///
    /// Example: `let merged = settings.get_level_settings(0);`
    pub fn get_level_settings(&self, level: usize) -> Option<Value> {
        let mut merged: Value = self.defaults[level].clone()?;
        for (key, value) in self.shared.iter().chain(&self.levels[level]) {
            if get_at(&merged, key).is_some() {
                set_at(&mut merged, key, value.clone()).expect("a key the settings have");
            }
        }
        Some(merged)
    }

    /// `to_answer` gives the `settings <json>` datagram the match server answers a settings
    /// message with.
    ///
    /// Example: `socket.send_to(&settings.to_answer(), client)?;`
    pub fn to_answer(&self) -> Vec<u8> {
        let mut answer: Vec<u8> = SETTINGS_ANSWER_PREFIX.as_bytes().to_vec();
        answer.extend(serde_json::to_vec(self).expect("game settings serialize to JSON"));
        answer
    }

    /// `FromAnswer` reads a `settings <json>` datagram: `None` for any other datagram (a match
    /// view), else the settings or why they don't read.
    ///
    /// Example: `if let Some(settings) = GameSettings::FromAnswer(&datagram) { ... }`
    #[allow(non_snake_case)]
    pub fn FromAnswer(datagram: &[u8]) -> Option<Result<GameSettings>> {
        let json: &[u8] = datagram.strip_prefix(SETTINGS_ANSWER_PREFIX.as_bytes())?;
        Some(serde_json::from_slice(json).map_err(|error| error.into()))
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `game_settings.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Messages | request, set (shared, a level), reset (shared, a level), past the last level, not a number, garbage | `messages_round_trip` |
    //! | Sections | a key in every level (shared), in two of three (shared), in one (its level's), a level without a file (empty) | `keys_split_into_shared_and_each_levels_own` |
    //! | Shared default | levels agree, most levels agree, a tie (the first level's) | `shared_defaults_are_the_most_common_value` |
    //! | Set | shared, a level's own key, a shared key for one level, a switch off; unknown key, a level's own key as shared, negative, infinite, a fraction where the default is whole, a switch at 0.5 | `sets_are_checked` |
    //! | Merge | no overrides (the defaults), shared over a level that has the key and not one that lacks it, a level's own over the shared, a reset, no file | `levels_play_by_their_merged_settings` |
    //! | Answer | written then read, a view isn't one; with a refusal, and one from an older server without the field | `answers_round_trip` |
    //! | Save | taken whole (overrides replaced, an earlier refusal cleared); a value breaking its kind; a group broken across the shared and a level's overrides; a key the scope lacks; overrides for another level count | `a_save_is_taken_whole_or_refused_with_the_reason` |
    //! | Files | YAML read, not a map, the same flag twice | `settings_files_load` |

    use super::*;
    use serde_json::json;

    /// `make_settings` gives three levels: all with `respawn.delay_s` (3, 3, 5), the first two
    /// with `movement.walk` (2.0, 1.2), whole `rifle.hits` and the switch `shooting.on`, the third
    /// alone with `glass.bounces`.
    ///
    /// Example: `let settings = make_settings();`
    fn make_settings() -> GameSettings {
        GameSettings::New(vec![
            Some(
                json!({"respawn": {"delay_s": 3.0}, "movement": {"walk": 2.0}, "rifle": {"hits": 2}, "shooting": {"on": true}}),
            ),
            Some(
                json!({"respawn": {"delay_s": 3.0}, "movement": {"walk": 1.2}, "rifle": {"hits": 2}, "shooting": {"on": true}}),
            ),
            Some(json!({"respawn": {"delay_s": 5.0}, "glass": {"bounces": 2}})),
        ])
    }

    /// `make_set` gives a set of `key` to `value` in `scope`.
    ///
    /// Example: `settings.apply(&make_set(SettingsScope::Shared, "respawn.delay_s", 4.0))?;`
    fn make_set(scope: SettingsScope, key: &str, value: f64) -> SettingsMessage {
        SettingsMessage::Set {
            scope,
            key: key.to_string(),
            value,
        }
    }

    #[test]
    fn messages_round_trip() {
        for message in [
            SettingsMessage::Request,
            make_set(SettingsScope::Shared, "respawn.delay_s", 4.5),
            make_set(SettingsScope::Level(4), "glass.max_bounce_count", 3.0),
            SettingsMessage::Reset {
                scope: SettingsScope::Shared,
                key: "respawn.delay_s".to_string(),
            },
            SettingsMessage::Reset {
                scope: SettingsScope::Level(0),
                key: "surface.gravity_per_jump_gravity".to_string(),
            },
        ] {
            let datagram: Vec<u8> = make_settings_message(&message);
            assert_eq!(parse_settings_message(&datagram, 5).unwrap(), message);
        }
        assert!(parse_settings_message(b"set 5 a.b 1", 5).is_err());
        assert!(parse_settings_message(b"set shared a.b fast", 5).is_err());
        assert!(parse_settings_message(b"hello blue", 5).is_err());
        assert!(parse_settings_message(b"play 2", 5).is_err());
    }

    #[test]
    fn keys_split_into_shared_and_each_levels_own() {
        let mut settings: GameSettings = make_settings();
        settings.defaults.push(None);
        settings.levels.push(BTreeMap::new());
        assert_eq!(
            settings.get_shared_keys(),
            [
                "movement.walk",
                "respawn.delay_s",
                "rifle.hits",
                "shooting.on"
            ]
        );
        assert!(settings.get_level_keys(0).is_empty());
        assert_eq!(settings.get_level_keys(2), ["glass.bounces"]);
        assert!(settings.get_level_keys(3).is_empty());
    }

    #[test]
    fn shared_defaults_are_the_most_common_value() {
        let settings: GameSettings = make_settings();
        let shared = |key: &str| settings.get_default(SettingsScope::Shared, key).cloned();
        assert_eq!(shared("rifle.hits"), Some(json!(2)));
        assert_eq!(shared("respawn.delay_s"), Some(json!(3.0)));
        assert_eq!(shared("movement.walk"), Some(json!(2.0)));
        assert_eq!(
            settings.get_default(SettingsScope::Level(1), "movement.walk"),
            Some(&json!(1.2))
        );
    }

    #[test]
    fn sets_are_checked() {
        let mut settings: GameSettings = make_settings();
        settings
            .apply(&make_set(SettingsScope::Shared, "respawn.delay_s", 4.0))
            .unwrap();
        settings
            .apply(&make_set(SettingsScope::Level(2), "glass.bounces", 3.0))
            .unwrap();
        settings
            .apply(&make_set(SettingsScope::Level(1), "movement.walk", 1.5))
            .unwrap();
        assert_eq!(settings.shared["respawn.delay_s"], json!(4.0));
        assert_eq!(settings.levels[2]["glass.bounces"], json!(3));
        settings
            .apply(&make_set(SettingsScope::Shared, "shooting.on", 0.0))
            .unwrap();
        assert_eq!(settings.shared["shooting.on"], json!(false));
        assert_eq!(
            settings.get_value(SettingsScope::Level(1), "movement.walk"),
            Some(&json!(1.5))
        );
        for refused in [
            make_set(SettingsScope::Shared, "respawn.delay_ms", 4.0),
            make_set(SettingsScope::Shared, "glass.bounces", 1.0),
            make_set(SettingsScope::Shared, "respawn.delay_s", -1.0),
            make_set(SettingsScope::Shared, "respawn.delay_s", f64::INFINITY),
            make_set(SettingsScope::Shared, "rifle.hits", 2.5),
            make_set(SettingsScope::Shared, "shooting.on", 0.5),
        ] {
            assert!(settings.apply(&refused).is_err(), "{refused:?}");
        }
    }

    #[test]
    fn levels_play_by_their_merged_settings() {
        let mut settings: GameSettings = make_settings();
        assert_eq!(settings.get_level_settings(2), settings.defaults[2]);
        settings
            .apply(&make_set(SettingsScope::Shared, "movement.walk", 3.0))
            .unwrap();
        settings
            .apply(&make_set(SettingsScope::Shared, "respawn.delay_s", 4.0))
            .unwrap();
        settings
            .apply(&make_set(SettingsScope::Level(0), "respawn.delay_s", 6.0))
            .unwrap();
        let merged: Vec<Value> = (0..3)
            .map(|level| settings.get_level_settings(level).unwrap())
            .collect();
        assert_eq!(merged[0]["movement"]["walk"], json!(3.0));
        assert_eq!(merged[0]["respawn"]["delay_s"], json!(6.0));
        assert_eq!(merged[1]["respawn"]["delay_s"], json!(4.0));
        assert_eq!(merged[2]["respawn"]["delay_s"], json!(4.0));
        assert!(merged[2].get("movement").is_none());
        settings
            .apply(&SettingsMessage::Reset {
                scope: SettingsScope::Level(0),
                key: "respawn.delay_s".to_string(),
            })
            .unwrap();
        assert_eq!(
            settings.get_level_settings(0).unwrap()["respawn"]["delay_s"],
            json!(4.0)
        );
        assert_eq!(GameSettings::New(vec![None]).get_level_settings(0), None);
    }

    #[test]
    fn answers_round_trip() {
        let mut settings: GameSettings = make_settings();
        settings
            .apply(&make_set(SettingsScope::Level(2), "glass.bounces", 0.0))
            .unwrap();
        let answer: Vec<u8> = settings.to_answer();
        assert_eq!(
            GameSettings::FromAnswer(&answer).unwrap().unwrap(),
            settings
        );
        assert!(GameSettings::FromAnswer(b"{\"phase\": \"Over\"}").is_none());
        settings.refusal = Some(Refusal {
            overrides: settings.get_overrides(),
            level: Some(1),
            reason: "respawn.delay_s must be above 0, not 0".to_string(),
        });
        assert_eq!(
            GameSettings::FromAnswer(&settings.to_answer())
                .unwrap()
                .unwrap(),
            settings
        );
        let older: &[u8] = b"settings {\"defaults\":[null],\"shared\":{},\"levels\":[{}]}";
        assert_eq!(
            GameSettings::FromAnswer(older).unwrap().unwrap(),
            GameSettings::New(vec![None])
        );
    }

    #[test]
    fn a_save_is_taken_whole_or_refused_with_the_reason() {
        let speckle: Value = json!({"beam_speckle": {"black_frac": 0.1, "white_frac": 0.1, "dropout_frac": 0.8}, "respawn": {"delay_s": 3.0}});
        let mut settings: GameSettings =
            GameSettings::New(vec![Some(speckle.clone()), Some(speckle), None]);
        let make_save = |shared: &[(&str, Value)], first: &[(&str, Value)]| -> SettingsMessage {
            let to_map = |pairs: &[(&str, Value)]| -> BTreeMap<String, Value> {
                pairs
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.clone()))
                    .collect()
            };
            SettingsMessage::Save(Overrides {
                shared: to_map(shared),
                levels: vec![to_map(first), BTreeMap::new(), BTreeMap::new()],
            })
        };
        let good: SettingsMessage = make_save(
            &[("beam_speckle.black_frac", json!(0.3))],
            &[("beam_speckle.dropout_frac", json!(0.6))],
        );
        let datagram: Vec<u8> = make_settings_message(&good);
        assert_eq!(parse_settings_message(&datagram, 3).unwrap(), good);
        assert!(parse_settings_message(&datagram, 5).is_err());
        let error: String = settings.apply(&good).unwrap_err().to_string();
        assert!(error.contains("must add up to 1"), "{error}");
        assert!(settings.shared.is_empty());
        assert_eq!(settings.refusal.as_ref().unwrap().level, Some(1));
        let kind: SettingsMessage = make_save(&[("respawn.delay_s", json!(0.0))], &[]);
        assert_eq!(
            settings.apply(&kind).unwrap_err().to_string(),
            "respawn.delay_s must be above 0, not 0"
        );
        assert_eq!(settings.refusal.as_ref().unwrap().level, Some(0));
        let unknown: SettingsMessage = make_save(&[("glass.bounces", json!(1))], &[]);
        assert!(settings.apply(&unknown).is_err());
        assert_eq!(settings.refusal.as_ref().unwrap().level, None);
        let whole: SettingsMessage = make_save(
            &[
                ("beam_speckle.black_frac", json!(0.3)),
                ("beam_speckle.dropout_frac", json!(0.6)),
            ],
            &[("respawn.delay_s", json!(7.5))],
        );
        settings.apply(&whole).unwrap();
        assert_eq!(settings.refusal, None);
        assert_eq!(settings.shared.len(), 2);
        assert_eq!(settings.find_problem(), None);
        assert_eq!(
            settings.get_level_settings(0).unwrap()["respawn"]["delay_s"],
            7.5
        );
        assert_eq!(settings.get_overrides().levels[0].len(), 1);
    }

    #[test]
    fn settings_files_load() {
        let folder = std::env::temp_dir().join(format!("game_settings_{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("a.yaml"),
            "respawn:\n  delay_s: 3.0  # a note\n",
        )
        .unwrap();
        fs::write(folder.join("b.yaml"), "- 1\n").unwrap();
        let level: LevelEntry = serde_yaml::from_str(
            "{name: A, folder: a, branch: b, status: s, program: self, server_settings: a.yaml}",
        )
        .unwrap();
        let loaded: GameSettings =
            GameSettings::Load(std::slice::from_ref(&level), &folder).unwrap();
        assert_eq!(
            loaded.defaults,
            [Some(json!({"respawn": {"delay_s": 3.0}}))]
        );
        assert!(read_settings_file(&folder.join("b.yaml")).is_err());
        let twice: LevelEntry = LevelEntry {
            server_args: vec![SERVER_SETTINGS_FLAG.to_string(), "x.yaml".to_string()],
            ..level
        };
        assert!(GameSettings::Load(&[twice], &folder).is_err());
        fs::remove_dir_all(&folder).unwrap();
    }
}
