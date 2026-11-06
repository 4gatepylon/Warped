"""`test_local_launcher.py` checks `LocalProcessLauncher` with real processes.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| process | answers `GET /health` | `test_runs_healthy_game_server` |
| | keeps running, never answers | `test_starts_and_stops_game_server`, |
| | | `test_fails_when_never_healthy` |
| | exits on its own | `test_fails_when_process_exits` |
| time without answering | within the health timeout | `test_starts_and_stops_game_server` |
| | beyond it | `test_fails_when_never_healthy` |
| game server ID | launched | `test_starts_and_stops_game_server` |
| | never launched | `test_fails_when_process_exits` |
| ports | some free | `test_runs_healthy_game_server` |
| | all used by other programs | `test_fails_without_free_port` |
| listed game servers | not stopped | `test_lists_until_stopped` |
| | stopped | `test_lists_until_stopped` |
"""

import socket
import sys
import time
import uuid

import pytest

from management_server import errors, local_launcher, models

_PORTS: range = range(20000, 20010)
_STATE_DEADLINE_S: float = 10.0
_POLL_INTERVAL_S: float = 0.05
_SLEEPING_GAME_SERVER: list[str] = [sys.executable, "-c", "import time; time.sleep(60)"]
_EXITING_GAME_SERVER: list[str] = [sys.executable, "-c", "raise SystemExit(3)"]
_HEALTHY_GAME_SERVER: list[str] = [
    sys.executable,
    "-c",
    "import http.server, sys\n"
    "class Handler(http.server.BaseHTTPRequestHandler):\n"
    "    def do_GET(self): self.send_response(200 if self.path == '/health' else 404); "
    "self.end_headers()\n"
    "port = int(sys.argv[sys.argv.index('--port') + 1])\n"
    "http.server.HTTPServer(('0.0.0.0', port), Handler).serve_forever()\n",
]
_SHORT_HEALTH_TIMEOUT_S: float = 0.3


def _free_port() -> int:
    """`_free_port` returns a TCP port that nothing on this machine listens on right now.

    Example:
        >>> port = _free_port()
    """
    with socket.socket() as probe_socket:
        probe_socket.bind(("127.0.0.1", 0))
        return probe_socket.getsockname()[1]


def _wait_for_state(
    game_server_launcher: local_launcher.LocalProcessLauncher,
    game_server_id: uuid.UUID,
    state: models.GameServerState,
) -> models.LaunchedGameServer:
    """`_wait_for_state` polls `get` until the game server is in `state`, or fails the test.

    Example:
        >>> _wait_for_state(game_server_launcher, game_server_id, models.GameServerState.FAILED)
    """
    deadline_s: float = time.monotonic() + _STATE_DEADLINE_S
    while time.monotonic() < deadline_s:
        launched_game_server: models.LaunchedGameServer = game_server_launcher.get(game_server_id)
        if launched_game_server.state is state:
            return launched_game_server
        time.sleep(_POLL_INTERVAL_S)
    pytest.fail(f"game server {game_server_id} didn't reach {state} in {_STATE_DEADLINE_S} s")


def test_starts_and_stops_game_server() -> None:
    """`test_starts_and_stops_game_server` checks that a game server starts, then stops once."""
    game_server_launcher: local_launcher.LocalProcessLauncher = local_launcher.LocalProcessLauncher(
        "127.0.0.1", _PORTS, _SLEEPING_GAME_SERVER, "http://127.0.0.1:8000"
    )
    game_server_id: uuid.UUID = uuid.uuid4()
    game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
    assert game_server_launcher.get(game_server_id).state is models.GameServerState.STARTING
    game_server_launcher.stop(game_server_id)
    game_server_launcher.stop(game_server_id)
    assert game_server_launcher.get(game_server_id).state is models.GameServerState.STOPPED


def test_fails_when_process_exits() -> None:
    """`test_fails_when_process_exits` checks that a game server that exits has failed."""
    game_server_launcher: local_launcher.LocalProcessLauncher = local_launcher.LocalProcessLauncher(
        "127.0.0.1", _PORTS, _EXITING_GAME_SERVER, "http://127.0.0.1:8000"
    )
    game_server_id: uuid.UUID = uuid.uuid4()
    game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
    failed: models.LaunchedGameServer = _wait_for_state(
        game_server_launcher, game_server_id, models.GameServerState.FAILED
    )
    assert (failed.address, failed.failure_reason) == (None, "exited with code 3")
    with pytest.raises(errors.NotFoundError):
        game_server_launcher.get(uuid.uuid4())
    with pytest.raises(errors.NotFoundError):
        game_server_launcher.stop(uuid.uuid4())


def test_runs_healthy_game_server() -> None:
    """`test_runs_healthy_game_server` checks that a healthy game server runs at its address."""
    port: int = _free_port()
    game_server_launcher: local_launcher.LocalProcessLauncher = local_launcher.LocalProcessLauncher(
        "10.0.0.5", range(port, port + 1), _HEALTHY_GAME_SERVER, "http://127.0.0.1:8000"
    )
    game_server_id: uuid.UUID = uuid.uuid4()
    game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
    running: models.LaunchedGameServer = _wait_for_state(
        game_server_launcher, game_server_id, models.GameServerState.RUNNING
    )
    game_server_launcher.stop(game_server_id)
    assert running.address == models.Address(host="10.0.0.5", port=port)


def test_fails_when_never_healthy() -> None:
    """`test_fails_when_never_healthy` checks that a game server that never answers fails."""
    game_server_launcher: local_launcher.LocalProcessLauncher = local_launcher.LocalProcessLauncher(
        "127.0.0.1",
        _PORTS,
        _SLEEPING_GAME_SERVER,
        "http://127.0.0.1:8000",
        health_timeout_s=_SHORT_HEALTH_TIMEOUT_S,
    )
    game_server_id: uuid.UUID = uuid.uuid4()
    game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
    failed: models.LaunchedGameServer = _wait_for_state(
        game_server_launcher, game_server_id, models.GameServerState.FAILED
    )
    assert failed.failure_reason == "didn't answer GET /health within 0.3 s"


def test_fails_without_free_port() -> None:
    """`test_fails_without_free_port` checks that a port another program uses isn't picked."""
    with socket.socket() as other_program_socket:
        other_program_socket.bind(("0.0.0.0", 0))
        other_program_socket.listen()
        port: int = other_program_socket.getsockname()[1]
        game_server_launcher: local_launcher.LocalProcessLauncher = (
            local_launcher.LocalProcessLauncher(
                "127.0.0.1", range(port, port + 1), _SLEEPING_GAME_SERVER, "http://127.0.0.1:8000"
            )
        )
        with pytest.raises(RuntimeError):
            game_server_launcher.launch(uuid.uuid4(), models.GameServerConfig(), 4)


def test_lists_until_stopped() -> None:
    """`test_lists_until_stopped` checks that `list` has the game servers that aren't stopped."""
    game_server_launcher: local_launcher.LocalProcessLauncher = local_launcher.LocalProcessLauncher(
        "127.0.0.1", _PORTS, _SLEEPING_GAME_SERVER, "http://127.0.0.1:8000"
    )
    game_server_id: uuid.UUID = uuid.uuid4()
    game_server_launcher.launch(game_server_id, models.GameServerConfig(), 4)
    listed_ids: list[uuid.UUID] = [
        launched_game_server.game_server_id for launched_game_server in game_server_launcher.list()
    ]
    game_server_launcher.stop(game_server_id)
    assert (listed_ids, game_server_launcher.list()) == ([game_server_id], [])
