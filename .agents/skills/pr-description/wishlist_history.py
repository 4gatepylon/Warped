"""`wishlist_history.py` prints the HISTORY section that the next commit on one wishlist item must carry.

Interface:
    WISHLIST: The folder that holds the checklist, relative to the repository root.
    ITEM_ID: The item's id, such as `W-203`.
    SHORT_NAME: The item's short name, the bold title it has in the checklist.
    --rev: The commit to search back from (default HEAD).
    --json: Print `{"section": ..., "previous_commit": ...}` instead of the section.

Example:
    $ python3 wishlist_history.py demos/hyperbolic_wormholes_rust W-203 "pull toward black holes"

Must know:
    Gotchas:
        - `wishlist_history.py` fails when the newest commit naming the item has no single transition line.
"""

import json
import re
import subprocess
from pathlib import Path

import click

_CHECKLIST_FILE_NAME: str = "USER_WISHLIST_CHECKLIST.md"
_HASH_CHAR_COUNT: int = 12
_MAX_BULLET_TEXT_CHAR_COUNT: int = 72
_ELLIPSIS: str = "..."
_ITEM_ID_PATTERN: str = r"W-\d+"
_STATUS_PATTERN: str = r"\[.\]|null"


def _git(*args: str) -> str:
    """`_git` runs one git command in the current directory and returns what it printed.

    Args:
        *args: The git subcommand and its arguments.

    Returns:
        Git's stdout.

    Raises:
        click.ClickException: Git exited with an error.

    Example:
        >>> _git("rev-parse", "--show-toplevel")
    """
    result: subprocess.CompletedProcess[str] = subprocess.run(
        ["git", *args], capture_output=True, text=True
    )
    if result.returncode != 0:
        raise click.ClickException(f"`git {' '.join(args)}` failed: {result.stderr.strip()}")
    return result.stdout


def _validate_args(wishlist: str, item_id: str, short_name: str) -> None:
    """`_validate_args` returns only when the arguments can name an existing wishlist item.

    Args:
        wishlist: The checklist's folder, relative to the repository root.
        item_id: The item's id.
        short_name: The item's short name.

    Raises:
        click.UsageError: An argument is malformed, or the wishlist has no checklist.

    Example:
        >>> _validate_args("demos/hyperbolic_wormholes_rust", "W-203", "pull toward black holes")
    """
    root: Path = Path(_git("rev-parse", "--show-toplevel").strip())
    if wishlist != Path(wishlist).as_posix() or not (root / wishlist / _CHECKLIST_FILE_NAME).is_file():
        raise click.UsageError(
            f"WISHLIST: got {wishlist!r}, expected a folder holding {_CHECKLIST_FILE_NAME}, "
            "relative to the repository root, e.g. demos/hyperbolic_wormholes_rust."
        )
    if re.fullmatch(_ITEM_ID_PATTERN, item_id) is None:
        raise click.UsageError(f"ITEM_ID: got {item_id!r}, expected W- and digits, e.g. W-203.")
    if re.fullmatch(r"[^()\n]+", short_name) is None:
        raise click.UsageError(
            f"SHORT_NAME: got {short_name!r}, expected text without parentheses or line breaks, "
            'e.g. "pull toward black holes".'
        )


def _history_after(commit_hash: str, message: str, item_prefix: str) -> list[str]:
    """`_history_after` returns a commit's own HISTORY bullets plus the bullet for the commit itself.

    Args:
        commit_hash: The commit that wrote `message`.
        message: The commit's full message.
        item_prefix: The start of every line that names the item, `[<wishlist>/<id> (`.

    Returns:
        The bullets of the item's HISTORY section in `message` (none when it has no section), then
        one bullet for the commit's transition line, its text cut to the length limit.

    Raises:
        click.ClickException: `message` has no transition line for the item, or several.

    Example:
        >>> _history_after(commit_hash, message, "[demos/hyperbolic_wormholes_rust/W-203 (")
    """
    lines: list[str] = message.splitlines()
    bullets: list[str] = []
    for index, line in enumerate(lines):
        if line.startswith(item_prefix) and line.endswith("] HISTORY"):
            for bullet in lines[index + 1 :]:
                if bullet.startswith("* "):
                    bullets.append(bullet)
                elif bullet.strip() or bullets:
                    break
            break
    transition_pattern: str = (
        rf"^\* {re.escape(item_prefix)}[^)]*\)\] \(({_STATUS_PATTERN}) -> ({_STATUS_PATTERN})\) "
        r"([A-Z' ]+): (.*)$"
    )
    matches: list[tuple[str, str, str, str]] = re.findall(transition_pattern, message, re.MULTILINE)
    if len(matches) != 1:
        raise click.ClickException(
            f"commit {commit_hash} has {len(matches)} transition lines for {item_prefix}...; "
            "expected exactly 1 (one commit changes one item)."
        )
    from_status, to_status, keyword, text = matches[0]
    if len(text) > _MAX_BULLET_TEXT_CHAR_COUNT:
        text = text[: _MAX_BULLET_TEXT_CHAR_COUNT - len(_ELLIPSIS)] + _ELLIPSIS
    return [*bullets, f"* {commit_hash[:_HASH_CHAR_COUNT]}: ({from_status} -> {to_status}) {keyword}: {text}"]


@click.command(
    epilog='Example: python3 wishlist_history.py demos/hyperbolic_wormholes_rust W-203 "pull toward black holes"'
)
@click.argument("wishlist")
@click.argument("item_id")
@click.argument("short_name")
@click.option("--rev", default="HEAD", show_default=True, help="The commit to search back from.")
@click.option("--json", "should_print_json", is_flag=True, help="Print JSON instead of the section.")
def main(wishlist: str, item_id: str, short_name: str, rev: str, should_print_json: bool) -> None:
    """`main` prints the HISTORY section the next commit on a wishlist item must carry.

    Args:
        wishlist: The checklist's folder, relative to the repository root.
        item_id: The item's id.
        short_name: The item's short name.
        rev: The commit to search back from.
        should_print_json: Whether to print JSON instead of the section.

    Example:
        $ python3 wishlist_history.py demos/hyperbolic_wormholes_rust W-203 "pull toward black holes"
    """
    _validate_args(wishlist, item_id, short_name)
    item_prefix: str = f"[{wishlist}/{item_id} ("
    previous_commit: str = _git(
        "log", "-1", "--fixed-strings", f"--grep={item_prefix}", "--format=%H", rev
    ).strip()
    bullets: list[str] = []
    if previous_commit != "":
        message: str = _git("log", "-1", "--format=%B", previous_commit)
        bullets = _history_after(previous_commit, message, item_prefix)
    header: str = f"[{wishlist}/{item_id} ({short_name})] HISTORY"
    section: str = f"{header}\n\n" + "\n".join(bullets) if bullets else header
    if should_print_json:
        click.echo(
            json.dumps(
                {
                    "section": section,
                    "previous_commit": None if previous_commit == "" else previous_commit,
                }
            )
        )
    else:
        click.echo(section)


if __name__ == "__main__":
    main()
