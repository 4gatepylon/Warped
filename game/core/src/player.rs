//! `player.rs` holds the movement keys a player holds, which `voxel_world.rs` moves the player by.
//!
//! Interface: `Input`.
//!
//! Example: `let input = Input { forward: true, ..Input::default() };`

/// `Input` is which movement keys are held this frame.
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
    pub crouch: bool,
}
