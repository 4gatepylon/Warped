"""`launcher.py` defines `Launcher`, the interface to wherever game servers run.

Interface:
    Launcher: The launcher operations of `management_server/internal_api.md`.

Example:
    >>> game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
"""

import typing
import uuid

from management_server import models


class Launcher(typing.Protocol):
    """`Launcher` starts, checks and stops game servers, wherever they run.

    Example:
        >>> game_server_launcher.get(game_server_id).state
    """

    def launch(
        self, game_server_id: uuid.UUID, config: models.GameServerConfig, max_player_count: int
    ) -> None:
        """`launch` starts a game server and returns without waiting for it to run.

        Args:
            game_server_id: The ID to start the game server with.
            config: The game server's configuration.
            max_player_count: How many players the game server may have at once.

        Example:
            >>> game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
        """

    def get(self, game_server_id: uuid.UUID) -> models.LaunchedGameServer:
        """`get` returns a game server's state and, once it is running, its address.

        Args:
            game_server_id: The game server to check.

        Raises:
            NotFoundError: This launcher never started the game server.

        Example:
            >>> game_server_launcher.get(game_server_id).address
        """
