//! `match_state.rs` brings in the match's rules and state, which moved to
//! `warped_shared::match_state` so the one-window app (`../warped_mvp_rust/`) shares them; this
//! file keeps `crate::match_state` naming them for the rest of this crate.
//!
//! Interface: everything public in `warped_shared::match_state` (`Team`, `MatchState`, ...).
//!
//! Example: `let state = crate::match_state::MatchState::New(5);`

pub use warped_shared::match_state::*;
