# Internal API

The management server starts, checks and stops game servers through a launcher, so that where game
servers run can change without changing the HTTP API (`common/protocol/management_server_api.md`).
The launcher is an interface inside the management server, which picks one implementation when it
starts:

- **Local processes** (first): runs each game server as a process on the management server's
  machine.
- **Later, for example:** one EC2 instance per game server.

Starting a game server may take minutes, so `launch` returns at once, and the management server
calls `get` to learn when the game server is running and at what address.

## Operations

### `launch`

Starts a game server for a game server ID, configuration and maximum number of players, without
waiting for it to run. The launcher passes these to the game server, along with the management
server's URL so that it can check players as they connect.

### `get`

Returns a game server's state (starting, running, stopped or failed) and, once it is running, the
address clients connect to.

### `list`

Lists every game server the launcher has started and not yet stopped, so the management server can
find and stop ones it no longer tracks, for example after a restart.

### `stop`

Stops a game server and frees whatever it ran on. Stopping a game server that is already stopped
does nothing.
