"""`test_purchases.py` checks buying online play through the fake Stripe and its webhooks.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| card | succeeds | `test_paying_unlocks_online_play` |
| | declined, then succeeds | `test_declined_card_leaves_the_page_open` |
| | not a test card | `test_declined_card_leaves_the_page_open` |
| payment page | open, already paid | `test_declined_card_leaves_the_page_open` |
| account | past its free allowance | `test_paying_unlocks_online_play` |
| | already purchased | `test_one_purchase_per_account` |
| webhook delivery | first | `test_paying_unlocks_online_play` |
| | again (Stripe's retry) | `test_redelivered_webhook_changes_nothing` |
| webhook signature | forged, replayed late | `test_forged_and_replayed_webhooks_are_refused` |
| payment's end | refunded, disputed | `test_refund_or_dispute_locks_online_play` |
| | refunded twice | `test_refund_or_dispute_locks_online_play` |
| event order | refund before its payment's completion | `test_refund_arriving_first_still_locks` |
"""

import json
from urllib.parse import parse_qs, urlsplit

import pytest
from sign_in_helpers import ADA, make_world, pay, sign_up

from system_prototype.account_rules import sign_webhook
from system_prototype.payment_provider import DECLINED_CARD, SUCCEEDING_CARD
from system_prototype.rules import AuthError, RuleError
from system_prototype.world import World

FREE_MATCH_S: float = 1800.0
PRICE_CENTS: int = 1337


def _read_webhook_secret(world: World) -> str:
    """`_read_webhook_secret` returns the secret that only Stripe and the backend know.

    Example:
        >>> secret = _read_webhook_secret(world)
    """
    return world.secret_manager.readSecret("payment_provider", "stripe_webhook_secret")


def _start_checkout(world: World, account_id: int) -> str:
    """`_start_checkout` opens a payment page for an account and returns its session ID.

    Example:
        >>> checkout_id = _start_checkout(world, 1)
    """
    checkout_url: str = world.purchases.startCheckout(account_id)["checkout_url"]
    return parse_qs(urlsplit(checkout_url).query)["session"][0]


def _read_table(world: World, name: str) -> dict:
    """`_read_table` returns a table as the system view shows it: its row count and newest rows.

    Example:
        >>> _read_table(world, "purchase")["row_count"]
    """
    return next(table for table in world.describeSystem()["tables"] if table["name"] == name)


def _make_event(event_id: str, event_type: str, data_object: dict) -> bytes:
    """`_make_event` returns the body of a Stripe event, as the backend receives it.

    Example:
        >>> _make_event("evt_1", "charge.refunded", {"payment_intent": "pi_1"})
    """
    return json.dumps(
        {"id": event_id, "type": event_type, "data": {"object": data_object}}
    ).encode()


def test_paying_unlocks_online_play() -> None:
    """`test_paying_unlocks_online_play` checks an account past its free allowance that pays."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    world.accounts.recordPlay(account_id, finished_match_count=0, played_s=FREE_MATCH_S)
    assert not world.accounts.describeAccount(account_id)["online_access"]["can_play_online"]
    pay(world, account_id)
    account: dict = world.accounts.describeAccount(account_id)
    assert account["has_purchase"]
    assert account["online_access"]["can_play_online"]
    assert account["online_access"]["reason"] == "purchased"
    assert [event["type"] for event in world.stripe.listEvents()] == ["checkout.session.completed"]


def test_declined_card_leaves_the_page_open() -> None:
    """`test_declined_card_leaves_the_page_open` checks a decline, a made-up card, a payment."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    checkout_id: str = _start_checkout(world, account_id)
    with pytest.raises(RuleError, match="declined"):
        world.stripe.payCheckout(checkout_id, DECLINED_CARD)
    with pytest.raises(RuleError, match="stand-in"):
        world.stripe.payCheckout(checkout_id, "5555 5555 5555 4444")
    assert not world.accounts.describeAccount(account_id)["has_purchase"]
    assert world.stripe.readCheckoutSession(checkout_id)["amount_cents"] == PRICE_CENTS
    assert world.stripe.payCheckout(checkout_id, SUCCEEDING_CARD) == {"redirect_url": "/site/#paid"}
    with pytest.raises(RuleError, match="already paid"):
        world.stripe.payCheckout(checkout_id, SUCCEEDING_CARD)


def test_one_purchase_per_account() -> None:
    """`test_one_purchase_per_account` checks that a second checkout is refused."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    pay(world, account_id)
    with pytest.raises(RuleError, match="already bought"):
        world.purchases.startCheckout(account_id)


def test_redelivered_webhook_changes_nothing() -> None:
    """`test_redelivered_webhook_changes_nothing` checks that a repeat is answered and ignored."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    pay(world, account_id)
    event_id: str = world.stripe.listEvents()[0]["event_id"]
    assert world.stripe.redeliverEvent(event_id) == {"received": True, "is_duplicate": True}
    assert world.stripe.listEvents()[0]["delivery_count"] == 2
    assert _read_table(world, "purchase")["row_count"] == 1
    assert _read_table(world, "webhook_event")["row_count"] == 1


def test_forged_and_replayed_webhooks_are_refused() -> None:
    """`test_forged_and_replayed_webhooks_are_refused` checks a wrong key and an old signature."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    body: bytes = _make_event(
        "evt_forged",
        "checkout.session.completed",
        {"client_reference_id": str(account_id), "payment_intent": "pi_x", "amount_total": 1},
    )
    with pytest.raises(AuthError, match="doesn't match"):
        world.purchases.receiveStripeWebhook(body, sign_webhook("whsec_guess", 0.0, body))
    old_signature: str = sign_webhook(_read_webhook_secret(world), 0.0, body)
    world.skipClock(301.0)
    with pytest.raises(AuthError, match="too far"):
        world.purchases.receiveStripeWebhook(body, old_signature)
    assert not world.accounts.describeAccount(account_id)["has_purchase"]


def test_refund_or_dispute_locks_online_play() -> None:
    """`test_refund_or_dispute_locks_online_play` checks both, a second refund, and buying again."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    world.accounts.recordPlay(account_id, finished_match_count=1, played_s=0.0)
    payment_id: str = pay(world, account_id)
    world.stripe.refundPayment(payment_id)
    account: dict = world.accounts.describeAccount(account_id)
    assert (account["has_purchase"], account["online_access"]["can_play_online"]) == (False, False)
    with pytest.raises(RuleError, match="already refunded"):
        world.stripe.refundPayment(payment_id)
    second_payment_id: str = pay(world, account_id)
    assert world.accounts.describeAccount(account_id)["online_access"]["can_play_online"]
    world.stripe.disputePayment(second_payment_id)
    assert not world.accounts.describeAccount(account_id)["online_access"]["can_play_online"]


def test_refund_arriving_first_still_locks() -> None:
    """`test_refund_arriving_first_still_locks` checks events that arrive out of order."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    secret: str = _read_webhook_secret(world)
    refund: bytes = _make_event("evt_2", "charge.refunded", {"payment_intent": "pi_1"})
    completed: bytes = _make_event(
        "evt_1",
        "checkout.session.completed",
        {"client_reference_id": str(account_id), "payment_intent": "pi_1", "amount_total": 1337},
    )
    for body in (refund, completed):
        world.purchases.receiveStripeWebhook(body, sign_webhook(secret, 0.0, body))
    assert not world.accounts.describeAccount(account_id)["has_purchase"]
    assert _read_table(world, "purchase")["rows"][0]["state"] == "refunded"
