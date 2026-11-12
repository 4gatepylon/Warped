//! `minimap` draws the level from above for the HUD: a round minimap of the forest around the
//! player, turned so they face up, and the menu's map page, the whole forest north up. Both show
//! marks (robots and players in team colours, consoles) and the player as a white arrow.
//!
//! Interface: `Minimap` (`New`, `draw_minimap`, `draw_map`, `get_minimap_image`, `get_map_image`),
//! `Mark`.
//!
//! Example: `minimap.draw_minimap(feet_m, yaw_rad, &marks); let image = minimap.get_minimap_image(1600.0);`
//!
//! Must know:
//! - The ground picture is drawn once from the arena (decks, bridges, trunks, ladders, rocks;
//!   canopies are left out so the walkways show); each frame only samples it, about 40,000
//!   pixels, with no allocation.
//! - `load_photo` swaps the map page's drawing for the photo-like picture `make map-picture`
//!   saves, at `MAP_PICTURE_PX_PER_M`; without it the map page shows the drawing.
//! - Map pixels run along +x to the right and +z down, so red's base (positive z) is at the
//!   bottom of the map page.

use std::path::Path;

use glam::{Vec2, Vec3};

use crate::game::world::{Arena, ShapeKind, Surface};
use crate::render::HudImage;

/// `MAP_PICTURE_PX_PER_M` is the resolution of the map page's photo-like picture.
pub const MAP_PICTURE_PX_PER_M: f32 = 10.0;
/// `GROUND_PX_PER_M` is the ground picture's resolution.
const GROUND_PX_PER_M: f32 = 4.0;
/// `MINIMAP_SIZE_PX` is the minimap's diameter in its own pixels.
const MINIMAP_SIZE_PX: usize = 200;
/// `MINIMAP_RANGE_M` is how far from the player the minimap's rim is.
const MINIMAP_RANGE_M: f32 = 26.0;
/// `MINIMAP_WIDTH_FRAC` is the minimap's diameter as a fraction of the window's height.
const MINIMAP_WIDTH_FRAC: f32 = 0.2;
const MINIMAP_MARGIN_FRAC: f32 = 0.02;
const RIM_PX: f32 = 2.0;
const ARROW_PX: f32 = 7.0;
const MAP_HEIGHT_FRAC: f32 = 0.62;
const MAP_TOP_FRAC: f32 = 0.34;
const MAP_ALPHA: u8 = 235;
const BYTES_PER_PIXEL: usize = 4;
const GROUND_RGBA: [u8; 4] = [46, 62, 34, 255];
const OUTSIDE_RGBA: [u8; 4] = [12, 14, 12, 255];
const RIM_RGBA: [u8; 4] = [225, 230, 210, 255];
const ARROW_RGBA: [u8; 4] = [255, 255, 255, 255];
const LADDER_RGBA: [u8; 4] = [235, 205, 110, 255];
const MIN_STRUCTURE_HEIGHT_M: f32 = 0.05;

/// `Mark` is a dot on the minimap and map: where it stands, its colour and radius in pixels.
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    pub position_m: Vec3,
    pub rgba: [u8; 4],
    pub radius_px: f32,
}

/// `Minimap` is the forest's ground picture and the minimap and map pictures drawn from it.
pub struct Minimap {
    ground_rgba: Vec<u8>,
    ground_size_px: (usize, usize),
    min_m: Vec2,
    minimap_rgba: Vec<u8>,
    map_rgba: Vec<u8>,
    size_m: Vec2,
    photo: Option<(Vec<u8>, (usize, usize))>,
}

/// `get_surface_rgba` gives the colour a surface has seen from above, or `None` for surfaces the
/// map leaves out (ground and leaves).
///
/// Example: `get_surface_rgba(Surface::Planks)`
fn get_surface_rgba(surface: Surface) -> Option<[u8; 4]> {
    match surface {
        Surface::Ground | Surface::Leaves => None,
        Surface::Bark => Some([92, 64, 42, 255]),
        Surface::Planks => Some([168, 128, 82, 255]),
        Surface::Rock => Some([112, 116, 106, 255]),
        Surface::Rope => Some([200, 180, 130, 255]),
        Surface::RedPaint | Surface::RedConsole => Some([190, 50, 40, 255]),
        Surface::BluePaint | Surface::BlueConsole => Some([50, 90, 200, 255]),
        Surface::Horizon => Some([8, 8, 8, 255]),
        Surface::Throat => Some([150, 100, 255, 255]),
        Surface::Accretion | Surface::Wax | Surface::Flame => None,
    }
}

/// `fill_disc` paints the pixels of `rgba` (`width_px` wide) within `radius_px` of `centre_px`.
///
/// Example: `fill_disc(&mut pixels, 200, Vec2::splat(100.0), 3.0, [255; 4]);`
fn fill_disc(rgba: &mut [u8], width_px: usize, centre_px: Vec2, radius_px: f32, color: [u8; 4]) {
    let height_px: usize = rgba.len() / BYTES_PER_PIXEL / width_px;
    let min_x: usize = (centre_px.x - radius_px).floor().max(0.0) as usize;
    let min_y: usize = (centre_px.y - radius_px).floor().max(0.0) as usize;
    let max_x: usize = ((centre_px.x + radius_px).ceil().max(0.0) as usize).min(width_px);
    let max_y: usize = ((centre_px.y + radius_px).ceil().max(0.0) as usize).min(height_px);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let offset: Vec2 = Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - centre_px;
            if offset.length_squared() <= radius_px * radius_px {
                let i: usize = (y * width_px + x) * BYTES_PER_PIXEL;
                rgba[i..i + BYTES_PER_PIXEL].copy_from_slice(&color);
            }
        }
    }
}

/// `fill_rect` paints the pixels of `rgba` (`width_px` wide) between two corners.
///
/// Example: `fill_rect(&mut pixels, 200, Vec2::ZERO, Vec2::splat(4.0), [255; 4]);`
fn fill_rect(rgba: &mut [u8], width_px: usize, min_px: Vec2, max_px: Vec2, color: [u8; 4]) {
    let height_px: usize = rgba.len() / BYTES_PER_PIXEL / width_px;
    let min_x: usize = min_px.x.round().max(0.0) as usize;
    let min_y: usize = min_px.y.round().max(0.0) as usize;
    let max_x: usize =
        (max_px.x.round().max(min_px.x.round() + 1.0).max(0.0) as usize).min(width_px);
    let max_y: usize =
        (max_px.y.round().max(min_px.y.round() + 1.0).max(0.0) as usize).min(height_px);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let i: usize = (y * width_px + x) * BYTES_PER_PIXEL;
            rgba[i..i + BYTES_PER_PIXEL].copy_from_slice(&color);
        }
    }
}

/// `fill_arrow` paints a triangle at `centre_px` pointing along `forward_px` (a unit vector).
///
/// Example: `fill_arrow(&mut pixels, 200, Vec2::splat(100.0), Vec2::NEG_Y);`
fn fill_arrow(rgba: &mut [u8], width_px: usize, centre_px: Vec2, forward_px: Vec2) {
    let side_px: Vec2 = forward_px.perp();
    let tip: Vec2 = centre_px + forward_px * ARROW_PX;
    let left: Vec2 = centre_px - forward_px * ARROW_PX * 0.7 + side_px * ARROW_PX * 0.7;
    let right: Vec2 = centre_px - forward_px * ARROW_PX * 0.7 - side_px * ARROW_PX * 0.7;
    let cross = |a: Vec2, b: Vec2, p: Vec2| (b - a).perp_dot(p - a);
    let height_px: usize = rgba.len() / BYTES_PER_PIXEL / width_px;
    let min: Vec2 = tip.min(left).min(right).floor().max(Vec2::ZERO);
    let max: Vec2 = tip.max(left).max(right).ceil();
    for y in min.y as usize..(max.y as usize).min(height_px) {
        for x in min.x as usize..(max.x as usize).min(width_px) {
            let p: Vec2 = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let signs: [f32; 3] = [
                cross(tip, left, p),
                cross(left, right, p),
                cross(right, tip, p),
            ];
            if signs.iter().all(|&s| s >= 0.0) || signs.iter().all(|&s| s <= 0.0) {
                let i: usize = (y * width_px + x) * BYTES_PER_PIXEL;
                rgba[i..i + BYTES_PER_PIXEL].copy_from_slice(&ARROW_RGBA);
            }
        }
    }
}

/// `get_forward_xz` gives the ground direction a player with `yaw_rad` faces, as (x, z).
///
/// Example: `get_forward_xz(0.0) == Vec2::new(0.0, -1.0)`
fn get_forward_xz(yaw_rad: f32) -> Vec2 {
    Vec2::new(-yaw_rad.sin(), -yaw_rad.cos())
}

impl Minimap {
    /// `New` draws the arena's ground picture from above.
    ///
    /// Example: `let minimap = Minimap::New(&arena);`
    #[allow(non_snake_case)]
    pub fn New(arena: &Arena) -> Minimap {
        let min_m: Vec2 = Vec2::new(arena.bounds_min_m.x, arena.bounds_min_m.z);
        let size_m: Vec2 = Vec2::new(arena.bounds_max_m.x, arena.bounds_max_m.z) - min_m;
        let ground_size_px: (usize, usize) = (
            (size_m.x * GROUND_PX_PER_M).ceil() as usize,
            (size_m.y * GROUND_PX_PER_M).ceil() as usize,
        );
        let width_px: usize = ground_size_px.0;
        let mut ground_rgba: Vec<u8> = GROUND_RGBA.repeat(ground_size_px.0 * ground_size_px.1);
        let to_px = |x_m: f32, z_m: f32| (Vec2::new(x_m, z_m) - min_m) * GROUND_PX_PER_M;
        let mut footprints: Vec<(f32, [u8; 4], Vec2, Vec2, bool)> = Vec::new();
        for world_box in &arena.boxes {
            let (min, max) = (world_box.aabb.min_m, world_box.aabb.max_m);
            if let Some(color) = get_surface_rgba(world_box.surface)
                && max.y > MIN_STRUCTURE_HEIGHT_M
            {
                footprints.push((
                    max.y,
                    color,
                    to_px(min.x, min.z),
                    to_px(max.x, max.z),
                    false,
                ));
            }
        }
        for shape in &arena.shapes {
            let Some(color) = get_surface_rgba(shape.surface) else {
                continue;
            };
            let top_m: f32 = shape.centre_m.y + shape.half_m.y;
            let (min, max) = (shape.centre_m - shape.half_m, shape.centre_m + shape.half_m);
            match shape.kind {
                ShapeKind::Cylinder { axis: 1 } => footprints.push((
                    top_m,
                    color,
                    to_px(shape.centre_m.x, shape.centre_m.z),
                    Vec2::splat(shape.half_m.x * GROUND_PX_PER_M),
                    true,
                )),
                ShapeKind::Cylinder { .. } => {
                    footprints.push((
                        top_m,
                        color,
                        to_px(min.x, min.z),
                        to_px(max.x, max.z),
                        false,
                    ));
                }
                ShapeKind::Ellipsoid => {}
            }
        }
        for ladder in &arena.ladders {
            let (min, max) = (ladder.aabb.min_m, ladder.aabb.max_m);
            footprints.push((
                max.y,
                LADDER_RGBA,
                to_px(min.x, min.z),
                to_px(max.x, max.z),
                false,
            ));
        }
        footprints.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, color, a, b, is_disc) in footprints {
            match is_disc {
                true => fill_disc(&mut ground_rgba, width_px, a, b.x.max(1.0), color),
                false => fill_rect(&mut ground_rgba, width_px, a, b, color),
            }
        }
        Minimap {
            map_rgba: ground_rgba.clone(),
            ground_rgba,
            ground_size_px,
            min_m,
            minimap_rgba: vec![0; MINIMAP_SIZE_PX * MINIMAP_SIZE_PX * BYTES_PER_PIXEL],
            size_m,
            photo: None,
        }
    }

    /// `load_photo` makes the map page show the picture at `path` (from `make map-picture`) under
    /// its marks instead of the drawing, if the picture exists.
    ///
    /// Example: `minimap.load_photo(Path::new("assets/map_picture.png"));`
    pub fn load_photo(&mut self, path: &Path) {
        if !path.exists() {
            return;
        }
        let picture: image::RgbaImage = image::open(path)
            .unwrap_or_else(|error| {
                panic!("can't read the map picture {}: {error}", path.display())
            })
            .to_rgba8();
        let size_px: (usize, usize) = (picture.width() as usize, picture.height() as usize);
        self.map_rgba = vec![0; size_px.0 * size_px.1 * BYTES_PER_PIXEL];
        self.photo = Some((picture.into_raw(), size_px));
    }

    /// `draw_minimap` draws the round minimap around `feet_m`, turned so `yaw_rad` faces up, with
    /// `marks` and the player's arrow.
    ///
    /// Example: `minimap.draw_minimap(player.feet_m, player.yaw_rad, &marks);`
    pub fn draw_minimap(&mut self, feet_m: Vec3, yaw_rad: f32, marks: &[Mark]) {
        let radius_px: f32 = MINIMAP_SIZE_PX as f32 / 2.0;
        let m_per_px: f32 = MINIMAP_RANGE_M / (radius_px - RIM_PX);
        let forward: Vec2 = get_forward_xz(yaw_rad);
        let right: Vec2 = Vec2::new(-forward.y, forward.x);
        let feet_xz: Vec2 = Vec2::new(feet_m.x, feet_m.z);
        let (ground_width_px, ground_height_px) = self.ground_size_px;
        for y in 0..MINIMAP_SIZE_PX {
            for x in 0..MINIMAP_SIZE_PX {
                let offset_px: Vec2 =
                    Vec2::new(x as f32 + 0.5 - radius_px, y as f32 + 0.5 - radius_px);
                let distance_px: f32 = offset_px.length();
                let color: [u8; 4] = if distance_px > radius_px {
                    [0; 4]
                } else if distance_px > radius_px - RIM_PX {
                    RIM_RGBA
                } else {
                    let world: Vec2 =
                        feet_xz + (right * offset_px.x - forward * offset_px.y) * m_per_px;
                    let ground_px: Vec2 = (world - self.min_m) * GROUND_PX_PER_M;
                    if ground_px.x < 0.0
                        || ground_px.y < 0.0
                        || ground_px.x as usize >= ground_width_px
                        || ground_px.y as usize >= ground_height_px
                    {
                        OUTSIDE_RGBA
                    } else {
                        let i: usize = (ground_px.y as usize * ground_width_px
                            + ground_px.x as usize)
                            * BYTES_PER_PIXEL;
                        let mut ground: [u8; 4] = [0; 4];
                        ground.copy_from_slice(&self.ground_rgba[i..i + BYTES_PER_PIXEL]);
                        ground[3] = MAP_ALPHA;
                        ground
                    }
                };
                let i: usize = (y * MINIMAP_SIZE_PX + x) * BYTES_PER_PIXEL;
                self.minimap_rgba[i..i + BYTES_PER_PIXEL].copy_from_slice(&color);
            }
        }
        let inner_px: f32 = radius_px - RIM_PX;
        for mark in marks {
            let relative: Vec2 = Vec2::new(mark.position_m.x, mark.position_m.z) - feet_xz;
            let screen_px: Vec2 = Vec2::new(relative.dot(right), -relative.dot(forward)) / m_per_px;
            if screen_px.length() < inner_px - mark.radius_px {
                fill_disc(
                    &mut self.minimap_rgba,
                    MINIMAP_SIZE_PX,
                    screen_px + radius_px,
                    mark.radius_px,
                    mark.rgba,
                );
            }
        }
        fill_arrow(
            &mut self.minimap_rgba,
            MINIMAP_SIZE_PX,
            Vec2::splat(radius_px),
            Vec2::NEG_Y,
        );
    }

    /// `draw_map` draws the whole forest north up with `marks` and the player's arrow at
    /// `feet_m` facing `yaw_rad`, for the menu's map page.
    ///
    /// Example: `minimap.draw_map(player.feet_m, player.yaw_rad, &marks);`
    pub fn draw_map(&mut self, feet_m: Vec3, yaw_rad: f32, marks: &[Mark]) {
        let (base_rgba, width_px, px_per_m): (&[u8], usize, f32) = match &self.photo {
            Some((rgba, size_px)) => (rgba, size_px.0, size_px.0 as f32 / self.size_m.x),
            None => (&self.ground_rgba, self.ground_size_px.0, GROUND_PX_PER_M),
        };
        self.map_rgba.copy_from_slice(base_rgba);
        let mark_scale: f32 = px_per_m / GROUND_PX_PER_M;
        let to_px = |p: Vec3| (Vec2::new(p.x, p.z) - self.min_m) * px_per_m;
        for mark in marks {
            fill_disc(
                &mut self.map_rgba,
                width_px,
                to_px(mark.position_m),
                mark.radius_px * mark_scale,
                mark.rgba,
            );
        }
        fill_arrow(
            &mut self.map_rgba,
            width_px,
            to_px(feet_m),
            get_forward_xz(yaw_rad) * mark_scale,
        );
    }

    /// `get_minimap_image` places the minimap in the top right corner of a window
    /// `window_width_px` wide and `window_height_px` high.
    ///
    /// Example: `let image = minimap.get_minimap_image(1600.0, 1200.0);`
    pub fn get_minimap_image(&self, window_width_px: f32, window_height_px: f32) -> HudImage<'_> {
        let scale: f32 = window_height_px * MINIMAP_WIDTH_FRAC / MINIMAP_SIZE_PX as f32;
        let margin_px: f32 = window_height_px * MINIMAP_MARGIN_FRAC;
        HudImage {
            pixels_rgba: &self.minimap_rgba,
            width_px: MINIMAP_SIZE_PX as u32,
            height_px: MINIMAP_SIZE_PX as u32,
            x_px: window_width_px - margin_px - MINIMAP_SIZE_PX as f32 * scale,
            y_px: margin_px,
            scale,
        }
    }

    /// `get_map_image` places the map page's picture in the middle of a window, below the menu's
    /// first lines.
    ///
    /// Example: `let image = minimap.get_map_image(1600.0, 1200.0);`
    pub fn get_map_image(&self, window_width_px: f32, window_height_px: f32) -> HudImage<'_> {
        let (width_px, height_px) = self
            .photo
            .as_ref()
            .map_or(self.ground_size_px, |(_, size_px)| *size_px);
        let scale: f32 = window_height_px * MAP_HEIGHT_FRAC / height_px as f32;
        HudImage {
            pixels_rgba: &self.map_rgba,
            width_px: width_px as u32,
            height_px: height_px as u32,
            x_px: (window_width_px - width_px as f32 * scale) / 2.0,
            y_px: window_height_px * MAP_TOP_FRAC,
            scale,
        }
    }
}
