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
//! - `map_edge`: the soft edge of every map: past the playable area players slow without end.
//! - `health`: the damage a hit adds, elimination, and what a fall in health shows (`HitFeedback`).
//! - `play_settings`: the settings files' sections every level reads alike.
//! - `shot_noise`: how far each shot strays from the crosshair, and its beam's speckle.
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
//!
//! Example: `use warped_shared::geometry::{AXIS_W, make_boost};`
//!
//! Must know:
//! - Gotchas: a file here is the same for every level; a level that needs it to differ passes the
//!   difference in, rather than keeping its own copy (`README.md`, "Adding a level").

pub mod app_bundle;
pub mod avatar_model;
pub mod fps_demo_assets;
pub mod gait;
pub mod geometry;
pub mod health;
#[cfg(target_os = "macos")]
pub mod hud;
#[cfg(target_os = "macos")]
pub mod hud_bars;
#[cfg(target_os = "macos")]
pub mod key_bindings;
pub mod map_edge;
#[cfg(target_os = "macos")]
pub mod menu_and_map;
pub mod mesh;
pub mod movement;
#[cfg(target_os = "macos")]
pub mod overlay;
pub mod play_settings;
#[cfg(target_os = "macos")]
pub mod render_mesh;
pub mod shapes;
pub mod shot_noise;
pub mod spaceships;
#[cfg(target_os = "macos")]
pub mod texture_arrays;
pub mod third_person;
#[cfg(target_os = "macos")]
pub mod viewmodel;
#[cfg(target_os = "macos")]
pub mod weapons;
