/**
 * @file
 * `session.js` keeps the signed-in session of one browser tab, which stands in for one computer running the app.
 *
 * Interface:
 * - `read_session()` returns `{ access_token, refresh_token, account }`, or `null` when this tab isn't signed in.
 * - `save_session(answer)` keeps the backend's token answer; `forget_session(reason)` signs this tab out.
 * - `call_signed_in(method, path, body)` calls the backend as the signed-in player. When the access token has
 *   expired it swaps the refresh token once and retries; when that fails too, it signs the tab out.
 * - `read_region()` and `save_region(code)` keep the country the simulated connection comes from, which Google
 *   Cloud's load balancer would put in the `X-Client-Region` header.
 * - Every change sends a `session_changed` event on `document`, with the reason for a sign-out as its `detail`.
 *
 * Example: const me = await call_signed_in('GET', '/api/me');
 *
 * Must know:
 * - Gotchas: the real app keeps its refresh token in the computer's keychain. Here each tab keeps its own in
 *   `sessionStorage`, so two tabs are two computers, and signing in on one signs the other out.
 */

const SESSION_STORAGE_KEY = 'warped_session';
const REGION_STORAGE_KEY = 'warped_region';
const DEFAULT_REGION = 'US';
const HTTP_UNAUTHORIZED = 401;

/**
 * `read_session` returns this tab's session, or `null`.
 *
 * @example if (read_session() === null) { show_window('signin'); }
 */
function read_session() {
  const saved_text = sessionStorage.getItem(SESSION_STORAGE_KEY);
  return saved_text === null ? null : JSON.parse(saved_text);
}

/**
 * `save_session` keeps a token answer of the backend (`access_token`, `refresh_token`, `account`).
 *
 * @example save_session(await call_api('POST', '/api/auth/refresh', { refresh_token }));
 */
function save_session(answer) {
  const { access_token, refresh_token, account } = answer;
  sessionStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify({ access_token, refresh_token, account }));
  document.dispatchEvent(new CustomEvent('session_changed', { detail: '' }));
}

/**
 * `forget_session` signs this tab out and says why.
 *
 * @example forget_session('You signed out.');
 */
function forget_session(reason) {
  sessionStorage.removeItem(SESSION_STORAGE_KEY);
  document.dispatchEvent(new CustomEvent('session_changed', { detail: reason }));
}

/**
 * `send_signed_in` sends one request with this tab's access token and returns the raw response.
 *
 * @example const response = await send_signed_in('GET', '/api/me');
 */
async function send_signed_in(method, path, body) {
  const options = {
    method,
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${read_session().access_token}` },
  };
  if (body !== undefined) {
    options.body = JSON.stringify(body);
  }
  return fetch(path, options);
}

/**
 * `call_signed_in` calls the backend as this tab's player and returns the JSON answer. It throws an `Error` with
 * the backend's `detail` on a refusal, and signs the tab out when the session has ended.
 *
 * @example await call_signed_in('PATCH', '/api/me', { icon: 'ring' });
 */
async function call_signed_in(method, path, body) {
  if (read_session() === null) {
    throw new Error('Sign in first.');
  }
  let response = await send_signed_in(method, path, body);
  if (response.status === HTTP_UNAUTHORIZED) {
    try {
      save_session(await call_api('POST', '/api/auth/refresh', { refresh_token: read_session().refresh_token }));
    } catch (error) {
      forget_session(error.message);
      throw error;
    }
    response = await send_signed_in(method, path, body);
  }
  const answer = await response.json();
  if (!response.ok) {
    throw new Error(typeof answer.detail === 'string' ? answer.detail : JSON.stringify(answer.detail));
  }
  return answer;
}

/**
 * `read_region` returns the two-letter country this tab's connection comes from.
 *
 * @example const headers = { 'X-Client-Region': read_region() };
 */
function read_region() {
  const saved_region = sessionStorage.getItem(REGION_STORAGE_KEY);
  return saved_region === null ? DEFAULT_REGION : saved_region;
}

/**
 * `save_region` sets the country this tab's connection comes from.
 *
 * @example save_region('FR');
 */
function save_region(country_code) {
  sessionStorage.setItem(REGION_STORAGE_KEY, country_code);
}
