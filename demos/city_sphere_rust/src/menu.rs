//! `menu.rs` is the in-game menu, in pages under a tab bar: the settings page, where a player
//! changes their own settings (`player_settings.yaml`: every key binding, the mouse look speed, the
//! head bob, motion blur, how a hit looks, the wormhole tint, the Milky Way's brightness, the
//! picture's brightness, contrast, saturation and gamma (W-51, basic visuals), and a row that saves
//! them), the map page, whose
//! picture the window draws (`map_layout.rs`), the scores page, with each player's team, kills
//! and deaths from the server, the credits page, with where the game's assets come from
//! (`credits.rs`), the quit page, which leaves the level, and, when the caller supplies its lines
//! (the one-window app's match status, `set_match_lines`), a MATCH page first. It keeps no window
//! state: the window
//! feeds it pressed controls and the scores, and draws its lines. `Esc` opens it and closes it, and
//! the map page's lines also head the map shown while `Tab` is held (W-140, hold Tab for the map,
//! Esc for the menu; `warped_shared::menu_and_map`).
//!
//! Interface: `Menu` (`New`, `handle`, `get_lines`, `get_page`, `show_message`,
//! `set_match_lines`), `MenuOutcome`,
//! `Page`, `Scoreboard`, `get_held_map_lines`.
//!
//! Example: `if menu.handle(control, &mut mine) == MenuOutcome::Close { ... }`
//!
//! Must know:
//! - Inside the menu the arrows, `Return`, `Escape`, `Q` and `E` work whatever the key bindings
//!   say, so a bad binding can always be fixed.

use std::cmp::Reverse;

use sdl3::keyboard::Scancode;

use crate::credits::CREDITS_LINES;
use crate::key_bindings::{Action, Control, KeyBindings};
use crate::menu_and_map::{MATCH_PAGE_NAME, QUIT_LINES, format_tab_bar, is_leave_control};
use crate::net::{Score, TEAM_NAMES};
use crate::settings::PerPlayerSettings;
use warped_shared::visuals::{VISUALS_ROWS, VisualsKnob};

const VISIBLE_ROW_COUNT: usize = 20;
const PAGES: [(Page, &str); 5] = [
    (Page::Scores, "SCORES"),
    (Page::Settings, "SETTINGS"),
    (Page::Map, "MAP"),
    (Page::Credits, "CREDITS"),
    (Page::Quit, "QUIT"),
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
const NUMBERS: [(Number, &str, f64, f64, f64); 9] = [
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
    (Number::WormholeTint, "wormhole tint 0-1", 0.1, 0.0, 1.0),
    (Number::MilkyWay, "milky way 0-1", 0.1, 0.0, 1.0),
];

/// `Page` is one page of the menu.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Settings,
    Map,
    Scores,
    Credits,
    Quit,
    Match,
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
    WormholeTint,
    MilkyWay,
    Visual(VisualsKnob),
}

/// `Row` is one line of the menu that can be chosen: an action's controls, a number, or saving.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Row {
    Key(Action, &'static str),
    Number(Number, &'static str, f64, f64, f64),
    Save,
}

/// `MenuOutcome` is what the window does after a control in the menu: nothing, close the menu,
/// save the settings, or leave the level (`menu_and_map::PlayOutcome::LeaveLevel`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MenuOutcome {
    Stay,
    Close,
    Save,
    LeaveLevel,
}

/// `get_held_map_lines` gives the lines over the map shown while the show-map control is held: a
/// title naming that control, and the map's legend.
///
/// Example: `let lines = get_held_map_lines(&mine.keys);`
pub fn get_held_map_lines(keys: &KeyBindings) -> Vec<(String, bool)> {
    vec![
        (
            format!(
                "MAP     let go of {} to hide it",
                keys.get_label(Action::ShowMap)
            ),
            true,
        ),
        (MAP_LEGEND.to_string(), false),
    ]
}

/// `get_rows` gives the menu's rows in order: every action, then the numbers, then the visuals, then
/// saving.
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
        .chain(VISUALS_ROWS.iter().map(|&(knob, label, step, low, high)| {
            Row::Number(Number::Visual(knob), label, step, low, high)
        }))
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
        Number::WormholeTint => settings.view.wormhole_tint_frac,
        Number::MilkyWay => settings.view.milky_way_brightness_frac,
        Number::Visual(knob) => settings.visuals.get(knob),
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
        Number::WormholeTint => &mut settings.view.wormhole_tint_frac,
        Number::MilkyWay => &mut settings.view.milky_way_brightness_frac,
        Number::Visual(knob) => settings.visuals.get_mut(knob),
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

/// `Menu` is the menu's state: the page shown (an index into `get_pages`), the settings page's
/// chosen row, whether it waits for the control to bind to that row's action, a line of news (a
/// saved file, a refused binding), and the MATCH page's lines (`None`: no MATCH page).
pub struct Menu {
    page_index: usize,
    row: usize,
    is_waiting_for_control: bool,
    message: String,
    match_lines: Option<Vec<(String, bool)>>,
}

impl Menu {
    /// `New` opens the menu on its first page, SCORES, with the settings page's first row chosen.
    ///
    /// Example: `let menu = Menu::New();`
    #[allow(non_snake_case)]
    pub fn New() -> Menu {
        Menu {
            page_index: 0,
            row: 0,
            is_waiting_for_control: false,
            message: String::new(),
            match_lines: None,
        }
    }

    /// `get_pages` gives the menu's pages in order: MATCH first when it has lines, then `PAGES`.
    ///
    /// Example: `let page_count = self.get_pages().len();`
    fn get_pages(&self) -> Vec<(Page, &'static str)> {
        let match_page: Option<(Page, &'static str)> = self
            .match_lines
            .as_ref()
            .map(|_| (Page::Match, MATCH_PAGE_NAME));
        match_page.into_iter().chain(PAGES).collect()
    }

    /// `set_match_lines` gives the MATCH page its lines (the one-window app's match status, at
    /// most 64 characters wide and 22 lines), or removes the page with `None`. The page opens
    /// first: the menu turns to it when it first gets lines, and keeps its page as they change.
    ///
    /// Example: `menu.set_match_lines(Some(vec![("ROUND 2 of 5".to_string(), true)]));`
    #[allow(dead_code)]
    pub fn set_match_lines(&mut self, lines: Option<Vec<(String, bool)>>) {
        match (self.match_lines.is_some(), lines.is_some()) {
            (false, true) => self.page_index = 0,
            (true, false) => self.page_index = self.page_index.saturating_sub(1),
            _ => {}
        }
        self.match_lines = lines;
    }

    /// `get_page` gives the page shown.
    ///
    /// Example: `if menu.get_page() == Page::Map { ... }`
    pub fn get_page(&self) -> Page {
        self.get_pages()[self.page_index].0
    }

    /// `show_message` sets the menu's line of news.
    ///
    /// Example: `menu.show_message("Saved".to_string());`
    pub fn show_message(&mut self, message: String) {
        self.message = message;
    }

    /// `handle` reacts to a pressed control: while waiting, it binds the control to the chosen
    /// action (`Escape` cancels); otherwise `Q` and `E` turn to the previous and next page,
    /// `Escape` or the open-menu control closes the menu, on the quit page `Return` or `Y` leaves
    /// the level, and on the settings page the arrows move between rows and change numbers and
    /// `Return` starts a binding or saves.
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
                let page_count: usize = self.get_pages().len();
                self.page_index = (self.page_index + page_count - 1) % page_count;
                return MenuOutcome::Stay;
            }
            Control::Key(Scancode::E) => {
                self.page_index = (self.page_index + 1) % self.get_pages().len();
                return MenuOutcome::Stay;
            }
            _ if self.get_page() == Page::Quit && is_leave_control(control) => {
                return MenuOutcome::LeaveLevel;
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
    /// them) and the line of news, on the map page the map's legend, on the scores page
    /// `scoreboard`'s lines (`get_score_lines`), on the credits page `CREDITS_LINES`, or on the
    /// quit page `QUIT_LINES`.
    /// `save_path` is where saving writes.
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
        let page_names: Vec<&str> = self.get_pages().iter().map(|(_, name)| *name).collect();
        let tab_bar: String = format_tab_bar(&page_names, self.page_index);
        match self.get_page() {
            Page::Match => {
                let mut lines: Vec<(String, bool)> = vec![(tab_bar, true), (String::new(), false)];
                lines.extend(self.match_lines.iter().flatten().cloned());
                return lines;
            }
            Page::Map => return vec![(tab_bar, true), (MAP_LEGEND.to_string(), false)],
            Page::Scores => {
                let mut lines: Vec<(String, bool)> = vec![(tab_bar, true), (String::new(), false)];
                lines.extend(get_score_lines(scoreboard));
                return lines;
            }
            Page::Credits | Page::Quit => {
                let page_lines: &[&str] = match self.get_page() {
                    Page::Credits => &CREDITS_LINES,
                    _ => &QUIT_LINES,
                };
                let mut lines: Vec<(String, bool)> = vec![(tab_bar, true), (String::new(), false)];
                lines.extend(page_lines.iter().map(|line| (line.to_string(), false)));
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
    //! | Visuals row (W-51, basic visuals) | stepped up to its highest, down to its lowest, back to 1 | `visuals_rows_step_within_their_ranges` |
    //! | Closing | Escape (the built-in open-menu control), another open-menu control; not the show-map control (`Tab`) | `escape_and_the_menu_key_close_it` |
    //! | Page | the first (SCORES, where the menu opens), turned forward past the last and back past the first; the map page ignoring the settings keys | `q_and_e_turn_pages` |
    //! | Quit page | `Return` or `Y` (leave the level), another key (stay); its lines; `Return` elsewhere (no leaving) | `the_quit_page_leaves_the_level` |
    //! | Held map | its title names the show-map control, then the legend | `the_held_map_names_its_key` |
    //! | MATCH page | none (a level run alone), lines supplied (shown first, in the page turning, its lines), lines changed, removed | `the_match_page_shows_only_with_lines` |
    //! | Credits page | its lines, ignoring the settings keys | `the_credits_page_lists_the_assets` |
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
        menu.handle(Control::Key(Scancode::E), &mut mine);
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
        assert_eq!(menu.get_page(), Page::Scores);
        for expected in [
            Page::Settings,
            Page::Map,
            Page::Credits,
            Page::Quit,
            Page::Scores,
        ] {
            menu.handle(Control::Key(Scancode::E), &mut mine);
            assert_eq!(menu.get_page(), expected);
        }
        for expected in [Page::Quit, Page::Credits, Page::Map] {
            menu.handle(Control::Key(Scancode::Q), &mut mine);
            assert_eq!(menu.get_page(), expected);
        }
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
    fn the_credits_page_lists_the_assets() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        assert_eq!(menu.get_page(), Page::Credits);
        menu.handle(Control::Key(Scancode::Down), &mut mine);
        menu.handle(Control::Key(Scancode::Return), &mut mine);
        assert!(!menu.is_waiting_for_control && menu.row == 0);
        let lines: Vec<(String, bool)> = menu.get_lines(&mine, "", Scoreboard::NoServer);
        assert!(lines[0].0.contains("[CREDITS]"));
        let texts: Vec<&str> = lines[2..].iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(texts, CREDITS_LINES);
        assert!(lines[2..].iter().all(|(_, is_highlighted)| !is_highlighted));
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
            MenuOutcome::Stay
        );
        mine.keys
            .set_controls(Action::OpenMenu, vec![Control::Key(Scancode::M)])
            .expect("a key for the menu");
        assert_eq!(
            menu.handle(Control::Key(Scancode::M), &mut mine),
            MenuOutcome::Close
        );
    }

    #[test]
    fn the_quit_page_leaves_the_level() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        assert_eq!(
            menu.handle(Control::Key(Scancode::Return), &mut mine),
            MenuOutcome::Stay
        );
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        assert_eq!(menu.get_page(), Page::Quit);
        let lines: Vec<(String, bool)> = menu.get_lines(&mine, "", Scoreboard::NoServer);
        assert!(lines[0].0.contains("[QUIT]"));
        let texts: Vec<&str> = lines[2..].iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(texts, QUIT_LINES);
        assert_eq!(
            menu.handle(Control::Key(Scancode::N), &mut mine),
            MenuOutcome::Stay
        );
        for key in [Scancode::Return, Scancode::Y] {
            assert_eq!(
                menu.handle(Control::Key(key), &mut mine),
                MenuOutcome::LeaveLevel,
                "{key:?}"
            );
        }
    }

    #[test]
    fn the_held_map_names_its_key() {
        let mine: PerPlayerSettings = PerPlayerSettings::Builtin();
        let lines: Vec<(String, bool)> = get_held_map_lines(&mine.keys);
        assert!(lines[0].0.contains("Tab") && lines[0].1);
        assert_eq!(lines[1], (MAP_LEGEND.to_string(), false));
    }

    #[test]
    fn the_scores_page_sorts_and_totals() {
        let (menu, mine) = (Menu::New(), PerPlayerSettings::Builtin());
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

    #[test]
    fn the_match_page_shows_only_with_lines() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        menu.handle(Control::Key(Scancode::E), &mut mine);
        assert_eq!(menu.get_page(), Page::Settings);
        let status = |round: u32| vec![(format!("ROUND {round} of 5"), true)];
        menu.set_match_lines(Some(status(1)));
        assert_eq!(menu.get_page(), Page::Match);
        let lines: Vec<(String, bool)> = menu.get_lines(&mine, "", Scoreboard::NoServer);
        assert!(lines[0].0.starts_with("[MATCH]"), "{}", lines[0].0);
        assert_eq!(lines[2..], status(1));
        menu.set_match_lines(Some(status(2)));
        assert_eq!(
            menu.get_lines(&mine, "", Scoreboard::NoServer)[2..],
            status(2)
        );
        menu.handle(Control::Key(Scancode::Q), &mut mine);
        assert_eq!(menu.get_page(), Page::Quit);
        menu.handle(Control::Key(Scancode::E), &mut mine);
        menu.handle(Control::Key(Scancode::E), &mut mine);
        assert_eq!(menu.get_page(), Page::Scores);
        menu.set_match_lines(None);
        assert_eq!(menu.get_page(), Page::Scores);
        assert!(
            menu.get_lines(&mine, "", Scoreboard::NoServer)[0]
                .0
                .starts_with("MENU  [SCORES]")
        );
    }

    #[test]
    fn visuals_rows_step_within_their_ranges() {
        let (mut menu, mut mine) = (Menu::New(), PerPlayerSettings::Builtin());
        menu.handle(Control::Key(Scancode::E), &mut mine);
        for (knob, _, step, low, high) in VISUALS_ROWS {
            let row: Row = get_rows()
                .into_iter()
                .find(|row| matches!(row, Row::Number(Number::Visual(k), ..) if *k == knob))
                .expect("a visuals row");
            move_to(&mut menu, &mut mine, row);
            let press =
                |menu: &mut Menu, mine: &mut PerPlayerSettings, key: Scancode, count: f64| {
                    for _ in 0..count.round() as usize {
                        menu.handle(Control::Key(key), mine);
                    }
                };
            press(
                &mut menu,
                &mut mine,
                Scancode::Right,
                (high - low) / step + 2.0,
            );
            assert_eq!(mine.visuals.get(knob), high, "{knob:?}");
            press(
                &mut menu,
                &mut mine,
                Scancode::Left,
                (high - low) / step + 2.0,
            );
            assert_eq!(mine.visuals.get(knob), low, "{knob:?}");
            press(&mut menu, &mut mine, Scancode::Right, (1.0 - low) / step);
            assert_eq!(
                mine.visuals.to_gpu(),
                warped_shared::visuals::GpuPlayerVisuals::IDENTITY,
                "{knob:?}"
            );
        }
    }
}
