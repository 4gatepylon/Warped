//! `map_photo.rs` renders a voxel map seen straight down with the level's own renderer, for the
//! menu's map page: a photo-like picture with the renderer's sun, shadows, blizzards and warps,
//! made once (`make map-picture`) and saved beside the code, which `map_layout.rs` puts under its
//! marks instead of its flat drawing when the file is there.
//!
//! Interface: `render_map_photo`, `get_map_photo_path`.
//!
//! Example: `let pixels = render_map_photo(&world, renderer.as_mut(), corner_m, m_per_px, size_px)?;`
//!
//! Must know:
//! - The picture is a mosaic of tiles, each the middle of a view straight down from
//!   `CAMERA_H_M`, so it is close to a map projection: only things that stand tall (the pylons)
//!   lean a little away from their tile's middle.
//! - The distance fog and the light snowfall are off for it, so the whole map stays clear.

use std::error::Error;
use std::path::PathBuf;

use glam::{DMat4, DVec4};

use crate::geometry::{invert_lorentz, make_fermi_point};
use crate::render_voxel::{HALF_FOV_RAD, VoxelFrame};
use crate::voxel_world::{VoxelPlayer, VoxelWorld};
use crate::{FrameData, Renderer};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;
const TILE_M: f64 = 8.0;
const CAMERA_H_M: f64 = 40.0;
const SUPERSAMPLE_COUNT: usize = 2;
const NO_FOG_M: f64 = 1.0e9;
const STRAIGHT_DOWN_RAD: f64 = -std::f64::consts::FRAC_PI_2;

/// `get_map_photo_path` gives where the map called `map_name` keeps its picture from above.
///
/// Example: `let path = get_map_photo_path("snow_planet");`
pub fn get_map_photo_path(map_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("map_pictures")
        .join(format!("{map_name}.png"))
}

/// `render_map_photo` renders `world` from above as an RGBA picture (top row first) of
/// `size_px`, whose top left corner is at Fermi (u, v) `corner_m`, u to the right and v down,
/// at `m_per_px` meters per pixel, supersampled `SUPERSAMPLE_COUNT` times each way.
///
/// Example: `let pixels = render_map_photo(&world, renderer.as_mut(), [-60.0, -40.0], 0.06, (2000, 1333))?;`
pub fn render_map_photo(
    world: &VoxelWorld,
    renderer: &mut dyn Renderer,
    corner_m: [f64; 2],
    m_per_px: f64,
    size_px: (u32, u32),
) -> Result<Vec<u8>> {
    let mut clear_world: VoxelWorld = world.clone();
    clear_world.snow_fog_m = NO_FOG_M;
    clear_world.snow_per_m3 = 0.0;
    let (width, height) = (
        size_px.0 as usize * SUPERSAMPLE_COUNT,
        size_px.1 as usize * SUPERSAMPLE_COUNT,
    );
    let m_per_fine_px: f64 = m_per_px / SUPERSAMPLE_COUNT as f64;
    let tile_px: usize = (TILE_M / m_per_fine_px).ceil() as usize;
    let view_px: u32 = ((2.0 * CAMERA_H_M * HALF_FOV_RAD.tan() / TILE_M) * tile_px as f64) as u32;
    let mut view_pixels: Vec<u8> = vec![0; (view_px * view_px) as usize * BYTES_PER_PIXEL];
    let mut fine: Vec<u8> = vec![0; width * height * BYTES_PER_PIXEL];
    let tan_half_fov: f64 = HALF_FOV_RAD.tan();
    for tile_y in 0..height.div_ceil(tile_px) {
        for tile_x in 0..width.div_ceil(tile_px) {
            let x_range = tile_x * tile_px..((tile_x + 1) * tile_px).min(width);
            let y_range = tile_y * tile_px..((tile_y + 1) * tile_px).min(height);
            let center_m: [f64; 2] = [
                corner_m[0] + (x_range.start + x_range.end) as f64 / 2.0 * m_per_fine_px,
                corner_m[1] + (y_range.start + y_range.end) as f64 / 2.0 * m_per_fine_px,
            ];
            let player: VoxelPlayer = VoxelPlayer::AtView(
                &clear_world,
                center_m,
                [center_m[0], center_m[1] - TILE_M],
                Some(CAMERA_H_M),
                STRAIGHT_DOWN_RAD,
            );
            let camera: DMat4 = player.get_camera_frame(&clear_world);
            let frame: VoxelFrame =
                VoxelFrame::Build(&clear_world, camera, &[], None, &[], (view_px, view_px));
            renderer.render(
                FrameData::Voxel(&frame),
                (view_px, view_px),
                &mut view_pixels,
            )?;
            let to_camera: DMat4 = invert_lorentz(camera);
            for y in y_range.clone() {
                for x in x_range.clone() {
                    let u_m: f64 = corner_m[0] + (x as f64 + 0.5) * m_per_fine_px;
                    let v_m: f64 = corner_m[1] + (y as f64 + 0.5) * m_per_fine_px;
                    let local: DVec4 = to_camera * make_fermi_point(u_m, v_m, 0.0, clear_world.r);
                    let ndc: [f64; 2] = [
                        local.x / -local.z / tan_half_fov,
                        local.y / -local.z / tan_half_fov,
                    ];
                    let view_x: usize = (((ndc[0] + 1.0) / 2.0 * f64::from(view_px)) as usize)
                        .min(view_px as usize - 1);
                    let view_y: usize = (((1.0 - ndc[1]) / 2.0 * f64::from(view_px)) as usize)
                        .min(view_px as usize - 1);
                    let from: usize = (view_x + view_y * view_px as usize) * BYTES_PER_PIXEL;
                    let to: usize = (x + y * width) * BYTES_PER_PIXEL;
                    fine[to..to + BYTES_PER_PIXEL]
                        .copy_from_slice(&view_pixels[from..from + BYTES_PER_PIXEL]);
                }
            }
        }
    }
    Ok(downsample(&fine, width, size_px))
}

/// `downsample` averages each `SUPERSAMPLE_COUNT` × `SUPERSAMPLE_COUNT` block of the `width`
/// pixels wide RGBA picture `fine` into one pixel of a `size_px` picture.
///
/// Example: `let pixels = downsample(&fine, 4000, (2000, 1333));`
fn downsample(fine: &[u8], width: usize, size_px: (u32, u32)) -> Vec<u8> {
    let mut pixels: Vec<u8> = vec![0; size_px.0 as usize * size_px.1 as usize * BYTES_PER_PIXEL];
    for y in 0..size_px.1 as usize {
        for x in 0..size_px.0 as usize {
            for c in 0..BYTES_PER_PIXEL {
                let mut temp_sum: u32 = 0;
                for dy in 0..SUPERSAMPLE_COUNT {
                    for dx in 0..SUPERSAMPLE_COUNT {
                        let (fx, fy) = (x * SUPERSAMPLE_COUNT + dx, y * SUPERSAMPLE_COUNT + dy);
                        temp_sum += u32::from(fine[(fx + fy * width) * BYTES_PER_PIXEL + c]);
                    }
                }
                pixels[(x + y * size_px.0 as usize) * BYTES_PER_PIXEL + c] =
                    (temp_sum / (SUPERSAMPLE_COUNT * SUPERSAMPLE_COUNT) as u32) as u8;
            }
        }
    }
    pixels
}
