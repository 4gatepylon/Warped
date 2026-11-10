//! `painter.rs` draws the app's menu pages: `render_page` draws a page's shapes on the CPU with
//! SDL's software renderer and debug font (`warped_shared::page::draw_shapes`, the match
//! window's look), and `Painter` puts those pixels into the window's `CAMetalLayer`, the layer a
//! level draws into when the app hands it the window.
//!
//! Interface: `render_page`, `PixelOrder`, `Painter` (`New`, `paint`).
//!
//! Example: `painter.paint(&layer, &render_page(size_px, page_size, &shapes, &[], PixelOrder::Bgra)?, size_px)?;`
//!
//! Must know:
//! - Performance: a page is drawn again only when its shapes or the window's size change, so the
//!   menus cost the GPU one copy per change.

use std::error::Error;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLDevice,
    MTLOrigin, MTLPixelFormat, MTLRegion, MTLSize, MTLStorageMode, MTLTexture,
    MTLTextureDescriptor, MTLTextureUsage,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};
use sdl3::pixels::PixelFormat;
use warped_shared::page::{Shape, Thumbnail, draw_shapes, make_thumbnail};

use crate::levels_manifest::Picture;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BYTES_PER_PIXEL: usize = 4;

/// `texture_slot_t` is a Metal texture with its size in pixels, or none yet.
#[allow(non_camel_case_types)]
type texture_slot_t = Option<(Retained<ProtocolObject<dyn MTLTexture>>, (usize, usize))>;

/// `PixelOrder` is the byte order of `render_page`'s pixels: BGRA for the Metal layer, RGBA for a
/// PNG.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelOrder {
    Bgra,
    Rgba,
}

/// `render_page` draws `shapes` (a page of `page_size` units, scaled to fit and centred) into
/// `size_px` pixels in `order`, rows top first, each `Shape::Thumbnail` drawing its picture from
/// `pictures`.
///
/// Example: `let pixels = render_page((1280, 800), [640.0, 455.0], &shapes, &[], PixelOrder::Rgba)?;`
pub fn render_page(
    size_px: (u32, u32),
    page_size: [f32; 2],
    shapes: &[Shape],
    pictures: &[Option<Picture>],
    order: PixelOrder,
) -> Result<Vec<u8>> {
    let format: PixelFormat = match order {
        PixelOrder::Bgra => PixelFormat::BGRA32,
        PixelOrder::Rgba => PixelFormat::RGBA32,
    };
    let surface = sdl3::surface::Surface::new(size_px.0, size_px.1, format)?;
    let mut canvas: sdl3::render::Canvas<sdl3::surface::Surface> = surface.into_canvas()?;
    let creator = canvas.texture_creator();
    let thumbnails: Vec<Option<Thumbnail>> = pictures
        .iter()
        .map(|picture| match picture {
            Some((pixels, picture_px)) => make_thumbnail(&creator, pixels, *picture_px).map(Some),
            None => Ok(None),
        })
        .collect::<Result<Vec<Option<Thumbnail>>>>()?;
    draw_shapes(&mut canvas, size_px, page_size, shapes, &thumbnails)?;
    canvas.present();
    drop(thumbnails);
    let surface = canvas.into_surface();
    let pitch: usize = surface.pitch() as usize;
    let row_bytes: usize = size_px.0 as usize * BYTES_PER_PIXEL;
    Ok(surface.with_lock(|pixels: &[u8]| {
        (0..size_px.1 as usize)
            .flat_map(|y| pixels[y * pitch..y * pitch + row_bytes].iter().copied())
            .collect()
    }))
}

/// `Painter` shows pages in a window's `CAMetalLayer`: the device's command queue, and a shared
/// texture of the drawable's size that each page is copied into.
pub struct Painter {
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    texture: texture_slot_t,
}

impl Painter {
    /// `New` makes a painter on `device`.
    ///
    /// Example: `let painter = Painter::New(window.get_device())?;`
    #[allow(non_snake_case)]
    pub fn New(device: &ProtocolObject<dyn MTLDevice>) -> Result<Painter> {
        let queue = device.newCommandQueue().ok_or("Metal: no command queue")?;
        Ok(Painter {
            queue,
            texture: None,
        })
    }

    /// `paint` shows `bgra` (BGRA pixels of the layer's drawable size, rows top first) in the
    /// layer's next drawable, on the layer's `device`, waiting for the copy; it returns false when
    /// the layer had no drawable free.
    ///
    /// Example: `painter.paint(layer, window.get_device(), &pixels, (2560, 1440))?;`
    pub fn paint(
        &mut self,
        layer: &CAMetalLayer,
        device: &ProtocolObject<dyn MTLDevice>,
        bgra: &[u8],
        size_px: (usize, usize),
    ) -> Result<bool> {
        assert_eq!(
            bgra.len(),
            size_px.0 * size_px.1 * BYTES_PER_PIXEL,
            "Painter::paint: the pixels aren't {}x{} BGRA",
            size_px.0,
            size_px.1
        );
        let Some(drawable) = layer.nextDrawable() else {
            return Ok(false);
        };
        let target = drawable.texture();
        if (target.width(), target.height()) != size_px {
            return Ok(false);
        }
        if self
            .texture
            .as_ref()
            .is_none_or(|(_, size)| *size != size_px)
        {
            // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TEXTURE OF THE WRONG FORMAT OR SIZE WILL HAPPEN
            let descriptor: Retained<MTLTextureDescriptor> = unsafe {
                MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                    MTLPixelFormat::BGRA8Unorm,
                    size_px.0,
                    size_px.1,
                    false,
                )
            };
            descriptor.setUsage(MTLTextureUsage::ShaderRead);
            descriptor.setStorageMode(MTLStorageMode::Shared);
            let texture = device
                .newTextureWithDescriptor(&descriptor)
                .ok_or("Metal: no memory for the menu's texture")?;
            self.texture = Some((texture, size_px));
        }
        let (texture, _) = self.texture.as_ref().expect("the texture was just made");
        let region: MTLRegion = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width: size_px.0,
                height: size_px.1,
                depth: 1,
            },
        };
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A READ PAST THE PIXELS WILL HAPPEN
        unsafe {
            texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
                region,
                0,
                NonNull::new(bgra.as_ptr() as *mut std::ffi::c_void).expect("a page's pixels"),
                size_px.0 * BYTES_PER_PIXEL,
            )
        };
        let command_buffer = self
            .queue
            .commandBuffer()
            .ok_or("Metal: no command buffer")?;
        let blit = command_buffer
            .blitCommandEncoder()
            .ok_or("Metal: no blit encoder")?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A COPY BETWEEN TEXTURES OF DIFFERENT SIZES WILL HAPPEN
        unsafe { blit.copyFromTexture_toTexture(texture, &target) };
        blit.endEncoding();
        command_buffer.presentDrawable(ProtocolObject::from_ref(&*drawable));
        command_buffer.commit();
        command_buffer.waitUntilCompleted();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `painter.rs`'s CPU half (the Metal half needs a window).
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | Pixel order | BGRA, RGBA of one red panel | `pages_render_in_either_order` |

    use super::*;

    #[test]
    fn pages_render_in_either_order() {
        let shapes: Vec<Shape> = vec![Shape::Panel {
            min: [0.0, 0.0],
            size: [10.0, 10.0],
            rgba: [255, 0, 0, 255],
            border_rgb: None,
        }];
        let rgba: Vec<u8> =
            render_page((10, 10), [10.0, 10.0], &shapes, &[], PixelOrder::Rgba).unwrap();
        let bgra: Vec<u8> =
            render_page((10, 10), [10.0, 10.0], &shapes, &[], PixelOrder::Bgra).unwrap();
        assert_eq!(rgba.len(), 10 * 10 * BYTES_PER_PIXEL);
        assert_eq!(rgba[..4], [255, 0, 0, 255]);
        assert_eq!(bgra[..4], [0, 0, 255, 255]);
    }
}
