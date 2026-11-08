//! `menu.rs` is the in-game menu, in pages under a tab bar: the settings page, where a player
//! changes their own settings (`player_settings.yaml`: every key binding, the mouse look speed, the
//! head bob, motion blur and how a hit looks, and a row that saves them), the map page, whose
//! picture the window draws (`map_layout.rs`), and the scores page, with each player's team, kills
//! and deaths from the server. It keeps no window state: the window feeds it pressed controls and
//! the scores, and draws its lines.
//!
//! Interface: `Menu` (`New`, `handle`, `get_lines`, `get_page`, `show_message`), `MenuOutcome`,
//! `Page`, `Scoreboard`.
//!
//! Example: `if menu.handle(control, &mut mine) == MenuOutcome::Close { ... }`
//!
//! Must know:
//! - Inside the menu the arrows, `Return`, `Escape`, `Q` and `E` work whatever the key bindings
//!   say, so a bad binding can always be fixed.

use std::cmp::Reverse;

use sdl3::keyboard::Scancode;

use crate::key_bindings::{Action, Control, KeyBindings};
use crate::net::{Score, TEAM_NAMES};
use crate::settings::PerPlayerSettings;

const VISIBLE_ROW_COUNT: usize = 20;
const PAGES: [(Page, &str); 3] = [
    (Page::Settings, "SETTINGS"),
    (Page::Map, "MAP"),
    (Page::Scores, "SCORES"),
];
const MAP_LEGEND: &str = "You: arrow  Squares: consoles  Rings: wormholes  Dashes: zones";
const NO_SERVER_LINE: &str = "No game on a server: host or join one to keep scores.";
const WAITING_LINE: &str = "No scores from the server yet: join a team first.";
const MAX_SCORE_ROW_COUNT: usize = 16;
const TEAM_WIDTH: usize = 6;
const PLAYER_WIDTH: usize = 16;
const KILLS_WIDTH: usize = 6;
const DEATHS_WIDTH: usize = 8;
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

/// `Page` is one page of the menu.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Settings,
    Map,
    Scores,
}

/// `Scoreboard` is what the scores page shows: that there is no game on a server, that the server
/// hasn't sent scores yet, or the latest scores of every player in the game with this player's id
/// (`None` before the server's first welcome).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Scoreboard<'a> {
    NoServer,
    Waiting,
    Scores {
        scores: &'a [Score],
        own_id: Option<u32>,
    },
}

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

/// `format_score_row` lays out one row of the scores page in its columns: team, player, kills and
/// deaths.
///
/// Example: `let header = format_score_row("TEAM", "PLAYER", "KILLS", "DEATHS");`
fn format_score_row(team: &str, player: &str, kills: &str, deaths: &str) -> String {
    format!(
        "{team:<TEAM_WIDTH$}{player:<PLAYER_WIDTH$}{kills:>KILLS_WIDTH$}{deaths:>DEATHS_WIDTH$}"
    )
}

/// `get_score_lines` gives the scores page's lines below the tab bar: without scores, why there
/// are none; with them, how many players are in the game and on each team, a row per player
/// (team, id, kills and deaths, at most `MAX_SCORE_ROW_COUNT`) sorted by team and then by most
/// kills, with this player's row marked "(you)" and highlighted, and each team's totals.
///
/// Example: `let lines = get_score_lines(Scoreboard::NoServer);`
fn get_score_lines(scoreboard: Scoreboard) -> Vec<(String, bool)> {
    let (scores, own_id) = match scoreboard {
        Scoreboard::NoServer => return vec![(NO_SERVER_LINE.to_string(), false)],
        Scoreboard::Waiting => return vec![(WAITING_LINE.to_string(), false)],
        Scoreboard::Scores { scores, own_id } => (scores, own_id),
    };
    let mut sorted: Vec<Score> = scores.to_vec();
    sorted.sort_by_key(|score| (score.team, Reverse(score.kills), score.id));
    let team_scores = |team: usize| sorted.iter().filter(move |s| s.team as usize == team);
    let team_counts: Vec<String> = TEAM_NAMES
        .iter()
        .enumerate()
        .map(|(team, name)| format!("{name} {}", team_scores(team).count()))
        .collect();
    let mut lines: Vec<(String, bool)> = vec![
        (
            format!("Players: {} ({})", sorted.len(), team_counts.join(", ")),
            false,
        ),
        (String::new(), false),
        (format_score_row("TEAM", "PLAYER", "KILLS", "DEATHS"), false),
    ];
    for score in sorted.iter().take(MAX_SCORE_ROW_COUNT) {
        let is_own: bool = Some(score.id) == own_id;
        let player: String = match is_own {
            true => format!("player {} (you)", score.id),
            false => format!("player {}", score.id),
        };
        let row: String = format_score_row(
            TEAM_NAMES[score.team as usize],
            &player,
            &score.kills.to_string(),
            &score.deaths.to_string(),
        );
        lines.push((row, is_own));
    }
    lines.push((String::new(), false));
    for (team, name) in TEAM_NAMES.iter().enumerate() {
        let kills: u32 = team_scores(team).map(|score| score.kills).sum();
        let deaths: u32 = team_scores(team).map(|score| score.deaths).sum();
        let row: String = format_score_row(name, "total", &kills.to_string(), &deaths.to_string());
        lines.push((row, false));
    }
    lines
}

/// `Menu` is the menu's state: the page shown (an index into `PAGES`), the settings page's chosen
/// row, whether it waits for the control to bind to that row's action, and a line of news (a saved
/// file, a refused binding).
pub struct Menu {
    page_index: usize,
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
            page_index: 0,
            row: 0,
            is_waiting_for_control: false,
            message: String::new(),
        }
    }

    /// `get_page` gives the page shown.
    ///
    /// Example: `if menu.get_page() == Page::Map { ... }`
    pub fn get_page(&self) -> Page {
        PAGES[self.page_index].0
    }

    /// `show_message` sets the menu's line of news.
    ///
    /// Example: `menu.show_message("Saved".to_string());`
    pub fn show_message(&mut self, message: String) {
        self.message = message;
    }

    /// `handle` reacts to a pressed control: while waiting, it binds the control to the chosen
    /// action (`Escape` cancels); otherwise `Q` and `E` turn to the previous and next page,
    /// `Escape` or the open-menu control closes the menu, and on the settings page the arrows move
    /// between rows and change numbers and `Return` starts a binding or saves.
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
        match control {
            _ if is_closing => return MenuOutcome::Close,
            Control::Key(Scancode::Q) => {
                self.page_index = (self.page_index + PAGES.len() - 1) % PAGES.len();
                return MenuOutcome::Stay;
            }
            Control::Key(Scancode::E) => {
                self.page_index = (self.page_index + 1) % PAGES.len();
                return MenuOutcome::Stay;
            }
            _ if self.get_page() != Page::Settings => return MenuOutcome::Stay,
            _ => {}
        }
        match (control, row) {
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

    /// `get_lines` gives the menu's text, each line with whether it is highlighted: the tab bar,
    /// then on the settings page a hint, the rows around the chosen one (`VISIBLE_ROW_COUNT` of
    /// them) and the line of news, on the map page the map's legend, or on the scores page
    /// `scoreboard`'s lines (`get_score_lines`). `save_path` is where saving writes.
    ///
    /// Example: `let lines = menu.get_lines(&mine, "~/.config/warped/player_settings.yaml", Scoreboard::NoServer);`
    pub fn get_lines(
        &self,
        settings: &PerPlayerSettings,
        save_path: &str,
        scoreboard: Scoreboard,
    ) -> Vec<(String, bool)> {
        let rows: Vec<Row> = get_rows();
        let first: usize = self
            .row
            .saturating_sub(VISIBLE_ROW_COUNT / 2)
            .min(rows.len().saturating_sub(VISIBLE_ROW_COUNT));
        let tabs: Vec<String> = PAGES
            .iter()
            .enumerate()
            .map(|(i, (_, name))| match i == self.page_index {
                true => format!("[{name}]"),
                false => format!(" {name} "),
            })
            .collect();
        let tab_bar: String = format!("MENU  {}     Q/E: page", tabs.join(" "));
        match self.get_page() {
            Page::Map => return vec![(tab_bar, true), (MAP_LEGEND.to_string(), false)],
            Page::Scores => {
                let mut lines: Vec<(String, bool)> = vec![(tab_bar, true), (String::new(), false)];
                lines.extend(get_score_lines(scoreboard));
                return lines;
            }
            Page::Settings => {}
        }
        let mut lines: Vec<(String, bool)> = vec![
            (tab_bar, true),
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
    //! | Page | turned forward past the last and back past the first; the map page ignoring the settings keys | `q_and_e_turn_pages` |
    //! | Scoreboard | no server, waiting for scores, scores (two teams, ties in kills, this player known or not yet) | `the_scores_page_sorts_and_totals` |

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
    fn q_and_e_turn_pages() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        for expected in [Page::Map, Page::Scores, Page::Settings] {
            menu.handle(Control::Key(Scancode::E), &mut mine);
            assert_eq!(menu.get_page(), expected);
        }
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        assert_eq!(menu.get_page(), Page::Map);
        menu.handle(Control::Key(Scancode::Down), &mut mine);
        menu.handle(Control::Key(Scancode::Return), &mut mine);
        assert!(!menu.is_waiting_for_control && menu.row == 0);
        assert_eq!(
            menu.get_lines(&mine, "", Scoreboard::NoServer)[1].0,
            MAP_LEGEND
        );
        assert_eq!(
            menu.handle(Control::Key(Scancode::Escape), &mut mine),
            MenuOutcome::Close
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

    #[test]
    fn the_scores_page_sorts_and_totals() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        assert_eq!(menu.get_page(), Page::Scores);
        let get_texts = |scoreboard: Scoreboard| -> Vec<(String, bool)> {
            menu.get_lines(&mine, "", scoreboard)[2..].to_vec()
        };
        assert_eq!(get_texts(Scoreboard::NoServer)[0].0, NO_SERVER_LINE);
        assert_eq!(get_texts(Scoreboard::Waiting)[0].0, WAITING_LINE);
        let score = |id: u32, team: u32, kills: u32, deaths: u32| Score {
            id,
            team,
            kills,
            deaths,
        };
        let scores: Vec<Score> = vec![
            score(1, 1, 0, 2),
            score(2, 0, 1, 0),
            score(3, 1, 4, 1),
            score(4, 0, 3, 1),
            score(5, 0, 3, 0),
        ];
        let lines: Vec<(String, bool)> = get_texts(Scoreboard::Scores {
            scores: &scores,
            own_id: Some(5),
        });
        let row = |team: &str, player: &str, kills: u32, deaths: u32| {
            format_score_row(team, player, &kills.to_string(), &deaths.to_string())
        };
        let expected: Vec<(String, bool)> = vec![
            ("Players: 5 (blue 3, red 2)".to_string(), false),
            (String::new(), false),
            (format_score_row("TEAM", "PLAYER", "KILLS", "DEATHS"), false),
            (row("blue", "player 4", 3, 1), false),
            (row("blue", "player 5 (you)", 3, 0), true),
            (row("blue", "player 2", 1, 0), false),
            (row("red", "player 3", 4, 1), false),
            (row("red", "player 1", 0, 2), false),
            (String::new(), false),
            (row("blue", "total", 7, 1), false),
            (row("red", "total", 4, 3), false),
        ];
        assert_eq!(lines, expected);
        let not_welcomed: Vec<(String, bool)> = get_texts(Scoreboard::Scores {
            scores: &scores,
            own_id: None,
        });
        assert!(
            not_welcomed
                .iter()
                .all(|(_, is_highlighted)| !is_highlighted)
        );
    }
}
