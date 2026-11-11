/**
 * @file
 * `account.js` plays the game app's side of accounts in the mock-up (`index.html`): signing in with Google, the
 * first-time form, buying online play, and the account settings, all against the simulated backend.
 *
 * Interface:
 * - Actions for `data-act` (added to `ACTIONS` of `../shared/windows.js`): `sign_in_with_google`, `sign_up`,
 *   `start_checkout`, `change_name`, `sign_out`, `sign_out_everywhere`, `delete_account`.
 * - Elements with `data-live="display_name"` or `data-live="icon"` show the signed-in account; signed out, they show
 *   the mock-up's made-up player.
 * - When the URL has `?code=&state=`, Google has just sent the browser back: the code goes to the backend.
 * - While signed in, it reads `GET /api/me` every second, so a payment, a refund or a sign-out elsewhere shows.
 *
 * Example: <script src="account.js"></script> after `session.js` and `windows.js`.
 *
 * Must know:
 * - Gotchas: the toolbar's "Your account" choice follows the signed-in account and can't be changed while signed in.
 * - Sign-in is OpenID Connect's authorization code flow with PKCE, as RFC 8252 says a desktop app should do it.
 */

const PKCE_STORAGE_KEY = 'warped_pkce';
const SIGN_UP_STORAGE_KEY = 'warped_sign_up';
const RANDOM_BYTE_COUNT = 32;
const ACCOUNT_REFRESH_MS = 1000;
const NAME_CHECK_DELAY_MS = 300;
const account_state = { me: null, icons: [], sign_up_icon: 'blue', is_waiting_for_payment: false, name_check_timer: null };

/**
 * `encode_base64url` writes bytes as base64url without padding, as PKCE and OAuth state use.
 *
 * @example encode_base64url(new Uint8Array([255])) // "_w"
 */
function encode_base64url(bytes) {
  return btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/**
 * `make_random_text` returns 32 random bytes as base64url, for the code verifier, the state and the nonce.
 *
 * @example const state = make_random_text();
 */
function make_random_text() {
  return encode_base64url(crypto.getRandomValues(new Uint8Array(RANDOM_BYTE_COUNT)));
}

/**
 * `start_google_sign_in` sends the browser to Google's sign-in page with a new PKCE verifier, state and nonce.
 *
 * @example await start_google_sign_in();
 */
async function start_google_sign_in() {
  const config = await call_api('GET', '/api/auth/google/config');
  const pkce = { verifier: make_random_text(), state: make_random_text(), nonce: make_random_text(),
    redirect_uri: `${location.origin}/game/` };
  sessionStorage.setItem(PKCE_STORAGE_KEY, JSON.stringify(pkce));
  const challenge = encode_base64url(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(pkce.verifier)));
  const query = new URLSearchParams({
    client_id: config.client_id, redirect_uri: pkce.redirect_uri, response_type: 'code', scope: config.scope,
    state: pkce.state, nonce: pkce.nonce, code_challenge: challenge, code_challenge_method: 'S256',
  });
  location.href = `${config.authorize_url}?${query}`;
}

/**
 * `finish_google_sign_in` hands the code Google sent back to the backend: a known player is signed in, a new one
 * gets the first-time form.
 *
 * @example await finish_google_sign_in(new URLSearchParams(location.search));
 */
async function finish_google_sign_in(query) {
  history.replaceState(null, '', '/game/#signin');
  const saved_text = sessionStorage.getItem(PKCE_STORAGE_KEY);
  sessionStorage.removeItem(PKCE_STORAGE_KEY);
  const pkce = saved_text === null ? null : JSON.parse(saved_text);
  if (pkce === null || query.get('state') !== pkce.state) {
    throw new Error("This sign-in wasn't started from this window, so it was ignored. Sign in again.");
  }
  const answer = await call_api('POST', '/api/auth/google', {
    code: query.get('code'), code_verifier: pkce.verifier, nonce: pkce.nonce, redirect_uri: pkce.redirect_uri,
  });
  if (answer.status === 'signed_in') {
    save_session(answer);
    show_window('home');
    return;
  }
  sessionStorage.setItem(SIGN_UP_STORAGE_KEY, JSON.stringify(answer));
  document.getElementById('signup_name').value = answer.suggested_name;
  draw_sign_up();
  check_name_soon();
}

/**
 * `draw_icons` draws the icons to pick from into `container`, with `chosen_icon` marked.
 *
 * @example draw_icons(document.getElementById('signup_icons'), 'blue');
 */
function draw_icons(container, chosen_icon) {
  container.innerHTML = account_state.icons.map((icon) =>
    `<span class="icon icon_pick i-${icon}${icon === chosen_icon ? ' is_on' : ''}" data-icon="${icon}" title="${icon}"></span>`).join(' ');
}

/**
 * `draw_sign_up` shows whom the first-time form is for, or that it waits for a first sign-in.
 *
 * @example draw_sign_up();
 */
function draw_sign_up() {
  const pending_text = sessionStorage.getItem(SIGN_UP_STORAGE_KEY);
  document.getElementById('signup_who').textContent = pending_text === null
    ? 'Opens after your first sign-in with Google.'
    : `Signed in with Google as ${JSON.parse(pending_text).email}. Make your account:`;
  draw_icons(document.getElementById('signup_icons'), account_state.sign_up_icon);
}

/**
 * `check_name_soon` asks the backend whether the typed display name is free, once typing pauses.
 *
 * @example document.getElementById('signup_name').addEventListener('input', check_name_soon);
 */
function check_name_soon() {
  clearTimeout(account_state.name_check_timer);
  account_state.name_check_timer = setTimeout(async () => {
    const display_name = document.getElementById('signup_name').value;
    const check = await call_api('GET', `/api/accounts/name-check?display_name=${encodeURIComponent(display_name)}`);
    const verdict = document.getElementById('signup_name_check');
    verdict.className = check.is_free ? 'blue' : 'red';
    verdict.textContent = check.is_free ? 'free' : check.reason;
  }, NAME_CHECK_DELAY_MS);
}

/**
 * `describe_access` says in a sentence whether the account may play online, and for how long.
 *
 * @example document.getElementById('home_access').textContent = describe_access(me.online_access);
 */
function describe_access(access) {
  if (access.reason === 'purchased') {
    return 'Online play: bought.';
  }
  if (access.reason === 'allowance_used') {
    return 'Free online play used up. Buy online play to keep playing online; local games stay free.';
  }
  const minutes_left = Math.ceil(access.free_s_left / SECONDS_PER_MINUTE);
  return `Free online play left: ${access.free_matches_left} finished match or ${minutes_left} min, whichever ends first.`;
}

/**
 * `draw_account` shows the signed-in account in every live element, or the mock-up's made-up player.
 *
 * @example draw_account();
 */
function draw_account() {
  const { me } = account_state;
  for (const element of document.querySelectorAll('[data-live]')) {
    if (!('mock' in element.dataset)) {
      element.dataset.mock = element.dataset.live === 'icon' ? element.className : element.textContent;
    }
    if (element.dataset.live === 'icon') {
      element.className = me === null ? element.dataset.mock : `icon i-${me.icon}`;
    } else {
      element.textContent = me === null ? element.dataset.mock : me.display_name;
    }
  }
  const account_choice = document.getElementById('view_account');
  account_choice.disabled = me !== null;
  document.getElementById('session_line').textContent = me === null
    ? 'Not signed in: the windows show made-up players.'
    : `Signed in as ${me.display_name}. Live: sign-in, Home, Purchase, Settings > Account.`;
  document.getElementById('home_access').textContent = me === null ? '' : describe_access(me.online_access);
  if (me === null) {
    return;
  }
  account_choice.value = me.online_access.can_play_online ? 'paid' : 'unpaid';
  const wait_days = Math.ceil(me.name_change_wait_s / SECONDS_PER_DAY);
  document.getElementById('settings_name_wait').textContent = wait_days > 0 ? `next change in ${wait_days} days` : 'you can change it now';
  if (document.activeElement !== document.getElementById('settings_name')) {
    document.getElementById('settings_name').value = me.display_name;
  }
  draw_icons(document.getElementById('settings_icons'), me.icon);
}

/**
 * `refresh_account` reads the signed-in account again; a payment that arrived closes the Purchase window.
 *
 * @example setInterval(refresh_account, ACCOUNT_REFRESH_MS);
 */
async function refresh_account() {
  if (read_session() === null) {
    account_state.me = null;
    draw_account();
    return;
  }
  try {
    account_state.me = await call_signed_in('GET', '/api/me');
  } catch (error) {
    document.getElementById('session_line').textContent = error.message;
    return;
  }
  draw_account();
  if (account_state.is_waiting_for_payment && account_state.me.has_purchase) {
    account_state.is_waiting_for_payment = false;
    show_window('home');
  }
}

/**
 * `run_account_action` runs one account request and shows its refusal in the element with `error_id`.
 *
 * @example run_account_action('settings_error', () => call_signed_in('PATCH', '/api/me', { icon: 'ring' }));
 */
async function run_account_action(error_id, send_request) {
  document.getElementById(error_id).textContent = '';
  try {
    await send_request();
    await refresh_account();
  } catch (error) {
    document.getElementById(error_id).textContent = error.message;
  }
}

Object.assign(ACTIONS, {
  sign_in_with_google: () => run_account_action('signin_reason', start_google_sign_in),
  sign_up: () => run_account_action('signup_error', async () => {
    const pending_text = sessionStorage.getItem(SIGN_UP_STORAGE_KEY);
    if (pending_text === null) {
      throw new Error('Sign in with Google first.');
    }
    const answer = await call_api('POST', '/api/accounts', {
      sign_up_token: JSON.parse(pending_text).sign_up_token,
      birth_date: document.getElementById('signup_birth').value,
      display_name: document.getElementById('signup_name').value,
      icon: account_state.sign_up_icon,
      has_accepted_terms: document.getElementById('signup_terms').checked,
    }, { 'X-Client-Region': read_region() });
    sessionStorage.removeItem(SIGN_UP_STORAGE_KEY);
    draw_sign_up();
    save_session(answer);
    show_window('home');
  }),
  start_checkout: () => run_account_action('purchase_error', async () => {
    const { checkout_url } = await call_signed_in('POST', '/api/purchases/checkout');
    account_state.is_waiting_for_payment = true;
    window.open(checkout_url, '_blank', 'noopener');
  }),
  change_name: () => run_account_action('settings_error', () =>
    call_signed_in('PATCH', '/api/me', { display_name: document.getElementById('settings_name').value })),
  sign_out: () => run_account_action('settings_error', async () => {
    await call_signed_in('POST', '/api/auth/sign-out', { is_everywhere: false });
    forget_session('You signed out.');
  }),
  sign_out_everywhere: () => run_account_action('settings_error', async () => {
    await call_signed_in('POST', '/api/auth/sign-out', { is_everywhere: true });
    forget_session('You signed out of every computer.');
  }),
  delete_account: () => run_account_action('settings_error', async () => {
    await call_signed_in('DELETE', '/api/me');
    forget_session('Your account was deleted.');
  }),
});

document.addEventListener('click', (event) => {
  const picked = event.target.closest('[data-icon]');
  if (picked === null) {
    return;
  }
  if (picked.closest('#signup_icons') !== null) {
    account_state.sign_up_icon = picked.dataset.icon;
    draw_sign_up();
  } else {
    run_account_action('settings_error', () => call_signed_in('PATCH', '/api/me', { icon: picked.dataset.icon }));
  }
});
document.addEventListener('session_changed', (event) => {
  if (read_session() === null) {
    account_state.me = null;
    document.getElementById('signin_reason').textContent = event.detail;
    show_window('signin');
  }
  refresh_account();
});
document.getElementById('signup_name').addEventListener('input', check_name_soon);

/**
 * `start_account` loads the icons, finishes a sign-in that Google just sent back, and keeps the account fresh.
 *
 * @example start_account();
 */
async function start_account() {
  account_state.icons = await call_api('GET', '/api/icons');
  draw_sign_up();
  const query = new URLSearchParams(location.search);
  if (query.has('code')) {
    await run_account_action('signin_reason', () => finish_google_sign_in(query));
  }
  await refresh_account();
  setInterval(refresh_account, ACCOUNT_REFRESH_MS);
}

start_account();
