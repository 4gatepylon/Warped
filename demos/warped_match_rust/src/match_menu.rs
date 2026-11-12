//! `match_menu.rs` brings in the match's progression page, which moved to
//! `warped_shared::match_menu` so the one-window app (`../warped_mvp_rust/`) shares it; this file
//! keeps `crate::match_menu` naming it for the rest of this crate.
//!
//! Interface: everything public in `warped_shared::match_menu` (`get_progression_shapes`,
//! `Shape`, `PAGE_SIZE`, ...).
//!
//! Example: `let shapes = crate::match_menu::get_progression_shapes(&view, None);`

pub use warped_shared::match_menu::*;
