"""`serve.py` serves the system prototype on this computer until it gets Ctrl-C.

Interface:
    --port: The HTTP port, 1024 to 65535 (default 8010).
    --database-file: The SQLite file that holds the world. An existing file is carried on from;
        by default each run starts a new world in its run folder.

Example:
    $ uv run system-prototype --port 8010

Must know:
    Side effects:
        - Each run writes `.artifacts/serve/<run_id>/command.txt`, and its database unless
          `--database-file` names another place.
"""

import shlex
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

import click
import uvicorn

from .clock import SimClock
from .http_api import make_app
from .store import open_store, read_rows
from .world import World

DEFAULT_PORT: int = 8010
MIN_PORT: int = 1024
MAX_PORT: int = 65535
LOCAL_HOST: str = "127.0.0.1"
_COMPONENT_FOLDER: Path = Path(__file__).resolve().parents[2]


def _validate_database_file(database_file: Path | None) -> None:
    """`_validate_database_file` passes only when the database file's folder exists.

    Args:
        database_file: The `--database-file` argument, or None when it wasn't given.

    Raises:
        click.UsageError: The file's folder doesn't exist.

    Example:
        >>> _validate_database_file(Path("/tmp/world.sqlite3"))
    """
    if database_file is not None and not database_file.resolve().parent.is_dir():
        raise click.UsageError(
            f"--database-file {database_file}: its folder {database_file.resolve().parent} doesn't"
            f" exist. Give a file in an existing folder, for example"
            f" --database-file /tmp/world.sqlite3"
        )


@click.command(epilog="Example: uv run system-prototype --port 8010")
@click.option(
    "--port",
    type=click.IntRange(MIN_PORT, MAX_PORT),
    default=DEFAULT_PORT,
    show_default=True,
    help="The HTTP port to serve on, on this computer only.",
)
@click.option(
    "--database-file",
    type=click.Path(dir_okay=False, path_type=Path),
    default=None,
    help="The SQLite file that holds the world; an existing one is carried on from."
    "  [default: a new file in this run's folder]",
)
def main(port: int, database_file: Path | None) -> None:
    """Serve the system prototype: the simulated system, its controls and its web pages."""
    _validate_database_file(database_file)
    run_id: str = datetime.now(UTC).strftime("%Y%m%d-%H%M%S")
    run_folder: Path = _COMPONENT_FOLDER / ".artifacts" / "serve" / run_id
    run_folder.mkdir(parents=True)
    git_commit: str = subprocess.run(
        ["git", "describe", "--always", "--dirty"],
        cwd=_COMPONENT_FOLDER,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    (run_folder / "command.txt").write_text(f"{shlex.join(sys.argv)}\n{git_commit}\n")
    world_file: Path = run_folder / "world.sqlite3" if database_file is None else database_file
    connection = open_store(world_file)
    start_s: float = read_rows(connection, "SELECT COALESCE(MAX(at_s), 0) AS s FROM message")[0][
        "s"
    ]
    world: World = World(connection, SimClock(start_s=start_s))
    click.echo(f"Run folder: {run_folder}")
    click.echo(f"Database:   {world_file}")
    click.echo(f"Open:       http://{LOCAL_HOST}:{port}/")
    uvicorn.run(
        make_app(world, _COMPONENT_FOLDER / "web"), host=LOCAL_HOST, port=port, log_level="warning"
    )
