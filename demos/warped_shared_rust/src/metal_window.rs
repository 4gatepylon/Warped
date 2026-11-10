//! `metal_window.rs` opens the one kind of window every Warped program draws in with Metal: an SDL3
//! window made with `SDL_WINDOW_METAL`, its Metal view and `CAMetalLayer` (pixel format
//! `BGRA8Unorm`, not framebuffer-only) on the system's Metal device, and SDL's event pump. The
//! one-window app (`../warped_mvp_rust/`) opens one for the whole game; a level run alone opens
//! its own and plays in it through the same entry point the app calls (`get_level_window` with
//! `NoMatchLink`).
//!
//! Interface: `MetalWindow` (`Open`, with `WindowOptions`; `get_window`, `get_layer`, `get_device`, `get_sdl`,
//! `get_event_pump_mut`, `get_level_window`), `NoMatchLink`, `set_window_title`.
//!
//! Example: `let mut window = MetalWindow::Open("Lunar Surface", (1280, 800), WindowOptions::default())?;
//! let end = play_level(&mut window.get_level_window(&mut NoMatchLink), &join)?;`
//!
//! Must know:
//! - Gotchas: SDL allows one `Sdl` and one event pump at a time, so a program opens one
//!   `MetalWindow` and hands its parts on rather than opening a second.
//! - Gotchas: `MetalWindow` must be dropped on the thread that opened it (macOS's main thread).

use std::error::Error;
use std::ffi::CString;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat};
use objc2_quartz_core::CAMetalLayer;
use sdl3::video::Window;

use crate::app::{LevelWindow, MatchLink};
use crate::match_protocol::MatchView;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// `WindowOptions` is how a window opens besides its title and size: covering the screen
/// (`is_fullscreen`, e.g. for a benchmark), or hidden (`is_hidden`, a bot's window, which draws
/// nothing). The default is neither.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WindowOptions {
    pub is_fullscreen: bool,
    pub is_hidden: bool,
}

/// `MetalWindow` is an open window with its Metal layer: SDL, the window, its Metal view (which
/// owns the layer), the Metal device the layer draws with, and SDL's event pump.
pub struct MetalWindow {
    view: sdl3::sys::metal::SDL_MetalView,
    window: Window,
    event_pump: sdl3::EventPump,
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    sdl: sdl3::Sdl,
}

impl MetalWindow {
    /// `Open` starts SDL and opens a window titled `title`, `size_pt` screen points wide and high
    /// (pixels on a 1x display), centred, resizable and at the display's full pixel density, with
    /// a Metal layer on the system's Metal device, full screen or hidden as `options` say.
    ///
    /// Example: `let window = MetalWindow::Open("Warped", (1280, 800), WindowOptions::default())?;`
    #[allow(non_snake_case)]
    pub fn Open(title: &str, size_pt: (u32, u32), options: WindowOptions) -> Result<MetalWindow> {
        let sdl: sdl3::Sdl = sdl3::init()?;
        let video: sdl3::VideoSubsystem = sdl.video()?;
        let mut builder: sdl3::video::WindowBuilder = video.window(title, size_pt.0, size_pt.1);
        builder.high_pixel_density().resizable().position_centered();
        if options.is_fullscreen {
            builder.fullscreen();
        }
        if options.is_hidden {
            builder.hidden();
        }
        let flags: u32 = builder.window_flags() | sdl3::sys::video::SDL_WINDOW_METAL.0 as u32;
        let window: Window = builder.set_window_flags(flags).build()?;
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
        let view: sdl3::sys::metal::SDL_MetalView =
            unsafe { sdl3::sys::metal::SDL_Metal_CreateView(window.raw()) };
        if view.is_null() {
            return Err(format!("SDL: couldn't make a Metal view: {}", sdl3::get_error()).into());
        }
        let device: Retained<ProtocolObject<dyn MTLDevice>> =
            MTLCreateSystemDefaultDevice().ok_or("Metal: this Mac has no GPU Metal can use")?;
        let event_pump: sdl3::EventPump = sdl.event_pump()?;
        let metal_window: MetalWindow = MetalWindow {
            view,
            window,
            event_pump,
            device,
            sdl,
        };
        let layer: &CAMetalLayer = metal_window.get_layer();
        layer.setDevice(Some(&metal_window.device));
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setFramebufferOnly(false);
        Ok(metal_window)
    }

    /// `get_window` gives the SDL window.
    ///
    /// Example: `let size_px = metal_window.get_window().size_in_pixels();`
    pub fn get_window(&self) -> &Window {
        &self.window
    }

    /// `get_layer` gives the window's `CAMetalLayer`.
    ///
    /// Example: `metal_window.get_layer().setDrawableSize(size);`
    pub fn get_layer(&self) -> &CAMetalLayer {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A LAYER POINTER THAT ISN'T A CAMetalLayer WILL HAPPEN
        unsafe { &*sdl3::sys::metal::SDL_Metal_GetLayer(self.view).cast::<CAMetalLayer>() }
    }

    /// `get_device` gives the Metal device the layer draws with.
    ///
    /// Example: `let renderer = MetalRenderer::OnDevice(metal_window.get_device(), true)?;`
    pub fn get_device(&self) -> &ProtocolObject<dyn MTLDevice> {
        &self.device
    }

    /// `get_sdl` gives SDL, for its mouse and other subsystems.
    ///
    /// Example: `let mouse = metal_window.get_sdl().mouse();`
    pub fn get_sdl(&self) -> &sdl3::Sdl {
        &self.sdl
    }

    /// `get_event_pump_mut` gives SDL's event pump, the window's one source of events.
    ///
    /// Example: `for event in metal_window.get_event_pump_mut().poll_iter() { ... }`
    pub fn get_event_pump_mut(&mut self) -> &mut sdl3::EventPump {
        &mut self.event_pump
    }

    /// `get_level_window` lends this window to a level (`app::play_level_fn_t`), with
    /// `match_link` for the match it plays in (`NoMatchLink` for a level run alone).
    ///
    /// Example: `let end = play_level(&mut metal_window.get_level_window(&mut NoMatchLink), &join)?;`
    pub fn get_level_window<'a>(
        &'a mut self,
        match_link: &'a mut dyn MatchLink,
    ) -> LevelWindow<'a> {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A LAYER POINTER THAT ISN'T A CAMetalLayer WILL HAPPEN
        let layer: &'a CAMetalLayer =
            unsafe { &*sdl3::sys::metal::SDL_Metal_GetLayer(self.view).cast::<CAMetalLayer>() };
        LevelWindow {
            sdl: &self.sdl,
            window: &self.window,
            event_pump: &mut self.event_pump,
            layer,
            device: &self.device,
            match_link,
        }
    }
}

impl Drop for MetalWindow {
    /// `drop` destroys the Metal view before its window closes.
    ///
    /// Example: `drop(metal_window);`
    fn drop(&mut self) {
        // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
        unsafe { sdl3::sys::metal::SDL_Metal_DestroyView(self.view) };
    }
}

/// `NoMatchLink` is the `MatchLink` of a level run alone: no match, so its round never ends and
/// its menu has no MATCH page.
pub struct NoMatchLink;

impl MatchLink for NoMatchLink {
    /// `is_round_over` says no: a level run alone has no match round.
    ///
    /// Example: `assert!(!NoMatchLink.is_round_over());`
    fn is_round_over(&mut self) -> bool {
        false
    }

    /// `get_match_view` has no match to show.
    ///
    /// Example: `assert!(NoMatchLink.get_match_view().is_none());`
    fn get_match_view(&self) -> Option<&MatchView> {
        None
    }

    /// `get_match_lines` has no MATCH page to show.
    ///
    /// Example: `assert!(NoMatchLink.get_match_lines().is_none());`
    fn get_match_lines(&self) -> Option<Vec<(String, bool)>> {
        None
    }
}

/// `set_window_title` sets `window`'s title through a shared reference, as a level lent the
/// app's window (`LevelWindow`) holds it.
///
/// Example: `set_window_title(level_window.window, "Lunar Surface: 60 fps")?;`
pub fn set_window_title(window: &Window, title: &str) -> Result<()> {
    let title: CString = CString::new(title)?;
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A TITLE POINTER OUTLIVING ITS STRING WILL HAPPEN
    let is_set: bool =
        unsafe { sdl3::sys::video::SDL_SetWindowTitle(window.raw(), title.as_ptr()) };
    match is_set {
        true => Ok(()),
        false => Err(format!(
            "SDL: couldn't set the window's title: {}",
            sdl3::get_error()
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `metal_window.rs`. Opening a window needs a display and the main thread, so
    //! the tests cover what needs neither: a level run alone has no match.
    //!
    //! | Partitioning | Parts | Tests |
    //! | --- | --- | --- |
    //! | `NoMatchLink`'s answers | round over, match view, match lines | `no_match_link_never_ends_a_round` |

    use super::*;

    #[test]
    fn no_match_link_never_ends_a_round() {
        let mut link: NoMatchLink = NoMatchLink;
        for _ in 0..3 {
            assert!(!link.is_round_over());
        }
        assert!(link.get_match_view().is_none());
        assert!(link.get_match_lines().is_none());
    }
}
