"""`serve.py` runs one square-game room, with the flags of `game_server_contract.md`.

Interface:
    --game-server-id: This game server's UUID (required).
    --config: Its `GameServerConfig` as JSON; the square game takes only `{}` (default `{}`).
    --max-player-count: How many players it holds at once (required).
    --management-server-url: Where to check players, e.g. `http://127.0.0.1:8000` (required).
    --port: The port to listen on, on every network interface (required).

Example:
    $ uv run game-server --game-server-id 6f1c... --max-player-count 1000 \\
        --management-server-url http://127.0.0.1:8000 --port 20000

Must know:
    Gotchas:
        - The management server's launcher starts game servers; people rarely run this by hand.
"""

import json
import uuid

import click
import uvicorn

from game_server import play, room

_LISTEN_HOST: str = "0.0.0.0"
_MAX_PORT: int = 65535
_GRACEFUL_SHUTDOWN_S: float = 2.0
_EPILOG: str = (
    "Example: uv run game-server --game-server-id 6f1c2a9e-0d1b-4a55-9a77-2f0e5d8c4b11 "
    "--max-player-count 1000 --management-server-url http://127.0.0.1:8000 --port 20000"
)


def _parse_config(_context: click.Context, _parameter: click.Parameter, text: str) -> dict:
    """`_parse_config` reads `--config`, which the square game takes only as `{}`.

    Raises:
        click.BadParameter: `text` isn't the JSON object `{}`.

    Example:
        >>> _parse_config(context, parameter, "{}")
        {}
    """
    try:
        config: object = json.loads(text)
    except json.JSONDecodeError as error:
        raise click.BadParameter(f"expected JSON, such as {{}}: {error}") from error
    if config != {}:
        raise click.BadParameter("the square game has no settings, so it takes only {}")
    return config


@click.command(help="Run one square-game room until SIGINT or SIGTERM.", epilog=_EPILOG)
@click.option("--game-server-id", type=click.UUID, required=True, help="This game server's UUID.")
@click.option(
    "--config",
    default="{}",
    show_default=True,
    callback=_parse_config,
    help="Its GameServerConfig as JSON.",
)
@click.option(
    "--max-player-count",
    type=click.IntRange(min=1),
    required=True,
    help="How many players it holds at once.",
)
@click.option("--management-server-url", required=True, help="Where to check players.")
@click.option(
    "--port",
    type=click.IntRange(1, _MAX_PORT),
    required=True,
    help="The port to listen on, on every network interface.",
)
def main(
    game_server_id: uuid.UUID,
    config: dict,
    max_player_count: int,
    management_server_url: str,
    port: int,
) -> None:
    """`main` serves one room until the process gets `SIGINT` or `SIGTERM`.

    Example:
        >>> main(["--game-server-id", game_server_id, "--max-player-count", "1000", ...])
    """
    uvicorn.run(
        play.create_app(room.Room(max_player_count), game_server_id, management_server_url),
        host=_LISTEN_HOST,
        port=port,
        timeout_graceful_shutdown=_GRACEFUL_SHUTDOWN_S,
    )


if __name__ == "__main__":
    main()
