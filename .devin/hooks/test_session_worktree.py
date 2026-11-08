"""`test_session_worktree.py` tests `session_worktree.py` on throwaway git repositories.

Partitionings and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| Folder a session starts in | main checkout | `test_start_in_main_adds_worktree`, `test_resume_in_main_reuses_worktree` |
| | free linked worktree | `test_start_in_free_worktree_claims_it` |
| | worktree this session holds | `test_resume_in_own_worktree_is_silent` |
| | worktree another session holds | `test_start_in_held_worktree_redirects` |
| Session's `agent-` folder at start | absent | `test_start_in_main_adds_worktree` |
| | a worktree | `test_resume_in_main_reuses_worktree` |
| | not a worktree | `test_stray_folder_is_an_error` |
| Main checkout's branch | the user's staging branch | `test_start_in_main_adds_worktree` |
| | another branch, with commits of its own | `test_start_on_other_branch_cuts_from_user_staging` |
| What `gh` says | the user's login | every test but `test_failing_gh_is_an_error` |
| | an error | `test_failing_gh_is_an_error` |
| Session id | safe | every test but `test_unsafe_session_id_is_an_error` |
| | unsafe | `test_unsafe_session_id_is_an_error` |
| Session's `agent-` worktree at end | absent | `test_end_keeps_another_sessions_claim` |
| | clean, in the user's staging branch | `test_end_removes_untouched_worktree` |
| | has a commit not in it | `test_end_keeps_unmerged_commit` |
| | has uncommitted changes | `test_end_keeps_uncommitted_changes` |
| Claim at end | this session's | `test_end_removes_untouched_worktree`, `test_end_keeps_unmerged_commit` |
| | another session's | `test_end_keeps_another_sessions_claim` |

Example:
    $ python3 -I -B -m unittest discover -s .devin/hooks
"""

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

_SCRIPT: Path = Path(__file__).resolve().parent / "session_worktree.py"
_GIT_IDENTITY: list[str] = ["-c", "user.name=test", "-c", "user.email=test@example.com"]
_USER_STAGING_BRANCH: str = "staging/user-tester"
_GH_STUB_FOR_TESTER: str = "#!/bin/sh\necho tester\n"
_GH_STUB_THAT_FAILS: str = "#!/bin/sh\necho 'gh: not logged in' >&2\nexit 1\n"


def _git(folder: Path, *args: str) -> str:
    """`_git` runs one git command in a folder, fails on error, and returns its stdout.

    Example:
        >>> _git(Path("."), "status", "--porcelain")
    """
    return subprocess.run(["git", *_GIT_IDENTITY, "-C", str(folder), *args], check=True,
                          capture_output=True, text=True).stdout


class SessionWorktreeTest(unittest.TestCase):
    """`SessionWorktreeTest` runs the hook in a clone with the user `tester`'s staging branch."""

    def setUp(self) -> None:
        """`setUp` makes the origin, the main checkout `self.main`, and a `gh` that says `tester`."""
        temp_folder: Path = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        _git(temp_folder, "init", "-q", "--bare", "origin.git")
        _git(temp_folder, "clone", "-q", "origin.git", "main")
        self.main: Path = temp_folder / "main"
        (self.main / ".devin").mkdir()
        (self.main / ".devin" / ".gitignore").write_text("worktrees/\n")
        _git(self.main, "checkout", "-q", "-b", _USER_STAGING_BRANCH)
        _git(self.main, "add", "-A")
        _git(self.main, "commit", "-qm", "start")
        _git(self.main, "push", "-q", "origin", _USER_STAGING_BRANCH)
        self.gh_stub: Path = temp_folder / "bin" / "gh"
        self.gh_stub.parent.mkdir()
        self.gh_stub.write_text(_GH_STUB_FOR_TESTER)
        self.gh_stub.chmod(0o755)

    def _run_hook(self, event: str, session_id: str, start_folder: Path) -> str:
        """`_run_hook` runs the hook for one event and returns the agent's context, or ''."""
        stdout: str = subprocess.run(
            ["python3", "-I", str(_SCRIPT)], check=True, capture_output=True, text=True,
            input=json.dumps({"hook_event_name": event, "session_id": session_id}),
            env={**os.environ, "DEVIN_PROJECT_DIR": str(start_folder),
                 "PATH": f"{self.gh_stub.parent}{os.pathsep}{os.environ['PATH']}"}).stdout
        return json.loads(stdout)["hookSpecificOutput"]["additionalContext"] if stdout else ""

    def _get_worktree(self, name: str) -> Path:
        """`_get_worktree` returns where the worktree `name` goes."""
        return self.main / ".devin" / "worktrees" / name

    def _get_claim(self, name: str) -> str:
        """`_get_claim` returns the session that holds the worktree `name`, or ''."""
        claim_path: Path = self.main / ".git" / "worktrees" / name / "devin-session-id"
        return claim_path.read_text() if claim_path.exists() else ""

    def test_start_in_main_adds_worktree(self) -> None:
        """A session in the main checkout gets its own claimed worktree, hidden from git status."""
        context: str = self._run_hook("SessionStart", "alpha", self.main)
        self.assertIn("WORKTREE WARNING", context)
        self.assertIn("branch `worktree-agent-alpha`", context)
        self.assertEqual(self._get_claim("agent-alpha"), "alpha")
        self.assertEqual(_git(self.main, "status", "--porcelain"), "")

    def test_start_on_other_branch_cuts_from_user_staging(self) -> None:
        """A worktree starts at the user's staging branch, whatever the main checkout is on."""
        _git(self.main, "checkout", "-q", "-b", "feature")
        _git(self.main, "commit", "-q", "--allow-empty", "-m", "feature work")
        self._run_hook("SessionStart", "alpha", self.main)
        self.assertEqual(_git(self._get_worktree("agent-alpha"), "rev-parse", "HEAD"),
                         _git(self.main, "rev-parse", f"origin/{_USER_STAGING_BRANCH}"))

    def test_failing_gh_is_an_error(self) -> None:
        """A `gh` that can't tell who the user is is reported."""
        self.gh_stub.write_text(_GH_STUB_THAT_FAILS)
        self.assertIn("gh: not logged in", self._run_hook("SessionStart", "alpha", self.main))

    def test_resume_in_main_reuses_worktree(self) -> None:
        """A resumed session gets the same worktree again."""
        first_context: str = self._run_hook("SessionStart", "alpha", self.main)
        self.assertEqual(self._run_hook("SessionStart", "alpha", self.main), first_context)

    def test_start_in_free_worktree_claims_it(self) -> None:
        """A session in a free worktree claims it and gets no warning."""
        _git(self.main, "worktree", "add", "-q", str(self._get_worktree("named")), "HEAD")
        self.assertEqual(self._run_hook("SessionStart", "alpha", self._get_worktree("named")), "")
        self.assertEqual(self._get_claim("named"), "alpha")

    def test_resume_in_own_worktree_is_silent(self) -> None:
        """A session resumed in the worktree it holds gets no warning."""
        self._run_hook("SessionStart", "alpha", self.main)
        self.assertEqual(self._run_hook("SessionStart", "alpha", self._get_worktree("agent-alpha")), "")

    def test_start_in_held_worktree_redirects(self) -> None:
        """A session in another session's worktree gets a worktree of its own."""
        self._run_hook("SessionStart", "alpha", self.main)
        context: str = self._run_hook("SessionStart", "beta", self._get_worktree("agent-alpha"))
        self.assertIn("which Devin session `alpha` holds", context)
        self.assertEqual(self._get_claim("agent-beta"), "beta")

    def test_stray_folder_is_an_error(self) -> None:
        """A folder that isn't a worktree at the session's `agent-` path is reported."""
        self._get_worktree("agent-alpha").mkdir(parents=True)
        self.assertIn("isn't a git worktree", self._run_hook("SessionStart", "alpha", self.main))

    def test_unsafe_session_id_is_an_error(self) -> None:
        """A session id that could escape the worktrees folder is reported."""
        self.assertIn("WORKTREE ERROR", self._run_hook("SessionStart", "../alpha", self.main))

    def test_end_removes_untouched_worktree(self) -> None:
        """An `agent-` worktree with nothing to lose goes away with its branch."""
        self._run_hook("SessionStart", "alpha", self.main)
        self._run_hook("SessionEnd", "alpha", self.main)
        self.assertFalse(self._get_worktree("agent-alpha").exists())
        self.assertNotIn("worktree-agent-alpha", _git(self.main, "branch"))

    def test_end_keeps_unmerged_commit(self) -> None:
        """An `agent-` worktree with a commit not in `origin/staging` stays, unclaimed."""
        self._run_hook("SessionStart", "alpha", self.main)
        _git(self._get_worktree("agent-alpha"), "commit", "-q", "--allow-empty", "-m", "work")
        self._run_hook("SessionEnd", "alpha", self.main)
        self.assertTrue(self._get_worktree("agent-alpha").exists())
        self.assertEqual(self._get_claim("agent-alpha"), "")

    def test_end_keeps_uncommitted_changes(self) -> None:
        """An `agent-` worktree with uncommitted changes stays."""
        self._run_hook("SessionStart", "alpha", self.main)
        (self._get_worktree("agent-alpha") / "work.txt").write_text("work")
        self._run_hook("SessionEnd", "alpha", self.main)
        self.assertTrue((self._get_worktree("agent-alpha") / "work.txt").exists())

    def test_end_keeps_another_sessions_claim(self) -> None:
        """A session that ends leaves another session's claim alone."""
        self._run_hook("SessionStart", "alpha", self.main)
        self._run_hook("SessionEnd", "beta", self._get_worktree("agent-alpha"))
        self.assertEqual(self._get_claim("agent-alpha"), "alpha")


if __name__ == "__main__":
    unittest.main()
