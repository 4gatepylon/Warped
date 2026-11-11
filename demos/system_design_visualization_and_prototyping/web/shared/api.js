/**
 * @file
 * `api.js` lets the prototype's pages call its local backend and show its clock.
 *
 * Interface:
 * - `call_api(method, path, body, headers)` returns the parsed JSON answer, and throws an `Error` whose message is
 *   the backend's `detail` when the answer isn't a success. `body` and `headers` may be left out.
 * - `format_clock(now_s)` writes the simulated time as "day 3, 14:05:09".
 * - `escape_html(text)` makes any value safe to put into HTML.
 *
 * Example: const status = await call_api('GET', '/api/status');
 */

const SECONDS_PER_DAY = 86400;
const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_MINUTE = 60;

/**
 * `call_api` sends one request to the backend and returns its JSON answer.
 *
 * @example await call_api('PUT', '/api/settings/max_match_count', { value: 50 });
 */
async function call_api(method, path, body, headers = {}) {
  const options = { method, headers: { 'Content-Type': 'application/json', ...headers } };
  if (body !== undefined) {
    options.body = JSON.stringify(body);
  }
  const response = await fetch(path, options);
  const answer = await response.json();
  if (!response.ok) {
    throw new Error(typeof answer.detail === 'string' ? answer.detail : JSON.stringify(answer.detail));
  }
  return answer;
}

/**
 * `format_clock` writes a simulated time in seconds as its day and time of day.
 *
 * @example format_clock(90061) // "day 2, 01:01:01"
 */
function format_clock(now_s) {
  const two_digits = (count) => String(Math.floor(count)).padStart(2, '0');
  const day = Math.floor(now_s / SECONDS_PER_DAY) + 1;
  const hours = (now_s % SECONDS_PER_DAY) / SECONDS_PER_HOUR;
  const minutes = (now_s % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
  return `day ${day}, ${two_digits(hours)}:${two_digits(minutes)}:${two_digits(now_s % SECONDS_PER_MINUTE)}`;
}

/**
 * `escape_html` returns `text` with the characters that HTML reads as markup replaced.
 *
 * @example element.innerHTML = `<td>${escape_html(message.detail)}</td>`;
 */
function escape_html(text) {
  return String(text).replace(/[&<>"']/g, (char) => `&#${char.charCodeAt(0)};`);
}
