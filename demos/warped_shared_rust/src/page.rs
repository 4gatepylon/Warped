//! `page.rs` is a menu page drawn in the menus' 8-unit debug font: the `Shape`s it is made of (in
//! page units, scaled to fit the window), and, on macOS, `draw_shapes`, which draws them with an
//! SDL canvas (a window's, or a surface's for a picture without a window).
//!
//! Interface: `Shape`, `GLYPH`, `push_text`, `get_page_point`, `Thumbnail`, `make_thumbnail`,
//! `draw_shapes` (macOS).
//!
//! Example: `draw_shapes(&mut canvas, size_px, [640.0, 455.0], &shapes, &thumbnails)?;`
//!
//! Must know:
//! - A `Thumbnail` shape names a picture by its index in the `thumbnails` given to `draw_shapes`;
//!   a missing picture draws as a dark box.

/// `GLYPH` is the width and height of one character of the debug font, in page units.
pub const GLYPH: f32 = 8.0;
#[cfg(target_os = "macos")]
const THUMBNAIL_EMPTY_ALPHA: u8 = 140;
#[cfg(target_os = "macos")]
const BYTES_PER_PIXEL: usize = 4;

/// `Shape` is one thing on a page: a filled panel with an optional border, a line of text in the
/// menu's font, or the box a picture fits in (`level`: its index in the page's pictures), all in
/// page units.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Panel {
        min: [f32; 2],
        size: [f32; 2],
        rgba: [u8; 4],
        border_rgb: Option<[u8; 3]>,
    },
    Text {
        at: [f32; 2],
        text: String,
        rgb: [u8; 3],
    },
    Thumbnail {
        level: usize,
        min: [f32; 2],
        size: [f32; 2],
    },
}

/// `push_text` adds a line of text to `shapes`.
///
/// Example: `push_text(&mut shapes, [20.0, 12.0], "WARPED", [235, 240, 250]);`
pub fn push_text(shapes: &mut Vec<Shape>, at: [f32; 2], text: impl Into<String>, rgb: [u8; 3]) {
    shapes.push(Shape::Text {
        at,
        text: text.into(),
        rgb,
    });
}

/// `get_page_point` gives where `point` (in the same units as `size`, e.g. a click in window
/// points) falls on a page of `page_size` units drawn to fit `size` and centred, as
/// `draw_shapes` draws it.
///
/// Example: `let at = get_page_point([640.0, 400.0], [1280.0, 800.0], [640.0, 455.0]);`
pub fn get_page_point(point: [f32; 2], size: [f32; 2], page_size: [f32; 2]) -> [f32; 2] {
    let scale: f32 = (size[0] / page_size[0]).min(size[1] / page_size[1]);
    [0, 1].map(|a| point[a] / scale - (size[a] / scale - page_size[a]) / 2.0)
}

/// `Thumbnail` is a picture on a page: its texture and its size in pixels.
#[cfg(target_os = "macos")]
pub struct Thumbnail<'creator> {
    pub texture: sdl3::render::Texture<'creator>,
    pub size_px: (u32, u32),
}

/// `make_thumbnail` makes a `Thumbnail` from `creator` of an RGBA picture (rows top first).
///
/// Example: `let thumbnail = make_thumbnail(&creator, &pixels, (320, 200))?;`
#[cfg(target_os = "macos")]
pub fn make_thumbnail<'creator, T>(
    creator: &'creator sdl3::render::TextureCreator<T>,
    rgba: &[u8],
    size_px: (u32, u32),
) -> Result<Thumbnail<'creator>, Box<dyn std::error::Error>> {
    let mut texture: sdl3::render::Texture =
        creator.create_texture_static(sdl3::pixels::PixelFormat::RGBA32, size_px.0, size_px.1)?;
    texture.update(None, rgba, size_px.0 as usize * BYTES_PER_PIXEL)?;
    Ok(Thumbnail { texture, size_px })
}

/// `draw_shapes` clears the canvas to black and draws `shapes`, scaling the page of `page_size`
/// units to fit `size_px` and centring it; each `Shape::Thumbnail` draws its picture from
/// `thumbnails`, fitted into its box.
///
/// Example: `draw_shapes(&mut canvas, (2560, 1440), [640.0, 455.0], &shapes, &[])?;`
#[cfg(target_os = "macos")]
pub fn draw_shapes<T: sdl3::render::RenderTarget>(
    canvas: &mut sdl3::render::Canvas<T>,
    (width, height): (u32, u32),
    page_size: [f32; 2],
    shapes: &[Shape],
    thumbnails: &[Option<Thumbnail>],
) -> Result<(), Box<dyn std::error::Error>> {
    use sdl3::pixels::Color;
    use sdl3::render::FRect;
    let [page_width, page_height] = page_size;
    let scale: f32 = (width as f32 / page_width).min(height as f32 / page_height);
    let left: f32 = (width as f32 / scale - page_width) / 2.0;
    let top: f32 = (height as f32 / scale - page_height) / 2.0;
    canvas.set_scale(1.0, 1.0)?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.set_scale(scale, scale)?;
    canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
    for shape in shapes {
        match shape {
            Shape::Panel {
                min,
                size,
                rgba,
                border_rgb,
            } => {
                let rect: FRect = FRect::new(left + min[0], top + min[1], size[0], size[1]);
                canvas.set_draw_color(Color::RGBA(rgba[0], rgba[1], rgba[2], rgba[3]));
                canvas.fill_rect(rect)?;
                if let Some([r, g, b]) = border_rgb {
                    canvas.set_draw_color(Color::RGB(*r, *g, *b));
                    canvas.draw_rect(rect)?;
                }
            }
            Shape::Text { at, text, rgb } => {
                canvas.set_draw_color(Color::RGB(rgb[0], rgb[1], rgb[2]));
                canvas.draw_debug_text(text.as_str(), (left + at[0], top + at[1]))?;
            }
            Shape::Thumbnail { level, min, size } => {
                let Some(Some(Thumbnail {
                    texture,
                    size_px: (width_px, height_px),
                })) = thumbnails.get(*level)
                else {
                    canvas.set_draw_color(Color::RGBA(0, 0, 0, THUMBNAIL_EMPTY_ALPHA));
                    canvas.fill_rect(FRect::new(left + min[0], top + min[1], size[0], size[1]))?;
                    continue;
                };
                let fit: f32 = (size[0] / *width_px as f32).min(size[1] / *height_px as f32);
                let [fit_width, fit_height] = [*width_px as f32 * fit, *height_px as f32 * fit];
                canvas.copy(
                    texture,
                    None,
                    FRect::new(
                        left + min[0] + (size[0] - fit_width) / 2.0,
                        top + min[1] + (size[1] - fit_height) / 2.0,
                        fit_width,
                        fit_height,
                    ),
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for `page.rs`.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Window shape | wider than the page (bars left and right), taller (bars above and below) | `points_map_back_to_the_page` |

    use super::*;

    #[test]
    fn points_map_back_to_the_page() {
        let page: [f32; 2] = [640.0, 455.0];
        assert_eq!(
            get_page_point([1280.0, 0.0], [2560.0, 910.0], page),
            [320.0, 0.0]
        );
        assert_eq!(
            get_page_point([0.0, 455.0], [640.0, 1365.0], page),
            [0.0, 0.0]
        );
        assert_eq!(
            get_page_point([640.0, 910.0], [640.0, 1365.0], page),
            [640.0, 455.0]
        );
    }
}
