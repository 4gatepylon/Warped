//! `text_overlay` draws lines of text with SDL's built-in 8x8 font into an RGBA picture for the
//! HUD (`HudImage`): the menu and the main menu. It redraws only when the lines change.
//!
//! Interface: `TextPicture` (`New`, `draw`, `draw_team_box`, `get_image`, `get_layout`),
//! `GLYPH_PX`.
//!
//! Example: `picture.draw(&[("MENU".to_string(), true)], false); let image = picture.get_image(1600, 1200, 0.85);`
//!
//! Must know:
//! - The picture is drawn at a whole number of window pixels per picture pixel, so the font stays
//!   sharp; `get_image` picks the largest that fits.

use sdl3::pixels::{Color, PixelFormat};
use sdl3::surface::Surface;

use crate::render::HudImage;
use warped_shared::hud::make_team_box_rgba;
use warped_shared::menu_and_map::TextLayout;

/// `GLYPH_PX` is the width and height of one character of SDL's debug font.
pub const GLYPH_PX: f32 = 8.0;
/// `LINE_PX` is the distance between the tops of two lines.
const LINE_PX: f32 = 12.0;
/// `TOP_PX` is the space above the first line.
const TOP_PX: f32 = 6.0;
/// `LEFT_PX` is where left-aligned lines start.
const LEFT_PX: f32 = 8.0;
/// `MAX_WIDTH_FRAC` is the most of the window's width the picture may take.
const MAX_WIDTH_FRAC: f32 = 0.95;
const TEXT_COLOR: Color = Color::RGB(235, 240, 250);
const HIGHLIGHT_COLOR: Color = Color::RGB(255, 196, 92);
/// `BANNER_TITLE_COLOR` and `BANNER_TOP_PX` are the Lunar Surface demo's banner ("YOU DIED"): its
/// title colour (`OVERLAY_TITLE_COLOR`) and the space above its first line (`TEXT_TOP_PX`).
const BANNER_TITLE_COLOR: Color = Color::RGB(255, 89, 77);
const BANNER_TOP_PX: f32 = 2.0;
const BYTES_PER_PIXEL: usize = 4;
/// `TEAM_BOX_PAD_PX` is how far the team box reaches past its lines' text.
const TEAM_BOX_PAD_PX: u32 = 2;

/// `TextPicture` is a picture of some lines of text and the lines it shows.
pub struct TextPicture {
    pub pixels_rgba: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    shown_lines: Option<(Vec<(String, bool)>, bool)>,
    title_color: Color,
    top_px: f32,
}

impl TextPicture {
    /// `New` makes an empty picture of `width_px` by `height_px`.
    ///
    /// Example: `let picture = TextPicture::New(512, 312);`
    #[allow(non_snake_case)]
    pub fn New(width_px: u32, height_px: u32) -> TextPicture {
        TextPicture {
            pixels_rgba: vec![0; width_px as usize * height_px as usize * BYTES_PER_PIXEL],
            width_px,
            height_px,
            shown_lines: None,
            title_color: HIGHLIGHT_COLOR,
            top_px: TOP_PX,
        }
    }

    /// `Banner` is an empty picture of `width_px` by `height_px` in the Lunar Surface demo's banner
    /// style ("YOU DIED"): its highlighted lines in the banner's red, from 2 px down.
    ///
    /// Example: `let banner = TextPicture::Banner(240, 24);`
    #[allow(non_snake_case)]
    pub fn Banner(width_px: u32, height_px: u32) -> TextPicture {
        TextPicture {
            title_color: BANNER_TITLE_COLOR,
            top_px: BANNER_TOP_PX,
            ..TextPicture::New(width_px, height_px)
        }
    }

    /// `draw` draws `lines` (each with whether it is highlighted), centred or from the left, cut
    /// to the picture's width; it does nothing if they are the lines already shown.
    ///
    /// Example: `picture.draw(&lines, true);`
    pub fn draw(&mut self, lines: &[(String, bool)], is_centered: bool) {
        if self
            .shown_lines
            .as_ref()
            .is_some_and(|(shown, was_centered)| shown == lines && *was_centered == is_centered)
        {
            return;
        }
        let surface: Surface = Surface::new(self.width_px, self.height_px, PixelFormat::RGBA32)
            .expect("SDL surface for text");
        let mut canvas = surface.into_canvas().expect("SDL canvas for text");
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 0));
        canvas.clear();
        let max_char_count: usize = (self.width_px as f32 / GLYPH_PX) as usize;
        for (i, (line, is_highlighted)) in lines.iter().enumerate() {
            let shown: String = line.chars().take(max_char_count).collect();
            let x_px: f32 = if is_centered {
                (self.width_px as f32 - GLYPH_PX * shown.chars().count() as f32) / 2.0
            } else {
                LEFT_PX
            };
            canvas.set_draw_color(if *is_highlighted {
                self.title_color
            } else {
                TEXT_COLOR
            });
            canvas
                .draw_debug_text(shown.as_str(), (x_px, self.top_px + LINE_PX * i as f32))
                .expect("SDL debug text");
        }
        canvas.present();
        let surface: Surface = canvas.into_surface();
        let pitch_bytes: usize = surface.pitch() as usize;
        let row_bytes: usize = self.width_px as usize * BYTES_PER_PIXEL;
        let pixels_rgba: &mut Vec<u8> = &mut self.pixels_rgba;
        surface.with_lock(|pixels: &[u8]| {
            for (y, row) in pixels_rgba.chunks_exact_mut(row_bytes).enumerate() {
                row.copy_from_slice(&pixels[y * pitch_bytes..y * pitch_bytes + row_bytes]);
            }
        });
        self.shown_lines = Some((lines.to_vec(), is_centered));
    }

    /// `draw_team_box` draws a box in `team`'s colour (`make_team_box_rgba`) under the text of
    /// left-aligned lines `lines` (first, end excluded), across the picture; it fills only clear
    /// pixels, so drawing it again over the same lines changes nothing.
    ///
    /// Example: `picture.draw_team_box(0, [5, 8]);`
    pub fn draw_team_box(&mut self, team: u32, lines: [usize; 2]) {
        let [top_px, bottom_excl_px] = lines.map(|line| self.top_px + LINE_PX * line as f32);
        let min_px: [u32; 2] = [
            LEFT_PX as u32 - TEAM_BOX_PAD_PX,
            top_px as u32 - TEAM_BOX_PAD_PX,
        ];
        let max_excl_px: [u32; 2] = [
            self.width_px - LEFT_PX as u32 + TEAM_BOX_PAD_PX,
            (bottom_excl_px - LINE_PX + GLYPH_PX) as u32 + TEAM_BOX_PAD_PX,
        ];
        assert!(
            max_excl_px[1] <= self.height_px,
            "draw_team_box: lines {lines:?} end below the picture's {} px",
            self.height_px
        );
        let size_px: (u32, u32) = (max_excl_px[0] - min_px[0], max_excl_px[1] - min_px[1]);
        let box_rgba: Vec<u8> = make_team_box_rgba(size_px, team);
        for (i, pixel) in box_rgba.as_chunks::<BYTES_PER_PIXEL>().0.iter().enumerate() {
            let (x, y) = (i % size_px.0 as usize, i / size_px.0 as usize);
            let at: usize =
                ((min_px[1] as usize + y) * self.width_px as usize + min_px[0] as usize + x)
                    * BYTES_PER_PIXEL;
            if self.pixels_rgba[at + BYTES_PER_PIXEL - 1] == 0 {
                self.pixels_rgba[at..at + BYTES_PER_PIXEL].copy_from_slice(pixel);
            }
        }
    }

    /// `get_layout` gives how the picture's left-aligned text lies in a window where `get_image`
    /// places it with `height_frac`, to find the text under the mouse.
    ///
    /// Example: `let cell = picture.get_layout(0.85).get_cell_at(window_px, pointer_px);`
    pub fn get_layout(&self, height_frac: f32) -> TextLayout {
        TextLayout {
            mask_px: (self.width_px, self.height_px),
            height_frac,
            max_width_frac: f64::from(MAX_WIDTH_FRAC),
            left_px: LEFT_PX,
            top_px: self.top_px,
            line_px: LINE_PX,
            glyph_px: GLYPH_PX,
        }
    }

    /// `get_image` places the picture in the middle of a window of `window_width_px` by
    /// `window_height_px`, as large as fits in `height_frac` of its height.
    ///
    /// Example: `let image = picture.get_image(1600, 1200, 0.85);`
    pub fn get_image(
        &self,
        window_width_px: f32,
        window_height_px: f32,
        height_frac: f32,
    ) -> HudImage<'_> {
        let scale: f32 = (window_height_px * height_frac / self.height_px as f32)
            .min(window_width_px * MAX_WIDTH_FRAC / self.width_px as f32)
            .floor()
            .max(1.0);
        HudImage {
            pixels_rgba: &self.pixels_rgba,
            width_px: self.width_px,
            height_px: self.height_px,
            x_px: ((window_width_px - self.width_px as f32 * scale) / 2.0).floor(),
            y_px: ((window_height_px - self.height_px as f32 * scale) / 2.0).floor(),
            scale,
        }
    }
}

/// `tests` checks the team box drawn under a menu's lines.
///
/// | Partitioning | Part | Covered by |
/// | --- | --- | --- |
/// | pixel | clear (filled), text (kept) | `test_team_box_fills_only_clear_pixels` |
/// | drawing | once, again over the same lines (unchanged) | `test_team_box_fills_only_clear_pixels` |
#[cfg(test)]
mod tests {
    use super::*;

    /// `get_alpha` is the opacity of `picture`'s pixel at `x`, `y`.
    fn get_alpha(picture: &TextPicture, x: u32, y: u32) -> u8 {
        picture.pixels_rgba[(y * picture.width_px + x) as usize * BYTES_PER_PIXEL + 3]
    }

    #[test]
    fn test_team_box_fills_only_clear_pixels() {
        let mut picture: TextPicture = TextPicture::New(64, 48);
        let text_at: (u32, u32) = (20, 20);
        let text_index: usize =
            (text_at.1 * picture.width_px + text_at.0) as usize * BYTES_PER_PIXEL;
        picture.pixels_rgba[text_index..text_index + BYTES_PER_PIXEL]
            .copy_from_slice(&[1, 2, 3, 255]);
        picture.draw_team_box(1, [1, 2]);
        let top_px: u32 = (TOP_PX + LINE_PX) as u32 - TEAM_BOX_PAD_PX;
        assert_eq!(get_alpha(&picture, LEFT_PX as u32, top_px), u8::MAX);
        assert!(get_alpha(&picture, LEFT_PX as u32 + 4, top_px + 4) < u8::MAX);
        assert_eq!(get_alpha(&picture, LEFT_PX as u32, top_px - 1), 0);
        assert_eq!(
            picture.pixels_rgba[text_index..text_index + BYTES_PER_PIXEL],
            [1, 2, 3, 255]
        );
        let once: Vec<u8> = picture.pixels_rgba.clone();
        picture.draw_team_box(1, [1, 2]);
        assert_eq!(picture.pixels_rgba, once);
    }
}
