//! `menu_and_map.rs` decides what covers a level's view, the same in every level (W-140, hold Tab
//! for the map, Esc for the menu): while the show-map control (`Tab`) is held the level's map
//! shows over the game, which goes on and keeps taking input; the open-menu control (`Escape`)
//! opens the one menu and frees the mouse, and closing it captures the mouse again; and the menu's
//! QUIT page leaves the level (`PlayOutcome::LeaveLevel`), which a level run alone treats as
//! quitting and the one-window app as going back to its game menu. Inside the one-window app the
//! menu also has a MATCH page first, with lines the app supplies (the match's status); a level run
//! alone has none. It also holds what every level's menu shares of its top two lines (W-151, menu
//! layout and help tooltip; W-150, arrow keys and clicks for menu pages): the menu's name with a
//! `?` whose help shows while the mouse is over it, the pages' names under it, which page a
//! click on that line opens, which keys turn the page, and where in the menu's text the mouse is.
//!
//! Interface: `MenuAndMap` (`New`, `open_menu`, `close_menu`, `get_menu`, `get_menu_mut`,
//! `is_menu_open`, `set_map_held`, `get_cover`), `Cover`, `MouseChange` (`apply`), `PlayOutcome`,
//! `QUIT_LINES`, `MATCH_PAGE_NAME`, `is_leave_control`, `format_title_line`, `format_tab_bar`,
//! `find_tab_at`, `is_help_mark_at`, `make_help_lines`, `PageTurn` (`FromControl`, `apply`),
//! `TextCell`, `TextLayout` (`get_cell_at`), `TITLE_LINE`, `TAB_BAR_LINE`.
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

/// `MATCH_PAGE_NAME` is the tab name of the menu's optional MATCH page, shown first when the caller
/// supplies its lines (each level's `Menu::set_match_lines`).
pub const MATCH_PAGE_NAME: &str = "MATCH";
const MENU_TITLE: &str = "MENU";
const HELP_MARK: &str = "?";
const HELP_MARK_COLUMN: usize = 62;
const HELP_MARK_REACH_COLUMNS: usize = 1;
const TAB_SEPARATOR: &str = " ";
const TAB_EDGE_CHAR_COUNT: usize = 2;
const HELP_LINES: [&str; 5] = [
    "HOW TO USE THE MENU",
    "",
    "Left/Right or Q/E     the page before or after",
    "Click a page's name   open that page",
    "Esc                   close the menu",
];
/// `TITLE_LINE` and `TAB_BAR_LINE` are the lines of the menu's name and of its pages' names in a
/// menu's lines.
pub const TITLE_LINE: usize = 0;
pub const TAB_BAR_LINE: usize = 1;
#[cfg(test)]
const MAX_LINE_CHAR_COUNT: usize = 64;

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

/// `format_title_line` gives the menu's first line: its name, and near the right end the `?`
/// whose help shows while the mouse is over it (`is_help_mark_at`).
///
/// Example: `let lines = vec![(format_title_line(), true)];`
pub fn format_title_line() -> String {
    format!("{MENU_TITLE:<HELP_MARK_COLUMN$}{HELP_MARK}")
}

/// `format_tab_bar` gives the menu's second line: every page's name, the shown one
/// (`shown_index`) in brackets.
///
/// Example: `let bar = format_tab_bar(&["SCORES", "QUIT"], 0);`
pub fn format_tab_bar(page_names: &[&str], shown_index: usize) -> String {
    let tabs: Vec<String> = page_names
        .iter()
        .enumerate()
        .map(|(i, name)| match i == shown_index {
            true => format!("[{name}]"),
            false => format!(" {name} "),
        })
        .collect();
    tabs.join(TAB_SEPARATOR)
}

/// `find_tab_at` gives the index of the page whose name, with its brackets or the spaces in their
/// place, covers `column` of `format_tab_bar(page_names, ..)`, or `None` between or past them.
///
/// Example: `assert_eq!(find_tab_at(&["SCORES", "QUIT"], 10), Some(1));`
pub fn find_tab_at(page_names: &[&str], column: usize) -> Option<usize> {
    let mut start: usize = 0;
    for (i, name) in page_names.iter().enumerate() {
        let end_excl: usize = start + name.chars().count() + TAB_EDGE_CHAR_COUNT;
        if (start..end_excl).contains(&column) {
            return Some(i);
        }
        start = end_excl + TAB_SEPARATOR.len();
    }
    None
}

/// `is_help_mark_at` says whether `cell` is on the title line's `?` or a column beside it (the
/// mark is one character, so the mouse gets a little room).
///
/// Example: `if pointer.is_some_and(is_help_mark_at) { ... }`
pub fn is_help_mark_at(cell: TextCell) -> bool {
    cell.line == TITLE_LINE && cell.column.abs_diff(HELP_MARK_COLUMN) <= HELP_MARK_REACH_COLUMNS
}

/// `make_help_lines` gives the help shown under the tab bar while the mouse is over the `?`: how
/// to turn pages and close the menu, then `page_lines`, the level's own lines about its pages.
///
/// Example: `lines.extend(make_help_lines(&["Up/Down   choose a row"]));`
pub fn make_help_lines(page_lines: &[&str]) -> Vec<(String, bool)> {
    HELP_LINES
        .iter()
        .chain(page_lines)
        .enumerate()
        .map(|(i, line)| (line.to_string(), i == 0))
        .collect()
}

/// `PageTurn` is a turn of the menu's page: to the one before or the one after.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageTurn {
    Before,
    After,
}

impl PageTurn {
    /// `FromControl` gives the turn `control` asks for: `Q` and `E` always, and the left and right
    /// arrows while `are_arrows_turning` (the page's own rows don't have the keys).
    ///
    /// Example: `if let Some(turn) = PageTurn::FromControl(control, true) { ... }`
    #[allow(non_snake_case)]
    pub fn FromControl(control: Control, are_arrows_turning: bool) -> Option<PageTurn> {
        match control {
            Control::Key(Scancode::Q) => Some(PageTurn::Before),
            Control::Key(Scancode::E) => Some(PageTurn::After),
            Control::Key(Scancode::Left) if are_arrows_turning => Some(PageTurn::Before),
            Control::Key(Scancode::Right) if are_arrows_turning => Some(PageTurn::After),
            _ => None,
        }
    }

    /// `apply` gives the page after this turn from `page_index` of `page_count` pages, going
    /// round past either end.
    ///
    /// Example: `self.page_index = turn.apply(self.page_index, page_count);`
    pub fn apply(self, page_index: usize, page_count: usize) -> usize {
        match self {
            PageTurn::Before => (page_index + page_count - 1) % page_count,
            PageTurn::After => (page_index + 1) % page_count,
        }
    }
}

/// `TextCell` is a place in the menu's text: a line and a column, both from 0.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextCell {
    pub line: usize,
    pub column: usize,
}

/// `TextLayout` is how a level draws its menu's text, to find the text under the mouse: the mask
/// the text is drawn into (`mask_px`), which is scaled by a whole number to at most `height_frac`
/// of the window's height and `max_width_frac` of its width and centred; and in that mask, where
/// the first character starts (`left_px`, `top_px`), a line's height and a character's width.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextLayout {
    pub mask_px: (u32, u32),
    pub height_frac: f32,
    pub max_width_frac: f64,
    pub left_px: f32,
    pub top_px: f32,
    pub line_px: f32,
    pub glyph_px: f32,
}

impl TextLayout {
    /// `get_cell_at` gives the text cell under `pointer_px` in a window of `window_px` (both in
    /// the window's pixels), or `None` outside the mask or left of or above its text.
    ///
    /// Example: `let cell = layout.get_cell_at(window.size_in_pixels(), (x_px, y_px));`
    pub fn get_cell_at(&self, window_px: (u32, u32), pointer_px: (f32, f32)) -> Option<TextCell> {
        let (mask_width, mask_height) = self.mask_px;
        let scale: f32 = ((window_px.1 as f32 * self.height_frac) as u32 / mask_height)
            .min((f64::from(window_px.0) * self.max_width_frac) as u32 / mask_width)
            .max(1) as f32;
        let x: f32 =
            (pointer_px.0 - (window_px.0 as f32 - mask_width as f32 * scale) / 2.0) / scale;
        let y: f32 =
            (pointer_px.1 - (window_px.1 as f32 - mask_height as f32 * scale) / 2.0) / scale;
        let is_in_text: bool = (self.left_px..mask_width as f32).contains(&x)
            && (self.top_px..mask_height as f32).contains(&y);
        is_in_text.then(|| TextCell {
            line: ((y - self.top_px) / self.line_px) as usize,
            column: ((x - self.left_px) / self.glyph_px) as usize,
        })
    }
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
    //! | Tab bar | five pages, six (MATCH first): the shown one in brackets, within 64 characters | `the_tab_bar_fits_with_the_match_page` |
    //! | Title line | the menu's name first, the `?` within 64 characters | `the_title_line_names_the_menu_and_ends_in_the_help_mark` |
    //! | Column on the tab bar | in a page's name, on its bracket, on the space between two, past the last | `a_column_finds_its_tab` |
    //! | Cell and the help mark | on it, beside it, two columns away, on another line | `the_help_mark_has_a_little_room` |
    //! | Help lines | none of the level's own, some: the first line highlighted, all within 64 characters | `help_lists_the_shared_keys_then_the_levels` |
    //! | Page turn | `Q`, `E`; arrows turning or not; another key; past the first and the last page | `q_e_and_arrows_turn_pages` |
    //! | Pointer in the window | on a character (scale 1, scale 3), left of the text, above it, outside the mask | `the_pointer_finds_its_text_cell` |

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

    #[test]
    fn the_tab_bar_fits_with_the_match_page() {
        let pages: [&str; 5] = ["SCORES", "SETTINGS", "MAP", "CREDITS", "QUIT"];
        let bar: String = format_tab_bar(&pages, 1);
        assert!(bar.starts_with(" SCORES  [SETTINGS]  MAP "), "{bar}");
        let with_match: Vec<&str> = [MATCH_PAGE_NAME].into_iter().chain(pages).collect();
        let bar: String = format_tab_bar(&with_match, 0);
        assert!(bar.starts_with("[MATCH]  SCORES "), "{bar}");
        assert!(bar.chars().count() <= MAX_LINE_CHAR_COUNT, "{bar}");
    }

    #[test]
    fn the_title_line_names_the_menu_and_ends_in_the_help_mark() {
        let title: String = format_title_line();
        assert!(title.starts_with("MENU "), "{title}");
        assert!(title.ends_with(" ?"), "{title}");
        assert_eq!(title.chars().count(), HELP_MARK_COLUMN + 1);
        assert!(title.chars().count() <= MAX_LINE_CHAR_COUNT);
    }

    #[test]
    fn a_column_finds_its_tab() {
        let pages: [&str; 3] = ["SCORES", "MAP", "QUIT"];
        let bar: String = format_tab_bar(&pages, 1);
        assert_eq!(bar, " SCORES  [MAP]  QUIT ");
        for (column, tab) in [
            (0, Some(0)),
            (3, Some(0)),
            (7, Some(0)),
            (8, None),
            (9, Some(1)),
            (13, Some(1)),
            (14, None),
            (17, Some(2)),
            (20, Some(2)),
            (21, None),
            (60, None),
        ] {
            assert_eq!(find_tab_at(&pages, column), tab, "column {column}");
        }
    }

    #[test]
    fn the_help_mark_has_a_little_room() {
        let cell = |line: usize, column: usize| TextCell { line, column };
        assert!(is_help_mark_at(cell(TITLE_LINE, HELP_MARK_COLUMN)));
        assert!(is_help_mark_at(cell(TITLE_LINE, HELP_MARK_COLUMN - 1)));
        assert!(is_help_mark_at(cell(TITLE_LINE, HELP_MARK_COLUMN + 1)));
        assert!(!is_help_mark_at(cell(TITLE_LINE, HELP_MARK_COLUMN - 2)));
        assert!(!is_help_mark_at(cell(TAB_BAR_LINE, HELP_MARK_COLUMN)));
    }

    #[test]
    fn help_lists_the_shared_keys_then_the_levels() {
        let shared: Vec<(String, bool)> = make_help_lines(&[]);
        assert_eq!(shared.len(), HELP_LINES.len());
        assert_eq!(shared[0], ("HOW TO USE THE MENU".to_string(), true));
        assert!(
            shared[1..]
                .iter()
                .all(|(_, is_highlighted)| !is_highlighted)
        );
        assert!(shared.iter().any(|(line, _)| line.contains("Q/E")));
        let with_own: Vec<(String, bool)> = make_help_lines(&["", "Up/Down   choose a row"]);
        assert_eq!(with_own.len(), HELP_LINES.len() + 2);
        assert_eq!(with_own[with_own.len() - 1].0, "Up/Down   choose a row");
        for (line, _) in &with_own {
            assert!(line.chars().count() <= MAX_LINE_CHAR_COUNT, "{line}");
        }
    }

    #[test]
    fn q_e_and_arrows_turn_pages() {
        let key = |scancode: Scancode, are_arrows_turning: bool| {
            PageTurn::FromControl(Control::Key(scancode), are_arrows_turning)
        };
        for are_arrows_turning in [false, true] {
            assert_eq!(key(Scancode::Q, are_arrows_turning), Some(PageTurn::Before));
            assert_eq!(key(Scancode::E, are_arrows_turning), Some(PageTurn::After));
            assert_eq!(key(Scancode::Down, are_arrows_turning), None);
        }
        assert_eq!(key(Scancode::Left, true), Some(PageTurn::Before));
        assert_eq!(key(Scancode::Right, true), Some(PageTurn::After));
        assert_eq!(key(Scancode::Left, false), None);
        assert_eq!(key(Scancode::Right, false), None);
        assert_eq!(PageTurn::FromControl(Control::Wheel, true), None);
        assert_eq!(PageTurn::After.apply(1, 5), 2);
        assert_eq!(PageTurn::After.apply(4, 5), 0);
        assert_eq!(PageTurn::Before.apply(1, 5), 0);
        assert_eq!(PageTurn::Before.apply(0, 5), 4);
    }

    #[test]
    fn the_pointer_finds_its_text_cell() {
        let layout: TextLayout = TextLayout {
            mask_px: (512, 312),
            height_frac: 0.85,
            max_width_frac: 0.8,
            left_px: 4.0,
            top_px: 2.0,
            line_px: 12.0,
            glyph_px: 8.0,
        };
        let cell = |line: usize, column: usize| Some(TextCell { line, column });
        let small: (u32, u32) = (800, 600);
        let (left, top) = (144.0, 144.0);
        assert_eq!(
            layout.get_cell_at(small, (left + 4.0, top + 2.0)),
            cell(0, 0)
        );
        assert_eq!(
            layout.get_cell_at(small, (left + 29.0, top + 15.0)),
            cell(1, 3)
        );
        assert_eq!(layout.get_cell_at(small, (left + 2.0, top + 15.0)), None);
        assert_eq!(layout.get_cell_at(small, (left + 29.0, top + 1.0)), None);
        assert_eq!(layout.get_cell_at(small, (left - 5.0, top + 15.0)), None);
        assert_eq!(layout.get_cell_at(small, (left + 513.0, top + 15.0)), None);
        let large: (u32, u32) = (2560, 1440);
        let (left, top, scale) = (512.0, 252.0, 3.0);
        assert_eq!(
            layout.get_cell_at(large, (left + scale * 4.0, top + scale * 2.0)),
            cell(0, 0)
        );
        assert_eq!(
            layout.get_cell_at(large, (left + scale * 500.0, top + scale * 16.0)),
            cell(1, 62)
        );
    }
}
