"""`sign_in_helpers.py` plays the app's side of sign-in and payment for the tests.

Interface:
    Hands: The real time that a test's clock reads, moved by the test.
    make_world: A new world on a clock the test moves.
    make_pkce: A code verifier and its challenge.
    authorize_params: The query of an authorize request, as the app builds it.
    read_code: The code in the URL Google redirects to.
    sign_in: Signs in with Google as a made-up person, as the app does.
    sign_up: Signs a new person in and up.
    pay: Buys online play for a signed-in account through the fake Stripe.

Example:
    >>> world, hands = make_world()
    >>> tokens = sign_up(world, ADA)
"""

import secrets
from datetime import date
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from system_prototype.account_rules import make_code_challenge
from system_prototype.clock import SimClock
from system_prototype.identity_provider import GOOGLE_CLIENT_ID
from system_prototype.payment_provider import SUCCEEDING_CARD
from system_prototype.store import open_store
from system_prototype.world import World

ADA: str = "100000000000000000001"
GRACE: str = "100000000000000000002"
REDIRECT_URI: str = "http://127.0.0.1:8010/game/"
ADULT_BIRTH_DATE: date = date(2000, 1, 1)
_NONCE_BYTES: int = 16


class Hands:
    """`Hands` is the real time that a test's clock reads, moved by the test.

    Example:
        >>> hands = Hands(); hands.real_s += 1.0
    """

    real_s: float = 0.0


def make_world() -> tuple[World, Hands]:
    """`make_world` returns a new world in memory and the hands of its clock.

    Example:
        >>> world, hands = make_world()
    """
    hands: Hands = Hands()
    return World(open_store(Path(":memory:")), SimClock(read_real_s=lambda: hands.real_s)), hands


def make_pkce() -> tuple[str, str]:
    """`make_pkce` returns a new code verifier and its S256 challenge.

    Example:
        >>> verifier, challenge = make_pkce()
    """
    verifier: str = secrets.token_urlsafe(_NONCE_BYTES * 2)
    return verifier, make_code_challenge(verifier)


def authorize_params(challenge: str, nonce: str, **changes: str) -> dict[str, str]:
    """`authorize_params` returns the authorize request's query, with `changes` applied.

    Example:
        >>> authorize_params(challenge, "n-1", redirect_uri="http://evil.test/")
    """
    return {
        "client_id": GOOGLE_CLIENT_ID,
        "redirect_uri": REDIRECT_URI,
        "response_type": "code",
        "scope": "openid email profile",
        "state": "state-1",
        "nonce": nonce,
        "code_challenge": challenge,
        "code_challenge_method": "S256",
        **changes,
    }


def read_code(redirect_url: str) -> str:
    """`read_code` returns the `code` in the URL Google sends the browser back to.

    Example:
        >>> read_code("http://127.0.0.1:8010/game/?code=abc&state=s")
        'abc'
    """
    return parse_qs(urlsplit(redirect_url).query)["code"][0]


def sign_in(world: World, subject: str) -> dict:
    """`sign_in` signs in with Google as `subject`, as the app does; returns the backend's answer.

    Example:
        >>> sign_in(world, ADA)["status"]
        'needs_sign_up'
    """
    verifier, challenge = make_pkce()
    nonce: str = secrets.token_urlsafe(_NONCE_BYTES)
    code: str = read_code(world.google.authorize(authorize_params(challenge, nonce), subject))
    return world.accounts.signInWithGoogle(code, verifier, nonce, REDIRECT_URI)


def sign_up(world: World, subject: str, display_name: str = "perihelion") -> dict:
    """`sign_up` signs a new person in with Google and up with good answers; returns the tokens.

    Example:
        >>> sign_up(world, ADA)["account"]["display_name"]
        'perihelion'
    """
    answer: dict = sign_in(world, subject)
    return world.accounts.signUp(
        answer["sign_up_token"], ADULT_BIRTH_DATE, display_name, "blue", True, "US"
    )


def pay(world: World, account_id: int) -> str:
    """`pay` buys online play for an account with the test card that succeeds; returns the payment.

    Example:
        >>> payment_id = pay(world, 1)
    """
    checkout_url: str = world.purchases.startCheckout(account_id)["checkout_url"]
    checkout_id: str = parse_qs(urlsplit(checkout_url).query)["session"][0]
    world.stripe.payCheckout(checkout_id, SUCCEEDING_CARD)
    return world.purchases.readPaymentId(account_id)
