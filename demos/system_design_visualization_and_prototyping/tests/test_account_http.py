"""`test_account_http.py` checks the account, purchase and provider routes over HTTP.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| flow | sign in, sign up, pay, as the app and the browser do | `test_the_whole_flow_over_http` |
| bearer token | none, made up | `test_calls_without_a_session_are_401` |
| sign-up region | allowed | `test_the_whole_flow_over_http` |
| | EU | `test_eu_connections_are_refused` |
| token endpoint form | wrong grant, a used code | `test_token_endpoint_answers_as_google` |
| webhook | right signature | `test_the_whole_flow_over_http` |
| | forged | `test_forged_webhook_is_401` |
"""

from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from fastapi.testclient import TestClient
from sign_in_helpers import ADA, ADULT_BIRTH_DATE, REDIRECT_URI, authorize_params, make_pkce

from system_prototype.clock import SimClock
from system_prototype.http_api import make_app
from system_prototype.identity_provider import GOOGLE_CLIENT_ID
from system_prototype.payment_provider import SUCCEEDING_CARD
from system_prototype.store import open_store
from system_prototype.world import World

_NONCE: str = "nonce-1"


def _make_client(tmp_path: Path) -> TestClient:
    """`_make_client` returns a client for a new world.

    Example:
        >>> client = _make_client(tmp_path)
    """
    world: World = World(open_store(Path(":memory:")), SimClock())
    return TestClient(make_app(world, tmp_path))


def _sign_in(client: TestClient) -> tuple[dict, str]:
    """`_sign_in` picks Ada on the fake Google's page and hands the code to the backend.

    Returns:
        The backend's answer and the code, which is now used.

    Example:
        >>> answer, code = _sign_in(client)
    """
    verifier, challenge = make_pkce()
    picked = client.post(
        "/fake-google/api/authorize",
        json={"subject": ADA, "params": authorize_params(challenge, _NONCE)},
    )
    code: str = parse_qs(urlsplit(picked.json()["redirect_url"]).query)["code"][0]
    answer = client.post(
        "/api/auth/google",
        json={
            "code": code,
            "code_verifier": verifier,
            "nonce": _NONCE,
            "redirect_uri": REDIRECT_URI,
        },
    )
    return answer.json(), code


def _sign_up(client: TestClient, region: str) -> dict:
    """`_sign_up` signs Ada in and sends the sign-up form from `region`; returns the response.

    Example:
        >>> _sign_up(client, "US").status_code
        200
    """
    answer, _ = _sign_in(client)
    return client.post(
        "/api/accounts",
        headers={"X-Client-Region": region},
        json={
            "sign_up_token": answer["sign_up_token"],
            "birth_date": ADULT_BIRTH_DATE.isoformat(),
            "display_name": "perihelion",
            "icon": "ring",
            "has_accepted_terms": True,
        },
    )


def test_the_whole_flow_over_http(tmp_path: Path) -> None:
    """`test_the_whole_flow_over_http` signs up, pays on the fake Stripe and sees online play."""
    with _make_client(tmp_path) as client:
        assert client.get("/api/auth/google/config").json()["client_id"] == GOOGLE_CLIENT_ID
        signed_up = _sign_up(client, "US")
        assert signed_up.status_code == 200
        bearer: dict = {"Authorization": f"Bearer {signed_up.json()['access_token']}"}
        assert client.get("/api/me", headers=bearer).json()["display_name"] == "perihelion"
        checkout_url: str = client.post("/api/purchases/checkout", headers=bearer).json()[
            "checkout_url"
        ]
        checkout_id: str = parse_qs(urlsplit(checkout_url).query)["session"][0]
        paid = client.post(
            f"/fake-stripe/api/checkout/{checkout_id}/pay", json={"card_number": SUCCEEDING_CARD}
        )
        assert paid.json() == {"redirect_url": "/site/#paid"}
        me: dict = client.get("/api/me", headers=bearer).json()
        assert (me["has_purchase"], me["online_access"]["reason"]) == (True, "purchased")


def test_calls_without_a_session_are_401(tmp_path: Path) -> None:
    """`test_calls_without_a_session_are_401` checks a missing and a made-up bearer token."""
    with _make_client(tmp_path) as client:
        assert client.get("/api/me").status_code == 401
        made_up = client.get("/api/me", headers={"Authorization": "Bearer not-a-token"})
        assert made_up.status_code == 401
        assert client.post("/api/purchases/checkout").status_code == 401


def test_eu_connections_are_refused(tmp_path: Path) -> None:
    """`test_eu_connections_are_refused` checks a sign-up from France."""
    with _make_client(tmp_path) as client:
        refused = _sign_up(client, "FR")
        assert refused.status_code == 409
        assert "European Union" in refused.json()["detail"]


def test_token_endpoint_answers_as_google(tmp_path: Path) -> None:
    """`test_token_endpoint_answers_as_google` checks a wrong grant and a code used twice."""
    with _make_client(tmp_path) as client:
        _, used_code = _sign_in(client)
        form: dict = {
            "grant_type": "authorization_code",
            "code": used_code,
            "code_verifier": "x" * 43,
            "client_id": GOOGLE_CLIENT_ID,
            "client_secret": "s",
            "redirect_uri": REDIRECT_URI,
        }
        wrong_grant = client.post("/fake-google/token", data={**form, "grant_type": "password"})
        reused = client.post("/fake-google/token", data=form)
        assert (wrong_grant.status_code, wrong_grant.json()["error"]) == (400, "invalid_request")
        assert reused.status_code == 400
        assert reused.json()["error"] in {"invalid_grant", "invalid_client"}


def test_forged_webhook_is_401(tmp_path: Path) -> None:
    """`test_forged_webhook_is_401` checks a webhook signed with a guessed secret."""
    with _make_client(tmp_path) as client:
        forged = client.post(
            "/api/webhooks/stripe",
            content=b'{"id": "evt_1", "type": "checkout.session.completed"}',
            headers={"Stripe-Signature": "t=0,v1=00"},
        )
        assert forged.status_code == 401
        assert client.get("/fake-google/jwks").json()["keys"][0]["alg"] == "RS256"
