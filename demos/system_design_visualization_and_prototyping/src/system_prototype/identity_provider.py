"""`identity_provider.py` plays Google's sign-in: OpenID Connect's code flow with PKCE.

The fake Google has its own made-up people, an authorize page where the player picks one, a token
endpoint that redeems a one-time code, and RS256-signed ID tokens whose public key it publishes as
a JWKS, as Google does. The backend verifies the tokens exactly as it would verify Google's.

Interface:
    FAKE_GOOGLE_ISSUER: The `iss` of every ID token.
    GOOGLE_CLIENT_ID: The app's client ID, the `aud` of every ID token.
    FakeGoogle: The fake identity provider.

Example:
    >>> google = FakeGoogle(bus)
    >>> redirect_url = google.authorize(params, "100000000000000000001")
    >>> google.exchangeCode(code, verifier, GOOGLE_CLIENT_ID, secret, redirect_uri)["id_token"]

Must know:
    Limitations:
        - Every made-up person has a verified email; real Google accounts may not.
        - Times inside tokens are simulated seconds, not Unix time.
"""

import hashlib
import json
import secrets
from urllib.parse import urlencode, urlsplit

import jwt
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa

from .account_rules import hash_token, make_code_challenge
from .bus import Bus
from .rules import AuthError, NotFoundError, RuleError

FAKE_GOOGLE_ISSUER: str = "https://accounts.fake-google.test"
GOOGLE_CLIENT_ID: str = "warped-app.apps.fake-google.test"
ID_TOKEN_S_ACCORDING_TO_GOOGLE_DOCS: float = 3600.0
_CODE_LIFETIME_S: float = 600.0
_RSA_KEY_BITS: int = 2048
_RSA_PUBLIC_EXPONENT: int = 65537
_KEY_ID_HEX_LENGTH: int = 16
_CODE_BYTES: int = 32
_SUBJECT_BASE: int = 100000000000000000000
_LOOPBACK_HOSTS: tuple[str, ...] = ("127.0.0.1", "localhost")
_APP_REDIRECT_PATH: str = "/game/"
_AUTHORIZE_PARAM_NAMES: tuple[str, ...] = (
    "client_id",
    "redirect_uri",
    "response_type",
    "scope",
    "state",
    "nonce",
    "code_challenge",
    "code_challenge_method",
)
_MADE_UP_PEOPLE: tuple[tuple[str, str], ...] = (
    ("Ada Lovelace", "ada.lovelace@example.com"),
    ("Grace Hopper", "grace.hopper@example.com"),
    ("Alan Turing", "alan.turing@example.com"),
    ("Emmy Noether", "emmy.noether@example.com"),
    ("Carl Gauss", "carl.gauss@example.com"),
    ("Sofia Kovalevskaya", "sofia.k@example.com"),
    ("Bernhard Riemann", "b.riemann@example.com"),
    ("Maryam Mirzakhani", "maryam.m@example.com"),
)


class FakeGoogle:
    """`FakeGoogle` signs players in as one of its made-up people and vouches for them.

    Args:
        bus: The shared database, clock and message log.

    Example:
        >>> FakeGoogle(bus).readJwks()["keys"][0]["alg"]
        'RS256'
    """

    def __init__(self, bus: Bus):
        self._bus: Bus = bus
        if len(bus.read("SELECT key_id FROM google_signing_key")) == 0:
            self._createSigningKey()
        for full_name, email in _MADE_UP_PEOPLE:
            if len(bus.read("SELECT subject FROM google_identity WHERE email = ?", (email,))) == 0:
                self.createIdentity(full_name, email)
        key_row: dict = bus.read("SELECT * FROM google_signing_key")[0]
        self._key_id: str = key_row["key_id"]
        self._private_key_pem: str = key_row["private_key_pem"]

    def listIdentities(self) -> list[dict]:
        """`listIdentities` returns the made-up people one can sign in as, for the authorize page.

        Example:
            >>> google.listIdentities()[0]["email"]
            'ada.lovelace@example.com'
        """
        return self._bus.read("SELECT * FROM google_identity ORDER BY subject")

    def createIdentity(self, full_name: str, email: str) -> dict:
        """`createIdentity` makes up one more Google account, as "Use another account" would.

        Raises:
            RuleError: The name is empty, or another account has the email.

        Example:
            >>> google.createIdentity("Henri Poincaré", "henri@example.com")["subject"]
            '100000000000000000009'
        """
        if full_name.strip() == "" or "@" not in email:
            raise RuleError("A Google account needs a name and an email address.")
        if len(self._bus.read("SELECT 1 FROM google_identity WHERE email = ?", (email,))) > 0:
            raise RuleError(f"A Google account with the email {email} already exists.")
        identity_count: int = self._bus.read("SELECT COUNT(*) AS n FROM google_identity")[0]["n"]
        subject: str = str(_SUBJECT_BASE + identity_count + 1)
        self._bus.execute(
            "INSERT INTO google_identity (subject, email, full_name) VALUES (?, ?, ?)",
            (subject, email, full_name.strip()),
        )
        return {"subject": subject, "email": email, "full_name": full_name.strip()}

    def authorize(self, params: dict[str, str], subject: str) -> str:
        """`authorize` signs the player in as `subject` and returns where to send their browser.

        Args:
            params: The authorize URL's query: `client_id`, `redirect_uri`, `response_type`
                ("code"), `scope` (with "openid"), `state`, `nonce`, `code_challenge` and
                `code_challenge_method` ("S256").
            subject: The made-up person the player picked.

        Returns:
            `redirect_uri` with a one-time `code` and the app's `state`.

        Raises:
            RuleError: A parameter is missing or wrong; Google shows such errors on its own page.
            NotFoundError: There is no such person.

        Example:
            >>> google.authorize(params, "100000000000000000001")
            'http://127.0.0.1:8010/game/?code=...&state=...'
        """
        self._checkAuthorizeParams(params)
        if len(self._bus.read("SELECT 1 FROM google_identity WHERE subject = ?", (subject,))) == 0:
            raise NotFoundError(f"There is no Google account {subject}.")
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "identity_provider", "sign in (browser)", subject)
        code: str = secrets.token_urlsafe(_CODE_BYTES)
        self._bus.execute(
            "INSERT INTO google_code (code_hash, subject, client_id, redirect_uri, code_challenge,"
            " nonce, expires_at_s) VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                hash_token(code),
                subject,
                params["client_id"],
                params["redirect_uri"],
                params["code_challenge"],
                params["nonce"],
                now_s + _CODE_LIFETIME_S,
            ),
        )
        return params["redirect_uri"] + "?" + urlencode({"code": code, "state": params["state"]})

    def exchangeCode(
        self, code: str, code_verifier: str, client_id: str, client_secret: str, redirect_uri: str
    ) -> dict:
        """`exchangeCode` redeems a one-time code for an ID token, as Google's token endpoint does.

        Args:
            code: The code from the redirect.
            code_verifier: The secret whose S256 hash was the `code_challenge`.
            client_id: The app's client ID.
            client_secret: The app's client secret, which only the backend holds.
            redirect_uri: The same `redirect_uri` the authorize request had.

        Returns:
            The token response: `id_token`, `token_type` and `expires_in`.

        Raises:
            AuthError: The client is wrong, or the code is unknown, used, expired, for another
                redirect, or doesn't match the verifier (OAuth's `invalid_grant`).

        Example:
            >>> google.exchangeCode(code, verifier, GOOGLE_CLIENT_ID, secret, redirect_uri)
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "backend", "identity_provider", "POST /token", "authorization_code")
        client_secret_rows: list[dict] = self._bus.read(
            "SELECT value FROM secret WHERE name = 'google_client_secret'"
        )
        if client_id != GOOGLE_CLIENT_ID or client_secret != client_secret_rows[0]["value"]:
            raise AuthError("invalid_client: the client ID or secret is wrong.")
        rows: list[dict] = self._bus.read(
            "SELECT * FROM google_code WHERE code_hash = ?", (hash_token(code),)
        )
        if len(rows) == 0:
            raise AuthError("invalid_grant: the code is unknown.")
        grant: dict = rows[0]
        if grant["used_at_s"] is not None:
            raise AuthError("invalid_grant: the code was already used.")
        self._bus.execute(
            "UPDATE google_code SET used_at_s = ? WHERE code_hash = ?", (now_s, grant["code_hash"])
        )
        if now_s > grant["expires_at_s"]:
            raise AuthError("invalid_grant: the code expired.")
        if redirect_uri != grant["redirect_uri"] or client_id != grant["client_id"]:
            raise AuthError("invalid_grant: the redirect URI doesn't match the authorize request.")
        if make_code_challenge(code_verifier) != grant["code_challenge"]:
            raise AuthError("invalid_grant: the code verifier doesn't match the code challenge.")
        person: dict = self._bus.read(
            "SELECT * FROM google_identity WHERE subject = ?", (grant["subject"],)
        )[0]
        claims: dict = {
            "iss": FAKE_GOOGLE_ISSUER,
            "aud": GOOGLE_CLIENT_ID,
            "sub": person["subject"],
            "email": person["email"],
            "email_verified": True,
            "name": person["full_name"],
            "nonce": grant["nonce"],
            "iat": int(now_s),
            "exp": int(now_s + ID_TOKEN_S_ACCORDING_TO_GOOGLE_DOCS),
        }
        id_token: str = jwt.encode(
            claims, self._private_key_pem, algorithm="RS256", headers={"kid": self._key_id}
        )
        return {
            "id_token": id_token,
            "token_type": "Bearer",
            "expires_in": int(ID_TOKEN_S_ACCORDING_TO_GOOGLE_DOCS),
        }

    def readJwks(self) -> dict:
        """`readJwks` returns the public keys that verify ID tokens, as Google's JWKS URI does.

        Example:
            >>> google.readJwks()["keys"][0]["kid"]
        """
        self._bus.send(self._bus.readSeconds(), "backend", "identity_provider", "GET /jwks")
        private_key = serialization.load_pem_private_key(
            self._private_key_pem.encode("ascii"), password=None
        )
        jwk: dict = json.loads(jwt.algorithms.RSAAlgorithm.to_jwk(private_key.public_key()))
        return {"keys": [{**jwk, "kid": self._key_id, "alg": "RS256", "use": "sig"}]}

    def _checkAuthorizeParams(self, params: dict[str, str]) -> None:
        """`_checkAuthorizeParams` refuses an authorize request Google would refuse.

        The redirect must be the app's loopback address on any port (RFC 8252, section 7.3).

        Raises:
            RuleError: A parameter is missing or wrong.

        Example:
            >>> self._checkAuthorizeParams({"client_id": GOOGLE_CLIENT_ID, ...})
        """
        for name in _AUTHORIZE_PARAM_NAMES:
            if name not in params or params[name] == "":
                raise RuleError(f"invalid_request: the parameter {name} is missing.")
        if params["client_id"] != GOOGLE_CLIENT_ID:
            raise RuleError("invalid_client: this app isn't registered with Google.")
        if params["response_type"] != "code" or params["code_challenge_method"] != "S256":
            raise RuleError("invalid_request: only the code flow with S256 PKCE is allowed.")
        if "openid" not in params["scope"].split():
            raise RuleError("invalid_scope: the scope must include openid.")
        redirect = urlsplit(params["redirect_uri"])
        if (
            redirect.scheme != "http"
            or redirect.hostname not in _LOOPBACK_HOSTS
            or redirect.path != _APP_REDIRECT_PATH
            or redirect.query != ""
        ):
            raise RuleError("redirect_uri_mismatch: the redirect URI isn't registered.")

    def _createSigningKey(self) -> None:
        """`_createSigningKey` makes the RSA key pair that signs ID tokens, once per world.

        Example:
            >>> self._createSigningKey()
        """
        private_key = rsa.generate_private_key(
            public_exponent=_RSA_PUBLIC_EXPONENT, key_size=_RSA_KEY_BITS
        )
        private_key_pem: str = private_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.PKCS8,
            serialization.NoEncryption(),
        ).decode("ascii")
        public_der: bytes = private_key.public_key().public_bytes(
            serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo
        )
        key_id: str = hashlib.sha256(public_der).hexdigest()[:_KEY_ID_HEX_LENGTH]
        self._bus.execute(
            "INSERT INTO google_signing_key (key_id, private_key_pem) VALUES (?, ?)",
            (key_id, private_key_pem),
        )
