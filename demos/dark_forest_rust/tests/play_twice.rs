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

use objc2::rc::autoreleasepool;
use objc2_metal::MTLDevice;
use sdl3::keyboard::Scancode;
use sdl3::sys::events::{SDL_EVENT_KEY_DOWN, SDL_Event, SDL_KeyboardEvent, SDL_PushEvent};
use warped_shared::app::{LevelEnd, LevelWindow};
use warped_shared::metal_window::{MetalWindow, NoMatchLink, WindowOptions};

use clap::Parser;
use warped_dark_forest::args::Args;
use warped_dark_forest::level::{Level, LevelFiles};

/// `PLAY_COUNT` is how many times the forest is played in the one process.
const PLAY_COUNT: usize = 2;
/// `LEAK_TOLERANCE_BYTES` is how much more GPU memory a later play may leave allocated than the
/// first did: far less than the forest's scene and render targets (hundreds of MB).
const LEAK_TOLERANCE_BYTES: usize = 32 << 20;
/// `IGNORED_FLAG` is the flag `cargo test -- --ignored` passes, which runs this test.
const IGNORED_FLAG: &str = "--ignored";

/// `push_key_down` queues the event of `scancode` going down, as the keyboard would; it goes through SDL's own
/// `SDL_PushEvent`, since the sdl3 crate's `push_event` doesn't take keyboard events.
///
/// Example: `push_key_down(Scancode::Escape);`
fn push_key_down(scancode: Scancode) {
    let key_down: SDL_KeyboardEvent = SDL_KeyboardEvent {
        r#type: SDL_EVENT_KEY_DOWN,
        scancode: scancode.into(),
        down: true,
        repeat: false,
        ..Default::default()
    };
    let mut event: SDL_Event = SDL_Event { key: key_down };
    // DANGEROUS DANGEROUS DANGEROUS IF YOU CHANGE THIS THEN SDL READING AN EVENT AS THE WRONG TYPE WILL HAPPEN
    let is_pushed: bool = unsafe { SDL_PushEvent(&mut event) };
    assert!(is_pushed, "SDL didn't queue the key: {}", sdl3::get_error());
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
    let mut metal_window: MetalWindow = MetalWindow::Open(
        "Dark Forest play_twice",
        (640, 400),
        WindowOptions {
            is_fullscreen: false,
            is_hidden: true,
        },
    )
    .expect("the window opens");
    let mut no_match_link: NoMatchLink = NoMatchLink;
    let mut window: LevelWindow = metal_window.get_level_window(&mut no_match_link);
    let mut allocated_bytes_after_plays: Vec<usize> = Vec::new();
    println!(
        "play_twice: GPU bytes allocated before the first play: {}",
        window.device.currentAllocatedSize()
    );
    for _ in 0..PLAY_COUNT {
        let end: Option<LevelEnd> = autoreleasepool(|_| {
            let files: LevelFiles = LevelFiles::Find(&args).expect("the map and assets are there");
            let level: Level = Level::Load(&window, &args, files);
            for scancode in [Scancode::Escape, Scancode::Q, Scancode::Return] {
                push_key_down(scancode);
            }
            level.play(&mut window, &args, false).expect("played")
        });
        assert_eq!(end, Some(LevelEnd::LeaveLevel));
        allocated_bytes_after_plays.push(window.device.currentAllocatedSize());
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
