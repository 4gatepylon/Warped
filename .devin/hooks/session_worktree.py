"""`session_worktree.py` gives every Devin session a git worktree of its own, like Claude Code's.

Interface:
    stdin: a Devin `SessionStart` or `SessionEnd` hook's JSON (`hook_event_name`, `session_id`).
    stdout: on `SessionStart`, a `hookSpecificOutput.additionalContext` object for the agent.
    DEVIN_PROJECT_DIR: the folder the session started in, which Devin sets.

Example:
    $ echo '{"hook_event_name": "SessionStart", "session_id": "balanced-starburst"}' \\
        | DEVIN_PROJECT_DIR=~/git/Warped python3 -I .devin/hooks/session_worktree.py

Must know:
    Side effects:
        - A session outside a free worktree gets `.devin/worktrees/agent-<session_id>/`.
        - That worktree is cut from `origin/staging/user-<login>`, with `gh` giving the login.
        - A `devin-session-id` file in a worktree's git folder claims it for one session.
        - `SessionEnd` releases the claim and removes an `agent-` worktree with nothing to lose.
    Gotchas:
        - A hook can't move a session, so a redirected agent passes its worktree as `workdir`.
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

_CLAIM_FILE_NAME: str = "devin-session-id"
_SESSION_ID_PATTERN: re.Pattern[str] = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
_REDIRECT_MESSAGE: str = (
    "WORKTREE WARNING: This Devin session {reason}, not in a worktree of its own, so the hook "
    "`.devin/hooks/session_worktree.py` gave it the worktree `{worktree}` on branch `{branch}`. "
    "At the start of your first response, tell the user this in one sentence. Then work only "
    "there: pass `{worktree}` as `workdir` to every shell command, use absolute paths under it "
    "for every file, and change nothing in `{start_folder}`. Follow the `devin-worktrees` skill."
)
_ERROR_MESSAGE: str = (
    "WORKTREE ERROR: The hook `.devin/hooks/session_worktree.py` couldn't give this Devin "
    "session a worktree of its own ({error}). At the start of your first response, tell the "
    "user this, and change no files until the user says where to work."
)


def _run_git(folder: Path, *args: str) -> str:
    """`_run_git` runs one git command in a folder and returns its stdout, stripped.

    Args:
        folder: The folder git runs in.
        *args: The git command's arguments.

    Raises:
        RuntimeError: git fails; the message holds the command and git's stderr.

    Example:
        >>> _run_git(Path("."), "branch", "--show-current")
    """
    result: subprocess.CompletedProcess[str] = subprocess.run(
        ["git", "-C", str(folder), *args], check=False, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise RuntimeError(f"`git {' '.join(args)}` in {folder}: {result.stderr.strip()}")
    return result.stdout.strip()


def _get_user_staging_branch() -> str:
    """`_get_user_staging_branch` returns the user's staging branch, `staging/user-<login>`.

    Raises:
        RuntimeError: `gh` can't tell who the user is; the message holds its stderr.

    Example:
        >>> _get_user_staging_branch()
        'staging/user-4gatepylon'
    """
    result: subprocess.CompletedProcess[str] = subprocess.run(
        ["gh", "api", "user", "-q", ".login"], check=False, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise RuntimeError(f"`gh api user -q .login`: {result.stderr.strip()}")
    return f"staging/user-{result.stdout.strip()}"


def _find_checkout(folder: Path) -> tuple[Path, Path, Path]:
    """`_find_checkout` returns a folder's checkout: its top folder, git folder and main checkout.

    Args:
        folder: A folder inside a checkout, either the main checkout or a linked worktree.

    Example:
        >>> toplevel, git_dir, main_checkout = _find_checkout(Path("."))
    """
    paths: list[Path] = [Path(line).resolve() for line in _run_git(
        folder, "rev-parse", "--path-format=absolute", "--show-toplevel", "--git-dir",
        "--git-common-dir").splitlines()]
    return paths[0], paths[1], paths[2].parent


def _get_session_worktree(main_checkout: Path, session_id: str) -> Path:
    """`_get_session_worktree` returns where a session's own `agent-` worktree goes.

    Args:
        main_checkout: The repository's main checkout.
        session_id: The session the worktree is for.

    Raises:
        ValueError: `session_id` isn't safe in a folder or branch name.

    Example:
        >>> _get_session_worktree(Path("~/git/Warped"), "balanced-starburst")
    """
    if not _SESSION_ID_PATTERN.fullmatch(session_id):
        raise ValueError(f"session_id {session_id!r} doesn't match {_SESSION_ID_PATTERN.pattern}")
    return main_checkout / ".devin" / "worktrees" / f"agent-{session_id}"


def _find_worktree_git_dir(worktree: Path) -> Path | None:
    """`_find_worktree_git_dir` returns a worktree's own git folder, or None if it doesn't exist.

    Args:
        worktree: The top folder of a linked worktree.

    Raises:
        RuntimeError: `worktree` exists but is no worktree's top folder.

    Example:
        >>> _find_worktree_git_dir(Path(".devin/worktrees/agent-balanced-starburst"))
    """
    if not worktree.exists():
        return None
    toplevel, git_dir, _ = _find_checkout(worktree)
    if toplevel != worktree:
        raise RuntimeError(f"{worktree} exists but isn't a git worktree")
    return git_dir


def _claim_worktree(git_dir: Path, session_id: str) -> str | None:
    """`_claim_worktree` claims a worktree for a session at once, unless another session holds it.

    Args:
        git_dir: The worktree's own git folder, where the claim file goes.
        session_id: The session that wants the worktree.

    Returns:
        None when `session_id` holds the claim now, or else the session that holds it.

    Example:
        >>> _claim_worktree(Path(".git/worktrees/agent-balanced-starburst"), "balanced-starburst")
    """
    try:
        with open(git_dir / _CLAIM_FILE_NAME, "x") as claim_file:
            claim_file.write(session_id)
        return None
    except FileExistsError:
        claim_holder: str = (git_dir / _CLAIM_FILE_NAME).read_text()
        return None if claim_holder == session_id else claim_holder


def _start_session(start_folder: Path, session_id: str) -> str | None:
    """`_start_session` makes sure a session has a worktree of its own.

    Args:
        start_folder: The folder the session started in.
        session_id: The session that starts or resumes.

    Returns:
        None when the session started in a free worktree, or else a warning for the agent.

    Raises:
        RuntimeError: Another session holds this session's `agent-` worktree, or git fails.

    Example:
        >>> _start_session(Path("~/git/Warped"), "balanced-starburst")
    """
    toplevel, git_dir, main_checkout = _find_checkout(start_folder)
    worktree: Path = _get_session_worktree(main_checkout, session_id)
    reason: str = f"started in the main checkout `{toplevel}`"
    if toplevel != main_checkout:
        claim_holder: str | None = _claim_worktree(git_dir, session_id)
        if claim_holder is None:
            return None
        reason = f"started in `{toplevel}`, which Devin session `{claim_holder}` holds"
    worktree_git_dir: Path | None = _find_worktree_git_dir(worktree)
    if worktree_git_dir is None:
        base_branch: str = _get_user_staging_branch()
        _run_git(main_checkout, "fetch", "-q", "origin", base_branch)
        _run_git(main_checkout, "worktree", "add", "-q", "--no-track", "-b",
                 f"worktree-agent-{session_id}", str(worktree), f"origin/{base_branch}")
        worktree_git_dir = _find_checkout(worktree)[1]
    claim_holder = _claim_worktree(worktree_git_dir, session_id)
    if claim_holder is not None:
        raise RuntimeError(f"Devin session `{claim_holder}` holds {worktree}")
    return _REDIRECT_MESSAGE.format(reason=reason, worktree=worktree, start_folder=toplevel,
                                    branch=_run_git(worktree, "branch", "--show-current"))


def _end_session(start_folder: Path, session_id: str) -> None:
    """`_end_session` releases a session's claims and removes its untouched `agent-` worktree.

    Args:
        start_folder: The folder the session started in.
        session_id: The session that ends.

    Example:
        >>> _end_session(Path("~/git/Warped"), "balanced-starburst")
    """
    _, git_dir, main_checkout = _find_checkout(start_folder)
    worktree: Path = _get_session_worktree(main_checkout, session_id)
    worktree_git_dir: Path | None = _find_worktree_git_dir(worktree)
    for temp_git_dir in {git_dir, worktree_git_dir} - {None}:
        claim_path: Path = temp_git_dir / _CLAIM_FILE_NAME
        if claim_path.is_file() and claim_path.read_text() == session_id:
            claim_path.unlink()
    is_merged: bool = worktree_git_dir is not None and subprocess.run(
        ["git", "-C", str(worktree), "merge-base", "--is-ancestor", "HEAD",
         f"origin/{_get_user_staging_branch()}"], check=False, capture_output=True,
    ).returncode == 0
    if is_merged and not _run_git(worktree, "status", "--porcelain"):
        branch: str = _run_git(worktree, "branch", "--show-current")
        # DANGEROUS DANGEROUS DANGEROUS IF YOU ADD --force THEN UNCOMMITTED WORK WILL BE DELETED
        _run_git(main_checkout, "worktree", "remove", str(worktree))
        _run_git(main_checkout, "branch", "-D", branch)


def main() -> None:
    """`main` reads one Devin hook event from stdin and handles it.

    Raises:
        ValueError: The event is neither `SessionStart` nor `SessionEnd`.

    Example:
        >>> main()
    """
    payload: dict = json.load(sys.stdin)
    if payload["hook_event_name"] == "SessionEnd":
        _end_session(Path(os.environ["DEVIN_PROJECT_DIR"]), payload["session_id"])
        return
    if payload["hook_event_name"] != "SessionStart":
        raise ValueError(f"hook_event_name {payload['hook_event_name']!r} isn't a session event")
    try:
        context: str | None = _start_session(Path(os.environ["DEVIN_PROJECT_DIR"]),
                                             payload["session_id"])
    except (KeyError, OSError, RuntimeError, ValueError) as error:
        context = _ERROR_MESSAGE.format(error=f"{type(error).__name__}: {error}")
    if context is not None:
        print(json.dumps({"hookSpecificOutput": {"hookEventName": "SessionStart",
                                                 "additionalContext": context}}))


if __name__ == "__main__":
    main()
