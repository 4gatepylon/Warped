//! `play_trace.rs` records a play trace: a player walked by a fixed script of keys at 60 steps a
//! second from a spawn of a voxel map, through `VoxelPlayer::step`, with every step's state
//! written exactly (`{:?}` of each `f64` is its shortest exact form), so two builds' traces can
//! be compared byte for byte to show a change left play the same.
//!
//! Interface: the ignored test `record_play_trace`, run with `PLAY_TRACE_MAP` (a `--map` name)
//! and `PLAY_TRACE_FILE` (where to write the trace), and optionally `PLAY_TRACE_AT` (where to
//! start instead of the first spawn: `<u_m>,<v_m>,<target_u_m>,<target_v_m>`, a map point and the
//! one to face, as `VoxelPlayer::AtView` takes them).
//!
//! Example: `PLAY_TRACE_MAP=dust PLAY_TRACE_FILE=/tmp/dust.txt cargo test --release record_play_trace -- --ignored`;
//! with `PLAY_TRACE_AT=-19,40,-14,40` on `lunar-surface` it starts about 12 m from wormhole
//! mouth A (5 m apart in u, which curved space stretches), facing it, so the trace shows the
//! mouth's pull (W-144): standing drifts in at about 3 m/s, walking in goes 13 to 16 m/s, not 10.

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

/// `get_start` gives where the trace starts on `world`: the first spawn, or the point that
/// `PLAY_TRACE_AT` names, facing its target.
fn get_start(world: &VoxelWorld) -> VoxelPlayer {
    let Ok(at) = std::env::var("PLAY_TRACE_AT") else {
        return VoxelPlayer::AtSpawn(world, 0);
    };
    let numbers_m: Vec<f64> = at
        .split(',')
        .map(|number| number.trim().parse().expect("PLAY_TRACE_AT holds numbers"))
        .collect();
    let [u_m, v_m, target_u_m, target_v_m] = numbers_m[..] else {
        panic!("PLAY_TRACE_AT is <u_m>,<v_m>,<target_u_m>,<target_v_m>, not {at}");
    };
    VoxelPlayer::AtView(world, [u_m, v_m], [target_u_m, target_v_m], None, 0.0)
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
    let mut player: VoxelPlayer = get_start(&world);
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
