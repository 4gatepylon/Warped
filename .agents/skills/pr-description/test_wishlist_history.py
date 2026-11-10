"""Tests for `wishlist_history.py`, which prints an item's next HISTORY section.

Input space: the repository's commits (which of them name the item, and how), the item's id, short name
and wishlist folder, and the output format.

Partitionings:

| Partitioning | Part | Tests |
| --- | --- | --- |
| Earlier commits naming the item | none | `test_no_earlier_commit_prints_the_header_only` |
| | one, without a HISTORY section | `test_earlier_commit_without_history_starts_the_history` |
| | one, with a HISTORY section | `test_earlier_commit_with_history_extends_it` |
| | several, other items' commits between | `test_newest_commit_for_the_item_wins` |
| Earlier commit's text | at most 72 characters | all but the next |
| | longer | `test_long_text_is_cut_with_an_ellipsis` |
| Ids that share a prefix | asked id is a prefix of another item's | `test_id_prefix_does_not_match_another_item` |
| Bad input | item id without `W-` and digits | `test_bad_item_id_is_a_usage_error` |
| | wishlist folder without a checklist | `test_unknown_wishlist_is_a_usage_error` |
| | earlier commit with no transition line | `test_commit_without_transition_line_fails` |
| Output format | text | all but the next |
| | JSON | `test_newest_commit_for_the_item_wins` |
"""

import json
import subprocess
from pathlib import Path

import pytest
from click.testing import CliRunner, Result

from wishlist_history import main

_WISHLIST: str = "demos/x"
_HASH_CHAR_COUNT: int = 12
_HEADER: str = f"[{_WISHLIST}/W-203 (name)] HISTORY"


def _git(*args: str) -> str:
    """Run one git command and return its stdout."""
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout.strip()


def _commit(item_id: str, keyword: str, status: str, text: str, history: str = "") -> str:
    """Commit a message that carries one transition line (and a history) and return the commit hash."""
    line: str = f"* [{_WISHLIST}/{item_id} (name)] {status} {keyword}: {text}"
    section: str = f"\n\n[{_WISHLIST}/{item_id} (name)] HISTORY\n\n{history}" if history else ""
    _git("commit", "--allow-empty", "-q", "-m", f"wishlist({_WISHLIST}): {keyword}\n\n{line}{section}")
    return _git("rev-parse", "HEAD")


def _bullet(commit_hash: str, status: str, keyword: str, text: str) -> str:
    """Return the history bullet that records a commit."""
    return f"* {commit_hash[:_HASH_CHAR_COUNT]}: {status} {keyword}: {text}"


def _run(*extra_args: str, item_id: str = "W-203", wishlist: str = _WISHLIST) -> Result:
    """Run `wishlist_history.py` on the current repository."""
    return CliRunner().invoke(main, [wishlist, item_id, "name", *extra_args])


@pytest.fixture(autouse=True)
def _repo(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Start every test in a new repository whose wishlist folder holds a checklist."""
    monkeypatch.chdir(tmp_path)
    _git("init", "-q")
    _git("config", "user.name", "test")
    _git("config", "user.email", "test@example.com")
    (tmp_path / _WISHLIST).mkdir(parents=True)
    (tmp_path / _WISHLIST / "USER_WISHLIST_CHECKLIST.md").write_text("# checklist\n")
    _git("add", ".")
    _git("commit", "-q", "-m", "start")


def test_no_earlier_commit_prints_the_header_only() -> None:
    result: Result = _run()
    assert (result.exit_code, result.output) == (0, f"{_HEADER}\n")


def test_earlier_commit_without_history_starts_the_history() -> None:
    first: str = _commit("W-203", "ADD TO WISHLIST", "(null -> [ ])", "the words")
    assert _run().output == f"{_HEADER}\n\n{_bullet(first, '(null -> [ ])', 'ADD TO WISHLIST', 'the words')}\n"


def test_earlier_commit_with_history_extends_it() -> None:
    first: str = _commit("W-203", "ADD TO WISHLIST", "(null -> [ ])", "the words")
    old_bullet: str = _bullet(first, "(null -> [ ])", "ADD TO WISHLIST", "the words")
    second: str = _commit("W-203", "CLAIM ON WISHLIST", "([ ] -> [~])", "who", old_bullet)
    new_bullet: str = _bullet(second, "([ ] -> [~])", "CLAIM ON WISHLIST", "who")
    assert _run().output == f"{_HEADER}\n\n{old_bullet}\n{new_bullet}\n"


def test_newest_commit_for_the_item_wins() -> None:
    _commit("W-203", "ADD TO WISHLIST", "(null -> [ ])", "the words")
    _commit("W-204", "ADD TO WISHLIST", "(null -> [ ])", "other")
    newest: str = _commit("W-203", "CLAIM ON WISHLIST", "([ ] -> [~])", "who")
    _commit("W-204", "CLAIM ON WISHLIST", "([ ] -> [~])", "other")
    result: dict[str, str] = json.loads(_run("--json").output)
    assert result["previous_commit"] == newest
    assert result["section"].endswith(_bullet(newest, "([ ] -> [~])", "CLAIM ON WISHLIST", "who"))


def test_long_text_is_cut_with_an_ellipsis() -> None:
    _commit("W-203", "ADD TO WISHLIST", "(null -> [ ])", "x" * 100)
    last_line: str = _run().output.splitlines()[-1]
    assert last_line.endswith("x" * 69 + "...")
    assert len(last_line.split(": ", 2)[2]) == 72


def test_id_prefix_does_not_match_another_item() -> None:
    _commit("W-203", "ADD TO WISHLIST", "(null -> [ ])", "the words")
    assert _run(item_id="W-20").output == f"[{_WISHLIST}/W-20 (name)] HISTORY\n"


def test_bad_item_id_is_a_usage_error() -> None:
    result: Result = _run(item_id="203")
    assert (result.exit_code, "ITEM_ID" in result.output) == (2, True)


def test_unknown_wishlist_is_a_usage_error() -> None:
    result: Result = _run(wishlist="demos/nope")
    assert (result.exit_code, "WISHLIST" in result.output) == (2, True)


def test_commit_without_transition_line_fails() -> None:
    _git("commit", "--allow-empty", "-q", "-m", f"oops\n\n[{_WISHLIST}/W-203 (name)] HISTORY")
    result: Result = _run()
    assert (result.exit_code, "expected exactly 1" in result.output) == (1, True)
