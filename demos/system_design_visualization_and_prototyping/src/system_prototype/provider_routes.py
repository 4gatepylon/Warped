"""`provider_routes.py` serves the fake Google's and the fake Stripe's own endpoints and page data.

Interface:
    add_provider_routes: Adds the routes to an app.
    Routes under `/fake-google/`: the OpenID discovery document, the JWKS, the token endpoint
        (form-encoded, as Google's), and the data of the authorize page at `/fake-google/`.
    Routes under `/fake-stripe/api/`: the data and the pay button of the payment page at
        `/fake-stripe/`.

Example:
    >>> add_provider_routes(app, world)
"""

from urllib.parse import parse_qs

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel

from .identity_provider import FAKE_GOOGLE_ISSUER
from .rules import AuthError
from .world import World

HTTP_BAD_REQUEST: int = 400


class PickedIdentity(BaseModel):
    """`PickedIdentity` is the authorize page's answer: the person picked and the request's query.

    Example:
        >>> PickedIdentity(subject="100000000000000000001", params={"client_id": "..."})
    """

    subject: str
    params: dict[str, str]


class NewIdentity(BaseModel):
    """`NewIdentity` makes up one more Google account on the authorize page.

    Example:
        >>> NewIdentity(full_name="Henri Poincaré", email="henri@example.com")
    """

    full_name: str
    email: str


class Card(BaseModel):
    """`Card` is what the player typed on the payment page.

    Example:
        >>> Card(card_number="4242 4242 4242 4242")
    """

    card_number: str


def add_provider_routes(app: FastAPI, world: World) -> None:
    """`add_provider_routes` adds the fake Google's and the fake Stripe's routes to `app`.

    Example:
        >>> add_provider_routes(app, world)
    """

    @app.get("/fake-google/.well-known/openid-configuration")
    async def get_discovery(request: Request) -> dict:
        """The fake Google's OpenID discovery document, shaped like Google's.

        Example:
            >>> GET /fake-google/.well-known/openid-configuration
        """
        base_url: str = str(request.base_url).rstrip("/")
        return {
            "issuer": FAKE_GOOGLE_ISSUER,
            "authorization_endpoint": f"{base_url}/fake-google/",
            "token_endpoint": f"{base_url}/fake-google/token",
            "jwks_uri": f"{base_url}/fake-google/jwks",
            "response_types_supported": ["code"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "code_challenge_methods_supported": ["S256"],
        }

    @app.get("/fake-google/jwks")
    async def get_jwks() -> dict:
        """The public keys that verify the fake Google's ID tokens.

        Example:
            >>> GET /fake-google/jwks
        """
        return world.google.readJwks()

    @app.post("/fake-google/token")
    async def redeem_code(request: Request) -> JSONResponse:
        """The token endpoint: a form-encoded `authorization_code` grant, answered as Google does.

        Example:
            >>> POST /fake-google/token grant_type=authorization_code&code=...&code_verifier=...
        """
        form: dict[str, list[str]] = parse_qs((await request.body()).decode("utf-8"))
        names: tuple[str, ...] = (
            "grant_type",
            "code",
            "code_verifier",
            "client_id",
            "client_secret",
            "redirect_uri",
        )
        if any(name not in form for name in names) or form["grant_type"] != ["authorization_code"]:
            return JSONResponse({"error": "invalid_request"}, status_code=HTTP_BAD_REQUEST)
        try:
            answer: dict = world.google.exchangeCode(
                form["code"][0],
                form["code_verifier"][0],
                form["client_id"][0],
                form["client_secret"][0],
                form["redirect_uri"][0],
            )
        except AuthError as error:
            error_code, description = str(error).split(": ", 1)
            return JSONResponse(
                {"error": error_code, "error_description": description},
                status_code=HTTP_BAD_REQUEST,
            )
        return JSONResponse(answer)

    @app.get("/fake-google/api/identities")
    async def list_identities() -> list[dict]:
        """The made-up people the authorize page lists.

        Example:
            >>> GET /fake-google/api/identities
        """
        return world.google.listIdentities()

    @app.post("/fake-google/api/identities")
    async def create_identity(identity: NewIdentity) -> dict:
        """Makes up one more Google account.

        Example:
            >>> POST /fake-google/api/identities {"full_name": "Henri", "email": "h@example.com"}
        """
        return world.google.createIdentity(identity.full_name, identity.email)

    @app.post("/fake-google/api/authorize")
    async def authorize(picked: PickedIdentity) -> dict:
        """Signs in as the picked person and returns where to send the browser.

        Example:
            >>> POST /fake-google/api/authorize {"subject": "...", "params": {...}}
        """
        return {"redirect_url": world.google.authorize(picked.params, picked.subject)}

    @app.get("/fake-stripe/api/checkout/{checkout_id}")
    async def read_checkout(checkout_id: str) -> dict:
        """What the payment page shows.

        Example:
            >>> GET /fake-stripe/api/checkout/cs_test_...
        """
        return world.stripe.readCheckoutSession(checkout_id)

    @app.post("/fake-stripe/api/checkout/{checkout_id}/pay")
    async def pay(checkout_id: str, card: Card) -> dict:
        """Charges a test card; 409 if it is declined.

        Example:
            >>> POST /fake-stripe/api/checkout/cs_test_.../pay {"card_number": "4242..."}
        """
        return world.stripe.payCheckout(checkout_id, card.card_number)
