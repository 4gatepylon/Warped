"""`test_management.py` checks the internal management API against a mock launcher.

Partitionings, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| game server ID | created | `test_creates_and_gets_game_server` |
| | never created | `test_get_fails_for_unknown_id` |
| launcher's state | starting | `test_creates_and_gets_game_server` |
| | running | `test_reports_launcher_address` |
| deleted game server ID | created | `test_deletes_game_server`, `test_lists_in_pages` |
| | never created | `test_delete_fails_for_unknown_id` |
| game servers listed | at most `limit` | `test_lists_open_game_servers` |
| | more than `limit` | `test_lists_in_pages` |
| `page_token` | none | `test_lists_in_pages`, `test_lists_open_game_servers` |
| | the previous page's | `test_lists_in_pages` |
| | not one it returned | `test_list_rejects_bad_arguments` |
| `limit` | below 1 | `test_list_rejects_bad_arguments` |
| | at least 1 | `test_lists_in_pages`, `test_lists_open_game_servers` |
| `has_open_place` | false | `test_lists_in_pages` |
| | true | `test_lists_open_game_servers` |
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


def test_deletes_game_server() -> None:
    """`test_deletes_game_server` checks that deleting stops the game server and forgets it."""
    game_server_launcher: unittest.mock.Mock = _make_launcher(models.GameServerState.RUNNING, None)
    management_api: management.Management = management.Management(game_server_launcher)
    created: models.GameServer = management_api.createGameServer(
        models.GameServerCreate(max_player_count=4)
    )
    management_api.deleteGameServer(created.game_server_id)
    game_server_launcher.stop.assert_called_once_with(created.game_server_id)
    with pytest.raises(errors.NotFoundError):
        management_api.getGameServer(created.game_server_id)


def test_delete_fails_for_unknown_id() -> None:
    """`test_delete_fails_for_unknown_id` checks that deleting an unknown game server stops none."""
    game_server_launcher: unittest.mock.Mock = _make_launcher(models.GameServerState.RUNNING, None)
    management_api: management.Management = management.Management(game_server_launcher)
    with pytest.raises(errors.NotFoundError):
        management_api.deleteGameServer(uuid.uuid4())
    game_server_launcher.stop.assert_not_called()


def test_lists_in_pages() -> None:
    """`test_lists_in_pages` checks paging, even when a page's last game server is deleted."""
    management_api: management.Management = management.Management(
        _make_launcher(models.GameServerState.RUNNING, _ADDRESS)
    )
    created_ids: list[uuid.UUID] = [
        management_api.createGameServer(models.GameServerCreate(max_player_count=4)).game_server_id
        for _ in range(3)
    ]
    first_page: models.GameServerList = management_api.listGameServers(False, 2, None)
    management_api.deleteGameServer(created_ids[1])
    second_page: models.GameServerList = management_api.listGameServers(
        False, 2, first_page.next_page_token
    )
    assert [game_server.game_server_id for game_server in first_page.game_servers] == created_ids[
        :2
    ]
    assert [game_server.game_server_id for game_server in second_page.game_servers] == created_ids[
        2:
    ]
    assert second_page.next_page_token is None


def test_lists_open_game_servers() -> None:
    """`test_lists_open_game_servers` checks that game servers with room fit on one page."""
    management_api: management.Management = management.Management(
        _make_launcher(models.GameServerState.RUNNING, _ADDRESS)
    )
    created: models.GameServer = management_api.createGameServer(
        models.GameServerCreate(max_player_count=4)
    )
    page: models.GameServerList = management_api.listGameServers(True, 50, None)
    assert (page.game_servers, page.next_page_token) == ([created], None)


def test_list_rejects_bad_arguments() -> None:
    """`test_list_rejects_bad_arguments` checks that a bad `limit` or `page_token` is an error."""
    management_api: management.Management = management.Management(
        _make_launcher(models.GameServerState.RUNNING, _ADDRESS)
    )
    with pytest.raises(ValueError):
        management_api.listGameServers(False, 0, None)
    with pytest.raises(errors.InvalidPageTokenError):
        management_api.listGameServers(False, 50, "not-a-token")
