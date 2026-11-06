# Management server

Starts and stops game servers and keeps track of which players may join each one. Its HTTP API is
specified in `common/protocol/management_server_api.md`. Game servers are started through a
launcher, specified in `internal_api.md`, so they can run as local processes now and elsewhere
later.
