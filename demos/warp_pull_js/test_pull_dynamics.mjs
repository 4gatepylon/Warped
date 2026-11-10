/**
 * `test_pull_dynamics.mjs` tests `pull_dynamics.js` with Node's test runner (`make test`).
 *
 * | Partitioning | Parts | Tests |
 * | --- | --- | --- |
 * | Pull by place | outside every radius (none, not in reach), between throat and radius (toward the centre, the profile's size), inside the throat (none, in reach) | `the pull is inside the radius only` |
 * | Profile | constant, stronger near the mouth, harmonic; an unknown name (fails) | `the pull is inside the radius only`, `profiles differ in where they pull hardest` |
 * | Standing start inside a radius | touching nothing: through both mouths and back to where it started, as fast out as in | `a player who touches nothing swings back to where it started` |
 * | Walking out | from near the edge (gets out), from beside the throat (pulled through); the closest place found | `walking gets out only from far enough` |
 * | Sprint | Shift held (the speed ramps up to the top), let go (ramps down at once), no energy (ramps down) | `sprinting ramps up and down` |
 * | Drift outside the radius | a player thrown out stops drifting | `drift is braked outside the radius` |
 * | Determinism | the same keys twice give the same path, bit for bit | `the same keys give the same path` |
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
    DEFAULT_SETTINGS,
    DT_S,
    find_period_s,
    find_walk_escape_m,
    get_fall_speed_m_per_s,
    get_mouths,
    get_pull,
    make_state,
    step_player,
} from "./pull_dynamics.js";

const STILL = { dir: [0, 0], is_sprint_held: false };

test("the pull is inside the radius only", () => {
    const settings = DEFAULT_SETTINGS;
    const [purple] = get_mouths(settings);
    const far = get_pull(purple[0] - settings.pull_radius_m - 0.1, 0, settings);
    assert.deepEqual(far, { pull: [0, 0], is_in_reach: false });
    const near = get_pull(purple[0] - 3, 0, settings);
    assert.ok(near.is_in_reach);
    assert.ok(Math.abs(near.pull[0] - settings.pull_m_per_s_per_s) < 1e-12 && near.pull[1] === 0);
    const above = get_pull(purple[0], 3, settings);
    assert.ok(Math.abs(above.pull[1] + settings.pull_m_per_s_per_s) < 1e-12);
    const inside = get_pull(purple[0] - 1, 0, settings);
    assert.deepEqual(inside, { pull: [0, 0], is_in_reach: true });
    assert.throws(() => get_pull(0, 0, { ...settings, pull_profile: "sideways" }), /pull_profile/);
});

test("profiles differ in where they pull hardest", () => {
    const [purple] = get_mouths(DEFAULT_SETTINGS);
    const pull_at = (profile, distance_m) =>
        get_pull(purple[0] - distance_m, 0, { ...DEFAULT_SETTINGS, pull_profile: profile }).pull[0];
    assert.ok(pull_at("stronger_near_mouth", 2) > pull_at("stronger_near_mouth", 4.5));
    assert.ok(pull_at("harmonic", 2) < pull_at("harmonic", 4.5));
    assert.equal(pull_at("constant", 2), pull_at("constant", 4.5));
});

test("a player who touches nothing swings back to where it started", () => {
    const settings = DEFAULT_SETTINGS;
    const [purple, green] = get_mouths(settings);
    const state = make_state(settings);
    const start_m = 4;
    [state.x, state.y] = [purple[0] - start_m, 0];
    let top_m_per_s = 0;
    let farthest_right_m = 0;
    while (state.pass_count < 2) {
        step_player(state, STILL, settings);
        top_m_per_s = Math.max(top_m_per_s, Math.hypot(...state.v));
        if (state.pass_count === 1) {
            farthest_right_m = Math.max(farthest_right_m, state.x - green[0]);
        }
    }
    const expected_m_per_s = Math.sqrt(2 * settings.pull_m_per_s_per_s * (start_m - settings.throat_radius_m));
    assert.ok(Math.abs(top_m_per_s - expected_m_per_s) < 0.2, `${top_m_per_s} vs ${expected_m_per_s}`);
    assert.ok(Math.abs(farthest_right_m - start_m) < 0.2, `${farthest_right_m}`);
    const period_s = find_period_s(settings);
    const stretch_m = settings.pull_radius_m - settings.throat_radius_m;
    assert.ok(Math.abs(period_s - 4 * Math.sqrt((2 * stretch_m) / settings.pull_m_per_s_per_s)) < 0.1, `${period_s}`);
    assert.ok(Math.abs(get_fall_speed_m_per_s(settings) - 9) < 0.01);
});

test("walking gets out only from far enough", () => {
    const settings = DEFAULT_SETTINGS;
    const closest_m = find_walk_escape_m(settings);
    const expected_m = settings.pull_radius_m - settings.walk_m_per_s ** 2 / (2 * settings.pull_m_per_s_per_s);
    assert.ok(Math.abs(closest_m - expected_m) < 0.05, `${closest_m} vs ${expected_m}`);
    assert.equal(find_walk_escape_m({ ...settings, pull_m_per_s_per_s: 1 }), settings.throat_radius_m);
    assert.equal(find_walk_escape_m({ ...settings, pull_m_per_s_per_s: 4000 }), null);
});

test("sprinting ramps up and down", () => {
    const settings = DEFAULT_SETTINGS;
    const state = make_state(settings);
    const sprint = { dir: [-1, 0], is_sprint_held: true };
    const walk = { dir: [-1, 0], is_sprint_held: false };
    step_player(state, sprint, settings);
    assert.ok(state.speed_cap_m_per_s > settings.walk_m_per_s && state.speed_cap_m_per_s < settings.sprint_m_per_s);
    const ramp_s = (settings.sprint_m_per_s - settings.walk_m_per_s) / settings.sprint_up_m_per_s_per_s;
    for (let t = 0; t < ramp_s; t += DT_S) {
        step_player(state, sprint, settings);
    }
    assert.equal(state.speed_cap_m_per_s, settings.sprint_m_per_s);
    step_player(state, walk, settings);
    assert.ok(state.speed_cap_m_per_s < settings.sprint_m_per_s);
    for (let t = 0; t < 2; t += DT_S) {
        step_player(state, walk, settings);
    }
    assert.equal(state.speed_cap_m_per_s, settings.walk_m_per_s);
    state.energy_frac = 0;
    step_player(state, sprint, settings);
    assert.ok(!state.is_sprinting && state.speed_cap_m_per_s === settings.walk_m_per_s);
});

test("drift is braked outside the radius", () => {
    const settings = DEFAULT_SETTINGS;
    const state = make_state(settings);
    state.u = [-6, 0];
    step_player(state, STILL, settings);
    assert.ok(Math.hypot(...state.u) < 6);
    for (let t = 0; t < 1; t += DT_S) {
        step_player(state, STILL, settings);
    }
    assert.equal(Math.hypot(...state.u), 0);
});

test("the same keys give the same path", () => {
    const run = () => {
        const state = make_state(DEFAULT_SETTINGS);
        for (let tick = 0; tick < 1200; tick++) {
            const input = { dir: [1, tick % 240 < 120 ? -1 : 0], is_sprint_held: tick > 300 };
            step_player(state, input, DEFAULT_SETTINGS);
        }
        return state;
    };
    assert.deepEqual(run(), run());
});
