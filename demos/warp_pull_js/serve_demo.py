"""`serve_demo.py` serves the pull-and-sprint tuning page and saves what the page sends.

It serves `index.html` and `pull_dynamics.js` on 127.0.0.1 and writes the page's settings (on every change), its
recordings of play and the settings the user approves into this run's folder, so that the settings a person tunes in
the browser can be read from disk and carried into the levels.

Interface:
    python3 serve_demo.py [--port 48510] [--label <label>]

    Writes to `.artifacts/serve_demo/<run_id>/`: `command.txt`, `settings_latest.json`, `settings_log.jsonl`,
    `recording_<n>.json`, `approved.json`.

Example:
    python3 serve_demo.py --port 48510

Must know:
    - Gotchas: it listens on 127.0.0.1 only, and serves only the two files of the page.
"""

import json
import subprocess
import sys
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import click

_COMPONENT_FOLDER: Path = Path(__file__).resolve().parent
_SERVED_FILES: dict[str, tuple[str, str]] = {
    "/": ("index.html", "text/html; charset=utf-8"),
    "/index.html": ("index.html", "text/html; charset=utf-8"),
    "/pull_dynamics.js": ("pull_dynamics.js", "text/javascript; charset=utf-8"),
}
_LOOPBACK_ADDRESS: str = "127.0.0.1"
_MAX_BODY_BYTES: int = 32 * 1024 * 1024
_FIRST_PORT_INCL: int = 1024
_LAST_PORT_INCL: int = 65535


def _make_run_folder(label: str | None) -> Path:
    """`_make_run_folder` makes this run's folder and writes its `command.txt`.

    Args:
        label: A suffix for the folder's name, or None.

    Returns:
        The folder, `.artifacts/serve_demo/<run_id>/`.

    Example:
        >>> folder = _make_run_folder(None)
    """
    run_id: str = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    if label is not None:
        run_id = f"{run_id}-{label}"
    folder: Path = _COMPONENT_FOLDER / ".artifacts" / "serve_demo" / run_id
    folder.mkdir(parents=True)
    commit: str = subprocess.run(
        ["git", "describe", "--always", "--dirty"], cwd=_COMPONENT_FOLDER, capture_output=True, text=True, check=True
    ).stdout.strip()
    (folder / "command.txt").write_text(f"{' '.join(sys.argv)}\ncommit: {commit}\n")
    return folder


def _make_handler(folder: Path) -> type[BaseHTTPRequestHandler]:
    """`_make_handler` makes the request handler that serves the page and saves its posts into `folder`.

    Args:
        folder: This run's folder.

    Returns:
        The handler class.

    Example:
        >>> server = ThreadingHTTPServer(("127.0.0.1", 48510), _make_handler(folder))
    """

    class _Handler(BaseHTTPRequestHandler):
        """`_Handler` serves the page's two files and saves `/settings`, `/recording` and `/approved` posts."""

        def log_message(self, format: str, *args: object) -> None:
            """`log_message` keeps the server quiet: the run folder holds what matters."""

        def _reply(self, status: int, content_type: str, body: bytes) -> None:
            """`_reply` sends one response."""
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self) -> None:
            """`do_GET` serves one of the page's files, or 404."""
            if self.path not in _SERVED_FILES:
                self._reply(404, "text/plain", b"not found")
                return
            name, content_type = _SERVED_FILES[self.path]
            self._reply(200, content_type, (_COMPONENT_FOLDER / name).read_bytes())

        def do_POST(self) -> None:
            """`do_POST` saves a settings change, a recording or an approval, and answers with where it went."""
            length: int = int(self.headers["Content-Length"])
            if length > _MAX_BODY_BYTES:
                self._reply(413, "text/plain", b"too large")
                return
            body: dict = json.loads(self.rfile.read(length))
            body["received_utc"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
            if self.path == "/settings":
                path: Path = folder / "settings_latest.json"
                with (folder / "settings_log.jsonl").open("a") as log:
                    log.write(json.dumps(body) + "\n")
            elif self.path == "/recording":
                count: int = len(list(folder.glob("recording_*.json")))
                path = folder / f"recording_{count + 1:03d}.json"
            elif self.path == "/approved":
                path = folder / "approved.json"
            else:
                self._reply(404, "text/plain", b"not found")
                return
            path.write_text(json.dumps(body, indent=1))
            answer: dict = {"saved": str(path), "received_utc": body["received_utc"]}
            self._reply(200, "application/json", json.dumps(answer).encode())

    return _Handler


@click.command(epilog="Example: python3 serve_demo.py --port 48510")
@click.option(
    "--port",
    type=click.IntRange(_FIRST_PORT_INCL, _LAST_PORT_INCL),
    default=48510,
    show_default=True,
    help="The TCP port to serve the page on, on 127.0.0.1.",
)
@click.option("--label", type=str, default=None, help="A suffix for the run folder's name.")
def main(port: int, label: str | None) -> None:
    """Serve the pull-and-sprint tuning page on 127.0.0.1 and save what it sends under .artifacts/serve_demo/."""
    if label is not None and not label.replace("-", "").replace("_", "").isalnum():
        raise click.UsageError(f"--label {label!r} must be letters, digits, - and _ (for example --label tuning)")
    folder: Path = _make_run_folder(label)
    server: ThreadingHTTPServer = ThreadingHTTPServer((_LOOPBACK_ADDRESS, port), _make_handler(folder))
    click.echo(f"Page: http://{_LOOPBACK_ADDRESS}:{port}/")
    click.echo(f"Run folder: {folder}")
    server.serve_forever()


if __name__ == "__main__":
    main()
