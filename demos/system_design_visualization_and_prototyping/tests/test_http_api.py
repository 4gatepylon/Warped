"""`test_http_api.py` checks the routes of `system_prototype.http_api` with FastAPI's test client.

Partitionings and the tests that cover each part:

| Partitioning | Part | Test |
| --- | --- | --- |
| route family | the backend's (`/api/`) | `test_status_and_settings_are_served` |
| | the simulation's (`/api/simulation/`) | `test_simulation_routes_move_the_world` |
| | the pages | `test_pages_are_served` |
| outcome | success | all three above |
| | something not found (404) | `test_errors_have_their_status` |
| | a broken rule (409) | `test_errors_have_their_status` |
| | a body that doesn't fit (422) | `test_errors_have_their_status` |
"""

from pathlib import Path

from fastapi.testclient import TestClient

from system_prototype.clock import SimClock
from system_prototype.http_api import make_app
from system_prototype.store import open_store
from system_prototype.world import World


def _make_client(tmp_path: Path) -> TestClient:
    """`_make_client` returns a client for a new world whose only page says "front page".

    Example:
        >>> client = _make_client(tmp_path)
    """
    (tmp_path / "index.html").write_text("front page")
    world: World = World(open_store(Path(":memory:")), SimClock())
    return TestClient(make_app(world, tmp_path))


def test_status_and_settings_are_served(tmp_path: Path) -> None:
    """`test_status_and_settings_are_served` checks the status, and a setting change that shows in
    it."""
    with _make_client(tmp_path) as client:
        assert client.get("/api/status").json()["caps"]["max_match_count"] == 100
        changed = client.put("/api/settings/max_match_count", json={"value": 50})
        assert changed.status_code == 200
        assert {"max_match_count": 50.0}.items() <= {
            row["name"]: row["value"] for row in client.get("/api/settings").json()
        }.items()
        assert client.get("/api/status").json()["caps"]["max_match_count"] == 50


def test_simulation_routes_move_the_world(tmp_path: Path) -> None:
    """`test_simulation_routes_move_the_world` checks a skip, the system view and a failed
    machine."""
    with _make_client(tmp_path) as client:
        skipped = client.post("/api/simulation/clock/skip", json={"seconds": 120})
        assert skipped.status_code == 200
        assert skipped.json()["now_s"] >= 120
        system: dict = client.get("/api/simulation/system").json()
        assert [m["state"] for m in system["machines"]] == ["ready", "ready"]
        machine_id: str = system["machines"][0]["machine_id"]
        assert client.post(f"/api/simulation/machines/{machine_id}/fail").status_code == 200


def test_pages_are_served(tmp_path: Path) -> None:
    """`test_pages_are_served` checks that the front page is at `/` and isn't cached."""
    with _make_client(tmp_path) as client:
        front_page = client.get("/")
        assert front_page.text == "front page"
        assert front_page.headers["Cache-Control"] == "no-store"


def test_errors_have_their_status(tmp_path: Path) -> None:
    """`test_errors_have_their_status` checks 404, 409 and 422, each with a `detail`."""
    with _make_client(tmp_path) as client:
        unknown = client.put("/api/settings/max_matches", json={"value": 5})
        negative = client.put("/api/settings/max_match_count", json={"value": -1})
        misfit = client.post("/api/simulation/clock/skip", json={"minutes": 5})
        missing = client.post("/api/simulation/game-servers/gs-99/fail")
        assert (unknown.status_code, negative.status_code) == (404, 409)
        assert (misfit.status_code, missing.status_code) == (422, 404)
        assert "max_matches" in unknown.json()["detail"]
        assert "negative" in negative.json()["detail"]
