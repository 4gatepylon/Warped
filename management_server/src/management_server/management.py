"""`management.py` is the internal management API: it owns game servers and their players.

Interface:
    Management: The internal management API of `management_server/internal_api.md`.

Example:
    >>> management_api = Management(game_server_launcher)
    >>> game_server = management_api.createGameServer(models.GameServerCreate(max_player_count=4))

Must know:
    Limitations:
        - Records live in memory, so they are lost when the management server stops.
    Gotchas:
        - Every method holds one lock, so calls from several threads run one at a time.
"""

import dataclasses
import datetime
import threading
import uuid

from management_server import errors, launcher, models


@dataclasses.dataclass
class _GameServerRecord:
    """`_GameServerRecord` is what the management server remembers about one game server.

    Attributes:
        The fields of `models.GameServer` that don't come from the launcher, and
        `player_ids`, the players added to the game server and not removed.

    Example:
        >>> record = _GameServerRecord(game_server_id, models.GameServerConfig(), 4, created_at)
    """

    game_server_id: uuid.UUID
    config: models.GameServerConfig
    max_player_count: int
    created_at: datetime.datetime
    player_ids: set[uuid.UUID] = dataclasses.field(default_factory=set)


class Management:
    """`Management` creates and finds game servers, and tracks who may join each one.

    Args:
        game_server_launcher: Where game servers run.

    Example:
        >>> management_api = Management(game_server_launcher)
    """

    def __init__(self, game_server_launcher: launcher.Launcher) -> None:
        self._game_server_launcher: launcher.Launcher = game_server_launcher
        self._lock: threading.Lock = threading.Lock()
        self._records: dict[uuid.UUID, _GameServerRecord] = {}

    def createGameServer(self, request: models.GameServerCreate) -> models.GameServer:
        """`createGameServer` launches a game server and returns it without waiting for it to run.

        Args:
            request: The game server's configuration and maximum number of players.

        Example:
            >>> management_api.createGameServer(models.GameServerCreate(max_player_count=4))
        """
        with self._lock:
            record: _GameServerRecord = _GameServerRecord(
                game_server_id=uuid.uuid4(),
                config=request.config,
                max_player_count=request.max_player_count,
                created_at=datetime.datetime.now(datetime.UTC),
            )
            self._game_server_launcher.launch(
                record.game_server_id, record.config, record.max_player_count
            )
            self._records[record.game_server_id] = record
            return self._toGameServer(record)

    def getGameServer(self, game_server_id: uuid.UUID) -> models.GameServer:
        """`getGameServer` returns a game server, with its state and address from the launcher.

        Args:
            game_server_id: The game server to return.

        Raises:
            NotFoundError: There is no such game server.

        Example:
            >>> management_api.getGameServer(game_server_id).state
        """
        with self._lock:
            return self._toGameServer(self._getRecord(game_server_id))

    def _getRecord(self, game_server_id: uuid.UUID) -> _GameServerRecord:
        """`_getRecord` returns a game server's record.

        Raises:
            NotFoundError: There is no such game server.

        Example:
            >>> record = self._getRecord(game_server_id)
        """
        if game_server_id not in self._records:
            raise errors.NotFoundError(f"no game server {game_server_id}")
        return self._records[game_server_id]

    def _toGameServer(self, record: _GameServerRecord) -> models.GameServer:
        """`_toGameServer` combines a record with the launcher's view of its game server.

        Example:
            >>> game_server = self._toGameServer(record)
        """
        launched_game_server: models.LaunchedGameServer = self._game_server_launcher.get(
            record.game_server_id
        )
        return models.GameServer(
            game_server_id=record.game_server_id,
            state=launched_game_server.state,
            address=launched_game_server.address,
            config=record.config,
            max_player_count=record.max_player_count,
            player_count=len(record.player_ids),
            created_at=record.created_at,
        )
