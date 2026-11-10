"""`store.py` opens the prototype's SQLite database, which stands in for the real PostgreSQL.

Interface:
    SCHEMA_SQL: The tables, a first draft of the real schema.
    DEFAULT_SETTINGS: Every tunable setting of the parts built so far, with its default.
    REGIONS: The regions machines run in.
    open_store: Opens (and, when new, fills) a database file.
    read_rows: Runs a query and returns its rows as dictionaries.

Example:
    >>> connection = open_store(Path(":memory:"))
"""

import sqlite3
from pathlib import Path

REGIONS: tuple[str, ...] = ("us-east", "us-west")

SCHEMA_SQL: str = """
CREATE TABLE IF NOT EXISTS setting (
    name TEXT PRIMARY KEY,
    section TEXT NOT NULL,
    meaning TEXT NOT NULL,
    value REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS machine (
    machine_id TEXT PRIMARY KEY,
    region TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('booting', 'ready', 'terminated', 'failed')),
    slot_count INTEGER NOT NULL,
    requested_at_s REAL NOT NULL,
    ready_at_s REAL,
    empty_since_s REAL,
    ended_at_s REAL
);
CREATE TABLE IF NOT EXISTS match_request (
    match_request_id INTEGER PRIMARY KEY AUTOINCREMENT,
    region TEXT NOT NULL,
    label TEXT NOT NULL,
    player_count INTEGER NOT NULL,
    match_s REAL NOT NULL,
    state TEXT NOT NULL
        CHECK (state IN ('waiting_for_room', 'waiting_for_server', 'placed', 'done')),
    requested_at_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS game_server (
    game_server_id TEXT PRIMARY KEY,
    machine_id TEXT NOT NULL REFERENCES machine (machine_id),
    match_request_id INTEGER NOT NULL REFERENCES match_request (match_request_id),
    state TEXT NOT NULL CHECK (state IN ('starting', 'running', 'stopped', 'failed')),
    requested_at_s REAL NOT NULL,
    running_at_s REAL,
    ended_at_s REAL
);
CREATE TABLE IF NOT EXISTS sim_player (
    sim_player_id INTEGER PRIMARY KEY AUTOINCREMENT,
    region TEXT NOT NULL,
    match_request_id INTEGER REFERENCES match_request (match_request_id)
);
CREATE TABLE IF NOT EXISTS message (
    message_id INTEGER PRIMARY KEY AUTOINCREMENT,
    at_s REAL NOT NULL,
    sender TEXT NOT NULL,
    receiver TEXT NOT NULL,
    what TEXT NOT NULL,
    detail TEXT NOT NULL
);
"""

DEFAULT_SETTINGS: tuple[tuple[str, str, str, float], ...] = (
    ("max_match_count", "Where and on what", "Most matches at once", 100),
    ("max_player_count", "Where and on what", "Most players at once", 1000),
    ("slots_per_machine", "Fleet", "Game servers one machine runs", 4),
    ("spare_slot_count", "Fleet", "Free game server slots kept ready in each region", 2),
    ("machine_boot_s", "Fleet", "Seconds a new machine takes to boot", 45),
    ("machine_empty_s", "Fleet", "Seconds a machine stays empty before it is turned off", 120),
    ("server_start_s", "Fleet", "Seconds a game server takes to start on a ready machine", 5),
    ("machine_usd_per_hour", "Fleet", "Price of one machine for one hour, in dollars", 0.145),
    ("online_player_target", "Simulation", "Simulated players online", 0),
    ("pretend_match_min", "Simulation", "Length of a pretend match, in minutes", 5),
    ("east_player_pct", "Simulation", "Share of simulated players nearest US East, in percent", 60),
)


def read_rows(connection: sqlite3.Connection, sql: str, args: tuple = ()) -> list[dict]:
    """`read_rows` runs `sql` and returns each row as a dictionary from column name to value.

    Args:
        connection: The database.
        sql: The query, with `?` for each argument.
        args: The arguments.

    Returns:
        The rows, in the query's order.

    Example:
        >>> read_rows(connection, "SELECT * FROM setting WHERE name = ?", ("max_match_count",))
    """
    return [dict(row) for row in connection.execute(sql, args).fetchall()]


def open_store(database_file: Path) -> sqlite3.Connection:
    """`open_store` opens `database_file`, creating its tables and default settings if it is new.

    Args:
        database_file: The SQLite file, or `Path(":memory:")` for a database that isn't kept.

    Returns:
        A connection whose rows read like dictionaries and that commits each statement.

    Example:
        >>> connection = open_store(Path(":memory:"))
    """
    connection: sqlite3.Connection = sqlite3.connect(
        str(database_file), isolation_level=None, check_same_thread=False
    )
    connection.row_factory = sqlite3.Row
    connection.executescript(SCHEMA_SQL)
    is_new_world: bool = connection.execute("SELECT COUNT(*) FROM setting").fetchone()[0] == 0
    if is_new_world:
        connection.executemany("INSERT INTO setting VALUES (?, ?, ?, ?)", DEFAULT_SETTINGS)
    return connection
