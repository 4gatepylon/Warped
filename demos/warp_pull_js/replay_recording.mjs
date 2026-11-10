/**
 * `replay_recording.mjs` replays a recording the page saved (`recording_<n>.json`, or `approved.json`'s) through
 * `pull_dynamics.js` and says how far the replayed path strays from the recorded one. A replay that matches shows the
 * recording holds everything the dynamics need; the levels' Rust code is checked against recordings the same way.
 *
 * Interface: `node replay_recording.mjs <recording.json>`; prints the ticks, the times through a mouth and the largest
 * distance between the two paths, and exits 1 if that is above 1e-9 m.
 *
 * Example: `node replay_recording.mjs .artifacts/serve_demo/20261110-182035/recording_002.json`
 */
import { readFileSync } from "node:fs";

import { step_player } from "./pull_dynamics.js";

const MAX_STRAY_M = 1e-9;

const [path] = process.argv.slice(2);
if (path === undefined) {
    console.error("usage: node replay_recording.mjs <recording.json>");
    process.exit(2);
}
const file = JSON.parse(readFileSync(path, "utf8"));
const recording = file.recording ?? file;
const bits_of = recording.input_bits;
const state = structuredClone(recording.initial_state);
const samples = new Map(recording.samples.map(([tick, x, y]) => [tick, [x, y]]));
let [next_input, bits, worst_m] = [0, 0, 0];
for (let tick = 0; tick < recording.tick_count; tick++) {
    while (next_input < recording.inputs.length && recording.inputs[next_input][0] === tick) {
        bits = recording.inputs[next_input][1];
        next_input += 1;
    }
    const input = {
        dir: [
            (bits & bits_of.right ? 1 : 0) - (bits & bits_of.left ? 1 : 0),
            (bits & bits_of.up ? 1 : 0) - (bits & bits_of.down ? 1 : 0),
        ],
        is_sprint_held: (bits & bits_of.sprint) !== 0,
    };
    step_player(state, input, recording.settings);
    const recorded = samples.get(tick + 1);
    if (recorded !== undefined) {
        worst_m = Math.max(worst_m, Math.hypot(state.x - recorded[0], state.y - recorded[1]));
    }
}
console.log(
    `${recording.tick_count} ticks, ${state.pass_count - recording.initial_state.pass_count} times through a mouth, ` +
        `${samples.size} samples: the replay strays at most ${worst_m} m from the recording`,
);
process.exit(worst_m <= MAX_STRAY_M ? 0 : 1);
