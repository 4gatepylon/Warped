"""`play_all_levels.py` plays one bot match steered through all five levels and checks it.

`play_all_levels.py` starts a match server on `e2e/match_levels_fast.yaml` and a bot for each team, steered with
`--bot-round-winners` (the bot of the team that should lose a round joins no level in it), so the match visits every
level in `_PLAN`. It logs each level's start, winner and duration, and when each of the loser's pylons (consoles) went
off, and fails on a stall (a round longer than `--round-timeout-s`), a process that stops early, a round, level
result file, pylon count or outcome that differs from `_PLAN`, a level joined twice, or a panic or error in a log.

Interface:
    --match-port: The match server's UDP port (default 48110, away from the game's 47850).
    --level-port: Each round's level server's UDP port (default 48112, away from the game's 47852).
    --round-timeout-s: Seconds a round may last before it counts as a stall (default 600).
    --width-px, --height-px: Size of the progression pictures (default 1280 x 800).
    --label: Suffix for the run folder's name.
    --dry-run: Print the plan and the commands, and start nothing.
    --json: Print the summary as one JSON object.
    Output: `.artifacts/play_all_levels/<run_id>/` with `command.txt`, `levels.log`, `summary.json`, `server.log`,
    `blue.log`, `red.log` and `pictures/` (blue's progression page at each round's start and at the end).

Example:
    $ make levels && cargo build --release && python3 -I e2e/play_all_levels.py

Must know:
    Limitations:
        - The losing team's bot joins no level in a round, so each round tests one team's level client.
        - Only `_WALKING_LEVELS`' bots report each pylon they reach on foot (`bot reached console ...`), so the
          check that every pylon was reached on foot covers those levels; City Sphere's own bots don't report it.
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
_PYLON_OFF_LINE: re.Pattern[str] = re.compile(r"turned off console \d+")
_JOIN_LINE: re.Pattern[str] = re.compile(r"joined the (?:blue|red) team")
_FAILURE_LINE: re.Pattern[str] = re.compile(r"panicked at|^Error: ", re.MULTILINE)
_WALKING_LEVELS: tuple[str, ...] = ("Lunar Surface", "Snow Planet", "Dust", "Dark Forest")
_ROUND_COMMAND_LINE: re.Pattern[str] = re.compile(r'^round (\d+): "', re.MULTILINE)
_REACHED_LINE: re.Pattern[str] = re.compile(r"^bot reached console (\d+) on foot in ([\d.]+) s", re.MULTILINE)
_round_t = tuple[str, str, str, list[float], list[str]]


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


def _start(arguments: list[str], log_path: Path | None) -> subprocess.Popen[str]:
    """`_start` runs this crate's program with `arguments` in a process group of its own.

    Args:
        arguments: The program's arguments.
        log_path: The file that gets its output, or `None` to pipe it back.

    Returns:
        The running process.

    Example:
        >>> server = _start(["--serve-match"], None)
    """
    command: list[str] = [str(_PROGRAM), *arguments]
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
    """`_stop` stops a process that `_start` started, and everything it started (its process group).

    Example:
        >>> _stop(server)
    """
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=_STOP_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def _note_pylons_off(level_log_path: Path, offset_bytes: int, start_s: float, pylon_off_s: list[float]) -> int:
    """`_note_pylons_off` adds the time of each pylon that a level server's log says went off since `offset_bytes`.

    Args:
        level_log_path: The round's level server log (it may not exist yet).
        offset_bytes: How much of the log was read before.
        start_s: The round's start, on `time.monotonic()`.
        pylon_off_s: Seconds from the round's start to each pylon going off, appended to.

    Returns:
        How much of the log has been read now (whole lines only).

    Example:
        >>> offset_bytes = _note_pylons_off(match_folder / "round_1_server.log", 0, start_s, pylon_off_s)
    """
    if not level_log_path.exists():
        return offset_bytes
    with level_log_path.open("rb") as level_log:
        level_log.seek(offset_bytes)
        text: bytes = level_log.read()
    whole_lines: bytes = text[: text.rfind(b"\n") + 1]
    pylon_off_s += [time.monotonic() - start_s] * len(_PYLON_OFF_LINE.findall(whole_lines.decode()))
    return offset_bytes + len(whole_lines)


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
    server: subprocess.Popen[str], bots: dict[str, subprocess.Popen[str]], run_folder: Path, round_timeout_s: float
) -> tuple[Path, list[_round_t]]:
    """`_watch_match` follows the match server's output until it exits, logging each round to `levels.log`.

    Args:
        server: The match server, its output piped.
        bots: Each team's bot match client.
        run_folder: The run folder, for `server.log` and `levels.log`.
        round_timeout_s: Seconds a round may last before it counts as a stall.

    Returns:
        The match server's run folder, and each round's level, attacker, winner, pylons' times off and the pylons
        its bot reached on foot (`_find_walks`).

    Raises:
        _MatchCheckFailed: A round stalled, or a process stopped before the match was over.

    Example:
        >>> match_folder, rounds = _watch_match(server, bots, run_folder, 600.0)
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
    pylon_off_s: list[float] = []
    offset_bytes: int = 0
    deadline_s: float = time.monotonic() + _START_TIMEOUT_S
    with (run_folder / "levels.log").open("w") as levels_log:
        while True:
            if started is not None and match_folder is not None:
                level_log_path: Path = match_folder / f"round_{len(rounds) + 1}_server.log"
                offset_bytes = _note_pylons_off(level_log_path, offset_bytes, started[2], pylon_off_s)
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
            elif start_match := _START_LINE.match(line):
                started = (start_match[2], _ATTACKER_NAMES[start_match[3]], time.monotonic())
                pylon_off_s, offset_bytes = [], 0
                deadline_s = started[2] + round_timeout_s
                _log(levels_log, f"round {start_match[1]} started: {started[0]}, attacker {started[1]}")
            elif (won_match := _WON_LINE.match(line)) and started is not None:
                duration_s: float = time.monotonic() - started[2]
                level_log_path = match_folder / f"round_{won_match[1]}_server.log"
                _note_pylons_off(level_log_path, offset_bytes, started[2], pylon_off_s)
                walks: list[str] = _find_walks(run_folder, int(won_match[1]), won_match[2])
                rounds.append((started[0], started[1], won_match[2], pylon_off_s, walks))
                deadline_s = time.monotonic() + _START_TIMEOUT_S
                times: str = ", ".join(f"{off_s:.1f}" for off_s in pylon_off_s)
                _log(
                    levels_log,
                    f"round {won_match[1]} won by {won_match[2]}: {started[0]} in {duration_s:.1f} s; "
                    f"the other team's pylons off: {len(pylon_off_s)}, at {times} s; on foot: "
                    f"{', '.join(walks) if walks else 'not reported by its bot'}",
                )
                started = None
    if server.wait() != 0 or match_folder is None:
        raise _MatchCheckFailed(f"the match server stopped ({server.returncode}); see server.log")
    return match_folder, rounds


def _check_match(match_folder: Path, rounds: list[_round_t]) -> None:
    """`_check_match` checks rounds, result files, pylons (on foot where bots walk), one join a level and the
    outcome against `_PLAN`.

    Args:
        match_folder: The match server's run folder.
        rounds: Each round's level, attacker, winner, pylons' times off and walks, from `_watch_match`.

    Raises:
        _MatchCheckFailed: Something differs from `_PLAN`.

    Example:
        >>> _check_match(match_folder, rounds)
    """
    played: list[tuple[str, str, str, int]] = [(*round_[:3], len(round_[3])) for round_ in rounds]
    for level, _, _, pylon_off_s, walks in rounds:
        if level in _WALKING_LEVELS and len(walks) != len(pylon_off_s):
            raise _MatchCheckFailed(f"{level}'s bot reached {len(walks)} of {len(pylon_off_s)} pylons on foot")
    if played != _PLAN:
        raise _MatchCheckFailed(f"rounds (level, attacker, winner, pylons off) {played} differ from the plan {_PLAN}")
    for number, (level, _, winner, _) in enumerate(_PLAN, start=1):
        result_path: Path = match_folder / f"round_{number}.json"
        if not result_path.exists() or json.loads(result_path.read_text()) != {"winner": winner}:
            raise _MatchCheckFailed(f"{result_path} (round {number}, {level}) doesn't say {winner} won")
        level_log: str = (match_folder / f"round_{number}_server.log").read_text()
        if (join_count := len(_JOIN_LINE.findall(level_log))) != 1:
            raise _MatchCheckFailed(f"{join_count} players joined {level} in round {number}, not the one bot")
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
@click.option("--width-px", type=click.IntRange(64, 7680), default=1280, show_default=True, help="Picture width.")
@click.option("--height-px", type=click.IntRange(64, 4320), default=800, show_default=True, help="Picture height.")
@click.option("--label", type=str, default=None, help="Suffix for the run folder's name.")
@click.option("--dry-run", is_flag=True, help="Print the plan and the commands, and start nothing.")
@click.option("--json", "is_json", is_flag=True, help="Print the summary as one JSON object.")
def main(
    match_port: int, level_port: int, round_timeout_s: float, width_px: int, height_px: int, label: str | None,
    dry_run: bool, is_json: bool,
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
        match_folder, rounds = _watch_match(processes[0], bots, run_folder, round_timeout_s)
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
    summary: dict[str, Any] = {
        "is_passed": failure is None, "failure": failure, "rounds": rounds, "run_folder": str(run_folder)
    }
    (run_folder / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    click.echo(json.dumps(summary) if is_json else f"{'FAIL: ' + failure if failure else 'PASS'}; see {run_folder}")
    sys.exit(0 if failure is None else 1)


if __name__ == "__main__":
    main()
