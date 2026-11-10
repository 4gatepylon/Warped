//! `level_test.rs` checks a level's entry point (`app::play_level_fn_t`) the way the one-window app
//! uses it, without a window on screen: SDL's offscreen video driver gives the window, a
//! `CAMetalLayer` of the check's own takes the frames, and a scripted `MatchLink` presses keys and
//! ends rounds. It plays the level three times in one process: the player confirms the menu's
//! QUIT page (`LevelEnd::LeaveLevel`), then the match's round ends (`LevelEnd::RoundOver`), then
//! QUIT again; the Metal device's allocated memory after the third must not exceed that after the
//! first by more than `MAX_GROWTH_MIBYTES`, so a level leaves no GPU resources behind.
//!
//! Interface: `check_level_in_one_window`, `make_sample_join`.
//!
//! Example: `check_level_in_one_window(lunar_surface_level::play_level, &join)?;`
//!
//! Must know:
//! - Gotchas: it uses the GPU, and SDL must start on the main thread, so a level calls it from a
//!   test without the test harness (`harness = false`), run only when asked (`RUN_GPU_TESTS=1`).
//! - Gotchas: no server answers at the join's address, so the level stays in its lobby; the check
//!   covers the window, the loop, loading, drawing, the menu and the endings, not play.

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat};
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::{Mod, Scancode};

use crate::app::{LevelEnd, LevelJoin, LevelWindow, MatchLink, play_level_fn_t};
use crate::match_protocol::{LevelEntry, MatchView};
use crate::match_state::Team;

/// `Result` is this module's result type.
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const OFFSCREEN_VIDEO_DRIVER: &str = "offscreen";
const WINDOW_TITLE: &str = "level check";
const WINDOW_PT: (u32, u32) = (640, 400);
const FRAMES_BEFORE_KEYS_COUNT: usize = 90;
const MAX_FRAME_COUNT: usize = 3000;
const MAX_GROWTH_MIBYTES: usize = 64;
const BYTES_PER_MIBYTE: usize = 1 << 20;
const QUIT_KEYS: [Scancode; 3] = [Scancode::Escape, Scancode::Q, Scancode::Y];

/// `ScriptedLink` is the check's `MatchLink`: after `FRAMES_BEFORE_KEYS_COUNT` frames it either
/// presses `QUIT_KEYS` (one a frame) or ends the round; it fails the check after
/// `MAX_FRAME_COUNT` frames by ending the round, which the check doesn't expect then.
struct ScriptedLink {
    events: sdl3::EventSubsystem,
    is_quitting: bool,
    frame_count: usize,
}

impl MatchLink for ScriptedLink {
    /// `is_round_over` counts a frame, presses the next key when it is due, and says whether the
    /// round is over.
    ///
    /// Example: `if link.is_round_over() { ... }`
    fn is_round_over(&mut self) -> bool {
        self.frame_count += 1;
        let key_index: Option<usize> = self.frame_count.checked_sub(FRAMES_BEFORE_KEYS_COUNT);
        match (self.is_quitting, key_index) {
            (_, None) => false,
            (true, Some(i)) => {
                if let Some(scancode) = QUIT_KEYS.get(i) {
                    self.events
                        .push_event(make_key_down(*scancode))
                        .expect("SDL takes a pushed key");
                }
                self.frame_count >= MAX_FRAME_COUNT
            }
            (false, Some(_)) => true,
        }
    }

    /// `get_match_view` has no match to show.
    ///
    /// Example: `assert!(link.get_match_view().is_none());`
    fn get_match_view(&self) -> Option<&MatchView> {
        None
    }

    /// `get_match_lines` gives a one-line MATCH page, so the menu shows it.
    ///
    /// Example: `menu.set_match_lines(link.get_match_lines());`
    fn get_match_lines(&self) -> Option<Vec<(String, bool)>> {
        Some(vec![("LEVEL CHECK".to_string(), true)])
    }
}

/// `make_key_down` gives the event of `scancode` pressed.
///
/// Example: `events.push_event(make_key_down(Scancode::Escape))?;`
fn make_key_down(scancode: Scancode) -> Event {
    Event::KeyDown {
        timestamp: 0,
        window_id: 0,
        keycode: None,
        scancode: Some(scancode),
        keymod: Mod::NOMOD,
        repeat: false,
        which: 0,
        raw: 0,
    }
}

/// `make_sample_join` gives a join of the level `name` (on `--map map`, if any) at `server`, on
/// `team`, with the settings file `player_settings_path`, for checks and tools.
///
/// Example: `let join = make_sample_join("Dust", Some("dust"), server, Team::Blue, path);`
pub fn make_sample_join(
    name: &str,
    map: Option<&str>,
    server: SocketAddr,
    team: Team,
    player_settings_path: PathBuf,
) -> LevelJoin {
    LevelJoin {
        level: LevelEntry {
            name: name.to_string(),
            folder: String::new(),
            branch: String::new(),
            status: String::new(),
            program: String::new(),
            map: map.map(str::to_string),
            server_args: Vec::new(),
            client_args: Vec::new(),
            stand_in_for: None,
            takes_bot_label: true,
            map_picture: None,
        },
        server,
        team,
        player_settings_path,
    }
}

/// `check_level_in_one_window` plays `join` with `play` three times in one offscreen window (QUIT,
/// round over, QUIT) and fails, saying which, if a play ends otherwise or the GPU's allocated
/// memory grew by more than `MAX_GROWTH_MIBYTES` from the first play's end to the third's. It
/// prints each play's end and memory.
///
/// Example: `check_level_in_one_window(dust_level::play_level, &join)?;`
pub fn check_level_in_one_window(play: play_level_fn_t, join: &LevelJoin) -> Result<()> {
    sdl3::hint::set("SDL_VIDEO_DRIVER", OFFSCREEN_VIDEO_DRIVER);
    let sdl: sdl3::Sdl = sdl3::init()?;
    let video: sdl3::VideoSubsystem = sdl.video()?;
    let window: sdl3::video::Window = video
        .window(WINDOW_TITLE, WINDOW_PT.0, WINDOW_PT.1)
        .high_pixel_density()
        .build()?;
    let mut event_pump: sdl3::EventPump = sdl.event_pump()?;
    let device: Retained<ProtocolObject<dyn MTLDevice>> =
        MTLCreateSystemDefaultDevice().ok_or("Metal: this Mac has no GPU Metal can use")?;
    let layer: Retained<CAMetalLayer> = CAMetalLayer::new();
    layer.setDevice(Some(&device));
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setFramebufferOnly(false);
    let mut allocated_bytes: Vec<usize> = Vec::new();
    for (is_quitting, expected) in [
        (true, LevelEnd::LeaveLevel),
        (false, LevelEnd::RoundOver { winner: None }),
        (true, LevelEnd::LeaveLevel),
    ] {
        let mut link: ScriptedLink = ScriptedLink {
            events: sdl.event()?,
            is_quitting,
            frame_count: 0,
        };
        let mut level_window: LevelWindow = LevelWindow {
            sdl: &sdl,
            window: &window,
            event_pump: &mut event_pump,
            layer: &layer,
            device: &device,
            match_link: &mut link,
        };
        let end: LevelEnd = play(&mut level_window, join)?;
        let bytes: usize = device.currentAllocatedSize();
        println!(
            "{}: {end:?} after {} frames, {} MiB on the GPU",
            join.level.name,
            link.frame_count,
            bytes / BYTES_PER_MIBYTE
        );
        if end != expected {
            return Err(format!("{}: expected {expected:?}, got {end:?}", join.level.name).into());
        }
        allocated_bytes.push(bytes);
    }
    let growth_bytes: usize = allocated_bytes[2].saturating_sub(allocated_bytes[0]);
    if growth_bytes > MAX_GROWTH_MIBYTES * BYTES_PER_MIBYTE {
        return Err(format!(
            "{}: the GPU's memory grew by {} MiB from the first play to the third; a play leaves resources behind",
            join.level.name,
            growth_bytes / BYTES_PER_MIBYTE
        )
        .into());
    }
    Ok(())
}
