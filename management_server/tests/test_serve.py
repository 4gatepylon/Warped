"""`test_serve.py` checks the `management-server` command: its arguments, serving and shutdown.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| arguments | valid | `test_runs_uvicorn_with_valid_arguments`, |
| | | `test_stops_game_servers_on_shutdown` |
| | `--port` out of range | `test_rejects_bad_arguments` |
| | `--game-server-ports` not FIRST-LAST | `test_rejects_bad_arguments` |
| | `--game-server-ports` with FIRST > LAST, or out of range | `test_rejects_bad_arguments` |
| | `--game-server-command` empty or missing | `test_rejects_bad_arguments` |
| shutdown signal | `SIGTERM`, `SIGINT` | `test_stops_game_servers_on_shutdown` |
"""

import collections.abc
import shlex
import signal
import subprocess
import sys
import time
import unittest.mock

import click.testing
import httpx2
import pytest
import stand_ins

from management_server import serve

_DEADLINE_S: float = 10.0
_POLL_INTERVAL_S: float = 0.05
_EXIT_TIMEOUT_S: float = 10.0
_USAGE_ERROR_EXIT_CODE: int = 2


def _wait_until(is_done: collections.abc.Callable[[], bool]) -> None:
    """`_wait_until` polls `is_done` until it returns true, or fails the test after a deadline.

    Example:
        >>> _wait_until(lambda: _is_answering(url))
    """
    deadline_s: float = time.monotonic() + _DEADLINE_S
    while time.monotonic() < deadline_s:
        if is_done():
            return
        time.sleep(_POLL_INTERVAL_S)
    pytest.fail(f"condition not met within {_DEADLINE_S} s")


def _is_answering(url: str) -> bool:
    """`_is_answering` checks whether an HTTP server answers a `GET` of `url`.

    Example:
        >>> _is_answering("http://127.0.0.1:8000/game-servers")
    """
    try:
        httpx2.get(url)
    except httpx2.ConnectError:
        return False
    return True


def test_runs_uvicorn_with_valid_arguments(monkeypatch: pytest.MonkeyPatch) -> None:
    """`test_runs_uvicorn_with_valid_arguments` checks that uvicorn serves on every interface."""
    uvicorn_run: unittest.mock.Mock = unittest.mock.Mock()
    monkeypatch.setattr(serve.uvicorn, "run", uvicorn_run)
    result: click.testing.Result = click.testing.CliRunner().invoke(
        serve.main, ["--port", "8123", "--game-server-command", "game-server"]
    )
    assert result.exit_code == 0, result.output
    uvicorn_run.assert_called_once_with(unittest.mock.ANY, host="0.0.0.0", port=8123)


@pytest.mark.parametrize(
    "arguments",
    [
        ["--port", "0", "--game-server-command", "game-server"],
        ["--game-server-ports", "20000", "--game-server-command", "game-server"],
        ["--game-server-ports", "20099-20000", "--game-server-command", "game-server"],
        ["--game-server-ports", "0-10", "--game-server-command", "game-server"],
        ["--game-server-command", ""],
        [],
    ],
)
def test_rejects_bad_arguments(arguments: list[str]) -> None:
    """`test_rejects_bad_arguments` checks that bad arguments are a usage error."""
    result: click.testing.Result = click.testing.CliRunner().invoke(serve.main, arguments)
    assert result.exit_code == _USAGE_ERROR_EXIT_CODE, result.output


@pytest.mark.parametrize("shutdown_signal", [signal.SIGTERM, signal.SIGINT])
def test_stops_game_servers_on_shutdown(shutdown_signal: signal.Signals) -> None:
    """`test_stops_game_servers_on_shutdown` runs a real server and launches a stand-in with it."""
    port: int = stand_ins.free_port()
    game_server_port: int = stand_ins.free_port()
    url: str = f"http://127.0.0.1:{port}"
    process: subprocess.Popen[bytes] = subprocess.Popen(
        [sys.executable, "-m", "management_server.serve", "--port", str(port)]
        + ["--game-server-ports", f"{game_server_port}-{game_server_port}"]
        + ["--game-server-command", shlex.join(stand_ins.HEALTHY_GAME_SERVER)]
    )
    try:
        _wait_until(lambda: _is_answering(f"{url}/game-servers"))
        game_server_id: str = httpx2.post(
            f"{url}/game-servers", json={"max_player_count": 2}
        ).json()["game_server_id"]
        _wait_until(
            lambda: httpx2.get(f"{url}/game-servers/{game_server_id}").json()["state"] == "running"
        )
        game_server: dict = httpx2.get(f"{url}/game-servers/{game_server_id}").json()
        assert game_server["address"] == {"host": "127.0.0.1", "port": game_server_port}
        assert _is_answering(f"http://127.0.0.1:{game_server_port}/health")
        process.send_signal(shutdown_signal)
        process.wait(timeout=_EXIT_TIMEOUT_S)
        assert not _is_answering(f"http://127.0.0.1:{game_server_port}/health")
    finally:
        process.kill()
        process.wait()
