//! `warped_shared` is the code every Rust level demo of Warped shares: Lunar Surface
//! (`../hyperbolic_wormholes_rust/`), Dust, Snow Planet, City Sphere and the match. Each level is
//! its own program; a level crate depends on this one and brings its modules in at its root
//! (`use warped_shared::geometry;`), so the level's own files keep naming them `crate::geometry`.
//!
//! Interface: the modules below; the ones that need Metal or SDL build on macOS only, so a level's
//! game server alone still builds for Linux.
//! - `geometry`: the hyperboloid model's maths.
//! - `app_bundle`, `fps_demo_assets`: where the game's files are, in the checkout or in `Warped.app`.
//! - `shapes`: the smooth solids structures are made of.
//! - `movement`: the rules of moving: crouching, sprint energy, the head bob, a hit's slow.
//! - `gravity`: the pull of black holes and wormhole mouths on a player on the ground.
//! - `health`: the damage a hit adds, elimination, and what a fall in health shows (`HitFeedback`).
//! - `play_settings`: the settings files' sections every level reads alike.
//! - `shot_noise`: how far each shot strays from the crosshair, and its beam's speckle.
//! - `shot_gate`: when a player may shoot after jumping, walking or sprinting (the `shooting` rules).
//! - `laser_drop`: bullet drop, how a shot's path bends down (`shooting.drop_m_per_s_per_s`).
//! - `mesh`, `avatar_model`: glTF models of the maps' structures and the players' robot body.
//! - `gait`: which of the robot's clips another player shows, and where in it, from their motion.
//! - `spaceships`: the spaceships in the sky.
//! - `key_bindings`, `menu_and_map`: a player's controls, and what covers the view (the menu on
//!   `Esc`, the map while `Tab` is held) with what that does to the mouse.
//! - `texture_arrays`, `render_mesh`, `viewmodel`: Metal textures, models and the gun held in hand.
//! - `overlay`, `hud_bars`, `hud`: what is drawn over a frame, the HUD's bars, its pulses, its
//!   crosshair, the hit effect and the banner's lobby and "YOU DIED" lines.
//! - `weapons`: the two guns, aiming, hit weights and the gun in hand's motion.
//! - `third_person`: the third-person view's switch, and its boom in flat space.
//! - `app`: the interface between the one-window app (`../warped_mvp_rust/`) and a level.
//! - `match_state`, `match_protocol`, `match_menu`: the match's rules, what its server and clients
//!   share (levels file, view, client, network beacon) and its progression page.
//! - `network_scope`: whether sockets reach the local network or stay on loopback
//!   (`WARPED_LOOPBACK_ONLY=1`, for tests).
//! - `page`: a menu page's shapes in the debug font, and drawing them with SDL.
//! - `metal_window`: the one window a program draws in with Metal (SDL3 window, `CAMetalLayer`,
//!   device, event pump), lent to a level as an `app::LevelWindow`.
//! - `player_settings`, `settings_editor`: `player_settings.yaml`, and a settings page any menu
//!   shows (numbers, key bindings, buttons).
//!
//! Example: `use warped_shared::geometry::{AXIS_W, make_boost};`
//!
//! Must know:
//! - Gotchas: a file here is the same for every level; a level that needs it to differ passes the
//!   difference in, rather than keeping its own copy (`README.md`, "Adding a level").

pub mod app;
pub mod app_bundle;
pub mod avatar_model;
pub mod fps_demo_assets;
pub mod gait;
pub mod geometry;
pub mod gravity;
pub mod health;
#[cfg(target_os = "macos")]
pub mod hud;
#[cfg(target_os = "macos")]
pub mod hud_bars;
#[cfg(target_os = "macos")]
pub mod key_bindings;
pub mod laser_drop;
pub mod match_menu;
pub mod match_protocol;
pub mod match_state;
#[cfg(target_os = "macos")]
pub mod menu_and_map;
pub mod mesh;
#[cfg(target_os = "macos")]
pub mod metal_window;
pub mod movement;
pub mod network_scope;
#[cfg(target_os = "macos")]
pub mod overlay;
pub mod page;
pub mod play_settings;
#[cfg(target_os = "macos")]
pub mod player_settings;
#[cfg(target_os = "macos")]
pub mod render_mesh;
pub mod settings_editor;
pub mod shapes;
pub mod shot_gate;
pub mod shot_noise;
pub mod spaceships;
#[cfg(target_os = "macos")]
pub mod texture_arrays;
pub mod third_person;
#[cfg(target_os = "macos")]
pub mod viewmodel;
#[cfg(target_os = "macos")]
pub mod weapons;
