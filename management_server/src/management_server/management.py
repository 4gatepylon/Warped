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

_SEQUENCE_BEFORE_FIRST: int = -1
_JOINABLE_STATES: frozenset[models.GameServerState] = frozenset(
    {models.GameServerState.STARTING, models.GameServerState.RUNNING}
)


@dataclasses.dataclass
class _GameServerRecord:
    """`_GameServerRecord` is what the management server remembers about one game server.

    Attributes:
        The fields of `models.GameServer` that don't come from the launcher; `sequence`, which
        orders game servers by creation; and `player_ids`, the players added and not removed.

    Example:
        >>> record = _GameServerRecord(game_server_id, 0, models.GameServerConfig(), 4, created_at)
    """

    game_server_id: uuid.UUID
    sequence: int
    config: models.GameServerConfig
    max_player_count: int
    created_at: datetime.datetime
    player_ids: set[uuid.UUID] = dataclasses.field(default_factory=set)


class Management:
    """`Management` creates, finds and deletes game servers, and tracks who may join each one.

    Args:
        game_server_launcher: Where game servers run.

    Example:
        >>> management_api = Management(game_server_launcher)
    """

    def __init__(self, game_server_launcher: launcher.Launcher) -> None:
        self._game_server_launcher: launcher.Launcher = game_server_launcher
        self._lock: threading.Lock = threading.Lock()
        self._records: dict[uuid.UUID, _GameServerRecord] = {}
        self._next_sequence: int = 0

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
                sequence=self._next_sequence,
                config=request.config,
                max_player_count=request.max_player_count,
                created_at=datetime.datetime.now(datetime.UTC),
            )
            self._game_server_launcher.launch(
                record.game_server_id, record.config, record.max_player_count
            )
            self._records[record.game_server_id] = record
            self._next_sequence += 1
            return self._toGameServer(record)

    def listGameServers(
        self, has_open_place: bool, limit: int, page_token: str | None
    ) -> models.GameServerList:
        """`listGameServers` returns one page of game servers, oldest first.

        Args:
            has_open_place: Whether to list only game servers that aren't full.
            limit: The most game servers to return; at least 1.
            page_token: The previous page's `next_page_token`, or None for the first page.

        Raises:
            ValueError: `limit` is below 1.
            InvalidPageTokenError: `page_token` isn't one that `listGameServers` returned.

        Example:
            >>> page = management_api.listGameServers(False, 50, None)
        """
        if limit < 1:
            raise ValueError(f"limit must be at least 1, not {limit}")
        with self._lock:
            after_sequence: int = self._parsePageToken(page_token)
            records: list[_GameServerRecord] = [
                record
                for record in self._records.values()
                if record.sequence > after_sequence
                and (not has_open_place or len(record.player_ids) < record.max_player_count)
            ]
            page: list[_GameServerRecord] = records[:limit]
            is_last_page: bool = len(records) <= limit
            return models.GameServerList(
                game_servers=[self._toGameServer(record) for record in page],
                next_page_token=None if is_last_page else str(page[-1].sequence),
            )

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

    def deleteGameServer(self, game_server_id: uuid.UUID) -> None:
        """`deleteGameServer` stops a game server and forgets it and its players.

        Args:
            game_server_id: The game server to delete.

        Raises:
            NotFoundError: There is no such game server.

        Example:
            >>> management_api.deleteGameServer(game_server_id)
        """
        with self._lock:
            self._getRecord(game_server_id)
            self._game_server_launcher.stop(game_server_id)
            del self._records[game_server_id]

    def addPlayer(self, game_server_id: uuid.UUID, request: models.PlayerCreate) -> models.Player:
        """`addPlayer` adds a player to a game server that is starting or running and has room.

        Args:
            game_server_id: The game server to add the player to.
            request: The player's details; there are none yet.

        Raises:
            NotFoundError: There is no such game server.
            UnavailableError: The game server is full, stopped or failed.

        Example:
            >>> player = management_api.addPlayer(game_server_id, models.PlayerCreate())
        """
        with self._lock:
            record: _GameServerRecord = self._getRecord(game_server_id)
            state: models.GameServerState = self._game_server_launcher.get(game_server_id).state
            if state not in _JOINABLE_STATES:
                raise errors.UnavailableError(f"game server {game_server_id} is {state}")
            if len(record.player_ids) >= record.max_player_count:
                raise errors.UnavailableError(f"game server {game_server_id} is full")
            player_id: uuid.UUID = uuid.uuid4()
            record.player_ids.add(player_id)
            return models.Player(player_id=player_id, game_server_id=game_server_id)

    def getPlayer(self, game_server_id: uuid.UUID, player_id: uuid.UUID) -> models.Player:
        """`getPlayer` returns a player who was added to a game server and not removed.

        Args:
            game_server_id: The game server the player was added to.
            player_id: The player to return.

        Raises:
            NotFoundError: There is no such game server, or no such player in it.

        Example:
            >>> player = management_api.getPlayer(game_server_id, player_id)
        """
        with self._lock:
            self._checkPlayer(self._getRecord(game_server_id), player_id)
            return models.Player(player_id=player_id, game_server_id=game_server_id)

    def removePlayer(self, game_server_id: uuid.UUID, player_id: uuid.UUID) -> None:
        """`removePlayer` removes a player from a game server, freeing their place.

        Args:
            game_server_id: The game server the player was added to.
            player_id: The player to remove.

        Raises:
            NotFoundError: There is no such game server, or no such player in it.

        Example:
            >>> management_api.removePlayer(game_server_id, player_id)
        """
        with self._lock:
            record: _GameServerRecord = self._getRecord(game_server_id)
            self._checkPlayer(record, player_id)
            record.player_ids.remove(player_id)

    def _checkPlayer(self, record: _GameServerRecord, player_id: uuid.UUID) -> None:
        """`_checkPlayer` checks that a player was added to a game server and not removed.

        Raises:
            NotFoundError: The player isn't in the game server.

        Example:
            >>> self._checkPlayer(record, player_id)
        """
        if player_id not in record.player_ids:
            raise errors.NotFoundError(f"no player {player_id} in {record.game_server_id}")

    def _parsePageToken(self, page_token: str | None) -> int:
        """`_parsePageToken` returns the sequence number after which a page starts.

        Raises:
            InvalidPageTokenError: `page_token` isn't one that `listGameServers` returned.

        Example:
            >>> after_sequence = self._parsePageToken(page.next_page_token)
        """
        if page_token is None:
            return _SEQUENCE_BEFORE_FIRST
        if not page_token.isdecimal():
            raise errors.InvalidPageTokenError(f"invalid page token {page_token!r}")
        return int(page_token)

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
