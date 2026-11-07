"""`http_api.py` serves the management server's HTTP API with FastAPI.

Interface:
    create_app: Returns an app with the routes of `common/protocol/management_server_api.md`.
    lifespan_t: What `create_app` takes to run code at startup and shutdown.

Example:
    >>> app = create_app(management.Management(game_server_launcher))
"""

import collections.abc
import contextlib
import typing
import uuid

import fastapi

from management_server import errors, management, models

type lifespan_t = collections.abc.Callable[
    [fastapi.FastAPI], contextlib.AbstractAsyncContextManager[None]
]
DEFAULT_LIST_LIMIT: int = 50
MAX_LIST_LIMIT: int = 100
_STATUS_CODE_PER_ERROR: dict[type[Exception], int] = {
    errors.NotFoundError: fastapi.status.HTTP_404_NOT_FOUND,
    errors.UnavailableError: fastapi.status.HTTP_409_CONFLICT,
    errors.InvalidPageTokenError: fastapi.status.HTTP_422_UNPROCESSABLE_CONTENT,
}
_GAME_SERVER_PATH: str = "/game-servers/{game_server_id}"
_PLAYER_PATH: str = "/game-servers/{game_server_id}/players/{player_id}"


def _handle_error(_: fastapi.Request, error: Exception) -> fastapi.responses.JSONResponse:
    """`_handle_error` turns an internal management API error into its status and body.

    Example:
        >>> app.add_exception_handler(errors.NotFoundError, _handle_error)
    """
    return fastapi.responses.JSONResponse(
        status_code=_STATUS_CODE_PER_ERROR[type(error)], content={"detail": str(error)}
    )


def create_app(
    management_api: management.Management, lifespan: lifespan_t | None = None
) -> fastapi.FastAPI:
    """`create_app` returns an app whose routes only translate HTTP to `management_api` calls.

    Args:
        management_api: The internal management API that the routes call.
        lifespan: What the app runs at startup and shutdown, if anything.

    Example:
        >>> app = create_app(management_api)
    """
    app: fastapi.FastAPI = fastapi.FastAPI(title="Warped management server", lifespan=lifespan)
    for error_type in _STATUS_CODE_PER_ERROR:
        app.add_exception_handler(error_type, _handle_error)

    @app.post("/game-servers", status_code=fastapi.status.HTTP_201_CREATED)
    def create_game_server(request: models.GameServerCreate) -> models.GameServer:
        """`create_game_server` serves `POST /game-servers`.

        Example:
            >>> client.post("/game-servers", json={"max_player_count": 4})
        """
        return management_api.createGameServer(request)

    @app.get("/game-servers")
    def list_game_servers(
        has_open_place: bool = False,
        limit: typing.Annotated[int, fastapi.Query(ge=1, le=MAX_LIST_LIMIT)] = DEFAULT_LIST_LIMIT,
        page_token: str | None = None,
    ) -> models.GameServerList:
        """`list_game_servers` serves `GET /game-servers`.

        Example:
            >>> client.get("/game-servers", params={"has_open_place": True})
        """
        return management_api.listGameServers(has_open_place, limit, page_token)

    @app.get(_GAME_SERVER_PATH)
    def get_game_server(game_server_id: uuid.UUID) -> models.GameServer:
        """`get_game_server` serves `GET /game-servers/{game_server_id}`.

        Example:
            >>> client.get(f"/game-servers/{game_server_id}")
        """
        return management_api.getGameServer(game_server_id)

    @app.delete(_GAME_SERVER_PATH, status_code=fastapi.status.HTTP_204_NO_CONTENT)
    def delete_game_server(game_server_id: uuid.UUID) -> None:
        """`delete_game_server` serves `DELETE /game-servers/{game_server_id}`.

        Example:
            >>> client.delete(f"/game-servers/{game_server_id}")
        """
        management_api.deleteGameServer(game_server_id)

    @app.post(f"{_GAME_SERVER_PATH}/players", status_code=fastapi.status.HTTP_201_CREATED)
    def add_player(game_server_id: uuid.UUID, request: models.PlayerCreate) -> models.Player:
        """`add_player` serves `POST /game-servers/{game_server_id}/players`.

        Example:
            >>> client.post(f"/game-servers/{game_server_id}/players", json={})
        """
        return management_api.addPlayer(game_server_id, request)

    @app.get(_PLAYER_PATH)
    def get_player(game_server_id: uuid.UUID, player_id: uuid.UUID) -> models.Player:
        """`get_player` serves `GET /game-servers/{game_server_id}/players/{player_id}`.

        Example:
            >>> client.get(f"/game-servers/{game_server_id}/players/{player_id}")
        """
        return management_api.getPlayer(game_server_id, player_id)

    @app.delete(_PLAYER_PATH, status_code=fastapi.status.HTTP_204_NO_CONTENT)
    def remove_player(game_server_id: uuid.UUID, player_id: uuid.UUID) -> None:
        """`remove_player` serves `DELETE /game-servers/{game_server_id}/players/{player_id}`.

        Example:
            >>> client.delete(f"/game-servers/{game_server_id}/players/{player_id}")
        """
        management_api.removePlayer(game_server_id, player_id)

    return app
