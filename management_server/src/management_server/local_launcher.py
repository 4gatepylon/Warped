"""`local_launcher.py` runs each game server as a process on the management server's machine.

Interface:
    LocalProcessLauncher: A `Launcher`, as the local-process launcher section of
        `management_server/internal_api.md` describes.

Example:
    >>> game_server_launcher = LocalProcessLauncher(host, range(20000, 20100), command, url)

Must know:
    Limitations:
        - It isn't thread-safe; `Management` makes one call at a time.
    Gotchas:
        - Nothing checks game servers in the background: `get` checks when it is called.
"""

import collections.abc
import dataclasses
import subprocess
import uuid

from management_server import errors, models

STOP_TIMEOUT_S: float = 5.0


@dataclasses.dataclass
class _GameServerProcess:
    """`_GameServerProcess` is what the launcher knows about one game server's process.

    Attributes:
        process: The process, running or exited.
        port: The port the game server listens on.
        state: The game server's state when it was last checked.
        failure_reason: Why it failed, if it did.

    Example:
        >>> game_server_process = _GameServerProcess(process, 20000)
    """

    process: subprocess.Popen[bytes]
    port: int
    state: models.GameServerState = models.GameServerState.STARTING
    failure_reason: str | None = None


class LocalProcessLauncher:
    """`LocalProcessLauncher` runs game servers as local processes with the contract's flags.

    Args:
        advertised_host: The host clients connect to, such as this machine's LAN IP.
        ports: The ports to pick from, one per game server.
        command: The game server command, without the contract's flags.
        management_server_url: Where game servers check players.

    Example:
        >>> game_server_launcher = LocalProcessLauncher(host, ports, command, url)
    """

    def __init__(
        self,
        advertised_host: str,
        ports: range,
        command: collections.abc.Sequence[str],
        management_server_url: str,
    ) -> None:
        self._advertised_host: str = advertised_host
        self._ports: range = ports
        self._command: collections.abc.Sequence[str] = command
        self._management_server_url: str = management_server_url
        self._game_server_processes: dict[uuid.UUID, _GameServerProcess] = {}

    def launch(
        self, game_server_id: uuid.UUID, config: models.GameServerConfig, max_player_count: int
    ) -> None:
        """`launch` implements `Launcher.launch` by running `command` on an unused port.

        Raises:
            RuntimeError: Every port in `ports` has a running game server.

        Example:
            >>> game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
        """
        port: int = self._pickPort()
        process: subprocess.Popen[bytes] = subprocess.Popen(
            [
                *self._command,
                *("--game-server-id", str(game_server_id), "--config", config.model_dump_json()),
                *("--max-player-count", str(max_player_count)),
                *("--management-server-url", self._management_server_url, "--port", str(port)),
            ]
        )
        self._game_server_processes[game_server_id] = _GameServerProcess(process, port)

    def get(self, game_server_id: uuid.UUID) -> models.LaunchedGameServer:
        """`get` implements `Launcher.get`, after checking the game server's process.

        Example:
            >>> game_server_launcher.get(game_server_id).state
        """
        game_server_process: _GameServerProcess = self._getProcess(game_server_id)
        self._refresh(game_server_process)
        return models.LaunchedGameServer(
            game_server_id=game_server_id,
            state=game_server_process.state,
            address=None,
            failure_reason=game_server_process.failure_reason,
        )

    def stop(self, game_server_id: uuid.UUID) -> None:
        """`stop` implements `Launcher.stop`: `SIGTERM`, then `SIGKILL` after `STOP_TIMEOUT_S`.

        Example:
            >>> game_server_launcher.stop(game_server_id)
        """
        game_server_process: _GameServerProcess = self._getProcess(game_server_id)
        if game_server_process.state is models.GameServerState.STOPPED:
            return
        game_server_process.process.terminate()
        try:
            game_server_process.process.wait(timeout=STOP_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            game_server_process.process.kill()
            game_server_process.process.wait()
        game_server_process.state = models.GameServerState.STOPPED

    def _getProcess(self, game_server_id: uuid.UUID) -> _GameServerProcess:
        """`_getProcess` returns a game server's process.

        Raises:
            NotFoundError: This launcher never started the game server.

        Example:
            >>> game_server_process = self._getProcess(game_server_id)
        """
        if game_server_id not in self._game_server_processes:
            raise errors.NotFoundError(f"no game server {game_server_id} was launched")
        return self._game_server_processes[game_server_id]

    def _pickPort(self) -> int:
        """`_pickPort` returns a port in `ports` that no running game server listens on.

        Raises:
            RuntimeError: Every port in `ports` has a running game server.

        Example:
            >>> port = self._pickPort()
        """
        used_ports: set[int] = {
            game_server_process.port
            for game_server_process in self._game_server_processes.values()
            if game_server_process.process.poll() is None
        }
        for port in self._ports:
            if port not in used_ports:
                return port
        raise RuntimeError(f"no free port in {self._ports}")

    def _refresh(self, game_server_process: _GameServerProcess) -> None:
        """`_refresh` marks a starting game server whose process exited as failed.

        Example:
            >>> self._refresh(game_server_process)
        """
        if game_server_process.state is not models.GameServerState.STARTING:
            return
        exit_code: int | None = game_server_process.process.poll()
        if exit_code is not None:
            game_server_process.state = models.GameServerState.FAILED
            game_server_process.failure_reason = f"exited with code {exit_code}"
