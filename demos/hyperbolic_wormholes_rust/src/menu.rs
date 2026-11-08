//! `menu.rs` is the in-game menu where a player changes their own settings (`player_settings.yaml`):
//! every key binding, the mouse look speed, the head bob, motion blur and how a hit looks, and a row
//! that saves them. It keeps no window state: the window feeds it pressed controls and draws its
//! lines.
//!
//! Interface: `Menu` (`New`, `handle`, `get_lines`, `show_message`), `MenuOutcome`.
//!
//! Example: `if menu.handle(control, &mut mine) == MenuOutcome::Close { ... }`
//!
//! Must know:
//! - Inside the menu the arrows, `Return` and `Escape` work whatever the key bindings say, so a bad
//!   binding can always be fixed.

use sdl3::keyboard::Scancode;

use crate::key_bindings::{Action, Control, KeyBindings};
use crate::settings::PerPlayerSettings;

const VISIBLE_ROW_COUNT: usize = 20;
const LABEL_WIDTH: usize = 24;
const NUMBERS: [(Number, &str, f64, f64, f64); 7] = [
    (
        Number::LookSpeed,
        "mouse look rad/px",
        0.00025,
        0.0005,
        0.01,
    ),
    (Number::MotionBlur, "motion blur s", 0.005, 0.0, 0.05),
    (Number::WalkBob, "head bob walking m", 0.01, 0.0, 0.3),
    (Number::SprintBob, "head bob sprinting m", 0.01, 0.0, 0.4),
    (Number::HitRed, "hit red 0-1", 0.1, 0.0, 1.0),
    (Number::HitShake, "hit shake px", 1.0, 0.0, 20.0),
    (Number::HitBlur, "hit blur px", 1.0, 0.0, 20.0),
];

/// `Number` is one of the player's numeric settings that the menu changes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Number {
    LookSpeed,
    MotionBlur,
    WalkBob,
    SprintBob,
    HitRed,
    HitShake,
    HitBlur,
}

/// `Row` is one line of the menu that can be chosen: an action's controls, a number, or saving.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Row {
    Key(Action, &'static str),
    Number(Number, &'static str, f64, f64, f64),
    Save,
}

/// `MenuOutcome` is what the window does after a control in the menu: nothing, close the menu, or
/// save the settings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MenuOutcome {
    Stay,
    Close,
    Save,
}

/// `get_rows` gives the menu's rows in order: every action, then the numbers, then saving.
///
/// Example: `let rows = get_rows();`
fn get_rows() -> Vec<Row> {
    KeyBindings::get_all()
        .iter()
        .map(|&(action, name)| Row::Key(action, name))
        .chain(
            NUMBERS.iter().map(|&(number, label, step, low, high)| {
                Row::Number(number, label, step, low, high)
            }),
        )
        .chain([Row::Save])
        .collect()
}

/// `get_number` gives a numeric setting's value.
///
/// Example: `let speed = get_number(&mine, Number::LookSpeed);`
fn get_number(settings: &PerPlayerSettings, number: Number) -> f64 {
    match number {
        Number::LookSpeed => settings.view.look_rad_per_px,
        Number::MotionBlur => settings.view.motion_blur_s,
        Number::WalkBob => settings.view.head_bob.walk_depth_m,
        Number::SprintBob => settings.view.head_bob.sprint_depth_m,
        Number::HitRed => settings.hit_effects.red_frac,
        Number::HitShake => settings.hit_effects.shake_px,
        Number::HitBlur => settings.hit_effects.blur_px,
    }
}

/// `get_number_slot` lends the field holding a numeric setting.
///
/// Example: `*get_number_slot(&mut mine, Number::HitRed) = 0.3;`
fn get_number_slot(settings: &mut PerPlayerSettings, number: Number) -> &mut f64 {
    match number {
        Number::LookSpeed => &mut settings.view.look_rad_per_px,
        Number::MotionBlur => &mut settings.view.motion_blur_s,
        Number::WalkBob => &mut settings.view.head_bob.walk_depth_m,
        Number::SprintBob => &mut settings.view.head_bob.sprint_depth_m,
        Number::HitRed => &mut settings.hit_effects.red_frac,
        Number::HitShake => &mut settings.hit_effects.shake_px,
        Number::HitBlur => &mut settings.hit_effects.blur_px,
    }
}

/// `Menu` is the menu's state: the chosen row, whether it waits for the control to bind to that
/// row's action, and a line of news (a saved file, a refused binding).
pub struct Menu {
    row: usize,
    is_waiting_for_control: bool,
    message: String,
}

impl Menu {
    /// `New` opens the menu on its first row.
    ///
    /// Example: `let menu = Menu::New();`
    #[allow(non_snake_case)]
    pub fn New() -> Menu {
        Menu {
            row: 0,
            is_waiting_for_control: false,
            message: String::new(),
        }
    }

    /// `show_message` sets the menu's line of news.
    ///
    /// Example: `menu.show_message("Saved".to_string());`
    pub fn show_message(&mut self, message: String) {
        self.message = message;
    }

    /// `handle` reacts to a pressed control: while waiting, it binds the control to the chosen
    /// action (`Escape` cancels); otherwise the arrows move between rows and change numbers,
    /// `Return` starts a binding or saves, and `Escape` or the open-menu control closes the menu.
    ///
    /// Example: `let outcome = menu.handle(Control::Key(Scancode::Down), &mut mine);`
    pub fn handle(&mut self, control: Control, settings: &mut PerPlayerSettings) -> MenuOutcome {
        let rows: Vec<Row> = get_rows();
        let row: Row = rows[self.row];
        if self.is_waiting_for_control {
            self.is_waiting_for_control = false;
            if let (Row::Key(action, name), false) =
                (row, control == Control::Key(Scancode::Escape))
            {
                self.message = match settings.keys.set_controls(action, vec![control]) {
                    Ok(()) => format!("{name} is now {}", settings.keys.get_names(action)),
                    Err(error) => format!("Not changed: {error}"),
                };
            }
            return MenuOutcome::Stay;
        }
        let is_closing: bool = control == Control::Key(Scancode::Escape)
            || settings.keys.matches(Action::OpenMenu, control);
        match (control, row) {
            _ if is_closing => return MenuOutcome::Close,
            (Control::Key(Scancode::Up), _) => self.row = (self.row + rows.len() - 1) % rows.len(),
            (Control::Key(Scancode::Down), _) => self.row = (self.row + 1) % rows.len(),
            (Control::Key(Scancode::Return), Row::Key(_, name)) => {
                self.is_waiting_for_control = true;
                self.message =
                    format!("Press the key, button or wheel for {name} (Escape cancels)");
            }
            (Control::Key(Scancode::Return), Row::Save) => return MenuOutcome::Save,
            (
                Control::Key(key @ (Scancode::Left | Scancode::Right)),
                Row::Number(n, _, step, low, high),
            ) => {
                let delta: f64 = if key == Scancode::Left { -step } else { step };
                let slot: &mut f64 = get_number_slot(settings, n);
                *slot = (*slot + delta).clamp(low, high);
            }
            _ => {}
        }
        MenuOutcome::Stay
    }

    /// `get_lines` gives the menu's text, each line with whether it is highlighted: a title, a
    /// hint, the rows around the chosen one (`VISIBLE_ROW_COUNT` of them), and the line of news.
    /// `save_path` is where saving writes.
    ///
    /// Example: `let lines = menu.get_lines(&mine, "~/.config/warped/player_settings.yaml");`
    pub fn get_lines(&self, settings: &PerPlayerSettings, save_path: &str) -> Vec<(String, bool)> {
        let rows: Vec<Row> = get_rows();
        let first: usize = self
            .row
            .saturating_sub(VISIBLE_ROW_COUNT / 2)
            .min(rows.len().saturating_sub(VISIBLE_ROW_COUNT));
        let mut lines: Vec<(String, bool)> = vec![
            ("MENU: YOUR SETTINGS".to_string(), true),
            (
                "Up/Down choose, Return change, Left/Right adjust, Esc closes".to_string(),
                false,
            ),
            (String::new(), false),
        ];
        for (i, row) in rows.iter().enumerate().skip(first).take(VISIBLE_ROW_COUNT) {
            let text: String = match *row {
                Row::Key(action, name) => {
                    format!("{name:<LABEL_WIDTH$}{}", settings.keys.get_names(action))
                }
                Row::Number(number, label, _, _, _) => {
                    format!(
                        "{label:<LABEL_WIDTH$}< {:.4} >",
                        get_number(settings, number)
                    )
                }
                Row::Save => format!("Save to {save_path}"),
            };
            lines.push((text, i == self.row));
        }
        lines.push((String::new(), false));
        lines.push((self.message.clone(), false));
        lines
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the menu's handling of controls.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Row kind | an action (rebound, or cancelled with Escape), a number (stepped, clamped), saving | `rows_rebind_step_and_save` |
    //! | Closing | Escape, the open-menu control | `escape_and_the_menu_key_close_it` |

    use super::*;

    /// `move_to` presses Down until the menu's chosen row is `target`.
    ///
    /// Example: `move_to(&mut menu, &mut mine, Row::Save);`
    fn move_to(menu: &mut Menu, settings: &mut PerPlayerSettings, target: Row) {
        while get_rows()[menu.row] != target {
            menu.handle(Control::Key(Scancode::Down), settings);
        }
    }

    #[test]
    fn rows_rebind_step_and_save() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        let jump: Row = Row::Key(Action::Jump, "jump");
        move_to(&mut menu, &mut mine, jump);
        menu.handle(Control::Key(Scancode::Return), &mut mine);
        menu.handle(Control::Key(Scancode::Escape), &mut mine);
        assert_eq!(mine.keys.get_names(Action::Jump), "Space");
        menu.handle(Control::Key(Scancode::Return), &mut mine);
        menu.handle(Control::Key(Scancode::J), &mut mine);
        assert_eq!(mine.keys.get_names(Action::Jump), "J");
        let red: Row = get_rows()
            .into_iter()
            .find(|row| matches!(row, Row::Number(Number::HitRed, ..)))
            .expect("a red row");
        move_to(&mut menu, &mut mine, red);
        for _ in 0..NUMBERS.len() * 3 {
            menu.handle(Control::Key(Scancode::Right), &mut mine);
        }
        assert_eq!(mine.hit_effects.red_frac, 1.0);
        move_to(&mut menu, &mut mine, Row::Save);
        assert_eq!(
            menu.handle(Control::Key(Scancode::Return), &mut mine),
            MenuOutcome::Save
        );
    }

    #[test]
    fn escape_and_the_menu_key_close_it() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        assert_eq!(
            menu.handle(Control::Key(Scancode::Escape), &mut mine),
            MenuOutcome::Close
        );
        assert_eq!(
            menu.handle(Control::Key(Scancode::Tab), &mut mine),
            MenuOutcome::Close
        );
    }
}
