//! `game_settings_page.rs` is the game master's settings page, opened from the game menu (W-158,
//! per-map game settings): the match server's game settings (`warped_shared::game_settings`) in
//! sections, the shared one (every level that has the key) and one per level (its own keys, shown
//! even when it has none), edited with the shared settings editor. Each change and each reset goes
//! to the match server, which holds the settings and gives each round's level server its merged
//! copy; the page shows the server's latest answer.
//!
//! Interface: `GameSettingsPage` (`New`, `update`, `is_waiting`, `handle`, `get_shapes`),
//! `GameSettingsOutcome`.
//!
//! Example: `if let GameSettingsOutcome::Send(message) = page.handle(MenuInput::Right) { client.send_settings(&message)?; }`
//!
//! Must know:
//! - Gotchas: the page's rows come from the keys the levels' settings files have, so settings that
//!   other changes add show up without changing this file.
//! - Gotchas: only finite numbers of at least 0 are allowed, whole where the default is; a level
//!   that refuses a value (a speed of 0, say) stops its round's level server at start.

use std::error::Error;

use serde_json::{Map, Value};
use warped_shared::game_settings::{GameSettings, SettingsMessage, SettingsScope};
use warped_shared::page::Shape;
use warped_shared::settings_editor::{EditorOutcome, EditorRow, RowKind, SettingsEditor, get_at};

use crate::menu_page::{
    LINE, MARGIN, MenuInput, MenuRow, get_editor_input, page_t, push_heading, push_rows, push_title,
};

const PAGE_SIZE: [f32; 2] = [800.0, 455.0];
const VISIBLE_ROW_COUNT: usize = 28;
const ROWS_TOP: f32 = 36.0;
const TITLE: &str = "GAME SETTINGS (GAME MASTER)";
const HELP: &str = "Left/Right: change   type a number, Return: set   R: back to the level's default   Escape: back";
const WAITING_LINE: &str = "Asking the match server for its game settings...";
const OLD_SERVER_LINE: &str = "(A match server older than the game settings never answers.)";
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
/// rows' keys, the values and defaults it shows (sections `shared` and `level_<index>`), and the
/// levels' names.
pub struct GameSettingsPage {
    editor: Option<SettingsEditor>,
    row_keys: Vec<String>,
    values: Value,
    defaults: Value,
    level_names: Vec<String>,
}

impl GameSettingsPage {
    /// `New` waits for the match server's settings for a game of the levels named `level_names`.
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
        }
    }

    /// `is_waiting` says whether no settings have come from the match server yet.
    ///
    /// Example: `if page.is_waiting() { ... }`
    pub fn is_waiting(&self) -> bool {
        self.editor.is_none()
    }

    /// `update` shows `settings`, the match server's latest: their values and defaults, and new
    /// rows (keeping the selection) only if their keys changed.
    ///
    /// Example: `page.update(client.settings.as_ref().unwrap());`
    pub fn update(&mut self, settings: &GameSettings) {
        let mut rows: Vec<EditorRow> = Vec::new();
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
                rows.push(EditorRow {
                    label: match do_levels_differ {
                        true => format!("  {key} (levels differ)"),
                        false => format!("  {key}"),
                    },
                    kind: RowKind::Number {
                        key: row_key.clone(),
                        step: get_step(&default),
                        min: 0.0,
                        max: MAX_GAME_SETTING,
                    },
                });
                put_at(&mut values, &row_key, value);
                put_at(&mut defaults, &row_key, default);
                row_keys.push(row_key);
            }
        }
        if self.editor.is_none() || row_keys != self.row_keys {
            self.editor = Some(SettingsEditor::New(TITLE, rows));
            self.row_keys = row_keys;
        }
        self.values = values;
        self.defaults = defaults;
    }

    /// `handle` applies one input: a change or a reset of a number is sent to the match server
    /// (and shown at once), Escape closes the page.
    ///
    /// Example: `let outcome = page.handle(MenuInput::Reset);`
    pub fn handle(&mut self, input: MenuInput) -> GameSettingsOutcome {
        let Some(editor) = &mut self.editor else {
            return match input {
                MenuInput::Back => GameSettingsOutcome::Close,
                _ => GameSettingsOutcome::Stay,
            };
        };
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
            (EditorOutcome::Changed, Some(row_key)) => {
                let (scope, key) = parse_row_key(&row_key);
                if is_reset {
                    return GameSettingsOutcome::Send(SettingsMessage::Reset { scope, key });
                }
                let value: f64 = get_at(&self.values, &row_key)
                    .and_then(Value::as_f64)
                    .expect("a number row's value");
                GameSettingsOutcome::Send(SettingsMessage::Set { scope, key, value })
            }
            _ => GameSettingsOutcome::Stay,
        }
    }

    /// `show_message` shows `message` under the rows until the next input.
    ///
    /// Example: `page.show_message("the game ended".to_string());`
    pub fn show_message(&mut self, message: String) {
        if let Some(editor) = &mut self.editor {
            editor.show_message(message);
        }
    }

    /// `get_shapes` gives the page and its size: the sections' rows, or a line while no settings
    /// have come.
    ///
    /// Example: `let (shapes, size) = page.get_shapes();`
    pub fn get_shapes(&self) -> page_t {
        let mut shapes: Vec<Shape> = Vec::new();
        push_title(&mut shapes, PAGE_SIZE, TITLE, HELP);
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
    //! | Sections | shared keys, a level's own, a level with none (shown, empty), a level without a file, the levels' names | `sections_show_every_level` |
    //! | Changes | a step up (set sent), a reset (reset sent), a typed fraction where the default is whole (refused, nothing sent), a heading (nothing) | `changes_go_to_the_match_server` |
    //! | Server's answer | values follow it, same keys keep the selection, waiting before it, Escape closes | `the_page_follows_the_servers_answer` |
    //! | Steps | whole, 2.0, 0.33, 250, 0 | `steps_follow_the_default` |

    use super::*;
    use serde_json::json;
    use warped_shared::settings_editor::set_at;

    /// `make_settings` gives three levels: two sharing `respawn.delay_s` and `rifle.hits`, the
    /// second with `glass.bounces` alone, and one without a settings file.
    ///
    /// Example: `let settings = make_settings();`
    fn make_settings() -> GameSettings {
        GameSettings::New(vec![
            Some(json!({"respawn": {"delay_s": 3.0}, "rifle": {"hits": 2}})),
            Some(
                json!({"respawn": {"delay_s": 5.0}, "rifle": {"hits": 2}, "glass": {"bounces": 2}}),
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
        for _ in 0..page.row_keys.len() * 4 {
            let row: &EditorRow = page.editor.as_ref().unwrap().get_selected_row();
            if matches!(&row.kind, RowKind::Number { key, .. } if key == row_key) {
                return;
            }
            page.handle(MenuInput::Down);
        }
        panic!("no row {row_key}");
    }

    #[test]
    fn sections_show_every_level() {
        let page: GameSettingsPage = make_page();
        let text: String = get_text(&page);
        for line in [
            "EVERY LEVEL",
            "respawn.delay_s (levels differ)",
            "rifle.hits",
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
                "level_1.glass.bounces"
            ]
        );
    }

    #[test]
    fn changes_go_to_the_match_server() {
        let mut page: GameSettingsPage = make_page();
        assert_eq!(page.handle(MenuInput::Activate), GameSettingsOutcome::Stay);
        select(&mut page, "shared.rifle.hits");
        assert_eq!(
            page.handle(MenuInput::Right),
            GameSettingsOutcome::Send(SettingsMessage::Set {
                scope: SettingsScope::Shared,
                key: "rifle.hits".to_string(),
                value: 3.0,
            })
        );
        assert_eq!(
            page.handle(MenuInput::Reset),
            GameSettingsOutcome::Send(SettingsMessage::Reset {
                scope: SettingsScope::Shared,
                key: "rifle.hits".to_string(),
            })
        );
        for input in [
            MenuInput::Typed('2'),
            MenuInput::Typed('.'),
            MenuInput::Typed('5'),
        ] {
            page.handle(input);
        }
        assert_eq!(page.handle(MenuInput::Activate), GameSettingsOutcome::Stay);
        assert!(get_text(&page).contains("whole number"));
        select(&mut page, "level_1.glass.bounces");
        assert_eq!(
            page.handle(MenuInput::Left),
            GameSettingsOutcome::Send(SettingsMessage::Set {
                scope: SettingsScope::Level(1),
                key: "glass.bounces".to_string(),
                value: 1.0,
            })
        );
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
    fn steps_follow_the_default() {
        assert_eq!(get_step(&json!(2)), 1.0);
        assert!((get_step(&json!(2.0)) - 0.1).abs() < 1e-12);
        assert!((get_step(&json!(0.33)) - 0.01).abs() < 1e-12);
        assert!((get_step(&json!(250.0)) - 10.0).abs() < 1e-9);
        assert_eq!(get_step(&json!(0.0)), DEFAULT_STEP);
    }
}
