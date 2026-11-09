//! `menu_and_map.rs` decides what covers a level's view, the same in every level (W-140, hold Tab
//! for the map, Esc for the menu): while the show-map control (`Tab`) is held the level's map
//! shows over the game, which goes on and keeps taking input; the open-menu control (`Escape`)
//! opens the one menu and frees the mouse, and closing it captures the mouse again; and the menu's
//! QUIT page leaves the level (`PlayOutcome::LeaveLevel`), which a level run alone treats as
//! quitting and the one-window app as going back to its game menu.
//!
//! Interface: `MenuAndMap` (`New`, `open_menu`, `close_menu`, `get_menu`, `get_menu_mut`,
//! `is_menu_open`, `set_map_held`, `get_cover`), `Cover`, `MouseChange` (`apply`), `PlayOutcome`,
//! `QUIT_LINES`, `is_leave_control`.
//!
//! Example: `let change = menu_and_map.open_menu(Menu::New()); change.apply(&mut is_captured, &window, &mouse);`
//!
//! Must know:
//! - Gotchas:
//!   - The QUIT page's keys (`Return` or `Y` leaves) work whatever the key bindings say, like the
//!     menu's other keys.

use sdl3::keyboard::Scancode;
use sdl3::mouse::MouseUtil;
use sdl3::video::Window;

use crate::key_bindings::Control;

/// `QUIT_LINES` is what the menu's QUIT page says under its tab bar.
pub const QUIT_LINES: [&str; 3] = [
    "Do you want to leave the game?",
    "",
    "Return/Y: leave   Esc: back to the game",
];

/// `Cover` is what covers the game view: nothing, the map while its control is held, or the menu.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cover {
    Nothing,
    Map,
    Menu,
}

/// `MouseChange` is what the window does with the mouse after the menu opens or closes: free it
/// for the menu, or capture it again for play.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MouseChange {
    Free,
    Capture,
}

/// `PlayOutcome` is what a level's window does after an event: play on, leave the level (the
/// QUIT page: a level run alone quits, the one-window app goes back to its game menu), or close
/// (the window's close button: quit).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PlayOutcome {
    Play,
    LeaveLevel,
    CloseWindow,
}

impl MouseChange {
    /// `apply` frees or captures the mouse in `window` (relative mouse mode) and records it in
    /// `is_captured`.
    ///
    /// Example: `menu_and_map.close_menu().apply(&mut self.is_captured, window, mouse);`
    pub fn apply(self, is_captured: &mut bool, window: &Window, mouse: &MouseUtil) {
        *is_captured = self == MouseChange::Capture;
        mouse.set_relative_mouse_mode(window, *is_captured);
    }
}

/// `is_leave_control` says whether `control`, pressed on the menu's QUIT page, leaves the level:
/// `Return` or `Y`.
///
/// Example: `if is_leave_control(Control::Key(Scancode::Y)) { ... }`
pub fn is_leave_control(control: Control) -> bool {
    matches!(control, Control::Key(Scancode::Return | Scancode::Y))
}

/// `MenuAndMap` is a level's menu (of the level's own type `M`, `None` while closed) and whether
/// the show-map control is held.
pub struct MenuAndMap<M> {
    menu: Option<M>,
    is_map_held: bool,
}

impl<M> MenuAndMap<M> {
    /// `New` starts with the menu closed and the map hidden.
    ///
    /// Example: `let menu_and_map: MenuAndMap<Menu> = MenuAndMap::New();`
    #[allow(non_snake_case)]
    pub fn New() -> MenuAndMap<M> {
        MenuAndMap {
            menu: None,
            is_map_held: false,
        }
    }

    /// `open_menu` opens `menu`, which then gets every pressed control, and frees the mouse.
    ///
    /// Example: `let change = menu_and_map.open_menu(Menu::New());`
    pub fn open_menu(&mut self, menu: M) -> MouseChange {
        assert!(self.menu.is_none(), "open_menu: the menu is already open");
        self.menu = Some(menu);
        MouseChange::Free
    }

    /// `close_menu` closes the menu and captures the mouse again, back to play.
    ///
    /// Example: `let change = menu_and_map.close_menu();`
    pub fn close_menu(&mut self) -> MouseChange {
        assert!(self.menu.is_some(), "close_menu: the menu isn't open");
        self.menu = None;
        MouseChange::Capture
    }

    /// `get_menu` gives the open menu, or `None` while it is closed.
    ///
    /// Example: `if let Some(menu) = menu_and_map.get_menu() { ... }`
    pub fn get_menu(&self) -> Option<&M> {
        self.menu.as_ref()
    }

    /// `get_menu_mut` lends the open menu, or `None` while it is closed.
    ///
    /// Example: `let menu = menu_and_map.get_menu_mut().expect("the menu is open");`
    pub fn get_menu_mut(&mut self) -> Option<&mut M> {
        self.menu.as_mut()
    }

    /// `is_menu_open` says whether the menu is open.
    ///
    /// Example: `if menu_and_map.is_menu_open() { ... }`
    pub fn is_menu_open(&self) -> bool {
        self.menu.is_some()
    }

    /// `set_map_held` records whether the show-map control is held now, read from the keyboard's
    /// state every frame, so a release the window missed can't leave the map up.
    ///
    /// Example: `menu_and_map.set_map_held(keys.is_held(Action::ShowMap, &keyboard, &mouse));`
    pub fn set_map_held(&mut self, is_held: bool) {
        self.is_map_held = is_held;
    }

    /// `get_cover` gives what covers the view: the menu while it is open, else the map while its
    /// control is held, else nothing.
    ///
    /// Example: `if menu_and_map.get_cover() == Cover::Map { ... }`
    pub fn get_cover(&self) -> Cover {
        match (self.menu.is_some(), self.is_map_held) {
            (true, _) => Cover::Menu,
            (false, true) => Cover::Map,
            (false, false) => Cover::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests of what covers the view and what happens to the mouse.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Show-map control | pressed (the map shows), let go (it hides), held while the menu opens and closes | `holding_tab_shows_the_map_until_it_is_let_go`, `the_menu_covers_the_held_map` |
    //! | Menu | opened (frees the mouse), closed (captures it), opened again after closing | `escape_opens_and_closes_the_menu` |
    //! | Leave control on the QUIT page | `Return`, `Y` (leave); `Escape`, another key, a mouse button, the wheel (stay) | `return_and_y_leave` |

    use sdl3::mouse::MouseButton;

    use super::*;

    #[test]
    fn holding_tab_shows_the_map_until_it_is_let_go() {
        let mut menu_and_map: MenuAndMap<()> = MenuAndMap::New();
        assert_eq!(menu_and_map.get_cover(), Cover::Nothing);
        menu_and_map.set_map_held(true);
        assert_eq!(menu_and_map.get_cover(), Cover::Map);
        assert!(!menu_and_map.is_menu_open());
        menu_and_map.set_map_held(false);
        assert_eq!(menu_and_map.get_cover(), Cover::Nothing);
    }

    #[test]
    fn escape_opens_and_closes_the_menu() {
        let mut menu_and_map: MenuAndMap<u32> = MenuAndMap::New();
        for _ in 0..2 {
            assert_eq!(menu_and_map.open_menu(7), MouseChange::Free);
            assert_eq!(menu_and_map.get_cover(), Cover::Menu);
            assert_eq!(menu_and_map.get_menu(), Some(&7));
            *menu_and_map.get_menu_mut().expect("the menu is open") = 8;
            assert_eq!(menu_and_map.get_menu(), Some(&8));
            assert_eq!(menu_and_map.close_menu(), MouseChange::Capture);
            assert_eq!(menu_and_map.get_cover(), Cover::Nothing);
            assert_eq!(menu_and_map.get_menu(), None);
        }
    }

    #[test]
    fn the_menu_covers_the_held_map() {
        let mut menu_and_map: MenuAndMap<()> = MenuAndMap::New();
        menu_and_map.set_map_held(true);
        menu_and_map.open_menu(());
        assert_eq!(menu_and_map.get_cover(), Cover::Menu);
        menu_and_map.close_menu();
        assert_eq!(menu_and_map.get_cover(), Cover::Map);
    }

    #[test]
    fn return_and_y_leave() {
        for key in [Scancode::Return, Scancode::Y] {
            assert!(is_leave_control(Control::Key(key)), "{key:?}");
        }
        for control in [
            Control::Key(Scancode::Escape),
            Control::Key(Scancode::N),
            Control::Mouse(MouseButton::Left),
            Control::Wheel,
        ] {
            assert!(!is_leave_control(control), "{control:?}");
        }
    }
}
