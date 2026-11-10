//! `screens.rs` is where the player is in the app and how Escape and the menus move them: a stack
//! of screens with the main menu at the bottom, the game menu above it while in a game, the level
//! above that while a round is played, and pages (settings, credits, typing an address, the
//! in-level menu) on top of whatever opened them.
//!
//! Interface: `Screen`, `Navigator` (`New`, `get_screen`, `open`, `go_back`, `enter_game`,
//! `enter_level`, `leave_level`, `leave_game`, `is_in_level`).
//!
//! Example: `navigator.go_back(); // in a level: opens the in-level menu`
//!
//! Must know:
//! - Gotchas: Escape never quits: in a level it opens the in-level menu, in the in-level menu it
//!   resumes the level, in the game menu it goes to the main menu (the user's rule: "If you exit
//!   from the menu, you go to the game menu. If you exit the game menu, you go to main menu."),
//!   and the main menu stays; quitting is the main menu's or the in-level menu's Quit row.

/// `Screen` is one screen of the app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    MainMenu,
    JoinByAddress,
    PlayerSettings,
    Credits,
    GameMenu,
    Level,
    LevelMenu,
}

/// `Navigator` is the stack of screens, the shown one on top; the main menu is always at the
/// bottom.
#[derive(Clone, PartialEq, Debug)]
pub struct Navigator {
    stack: Vec<Screen>,
}

impl Navigator {
    /// `New` starts on the main menu.
    ///
    /// Example: `let navigator = Navigator::New();`
    #[allow(non_snake_case)]
    pub fn New() -> Navigator {
        Navigator {
            stack: vec![Screen::MainMenu],
        }
    }

    /// `get_screen` gives the screen shown now.
    ///
    /// Example: `if navigator.get_screen() == Screen::Level { ... }`
    pub fn get_screen(&self) -> Screen {
        *self
            .stack
            .last()
            .expect("the main menu is always at the bottom")
    }

    /// `is_in_level` says whether a level is being played (on the stack), even under the
    /// in-level menu or a page over it.
    ///
    /// Example: `if navigator.is_in_level() { ... }`
    pub fn is_in_level(&self) -> bool {
        self.stack.contains(&Screen::Level)
    }

    /// `open` shows a page over the screen shown now (`JoinByAddress`, `PlayerSettings`,
    /// `Credits`, `LevelMenu`); the game menu and the level have their own entries.
    ///
    /// Example: `navigator.open(Screen::PlayerSettings);`
    pub fn open(&mut self, page: Screen) {
        assert!(
            !matches!(page, Screen::MainMenu | Screen::GameMenu | Screen::Level),
            "Navigator::open: {page:?} isn't a page; use enter_game or enter_level"
        );
        self.stack.push(page);
    }

    /// `go_back` does what Escape does: in a level it opens the in-level menu; on the main menu
    /// nothing; elsewhere it closes the screen shown (the in-level menu back to the level, the
    /// game menu back to the main menu, a page back to what opened it).
    ///
    /// Example: `navigator.go_back();`
    pub fn go_back(&mut self) {
        match self.get_screen() {
            Screen::MainMenu => {}
            Screen::Level => self.stack.push(Screen::LevelMenu),
            _ => {
                self.stack.pop();
            }
        }
    }

    /// `enter_game` shows the game menu over the main menu, closing anything else.
    ///
    /// Example: `navigator.enter_game();`
    pub fn enter_game(&mut self) {
        self.stack = vec![Screen::MainMenu, Screen::GameMenu];
    }

    /// `enter_level` shows the level over the game menu, closing anything else.
    ///
    /// Example: `navigator.enter_level();`
    pub fn enter_level(&mut self) {
        self.stack = vec![Screen::MainMenu, Screen::GameMenu, Screen::Level];
    }

    /// `leave_level` goes back to the game menu (the round ended, or the in-level menu's exit).
    ///
    /// Example: `navigator.leave_level();`
    pub fn leave_level(&mut self) {
        self.enter_game();
    }

    /// `leave_game` goes back to the main menu.
    ///
    /// Example: `navigator.leave_game();`
    pub fn leave_game(&mut self) {
        self.stack = vec![Screen::MainMenu];
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `screens.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Escape on each screen | main menu, a page over it, game menu, a page over it, level, in-level menu, a page over that | `escape_follows_the_users_rules` |
    //! | Exits | the in-level menu's exit, a round ending under a page (still in the level until then), leaving the game | `exits_go_to_the_game_menu_then_the_main_menu` |
    //! | Pages | each page opens over its screen; the game menu and level are refused as pages | `pages_open_over_any_screen`, `the_game_menu_is_not_a_page` |

    use super::*;

    #[test]
    fn escape_follows_the_users_rules() {
        let mut navigator: Navigator = Navigator::New();
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::MainMenu);
        navigator.open(Screen::Credits);
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::MainMenu);
        navigator.enter_game();
        navigator.open(Screen::PlayerSettings);
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::GameMenu);
        navigator.enter_level();
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::LevelMenu);
        navigator.open(Screen::PlayerSettings);
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::LevelMenu);
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::Level);
        navigator.go_back();
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::Level);
        navigator.leave_level();
        navigator.go_back();
        assert_eq!(navigator.get_screen(), Screen::MainMenu);
    }

    #[test]
    fn exits_go_to_the_game_menu_then_the_main_menu() {
        let mut navigator: Navigator = Navigator::New();
        navigator.enter_level();
        navigator.go_back();
        navigator.leave_level();
        assert_eq!(navigator.get_screen(), Screen::GameMenu);
        navigator.enter_level();
        navigator.go_back();
        navigator.open(Screen::PlayerSettings);
        assert!(navigator.is_in_level());
        navigator.leave_level();
        assert_eq!(navigator.get_screen(), Screen::GameMenu);
        assert!(!navigator.is_in_level());
        navigator.leave_game();
        assert_eq!(navigator, Navigator::New());
    }

    #[test]
    fn pages_open_over_any_screen() {
        for page in [
            Screen::JoinByAddress,
            Screen::PlayerSettings,
            Screen::Credits,
            Screen::LevelMenu,
        ] {
            let mut navigator: Navigator = Navigator::New();
            navigator.enter_game();
            navigator.open(page);
            assert_eq!(navigator.get_screen(), page);
        }
    }

    #[test]
    #[should_panic(expected = "isn't a page")]
    fn the_game_menu_is_not_a_page() {
        Navigator::New().open(Screen::GameMenu);
    }
}
