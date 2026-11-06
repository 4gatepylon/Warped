# Management server API

The management server's HTTP API, served with FastAPI. Clients call it to create, find and delete
game servers and to add and remove players. Game servers call it to check who may connect.

**Admission:** a player's ID is a random UUID that only the management server and that player's
client know. When a client connects, the game server looks the ID up with
`GET /game-servers/{game_server_id}/players/{player_id}` and lets the client in only if it is found.
This assumes clients aren't malicious: it keeps out players who weren't added, not ones who steal
an ID.

## Data models

Bodies are JSON. IDs are UUID strings, and times are ISO 8601 strings in UTC.

`GameServerConfig` and `PlayerCreate` are empty objects for now.

`GameServerCreate`:

| Field | Type | Meaning |
| --- | --- | --- |
| `config` | `GameServerConfig` | optional; defaults to `{}` |
| `max_player_count` | integer, at least 1 | how many players it may have at once |

`GameServer`:

| Field | Type | Meaning |
| --- | --- | --- |
| `game_server_id` | UUID | |
| `state` | `"starting"`, `"running"`, `"stopped"` or `"failed"` | as reported by the launcher |
| `address` | `{"host": string, "port": integer}` or `null` | where clients connect; `null` until running |
| `config` | `GameServerConfig` | |
| `max_player_count` | integer | |
| `player_count` | integer | players added and not removed |
| `created_at` | time | |

`GameServerList`:

| Field | Type | Meaning |
| --- | --- | --- |
| `game_servers` | list of `GameServer` | oldest first |
| `next_page_token` | string or `null` | pass as `page_token` to get the next page; `null` on the last |

`Player`:

| Field | Type | Meaning |
| --- | --- | --- |
| `player_id` | UUID | what the player's client presents when it connects |
| `game_server_id` | UUID | |

Errors have FastAPI's default body, `{"detail": ...}`. A request that doesn't match its model
fails with `422`.

## Routes

There are two resources, `/game-servers` and `/game-servers/{game_server_id}/players`. The path
parameters `game_server_id` and `player_id` are UUIDs that the management server assigns.

### `POST /game-servers`

Called by clients. Starts a game server with a configuration (empty for now) and a maximum number
of players, and returns its ID without waiting for it to run, since starting may take minutes.
Once it is running, looking it up gives the address clients connect to.

- **Request:** `GameServerCreate`.
- **Response:** `201` with the `GameServer`, in state `"starting"`.

### `GET /game-servers`

Called by clients. Lists game servers, oldest first, so a player can find one to join. It never
reveals player IDs, since a player ID is what lets a client in.

- **Query parameters,** all optional:
  - `has_open_place` (boolean): if `true`, lists only game servers that aren't full.
  - `limit` (integer, 1 to 100, default 50): lists at most this many game servers.
  - `page_token` (string): the previous page's `next_page_token`, to get the next page.
- **Response:** `200` with a `GameServerList`.

### `GET /game-servers/{game_server_id}`

Called by clients. Returns one game server, including whether it is running yet and, once it is,
its address.

- **Response:** `200` with the `GameServer`.
- **Errors:** `404` if there is no such game server.

### `DELETE /game-servers/{game_server_id}`

Called by clients. Stops the game server and deletes it, along with its players.

- **Response:** `204`.
- **Errors:** `404` if there is no such game server.

### `POST /game-servers/{game_server_id}/players`

Called by clients. Adds a player to the game server and returns the player's ID, which the
player's client presents when it connects.

- **Request:** `PlayerCreate`.
- **Response:** `201` with the `Player`.
- **Errors:** `404` if there is no such game server; `409` if it is full, stopped or failed.

### `GET /game-servers/{game_server_id}/players/{player_id}`

Called by game servers. Returns the player if they were added to this game server and haven't been
removed.

- **Response:** `200` with the `Player`.
- **Errors:** `404` if there is no such player in this game server.

### `DELETE /game-servers/{game_server_id}/players/{player_id}`

Called by clients. Removes the player from the game server, freeing their place, so the game server
no longer lets them connect. A player who is still connected is trusted to leave on their own.

- **Response:** `204`.
- **Errors:** `404` if there is no such player in this game server.
