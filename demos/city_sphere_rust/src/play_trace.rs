//! `play_trace.rs` records a play trace: a player walked by a fixed script of keys at 60 steps a
//! second from a spawn of a voxel map, through `VoxelPlayer::step`, with every step's state
//! written exactly (`{:?}` of each `f64` is its shortest exact form), so two builds' traces can
//! be compared byte for byte to show a change left play the same. `record_city_play_trace` does
//! the same on the City Sphere map, through `CityWorld::move_player`.
//!
//! Interface: the ignored tests `record_play_trace`, run with `PLAY_TRACE_MAP` (a `--map` name)
//! and `PLAY_TRACE_FILE` (where to write the trace), and `record_city_play_trace`, run with
//! `PLAY_TRACE_FILE`, and `PLAY_TRACE_CROUCH=0` or `PLAY_TRACE_HIT=0` to leave out the script's
//! crouching or its hit.
//!
//! Example: `PLAY_TRACE_MAP=dust PLAY_TRACE_FILE=/tmp/dust.txt cargo test --release record_play_trace -- --ignored`

use std::fmt::Write as _;

use clap::ValueEnum;

use crate::map::MapName;
use crate::physics::Input;
use crate::settings::ServerSettings;
use crate::voxel_world::{VoxelPlayer, VoxelWorld};

const STEP_S: f64 = 1.0 / 60.0;
const SLOWED_AFTER_HIT_S: f64 = 1.0;

/// `Leg` is one part of the script: which keys are held, for how many seconds, and whether a hit
/// lands at its start.
struct Leg {
    keys: &'static str,
    duration_s: f64,
    is_hit: bool,
}

/// `SCRIPT` walks, sprints until out of energy, crouches, strafes, jumps, stands, backs off, is
/// hit and turns; each letter is a key: `w`, `a`, `s`, `d`, `j` (jump), `r` (run), `c` (crouch).
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
const TURN_PX_PER_STEP: f64 = 4.0;

/// `get_input` gives the movement keys of `keys`.
fn get_input(keys: &str) -> Input {
    Input {
        forward: keys.contains('w'),
        back: keys.contains('s'),
        left: keys.contains('a'),
        right: keys.contains('d'),
        up: keys.contains('j'),
        down: false,
        sprint: keys.contains('r'),
        crouch: keys.contains('c'),
    }
}

/// `describe` gives one step's state of `player` exactly.
fn describe(step: usize, player: &VoxelPlayer, forward_m_per_s: f64) -> String {
    format!(
        "{step} body={:?} h={:?} pitch={:?} vz={:?} flying={} ground={} energy={:?} sprint={} bob={:?} {:?} slowed={:?} crouch={:?} forward={:?}\n",
        player.body.to_cols_array(),
        player.h_m,
        player.pitch_rad,
        player.vertical_m_per_s,
        player.is_flying,
        player.is_on_ground,
        player.energy_frac,
        player.is_sprinting,
        player.bob_phase_rad,
        player.bob_frac,
        player.slowed_s,
        player.crouch_frac,
        forward_m_per_s,
    )
}

#[test]
#[ignore]
fn record_play_trace() {
    let name: String = std::env::var("PLAY_TRACE_MAP").expect("PLAY_TRACE_MAP names a --map");
    let path: String =
        std::env::var("PLAY_TRACE_FILE").expect("PLAY_TRACE_FILE names the trace file");
    let map_name: MapName = MapName::from_str(&name, true).expect("a --map name");
    let settings: ServerSettings = ServerSettings::Builtin();
    let map = crate::load_map(map_name, &settings).expect("the map");
    let mut world: VoxelWorld = VoxelWorld::FromMap(&map).expect("a voxel map");
    world.set_settings(settings);
    let mut player: VoxelPlayer = VoxelPlayer::AtSpawn(&world, 0);
    let mut trace: String = String::new();
    let mut step: usize = 0;
    for leg in &SCRIPT {
        if leg.is_hit {
            player.slowed_s = SLOWED_AFTER_HIT_S;
        }
        let input: Input = get_input(leg.keys);
        for _ in 0..(leg.duration_s / STEP_S).round() as usize {
            if leg.keys.contains('d') {
                player.look(TURN_PX_PER_STEP, 0.0);
            }
            let report = player.step(&world, &input, STEP_S, 0);
            trace.push_str(&describe(step, &player, report.forward_m_per_s));
            for message in report.messages {
                writeln!(trace, "{step} message {message}").expect("a string takes writes");
            }
            step += 1;
        }
    }
    std::fs::write(&path, trace).expect("the trace file is written");
}

/// `TURN_RAD_PER_STEP` is how far City Sphere's trace turns the body each step a `d` leg holds.
const TURN_RAD_PER_STEP: f64 = 0.004;

/// `get_city_input` gives City Sphere's movement keys of `keys`, as the window reads them.
fn get_city_input(keys: &str) -> crate::city_world::CityInput {
    let axis = |plus: char, minus: char| -> f64 {
        f64::from(u8::from(keys.contains(plus))) - f64::from(u8::from(keys.contains(minus)))
    };
    crate::city_world::CityInput {
        forward_frac: axis('w', 's'),
        right_frac: axis('d', 'a'),
        is_sprinting: keys.contains('r'),
        is_jumping: keys.contains('j'),
        is_crouching: keys.contains('c'),
    }
}

#[test]
#[ignore]
fn record_city_play_trace() {
    let path: String =
        std::env::var("PLAY_TRACE_FILE").expect("PLAY_TRACE_FILE names the trace file");
    let is_crouch_kept: bool = std::env::var("PLAY_TRACE_CROUCH").as_deref() != Ok("0");
    let is_hit_kept: bool = std::env::var("PLAY_TRACE_HIT").as_deref() != Ok("0");
    let world = crate::city_world::CityWorld::Builtin().expect("the built-in map");
    let settings: ServerSettings = ServerSettings::Builtin();
    let bob = crate::settings::PerPlayerSettings::Builtin().view.head_bob;
    let mut player = world.make_player(0, 0);
    let mut trace: String = String::new();
    let mut step: usize = 0;
    for leg in &SCRIPT {
        let keys: String = leg
            .keys
            .chars()
            .filter(|&key| is_crouch_kept || key != 'c')
            .collect();
        if leg.is_hit && is_hit_kept {
            player.move_state.slowed_s = SLOWED_AFTER_HIT_S;
        }
        for _ in 0..(leg.duration_s / STEP_S).round() as usize {
            if keys.contains('d') {
                player.look_around(-TURN_RAD_PER_STEP, 0.0);
            }
            let input = get_city_input(&keys);
            world.move_player(&mut player, &input, &settings, &bob, STEP_S);
            let moves = player.move_state;
            writeln!(
                trace,
                "{step} feet={:?} body={:?} pitch={:?} v={:?} ground={} energy={:?} sprint={} bob={:?} {:?} slowed={:?} eye={:?}",
                player.feet.to_array(),
                player.body.to_cols_array(),
                player.pitch_rad,
                player.velocity.to_array(),
                player.is_on_ground,
                moves.energy_frac,
                moves.is_sprinting,
                moves.bob_phase_rad,
                moves.bob_frac,
                moves.slowed_s,
                player.get_eye_frame().to_cols_array(),
            )
            .expect("a string takes writes");
            step += 1;
        }
    }
    std::fs::write(&path, trace).expect("the trace file is written");
}
