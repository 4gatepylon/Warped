/**
 * @file
 * `sim_controls.js` puts the simulation controls at the top of a page: the yellow band that steers the simulated
 * world and that the real game will not have.
 *
 * Interface:
 * - Include it after `api.js` and `session.js`. It adds `<div id="sim">` as the page's first element and refreshes
 *   it every second.
 * - The band skips the clock, sets how many simulated players are online, shows the caps, the wait queue and the
 *   fleet's cost so far, and links the prototype's pages.
 * - Its second line acts on this tab's account: sign in as any account without Google, add finished matches and
 *   time played, refund or dispute the purchase, send Stripe's last webhook again, and pick the country the
 *   connection comes from.
 *
 * Example: <script src="../shared/api.js"></script><script src="../shared/sim_controls.js"></script>
 *
 * Must know:
 * - Gotchas: the band is the only place (with the system view's FAIL buttons) that calls `/api/simulation/`.
 */

const SIM_REFRESH_MS = 1000;
const CLOCK_SKIPS = [['+1 MIN', 60], ['+10 MIN', 600], ['+1 H', 3600], ['+1 DAY', 86400], ['+1 WEEK', 604800]];
const PLAYER_PRESETS = [0, 50, 200, 1000];
const REGION_CHOICES = [['US', 'the US'], ['CA', 'Canada'], ['GB', 'the UK'], ['FR', 'France (EU)'], ['DE', 'Germany (EU)']];
const PLAYED_STEP_S = 600;
const ACCOUNT_ACTIONS = {
  play_match: (account_id) => call_api('POST', `/api/simulation/accounts/${account_id}/play`, { finished_match_count: 1, played_s: 0 }),
  play_time: (account_id) => call_api('POST', `/api/simulation/accounts/${account_id}/play`, { finished_match_count: 0, played_s: PLAYED_STEP_S }),
  refund: (account_id) => call_api('POST', `/api/simulation/accounts/${account_id}/refund`),
  dispute: (account_id) => call_api('POST', `/api/simulation/accounts/${account_id}/dispute`),
};
const sim_state = { account_list_text: '' };

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
    <span class="sim_line"><b>THIS TAB</b><span id="sim_session"></span>
    <span>Act as</span><select id="sim_act_as"></select>
    <button class="btn small" data-account-act="play_match">+1 FINISHED MATCH</button>
    <button class="btn small" data-account-act="play_time">+10 MIN PLAYED</button>
    <button class="btn small" data-account-act="refund">REFUND</button>
    <button class="btn small" data-account-act="dispute">DISPUTE</button>
    <button class="btn small" id="sim_redeliver">SEND LAST WEBHOOK AGAIN</button>
    <span>Connects from</span><select id="sim_region">${REGION_CHOICES.map(([code, label]) =>
      `<option value="${code}">${label}</option>`).join('')}</select></span>
    <span id="sim_error" class="red"></span>`;
  document.getElementById('sim_region').value = read_region();
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
  await refresh_account_controls();
}

/**
 * `refresh_account_controls` shows whom this tab is signed in as, and lists the accounts to act as.
 *
 * @example await refresh_account_controls();
 */
async function refresh_account_controls() {
  const session = read_session();
  document.getElementById('sim_session').textContent = session === null
    ? 'signed out'
    : `signed in as ${session.account.display_name} (account ${session.account.account_id})`;
  const accounts = await call_api('GET', '/api/simulation/accounts');
  const account_list_text = JSON.stringify(accounts);
  if (account_list_text !== sim_state.account_list_text) {
    sim_state.account_list_text = account_list_text;
    const options = accounts.filter((account) => account.state === 'active').map((account) =>
      `<option value="${account.account_id}">${escape_html(account.display_name)} (${escape_html(account.email)})</option>`);
    document.getElementById('sim_act_as').innerHTML = `<option value="">pick an account</option>${options.join('')}`;
  }
}

/**
 * `read_tab_account_id` returns the account this tab is signed in as, and throws when it isn't signed in.
 *
 * @example await call_api('POST', `/api/simulation/accounts/${read_tab_account_id()}/refund`);
 */
function read_tab_account_id() {
  const session = read_session();
  if (session === null) {
    throw new Error('Sign this tab in first, with the game app or "Act as".');
  }
  return session.account.account_id;
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
    } else if ('accountAct' in button.dataset) {
      run_sim_action(() => ACCOUNT_ACTIONS[button.dataset.accountAct](read_tab_account_id()));
    } else if (button.id === 'sim_redeliver') {
      run_sim_action(async () => {
        const events = await call_api('GET', '/api/simulation/stripe/events');
        if (events.length === 0) {
          throw new Error('Stripe has sent no webhook yet.');
        }
        await call_api('POST', `/api/simulation/stripe/events/${events[0].event_id}/redeliver`);
      });
    }
  });
  band.addEventListener('change', (event) => {
    if (event.target.id === 'sim_region') {
      save_region(event.target.value);
    } else if (event.target.id === 'sim_act_as' && event.target.value !== '') {
      run_sim_action(async () => {
        save_session(await call_api('POST', `/api/simulation/accounts/${event.target.value}/act-as`));
        event.target.value = '';
      });
    }
  });
  document.addEventListener('session_changed', () => run_sim_action(refresh_account_controls));
  run_sim_action(async () => {});
  setInterval(() => run_sim_action(async () => {}), SIM_REFRESH_MS);
}

start_sim_controls();
