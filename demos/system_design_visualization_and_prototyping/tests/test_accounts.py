"""`test_accounts.py` checks sign-in with the fake Google, sign-up, sessions and settings.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| authorize request | right | `test_new_google_account_must_sign_up` |
| | wrong redirect, client, method or missing state | `test_bad_authorize_requests_are_refused` |
| code redemption | right, once | `test_new_google_account_must_sign_up` |
| | used twice, expired, wrong verifier, wrong redirect | `test_bad_codes_are_refused` |
| ID token | right | `test_new_google_account_must_sign_up` |
| | other nonce, signed by another key | `test_bad_id_tokens_are_refused` |
| Google account | new | `test_new_google_account_must_sign_up` |
| | known | `test_known_google_account_signs_in_at_once` |
| sign-up form | right | `test_known_google_account_signs_in_at_once` |
| | each rule broken | `test_sign_up_rules_are_enforced` |
| | expired, or a second account | `test_sign_up_token_is_short_lived_and_single` |
| access token | fresh, expired | `test_access_tokens_expire_and_refresh` |
| refresh token | used once, twice, expired | `test_refresh_tokens_rotate_and_reuse_signs_out` |
| sessions | a second sign-in | `test_second_sign_in_signs_the_first_out` |
| | sign out here, everywhere | `test_sign_out_here_or_everywhere` |
| name change | too soon, after 30 days, taken, case only | `test_name_changes_wait_30_days` |
| icon change | shipped icon, other | `test_icons_come_from_the_shipped_set` |
| deletion | signs out, frees the name | `test_deleting_an_account_keeps_a_placeholder` |
| | signing up again keeps the used allowance | `test_deleting_an_account_keeps_a_placeholder` |
"""

import secrets
from datetime import date

import jwt
import pytest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from sign_in_helpers import (
    ADA,
    ADULT_BIRTH_DATE,
    GRACE,
    REDIRECT_URI,
    authorize_params,
    make_pkce,
    make_world,
    read_code,
    sign_in,
    sign_up,
)

from system_prototype.accounts import PLACEHOLDER_NAME
from system_prototype.identity_provider import FAKE_GOOGLE_ISSUER, GOOGLE_CLIENT_ID
from system_prototype.rules import AuthError, RuleError

SECONDS_PER_DAY: int = 86400
SECONDS_PER_MINUTE: int = 60


def test_new_google_account_must_sign_up() -> None:
    """`test_new_google_account_must_sign_up` checks the first sign-in of a Google account."""
    world, _ = make_world()
    answer: dict = sign_in(world, ADA)
    assert answer["status"] == "needs_sign_up"
    assert answer["email"] == "ada.lovelace@example.com"
    assert answer["suggested_name"] == "AdaLovelace"
    assert "access_token" not in answer


def test_bad_authorize_requests_are_refused() -> None:
    """`test_bad_authorize_requests_are_refused` checks what Google refuses on its own page."""
    world, _ = make_world()
    _, challenge = make_pkce()
    bad_changes: list[tuple[dict, str]] = [
        ({"redirect_uri": "http://evil.test/game/"}, "redirect_uri_mismatch"),
        ({"redirect_uri": "http://127.0.0.1:8010/site/"}, "redirect_uri_mismatch"),
        ({"client_id": "someone-else"}, "invalid_client"),
        ({"code_challenge_method": "plain"}, "S256"),
        ({"state": ""}, "state is missing"),
    ]
    for changes, reason in bad_changes:
        with pytest.raises(RuleError, match=reason):
            world.google.authorize(authorize_params(challenge, "n-1", **changes), ADA)
    world.google.authorize(
        authorize_params(challenge, "n-1", redirect_uri="http://localhost:51234/game/"), ADA
    )


def test_bad_codes_are_refused() -> None:
    """`test_bad_codes_are_refused` checks each reason Google's token endpoint refuses a code."""
    world, _ = make_world()
    verifier, challenge = make_pkce()

    def new_code() -> str:
        """`new_code` returns a fresh code for Ada."""
        return read_code(world.google.authorize(authorize_params(challenge, "n-1"), ADA))

    used_code: str = new_code()
    world.accounts.signInWithGoogle(used_code, verifier, "n-1", REDIRECT_URI)
    with pytest.raises(AuthError, match="already used"):
        world.accounts.signInWithGoogle(used_code, verifier, "n-1", REDIRECT_URI)
    with pytest.raises(AuthError, match="verifier"):
        world.accounts.signInWithGoogle(new_code(), "wrong-verifier", "n-1", REDIRECT_URI)
    with pytest.raises(AuthError, match="redirect"):
        world.accounts.signInWithGoogle(new_code(), verifier, "n-1", "http://127.0.0.1:9000/game/")
    stale_code: str = new_code()
    world.skipClock(601.0)
    with pytest.raises(AuthError, match="expired"):
        world.accounts.signInWithGoogle(stale_code, verifier, "n-1", REDIRECT_URI)


def test_bad_id_tokens_are_refused(monkeypatch: pytest.MonkeyPatch) -> None:
    """`test_bad_id_tokens_are_refused` checks a token for another sign-in and a forged one."""
    world, _ = make_world()
    verifier, challenge = make_pkce()
    code: str = read_code(world.google.authorize(authorize_params(challenge, "n-1"), ADA))
    with pytest.raises(AuthError, match="nonce"):
        world.accounts.signInWithGoogle(code, verifier, "n-other", REDIRECT_URI)
    forger_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    real_key_id: str = world.google.readJwks()["keys"][0]["kid"]
    forged_token: str = jwt.encode(
        {
            "iss": FAKE_GOOGLE_ISSUER,
            "aud": GOOGLE_CLIENT_ID,
            "sub": ADA,
            "nonce": "n-1",
            "email": "ada.lovelace@example.com",
            "email_verified": True,
            "name": "Ada",
            "exp": 9e9,
        },
        forger_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.PKCS8,
            serialization.NoEncryption(),
        ),
        algorithm="RS256",
        headers={"kid": real_key_id},
    )
    monkeypatch.setattr(world.google, "exchangeCode", lambda *_: {"id_token": forged_token})
    with pytest.raises(AuthError, match="invalid"):
        world.accounts.signInWithGoogle("any", verifier, "n-1", REDIRECT_URI)


def test_known_google_account_signs_in_at_once() -> None:
    """`test_known_google_account_signs_in_at_once` checks a sign-up, then a second sign-in."""
    world, _ = make_world()
    tokens: dict = sign_up(world, ADA)
    assert tokens["account"]["display_name"] == "perihelion"
    assert tokens["account"]["online_access"]["reason"] == "free_allowance"
    again: dict = sign_in(world, ADA)
    assert again["status"] == "signed_in"
    assert again["account"]["account_id"] == tokens["account"]["account_id"]


def test_sign_up_rules_are_enforced() -> None:
    """`test_sign_up_rules_are_enforced` checks age, country, terms, icon and a taken name."""
    world, _ = make_world()
    sign_up(world, GRACE, display_name="Zenith")
    token: str = sign_in(world, ADA)["sign_up_token"]
    turns_13_tomorrow: date = date(2013, 11, 11)
    bad_forms: list[tuple[tuple, str]] = [
        ((turns_13_tomorrow, "perihelion", "blue", True, "US"), "13 or older"),
        ((ADULT_BIRTH_DATE, "perihelion", "blue", True, "FR"), "European Union"),
        ((ADULT_BIRTH_DATE, "perihelion", "blue", False, "US"), "accept the terms"),
        ((ADULT_BIRTH_DATE, "perihelion", "uploaded.png", True, "US"), "icons"),
        ((ADULT_BIRTH_DATE, "zenith", "blue", True, "US"), "already called"),
        ((ADULT_BIRTH_DATE, "staff_ada", "blue", True, "US"), "imitates the staff"),
    ]
    for form, reason in bad_forms:
        with pytest.raises(RuleError, match=reason):
            world.accounts.signUp(token, *form)
    assert len(world.accounts.listAccounts()) == 1
    world.accounts.signUp(token, date(2013, 11, 10), "perihelion", "blue", True, "US")


def test_sign_up_token_is_short_lived_and_single() -> None:
    """`test_sign_up_token_is_short_lived_and_single` checks 10 minutes and one account."""
    world, _ = make_world()
    token: str = sign_in(world, ADA)["sign_up_token"]
    world.accounts.signUp(token, ADULT_BIRTH_DATE, "perihelion", "blue", True, "US")
    with pytest.raises(RuleError, match="already has"):
        world.accounts.signUp(token, ADULT_BIRTH_DATE, "aphelion", "blue", True, "US")
    late_token: str = sign_in(world, GRACE)["sign_up_token"]
    world.skipClock(601.0)
    with pytest.raises(AuthError, match="expired"):
        world.accounts.signUp(late_token, ADULT_BIRTH_DATE, "zenith", "blue", True, "US")


def test_access_tokens_expire_and_refresh() -> None:
    """`test_access_tokens_expire_and_refresh` checks 15 minutes, then a refresh."""
    world, _ = make_world()
    tokens: dict = sign_up(world, ADA)
    assert world.accounts.authenticate(tokens["access_token"]).account_id == 1
    world.skipClock(15 * SECONDS_PER_MINUTE + 1.0)
    with pytest.raises(AuthError, match="expired"):
        world.accounts.authenticate(tokens["access_token"])
    fresh: dict = world.accounts.refreshSession(tokens["refresh_token"])
    assert world.accounts.authenticate(fresh["access_token"]).account_id == 1


def test_refresh_tokens_rotate_and_reuse_signs_out() -> None:
    """`test_refresh_tokens_rotate_and_reuse_signs_out` checks rotation, reuse and expiry."""
    world, _ = make_world()
    first: dict = sign_up(world, ADA)
    second: dict = world.accounts.refreshSession(first["refresh_token"])
    assert second["refresh_token"] != first["refresh_token"]
    with pytest.raises(AuthError, match="used again"):
        world.accounts.refreshSession(first["refresh_token"])
    with pytest.raises(AuthError, match="used again"):
        world.accounts.authenticate(second["access_token"])
    with pytest.raises(AuthError, match="used again"):
        world.accounts.refreshSession(second["refresh_token"])
    with pytest.raises(AuthError, match="unknown"):
        world.accounts.refreshSession(secrets.token_urlsafe(32))
    unused: dict = sign_in(world, ADA)
    world.skipClock(61.0 * SECONDS_PER_DAY)
    with pytest.raises(AuthError, match="expired"):
        world.accounts.refreshSession(unused["refresh_token"])


def test_second_sign_in_signs_the_first_out() -> None:
    """`test_second_sign_in_signs_the_first_out` checks one session per account."""
    world, _ = make_world()
    first_computer: dict = sign_up(world, ADA)
    second_computer: dict = sign_in(world, ADA)
    with pytest.raises(AuthError, match="another computer"):
        world.accounts.authenticate(first_computer["access_token"])
    with pytest.raises(AuthError, match="another computer"):
        world.accounts.refreshSession(first_computer["refresh_token"])
    assert world.accounts.authenticate(second_computer["access_token"]).account_id == 1


def test_sign_out_here_or_everywhere() -> None:
    """`test_sign_out_here_or_everywhere` checks both kinds of sign-out."""
    world, _ = make_world()
    tokens: dict = sign_up(world, ADA)
    world.accounts.signOut(world.accounts.authenticate(tokens["access_token"]), False)
    with pytest.raises(AuthError, match="You signed out."):
        world.accounts.authenticate(tokens["access_token"])
    again: dict = sign_in(world, ADA)
    world.accounts.signOut(world.accounts.authenticate(again["access_token"]), True)
    with pytest.raises(AuthError, match="every computer"):
        world.accounts.refreshSession(again["refresh_token"])


def test_name_changes_wait_30_days() -> None:
    """`test_name_changes_wait_30_days` checks the wait, a taken name and a change of case."""
    world, _ = make_world()
    sign_up(world, GRACE, display_name="zenith")
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    with pytest.raises(RuleError, match="10 December 2026"):
        world.accounts.changeName(account_id, "aphelion")
    world.skipClock(30.0 * SECONDS_PER_DAY)
    with pytest.raises(RuleError, match="already called"):
        world.accounts.changeName(account_id, "Zenith")
    world.accounts.changeName(account_id, "Perihelion")
    assert world.accounts.readDisplayName(account_id) == "Perihelion"
    with pytest.raises(RuleError, match="change your name again"):
        world.accounts.changeName(account_id, "aphelion")


def test_icons_come_from_the_shipped_set() -> None:
    """`test_icons_come_from_the_shipped_set` checks a shipped icon and an uploaded picture."""
    world, _ = make_world()
    account_id: int = sign_up(world, ADA)["account"]["account_id"]
    world.accounts.changeIcon(account_id, "ring")
    assert world.accounts.describeAccount(account_id)["icon"] == "ring"
    with pytest.raises(RuleError, match="icons"):
        world.accounts.changeIcon(account_id, "me.png")


def test_deleting_an_account_keeps_a_placeholder() -> None:
    """`test_deleting_an_account_keeps_a_placeholder` checks what deletion removes and keeps."""
    world, _ = make_world()
    tokens: dict = sign_up(world, ADA)
    account_id: int = tokens["account"]["account_id"]
    world.accounts.recordPlay(account_id, finished_match_count=1, played_s=600.0)
    world.accounts.deleteAccount(account_id)
    with pytest.raises(AuthError, match="deleted"):
        world.accounts.authenticate(tokens["access_token"])
    assert world.accounts.readDisplayName(account_id) == PLACEHOLDER_NAME
    assert world.accounts.checkName("perihelion")["is_free"]
    assert sign_in(world, ADA)["status"] == "needs_sign_up"
    reborn: dict = sign_up(world, ADA, display_name="aphelion")
    assert reborn["account"]["account_id"] != account_id
    assert reborn["account"]["online_access"]["can_play_online"] is False
