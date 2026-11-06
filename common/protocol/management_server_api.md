# Management server API

The management server's HTTP API, served with FastAPI. Clients call it to create and delete game
servers and to add and remove players. Game servers call it to check who may connect.

**Admission:** a player's ID is a random UUID that only the management server and that player's
client know. When a client connects, the game server looks the ID up with
`GET /game-servers/{game_server_id}/players/{player_id}` and lets the client in only if it is found.
This assumes clients aren't malicious: it keeps out players who weren't added, not ones who steal
an ID.

## Routes

### `POST /game-servers`

Called by clients. Starts a game server with a configuration (empty for now) and a maximum number
of players, and returns its ID and the address clients connect to.

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
