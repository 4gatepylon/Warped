/**
 * @file
 * `mockup.js` makes the window mock-ups of `index.html` navigable: it shows one window at a time, hides what the
 * chosen role and account may not see, plays a pretend search for a match, and keeps the reviewer's feedback.
 *
 * Interface, all through attributes in `index.html`:
 * - `data-go="<window>"` opens that window (`data-kind` sets the kind of game for the lobby);
 * - `data-open="<modal>"` and `data-close` open and close a window drawn over the others;
 * - `data-tab="<pane>"` inside a `data-tabs` element shows that element's `data-pane="<pane>"`;
 * - `data-needs="<word> ..."` shows an element only when every word holds: `admin`, `root`, `paid`, `unpaid`,
 *   `custom` or `matchmade`;
 * - `data-act="<action>"` runs one of `ACTIONS`.
 * - URL hash: `#play` opens the Play window, and the hash follows the open window.
 *
 * Example: open `game_design/window_design/index.html` in a browser.
 *
 * Must know:
 * - Gotchas: nothing here talks to a server. Players, games and timings are made up, and feedback is kept in this
 *   browser only (`localStorage`), so use "Copy all feedback" to hand it on.
 */

const SCREEN_WIDTH_PX = 1280;
const SCREEN_HEIGHT_PX = 800;
const SEARCH_TICK_MS = 1000;
const BOTS_OFFER_AFTER_S = 5;
const READY_CHECK_AFTER_S = 10;
const READY_CHECK_S = 20;
const FEEDBACK_STORAGE_KEY = 'warped_window_mockup_feedback';
const FIRST_WINDOW_NAME = 'home';
const ROLE_WORDS = { player: [], admin: ['admin'], root: ['admin', 'root'] };

const state = { window_name: FIRST_WINDOW_NAME, game_kind: 'custom', waited_s: 0, ready_left_s: 0, search_timer: null };

/**
 * `get_element` returns the element with `id`, and throws when the page has none.
 *
 * @example get_element('screen').classList.add('is_open');
 */
function get_element(id) {
  const element = document.getElementById(id);
  if (element === null) {
    throw new Error(`index.html has no element with id "${id}"`);
  }
  return element;
}

/**
 * `read_feedback` returns the saved feedback, an object from window name to `{ verdict, note }`.
 *
 * @example const note = read_feedback().play.note;
 */
function read_feedback() {
  const saved_text = localStorage.getItem(FEEDBACK_STORAGE_KEY);
  return saved_text === null ? {} : JSON.parse(saved_text);
}

/**
 * `save_feedback` stores the open window's verdict and note, and redraws the feedback bar.
 *
 * @example save_feedback({ verdict: 'like' });
 */
function save_feedback(change) {
  const feedback = read_feedback();
  const previous = state.window_name in feedback ? feedback[state.window_name] : { verdict: '', note: '' };
  feedback[state.window_name] = { ...previous, ...change };
  localStorage.setItem(FEEDBACK_STORAGE_KEY, JSON.stringify(feedback));
  draw_feedback();
}

/**
 * `write_feedback_text` returns all feedback as plain text, one block per window, ready to paste into a chat.
 *
 * @example navigator.clipboard.writeText(write_feedback_text());
 */
function write_feedback_text() {
  const feedback = read_feedback();
  const blocks = Object.keys(feedback)
    .filter((window_name) => feedback[window_name].verdict !== '' || feedback[window_name].note !== '')
    .map((window_name) => `${window_name}: ${feedback[window_name].verdict}\n${feedback[window_name].note}`.trim());
  return blocks.length === 0 ? 'No feedback yet.' : blocks.join('\n\n');
}

/**
 * `draw_feedback` shows the open window's saved verdict and note in the feedback bar, and all of it in the
 * "All feedback" window.
 *
 * @example draw_feedback();
 */
function draw_feedback() {
  const feedback = read_feedback();
  const own = state.window_name in feedback ? feedback[state.window_name] : { verdict: '', note: '' };
  if (get_element('feedback_note').value !== own.note) {
    get_element('feedback_note').value = own.note;
  }
  for (const button of document.querySelectorAll('[data-verdict]')) {
    button.classList.toggle('is_on', button.dataset.verdict === own.verdict);
  }
  get_element('all_feedback').textContent = write_feedback_text();
}

/**
 * `apply_view` hides every `data-needs` element whose words the chosen role, account and kind of game don't meet.
 *
 * @example get_element('view_role').value = 'admin'; apply_view();
 */
function apply_view() {
  const role_name = get_element('view_role').value;
  const account_word = get_element('view_account').value;
  const words_that_hold = new Set([...ROLE_WORDS[role_name], account_word, state.game_kind]);
  for (const element of document.querySelectorAll('[data-needs]')) {
    const has_every_word = element.dataset.needs.split(' ').every((word) => words_that_hold.has(word));
    element.classList.toggle('hidden', !has_every_word);
  }
}

/**
 * `show_window` opens the window named `window_name` and closes the others. A window marked `data-online` opens
 * the purchase window instead while the account is set to unpaid.
 *
 * @example show_window('play');
 */
function show_window(window_name) {
  const wanted = get_element(`w-${window_name}`);
  const is_blocked = 'online' in wanted.dataset && get_element('view_account').value === 'unpaid';
  const shown = is_blocked ? get_element('w-purchase') : wanted;
  state.window_name = shown.id.slice('w-'.length);
  for (const other of document.querySelectorAll('.window')) {
    other.classList.toggle('is_open', other === shown);
  }
  get_element('side').classList.toggle('is_open', 'side' in shown.dataset);
  shown.classList.toggle('has_side', 'side' in shown.dataset);
  get_element('view_window').value = state.window_name;
  history.replaceState(null, '', `#${state.window_name}`);
  close_modals();
  apply_view();
  draw_feedback();
}

/**
 * `close_modals` closes every window drawn over the others.
 *
 * @example close_modals();
 */
function close_modals() {
  for (const modal of document.querySelectorAll('.modal')) {
    modal.classList.remove('is_open');
  }
}

/**
 * `show_tab` shows the pane named `pane_name` inside `tabs_element` and marks its tab.
 *
 * @example show_tab(get_element('lobby_admin'), 'watching');
 */
function show_tab(tabs_element, pane_name) {
  for (const tab of tabs_element.querySelectorAll('[data-tab]')) {
    if (tab.closest('[data-tabs]') === tabs_element) {
      tab.classList.toggle('is_on', tab.dataset.tab === pane_name);
    }
  }
  for (const pane of tabs_element.querySelectorAll('[data-pane]')) {
    if (pane.closest('[data-tabs]') === tabs_element) {
      pane.classList.toggle('is_open', pane.dataset.pane === pane_name);
    }
  }
}

/**
 * `stop_search` ends the pretend search for a match and puts the Play window back as it was.
 *
 * @example stop_search();
 */
function stop_search() {
  clearInterval(state.search_timer);
  state.search_timer = null;
  get_element('search_idle').classList.remove('hidden');
  get_element('search_running').classList.add('hidden');
}

/**
 * `tick_search` advances the pretend search by one second: it offers bots, then starts the ready check, then
 * counts the ready check down and drops the match when nobody confirms.
 *
 * @example state.search_timer = setInterval(tick_search, SEARCH_TICK_MS);
 */
function tick_search() {
  state.waited_s += 1;
  get_element('search_waited').textContent = `${state.waited_s} s`;
  if (state.waited_s === BOTS_OFFER_AFTER_S) {
    get_element('m-bots').classList.add('is_open');
  }
  if (state.waited_s === READY_CHECK_AFTER_S) {
    close_modals();
    state.ready_left_s = READY_CHECK_S;
    get_element('m-ready').classList.add('is_open');
  }
  if (state.waited_s > READY_CHECK_AFTER_S) {
    state.ready_left_s -= 1;
    if (state.ready_left_s === 0) {
      close_modals();
      stop_search();
    }
  }
  get_element('ready_left').textContent = `${state.ready_left_s} s`;
}

const ACTIONS = {
  find_match() {
    state.waited_s = 0;
    get_element('search_waited').textContent = '0 s';
    get_element('search_idle').classList.add('hidden');
    get_element('search_running').classList.remove('hidden');
    state.search_timer = setInterval(tick_search, SEARCH_TICK_MS);
  },
  stop_search,
  confirm_ready() {
    stop_search();
    state.game_kind = 'matchmade';
    show_window('lobby');
  },
  verdict(button) {
    save_feedback({ verdict: button.dataset.verdict });
  },
  copy_feedback() {
    navigator.clipboard.writeText(write_feedback_text());
  },
  clear_feedback() {
    localStorage.removeItem(FEEDBACK_STORAGE_KEY);
    draw_feedback();
  },
};

/**
 * `handle_click` carries out the `data-` attribute of whatever was clicked (see this file's interface).
 *
 * @example document.addEventListener('click', handle_click);
 */
function handle_click(event) {
  const target = event.target.closest('[data-go], [data-open], [data-close], [data-tab], [data-act]');
  if (target === null) {
    return;
  }
  if ('tab' in target.dataset) {
    show_tab(target.closest('[data-tabs]'), target.dataset.tab);
  } else if ('act' in target.dataset) {
    ACTIONS[target.dataset.act](target);
  } else if ('open' in target.dataset) {
    get_element(`m-${target.dataset.open}`).classList.add('is_open');
  } else if ('close' in target.dataset) {
    close_modals();
  } else {
    if ('kind' in target.dataset) {
      state.game_kind = target.dataset.kind;
    }
    show_window(target.dataset.go);
  }
}

/**
 * `fit_screen` scales the 1280 by 800 mock screen down to the browser window's width.
 *
 * @example window.addEventListener('resize', fit_screen);
 */
function fit_screen() {
  const scale_frac = Math.min(1, document.documentElement.clientWidth / SCREEN_WIDTH_PX);
  get_element('screen').style.transform = `scale(${scale_frac})`;
  get_element('stage').style.height = `${SCREEN_HEIGHT_PX * scale_frac}px`;
}

/**
 * `main` fills the toolbar's list of windows, wires the page's events and opens the window the URL's hash names.
 *
 * @example main();
 */
function main() {
  for (const mocked_window of document.querySelectorAll('.window')) {
    const option = document.createElement('option');
    option.value = mocked_window.id.slice('w-'.length);
    option.textContent = mocked_window.dataset.title;
    get_element('view_window').append(option);
  }
  for (const tabs_element of document.querySelectorAll('[data-tabs]')) {
    show_tab(tabs_element, tabs_element.dataset.tabs);
  }
  document.addEventListener('click', handle_click);
  get_element('view_window').addEventListener('change', (event) => show_window(event.target.value));
  get_element('view_role').addEventListener('change', apply_view);
  get_element('view_account').addEventListener('change', () => show_window(state.window_name));
  get_element('feedback_note').addEventListener('input', (event) => save_feedback({ note: event.target.value }));
  window.addEventListener('resize', fit_screen);
  window.addEventListener('hashchange', () => show_window(location.hash.slice(1)));
  fit_screen();
  show_window(location.hash.length > 1 ? location.hash.slice(1) : FIRST_WINDOW_NAME);
}

main();
