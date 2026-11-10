//! `game_menu.rs` is the game's own screens between the main menu and a level: the game menu
//! (the match's status from the shared progression page, the player's team, switching team,
//! rejoining the round, the game master's choice of what to play, settings, leaving), the
//! in-level menu (Escape in a level: the same match status, resuming, settings, exiting to the
//! game menu, quitting), and the level's stand-in until the levels are libraries (W-141, one
//! window for the whole game). They keep no window state.
//!
//! Interface: `GameMenu` (`New`, `get_rows`, `handle`, `get_shapes`), `GameRow`, `GameAction`,
//! `LevelMenu` (`New`, `handle`, `get_shapes`), `LevelMenuAction`, `get_level_shapes`,
//! `get_round`.
//!
//! Example: `let action = game_menu.handle(MenuInput::Activate, &rows, Some(Team::Blue), 5);`
//!
//! Must know:
//! - Gotchas: the match status is `warped_shared::match_menu`'s page, the one the match window
//!   and the levels' in-level menus show; its `Thumbnail` shapes index the levels' map pictures.

use warped_shared::match_menu::{PAGE_SIZE, TEAM_RGB, get_progression_shapes_with_footer};
use warped_shared::match_protocol::{MatchView, Phase};
use warped_shared::match_state::Team;
use warped_shared::page::{Shape, push_text};

use crate::menu_page::{
    LINE, MARGIN, MenuInput, MenuRow, move_selection, push_heading, push_rows, push_title,
};

const ROWS_PANEL_TOP: f32 = PAGE_SIZE[1] + LINE;
const STATUS_FOOTER: &str = "Escape: back. Each round opens its level in this window.";
const LEVEL_HELP: &str = "Escape: menu   hold Tab: map";
const MAP_BOX_TOP: f32 = 110.0;
const MAP_BOX_HEIGHT: f32 = 300.0;

/// `get_round` gives the round being played in `view`: its number and level index, or `None`
/// between rounds.
///
/// Example: `if let Some((round, level)) = get_round(&view) { ... }`
pub fn get_round(view: &MatchView) -> Option<(usize, usize)> {
    match view.phase {
        Phase::Playing { round, level, .. } => Some((round, level)),
        _ => None,
    }
}

/// `GameRow` is one row of the game menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameRow {
    PickTeam(Team),
    SwitchTeam,
    Rejoin,
    GameMasterChoice,
    GameSettings,
    PlayerSettings,
    Leave,
}

/// `GameAction` is what the app does after an input on the game menu: nothing, play for a team,
/// rejoin the round being played, ask the match server to play `choice` (a level's index, or
/// `None` for the match), open the player's settings, or leave to the main menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameAction {
    Stay,
    SetTeam(Team),
    Rejoin,
    Play(Option<usize>),
    OpenPlayerSettings,
    Leave,
}

/// `GameMenu` is the game menu's state: the selected row, the game master's choice (0 for the
/// match, else a level's index plus 1), and a message.
pub struct GameMenu {
    selected: usize,
    master_choice: usize,
    pub message: Option<String>,
}

/// `get_status_page` gives the match's status (the progression page, or a line while no view has
/// come) with `rows` in a panel under it, and the page's size.
///
/// Example: `let (shapes, size) = get_status_page(Some(&view), Some(Team::Blue), "WARPED", &rows, 0);`
fn get_status_page(
    view: Option<&MatchView>,
    team: Option<Team>,
    waiting_line: &str,
    rows: &[Option<MenuRow>],
    selected: usize,
) -> (Vec<Shape>, [f32; 2]) {
    let size: [f32; 2] = [
        PAGE_SIZE[0],
        ROWS_PANEL_TOP + LINE * (rows.len() as f32 + 3.0),
    ];
    let mut shapes: Vec<Shape> = Vec::new();
    push_title(&mut shapes, size, "", "");
    match view {
        Some(view) => shapes.extend(get_progression_shapes_with_footer(
            view,
            team,
            STATUS_FOOTER,
        )),
        None => push_heading(&mut shapes, [MARGIN, 2.0 * LINE], waiting_line),
    }
    push_rows(&mut shapes, ROWS_PANEL_TOP, rows, Some(selected));
    (shapes, size)
}

impl GameMenu {
    /// `New` starts with the first row selected and the match as the game master's choice.
    ///
    /// Example: `let game_menu = GameMenu::New();`
    #[allow(non_snake_case)]
    pub fn New() -> GameMenu {
        GameMenu {
            selected: 0,
            master_choice: 0,
            message: None,
        }
    }

    /// `get_rows` gives the rows for a player on `team` (`None` before they pick one).
    ///
    /// Example: `let rows = game_menu.get_rows(Some(Team::Blue));`
    pub fn get_rows(&self, team: Option<Team>) -> Vec<GameRow> {
        let mut rows: Vec<GameRow> = match team {
            None => vec![GameRow::PickTeam(Team::Blue), GameRow::PickTeam(Team::Red)],
            Some(_) => vec![GameRow::SwitchTeam, GameRow::Rejoin],
        };
        rows.extend([
            GameRow::GameMasterChoice,
            GameRow::GameSettings,
            GameRow::PlayerSettings,
            GameRow::Leave,
        ]);
        rows
    }

    /// `handle` applies one input with `rows` shown, for a player on `team`, in a match of
    /// `level_count` levels (`view`'s, `None` before it came), and says what the app does.
    ///
    /// Example: `let action = game_menu.handle(MenuInput::Activate, &rows, Some(Team::Red), Some(&view));`
    pub fn handle(
        &mut self,
        input: MenuInput,
        rows: &[GameRow],
        team: Option<Team>,
        view: Option<&MatchView>,
    ) -> GameAction {
        self.message = None;
        self.selected = move_selection(self.selected, &input, rows.len());
        let level_count: usize = view.map_or(0, |view| view.levels.len());
        let row: GameRow = rows[self.selected];
        match (&input, row) {
            (MenuInput::Left | MenuInput::Right, GameRow::GameMasterChoice) if level_count > 0 => {
                let step: i64 = if input == MenuInput::Left { -1 } else { 1 };
                let count: i64 = level_count as i64 + 1;
                self.master_choice = (self.master_choice as i64 + step).rem_euclid(count) as usize;
                GameAction::Stay
            }
            (MenuInput::Back, _) => GameAction::Leave,
            (MenuInput::Activate, GameRow::PickTeam(team)) => GameAction::SetTeam(team),
            (MenuInput::Activate, GameRow::SwitchTeam) => match team {
                Some(team) => GameAction::SetTeam(team.get_other()),
                None => GameAction::Stay,
            },
            (MenuInput::Activate, GameRow::Rejoin) => match view.and_then(get_round) {
                Some(_) => GameAction::Rejoin,
                None => {
                    self.message = Some("No round is being played now.".to_string());
                    GameAction::Stay
                }
            },
            (MenuInput::Activate, GameRow::GameMasterChoice) if level_count > 0 => {
                GameAction::Play(self.master_choice.checked_sub(1))
            }
            (MenuInput::Activate, GameRow::GameSettings) => {
                self.message =
                    Some("The game master's settings come in the next update.".to_string());
                GameAction::Stay
            }
            (MenuInput::Activate, GameRow::PlayerSettings) => GameAction::OpenPlayerSettings,
            (MenuInput::Activate, GameRow::Leave) => GameAction::Leave,
            _ => GameAction::Stay,
        }
    }

    /// `format_row` gives a row's text.
    ///
    /// Example: `let text = game_menu.format_row(GameRow::SwitchTeam, Some(Team::Blue), view);`
    fn format_row(&self, row: GameRow, team: Option<Team>, view: Option<&MatchView>) -> MenuRow {
        let round: Option<(usize, usize)> = view.and_then(get_round);
        let (text, is_enabled): (String, bool) = match row {
            GameRow::PickTeam(team) => (format!("Play for {}", team.get_name()), true),
            GameRow::SwitchTeam => match team {
                Some(team) => (
                    format!(
                        "You play {}: switch to {}",
                        team.get_name(),
                        team.get_other().get_name()
                    ),
                    true,
                ),
                None => ("Pick a team".to_string(), false),
            },
            GameRow::Rejoin => match (round, view) {
                (Some((number, level)), Some(view)) => (
                    format!("Rejoin round {number}: {}", view.levels[level].name),
                    true,
                ),
                _ => ("Rejoin the round (none is being played)".to_string(), false),
            },
            GameRow::GameMasterChoice => match (view, self.master_choice.checked_sub(1)) {
                (None, _) => (
                    "Game master: play... (waiting for the server)".to_string(),
                    false,
                ),
                (Some(view), None) => (
                    format!(
                        "Game master: play  < the match: all {} levels >",
                        view.levels.len()
                    ),
                    true,
                ),
                (Some(view), Some(level)) => (
                    format!(
                        "Game master: play  < one level: {} >",
                        view.levels[level].name
                    ),
                    true,
                ),
            },
            GameRow::GameSettings => (
                "Game settings (game master): next update".to_string(),
                false,
            ),
            GameRow::PlayerSettings => ("Player settings (keys, mouse, look)".to_string(), true),
            GameRow::Leave => ("Leave to the main menu".to_string(), true),
        };
        MenuRow { text, is_enabled }
    }

    /// `get_shapes` gives the game menu's page for a player on `team` at `server_label`: the
    /// match's status, then the rows and the message, and the page's size.
    ///
    /// Example: `let (shapes, size) = game_menu.get_shapes(&rows, Some(Team::Blue), Some(&view), "127.0.0.1:47850");`
    pub fn get_shapes(
        &self,
        rows: &[GameRow],
        team: Option<Team>,
        view: Option<&MatchView>,
        server_label: &str,
    ) -> (Vec<Shape>, [f32; 2]) {
        let mut lines: Vec<Option<MenuRow>> = rows
            .iter()
            .map(|&row| Some(self.format_row(row, team, view)))
            .collect();
        if let Some(message) = &self.message {
            lines.push(None);
            lines.push(Some(MenuRow {
                text: message.clone(),
                is_enabled: false,
            }));
        }
        let waiting: String = format!("Connecting to the game at {server_label}...");
        let selected: usize = self.selected.min(rows.len() - 1);
        get_status_page(view, team, &waiting, &lines, selected)
    }
}

/// `LevelMenuAction` is what the app does after an input on the in-level menu: nothing, resume
/// the level, open the player's settings, exit to the game menu, or quit the app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LevelMenuAction {
    Stay,
    Resume,
    OpenPlayerSettings,
    ExitToGameMenu,
    Quit,
}

const LEVEL_MENU_ROWS: [(LevelMenuAction, &str); 4] = [
    (LevelMenuAction::Resume, "Resume"),
    (
        LevelMenuAction::OpenPlayerSettings,
        "Player settings (keys, mouse, look)",
    ),
    (LevelMenuAction::ExitToGameMenu, "Exit to the game menu"),
    (LevelMenuAction::Quit, "Quit Warped"),
];

/// `LevelMenu` is the in-level menu's state: the selected row.
pub struct LevelMenu {
    selected: usize,
}

impl LevelMenu {
    /// `New` starts on Resume.
    ///
    /// Example: `let level_menu = LevelMenu::New();`
    #[allow(non_snake_case)]
    pub fn New() -> LevelMenu {
        LevelMenu { selected: 0 }
    }

    /// `handle` applies one input and says what the app does; Escape resumes.
    ///
    /// Example: `let action = level_menu.handle(MenuInput::Activate);`
    pub fn handle(&mut self, input: MenuInput) -> LevelMenuAction {
        self.selected = move_selection(self.selected, &input, LEVEL_MENU_ROWS.len());
        match input {
            MenuInput::Back => LevelMenuAction::Resume,
            MenuInput::Activate => LEVEL_MENU_ROWS[self.selected].0,
            _ => LevelMenuAction::Stay,
        }
    }

    /// `get_shapes` gives the in-level menu: the match's status with the menu's rows under it.
    ///
    /// Example: `let (shapes, size) = level_menu.get_shapes(Some(&view), Some(Team::Blue));`
    pub fn get_shapes(
        &self,
        view: Option<&MatchView>,
        team: Option<Team>,
    ) -> (Vec<Shape>, [f32; 2]) {
        let rows: Vec<Option<MenuRow>> = LEVEL_MENU_ROWS
            .iter()
            .map(|(_, text)| {
                Some(MenuRow {
                    text: text.to_string(),
                    is_enabled: true,
                })
            })
            .collect();
        get_status_page(view, team, "Waiting for the match...", &rows, self.selected)
    }
}

/// `get_level_shapes` gives the level's stand-in for a player on `team` in round `round` of
/// `level` (an index into `view.levels`): which level it is, that it plays here once the levels
/// are libraries, and while `is_map_held` (Tab), the level's map picture.
///
/// Example: `let (shapes, size) = get_level_shapes(&view, Team::Blue, (3, 1), false);`
pub fn get_level_shapes(
    view: &MatchView,
    team: Team,
    (round, level): (usize, usize),
    is_map_held: bool,
) -> (Vec<Shape>, [f32; 2]) {
    let mut shapes: Vec<Shape> = Vec::new();
    let name: &str = &view.levels[level].name;
    push_title(
        &mut shapes,
        PAGE_SIZE,
        &format!("{} - round {round}", name.to_uppercase()),
        LEVEL_HELP,
    );
    push_text(
        &mut shapes,
        [MARGIN, 3.0 * LINE],
        format!("You play {}.", team.get_name()),
        TEAM_RGB[team.get_index()],
    );
    match is_map_held {
        true => shapes.push(Shape::Thumbnail {
            level,
            min: [MARGIN, MAP_BOX_TOP],
            size: [PAGE_SIZE[0] - 2.0 * MARGIN, MAP_BOX_HEIGHT],
        }),
        false => {
            for (i, line) in [
                "This level will draw here once the levels are libraries (W-141).",
                "Until then the round goes on without you: the match server",
                "plays it, and this screen follows the match.",
            ]
            .iter()
            .enumerate()
            {
                push_heading(&mut shapes, [MARGIN, 5.0 * LINE + LINE * i as f32], line);
            }
        }
    }
    (shapes, PAGE_SIZE)
}

#[cfg(test)]
mod tests {
    //! Unit tests for `game_menu.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Team | none (pick blue or red), blue (switch to red), red (switch to blue) | `teams_are_picked_and_switched` |
    //! | Rejoin | a round being played, none | `rejoining_needs_a_round` |
    //! | Game master's choice | the match, every level of the view (both ways, wrapping), no view yet | `the_game_master_can_choose_every_level` |
    //! | Other rows and Escape | settings, game settings (not yet), leave, Escape | `the_other_rows_and_escape` |
    //! | In-level menu | resume, settings, exit, quit, Escape resumes | `the_in_level_menu_resumes_exits_or_quits` |
    //! | Pages | status with a view, without one, the level's stand-in with and without the map | `pages_show_the_match_status` |

    use super::*;
    use warped_shared::match_protocol::LevelEntry;
    use warped_shared::match_state::MatchState;

    /// `make_view` gives a view of a new match of the shipped manifest's levels in `phase`.
    ///
    /// Example: `let view = make_view(Phase::WaitingForTeams);`
    fn make_view(phase: Phase) -> MatchView {
        use crate::levels_manifest::{get_levels_manifest_path, load_levels};
        let levels: Vec<LevelEntry> = load_levels(&get_levels_manifest_path()).unwrap();
        MatchView {
            state: MatchState::New(levels.len()),
            phase,
            levels,
            team_counts: [1, 1],
            match_left_s: None,
        }
    }

    /// `select` moves `menu` to `target`'s row.
    ///
    /// Example: `select(&mut menu, &rows, GameRow::Leave);`
    fn select(menu: &mut GameMenu, rows: &[GameRow], target: GameRow) {
        menu.selected = rows.iter().position(|&row| row == target).unwrap();
    }

    #[test]
    fn teams_are_picked_and_switched() {
        let mut menu: GameMenu = GameMenu::New();
        let view: MatchView = make_view(Phase::WaitingForTeams);
        let rows: Vec<GameRow> = menu.get_rows(None);
        for team in [Team::Blue, Team::Red] {
            select(&mut menu, &rows, GameRow::PickTeam(team));
            assert_eq!(
                menu.handle(MenuInput::Activate, &rows, None, Some(&view)),
                GameAction::SetTeam(team)
            );
        }
        for team in [Team::Blue, Team::Red] {
            let rows: Vec<GameRow> = menu.get_rows(Some(team));
            select(&mut menu, &rows, GameRow::SwitchTeam);
            assert_eq!(
                menu.handle(MenuInput::Activate, &rows, Some(team), Some(&view)),
                GameAction::SetTeam(team.get_other())
            );
        }
    }

    #[test]
    fn rejoining_needs_a_round() {
        let mut menu: GameMenu = GameMenu::New();
        let rows: Vec<GameRow> = menu.get_rows(Some(Team::Blue));
        select(&mut menu, &rows, GameRow::Rejoin);
        let playing: MatchView = make_view(Phase::Playing {
            round: 2,
            level: 1,
            port: 1,
        });
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, Some(Team::Blue), Some(&playing)),
            GameAction::Rejoin
        );
        let waiting: MatchView = make_view(Phase::Intermission { left_s: 3.0 });
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, Some(Team::Blue), Some(&waiting)),
            GameAction::Stay
        );
        assert!(menu.message.is_some());
    }

    #[test]
    fn the_game_master_can_choose_every_level() {
        let mut menu: GameMenu = GameMenu::New();
        let view: MatchView = make_view(Phase::WaitingForTeams);
        let rows: Vec<GameRow> = menu.get_rows(Some(Team::Red));
        select(&mut menu, &rows, GameRow::GameMasterChoice);
        let team: Option<Team> = Some(Team::Red);
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, team, Some(&view)),
            GameAction::Play(None)
        );
        for level in 0..view.levels.len() {
            menu.handle(MenuInput::Right, &rows, team, Some(&view));
            assert_eq!(
                menu.handle(MenuInput::Activate, &rows, team, Some(&view)),
                GameAction::Play(Some(level))
            );
            let text: String = menu
                .format_row(GameRow::GameMasterChoice, team, Some(&view))
                .text;
            assert!(text.contains(&view.levels[level].name), "{text}");
        }
        menu.handle(MenuInput::Right, &rows, team, Some(&view));
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, team, Some(&view)),
            GameAction::Play(None)
        );
        menu.handle(MenuInput::Left, &rows, team, Some(&view));
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, team, Some(&view)),
            GameAction::Play(Some(view.levels.len() - 1))
        );
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, team, None),
            GameAction::Stay
        );
    }

    #[test]
    fn the_other_rows_and_escape() {
        let mut menu: GameMenu = GameMenu::New();
        let rows: Vec<GameRow> = menu.get_rows(Some(Team::Blue));
        let team: Option<Team> = Some(Team::Blue);
        for (row, action) in [
            (GameRow::PlayerSettings, GameAction::OpenPlayerSettings),
            (GameRow::GameSettings, GameAction::Stay),
            (GameRow::Leave, GameAction::Leave),
        ] {
            select(&mut menu, &rows, row);
            assert_eq!(menu.handle(MenuInput::Activate, &rows, team, None), action);
        }
        assert_eq!(
            menu.handle(MenuInput::Back, &rows, team, None),
            GameAction::Leave
        );
    }

    #[test]
    fn the_in_level_menu_resumes_exits_or_quits() {
        let mut menu: LevelMenu = LevelMenu::New();
        assert_eq!(menu.handle(MenuInput::Back), LevelMenuAction::Resume);
        assert_eq!(menu.handle(MenuInput::Activate), LevelMenuAction::Resume);
        for action in [
            LevelMenuAction::OpenPlayerSettings,
            LevelMenuAction::ExitToGameMenu,
            LevelMenuAction::Quit,
        ] {
            menu.handle(MenuInput::Down);
            assert_eq!(menu.handle(MenuInput::Activate), action);
        }
    }

    #[test]
    fn pages_show_the_match_status() {
        let menu: GameMenu = GameMenu::New();
        let view: MatchView = make_view(Phase::Playing {
            round: 1,
            level: 2,
            port: 1,
        });
        let rows: Vec<GameRow> = menu.get_rows(Some(Team::Blue));
        let (with_view, size) = menu.get_shapes(&rows, Some(Team::Blue), Some(&view), "x");
        assert!(
            with_view
                .iter()
                .any(|shape| matches!(shape, Shape::Thumbnail { level: 2, .. }))
        );
        assert!(size[1] > PAGE_SIZE[1]);
        let (without_view, _) = menu.get_shapes(&rows, Some(Team::Blue), None, "10.0.0.2:47850");
        assert!(without_view.iter().any(
            |shape| matches!(shape, Shape::Text { text, .. } if text.contains("10.0.0.2:47850"))
        ));
        let (in_level, _) = LevelMenu::New().get_shapes(Some(&view), Some(Team::Blue));
        assert!(
            in_level
                .iter()
                .any(|shape| matches!(shape, Shape::Thumbnail { .. }))
        );
        let (map, _) = get_level_shapes(&view, Team::Blue, (1, 2), true);
        assert!(
            map.iter()
                .any(|shape| matches!(shape, Shape::Thumbnail { level: 2, .. }))
        );
        let (no_map, _) = get_level_shapes(&view, Team::Blue, (1, 2), false);
        assert!(
            !no_map
                .iter()
                .any(|shape| matches!(shape, Shape::Thumbnail { .. }))
        );
    }
}
