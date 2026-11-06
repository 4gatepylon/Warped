"""`serve.py` runs the management server, with game servers as processes on this machine.

Interface:
    --port: The management server's HTTP port (default 8000).
    --advertised-host: The host clients reach game servers at; this machine's LAN IP for LAN
        play (default 127.0.0.1).
    --game-server-ports: The ports game servers may listen on, as FIRST-LAST (default
        20000-20099).
    --game-server-command: The game server command, without the contract's flags (required).

Example:
    $ uv run management-server --advertised-host 10.0.0.5 \\
        --game-server-command "uv run --project ../game_server game-server"

Must know:
    Gotchas:
        - The management server and its game servers listen on every network interface.
    Side effects:
        - When the management server shuts down, it stops every game server it started.
    Performance:
        - Shutting down takes up to 5 s for each game server that ignores `SIGTERM`.
"""

import collections.abc
import contextlib
import functools
import shlex

import click
import fastapi
import uvicorn

from management_server import http_api, local_launcher, management

DEFAULT_PORT: int = 8000
DEFAULT_ADVERTISED_HOST: str = "127.0.0.1"
DEFAULT_GAME_SERVER_PORTS: str = "20000-20099"
_LISTEN_HOST: str = "0.0.0.0"
_MANAGEMENT_SERVER_HOST_FOR_GAME_SERVERS: str = "127.0.0.1"
_MAX_PORT: int = 65535
_EPILOG: str = (
    "Example: uv run management-server --advertised-host 10.0.0.5 "
    "--game-server-command 'uv run --project ../game_server game-server'"
)


def _parse_port_range(_context: click.Context, _parameter: click.Parameter, text: str) -> range:
    """`_parse_port_range` turns `FIRST-LAST` into the range of ports from FIRST to LAST.

    Raises:
        click.BadParameter: `text` isn't two ports with 1 <= FIRST <= LAST <= 65535.

    Example:
        >>> _parse_port_range(context, parameter, "20000-20099")
        range(20000, 20100)
    """
    first_port_text: str
    last_port_text: str
    first_port_text, _, last_port_text = text.partition("-")
    if not (first_port_text.isdecimal() and last_port_text.isdecimal()):
        raise click.BadParameter(f"expected FIRST-LAST, such as {DEFAULT_GAME_SERVER_PORTS}")
    first_port: int = int(first_port_text)
    last_port_incl: int = int(last_port_text)
    if not 1 <= first_port <= last_port_incl <= _MAX_PORT:
        raise click.BadParameter(f"expected 1 <= FIRST <= LAST <= {_MAX_PORT}")
    return range(first_port, last_port_incl + 1)


def _parse_command(_context: click.Context, _parameter: click.Parameter, text: str) -> list[str]:
    """`_parse_command` splits a command line into its words, as a POSIX shell would.

    Raises:
        click.BadParameter: `text` has no words.

    Example:
        >>> _parse_command(context, parameter, "uv run game-server")
        ['uv', 'run', 'game-server']
    """
    words: list[str] = shlex.split(text)
    if not words:
        raise click.BadParameter("expected a command, such as 'uv run game-server'")
    return words


@contextlib.asynccontextmanager
async def _stop_game_servers_on_shutdown(
    _app: fastapi.FastAPI, game_server_launcher: local_launcher.LocalProcessLauncher
) -> collections.abc.AsyncIterator[None]:
    """`_stop_game_servers_on_shutdown` is a lifespan that stops every game server at shutdown.

    Example:
        >>> functools.partial(_stop_game_servers_on_shutdown, game_server_launcher=launcher)
    """
    yield
    for launched_game_server in game_server_launcher.list():
        game_server_launcher.stop(launched_game_server.game_server_id)


@click.command(
    help="Run the management server until it gets SIGINT or SIGTERM, then stop its game servers.",
    epilog=_EPILOG,
)
@click.option(
    "--port",
    type=click.IntRange(1, _MAX_PORT),
    default=DEFAULT_PORT,
    show_default=True,
    help="The management server's HTTP port.",
)
@click.option(
    "--advertised-host",
    default=DEFAULT_ADVERTISED_HOST,
    show_default=True,
    help="The host clients reach game servers at; this machine's LAN IP for LAN play.",
)
@click.option(
    "--game-server-ports",
    default=DEFAULT_GAME_SERVER_PORTS,
    show_default=True,
    callback=_parse_port_range,
    help="The ports game servers may listen on, as FIRST-LAST.",
)
@click.option(
    "--game-server-command",
    required=True,
    callback=_parse_command,
    help="The game server command, without the contract's flags.",
)
def main(
    port: int, advertised_host: str, game_server_ports: range, game_server_command: list[str]
) -> None:
    """`main` serves the HTTP API until the process gets `SIGINT` or `SIGTERM`.

    Example:
        >>> main(["--game-server-command", "uv run game-server"])
    """
    game_server_launcher: local_launcher.LocalProcessLauncher = local_launcher.LocalProcessLauncher(
        advertised_host,
        game_server_ports,
        game_server_command,
        f"http://{_MANAGEMENT_SERVER_HOST_FOR_GAME_SERVERS}:{port}",
    )
    app: fastapi.FastAPI = http_api.create_app(
        management.Management(game_server_launcher),
        lifespan=functools.partial(
            _stop_game_servers_on_shutdown, game_server_launcher=game_server_launcher
        ),
    )
    click.echo(f"Management server: http://{advertised_host}:{port}", err=True)
    uvicorn.run(app, host=_LISTEN_HOST, port=port)


if __name__ == "__main__":
    main()
