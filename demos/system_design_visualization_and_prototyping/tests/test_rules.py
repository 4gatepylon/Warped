"""`test_rules.py` tests the plain rules of `system_prototype.rules`.

Partitionings and the tests that cover each part:

| Function | Partitioning | Part | Test |
| --- | --- | --- | --- |
| `has_room_under_caps` | which cap binds | neither | `test_caps_have_room_below_both` |
| | | both reached exactly | `test_caps_have_room_when_exactly_reached` |
| | | the match cap | `test_caps_refuse_one_match_too_many` |
| | | the player cap | `test_caps_refuse_one_player_too_many` |
| `pick_machine` | the machines' free slots | no machines | `test_pick_machine_without_machines` |
| | | every machine full | `test_pick_machine_when_all_full` |
| | | one with room | `test_pick_machine_takes_the_fullest_with_room` |
| | | several equally full | `test_pick_machine_takes_the_first_of_equals` |
| `count_machines_to_start` | missing slots | none | `test_no_machine_when_slots_cover_need` |
| | | booting machines cover them | `test_no_machine_when_booting_ones_cover_need` |
| | | less than one machine's worth | `test_one_machine_for_a_few_missing_slots` |
| | | several machines' worth | `test_machines_round_up` |
| | `slots_per_machine` | under 1 | `test_slots_per_machine_must_be_positive` |
| `split_steps` | the gap | none | `test_no_steps_without_time` |
| | | under one step | `test_one_step_for_a_short_gap` |
| | | several steps | `test_steps_end_exactly_at_the_end` |
| | | more than the step limit allows | `test_steps_stretch_past_the_limit` |
| | | negative | `test_steps_refuse_going_back` |
"""

import pytest

from system_prototype.rules import (
    MachineLoad,
    count_machines_to_start,
    has_room_under_caps,
    pick_machine,
    split_steps,
)

CAPS: dict[str, float] = {"max_match_count": 100, "max_player_count": 1000}


def test_caps_have_room_below_both() -> None:
    """`test_caps_have_room_below_both` checks that a match fits well under both caps."""
    assert has_room_under_caps(10, 100, 10, CAPS)


def test_caps_have_room_when_exactly_reached() -> None:
    """`test_caps_have_room_when_exactly_reached` checks that a match may fill both caps exactly."""
    assert has_room_under_caps(99, 990, 10, CAPS)


def test_caps_refuse_one_match_too_many() -> None:
    """`test_caps_refuse_one_match_too_many` checks that the match cap alone refuses a match."""
    assert not has_room_under_caps(100, 500, 10, CAPS)


def test_caps_refuse_one_player_too_many() -> None:
    """`test_caps_refuse_one_player_too_many` checks that the player cap alone refuses a match."""
    assert not has_room_under_caps(50, 991, 10, CAPS)


def test_pick_machine_without_machines() -> None:
    """`test_pick_machine_without_machines` checks that no machine is picked when there are none."""
    assert pick_machine([]) is None


def test_pick_machine_when_all_full() -> None:
    """`test_pick_machine_when_all_full` checks that no machine is picked when all are full."""
    assert pick_machine([MachineLoad("m-1", 4, 4), MachineLoad("m-2", 2, 2)]) is None


def test_pick_machine_takes_the_fullest_with_room() -> None:
    """`test_pick_machine_takes_the_fullest_with_room` checks that the fullest machine with room
    wins."""
    machine_loads: list[MachineLoad] = [
        MachineLoad("m-1", 4, 0),
        MachineLoad("m-2", 4, 3),
        MachineLoad("m-3", 4, 4),
    ]
    assert pick_machine(machine_loads) == "m-2"


def test_pick_machine_takes_the_first_of_equals() -> None:
    """`test_pick_machine_takes_the_first_of_equals` checks that the first of equally full machines
    wins."""
    assert pick_machine([MachineLoad("m-1", 4, 2), MachineLoad("m-2", 4, 2)]) == "m-1"


def test_no_machine_when_slots_cover_need() -> None:
    """`test_no_machine_when_slots_cover_need` checks that free slots that cover the need start
    nothing."""
    assert count_machines_to_start(3, 3, 0, 4) == 0


def test_no_machine_when_booting_ones_cover_need() -> None:
    """`test_no_machine_when_booting_ones_cover_need` checks that booting machines count as
    cover."""
    assert count_machines_to_start(6, 2, 4, 4) == 0


def test_one_machine_for_a_few_missing_slots() -> None:
    """`test_one_machine_for_a_few_missing_slots` checks that a few missing slots start one
    machine."""
    assert count_machines_to_start(3, 1, 0, 4) == 1


def test_machines_round_up() -> None:
    """`test_machines_round_up` checks that missing slots round up to whole machines."""
    assert count_machines_to_start(12, 1, 2, 4) == 3


def test_slots_per_machine_must_be_positive() -> None:
    """`test_slots_per_machine_must_be_positive` checks that a machine without slots is refused."""
    with pytest.raises(ValueError, match="slots_per_machine"):
        count_machines_to_start(1, 0, 0, 0)


def test_no_steps_without_time() -> None:
    """`test_no_steps_without_time` checks that no time gives no steps."""
    assert split_steps(5.0, 5.0, 2.0, 100) == []


def test_one_step_for_a_short_gap() -> None:
    """`test_one_step_for_a_short_gap` checks that a gap under one step is one step."""
    assert split_steps(0.0, 0.25, 2.0, 100) == [0.25]


def test_steps_end_exactly_at_the_end() -> None:
    """`test_steps_end_exactly_at_the_end` checks that even steps end exactly at the end time."""
    assert split_steps(0.0, 5.0, 2.0, 100) == pytest.approx([5 / 3, 10 / 3, 5.0])


def test_steps_stretch_past_the_limit() -> None:
    """`test_steps_stretch_past_the_limit` checks that a long gap takes at most the step limit."""
    steps: list[float] = split_steps(0.0, 1000.0, 2.0, 10)
    assert len(steps) == 10
    assert steps[-1] == 1000.0


def test_steps_refuse_going_back() -> None:
    """`test_steps_refuse_going_back` checks that a negative gap is refused."""
    with pytest.raises(ValueError, match="before"):
        split_steps(5.0, 4.0, 2.0, 100)
