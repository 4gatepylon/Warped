"""`account_rules.py` holds the rules of accounts, sign-in and paying that need no stored state.

Interface:
    ICONS: The icons a player may pick, which the game ships with.
    MIN_AGE_YEARS: The youngest a player may be.
    OnlineAccess: Whether an account may play online, and why.
    check_display_name: Refuses a display name that breaks the naming rules.
    read_sim_date: The calendar date of a simulated time.
    is_old_enough: Whether a birth date is at least 13 years before a date.
    is_country_allowed: Whether sign-up accepts a connection from a country.
    read_name_change_wait_s: How long until an account may change its name again.
    describe_online_access: Whether an account has bought online play or still has free allowance.
    make_code_challenge: The PKCE code challenge of a code verifier.
    hash_token: The hash kept in place of a code or token.
    sign_webhook: The `Stripe-Signature` header for a webhook's body.
    check_webhook_signature: Refuses a webhook whose signature is wrong or too old.

Example:
    >>> check_display_name("perihelion")
    >>> is_old_enough(date(2013, 11, 10), date(2026, 11, 10))
    True
"""

import base64
import hashlib
import hmac
import re
from dataclasses import dataclass
from datetime import date, timedelta

from .rules import AuthError, RuleError

ICONS: tuple[str, ...] = ("violet", "blue", "red", "amber", "green", "white", "ring", "diamond")
MIN_AGE_YEARS: int = 13
SIM_FIRST_DAY: date = date(2026, 11, 10)
SECONDS_PER_DAY: int = 86400
SECONDS_PER_MINUTE: int = 60
MIN_NAME_LENGTH: int = 3
MAX_NAME_LENGTH: int = 16
_NAME_PATTERN: re.Pattern = re.compile(rf"[A-Za-z0-9_]{{{MIN_NAME_LENGTH},{MAX_NAME_LENGTH}}}")
_STAFF_NAME_PARTS: tuple[str, ...] = (
    "admin",
    "warped",
    "staff",
    "moderator",
    "official",
    "support",
    "developer",
)
_OFFENSIVE_NAME_PARTS: tuple[str, ...] = ("idiot", "loser", "noob")
_EU_COUNTRY_CODES: frozenset[str] = frozenset(
    "AT BE BG HR CY CZ DK EE FI FR DE GR HU IE IT LV LT LU MT NL PL PT RO SK SI ES SE".split()
)
WEBHOOK_TOLERANCE_S_ACCORDING_TO_STRIPE_DOCS: float = 300.0


@dataclass(frozen=True)
class OnlineAccess:
    """`OnlineAccess` says whether an account may play online, and how much free allowance is left.

    Args:
        can_play_online: Whether it may join the queue or a custom game now.
        reason: "purchased", "free_allowance" or "allowance_used".
        free_matches_left: Finished matches left in the free allowance.
        free_s_left: Seconds of matches left in the free allowance.

    Example:
        >>> OnlineAccess(True, "free_allowance", 1, 1800.0)
    """

    can_play_online: bool
    reason: str
    free_matches_left: int
    free_s_left: float


def check_display_name(name: str) -> None:
    """`check_display_name` refuses a display name that breaks the naming rules.

    A name is 3 to 16 letters, digits or underscores, doesn't imitate staff, and isn't offensive.
    Whether another account has it is the database's question, not this one's.

    Args:
        name: The name the player asked for.

    Raises:
        RuleError: The name breaks a rule; the message says which.

    Example:
        >>> check_display_name("admin_7")
        Traceback (most recent call last):
        RuleError: A name can't contain "admin", so that nobody imitates the staff.
    """
    if _NAME_PATTERN.fullmatch(name) is None:
        raise RuleError(
            f"A name is {MIN_NAME_LENGTH} to {MAX_NAME_LENGTH} letters, digits or underscores."
        )
    lowered_name: str = name.lower()
    for part in _STAFF_NAME_PARTS:
        if part in lowered_name:
            raise RuleError(f'A name can\'t contain "{part}", so that nobody imitates the staff.')
    for part in _OFFENSIVE_NAME_PARTS:
        if part in lowered_name:
            raise RuleError("That name isn't allowed.")


def read_sim_date(sim_s: float) -> date:
    """`read_sim_date` returns the calendar date of a simulated time; day 1 is 10 November 2026.

    Example:
        >>> read_sim_date(86400.0)
        datetime.date(2026, 11, 11)
    """
    return SIM_FIRST_DAY + timedelta(days=int(sim_s // SECONDS_PER_DAY))


def is_old_enough(birth_date: date, today: date) -> bool:
    """`is_old_enough` says whether someone born on `birth_date` is at least 13 on `today`.

    Someone born on 29 February turns a year older on 1 March in years without one.

    Example:
        >>> is_old_enough(date(2013, 11, 11), date(2026, 11, 10))
        False
    """
    has_had_birthday: bool = (today.month, today.day) >= (birth_date.month, birth_date.day)
    age_years: int = today.year - birth_date.year - (0 if has_had_birthday else 1)
    return age_years >= MIN_AGE_YEARS


def is_country_allowed(country_code: str) -> bool:
    """`is_country_allowed` says whether sign-up accepts a connection from a country.

    Args:
        country_code: The connection's ISO 3166-1 alpha-2 code, such as "US".

    Example:
        >>> is_country_allowed("FR")
        False
    """
    return country_code.upper() not in _EU_COUNTRY_CODES


def read_name_change_wait_s(
    name_changed_at_s: float, now_s: float, name_change_days: float
) -> float:
    """`read_name_change_wait_s` returns how many seconds are left before a name may change again.

    Args:
        name_changed_at_s: When the account last picked its name (sign-up counts).
        now_s: Now.
        name_change_days: The `name_change_days` setting.

    Returns:
        0 if the name may change now, else the seconds to wait.

    Example:
        >>> read_name_change_wait_s(0.0, 86400.0, 30)
        2505600.0
    """
    return max(0.0, name_changed_at_s + name_change_days * SECONDS_PER_DAY - now_s)


def describe_online_access(
    has_purchase: bool, finished_match_count: int, played_s: float, settings: dict[str, float]
) -> OnlineAccess:
    """`describe_online_access` says whether an account may play online.

    The free allowance ends at whichever comes first: `free_match_count` finished matches or
    `free_match_min` minutes in matches.

    Args:
        has_purchase: Whether the account has a payment that wasn't refunded or disputed.
        finished_match_count: Online matches it has finished.
        played_s: Seconds it has spent in online matches.
        settings: The settings, with `free_match_count` and `free_match_min`.

    Example:
        >>> describe_online_access(False, 1, 60.0, settings).can_play_online
        False
    """
    free_matches_left: int = max(0, int(settings["free_match_count"]) - finished_match_count)
    free_s_left: float = max(0.0, settings["free_match_min"] * SECONDS_PER_MINUTE - played_s)
    if has_purchase:
        return OnlineAccess(True, "purchased", free_matches_left, free_s_left)
    if free_matches_left > 0 and free_s_left > 0:
        return OnlineAccess(True, "free_allowance", free_matches_left, free_s_left)
    return OnlineAccess(False, "allowance_used", free_matches_left, free_s_left)


def make_code_challenge(code_verifier: str) -> str:
    """`make_code_challenge` returns the S256 PKCE challenge of a verifier (RFC 7636, section 4.2).

    Example:
        >>> make_code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
        'E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM'
    """
    digest: bytes = hashlib.sha256(code_verifier.encode("ascii")).digest()
    return base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")


def hash_token(token: str) -> str:
    """`hash_token` returns the SHA-256 hex digest kept in the database in place of a secret token.

    Example:
        >>> len(hash_token("abc"))
        64
    """
    return hashlib.sha256(token.encode("utf-8")).hexdigest()


def sign_webhook(secret: str, at_s: float, payload: bytes) -> str:
    """`sign_webhook` returns the `Stripe-Signature` header Stripe would send with `payload`.

    The signature is HMAC-SHA256, keyed by the endpoint's secret, of "<timestamp>.<body>".

    Example:
        >>> sign_webhook("whsec_x", 12.0, b"{}")
        't=12,v1=...'
    """
    timestamp: str = str(int(at_s))
    signed: bytes = timestamp.encode("ascii") + b"." + payload
    signature: str = hmac.new(secret.encode("utf-8"), signed, hashlib.sha256).hexdigest()
    return f"t={timestamp},v1={signature}"


def check_webhook_signature(header: str, payload: bytes, secret: str, now_s: float) -> None:
    """`check_webhook_signature` refuses a webhook that Stripe didn't sign, or signed too long ago.

    Args:
        header: The `Stripe-Signature` header, "t=<timestamp>,v1=<signature>[,v1=...]".
        payload: The request's body, exactly as received.
        secret: The endpoint's signing secret.
        now_s: Now, to refuse a replay older than 5 minutes.

    Raises:
        AuthError: The header is malformed, no signature matches, or the timestamp is too old.

    Example:
        >>> check_webhook_signature(sign_webhook("whsec_x", 0.0, b"{}"), b"{}", "whsec_x", 1.0)
    """
    parts: list[tuple[str, str]] = [
        tuple(item.split("=", 1)) for item in header.split(",") if "=" in item
    ]
    timestamps: list[str] = [value for key, value in parts if key == "t"]
    signatures: list[str] = [value for key, value in parts if key == "v1"]
    if len(timestamps) != 1 or not timestamps[0].isdigit() or len(signatures) == 0:
        raise AuthError("The Stripe-Signature header is malformed.")
    if abs(now_s - int(timestamps[0])) > WEBHOOK_TOLERANCE_S_ACCORDING_TO_STRIPE_DOCS:
        raise AuthError("The webhook's timestamp is too far from now.")
    expected: str = sign_webhook(secret, int(timestamps[0]), payload).split("v1=", 1)[1]
    if not any(hmac.compare_digest(expected, signature) for signature in signatures):
        raise AuthError("The webhook's signature doesn't match.")
