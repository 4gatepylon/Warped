# Internal API

The management server has three layers, so that each can change without the others:

1. **HTTP API** (`common/protocol/management_server_api.md`): what clients call. It only turns
   HTTP requests into calls to the internal management API and its results into responses.
2. **Internal management API:** the management logic. It owns the records of game servers and
   their players, decides admission, and may later do more than clients ask for, such as keeping
   spare game servers running so new ones start quickly.
3. **Launcher:** where and how game servers run. The first launcher runs local processes; another
   could start EC2 instances. The management server picks one when it starts.

Any game server that follows `common/protocol/game_server_contract.md` works with any launcher.

## Internal management API

For now, there is one operation per HTTP route: create, list, get and delete game servers, and
add, get and remove players. Creating and deleting a game server call the launcher. Getting and
listing game servers ask the launcher for their state and address.

## Launcher

Starting a game server may take minutes, so `launch` returns at once and `get` says when the game
server is running.

- **`launch`:** starts a game server for a game server ID, configuration and maximum number of
  players, without waiting for it to run.
- **`get`:** returns a game server's state (starting, running, stopped or failed) and, once it is
  running, the address clients connect to.
- **`list`:** lists the game servers the launcher has started and not yet stopped, so the
  management server can stop ones it no longer tracks, for example after a restart.
- **`stop`:** stops a game server and frees whatever it ran on. Stopping a game server that is
  already stopped does nothing.
