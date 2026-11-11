"""`test_arena.py` checks the stand-in game's rules in `system_prototype.arena`.

The arena has open lanes along y = 1 m and x = 14 m, which the tests use to keep walls away.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| teams at creation | valid | `test_players_start_at_their_spawn_points` |
| | a team other than 0 or 1, more than 5 on a team, a limit of 0 | `test_bad_arenas_are_refused` |
| run direction | straight, diagonal | `test_running_is_as_fast_in_every_direction` |
| run obstacle | none, a wall, the arena's edge | `test_walls_and_edges_stop_a_runner` |
| shot target | enemy in range, beyond range, behind a wall | `test_shots_stop_at_range_and_walls` |
| | teammate in the way | `test_shots_pass_through_teammates` |
| shooter | ready, cooling down, dead | `test_cooldown_and_death_stop_the_trigger` |
| shots in one tick | two at each other, two at one | `test_shots_of_one_tick_land_together` |
| death | before and after the respawn time | `test_the_dead_respawn_at_their_spawn_point` |
| end | score limit, time limit with a leader or level | `test_matches_end_at_score_or_time` |
| | stepping an ended match | `test_matches_end_at_score_or_time` |
| bot | sees an enemy, sees none, dead | `test_bots_aim_or_run` |
| seed | same seed twice | `test_a_seeded_bot_match_replays_the_same` |
| frame | rounding, shots | `test_frames_are_rounded_and_list_shots` |
"""

import math
import random

import pytest

from system_prototype.arena import (
    ARENA_WIDTH_M,
    PLAYER_RADIUS_M,
    RESPAWN_S,
    RUN_SPEED_M_PER_S,
    SHOT_COOLDOWN_S,
    SHOT_RANGE_M,
    TICK_HZ,
    Arena,
    PlayerInput,
    create_arena,
    decide_bot_input,
    describe_frame,
    describe_layout,
    step_arena,
)

OPEN_LANE_Y_M: float = 1.0
OPEN_LANE_X_M: float = 14.0
STEP_M: float = RUN_SPEED_M_PER_S / TICK_HZ
SHOOT_EAST: PlayerInput = PlayerInput(0.0, 0.0, 0.0, True)
SHOOT_WEST: PlayerInput = PlayerInput(0.0, 0.0, math.pi, True)
LONG_MATCH_S: float = 600.0


def _make_duel(*positions: tuple[int, float, float]) -> Arena:
    """`_make_duel` returns an arena with one fighter per `(team, x, y)`, placed by hand."""
    arena: Arena = create_arena([team for team, _, _ in positions], 10, LONG_MATCH_S)
    for fighter, (_, x, y) in zip(arena.fighters, positions, strict=True):
        fighter.x, fighter.y = x, y
    return arena


def test_players_start_at_their_spawn_points() -> None:
    """`test_players_start_at_their_spawn_points` checks spawns and starting aims."""
    arena: Arena = create_arena([0, 1, 0, 1], 10, 180.0)
    spawns_m: list[tuple[float, float]] = [(2.0, 4.0), (38.0, 4.0), (2.0, 8.0), (38.0, 8.0)]
    assert [(f.x, f.y) for f in arena.fighters] == spawns_m
    assert [f.aim_rad for f in arena.fighters] == [0.0, math.pi, 0.0, math.pi]
    assert arena.scores == [0, 0] and not arena.is_over


def test_bad_arenas_are_refused() -> None:
    """`test_bad_arenas_are_refused` checks each precondition of `create_arena`."""
    with pytest.raises(ValueError, match="0 or 1"):
        create_arena([0, 2], 10, 180.0)
    with pytest.raises(ValueError, match="at most 5"):
        create_arena([0] * 6, 10, 180.0)
    with pytest.raises(ValueError, match="positive"):
        create_arena([0, 1], 0, 180.0)


def test_running_is_as_fast_in_every_direction() -> None:
    """`test_running_is_as_fast_in_every_direction` checks that a diagonal is no faster."""
    arena: Arena = _make_duel((0, OPEN_LANE_X_M, 12.0), (1, OPEN_LANE_X_M, 18.0))
    step_arena(arena, {0: PlayerInput(1.0, 0.0, 0.0, False), 1: PlayerInput(1.0, 1.0, 0.0, False)})
    assert arena.fighters[0].x == pytest.approx(OPEN_LANE_X_M + STEP_M)
    moved_m: float = math.hypot(arena.fighters[1].x - OPEN_LANE_X_M, arena.fighters[1].y - 18.0)
    assert moved_m == pytest.approx(STEP_M)


def test_walls_and_edges_stop_a_runner() -> None:
    """`test_walls_and_edges_stop_a_runner` checks stopping and sliding."""
    arena: Arena = _make_duel((0, 17.4, 12.0), (1, PLAYER_RADIUS_M + 0.1, OPEN_LANE_Y_M))
    step_arena(arena, {0: PlayerInput(1.0, 1.0, 0.0, False), 1: PlayerInput(-1.0, 0.0, 0.0, False)})
    assert arena.fighters[0].x == 17.4
    assert arena.fighters[0].y == pytest.approx(12.0 + STEP_M / math.sqrt(2))
    assert arena.fighters[1].x == PLAYER_RADIUS_M


def test_shots_stop_at_range_and_walls() -> None:
    """`test_shots_stop_at_range_and_walls` checks a hit, a miss and a wall."""
    in_range: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 20.0, OPEN_LANE_Y_M))
    step_arena(in_range, {0: SHOOT_EAST})
    assert in_range.shots[0].hit_slot == 1
    assert in_range.shots[0].end_x == pytest.approx(20.0 - PLAYER_RADIUS_M)
    too_far: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 2.0 + SHOT_RANGE_M + 1, OPEN_LANE_Y_M))
    step_arena(too_far, {0: SHOOT_EAST})
    assert too_far.shots[0].hit_slot is None
    assert too_far.shots[0].end_x == pytest.approx(2.0 + SHOT_RANGE_M)
    behind_wall: Arena = _make_duel((0, 2.0, 12.0), (1, 25.0, 12.0))
    step_arena(behind_wall, {0: SHOOT_EAST})
    assert behind_wall.shots[0].hit_slot is None
    assert behind_wall.shots[0].end_x == pytest.approx(18.0)


def test_shots_pass_through_teammates() -> None:
    """`test_shots_pass_through_teammates` checks that friendly fire is off."""
    arena: Arena = _make_duel(
        (0, 2.0, OPEN_LANE_Y_M), (0, 8.0, OPEN_LANE_Y_M), (1, 12.0, OPEN_LANE_Y_M)
    )
    step_arena(arena, {0: SHOOT_EAST})
    assert arena.shots[0].hit_slot == 2
    assert arena.fighters[1].is_alive


def test_cooldown_and_death_stop_the_trigger() -> None:
    """`test_cooldown_and_death_stop_the_trigger` checks the trigger's limits."""
    arena: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 30.0, OPEN_LANE_Y_M))
    arena.fighters[1].x = 2.0 + SHOT_RANGE_M + 5
    cooldown_tick_count: int = round(SHOT_COOLDOWN_S * TICK_HZ)
    firing_ticks: list[int] = []
    for tick in range(2 * cooldown_tick_count + 1):
        step_arena(arena, {0: SHOOT_EAST})
        if arena.shots:
            firing_ticks.append(tick)
    assert firing_ticks == [0, cooldown_tick_count, 2 * cooldown_tick_count]
    arena.fighters[0].is_alive = False
    arena.fighters[0].respawn_at_s = LONG_MATCH_S
    step_arena(arena, {0: SHOOT_EAST})
    assert len(arena.shots) == 0


def test_shots_of_one_tick_land_together() -> None:
    """`test_shots_of_one_tick_land_together` checks a trade, and a kill shared by two shooters."""
    trade: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 20.0, OPEN_LANE_Y_M))
    step_arena(trade, {0: SHOOT_EAST, 1: SHOOT_WEST})
    assert not trade.fighters[0].is_alive and not trade.fighters[1].is_alive
    assert trade.scores == [1, 1]
    shared: Arena = _make_duel(
        (0, 2.0, OPEN_LANE_Y_M),
        (0, OPEN_LANE_X_M, 12.0),
        (1, OPEN_LANE_X_M, OPEN_LANE_Y_M),
    )
    shoot_down: PlayerInput = PlayerInput(0.0, 0.0, -math.pi / 2, True)
    step_arena(shared, {0: SHOOT_EAST, 1: shoot_down})
    assert [f.kill_count for f in shared.fighters] == [1, 0, 0]
    assert shared.fighters[2].death_count == 1 and shared.scores == [1, 0]


def test_the_dead_respawn_at_their_spawn_point() -> None:
    """`test_the_dead_respawn_at_their_spawn_point` checks the wait and the place."""
    arena: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 20.0, OPEN_LANE_Y_M))
    step_arena(arena, {0: SHOOT_EAST})
    for _ in range(round(RESPAWN_S * TICK_HZ) - 1):
        step_arena(arena, {})
    assert not arena.fighters[1].is_alive
    step_arena(arena, {})
    assert arena.fighters[1].is_alive
    assert (arena.fighters[1].x, arena.fighters[1].y) == (ARENA_WIDTH_M - 2.0, 4.0)


def test_matches_end_at_score_or_time() -> None:
    """`test_matches_end_at_score_or_time` checks each way a match ends, and after."""
    by_score: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 20.0, OPEN_LANE_Y_M))
    by_score.score_limit = 1
    step_arena(by_score, {0: SHOOT_EAST})
    assert by_score.is_over and by_score.winner_team == 0
    with pytest.raises(ValueError, match="over"):
        step_arena(by_score, {})
    by_time: Arena = create_arena([0, 1], 10, 1.0)
    by_time.scores = [2, 3]
    for _ in range(TICK_HZ):
        step_arena(by_time, {})
    assert by_time.is_over and by_time.winner_team == 1
    level: Arena = create_arena([0, 1], 10, 1.0)
    for _ in range(TICK_HZ):
        step_arena(level, {})
    assert level.is_over and level.winner_team is None


def test_bots_aim_or_run() -> None:
    """`test_bots_aim_or_run` checks that bots aim at what they see and run toward the rest."""
    rng: random.Random = random.Random(1)
    seen: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 20.0, OPEN_LANE_Y_M))
    aims_rad: list[float] = [decide_bot_input(seen, 0, rng).aim_rad for _ in range(200)]
    assert abs(sum(aims_rad) / len(aims_rad)) < 0.05
    hidden: Arena = _make_duel((0, 2.0, 12.0), (1, 25.0, 12.0))
    run: PlayerInput = decide_bot_input(hidden, 0, rng)
    assert run.move_x > 0 and not run.is_shooting
    hidden.fighters[0].is_alive = False
    assert decide_bot_input(hidden, 0, rng) == PlayerInput(0.0, 0.0, 0.0, False)


def _play_bot_match(seed: int) -> list[dict]:
    """`_play_bot_match` plays ten bots to the end and returns every frame."""
    arena: Arena = create_arena([0] * 5 + [1] * 5, 20, 180.0)
    rng: random.Random = random.Random(seed)
    frames: list[dict] = []
    while not arena.is_over:
        step_arena(arena, {slot: decide_bot_input(arena, slot, rng) for slot in range(10)})
        frames.append(describe_frame(arena))
    return frames


def test_a_seeded_bot_match_replays_the_same() -> None:
    """`test_a_seeded_bot_match_replays_the_same` checks that a seed fixes a whole match."""
    frames: list[dict] = _play_bot_match(7)
    assert frames == _play_bot_match(7)
    assert max(frames[-1]["s"]) == 20


def test_frames_are_rounded_and_list_shots() -> None:
    """`test_frames_are_rounded_and_list_shots` checks the frame's shape."""
    arena: Arena = _make_duel((0, 2.0, OPEN_LANE_Y_M), (1, 20.123456, OPEN_LANE_Y_M))
    step_arena(arena, {0: SHOOT_EAST})
    frame: dict = describe_frame(arena)
    assert frame["t"] == 0.05 and frame["s"] == [1, 0]
    assert frame["f"][1] == [20.12, 1.0, 0.0, 0]
    assert frame["k"] == [[0, 19.62, 1.0, 1]]
    assert len(describe_layout()["walls"]) > 0
