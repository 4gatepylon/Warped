//! `main_menu.rs` is the main menu a double-clicked app opens on (W-58): the choices of game
//! (online games, or joining one on this Wi-Fi on either map), the one selected, and the flags that
//! start each. It keeps no window state: the window feeds it keys and
//! draws its lines, as with `menu.rs`.
//!
//! Interface: `MainMenu` (`New`, `get_lines`, `move_selection`, `get_selected`), `Choice`
//! (`get_flags`, `is_online`).
//!
//! Example: `let title = MainMenu::New("34.1.2.3:47810 lunar-surface Lunar Surface 1")?;`
//!
//! Must know:
//! - The online games come from the app's `online_games.txt`, one game a line:
//!   `<address>:<port> <map> <name>`, such as `34.1.2.3:47810 lunar-surface Lunar Surface 1`.
//! - It offers no hosting and no playing alone: the game plays only on a game server (W-79).

use std::error::Error;

use clap::ValueEnum;

use crate::map::MapName;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const MAPS: [MapName; 3] = [MapName::Dust, MapName::LunarSurface, MapName::HouseDemo];
const HEADER_LINES: [&str; 3] = [
    "WARPED",
    "Up and Down choose, Return starts, Escape quits.",
    "",
];

/// `Choice` is one line of the main menu: a game to start, or quitting.
#[derive(Clone, PartialEq, Debug)]
pub enum Choice {
    Online {
        name: String,
        address: String,
        map: MapName,
    },
    Join(MapName),
    Quit,
}

/// `MainMenu` is the main menu's choices and the selected one.
pub struct MainMenu {
    choices: Vec<Choice>,
    selected: usize,
}

/// `get_map_label` gives a map's name as players see it.
///
/// Example: `let label = get_map_label(MapName::LunarSurface);`
fn get_map_label(map: MapName) -> &'static str {
    match map {
        MapName::Dust => "Dust",
        MapName::HouseDemo => "House",
        MapName::LunarSurface => "Lunar Surface",
    }
}

/// `get_map_flag` gives a map's name as `--map` takes it, e.g. `lunar-surface`.
///
/// Example: `let flag = get_map_flag(MapName::LunarSurface);`
fn get_map_flag(map: MapName) -> String {
    map.to_possible_value()
        .expect("every MapName has a --map value")
        .get_name()
        .to_string()
}

impl Choice {
    /// `get_label` gives the choice's line on the main menu.
    ///
    /// Example: `let line = choice.get_label();`
    fn get_label(&self) -> String {
        match self {
            Choice::Online { name, .. } => format!("Play online: {name}"),
            Choice::Join(map) => format!("Join a game on this Wi-Fi: {}", get_map_label(*map)),
            Choice::Quit => "Quit".to_string(),
        }
    }

    /// `get_flags` gives the flags that start the chosen game (none for `Quit`); an online game
    /// also needs `--game-password-file`.
    ///
    /// Example: `let flags = Choice::Join(MapName::HouseDemo).get_flags();`
    pub fn get_flags(&self) -> Vec<String> {
        let (address, map): (&str, MapName) = match self {
            Choice::Online { address, map, .. } => (address, *map),
            Choice::Join(map) => ("auto", *map),
            Choice::Quit => return Vec::new(),
        };
        ["--join", address, "--map", &get_map_flag(map)]
            .map(str::to_string)
            .to_vec()
    }

    /// `is_online` says whether the choice is a game on a server on the internet.
    ///
    /// Example: `if choice.is_online() { ... }`
    pub fn is_online(&self) -> bool {
        matches!(self, Choice::Online { .. })
    }
}

impl MainMenu {
    /// `New` makes the main menu from the text of `online_games.txt` (empty for none), with
    /// the first choice selected.
    ///
    /// Example: `let title = MainMenu::New("")?;`
    #[allow(non_snake_case)]
    pub fn New(online_games: &str) -> Result<MainMenu> {
        let mut choices: Vec<Choice> = Vec::new();
        for line in online_games.lines().filter(|line| !line.trim().is_empty()) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let [address, map, name @ ..] = &words[..] else {
                return Err(format!(
                    "title: online game {line:?} isn't \"<address>:<port> <map> <name>\""
                )
                .into());
            };
            let map: MapName = MapName::from_str(map, false)
                .map_err(|error| format!("title: online game {line:?}: {error}"))?;
            if name.is_empty() {
                return Err(format!("title: online game {line:?} has no name").into());
            }
            choices.push(Choice::Online {
                name: name.join(" "),
                address: address.to_string(),
                map,
            });
        }
        choices.extend(MAPS.map(Choice::Join));
        choices.push(Choice::Quit);
        Ok(MainMenu {
            choices,
            selected: 0,
        })
    }

    /// `get_lines` gives the main menu's lines, each with whether it is highlighted (the
    /// title and the selected choice).
    ///
    /// Example: `let lines = title.get_lines();`
    pub fn get_lines(&self) -> Vec<(String, bool)> {
        let header = HEADER_LINES
            .iter()
            .enumerate()
            .map(|(i, line)| (line.to_string(), i == 0));
        let choices = self.choices.iter().enumerate().map(|(i, choice)| {
            let marker: &str = if i == self.selected { "> " } else { "  " };
            (
                format!("{marker}{}", choice.get_label()),
                i == self.selected,
            )
        });
        header.chain(choices).collect()
    }

    /// `move_selection` selects the choice `step` lines below the selected one (above for a
    /// negative step), wrapping around.
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
    //! Tests of the main menu's choices.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | `online_games.txt` | empty, games (blank lines skipped), a line missing its name, an unknown map | `without_online_games_only_local_choices_show`, `online_games_come_first`, `bad_online_games_fail_naming_the_line` |
    //! | Choice | online, join on this Wi-Fi, quit | `choices_start_their_games` |
    //! | Selection move | down, up past the first (wraps) | `selection_wraps_around` |

    use super::*;

    const LOCAL_CHOICE_COUNT: usize = 4;

    #[test]
    fn without_online_games_only_local_choices_show() {
        let title: MainMenu = MainMenu::New("").expect("a main menu");
        assert_eq!(title.choices.len(), LOCAL_CHOICE_COUNT);
        assert_eq!(title.get_selected(), &Choice::Join(MapName::Dust));
    }

    #[test]
    fn online_games_come_first() {
        let text: &str =
            "34.1.2.3:47810 lunar-surface Lunar Surface 1\n\n34.1.2.3:47812 house-demo House\n";
        let title: MainMenu = MainMenu::New(text).expect("a main menu");
        assert_eq!(title.choices.len(), LOCAL_CHOICE_COUNT + 2);
        assert_eq!(
            title.get_selected(),
            &Choice::Online {
                name: "Lunar Surface 1".to_string(),
                address: "34.1.2.3:47810".to_string(),
                map: MapName::LunarSurface
            }
        );
        let lines: Vec<(String, bool)> = title.get_lines();
        assert_eq!(
            lines[HEADER_LINES.len()],
            ("> Play online: Lunar Surface 1".to_string(), true)
        );
    }

    #[test]
    fn bad_online_games_fail_naming_the_line() {
        for line in [
            "34.1.2.3:47810 lunar-surface",
            "34.1.2.3:47810 moon Moon Base",
        ] {
            let error: String = MainMenu::New(line).err().expect("a bad line").to_string();
            assert!(error.contains(line), "{error}");
        }
    }

    #[test]
    fn choices_start_their_games() {
        let online: Choice = Choice::Online {
            name: "House".to_string(),
            address: "34.1.2.3:47812".to_string(),
            map: MapName::HouseDemo,
        };
        assert_eq!(
            online.get_flags(),
            ["--join", "34.1.2.3:47812", "--map", "house-demo"]
        );
        assert!(online.is_online());
        assert_eq!(
            Choice::Join(MapName::HouseDemo).get_flags(),
            ["--join", "auto", "--map", "house-demo"]
        );
        assert!(Choice::Quit.get_flags().is_empty());
        assert!(!Choice::Quit.is_online());
    }

    #[test]
    fn selection_wraps_around() {
        let mut title: MainMenu = MainMenu::New("").expect("a main menu");
        title.move_selection(1);
        assert_eq!(title.get_selected(), &Choice::Join(MapName::LunarSurface));
        title.move_selection(-2);
        assert_eq!(title.get_selected(), &Choice::Quit);
    }
}
