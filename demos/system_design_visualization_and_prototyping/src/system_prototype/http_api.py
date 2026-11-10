"""`http_api.py` serves the prototype over HTTP: the backend's API, the simulation and the pages.

Interface:
    make_app: Builds the FastAPI app around a `World`.
    Routes under `/api/`: what the real backend would offer (`/docs` lists them with their schemas);
        the account and purchase ones are in `account_routes.py`.
    Routes under `/fake-google/` and `/fake-stripe/`: the stand-ins' own (`provider_routes.py`).
    Routes under `/api/simulation/`: the simulation controls and the system view, which the real
        system doesn't have.
    Every other path: the web pages in `web/`.

Example:
    >>> app = make_app(World(open_store(Path(":memory:")), SimClock()), Path("web"))
"""

import asyncio
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, Response
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel
from starlette.middleware.base import RequestResponseEndpoint

from .account_routes import add_account_routes
from .provider_routes import add_provider_routes
from .rules import AuthError, NotFoundError, RuleError
from .world import World

TICK_INTERVAL_S: float = 0.25
HTTP_UNAUTHORIZED: int = 401
HTTP_NOT_FOUND: int = 404
HTTP_CONFLICT: int = 409


class SettingChange(BaseModel):
    """`SettingChange` is the body of a request that sets one setting.

    Attributes:
        value: The setting's new value, at least 0.

    Example:
        >>> SettingChange(value=50)
    """

    value: float


class ClockSkip(BaseModel):
    """`ClockSkip` is the body of a request that skips the clock forward.

    Attributes:
        seconds: How far to skip, more than 0.

    Example:
        >>> ClockSkip(seconds=3600)
    """

    seconds: float


async def _tick_forever(world: World) -> None:
    """`_tick_forever` moves the world forward four times a second until it is cancelled.

    Example:
        >>> task = asyncio.create_task(_tick_forever(world))
    """
    while True:
        world.tick()
        await asyncio.sleep(TICK_INTERVAL_S)


def make_app(world: World, web_folder: Path) -> FastAPI:
    """`make_app` builds the app that serves `world` and the pages in `web_folder`.

    Args:
        world: The simulated system.
        web_folder: The folder of static pages, served at `/`.

    Returns:
        The app. While it runs, it ticks `world` four times a second.

    Example:
        >>> app = make_app(world, Path("web"))
    """

    @asynccontextmanager
    async def run_world(app: FastAPI) -> AsyncIterator[None]:
        """`run_world` keeps the world ticking for as long as the app runs.

        Example:
            >>> FastAPI(lifespan=run_world)
        """
        tick_task: asyncio.Task = asyncio.create_task(_tick_forever(world))
        yield
        tick_task.cancel()

    app: FastAPI = FastAPI(title="Warped system prototype", lifespan=run_world)

    @app.middleware("http")
    async def forbid_caching(request: Request, call_next: RequestResponseEndpoint) -> Response:
        """`forbid_caching` tells the browser to keep nothing, so an edited page shows on reload.

        Example:
            >>> GET /shared/look.css answers with "Cache-Control: no-store".
        """
        response: Response = await call_next(request)
        response.headers["Cache-Control"] = "no-store"
        return response

    @app.exception_handler(RuleError)
    async def answer_rule_error(request: Request, error: RuleError) -> JSONResponse:
        """`answer_rule_error` turns a broken rule into a 409 with the rule in `detail`.

        Example:
            >>> PUT /api/settings/max_match_count {"value": -1} gives 409.
        """
        return JSONResponse({"detail": str(error)}, status_code=HTTP_CONFLICT)

    @app.exception_handler(NotFoundError)
    async def answer_not_found(request: Request, error: NotFoundError) -> JSONResponse:
        """`answer_not_found` turns a missing thing into a 404 with its name in `detail`.

        Example:
            >>> PUT /api/settings/nothing {"value": 1} gives 404.
        """
        return JSONResponse({"detail": str(error)}, status_code=HTTP_NOT_FOUND)

    @app.exception_handler(AuthError)
    async def answer_auth_error(request: Request, error: AuthError) -> JSONResponse:
        """`answer_auth_error` turns refused credentials into a 401 with the reason in `detail`.

        Example:
            >>> GET /api/me without a bearer token gives 401.
        """
        return JSONResponse({"detail": str(error)}, status_code=HTTP_UNAUTHORIZED)

    @app.get("/api/status")
    async def get_status() -> dict:
        """The public status page's data: the caps and their use, and each region's machines.

        Example:
            >>> GET /api/status
        """
        return world.describeStatus()

    @app.get("/api/settings")
    async def list_settings() -> list[dict]:
        """Every tunable setting, for the superadmin console.

        Example:
            >>> GET /api/settings
        """
        return world.listSettings()

    @app.put("/api/settings/{name}")
    async def put_setting(name: str, change: SettingChange) -> list[dict]:
        """Sets one setting and returns them all. 404 for an unknown name, 409 for a negative value.

        Example:
            >>> PUT /api/settings/max_match_count {"value": 50}
        """
        world.changeSetting(name, change.value)
        return world.listSettings()

    @app.get("/api/simulation/system")
    async def get_system() -> dict:
        """Simulation only: everything the system view draws.

        Example:
            >>> GET /api/simulation/system
        """
        return world.describeSystem()

    @app.post("/api/simulation/clock/skip")
    async def skip_clock(skip: ClockSkip) -> dict:
        """Simulation only: skips the clock forward and lets the system catch up.

        Example:
            >>> POST /api/simulation/clock/skip {"seconds": 3600}
        """
        world.skipClock(skip.seconds)
        return world.describeStatus()

    @app.post("/api/simulation/game-servers/{game_server_id}/fail")
    async def fail_game_server(game_server_id: str) -> dict:
        """Simulation only: makes a game server die, which voids its match.

        Example:
            >>> POST /api/simulation/game-servers/gs-7/fail
        """
        world.failGameServer(game_server_id)
        return world.describeStatus()

    @app.post("/api/simulation/machines/{machine_id}/fail")
    async def fail_machine(machine_id: str) -> dict:
        """Simulation only: makes a machine die, with every game server on it.

        Example:
            >>> POST /api/simulation/machines/m-us-east-1/fail
        """
        world.failMachine(machine_id)
        return world.describeStatus()

    add_account_routes(app, world)
    add_provider_routes(app, world)
    app.mount("/", StaticFiles(directory=web_folder, html=True), name="web")
    return app
