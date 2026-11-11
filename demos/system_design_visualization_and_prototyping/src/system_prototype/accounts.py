"""`accounts.py` is the backend's part that signs players in and keeps their accounts.

A player signs in with Google: the app sends the backend the one-time code and its PKCE verifier,
the backend redeems them with Google and checks the ID token. A new Google account then signs up
(birth date, name, icon, terms); a known one gets a session at once. A session is a family of
tokens: a short-lived access token for each call and a refresh token that is replaced each time it
is used. Using a refresh token twice means it was copied, so the whole family is signed out.

Interface:
    PLACEHOLDER_NAME: The name a deleted account shows in other players' histories.
    Session: Whom an access token belongs to.
    Accounts: Sign-in, sign-up, sessions and the account's own settings.

Example:
    >>> accounts = Accounts(bus, google, secret_manager)
    >>> answer = accounts.signInWithGoogle(code, verifier, nonce, redirect_uri)
    >>> answer["status"]
    'needs_sign_up'

Must know:
    Limitations:
        - Times inside tokens are simulated seconds, so tokens from another world don't work.
"""

import re
import secrets
from dataclasses import dataclass
from datetime import date

import jwt

from .account_rules import (
    ICONS,
    MAX_NAME_LENGTH,
    check_display_name,
    describe_online_access,
    hash_token,
    is_country_allowed,
    is_old_enough,
    read_name_change_wait_s,
    read_sim_date,
)
from .bus import Bus
from .identity_provider import FAKE_GOOGLE_ISSUER, GOOGLE_CLIENT_ID, FakeGoogle
from .rules import AuthError, NotFoundError, RuleError
from .secret_manager import SecretManager

PLACEHOLDER_NAME: str = "deleted player"
GOOGLE_PROVIDER: str = "google"
_BACKEND_TOKEN_ALGORITHM: str = "HS256"
_SIGN_UP_WINDOW_S: float = 600.0
_REFRESH_TOKEN_BYTES: int = 32
SECONDS_PER_MINUTE: int = 60
SECONDS_PER_DAY: int = 86400
_SIGNED_IN_ELSEWHERE: str = (
    "You were signed out because your account signed in on another computer."
)
_SIGNED_OUT: str = "You signed out."
_SIGNED_OUT_EVERYWHERE: str = "You signed out of every computer."
_REFRESH_TOKEN_REUSED: str = (
    "You were signed out because an old sign-in token was used again, which can mean it was copied."
)
_ACCOUNT_DELETED: str = "This account was deleted."
_NOT_NAME_CHARACTER: re.Pattern = re.compile(r"[^A-Za-z0-9_]")


@dataclass(frozen=True)
class Session:
    """`Session` says which account and which session family an access token belongs to.

    Example:
        >>> Session(account_id=1, family_id=3)
    """

    account_id: int
    family_id: int


class Accounts:
    """`Accounts` signs players in with Google, signs them up, and keeps one session per account.

    Args:
        bus: The shared database, clock and message log.
        google: The identity provider the backend redeems codes with.
        secret_manager: Where the backend reads its keys, once, when it starts.

    Example:
        >>> accounts = Accounts(bus, google, secret_manager)
    """

    def __init__(self, bus: Bus, google: FakeGoogle, secret_manager: SecretManager):
        self._bus: Bus = bus
        self._google: FakeGoogle = google
        self._access_token_key: str = secret_manager.readSecret("backend", "access_token_key")
        self._sign_up_token_key: str = secret_manager.readSecret("backend", "sign_up_token_key")
        self._google_client_secret: str = secret_manager.readSecret(
            "backend", "google_client_secret"
        )
        self._google_keys: dict[str, dict] = {}

    def signInWithGoogle(
        self, code: str, code_verifier: str, nonce: str, redirect_uri: str
    ) -> dict:
        """`signInWithGoogle` redeems the app's code with Google and signs the player in or up.

        Args:
            code: The one-time code Google's redirect gave the app.
            code_verifier: The app's PKCE secret for this sign-in.
            nonce: The nonce the app put in the authorize request, which the ID token must carry.
            redirect_uri: The redirect the authorize request named.

        Returns:
            Either `{"status": "signed_in", ...tokens}` for a known Google account, or
            `{"status": "needs_sign_up", "sign_up_token", "email", "suggested_name"}`.

        Raises:
            AuthError: Google refused the code, or the ID token is wrong.

        Example:
            >>> accounts.signInWithGoogle(code, verifier, nonce, "http://127.0.0.1:8010/game/")
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "POST /auth/google")
        token_response: dict = self._google.exchangeCode(
            code, code_verifier, GOOGLE_CLIENT_ID, self._google_client_secret, redirect_uri
        )
        claims: dict = self._verifyIdToken(token_response["id_token"], nonce, now_s)
        self._bus.send(now_s, "backend", "database", "SELECT identity", claims["email"])
        rows: list[dict] = self._bus.read(
            "SELECT account_id FROM identity WHERE provider = ? AND subject = ?",
            (GOOGLE_PROVIDER, claims["sub"]),
        )
        if len(rows) > 0:
            return {"status": "signed_in", **self._startSession(rows[0]["account_id"], now_s)}
        sign_up_token: str = jwt.encode(
            {
                "sub": claims["sub"],
                "email": claims["email"],
                "exp": int(now_s + _SIGN_UP_WINDOW_S),
            },
            self._sign_up_token_key,
            algorithm=_BACKEND_TOKEN_ALGORITHM,
        )
        return {
            "status": "needs_sign_up",
            "sign_up_token": sign_up_token,
            "email": claims["email"],
            "suggested_name": self._suggestName(claims["name"]),
        }

    def signUp(
        self,
        sign_up_token: str,
        birth_date: date,
        display_name: str,
        icon: str,
        has_accepted_terms: bool,
        country_code: str,
    ) -> dict:
        """`signUp` makes the account of a Google account that has none, and signs it in.

        Warped keeps that the player confirmed being 13 or older, and when, but not the date.

        Args:
            sign_up_token: From `signInWithGoogle`, at most 10 minutes old.
            birth_date: The date the player entered.
            display_name: The name they picked.
            icon: One of `ICONS`.
            has_accepted_terms: Whether they accepted the terms, privacy policy and code of conduct.
            country_code: Where the connection comes from, as the load balancer sees it.

        Returns:
            `{"status": "signed_in", ...tokens}`.

        Raises:
            AuthError: The sign-up token is wrong or expired.
            RuleError: A sign-up rule is broken; the message says which.

        Example:
            >>> accounts.signUp(token, date(2000, 1, 1), "perihelion", "blue", True, "US")
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "POST /accounts", display_name)
        claims: dict = self._decodeBackendToken(sign_up_token, self._sign_up_token_key, now_s)
        if not is_country_allowed(country_code):
            raise RuleError("Warped isn't offered in the European Union.")
        if not is_old_enough(birth_date, read_sim_date(now_s)):
            raise RuleError("You must be 13 or older to play Warped.")
        if not has_accepted_terms:
            raise RuleError(
                "To sign up, accept the terms, the privacy policy and the code of conduct."
            )
        if icon not in ICONS:
            raise RuleError(f"Pick one of the icons the game ships with, not {icon}.")
        self._checkNameIsFree(display_name)
        if (
            len(
                self._bus.read(
                    "SELECT 1 FROM identity WHERE provider = ? AND subject = ?",
                    (GOOGLE_PROVIDER, claims["sub"]),
                )
            )
            > 0
        ):
            raise RuleError("This Google account already has a Warped account.")
        used_rows: list[dict] = self._bus.read(
            "SELECT * FROM used_allowance WHERE identity_hash = ?",
            (self._hashIdentity(claims["sub"]),),
        )
        finished_match_count: int = used_rows[0]["finished_match_count"] if used_rows else 0
        played_s: float = used_rows[0]["played_s"] if used_rows else 0.0
        self._bus.send(now_s, "backend", "database", "INSERT account", display_name)
        account_id: int = self._bus.execute(
            "INSERT INTO account (display_name, icon, role, state, created_at_s,"
            " age_confirmed_at_s, terms_accepted_at_s, name_changed_at_s, finished_match_count,"
            " played_s)"
            " VALUES (?, ?, 'player', 'active', ?, ?, ?, ?, ?, ?)",
            (display_name, icon, now_s, now_s, now_s, now_s, finished_match_count, played_s),
        ).lastrowid
        self._bus.execute(
            "INSERT INTO identity (provider, subject, account_id, email) VALUES (?, ?, ?, ?)",
            (GOOGLE_PROVIDER, claims["sub"], account_id, claims["email"]),
        )
        return {"status": "signed_in", **self._startSession(account_id, now_s)}

    def refreshSession(self, refresh_token: str) -> dict:
        """`refreshSession` swaps a refresh token for a new access token and a new refresh token.

        Raises:
            AuthError: The token is unknown, expired, or its session was signed out. A token used
                a second time signs its whole session out.

        Example:
            >>> accounts.refreshSession(tokens["refresh_token"])["access_token"]
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "POST /auth/refresh")
        self._bus.send(now_s, "backend", "database", "SELECT refresh_token")
        rows: list[dict] = self._bus.read(
            "SELECT t.*, f.account_id, f.revoked_because FROM refresh_token t"
            " JOIN session_family f USING (family_id) WHERE t.token_hash = ?",
            (hash_token(refresh_token),),
        )
        if len(rows) == 0:
            raise AuthError("This sign-in token is unknown. Sign in again.")
        token: dict = rows[0]
        if token["revoked_because"] is not None:
            raise AuthError(token["revoked_because"])
        if token["used_at_s"] is not None:
            self._revokeFamilies(
                "family_id = ?", (token["family_id"],), _REFRESH_TOKEN_REUSED, now_s
            )
            raise AuthError(_REFRESH_TOKEN_REUSED)
        if now_s > token["expires_at_s"]:
            raise AuthError("Your sign-in expired. Sign in again.")
        self._bus.execute(
            "UPDATE refresh_token SET used_at_s = ? WHERE token_hash = ?",
            (now_s, token["token_hash"]),
        )
        return self._issueTokens(token["account_id"], token["family_id"], now_s)

    def authenticate(self, access_token: str) -> Session:
        """`authenticate` returns whom an access token belongs to, if its session is still on.

        Raises:
            AuthError: The token is wrong or expired, or its session was signed out.

        Example:
            >>> accounts.authenticate(tokens["access_token"]).account_id
            1
        """
        now_s: float = self._bus.readSeconds()
        claims: dict = self._decodeBackendToken(access_token, self._access_token_key, now_s)
        family_id: int = claims["sid"]
        rows: list[dict] = self._bus.read(
            "SELECT revoked_because FROM session_family WHERE family_id = ?", (family_id,)
        )
        if len(rows) == 0:
            raise AuthError("This session is unknown. Sign in again.")
        if rows[0]["revoked_because"] is not None:
            raise AuthError(rows[0]["revoked_because"])
        return Session(account_id=int(claims["sub"]), family_id=family_id)

    def signOut(self, session: Session, is_everywhere: bool) -> None:
        """`signOut` ends this session, or every session of the account.

        Example:
            >>> accounts.signOut(session, is_everywhere=True)
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "POST /auth/sign-out")
        if is_everywhere:
            self._revokeFamilies(
                "account_id = ?", (session.account_id,), _SIGNED_OUT_EVERYWHERE, now_s
            )
        else:
            self._revokeFamilies("family_id = ?", (session.family_id,), _SIGNED_OUT, now_s)

    def describeAccount(self, account_id: int) -> dict:
        """`describeAccount` returns what the app shows about the signed-in player's own account.

        Raises:
            NotFoundError: There is no such account.

        Example:
            >>> accounts.describeAccount(1)["online_access"]["can_play_online"]
            True
        """
        now_s: float = self._bus.readSeconds()
        rows: list[dict] = self._bus.read(
            "SELECT a.*, EXISTS (SELECT 1 FROM purchase p WHERE p.account_id = a.account_id"
            " AND p.state = 'paid') AS has_purchase FROM account a WHERE a.account_id = ?",
            (account_id,),
        )
        if len(rows) == 0:
            raise NotFoundError(f"There is no account {account_id}.")
        account: dict = rows[0]
        settings: dict[str, float] = self._bus.readSettings()
        access = describe_online_access(
            bool(account["has_purchase"]),
            account["finished_match_count"],
            account["played_s"],
            settings,
        )
        return {
            "account_id": account["account_id"],
            "display_name": account["display_name"],
            "icon": account["icon"],
            "role": account["role"],
            "state": account["state"],
            "has_purchase": bool(account["has_purchase"]),
            "finished_match_count": account["finished_match_count"],
            "played_s": account["played_s"],
            "name_change_wait_s": read_name_change_wait_s(
                account["name_changed_at_s"], now_s, settings["name_change_days"]
            ),
            "online_access": {
                "can_play_online": access.can_play_online,
                "reason": access.reason,
                "free_matches_left": access.free_matches_left,
                "free_s_left": access.free_s_left,
            },
        }

    def checkName(self, display_name: str) -> dict:
        """`checkName` says whether a name may be picked, for the sign-up form to show as one types.

        Example:
            >>> accounts.checkName("admin")
            {'is_free': False, 'reason': 'A name can\\'t contain "admin", ...'}
        """
        try:
            self._checkNameIsFree(display_name)
        except RuleError as error:
            return {"is_free": False, "reason": str(error)}
        return {"is_free": True, "reason": ""}

    def changeName(self, account_id: int, display_name: str) -> None:
        """`changeName` gives an account a new display name, at most once every 30 days.

        Raises:
            RuleError: The name breaks a rule, is taken, or the last change was too recent.

        Example:
            >>> accounts.changeName(1, "aphelion")
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "PATCH /me", "display_name")
        account: dict = self.describeAccount(account_id)
        if account["name_change_wait_s"] > 0:
            next_day: date = read_sim_date(now_s + account["name_change_wait_s"])
            raise RuleError(f"You can change your name again on {next_day:%d %B %Y}.")
        if display_name == account["display_name"]:
            raise RuleError("That is already your name.")
        if display_name.lower() != account["display_name"].lower():
            self._checkNameIsFree(display_name)
        else:
            check_display_name(display_name)
        self._bus.send(now_s, "backend", "database", "UPDATE account", "display_name")
        self._bus.execute(
            "UPDATE account SET display_name = ?, name_changed_at_s = ? WHERE account_id = ?",
            (display_name, now_s, account_id),
        )

    def changeIcon(self, account_id: int, icon: str) -> None:
        """`changeIcon` gives an account another of the icons the game ships with.

        Raises:
            RuleError: `icon` isn't one of `ICONS`.

        Example:
            >>> accounts.changeIcon(1, "ring")
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "PATCH /me", "icon")
        if icon not in ICONS:
            raise RuleError(f"Pick one of the icons the game ships with, not {icon}.")
        self._bus.execute("UPDATE account SET icon = ? WHERE account_id = ?", (icon, account_id))

    def deleteAccount(self, account_id: int) -> None:
        """`deleteAccount` deletes an account's name, icon and sign-in, and signs it out everywhere.

        The account's row stays, so past matches can show `PLACEHOLDER_NAME`, and so does its
        purchase, which is a financial record. A hash of the Google account remembers how much free
        allowance it used, so signing up again doesn't give a new one.

        Example:
            >>> accounts.deleteAccount(1)
        """
        now_s: float = self._bus.readSeconds()
        self._bus.send(now_s, "game_app", "backend", "DELETE /me")
        account: dict = self.describeAccount(account_id)
        for identity in self._bus.read(
            "SELECT subject FROM identity WHERE account_id = ?", (account_id,)
        ):
            self._bus.execute(
                "INSERT OR REPLACE INTO used_allowance (identity_hash, finished_match_count,"
                " played_s) VALUES (?, ?, ?)",
                (
                    self._hashIdentity(identity["subject"]),
                    account["finished_match_count"],
                    account["played_s"],
                ),
            )
        self._bus.send(now_s, "backend", "database", "UPDATE account", "deleted")
        self._bus.execute("DELETE FROM identity WHERE account_id = ?", (account_id,))
        self._bus.execute(
            "UPDATE account SET display_name = NULL, icon = NULL, state = 'deleted',"
            " deleted_at_s = ? WHERE account_id = ?",
            (now_s, account_id),
        )
        self._revokeFamilies("account_id = ?", (account_id,), _ACCOUNT_DELETED, now_s)

    def readDisplayName(self, account_id: int) -> str:
        """`readDisplayName` returns the name other players see, or `PLACEHOLDER_NAME` once deleted.

        Example:
            >>> accounts.readDisplayName(1)
            'perihelion'
        """
        name: str | None = self.describeAccount(account_id)["display_name"]
        return PLACEHOLDER_NAME if name is None else name

    def recordPlay(self, account_id: int, finished_match_count: int, played_s: float) -> None:
        """`recordPlay` adds finished online matches and time in them, using up free allowance.

        Example:
            >>> accounts.recordPlay(1, finished_match_count=1, played_s=600.0)
        """
        self._bus.execute(
            "UPDATE account SET finished_match_count = finished_match_count + ?,"
            " played_s = played_s + ? WHERE account_id = ?",
            (finished_match_count, played_s, account_id),
        )

    def listAccounts(self) -> list[dict]:
        """`listAccounts` returns every account and its Google email, for the simulation's "act as".

        Example:
            >>> accounts.listAccounts()[0]["display_name"]
        """
        return self._bus.read(
            "SELECT a.account_id, a.display_name, a.state, i.email FROM account a"
            " LEFT JOIN identity i USING (account_id) ORDER BY a.account_id"
        )

    def actAs(self, account_id: int) -> dict:
        """`actAs` signs this browser tab in as an account without Google; only the simulation may.

        Raises:
            RuleError: The account was deleted.

        Example:
            >>> accounts.actAs(2)["access_token"]
        """
        if self.describeAccount(account_id)["state"] != "active":
            raise RuleError(f"The account {account_id} was deleted.")
        return {"status": "signed_in", **self._startSession(account_id, self._bus.readSeconds())}

    def _startSession(self, account_id: int, now_s: float) -> dict:
        """`_startSession` signs the account out of every other computer and starts a new session.

        Example:
            >>> self._startSession(1, now_s)["refresh_token"]
        """
        self._revokeFamilies("account_id = ?", (account_id,), _SIGNED_IN_ELSEWHERE, now_s)
        self._bus.send(now_s, "backend", "database", "INSERT session_family", str(account_id))
        family_id: int = self._bus.execute(
            "INSERT INTO session_family (account_id, created_at_s) VALUES (?, ?)",
            (account_id, now_s),
        ).lastrowid
        return self._issueTokens(account_id, family_id, now_s)

    def _issueTokens(self, account_id: int, family_id: int, now_s: float) -> dict:
        """`_issueTokens` makes an access token and a new refresh token for a session family.

        Example:
            >>> self._issueTokens(1, 3, now_s)["access_expires_at_s"]
        """
        settings: dict[str, float] = self._bus.readSettings()
        access_expires_at_s: float = now_s + settings["access_token_min"] * SECONDS_PER_MINUTE
        access_token: str = jwt.encode(
            {"sub": str(account_id), "sid": family_id, "exp": int(access_expires_at_s)},
            self._access_token_key,
            algorithm=_BACKEND_TOKEN_ALGORITHM,
        )
        refresh_token: str = secrets.token_urlsafe(_REFRESH_TOKEN_BYTES)
        self._bus.execute(
            "INSERT INTO refresh_token (token_hash, family_id, issued_at_s, expires_at_s)"
            " VALUES (?, ?, ?, ?)",
            (
                hash_token(refresh_token),
                family_id,
                now_s,
                now_s + settings["refresh_token_days"] * SECONDS_PER_DAY,
            ),
        )
        return {
            "access_token": access_token,
            "access_expires_at_s": access_expires_at_s,
            "refresh_token": refresh_token,
            "account": self.describeAccount(account_id),
        }

    def _revokeFamilies(self, where_sql: str, args: tuple, because: str, now_s: float) -> None:
        """`_revokeFamilies` signs out the session families that match `where_sql` and are still on.

        Example:
            >>> self._revokeFamilies("account_id = ?", (1,), _SIGNED_OUT, now_s)
        """
        self._bus.execute(
            f"UPDATE session_family SET revoked_at_s = ?, revoked_because = ?"
            f" WHERE revoked_at_s IS NULL AND {where_sql}",
            (now_s, because, *args),
        )

    def _verifyIdToken(self, id_token: str, nonce: str, now_s: float) -> dict:
        """`_verifyIdToken` checks an ID token's signature, issuer, audience, expiry and nonce.

        Raises:
            AuthError: Any check fails.

        Example:
            >>> self._verifyIdToken(token_response["id_token"], nonce, now_s)["sub"]
        """
        try:
            key_id: str = jwt.get_unverified_header(id_token)["kid"]
            if key_id not in self._google_keys:
                self._google_keys = {key["kid"]: key for key in self._google.readJwks()["keys"]}
            claims: dict = jwt.decode(
                id_token,
                jwt.PyJWK(self._google_keys[key_id]).key,
                algorithms=["RS256"],
                audience=GOOGLE_CLIENT_ID,
                issuer=FAKE_GOOGLE_ISSUER,
                options={"verify_exp": False, "verify_iat": False},
            )
        except (jwt.InvalidTokenError, KeyError) as error:
            raise AuthError(f"Google's ID token is invalid: {error}") from error
        if now_s > claims["exp"]:
            raise AuthError("Google's ID token expired.")
        if claims["nonce"] != nonce:
            raise AuthError("Google's ID token was made for another sign-in (nonce mismatch).")
        if claims["email_verified"] is not True:
            raise AuthError("Google hasn't verified this account's email.")
        return claims

    def _decodeBackendToken(self, token: str, key: str, now_s: float) -> dict:
        """`_decodeBackendToken` checks a token the backend signed itself, against the sim clock.

        Raises:
            AuthError: The token is malformed, signed with another key, or expired.

        Example:
            >>> self._decodeBackendToken(access_token, self._access_token_key, now_s)["sid"]
        """
        try:
            claims: dict = jwt.decode(
                token,
                key,
                algorithms=[_BACKEND_TOKEN_ALGORITHM],
                options={"verify_exp": False, "verify_iat": False},
            )
        except jwt.InvalidTokenError as error:
            raise AuthError(f"The token is invalid: {error}") from error
        if now_s > claims["exp"]:
            raise AuthError("The token expired.")
        return claims

    def _checkNameIsFree(self, display_name: str) -> None:
        """`_checkNameIsFree` refuses a name that breaks a naming rule or that another account has.

        Raises:
            RuleError: The name breaks a rule or is taken, in any mix of upper and lower case.

        Example:
            >>> self._checkNameIsFree("perihelion")
        """
        check_display_name(display_name)
        if len(self._bus.read("SELECT 1 FROM account WHERE display_name = ?", (display_name,))) > 0:
            raise RuleError(f"Another player is already called {display_name}.")

    def _suggestName(self, full_name: str) -> str:
        """`_suggestName` proposes a free display name from the Google account's name, or "".

        Example:
            >>> self._suggestName("Ada Lovelace")
            'AdaLovelace'
        """
        suggestion: str = _NOT_NAME_CHARACTER.sub("", full_name)[:MAX_NAME_LENGTH]
        return suggestion if self.checkName(suggestion)["is_free"] else ""

    def _hashIdentity(self, subject: str) -> str:
        """`_hashIdentity` returns the one-way hash a deleted account's Google account is kept as.

        Example:
            >>> self._hashIdentity("100000000000000000001")
        """
        return hash_token(f"{GOOGLE_PROVIDER}:{subject}")
