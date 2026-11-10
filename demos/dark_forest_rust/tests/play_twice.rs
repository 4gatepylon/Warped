//! `play_twice` checks that the forest can be played twice in one process, as the one-window app
//! does when the player goes back to the game menu and joins again: each play is set up in the
//! same window, left from the menu's QUIT page (`Esc`, `Q`, `Return`) with `LevelEnd::LeaveLevel`,
//! and gives back the GPU memory it took. It runs on the main thread (`harness = false`, since
//! SDL's video must), in a hidden window, and needs the GPU and the downloaded textures, so it
//! runs only when asked: `cargo test --test play_twice -- --ignored`.
//!
//! | Partitioning | Parts | Covered by |
//! | --- | --- | --- |
//! | Play in one process | first, second | `main`'s two rounds |
//! | GPU memory after a play | back to what it was after the first (no leak) | `main`'s check of `currentAllocatedSize` |
//!
//! Example: `cargo test --test play_twice -- --ignored`

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice};
use objc2_quartz_core::CAMetalLayer;
use sdl3::event::Event;
use sdl3::keyboard::{Mod, Scancode};
use warped_shared::app::{LevelEnd, LevelWindow};

use clap::Parser;
use warped_dark_forest::args::Args;
use warped_dark_forest::level::{Alone, Level, LevelFiles};
use warped_dark_forest::platform::Platform;

/// `PLAY_COUNT` is how many times the forest is played in the one process.
const PLAY_COUNT: usize = 2;
/// `LEAK_TOLERANCE_BYTES` is how much more GPU memory a later play may leave allocated than the
/// first did: far less than the forest's scene and render targets (hundreds of MB).
const LEAK_TOLERANCE_BYTES: usize = 32 << 20;
/// `IGNORED_FLAG` is the flag `cargo test -- --ignored` passes, which runs this test.
const IGNORED_FLAG: &str = "--ignored";

/// `press` is the event of `scancode` going down.
///
/// Example: `sdl.event()?.push_event(press(Scancode::Escape))?;`
fn press(scancode: Scancode) -> Event {
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

fn main() {
    if !std::env::args().any(|arg| arg == IGNORED_FLAG) {
        println!("play_twice: ignored (needs the GPU and a window); run with -- {IGNORED_FLAG}");
        return;
    }
    let args: Args = Args::try_parse_from([
        "dark-forest",
        "--no-title",
        "--bot-count",
        "0",
        "--width-pt",
        "640",
        "--height-pt",
        "400",
    ])
    .expect("valid flags");
    let platform: Platform = Platform::new("Dark Forest play_twice", 640, 400, false, true);
    let layer: Retained<CAMetalLayer> = platform.metal_layer();
    let device: Retained<ProtocolObject<dyn MTLDevice>> =
        MTLCreateSystemDefaultDevice().expect("this Mac has no Metal device");
    let mut event_pump = platform.sdl.event_pump().expect("SDL event pump failed");
    let events = platform.sdl.event().expect("SDL events failed");
    let mut alone: Alone = Alone;
    let mut window: LevelWindow = LevelWindow {
        sdl: &platform.sdl,
        window: &platform.window,
        event_pump: &mut event_pump,
        layer: &layer,
        device: &device,
        match_link: &mut alone,
    };
    let mut allocated_bytes_after_plays: Vec<usize> = Vec::new();
    for _ in 0..PLAY_COUNT {
        let end: Option<LevelEnd> = autoreleasepool(|_| {
            let files: LevelFiles = LevelFiles::Find(&args).expect("the map and assets are there");
            let level: Level = Level::Load(&window, &args, files);
            for scancode in [Scancode::Escape, Scancode::Q, Scancode::Return] {
                events
                    .push_event(press(scancode))
                    .expect("SDL took the event");
            }
            level.play(&mut window, &args, false).expect("played")
        });
        assert_eq!(end, Some(LevelEnd::LeaveLevel));
        allocated_bytes_after_plays.push(device.currentAllocatedSize());
    }
    println!("play_twice: GPU bytes allocated after each play: {allocated_bytes_after_plays:?}");
    let first_bytes: usize = allocated_bytes_after_plays[0];
    for bytes in &allocated_bytes_after_plays[1..] {
        assert!(
            *bytes <= first_bytes + LEAK_TOLERANCE_BYTES,
            "a later play left {bytes} GPU bytes allocated, the first {first_bytes}"
        );
    }
    println!("play_twice: ok");
}
