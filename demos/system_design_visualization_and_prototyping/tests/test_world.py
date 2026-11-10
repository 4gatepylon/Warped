"""`test_world.py` checks the simulated system of `system_prototype.world` on a clock tests move.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| players online in a region | none | `test_idle_world_keeps_only_spare_machines` |
| | fewer than a match needs | `test_too_few_players_get_no_match` |
| | enough for matches | `test_players_get_game_servers` |
| slots needed | within the spare slots | `test_idle_world_keeps_only_spare_machines` |
| | more than the spare slots | `test_fleet_grows_with_demand` |
| | fewer again, machines empty | `test_empty_machines_are_turned_off` |
| caps | room | `test_players_get_game_servers` |
| | full, then room again | `test_full_caps_fill_the_wait_queue_then_admit` |
| failed thing | a running game server | `test_failed_game_server_voids_its_match` |
| | a machine with game servers | `test_failed_machine_fails_its_game_servers` |
| | one that doesn't exist | `test_failing_unknown_things_is_refused` |
| | one that already ended | `test_failing_ended_things_is_refused` |
| setting change | a known name, a value of at least 0 | `test_players_get_game_servers` |
| | an unknown name | `test_bad_setting_changes_are_refused` |
| | a negative value | `test_bad_setting_changes_are_refused` |
| clock skip | positive | `test_a_long_skip_plays_matches_out` |
| | not positive | `test_skipping_backwards_is_refused` |
"""

from pathlib import Path

import pytest

from system_prototype.clock import SimClock
from system_prototype.rules import NotFoundError, RuleError
from system_prototype.store import open_store
from system_prototype.world import World

BOOT_AND_START_S: int = 60


class _Hands:
    """`_Hands` is the real time that a test's clock reads, moved by the test.

    Attributes:
        real_s: The time the clock reads.

    Example:
        >>> hands = _Hands(); hands.real_s += 1.0
    """

    real_s: float = 0.0


def _make_world(**settings: float) -> tuple[World, _Hands]:
    """`_make_world` returns a new world with `settings` changed, and the hands of its clock.

    Example:
        >>> world, hands = _make_world(online_player_target=40)
    """
    hands: _Hands = _Hands()
    world: World = World(open_store(Path(":memory:")), SimClock(read_real_s=lambda: hands.real_s))
    for name, value in settings.items():
        world.changeSetting(name, value)
    return world, hands


def _run(world: World, hands: _Hands, seconds: int) -> None:
    """`_run` lets `seconds` pass, ticking the world once a second.

    Example:
        >>> _run(world, hands, 60)
    """
    for _ in range(seconds):
        hands.real_s += 1.0
        world.tick()


def _count_servers(world: World, state: str) -> int:
    """`_count_servers` returns how many game servers on live machines are in `state`.

    Example:
        >>> _count_servers(world, "running")
    """
    return sum(
        server["state"] == state
        for machine in world.describeSystem()["machines"]
        for server in machine["game_servers"]
    )


def test_idle_world_keeps_only_spare_machines() -> None:
    """`test_idle_world_keeps_only_spare_machines` checks that no players means one machine a
    region."""
    world, hands = _make_world()
    _run(world, hands, BOOT_AND_START_S)
    machines: list[dict] = world.describeSystem()["machines"]
    assert [(m["region"], m["state"], m["game_servers"]) for m in machines] == [
        ("us-east", "ready", []),
        ("us-west", "ready", []),
    ]


def test_too_few_players_get_no_match() -> None:
    """`test_too_few_players_get_no_match` checks that nine players in a region wait for a tenth."""
    world, hands = _make_world(online_player_target=9, east_player_pct=100)
    _run(world, hands, BOOT_AND_START_S)
    system: dict = world.describeSystem()
    assert system["players"] == {"online_count": 9, "idle_count": 9, "in_match_count": 0}
    assert system["caps"]["match_count"] == 0


def test_players_get_game_servers() -> None:
    """`test_players_get_game_servers` checks that forty players end up in four running matches."""
    world, hands = _make_world(online_player_target=40, east_player_pct=100)
    _run(world, hands, BOOT_AND_START_S)
    system: dict = world.describeSystem()
    assert _count_servers(world, "running") == 4
    assert system["players"] == {"online_count": 40, "idle_count": 0, "in_match_count": 40}
    assert (system["caps"]["match_count"], system["caps"]["player_count"]) == (4, 40)


def test_fleet_grows_with_demand() -> None:
    """`test_fleet_grows_with_demand` checks that eight matches and two spares need three
    machines."""
    world, hands = _make_world(online_player_target=80, east_player_pct=100)
    _run(world, hands, 2 * BOOT_AND_START_S)
    east_machines: list[dict] = [
        m for m in world.describeSystem()["machines"] if m["region"] == "us-east"
    ]
    assert len(east_machines) == 3
    assert _count_servers(world, "running") == 8


def test_empty_machines_are_turned_off() -> None:
    """`test_empty_machines_are_turned_off` checks that the fleet shrinks back to its spares."""
    world, hands = _make_world(online_player_target=80, east_player_pct=100, pretend_match_min=1)
    _run(world, hands, 2 * BOOT_AND_START_S)
    world.changeSetting("online_player_target", 0)
    _run(world, hands, 400)
    system: dict = world.describeSystem()
    assert [m["region"] for m in system["machines"]] == ["us-east", "us-west"]
    assert system["players"]["online_count"] == 0


def test_full_caps_fill_the_wait_queue_then_admit() -> None:
    """`test_full_caps_fill_the_wait_queue_then_admit` checks that waiting matches get in, in
    order."""
    world, hands = _make_world(
        online_player_target=40, east_player_pct=100, max_match_count=2, pretend_match_min=1
    )
    _run(world, hands, BOOT_AND_START_S)
    system: dict = world.describeSystem()
    assert system["caps"]["match_count"] == 2
    assert [r["match_request_id"] for r in system["wait_queue"]] == [3, 4]
    world.changeSetting("online_player_target", 0)
    _run(world, hands, 3 * BOOT_AND_START_S)
    system = world.describeSystem()
    assert system["wait_queue"] == []
    server_table: dict = next(t for t in system["tables"] if t["name"] == "game_server")
    assert server_table["row_count"] == 4
    assert {row["state"] for row in server_table["rows"]} == {"stopped"}


def test_failed_game_server_voids_its_match() -> None:
    """`test_failed_game_server_voids_its_match` checks that a dead server frees its players."""
    world, hands = _make_world(online_player_target=10, east_player_pct=100)
    _run(world, hands, BOOT_AND_START_S)
    world.changeSetting("online_player_target", 0)
    world.failGameServer("gs-1")
    _run(world, hands, 1)
    system: dict = world.describeSystem()
    assert _count_servers(world, "running") == 0
    assert system["players"]["online_count"] == 0
    assert system["messages"][0]["what"] != "INSERT match result"


def test_failed_machine_fails_its_game_servers() -> None:
    """`test_failed_machine_fails_its_game_servers` checks that a dead machine takes its servers."""
    world, hands = _make_world(online_player_target=20, east_player_pct=100)
    _run(world, hands, BOOT_AND_START_S)
    world.changeSetting("online_player_target", 0)
    world.failMachine("m-us-east-1")
    system: dict = world.describeSystem()
    assert "m-us-east-1" not in [m["machine_id"] for m in system["machines"]]
    assert _count_servers(world, "running") == 0


def test_failing_unknown_things_is_refused() -> None:
    """`test_failing_unknown_things_is_refused` checks that unknown IDs give `NotFoundError`."""
    world, _ = _make_world()
    with pytest.raises(NotFoundError, match="gs-99"):
        world.failGameServer("gs-99")
    with pytest.raises(NotFoundError, match="m-nowhere-1"):
        world.failMachine("m-nowhere-1")


def test_failing_ended_things_is_refused() -> None:
    """`test_failing_ended_things_is_refused` checks that nothing can fail twice."""
    world, hands = _make_world(online_player_target=10, east_player_pct=100)
    _run(world, hands, BOOT_AND_START_S)
    world.failMachine("m-us-east-1")
    with pytest.raises(RuleError, match="already ended"):
        world.failMachine("m-us-east-1")
    with pytest.raises(RuleError, match="already ended"):
        world.failGameServer("gs-1")


def test_bad_setting_changes_are_refused() -> None:
    """`test_bad_setting_changes_are_refused` checks unknown names and negative values."""
    world, _ = _make_world()
    with pytest.raises(NotFoundError, match="max_matches"):
        world.changeSetting("max_matches", 5)
    with pytest.raises(RuleError, match="negative"):
        world.changeSetting("max_match_count", -1)


def test_a_long_skip_plays_matches_out() -> None:
    """`test_a_long_skip_plays_matches_out` checks that an hour's skip boots, plays and finishes."""
    world, hands = _make_world(online_player_target=10, east_player_pct=100, pretend_match_min=1)
    _run(world, hands, 1)
    world.changeSetting("online_player_target", 0)
    world.skipClock(3600.0)
    system: dict = world.describeSystem()
    assert system["players"]["online_count"] == 0
    assert system["caps"]["match_count"] == 0
    assert any(m["what"] == "INSERT match result" for m in system["messages"])


def test_skipping_backwards_is_refused() -> None:
    """`test_skipping_backwards_is_refused` checks that the clock only skips forward."""
    world, _ = _make_world()
    with pytest.raises(RuleError, match="forward"):
        world.skipClock(0.0)
