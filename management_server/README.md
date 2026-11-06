# Management server

Starts and stops game servers and keeps track of which players may join each one. Its HTTP API is
specified in `common/protocol/management_server_api.md`, and its internal layers (the management
logic, and the launcher that decides where game servers run) in `internal_api.md`.

A Python (uv) project: `make help` lists its commands, `make test` runs its tests, and
`uv run management-server --help` shows how to run the server.
