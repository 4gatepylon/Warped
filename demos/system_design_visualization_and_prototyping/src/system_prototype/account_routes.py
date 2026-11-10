"""`account_routes.py` serves the backend's account and purchase API, and their simulation controls.

Interface:
    add_account_routes: Adds the routes to an app.
    Routes under `/api/auth/`, `/api/accounts`, `/api/me`, `/api/purchases/` and `/api/webhooks/`:
        what the real backend offers. Calls that need a signed-in player take
        `Authorization: Bearer <access token>`; sign-up reads the `X-Client-Region` header that
        Google Cloud's load balancer adds.
    Routes under `/api/simulation/accounts` and `/api/simulation/stripe/`: "act as", play, refund,
        dispute and redeliver, which the real system doesn't have.

Example:
    >>> add_account_routes(app, world)
"""

from datetime import date
from typing import Annotated

from fastapi import Depends, FastAPI, Header, Request
from pydantic import BaseModel

from .account_rules import ICONS
from .accounts import Session
from .identity_provider import GOOGLE_CLIENT_ID
from .rules import AuthError
from .world import World

GOOGLE_SCOPE: str = "openid email profile"
FAKE_GOOGLE_AUTHORIZE_PATH: str = "/fake-google/"
_BEARER_PREFIX: str = "Bearer "


class GoogleSignIn(BaseModel):
    """`GoogleSignIn` is what the app sends after Google's redirect.

    Example:
        >>> GoogleSignIn(code="...", code_verifier="...", nonce="...", redirect_uri="...")
    """

    code: str
    code_verifier: str
    nonce: str
    redirect_uri: str


class SignUp(BaseModel):
    """`SignUp` is the first-time form: birth date, name, icon and the terms.

    Example:
        >>> SignUp(sign_up_token="...", birth_date=date(2000, 1, 1), display_name="perihelion",
        ...        icon="blue", has_accepted_terms=True)
    """

    sign_up_token: str
    birth_date: date
    display_name: str
    icon: str
    has_accepted_terms: bool


class Refresh(BaseModel):
    """`Refresh` carries the refresh token to swap.

    Example:
        >>> Refresh(refresh_token="...")
    """

    refresh_token: str


class SignOut(BaseModel):
    """`SignOut` says whether to sign out of this computer or every computer.

    Example:
        >>> SignOut(is_everywhere=True)
    """

    is_everywhere: bool


class ProfileChange(BaseModel):
    """`ProfileChange` sets a new display name or a new icon, or both.

    Example:
        >>> ProfileChange(icon="ring")
    """

    display_name: str | None = None
    icon: str | None = None


class Play(BaseModel):
    """`Play` is what the simulation adds to an account's online play, until real matches count.

    Example:
        >>> Play(finished_match_count=1, played_s=600)
    """

    finished_match_count: int
    played_s: float


def add_account_routes(app: FastAPI, world: World) -> None:
    """`add_account_routes` adds the account, purchase and webhook routes to `app`.

    Args:
        app: The app, whose handlers already answer `RuleError`, `NotFoundError` and `AuthError`.
        world: The simulated system.

    Example:
        >>> add_account_routes(app, world)
    """

    def read_session(authorization: Annotated[str, Header()] = "") -> Session:
        """`read_session` returns whom the request's bearer token belongs to.

        Raises:
            AuthError: There is no bearer token, or `Accounts.authenticate` refuses it.

        Example:
            >>> session: Session = Depends(read_session)
        """
        if not authorization.startswith(_BEARER_PREFIX):
            raise AuthError("Sign in first.")
        return world.accounts.authenticate(authorization.removeprefix(_BEARER_PREFIX))

    signed_in = Annotated[Session, Depends(read_session)]

    @app.get("/api/auth/google/config")
    async def get_google_config() -> dict:
        """What the app needs to open Google's sign-in page: the client ID, the page and the scope.

        Example:
            >>> GET /api/auth/google/config
        """
        return {
            "client_id": GOOGLE_CLIENT_ID,
            "authorize_url": FAKE_GOOGLE_AUTHORIZE_PATH,
            "scope": GOOGLE_SCOPE,
        }

    @app.post("/api/auth/google")
    async def sign_in_with_google(sign_in: GoogleSignIn) -> dict:
        """Redeems Google's code: tokens for a known player, or a sign-up token for a new one.

        Example:
            >>> POST /api/auth/google {"code": "...", "code_verifier": "...", ...}
        """
        return world.accounts.signInWithGoogle(
            sign_in.code, sign_in.code_verifier, sign_in.nonce, sign_in.redirect_uri
        )

    @app.post("/api/accounts")
    async def sign_up(form: SignUp, x_client_region: Annotated[str, Header()]) -> dict:
        """Makes a new account and signs it in. 409 names the sign-up rule a form breaks.

        Example:
            >>> POST /api/accounts {"sign_up_token": "...", "display_name": "perihelion", ...}
        """
        return world.accounts.signUp(
            form.sign_up_token,
            form.birth_date,
            form.display_name,
            form.icon,
            form.has_accepted_terms,
            x_client_region,
        )

    @app.get("/api/accounts/name-check")
    async def check_name(display_name: str) -> dict:
        """Whether a display name may be picked, and why not.

        Example:
            >>> GET /api/accounts/name-check?display_name=admin
        """
        return world.accounts.checkName(display_name)

    @app.get("/api/icons")
    async def list_icons() -> list[str]:
        """The icons a player may pick.

        Example:
            >>> GET /api/icons
        """
        return list(ICONS)

    @app.post("/api/auth/refresh")
    async def refresh(body: Refresh) -> dict:
        """Swaps a refresh token for new tokens; 401 if unknown, expired, reused or signed out.

        Example:
            >>> POST /api/auth/refresh {"refresh_token": "..."}
        """
        return world.accounts.refreshSession(body.refresh_token)

    @app.post("/api/auth/sign-out")
    async def sign_out(body: SignOut, session: signed_in) -> dict:
        """Signs out this computer, or every computer.

        Example:
            >>> POST /api/auth/sign-out {"is_everywhere": false}
        """
        world.accounts.signOut(session, body.is_everywhere)
        return {"is_signed_out": True}

    @app.get("/api/me")
    async def get_me(session: signed_in) -> dict:
        """The signed-in player's own account, with whether they may play online.

        Example:
            >>> GET /api/me
        """
        return world.accounts.describeAccount(session.account_id)

    @app.patch("/api/me")
    async def change_me(change: ProfileChange, session: signed_in) -> dict:
        """Changes the display name (once every 30 days) or the icon.

        Example:
            >>> PATCH /api/me {"icon": "ring"}
        """
        if change.display_name is not None:
            world.accounts.changeName(session.account_id, change.display_name)
        if change.icon is not None:
            world.accounts.changeIcon(session.account_id, change.icon)
        return world.accounts.describeAccount(session.account_id)

    @app.delete("/api/me")
    async def delete_me(session: signed_in) -> dict:
        """Deletes the account and signs it out everywhere.

        Example:
            >>> DELETE /api/me
        """
        world.accounts.deleteAccount(session.account_id)
        return {"is_deleted": True}

    @app.post("/api/purchases/checkout")
    async def start_checkout(session: signed_in) -> dict:
        """Asks Stripe for a payment page; the app opens `checkout_url` in the browser.

        Example:
            >>> POST /api/purchases/checkout
        """
        return world.purchases.startCheckout(session.account_id)

    @app.post("/api/webhooks/stripe")
    async def receive_stripe_webhook(
        request: Request, stripe_signature: Annotated[str, Header()]
    ) -> dict:
        """Stripe's webhook. 401 for a bad signature, so Stripe retries; 200 for a repeat.

        Example:
            >>> POST /api/webhooks/stripe with "Stripe-Signature: t=...,v1=..."
        """
        return world.purchases.receiveStripeWebhook(await request.body(), stripe_signature)

    @app.get("/api/simulation/accounts")
    async def list_accounts() -> list[dict]:
        """Simulation only: every account, for "act as".

        Example:
            >>> GET /api/simulation/accounts
        """
        return world.accounts.listAccounts()

    @app.post("/api/simulation/accounts/{account_id}/act-as")
    async def act_as(account_id: int) -> dict:
        """Simulation only: signs this tab in as an account, which signs its other session out.

        Example:
            >>> POST /api/simulation/accounts/2/act-as
        """
        return world.accounts.actAs(account_id)

    @app.post("/api/simulation/accounts/{account_id}/play")
    async def add_play(account_id: int, play: Play) -> dict:
        """Simulation only: counts finished matches and time in matches toward the free allowance.

        Example:
            >>> POST /api/simulation/accounts/2/play {"finished_match_count": 1, "played_s": 0}
        """
        world.accounts.recordPlay(account_id, play.finished_match_count, play.played_s)
        return world.accounts.describeAccount(account_id)

    @app.post("/api/simulation/accounts/{account_id}/refund")
    async def refund(account_id: int) -> dict:
        """Simulation only: refunds the account's purchase from Stripe's side.

        Example:
            >>> POST /api/simulation/accounts/2/refund
        """
        event_id: str = world.stripe.refundPayment(world.purchases.readPaymentId(account_id))
        return {"event_id": event_id, "account": world.accounts.describeAccount(account_id)}

    @app.post("/api/simulation/accounts/{account_id}/dispute")
    async def dispute(account_id: int) -> dict:
        """Simulation only: the card's bank disputes the account's purchase.

        Example:
            >>> POST /api/simulation/accounts/2/dispute
        """
        event_id: str = world.stripe.disputePayment(world.purchases.readPaymentId(account_id))
        return {"event_id": event_id, "account": world.accounts.describeAccount(account_id)}

    @app.get("/api/simulation/stripe/events")
    async def list_stripe_events() -> list[dict]:
        """Simulation only: Stripe's events, newest first.

        Example:
            >>> GET /api/simulation/stripe/events
        """
        return world.stripe.listEvents()

    @app.post("/api/simulation/stripe/events/{event_id}/redeliver")
    async def redeliver(event_id: str) -> dict:
        """Simulation only: Stripe sends an event's webhook again, as when it retries.

        Example:
            >>> POST /api/simulation/stripe/events/evt_test_.../redeliver
        """
        return world.stripe.redeliverEvent(event_id)
