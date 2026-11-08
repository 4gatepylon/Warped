//! `credits.rs` holds the text of the menu's CREDITS page: where every asset in the game comes
//! from (its author, source and license), the font, and the libraries the game is built on.
//!
//! Interface: `CREDITS_LINES`.
//!
//! Example: `lines.extend(CREDITS_LINES.iter().map(|line| (line.to_string(), false)));`
//!
//! Must know:
//! - Every asset added to the game (a texture, model, sound, font or picture) gets its lines
//!   here, with its author, source and license, as this folder's `AGENTS.md` says.
//! - Each line fits the menu's width, 64 characters (`MENU_MASK_PX` over `DEBUG_GLYPH_PX` in
//!   `main.rs`), which `overlay_text_fits_its_mask` checks; a long link goes over two lines.

/// `CREDITS_LINES` is the CREDITS page's text, one menu line each.
pub const CREDITS_LINES: [&str; 24] = [
    "Warped: hyperbolic wormholes",
    "",
    "SURFACES  Poly Haven textures, CC0 (polyhaven.com/a/<name>)",
    "  moon_01, moon_meteor_02, concrete_wall_008, metal_plate,",
    "  corrugated_iron_02, rusty_metal_02, concrete_pavement_02,",
    "  concrete_floor_worn_001, asphalt_02, brick_wall_02",
    "SNIPER    \"Bolt Action Rifle 7.62\" by Mateusz Sadek, CC0",
    "  Poly Haven, polyhaven.com/a/bolt_action_rifle_7_62",
    "RIFLE     \"Sci-fi revolver\" by Ramhat, Sketchfab Standard",
    "  sketchfab.com/3d-models/",
    "  sci-fi-revolver-f6bd1cbad4174048bcf02f8e52771adb",
    "MODELS    Sketchfab, CC BY 4.0, links in common/maps/README.md",
    "  Sjoerd van Riel (kazemat), Santeri Riikonen (garage),",
    "  Golden (bunker), AMMediaGames (beacon), polbrainstorm (Hesco)",
    "PLAYERS   \"Teal V.2\" by Lagst, Sketchfab Standard, clips here",
    "  sketchfab.com/3d-models/",
    "  teal-v2-d6a54ff9468c451bbdbd879afa03f20a",
    "TEXT      SDL3's built-in 8x8 debug font (libsdl.org, zlib)",
    "CODE      Rust crates: sdl3, glam, gltf, image, png, rayon, clap",
    "  objc2, objc2-metal (Metal, MetalFX), serde (json, yaml), hmac,",
    "  sha2, getrandom: each under its own open-source license",
    "",
    "Everything else (maps, terrain, sky, ships, bars) is made in",
    "code in this repository.",
];
