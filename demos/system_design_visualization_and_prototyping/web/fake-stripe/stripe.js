/**
 * @file
 * `stripe.js` runs the fake Stripe's payment page (`index.html`): it shows the price and charges the card typed in.
 *
 * Interface:
 * - Reads the session from `?session=` with `GET /fake-stripe/api/checkout/<id>` and pays it with
 *   `POST /fake-stripe/api/checkout/<id>/pay`, then follows the answer's `redirect_url`.
 *
 * Example: <script src="stripe.js"></script> after `api.js`.
 */

const CHECKOUT_ID = new URLSearchParams(location.search).get('session');
const CENTS_PER_DOLLAR = 100;

/**
 * `show_checkout` shows the price, the way back, and whether the page was already paid.
 *
 * @example show_checkout(await call_api('GET', `/fake-stripe/api/checkout/${CHECKOUT_ID}`));
 */
function show_checkout(checkout) {
  document.getElementById('checkout_amount').textContent = `$${(checkout.amount_cents / CENTS_PER_DOLLAR).toFixed(2)}`;
  document.getElementById('pay').textContent = `PAY $${(checkout.amount_cents / CENTS_PER_DOLLAR).toFixed(2)}`;
  document.getElementById('checkout_cancel').href = checkout.cancel_url;
  if (checkout.state !== 'open') {
    document.getElementById('pay').disabled = true;
    document.getElementById('checkout_error').textContent = 'This payment page was already paid.';
  }
}

/**
 * `pay` charges the typed card and follows Stripe's redirect, or shows why the card was refused.
 *
 * @example await pay();
 */
async function pay() {
  const error_box = document.getElementById('checkout_error');
  error_box.textContent = '';
  try {
    const card_number = document.getElementById('card_number').value;
    const answer = await call_api('POST', `/fake-stripe/api/checkout/${CHECKOUT_ID}/pay`, { card_number });
    location.href = answer.redirect_url;
  } catch (error) {
    error_box.textContent = error.message;
  }
}

document.getElementById('pay').addEventListener('click', pay);
call_api('GET', `/fake-stripe/api/checkout/${CHECKOUT_ID}`)
  .then(show_checkout)
  .catch((error) => { document.getElementById('checkout_error').textContent = error.message; });
