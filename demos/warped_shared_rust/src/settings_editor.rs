//! `settings_editor.rs` is a settings page that any menu can show: rows of numbers (changed with
//! the arrows or typed), key bindings (rebound by pressing a control) and buttons, over a
//! settings file's keys held as a JSON value. Player settings and the game master's server
//! settings use it alike; it keeps no window state, so a window feeds it inputs and draws its
//! lines.
//!
//! Interface: `SettingsEditor` (`New`, `handle`, `is_rebinding`, `get_lines`, `show_message`,
//! `get_selected_row`),
//! `EditorRow`, `RowKind`, `EditorInput`, `EditorOutcome`, `check_fn_t`, `get_at`, `set_at`.
//!
//! Example: `let outcome = editor.handle(EditorInput::Right, &mut values, &defaults, &check);`
//!
//! Must know:
//! - Gotchas: every change is checked by the caller's `check` on a copy first, so `values` only
//!   ever holds settings that load; a refused change leaves them as they were and shows why.
//! - Gotchas: a row's key is a dotted path into the value (`view.motion_blur_s`), the file's.

use std::error::Error;

use serde_json::Value;

/// `check_fn_t` is a check of edited settings: it fails, saying why, on settings that wouldn't
/// load.
#[allow(non_camel_case_types)]
pub type check_fn_t = dyn Fn(&Value) -> Result<(), Box<dyn Error>>;

/// `ROUNDING_PER_UNIT` is how finely a stepped number is rounded, so steps don't pile up float
/// noise (`0.30000000000000004`).
const ROUNDING_PER_UNIT: f64 = 1e9;
const CHANGED_MARK: &str = "*";
const UNCHANGED_MARK: &str = " ";
const LABEL_WIDTH: usize = 28;

/// `RowKind` is what a row edits: a number at `key` between `min` and `max` in steps of `step`,
/// the controls of the action at `keys.<action>`, or a button the caller acts on (`id`).
#[derive(Clone, PartialEq, Debug)]
pub enum RowKind {
    Number {
        key: String,
        step: f64,
        min: f64,
        max: f64,
    },
    Keys {
        action: String,
    },
    Button {
        id: &'static str,
    },
}

/// `EditorRow` is one row of the page: its label and what it edits.
#[derive(Clone, PartialEq, Debug)]
pub struct EditorRow {
    pub label: String,
    pub kind: RowKind,
}

/// `EditorInput` is what the window passes on: moving between rows, stepping a number down or
/// up, choosing a row (`Activate`: start rebinding, commit a typed number, press a button),
/// resetting the row to its default, going back, typing a character of a number, erasing one,
/// and, while rebinding, the name of the control pressed.
#[derive(Clone, PartialEq, Debug)]
pub enum EditorInput {
    Up,
    Down,
    Left,
    Right,
    Activate,
    Reset,
    Back,
    Typed(char),
    Erase,
    Control(String),
}

/// `EditorOutcome` is what happened: nothing for the caller to do, a setting changed (and is in
/// `values`), the player went back (leave the page), or a button was pressed.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EditorOutcome {
    Stay,
    Changed,
    Back,
    Pressed(&'static str),
}

/// `get_at` gives the value at a dotted `key` of `value`, or `None` if it has none.
///
/// Example: `let blur = get_at(&values, "view.motion_blur_s");`
pub fn get_at<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(value, |node, part| node.get(part))
}

/// `set_at` puts `new` at a dotted `key` of `value`, whose parent must exist.
///
/// Example: `set_at(&mut values, "view.motion_blur_s", json!(0.0))?;`
pub fn set_at(value: &mut Value, key: &str, new: Value) -> Result<(), Box<dyn Error>> {
    let (parent_key, name) = match key.rsplit_once('.') {
        Some((parent_key, name)) => (Some(parent_key), name),
        None => (None, key),
    };
    let parent: &mut Value = match parent_key {
        Some(parent_key) => parent_key
            .split('.')
            .try_fold(value, |node, part| node.get_mut(part))
            .ok_or_else(|| format!("the settings have no {parent_key}"))?,
        None => value,
    };
    let fields = parent
        .as_object_mut()
        .ok_or_else(|| format!("{key}'s parent isn't a section"))?;
    fields.insert(name.to_string(), new);
    Ok(())
}

/// `SettingsEditor` is the page's state: its title, rows, the selected row, a number being typed,
/// whether it waits for a control to rebind the selected action, and a message to show.
pub struct SettingsEditor {
    pub title: String,
    rows: Vec<EditorRow>,
    selected: usize,
    typed: Option<String>,
    is_rebinding: bool,
    message: Option<String>,
}

impl SettingsEditor {
    /// `New` makes a page of `rows` (at least one) under `title`, the first row selected.
    ///
    /// Example: `let editor = SettingsEditor::New("PLAYER SETTINGS", rows);`
    #[allow(non_snake_case)]
    pub fn New(title: &str, rows: Vec<EditorRow>) -> SettingsEditor {
        assert!(!rows.is_empty(), "SettingsEditor::New: a page needs rows");
        SettingsEditor {
            title: title.to_string(),
            rows,
            selected: 0,
            typed: None,
            is_rebinding: false,
            message: None,
        }
    }

    /// `is_rebinding` says whether the page waits for the control to bind to the selected
    /// action, so the window passes the next control as `EditorInput::Control` (and Escape as
    /// `Back`, which cancels).
    ///
    /// Example: `if editor.is_rebinding() { ... }`
    pub fn is_rebinding(&self) -> bool {
        self.is_rebinding
    }

    /// `get_selected_row` gives the row selected now, so a caller can act on its key (the game
    /// master's page sends each change to the match server).
    ///
    /// Example: `let row: &EditorRow = editor.get_selected_row();`
    pub fn get_selected_row(&self) -> &EditorRow {
        &self.rows[self.selected]
    }

    /// `show_message` shows `message` under the rows until the next input.
    ///
    /// Example: `editor.show_message("Saved.".to_string());`
    pub fn show_message(&mut self, message: String) {
        self.message = Some(message);
    }

    /// `try_change` applies `change` to a copy of `values`, keeps it if `check` passes, and
    /// otherwise shows why it was refused.
    ///
    /// Example: `self.try_change(values, check, |copy| set_at(copy, key, new))`
    fn try_change(
        &mut self,
        values: &mut Value,
        check: &check_fn_t,
        change: impl FnOnce(&mut Value) -> Result<(), Box<dyn Error>>,
    ) -> EditorOutcome {
        let mut copy: Value = values.clone();
        match change(&mut copy).and_then(|()| check(&copy)) {
            Ok(()) => {
                *values = copy;
                EditorOutcome::Changed
            }
            Err(error) => {
                self.message = Some(error.to_string());
                EditorOutcome::Stay
            }
        }
    }

    /// `handle` applies one input to the page and to `values` (whose defaults are `defaults`,
    /// for resets), checking every change with `check`.
    ///
    /// Example: `match editor.handle(input, &mut values, &defaults, &check) { ... }`
    pub fn handle(
        &mut self,
        input: EditorInput,
        values: &mut Value,
        defaults: &Value,
        check: &check_fn_t,
    ) -> EditorOutcome {
        self.message = None;
        let row: EditorRow = self.rows[self.selected].clone();
        if self.is_rebinding {
            self.is_rebinding = false;
            return match (input, &row.kind) {
                (EditorInput::Control(name), RowKind::Keys { action }) => {
                    let key: String = format!("keys.{action}");
                    self.try_change(values, check, |copy| {
                        set_at(copy, &key, Value::from(vec![name]))
                    })
                }
                _ => EditorOutcome::Stay,
            };
        }
        if let (Some(typed), RowKind::Number { key, min, max, .. }) = (&self.typed, &row.kind) {
            let mut typed: String = typed.clone();
            match input {
                EditorInput::Typed(c) if c.is_ascii_digit() || c == '.' => typed.push(c),
                EditorInput::Erase => {
                    typed.pop();
                }
                EditorInput::Activate => {
                    self.typed = None;
                    let Ok(x) = typed.parse::<f64>() else {
                        self.message = Some(format!("{typed:?} isn't a number"));
                        return EditorOutcome::Stay;
                    };
                    if !(*min..=*max).contains(&x) {
                        self.message = Some(format!("{} goes from {min} to {max}", row.label));
                        return EditorOutcome::Stay;
                    }
                    let key: String = key.clone();
                    return self.try_change(values, check, |copy| set_at(copy, &key, x.into()));
                }
                EditorInput::Back => {
                    self.typed = None;
                    return EditorOutcome::Stay;
                }
                _ => {}
            }
            self.typed = Some(typed);
            return EditorOutcome::Stay;
        }
        match (input, &row.kind) {
            (EditorInput::Up, _) => {
                self.selected = (self.selected + self.rows.len() - 1) % self.rows.len();
                EditorOutcome::Stay
            }
            (EditorInput::Down, _) => {
                self.selected = (self.selected + 1) % self.rows.len();
                EditorOutcome::Stay
            }
            (EditorInput::Back, _) => EditorOutcome::Back,
            (
                input @ (EditorInput::Left | EditorInput::Right),
                RowKind::Number {
                    key,
                    step,
                    min,
                    max,
                },
            ) => {
                let x: f64 = get_at(values, key).and_then(Value::as_f64).unwrap_or(*min);
                let delta: f64 = if input == EditorInput::Left {
                    -step
                } else {
                    *step
                };
                let stepped: f64 = ((x + delta) * ROUNDING_PER_UNIT).round() / ROUNDING_PER_UNIT;
                let new: f64 = stepped.clamp(*min, *max);
                let key: String = key.clone();
                self.try_change(values, check, |copy| set_at(copy, &key, new.into()))
            }
            (EditorInput::Typed(c), RowKind::Number { .. }) if c.is_ascii_digit() || c == '.' => {
                self.typed = Some(c.to_string());
                EditorOutcome::Stay
            }
            (EditorInput::Activate, RowKind::Keys { action }) => {
                self.is_rebinding = true;
                self.message = Some(format!(
                    "Press the key or mouse button for {action} (Escape cancels)."
                ));
                EditorOutcome::Stay
            }
            (EditorInput::Activate, RowKind::Button { id }) => EditorOutcome::Pressed(id),
            (EditorInput::Reset, RowKind::Number { key, .. }) => {
                self.reset_key(values, defaults, check, key.clone())
            }
            (EditorInput::Reset, RowKind::Keys { action }) => {
                self.reset_key(values, defaults, check, format!("keys.{action}"))
            }
            _ => EditorOutcome::Stay,
        }
    }

    /// `reset_key` puts `defaults`' value at `key` into `values`.
    ///
    /// Example: `self.reset_key(values, defaults, check, "view.motion_blur_s".to_string())`
    fn reset_key(
        &mut self,
        values: &mut Value,
        defaults: &Value,
        check: &check_fn_t,
        key: String,
    ) -> EditorOutcome {
        let Some(default) = get_at(defaults, &key).cloned() else {
            self.message = Some(format!("{key} has no default"));
            return EditorOutcome::Stay;
        };
        self.try_change(values, check, |copy| set_at(copy, &key, default))
    }

    /// `format_row` gives one row's line: a mark if it differs from its default, its label and
    /// its value (the typed text while typing, `?` while rebinding).
    ///
    /// Example: `let line = self.format_row(0, &values, &defaults);`
    fn format_row(&self, index: usize, values: &Value, defaults: &Value) -> String {
        let row: &EditorRow = &self.rows[index];
        let is_selected: bool = index == self.selected;
        let key: Option<String> = match &row.kind {
            RowKind::Number { key, .. } => Some(key.clone()),
            RowKind::Keys { action } => Some(format!("keys.{action}")),
            RowKind::Button { .. } => None,
        };
        let value: Option<&Value> = key.as_deref().and_then(|key| get_at(values, key));
        let is_changed: bool =
            key.is_some() && value != key.as_deref().and_then(|key| get_at(defaults, key));
        let mark: &str = if is_changed {
            CHANGED_MARK
        } else {
            UNCHANGED_MARK
        };
        let shown: String = match (&row.kind, value) {
            (RowKind::Number { .. }, _) if is_selected && self.typed.is_some() => {
                format!("{}_", self.typed.as_deref().unwrap_or_default())
            }
            (RowKind::Number { .. }, Some(value)) if is_selected => format!("< {value} >"),
            (RowKind::Keys { .. }, _) if is_selected && self.is_rebinding => "?".to_string(),
            (RowKind::Keys { .. }, Some(Value::Array(names))) => names
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<&str>>()
                .join(", "),
            (_, Some(value)) => value.to_string(),
            (RowKind::Button { .. }, None) => String::new(),
            (_, None) => "(missing)".to_string(),
        };
        format!("{mark}{:<LABEL_WIDTH$} {shown}", row.label)
    }

    /// `get_lines` gives at most `visible_row_count` rows around the selected one, each with
    /// whether it is selected, then the message if there is one (highlighted).
    ///
    /// Example: `for (line, is_highlighted) in editor.get_lines(&values, &defaults, 20) { ... }`
    pub fn get_lines(
        &self,
        values: &Value,
        defaults: &Value,
        visible_row_count: usize,
    ) -> Vec<(String, bool)> {
        let first: usize = self
            .selected
            .saturating_sub(visible_row_count / 2)
            .min(self.rows.len().saturating_sub(visible_row_count));
        let mut lines: Vec<(String, bool)> = (first..self.rows.len())
            .take(visible_row_count)
            .map(|index| {
                (
                    self.format_row(index, values, defaults),
                    index == self.selected,
                )
            })
            .collect();
        if let Some(message) = &self.message {
            lines.push((String::new(), false));
            lines.push((message.clone(), true));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `settings_editor.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Moving | down, up, wrapping at both ends | `moving_wraps_around` |
    //! | Stepping a number | up, down, clamped at its max and min, refused by `check` | `numbers_step_and_stay_in_range` |
    //! | Typing a number | in range, out of range, not a number, erased, cancelled | `typed_numbers_commit_in_range` |
    //! | Rebinding | a control, cancelled with Back, refused by `check` | `actions_are_rebound` |
    //! | Reset | a number, an action's keys | `rows_reset_to_their_defaults` |
    //! | Buttons and back | a button pressed, Back | `buttons_and_back_reach_the_caller` |
    //! | Lines | a changed row marked, the window around the selection, the message | `lines_mark_changes_and_follow_the_selection` |
    //! | Paths | nested key read and written, a missing parent | `paths_read_and_write_nested_keys` |

    use super::*;
    use serde_json::json;

    /// `make_rows` gives a page with a number (0 to 1 in 0.25), an action's keys and a button.
    ///
    /// Example: `let editor = SettingsEditor::New("T", make_rows());`
    fn make_rows() -> Vec<EditorRow> {
        vec![
            EditorRow {
                label: "blur".to_string(),
                kind: RowKind::Number {
                    key: "view.blur".to_string(),
                    step: 0.25,
                    min: 0.0,
                    max: 1.0,
                },
            },
            EditorRow {
                label: "jump".to_string(),
                kind: RowKind::Keys {
                    action: "jump".to_string(),
                },
            },
            EditorRow {
                label: "save".to_string(),
                kind: RowKind::Button { id: "save" },
            },
        ]
    }

    /// `make_values` gives settings with `view.blur` 0.5 and jump on Space.
    ///
    /// Example: `let values = make_values();`
    fn make_values() -> Value {
        json!({"view": {"blur": 0.5}, "keys": {"jump": ["Space"]}})
    }

    /// `accept` is a check that takes every change.
    ///
    /// Example: `editor.handle(input, &mut values, &defaults, &accept);`
    fn accept(_: &Value) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// `refuse_wheel` is a check that refuses binding the Wheel.
    ///
    /// Example: `editor.handle(input, &mut values, &defaults, &refuse_wheel);`
    fn refuse_wheel(value: &Value) -> Result<(), Box<dyn Error>> {
        match get_at(value, "keys.jump") == Some(&json!(["Wheel"])) {
            true => Err("jump is held, so it can't use the Wheel".into()),
            false => Ok(()),
        }
    }

    #[test]
    fn moving_wraps_around() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let (mut values, defaults) = (make_values(), make_values());
        editor.handle(EditorInput::Up, &mut values, &defaults, &accept);
        assert_eq!(editor.selected, 2);
        editor.handle(EditorInput::Down, &mut values, &defaults, &accept);
        assert_eq!(editor.selected, 0);
        editor.handle(EditorInput::Down, &mut values, &defaults, &accept);
        assert_eq!(editor.selected, 1);
    }

    #[test]
    fn numbers_step_and_stay_in_range() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let (mut values, defaults) = (make_values(), make_values());
        for (input, expected) in [
            (EditorInput::Right, 0.75),
            (EditorInput::Right, 1.0),
            (EditorInput::Right, 1.0),
            (EditorInput::Left, 0.75),
        ] {
            editor.handle(input, &mut values, &defaults, &accept);
            assert_eq!(get_at(&values, "view.blur"), Some(&json!(expected)));
        }
        for _ in 0..5 {
            editor.handle(EditorInput::Left, &mut values, &defaults, &accept);
        }
        assert_eq!(get_at(&values, "view.blur"), Some(&json!(0.0)));
        let refuse_all = |_: &Value| -> Result<(), Box<dyn Error>> { Err("no".into()) };
        let outcome = editor.handle(EditorInput::Right, &mut values, &defaults, &refuse_all);
        assert_eq!(outcome, EditorOutcome::Stay);
        assert_eq!(get_at(&values, "view.blur"), Some(&json!(0.0)));
    }

    #[test]
    fn typed_numbers_commit_in_range() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let (mut values, defaults) = (make_values(), make_values());
        let type_text = |editor: &mut SettingsEditor, values: &mut Value, text: &str| {
            for c in text.chars() {
                editor.handle(EditorInput::Typed(c), values, &defaults, &accept);
            }
        };
        type_text(&mut editor, &mut values, "0.29");
        editor.handle(EditorInput::Erase, &mut values, &defaults, &accept);
        let outcome = editor.handle(EditorInput::Activate, &mut values, &defaults, &accept);
        assert_eq!(outcome, EditorOutcome::Changed);
        assert_eq!(get_at(&values, "view.blur"), Some(&json!(0.2)));
        for text in ["3", "1..2"] {
            type_text(&mut editor, &mut values, text);
            let outcome = editor.handle(EditorInput::Activate, &mut values, &defaults, &accept);
            assert_eq!(outcome, EditorOutcome::Stay, "{text}");
            assert!(editor.message.is_some());
        }
        type_text(&mut editor, &mut values, "1");
        editor.handle(EditorInput::Back, &mut values, &defaults, &accept);
        assert_eq!(get_at(&values, "view.blur"), Some(&json!(0.2)));
        assert!(editor.typed.is_none());
    }

    #[test]
    fn actions_are_rebound() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let (mut values, defaults) = (make_values(), make_values());
        editor.handle(EditorInput::Down, &mut values, &defaults, &accept);
        editor.handle(EditorInput::Activate, &mut values, &defaults, &accept);
        assert!(editor.is_rebinding());
        let control: EditorInput = EditorInput::Control("J".to_string());
        assert_eq!(
            editor.handle(control, &mut values, &defaults, &accept),
            EditorOutcome::Changed
        );
        assert_eq!(get_at(&values, "keys.jump"), Some(&json!(["J"])));
        editor.handle(EditorInput::Activate, &mut values, &defaults, &accept);
        editor.handle(EditorInput::Back, &mut values, &defaults, &accept);
        assert!(!editor.is_rebinding());
        assert_eq!(get_at(&values, "keys.jump"), Some(&json!(["J"])));
        editor.handle(EditorInput::Activate, &mut values, &defaults, &accept);
        let wheel: EditorInput = EditorInput::Control("Wheel".to_string());
        editor.handle(wheel, &mut values, &defaults, &refuse_wheel);
        assert_eq!(get_at(&values, "keys.jump"), Some(&json!(["J"])));
        assert!(editor.message.as_deref().unwrap().contains("Wheel"));
    }

    #[test]
    fn rows_reset_to_their_defaults() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let defaults: Value = make_values();
        let mut values: Value = json!({"view": {"blur": 1.0}, "keys": {"jump": ["J"]}});
        editor.handle(EditorInput::Reset, &mut values, &defaults, &accept);
        editor.handle(EditorInput::Down, &mut values, &defaults, &accept);
        editor.handle(EditorInput::Reset, &mut values, &defaults, &accept);
        assert_eq!(values, defaults);
    }

    #[test]
    fn buttons_and_back_reach_the_caller() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let (mut values, defaults) = (make_values(), make_values());
        editor.handle(EditorInput::Up, &mut values, &defaults, &accept);
        let pressed = editor.handle(EditorInput::Activate, &mut values, &defaults, &accept);
        assert_eq!(pressed, EditorOutcome::Pressed("save"));
        let back = editor.handle(EditorInput::Back, &mut values, &defaults, &accept);
        assert_eq!(back, EditorOutcome::Back);
    }

    #[test]
    fn lines_mark_changes_and_follow_the_selection() {
        let mut editor: SettingsEditor = SettingsEditor::New("T", make_rows());
        let defaults: Value = make_values();
        let mut values: Value = json!({"view": {"blur": 1.0}, "keys": {"jump": ["Space"]}});
        let lines: Vec<(String, bool)> = editor.get_lines(&values, &defaults, 3);
        assert!(lines[0].0.starts_with(CHANGED_MARK) && lines[0].0.ends_with("< 1.0 >"));
        assert!(lines[0].1 && !lines[1].1);
        assert!(lines[1].0.starts_with(UNCHANGED_MARK) && lines[1].0.ends_with("Space"));
        editor.handle(EditorInput::Up, &mut values, &defaults, &accept);
        let window: Vec<(String, bool)> = editor.get_lines(&values, &defaults, 2);
        assert_eq!(window.len(), 2);
        assert!(window[1].1 && window[1].0.contains("save"));
        editor.show_message("Saved.".to_string());
        let with_message: Vec<(String, bool)> = editor.get_lines(&values, &defaults, 2);
        assert_eq!(with_message.last(), Some(&("Saved.".to_string(), true)));
    }

    #[test]
    fn paths_read_and_write_nested_keys() {
        let mut values: Value = make_values();
        set_at(&mut values, "view.blur", json!(0.1)).unwrap();
        assert_eq!(get_at(&values, "view.blur"), Some(&json!(0.1)));
        assert!(set_at(&mut values, "missing.blur", json!(0.1)).is_err());
        assert_eq!(get_at(&values, "missing.blur"), None);
    }
}
