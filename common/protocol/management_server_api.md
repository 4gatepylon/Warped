# Management server API

The management server's HTTP API, served with FastAPI. Clients call it to create and delete game
servers and to add and remove players. Game servers call it to check who may connect.

**Admission:** a player's ID is a random UUID that only the management server and that player's
client know. When a client connects, the game server looks the ID up with
`GET /game-servers/{game_server_id}/players/{player_id}` and lets the client in only if it is found.
This assumes clients aren't malicious: it keeps out players who weren't added, not ones who steal
an ID.

## Routes

There are two resources, `/game-servers` and `/game-servers/{game_server_id}/players`. The path
parameters `game_server_id` and `player_id` are UUIDs that the management server assigns.

### `POST /game-servers`

Called by clients. Starts a game server with a configuration (empty for now) and a maximum number
of players, and returns its ID without waiting for it to run, since starting may take minutes.
Once it is running, looking it up gives the address clients connect to.

### `GET /game-servers`

Called by clients. Lists game servers, oldest first, so a player can find one to join. It never
reveals player IDs, since a player ID is what lets a client in.

Query parameters, all optional:

- `has_open_place`: if `true`, lists only game servers that aren't full.
- `limit`: lists at most this many game servers (default 50, at most 100).
- `after`: a game server ID from the previous page; lists only game servers created after it.

### `GET /game-servers/{game_server_id}`

Called by clients. Returns one game server, including whether it is running yet and, once it is,
its address. Fails with "not found" if there is no such game server.

### `DELETE /game-servers/{game_server_id}`

Called by clients. Stops the game server and deletes it, along with its players.

### `POST /game-servers/{game_server_id}/players`

Called by clients. Adds a player to the game server and returns the player's ID, which the
player's client presents when it connects. Fails if the game server already has its maximum number
of players.

### `GET /game-servers/{game_server_id}/players/{player_id}`

Called by game servers. Returns the player if they were added to this game server and haven't been
removed, and fails with "not found" otherwise.

### `DELETE /game-servers/{game_server_id}/players/{player_id}`

Called by clients. Removes the player from the game server, freeing their place, so the game server
no longer lets them connect. A player who is still connected is trusted to leave on their own.
