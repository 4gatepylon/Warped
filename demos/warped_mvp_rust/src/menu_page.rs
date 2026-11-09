//! `menu_page.rs` is what the app's menus share: the inputs a menu takes (`MenuInput`, mapped from
//! the window's keys), moving a selection through rows, and drawing a list of rows and a title in
//! the debug font (`warped_shared::page`).
//!
//! Interface: `MenuInput`, `MenuRow`, `move_selection`, `push_rows`, `push_title`,
//! `push_heading`, `get_editor_input`, `page_t`, `PAGE_WIDTH`, `LINE`, `MARGIN`.
//!
//! Example: `push_rows(&mut shapes, 60.0, &rows, selected);`

use warped_shared::match_menu::{DIM_RGB, HIGHLIGHT_RGB, HUD_RGB};
use warped_shared::page::{GLYPH, Shape, push_text};
use warped_shared::settings_editor::EditorInput;

/// `PAGE_WIDTH` is the width of every menu page in page units, the progression page's.
pub const PAGE_WIDTH: f32 = 640.0;
/// `LINE` is the height of one row of text in page units.
pub const LINE: f32 = 12.0;
/// `MARGIN` is the left margin of a page's text in page units.
pub const MARGIN: f32 = 20.0;
/// `page_t` is a page: its shapes, and its size in page units.
#[allow(non_camel_case_types)]
pub type page_t = (Vec<Shape>, [f32; 2]);
const SELECTED_RGBA: [u8; 4] = [60, 70, 95, 230];
const ROW_PAD: f32 = 2.0;
const TITLE_RGB: [u8; 3] = [255, 255, 255];

/// `MenuInput` is what a key does in a menu: move up or down, change the row's value down or up,
/// choose the row, go back (Escape), reset the row, type a character or erase one, or, while a
/// settings page rebinds an action, the name of the control pressed.
#[derive(Clone, PartialEq, Debug)]
pub enum MenuInput {
    Up,
    Down,
    Left,
    Right,
    Activate,
    Back,
    Reset,
    Typed(char),
    Erase,
    Control(String),
}

/// `MenuRow` is one row as a page shows it: its text, and whether it can be chosen now (a row
/// that can't is dimmed).
#[derive(Clone, PartialEq, Debug)]
pub struct MenuRow {
    pub text: String,
    pub is_enabled: bool,
}

/// `move_selection` gives the row selected after `input` among `row_count` rows (wrapping), or
/// `selected` for any input that doesn't move.
///
/// Example: `self.selected = move_selection(self.selected, &input, rows.len());`
pub fn move_selection(selected: usize, input: &MenuInput, row_count: usize) -> usize {
    if row_count == 0 {
        return 0;
    }
    let selected: usize = selected.min(row_count - 1);
    match input {
        MenuInput::Up => (selected + row_count - 1) % row_count,
        MenuInput::Down => (selected + 1) % row_count,
        _ => selected,
    }
}

/// `get_editor_input` gives the settings editor's input for a menu input.
///
/// Example: `editor.handle(get_editor_input(input), &mut values, &defaults, &check);`
pub fn get_editor_input(input: MenuInput) -> EditorInput {
    match input {
        MenuInput::Up => EditorInput::Up,
        MenuInput::Down => EditorInput::Down,
        MenuInput::Left => EditorInput::Left,
        MenuInput::Right => EditorInput::Right,
        MenuInput::Activate => EditorInput::Activate,
        MenuInput::Back => EditorInput::Back,
        MenuInput::Reset => EditorInput::Reset,
        MenuInput::Typed(c) => EditorInput::Typed(c),
        MenuInput::Erase => EditorInput::Erase,
        MenuInput::Control(name) => EditorInput::Control(name),
    }
}

/// `push_title` adds a page's backdrop of `size`, its title and its one-line help at the bottom.
///
/// Example: `push_title(&mut shapes, [640.0, 455.0], "WARPED", "Up/Down choose, Return starts");`
pub fn push_title(shapes: &mut Vec<Shape>, size: [f32; 2], title: &str, help: &str) {
    shapes.push(Shape::Panel {
        min: [0.0, 0.0],
        size,
        rgba: [8, 10, 16, 255],
        border_rgb: None,
    });
    push_text(shapes, [MARGIN, 12.0], title, TITLE_RGB);
    push_text(shapes, [MARGIN, size[1] - LINE - 4.0], help, DIM_RGB);
}

/// `push_rows` adds `rows` from `top`, one `LINE` apart: the `selected` one (counting only the
/// rows, not the gaps) on a panel, dimmed rows that can't be chosen, and a blank line for each
/// `None`.
///
/// Example: `push_rows(&mut shapes, 40.0, &rows, 2);`
pub fn push_rows(
    shapes: &mut Vec<Shape>,
    top: f32,
    rows: &[Option<MenuRow>],
    selected: Option<usize>,
) {
    let mut choosable_index: usize = 0;
    for (i, row) in rows.iter().enumerate() {
        let y: f32 = top + LINE * i as f32;
        match row {
            None => {}
            Some(row) => {
                let is_selected: bool = selected == Some(choosable_index);
                if is_selected {
                    shapes.push(Shape::Panel {
                        min: [MARGIN - 6.0, y - ROW_PAD],
                        size: [PAGE_WIDTH - 2.0 * MARGIN + 12.0, GLYPH + 2.0 * ROW_PAD],
                        rgba: SELECTED_RGBA,
                        border_rgb: Some(HIGHLIGHT_RGB),
                    });
                }
                let rgb: [u8; 3] = if row.is_enabled { HUD_RGB } else { DIM_RGB };
                push_text(shapes, [MARGIN, y], row.text.clone(), rgb);
                choosable_index += 1;
            }
        }
    }
}

/// `push_heading` adds a section heading at `at`.
///
/// Example: `push_heading(&mut shapes, [20.0, 40.0], "GAMES ON THIS NETWORK");`
pub fn push_heading(shapes: &mut Vec<Shape>, at: [f32; 2], text: &str) {
    push_text(shapes, at, text, HIGHLIGHT_RGB);
}

#[cfg(test)]
mod tests {
    //! Unit tests for `menu_page.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Selection | down, up, wrapping at both ends, no rows, a selection past the rows, other inputs | `selection_moves_and_wraps` |
    //! | Rows | the selected row on a panel (counted past a gap), a dimmed row, a gap | `rows_draw_the_selection_and_dim_disabled_rows` |

    use super::*;

    #[test]
    fn selection_moves_and_wraps() {
        assert_eq!(move_selection(0, &MenuInput::Down, 3), 1);
        assert_eq!(move_selection(2, &MenuInput::Down, 3), 0);
        assert_eq!(move_selection(0, &MenuInput::Up, 3), 2);
        assert_eq!(move_selection(0, &MenuInput::Up, 0), 0);
        assert_eq!(move_selection(7, &MenuInput::Left, 3), 2);
    }

    #[test]
    fn rows_draw_the_selection_and_dim_disabled_rows() {
        let rows: Vec<Option<MenuRow>> = vec![
            Some(MenuRow {
                text: "a".to_string(),
                is_enabled: true,
            }),
            None,
            Some(MenuRow {
                text: "b".to_string(),
                is_enabled: false,
            }),
        ];
        let mut shapes: Vec<Shape> = Vec::new();
        push_rows(&mut shapes, 40.0, &rows, Some(1));
        assert!(
            matches!(shapes[1], Shape::Panel { min, .. } if min[1] == 40.0 + 2.0 * LINE - ROW_PAD)
        );
        assert_eq!(
            shapes[2],
            Shape::Text {
                at: [MARGIN, 40.0 + 2.0 * LINE],
                text: "b".to_string(),
                rgb: DIM_RGB
            }
        );
    }
}
