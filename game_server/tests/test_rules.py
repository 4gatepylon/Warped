"""`test_rules.py` checks the square game's laser physics and room rules.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| impact parameter b | below capture (2.6 r_s) | `test_laser_falls_in_below_capture_limit` |
| | just above it | `test_laser_falls_in_below_capture_limit` |
| | far away (weak field) | `test_laser_bends_by_weak_field_angle` |
| target square | on the path | `test_laser_eliminates_target` |
| | beside the path | `test_laser_eliminates_target` |
| shooter | alive, recharged | `test_laser_eliminates_target` |
| | alive, recharging | `test_shots_wait_for_cooldown` |
| | eliminated | `test_eliminated_player_cannot_act` |
"""

import math
import uuid

from game_server import laser, room

_CENTER_M: float = 1000.0
_FAR_M: float = 300.0
_R_S_M: float = 1.0
_CAPTURE_LIMIT_PER_R_S: float = 3 * math.sqrt(3) / 2
_WEAK_FIELD_TOLERANCE_FRAC: float = 0.1
_TICK_S: float = 1 / 30


def _trace(impact_parameter_m: float) -> list[tuple[float, float]]:
    """`_trace` fires a laser from far away past a black hole at `impact_parameter_m`.

    Example:
        >>> points = _trace(3.0)
    """
    return laser.trace_path(
        (_CENTER_M - _FAR_M, _CENTER_M + impact_parameter_m),
        (1.0, 0.0),
        (_CENTER_M, _CENTER_M),
        _R_S_M,
        2 * _CENTER_M,
        4 * _FAR_M,
    )


def _room_with(*positions_m: tuple[float, float]) -> tuple[room.Room, list[uuid.UUID]]:
    """`_room_with` returns a room with one player at each position.

    Example:
        >>> game_room, (a, b) = _room_with((2.0, 2.0), (6.0, 2.0))
    """
    game_room: room.Room = room.Room(max_player_count=len(positions_m))
    player_ids: list[uuid.UUID] = [uuid.uuid4() for _ in positions_m]
    for player_id, (x_m, y_m) in zip(player_ids, positions_m, strict=True):
        game_room.addPlayer(player_id)
        game_room._players[player_id].x_m = x_m
        game_room._players[player_id].y_m = y_m
    return game_room, player_ids


def _living_numbers(game_room: room.Room) -> set[int]:
    """`_living_numbers` returns the numbers of the room's living players.

    Example:
        >>> _living_numbers(game_room)
    """
    return {player["number"] for player in game_room.toState()["players"]}


def test_laser_falls_in_below_capture_limit() -> None:
    """`test_laser_falls_in_below_capture_limit` checks capture on either side of 2.6 r_s."""
    for impact_parameter_per_r_s, should_fall_in in ((2.55, True), (2.65, False)):
        end_m: tuple[float, float] = _trace(impact_parameter_per_r_s * _R_S_M)[-1]
        distance_m: float = math.dist(end_m, (_CENTER_M, _CENTER_M))
        assert (distance_m <= _R_S_M + 1e-6) == should_fall_in
    assert 2.55 < _CAPTURE_LIMIT_PER_R_S < 2.65


def test_laser_bends_by_weak_field_angle() -> None:
    """`test_laser_bends_by_weak_field_angle` checks a far pass bends by about 2 r_s / b."""
    impact_parameter_m: float = 20 * _R_S_M
    points: list[tuple[float, float]] = _trace(impact_parameter_m)
    (x0, y0), (x1, y1) = points[-2], points[-1]
    deflection: float = math.atan2(-(y1 - y0), x1 - x0)
    expected: float = 2 * _R_S_M / impact_parameter_m
    assert abs(deflection - expected) <= _WEAK_FIELD_TOLERANCE_FRAC * expected


def test_laser_eliminates_target() -> None:
    """`test_laser_eliminates_target` checks a laser takes out the square it touches only."""
    game_room, (shooter, target, bystander) = _room_with((2.0, 2.0), (6.0, 2.0), (2.0, 6.0))
    game_room.shoot(shooter, 6.0, 2.0)
    for _ in range(round(1 / _TICK_S)):
        game_room.step(_TICK_S)
    assert _living_numbers(game_room) == {1, 3}


def test_shots_wait_for_cooldown() -> None:
    """`test_shots_wait_for_cooldown` checks a second shot within 2 s is ignored."""
    game_room, (shooter, _) = _room_with((2.0, 2.0), (2.0, 20.0))
    game_room.shoot(shooter, 20.0, 2.0)
    game_room.step(_TICK_S)
    game_room.shoot(shooter, 2.0, 20.0)
    assert len(game_room.toState()["lasers"]) == 1


def test_eliminated_player_cannot_act() -> None:
    """`test_eliminated_player_cannot_act` checks an eliminated player neither moves nor shoots."""
    game_room, (shooter, target) = _room_with((2.0, 2.0), (6.0, 2.0))
    game_room.shoot(shooter, 6.0, 2.0)
    for _ in range(round(1 / _TICK_S)):
        game_room.step(_TICK_S)
    game_room.setDirection(target, 1.0, 0.0)
    game_room.step(room.SHOT_COOLDOWN_S)
    game_room.shoot(target, 2.0, 2.0)
    game_room.step(_TICK_S)
    assert game_room._players[target].x_m == 6.0
    assert game_room.toState()["lasers"] == []
