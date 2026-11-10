//! `play_trace.rs` records a play trace: a player walked by a fixed script of keys at 60 steps a
//! second on open flat ground, through `Player::update`, with every step's state written exactly
//! (`{:?}` of each `f64` is its shortest exact form). The script and the movement rules' fields
//! (energy, sprint, bob, slowed, crouch, forward speed) are written as Lunar Surface's own
//! `play_trace.rs` writes them, so the two levels' traces can be compared field by field
//! (W-126 (same play in every level)), and two builds' traces byte for byte.
//!
//! Interface: the ignored test `record_play_trace`, run with `PLAY_TRACE_FILE` (where to write the
//! trace).
//!
//! Example: `PLAY_TRACE_FILE=/tmp/forest.txt cargo test --release record_play_trace -- --ignored`

use std::fmt::Write as _;

use glam::{Quat, Vec2, Vec3};

use super::collision::Aabb;
use super::player::{Movement, Player, PlayerInput};
use crate::map::{self, MapFile};
use crate::settings::ServerSettings;

const STEP_S: f32 = 1.0 / 60.0;
const SLOWED_AFTER_HIT_S: f64 = 1.0;
const GROUND_HALF_M: f32 = 500.0;
const GROUND_DEPTH_M: f32 = 1.0;
const TURN_RAD_PER_STEP: f32 = 0.004;

/// `Leg` is one part of the script: which keys are held, for how many seconds, and whether a hit
/// lands at its start.
struct Leg {
    keys: &'static str,
    duration_s: f32,
    is_hit: bool,
}

/// `SCRIPT` is Lunar Surface's play trace script: it walks, sprints until out of energy,
/// crouches, strafes, jumps, stands, backs off, is hit and turns; each letter is a key: `w`, `a`,
/// `s`, `d`, `j` (jump), `r` (run), `c` (crouch).
const SCRIPT: [Leg; 11] = [
    Leg {
        keys: "",
        duration_s: 0.5,
        is_hit: false,
    },
    Leg {
        keys: "w",
        duration_s: 2.0,
        is_hit: false,
    },
    Leg {
        keys: "wr",
        duration_s: 9.0,
        is_hit: false,
    },
    Leg {
        keys: "wc",
        duration_s: 2.0,
        is_hit: false,
    },
    Leg {
        keys: "wd",
        duration_s: 1.0,
        is_hit: false,
    },
    Leg {
        keys: "wj",
        duration_s: 1.5,
        is_hit: false,
    },
    Leg {
        keys: "",
        duration_s: 1.0,
        is_hit: false,
    },
    Leg {
        keys: "sa",
        duration_s: 1.0,
        is_hit: false,
    },
    Leg {
        keys: "wrj",
        duration_s: 1.0,
        is_hit: false,
    },
    Leg {
        keys: "w",
        duration_s: 1.5,
        is_hit: true,
    },
    Leg {
        keys: "dc",
        duration_s: 1.0,
        is_hit: false,
    },
];

/// `get_input` gives the controls of `keys`.
fn get_input(keys: &str) -> PlayerInput {
    let axis = |plus: char, minus: char| -> f32 {
        f32::from(u8::from(keys.contains(plus))) - f32::from(u8::from(keys.contains(minus)))
    };
    PlayerInput {
        move_axes: Vec2::new(axis('d', 'a'), axis('w', 's')),
        look_delta_rad: Vec2::ZERO,
        is_sprint_held: keys.contains('r'),
        is_crouch_held: keys.contains('c'),
        is_jump_pressed: keys.contains('j'),
    }
}

#[test]
#[ignore]
fn record_play_trace() {
    let path: String =
        std::env::var("PLAY_TRACE_FILE").expect("PLAY_TRACE_FILE names the trace file");
    let map: MapFile = map::load(&map::default_map_path()).expect("the map loads");
    let movement: Movement = Movement::New(
        &map.player,
        map.gravity_m_per_s2,
        &ServerSettings::Builtin(),
    );
    let solids: [Aabb; 1] = [Aabb {
        min_m: Vec3::new(-GROUND_HALF_M, -GROUND_DEPTH_M, -GROUND_HALF_M),
        max_m: Vec3::new(GROUND_HALF_M, 0.0, GROUND_HALF_M),
    }];
    let mut player: Player = Player::new(Vec3::ZERO, 0.0);
    player.is_on_ground = true;
    let mut trace: String = String::new();
    let mut step: usize = 0;
    for leg in &SCRIPT {
        if leg.is_hit {
            player.moves.slowed_s = SLOWED_AFTER_HIT_S;
        }
        let mut input: PlayerInput = get_input(leg.keys);
        for _ in 0..(leg.duration_s / STEP_S).round() as usize {
            input.look_delta_rad.x = if leg.keys.contains('d') {
                TURN_RAD_PER_STEP
            } else {
                0.0
            };
            player.update(&input, &movement, &solids, &[], None, STEP_S);
            let facing: Vec3 = Quat::from_rotation_y(player.yaw_rad) * Vec3::NEG_Z;
            let moves = player.moves;
            writeln!(
                trace,
                "{step} feet={:?} ground={} energy={:?} sprint={} bob={:?} {:?} slowed={:?} crouch={:?} forward={:?}",
                player.feet_m.to_array(),
                player.is_on_ground,
                moves.energy_frac,
                moves.is_sprinting,
                moves.bob_phase_rad,
                moves.bob_frac,
                moves.slowed_s,
                moves.crouch_frac,
                f64::from(player.velocity_m_s.with_y(0.0).dot(facing)),
            )
            .expect("a string takes writes");
            step += 1;
        }
    }
    std::fs::write(&path, trace).expect("the trace file is written");
}
