"""`purchases.py` is the backend's part that sells online play and listens to Stripe.

Interface:
    Purchases: Starts a checkout and handles Stripe's webhooks.

Example:
    >>> purchases = Purchases(bus, secret_manager)
    >>> purchases.connectStripe(FakeStripe(bus, purchases.receiveStripeWebhook))
    >>> purchases.startCheckout(1)["checkout_url"]
    '/fake-stripe/?session=cs_test_...'

Must know:
    Gotchas:
        - Stripe may send one event more than once and in any order, so each event is recorded
          by its ID in the same transaction that applies it, and a repeat changes nothing. A
          refund that arrives before its payment's "completed" event still locks the purchase.
"""

import json
import sqlite3

from .account_rules import check_webhook_signature
from .bus import Bus
from .payment_provider import FakeStripe
from .rules import RuleError
from .secret_manager import SecretManager

CENTS_PER_DOLLAR: int = 100
SUCCESS_URL: str = "/site/#paid"
CANCEL_URL: str = "/site/#not-paid"
_LOCKING_EVENTS: dict[str, str] = {
    "charge.refunded": "refunded",
    "charge.dispute.created": "disputed",
}


class Purchases:
    """`Purchases` unlocks an account's online play when Stripe confirms its payment.

    Args:
        bus: The shared database, clock and message log.
        secret_manager: Where the backend reads Stripe's API key and webhook secret.

    Example:
        >>> Purchases(bus, secret_manager)
    """

    def __init__(self, bus: Bus, secret_manager: SecretManager):
        self._bus: Bus = bus
        self._stripe_api_key: str = secret_manager.readSecret("backend", "stripe_api_key")
        self._webhook_secret: str = secret_manager.readSecret("backend", "stripe_webhook_secret")
        self._stripe: FakeStripe | None = None

    def connectStripe(self, stripe: FakeStripe) -> None:
        """`connectStripe` gives the backend its Stripe client, made after the webhook exists.

        Example:
            >>> purchases.connectStripe(stripe)
        """
        self._stripe = stripe

    def startCheckout(self, account_id: int) -> dict:
        """`startCheckout` asks Stripe for a payment page for this account's purchase.

        Raises:
            RuleError: The account already bought online play.

        Example:
            >>> purchases.startCheckout(1)
            {'checkout_url': '/fake-stripe/?session=cs_test_...'}
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "POST /purchases/checkout", str(account_id))
        if self.hasPurchase(account_id):
            raise RuleError("You already bought online play.")
        price_usd: float = self._bus.readSettings()["price_usd"]
        session: dict = self._readStripe().createCheckoutSession(
            self._stripe_api_key,
            str(account_id),
            round(price_usd * CENTS_PER_DOLLAR),
            SUCCESS_URL,
            CANCEL_URL,
        )
        return {"checkout_url": session["url"]}

    def hasPurchase(self, account_id: int) -> bool:
        """`hasPurchase` says whether the account has a payment that wasn't refunded or disputed.

        Example:
            >>> purchases.hasPurchase(1)
            False
        """
        return (
            len(
                self._bus.read(
                    "SELECT 1 FROM purchase WHERE account_id = ? AND state = 'paid'", (account_id,)
                )
            )
            > 0
        )

    def readPaymentId(self, account_id: int) -> str:
        """`readPaymentId` returns the Stripe payment of the account's purchase, for a refund.

        Raises:
            RuleError: The account has no purchase that is still paid.

        Example:
            >>> purchases.readPaymentId(1)
            'pi_test_...'
        """
        rows: list[dict] = self._bus.read(
            "SELECT payment_id FROM purchase WHERE account_id = ? AND state = 'paid'", (account_id,)
        )
        if len(rows) == 0:
            raise RuleError(f"The account {account_id} has no paid purchase.")
        return rows[0]["payment_id"]

    def receiveStripeWebhook(self, payload: bytes, signature_header: str) -> dict:
        """`receiveStripeWebhook` checks a webhook's signature and applies its event once.

        Args:
            payload: The request's raw body.
            signature_header: Its `Stripe-Signature` header.

        Returns:
            `{"received": True, "is_duplicate": ...}`, which HTTP answers with 200 so that Stripe
            stops retrying.

        Raises:
            AuthError: The signature is wrong or too old; HTTP answers 401 and Stripe retries.

        Example:
            >>> purchases.receiveStripeWebhook(body, request.headers["Stripe-Signature"])
        """
        now_s: float = self._bus.readSeconds()
        check_webhook_signature(signature_header, payload, self._webhook_secret, now_s)
        event: dict = json.loads(payload)
        data_object: dict = event["data"]["object"]
        self._bus.send(now_s, "backend", "database", "INSERT webhook_event", event["type"])
        with self._bus.transaction():
            try:
                self._bus.execute(
                    "INSERT INTO webhook_event (event_id, type, payment_id, received_at_s)"
                    " VALUES (?, ?, ?, ?)",
                    (event["id"], event["type"], data_object["payment_intent"], now_s),
                )
            except sqlite3.IntegrityError:
                return {"received": True, "is_duplicate": True}
            if event["type"] == "checkout.session.completed":
                earlier_locks: list[dict] = self._bus.read(
                    f"SELECT type FROM webhook_event WHERE payment_id = ?"
                    f" AND type IN {tuple(_LOCKING_EVENTS)}",
                    (data_object["payment_intent"],),
                )
                self._bus.execute(
                    "INSERT OR IGNORE INTO purchase (payment_id, account_id, amount_cents, state,"
                    " paid_at_s, locked_at_s) VALUES (?, ?, ?, ?, ?, ?)",
                    (
                        data_object["payment_intent"],
                        int(data_object["client_reference_id"]),
                        data_object["amount_total"],
                        _LOCKING_EVENTS[earlier_locks[0]["type"]] if earlier_locks else "paid",
                        now_s,
                        now_s if earlier_locks else None,
                    ),
                )
            elif event["type"] in _LOCKING_EVENTS:
                self._bus.execute(
                    "UPDATE purchase SET state = ?, locked_at_s = ? WHERE payment_id = ?"
                    " AND state = 'paid'",
                    (_LOCKING_EVENTS[event["type"]], now_s, data_object["payment_intent"]),
                )
        return {"received": True, "is_duplicate": False}

    def _readStripe(self) -> FakeStripe:
        """`_readStripe` returns the Stripe client, which `connectStripe` must have given.

        Raises:
            RuntimeError: `connectStripe` wasn't called.

        Example:
            >>> self._readStripe().createCheckoutSession(...)
        """
        if self._stripe is None:
            raise RuntimeError("Purchases.connectStripe must be called before a checkout.")
        return self._stripe
