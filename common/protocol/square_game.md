# Square game

The starter game, not the real one: one square map with a black hole at its center. Players move
their squares and shoot lasers that bend around the black hole. It exists to check that game
servers start, admit players, and let people play together.

## Connecting

A client that the management server added as a player opens a WebSocket to the game server's
`address`: `ws://<host>:<port>/play?player_id=<player_id>`. Every message is a JSON text frame.

The game server closes the WebSocket with:

- `4403` if the management server doesn't know the player;
- `4409` if the player is already connected or the room is full;
- `4400` if a message doesn't match this spec.

## Messages

Server to client:

- `welcome`, once, right after connecting: `{"type": "welcome", "number": integer, "color_hex":
  "#rrggbb", "map_size_m": number, "player_size_m": number, "schwarzschild_radius_m": number,
  "shot_cooldown_s": number}`. `number` identifies the player in this room; player IDs stay
  secret.
- `state`, every tick (30 times a second): `{"type": "state", "players": [{"number", "color_hex",
  "x_m", "y_m"}, ...], "lasers": [{"points_m": [[x, y], ...]}, ...]}`, with every living player
  (its square's center) and the visible part of every laser.

Client to server:

- `move`, whenever the player's direction changes: `{"type": "move", "direction_x": number,
  "direction_y": number}`.
- `shoot`: `{"type": "shoot", "aim_x_m": number, "aim_y_m": number}`, the map point to fire
  toward.

## Rules

- The map runs from (0, 0) to (`map_size_m`, `map_size_m`), with x to the right and y down. The
  black hole sits at its center. Players don't feel it; only lasers do.
- A player gets a random color and a random place away from the black hole when it joins.
- Each tick, a living player moves by 6 m/s times its direction (shortened to length 1 if longer),
  staying inside the map. Squares may overlap.
- A living player may shoot once every `shot_cooldown_s`; the game server ignores earlier shots.
- A laser starts at its shooter's center and grows at 24 m/s along the path that
  `common/physics/README.md` specifies, up to 48 m. It ends where it falls into the black hole or
  leaves the map, and disappears 0.3 s after it has grown completely.
- While it grows, a laser moves rigidly with its shooter: the whole laser moves as the shooter
  moves.
- A laser eliminates every other living player whose square it touches. An eliminated player
  leaves `players`, along with its lasers, but keeps getting `state` until it disconnects.
- A player leaves the room when its WebSocket closes. Joining again makes a new player.
