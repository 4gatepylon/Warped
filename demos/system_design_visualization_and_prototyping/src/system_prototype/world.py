"""`world.py` simulates the components of the system around a match and the messages between them.

Each component of the real design is played here by a few methods of `World`: the backend (which
admits matches under the caps), the fleet manager (which keeps game servers on machines), the cloud
(which boots and turns off machines), the game servers (which start, run a match and report its
result) and the database. Whenever one component would call another, `World` writes a row to the
`message` table, which the system view draws.

Interface:
    World: The simulated system.
    COMPONENTS: The names components have in messages.

Example:
    >>> world = World(open_store(Path(":memory:")), SimClock())
    >>> world.changeSetting("online_player_target", 40)
    >>> world.tick()

Must know:
    Limitations:
        - Matches are pretend ones that only last a while; the stand-in game comes in a later part.
        - Players are anonymous simulated ones; accounts come in a later part.
"""

import random
import sqlite3

from .clock import SimClock
from .rules import (
    MachineLoad,
    NotFoundError,
    RuleError,
    count_machines_to_start,
    has_room_under_caps,
    pick_machine,
    split_steps,
)
from .store import REGIONS, read_rows

COMPONENTS: tuple[str, ...] = (
    "simulated_players",
    "website",
    "backend",
    "database",
    "fleet_manager",
    "cloud",
    "machine",
    "game_server",
)
MAX_STEP_S: float = 2.0
MAX_STEP_COUNT: int = 600
PRETEND_MATCH_PLAYER_COUNT: int = 10
MATCH_LENGTH_SPREAD_FRAC: float = 0.3
RECENT_MESSAGE_S: float = 3.0
MESSAGE_LIMIT: int = 80
TABLE_ROW_LIMIT: int = 8
SECONDS_PER_MINUTE: int = 60
SECONDS_PER_HOUR: int = 3600
PERCENT: int = 100
_ACTIVE_SERVER_STATES: str = "('starting', 'running')"
_OPEN_REQUEST_STATES: str = "('waiting_for_server', 'placed')"


class World:
    """`World` holds the simulated system's state in a database and moves it forward in time.

    Args:
        connection: The database, from `open_store`.
        clock: The prototype's clock.
        random_seed: The seed for match lengths and players' regions (default 1).

    Example:
        >>> world = World(open_store(Path(":memory:")), SimClock())

    Must know:
        Limitations:
            - A `World` is not thread-safe: call it from one thread or one event loop.
    """

    def __init__(self, connection: sqlite3.Connection, clock: SimClock, random_seed: int = 1):
        self._connection: sqlite3.Connection = connection
        self._clock: SimClock = clock
        self._random: random.Random = random.Random(random_seed)
        self._last_tick_s: float = clock.readSeconds()

    def readSettings(self) -> dict[str, float]:
        """`readSettings` returns every setting's value by name.

        Returns:
            A dictionary from setting name to value.

        Example:
            >>> world.readSettings()["max_match_count"]
            100.0
        """
        return {row["name"]: row["value"] for row in self._read("SELECT name, value FROM setting")}

    def listSettings(self) -> list[dict]:
        """`listSettings` returns every setting with its section, meaning and value.

        Returns:
            The `setting` table's rows, by section and name.

        Example:
            >>> world.listSettings()[0]["section"]
            'Fleet'
        """
        return self._read("SELECT * FROM setting ORDER BY section, name")

    def changeSetting(self, name: str, value: float) -> None:
        """`changeSetting` sets one setting, as a superadmin does in the console.

        Args:
            name: The setting's name.
            value: Its new value, at least 0.

        Raises:
            NotFoundError: There is no setting named `name`.
            RuleError: `value` is negative.

        Example:
            >>> world.changeSetting("max_match_count", 50)
        """
        if name not in self.readSettings():
            raise NotFoundError(f"There is no setting named {name}.")
        if value < 0:
            raise RuleError(f"The setting {name} can't be negative, got {value}.")
        now_s: float = self._clock.readSeconds()
        self._send(now_s, "website", "backend", "PUT /settings", f"{name} = {value:g}")
        self._connection.execute("UPDATE setting SET value = ? WHERE name = ?", (value, name))
        self._send(now_s, "backend", "database", "UPDATE setting", name)

    def skipClock(self, seconds: float) -> None:
        """`skipClock` moves the clock forward and lets the system catch up.

        Args:
            seconds: How far to skip, more than 0.

        Raises:
            RuleError: `seconds` isn't positive.

        Example:
            >>> world.skipClock(3600.0)
        """
        if seconds <= 0:
            raise RuleError(f"The clock only skips forward, got {seconds} s.")
        self._clock.skip(seconds)
        self.tick()

    def tick(self) -> None:
        """`tick` moves every component forward to the clock's time, in steps of at most 2 s.

        Example:
            >>> world.tick()
        """
        now_s: float = self._clock.readSeconds()
        for step_s in split_steps(self._last_tick_s, now_s, MAX_STEP_S, MAX_STEP_COUNT):
            settings: dict[str, float] = self.readSettings()
            self._advanceMachines(step_s, settings)
            self._advanceGameServers(step_s, settings)
            self._matchPopulation(step_s, settings)
            self._admitWaiting(step_s, settings)
            self._placeRequests(step_s)
            self._scaleFleet(step_s, settings)
        self._last_tick_s = now_s

    def failGameServer(self, game_server_id: str) -> None:
        """`failGameServer` makes a starting or running game server die, which voids its match.

        Args:
            game_server_id: The game server.

        Raises:
            NotFoundError: There is no such game server.
            RuleError: The game server already ended.

        Example:
            >>> world.failGameServer("gs-7")
        """
        rows: list[dict] = self._read(
            "SELECT state FROM game_server WHERE game_server_id = ?", (game_server_id,)
        )
        if len(rows) == 0:
            raise NotFoundError(f"There is no game server {game_server_id}.")
        if rows[0]["state"] not in ("starting", "running"):
            raise RuleError(f"The game server {game_server_id} already ended.")
        self._endGameServer(self._clock.readSeconds(), game_server_id, "failed")

    def failMachine(self, machine_id: str) -> None:
        """`failMachine` makes a booting or ready machine die, with every game server on it.

        Args:
            machine_id: The machine.

        Raises:
            NotFoundError: There is no such machine.
            RuleError: The machine already ended.

        Example:
            >>> world.failMachine("m-us-east-1")
        """
        rows: list[dict] = self._read(
            "SELECT state FROM machine WHERE machine_id = ?", (machine_id,)
        )
        if len(rows) == 0:
            raise NotFoundError(f"There is no machine {machine_id}.")
        if rows[0]["state"] not in ("booting", "ready"):
            raise RuleError(f"The machine {machine_id} already ended.")
        now_s: float = self._clock.readSeconds()
        self._connection.execute(
            "UPDATE machine SET state = 'failed', ended_at_s = ? WHERE machine_id = ?",
            (now_s, machine_id),
        )
        self._send(now_s, "cloud", "fleet_manager", "machine lost", machine_id)
        for server in self._read(
            f"SELECT game_server_id FROM game_server WHERE machine_id = ?"
            f" AND state IN {_ACTIVE_SERVER_STATES}",
            (machine_id,),
        ):
            self._endGameServer(now_s, server["game_server_id"], "failed")

    def describeStatus(self) -> dict:
        """`describeStatus` returns what the public status page shows.

        Returns:
            The clock's time, the caps and their use, and each region's ready machines.

        Example:
            >>> world.describeStatus()["caps"]["max_match_count"]
            100
        """
        settings: dict[str, float] = self.readSettings()
        match_count, player_count = self._countUnderCaps()
        regions: list[dict] = self._read(
            "SELECT region, COUNT(*) AS ready_machine_count FROM machine"
            " WHERE state = 'ready' GROUP BY region"
        )
        return {
            "now_s": self._clock.readSeconds(),
            "caps": {
                "match_count": match_count,
                "max_match_count": int(settings["max_match_count"]),
                "player_count": player_count,
                "max_player_count": int(settings["max_player_count"]),
            },
            "wait_queue_count": self._read(
                "SELECT COUNT(*) AS n FROM match_request WHERE state = 'waiting_for_room'"
            )[0]["n"],
            "regions": regions,
        }

    def describeSystem(self) -> dict:
        """`describeSystem` returns everything the system view draws.

        Returns:
            The status, the settings, the simulated players, the cost so far, each region's
            machines with their game servers, the wait queue, how many messages went between each
            pair of components, the newest messages, and every table with its newest rows.

        Example:
            >>> world.describeSystem()["tables"][0]["name"]
            'game_server'
        """
        now_s: float = self._clock.readSeconds()
        settings: dict[str, float] = self.readSettings()
        machine_hours: float = (
            self._read(
                "SELECT COALESCE(SUM(COALESCE(ended_at_s, ?) - requested_at_s), 0) AS s"
                " FROM machine",
                (now_s,),
            )[0]["s"]
            / SECONDS_PER_HOUR
        )
        machines: list[dict] = self._read(
            "SELECT * FROM machine WHERE state IN ('booting', 'ready') ORDER BY region, machine_id"
        )
        for machine in machines:
            machine["game_servers"] = self._read(
                f"SELECT s.game_server_id, s.state, s.running_at_s, r.label, r.player_count,"
                f" r.match_s FROM game_server s JOIN match_request r USING (match_request_id)"
                f" WHERE s.machine_id = ? AND s.state IN {_ACTIVE_SERVER_STATES}"
                f" ORDER BY s.requested_at_s",
                (machine["machine_id"],),
            )
        tables: list[dict] = self._read(
            "SELECT name, sql FROM sqlite_master WHERE type = 'table'"
            " AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        for table in tables:
            name: str = table["name"]
            table["row_count"] = self._read(f"SELECT COUNT(*) AS n FROM {name}")[0]["n"]
            table["rows"] = self._read(
                f"SELECT * FROM {name} ORDER BY rowid DESC LIMIT {TABLE_ROW_LIMIT}"
            )
        return {
            **self.describeStatus(),
            "settings": self.listSettings(),
            "players": self._read(
                "SELECT COUNT(*) AS online_count,"
                " COALESCE(SUM(p.match_request_id IS NULL), 0) AS idle_count,"
                " COALESCE(SUM(r.state = 'placed'), 0) AS in_match_count"
                " FROM sim_player p LEFT JOIN match_request r USING (match_request_id)"
            )[0],
            "cost": {
                "machine_hours": machine_hours,
                "usd": machine_hours * settings["machine_usd_per_hour"],
            },
            "machines": machines,
            "wait_queue": self._read(
                "SELECT * FROM match_request WHERE state = 'waiting_for_room'"
                " ORDER BY match_request_id"
            ),
            "links": self._read(
                "SELECT sender, receiver, COUNT(*) AS message_count,"
                " SUM(at_s > ?) AS recent_count FROM message GROUP BY sender, receiver",
                (now_s - RECENT_MESSAGE_S,),
            ),
            "messages": self._read(
                f"SELECT * FROM message ORDER BY message_id DESC LIMIT {MESSAGE_LIMIT}"
            ),
            "tables": tables,
        }

    def _read(self, sql: str, args: tuple = ()) -> list[dict]:
        """`_read` runs a query on the world's database (see `store.read_rows`).

        Example:
            >>> self._read("SELECT * FROM machine WHERE region = ?", ("us-east",))
        """
        return read_rows(self._connection, sql, args)

    def _send(self, at_s: float, sender: str, receiver: str, what: str, detail: str = "") -> None:
        """`_send` records that `sender` called `receiver`, for the system view.

        Args:
            at_s: When.
            sender: The calling component, one of `COMPONENTS`.
            receiver: The called component, one of `COMPONENTS`.
            what: The call, such as "POST /matches".
            detail: Which machine, game server or request it was about.

        Raises:
            ValueError: `sender` or `receiver` isn't a component.

        Example:
            >>> self._send(now_s, "fleet_manager", "cloud", "start machine", "m-us-east-3")
        """
        if sender not in COMPONENTS or receiver not in COMPONENTS:
            raise ValueError(f"unknown component in message {sender} -> {receiver}")
        self._connection.execute(
            "INSERT INTO message (at_s, sender, receiver, what, detail) VALUES (?, ?, ?, ?, ?)",
            (at_s, sender, receiver, what, detail),
        )

    def _countUnderCaps(self) -> tuple[int, int]:
        """`_countUnderCaps` returns how many matches and players count toward the caps.

        Returns:
            The number of matches that have or await a game server, and the players in them.

        Example:
            >>> match_count, player_count = self._countUnderCaps()
        """
        row: dict = self._read(
            f"SELECT COUNT(*) AS match_count, COALESCE(SUM(player_count), 0) AS player_count"
            f" FROM match_request WHERE state IN {_OPEN_REQUEST_STATES}"
        )[0]
        return row["match_count"], row["player_count"]

    def _advanceMachines(self, now_s: float, settings: dict[str, float]) -> None:
        """`_advanceMachines` plays the cloud: machines that finished booting become ready.

        Example:
            >>> self._advanceMachines(now_s, settings)
        """
        for machine in self._read(
            "SELECT machine_id FROM machine WHERE state = 'booting' AND requested_at_s + ? <= ?",
            (settings["machine_boot_s"], now_s),
        ):
            self._connection.execute(
                "UPDATE machine SET state = 'ready', ready_at_s = ?, empty_since_s = ?"
                " WHERE machine_id = ?",
                (now_s, now_s, machine["machine_id"]),
            )
            self._send(now_s, "cloud", "fleet_manager", "machine ready", machine["machine_id"])

    def _advanceGameServers(self, now_s: float, settings: dict[str, float]) -> None:
        """`_advanceGameServers` plays the game servers: they come up, then finish their match.

        Example:
            >>> self._advanceGameServers(now_s, settings)
        """
        for server in self._read(
            "SELECT game_server_id FROM game_server WHERE state = 'starting'"
            " AND requested_at_s + ? <= ?",
            (settings["server_start_s"], now_s),
        ):
            self._connection.execute(
                "UPDATE game_server SET state = 'running', running_at_s = ?"
                " WHERE game_server_id = ?",
                (now_s, server["game_server_id"]),
            )
            self._send(
                now_s, "fleet_manager", "game_server", "GET /health: 200", server["game_server_id"]
            )
        for server in self._read(
            "SELECT s.game_server_id FROM game_server s JOIN match_request r"
            " USING (match_request_id) WHERE s.state = 'running'"
            " AND s.running_at_s + r.match_s <= ?",
            (now_s,),
        ):
            self._endGameServer(now_s, server["game_server_id"], "stopped")

    def _endGameServer(self, now_s: float, game_server_id: str, end_state: str) -> None:
        """`_endGameServer` ends a game server and frees its players and its machine's slot.

        Args:
            now_s: When.
            game_server_id: The game server.
            end_state: "stopped" when its match finished, "failed" when it died.

        Example:
            >>> self._endGameServer(now_s, "gs-7", "stopped")
        """
        server: dict = self._read(
            "SELECT machine_id, match_request_id FROM game_server WHERE game_server_id = ?",
            (game_server_id,),
        )[0]
        self._connection.execute(
            "UPDATE game_server SET state = ?, ended_at_s = ? WHERE game_server_id = ?",
            (end_state, now_s, game_server_id),
        )
        if end_state == "stopped":
            self._send(now_s, "game_server", "backend", "POST /internal/matches", game_server_id)
            self._send(now_s, "backend", "database", "INSERT match result", game_server_id)
        else:
            self._send(now_s, "fleet_manager", "backend", "game server failed", game_server_id)
            self._send(now_s, "backend", "database", "void the match", game_server_id)
        request_id: int = server["match_request_id"]
        self._connection.execute(
            "UPDATE match_request SET state = 'done' WHERE match_request_id = ?", (request_id,)
        )
        self._connection.execute(
            "UPDATE sim_player SET match_request_id = NULL WHERE match_request_id = ?",
            (request_id,),
        )
        self._connection.execute(
            f"UPDATE machine SET empty_since_s = ? WHERE machine_id = ? AND NOT EXISTS"
            f" (SELECT 1 FROM game_server WHERE machine_id = machine.machine_id"
            f" AND state IN {_ACTIVE_SERVER_STATES})",
            (now_s, server["machine_id"]),
        )

    def _matchPopulation(self, now_s: float, settings: dict[str, float]) -> None:
        """`_matchPopulation` plays the simulated players: they come online and ask for matches.

        It brings the number of players online toward `online_player_target` (players in a match
        stay until it ends), then puts every ten idle players of a region into a pretend match.

        Example:
            >>> self._matchPopulation(now_s, settings)
        """
        target_count: int = int(settings["online_player_target"])
        online_count: int = self._read("SELECT COUNT(*) AS n FROM sim_player")[0]["n"]
        for _ in range(target_count - online_count):
            is_east: bool = self._random.random() * PERCENT < settings["east_player_pct"]
            self._connection.execute(
                "INSERT INTO sim_player (region) VALUES (?)", (REGIONS[0 if is_east else 1],)
            )
        if online_count > target_count:
            self._connection.execute(
                "DELETE FROM sim_player WHERE sim_player_id IN (SELECT sim_player_id"
                " FROM sim_player WHERE match_request_id IS NULL LIMIT ?)",
                (online_count - target_count,),
            )
        for region in REGIONS:
            idle_ids: list[int] = [
                row["sim_player_id"]
                for row in self._read(
                    "SELECT sim_player_id FROM sim_player WHERE region = ?"
                    " AND match_request_id IS NULL ORDER BY sim_player_id",
                    (region,),
                )
            ]
            while len(idle_ids) >= PRETEND_MATCH_PLAYER_COUNT:
                group_ids: list[int] = idle_ids[:PRETEND_MATCH_PLAYER_COUNT]
                idle_ids = idle_ids[PRETEND_MATCH_PLAYER_COUNT:]
                spread_frac: float = self._random.uniform(
                    1 - MATCH_LENGTH_SPREAD_FRAC, 1 + MATCH_LENGTH_SPREAD_FRAC
                )
                match_s: float = settings["pretend_match_min"] * SECONDS_PER_MINUTE * spread_frac
                request_id: int = self._requestMatch(now_s, region, match_s, settings)
                marks: str = ", ".join("?" * len(group_ids))
                self._connection.execute(
                    f"UPDATE sim_player SET match_request_id = ? WHERE sim_player_id IN ({marks})",
                    (request_id, *group_ids),
                )

    def _requestMatch(
        self, now_s: float, region: str, match_s: float, settings: dict[str, float]
    ) -> int:
        """`_requestMatch` plays the backend: it gives a match a game server or a wait queue place.

        Args:
            now_s: When.
            region: The region the match runs in.
            match_s: How long the pretend match lasts.
            settings: The settings, with the caps.

        Returns:
            The new match request's ID.

        Example:
            >>> request_id = self._requestMatch(now_s, "us-east", 300.0, settings)
        """
        match_count, player_count = self._countUnderCaps()
        has_room: bool = has_room_under_caps(
            match_count, player_count, PRETEND_MATCH_PLAYER_COUNT, settings
        )
        state: str = "waiting_for_server" if has_room else "waiting_for_room"
        self._send(now_s, "simulated_players", "backend", "POST /matches", region)
        request_id: int = self._connection.execute(
            "INSERT INTO match_request (region, label, player_count, match_s, state,"
            " requested_at_s) VALUES (?, 'pretend match', ?, ?, ?, ?)",
            (region, PRETEND_MATCH_PLAYER_COUNT, match_s, state, now_s),
        ).lastrowid
        self._send(now_s, "backend", "database", "INSERT match_request", f"request {request_id}")
        if has_room:
            self._send(
                now_s, "backend", "fleet_manager", "launch game server", f"request {request_id}"
            )
        return request_id

    def _admitWaiting(self, now_s: float, settings: dict[str, float]) -> None:
        """`_admitWaiting` plays the backend: it lets the wait queue in, first come first served.

        Example:
            >>> self._admitWaiting(now_s, settings)
        """
        for request in self._read(
            "SELECT match_request_id, player_count FROM match_request"
            " WHERE state = 'waiting_for_room' ORDER BY match_request_id"
        ):
            match_count, player_count = self._countUnderCaps()
            if not has_room_under_caps(
                match_count, player_count, request["player_count"], settings
            ):
                return
            request_id: int = request["match_request_id"]
            self._connection.execute(
                "UPDATE match_request SET state = 'waiting_for_server' WHERE match_request_id = ?",
                (request_id,),
            )
            self._send(
                now_s, "backend", "fleet_manager", "launch game server", f"request {request_id}"
            )

    def _readMachineLoads(self, region: str) -> list[MachineLoad]:
        """`_readMachineLoads` returns how full each ready machine of `region` is.

        Example:
            >>> machine_loads = self._readMachineLoads("us-east")
        """
        return [
            MachineLoad(row["machine_id"], row["slot_count"], row["used_slot_count"])
            for row in self._read(
                f"SELECT m.machine_id, m.slot_count, (SELECT COUNT(*) FROM game_server s"
                f" WHERE s.machine_id = m.machine_id AND s.state IN {_ACTIVE_SERVER_STATES})"
                f" AS used_slot_count FROM machine m WHERE m.state = 'ready' AND m.region = ?"
                f" ORDER BY m.machine_id",
                (region,),
            )
        ]

    def _placeRequests(self, now_s: float) -> None:
        """`_placeRequests` plays the fleet manager: it starts game servers on ready machines.

        Example:
            >>> self._placeRequests(now_s)
        """
        for request in self._read(
            "SELECT match_request_id, region FROM match_request"
            " WHERE state = 'waiting_for_server' ORDER BY match_request_id"
        ):
            machine_id: str | None = pick_machine(self._readMachineLoads(request["region"]))
            if machine_id is None:
                continue
            server_count: int = self._read("SELECT COUNT(*) AS n FROM game_server")[0]["n"]
            game_server_id: str = f"gs-{server_count + 1}"
            self._connection.execute(
                "INSERT INTO game_server (game_server_id, machine_id, match_request_id, state,"
                " requested_at_s) VALUES (?, ?, ?, 'starting', ?)",
                (game_server_id, machine_id, request["match_request_id"], now_s),
            )
            self._connection.execute(
                "UPDATE match_request SET state = 'placed' WHERE match_request_id = ?",
                (request["match_request_id"],),
            )
            self._connection.execute(
                "UPDATE machine SET empty_since_s = NULL WHERE machine_id = ?", (machine_id,)
            )
            self._send(
                now_s,
                "fleet_manager",
                "machine",
                "start game server",
                f"{game_server_id} on {machine_id}",
            )

    def _scaleFleet(self, now_s: float, settings: dict[str, float]) -> None:
        """`_scaleFleet` plays the fleet manager's autoscaler, region by region.

        It asks the cloud for machines until the matches awaiting a game server and the spare
        slots are covered, and turns off machines that have been empty for `machine_empty_s`
        when the region keeps its spare slots without them.

        Example:
            >>> self._scaleFleet(now_s, settings)
        """
        slots_per_machine: int = int(settings["slots_per_machine"])
        for region in REGIONS:
            machine_loads: list[MachineLoad] = self._readMachineLoads(region)
            free_slot_count: int = sum(
                load.slot_count - load.used_slot_count for load in machine_loads
            )
            booting_slot_count: int = self._read(
                "SELECT COALESCE(SUM(slot_count), 0) AS n FROM machine"
                " WHERE state = 'booting' AND region = ?",
                (region,),
            )[0]["n"]
            pending_count: int = self._read(
                "SELECT COUNT(*) AS n FROM match_request"
                " WHERE state = 'waiting_for_server' AND region = ?",
                (region,),
            )[0]["n"]
            needed_slot_count: int = pending_count + int(settings["spare_slot_count"])
            machine_count: int = self._read(
                "SELECT COUNT(*) AS n FROM machine WHERE region = ?", (region,)
            )[0]["n"]
            for i in range(
                count_machines_to_start(
                    needed_slot_count, free_slot_count, booting_slot_count, slots_per_machine
                )
            ):
                machine_id: str = f"m-{region}-{machine_count + i + 1}"
                self._connection.execute(
                    "INSERT INTO machine (machine_id, region, state, slot_count, requested_at_s)"
                    " VALUES (?, ?, 'booting', ?, ?)",
                    (machine_id, region, slots_per_machine, now_s),
                )
                self._send(now_s, "fleet_manager", "cloud", "start machine", machine_id)
            for machine in self._read(
                "SELECT machine_id, slot_count FROM machine WHERE state = 'ready' AND region = ?"
                " AND empty_since_s IS NOT NULL AND empty_since_s + ? <= ? ORDER BY machine_id",
                (region, settings["machine_empty_s"], now_s),
            ):
                if free_slot_count - machine["slot_count"] < needed_slot_count:
                    break
                free_slot_count -= machine["slot_count"]
                self._connection.execute(
                    "UPDATE machine SET state = 'terminated', ended_at_s = ? WHERE machine_id = ?",
                    (now_s, machine["machine_id"]),
                )
                self._send(
                    now_s, "fleet_manager", "cloud", "turn machine off", machine["machine_id"]
                )
