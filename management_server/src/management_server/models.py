"""`models.py` defines the data models of the management server's HTTP and internal APIs.

Interface:
    The models of `common/protocol/management_server_api.md` and
    `management_server/internal_api.md`, with the same names and fields. Every model rejects
    fields it doesn't define.

Example:
    >>> request = GameServerCreate(max_player_count=4)
"""

import datetime
import enum
import uuid

import pydantic


class GameServerState(enum.StrEnum):
    """`GameServerState` is a game server's state, as its launcher reports it.

    Example:
        >>> GameServerState.RUNNING
    """

    STARTING = "starting"
    RUNNING = "running"
    STOPPED = "stopped"
    FAILED = "failed"


class _Model(pydantic.BaseModel):
    """`_Model` is the base of every model, and rejects fields that the model doesn't define.

    Example:
        >>> class Thing(_Model): ...
    """

    model_config = pydantic.ConfigDict(extra="forbid")


class GameServerConfig(_Model):
    """`GameServerConfig` configures a game server, and has no fields yet.

    Example:
        >>> config = GameServerConfig()
    """


class PlayerCreate(_Model):
    """`PlayerCreate` is the body of a request to add a player, and has no fields yet.

    Example:
        >>> request = PlayerCreate()
    """


class Address(_Model):
    """`Address` is where clients connect to a game server.

    Attributes:
        host: Host name or IP address.
        port: Port number.

    Example:
        >>> address = Address(host="10.0.0.5", port=20000)
    """

    host: str
    port: int


class GameServerCreate(_Model):
    """`GameServerCreate` is the body of a request to create a game server.

    Attributes:
        config: The game server's configuration; empty by default.
        max_player_count: How many players the game server may have at once; at least 1.

    Example:
        >>> request = GameServerCreate(max_player_count=4)
    """

    config: GameServerConfig = pydantic.Field(default_factory=GameServerConfig)
    max_player_count: int = pydantic.Field(ge=1)


class GameServer(_Model):
    """`GameServer` is a game server as clients see it; it never includes player IDs.

    Attributes:
        As specified for `GameServer` in `common/protocol/management_server_api.md`.

    Example:
        >>> game_server.state is GameServerState.RUNNING
    """

    game_server_id: uuid.UUID
    state: GameServerState
    address: Address | None
    config: GameServerConfig
    max_player_count: int
    player_count: int
    created_at: datetime.datetime


class GameServerList(_Model):
    """`GameServerList` is one page of game servers, oldest first.

    Attributes:
        As specified for `GameServerList` in `common/protocol/management_server_api.md`.

    Example:
        >>> page = GameServerList(game_servers=[], next_page_token=None)
    """

    game_servers: list[GameServer]
    next_page_token: str | None


class Player(_Model):
    """`Player` is a player added to a game server.

    Attributes:
        player_id: What the player's client presents when it connects.
        game_server_id: The game server the player was added to.

    Example:
        >>> player = Player(player_id=uuid.uuid4(), game_server_id=game_server_id)
    """

    player_id: uuid.UUID
    game_server_id: uuid.UUID


class LaunchedGameServer(_Model):
    """`LaunchedGameServer` is a game server as its launcher sees it.

    Attributes:
        As specified for `LaunchedGameServer` in `management_server/internal_api.md`.

    Example:
        >>> launched_game_server = launcher.get(game_server_id)
    """

    game_server_id: uuid.UUID
    state: GameServerState
    address: Address | None
    failure_reason: str | None
