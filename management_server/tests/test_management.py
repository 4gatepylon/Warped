"""`test_management.py` checks the internal management API against a mock launcher.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| game server ID | created | `test_creates_and_gets_game_server` |
| | never created | `test_get_fails_for_unknown_id` |
| launcher's state | starting | `test_creates_and_gets_game_server` |
| | running | `test_reports_launcher_address` |
"""

import unittest.mock
import uuid

import pytest

from management_server import errors, launcher, management, models

_ADDRESS: models.Address = models.Address(host="10.0.0.5", port=20000)


def _make_launcher(
    state: models.GameServerState, address: models.Address | None
) -> unittest.mock.Mock:
    """`_make_launcher` returns a mock launcher that reports every game server in one state.

    Example:
        >>> game_server_launcher = _make_launcher(models.GameServerState.STARTING, None)
    """
    game_server_launcher: unittest.mock.Mock = unittest.mock.create_autospec(
        launcher.Launcher, instance=True
    )
    game_server_launcher.get.side_effect = lambda game_server_id: models.LaunchedGameServer(
        game_server_id=game_server_id, state=state, address=address, failure_reason=None
    )
    return game_server_launcher


def test_creates_and_gets_game_server() -> None:
    """`test_creates_and_gets_game_server` checks that a new game server is launched and found."""
    game_server_launcher: unittest.mock.Mock = _make_launcher(models.GameServerState.STARTING, None)
    management_api: management.Management = management.Management(game_server_launcher)
    created: models.GameServer = management_api.createGameServer(
        models.GameServerCreate(max_player_count=4)
    )
    game_server_launcher.launch.assert_called_once_with(
        created.game_server_id, models.GameServerConfig(), 4
    )
    assert (created.state, created.address, created.player_count) == ("starting", None, 0)
    assert management_api.getGameServer(created.game_server_id) == created


def test_reports_launcher_address() -> None:
    """`test_reports_launcher_address` checks that a running game server has its address."""
    management_api: management.Management = management.Management(
        _make_launcher(models.GameServerState.RUNNING, _ADDRESS)
    )
    created: models.GameServer = management_api.createGameServer(
        models.GameServerCreate(max_player_count=4)
    )
    assert management_api.getGameServer(created.game_server_id).address == _ADDRESS


def test_get_fails_for_unknown_id() -> None:
    """`test_get_fails_for_unknown_id` checks that an unknown game server isn't found."""
    management_api: management.Management = management.Management(
        _make_launcher(models.GameServerState.RUNNING, _ADDRESS)
    )
    with pytest.raises(errors.NotFoundError):
        management_api.getGameServer(uuid.uuid4())
