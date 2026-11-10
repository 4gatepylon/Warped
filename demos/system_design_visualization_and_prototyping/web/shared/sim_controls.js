/**
 * @file
 * `sim_controls.js` puts the simulation controls at the top of a page: the yellow band that steers the simulated
 * world and that the real game will not have.
 *
 * Interface:
 * - Include it after `api.js`. It adds `<div id="sim">` as the page's first element and refreshes it every second.
 * - The band skips the clock, sets how many simulated players are online, shows the caps, the wait queue and the
 *   fleet's cost so far, and links the prototype's pages.
 *
 * Example: <script src="../shared/api.js"></script><script src="../shared/sim_controls.js"></script>
 *
 * Must know:
 * - Gotchas: the band is the only place (with the system view's FAIL buttons) that calls `/api/simulation/`.
 */

const SIM_REFRESH_MS = 1000;
const CLOCK_SKIPS = [['+1 MIN', 60], ['+10 MIN', 600], ['+1 H', 3600], ['+1 DAY', 86400], ['+1 WEEK', 604800]];
const PLAYER_PRESETS = [0, 50, 200, 1000];

/**
 * `draw_sim_controls` writes the band's buttons and readouts into `band`, once.
 *
 * @example draw_sim_controls(document.getElementById('sim'));
 */
function draw_sim_controls(band) {
  const skips = CLOCK_SKIPS.map(([label, seconds]) => `<button class="btn small" data-skip-s="${seconds}">${label}</button>`);
  const presets = PLAYER_PRESETS.map((count) => `<button class="btn small" data-players="${count}">${count}</button>`);
  band.innerHTML = `<b>SIMULATION CONTROLS</b><span>not part of the game</span>
    <span id="sim_clock"></span>${skips.join('')}
    <span>Players online</span><input id="sim_players" type="number" min="0">
    <button class="btn small" id="sim_players_set">SET</button>${presets.join('')}
    <span id="sim_readout"></span>
    <span class="right"><a href="/game/">GAME APP</a> <a href="/site/">WEBSITE</a>
      <a href="/system/">SYSTEM VIEW</a> <a href="/docs">API</a></span>
    <span id="sim_error" class="red"></span>`;
}

/**
 * `refresh_sim_controls` reads the world's state and shows the clock, the players online and the caps.
 *
 * @example setInterval(refresh_sim_controls, SIM_REFRESH_MS);
 */
async function refresh_sim_controls() {
  const system = await call_api('GET', '/api/simulation/system');
  const { caps, players, cost, wait_queue_count, machines } = system;
  document.getElementById('sim_clock').textContent = format_clock(system.now_s);
  const player_box = document.getElementById('sim_players');
  if (document.activeElement !== player_box) {
    player_box.value = system.settings.find((setting) => setting.name === 'online_player_target').value;
  }
  document.getElementById('sim_readout').textContent =
    `online ${players.online_count} | matches ${caps.match_count}/${caps.max_match_count}` +
    ` | players in matches ${caps.player_count}/${caps.max_player_count} | wait queue ${wait_queue_count}` +
    ` | machines ${machines.length} | fleet cost so far $${cost.usd.toFixed(2)}`;
  document.dispatchEvent(new CustomEvent('system_refreshed', { detail: system }));
}

/**
 * `run_sim_action` runs one of the band's requests, then refreshes it and shows what went wrong, if anything.
 *
 * @example run_sim_action(() => call_api('POST', '/api/simulation/clock/skip', { seconds: 60 }));
 */
async function run_sim_action(send_request) {
  const error_box = document.getElementById('sim_error');
  error_box.textContent = '';
  try {
    await send_request();
    await refresh_sim_controls();
  } catch (error) {
    error_box.textContent = error.message;
  }
}

/**
 * `start_sim_controls` adds the band to the page, wires its buttons and keeps it fresh.
 *
 * @example start_sim_controls();
 */
function start_sim_controls() {
  const band = document.createElement('div');
  band.id = 'sim';
  document.body.prepend(band);
  draw_sim_controls(band);
  const set_players = (count) => call_api('PUT', '/api/settings/online_player_target', { value: Number(count) });
  band.addEventListener('click', (event) => {
    const button = event.target.closest('button');
    if (button === null) {
      return;
    }
    if ('skipS' in button.dataset) {
      run_sim_action(() => call_api('POST', '/api/simulation/clock/skip', { seconds: Number(button.dataset.skipS) }));
    } else if ('players' in button.dataset) {
      run_sim_action(() => set_players(button.dataset.players));
    } else if (button.id === 'sim_players_set') {
      run_sim_action(() => set_players(document.getElementById('sim_players').value));
    }
  });
  run_sim_action(async () => {});
  setInterval(() => run_sim_action(async () => {}), SIM_REFRESH_MS);
}

start_sim_controls();
