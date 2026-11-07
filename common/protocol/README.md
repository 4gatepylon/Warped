# Protocol

Every message between clients, the management server and game servers, specified once:

- `management_server_api.md`: the management server's HTTP API (FastAPI), called by clients and
  game servers.
- `game_server_contract.md`: how any game server is started, checked and stopped, and how it
  admits players.
- Real-time play (client ↔ game server): Protocol Buffers (`.proto`), with the transport chosen
  separately.

Each component generates its own code into its git-ignored `.build/`.
