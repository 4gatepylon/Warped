//! `platform` opens the SDL window and hands its `CAMetalLayer` to the renderer.
//!
//! Example:
//!     let platform: Platform = Platform::new("Warped FPS demo", 1600, 900, false);

use objc2::rc::Retained;
use objc2_quartz_core::CAMetalLayer;
use sdl3::sys::metal::{
    SDL_Metal_CreateView, SDL_Metal_DestroyView, SDL_Metal_GetLayer, SDL_MetalView,
};
use sdl3::video::{Window, WindowFlags};

/// `Platform` is the SDL context, the window, and the window's Metal view.
pub struct Platform {
    pub sdl: sdl3::Sdl,
    pub window: Window,
    metal_view: SDL_MetalView,
}

impl Platform {
    /// `new` opens a high-pixel-density Metal window.
    ///
    /// Example:
    ///     let platform: Platform = Platform::new("demo", 1280, 720, false);
    pub fn new(title: &str, width_pt: u32, height_pt: u32, is_fullscreen: bool) -> Platform {
        let sdl: sdl3::Sdl = sdl3::init().expect("SDL failed to start");
        let video = sdl.video().expect("SDL video failed to start");
        let mut builder = video.window(title, width_pt, height_pt);
        builder
            .set_window_flags(WindowFlags::METAL.bits() as u32)
            .high_pixel_density()
            .resizable()
            .position_centered();
        if is_fullscreen {
            builder.fullscreen();
        }
        let window: Window = builder.build().expect("SDL window creation failed");
        let metal_view: SDL_MetalView = unsafe { SDL_Metal_CreateView(window.raw()) };
        assert!(!metal_view.is_null(), "SDL Metal view creation failed");
        Platform {
            sdl,
            window,
            metal_view,
        }
    }

    /// `metal_layer` returns the window's `CAMetalLayer`.
    ///
    /// Example:
    ///     let layer: Retained<CAMetalLayer> = platform.metal_layer();
    pub fn metal_layer(&self) -> Retained<CAMetalLayer> {
        let layer: *mut CAMetalLayer = unsafe { SDL_Metal_GetLayer(self.metal_view) }.cast();
        unsafe { Retained::retain(layer) }.expect("SDL Metal view has no layer")
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        unsafe { SDL_Metal_DestroyView(self.metal_view) };
    }
}
