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
CREATE TABLE IF NOT EXISTS secret (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS google_signing_key (
    key_id TEXT PRIMARY KEY,
    private_key_pem TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS google_identity (
    subject TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    full_name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS google_code (
    code_hash TEXT PRIMARY KEY,
    subject TEXT NOT NULL REFERENCES google_identity (subject),
    client_id TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    nonce TEXT NOT NULL,
    expires_at_s REAL NOT NULL,
    used_at_s REAL
);
CREATE TABLE IF NOT EXISTS account (
    account_id INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT UNIQUE COLLATE NOCASE,
    icon TEXT,
    role TEXT NOT NULL CHECK (role IN ('player', 'superadmin')),
    state TEXT NOT NULL CHECK (state IN ('active', 'deleted')),
    created_at_s REAL NOT NULL,
    age_confirmed_at_s REAL NOT NULL,
    terms_accepted_at_s REAL NOT NULL,
    name_changed_at_s REAL NOT NULL,
    finished_match_count INTEGER NOT NULL DEFAULT 0,
    played_s REAL NOT NULL DEFAULT 0,
    deleted_at_s REAL
);
CREATE TABLE IF NOT EXISTS identity (
    provider TEXT NOT NULL,
    subject TEXT NOT NULL,
    account_id INTEGER NOT NULL REFERENCES account (account_id),
    email TEXT NOT NULL,
    PRIMARY KEY (provider, subject)
);
CREATE TABLE IF NOT EXISTS used_allowance (
    identity_hash TEXT PRIMARY KEY,
    finished_match_count INTEGER NOT NULL,
    played_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS session_family (
    family_id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES account (account_id),
    created_at_s REAL NOT NULL,
    revoked_at_s REAL,
    revoked_because TEXT
);
CREATE TABLE IF NOT EXISTS refresh_token (
    token_hash TEXT PRIMARY KEY,
    family_id INTEGER NOT NULL REFERENCES session_family (family_id),
    issued_at_s REAL NOT NULL,
    expires_at_s REAL NOT NULL,
    used_at_s REAL
);
CREATE TABLE IF NOT EXISTS stripe_checkout (
    checkout_id TEXT PRIMARY KEY,
    client_reference_id TEXT NOT NULL,
    amount_cents INTEGER NOT NULL,
    success_url TEXT NOT NULL,
    cancel_url TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('open', 'complete')),
    payment_id TEXT,
    created_at_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS stripe_payment (
    payment_id TEXT PRIMARY KEY,
    checkout_id TEXT NOT NULL REFERENCES stripe_checkout (checkout_id),
    amount_cents INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('succeeded', 'refunded', 'disputed')),
    paid_at_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS stripe_event (
    event_id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at_s REAL NOT NULL,
    delivery_count INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS purchase (
    payment_id TEXT PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES account (account_id),
    amount_cents INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('paid', 'refunded', 'disputed')),
    paid_at_s REAL NOT NULL,
    locked_at_s REAL
);
CREATE TABLE IF NOT EXISTS webhook_event (
    event_id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    payment_id TEXT NOT NULL,
    received_at_s REAL NOT NULL
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
    ("name_change_days", "Accounts", "Days between name changes", 30),
    ("access_token_min", "Sign-in", "Minutes an access token lasts", 15),
    ("refresh_token_days", "Sign-in", "Days a refresh token lasts unused", 60),
    ("price_usd", "Paying", "Price of the purchase, in dollars", 13.37),
    ("free_match_count", "Paying", "Free allowance, finished matches", 1),
    ("free_match_min", "Paying", "Free allowance, minutes in matches", 30),
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
    """`open_store` opens `database_file`, adding any missing table and default setting.

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
    connection.executemany("INSERT OR IGNORE INTO setting VALUES (?, ?, ?, ?)", DEFAULT_SETTINGS)
    return connection
