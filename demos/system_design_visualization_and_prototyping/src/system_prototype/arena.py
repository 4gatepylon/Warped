"""`arena.py` is the stand-in game's rules: two teams in a flat arena seen from above.

The stand-in game gives game servers something real to run, record and show until the real game
takes its place. Its rules are plain functions over an `Arena`, so they are deterministic: the same
inputs and the same random seed give the same match.

Interface:
    TICK_HZ: How many times a second a game server steps the arena.
    PlayerInput: What one player does in one tick.
    Fighter: One player's body in the arena.
    Shot: One shot fired in the last tick.
    Arena: A match's whole state.
    create_arena: Puts the players at their teams' spawn points.
    step_arena: Moves the arena forward by one tick.
    decide_bot_input: What a bot does this tick.
    describe_layout: The arena's fixed shape, for viewers.
    describe_frame: The arena's state now, small enough to send and record every tick.

Example:
    >>> arena = create_arena([0, 0, 1, 1], score_limit=10, time_limit_s=180.0)
    >>> step_arena(arena, {0: PlayerInput(1.0, 0.0, 0.0, True)})

Must know:
    Limitations:
        - Players don't collide with each other, and shots pass through teammates.
"""

import math
import random
from dataclasses import dataclass, field

TICK_HZ: int = 20
DT_S: float = 1.0 / TICK_HZ
ARENA_WIDTH_M: float = 40.0
ARENA_HEIGHT_M: float = 24.0
PLAYER_RADIUS_M: float = 0.5
RUN_SPEED_M_PER_S: float = 6.0
SHOT_RANGE_M: float = 30.0
SHOT_COOLDOWN_S: float = 0.6
RESPAWN_S: float = 2.0
TEAM_COUNT: int = 2
SPAWN_INSET_M: float = 2.0
SPAWN_YS_M: tuple[float, ...] = (4.0, 8.0, 12.0, 16.0, 20.0)
WALLS: tuple[tuple[float, float, float, float], ...] = (
    (18.0, 9.0, 22.0, 15.0),
    (10.0, 4.0, 12.0, 10.0),
    (28.0, 14.0, 30.0, 20.0),
    (10.0, 14.0, 12.0, 20.0),
    (28.0, 4.0, 30.0, 10.0),
    (16.0, 2.0, 24.0, 3.0),
    (16.0, 21.0, 24.0, 22.0),
)
BOT_AIM_ERROR_RAD: float = 0.25
BOT_TRIGGER_CHANCE_PER_TICK: float = 0.06
BOT_STRAFE_PERIOD_S: float = 1.5
BOT_WANDER_RAD: float = 0.6
FRAME_DECIMALS: int = 2
TIME_TOLERANCE_S: float = 1e-9


@dataclass(frozen=True)
class PlayerInput:
    """`PlayerInput` is what one player does in one tick.

    Attributes:
        move_x: The run direction's x, from -1 to 1.
        move_y: The run direction's y, from -1 to 1; a longer (x, y) runs no faster.
        aim_rad: Where the player aims, counterclockwise from the +x axis.
        is_shooting: Whether the trigger is held.

    Example:
        >>> PlayerInput(move_x=1.0, move_y=0.0, aim_rad=0.0, is_shooting=False)
    """

    move_x: float
    move_y: float
    aim_rad: float
    is_shooting: bool


IDLE_INPUT: PlayerInput = PlayerInput(0.0, 0.0, 0.0, False)


@dataclass
class Fighter:
    """`Fighter` is one player's body in the arena.

    Attributes:
        slot: The player's place in the match, from 0.
        team: 0 or 1.
        x: Position in meters from the left wall.
        y: Position in meters from the bottom wall.
        aim_rad: Where it aims.
        is_alive: Whether it is in play; a dead fighter waits to respawn.
        respawn_at_s: When a dead fighter comes back, in match time.
        shot_ready_at_s: When it may shoot again, in match time.
        kill_count: Kills so far.
        death_count: Deaths so far.

    Example:
        >>> Fighter(0, 0, 2.0, 4.0)
    """

    slot: int
    team: int
    x: float
    y: float
    aim_rad: float = 0.0
    is_alive: bool = True
    respawn_at_s: float = 0.0
    shot_ready_at_s: float = 0.0
    kill_count: int = 0
    death_count: int = 0


@dataclass(frozen=True)
class Shot:
    """`Shot` is one shot fired in the last tick, from the shooter to where it stopped.

    Attributes:
        shooter_slot: Who fired.
        end_x: Where the shot stopped: a wall, a fighter or its range.
        end_y: The same point's y.
        hit_slot: The fighter it killed, or None.

    Example:
        >>> Shot(0, 10.0, 4.0, None)
    """

    shooter_slot: int
    end_x: float
    end_y: float
    hit_slot: int | None


@dataclass
class Arena:
    """`Arena` is a match's whole state.

    Attributes:
        fighters: One per slot, in slot order.
        scores: Kills per team.
        score_limit: Kills that win.
        time_limit_s: The match's longest length; the higher score then wins.
        time_s: Match time so far.
        shots: The shots of the last tick.
        is_over: Whether the match has ended.
        winner_team: The team that won, or None for a draw or a match still going.

    Example:
        >>> create_arena([0, 1], score_limit=10, time_limit_s=180.0).scores
        [0, 0]
    """

    fighters: list[Fighter]
    score_limit: int
    time_limit_s: float
    scores: list[int] = field(default_factory=lambda: [0] * TEAM_COUNT)
    time_s: float = 0.0
    shots: list[Shot] = field(default_factory=list)
    is_over: bool = False
    winner_team: int | None = None


def create_arena(team_by_slot: list[int], score_limit: int, time_limit_s: float) -> Arena:
    """`create_arena` puts each player at its team's spawn point, aiming at the other side.

    Args:
        team_by_slot: Each slot's team, 0 or 1; at most 5 per team.
        score_limit: Kills that win, at least 1.
        time_limit_s: The match's longest length, more than 0.

    Raises:
        ValueError: A team is not 0 or 1, a team has more than 5 players, or a limit is too small.

    Example:
        >>> create_arena([0, 0, 1, 1], score_limit=10, time_limit_s=180.0)
    """
    if score_limit < 1 or time_limit_s <= 0:
        raise ValueError(f"limits must be positive, got {score_limit} kills and {time_limit_s} s")
    if any(team not in range(TEAM_COUNT) for team in team_by_slot):
        raise ValueError(f"each team must be 0 or 1, got {team_by_slot}")
    if any(team_by_slot.count(team) > len(SPAWN_YS_M) for team in range(TEAM_COUNT)):
        raise ValueError(f"a team has at most {len(SPAWN_YS_M)} players, got {team_by_slot}")
    fighters: list[Fighter] = []
    for slot, team in enumerate(team_by_slot):
        x, y = _read_spawn_point(team, team_by_slot[:slot].count(team))
        fighters.append(Fighter(slot, team, x, y, aim_rad=0.0 if team == 0 else math.pi))
    return Arena(fighters, score_limit, time_limit_s)


def step_arena(arena: Arena, inputs_by_slot: dict[int, PlayerInput], dt_s: float = DT_S) -> None:
    """`step_arena` moves the arena forward by one tick: respawns, runs, then shots, then the end.

    Everyone's shots of a tick are traced at once, before anyone falls, so two fighters who shoot
    each other in the same tick both die. A fighter hit by two shots counts as the kill of the
    lowest slot that hit it.

    Args:
        arena: The arena, changed in place.
        inputs_by_slot: Each slot's input; a slot that is missing stands still.
        dt_s: The tick's length.

    Raises:
        ValueError: The match is already over.

    Example:
        >>> step_arena(arena, {0: PlayerInput(1.0, 0.0, 0.0, True)})
    """
    if arena.is_over:
        raise ValueError("the match is already over")
    arena.time_s += dt_s
    for fighter in arena.fighters:
        if not fighter.is_alive and arena.time_s >= fighter.respawn_at_s:
            _respawn(arena, fighter)
    for fighter in arena.fighters:
        player_input: PlayerInput = inputs_by_slot.get(fighter.slot, IDLE_INPUT)
        if fighter.is_alive:
            fighter.aim_rad = player_input.aim_rad
            _move_fighter(fighter, player_input.move_x, player_input.move_y, dt_s)
    arena.shots = [
        _cast_shot(arena, fighter)
        for fighter in arena.fighters
        if fighter.is_alive
        and inputs_by_slot.get(fighter.slot, IDLE_INPUT).is_shooting
        and arena.time_s >= fighter.shot_ready_at_s - TIME_TOLERANCE_S
    ]
    killed_slots: set[int] = set()
    for shot in arena.shots:
        arena.fighters[shot.shooter_slot].shot_ready_at_s = arena.time_s + SHOT_COOLDOWN_S
        if shot.hit_slot is None or shot.hit_slot in killed_slots:
            continue
        killed_slots.add(shot.hit_slot)
        shooter: Fighter = arena.fighters[shot.shooter_slot]
        victim: Fighter = arena.fighters[shot.hit_slot]
        shooter.kill_count += 1
        arena.scores[shooter.team] += 1
        victim.death_count += 1
        victim.is_alive = False
        victim.respawn_at_s = arena.time_s + RESPAWN_S
    is_time_up: bool = arena.time_s >= arena.time_limit_s - TIME_TOLERANCE_S
    if max(arena.scores) >= arena.score_limit or is_time_up:
        arena.is_over = True
        arena.winner_team = _read_leading_team(arena.scores)


def decide_bot_input(arena: Arena, slot: int, rng: random.Random) -> PlayerInput:
    """`decide_bot_input` says what the bot in `slot` does this tick.

    A bot that sees an enemy in range aims at the nearest one, with a small error, pulls the
    trigger now and then, and strafes. A bot that sees none runs toward the nearest enemy.

    Args:
        arena: The arena.
        slot: The bot's slot.
        rng: The match's random numbers, so that a seeded match replays the same.

    Returns:
        The bot's input.

    Example:
        >>> decide_bot_input(arena, 3, random.Random(1))
    """
    bot: Fighter = arena.fighters[slot]
    enemies: list[Fighter] = [
        fighter for fighter in arena.fighters if fighter.team != bot.team and fighter.is_alive
    ]
    if not bot.is_alive or len(enemies) == 0:
        return IDLE_INPUT
    enemies.sort(key=lambda enemy: math.hypot(enemy.x - bot.x, enemy.y - bot.y))
    visible_enemies: list[Fighter] = [
        enemy
        for enemy in enemies
        if math.hypot(enemy.x - bot.x, enemy.y - bot.y) <= SHOT_RANGE_M
        and _has_line_of_sight(bot.x, bot.y, enemy.x, enemy.y)
    ]
    if len(visible_enemies) == 0:
        heading_rad: float = math.atan2(enemies[0].y - bot.y, enemies[0].x - bot.x)
        heading_rad += BOT_WANDER_RAD * math.sin(arena.time_s + slot)
        return PlayerInput(math.cos(heading_rad), math.sin(heading_rad), heading_rad, False)
    target: Fighter = visible_enemies[0]
    aim_rad: float = math.atan2(target.y - bot.y, target.x - bot.x)
    aim_rad += rng.gauss(0.0, BOT_AIM_ERROR_RAD)
    strafe_sign: float = 1.0 if (int(arena.time_s / BOT_STRAFE_PERIOD_S) + slot) % 2 else -1.0
    return PlayerInput(
        -math.sin(aim_rad) * strafe_sign,
        math.cos(aim_rad) * strafe_sign,
        aim_rad,
        rng.random() < BOT_TRIGGER_CHANCE_PER_TICK,
    )


def describe_layout() -> dict:
    """`describe_layout` returns the arena's fixed shape, which a viewer draws under every frame.

    Example:
        >>> describe_layout()["width_m"]
        40.0
    """
    return {
        "width_m": ARENA_WIDTH_M,
        "height_m": ARENA_HEIGHT_M,
        "player_radius_m": PLAYER_RADIUS_M,
        "shot_range_m": SHOT_RANGE_M,
        "tick_hz": TICK_HZ,
        "walls": [list(wall) for wall in WALLS],
    }


def describe_frame(arena: Arena) -> dict:
    """`describe_frame` returns the arena's state now, rounded to centimeters, to send and record.

    Returns:
        `t`, the match time; `f`, each fighter as `[x, y, aim_rad, is_alive]`; `s`, the scores; and
        `k`, each shot of the tick as `[shooter_slot, end_x, end_y, hit_slot or -1]`.

    Example:
        >>> describe_frame(arena)["s"]
        [0, 0]
    """
    return {
        "t": round(arena.time_s, FRAME_DECIMALS),
        "f": [
            [
                round(fighter.x, FRAME_DECIMALS),
                round(fighter.y, FRAME_DECIMALS),
                round(fighter.aim_rad, FRAME_DECIMALS),
                1 if fighter.is_alive else 0,
            ]
            for fighter in arena.fighters
        ],
        "s": list(arena.scores),
        "k": [
            [
                shot.shooter_slot,
                round(shot.end_x, FRAME_DECIMALS),
                round(shot.end_y, FRAME_DECIMALS),
                -1 if shot.hit_slot is None else shot.hit_slot,
            ]
            for shot in arena.shots
        ],
    }


def _read_spawn_point(team: int, index_in_team: int) -> tuple[float, float]:
    """`_read_spawn_point` returns where the `index_in_team`-th player of `team` (re)spawns.

    Example:
        >>> _read_spawn_point(1, 0)
        (38.0, 4.0)
    """
    x: float = SPAWN_INSET_M if team == 0 else ARENA_WIDTH_M - SPAWN_INSET_M
    return x, SPAWN_YS_M[index_in_team]


def _respawn(arena: Arena, fighter: Fighter) -> None:
    """`_respawn` brings a dead fighter back at its own spawn point.

    Example:
        >>> _respawn(arena, arena.fighters[2])
    """
    index_in_team: int = [other.team for other in arena.fighters[: fighter.slot]].count(
        fighter.team
    )
    fighter.x, fighter.y = _read_spawn_point(fighter.team, index_in_team)
    fighter.is_alive = True


def _read_leading_team(scores: list[int]) -> int | None:
    """`_read_leading_team` returns the team with the most kills, or None when they are level.

    Example:
        >>> _read_leading_team([3, 5])
        1
    """
    if scores[0] == scores[1]:
        return None
    return 0 if scores[0] > scores[1] else 1


def _is_in_wall(x: float, y: float) -> bool:
    """`_is_in_wall` says whether a fighter centred at (x, y) overlaps a wall.

    Example:
        >>> _is_in_wall(20.0, 12.0)
        True
    """
    for x0, y0, x1, y1 in WALLS:
        nearest_x: float = min(max(x, x0), x1)
        nearest_y: float = min(max(y, y0), y1)
        if math.hypot(x - nearest_x, y - nearest_y) < PLAYER_RADIUS_M:
            return True
    return False


def _move_fighter(fighter: Fighter, move_x: float, move_y: float, dt_s: float) -> None:
    """`_move_fighter` runs a fighter along (move_x, move_y), sliding along walls it meets.

    Example:
        >>> _move_fighter(fighter, 1.0, 0.0, DT_S)
    """
    length: float = math.hypot(move_x, move_y)
    if length == 0:
        return
    step_m: float = RUN_SPEED_M_PER_S * dt_s / max(length, 1.0)
    new_x: float = min(
        max(fighter.x + move_x * step_m, PLAYER_RADIUS_M), ARENA_WIDTH_M - PLAYER_RADIUS_M
    )
    if not _is_in_wall(new_x, fighter.y):
        fighter.x = new_x
    new_y: float = min(
        max(fighter.y + move_y * step_m, PLAYER_RADIUS_M), ARENA_HEIGHT_M - PLAYER_RADIUS_M
    )
    if not _is_in_wall(fighter.x, new_y):
        fighter.y = new_y


def _trace_to_wall(x: float, y: float, dx: float, dy: float, length_m: float) -> float:
    """`_trace_to_wall` returns how far a ray from (x, y) along unit (dx, dy) goes before a wall.

    Returns:
        The distance to the first wall or the arena's edge, at most `length_m`.

    Example:
        >>> _trace_to_wall(2.0, 12.0, 1.0, 0.0, 30.0)
        16.0
    """
    nearest_m: float = length_m
    for wall in WALLS:
        wall_m: float | None = _trace_into_box(x, y, dx, dy, wall)
        if wall_m is not None:
            nearest_m = min(nearest_m, wall_m)
    for origin, direction, size_m in ((x, dx, ARENA_WIDTH_M), (y, dy, ARENA_HEIGHT_M)):
        if direction > 0:
            nearest_m = min(nearest_m, (size_m - origin) / direction)
        elif direction < 0:
            nearest_m = min(nearest_m, -origin / direction)
    return nearest_m


def _trace_into_box(
    x: float, y: float, dx: float, dy: float, box: tuple[float, float, float, float]
) -> float | None:
    """`_trace_into_box` returns how far a ray from outside `box` goes before it enters it.

    Returns:
        The distance, or None when the ray misses the box.

    Example:
        >>> _trace_into_box(2.0, 12.0, 1.0, 0.0, (18.0, 9.0, 22.0, 15.0))
        16.0
    """
    t_enter: float = 0.0
    t_exit: float = math.inf
    for origin, direction, low, high in ((x, dx, box[0], box[2]), (y, dy, box[1], box[3])):
        if direction == 0:
            if origin < low or origin > high:
                return None
            continue
        t_low: float = (low - origin) / direction
        t_high: float = (high - origin) / direction
        t_enter = max(t_enter, min(t_low, t_high))
        t_exit = min(t_exit, max(t_low, t_high))
    return t_enter if t_enter <= t_exit else None


def _has_line_of_sight(x0: float, y0: float, x1: float, y1: float) -> bool:
    """`_has_line_of_sight` says whether no wall stands between (x0, y0) and (x1, y1).

    Example:
        >>> _has_line_of_sight(2.0, 12.0, 38.0, 12.0)
        False
    """
    distance_m: float = math.hypot(x1 - x0, y1 - y0)
    if distance_m == 0:
        return True
    dx: float = (x1 - x0) / distance_m
    dy: float = (y1 - y0) / distance_m
    return _trace_to_wall(x0, y0, dx, dy, distance_m) >= distance_m


def _cast_shot(arena: Arena, shooter: Fighter) -> Shot:
    """`_cast_shot` traces a shooter's shot to the nearest wall or living enemy, within range.

    Example:
        >>> _cast_shot(arena, arena.fighters[0])
    """
    dx: float = math.cos(shooter.aim_rad)
    dy: float = math.sin(shooter.aim_rad)
    nearest_m: float = _trace_to_wall(shooter.x, shooter.y, dx, dy, SHOT_RANGE_M)
    hit_slot: int | None = None
    for target in arena.fighters:
        if target.team == shooter.team or not target.is_alive:
            continue
        to_x: float = target.x - shooter.x
        to_y: float = target.y - shooter.y
        along_m: float = to_x * dx + to_y * dy
        off_squared: float = to_x * to_x + to_y * to_y - along_m * along_m
        if along_m < 0 or off_squared > PLAYER_RADIUS_M * PLAYER_RADIUS_M:
            continue
        hit_m: float = along_m - math.sqrt(PLAYER_RADIUS_M * PLAYER_RADIUS_M - off_squared)
        if hit_m < nearest_m:
            nearest_m = hit_m
            hit_slot = target.slot
    return Shot(shooter.slot, shooter.x + dx * nearest_m, shooter.y + dy * nearest_m, hit_slot)
