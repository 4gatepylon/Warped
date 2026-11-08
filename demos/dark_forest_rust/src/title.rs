//! `title` is the title screen the window opens on when no flag says what to play (ported from
//! the hyperbolic demo's `title.rs`): playing alone against robots, hosting a game on this Wi-Fi,
//! joining one, or quitting. It keeps no window state: the window feeds it keys and draws its lines.
//!
//! Interface: `TitleScreen` (`New`, `get_lines`, `move_selection`, `get_selected`), `Choice`.
//!
//! Example: `let mut title = TitleScreen::New(); title.move_selection(1); let choice = title.get_selected();`

/// `Choice` is one thing the title screen can start.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Choice {
    PlayAlone,
    Host,
    Join,
    Quit,
}

const CHOICES: [(Choice, &str); 4] = [
    (Choice::PlayAlone, "PLAY ALONE AGAINST ROBOTS"),
    (Choice::Host, "HOST A GAME ON THIS WI-FI"),
    (Choice::Join, "JOIN A GAME ON THIS WI-FI"),
    (Choice::Quit, "QUIT"),
];
const HEADER_LINES: [&str; 5] = [
    "DARK FOREST",
    "",
    "Climb the treehouses, cross the bridges and switch",
    "off both of the other team's consoles to win.",
    "",
];
const FOOTER_LINE: &str = "Up/Down choose   Return start   Esc quits";

/// `TitleScreen` is the title screen's selected choice.
pub struct TitleScreen {
    selected_index: usize,
}

impl TitleScreen {
    /// `New` makes the title screen with its first choice selected.
    ///
    /// Example: `let title = TitleScreen::New();`
    #[allow(non_snake_case)]
    pub fn New() -> TitleScreen {
        TitleScreen { selected_index: 0 }
    }

    /// `get_lines` gives the screen's lines, each with whether it is highlighted: the name and
    /// goal, the choices with the selected one highlighted, and the keys.
    ///
    /// Example: `picture.draw(&title.get_lines(), true);`
    pub fn get_lines(&self) -> Vec<(String, bool)> {
        let mut lines: Vec<(String, bool)> = HEADER_LINES
            .iter()
            .enumerate()
            .map(|(i, line)| (line.to_string(), i == 0))
            .collect();
        for (i, (_, label)) in CHOICES.iter().enumerate() {
            lines.push(match i == self.selected_index {
                true => (format!("> {label} <"), true),
                false => (label.to_string(), false),
            });
        }
        lines.push((String::new(), false));
        lines.push((FOOTER_LINE.to_string(), false));
        lines
    }

    /// `move_selection` moves the selection `step` choices down (up if negative), wrapping round.
    ///
    /// Example: `title.move_selection(-1);`
    pub fn move_selection(&mut self, step: i32) {
        let count: i32 = CHOICES.len() as i32;
        self.selected_index = (self.selected_index as i32 + step).rem_euclid(count) as usize;
    }

    /// `get_selected` gives the selected choice.
    ///
    /// Example: `if title.get_selected() == Choice::Quit { return; }`
    pub fn get_selected(&self) -> Choice {
        CHOICES[self.selected_index].0
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the title screen's selection.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Step | down, up past the first (wraps), down past the last (wraps) | `selection_wraps` |
    //! | Lines | the selected choice highlighted, the others not | `only_the_selected_choice_is_marked` |

    use super::*;

    #[test]
    fn selection_wraps() {
        let mut title: TitleScreen = TitleScreen::New();
        assert_eq!(title.get_selected(), Choice::PlayAlone);
        title.move_selection(1);
        assert_eq!(title.get_selected(), Choice::Host);
        title.move_selection(-2);
        assert_eq!(title.get_selected(), Choice::Quit);
        title.move_selection(1);
        assert_eq!(title.get_selected(), Choice::PlayAlone);
    }

    #[test]
    fn only_the_selected_choice_is_marked() {
        let mut title: TitleScreen = TitleScreen::New();
        title.move_selection(2);
        let marked: Vec<String> = title
            .get_lines()
            .into_iter()
            .filter(|(line, _)| line.starts_with('>'))
            .map(|(line, _)| line)
            .collect();
        assert_eq!(marked, vec!["> JOIN A GAME ON THIS WI-FI <".to_string()]);
    }
}
