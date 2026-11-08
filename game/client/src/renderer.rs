//! `renderer.rs` is what draws a packed frame into pixels or a window's layer, and makes the one
//! the command line asks for (Metal).
//!
//! Interface: `Renderer`, `make_renderer`.
//!
//! Example: `let renderer: Box<dyn Renderer> = make_renderer(RendererKind::Metal)?;`

use crate::render_metal::FrameData;

use crate::cli::RendererKind;
use crate::cli::Sampling;
use crate::{Result, render_metal};

/// `Renderer` draws one frame's packed scene into an RGBA pixel buffer.
pub(crate) trait Renderer {
    /// `render` draws `data` (ray-traced at its own render size) into `pixels` at `output_px`
    /// (top row first, 4 bytes per pixel), upscaling it if `output_px` is larger.
    ///
    /// Example: `renderer.render(FrameData::Voxel(&frame), (2560, 1600), &mut pixels)?;`
    fn render(&mut self, data: FrameData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()>;

    /// `name` names the renderer, for the HUD and results.
    ///
    /// Example: `println!("{}", renderer.name());`
    fn name(&self) -> &'static str;

    /// `gpu_ms` gives the GPU's own time for the last frame, for renderers that use one.
    ///
    /// Example: `if let Some(ms) = renderer.gpu_ms() { println!("{ms}"); }`
    fn gpu_ms(&self) -> Option<f64> {
        None
    }

    /// `as_metal` gives the Metal renderer, if this is it, so a window can present straight from
    /// the GPU.
    ///
    /// Example: `if let Some(metal) = renderer.as_metal() { present(metal); }`
    fn as_metal(&mut self) -> Option<&mut render_metal::MetalRenderer> {
        None
    }
}

/// `make_renderer` makes the renderer the command line asked for.
///
/// Example: `let mut renderer = make_renderer(RendererKind::Metal, Sampling::Adaptive)?;`
pub(crate) fn make_renderer(kind: RendererKind, sampling: Sampling) -> Result<Box<dyn Renderer>> {
    match kind {
        RendererKind::Metal => Ok(Box::new(render_metal::MetalRenderer::New(
            sampling == Sampling::Adaptive,
        )?)),
    }
}

impl Renderer for render_metal::MetalRenderer {
    fn render(&mut self, data: FrameData, output_px: (u32, u32), pixels: &mut [u8]) -> Result<()> {
        self.render_to_pixels(data, (output_px.0 as usize, output_px.1 as usize), pixels)
    }

    fn as_metal(&mut self) -> Option<&mut render_metal::MetalRenderer> {
        Some(self)
    }

    fn name(&self) -> &'static str {
        "metal"
    }

    fn gpu_ms(&self) -> Option<f64> {
        Some(self.last_gpu_ms)
    }
}
