//! `main_menu.rs` is the app's first screen: the games heard on the network (from the match
//! servers' beacons) to join, joining one by address, starting a game (the five-level match or
//! any one level of the manifest, hosted for others, alone, or against bots), the player's
//! settings, the credits, and quitting. It keeps no window state: the window feeds it inputs and
//! draws its shapes.
//!
//! Interface: `MainMenu` (`New`, `get_rows`, `handle`, `get_choice`, `get_shapes`), `MainRow`,
//! `MainAction`.
//!
//! Example: `let action = main_menu.handle(MenuInput::Activate, &rows, levels.len());`

use std::net::SocketAddr;

use warped_shared::match_protocol::{FoundMatch, LevelEntry};
use warped_shared::page::Shape;

use crate::children::{BotObjective, GameKind, MAX_ALLY_COUNT, MAX_BOT_COUNT, MAX_ENEMY_COUNT};
use crate::menu_page::{
    LINE, MARGIN, MenuInput, MenuRow, move_selection, push_heading, push_rows, push_title,
};

const PAGE_SIZE: [f32; 2] = [640.0, 455.0];
const ROWS_TOP: f32 = 44.0;
const HELP: &str = "Up/Down: choose   Left/Right: change   Return: go   Escape: back";

/// `MainRow` is one row of the main menu.
#[derive(Clone, PartialEq, Debug)]
pub enum MainRow {
    Join(FoundMatch),
    JoinByAddress,
    GameChoice,
    Host,
    Alone,
    Allies,
    Enemies,
    BotObjective,
    StartWithBots,
    BackToGame,
    StopHosting,
    PlayerSettings,
    Credits,
    Quit,
}

/// `MainAction` is what the app does after an input on the main menu: nothing, join the match
/// server at an address, open a page, start a game (its kind, and `None` for the match or a
/// level's index for one level), go back to the game this app hosts, stop hosting it, or quit.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MainAction {
    Stay,
    Join(SocketAddr),
    OpenJoinByAddress,
    Start {
        kind: GameKind,
        choice: Option<usize>,
    },
    BackToGame,
    StopHosting,
    OpenPlayerSettings,
    OpenCredits,
    Quit,
}

/// `MainMenu` is the main menu's state: the selected row, the game chosen (0 for the match, else
/// a level's index plus 1), the bots on the player's team and on the other (allies, enemies),
/// what the bots do, and a message.
pub struct MainMenu {
    selected: usize,
    game_choice: usize,
    bot_counts: [u32; 2],
    bot_objective: BotObjective,
    pub message: Option<String>,
}

impl MainMenu {
    /// `New` starts with the first row selected, the match chosen, and one enemy console bot.
    ///
    /// Example: `let main_menu = MainMenu::New();`
    #[allow(non_snake_case)]
    pub fn New() -> MainMenu {
        MainMenu {
            selected: 0,
            game_choice: 0,
            bot_counts: [0, 1],
            bot_objective: BotObjective::Consoles,
            message: None,
        }
    }

    /// `get_choice` gives the game chosen: `None` for the five-level match, else a level's index.
    ///
    /// Example: `let choice = main_menu.get_choice();`
    pub fn get_choice(&self) -> Option<usize> {
        self.game_choice.checked_sub(1)
    }

    /// `get_rows` gives the rows: each game in `found`, then joining by address, the game to
    /// start and how, the game this app hosts (when `is_hosting`), settings, credits and quit.
    ///
    /// Example: `let rows = main_menu.get_rows(listener.get_found(), hosted.is_some());`
    pub fn get_rows(&self, found: &[FoundMatch], is_hosting: bool) -> Vec<MainRow> {
        let mut rows: Vec<MainRow> = found.iter().copied().map(MainRow::Join).collect();
        rows.extend([
            MainRow::JoinByAddress,
            MainRow::GameChoice,
            MainRow::Host,
            MainRow::Alone,
            MainRow::Allies,
            MainRow::Enemies,
            MainRow::BotObjective,
            MainRow::StartWithBots,
        ]);
        if is_hosting {
            rows.extend([MainRow::BackToGame, MainRow::StopHosting]);
        }
        rows.extend([MainRow::PlayerSettings, MainRow::Credits, MainRow::Quit]);
        rows
    }

    /// `handle` applies one input to the menu with `rows` shown and `level_count` levels in the
    /// manifest, and says what the app does.
    ///
    /// Example: `let action = main_menu.handle(MenuInput::Down, &rows, 5);`
    pub fn handle(&mut self, input: MenuInput, rows: &[MainRow], level_count: usize) -> MainAction {
        self.message = None;
        self.selected = move_selection(self.selected, &input, rows.len());
        let row: &MainRow = &rows[self.selected];
        let step: i64 = match input {
            MenuInput::Left => -1,
            MenuInput::Right => 1,
            _ => 0,
        };
        if step != 0 {
            match row {
                MainRow::GameChoice => {
                    let count: i64 = level_count as i64 + 1;
                    self.game_choice = (self.game_choice as i64 + step).rem_euclid(count) as usize;
                }
                MainRow::Allies | MainRow::Enemies => {
                    let (side, max): (usize, u32) = match row {
                        MainRow::Allies => (0, MAX_ALLY_COUNT),
                        _ => (1, MAX_ENEMY_COUNT),
                    };
                    let room: u32 = MAX_BOT_COUNT - self.bot_counts[1 - side];
                    let count: &mut u32 = &mut self.bot_counts[side];
                    *count = (i64::from(*count) + step).clamp(0, i64::from(max.min(room))) as u32;
                }
                MainRow::BotObjective => {
                    self.bot_objective = match self.bot_objective {
                        BotObjective::Consoles => BotObjective::Fight,
                        BotObjective::Fight => BotObjective::Consoles,
                    };
                }
                _ => {}
            }
            return MainAction::Stay;
        }
        if input != MenuInput::Activate {
            return MainAction::Stay;
        }
        let choice: Option<usize> = self.get_choice();
        match row {
            MainRow::Join(found) => MainAction::Join(found.server),
            MainRow::JoinByAddress => MainAction::OpenJoinByAddress,
            MainRow::Host => MainAction::Start {
                kind: GameKind::Host,
                choice,
            },
            MainRow::Alone => MainAction::Start {
                kind: GameKind::Alone,
                choice,
            },
            MainRow::StartWithBots | MainRow::Allies | MainRow::Enemies | MainRow::BotObjective => {
                MainAction::Start {
                    kind: GameKind::Bots {
                        ally_count: self.bot_counts[0],
                        enemy_count: self.bot_counts[1],
                        objective: self.bot_objective,
                    },
                    choice,
                }
            }
            MainRow::GameChoice => MainAction::Stay,
            MainRow::BackToGame => MainAction::BackToGame,
            MainRow::StopHosting => MainAction::StopHosting,
            MainRow::PlayerSettings => MainAction::OpenPlayerSettings,
            MainRow::Credits => MainAction::OpenCredits,
            MainRow::Quit => MainAction::Quit,
        }
    }

    /// `format_row` gives a row's text, naming the manifest's `levels`.
    ///
    /// Example: `let text = main_menu.format_row(&MainRow::GameChoice, &levels);`
    fn format_row(&self, row: &MainRow, levels: &[LevelEntry]) -> String {
        match row {
            MainRow::Join(found) => format!("Join {}   {}", found.server, found.beacon.describe()),
            MainRow::JoinByAddress => "Join a game by its address...".to_string(),
            MainRow::GameChoice => match self.get_choice() {
                None => format!("Game:  < The match: all {} levels >", levels.len()),
                Some(level) => format!("Game:  < One level: {} >", levels[level].name),
            },
            MainRow::Host => "  Host it for others on this network".to_string(),
            MainRow::Alone => "  Play it alone".to_string(),
            MainRow::Allies => format!("    Bots on your team:  < {} >", self.bot_counts[0]),
            MainRow::Enemies => format!("    Bots on the other team:  < {} >", self.bot_counts[1]),
            MainRow::BotObjective => format!(
                "    Bots:  < {} >",
                match self.bot_objective {
                    BotObjective::Consoles => "turn off consoles",
                    BotObjective::Fight => "fight",
                }
            ),
            MainRow::StartWithBots => "  Play it against bots".to_string(),
            MainRow::BackToGame => "Back to the game you host".to_string(),
            MainRow::StopHosting => "Stop hosting (ends it for everyone in it)".to_string(),
            MainRow::PlayerSettings => "Player settings (keys, mouse, look)".to_string(),
            MainRow::Credits => "Credits".to_string(),
            MainRow::Quit => "Quit".to_string(),
        }
    }

    /// `get_shapes` gives the main menu's page: the title, the games on the network (or why
    /// none shows: `listening`'s message), the rows, and the message.
    ///
    /// Example: `let shapes = main_menu.get_shapes(&rows, &levels, None);`
    pub fn get_shapes(
        &self,
        rows: &[MainRow],
        levels: &[LevelEntry],
        listening_problem: Option<&str>,
    ) -> (Vec<Shape>, [f32; 2]) {
        let mut shapes: Vec<Shape> = Vec::new();
        push_title(&mut shapes, PAGE_SIZE, "WARPED", HELP);
        push_heading(
            &mut shapes,
            [MARGIN, ROWS_TOP - LINE],
            "GAMES ON THIS NETWORK",
        );
        let mut lines: Vec<Option<MenuRow>> = Vec::new();
        if !rows.iter().any(|row| matches!(row, MainRow::Join(_))) {
            let note: String = listening_problem
                .map_or("(none found yet; looking...)".to_string(), str::to_string);
            lines.push(None);
            push_heading(&mut shapes, [MARGIN, ROWS_TOP], &note);
        }
        for row in rows {
            if matches!(row, MainRow::GameChoice | MainRow::PlayerSettings)
                || row == &MainRow::BackToGame
            {
                lines.push(None);
            }
            lines.push(Some(MenuRow {
                text: self.format_row(row, levels),
                is_enabled: true,
            }));
        }
        push_rows(
            &mut shapes,
            ROWS_TOP,
            &lines,
            Some(self.selected.min(rows.len() - 1)),
        );
        if let Some(message) = &self.message {
            push_heading(&mut shapes, [MARGIN, PAGE_SIZE[1] - 3.0 * LINE], message);
        }
        (shapes, PAGE_SIZE)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `main_menu.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Game choice | the match, every level of the shipped manifest, wrapping both ways | `every_manifest_level_can_be_chosen` |
    //! | Start | hosted, alone, with bots (allies and enemies clamped to their maxima and to 7 together, objective switched) | `games_start_with_their_kind_and_bots` |
    //! | Rows | found games first, hosting rows only while hosting | `rows_list_found_games_and_hosting` |
    //! | Other rows | join a found game, by address, back to the game, stop, settings, credits, quit | `rows_lead_to_their_actions` |

    use super::*;
    use crate::levels_manifest::{get_levels_manifest_path, load_levels};
    use std::time::Instant;
    use warped_shared::match_protocol::{BeaconPhase, MatchBeacon};

    /// `select` moves `menu` to the first row equal to `target`.
    ///
    /// Example: `select(&mut menu, &rows, &MainRow::Host);`
    fn select(menu: &mut MainMenu, rows: &[MainRow], target: &MainRow) {
        menu.selected = rows.iter().position(|row| row == target).unwrap();
    }

    /// `make_found` gives a match heard from `server`.
    ///
    /// Example: `let found = make_found("10.0.0.2:48304");`
    fn make_found(server: &str) -> FoundMatch {
        FoundMatch {
            server: server.parse().unwrap(),
            beacon: MatchBeacon {
                match_port: 48_304,
                team_counts: [1, 0],
                phase: BeaconPhase::WaitingForTeams,
                round: 0,
            },
            heard_at: Instant::now(),
        }
    }

    #[test]
    fn every_manifest_level_can_be_chosen() {
        let levels: Vec<LevelEntry> = load_levels(&get_levels_manifest_path()).unwrap();
        let mut menu: MainMenu = MainMenu::New();
        let rows: Vec<MainRow> = menu.get_rows(&[], false);
        select(&mut menu, &rows, &MainRow::GameChoice);
        assert_eq!(menu.get_choice(), None);
        for level in 0..levels.len() {
            menu.handle(MenuInput::Right, &rows, levels.len());
            assert_eq!(menu.get_choice(), Some(level));
            assert!(
                menu.format_row(&MainRow::GameChoice, &levels)
                    .contains(&levels[level].name)
            );
            select(&mut menu, &rows, &MainRow::Alone);
            let action: MainAction = menu.handle(MenuInput::Activate, &rows, levels.len());
            assert_eq!(
                action,
                MainAction::Start {
                    kind: GameKind::Alone,
                    choice: Some(level)
                }
            );
            select(&mut menu, &rows, &MainRow::GameChoice);
        }
        menu.handle(MenuInput::Right, &rows, levels.len());
        assert_eq!(menu.get_choice(), None);
        menu.handle(MenuInput::Left, &rows, levels.len());
        assert_eq!(menu.get_choice(), Some(levels.len() - 1));
    }

    #[test]
    fn games_start_with_their_kind_and_bots() {
        let mut menu: MainMenu = MainMenu::New();
        let rows: Vec<MainRow> = menu.get_rows(&[], false);
        select(&mut menu, &rows, &MainRow::Host);
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, 5),
            MainAction::Start {
                kind: GameKind::Host,
                choice: None
            }
        );
        select(&mut menu, &rows, &MainRow::Allies);
        for _ in 0..9 {
            menu.handle(MenuInput::Right, &rows, 5);
        }
        select(&mut menu, &rows, &MainRow::Enemies);
        for _ in 0..9 {
            menu.handle(MenuInput::Right, &rows, 5);
        }
        select(&mut menu, &rows, &MainRow::BotObjective);
        menu.handle(MenuInput::Right, &rows, 5);
        select(&mut menu, &rows, &MainRow::StartWithBots);
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, 5),
            MainAction::Start {
                kind: GameKind::Bots {
                    ally_count: MAX_ALLY_COUNT,
                    enemy_count: MAX_BOT_COUNT - MAX_ALLY_COUNT,
                    objective: BotObjective::Fight
                },
                choice: None
            }
        );
        select(&mut menu, &rows, &MainRow::Allies);
        for _ in 0..9 {
            menu.handle(MenuInput::Left, &rows, 5);
        }
        select(&mut menu, &rows, &MainRow::Enemies);
        for _ in 0..9 {
            menu.handle(MenuInput::Right, &rows, 5);
        }
        select(&mut menu, &rows, &MainRow::StartWithBots);
        assert_eq!(
            menu.handle(MenuInput::Activate, &rows, 5),
            MainAction::Start {
                kind: GameKind::Bots {
                    ally_count: 0,
                    enemy_count: MAX_ENEMY_COUNT,
                    objective: BotObjective::Fight
                },
                choice: None
            }
        );
    }

    #[test]
    fn rows_list_found_games_and_hosting() {
        let menu: MainMenu = MainMenu::New();
        let found: [FoundMatch; 2] = [make_found("10.0.0.2:48304"), make_found("10.0.0.3:48304")];
        let rows: Vec<MainRow> = menu.get_rows(&found, false);
        assert_eq!(
            rows[..2],
            [MainRow::Join(found[0]), MainRow::Join(found[1])]
        );
        assert!(!rows.contains(&MainRow::StopHosting));
        let hosting: Vec<MainRow> = menu.get_rows(&[], true);
        assert!(hosting.contains(&MainRow::BackToGame) && hosting.contains(&MainRow::StopHosting));
        assert_eq!(hosting.last(), Some(&MainRow::Quit));
    }

    #[test]
    fn rows_lead_to_their_actions() {
        let mut menu: MainMenu = MainMenu::New();
        let found: FoundMatch = make_found("10.0.0.2:48304");
        let rows: Vec<MainRow> = menu.get_rows(&[found], true);
        for (row, action) in [
            (MainRow::Join(found), MainAction::Join(found.server)),
            (MainRow::JoinByAddress, MainAction::OpenJoinByAddress),
            (MainRow::BackToGame, MainAction::BackToGame),
            (MainRow::StopHosting, MainAction::StopHosting),
            (MainRow::PlayerSettings, MainAction::OpenPlayerSettings),
            (MainRow::Credits, MainAction::OpenCredits),
            (MainRow::Quit, MainAction::Quit),
            (MainRow::GameChoice, MainAction::Stay),
        ] {
            select(&mut menu, &rows, &row);
            assert_eq!(
                menu.handle(MenuInput::Activate, &rows, 5),
                action,
                "{row:?}"
            );
        }
    }
}
