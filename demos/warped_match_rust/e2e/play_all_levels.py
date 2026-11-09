"""`play_all_levels.py` plays one bot match steered through all five levels and checks it.

`play_all_levels.py` starts a match server on `e2e/match_levels_fast.yaml` and a bot for each team, steered with
`--bot-round-winners` (the bot of the team that should lose a round joins no level in it), so the match visits every
level in `_PLAN`. In each round but Dark Forest's it also runs a fighting bot of each team (`--fight-s`), which walks
after the nearest enemy and shoots. It logs each level's start, winner and duration, when each of the loser's pylons
(consoles) went off, which pylons the console bot reached on foot, the eliminations and the respawns, then a table of
them per level, and fails on a stall (a round longer than `--round-timeout-s`), a process that stops early, a round,
level result file, pylon count or outcome that differs from `_PLAN`, a pylon turned off but not reached on foot, a
fighting level without an elimination or a respawn, a respawn early or late against the settings' `respawn.delay_s`,
a player more than the level's bots, or a panic or error in a log.

Interface:
    --match-port: The match server's UDP port (default 48110, away from the game's 47850).
    --level-port: Each round's level server's UDP port (default 48112, away from the game's 47852).
    --round-timeout-s: Seconds a round may last before it counts as a stall (default 600).
    --fight-s: Seconds the fighting bots play in each round (default 45; 0 for none).
    --width-px, --height-px: Size of the progression pictures (default 1280 x 800).
    --label: Suffix for the run folder's name.
    --dry-run: Print the plan and the commands, and start nothing.
    --json: Print the summary as one JSON object.
    Output: `.artifacts/play_all_levels/<run_id>/` with `command.txt`, `levels.log`, `summary.json`, `server.log`,
    `blue.log`, `red.log`, each fighter's `round_<n>_<team>_fighter.log` and `pictures/` (blue's progression page at
    each round's start and at the end).

Example:
    $ make levels && cargo build --release && python3 -I e2e/play_all_levels.py

Must know:
    Limitations:
        - The losing team's bot joins no level in a round, so each round tests one team's level client.
        - Only `_WALKING_LEVELS`' bots report each pylon they reach on foot (`bot reached console ...`), so the
          check that every pylon was reached on foot covers those levels; City Sphere's and Dark Forest's own bots
          don't report it.
    Side effects:
        - When it exits, `play_all_levels.py` stops every process it started, with the levels they started.
"""

import json
import os
import queue
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, TextIO

import click

_CRATE_FOLDER: Path = Path(__file__).resolve().parent.parent
_REPO_FOLDER: Path = _CRATE_FOLDER.parent.parent
_PROGRAM: Path = _CRATE_FOLDER / ".build" / "release" / "hyperbolic-wormholes"
_LEVELS_FILE: Path = _CRATE_FOLDER / "e2e" / "match_levels_fast.yaml"
_PLAN: list[tuple[str, str, str, int]] = [
    ("City Sphere", "none", "blue", 1),
    ("Dark Forest", "blue", "blue", 2),
    ("Lunar Surface", "blue", "red", 3),
    ("Snow Planet", "red", "red", 2),
    ("Dust", "red", "red", 3),
]
_OUTCOME: dict[str, str] = {"Won": "red"}
_ATTACKER_NAMES: dict[str, str] = {"None": "none", "Some(Blue)": "blue", "Some(Red)": "red"}
_PAUSE_S: float = 2.0
_START_TIMEOUT_S: float = 60.0
_STOP_TIMEOUT_S: float = 10.0
_POLL_S: float = 0.25
_START_LINE: re.Pattern[str] = re.compile(r"^Round (\d+): (.+), attacker (\S+)$")
_WON_LINE: re.Pattern[str] = re.compile(r"^Round (\d+): (blue|red) won (.+)$")
_MATCH_FOLDER_LINE: re.Pattern[str] = re.compile(r"^Run folder: (.+)$")
_PICTURE_FOLDER_LINE: re.Pattern[str] = re.compile(r"^Run folder: (.+/progression_pictures/.+)$", re.MULTILINE)
_PROGRAM_LINE: re.Pattern[str] = re.compile(r"^\s*program: (\S+)$", re.MULTILINE)
_LEVEL_BLOCK: re.Pattern[str] = re.compile(r"^  - name: .*?(?=^  - name: |\Z)", re.MULTILINE | re.DOTALL)
_FOLDER_LINE: re.Pattern[str] = re.compile(r"^\s*folder: (\S+)$", re.MULTILINE)
_SETTINGS_ARGUMENT: re.Pattern[str] = re.compile(r"--server-settings, (\S+?)\]")
_HOLD_KEY: str = "hold_to_disable_s"
_PYLON_OFF_LINE: re.Pattern[str] = re.compile(r"turned off console (\d+)")
_JOIN_LINE: re.Pattern[str] = re.compile(r"^player (\d+) joined the (blue|red) team", re.MULTILINE)
_KILL_LINE: re.Pattern[str] = re.compile(r"^player (\d+) (?:was hit by player (\d+)|froze in a blizzard)", re.MULTILINE)
_MAP_LINE: re.Pattern[str] = re.compile(r"^\s*map: (\S+)$", re.MULTILINE)
_RESPAWN_DELAY: re.Pattern[str] = re.compile(r"^respawn:.*\n\s+delay_s: ([\d.]+)", re.MULTILINE)
_NO_BOT_LABEL: str = "takes_bot_label: false"
_FIGHTLESS_LEVELS: tuple[str, ...] = ("Dark Forest",)
_RESPAWN_EARLY_S: float = 0.5
_RESPAWN_LATE_S: float = 3.0
_FAILURE_LINE: re.Pattern[str] = re.compile(r"panicked at|^Error: ", re.MULTILINE)
_WALKING_LEVELS: tuple[str, ...] = ("Lunar Surface", "Snow Planet", "Dust")
_ROUND_COMMAND_LINE: re.Pattern[str] = re.compile(r'^round (\d+): "', re.MULTILINE)
_REACHED_LINE: re.Pattern[str] = re.compile(r"^bot reached console (\d+) on foot in ([\d.]+) s", re.MULTILINE)
_round_t = dict[str, Any]


class _MatchCheckFailed(Exception):
    """`_MatchCheckFailed` says the match stalled, a process stopped early, or a result differs from `_PLAN`.

    Example:
        >>> raise _MatchCheckFailed("stall in round 3 (Lunar Surface)")
    """


def _log(levels_log: TextIO, message: str) -> None:
    """`_log` writes `message` after the UTC time to `levels.log` and to stderr.

    Args:
        levels_log: The run's open `levels.log`.
        message: The line to write.

    Example:
        >>> _log(levels_log, "round 1 started: City Sphere, attacker none")
    """
    line: str = f"{datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')} {message}"
    levels_log.write(line + "\n")
    levels_log.flush()
    click.echo(line, err=True)


def _start(arguments: list[str], log_path: Path | None, program: Path = _PROGRAM) -> subprocess.Popen[str]:
    """`_start` runs `program` (this crate's by default) with `arguments` in a process group of its own.

    Args:
        arguments: The program's arguments.
        log_path: The file that gets its output, or `None` to pipe it back.
        program: The program to run.

    Returns:
        The running process.

    Example:
        >>> server = _start(["--serve-match"], None)
    """
    command: list[str] = [str(program), *arguments]
    if log_path is None:
        return subprocess.Popen(
            command, cwd=_CRATE_FOLDER, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            start_new_session=True,
        )
    with log_path.open("w") as log:
        return subprocess.Popen(
            command, cwd=_CRATE_FOLDER, stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True
        )


def _stop(process: subprocess.Popen[str]) -> None:
    """`_stop` stops a process that `_start` started, and everything it started (its process group). A group with
    nothing left running (gone, or only processes that already ended, which macOS refuses to signal) is only reaped.

    Example:
        >>> _stop(server)
    """
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except (ProcessLookupError, PermissionError):
        process.wait()
        return
    try:
        process.wait(timeout=_STOP_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


class _LevelLog:
    """`_LevelLog` reads a round's level server log as it grows and times each event from the round's start.

    Args:
        path: The round's level server log (it may not exist yet).
        start_s: The round's start, on `time.monotonic()`.

    Attributes:
        pylons_off: Each console turned off: seconds from the start and its index.
        kills: Each elimination: seconds, the victim's id and the shooter's (`None` for a blizzard).
        joins: Each join or rejoin: seconds, the player's id and team.

    Example:
        >>> level_log = _LevelLog(match_folder / "round_1_server.log", time.monotonic())
    """

    def __init__(self, path: Path, start_s: float) -> None:
        self.path: Path = path
        self.start_s: float = start_s
        self.offset_bytes: int = 0
        self.pylons_off: list[tuple[float, int]] = []
        self.kills: list[tuple[float, int, int | None]] = []
        self.joins: list[tuple[float, int, str]] = []

    def read(self) -> None:
        """`read` notes the events of the whole lines written since the last read, timed now.

        Example:
            >>> level_log.read()
        """
        if not self.path.exists():
            return
        with self.path.open("rb") as level_log:
            level_log.seek(self.offset_bytes)
            text: bytes = level_log.read()
        whole_lines: str = text[: text.rfind(b"\n") + 1].decode()
        self.offset_bytes += len(whole_lines.encode())
        now_s: float = time.monotonic() - self.start_s
        self.pylons_off += [(now_s, int(line[1])) for line in _PYLON_OFF_LINE.finditer(whole_lines)]
        self.kills += [
            (now_s, int(line[1]), int(line[2]) if line[2] else None) for line in _KILL_LINE.finditer(whole_lines)
        ]
        self.joins += [(now_s, int(line[1]), line[2]) for line in _JOIN_LINE.finditer(whole_lines)]

    def get_respawn_delays_s(self) -> list[float]:
        """`get_respawn_delays_s` gives, for each elimination followed by a rejoin of its victim, the seconds between.

        Example:
            >>> delays_s = level_log.get_respawn_delays_s()
        """
        delays_s: list[float] = []
        for kill_s, victim, _ in self.kills:
            rejoins_s: list[float] = [join_s for join_s, id_, _ in self.joins if id_ == victim and join_s > kill_s]
            if rejoins_s:
                delays_s.append(min(rejoins_s) - kill_s)
        return delays_s


def _find_level_block(level: str) -> str:
    """`_find_level_block` gives the levels file's block for `level`.

    Raises:
        KeyError: The levels file has no such level.

    Example:
        >>> block = _find_level_block("Dust")
    """
    blocks: dict[str, str] = {
        block.split("\n")[0].split("name: ")[1]: block for block in _LEVEL_BLOCK.findall(_LEVELS_FILE.read_text())
    }
    return blocks[level]


def _get_respawn_delay_s(level: str) -> float:
    """`_get_respawn_delay_s` gives the `respawn.delay_s` of the server settings `level` plays by in this test.

    Example:
        >>> delay_s = _get_respawn_delay_s("Dust")
    """
    block: str = _find_level_block(level)
    argument: re.Match[str] | None = _SETTINGS_ARGUMENT.search(block)
    is_self: bool = _PROGRAM_LINE.search(block)[1] == "self"
    level_folder: Path = _CRATE_FOLDER if is_self else _REPO_FOLDER / _FOLDER_LINE.search(block)[1]
    settings_path: Path = _CRATE_FOLDER / argument[1] if argument else level_folder / "server_settings.yaml"
    return float(_RESPAWN_DELAY.search(settings_path.read_text())[1])


def _start_fighters(
    level: str, level_port: int, run_folder: Path, number: int, fight_s: float
) -> list[subprocess.Popen[str]]:
    """`_start_fighters` starts a fighting bot of each team in round `number`'s level, for `fight_s` seconds.

    Args:
        level: The round's level.
        level_port: The level server's UDP port.
        run_folder: The run folder, for each fighter's log.
        number: The round, from 1.
        fight_s: Seconds the fighters play before they leave.

    Returns:
        The fighters' processes, none in `_FIGHTLESS_LEVELS` or with `fight_s` 0.

    Example:
        >>> fighters = _start_fighters("Dust", 48112, run_folder, 5, 45.0)
    """
    if level in _FIGHTLESS_LEVELS or fight_s == 0:
        return []
    block: str = _find_level_block(level)
    program_name: str = _PROGRAM_LINE.search(block)[1]
    program: Path = _PROGRAM if program_name == "self" else _CRATE_FOLDER / program_name
    map_arguments: list[str] = ["--map", map_match[1]] if (map_match := _MAP_LINE.search(block)) else []
    fighters: list[subprocess.Popen[str]] = []
    for team in ("blue", "red"):
        label: list[str] = [] if _NO_BOT_LABEL in block else ["--label", f"fighter-{team}-round-{number}"]
        arguments: list[str] = [
            "--join", "127.0.0.1", "--port", str(level_port), *map_arguments, "--team", team, "--bot",
            "--exit-after-s", str(fight_s), *label,
        ]
        fighters.append(_start(arguments, run_folder / f"round_{number}_{team}_fighter.log", program))
    return fighters


def _find_walks(run_folder: Path, number: int, winner: str) -> list[str]:
    """`_find_walks` gives each pylon the winner's bot reached on foot in round `number`, from its log.

    Args:
        run_folder: The run folder, with each team's bot log (`<team>.log`).
        number: The round, from 1.
        winner: The team that won it.

    Returns:
        One `console <i> in <t> s` per pylon its level bot walked to, in that order.

    Example:
        >>> walks = _find_walks(run_folder, 3, "red")
    """
    log: str = (run_folder / f"{winner}.log").read_text()
    starts: list[tuple[int, int]] = [(int(line[1]), line.start()) for line in _ROUND_COMMAND_LINE.finditer(log)]
    walks: list[str] = []
    for index, (round_number, start) in enumerate(starts):
        end: int = starts[index + 1][1] if index + 1 < len(starts) else len(log)
        if round_number == number:
            walks += [f"console {line[1]} in {line[2]} s" for line in _REACHED_LINE.finditer(log, start, end)]
    return walks


def _watch_match(
    server: subprocess.Popen[str],
    bots: dict[str, subprocess.Popen[str]],
    run_folder: Path,
    round_timeout_s: float,
    level_port: int,
    fight_s: float,
) -> tuple[Path, list[_round_t]]:
    """`_watch_match` follows the match server's output until it exits, starting each round's fighting bots and logging
    each round to `levels.log`.

    Args:
        server: The match server, its output piped.
        bots: Each team's bot match client.
        run_folder: The run folder, for `server.log`, `levels.log` and the fighters' logs.
        round_timeout_s: Seconds a round may last before it counts as a stall.
        level_port: The level servers' UDP port, for the fighters.
        fight_s: Seconds the fighters play in each round (0 for none).

    Returns:
        The match server's run folder, and each round: its level, attacker, winner, duration, level log
        (`_LevelLog`), the pylons its console bot reached on foot (`_find_walks`) and whether it had fighters.

    Raises:
        _MatchCheckFailed: A round stalled, or a process stopped before the match was over.

    Example:
        >>> match_folder, rounds = _watch_match(server, bots, run_folder, 600.0, 48112, 45.0)
    """
    lines: queue.Queue[str | None] = queue.Queue()

    def _read_server() -> None:
        """`_read_server` copies the server's lines to `server.log` and to `lines`, then `None` at its end."""
        with (run_folder / "server.log").open("w") as server_log:
            for line in server.stdout:
                server_log.write(line)
                server_log.flush()
                lines.put(line.rstrip("\n"))
        lines.put(None)

    threading.Thread(target=_read_server, daemon=True).start()
    match_folder: Path | None = None
    rounds: list[_round_t] = []
    started: tuple[str, str, float] | None = None
    level_log: _LevelLog | None = None
    fighters: list[subprocess.Popen[str]] = []
    deadline_s: float = time.monotonic() + _START_TIMEOUT_S
    try:
        with (run_folder / "levels.log").open("w") as levels_log:
            while True:
                if level_log is not None:
                    level_log.read()
                for team, bot in bots.items():
                    if bot.poll() is not None and (bot.returncode != 0 or len(rounds) < len(_PLAN)):
                        raise _MatchCheckFailed(f"{team}'s bot stopped ({bot.returncode}) mid-match; see {team}.log")
                if time.monotonic() > deadline_s:
                    level: str = started[0] if started else "not started"
                    raise _MatchCheckFailed(f"stall in round {len(rounds) + 1} ({level}); see server.log")
                try:
                    line: str | None = lines.get(timeout=_POLL_S)
                except queue.Empty:
                    continue
                if line is None:
                    break
                if folder_match := _MATCH_FOLDER_LINE.match(line):
                    match_folder = Path(folder_match[1])
                elif (start_match := _START_LINE.match(line)) and match_folder is not None:
                    number: int = int(start_match[1])
                    started = (start_match[2], _ATTACKER_NAMES[start_match[3]], time.monotonic())
                    level_log = _LevelLog(match_folder / f"round_{number}_server.log", started[2])
                    fighters = _start_fighters(started[0], level_port, run_folder, number, fight_s)
                    deadline_s = started[2] + round_timeout_s
                    _log(levels_log, f"round {number} started: {started[0]}, attacker {started[1]}")
                elif (won_match := _WON_LINE.match(line)) and started is not None and level_log is not None:
                    for fighter in fighters:
                        _stop(fighter)
                    duration_s: float = time.monotonic() - started[2]
                    level_log.read()
                    walks: list[str] = _find_walks(run_folder, int(won_match[1]), won_match[2])
                    round_: _round_t = {
                        "level": started[0], "attacker": started[1], "winner": won_match[2],
                        "duration_s": duration_s, "level_log": level_log, "walks": walks,
                        "has_fighters": bool(fighters),
                    }
                    rounds.append(round_)
                    deadline_s = time.monotonic() + _START_TIMEOUT_S
                    _log(levels_log, f"round {won_match[1]} won by {won_match[2]}: {_describe_round(round_)}")
                    started, level_log, fighters = None, None, []
            _log(levels_log, "per level:\n" + _make_table(rounds))
    finally:
        for fighter in fighters:
            _stop(fighter)
    if server.wait() != 0 or match_folder is None:
        raise _MatchCheckFailed(f"the match server stopped ({server.returncode}); see server.log")
    return match_folder, rounds


def _describe_round(round_: _round_t) -> str:
    """`_describe_round` gives one round's line for `levels.log`: duration, pylons off, walks, eliminations, respawns.

    Example:
        >>> line = _describe_round(rounds[0])
    """
    level_log: _LevelLog = round_["level_log"]
    times: str = ", ".join(f"{off_s:.1f}" for off_s, _ in level_log.pylons_off)
    delays: str = ", ".join(f"{delay_s:.1f}" for delay_s in level_log.get_respawn_delays_s())
    walks: list[str] = round_["walks"]
    return (
        f"{round_['level']} in {round_['duration_s']:.1f} s; the other team's pylons off: {len(level_log.pylons_off)}, "
        f"at {times} s; on foot: {', '.join(walks) if walks else 'not reported by its bot'}; "
        f"eliminations: {len(level_log.kills)}; respawns after {delays or 'none'} s"
    )


def _make_table(rounds: list[_round_t]) -> str:
    """`_make_table` gives a Markdown table of each round: kills by team, deaths, respawns and walks.

    Example:
        >>> table = _make_table(rounds)
    """
    rows: list[str] = [
        "| Level | Won in | Kills blue / red | Deaths blue / red | Respawns (mean s) | Pylons on foot (s per walk) |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for round_ in rounds:
        level_log: _LevelLog = round_["level_log"]
        teams: dict[int, str] = {id_: team for _, id_, team in level_log.joins}
        kills: list[int] = [
            sum(1 for _, _, shooter in level_log.kills if shooter is not None and teams.get(shooter) == team)
            for team in ("blue", "red")
        ]
        deaths: list[int] = [
            sum(1 for _, victim, _ in level_log.kills if teams.get(victim) == team) for team in ("blue", "red")
        ]
        delays_s: list[float] = level_log.get_respawn_delays_s()
        mean: str = f" ({sum(delays_s) / len(delays_s):.1f})" if delays_s else ""
        walks: str = ", ".join(walk.split(" in ")[1].removesuffix(" s") for walk in round_["walks"]) or "not reported"
        rows.append(
            f"| {round_['level']} | {round_['duration_s']:.1f} s | {kills[0]} / {kills[1]} | {deaths[0]} / {deaths[1]} "
            f"| {len(delays_s)}{mean} | {walks} |"
        )
    return "\n".join(rows)


def _check_match(match_folder: Path, rounds: list[_round_t]) -> None:
    """`_check_match` checks rounds, result files, pylons (reached on foot where bots walk), the fights, the players
    in each level and the outcome against `_PLAN`.

    Args:
        match_folder: The match server's run folder.
        rounds: Each round, from `_watch_match`.

    Raises:
        _MatchCheckFailed: Something differs from `_PLAN`, or a fight check fails.

    Example:
        >>> _check_match(match_folder, rounds)
    """
    played: list[tuple[str, str, str, int]] = [
        (round_["level"], round_["attacker"], round_["winner"], len(round_["level_log"].pylons_off))
        for round_ in rounds
    ]
    if played != _PLAN:
        raise _MatchCheckFailed(f"rounds (level, attacker, winner, pylons off) {played} differ from the plan {_PLAN}")
    for number, round_ in enumerate(rounds, start=1):
        level: str = round_["level"]
        level_log: _LevelLog = round_["level_log"]
        off_consoles: set[int] = {console for _, console in level_log.pylons_off}
        reached: set[int] = {int(walk.split()[1]) for walk in round_["walks"]}
        if level in _WALKING_LEVELS and not off_consoles <= reached:
            missed: list[int] = sorted(off_consoles - reached)
            raise _MatchCheckFailed(f"{level}: pylons {missed} went off but weren't reached on foot")
        result_path: Path = match_folder / f"round_{number}.json"
        if not result_path.exists() or json.loads(result_path.read_text()) != {"winner": round_["winner"]}:
            raise _MatchCheckFailed(f"{result_path} (round {number}, {level}) doesn't say {round_['winner']} won")
        player_count: int = len({id_ for _, id_, _ in level_log.joins})
        expected_count: int = 3 if round_["has_fighters"] else 1
        if player_count != expected_count:
            raise _MatchCheckFailed(f"{player_count} players joined {level} in round {number}, not {expected_count}")
        if not round_["has_fighters"]:
            continue
        delays_s: list[float] = level_log.get_respawn_delays_s()
        if not level_log.kills or not delays_s:
            raise _MatchCheckFailed(f"{level}: {len(level_log.kills)} eliminations and {len(delays_s)} respawns")
        delay_s: float = _get_respawn_delay_s(level)
        if wrong := [d for d in delays_s if not delay_s - _RESPAWN_EARLY_S <= d <= delay_s + _RESPAWN_LATE_S]:
            raise _MatchCheckFailed(f"{level}: respawns after {wrong} s, not about {delay_s} s")
    report: dict[str, Any] = json.loads((match_folder / "match.json").read_text())
    if report["outcome"] != _OUTCOME:
        raise _MatchCheckFailed(f"the match ended {report['outcome']}, not {_OUTCOME}")


def _get_setting_lines(settings_path: Path) -> list[str]:
    """`_get_setting_lines` gives a server settings file's lines without comments, but the console hold time.

    Example:
        >>> lines = _get_setting_lines(_CRATE_FOLDER / "server_settings.yaml")
    """
    lines: list[str] = [line.split("#")[0].rstrip() for line in settings_path.read_text().splitlines()]
    return [line for line in lines if line.strip() and _HOLD_KEY not in line]


def _find_drifted_settings() -> list[str]:
    """`_find_drifted_settings` gives each fast settings file of the levels file that differs from its level's
    `server_settings.yaml` in more than the console hold time, so the test plays each level as it is.

    Example:
        >>> drifted = _find_drifted_settings()
    """
    drifted: list[str] = []
    for block in _LEVEL_BLOCK.findall(_LEVELS_FILE.read_text()):
        if (argument := _SETTINGS_ARGUMENT.search(block)) is None:
            continue
        is_self: bool = _PROGRAM_LINE.search(block)[1] == "self"
        level_folder: Path = _CRATE_FOLDER if is_self else _REPO_FOLDER / _FOLDER_LINE.search(block)[1]
        level_lines: list[str] = _get_setting_lines(level_folder / "server_settings.yaml")
        if level_lines != _get_setting_lines(_CRATE_FOLDER / argument[1]):
            drifted.append(argument[1])
    return drifted


def _validate_args(match_port: int, level_port: int, label: str | None) -> None:
    """`_validate_args` checks that the ports differ, the label, that every program of the match is built and that
    its fast settings files match their levels' settings but for the console hold time.

    Args:
        match_port: `--match-port`.
        level_port: `--level-port`.
        label: `--label`, or `None`.

    Raises:
        click.UsageError: An argument is wrong, or a program isn't built.

    Example:
        >>> _validate_args(48110, 48112, None)
    """
    if match_port == level_port:
        raise click.UsageError(f"--match-port and --level-port must differ, got {match_port}; e.g. 48110 and 48112")
    if label is not None and not re.fullmatch(r"[A-Za-z0-9_-]+", label):
        raise click.UsageError(f"--label must be letters, digits, - and _, got {label!r}; e.g. --label m3-pro")
    level_programs: list[str] = _PROGRAM_LINE.findall(_LEVELS_FILE.read_text())
    programs: list[Path] = [_PROGRAM] + [_CRATE_FOLDER / program for program in level_programs if program != "self"]
    if missing := [str(program) for program in programs if not program.exists()]:
        raise click.UsageError(f"{missing} aren't built; run `make levels && cargo build --release` first")
    if drifted := _find_drifted_settings():
        raise click.UsageError(
            f"{drifted} differ from their levels' server_settings.yaml in more than {_HOLD_KEY}; copy the changes in"
        )


@click.command()
@click.option("--match-port", type=click.IntRange(1024, 65535), default=48110, show_default=True, help="Match port.")
@click.option("--level-port", type=click.IntRange(1024, 65535), default=48112, show_default=True, help="Level port.")
@click.option(
    "--round-timeout-s", type=click.FloatRange(min=1.0), default=600.0, show_default=True, help="Stall after this."
)
@click.option(
    "--fight-s", type=click.FloatRange(min=0.0), default=45.0, show_default=True, help="Fighters' time a round."
)
@click.option("--width-px", type=click.IntRange(64, 7680), default=1280, show_default=True, help="Picture width.")
@click.option("--height-px", type=click.IntRange(64, 4320), default=800, show_default=True, help="Picture height.")
@click.option("--label", type=str, default=None, help="Suffix for the run folder's name.")
@click.option("--dry-run", is_flag=True, help="Print the plan and the commands, and start nothing.")
@click.option("--json", "is_json", is_flag=True, help="Print the summary as one JSON object.")
def main(
    match_port: int, level_port: int, round_timeout_s: float, fight_s: float, width_px: int, height_px: int,
    label: str | None, dry_run: bool, is_json: bool,
) -> None:
    """`main` plays one bot match steered through all five levels and checks it (see the file's docstring).

    Example:
        $ python3 -I e2e/play_all_levels.py --label m3-pro
    """
    _validate_args(match_port, level_port, label)
    run_id: str = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S") + (f"-{label}" if label else "")
    run_folder: Path = _CRATE_FOLDER / ".artifacts" / "play_all_levels" / run_id
    child_label: list[str] = ["--label", f"all-levels-{run_id}"]
    server_arguments: list[str] = [
        "--serve-match", "--match-levels", str(_LEVELS_FILE), "--match-port", str(match_port), "--port",
        str(level_port), "--intermission-s", str(_PAUSE_S), "--round-over-s", str(_PAUSE_S), *child_label,
    ]
    bot_arguments: dict[str, list[str]] = {
        team: [
            "--match", "127.0.0.1", "--match-port", str(match_port), "--bot", "--team", team,
            "--bot-round-winners", ",".join(winner for _, _, winner, _ in _PLAN),
        ]
        for team in ("blue", "red")
    }
    bot_arguments["blue"] += [
        "--progression-pictures", "--width-px", str(width_px), "--height-px", str(height_px), *child_label,
    ]
    if dry_run:
        click.echo(f"plan (level, attacker, winner, pylons a team): {_PLAN}, then {_OUTCOME}; run folder: {run_folder}")
        for arguments in [server_arguments, *bot_arguments.values()]:
            click.echo(" ".join([str(_PROGRAM), *arguments]))
        return
    run_folder.mkdir(parents=True)
    commit: str = subprocess.run(
        ["git", "describe", "--always", "--dirty"], cwd=_CRATE_FOLDER, capture_output=True, text=True, check=True
    ).stdout.strip()
    (run_folder / "command.txt").write_text(f"{' '.join(sys.argv)}\ncommit: {commit}\n")
    processes: list[subprocess.Popen[str]] = []
    failure: str | None = None
    rounds: list[_round_t] = []
    try:
        processes.append(_start(server_arguments, None))
        bots: dict[str, subprocess.Popen[str]] = {
            team: _start(arguments, run_folder / f"{team}.log") for team, arguments in bot_arguments.items()
        }
        processes += bots.values()
        match_folder, rounds = _watch_match(processes[0], bots, run_folder, round_timeout_s, level_port, fight_s)
        _check_match(match_folder, rounds)
        for team, bot in bots.items():
            if bot.wait(timeout=_START_TIMEOUT_S) != 0:
                raise _MatchCheckFailed(f"{team}'s bot stopped ({bot.returncode}); see {team}.log")
        logs: list[Path] = [*run_folder.glob("*.log"), *match_folder.glob("*.log")]
        if failed_logs := [str(log) for log in logs if _FAILURE_LINE.search(log.read_text())]:
            raise _MatchCheckFailed(f"{failed_logs} show a panic or an error")
        picture_folder_match: re.Match[str] | None = _PICTURE_FOLDER_LINE.search((run_folder / "blue.log").read_text())
        if picture_folder_match is None:
            raise _MatchCheckFailed("blue's bot didn't say where its progression pictures are; see blue.log")
        shutil.copytree(picture_folder_match[1], run_folder / "pictures")
        if len(list((run_folder / "pictures").glob("*.png"))) != len(_PLAN) + 1:
            raise _MatchCheckFailed(f"{picture_folder_match[1]} lacks the picture of a round or of the end")
    except _MatchCheckFailed as error:
        failure = str(error)
    finally:
        for process in processes:
            _stop(process)
    rounds_json: list[dict[str, Any]] = [
        {
            **{key: value for key, value in round_.items() if key != "level_log"},
            "pylons_off": round_["level_log"].pylons_off,
            "kills": round_["level_log"].kills,
            "joins": round_["level_log"].joins,
            "respawn_delays_s": round_["level_log"].get_respawn_delays_s(),
        }
        for round_ in rounds
    ]
    summary: dict[str, Any] = {
        "is_passed": failure is None, "failure": failure, "rounds": rounds_json, "run_folder": str(run_folder)
    }
    (run_folder / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    click.echo(json.dumps(summary) if is_json else f"{'FAIL: ' + failure if failure else 'PASS'}; see {run_folder}")
    sys.exit(0 if failure is None else 1)


if __name__ == "__main__":
    main()
