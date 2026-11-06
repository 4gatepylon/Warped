"""`test_local_launcher.py` checks `LocalProcessLauncher` with real processes.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| process | keeps running | `test_starts_and_stops_game_server` |
| | exits on its own | `test_fails_when_process_exits` |
| game server ID | launched | `test_starts_and_stops_game_server` |
| | never launched | `test_fails_when_process_exits` |
"""

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
