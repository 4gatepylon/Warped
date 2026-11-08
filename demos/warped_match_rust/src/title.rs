//! `title.rs` is the title screen a double-clicked app opens on (W-58, W-126): the matches to join
//! (the online matches, or one at an address the player types), the one selected, and the flags
//! that start each. It keeps no window state: the window feeds it keys and draws its lines, as with
//! `menu.rs`.
//!
//! Interface: `TitleScreen` (`New`, `get_lines`, `move_selection`, `get_selected`), `Choice`
//! (`get_flags`, `is_online`, `needs_address`).
//!
//! Example: `let title = TitleScreen::New("34.1.2.3:47850 Warped match")?;`
//!
//! Must know:
//! - The online matches come from the app's `online_games.txt`, one match a line:
//!   `<address>:<port> <name>`, such as `34.1.2.3:47850 Warped match`.
//! - Every choice joins a match server with the game's password; the app plays no game alone.

use std::error::Error;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HEADER_LINES: [&str; 3] = [
    "WARPED",
    "Up and Down choose, Return starts, Escape quits.",
    "",
];

/// `Choice` is one line of the title screen: a match to join, or quitting.
#[derive(Clone, PartialEq, Debug)]
pub enum Choice {
    Online { name: String, address: String },
    Address,
    Quit,
}

/// `TitleScreen` is the title screen's choices and the selected one.
pub struct TitleScreen {
    choices: Vec<Choice>,
    selected: usize,
}

impl Choice {
    /// `get_label` gives the choice's line on the title screen.
    ///
    /// Example: `let line = choice.get_label();`
    fn get_label(&self) -> String {
        match self {
            Choice::Online { name, .. } => format!("Play online: {name}"),
            Choice::Address => "Join a match by its address".to_string(),
            Choice::Quit => "Quit".to_string(),
        }
    }

    /// `get_flags` gives the command-line flags that start the choice's match; a choice that
    /// `needs_address` gets the address the player types after `--match`.
    ///
    /// Example: `let flags = Choice::Address.get_flags();`
    pub fn get_flags(&self) -> Vec<String> {
        match self {
            Choice::Online { address, .. } => vec!["--match".to_string(), address.clone()],
            Choice::Address => vec!["--match".to_string()],
            Choice::Quit => Vec::new(),
        }
    }

    /// `is_online` says whether the choice joins a match server, which needs the game's password.
    ///
    /// Example: `if choice.is_online() { ask_game_password()?; }`
    pub fn is_online(&self) -> bool {
        matches!(self, Choice::Online { .. } | Choice::Address)
    }

    /// `needs_address` says whether the player types the match server's address.
    ///
    /// Example: `if choice.needs_address() { ask_server_address(&path)?; }`
    pub fn needs_address(&self) -> bool {
        matches!(self, Choice::Address)
    }
}

impl TitleScreen {
    /// `New` makes the title screen: the online matches in `online_games` (the app's
    /// `online_games.txt`, possibly empty), then joining one by its address, then quitting.
    ///
    /// Example: `let title = TitleScreen::New("")?;`
    #[allow(non_snake_case)]
    pub fn New(online_games: &str) -> Result<TitleScreen> {
        let mut choices: Vec<Choice> = Vec::new();
        for line in online_games.lines().filter(|line| !line.trim().is_empty()) {
            let Some((address, name)) = line.trim().split_once(' ') else {
                return Err(
                    format!("online_games.txt: {line:?} isn't `<address>:<port> <name>`").into(),
                );
            };
            choices.push(Choice::Online {
                name: name.trim().to_string(),
                address: address.to_string(),
            });
        }
        choices.extend([Choice::Address, Choice::Quit]);
        Ok(TitleScreen {
            choices,
            selected: 0,
        })
    }

    /// `get_lines` gives the screen's lines, each with whether it is the selected one.
    ///
    /// Example: `let lines = title.get_lines();`
    pub fn get_lines(&self) -> Vec<(String, bool)> {
        let header = HEADER_LINES.iter().map(|line| (line.to_string(), false));
        let choices = self
            .choices
            .iter()
            .enumerate()
            .map(|(i, choice)| (choice.get_label(), i == self.selected));
        header.chain(choices).collect()
    }

    /// `move_selection` moves the selection `step` lines down (up when negative), wrapping around.
    ///
    /// Example: `title.move_selection(-1);`
    pub fn move_selection(&mut self, step: i32) {
        let count: i32 = self.choices.len() as i32;
        self.selected = (self.selected as i32 + step).rem_euclid(count) as usize;
    }

    /// `get_selected` gives the selected choice.
    ///
    /// Example: `let choice = title.get_selected();`
    pub fn get_selected(&self) -> &Choice {
        &self.choices[self.selected]
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `title.rs`.
    //!
    //! | Behaviour | Cases | Test |
    //! | --- | --- | --- |
    //! | Choices | no online matches, two online matches first, a bad line | `without_online_matches_only_the_address_shows`, `online_matches_come_first`, `bad_online_lines_fail_naming_the_line` |
    //! | Flags | online, by address, quit | `choices_start_their_matches` |
    //! | Selection | wrapping both ways | `selection_wraps_around` |

    use super::*;

    #[test]
    fn without_online_matches_only_the_address_shows() {
        let title: TitleScreen = TitleScreen::New("").unwrap();
        assert_eq!(title.get_selected(), &Choice::Address);
        assert_eq!(title.get_lines().len(), HEADER_LINES.len() + 2);
    }

    #[test]
    fn online_matches_come_first() {
        let title: TitleScreen =
            TitleScreen::New("34.1.2.3:47850 Warped match\n\n10.0.0.2:47850 Test\n").unwrap();
        assert_eq!(
            title.get_selected(),
            &Choice::Online {
                name: "Warped match".to_string(),
                address: "34.1.2.3:47850".to_string()
            }
        );
        let labels: Vec<String> = title
            .get_lines()
            .into_iter()
            .map(|(line, _)| line)
            .collect();
        assert_eq!(
            labels[HEADER_LINES.len()..],
            [
                "Play online: Warped match",
                "Play online: Test",
                "Join a match by its address",
                "Quit"
            ]
        );
    }

    #[test]
    fn bad_online_lines_fail_naming_the_line() {
        let error = TitleScreen::New("34.1.2.3:47850").err().unwrap();
        assert!(error.to_string().contains("34.1.2.3:47850"));
    }

    #[test]
    fn choices_start_their_matches() {
        let online: Choice = Choice::Online {
            name: "Warped match".to_string(),
            address: "34.1.2.3:47850".to_string(),
        };
        assert_eq!(online.get_flags(), ["--match", "34.1.2.3:47850"]);
        assert!(online.is_online() && !online.needs_address());
        assert_eq!(Choice::Address.get_flags(), ["--match"]);
        assert!(Choice::Address.is_online() && Choice::Address.needs_address());
        assert!(Choice::Quit.get_flags().is_empty());
        assert!(!Choice::Quit.is_online());
    }

    #[test]
    fn selection_wraps_around() {
        let mut title: TitleScreen = TitleScreen::New("").unwrap();
        title.move_selection(-1);
        assert_eq!(title.get_selected(), &Choice::Quit);
        title.move_selection(1);
        assert_eq!(title.get_selected(), &Choice::Address);
    }
}
