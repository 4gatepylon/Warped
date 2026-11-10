"""`rules.py` holds the rules that need no stored state, as plain functions that tests can call.

Interface:
    RuleError: Raised when a request breaks a rule.
    NotFoundError: Raised when a request names something that doesn't exist.
    MachineLoad: How full one machine is.
    has_room_under_caps: Whether one more match fits under the caps.
    pick_machine: Which machine a new game server goes on.
    count_machines_to_start: How many machines the fleet manager must ask the cloud for.
    split_steps: The moments a tick stops at between two times.

Example:
    >>> pick_machine([MachineLoad("m-1", 4, 4), MachineLoad("m-2", 4, 1)])
    'm-2'
"""

import math
from dataclasses import dataclass


class RuleError(Exception):
    """`RuleError` says which rule a request broke, in words a player or superadmin can read.

    Args:
        message: The rule and what broke it.

    Example:
        >>> raise RuleError("A setting can't be negative.")
    """


class NotFoundError(LookupError):
    """`NotFoundError` names the thing a request asked for that doesn't exist.

    Args:
        message: What was asked for.

    Example:
        >>> raise NotFoundError("There is no setting named max_matches.")
    """


@dataclass(frozen=True)
class MachineLoad:
    """`MachineLoad` says how many of a ready machine's game server slots are in use.

    Args:
        machine_id: The machine.
        slot_count: How many game servers it can run.
        used_slot_count: How many it runs now.

    Example:
        >>> MachineLoad("m-us-east-1", 4, 2)
    """

    machine_id: str
    slot_count: int
    used_slot_count: int


def has_room_under_caps(
    match_count: int, player_count: int, new_player_count: int, settings: dict[str, float]
) -> bool:
    """`has_room_under_caps` says whether one more match fits under the caps.

    Args:
        match_count: How many matches have or await a game server.
        player_count: How many players are in them.
        new_player_count: How many players the new match has.
        settings: The settings, with `max_match_count` and `max_player_count`.

    Returns:
        True if the match count stays at most `max_match_count` and the player count at most
        `max_player_count`.

    Example:
        >>> has_room_under_caps(99, 990, 10, {"max_match_count": 100, "max_player_count": 1000})
        True
    """
    has_match_room: bool = match_count + 1 <= settings["max_match_count"]
    has_player_room: bool = player_count + new_player_count <= settings["max_player_count"]
    return has_match_room and has_player_room


def pick_machine(machine_loads: list[MachineLoad]) -> str | None:
    """`pick_machine` returns the fullest machine that still has a free slot.

    Filling the fullest machine first leaves the emptiest ones to drain, so the fleet manager can
    turn them off.

    Args:
        machine_loads: The ready machines of one region.

    Returns:
        The chosen machine's ID (the first of several equally full), or None if all are full.

    Example:
        >>> pick_machine([MachineLoad("m-1", 4, 0), MachineLoad("m-2", 4, 3)])
        'm-2'
    """
    best: MachineLoad | None = None
    for machine_load in machine_loads:
        has_free_slot: bool = machine_load.used_slot_count < machine_load.slot_count
        if has_free_slot and (best is None or machine_load.used_slot_count > best.used_slot_count):
            best = machine_load
    return None if best is None else best.machine_id


def count_machines_to_start(
    needed_slot_count: int, free_slot_count: int, booting_slot_count: int, slots_per_machine: int
) -> int:
    """`count_machines_to_start` returns how many machines to request from the cloud.

    Args:
        needed_slot_count: Slots wanted: one per match awaiting a game server, plus the spares.
        free_slot_count: Free slots on ready machines.
        booting_slot_count: Slots on machines that are still booting.
        slots_per_machine: How many game servers one machine runs, at least 1.

    Returns:
        The fewest machines that cover the slots still missing, 0 if none are.

    Raises:
        ValueError: `slots_per_machine` is under 1.

    Example:
        >>> count_machines_to_start(7, 1, 4, 4)
        1
    """
    if slots_per_machine < 1:
        raise ValueError(f"slots_per_machine must be at least 1, got {slots_per_machine}")
    missing_slot_count: int = needed_slot_count - free_slot_count - booting_slot_count
    return max(0, math.ceil(missing_slot_count / slots_per_machine))


def split_steps(from_s: float, to_s: float, max_step_s: float, max_step_count: int) -> list[float]:
    """`split_steps` returns the moments a tick stops at on its way from `from_s` to `to_s`.

    A tick after a long skip of the clock walks through the skipped time in steps, so that
    machines boot, matches end and new ones start in order instead of all at once.

    Args:
        from_s: The time of the last tick.
        to_s: The time now, at least `from_s`.
        max_step_s: The longest step, unless that would take more than `max_step_count` steps.
        max_step_count: The most steps to return.

    Returns:
        Increasing times after `from_s`, the last one `to_s`; empty if no time passed.

    Raises:
        ValueError: `to_s` is before `from_s`.

    Example:
        >>> split_steps(0.0, 5.0, 2.0, 100)
        [2.0, 4.0, 5.0]
    """
    if to_s < from_s:
        raise ValueError(f"to_s {to_s} is before from_s {from_s}")
    gap_s: float = to_s - from_s
    if gap_s == 0:
        return []
    step_count: int = min(max_step_count, math.ceil(gap_s / max_step_s))
    step_s: float = gap_s / step_count
    return [from_s + step_s * i for i in range(1, step_count)] + [to_s]
