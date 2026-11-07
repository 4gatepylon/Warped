"""`stand_ins.py` holds a stand-in game server and a port helper that several tests share.

Interface:
    HEALTHY_GAME_SERVER: A command that answers `GET /health` on its `--port`, as the game server
        contract asks, and does nothing else.
    free_port: Returns a TCP port that nothing on this machine listens on right now.

Example:
    >>> port = stand_ins.free_port()
"""

import socket
import sys

HEALTHY_GAME_SERVER: list[str] = [
    sys.executable,
    "-c",
    "import http.server, sys\n"
    "class Handler(http.server.BaseHTTPRequestHandler):\n"
    "    def do_GET(self): self.send_response(200 if self.path == '/health' else 404); "
    "self.end_headers()\n"
    "port = int(sys.argv[sys.argv.index('--port') + 1])\n"
    "http.server.HTTPServer(('0.0.0.0', port), Handler).serve_forever()\n",
]


def free_port() -> int:
    """`free_port` returns a TCP port that nothing on this machine listens on right now.

    Example:
        >>> port = free_port()
    """
    with socket.socket() as probe_socket:
        probe_socket.bind(("127.0.0.1", 0))
        return probe_socket.getsockname()[1]
