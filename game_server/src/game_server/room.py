"""`room.py` holds one square map: its players, their lasers, and the black hole at its center.

Interface:
    MAP_SIZE_M, PLAYER_SIZE_M, PLAYER_SPEED_M_PER_S, SCHWARZSCHILD_RADIUS_M, LASER_SPEED_M_PER_S,
        LASER_MAX_LENGTH_M, LASER_LINGER_S, SHOT_COOLDOWN_S: The rules of
        `common/protocol/square_game.md`.
    CannotJoinError: A player can't join the room.
    Room: The players and lasers in the map, and how they move.

Example:
    >>> room = Room(max_player_count=1000)
    >>> number, color_hex = room.addPlayer(player_id)
"""

import colorsys
import dataclasses
import math
import random
import uuid

from game_server import laser

MAP_SIZE_M: float = 24.0
PLAYER_SIZE_M: float = 0.8
PLAYER_SPEED_M_PER_S: float = 6.0
SCHWARZSCHILD_RADIUS_M: float = 1.2
LASER_SPEED_M_PER_S: float = 24.0
LASER_MAX_LENGTH_M: float = 48.0
LASER_LINGER_S: float = 0.3
SHOT_COOLDOWN_S: float = 2.0
_BLACK_HOLE_M: tuple[float, float] = (MAP_SIZE_M / 2, MAP_SIZE_M / 2)
_SPAWN_CLEARANCE_M: float = 3 * SCHWARZSCHILD_RADIUS_M
_COLOR_LIGHTNESS_FRAC: float = 0.6
_COLOR_SATURATION_FRAC: float = 0.8
_GOLDEN_RATIO_CONJUGATE: float = (math.sqrt(5) - 1) / 2
_BYTE_MAX: int = 255


class CannotJoinError(Exception):
    """`CannotJoinError` means a player is already in the room, or the room is full."""


@dataclasses.dataclass
class _Player:
    """`_Player` is one player's square, where it is heading, and whether it is still alive.

    Example:
        >>> _Player(1, "#e6194b", 10.0, 10.0)
    """

    number: int
    color_hex: str
    x_m: float
    y_m: float
    direction_x: float = 0.0
    direction_y: float = 0.0
    is_alive: bool = True
    last_shot_s: float = -math.inf


@dataclasses.dataclass
class _Laser:
    """`_Laser` is a shot: its whole path as traced when fired, and how long ago it was fired.

    Example:
        >>> _Laser(shooter_id, points, (2.0, 12.0))
    """

    shooter_id: uuid.UUID
    points_m: list[tuple[float, float]]
    fired_from_m: tuple[float, float]
    age_s: float = 0.0


def _color_hex(hue_frac: float) -> str:
    """`_color_hex` returns a bright color of a hue from 0 to 1, as `#rrggbb`.

    Example:
        >>> _color_hex(0.5)
        '#47ebeb'
    """
    rgb: tuple[float, float, float] = colorsys.hls_to_rgb(
        hue_frac, _COLOR_LIGHTNESS_FRAC, _COLOR_SATURATION_FRAC
    )
    return "#" + "".join(f"{round(channel * _BYTE_MAX):02x}" for channel in rgb)


def _clamp(value: float, low: float, high: float) -> float:
    """`_clamp` returns `value` moved into the range from `low` to `high`.

    Example:
        >>> _clamp(25.0, 0.4, 23.6)
        23.6
    """
    return min(max(value, low), high)


class Room:
    """`Room` holds the players and lasers of one map and moves them each tick.

    Args:
        max_player_count: How many players the room holds at once.

    Example:
        >>> room = Room(max_player_count=1000)

    Must know:
        Limitations:
            - A `Room` is not thread-safe; the game server uses it from one event loop.
    """

    def __init__(self, max_player_count: int) -> None:
        self._max_player_count: int = max_player_count
        self._players: dict[uuid.UUID, _Player] = {}
        self._lasers: list[_Laser] = []
        self._next_number: int = 1
        self._time_s: float = 0.0
        self._first_hue_frac: float = random.random()

    def addPlayer(self, player_id: uuid.UUID) -> tuple[int, str]:
        """`addPlayer` puts a new player at a random place away from the black hole.

        The room starts its colors at a random hue and turns each next one by the golden ratio,
        so that colors look random but stay far apart.

        Returns:
            The player's number in this room and its color, as `#rrggbb`.

        Raises:
            CannotJoinError: The player is already in the room, or the room is full.

        Example:
            >>> number, color_hex = room.addPlayer(player_id)
        """
        if player_id in self._players:
            raise CannotJoinError(f"player {player_id} is already in the room")
        if len(self._players) >= self._max_player_count:
            raise CannotJoinError(f"the room is full ({self._max_player_count} players)")
        half_size_m: float = PLAYER_SIZE_M / 2
        while True:
            x_m: float = random.uniform(half_size_m, MAP_SIZE_M - half_size_m)
            y_m: float = random.uniform(half_size_m, MAP_SIZE_M - half_size_m)
            if math.dist((x_m, y_m), _BLACK_HOLE_M) > _SPAWN_CLEARANCE_M:
                break
        hue_frac: float = (self._first_hue_frac + self._next_number * _GOLDEN_RATIO_CONJUGATE) % 1
        player: _Player = _Player(self._next_number, _color_hex(hue_frac), x_m, y_m)
        self._players[player_id] = player
        self._next_number += 1
        return player.number, player.color_hex

    def removePlayer(self, player_id: uuid.UUID) -> None:
        """`removePlayer` takes a player and its lasers out of the room.

        Raises:
            KeyError: The player isn't in the room.

        Example:
            >>> room.removePlayer(player_id)
        """
        del self._players[player_id]
        self._lasers = [shot for shot in self._lasers if shot.shooter_id != player_id]

    def setDirection(self, player_id: uuid.UUID, direction_x: float, direction_y: float) -> None:
        """`setDirection` sets where a living player moves, shortening a direction longer than 1.

        Raises:
            KeyError: The player isn't in the room.

        Example:
            >>> room.setDirection(player_id, 1.0, 0.0)
        """
        length: float = math.hypot(direction_x, direction_y)
        scale: float = 1 / length if length > 1 else 1.0
        player: _Player = self._players[player_id]
        player.direction_x = direction_x * scale
        player.direction_y = direction_y * scale

    def shoot(self, player_id: uuid.UUID, aim_x_m: float, aim_y_m: float) -> None:
        """`shoot` fires a laser toward a point, if the player is alive and its laser has recharged.

        Raises:
            KeyError: The player isn't in the room.

        Example:
            >>> room.shoot(player_id, 20.0, 5.0)
        """
        player: _Player = self._players[player_id]
        direction: tuple[float, float] = (aim_x_m - player.x_m, aim_y_m - player.y_m)
        is_recharged: bool = self._time_s - player.last_shot_s >= SHOT_COOLDOWN_S
        if not player.is_alive or not is_recharged or direction == (0.0, 0.0):
            return
        player.last_shot_s = self._time_s
        start_m: tuple[float, float] = (player.x_m, player.y_m)
        points_m: list[tuple[float, float]] = laser.trace_path(
            start_m,
            direction,
            _BLACK_HOLE_M,
            SCHWARZSCHILD_RADIUS_M,
            MAP_SIZE_M,
            LASER_MAX_LENGTH_M,
        )
        self._lasers.append(_Laser(player_id, points_m, start_m))

    def step(self, dt: float) -> None:
        """`step` moves players and lasers `dt` seconds, and eliminates players that lasers touch.

        Example:
            >>> room.step(1 / 30)
        """
        self._time_s += dt
        half_size_m: float = PLAYER_SIZE_M / 2
        for player in self._players.values():
            if not player.is_alive:
                continue
            player.x_m = _clamp(
                player.x_m + player.direction_x * PLAYER_SPEED_M_PER_S * dt,
                half_size_m,
                MAP_SIZE_M - half_size_m,
            )
            player.y_m = _clamp(
                player.y_m + player.direction_y * PLAYER_SPEED_M_PER_S * dt,
                half_size_m,
                MAP_SIZE_M - half_size_m,
            )
        for shot in self._lasers:
            shot.age_s += dt
        self._lasers = [
            shot
            for shot in self._lasers
            if self._players[shot.shooter_id].is_alive
            and shot.age_s <= self._growingTime(shot) + LASER_LINGER_S
        ]
        for shot in self._lasers:
            points_m: list[tuple[float, float]] = self._visiblePoints(shot)
            for player_id, player in self._players.items():
                if (
                    player_id != shot.shooter_id
                    and player.is_alive
                    and laser.touches_square(points_m, (player.x_m, player.y_m), half_size_m)
                ):
                    player.is_alive = False

    def toState(self) -> dict:
        """`toState` returns the spec's `state` message: living players and visible lasers.

        Example:
            >>> websocket.send_json(room.toState())
        """
        return {
            "type": "state",
            "players": [
                {
                    "number": player.number,
                    "color_hex": player.color_hex,
                    "x_m": round(player.x_m, 3),
                    "y_m": round(player.y_m, 3),
                }
                for player in self._players.values()
                if player.is_alive
            ],
            "lasers": [
                {"points_m": [[round(x, 3), round(y, 3)] for x, y in self._visiblePoints(shot)]}
                for shot in self._lasers
            ],
        }

    def _growingTime(self, shot: _Laser) -> float:
        """`_growingTime` returns how long a laser takes to grow to its whole path.

        Example:
            >>> self._growingTime(shot)
        """
        return len(shot.points_m) * laser.POINT_SPACING_M / LASER_SPEED_M_PER_S

    def _visiblePoints(self, shot: _Laser) -> list[tuple[float, float]]:
        """`_visiblePoints` returns the part of a laser grown so far, moved along with its shooter.

        Example:
            >>> points_m = self._visiblePoints(shot)
        """
        grown_point_count: int = 1 + int(shot.age_s * LASER_SPEED_M_PER_S / laser.POINT_SPACING_M)
        shooter: _Player = self._players[shot.shooter_id]
        offset_x_m: float = shooter.x_m - shot.fired_from_m[0]
        offset_y_m: float = shooter.y_m - shot.fired_from_m[1]
        return [(x + offset_x_m, y + offset_y_m) for x, y in shot.points_m[:grown_point_count]]
