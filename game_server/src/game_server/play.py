"""`play.py` serves one room over HTTP and WebSockets, as `common/protocol/square_game.md` says.

Interface:
    TICK_HZ: How many times a second the room moves and every client gets its state.
    create_app: Returns the app that answers `GET /health` and plays over `/play`.

Example:
    >>> app = create_app(room.Room(1000), game_server_id, "http://127.0.0.1:8000")
"""

import asyncio
import collections.abc
import contextlib
import json
import time
import typing
import urllib.error
import urllib.request
import uuid

import fastapi
import pydantic

from game_server import room

TICK_HZ: float = 30.0
_NOT_ADMITTED_CLOSE_CODE: int = 4403
_CANNOT_JOIN_CLOSE_CODE: int = 4409
_BAD_MESSAGE_CLOSE_CODE: int = 4400
_HTTP_NOT_FOUND: int = 404
_ADMISSION_TIMEOUT_S: float = 5.0
_BAD_MESSAGE_REASON: str = "expected a move or shoot message, as square_game.md specifies"


class _MoveMessage(pydantic.BaseModel):
    """`_MoveMessage` is the spec's `move` message: which way the player wants to go.

    Example:
        >>> _MoveMessage(type="move", direction_x=1.0, direction_y=0.0)
    """

    model_config = pydantic.ConfigDict(extra="forbid")
    type: typing.Literal["move"]
    direction_x: float
    direction_y: float


class _ShootMessage(pydantic.BaseModel):
    """`_ShootMessage` is the spec's `shoot` message: the point the player fires a laser toward.

    Example:
        >>> _ShootMessage(type="shoot", aim_x_m=20.0, aim_y_m=5.0)
    """

    model_config = pydantic.ConfigDict(extra="forbid")
    type: typing.Literal["shoot"]
    aim_x_m: float
    aim_y_m: float


_CLIENT_MESSAGE: pydantic.TypeAdapter[_MoveMessage | _ShootMessage] = pydantic.TypeAdapter(
    typing.Annotated[_MoveMessage | _ShootMessage, pydantic.Field(discriminator="type")]
)


def _is_admitted(
    management_server_url: str, game_server_id: uuid.UUID, player_id: uuid.UUID
) -> bool:
    """`_is_admitted` asks the management server whether it added `player_id` to this game server.

    Raises:
        urllib.error.URLError: The management server didn't answer, or answered with an error.

    Example:
        >>> _is_admitted("http://127.0.0.1:8000", game_server_id, player_id)
    """
    url: str = f"{management_server_url}/game-servers/{game_server_id}/players/{player_id}"
    try:
        with urllib.request.urlopen(url, timeout=_ADMISSION_TIMEOUT_S):
            return True
    except urllib.error.HTTPError as error:
        if error.code == _HTTP_NOT_FOUND:
            return False
        raise


async def _tick_forever(
    game_room: room.Room, state_queues: dict[uuid.UUID, asyncio.Queue[str]]
) -> None:
    """`_tick_forever` moves the room `TICK_HZ` times a second and queues its state for clients.

    Example:
        >>> asyncio.create_task(_tick_forever(game_room, state_queues))
    """
    previous_s: float = time.monotonic()
    while True:
        await asyncio.sleep(1 / TICK_HZ)
        now_s: float = time.monotonic()
        game_room.step(now_s - previous_s)
        previous_s = now_s
        state: str = json.dumps(game_room.toState())
        for state_queue in state_queues.values():
            if state_queue.full():
                state_queue.get_nowait()
            state_queue.put_nowait(state)


async def _send_states(websocket: fastapi.WebSocket, state_queue: asyncio.Queue[str]) -> None:
    """`_send_states` sends a client each state it is given, until the client disconnects.

    Example:
        >>> asyncio.create_task(_send_states(websocket, state_queue))
    """
    try:
        while True:
            await websocket.send_text(await state_queue.get())
    except fastapi.WebSocketDisconnect:
        return


def create_app(
    game_room: room.Room, game_server_id: uuid.UUID, management_server_url: str
) -> fastapi.FastAPI:
    """`create_app` returns an app that plays `game_room` with the players the server admits.

    Args:
        game_room: The room that players join.
        game_server_id: This game server's ID, for admission checks.
        management_server_url: Where to check that a player was added.

    Example:
        >>> app = create_app(game_room, game_server_id, "http://127.0.0.1:8000")
    """
    state_queues: dict[uuid.UUID, asyncio.Queue[str]] = {}

    @contextlib.asynccontextmanager
    async def tick_while_running(_app: fastapi.FastAPI) -> collections.abc.AsyncIterator[None]:
        """`tick_while_running` is a lifespan that ticks the room from startup to shutdown.

        Example:
            >>> fastapi.FastAPI(lifespan=tick_while_running)
        """
        ticker: asyncio.Task[None] = asyncio.create_task(_tick_forever(game_room, state_queues))
        yield
        ticker.cancel()

    app: fastapi.FastAPI = fastapi.FastAPI(title="Warped game server", lifespan=tick_while_running)

    @app.get("/health")
    def get_health() -> dict[str, str]:
        """`get_health` answers `GET /health` once the game server accepts players.

        Example:
            >>> client.get("/health")
        """
        return {"status": "ok"}

    @app.websocket("/play")
    async def play(websocket: fastapi.WebSocket, player_id: uuid.UUID) -> None:
        """`play` admits a player, then plays until the player disconnects.

        Example:
            >>> client.websocket_connect(f"/play?player_id={player_id}")
        """
        await websocket.accept()
        if not await asyncio.to_thread(
            _is_admitted, management_server_url, game_server_id, player_id
        ):
            await websocket.close(_NOT_ADMITTED_CLOSE_CODE, f"player {player_id} wasn't added")
            return
        try:
            number, color_hex = game_room.addPlayer(player_id)
        except room.CannotJoinError as error:
            await websocket.close(_CANNOT_JOIN_CLOSE_CODE, str(error))
            return
        state_queue: asyncio.Queue[str] = asyncio.Queue(maxsize=1)
        state_queues[player_id] = state_queue
        try:
            await websocket.send_json(
                {
                    "type": "welcome",
                    "number": number,
                    "color_hex": color_hex,
                    "map_size_m": room.MAP_SIZE_M,
                    "player_size_m": room.PLAYER_SIZE_M,
                    "schwarzschild_radius_m": room.SCHWARZSCHILD_RADIUS_M,
                    "shot_cooldown_s": room.SHOT_COOLDOWN_S,
                }
            )
            sender: asyncio.Task[None] = asyncio.create_task(_send_states(websocket, state_queue))
            try:
                async for text in websocket.iter_text():
                    message: _MoveMessage | _ShootMessage = _CLIENT_MESSAGE.validate_json(text)
                    if isinstance(message, _MoveMessage):
                        game_room.setDirection(player_id, message.direction_x, message.direction_y)
                    else:
                        game_room.shoot(player_id, message.aim_x_m, message.aim_y_m)
            except pydantic.ValidationError:
                await websocket.close(_BAD_MESSAGE_CLOSE_CODE, _BAD_MESSAGE_REASON)
            finally:
                sender.cancel()
        finally:
            del state_queues[player_id]
            game_room.removePlayer(player_id)

    return app
