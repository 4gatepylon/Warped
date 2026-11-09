//! `exit_prompt.rs` is a copy of `../hyperbolic_wormholes_rust/src/exit_prompt.rs` that takes SDL
//! scancodes for its key-binding controls (this level has no key bindings). It asks "Do you want to
//! exit?" before the game window quits, so a stray `Escape`
//! doesn't end a game: the release-mouse control (`Escape`) frees a captured mouse, pressed again
//! with the mouse free it shows the prompt, and the prompt's answer quits or goes back to the game.
//! It keeps no window state: the window feeds it pressed controls and draws its lines.
//!
//! Interface: `ExitPrompt` (`New`, `is_shown`, `press_release_mouse`, `answer`), `ExitOutcome`,
//! `get_lines`.
//!
//! Example: `if prompt.answer(scancode) == ExitOutcome::Exit { break; }`
//!
//! Must know:
//! - Gotchas:
//!   - The prompt's keys (`Return` or `Y` exits, `Escape` or `N` stays) work whatever the key
//!     bindings say, and while it shows no other control reaches the game or the menu.

use sdl3::keyboard::Scancode;

const QUESTION_LINE: &str = "Do you want to exit?";
const KEYS_LINE: &str = "Return/Y: exit   Esc/N: stay";

/// `ExitOutcome` is what the window does after a control the exit prompt took: nothing more (the
/// prompt shows or hides by itself), free the mouse, or quit.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ExitOutcome {
    Stay,
    ReleaseMouse,
    Exit,
}

/// `ExitPrompt` is whether the window asks "Do you want to exit?".
pub struct ExitPrompt {
    is_shown: bool,
}

impl ExitPrompt {
    /// `New` starts with the prompt hidden.
    ///
    /// Example: `let prompt = ExitPrompt::New();`
    #[allow(non_snake_case)]
    pub fn New() -> ExitPrompt {
        ExitPrompt { is_shown: false }
    }

    /// `is_shown` says whether the prompt shows, so the window passes it every pressed control and
    /// freezes play.
    ///
    /// Example: `if prompt.is_shown() { ... }`
    pub fn is_shown(&self) -> bool {
        self.is_shown
    }

    /// `press_release_mouse` reacts to the release-mouse control pressed in play: with the mouse
    /// captured it frees the mouse, and with the mouse free it shows the prompt.
    ///
    /// Example: `let outcome = prompt.press_release_mouse(self.is_captured);`
    pub fn press_release_mouse(&mut self, is_captured: bool) -> ExitOutcome {
        if is_captured {
            return ExitOutcome::ReleaseMouse;
        }
        self.is_shown = true;
        ExitOutcome::Stay
    }

    /// `answer` reacts to a control pressed while the prompt shows: `Return` or `Y` quits,
    /// `Escape` or `N` hides the prompt, and any other control does nothing.
    ///
    /// Example: `if prompt.answer(Scancode::Y) == ExitOutcome::Exit { ... }`
    pub fn answer(&mut self, scancode: Scancode) -> ExitOutcome {
        assert!(self.is_shown, "answer: the exit prompt isn't shown");
        match scancode {
            Scancode::Return | Scancode::Y => ExitOutcome::Exit,
            Scancode::Escape | Scancode::N => {
                self.is_shown = false;
                ExitOutcome::Stay
            }
            _ => ExitOutcome::Stay,
        }
    }
}

/// `get_lines` gives the prompt's two lines, centred over the view like a banner: the question and
/// its keys.
///
/// Example: `let [question, keys] = get_lines();`
pub fn get_lines() -> [String; 2] {
    [QUESTION_LINE.to_string(), KEYS_LINE.to_string()]
}

#[cfg(test)]
mod tests {
    //! Tests of the exit prompt's state machine.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Mouse when the release-mouse control is pressed | captured (freed, no prompt), free (prompt shown) | `escape_frees_the_mouse_then_asks` |
    //! | Answer | `Return`, `Y` (exit); `Escape`, `N` (stay, prompt hidden); another key (nothing) | `return_and_y_exit`, `escape_and_n_stay`, `other_controls_do_nothing` |
    //! | Prompt after staying | asked again by the release-mouse control | `escape_and_n_stay` |

    use super::*;

    /// `make_shown` gives a prompt that shows, as after `Escape` with the mouse free.
    ///
    /// Example: `let mut prompt = make_shown();`
    fn make_shown() -> ExitPrompt {
        let mut prompt: ExitPrompt = ExitPrompt::New();
        prompt.press_release_mouse(false);
        prompt
    }

    #[test]
    fn escape_frees_the_mouse_then_asks() {
        let mut prompt: ExitPrompt = ExitPrompt::New();
        assert_eq!(prompt.press_release_mouse(true), ExitOutcome::ReleaseMouse);
        assert!(!prompt.is_shown());
        assert_eq!(prompt.press_release_mouse(false), ExitOutcome::Stay);
        assert!(prompt.is_shown());
    }

    #[test]
    fn return_and_y_exit() {
        for key in [Scancode::Return, Scancode::Y] {
            let mut prompt: ExitPrompt = make_shown();
            assert_eq!(prompt.answer(key), ExitOutcome::Exit, "{key:?}");
        }
    }

    #[test]
    fn escape_and_n_stay() {
        for key in [Scancode::Escape, Scancode::N] {
            let mut prompt: ExitPrompt = make_shown();
            assert_eq!(prompt.answer(key), ExitOutcome::Stay, "{key:?}");
            assert!(!prompt.is_shown(), "{key:?}");
            prompt.press_release_mouse(false);
            assert!(prompt.is_shown(), "{key:?}");
        }
    }

    #[test]
    fn other_controls_do_nothing() {
        let mut prompt: ExitPrompt = make_shown();
        for key in [Scancode::Tab, Scancode::W, Scancode::Space] {
            assert_eq!(prompt.answer(key), ExitOutcome::Stay, "{key:?}");
            assert!(prompt.is_shown(), "{key:?}");
        }
    }
}
