/**
 * `pull_dynamics.js` is the proposed pull of wormhole mouths on a player and the proposed sprint (W-202, pull toward
 * wormholes and black holes in the game settings), on a flat floor seen from above, stepped at a fixed 120 times a
 * second so that the same keys always give the same path. The page (`index.html`) plays it and records it; the levels'
 * Rust code is to follow the same equations, and a recording made here is the check that it does.
 *
 * Interface: `DT_S`, `DEFAULT_SETTINGS`, `SETTING_RANGES`, `PROFILES`, `make_state`, `get_mouths`, `get_pull`,
 * `step_player`, `get_fall_speed_m_per_s`, `find_walk_escape_m`, `find_period_s`.
 *
 * Example: `let state = make_state(DEFAULT_SETTINGS); step_player(state, {dir: [1, 0], is_sprint_held: false}, DEFAULT_SETTINGS);`
 *
 * Must know:
 * - The equations, with `s` the legs' speed, `u` the drift the mouths give, `h` the keys' unit direction:
 *   - `s` rises to the sprint speed at `sprint_up_m_per_s_per_s` while sprinting and falls back to the walking speed at
 *     `sprint_down_m_per_s_per_s` otherwise; walking itself starts and stops at once, as today.
 *   - Each mouth pulls toward its centre with `a(d)` (the profile) while the player is between its throat and
 *     `pull_radius_m` from its centre; `du/dt` is the sum of the pulls. Outside every radius `|u|` falls at
 *     `drift_brake_m_per_s_per_s`.
 *   - The player moves at `s h + u`. Crossing a throat inward puts the player at the opposite point of the other
 *     mouth's throat with `s` and `u` unchanged, so it is thrown out as fast as it went in.
 */

export const DT_S = 1 / 120;
const MIN_LENGTH = 1e-12;
const MAX_SEARCH_S = 120;
const SEARCH_STEP_COUNT = 40;

/** `PROFILES` is how a mouth's pull depends on the distance from its centre, between the throat (0) and the radius (1). */
export const PROFILES = {
    constant: (_edge_frac) => 1,
    stronger_near_mouth: (edge_frac) => 1 - edge_frac,
    harmonic: (edge_frac) => edge_frac,
};

/** `DEFAULT_SETTINGS` is the proposal: Snow Planet's speeds, a radius of 2.5 bodies of 2 m, and a pull that gives
 * 1.5 times today's strongest pull speed (0.75 of walking speed) by the time a player falls from the radius to the
 * throat. */
export const DEFAULT_SETTINGS = {
    pull_radius_m: 5.0,
    pull_m_per_s_per_s: 11.9,
    pull_profile: "constant",
    throat_radius_m: 1.6,
    walk_m_per_s: 8.0,
    sprint_m_per_s: 15.5,
    sprint_up_m_per_s_per_s: 15.0,
    sprint_down_m_per_s_per_s: 15.0,
    drift_brake_m_per_s_per_s: 15.0,
    energy_drain_s: 7.5,
    energy_refill_s: 15.0,
    mouth_gap_m: 24.0,
};

/** `SETTING_RANGES` is each numeric setting's label, lowest, highest and step on the page. */
export const SETTING_RANGES = {
    pull_radius_m: ["Pull radius (m from the mouth's centre)", 2, 15, 0.1],
    pull_m_per_s_per_s: ["Pull strength (m/s²)", 0, 40, 0.1],
    throat_radius_m: ["Mouth's throat radius (m)", 0.5, 3, 0.1],
    walk_m_per_s: ["Walking speed (m/s)", 2, 12, 0.1],
    sprint_m_per_s: ["Sprint top speed (m/s)", 4, 24, 0.1],
    sprint_up_m_per_s_per_s: ["Sprint speeding up (m/s²)", 1, 60, 0.5],
    sprint_down_m_per_s_per_s: ["Sprint slowing down (m/s²)", 1, 60, 0.5],
    drift_brake_m_per_s_per_s: ["Drift braking outside the radius (m/s²)", 0, 60, 0.5],
    energy_drain_s: ["Sprint energy lasts (s)", 1, 20, 0.5],
    energy_refill_s: ["Sprint energy refills in (s)", 1, 30, 0.5],
    mouth_gap_m: ["Distance between the mouths (m)", 12, 40, 1],
};

/**
 * `get_mouths` gives the two linked mouths' centres, `mouth_gap_m` apart on the x axis.
 *
 * Example: `const [purple, green] = get_mouths(settings);`
 */
export function get_mouths(settings) {
    return [
        [-settings.mouth_gap_m / 2, 0],
        [settings.mouth_gap_m / 2, 0],
    ];
}

/**
 * `make_state` gives a player standing still with full energy, left of the purple mouth and outside its radius.
 *
 * Example: `const state = make_state(DEFAULT_SETTINGS);`
 */
export function make_state(settings) {
    return {
        tick: 0,
        x: -settings.mouth_gap_m / 2 - settings.pull_radius_m - 3,
        y: 3,
        speed_cap_m_per_s: settings.walk_m_per_s,
        u: [0, 0],
        v: [0, 0],
        energy_frac: 1,
        is_sprinting: false,
        pass_count: 0,
    };
}

/**
 * `get_pull` gives the mouths' pull at (`x`, `y`) in m/s² and whether the point is inside some mouth's radius.
 *
 * Example: `const { pull, is_in_reach } = get_pull(-14, 0, DEFAULT_SETTINGS);`
 */
export function get_pull(x, y, settings) {
    const profile = PROFILES[settings.pull_profile];
    if (profile === undefined) {
        throw new Error(`pull_profile ${settings.pull_profile} isn't one of ${Object.keys(PROFILES).join(", ")}`);
    }
    const pull = [0, 0];
    let is_in_reach = false;
    for (const [cx, cy] of get_mouths(settings)) {
        const [dx, dy] = [cx - x, cy - y];
        const distance_m = Math.hypot(dx, dy);
        if (distance_m > settings.pull_radius_m) {
            continue;
        }
        is_in_reach = true;
        if (distance_m <= settings.throat_radius_m || distance_m < MIN_LENGTH) {
            continue;
        }
        const edge_frac =
            (distance_m - settings.throat_radius_m) / (settings.pull_radius_m - settings.throat_radius_m);
        const strength = settings.pull_m_per_s_per_s * profile(edge_frac);
        pull[0] += (dx / distance_m) * strength;
        pull[1] += (dy / distance_m) * strength;
    }
    return { pull, is_in_reach };
}

/**
 * `step_player` moves `state` on by `DT_S` under `input` (`dir`: the move keys' direction, each part -1, 0 or 1;
 * `is_sprint_held`) and `settings`, in place.
 *
 * Example: `step_player(state, { dir: [0, -1], is_sprint_held: true }, settings);`
 */
export function step_player(state, input, settings) {
    const length = Math.hypot(input.dir[0], input.dir[1]);
    const is_moving = length > 0;
    const h = is_moving ? [input.dir[0] / length, input.dir[1] / length] : [0, 0];
    const wants_to_sprint = input.is_sprint_held && is_moving;
    state.is_sprinting = wants_to_sprint && state.energy_frac > 0;
    const energy_per_s = wants_to_sprint ? -1 / settings.energy_drain_s : 1 / settings.energy_refill_s;
    state.energy_frac = Math.min(1, Math.max(0, state.energy_frac + energy_per_s * DT_S));

    const target_m_per_s = state.is_sprinting ? settings.sprint_m_per_s : settings.walk_m_per_s;
    if (state.speed_cap_m_per_s < target_m_per_s) {
        state.speed_cap_m_per_s = Math.min(
            target_m_per_s,
            state.speed_cap_m_per_s + settings.sprint_up_m_per_s_per_s * DT_S,
        );
    } else {
        state.speed_cap_m_per_s = Math.max(
            target_m_per_s,
            state.speed_cap_m_per_s - settings.sprint_down_m_per_s_per_s * DT_S,
        );
    }

    const { pull, is_in_reach } = get_pull(state.x, state.y, settings);
    if (is_in_reach) {
        state.u[0] += pull[0] * DT_S;
        state.u[1] += pull[1] * DT_S;
    } else {
        const drift_m_per_s = Math.hypot(state.u[0], state.u[1]);
        const braked_m_per_s = Math.max(0, drift_m_per_s - settings.drift_brake_m_per_s_per_s * DT_S);
        const keep_frac = drift_m_per_s > MIN_LENGTH ? braked_m_per_s / drift_m_per_s : 0;
        state.u[0] *= keep_frac;
        state.u[1] *= keep_frac;
    }

    state.v = [h[0] * state.speed_cap_m_per_s + state.u[0], h[1] * state.speed_cap_m_per_s + state.u[1]];
    state.x += state.v[0] * DT_S;
    state.y += state.v[1] * DT_S;

    const mouths = get_mouths(settings);
    for (let i = 0; i < mouths.length; i++) {
        const [ox, oy] = [state.x - mouths[i][0], state.y - mouths[i][1]];
        const is_inside = Math.hypot(ox, oy) < settings.throat_radius_m;
        const is_going_in = ox * state.v[0] + oy * state.v[1] < 0;
        if (is_inside && is_going_in) {
            const other = mouths[1 - i];
            state.x = other[0] - ox;
            state.y = other[1] - oy;
            state.pass_count += 1;
            break;
        }
    }
    state.tick += 1;
}

/**
 * `get_fall_speed_m_per_s` gives the speed a player standing at a mouth's radius has gained when it reaches the
 * throat: the square root of twice the pull's work over that stretch.
 *
 * Example: `const speed = get_fall_speed_m_per_s(DEFAULT_SETTINGS);`
 */
export function get_fall_speed_m_per_s(settings) {
    const stretch_m = Math.max(0, settings.pull_radius_m - settings.throat_radius_m);
    const mean_frac = settings.pull_profile === "constant" ? 1 : 0.5;
    return Math.sqrt(2 * settings.pull_m_per_s_per_s * mean_frac * stretch_m);
}

/**
 * `does_walk_out` says whether a player starting at rest `distance_m` left of the purple mouth's centre, holding
 * the key away from it, gets out of its radius (rather than being pulled through).
 */
function does_walk_out(distance_m, settings) {
    const state = make_state(settings);
    [state.x, state.y] = [get_mouths(settings)[0][0] - distance_m, 0];
    for (let t = 0; t < MAX_SEARCH_S; t += DT_S) {
        step_player(state, { dir: [-1, 0], is_sprint_held: false }, settings);
        if (state.pass_count > 0) {
            return false;
        }
        if (get_mouths(settings)[0][0] - state.x > settings.pull_radius_m) {
            return true;
        }
    }
    return false;
}

/**
 * `find_walk_escape_m` gives the closest distance to a mouth's centre from which a player at rest can still walk
 * straight out of its pull, or `null` if walking gets out from nowhere inside the radius.
 *
 * Example: `const closest_m = find_walk_escape_m(DEFAULT_SETTINGS);`
 */
export function find_walk_escape_m(settings) {
    let [low_m, high_m] = [settings.throat_radius_m, settings.pull_radius_m];
    if (!does_walk_out(high_m - 1e-6, settings)) {
        return null;
    }
    if (does_walk_out(low_m + 1e-6, settings)) {
        return low_m;
    }
    for (let i = 0; i < SEARCH_STEP_COUNT; i++) {
        const middle_m = (low_m + high_m) / 2;
        if (does_walk_out(middle_m, settings)) {
            high_m = middle_m;
        } else {
            low_m = middle_m;
        }
    }
    return high_m;
}

/**
 * `find_period_s` gives how long a player who stands just inside a mouth's radius and touches nothing takes to come
 * back to where it started (through both mouths and back), or `null` if it doesn't within two minutes.
 *
 * Example: `const period_s = find_period_s(DEFAULT_SETTINGS);`
 */
export function find_period_s(settings) {
    const state = make_state(settings);
    const start_x = get_mouths(settings)[0][0] - settings.pull_radius_m + 0.01;
    [state.x, state.y] = [start_x, 0];
    for (let t = DT_S; t < MAX_SEARCH_S; t += DT_S) {
        const before = state.v[0];
        step_player(state, { dir: [0, 0], is_sprint_held: false }, settings);
        const is_back = state.pass_count >= 2 && state.pass_count % 2 === 0 && before < 0 && state.v[0] >= 0;
        if (is_back) {
            return t;
        }
    }
    return null;
}
