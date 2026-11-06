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
add, get and remove players. They take and return the HTTP API's models. Instead of status codes,
they fail with **not found** (no such game server or player) or **unavailable** (adding a player
to a game server that is full, stopped or failed), which the HTTP API turns into `404` and `409`.

Creating and deleting a game server call the launcher. Getting and listing game servers ask the
launcher for their `state` and `address`.

## Launcher

Starting a game server may take minutes, so `launch` returns at once and `get` says when the game
server is running.

| Operation | Takes | Returns |
| --- | --- | --- |
| `launch` | `game_server_id`, `config`, `max_player_count` | nothing, without waiting for it to run |
| `get` | `game_server_id` | a `LaunchedGameServer`, or fails with **not found** |
| `list` | nothing | every `LaunchedGameServer` started and not yet stopped |
| `stop` | `game_server_id` | nothing; does nothing if it is already stopped |

`list` lets the management server stop game servers it no longer tracks, for example after a
restart. `stop` frees whatever the game server ran on.

`LaunchedGameServer`:

| Field | Type | Meaning |
| --- | --- | --- |
| `game_server_id` | UUID | |
| `state` | `"starting"`, `"running"`, `"stopped"` or `"failed"` | |
| `address` | `{"host": string, "port": integer}` or `null` | where clients connect; `null` until running |
| `failure_reason` | string or `null` | why it failed; `null` unless `state` is `"failed"` |

### Local-process launcher

Runs each game server as a process on the management server's machine. Its settings:

- `advertised_host`: the host put in `address`. For LAN play it is the machine's LAN IP, so that
  other machines can connect; `127.0.0.1` keeps play on this machine.
- `ports`: the range it picks a free port from, one per game server.
- `command`: the game server command, run with the contract's flags.

A game server is `"starting"` until `GET /health` answers, then `"running"`. It is `"failed"` if
it doesn't answer within 30 seconds or exits on its own. `stop` sends `SIGTERM`, then `SIGKILL`
after 5 seconds, and leaves it `"stopped"`.
