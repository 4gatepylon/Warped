//! `game_settings_page.rs` is the game master's settings page, opened from the map menu (W-158,
//! per-map game settings): the game server's game settings (`warped_shared::game_settings`) in
//! sections, the shared one (every level that has the key) and one per level (its own keys, shown
//! even when it has none), edited with the shared settings editor. Changes and resets stay on the
//! page until the game master chooses SAVE SETTINGS (W-209, game settings checked when saved,
//! with the reason): the page then checks every level's settings against the rules
//! (`warped_shared::setting_rules`) and either says which rule is broken, where and why, or sends
//! them all to the game server, which checks them again, holds them and gives each round's level
//! server its merged copy.
//!
//! Interface: `GameSettingsPage` (`New`, `update`, `handle`, `save`, `select`,
//! `has_unsaved_changes`, `show_message`, `get_shapes`, `is_waiting`; in tests `get_row_keys`),
//! `GameSettingsOutcome`.
//!
//! Example: `if let GameSettingsOutcome::Send(message) = page.handle(MenuInput::Right) { client.send_settings(&message)?; }`
//!
//! Must know:
//! - Gotchas: the page's rows come from the keys the levels' settings files have, so settings that
//!   other changes add show up without changing this file.
//! - Gotchas: a switch (`true` or `false` in the file) shows as 1 or 0, stepping between them.
//! - Gotchas: while a number is typed or stepped only "a finite number of at least 0, whole where
//!   the default is" is asked of it, so a group can be changed one setting at a time (three
//!   shares that must add up to 1); the rules are checked when the settings are saved.
//! - Gotchas: Escape with unsaved changes asks once; a second Escape leaves and drops them.
//! - Gotchas: a save the game server doesn't answer within `SAVE_ANSWER_TIMEOUT_S` (a match
//!   server from before saves answers with nothing) is reported as not saved; the changes stay.

use std::error::Error;
use std::time::Instant;

use serde_json::{Map, Value};
use warped_shared::game_settings::{GameSettings, Overrides, SettingsMessage, SettingsScope};
use warped_shared::page::Shape;
use warped_shared::settings_editor::{EditorOutcome, EditorRow, RowKind, SettingsEditor, get_at};

use crate::menu_page::{
    LINE, MARGIN, MenuInput, MenuRow, get_editor_input, page_t, push_heading, push_rows, push_title,
};

const PAGE_SIZE: [f32; 2] = [800.0, 455.0];
const VISIBLE_ROW_COUNT: usize = 28;
const ROWS_TOP: f32 = 36.0;
const TITLE: &str = "GAME SETTINGS (GAME MASTER)";
const HELP: &str = "Left/Right: change  type a number, Return: set  R: default  SAVE SETTINGS: apply  Escape: back";
const UNSAVED_HELP: &str =
    "UNSAVED CHANGES  Return on SAVE SETTINGS: apply  on DROP: undo  Escape twice: leave them";
const MESSAGE_WIDTH_CHARS: usize = 92;
const SAVE_ID: &str = "save";
const SECTION_ROW_ALLOWANCE: usize = 32;
const DROP_ID: &str = "drop";
const SAVE_LABEL: &str = "SAVE SETTINGS (checks them all, then applies them)";
const DROP_LABEL: &str = "DROP UNSAVED CHANGES";
const NOTHING_TO_SAVE_LINE: &str = "Nothing to save: no unsaved changes.";
const SAVING_LINE: &str = "Checked. Saving to the game server...";
const SAVED_LINE: &str = "Saved: the next round of each level plays by these settings.";
const DROPPED_LINE: &str = "Dropped the unsaved changes.";
const SAVE_ANSWER_TIMEOUT_S: f64 = 3.0;
const UNANSWERED_LINE: &str = "Not saved: the game server didn't answer the save within 3 s. It may be from an older build of the game, which can't take a save; the changes are still here.";
const LEAVE_LINE: &str =
    "Unsaved changes: save or drop them, or press Escape again to leave without them.";
const WAITING_LINE: &str = "Asking the game server for its game settings...";
const OLD_SERVER_LINE: &str = "(A game server older than the game settings never answers.)";
const HEADING_ID: &str = "heading";
const SHARED_SECTION: &str = "shared";
const LEVEL_SECTION_PREFIX: &str = "level_";
const MAX_GAME_SETTING: f64 = 1e6;
const DECIMAL_BASE: f64 = 10.0;
const DEFAULT_STEP: f64 = 0.1;

/// `GameSettingsOutcome` is what the app does after an input on the page: nothing, send the match
/// server a change, or close the page.
#[derive(Clone, PartialEq, Debug)]
pub enum GameSettingsOutcome {
    Stay,
    Send(SettingsMessage),
    Close,
}

/// `get_step` gives a number's step for the arrows: 1 for a whole default, else a tenth of the
/// default's order of magnitude (0.1 for 2.0, 0.01 for 0.33, 10 for 250).
///
/// Example: `assert_eq!(get_step(&json!(2)), 1.0);`
fn get_step(default: &Value) -> f64 {
    if default.is_u64() {
        return 1.0;
    }
    let x: f64 = default.as_f64().unwrap_or(0.0).abs();
    if x == 0.0 {
        return DEFAULT_STEP;
    }
    DECIMAL_BASE.powf(x.log10().floor() - 1.0)
}

/// `get_switch_as_number` gives a switch (`true` or `false`) as the page edits it, 1 or 0, and any
/// other value as it is.
///
/// Example: `assert_eq!(get_switch_as_number(json!(true)), json!(1));`
fn get_switch_as_number(value: Value) -> Value {
    match value {
        Value::Bool(is_on) => Value::from(u64::from(is_on)),
        other => other,
    }
}

/// `put_at` puts `new` at a dotted `key` of `value`, making the sections on the way.
///
/// Example: `put_at(&mut values, "shared.respawn.delay_s", json!(3.0));`
fn put_at(value: &mut Value, key: &str, new: Value) {
    let (parent, name) = key.rsplit_once('.').unwrap_or(("", key));
    let mut node: &mut Value = value;
    for part in parent.split('.').filter(|part| !part.is_empty()) {
        node = node
            .as_object_mut()
            .expect("sections hold sections")
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    node.as_object_mut()
        .expect("a section")
        .insert(name.to_string(), new);
}

/// `parse_row_key` gives a row's scope and settings key from its key in the page's values
/// (`shared.<key>` or `level_<index>.<key>`).
///
/// Example: `assert_eq!(parse_row_key("level_2.glass.range_m"), (SettingsScope::Level(2), "glass.range_m".to_string()));`
fn parse_row_key(row_key: &str) -> (SettingsScope, String) {
    let (section, key) = row_key.split_once('.').expect("a row key has a section");
    let scope: SettingsScope = match section.strip_prefix(LEVEL_SECTION_PREFIX) {
        Some(level) => SettingsScope::Level(level.parse().expect("a level's index")),
        None => SettingsScope::Shared,
    };
    (scope, key.to_string())
}

/// `check_numbers` fails, naming the key, on a number in `values` that isn't finite and at least
/// 0, or isn't whole where its default in `defaults` is.
///
/// Example: `check_numbers(&values, &defaults, "")?;`
fn check_numbers(values: &Value, defaults: &Value, key: &str) -> Result<(), Box<dyn Error>> {
    match defaults {
        Value::Object(fields) => fields.iter().try_for_each(|(name, default)| {
            let path: String = match key {
                "" => name.clone(),
                _ => format!("{key}.{name}"),
            };
            let value: &Value = values.get(name).ok_or_else(|| format!("{path} is gone"))?;
            check_numbers(value, default, &path)
        }),
        Value::Number(_) => {
            let x: f64 = values
                .as_f64()
                .ok_or_else(|| format!("{key} isn't a number"))?;
            if !x.is_finite() || x < 0.0 {
                return Err(format!("{key} must be a number of at least 0").into());
            }
            if defaults.is_u64() && x.fract() != 0.0 {
                return Err(format!("{key} must be a whole number").into());
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// `wrap_message` breaks `message` between words into lines of at most `width_chars` characters
/// (a longer word gets a line of its own), joined by newlines.
///
/// Example: `assert_eq!(wrap_message("not saved at all", 9), "not saved\nat all");`
fn wrap_message(message: &str, width_chars: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    for word in message.split(' ') {
        match lines.last_mut() {
            Some(line) if line.len() + 1 + word.len() <= width_chars => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines.join("\n")
}

/// `push_heading_row` adds a row that only labels: a section's heading, or a note under it.
///
/// Example: `push_heading_row(&mut rows, "DUST ONLY".to_string());`
fn push_heading_row(rows: &mut Vec<EditorRow>, label: String) {
    rows.push(EditorRow {
        label,
        kind: RowKind::Button { id: HEADING_ID },
    });
}

/// `GameSettingsPage` is the page's state: the editor (once the server's settings came), its
/// rows' keys, the values and defaults it shows (sections `shared` and `level_<index>`), the
/// levels' names, the game server's latest settings, those settings with the game master's
/// unsaved changes, whether a save is on its way, and whether Escape was pressed once over
/// unsaved changes.
pub struct GameSettingsPage {
    editor: Option<SettingsEditor>,
    row_keys: Vec<String>,
    values: Value,
    defaults: Value,
    level_names: Vec<String>,
    server: Option<GameSettings>,
    edited: Option<GameSettings>,
    is_saving: bool,
    saving_since: Option<Instant>,
    is_leaving: bool,
}

impl GameSettingsPage {
    /// `New` waits for the game server's settings for a game of the levels named `level_names`.
    ///
    /// Example: `let page = GameSettingsPage::New(vec!["Dust".to_string()]);`
    #[allow(non_snake_case)]
    pub fn New(level_names: Vec<String>) -> GameSettingsPage {
        GameSettingsPage {
            editor: None,
            row_keys: Vec::new(),
            values: Value::Object(Map::new()),
            defaults: Value::Object(Map::new()),
            level_names,
            server: None,
            edited: None,
            is_saving: false,
            saving_since: None,
            is_leaving: false,
        }
    }

    /// `has_unsaved_changes` says whether the page holds changes the game server doesn't have.
    ///
    /// Example: `if page.has_unsaved_changes() { ... }`
    pub fn has_unsaved_changes(&self) -> bool {
        let get: fn(&GameSettings) -> Overrides = GameSettings::get_overrides;
        self.edited.as_ref().map(get) != self.server.as_ref().map(get)
    }

    /// `name_level` gives `level`'s name for a message, or `the save` for no level.
    ///
    /// Example: `let name: String = self.name_level(Some(2));`
    fn name_level(&self, level: Option<usize>) -> String {
        match level {
            Some(level) => self
                .level_names
                .get(level)
                .cloned()
                .unwrap_or(format!("level {level}")),
            None => "the save".to_string(),
        }
    }

    /// `get_row_keys` gives the keys of the page's number rows (`shared.<key>`,
    /// `level_<index>.<key>`), in order.
    ///
    /// Example: `assert!(page.get_row_keys().contains(&"shared.respawn.start_s".to_string()));`
    #[cfg(test)]
    pub fn get_row_keys(&self) -> &[String] {
        &self.row_keys
    }

    /// `is_waiting` says whether no settings have come from the game server yet.
    ///
    /// Example: `if page.is_waiting() { ... }`
    pub fn is_waiting(&self) -> bool {
        self.editor.is_none()
    }

    /// `update` takes `settings`, the game server's latest. Without unsaved changes the page
    /// shows them; with some it keeps showing those. A save on its way ends here: taken (the
    /// server has the page's overrides) or refused (the server says why, for these overrides).
    ///
    /// Example: `page.update(client.settings.as_ref().unwrap());`
    pub fn update(&mut self, settings: &GameSettings) {
        let had_unsaved_changes: bool = self.has_unsaved_changes();
        self.server = Some(settings.clone());
        let mut message: Option<String> = None;
        match &mut self.edited {
            Some(edited) if had_unsaved_changes => {
                edited.defaults = settings.defaults.clone();
                let overrides: Overrides = edited.get_overrides();
                let refusal = settings
                    .refusal
                    .as_ref()
                    .filter(|refusal| self.is_saving && refusal.overrides == overrides);
                if let Some(refusal) = refusal {
                    message = Some(format!(
                        "Not saved. {}: {}",
                        self.name_level(refusal.level),
                        refusal.reason
                    ));
                    self.is_saving = false;
                }
            }
            _ => self.edited = Some(settings.clone()),
        }
        if self.is_saving && !self.has_unsaved_changes() {
            self.is_saving = false;
            message = Some(SAVED_LINE.to_string());
        }
        let is_unanswered: bool = self.is_saving
            && self
                .saving_since
                .is_some_and(|since| since.elapsed().as_secs_f64() >= SAVE_ANSWER_TIMEOUT_S);
        if is_unanswered {
            self.is_saving = false;
            message = Some(UNANSWERED_LINE.to_string());
        }
        self.show_edited();
        if let Some(message) = message {
            self.show_message(message);
        }
    }

    /// `show_edited` shows the settings with the unsaved changes: their values and defaults, and
    /// new rows (keeping the selection) only if their keys changed.
    ///
    /// Example: `self.show_edited();`
    fn show_edited(&mut self) {
        let settings: GameSettings = self.edited.clone().expect("the server's settings came");
        let settings: &GameSettings = &settings;
        let mut rows: Vec<EditorRow> = [(SAVE_LABEL, SAVE_ID), (DROP_LABEL, DROP_ID)]
            .map(|(label, id)| EditorRow {
                label: label.to_string(),
                kind: RowKind::Button { id },
            })
            .to_vec();
        let mut row_keys: Vec<String> = Vec::new();
        let (mut values, mut defaults) = (Value::Object(Map::new()), Value::Object(Map::new()));
        let sections: Vec<(SettingsScope, String, Vec<String>)> = std::iter::once((
            SettingsScope::Shared,
            SHARED_SECTION.to_string(),
            settings.get_shared_keys(),
        ))
        .chain((0..settings.defaults.len()).map(|level| {
            (
                SettingsScope::Level(level),
                format!("{LEVEL_SECTION_PREFIX}{level}"),
                settings.get_level_keys(level),
            )
        }))
        .collect();
        for (scope, section, keys) in sections {
            let heading: String = match scope {
                SettingsScope::Shared => {
                    "EVERY LEVEL (each level that has the setting)".to_string()
                }
                SettingsScope::Level(level) => format!(
                    "{} ONLY",
                    self.level_names
                        .get(level)
                        .map_or(format!("LEVEL {level}"), |name| name.to_uppercase())
                ),
            };
            push_heading_row(&mut rows, heading);
            if keys.is_empty() {
                let note: &str = match scope {
                    SettingsScope::Level(level) if settings.defaults[level].is_none() => {
                        "  (no settings file)"
                    }
                    _ => "  (no settings of its own)",
                };
                push_heading_row(&mut rows, note.to_string());
            }
            for key in keys {
                let default: Value = settings
                    .get_default(scope, &key)
                    .cloned()
                    .expect("a key the scope has");
                let value: Value = settings
                    .get_value(scope, &key)
                    .cloned()
                    .expect("a key the scope has");
                let do_levels_differ: bool = scope == SettingsScope::Shared
                    && settings
                        .defaults
                        .iter()
                        .flatten()
                        .filter_map(|level| get_at(level, &key))
                        .any(|other| other != &default);
                let row_key: String = format!("{section}.{key}");
                let is_switch: bool = default.is_boolean();
                let note: &str = match (is_switch, do_levels_differ) {
                    (true, true) => " (0 off, 1 on; levels differ)",
                    (true, false) => " (0 off, 1 on)",
                    (false, true) => " (levels differ)",
                    (false, false) => "",
                };
                let (default, value) = (get_switch_as_number(default), get_switch_as_number(value));
                rows.push(EditorRow {
                    label: format!("  {key}{note}"),
                    kind: RowKind::Number {
                        key: row_key.clone(),
                        step: get_step(&default),
                        min: 0.0,
                        max: if is_switch { 1.0 } else { MAX_GAME_SETTING },
                    },
                });
                put_at(&mut values, &row_key, value);
                put_at(&mut defaults, &row_key, default);
                row_keys.push(row_key);
            }
        }
        let label_width: usize = rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::Number { .. }))
            .map(|row| row.label.len())
            .max()
            .unwrap_or(0);
        for row in rows
            .iter_mut()
            .filter(|row| matches!(row.kind, RowKind::Number { .. }))
        {
            row.label = format!("{:<label_width$}", row.label);
        }
        if self.editor.is_none() || row_keys != self.row_keys {
            self.editor = Some(SettingsEditor::New(TITLE, rows));
            self.row_keys = row_keys;
        }
        self.values = values;
        self.defaults = defaults;
    }

    /// `save` checks every level's settings with the unsaved changes against the rules: broken,
    /// it says where and why and sends nothing; kept, it sends them all to the game server.
    ///
    /// Example: `let outcome = page.save();`
    pub fn save(&mut self) -> GameSettingsOutcome {
        if !self.has_unsaved_changes() {
            self.show_message(NOTHING_TO_SAVE_LINE.to_string());
            return GameSettingsOutcome::Stay;
        }
        let edited: &GameSettings = self.edited.as_ref().expect("the server's settings came");
        if let Some((level, reason)) = edited.find_problem() {
            let message: String = format!("Not saved. {}: {reason}", self.name_level(Some(level)));
            self.show_message(message);
            return GameSettingsOutcome::Stay;
        }
        let overrides: Overrides = edited.get_overrides();
        self.is_saving = true;
        self.saving_since = Some(Instant::now());
        self.show_message(SAVING_LINE.to_string());
        GameSettingsOutcome::Send(SettingsMessage::Save(overrides))
    }

    /// `select` moves the selection down to the number row `row_key` (`shared.<key>` or
    /// `level_<index>.<key>`); it fails if the page has no such row.
    ///
    /// Example: `page.select("shared.beam_speckle.black_frac")?;`
    pub fn select(&mut self, row_key: &str) -> Result<(), String> {
        for _ in 0..(self.row_keys.len() + SECTION_ROW_ALLOWANCE) * 2 {
            let is_selected: bool = self.editor.as_ref().is_some_and(|editor| {
                matches!(&editor.get_selected_row().kind, RowKind::Number { key, .. } if key == row_key)
            });
            if is_selected {
                return Ok(());
            }
            self.handle(MenuInput::Down);
        }
        Err(format!("the game settings page has no row {row_key}"))
    }

    /// `keep_change` keeps the change just made to the row `row_key` among the unsaved changes:
    /// its value as an override of its scope, or no override after a reset.
    ///
    /// Example: `self.keep_change("shared.respawn.delay_s", false);`
    fn keep_change(&mut self, row_key: &str, is_reset: bool) {
        let (scope, key) = parse_row_key(row_key);
        let value: f64 = get_at(&self.values, row_key)
            .and_then(Value::as_f64)
            .expect("a number row's value");
        let edited: &mut GameSettings = self.edited.as_mut().expect("the server's settings came");
        let default: Value = edited
            .get_default(scope, &key)
            .cloned()
            .expect("a key the scope has");
        let overrides: &mut std::collections::BTreeMap<String, Value> = match scope {
            SettingsScope::Shared => &mut edited.shared,
            SettingsScope::Level(level) => &mut edited.levels[level],
        };
        if is_reset {
            overrides.remove(&key);
            return;
        }
        let held: Value = GameSettings::to_setting_value(&key, &default, value)
            .expect("the editor checked the number");
        overrides.insert(key, held);
    }

    /// `handle` applies one input: a change or a reset of a number is kept on the page, SAVE
    /// SETTINGS checks and sends them all (`save`), DROP UNSAVED CHANGES shows the game server's
    /// again, and Escape closes the page (asking once over unsaved changes).
    ///
    /// Example: `let outcome = page.handle(MenuInput::Reset);`
    pub fn handle(&mut self, input: MenuInput) -> GameSettingsOutcome {
        let has_unsaved_changes: bool = self.has_unsaved_changes();
        let Some(editor) = &mut self.editor else {
            return match input {
                MenuInput::Back => GameSettingsOutcome::Close,
                _ => GameSettingsOutcome::Stay,
            };
        };
        if input == MenuInput::Back && has_unsaved_changes && !self.is_leaving {
            self.is_leaving = true;
            editor.show_message(wrap_message(LEAVE_LINE, MESSAGE_WIDTH_CHARS));
            return GameSettingsOutcome::Stay;
        }
        self.is_leaving = false;
        let is_reset: bool = input == MenuInput::Reset;
        let defaults: Value = self.defaults.clone();
        let check = move |values: &Value| check_numbers(values, &defaults, "");
        let outcome: EditorOutcome = editor.handle(
            get_editor_input(input),
            &mut self.values,
            &self.defaults,
            &check,
        );
        let row_key: Option<String> = match &editor.get_selected_row().kind {
            RowKind::Number { key, .. } => Some(key.clone()),
            _ => None,
        };
        match (outcome, row_key) {
            (EditorOutcome::Back, _) => GameSettingsOutcome::Close,
            (EditorOutcome::Pressed(SAVE_ID), _) => self.save(),
            (EditorOutcome::Pressed(DROP_ID), _) => {
                self.edited = self.server.clone();
                self.is_saving = false;
                self.show_edited();
                self.show_message(DROPPED_LINE.to_string());
                GameSettingsOutcome::Stay
            }
            (EditorOutcome::Changed, Some(row_key)) => {
                self.keep_change(&row_key, is_reset);
                GameSettingsOutcome::Stay
            }
            _ => GameSettingsOutcome::Stay,
        }
    }

    /// `show_message` shows `message` under the rows until the next input, on as many lines as
    /// it needs (`wrap_message`).
    ///
    /// Example: `page.show_message("the game ended".to_string());`
    pub fn show_message(&mut self, message: String) {
        if let Some(editor) = &mut self.editor {
            editor.show_message(wrap_message(&message, MESSAGE_WIDTH_CHARS));
        }
    }

    /// `get_shapes` gives the page and its size: the sections' rows, or a line while no settings
    /// have come.
    ///
    /// Example: `let (shapes, size) = page.get_shapes();`
    pub fn get_shapes(&self) -> page_t {
        let mut shapes: Vec<Shape> = Vec::new();
        let help: &str = if self.has_unsaved_changes() {
            UNSAVED_HELP
        } else {
            HELP
        };
        push_title(&mut shapes, PAGE_SIZE, TITLE, help);
        let Some(editor) = &self.editor else {
            push_heading(&mut shapes, [MARGIN, ROWS_TOP + LINE], WAITING_LINE);
            push_heading(
                &mut shapes,
                [MARGIN, ROWS_TOP + 2.0 * LINE],
                OLD_SERVER_LINE,
            );
            return (shapes, PAGE_SIZE);
        };
        let lines: Vec<(String, bool)> =
            editor.get_lines(&self.values, &self.defaults, VISIBLE_ROW_COUNT);
        let selected: Option<usize> = lines.iter().position(|(_, is_selected)| *is_selected);
        let rows: Vec<Option<MenuRow>> = lines
            .into_iter()
            .map(|(text, _)| {
                Some(MenuRow {
                    is_enabled: !text.trim_start().starts_with('('),
                    text,
                })
            })
            .collect();
        push_rows(&mut shapes, ROWS_TOP, &rows, selected);
        (shapes, PAGE_SIZE)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `game_settings_page.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Sections | shared keys, a switch, a level's own, a level with none (shown, empty), a level without a file, the levels' names | `sections_show_every_level` |
    //! | Changes | nothing changed (nothing to save), a step up (kept, nothing sent), a reset (no longer unsaved), a typed fraction where the default is whole (refused), a switch turned off and a level's own key; then saved: one message with them all, unsaved until the server has them, then "Saved" | `changes_wait_for_a_save` |
    //! | A save that breaks a rule | refused on the page (the level and the reason, nothing sent); refused by the server (its reason, only for these overrides, not an older refusal); never answered (said so after the wait, the changes kept) | `a_save_that_breaks_a_rule_says_why` |
    //! | Unsaved changes | dropped on DROP; Escape once asks, twice leaves | `unsaved_changes_are_dropped_on_request_or_on_leaving_twice` |
    //! | Server's answer | values follow it, same keys keep the selection, waiting before it, Escape closes | `the_page_follows_the_servers_answer` |
    //! | Steps | whole, 2.0, 0.33, 250, 0 | `steps_follow_the_default` |
    //! | Message length | one line, two lines, a word longer than a line | `long_messages_wrap_between_words` |

    use super::*;
    use serde_json::json;
    use warped_shared::game_settings::Refusal;
    use warped_shared::settings_editor::set_at;

    /// `make_settings` gives three levels: two sharing `respawn.delay_s`, `rifle.hits` and the switch `shooting.on`, the
    /// second with `glass.bounces` alone, and one without a settings file.
    ///
    /// Example: `let settings = make_settings();`
    fn make_settings() -> GameSettings {
        GameSettings::New(vec![
            Some(
                json!({"respawn": {"delay_s": 3.0}, "rifle": {"hits": 2}, "shooting": {"on": true}}),
            ),
            Some(
                json!({"respawn": {"delay_s": 5.0}, "rifle": {"hits": 2}, "shooting": {"on": true}, "glass": {"bounces": 2}}),
            ),
            None,
        ])
    }

    /// `make_page` gives a page showing `make_settings`.
    ///
    /// Example: `let page = make_page();`
    fn make_page() -> GameSettingsPage {
        let mut page: GameSettingsPage = GameSettingsPage::New(
            ["Dark Forest", "City Sphere", "Old"]
                .map(str::to_string)
                .to_vec(),
        );
        page.update(&make_settings());
        page
    }

    /// `get_text` gives every line of the page.
    ///
    /// Example: `let text = get_text(&page);`
    fn get_text(page: &GameSettingsPage) -> String {
        page.editor
            .as_ref()
            .unwrap()
            .get_lines(&page.values, &page.defaults, usize::MAX)
            .into_iter()
            .map(|(line, _)| line)
            .collect::<Vec<String>>()
            .join("\n")
    }

    /// `select` moves the page's selection down to the row of `row_key`.
    ///
    /// Example: `select(&mut page, "shared.rifle.hits");`
    fn select(page: &mut GameSettingsPage, row_key: &str) {
        page.select(row_key).unwrap();
    }

    #[test]
    fn sections_show_every_level() {
        let page: GameSettingsPage = make_page();
        let text: String = get_text(&page);
        for line in [
            SAVE_LABEL,
            DROP_LABEL,
            "EVERY LEVEL",
            "respawn.delay_s (levels differ)",
            "rifle.hits",
            "shooting.on (0 off, 1 on)",
            "DARK FOREST ONLY",
            "(no settings of its own)",
            "CITY SPHERE ONLY",
            "glass.bounces",
            "OLD ONLY",
            "(no settings file)",
        ] {
            assert!(text.contains(line), "{line} in\n{text}");
        }
        assert_eq!(
            page.row_keys,
            [
                "shared.respawn.delay_s",
                "shared.rifle.hits",
                "shared.shooting.on",
                "level_1.glass.bounces"
            ]
        );
    }

    /// `press` gives the page `input` on its button `id`'s row (SAVE or DROP, the first rows).
    ///
    /// Example: `let outcome = press(&mut page, SAVE_ID);`
    fn press(page: &mut GameSettingsPage, id: &'static str) -> GameSettingsOutcome {
        for _ in 0..page.row_keys.len() * 4 {
            let row: &EditorRow = page.editor.as_ref().unwrap().get_selected_row();
            if matches!(&row.kind, RowKind::Button { id: row_id } if *row_id == id) {
                return page.handle(MenuInput::Activate);
            }
            page.handle(MenuInput::Up);
        }
        panic!("no button {id}");
    }

    /// `get_message` gives the page's text with its message line.
    ///
    /// Example: `assert!(get_message(&page).contains("Saved"));`
    fn get_message(page: &GameSettingsPage) -> String {
        format!("{:?}", page.get_shapes().0)
    }

    #[test]
    fn changes_wait_for_a_save() {
        let mut page: GameSettingsPage = make_page();
        assert!(!page.has_unsaved_changes());
        assert_eq!(press(&mut page, SAVE_ID), GameSettingsOutcome::Stay);
        assert!(get_message(&page).contains(NOTHING_TO_SAVE_LINE));
        select(&mut page, "shared.rifle.hits");
        assert_eq!(page.handle(MenuInput::Right), GameSettingsOutcome::Stay);
        assert!(page.has_unsaved_changes());
        assert!(get_message(&page).contains("UNSAVED CHANGES"));
        assert_eq!(page.handle(MenuInput::Reset), GameSettingsOutcome::Stay);
        assert!(!page.has_unsaved_changes());
        for input in [
            MenuInput::Typed('2'),
            MenuInput::Typed('.'),
            MenuInput::Typed('5'),
        ] {
            page.handle(input);
        }
        assert_eq!(page.handle(MenuInput::Activate), GameSettingsOutcome::Stay);
        assert!(get_text(&page).contains("whole number"));
        select(&mut page, "shared.shooting.on");
        assert_eq!(page.handle(MenuInput::Left), GameSettingsOutcome::Stay);
        select(&mut page, "level_1.glass.bounces");
        assert_eq!(page.handle(MenuInput::Left), GameSettingsOutcome::Stay);
        let mut expected: Overrides = make_settings().get_overrides();
        expected
            .shared
            .insert("shooting.on".to_string(), json!(false));
        expected.levels[1].insert("glass.bounces".to_string(), json!(1));
        assert_eq!(
            press(&mut page, SAVE_ID),
            GameSettingsOutcome::Send(SettingsMessage::Save(expected.clone()))
        );
        let mut answer: GameSettings = make_settings();
        page.update(&answer);
        assert!(page.has_unsaved_changes());
        answer.apply(&SettingsMessage::Save(expected)).unwrap();
        page.update(&answer);
        assert!(!page.has_unsaved_changes());
        assert!(get_message(&page).contains(SAVED_LINE));
        assert_eq!(page.values["shared"]["shooting"]["on"], json!(0));
    }

    #[test]
    fn a_save_that_breaks_a_rule_says_why() {
        let mut page: GameSettingsPage = make_page();
        select(&mut page, "shared.respawn.delay_s");
        page.handle(MenuInput::Typed('0'));
        assert_eq!(page.handle(MenuInput::Activate), GameSettingsOutcome::Stay);
        assert_eq!(press(&mut page, SAVE_ID), GameSettingsOutcome::Stay);
        let text: String = get_message(&page);
        assert!(
            text.contains("Not saved. Dark Forest: respawn.delay_s must be above 0, not 0"),
            "{text}"
        );
        assert!(page.has_unsaved_changes());
        select(&mut page, "shared.respawn.delay_s");
        page.handle(MenuInput::Typed('4'));
        page.handle(MenuInput::Activate);
        let GameSettingsOutcome::Send(SettingsMessage::Save(overrides)) = press(&mut page, SAVE_ID)
        else {
            panic!("a save that keeps the rules is sent");
        };
        let mut refusing: GameSettings = make_settings();
        refusing.refusal = Some(Refusal {
            overrides: make_settings().get_overrides(),
            level: Some(1),
            reason: "an older refusal".to_string(),
        });
        page.update(&refusing);
        assert!(!get_message(&page).contains("an older refusal"));
        refusing.refusal = Some(Refusal {
            overrides,
            level: Some(1),
            reason: "the server's reason".to_string(),
        });
        page.update(&refusing);
        let text: String = get_message(&page);
        assert!(
            text.contains("Not saved. City Sphere: the server's reason"),
            "{text}"
        );
        assert!(page.has_unsaved_changes());
        let GameSettingsOutcome::Send(_) = press(&mut page, SAVE_ID) else {
            panic!("the same save is sent again");
        };
        page.saving_since = Some(Instant::now() - std::time::Duration::from_secs(4));
        page.update(&make_settings());
        let text: String = get_message(&page);
        assert!(text.contains("didn't answer the save"), "{text}");
        assert!(page.has_unsaved_changes() && !page.is_saving);
    }

    #[test]
    fn unsaved_changes_are_dropped_on_request_or_on_leaving_twice() {
        let mut page: GameSettingsPage = make_page();
        select(&mut page, "shared.rifle.hits");
        page.handle(MenuInput::Right);
        assert_eq!(page.values["shared"]["rifle"]["hits"].as_f64(), Some(3.0));
        assert_eq!(press(&mut page, DROP_ID), GameSettingsOutcome::Stay);
        assert!(!page.has_unsaved_changes());
        assert_eq!(page.values["shared"]["rifle"]["hits"].as_f64(), Some(2.0));
        select(&mut page, "shared.rifle.hits");
        page.handle(MenuInput::Right);
        assert_eq!(page.handle(MenuInput::Back), GameSettingsOutcome::Stay);
        assert!(get_message(&page).contains(LEAVE_LINE));
        assert_eq!(page.handle(MenuInput::Back), GameSettingsOutcome::Close);
    }

    #[test]
    fn the_page_follows_the_servers_answer() {
        let mut waiting: GameSettingsPage = GameSettingsPage::New(Vec::new());
        assert!(waiting.is_waiting());
        assert_eq!(waiting.handle(MenuInput::Back), GameSettingsOutcome::Close);
        let mut page: GameSettingsPage = make_page();
        select(&mut page, "shared.rifle.hits");
        let mut settings: GameSettings = make_settings();
        settings
            .apply(&SettingsMessage::Set {
                scope: SettingsScope::Shared,
                key: "respawn.delay_s".to_string(),
                value: 9.0,
            })
            .unwrap();
        page.update(&settings);
        assert_eq!(page.values["shared"]["respawn"]["delay_s"], json!(9.0));
        let row: &EditorRow = page.editor.as_ref().unwrap().get_selected_row();
        assert!(matches!(&row.kind, RowKind::Number { key, .. } if key == "shared.rifle.hits"));
        set_at(&mut page.values, "shared.rifle.hits", json!(4)).unwrap();
        assert!(check_numbers(&page.values, &page.defaults, "").is_ok());
        assert_eq!(page.handle(MenuInput::Back), GameSettingsOutcome::Close);
    }

    #[test]
    fn long_messages_wrap_between_words() {
        assert_eq!(wrap_message("Saved.", 9), "Saved.");
        assert_eq!(wrap_message("not saved at all", 9), "not saved\nat all");
        assert_eq!(
            wrap_message("a beam_speckle.black_frac b", 9),
            "a\nbeam_speckle.black_frac\nb"
        );
    }

    #[test]
    fn steps_follow_the_default() {
        assert_eq!(get_step(&json!(2)), 1.0);
        assert!((get_step(&json!(2.0)) - 0.1).abs() < 1e-12);
        assert!((get_step(&json!(0.33)) - 0.01).abs() < 1e-12);
        assert!((get_step(&json!(250.0)) - 10.0).abs() < 1e-9);
        assert_eq!(get_step(&json!(0.0)), DEFAULT_STEP);
    }
}
