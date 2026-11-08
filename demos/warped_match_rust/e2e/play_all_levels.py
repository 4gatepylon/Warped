"""`play_all_levels.py` plays one bot match steered through all five levels and checks it.

`play_all_levels.py` starts a match server on `e2e/match_levels_fast.yaml` and a bot for each team, steered with
`--bot-round-winners` (the bot of the team that should lose a round joins no level in it), so the match visits every
level in `_PLAN`. It logs each level's start, winner and duration, and fails on a stall (a round longer than
`--round-timeout-s`), a process that stops early, or a round, level result file or outcome that differs from `_PLAN`.

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
_PROGRAM: Path = _CRATE_FOLDER / ".build" / "release" / "hyperbolic-wormholes"
_LEVELS_FILE: Path = _CRATE_FOLDER / "e2e" / "match_levels_fast.yaml"
_PLAN: list[tuple[str, str, str]] = [
    ("City Sphere", "none", "blue"),
    ("Dark Forest", "blue", "blue"),
    ("Lunar Surface", "blue", "red"),
    ("Snow Planet", "red", "red"),
    ("Dust", "red", "red"),
]
_OUTCOME: dict[str, str] = {"Won": "red"}
_ATTACKER_NAMES: dict[str, str] = {"None": "none", "Some(Blue)": "blue", "Some(Red)": "red"}
_PAUSE_S: float = 2.0
_START_TIMEOUT_S: float = 60.0
_STOP_TIMEOUT_S: float = 10.0
_POLL_S: float = 1.0
_START_LINE: re.Pattern[str] = re.compile(r"^Round (\d+): (.+), attacker (\S+)$")
_WON_LINE: re.Pattern[str] = re.compile(r"^Round (\d+): (blue|red) won (.+)$")
_MATCH_FOLDER_LINE: re.Pattern[str] = re.compile(r"^Run folder: (.+)$")
_PICTURE_FOLDER_LINE: re.Pattern[str] = re.compile(r"^Run folder: (.+/progression_pictures/.+)$", re.MULTILINE)
_PROGRAM_LINE: re.Pattern[str] = re.compile(r"^\s*program: (\S+)$", re.MULTILINE)


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


def _watch_match(
    server: subprocess.Popen[str], bots: dict[str, subprocess.Popen[str]], run_folder: Path, round_timeout_s: float
) -> tuple[Path, list[tuple[str, str, str]]]:
    """`_watch_match` follows the match server's output until it exits, logging each round to `levels.log`.

    Args:
        server: The match server, its output piped.
        bots: Each team's bot match client.
        run_folder: The run folder, for `server.log` and `levels.log`.
        round_timeout_s: Seconds a round may last before it counts as a stall.

    Returns:
        The match server's run folder, and each round's level, attacker and winner.

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
    rounds: list[tuple[str, str, str]] = []
    started: tuple[str, str, float] | None = None
    deadline_s: float = time.monotonic() + _START_TIMEOUT_S
    with (run_folder / "levels.log").open("w") as levels_log:
        while True:
            for team, bot in bots.items():
                if bot.poll() is not None and (bot.returncode != 0 or len(rounds) < len(_PLAN)):
                    raise _MatchCheckFailed(f"{team}'s bot stopped ({bot.returncode}) mid-match; see {team}.log")
            if time.monotonic() > deadline_s:
                level: str = started[0] if started else "before it started"
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
                deadline_s = started[2] + round_timeout_s
                _log(levels_log, f"round {start_match[1]} started: {started[0]}, attacker {started[1]}")
            elif (won_match := _WON_LINE.match(line)) and started is not None:
                duration_s: float = time.monotonic() - started[2]
                rounds.append((started[0], started[1], won_match[2]))
                deadline_s = time.monotonic() + _START_TIMEOUT_S
                _log(levels_log, f"round {won_match[1]} won by {won_match[2]}: {started[0]} in {duration_s:.1f} s")
    if server.wait() != 0 or match_folder is None:
        raise _MatchCheckFailed(f"the match server stopped ({server.returncode}); see server.log")
    return match_folder, rounds


def _check_match(match_folder: Path, rounds: list[tuple[str, str, str]]) -> None:
    """`_check_match` checks the rounds played, each level's result file and the match's outcome against `_PLAN`.

    Args:
        match_folder: The match server's run folder.
        rounds: Each round's level, attacker and winner, from the server's output.

    Raises:
        _MatchCheckFailed: Something differs from `_PLAN`.

    Example:
        >>> _check_match(match_folder, rounds)
    """
    if rounds != _PLAN:
        raise _MatchCheckFailed(f"rounds {rounds} differ from the plan {_PLAN}")
    for number, (level, _, winner) in enumerate(_PLAN, start=1):
        result_path: Path = match_folder / f"round_{number}.json"
        if not result_path.exists() or json.loads(result_path.read_text()) != {"winner": winner}:
            raise _MatchCheckFailed(f"{result_path} (round {number}, {level}) doesn't say {winner} won")
    report: dict[str, Any] = json.loads((match_folder / "match.json").read_text())
    if report["outcome"] != _OUTCOME:
        raise _MatchCheckFailed(f"the match ended {report['outcome']}, not {_OUTCOME}")


def _validate_args(match_port: int, level_port: int, label: str | None) -> None:
    """`_validate_args` checks the ports apart and the label, and that every program of the match is built.

    Raises:
        click.UsageError: An argument is wrong, or a program isn't built.

    Example:
        >>> _validate_args(48110, 48112, None)
    """
    if match_port in (level_port, level_port + 1):
        raise click.UsageError(f"--match-port {match_port} must differ from --level-port and the port after it")
    if label is not None and not re.fullmatch(r"[A-Za-z0-9_-]+", label):
        raise click.UsageError(f"--label must be letters, digits, - and _, got {label!r}; e.g. --label m3-pro")
    level_programs: list[str] = _PROGRAM_LINE.findall(_LEVELS_FILE.read_text())
    programs: list[Path] = [_PROGRAM] + [_CRATE_FOLDER / program for program in level_programs if program != "self"]
    if missing := [str(program) for program in programs if not program.exists()]:
        raise click.UsageError(f"{missing} aren't built; run `make levels && cargo build --release` first")


@click.command()
@click.option("--match-port", type=click.IntRange(1024, 65535), default=48110, show_default=True)
@click.option("--level-port", type=click.IntRange(1024, 65534), default=48112, show_default=True)
@click.option("--round-timeout-s", type=click.FloatRange(min=1.0), default=600.0, show_default=True)
@click.option("--width-px", type=click.IntRange(64, 7680), default=1280, show_default=True)
@click.option("--height-px", type=click.IntRange(64, 4320), default=800, show_default=True)
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
            "--bot-round-winners", ",".join(winner for _, _, winner in _PLAN),
        ]
        for team in ("blue", "red")
    }
    bot_arguments["blue"] += [
        "--progression-pictures", "--width-px", str(width_px), "--height-px", str(height_px), *child_label,
    ]
    if dry_run:
        click.echo(f"plan (level, attacker, winner): {_PLAN}, then {_OUTCOME}; run folder: {run_folder}")
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
    rounds: list[tuple[str, str, str]] = []
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
