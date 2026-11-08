//! `play.rs` runs a game window: it opens the window, shows the loading screen, then each frame
//! handles input, updates the `Session`, draws it with Metal into the window's layer (or on an SDL
//! canvas) and draws the HUD's text and rectangles over it.
//!
//! Interface: `play`, `draw_text_overlay`.
//!
//! Example: `play(&arguments, renderer)?;`

use std::time::Instant;

use objc2_core_foundation::CGSize;
use objc2_metal::MTLPixelFormat;
use objc2_quartz_core::CAMetalLayer;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Texture;

use crate::{
    Arguments, BANNER_HEIGHT_FRAC, BYTES_PER_PIXEL, DEATH_MASK_PX, DISABLE_BAR_MIN_FRAC,
    DISABLE_BAR_RGB, DISABLE_BAR_SIZE_FRAC, MS_PER_S, OverlayText, PackedFrame, Renderer, Result,
    Session, WINDOW_TITLE, get_text_positions, hud_bars, make_text_mask, render_metal, viewmodel,
};

const HUD_TEXT_SCALE: f32 = 2.0;
const HUD_LINE_PX: f32 = 10.0;
const HUD_MARGIN_PX: f32 = 4.0;
const HUD_COLOR: Color = Color::RGB(235, 240, 250);
const LOADING_DIM_FRAC: f32 = 1.0;
const LOADING_TITLE: &str = "LOADING";
const LOADING_DONE_LABEL: &str = "ready";

/// `play` opens the window and runs the demo until it is closed, presenting with Metal straight
/// to the window's layer or, for the CPU, through SDL's renderer.
///
/// Example: `play(&arguments, renderer)?;`
pub(crate) fn play(arguments: &Arguments, mut renderer: Box<dyn Renderer>) -> Result<()> {
    let sdl: sdl3::Sdl = sdl3::init()?;
    let video: sdl3::VideoSubsystem = sdl.video()?;
    let mut session: Session = Session::New(arguments)?;
    let window_size: (u32, u32) = (arguments.width_px, arguments.height_px);
    match renderer.as_metal() {
        Some(metal) => play_on_layer(&sdl, &video, window_size, &mut session, metal),
        None => play_on_canvas(&sdl, &video, window_size, &mut session, renderer.as_mut()),
    }
}

/// `play_on_canvas` runs the window through SDL's renderer: each frame is drawn into pixels, put
/// in a texture scaled to the window, and the HUD is drawn over it with SDL's built-in text.
///
/// Example: `play_on_canvas(&sdl, &video, (1280, 800), &mut session, renderer.as_mut())?;`
fn play_on_canvas(
    sdl: &sdl3::Sdl,
    video: &sdl3::VideoSubsystem,
    window_size: (u32, u32),
    session: &mut Session,
    renderer: &mut dyn Renderer,
) -> Result<()> {
    let window: sdl3::video::Window = video
        .window(WINDOW_TITLE, window_size.0, window_size.1)
        .high_pixel_density()
        .resizable()
        .position_centered()
        .build()?;
    let mouse: sdl3::mouse::MouseUtil = sdl.mouse();
    let mut canvas: sdl3::render::WindowCanvas = window.into_canvas();
    let texture_creator: sdl3::render::TextureCreator<sdl3::video::WindowContext> =
        canvas.texture_creator();
    let mut texture: Option<(Texture, (u32, u32))> = None;
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let mut pixels: Vec<u8> = Vec::new();
    loop {
        for event in event_pump.poll_iter() {
            if !session.handle_event(event, canvas.window(), &mouse) {
                return Ok(());
            }
        }
        if session.is_finished() {
            return Ok(());
        }
        session.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;
        let render_px: (u32, u32) = session.get_render_size(canvas.window().size_in_pixels());
        pixels.resize(
            render_px.0 as usize * render_px.1 as usize * BYTES_PER_PIXEL,
            0,
        );
        let render_start: Instant = Instant::now();
        let mut packed: PackedFrame = session.pack(render_px);
        packed.zoom_in(session.get_zoom());
        renderer.render(packed.as_frame_data(), render_px, &mut pixels)?;
        session.record_frame(
            render_start.elapsed().as_secs_f64() * MS_PER_S,
            renderer.gpu_ms(),
        );
        if texture.as_ref().is_none_or(|(_, size)| *size != render_px) {
            texture = Some((
                texture_creator.create_texture_streaming(
                    PixelFormat::RGBA32,
                    render_px.0,
                    render_px.1,
                )?,
                render_px,
            ));
        }
        let (current_texture, _) = texture.as_mut().expect("the texture was just made");
        current_texture.update(None, &pixels, render_px.0 as usize * BYTES_PER_PIXEL)?;
        canvas.set_scale(1.0, 1.0)?;
        canvas.copy(current_texture, None, None)?;
        let density: f32 = canvas.window().pixel_density();
        canvas.set_scale(density * HUD_TEXT_SCALE, density * HUD_TEXT_SCALE)?;
        canvas.set_draw_color(HUD_COLOR);
        let window_px: (u32, u32) = canvas.window().size_in_pixels();
        for (i, line) in session
            .get_hud_lines(renderer.name(), render_px, window_px)
            .iter()
            .enumerate()
        {
            canvas.draw_debug_text(
                line.as_str(),
                (HUD_MARGIN_PX, HUD_MARGIN_PX + HUD_LINE_PX * i as f32),
            )?;
        }
        draw_overlay_rects(&mut canvas, &session.get_overlay_rects())?;
        if let Some(text) = session.get_overlay_text() {
            draw_text_overlay(&mut canvas, &text)?;
        }
        canvas.present();
    }
}

/// `draw_loading_screen` draws the loading screen on a window's layer: black, the banner
/// `LOADING_TITLE` over `label` (what is loading), and a bar in the pylon-disabling bar's place and
/// look, filled to `done_frac`.
///
/// Example: `draw_loading_screen(metal, layer, 0.5, "revolver")?;`
fn draw_loading_screen(
    metal: &mut render_metal::MetalRenderer,
    layer: &CAMetalLayer,
    done_frac: f32,
    label: &str,
) -> Result<()> {
    let text: OverlayText = OverlayText {
        lines: vec![
            (LOADING_TITLE.to_string(), true),
            (label.to_string(), false),
        ],
        mask_px: DEATH_MASK_PX,
        height_frac: BANNER_HEIGHT_FRAC,
        is_centered: true,
        dim_frac: LOADING_DIM_FRAC,
        picture: None,
    };
    let mask: Vec<u8> = make_text_mask(&text)?;
    let rects: Vec<render_metal::OverlayRect> = hud_bars::make_progress_bar(
        DISABLE_BAR_MIN_FRAC,
        DISABLE_BAR_SIZE_FRAC,
        done_frac,
        DISABLE_BAR_RGB,
    );
    metal.render_overlay_to_layer(
        layer,
        &render_metal::Overlay {
            dim_frac: text.dim_frac,
            mask: Some((&mask, (text.mask_px.0 as usize, text.mask_px.1 as usize))),
            rects: &rects,
            wand: None,
            blur_frac: 0.0,
            hit: render_metal::HitEffect::default(),
            mask_height_frac: text.height_frac,
            picture: None,
            stretch_frac: 0.0,
            minimap: None,
            blueshift_frac: 0.0,
        },
    )?;
    Ok(())
}

/// `play_on_layer` runs the window with Metal presenting straight to the window's `CAMetalLayer`:
/// no pixels go through the CPU, MetalFX upscales the ray-traced image to the window's size, and
/// the HUD is shown in the window's title, since SDL's renderer can't draw on the same layer.
///
/// Example: `play_on_layer(&sdl, &video, (1280, 800), &mut session, metal)?;`
fn play_on_layer(
    sdl: &sdl3::Sdl,
    video: &sdl3::VideoSubsystem,
    window_size: (u32, u32),
    session: &mut Session,
    metal: &mut render_metal::MetalRenderer,
) -> Result<()> {
    let mut builder: sdl3::video::WindowBuilder =
        video.window(WINDOW_TITLE, window_size.0, window_size.1);
    builder.high_pixel_density().resizable().position_centered();
    let flags: u32 = builder.window_flags() | sdl3::sys::video::SDL_WINDOW_METAL.0 as u32;
    let mut window: sdl3::video::Window = builder.set_window_flags(flags).build()?;
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
    let view: sdl3::sys::metal::SDL_MetalView =
        unsafe { sdl3::sys::metal::SDL_Metal_CreateView(window.raw()) };
    if view.is_null() {
        return Err(format!("SDL: couldn't make a Metal view: {}", sdl3::get_error()).into());
    }
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A LAYER POINTER THAT ISN'T A CAMetalLayer WILL HAPPEN
    let layer: &CAMetalLayer =
        unsafe { &*sdl3::sys::metal::SDL_Metal_GetLayer(view).cast::<CAMetalLayer>() };
    layer.setDevice(Some(metal.get_device()));
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setFramebufferOnly(false);
    let mouse: sdl3::mouse::MouseUtil = sdl.mouse();
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let loading_steps: &[render_metal::LoadingStep] = render_metal::get_loading_steps();
    let mut loading_step_index: usize = 0;
    let result: Result<()> = (|| loop {
        for event in event_pump.poll_iter() {
            if !session.handle_event(event, &window, &mouse) {
                return Ok(());
            }
        }
        if session.is_finished() {
            return Ok(());
        }
        session.update(&event_pump.keyboard_state(), &event_pump.mouse_state())?;
        let window_px: (u32, u32) = window.size_in_pixels();
        layer.setDrawableSize(CGSize {
            width: f64::from(window_px.0),
            height: f64::from(window_px.1),
        });
        if loading_step_index <= loading_steps.len() {
            let step: Option<render_metal::LoadingStep> =
                loading_steps.get(loading_step_index).copied();
            let done_frac: f32 = loading_step_index as f32 / loading_steps.len() as f32;
            let label: &str = step.map_or(LOADING_DONE_LABEL, render_metal::LoadingStep::get_label);
            draw_loading_screen(metal, layer, done_frac, label)?;
            if let Some(step) = step {
                metal.load(step)?;
            }
            loading_step_index += 1;
            continue;
        }
        let render_px: (u32, u32) = session.get_render_size(window_px);
        let render_start: Instant = Instant::now();
        let mut packed: PackedFrame = session.pack(render_px);
        packed.zoom_in(session.get_zoom());
        let overlay_text: Option<OverlayText> = session.get_overlay_text();
        let text_mask: Option<Vec<u8>> = overlay_text.as_ref().map(make_text_mask).transpose()?;
        let rects: Vec<render_metal::OverlayRect> = session.get_overlay_rects();
        let wand: Option<(render_metal::OverlayWand, viewmodel::GunModel)> = session
            .get_wand()
            .map(|wand| (wand, session.gun.get_model()));
        let stretch_frac: f32 = session.get_edge_stretch_frac();
        let minimap: Option<render_metal::OverlayMinimap> = session.make_overlay_minimap(window_px);
        let blueshift_frac: f32 = session.blueshift_frac;
        let overlay: Option<render_metal::Overlay> = (text_mask.is_some()
            || !rects.is_empty()
            || wand.is_some()
            || stretch_frac > 0.0
            || blueshift_frac != 0.0
            || minimap.is_some())
        .then(|| render_metal::Overlay {
            dim_frac: match &overlay_text {
                Some(text) => text.dim_frac,
                None => 0.0,
            },
            mask: text_mask
                .as_deref()
                .zip(overlay_text.as_ref())
                .map(|(pixels, text)| (pixels, (text.mask_px.0 as usize, text.mask_px.1 as usize))),
            mask_height_frac: match &overlay_text {
                Some(text) => text.height_frac,
                None => 0.0,
            },
            picture: overlay_text
                .as_ref()
                .and_then(|text| text.picture.as_ref())
                .map(|picture| render_metal::OverlayPicture {
                    pixels: &picture.pixels,
                    size_px: (picture.size_px.0 as usize, picture.size_px.1 as usize),
                    min_mask_px: picture.min_mask_px,
                    max_excl_mask_px: picture.max_excl_mask_px,
                }),
            stretch_frac,
            minimap,
            rects: &rects,
            wand,
            blur_frac: session.motion_blur_frac,
            hit: session.get_hit_effect(),
            blueshift_frac,
        });
        if metal.render_to_layer(packed.as_frame_data(), layer, overlay.as_ref())? {
            session.record_frame(
                render_start.elapsed().as_secs_f64() * MS_PER_S,
                Some(metal.last_gpu_ms),
            );
        }
        if session.hud_frame_count == 0 {
            window.set_title(&format!(
                "{WINDOW_TITLE}: {}",
                session
                    .get_hud_lines("metal", render_px, window_px)
                    .join("  |  ")
            ))?;
        }
    })();
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN A METAL VIEW OUTLIVING ITS WINDOW WILL HAPPEN
    unsafe { sdl3::sys::metal::SDL_Metal_DestroyView(view) };
    result
}

/// `draw_overlay_rects` draws the HUD's rectangles (`Session::get_overlay_rects`) on the canvas,
/// for the SDL-renderer window: flat boxes in their colours, without the Metal overlay's style
/// (slant, outline, gloss, glow) or pulses.
///
/// Example: `draw_overlay_rects(&mut canvas, &session.get_overlay_rects())?;`
fn draw_overlay_rects(
    canvas: &mut sdl3::render::WindowCanvas,
    rects: &[render_metal::OverlayRect],
) -> Result<()> {
    let (width, height) = canvas.window().size_in_pixels();
    canvas.set_scale(1.0, 1.0)?;
    canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
    for rect in rects {
        let [r, g, b, a] = rect.color.map(|c| (c * 255.0) as u8);
        canvas.set_draw_color(Color::RGBA(r, g, b, a));
        canvas.fill_rect(sdl3::render::FRect::new(
            rect.min_frac[0] * width as f32,
            rect.min_frac[1] * height as f32,
            (rect.max_frac[0] - rect.min_frac[0]) * width as f32,
            (rect.max_frac[1] - rect.min_frac[1]) * height as f32,
        ))?;
    }
    Ok(())
}

/// `draw_text_overlay` dims the canvas and draws `text` in its middle, scaled like the Metal
/// overlay's mask (`text.height_frac` of the height, at most `OVERLAY_MAX_WIDTH_FRAC` of the
/// width), for the SDL-renderer window.
///
/// Example: `draw_text_overlay(&mut canvas, &text)?;`
pub(crate) fn draw_text_overlay(
    canvas: &mut sdl3::render::WindowCanvas,
    text: &OverlayText,
) -> Result<()> {
    let (width, height) = canvas.window().size_in_pixels();
    let (mask_width, mask_height) = text.mask_px;
    let scale: f32 = ((height as f32 * text.height_frac) as u32 / mask_height)
        .min((f64::from(width) * render_metal::OVERLAY_MAX_WIDTH_FRAC) as u32 / mask_width)
        .max(1) as f32;
    canvas.set_scale(1.0, 1.0)?;
    canvas.set_blend_mode(sdl3::render::BlendMode::Blend);
    canvas.set_draw_color(Color::RGBA(0, 0, 0, (text.dim_frac * 255.0) as u8));
    canvas.fill_rect(None)?;
    canvas.set_scale(scale, scale)?;
    let left: f32 = (width as f32 / scale - mask_width as f32) / 2.0;
    let top: f32 = (height as f32 / scale - mask_height as f32) / 2.0;
    if let Some(picture) = &text.picture {
        let creator = canvas.texture_creator();
        let mut texture = creator.create_texture_static(
            PixelFormat::RGBA32,
            picture.size_px.0,
            picture.size_px.1,
        )?;
        texture.update(
            None,
            &picture.pixels,
            picture.size_px.0 as usize * BYTES_PER_PIXEL,
        )?;
        let [min_x, min_y] = picture.min_mask_px.map(|n| n as f32);
        let [max_x, max_y] = picture.max_excl_mask_px.map(|n| n as f32);
        canvas.copy(
            &texture,
            None,
            sdl3::render::FRect::new(left + min_x, top + min_y, max_x - min_x, max_y - min_y),
        )?;
    }
    for (line, (x, y), is_highlighted) in get_text_positions(text) {
        canvas.set_draw_color(if is_highlighted {
            Color::RGB(255, 90, 80)
        } else {
            HUD_COLOR
        });
        canvas.draw_debug_text(line.as_str(), (left + x, top + y))?;
    }
    Ok(())
}
