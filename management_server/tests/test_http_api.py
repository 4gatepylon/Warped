"""`test_http_api.py` checks that the HTTP routes call the internal management API as specified.

Partitionings of requests, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| game server route | `POST`, `GET` list, `GET`, `DELETE` | `test_game_server_routes` |
| player route | `POST`, `GET`, `DELETE` | `test_player_routes` |
| outcome | success (`200`, `201`, `204`) | `test_game_server_routes`, `test_player_routes` |
| | unknown ID (`404`) | `test_game_server_routes`, `test_player_routes` |
| | full game server (`409`) | `test_player_routes` |
| | bad request (`422`) | `test_rejects_bad_requests` |
"""

import unittest.mock
import uuid

import fastapi.testclient
import httpx2
import pytest

from management_server import http_api, launcher, management, models


def _make_client() -> fastapi.testclient.TestClient:
    """`_make_client` returns a test client for an app whose launcher runs every game server.

    Example:
        >>> client = _make_client()
    """
    game_server_launcher: unittest.mock.Mock = unittest.mock.create_autospec(
        launcher.Launcher, instance=True
    )
    game_server_launcher.get.side_effect = lambda game_server_id: models.LaunchedGameServer(
        game_server_id=game_server_id,
        state=models.GameServerState.RUNNING,
        address=models.Address(host="10.0.0.5", port=20000),
        failure_reason=None,
    )
    return fastapi.testclient.TestClient(
        http_api.create_app(management.Management(game_server_launcher))
    )


def test_game_server_routes() -> None:
    """`test_game_server_routes` checks creating, listing, getting and deleting a game server."""
    client: fastapi.testclient.TestClient = _make_client()
    created: dict = client.post("/game-servers", json={"max_player_count": 4}).json()
    path: str = f"/game-servers/{created['game_server_id']}"
    assert created["address"] == {"host": "10.0.0.5", "port": 20000}
    assert client.get("/game-servers").json() == {
        "game_servers": [created],
        "next_page_token": None,
    }
    assert client.get(path).json() == created
    assert client.delete(path).status_code == fastapi.status.HTTP_204_NO_CONTENT
    assert client.get(path).status_code == fastapi.status.HTTP_404_NOT_FOUND


def test_player_routes() -> None:
    """`test_player_routes` checks adding, getting and removing a player, and a full game server."""
    client: fastapi.testclient.TestClient = _make_client()
    game_server_id: str = client.post("/game-servers", json={"max_player_count": 1}).json()[
        "game_server_id"
    ]
    players_path: str = f"/game-servers/{game_server_id}/players"
    added: httpx2.Response = client.post(players_path, json={})
    player_path: str = f"{players_path}/{added.json()['player_id']}"
    assert added.status_code == fastapi.status.HTTP_201_CREATED
    assert client.post(players_path, json={}).status_code == fastapi.status.HTTP_409_CONFLICT
    assert client.get(player_path).json() == added.json()
    assert client.delete(player_path).status_code == fastapi.status.HTTP_204_NO_CONTENT
    assert client.get(player_path).status_code == fastapi.status.HTTP_404_NOT_FOUND
    unknown_players_path: str = f"/game-servers/{uuid.uuid4()}/players"
    assert client.post(unknown_players_path, json={}).status_code == (
        fastapi.status.HTTP_404_NOT_FOUND
    )


@pytest.mark.parametrize(
    ("method", "path", "body"),
    [
        ("POST", "/game-servers", {"max_player_count": 0}),
        ("POST", "/game-servers", {"max_player_count": 4, "map": "cube"}),
        ("GET", "/game-servers?limit=0", None),
        ("GET", "/game-servers?limit=101", None),
        ("GET", "/game-servers?page_token=not-a-token", None),
        ("GET", "/game-servers/not-a-uuid", None),
    ],
)
def test_rejects_bad_requests(method: str, path: str, body: dict | None) -> None:
    """`test_rejects_bad_requests` checks that a request the spec doesn't allow gets `422`."""
    response: httpx2.Response = _make_client().request(method, path, json=body)
    assert response.status_code == fastapi.status.HTTP_422_UNPROCESSABLE_CONTENT
