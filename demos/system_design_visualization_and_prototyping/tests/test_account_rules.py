"""`test_account_rules.py` checks the stateless rules of `system_prototype.account_rules`.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| name length | under 3, 3, 16, over 16 | `test_name_length_is_3_to_16` |
| name characters | letters, digits, underscore | `test_name_length_is_3_to_16` |
| | anything else (space, hyphen, accent) | `test_name_characters_are_limited` |
| name meaning | imitates staff, in any case | `test_names_that_imitate_staff_are_refused` |
| | offensive | `test_offensive_names_are_refused` |
| age on the day | 13 today, 13 tomorrow | `test_age_turns_13_on_the_birthday` |
| | born on 29 February | `test_leap_day_birthdays_count_from_1_march` |
| connection country | outside the EU, inside (any case) | `test_eu_connections_are_refused` |
| sim time | start, end of day 1, day 2 | `test_sim_dates_start_on_10_november_2026` |
| name change time | just changed, after the wait, at its end | `test_name_change_waits_30_days` |
| online access | purchased with the allowance used | `test_purchase_unlocks_online_play` |
| | unpaid with allowance left | `test_free_allowance_ends_at_whichever_comes_first` |
| | unpaid, matches used / time used | `test_free_allowance_ends_at_whichever_comes_first` |
| PKCE | RFC 7636's example | `test_code_challenge_matches_rfc_7636` |
| webhook signature | right | `test_right_webhook_signatures_pass` |
| | one of several `v1`s right | `test_right_webhook_signatures_pass` |
| | wrong secret, changed body, too old, malformed | `test_wrong_webhook_signatures_are_refused` |
"""

from datetime import date

import pytest

from system_prototype.account_rules import (
    check_display_name,
    check_webhook_signature,
    describe_online_access,
    is_country_allowed,
    is_old_enough,
    make_code_challenge,
    read_name_change_wait_s,
    read_sim_date,
    sign_webhook,
)
from system_prototype.rules import AuthError, RuleError
from system_prototype.store import DEFAULT_SETTINGS

SETTINGS: dict[str, float] = {name: value for name, _, _, value in DEFAULT_SETTINGS}
SECONDS_PER_DAY: int = 86400


def test_name_length_is_3_to_16() -> None:
    """`test_name_length_is_3_to_16` checks both ends of the length rule."""
    check_display_name("a_1")
    check_display_name("Perihelion_12345")
    for bad_name in ("ab", "Perihelion_123456"):
        with pytest.raises(RuleError, match="3 to 16"):
            check_display_name(bad_name)


def test_name_characters_are_limited() -> None:
    """`test_name_characters_are_limited` checks that only letters, digits and `_` are allowed."""
    for bad_name in ("peri helion", "peri-helion", "périhélion"):
        with pytest.raises(RuleError, match="letters, digits or underscores"):
            check_display_name(bad_name)


def test_names_that_imitate_staff_are_refused() -> None:
    """`test_names_that_imitate_staff_are_refused` checks a staff word anywhere, in any case."""
    for bad_name in ("SuperAdmin", "WARPED_team", "the_moderator"):
        with pytest.raises(RuleError, match="imitates the staff"):
            check_display_name(bad_name)


def test_offensive_names_are_refused() -> None:
    """`test_offensive_names_are_refused` checks the placeholder word list."""
    with pytest.raises(RuleError, match="isn't allowed"):
        check_display_name("big_Loser")


def test_age_turns_13_on_the_birthday() -> None:
    """`test_age_turns_13_on_the_birthday` checks the eve and the day of the 13th birthday."""
    assert is_old_enough(date(2013, 11, 10), date(2026, 11, 10))
    assert not is_old_enough(date(2013, 11, 11), date(2026, 11, 10))
    assert not is_old_enough(date(2020, 1, 1), date(2026, 11, 10))


def test_leap_day_birthdays_count_from_1_march() -> None:
    """`test_leap_day_birthdays_count_from_1_march` checks someone born on 29 February 2012."""
    assert not is_old_enough(date(2012, 2, 29), date(2025, 2, 28))
    assert is_old_enough(date(2012, 2, 29), date(2025, 3, 1))


def test_eu_connections_are_refused() -> None:
    """`test_eu_connections_are_refused` checks a country outside the EU and two inside."""
    assert is_country_allowed("US")
    assert is_country_allowed("GB")
    assert not is_country_allowed("FR")
    assert not is_country_allowed("de")


def test_sim_dates_start_on_10_november_2026() -> None:
    """`test_sim_dates_start_on_10_november_2026` checks day 1 and the turn to day 2."""
    assert read_sim_date(0.0) == date(2026, 11, 10)
    assert read_sim_date(SECONDS_PER_DAY - 1.0) == date(2026, 11, 10)
    assert read_sim_date(float(SECONDS_PER_DAY)) == date(2026, 11, 11)


def test_name_change_waits_30_days() -> None:
    """`test_name_change_waits_30_days` checks the wait after a change, at its end and later."""
    assert read_name_change_wait_s(0.0, 0.0, 30) == 30 * SECONDS_PER_DAY
    assert read_name_change_wait_s(0.0, 30.0 * SECONDS_PER_DAY, 30) == 0.0
    assert read_name_change_wait_s(0.0, 31.0 * SECONDS_PER_DAY, 30) == 0.0


def test_purchase_unlocks_online_play() -> None:
    """`test_purchase_unlocks_online_play` checks a purchase after the allowance is used."""
    access = describe_online_access(True, 5, 9000.0, SETTINGS)
    assert (access.can_play_online, access.reason) == (True, "purchased")


def test_free_allowance_ends_at_whichever_comes_first() -> None:
    """`test_free_allowance_ends_at_whichever_comes_first` checks 1 match or 30 minutes."""
    fresh = describe_online_access(False, 0, 0.0, SETTINGS)
    assert (fresh.can_play_online, fresh.free_matches_left, fresh.free_s_left) == (
        True,
        1,
        1800.0,
    )
    assert describe_online_access(False, 0, 1799.0, SETTINGS).can_play_online
    assert not describe_online_access(False, 1, 60.0, SETTINGS).can_play_online
    used_time = describe_online_access(False, 0, 1800.0, SETTINGS)
    assert (used_time.can_play_online, used_time.reason) == (False, "allowance_used")


def test_code_challenge_matches_rfc_7636() -> None:
    """`test_code_challenge_matches_rfc_7636` checks the example in RFC 7636's appendix B."""
    assert (
        make_code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")
        == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    )


def test_right_webhook_signatures_pass() -> None:
    """`test_right_webhook_signatures_pass` checks one signature, and a right one among others."""
    header: str = sign_webhook("whsec_a", 100.0, b'{"id":"evt_1"}')
    check_webhook_signature(header, b'{"id":"evt_1"}', "whsec_a", 399.0)
    rolled_header: str = header + "," + sign_webhook("whsec_b", 100.0, b"{}").split(",")[1]
    check_webhook_signature(rolled_header, b'{"id":"evt_1"}', "whsec_a", 100.0)


def test_wrong_webhook_signatures_are_refused() -> None:
    """`test_wrong_webhook_signatures_are_refused` checks each way a webhook can be forged."""
    header: str = sign_webhook("whsec_a", 100.0, b'{"id":"evt_1"}')
    with pytest.raises(AuthError, match="doesn't match"):
        check_webhook_signature(header, b'{"id":"evt_1"}', "whsec_b", 100.0)
    with pytest.raises(AuthError, match="doesn't match"):
        check_webhook_signature(header, b'{"id":"evt_2"}', "whsec_a", 100.0)
    with pytest.raises(AuthError, match="too far"):
        check_webhook_signature(header, b'{"id":"evt_1"}', "whsec_a", 401.0)
    with pytest.raises(AuthError, match="malformed"):
        check_webhook_signature("v1=abc", b'{"id":"evt_1"}', "whsec_a", 100.0)
