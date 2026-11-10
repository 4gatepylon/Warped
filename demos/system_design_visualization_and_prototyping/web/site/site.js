/**
 * @file
 * `site.js` fills the live parts of the website mock-up (`index.html`) from the simulated backend.
 *
 * Interface:
 * - The status page redraws on every `system_refreshed` event of `../shared/sim_controls.js`.
 * - The console's SETTINGS tab lists `GET /api/settings`; changing a value sends
 *   `PUT /api/settings/<name>` and shows the backend's refusal, if any.
 *
 * Example: <script src="site.js"></script> after `sim_controls.js` and `windows.js`.
 */

/**
 * `draw_status` shows which regions have machines ready, and how full the caps and the wait queue are.
 *
 * @example draw_status(system);
 */
function draw_status(system) {
  const ready_by_region = Object.fromEntries(system.regions.map((region) => [region.region, region.ready_machine_count]));
  const region_names = [...new Set(system.machines.map((machine) => machine.region))];
  const region_rows = region_names.map((name) => (name in ready_by_region
    ? `<div class="row">${escape_html(name)} <span class="blue right">up, ${ready_by_region[name]} machines</span></div>`
    : `<div class="row">${escape_html(name)} <span class="warn right">starting</span></div>`));
  document.getElementById('status_services').innerHTML =
    `<div class="row">Sign-in <span class="dim right">arrives with a later part</span></div>${region_rows.join('')}`;
  const { caps } = system;
  document.getElementById('status_caps').innerHTML =
    `<div class="row">Matches <span class="right">${caps.match_count} of ${caps.max_match_count}</span></div>
     <div class="row">Players in matches <span class="right">${caps.player_count} of ${caps.max_player_count}</span></div>
     <div class="row">Matches waiting for room <span class="right">${system.wait_queue_count}</span></div>`;
}

/**
 * `draw_settings` lists every setting in the console, each with a box to change it.
 *
 * @example draw_settings(await call_api('GET', '/api/settings'));
 */
function draw_settings(settings) {
  const rows = settings.map((setting) => `<tr><td>${escape_html(setting.section)}</td>
    <td>${escape_html(setting.meaning)}</td>
    <td><input size="8" data-setting="${escape_html(setting.name)}" value="${setting.value}"></td></tr>`);
  document.getElementById('settings_table').innerHTML =
    `<tr><th>Section</th><th>Setting</th><th>Value</th></tr>${rows.join('')}`;
}

/**
 * `save_setting` sends one changed setting to the backend and redraws the list, or shows why it was refused.
 *
 * @example save_setting('max_match_count', '50');
 */
async function save_setting(name, value_text) {
  const error_box = document.getElementById('settings_error');
  error_box.textContent = '';
  try {
    draw_settings(await call_api('PUT', `/api/settings/${name}`, { value: Number(value_text) }));
  } catch (error) {
    error_box.textContent = error.message;
  }
}

document.addEventListener('system_refreshed', (event) => draw_status(event.detail));
document.getElementById('settings_table').addEventListener('change', (event) => {
  save_setting(event.target.dataset.setting, event.target.value);
});
call_api('GET', '/api/settings').then(draw_settings);
