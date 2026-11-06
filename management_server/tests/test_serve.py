"""`test_serve.py` checks the `management-server` command's arguments.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| arguments | valid | `test_runs_uvicorn_with_valid_arguments` |
| | `--port` out of range | `test_rejects_bad_arguments` |
| | `--game-server-ports` not FIRST-LAST | `test_rejects_bad_arguments` |
| | `--game-server-ports` with FIRST > LAST, or out of range | `test_rejects_bad_arguments` |
| | `--game-server-command` empty or missing | `test_rejects_bad_arguments` |
"""

import unittest.mock

import click.testing
import pytest

from management_server import serve

_USAGE_ERROR_EXIT_CODE: int = 2


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
