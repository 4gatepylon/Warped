//! `player_settings_page.rs` is the player's settings page, reachable from every menu: every key
//! binding and the level menu's numbers (`PLAYER_NUMBER_ROWS`), edited with the shared
//! settings editor and saved to the player's `player_settings.yaml`, the file every level reads.
//!
//! Interface: `PlayerSettingsPage` (`New`, `handle`, `is_rebinding`, `get_shapes`), `PageOutcome`.
//!
//! Example: `if page.handle(MenuInput::Back) == PageOutcome::Close { navigator.go_back(); }`
//!
//! Must know:
//! - Side effects: leaving the page after a change saves the file; so does its Save row.

use std::error::Error;
use std::path::{Path, PathBuf};

use serde_json::Value;
use warped_shared::key_bindings::KeyBindings;
use warped_shared::page::Shape;
use warped_shared::player_settings::{PLAYER_NUMBER_ROWS, PerPlayerSettings};
use warped_shared::settings_editor::{EditorOutcome, EditorRow, RowKind, SettingsEditor};

use crate::menu_page::{MenuInput, MenuRow, get_editor_input, push_rows, push_title};

const PAGE_SIZE: [f32; 2] = [640.0, 455.0];
const VISIBLE_ROW_COUNT: usize = 28;
const ROWS_TOP: f32 = 36.0;
const SAVE_ID: &str = "save";
const RESET_ALL_ID: &str = "reset_all";
const HELP: &str = "Return: rebind/type   Left/Right: change   R: reset row   Escape: save, back";
const SOURCE: &str = "the settings page";

/// `PageOutcome` is what the menu does after an input on the page: stay on it, or close it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageOutcome {
    Stay,
    Close,
}

/// `check_player_values` checks edited settings as loading them would.
///
/// Example: `check_player_values(&values)?;`
fn check_player_values(values: &Value) -> Result<(), Box<dyn Error>> {
    PerPlayerSettings::FromValue(values, SOURCE).map(|_| ())
}

/// `PlayerSettingsPage` is the page's state: the editor, the settings being edited and their
/// built-in defaults (as the file's keys), the file they save to, and whether they changed since
/// the last save.
pub struct PlayerSettingsPage {
    editor: SettingsEditor,
    values: Value,
    defaults: Value,
    path: PathBuf,
    is_changed: bool,
}

impl PlayerSettingsPage {
    /// `New` edits `settings`, saving to `path`.
    ///
    /// Example: `let page = PlayerSettingsPage::New(&mine, &path);`
    #[allow(non_snake_case)]
    pub fn New(settings: &PerPlayerSettings, path: &Path) -> PlayerSettingsPage {
        let mut rows: Vec<EditorRow> = KeyBindings::get_all()
            .iter()
            .map(|(_, name)| EditorRow {
                label: name.replace('_', " "),
                kind: RowKind::Keys {
                    action: name.to_string(),
                },
            })
            .collect();
        rows.extend(
            PLAYER_NUMBER_ROWS
                .iter()
                .map(|&(key, label, step, min, max)| EditorRow {
                    label: label.to_string(),
                    kind: RowKind::Number {
                        key: key.to_string(),
                        step,
                        min,
                        max,
                    },
                }),
        );
        rows.push(EditorRow {
            label: format!("Save to {}", path.display()),
            kind: RowKind::Button { id: SAVE_ID },
        });
        rows.push(EditorRow {
            label: "Reset everything to the built-in settings".to_string(),
            kind: RowKind::Button { id: RESET_ALL_ID },
        });
        PlayerSettingsPage {
            editor: SettingsEditor::New("PLAYER SETTINGS", rows),
            values: settings.to_value(),
            defaults: PerPlayerSettings::Builtin().to_value(),
            path: path.to_path_buf(),
            is_changed: false,
        }
    }

    /// `is_rebinding` says whether the page waits for the control to bind, so the window passes
    /// the next control's name (and Escape as `Back`).
    ///
    /// Example: `if page.is_rebinding() { ... }`
    pub fn is_rebinding(&self) -> bool {
        self.editor.is_rebinding()
    }

    /// `save` writes the settings to the page's file and says so on the page.
    ///
    /// Example: `self.save()?;`
    fn save(&mut self) -> Result<(), Box<dyn Error>> {
        PerPlayerSettings::FromValue(&self.values, SOURCE)?.save(&self.path)?;
        self.is_changed = false;
        self.editor
            .show_message(format!("Saved to {}.", self.path.display()));
        Ok(())
    }

    /// `handle` applies one input; going back saves a changed page first and closes it (or stays,
    /// showing why the save failed).
    ///
    /// Example: `let outcome = page.handle(MenuInput::Right);`
    pub fn handle(&mut self, input: MenuInput) -> PageOutcome {
        let outcome: EditorOutcome = self.editor.handle(
            get_editor_input(input),
            &mut self.values,
            &self.defaults,
            &check_player_values,
        );
        let result: Result<PageOutcome, Box<dyn Error>> = match outcome {
            EditorOutcome::Stay => Ok(PageOutcome::Stay),
            EditorOutcome::Changed => {
                self.is_changed = true;
                Ok(PageOutcome::Stay)
            }
            EditorOutcome::Pressed(id) if id == SAVE_ID => self.save().map(|()| PageOutcome::Stay),
            EditorOutcome::Pressed(_) => {
                self.values = self.defaults.clone();
                self.is_changed = true;
                Ok(PageOutcome::Stay)
            }
            EditorOutcome::Back if self.is_changed => self.save().map(|()| PageOutcome::Close),
            EditorOutcome::Back => Ok(PageOutcome::Close),
        };
        result.unwrap_or_else(|error| {
            self.editor.show_message(error.to_string());
            PageOutcome::Stay
        })
    }

    /// `get_shapes` gives the page and its size.
    ///
    /// Example: `let (shapes, size) = page.get_shapes();`
    pub fn get_shapes(&self) -> (Vec<Shape>, [f32; 2]) {
        let mut shapes: Vec<Shape> = Vec::new();
        push_title(&mut shapes, PAGE_SIZE, &self.editor.title, HELP);
        let lines: Vec<(String, bool)> =
            self.editor
                .get_lines(&self.values, &self.defaults, VISIBLE_ROW_COUNT);
        let selected: Option<usize> = lines.iter().position(|(_, is_selected)| *is_selected);
        let rows: Vec<Option<MenuRow>> = lines
            .into_iter()
            .map(|(text, _)| {
                Some(MenuRow {
                    text,
                    is_enabled: true,
                })
            })
            .collect();
        push_rows(&mut shapes, ROWS_TOP, &rows, selected);
        (shapes, PAGE_SIZE)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `player_settings_page.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Leaving | unchanged (no file written), changed (saved, loads back) | `leaving_saves_only_changes` |
    //! | Rows | every action and every number has a row | `every_setting_has_a_row` |
    //! | Buttons | save, reset everything | `the_buttons_save_and_reset` |

    use super::*;
    use warped_shared::key_bindings::Action;

    /// `get_test_path` gives a scratch settings file for `name`, never the player's own.
    ///
    /// Example: `let path = get_test_path("leave");`
    fn get_test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("warped_page_{name}_{}.yaml", std::process::id()))
    }

    #[test]
    fn leaving_saves_only_changes() {
        let path: PathBuf = get_test_path("leave");
        let mut page: PlayerSettingsPage =
            PlayerSettingsPage::New(&PerPlayerSettings::Builtin(), &path);
        assert_eq!(page.handle(MenuInput::Back), PageOutcome::Close);
        assert!(!path.exists());
        let mut page: PlayerSettingsPage =
            PlayerSettingsPage::New(&PerPlayerSettings::Builtin(), &path);
        page.handle(MenuInput::Activate);
        assert!(page.is_rebinding());
        page.handle(MenuInput::Control("Up".to_string()));
        assert_eq!(page.handle(MenuInput::Back), PageOutcome::Close);
        let saved: PerPlayerSettings = PerPlayerSettings::Load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(saved.keys.get_names(Action::MoveForward), "Up");
    }

    #[test]
    fn every_setting_has_a_row() {
        let page: PlayerSettingsPage =
            PlayerSettingsPage::New(&PerPlayerSettings::Builtin(), &get_test_path("rows"));
        let text: String = page
            .editor
            .get_lines(&page.values, &page.defaults, usize::MAX)
            .into_iter()
            .map(|(line, _)| line)
            .collect::<Vec<String>>()
            .join("\n");
        for (_, name) in KeyBindings::get_all() {
            assert!(text.contains(&name.replace('_', " ")), "{name}");
        }
        for (_, label, ..) in PLAYER_NUMBER_ROWS {
            assert!(text.contains(label), "{label}");
        }
    }

    #[test]
    fn the_buttons_save_and_reset() {
        let path: PathBuf = get_test_path("buttons");
        let mut mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        mine.view.motion_blur_s = 0.0;
        let mut page: PlayerSettingsPage = PlayerSettingsPage::New(&mine, &path);
        page.handle(MenuInput::Up);
        page.handle(MenuInput::Activate);
        assert_eq!(page.values, page.defaults);
        page.handle(MenuInput::Up);
        page.handle(MenuInput::Activate);
        let saved: PerPlayerSettings = PerPlayerSettings::Load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(saved.to_yaml(), PerPlayerSettings::Builtin().to_yaml());
    }
}
