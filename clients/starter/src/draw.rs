//! `draw.rs` draws the lobby and the game with SDL3's renderer, in window points.
//!
//! Interface: `WINDOW_WIDTH`, `WINDOW_HEIGHT`, `Map`, `begin_frame`, `draw_text`, `draw_map`,
//! `draw_square`, `draw_laser`, `parse_color`.
//!
//! Example: `draw_text(canvas, "Esc  quit", 20.0, 20.0, TEXT_COLOR)?;`

use sdl3::pixels::Color;
use sdl3::render::{FPoint, FRect, WindowCanvas};

use crate::management::Result;

pub const WINDOW_WIDTH: u32 = 760;
pub const WINDOW_HEIGHT: u32 = 800;
pub const TEXT_COLOR: Color = Color::RGB(220, 225, 240);
pub const TEXT_LINE_HEIGHT: f32 = 24.0;
const BACKGROUND: Color = Color::RGB(8, 9, 16);
const MAP_COLOR: Color = Color::RGB(20, 22, 36);
const GRID_COLOR: Color = Color::RGB(32, 35, 54);
const BORDER_COLOR: Color = Color::RGB(80, 90, 130);
const MAP_LEFT: f32 = 20.0;
const MAP_TOP: f32 = 60.0;
const MAP_SIDE: f32 = 720.0;
const GRID_SPACING_M: f32 = 2.0;
const TEXT_SCALE: f32 = 2.0;
const CIRCLE_SEGMENT_COUNT: usize = 96;
const PHOTON_SPHERE_PER_SCHWARZSCHILD_RADIUS: f32 = 1.5;
/// `LASER_GLOW` is each pass of a laser, widest first: its offset in points, and its color.
const LASER_GLOW: [(f32, Color); 3] = [
    (4.0, Color::RGBA(255, 20, 40, 45)),
    (2.0, Color::RGBA(255, 50, 60, 140)),
    (0.0, Color::RGBA(255, 225, 225, 255)),
];

/// `Map` turns map meters into window points.
pub struct Map {
    pub size_m: f32,
}

impl Map {
    /// `to_screen` returns where a map point is in the window.
    ///
    /// Example: `let (x, y) = map.to_screen((12.0, 12.0));`
    pub fn to_screen(&self, point_m: (f32, f32)) -> (f32, f32) {
        let points_per_m: f32 = MAP_SIDE / self.size_m;
        (
            MAP_LEFT + point_m.0 * points_per_m,
            MAP_TOP + point_m.1 * points_per_m,
        )
    }

    /// `to_map` returns which map point a window point is over.
    ///
    /// Example: `let aim_m = map.to_map((mouse_x, mouse_y));`
    pub fn to_map(&self, point: (f32, f32)) -> (f32, f32) {
        let m_per_point: f32 = self.size_m / MAP_SIDE;
        (
            (point.0 - MAP_LEFT) * m_per_point,
            (point.1 - MAP_TOP) * m_per_point,
        )
    }

    /// `length` returns how many window points a map length takes.
    ///
    /// Example: `let side = map.length(0.8);`
    pub fn length(&self, length_m: f32) -> f32 {
        length_m * MAP_SIDE / self.size_m
    }
}

/// `begin_frame` clears the window and makes drawing use window points on any display.
///
/// Example: `begin_frame(&mut canvas)?;`
pub fn begin_frame(canvas: &mut WindowCanvas) -> Result<()> {
    let density: f32 = canvas.window().pixel_density();
    canvas.set_scale(density, density)?;
    canvas.set_draw_color(BACKGROUND);
    canvas.clear();
    Ok(())
}

/// `draw_text` writes one line in SDL's built-in font, ASCII only, at twice its size.
///
/// Example: `draw_text(canvas, "Esc  quit", 20.0, 20.0, TEXT_COLOR)?;`
pub fn draw_text(
    canvas: &mut WindowCanvas,
    text: &str,
    x: f32,
    y: f32,
    color: Color,
) -> Result<()> {
    let density: f32 = canvas.window().pixel_density();
    canvas.set_scale(density * TEXT_SCALE, density * TEXT_SCALE)?;
    canvas.set_draw_color(color);
    canvas.draw_debug_text(text, (x / TEXT_SCALE, y / TEXT_SCALE))?;
    canvas.set_scale(density, density)?;
    Ok(())
}

/// `draw_circle` draws a circle's outline around a window point.
///
/// Example: `draw_circle(canvas, (380.0, 420.0), 36.0, Color::RGB(255, 160, 60))?;`
fn draw_circle(
    canvas: &mut WindowCanvas,
    center: (f32, f32),
    radius: f32,
    color: Color,
) -> Result<()> {
    let points: Vec<FPoint> = (0..=CIRCLE_SEGMENT_COUNT)
        .map(|i| {
            let angle: f32 = i as f32 * std::f32::consts::TAU / CIRCLE_SEGMENT_COUNT as f32;
            FPoint::new(
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect();
    canvas.set_draw_color(color);
    canvas.draw_lines(points.as_slice())?;
    Ok(())
}

/// `draw_map` draws the map, its grid, and the black hole at its center with a soft glow.
///
/// Example: `draw_map(canvas, &map, 1.2)?;`
pub fn draw_map(canvas: &mut WindowCanvas, map: &Map, schwarzschild_radius_m: f32) -> Result<()> {
    canvas.set_draw_color(MAP_COLOR);
    canvas.fill_rect(FRect::new(MAP_LEFT, MAP_TOP, MAP_SIDE, MAP_SIDE))?;
    canvas.set_draw_color(GRID_COLOR);
    let mut grid_m: f32 = GRID_SPACING_M;
    while grid_m < map.size_m {
        let (x, y) = map.to_screen((grid_m, grid_m));
        canvas.draw_line((x, MAP_TOP), (x, MAP_TOP + MAP_SIDE))?;
        canvas.draw_line((MAP_LEFT, y), (MAP_LEFT + MAP_SIDE, y))?;
        grid_m += GRID_SPACING_M;
    }
    canvas.set_draw_color(BORDER_COLOR);
    canvas.draw_rect(FRect::new(MAP_LEFT, MAP_TOP, MAP_SIDE, MAP_SIDE))?;
    let center: (f32, f32) = map.to_screen((map.size_m / 2.0, map.size_m / 2.0));
    let radius: f32 = map.length(schwarzschild_radius_m);
    for ring in 1..=8 {
        let alpha: u8 = 120 - 14 * ring as u8;
        draw_circle(
            canvas,
            center,
            radius * (1.0 + 0.12 * ring as f32),
            Color::RGBA(255, 150, 60, alpha),
        )?;
    }
    let photon_sphere: f32 = radius * PHOTON_SPHERE_PER_SCHWARZSCHILD_RADIUS;
    draw_circle(
        canvas,
        center,
        photon_sphere,
        Color::RGBA(255, 220, 160, 90),
    )?;
    canvas.set_draw_color(Color::RGB(0, 0, 0));
    let mut dy: f32 = -radius;
    while dy <= radius {
        let half_width: f32 = (radius * radius - dy * dy).sqrt();
        canvas.draw_line(
            (center.0 - half_width, center.1 + dy),
            (center.0 + half_width, center.1 + dy),
        )?;
        dy += 0.5;
    }
    Ok(())
}

/// `draw_square` draws a player's square, outlined in white when `is_own` is true.
///
/// Example: `draw_square(canvas, &map, (3.0, 4.0), 0.8, color, true)?;`
pub fn draw_square(
    canvas: &mut WindowCanvas,
    map: &Map,
    center_m: (f32, f32),
    size_m: f32,
    color: Color,
    is_own: bool,
) -> Result<()> {
    let (x, y) = map.to_screen(center_m);
    let side: f32 = map.length(size_m);
    let square: FRect = FRect::new(x - side / 2.0, y - side / 2.0, side, side);
    canvas.set_draw_color(color);
    canvas.fill_rect(square)?;
    if is_own {
        canvas.set_draw_color(Color::RGB(255, 255, 255));
        canvas.draw_rect(FRect::new(
            square.x - 2.0,
            square.y - 2.0,
            side + 4.0,
            side + 4.0,
        ))?;
    }
    Ok(())
}

/// `draw_laser` draws a laser as a glowing red line with a bright tip.
///
/// Example: `draw_laser(canvas, &map, &laser.points_m)?;`
pub fn draw_laser(canvas: &mut WindowCanvas, map: &Map, points_m: &[[f32; 2]]) -> Result<()> {
    let points: Vec<FPoint> = points_m
        .iter()
        .map(|point_m| {
            let (x, y) = map.to_screen((point_m[0], point_m[1]));
            FPoint::new(x, y)
        })
        .collect();
    let Some(tip) = points.last().copied() else {
        return Ok(());
    };
    for (offset, color) in LASER_GLOW {
        canvas.set_draw_color(color);
        for (dx, dy) in [
            (0.0, 0.0),
            (offset, 0.0),
            (-offset, 0.0),
            (0.0, offset),
            (0.0, -offset),
        ] {
            let shifted: Vec<FPoint> = points
                .iter()
                .map(|point| FPoint::new(point.x + dx, point.y + dy))
                .collect();
            canvas.draw_lines(shifted.as_slice())?;
        }
    }
    canvas.set_draw_color(Color::RGBA(255, 240, 240, 255));
    canvas.fill_rect(FRect::new(tip.x - 2.5, tip.y - 2.5, 5.0, 5.0))?;
    Ok(())
}

/// `parse_color` reads a `#rrggbb` color.
///
/// Example: `let color = parse_color("#e6194b")?;`
pub fn parse_color(hex: &str) -> Result<Color> {
    let digits: &str = hex
        .strip_prefix('#')
        .ok_or("expected a color like #rrggbb")?;
    let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16);
    Ok(Color::RGB(channel(0)?, channel(2)?, channel(4)?))
}
