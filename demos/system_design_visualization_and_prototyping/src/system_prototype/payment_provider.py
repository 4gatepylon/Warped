"""`payment_provider.py` plays Stripe: Checkout, test cards, refunds, disputes and webhooks.

The backend asks the fake Stripe for a Checkout Session, the app opens its page in the browser, and
the player types a test card there, so Warped never sees the card. Stripe then tells the backend
what happened with webhooks signed as Stripe signs them, and can send one again, as Stripe retries.

Interface:
    SUCCEEDING_CARD: The test card that pays.
    DECLINED_CARD: The test card that is declined.
    FakeStripe: The fake payment provider.

Example:
    >>> stripe = FakeStripe(bus, purchases.receiveStripeWebhook)
    >>> session = stripe.createCheckoutSession(api_key, "1", 1337, success_url, cancel_url)
    >>> stripe.payCheckout(session["id"], SUCCEEDING_CARD)

Must know:
    Limitations:
        - The webhook is delivered at once, in-process; real Stripe sends it over HTTPS, later,
          and retries for up to 3 days until the endpoint answers 2xx.
"""

import json
import secrets
from collections.abc import Callable

from .account_rules import sign_webhook
from .bus import Bus
from .rules import AuthError, NotFoundError, RuleError

SUCCEEDING_CARD: str = "4242424242424242"
DECLINED_CARD: str = "4000000000000002"
_ID_BYTES: int = 12
_CHECKOUT_PAGE_PATH: str = "/fake-stripe/"


class FakeStripe:
    """`FakeStripe` takes payments on its own page and reports them to the backend's webhook.

    Args:
        bus: The shared database, clock and message log.
        deliver_webhook: The backend's webhook endpoint: it takes the raw body and the
            `Stripe-Signature` header, and returns its answer.

    Example:
        >>> FakeStripe(bus, purchases.receiveStripeWebhook)
    """

    def __init__(self, bus: Bus, deliver_webhook: Callable[[bytes, str], dict]):
        self._bus: Bus = bus
        self._deliver_webhook: Callable[[bytes, str], dict] = deliver_webhook

    def createCheckoutSession(
        self,
        api_key: str,
        client_reference_id: str,
        amount_cents: int,
        success_url: str,
        cancel_url: str,
    ) -> dict:
        """`createCheckoutSession` makes a payment page, as `POST /v1/checkout/sessions` does.

        Args:
            api_key: The backend's secret API key.
            client_reference_id: The backend's account ID, which the webhook will carry back.
            amount_cents: The price.
            success_url: Where the page sends the browser once the payment succeeds.
            cancel_url: Where it sends the browser if the player gives up.

        Returns:
            The session's `id` and the `url` of its page.

        Raises:
            AuthError: The API key is wrong.

        Example:
            >>> stripe.createCheckoutSession(api_key, "1", 1337, "/site/#paid", "/game/#purchase")
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(
            now_s, "backend", "payment_provider", "POST /v1/checkout/sessions", client_reference_id
        )
        self._checkApiKey(api_key)
        checkout_id: str = "cs_test_" + secrets.token_hex(_ID_BYTES)
        self._bus.execute(
            "INSERT INTO stripe_checkout (checkout_id, client_reference_id, amount_cents,"
            " success_url, cancel_url, state, created_at_s) VALUES (?, ?, ?, ?, ?, 'open', ?)",
            (checkout_id, client_reference_id, amount_cents, success_url, cancel_url, now_s),
        )
        return {"id": checkout_id, "url": f"{_CHECKOUT_PAGE_PATH}?session={checkout_id}"}

    def readCheckoutSession(self, checkout_id: str) -> dict:
        """`readCheckoutSession` returns what the payment page shows: the price and its state.

        Raises:
            NotFoundError: There is no such session.

        Example:
            >>> stripe.readCheckoutSession("cs_test_...")["amount_cents"]
            1337
        """
        rows: list[dict] = self._bus.read(
            "SELECT checkout_id, amount_cents, state, success_url, cancel_url FROM stripe_checkout"
            " WHERE checkout_id = ?",
            (checkout_id,),
        )
        if len(rows) == 0:
            raise NotFoundError(f"There is no checkout session {checkout_id}.")
        return rows[0]

    def payCheckout(self, checkout_id: str, card_number: str) -> dict:
        """`payCheckout` charges a test card on a payment page, then tells the backend by webhook.

        Args:
            checkout_id: The session.
            card_number: What the player typed; spaces don't matter.

        Returns:
            `{"redirect_url": success_url}`.

        Raises:
            NotFoundError: There is no such session.
            RuleError: The session was already paid, the card was declined, or it isn't a test card.

        Example:
            >>> stripe.payCheckout(checkout_id, "4242 4242 4242 4242")
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "payment_provider", "pay (browser)", checkout_id)
        checkout: dict = self.readCheckoutSession(checkout_id)
        if checkout["state"] != "open":
            raise RuleError("This payment page was already paid.")
        digits: str = "".join(character for character in card_number if not character.isspace())
        if digits == DECLINED_CARD:
            raise RuleError("Your card was declined.")
        if digits != SUCCEEDING_CARD:
            raise RuleError(
                "This is a stand-in for Stripe: use 4242 4242 4242 4242 to pay, or"
                " 4000 0000 0000 0002 to be declined."
            )
        payment_id: str = "pi_test_" + secrets.token_hex(_ID_BYTES)
        self._bus.execute(
            "INSERT INTO stripe_payment (payment_id, checkout_id, amount_cents, state, paid_at_s)"
            " VALUES (?, ?, ?, 'succeeded', ?)",
            (payment_id, checkout_id, checkout["amount_cents"], now_s),
        )
        self._bus.execute(
            "UPDATE stripe_checkout SET state = 'complete', payment_id = ? WHERE checkout_id = ?",
            (payment_id, checkout_id),
        )
        client_reference_id: str = self._bus.read(
            "SELECT client_reference_id FROM stripe_checkout WHERE checkout_id = ?", (checkout_id,)
        )[0]["client_reference_id"]
        self._createEvent(
            "checkout.session.completed",
            {
                "object": "checkout.session",
                "id": checkout_id,
                "client_reference_id": client_reference_id,
                "payment_intent": payment_id,
                "amount_total": checkout["amount_cents"],
                "payment_status": "paid",
            },
        )
        return {"redirect_url": checkout["success_url"]}

    def refundPayment(self, payment_id: str) -> str:
        """`refundPayment` refunds a payment in full, as someone would from Stripe's dashboard.

        Returns:
            The ID of the `charge.refunded` event.

        Raises:
            NotFoundError: There is no such payment.
            RuleError: It was already refunded or disputed.

        Example:
            >>> stripe.refundPayment("pi_test_...")
        """
        return self._endPayment(payment_id, "refunded", "charge.refunded")

    def disputePayment(self, payment_id: str) -> str:
        """`disputePayment` makes the card's bank dispute a payment, as a chargeback would.

        Returns:
            The ID of the `charge.dispute.created` event.

        Raises:
            NotFoundError: There is no such payment.
            RuleError: It was already refunded or disputed.

        Example:
            >>> stripe.disputePayment("pi_test_...")
        """
        return self._endPayment(payment_id, "disputed", "charge.dispute.created")

    def redeliverEvent(self, event_id: str) -> dict:
        """`redeliverEvent` sends an event's webhook again, as Stripe does when it retries.

        Returns:
            The backend's answer.

        Raises:
            NotFoundError: There is no such event.

        Example:
            >>> stripe.redeliverEvent("evt_test_...")["is_duplicate"]
            True
        """
        if len(self._bus.read("SELECT 1 FROM stripe_event WHERE event_id = ?", (event_id,))) == 0:
            raise NotFoundError(f"There is no Stripe event {event_id}.")
        return self._deliver(event_id)

    def listEvents(self) -> list[dict]:
        """`listEvents` returns every event, newest first, as Stripe's dashboard lists them.

        Example:
            >>> stripe.listEvents()[0]["type"]
            'checkout.session.completed'
        """
        return self._bus.read(
            "SELECT event_id, type, created_at_s, delivery_count FROM stripe_event"
            " ORDER BY rowid DESC"
        )

    def _endPayment(self, payment_id: str, new_state: str, event_type: str) -> str:
        """`_endPayment` marks a succeeded payment refunded or disputed and reports it by webhook.

        Example:
            >>> self._endPayment(payment_id, "refunded", "charge.refunded")
        """
        rows: list[dict] = self._bus.read(
            "SELECT * FROM stripe_payment WHERE payment_id = ?", (payment_id,)
        )
        if len(rows) == 0:
            raise NotFoundError(f"There is no Stripe payment {payment_id}.")
        if rows[0]["state"] != "succeeded":
            raise RuleError(f"The payment {payment_id} was already {rows[0]['state']}.")
        self._bus.execute(
            "UPDATE stripe_payment SET state = ? WHERE payment_id = ?", (new_state, payment_id)
        )
        return self._createEvent(
            event_type,
            {"object": "charge", "payment_intent": payment_id, "amount": rows[0]["amount_cents"]},
        )

    def _createEvent(self, event_type: str, data_object: dict) -> str:
        """`_createEvent` records an event and delivers its webhook once.

        Example:
            >>> self._createEvent("charge.refunded", {"payment_intent": payment_id})
        """
        now_s: float = self._bus.readSeconds()
        event_id: str = "evt_test_" + secrets.token_hex(_ID_BYTES)
        payload: str = json.dumps(
            {
                "id": event_id,
                "type": event_type,
                "created": int(now_s),
                "data": {"object": data_object},
            }
        )
        self._bus.execute(
            "INSERT INTO stripe_event (event_id, type, payload, created_at_s) VALUES (?, ?, ?, ?)",
            (event_id, event_type, payload, now_s),
        )
        self._deliver(event_id)
        return event_id

    def _deliver(self, event_id: str) -> dict:
        """`_deliver` signs an event's body with the endpoint's secret and posts it to the backend.

        Example:
            >>> self._deliver("evt_test_...")
        """
        now_s: float = self._bus.readSeconds()
        event: dict = self._bus.read("SELECT * FROM stripe_event WHERE event_id = ?", (event_id,))[
            0
        ]
        secret: str = self._bus.read(
            "SELECT value FROM secret WHERE name = 'stripe_webhook_secret'"
        )[0]["value"]
        payload: bytes = event["payload"].encode("utf-8")
        self._bus.execute(
            "UPDATE stripe_event SET delivery_count = delivery_count + 1 WHERE event_id = ?",
            (event_id,),
        )
        self._bus.send(now_s, "payment_provider", "backend", "POST /webhooks/stripe", event["type"])
        return self._deliver_webhook(payload, sign_webhook(secret, now_s, payload))

    def _checkApiKey(self, api_key: str) -> None:
        """`_checkApiKey` refuses a request whose secret API key isn't the backend's.

        Raises:
            AuthError: The key is wrong.

        Example:
            >>> self._checkApiKey(api_key)
        """
        expected: str = self._bus.read("SELECT value FROM secret WHERE name = 'stripe_api_key'")[0][
            "value"
        ]
        if api_key != expected:
            raise AuthError("Invalid API key provided.")
