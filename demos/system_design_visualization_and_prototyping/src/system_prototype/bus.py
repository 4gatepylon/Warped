"""`bus.py` gives every simulated component the same database, clock and message log.

Interface:
    COMPONENTS: The names components have in messages, which the system view draws as boxes.
    Bus: The shared database, clock and message log.

Example:
    >>> bus = Bus(open_store(Path(":memory:")), SimClock())
    >>> bus.send(bus.readSeconds(), "game_app", "backend", "GET /me")
"""

import sqlite3
from collections.abc import Iterator
from contextlib import contextmanager

from .clock import SimClock
from .store import read_rows

COMPONENTS: tuple[str, ...] = (
    "game_app",
    "simulated_players",
    "website",
    "identity_provider",
    "payment_provider",
    "secret_manager",
    "backend",
    "database",
    "fleet_manager",
    "cloud",
    "machine",
    "game_server",
)


class Bus:
    """`Bus` is what the simulated components share: the database, the clock and the message log.

    Args:
        connection: The database, from `open_store`.
        clock: The prototype's clock.

    Example:
        >>> bus = Bus(open_store(Path(":memory:")), SimClock())

    Must know:
        Limitations:
            - A `Bus` is not thread-safe: call it from one thread or one event loop.
    """

    def __init__(self, connection: sqlite3.Connection, clock: SimClock):
        self.connection: sqlite3.Connection = connection
        self.clock: SimClock = clock

    def readSeconds(self) -> float:
        """`readSeconds` returns the simulated time now, in seconds.

        Example:
            >>> bus.readSeconds()
            0.0
        """
        return self.clock.readSeconds()

    def read(self, sql: str, args: tuple = ()) -> list[dict]:
        """`read` runs a query on the database (see `store.read_rows`).

        Example:
            >>> bus.read("SELECT * FROM account WHERE account_id = ?", (1,))
        """
        return read_rows(self.connection, sql, args)

    def execute(self, sql: str, args: tuple = ()) -> sqlite3.Cursor:
        """`execute` runs one statement on the database and returns its cursor.

        Example:
            >>> bus.execute("DELETE FROM refresh_token WHERE family_id = ?", (3,))
        """
        return self.connection.execute(sql, args)

    @contextmanager
    def transaction(self) -> Iterator[None]:
        """`transaction` runs the statements inside it all or not at all.

        Example:
            >>> with bus.transaction():
            ...     bus.execute("INSERT INTO webhook_event ...")
        """
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            yield
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise
        self.connection.execute("COMMIT")

    def readSettings(self) -> dict[str, float]:
        """`readSettings` returns every setting's value by name.

        Example:
            >>> bus.readSettings()["max_match_count"]
            100.0
        """
        return {row["name"]: row["value"] for row in self.read("SELECT name, value FROM setting")}

    def send(self, at_s: float, sender: str, receiver: str, what: str, detail: str = "") -> None:
        """`send` records that `sender` called `receiver`, for the system view.

        Args:
            at_s: When.
            sender: The calling component, one of `COMPONENTS`.
            receiver: The called component, one of `COMPONENTS`.
            what: The call, such as "POST /matches".
            detail: Which account, machine, game server or request it was about.

        Raises:
            ValueError: `sender` or `receiver` isn't a component.

        Example:
            >>> bus.send(now_s, "fleet_manager", "cloud", "start machine", "m-us-east-3")
        """
        if sender not in COMPONENTS or receiver not in COMPONENTS:
            raise ValueError(f"unknown component in message {sender} -> {receiver}")
        self.connection.execute(
            "INSERT INTO message (at_s, sender, receiver, what, detail) VALUES (?, ?, ?, ?, ?)",
            (at_s, sender, receiver, what, detail),
        )
