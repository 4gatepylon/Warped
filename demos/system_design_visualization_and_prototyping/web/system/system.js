/**
 * @file
 * `system.js` draws the system view of `index.html` from the simulated system's state.
 *
 * Interface:
 * - It redraws on every `system_refreshed` event, which `../shared/sim_controls.js` sends once a second with the
 *   answer of `GET /api/simulation/system` as its `detail`.
 * - A click on an element with `data-fail-machine` or `data-fail-server` kills that machine or game server.
 *
 * Example: <script src="system.js"></script> after `sim_controls.js`.
 */

const BOX_WIDTH_PX = 190;
const BOX_HEIGHT_PX = 46;
const ARROW_PX = 9;
const LANE_SHIFT_PX = 9;
const COUNT_PLACE_FRAC = 0.3;
const NODES = {
  simulated_players: { x: 110, y: 70, label: 'Simulated players' },
  website: { x: 110, y: 250, label: 'Website' },
  backend: { x: 400, y: 160, label: 'Backend' },
  database: { x: 400, y: 320, label: 'Database' },
  fleet_manager: { x: 680, y: 160, label: 'Fleet manager' },
  cloud: { x: 930, y: 60, label: 'Cloud' },
  machine: { x: 930, y: 190, label: 'Machines' },
  game_server: { x: 680, y: 320, label: 'Game servers' },
};

/**
 * `draw_link` returns the SVG of one arrow from the link's sender to its receiver, with its message count.
 * Opposite arrows between the same two boxes run side by side.
 *
 * @example svg_parts.push(draw_link({ sender: 'backend', receiver: 'database', message_count: 4, recent_count: 1 }));
 */
function draw_link(link) {
  const from = NODES[link.sender];
  const to = NODES[link.receiver];
  const length_px = Math.hypot(to.x - from.x, to.y - from.y);
  const along = { x: (to.x - from.x) / length_px, y: (to.y - from.y) / length_px };
  const shift = { x: -along.y * LANE_SHIFT_PX, y: along.x * LANE_SHIFT_PX };
  const edge_px = Math.min(BOX_WIDTH_PX / 2 / Math.abs(along.x), BOX_HEIGHT_PX / 2 / Math.abs(along.y));
  const start = { x: from.x + shift.x + along.x * edge_px, y: from.y + shift.y + along.y * edge_px };
  const tip = { x: to.x + shift.x - along.x * edge_px, y: to.y + shift.y - along.y * edge_px };
  const back = { x: tip.x - along.x * ARROW_PX, y: tip.y - along.y * ARROW_PX };
  const wing = { x: -along.y * ARROW_PX * 0.5, y: along.x * ARROW_PX * 0.5 };
  const count_at = { x: start.x + (tip.x - start.x) * COUNT_PLACE_FRAC, y: start.y + (tip.y - start.y) * COUNT_PLACE_FRAC };
  const recent_class = link.recent_count > 0 ? ' class="is_recent"' : '';
  return `<line${recent_class} x1="${start.x}" y1="${start.y}" x2="${back.x}" y2="${back.y}"/>
    <polygon${recent_class} points="${tip.x},${tip.y} ${back.x + wing.x},${back.y + wing.y} ${back.x - wing.x},${back.y - wing.y}"/>
    <text class="count" x="${count_at.x + shift.x}" y="${count_at.y + shift.y + 4}" text-anchor="middle">${link.message_count}</text>`;
}

/**
 * `draw_diagram` draws the component boxes and one arrow per link into the `#diagram` SVG.
 *
 * @example draw_diagram(system.links);
 */
function draw_diagram(links) {
  const boxes = Object.values(NODES).map((node) => `
    <rect x="${node.x - BOX_WIDTH_PX / 2}" y="${node.y - BOX_HEIGHT_PX / 2}" width="${BOX_WIDTH_PX}" height="${BOX_HEIGHT_PX}"/>
    <text x="${node.x}" y="${node.y + 5}" text-anchor="middle">${node.label}</text>`);
  document.getElementById('diagram').innerHTML = links.map(draw_link).join('') + boxes.join('');
}

/**
 * `draw_fleet` draws each region's machines, with one line per game server slot.
 *
 * @example draw_fleet(system);
 */
function draw_fleet(system) {
  const draw_slot = (server) => {
    if (server === undefined) {
      return '<div class="slot empty">free slot</div>';
    }
    const left_s = server.state === 'running' ? Math.max(0, server.running_at_s + server.match_s - system.now_s) : null;
    const status = left_s === null ? 'starting' : `${Math.ceil(left_s)} s left`;
    return `<div class="slot ${server.state}">${escape_html(server.game_server_id)} ${escape_html(server.label)},
      ${server.player_count} players, ${status}
      <button class="btn small sim_only right" data-fail-server="${escape_html(server.game_server_id)}">FAIL</button></div>`;
  };
  const draw_machine = (machine) => {
    const slots = Array.from({ length: machine.slot_count }, (_, i) => draw_slot(machine.game_servers[i]));
    return `<div class="box machine ${machine.state}"><div class="row">${escape_html(machine.machine_id)}
      <span class="dim">${machine.state}</span>
      <button class="btn small sim_only right" data-fail-machine="${escape_html(machine.machine_id)}">FAIL</button></div>
      ${slots.join('')}</div>`;
  };
  const regions = [...new Set(system.machines.map((machine) => machine.region))];
  document.getElementById('fleet').innerHTML = regions.map((region) => `<h3>${escape_html(region)}</h3>
    ${system.machines.filter((machine) => machine.region === region).map(draw_machine).join('')}`).join('')
    || '<p class="dim">No machines yet.</p>';
}

/**
 * `draw_caps` draws how full the two caps are, and the matches in the wait queue.
 *
 * @example draw_caps(system);
 */
function draw_caps(system) {
  const draw_bar = (label, count, max_count) => `<div>${label}: ${count} of ${max_count}</div>
    <div class="bar ${count >= max_count ? 'is_full' : ''}"><b style="width:${Math.min(100, (100 * count) / max_count)}%"></b></div>`;
  const waiting = system.wait_queue.map((request, i) =>
    `<div class="row">Place ${i + 1}: request ${request.match_request_id}, ${escape_html(request.region)},
      ${request.player_count} players</div>`);
  document.getElementById('caps').innerHTML =
    draw_bar('Matches', system.caps.match_count, system.caps.max_match_count) +
    draw_bar('Players in matches', system.caps.player_count, system.caps.max_player_count) +
    `<div>Wait queue: ${system.wait_queue.length} matches</div>${waiting.join('')}`;
}

/**
 * `draw_rows` returns the HTML of a table's header and rows from a list of row objects.
 *
 * @example table.innerHTML = draw_rows([{ name: 'max_match_count', value: 100 }]);
 */
function draw_rows(rows) {
  if (rows.length === 0) {
    return '<tr><td class="dim">no rows</td></tr>';
  }
  const names = Object.keys(rows[0]);
  const header = `<tr>${names.map((name) => `<th>${escape_html(name)}</th>`).join('')}</tr>`;
  const body = rows.map((row) => `<tr>${names.map((name) => `<td>${escape_html(row[name] ?? '')}</td>`).join('')}</tr>`);
  return header + body.join('');
}

/**
 * `draw_system` redraws every part of the system view from one answer of `GET /api/simulation/system`.
 *
 * @example document.addEventListener('system_refreshed', (event) => draw_system(event.detail));
 */
function draw_system(system) {
  draw_diagram(system.links);
  draw_fleet(system);
  draw_caps(system);
  document.getElementById('messages').innerHTML = draw_rows(system.messages.map((message) => ({
    when: format_clock(message.at_s),
    from: NODES[message.sender].label,
    to: NODES[message.receiver].label,
    message: message.what,
    about: message.detail,
  })));
  document.getElementById('tables').innerHTML = system.tables.map((table) => `<div class="box">
    <div class="row"><b>${escape_html(table.name)}</b><span class="dim">${table.row_count} rows</span></div>
    <pre>${escape_html(table.sql)}</pre>
    <div class="scroll"><table>${draw_rows(table.rows)}</table></div></div>`).join('');
}

document.addEventListener('system_refreshed', (event) => draw_system(event.detail));
document.addEventListener('click', (event) => {
  const button = event.target.closest('[data-fail-machine], [data-fail-server]');
  if (button === null) {
    return;
  }
  const path = 'failMachine' in button.dataset
    ? `/api/simulation/machines/${button.dataset.failMachine}/fail`
    : `/api/simulation/game-servers/${button.dataset.failServer}/fail`;
  run_sim_action(() => call_api('POST', path));
});
