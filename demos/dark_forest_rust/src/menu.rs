//! `menu` is the in-game menu, in pages under a tab bar (ported from the hyperbolic demo's
//! `menu.rs`): the scores page, with each player's team, kills and deaths from the server; the
//! settings page, where a player changes the look speed, render scale, vsync, frame-time readout,
//! the picture's brightness, contrast, saturation and gamma (W-51, basic visuals) and minimap for
//! this run and sees the controls; the map page, whose picture the window draws
//! (`minimap.rs`); the credits page (`credits.rs`); the quit page, which leaves the level; and,
//! when the caller supplies its lines (the one-window app's match status, `set_match_lines`), a
//! MATCH page first.
//! It keeps no window state: the window feeds it keys and the scores, and draws its lines. `Esc`
//! opens and closes it, and the map page's lines also head the map shown while `Tab` is held
//! (W-140, hold Tab for the map, Esc for the menu; `warped_shared::menu_and_map`).
//!
//! Interface: `Menu` (`New`, `handle`, `get_lines`, `get_page`, `set_match_lines`), `MenuOutcome`,
//! `MenuSettings`,
//! `Page`, `Scoreboard`, `MENU_PICTURE_PX`, `get_held_map_lines`.
//!
//! Example: `if menu.handle(Scancode::Escape, &mut menu_settings) == MenuOutcome::Close { ... }`
//!
//! Must know:
//! - The settings last only for this run: the level has no saved player settings yet.

use std::cmp::Reverse;

use sdl3::keyboard::Scancode;
use warped_shared::key_bindings::Control;
use warped_shared::menu_and_map::{MATCH_PAGE_NAME, QUIT_LINES, format_tab_bar, is_leave_control};
use warped_shared::visuals::{VisualsKnob, VisualsSettings, get_visuals_row};

use crate::credits::CREDITS_LINES;
use crate::net::{Score, TEAM_NAMES};

/// `MENU_PICTURE_PX` is the size of the menu's text picture: 64 characters by 25 lines.
pub const MENU_PICTURE_PX: (u32, u32) = (528, 312);

/// `PAGE_COUNT` is how many pages the menu has.
pub const PAGE_COUNT: usize = PAGES.len();
const PAGES: [(Page, &str); 5] = [
    (Page::Scores, "SCORES"),
    (Page::Settings, "SETTINGS"),
    (Page::Map, "MAP"),
    (Page::Credits, "CREDITS"),
    (Page::Quit, "QUIT"),
];
const ROWS: [Row; 9] = [
    Row::LookSpeed,
    Row::RenderScale,
    Row::Vsync,
    Row::FrameTime,
    Row::Visual(VisualsKnob::Brightness),
    Row::Visual(VisualsKnob::Contrast),
    Row::Visual(VisualsKnob::Saturation),
    Row::Visual(VisualsKnob::Gamma),
    Row::Minimap,
];
const CONTROLS_LINES: [&str; 6] = [
    "CONTROLS",
    "WASD move, Shift sprint, Ctrl or C crouch, Space jump",
    "Mouse look, left button fire, right aim, R reload",
    "Walk into a rope ladder to climb: W up, S down, Space lets go",
    "Hold E at an enemy console for 3 s to switch it off",
    "Hold Tab for the map; Esc opens and closes this menu",
];
const MAP_LEGEND: &str = "You: white arrow  Robots: team colours  Squares: consoles";
const HELD_MAP_TITLE: &str = "MAP     let go of Tab to hide it";
const WAITING_LINE: &str = "No scores from the server yet: join a team first.";
const LOOK_STEP_RAD_PER_PT: f32 = 0.0002;
const LOOK_RANGE_RAD_PER_PT: (f32, f32) = (0.0004, 0.01);
const RENDER_SCALE_STEP_FRAC: f32 = 0.05;
const RENDER_SCALE_RANGE_FRAC: (f32, f32) = (0.5, 1.0);
const MAX_SCORE_ROW_COUNT: usize = 14;
const TEAM_WIDTH: usize = 6;
const PLAYER_WIDTH: usize = 16;
const KILLS_WIDTH: usize = 6;
const DEATHS_WIDTH: usize = 8;
const LABEL_WIDTH: usize = 24;

/// `Page` is one page of the menu.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Page {
    Scores,
    Settings,
    Map,
    Credits,
    Quit,
    Match,
}

/// `Scoreboard` is what the scores page shows: this player's own kills and deaths when playing
/// alone, that the server hasn't sent scores yet, or every player's latest scores with this
/// player's id (`None` before the server's first welcome).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Scoreboard<'a> {
    Alone {
        kill_count: u32,
        death_count: u32,
    },
    Waiting,
    Scores {
        scores: &'a [Score],
        own_id: Option<u32>,
    },
}

/// `MenuSettings` are the settings the settings page changes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MenuSettings {
    pub look_rad_per_pt: f32,
    pub render_scale_frac: f32,
    pub is_vsync_enabled: bool,
    pub is_showing_frame_time: bool,
    pub is_showing_minimap: bool,
    pub visuals: VisualsSettings,
}

/// `Row` is one line of the settings page that can be chosen.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Row {
    LookSpeed,
    RenderScale,
    Vsync,
    FrameTime,
    Visual(VisualsKnob),
    Minimap,
}

/// `MenuOutcome` is what the window does after a key in the menu: nothing, close it, or leave the
/// level (`menu_and_map::PlayOutcome::LeaveLevel`; the forest run alone quits).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MenuOutcome {
    Stay,
    Close,
    LeaveLevel,
}

/// `get_held_map_lines` gives the lines over the map shown while `Tab` is held: its title and the
/// map's legend.
///
/// Example: `picture.draw(&get_held_map_lines(), false);`
pub fn get_held_map_lines() -> Vec<(String, bool)> {
    vec![
        (HELD_MAP_TITLE.to_string(), true),
        (MAP_LEGEND.to_string(), false),
    ]
}

/// `Menu` is the menu's open page (an index into `get_pages`), the settings row chosen on it, and
/// the MATCH page's lines (`None`: no MATCH page).
pub struct Menu {
    page_index: usize,
    row_index: usize,
    match_lines: Option<Vec<(String, bool)>>,
}

/// `format_score_row` lines up a scores row's columns.
///
/// Example: `let header = format_score_row("TEAM", "PLAYER", "KILLS", "DEATHS");`
fn format_score_row(team: &str, player: &str, kills: &str, deaths: &str) -> String {
    format!(
        "{team:<TEAM_WIDTH$}{player:<PLAYER_WIDTH$}{kills:>KILLS_WIDTH$}{deaths:>DEATHS_WIDTH$}"
    )
}

/// `get_score_lines` gives the scores page's lines below the tab bar: alone, this player's kills
/// and deaths; before the server's scores, why there are none; with them, how many players are
/// on each team, a row per player sorted by team and then by most kills (this player's
/// highlighted), and each team's totals.
///
/// Example: `lines.extend(get_score_lines(Scoreboard::Waiting));`
fn get_score_lines(scoreboard: Scoreboard) -> Vec<(String, bool)> {
    let (scores, own_id) = match scoreboard {
        Scoreboard::Alone {
            kill_count,
            death_count,
        } => {
            return vec![
                ("Playing alone against robots.".to_string(), false),
                (String::new(), false),
                (format_score_row("TEAM", "PLAYER", "KILLS", "DEATHS"), false),
                (
                    format_score_row("", "you", &kill_count.to_string(), &death_count.to_string()),
                    true,
                ),
            ];
        }
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
        lines.push((
            format_score_row(name, "total", &kills.to_string(), &deaths.to_string()),
            false,
        ));
    }
    lines
}

/// `format_switch` shows a yes/no setting.
///
/// Example: `format_switch(true) == "on"`
fn format_switch(is_on: bool) -> &'static str {
    if is_on { "on" } else { "off" }
}

impl Menu {
    /// `New` makes a menu open on the scores page.
    ///
    /// Example: `let menu = Menu::New();`
    #[allow(non_snake_case)]
    pub fn New() -> Menu {
        Menu {
            page_index: 0,
            row_index: 0,
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
    /// most 64 characters wide and 23 lines), or removes the page with `None`. The page opens
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

    /// `get_page` gives the open page.
    ///
    /// Example: `if menu.get_page() == Page::Map { ... }`
    pub fn get_page(&self) -> Page {
        self.get_pages()[self.page_index].0
    }

    /// `handle` acts on one pressed key: Escape closes the menu, Q and E turn the page, on the
    /// quit page Return or Y leaves the level, and on the settings page Up and Down choose a row,
    /// Left and Right change it and Return switches it.
    ///
    /// Example: `let outcome = menu.handle(Scancode::E, &mut menu_settings);`
    pub fn handle(&mut self, key: Scancode, settings: &mut MenuSettings) -> MenuOutcome {
        match key {
            Scancode::Escape => return MenuOutcome::Close,
            Scancode::Q => {
                let page_count: usize = self.get_pages().len();
                self.page_index = (self.page_index + page_count - 1) % page_count;
                return MenuOutcome::Stay;
            }
            Scancode::E => {
                self.page_index = (self.page_index + 1) % self.get_pages().len();
                return MenuOutcome::Stay;
            }
            _ if self.get_page() == Page::Quit && is_leave_control(Control::Key(key)) => {
                return MenuOutcome::LeaveLevel;
            }
            _ if self.get_page() != Page::Settings => return MenuOutcome::Stay,
            _ => {}
        }
        let direction: f32 = match key {
            Scancode::Left => -1.0,
            Scancode::Right | Scancode::Return => 1.0,
            Scancode::Up => {
                self.row_index = (self.row_index + ROWS.len() - 1) % ROWS.len();
                return MenuOutcome::Stay;
            }
            Scancode::Down => {
                self.row_index = (self.row_index + 1) % ROWS.len();
                return MenuOutcome::Stay;
            }
            _ => return MenuOutcome::Stay,
        };
        match ROWS[self.row_index] {
            Row::LookSpeed => {
                settings.look_rad_per_pt = (settings.look_rad_per_pt
                    + direction * LOOK_STEP_RAD_PER_PT)
                    .clamp(LOOK_RANGE_RAD_PER_PT.0, LOOK_RANGE_RAD_PER_PT.1);
            }
            Row::RenderScale => {
                settings.render_scale_frac = (settings.render_scale_frac
                    + direction * RENDER_SCALE_STEP_FRAC)
                    .clamp(RENDER_SCALE_RANGE_FRAC.0, RENDER_SCALE_RANGE_FRAC.1);
            }
            Row::Vsync => settings.is_vsync_enabled = !settings.is_vsync_enabled,
            Row::FrameTime => settings.is_showing_frame_time = !settings.is_showing_frame_time,
            Row::Visual(knob) => settings.visuals.step(knob, f64::from(direction)),
            Row::Minimap => settings.is_showing_minimap = !settings.is_showing_minimap,
        }
        MenuOutcome::Stay
    }

    /// `get_lines` gives the open page's lines, each with whether it is highlighted, under the
    /// tab bar.
    ///
    /// Example: `picture.draw(&menu.get_lines(&menu_settings, Scoreboard::Waiting), false);`
    pub fn get_lines(
        &self,
        settings: &MenuSettings,
        scoreboard: Scoreboard,
    ) -> Vec<(String, bool)> {
        let page_names: Vec<&str> = self.get_pages().iter().map(|(_, name)| *name).collect();
        let mut lines: Vec<(String, bool)> = vec![
            (format_tab_bar(&page_names, self.page_index), true),
            (String::new(), false),
        ];
        match self.get_page() {
            Page::Match => lines.extend(self.match_lines.iter().flatten().cloned()),
            Page::Scores => lines.extend(get_score_lines(scoreboard)),
            Page::Map => lines.push((MAP_LEGEND.to_string(), false)),
            Page::Credits => {
                lines.extend(CREDITS_LINES.iter().map(|line| (line.to_string(), false)));
            }
            Page::Quit => lines.extend(QUIT_LINES.iter().map(|line| (line.to_string(), false))),
            Page::Settings => {
                lines.push((
                    "Up/Down choose, Left/Right change, Return switches".to_string(),
                    false,
                ));
                lines.push((String::new(), false));
                for (i, row) in ROWS.iter().enumerate() {
                    let text: String = match row {
                        Row::LookSpeed => format!(
                            "{:<LABEL_WIDTH$}< {:.4} >",
                            "mouse look rad/pt", settings.look_rad_per_pt
                        ),
                        Row::RenderScale => format!(
                            "{:<LABEL_WIDTH$}< {:.2} >",
                            "render scale", settings.render_scale_frac
                        ),
                        Row::Vsync => format!(
                            "{:<LABEL_WIDTH$}{}",
                            "vsync",
                            format_switch(settings.is_vsync_enabled)
                        ),
                        Row::FrameTime => format!(
                            "{:<LABEL_WIDTH$}{}",
                            "frame time",
                            format_switch(settings.is_showing_frame_time)
                        ),
                        Row::Visual(knob) => format!(
                            "{:<LABEL_WIDTH$}< {:.2} >",
                            get_visuals_row(*knob).1,
                            settings.visuals.get(*knob)
                        ),
                        Row::Minimap => format!(
                            "{:<LABEL_WIDTH$}{}",
                            "minimap",
                            format_switch(settings.is_showing_minimap)
                        ),
                    };
                    lines.push((text, i == self.row_index));
                }
                lines.push((String::new(), false));
                lines.extend(CONTROLS_LINES.iter().map(|line| (line.to_string(), false)));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the menu's pages and keys.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Page turning | forward past the last, back past the first; settings keys ignored off the settings page | `q_and_e_turn_pages` |
    //! | Settings row | a number (stepped, clamped), a switch | `settings_rows_change` |
    //! | Visuals row (W-51, basic visuals) | stepped up to its highest, down to its lowest, back to 1 | `visuals_rows_step_within_their_ranges` |
    //! | Closing | Escape; not Tab (the held map's key) | `escape_closes_it_and_tab_does_not` |
    //! | Quit page | Return or Y (leave the level), another key (stay); Return elsewhere (no leaving) | `the_quit_page_leaves_the_level` |
    //! | Scoreboard | alone, waiting, scores (two teams, ties in kills, this player known or not) | `the_scores_page_sorts_and_totals` |
    //! | MATCH page | none (the forest run alone), lines supplied (shown first, in the page turning, its lines), lines changed, removed | `the_match_page_shows_only_with_lines` |
    //! | Fit | every page's lines within the picture's 64 characters and 25 lines | `every_page_fits_the_picture` |

    use super::*;

    const SETTINGS: MenuSettings = MenuSettings {
        look_rad_per_pt: 0.0022,
        render_scale_frac: 1.0,
        is_vsync_enabled: true,
        is_showing_frame_time: false,
        is_showing_minimap: true,
        visuals: VisualsSettings::IDENTITY,
    };

    #[test]
    fn q_and_e_turn_pages() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        assert_eq!(menu.get_page(), Page::Scores);
        for expected in [
            Page::Settings,
            Page::Map,
            Page::Credits,
            Page::Quit,
            Page::Scores,
        ] {
            menu.handle(Scancode::E, &mut settings);
            assert_eq!(menu.get_page(), expected);
        }
        menu.handle(Scancode::Q, &mut settings);
        menu.handle(Scancode::Q, &mut settings);
        assert_eq!(menu.get_page(), Page::Credits);
        menu.handle(Scancode::Down, &mut settings);
        menu.handle(Scancode::Right, &mut settings);
        assert_eq!((menu.row_index, settings), (0, SETTINGS));
    }

    #[test]
    fn settings_rows_change() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        menu.handle(Scancode::E, &mut settings);
        menu.handle(Scancode::Down, &mut settings);
        for _ in 0..20 {
            menu.handle(Scancode::Left, &mut settings);
        }
        assert_eq!(settings.render_scale_frac, RENDER_SCALE_RANGE_FRAC.0);
        menu.handle(Scancode::Down, &mut settings);
        menu.handle(Scancode::Return, &mut settings);
        assert!(!settings.is_vsync_enabled);
        menu.handle(Scancode::Up, &mut settings);
        menu.handle(Scancode::Up, &mut settings);
        menu.handle(Scancode::Up, &mut settings);
        assert_eq!(
            menu.handle(Scancode::Return, &mut settings),
            MenuOutcome::Stay
        );
        assert!(!settings.is_showing_minimap);
    }

    #[test]
    fn visuals_rows_step_within_their_ranges() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        menu.handle(Scancode::E, &mut settings);
        for (i, row) in ROWS.iter().enumerate() {
            let Row::Visual(knob) = *row else { continue };
            while menu.row_index != i {
                menu.handle(Scancode::Down, &mut settings);
            }
            let (_, label, step, low, high) = get_visuals_row(knob);
            let step_count: usize = ((high - low) / step).round() as usize + 2;
            for _ in 0..step_count {
                menu.handle(Scancode::Right, &mut settings);
            }
            assert_eq!(settings.visuals.get(knob), high, "{knob:?}");
            for _ in 0..step_count {
                menu.handle(Scancode::Left, &mut settings);
            }
            assert_eq!(settings.visuals.get(knob), low, "{knob:?}");
            for _ in 0..((1.0 - low) / step).round() as usize {
                menu.handle(Scancode::Right, &mut settings);
            }
            assert_eq!(
                settings.visuals.to_gpu(),
                VisualsSettings::IDENTITY.to_gpu()
            );
            let lines: Vec<(String, bool)> = menu.get_lines(&settings, Scoreboard::Waiting);
            assert!(
                lines
                    .iter()
                    .any(|(text, is_chosen)| *is_chosen && text.starts_with(label))
            );
        }
    }

    #[test]
    fn the_quit_page_leaves_the_level() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        menu.handle(Scancode::Q, &mut settings);
        assert_eq!(menu.get_page(), Page::Quit);
        let lines: Vec<(String, bool)> = menu.get_lines(&settings, Scoreboard::Waiting);
        let texts: Vec<&str> = lines[2..].iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(texts, QUIT_LINES);
        assert_eq!(menu.handle(Scancode::N, &mut settings), MenuOutcome::Stay);
        for key in [Scancode::Return, Scancode::Y] {
            assert_eq!(
                menu.handle(key, &mut settings),
                MenuOutcome::LeaveLevel,
                "{key:?}"
            );
        }
        let mut scores_menu: Menu = Menu::New();
        assert_eq!(
            scores_menu.handle(Scancode::Return, &mut settings),
            MenuOutcome::Stay
        );
    }

    #[test]
    fn escape_closes_it_and_tab_does_not() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        assert_eq!(
            menu.handle(Scancode::Escape, &mut settings),
            MenuOutcome::Close
        );
        assert_eq!(menu.handle(Scancode::Tab, &mut settings), MenuOutcome::Stay);
    }

    #[test]
    fn the_scores_page_sorts_and_totals() {
        let menu: Menu = Menu::New();
        let get_texts = |scoreboard: Scoreboard| -> Vec<(String, bool)> {
            menu.get_lines(&SETTINGS, scoreboard)[2..].to_vec()
        };
        let alone: Vec<(String, bool)> = get_texts(Scoreboard::Alone {
            kill_count: 3,
            death_count: 1,
        });
        assert_eq!(alone[3], (format_score_row("", "you", "3", "1"), true));
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
        let own: Vec<(String, bool)> = get_texts(Scoreboard::Scores {
            scores: &scores,
            own_id: Some(5),
        });
        assert_eq!(own, expected);
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
    fn every_page_fits_the_picture() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        let scores: Vec<Score> = (0..20)
            .map(|id| Score {
                id,
                team: id % 2,
                kills: id,
                deaths: 0,
            })
            .collect();
        for _ in PAGES {
            let lines: Vec<(String, bool)> = menu.get_lines(
                &settings,
                Scoreboard::Scores {
                    scores: &scores,
                    own_id: None,
                },
            );
            assert!(
                lines.len() <= 25,
                "{:?} has {} lines",
                menu.get_page(),
                lines.len()
            );
            for (line, _) in &lines {
                assert!(line.chars().count() <= 64, "{line:?} is too wide");
            }
            menu.handle(Scancode::E, &mut settings);
        }
        for (line, _) in get_held_map_lines() {
            assert!(line.chars().count() <= 64, "{line:?} is too wide");
        }
    }

    #[test]
    fn the_match_page_shows_only_with_lines() {
        let (mut menu, mut settings) = (Menu::New(), SETTINGS);
        menu.handle(Scancode::E, &mut settings);
        assert_eq!(menu.get_page(), Page::Settings);
        let status = |round: u32| vec![(format!("ROUND {round} of 5"), true)];
        menu.set_match_lines(Some(status(1)));
        assert_eq!(menu.get_page(), Page::Match);
        let lines: Vec<(String, bool)> = menu.get_lines(&settings, Scoreboard::Waiting);
        assert!(lines[0].0.starts_with("[MATCH]"), "{}", lines[0].0);
        assert!(lines[0].0.chars().count() <= 64, "{}", lines[0].0);
        assert_eq!(lines[2..], status(1));
        menu.set_match_lines(Some(status(2)));
        assert_eq!(
            menu.get_lines(&settings, Scoreboard::Waiting)[2..],
            status(2)
        );
        menu.handle(Scancode::Q, &mut settings);
        assert_eq!(menu.get_page(), Page::Quit);
        menu.handle(Scancode::E, &mut settings);
        menu.handle(Scancode::E, &mut settings);
        assert_eq!(menu.get_page(), Page::Scores);
        menu.set_match_lines(None);
        assert_eq!(menu.get_page(), Page::Scores);
        assert!(
            menu.get_lines(&settings, Scoreboard::Waiting)[0]
                .0
                .starts_with("MENU  [SCORES]")
        );
    }
}
