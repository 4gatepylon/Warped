/**
 * @file
 * `google.js` runs the fake Google's sign-in page (`index.html`): it lists the made-up people and, when one is
 * picked, sends the browser back to the app with a code.
 *
 * Interface:
 * - Reads the authorize request from the URL's query and posts it with the picked person to
 *   `POST /fake-google/api/authorize`, which answers with the URL to send the browser to.
 *
 * Example: <script src="google.js"></script> after `api.js`.
 */

const AUTHORIZE_PARAMS = Object.fromEntries(new URLSearchParams(location.search));

/**
 * `pick_identity` signs in as the person with `subject` and follows Google's redirect, or shows Google's error.
 *
 * @example await pick_identity('100000000000000000001');
 */
async function pick_identity(subject) {
  try {
    const answer = await call_api('POST', '/fake-google/api/authorize', { subject, params: AUTHORIZE_PARAMS });
    location.href = answer.redirect_url;
  } catch (error) {
    document.getElementById('google_error').textContent = `Error 400: ${error.message}`;
  }
}

/**
 * `draw_identities` lists the made-up people, one button each.
 *
 * @example draw_identities(await call_api('GET', '/fake-google/api/identities'));
 */
function draw_identities(identities) {
  document.getElementById('identities').innerHTML = identities.map((person) => `
    <button class="choice" data-subject="${escape_html(person.subject)}">
      <span class="avatar">${escape_html(person.full_name.slice(0, 1))}</span>
      <span>${escape_html(person.full_name)}<small>${escape_html(person.email)}</small></span>
    </button>`).join('');
}

document.getElementById('identities').addEventListener('click', (event) => {
  const choice = event.target.closest('[data-subject]');
  if (choice !== null) {
    pick_identity(choice.dataset.subject);
  }
});
document.getElementById('new_identity').addEventListener('click', async () => {
  try {
    const person = await call_api('POST', '/fake-google/api/identities', {
      full_name: document.getElementById('new_name').value,
      email: document.getElementById('new_email').value,
    });
    await pick_identity(person.subject);
  } catch (error) {
    document.getElementById('google_error').textContent = error.message;
  }
});
call_api('GET', '/fake-google/api/identities').then(draw_identities);
