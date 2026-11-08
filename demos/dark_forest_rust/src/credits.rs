//! `credits` holds the text of the menu's CREDITS page: where every asset in the level comes from
//! (its author, source and license), the font, and the libraries the level is built on.
//!
//! Interface: `CREDITS_LINES`.
//!
//! Example: `lines.extend(CREDITS_LINES.iter().map(|line| (line.to_string(), false)));`
//!
//! Must know:
//! - Every asset added to the level gets its lines here, as the README's Assets table does.
//! - Each line fits the menu's width, 64 characters (`MENU_PICTURE_PX` over `GLYPH_PX`), which
//!   `menu`'s tests check.

/// `CREDITS_LINES` is the CREDITS page's text, one menu line each.
pub const CREDITS_LINES: [&str; 22] = [
    "Warped: Dark Forest",
    "",
    "SURFACES  Poly Haven textures, CC0 (polyhaven.com/a/<name>)",
    "  forest_ground_04 (Rob Tuytel, Rico Cilliers), bamboo_wall",
    "  (Amal Kumar), old_planks_02, forest_leaves_02, mossy_rock",
    "  (Rob Tuytel)",
    "SKY       kloofendal_48d_partly_cloudy_puresky, Poly Haven, CC0",
    "ROBOTS    RobotExpressive by Tomas Laulhe, modified by",
    "  Don McCurdy, CC0 (three.js examples)",
    "RIFLE     bolt_action_rifle_7_62, Mateusz Sadek, Poly Haven, CC0",
    "REVOLVER  \"Sci-fi revolver\" by Ramhat, Sketchfab Standard",
    "  sketchfab.com/3d-models/none-f6bd1cbad4174048bcf02f8e52771adb",
    "ARMS      \"FPS- Arms\" by 3DHaupt, CC BY-NC 4.0",
    "  sketchfab.com/3d-models/none-22274e479c794377b58671f1652f61fc",
    "TEXT      SDL3's built-in 8x8 debug font (libsdl.org, zlib)",
    "CODE      Rust crates: sdl3, glam, gltf, image, rayon, clap,",
    "  objc2, objc2-metal, serde, hmac, sha2, getrandom, ureq:",
    "  each under its own open-source license",
    "",
    "The forest, treehouses, ladders, bridges and consoles are",
    "made in code in this repository (common/maps/dark_forest).",
    "",
];
