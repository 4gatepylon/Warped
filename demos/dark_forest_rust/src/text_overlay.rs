//! `text_overlay` draws lines of text with SDL's built-in 8x8 font into an RGBA picture for the
//! HUD (`HudImage`): the menu and the title screen. It redraws only when the lines change.
//!
//! Interface: `TextPicture` (`New`, `draw`, `get_image`), `GLYPH_PX`.
//!
//! Example: `picture.draw(&[("MENU".to_string(), true)], false); let image = picture.get_image(1600, 1200, 0.85);`
//!
//! Must know:
//! - The picture is drawn at a whole number of window pixels per picture pixel, so the font stays
//!   sharp; `get_image` picks the largest that fits.

use sdl3::pixels::{Color, PixelFormat};
use sdl3::surface::Surface;

use crate::render::HudImage;

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
const BYTES_PER_PIXEL: usize = 4;

/// `TextPicture` is a picture of some lines of text and the lines it shows.
pub struct TextPicture {
    pub pixels_rgba: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
    shown_lines: Option<(Vec<(String, bool)>, bool)>,
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
                HIGHLIGHT_COLOR
            } else {
                TEXT_COLOR
            });
            canvas
                .draw_debug_text(shown.as_str(), (x_px, TOP_PX + LINE_PX * i as f32))
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
